//! Web-service authentication: password login, session-token gating, persistent pairing token, and devices.
//!
//! - Passwords are verified against a memory-hard Argon2id PHC string; the salt lives inside the PHC
//!   string and no plaintext password is ever persisted. Handlers verify through
//!   [`AuthState::verify_password_async`], which offloads to the blocking pool behind a process-wide
//!   semaphore so the unauthenticated login surface cannot stall the async executor (see `rate_limit`).
//! - Successful HTTP login creates a random in-memory session token returned in JSON. Clients present it
//!   through `Authorization: Bearer` or WebSocket `?token=`. Cookies were removed because domain-wide,
//!   port-agnostic sharing caused windows to overwrite one another's credentials.
//! - The shared pairing-admission token embedded in pairing links persists in the data directory
//!   (`access_store`), so restarting the app or server keeps old links valid. Only the explicit rotate
//!   action replaces the token and invalidates every link.
//! - Clients self-report device ID and name during handshake for a display registry. The registry and the
//!   blocklist persist alongside the token, so a revoked device stays revoked across restarts. Rotation
//!   replaces the shared token and requires every device to reconnect.
//! - Pairing token, registry, and blocklist live in one [`PairingState`] shared per data directory across
//!   every in-process server instance (CLI `--serve` primary, auto-started secondary, GUI/Electron), so a
//!   rotation or revocation on one instance is immediately effective on the others and can no longer be
//!   overwritten by a sister instance's stale write-through (last-writer-wins fix).

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::rate_limit::VERIFY_SEMAPHORE;
use super::{access_store, Ctx};

/// Registered device self-reported during handshake; persisted with the pairing state (`access_store`)
/// so the panel's registry survives restarts. Used for display and revocation bookkeeping.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceEntry {
    pub device_id: String,
    pub name: String,
    pub first_seen_at: u64,
    pub last_seen_at: u64,
}

/// Cap on the persisted device registry: entries are client-reported, so an attacker with a valid pairing
/// token must not be able to grow the file without bound. A new device beyond the cap evicts the entry
/// with the oldest last_seen_at.
const MAX_DEVICES: usize = 32;
/// Self-reported name length cap (characters, multibyte-safe truncation).
const MAX_DEVICE_NAME_CHARS: usize = 64;
/// Self-reported device-ID length cap (characters, multibyte-safe truncation).
const MAX_DEVICE_ID_CHARS: usize = 128;

/// Current Unix time in seconds.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Per-instance authentication state: the instance's own Argon2id verifier and HTTP session tokens, plus
/// a handle to the per-data-dir shared [`PairingState`].
pub struct AuthState {
    /// Argon2id PHC verifier string; immutable for the lifetime of the instance (a restart with a new
    /// password builds a new AuthState). The salt is embedded; no plaintext password is held.
    verifier_phc: String,
    /// HTTP session tokens used by plaintext and legacy authentication paths. Deliberately not
    /// persisted: password-login windows re-authenticate after a restart; only pairing survives.
    tokens: Mutex<HashSet<String>>,
    /// Pairing token, device registry, and blocklist, shared per data directory across instances.
    pairing: Arc<PairingState>,
}

/// Shared pairing state for one data directory: one Mutex serializes every mutation and its write-through
/// persistence, so two in-process server instances can no longer race each other's file writes.
pub struct PairingState {
    inner: Mutex<PairingInner>,
    /// Data directory for write-through persistence of token, registry, and blocklist.
    store_dir: PathBuf,
}

struct PairingInner {
    /// Shared admission token embedded in pairing links, persisted and replaced only by `rotate`.
    pairing_token: String,
    /// Display registry of client-reported devices, persisted with the token.
    devices: Vec<DeviceEntry>,
    /// Blocked device IDs. E2EE handshake rejects reconnection, and heartbeat disconnects active matches.
    /// Persisted so revocations survive restarts; cleared together with the registry on rotation.
    blocked: HashSet<String>,
}

/// Process-wide registry of live pairing stores keyed by canonicalized data directory. Weak entries let a
/// store die with its last server instance, after which a later instance reloads from the file — keeping
/// the file the source of truth across true restarts while sharing state within one process.
static PAIRING_STORES: OnceLock<Mutex<HashMap<PathBuf, Weak<PairingState>>>> = OnceLock::new();

