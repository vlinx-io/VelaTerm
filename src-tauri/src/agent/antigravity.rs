//! Generation and installation of status and Stop-park hooks for Google Antigravity CLI (`agy`).
//!
//! Antigravity hooks support only fixed global/project JSON files, with no inline or runtime-directory
//! injection like Claude, OpenCode, or Cline. To avoid modifying repositories, merge into the global user
//! `hooks.json`, as with Cursor:
//!
//! - On Unix, install a static script under `~/.gemini/vlx-term/`. On Windows, call the VelaTerm executable's
//!   native `--agent-hook antigravity` bridge instead, so no PowerShell script is involved.
//! - Replace only the namespaced top-level `vlx-term-status` group in global hooks.json, preserving all user
//!   groups and refusing a nonobject root.
//! - `PreInvocation` and `PostInvocation` forward stdin to `/hook/<sid>?t=<token>&e=<state>` with a short
//!   timeout and print `{}` (matching Antigravity's protojson `PreInvocationHookResult` /
//!   `PostInvocationHookResult`, which have no `decision` field).
//! - `Stop` forwards stdin to `/agy-stop/<sid>?t=<token>` with a long execution budget (`timeout: 86400`)
//!   so live Antigravity ACP sessions can keep their `session/prompt` stream parked between turns and wake on
//!   background tasks with `{"decision":"continue"}`, while TUI and unmanaged sessions immediately receive
//!   `{"decision":"allow"}`.
//! - Missing environment variables in unmanaged sessions produce a harmless `{}` or `{"decision":"allow"}`.
//! - Install lazily when an Antigravity TUI or Chat session launches.
//!
//! Each payload carries top-level `conversationId` (protojson camelCase), the `--conversation=<id>` resume
//! anchor persisted by `server.rs`, plus `transcriptPath`. No payload carries the user's prompt, so the
//! automatic first-message rename reads it from that transcript; see [`first_prompt_from_transcript`].

use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Monotonic epoch bumped whenever Antigravity credentials are refreshed, replaced, or signed out.
static AUTH_EPOCH: AtomicU64 = AtomicU64::new(1);
/// Monotonic id for active auth-gate leases so an expired lease never clears a newer holder.
static NEXT_GATE_ID: AtomicU64 = AtomicU64::new(1);

/// Maximum time one `agy` process may hold the credential startup gate before letting peers proceed.
/// Slightly above `agy`'s internal 10s `keyringAuth` timeout so an unresponsive child cannot stall peers.
#[cfg(not(test))]
const DEFAULT_AUTH_GATE_LEASE: Duration = Duration::from_secs(12);
#[cfg(test)]
const DEFAULT_AUTH_GATE_LEASE: Duration = Duration::from_millis(500);

/// Reuse a completed `session/list` readiness answer across simultaneous pane focus checks.
const READINESS_CACHE_TTL: Duration = Duration::from_secs(10);

#[derive(Default)]
struct ScopeGate {
    active: Option<(u64, Instant)>,
}

struct ScopeGateEntry {
    state: Mutex<ScopeGate>,
    cv: Condvar,
}

impl Default for ScopeGateEntry {
    fn default() -> Self {
        Self {
            state: Mutex::new(ScopeGate::default()),
            cv: Condvar::new(),
        }
    }
}

/// RAII lease serializing the initial credential-load / token-refresh window across concurrent `agy` spawns.
pub struct AuthGatePermit {
    entry: Arc<ScopeGateEntry>,
    id: u64,
}

impl Drop for AuthGatePermit {
    fn drop(&mut self) {
        let mut state = self.entry.state.lock().unwrap();
        if state.active.is_some_and(|(id, _)| id == self.id) {
            state.active = None;
            self.entry.cv.notify_one();
        }
    }
}

fn auth_gates() -> &'static Mutex<HashMap<String, Arc<ScopeGateEntry>>> {
    static GATES: OnceLock<Mutex<HashMap<String, Arc<ScopeGateEntry>>>> = OnceLock::new();
    GATES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Default)]
struct ReadinessSlot {
    in_flight: bool,
    cached: Option<(Option<bool>, Instant)>,
}

struct ReadinessEntry {
    state: Mutex<ReadinessSlot>,
    cv: Condvar,
}

impl Default for ReadinessEntry {
    fn default() -> Self {
        Self {
            state: Mutex::new(ReadinessSlot::default()),
            cv: Condvar::new(),
        }
    }
}

