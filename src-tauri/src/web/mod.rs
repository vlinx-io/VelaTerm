//! Browser remote-access service hosting frontend assets and bridging commands, events, and PTY streams over WebSocket.
//!
//! Runs axum on a Tokio runtime in a dedicated thread, isolated from Tauri's main event loop. A shared `AppCtx`
//! resolves PtyManager, Db, and HookServer and exchanges events, so browser and desktop/headless clients naturally
//! use the same PTY manager and SQLite database (see `host.rs`).
//!
//! - LAN mode binds `0.0.0.0` and authenticates through the password login flow (see `auth`).
//! - `rust-embed` bundles `../dist` for offline release use.
//! - `/ws` multiplexes invokes, events, and PTY traffic (see `ws`).

mod access_store;
mod auth;
// desktop_call also uses dispatch, so expose it within the crate rather than keeping it private to web transport.
pub(crate) mod dispatch;
mod e2ee;
mod rate_limit;
mod sniff;
mod tls;
pub mod tunnel;
mod ws;

use std::sync::Mutex;

use axum::extract::State;
use axum::http::{header, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;

use crate::host::AppCtx;

use auth::AuthState;
use base64::Engine;
use std::sync::Arc;

/// Default listening port, overridable by `web_server_start`.
const DEFAULT_PORT: u16 = 8799;

/// Frontend build output at `src-tauri/../dist`, read from disk in debug and embedded in release.
#[derive(rust_embed::Embed)]
#[folder = "../dist"]
struct Assets;

/// Context shared by axum handlers.
#[derive(Clone)]
pub(crate) struct Ctx {
    pub app: AppCtx,
    pub auth: Arc<AuthState>,
    /// Long-lived server E2EE key used for handshake ECDH; its public key is embedded in pairing URLs.
    pub e2ee_keys: Arc<e2ee::ServerKeys>,
    /// Serve mode used by WS to require paired encryption and reject plaintext in network-exposed LanTls mode.
    pub mode: ServeMode,
    /// Failed-login limiter, checked before any Argon2 work on `/api/login` and the WS handshake.
    /// Shared per data directory across in-process instances (see `LoginRateLimiter::shared`), so a
    /// dual-instance `--serve` setup cannot double the per-IP attempt budget.
    pub limiter: Arc<rate_limit::LoginRateLimiter>,
}

/// Public web-service status returned to the frontend in camelCase.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebServerStatus {
    pub running: bool,
    pub port: Option<u16>,
    /// Primary candidate URL, the first in `urls`, preferring a normal LAN interface.
    pub url: Option<String>,
    /// All candidate URLs across Wi-Fi, Ethernet, and VPN interfaces, with point-to-point tunnels ranked last.
    pub urls: Vec<String>,
    /// Colon-separated uppercase SHA-256 fingerprint of the self-signed certificate for host verification; None when stopped.
    pub fingerprint: Option<String>,
    /// Error message of the most recent failed auto-start (e.g. port in use); cleared by any successful start.
    pub autostart_error: Option<String>,
    /// Last persisted port from app settings, used by the panel to prefill the port field; filled in command_core.
    pub saved_port: Option<u16>,
    /// Whether the persisted enabled flag would auto-start the service on next launch; filled in command_core.
    pub auto_start: bool,
    /// URL scheme of the running service ("https" for LanTls, "http" for the plaintext modes); None when
    /// stopped. Explicit so the frontend can synthesize a URL for an interface that appeared after start
    /// (absent from the `urls` snapshot) without guessing the scheme from the snapshot's first entry.
    pub scheme: Option<String>,
}

impl WebServerStatus {
    fn stopped() -> Self {
        Self {
            running: false,
            port: None,
            url: None,
            urls: Vec::new(),
            fingerprint: None,
            autostart_error: None,
            saved_port: None,
            auto_start: false,
            scheme: None,
        }
    }
}

/// Web serving modes separating bind scope from encryption, unlike the former `local_http` boolean. This enables
/// the plaintext-LAN combination required by native mobile shells.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ServeMode {
    /// LAN with self-signed TLS on `0.0.0.0`, including HTTP-to-HTTPS sniffing redirect; desktop remote default.
    LanTls,
    /// Plain HTTP/ws on `127.0.0.1` for Electron sidecar; loopback never leaves the host.
    LoopbackHttp,
    /// Plain HTTP/ws on LAN for native mobile shells whose RN WebView cannot bypass self-signed certificates,
    /// especially Android. This sacrifices LAN encryption (architecture section 20).
    LanHttp,
}

