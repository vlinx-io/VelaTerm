//! Z.ai ZCode (`zcode`): starting its agent, listing its models, and reading its session store.
//!
//! ZCode publishes no command-line package. Its agent is the `glm/zcode.cjs` bundle inside the desktop
//! release, which the desktop app starts with its own Electron acting as Node (`ELECTRON_RUN_AS_NODE=1`) and
//! the provider config and search tools of the release passed by environment. VelaTerm starts it the same
//! way when the executable it resolved is the desktop app's own binary. A `zcode` command, such as the shim
//! VelaTerm's one-click install writes to `~/.local/bin` or ZCode's source-built installer, starts directly.
//!
//! VelaTerm drives only the agent's protocol server (`app-server --stdio`), from the conversation view; ZCode
//! has no terminal session, hooks, or plugin of VelaTerm's.

use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

const DEFAULT_ENDPOINT_ORIGIN: &str = "https://zcode.z.ai";
/// Matches ZCode's bundled CLI version when run outside the desktop shell.
const CLIENT_VERSION: &str = "0.0.0-dev";
const REFRESH_TTL: Duration = Duration::from_secs(30 * 60);
const RETRY_BACKOFF: Duration = Duration::from_secs(5 * 60);
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONFIG_BYTES: usize = 10 * 1024 * 1024;

#[derive(Default)]
struct RefreshEntry {
    in_flight: bool,
    next_check: Option<Instant>,
    last_url: Option<String>,
}

static REFRESH_STATE: Mutex<Option<HashMap<PathBuf, RefreshEntry>>> = Mutex::new(None);

/// The desktop release's resource directory beside its main binary: `Contents/Resources` in a macOS app
/// bundle, `resources` in a Windows install. Only one that carries the agent bundle counts.
pub(crate) fn desktop_resources(bin: &Path) -> Option<PathBuf> {
    let dir = bin.parent()?;
    let mac = dir.parent().map(|contents| contents.join("Resources"));
    mac.into_iter()
        .chain([dir.join("resources")])
        .find(|resources| resources.join("glm").join("zcode.cjs").is_file())
}

/// The environment the desktop app gives its agent besides `ELECTRON_RUN_AS_NODE`: the provider config the
/// release ships, which the agent otherwise looks for beside its bundle, and the search tools when present.
fn desktop_env(resources: &Path, win: bool) -> Vec<(&'static str, PathBuf)> {
    let mut env = vec![(
        "ZCODE_BUILTIN_PROVIDER_CONFIG_FILE",
        resources.join("config").join("provider").join("zcode-builtin.json"),
    )];
    let exe = |name: &str| if win { format!("{name}.exe") } else { name.to_string() };
    for (key, dir, name) in [
        ("ZCODE_RG_BINARY", "ripgrep", "rg"),
        ("ZCODE_BFS_BINARY", "bfs", "bfs"),
        ("ZCODE_UGREP_BINARY", "ugrep", "ugrep"),
    ] {
        let tool = resources.join("tools").join(dir).join(exe(name));
        if tool.is_file() {
            env.push((key, tool));
        }
    }
    env
}

/// The built-in provider config the agent at `bin` ships with: the desktop release's, or the copy VelaTerm's
/// one-click install keeps beside the `zcode` shim it writes. Unknown for any other `zcode` command.
fn shipped_config(bin: &Path, resources: Option<&Path>) -> Option<PathBuf> {
    if let Some(resources) = resources {
        return Some(resources.join("config").join("provider").join("zcode-builtin.json"));
    }
    let shim = std::fs::read(bin).ok().filter(|bytes| bytes.len() < 4096)?;
    if !String::from_utf8_lossy(&shim).contains("Installed by VelaTerm") {
        return None;
    }
    let config = zcode_home()?.join("vlx-term").join("agent").join("config").join("provider").join("zcode-builtin.json");
    config.is_file().then_some(config)
}

fn exported(key: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(key).filter(|v| !v.is_empty())
}

fn exported_str(key: &str) -> Option<String> {
    exported(key)
        .map(|v| v.to_string_lossy().trim().to_string())
        .filter(|v| !v.is_empty())
}

/// ZCode's platform tag for `/api/v1/client/configs`, matching `resolveZCodeBuiltinClientPlatform`.
fn client_platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "windows",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        other => other,
    };
    format!("{os}-{arch}")
}