impl PairingState {
    /// Open the shared pairing store for a data directory: return the live in-process instance when one
    /// exists, otherwise load (or create) it from `access_store` and register it.
    ///
    /// Fail-closed keying: the registry key MUST be the canonicalized directory (so `/tmp/x` and
    /// `/private/tmp/x` on macOS resolve to the same store). A silent fallback to the raw path could
    /// hand two instances of the same directory two DIFFERENT stores — exactly the split-brain the
    /// registry exists to prevent — so the directory is created first (canonicalize needs it to
    /// exist) and a canonicalization failure is an error, never a fallback.
    pub fn open(data_dir: &Path) -> Result<Arc<Self>, String> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("failed to create the data directory {}: {e}", data_dir.display()))?;
        let key = data_dir.canonicalize().map_err(|e| {
            format!(
                "failed to resolve the data directory {} for the pairing store: {e}",
                data_dir.display()
            )
        })?;
        let registry = PAIRING_STORES.get_or_init(|| Mutex::new(HashMap::new()));
        let mut map = registry.lock().unwrap();
        // Prune dead entries so the map does not accumulate one Weak per finished server lifetime.
        map.retain(|_, w| w.strong_count() > 0);
        if let Some(existing) = map.get(&key).and_then(Weak::upgrade) {
            return Ok(existing);
        }
        let state = Arc::new(Self::load_or_create(data_dir));
        map.insert(key, Arc::downgrade(&state));
        Ok(state)
    }

    /// Load the persisted pairing token, device registry, and blocklist from the data directory, or
    /// create a fresh token and write the file when it is missing or corrupt (the E2EE key pattern).
    fn load_or_create(data_dir: &Path) -> Self {
        let (pairing_token, devices, blocked) = match access_store::load(data_dir) {
            Some(p) => (
                p.pairing_token,
                p.devices,
                p.blocked_devices.into_iter().collect(),
            ),
            None => {
                let token = new_token();
                if let Err(e) = access_store::save(
                    data_dir,
                    &access_store::PersistedAccess {
                        pairing_token: token.clone(),
                        blocked_devices: Vec::new(),
                        devices: Vec::new(),
                    },
                ) {
                    // Nonfatal: the service still works for this run; only restart persistence degrades.
                    eprintln!("failed to persist remote-access state: {e}");
                }
                (token, Vec::new(), HashSet::new())
            }
        };
        Self {
            inner: Mutex::new(PairingInner {
                pairing_token,
                devices,
                blocked,
            }),
            store_dir: data_dir.to_path_buf(),
        }
    }

    /// Write the current pairing state through to disk. Called with the lock held so concurrent
    /// mutations cannot persist out of order. The in-memory state stays valid even on failure; callers
    /// decide whether the error must surface (rotation/revocation) or is display-only (registration).
    fn persist(&self, inner: &PairingInner) -> Result<(), String> {
        let access = access_store::PersistedAccess {
            pairing_token: inner.pairing_token.clone(),
            blocked_devices: inner.blocked.iter().cloned().collect(),
            devices: inner.devices.clone(),
        };
        access_store::save(&self.store_dir, &access)
    }
}

/// Test-only introspection: whether a live in-process pairing store exists for this directory. Lets
/// restart tests PROVE they exercise the file-reload path rather than the shared in-process state.
#[cfg(test)]
pub(super) fn pairing_store_alive(data_dir: &Path) -> bool {
    let key = data_dir
        .canonicalize()
        .unwrap_or_else(|_| data_dir.to_path_buf());
    PAIRING_STORES
        .get()
        .and_then(|registry| registry.lock().unwrap().get(&key).and_then(Weak::upgrade))
        .is_some()
}

/// Hash a plaintext password into an Argon2id PHC string (default parameters: memory-hard, ~tens of
/// milliseconds per verify). The PHC string is the only password-derived value ever persisted.
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Failed to hash password: {e}"))
}