impl ServeMode {
    /// Whether transport is plaintext; only `LanTls` uses TLS.
    fn plaintext(self) -> bool {
        !matches!(self, ServeMode::LanTls)
    }
    /// Bind address octets: LoopbackHttp uses 127.0.0.1; both LAN modes use 0.0.0.0.
    fn bind_octets(self) -> [u8; 4] {
        match self {
            ServeMode::LoopbackHttp => [127, 0, 0, 1],
            _ => [0, 0, 0, 0],
        }
    }
    /// Bind-host string used for port preflight.
    fn bind_host(self) -> &'static str {
        match self {
            ServeMode::LoopbackHttp => "127.0.0.1",
            _ => "0.0.0.0",
        }
    }
}

/// Production identifiers end in `.release` for packaged desktop or `.server` for deployed headless SSH service;
/// development/test uses the default identifier. Production forbids plaintext `LanHttp` on 0.0.0.0, which exists
/// only for device testing. Production mobile uses HTTPS with certificate pinning. Loopback `--local-http` remains
/// valid for SSH server use; the `.server` check only blocks someone manually starting it with `--lan-http`.
pub fn is_production_identifier(identifier: &str) -> bool {
    identifier.ends_with(".release") || identifier.ends_with(".server")
}

/// Handle for a running service.
struct Running {
    port: u16,
    lan_ips: Vec<String>,
    fingerprint: Option<String>,
    /// Startup mode, determining status URL scheme and host.
    mode: ServeMode,
    /// Authentication state and device registry used for pairing links, device listing, and revocation.
    auth: Arc<AuthState>,
    /// Server E2EE key whose public half is embedded in pairing links.
    e2ee_keys: Arc<e2ee::ServerKeys>,
    handle: axum_server::Handle<std::net::SocketAddr>,
    thread: std::thread::JoinHandle<()>,
}

/// Pairing result returned to the frontend and encoded as a QR code, serialized in camelCase.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingInfo {
    /// Browser URL: `scheme://host:port/#pair=<base64url pairing data>`.
    pub url: String,
    /// Issued device token, also embedded in the URL and exposed separately for display/copy.
    pub device_token: String,
}

/// Public device-entry alias used by commands.rs return types.
pub use auth::DeviceEntry;
/// Argon2id PHC hashing, re-exported so command_core can persist the verifier it passes back on auto-start.
pub use auth::hash_password;

/// Password material for [`WebServer::start`].
pub enum StartAuth {
    /// Plaintext password from an interactive caller or the `--serve` CLI; hashed internally.
    Password(String),
    /// Persisted Argon2id PHC verifier string used by auto-start, where no plaintext exists anymore.
    PasswordHash(String),
}

/// Tauri-managed web-service state holding the running handle behind a Mutex for start/stop transitions.
/// The pairing token, device registry, and blocklist persist in the data directory (`access_store`), so a
/// restart no longer invalidates pairing links; only explicit rotation does.
pub struct WebServer {
    inner: Mutex<Option<Running>>,
    /// Message of the most recent failed auto-start, surfaced through status(); cleared by a successful start.
    autostart_error: Mutex<Option<String>>,
}

