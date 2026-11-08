//! ZCode inside the chat engine: one `zcode app-server` process, its V4 conversation subscription, and the
//! timeline it produces.
//!
//! The server owns the conversation and projects it into rows; this module subscribes to them and mirrors
//! each into the view. A new conversation is created lazily by the first message (`createSession`), a resumed
//! one is opened by subscribing to it, which also restores it from ZCode's own store. Every user turn carries
//! the session's model, reasoning level and permission mode, so a change made in the composer needs no
//! separate request: the next message applies it.
//!
//! A turn is the engine's from the moment it sends a message until the `turnHeader` row that message opened
//! reaches a terminal state. Approvals and questions appear as the snapshot's pending interactions; they
//! become permission cards and are answered with `resolveInteraction`. The legacy reverse requests the server
//! raises for the same prompts in parallel are left unanswered: the first answer to arrive wins, and ours
//! travels the V4 path.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use base64::Engine as _;
use serde_json::{json, Value};

use super::{emit, emit_state, handle_turn_end, reset_event, start_waiting_message, ChatProcess, ChatRow};
use crate::agent::chat::protocol::ChatImage;
use crate::agent::chat::zcode_protocol::{self as wire, Incoming};
use crate::host::AppCtx;
use crate::pty::AgentState;

/// Rows older than the subscription's tail are paged in after it, at most this many.
const HISTORY_ROW_LIMIT: usize = 5000;

/// Everything the engine keeps about the ZCode side of one session.
#[derive(Default)]
pub(super) struct ZcodeState {
    /// The subscription's connection identity; ZCode scopes deltas and resyncs to it.
    connection_id: String,
    subscription_id: Option<String>,
    subscribed: bool,
    /// The last frame sequence applied, so a gap can ask for a fresh snapshot.
    last_seq: Option<u64>,
    /// Raw projected rows by server row id, kept so streaming deltas can be applied to them.
    rows: HashMap<i64, Value>,
    /// Commands this engine sent with a user message, mapped to the view row that message already drew.
    own_inputs: HashMap<String, String>,
    /// The command whose turn the engine is waiting on, and the turn header it opened once seen.
    awaiting: Option<String>,
    awaited_turn: Option<i64>,
    /// A turn that started without this engine, such as a queued input ZCode drained itself.
    foreign_turn: Option<i64>,
    /// Interaction ids currently shown as cards.
    interactions: HashSet<String>,
    /// The latest error the server reported for the session, drawn when a turn fails.
    last_error: Option<String>,
    /// The model catalogue in the composer's shape.
    pub(super) models: Vec<Value>,
    /// Large frames arrive as fragments sharing a logical id.
    fragments: HashMap<String, Vec<Option<String>>>,
    history_requested: bool,
    /// A resumed conversation becomes ready with its first snapshot, not with the subscription's answer:
    /// the answer arrives before the history it opens, and a message drawn in between would sit above it.
    ready_on_snapshot: bool,
    /// A `zcode login` the account panel started and is waiting on.
    pub(in crate::agent::chat::engine) login: Option<Arc<std::sync::Mutex<std::process::Child>>>,
    /// The saved account, which the server hears from this client; see `agent::zcode_account`.
    account: Option<Arc<crate::agent::zcode_account::Account>>,
}

/// The view's code for a signed-in account the platform says has no usable plan (1113):
/// `zcode_plan_unavailable:<platform>:<the provider's own words>`.
const PLAN_UNAVAILABLE: &str = "zcode_plan_unavailable";
/// The view's code for a signed-in account ZCode lists no model for: `zcode_no_models:<platform>`. No plan
/// of that platform was found in the sign-in, which is not the same as the platform refusing one.
const NO_MODELS: &str = "zcode_no_models";

/// Z.ai and BigModel answer 1113 when the account has no plan package or its quota is spent. Said as it
/// is, it reads like a billing fault; the view names the platform signed in to and how to switch instead.
fn plan_error(proc: &ChatProcess, message: String) -> String {
    if !message.contains("[1113]") {
        return message;
    }
    let account = proc.zcode.lock().unwrap().account.clone();
    match account.and_then(|account| account.signed_in()) {
        Some(platform) => format!("{PLAN_UNAVAILABLE}:{platform}:{message}"),
        None => message,
    }
}

fn workspace(proc: &ChatProcess) -> Result<String, String> {
    proc.cwd
        .clone()
        .filter(|cwd| !cwd.trim().is_empty())
        .ok_or_else(|| "ZCode needs a working directory for this conversation".to_string())
}

