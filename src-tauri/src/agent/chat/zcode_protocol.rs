//! ZCode's `app-server` wire format and the rows it projects.
//!
//! `zcode app-server --stdio` speaks the "ZCode Protocol" as NDJSON: JSON-RPC-shaped requests, responses and
//! notifications without a `jsonrpc` field (the server's schemas are strict and reject unknown keys). A chat
//! client uses its V4 surface, the one ZCode's own desktop app drives: `v4/command` carries every user action
//! (`createSession`, `sendText`, `stop`, `compact`, `resolveInteraction`), and a subscription to
//! `conversation/<sessionId>` streams the conversation back as `v4/conversation/frame` notifications. A frame
//! holds either a snapshot (state plus the last rows) or a list of deltas against it.
//!
//! The server projects the conversation into rows that are already close to what the view draws: a user
//! input, an assistant text, a reasoning block, a tool call with its lifecycle, a subagent, a timeline
//! marker. This module turns those into `ChatRow`s and builds the messages the engine sends; the engine side
//! lives in `engine/zcode.rs`. Shapes were checked against ZCode 0.17.0 (`packages/shared/src/zcode-protocol*`)
//! and a live `app-server` process.

use std::collections::HashMap;

use base64::Engine as _;
use serde_json::{json, Value};

use super::engine::ChatRow;

/// The client identity every command carries. ZCode records it on the inputs a client submits.
pub const CLIENT_ID: &str = "velaterm";

/// The streaming mode whose deltas cover assistant text, tool input and tool output.
pub const CLIENT_MODE: &str = "desktop-continuous";

/// Row ids drawn from the server's numeric row ids, kept apart from replayed (`h-`) and local (`u-`) rows.
pub fn row_id(raw: i64) -> String {
    format!("z-{raw}")
}

/// One NDJSON line from the server.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    /// The answer to one of our requests.
    Response { id: String, result: Value, error: Option<String> },
    /// A request the server sends us; it waits until it is answered.
    Request { id: Value, method: String, params: Value },
    /// Any notification, conversation frames included.
    Notification { method: String, params: Value },
}

pub fn parse_line(line: &str) -> Option<Incoming> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let method = value.get("method").and_then(Value::as_str).map(str::to_string);
    let id = value.get("id").cloned().filter(|id| !id.is_null());
    match (method, id) {
        (Some(method), Some(id)) => Some(Incoming::Request {
            id,
            method,
            params: value.get("params").cloned().unwrap_or(Value::Null),
        }),
        (Some(method), None) => Some(Incoming::Notification {
            method,
            params: value.get("params").cloned().unwrap_or(Value::Null),
        }),
        (None, Some(id)) => {
            let id = match id {
                Value::String(id) => id,
                other => other.to_string(),
            };
            let error = value.get("error").map(|error| {
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("The agent rejected the request")
                    .to_string()
            });
            Some(Incoming::Response {
                id,
                result: value.get("result").cloned().unwrap_or(Value::Null),
                error,
            })
        }
        (None, None) => None,
    }
}

pub fn request(id: &str, method: &str, params: Value) -> Value {
    json!({"id": id, "method": method, "params": params})
}

/// Answer one of the server's requests.
pub fn reply(id: &Value, result: Value) -> Value {
    json!({"id": id, "result": result})
}

/// Decline a server request this client does not implement; the server falls back on its own default.
pub fn reply_unsupported(id: &Value) -> Value {
    json!({"id": id, "error": {"code": -32601, "message": "Not supported by this client"}})
}

/// A `v4/command` request. `base_revision` is required only by compare-and-set command types.
pub fn command(
    id: &str,
    command_id: &str,
    session_id: Option<&str>,
    kind: &str,
    payload: Value,
    now_ms: u64,
) -> Value {
    request(
        id,
        "v4/command",
        json!({
            "commandId": command_id,
            "clientId": CLIENT_ID,
            "sessionId": session_id,
            "type": kind,
            "payload": payload,
            "issuedAt": now_ms,
        }),
    )
}

/// Whether a command acknowledgement means the command took effect. `duplicate` and `noop` are successes
/// too: a retried command, or a change that was already in place.
pub fn ack_ok(result: &Value) -> bool {
    matches!(
        result.get("status").and_then(Value::as_str),
        Some("accepted" | "duplicate" | "noop")
    )
}

/// Why a command acknowledgement refused the command.
pub fn ack_error(result: &Value) -> String {
    result
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| result.get("reasonCode").and_then(Value::as_str))
        .unwrap_or("ZCode rejected the request")
        .to_string()
}

/// Shown when a conversation has no reply a fork can branch from yet.
pub const FORK_NOT_READY: &str = "This conversation can be forked once its latest reply has finished.";
/// Shown for any other failure to fork; the reason itself goes to the diagnostics log.
pub const FORK_FAILED: &str = "ZCode could not fork this conversation. Please try again.";

/// The reply a fork branches from, with the conversation state it was read at.
#[derive(Debug, PartialEq)]
pub struct ForkTarget {
    pub row_id: i64,
    pub entity_id: String,
    pub revision: u64,
    pub log_epoch: String,
}