impl WebServer {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            autostart_error: Mutex::new(None),
        }
    }

    /// Record or clear the auto-start failure shown in the remote-access panel.
    pub fn set_autostart_error(&self, msg: Option<String>) {
        *self.autostart_error.lock().unwrap() = msg;
    }

    /// Starts the service, first stopping any running instance so password, port, and mode changes apply.
    ///
    /// Modes (see [`ServeMode`]): LanTls is self-signed TLS on 0.0.0.0 with sniff redirect; LoopbackHttp is
    /// plaintext 127.0.0.1 for Electron without per-message TLS overhead; LanHttp is plaintext 0.0.0.0 for native
    /// mobile shells that cannot bypass self-signed certificates.
    ///
    /// All modes retain authentication because other local/LAN clients can reach even plaintext ports.
    pub fn start(
        &self,
        app: AppCtx,
        auth: StartAuth,
        port: Option<u16>,
        mode: ServeMode,
    ) -> Result<WebServerStatus, String> {
        // Normalize both variants to an Argon2id PHC verifier; plaintext never outlives this scope.
        let verifier_phc = match auth {
            StartAuth::Password(pw) => {
                if pw.trim().is_empty() {
                    return Err("Please set an access password first".into());
                }
                auth::hash_password(&pw)?
            }
            StartAuth::PasswordHash(phc) => {
                if phc.trim().is_empty() {
                    return Err("Please set an access password first".into());
                }
                phc
            }
        };
        let mut guard = self.inner.lock().unwrap();
        if let Some(running) = guard.take() {
            running.handle.shutdown();
            let _ = running.thread.join();
        }

        let port = port.unwrap_or(DEFAULT_PORT);
        let bind_host = mode.bind_host();
        // Synchronously preflight port availability, release it immediately, then let axum-server bind.
        drop(
            std::net::TcpListener::bind((bind_host, port))
                .map_err(|e| format!("Port {port} is already in use: {e}"))?,
        );

        let lan_ips = lan_ips();
        // Persist the E2EE server key, device registry, and self-signed TLS certificate in the data directory.
        let data_dir = app.data_dir()?;
        let e2ee_keys = Arc::new(e2ee::ServerKeys::load_or_create(&data_dir)?);
        // Plaintext needs no certificate; TLS creates one and calculates its frontend fingerprint before moving into the thread.
        let tls_pem: Option<(Vec<u8>, Vec<u8>)>;
        let fingerprint: Option<String>;
        if mode.plaintext() {
            tls_pem = None;
            fingerprint = None;
        } else {
            let (cert_pem, key_pem) = tls::ensure_cert(&data_dir, &lan_ips)?;
            fingerprint = tls::fingerprint_sha256(&cert_pem);
            tls_pem = Some((cert_pem, key_pem));
        }

        // Remove the obsolete per-device-token registry, replaced by one rotation token and the persisted
        // pairing state in access_store.
        let _ = std::fs::remove_file(data_dir.join("vlx-devices.json"));
        let auth = Arc::new(AuthState::load_or_create(&verifier_phc, &data_dir)?);
        let ctx = Ctx {
            app,
            auth: auth.clone(),
            e2ee_keys: e2ee_keys.clone(),
            mode,
            limiter: rate_limit::LoginRateLimiter::shared(&data_dir),
        };
        let handle = axum_server::Handle::new();
        let handle_clone = handle.clone();
        let bind_octets = mode.bind_octets();
        let addr = std::net::SocketAddr::from((bind_octets, port));

        let thread = std::thread::Builder::new()
            .name("vlx-web".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        eprintln!("failed to start Web service tokio runtime: {e}");
                        return;
                    }
                };
                rt.block_on(async move {
                    let router = build_router(ctx);
                    match tls_pem {
                        Some((cert_pem, key_pem)) => {
                            // rustls 0.23 requires an explicit global CryptoProvider when both aws-lc-rs and ring
                            // are present. Match tunnel.rs with aws-lc-rs; installation is process-idempotent.
                            let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider()
                                .install_default();
                            let config = match axum_server::tls_rustls::RustlsConfig::from_pem(
                                cert_pem, key_pem,
                            )
                            .await
                            {
                                Ok(c) => c,
                                Err(e) => {
                                    eprintln!("TLS configuration failed: {e}");
                                    return;
                                }
                            };
                            // The TLS acceptor sniffs plaintext HTTP and redirects to HTTPS, avoiding errors when users omit the scheme.
                            let acceptor = axum_server::tls_rustls::RustlsAcceptor::new(config)
                                .acceptor(sniff::HttpSniff);
                            // ConnectInfo exposes the peer SocketAddr to handlers for the per-IP login limiter.
                            if let Err(e) = axum_server::bind(addr)
                                .acceptor(acceptor)
                                .handle(handle_clone)
                                .serve(router.into_make_service_with_connect_info::<std::net::SocketAddr>())
                                .await
                            {
                                eprintln!("Web service exited abnormally: {e}");
                            }
                        }
                        None => {
                            // Local unencrypted path: plain loopback HTTP without TLS acceptor or sniff redirect.
                            if let Err(e) = axum_server::bind(addr)
                                .handle(handle_clone)
                                .serve(router.into_make_service_with_connect_info::<std::net::SocketAddr>())
                                .await
                            {
                                eprintln!("Web service exited abnormally: {e}");
                            }
                        }
                    }
                });
            })
            .map_err(|e| format!("Failed to start web server thread: {e}"))?;

        *guard = Some(Running {
            port,
            lan_ips: lan_ips.clone(),
            fingerprint: fingerprint.clone(),
            mode,
            auth,
            e2ee_keys,
            handle,
            thread,
        });

        // Any successful start supersedes a previous auto-start failure.
        self.set_autostart_error(None);

        Ok(status_from(port, lan_ips, fingerprint, mode))
    }

    /// Stops the service. A manual stop also retires any stale auto-start error: the panel must show the
    /// current truth, not the failure of a boot that the user has since overridden.
    pub fn stop(&self) {
        if let Some(running) = self.inner.lock().unwrap().take() {
            running.handle.shutdown();
            let _ = running.thread.join();
        }
        self.set_autostart_error(None);
    }

    /// Returns current status, including the last auto-start failure for the panel.
    pub fn status(&self) -> WebServerStatus {
        let mut status = match &*self.inner.lock().unwrap() {
            Some(r) => status_from(r.port, r.lan_ips.clone(), r.fingerprint.clone(), r.mode),
            None => WebServerStatus::stopped(),
        };
        status.autostart_error = self.autostart_error.lock().unwrap().clone();
        status
    }

    /// Generates a browser pairing URL containing the current shared token and server public key. `address` chooses
    /// an interface IP; `rotate=true` replaces the token, invalidates old links, and clears all registrations.
    pub fn create_pairing(
        &self,
        address: Option<String>,
        rotate: bool,
    ) -> Result<PairingInfo, String> {
        let guard = self.inner.lock().unwrap();
        let running = guard.as_ref().ok_or("Web server not started")?;
        // Rotate the shared token and clear registrations when requested; otherwise reuse the current token.
        // A rotation that failed to persist is surfaced: silently succeeding would revive old links on restart.
        let token = if rotate {
            running.auth.rotate_pairing_token()?
        } else {
            running.auth.pairing_token()
        };
        let scheme = if running.mode == ServeMode::LanTls {
            "https"
        } else {
            "http"
        };
        let host = address
            .filter(|s| !s.trim().is_empty())
            .or_else(|| running.lan_ips.first().cloned())
            .unwrap_or_else(|| "localhost".to_string());
        // Put token and server public key in the URL fragment so they never reach the server, proxy logs, or Referer.
        let offer = serde_json::json!({
            "t": token,
            "k": running.e2ee_keys.public_key_b64(),
        });
        let code = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(offer.to_string());
        let url = format!("{scheme}://{host}:{}/#pair={code}", running.port);
        Ok(PairingInfo {
            url,
            device_token: token,
        })
    }

    /// Lists paired devices that actually connected; returns empty while stopped.
    pub fn list_devices(&self) -> Vec<DeviceEntry> {
        match &*self.inner.lock().unwrap() {
            Some(r) => r.auth.list_devices(),
            None => Vec::new(),
        }
    }

    /// Revokes a device by blacklisting and deregistering it. E2EE rejects reconnects and an active connection is
    /// dropped within one heartbeat; other devices are unaffected. Returns false while stopped. Errors when the
    /// revocation cannot be persisted, because it would otherwise be undone by the next restart.
    pub fn revoke_device(&self, device_id: &str) -> Result<bool, String> {
        match &*self.inner.lock().unwrap() {
            Some(r) => r.auth.block_device(device_id),
            None => Ok(false),
        }
    }
}

