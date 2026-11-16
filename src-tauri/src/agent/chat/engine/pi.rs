//! Pi and OMP inside the chat engine: one process, its JSONL event stream, and the timeline it produces.
//!
//! Both CLIs are driven the same way over their headless protocol: a prompt goes in, and text, reasoning,
//! tool calls and their results stream back as events. This module owns that loop. The two differ only in
//! the variant-specific command names and model shapes handled by `chat::pi_protocol`.
//!
//! Permission handling follows each agent's own design. Pi has no approval step, so an extension dialog is
//! answered automatically and no permission control is offered. OMP can ask, and its question arrives as an
//! extension dialog; that becomes a permission card, and the answer travels back as the dialog response.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{
    append, emit, emit_state, handle_compaction, handle_turn_end, start_waiting_message, ChatProcess,
    ChatRow,
};
use crate::agent::chat::pi_protocol::{self as wire, PiIncoming, PiVariant};
use crate::agent::chat::protocol::ChatImage;
use crate::host::AppCtx;
use crate::pty::AgentState;

/// Everything the engine keeps about the Pi/OMP side of one session.
#[derive(Default)]
pub(super) struct PiState {
    pub(super) variant: Option<PiVariant>,
    /// The raw `get_available_models` catalogue, for resolving a model id to its provider.
    pub(super) models: Vec<Value>,
    /// Per-assistant-message sequence, which is what gives streaming rows their ids.
    assistant_seq: u64,
    /// Base row id of the assistant message currently streaming, if any.
    current: Option<String>,
    /// Extension dialogs waiting for an answer, keyed by request id.
    pub(super) requests: HashMap<String, Value>,
    /// Tool-call argument buffers, keyed by call id.
    args: HashMap<String, String>,
    /// Tool-call names, keyed by call id.
    names: HashMap<String, String>,
    /// The most recent tool call opened in the current assistant message; deltas do not repeat its id.
    last_tool: Option<String>,
    /// Whether a compaction lifecycle event arrived since the last `/compact`, so the command's own
    /// response does not draw a second marker.
    compaction_seen: bool,
}

fn variant(proc: &ChatProcess) -> PiVariant {
    proc.pi
        .lock()
        .unwrap()
        .variant
        .or_else(|| PiVariant::of(proc.kind))
        .unwrap_or(PiVariant::Pi)
}

/// Send the opening commands. `get_state` reveals the native session and is what finishes the handshake.
pub(super) fn bootstrap(proc: &Arc<ChatProcess>) {
    let variant = variant(proc);
    if variant == PiVariant::Omp {
        let id = proc.request_id("negotiate_protocol");
        let _ = proc.write(&wire::negotiate(&id));
    }
    let state_id = proc.request_id("get_state");
    let _ = proc.write(&wire::get_state(&state_id));
    let models_id = proc.request_id("get_available_models");
    let _ = proc.write(&wire::get_available_models(&models_id));
    let commands_id = proc.request_id("get_commands");
    let _ = proc.write(&wire::get_commands(&commands_id, variant));
}

// ─────────────────────────── Incoming ───────────────────────────

pub(super) fn handle_line(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, line: &str) {
    let Some(incoming) = wire::parse_line(line) else {
        return;
    };
    match incoming {
        PiIncoming::Ready => {}
        PiIncoming::Commands(commands) => set_commands(app, session_id, proc, commands),
        PiIncoming::ExtensionUi(request) => handle_extension_ui(app, session_id, proc, request),
        PiIncoming::Response { command, success, data, error, .. } => {
            handle_response(app, session_id, proc, &command, success, data, error);
        }
        PiIncoming::Event(value) => handle_event(app, session_id, proc, value),
    }
}

