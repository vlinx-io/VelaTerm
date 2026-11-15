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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{emit, emit_state, handle_turn_end, now_ms, start_waiting_message, verify_native_identity, ChatProcess, ChatRow};
use crate::agent::chat::acp_protocol::{self as wire, Catalog, Incoming, Mapper};
use crate::agent::chat::codex_protocol::CollaborationModePreset;
use crate::agent::chat::protocol::ChatImage;
use crate::host::AppCtx;
use crate::models::SessionKind;
use crate::pty::AgentState;

/// Journal lines are written in batches; a turn that streams for a long time still lands on disk this often.
const JOURNAL_BATCH: usize = 64;

fn native_cwd(cwd: &str, windows: bool) -> String {
    if windows { cwd.replace('/', "\\") } else { cwd.to_string() }
}

/// Everything the engine keeps about the ACP side of one session.
#[derive(Default)]
pub(super) struct AcpState {
    pub(super) mapper: Mapper,
    pub(super) catalog: Catalog,
    /// Whether the agent accepts image blocks in a prompt.
    pub(super) images: bool,
    load_session: bool,
    /// Whether the agent can reopen a conversation without replaying it (`session/resume`).
    resume_session: bool,
    /// The conversation to continue, set before the handshake.
    resume: Option<String>,
    /// The working directory the session was opened in; ACP requires an absolute one.
    cwd: String,
    /// The request id of the running prompt.
    prompt: Option<String>,
    /// Requests the agent is waiting on, by permission card id: the id it sent, and its parameters.
    pub(super) requests: HashMap<String, (Value, Value)>,
    /// While `session/load` replays: its rows and updates, and the rows the old recording had.
    loading: Option<Replay>,
    journal: Option<PathBuf>,
    pending_journal: Vec<String>,
    /// The collaboration mode saved with the session, applied once the session reports its own.
    desired_mode: Option<String>,
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

    /// Give each replayed message the time and duration its recorded counterpart had.
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
        (times, durations)
    }
}

/// The conversation's journal file, for agents whose own recording VelaTerm cannot read. Grok's native
/// stream and DeepSeek Harness's session log already are that recording.
fn journal_path(app: &AppCtx, kind: SessionKind, native: &str) -> Option<PathBuf> {
    if matches!(kind, SessionKind::Grok | SessionKind::Deepseek) { return None; }
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
    bootstrap_for_platform(app, proc, resume, cwd, collaboration_mode, cfg!(windows));
}

fn bootstrap_for_platform(app: &AppCtx, proc: &Arc<ChatProcess>, resume: Option<&str>, cwd: Option<&str>, collaboration_mode: Option<&str>, windows: bool) {
    {
        let mut state = proc.acp.lock().unwrap();
        state.mapper = Mapper::for_kind(proc.kind);
        state.resume = resume.map(str::to_string);
        state.cwd = cwd.map(|path| native_cwd(path, windows))
            .or_else(|| crate::host::home_dir().map(|home| home.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "/".into());
        state.journal = resume.and_then(|native| journal_path(app, proc.kind, native));
        state.desired_mode = collaboration_mode.map(str::to_string).filter(|mode| !mode.is_empty());
    }
    let id = proc.request_id("acp_initialize");
    let _ = proc.write(&wire::initialize(&id));
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
            Some(error) => {
                proc.acp.lock().unwrap().loading = None;
                fail_start(app, session_id, proc, &error);
            }
            None => on_open(app, session_id, proc, &result.unwrap_or(Value::Null), kind != Some("acp_session_new")),
        },
        Some("prompt") => on_prompt_end(app, session_id, proc, id, result, error),
        _ => {}
    }
}

