//! Codex inside the chat engine, beyond its canonical item stream.
//!
//! The item notifications — messages, reasoning, commands, file changes — are projected onto the timeline
//! in `engine.rs`. This module holds what Codex reports *around* those items: why a turn failed, warnings
//! and reroutes, context usage, the streamed parts of a plan or a reasoning summary, the turn's
//! aggregated diff, and the two requests its own interface exposes as `/compact` and `/review`. It also
//! builds the permission cards for the approval requests whose answers carry more than a yes or a no.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{
    append, emit, emit_extras, emit_state, now_ms, place_codex, ChatProcess, ChatRow, SubagentInfo,
};
use crate::agent::chat::codex_protocol;
use crate::host::AppCtx;
use crate::pty::AgentState;

/// A fresh id for a row that has no native item behind it, such as a warning or a retry notice.
fn notice_id(proc: &ChatProcess, tag: &str) -> String {
    format!("n-{tag}-{}", proc.next_request.fetch_add(1, Ordering::Relaxed))
}

fn notice(proc: &Arc<ChatProcess>, tag: &str, message: String) {
    let id = notice_id(proc, tag);
    proc.timeline.lock().unwrap().upsert(ChatRow::Notice { id, message });
}

/// A `TurnError` as one sentence: the message, then any detail Codex attached to it.
///
/// The error class is spelled out only when the message does not already say it, so "usage limit
/// exceeded" is not followed by "(usage limit exceeded)".
pub(super) fn error_text(error: &Value) -> String {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("Codex reported an error");
    let mut text = message.to_string();
    if let Some(class) = error_class(error.get("codexErrorInfo")) {
        if !text.to_lowercase().contains(&class.to_lowercase()) {
            text = format!("{text} ({class})");
        }
    }
    for detail in [
        error.get("additionalDetails").and_then(Value::as_str),
        error.pointer("/misalignment/detailedExplanation").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|detail| !detail.is_empty() && *detail != message)
    {
        text.push_str("\n");
        text.push_str(detail);
    }
    text
}

/// The `CodexErrorInfo` variant in words a person can act on.
fn error_class(info: Option<&Value>) -> Option<&'static str> {
    let name = match info? {
        Value::String(name) => name.as_str(),
        Value::Object(map) => map.keys().next()?.as_str(),
        _ => return None,
    };
    Some(match name {
        "contextWindowExceeded" => "context window exceeded",
        "sessionBudgetExceeded" => "session budget exceeded",
        "usageLimitExceeded" => "usage limit exceeded",
        "rateLimitExceeded" => "rate limit exceeded",
        "serverOverloaded" => "server overloaded",
        "cyberPolicy" => "blocked by cyber policy",
        "misalignmentPolicyViolation" => "blocked by misalignment policy",
        "internalServerError" => "internal server error",
        "unauthorized" => "unauthorized",
        "badRequest" => "bad request",
        "threadRollbackFailed" => "thread rollback failed",
        "sandboxError" => "sandbox error",
        "httpConnectionFailed" => "HTTP connection failed",
        "responseStreamConnectionFailed" => "response stream connection failed",
        "responseStreamDisconnected" => "response stream disconnected",
        "responseTooManyFailedAttempts" => "too many failed attempts",
        _ => return None,
    })
}

/// Why the turn that just completed failed, if Codex said.
///
/// The `turn/completed` notification carries the error when the turn failed. When it does not, the
/// `error` notification that preceded it does, so its text is kept until the turn ends and read here.
pub(super) fn turn_failure(proc: &ChatProcess, params: &Value) -> Option<String> {
    let stored = proc.codex_turn_error.lock().unwrap().take();
    params
        .pointer("/turn/error")
        .filter(|error| !error.is_null())
        .map(error_text)
        .or(stored)
}

