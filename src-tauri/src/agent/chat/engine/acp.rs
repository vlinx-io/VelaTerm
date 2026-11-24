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
use crate::agent::chat::antigravity_outcome::{HiddenTurnFailure, PromptOutcomeWatch};
use crate::agent::chat::antigravity_protocol::{
    acp_mode_for_permission, is_invalid_thinking_signature, is_transient_upstream_error,
    is_usage_limit_error, parse_usage_limit_reset, permission_for_acp_mode,
    strip_streamed_401_warning, strip_streamed_transient_warning, tool_row,
    AUTH_RECOVERY_PROMPT, TRANSIENT_RECOVERY_PROMPT,
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
/// Default silence threshold before a prompt with no open tool, permission, background task, or
/// native DB write activity is recovered by replacing the wedged Antigravity process.
/// Large tool-call generations (e.g. writing a full source file) produce no ACP `session/update`
/// until the call completes and may not flush to the native DB for several minutes.
const DEFAULT_PROMPT_INACTIVITY_MS: u64 = 600_000;

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

/// Backoff delays when retrying an Antigravity turn interrupted by a transient upstream API or
/// transport error (such as HTTP 404/500/503/504 or mid-stream EOF/connection reset).
fn transient_upstream_retry_delays() -> &'static [Duration] {
    #[cfg(not(test))]
    const DELAYS: &[Duration] = &[
        Duration::from_secs(1),
        Duration::from_secs(2),
        Duration::from_secs(4),
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
    had_other_output: bool,
    sole_assistant_id: Option<String>,
    chunk_cold_error: Option<String>,
}

#[derive(Clone)]
pub(in crate::agent::chat::engine) struct StartupPrompt {
    pub id: String,
    pub text: String,
    pub images: Vec<ChatImage>,
    pub sent_at: u64,
}

#[derive(Clone)]
pub(in crate::agent::chat::engine) struct Pending401Continuation {
    text: String,
    images: Vec<ChatImage>,
    ensure_user_row: bool,
    started_at: Option<u64>,
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
    /// First user turn accepted onto the timeline while the Antigravity ACP handshake is still opening.
    pub(in crate::agent::chat::engine) startup_prompt: Option<StartupPrompt>,
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
    /// Credential startup gate held only until this ACP process finishes opening or fails startup.
    pub(in crate::agent::chat::engine) auth_gate_permit: Option<crate::agent::antigravity::AuthGatePermit>,
    /// Antigravity credential epoch observed when this ACP process launched.
    pub(in crate::agent::chat::engine) auth_epoch_at_launch: u64,
    /// Streamed OAuth 401 warning captured during the active turn before `end_turn` arrives.
    turn_auth_error: Option<String>,
    /// Assistant row id holding a split streamed OAuth 401 warning chunk (`"\n⚠ "` + error text).
    split_auth_row_id: Option<String>,
    /// Streamed transient upstream warning captured during the active turn before `end_turn` arrives.
    turn_transient_error: Option<String>,
    /// Number of automatic transient upstream retries already spent in the current visible turn.
    transient_retry_attempts: usize,
    /// Wakes a sleeping transient upstream retry on cancel, new dispatch, or shutdown.
    transient_retry_wait: Option<Arc<RetryWait>>,
    /// Assistant row id holding a split streamed transient warning chunk (`"\n⚠ "` + error text).
    split_transient_row_id: Option<String>,
    /// Pre-prompt SQLite `steps` cursor used to uncover upstream errors hidden behind `end_turn`.
    outcome_watch: Option<PromptOutcomeWatch>,
    /// Quota or rate-limit reset timestamp parsed from the latest Antigravity refusal, for `auto_continue`.
    usage_limit_reset: Option<crate::agent::chat::auto_continue::Reset>,
    /// Set when an upstream or executor error poisoned this `agy acp` process so the next turn rotates it.
    pub(in crate::agent::chat::engine) needs_error_rotate: bool,
    /// Bounded conversation context carried into a fresh `session/new` after `InvalidThinkingSignature`.
    pub(in crate::agent::chat::engine) pending_context_carryover: Option<String>,
    /// Monotonic epoch ms of the latest stream activity or prompt dispatch for inactivity watchdog checks.
    last_prompt_activity_ms: u64,
    /// Wakes the prompt inactivity watchdog when the prompt settles, retries, cancels, or exits.
    inactivity_wait: Option<Arc<RetryWait>>,
    /// Optional per-process inactivity limit override (used by unit tests).
    pub(super) prompt_inactivity_limit_ms: Option<u64>,
    /// Set before restarting for an OAuth 401 so `restore` suppresses the stale 401 chunk during replay.
    pub(in crate::agent::chat::engine) recovering_401: bool,
    /// Suppress replayed `⚠ UNAUTHENTICATED (code 401)` chunks while `session/load` rebuilds history.
    pub(in crate::agent::chat::engine) suppress_replay_401: bool,
    /// Turn continuation queued to fire immediately once a 401-restarted ACP process finishes opening.
    pub(in crate::agent::chat::engine) pending_401_continuation: Option<Pending401Continuation>,
    #[cfg(test)]
    stop_execution_nums: Vec<u64>,
}

impl AcpState {
    pub(in crate::agent::chat::engine) fn with_antigravity_launch(
        auth_gate_permit: Option<crate::agent::antigravity::AuthGatePermit>,
        auth_epoch_at_launch: u64,
    ) -> Self {
        Self {
            auth_gate_permit,
            auth_epoch_at_launch,
            ..Self::default()
        }
    }

    pub(super) fn prompt_active(&self) -> bool {
        self.prompt.is_some()
            || !self.steers.is_empty()
            || self.retrying_prompt
            || self.startup_prompt.is_some()
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
                    state.journal = None;
                    state.mapper = Mapper::for_kind(proc.kind);
                    state.cwd.clone()
                };
                // The recorded conversation is gone for good: stop expecting its id so the replacement the
                // agent opens next passes the identity guard (and still claims its own writer lease).
                *proc.expected_native.lock().unwrap() = None;
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
    let (resume, cwd, images, load_session, resume_session) = {
        let mut state = proc.acp.lock().unwrap();
        state.initialized = true;
        state.auth_logout_supported = capabilities.pointer("/auth/logout").is_some_and(|v| !v.is_null());
        state.load_session = capabilities.get("loadSession").and_then(Value::as_bool).unwrap_or(false);
        state.resume_session = capabilities.pointer("/sessionCapabilities/resume").is_some_and(|resume| !resume.is_null());
        let images = capabilities.pointer("/promptCapabilities/image").and_then(Value::as_bool).unwrap_or(false);
        state.images = images;
        state.images_known = true;
        (state.resume.clone(), state.cwd.clone(), images, state.load_session, state.resume_session)
    };
    emit(app, session_id, json!({"type":"capabilities","images":images}));
    // Antigravity advertises both `loadSession` and `sessionCapabilities.resume`; prefer `session/resume`
    // because VelaTerm already restores the conversation from Antigravity's native SQLite history and
    // `session/load` replays every historical chunk over stdout with a post-replay idle wait.
    let prefer_resume = proc.kind == SessionKind::Antigravity && resume_session;
    match resume {
        // Resuming replays nothing: the rows on screen came from the agent's own recording and stay.
        Some(native) if prefer_resume || (!load_session && resume_session) => {
            let id = proc.request_id("acp_session_resume");
            let _ = proc.write(&wire::session_resume(&id, &native, &cwd));
        }
        Some(native) => {
            if !load_session {
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
    let gate_permit = proc.acp.lock().unwrap().auth_gate_permit.take();
    drop(gate_permit);
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
    if loaded {
        finish_replay(proc);
        // Publish the replayed baseline before `session` wakes a waiting client's first submission.
        super::flush_timeline_now(app, session_id, proc);
    }
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
    let pending_401 = {
        let mut state = proc.acp.lock().unwrap();
        state.suppress_replay_401 = false;
        state.pending_401_continuation.take()
    };
    if let Some(cont) = pending_401 {
        if resume_after_401_restart(app, session_id, proc, cont) {
            proc.ready.store(true, Ordering::Relaxed);
            emit(app, session_id, json!({"type":"session","agentSessionId":native,"model":proc.model.lock().unwrap().clone()}));
            return;
        }
    }
    let startup_prompt = proc.acp.lock().unwrap().startup_prompt.take();
    let opening = !proc.ready.swap(true, Ordering::Relaxed);
    emit(app, session_id, json!({"type":"session","agentSessionId":native,"model":proc.model.lock().unwrap().clone()}));
    if let Some(prompt) = startup_prompt {
        let outcome = validate(proc, "queue", prompt.images.len())
            .and_then(|()| super::claim_native_writer(app, proc))
            .and_then(|()| dispatch(proc, &prompt.text, &prompt.images));
        match outcome {
            Ok(()) => {
                emit_state(app, session_id, AgentState::Working);
                super::antigravity_tasks::ensure_poller(app, session_id, proc);
                start_prompt_watchdog(app, session_id, proc);
                return;
            }
            Err(message) => {
                if proc.timeline.lock().unwrap().remove_row(&prompt.id) {
                    super::flush_timeline_now(app, session_id, proc);
                }
                {
                    let mut turn = proc.turn.lock().unwrap();
                    turn.running = false;
                    turn.started_at = None;
                }
                emit(app, session_id, json!({"type":"unsent","id":prompt.id,"text":prompt.text,"images":prompt.images}));
                emit(app, session_id, json!({"type":"error","message":message}));
                emit(app, session_id, json!({"type":"turnCompleted"}));
                emit_state(app, session_id, AgentState::Waiting);
                start_waiting_message(app, session_id, proc);
                return;
            }
        }
    }
    if opening {
        emit_state(app, session_id, AgentState::Waiting);
        start_waiting_message(app, session_id, proc);
    }
}

/// The replay is complete: its rows replace the recording's, and the journal is rewritten from it.
fn finish_replay(proc: &Arc<ChatProcess>) {
    let (rows, lines, journal, startup_prompt) = {
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
        (replay.rows, lines, state.journal.clone(), state.startup_prompt.clone())
    };
    let mut rows = rows;
    super::super::shell::map_replayed_rows(&mut rows);
    if let Some(prompt) = startup_prompt {
        if rows.iter().all(|row| row.id() != prompt.id) {
            rows.push(super::user_row(prompt.id, &prompt.text, prompt.images, Some(prompt.sent_at as i64)));
        }
    }
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
            model: proc.model.lock().unwrap().clone().filter(|m| !m.trim().is_empty()),
            effort: proc.effort.lock().unwrap().clone().filter(|e| !e.trim().is_empty()),
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
                let fallback = catalog
                    .model
                    .clone()
                    .or_else(|| {
                        (proc.kind == SessionKind::Antigravity)
                            .then(|| catalog.models.first().map(|c| c.id.clone()))
                            .flatten()
                    });
                *proc.model.lock().unwrap() = fallback;
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
        let desired = proc
            .model
            .lock()
            .unwrap()
            .clone()
            .filter(|m| !m.trim().is_empty());
        let target = desired
            .filter(|model| {
                catalog.model.as_deref() != Some(model.as_str())
                    && catalog.models.iter().any(|choice| &choice.id == model)
            })
            .or_else(|| {
                // Antigravity can open a session with an empty `currentValue` ("plan model not specified");
                // proactively select the first available model before any prompt is dispatched.
                (proc.kind == SessionKind::Antigravity && catalog.model.is_none())
                    .then(|| catalog.models.first().map(|choice| choice.id.clone()))
                    .flatten()
            });
        match target {
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
    let (gate_permit, had_401_continuation, startup_prompt) = {
        let mut state = proc.acp.lock().unwrap();
        (
            state.auth_gate_permit.take(),
            state.pending_401_continuation.take().is_some(),
            state.startup_prompt.take(),
        )
    };
    drop(gate_permit);
    if let Some(ref prompt) = startup_prompt {
        if proc.timeline.lock().unwrap().remove_row(&prompt.id) {
            super::flush_timeline_now(app, session_id, proc);
        }
    }
    if had_401_continuation || startup_prompt.is_some() {
        let mut turn = proc.turn.lock().unwrap();
        turn.running = false;
        turn.started_at = None;
        drop(turn);
        emit(app, session_id, json!({"type":"turnCompleted"}));
        emit_state(app, session_id, AgentState::Waiting);
    }
    if let Some(prompt) = startup_prompt {
        emit(app, session_id, json!({"type":"unsent","id":prompt.id,"text":prompt.text,"images":prompt.images}));
    }
    if proc.kind == SessionKind::Antigravity && (wire::auth_required(error) || wire::antigravity_401_error(error)) {
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
    if proc.kind == SessionKind::Antigravity && state.loading.is_none() {
        state.last_prompt_activity_ms = now_ms();
    }
    if proc.kind == SessionKind::Antigravity {
        if kind == "agent_message_chunk" {
            if let Some(text) = wire::update_text(update) {
                if let Some(err_msg) = strip_streamed_401_warning(&text) {
                    if state.loading.is_some() {
                        let settled = state.mapper.finish();
                        let suppress = state.suppress_replay_401;
                        if let Some(replay) = state.loading.as_mut() {
                            if let Some(row) = settled {
                                replay.place(row);
                            }
                            if !suppress {
                                let id = format!("e-acp-replay-{}", replay.rows.len());
                                replay.place(ChatRow::Error { id, message: err_msg });
                            }
                        }
                        return;
                    }
                    state.turn_auth_error = Some(err_msg);
                    return;
                }
                if let Some(err_msg) = strip_streamed_transient_warning(&text) {
                    if state.loading.is_some() {
                        let settled = state.mapper.finish();
                        if let Some(replay) = state.loading.as_mut() {
                            if let Some(row) = settled {
                                replay.place(row);
                            }
                            let id = format!("e-acp-replay-{}", replay.rows.len());
                            replay.place(ChatRow::Error { id, message: err_msg });
                        }
                        return;
                    }
                    state.turn_transient_error = Some(err_msg);
                    return;
                }
            }
        } else if kind == "user_message_chunk" && state.loading.is_some() {
            if let Some(text) = wire::update_text(update) {
                let trimmed = text.trim();
                if trimmed == AUTH_RECOVERY_PROMPT || trimmed == TRANSIENT_RECOVERY_PROMPT {
                    if let Some(row) = state.mapper.finish() {
                        if let Some(replay) = state.loading.as_mut() {
                            replay.place(row);
                        }
                    }
                    if let Some(replay) = state.loading.as_mut() {
                        let pop_err = if trimmed == AUTH_RECOVERY_PROMPT {
                            replay.rows.last().is_some_and(|r| {
                                matches!(r, ChatRow::Error { message, .. } if crate::agent::chat::antigravity_protocol::is_oauth_401_error(message))
                            })
                        } else {
                            replay.rows.last().is_some_and(|r| {
                                matches!(r, ChatRow::Error { message, .. } if is_transient_upstream_error(message))
                            })
                        };
                        if pop_err {
                            replay.rows.pop();
                        }
                    }
                    return;
                }
            }
        }
    }
    let rows = state.mapper.update(update);
    if let Some(replay) = state.loading.as_mut() {
        replay.updates.push((update.clone(), rows.first().map(|row| row.id().to_string())));
        for row in rows { replay.place(row); }
        return;
    }
    let mut suppress_rows = false;
    let mut retract_row_id = None;
    if !rows.is_empty() {
        let mut matched_split_warning = false;
        if proc.kind == SessionKind::Antigravity && rows.len() == 1 {
            if let ChatRow::Assistant { id, text, .. } = &rows[0] {
                if let Some(msg) = strip_streamed_401_warning(text) {
                    state.turn_auth_error = Some(msg);
                    state.split_auth_row_id = Some(id.clone());
                    suppress_rows = true;
                    retract_row_id = Some(id.clone());
                    matched_split_warning = true;
                } else if let Some(msg) = strip_streamed_transient_warning(text) {
                    state.turn_transient_error = Some(msg);
                    state.split_transient_row_id = Some(id.clone());
                    suppress_rows = true;
                    retract_row_id = Some(id.clone());
                    matched_split_warning = true;
                } else {
                    if state.split_auth_row_id.as_deref() == Some(id.as_str()) {
                        state.split_auth_row_id = None;
                        state.turn_auth_error = None;
                    }
                    if state.split_transient_row_id.as_deref() == Some(id.as_str()) {
                        state.split_transient_row_id = None;
                        state.turn_transient_error = None;
                    }
                }
            }
        }
        if !matched_split_warning {
            if state.split_auth_row_id.is_some() {
                state.split_auth_row_id = None;
                state.turn_auth_error = None;
            }
            if state.turn_transient_error.is_some() {
                state.turn_transient_error = None;
                state.split_transient_row_id = None;
            }
        }
        if !matched_split_warning {
            if let Some(active) = state.active_prompt.as_mut() {
                if proc.kind == SessionKind::Antigravity
                    && !active.had_other_output
                    && rows.len() == 1
                {
                    if let ChatRow::Assistant { id, text, .. } = &rows[0] {
                        if active.sole_assistant_id.as_deref().is_none_or(|prev| prev == id) {
                            let previously_streamed = active.sole_assistant_id.as_deref() == Some(id.as_str())
                                && active.chunk_cold_error.is_none();
                            active.sole_assistant_id = Some(id.clone());
                            if let Some(msg) = wire::antigravity_cold_model_chunk_error(text) {
                                active.chunk_cold_error = Some(msg);
                                active.had_output = false;
                                suppress_rows = true;
                                if previously_streamed {
                                    retract_row_id = Some(id.clone());
                                }
                            } else {
                                active.chunk_cold_error = None;
                                active.had_output = true;
                            }
                        } else {
                            active.had_other_output = true;
                            active.had_output = true;
                            active.chunk_cold_error = None;
                        }
                    } else {
                        active.had_other_output = true;
                        active.had_output = true;
                        active.chunk_cold_error = None;
                    }
                } else {
                    active.had_other_output = true;
                    active.had_output = true;
                    active.chunk_cold_error = None;
                }
            }
        }
    }
    if state.journal.is_some() {
        state.pending_journal.push(wire::journal_line(update, now_ms() as i64));
        if state.pending_journal.len() >= JOURNAL_BATCH { flush_journal_locked(&mut state); }
    }
    drop(state);
    if let Some(row_id) = retract_row_id {
        proc.timeline.lock().unwrap().remove_row(&row_id);
    }
    if !suppress_rows {
        let model = proc.model.lock().unwrap().as_deref().map(|model| wire::reply_model(proc.kind, model));
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
    if proc.kind == SessionKind::Antigravity {
        proc.acp.lock().unwrap().last_prompt_activity_ms = now_ms();
    }
    match method {
        "session/request_permission" => {
            if proc.mode.lock().unwrap().as_str() == "bypassPermissions"
                && !wire::is_antigravity_interaction_question(proc.kind, &params)
            {
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
            let stored_card = proc.permissions.lock().unwrap().get(card).cloned();
            let is_interaction_question = stored_card
                .as_ref()
                .and_then(|c| c.pointer("/input/_acpInteractionQuestion"))
                .and_then(Value::as_bool)
                == Some(true);
            let option = if is_interaction_question {
                stored_card
                    .as_ref()
                    .and_then(|c| wire::interaction_question_option(c, &params, allow, updated_input))
            } else {
                let always = updated_permissions.and_then(Value::as_array).into_iter().flatten()
                    .find(|value| value.get("type").and_then(Value::as_str) == Some("acpAllowAlways"))
                    .and_then(|value| value.get("optionId").and_then(Value::as_str));
                wire::choose_option(params.get("options"), allow, always)
            };
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
                if proc.kind == SessionKind::Antigravity {
                    super::antigravity_tasks::ensure_poller(app, session_id, proc);
                    start_prompt_watchdog(app, session_id, proc);
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
            PendingRequest::Permission { id, params }
                if !wire::is_antigravity_interaction_question(proc.kind, params) =>
            {
                Some((card.clone(), id.clone(), params.clone()))
            }
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
    let stop_reason = result
        .as_ref()
        .and_then(|res| res.get("stopReason"))
        .and_then(Value::as_str)
        .unwrap_or("end_turn");
    let mut cold_retry = None;
    let mut transient_retry = None;
    let mut suppressed_chunk_error = None;
    let mut db_usage_limit: Option<(String, Option<crate::agent::chat::auto_continue::Reset>)> = None;
    let mut db_broken_signature: Option<String> = None;
    let mut db_provider_error: Option<String> = None;
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
        let now_secs = Some((now_ms() / 1_000) as i64);
        let hidden_outcome = if proc.kind == SessionKind::Antigravity
            && !canceled
            && !was_settled_while_parked
            && state.steers.is_empty()
            && error.is_none()
        {
            state
                .outcome_watch
                .as_ref()
                .and_then(|watch| watch.check(stop_reason, now_secs))
        } else {
            None
        };
        if let Some(HiddenTurnFailure::Auth401 { ref message }) = hidden_outcome {
            if state.turn_auth_error.is_none() {
                state.turn_auth_error = Some(message.clone());
            }
        }
        // Antigravity can answer `session/resume` or `session/new` before `FetchAvailableModels`
        // finishes populating its in-memory model map, rejecting the first `session/prompt` with
        // `failed to construct executor: unknown model key MODEL_PLACEHOLDER_...` or `plan model
        // not specified` — delivered either as a JSON-RPC error, a `⚠ ...` assistant chunk, or a
        // native `ERROR_MESSAGE` step followed by `stopReason: "end_turn"`. Retry transparently.
        if proc.kind == SessionKind::Antigravity
            && !canceled
            && !was_settled_while_parked
            && state.steers.is_empty()
            && state.requests.is_empty()
            && proc.alive.load(Ordering::Relaxed)
            && !proc.released.load(Ordering::Relaxed)
        {
            let cold_error = error
                .as_ref()
                .filter(|err| wire::antigravity_model_not_ready(err))
                .map(|err| (wire::error_text(err), wire::antigravity_plan_model_missing(err)))
                .or_else(|| {
                    state.active_prompt.as_ref().and_then(|active| {
                        (!active.had_output)
                            .then(|| active.chunk_cold_error.clone())
                            .flatten()
                            .map(|msg| {
                                let missing = wire::antigravity_plan_model_missing_text(&msg);
                                (msg, missing)
                            })
                    })
                })
                .or_else(|| match &hidden_outcome {
                    Some(HiddenTurnFailure::ColdModelNotReady { message, plan_model_missing }) => {
                        Some((message.clone(), *plan_model_missing))
                    }
                    _ => None,
                });
            if let Some((reason, plan_model_missing)) = cold_error {
                let delays = cold_model_retry_delays();
                if let Some(active) = state.active_prompt.as_mut() {
                    if !active.had_output && active.attempts < delays.len() {
                        let delay = delays[active.attempts];
                        active.attempts += 1;
                        active.had_other_output = false;
                        active.sole_assistant_id = None;
                        active.chunk_cold_error = None;
                        let attempt = active.attempts;
                        let payload = (active.text.clone(), active.images.clone());
                        let wait = Arc::new(RetryWait::default());
                        if let Some(prev) = state.model_retry_wait.replace(wait.clone()) {
                            prev.wake();
                        }
                        if let Some(inact) = state.inactivity_wait.take() {
                            inact.wake();
                        }
                        state.retrying_prompt = true;
                        let _ = state.mapper.finish();
                        cold_retry = Some((
                            attempt,
                            delay,
                            payload.0,
                            payload.1,
                            wait,
                            plan_model_missing,
                            reason,
                        ));
                    }
                }
            }
        }
        // Antigravity can also abort mid-turn on a transient Google CloudCode or network error
        // (e.g. HTTP 404 after model output, 500/502/503/504, `There was a network issue...`, or
        // stream EOF/connection reset), emitting a `⚠ ...` assistant chunk or a hidden DB error
        // step followed by `stopReason: "end_turn"`. Retry on the same process with backoff.
        if cold_retry.is_none()
            && proc.kind == SessionKind::Antigravity
            && !canceled
            && !was_settled_while_parked
            && state.steers.is_empty()
            && state.requests.is_empty()
            && state.turn_auth_error.is_none()
            && proc.alive.load(Ordering::Relaxed)
            && !proc.released.load(Ordering::Relaxed)
        {
            let transient_error = error
                .as_ref()
                .filter(|err| wire::antigravity_transient_error(err))
                .map(|err| (wire::error_text(err), true))
                .or_else(|| state.turn_transient_error.clone().map(|msg| (msg, false)))
                .or_else(|| match &hidden_outcome {
                    Some(HiddenTurnFailure::TransientUpstream { message }) => {
                        Some((message.clone(), false))
                    }
                    _ => None,
                });
            if let Some((reason, rpc_rejected_before_record)) = transient_error {
                let delays = transient_upstream_retry_delays();
                if state.transient_retry_attempts < delays.len() {
                    let delay = delays[state.transient_retry_attempts];
                    state.transient_retry_attempts += 1;
                    let attempt = state.transient_retry_attempts;
                    state.turn_transient_error = None;
                    let split_row_id = state.split_transient_row_id.take();
                    let (retry_text, retry_images) = if rpc_rejected_before_record {
                        state
                            .active_prompt
                            .as_ref()
                            .filter(|active| !active.had_output)
                            .map(|active| (active.text.clone(), active.images.clone()))
                            .unwrap_or_else(|| (TRANSIENT_RECOVERY_PROMPT.to_string(), Vec::new()))
                    } else {
                        (TRANSIENT_RECOVERY_PROMPT.to_string(), Vec::new())
                    };
                    let pre_rows: Vec<ChatRow> = state
                        .mapper
                        .finish()
                        .into_iter()
                        .filter(|row| split_row_id.as_deref() != Some(row.id()))
                        .collect();
                    let wait = Arc::new(RetryWait::default());
                    if let Some(prev) = state.transient_retry_wait.replace(wait.clone()) {
                        prev.wake();
                    }
                    if let Some(inact) = state.inactivity_wait.take() {
                        inact.wake();
                    }
                    state.retrying_prompt = true;
                    transient_retry = Some((
                        attempt,
                        delay,
                        retry_text,
                        retry_images,
                        wait,
                        reason,
                        pre_rows,
                    ));
                }
            }
        }
    if cold_retry.is_some() || transient_retry.is_some() {
        (false, None, None, None)
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
        state.outcome_watch = None;
        if let Some(inact) = state.inactivity_wait.take() {
            inact.wake();
        }
        let finished_prompt = state.active_prompt.take();
        let saved_prompt_for_401 = finished_prompt.as_ref().map(|p| (p.text.clone(), p.images.clone()));
        let streamed_auth_error = state.turn_auth_error.take();
        let split_auth_row_id = state.split_auth_row_id.take();
        let mut streamed_transient_error = state.turn_transient_error.take();
        let split_transient_row_id = state.split_transient_row_id.take();
        state.transient_retry_attempts = 0;
        let suppressed_row_id = finished_prompt.as_ref().and_then(|active| {
            active.chunk_cold_error.as_ref().and_then(|_| active.sole_assistant_id.clone())
        });
        suppressed_chunk_error = finished_prompt.and_then(|active| active.chunk_cold_error);
        match hidden_outcome {
            Some(HiddenTurnFailure::UsageLimit { message, reset }) => {
                db_usage_limit = Some((message, reset));
            }
            Some(HiddenTurnFailure::InvalidThinkingSignature { message }) => {
                db_broken_signature = Some(message);
            }
            Some(HiddenTurnFailure::ProviderError { message }) => {
                db_provider_error = Some(message);
            }
            Some(HiddenTurnFailure::TransientUpstream { message }) => {
                if streamed_transient_error.is_none() {
                    streamed_transient_error = Some(message);
                }
            }
            Some(HiddenTurnFailure::ColdModelNotReady { message, .. }) => {
                if suppressed_chunk_error.is_none() {
                    suppressed_chunk_error = Some(message);
                }
            }
            Some(HiddenTurnFailure::Auth401 { .. }) | None => {}
        }
        state.retrying_prompt = false;
        if let Some(wait) = state.model_retry_wait.take() {
            wait.wake();
        }
        if let Some(wait) = state.transient_retry_wait.take() {
            wait.wake();
        }
        state.settled_while_parked = false;
        let stop_park = state.stop_park.take();
        let rows: Vec<ChatRow> = state
            .mapper
            .finish()
            .into_iter()
            .filter(|row| {
                suppressed_row_id.as_deref() != Some(row.id())
                    && split_auth_row_id.as_deref() != Some(row.id())
                    && split_transient_row_id.as_deref() != Some(row.id())
            })
            .chain(state.mapper.settle_running(canceled))
            .collect();
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
        (was_settled_while_parked, streamed_auth_error, streamed_transient_error, saved_prompt_for_401)
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
    if let Some((attempt, delay, text, images, wait, reason, pre_rows)) = transient_retry {
        if !pre_rows.is_empty() {
            let mut timeline = proc.timeline.lock().unwrap();
            for row in pre_rows { timeline.upsert(row); }
        }
        crate::diagnostics::record("WARN", "agent_chat", json!({
            "sessionId": session_id,
            "event": "antigravity_transient_retry",
            "attempt": attempt,
            "delayMs": delay.as_millis() as u64,
            "reason": reason,
        }));
        spawn_transient_retry(app.clone(), session_id.to_string(), proc.clone(), text, images, delay, wait);
        return;
    }
    let (settled_while_parked, streamed_auth_error, streamed_transient_error, saved_prompt_for_401) = settled_while_parked;
    cancel_pending(app, session_id, proc);
    if proc.kind == SessionKind::Antigravity && !canceled {
        let auth_401 = streamed_auth_error
            .as_deref()
            .map(|msg| (msg.to_string(), false))
            .or_else(|| {
                error
                    .as_ref()
                    .filter(|err| wire::antigravity_401_error(err))
                    .map(|err| (wire::error_text(err), true))
            });
        if let Some((reason, rpc_rejected_before_record)) = auth_401 {
            if trigger_antigravity_401_recovery(
                app,
                session_id,
                proc,
                &reason,
                saved_prompt_for_401,
                rpc_rejected_before_record,
                started_at,
            ) {
                return;
            }
            let turn_num = proc.acp.lock().unwrap().mapper.turn();
            proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                id: format!("e-acp-{turn_num}"),
                message: reason,
            });
            super::antigravity_tasks::before_turn_end(app, session_id, proc);
            handle_turn_end(app, session_id, proc, "success", None);
            evict_or_require_auth_after_401(app.clone(), session_id.to_string(), proc.clone());
            return;
        }
        if let Some(err) = error.as_ref() {
            let err_text = wire::error_text(err);
            if db_broken_signature.is_none() && is_invalid_thinking_signature(&err_text) {
                db_broken_signature = Some(
                    "Antigravity's saved conversation can no longer be sent to the model. Your next message will automatically start a fresh Antigravity session with this conversation's context.".to_string(),
                );
            } else if db_usage_limit.is_none() && is_usage_limit_error(&err_text) {
                let reset = parse_usage_limit_reset(&err_text, Some((now_ms() / 1_000) as i64));
                db_usage_limit = Some((err_text, reset));
            }
        }
        if let Some(ref msg) = db_broken_signature {
            retire_broken_antigravity_conversation(app, session_id, proc);
            let turn_num = proc.acp.lock().unwrap().mapper.turn();
            proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                id: format!("e-acp-{turn_num}"),
                message: msg.clone(),
            });
            super::antigravity_tasks::before_turn_end(app, session_id, proc);
            handle_turn_end(app, session_id, proc, "success", None);
            return;
        }
        if let Some((msg, reset)) = db_usage_limit {
            proc.acp.lock().unwrap().usage_limit_reset = reset;
            proc.acp.lock().unwrap().needs_error_rotate = true;
            proc.extras.lock().unwrap().usage_limit_hit = true;
            let turn_num = proc.acp.lock().unwrap().mapper.turn();
            proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                id: format!("e-acp-{turn_num}"),
                message: msg,
            });
            super::antigravity_tasks::before_turn_end(app, session_id, proc);
            handle_turn_end(app, session_id, proc, "success", None);
            return;
        }
    }
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
            db_provider_error
                .or_else(|| wire::stop_reason_error(stop_reason).map(str::to_string))
                .or(suppressed_chunk_error)
                .or(streamed_transient_error)
        }
    };
    if error.is_none() && failure.is_none() {
        super::auth::turn_succeeded(app, session_id, proc);
    } else if proc.kind == SessionKind::Antigravity && !canceled {
        // Mark the process for idle replacement before the next turn so broken upstream HTTP
        // streams, failed model maps, or stale cached credentials do not poison later prompts.
        proc.acp.lock().unwrap().needs_error_rotate = true;
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
            state.last_prompt_activity_ms = now_ms();
            state.outcome_watch = Some(PromptOutcomeWatch::capture(&native, &proc.auth_settings_args));
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
                state.outcome_watch = None;
                state.mapper.turn()
            };
            proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                id: format!("e-acp-{turn_num}"),
                message: error,
            });
            handle_turn_end(&app, &session_id, &proc, "success", None);
        } else {
            start_prompt_watchdog(&app, &session_id, &proc);
        }
    });
}