fn readiness_cache() -> &'static Mutex<HashMap<String, Arc<ReadinessEntry>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<ReadinessEntry>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Credential storage key shared by `agy` processes using the same binary and config directory.
pub fn auth_scope_key(bin: &str, args: &[String]) -> String {
    let mut dir_override: Option<String> = None;
    let mut cli_name: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let trimmed = arg.trim();
        if matches!(trimmed, "--gemini-dir" | "--gemini_dir" | "--app-data-dir" | "--app_data_dir") {
            if let Some(val) = iter.next() {
                dir_override = Some(val.trim().to_string());
            }
        } else if trimmed == "--cli_dir_name" {
            if let Some(val) = iter.next() {
                cli_name = Some(val.trim().to_string());
            }
        } else if let Some(val) = trimmed
            .strip_prefix("--gemini-dir=")
            .or_else(|| trimmed.strip_prefix("--gemini_dir="))
            .or_else(|| trimmed.strip_prefix("--app-data-dir="))
            .or_else(|| trimmed.strip_prefix("--app_data_dir="))
        {
            dir_override = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("--cli_dir_name=") {
            cli_name = Some(val.trim().to_string());
        }
    }
    let home = dir_override.unwrap_or_else(|| {
        crate::host::home_dir()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    format!("{}|{}|{}", bin.trim(), home, cli_name.unwrap_or_default())
}

/// Acquire the credential startup gate for `(bin, args)` so concurrent `agy` processes do not
/// contend on the system keyring or trigger duplicate OAuth token refreshes simultaneously.
pub fn acquire_auth_gate(bin: &str, args: &[String]) -> AuthGatePermit {
    acquire_auth_gate_with_lease(bin, args, DEFAULT_AUTH_GATE_LEASE)
}

pub fn acquire_auth_gate_with_lease(bin: &str, args: &[String], lease: Duration) -> AuthGatePermit {
    let scope = auth_scope_key(bin, args);
    let entry = {
        let mut map = auth_gates().lock().unwrap();
        map.entry(scope).or_default().clone()
    };
    let id = NEXT_GATE_ID.fetch_add(1, Ordering::Relaxed);
    let mut state = entry.state.lock().unwrap();
    loop {
        let now = Instant::now();
        match state.active {
            Some((_, deadline)) if now < deadline => {
                let wait = deadline.saturating_duration_since(now);
                let (next, _) = entry.cv.wait_timeout(state, wait).unwrap();
                state = next;
            }
            _ => {
                state.active = Some((id, now + lease));
                return AuthGatePermit { entry: entry.clone(), id };
            }
        }
    }
}

/// Current Antigravity credential epoch. Idle ACP processes launched under an older epoch rotate
/// before their next prompt so they pick up refreshed credentials from disk.
pub fn auth_epoch() -> u64 {
    AUTH_EPOCH.load(Ordering::Relaxed)
}

/// Advance the Antigravity credential epoch after sign-in, sign-out, or a 401 recovery refresh.
pub fn bump_auth_epoch() -> u64 {
    AUTH_EPOCH.fetch_add(1, Ordering::Relaxed) + 1
}

/// Serialize tests that observe or mutate the process-wide Antigravity credential epoch so a
/// concurrent epoch bump cannot trigger an unexpected idle rotation in an unrelated test.
#[cfg(test)]
pub(crate) fn test_auth_epoch_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
}

/// Clear cached readiness results when account state changes.
pub fn invalidate_readiness_cache() {
    let entries: Vec<Arc<ReadinessEntry>> = readiness_cache()
        .lock()
        .unwrap()
        .values()
        .cloned()
        .collect();
    for entry in entries {
        entry.state.lock().unwrap().cached = None;
    }
}

/// Coalesce concurrent `agy acp` readiness probes for the same credential scope and reuse fresh answers.
pub fn cached_or_probe_readiness(
    bin: &str,
    args: &[String],
    probe: impl FnOnce() -> Option<bool>,
) -> Option<bool> {
    let scope = auth_scope_key(bin, args);
    let entry = {
        let mut map = readiness_cache().lock().unwrap();
        map.entry(scope).or_default().clone()
    };
    {
        let mut state = entry.state.lock().unwrap();
        loop {
            if let Some((ans, at)) = state.cached {
                if at.elapsed() < READINESS_CACHE_TTL {
                    return ans;
                }
                state.cached = None;
            }
            if !state.in_flight {
                state.in_flight = true;
                break;
            }
            state = entry.cv.wait(state).unwrap();
        }
    }
    struct ResetInFlight<'a>(&'a ReadinessEntry, Option<Option<bool>>);
    impl Drop for ResetInFlight<'_> {
        fn drop(&mut self) {
            let mut state = self.0.state.lock().unwrap();
            state.in_flight = false;
            if let Some(ans) = self.1 {
                if ans.is_some() {
                    state.cached = Some((ans, Instant::now()));
                }
            }
            self.0.cv.notify_all();
        }
    }
    let mut guard = ResetInFlight(&entry, None);
    let ans = probe();
    guard.1 = Some(ans);
    ans
}