/// Normalize a URL string to its HTTP(S) origin (`scheme://host[:port]`) without embedded credentials.
fn normalize_origin(raw: &str) -> Option<String> {
    let url = url::Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "https" | "http") || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    })
}

/// The ZCode control-plane origin, honoring the same environment overrides ZCode reads.
fn endpoint_origin() -> String {
    let is_test = exported_str("ZCODE_ENV").is_some_and(|v| v.eq_ignore_ascii_case("test"));
    let env_specific = if is_test { "ZCODE_TEST_BASE_URL" } else { "ZCODE_PRODUCTION_BASE_URL" };
    for key in ["ZCODE_BASE_URL", "ZCODE_ENDPOINT_ORIGIN", env_specific] {
        if let Some(origin) = exported_str(key).and_then(|v| normalize_origin(&v)) {
            return origin;
        }
    }
    DEFAULT_ENDPOINT_ORIGIN.to_string()
}

/// Endpoint cache key matching ZCode's `createZCodeBuiltinEndpointKey`.
fn endpoint_key(origin: &str) -> String {
    let digest = Sha256::digest(origin.as_bytes());
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("endpoint-{hex}")
}

fn velaterm_builtin_cache_path(base: &Path, platform: &str, origin: &str) -> PathBuf {
    base.join(".zcode")
        .join("vlx-term")
        .join("provider")
        .join(platform)
        .join(endpoint_key(origin))
        .join("zcode-builtin.json")
}

/// Read-only discovery of ZCode's own refreshed caches for `platform` and `origin`, so an offline session
/// can still pick up a newer release ZCode Desktop already fetched.
fn zcode_runtime_builtin_candidates(base: &Path, platform: &str, origin: &str) -> Vec<PathBuf> {
    let root = base.join(".zcode").join("v2").join("runtime").join("provider").join(platform);
    let key = endpoint_key(origin);
    let Ok(entries) = std::fs::read_dir(&root) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(&key).join("zcode-builtin.json"))
        .filter(|path| path.is_file())
        .collect()
}

/// Validate a built-in provider config payload and return its positive `revision`.
fn validate_builtin_bytes(bytes: &[u8]) -> Option<u64> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if value.get("schemaVersion").and_then(serde_json::Value::as_u64) != Some(1) {
        return None;
    }
    let revision = value.get("revision").and_then(serde_json::Value::as_u64).filter(|r| *r > 0)?;
    let rules = value
        .pointer("/config/providerConfigRules/providerRules")
        .and_then(serde_json::Value::as_array)?;
    if rules.is_empty() || !value.pointer("/config/modelConfigRules").is_some_and(serde_json::Value::is_object) {
        return None;
    }
    Some(revision)
}

fn builtin_revision(path: &Path) -> Option<u64> {
    let bytes = std::fs::read(path).ok()?;
    validate_builtin_bytes(&bytes)
}

/// Pick the built-in provider config with the highest valid revision among the shipped copy, VelaTerm's
/// cached remote copy, and any ZCode runtime cache. Never downgrades below `shipped`, and returns `shipped`
/// untouched when `shipped` is absent (e.g. synthetic test paths).
fn best_builtin_config(shipped: PathBuf, base: &Path, platform: &str, origin: &str) -> PathBuf {
    let Some(shipped_rev) = builtin_revision(&shipped) else {
        return shipped;
    };
    let mut best_rev = shipped_rev;
    let mut best_path = shipped;
    let mut candidates = vec![velaterm_builtin_cache_path(base, platform, origin)];
    candidates.extend(zcode_runtime_builtin_candidates(base, platform, origin));
    for candidate in candidates {
        if let Some(rev) = builtin_revision(&candidate) {
            if rev > best_rev {
                best_rev = rev;
                best_path = candidate;
            }
        }
    }
    best_path
}

/// Validate a CDN download URL returned by `/api/v1/client/configs`. Requires HTTPS whenever the control
/// plane itself uses HTTPS, and rejects embedded credentials.
fn validate_cdn_url(origin: &str, raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw.trim()).ok()?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    let allow_http = origin.starts_with("http://");
    let scheme_ok = parsed.scheme() == "https" || (allow_http && parsed.scheme() == "http");
    scheme_ok.then(|| parsed.to_string())
}

enum RemoteFetch {
    Updated { bytes: Vec<u8>, revision: u64, url: String },
    NotModified,
    Missing,
}