impl Default for WebServer {
    fn default() -> Self {
        Self::new()
    }
}

fn status_from(
    port: u16,
    lan_ips: Vec<String>,
    fingerprint: Option<String>,
    mode: ServeMode,
) -> WebServerStatus {
    // The single scheme of this serve mode, reported explicitly in the status so the frontend never
    // has to infer it from the URL snapshot.
    let scheme = if matches!(mode, ServeMode::LanTls) {
        "https"
    } else {
        "http"
    };
    let urls: Vec<String> = match mode {
        // Loopback plaintext exposes only 127.0.0.1 for Electron sidecar.
        ServeMode::LoopbackHttp => vec![format!("http://127.0.0.1:{port}")],
        // LAN modes enumerate LAN addresses and choose HTTP for mobile or HTTPS for browser remote access.
        ServeMode::LanHttp | ServeMode::LanTls => {
            // Fall back to localhost when no LAN IP is found, preserving local access.
            let hosts = if lan_ips.is_empty() {
                vec!["localhost".to_string()]
            } else {
                lan_ips
            };
            hosts
                .iter()
                .map(|h| format!("{scheme}://{h}:{port}"))
                .collect()
        }
    };
    WebServerStatus {
        running: true,
        port: Some(port),
        url: urls.first().cloned(),
        urls,
        fingerprint,
        autostart_error: None,
        saved_port: None,
        auto_start: false,
        scheme: Some(scheme.to_string()),
    }
}

/// Builds public login/static routes and protected `/api/*` plus `/ws` routes.
fn build_router(ctx: Ctx) -> Router {
    Router::new()
        .route("/api/login", post(auth::login))
        .route("/api/me", get(auth::me))
        .route("/api/mode", get(mode_info))
        .route("/api/logout", post(auth::logout))
        // Image upload now invokes save_pasted_image over authenticated WS with paired E2EE. The cookie-only
        // POST /api/upload path was removed because paired sessions had no cookie and received 401.
        .route("/ws", get(ws::ws_handler))
        .fallback(static_handler)
        .with_state(ctx)
}

/// Tells the frontend whether pairing is mandatory (LanTls only). Without pairing data, mandatory mode asks for a
/// pairing link instead of showing password login; plaintext loopback/LAN modes retain password login.
async fn mode_info(State(ctx): State<Ctx>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "requirePairing": ctx.mode == ServeMode::LanTls,
    }))
}