fn handle_response(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    command: &str,
    success: bool,
    data: Value,
    error: Option<String>,
) {
    match command {
        "get_state" if success => on_state(app, session_id, proc, &data),
        "get_available_models" if success => {
            let models = data
                .get("models")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            proc.pi.lock().unwrap().models = models;
        }
        "get_commands" | "get_available_commands" if success => {
            let commands = data
                .get("commands")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            set_commands(app, session_id, proc, commands);
        }
        // Not every compaction emits lifecycle events; when none did, the command's own answer is the
        // only signal that the conversation was summarized, so draw the marker from it.
        "compact" if success => {
            let seen = proc.pi.lock().unwrap().compaction_seen;
            if !seen {
                let pre_tokens = data.get("tokensBefore").and_then(Value::as_u64);
                handle_compaction(proc, true, Some("manual".to_string()), pre_tokens);
            }
        }
        "negotiate_protocol" | "set_model" | "set_thinking_level" | "abort" | "compact" => {}
        _ => {}
    }
    // A rejected prompt or steer never starts a turn, so the queue would wait forever without this.
    if !success {
        if let Some(error) = error.filter(|_| matches!(command, "prompt" | "steer")) {
            emit(app, session_id, json!({"type":"error","message":error}));
            let running = proc.turn.lock().unwrap().running;
            if running {
                handle_turn_end(app, session_id, proc, "request_failed", None);
            }
        }
    }
}

/// The handshake answer: it names the native session, the model it started with, and the thinking level.
fn on_state(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, data: &Value) {
    let session = data
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty());
    if let Some(id) = session {
        if !super::verify_native_identity(app,session_id,proc,id) { return; }
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
    }
    let model = data.get("model").and_then(|model| {
        model
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
    });
    if let Some(model) = &model {
        *proc.model.lock().unwrap() = Some(model.clone());
    }
    let reported = data.get("thinkingLevel").and_then(Value::as_str);
    // The launch carries the model but not the reasoning level, so a level saved with the session is
    // applied here, once the agent is listening.
    let desired = proc.effort.lock().unwrap().clone();
    if let Some(desired) = desired.as_deref().filter(|desired| Some(*desired) != reported) {
        let _ = set_effort(proc, desired);
    }
    if let Some(level) = reported {
        *proc.effort.lock().unwrap() = Some(level.to_string());
    }
    emit(
        app,
        session_id,
        json!({"type":"session","agentSessionId":session,"model":model}),
    );
    if !proc.ready.swap(true, Ordering::Relaxed) {
        emit_state(app, session_id, AgentState::Waiting);
        start_waiting_message(app, session_id, proc);
    }
}

fn set_commands(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, commands: Vec<Value>) {
    let normalized = wire::to_chat_commands(&commands);
    *proc.commands.lock().unwrap() = normalized.clone();
    emit(app, session_id, json!({"type":"commands","commands":normalized}));
}

// ─────────────────────────── Events ───────────────────────────

fn handle_event(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, value: Value) {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "agent_start" | "turn_start" => emit_state(app, session_id, AgentState::Working),
        "message_start" => start_message(proc, &value),
        "message_update" => update_message(proc, &value),
        "message_end" => end_message(proc, &value),
        "tool_execution_start" => tool_start(proc, &value),
        "tool_execution_update" => tool_update(proc, &value, false),
        "tool_execution_end" => tool_update(proc, &value, true),
        "compaction_start" | "auto_compaction_start" => {
            proc.pi.lock().unwrap().compaction_seen = true;
            handle_compaction(proc, false, reason_of(&value), None);
        }
        "compaction_end" | "auto_compaction_end" => {
            proc.pi.lock().unwrap().compaction_seen = true;
            let pre_tokens = value
                .pointer("/result/tokensBefore")
                .and_then(Value::as_u64);
            handle_compaction(proc, true, reason_of(&value), pre_tokens);
        }
        "auto_retry_start" => {
            let attempt = value.get("attempt").and_then(Value::as_u64).unwrap_or(0);
            let max = value.get("maxAttempts").and_then(Value::as_u64).unwrap_or(0);
            notice(proc, "n-retry", format!("Retrying ({attempt}/{max})…"));
        }
        "auto_retry_end" => {
            if value.get("success").and_then(Value::as_bool) == Some(false) {
                let message = value
                    .get("finalError")
                    .and_then(Value::as_str)
                    .unwrap_or("The automatic retry failed");
                notice(proc, "n-retry", message.to_string());
            }
        }
        "notice" => {
            let message = value.get("message").and_then(Value::as_str).unwrap_or("");
            if !message.is_empty() {
                notice(proc, "n-agent", message.to_string());
            }
        }
        "extension_error" => {
            let message = value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("An extension failed");
            error_row(proc, "e-extension", message.to_string());
        }
        "thinking_level_changed" => {
            if let Some(level) = value.get("thinkingLevel").and_then(Value::as_str) {
                *proc.effort.lock().unwrap() = Some(level.to_string());
            }
        }
        "agent_settled" => finish_turn(app, session_id, proc),
        "agent_end" => {
            let retry = value.get("willRetry").and_then(Value::as_bool).unwrap_or(false);
            let terminal = value
                .get("isTerminal")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            if !retry && terminal {
                surface_error(proc, &value);
                finish_turn(app, session_id, proc);
            }
        }
        _ => {}
    }
}