/// Verify a plaintext password against an Argon2id PHC verifier string.
fn verify_phc(phc: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

impl AuthState {
    /// Build an instance's auth state: the Argon2id PHC verifier comes from the caller and is never
    /// stored in the pairing-state file; pairing token, registry, and blocklist come from the shared
    /// per-data-dir store (loaded from disk only when no in-process instance already holds it).
    pub fn load_or_create(verifier_phc: &str, data_dir: &Path) -> Result<Self, String> {
        Ok(Self {
            verifier_phc: verifier_phc.to_string(),
            tokens: Mutex::new(HashSet::new()),
            pairing: PairingState::open(data_dir)?,
        })
    }

    /// Synchronous password verification, kept for tests only: every request handler must go through
    /// [`verify_password_async`](Self::verify_password_async), which offloads and bounds the Argon2 work.
    #[cfg(test)]
    pub fn verify_password(&self, password: &str) -> bool {
        verify_phc(&self.verifier_phc, password)
    }

    /// Async password verification for request handlers: bounds concurrent Argon2 runs process-wide via
    /// [`VERIFY_SEMAPHORE`] and executes the memory-hard hash on the blocking pool, so the unauthenticated
    /// login surface can never occupy the async executor.
    pub async fn verify_password_async(&self, password: &str) -> bool {
        let Ok(_permit) = VERIFY_SEMAPHORE.acquire().await else {
            // The static semaphore is never closed; treat the impossible case as a failed verify.
            return false;
        };
        let phc = self.verifier_phc.clone();
        let password = password.to_string();
        tokio::task::spawn_blocking(move || verify_phc(&phc, &password))
            .await
            .unwrap_or(false)
    }

    /// Pairing token for this run, embedded in links and checked during handshake.
    pub fn pairing_token(&self) -> String {
        self.pairing.inner.lock().unwrap().pairing_token.clone()
    }

    /// Verify a pairing token against the current value in constant time.
    pub fn validate_pairing_token(&self, token: &str) -> bool {
        let inner = self.pairing.inner.lock().unwrap();
        constant_time_eq(token.as_bytes(), inner.pairing_token.as_bytes())
    }

    /// Rotate the pairing token and clear devices, effectively replacing links for everyone. Existing
    /// connections retain negotiated keys; the new token blocks only new and reconnecting clients. Restart
    /// the service to disconnect all clients immediately. The persisted file is overwritten, so rotation
    /// remains the explicit invalidation path for the restart-surviving token. A persistence failure is
    /// an error: an unpersisted rotation would silently revive the old links after the next restart.
    /// The candidate state is persisted BEFORE memory is touched, so a failed persist leaves memory and
    /// disk consistently on the previous state instead of diverging until the next restart.
    pub fn rotate_pairing_token(&self) -> Result<String, String> {
        let mut inner = self.pairing.inner.lock().unwrap();
        // Full reset: invalidate old links, require every device to pair again, and clear the blocklist.
        let token = new_token();
        let candidate = access_store::PersistedAccess {
            pairing_token: token.clone(),
            blocked_devices: Vec::new(),
            devices: Vec::new(),
        };
        access_store::save(&self.pairing.store_dir, &candidate)
            .map_err(|e| format!("Failed to persist rotated pairing token: {e}"))?;
        // Commit to memory only after the durable write succeeded.
        inner.pairing_token = token.clone();
        inner.devices.clear();
        inner.blocked.clear();
        Ok(token)
    }

    /// Register or update a device after handshake, using placeholders for missing self-reported fields.
    /// Self-reported values are length-capped and the registry size is bounded (see [`MAX_DEVICES`]);
    /// a persistence failure only degrades restart display and never aborts an authenticated handshake.
    pub fn register_device(&self, device_id: Option<&str>, name: Option<&str>) {
        let id = truncate_chars(
            device_id
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("(unknown)"),
            MAX_DEVICE_ID_CHARS,
        );
        let nm = truncate_chars(
            name.map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("Browser"),
            MAX_DEVICE_NAME_CHARS,
        );
        let now = now_secs();
        let mut inner = self.pairing.inner.lock().unwrap();
        // The registry vector is kept in least-recently-seen-first order: refreshing an entry moves it to
        // the back, and the cap evicts from the front. Ordering by position instead of last_seen_at makes
        // eviction deterministic even when many handshakes share the same wall-clock second.
        if let Some(pos) = inner.devices.iter().position(|d| d.device_id == id) {
            let mut d = inner.devices.remove(pos);
            d.last_seen_at = now;
            d.name = nm;
            inner.devices.push(d);
        } else {
            // Cap the persisted registry: evict the least recently seen device to make room.
            while inner.devices.len() >= MAX_DEVICES {
                inner.devices.remove(0);
            }
            inner.devices.push(DeviceEntry {
                device_id: id,
                name: nm,
                first_seen_at: now,
                last_seen_at: now,
            });
        }
        if let Err(e) = self.pairing.persist(&inner) {
            eprintln!("failed to persist remote-access state: {e}");
        }
    }

    /// List devices registered during this run, distinguished by self-reported identifiers.
    pub fn list_devices(&self) -> Vec<DeviceEntry> {
        self.pairing.inner.lock().unwrap().devices.clone()
    }

    /// Block a device by ID and remove it from the display registry. [`is_blocked`] rejects its E2EE
    /// handshake even with valid credentials, while heartbeat disconnects an existing connection. Other
    /// devices are unaffected. Returns whether it was registered. IDs are self-reported and spoofable.
    /// A persistence failure is an error: an unpersisted revocation would be undone by the next restart.
    /// The candidate state is persisted BEFORE memory is touched, so a failed persist leaves memory and
    /// disk consistently on the previous state (the device stays unblocked and visible) instead of an
    /// in-memory revocation that silently evaporates on restart.
    pub fn block_device(&self, device_id: &str) -> Result<bool, String> {
        let mut inner = self.pairing.inner.lock().unwrap();
        let mut blocked = inner.blocked.clone();
        blocked.insert(device_id.to_string());
        let devices: Vec<DeviceEntry> = inner
            .devices
            .iter()
            .filter(|d| d.device_id != device_id)
            .cloned()
            .collect();
        let candidate = access_store::PersistedAccess {
            pairing_token: inner.pairing_token.clone(),
            blocked_devices: blocked.iter().cloned().collect(),
            devices: devices.clone(),
        };
        access_store::save(&self.pairing.store_dir, &candidate)
            .map_err(|e| format!("Failed to persist device revocation: {e}"))?;
        // Commit to memory only after the durable write succeeded.
        let was_registered = devices.len() < inner.devices.len();
        inner.blocked = blocked;
        inner.devices = devices;
        Ok(was_registered)
    }

    /// Whether a device ID is blocked, shared by handshake rejection and heartbeat eviction.
    pub fn is_blocked(&self, device_id: &str) -> bool {
        self.pairing.inner.lock().unwrap().blocked.contains(device_id)
    }

    fn mint(&self) -> String {
        let token = Uuid::new_v4().to_string();
        self.tokens.lock().unwrap().insert(token.clone());
        token
    }

    fn check(&self, token: &str) -> bool {
        self.tokens.lock().unwrap().contains(token)
    }

    /// Validate a raw session token for WebSocket `?token=` authentication. Browser WebSocket APIs cannot
    /// set custom headers, so this checks the same login-issued value stored in the token set.
    pub fn token_valid(&self, token: &str) -> bool {
        self.check(token)
    }

    fn revoke(&self, token: &str) {
        self.tokens.lock().unwrap().remove(token);
    }
}

/// Truncate a string to at most `max` characters on a char boundary (multibyte-safe).
fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Generate an unpredictable token by joining two simple UUID strings.
fn new_token() -> String {
    Uuid::new_v4().simple().to_string() + &Uuid::new_v4().simple().to_string()
}

/// Constant-time comparison for equal-length values to avoid hash timing side channels.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Extract a session token only from `Authorization: Bearer`. Cookies were removed on 2026-07-03; each
/// window holds and presents its own credential through sessionStorage or mobile memory, eliminating
/// cross-window overwrites and WKWebView cookie timing issues. Browser WebSockets use `?token=` because
/// their API cannot set custom headers.
pub(super) fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let t = auth.strip_prefix("Bearer ")?.trim();
    if t.is_empty() {
        return None;
    }
    Some(t.to_string())
}