fn read_bounded(resp: ureq::Response, max_bytes: usize) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let limit = (max_bytes as u64).saturating_add(1);
    resp.into_reader()
        .take(limit)
        .read_to_end(&mut buf)
        .map_err(|e| format!("read failed: {e}"))?;
    if buf.len() > max_bytes {
        return Err("body limit exceeded".to_string());
    }
    Ok(buf)
}

fn fetch_remote_builtin(origin: &str, platform: &str, reuse_url: Option<&str>) -> Result<RemoteFetch, String> {
    let agent = ureq::AgentBuilder::new().redirects(0).timeout(HTTP_TIMEOUT).build();
    let mut config_url = url::Url::parse(origin)
        .and_then(|base| base.join("/api/v1/client/configs"))
        .map_err(|_| "invalid endpoint origin".to_string())?;
    config_url
        .query_pairs_mut()
        .append_pair("app_version", CLIENT_VERSION)
        .append_pair("platform", platform);
    let resp = agent
        .get(config_url.as_str())
        .set("Accept", "application/json")
        .call()
        .map_err(|e| format!("client-config request failed: {e}"))?;
    let body = read_bounded(resp, 1024 * 1024)?;
    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| "client-config response is not JSON".to_string())?;
    if payload.get("code").and_then(serde_json::Value::as_u64) != Some(0) {
        return Err("client-config returned non-zero code".to_string());
    }
    let Some(raw_url) = payload
        .pointer("/data/configs/builtin_provider_config_json")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(RemoteFetch::Missing);
    };
    let cdn_url = validate_cdn_url(origin, raw_url)
        .ok_or_else(|| "client-config returned an invalid CDN URL".to_string())?;
    if reuse_url == Some(cdn_url.as_str()) {
        return Ok(RemoteFetch::NotModified);
    }
    let cdn_resp = agent
        .get(&cdn_url)
        .set("Accept", "application/json")
        .call()
        .map_err(|e| format!("cdn request failed: {e}"))?;
    let bytes = read_bounded(cdn_resp, MAX_CONFIG_BYTES)?;
    let revision = validate_builtin_bytes(&bytes)
        .ok_or_else(|| "cdn payload failed built-in provider config validation".to_string())?;
    Ok(RemoteFetch::Updated { bytes, revision, url: cdn_url })
}

fn write_atomic_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(())
}

/// Refresh VelaTerm's cached copy of ZCode's built-in provider config from ZCode's CDN when stale.
///
/// VelaTerm pins `ZCODE_BUILTIN_PROVIDER_CONFIG_FILE` and `ZCODE_PERSONAL_PROVIDER_CONFIG_FILE` so the
/// account payload can name the exact revision `app-server` loaded, which causes ZCode's own CLI wrapper to
/// skip remote refresh. Running this before `command(bin)` on background model/session startup keeps new
/// cloud-published models (such as GLM-5.4) available without waiting for a new desktop release.
///
/// Best-effort: never blocks longer than `HTTP_TIMEOUT`, rate-limited by `REFRESH_TTL` / `RETRY_BACKOFF`,
/// single-flight per cache path, and silently falls back to the best local copy on any failure.
pub fn refresh_builtin_config(bin: &str) {
    use crate::agent::zcode_account::BUILTIN_CONFIG;
    if exported(BUILTIN_CONFIG).is_some() {
        return;
    }
    let resources = desktop_resources(Path::new(bin));
    let Some(shipped) = shipped_config(Path::new(bin), resources.as_deref()) else { return };
    let Some(shipped_rev) = builtin_revision(&shipped) else { return };
    let Some(base) = exported("ZCODE_DATA_BASE_DIR").map(PathBuf::from).or_else(crate::host::home_dir) else {
        return;
    };
    let platform = client_platform();
    let origin = endpoint_origin();
    let cache_path = velaterm_builtin_cache_path(&base, &platform, &origin);
    let current_cached_rev = builtin_revision(&cache_path);
    let now = Instant::now();
    let reuse_url = {
        let mut guard = REFRESH_STATE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        let entry = map.entry(cache_path.clone()).or_default();
        if entry.in_flight || entry.next_check.is_some_and(|next| now < next) {
            return;
        }
        entry.in_flight = true;
        current_cached_rev.and_then(|_| entry.last_url.clone())
    };

    let outcome = fetch_remote_builtin(&origin, &platform, reuse_url.as_deref());
    let mut new_url = reuse_url;
    let backoff = match outcome {
        Ok(RemoteFetch::Updated { bytes, revision, url }) => {
            let floor = current_cached_rev.unwrap_or(0).max(shipped_rev);
            if revision > floor {
                match write_atomic_private(&cache_path, &bytes) {
                    Ok(()) => {
                        new_url = Some(url);
                        crate::diagnostics::record("INFO", "zcode_builtin_config_updated", serde_json::json!({
                            "revision": revision, "previousRevision": floor,
                        }));
                    }
                    Err(err) => {
                        crate::diagnostics::record("WARN", "zcode_builtin_config_refresh_failed", serde_json::json!({
                            "reason": format!("write failed: {err}"),
                        }));
                    }
                }
            } else {
                new_url = Some(url);
            }
            REFRESH_TTL
        }
        Ok(RemoteFetch::NotModified | RemoteFetch::Missing) => REFRESH_TTL,
        Err(reason) => {
            crate::diagnostics::record("WARN", "zcode_builtin_config_refresh_failed", serde_json::json!({
                "reason": reason,
            }));
            RETRY_BACKOFF
        }
    };

    let mut guard = REFRESH_STATE.lock().unwrap();
    if let Some(entry) = guard.as_mut().and_then(|m| m.get_mut(&cache_path)) {
        entry.in_flight = false;
        entry.next_check = Some(Instant::now() + backoff);
        entry.last_url = new_url;
    }
}