/// Send the opening requests: the saved account, without which the server holds no plan and lists no model;
/// a capability probe that also waits out ZCode's storage start; and a draft session whose answer carries the
/// model catalogue and slash commands without persisting anything. The server answers them in order.
pub(super) fn bootstrap(proc: &Arc<ChatProcess>, account: Option<crate::agent::zcode_account::Account>) {
    let account = account.map(Arc::new);
    {
        let mut state = proc.zcode.lock().unwrap();
        state.connection_id = format!("velaterm-{}", uuid::Uuid::new_v4());
        state.account = account.clone();
    }
    if let Some(params) = account.and_then(|account| account.config()) {
        let id = proc.request_id("zcode_account");
        let _ = proc.write(&wire::request(&id, "provider/updateAccountConfig", params));
    }
    let id = proc.request_id("zcode_capabilities");
    let _ = proc.write(&wire::request(&id, "runtime/capabilities", json!({})));
    if let Ok(cwd) = workspace(proc) {
        let id = proc.request_id("zcode_catalog");
        let _ = proc.write(&wire::request(
            &id,
            "session/create",
            json!({"workspace": {"workspacePath": cwd, "workspaceKey": cwd}}),
        ));
    }
}

// ─────────────────────────── Incoming ───────────────────────────

pub(super) fn handle_line(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, line: &str) {
    let Some(incoming) = wire::parse_line(line) else { return };
    match incoming {
        Incoming::Response { id, result, error } => handle_response(app, session_id, proc, &id, result, error),
        Incoming::Request { id, method, params } => match method.as_str() {
            "session/requestRuntimePreferences" => {
                let _ = proc.write(&wire::reply(&id, json!({"nativeSearchEnhancementsEnabled": false})));
            }
            // Before each model call on a plan: the plan's API key, which the desktop app supplies too.
            "interaction/requestProviderRuntimeHeaders" => {
                let account = proc.zcode.lock().unwrap().account.clone();
                match account {
                    // An OAuth sign-in exchanges its token with the platform first; this thread reads the
                    // agent's output and must not wait on the network.
                    Some(account) if account.runtime_headers_need_network(&params) => {
                        let proc = proc.clone();
                        std::thread::spawn(move || {
                            let _ = proc.write(&wire::reply(&id, account.runtime_headers(&params)));
                        });
                    }
                    Some(account) => {
                        let _ = proc.write(&wire::reply(&id, account.runtime_headers(&params)));
                    }
                    None => {
                        let refusal = json!({"headersApplied": false, "errorMessage": "Sign in to ZCode to use this plan."});
                        let _ = proc.write(&wire::reply(&id, refusal));
                    }
                }
            }
            // Answered through the V4 interaction instead; see the module documentation.
            "interaction/requestPermission" | "interaction/requestUserInput" => {}
            _ => {
                let _ = proc.write(&wire::reply_unsupported(&id));
            }
        },
        Incoming::Notification { method, params } => match method.as_str() {
            "v4/conversation/frame" => {
                let frame = {
                    let mut state = proc.zcode.lock().unwrap();
                    wire::decode_frame(&params, &mut state.fragments)
                };
                if let Some(frame) = frame {
                    handle_frame(app, session_id, proc, &frame);
                }
            }
            "startup/storageState" if params.get("phase").and_then(Value::as_str) == Some("failed") => {
                let code = params.get("errorCode").and_then(Value::as_str).unwrap_or("unknown");
                emit(app, session_id, json!({"type":"error","message":format!("ZCode could not open its session store ({code})")}));
            }
            _ => {}
        },
    }
}

fn handle_response(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    id: &str,
    result: Value,
    error: Option<String>,
) {
    let kind = proc.pending.lock().unwrap().remove(id);
    if let Some(waiter) = proc.waiters.lock().unwrap().remove(id) {
        let _ = waiter.send(match error {
            Some(message) => Err(message),
            None => Ok(result),
        });
        return;
    }
    match kind {
        Some("zcode_capabilities") => {
            let resume = proc.agent_session_id.lock().unwrap().clone();
            match resume {
                Some(native) => subscribe(proc, &native),
                // A new conversation is created by its first message; nothing else has to happen first.
                None => mark_ready(app, session_id, proc),
            }
        }
        // A refusal leaves every plan unentitled, which the first message then reports as signed out.
        Some("zcode_account") => {
            crate::diagnostics::record(if error.is_some() { "WARN" } else { "INFO" }, "zcode_account_config", json!({
                "sessionId": session_id, "status": result.get("status"), "providerCount": result.get("providerCount"), "error": error,
            }));
        }
        Some("zcode_subscribe") => {
            if let Some(message) = error {
                emit(app, session_id, json!({"type":"error","message":format!("ZCode could not open this conversation: {message}")}));
                return;
            }
            on_subscribed(proc, &result);
        }
        Some("zcode_catalog") => {
            let available = result
                .pointer("/settings/model/available")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let models = wire::to_chat_models(&available);
            if !models.is_empty() {
                proc.zcode.lock().unwrap().models = models;
                emit(app, session_id, json!({"type":"models"}));
            }
            if let Some(commands) = result.get("slashCommands").and_then(Value::as_array) {
                let normalized = wire::slash_commands(commands);
                *proc.commands.lock().unwrap() = normalized.clone();
                emit(app, session_id, json!({"type":"commands","commands":normalized}));
            }
            // The draft only existed to answer this; release it.
            if let Some(draft) = result.pointer("/session/sessionId").and_then(Value::as_str) {
                let id = proc.request_id("zcode_close_draft");
                let _ = proc.write(&wire::request(&id, "session/close", json!({"sessionId": draft})));
            }
        }
        Some("zcode_command") => {
            if let Some(message) = error.or_else(|| (!wire::ack_ok(&result)).then(|| wire::ack_error(&result))) {
                emit(app, session_id, json!({"type":"error","message":message}));
            }
        }
        _ => {}
    }
}

