//! Local loopback HTTP service for status callbacks from official agent hooks and Codex notify.
//!
//! - Binds only to `127.0.0.1` on a random port with a per-process random token.
//! - `/hook/<sid>?t=<token>&e=<event>` identifies the session directly, rejects forged callbacks
//!   from other local processes, and maps the event to `StatusSignal`.
//! - Emits `pty://status/{sid}` and returns 200 immediately so the agent is never delayed.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::host::AppCtx;
use crate::pty::{AgentState, StatusSignal};

/// Request to spawn an independent child session, triggered by `vspawn` or Claude's `/vspawn` skill.
/// `/spawn` parses and emits it so the frontend can create the child, open a worktree, start it,
/// and submit the prompt.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnRequest {
    /// Parent session ID, obtained by the skill from `VLX_SESSION_ID`.
    pub parent_session_id: String,
    /// Self-contained task description for the child session.
    pub prompt: String,
    /// Child kind (claude/codex/terminal); the frontend chooses a default when omitted.
    #[serde(default)]
    pub kind: Option<String>,
    /// Whether to create a dedicated Git worktree; the frontend defaults to true.
    #[serde(default)]
    pub worktree: Option<bool>,
    /// Structured model selection, persisted on the child and translated to agent flags at launch.
    #[serde(default)]
    pub model: Option<String>,
    /// Structured reasoning-effort selection; handled like `model`.
    #[serde(default)]
    pub effort: Option<String>,
    /// Child session name; the frontend derives one from the prompt when omitted.
    #[serde(default)]
    pub name: Option<String>,
    /// Raw launch arguments; when omitted the frontend applies the per-agent defaults.
    #[serde(default)]
    pub agent_args: Option<String>,
    /// Permission mode for the child. An omitted value inherits the parent's mode.
    #[serde(default)]
    pub permission_mode: Option<String>,
    /// Whether `/orch` may launch the child without the confirmation card.
    #[serde(default)]
    pub auto_approve: bool,
    /// Correlation id for `vagent spawn`. When set, the frontend reports the created session (or
    /// failure) back through the `spawn_result` command so the parked CLI request can answer.
    #[serde(default)]
    pub request_id: Option<String>,
    /// Set when the spawn crosses the confirmation threshold.
    #[serde(default)]
    pub force_confirm: Option<bool>,
}

/// Request from `view <file|URL>` to open a tab.
/// `/view` validates and emits it. Relative files are resolved against cwd, canonicalized, and must
/// exist as regular files; HTTP(S) URLs open a built-in browser tab (desktop only; see section 17).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewRequest {
    /// Originating session ID for diagnostics; currently unused by the frontend.
    pub session_id: String,
    /// Canonical **absolute path**, suitable as a docTabs deduplication key; when `is_url` is true,
    /// this contains the original HTTP(S) URL instead.
    pub path: String,
    /// Whether path is a URL, in which case the frontend opens a browser rather than document tab.
    #[serde(default)]
    pub is_url: bool,
}

/// Raw `/view` POST body sent by the script; path may be relative.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewBody {
    session_id: String,
    path: String,
    cwd: String,
}

/// Server endpoint (port and token) required during spawn injection.
#[derive(Clone)]
pub struct HookEndpoint {
    pub port: u16,
    pub token: String,
}

/// Local hook-service handle, injected as Tauri managed state in GUI mode and retained by the
/// process host in headless mode. Spawn embeds its port/token into injected arguments.
pub struct HookServer {
    pub port: u16,
    pub token: String,
}

impl HookServer {
    /// Start on a random `127.0.0.1` port and receive requests on a blocking background thread.
    pub fn start(app: AppCtx) -> Result<Self, String> {
        let server = tiny_http::Server::http("127.0.0.1:0")
            .map_err(|e| format!("Failed to start local hook server: {e}"))?;
        let port = server
            .server_addr()
            .to_ip()
            .ok_or("Failed to get hook server port")?
            .port();
        let token = Uuid::new_v4().to_string();

        let token_for_thread = token.clone();
        std::thread::spawn(move || serve_loop(server, app, token_for_thread));

        Ok(Self { port, token })
    }

    /// Return the endpoint used for injection.
    pub fn endpoint(&self) -> HookEndpoint {
        HookEndpoint {
            port: self.port,
            token: self.token.clone(),
        }
    }
}

/// Request loop: parse, validate, map, emit, and immediately return 200.
fn serve_loop(server: tiny_http::Server, app: AppCtx, token: String) {
    let app_for_db = app.clone();
    let app_for_spawn = app.clone();
    let app_for_prompt = app.clone();
    let app_for_view = app.clone();
    let app_for_agent = app.clone();
    serve_with(
        server,
        &token,
        |sid, signal| {
            app.emit(&StatusSignal::event_name(&sid), signal);
        },
        |sid, prompt| {
            // If the first user message arrives while the session still has an automatic name such
            // as `Claude 1`, the **backend alone** condenses it into a meaningful title, writes once,
            // and broadcasts tree://changed. Previously every client received the prompt and called
            // renameNode, multiplying DB writes and tree reloads with multiple clients. Once renamed,
            // later messages no longer match the placeholder; manually chosen names are never changed.
            try_auto_rename(&app_for_prompt, &sid, &prompt);
        },
        |sid, agent_session_id| {
            // Persist the agent's session_id from the hook body for exact future resume. Store it
            // under the session's own kind: supported agent hooks return their ID in the body
            // (Cline uses top-level taskId; Crush/Codex use top-level session_id). Skip non-agent kinds.
            let changed = {
                let db = app_for_db.db();
                let Ok(conn) = db.conn.lock() else {
                    return;
                };
                match crate::db::repo::get_session_kind(&conn, &sid) {
                    Ok(Some(kind))
                        if matches!(
                            kind,
                            crate::models::SessionKind::Claude
                                | crate::models::SessionKind::Codex
                                | crate::models::SessionKind::Opencode
                                | crate::models::SessionKind::Copilot
                                | crate::models::SessionKind::Cursor
                                | crate::models::SessionKind::Antigravity
                                | crate::models::SessionKind::Cline
                                | crate::models::SessionKind::Pi
                                | crate::models::SessionKind::Crush
                                | crate::models::SessionKind::Kimi
                                | crate::models::SessionKind::Kiro
                                | crate::models::SessionKind::Grok
                        ) =>
                    {
                        crate::db::repo::set_agent_session_id(&conn, &sid, &agent_session_id, kind)
                            .unwrap_or(false)
                    }
                    _ => false,
                }
            };
            // Broadcast a tree reload only on first capture or value change. agentSessionId controls
            // frontend Fork/export availability, so every client needs the updated session data.
            if changed {
                app_for_db.emit(crate::host::TREE_CHANGED, ());
            }
        },
        |req| {
            // Forward an in-session child-task request so the frontend can create and start it.
            app_for_spawn.emit("spawn://request", req);
        },
        |req| {
            // Forward an in-session `view <file>` request to open a document tab; ws.rs relays it to browsers.
            app_for_view.emit("view://request", req);
        },
        Some(app_for_agent),
    );
}

/// Rename an automatically numbered placeholder once from the first user message.
///
/// Hooks and Codex rollout capture share this atomic check. User names are preserved, and if both
/// paths arrive concurrently only the first holder of the database lock can rename.
pub(crate) fn try_auto_rename(app: &AppCtx, sid: &str, prompt: &str) -> bool {
    let title = condense_title(prompt);
    if title.is_empty() {
        return false;
    }
    let renamed = {
        let db = app.db();
        let Ok(conn) = db.conn.lock() else {
            return false;
        };
        match crate::db::repo::get_session_name(&conn, sid) {
            Ok(Some(name)) if is_auto_name(&name) => {
                crate::db::repo::rename_node(&conn, crate::models::NodeKind::Session, sid, &title)
                    .is_ok()
            }
            _ => false,
        }
    };
    if renamed {
        app.emit(crate::host::TREE_CHANGED, ());
    }
    renamed
}

