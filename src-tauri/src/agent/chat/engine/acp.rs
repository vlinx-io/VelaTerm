//! ACP agents inside the chat engine: one process, its JSON-RPC stream, and the timeline it produces.
//!
//! The handshake is `initialize`, then `session/load` to continue a conversation or `session/new` to begin
//! one; the session is ready once that answers. Each turn is a `session/prompt` request, and its response
//! is what ends the turn. Everything the agent does meanwhile arrives as `session/update` notifications,
//! which `acp_protocol::Mapper` turns into rows.
//!
//! **Continuing a conversation.** The view first shows the conversation as last recorded (the journal, or
//! Grok's native stream), then `session/load` replays it from the agent and the replay replaces those rows
//! in one step. The replay is the authority because it includes turns taken in the terminal view. Rows the
//! replay produces carry the same ids as the recording's, so their times carry over.
//!
//! **Permissions.** In `default` mode a `session/request_permission` becomes a permission card and the
//! answer selects one of the agent's own options. In `bypassPermissions` the engine selects the allow
//! option itself. A cancelled turn answers its waiting requests as cancelled, as the protocol requires.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{emit, emit_state, handle_turn_end, now_ms, start_waiting_message, verify_native_identity, ChatProcess, ChatRow};
use crate::agent::chat::acp_protocol::{self as wire, Catalog, Incoming, Mapper};
use crate::agent::chat::antigravity_protocol::{
    acp_mode_for_permission, permission_for_acp_mode, tool_row,
};
use crate::agent::chat::codex_protocol::CollaborationModePreset;
use crate::agent::chat::protocol::ChatImage;
use crate::host::AppCtx;
use crate::models::SessionKind;
use crate::pty::AgentState;

/// Journal lines are written in batches; a turn that streams for a long time still lands on disk this often.
const JOURNAL_BATCH: usize = 64;
/// Bound how many automatic `Stop` hook continuations a single ACP user turn can trigger.
const MAX_SAFE_STOP_CONTINUATIONS: u64 = 32;
/// How long to wait for a parked `Stop` prompt to unwind when starting a new user turn.
const RETIRE_WAIT_TIMEOUT: Duration = Duration::from_secs(3);
/// Wake interval for a parked `Stop` hook checking `.system_generated/messages/` for deliveries.
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Backoff delays when retrying an Antigravity prompt rejected during cold-start model loading.
fn cold_model_retry_delays() -> &'static [Duration] {
    #[cfg(not(test))]
    const DELAYS: &[Duration] = &[
        Duration::from_secs(1),
        Duration::from_secs(2),
        Duration::from_secs(3),
        Duration::from_secs(4),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
    ];
    #[cfg(test)]
    const DELAYS: &[Duration] = &[
        Duration::from_millis(25),
        Duration::from_millis(25),
        Duration::from_millis(25),
    ];
    DELAYS
}

#[derive(Default)]
struct RetryWait {
    woken: Mutex<bool>,
    cv: Condvar,
}

impl RetryWait {
    fn wake(&self) {
        let mut woken = self.woken.lock().unwrap();
        *woken = true;
        self.cv.notify_all();
    }

    fn wait_for(&self, duration: Duration) {
        let deadline = Instant::now() + duration;
        let mut woken = self.woken.lock().unwrap();
        while !*woken {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            let (next, timed_out) = self.cv.wait_timeout(woken, left).unwrap();
            woken = next;
            if timed_out.timed_out() {
                break;
            }
        }
    }
}

#[derive(Clone)]
struct ActivePrompt {
    text: String,
    images: Vec<ChatImage>,
    attempts: usize,
    had_output: bool,
}

#[derive(Clone)]
pub(super) enum PendingRequest {
    Permission { id: Value, params: Value },
    Elicitation { id: Value, params: Value, card_payload: Value },
    /// One of Grok's own `_x.ai/...` dialogs, which takes its own method's answer shape.
    GrokDialog { id: Value, method: String, params: Value },
    DetachedRecovery { row_id: String, tool_name: String, args: Value, card_payload: Value },
}

#[derive(Default)]
pub(super) struct StopPark {
    decision: Mutex<Option<&'static str>>,
    cv: Condvar,
}

impl StopPark {
    fn release(&self, decision: &'static str) {
        let mut guard = self.decision.lock().unwrap();
        if guard.is_none() {
            *guard = Some(decision);
            self.cv.notify_all();
        }
    }
}

#[derive(Default)]
pub(super) struct RetireWaiter {
    pending: Mutex<HashSet<String>>,
    cv: Condvar,
}

/// Everything the engine keeps about the ACP side of one session.
#[derive(Default)]
pub(super) struct AcpState {
    pub(super) mapper: Mapper,
    pub(super) catalog: Catalog,
    /// Whether the agent accepts image blocks in a prompt.
    pub(super) images: bool,
    /// Set once `initialize` has answered; before that the view cannot know.
    pub(super) images_known: bool,
    /// Set once `initialize` has answered, so account operations know the server is listening.
    pub(in crate::agent::chat::engine) initialized: bool,
    /// Whether `initialize` advertised `agentCapabilities.auth.logout`.
    pub(in crate::agent::chat::engine) auth_logout_supported: bool,
    /// Running `agy acp --login` helper process while Antigravity sign-in is in progress.
    pub(in crate::agent::chat::engine) login: Option<Arc<Mutex<super::auth::antigravity::AntigravityLogin>>>,
    load_session: bool,
    /// Whether the agent can reopen a conversation without replaying it (`session/resume`).
    resume_session: bool,
    /// The conversation to continue, set before the handshake.
    resume: Option<String>,
    /// The working directory the session was opened in; ACP requires an absolute one.
    cwd: String,
    /// The request id of the running turn's first prompt.
    pub(super) prompt: Option<String>,
    /// Prompts sent into the running turn. The turn ends only after the first prompt and all of these
    /// have answered; Antigravity delivers each at the next model invocation.
    steers: Vec<String>,
    /// Requests the agent is waiting on, by permission card id.
    pub(super) requests: HashMap<String, PendingRequest>,
    /// While `session/load` replays: its rows and updates, and the rows the old recording had.
    loading: Option<Replay>,
    journal: Option<PathBuf>,
    pending_journal: Vec<String>,
    /// The collaboration mode saved with the session, applied once the session reports its own.
    pub(super) desired_mode: Option<String>,
    /// Target ACP mode id while an `acp_set_mode` request is in flight.
    pending_mode: Option<String>,
    /// Whether startup model/effort selection is waiting for the startup mode switch to finish.
    pending_startup_model_sync: bool,
    /// Whether an async startup `acp_set_model` request is still in flight.
    pending_model_switch: bool,
    /// Native session id waiting to finish Antigravity startup once mode/model sync settles.
    startup_native: Option<String>,
    /// Set when Antigravity opened before `FetchAvailableModels` populated its model list.
    awaiting_initial_models: bool,
    /// Reasoning effort waiting for a startup model switch to publish the target model's options.
    pending_effort: Option<String>,
    /// Active prompt payload kept so Antigravity cold-start model-not-ready errors can retry cleanly.
    active_prompt: Option<ActivePrompt>,
    /// True while a cold-start Antigravity prompt is sleeping before re-dispatching.
    retrying_prompt: bool,
    /// Wakes a sleeping cold-start retry early on `config_option_update`, cancel, or shutdown.
    model_retry_wait: Option<Arc<RetryWait>>,
    /// Antigravity background task state read from `.system_generated/`.
    pub(super) tasks: super::antigravity_tasks::State,
    /// Whether an Antigravity `Stop` hook is currently keeping `session/prompt` open while idle.
    parked_idle: bool,
    /// Whether the active prompt already settled the visible UI turn upon entering `parked_idle`.
    settled_while_parked: bool,
    /// Retired prompt ids that were unparked specifically to hand over to a fresh user turn.
    retiring_prompts: HashSet<String>,
    stop_park: Option<Arc<StopPark>>,
    retire_waiter: Arc<RetireWaiter>,
    /// Message filenames already surfaced by a parked `Stop` hook in this process.
    surfaced_unread: HashSet<String>,
    /// One-shot `schedule` timer keys (`<task_id>:0`) already woken by a parked `Stop` hook.
    woken_schedules: HashSet<String>,
    #[cfg(test)]
    stop_execution_nums: Vec<u64>,
}

impl AcpState {
    pub(super) fn prompt_active(&self) -> bool {
        self.prompt.is_some() || !self.steers.is_empty() || self.retrying_prompt
    }
}

#[derive(Default)]
struct Replay {
    rows: Vec<ChatRow>,
    at: HashMap<String, usize>,
    /// Each replayed update with the first row it produced.
    updates: Vec<(Value, Option<String>)>,
    /// The rows shown before the replay, from the recording; their times and durations carry over.
    previous: Vec<ChatRow>,
}

impl Replay {
    fn place(&mut self, row: ChatRow) {
        match self.at.get(row.id()) {
            Some(&index) => self.rows[index] = row,
            None => { self.at.insert(row.id().to_string(), self.rows.len()); self.rows.push(row); }
        }
    }

    /// Give each replayed message the time, duration and images its recorded counterpart had.
    ///
    /// Rows are matched by role and text, in order, rather than by id: an agent's replay need not repeat
    /// every block its live turn streamed (Copilot replays no thoughts), and ids follow block order.
    fn carry_over(&mut self) -> (HashMap<String, i64>, HashMap<String, u64>) {
        let mut recorded: HashMap<(bool, String), std::collections::VecDeque<(Option<i64>, Option<u64>)>> = HashMap::new();
        for row in &self.previous {
            match row {
                ChatRow::User { text, at, .. } => recorded.entry((true, text.clone())).or_default().push_back((*at, None)),
                ChatRow::Assistant { text, at, duration_ms, .. } => recorded.entry((false, text.clone())).or_default().push_back((*at, *duration_ms)),
                _ => {}
            }
        }
        let (mut times, mut durations) = (HashMap::new(), HashMap::new());
        for row in &mut self.rows {
            let (id, found) = match row {
                ChatRow::User { id, text, .. } => (id.clone(), recorded.get_mut(&(true, text.clone())).and_then(|queue| queue.pop_front())),
                ChatRow::Assistant { id, text, .. } => (id.clone(), recorded.get_mut(&(false, text.clone())).and_then(|queue| queue.pop_front())),
                _ => continue,
            };
            let Some((time, duration)) = found else { continue };
            match row {
                ChatRow::User { at, .. } => *at = time,
                ChatRow::Assistant { at, duration_ms, .. } => { *at = time; *duration_ms = duration; }
                _ => {}
            }
            if let Some(time) = time { times.insert(id.clone(), time); }
            if let Some(duration) = duration { durations.insert(id, duration); }
        }
        // A replay may not repeat the image blocks its live prompt carried. The recording has them, so
        // put them back on the user message with the same text, in order.
        let mut images: HashMap<String, std::collections::VecDeque<Vec<ChatImage>>> = HashMap::new();
        for row in &self.previous {
            if let ChatRow::User { text, images: recorded, .. } = row {
                images.entry(text.clone()).or_default().push_back(recorded.clone());
            }
        }
        for row in &mut self.rows {
            if let ChatRow::User { text, images: replayed, .. } = row {
                if replayed.is_empty() {
                    if let Some(recorded) = images.get_mut(text).and_then(|queue| queue.pop_front()) {
                        *replayed = recorded;
                    }
                }
            }
        }
        // Carry over recorded AskUserQuestion answers when session/load replays only the tool proposal.
        let mut recorded_answers: std::collections::VecDeque<(Option<Value>, Option<Value>, Option<String>)> =
            std::collections::VecDeque::new();
        for row in &self.previous {
            if let ChatRow::Tool { name, input, output, .. } = row {
                if name == "AskUserQuestion" {
                    recorded_answers.push_back((
                        input.get("answers").cloned(),
                        input.get("_answerList").cloned(),
                        output.clone(),
                    ));
                }
            }
        }
        for row in &mut self.rows {
            if let ChatRow::Tool { name, input, output, .. } = row {
                if name == "AskUserQuestion" {
                    let Some((rec_answers, rec_list, rec_output)) = recorded_answers.pop_front() else {
                        break;
                    };
                    if let Some(obj) = input.as_object_mut() {
                        if !obj.contains_key("answers") {
                            if let Some(answers) = rec_answers {
                                obj.insert("answers".into(), answers);
                            }
                        }
                        if !obj.contains_key("_answerList") {
                            if let Some(list) = rec_list {
                                obj.insert("_answerList".into(), list);
                            }
                        }
                    }
                    if output.is_none() {
                        *output = rec_output;
                    }
                }
            }
        }
        (times, durations)
    }
}

/// The conversation's journal file, for agents whose own recording VelaTerm cannot read. Grok's native
/// stream and Antigravity's transcript already are that recording.
fn journal_path(app: &AppCtx, kind: SessionKind, native: &str) -> Option<PathBuf> {
    if matches!(kind, SessionKind::Grok | SessionKind::Antigravity) { return None; }
    wire::journal_path(&app.data_dir().ok()?, kind, native)
}

/// Show the recorded conversation while the agent replays it, so a resumed view does not go blank.
pub(super) fn prefill(app: &AppCtx, proc: &Arc<ChatProcess>, native: &str) {
    let rows = match wire::replay(app.data_dir().ok().as_deref(), proc.kind, native) {
        Ok(rows) => rows,
        Err(_) => return,
    };
    proc.timeline.lock().unwrap().replace_all(rows);
}

/// Send `initialize`; the rest of the handshake follows from its answer.
pub(super) fn bootstrap(app: &AppCtx, proc: &Arc<ChatProcess>, resume: Option<&str>, cwd: Option<&str>, collaboration_mode: Option<&str>) {
    {
        let mut state = proc.acp.lock().unwrap();
        state.mapper = Mapper::for_kind(proc.kind);
        state.resume = resume.map(str::to_string);
        // Grok reopens a session only under the exact cwd text it was filed with, so take that from its store.
        let filed = resume.filter(|_| proc.kind == SessionKind::Grok).and_then(crate::agent::resume::grok_session_cwd);
        state.cwd = filed.unwrap_or_else(|| native_cwd(cwd.map(str::to_string)
            .or_else(|| crate::host::home_dir().map(|home| home.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "/".into())));
        state.journal = resume.and_then(|native| journal_path(app, proc.kind, native));
        state.desired_mode = collaboration_mode.map(str::to_string).filter(|mode| !mode.is_empty());
    }
    let id = proc.request_id("acp_initialize");
    let _ = proc.write(&wire::initialize(&id));
}

/// The cwd in the platform's own spelling. Windows stores worktree paths as `C:/...` (Git's spelling), while an
/// agent started in the terminal sees its process cwd as `C:\...`; agents that key sessions by the cwd text
/// (Grok's directory name, Cursor's hash) must get the same text from both views.
fn native_cwd(cwd: String) -> String {
    #[cfg(windows)]
    {
        let native = std::path::Path::new(&cwd).components().collect::<PathBuf>();
        if !native.as_os_str().is_empty() { return native.to_string_lossy().into_owned(); }
    }
    cwd
}

// ─────────────────────────── Incoming ───────────────────────────

pub(super) fn handle_line(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, line: &str) {
    match wire::parse_line(line) {
        Some(Incoming::Response { id, result, error }) => handle_response(app, session_id, proc, &id, result, error),
        Some(Incoming::Request { id, method, params }) => handle_request(app, session_id, proc, id, &method, params),
        Some(Incoming::Notification { method, params }) => {
            if method == "session/update" {
                if let Some(update) = params.get("update") { handle_update(app, session_id, proc, update); }
            }
        }
        None => {}
    }
}

fn handle_response(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, id: &str, result: Option<Value>, error: Option<Value>) {
    let kind = proc.pending.lock().unwrap().remove(id);
    if let Some(waiter) = proc.waiters.lock().unwrap().remove(id) {
        let _ = waiter.send(match &error {
            Some(error) => Err(wire::error_text(error)),
            None => Ok(result.clone().unwrap_or(Value::Null)),
        });
        return;
    }
    match kind {
        Some("acp_initialize") => match error {
            Some(error) => fail_start(app, session_id, proc, &error),
            None => on_initialize(app, session_id, proc, &result.unwrap_or(Value::Null)),
        },
        Some("acp_session_new") | Some("acp_session_load") | Some("acp_session_resume") => match error {
            // The recorded conversation is gone: its directory, its recording, or the session itself.
            // Start a fresh native conversation instead of leaving the pane unusable, and say so — the
            // rows on screen come from VelaTerm's own recording, which the agent no longer has.
            Some(error) if kind != Some("acp_session_new") && wire::conversation_missing(&error) => {
                crate::diagnostics::record("WARN", "agent_chat", json!({
                    "sessionId":session_id,"event":"native_conversation_missing",
                    "agent":proc.kind.as_str(),"reason":wire::error_text(&error)
                }));
                let cwd = {
                    let mut state = proc.acp.lock().unwrap();
                    state.loading = None;
                    state.resume = None;
                    state.mapper = Mapper::for_kind(proc.kind);
                    state.cwd.clone()
                };
                emit(app, session_id, json!({"type":"error","message":format!(
                    "{} could not reopen that conversation; a new one has been started. Earlier messages stay in this view.",
                    wire::label(proc.kind))}));
                let id = proc.request_id("acp_session_new");
                if proc.write(&wire::session_new(&id, &cwd)).is_err() {
                    proc.acp.lock().unwrap().loading = None;
                    fail_start(app, session_id, proc, &error);
                }
            }
            Some(error) => {
                proc.acp.lock().unwrap().loading = None;
                fail_start(app, session_id, proc, &error);
            }
            None => on_open(app, session_id, proc, &result.unwrap_or(Value::Null), kind != Some("acp_session_new")),
        },
        Some("acp_set_mode") => on_mode_switched(app, session_id, proc, result, error),
        Some("acp_set_model") => on_model_switched(app, session_id, proc, result, error),
        Some("prompt") => on_prompt_end(app, session_id, proc, id, result, error),
        _ => {}
    }
}

fn on_initialize(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, result: &Value) {
    let capabilities = result.get("agentCapabilities").cloned().unwrap_or(Value::Null);
    let (resume, cwd, images) = {
        let mut state = proc.acp.lock().unwrap();
        state.initialized = true;
        state.auth_logout_supported = capabilities.pointer("/auth/logout").is_some_and(|v| !v.is_null());
        state.load_session = capabilities.get("loadSession").and_then(Value::as_bool).unwrap_or(false);
        state.resume_session = capabilities.pointer("/sessionCapabilities/resume").is_some_and(|resume| !resume.is_null());
        let images = capabilities.pointer("/promptCapabilities/image").and_then(Value::as_bool).unwrap_or(false);
        state.images = images;
        state.images_known = true;
        (state.resume.clone(), state.cwd.clone(), images)
    };
    emit(app, session_id, json!({"type":"capabilities","images":images}));
    match resume {
        // Resuming replays nothing: the rows on screen came from the agent's own recording and stay.
        Some(native) if !proc.acp.lock().unwrap().load_session && proc.acp.lock().unwrap().resume_session => {
            let id = proc.request_id("acp_session_resume");
            let _ = proc.write(&wire::session_resume(&id, &native, &cwd));
        }
        Some(native) => {
            if !proc.acp.lock().unwrap().load_session {
                emit(app, session_id, json!({"type":"error","message":
                    format!("{} cannot reopen an earlier conversation. Continue it in the terminal view.", wire::label(proc.kind))}));
                super::release_process(app, session_id, proc);
                return;
            }
            // The rows already on screen came from the recording; their times carry over to the replay.
            let previous = proc.timeline.lock().unwrap().rows.clone();
            {
                let mut state = proc.acp.lock().unwrap();
                state.mapper = Mapper::replay();
                state.loading = Some(Replay { previous, ..Replay::default() });
            }
            let id = proc.request_id("acp_session_load");
            let _ = proc.write(&wire::session_load(&id, &native, &cwd));
        }
        None => {
            let id = proc.request_id("acp_session_new");
            let _ = proc.write(&wire::session_new(&id, &cwd));
        }
    }
}

/// The session is open: record its identity and catalogue, settle a replay, and start taking turns.
fn on_open(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, result: &Value, loaded: bool) {
    let native = if loaded {
        proc.acp.lock().unwrap().resume.clone()
    } else {
        result.get("sessionId").and_then(Value::as_str).map(str::to_string)
    };
    let Some(native) = native.filter(|id| !id.is_empty()) else {
        emit(app, session_id, json!({"type":"error","message":"The agent did not open a conversation."}));
        super::release_process(app, session_id, proc);
        return;
    };
    if !remember_session(app, session_id, proc, &native) { return; }
    if loaded { finish_replay(proc); }
    let (catalog_snapshot, presets, current) = {
        let mut state = proc.acp.lock().unwrap();
        state.catalog.absorb_session(result);
        if state.journal.is_none() { state.journal = journal_path(app, proc.kind, &native); }
        let presets = presets(&state.catalog);
        let current = state.catalog.mode.clone();
        (state.catalog.clone(), presets, current)
    };
    if proc.kind == SessionKind::Antigravity {
        proc.acp.lock().unwrap().awaiting_initial_models = !catalog_snapshot.model_switchable();
        crate::agent::antigravity_models::remember_acp(app, &catalog_snapshot);
        super::antigravity_tasks::on_open(app, session_id, proc, &native);
    }
    apply_saved_selection(app, session_id, proc, &presets, current);
    let images = {
        let state = proc.acp.lock().unwrap();
        state.images_known.then_some(state.images)
    };
    emit(app, session_id, json!({"type":"settingsChanged","model":proc.model.lock().unwrap().clone(),
        "effort":proc.effort.lock().unwrap().clone(), "images":images}));
    // Hold Antigravity's ready announcement until startup mode/model switches have written their
    // final config request, so the first queued prompt cannot race ahead of them on stdin.
    let defer_startup = proc.kind == SessionKind::Antigravity && {
        let mut state = proc.acp.lock().unwrap();
        if state.pending_startup_model_sync || state.pending_model_switch {
            state.startup_native = Some(native.clone());
            true
        } else {
            false
        }
    };
    if !defer_startup {
        finish_startup(app, session_id, proc, &native);
    }
}

fn maybe_finish_deferred_startup(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let ready_native = {
        let mut state = proc.acp.lock().unwrap();
        if state.pending_startup_model_sync || state.pending_model_switch {
            None
        } else {
            state.startup_native.take()
        }
    };
    if let Some(native) = ready_native {
        finish_startup(app, session_id, proc, &native);
    }
}

/// Mark the engine ready and then announce the native session.
///
/// A client that waits for `session` sends its first message as soon as the event arrives. Announcing
/// earlier — while the replay or the saved selection is still settling — would put that message into the
/// startup queue, so it would leave the conversation for the pending list and come back once it drained.
pub(super) fn finish_startup(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, native: &str) {
    let opening = !proc.ready.swap(true, Ordering::Relaxed);
    emit(app, session_id, json!({"type":"session","agentSessionId":native,"model":proc.model.lock().unwrap().clone()}));
    if opening {
        emit_state(app, session_id, AgentState::Waiting);
        start_waiting_message(app, session_id, proc);
    }
}

/// The replay is complete: its rows replace the recording's, and the journal is rewritten from it.
fn finish_replay(proc: &Arc<ChatProcess>) {
    let (rows, lines, journal) = {
        let mut state = proc.acp.lock().unwrap();
        let Some(mut replay) = state.loading.take() else { return };
        let tail: Vec<ChatRow> = state.mapper.finish().into_iter().chain(state.mapper.settle_running(true)).collect();
        for row in tail { replay.place(row); }
        state.mapper.replaying = false;
        let (times, durations) = replay.carry_over();
        let lines: Vec<(String, Option<String>)> = replay.updates.iter().map(|(update, row)| {
            let line = match row.as_ref().and_then(|id| times.get(id)) {
                Some(at) => wire::journal_line(update, *at),
                None => json!({"method":"session/update","params":{"update":update}}).to_string(),
            };
            (line, row.clone())
        }).collect();
        let lines = journal_with_turn_ends(&lines, &durations);
        (replay.rows, lines, state.journal.clone())
    };
    let mut rows = rows;
    super::super::shell::map_replayed_rows(&mut rows);
    proc.timeline.lock().unwrap().replace_all(rows);
    if let Some(path) = journal {
        if let Err(error) = wire::rewrite_journal(&path, &lines) {
            crate::diagnostic_warn!("chat: ACP journal rewrite failed: {error}");
        }
    }
}

/// Replayed updates as journal lines, with the recorded turn durations put back after each turn's answer.
fn journal_with_turn_ends(lines: &[(String, Option<String>)], durations: &HashMap<String, u64>) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut pending = None;
    for (line, row) in lines {
        let row = row.as_deref().unwrap_or("");
        if row.starts_with("acp-u") {
            if let Some(duration) = pending.take() { out.push(wire::journal_turn_end(duration, 0)); }
        } else if let Some(duration) = durations.get(row) {
            pending = Some(*duration);
        }
        out.push(line.clone());
    }
    if let Some(duration) = pending { out.push(wire::journal_turn_end(duration, 0)); }
    out
}