/// Unified authentication gate for `/ws` and data endpoints.
pub fn is_authed(ctx: &Ctx, headers: &HeaderMap) -> bool {
    token_from_headers(headers)
        .map(|t| ctx.auth.check(&t))
        .unwrap_or(false)
}

#[derive(serde::Deserialize)]
pub struct LoginBody {
    password: String,
}

/// Verify login password, issue a token, and return it in a JSON body as `{"token":"…"}`.
///
/// The token is the sole credential. Web clients keep it in per-window sessionStorage and mobile clients
/// in memory, presenting it in WebSocket URLs or HTTP headers. No cookie is issued.
///
/// This endpoint is reachable unauthenticated, so it is rate-limited per IP **before** any Argon2 work,
/// and the verification itself runs on the blocking pool behind the process-wide semaphore.
pub async fn login(
    State(ctx): State<Ctx>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<LoginBody>,
) -> impl IntoResponse {
    let ip = addr.ip();
    // The RAII guard keeps the reservation alive across the Argon2 await; if the client disconnects
    // and this future is cancelled mid-verify, dropping the guard releases the slot immediately.
    let Some(attempt) = ctx.limiter.allow(ip) else {
        // The frontend maps 429 to its own localized message; this body is a log/curl-facing fallback.
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "Too many failed login attempts. Try again later.",
        )
            .into_response();
    };
    if !ctx.auth.verify_password_async(&body.password).await {
        attempt.failure();
        return (StatusCode::UNAUTHORIZED, "Wrong password").into_response();
    }
    attempt.success();
    let token = ctx.auth.mint();
    Json(serde_json::json!({ "token": token })).into_response()
}