/// Retry an Antigravity turn interrupted by a transient upstream API or transport error after a
/// short backoff without duplicating the user row or ending the visible turn.
fn spawn_transient_retry(
    app: AppCtx,
    session_id: String,
    proc: Arc<ChatProcess>,
    text: String,
    images: Vec<ChatImage>,
    delay: Duration,
    wait: Arc<RetryWait>,
) {
    std::thread::spawn(move || {
        wait.wait_for(delay);
        if !proc.alive.load(Ordering::Relaxed) || proc.released.load(Ordering::Relaxed) {
            return;
        }
        let Some(native) = proc.agent_session_id.lock().unwrap().clone() else {
            return;
        };
        // Follow the established `turn -> acp` lock order used by `interrupt_locked`.
        let next_id = {
            let turn = proc.turn.lock().unwrap();
            if !turn.running
                || turn.interrupted
                || !proc.alive.load(Ordering::Relaxed)
                || proc.released.load(Ordering::Relaxed)
            {
                let mut state = proc.acp.lock().unwrap();
                if state.transient_retry_wait.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &wait)) {
                    state.retrying_prompt = false;
                    state.transient_retry_wait = None;
                    state.active_prompt = None;
                    state.transient_retry_attempts = 0;
                    state.turn_transient_error = None;
                    state.split_transient_row_id = None;
                }
                return;
            }
            let mut state = proc.acp.lock().unwrap();
            if !state.retrying_prompt
                || !state.transient_retry_wait.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &wait))
            {
                return;
            }
            let id = proc.request_id("prompt");
            state.retrying_prompt = false;
            state.transient_retry_wait = None;
            state.turn_transient_error = None;
            state.split_transient_row_id = None;
            state.last_prompt_activity_ms = now_ms();
            state.outcome_watch = Some(PromptOutcomeWatch::capture(&native, &proc.auth_settings_args));
            state.mapper.begin_turn();
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
                state.outcome_watch = None;
                state.transient_retry_attempts = 0;
                state.mapper.turn()
            };
            proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                id: format!("e-acp-{turn_num}"),
                message: error,
            });
            handle_turn_end(&app, &session_id, &proc, "success", None);
        } else {
            start_prompt_watchdog(&app, &session_id, &proc);
        }
    });
}