/// Suppresses Codex mid-turn callbacks that arrive after their turn already ended.
///
/// Every Codex hook runs in its own short-lived process, so the order in which their HTTP callbacks
/// reach this service is not the order in which Codex fired them. A `PreToolUse` issued just before a
/// turn ends can land after `Stop`, and because Codex sends no further event, the session would display
/// working until the user types again.
///
/// Only mid-turn working (`codex_tool`) is dropped, and only inside a short window after a turn ended.
/// `codex_working` comes from UserPromptSubmit and always opens a new turn, so a user who replies the
/// instant a turn finishes is never suppressed.
#[derive(Default)]
struct CodexTurnGuard {
    /// Time each session's most recent turn ended, cleared when a new turn opens.
    ended: HashMap<String, Instant>,
}

impl CodexTurnGuard {
    /// Window covering hook-process startup jitter; well above the observed spread and far below the
    /// time a user needs to read a reply and respond.
    const WINDOW: Duration = Duration::from_millis(1500);

    /// Records turn boundaries and reports whether this event's signal may be emitted.
    fn admit(&mut self, sid: &str, event: &str, now: Instant) -> bool {
        match event {
            "codex_waiting" => {
                self.ended.insert(sid.to_string(), now);
                true
            }
            "codex_working" | "codex_asking" => {
                self.ended.remove(sid);
                true
            }
            "codex_tool" => match self.ended.get(sid) {
                Some(ended) => now.duration_since(*ended) >= Self::WINDOW,
                None => true,
            },
            _ => true,
        }
    }
}

/// Core request loop. Pass mapped status to `on_signal`, the first user message to `on_prompt` for
/// automatic naming, and the agent's parsed session_id to `on_session_id`, then return 200.
/// Callback extraction permits real-HTTP integration tests without Tauri.
#[allow(clippy::too_many_arguments)]
fn serve_with(
    server: tiny_http::Server,
    token: &str,
    mut on_signal: impl FnMut(String, StatusSignal),
    mut on_prompt: impl FnMut(String, String),
    mut on_session_id: impl FnMut(String, String),
    mut on_spawn: impl FnMut(SpawnRequest),
    mut on_view: impl FnMut(ViewRequest),
    agent_app: Option<AppCtx>,
) {
    let mut turn_guard = CodexTurnGuard::default();
    for mut request in server.incoming_requests() {
        // Copy the URL before borrowing the request to read its body.
        let url = request.url().to_string();
        // Hook, spawn, and view POST requests all carry JSON bodies.
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);

        // `/agent/<op>` runs on a worker thread because wait/spawn block; the hook loop itself must
        // never stall, or every agent status callback would queue behind one slow vagent request.
        if let Some(op) = agent_op_path(&url) {
            if query_token(&url) != Some(token.to_string()) {
                let _ = request.respond(tiny_http::Response::empty(403));
            } else if let Some(app) = agent_app.clone() {
                std::thread::spawn(move || {
                    let (status, resp) = crate::agent::ctl::handle(&op, &body, &app);
                    let _ = request.respond(
                        tiny_http::Response::from_string(resp).with_status_code(status),
                    );
                });
            } else {
                let _ = request.respond(tiny_http::Response::empty(404));
            }
            continue;
        }

        // `/view` is special: unlike always-200 routes, path validation returns a reason with 404
        // so the script can report missing files through curl -f.
        if is_view_path(&url) {
            match parse_view(&url, &body, token) {
                Ok(req) => {
                    on_view(req);
                    let _ = request.respond(tiny_http::Response::empty(200));
                }
                Err(reason) => {
                    let _ = request
                        .respond(tiny_http::Response::from_string(reason).with_status_code(404));
                }
            }
            continue;
        }

        if let Some(req) = parse_spawn(&url, &body, token) {
            // Validated request to spawn a child task from an agent session.
            on_spawn(req);
        } else if let Some((sid, signal)) = handle(&url, token) {
            // For a valid hook, capture any body session_id before reporting status.
            if let Some(agent_session_id) = parse_session_id(&body) {
                on_session_id(sid.clone(), agent_session_id);
            }
            // UserPromptSubmit carries the original message. Let the backend condense an automatic
            // session name once instead of asking every client to rename it.
            if let Some(text) = parse_first_prompt(&body) {
                on_prompt(sid.clone(), text);
            }
            // Emit an additional Tool signal from lifecycle fields so Info can show active tooling.
            if let Some(tool_signal) = parse_tool_signal(&body) {
                on_signal(sid.clone(), tool_signal);
            }
            // A valid event may have no status signal, such as Copilot `boot`, which only captures its ID.
            if let Some(signal) = signal {
                // Codex callbacks can arrive out of order; drop mid-turn working that lost the race
                // against its own turn's Stop. Every other agent passes through untouched.
                let event = parse_url(&url).map(|(_, _, e)| e).unwrap_or_default();
                if turn_guard.admit(&sid, &event, Instant::now()) {
                    on_signal(sid, signal);
                }
            }
        }

        let _ = request.respond(tiny_http::Response::empty(200));
    }
}

/// Extract an agent session ID from a hook JSON body. Support `session_id`, `sessionId`,
/// `taskId`/`task_id`, and Codex notify's `thread-id`/`thread_id`. Because hook URLs already embed
/// the VelaTerm sid, this mapping is more precise than scanning rollouts by cwd/mtime.
fn parse_session_id(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let sid = v
        .get("session_id")
        .or_else(|| v.get("sessionId"))
        .or_else(|| v.get("taskId"))
        .or_else(|| v.get("task_id"))
        .or_else(|| v.get("thread-id"))
        .or_else(|| v.get("thread_id"))?
        .as_str()?
        .trim();
    if sid.is_empty() {
        None
    } else {
        Some(sid.to_string())
    }
}

/// Extract the first user message from the various hook JSON shapes:
/// - Claude `UserPromptSubmit` and Cursor `beforeSubmitPrompt`: top-level `prompt`.
/// - Cline `prompt_submit`: nested `userPromptSubmit.prompt`.
/// - Codex `agent-turn-complete`: `input-messages[]`, so naming happens after the first turn.
///
/// Other hooks and non-JSON bodies return None. The backend uses this text to name placeholders.
fn parse_first_prompt(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    // Codex notify supplies string-array input-messages. For completed turns, use the first nonempty
    // input; later turns cannot overwrite a session name that has already changed.
    if v.get("type").and_then(|e| e.as_str()) == Some("agent-turn-complete") {
        return v
            .get("input-messages")
            .and_then(|messages| messages.as_array())
            .and_then(|messages| {
                messages
                    .iter()
                    .filter_map(|message| message.as_str())
                    .find_map(|message| non_empty_trimmed(Some(message)))
            });
    }
    // Claude/Cursor use snake-case `hook_event_name`; Grok uses camel-case `hookEventName`
    // with a snake-case value. Kiro also uses `hook_event_name`, but spells the value `userPromptSubmit`
    // to match its agent-config trigger names. All carry the submitted text in top-level `prompt`.
    // Kiro's `agentSpawn` repeats the same `prompt`, so it is deliberately excluded: only the submit event
    // should name the session.
    if matches!(
        v.get("hook_event_name").and_then(|e| e.as_str()),
        Some("UserPromptSubmit") | Some("beforeSubmitPrompt") | Some("userPromptSubmit")
    ) || matches!(
        v.get("hookEventName").and_then(|e| e.as_str()),
        Some("UserPromptSubmit") | Some("user_prompt_submit")
    ) {
        return non_empty_trimmed(v.get("prompt").and_then(|p| p.as_str()));
    }
    // Cline uses hookName `prompt_submit` with nested userPromptSubmit.prompt.
    if v.get("hookName").and_then(|e| e.as_str()) == Some("prompt_submit") {
        return non_empty_trimmed(
            v.get("userPromptSubmit")
                .and_then(|u| u.get("prompt"))
                .and_then(|p| p.as_str()),
        );
    }
    None
}

