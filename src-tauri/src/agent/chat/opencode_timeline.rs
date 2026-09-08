//! OpenCode messages and parts, turned into timeline rows and export events.
//!
//! OpenCode describes a conversation as messages made of parts: a user message carries text and file
//! parts, an assistant message carries reasoning, text, tool, and bookkeeping parts. The same shapes come
//! from its HTTP API while a session runs and from its SQLite store afterwards, so one converter serves the
//! live engine's replay, the read-only session view, and the Markdown export.
//!
//! Row ids are OpenCode's own part ids. That is what lets a replayed row and the live event that later
//! updates the same part land on one row instead of two.

use serde_json::Value;

use super::engine::{ChatRow, SubagentInfo};
use crate::agent::export::Event;
use crate::agent::opencode_store::OpencodeMessage;

/// Root timelines keep this many rows; the rest is summarized by one notice.
pub const REPLAY_LIMIT: usize = 400;
/// A subagent's card keeps this many rows.
pub const CHILD_REPLAY_LIMIT: usize = 200;
/// How deep nested subagents are followed before their work is left out.
const MAX_CHILD_DEPTH: usize = 4;

/// The name the tool cards know a tool by. OpenCode names its tools in lowercase; the cards were written
/// for Claude's names and read the same fields, so the names are mapped rather than the cards duplicated.
pub fn tool_name(raw: &str) -> String {
    match raw {
        "bash" => "Bash",
        "edit" => "Edit",
        "multiedit" => "MultiEdit",
        "write" => "Write",
        "read" => "Read",
        "glob" => "Glob",
        "grep" => "Grep",
        "list" => "List",
        "webfetch" => "WebFetch",
        "websearch" => "WebSearch",
        "task" => "Task",
        "todowrite" => "TodoWrite",
        "todoread" => "TodoRead",
        "skill" => "Skill",
        "question" => "AskUserQuestion",
        "patch" => "Patch",
        "apply_patch" => "Patch",
        other => other,
    }
    .to_string()
}

/// Tool arguments with the keys the cards read. OpenCode writes `filePath`, `oldString`, `newString`;
/// the cards read `file_path`, `old_string`, `new_string`. Everything else passes through unchanged.
pub fn normalize_input(input: &Value) -> Value {
    let Some(map) = input.as_object() else { return input.clone() };
    let mut out = serde_json::Map::with_capacity(map.len());
    for (key, value) in map {
        let renamed = match key.as_str() {
            "filePath" => "file_path",
            "oldString" => "old_string",
            "newString" => "new_string",
            "replaceAll" => "replace_all",
            "notebookPath" => "notebook_path",
            other => other,
        };
        out.insert(renamed.to_string(), value.clone());
    }
    Value::Object(out)
}