/// A command that runs ZCode from `bin`, before its subcommand. The desktop app's binary runs the bundled
/// agent as Node; anything else is the `zcode` command itself. A variable the user exported already wins.
///
/// Both provider config files are pinned when the shipped one is known. Given both, ZCode reads them as they
/// are instead of a copy it refreshes on its own, so the account VelaTerm reports can name the revision the
/// server loaded (see `zcode_account`). When VelaTerm's remote cache or ZCode's runtime cache holds a newer
/// revision than the shipped file, that file is pinned instead.
pub fn command(bin: &str) -> std::process::Command {
    use crate::agent::zcode_account::{BUILTIN_CONFIG, PERSONAL_CONFIG};
    let mut cmd = crate::host::command(bin);
    let resources = desktop_resources(Path::new(bin));
    if let Some(resources) = &resources {
        cmd.arg(resources.join("glm").join("zcode.cjs"));
        cmd.env("ELECTRON_RUN_AS_NODE", "1");
        for (key, value) in desktop_env(resources, cfg!(windows)) {
            if exported(key).is_none() {
                cmd.env(key, value);
            }
        }
    }
    let base = exported("ZCODE_DATA_BASE_DIR").map(PathBuf::from).or_else(crate::host::home_dir);
    let builtin = exported(BUILTIN_CONFIG).map(PathBuf::from).or_else(|| {
        let shipped = shipped_config(Path::new(bin), resources.as_deref())?;
        Some(match base.as_deref() {
            Some(base) => best_builtin_config(shipped, base, &client_platform(), &endpoint_origin()),
            None => shipped,
        })
    });
    if let (Some(builtin), None, Some(base)) = (builtin, exported(PERSONAL_CONFIG), base) {
        cmd.env(BUILTIN_CONFIG, builtin);
        cmd.env(PERSONAL_CONFIG, base.join(".zcode").join("v2").join("provider_config.json"));
    }
    cmd
}

/// ZCode resolves every default path from the OS home directory; there is no ZCODE_HOME override.
fn zcode_home() -> Option<PathBuf> {
    crate::host::home_dir().map(|home| home.join(".zcode"))
}

/// The session database, honoring the same overrides ZCode reads (`ZCODE_SESSION_DB_PATH`, then
/// `ZCODE_SESSION_DB`) when VelaTerm itself inherits them.
pub fn session_db_path() -> Option<PathBuf> {
    for key in ["ZCODE_SESSION_DB_PATH", "ZCODE_SESSION_DB"] {
        if let Some(path) = std::env::var_os(key).filter(|p| !p.is_empty()) {
            return Some(PathBuf::from(path));
        }
    }
    zcode_home().map(|home| home.join("cli").join("db").join("db.sqlite"))
}

/// Whether a ZCode session is **confirmed** absent: the database opens and holds no live row for `id`.
/// An unreadable or missing database cannot prove anything, so it returns false and resume is attempted.
pub fn session_missing(id: &str) -> bool {
    session_db_path().is_some_and(|db| session_missing_in(&db, id))
}