fn remember_session(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, native: &str) -> bool {
    if !verify_native_identity(app, session_id, proc, native) { return false; }
    let changed = {
        let mut previous = proc.agent_session_id.lock().unwrap();
        let changed = previous.as_deref() != Some(native);
        *previous = Some(native.to_string());
        changed
    };
    if changed {
        let stored = crate::db::repo::set_agent_session_id(&app.db().conn.lock().unwrap(), session_id, native, proc.kind)
            .unwrap_or(false);
        if stored { app.emit(crate::host::TREE_CHANGED, ()); }
    }
    true
}

/// Session modes as collaboration presets, which is how the view offers a choice of working style.
fn presets(catalog: &Catalog) -> Vec<CollaborationModePreset> {
    catalog.modes.iter().map(|mode| CollaborationModePreset {
        name: mode.name.clone(), mode: mode.id.clone(), reasoning_effort: None, description: mode.description.clone(),
    }).collect()
}

/// Pick the ACP mode that backs Antigravity's Bypass mode when `auto` may be omitted.
fn antigravity_bypass_acp_mode(modes: &[wire::Choice]) -> &'static str {
    ["auto", "accept-edits", "default"]
        .into_iter()
        .find(|candidate| modes.iter().any(|choice| choice.id == *candidate))
        .unwrap_or("auto")
}

/// Bring the opened session to the mode, model and reasoning level saved with this conversation.
fn apply_saved_selection(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, presets: &[CollaborationModePreset], current: Option<String>) {
    if proc.kind == SessionKind::Antigravity {
        let mut selection = crate::agent::session_settings::Selection {
            model: proc.model.lock().unwrap().clone(),
            effort: proc.effort.lock().unwrap().clone(),
        };
        crate::agent::antigravity_models::normalize_selection(app, &mut selection);
        *proc.model.lock().unwrap() = selection.model;
        *proc.effort.lock().unwrap() = selection.effort;
    }
    let desired_mode = {
        let mut state = proc.acp.lock().unwrap();
        state.desired_mode.take().or_else(|| {
            (proc.kind == SessionKind::Antigravity)
                .then(|| acp_mode_for_permission(&proc.mode.lock().unwrap()).to_string())
        })
    };
    let desired_mode = if proc.kind == SessionKind::Antigravity
        && proc.mode.lock().unwrap().as_str() == "bypassPermissions"
        && desired_mode.as_deref().is_some_and(|mode| !presets.iter().any(|preset| preset.mode == mode))
    {
        let catalog_modes = proc.acp.lock().unwrap().catalog.modes.clone();
        Some(antigravity_bypass_acp_mode(&catalog_modes).to_string())
    } else {
        desired_mode
    };
    let mut selected = current.clone();
    let mut mode_switching = false;
    if let Some(mode) = desired_mode.filter(|mode| presets.iter().any(|preset| &preset.mode == mode)) {
        if Some(&mode) != current.as_ref() {
            // Wait for the mode switch to complete before sending model/effort changes so the agent
            // cannot emit `config_option_update` with the pre-switch mode while both run concurrently.
            proc.acp.lock().unwrap().pending_startup_model_sync = true;
            if set_mode_request(proc, &mode, false).is_ok() {
                selected = Some(mode);
                mode_switching = true;
            } else {
                proc.acp.lock().unwrap().pending_startup_model_sync = false;
            }
        }
    }
    *proc.collaboration_mode.lock().unwrap() = selected.clone();
    if proc.kind != SessionKind::Antigravity {
        *proc.collaboration_modes.lock().unwrap() = presets.to_vec();
        emit(app, session_id, json!({"type":"collaborationModes","modes":presets,"mode":selected}));
    }

    if mode_switching {
        let catalog = proc.acp.lock().unwrap().catalog.clone();
        if catalog.model_switchable() {
            let invalid_saved = {
                let saved = proc.model.lock().unwrap();
                saved.as_deref().is_none_or(|model| !catalog.models.iter().any(|choice| choice.id == model))
            };
            if invalid_saved {
                *proc.model.lock().unwrap() = catalog.model.clone();
                if !catalog.effort_switchable() {
                    *proc.effort.lock().unwrap() = None;
                }
            }
        }
    } else {
        apply_saved_model_and_effort(proc);
    }
}

fn apply_saved_model_and_effort(proc: &Arc<ChatProcess>) {
    let catalog = proc.acp.lock().unwrap().catalog.clone();
    let mut model_switching = false;
    if catalog.model_switchable() {
        let desired = proc.model.lock().unwrap().clone();
        match desired.filter(|model| catalog.model.as_deref() != Some(model.as_str()) && catalog.models.iter().any(|choice| &choice.id == model)) {
            Some(model) => {
                // Switching models can widen or drop `thought_level` options, so wait for the model
                // response before applying the saved effort.
                let desired_effort = proc.effort.lock().unwrap().clone();
                if set_model_request(proc, &model, false).is_ok() {
                    let mut state = proc.acp.lock().unwrap();
                    state.pending_effort = desired_effort;
                    state.pending_model_switch = true;
                    model_switching = true;
                } else {
                    *proc.model.lock().unwrap() = catalog.model.clone();
                }
            }
            None => *proc.model.lock().unwrap() = catalog.model.clone(),
        }
    }
    if !model_switching {
        if catalog.effort_switchable() {
            let desired = proc.effort.lock().unwrap().clone();
            match desired.filter(|effort| catalog.effort.as_deref() != Some(effort.as_str()) && catalog.efforts.iter().any(|choice| &choice.id == effort)) {
                Some(effort) => { if set_effort_request(proc, &effort, false).is_err() { *proc.effort.lock().unwrap() = catalog.effort.clone(); } }
                None => *proc.effort.lock().unwrap() = catalog.effort.clone(),
            }
        } else if catalog.model_switchable() {
            *proc.effort.lock().unwrap() = None;
        }
    }
}

/// Complete an asynchronous mode switch and resume deferred startup model/effort selection.
fn on_mode_switched(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, result: Option<Value>, error: Option<Value>) {
    let (expected, run_model_sync, fallback_mode, catalog) = {
        let mut state = proc.acp.lock().unwrap();
        let expected = state.pending_mode.take();
        let run_model_sync = std::mem::take(&mut state.pending_startup_model_sync);
        if error.is_none() {
            if let Some(options) = result.as_ref().and_then(|res| res.get("configOptions")).and_then(Value::as_array) {
                state.catalog.absorb_options(options);
            }
            if let Some(mode) = expected.as_ref() {
                state.catalog.mode = Some(mode.clone());
            }
        }
        let fallback_mode = error.as_ref().and_then(|_| state.catalog.mode.clone());
        (expected, run_model_sync, fallback_mode, state.catalog.clone())
    };
    if proc.kind == SessionKind::Antigravity && error.is_none() {
        crate::agent::antigravity_models::remember_acp(app, &catalog);
    }
    if let Some(actual) = fallback_mode {
        adopt_mode(app, session_id, proc, &actual);
    } else if let Some(mode) = expected {
        adopt_mode(app, session_id, proc, &mode);
    }
    if run_model_sync {
        let prev_model = proc.model.lock().unwrap().clone();
        let prev_effort = proc.effort.lock().unwrap().clone();
        apply_saved_model_and_effort(proc);
        let next_model = proc.model.lock().unwrap().clone();
        let next_effort = proc.effort.lock().unwrap().clone();
        if next_model != prev_model || next_effort != prev_effort {
            emit(app, session_id, json!({"type":"settingsChanged","model":next_model,"effort":next_effort}));
        }
    }
    maybe_finish_deferred_startup(app, session_id, proc);
}

/// Complete a startup model switch after `session/set_config_option` returns the new model's options.
fn on_model_switched(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, result: Option<Value>, error: Option<Value>) {
    if error.is_some() {
        let (model, retry_wait) = {
            let mut state = proc.acp.lock().unwrap();
            state.pending_model_switch = false;
            state.pending_effort = None;
            (state.catalog.model.clone(), state.model_retry_wait.clone())
        };
        *proc.model.lock().unwrap() = model.clone();
        emit(app, session_id, json!({"type":"settingsChanged","model":model,"effort":proc.effort.lock().unwrap().clone()}));
        maybe_finish_deferred_startup(app, session_id, proc);
        if let Some(wait) = retry_wait {
            wait.wake();
        }
        return;
    }
    let current_model = proc.model.lock().unwrap().clone();
    let (catalog, desired_effort, retry_wait) = {
        let mut state = proc.acp.lock().unwrap();
        state.pending_model_switch = false;
        let pending_mode = state.pending_mode.clone();
        if let Some(options) = result.as_ref().and_then(|res| res.get("configOptions")).and_then(Value::as_array) {
            state.catalog.absorb_options(options);
            if let Some(expected) = pending_mode {
                state.catalog.mode = Some(expected);
            }
        } else if let Some(model) = current_model {
            state.catalog.model = Some(model);
        }
        let desired_effort = state.pending_effort.take();
        (state.catalog.clone(), desired_effort, state.model_retry_wait.clone())
    };
    if proc.kind == SessionKind::Antigravity {
        crate::agent::antigravity_models::remember_acp(app, &catalog);
    }
    if catalog.effort_switchable() {
        let valid = desired_effort.filter(|eff| catalog.efforts.iter().any(|c| &c.id == eff));
        match valid {
            Some(eff) if catalog.effort.as_deref() != Some(eff.as_str()) => {
                if set_effort_request(proc, &eff, false).is_err() {
                    *proc.effort.lock().unwrap() = catalog.effort.clone();
                }
            }
            Some(eff) => *proc.effort.lock().unwrap() = Some(eff),
            None => *proc.effort.lock().unwrap() = catalog.effort.clone(),
        }
    } else {
        *proc.effort.lock().unwrap() = None;
    }
    emit(app, session_id, json!({"type":"models"}));
    emit(app, session_id, json!({"type":"settingsChanged","model":proc.model.lock().unwrap().clone(),
        "effort":proc.effort.lock().unwrap().clone()}));
    maybe_finish_deferred_startup(app, session_id, proc);
    if let Some(wait) = retry_wait {
        wait.wake();
    }
}

/// The agent refused to start the conversation. Signing in is the one cause the person can act on, so it
/// becomes a stable code the view explains; anything else is shown as the agent wrote it.
fn fail_start(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, error: &Value) {
    if proc.kind == SessionKind::Antigravity && wire::auth_required(error) {
        let waiting: Vec<_> = proc.turn.lock().unwrap().waiting.drain(..).collect();
        if !waiting.is_empty() {
            let ids: Vec<String> = waiting.iter().map(|item| item.id.clone()).collect();
            let _ = crate::agent::spawn_requests::cancel_queued_many(app, session_id, &ids);
            super::emit_queue(app, session_id, proc);
            for item in waiting.into_iter().rev() {
                emit(app, session_id, json!({"type":"unsent","id":item.id,"text":item.text,"images":item.images}));
            }
        }
        // Keep the initialized ACP process alive so the inline Antigravity account panel can drive sign-in.
        super::auth::antigravity::require(app, session_id, proc);
        return;
    }
    let message = if wire::auth_required(error) {
        format!("agent_sign_in_required:{}", proc.kind.as_str())
    } else if wire::api_key_missing(error) {
        format!("agent_api_key_required:{}", proc.kind.as_str())
    } else {
        format!("{} could not open the conversation: {}", wire::label(proc.kind), wire::error_text(error))
    };
    // The exit that follows clears the queue; its text goes back to the view rather than being lost.
    let unsent: Vec<String> = proc.turn.lock().unwrap().waiting.iter().map(|item| item.text.clone()).collect();
    emit(app, session_id, json!({"type":"error","message":message,"unsent":unsent}));
    super::release_process(app, session_id, proc);
}