fn mark_ready(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    if !proc.ready.swap(true, Ordering::Relaxed) {
        emit_state(app, session_id, AgentState::Waiting);
        start_waiting_message(app, session_id, proc);
    }
}

fn subscribe(proc: &Arc<ChatProcess>, native: &str) {
    let Ok(cwd) = workspace(proc) else { return };
    let connection = proc.zcode.lock().unwrap().connection_id.clone();
    let id = proc.request_id("zcode_subscribe");
    let _ = proc.write(&wire::request(&id, "v4/conversation/subscribe", wire::subscribe_params(native, &connection, &cwd)));
}

fn on_subscribed(proc: &Arc<ChatProcess>, result: &Value) {
    let mut state = proc.zcode.lock().unwrap();
    state.subscribed = true;
    state.subscription_id = result
        .pointer("/ack/subscriptionId")
        .and_then(Value::as_str)
        .map(str::to_string);
    if proc.ready.load(Ordering::Relaxed) {
        return;
    }
    // The snapshot frame follows this answer; readiness waits for it. See `ready_on_snapshot`.
    state.ready_on_snapshot = true;
}

/// Record the native conversation id, the way every other driver does once the agent names it.
fn adopt_session(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, id: &str) -> bool {
    if !super::verify_native_identity(app, session_id, proc, id) {
        return false;
    }
    let previous = proc.agent_session_id.lock().unwrap().clone();
    if previous.as_deref() != Some(id) {
        *proc.agent_session_id.lock().unwrap() = Some(id.to_string());
        let changed = {
            let conn = app.db().conn.lock().unwrap();
            crate::db::repo::set_agent_session_id(&conn, session_id, id, proc.kind).unwrap_or(false)
        };
        if changed {
            app.emit(crate::host::TREE_CHANGED, ());
        }
    }
    let model = proc.model.lock().unwrap().clone();
    emit(app, session_id, json!({"type":"session","agentSessionId":id,"model":model}));
    true
}

// ─────────────────────────── Frames ───────────────────────────

fn handle_frame(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, frame: &Value) {
    let native = proc.agent_session_id.lock().unwrap().clone();
    let topic = frame.get("topic").and_then(Value::as_str).unwrap_or("");
    if native.as_deref().map(|id| format!("conversation/{id}")).as_deref() != Some(topic) {
        return;
    }
    let Some(payload) = frame.get("payload") else { return };
    let from = frame.get("fromSeq").and_then(Value::as_u64);
    let to = frame.get("toSeq").and_then(Value::as_u64);
    let snapshot = payload.get("kind").and_then(Value::as_str) == Some("snapshot");
    {
        let mut state = proc.zcode.lock().unwrap();
        if !snapshot && state.last_seq.is_some() && from.is_some() && state.last_seq != from {
            // A delta frame that does not continue the last one cannot be applied safely; ask for a snapshot.
            state.last_seq = None;
            let subscription = state.subscription_id.clone();
            let connection = state.connection_id.clone();
            drop(state);
            let id = proc.request_id("zcode_resync");
            let _ = proc.write(&wire::request(&id, "v4/conversation/resync", json!({
                "topic": topic, "connectionId": connection, "subscriptionId": subscription,
                "base": Value::Null, "forceSnapshot": true,
            })));
            return;
        }
        state.last_seq = to.or(state.last_seq);
    }
    if snapshot {
        if let Some(snapshot) = payload.get("snapshot") {
            apply_snapshot(app, session_id, proc, snapshot);
        }
        return;
    }
    for delta in payload.get("deltas").and_then(Value::as_array).into_iter().flatten() {
        apply_delta(app, session_id, proc, delta);
    }
}

fn apply_snapshot(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, snapshot: &Value) {
    apply_snapshot_rows(app, session_id, proc, snapshot);
    if std::mem::take(&mut proc.zcode.lock().unwrap().ready_on_snapshot) {
        mark_ready(app, session_id, proc);
    }
}

fn apply_snapshot_rows(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, snapshot: &Value) {
    for key in ["config", "control", "usage", "pendingInteractions"] {
        if let Some(value) = snapshot.get(key) {
            apply_state(app, session_id, proc, key, value);
        }
    }
    let window: Vec<Value> = snapshot
        .pointer("/rows/window")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for row in &window {
        upsert_raw(app, session_id, proc, row.clone());
    }
    let total = snapshot.pointer("/rows/totalCount").and_then(Value::as_u64).unwrap_or(0) as usize;
    let first = window.first().and_then(|row| row.get("rowId")).and_then(Value::as_i64);
    if total > window.len() {
        if let Some(first) = first {
            let start = {
                let mut state = proc.zcode.lock().unwrap();
                !std::mem::replace(&mut state.history_requested, true)
            };
            if start {
                spawn_history(app.clone(), session_id.to_string(), proc.clone(), first);
            }
        }
    }
}