fn reason_of(value: &Value) -> Option<String> {
    value.get("reason").and_then(Value::as_str).map(str::to_string)
}

fn finish_turn(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    if !proc.turn.lock().unwrap().running {
        return;
    }
    handle_turn_end(app, session_id, proc, "success", None);
}

/// A failed run says why in its last assistant message; otherwise the turn would end with no reason shown.
fn surface_error(proc: &Arc<ChatProcess>, value: &Value) {
    let Some(messages) = value.get("messages").and_then(Value::as_array) else {
        return;
    };
    let Some(message) = messages.iter().rev().find(|message| {
        message.get("role").and_then(Value::as_str) == Some("assistant")
    }) else {
        return;
    };
    let stop = message.get("stopReason").and_then(Value::as_str).unwrap_or("");
    if stop != "error" {
        return;
    }
    let text = message
        .get("errorMessage")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .unwrap_or("The turn failed");
    error_row(proc, "e-turn", text.to_string());
}

fn notice(proc: &Arc<ChatProcess>, id: &str, message: String) {
    proc.timeline.lock().unwrap().upsert(ChatRow::Notice {
        id: id.to_string(),
        message,
    });
}

fn error_row(proc: &Arc<ChatProcess>, id: &str, message: String) {
    proc.timeline.lock().unwrap().upsert(ChatRow::Error {
        id: id.to_string(),
        message,
    });
}

// ─────────────────────────── Streaming ───────────────────────────

fn start_message(proc: &Arc<ChatProcess>, value: &Value) {
    let role = value
        .pointer("/message/role")
        .and_then(Value::as_str)
        .unwrap_or("");
    if role != "assistant" {
        return;
    }
    // Every assistant message gets a fresh base even if a previous `message_end` was missed, so a later
    // block can never overwrite the earlier message's rows.
    let base = new_base(proc);
    proc.extras
        .lock()
        .unwrap()
        .generation
        .pi_start(&base, super::now_ms());
}

fn new_base(proc: &Arc<ChatProcess>) -> String {
    let mut state = proc.pi.lock().unwrap();
    let base = format!("pi-a{}", state.assistant_seq);
    state.assistant_seq += 1;
    state.current = Some(base.clone());
    base
}

fn current_base(proc: &Arc<ChatProcess>) -> String {
    if let Some(base) = proc.pi.lock().unwrap().current.clone() {
        return base;
    }
    new_base(proc)
}

fn model_of(proc: &ChatProcess) -> Option<String> {
    proc.model.lock().unwrap().clone()
}

fn upsert_assistant(proc: &Arc<ChatProcess>, id: &str, text: String, streaming: bool) {
    let model = model_of(proc);
    proc.timeline.lock().unwrap().upsert(ChatRow::Assistant {
        id: id.to_string(),
        text,
        streaming,
        model,
        at: None,
        duration_ms: None,
    });
}

fn upsert_reasoning(proc: &Arc<ChatProcess>, id: &str, text: String, streaming: bool) {
    proc.timeline.lock().unwrap().upsert(ChatRow::Reasoning {
        id: id.to_string(),
        text,
        streaming,
    });
}