fn session_missing_in(db: &Path, id: &str) -> bool {
    use rusqlite::{Connection, OpenFlags, OptionalExtension};
    if !db.is_file() {
        return false;
    }
    let Ok(conn) = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return false;
    };
    // ZCode writes the same WAL database while it runs; wait briefly rather than misread a busy lock.
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    // `time_deleted` marks a soft-deleted session, which `--resume` reports as not found.
    match conn
        .query_row("SELECT time_deleted FROM session WHERE id = ?1", [id], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .optional()
    {
        Ok(None) => true,
        Ok(Some(deleted)) => deleted.is_some(),
        // A schema change or lock error is not evidence of absence.
        Err(_) => false,
    }
}

/// The model catalogue a short-lived `zcode app-server` reports, for a conversation with no running process.
///
/// A draft `session/create` answers with the catalogue and is closed straight away; ZCode persists a session
/// only once it receives a message, so the lookup leaves nothing behind.
pub fn list_models(bin: &str, cwd: &str) -> Result<Vec<serde_json::Value>, String> {
    use crate::agent::chat::zcode_protocol::{self as wire, Incoming};
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;

    refresh_builtin_config(bin);
    let mut cmd = command(bin);
    cmd.arg("app-server").arg("--stdio").current_dir(cwd);
    crate::login_env::refresh_command(&mut cmd);
    crate::agent::executable::prepare_command(&mut cmd, bin);
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS {
        cmd.env_remove(key);
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let account = crate::agent::zcode_account::Account::for_command(&cmd).and_then(|account| account.config());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start ZCode {bin:?} to read its model catalogue: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("ZCode has no input stream")?;
    let stdout = child.stdout.take().ok_or("ZCode has no output stream")?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let send = |stdin: &mut std::process::ChildStdin, value: &serde_json::Value| {
        let _ = stdin.write_all(format!("{value}\n").as_bytes()).and_then(|_| stdin.flush());
    };
    // The catalogue lists only the plans the account holds, and the server learns those from the client.
    // It answers in order, so the account is in place before the draft is created.
    if let Some(params) = account {
        send(&mut stdin, &wire::request("vlx-account", "provider/updateAccountConfig", params));
    }
    send(&mut stdin, &wire::request("vlx-catalog", "session/create",
        serde_json::json!({"workspace": {"workspacePath": cwd, "workspaceKey": cwd}})));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut found = None;
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        let Ok(line) = rx.recv_timeout(left) else { break };
        match wire::parse_line(&line) {
            Some(Incoming::Request { id, method, .. }) => {
                let answer = if method == "session/requestRuntimePreferences" {
                    wire::reply(&id, serde_json::json!({"nativeSearchEnhancementsEnabled": false}))
                } else {
                    wire::reply_unsupported(&id)
                };
                send(&mut stdin, &answer);
            }
            Some(Incoming::Response { id, result, error }) if id == "vlx-catalog" => {
                if error.is_none() {
                    if let Some(draft) = result.pointer("/session/sessionId").and_then(serde_json::Value::as_str) {
                        send(&mut stdin, &wire::request("vlx-close", "session/close", serde_json::json!({"sessionId": draft})));
                    }
                    found = result.pointer("/settings/model/available").and_then(serde_json::Value::as_array).cloned();
                }
                break;
            }
            _ => {}
        }
    }
    // Closing stdin lets ZCode shut down on its own; the kill is the bound if it does not.
    drop(stdin);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let _ = child.kill();
    let _ = child.wait();
    let available = found.ok_or("ZCode did not report its model catalogue")?;
    Ok(wire::to_chat_models(&available))
}

/// A ZCode conversation read back from its session store, oldest first, for replay and export.
///
/// Messages and their parts live in the `message` and `part` tables as JSON. User messages ZCode itself
/// injected (environment reminders, tool listings) are marked as not visible and left out, as ZCode's own
/// views do. Nothing here writes.
pub(crate) fn history_events(id: &str) -> Result<Vec<crate::agent::export::Event>, String> {
    let db = session_db_path().ok_or("ZCode's session store could not be located")?;
    history_events_in(&db, id)
}