/// If a cold-started Antigravity session failed with `plan model not specified`, try pushing the
/// conversation's saved model (or available fallback models) over `session/set_config_option` or
/// `session/set_model` before retrying the prompt.
fn try_sync_antigravity_plan_model(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let saved_model = proc
        .model
        .lock()
        .unwrap()
        .clone()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty());
    let Some(native) = proc.agent_session_id.lock().unwrap().clone() else { return; };
    let (option, models_from_state, catalog_current, catalog_choices) = {
        let state = proc.acp.lock().unwrap();
        let option = state.catalog.model_option.clone().unwrap_or_else(|| "model".to_string());
        let current = state
            .catalog
            .model
            .clone()
            .filter(|m| !m.trim().is_empty());
        let choices: Vec<String> = state
            .catalog
            .models
            .iter()
            .map(|c| c.id.trim().to_string())
            .filter(|m| !m.is_empty())
            .take(2)
            .collect();
        (option, state.catalog.models_from_state, current, choices)
    };
    let mut candidates: Vec<String> = Vec::new();
    let mut push_unique = |candidate: Option<String>| {
        if let Some(m) = candidate.filter(|s| !s.trim().is_empty()) {
            if !candidates.iter().any(|existing| existing == &m) {
                candidates.push(m);
            }
        }
    };
    if let Some(ref saved) = saved_model {
        push_unique(Some(saved.clone()));
        // Only fall back past an explicit saved model when the live ACP session has already
        // advertised real model choices (for instance, when the saved model ID was retired).
        if !catalog_choices.is_empty() {
            push_unique(catalog_current);
            push_unique(catalog_choices.into_iter().next());
        }
    } else {
        push_unique(catalog_current);
        if let Some(first_live) = catalog_choices.into_iter().next() {
            push_unique(Some(first_live));
        } else {
            push_unique(crate::agent::antigravity_models::cached(app).into_iter().next().map(|m| m.id));
        }
    }
    if candidates.is_empty() {
        return;
    }

    let is_method_not_found = |err: &str| {
        let lower = err.to_ascii_lowercase();
        lower.contains("method not found") || lower.contains("unknown method") || lower.contains("-32601")
    };
    let mut applied: Option<(String, Value)> = None;
    for model in candidates {
        let primary = if models_from_state {
            proc.request_and_wait("acp_set_model", |id| wire::set_model(id, &native, &model))
                .or_else(|err| {
                    if is_method_not_found(&err) {
                        proc.request_and_wait("acp_set_model", |id| {
                            wire::set_config_option(id, &native, &option, &model)
                        })
                    } else {
                        Err(err)
                    }
                })
        } else {
            proc.request_and_wait("acp_set_model", |id| {
                wire::set_config_option(id, &native, &option, &model)
            })
            .or_else(|err| {
                if is_method_not_found(&err) {
                    proc.request_and_wait("acp_set_model", |id| wire::set_model(id, &native, &model))
                } else {
                    Err(err)
                }
            })
        };
        if let Ok(result) = primary {
            applied = Some((model, result));
            break;
        }
    }
    let Some((model, result)) = applied else {
        return;
    };
    let catalog = {
        let mut state = proc.acp.lock().unwrap();
        state.awaiting_initial_models = false;
        state.catalog.model = Some(model.clone());
        if let Some(options) = result.get("configOptions").and_then(Value::as_array) {
            state.catalog.absorb_options(options);
        }
        state.catalog.clone()
    };
    if saved_model.as_deref() != Some(model.as_str()) {
        *proc.model.lock().unwrap() = catalog.model.clone().or(Some(model));
    }
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
    // Only park the Stop hook when background tasks, timers, subagents, or unread deliveries exist;
    // ordinary completed turns allow Stop immediately so the next user message sends with zero wait.
    let has_turn_tasks = {
        let turn = proc.turn.lock().unwrap();
        !turn.active_tasks.is_empty() || !turn.background_tasks.is_empty()
    };
    if !has_turn_tasks
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
        let has_turn_tasks = {
            let turn = proc.turn.lock().unwrap();
            !turn.active_tasks.is_empty() || !turn.background_tasks.is_empty()
        };
        if !has_turn_tasks
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

pub(super) fn has_background_activity(proc: &Arc<ChatProcess>) -> bool {
    super::antigravity_tasks::has_background_activity(proc)
}

/// Handle a headless reactive Antigravity wakeup that wrote an OAuth 401 `ERROR_MESSAGE` to transcript.
pub(super) fn handle_reactive_401(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    step_index: Option<u64>,
    message: &str,
) {
    let started_at = proc.turn.lock().unwrap().started_at;
    if trigger_antigravity_401_recovery(app, session_id, proc, message, None, false, started_at) {
        return;
    }
    let id = step_index
        .map(crate::agent::chat::antigravity_protocol::step_id)
        .unwrap_or_else(|| format!("e-acp-{}", proc.acp.lock().unwrap().mapper.turn()));
    proc.timeline.lock().unwrap().upsert(ChatRow::Error {
        id,
        message: message.to_string(),
    });
    handle_turn_end(app, session_id, proc, "success", None);
    evict_or_require_auth_after_401(app.clone(), session_id.to_string(), proc.clone());
}

/// Dispatch the continuation saved across an automatic Antigravity 401 process restart.
fn resume_after_401_restart(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    cont: Pending401Continuation,
) -> bool {
    if cont.ensure_user_row {
        let mut timeline = proc.timeline.lock().unwrap();
        let already_present = timeline.rows.last().is_some_and(|row| {
            matches!(row, ChatRow::User { text, .. } if text == &cont.text)
        });
        if !already_present {
            let id = format!("u-401-{}", now_ms());
            timeline.upsert(super::user_row(
                id,
                &cont.text,
                cont.images.clone(),
                Some(now_ms() as i64),
            ));
        }
    }
    let started_at = {
        let mut turn = proc.turn.lock().unwrap();
        if turn.interrupted {
            return false;
        }
        turn.running = true;
        turn.interrupted = false;
        let ts = cont.started_at.unwrap_or_else(now_ms);
        turn.started_at = Some(ts);
        ts
    };
    emit(app, session_id, json!({"type":"turnStarted","startedAt":started_at}));
    emit_state(app, session_id, AgentState::Working);
    if let Err(error) = dispatch(proc, &cont.text, &cont.images) {
        let turn_num = proc.acp.lock().unwrap().mapper.turn();
        proc.timeline.lock().unwrap().upsert(ChatRow::Error {
            id: format!("e-acp-{turn_num}"),
            message: error,
        });
        handle_turn_end(app, session_id, proc, "success", None);
        return true;
    }
    super::antigravity_tasks::ensure_poller(app, session_id, proc);
    start_prompt_watchdog(app, session_id, proc);
    true
}

/// Take the reset timestamp recorded when Antigravity hit a quota or rate limit on this turn.
pub(super) fn take_usage_limit_reset(
    proc: &ChatProcess,
) -> Option<crate::agent::chat::auto_continue::Reset> {
    proc.acp.lock().unwrap().usage_limit_reset.take()
}