fn apply_delta(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, delta: &Value) {
    match delta.get("op").and_then(Value::as_str).unwrap_or("") {
        "row.appended" | "row.upserted" => {
            if let Some(row) = delta.get("row") {
                upsert_raw(app, session_id, proc, row.clone());
            }
        }
        "row.delta" => {
            let Some(row_id) = delta.get("rowId").and_then(Value::as_i64) else { return };
            let path = delta.get("path").and_then(Value::as_str).unwrap_or("text");
            let append = delta.get("append").and_then(Value::as_str).unwrap_or("");
            let row = {
                let mut state = proc.zcode.lock().unwrap();
                let Some(row) = state.rows.get_mut(&row_id) else { return };
                wire::apply_delta(row, path, append);
                row.clone()
            };
            render(proc, &row);
        }
        "row.removed" => {
            if let Some(from) = delta.get("fromRowId").and_then(Value::as_i64) {
                remove_from(proc, from);
            }
        }
        "state.updated" => {
            if let Some(patch) = delta.get("patch").and_then(Value::as_object) {
                for (key, value) in patch {
                    apply_state(app, session_id, proc, key, value);
                }
            }
        }
        _ => {}
    }
}

fn apply_state(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, key: &str, value: &Value) {
    match key {
        "config" => apply_config(app, session_id, proc, value),
        "control" => {
            let error = value
                .pointer("/lastError/message")
                .and_then(Value::as_str)
                .filter(|m| !m.is_empty())
                .map(str::to_string);
            proc.zcode.lock().unwrap().last_error = error;
        }
        "pendingInteractions" => sync_interactions(app, session_id, proc, value),
        "usage" => apply_usage(app, session_id, proc, value),
        _ => {}
    }
}

/// The context window the server measures after each request, for the composer's meter and the Info panel.
/// A null window means the session has not reached the model yet and leaves the last reading in place.
fn apply_usage(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, usage: &Value) {
    let Some(window) = usage.get("contextWindow").filter(|w| w.is_object()) else { return };
    let used = window.get("usedTokens").and_then(Value::as_f64).filter(|n| *n >= 0.0);
    let max = window.get("maxTokens").and_then(Value::as_f64).filter(|n| *n > 0.0);
    let (Some(used), Some(max)) = (used, max) else { return };
    {
        let mut extras = proc.extras.lock().unwrap();
        extras.context_tokens = Some(used as u64);
        extras.context_window = Some(max as u64);
    }
    super::emit_extras(app, session_id, proc);
}

/// Adopt the model, reasoning level and mode the session reports, when the view has not chosen its own.
fn apply_config(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, config: &Value) {
    let reported = wire::model_id(
        config.get("provider").and_then(Value::as_str),
        config.get("model").and_then(Value::as_str),
    );
    let mut changed = false;
    if let Some(model) = reported {
        let mut current = proc.model.lock().unwrap();
        if current.is_none() {
            *current = Some(model);
            changed = true;
        }
    }
    if let Some(thought) = config.get("thought").and_then(Value::as_str).filter(|t| !t.is_empty()) {
        let mut current = proc.effort.lock().unwrap();
        if current.is_none() {
            *current = Some(thought.to_string());
            changed = true;
        }
    }
    if changed {
        let model = proc.model.lock().unwrap().clone();
        let effort = proc.effort.lock().unwrap().clone();
        emit(app, session_id, json!({"type":"settingsChanged","model":model,"effort":effort}));
    }
}

/// Mirror one projected row: turn headers drive the turn bookkeeping, every other row is drawn.
fn upsert_raw(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, row: Value) {
    let Some(raw_id) = row.get("rowId").and_then(Value::as_i64) else { return };
    proc.zcode.lock().unwrap().rows.insert(raw_id, row.clone());
    if row.get("kind").and_then(Value::as_str) == Some("turnHeader") {
        turn_header(app, session_id, proc, raw_id, &row);
        return;
    }
    render(proc, &row);
}

fn render(proc: &Arc<ChatProcess>, row: &Value) {
    let Some(raw_id) = row.get("rowId").and_then(Value::as_i64) else { return };
    if row.get("kind").and_then(Value::as_str) == Some("userInput") {
        let source = row.get("sourceCommandId").and_then(Value::as_str).unwrap_or("");
        // The bubble the engine drew when it sent this message is the one to keep: it holds the images.
        if proc.zcode.lock().unwrap().own_inputs.contains_key(source) {
            return;
        }
    }
    if let Some(view) = wire::convert_row(row, wire::row_id(raw_id)) {
        proc.timeline.lock().unwrap().upsert(view);
    }
}

fn remove_from(proc: &Arc<ChatProcess>, from: i64) {
    let mut state = proc.zcode.lock().unwrap();
    state.rows.retain(|id, _| *id < from);
    drop(state);
    let mut timeline = proc.timeline.lock().unwrap();
    let kept: Vec<ChatRow> = timeline
        .rows
        .iter()
        .filter(|row| {
            row.id()
                .strip_prefix("z-")
                .and_then(|n| n.parse::<i64>().ok())
                .is_none_or(|n| n < from)
        })
        .cloned()
        .collect();
    if kept.len() != timeline.rows.len() {
        timeline.replace_all(kept);
    }
}