/// Notifications Codex sends outside its item stream. Answers true when the method was one of them.
///
/// Only the root thread reaches here: a subagent's warnings belong to its own Task card, and those are
/// routed before this is called.
pub(super) fn handle_notification(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    method: &str,
    params: &Value,
) -> bool {
    match method {
        "error" => {
            let message = params.get("error").map(error_text).unwrap_or_else(|| "Codex reported an error".into());
            if params.get("willRetry").and_then(Value::as_bool).unwrap_or(false) {
                notice(proc, "retry", format!("Codex is retrying after an error: {message}"));
            } else {
                // The turn that ends next carries this as its reason; drawing it now as well would show
                // the same failure twice.
                *proc.codex_turn_error.lock().unwrap() = Some(message);
            }
        }
        "warning" | "guardianWarning" => {
            if let Some(message) = params.get("message").and_then(Value::as_str) {
                notice(proc, "warn", message.to_string());
            }
        }
        "deprecationNotice" | "configWarning" => {
            let Some(summary) = params.get("summary").and_then(Value::as_str) else { return true };
            let mut message = summary.to_string();
            if let Some(path) = params.get("path").and_then(Value::as_str) {
                message.push_str(&format!(" ({path})"));
            }
            if let Some(details) = params.get("details").and_then(Value::as_str) {
                message.push_str(": ");
                message.push_str(details);
            }
            notice(proc, "warn", message);
        }
        "model/rerouted" => {
            let from = params.get("fromModel").and_then(Value::as_str).unwrap_or("?");
            let to = params.get("toModel").and_then(Value::as_str).unwrap_or("?");
            let reason = match params.get("reason").and_then(Value::as_str) {
                Some("highRiskCyberActivity") => "the request was judged high-risk cyber activity",
                Some(other) => other,
                None => "no reason given",
            };
            notice(proc, "reroute", format!("Codex rerouted this turn from {from} to {to}: {reason}."));
        }
        "model/safetyBuffering/updated" => {
            // One row per turn, so a notification that repeats while the wait continues does not stack.
            if params.get("showBufferingUi").and_then(Value::as_bool).unwrap_or(false) {
                let turn = params.get("turnId").and_then(Value::as_str).unwrap_or("turn");
                let mut message =
                    "Codex is taking extra time to review this request before answering.".to_string();
                if let Some(faster) = params.get("fasterModel").and_then(Value::as_str) {
                    message.push_str(&format!(" A faster answer is available from {faster}."));
                }
                proc.timeline.lock().unwrap().upsert(ChatRow::Notice {
                    id: format!("n-buffer-{turn}"),
                    message,
                });
            }
        }
        "thread/tokenUsage/updated" => {
            let Some(usage) = params.get("tokenUsage") else { return true };
            let field = |key: &str| usage.pointer(&format!("/last/{key}")).and_then(Value::as_u64).unwrap_or(0);
            // What the model was last sent and what it wrote back is what sits in the window now; the
            // reasoning it produced along the way is not carried into the next request.
            let in_window = field("totalTokens").saturating_sub(field("reasoningOutputTokens"));
            let window = usage.get("modelContextWindow").and_then(Value::as_u64);
            {
                let mut extras = proc.extras.lock().unwrap();
                extras.context_tokens = Some(in_window);
                if window.is_some() {
                    extras.context_window = window;
                }
            }
            emit_extras(app, session_id, proc);
        }
        "item/reasoning/summaryPartAdded" => {
            // Codex writes a summary as several parts and the deltas of one part run straight into the
            // next. The break between them is the paragraph the reader expects.
            let Some(id) = params.get("itemId").and_then(Value::as_str) else { return true };
            if params.get("summaryIndex").and_then(Value::as_u64).unwrap_or(0) == 0 {
                return true;
            }
            let text = append(proc, id, "\n\n");
            place_codex(
                &mut proc.timeline.lock().unwrap(),
                None,
                ChatRow::Reasoning { id: id.to_string(), text, streaming: true },
            );
        }
        "item/plan/delta" => {
            let Some(id) = params.get("itemId").and_then(Value::as_str) else { return true };
            let delta = params.get("delta").and_then(Value::as_str).unwrap_or("");
            // Its own buffer: the completed plan item replaces this text wholesale, and the protocol says
            // the concatenated deltas need not equal it.
            let text = append(proc, &format!("plan:{id}"), delta);
            place_codex(
                &mut proc.timeline.lock().unwrap(),
                None,
                ChatRow::Tool {
                    id: id.to_string(),
                    name: "ExitPlanMode".to_string(),
                    input: json!({"plan":text}),
                    output: None,
                    is_error: false,
                    status: "running",
                    subagent: None,
                    children: Vec::new(),
                },
            );
        }
        "turn/diff/updated" => {
            let Some(turn) = params.get("turnId").and_then(Value::as_str) else { return true };
            let diff = params.get("diff").and_then(Value::as_str).unwrap_or("");
            if diff.trim().is_empty() {
                return true;
            }
            // One card per turn, updated in place as more files change: the same aggregate Codex's own
            // interface prints for `/diff`, without having to ask for it.
            proc.timeline.lock().unwrap().upsert(ChatRow::Tool {
                id: format!("diff-{turn}"),
                name: "Diff".to_string(),
                input: json!({"diff":diff}),
                output: None,
                is_error: false,
                status: "completed",
                subagent: None,
                children: Vec::new(),
            });
        }
        "item/commandExecution/terminalInteraction" => {
            let Some(id) = params.get("itemId").and_then(Value::as_str) else { return true };
            let stdin = params.get("stdin").and_then(Value::as_str).unwrap_or("");
            if stdin.is_empty() {
                return true;
            }
            // What was typed into the running command, marked so it reads apart from the output.
            let written = stdin
                .lines()
                .map(|line| format!("\u{203a} {line}\n"))
                .collect::<String>();
            append_tool_output(proc, id, &written, None);
        }
        "item/mcpToolCall/progress" => {
            let Some(id) = params.get("itemId").and_then(Value::as_str) else { return true };
            if let Some(message) = params.get("message").and_then(Value::as_str) {
                append_tool_output(proc, id, &format!("{message}\n"), None);
            }
        }
        "item/fileChange/patchUpdated" => {
            let Some(id) = params.get("itemId").and_then(Value::as_str) else { return true };
            let Some(changes) = params.get("changes").cloned() else { return true };
            let mut timeline = proc.timeline.lock().unwrap();
            let Some(ChatRow::Tool { name, output, is_error, status, subagent, children, input, .. }) =
                timeline.get(id).cloned()
            else {
                return true;
            };
            let mut input = input;
            input["changes"] = changes;
            timeline.upsert(ChatRow::Tool { id: id.to_string(), name, input, output, is_error, status, subagent, children });
        }
        _ => return false,
    }
    true
}