fn handle_update(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, update: &Value) {
    let kind = update.get("sessionUpdate").and_then(Value::as_str).unwrap_or("");
    match kind {
        "available_commands_update" => {
            let commands = wire::commands(update);
            *proc.commands.lock().unwrap() = commands.clone();
            emit(app, session_id, json!({"type":"commands","commands":commands}));
            return;
        }
        "current_mode_update" => {
            if let Some(mode) = update.get("currentModeId").and_then(Value::as_str) {
                let stale = {
                    let mut state = proc.acp.lock().unwrap();
                    if state.pending_mode.as_deref().is_some_and(|expected| expected != mode) {
                        true
                    } else {
                        state.catalog.mode = Some(mode.to_string());
                        state.desired_mode.as_deref().is_some_and(|desired| desired != mode)
                    }
                };
                if !stale {
                    adopt_mode(app, session_id, proc, mode);
                }
            }
            return;
        }
        "config_option_update" => {
            let options = update.get("configOptions").and_then(Value::as_array).cloned().unwrap_or_default();
            let (changed_models, changed_modes, catalog, mode, model, effort, deferring_model, switching, initial_models_arrived, retry_wait) = {
                let mut state = proc.acp.lock().unwrap();
                let prev_models = state.catalog.models.clone();
                let prev_efforts = state.catalog.efforts.clone();
                let prev_model_efforts = state.catalog.model_efforts.clone();
                let prev_modes = state.catalog.modes.clone();
                let saw_mode = state.catalog.absorb_options(&options);
                let mut adopt = if saw_mode { state.catalog.mode.clone() } else { None };
                if let Some(expected) = state.pending_mode.as_deref() {
                    if state.catalog.mode.as_deref() != Some(expected) {
                        adopt = None;
                    }
                }
                if state.desired_mode.as_deref().is_some_and(|desired| Some(desired) != state.catalog.mode.as_deref()) {
                    adopt = None;
                }
                let changed_models = state.catalog.models != prev_models
                    || state.catalog.efforts != prev_efforts
                    || state.catalog.model_efforts != prev_model_efforts;
                let changed_modes = state.catalog.modes != prev_modes;
                let initial_models_arrived = proc.kind == SessionKind::Antigravity
                    && state.awaiting_initial_models
                    && state.catalog.model_switchable();
                if initial_models_arrived {
                    state.awaiting_initial_models = false;
                }
                (
                    changed_models,
                    changed_modes,
                    state.catalog.clone(),
                    adopt,
                    state.catalog.model.clone(),
                    state.catalog.effort.clone(),
                    state.pending_startup_model_sync || initial_models_arrived,
                    state.pending_effort.is_some()
                        || state.pending_model_switch
                        || state.pending_startup_model_sync
                        || initial_models_arrived,
                    initial_models_arrived,
                    state.model_retry_wait.clone(),
                )
            };
            if proc.kind == SessionKind::Antigravity {
                crate::agent::antigravity_models::remember_acp(app, &catalog);
            }
            if let Some(mode) = mode { adopt_mode(app, session_id, proc, &mode); }
            if changed_modes && proc.kind != SessionKind::Antigravity {
                let next_presets = presets(&catalog);
                *proc.collaboration_modes.lock().unwrap() = next_presets.clone();
                emit(app, session_id, json!({"type":"collaborationModes","modes":next_presets,
                    "mode":proc.collaboration_mode.lock().unwrap().clone()}));
            }
            if initial_models_arrived && !proc.acp.lock().unwrap().pending_startup_model_sync {
                // `session/new` or `session/resume` returned before `FetchAvailableModels` finished;
                // apply the conversation's saved model and effort now instead of keeping the default.
                apply_saved_model_and_effort(proc);
            } else {
                if !deferring_model && catalog.model_switchable() && model.is_some() { *proc.model.lock().unwrap() = model; }
                // Wait for `on_model_switched` while a startup model switch's effort is still pending.
                if !switching {
                    if catalog.effort_switchable() {
                        if effort.is_some() { *proc.effort.lock().unwrap() = effort; }
                    } else if catalog.model_switchable() {
                        *proc.effort.lock().unwrap() = None;
                    }
                }
            }
            if (initial_models_arrived || (changed_models && catalog.model_switchable()))
                && !proc.acp.lock().unwrap().pending_model_switch
            {
                if let Some(wait) = retry_wait {
                    wait.wake();
                }
            }
            if changed_models {
                emit(app, session_id, json!({"type":"models"}));
            }
            emit(app, session_id, json!({"type":"settingsChanged","model":proc.model.lock().unwrap().clone(),
                "effort":proc.effort.lock().unwrap().clone()}));
            return;
        }
        // How full the context window is, which the composer's usage ring shows.
        "usage_update" => {
            let used = update.get("used").and_then(Value::as_u64);
            let size = update.get("size").and_then(Value::as_u64).filter(|size| *size > 0);
            if used.is_some() || size.is_some() {
                {
                    let mut extras = proc.extras.lock().unwrap();
                    if used.is_some() { extras.context_tokens = used; }
                    if size.is_some() { extras.context_window = size; }
                }
                super::emit_extras(app, session_id, proc);
            }
            return;
        }
        "session_info_update" => {
            // Grok names its own conversation and refreshes that title as it develops. Adopt it in the
            // sidebar while the session still carries a placeholder or a title the agent itself wrote.
            if proc.kind == SessionKind::Grok {
                if let Some(title) = update.get("title").and_then(Value::as_str) {
                    crate::agent::server::rename_from_agent_title(app, session_id, title);
                }
            }
            return;
        }
        _ => {}
    }
    let mut state = proc.acp.lock().unwrap();
    let rows = state.mapper.update(update);
    if let Some(replay) = state.loading.as_mut() {
        replay.updates.push((update.clone(), rows.first().map(|row| row.id().to_string())));
        for row in rows { replay.place(row); }
        return;
    }
    if !rows.is_empty() {
        if let Some(active) = state.active_prompt.as_mut() {
            active.had_output = true;
        }
    }
    if state.journal.is_some() {
        state.pending_journal.push(wire::journal_line(update, now_ms() as i64));
        if state.pending_journal.len() >= JOURNAL_BATCH { flush_journal_locked(&mut state); }
    }
    drop(state);
    let model = proc.model.lock().unwrap().as_deref().map(|model| wire::reply_model(proc.kind, model));
    {
        let mut timeline = proc.timeline.lock().unwrap();
        for mut row in rows {
            // The first chunk dates the reply; later chunks keep that time.
            let first_at = match timeline.get(row.id()) { Some(ChatRow::Assistant { at, .. }) => *at, _ => None };
            if let ChatRow::Assistant { at, model: reply_model, .. } = &mut row {
                *at = first_at.or(Some(now_ms() as i64));
                *reply_model = model.clone();
            }
            timeline.upsert(row);
        }
    }
    if proc.kind == SessionKind::Antigravity && matches!(kind, "tool_call" | "tool_call_update") {
        super::antigravity_tasks::ensure_poller(app, session_id, proc);
    }
}

/// The agent changed its own mode, or answered a change: the view's capsule follows.
fn adopt_mode(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, mode: &str) {
    let previous = proc.collaboration_mode.lock().unwrap().replace(mode.to_string());
    if previous.as_deref() != Some(mode) {
        let _ = crate::db::repo::set_collaboration_mode(&app.db().conn.lock().unwrap(), session_id, mode);
        if proc.kind != SessionKind::Antigravity {
            emit(app, session_id, json!({"type":"collaborationModeChanged","mode":mode}));
        }
    }
    if proc.kind == SessionKind::Antigravity {
        let current_perm = proc.mode.lock().unwrap().clone();
        let bypass_backing = {
            let state = proc.acp.lock().unwrap();
            antigravity_bypass_acp_mode(&state.catalog.modes)
        };
        let next_perm = if current_perm == "bypassPermissions" && mode == bypass_backing {
            "bypassPermissions"
        } else {
            permission_for_acp_mode(mode, &current_perm)
        };
        if next_perm != current_perm {
            *proc.mode.lock().unwrap() = next_perm.to_string();
            let stored = if next_perm == "bypassPermissions" { "skip" } else { next_perm };
            let _ = crate::db::repo::set_permission_mode(&app.db().conn.lock().unwrap(), session_id, stored);
            emit(app, session_id, json!({"type":"settingsChanged","mode":next_perm,"pendingPermissionMode":Value::Null}));
        }
    }
}

fn handle_request(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, id: Value, method: &str, params: Value) {
    match method {
        "session/request_permission" => {
            if proc.mode.lock().unwrap().as_str() == "bypassPermissions" {
                let option = wire::choose_option(params.get("options"), true, None);
                let _ = proc.write(&wire::permission_outcome(&id, option.as_deref()));
                return;
            }
            let card = format!("acp-{}", wire::id_key(&id));
            let payload = wire::permission_payload(proc.kind, &card, &params, Some(&proc.acp.lock().unwrap().mapper));
            proc.acp.lock().unwrap().requests.insert(card.clone(), PendingRequest::Permission { id, params });
            proc.permissions.lock().unwrap().insert(card, payload.clone());
            emit(app, session_id, json!({"type":"permission","request":payload}));
            emit_state(app, session_id, AgentState::Asking);
        }
        "elicitation/create" => {
            // Keep elicitation prompts (including `ask_question`) interactive even in bypassPermissions
            // so the user still answers questions instead of auto-submitting empty forms.
            let card = format!("acp-elicit-{}", wire::id_key(&id));
            let payload = wire::elicitation_payload(proc.kind, &card, &params);
            proc.acp.lock().unwrap().requests.insert(
                card.clone(),
                PendingRequest::Elicitation { id, params, card_payload: payload.clone() },
            );
            proc.permissions.lock().unwrap().insert(card, payload.clone());
            emit(app, session_id, json!({"type":"permission","request":payload}));
            emit_state(app, session_id, AgentState::Asking);
        }
        wire::EXIT_PLAN_MODE | wire::ASK_USER_QUESTION | wire::MCP_ELICIT => {
            // These wait for the person even in bypass, as in Grok's own terminal: an error answer would
            // cancel a plan, a question, or a form the model is waiting on.
            let card = format!("acp-grok-{}", wire::id_key(&id));
            let Some(payload) = wire::grok_dialog(proc.kind, &card, method, &params) else {
                let _ = proc.write(&wire::error_response(&id, -32601, &format!("{method} is not supported by this client")));
                return;
            };
            proc.acp.lock().unwrap().requests.insert(
                card.clone(),
                PendingRequest::GrokDialog { id, method: method.to_string(), params },
            );
            proc.permissions.lock().unwrap().insert(card, payload.clone());
            emit(app, session_id, json!({"type":"permission","request":payload}));
            emit_state(app, session_id, AgentState::Asking);
        }
        _ => {
            // Neither file system nor terminal access was offered; anything else is unknown to this client.
            let _ = proc.write(&wire::error_response(&id, -32601, &format!("{method} is not supported by this client")));
        }
    }
}

/// Answer an interactive card: permission, elicitation (`ask_question` / MCP), or detached recovery.
pub(super) fn respond(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    card: &str,
    allow: bool,
    updated_input: Option<&Value>,
    message: Option<&str>,
    updated_permissions: Option<&Value>,
) -> Result<(), String> {
    let entry = proc.acp.lock().unwrap().requests.get(card).cloned()
        .ok_or("That permission request is no longer waiting for an answer")?;
    match entry {
        PendingRequest::Permission { id, params } => {
            let always = updated_permissions.and_then(Value::as_array).into_iter().flatten()
                .find(|value| value.get("type").and_then(Value::as_str) == Some("acpAllowAlways"))
                .and_then(|value| value.get("optionId").and_then(Value::as_str));
            let option = wire::choose_option(params.get("options"), allow, always);
            proc.write(&wire::permission_outcome(&id, option.as_deref()))?;
            resolve(app, session_id, proc, card);
        }
        PendingRequest::Elicitation { id, params, card_payload } => {
            proc.write(&wire::elicitation_outcome(&id, &card_payload, &params, allow, updated_input, message))?;
            let updated_row = if card_payload.get("tool_name").and_then(Value::as_str) == Some("AskUserQuestion") {
                proc.acp.lock().unwrap().mapper.record_ask_question_answer(updated_input, allow)
            } else {
                None
            };
            if let Some(row) = updated_row {
                proc.timeline.lock().unwrap().upsert(row);
            }
            resolve(app, session_id, proc, card);
        }
        PendingRequest::GrokDialog { id, method, .. } => {
            proc.write(&wire::grok_dialog_answer(&id, &method, allow, updated_input, message))?;
            resolve(app, session_id, proc, card);
        }
        PendingRequest::DetachedRecovery { row_id, tool_name, args, card_payload } => {
            if tool_name == "ask_question" {
                let updated_row = {
                    let timeline = proc.timeline.lock().unwrap();
                    timeline.get(&row_id).cloned().and_then(|row| match row {
                        ChatRow::Tool { id, name, input, mut output, is_error, status, subagent, children } => {
                            let mut input = crate::agent::chat::antigravity_protocol::normalize_ask_question_input(&input);
                            if allow {
                                if let Some(obj) = input.as_object_mut() {
                                    if let Some(answers) = updated_input.and_then(|u| u.get("answers")) {
                                        obj.insert("answers".into(), answers.clone());
                                    }
                                    if let Some(list) = updated_input.and_then(|u| u.get("_answerList")) {
                                        obj.insert("_answerList".into(), list.clone());
                                    }
                                }
                            } else if output.is_none() {
                                output = Some("Dismissed by user".to_string());
                            }
                            Some(ChatRow::Tool { id, name, input, output, is_error, status, subagent, children })
                        }
                        _ => None,
                    })
                };
                if let Some(row) = updated_row {
                    proc.timeline.lock().unwrap().upsert(row);
                }
            }
            super::antigravity_tasks::settle_rows(proc, std::slice::from_ref(&row_id));
            resolve(app, session_id, proc, card);
            if allow {
                let resume_text = detached_recovery_prompt(&tool_name, &args, &card_payload, updated_input);
                retire_parked_stop_before_send(proc);
                let started_at = {
                    let mut turn = proc.turn.lock().unwrap();
                    turn.running = true;
                    turn.interrupted = false;
                    let now = now_ms();
                    turn.started_at = Some(now);
                    now
                };
                emit(app, session_id, json!({"type":"turnStarted","startedAt":started_at}));
                emit_state(app, session_id, AgentState::Working);
                if let Err(error) = dispatch(proc, &resume_text, &[]) {
                    let mut turn = proc.turn.lock().unwrap();
                    turn.running = false;
                    turn.started_at = None;
                    drop(turn);
                    emit(app, session_id, json!({"type":"turnCompleted"}));
                    emit_state(app, session_id, AgentState::Waiting);
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}

fn detached_recovery_prompt(tool_name: &str, args: &Value, card_payload: &Value, updated_input: Option<&Value>) -> String {
    if tool_name == "ask_question" {
        let answers = updated_input
            .and_then(|v| v.get("answers"))
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or_else(|| card_payload.get("input").cloned().unwrap_or(Value::Null));
        let json_answers = serde_json::to_string(&answers).unwrap_or_else(|_| "{}".to_string());
        format!("The user answered your paused question: {json_answers}. Continue your work from where you paused.")
    } else {
        let json_args = serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string());
        format!("The user granted permission for tool `{tool_name}` with arguments {json_args}. Proceed with executing this step and continue your task.")
    }
}

/// Recover a headless reactive tool call that stalled without an open ACP `session/prompt`.
pub(super) fn recover_stalled_tool(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    row_id: &str,
    tool_name: &str,
    args: &Value,
) {
    if let Ok(native) = session(proc) {
        let _ = proc.write(&wire::cancel(&native));
    }
    let other_rows: Vec<String> = {
        let mut state = proc.acp.lock().unwrap();
        state
            .tasks
            .take_aborted_reactive_rows()
            .into_iter()
            .filter(|id| id != row_id)
            .collect()
    };
    super::antigravity_tasks::settle_rows(proc, &other_rows);
    let card = format!("acp-detached-{row_id}");
    let payload = if tool_name == "ask_question" {
        let mut input = crate::agent::chat::antigravity_protocol::normalize_ask_question_input(args);
        if let Some(obj) = input.as_object_mut() {
            obj.insert("_detachedRecovery".into(), json!(true));
        }
        json!({
            "id": card,
            "tool_name": "AskUserQuestion",
            "display_name": wire::label(proc.kind),
            "description": args.get("message").or_else(|| args.get("prompt")).cloned().unwrap_or(Value::Null),
            "input": input,
        })
    } else {
        let (mapped_name, mapped_input) = match tool_row(row_id.to_string(), tool_name, args, None, false, false) {
            ChatRow::Tool { name, input, .. } => (name, input),
            _ => (tool_name.to_string(), args.clone()),
        };
        json!({
            "id": card,
            "tool_name": mapped_name,
            "display_name": wire::label(proc.kind),
            "input": mapped_input,
        })
    };
    proc.acp.lock().unwrap().requests.insert(
        card.clone(),
        PendingRequest::DetachedRecovery {
            row_id: row_id.to_string(),
            tool_name: tool_name.to_string(),
            args: args.clone(),
            card_payload: payload.clone(),
        },
    );
    proc.permissions.lock().unwrap().insert(card, payload.clone());
    emit(app, session_id, json!({"type":"permission","request":payload}));
    emit_state(app, session_id, AgentState::Asking);
}

fn resolve(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, card: &str) {
    proc.acp.lock().unwrap().requests.remove(card);
    proc.permissions.lock().unwrap().remove(card);
    emit(app, session_id, json!({"type":"permissionResolved","id":card}));
    if !proc.permissions.lock().unwrap().is_empty() { return; }
    emit_state(app, session_id, if proc.turn.lock().unwrap().running { AgentState::Working } else { AgentState::Waiting });
}

/// Let every waiting permission request through, used when the session switches to bypass.
/// Elicitation dialogs (`ask_question`) and detached recoveries still wait for explicit input.
pub(super) fn approve_pending(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let requests: Vec<(String, Value, Value)> = proc.acp.lock().unwrap().requests.iter()
        .filter_map(|(card, entry)| match entry {
            PendingRequest::Permission { id, params } => Some((card.clone(), id.clone(), params.clone())),
            _ => None,
        })
        .collect();
    for (card, id, params) in requests {
        let option = wire::choose_option(params.get("options"), true, None);
        if proc.write(&wire::permission_outcome(&id, option.as_deref())).is_ok() { resolve(app, session_id, proc, &card); }
    }
}

/// A finished turn leaves no question open: any request still waiting is answered as cancelled, and every
/// card is cleared, including those whose answer `cancel` already sent.
fn cancel_pending(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let drained: Vec<PendingRequest> = proc.acp.lock().unwrap().requests.drain().map(|(_, entry)| entry).collect();
    for entry in drained {
        match entry {
            PendingRequest::Permission { id, .. } => {
                let _ = proc.write(&wire::permission_outcome(&id, None));
            }
            PendingRequest::Elicitation { id, params, card_payload } => {
                let _ = proc.write(&wire::elicitation_outcome(&id, &card_payload, &params, false, None, Some("cancel")));
            }
            PendingRequest::GrokDialog { id, method, .. } => {
                let _ = proc.write(&wire::grok_cancelled_outcome(&id, &method));
            }
            PendingRequest::DetachedRecovery { row_id, .. } => {
                super::antigravity_tasks::settle_rows(proc, std::slice::from_ref(&row_id));
            }
        }
    }
    let cards: Vec<String> = proc.permissions.lock().unwrap().keys().cloned().collect();
    for card in cards { resolve(app, session_id, proc, &card); }
}

/// The prompt answered: the turn is over.
fn on_prompt_end(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, id: &str, result: Option<Value>, error: Option<Value>) {
    {
        let mut state = proc.acp.lock().unwrap();
        if state.retiring_prompts.remove(id) {
            let waiter = state.retire_waiter.clone();
            drop(state);
            waiter.pending.lock().unwrap().remove(id);
            waiter.cv.notify_all();
            return;
        }
    }
    let (canceled, started_at) = {
        let turn = proc.turn.lock().unwrap();
        (turn.interrupted, turn.started_at)
    };
    let mut cold_retry = None;
    let settled_while_parked = {
        let mut state = proc.acp.lock().unwrap();
        // A response can belong to the first prompt or to one sent into the running turn. Anything else is
        // stale, such as the answer to a prompt a later turn already replaced.
        let recognized = if state.prompt.as_deref() == Some(id) {
            state.prompt = None;
            true
        } else if let Some(index) = state.steers.iter().position(|steer| steer == id) {
            state.steers.remove(index);
            true
        } else {
            false
        };
        if !recognized { return; }
        let parked_flag = state.parked_idle;
        let was_settled_while_parked = state.settled_while_parked;
        state.parked_idle = false;
        // Antigravity can answer `session/resume` or `session/new` before `FetchAvailableModels`
        // finishes populating its in-memory model map, rejecting the first `session/prompt` with
        // `failed to construct executor: unknown model key MODEL_PLACEHOLDER_...` or `plan model
        // not specified`. Retry transparently while keeping the turn visibly active.
        if proc.kind == SessionKind::Antigravity
            && !canceled
            && !was_settled_while_parked
            && state.steers.is_empty()
            && state.requests.is_empty()
            && proc.alive.load(Ordering::Relaxed)
            && !proc.released.load(Ordering::Relaxed)
        {
            if let Some(err) = error.as_ref().filter(|err| wire::antigravity_model_not_ready(err)) {
                let delays = cold_model_retry_delays();
                if let Some(active) = state.active_prompt.as_mut() {
                    if !active.had_output && active.attempts < delays.len() {
                        let delay = delays[active.attempts];
                        active.attempts += 1;
                        let attempt = active.attempts;
                        let payload = (active.text.clone(), active.images.clone());
                        let wait = Arc::new(RetryWait::default());
                        if let Some(prev) = state.model_retry_wait.replace(wait.clone()) {
                            prev.wake();
                        }
                        state.retrying_prompt = true;
                        let _ = state.mapper.finish();
                        cold_retry = Some((
                            attempt,
                            delay,
                            payload.0,
                            payload.1,
                            wait,
                            wire::antigravity_plan_model_missing(err),
                            wire::error_text(err),
                        ));
                    }
                }
            }
        }
        if cold_retry.is_some() {
            false
        } else {
        // Antigravity answers the first prompt as soon as a steer is sent, while the turn keeps streaming.
        // Wait for every prompt that was sent into the turn before ending it. A cancellation or error is
        // terminal, so the remaining prompts are dropped instead of holding the turn open forever.
        if canceled || error.is_some() {
            state.prompt = None;
            state.steers.clear();
        } else if state.prompt.is_some() || !state.steers.is_empty() {
            if parked_flag {
                state.parked_idle = true;
            }
            return;
        }
        state.active_prompt = None;
        state.retrying_prompt = false;
        if let Some(wait) = state.model_retry_wait.take() {
            wait.wake();
        }
        state.settled_while_parked = false;
        let stop_park = state.stop_park.take();
        let rows: Vec<ChatRow> = state.mapper.finish().into_iter().chain(state.mapper.settle_running(canceled)).collect();
        if !was_settled_while_parked {
            if let (Some(started_at), true) = (started_at, state.journal.is_some()) {
                let now = now_ms();
                state.pending_journal.push(wire::journal_turn_end(now.saturating_sub(started_at), now as i64));
            }
        }
        flush_journal_locked(&mut state);
        drop(state);
        if let Some(park) = stop_park {
            park.release("allow");
        }
        let mut timeline = proc.timeline.lock().unwrap();
        for row in rows { timeline.upsert(row); }
        was_settled_while_parked
        }
    };
    if let Some((attempt, delay, text, images, wait, plan_model_missing, reason)) = cold_retry {
        crate::diagnostics::record("WARN", "agent_chat", json!({
            "sessionId": session_id,
            "event": "antigravity_cold_model_retry",
            "attempt": attempt,
            "delayMs": delay.as_millis() as u64,
            "planModelMissing": plan_model_missing,
            "reason": reason,
        }));
        spawn_cold_model_retry(app.clone(), session_id.to_string(), proc.clone(), text, images, delay, wait, plan_model_missing);
        return;
    }
    cancel_pending(app, session_id, proc);
    let failure = match &error {
        Some(error) if wire::auth_required(error) => {
            if proc.kind == SessionKind::Antigravity {
                super::auth::antigravity::require(app, session_id, proc);
            } else {
                emit(app, session_id, json!({"type":"error","message":format!("agent_sign_in_required:{}", proc.kind.as_str())}));
            }
            Some(wire::error_text(error))
        }
        // The turn never reached the model; the view explains where the key goes instead of repeating the
        // agent's internal message as a failure.
        Some(error) if wire::api_key_missing(error) => {
            emit(app, session_id, json!({"type":"error","message":format!("agent_api_key_required:{}", proc.kind.as_str())}));
            None
        }
        Some(error) => Some(wire::error_text(error)),
        None => {
            let reason = result.as_ref().and_then(|result| result.get("stopReason")).and_then(Value::as_str).unwrap_or("end_turn");
            wire::stop_reason_error(reason).map(str::to_string)
        }
    };
    if error.is_none() && failure.is_none() {
        super::auth::turn_succeeded(app, session_id, proc);
    }
    if let Some(message) = failure.as_deref().filter(|_| !canceled) {
        let turn = proc.acp.lock().unwrap().mapper.turn();
        proc.timeline.lock().unwrap().upsert(ChatRow::Error { id: format!("e-acp-{turn}"), message: message.to_string() });
    }
    if proc.kind == SessionKind::Antigravity {
        super::antigravity_tasks::before_turn_end(app, session_id, proc);
    }
    let needs_turn_end = !settled_while_parked
        || failure.is_some()
        || {
            let turn = proc.turn.lock().unwrap();
            turn.running || !turn.waiting.is_empty()
        };
    if needs_turn_end {
        handle_turn_end(app, session_id, proc, "success", None);
    } else {
        super::release_if_idle(app, session_id, proc);
    }
    if proc.kind == SessionKind::Grok {
        // Grok may not re-emit `session_info_update` for a resumed conversation, so also read the title
        // it persisted beside its recording and let a placeholder or adopted name follow it.
        crate::agent::server::sync_grok_title_from_disk(app, session_id);
    }
}

/// Retry an Antigravity prompt after a short backoff (or immediately once `config_option_update`
/// publishes the model list) without duplicating the user row or ending the turn.
#[allow(clippy::too_many_arguments)]
fn spawn_cold_model_retry(
    app: AppCtx,
    session_id: String,
    proc: Arc<ChatProcess>,
    text: String,
    images: Vec<ChatImage>,
    delay: Duration,
    wait: Arc<RetryWait>,
    plan_model_missing: bool,
) {
    std::thread::spawn(move || {
        wait.wait_for(delay);
        if !proc.alive.load(Ordering::Relaxed) || proc.released.load(Ordering::Relaxed) {
            return;
        }
        let still_retrying = {
            let state = proc.acp.lock().unwrap();
            state.retrying_prompt
                && state.model_retry_wait.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &wait))
        };
        if !still_retrying {
            return;
        }
        if plan_model_missing {
            try_sync_antigravity_plan_model(&app, &session_id, &proc);
        }
        let Some(native) = proc.agent_session_id.lock().unwrap().clone() else {
            return;
        };
        // Follow the established `turn -> acp` lock order used by `interrupt_locked`.
        let next_id = {
            let turn = proc.turn.lock().unwrap();
            if !turn.running || turn.interrupted || !proc.alive.load(Ordering::Relaxed) || proc.released.load(Ordering::Relaxed) {
                let mut state = proc.acp.lock().unwrap();
                if state.model_retry_wait.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &wait)) {
                    state.retrying_prompt = false;
                    state.model_retry_wait = None;
                    state.active_prompt = None;
                }
                return;
            }
            let mut state = proc.acp.lock().unwrap();
            if !state.retrying_prompt
                || !state.model_retry_wait.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &wait))
            {
                return;
            }
            let id = proc.request_id("prompt");
            state.retrying_prompt = false;
            state.model_retry_wait = None;
            state.prompt = Some(id.clone());
            id
        };
        if let Err(error) = proc.write(&wire::prompt(&next_id, &native, &text, &images)) {
            proc.pending.lock().unwrap().remove(&next_id);
            let turn_num = {
                let mut state = proc.acp.lock().unwrap();
                if state.prompt.as_deref() == Some(next_id.as_str()) {
                    state.prompt = None;
                }
                state.active_prompt = None;
                state.mapper.turn()
            };
            proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                id: format!("e-acp-{turn_num}"),
                message: error,
            });
            handle_turn_end(&app, &session_id, &proc, "success", None);
        }
    });
}