/// Serves embedded SPA assets, falling back to index.html for frontend routing. Assets contain no secrets and are
/// public; actual data and terminal access remain protected through `/ws`.
async fn static_handler(State(_ctx): State<Ctx>, uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = Assets::get(path) {
        let mime = mime_for(path);
        return ([(header::CONTENT_TYPE, mime)], content.data.into_owned()).into_response();
    }
    // SPA fallback sends unknown paths to frontend routing.
    match Assets::get("index.html") {
        Some(content) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            content.data.into_owned(),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            "Frontend assets not found (dist not built?)",
        )
            .into_response(),
    }
}

/// Write a secret file so it is owner-only (0600) from the moment it exists, instead of chmod-after-write,
/// which leaves a window where the file carries default umask permissions. On non-Unix platforms this is a
/// plain create-truncate write. An idempotent set_permissions afterwards also repairs a pre-existing file
/// that was created with wider permissions by an older build.
pub(crate) fn write_owner_only(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path)?;
    f.write_all(bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // `mode` applies only at creation; tighten a pre-existing file too.
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Minimal extension-to-MIME mapping that avoids another dependency.
fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Enumerates LAN IPv4 candidates that other devices can actually reach, for UI listing, pairing URLs, and TLS SANs.
/// Filtering is deliberately strict because the first result becomes the pairing-link host.
///
/// Rules:
/// 1. Keep only non-loopback private IPv4 ranges, plus the RFC 6598 CGNAT range (100.64.0.0/10) that VPN meshes
///    such as Tailscale assign; `is_private()` does not cover CGNAT, but peers on the same mesh can reach it.
/// 2. Exclude network and broadcast addresses; macOS internal interfaces often report unreachable `.0` noise.
/// 3. Exclude system-internal, VM, container, and bridge interfaces by name.
/// 4. Rank broadcast-capable Wi-Fi/Ethernet first and point-to-point VPN/tunnel interfaces last; CGNAT
///    addresses always rank behind every non-CGNAT entry, regardless of broadcast capability. Tunnels remain
///    as fallbacks because they can be the only reachable address for VPN users, but must not become the default.
fn lan_ips() -> Vec<String> {
    iface_candidates().into_iter().map(|c| c.ip).collect()
}

/// A reachable interface candidate exposed to the remote-access panel's IP selector.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    /// Interface name as reported by the OS (e.g. `en0`, `utun3`, `tailscale0`), shown for recognition.
    pub name: String,
    /// IPv4 address in dotted-decimal form; feeds `web_pairing_create`'s `address` argument.
    pub ip: String,
    /// Point-to-point interface without a broadcast address, usually a VPN/tunnel; marked in the selector.
    pub vpn: bool,
}

/// Lists the selectable interface candidates with names and a VPN flag for the UI's IP selector.
/// Shares [`iface_candidates`] with [`lan_ips`] so selector options, URL list, and TLS SANs
/// are identically filtered by construction.
pub fn network_interfaces_list() -> Vec<NetworkInterface> {
    iface_candidates()
}

/// Single enumeration behind [`lan_ips`] and [`network_interfaces_list`]; applies the documented rules
/// and delegates ranking to the pure [`order_candidates`].
fn iface_candidates() -> Vec<NetworkInterface> {
    let mut candidates: Vec<NetworkInterface> = Vec::new();
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if iface.is_loopback() || is_virtual_iface(&iface.name) {
                continue;
            }
            let name = iface.name;
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                if !(v4.ip.is_private() || is_cgnat(v4.ip))
                    || is_network_or_broadcast(v4.ip, v4.netmask)
                {
                    continue;
                }
                // A broadcast address indicates normal LAN; its absence usually indicates a point-to-point VPN.
                let vpn = v4.broadcast.is_none();
                candidates.push(NetworkInterface {
                    name,
                    ip: v4.ip.to_string(),
                    vpn,
                });
            }
        }
    }
    order_candidates(candidates)
}

/// Pure ranking of accepted candidates: CGNAT (100.64.0.0/10, e.g. Tailscale) addresses always rank
/// behind every non-CGNAT entry regardless of broadcast capability, and within each of those groups
/// broadcast-capable LAN interfaces come before point-to-point tunnels. The sort is stable, so OS
/// enumeration order is preserved inside each bucket. Separated from [`iface_candidates`] so the
/// ordering invariant is unit-testable without depending on the machine's real interfaces.
fn order_candidates(mut candidates: Vec<NetworkInterface>) -> Vec<NetworkInterface> {
    let cgnat = |c: &NetworkInterface| {
        c.ip
            .parse::<std::net::Ipv4Addr>()
            .map(is_cgnat)
            .unwrap_or(false)
    };
    candidates.sort_by_key(|c| (cgnat(c), c.vpn));
    candidates
}

/// Whether an IPv4 lies in the RFC 6598 carrier-grade NAT range 100.64.0.0/10, used by Tailscale.
/// Not covered by `Ipv4Addr::is_private()`; a bitmask check avoids the unstable `is_shared()`.
fn is_cgnat(ip: std::net::Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (o[1] & 0b1100_0000) == 64
}