/// Add text to a tool card's output without disturbing anything else on it. A card that is not there yet
/// is created running, the way a streamed output delta would create it.
fn append_tool_output(proc: &Arc<ChatProcess>, id: &str, text: &str, subagent: Option<SubagentInfo>) {
    let mut timeline = proc.timeline.lock().unwrap();
    let (name, input, output, previous_subagent, children, status, is_error) = match timeline.get(id) {
        Some(ChatRow::Tool { name, input, output, subagent, children, status, is_error, .. }) => (
            name.clone(),
            input.clone(),
            output.clone().unwrap_or_default(),
            subagent.clone(),
            children.clone(),
            *status,
            *is_error,
        ),
        _ => ("tool".to_string(), Value::Null, String::new(), None, Vec::new(), "running", false),
    };
    timeline.upsert(ChatRow::Tool {
        id: id.to_string(),
        name,
        input,
        output: Some(format!("{output}{text}")),
        is_error,
        status,
        subagent: subagent.or(previous_subagent),
        children,
    });
}

/// The card for a command Codex wants to run, with every standing rule the request lets it offer.
///
/// Besides allowing the command for the rest of the session, Codex may propose an execpolicy amendment —
/// a command prefix to stop asking about, written to the user's rules — and, when the command needs the
/// network, a rule for the host it wants to reach. Each becomes one button; `codex_protocol` turns the
/// one pressed back into the matching decision.
pub(super) fn command_approval_payload(key: &str, params: &Value) -> Value {
    let command = params.get("command").cloned().unwrap_or(Value::Null);
    let cwd = params.get("cwd").cloned().unwrap_or(Value::Null);
    let subject = command_text(&command).unwrap_or_else(|| "this command".to_string());
    let mut suggestions = vec![json!({
        "type":"codexAcceptForSession",
        "destination":"session",
        "subject":subject
    })];
    let amendment = params
        .get("proposedExecpolicyAmendment")
        .and_then(Value::as_array)
        .filter(|words| !words.is_empty());
    if let Some(words) = amendment {
        let prefix = words
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        if !prefix.is_empty() {
            suggestions.push(json!({
                "type":"codexExecpolicyAmendment",
                "destination":"userSettings",
                "subject":prefix,
                "amendment":words
            }));
        }
    }
    for entry in params
        .get("proposedNetworkPolicyAmendments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        // Only the rule that widens access is a button; a standing refusal is what "deny" already does.
        if entry.get("action").and_then(Value::as_str) != Some("allow") {
            continue;
        }
        let Some(host) = entry.get("host").and_then(Value::as_str) else { continue };
        suggestions.push(json!({
            "type":"codexNetworkPolicyAmendment",
            "destination":"userSettings",
            "subject":host,
            "amendment":entry
        }));
    }
    let mut input = json!({"command":command,"cwd":cwd});
    if let Some(network) = params.get("networkApprovalContext").filter(|value| value.is_object()) {
        input["network"] = network.clone();
    }
    json!({
        "id":key,
        "tool_name":"Bash",
        "display_name":"Codex",
        "description":params.get("reason").cloned().unwrap_or(Value::Null),
        "input":input,
        "permission_suggestions":suggestions
    })
}