/// The session a `task` call runs its subagent in, once the tool has started.
pub fn child_session_of(part: &Value) -> Option<String> {
    if part.get("tool").and_then(Value::as_str) != Some("task") {
        return None;
    }
    part.pointer("/state/metadata/sessionId")
        .or_else(|| part.pointer("/state/metadata/sessionID"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// A timeline row for one tool part, without subagent children; the caller attaches those.
pub fn tool_row(part: &Value) -> Option<ChatRow> {
    let id = part.get("id").and_then(Value::as_str)?;
    let raw_tool = part.get("tool").and_then(Value::as_str).unwrap_or("tool");
    let state = part.get("state").cloned().unwrap_or(Value::Null);
    let status = match state.get("status").and_then(Value::as_str) {
        Some("completed") => "completed",
        Some("error") => "failed",
        _ => "running",
    };
    let input = normalize_input(state.get("input").unwrap_or(&Value::Null));
    let output = match status {
        "completed" => state.get("output").and_then(Value::as_str).map(str::to_string),
        "failed" => state.get("error").and_then(Value::as_str).map(str::to_string),
        _ => None,
    };
    let subagent = (raw_tool == "task").then(|| SubagentInfo {
        title: input.get("subagent_type").and_then(Value::as_str).map(str::to_string),
        description: input.get("description").and_then(Value::as_str).map(str::to_string),
        model: state
            .pointer("/metadata/model/modelID")
            .and_then(Value::as_str)
            .map(str::to_string),
        total_tokens: None,
        tool_uses: None,
        duration_ms: None,
    });
    Some(ChatRow::Tool {
        id: id.to_string(),
        name: tool_name(raw_tool),
        input,
        output,
        is_error: status == "failed",
        status,
        subagent,
        children: Vec::new(),
    })
}

/// What a user message said, as one text: its text parts, with a placeholder for each attachment.
pub fn user_text(parts: &[Value]) -> String {
    let mut out: Vec<String> = Vec::new();
    for part in parts {
        match part.get("type").and_then(Value::as_str) {
            Some("text") => {
                // Injected context (the summary after a compaction, a command's expanded template) is
                // marked synthetic; the person did not type it, so it is not their message.
                if part.get("synthetic").and_then(Value::as_bool).unwrap_or(false)
                    || part.get("ignored").and_then(Value::as_bool).unwrap_or(false)
                {
                    continue;
                }
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        out.push(text.to_string());
                    }
                }
            }
            Some("file") => {
                let mime = part.get("mime").and_then(Value::as_str).unwrap_or("");
                if mime.starts_with("image/") {
                    out.push("[image]".to_string());
                } else {
                    let name = part.get("filename").and_then(Value::as_str).unwrap_or(mime);
                    out.push(format!("[file: {name}]"));
                }
            }
            _ => {}
        }
    }
    out.join("\n")
}

/// Whether an assistant message is the summary OpenCode wrote while compacting, rather than an answer.
pub fn is_summary(info: &Value) -> bool {
    info.get("summary").and_then(Value::as_bool) == Some(true)
        || info.get("agent").and_then(Value::as_str) == Some("compaction")
        || info.get("mode").and_then(Value::as_str) == Some("compaction")
}

/// The error an assistant message ended with, or `None` for an abort — which is the interrupt working.
pub fn error_message(error: &Value) -> Option<String> {
    if error.is_null() {
        return None;
    }
    let name = error.get("name").and_then(Value::as_str).unwrap_or("");
    if name == "MessageAbortedError" {
        return None;
    }
    let message = error
        .pointer("/data/message")
        .and_then(Value::as_str)
        .filter(|m| !m.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| error.to_string());
    Some(if name.is_empty() { message } else { format!("{name}: {message}") })
}

/// Shell turns store a synthetic marker instead of the command the person typed. Recover the command
/// only from that marker's own assistant child, without exposing other injected context as user text.
fn message_user_text(message: &OpencodeMessage, messages: &[OpencodeMessage]) -> String {
    let text = user_text(&message.parts);
    if !text.trim().is_empty() {
        return text;
    }
    let shell = message.parts.iter().any(|part| {
        part.get("type").and_then(Value::as_str) == Some("text")
            && part.get("synthetic").and_then(Value::as_bool) == Some(true)
            && part.get("text").and_then(Value::as_str) == Some("The following tool was executed by the user")
    });
    if !shell { return text; }
    let Some(id) = message.info.get("id").and_then(Value::as_str) else { return text };
    messages.iter()
        .filter(|child| child.info.get("role").and_then(Value::as_str) == Some("assistant")
            && child.info.get("parentID").and_then(Value::as_str) == Some(id))
        .flat_map(|child| &child.parts)
        .find_map(|part| {
            if part.get("type").and_then(Value::as_str) != Some("tool")
                || part.get("tool").and_then(Value::as_str) != Some("bash") { return None; }
            part.pointer("/state/input/command").and_then(Value::as_str).map(|command| format!("!{command}"))
        })
        .unwrap_or(text)
}

pub fn user_row_id(message_id: &str) -> String {
    format!("u:{message_id}")
}

pub fn error_row_id(message_id: &str) -> String {
    format!("e:{message_id}")
}

pub fn compaction_row_id(part_id: &str) -> String {
    format!("c:{part_id}")
}

fn ms(value: Option<&Value>) -> Option<i64> {
    value.and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
}

/// Sum of a message's reported tokens, for the subagent card's usage line.
fn total_tokens(info: &Value) -> Option<u64> {
    let tokens = info.get("tokens")?;
    if let Some(total) = tokens.get("total").and_then(Value::as_u64).filter(|t| *t > 0) {
        return Some(total);
    }
    let read = |key: &str| tokens.get(key).and_then(Value::as_u64).unwrap_or(0);
    let cache = tokens.get("cache");
    let cached = cache
        .map(|c| ["read", "write"].iter().map(|k| c.get(k).and_then(Value::as_u64).unwrap_or(0)).sum::<u64>())
        .unwrap_or(0);
    let sum = read("input") + read("output") + read("reasoning") + cached;
    (sum > 0).then_some(sum)
}

/// Read one session's messages as timeline rows, following `task` calls into their child sessions.
///
/// `children` answers a child session id with that session's messages, or `None` when it cannot be
/// read; a card whose child is gone keeps its title and result without pretending to know the steps.
pub fn rows(
    messages: &[OpencodeMessage],
    children: &dyn Fn(&str) -> Option<Vec<OpencodeMessage>>,
) -> Vec<ChatRow> {
    rows_at_depth(messages, children, 0, REPLAY_LIMIT)
}

/// Use the native message's model identifier as its display name.
pub fn message_model(info: &Value) -> Option<String> {
    info.get("modelID").and_then(Value::as_str).map(str::trim)
        .filter(|model| !model.is_empty()).map(str::to_string)
}

fn rows_at_depth(
    messages: &[OpencodeMessage],
    children: &dyn Fn(&str) -> Option<Vec<OpencodeMessage>>,
    depth: usize,
    limit: usize,
) -> Vec<ChatRow> {
    let mut out: Vec<ChatRow> = Vec::new();
    for message in messages {
        let info = &message.info;
        let Some(message_id) = info.get("id").and_then(Value::as_str) else { continue };
        match info.get("role").and_then(Value::as_str) {
            Some("user") => {
                let text = message_user_text(message, messages);
                if text.trim().is_empty() {
                    continue;
                }
                out.push(ChatRow::User {
                    id: user_row_id(message_id),
                    text,
                    images: Vec::new(),
                    at: ms(info.pointer("/time/created")),
                });
            }
            Some("assistant") => {
                let summary = is_summary(info);
                for part in &message.parts {
                    let Some(part_id) = part.get("id").and_then(Value::as_str) else { continue };
                    match part.get("type").and_then(Value::as_str) {
                        Some("text") if !summary => {
                            let text = part.get("text").and_then(Value::as_str).unwrap_or("");
                            if text.trim().is_empty() {
                                continue;
                            }
                            out.push(ChatRow::Assistant {
                                id: part_id.to_string(),
                                text: text.to_string(),
                                streaming: false,
                                model: message_model(info),
                                at: ms(part.pointer("/time/end"))
                                    .or_else(|| ms(info.pointer("/time/completed"))),
                                duration_ms: None,
                            });
                        }
                        Some("reasoning") => {
                            let text = part.get("text").and_then(Value::as_str).unwrap_or("");
                            if text.trim().is_empty() {
                                continue;
                            }
                            out.push(ChatRow::Reasoning {
                                id: part_id.to_string(),
                                text: text.to_string(),
                                streaming: false,
                            });
                        }
                        Some("tool") => {
                            let Some(mut row) = tool_row(part) else { continue };
                            if depth < MAX_CHILD_DEPTH {
                                if let Some(child_id) = child_session_of(part) {
                                    attach_child(&mut row, &child_id, children, depth);
                                }
                            }
                            out.push(row);
                        }
                        Some("compaction") => {
                            out.push(ChatRow::Compaction {
                                id: compaction_row_id(part_id),
                                status: "completed",
                                trigger: Some(
                                    if part.get("auto").and_then(Value::as_bool).unwrap_or(false) {
                                        "auto"
                                    } else {
                                        "manual"
                                    }
                                    .to_string(),
                                ),
                                pre_tokens: None,
                            });
                        }
                        _ => {}
                    }
                }
                if let Some(message) = info.get("error").and_then(error_message) {
                    out.push(ChatRow::Error { id: error_row_id(message_id), message });
                }
            }
            _ => {}
        }
    }
    let omitted = out.len().saturating_sub(limit);
    if omitted > 0 {
        out.drain(..omitted);
        out.insert(
            0,
            ChatRow::Notice {
                id: if depth == 0 { "h-omitted".to_string() } else { "h-omitted-child".to_string() },
                message: format!("{omitted} earlier messages are not shown. The agent still has them."),
            },
        );
    }
    out
}

fn attach_child(
    row: &mut ChatRow,
    child_id: &str,
    children: &dyn Fn(&str) -> Option<Vec<OpencodeMessage>>,
    depth: usize,
) {
    let Some(messages) = children(child_id) else { return };
    let tokens = messages
        .iter()
        .rev()
        .find(|m| m.info.get("role").and_then(Value::as_str) == Some("assistant"))
        .and_then(|m| total_tokens(&m.info));
    let model = messages
        .iter()
        .rev()
        .find_map(|m| m.info.get("modelID").and_then(Value::as_str).map(str::to_string));
    let child_rows = rows_at_depth(&messages, children, depth + 1, CHILD_REPLAY_LIMIT);
    if let ChatRow::Tool { children: slot, subagent, .. } = row {
        *slot = child_rows;
        if let Some(facts) = subagent {
            if facts.total_tokens.is_none() {
                facts.total_tokens = tokens;
            }
            if facts.model.is_none() {
                facts.model = model;
            }
        }
    }
}

/// Milliseconds since the epoch as the ISO string the transcript readers pass through.
pub fn iso_from_ms(ms: i64) -> Option<String> {
    let seconds = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let dt = time::OffsetDateTime::from_unix_timestamp(seconds).ok()?;
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        millis
    ))
}