fn turn_header(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, raw_id: i64, row: &Value) {
    let state_name = row.get("state").and_then(Value::as_str).unwrap_or("");
    let source = row
        .get("rootSourceCommandId")
        .or_else(|| row.get("sourceCommandId"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let running = state_name == "running";
    let (ours, foreign) = {
        let mut state = proc.zcode.lock().unwrap();
        let ours = state.awaited_turn == Some(raw_id)
            || (!source.is_empty() && state.awaiting.as_deref() == Some(source.as_str()));
        if ours && running {
            state.awaited_turn = Some(raw_id);
        }
        let foreign = !ours && (state.foreign_turn == Some(raw_id) || running);
        if foreign && running {
            state.foreign_turn = Some(raw_id);
        }
        (ours, foreign)
    };
    if running {
        if foreign && !proc.turn.lock().unwrap().running {
            emit_state(app, session_id, AgentState::Working);
        }
        return;
    }
    if ours {
        let error = {
            let mut state = proc.zcode.lock().unwrap();
            state.awaiting = None;
            state.awaited_turn = None;
            state.last_error.clone()
        };
        if state_name == "failed" {
            let message = error.unwrap_or_else(|| "The turn failed".to_string());
            proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                id: format!("e-{}", wire::row_id(raw_id)),
                message: plan_error(proc, message),
            });
        }
        if state_name != "failed" {
            // A turn the model answered confirms the account, however it was signed in.
            super::auth::turn_succeeded(app, session_id, proc);
        }
        if proc.turn.lock().unwrap().running {
            handle_turn_end(app, session_id, proc, "success", None);
        }
    } else if foreign {
        proc.zcode.lock().unwrap().foreign_turn = None;
        if !proc.turn.lock().unwrap().running {
            emit_state(app, session_id, AgentState::Waiting);
        }
    }
}

/// Page in the rows older than the subscription's tail, newest page first, and put them in front.
fn spawn_history(app: AppCtx, session_id: String, proc: Arc<ChatProcess>, first: i64) {
    std::thread::spawn(move || {
        let Some(native) = proc.agent_session_id.lock().unwrap().clone() else { return };
        let mut before = first;
        let mut pages: Vec<Vec<Value>> = Vec::new();
        let mut count = 0usize;
        while count < HISTORY_ROW_LIMIT {
            let answer = proc.request_and_wait("zcode_rows", |id| {
                wire::request(id, "v4/conversation/rowsRange", json!({
                    "sessionId": native, "beforeRowId": before, "limit": 200,
                }))
            });
            let Ok(answer) = answer else { break };
            let rows: Vec<Value> = answer.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
            let Some(oldest) = rows.first().and_then(|row| row.get("rowId")).and_then(Value::as_i64) else { break };
            count += rows.len();
            pages.push(rows);
            if answer.get("hasMore").and_then(Value::as_bool) != Some(true) || oldest >= before {
                break;
            }
            before = oldest;
        }
        if pages.is_empty() || !proc.alive.load(Ordering::Relaxed) {
            return;
        }
        let rows: Vec<ChatRow> = pages
            .into_iter()
            .rev()
            .flatten()
            .filter_map(|row| {
                let raw_id = row.get("rowId").and_then(Value::as_i64)?;
                let mut view = wire::convert_row(&row, wire::row_id(raw_id))?;
                // A replayed row belongs to work that has finished; it cannot still be streaming.
                match &mut view {
                    ChatRow::Assistant { streaming, .. } | ChatRow::Reasoning { streaming, .. } => *streaming = false,
                    ChatRow::Tool { status, .. } if *status == "running" => *status = "canceled",
                    _ => {}
                }
                Some(view)
            })
            .collect();
        if proc.timeline.lock().unwrap().restore_prefix(rows) {
            emit(&app, &session_id, reset_event(&proc));
        }
    });
}

// ─────────────────────────── Interactions ───────────────────────────

fn bypass(proc: &ChatProcess) -> bool {
    proc.mode.lock().unwrap().as_str() == "bypassPermissions"
}

fn sync_interactions(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, list: &Value) {
    let pending: Vec<Value> = list.as_array().cloned().unwrap_or_default();
    let ids: HashSet<String> = pending
        .iter()
        .filter_map(|item| item.get("interactionId").and_then(Value::as_str).map(str::to_string))
        .collect();
    let gone: Vec<String> = {
        let state = proc.zcode.lock().unwrap();
        state.interactions.difference(&ids).cloned().collect()
    };
    for id in &gone {
        proc.zcode.lock().unwrap().interactions.remove(id);
        if proc.permissions.lock().unwrap().remove(id).is_some() {
            emit(app, session_id, json!({"type":"permissionResolved","id":id}));
        }
    }
    let mut asked = false;
    for item in &pending {
        let Some(card) = wire::interaction_payload(item) else { continue };
        let id = card["id"].as_str().unwrap_or_default().to_string();
        if !proc.zcode.lock().unwrap().interactions.insert(id.clone()) {
            continue;
        }
        if bypass(proc) && card["_zcodeKind"] == "permission" {
            let _ = resolve(proc, &id, json!({"optionId": "allowOnce"}));
            continue;
        }
        proc.permissions.lock().unwrap().insert(id, card.clone());
        emit(app, session_id, json!({"type":"permission","request":card}));
        asked = true;
    }
    if asked {
        emit_state(app, session_id, AgentState::Asking);
    } else if !gone.is_empty() && proc.permissions.lock().unwrap().is_empty() {
        let running = proc.turn.lock().unwrap().running;
        emit_state(app, session_id, if running { AgentState::Working } else { AgentState::Waiting });
    }
}