/// Return a trimmed string, or None for missing/blank values.
fn non_empty_trimmed(s: Option<&str>) -> Option<String> {
    let t = s?.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Whether a session name is still an automatically numbered placeholder such as `Claude 1` or
/// `Session 8`. Only these may be replaced by the first prompt; user names never match. This is a
/// dependency-free equivalent of the frontend AUTO_NAME regex. Prefixes include localized and
/// legacy defaults for old data. `Pi` still requires whitespace plus digits, so `Pilot 3` is safe.
fn is_auto_name(name: &str) -> bool {
    let rest = [
        "Claude",
        "Codex",
        "OpenCode",
        "Copilot",
        "Cursor",
        "Antigravity",
        "Cline",
        "Pi",
        "Crush",
        "Kimi Code",
        "Kimi",
        "Kiro",
        "Grok",
        "Grok Build",
        // Localized common.session values used as default terminal-session name prefixes.
        "Session",    // en
        "会话",       // zh-CN, retained for compatibility with existing data
        "會話",       // zh-TW
        "セッション", // ja
        "세션",       // ko
        "Sesión",     // es
        "Sessão",     // pt-BR
        "Sitzung",    // de
        "Сессия",     // ru
                      // French “Session” is identical to English and needs no separate entry.
    ]
    .iter()
    .find_map(|p| name.strip_prefix(p));
    match rest {
        Some(rest) => {
            let digits = rest.trim_start();
            !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// Condense the first user message into a title: take the first nonempty line, remove leading
/// Markdown punctuation/space, and truncate to 20 **characters** without splitting Unicode.
fn condense_title(prompt: &str) -> String {
    let first_line = prompt
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let cleaned = first_line
        .trim_start_matches(|c: char| matches!(c, '#' | '>' | '-' | '*') || c.is_whitespace())
        .trim();
    const MAX: usize = 20;
    let chars: Vec<char> = cleaned.chars().collect();
    if chars.len() > MAX {
        let mut s: String = chars[..MAX].iter().collect();
        s.push('…');
        s
    } else {
        cleaned.to_string()
    }
}

/// Parse the active-tool signal from hook JSON. Nonempty PreToolUse yields Tool(Some(name)); Stop
/// clears it. **PostToolUse emits nothing**, keeping the most recent tool visible throughout a turn
/// instead of flickering blank between calls. Other hooks/non-JSON return None.
fn parse_tool_signal(body: &str) -> Option<StatusSignal> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let event = v
        .get("hook_event_name")
        .or_else(|| v.get("hookEventName"))?
        .as_str()?;
    match event {
        "PreToolUse" | "pre_tool_use" => {
            let name = v
                .get("tool_name")
                .or_else(|| v.get("toolName"))?
                .as_str()?
                .trim();
            if name.is_empty() {
                None
            } else {
                Some(StatusSignal::Tool {
                    tool: Some(name.to_string()),
                })
            }
        }
        "Stop" | "stop" => Some(StatusSignal::Tool { tool: None }),
        _ => None,
    }
}

/// Validate `/spawn?t=<token>` plus JSON and return a child request. Wrong path/token or missing
/// parentSessionId/prompt returns None.
fn parse_spawn(url: &str, body: &str, expected_token: &str) -> Option<SpawnRequest> {
    let (path, query) = match url.split_once('?') {
        Some((p, q)) => (p, q),
        None => (url, ""),
    };
    if path != "/spawn" {
        return None;
    }
    let mut token = None;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == "t" {
                token = Some(v);
            }
        }
    }
    if token? != expected_token {
        return None;
    }
    let req: SpawnRequest = serde_json::from_str(body).ok()?;
    if req.parent_session_id.trim().is_empty() || req.prompt.trim().is_empty() {
        return None;
    }
    Some(req)
}

/// Extract the operation from an `/agent/<op>` URL path, or None for other paths.
fn agent_op_path(url: &str) -> Option<String> {
    let path = url.split_once('?').map(|(p, _)| p).unwrap_or(url);
    let op = path.strip_prefix("/agent/")?;
    if op.is_empty() || op.contains('/') {
        return None;
    }
    Some(op.to_string())
}

/// Extract the `t` query-parameter token from a URL.
fn query_token(url: &str) -> Option<String> {
    let query = url.split_once('?').map(|(_, q)| q)?;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == "t" {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Whether the URL path is `/view`, selecting its dedicated 404-capable branch.
fn is_view_path(url: &str) -> bool {
    let path = url.split_once('?').map(|(p, _)| p).unwrap_or(url);
    path == "/view"
}

/// Validate `/view?t=<token>` plus JSON and return an absolute-path request. Errors are user-facing
/// 404 bodies for invalid tokens/fields, failed cwd-relative canonicalization, or non-regular files.
fn parse_view(url: &str, body: &str, expected_token: &str) -> Result<ViewRequest, String> {
    let query = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut token = None;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == "t" {
                token = Some(v);
            }
        }
    }
    if token != Some(expected_token) {
        return Err("Token validation failed".to_string());
    }

    let parsed: ViewBody =
        serde_json::from_str(body).map_err(|_| "Request body is not valid JSON".to_string())?;
    if parsed.session_id.trim().is_empty()
        || parsed.path.trim().is_empty()
        || parsed.cwd.trim().is_empty()
    {
        return Err("Missing required fields (sessionId/path/cwd)".to_string());
    }

    // Pass HTTP(S) URLs through for the built-in browser. Other schemes are treated as file paths,
    // matching the browser-side allowlist, and fail canonicalization if nonexistent.
    let trimmed = parsed.path.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(ViewRequest {
            session_id: parsed.session_id,
            path: trimmed.to_string(),
            is_url: true,
        });
    }

    // Resolve relative paths against the caller's cwd and canonicalize `..`/symlinks, producing a
    // stable absolute string that docTabs can deduplicate.
    let raw = std::path::Path::new(&parsed.path);
    let joined = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        std::path::Path::new(&parsed.cwd).join(raw)
    };
    let abs = joined
        .canonicalize()
        .map_err(|_| format!("File does not exist: {}", joined.display()))?;
    let meta = std::fs::metadata(&abs).map_err(|e| format!("Failed to read file metadata: {e}"))?;
    if !meta.is_file() {
        return Err(format!(
            "Not a regular file (a directory?): {}",
            abs.display()
        ));
    }

    Ok(ViewRequest {
        session_id: parsed.session_id,
        path: abs.to_string_lossy().to_string(),
        is_url: false,
    })
}