/// Namespaced top-level hooks.json group; all other groups remain untouched.
const GROUP: &str = "vlx-term-status";

/// Lines scanned at the head of a transcript while looking for the first user step. The user input is
/// step 0 in practice; a small budget keeps a long transcript from being read on every hook call.
const TRANSCRIPT_SCAN_LINES: usize = 32;

/// Whether an id is a native conversation identifier, never a path supplied by a client.
fn native_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 200 && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

pub(crate) fn is_native_id(id: &str) -> bool {
    native_id(id)
}

/// Apply standard runtime environment hardening to an `agy` child process:
/// disable in-place CLI auto-updates during active sessions, keep Python stdio unbuffered, and on
/// Linux supply a verified system CA bundle when `SSL_CERT_FILE` / `SSL_CERT_DIR` are unset so the
/// bundled Python runtime does not fail TLS handshakes on distributions with non-default cert paths.
pub fn prepare_command_env(cmd: &mut std::process::Command) {
    let has_env = |cmd: &std::process::Command, key: &str| -> bool {
        if let Some((_, val)) = cmd.get_envs().find(|(k, _)| *k == key) {
            return val.is_some_and(|v| !v.is_empty());
        }
        std::env::var_os(key).is_some_and(|v| !v.is_empty())
    };
    if !has_env(cmd, "AGY_CLI_DISABLE_AUTO_UPDATE") {
        cmd.env("AGY_CLI_DISABLE_AUTO_UPDATE", "1");
    }
    if !has_env(cmd, "PYTHONUNBUFFERED") {
        cmd.env("PYTHONUNBUFFERED", "1");
    }
    #[cfg(target_os = "linux")]
    if !has_env(cmd, "SSL_CERT_FILE") && !has_env(cmd, "SSL_CERT_DIR") {
        let nix_ca = std::env::var("NIX_SSL_CERT_FILE").ok();
        let candidates = [
            nix_ca.as_deref(),
            Some("/etc/ssl/certs/ca-bundle.crt"),
            Some("/etc/ssl/certs/ca-certificates.crt"),
            Some("/etc/pki/tls/certs/ca-bundle.crt"),
            Some("/etc/ssl/ca-bundle.pem"),
            Some("/etc/ssl/cert.pem"),
        ];
        for candidate in candidates.into_iter().flatten() {
            let path = Path::new(candidate);
            if path.is_absolute() && path.is_file() {
                cmd.env("SSL_CERT_FILE", path);
                break;
            }
        }
    }
}

/// Resolve an Antigravity conversation's `.system_generated` directory from a native id.
pub fn system_dir(id: &str) -> Option<PathBuf> {
    if !native_id(id) {
        return None;
    }
    Some(crate::host::home_dir()?.join(".gemini/antigravity-cli/brain").join(id).join(".system_generated"))
}

/// Resolve only a native conversation identifier, never a path supplied by a client.
pub fn transcript_path(id: &str) -> Option<PathBuf> {
    let path = system_dir(id)?.join("logs/transcript_full.jsonl");
    path.is_file().then_some(path)
}

/// Antigravity's per-conversation step store, which records when each model response started and
/// finished. Only `antigravity_stats` reads it, to estimate output speed.
pub fn conversation_db_path(id: &str) -> Option<PathBuf> {
    if !native_id(id) {
        return None;
    }
    let path = crate::host::home_dir()?
        .join(".gemini/antigravity-cli/conversations")
        .join(format!("{id}.db"));
    path.is_file().then_some(path)
}

pub fn read_transcript(id: &str) -> Result<String, String> {
    let path = transcript_path(id).ok_or("Antigravity transcript file not found")?;
    crate::agent::history_store::compat::read_path(&path)
}