fn resolve(proc: &Arc<ChatProcess>, interaction: &str, answer: Value) -> Result<(), String> {
    let native = proc.agent_session_id.lock().unwrap().clone().ok_or("ZCode has not opened this conversation yet")?;
    let id = proc.request_id("zcode_command");
    proc.write(&wire::command(
        &id,
        &uuid::Uuid::new_v4().to_string(),
        Some(&native),
        "resolveInteraction",
        json!({"interactionId": interaction, "answer": answer}),
        super::now_ms(),
    ))
}

/// Answer a card. The card goes away now; the server confirms by dropping the interaction from its list.
#[allow(clippy::too_many_arguments)]
pub(super) fn respond(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    request_id: &str,
    request: &Value,
    allow: bool,
    updated_input: Option<&Value>,
    message: Option<&str>,
    updated_permissions: Option<&Value>,
) -> Result<(), String> {
    let answer = wire::interaction_answer(request, allow, updated_input, message, updated_permissions);
    resolve(proc, request_id, answer)?;
    proc.permissions.lock().unwrap().remove(request_id);
    emit(app, session_id, json!({"type":"permissionResolved","id":request_id}));
    if proc.permissions.lock().unwrap().is_empty() {
        let running = proc.turn.lock().unwrap().running;
        emit_state(app, session_id, if running { AgentState::Working } else { AgentState::Waiting });
    }
    Ok(())
}

/// Bypass was switched on while approvals waited: allow them. Questions still need a person.
pub(super) fn approve_pending(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let pending: Vec<String> = proc
        .permissions
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, card)| card["_zcodeKind"] == "permission")
        .map(|(id, _)| id.clone())
        .collect();
    for id in pending {
        if resolve(proc, &id, json!({"optionId": "allowOnce"})).is_ok() {
            proc.permissions.lock().unwrap().remove(&id);
            emit(app, session_id, json!({"type":"permissionResolved","id":id}));
        }
    }
}

// ─────────────────────────── Outgoing ───────────────────────────

/// The selection a message carries: the session's model with its reasoning level, or the model's default
/// level when none was chosen. ZCode refuses a model selection without a level the model accepts.
///
/// A model the catalogue no longer lists, such as one of a plan left behind by signing in to the other
/// platform, is not sent: ZCode's own default for the account takes its place.
fn selection(proc: &ChatProcess) -> Option<Value> {
    let model = proc.model.lock().unwrap().clone()?;
    {
        let state = proc.zcode.lock().unwrap();
        if !state.models.is_empty() && !state.models.iter().any(|m| m["id"].as_str() == Some(model.as_str())) {
            return None;
        }
    }
    let effort = proc.effort.lock().unwrap().clone().filter(|e| !e.is_empty()).or_else(|| {
        let state = proc.zcode.lock().unwrap();
        wire::default_effort(&state.models, &model)
    });
    wire::model_selection(&model, effort.as_deref())
}

fn send_command(proc: &Arc<ChatProcess>, session: Option<&str>, kind: &str, payload: Value, command_id: &str) -> Result<Value, String> {
    let answer = proc.request_and_wait("zcode_send", |id| {
        wire::command(id, command_id, session, kind, payload.clone(), super::now_ms())
    })?;
    if wire::ack_ok(&answer) {
        return Ok(answer);
    }
    // After a stop ZCode may hold its own queue and wants to be told what to do with it. The view keeps its
    // queue itself, so whatever ZCode still holds stays queued.
    if kind == "sendText" && answer.get("reasonCode").and_then(Value::as_str) == Some("heldQueueDispositionRequired") {
        let mut payload = payload;
        payload["heldQueueDisposition"] = json!("keepQueueAndSend");
        let retry = proc.request_and_wait("zcode_send", |id| {
            wire::command(id, &uuid::Uuid::new_v4().to_string(), session, kind, payload.clone(), super::now_ms())
        })?;
        if wire::ack_ok(&retry) {
            return Ok(retry);
        }
        return Err(wire::ack_error(&retry));
    }
    Err(wire::ack_error(&answer))
}