fn update_message(proc: &Arc<ChatProcess>, value: &Value) {
    let Some(event) = value.get("assistantMessageEvent") else {
        return;
    };
    let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
    let index = event.get("contentIndex").and_then(Value::as_u64).unwrap_or(0);
    let row_id = format!("{}:{index}", current_base(proc));
    match kind {
        "text_start" => upsert_assistant(proc, &row_id, String::new(), true),
        "text_delta" => {
            let delta = event.get("delta").and_then(Value::as_str).unwrap_or("");
            let text = append(proc, &row_id, delta);
            upsert_assistant(proc, &row_id, text, true);
        }
        "text_end" => {
            let text = event
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| append(proc, &row_id, ""));
            upsert_assistant(proc, &row_id, text, false);
        }
        "thinking_start" => upsert_reasoning(proc, &row_id, String::new(), true),
        "thinking_delta" => {
            let delta = event.get("delta").and_then(Value::as_str).unwrap_or("");
            let text = append(proc, &row_id, delta);
            upsert_reasoning(proc, &row_id, text, true);
        }
        "thinking_end" => {
            let text = event
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| append(proc, &row_id, ""));
            upsert_reasoning(proc, &row_id, text, false);
        }
        "toolcall_start" => {
            let id = event.get("id").and_then(Value::as_str).unwrap_or("");
            let name = event
                .get("toolName")
                .and_then(Value::as_str)
                .unwrap_or("tool");
            if id.is_empty() {
                return;
            }
            {
                let mut state = proc.pi.lock().unwrap();
                state.names.insert(id.to_string(), name.to_string());
                state.last_tool = Some(id.to_string());
            }
            upsert_tool(proc, id, Some(name), Some(json!({})), None, false, "running");
        }
        "toolcall_delta" => {
            let id = current_tool(proc, event);
            if let Some(id) = id {
                let delta = event.get("delta").and_then(Value::as_str).unwrap_or("");
                let args = tool_args(proc, &id, delta);
                let parsed = serde_json::from_str(&args).unwrap_or_else(|_| json!(args));
                upsert_tool(proc, &id, None, Some(parsed), None, false, "running");
            }
        }
        "toolcall_end" => {
            let call = event.get("toolCall").cloned().unwrap_or(Value::Null);
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| event.get("id").and_then(Value::as_str))
                .unwrap_or("");
            if id.is_empty() {
                return;
            }
            {
                let mut state = proc.pi.lock().unwrap();
                if let Some(name) = call.get("name").and_then(Value::as_str) {
                    state.names.insert(id.to_string(), name.to_string());
                }
                state.last_tool = Some(id.to_string());
            }
            let input = call.get("arguments").cloned().unwrap_or(json!({}));
            upsert_tool(proc, id, None, Some(input), None, false, "running");
        }
        _ => {}
    }
}

/// The tool call a `toolcall_delta` belongs to, taken from the current message when the event omits it.
fn current_tool(proc: &Arc<ChatProcess>, event: &Value) -> Option<String> {
    if let Some(id) = event.get("id").and_then(Value::as_str) {
        return Some(id.to_string());
    }
    // A delta does not repeat the id; the last call opened in the current message is the one.
    proc.pi.lock().unwrap().last_tool.clone()
}

fn tool_args(proc: &Arc<ChatProcess>, id: &str, delta: &str) -> String {
    let mut state = proc.pi.lock().unwrap();
    let _=proc;let buffer=state.args.entry(id.into()).or_default();buffer.push_str(delta);buffer.clone()
}

fn tool_name(proc: &ChatProcess, id: &str, explicit: Option<&str>) -> String {
    if let Some(name) = explicit.filter(|name| !name.is_empty()) {
        return name.to_string();
    }
    proc.pi
        .lock()
        .unwrap()
        .names
        .get(id)
        .cloned()
        .unwrap_or_else(|| "tool".to_string())
}

fn upsert_tool(
    proc: &Arc<ChatProcess>,
    id: &str,
    name: Option<&str>,
    input: Option<Value>,
    output: Option<String>,
    is_error: bool,
    status: &'static str,
) {
    let name = tool_name(proc, id, name);
    let existing = proc.timeline.lock().unwrap().get(id).cloned();
    let (existing_input, existing_output) = match existing {
        Some(ChatRow::Tool { input, output, .. }) => (Some(input), output),
        _ => (None, None),
    };
    let input = input.or(existing_input).unwrap_or(json!({}));
    let output = output.or(existing_output);
    proc.timeline.lock().unwrap().upsert(ChatRow::Tool {
        id: id.to_string(),
        name,
        input,
        output,
        is_error,
        status,
        subagent: None,
        children: Vec::new(),
    });
}