/// Validate a hook request and return (session ID, optional status signal); reject bad tokens/events.
///
/// `boot` is a **capture-only** Copilot sessionStart event. It yields `(sid, None)` while still
/// allowing the caller to persist the body's agent session ID as a resume anchor.
fn handle(url: &str, expected_token: &str) -> Option<(String, Option<StatusSignal>)> {
    let (sid, token, event) = parse_url(url)?;
    if token != expected_token {
        return None;
    }
    let signal = match event.as_str() {
        "working" => Some(StatusSignal::State {
            state: AgentState::Working,
            silent: false,
            authoritative: false,
        }),
        "asking" => Some(StatusSignal::State {
            state: AgentState::Asking,
            silent: false,
            authoritative: false,
        }),
        "waiting" => Some(StatusSignal::State {
            state: AgentState::Waiting,
            silent: false,
            authoritative: false,
        }),
        // Codex lifecycle hooks cover a full turn, unlike legacy waiting-only notify. Any such event
        // lets the frontend lock authoritative mode and prevents screen/busy heuristics overriding Stop.
        "codex_working" => Some(StatusSignal::State {
            state: AgentState::Working,
            silent: false,
            authoritative: true,
        }),
        // Mid-turn working from PreToolUse. It carries the same state as `codex_working` but is subject
        // to `CodexTurnGuard`, which discards it when it arrives after the turn already ended.
        "codex_tool" => Some(StatusSignal::State {
            state: AgentState::Working,
            silent: false,
            authoritative: true,
        }),
        "codex_asking" => Some(StatusSignal::State {
            state: AgentState::Asking,
            silent: false,
            authoritative: true,
        }),
        "codex_waiting" => Some(StatusSignal::State {
            state: AgentState::Waiting,
            silent: false,
            authoritative: true,
        }),
        "codex_ready" => Some(StatusSignal::HookReady),
        // Kimi Code hooks cover the full lifecycle; distinct event names lock authoritative mode.
        "kimi_working" => Some(StatusSignal::State {
            state: AgentState::Working,
            silent: false,
            authoritative: true,
        }),
        "kimi_asking" => Some(StatusSignal::State {
            state: AgentState::Asking,
            silent: false,
            authoritative: true,
        }),
        "kimi_waiting" => Some(StatusSignal::State {
            state: AgentState::Waiting,
            silent: false,
            authoritative: true,
        }),
        "kimi_idle" => Some(StatusSignal::State {
            state: AgentState::Waiting,
            silent: true,
            authoritative: true,
        }),
        // Kiro hooks cover prompt submission, both tool phases, and turn end. There is no permission-request
        // hook, so Kiro never reports asking; the status dot stays working while it waits for approval.
        "kiro_working" => Some(StatusSignal::State {
            state: AgentState::Working,
            silent: false,
            authoritative: true,
        }),
        "kiro_waiting" => Some(StatusSignal::State {
            state: AgentState::Waiting,
            silent: false,
            authoritative: true,
        }),
        // Claude idle_prompt corrects status to waiting without notification. It means the agent has
        // been idle awaiting input, not that it just finished a reply.
        "idle" => Some(StatusSignal::State {
            state: AgentState::Waiting,
            silent: true,
            authoritative: false,
        }),
        // notfound reports an agent missing from PATH so the frontend can show installation guidance.
        "notfound" => Some(StatusSignal::AgentMissing),
        "boot" => None,
        _ => return None,
    };
    Some((sid, signal))
}

/// Extract (sid, token, event) from `/hook/<sid>?t=<token>&e=<event>`; require all components.
fn parse_url(url: &str) -> Option<(String, String, String)> {
    let (path, query) = match url.split_once('?') {
        Some((p, q)) => (p, q),
        None => (url, ""),
    };
    let sid = path.strip_prefix("/hook/")?;
    if sid.is_empty() || sid.contains('/') {
        return None;
    }
    let mut token = None;
    let mut event = None;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            match k {
                "t" => token = Some(v.to_string()),
                "e" => event = Some(v.to_string()),
                _ => {}
            }
        }
    }
    Some((sid.to_string(), token?, event?))
}

/// Hidden `vlx-term --notify <url> <json>` forwarding implementation. Send an HTTP/1.1 POST over a
/// raw TcpStream without an HTTP client dependency. Fail silently because notification is best-effort.
pub fn forward_notify(url: &str, body: &str) {
    let Some((host, port, path)) = split_http_url(url) else {
        return;
    };
    let Ok(mut stream) = TcpStream::connect((host.as_str(), port)) else {
        return;
    };
    let req = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\r\n\
         {body}",
        len = body.len()
    );
    let _ = stream.write_all(req.as_bytes());
    let _ = stream.flush();
    // Drain the response before closing to ensure the server finishes processing.
    let mut sink = Vec::new();
    let _ = stream.read_to_end(&mut sink);
}