fn upload(proc: &Arc<ChatProcess>, native: &str, index: usize, image: &ChatImage) -> Result<Value, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image.data.as_bytes())
        .map_err(|_| "An attached image could not be read".to_string())?;
    let connection = proc.zcode.lock().unwrap().connection_id.clone();
    let upload = uuid::Uuid::new_v4().to_string();
    let extension = image.mime_type.rsplit('/').next().unwrap_or("png");
    let file_name = format!("image-{}.{extension}", index + 1);
    let chunks: Vec<&[u8]> = bytes.chunks(wire::ATTACHMENT_CHUNK).collect();
    let begun = proc.request_and_wait("zcode_upload", |id| {
        wire::request(id, "v4/attachment/begin", json!({
            "connectionId": connection, "uploadId": upload, "sessionId": native, "fileName": file_name,
            "mime": image.mime_type, "totalBytes": bytes.len(), "totalChunks": chunks.len().max(1),
            "checksum": wire::attachment_checksum(&bytes),
        }))
    })?;
    let reference = if begun.get("state").and_then(Value::as_str) == Some("committed") {
        begun.get("ref").cloned()
    } else {
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            proc.request_and_wait("zcode_upload", |id| {
                wire::request(id, "v4/attachment/chunk", json!({
                    "connectionId": connection, "uploadId": upload, "sessionId": native,
                    "chunkIndex": chunk_index,
                    "dataBase64": base64::engine::general_purpose::STANDARD.encode(chunk),
                }))
            })?;
        }
        let committed = proc.request_and_wait("zcode_upload", |id| {
            wire::request(id, "v4/attachment/commit", json!({
                "connectionId": connection, "uploadId": upload, "sessionId": native,
            }))
        })?;
        committed.get("ref").cloned()
    };
    let reference = reference.filter(|r| r.is_string()).ok_or("ZCode did not accept the attached image")?;
    Ok(json!({"ref": reference, "fileName": file_name, "mime": image.mime_type, "bytes": bytes.len()}))
}

/// Send one user turn. Runs on its own thread: opening a conversation and uploading images take several
/// round trips, and the caller may be the reader thread that delivers their answers.
pub(super) fn dispatch(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    row_id: &str,
    text: &str,
    images: &[ChatImage],
) -> Result<(), String> {
    let cwd = workspace(proc)?;
    let command_id = uuid::Uuid::new_v4().to_string();
    {
        let mut state = proc.zcode.lock().unwrap();
        state.own_inputs.insert(command_id.clone(), row_id.to_string());
        state.awaiting = Some(command_id.clone());
        state.awaited_turn = None;
    }
    let (app, session_id, proc) = (app.clone(), session_id.to_string(), proc.clone());
    let (row_id, text, images) = (row_id.to_string(), text.to_string(), images.to_vec());
    std::thread::spawn(move || {
        if let Err(message) = send_turn(&app, &session_id, &proc, &cwd, &command_id, &text, &images) {
            let no_plan = {
                let mut state = proc.zcode.lock().unwrap();
                if state.awaiting.as_deref() == Some(command_id.as_str()) {
                    state.awaiting = None;
                }
                state.models.is_empty()
            } && selection(&proc).is_none();
            // No plan, ZCode lists no models, and its refusal only says the message named none ("Session model
            // must be provider-qualified"). Signed out, the account panel takes its place and signs the user
            // in; signed in, no plan of that platform was found in the sign-in, which a sign-in prompt would
            // misstate and which is not the platform refusing the plan either.
            let platform = no_plan.then(|| proc.zcode.lock().unwrap().account.clone()).flatten()
                .and_then(|account| account.signed_in());
            if let Some(platform) = &platform {
                emit(&app, &session_id, json!({"type":"error","message":format!("{NO_MODELS}:{platform}")}));
            } else if no_plan {
                hand_back(&app, &session_id, &proc, &row_id, &text, &images);
                super::auth::zcode::require(&app, &session_id, &proc);
            } else {
                emit(&app, &session_id, json!({"type":"error","message":message}));
            }
            if proc.turn.lock().unwrap().running {
                let prompted = no_plan && platform.is_none();
                handle_turn_end(&app, &session_id, &proc, if prompted { "success" } else { "request_failed" }, None);
            }
        }
    });
    Ok(())
}

/// ZCode refused the message, so this timeline holds the only copy, and signing in replaces the process that
/// owns it. The message goes back to the composer rather than staying on screen as if it had been delivered.
fn hand_back(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, row_id: &str, text: &str, images: &[ChatImage]) {
    {
        let mut timeline = proc.timeline.lock().unwrap();
        let kept = timeline.rows.iter().filter(|row| row.id() != row_id).cloned().collect();
        timeline.replace_all(kept);
    }
    emit(app, session_id, reset_event(proc));
    super::emit_queue(app, session_id, proc);
    emit(app, session_id, json!({"type":"unsent","text":text,"images":images}));
}