/// Where a fork of this snapshot branches: the latest reply, provided ZCode offers to fork from it. It does not
/// once that reply's turn was cancelled or failed.
pub fn fork_target(snapshot: &Value) -> Option<ForkTarget> {
    let reply = snapshot
        .pointer("/rows/window")?
        .as_array()?
        .iter()
        .rev()
        .find(|row| row.get("kind").and_then(Value::as_str) == Some("assistantText"))?;
    if reply.pointer("/actions/canFork").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    Some(ForkTarget {
        row_id: reply.get("rowId")?.as_i64()?,
        entity_id: reply.get("entityId")?.as_str()?.to_string(),
        revision: snapshot.get("revision")?.as_u64()?,
        log_epoch: snapshot.get("logEpoch")?.as_str()?.to_string(),
    })
}

/// A `forkAssistant` command, which copies the conversation up to the reply and leaves the workspace alone. It
/// is compare-and-set, so unlike `command` it carries the state the target was read at.
pub fn fork_command(id: &str, command_id: &str, session_id: &str, target: &ForkTarget, now_ms: u64) -> Value {
    request(
        id,
        "v4/command",
        json!({
            "commandId": command_id,
            "clientId": CLIENT_ID,
            "sessionId": session_id,
            "type": "forkAssistant",
            "baseRevision": target.revision,
            "baseLogEpoch": target.log_epoch,
            "payload": {"target": {"rowId": target.row_id, "entityId": target.entity_id}},
            "issuedAt": now_ms,
        }),
    )
}

/// The conversation's current revision, when a compare-and-set command was turned down only for naming an
/// older one. Retrying with it is safe: the target row is still checked by its entity id.
pub fn stale_revision(ack: &Value) -> Option<u64> {
    if ack.get("reasonCode").and_then(Value::as_str) != Some("proto.staleRevision") {
        return None;
    }
    ack.get("revisionAtDecision").and_then(Value::as_u64)
}

/// The message for a refused fork. ZCode's `guard.*` reasons all mean the latest reply cannot be branched from.
pub fn fork_refusal(ack: &Value) -> &'static str {
    let guarded = ack.get("reasonCode").and_then(Value::as_str).is_some_and(|code| code.starts_with("guard."));
    if guarded { FORK_NOT_READY } else { FORK_FAILED }
}

/// A compare-and-set `v4/command`: ZCode applies it only while the conversation is still at `revision` in
/// `log_epoch`, the state the caller read its target in.
#[allow(clippy::too_many_arguments)]
pub fn cas_command(
    id: &str,
    command_id: &str,
    session_id: &str,
    kind: &str,
    payload: Value,
    revision: u64,
    log_epoch: &str,
    now_ms: u64,
) -> Value {
    request(
        id,
        "v4/command",
        json!({
            "commandId": command_id,
            "clientId": CLIENT_ID,
            "sessionId": session_id,
            "type": kind,
            "baseRevision": revision,
            "baseLogEpoch": log_epoch,
            "payload": payload,
            "issuedAt": now_ms,
        }),
    )
}

/// How a row action names its row: the projection id and the entity behind it, which ZCode checks are still
/// the same before acting.
pub fn row_target(row: &Value) -> Option<Value> {
    Some(json!({"rowId": row.get("rowId")?.as_i64()?, "entityId": row.get("entityId")?.as_str()?}))
}

/// The latest message the user typed, and the turn header it opened. ZCode offers to edit only this one
/// (`actions.canEdit`), and its file changes are the only ones whose restore means "as before this message":
/// an older turn's restore undoes that turn alone.
pub fn latest_query<'a>(rows: impl IntoIterator<Item = &'a Value>) -> Option<(&'a Value, Option<&'a Value>)> {
    let rows: Vec<&Value> = rows.into_iter().collect();
    let query = rows
        .iter()
        .filter(|row| str_of(row, "kind") == "userInput" && str_of(row, "origin") == "realUser")
        .max_by_key(|row| row.get("rowId").and_then(Value::as_i64).unwrap_or(i64::MIN))
        .copied()?;
    let turn = str_of(query, "turnId");
    let header = rows
        .iter()
        .find(|row| str_of(row, "kind") == "turnHeader" && !turn.is_empty() && str_of(row, "turnId") == turn)
        .copied();
    Some((query, header))
}

pub fn can_edit(query: &Value) -> bool {
    query.pointer("/actions/canEdit").and_then(Value::as_bool) == Some(true)
}

pub fn can_restore_files(header: Option<&Value>) -> bool {
    header.and_then(|h| h.pointer("/actions/canRewindFiles")).and_then(Value::as_bool) == Some(true)
}

/// Shown when ZCode has nothing to restore for the latest turn, or restored nothing.
pub const NO_FILE_CHANGES: &str = "ZCode has no file changes it can restore for this message.";