/// If a cold-started Antigravity session failed with `plan model not specified`, try pushing the
/// conversation's saved model over `session/set_config_option` once models may have loaded.
fn try_sync_antigravity_plan_model(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let desired_model = proc.model.lock().unwrap().clone();
    let Some(model) = desired_model else { return; };
    let Some(native) = proc.agent_session_id.lock().unwrap().clone() else { return; };
    let (option, already_ready) = {
        let state = proc.acp.lock().unwrap();
        let option = state.catalog.model_option.clone().unwrap_or_else(|| "model".to_string());
        let already_ready = state.catalog.model_switchable() && state.catalog.model.as_deref() == Some(model.as_str());
        (option, already_ready)
    };
    if already_ready {
        return;
    }
    let Ok(result) = proc.request_and_wait("acp_set_model", |id| {
        wire::set_config_option(id, &native, &option, &model)
    }) else {
        return;
    };
    let catalog = {
        let mut state = proc.acp.lock().unwrap();
        state.awaiting_initial_models = false;
        state.catalog.model = Some(model);
        if let Some(options) = result.get("configOptions").and_then(Value::as_array) {
            state.catalog.absorb_options(options);
        }
        state.catalog.clone()
    };
    crate::agent::antigravity_models::remember_acp(app, &catalog);
    if catalog.effort_switchable() {
        let desired_effort = proc.effort.lock().unwrap().clone();
        if let Some(eff) = desired_effort.filter(|eff| {
            catalog.effort.as_deref() != Some(eff.as_str())
                && catalog.efforts.iter().any(|c| &c.id == eff)
        }) {
            let _ = set_effort_request(proc, &eff, true);
        }
    }
    emit(app, session_id, json!({"type":"models"}));
    emit(app, session_id, json!({"type":"settingsChanged","model":proc.model.lock().unwrap().clone(),
        "effort":proc.effort.lock().unwrap().clone()}));
}

/// Release any parked Antigravity `Stop` hook immediately so the underlying process can finish or exit.
pub(super) fn release_parked_stop(proc: &ChatProcess, decision: &'static str) {
    let park = {
        let mut state = proc.acp.lock().unwrap();
        state.parked_idle = false;
        state.stop_park.take()
    };
    if let Some(park) = park {
        park.release(decision);
    }
}

pub(super) fn is_stop_parked(proc: &ChatProcess) -> bool {
    proc.acp.lock().unwrap().parked_idle
}

/// Retire a parked `Stop` hook cleanly before starting a new user turn so the previous turn finishes
/// and resets `agy`'s per-turn `executionNum` counter back to 0.
pub(super) fn retire_parked_stop_before_send(proc: &Arc<ChatProcess>) {
    let (park, waiter, prompt_id) = {
        let mut state = proc.acp.lock().unwrap();
        if !state.parked_idle {
            return;
        }
        let Some(prompt_id) = state.prompt.take() else {
            state.parked_idle = false;
            state.settled_while_parked = false;
            if let Some(park) = state.stop_park.take() {
                park.release("allow");
            }
            return;
        };
        state.parked_idle = false;
        state.settled_while_parked = false;
        state.retiring_prompts.insert(prompt_id.clone());
        let waiter = state.retire_waiter.clone();
        waiter.pending.lock().unwrap().insert(prompt_id.clone());
        let park = state.stop_park.take();
        (park, waiter, prompt_id)
    };
    if let Some(park) = park {
        park.release("allow");
    }
    let deadline = Instant::now() + RETIRE_WAIT_TIMEOUT;
    let mut pending = waiter.pending.lock().unwrap();
    while pending.contains(&prompt_id) && proc.alive.load(Ordering::Relaxed) {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        let (guard, timed_out) = waiter.cv.wait_timeout(pending, left).unwrap();
        pending = guard;
        if timed_out.timed_out() {
            break;
        }
    }
}