/// Unbind a native Antigravity session whose persisted history contains an invalid Claude thinking
/// signature, while retaining bounded recent context so the next turn seamlessly opens a fresh
/// native conversation without losing the visible thread history.
fn retire_broken_antigravity_conversation(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let carryover = build_carryover_context(&proc.timeline.lock().unwrap().rows);
    let old_native = proc.agent_session_id.lock().unwrap().take();
    *proc.expected_native.lock().unwrap() = None;
    {
        let mut state = proc.acp.lock().unwrap();
        state.resume = None;
        state.pending_context_carryover = carryover;
        state.needs_error_rotate = true;
    }
    if let Some(old_native) = old_native {
        let changed = crate::db::repo::forget_agent_session_id(
            &app.db().conn.lock().unwrap(),
            session_id,
            &old_native,
            SessionKind::Antigravity,
        )
        .unwrap_or(false);
        if changed {
            app.emit(crate::host::TREE_CHANGED, ());
        }
    }
    crate::diagnostics::record("WARN", "agent_chat", json!({
        "sessionId": session_id,
        "event": "antigravity_broken_thinking_signature_retired",
    }));
}

fn build_carryover_context(rows: &[ChatRow]) -> Option<String> {
    const MAX_EXCHANGES: usize = 8;
    const MAX_CHARS: usize = 12_000;
    let relevant: Vec<(&'static str, &str)> = rows
        .iter()
        .filter_map(|row| match row {
            ChatRow::User { text, .. } if !text.trim().is_empty() => Some(("User", text.trim())),
            ChatRow::Assistant { text, .. } if !text.trim().is_empty() => Some(("Assistant", text.trim())),
            _ => None,
        })
        .collect();
    if relevant.is_empty() {
        return None;
    }
    let start = relevant.len().saturating_sub(MAX_EXCHANGES);
    let mut parts = Vec::new();
    let mut used = 0usize;
    for &(role, text) in &relevant[start..] {
        let clipped: String = text.chars().take(2_500).collect();
        let entry = format!("{role}: {clipped}");
        if used + entry.len() > MAX_CHARS && !parts.is_empty() {
            break;
        }
        used += entry.len();
        parts.push(entry);
    }
    (!parts.is_empty()).then(|| {
        format!(
            "Prior conversation context from this thread (the previous Antigravity session could not be resumed):\n\n{}",
            parts.join("\n\n")
        )
    })
}

/// Start a slice-based inactivity watchdog for an active Antigravity `session/prompt`.
///
/// Long-running tool calls, open permission/question cards, background tasks, parked `Stop` hooks,
/// and ongoing writes to Antigravity's native conversation DB keep the timer alive; genuine
/// silence past the threshold replaces the wedged `agy acp` process.
pub(super) fn start_prompt_watchdog(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    if proc.kind != SessionKind::Antigravity {
        return;
    }
    let (wait, limit_ms, initial_watch) = {
        let mut state = proc.acp.lock().unwrap();
        if state.prompt.is_none() {
            return;
        }
        let limit_ms = state.prompt_inactivity_limit_ms.unwrap_or(DEFAULT_PROMPT_INACTIVITY_MS);
        state.last_prompt_activity_ms = now_ms();
        let wait = Arc::new(RetryWait::default());
        if let Some(prev) = state.inactivity_wait.replace(wait.clone()) {
            prev.wake();
        }
        (wait, limit_ms, state.outcome_watch.clone())
    };
    let slice = Duration::from_millis((limit_ms / 3).clamp(15, 5_000));
    let (app, session_id, proc) = (app.clone(), session_id.to_string(), proc.clone());
    std::thread::spawn(move || {
        let mut last_watch_path = initial_watch
            .as_ref()
            .and_then(|w| w.db_path().map(PathBuf::from));
        let mut last_db_activity = initial_watch
            .as_ref()
            .and_then(PromptOutcomeWatch::activity_fingerprint);
        loop {
            wait.wait_for(slice);
            if !proc.alive.load(Ordering::Relaxed) || proc.released.load(Ordering::Relaxed) {
                return;
            }
            let has_open_permissions = !proc.permissions.lock().unwrap().is_empty();
            let (turn_running, turn_interrupted, started_at, has_bg_tasks) = {
                let turn = proc.turn.lock().unwrap();
                (
                    turn.running,
                    turn.interrupted,
                    turn.started_at,
                    !turn.background_tasks.is_empty() || !turn.active_tasks.is_empty(),
                )
            };
            if !turn_running || turn_interrupted {
                return;
            }
            let (watch, paused) = {
                let mut state = proc.acp.lock().unwrap();
                if !state.inactivity_wait.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &wait))
                    || state.prompt.is_none()
                    || state.retrying_prompt
                    || state.parked_idle
                {
                    return;
                }
                let paused = has_open_permissions
                    || has_bg_tasks
                    || !state.requests.is_empty()
                    || state.mapper.has_running_tool()
                    || state.tasks.reactive_active();
                if paused {
                    state.last_prompt_activity_ms = now_ms();
                    (None, true)
                } else {
                    (state.outcome_watch.clone(), false)
                }
            };
            if paused {
                continue;
            }

            // Probe Antigravity's native conversation DB outside the `acp` lock so ongoing
            // generation (which flushes into `steps` / `-wal` without emitting ACP updates)
            // keeps the turn alive.
            if let Some(ref w) = watch {
                let cur_path = w.db_path().map(PathBuf::from);
                if cur_path != last_watch_path {
                    last_watch_path = cur_path;
                    last_db_activity = w.activity_fingerprint();
                } else if let Some(sample) = w.activity_fingerprint() {
                    let progressed = match last_db_activity.as_mut() {
                        Some(prev) => prev.advance_from(sample),
                        None => {
                            last_db_activity = Some(sample);
                            true
                        }
                    };
                    if progressed {
                        let mut state = proc.acp.lock().unwrap();
                        if state.inactivity_wait.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &wait)) {
                            state.last_prompt_activity_ms = now_ms();
                        }
                        continue;
                    }
                }
            }

            let stalled = {
                let mut state = proc.acp.lock().unwrap();
                if !state.inactivity_wait.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &wait))
                    || state.prompt.is_none()
                    || state.retrying_prompt
                    || state.parked_idle
                {
                    return;
                }
                let recheck_paused = !proc.permissions.lock().unwrap().is_empty()
                    || !state.requests.is_empty()
                    || state.mapper.has_running_tool()
                    || state.tasks.reactive_active();
                if recheck_paused {
                    state.last_prompt_activity_ms = now_ms();
                    None
                } else {
                    let elapsed = now_ms().saturating_sub(state.last_prompt_activity_ms);
                    if elapsed >= limit_ms {
                        state.inactivity_wait = None;
                        let saved = state
                            .active_prompt
                            .as_ref()
                            .map(|p| (p.text.clone(), p.images.clone(), p.had_output));
                        Some((saved, elapsed))
                    } else {
                        None
                    }
                }
            };
            if let Some((saved, elapsed_ms)) = stalled {
                trigger_antigravity_stall_recovery(
                    &app,
                    &session_id,
                    &proc,
                    saved,
                    started_at,
                    elapsed_ms,
                    limit_ms,
                );
                return;
            }
        }
    });
}

fn trigger_antigravity_stall_recovery(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    saved_prompt: Option<(String, Vec<ChatImage>, bool)>,
    started_at: Option<u64>,
    elapsed_ms: u64,
    threshold_ms: u64,
) {
    if let Ok(native) = session(proc) {
        let _ = proc.write(&wire::cancel(&native));
    }
    if let Some(cancel_token) = app.chat().claim_antigravity_stall_recovery(session_id) {
        let (text, images) = match saved_prompt {
            Some((text, images, false)) => (text, images),
            _ => (TRANSIENT_RECOVERY_PROMPT.to_string(), Vec::new()),
        };
        let continuation = Pending401Continuation {
            text,
            images,
            ensure_user_row: false,
            started_at,
        };
        crate::diagnostics::record("WARN", "antigravity_prompt_inactivity", json!({
            "sessionId": session_id,
            "status": "recovered",
            "errorCode": "timeout",
            "elapsedMs": elapsed_ms,
            "thresholdMs": threshold_ms,
        }));
        let (app, session_id, proc) = (app.clone(), session_id.to_string(), proc.clone());
        std::thread::spawn(move || {
            if cancel_token.load(Ordering::Relaxed) {
                return;
            }
            let (model, effort) = {
                let _action = proc.action.lock().unwrap();
                let _callbacks = proc.callbacks.read().unwrap();
                if !app.chat().owns_process(&session_id, &proc)
                    || cancel_token.load(Ordering::Relaxed)
                {
                    return;
                }
                let model = proc.model.lock().unwrap().clone();
                let effort = proc.effort.lock().unwrap().clone();
                {
                    let mut state = proc.acp.lock().unwrap();
                    state.pending_401_continuation = Some(continuation);
                }
                proc.auth_restart.store(true, Ordering::Relaxed);
                super::release_process(&app, &session_id, &proc);
                (model, effort)
            };
            if cancel_token.load(Ordering::Relaxed) {
                proc.acp.lock().unwrap().pending_401_continuation = None;
                return;
            }
            if let Err(_err) = crate::command_core::chat_start(
                &app,
                &session_id,
                model.as_deref(),
                effort.as_deref(),
                false,
            ) {
                proc.acp.lock().unwrap().pending_401_continuation = None;
                proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                    id: format!("e-acp-stall-{}", now_ms()),
                    message: "Antigravity did not respond for too long, and automatic recovery could not restart it. Please try again.".into(),
                });
                emit(&app, &session_id, json!({"type":"turnCompleted"}));
                emit_state(&app, &session_id, AgentState::Waiting);
            }
        });
        return;
    }

    crate::diagnostics::record("WARN", "antigravity_prompt_inactivity", json!({
        "sessionId": session_id,
        "status": "failed",
        "errorCode": "timeout",
        "elapsedMs": elapsed_ms,
        "thresholdMs": threshold_ms,
    }));
    let (rows, turn_num) = {
        let mut state = proc.acp.lock().unwrap();
        state.prompt = None;
        state.steers.clear();
        state.active_prompt = None;
        state.outcome_watch = None;
        state.needs_error_rotate = true;
        let rows: Vec<ChatRow> = state
            .mapper
            .finish()
            .into_iter()
            .chain(state.mapper.settle_running(false))
            .collect();
        (rows, state.mapper.turn())
    };
    {
        let mut timeline = proc.timeline.lock().unwrap();
        for row in rows {
            timeline.upsert(row);
        }
        timeline.upsert(ChatRow::Error {
            id: format!("e-acp-{turn_num}"),
            message: "Antigravity stopped sending updates for too long, so this turn was stopped. Please try again.".into(),
        });
    }
    cancel_pending(app, session_id, proc);
    super::antigravity_tasks::before_turn_end(app, session_id, proc);
    handle_turn_end(app, session_id, proc, "success", None);
}

/// Claim a one-shot automatic process restart to recover an Antigravity turn interrupted by HTTP 401.
fn trigger_antigravity_401_recovery(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    reason: &str,
    saved_prompt: Option<(String, Vec<ChatImage>)>,
    rpc_rejected_before_record: bool,
    started_at: Option<u64>,
) -> bool {
    let Some(cancel_token) = app.chat().claim_antigravity_401_recovery(session_id) else {
        return false;
    };
    let continuation = match saved_prompt.filter(|_| rpc_rejected_before_record) {
        Some((text, images)) => Pending401Continuation {
            text,
            images,
            ensure_user_row: true,
            started_at,
        },
        None => Pending401Continuation {
            text: AUTH_RECOVERY_PROMPT.to_string(),
            images: Vec::new(),
            ensure_user_row: false,
            started_at,
        },
    };
    crate::diagnostics::record("WARN", "agent_chat", json!({
        "sessionId": session_id,
        "event": "antigravity_401_auto_recovery",
        "rpcRejected": rpc_rejected_before_record,
        "reason": reason,
    }));
    let (app, session_id, proc, reason_owned) = (
        app.clone(),
        session_id.to_string(),
        proc.clone(),
        reason.to_string(),
    );
    std::thread::spawn(move || {
        crate::agent::antigravity::invalidate_readiness_cache();
        if cancel_token.load(Ordering::Relaxed) {
            return;
        }
        let (model, effort) = {
            let _action = proc.action.lock().unwrap();
            let _callbacks = proc.callbacks.read().unwrap();
            if !app.chat().owns_process(&session_id, &proc)
                || cancel_token.load(Ordering::Relaxed)
            {
                return;
            }
            let model = proc.model.lock().unwrap().clone();
            let effort = proc.effort.lock().unwrap().clone();
            {
                let mut state = proc.acp.lock().unwrap();
                state.recovering_401 = true;
                state.pending_401_continuation = Some(continuation);
            }
            proc.auth_restart.store(true, Ordering::Relaxed);
            super::release_process(&app, &session_id, &proc);
            (model, effort)
        };

        // Run a readiness probe so `agy` refreshes any expired OAuth token on disk cleanly
        // before the replacement ACP session starts, or confirms the account is genuinely signed out.
        let signed_in = super::auth::readiness::probe_antigravity(&app, &session_id);
        if cancel_token.load(Ordering::Relaxed) {
            proc.acp.lock().unwrap().pending_401_continuation = None;
            return;
        }
        if signed_in == Some(false) {
            proc.acp.lock().unwrap().pending_401_continuation = None;
            let _ = crate::command_core::chat_start(
                &app,
                &session_id,
                model.as_deref(),
                effort.as_deref(),
                false,
            );
            emit(&app, &session_id, json!({"type":"turnCompleted"}));
            emit_state(&app, &session_id, AgentState::Waiting);
            if let Ok(next) = app.chat().get(&session_id) {
                super::auth::antigravity::require(&app, &session_id, &next);
            }
            return;
        }

        if let Err(err) = crate::command_core::chat_start(
            &app,
            &session_id,
            model.as_deref(),
            effort.as_deref(),
            false,
        ) {
            proc.acp.lock().unwrap().pending_401_continuation = None;
            proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                id: format!("e-acp-401-{}", now_ms()),
                message: format!("{reason_owned} ({err})"),
            });
            emit(&app, &session_id, json!({"type":"turnCompleted"}));
            emit_state(&app, &session_id, AgentState::Waiting);
        }
    });
    true
}