/// A command as one line, whether it arrived as a string or as its argument list.
fn command_text(command: &Value) -> Option<String> {
    match command {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Array(words) => {
            let text = words.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ");
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

/// The card for an MCP server asking the person something, in the shape the elicitation card reads.
///
/// A form carries its schema and is filled in field by field; a URL asks for something to be completed
/// in the browser. Both are the same card Claude's elicitations use, so the answer travels the same way.
pub(super) fn elicitation_payload(key: &str, params: &Value) -> Value {
    let mode = match params.get("mode").and_then(Value::as_str) {
        Some("url") => "url",
        _ => "form",
    };
    json!({
        "id":key,
        "subtype":"elicitation",
        "tool_name":"MCP",
        "display_name":"Codex",
        "mcp_server_name":params.get("serverName").cloned().unwrap_or(Value::Null),
        "message":params.get("message").cloned().unwrap_or(Value::Null),
        "mode":mode,
        "url":params.get("url").cloned().unwrap_or(Value::Null),
        "requested_schema":params.get("requestedSchema").cloned().unwrap_or(Value::Null),
        "input":{
            "server":params.get("serverName").cloned().unwrap_or(Value::Null),
            "mode":params.get("mode").cloned().unwrap_or(Value::Null)
        }
    })
}

/// Fetch every page; runtime state and configured enablement are distinct facts.
pub(super) fn mcp_status(proc: &Arc<ChatProcess>) -> Result<Value, String> {
    let thread_id = open_thread_id(proc)?;
    let config = proc.request_and_wait("codex_mcp_config", |id|
        codex_protocol::request(id, "config/read", json!({"cwd":proc.cwd,"includeLayers":false})))?;
    let configured = config.pointer("/config/mcp_servers");
    let mut cursor = Value::Null;
    let mut seen = std::collections::HashSet::new();
    let mut servers = Vec::new();
    loop {
        let page = proc.request_and_wait("codex_mcp_status", |id|
            codex_protocol::request(id, "mcpServerStatus/list", json!({"threadId":thread_id,"cursor":cursor})))?;
        let data = page.get("data").and_then(Value::as_array).ok_or("Invalid Codex MCP catalogue")?;
        for entry in data {
            let Some(name) = entry.get("name").and_then(Value::as_str) else { continue };
            let settings = configured.and_then(|value| value.get(name));
            servers.push(mcp_server_row(entry, settings));
        }
        cursor = page.get("nextCursor").cloned().unwrap_or(Value::Null);
        if cursor.is_null() { break; }
        if !seen.insert(cursor.to_string()) { return Err("Codex repeated an MCP catalogue cursor".into()); }
    }
    Ok(json!({"mcpServers":servers}))
}

fn mcp_server_row(entry: &Value, settings: Option<&Value>) -> Value {
    let name = entry.get("name").and_then(Value::as_str).unwrap_or("");
    let enabled = settings.and_then(|s| s.get("enabled")).and_then(Value::as_bool).unwrap_or(true);
    let status = if !enabled { "disabled" } else {
        match entry.get("runtimeStatus").and_then(Value::as_str) {
            Some("connected") => "connected",
            Some("starting") => "pending",
            Some("disabled") => "disabled",
            Some("failed" | "authenticationRequired") => "failed",
            _ => "disconnected",
        }
    };
    let tools = entry.get("tools").and_then(Value::as_object).map(|tools|
        tools.keys().map(|name| json!({"name":name})).collect::<Vec<_>>()).unwrap_or_default();
    json!({"name":name,"status":status,"tools":tools,"serverInfo":entry.get("serverInfo"),
        "scope":"user","canToggle":settings.is_some() && config_key_name(name) && entry.get("pluginId").is_none_or(Value::is_null)})
}

fn config_key_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

pub(super) fn mcp_reconnect(proc: &Arc<ChatProcess>, server: &str) -> Result<Value, String> {
    ensure_mcp_idle(proc)?;
    let catalogue = mcp_status(proc)?;
    if !catalogue["mcpServers"].as_array().unwrap().iter().any(|s| s["name"] == server) {
        return Err("Unknown Codex MCP server".into());
    }
    proc.request_and_wait("codex_mcp_reload", |id|
        codex_protocol::request(id, "config/mcpServer/reload", json!({})))?;
    mcp_status(proc)
}

fn ensure_mcp_idle(proc: &ChatProcess) -> Result<(), String> {
    if proc.turn.lock().unwrap().running {
        return Err("Wait for the current turn to finish before changing MCP configuration".into());
    }
    Ok(())
}

/// Codex owns configuration writes and policy validation. The UI explains the user-wide scope.
pub(super) fn mcp_toggle(proc: &Arc<ChatProcess>, server: &str, enabled: bool) -> Result<Value, String> {
    ensure_mcp_idle(proc)?;
    let catalogue = mcp_status(proc)?;
    if !catalogue["mcpServers"].as_array().unwrap().iter().any(|s| s["name"] == server && s["canToggle"] == true) {
        return Err("This MCP server cannot be toggled through Codex configuration".into());
    }
    proc.request_and_wait("codex_mcp_toggle", |id|
        codex_protocol::request(id, "config/value/write", json!({
            "keyPath":format!("mcp_servers.{server}.enabled"),"value":enabled,"mergeStrategy":"replace"})))?;
    proc.request_and_wait("codex_mcp_reload", |id|
        codex_protocol::request(id, "config/mcpServer/reload", json!({})))?;
    mcp_status(proc)
}

fn open_thread_id(proc: &ChatProcess) -> Result<String, String> {
    if !proc.ready.load(Ordering::Relaxed) {
        return Err("Codex has not opened its thread yet".to_string());
    }
    proc.agent_session_id
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "Codex has not opened its thread yet".to_string())
}

/// `/compact`: summarize the conversation now. Codex reports the work as a `contextCompaction` item,
/// which draws the same marker an automatic compaction does.
pub(super) fn compact(proc: &Arc<ChatProcess>) -> Result<(), String> {
    let thread_id = open_thread_id(proc)?;
    let id = proc.request_id("compact");
    proc.write(&codex_protocol::thread_compact_start(&id, &thread_id))
}

/// `/review`: run Codex's code review as a turn of this conversation.
///
/// It is a turn like any other — it starts, streams, and completes — so it takes the turn lock the way a
/// message does, and what was asked for is shown as the message that started it.
pub(super) fn review(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    args: &str,
) -> Result<(), String> {
    let _action = proc.action.lock().unwrap();
    let thread_id = open_thread_id(proc)?;
    let started_at = {
        let mut turn = proc.turn.lock().unwrap();
        if turn.running {
            return Err("A turn is still running; wait for it to finish before starting a review".to_string());
        }
        turn.running = true;
        turn.interrupted = false;
        let now = now_ms();
        turn.started_at = Some(now);
        now
    };
    let row_id = format!("u-{}", proc.next_request.fetch_add(1, Ordering::Relaxed));
    let shown = if args.trim().is_empty() {
        "/review".to_string()
    } else {
        format!("/review {}", args.trim())
    };
    proc.timeline.lock().unwrap().upsert(ChatRow::User {
        at: Some(started_at as i64),
        id: row_id.clone(),
        text: shown,
        images: Vec::new(),
    });
    proc.pending_user_rows.lock().unwrap().push(row_id);
    let id = proc.request_id("review_start");
    if let Err(error) = proc.write(&codex_protocol::review_start(
        &id,
        &thread_id,
        codex_protocol::review_target(args),
    )) {
        let mut turn = proc.turn.lock().unwrap();
        turn.running = false;
        turn.started_at = None;
        return Err(error);
    }
    emit(app, session_id, json!({"type":"turnStarted","startedAt":started_at}));
    emit_state(app, session_id, AgentState::Working);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{ctx, inert_process};
    use super::*;
    use crate::models::SessionKind;

    fn rows(proc: &Arc<ChatProcess>) -> Vec<ChatRow> {
        proc.timeline.lock().unwrap().rows.clone()
    }

    /// A failed turn says why, in Codex's words, rather than only that it failed.
    #[test]
    fn a_turn_error_is_kept_for_the_turn_that_ends_with_it() {
        let app = ctx("codex-error");
        let proc = inert_process(SessionKind::Codex);
        let handled = handle_notification(
            &app,
            "s",
            &proc,
            "error",
            &json!({"threadId":"t","turnId":"turn-1","willRetry":false,
                    "error":{"message":"You have hit your usage limit","codexErrorInfo":"usageLimitExceeded"}}),
        );
        assert!(handled);
        assert!(rows(&proc).is_empty(), "the failure is drawn once, when the turn ends");
        let message = turn_failure(&proc, &json!({"turn":{"id":"turn-1","status":"failed","error":null}}));
        assert_eq!(message.as_deref(), Some("You have hit your usage limit (usage limit exceeded)"));
        assert!(turn_failure(&proc, &json!({"turn":{"status":"failed"}})).is_none(), "read once");
    }

    #[test]
    fn a_turn_error_on_the_completion_wins_and_names_its_class() {
        let proc = inert_process(SessionKind::Codex);
        let message = turn_failure(
            &proc,
            &json!({"turn":{"status":"failed","error":{
                "message":"The request was too large","codexErrorInfo":"contextWindowExceeded",
                "additionalDetails":"Compact the conversation and try again"}}}),
        );
        assert_eq!(
            message.as_deref(),
            Some("The request was too large (context window exceeded)\nCompact the conversation and try again")
        );
    }

    #[test]
    fn a_retried_error_is_a_notice_and_warnings_are_notices() {
        let app = ctx("codex-warn");
        let proc = inert_process(SessionKind::Codex);
        handle_notification(
            &app,
            "s",
            &proc,
            "error",
            &json!({"willRetry":true,"error":{"message":"stream disconnected"}}),
        );
        handle_notification(&app, "s", &proc, "warning", &json!({"message":"Sandbox is unavailable"}));
        handle_notification(
            &app,
            "s",
            &proc,
            "model/rerouted",
            &json!({"fromModel":"gpt-5.6-terra","toModel":"gpt-5.6-sol","reason":"highRiskCyberActivity"}),
        );
        let texts = rows(&proc)
            .into_iter()
            .map(|row| match row {
                ChatRow::Notice { message, .. } => message,
                other => panic!("unexpected row {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(texts.len(), 3);
        assert!(texts[0].contains("retrying") && texts[0].contains("stream disconnected"));
        assert_eq!(texts[1], "Sandbox is unavailable");
        assert!(texts[2].contains("gpt-5.6-terra") && texts[2].contains("gpt-5.6-sol"));
    }

    /// Token usage is what the composer's context meter reads; reasoning tokens are not in the window.
    #[test]
    fn token_usage_feeds_the_context_meter() {
        let app = ctx("codex-usage");
        let proc = inert_process(SessionKind::Codex);
        handle_notification(
            &app,
            "s",
            &proc,
            "thread/tokenUsage/updated",
            &json!({"threadId":"t","turnId":"u","tokenUsage":{
                "modelContextWindow":272000,
                "last":{"inputTokens":40000,"cachedInputTokens":30000,"outputTokens":2000,"reasoningOutputTokens":1500,"totalTokens":42000},
                "total":{"inputTokens":90000,"cachedInputTokens":0,"outputTokens":5000,"reasoningOutputTokens":3000,"totalTokens":95000}
            }}),
        );
        let extras = proc.extras.lock().unwrap().clone();
        assert_eq!(extras.context_tokens, Some(40500));
        assert_eq!(extras.context_window, Some(272000));
    }

    #[test]
    fn summary_parts_are_paragraphs_and_plan_deltas_stream() {
        let app = ctx("codex-parts");
        let proc = inert_process(SessionKind::Codex);
        append(&proc, "r1", "First thought");
        handle_notification(
            &app,
            "s",
            &proc,
            "item/reasoning/summaryPartAdded",
            &json!({"itemId":"r1","summaryIndex":1,"threadId":"t","turnId":"u"}),
        );
        assert_eq!(append(&proc, "r1", "Second"), "First thought\n\nSecond");
        handle_notification(
            &app,
            "s",
            &proc,
            "item/plan/delta",
            &json!({"itemId":"p1","delta":"1. Read","threadId":"t","turnId":"u"}),
        );
        handle_notification(
            &app,
            "s",
            &proc,
            "item/plan/delta",
            &json!({"itemId":"p1","delta":" the code","threadId":"t","turnId":"u"}),
        );
        let plan = rows(&proc)
            .into_iter()
            .find_map(|row| match row {
                ChatRow::Tool { id, input, status, .. } if id == "p1" => Some((input, status)),
                _ => None,
            })
            .expect("a plan card");
        assert_eq!(plan.0["plan"], "1. Read the code");
        assert_eq!(plan.1, "running");
    }

    #[test]
    fn the_turn_diff_is_one_card_updated_in_place() {
        let app = ctx("codex-diff");
        let proc = inert_process(SessionKind::Codex);
        for diff in ["--- a\n+++ b\n+one", "--- a\n+++ b\n+one\n+two"] {
            handle_notification(
                &app,
                "s",
                &proc,
                "turn/diff/updated",
                &json!({"threadId":"t","turnId":"u","diff":diff}),
            );
        }
        let cards = rows(&proc);
        assert_eq!(cards.len(), 1);
        match &cards[0] {
            ChatRow::Tool { id, name, input, .. } => {
                assert_eq!(id, "diff-u");
                assert_eq!(name, "Diff");
                assert_eq!(input["diff"], "--- a\n+++ b\n+one\n+two");
            }
            other => panic!("unexpected row {other:?}"),
        }
    }

    #[test]
    fn typed_stdin_lands_on_the_command_card_marked_as_input() {
        let app = ctx("codex-stdin");
        let proc = inert_process(SessionKind::Codex);
        proc.timeline.lock().unwrap().upsert(ChatRow::Tool {
            id: "c1".into(),
            name: "Bash".into(),
            input: json!({"command":"python"}),
            output: Some(">>> ".into()),
            is_error: false,
            status: "running",
            subagent: None,
            children: Vec::new(),
        });
        handle_notification(
            &app,
            "s",
            &proc,
            "item/commandExecution/terminalInteraction",
            &json!({"itemId":"c1","processId":"p","stdin":"print(1)\n","threadId":"t","turnId":"u"}),
        );
        match &rows(&proc)[0] {
            ChatRow::Tool { output, status, .. } => {
                assert_eq!(output.as_deref(), Some(">>> \u{203a} print(1)\n"));
                assert_eq!(*status, "running");
            }
            other => panic!("unexpected row {other:?}"),
        }
    }

    #[test]
    fn a_command_approval_offers_every_rule_the_request_allows() {
        let payload = command_approval_payload(
            "7",
            &json!({
                "command":["cargo","test","--all"],
                "cwd":"/work",
                "reason":"Run the test suite",
                "proposedExecpolicyAmendment":["cargo","test"],
                "networkApprovalContext":{"host":"crates.io","protocol":"https"},
                "proposedNetworkPolicyAmendments":[
                    {"action":"allow","host":"crates.io"},
                    {"action":"deny","host":"crates.io"}
                ]
            }),
        );
        assert_eq!(payload["tool_name"], "Bash");
        assert_eq!(payload["description"], "Run the test suite");
        assert_eq!(payload["input"]["network"]["host"], "crates.io");
        let offers = payload["permission_suggestions"].as_array().unwrap();
        assert_eq!(offers.len(), 3, "session, execpolicy, and the one allow rule");
        assert_eq!(offers[0]["type"], "codexAcceptForSession");
        assert_eq!(offers[0]["subject"], "cargo test --all");
        assert_eq!(offers[1]["type"], "codexExecpolicyAmendment");
        assert_eq!(offers[1]["subject"], "cargo test");
        assert_eq!(offers[1]["amendment"], json!(["cargo","test"]));
        assert_eq!(offers[2]["type"], "codexNetworkPolicyAmendment");
        assert_eq!(offers[2]["amendment"], json!({"action":"allow","host":"crates.io"}));
    }

    #[test]
    fn codex_mcp_inventory_preserves_runtime_state_and_configuration_scope() {
        let entry = json!({"name":"local-server","runtimeStatus":"starting","tools":{"read":{},"write":{}},"pluginId":null});
        let pending = mcp_server_row(&entry, Some(&json!({"enabled":true})));
        assert_eq!(pending["status"], "pending");
        assert_eq!(pending["tools"].as_array().unwrap().len(), 2);
        assert_eq!(pending["canToggle"], true);
        assert_eq!(mcp_server_row(&entry, Some(&json!({"enabled":false})))["status"], "disabled");
        assert_eq!(mcp_server_row(&entry, None)["canToggle"], false);
        let plugin = json!({"name":"plugin","runtimeStatus":"connected","pluginId":"x"});
        assert_eq!(mcp_server_row(&plugin, Some(&json!({})))["canToggle"], false);
        for name in ["", "a.b", "x\"", "x\n"] { assert!(!config_key_name(name)); }
    }

    #[test]
    fn an_elicitation_becomes_the_shared_form_card() {
        let payload = elicitation_payload(
            "9",
            &json!({
                "serverName":"github",
                "message":"Which repository?",
                "mode":"form",
                "requestedSchema":{"type":"object","properties":{"repo":{"type":"string"}},"required":["repo"]},
                "threadId":"t","turnId":null
            }),
        );
        assert_eq!(payload["subtype"], "elicitation");
        assert_eq!(payload["mode"], "form");
        assert_eq!(payload["mcp_server_name"], "github");
        assert_eq!(payload["requested_schema"]["required"][0], "repo");
        let url = elicitation_payload(
            "10",
            &json!({"serverName":"x","message":"Sign in","mode":"url","url":"https://example.test/login","elicitationId":"e"}),
        );
        assert_eq!(url["mode"], "url");
        assert_eq!(url["url"], "https://example.test/login");
    }

    /// A review is a turn: it takes the lock, shows what it was asked, and refuses to overlap another turn.
    #[test]
    fn a_review_runs_as_a_turn_of_the_conversation() {
        let app = ctx("codex-review");
        let proc = inert_process(SessionKind::Codex);
        *proc.agent_session_id.lock().unwrap() = Some("thread-1".into());
        // No stdin on an inert process: the write fails, and the turn lock is released again.
        let error = review(&app, "s", &proc, "branch main").unwrap_err();
        assert!(error.contains("not accepting input"), "{error}");
        assert!(!proc.turn.lock().unwrap().running);
        assert!(matches!(&rows(&proc)[0], ChatRow::User { text, .. } if text == "/review branch main"));
        proc.turn.lock().unwrap().running = true;
        let error = review(&app, "s", &proc, "").unwrap_err();
        assert!(error.contains("still running"), "{error}");
    }
}