/// First user message of an Antigravity conversation, read from the transcript a hook payload names.
///
/// Unlike Claude or Cursor, Antigravity hook payloads carry no prompt text: `PreInvocation`,
/// `PostInvocation`, and `Stop` all deliver only conversation metadata. Every payload does carry
/// `transcriptPath`, and the transcript's first `USER_INPUT` step holds the submitted message wrapped in
/// `<USER_REQUEST>` tags, next to metadata blocks that must not become the session name. Reading it lets
/// Antigravity share the automatic first-message rename that `server.rs` performs for every other agent.
///
/// Returns None when the body is not an Antigravity payload, the transcript is not yet written, or the
/// first user step has not been flushed; the caller simply retries on the session's next hook event.
pub fn first_prompt_from_transcript(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let path = v.get("transcriptPath").and_then(|p| p.as_str())?.trim();
    if path.is_empty() {
        return None;
    }
    first_user_request(Path::new(path))
}

/// Scan the head of a transcript for the first `USER_INPUT` step and return its request text.
fn first_user_request(transcript: &Path) -> Option<String> {
    let file = std::fs::File::open(transcript).ok()?;
    for line in std::io::BufReader::new(file)
        .lines()
        .take(TRANSCRIPT_SCAN_LINES)
        .map_while(Result::ok)
    {
        let Ok(step) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if step.get("type").and_then(|t| t.as_str()) != Some("USER_INPUT") {
            continue;
        }
        if let Some(text) = step
            .get("content")
            .and_then(|c| c.as_str())
            .and_then(user_request_text)
        {
            return Some(text);
        }
    }
    None
}