/// ZCode's file restore preview in the view's shape: whether it can apply, the files it would restore
/// (relative to the workspace when inside it), and the turn's line counts.
pub fn rewind_preview(preview: &Value, header: &Value, workspace: &str) -> Value {
    let relative = |path: &str| {
        let root = workspace.trim_end_matches('/');
        path.strip_prefix(root).and_then(|rest| rest.strip_prefix('/')).unwrap_or(path).to_string()
    };
    let paths = |key: &str| -> Vec<String> {
        preview.get(key).and_then(Value::as_array).into_iter().flatten()
            .filter_map(|file| file.get("path").and_then(Value::as_str)).map(relative).collect()
    };
    let can_apply = preview.get("canApply").and_then(Value::as_bool) == Some(true);
    let mut value = json!({
        "canRewind": can_apply,
        "filesChanged": paths("safeFiles"),
        "insertions": header.pointer("/fileChanges/additions").and_then(Value::as_u64).unwrap_or(0),
        "deletions": header.pointer("/fileChanges/deletions").and_then(Value::as_u64).unwrap_or(0),
    });
    if !can_apply {
        let blocked = paths("unsafeFiles");
        value["error"] = json!(if blocked.is_empty() {
            NO_FILE_CHANGES.to_string()
        } else {
            format!("ZCode cannot restore files that changed after this message: {}", blocked.join(", "))
        });
    }
    value
}

pub fn subscribe_params(session_id: &str, connection_id: &str, workspace: &str) -> Value {
    json!({
        "topic": format!("conversation/{session_id}"),
        "connectionId": connection_id,
        "clientMode": CLIENT_MODE,
        "workspace": {"workspacePath": workspace, "workspaceKey": workspace},
    })
}

/// ZCode's permission mode for one of the view's modes. The view reuses the shared vocabulary: ask before
/// changes (`default`, ZCode's `build`), accept edits (`edit`), plan, and bypass (`yolo`).
pub fn native_mode(mode: &str) -> &'static str {
    match mode {
        "plan" => "plan",
        "acceptEdits" => "edit",
        "bypassPermissions" => "yolo",
        _ => "build",
    }
}

/// Split a view model id (`<providerId>/<modelId>`) into ZCode's selection. The provider id never contains a
/// slash; a model id may.
pub fn model_selection(model: &str, effort: Option<&str>) -> Option<Value> {
    let (provider, model) = model.split_once('/')?;
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    let mut selection = json!({"providerId": provider, "modelId": model});
    if let Some(level) = effort.map(str::trim).filter(|level| !level.is_empty()) {
        selection["options"] = json!({"reasoningLevel": level});
    }
    Some(selection)
}

/// The view model id for a ZCode selection or session config.
pub fn model_id(provider: Option<&str>, model: Option<&str>) -> Option<String> {
    let provider = provider.filter(|p| !p.is_empty())?;
    let model = model.filter(|m| !m.is_empty())?;
    Some(format!("{provider}/{model}"))
}

/// The catalogue `settings.model.available` lists, in the shape the composer's model picker reads.
pub fn to_chat_models(available: &[Value]) -> Vec<Value> {
    available
        .iter()
        .filter(|model| model.get("disabledReason").is_none())
        .filter_map(|model| {
            let id = model_id(
                model.pointer("/ref/providerId").and_then(Value::as_str),
                model.pointer("/ref/modelId").and_then(Value::as_str),
            )?;
            let label = model
                .get("label")
                .and_then(Value::as_str)
                .filter(|label| !label.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| id.clone());
            let levels: Vec<String> = model
                .pointer("/reasoning/levels")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|level| level.get("value").and_then(Value::as_str).map(str::to_string))
                .collect();
            Some(json!({
                "id": id,
                "label": label,
                "description": model.get("providerLabel").cloned().unwrap_or(Value::Null),
                "effortLevels": levels,
                "defaultEffort": model.pointer("/reasoning/defaultLevel").cloned().unwrap_or(Value::Null),
                "contextWindow": model.get("contextWindow").and_then(Value::as_u64).filter(|w| *w > 0),
                "curated": true,
                "largeContext": false,
            }))
        })
        .collect()
}

/// The reasoning level a catalogue entry starts with, for a model chosen without one.
pub fn default_effort(models: &[Value], model: &str) -> Option<String> {
    models
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(model))
        .and_then(|entry| {
            entry
                .get("defaultEffort")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    entry
                        .get("effortLevels")
                        .and_then(Value::as_array)
                        .and_then(|levels| levels.last())
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
        })
}

/// Slash commands for the composer's completion. `/compact` is excluded: the view runs it as a command of
/// its own rather than as text.
pub fn slash_commands(commands: &[Value]) -> Vec<Value> {
    commands
        .iter()
        .filter_map(|command| {
            let name = command.get("name").and_then(Value::as_str)?;
            if name.is_empty() || name == "compact" {
                return None;
            }
            Some(json!({
                "name": name,
                "description": command.get("description").cloned().unwrap_or(Value::Null),
                "argumentHint": command.get("inputHint").cloned().unwrap_or(Value::Null),
                "invocation": "/",
            }))
        })
        .collect()
}

/// The tool name the view's cards know. ZCode's tools mirror Claude Code's names and input fields, except
/// that the subagent tool is called `Agent` rather than `Task`.
pub fn tool_name(name: &str) -> String {
    match name {
        "Agent" => "Task".to_string(),
        other => other.to_string(),
    }
}

fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn tool_status(status: &str) -> &'static str {
    match status {
        "success" => "completed",
        "error" => "failed",
        "cancelled" => "canceled",
        _ => "running",
    }
}