/// Handle `agy`'s `Stop` hook HTTP callback for a running ACP chat session.
///
/// Parking the main conversation's `NO_TOOL_CALL` stop inside an active `session/prompt` keeps the
/// ACP stdio pipe alive across timer/subagent/task wakeups so native `session/request_permission`
/// and `elicitation/create` remain available on those wakeups.
pub(super) fn handle_stop_hook(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, body: &str) -> String {
    if !proc.alive.load(Ordering::Relaxed) || proc.released.load(Ordering::Relaxed) {
        return r#"{"decision":"allow"}"#.to_string();
    }
    let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let conv_id = parsed.get("conversationId").and_then(Value::as_str);
    if let Some(cid) = conv_id {
        let main_id = proc.agent_session_id.lock().unwrap().clone();
        if main_id.as_deref().is_some_and(|main| main != cid) {
            return r#"{"decision":"allow"}"#.to_string();
        }
    }
    let termination_reason = parsed.get("terminationReason").and_then(Value::as_str);
    let execution_num = parsed.get("executionNum").and_then(Value::as_u64).unwrap_or(0);
    #[cfg(test)]
    {
        proc.acp.lock().unwrap().stop_execution_nums.push(execution_num);
    }
    if termination_reason.is_some_and(|reason| reason != "NO_TOOL_CALL")
        || execution_num >= MAX_SAFE_STOP_CONTINUATIONS
    {
        return r#"{"decision":"allow"}"#.to_string();
    }
    {
        let mut state = proc.acp.lock().unwrap();
        if state.tasks.system_dir.is_none() {
            if let Some(cid) = conv_id.filter(|id| crate::agent::antigravity::is_native_id(id)) {
                state.tasks.system_dir = crate::agent::antigravity::system_dir(cid);
            }
        }
    }
    super::antigravity_tasks::refresh(app, session_id, proc, false);
    {
        let turn = proc.turn.lock().unwrap();
        if !turn.waiting.is_empty() || turn.interrupted {
            return r#"{"decision":"allow"}"#.to_string();
        }
    }
    let (system_dir, surfaced, woken_deliveries, woken_schedules, can_park) = {
        let state = proc.acp.lock().unwrap();
        let active_prompt = state
            .prompt
            .as_ref()
            .is_some_and(|id| !state.retiring_prompts.contains(id));
        (
            state.tasks.system_dir.clone(),
            state.surfaced_unread.clone(),
            state.tasks.woken_deliveries(),
            state.woken_schedules.clone(),
            active_prompt,
        )
    };
    if !can_park {
        return r#"{"decision":"allow"}"#.to_string();
    }
    if let Some(dir) = system_dir.as_deref() {
        if let Some(msg) = super::antigravity_tasks::next_unread_message(dir, &surfaced, &woken_deliveries) {
            {
                let mut state = proc.acp.lock().unwrap();
                if !proc.alive.load(Ordering::Relaxed)
                    || proc.released.load(Ordering::Relaxed)
                    || state.prompt.as_ref().is_none_or(|id| state.retiring_prompts.contains(id))
                    || !state.surfaced_unread.insert(msg.file_name.clone())
                {
                    return r#"{"decision":"allow"}"#.to_string();
                }
                state.parked_idle = false;
                state.settled_while_parked = false;
                state.mapper.begin_turn();
            }
            super::antigravity_tasks::emit_unread_wake(app, session_id, proc, &msg);
            super::antigravity_tasks::ensure_poller(app, session_id, proc);
            return r#"{"decision":"continue"}"#.to_string();
        }
    }
    if let Some(due) = super::antigravity_tasks::next_due_schedule(proc, now_ms(), &woken_schedules) {
        {
            let mut state = proc.acp.lock().unwrap();
            if !proc.alive.load(Ordering::Relaxed)
                || proc.released.load(Ordering::Relaxed)
                || state.prompt.as_ref().is_none_or(|id| state.retiring_prompts.contains(id))
                || !state.woken_schedules.insert(due.wake_key.clone())
            {
                return r#"{"decision":"allow"}"#.to_string();
            }
            state.stop_park = None;
            state.parked_idle = false;
            state.settled_while_parked = false;
            state.mapper.begin_turn();
        }
        super::antigravity_tasks::emit_schedule_wake(app, session_id, proc, &due);
        super::antigravity_tasks::ensure_poller(app, session_id, proc);
        return r#"{"decision":"continue"}"#.to_string();
    }
    if proc.release_when_idle.load(Ordering::Relaxed)
        && !super::antigravity_tasks::has_pending_or_unread(proc, &surfaced, &woken_deliveries)
    {
        return r#"{"decision":"allow"}"#.to_string();
    }

    let (rows, prior_park, park) = {
        let mut state = proc.acp.lock().unwrap();
        if !proc.alive.load(Ordering::Relaxed)
            || proc.released.load(Ordering::Relaxed)
            || state.prompt.as_ref().is_none_or(|id| state.retiring_prompts.contains(id))
        {
            return r#"{"decision":"allow"}"#.to_string();
        }
        let rows = state.mapper.finish();
        flush_journal_locked(&mut state);
        let park = Arc::new(StopPark::default());
        let prior_park = state.stop_park.replace(park.clone());
        state.parked_idle = true;
        state.settled_while_parked = true;
        (rows, prior_park, park)
    };
    if let Some(prev) = prior_park {
        prev.release("allow");
    }
    let bg = {
        let mut turn = proc.turn.lock().unwrap();
        if !turn.waiting.is_empty() || turn.interrupted {
            drop(turn);
            let mut state = proc.acp.lock().unwrap();
            if state.stop_park.as_ref().is_some_and(|p| Arc::ptr_eq(p, &park)) {
                state.stop_park = None;
                state.parked_idle = false;
            }
            park.release("allow");
            return r#"{"decision":"allow"}"#.to_string();
        }
        let duration_ms = turn.started_at.take().map(|s| now_ms().saturating_sub(s));
        turn.running = false;
        let bg = super::background_pending(&turn);
        drop(turn);
        let mut timeline = proc.timeline.lock().unwrap();
        if let Some(row) = rows {
            timeline.upsert(row);
        }
        if let Some(ms) = duration_ms {
            timeline.finish_latest_turn(ms);
        }
        bg
    };
    emit(app, session_id, json!({"type":"turnCompleted"}));
    emit_state(app, session_id, if bg { AgentState::Background } else { AgentState::Waiting });
    if bg {
        super::settle_background_state(app, session_id, proc);
    }

    loop {
        {
            let mut guard = park.decision.lock().unwrap();
            if guard.is_none() {
                let (next_guard, _) = park.cv.wait_timeout(guard, STOP_POLL_INTERVAL).unwrap();
                guard = next_guard;
            }
            if let Some(decision) = *guard {
                let mut state = proc.acp.lock().unwrap();
                if state.stop_park.as_ref().is_some_and(|p| Arc::ptr_eq(p, &park)) {
                    state.stop_park = None;
                    state.parked_idle = false;
                }
                return format!(r#"{{"decision":"{decision}"}}"#);
            }
        }
        if !proc.alive.load(Ordering::Relaxed)
            || proc.released.load(Ordering::Relaxed)
            || !proc.turn.lock().unwrap().waiting.is_empty()
        {
            let mut state = proc.acp.lock().unwrap();
            if state.stop_park.as_ref().is_some_and(|p| Arc::ptr_eq(p, &park)) {
                state.stop_park = None;
                state.parked_idle = false;
            }
            return r#"{"decision":"allow"}"#.to_string();
        }
        let (dir, surfaced, woken_deliveries, woken_schedules) = {
            let state = proc.acp.lock().unwrap();
            if !state.stop_park.as_ref().is_some_and(|p| Arc::ptr_eq(p, &park)) {
                return r#"{"decision":"allow"}"#.to_string();
            }
            (
                state.tasks.system_dir.clone(),
                state.surfaced_unread.clone(),
                state.tasks.woken_deliveries(),
                state.woken_schedules.clone(),
            )
        };
        if let Some(dir) = dir.as_deref() {
            if let Some(msg) = super::antigravity_tasks::next_unread_message(dir, &surfaced, &woken_deliveries) {
                {
                    let mut state = proc.acp.lock().unwrap();
                    if !state.stop_park.as_ref().is_some_and(|p| Arc::ptr_eq(p, &park)) {
                        return r#"{"decision":"allow"}"#.to_string();
                    }
                    if !state.surfaced_unread.insert(msg.file_name.clone()) {
                        continue;
                    }
                    state.stop_park = None;
                    state.parked_idle = false;
                    state.settled_while_parked = false;
                    state.mapper.begin_turn();
                }
                super::antigravity_tasks::emit_unread_wake(app, session_id, proc, &msg);
                super::antigravity_tasks::ensure_poller(app, session_id, proc);
                return r#"{"decision":"continue"}"#.to_string();
            }
        }
        if let Some(due) = super::antigravity_tasks::next_due_schedule(proc, now_ms(), &woken_schedules) {
            {
                let mut state = proc.acp.lock().unwrap();
                if !state.stop_park.as_ref().is_some_and(|p| Arc::ptr_eq(p, &park)) {
                    return r#"{"decision":"allow"}"#.to_string();
                }
                if !state.woken_schedules.insert(due.wake_key.clone()) {
                    continue;
                }
                state.stop_park = None;
                state.parked_idle = false;
                state.settled_while_parked = false;
                state.mapper.begin_turn();
            }
            super::antigravity_tasks::emit_schedule_wake(app, session_id, proc, &due);
            super::antigravity_tasks::ensure_poller(app, session_id, proc);
            return r#"{"decision":"continue"}"#.to_string();
        }
        if proc.release_when_idle.load(Ordering::Relaxed)
            && !super::antigravity_tasks::has_pending_or_unread(proc, &surfaced, &woken_deliveries)
        {
            let mut state = proc.acp.lock().unwrap();
            if state.stop_park.as_ref().is_some_and(|p| Arc::ptr_eq(p, &park)) {
                state.stop_park = None;
                state.parked_idle = false;
            }
            return r#"{"decision":"allow"}"#.to_string();
        }
    }
}

fn flush_journal_locked(state: &mut AcpState) {
    let Some(path) = state.journal.clone() else { state.pending_journal.clear(); return };
    let lines = std::mem::take(&mut state.pending_journal);
    if let Err(error) = wire::append_journal(&path, &lines) {
        crate::diagnostic_warn!("chat: ACP journal append failed: {error}");
    }
}

pub(super) fn has_active_prompt(proc: &ChatProcess) -> bool {
    proc.acp.lock().unwrap().prompt_active()
}

/// The process is gone: close whatever was streaming and keep the journal complete.
pub(super) fn finish(proc: &Arc<ChatProcess>) {
    let (rows, reactive_rows, park, retry_wait, waiter) = {
        let mut state = proc.acp.lock().unwrap();
        state.parked_idle = false;
        state.settled_while_parked = false;
        state.retrying_prompt = false;
        state.active_prompt = None;
        state.startup_native = None;
        let park = state.stop_park.take();
        let retry_wait = state.model_retry_wait.take();
        let waiter = state.retire_waiter.clone();
        let rows: Vec<ChatRow> = state.mapper.finish().into_iter().chain(state.mapper.settle_running(true)).collect();
        let reactive_rows = state.tasks.take_aborted_reactive_rows();
        flush_journal_locked(&mut state);
        (rows, reactive_rows, park, retry_wait, waiter)
    };
    if let Some(park) = park {
        park.release("allow");
    }
    if let Some(wait) = retry_wait {
        wait.wake();
    }
    waiter.pending.lock().unwrap().clear();
    waiter.cv.notify_all();
    {
        let mut timeline = proc.timeline.lock().unwrap();
        for row in rows { timeline.upsert(row); }
    }
    super::antigravity_tasks::settle_rows(proc, &reactive_rows);
}

// ─────────────────────────── Outgoing ───────────────────────────

pub(super) fn validate(proc: &ChatProcess, behavior: &str, image_count: usize) -> Result<(), String> {
    let state = proc.acp.lock().unwrap();
    // The handshake has not answered yet: the message waits in the queue, and `dispatch` checks again
    // before it is written. Refusing here would lose the first image of a conversation already starting.
    let images = !state.images_known || state.images;
    drop(state);
    wire::validate(behavior, image_count, images, proc.kind == SessionKind::Antigravity)
}

pub(super) fn dispatch(proc: &Arc<ChatProcess>, text: &str, images: &[ChatImage]) -> Result<(), String> {
    let session = proc.agent_session_id.lock().unwrap().clone().ok_or("The agent has not opened its conversation yet")?;
    let id = proc.request_id("prompt");
    let (reactive_rows, prev_retry_wait) = {
        let mut state = proc.acp.lock().unwrap();
        let reactive_rows = state.tasks.take_aborted_reactive_rows();
        state.parked_idle = false;
        state.settled_while_parked = false;
        state.retrying_prompt = false;
        let prev_retry_wait = state.model_retry_wait.take();
        state.active_prompt = (proc.kind == SessionKind::Antigravity).then(|| ActivePrompt {
            text: text.to_string(),
            images: images.to_vec(),
            attempts: 0,
            had_output: false,
        });
        state.mapper.begin_turn();
        state.prompt = Some(id.clone());
        state.steers.clear();
        if state.journal.is_some() {
            state.pending_journal.push(wire::journal_prompt(text, now_ms() as i64));
            flush_journal_locked(&mut state);
        }
        (reactive_rows, prev_retry_wait)
    };
    if let Some(wait) = prev_retry_wait {
        wait.wake();
    }
    super::antigravity_tasks::settle_rows(proc, &reactive_rows);
    let result = proc.write(&wire::prompt(&id, &session, text, images));
    if result.is_err() {
        proc.pending.lock().unwrap().remove(&id);
        let mut state = proc.acp.lock().unwrap();
        state.prompt = None;
        state.active_prompt = None;
    }
    result
}

/// Send a prompt into the running turn.
///
/// Antigravity accepts another `session/prompt` while one runs and delivers it at the next model
/// invocation. Its answer to the first prompt arrives early, before the turn is actually over, so
/// `on_prompt_end` keeps the turn open until every prompt has answered. The current assistant block is
/// closed here so the steer message becomes a boundary in the transcript.
pub(super) fn steer(proc: &Arc<ChatProcess>, text: &str, images: &[ChatImage]) -> Result<(), String> {
    let session = proc.agent_session_id.lock().unwrap().clone().ok_or("The agent has not opened its conversation yet")?;
    let (id, rows, reactive_rows) = {
        let mut state = proc.acp.lock().unwrap();
        if state.retrying_prompt {
            return Err("Antigravity is still loading models; queue the message instead".into());
        }
        // The engine checks `turn.running` before choosing this path; a missing prompt here means the
        // agent already answered the turn (unless Antigravity is running a reactive wakeup turn).
        let reactive_rows = if state.prompt.is_none() && state.steers.is_empty() {
            if proc.kind == SessionKind::Antigravity && state.tasks.reactive_active() {
                state.mapper.begin_turn();
                state.tasks.take_aborted_reactive_rows()
            } else {
                return Err("The agent has no running turn to steer".into());
            }
        } else {
            Vec::new()
        };
        let id = proc.request_id("prompt");
        let rows: Vec<ChatRow> = state.mapper.finish().into_iter().collect();
        state.active_prompt = None;
        state.steers.push(id.clone());
        if state.journal.is_some() {
            state.pending_journal.push(wire::journal_prompt(text, now_ms() as i64));
            flush_journal_locked(&mut state);
        }
        (id, rows, reactive_rows)
    };
    super::antigravity_tasks::settle_rows(proc, &reactive_rows);
    for row in rows { proc.timeline.lock().unwrap().upsert(row); }
    let result = proc.write(&wire::prompt(&id, &session, text, images));
    if result.is_err() {
        proc.pending.lock().unwrap().remove(&id);
        proc.acp.lock().unwrap().steers.retain(|steer| steer != &id);
    }
    result
}

/// Cancel the running prompt. Waiting requests are answered at once, because an agent may hold the
/// prompt open until they are; their cards are cleared when the prompt answers.
pub(super) fn cancel(proc: &Arc<ChatProcess>) -> Result<(), String> {
    let session = proc.agent_session_id.lock().unwrap().clone().ok_or("The agent has not opened its conversation yet")?;
    release_parked_stop(proc, "allow");
    let retry_wait = {
        let mut state = proc.acp.lock().unwrap();
        state.retrying_prompt = false;
        state.active_prompt = None;
        state.model_retry_wait.take()
    };
    if let Some(wait) = retry_wait {
        wait.wake();
    }
    proc.write(&wire::cancel(&session))?;
    let drained: Vec<PendingRequest> = proc.acp.lock().unwrap().requests.drain().map(|(_, entry)| entry).collect();
    for entry in drained {
        match entry {
            PendingRequest::Permission { id, .. } => {
                let _ = proc.write(&wire::permission_outcome(&id, None));
            }
            PendingRequest::Elicitation { id, params, card_payload } => {
                let _ = proc.write(&wire::elicitation_outcome(&id, &card_payload, &params, false, None, Some("cancel")));
            }
            PendingRequest::GrokDialog { id, method, .. } => {
                let _ = proc.write(&wire::grok_cancelled_outcome(&id, &method));
            }
            PendingRequest::DetachedRecovery { row_id, .. } => {
                super::antigravity_tasks::settle_rows(proc, std::slice::from_ref(&row_id));
            }
        }
    }
    Ok(())
}

fn session(proc: &ChatProcess) -> Result<String, String> {
    proc.agent_session_id.lock().unwrap().clone().ok_or_else(|| "The agent has not opened its conversation yet".into())
}

/// Switch the session mode. `wait` blocks for the answer; the handshake sends without waiting because the
/// reader thread that would deliver the answer is the one running it.
fn set_mode_request(proc: &Arc<ChatProcess>, mode: &str, wait: bool) -> Result<(), String> {
    let session = session(proc)?;
    let option = {
        let mut state = proc.acp.lock().unwrap();
        state.pending_mode = Some(mode.to_string());
        state.catalog.mode_option.clone()
    };
    let make = |id: &str| match &option {
        Some(option) => wire::set_config_option(id, &session, option, mode),
        None => wire::set_mode(id, &session, mode),
    };
    let result = match send(proc, "acp_set_mode", wait, make) {
        Ok(result) => result,
        Err(err) => {
            proc.acp.lock().unwrap().pending_mode = None;
            return Err(err);
        }
    };
    if wait {
        let mut state = proc.acp.lock().unwrap();
        if let Some(options) = result.as_ref().and_then(|res| res.get("configOptions")).and_then(Value::as_array) {
            state.catalog.absorb_options(options);
        }
        state.pending_mode = None;
        state.catalog.mode = Some(mode.to_string());
    }
    Ok(())
}

fn set_model_request(proc: &Arc<ChatProcess>, model: &str, wait: bool) -> Result<(), String> {
    let session = session(proc)?;
    let catalog = proc.acp.lock().unwrap().catalog.clone();
    let make = |id: &str| match (&catalog.model_option, catalog.models_from_state) {
        (Some(option), false) => wire::set_config_option(id, &session, option, model),
        _ => wire::set_model(id, &session, model),
    };
    let result = send(proc, "acp_set_model", wait, make)?;
    if wait {
        let effort = {
            let mut state = proc.acp.lock().unwrap();
            let pending_mode = state.pending_mode.clone();
            state.catalog.model = Some(model.to_string());
            if let Some(options) = result.as_ref().and_then(|res| res.get("configOptions")).and_then(Value::as_array) {
                state.catalog.absorb_options(options);
                if let Some(expected) = pending_mode {
                    state.catalog.mode = Some(expected);
                }
            }
            if state.catalog.effort_switchable() { state.catalog.effort.clone() } else { None }
        };
        *proc.effort.lock().unwrap() = effort;
    }
    *proc.model.lock().unwrap() = Some(model.to_string());
    Ok(())
}

fn set_effort_request(proc: &Arc<ChatProcess>, effort: &str, wait: bool) -> Result<(), String> {
    let session = session(proc)?;
    let option = proc.acp.lock().unwrap().catalog.effort_option.clone().ok_or("This agent has no reasoning setting")?;
    send(proc, "acp_set_effort", wait, |id| wire::set_config_option(id, &session, &option, effort))?;
    proc.acp.lock().unwrap().catalog.effort = Some(effort.to_string());
    *proc.effort.lock().unwrap() = Some(effort.to_string());
    Ok(())
}

fn send(proc: &Arc<ChatProcess>, kind: &'static str, wait: bool, make: impl FnOnce(&str) -> Value) -> Result<Option<Value>, String> {
    if wait {
        proc.request_and_wait(kind, make).map(Some)
    } else {
        let id = proc.request_id(kind);
        proc.write(&make(&id)).map(|()| None)
    }
}

pub(super) fn set_mode(proc: &Arc<ChatProcess>, mode: &str) -> Result<(), String> {
    if !proc.acp.lock().unwrap().catalog.modes.iter().any(|choice| choice.id == mode) {
        return Err(format!("{} has no mode named {mode}", wire::label(proc.kind)));
    }
    set_mode_request(proc, mode, true).map(|_| ())
}

/// Sync an Antigravity permission change onto `agy acp`, falling back cleanly on Bypass when `auto` is omitted.
pub(super) fn sync_antigravity_permission(proc: &Arc<ChatProcess>, permission_mode: &str) -> Result<(), String> {
    let acp_mode = acp_mode_for_permission(permission_mode);
    if permission_mode == "bypassPermissions" {
        let (modes, current) = {
            let state = proc.acp.lock().unwrap();
            (
                state.catalog.modes.clone(),
                state.pending_mode.clone().or_else(|| state.catalog.mode.clone()),
            )
        };
        let chosen = antigravity_bypass_acp_mode(&modes);
        if modes.iter().any(|choice| choice.id == chosen) {
            if current.as_deref() != Some(chosen) {
                set_mode_request(proc, chosen, true)?;
            }
            *proc.collaboration_mode.lock().unwrap() = Some(chosen.to_string());
        } else {
            *proc.collaboration_mode.lock().unwrap() = Some(acp_mode.to_string());
        }
        return Ok(());
    }
    set_mode(proc, acp_mode)?;
    *proc.collaboration_mode.lock().unwrap() = Some(acp_mode.to_string());
    Ok(())
}

pub(super) fn set_model(proc: &Arc<ChatProcess>, model: &str) -> Result<(), String> {
    if !proc.acp.lock().unwrap().catalog.model_switchable() {
        return Err(format!("{} cannot change its model during a conversation", wire::label(proc.kind)));
    }
    set_model_request(proc, model, true)
}

pub(super) fn set_effort(proc: &Arc<ChatProcess>, effort: &str) -> Result<(), String> {
    set_effort_request(proc, effort, true)
}

/// The model choices a running session offers, in the shape the composer's model picker reads.
pub(super) fn chat_models(app: &AppCtx, proc: &Arc<ChatProcess>) -> Option<Vec<Value>> {
    let catalog = proc.acp.lock().unwrap().catalog.clone();
    if !catalog.model_switchable() { return None; }
    if proc.kind == SessionKind::Antigravity {
        let live = crate::agent::antigravity_models::models_from_acp(app, &catalog);
        return Some(
            crate::agent::antigravity_models::merge_live_with_cached(app, live)
                .into_iter()
                .filter_map(|model| serde_json::to_value(model).ok())
                .collect(),
        );
    }
    Some(catalog_models(&catalog))
}

fn catalog_models(catalog: &Catalog) -> Vec<Value> {
    let shared: Vec<&str> = catalog.efforts.iter().map(|choice| choice.id.as_str()).collect();
    catalog.models.iter().map(|model| {
        // Prefer the ladder the model stated for itself; fall back to the session-wide one.
        let efforts: Vec<&str> = match catalog.model_efforts.get(&model.id) {
            Some(levels) => levels.iter().map(String::as_str).collect(),
            None => shared.clone(),
        };
        json!({
            "id": model.id, "label": model.name, "description": model.description.clone().unwrap_or_default(),
            "effortLevels": efforts,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    use super::super::tests::{ctx, inert_process};
    use super::*;

    /// An ACP process whose writes can be read back, standing in for the agent's stdin.
    fn wired(kind: SessionKind) -> (Arc<ChatProcess>, BufReader<std::process::ChildStdout>) {
        let proc = inert_process(kind);
        let mut child = Command::new("cat").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        *proc.stdin.lock().unwrap() = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        *proc.child.lock().unwrap() = child;
        (proc, stdout)
    }

    fn written(reader: &mut BufReader<std::process::ChildStdout>) -> Value {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn reply(app: &AppCtx, proc: &Arc<ChatProcess>, request: &Value, result: Value) {
        handle_line(app, "s", proc, &json!({"jsonrpc":"2.0","id":request["id"],"result":result}).to_string());
    }

    fn notify(app: &AppCtx, proc: &Arc<ChatProcess>, update: Value) {
        handle_line(app, "s", proc, &json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":update}}).to_string());
    }

    #[test]
    fn catalog_models_report_each_models_own_ladder() {
        let mut catalog = Catalog::default();
        catalog.models = vec![
            wire::Choice { id: "a".into(), name: "A".into(), description: None },
            wire::Choice { id: "b".into(), name: "B".into(), description: None },
        ];
        catalog.efforts = vec![wire::Choice { id: "low".into(), name: "low".into(), description: None }];
        catalog.model_efforts.insert("a".into(), vec!["none".into(), "high".into()]);
        let models = catalog_models(&catalog);
        assert_eq!(models[0]["effortLevels"], json!(["none", "high"]));
        assert_eq!(models[1]["effortLevels"], json!(["low"]));
    }

    /// A conversation whose native recording is gone opens a fresh one, and says so; anything else is
    /// still reported as the failure it is instead of silently replacing the conversation.
    #[test]
    fn a_missing_native_conversation_opens_a_fresh_one_and_other_failures_still_fail() {
        let app = ctx("acp-load-missing");
        let (proc, mut agent) = wired(SessionKind::Grok);
        bootstrap(&app, &proc, Some("native-lost"), Some("/w"), None);
        let initialize = written(&mut agent);
        reply(&app, &proc, &initialize, json!({"agentCapabilities":{"loadSession":true}}));
        let load = written(&mut agent);
        assert_eq!(load["method"], "session/load");
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":load["id"],
            "error":{"code":-32603,"message":"Path not found.","data":{"code":"FS_NOT_FOUND"}}}).to_string());
        let fresh = written(&mut agent);
        assert_eq!(fresh["method"], "session/new");
        assert_eq!(fresh["params"]["cwd"], "/w");
        assert!(proc.ready.load(Ordering::Relaxed) || !proc.released.load(Ordering::Relaxed));

        // A protocol error is not a lost conversation: the start fails as before.
        let (other, mut other_agent) = wired(SessionKind::Grok);
        bootstrap(&app, &other, Some("native-broken"), Some("/w"), None);
        let initialize = written(&mut other_agent);
        reply(&app, &other, &initialize, json!({"agentCapabilities":{"loadSession":true}}));
        let load = written(&mut other_agent);
        handle_line(&app, "s", &other, &json!({"jsonrpc":"2.0","id":load["id"],
            "error":{"code":-32602,"message":"Invalid params"}}).to_string());
        assert!(other.released.load(Ordering::Relaxed));
    }

    #[test]
    fn a_resumed_conversation_is_replaced_by_its_replay_and_keeps_recorded_times() {
        let app = ctx("acp-resume");
        let (proc, mut agent) = wired(SessionKind::Copilot);
        // The recording drew a thought the replay leaves out, so ids differ and only the text matches.
        proc.timeline.lock().unwrap().replace_all(vec![
            ChatRow::User { id: "acp-u1".into(), text: "hi".into(), images: vec![ChatImage { mime_type: "image/png".into(), data: "UE5H".into() }], at: Some(1_000) },
            ChatRow::Reasoning { id: "acp-r2".into(), text: "thinking".into(), streaming: false },
            ChatRow::Assistant { id: "acp-a3".into(), text: "hello".into(), streaming: false, model: None, at: Some(2_000), duration_ms: Some(500) },
        ]);
        bootstrap(&app, &proc, Some("s1"), Some("/w"), Some("plan"));
        let initialize = written(&mut agent);
        assert_eq!(initialize["method"], "initialize");
        assert_eq!(initialize["params"]["clientCapabilities"]["terminal"], false);
        reply(&app, &proc, &initialize, json!({"agentCapabilities":{"loadSession":true,"promptCapabilities":{"image":true}}}));
        let load = written(&mut agent);
        assert_eq!((load["method"].as_str(), load["params"]["sessionId"].as_str(), load["params"]["cwd"].as_str()),
            (Some("session/load"), Some("s1"), Some("/w")));
        notify(&app, &proc, json!({"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"hi"}}));
        notify(&app, &proc, json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hello"}}));
        notify(&app, &proc, json!({"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"from the terminal"}}));
        // Nothing replayed reaches the screen until the replay is complete.
        assert!(matches!(&proc.timeline.lock().unwrap().rows[1], ChatRow::Reasoning { .. }));
        reply(&app, &proc, &load, json!({"modes":{"availableModes":[{"id":"agent","name":"Agent"},{"id":"plan","name":"Plan"}],"currentModeId":"agent"}}));
        let mode = written(&mut agent);
        assert_eq!((mode["method"].as_str(), mode["params"]["modeId"].as_str()), (Some("session/set_mode"), Some("plan")));
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(), 3);
        assert!(matches!(&rows[0], ChatRow::User { text, images, at: Some(1_000), .. }
            if text == "hi" && images.len() == 1 && images[0].data == "UE5H"));
        assert!(matches!(&rows[1], ChatRow::Assistant { text, at: Some(2_000), streaming: false, duration_ms: Some(500), .. } if text == "hello"));
        assert!(matches!(&rows[2], ChatRow::User { text, at: None, .. } if text == "from the terminal"));
        assert!(proc.ready.load(Ordering::Relaxed));
        assert!(proc.acp.lock().unwrap().images);
        assert_eq!(proc.collaboration_mode.lock().unwrap().as_deref(), Some("plan"));
        assert_eq!(proc.collaboration_modes.lock().unwrap().len(), 2);
        assert_eq!(proc.agent_session_id.lock().unwrap().as_deref(), Some("s1"));
    }

    #[test]
    fn a_rewritten_journal_puts_each_recorded_duration_back_before_the_next_turn() {
        let lines = vec![("u1".to_string(), Some("acp-u1".to_string())), ("a2".into(), Some("acp-a2".into())),
            ("a2b".into(), Some("acp-a2".into())), ("u3".into(), Some("acp-u3".into())), ("a4".into(), Some("acp-a4".into()))];
        let durations = HashMap::from([("acp-a2".to_string(), 500u64), ("acp-a4".to_string(), 700u64)]);
        let out = journal_with_turn_ends(&lines, &durations);
        assert_eq!(out.len(), 7);
        assert_eq!((out[2].as_str(), out[4].as_str()), ("a2b", "u3"));
        assert!(out[3].contains("\"durationMs\":500") && out[6].contains("\"durationMs\":700"));
    }

    #[test]
    fn a_live_turn_streams_asks_for_permission_and_ends_with_the_prompt_answer() {
        let app = ctx("acp-turn");
        let (proc, mut agent) = wired(SessionKind::Kimi);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "run the tests", &[]).unwrap();
        let prompt = written(&mut agent);
        assert_eq!(prompt["params"]["prompt"], json!([{"type":"text","text":"run the tests"}]));
        notify(&app, &proc, json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Running"}}));
        notify(&app, &proc, json!({"sessionUpdate":"tool_call","toolCallId":"c1","title":"cargo test","kind":"execute","status":"pending","rawInput":{"command":"cargo test"}}));
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":7,"method":"session/request_permission","params":{"sessionId":"s1",
            "toolCall":{"toolCallId":"c1","title":"cargo test","kind":"execute","rawInput":{"command":"cargo test"}},
            "options":[{"optionId":"once","name":"Allow","kind":"allow_once"},{"optionId":"always","name":"Always","kind":"allow_always"},
                {"optionId":"no","name":"Reject","kind":"reject_once"}]}}).to_string());
        let card = proc.permissions.lock().unwrap().get("acp-7").cloned().expect("the request is a permission card");
        assert_eq!(card["tool_name"], "Bash");
        respond(&app, "s", &proc, "acp-7", true, None, None, Some(&json!([{"type":"acpAllowAlways","optionId":"always"}]))).unwrap();
        let outcome = written(&mut agent);
        assert_eq!(outcome, json!({"jsonrpc":"2.0","id":7,"result":{"outcome":{"outcome":"selected","optionId":"always"}}}));
        assert!(proc.permissions.lock().unwrap().is_empty());
        notify(&app, &proc, json!({"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"completed",
            "content":[{"type":"content","content":{"type":"text","text":"ok"}}]}));
        notify(&app, &proc, json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"All passed"}}));
        reply(&app, &proc, &prompt, json!({"stopReason":"end_turn"}));
        assert!(!proc.turn.lock().unwrap().running);
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert!(rows.iter().any(|row| matches!(row, ChatRow::Tool { name, output: Some(output), status: "completed", .. } if name == "Bash" && output == "ok")));
        assert!(rows.iter().any(|row| matches!(row, ChatRow::Assistant { text, streaming: false, .. } if text == "All passed")));
        assert!(!rows.iter().any(|row| matches!(row, ChatRow::Error { .. })));
    }

    #[test]
    fn a_steer_keeps_the_turn_open_until_its_own_prompt_answers() {
        let app = ctx("acp-steer");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "start", &[]).unwrap();
        let first = written(&mut agent);
        steer(&proc, "next", &[]).unwrap();
        let second = written(&mut agent);
        assert_eq!(first["params"]["prompt"], json!([{"type":"text","text":"start"}]));
        assert_eq!(second["params"]["prompt"], json!([{"type":"text","text":"next"}]));
        // Antigravity answers the first prompt as soon as the steer goes out; that is not the end.
        reply(&app, &proc, &first, json!({"stopReason":"end_turn"}));
        assert!(proc.turn.lock().unwrap().running);
        assert_eq!(proc.acp.lock().unwrap().steers.len(), 1);
        reply(&app, &proc, &second, json!({"stopReason":"end_turn"}));
        assert!(!proc.turn.lock().unwrap().running);
        assert!(proc.acp.lock().unwrap().prompt.is_none());
        assert!(proc.acp.lock().unwrap().steers.is_empty());
    }

    /// Grok asks for plan approval through its own request; the card is VelaTerm's plan card, and the
    /// answer has to come back in Grok's shape rather than as an ACP permission outcome.
    #[test]
    fn grok_plan_approval_is_a_plan_card_answered_with_its_outcome() {
        let app = ctx("acp-grok-plan");
        let (proc, mut agent) = wired(SessionKind::Grok);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        *proc.mode.lock().unwrap() = "bypassPermissions".into();
        let ask = |id: i64| json!({"jsonrpc":"2.0","id":id,"method":"_x.ai/exit_plan_mode",
            "params":{"sessionId":"s1","toolCallId":"t1","planContent":"1. Do it"}}).to_string();
        handle_line(&app, "s", &proc, &ask(0));
        let card = proc.permissions.lock().unwrap().get("acp-grok-0").cloned().expect("the plan waits for the person");
        assert_eq!((card["tool_name"].as_str(), card["input"]["plan"].as_str()), (Some("ExitPlanMode"), Some("1. Do it")));
        // Bypass approves tool permissions, never a plan: the person still has to say yes.
        approve_pending(&app, "s", &proc);
        assert!(proc.permissions.lock().unwrap().contains_key("acp-grok-0"));
        respond(&app, "s", &proc, "acp-grok-0", true, None, None, None).unwrap();
        assert_eq!(written(&mut agent), json!({"jsonrpc":"2.0","id":0,"result":{"outcome":"approved"}}));
        handle_line(&app, "s", &proc, &ask(1));
        respond(&app, "s", &proc, "acp-grok-1", false, None, None, None).unwrap();
        assert_eq!(written(&mut agent), json!({"jsonrpc":"2.0","id":1,"result":{"outcome":"rejected"}}));
        assert!(proc.permissions.lock().unwrap().is_empty());
    }

    /// Questions and MCP forms arrive as Grok's own requests and are answered in its three shapes.
    #[test]
    fn grok_questions_and_mcp_forms_are_cards_answered_in_grok_shape() {
        let app = ctx("acp-grok-dialogs");
        let (proc, mut agent) = wired(SessionKind::Grok);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":2,"method":"_x.ai/ask_user_question","params":{"sessionId":"s1",
            "toolCallId":"t1","questions":[{"question":"Pick a color?","options":[{"label":"Red"},{"label":"Blue"}],"multiSelect":null}],
            "mode":"default"}}).to_string());
        let card = proc.permissions.lock().unwrap().get("acp-grok-2").cloned().expect("the question waits for the person");
        assert_eq!((card["tool_name"].as_str(), card["input"]["questions"][0]["header"].as_str()),
            (Some("AskUserQuestion"), Some("Q1")));
        let answered = json!({"questions":card["input"]["questions"],"answers":{"Pick a color?":"Red"}});
        respond(&app, "s", &proc, "acp-grok-2", true, Some(&answered), None, None).unwrap();
        assert_eq!(written(&mut agent), json!({"jsonrpc":"2.0","id":2,
            "result":{"outcome":"accepted","answers":{"Pick a color?":"Red"}}}));
        let elicit = |id: i64| json!({"jsonrpc":"2.0","id":id,"method":"_x.ai/mcp/elicit","params":{"sessionId":"s1","toolCallId":"m1",
            "serverName":"srv","message":"Name?","mode":"form","requestedSchema":{"type":"object","properties":{"name":{"type":"string"}}}}}).to_string();
        handle_line(&app, "s", &proc, &elicit(3));
        let card = proc.permissions.lock().unwrap().get("acp-grok-3").cloned().expect("the form waits for the person");
        assert_eq!((card["subtype"].as_str(), card["mcp_server_name"].as_str()), (Some("elicitation"), Some("srv")));
        respond(&app, "s", &proc, "acp-grok-3", true, Some(&json!({"name":"Kai"})), None, None).unwrap();
        assert_eq!(written(&mut agent), json!({"jsonrpc":"2.0","id":3,"result":{"outcome":"accept","content":{"name":"Kai"}}}));
        handle_line(&app, "s", &proc, &elicit(4));
        respond(&app, "s", &proc, "acp-grok-4", false, None, Some("cancel"), None).unwrap();
        assert_eq!(written(&mut agent), json!({"jsonrpc":"2.0","id":4,"result":{"outcome":"cancel"}}));
        // A dismissed question skips the interview rather than failing the turn.
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":5,"method":"_x.ai/ask_user_question","params":{"sessionId":"s1",
            "questions":[{"question":"Pick a color?","options":[]}]}}).to_string());
        respond(&app, "s", &proc, "acp-grok-5", false, None, None, None).unwrap();
        assert_eq!(written(&mut agent), json!({"jsonrpc":"2.0","id":5,"result":{"outcome":"skip_interview"}}));
        // A turn that ends with a form still open answers it as cancelled, so the agent is never left waiting.
        handle_line(&app, "s", &proc, &elicit(6));
        cancel_pending(&app, "s", &proc);
        assert_eq!(written(&mut agent), json!({"jsonrpc":"2.0","id":6,"result":{"outcome":"cancel"}}));
        assert!(proc.permissions.lock().unwrap().is_empty());
    }

    #[test]
    fn a_cancelled_turn_answers_its_questions_and_reports_no_error() {
        let app = ctx("acp-cancel");
        let (proc, mut agent) = wired(SessionKind::Grok);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "long task", &[]).unwrap();
        let prompt = written(&mut agent);
        notify(&app, &proc, json!({"sessionUpdate":"tool_call","toolCallId":"c1","title":"sleep","kind":"execute","status":"in_progress","rawInput":{"command":"sleep 99"}}));
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":"p1","method":"session/request_permission","params":{"sessionId":"s1",
            "toolCall":{"toolCallId":"c2","kind":"edit"},"options":[{"optionId":"y","kind":"allow_once"}]}}).to_string());
        {
            let mut turn = proc.turn.lock().unwrap();
            super::super::interrupt_locked(&app, &proc, &mut turn).unwrap();
        }
        assert_eq!(written(&mut agent)["method"], "session/cancel");
        assert_eq!(written(&mut agent)["result"]["outcome"]["outcome"], "cancelled");
        reply(&app, &proc, &prompt, json!({"stopReason":"cancelled"}));
        assert!(proc.permissions.lock().unwrap().is_empty());
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert!(rows.iter().any(|row| matches!(row, ChatRow::Tool { status: "canceled", .. })));
        assert!(!rows.iter().any(|row| matches!(row, ChatRow::Error { .. })));
    }

    #[test]
    fn a_sign_in_refusal_becomes_the_stable_code_and_releases_the_process() {
        let app = ctx("acp-auth");
        let (proc, mut agent) = wired(SessionKind::Cursor);
        // The fixture starts ready; a real process is not until its session opens.
        proc.ready.store(false, Ordering::Relaxed);
        bootstrap(&app, &proc, None, Some("/w"), None);
        let initialize = written(&mut agent);
        reply(&app, &proc, &initialize, json!({"agentCapabilities":{"loadSession":true}}));
        let open = written(&mut agent);
        assert_eq!(open["method"], "session/new");
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":open["id"],"error":{"code":-32000,"message":"Authentication required"}}).to_string());
        assert!(proc.released.load(Ordering::Relaxed));
        assert!(!proc.ready.load(Ordering::Relaxed));
    }

    #[test]
    fn images_wait_in_the_queue_until_the_handshake_answers() {
        let proc = inert_process(SessionKind::Copilot);
        // The first message of a conversation can arrive before `initialize` does; it is queued, and the
        // dispatch path checks again once the model's capability is known.
        assert!(validate(&proc, "queue", 1).is_ok());
        {
            let mut state = proc.acp.lock().unwrap();
            state.images_known = true;
            state.images = false;
        }
        assert!(validate(&proc, "queue", 1).is_err());
    }

    #[test]
    fn a_missing_api_key_is_a_stable_code_and_not_a_failed_turn() {
        let app = ctx("acp-api-key");
        let (proc, mut agent) = wired(SessionKind::Copilot);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "hi", &[]).unwrap();
        let prompt = written(&mut agent);
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":prompt["id"],"error":{"code":-32603,
            "message":"Internal error: turn failed: no API key for provider route"}}).to_string());
        assert!(!proc.turn.lock().unwrap().running);
        assert!(!proc.timeline.lock().unwrap().rows.iter().any(|row| matches!(row, ChatRow::Error { .. })));
        assert!(wire::api_key_missing(&json!({"message":"no API key for provider route"})));
        assert!(!wire::api_key_missing(&json!({"message":"rate limited"})));
    }

    #[test]
    fn unoffered_client_capabilities_are_refused() {
        let app = ctx("acp-fs");
        let (proc, mut agent) = wired(SessionKind::Copilot);
        handle_line(&app, "s", &proc, r#"{"jsonrpc":"2.0","id":3,"method":"fs/read_text_file","params":{"path":"/etc/hosts"}}"#);
        let answer = written(&mut agent);
        assert_eq!((answer["id"].as_i64(), answer["error"]["code"].as_i64()), (Some(3), Some(-32601)));
    }

    #[test]
    fn antigravity_caches_per_model_efforts_and_carries_pending_effort_across_model_switch() {
        let app = ctx("acp-agy-cache");
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        app.listen(&super::super::event_name("s"), move |payload| {
            captured.lock().unwrap().push(serde_json::from_str(payload).unwrap());
        });
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        proc.ready.store(false, Ordering::Relaxed);
        *proc.model.lock().unwrap() = Some("gemini-4-hydrogen-xhigh".into());
        *proc.effort.lock().unwrap() = None;
        bootstrap(&app, &proc, None, Some("/w"), None);
        let init = written(&mut agent);
        reply(&app, &proc, &init, json!({"agentCapabilities":{"loadSession":true}}));
        let open = written(&mut agent);
        reply(&app, &proc, &open, json!({
            "sessionId": "agy-1",
            "models": {
                "currentModelId": "gemini-4-argon-medium",
                "availableModels": [
                    {"modelId": "gemini-4-argon-low", "name": "Gemini 4 Argon (Low)"},
                    {"modelId": "gemini-4-argon-medium", "name": "Gemini 4 Argon (Medium)"},
                    {"modelId": "gemini-4-argon-high", "name": "Gemini 4 Argon (High)"},
                    {"modelId": "gemini-4-hydrogen-low", "name": "Gemini 4 Hydrogen (Low)"},
                    {"modelId": "gemini-4-hydrogen-medium", "name": "Gemini 4 Hydrogen (Medium)"},
                    {"modelId": "gemini-4-hydrogen-high", "name": "Gemini 4 Hydrogen (High)"},
                    {"modelId": "gemini-4-hydrogen-xhigh", "name": "Gemini 4 Hydrogen (Extra)"},
                    {"modelId": "gemini-pro-orchestrator", "name": "Orchestrator (Carbon)"},
                    {"modelId": "gpt-oss-120b-medium", "name": "GPT-OSS 120B (Medium)"}
                ]
            },
            "modes": {
                "currentModeId": "default",
                "availableModes": [
                    {"id": "default", "name": "Default"},
                    {"id": "plan", "name": "Plan"}
                ]
            },
            "configOptions": [
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-argon", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"},
                    {"value": "gemini-pro-orchestrator", "name": "Orchestrator (Carbon)"},
                    {"value": "gpt-oss-120b-medium", "name": "GPT-OSS 120B (Medium)"}
                ]},
                {"type": "select", "id": "thought_level", "category": "thought_level", "currentValue": "medium", "options": [
                    {"value": "low", "name": "Low"},
                    {"value": "medium", "name": "Medium"},
                    {"value": "high", "name": "High"}
                ]}
            ]
        }));
        let set_model_req = written(&mut agent);
        assert_eq!(set_model_req["params"]["configId"], "model");
        assert_eq!(set_model_req["params"]["value"], "gemini-4-hydrogen");

        // Cache was written immediately with Hydrogen's xhigh and Argon's low..high.
        let cached = crate::agent::antigravity_models::list(&app, "/nonexistent/agy").unwrap();
        assert_eq!(cached.len(), 4);
        assert_eq!(cached[0].effort_levels, ["low", "medium", "high"]);
        assert_eq!(cached[1].effort_levels, ["low", "medium", "high", "xhigh"]);
        assert!(cached[2].effort_levels.is_empty());
        assert!(cached[3].effort_levels.is_empty());

        // Emulate the config_option_update notification before the RPC answer: pending_effort keeps xhigh.
        notify(&app, &proc, json!({
            "sessionUpdate": "config_option_update",
            "configOptions": [
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-hydrogen", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"},
                    {"value": "gemini-pro-orchestrator", "name": "Orchestrator (Carbon)"},
                    {"value": "gpt-oss-120b-medium", "name": "GPT-OSS 120B (Medium)"}
                ]},
                {"type": "select", "id": "thought_level", "category": "thought_level", "currentValue": "medium", "options": [
                    {"value": "low", "name": "Low"},
                    {"value": "medium", "name": "Medium"},
                    {"value": "high", "name": "High"},
                    {"value": "xhigh", "name": "Extra"}
                ]}
            ]
        }));
        assert_eq!(proc.effort.lock().unwrap().as_deref(), Some("xhigh"));

        // Completing the startup model switch sends thought_level = xhigh now that Hydrogen offers it.
        reply(&app, &proc, &set_model_req, json!({
            "configOptions": [
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-hydrogen", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"},
                    {"value": "gemini-pro-orchestrator", "name": "Orchestrator (Carbon)"},
                    {"value": "gpt-oss-120b-medium", "name": "GPT-OSS 120B (Medium)"}
                ]},
                {"type": "select", "id": "thought_level", "category": "thought_level", "currentValue": "medium", "options": [
                    {"value": "low", "name": "Low"},
                    {"value": "medium", "name": "Medium"},
                    {"value": "high", "name": "High"},
                    {"value": "xhigh", "name": "Extra"}
                ]}
            ]
        }));
        let set_effort_req = written(&mut agent);
        assert_eq!(set_effort_req["params"]["configId"], "thought_level");
        assert_eq!(set_effort_req["params"]["value"], "xhigh");
        assert_eq!(proc.effort.lock().unwrap().as_deref(), Some("xhigh"));

        // Switching to a model with no thought_level option clears effort and notifies the UI.
        events.lock().unwrap().clear();
        notify(&app, &proc, json!({
            "sessionUpdate": "config_option_update",
            "configOptions": [
                {"type": "select", "id": "model", "category": "model", "currentValue": "gpt-oss-120b-medium", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"},
                    {"value": "gemini-pro-orchestrator", "name": "Orchestrator (Carbon)"},
                    {"value": "gpt-oss-120b-medium", "name": "GPT-OSS 120B (Medium)"}
                ]}
            ]
        }));
        assert_eq!(proc.model.lock().unwrap().as_deref(), Some("gpt-oss-120b-medium"));
        assert!(proc.effort.lock().unwrap().is_none());
        let seen = events.lock().unwrap();
        assert!(seen.iter().any(|e| e["type"] == "models"));
        assert!(seen.iter().any(|e| e["type"] == "settingsChanged" && e["effort"].is_null()));
    }

    #[test]
    fn antigravity_omits_collaboration_modes_and_syncs_permissions_including_bypass_fallback() {
        let app = ctx("acp-agy-single-perm");
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        app.listen(&super::super::event_name("s"), move |payload| {
            captured.lock().unwrap().push(serde_json::from_str(payload).unwrap());
        });
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        proc.ready.store(false, Ordering::Relaxed);
        *proc.mode.lock().unwrap() = "bypassPermissions".into();
        bootstrap(&app, &proc, None, Some("/w"), None);
        let init = written(&mut agent);
        reply(&app, &proc, &init, json!({"agentCapabilities":{"loadSession":true}}));
        let open = written(&mut agent);
        reply(&app, &proc, &open, json!({
            "sessionId": "agy-perm-1",
            "modes": {
                "currentModeId": "plan",
                "availableModes": [
                    {"id": "default", "name": "Default"},
                    {"id": "plan", "name": "Plan"}
                ]
            }
        }));
        // Startup in Bypass falls back to `default` when `availableModes` omits `auto`.
        let startup_mode = written(&mut agent);
        assert_eq!(startup_mode["method"], "session/set_mode");
        assert_eq!(startup_mode["params"]["modeId"], "default");
        reply(&app, &proc, &startup_mode, json!({}));
        notify(&app, &proc, json!({"sessionUpdate":"current_mode_update","currentModeId":"default"}));
        assert_eq!(proc.mode.lock().unwrap().as_str(), "bypassPermissions");
        assert!(proc.collaboration_modes.lock().unwrap().is_empty());
        assert!(!events.lock().unwrap().iter().any(|e| e["type"] == "collaborationModes"));

        let (app_t, proc_t) = (app.clone(), proc.clone());
        let responder = std::thread::spawn(move || {
            let plan_req = written(&mut agent);
            assert_eq!(plan_req["method"], "session/set_mode");
            assert_eq!(plan_req["params"]["modeId"], "plan");
            reply(&app_t, &proc_t, &plan_req, json!({}));

            let bypass_req = written(&mut agent);
            assert_eq!(bypass_req["method"], "session/set_mode");
            assert_eq!(bypass_req["params"]["modeId"], "default");
            reply(&app_t, &proc_t, &bypass_req, json!({}));
        });

        sync_antigravity_permission(&proc, "plan").unwrap();
        assert_eq!(proc.acp.lock().unwrap().catalog.mode.as_deref(), Some("plan"));

        *proc.mode.lock().unwrap() = "bypassPermissions".into();
        sync_antigravity_permission(&proc, "bypassPermissions").unwrap();
        assert_eq!(proc.acp.lock().unwrap().catalog.mode.as_deref(), Some("default"));
        responder.join().unwrap();

        events.lock().unwrap().clear();
        notify(&app, &proc, json!({"sessionUpdate":"current_mode_update","currentModeId":"plan"}));
        assert_eq!(proc.mode.lock().unwrap().as_str(), "plan");
        let seen = events.lock().unwrap();
        assert!(seen.iter().any(|e| e["type"] == "settingsChanged" && e["mode"] == "plan"));
        assert!(!seen.iter().any(|e| e["type"] == "collaborationModeChanged"));
    }

    #[test]
    fn antigravity_bypass_startup_serializes_mode_before_model_and_ignores_stale_default_updates() {
        let app = ctx("acp-agy-bypass-startup");
        app.db().conn.lock().unwrap().execute_batch(
            "INSERT INTO projects(id,name,root_path,created_at) VALUES ('p','Test','/w',0); \
             INSERT INTO sessions(id,project_id,name,kind,engine,permission_mode,created_at) VALUES ('s','p','Antigravity 1','antigravity','chat','skip',0);"
        ).unwrap();
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        app.listen(&super::super::event_name("s"), move |payload| {
            captured.lock().unwrap().push(serde_json::from_str(payload).unwrap());
        });
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        proc.ready.store(false, Ordering::Relaxed);
        *proc.mode.lock().unwrap() = "bypassPermissions".into();
        *proc.model.lock().unwrap() = Some("gemini-4-hydrogen-xhigh".into());
        *proc.effort.lock().unwrap() = None;
        bootstrap(&app, &proc, None, Some("/w"), Some("auto"));
        let init = written(&mut agent);
        reply(&app, &proc, &init, json!({"agentCapabilities":{"loadSession":true}}));
        let open = written(&mut agent);
        reply(&app, &proc, &open, json!({
            "sessionId": "agy-bypass-1",
            "models": {
                "currentModelId": "gemini-4-argon-medium",
                "availableModels": [
                    {"modelId": "gemini-4-argon-low", "name": "Gemini 4 Argon (Low)"},
                    {"modelId": "gemini-4-argon-medium", "name": "Gemini 4 Argon (Medium)"},
                    {"modelId": "gemini-4-argon-high", "name": "Gemini 4 Argon (High)"},
                    {"modelId": "gemini-4-hydrogen-low", "name": "Gemini 4 Hydrogen (Low)"},
                    {"modelId": "gemini-4-hydrogen-medium", "name": "Gemini 4 Hydrogen (Medium)"},
                    {"modelId": "gemini-4-hydrogen-high", "name": "Gemini 4 Hydrogen (High)"},
                    {"modelId": "gemini-4-hydrogen-xhigh", "name": "Gemini 4 Hydrogen (Extra)"}
                ]
            },
            "modes": {
                "currentModeId": "default",
                "availableModes": [
                    {"id": "default", "name": "Default"},
                    {"id": "accept-edits", "name": "Accept Edits"},
                    {"id": "plan", "name": "Plan"},
                    {"id": "auto", "name": "Auto"}
                ]
            },
            "configOptions": [
                {"type": "select", "id": "mode", "category": "mode", "currentValue": "default", "options": [
                    {"value": "default", "name": "Default"},
                    {"value": "accept-edits", "name": "Accept Edits"},
                    {"value": "plan", "name": "Plan"},
                    {"value": "auto", "name": "Auto"}
                ]},
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-argon", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"}
                ]},
                {"type": "select", "id": "thought_level", "category": "thought_level", "currentValue": "medium", "options": [
                    {"value": "low", "name": "Low"},
                    {"value": "medium", "name": "Medium"},
                    {"value": "high", "name": "High"}
                ]}
            ]
        }));

        // Startup must switch mode to `auto` first and hold model/effort switches until it settles.
        let mode_req = written(&mut agent);
        assert_eq!(mode_req["method"], "session/set_config_option");
        assert_eq!(mode_req["params"]["configId"], "mode");
        assert_eq!(mode_req["params"]["value"], "auto");

        // Stale `default` notifications arriving before the mode switch completes must not drop Bypass.
        notify(&app, &proc, json!({"sessionUpdate":"current_mode_update","currentModeId":"default"}));
        notify(&app, &proc, json!({
            "sessionUpdate": "config_option_update",
            "configOptions": [
                {"type": "select", "id": "mode", "category": "mode", "currentValue": "default", "options": [
                    {"value": "default", "name": "Default"},
                    {"value": "accept-edits", "name": "Accept Edits"},
                    {"value": "plan", "name": "Plan"},
                    {"value": "auto", "name": "Auto"}
                ]},
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-argon", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"}
                ]},
                {"type": "select", "id": "thought_level", "category": "thought_level", "currentValue": "medium", "options": [
                    {"value": "low", "name": "Low"},
                    {"value": "medium", "name": "Medium"},
                    {"value": "high", "name": "High"}
                ]}
            ]
        }));
        assert_eq!(proc.mode.lock().unwrap().as_str(), "bypassPermissions");
        assert_eq!(
            crate::db::repo::get_permission_mode(&app.db().conn.lock().unwrap(), "s").unwrap().as_deref(),
            Some("skip")
        );
        assert_eq!(proc.model.lock().unwrap().as_deref(), Some("gemini-4-hydrogen"));
        assert_eq!(proc.effort.lock().unwrap().as_deref(), Some("xhigh"));

        // Completing the mode switch dispatches `set_model`.
        reply(&app, &proc, &mode_req, json!({}));
        let set_model_req = written(&mut agent);
        assert_eq!(set_model_req["method"], "session/set_config_option");
        assert_eq!(set_model_req["params"]["configId"], "model");
        assert_eq!(set_model_req["params"]["value"], "gemini-4-hydrogen");

        notify(&app, &proc, json!({"sessionUpdate":"current_mode_update","currentModeId":"auto"}));
        reply(&app, &proc, &set_model_req, json!({
            "configOptions": [
                {"type": "select", "id": "mode", "category": "mode", "currentValue": "auto", "options": [
                    {"value": "default", "name": "Default"},
                    {"value": "accept-edits", "name": "Accept Edits"},
                    {"value": "plan", "name": "Plan"},
                    {"value": "auto", "name": "Auto"}
                ]},
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-hydrogen", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"}
                ]},
                {"type": "select", "id": "thought_level", "category": "thought_level", "currentValue": "medium", "options": [
                    {"value": "low", "name": "Low"},
                    {"value": "medium", "name": "Medium"},
                    {"value": "high", "name": "High"},
                    {"value": "xhigh", "name": "Extra"}
                ]}
            ]
        }));
        let set_effort_req = written(&mut agent);
        assert_eq!(set_effort_req["method"], "session/set_config_option");
        assert_eq!(set_effort_req["params"]["configId"], "thought_level");
        assert_eq!(set_effort_req["params"]["value"], "xhigh");

        assert_eq!(proc.mode.lock().unwrap().as_str(), "bypassPermissions");
        assert_eq!(
            crate::db::repo::get_permission_mode(&app.db().conn.lock().unwrap(), "s").unwrap().as_deref(),
            Some("skip")
        );
        let seen = events.lock().unwrap();
        assert!(!seen.iter().any(|e| {
            e["type"] == "settingsChanged" && matches!(e["mode"].as_str(), Some("default") | Some("auto"))
        }));
    }

    #[test]
    fn elicitation_stays_interactive_in_bypass_and_round_trips_answers() {
        let app = ctx("acp-elicit-bypass");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        *proc.mode.lock().unwrap() = "bypassPermissions".into();
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;

        // `session/request_permission` still auto-approves immediately in bypass mode.
        handle_line(&app, "s", &proc, &json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "session/request_permission",
            "params": {
                "sessionId": "s1",
                "toolCall": {"toolCallId": "c1", "kind": "execute"},
                "options": [{"optionId": "once", "kind": "allow_once"}]
            }
        }).to_string());
        assert_eq!(written(&mut agent), json!({"jsonrpc":"2.0","id":10,"result":{"outcome":{"outcome":"selected","optionId":"once"}}}));
        assert!(proc.permissions.lock().unwrap().is_empty());

        // `elicitation/create` (`ask_question`) must stay interactive even in bypassPermissions.
        handle_line(&app, "s", &proc, &json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "elicitation/create",
            "params": {
                "mode": "form",
                "message": "Confirm release",
                "requestedSchema": {
                    "type": "object",
                    "properties": {
                        "q1": {
                            "type": "string",
                            "title": "Target environment?",
                            "oneOf": [{"const": "1", "title": "Staging"}, {"const": "2", "title": "Prod"}]
                        },
                        "q1_other": {"type": "string", "title": "Other"}
                    },
                    "required": ["q1"]
                }
            }
        }).to_string());
        let card = proc.permissions.lock().unwrap().get("acp-elicit-11").cloned().expect("elicitation card created");
        assert_eq!(card["tool_name"], "AskUserQuestion");
        assert_eq!(card["input"]["_acpAskQuestion"], true);

        // Switching/running approve_pending must never auto-dismiss open elicitation cards.
        approve_pending(&app, "s", &proc);
        assert!(proc.permissions.lock().unwrap().contains_key("acp-elicit-11"));

        let answers = json!({"answers": {"Target environment?": "Prod"}});
        respond(&app, "s", &proc, "acp-elicit-11", true, Some(&answers), None, None).unwrap();
        assert_eq!(written(&mut agent), json!({
            "jsonrpc": "2.0",
            "id": 11,
            "result": {"action": "accept", "content": {"q1": "2"}}
        }));
        assert!(proc.permissions.lock().unwrap().is_empty());
    }

    #[test]
    fn antigravity_stop_hook_parks_wakes_and_retires_cleanly() {
        let app = ctx("acp-stop-park");
        let sys_dir = std::env::temp_dir().join(format!("acp-stop-park-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(sys_dir.join("messages")).unwrap();
        let (proc, _agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("main-conv".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.acp.lock().unwrap().tasks.system_dir = Some(sys_dir.clone());
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "turn one", &[]).unwrap();
        let first_prompt_id = proc.acp.lock().unwrap().prompt.clone().unwrap();

        // Subagent stop hooks and non-NO_TOOL_CALL / safety-capped invocations must allow immediately.
        assert_eq!(handle_stop_hook(&app, "s", &proc, r#"{"conversationId":"sub-conv","terminationReason":"NO_TOOL_CALL","executionNum":0}"#), r#"{"decision":"allow"}"#);
        assert_eq!(handle_stop_hook(&app, "s", &proc, r#"{"conversationId":"main-conv","terminationReason":"USER_CANCELED","executionNum":0}"#), r#"{"decision":"allow"}"#);
        assert_eq!(handle_stop_hook(&app, "s", &proc, &format!(r#"{{"conversationId":"main-conv","terminationReason":"NO_TOOL_CALL","executionNum":{}}}"#, MAX_SAFE_STOP_CONTINUATIONS)), r#"{"decision":"allow"}"#);

        // Main NO_TOOL_CALL stop parks the turn until an unread delivery arrives.
        let (app_t, proc_t) = (app.clone(), proc.clone());
        let h1 = std::thread::spawn(move || {
            handle_stop_hook(&app_t, "s", &proc_t, r#"{"conversationId":"main-conv","terminationReason":"NO_TOOL_CALL","executionNum":0}"#)
        });
        for _ in 0..40 {
            if is_stop_parked(&proc) { break; }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(is_stop_parked(&proc));
        assert!(!proc.turn.lock().unwrap().running);

        std::fs::write(
            sys_dir.join("messages/m1.json"),
            json!({"sender":"task-1","timestamp":"2026-11-16T02:00:00Z","content":"Background command \"Timer\" completed (exit code 0)"}).to_string(),
        ).unwrap();
        assert_eq!(h1.join().unwrap(), r#"{"decision":"continue"}"#);
        assert!(proc.turn.lock().unwrap().running);
        assert!(proc.timeline.lock().unwrap().rows.iter().any(|r| matches!(r, ChatRow::Wake { .. })));

        // Re-park on executionNum 1 and retire cleanly when a new user prompt starts.
        let (app_t2, proc_t2) = (app.clone(), proc.clone());
        let h2 = std::thread::spawn(move || {
            handle_stop_hook(&app_t2, "s", &proc_t2, r#"{"conversationId":"main-conv","terminationReason":"NO_TOOL_CALL","executionNum":1}"#)
        });
        for _ in 0..40 {
            if is_stop_parked(&proc) { break; }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(is_stop_parked(&proc));

        let (app_end, proc_end, pid_end) = (app.clone(), proc.clone(), first_prompt_id.clone());
        let end_thread = std::thread::spawn(move || {
            for _ in 0..40 {
                if !is_stop_parked(&proc_end) { break; }
                std::thread::sleep(Duration::from_millis(20));
            }
            handle_line(&app_end, "s", &proc_end, &json!({"jsonrpc":"2.0","id":pid_end,"result":{"stopReason":"end_turn"}}).to_string());
        });
        retire_parked_stop_before_send(&proc);
        assert_eq!(h2.join().unwrap(), r#"{"decision":"allow"}"#);
        end_thread.join().unwrap();
        assert!(proc.acp.lock().unwrap().prompt.is_none());
        let _ = std::fs::remove_dir_all(sys_dir);
    }

    #[test]
    fn detached_recovery_cancels_stalled_tool_and_resumes_live_prompt() {
        let app = ctx("acp-detached-recovery");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);

        recover_stalled_tool(
            &app,
            "s",
            &proc,
            "agy-step-5-call-0",
            "ask_question",
            &json!({"questions": [{"question": "Continue?", "multi_select": false, "options": [{"label": "Yes"}]}]}),
        );
        assert_eq!(written(&mut agent)["method"], "session/cancel");
        let card = proc.permissions.lock().unwrap().get("acp-detached-agy-step-5-call-0").cloned().unwrap();
        assert_eq!(card["tool_name"], "AskUserQuestion");
        assert_eq!(card["input"]["questions"][0]["multiSelect"], false);

        respond(&app, "s", &proc, "acp-detached-agy-step-5-call-0", true, Some(&json!({"answers": {"Continue?": "Yes"}})), None, None).unwrap();
        assert!(proc.permissions.lock().unwrap().is_empty());
        let resume = written(&mut agent);
        assert_eq!(resume["method"], "session/prompt");
        assert!(resume["params"]["prompt"][0]["text"].as_str().unwrap().contains("Continue?"));
    }

    fn resolve_real_agy_bin() -> String {
        for candidate in [
            std::env::var("VLX_TEST_AGY_BIN").ok(),
            Some("/Users/vlinx/.local/bin/agy".to_string()),
        ]
        .into_iter()
        .flatten()
        {
            if std::path::Path::new(&candidate).is_file() {
                return candidate;
            }
        }
        "agy".to_string()
    }

    struct RealAgyHarness {
        app: AppCtx,
        sid: String,
        dir: PathBuf,
        workdir: PathBuf,
        proc: Option<Arc<ChatProcess>>,
    }

    impl RealAgyHarness {
        fn start(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("vlx-agy-e2e-{tag}-{}", uuid::Uuid::new_v4()));
            let workdir = dir.join("work");
            std::fs::create_dir_all(&workdir).unwrap();
            let db = crate::db::Db::open(&dir.join("t.db")).unwrap();
            let app = AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(dir.clone(), db)));
            let hooks = crate::agent::server::HookServer::start(app.clone()).unwrap();
            #[allow(irrefutable_let_patterns)]
            if let AppCtx::Headless(host) = &app {
                host.set_hooks(hooks);
            }
            let sid = format!("e2e-{tag}-{}", uuid::Uuid::new_v4());
            let bin = resolve_real_agy_bin();
            {
                let conn = app.db().conn.lock().unwrap();
                crate::db::system_collections::ensure(&conn).unwrap();
                crate::db::repo::create_session_identified(
                    &conn,
                    &sid,
                    crate::db::system_collections::CHAT,
                    None,
                    tag,
                    SessionKind::Antigravity,
                    None,
                    workdir.to_str(),
                    None,
                    None,
                    None,
                    None,
                    Some("default"),
                    None,
                    None,
                    Some(&bin),
                    Some("chat"),
                )
                .unwrap();
            }
            crate::command_core::chat_start(&app, &sid, None, None, false).unwrap();
            let proc = app.chat().get(&sid).unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            while !proc.ready.load(Ordering::Relaxed) && Instant::now() < deadline {
                assert!(proc.alive.load(Ordering::Relaxed), "agy ACP exited during handshake");
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(proc.ready.load(Ordering::Relaxed), "timed out waiting for agy ACP ready");
            assert_eq!(proc.mode.lock().unwrap().as_str(), "default");
            Self { app, sid, dir, workdir, proc: Some(proc) }
        }

        fn wait_until<T>(&self, timeout: Duration, label: &str, mut check: impl FnMut(&Arc<ChatProcess>) -> Option<T>) -> T {
            let proc = self.proc.as_ref().unwrap();
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                if let Some(val) = check(proc) {
                    return val;
                }
                assert!(proc.alive.load(Ordering::Relaxed), "agy ACP exited unexpectedly while waiting for {label}");
                std::thread::sleep(Duration::from_millis(100));
            }
            let snap = self.app.chat().snapshot(&self.sid);
            panic!("timed out waiting for {label}; proc_alive={}, turn_running={}, turn_started_at={:?}, parked={}, perms={}, rows={:?}",
                snap.running,
                proc.turn.lock().unwrap().running,
                snap.turn_started_at,
                is_stop_parked(proc),
                snap.permissions.len(),
                snap.rows,
            );
        }
    }

    impl Drop for RealAgyHarness {
        fn drop(&mut self) {
            let _ = crate::command_core::chat_stop(&self.app, &self.sid);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn process_exists(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|s| s.success())
    }

    #[test]
    #[ignore = "hits real agy v1.5.9 backend"]
    fn real_antigravity_acp_permissions_schedule_wake_multiturn_and_cleanup() {
        let h = RealAgyHarness::start("perm-wake");
        let marker1 = h.workdir.join("turn1.ok");
        let marker2 = h.workdir.join("wake.ok");
        let mid1 = format!("msg-{}", uuid::Uuid::new_v4());
        crate::command_core::chat_send(
            &h.app,
            &h.sid,
            &format!(
                "Use run_command (WaitMsBeforeAsync: 5000, do not use write_file) to execute the exact shell command: touch '{}' && echo TURN1_OK > '{}'. After the command completes, reply DONE.",
                marker1.display(),
                marker1.display()
            ),
            vec![],
            Some("queue"),
            Some(&mid1),
        )
        .unwrap();

        let card1 = h.wait_until(Duration::from_secs(60), "Turn 1 session/request_permission", |_| {
            h.app.chat().snapshot(&h.sid).permissions.into_iter().find(|p| {
                p.get("id").and_then(Value::as_str).is_some_and(|id| !id.starts_with("acp-elicit-"))
            })
        });
        let card1_id = card1["id"].as_str().unwrap().to_string();
        h.app.chat().respond_permission(&h.app, &h.sid, &card1_id, true, None, None, None).unwrap();

        h.wait_until(Duration::from_secs(60), "Turn 1 parked Stop", |proc| {
            let snap = h.app.chat().snapshot(&h.sid);
            (!proc.turn.lock().unwrap().running && snap.turn_started_at.is_none() && is_stop_parked(proc) && marker1.exists()).then_some(())
        });
        assert_eq!(h.proc.as_ref().unwrap().acp.lock().unwrap().stop_execution_nums, vec![0]);

        // Send Turn 2 while Turn 1 Stop is parked; Stop must retire with allow and Turn 2 resets executionNum to 0.
        let mid2 = format!("msg-{}", uuid::Uuid::new_v4());
        crate::command_core::chat_send(
            &h.app,
            &h.sid,
            &format!(
                "Use the schedule tool with DurationSeconds: 6 and Prompt: 'Use run_command (WaitMsBeforeAsync: 5000, do not use write_file) to execute: echo WAKE_OK > {}, then reply WAKE_DONE.' Immediately after calling schedule, reply SCHEDULED and end your turn (call no more tools) so the timer notification wakes you.",
                marker2.display()
            ),
            vec![],
            Some("queue"),
            Some(&mid2),
        )
        .unwrap();

        // Approve any permission required on Turn 2 or on the post-timer wakeup, and wait for ChatRow::Wake + marker2.
        let wake_deadline = Instant::now() + Duration::from_secs(90);
        let mut saw_wake_row = false;
        while Instant::now() < wake_deadline {
            let snap = h.app.chat().snapshot(&h.sid);
            if snap.rows.iter().any(|r| matches!(r, ChatRow::Wake { .. })) {
                saw_wake_row = true;
            }
            for perm in &snap.permissions {
                if let Some(pid) = perm.get("id").and_then(Value::as_str) {
                    if !pid.starts_with("acp-elicit-") {
                        let _ = h.app.chat().respond_permission(&h.app, &h.sid, pid, true, None, None, None);
                    }
                }
            }
            if saw_wake_row && marker2.exists() && snap.turn_started_at.is_none() {
                break;
            }
            if marker2.exists() && snap.turn_started_at.is_none() && !saw_wake_row {
                crate::agent::chat::engine::antigravity_tasks::refresh(&h.app, &h.sid, h.proc.as_ref().unwrap(), false);
                saw_wake_row = h.app.chat().snapshot(&h.sid).rows.iter().any(|r| matches!(r, ChatRow::Wake { .. }));
                if saw_wake_row {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        assert!(saw_wake_row, "expected ChatRow::Wake when schedule timer woke parked ACP session");
        assert!(marker2.exists(), "expected post-wakeup run_command permission approval to create {}", marker2.display());
        h.wait_until(Duration::from_secs(30), "Turn 2 re-parked Stop", |proc| {
            let snap = h.app.chat().snapshot(&h.sid);
            (!proc.turn.lock().unwrap().running && snap.turn_started_at.is_none() && is_stop_parked(proc)).then_some(())
        });

        // Turn 3 sent after a Stop-continue wakeup must retire Turn 2 Stop and reset executionNum to 0.
        let mid3 = format!("msg-{}", uuid::Uuid::new_v4());
        crate::command_core::chat_send(
            &h.app,
            &h.sid,
            "Do not call any tools. Reply ONLY with TURN3_OK.",
            vec![],
            Some("queue"),
            Some(&mid3),
        )
        .unwrap();
        h.wait_until(Duration::from_secs(60), "Turn 3 reply and parked Stop", |proc| {
            let snap = h.app.chat().snapshot(&h.sid);
            let has_reply = snap.rows.iter().rev().any(|r| {
                matches!(r, ChatRow::Assistant { text, streaming: false, .. } if text.contains("TURN3_OK"))
            });
            (!proc.turn.lock().unwrap().running && snap.turn_started_at.is_none() && is_stop_parked(proc) && has_reply).then_some(())
        });
        {
            let nums = h.proc.as_ref().unwrap().acp.lock().unwrap().stop_execution_nums.clone();
            assert_eq!(
                nums.last().copied(),
                Some(0),
                "expected executionNum to reset to 0 for Turn 3 after Turn 2 wakeup retirement, got {nums:?}"
            );
        }

        // Interrupt/stop while session is parked/idle and confirm OS child exits cleanly with no orphans.
        let pid = h.proc.as_ref().unwrap().pid;
        let _ = crate::command_core::chat_interrupt(&h.app, &h.sid);
        crate::command_core::chat_stop(&h.app, &h.sid).unwrap();
        let stop_deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < stop_deadline && process_exists(pid) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!process_exists(pid), "agy child process {pid} survived chat_stop");
    }

    #[test]
    #[ignore = "hits real agy v1.5.9 backend"]
    fn real_antigravity_acp_ask_question_elicitation_round_trip() {
        let h = RealAgyHarness::start("ask-question");
        let mid = format!("msg-{}", uuid::Uuid::new_v4());
        crate::command_core::chat_send(
            &h.app,
            &h.sid,
            "Do not run any commands or edit files. Immediately call ask_question with two questions: question 1 with is_multi_select: false titled 'Release channel?' with options 'Stable' and 'Beta'; question 2 with is_multi_select: true titled 'Target OS?' with options 'macOS' and 'Linux'. After I answer, reply repeating EVERY value from my answer verbatim.",
            vec![],
            Some("queue"),
            Some(&mid),
        )
        .unwrap();

        let card = h.wait_until(Duration::from_secs(60), "AskUserQuestion elicitation card", |_| {
            h.app.chat().snapshot(&h.sid).permissions.into_iter().find(|p| p["tool_name"] == "AskUserQuestion")
        });
        let card_id = card["id"].as_str().unwrap().to_string();
        assert_eq!(card["input"]["_acpAskQuestion"], true);
        let qs = card["input"]["questions"].as_array().unwrap();
        assert!(qs.len() >= 2, "expected 2 questions in card: {card:?}");
        let q1_text = qs[0]["question"].as_str().unwrap().to_string();
        let q2_text = qs[1]["question"].as_str().unwrap().to_string();
        h.app.chat().respond_permission(
            &h.app,
            &h.sid,
            &card_id,
            true,
            Some(json!({
                "answers": {
                    q1_text: "NightlyCustom",
                    q2_text: "macOS, WinCustom"
                }
            })),
            None,
            None,
        ).unwrap();

        h.wait_until(Duration::from_secs(60), "assistant reply after ask_question", |proc| {
            let snap = h.app.chat().snapshot(&h.sid);
            if proc.turn.lock().unwrap().running || snap.turn_started_at.is_some() { return None; }
            snap.rows.iter().rev().find_map(|r| match r {
                ChatRow::Assistant { text, streaming: false, .. }
                    if text.contains("NightlyCustom") && text.contains("macOS") => Some(text.clone()),
                _ => None,
            })
        });

        let pid = h.proc.as_ref().unwrap().pid;
        crate::command_core::chat_stop(&h.app, &h.sid).unwrap();
        let stop_deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < stop_deadline && process_exists(pid) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!process_exists(pid), "agy child process {pid} survived chat_stop");
    }

    #[test]
    #[ignore = "hits real agy v1.5.9 CLI binary"]
    fn real_antigravity_tui_prompt_interactive_stays_alive() {
        let bin = resolve_real_agy_bin();
        let dir = std::env::temp_dir().join(format!("vlx-agy-tui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let ep = crate::agent::server::HookEndpoint { port: 18999, token: "test-tok".into() };
        let spawn = crate::agent::inject::prepare(
            SessionKind::Antigravity,
            "/bin/zsh",
            &bin,
            &ep,
            "sid-tui-e2e",
            None,
            false,
            Some("Reply ONLY with TUI_OK"),
        );
        let launch = spawn.launch.expect("Antigravity TUI produces a shell launch command");
        assert!(launch.contains("--prompt-interactive \"$VLX_INIT_PROMPT\""), "launch: {launch}");
        let mut envs: Vec<(String, String)> = spawn.env;
        envs.push(("VLX_SPAWN_URL".into(), "http://127.0.0.1:18999".into()));

        // Run in a real pseudo-terminal for 4s: `--prompt-interactive` must keep the TUI open rather than
        // exiting immediately as a bare positional `--print` prompt would.
        let py = r#"
import os, pty, signal, subprocess, sys, time
master, slave = pty.openpty()
proc = subprocess.Popen(["/bin/zsh", "-lc", sys.argv[1]], cwd=sys.argv[2], stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
os.close(slave)
time.sleep(4)
alive = proc.poll() is None
try:
    os.killpg(proc.pid, signal.SIGKILL)
except ProcessLookupError:
    pass
proc.wait()
os.close(master)
sys.exit(0 if alive else 2)
"#;
        let mut cmd = Command::new("python3");
        cmd.args(["-c", py, &launch, dir.to_str().unwrap()]);
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let status = cmd.status().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(status.success(), "expected agy --prompt-interactive TUI to stay alive after 4s, status={status}");
    }

    #[test]
    fn recover_stalled_ask_question_normalizes_input_and_carry_over_keeps_answers() {
        let app = ctx("acp-recovery-carry-over");
        let (proc, _agent) = wired(SessionKind::Antigravity);
        recover_stalled_tool(
            &app,
            "s",
            &proc,
            "agy-step-1-call-0",
            "ask_question",
            &json!({"questions": [{"question": "Deploy?", "is_multi_select": true, "options": [{"id": "1", "text": "Yes"}]}]}),
        );
        let perms = proc.permissions.lock().unwrap();
        let card = perms.values().next().expect("detached recovery card");
        assert_eq!(card["tool_name"], "AskUserQuestion");
        assert_eq!(card["input"]["_detachedRecovery"], true);
        assert_eq!(card["input"]["questions"][0]["header"], "Q1");
        assert_eq!(card["input"]["questions"][0]["multiSelect"], true);
        assert_eq!(card["input"]["questions"][0]["options"], json!([{"label": "Yes"}]));
        drop(perms);

        let mut replay = Replay::default();
        replay.previous = vec![ChatRow::Tool {
            id: "old".into(),
            name: "AskUserQuestion".into(),
            input: json!({"questions": [{"header": "Q1", "question": "Deploy?"}], "answers": {"Deploy?": "Yes"}, "_answerList": ["Yes"]}),
            output: Some("A1: Yes".into()),
            is_error: false,
            status: "completed",
            subagent: None,
            children: Vec::new(),
        }];
        replay.place(ChatRow::Tool {
            id: "acp-t-1".into(),
            name: "AskUserQuestion".into(),
            input: json!({"questions": [{"header": "Q1", "question": "Deploy?"}]}),
            output: None,
            is_error: false,
            status: "completed",
            subagent: None,
            children: Vec::new(),
        });
        let _ = replay.carry_over();
        let ChatRow::Tool { input, output, .. } = &replay.rows[0] else { panic!() };
        assert_eq!(input["answers"], json!({"Deploy?": "Yes"}));
        assert_eq!(input["_answerList"], json!(["Yes"]));
        assert_eq!(output.as_deref(), Some("A1: Yes"));
    }

    #[test]
    fn antigravity_running_session_merges_newer_cached_models_and_restarts_idle_on_unknown_model() {
        let app = ctx("acp-agy-merge-restart");
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        app.listen(&super::super::event_name("s"), move |payload| {
            captured.lock().unwrap().push(serde_json::from_str(payload).unwrap());
        });
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        proc.ready.store(false, Ordering::Relaxed);
        bootstrap(&app, &proc, None, Some("/w"), None);
        let init = written(&mut agent);
        reply(&app, &proc, &init, json!({"agentCapabilities":{"loadSession":true}}));
        let open = written(&mut agent);
        reply(&app, &proc, &open, json!({
            "sessionId": "agy-old-1",
            "models": {
                "currentModelId": "gemini-4-argon-medium",
                "availableModels": [
                    {"modelId": "gemini-4-argon-medium", "name": "Gemini 4 Argon (Medium)"},
                    {"modelId": "gemini-4-hydrogen-high", "name": "Gemini 4 Hydrogen (High)"}
                ]
            },
            "configOptions": [
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-argon", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"}
                ]}
            ]
        }));

        // Register the live session in `app.chat()` so `remember_acp` notifies it immediately.
        app.chat().sessions.lock().unwrap().insert("s".into(), proc.clone());

        // Simulate a newer model (`gemini-4-neon`) discovered later by another session or background refresh.
        let mut newer = Catalog::default();
        newer.absorb_session(&json!({
            "configOptions": [
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-neon", "options": [
                    {"value": "gemini-4-neon", "name": "Gemini 4 Neon"},
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"}
                ]}
            ]
        }));
        crate::agent::antigravity_models::remember_acp(&app, &newer);

        let models = chat_models(&app, &proc).unwrap();
        assert!(models.iter().any(|m| m["id"] == "gemini-4-neon"));
        assert!(events.lock().unwrap().iter().any(|e| e["type"] == "models"));

        // Selecting `gemini-4-neon` on the stale running process releases it cleanly when the process
        // replies with `Unknown model ID`, allowing the next turn to start a fresh `agy acp`.
        let proc_t = proc.clone();
        let app_t = app.clone();
        let responder = std::thread::spawn(move || {
            let req = written(&mut agent);
            assert_eq!(req["method"], "session/set_config_option");
            assert_eq!(req["params"]["value"], "gemini-4-neon");
            handle_line(&app_t, "s", &proc_t, &json!({
                "jsonrpc": "2.0",
                "id": req["id"],
                "error": {"code": -32602, "message": "Unknown model ID: gemini-4-neon"}
            }).to_string());
        });
        app.chat().set_model_with_app(&app, "s", Some("gemini-4-neon")).unwrap();
        responder.join().unwrap();
        assert!(!proc.alive.load(Ordering::Relaxed));
        assert!(proc.released.load(Ordering::Relaxed));
        assert_eq!(proc.model.lock().unwrap().as_deref(), Some("gemini-4-neon"));
    }

    #[test]
    fn antigravity_cold_model_error_retries_silently_and_recovers() {
        let app = ctx("acp-agy-cold-retry");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;

        dispatch(&proc, "continue", &[]).unwrap();
        let first = written(&mut agent);
        assert_eq!(first["method"], "session/prompt");
        assert_eq!(first["params"]["prompt"], json!([{"type":"text","text":"continue"}]));

        // Simulate `agy acp` rejecting the prompt before `FetchAvailableModels` finishes.
        handle_line(&app, "s", &proc, &json!({
            "jsonrpc": "2.0",
            "id": first["id"],
            "error": {
                "code": -32603,
                "message": "failed to construct executor: unknown model key MODEL_PLACEHOLDER_M532: model not found"
            }
        }).to_string());
        assert!(proc.turn.lock().unwrap().running);
        assert!(has_active_prompt(&proc));
        assert!(!proc.timeline.lock().unwrap().rows.iter().any(|r| matches!(r, ChatRow::Error { .. })));

        // The retry thread re-dispatches `session/prompt` transparently after backoff.
        let retry = written(&mut agent);
        assert_eq!(retry["method"], "session/prompt");
        assert_ne!(retry["id"], first["id"]);
        assert_eq!(retry["params"]["prompt"], json!([{"type":"text","text":"continue"}]));

        notify(&app, &proc, json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Recovered"}}));
        reply(&app, &proc, &retry, json!({"stopReason":"end_turn"}));
        assert!(!proc.turn.lock().unwrap().running);
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert!(!rows.iter().any(|r| matches!(r, ChatRow::Error { .. })));
        assert!(rows.iter().any(|r| matches!(r, ChatRow::Assistant { text, streaming: false, .. } if text == "Recovered")));
    }

    #[test]
    fn antigravity_cold_model_retry_cancels_cleanly_and_surfaces_error_after_exhaustion() {
        // Case 1: interrupting during retry backoff finishes the turn without an error row.
        let app = ctx("acp-agy-cold-cancel");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;

        dispatch(&proc, "continue", &[]).unwrap();
        let first = written(&mut agent);
        handle_line(&app, "s", &proc, &json!({
            "jsonrpc": "2.0",
            "id": first["id"],
            "error": {
                "code": -32603,
                "message": "failed to construct executor: unknown model key MODEL_PLACEHOLDER_M532: model not found"
            }
        }).to_string());
        let ended = {
            let mut turn = proc.turn.lock().unwrap();
            super::super::interrupt_locked(&app, &proc, &mut turn).unwrap()
        };
        assert!(ended.is_some());
        assert!(!proc.turn.lock().unwrap().running);
        assert!(!has_active_prompt(&proc));
        assert!(!proc.timeline.lock().unwrap().rows.iter().any(|r| matches!(r, ChatRow::Error { .. })));

        // Case 2: if every retry keeps failing, the real error is surfaced once the budget runs out.
        let app2 = ctx("acp-agy-cold-exhaust");
        let (proc2, mut agent2) = wired(SessionKind::Antigravity);
        *proc2.agent_session_id.lock().unwrap() = Some("s2".into());
        proc2.ready.store(true, Ordering::Relaxed);
        proc2.turn.lock().unwrap().running = true;

        dispatch(&proc2, "continue", &[]).unwrap();
        let mut current = written(&mut agent2);
        for attempt in 0..=cold_model_retry_delays().len() {
            handle_line(&app2, "s", &proc2, &json!({
                "jsonrpc": "2.0",
                "id": current["id"],
                "error": {
                    "code": -32603,
                    "message": "failed to construct executor: unknown model key MODEL_PLACEHOLDER_M532: model not found"
                }
            }).to_string());
            if attempt < cold_model_retry_delays().len() {
                current = written(&mut agent2);
            }
        }
        assert!(!proc2.turn.lock().unwrap().running);
        assert!(proc2.timeline.lock().unwrap().rows.iter().any(|r| {
            matches!(r, ChatRow::Error { message, .. } if message.contains("MODEL_PLACEHOLDER_M532"))
        }));
    }

    #[test]
    fn antigravity_startup_holds_ready_until_mode_and_model_settle_and_applies_late_models() {
        let app = ctx("acp-agy-startup-hold");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        proc.ready.store(false, Ordering::Relaxed);
        *proc.mode.lock().unwrap() = "bypassPermissions".into();
        *proc.model.lock().unwrap() = Some("gemini-4-neon".into());
        *proc.effort.lock().unwrap() = Some("high".into());

        bootstrap(&app, &proc, None, Some("/w"), Some("auto"));
        let init = written(&mut agent);
        reply(&app, &proc, &init, json!({"agentCapabilities":{"loadSession":true}}));
        let open = written(&mut agent);
        // Simulate `session/new` returning before `FetchAvailableModels` has populated models.
        reply(&app, &proc, &open, json!({
            "sessionId": "agy-cold-1",
            "modes": {
                "currentModeId": "default",
                "availableModes": [
                    {"id": "default", "name": "Default"},
                    {"id": "auto", "name": "Auto"}
                ]
            }
        }));
        // Startup must wait for `set_mode` before marking ready.
        let mode_req = written(&mut agent);
        assert_eq!(mode_req["method"], "session/set_mode");
        assert!(!proc.ready.load(Ordering::Relaxed));
        assert!(proc.acp.lock().unwrap().awaiting_initial_models);

        reply(&app, &proc, &mode_req, json!({}));
        assert!(proc.ready.load(Ordering::Relaxed));
        // Saved model stays preserved while waiting for `config_option_update`.
        assert_eq!(proc.model.lock().unwrap().as_deref(), Some("gemini-4-neon"));

        // Late `config_option_update` arrives once `FetchAvailableModels` finishes: apply saved model
        // instead of overwriting it with the agent's default `gemini-4-argon`.
        notify(&app, &proc, json!({
            "sessionUpdate": "config_option_update",
            "configOptions": [
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-argon", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-neon", "name": "Gemini 4 Neon"}
                ]},
                {"type": "select", "id": "thought_level", "category": "thought_level", "currentValue": "medium", "options": [
                    {"value": "low", "name": "Low"},
                    {"value": "medium", "name": "Medium"},
                    {"value": "high", "name": "High"}
                ]}
            ]
        }));
        let set_model_req = written(&mut agent);
        assert_eq!(set_model_req["method"], "session/set_config_option");
        assert_eq!(set_model_req["params"]["configId"], "model");
        assert_eq!(set_model_req["params"]["value"], "gemini-4-neon");
        assert_eq!(proc.model.lock().unwrap().as_deref(), Some("gemini-4-neon"));
    }
}