fn send_turn(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    cwd: &str,
    command_id: &str,
    text: &str,
    images: &[ChatImage],
) -> Result<(), String> {
    let mode = wire::native_mode(proc.mode.lock().unwrap().as_str());
    let selection = selection(proc);
    let native = proc.agent_session_id.lock().unwrap().clone();
    let native = match native {
        Some(native) => native,
        None => {
            // Open the conversation. Text alone goes in with it; images need the session first, because an
            // upload belongs to one.
            let mut config = json!({"mode": mode});
            if let Some(selection) = &selection {
                config["modelSelection"] = selection.clone();
            }
            let mut payload = json!({"workspaceId": cwd, "config": config});
            if images.is_empty() {
                payload["firstInput"] = json!({"text": text, "mode": mode});
            }
            let create_id = if images.is_empty() { command_id.to_string() } else { uuid::Uuid::new_v4().to_string() };
            let answer = send_command(proc, None, "createSession", payload, &create_id)?;
            let native = answer
                .pointer("/result/sessionId")
                .and_then(Value::as_str)
                .ok_or("ZCode did not open a conversation")?
                .to_string();
            if !adopt_session(app, session_id, proc, &native) {
                return Err("The conversation identity changed during startup".to_string());
            }
            let connection = proc.zcode.lock().unwrap().connection_id.clone();
            let subscribed = proc.request_and_wait("zcode_subscribe", |id| {
                wire::request(id, "v4/conversation/subscribe", wire::subscribe_params(&native, &connection, cwd))
            })?;
            on_subscribed(proc, &subscribed);
            if images.is_empty() {
                return Ok(());
            }
            native
        }
    };
    let attachments = images
        .iter()
        .enumerate()
        .map(|(index, image)| upload(proc, &native, index, image))
        .collect::<Result<Vec<_>, _>>()?;
    let mut payload = json!({"text": text, "mode": mode, "requestedDelivery": "startNow"});
    if let Some(selection) = selection {
        payload["modelSelection"] = selection;
    }
    if !attachments.is_empty() {
        payload["attachments"] = json!(attachments);
    }
    send_command(proc, Some(&native), "sendText", payload, command_id)?;
    Ok(())
}

/// Steer one user message into the turn that is already running. `guide` delivery hands it to that turn
/// at its next tool boundary, and ZCode itself queues it when the turn has none left to offer, so a
/// refusal here belongs to this message alone: the turn keeps running either way. Runs on its own thread
/// like `dispatch`, for the same round trips.
pub(super) fn steer(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    row_id: &str,
    text: &str,
    images: &[ChatImage],
) -> Result<(), String> {
    let native = proc.agent_session_id.lock().unwrap().clone()
        .ok_or("ZCode has not opened this conversation yet")?;
    let command_id = uuid::Uuid::new_v4().to_string();
    // The steered message comes back as a projected row; claiming the command keeps the bubble the
    // engine drew, which holds the images, instead of drawing both.
    proc.zcode.lock().unwrap().own_inputs.insert(command_id.clone(), row_id.to_string());
    let (app, session_id, proc) = (app.clone(), session_id.to_string(), proc.clone());
    let (text, images) = (text.to_string(), images.to_vec());
    std::thread::spawn(move || {
        let guide = (|| {
            let attachments = images
                .iter()
                .enumerate()
                .map(|(index, image)| upload(&proc, &native, index, image))
                .collect::<Result<Vec<_>, _>>()?;
            let mut payload = json!({"text": text, "requestedDelivery": "guide"});
            if !attachments.is_empty() {
                payload["attachments"] = json!(attachments);
            }
            send_command(&proc, Some(&native), "sendText", payload, &command_id)
        })();
        if let Err(message) = guide {
            emit(&app, &session_id, json!({"type":"error","message":message}));
        }
    });
    Ok(())
}

pub(super) fn stop(proc: &Arc<ChatProcess>) -> Result<(), String> {
    let native = proc.agent_session_id.lock().unwrap().clone().ok_or("ZCode has not opened this conversation yet")?;
    let id = proc.request_id("zcode_command");
    proc.write(&wire::command(&id, &uuid::Uuid::new_v4().to_string(), Some(&native), "stop", json!({}), super::now_ms()))
}

pub(super) fn compact(proc: &Arc<ChatProcess>) -> Result<(), String> {
    let native = proc.agent_session_id.lock().unwrap().clone().ok_or("There is no conversation to compact yet")?;
    let id = proc.request_id("zcode_command");
    proc.write(&wire::command(&id, &uuid::Uuid::new_v4().to_string(), Some(&native), "compact", json!({}), super::now_ms()))
}

/// The catalogue as the composer wants it, or an empty list before the agent has answered.
pub(super) fn chat_models(proc: &Arc<ChatProcess>) -> Vec<Value> {
    proc.zcode.lock().unwrap().models.clone()
}

/// Put a process in the state a dispatched message leaves it in, for tests driving frames by hand.
#[cfg(test)]
pub(super) fn test_await(proc: &ChatProcess, command_id: &str, row_id: &str) {
    let mut state = proc.zcode.lock().unwrap();
    state.own_inputs.insert(command_id.to_string(), row_id.to_string());
    state.awaiting = Some(command_id.to_string());
}

/// Whether any command was claimed for a row, for tests checking sends without waiting for frames.
#[cfg(test)]
pub(super) fn test_claimed(proc: &ChatProcess, row_id: &str) -> bool {
    proc.zcode.lock().unwrap().own_inputs.values().any(|row| row == row_id)
}