fn on_initialize(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, result: &Value) {
    let capabilities = result.get("agentCapabilities").cloned().unwrap_or(Value::Null);
    let (resume, cwd) = {
        let mut state = proc.acp.lock().unwrap();
        state.load_session = capabilities.get("loadSession").and_then(Value::as_bool).unwrap_or(false);
        state.resume_session = capabilities.pointer("/sessionCapabilities/resume").is_some_and(|resume| !resume.is_null());
        state.images = capabilities.pointer("/promptCapabilities/image").and_then(Value::as_bool).unwrap_or(false);
        (state.resume.clone(), state.cwd.clone())
    };
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
    let (presets, current) = {
        let mut state = proc.acp.lock().unwrap();
        state.catalog.absorb_session(result);
        if state.journal.is_none() { state.journal = journal_path(app, proc.kind, &native); }
        remember_catalog(app, proc.kind, &state.catalog);
        let presets = presets(&state.catalog);
        (presets, state.catalog.mode.clone())
    };
    apply_saved_selection(app, session_id, proc, &presets, current);
    emit(app, session_id, json!({"type":"settingsChanged","model":proc.model.lock().unwrap().clone(),
        "effort":proc.effort.lock().unwrap().clone()}));
    if !proc.ready.swap(true, Ordering::Relaxed) {
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
    emit(app, session_id, json!({"type":"session","agentSessionId":native,"model":proc.model.lock().unwrap().clone()}));
    true
}

/// Session modes as collaboration presets, which is how the view offers a choice of working style.
fn presets(catalog: &Catalog) -> Vec<CollaborationModePreset> {
    catalog.modes.iter().map(|mode| CollaborationModePreset {
        name: mode.name.clone(), mode: mode.id.clone(), reasoning_effort: None, description: mode.description.clone(),
    }).collect()
}

/// Bring the opened session to the mode, model and reasoning level saved with this conversation.
fn apply_saved_selection(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, presets: &[CollaborationModePreset], current: Option<String>) {
    let desired_mode = proc.acp.lock().unwrap().desired_mode.take();
    let mut selected = current.clone();
    if let Some(mode) = desired_mode.filter(|mode| presets.iter().any(|preset| &preset.mode == mode)) {
        if Some(&mode) != current.as_ref() && set_mode_request(proc, &mode, false).is_ok() { selected = Some(mode); }
    }
    *proc.collaboration_modes.lock().unwrap() = presets.to_vec();
    *proc.collaboration_mode.lock().unwrap() = selected.clone();
    emit(app, session_id, json!({"type":"collaborationModes","modes":presets,"mode":selected}));

    let catalog = proc.acp.lock().unwrap().catalog.clone();
    if catalog.model_switchable() {
        let desired = proc.model.lock().unwrap().clone();
        match desired.filter(|model| catalog.model.as_deref() != Some(model.as_str()) && catalog.models.iter().any(|choice| &choice.id == model)) {
            Some(model) => { if set_model_request(proc, &model, false).is_err() { *proc.model.lock().unwrap() = catalog.model.clone(); } }
            None => *proc.model.lock().unwrap() = catalog.model.clone(),
        }
    }
    if catalog.effort_switchable() {
        let desired = proc.effort.lock().unwrap().clone();
        match desired.filter(|effort| catalog.effort.as_deref() != Some(effort.as_str()) && catalog.efforts.iter().any(|choice| &choice.id == effort)) {
            Some(effort) => { if set_effort_request(proc, &effort, false).is_err() { *proc.effort.lock().unwrap() = catalog.effort.clone(); } }
            None => *proc.effort.lock().unwrap() = catalog.effort.clone(),
        }
    }
}

/// The agent refused to start the conversation. Signing in is the one cause the person can act on, so it
/// becomes a stable code the view explains; anything else is shown as the agent wrote it.
fn fail_start(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, error: &Value) {
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
    match update.get("sessionUpdate").and_then(Value::as_str).unwrap_or("") {
        "available_commands_update" => {
            let commands = wire::commands(update);
            *proc.commands.lock().unwrap() = commands.clone();
            emit(app, session_id, json!({"type":"commands","commands":commands}));
            return;
        }
        "current_mode_update" => {
            if let Some(mode) = update.get("currentModeId").and_then(Value::as_str) {
                proc.acp.lock().unwrap().catalog.mode = Some(mode.to_string());
                adopt_mode(app, session_id, proc, mode);
            }
            return;
        }
        "config_option_update" => {
            let options = update.get("configOptions").and_then(Value::as_array).cloned().unwrap_or_default();
            let (mode, model, effort) = {
                let mut state = proc.acp.lock().unwrap();
                state.catalog.absorb_options(&options);
                remember_catalog(app, proc.kind, &state.catalog);
                (state.catalog.mode.clone(), state.catalog.model.clone(), state.catalog.effort.clone())
            };
            if let Some(mode) = mode { adopt_mode(app, session_id, proc, &mode); }
            let catalog = proc.acp.lock().unwrap().catalog.clone();
            if catalog.model_switchable() && model.is_some() { *proc.model.lock().unwrap() = model; }
            if catalog.effort_switchable() && effort.is_some() { *proc.effort.lock().unwrap() = effort; }
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
        "session_info_update" => return,
        _ => {}
    }
    let mut state = proc.acp.lock().unwrap();
    let rows = state.mapper.update(update);
    if let Some(replay) = state.loading.as_mut() {
        replay.updates.push((update.clone(), rows.first().map(|row| row.id().to_string())));
        for row in rows { replay.place(row); }
        return;
    }
    if state.journal.is_some() {
        state.pending_journal.push(wire::journal_line(update, now_ms() as i64));
        if state.pending_journal.len() >= JOURNAL_BATCH { flush_journal_locked(&mut state); }
    }
    drop(state);
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

/// The agent changed its own mode, or answered a change: the view's capsule follows.
fn adopt_mode(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, mode: &str) {
    let previous = proc.collaboration_mode.lock().unwrap().replace(mode.to_string());
    if previous.as_deref() == Some(mode) { return; }
    let _ = crate::db::repo::set_collaboration_mode(&app.db().conn.lock().unwrap(), session_id, mode);
    emit(app, session_id, json!({"type":"collaborationModeChanged","mode":mode}));
}

fn handle_request(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, id: Value, method: &str, params: Value) {
    if method != "session/request_permission" {
        // Neither file system nor terminal access was offered; anything else is unknown to this client.
        let _ = proc.write(&wire::error_response(&id, -32601, &format!("{method} is not supported by this client")));
        return;
    }
    if proc.mode.lock().unwrap().as_str() == "bypassPermissions" {
        let option = wire::choose_option(params.get("options"), true, None);
        let _ = proc.write(&wire::permission_outcome(&id, option.as_deref()));
        return;
    }
    let card = format!("acp-{}", wire::id_key(&id));
    let payload = wire::permission_payload(proc.kind, &card, &params, Some(&proc.acp.lock().unwrap().mapper));
    proc.acp.lock().unwrap().requests.insert(card.clone(), (id, params));
    proc.permissions.lock().unwrap().insert(card, payload.clone());
    emit(app, session_id, json!({"type":"permission","request":payload}));
    emit_state(app, session_id, AgentState::Asking);
}

/// Answer a permission card. A standing rule the card offered travels in `updated_permissions`.
pub(super) fn respond(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, card: &str, allow: bool, updated_permissions: Option<&Value>) -> Result<(), String> {
    let (id, params) = proc.acp.lock().unwrap().requests.get(card).cloned()
        .ok_or("That permission request is no longer waiting for an answer")?;
    let always = updated_permissions.and_then(Value::as_array).into_iter().flatten()
        .find(|value| value.get("type").and_then(Value::as_str) == Some("acpAllowAlways"))
        .and_then(|value| value.get("optionId").and_then(Value::as_str));
    let option = wire::choose_option(params.get("options"), allow, always);
    proc.write(&wire::permission_outcome(&id, option.as_deref()))?;
    resolve(app, session_id, proc, card);
    Ok(())
}

fn resolve(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, card: &str) {
    proc.acp.lock().unwrap().requests.remove(card);
    proc.permissions.lock().unwrap().remove(card);
    emit(app, session_id, json!({"type":"permissionResolved","id":card}));
    if !proc.permissions.lock().unwrap().is_empty() { return; }
    emit_state(app, session_id, if proc.turn.lock().unwrap().running { AgentState::Working } else { AgentState::Waiting });
}

/// Let every waiting request through, used when the session switches to bypass.
pub(super) fn approve_pending(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let requests: Vec<(String, Value, Value)> = proc.acp.lock().unwrap().requests.iter()
        .map(|(card, (id, params))| (card.clone(), id.clone(), params.clone())).collect();
    for (card, id, params) in requests {
        let option = wire::choose_option(params.get("options"), true, None);
        if proc.write(&wire::permission_outcome(&id, option.as_deref())).is_ok() { resolve(app, session_id, proc, &card); }
    }
}

/// A finished turn leaves no question open: any request still waiting is answered as cancelled, and every
/// card is cleared, including those whose answer `cancel` already sent.
fn cancel_pending(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let requests: Vec<Value> = proc.acp.lock().unwrap().requests.drain().map(|(_, (id, _))| id).collect();
    for id in requests { let _ = proc.write(&wire::permission_outcome(&id, None)); }
    let cards: Vec<String> = proc.permissions.lock().unwrap().keys().cloned().collect();
    for card in cards { resolve(app, session_id, proc, &card); }
}

/// The prompt answered: the turn is over.
fn on_prompt_end(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, id: &str, result: Option<Value>, error: Option<Value>) {
    let (canceled, started_at) = {
        let turn = proc.turn.lock().unwrap();
        (turn.interrupted, turn.started_at)
    };
    {
        let mut state = proc.acp.lock().unwrap();
        if state.prompt.as_deref() == Some(id) { state.prompt = None; }
        let rows: Vec<ChatRow> = state.mapper.finish().into_iter().chain(state.mapper.settle_running(canceled)).collect();
        if let (Some(started_at), true) = (started_at, state.journal.is_some()) {
            let now = now_ms();
            state.pending_journal.push(wire::journal_turn_end(now.saturating_sub(started_at), now as i64));
        }
        flush_journal_locked(&mut state);
        drop(state);
        let mut timeline = proc.timeline.lock().unwrap();
        for row in rows { timeline.upsert(row); }
    }
    cancel_pending(app, session_id, proc);
    let failure = match &error {
        Some(error) if wire::auth_required(error) => {
            emit(app, session_id, json!({"type":"error","message":format!("agent_sign_in_required:{}", proc.kind.as_str())}));
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
    if let Some(message) = failure.filter(|_| !canceled) {
        let turn = proc.acp.lock().unwrap().mapper.turn();
        proc.timeline.lock().unwrap().upsert(ChatRow::Error { id: format!("e-acp-{turn}"), message });
    }
    handle_turn_end(app, session_id, proc, "success", None);
}

fn flush_journal_locked(state: &mut AcpState) {
    let Some(path) = state.journal.clone() else { state.pending_journal.clear(); return };
    let lines = std::mem::take(&mut state.pending_journal);
    if let Err(error) = wire::append_journal(&path, &lines) {
        crate::diagnostic_warn!("chat: ACP journal append failed: {error}");
    }
}

/// The process is gone: close whatever was streaming and keep the journal complete.
pub(super) fn finish(proc: &Arc<ChatProcess>) {
    let mut state = proc.acp.lock().unwrap();
    let rows: Vec<ChatRow> = state.mapper.finish().into_iter().chain(state.mapper.settle_running(true)).collect();
    flush_journal_locked(&mut state);
    drop(state);
    let mut timeline = proc.timeline.lock().unwrap();
    for row in rows { timeline.upsert(row); }
}

// ─────────────────────────── Outgoing ───────────────────────────

pub(super) fn validate(proc: &ChatProcess, behavior: &str, image_count: usize) -> Result<(), String> {
    let images = proc.acp.lock().unwrap().images;
    wire::validate(behavior, image_count, images)
}

pub(super) fn dispatch(proc: &Arc<ChatProcess>, text: &str, images: &[ChatImage]) -> Result<(), String> {
    let session = proc.agent_session_id.lock().unwrap().clone().ok_or("The agent has not opened its conversation yet")?;
    let id = proc.request_id("prompt");
    {
        let mut state = proc.acp.lock().unwrap();
        state.mapper.begin_turn();
        state.prompt = Some(id.clone());
        if state.journal.is_some() {
            state.pending_journal.push(wire::journal_prompt(text, now_ms() as i64));
            flush_journal_locked(&mut state);
        }
    }
    let result = proc.write(&wire::prompt(&id, &session, text, images));
    if result.is_err() {
        proc.pending.lock().unwrap().remove(&id);
        proc.acp.lock().unwrap().prompt = None;
    }
    result
}

/// Cancel the running prompt. Waiting requests are answered at once, because an agent may hold the
/// prompt open until they are; their cards are cleared when the prompt answers.
pub(super) fn cancel(proc: &Arc<ChatProcess>) -> Result<(), String> {
    let session = proc.agent_session_id.lock().unwrap().clone().ok_or("The agent has not opened its conversation yet")?;
    proc.write(&wire::cancel(&session))?;
    let requests: Vec<Value> = proc.acp.lock().unwrap().requests.drain().map(|(_, (id, _))| id).collect();
    for id in requests { let _ = proc.write(&wire::permission_outcome(&id, None)); }
    Ok(())
}

fn session(proc: &ChatProcess) -> Result<String, String> {
    proc.agent_session_id.lock().unwrap().clone().ok_or_else(|| "The agent has not opened its conversation yet".into())
}

/// Switch the session mode. `wait` blocks for the answer; the handshake sends without waiting because the
/// reader thread that would deliver the answer is the one running it.
fn set_mode_request(proc: &Arc<ChatProcess>, mode: &str, wait: bool) -> Result<(), String> {
    let session = session(proc)?;
    let option = proc.acp.lock().unwrap().catalog.mode_option.clone();
    let make = |id: &str| match &option {
        Some(option) => wire::set_config_option(id, &session, option, mode),
        None => wire::set_mode(id, &session, mode),
    };
    send(proc, "acp_set_mode", wait, make)?;
    proc.acp.lock().unwrap().catalog.mode = Some(mode.to_string());
    Ok(())
}

fn set_model_request(proc: &Arc<ChatProcess>, model: &str, wait: bool) -> Result<(), String> {
    let session = session(proc)?;
    let catalog = proc.acp.lock().unwrap().catalog.clone();
    let make = |id: &str| match (&catalog.model_option, catalog.models_from_state) {
        (Some(option), false) => wire::set_config_option(id, &session, option, model),
        _ => wire::set_model(id, &session, model),
    };
    send(proc, "acp_set_model", wait, make)?;
    proc.acp.lock().unwrap().catalog.model = Some(model.to_string());
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

fn send(proc: &Arc<ChatProcess>, kind: &'static str, wait: bool, make: impl FnOnce(&str) -> Value) -> Result<(), String> {
    if wait {
        proc.request_and_wait(kind, make).map(|_| ())
    } else {
        let id = proc.request_id(kind);
        proc.write(&make(&id))
    }
}

pub(super) fn set_mode(proc: &Arc<ChatProcess>, mode: &str) -> Result<(), String> {
    if !proc.acp.lock().unwrap().catalog.modes.iter().any(|choice| choice.id == mode) {
        return Err(format!("{} has no mode named {mode}", wire::label(proc.kind)));
    }
    set_mode_request(proc, mode, true)
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
pub(super) fn chat_models(proc: &Arc<ChatProcess>) -> Option<Vec<Value>> {
    let catalog = proc.acp.lock().unwrap().catalog.clone();
    if !catalog.model_switchable() { return None; }
    Some(catalog_models(&catalog))
}

fn catalog_models(catalog: &Catalog) -> Vec<Value> {
    let efforts: Vec<&str> = catalog.efforts.iter().map(|choice| choice.id.as_str()).collect();
    catalog.models.iter().map(|model| json!({
        "id": model.id, "label": model.name, "description": model.description.clone().unwrap_or_default(),
        "effortLevels": efforts,
    })).collect()
}

/// Backend setting holding the last model catalogue a DeepSeek session reported.
pub const DEEPSEEK_MODELS_SETTING: &str = "deepseek.models";

/// Keep DeepSeek's catalogue for the model picker of a conversation whose agent is not running. Asking a
/// short-lived dsh instead would leave an empty conversation behind: it records a session as soon as one
/// is opened, prompt or not.
fn remember_catalog(app: &AppCtx, kind: SessionKind, catalog: &Catalog) {
    if kind != SessionKind::Deepseek || !catalog.model_switchable() { return; }
    let Ok(value) = serde_json::to_string(&catalog_models(catalog)) else { return };
    let conn = app.db().conn.lock().unwrap();
    if crate::db::repo::get_app_settings(&conn).ok().and_then(|settings| settings.get(DEEPSEEK_MODELS_SETTING).cloned()).as_deref() == Some(value.as_str()) {
        return;
    }
    if let Err(error) = crate::db::repo::set_app_settings(&conn, &HashMap::from([(DEEPSEEK_MODELS_SETTING.to_string(), value)])) {
        crate::diagnostic_warn!("chat: DeepSeek model catalogue could not be saved: {error}");
    }
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
    fn windows_cwd_is_normalized_and_unix_cwd_is_preserved() {
        assert_eq!(native_cwd("C:/Users/alice/workspace/demo", true), r"C:\Users\alice\workspace\demo");
        assert_eq!(native_cwd(r"C:\Users\alice\workspace\demo", true), r"C:\Users\alice\workspace\demo");
        assert_eq!(native_cwd("/Users/alice/workspace/demo", false), "/Users/alice/workspace/demo");
    }

    #[test]
    fn windows_cwd_uses_native_separators_for_new_load_and_resume() {
        let cwd = "C:/Users/alice/workspace/demo.worktrees/feature-1";
        let cases = [
            (Some("load-id"), json!({"loadSession":true}), "session/load"),
            (Some("resume-id"), json!({"sessionCapabilities":{"resume":{}}}), "session/resume"),
            (None, json!({}), "session/new"),
        ];
        for (resume, capabilities, method) in cases {
            let app = ctx("acp-windows-cwd");
            let (proc, mut agent) = wired(SessionKind::Grok);
            bootstrap_for_platform(&app, &proc, resume, Some(cwd), None, true);
            let initialize = written(&mut agent);
            reply(&app, &proc, &initialize, json!({"agentCapabilities":capabilities}));
            let open = written(&mut agent);
            assert_eq!(open["method"], method);
            assert_eq!(open["params"]["cwd"], r"C:\Users\alice\workspace\demo.worktrees\feature-1");
        }
    }

    #[test]
    fn a_resumed_conversation_is_replaced_by_its_replay_and_keeps_recorded_times() {
        let app = ctx("acp-resume");
        let (proc, mut agent) = wired(SessionKind::Copilot);
        // The recording drew a thought the replay leaves out, so ids differ and only the text matches.
        proc.timeline.lock().unwrap().replace_all(vec![
            ChatRow::User { id: "acp-u1".into(), text: "hi".into(), images: Vec::new(), at: Some(1_000) },
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
        respond(&app, "s", &proc, "acp-7", true, Some(&json!([{"type":"acpAllowAlways","optionId":"always"}]))).unwrap();
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
    fn deepseek_resumes_without_a_replay_and_draws_rows_its_log_replays_to() {
        let app = ctx("acp-deepseek");
        let (proc, mut agent) = wired(SessionKind::Deepseek);
        proc.ready.store(false, Ordering::Relaxed);
        // The rows on screen came from dsh's own log and must survive the resume.
        proc.timeline.lock().unwrap().replace_all(vec![
            ChatRow::User { id: "dsh-u-1".into(), text: "earlier".into(), images: Vec::new(), at: Some(1_000) },
        ]);
        bootstrap(&app, &proc, Some("s1"), Some("/w"), None);
        let initialize = written(&mut agent);
        reply(&app, &proc, &initialize, json!({"agentCapabilities":{"promptCapabilities":{"image":false},
            "sessionCapabilities":{"close":{},"list":{},"resume":{}}}}));
        let resume = written(&mut agent);
        assert_eq!((resume["method"].as_str(), resume["params"]["sessionId"].as_str(), resume["params"]["cwd"].as_str()),
            (Some("session/resume"), Some("s1"), Some("/w")));
        reply(&app, &proc, &resume, json!({"configOptions":[
            {"id":"model","category":"model","type":"select","currentValue":"[\"deepseek-official\",\"deepseek-v4-flash\"]","options":[{"group":"deepseek-official","name":"DeepSeek","options":[
                {"value":"[\"deepseek-official\",\"deepseek-v4-flash\"]","name":"deepseek-v4-flash"},{"value":"[\"deepseek-official\",\"deepseek-v4-pro\"]","name":"DeepSeek-V4-Pro"}]}]},
            {"id":"reasoning_effort","category":"thought_level","type":"select","currentValue":"high","options":[{"value":"off","name":"Off"},{"value":"high","name":"High"}]}]}));
        assert!(proc.ready.load(Ordering::Relaxed));
        assert_eq!(proc.timeline.lock().unwrap().rows.len(), 1);
        assert_eq!(chat_models(&proc).unwrap().len(), 2);
        assert!(proc.acp.lock().unwrap().journal.is_none(), "dsh's own log is the history");

        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "make a file", &[]).unwrap();
        let prompt = written(&mut agent);
        notify(&app, &proc, json!({"sessionUpdate":"agent_message_chunk","messageId":"m1","content":{"type":"text","text":"Creating it."}}));
        notify(&app, &proc, json!({"sessionUpdate":"tool_call","toolCallId":"call_1","title":"write","kind":"other","status":"in_progress",
            "rawInput":{"file_path":"/w/a.txt","content":"hi"}}));
        notify(&app, &proc, json!({"sessionUpdate":"tool_call_update","toolCallId":"call_1","status":"completed",
            "content":[{"type":"content","content":{"type":"text","text":"Created file"}}]}));
        notify(&app, &proc, json!({"sessionUpdate":"agent_thought_chunk","messageId":"m2","content":{"type":"text","text":"Done."}}));
        notify(&app, &proc, json!({"sessionUpdate":"agent_message_chunk","messageId":"m2","content":{"type":"text","text":"Created a.txt."}}));
        notify(&app, &proc, json!({"sessionUpdate":"usage_update","used":6040,"size":1000000}));
        reply(&app, &proc, &prompt, json!({"stopReason":"end_turn"}));
        let rows = proc.timeline.lock().unwrap().rows.clone();
        let ids: Vec<&str> = rows.iter().map(|row| row.id()).collect();
        assert_eq!(&ids[ids.len() - 4..], ["acp-a-m1", "acp-t-call_1", "acp-r-m2", "acp-a-m2"]);
        assert!(rows.iter().any(|row| matches!(row, ChatRow::Tool { name, status: "completed", input, .. } if name == "Write" && input["file_path"] == "/w/a.txt")));
        assert!(rows.iter().all(|row| !matches!(row, ChatRow::Assistant { streaming: true, .. })));
        let extras = proc.extras.lock().unwrap();
        assert_eq!((extras.context_tokens, extras.context_window), (Some(6040), Some(1_000_000)));
    }

    #[test]
    fn a_missing_api_key_is_a_stable_code_and_not_a_failed_turn() {
        let app = ctx("acp-deepseek-key");
        let (proc, mut agent) = wired(SessionKind::Deepseek);
        *proc.agent_session_id.lock().unwrap() = Some("s1".into());
        proc.ready.store(true, Ordering::Relaxed);
        proc.turn.lock().unwrap().running = true;
        dispatch(&proc, "hi", &[]).unwrap();
        let prompt = written(&mut agent);
        handle_line(&app, "s", &proc, &json!({"jsonrpc":"2.0","id":prompt["id"],"error":{"code":-32603,
            "message":"Internal error: turn failed: llm-deepseek: no API key for provider route \"deepseek-official\"; store DEEPSEEK_API_KEY through the credentials service"}}).to_string());
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
}