fn iso(value: Option<&Value>) -> Option<String> {
    ms(value).and_then(iso_from_ms)
}

/// The same conversation as the flat events the Markdown export and the transcript readers consume.
pub fn events(messages: &[OpencodeMessage]) -> Vec<Event> {
    let mut out: Vec<Event> = Vec::new();
    for message in messages {
        let info = &message.info;
        match info.get("role").and_then(Value::as_str) {
            Some("user") => {
                let text = message_user_text(message, messages);
                if text.trim().is_empty() {
                    continue;
                }
                out.push(Event::User { text, ts: iso(info.pointer("/time/created")) });
            }
            Some("assistant") => {
                let summary = is_summary(info);
                for part in &message.parts {
                    match part.get("type").and_then(Value::as_str) {
                        Some("text") if !summary => {
                            let text = part.get("text").and_then(Value::as_str).unwrap_or("");
                            if text.trim().is_empty() {
                                continue;
                            }
                            out.push(Event::AssistantText {
                                text: text.to_string(),
                                ts: iso(part.pointer("/time/start"))
                                    .or_else(|| iso(info.pointer("/time/created"))),
                            });
                        }
                        Some("reasoning") => {
                            let text = part.get("text").and_then(Value::as_str).unwrap_or("");
                            if text.trim().is_empty() {
                                continue;
                            }
                            out.push(Event::Thinking {
                                text: text.to_string(),
                                ts: iso(part.pointer("/time/start")),
                            });
                        }
                        Some("tool") => {
                            let call_id = part
                                .get("callID")
                                .and_then(Value::as_str)
                                .or_else(|| part.get("id").and_then(Value::as_str))
                                .map(str::to_string);
                            let name = tool_name(part.get("tool").and_then(Value::as_str).unwrap_or("tool"));
                            let state = part.get("state").cloned().unwrap_or(Value::Null);
                            out.push(Event::ToolUse {
                                id: call_id.clone(),
                                name: name.clone(),
                                input: normalize_input(state.get("input").unwrap_or(&Value::Null)),
                                ts: iso(state.pointer("/time/start")),
                            });
                            match state.get("status").and_then(Value::as_str) {
                                Some("completed") => out.push(Event::ToolResult {
                                    id: call_id,
                                    name: Some(name),
                                    text: state.get("output").and_then(Value::as_str).unwrap_or("").to_string(),
                                    is_error: false,
                                }),
                                Some("error") => out.push(Event::ToolResult {
                                    id: call_id,
                                    name: Some(name),
                                    text: state.get("error").and_then(Value::as_str).unwrap_or("").to_string(),
                                    is_error: true,
                                }),
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Provider-native identities of the genuine user turns, for rewind.
pub fn user_turns(messages: &[OpencodeMessage]) -> Vec<super::history::RewindTarget> {
    messages
        .iter()
        .filter(|m| m.info.get("role").and_then(Value::as_str) == Some("user"))
        .filter_map(|m| {
            let id = m.info.get("id").and_then(Value::as_str)?;
            let text = message_user_text(m, messages);
            if text.trim().is_empty() {
                return None;
            }
            Some(super::history::RewindTarget {
                message_id: id.to_string(),
                turn_id: None,
                text,
            })
        })
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn message(id: &str, role: &str, parts: Vec<Value>) -> OpencodeMessage {
        let mut parts = parts;
        for (i, part) in parts.iter_mut().enumerate() {
            if let Value::Object(map) = part {
                map.entry("id").or_insert(json!(format!("prt_{id}_{i}")));
                map.insert("messageID".into(), json!(id));
            }
        }
        OpencodeMessage {
            info: json!({"id":id,"role":role,"time":{"created":1_700_000_000_000i64}}),
            parts,
        }
    }

    #[test]
    fn tool_names_and_fields_match_the_cards() {
        let part = json!({
            "id":"prt_1","type":"tool","tool":"edit","callID":"call_1",
            "state":{"status":"completed","input":{"filePath":"/a.rs","oldString":"x","newString":"y"},"output":"ok","time":{"start":1,"end":2}}
        });
        let ChatRow::Tool { name, input, output, status, .. } = tool_row(&part).unwrap() else {
            panic!("a tool part becomes a tool row");
        };
        assert_eq!(name, "Edit");
        assert_eq!(input["file_path"], "/a.rs");
        assert_eq!(input["old_string"], "x");
        assert_eq!(output.as_deref(), Some("ok"));
        assert_eq!(status, "completed");
    }

    #[test]
    fn a_failed_tool_reports_its_error_as_output() {
        let part = json!({"id":"prt_1","type":"tool","tool":"bash","callID":"c",
            "state":{"status":"error","input":{"command":"false"},"error":"exit 1","time":{"start":1,"end":2}}});
        let ChatRow::Tool { output, is_error, status, .. } = tool_row(&part).unwrap() else { panic!() };
        assert_eq!(output.as_deref(), Some("exit 1"));
        assert!(is_error);
        assert_eq!(status, "failed");
    }

    #[test]
    fn replies_keep_their_own_models_in_history() {
        let mut first = message("a", "assistant", vec![json!({"type":"text","text":"first"})]);
        first.info["modelID"] = json!("deepseek-v4-pro");
        let mut second = message("b", "assistant", vec![json!({"type":"text","text":"second"})]);
        second.info["modelID"] = json!("glm-5.3");
        let mut legacy = message("c", "assistant", vec![json!({"type":"text","text":"legacy"})]);
        legacy.info["modelID"] = json!("  ");
        let rows = rows(&[first, second, legacy], &|_| None);
        let models: Vec<_> = rows.iter().map(|row| match row {
            ChatRow::Assistant { model, .. } => model.as_deref(),
            _ => panic!("expected assistant"),
        }).collect();
        assert_eq!(models, vec![Some("deepseek-v4-pro"), Some("glm-5.3"), None]);
        assert_eq!(message_model(&json!({})), None);
        assert_eq!(serde_json::to_value(&rows[0]).unwrap()["model"], "deepseek-v4-pro");
        assert!(serde_json::to_value(&rows[2]).unwrap().get("model").is_none());
    }

    #[test]
    fn a_conversation_becomes_addressable_rows() {
        let messages = vec![
            message("msg_u", "user", vec![json!({"type":"text","text":"hello"})]),
            message(
                "msg_a",
                "assistant",
                vec![
                    json!({"type":"step-start"}),
                    json!({"type":"reasoning","text":"thinking"}),
                    json!({"type":"tool","tool":"read","callID":"c1","state":{"status":"completed","input":{"filePath":"/f"},"output":"data","time":{"start":1,"end":2}}}),
                    json!({"type":"text","text":"done","time":{"start":1,"end":5}}),
                ],
            ),
        ];
        let rows = rows(&messages, &|_| None);
        let ids: Vec<&str> = rows.iter().map(|r| r.id()).collect();
        assert_eq!(ids, vec!["u:msg_u", "prt_msg_a_1", "prt_msg_a_2", "prt_msg_a_3"]);
        assert!(matches!(&rows[0], ChatRow::User { text, .. } if text == "hello"));
        assert!(matches!(&rows[3], ChatRow::Assistant { at: Some(5), .. }));
    }

    #[test]
    fn a_compaction_summary_is_a_marker_not_an_answer() {
        let mut summary = message("msg_s", "assistant", vec![json!({"type":"text","text":"summary text"})]);
        summary.info["summary"] = json!(true);
        let rows = rows(&[summary], &|_| None);
        assert!(rows.is_empty(), "the summary's prose is not shown");
        assert!(events(&[message("msg_s", "assistant", vec![json!({"type":"compaction","auto":true})])]).is_empty());
    }

    #[test]
    fn synthetic_user_text_is_not_the_persons_message() {
        let messages = vec![message(
            "msg_u",
            "user",
            vec![
                json!({"type":"text","text":"injected","synthetic":true}),
                json!({"type":"file","mime":"image/png","url":"data:..."}),
                json!({"type":"text","text":"real"}),
            ],
        )];
        assert_eq!(user_text(&messages[0].parts), "[image]\nreal");
    }

    #[test]
    fn a_shell_command_survives_replay_export_and_rewind() {
        let user = message("msg_shell", "user", vec![json!({
            "type":"text", "text":"The following tool was executed by the user", "synthetic":true
        })]);
        let mut assistant = message("msg_result", "assistant", vec![json!({
            "type":"tool", "tool":"bash", "state":{"status":"completed", "input":{"command":"pwd"}, "output":"/repo"}
        })]);
        assistant.info["parentID"] = json!("msg_shell");
        let messages = vec![user, assistant];
        assert!(matches!(&rows(&messages, &|_| None)[0], ChatRow::User { id, text, .. }
            if id == "u:msg_shell" && text == "!pwd"));
        assert!(matches!(&events(&messages)[0], Event::User { text, .. } if text == "!pwd"));
        let targets = user_turns(&messages);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].message_id, "msg_shell");
        assert_eq!(targets[0].text, "!pwd");

        let mut unrelated = messages.clone();
        unrelated[1].info["parentID"] = json!("another_user");
        assert!(user_turns(&unrelated).is_empty(), "an unrelated tool is not this person's command");
    }

    #[test]
    fn a_task_call_folds_its_child_session_into_the_card() {
        let root = vec![message(
            "msg_a",
            "assistant",
            vec![json!({"type":"tool","tool":"task","callID":"c1",
                "state":{"status":"completed","input":{"description":"look","prompt":"p","subagent_type":"explore"},
                         "output":"found","metadata":{"sessionId":"ses_child","model":{"modelID":"m1"}},"time":{"start":1,"end":2}}})],
        )];
        let child = |id: &str| {
            (id == "ses_child").then(|| {
                vec![message(
                    "msg_c",
                    "assistant",
                    vec![json!({"type":"text","text":"child answer","time":{"start":1,"end":2}})],
                )]
            })
        };
        let rows = rows(&root, &child);
        let ChatRow::Tool { name, children, subagent, .. } = &rows[0] else { panic!() };
        assert_eq!(name, "Task");
        assert_eq!(children.len(), 1);
        assert_eq!(subagent.as_ref().unwrap().title.as_deref(), Some("explore"));
        assert_eq!(subagent.as_ref().unwrap().model.as_deref(), Some("m1"));
    }

    #[test]
    fn an_abort_is_not_an_error_but_a_failure_is() {
        assert_eq!(error_message(&json!({"name":"MessageAbortedError","data":{}})), None);
        assert_eq!(
            error_message(&json!({"name":"APIError","data":{"message":"quota"}})).as_deref(),
            Some("APIError: quota")
        );
    }

    #[test]
    fn export_events_pair_calls_with_results() {
        let messages = vec![message(
            "msg_a",
            "assistant",
            vec![json!({"type":"tool","tool":"bash","callID":"c1","state":{"status":"completed","input":{"command":"ls"},"output":"a\nb","time":{"start":1,"end":2}}})],
        )];
        let events = events(&messages);
        assert!(matches!(&events[0], Event::ToolUse { id: Some(id), name, .. } if id == "c1" && name == "Bash"));
        assert!(matches!(&events[1], Event::ToolResult { id: Some(id), text, .. } if id == "c1" && text == "a\nb"));
    }

    #[test]
    fn timestamps_render_as_iso() {
        assert_eq!(iso_from_ms(0).as_deref(), Some("1970-01-01T00:00:00.000Z"));
        assert_eq!(iso_from_ms(1_700_000_000_123).as_deref(), Some("2023-11-14T22:13:20.123Z"));
    }
}