/// Extract the text between the `<USER_REQUEST>` tags, dropping the metadata blocks that follow it.
fn user_request_text(content: &str) -> Option<String> {
    const OPEN: &str = "<USER_REQUEST>";
    const CLOSE: &str = "</USER_REQUEST>";
    let start = content.find(OPEN)? + OPEN.len();
    let end = start + content[start..].find(CLOSE)?;
    let text = content[start..end].trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Static POSIX script where `$1` is `working`, `waiting`, or `stop`.
///
/// `PreInvocation` (`working`) and `PostInvocation` (`waiting`) forward stdin asynchronously within 3s and
/// print `{}` as required by Antigravity's protojson schemas. `Stop` (`stop`) posts to `/agy-stop/<sid>`
/// and forwards the JSON decision (`allow` or `continue`), failing open to `{"decision":"allow"}` when
/// unmanaged or disconnected.
const SH_SCRIPT: &str = r#"#!/bin/sh
# vlx-term status and Stop-park bridge hook (auto-installed by vlx-term; safe to delete).
# Only active in Antigravity (agy) sessions launched by vlx-term (reads VLX_* env vars).
EVENT="${1:-waiting}"
TOK="${VLX_TOKEN:-$VLX_AUTH}"
BASE="${VLX_SPAWN_URL%/}"
if [ "$EVENT" = "stop" ]; then
  if [ -z "$BASE" ] || [ -z "$VLX_SESSION_ID" ] || [ -z "$TOK" ]; then
    cat >/dev/null 2>&1
    printf '{"decision":"allow"}\n'
    exit 0
  fi
  URL="$BASE/agy-stop/$VLX_SESSION_ID?t=$TOK"
  if command -v curl >/dev/null 2>&1; then
    if OUT=$(curl -fsS --connect-timeout 2 -m 86400 -X POST -H "Content-Type: application/json" --data-binary @- "$URL" 2>/dev/null) \
       && printf '%s' "$OUT" | grep -q '"decision"'; then
      printf '%s\n' "$OUT"
    else
      printf '{"decision":"allow"}\n'
    fi
  elif command -v wget >/dev/null 2>&1; then
    BODY=$(cat)
    if OUT=$(wget -qO- -T 86400 --header="Content-Type: application/json" --post-data="$BODY" "$URL" 2>/dev/null) \
       && printf '%s' "$OUT" | grep -q '"decision"'; then
      printf '%s\n' "$OUT"
    else
      printf '{"decision":"allow"}\n'
    fi
  else
    cat >/dev/null 2>&1
    printf '{"decision":"allow"}\n'
  fi
  exit 0
fi
if [ -z "$BASE" ] || [ -z "$VLX_SESSION_ID" ] || [ -z "$TOK" ]; then
  cat >/dev/null 2>&1
  printf '{}\n'
  exit 0
fi
if command -v curl >/dev/null 2>&1; then
  curl -s -m 3 -X POST -H "Content-Type: application/json" --data-binary @- \
    "$BASE/hook/$VLX_SESSION_ID?t=$TOK&e=$EVENT" >/dev/null 2>&1
elif command -v wget >/dev/null 2>&1; then
  BODY=$(cat)
  wget -qO- -T 3 --header="Content-Type: application/json" --post-data="$BODY" \
    "$BASE/hook/$VLX_SESSION_ID?t=$TOK&e=$EVENT" >/dev/null 2>&1
else
  cat >/dev/null 2>&1
fi
printf '{}\n'
exit 0
"#;

/// `(Antigravity event, script/bridge argument, timeout seconds)` entries to register.
const EVENTS: [(&str, &str, u64); 3] = [
    ("PreInvocation", "working", 10),
    ("PostInvocation", "waiting", 10),
    ("Stop", "stop", 86400),
];

/// Gemini configuration root: `~/.gemini`.
fn gemini_home() -> Option<PathBuf> {
    crate::host::home_dir().map(|h| h.join(".gemini"))
}

/// Unix hook script path: `~/.gemini/vlx-term/hook.sh`. Earlier Windows versions wrote `hook.ps1` beside it.
fn hook_script_path() -> Option<PathBuf> {
    gemini_home().map(|h| h.join("vlx-term").join("hook.sh"))
}

/// Verified global hooks path: `~/.gemini/config/hooks.json`. Embedded docs and Orca confirm it; the older
/// community-reported antigravity-cli path is not recognized and is never written.
fn hooks_json_path() -> Option<PathBuf> {
    gemini_home().map(|h| h.join("config").join("hooks.json"))
}

/// Build the hooks.json command for `target`: execute the shebang script directly on Unix, or call the
/// VelaTerm executable's native bridge on Windows. Antigravity does not document which shell runs hooks on
/// Windows, so the bridge command uses the form cmd.exe, PowerShell, and POSIX shells all parse alike.
fn hook_command(target: &Path, event: &str) -> String {
    if cfg!(windows) {
        let args = format!("--agent-hook antigravity {event}");
        super::hook_bridge::command(target, super::hook_bridge::Shell::Any, &args)
    } else {
        format!("{} {event}", target.display())
    }
}

/// Construct our flat event group without nested hooks or matcher, specifying explicit handler timeouts.
fn build_group(target: &Path) -> serde_json::Value {
    let mut group = serde_json::Map::new();
    for (event, arg, timeout) in EVENTS {
        group.insert(
            event.to_string(),
            serde_json::json!([
                {
                    "type": "command",
                    "command": hook_command(target, arg),
                    "timeout": timeout
                }
            ]),
        );
    }
    serde_json::Value::Object(group)
}

/// Merge only `root["vlx-term-status"]` in place, preserving other groups; reject a nonobject root.
fn merge_into(root: &mut serde_json::Value, target: &Path) -> Result<(), String> {
    let obj = root
        .as_object_mut()
        .ok_or("hooks.json root is not a JSON object")?;
    obj.insert(GROUP.to_string(), build_group(target));
    Ok(())
}

/// Install the bridge, merge global hooks.json, and return its path. Unix writes the hook script; Windows
/// points the hooks at `exe` and removes the PowerShell script earlier versions wrote.
///
/// Idempotently preserve matching content/mtime. Missing home, permissions, or malformed JSON returns Err
/// for logging; Antigravity still starts with screen-detected state only.
pub fn install(exe: &Path) -> Result<PathBuf, String> {
    let script = hook_script_path().ok_or("Failed to resolve user home directory")?;
    let hooks = hooks_json_path().ok_or("Failed to resolve user home directory")?;
    install_at(&script, &hooks, exe)
}

/// As `install`, with explicit target paths for tests.
fn install_at(script: &Path, hooks_json: &Path, exe: &Path) -> Result<PathBuf, String> {
    if cfg!(windows) {
        let _ = std::fs::remove_file(script.with_file_name("hook.ps1"));
        merge_one(exe, hooks_json)?;
        return Ok(hooks_json.to_path_buf());
    }
    // 1. Preserve matching script content and make Unix files executable.
    let body = SH_SCRIPT;
    let script_current = std::fs::read_to_string(script)
        .map(|s| s == body)
        .unwrap_or(false);
    if !script_current {
        if let Some(parent) = script.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create script directory: {e}"))?;
        }
        std::fs::write(script, body).map_err(|e| format!("Failed to write hook script: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Failed to set script executable bit: {e}"))?;
        }
    }

    // 2. Read hooks.json or an empty object, merge, and write only changes; reject invalid JSON unchanged.
    merge_one(script, hooks_json)?;
    Ok(hooks_json.to_path_buf())
}