fn tool_start(proc: &Arc<ChatProcess>, value: &Value) {
    let id = value.get("toolCallId").and_then(Value::as_str).unwrap_or("");
    let name = value.get("toolName").and_then(Value::as_str);
    if id.is_empty() {
        return;
    }
    let input = value.get("args").cloned().unwrap_or(json!({}));
    upsert_tool(proc, id, name, Some(input), None, false, "running");
}

fn tool_update(proc: &Arc<ChatProcess>, value: &Value, done: bool) {
    let id = value.get("toolCallId").and_then(Value::as_str).unwrap_or("");
    if id.is_empty() {
        return;
    }
    let name = value.get("toolName").and_then(Value::as_str);
    let is_error = value.get("isError").and_then(Value::as_bool).unwrap_or(false);
    let output = if done {
        value.get("result").and_then(result_text)
    } else {
        value.get("partialResult").and_then(result_text)
    };
    upsert_tool(
        proc,
        id,
        name,
        None,
        output,
        is_error,
        if done {
            if is_error { "failed" } else { "completed" }
        } else {
            "running"
        },
    );
}

/// Text out of a tool result, which is either a plain string or an array of content blocks.
fn result_text(result: &Value) -> Option<String> {
    if let Some(text) = result.as_str() {
        return Some(text.to_string());
    }
    let blocks = result.get("content").and_then(Value::as_array)?;
    let text: Vec<String> = blocks
        .iter()
        .filter_map(|block| {
            block
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect();
    if text.is_empty() {
        None
    } else {
        Some(text.join("\n"))
    }
}

fn end_message(proc: &Arc<ChatProcess>, value: &Value) {
    let message = value.get("message").cloned().unwrap_or(Value::Null);
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let base = {
        let mut state = proc.pi.lock().unwrap();
        state.current.take().unwrap_or_else(|| {
            let base = format!("pi-a{}", state.assistant_seq);
            state.assistant_seq += 1;
            base
        })
    };
    // The completed message carries the provider's own output count, which pairs the stream with a rate.
    proc.extras.lock().unwrap().generation.pi_end(
        &base,
        message.pointer("/usage/output").and_then(Value::as_u64),
        super::now_ms(),
    );
    if let Some(model) = message.get("model").and_then(Value::as_str) {
        if !model.is_empty() {
            *proc.model.lock().unwrap() = Some(model.to_string());
        }
    }
    let Some(blocks) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    for (index, block) in blocks.iter().enumerate() {
        let row_id = format!("{base}:{index}");
        match block.get("type").and_then(Value::as_str).unwrap_or("") {
            "text" => {
                let text = block.get("text").and_then(Value::as_str).unwrap_or("").to_string();
                upsert_assistant(proc, &row_id, text, false);
            }
            "thinking" => {
                let text = block
                    .get("thinking")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                upsert_reasoning(proc, &row_id, text, false);
            }
            "toolCall" => {
                let id = block.get("id").and_then(Value::as_str).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                if let Some(name) = block.get("name").and_then(Value::as_str) {
                    proc.pi.lock().unwrap().names.insert(id.to_string(), name.to_string());
                }
                let input = block.get("arguments").cloned().unwrap_or(json!({}));
                upsert_tool(proc, id, None, Some(input), None, false, "running");
            }
            _ => {}
        }
    }
}

// ─────────────────────────── Outgoing ───────────────────────────

pub(super) fn dispatch(
    proc: &Arc<ChatProcess>,
    text: &str,
    images: &[ChatImage],
) -> Result<(), String> {
    let id = proc.request_id("prompt");
    proc.write_source(|writer|super::outgoing_wire::pi(writer,&id,"prompt",text,images))
}

pub(super) fn steer(proc: &Arc<ChatProcess>, text: &str, images: &[ChatImage]) -> Result<(), String> {
    let id = proc.request_id("steer");
    proc.write_source(|writer|super::outgoing_wire::pi(writer,&id,"steer",text,images))
}

pub(super) fn abort(proc: &Arc<ChatProcess>) -> Result<(), String> {
    let id = proc.request_id("abort");
    proc.write(&wire::abort(&id))
}

pub(super) fn compact(proc: &Arc<ChatProcess>) -> Result<(), String> {
    proc.pi.lock().unwrap().compaction_seen = false;
    let id = proc.request_id("compact");
    proc.write(&wire::compact(&id))
}

/// Apply the stored model to the running process. The catalogue resolves the id to its provider.
pub(super) fn set_model(proc: &Arc<ChatProcess>, model: &str) -> Result<(), String> {
    let models = proc.pi.lock().unwrap().models.clone();
    let (provider, model_id) = wire::resolve_model(&models, model);
    let id = proc.request_id("set_model");
    proc.write(&wire::set_model(&id, &provider, &model_id))
}

pub(super) fn set_effort(proc: &Arc<ChatProcess>, level: &str) -> Result<(), String> {
    let id = proc.request_id("set_thinking_level");
    proc.write(&wire::set_thinking_level(&id, level))
}

// ─────────────────────────── Extension UI ───────────────────────────

fn handle_extension_ui(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, request: Value) {
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    if method == "notify" {
        if let Some(message) = request.get("message").and_then(Value::as_str) {
            notice(proc, "n-notify", message.to_string());
        }
        return;
    }
    if !wire::extension_is_dialog(method) {
        return;
    }
    let bypass = proc.mode.lock().unwrap().as_str() == "bypassPermissions";
    if bypass || variant(proc) == PiVariant::Pi {
        // Pi has no approval step, and a bypassed OMP session is not asking either: answer so the extension
        // does not block, choosing the affirmative option where one exists.
        let allow = variant(proc) != PiVariant::Pi || method == "select" || method == "confirm";
        let response = if allow {
            wire::extension_response(&request, true)
        } else {
            wire::extension_response(&request, false)
        };
        let _ = proc.write(&response);
        return;
    }
    let id = request.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    if id.is_empty() {
        return;
    }
    let payload = wire::extension_payload(variant(proc), &request);
    proc.pi.lock().unwrap().requests.insert(id.clone(), request.clone());
    proc.permissions.lock().unwrap().insert(id.clone(), payload.clone());
    emit(app, session_id, json!({"type":"permission","request":payload}));
    emit_state(app, session_id, AgentState::Asking);
}

/// Answer an OMP extension dialog. The stored request carries the method and options the reply needs.
pub(super) fn respond_extension_ui(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    request_id: &str,
    allow: bool,
) -> Result<(), String> {
    let request = proc
        .pi
        .lock()
        .unwrap()
        .requests
        .get(request_id)
        .cloned()
        .ok_or("That permission request is no longer waiting for an answer")?;
    let response = wire::extension_response(&request, allow);
    proc.write(&response)?;
    proc.pi.lock().unwrap().requests.remove(request_id);
    proc.permissions.lock().unwrap().remove(request_id);
    emit(app, session_id, json!({"type":"permissionResolved","id":request_id}));
    if !proc.permissions.lock().unwrap().is_empty() {
        return Ok(());
    }
    if proc.turn.lock().unwrap().running {
        emit_state(app, session_id, AgentState::Working);
    } else {
        emit_state(app, session_id, AgentState::Waiting);
    }
    Ok(())
}

/// Let every waiting extension dialog through, used when the session switches to bypass.
pub(super) fn approve_pending(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let requests: Vec<(String, Value)> = proc
        .pi
        .lock()
        .unwrap()
        .requests
        .iter().map(|(id,request)|(id.clone(),request.clone()))
        .collect();
    if requests.is_empty() {
        return;
    }
    for (id, request) in &requests {
        let response = wire::extension_response(request, true);
        if proc.write(&response).is_err() {continue;}
        proc.pi.lock().unwrap().requests.remove(id);
        proc.permissions.lock().unwrap().remove(id);
        emit(app, session_id, json!({"type":"permissionResolved","id":id}));
    }
}

/// The catalogue as the composer wants it, or an empty list before the agent has answered.
pub(super) fn chat_models(proc: &Arc<ChatProcess>) -> Vec<Value> {
    let state = proc.pi.lock().unwrap();
    wire::to_chat_models(state.variant.unwrap_or(PiVariant::Pi), &state.models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_text_reads_strings_and_blocks() {
        assert_eq!(result_text(&json!("hello")), Some("hello".to_string()));
        assert_eq!(
            result_text(&json!({"content":[{"type":"text","text":"a"},{"type":"image"}]})),
            Some("a".to_string())
        );
        assert_eq!(result_text(&json!({"content":[]})), None);
    }
}