fn history_events_in(db: &Path, id: &str) -> Result<Vec<crate::agent::export::Event>, String> {
    use crate::agent::export::Event;
    use rusqlite::{Connection, OpenFlags};
    use serde_json::Value;

    if !db.is_file() {
        return Err("ZCode has not recorded any conversation on this machine".to_string());
    }
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| format!("Failed to open ZCode's session store: {e}"))?;
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    let read_rows = |sql: &str| -> Result<Vec<(String, String)>, String> {
        let mut stmt = conn.prepare(sql).map_err(|e| format!("Failed to read ZCode's session store: {e}"))?;
        let rows = stmt
            .query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| format!("Failed to read ZCode's session store: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| format!("Failed to read ZCode's session store: {e}"))
    };
    let messages = read_rows(
        "SELECT id, data FROM message WHERE session_id = ?1 ORDER BY COALESCE(sequence, 0), time_created, id",
    )?;
    if messages.is_empty() {
        return Err(format!("ZCode has no conversation {id}"));
    }
    let parts = read_rows(
        "SELECT message_id, data FROM part WHERE session_id = ?1 ORDER BY COALESCE(sequence, 0), time_created, id",
    )?;
    let mut by_message: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();
    for (message, data) in parts {
        if let Ok(value) = serde_json::from_str::<Value>(&data) {
            by_message.entry(message).or_default().push(value);
        }
    }
    let ts = |value: &Value| {
        value
            .pointer("/time/created")
            .and_then(Value::as_i64)
            .and_then(crate::agent::chat::opencode_timeline::iso_from_ms)
    };
    let mut events = Vec::new();
    for (message_id, data) in messages {
        let Ok(info) = serde_json::from_str::<Value>(&data) else { continue };
        let parts = by_message.remove(&message_id).unwrap_or_default();
        let text_of = |part: &Value| {
            let hidden = part.get("synthetic").and_then(Value::as_bool) == Some(true)
                || part.get("ignored").and_then(Value::as_bool) == Some(true);
            (!hidden).then(|| part.get("text").and_then(Value::as_str).unwrap_or("").to_string())
        };
        match info.get("role").and_then(Value::as_str) {
            Some("user") => {
                let visible = match info.pointer("/semantics/uiVisibility").and_then(Value::as_str) {
                    Some(visibility) => visibility == "visible",
                    None => {
                        info.get("synthetic").and_then(Value::as_bool) != Some(true)
                            && info.pointer("/metadata/visibility").and_then(Value::as_str) != Some("model-only")
                    }
                };
                if !visible {
                    continue;
                }
                let text: Vec<String> = parts
                    .iter()
                    .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(text_of)
                    .filter(|text| !text.is_empty())
                    .collect();
                if !text.is_empty() {
                    events.push(Event::User { text: text.join("\n\n"), ts: ts(&info) });
                }
            }
            Some("assistant") => {
                for part in &parts {
                    match part.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(text) = text_of(part).filter(|t| !t.is_empty()) {
                                events.push(Event::AssistantText { text, ts: ts(&info) });
                            }
                        }
                        Some("reasoning") => {
                            if let Some(text) = part.get("text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                                events.push(Event::Thinking { text: text.to_string(), ts: ts(&info) });
                            }
                        }
                        Some("tool") => {
                            let call = part.get("callID").and_then(Value::as_str).map(str::to_string);
                            let name = crate::agent::chat::zcode_protocol::tool_name(
                                part.get("tool").and_then(Value::as_str).unwrap_or("tool"),
                            );
                            let state = part.get("state").cloned().unwrap_or(Value::Null);
                            events.push(Event::ToolUse {
                                id: call.clone(),
                                name: name.clone(),
                                input: state.get("input").cloned().unwrap_or_else(|| serde_json::json!({})),
                                ts: ts(&info),
                            });
                            let status = state.get("status").and_then(Value::as_str).unwrap_or("");
                            if matches!(status, "completed" | "error") {
                                let text = state
                                    .get("output")
                                    .and_then(Value::as_str)
                                    .or_else(|| state.get("error").and_then(Value::as_str))
                                    .unwrap_or("")
                                    .to_string();
                                events.push(Event::ToolResult { id: call, name: Some(name), text, is_error: status == "error" });
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vlx-zcode-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp home");
        dir
    }

    #[test]
    fn desktop_binary_runs_the_bundled_agent_as_node() {
        let home = temp_home("desktop");
        let contents = home.join("ZCode.app").join("Contents");
        let bin = contents.join("MacOS").join("ZCode");
        let resources = contents.join("Resources");
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, "").unwrap();
        assert_eq!(desktop_resources(&bin), None, "an app without the agent bundle is not ZCode's desktop release");

        let entry = resources.join("glm").join("zcode.cjs");
        let rg = resources.join("tools").join("ripgrep").join(if cfg!(windows) { "rg.exe" } else { "rg" });
        for file in [&entry, &rg] {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "").unwrap();
        }
        assert_eq!(desktop_resources(&bin), Some(resources.clone()));
        let cmd = command(bin.to_str().unwrap());
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), [entry.as_os_str()]);
        let env: std::collections::HashMap<_, _> =
            cmd.get_envs().filter_map(|(k, v)| Some((k.to_string_lossy().into_owned(), v?.to_os_string()))).collect();
        assert_eq!(env["ELECTRON_RUN_AS_NODE"], "1");
        assert_eq!(Path::new(&env["ZCODE_BUILTIN_PROVIDER_CONFIG_FILE"]), resources.join("config/provider/zcode-builtin.json"));
        assert_eq!(Path::new(&env["ZCODE_RG_BINARY"]), rg);
        assert!(!env.contains_key("ZCODE_BFS_BINARY"), "only tools the release ships are passed");

        // A Windows install keeps its resources beside the executable.
        let win_bin = home.join("Programs").join("ZCode").join("ZCode.exe");
        let win_entry = win_bin.parent().unwrap().join("resources").join("glm").join("zcode.cjs");
        std::fs::create_dir_all(win_entry.parent().unwrap()).unwrap();
        std::fs::write(&win_entry, "").unwrap();
        assert_eq!(desktop_resources(&win_bin), Some(win_bin.parent().unwrap().join("resources")));

        // Any other executable is the `zcode` command itself.
        let shim = home.join("bin").join("zcode");
        assert_eq!(command(shim.to_str().unwrap()).get_args().count(), 0);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn history_reads_visible_messages_and_tool_parts_in_order() {
        use crate::agent::export::Event;
        let home = temp_home("history");
        let db = home.join("db.sqlite");
        let conn = rusqlite::Connection::open(&db).unwrap();
        // Rows shaped like ZCode 0.17.0's store: an injected reminder, a real prompt, and a tool turn.
        conn.execute_batch(r#"
            CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT, sequence INTEGER);
            CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT, sequence INTEGER);
            INSERT INTO message VALUES ('m0','s',1,1,'{"role":"user","time":{"created":1792981439350},"semantics":{"uiVisibility":"visible"}}',0);
            INSERT INTO message VALUES ('m1','s',2,2,'{"role":"user","metadata":{"visibility":"model-only"}}',1);
            INSERT INTO message VALUES ('m2','s',3,3,'{"role":"assistant","time":{"created":1792981439356}}',2);
            INSERT INTO part VALUES ('p0','m0','s',1,1,'{"type":"text","text":"please use a tool"}',0);
            INSERT INTO part VALUES ('p1','m1','s',2,2,'{"type":"text","text":"Available agent types"}',0);
            INSERT INTO part VALUES ('p2','m2','s',3,3,'{"type":"step-start"}',0);
            INSERT INTO part VALUES ('p3','m2','s',3,3,'{"type":"reasoning","text":"Need a tool."}',1);
            INSERT INTO part VALUES ('p4','m2','s',3,3,'{"type":"tool","callID":"call_1","tool":"Bash","state":{"status":"completed","input":{"command":"touch made.txt"},"output":"(Bash completed with no output)"}}',2);
        "#).unwrap();
        drop(conn);
        let events = history_events_in(&db, "s").unwrap();
        assert_eq!(events.len(), 4);
        assert!(matches!(&events[0], Event::User { text, ts: Some(_) } if text == "please use a tool"));
        assert!(matches!(&events[1], Event::Thinking { text, .. } if text == "Need a tool."));
        assert!(matches!(&events[2], Event::ToolUse { name, input, .. } if name == "Bash" && input["command"] == "touch made.txt"));
        assert!(matches!(&events[3], Event::ToolResult { is_error: false, text, .. } if text == "(Bash completed with no output)"));
        assert!(history_events_in(&db, "other").is_err());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn session_missing_only_when_database_proves_it() {
        let home = temp_home("db");
        let db = home.join("db.sqlite");
        assert!(!session_missing_in(&db, "sess_a"), "no database proves nothing");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE session (id TEXT PRIMARY KEY, time_deleted INTEGER);
             INSERT INTO session VALUES ('sess_live', NULL), ('sess_gone', 42);",
        )
        .unwrap();
        drop(conn);
        assert!(!session_missing_in(&db, "sess_live"));
        assert!(session_missing_in(&db, "sess_gone"));
        assert!(session_missing_in(&db, "sess_unknown"));
        let _ = std::fs::remove_dir_all(&home);
    }

    fn sample_builtin(revision: u64) -> String {
        serde_json::json!({
            "schemaVersion": 1,
            "revision": revision,
            "config": {
                "providerConfigRules": {
                    "providerRules": [{"providerId": "account:zai-individual-coding-plan"}]
                },
                "modelConfigRules": {}
            }
        })
        .to_string()
    }

    #[test]
    fn best_builtin_config_prefers_higher_revision_and_never_downgrades() {
        let home = temp_home("builtin-best");
        let shipped = home.join("shipped").join("zcode-builtin.json");
        std::fs::create_dir_all(shipped.parent().unwrap()).unwrap();
        std::fs::write(&shipped, sample_builtin(44)).unwrap();

        let platform = "darwin-aarch64";
        let origin = "https://zcode.z.ai";
        let cached = velaterm_builtin_cache_path(&home, platform, origin);
        std::fs::create_dir_all(cached.parent().unwrap()).unwrap();

        // Older or equal cache keeps the shipped file.
        std::fs::write(&cached, sample_builtin(43)).unwrap();
        assert_eq!(best_builtin_config(shipped.clone(), &home, platform, origin), shipped);
        std::fs::write(&cached, sample_builtin(44)).unwrap();
        assert_eq!(best_builtin_config(shipped.clone(), &home, platform, origin), shipped);

        // Corrupt cache is ignored.
        std::fs::write(&cached, "{not json").unwrap();
        assert_eq!(best_builtin_config(shipped.clone(), &home, platform, origin), shipped);

        // Newer VelaTerm cache wins.
        std::fs::write(&cached, sample_builtin(47)).unwrap();
        assert_eq!(best_builtin_config(shipped.clone(), &home, platform, origin), cached);

        // Even newer ZCode Desktop runtime cache wins over both.
        let runtime = home
            .join(".zcode/v2/runtime/provider")
            .join(platform)
            .join("4.0.0")
            .join(endpoint_key(origin))
            .join("zcode-builtin.json");
        std::fs::create_dir_all(runtime.parent().unwrap()).unwrap();
        std::fs::write(&runtime, sample_builtin(48)).unwrap();
        assert_eq!(best_builtin_config(shipped.clone(), &home, platform, origin), runtime);

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn builtin_validation_and_endpoint_helpers_match_zcode() {
        assert_eq!(validate_builtin_bytes(sample_builtin(47).as_bytes()), Some(47));
        assert_eq!(validate_builtin_bytes(sample_builtin(0).as_bytes()), None);
        assert_eq!(validate_builtin_bytes(br#"{"schemaVersion":2,"revision":47,"config":{"providerConfigRules":{"providerRules":[{}]},"modelConfigRules":{}}}"#), None);
        assert_eq!(validate_builtin_bytes(br#"{"schemaVersion":1,"revision":47,"config":{"providerConfigRules":{"providerRules":[]},"modelConfigRules":{}}}"#), None);

        // Matches the endpoint hash ZCode itself computes for https://zcode.z.ai.
        assert_eq!(endpoint_key("https://zcode.z.ai"), "endpoint-78d7c3bef4024722642626fe3669a799");
        assert_eq!(normalize_origin("https://zcode.z.ai/some/path?x=1").as_deref(), Some("https://zcode.z.ai"));
        assert_eq!(normalize_origin("https://user:pass@zcode.z.ai"), None);
        assert_eq!(normalize_origin("ftp://zcode.z.ai"), None);

        assert_eq!(
            validate_cdn_url("https://zcode.z.ai", "https://cdn-zcode.z.ai/zcode/config/r47.json").as_deref(),
            Some("https://cdn-zcode.z.ai/zcode/config/r47.json"),
        );
        assert_eq!(validate_cdn_url("https://zcode.z.ai", "http://cdn-zcode.z.ai/r47.json"), None);
        assert_eq!(validate_cdn_url("http://127.0.0.1:3000", "http://127.0.0.1:3000/r47.json").as_deref(), Some("http://127.0.0.1:3000/r47.json"));
        assert_eq!(validate_cdn_url("https://zcode.z.ai", "https://u:p@cdn-zcode.z.ai/r47.json"), None);
    }
}