/// Convert one projected row into the view's row under `id`. Rows the view does not draw — turn headers,
/// artifacts, hook invocations, rows ZCode marks as hidden — return None.
pub fn convert_row(row: &Value, id: String) -> Option<ChatRow> {
    if row.get("visibility").and_then(Value::as_str).is_some_and(|v| v != "visible") {
        return None;
    }
    let at = row.get("createdAt").and_then(Value::as_i64);
    let state = str_of(row, "state");
    match str_of(row, "kind") {
        "userInput" => Some(super::engine::user_row(id, str_of(row, "text"), Vec::new(), at)),
        "assistantText" => Some(ChatRow::Assistant {
            id,
            text: str_of(row, "text").to_string(),
            streaming: state == "streaming",
            model: row
                .get("model")
                .and_then(Value::as_str)
                .filter(|m| !m.is_empty())
                .map(str::to_string),
            at,
            duration_ms: None,
        }),
        "reasoning" => Some(ChatRow::Reasoning {
            id,
            text: str_of(row, "text").to_string(),
            streaming: state == "streaming",
        }),
        "toolCall" => {
            let status = str_of(row, "status");
            let input = row
                .get("input")
                .filter(|input| !input.is_null())
                .cloned()
                .or_else(|| serde_json::from_str(str_of(row, "inputText")).ok())
                .unwrap_or_else(|| json!({}));
            let output = row
                .pointer("/output/text")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| row.pointer("/error/message").and_then(Value::as_str).map(str::to_string));
            Some(ChatRow::Tool {
                id,
                name: tool_name(str_of(row, "toolName")),
                input,
                output,
                is_error: status == "error",
                status: tool_status(status),
                subagent: None,
                children: Vec::new(),
            })
        }
        "subagent" => {
            let status = str_of(row, "status");
            let summary = str_of(row, "summaryText").to_string();
            let done = status != "running";
            Some(ChatRow::Tool {
                id,
                name: "Task".to_string(),
                input: json!({"subagent_type": str_of(row, "subagentType"), "description": summary}),
                output: done.then_some(summary).filter(|s| !s.is_empty()),
                is_error: status == "failed",
                status: match status {
                    "success" => "completed",
                    "failed" => "failed",
                    "cancelled" => "canceled",
                    _ => "running",
                },
                subagent: None,
                children: Vec::new(),
            })
        }
        "timelineMarker" => marker_row(row.get("marker")?, id),
        _ => None,
    }
}

fn marker_row(marker: &Value, id: String) -> Option<ChatRow> {
    match str_of(marker, "type") {
        "compact" => {
            let trigger = marker.get("origin").and_then(Value::as_str).map(str::to_string);
            let pre_tokens = marker.get("tokensBefore").and_then(Value::as_u64);
            match str_of(marker, "status") {
                "running" => Some(ChatRow::Compaction { id, status: "loading", trigger, pre_tokens }),
                "success" => Some(ChatRow::Compaction { id, status: "completed", trigger, pre_tokens }),
                "noop" => Some(ChatRow::Notice { id, message: "There was nothing to compact.".to_string() }),
                "cancelled" => Some(ChatRow::Notice { id, message: "Compaction was cancelled.".to_string() }),
                _ => Some(ChatRow::Error { id, message: "Compaction failed.".to_string() }),
            }
        }
        "modelChange" => {
            let model = str_of(marker, "toModel");
            (!model.is_empty()).then(|| ChatRow::Notice { id, message: format!("Switched to {model}") })
        }
        "retryNotice" => {
            let attempt = marker.get("attempt").and_then(Value::as_u64).unwrap_or(0);
            Some(ChatRow::Notice { id, message: format!("Retrying the request (attempt {attempt})…") })
        }
        "goalSet" => {
            let objective = str_of(marker, "objective");
            (!objective.is_empty()).then(|| ChatRow::Notice { id, message: format!("Goal: {objective}") })
        }
        _ => None,
    }
}

/// Apply a streaming text delta to a cached row. `path` names the field: `text`, `inputText`, `output.text`
/// or `summaryText`.
pub fn apply_delta(row: &mut Value, path: &str, append: &str) {
    let Some(object) = row.as_object_mut() else { return };
    if let Some((parent, field)) = path.split_once('.') {
        let parent = object.entry(parent.to_string()).or_insert_with(|| json!({}));
        if !parent.is_object() {
            *parent = json!({});
        }
        let slot = parent.as_object_mut().expect("object").entry(field.to_string()).or_insert(json!(""));
        let mut text = slot.as_str().unwrap_or("").to_string();
        text.push_str(append);
        *slot = Value::String(text);
    } else {
        let slot = object.entry(path.to_string()).or_insert(json!(""));
        let mut text = slot.as_str().unwrap_or("").to_string();
        text.push_str(append);
        *slot = Value::String(text);
    }
}