/// Split an HTTP-only URL into (host, port, path+query).
pub(crate) fn split_http_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.split_once('/') {
        Some((a, p)) => (a, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = authority.split_once(':')?;
    let port: u16 = port.parse().ok()?;
    Some((host.to_string(), port, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_extracts_parts() {
        let (sid, token, event) =
            parse_url("/hook/abc-123?t=secret&e=working").expect("parsing should succeed");
        assert_eq!(sid, "abc-123");
        assert_eq!(token, "secret");
        assert_eq!(event, "working");
    }

    #[test]
    fn parse_url_rejects_wrong_path() {
        assert!(parse_url("/other/abc?t=x&e=y").is_none());
        assert!(parse_url("/hook/?t=x&e=y").is_none(), "an empty sid should be rejected");
        assert!(parse_url("/hook/a/b?t=x&e=y").is_none(), "an sid must not contain /");
    }

    #[test]
    fn parse_url_requires_token_and_event() {
        assert!(parse_url("/hook/sid?e=working").is_none(), "missing token");
        assert!(parse_url("/hook/sid?t=tok").is_none(), "missing event");
    }

    #[test]
    fn handle_maps_events_to_signals() {
        // A valid token maps each event to its authoritative state.
        let cases = [
            ("working", AgentState::Working),
            ("asking", AgentState::Asking),
            ("waiting", AgentState::Waiting),
        ];
        for (e, expect) in cases {
            let url = format!("/hook/s1?t=tok&e={e}");
            let (sid, sig) = handle(&url, "tok").expect("should map");
            assert_eq!(sid, "s1");
            match sig {
                Some(StatusSignal::State { state, silent, .. }) => {
                    assert_eq!(state, expect);
                    assert!(!silent, "working, asking and waiting should all notify rather than stay silent");
                }
                _ => panic!("expected a State signal"),
            }
        }
    }

    #[test]
    fn handle_marks_codex_lifecycle_events_authoritative() {
        let (_, ready) =
            handle("/hook/c1?t=tok&e=codex_ready", "tok").expect("the Codex startup handshake should be valid");
        assert!(
            matches!(ready, Some(StatusSignal::HookReady)),
            "SessionStart may only produce a health handshake, never a fabricated completion state"
        );

        let cases = [
            ("codex_working", AgentState::Working),
            ("codex_tool", AgentState::Working),
            ("codex_asking", AgentState::Asking),
            ("codex_waiting", AgentState::Waiting),
        ];
        for (event, expected) in cases {
            let url = format!("/hook/c1?t=tok&e={event}");
            let (_, signal) = handle(&url, "tok").expect("the Codex hook event should be valid");
            match signal {
                Some(StatusSignal::State {
                    state,
                    silent,
                    authoritative,
                }) => {
                    assert_eq!(state, expected);
                    assert!(!silent);
                    assert!(authoritative, "a complete set of Codex hooks must lock into authoritative mode");
                }
                _ => panic!("expected a fully authoritative State signal"),
            }
        }
    }

    #[test]
    fn codex_turn_guard_drops_late_mid_turn_working() {
        let mut guard = CodexTurnGuard::default();
        let start = Instant::now();

        // A normal turn: prompt opens it, tool calls keep it working, Stop ends it.
        assert!(guard.admit("s1", "codex_working", start));
        assert!(guard.admit("s1", "codex_tool", start + Duration::from_millis(100)));
        assert!(guard.admit("s1", "codex_waiting", start + Duration::from_millis(200)));

        // A PreToolUse callback from the finished turn arrives late and must not revive working.
        assert!(
            !guard.admit("s1", "codex_tool", start + Duration::from_millis(260)),
            "the turn has ended, so a late mid-turn working must not push the state back"
        );

        // Another session's callbacks are unaffected by this one's turn boundary.
        assert!(guard.admit("s2", "codex_tool", start + Duration::from_millis(260)));

        // A user replying immediately still opens a new turn, and its tool calls apply again.
        assert!(guard.admit("s1", "codex_working", start + Duration::from_millis(300)));
        assert!(guard.admit("s1", "codex_tool", start + Duration::from_millis(320)));

        // Outside the window a mid-turn event is trusted again: it belongs to a turn we never saw start.
        assert!(guard.admit("s1", "codex_waiting", start + Duration::from_millis(400)));
        assert!(guard.admit("s1", "codex_tool", start + Duration::from_secs(3)));

        // Non-Codex events are never filtered.
        assert!(guard.admit("s3", "working", start));
        assert!(guard.admit("s3", "waiting", start));
        assert!(guard.admit("s3", "working", start + Duration::from_millis(10)));
    }

    #[test]
    fn handle_idle_is_silent_waiting() {
        // Claude idle maps silently to waiting: correct status without notifying.
        let (sid, sig) = handle("/hook/s1?t=tok&e=idle", "tok").expect("idle should be a valid event");
        assert_eq!(sid, "s1");
        match sig {
            Some(StatusSignal::State { state, silent, .. }) => {
                assert_eq!(state, AgentState::Waiting);
                assert!(silent, "idle should stay silent and raise no replied notification");
            }
            _ => panic!("idle should map to a State signal"),
        }
    }

    #[test]
    fn handle_boot_is_capture_only() {
        // Copilot sessionStart boot is valid but emits no status; it only captures the body ID.
        let (sid, sig) = handle("/hook/s1?t=tok&e=boot", "tok").expect("boot should be a valid event");
        assert_eq!(sid, "s1");
        assert!(sig.is_none(), "boot should produce no state signal");
        // Still reject an invalid token.
        assert!(handle("/hook/s1?t=wrong&e=boot", "tok").is_none());
    }

    #[test]
    fn handle_notfound_is_agent_missing() {
        // notfound maps to AgentMissing so the frontend can show installation guidance.
        let (sid, sig) = handle("/hook/s1?t=tok&e=notfound", "tok").expect("notfound should be a valid event");
        assert_eq!(sid, "s1");
        assert!(
            matches!(sig, Some(StatusSignal::AgentMissing)),
            "notfound should map to AgentMissing"
        );
        // Still reject an invalid token.
        assert!(handle("/hook/s1?t=wrong&e=notfound", "tok").is_none());
    }

    #[test]
    fn is_auto_name_matches_placeholder_names() {
        // Automatically numbered placeholders match with optional whitespace.
        for name in [
            "Claude 1",
            "Codex 22",
            "OpenCode 3",
            "Copilot 5",
            "Cursor 6",
            "Antigravity 3",
            "Cline 7",
            "Cline1",
            "Pi 6",
            "Pi7",
            "Session 4",
            "会话 4",
            "Claude1",
            "会话12",
            "セッション 2",
        ] {
            assert!(is_auto_name(name), "{name} should count as an auto-numbered name");
        }
        // User/other names do not match, including words such as `Pilot 3` that merely start with Pi.
        for name in [
            "修登录页样式",
            "Claude",
            "Claude 1a",
            "my-claude 1",
            "Claude 1 副本",
            "Pilot 3",
            "Pi",
            "",
        ] {
            assert!(!is_auto_name(name), "{name} should not count as an auto-numbered name");
        }
    }

    #[test]
    fn condense_title_takes_first_line_and_truncates() {
        // Take the first nonempty line and strip leading Markdown noise.
        assert_eq!(
            condense_title("\n\n# 修登录页样式\n其余细节"),
            "修登录页样式"
        );
        assert_eq!(condense_title("- 改 bug"), "改 bug");
        // Truncate beyond 20 characters without splitting Unicode, then append an ellipsis.
        let long = "一二三四五六七八九十一二三四五六七八九十超出";
        let got = condense_title(long);
        assert_eq!(got.chars().count(), 21);
        assert!(got.ends_with('…'));
        // All whitespace becomes an empty string, telling the caller to skip renaming.
        assert_eq!(condense_title("   \n  "), "");
    }

    #[test]
    fn handle_rejects_bad_token() {
        assert!(handle("/hook/s1?t=wrong&e=working", "tok").is_none());
    }

    #[test]
    fn handle_ignores_unknown_event() {
        assert!(handle("/hook/s1?t=tok&e=bogus", "tok").is_none());
    }

    #[test]
    fn parse_spawn_extracts_and_validates() {
        let body =
            r#"{"parentSessionId":"p1","prompt":"fix the login bug","kind":"claude","worktree":true}"#;
        let req = parse_spawn("/spawn?t=tok", body, "tok").expect("should parse");
        assert_eq!(req.parent_session_id, "p1");
        assert_eq!(req.prompt, "fix the login bug");
        assert_eq!(req.kind.as_deref(), Some("claude"));
        assert_eq!(req.worktree, Some(true));

        // Launch-configuration fields pass through unchanged.
        let full = r#"{"parentSessionId":"p1","prompt":"x","kind":"codex","worktree":true,
            "model":"gpt-5.6-luna","effort":"xhigh","name":"update-types","agentArgs":"--foo bar"}"#;
        let req_full = parse_spawn("/spawn?t=tok", full, "tok").expect("should parse");
        assert_eq!(req_full.model.as_deref(), Some("gpt-5.6-luna"));
        assert_eq!(req_full.effort.as_deref(), Some("xhigh"));
        assert_eq!(req_full.name.as_deref(), Some("update-types"));
        assert_eq!(req_full.agent_args.as_deref(), Some("--foo bar"));

        // kind, worktree, and the launch-configuration fields are optional.
        let req2 = parse_spawn(
            "/spawn?t=tok",
            r#"{"parentSessionId":"p","prompt":"x"}"#,
            "tok",
        )
        .unwrap();
        assert_eq!(req2.kind, None);
        assert_eq!(req2.worktree, None);
        assert_eq!(req2.model, None);
        assert_eq!(req2.effort, None);
        assert_eq!(req2.name, None);
        assert_eq!(req2.agent_args, None);

        // Wrong token/path, empty required fields, or invalid JSON yields None.
        assert!(parse_spawn("/spawn?t=wrong", body, "tok").is_none());
        assert!(parse_spawn("/hook/x?t=tok", body, "tok").is_none());
        assert!(parse_spawn(
            "/spawn?t=tok",
            r#"{"parentSessionId":"","prompt":"x"}"#,
            "tok"
        )
        .is_none());
        assert!(parse_spawn(
            "/spawn?t=tok",
            r#"{"parentSessionId":"p","prompt":"  "}"#,
            "tok"
        )
        .is_none());
        assert!(parse_spawn("/spawn?t=tok", "not json", "tok").is_none());
    }

    /// Create a temporary file and return (directory, filename, absolute path) for parse_view tests.
    fn view_fixture(tag: &str) -> (std::path::PathBuf, String, String) {
        let dir =
            std::env::temp_dir().join(format!("vlx-view-test-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name = "notes.md".to_string();
        let file = dir.join(&name);
        std::fs::write(&file, "# hi").unwrap();
        // Canonicalize away /tmp versus /private/tmp symlink differences for stable comparisons.
        let abs = file.canonicalize().unwrap().to_string_lossy().to_string();
        (dir, name, abs)
    }

    #[test]
    fn parse_view_resolves_relative_path_against_cwd() {
        let (dir, name, abs) = view_fixture("rel");
        let body = format!(
            r#"{{"sessionId":"s1","path":"{name}","cwd":"{}"}}"#,
            dir.display()
        );
        let req = parse_view("/view?t=tok", &body, "tok").expect("a relative path should resolve against cwd");
        assert_eq!(req.session_id, "s1");
        assert_eq!(req.path, abs, "the canonicalized absolute path should be returned");

        // Use absolute paths directly without joining cwd.
        let body_abs = format!(r#"{{"sessionId":"s1","path":"{abs}","cwd":"/elsewhere"}}"#);
        let req2 = parse_view("/view?t=tok", &body_abs, "tok").unwrap();
        assert_eq!(req2.path, abs);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_view_passes_through_http_urls() {
        // HTTP(S) URLs bypass file checks and set is_url for the built-in browser.
        let body = r#"{"sessionId":"s1","path":"https://github.com/a/b?x=1","cwd":"/tmp"}"#;
        let req = parse_view("/view?t=tok", body, "tok").expect("a URL should be passed straight through");
        assert!(req.is_url);
        assert_eq!(req.path, "https://github.com/a/b?x=1");

        let body = r#"{"sessionId":"s1","path":"HTTP://example.com","cwd":"/tmp"}"#;
        assert!(
            parse_view("/view?t=tok", body, "tok").unwrap().is_url,
            "the scheme is case-insensitive"
        );

        // Other schemes are treated as nonexistent file paths and keep is_url false.
        let body = r#"{"sessionId":"s1","path":"ftp://example.com/x","cwd":"/tmp"}"#;
        assert!(parse_view("/view?t=tok", body, "tok").is_err());
    }

    #[test]
    fn parse_view_rejects_invalid() {
        let (dir, name, _abs) = view_fixture("invalid");
        let good = format!(
            r#"{{"sessionId":"s1","path":"{name}","cwd":"{}"}}"#,
            dir.display()
        );
        // Invalid token.
        assert!(parse_view("/view?t=wrong", &good, "tok").is_err());
        assert!(parse_view("/view", &good, "tok").is_err(), "missing token");
        // Missing file.
        let missing = format!(
            r#"{{"sessionId":"s1","path":"missing.md","cwd":"{}"}}"#,
            dir.display()
        );
        let err = parse_view("/view?t=tok", &missing, "tok").unwrap_err();
        assert!(err.contains("does not exist"), "got: {err}");
        // Path is a directory.
        let isdir = format!(
            r#"{{"sessionId":"s1","path":"{0}","cwd":"{0}"}}"#,
            dir.display()
        );
        let err = parse_view("/view?t=tok", &isdir, "tok").unwrap_err();
        assert!(err.contains("Not a regular file"), "got: {err}");
        // Missing/empty fields and invalid JSON.
        assert!(parse_view("/view?t=tok", r#"{"sessionId":"s1","path":"x"}"#, "tok").is_err());
        assert!(parse_view(
            "/view?t=tok",
            r#"{"sessionId":"","path":"x","cwd":"/"}"#,
            "tok"
        )
        .is_err());
        assert!(parse_view("/view?t=tok", "not json", "tok").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Raw TcpStream POST that returns the HTTP status for tests of the 404 branch.
    fn post_status(url: &str, body: &str) -> u16 {
        let (host, port, path) = split_http_url(url).unwrap();
        let mut stream = TcpStream::connect((host.as_str(), port)).unwrap();
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
            len = body.len()
        );
        stream.write_all(req.as_bytes()).unwrap();
        stream.flush().unwrap();
        let mut resp = Vec::new();
        let _ = stream.read_to_end(&mut resp);
        let line = String::from_utf8_lossy(&resp);
        // A status line has the form `HTTP/1.1 404 Not Found`.
        line.split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    /// Real HTTP round trip: valid POST /view returns 200 and an absolute callback path; a missing
    /// path returns 404 without invoking the callback.
    #[test]
    fn serve_with_routes_view_request_and_404s_missing_file() {
        use std::sync::mpsc;
        use std::time::Duration;

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            serve_with(
                server,
                "tok",
                |_sid, _sig| {},
                |_sid, _prompt| {},
                |_a, _b| {},
                |_req| {},
                move |req| {
                    let _ = tx.send(req);
                },
                None,
            );
        });

        let (dir, name, abs) = view_fixture("http");
        let url = format!("http://127.0.0.1:{port}/view?t=tok");
        let body = format!(
            r#"{{"sessionId":"s9","path":"{name}","cwd":"{}"}}"#,
            dir.display()
        );
        assert_eq!(post_status(&url, &body), 200);
        let req = rx
            .recv_timeout(Duration::from_secs(3))
            .expect("should route to on_view");
        assert_eq!(req.session_id, "s9");
        assert_eq!(req.path, abs);

        // A missing file returns 404 and does not invoke the callback.
        let missing = format!(
            r#"{{"sessionId":"s9","path":"missing.md","cwd":"{}"}}"#,
            dir.display()
        );
        assert_eq!(post_status(&url, &missing), 404);
        assert!(
            rx.recv_timeout(Duration::from_millis(300)).is_err(),
            "a failed validation must not trigger on_view"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn split_http_url_parses() {
        let (h, p, path) = split_http_url("http://127.0.0.1:8080/hook/x?t=a&e=b").unwrap();
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 8080);
        assert_eq!(path, "/hook/x?t=a&e=b");
    }

    /// Real HTTP round trip through temporary tiny_http verifies that forward_notify's raw
    /// TcpStream client delivers its body, covering the Codex notify path.
    #[test]
    fn forward_notify_round_trip() {
        use std::sync::mpsc;

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            if let Some(mut req) = server.incoming_requests().next() {
                let url = req.url().to_string();
                let mut body = String::new();
                let _ = req.as_reader().read_to_string(&mut body);
                let _ = req.respond(tiny_http::Response::empty(200));
                let _ = tx.send((url, body));
            }
        });

        let url = format!("http://127.0.0.1:{port}/hook/sid-x?t=tok&e=waiting");
        forward_notify(&url, "{\"type\":\"agent-turn-complete\"}");

        let (got_url, got_body) = rx.recv_timeout(std::time::Duration::from_secs(3)).unwrap();
        assert_eq!(got_url, "/hook/sid-x?t=tok&e=waiting");
        assert_eq!(got_body, "{\"type\":\"agent-turn-complete\"}");
        let _ = handle.join();
    }

    /// End-to-end test using production injection and a real tiny_http serve_with instance. Run one
    /// `claude -p` turn and verify authoritative working (UserPromptSubmit) then waiting (Stop).
    ///
    /// Ignored by default because it requires an authenticated Claude installation and consumes quota.
    /// `cargo test --lib -- --ignored claude_http_hooks_end_to_end`
    #[test]
    #[ignore = "requires claude to be installed and logged in, and consumes API quota"]
    fn claude_http_hooks_end_to_end() {
        use crate::agent::inject;
        use std::sync::mpsc;
        use std::time::Duration;

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let ep = HookEndpoint {
            port,
            token: "e2e-token".to_string(),
        };
        let sid = "e2e-sid";

        let (tx, rx) = mpsc::channel();
        // Separately capture Claude's session_id to verify that HTTP hook bodies actually include it.
        let (tx_sid, rx_sid) = mpsc::channel();
        let token = ep.token.clone();
        std::thread::spawn(move || {
            serve_with(
                server,
                &token,
                move |got_sid, sig| {
                    let _ = tx.send((got_sid, sig));
                },
                |_sid, _prompt| {},
                move |vlx_sid, agent_sid| {
                    let _ = tx_sid.send((vlx_sid, agent_sid));
                },
                |_req| {},
                |_req| {},
                None,
            );
        });

        // Generate settings through the production code path.
        let settings = inject::build_claude_settings(&ep, sid);
        let output = std::process::Command::new("claude")
            .args(["-p", "Reply with exactly: OK", "--settings", &settings])
            .output()
            .expect("failed to run claude (not installed, or not logged in?)");
        assert!(
            output.status.success(),
            "claude exited non-zero: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Collect authoritative signals, including working on submit and waiting on completion.
        let mut states = Vec::new();
        while let Ok((got_sid, sig)) = rx.recv_timeout(Duration::from_secs(5)) {
            assert_eq!(got_sid, sid, "the session id in the URL should come back verbatim");
            if let StatusSignal::State { state, .. } = sig {
                states.push(state);
            }
            if states.contains(&AgentState::Working) && states.contains(&AgentState::Waiting) {
                break;
            }
        }
        assert!(
            states.contains(&AgentState::Working),
            "expected to receive working, got {states:?}"
        );
        assert!(
            states.contains(&AgentState::Waiting),
            "expected to receive waiting, got {states:?}"
        );

        // Verify the body carries Claude's session_id and the URL preserves the VelaTerm sid.
        let (vlx_sid, agent_sid) = rx_sid
            .recv_timeout(Duration::from_secs(5))
            .expect("a claude session_id should be parsed out of the hook body");
        assert_eq!(vlx_sid, sid, "the vlx session id in the callback should match the URL");
        assert!(!agent_sid.is_empty(), "the claude session_id must not be empty");
    }

    #[test]
    fn parse_session_id_extracts_field() {
        // Claude hook body includes session_id alongside other fields.
        let body =
            r#"{"session_id":"abc-123","transcript_path":"/x.jsonl","hook_event_name":"Stop"}"#;
        assert_eq!(parse_session_id(body).as_deref(), Some("abc-123"));
        // Copilot camelCase sessionId is supported because the hook forwards stdin unchanged.
        let copilot = r#"{"sessionId":"0cb916db-26aa-40f2-86b5-1ba81b225fd2","timestamp":1700000000000,"cwd":"/x"}"#;
        assert_eq!(
            parse_session_id(copilot).as_deref(),
            Some("0cb916db-26aa-40f2-86b5-1ba81b225fd2")
        );
        // Cline top-level camelCase taskId is supported.
        let cline = r#"{"clineVersion":"3.0.34","hookName":"agent_start","taskId":"cline-task-42","timestamp":1700000000000}"#;
        assert_eq!(parse_session_id(cline).as_deref(), Some("cline-task-42"));
        // Cline's snake_case task_id variant is also supported.
        assert_eq!(
            parse_session_id(r#"{"task_id":"t-snake"}"#).as_deref(),
            Some("t-snake")
        );
        // Prefer snake_case when both exist, preserving Claude semantics.
        let both = r#"{"session_id":"snake","sessionId":"camel"}"#;
        assert_eq!(parse_session_id(both).as_deref(), Some("snake"));
        // A Codex notify body without a session ID yields None.
        let codex = r#"{"type":"agent-turn-complete","thread-id":"codex-thread-1","turn-id":"t1"}"#;
        assert_eq!(parse_session_id(codex).as_deref(), Some("codex-thread-1"));
        let codex_snake = r#"{"type":"agent-turn-complete","thread_id":"codex-thread-2"}"#;
        assert_eq!(
            parse_session_id(codex_snake).as_deref(),
            Some("codex-thread-2")
        );
        // Blank/missing fields and invalid JSON yield None.
        assert!(parse_session_id(r#"{"session_id":"  "}"#).is_none());
        assert!(parse_session_id(r#"{"sessionId":"  "}"#).is_none());
        assert!(parse_session_id("{}").is_none());
        assert!(parse_session_id("not json").is_none());
    }

    #[test]
    fn parse_first_prompt_from_supported_prompt_events() {
        // Extract and trim prompt from UserPromptSubmit.
        let body = r#"{"session_id":"s","hook_event_name":"UserPromptSubmit","prompt":"  Fix the login page styling  "}"#;
        assert_eq!(parse_first_prompt(body).as_deref(), Some("Fix the login page styling"));
        // Kiro spells the event `userPromptSubmit`; payload captured from kiro-cli 2.16.2.
        let kiro = r#"{"hook_event_name":"userPromptSubmit","cwd":"/tmp","prompt":"say OK"}"#;
        assert_eq!(parse_first_prompt(kiro).as_deref(), Some("say OK"));
        // Kiro's agentSpawn repeats the same prompt but must not name the session.
        let spawn = r#"{"hook_event_name":"agentSpawn","cwd":"/tmp","prompt":"say OK"}"#;
        assert_eq!(parse_first_prompt(spawn), None);
        // Cursor beforeSubmitPrompt forwards an equivalent payload.
        let cursor = r#"{"conversation_id":"c","session_id":"c","hook_event_name":"beforeSubmitPrompt","prompt":"Fix the login timeout","attachments":[]}"#;
        assert_eq!(parse_first_prompt(cursor).as_deref(), Some("Fix the login timeout"));
        // Grok uses camelCase field names and a snake-case lifecycle value.
        let grok = r#"{"hookEventName":"user_prompt_submit","sessionId":"g","prompt":"  Fix the Grok clone  "}"#;
        assert_eq!(parse_first_prompt(grok).as_deref(), Some("Fix the Grok clone"));
        // Cline prompt_submit stores text in nested userPromptSubmit.prompt.
        let cline = r#"{"clineVersion":"3.0.34","hookName":"prompt_submit","taskId":"t1","userPromptSubmit":{"prompt":"  Refactor the login module  "}}"#;
        assert_eq!(parse_first_prompt(cline).as_deref(), Some("Refactor the login module"));
        // Codex notify uses the first nonempty input-messages item on turn completion.
        let codex = r#"{"type":"agent-turn-complete","thread-id":"c1","input-messages":["  ","  Name Codex sessions automatically  "],"last-assistant-message":"done"}"#;
        assert_eq!(
            parse_first_prompt(codex).as_deref(),
            Some("Name Codex sessions automatically")
        );
        // Ignore other events, wrong field types, and empty arrays.
        assert!(
            parse_first_prompt(r#"{"type":"approval-requested","input-messages":["x"]}"#).is_none()
        );
        assert!(
            parse_first_prompt(r#"{"type":"agent-turn-complete","input-messages":"x"}"#).is_none()
        );
        assert!(
            parse_first_prompt(r#"{"type":"agent-turn-complete","input-messages":[]}"#).is_none()
        );
        // Missing/blank nested Cline fields yield None without falling back to unrelated fields.
        assert!(parse_first_prompt(r#"{"hookName":"prompt_submit","taskId":"t1"}"#).is_none());
        assert!(parse_first_prompt(
            r#"{"hookName":"prompt_submit","userPromptSubmit":{"prompt":"  "}}"#
        )
        .is_none());
        // Ignore Cline internal events other than prompt_submit.
        assert!(parse_first_prompt(r#"{"hookName":"agent_start","taskId":"t1"}"#).is_none());
        // Other hooks without prompts yield None.
        let pre = r#"{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"Bash"}"#;
        assert!(parse_first_prompt(pre).is_none());
        // Require UserPromptSubmit even when a prompt field exists.
        let other = r#"{"hook_event_name":"Stop","prompt":"x"}"#;
        assert!(parse_first_prompt(other).is_none());
        // Empty prompts, missing fields, and invalid JSON yield None.
        assert!(
            parse_first_prompt(r#"{"hook_event_name":"UserPromptSubmit","prompt":"  "}"#).is_none()
        );
        assert!(parse_first_prompt(r#"{"hook_event_name":"UserPromptSubmit"}"#).is_none());
        assert!(parse_first_prompt("not json").is_none());
    }

    /// PreToolUse extracts a tool; Stop clears it; PostToolUse preserves the most recent display.
    #[test]
    fn parse_tool_signal_extracts_and_clears() {
        let pre = r#"{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{}}"#;
        match parse_tool_signal(pre) {
            Some(StatusSignal::Tool { tool }) => assert_eq!(tool.as_deref(), Some("Bash")),
            other => panic!("a tool name should have been parsed: {other:?}"),
        }
        // Stop clears the tool at turn completion.
        match parse_tool_signal(r#"{"hook_event_name":"Stop"}"#) {
            Some(StatusSignal::Tool { tool }) => assert!(tool.is_none()),
            other => panic!("Stop should clear the tool: {other:?}"),
        }
        // Grok uses camelCase fields with snake-case event names.
        let grok = r#"{"hookEventName":"pre_tool_use","toolName":"run_terminal_command"}"#;
        match parse_tool_signal(grok) {
            Some(StatusSignal::Tool { tool }) => {
                assert_eq!(tool.as_deref(), Some("run_terminal_command"))
            }
            other => panic!("the Grok tool name should have been parsed: {other:?}"),
        }
        assert!(matches!(
            parse_tool_signal(r#"{"hookEventName":"stop"}"#),
            Some(StatusSignal::Tool { tool: None })
        ));
        // PostToolUse emits nothing, preserving the previous tool name.
        assert!(
            parse_tool_signal(r#"{"hook_event_name":"PostToolUse","tool_name":"Bash"}"#).is_none()
        );
        // Other events, empty tool names, and non-JSON emit nothing.
        assert!(
            parse_tool_signal(r#"{"hook_event_name":"UserPromptSubmit","prompt":"x"}"#).is_none()
        );
        assert!(
            parse_tool_signal(r#"{"hook_event_name":"PreToolUse","tool_name":"  "}"#).is_none()
        );
        assert!(parse_tool_signal("not json").is_none());
    }

    /// Real HTTP serve_with test verifies that a hook carrying session_id invokes on_session_id with
    /// (vlx_sid, agent_session_id), without launching Claude.
    #[test]
    fn serve_with_captures_session_id_from_body() {
        use std::sync::mpsc;
        use std::time::Duration;

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let (tx_sid, rx_sid) = mpsc::channel();
        std::thread::spawn(move || {
            serve_with(
                server,
                "tok",
                |_sid, _sig| {},
                |_sid, _prompt| {},
                move |vlx_sid, agent_sid| {
                    let _ = tx_sid.send((vlx_sid, agent_sid));
                },
                |_req| {},
                |_req| {},
                None,
            );
        });

        let url = format!("http://127.0.0.1:{port}/hook/vlx-1?t=tok&e=working");
        forward_notify(
            &url,
            r#"{"session_id":"claude-xyz","hook_event_name":"UserPromptSubmit"}"#,
        );

        let (vlx_sid, agent_sid) = rx_sid
            .recv_timeout(Duration::from_secs(3))
            .expect("on_session_id should have been triggered");
        assert_eq!(vlx_sid, "vlx-1");
        assert_eq!(agent_sid, "claude-xyz");
    }

    /// A Codex UserPromptSubmit carries both agent session ID and prompt. Bind the exact ID before
    /// naming so an authoritative hook corrects any earlier rollout guess and titles only its URL node.
    #[test]
    fn serve_with_routes_codex_identity_before_prompt() {
        use std::sync::mpsc;
        use std::time::Duration;

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let (tx, rx) = mpsc::channel();
        let tx_prompt = tx.clone();
        std::thread::spawn(move || {
            serve_with(
                server,
                "tok",
                |_sid, _sig| {},
                move |sid, prompt| {
                    let _ = tx_prompt.send(format!("prompt:{sid}:{prompt}"));
                },
                move |sid, agent_sid| {
                    let _ = tx.send(format!("identity:{sid}:{agent_sid}"));
                },
                |_req| {},
                |_req| {},
                None,
            );
        });

        let url = format!("http://127.0.0.1:{port}/hook/vlx-codex?t=tok&e=codex_working");
        forward_notify(
            &url,
            r#"{"session_id":"codex-exact","hook_event_name":"UserPromptSubmit","prompt":"the current session title"}"#,
        );

        assert_eq!(
            rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            "identity:vlx-codex:codex-exact"
        );
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            "prompt:vlx-codex:the current session title"
        );
    }

    /// Real HTTP serve_with test posts the same /spawn body as `vlx-spawn` and verifies routing and
    /// parsing through HTTP, token validation, and the on_spawn callback.
    #[test]
    fn serve_with_routes_spawn_request() {
        use std::sync::mpsc;
        use std::time::Duration;

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            serve_with(
                server,
                "tok",
                |_sid, _sig| {},
                |_sid, _prompt| {},
                |_a, _b| {},
                move |req| {
                    let _ = tx.send(req);
                },
                |_req| {},
                None,
            );
        });

        let url = format!("http://127.0.0.1:{port}/spawn?t=tok");
        forward_notify(
            &url,
            r#"{"parentSessionId":"p1","prompt":"fix a bug","kind":"claude","worktree":false}"#,
        );

        let req = rx
            .recv_timeout(Duration::from_secs(3))
            .expect("should route to on_spawn");
        assert_eq!(req.parent_session_id, "p1");
        assert_eq!(req.prompt, "fix a bug");
        assert_eq!(req.kind.as_deref(), Some("claude"));
        assert_eq!(req.worktree, Some(false));

        // An invalid token must not reach on_spawn within the timeout.
        let url_bad = format!("http://127.0.0.1:{port}/spawn?t=wrong");
        forward_notify(&url_bad, r#"{"parentSessionId":"p2","prompt":"x"}"#);
        assert!(
            rx.recv_timeout(Duration::from_millis(500)).is_err(),
            "a /spawn with the wrong token must not trigger on_spawn"
        );
    }

    /// Real HTTP test for `/agent/<op>` routing: token enforcement, worker-thread handling, and a
    /// JSON response from the ctl handler backed by a headless AppCtx.
    #[test]
    fn serve_with_routes_agent_ops() {
        let db_path = std::env::temp_dir().join(format!(
            "vlx-agent-route-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let db = crate::db::Db::open(&db_path).unwrap();
        let parent_id = {
            let conn = db.conn.lock().unwrap();
            let project = crate::db::repo::import_project(
                &conn,
                std::env::temp_dir().to_str().unwrap(),
            )
            .unwrap();
            crate::db::repo::create_session_full(
                &conn,
                &project.id,
                None,
                "lead",
                crate::models::SessionKind::Claude,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap()
            .id
        };
        let app = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(
            std::env::temp_dir(),
            db,
        )));

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        std::thread::spawn(move || {
            serve_with(
                server,
                "tok",
                |_sid, _sig| {},
                |_sid, _prompt| {},
                |_a, _b| {},
                |_req| {},
                |_req| {},
                Some(app),
            );
        });

        let body = format!(r#"{{"parentSessionId":"{parent_id}"}}"#);
        let url = format!("http://127.0.0.1:{port}/agent/list?t=tok");
        let (status, resp) =
            crate::agent::ctl_client::post_json(&url, &body).expect("should connect");
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["sessions"], serde_json::json!([]));

        // Wrong token is rejected before reaching the handler; unknown ops are 404.
        let url_bad = format!("http://127.0.0.1:{port}/agent/list?t=wrong");
        assert_eq!(crate::agent::ctl_client::post_json(&url_bad, &body).unwrap().0, 403);
        let url_unknown = format!("http://127.0.0.1:{port}/agent/bogus?t=tok");
        assert_eq!(
            crate::agent::ctl_client::post_json(&url_unknown, &body).unwrap().0,
            404
        );
    }
}