/// Check whether the current request is authenticated when the frontend enters the page.
pub async fn me(State(ctx): State<Ctx>, headers: HeaderMap) -> impl IntoResponse {
    if is_authed(&ctx, &headers) {
        StatusCode::OK
    } else {
        StatusCode::UNAUTHORIZED
    }
}

/// Log out by revoking the bearer token. Each window clears its local copy; sessionStorage also expires
/// when the window closes.
pub async fn logout(State(ctx): State<Ctx>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(t) = token_from_headers(&headers) {
        ctx.auth.revoke(&t);
    }
    "ok".into_response()
}

#[cfg(test)]
mod tests {
    use super::{hash_password, AuthState, MAX_DEVICES};
    use std::path::PathBuf;

    /// Unique per test-invocation: the in-process pairing-store registry is keyed by directory, so two
    /// tests (or two runs) must never share a directory unless sharing is exactly what they assert.
    fn tempdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vlx-auth-{tag}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Test constructor: hash the plaintext once and build the state in a tag-specific tempdir.
    fn new_auth(password: &str, dir: &PathBuf) -> AuthState {
        AuthState::load_or_create(&hash_password(password).unwrap(), dir)
            .expect("opening the pairing store in a tempdir must succeed")
    }

    #[test]
    fn password_verify_and_token_lifecycle() {
        let dir = tempdir("pw");
        let auth = new_auth("s3cret", &dir);
        // Password verification against the Argon2id PHC verifier.
        assert!(auth.verify_password("s3cret"));
        assert!(!auth.verify_password("wrong"));
        assert!(!auth.verify_password(""));

        // Session-token lifecycle: issue, validate, revoke, invalidate.
        let token = auth.mint();
        assert!(auth.check(&token));
        assert!(!auth.check("not-a-token"));
        auth.revoke(&token);
        assert!(!auth.check(&token));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The async verify path (semaphore + spawn_blocking) matches the sync verifier's verdicts.
    #[test]
    fn verify_password_async_matches_sync() {
        let dir = tempdir("async");
        let auth = new_auth("s3cret", &dir);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(rt.block_on(auth.verify_password_async("s3cret")));
        assert!(!rt.block_on(auth.verify_password_async("wrong")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pairing_token_rotate_and_device_registry() {
        let dir = tempdir("rotate");
        let auth = new_auth("pw", &dir);
        // The run's pairing token is stable and verifiable.
        let tok = auth.pairing_token();
        assert!(auth.validate_pairing_token(&tok));
        assert!(!auth.validate_pairing_token("nope"));

        // Register two devices.
        auth.register_device(Some("dev-a"), Some("Mac"));
        auth.register_device(Some("dev-b"), Some("Phone"));
        assert_eq!(auth.list_devices().len(), 2);
        // Registering the same ID updates rather than duplicates it.
        auth.register_device(Some("dev-a"), Some("Mac mini"));
        assert_eq!(auth.list_devices().len(), 2);

        // Rotation invalidates the old token and clears the registry.
        let tok2 = auth.rotate_pairing_token().unwrap();
        assert_ne!(tok, tok2);
        assert!(!auth.validate_pairing_token(&tok));
        assert!(auth.validate_pairing_token(&tok2));
        assert_eq!(auth.list_devices().len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn block_device_blocks_and_rotate_clears() {
        let dir = tempdir("block");
        let auth = new_auth("pw", &dir);
        auth.register_device(Some("dev-a"), Some("Mac"));
        auth.register_device(Some("dev-b"), Some("Phone"));

        // Neither device is initially blocked.
        assert!(!auth.is_blocked("dev-a"));
        assert!(!auth.is_blocked("dev-b"));

        // Blocking dev-a returns true, adds it to the blocklist, removes it from display, and spares dev-b.
        assert!(auth.block_device("dev-a").unwrap());
        assert!(auth.is_blocked("dev-a"));
        assert!(!auth.is_blocked("dev-b"));
        assert_eq!(auth.list_devices().len(), 1);
        assert_eq!(auth.list_devices()[0].device_id, "dev-b");

        // Blocking an unknown ID returns false but still rejects future handshakes using it.
        assert!(!auth.block_device("dev-x").unwrap());
        assert!(auth.is_blocked("dev-x"));

        // Rotation fully resets the blocklist as well.
        auth.rotate_pairing_token().unwrap();
        assert!(!auth.is_blocked("dev-a"));
        assert!(!auth.is_blocked("dev-x"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two live instances over the same data directory share ONE pairing store: rotation and revocation
    /// on one are immediately effective on the other, so a sister instance can no longer re-persist stale
    /// state over them (the dual-instance --serve fix of issue #24).
    #[test]
    fn live_instances_share_pairing_state() {
        let dir = tempdir("shared");
        let phc = hash_password("pw").unwrap();

        let a = AuthState::load_or_create(&phc, &dir).unwrap();
        let b = AuthState::load_or_create(&phc, &dir).unwrap();

        // Registration on B is visible on A.
        b.register_device(Some("dev-a"), Some("Phone"));
        assert_eq!(a.list_devices().len(), 1);

        // Revocation on A is immediately effective on B.
        assert!(a.block_device("dev-a").unwrap());
        assert!(b.is_blocked("dev-a"));

        // Rotation on A: B validates only the new token from then on.
        let old = b.pairing_token();
        let rotated = a.rotate_pairing_token().unwrap();
        assert!(!b.validate_pairing_token(&old));
        assert!(b.validate_pairing_token(&rotated));
        assert_eq!(b.pairing_token(), rotated);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Token, device registry, and blocklist survive a "restart" (a second AuthState from the same
    /// directory **after the first is dropped**, so the state genuinely reloads from the file rather
    /// than from the shared in-process store), which is the invariant GitHub issue #15 demands.
    #[test]
    fn pairing_state_persists_across_instances() {
        let dir = tempdir("persist");
        let phc = hash_password("pw").unwrap();

        let a = AuthState::load_or_create(&phc, &dir).unwrap();
        let token = a.pairing_token();
        a.register_device(Some("dev-a"), Some("Phone"));
        a.register_device(Some("dev-b"), Some("Tablet"));
        assert!(a.block_device("dev-b").unwrap());
        // Drop the first instance so its shared store dies and the next load takes the FILE path.
        drop(a);

        // A fresh instance from the same data dir sees the same token, registry, and blocklist.
        let b = AuthState::load_or_create(&phc, &dir).unwrap();
        assert_eq!(b.pairing_token(), token);
        assert!(b.validate_pairing_token(&token));
        assert_eq!(b.list_devices().len(), 1);
        assert_eq!(b.list_devices()[0].device_id, "dev-a");
        assert!(b.is_blocked("dev-b"));
        assert!(!b.is_blocked("dev-a"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Rotation overwrites the persisted file: a later instance (first one dropped, file path proven)
    /// sees the new token and empty blocklist.
    #[test]
    fn rotate_persists_new_token_and_clears_state() {
        let dir = tempdir("rotate-persist");
        let phc = hash_password("pw").unwrap();

        let a = AuthState::load_or_create(&phc, &dir).unwrap();
        let old = a.pairing_token();
        a.register_device(Some("dev-a"), Some("Phone"));
        a.block_device("dev-x").unwrap();
        let rotated = a.rotate_pairing_token().unwrap();
        assert_ne!(old, rotated);
        drop(a);

        let b = AuthState::load_or_create(&phc, &dir).unwrap();
        assert_eq!(b.pairing_token(), rotated);
        assert!(!b.validate_pairing_token(&old));
        assert!(b.list_devices().is_empty());
        assert!(!b.is_blocked("dev-x"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Registry caps: the device count is bounded (oldest last_seen_at evicted) and self-reported
    /// name/ID are truncated multibyte-safely.
    #[test]
    fn device_registry_is_capped_and_fields_truncated() {
        let dir = tempdir("caps");
        let auth = new_auth("pw", &dir);
        for i in 0..MAX_DEVICES {
            auth.register_device(Some(&format!("dev-{i:03}")), Some("Phone"));
        }
        assert_eq!(auth.list_devices().len(), MAX_DEVICES);
        // Refresh dev-000 so it is no longer the oldest; the next new device evicts dev-001 instead.
        auth.register_device(Some("dev-000"), Some("Phone"));
        auth.register_device(Some("dev-new"), Some("Phone"));
        let devices = auth.list_devices();
        assert_eq!(devices.len(), MAX_DEVICES);
        assert!(devices.iter().any(|d| d.device_id == "dev-new"));
        assert!(devices.iter().any(|d| d.device_id == "dev-000"));
        assert!(!devices.iter().any(|d| d.device_id == "dev-001"));

        // Over-long multibyte name and ID are truncated on char boundaries, never panicking.
        let long_name = "ü".repeat(300);
        let long_id = "漢".repeat(300);
        auth.register_device(Some(&long_id), Some(&long_name));
        let stored = auth
            .list_devices()
            .into_iter()
            .find(|d| d.device_id.starts_with('漢'))
            .expect("truncated device must be registered");
        assert_eq!(stored.device_id.chars().count(), 128);
        assert_eq!(stored.name.chars().count(), 64);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Persistence failures surface to the caller for the operations whose loss would be a security
    /// regression after restart: revocation and rotation. Persist-first ordering: on failure the
    /// in-memory state must be UNCHANGED, so memory and disk stay consistent on the previous state
    /// instead of an in-memory-only revocation/rotation that evaporates on restart.
    #[test]
    fn block_and_rotate_surface_persist_failures_and_leave_memory_unchanged() {
        let dir = tempdir("persist-err");
        let auth = new_auth("pw", &dir);
        auth.register_device(Some("dev-a"), Some("Phone"));
        let token_before = auth.pairing_token();
        // Make the store directory unusable: replace it with a regular file so the tmp-file write fails.
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::write(&dir, b"not a directory").unwrap();

        assert!(auth.block_device("dev-a").is_err());
        // Memory rolled with the failure: the device is neither blocked nor deregistered.
        assert!(!auth.is_blocked("dev-a"), "a failed persist must not block in memory only");
        assert_eq!(auth.list_devices().len(), 1, "a failed persist must not deregister in memory");

        assert!(auth.rotate_pairing_token().is_err());
        // Memory keeps the old token: a memory-only rotation would strand every existing link
        // while the disk still admits the old one after restart.
        assert!(
            auth.validate_pairing_token(&token_before),
            "a failed persist must not rotate the token in memory only"
        );
        let _ = std::fs::remove_file(&dir);
    }

    /// Fail-closed registry keying: a data directory that cannot be created/canonicalized (its parent
    /// is a regular file) is an error — never a silent raw-path fallback, which could hand two
    /// instances of the same directory two different pairing stores.
    #[test]
    fn open_fails_closed_when_the_directory_cannot_be_resolved() {
        let parent = tempdir("open-fail");
        let file = parent.join("occupied");
        std::fs::write(&file, b"a file, not a directory").unwrap();
        let unresolvable = file.join("data");
        assert!(
            AuthState::load_or_create(&hash_password("pw").unwrap(), &unresolvable).is_err(),
            "an unresolvable data dir must be an error, not a raw-path fallback"
        );
        let _ = std::fs::remove_dir_all(&parent);
    }

    /// hash_password produces a PHC verifier that accepts the original password and rejects others.
    #[test]
    fn hash_password_roundtrip() {
        let dir = tempdir("hash");
        let phc = hash_password("correct horse").unwrap();
        assert!(phc.starts_with("$argon2id$"), "expected Argon2id PHC, got: {phc}");
        let auth = AuthState::load_or_create(&phc, &dir).unwrap();
        assert!(auth.verify_password("correct horse"));
        assert!(!auth.verify_password("battery staple"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