/// Frames larger than the server's single-line budget arrive in base64 fragments that share a logical id.
/// Returns the reassembled inner frame once every fragment is in, or the inner frame of a complete one.
pub fn decode_frame(params: &Value, fragments: &mut HashMap<String, Vec<Option<String>>>) -> Option<Value> {
    if params.get("kind").and_then(Value::as_str) != Some("fragment") {
        return params.get("frame").cloned();
    }
    let key = params.get("logicalFrameId").and_then(Value::as_str)?.to_string();
    let index = params.get("fragmentIndex").and_then(Value::as_u64)? as usize;
    let count = params.get("fragmentCount").and_then(Value::as_u64)? as usize;
    if count == 0 || index >= count || count > 4096 {
        return None;
    }
    let data = params.get("dataBase64").and_then(Value::as_str)?.to_string();
    let parts = fragments.entry(key.clone()).or_insert_with(|| vec![None; count]);
    if parts.len() != count {
        fragments.remove(&key);
        return None;
    }
    parts[index] = Some(data);
    if parts.iter().any(Option::is_none) {
        return None;
    }
    let parts = fragments.remove(&key)?;
    let mut bytes = Vec::new();
    for part in parts.into_iter().flatten() {
        bytes.extend(base64::engine::general_purpose::STANDARD.decode(part).ok()?);
    }
    let frame: Value = serde_json::from_slice(&bytes).ok()?;
    // A fragment set may carry the inner frame directly or the complete wire frame around it.
    Some(frame.get("frame").cloned().unwrap_or(frame))
}

/// A pending interaction as the permission card understands it: an approval, a question form, or a plan
/// to approve. The ZCode request travels along under `_zcode*` keys so the answer can be built from it.
pub fn interaction_payload(interaction: &Value) -> Option<Value> {
    let id = interaction.get("interactionId").and_then(Value::as_str).filter(|id| !id.is_empty())?;
    let payload = interaction.get("payload")?;
    let tool = str_of(payload, "toolName");
    match str_of(payload, "kind") {
        "permission" => {
            let options = payload.get("options").and_then(Value::as_array).cloned().unwrap_or_default();
            let suggestions: Vec<Value> = options
                .iter()
                .filter_map(|option| {
                    let option_id = option.get("optionId").and_then(Value::as_str)?;
                    let kind = str_of(option, "kind");
                    let session = kind == "allowSession" || option_id == "allowSession";
                    if kind != "allowAlways" && !session {
                        return None;
                    }
                    let subject = option
                        .pointer("/response/permissionUpdates/0/rules/0/ruleContent")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .unwrap_or(if tool.is_empty() { "this action" } else { tool });
                    Some(json!({
                        "type": if session { "zcodeSession" } else { "zcodeAlways" },
                        "subject": subject,
                        "optionId": option_id,
                    }))
                })
                .collect();
            let mut card = json!({
                "id": id,
                "tool_name": tool_name(if tool.is_empty() { "tool" } else { tool }),
                "display_name": "ZCode",
                "input": payload.get("detail").cloned().unwrap_or_else(|| json!({})),
                "_zcodeKind": "permission",
            });
            if let Some(summary) = payload.get("summary").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                card["description"] = json!(summary);
            }
            if !suggestions.is_empty() {
                card["permission_suggestions"] = json!(suggestions);
            }
            Some(card)
        }
        "userInput" => {
            let prompt = str_of(payload, "prompt");
            if tool == "ExitPlanMode" {
                return Some(json!({
                    "id": id,
                    "tool_name": "ExitPlanMode",
                    "display_name": "ZCode",
                    "input": {"plan": prompt},
                    "_zcodeKind": "plan",
                }));
            }
            let mut questions: Vec<Value> = payload
                .get("questions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|question| {
                    let text = question.get("question").and_then(Value::as_str)?;
                    let options: Vec<Value> = question
                        .get("options")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|option| {
                            let label = option.get("label").and_then(Value::as_str)?;
                            let mut out = json!({"label": label});
                            if let Some(description) = option.get("description").and_then(Value::as_str) {
                                out["description"] = json!(description);
                            }
                            Some(out)
                        })
                        .collect();
                    Some(json!({
                        "question": text,
                        "header": question.get("header").and_then(Value::as_str).unwrap_or(text),
                        "options": options,
                        "multiSelect": question.get("multiSelect").and_then(Value::as_bool).unwrap_or(false),
                        "allowOther": payload.get("freeText").and_then(Value::as_bool).unwrap_or(true),
                        "allowEmpty": false,
                    }))
                })
                .collect();
            if questions.is_empty() {
                // A request with no structured questions still needs an answer: ask the prompt itself.
                let text = if prompt.is_empty() { "ZCode needs your input" } else { prompt };
                questions.push(json!({
                    "question": text, "header": "ZCode", "options": [], "multiSelect": false,
                    "allowOther": true, "allowEmpty": false,
                }));
            }
            Some(json!({
                "id": id,
                "tool_name": "AskUserQuestion",
                "display_name": "ZCode",
                "input": {"questions": questions},
                "_zcodeKind": "question",
            }))
        }
        _ => None,
    }
}