/// Evict a poisoned ACP process after a repeated 401, or surface the sign-in panel if logged out.
fn evict_or_require_auth_after_401(app: AppCtx, session_id: String, proc: Arc<ChatProcess>) {
    std::thread::spawn(move || {
        crate::agent::antigravity::invalidate_readiness_cache();
        let signed_in = super::auth::readiness::probe_antigravity(&app, &session_id);
        let _action = proc.action.lock().unwrap();
        let _callbacks = proc.callbacks.read().unwrap();
        if !app.chat().owns_process(&session_id, &proc) || !proc.alive.load(Ordering::Relaxed) {
            return;
        }
        if signed_in == Some(false) {
            super::auth::antigravity::require(&app, &session_id, &proc);
        } else {
            proc.auth_restart.store(true, Ordering::Relaxed);
            super::release_process(&app, &session_id, &proc);
        }
    });
}

/// The process is gone: close whatever was streaming and keep the journal complete.
pub(super) fn finish(proc: &Arc<ChatProcess>) {
    let (rows, reactive_rows, park, retry_wait, transient_wait, inactivity_wait, waiter, gate_permit) = {
        let mut state = proc.acp.lock().unwrap();
        state.parked_idle = false;
        state.settled_while_parked = false;
        state.retrying_prompt = false;
        state.active_prompt = None;
        state.outcome_watch = None;
        state.turn_auth_error = None;
        state.split_auth_row_id = None;
        state.turn_transient_error = None;
        state.transient_retry_attempts = 0;
        state.split_transient_row_id = None;
        state.startup_native = None;
        state.startup_prompt = None;
        let gate_permit = state.auth_gate_permit.take();
        let park = state.stop_park.take();
        let retry_wait = state.model_retry_wait.take();
        let transient_wait = state.transient_retry_wait.take();
        let inactivity_wait = state.inactivity_wait.take();
        let waiter = state.retire_waiter.clone();
        let rows: Vec<ChatRow> = state.mapper.finish().into_iter().chain(state.mapper.settle_running(true)).collect();
        let reactive_rows = state.tasks.take_aborted_reactive_rows();
        flush_journal_locked(&mut state);
        (rows, reactive_rows, park, retry_wait, transient_wait, inactivity_wait, waiter, gate_permit)
    };
    drop(gate_permit);
    if let Some(park) = park {
        park.release("allow");
    }
    if let Some(wait) = retry_wait {
        wait.wake();
    }
    if let Some(wait) = transient_wait {
        wait.wake();
    }
    if let Some(wait) = inactivity_wait {
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
    let outcome_watch = (proc.kind == SessionKind::Antigravity)
        .then(|| PromptOutcomeWatch::capture(&session, &proc.auth_settings_args));
    let (wire_text, reactive_rows, prev_retry_wait, prev_transient_wait, prev_inactivity_wait) = {
        let mut state = proc.acp.lock().unwrap();
        let wire_text = match (proc.kind == SessionKind::Antigravity)
            .then(|| state.pending_context_carryover.take())
            .flatten()
        {
            Some(carryover) if !text.trim().is_empty() => {
                format!("{carryover}\n\nCurrent user request:\n{text}")
            }
            Some(carryover) => carryover,
            None => text.to_string(),
        };
        let reactive_rows = state.tasks.take_aborted_reactive_rows();
        state.parked_idle = false;
        state.settled_while_parked = false;
        state.retrying_prompt = false;
        state.needs_error_rotate = false;
        state.usage_limit_reset = None;
        state.turn_auth_error = None;
        state.split_auth_row_id = None;
        state.turn_transient_error = None;
        state.transient_retry_attempts = 0;
        state.split_transient_row_id = None;
        state.last_prompt_activity_ms = now_ms();
        state.outcome_watch = outcome_watch;
        let prev_retry_wait = state.model_retry_wait.take();
        let prev_transient_wait = state.transient_retry_wait.take();
        let prev_inactivity_wait = state.inactivity_wait.take();
        state.active_prompt = (proc.kind == SessionKind::Antigravity).then(|| ActivePrompt {
            text: wire_text.clone(),
            images: images.to_vec(),
            attempts: 0,
            had_output: false,
            had_other_output: false,
            sole_assistant_id: None,
            chunk_cold_error: None,
        });
        state.mapper.begin_turn();
        state.prompt = Some(id.clone());
        state.steers.clear();
        if state.journal.is_some() {
            state.pending_journal.push(wire::journal_prompt(text, now_ms() as i64));
            flush_journal_locked(&mut state);
        }
        (wire_text, reactive_rows, prev_retry_wait, prev_transient_wait, prev_inactivity_wait)
    };
    if let Some(wait) = prev_retry_wait {
        wait.wake();
    }
    if let Some(wait) = prev_transient_wait {
        wait.wake();
    }
    if let Some(wait) = prev_inactivity_wait {
        wait.wake();
    }
    super::antigravity_tasks::settle_rows(proc, &reactive_rows);
    let result = proc.write(&wire::prompt(&id, &session, &wire_text, images));
    if result.is_err() {
        proc.pending.lock().unwrap().remove(&id);
        let mut state = proc.acp.lock().unwrap();
        state.prompt = None;
        state.active_prompt = None;
        state.outcome_watch = None;
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
    let outcome_watch = (proc.kind == SessionKind::Antigravity)
        .then(|| PromptOutcomeWatch::capture(&session, &proc.auth_settings_args));
    let (id, rows, reactive_rows) = {
        let mut state = proc.acp.lock().unwrap();
        if state.retrying_prompt {
            if state.transient_retry_wait.is_some() {
                return Err("Antigravity is retrying the current step; queue the message instead".into());
            }
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
        state.last_prompt_activity_ms = now_ms();
        if proc.kind == SessionKind::Antigravity {
            state.outcome_watch = outcome_watch;
        }
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
    if proc.acp.lock().unwrap().startup_prompt.take().is_some() {
        return Ok(());
    }
    let session = proc.agent_session_id.lock().unwrap().clone().ok_or("The agent has not opened its conversation yet")?;
    release_parked_stop(proc, "allow");
    let (retry_wait, transient_wait, inactivity_wait) = {
        let mut state = proc.acp.lock().unwrap();
        state.retrying_prompt = false;
        if state.active_prompt.as_ref().is_some_and(|active| active.chunk_cold_error.is_some())
            || state.split_auth_row_id.is_some()
            || state.split_transient_row_id.is_some()
        {
            let _ = state.mapper.finish();
        }
        state.active_prompt = None;
        state.outcome_watch = None;
        state.split_auth_row_id = None;
        state.turn_transient_error = None;
        state.transient_retry_attempts = 0;
        state.split_transient_row_id = None;
        (
            state.model_retry_wait.take(),
            state.transient_retry_wait.take(),
            state.inactivity_wait.take(),
        )
    };
    if let Some(wait) = retry_wait {
        wait.wake();
    }
    if let Some(wait) = transient_wait {
        wait.wake();
    }
    if let Some(wait) = inactivity_wait {
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

    /// A missing recorded conversation is deliberately replaced with a fresh one. On a resume
    /// `expected_native` still names the lost conversation, so unless it is cleared the identity guard
    /// rejects the replacement and tears the process down right after the "a new one has been started"
    /// message.
    #[test]
    fn a_recovered_missing_conversation_survives_the_identity_guard() {
        let app = ctx("acp-load-missing-identity");
        let (mut proc, mut agent) = wired(SessionKind::Grok);
        *Arc::get_mut(&mut proc).unwrap().expected_native.lock().unwrap() = Some("native-lost".into());
        let errors = Arc::new(Mutex::new(Vec::<String>::new()));
        {
            let errors = errors.clone();
            app.listen("chat://event/s", move |payload| {
                let value: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
                if value["type"] == "error" {
                    errors.lock().unwrap().push(value["message"].as_str().unwrap_or_default().to_string());
                }
            });
        }

        bootstrap(&app, &proc, Some("native-lost"), Some("/w"), None);
        let initialize = written(&mut agent);
        reply(&app, &proc, &initialize, json!({"agentCapabilities":{"loadSession":true}}));
        let load = written(&mut agent);
        assert_eq!(load["method"], "session/load");
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":load["id"],
            "error":{"code":-32603,"message":"Path not found.","data":{"code":"FS_NOT_FOUND"}}}).to_string());
        let fresh = written(&mut agent);
        assert_eq!(fresh["method"], "session/new");

        // The agent opens the replacement conversation.
        reply(&app, &proc, &fresh, json!({"sessionId":"native-fresh"}));

        let seen = errors.lock().unwrap().clone();
        assert!(
            !seen.iter().any(|m| m.contains("different native conversation")),
            "the replacement conversation must not be vetoed by the identity guard; errors: {seen:?}"
        );
        assert!(proc.alive.load(Ordering::Relaxed), "the replacement conversation must stay alive");
        assert_eq!(proc.agent_session_id.lock().unwrap().as_deref(), Some("native-fresh"));
    }

    /// Agents that persist their own ACP journal (Copilot, Cursor, Kimi) must bind the journal to the
    /// replacement conversation after a missing-conversation fallback instead of appending to the lost id's file.
    #[test]
    fn a_recovered_missing_conversation_rebinds_the_journal_to_the_replacement() {
        let app = ctx("acp-load-missing-journal");
        let (mut proc, mut agent) = wired(SessionKind::Copilot);
        *Arc::get_mut(&mut proc).unwrap().expected_native.lock().unwrap() = Some("native-lost".into());

        bootstrap(&app, &proc, Some("native-lost"), Some("/w"), None);
        assert_eq!(
            proc.acp.lock().unwrap().journal,
            journal_path(&app, SessionKind::Copilot, "native-lost")
        );
        let initialize = written(&mut agent);
        reply(&app, &proc, &initialize, json!({"agentCapabilities":{"loadSession":true}}));
        let load = written(&mut agent);
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":load["id"],
            "error":{"code":-32000,"message":"Session not found"}}).to_string());
        let fresh = written(&mut agent);
        assert_eq!(fresh["method"], "session/new");
        reply(&app, &proc, &fresh, json!({"sessionId":"native-fresh"}));

        assert_eq!(
            proc.acp.lock().unwrap().journal,
            journal_path(&app, SessionKind::Copilot, "native-fresh"),
            "subsequent turns must write to the replacement conversation's journal"
        );
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

        // Subagent stop hooks, non-NO_TOOL_CALL, safety-capped invocations, and ordinary turns with no
        // background tasks or unread deliveries must allow immediately without parking.
        assert_eq!(handle_stop_hook(&app, "s", &proc, r#"{"conversationId":"sub-conv","terminationReason":"NO_TOOL_CALL","executionNum":0}"#), r#"{"decision":"allow"}"#);
        assert_eq!(handle_stop_hook(&app, "s", &proc, r#"{"conversationId":"main-conv","terminationReason":"USER_CANCELED","executionNum":0}"#), r#"{"decision":"allow"}"#);
        assert_eq!(handle_stop_hook(&app, "s", &proc, &format!(r#"{{"conversationId":"main-conv","terminationReason":"NO_TOOL_CALL","executionNum":{}}}"#, MAX_SAFE_STOP_CONTINUATIONS)), r#"{"decision":"allow"}"#);
        assert_eq!(handle_stop_hook(&app, "s", &proc, r#"{"conversationId":"main-conv","terminationReason":"NO_TOOL_CALL","executionNum":0}"#), r#"{"decision":"allow"}"#);
        assert!(!is_stop_parked(&proc));

        // When background work is active, NO_TOOL_CALL parks the turn until an unread delivery arrives.
        proc.turn.lock().unwrap().active_tasks.insert("task-1".into());
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

        h.wait_until(Duration::from_secs(60), "Turn 1 completion", |proc| {
            let snap = h.app.chat().snapshot(&h.sid);
            (!proc.turn.lock().unwrap().running && snap.turn_started_at.is_none() && marker1.exists()).then_some(())
        });
        assert!(!is_stop_parked(h.proc.as_ref().unwrap()));
        assert_eq!(h.proc.as_ref().unwrap().acp.lock().unwrap().stop_execution_nums, vec![0]);

        // Send Turn 2 with a `schedule` timer; Stop parks while the timer is active and wakes on expiry.
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
        h.wait_until(Duration::from_secs(30), "Turn 2 settled after wakeup", |proc| {
            let snap = h.app.chat().snapshot(&h.sid);
            (!proc.turn.lock().unwrap().running && snap.turn_started_at.is_none()).then_some(())
        });

        // Turn 3 sent after a Stop-continue wakeup resets executionNum to 0 and finishes unparked.
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
        h.wait_until(Duration::from_secs(60), "Turn 3 reply", |proc| {
            let snap = h.app.chat().snapshot(&h.sid);
            let has_reply = snap.rows.iter().rev().any(|r| {
                matches!(r, ChatRow::Assistant { text, streaming: false, .. } if text.contains("TURN3_OK"))
            });
            (!proc.turn.lock().unwrap().running && snap.turn_started_at.is_none() && has_reply).then_some(())
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

    #[test]
    fn antigravity_chunk_cold_model_error_retries_silently_and_syncs_default_model() {
        let app = ctx("acp-agy-chunk-cold-retry");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;

        dispatch(&proc, "vtell?", &[]).unwrap();
        let first = written(&mut agent);
        assert_eq!(first["method"], "session/prompt");

        // Simulate `agy acp` 1.5.14 emitting `ERROR_MESSAGE` as a warning-prefixed
        // `agent_message_chunk` followed by a normal `stopReason: "end_turn"` response.
        notify(&app, &proc, json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {
                "type": "text",
                "text": "\n⚠ failed to construct executor: plan model not specified; you must specify a valid model\n"
            }
        }));
        reply(&app, &proc, &first, json!({"stopReason": "end_turn"}));

        assert!(proc.turn.lock().unwrap().running);
        assert!(has_active_prompt(&proc));
        assert!(proc.timeline.lock().unwrap().rows.is_empty());

        // With `proc.model` unset, retry pushes a fallback model via `session/set_config_option`.
        let set_model_req = written(&mut agent);
        assert_eq!(set_model_req["method"], "session/set_config_option");
        assert_eq!(set_model_req["params"]["configId"], "model");
        let chosen = set_model_req["params"]["value"].as_str().unwrap().to_string();
        assert!(!chosen.is_empty());
        reply(&app, &proc, &set_model_req, json!({
            "configOptions": [{
                "type": "select",
                "id": "model",
                "category": "model",
                "currentValue": chosen,
                "options": [
                    {"value": "gemini-4-neon", "name": "Gemini 4 Neon"},
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"}
                ]
            }]
        }));

        let retry = written(&mut agent);
        assert_eq!(proc.model.lock().unwrap().as_deref(), Some(chosen.as_str()));
        assert_eq!(retry["method"], "session/prompt");
        assert_ne!(retry["id"], first["id"]);
        assert_eq!(retry["params"]["prompt"], json!([{"type":"text","text":"vtell?"}]));

        notify(&app, &proc, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "Recovered"}}));
        reply(&app, &proc, &retry, json!({"stopReason": "end_turn"}));
        assert!(!proc.turn.lock().unwrap().running);
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], ChatRow::Assistant { text, streaming: false, .. } if text == "Recovered"));
    }

    #[test]
    fn antigravity_chunk_cold_model_error_retracts_split_chunks_and_surfaces_error_on_exhaustion() {
        // Case 1: split chunks retract the partial warning row once the full cold-start error arrives.
        let app = ctx("acp-agy-chunk-split");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        *proc.model.lock().unwrap() = Some("gemini-4-neon".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;

        dispatch(&proc, "hello", &[]).unwrap();
        let first = written(&mut agent);
        notify(&app, &proc, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "\n⚠ "}}));
        assert_eq!(proc.timeline.lock().unwrap().rows.len(), 1);
        notify(&app, &proc, json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "failed to construct executor: unknown model key MODEL_PLACEHOLDER_M532: model not found\n"}
        }));
        assert!(proc.timeline.lock().unwrap().rows.is_empty());
        reply(&app, &proc, &first, json!({"stopReason": "end_turn"}));

        let retry = written(&mut agent);
        assert_eq!(retry["method"], "session/prompt");
        notify(&app, &proc, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "All good"}}));
        reply(&app, &proc, &retry, json!({"stopReason": "end_turn"}));
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], ChatRow::Assistant { text, streaming: false, .. } if text == "All good"));

        // Case 2: if every retry keeps returning the warning chunk, surface a real Error row on exhaustion.
        let app2 = ctx("acp-agy-chunk-exhaust");
        let (proc2, mut agent2) = wired(SessionKind::Antigravity);
        *proc2.agent_session_id.lock().unwrap() = Some("s2".into());
        *proc2.model.lock().unwrap() = Some("gemini-4-neon".into());
        proc2.ready.store(true, Ordering::Relaxed);
        proc2.turn.lock().unwrap().running = true;

        dispatch(&proc2, "hello", &[]).unwrap();
        let mut current = written(&mut agent2);
        for attempt in 0..=cold_model_retry_delays().len() {
            notify(&app2, &proc2, json!({
                "sessionUpdate": "agent_message_chunk",
                "content": {
                    "type": "text",
                    "text": "\n⚠ failed to construct executor: plan model not specified; you must specify a valid model\n"
                }
            }));
            reply(&app2, &proc2, &current, json!({"stopReason": "end_turn"}));
            if attempt < cold_model_retry_delays().len() {
                let set_model = written(&mut agent2);
                assert_eq!(set_model["method"], "session/set_config_option");
                handle_line(&app2, "s", &proc2, &json!({
                    "jsonrpc": "2.0",
                    "id": set_model["id"],
                    "error": {"code": -32603, "message": "models unavailable"}
                }).to_string());
                current = written(&mut agent2);
                assert_eq!(current["method"], "session/prompt");
            }
        }
        assert!(!proc2.turn.lock().unwrap().running);
        let rows = proc2.timeline.lock().unwrap().rows.clone();
        assert!(!rows.iter().any(|r| matches!(r, ChatRow::Assistant { .. })));
        assert!(rows.iter().any(|r| {
            matches!(r, ChatRow::Error { message, .. } if message.contains("plan model not specified"))
        }));
    }

    #[test]
    fn session_load_flushes_replace_rows_before_announcing_session_ready() {
        let app = ctx("acp-flush-before-session");
        let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let _listener = app.listen("chat://event/s", move |payload| {
            if let Ok(val) = serde_json::from_str::<Value>(payload) {
                if let Some(kind) = val.get("type").and_then(Value::as_str) {
                    sink.lock().unwrap().push(kind.to_string());
                }
            }
        });

        let (proc, mut agent) = wired(SessionKind::Antigravity);
        bootstrap(&app, &proc, Some("agy-resume-1"), Some("/w"), None);
        let init = written(&mut agent);
        reply(&app, &proc, &init, json!({"agentCapabilities":{"loadSession":true}}));
        let load = written(&mut agent);
        assert_eq!(load["method"], "session/load");
        notify(&app, &proc, json!({"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"Earlier prompt"}}));
        notify(&app, &proc, json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Earlier answer"}}));
        reply(&app, &proc, &load, json!({"sessionId":"agy-resume-1"}));

        let recorded = events.lock().unwrap().clone();
        let replace_idx = recorded.iter().position(|kind| kind == "replaceRows").expect("replaceRows emitted synchronously");
        let session_idx = recorded.iter().position(|kind| kind == "session").expect("session ready emitted");
        assert!(replace_idx < session_idx, "expected replaceRows before session, got {recorded:?}");
        assert!(matches!(proc.timeline.lock().unwrap().take_flush(), super::super::TimelineFlush::None));

        // When Antigravity advertises both `loadSession` and `sessionCapabilities.resume`, `session/resume`
        // must be preferred over `session/load`, and a cold-start prompt accepted before the handshake
        // answers must dispatch automatically once `finish_startup` runs.
        let (proc_resume, mut agent_resume) = wired(SessionKind::Antigravity);
        proc_resume.ready.store(false, Ordering::Relaxed);
        app.chat().sessions.lock().unwrap().insert("s".into(), proc_resume.clone());
        bootstrap(&app, &proc_resume, Some("agy-resume-2"), Some("/w"), None);
        let init_resume = written(&mut agent_resume);
        let send_outcome = app.chat().send_identified(&app, "s", "First cold prompt", vec![], "queue", None).unwrap();
        assert_eq!(send_outcome, "sent");
        assert!(proc_resume.turn.lock().unwrap().running);
        assert!(proc_resume.timeline.lock().unwrap().rows.iter().any(|row| {
            matches!(row, ChatRow::User { text, .. } if text == "First cold prompt")
        }));
        reply(&app, &proc_resume, &init_resume, json!({
            "agentCapabilities": {
                "loadSession": true,
                "sessionCapabilities": { "resume": {} }
            }
        }));
        let resume_req = written(&mut agent_resume);
        assert_eq!(resume_req["method"], "session/resume");
        reply(&app, &proc_resume, &resume_req, json!({"sessionId":"agy-resume-2"}));
        let prompt_req = written(&mut agent_resume);
        assert_eq!(prompt_req["method"], "session/prompt");
        assert_eq!(prompt_req["params"]["prompt"][0]["text"], "First cold prompt");
        app.chat().sessions.lock().unwrap().remove("s");
    }

    #[cfg(unix)]
    fn write_mock_agy_script(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let state_path = dir.join("state.json");
        std::fs::write(
            &state_path,
            json!({
                "acp_spawns": 0,
                "list_probes": 0,
                "always_401": false,
                "prompts": [],
                "history": [],
            })
            .to_string(),
        )
        .unwrap();

        let conv_id = format!("conv-{}", uuid::Uuid::new_v4());
        let bin_path = dir.join("mock-agy");
        let script = format!(
            r#"#!/usr/bin/env python3
import json, os, sys

STATE_PATH = {state:?}
CONV_ID = {conv_id:?}
ERR_401 = "\n⚠ UNAUTHENTICATED (code 401): Request had invalid authentication credentials. Expected OAuth 2 access token, login cookie or other valid authentication credential.\n"
RECOVERY = {recovery:?}

def load_state():
    with open(STATE_PATH, "r", encoding="utf-8") as f:
        return json.load(f)

def save_state(st):
    tmp = STATE_PATH + f".tmp.{{os.getpid()}}"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(st, f)
    os.replace(tmp, STATE_PATH)

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def notify(sid, update):
    send({{"jsonrpc": "2.0", "method": "session/update", "params": {{"sessionId": sid, "update": update}}}})

if any(arg in ("--version", "-v", "version") for arg in sys.argv[1:]):
    sys.stdout.write("agy 1.5.14\n")
    sys.stdout.flush()
    sys.exit(0)

while True:
    raw = sys.stdin.readline()
    if not raw:
        break
    raw = raw.strip()
    if not raw:
        continue
    req = json.loads(raw)
    method = req.get("method")
    rid = req.get("id")
    params = req.get("params") or {{}}
    if method == "initialize":
        send({{"jsonrpc": "2.0", "id": rid, "result": {{"agentCapabilities": {{"loadSession": True}}}}}})
    elif method == "session/list":
        st = load_state()
        st["list_probes"] = st.get("list_probes", 0) + 1
        save_state(st)
        send({{"jsonrpc": "2.0", "id": rid, "result": {{"sessions": []}}}})
    elif method == "session/new":
        st = load_state()
        st["acp_spawns"] = st.get("acp_spawns", 0) + 1
        save_state(st)
        send({{"jsonrpc": "2.0", "id": rid, "result": {{
            "sessionId": CONV_ID,
            "modes": {{"currentModeId": "default", "availableModes": [{{"id": "default", "name": "Default"}}]}}
        }}}})
    elif method == "session/load":
        sid = params.get("sessionId") or CONV_ID
        st = load_state()
        st["acp_spawns"] = st.get("acp_spawns", 0) + 1
        history = list(st.get("history") or [])
        save_state(st)
        for upd in history:
            notify(sid, upd)
        send({{"jsonrpc": "2.0", "id": rid, "result": {{
            "sessionId": sid,
            "modes": {{"currentModeId": "default", "availableModes": [{{"id": "default", "name": "Default"}}]}}
        }}}})
    elif method in ("session/set_mode", "session/set_config_option"):
        send({{"jsonrpc": "2.0", "id": rid, "result": {{}}}})
    elif method == "session/prompt":
        sid = params.get("sessionId") or CONV_ID
        prompt_items = params.get("prompt") or []
        text = prompt_items[0].get("text", "") if prompt_items else ""
        st = load_state()
        st["prompts"].append(text)
        user_upd = {{"sessionUpdate": "user_message_chunk", "content": {{"type": "text", "text": text}}}}
        st["history"].append(user_upd)
        if st.get("always_401"):
            err_upd = {{"sessionUpdate": "agent_message_chunk", "content": {{"type": "text", "text": ERR_401}}}}
            st["history"].append(err_upd)
            save_state(st)
            notify(sid, err_upd)
            send({{"jsonrpc": "2.0", "id": rid, "result": {{"stopReason": "end_turn"}}}})
        elif text == "Submit 6 Play Console declarations":
            part_upd = {{"sessionUpdate": "agent_message_chunk", "content": {{"type": "text", "text": "Submitting 6 declarations..."}}}}
            err_upd = {{"sessionUpdate": "agent_message_chunk", "content": {{"type": "text", "text": ERR_401}}}}
            st["history"].extend([part_upd, err_upd])
            save_state(st)
            notify(sid, part_upd)
            notify(sid, err_upd)
            send({{"jsonrpc": "2.0", "id": rid, "result": {{"stopReason": "end_turn"}}}})
        else:
            reply_text = "Completed all 6 declarations." if text == RECOVERY else f"Completed: {{text}}"
            ans_upd = {{"sessionUpdate": "agent_message_chunk", "content": {{"type": "text", "text": reply_text}}}}
            st["history"].append(ans_upd)
            save_state(st)
            notify(sid, ans_upd)
            send({{"jsonrpc": "2.0", "id": rid, "result": {{"stopReason": "end_turn"}}}})
"#,
            state = state_path.to_string_lossy(),
            conv_id = conv_id,
            recovery = AUTH_RECOVERY_PROMPT,
        );
        std::fs::write(&bin_path, script).unwrap();
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        (bin_path, state_path)
    }

    #[cfg(unix)]
    fn setup_mock_agy_session(tag: &str) -> (AppCtx, String, std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("vlx-mock-agy-{tag}-{}", uuid::Uuid::new_v4()));
        let workdir = dir.join("work");
        std::fs::create_dir_all(&workdir).unwrap();
        let (bin_path, state_path) = write_mock_agy_script(&dir);
        let db = crate::db::Db::open(&dir.join("t.db")).unwrap();
        let app = AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(dir.clone(), db)));
        #[allow(irrefutable_let_patterns)]
        if let AppCtx::Headless(host) = &app {
            host.set_hooks(crate::agent::server::HookServer { port: 0, token: "fixture".into() });
        }
        let sid = format!("sid-{tag}-{}", uuid::Uuid::new_v4());
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
                bin_path.to_str(),
                Some("chat"),
            )
            .unwrap();
        }
        crate::command_core::chat_start(&app, &sid, None, None, false).unwrap();
        let proc = app.chat().get(&sid).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !proc.ready.load(Ordering::Relaxed) && Instant::now() < deadline {
            if !proc.alive.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            proc.ready.load(Ordering::Relaxed),
            "mock agy ACP did not become ready: alive={}, stderr={:?}, rows={:?}",
            proc.alive.load(Ordering::Relaxed),
            proc.stderr.lock().unwrap().as_str(),
            app.chat().snapshot(&sid).rows
        );
        (app, sid, dir, state_path)
    }

    #[cfg(unix)]
    #[test]
    fn antigravity_streamed_401_restarts_process_resumes_turn_and_stops_on_second_401() {
        let _epoch_guard = crate::agent::antigravity::test_auth_epoch_lock();
        let (app, sid, dir, state_path) = setup_mock_agy_session("recover-401");
        let pid1 = app.chat().get(&sid).unwrap().pid;

        // Turn 1 hits a mid-turn streamed 401 warning; VelaTerm must restart `agy acp`, suppress the
        // raw warning and internal continuation prompt, and finish the turn cleanly.
        let mid1 = format!("msg-{}", uuid::Uuid::new_v4());
        crate::command_core::chat_send(
            &app,
            &sid,
            "Submit 6 Play Console declarations",
            vec![],
            Some("queue"),
            Some(&mid1),
        )
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut recovered_pid = pid1;
        loop {
            let snap = app.chat().snapshot(&sid);
            let completed = snap.rows.iter().any(|r| {
                matches!(r, ChatRow::Assistant { text, streaming: false, .. } if text == "Completed all 6 declarations.")
            });
            if completed && snap.turn_started_at.is_none() {
                if let Ok(proc) = app.chat().get(&sid) {
                    recovered_pid = proc.pid;
                }
                break;
            }
            assert!(Instant::now() < deadline, "401 auto-recovery did not complete in time; rows={:?}", snap.rows);
            std::thread::sleep(Duration::from_millis(25));
        }
        assert_ne!(recovered_pid, pid1, "expected `agy acp` to restart onto a fresh OS process");
        let snap1 = app.chat().snapshot(&sid);
        assert!(!snap1.rows.iter().any(|r| matches!(r, ChatRow::Error { .. })), "unexpected error row: {:?}", snap1.rows);
        assert!(!snap1.rows.iter().any(|r| matches!(r, ChatRow::User { text, .. } if text == AUTH_RECOVERY_PROMPT)));
        assert!(!snap1.rows.iter().any(|r| matches!(r, ChatRow::Assistant { text, .. } if text.contains("UNAUTHENTICATED"))));
        assert!(snap1.rows.iter().any(|r| matches!(r, ChatRow::Assistant { text, .. } if text == "Submitting 6 declarations...")));

        // Cold reopen (`session/load` with `suppress_replay_401 == false`) must keep the earlier recovered 401 hidden.
        crate::command_core::chat_stop(&app, &sid).unwrap();
        crate::command_core::chat_start(&app, &sid, None, None, false).unwrap();
        let reopened = app.chat().get(&sid).unwrap();
        let reopen_deadline = Instant::now() + Duration::from_secs(10);
        while !reopened.ready.load(Ordering::Relaxed) && Instant::now() < reopen_deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let reopened_rows = app.chat().snapshot(&sid).rows;
        assert!(!reopened_rows.iter().any(|r| matches!(r, ChatRow::Error { .. })), "reopened rows had error: {reopened_rows:?}");
        assert!(!reopened_rows.iter().any(|r| matches!(r, ChatRow::Assistant { text, .. } if text.contains("UNAUTHENTICATED"))));
        assert!(!reopened_rows.iter().any(|r| matches!(r, ChatRow::User { text, .. } if text == AUTH_RECOVERY_PROMPT)));

        // Turn 2: if 401 persists after the one-shot restart, stop retrying and surface the error row.
        let mut st: Value = serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        let spawns_before_turn2 = st["acp_spawns"].as_u64().unwrap();
        st["always_401"] = json!(true);
        std::fs::write(&state_path, st.to_string()).unwrap();

        let mid2 = format!("msg-{}", uuid::Uuid::new_v4());
        crate::command_core::chat_send(&app, &sid, "Trigger persistent 401", vec![], Some("queue"), Some(&mid2)).unwrap();
        let err_deadline = Instant::now() + Duration::from_secs(25);
        loop {
            let snap = app.chat().snapshot(&sid);
            let has_err = snap.rows.iter().any(|r| {
                matches!(r, ChatRow::Error { message, .. } if message.contains("UNAUTHENTICATED (code 401)"))
            });
            if has_err && snap.turn_started_at.is_none() {
                break;
            }
            assert!(Instant::now() < err_deadline, "persistent 401 did not settle with Error row; rows={:?}", snap.rows);
            std::thread::sleep(Duration::from_millis(25));
        }
        let st_after: Value = serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        assert_eq!(
            st_after["acp_spawns"].as_u64().unwrap(),
            spawns_before_turn2 + 2,
            "persistent 401 must restart at most MAX_ANTIGRAVITY_401_RESTARTS (2) times per turn"
        );

        let _ = crate::command_core::chat_stop(&app, &sid);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn antigravity_idle_session_rotates_proactively_after_auth_epoch_bump() {
        let _epoch_guard = crate::agent::antigravity::test_auth_epoch_lock();
        let (app, sid, dir, _state_path) = setup_mock_agy_session("idle-rotate");
        let pid1 = app.chat().get(&sid).unwrap().pid;

        // Simulate another session or login refreshing Antigravity credentials on disk.
        crate::agent::antigravity::bump_auth_epoch();

        let mid = format!("msg-{}", uuid::Uuid::new_v4());
        crate::command_core::chat_send(&app, &sid, "Run after token refresh", vec![], Some("queue"), Some(&mid)).unwrap();

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let snap = app.chat().snapshot(&sid);
            let done = snap.rows.iter().any(|r| {
                matches!(r, ChatRow::Assistant { text, streaming: false, .. } if text == "Completed: Run after token refresh")
            });
            if done && snap.turn_started_at.is_none() {
                break;
            }
            assert!(Instant::now() < deadline, "rotated prompt did not finish; rows={:?}", snap.rows);
            std::thread::sleep(Duration::from_millis(25));
        }
        let pid2 = app.chat().get(&sid).unwrap().pid;
        assert_ne!(pid2, pid1, "idle ACP process launched under an older auth_epoch must rotate before sending");

        let _ = crate::command_core::chat_stop(&app, &sid);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn antigravity_mid_turn_transient_404_retries_in_place_and_keeps_prior_tools() {
        let app = ctx("acp-agy-transient-404");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;

        dispatch(&proc, "Inspect readiness", &[]).unwrap();
        let first = written(&mut agent);
        assert_eq!(first["method"], "session/prompt");

        // Simulate two completed Read tool calls followed by a transient upstream 404 chunk.
        notify(&app, &proc, json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "read-1",
            "title": "Read useAgentReadiness.ts",
            "kind": "read",
            "status": "completed"
        }));
        notify(&app, &proc, json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "read-2",
            "title": "Read readiness.rs",
            "kind": "read",
            "status": "completed"
        }));
        notify(&app, &proc, json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {
                "type": "text",
                "text": "\n⚠ NOT_FOUND (code 404): Requested entity was not found.\n"
            }
        }));
        reply(&app, &proc, &first, json!({"stopReason": "end_turn"}));

        // The turn must remain active and automatically send the internal continuation prompt.
        assert!(proc.turn.lock().unwrap().running);
        assert!(has_active_prompt(&proc));
        let retry = written(&mut agent);
        assert_eq!(retry["method"], "session/prompt");
        assert_eq!(retry["params"]["prompt"][0]["text"], TRANSIENT_RECOVERY_PROMPT);

        // Once the retry succeeds, the prior tool calls and final answer remain cleanly on screen.
        notify(&app, &proc, json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "Readiness analysis finished."}
        }));
        reply(&app, &proc, &retry, json!({"stopReason": "end_turn"}));

        assert!(!proc.turn.lock().unwrap().running);
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(), 3, "expected 2 tool rows + 1 assistant row, got {rows:?}");
        assert!(rows.iter().all(|r| !matches!(r, ChatRow::Error { .. })));
        assert!(rows.iter().all(|r| !matches!(r, ChatRow::Assistant { text, .. } if text.contains("NOT_FOUND"))));
        assert!(matches!(&rows[2], ChatRow::Assistant { text, streaming: false, .. } if text == "Readiness analysis finished."));

        // Replay via `session/load` hides a recovered transient warning + continuation prompt,
        // while converting an unrecovered trailing transient warning into a proper Error row.
        let (replay_proc, mut replay_agent) = wired(SessionKind::Antigravity);
        bootstrap(&app, &replay_proc, Some("s1"), Some("/w"), None);
        let init = written(&mut replay_agent);
        reply(&app, &replay_proc, &init, json!({"agentCapabilities": {"loadSession": true}}));
        let load = written(&mut replay_agent);
        assert_eq!(load["method"], "session/load");
        notify(&app, &replay_proc, json!({"sessionUpdate": "user_message_chunk", "content": {"type": "text", "text": "Inspect readiness"}}));
        notify(&app, &replay_proc, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "\n⚠ NOT_FOUND (code 404): Requested entity was not found.\n"}}));
        notify(&app, &replay_proc, json!({"sessionUpdate": "user_message_chunk", "content": {"type": "text", "text": TRANSIENT_RECOVERY_PROMPT}}));
        notify(&app, &replay_proc, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "Readiness analysis finished."}}));
        notify(&app, &replay_proc, json!({"sessionUpdate": "user_message_chunk", "content": {"type": "text", "text": "Follow up"}}));
        notify(&app, &replay_proc, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "\n⚠ INTERNAL (code 500): Internal error encountered.\n"}}));
        reply(&app, &replay_proc, &load, json!({"sessionId": "s1"}));

        let replayed = replay_proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(replayed.len(), 4, "expected recovered 404 hidden and unrecovered 500 as Error row: {replayed:?}");
        assert!(matches!(&replayed[0], ChatRow::User { text, .. } if text == "Inspect readiness"));
        assert!(matches!(&replayed[1], ChatRow::Assistant { text, .. } if text == "Readiness analysis finished."));
        assert!(matches!(&replayed[2], ChatRow::User { text, .. } if text == "Follow up"));
        assert!(matches!(&replayed[3], ChatRow::Error { message, .. } if message == "INTERNAL (code 500): Internal error encountered."));
    }

    #[test]
    fn antigravity_split_transient_error_retracts_exhausts_to_error_and_cancels_cleanly() {
        // Case 1: split chunks retract cleanly and convert to a proper Error row after retries exhaust.
        let app = ctx("acp-agy-transient-exhaust");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;

        dispatch(&proc, "Check status", &[]).unwrap();
        let mut current = written(&mut agent);
        for attempt in 0..=transient_upstream_retry_delays().len() {
            notify(&app, &proc, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "\n⚠ "}}));
            notify(&app, &proc, json!({
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": "NOT_FOUND (code 404): Requested entity was not found.\n"}
            }));
            assert!(proc.timeline.lock().unwrap().rows.is_empty(), "split transient warning must be retracted immediately");
            reply(&app, &proc, &current, json!({"stopReason": "end_turn"}));
            if attempt < transient_upstream_retry_delays().len() {
                current = written(&mut agent);
                assert_eq!(current["method"], "session/prompt");
                assert_eq!(current["params"]["prompt"][0]["text"], TRANSIENT_RECOVERY_PROMPT);
            }
        }
        assert!(!proc.turn.lock().unwrap().running);
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert!(!rows.iter().any(|r| matches!(r, ChatRow::Assistant { .. })));
        assert!(rows.iter().any(|r| {
            matches!(r, ChatRow::Error { message, .. } if message == "NOT_FOUND (code 404): Requested entity was not found.")
        }));

        // Case 2: canceling while sleeping in transient backoff aborts without sending a retry prompt.
        let app2 = ctx("acp-agy-transient-cancel");
        let (proc2, mut agent2) = wired(SessionKind::Antigravity);
        *proc2.agent_session_id.lock().unwrap() = Some("s2".into());
        proc2.ready.store(true, Ordering::Relaxed);
        proc2.turn.lock().unwrap().running = true;

        dispatch(&proc2, "Run task", &[]).unwrap();
        let first = written(&mut agent2);
        notify(&app2, &proc2, json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "\n⚠ request failed: Post \"https://daily-cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse\": EOF\n"}
        }));
        reply(&app2, &proc2, &first, json!({"stopReason": "end_turn"}));
        assert!(proc2.acp.lock().unwrap().retrying_prompt);

        proc2.turn.lock().unwrap().interrupted = true;
        cancel(&proc2).unwrap();
        let cancel_msg = written(&mut agent2);
        assert_eq!(cancel_msg["method"], "session/cancel");
        std::thread::sleep(Duration::from_millis(60));
        assert!(!proc2.acp.lock().unwrap().retrying_prompt);
        assert!(proc2.acp.lock().unwrap().prompt.is_none());
    }

    #[test]
    fn antigravity_empty_current_model_on_open_selects_first_choice_and_plan_model_fallback_recovers() {
        // Case 1: `session/new` returns `currentValue: ""` with available model options while `proc.model`
        // is `None`. Startup must proactively issue `session/set_config_option` for the first available
        // model before marking the session ready, preventing `plan model not specified` on turn 1.
        let app = ctx("acp-agy-empty-current-model");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        proc.ready.store(false, Ordering::Relaxed);
        *proc.mode.lock().unwrap() = "default".into();
        *proc.model.lock().unwrap() = None;

        bootstrap(&app, &proc, None, Some("/w"), None);
        let init = written(&mut agent);
        reply(&app, &proc, &init, json!({"agentCapabilities": {"loadSession": true}}));
        let open = written(&mut agent);
        reply(&app, &proc, &open, json!({
            "sessionId": "agy-empty-model-1",
            "modes": {
                "currentModeId": "default",
                "availableModes": [{"id": "default", "name": "Default"}]
            },
            "configOptions": [{
                "type": "select",
                "id": "model",
                "category": "model",
                "currentValue": "",
                "options": [
                    {"value": "claude-opus-4-6-thinking", "name": "Claude Opus 4.6 Thinking"},
                    {"value": "gemini-4-neon", "name": "Gemini 4 Neon"}
                ]
            }]
        }));
        assert!(!proc.ready.load(Ordering::Relaxed), "must wait for initial model switch");
        let set_model_req = written(&mut agent);
        assert_eq!(set_model_req["method"], "session/set_config_option");
        assert_eq!(set_model_req["params"]["configId"], "model");
        assert_eq!(set_model_req["params"]["value"], "claude-opus-4-6-thinking");
        reply(&app, &proc, &set_model_req, json!({
            "configOptions": [{
                "type": "select",
                "id": "model",
                "category": "model",
                "currentValue": "claude-opus-4-6-thinking",
                "options": [
                    {"value": "claude-opus-4-6-thinking", "name": "Claude Opus 4.6 Thinking"},
                    {"value": "gemini-4-neon", "name": "Gemini 4 Neon"}
                ]
            }]
        }));
        assert!(proc.ready.load(Ordering::Relaxed));
        assert_eq!(proc.model.lock().unwrap().as_deref(), Some("claude-opus-4-6-thinking"));

        // Case 2: if `proc.model` holds a retired ID and `plan model not specified` fires,
        // `try_sync_antigravity_plan_model` falls back to the first live choice in `catalog.models`.
        let app2 = ctx("acp-agy-retired-plan-fallback");
        let (proc2, mut agent2) = wired(SessionKind::Antigravity);
        *proc2.agent_session_id.lock().unwrap() = Some("s-retired".into());
        *proc2.model.lock().unwrap() = Some("retired-model-v0".into());
        {
            let mut state = proc2.acp.lock().unwrap();
            state.catalog.absorb_options(&[json!({
                "type": "select",
                "id": "model",
                "category": "model",
                "currentValue": "",
                "options": [
                    {"value": "claude-opus-4-6-thinking", "name": "Claude Opus 4.6 Thinking"}
                ]
            })]);
        }
        proc2.ready.store(true, Ordering::Relaxed);
        proc2.turn.lock().unwrap().running = true;

        dispatch(&proc2, "Run check", &[]).unwrap();
        let first = written(&mut agent2);
        notify(&app2, &proc2, json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "\n⚠ failed to construct executor: plan model not specified; you must specify a valid model\n"}
        }));
        reply(&app2, &proc2, &first, json!({"stopReason": "end_turn"}));

        let attempt1 = written(&mut agent2);
        assert_eq!(attempt1["method"], "session/set_config_option");
        assert_eq!(attempt1["params"]["value"], "retired-model-v0");
        handle_line(&app2, "s", &proc2, &json!({
            "jsonrpc": "2.0",
            "id": attempt1["id"],
            "error": {"code": -32603, "message": "invalid model"}
        }).to_string());

        let attempt2 = written(&mut agent2);
        assert_eq!(attempt2["method"], "session/set_config_option");
        assert_eq!(attempt2["params"]["value"], "claude-opus-4-6-thinking");
        reply(&app2, &proc2, &attempt2, json!({}));

        let retried = written(&mut agent2);
        assert_eq!(retried["method"], "session/prompt");
        assert_eq!(proc2.model.lock().unwrap().as_deref(), Some("claude-opus-4-6-thinking"));
        notify(&app2, &proc2, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "OK"}}));
        reply(&app2, &proc2, &retried, json!({"stopReason": "end_turn"}));
        assert!(!proc2.turn.lock().unwrap().running);
    }

    #[test]
    fn antigravity_network_issue_warning_and_split_401_retract_and_mark_error_rotate() {
        // Generic `There was a network issue connecting to the server, please try again.` warning
        // retries transparently via `TRANSIENT_RECOVERY_PROMPT` without leaking the warning bubble.
        let app = ctx("acp-agy-generic-net-issue");
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.agent_session_id.lock().unwrap() = Some("s-net".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;

        dispatch(&proc, "Explain architecture", &[]).unwrap();
        let first = written(&mut agent);
        notify(&app, &proc, json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "\n⚠ There was a network issue connecting to the server, please try again.\n"}
        }));
        assert!(proc.timeline.lock().unwrap().rows.is_empty());
        reply(&app, &proc, &first, json!({"stopReason": "end_turn"}));

        let retry = written(&mut agent);
        assert_eq!(retry["method"], "session/prompt");
        assert_eq!(retry["params"]["prompt"][0]["text"], TRANSIENT_RECOVERY_PROMPT);
        notify(&app, &proc, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "Architecture summary."}}));
        reply(&app, &proc, &retry, json!({"stopReason": "end_turn"}));
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], ChatRow::Assistant { text, .. } if text == "Architecture summary."));
        assert!(!proc.acp.lock().unwrap().needs_error_rotate);

        // Split `⚠ ` + `UNAUTHENTICATED (code 401)...` chunks retract the partial bubble immediately.
        let app2 = ctx("acp-agy-split-401");
        let (proc2, mut agent2) = wired(SessionKind::Antigravity);
        *proc2.agent_session_id.lock().unwrap() = Some("s-401".into());
        proc2.ready.store(true, Ordering::Relaxed);
        proc2.turn.lock().unwrap().running = true;

        dispatch(&proc2, "Check auth", &[]).unwrap();
        let _p = written(&mut agent2);
        notify(&app2, &proc2, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "\n⚠ "}}));
        assert_eq!(proc2.timeline.lock().unwrap().rows.len(), 1);
        notify(&app2, &proc2, json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "UNAUTHENTICATED (code 401): Request had invalid authentication credentials.\n"}
        }));
        assert!(proc2.timeline.lock().unwrap().rows.is_empty(), "partial split 401 bubble must retract");
        assert!(proc2.acp.lock().unwrap().turn_auth_error.is_some());
    }

    #[test]
    fn antigravity_steps_db_outcomes_interaction_questions_and_watchdog_behave_safely() {
        let dir = std::env::temp_dir().join(format!("vlx-acp-db-outcomes-{}", uuid::Uuid::new_v4()));
        let conv_dir = dir.join("conversations");
        std::fs::create_dir_all(&conv_dir).unwrap();
        let conv_id = "11111111-2222-3333-4444-555555555555";
        let db_path = conv_dir.join(format!("{conv_id}.db"));
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE steps (
                    idx INTEGER PRIMARY KEY,
                    step_type INTEGER NOT NULL,
                    step_payload BLOB NOT NULL
                );",
            )
            .unwrap();
        }

        let app = ctx("acp-agy-db-outcomes");
        let sid_owned = format!("s-db-{}", uuid::Uuid::new_v4());
        let sid = sid_owned.as_str();
        {
            let conn = app.db().conn.lock().unwrap();
            crate::db::system_collections::ensure(&conn).unwrap();
            crate::db::repo::create_session_identified(
                &conn,
                sid,
                crate::db::system_collections::CHAT,
                None,
                "agy-db",
                SessionKind::Antigravity,
                None,
                None,
                None,
                None,
                None,
                Some(conv_id),
                Some("bypassPermissions"),
                None,
                None,
                None,
                Some("chat"),
            )
            .unwrap();
        }
        let (proc, mut agent) = wired(SessionKind::Antigravity);
        *proc.mode.lock().unwrap() = "bypassPermissions".into();
        *proc.agent_session_id.lock().unwrap() = Some(conv_id.into());
        proc.ready.store(true, Ordering::Relaxed);

        // 1) Hidden DB `UsageLimit` marks `usage_limit_hit`, `needs_error_rotate`, and captures `Reset`.
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "Generate report", &[]).unwrap();
        proc.acp.lock().unwrap().outcome_watch = Some(
        PromptOutcomeWatch::capture_at(Some(db_path.clone())),
        );
        let p1 = written(&mut agent);
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO steps (idx, step_type, step_payload) VALUES (0, 17, ?1)",
                [json!({
                    "errorMessage": {
                        "error": {
                            "userErrorMessage": "Model quota reached.",
                            "details": r#"{"error":{"status":"RESOURCE_EXHAUSTED","details":[{"metadata":{"quotaResetDelay":"3m"}}]}}"#
                        }
                    }
                })
                .to_string()
                .as_bytes()],
            )
            .unwrap();
        }
        handle_line(&app, sid, &proc, &json!({"jsonrpc": "2.0", "id": p1["id"], "result": {"stopReason": "end_turn"}}).to_string());
        assert!(proc.acp.lock().unwrap().needs_error_rotate);
        assert!(
            crate::agent::chat::auto_continue::waiting(sid).is_some_and(|w| w.continue_at > 0),
            "expected auto_continue wait scheduled after Antigravity quota exhaustion"
        );

        // 2) Hidden DB `InvalidThinkingSignature` unbinds the broken native conversation and carries
        // recent thread context into the next fresh turn's prompt.
        proc.timeline.lock().unwrap().replace_all(vec![
            super::super::user_row("u1".into(), "Refactor auth module", vec![], None),
            ChatRow::Assistant {
                id: "a1".into(),
                text: "I inspected readiness.rs and engine.rs.".into(),
                streaming: false,
                model: None,
                at: None,
                duration_ms: None,
            },
        ]);
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "Continue refactor", &[]).unwrap();
        proc.acp.lock().unwrap().outcome_watch = Some(
        PromptOutcomeWatch::capture_at(Some(db_path.clone())),
        );
        let p2 = written(&mut agent);
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO steps (idx, step_type, step_payload) VALUES (1, 17, ?1)",
                [json!({
                    "errorMessage": {
                        "error": {
                            "shortError": "INVALID_ARGUMENT (code 400): messages.2.content.0: Invalid `signature` in `thinking` block"
                        }
                    }
                })
                .to_string()
                .as_bytes()],
            )
            .unwrap();
        }
        handle_line(&app, sid, &proc, &json!({"jsonrpc": "2.0", "id": p2["id"], "result": {"stopReason": "end_turn"}}).to_string());
        assert!(proc.agent_session_id.lock().unwrap().is_none(), "broken native session must be unbound");
        assert!(proc.acp.lock().unwrap().needs_error_rotate);
        let db_session = crate::db::repo::get_session(&app.db().conn.lock().unwrap(), sid).unwrap().unwrap();
        assert!(db_session.agent_session_id.is_none(), "DB agent_session_id must be cleared");

        // Next turn on a fresh native session carries bounded thread context automatically.
        *proc.agent_session_id.lock().unwrap() = Some("22222222-3333-4444-5555-666666666666".into());
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "Finish tests", &[]).unwrap();
        let p3 = written(&mut agent);
        let carried_text = p3["params"]["prompt"][0]["text"].as_str().unwrap();
        assert!(carried_text.contains("Prior conversation context from this thread"));
        assert!(carried_text.contains("Refactor auth module"));
        assert!(carried_text.contains("Current user request:\nFinish tests"));

        // 3) `interaction_*` fixed-choice question stays interactive even in `bypassPermissions`
        // and maps the chosen option label back to its `optionId`.
        handle_line(&app, sid, &proc, &json!({
            "jsonrpc": "2.0",
            "id": 77,
            "method": "session/request_permission",
            "params": {
                "toolCall": {
                    "toolCallId": "interaction_0",
                    "title": "Select target environment",
                    "rawInput": {"question": "Which environment should we deploy to?"}
                },
                "options": [
                    {"optionId": "opt-staging", "name": "Staging", "kind": "allow_once"},
                    {"optionId": "opt-prod", "name": "Production", "kind": "allow_once"}
                ]
            }
        }).to_string());
        approve_pending(&app, sid, &proc);
        let card_id = {
            let perms = proc.permissions.lock().unwrap();
            assert_eq!(perms.len(), 1, "interaction_* question must not be auto-approved in bypass mode");
            let (id, card) = perms.iter().next().unwrap();
            assert_eq!(card["tool_name"], "AskUserQuestion");
            id.clone()
        };
        respond(
            &app,
            sid,
            &proc,
            &card_id,
            true,
            Some(&json!({"answers": {"Which environment should we deploy to?": "Production"}})),
            None,
            None,
        )
        .unwrap();
        let perm_reply = written(&mut agent);
        assert_eq!(perm_reply["id"], 77);
        assert_eq!(perm_reply["result"]["outcome"]["optionId"], "opt-prod");
        handle_line(&app, sid, &proc, &json!({"jsonrpc": "2.0", "id": p3["id"], "result": {"stopReason": "end_turn"}}).to_string());

        // 4) Prompt inactivity watchdog pauses while a tool is running or while Antigravity's
        // native conversation DB is still receiving writes, then fires once genuinely silent.
        let _ = app.chat().claim_antigravity_stall_recovery(sid); // Exhaust restart budget to test direct settle.
        proc.acp.lock().unwrap().prompt_inactivity_limit_ms = Some(120);
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "Hang test", &[]).unwrap();
        proc.acp.lock().unwrap().outcome_watch = Some(
            PromptOutcomeWatch::capture_at(Some(db_path.clone())),
        );
        let _p4 = written(&mut agent);
        start_prompt_watchdog(&app, sid, &proc);
        handle_line(&app, sid, &proc, &json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "22222222-3333-4444-5555-666666666666",
                "update": {
                    "sessionUpdate": "tool_call",
                    "toolCallId": "slow-tool-1",
                    "title": "Running cargo build",
                    "kind": "execute",
                    "status": "in_progress"
                }
            }
        }).to_string());
        std::thread::sleep(Duration::from_millis(160));
        assert!(proc.turn.lock().unwrap().running, "watchdog must stay paused while a tool is running");

        handle_line(&app, sid, &proc, &json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "22222222-3333-4444-5555-666666666666",
                "update": {
                    "sessionUpdate": "tool_call_update",
                    "toolCallId": "slow-tool-1",
                    "status": "completed"
                }
            }
        }).to_string());

        // Growing `step_payload` in the native DB keeps the watchdog alive even with silent stdout.
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO steps (idx, step_type, step_payload) VALUES (2, 15, ?1)",
                [vec![b'a'; 64]],
            )
            .unwrap();
        }
        for round in 1..=3 {
            std::thread::sleep(Duration::from_millis(55));
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute(
                "UPDATE steps SET step_payload = ?1 WHERE idx = 2",
                [vec![b'a'; 64 * (round + 1)]],
            )
            .unwrap();
        }
        assert!(
            proc.turn.lock().unwrap().running,
            "watchdog must stay alive while native conversation DB keeps growing"
        );

        let stall_deadline = Instant::now() + Duration::from_secs(2);
        while proc.turn.lock().unwrap().running && Instant::now() < stall_deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!proc.turn.lock().unwrap().running, "watchdog must end wedged silent prompt");
        assert!(proc.acp.lock().unwrap().needs_error_rotate);
        assert!(proc.timeline.lock().unwrap().rows.iter().any(|r| {
            matches!(r, ChatRow::Error { message, .. } if message.contains("stopped sending updates for too long"))
        }));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