/// Read, merge, and write one hooks.json only on change; reject invalid JSON without overwriting.
fn merge_one(target: &Path, hooks_json: &Path) -> Result<(), String> {
    let existing = std::fs::read_to_string(hooks_json).ok();
    let mut root: serde_json::Value = match &existing {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text).map_err(|_| {
            format!(
                "{} is not valid JSON; not written (please check manually)",
                hooks_json.display()
            )
        })?,
        _ => serde_json::json!({}),
    };
    merge_into(&mut root, target)?;
    let content = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize hooks.json: {e}"))?;
    if existing.as_deref() != Some(content.as_str()) {
        if let Some(parent) = hooks_json.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create hooks.json directory: {e}"))?;
        }
        std::fs::write(hooks_json, &content)
            .map_err(|e| format!("Failed to write hooks.json: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vlx-antigravity-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Real CLI payload shape: metadata only, naming the transcript that holds the prompt.
    fn payload(transcript: &Path) -> String {
        serde_json::json!({
            "conversationId": "f1d8a47f-6de3-4db2-87ea-6908fd1e7697",
            "invocationNum": 0,
            "modelName": "gemini-3.7-flash-high",
            "transcriptPath": transcript.display().to_string(),
            "workspacePaths": []
        })
        .to_string()
    }

    /// One transcript line, matching the captured CLI shape: the request sits in `<USER_REQUEST>` tags and
    /// is followed by metadata blocks that must not leak into the session name.
    fn user_step(request: &str) -> String {
        serde_json::json!({
            "step_index": 0,
            "source": "USER_EXPLICIT",
            "type": "USER_INPUT",
            "status": "DONE",
            "content": format!(
                "<USER_REQUEST>\n{request}\n</USER_REQUEST>\n<ADDITIONAL_METADATA>\nThe current local time is: 2026-08-26T10:05:14+08:00.\n</ADDITIONAL_METADATA>"
            )
        })
        .to_string()
    }

    /// A system step, which never names a session.
    fn checkpoint_step() -> String {
        serde_json::json!({
            "step_index": 1,
            "source": "SYSTEM",
            "type": "CHECKPOINT",
            "status": "DONE",
            "content": "the earlier parts of this conversation have been truncated"
        })
        .to_string()
    }

    #[test]
    fn first_prompt_reads_the_user_request_from_the_transcript() {
        let dir = tmp_dir("transcript");
        let transcript = dir.join("transcript_full.jsonl");
        std::fs::write(
            &transcript,
            format!("{}\n{}\n", user_step("Fix the login page styling"), checkpoint_step()),
        )
        .unwrap();

        assert_eq!(
            first_prompt_from_transcript(&payload(&transcript)).as_deref(),
            Some("Fix the login page styling")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_prompt_skips_leading_non_user_steps_and_trims() {
        let dir = tmp_dir("transcript-skip");
        let transcript = dir.join("transcript_full.jsonl");
        std::fs::write(
            &transcript,
            format!("{}\n{}\n", checkpoint_step(), user_step("  Rewrite the article  ")),
        )
        .unwrap();

        assert_eq!(
            first_prompt_from_transcript(&payload(&transcript)).as_deref(),
            Some("Rewrite the article")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_prompt_is_none_until_a_user_request_exists() {
        let dir = tmp_dir("transcript-empty");
        let transcript = dir.join("transcript_full.jsonl");
        // A transcript the CLI has not written yet: the caller retries on the session's next hook event.
        assert!(first_prompt_from_transcript(&payload(&transcript)).is_none());

        // System steps only, plus a blank request, leave nothing worth renaming with.
        std::fs::write(
            &transcript,
            format!("{}\n{}\n", checkpoint_step(), user_step("   ")),
        )
        .unwrap();
        assert!(first_prompt_from_transcript(&payload(&transcript)).is_none());

        // Payloads from the other agents never name a transcript.
        assert!(first_prompt_from_transcript(
            r#"{"hook_event_name":"UserPromptSubmit","prompt":"hi"}"#
        )
        .is_none());
        assert!(first_prompt_from_transcript("not json").is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Executable path handed to `install_at`; only Windows embeds it in the hook commands.
    const EXE: &str = "/opt/VelaTerm/velaterm";

    #[cfg(unix)]
    #[test]
    fn fresh_install_writes_script_and_group() {
        let dir = tmp_dir("fresh");
        let script = dir.join("vlx-term/hook.sh");
        let hooks = dir.join("config/hooks.json");

        install_at(&script, &hooks, Path::new(EXE)).expect("installation should succeed");

        // The script is installed and executable on Unix.
        assert!(std::fs::read_to_string(&script)
            .unwrap()
            .contains("VLX_SESSION_ID"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&script).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "the script should be executable");
        }

        // hooks.json contains our group and three flat event mappings.
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&hooks).unwrap()).unwrap();
        let pre = v[GROUP]["PreInvocation"][0]["command"].as_str().unwrap();
        let post = v[GROUP]["PostInvocation"][0]["command"].as_str().unwrap();
        let stop = v[GROUP]["Stop"][0]["command"].as_str().unwrap();
        assert!(
            pre.ends_with(" working"),
            "PreInvocation should report working: {pre}"
        );
        assert!(
            post.ends_with(" waiting"),
            "PostInvocation should report waiting: {post}"
        );
        assert!(stop.ends_with(" stop"), "Stop should call stop handler: {stop}");
        assert_eq!(v[GROUP]["PreInvocation"][0]["timeout"], 10);
        assert_eq!(v[GROUP]["PostInvocation"][0]["timeout"], 10);
        assert_eq!(v[GROUP]["Stop"][0]["timeout"], 86400);
        assert_eq!(v[GROUP]["PreInvocation"][0]["type"], "command");
        // Invocation events are flat, without nested hooks or matcher, matching Orca production form.
        assert!(
            v[GROUP]["PreInvocation"][0].get("hooks").is_none(),
            "it should not wrap the entries in a hooks sub-array"
        );
        assert!(
            v[GROUP]["PreInvocation"][0].get("matcher").is_none(),
            "invocation-level events carry no matcher"
        );

        // Verify shell script outputs `{}` for working/waiting and `{"decision":"allow"}` fallback for stop.
        let run_script = |arg: &str, body: &str| {
            let mut child = std::process::Command::new("/bin/sh")
                .arg(&script)
                .arg(arg)
                .env_remove("VLX_SPAWN_URL")
                .env_remove("VLX_SESSION_ID")
                .env_remove("VLX_TOKEN")
                .env_remove("VLX_AUTH")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            use std::io::Write;
            child.stdin.take().unwrap().write_all(body.as_bytes()).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success());
            String::from_utf8(out.stdout).unwrap()
        };
        assert_eq!(run_script("working", "{}").trim(), "{}");
        assert_eq!(run_script("waiting", "{}").trim(), "{}");
        assert_eq!(run_script("stop", "{}").trim(), r#"{"decision":"allow"}"#);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_install_calls_the_native_bridge_and_removes_the_script() {
        let dir = tmp_dir("native");
        let script = dir.join("vlx-term/hook.sh");
        let hooks = dir.join("config/hooks.json");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(script.with_file_name("hook.ps1"), "# old bridge").unwrap();
        let exe = dir.join("velaterm.exe");
        install_at(&script, &hooks, &exe).expect("installation should succeed");
        assert!(!script.with_file_name("hook.ps1").exists(), "the earlier PowerShell bridge should be removed");
        assert!(!script.exists(), "Windows writes no hook script");
        let raw = std::fs::read_to_string(&hooks).unwrap();
        assert!(!raw.to_ascii_lowercase().contains("powershell"), "{raw}");
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let pre = v[GROUP]["PreInvocation"][0]["command"].as_str().unwrap();
        assert!(pre.ends_with(" --agent-hook antigravity working"), "{pre}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn install_is_idempotent() {
        let dir = tmp_dir("idem");
        let script = dir.join("vlx-term/hook.sh");
        let hooks = dir.join("config/hooks.json");

        install_at(&script, &hooks, Path::new(EXE)).unwrap();
        let mtime1 = std::fs::metadata(&hooks).unwrap().modified().unwrap();
        let smtime1 = std::fs::metadata(&script).unwrap().modified().unwrap();
        install_at(&script, &hooks, Path::new(EXE)).unwrap();
        assert_eq!(
            std::fs::metadata(&hooks).unwrap().modified().unwrap(),
            mtime1,
            "hooks.json should not be rewritten when the contents are identical"
        );
        assert_eq!(
            std::fs::metadata(&script).unwrap().modified().unwrap(),
            smtime1,
            "the script should not be rewritten when the contents are identical"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_preserves_other_top_level_groups() {
        let dir = tmp_dir("preserve");
        let script = dir.join("vlx-term/hook.sh");
        let hooks = dir.join("config/hooks.json");
        std::fs::create_dir_all(hooks.parent().unwrap()).unwrap();
        // Seed user groups and an old VelaTerm group to be replaced as a whole.
        std::fs::write(
            &hooks,
            r#"{
  "user-linter": { "PostToolUse": [ { "matcher": "write", "hooks": [ { "command": "/my/lint.sh" } ] } ] },
  "vlx-term-status": { "PreInvocation": [ { "hooks": [ { "command": "/old/hook.sh working" } ] } ] }
}"#,
        )
        .unwrap();

        install_at(&script, &hooks, Path::new(EXE)).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&hooks).unwrap()).unwrap();

        // User groups remain unchanged.
        assert_eq!(
            v["user-linter"]["PostToolUse"][0]["hooks"][0]["command"],
            "/my/lint.sh"
        );
        // Our group now points to the current script and no longer contains the old path.
        let pre = v[GROUP]["PreInvocation"][0]["command"].as_str().unwrap();
        #[cfg(unix)]
        assert!(pre.starts_with(&script.display().to_string()) || pre.contains("hook.sh"));
        assert!(!pre.contains("/old/hook.sh"), "the old vlx group should be replaced wholesale");
        assert!(v[GROUP].get("PostInvocation").is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_refuses_to_clobber_invalid_json() {
        let dir = tmp_dir("invalid");
        let script = dir.join("vlx-term/hook.sh");
        let hooks = dir.join("config/hooks.json");
        std::fs::create_dir_all(hooks.parent().unwrap()).unwrap();
        std::fs::write(&hooks, "{ not json").unwrap();

        // Invalid JSON returns Err and leaves the original file unchanged.
        assert!(install_at(&script, &hooks, Path::new(EXE)).is_err(), "invalid JSON should be refused rather than written");
        assert_eq!(
            std::fs::read_to_string(&hooks).unwrap(),
            "{ not json",
            "invalid JSON should be left untouched"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auth_gate_serializes_same_scope_and_allows_distinct_scopes() {
        assert_eq!(
            auth_scope_key("/bin/agy", &["--gemini-dir=/tmp/g1".into(), "--sandbox".into()]),
            auth_scope_key("/bin/agy", &["--gemini-dir".into(), "/tmp/g1".into()])
        );
        assert_ne!(
            auth_scope_key("/bin/agy", &["--gemini-dir=/tmp/g1".into()]),
            auth_scope_key("/bin/agy", &["--gemini-dir=/tmp/g2".into()])
        );

        let scope_args = vec!["--gemini-dir=/tmp/vlx-gate-scope-a".to_string()];
        let other_args = vec!["--gemini-dir=/tmp/vlx-gate-scope-b".to_string()];
        let permit1 = acquire_auth_gate_with_lease("/mock/agy-gate", &scope_args, Duration::from_secs(2));

        // A distinct credential directory must not wait behind `permit1`.
        let other_start = Instant::now();
        let other_permit = acquire_auth_gate_with_lease("/mock/agy-gate", &other_args, Duration::from_secs(2));
        assert!(other_start.elapsed() < Duration::from_millis(100));
        drop(other_permit);

        // A second caller on the same credential scope must wait until `permit1` drops.
        let args_clone = scope_args.clone();
        let waiter = std::thread::spawn(move || {
            let start = Instant::now();
            let _permit2 = acquire_auth_gate_with_lease("/mock/agy-gate", &args_clone, Duration::from_secs(2));
            start.elapsed()
        });
        std::thread::sleep(Duration::from_millis(80));
        drop(permit1);
        let waited = waiter.join().unwrap();
        assert!(waited >= Duration::from_millis(60), "expected same-scope caller to block, waited {waited:?}");
    }

    #[test]
    fn readiness_probe_coalesces_concurrent_callers_and_honors_invalidation() {
        use std::sync::atomic::AtomicUsize;

        let calls = Arc::new(AtomicUsize::new(0));
        let args = vec!["--gemini-dir=/tmp/vlx-readiness-coalesce".to_string()];
        let mut handles = Vec::new();
        for _ in 0..4 {
            let calls_t = calls.clone();
            let args_t = args.clone();
            handles.push(std::thread::spawn(move || {
                cached_or_probe_readiness("/mock/agy-probe", &args_t, || {
                    calls_t.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(60));
                    Some(true)
                })
            }));
        }
        for h in handles {
            assert_eq!(h.join().unwrap(), Some(true));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "concurrent probes must coalesce into one subprocess");

        // Fresh cache hit skips spawning a probe.
        let cached = cached_or_probe_readiness("/mock/agy-probe", &args, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Some(false)
        });
        assert_eq!(cached, Some(true));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // Invalidating the cache forces the next probe to execute.
        invalidate_readiness_cache();
        let refreshed = cached_or_probe_readiness("/mock/agy-probe", &args, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Some(false)
        });
        assert_eq!(refreshed, Some(false));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