/// The `resolveInteraction` answer for a card's reply. An allowed approval picks the standing rule the card
/// adopted, if any, and otherwise allows once; a denial may carry the person's reason as free text.
pub fn interaction_answer(
    request: &Value,
    allow: bool,
    updated_input: Option<&Value>,
    message: Option<&str>,
    updated_permissions: Option<&Value>,
) -> Value {
    let message = message.map(str::trim).filter(|m| !m.is_empty());
    match str_of(request, "_zcodeKind") {
        "question" => {
            if !allow {
                return json!({"action": "decline"});
            }
            let answers = updated_input
                .and_then(|input| input.get("answers"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            json!({"action": "accept", "content": {"answers": answers}})
        }
        "plan" => {
            if allow {
                json!({"optionId": "allowOnce"})
            } else if let Some(message) = message {
                json!({"freeText": message})
            } else {
                json!({"action": "decline"})
            }
        }
        _ => {
            if !allow {
                let mut answer = json!({"optionId": "deny"});
                if let Some(message) = message {
                    answer["freeText"] = json!(message);
                }
                return answer;
            }
            let adopted = updated_permissions
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find_map(|suggestion| {
                    matches!(str_of(suggestion, "type"), "zcodeAlways" | "zcodeSession")
                        .then(|| suggestion.get("optionId").and_then(Value::as_str))
                        .flatten()
                });
            json!({"optionId": adopted.unwrap_or("allowOnce")})
        }
    }
}

/// The checksum `v4/attachment/begin` requires: SHA-256 of the whole file, hex encoded.
pub fn attachment_checksum(bytes: &[u8]) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

/// Upload chunk size, under the server's 512 KiB-per-chunk limit.
pub const ATTACHMENT_CHUNK: usize = 256 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fork_branches_from_the_latest_reply_only_when_zcode_offers_it() {
        let reply = |actions: Value| json!({"rowId":7,"kind":"assistantText","entityId":"msg_1","actions":actions});
        let snapshot = |rows: Value| json!({"revision":5,"logEpoch":"e1","rows":{"window":rows}});
        let marker = json!({"rowId":8,"kind":"timelineMarker","entityId":"m"});
        assert_eq!(
            fork_target(&snapshot(json!([reply(json!({"canFork":true})), marker]))),
            Some(ForkTarget { row_id: 7, entity_id: "msg_1".into(), revision: 5, log_epoch: "e1".into() })
        );
        assert_eq!(fork_target(&snapshot(json!([reply(json!({}))]))), None);
        assert_eq!(fork_target(&snapshot(json!([marker]))), None);
    }

    #[test]
    fn a_fork_command_carries_the_state_its_target_was_read_at() {
        let target = ForkTarget { row_id: 7, entity_id: "msg_1".into(), revision: 5, log_epoch: "e1".into() };
        let command = fork_command("r1", "c1", "sess_1", &target, 9);
        assert_eq!(command["method"], "v4/command");
        assert_eq!(command["params"]["type"], "forkAssistant");
        assert_eq!(command["params"]["baseRevision"], 5);
        assert_eq!(command["params"]["baseLogEpoch"], "e1");
        assert_eq!(command["params"]["payload"]["target"], json!({"rowId":7,"entityId":"msg_1"}));
    }

    #[test]
    fn fork_acknowledgements_say_whether_to_retry_and_what_to_show() {
        let stale = json!({"status":"stale","reasonCode":"proto.staleRevision","revisionAtDecision":9});
        assert_eq!(stale_revision(&stale), Some(9));
        assert_eq!(stale_revision(&json!({"status":"stale","reasonCode":"proto.staleLogEpoch","revisionAtDecision":9})), None);
        assert_eq!(fork_refusal(&json!({"status":"rejected","reasonCode":"guard.forkTargetNotStable"})), FORK_NOT_READY);
        assert_eq!(fork_refusal(&stale), FORK_FAILED);
    }

    #[test]
    fn only_the_latest_query_and_its_own_turn_are_rewind_targets() {
        let rows = [
            json!({"rowId":1,"kind":"turnHeader","turnId":"t1","entityId":"t1","actions":{"canRewindFiles":true}}),
            json!({"rowId":2,"kind":"userInput","origin":"realUser","turnId":"t1","entityId":"m1"}),
            json!({"rowId":8,"kind":"turnHeader","turnId":"t2","entityId":"t2","actions":{"canRewindFiles":true}}),
            json!({"rowId":9,"kind":"userInput","origin":"realUser","turnId":"t2","entityId":"m2","actions":{"canEdit":true}}),
            json!({"rowId":10,"kind":"userInput","origin":"backgroundResult","turnId":"t3","entityId":"m3"}),
        ];
        let (query, header) = latest_query(rows.iter().rev()).unwrap();
        assert_eq!(query["rowId"], 9);
        assert_eq!(header.unwrap()["rowId"], 8);
        assert!(can_edit(query) && can_restore_files(header));
        assert_eq!(row_target(query), Some(json!({"rowId":9,"entityId":"m2"})));
        assert!(!can_restore_files(Some(&json!({"rowId":8,"kind":"turnHeader"}))));
        assert!(latest_query([&json!({"rowId":3,"kind":"assistantText"})]).is_none());
    }

    #[test]
    fn row_actions_are_compare_and_set() {
        let command = cas_command("r1", "c1", "sess_1", "editUserQuery", json!({"newText":"x"}), 27, "e1", 9);
        assert_eq!(command["params"]["type"], "editUserQuery");
        assert_eq!(command["params"]["baseRevision"], 27);
        assert_eq!(command["params"]["baseLogEpoch"], "e1");
        assert_eq!(command["params"]["sessionId"], "sess_1");
    }

    #[test]
    fn file_restore_previews_list_workspace_paths_and_say_why_not() {
        let header = json!({"fileChanges":{"additions":3,"deletions":1}});
        let safe = json!({"canApply":true,"safeFiles":[{"path":"/w/src/a.txt"},{"path":"/elsewhere/b.txt"}],"unsafeFiles":[]});
        assert_eq!(rewind_preview(&safe, &header, "/w/"),
            json!({"canRewind":true,"filesChanged":["src/a.txt","/elsewhere/b.txt"],"insertions":3,"deletions":1}));
        let blocked = json!({"canApply":false,"safeFiles":[],"unsafeFiles":[{"path":"/w/a.txt","reason":"external_modified"}]});
        let value = rewind_preview(&blocked, &header, "/w");
        assert_eq!(value["canRewind"], false);
        assert!(value["error"].as_str().unwrap().ends_with("a.txt"));
        assert_eq!(rewind_preview(&json!({"canApply":false}), &header, "/w")["error"], NO_FILE_CHANGES);
    }

    #[test]
    fn parse_line_tells_responses_requests_and_notifications_apart() {
        assert_eq!(
            parse_line(r#"{"id":"vlx-1","result":{"ok":true}}"#),
            Some(Incoming::Response { id: "vlx-1".into(), result: json!({"ok":true}), error: None })
        );
        assert_eq!(
            parse_line(r#"{"id":7,"error":{"code":-32601,"message":"nope"}}"#),
            Some(Incoming::Response { id: "7".into(), result: Value::Null, error: Some("nope".into()) })
        );
        assert!(matches!(
            parse_line(r#"{"id":"server-3","method":"session/requestRuntimePreferences","params":{}}"#),
            Some(Incoming::Request { method, .. }) if method == "session/requestRuntimePreferences"
        ));
        assert!(matches!(
            parse_line(r#"{"method":"v4/conversation/frame","params":{"kind":"complete"}}"#),
            Some(Incoming::Notification { method, .. }) if method == "v4/conversation/frame"
        ));
        assert_eq!(parse_line("not json"), None);
    }

    #[test]
    fn modes_and_model_selection_round_trip() {
        let modes: Vec<&str> = ["plan", "default", "acceptEdits", "bypassPermissions"].into_iter().map(native_mode).collect();
        assert_eq!(modes, ["plan", "build", "edit", "yolo"]);
        assert_eq!(
            model_selection("zai/glm-5.2", Some("high")),
            Some(json!({"providerId":"zai","modelId":"glm-5.2","options":{"reasoningLevel":"high"}}))
        );
        assert_eq!(model_selection("openrouter/z-ai/glm", None).unwrap()["modelId"], "z-ai/glm");
        assert_eq!(model_selection("no-provider", None), None);
    }

    #[test]
    fn catalogue_uses_provider_qualified_ids_and_reasoning_levels() {
        // Shape captured from `session/create` on ZCode 0.17.0.
        let available = vec![
            json!({"ref":{"providerId":"fake","modelId":"fake-model"},"label":"fake-model","providerLabel":"Fake API",
                "contextWindow":1000000,"reasoning":{"levels":[{"value":"disabled","label":"disabled"},{"value":"enabled","label":"enabled"}],"defaultLevel":"enabled"}}),
            json!({"ref":{"providerId":"fake","modelId":"off"},"label":"off","disabledReason":"no key"}),
        ];
        let models = to_chat_models(&available);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["id"], "fake/fake-model");
        assert_eq!(models[0]["effortLevels"], json!(["disabled", "enabled"]));
        assert_eq!(default_effort(&models, "fake/fake-model").as_deref(), Some("enabled"));
    }

    #[test]
    fn rows_convert_to_view_rows() {
        let tool = json!({"rowId":8,"kind":"toolCall","visibility":"visible","toolCallId":"call_1","toolName":"Bash",
            "status":"success","inputText":"{\"command\":\"touch made.txt\"}","input":{"command":"touch made.txt"},
            "output":{"text":"(Bash completed with no output)"}});
        match convert_row(&tool, row_id(8)) {
            Some(ChatRow::Tool { id, name, input, output, status, is_error, .. }) => {
                assert_eq!((id.as_str(), name.as_str(), status, is_error), ("z-8", "Bash", "completed", false));
                assert_eq!(input["command"], "touch made.txt");
                assert_eq!(output.as_deref(), Some("(Bash completed with no output)"));
            }
            other => panic!("unexpected {other:?}"),
        }
        let streaming = json!({"rowId":2,"kind":"toolCall","toolName":"Agent","status":"inputStreaming","inputText":"{\"desc"});
        match convert_row(&streaming, row_id(2)) {
            Some(ChatRow::Tool { name, input, status, .. }) => {
                assert_eq!((name.as_str(), status), ("Task", "running"));
                assert_eq!(input, json!({}));
            }
            other => panic!("unexpected {other:?}"),
        }
        let text = json!({"rowId":4,"kind":"assistantText","text":"Hi","state":"streaming","model":"glm"});
        assert!(matches!(convert_row(&text, row_id(4)), Some(ChatRow::Assistant { streaming: true, .. })));
        let header = json!({"rowId":1,"kind":"turnHeader","state":"running"});
        assert!(convert_row(&header, row_id(1)).is_none());
        let hidden = json!({"rowId":3,"kind":"userInput","text":"x","visibility":"hidden"});
        assert!(convert_row(&hidden, row_id(3)).is_none());
        let compact = json!({"rowId":9,"kind":"timelineMarker","marker":{"type":"compact","origin":"manual","status":"running"}});
        assert!(matches!(convert_row(&compact, row_id(9)), Some(ChatRow::Compaction { status: "loading", .. })));
    }

    #[test]
    fn deltas_append_to_plain_and_nested_fields() {
        let mut row = json!({"text":"Hel"});
        apply_delta(&mut row, "text", "lo");
        apply_delta(&mut row, "output.text", "out");
        apply_delta(&mut row, "output.text", "put");
        assert_eq!(row, json!({"text":"Hello","output":{"text":"output"}}));
    }

    #[test]
    fn fragments_reassemble_into_one_frame() {
        let inner = serde_json::to_vec(&json!({"frame":{"toSeq":3,"payload":{"kind":"deltas","deltas":[]}}})).unwrap();
        let (a, b) = inner.split_at(inner.len() / 2);
        let enc = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
        let mut store = HashMap::new();
        let first = json!({"kind":"fragment","logicalFrameId":"lf-1","fragmentIndex":1,"fragmentCount":2,"dataBase64":enc(b)});
        assert!(decode_frame(&first, &mut store).is_none());
        let second = json!({"kind":"fragment","logicalFrameId":"lf-1","fragmentIndex":0,"fragmentCount":2,"dataBase64":enc(a)});
        assert_eq!(decode_frame(&second, &mut store).unwrap()["toSeq"], 3);
        assert!(store.is_empty());
        let complete = json!({"kind":"complete","frame":{"toSeq":4}});
        assert_eq!(decode_frame(&complete, &mut store).unwrap()["toSeq"], 4);
    }

    #[test]
    fn permission_cards_and_answers() {
        // Pending interaction captured from ZCode 0.17.0 while a Bash call waited for approval.
        let pending = json!({"interactionId":"perm_1","kind":"permission","payload":{"kind":"permission","toolCallId":"call_1",
            "toolName":"Bash","summary":"High risk tools require explicit approval","detail":{"command":"touch made.txt"},
            "options":[{"optionId":"allowOnce","kind":"allowOnce"},
                {"optionId":"allowAlways","kind":"allowAlways","response":{"permissionUpdates":[{"rules":[{"ruleContent":"touch made.txt","toolName":"Bash"}]}]}},
                {"optionId":"deny","kind":"deny"}]}});
        let card = interaction_payload(&pending).unwrap();
        assert_eq!(card["tool_name"], "Bash");
        assert_eq!(card["input"]["command"], "touch made.txt");
        assert_eq!(card["permission_suggestions"][0]["type"], "zcodeAlways");
        assert_eq!(card["permission_suggestions"][0]["subject"], "touch made.txt");
        assert_eq!(interaction_answer(&card, true, None, None, None), json!({"optionId":"allowOnce"}));
        let adopted = json!([card["permission_suggestions"][0].clone()]);
        assert_eq!(interaction_answer(&card, true, None, None, Some(&adopted)), json!({"optionId":"allowAlways"}));
        assert_eq!(
            interaction_answer(&card, false, None, Some("use git stash"), None),
            json!({"optionId":"deny","freeText":"use git stash"})
        );

        let question = json!({"interactionId":"ask_1","payload":{"kind":"userInput","toolName":"AskUserQuestion","freeText":true,
            "questions":[{"question":"Which port?","header":"Port","options":[{"value":"a","label":"18000"}]}]}});
        let card = interaction_payload(&question).unwrap();
        assert_eq!(card["tool_name"], "AskUserQuestion");
        assert_eq!(card["input"]["questions"][0]["options"][0]["label"], "18000");
        let input = json!({"answers":{"Which port?":"18000"}});
        assert_eq!(
            interaction_answer(&card, true, Some(&input), None, None),
            json!({"action":"accept","content":{"answers":{"Which port?":"18000"}}})
        );
        assert_eq!(interaction_answer(&card, false, None, None, None), json!({"action":"decline"}));

        let plan = json!({"interactionId":"plan_1","payload":{"kind":"userInput","toolName":"ExitPlanMode","prompt":"1. Do it"}});
        let card = interaction_payload(&plan).unwrap();
        assert_eq!((card["tool_name"].as_str(), card["input"]["plan"].as_str()), (Some("ExitPlanMode"), Some("1. Do it")));
        assert_eq!(interaction_answer(&card, false, None, Some("smaller steps"), None), json!({"freeText":"smaller steps"}));
    }

    #[test]
    fn slash_commands_leave_compact_to_the_view() {
        let commands = vec![json!({"name":"compact","description":"c"}), json!({"name":"init","description":"Create AGENTS.md","inputHint":"/init [notes]"})];
        let out = slash_commands(&commands);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["name"], "init");
        assert!(attachment_checksum(b"abc").starts_with("sha256:ba7816bf"));
    }
}