/// Whether an interface is system-internal, virtual-machine, container, or bridge traffic unreachable from LAN peers.
///
/// VPN tunnels (`utun`/`ppp`) are retained but ranked last by [`lan_ips`] because VPN access may depend on them.
/// Bridges are excluded because they are usually VM NAT gateways or macOS Internet Sharing interfaces unreachable
/// from the real LAN. This intentionally omits the rare direct-phone Internet Sharing case.
fn is_virtual_iface(name: &str) -> bool {
    // Prefixes covering common virtual interfaces across all three platforms:
    const VIRTUAL_PREFIXES: &[&str] = &[
        // macOS AirDrop, low-latency WLAN, Apple private, and software AP.
        "awdl", "llw", "anpi", "ap", // VMware, VirtualBox, and Parallels.
        "vmnet", "vboxnet", "vnic", // Containers and Linux virtual bridges.
        "docker", "veth", "virbr",
        // Bridges/Internet Sharing, usually host-side .1 NAT gateways unreachable externally.
        "bridge", // Overlay networks such as ZeroTier.
        "zt",
    ];
    let name = name.to_ascii_lowercase();
    VIRTUAL_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// Whether an IPv4 is its subnet's network or broadcast address, neither of which is a connectable host.
///
/// For /24, `.0` is network and `.255` broadcast. /31 and /32 point-to-point masks have no conventional host
/// distinction and are allowed directly.
fn is_network_or_broadcast(ip: std::net::Ipv4Addr, netmask: std::net::Ipv4Addr) -> bool {
    let ip = u32::from(ip);
    let host_mask = !u32::from(netmask); // Host bits are 1; network bits are 0.
    if host_mask <= 1 {
        // Allow /32 and /31, which have no conventional network/broadcast distinction.
        return false;
    }
    let host_part = ip & host_mask;
    host_part == 0 || host_part == host_mask
}

#[cfg(test)]
mod tests {
    use super::{
        hash_password, is_cgnat, is_network_or_broadcast, is_production_identifier,
        is_virtual_iface, order_candidates, NetworkInterface, ServeMode, StartAuth, WebServer,
    };
    use std::net::Ipv4Addr;

    /// Find a free port by binding zero then releasing it; the small race is acceptable in tests.
    fn free_port() -> u16 {
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        l.local_addr().unwrap().port()
    }

    /// Restarting the service against the same data dir keeps the pairing token, and a persisted PHC
    /// verifier (the auto-start path) still accepts the original password. Loopback-only; the server is
    /// stopped immediately (never bind 0.0.0.0 in tests). Since the in-process PAIRING_STORES registry
    /// shares live pairing state, the first server AND its references are dropped and the registry is
    /// asserted empty before the restart — proving the token really reloads from the FILE.
    #[test]
    fn pairing_token_survives_restart_and_hash_start_verifies_password() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-web-restart-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let db = crate::db::Db::open(&tmp.join("t.db")).unwrap();
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(tmp.clone(), db));
        let ctx = crate::host::AppCtx::Headless(host);

        let port = free_port();
        let web = WebServer::new();
        web.start(
            ctx.clone(),
            StartAuth::Password("pw".into()),
            Some(port),
            ServeMode::LoopbackHttp,
        )
        .expect("first start should succeed");
        let token1 = web.create_pairing(None, false).unwrap().device_token;
        web.stop();
        // Drop every reference to the first server's pairing state. stop() joins the server thread,
        // so its Ctx clones are gone; dropping the WebServer releases the Running handle's AuthState.
        drop(web);
        assert!(
            !super::auth::pairing_store_alive(&tmp),
            "the shared in-process pairing store must be dead before the restart, otherwise this \
             test would prove shared-state reuse instead of the file reload it claims"
        );

        // "Restart": a fresh WebServer started from a persisted Argon2id hash, as auto-start does.
        let phc = hash_password("pw").unwrap();
        let web2 = WebServer::new();
        // A stale auto-start error must be cleared by a successful start (panel shows current truth).
        web2.set_autostart_error(Some("stale error".into()));
        web2.start(
            ctx,
            StartAuth::PasswordHash(phc),
            Some(port),
            ServeMode::LoopbackHttp,
        )
        .expect("restart from persisted hash should succeed");
        assert!(
            web2.status().autostart_error.is_none(),
            "a successful start must clear a stale autostart error"
        );
        let token2 = web2.create_pairing(None, false).unwrap().device_token;
        assert_eq!(token1, token2, "pairing token must survive a restart");

        // The persisted verifier still accepts the original password (second factor of the handshake).
        {
            let guard = web2.inner.lock().unwrap();
            let running = guard.as_ref().unwrap();
            assert!(running.auth.verify_password("pw"));
            assert!(!running.auth.verify_password("wrong"));
        }
        web2.stop();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Raw loopback HTTP POST to /api/login with `Connection: close`, returning the full response text.
    /// Connecting retries briefly: start() returns once the thread is spawned, slightly before axum's
    /// async bind actually accepts connections.
    fn http_post_login(port: u16, password: &str) -> String {
        use std::io::{Read, Write};
        let mut stream = None;
        for _ in 0..100 {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        let mut stream = stream.expect("web server never started accepting connections");
        let body = format!("{{\"password\":\"{password}\"}}");
        let req = format!(
            "POST /api/login HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = stream.read_to_string(&mut resp);
        resp
    }

    /// Repeated failed logins are rate-limited per IP: five wrong passwords yield 401, the sixth attempt
    /// is rejected with 429 before any Argon2 work — even with the correct password — and a correct
    /// password within the limit returns 200 plus a token. Loopback-only; this also proves the
    /// ConnectInfo wiring works with axum-server for real connections.
    #[test]
    fn login_is_rate_limited_per_ip_before_argon2() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-web-ratelimit-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let db = crate::db::Db::open(&tmp.join("t.db")).unwrap();
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(tmp.clone(), db));
        let ctx = crate::host::AppCtx::Headless(host);

        // Ephemeral-port retry pattern shared with the dispatch tests: the probed port can be stolen.
        let web = WebServer::new();
        let mut port = 0;
        let mut started = Err("never attempted".to_string());
        for _ in 0..5 {
            port = free_port();
            started = web.start(
                ctx.clone(),
                StartAuth::Password("right-pw".into()),
                Some(port),
                ServeMode::LoopbackHttp,
            );
            if started.is_ok() {
                break;
            }
        }
        started.expect("failed to start the loopback web server after retries");

        // A correct password within the limit succeeds and returns a session token.
        let ok = http_post_login(port, "right-pw");
        assert!(ok.starts_with("HTTP/1.1 200"), "expected 200, got: {ok}");
        assert!(ok.contains("token"), "expected a token body, got: {ok}");

        // Five wrong passwords are individually rejected as 401 (success above cleared the counter).
        for i in 0..5 {
            let r = http_post_login(port, "wrong");
            assert!(r.starts_with("HTTP/1.1 401"), "attempt {i}: expected 401, got: {r}");
        }
        // The sixth attempt hits the limiter BEFORE Argon2: even the correct password now yields 429.
        let blocked = http_post_login(port, "right-pw");
        assert!(
            blocked.starts_with("HTTP/1.1 429"),
            "expected 429 after five failures, got: {blocked}"
        );

        web.stop();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// write_owner_only creates secret files with 0600 at open time and repairs looser pre-existing modes.
    #[cfg(unix)]
    #[test]
    fn write_owner_only_creates_and_repairs_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "vlx-write-owner-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secret.txt");
        super::write_owner_only(&path, b"s3cret").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read(&path).unwrap(), b"s3cret");

        // A pre-existing file with wide permissions is overwritten AND tightened.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        super::write_owner_only(&path, b"rotated").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read(&path).unwrap(), b"rotated");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The status carries the serve mode's scheme explicitly (FIX for the frontend's proxy inference
    /// from the first snapshot URL): https only for LanTls, http for both plaintext modes, None stopped.
    #[test]
    fn status_reports_the_explicit_scheme_per_mode() {
        use super::{status_from, WebServerStatus};
        let s = |mode| status_from(8799, vec![], None, mode);
        assert_eq!(s(ServeMode::LanTls).scheme.as_deref(), Some("https"));
        assert_eq!(s(ServeMode::LanHttp).scheme.as_deref(), Some("http"));
        assert_eq!(s(ServeMode::LoopbackHttp).scheme.as_deref(), Some("http"));
        assert!(WebServerStatus::stopped().scheme.is_none());
    }

    #[test]
    fn release_identifier_is_production() {
        // Release-script identifiers end in .release and forbid plaintext LAN mode.
        assert!(is_production_identifier("io.vlinx.vlxterm.release"));
    }

    #[test]
    fn dev_identifier_is_not_production() {
        // Default development/test identifier permits plaintext LAN for mobile-device testing.
        assert!(!is_production_identifier("io.vlinx.vlxterm"));
    }

    #[test]
    fn server_identifier_is_production() {
        // Headless SSH server identifiers end in .server and also forbid plaintext LAN defensively.
        assert!(is_production_identifier("io.vlinx.vlxterm.server"));
    }

    #[test]
    fn network_and_broadcast_addresses_are_filtered() {
        let mask24 = Ipv4Addr::new(255, 255, 255, 0);
        // Filter network addresses whose host bits are all zero.
        assert!(is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 97, 0),
            mask24
        ));
        // Filter broadcast addresses whose host bits are all one.
        assert!(is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 97, 255),
            mask24
        ));
        // Keep normal host addresses.
        assert!(!is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 88, 205),
            mask24
        ));
        // Keep a VM gateway .1 here; interface-name filtering handles it separately.
        assert!(!is_network_or_broadcast(
            Ipv4Addr::new(172, 16, 68, 1),
            mask24
        ));
        // Allow /32 point-to-point addresses common to VPNs.
        assert!(!is_network_or_broadcast(
            Ipv4Addr::new(10, 10, 10, 134),
            Ipv4Addr::new(255, 255, 255, 255)
        ));
        // Under /16, 192.168.0.0 is network while 192.168.97.0 is a valid host.
        let mask16 = Ipv4Addr::new(255, 255, 0, 0);
        assert!(is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 0, 0),
            mask16
        ));
        assert!(!is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 97, 0),
            mask16
        ));
    }

    #[test]
    fn cgnat_range_boundaries_are_exact() {
        // 100.64.0.0/10 (RFC 6598, Tailscale) is inside; its immediate neighbours are outside.
        assert!(!is_cgnat(Ipv4Addr::new(100, 63, 255, 255)));
        assert!(is_cgnat(Ipv4Addr::new(100, 64, 0, 0)));
        assert!(is_cgnat(Ipv4Addr::new(100, 100, 83, 2)));
        assert!(is_cgnat(Ipv4Addr::new(100, 127, 255, 255)));
        assert!(!is_cgnat(Ipv4Addr::new(100, 128, 0, 0)));
        // Ordinary private ranges are not CGNAT; they stay accepted via is_private().
        assert!(!is_cgnat(Ipv4Addr::new(192, 168, 1, 5)));
        assert!(!is_cgnat(Ipv4Addr::new(10, 0, 0, 5)));
    }

    /// Shorthand fixture for [`order_candidates`] tests; deliberately independent of real interfaces.
    fn cand(name: &str, ip: &str, vpn: bool) -> NetworkInterface {
        NetworkInterface {
            name: name.into(),
            ip: ip.into(),
            vpn,
        }
    }

    #[test]
    fn cgnat_candidates_rank_behind_lan_regardless_of_broadcast_capability() {
        // Worst-case input order: a broadcast-capable CGNAT entry first (the case the old
        // broadcast-only heuristic misplaced), then LAN, a non-CGNAT tunnel, and a CGNAT tunnel.
        let ordered = order_candidates(vec![
            cand("feth0", "100.100.83.2", false), // CGNAT with broadcast: must not count as LAN.
            cand("en0", "192.168.1.5", false),
            cand("utun1", "10.8.0.2", true), // Non-CGNAT VPN tunnel.
            cand("utun3", "100.101.0.7", true), // CGNAT VPN tunnel (typical Tailscale).
        ]);
        let ips: Vec<&str> = ordered.iter().map(|c| c.ip.as_str()).collect();
        assert_eq!(
            ips,
            ["192.168.1.5", "10.8.0.2", "100.100.83.2", "100.101.0.7"],
            "CGNAT must rank behind every non-CGNAT entry; broadcast-capable before tunnels within each group"
        );
    }

    #[test]
    fn order_candidates_is_stable_within_buckets() {
        // OS enumeration order is meaningful inside a bucket and must survive the ranking.
        let ordered = order_candidates(vec![
            cand("en0", "192.168.1.5", false),
            cand("en1", "10.0.0.5", false),
            cand("utun3", "100.100.83.2", true),
            cand("utun4", "100.100.83.3", true),
        ]);
        let ips: Vec<&str> = ordered.iter().map(|c| c.ip.as_str()).collect();
        assert_eq!(
            ips,
            ["192.168.1.5", "10.0.0.5", "100.100.83.2", "100.100.83.3"]
        );
    }

    #[test]
    fn virtual_interfaces_are_flagged() {
        // Exclude system, VM, container, and bridge interfaces, including host-side .1 NAT/Internet Sharing gateways.
        for n in [
            "awdl0",
            "llw0",
            "anpi0",
            "anpi3",
            "ap1",
            "vmnet1",
            "vmnet8",
            "vboxnet0",
            "vnic0",
            "docker0",
            "veth1a2b3c",
            "virbr0",
            "zt5u4i",
            "bridge0",
            "bridge100",
        ] {
            assert!(is_virtual_iface(n), "{n} should be treated as a virtual interface");
        }
        // Keep real LAN and VPN tunnel interfaces; lan_ips merely ranks VPNs lower.
        for n in ["en0", "en1", "eth0", "wlan0", "utun0", "ppp0"] {
            assert!(!is_virtual_iface(n), "{n} should not be treated as a virtual interface");
        }
    }
}
