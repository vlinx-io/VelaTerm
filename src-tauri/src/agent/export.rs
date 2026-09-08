//! Session-context export that renders a complete Claude transcript or Codex rollout as Markdown.
//!
//! Unlike the reading view in `transcript.rs`, which keeps only user and assistant prose, exports retain
//! **all main-conversation context**: user messages, assistant replies and reasoning, and complete tool
//! inputs and results. Only content outside the main conversation is excluded:
//! - subagent internals (`isSidechain`), whose conclusions already appear as tool results;
//! - system bookkeeping noise (`isMeta`, injected `<system-reminder>` blocks, and Codex
//!   `<environment_context>`/`<user_instructions>`).
//!
//! Source formats match `transcript.rs`—Claude JSONL and Codex rollouts—but parsing is more complete:
//! - Claude: assistant `thinking`, `tool_use.input`, and user `tool_result` blocks, whose content may be
//!   a string or text/image array and whose errors are marked by `is_error`.
//! - Codex: JSON-string `arguments` from function/custom tool calls, their output paired to tool names by
//!   `call_id`, and reasoning summaries.

use std::collections::HashMap;

use serde_json::Value;

use crate::models::SessionKind;

use super::resume;
use super::transcript::is_injected_context;

/// One parsed context event, the smallest Markdown rendering unit, ordered as it appears in the file.
///
/// The session view (`agent::chat`) reads the same events, so tool calls keep their raw id and raw input
/// value: Markdown export pretty-prints them, while the chat cards need the individual fields (a Bash
/// `command`, an Edit `old_string`/`new_string`) and the id that pairs a call with its result.
pub(crate) enum Event {
    User {
        text: String,
        ts: Option<String>,
    },
    AssistantText {
        text: String,
        ts: Option<String>,
    },
    Thinking {
        text: String,
        ts: Option<String>,
    },
    ToolUse {
        /// tool_use_id (Claude) or call_id (Codex); None when the recording omits it.
        id: Option<String>,
        name: String,
        /// Raw input value. Codex stores arguments as a JSON string; unparseable text is kept as a string.
        input: Value,
        ts: Option<String>,
    },
    ToolResult {
        /// The id of the call this result answers, used to pair the two into one card.
        id: Option<String>,
        /// Tool name resolved by tool_use_id or call_id, or None when unavailable.
        name: Option<String>,
        text: String,
        is_error: bool,
    },
    /// Output of a slash command the CLI answered itself, such as `/effort` or `/context`.
    ///
    /// Nobody said it and no tool produced it, so it is neither speech nor a tool result: it is the
    /// answer to something typed into the box, and the view draws it as its own kind of row.
    Command {
        text: String,
        ts: Option<String>,
    },
}

/// Locate a session recording by type and export its complete context as Markdown. Return Err for a
/// deleted, remote, or unsupported recording. `exported_at` is frontend-formatted local time that the
/// backend writes unchanged because it has no time-zone library.
pub fn export_markdown(
    kind: SessionKind,
    agent_session_id: &str,
    session_name: &str,
    exported_at: Option<&str>,
) -> Result<String, String> {
    let (path, events) = match kind {
        SessionKind::Claude => {
            let path = resume::find_claude_transcript(agent_session_id)
                .ok_or("Claude transcript file not found")?;
            (path, claude_events(&resume::read_claude_transcript(agent_session_id)?))
        }
        SessionKind::Codex => {
            let (path, content) = resume::read_codex_rollout_chain(agent_session_id)?;
            (path, codex_events(&content))
        }
        SessionKind::Opencode => {
            let path = crate::agent::opencode_store::db_path()
                .ok_or("OpenCode's data directory could not be located")?;
            let messages = crate::agent::opencode_store::messages(agent_session_id)?;
            (path, crate::agent::chat::opencode_timeline::events(&messages))
        }
        SessionKind::Terminal => {
            return Err("Terminal sessions have no agent transcript".to_string())
        }
        // Copilot, Cursor, Cline, and similar agents do not expose locally parseable flat files.
        // Use a fallback rather than exhaustive matching so new session types compile as unsupported.
        other => {
            return Err(format!(
                "Context export is not supported for {} sessions yet",
                other.as_str()
            ))
        }
    };

    Ok(render(
        &events,
        session_name,
        kind.as_str(),
        agent_session_id,
        &path.display().to_string(),
        exported_at,
    ))
}

// ─────────────────────────── Markdown rendering ───────────────────────────

/// Current user or assistant section, used to decide when to insert role headings.
#[derive(PartialEq, Clone, Copy)]
enum Sec {
    None,
    User,
    Assistant,
}

fn render(
    events: &[Event],
    session_name: &str,
    kind: &str,
    agent_session_id: &str,
    source_path: &str,
    exported_at: Option<&str>,
) -> String {
    let mut blocks: Vec<String> = Vec::new();
    let mut sec = Sec::None;
    let (mut users, mut turns, mut tool_calls) = (0u32, 0u32, 0u32);

    for ev in events {
        // The export is the conversation. What a slash command printed back into the session belongs to
        // the moment it was typed, not to the record of what was said.
        if matches!(ev, Event::Command { .. }) {
            continue;
        }
        match ev {
            Event::User { text, ts } => {
                if sec != Sec::User {
                    blocks.push(section_header("👤 User", ts.as_deref()));
                    sec = Sec::User;
                }
                users += 1;
                blocks.push(text.clone());
            }
            _ => {
                if sec != Sec::Assistant {
                    let ts = match ev {
                        Event::AssistantText { ts, .. }
                        | Event::Thinking { ts, .. }
                        | Event::ToolUse { ts, .. } => ts.as_deref(),
                        _ => None,
                    };
                    blocks.push(section_header("🤖 Assistant", ts));
                    sec = Sec::Assistant;
                    turns += 1;
                }
                match ev {
                    Event::AssistantText { text, .. } => blocks.push(text.clone()),
                    Event::Thinking { text, .. } => {
                        blocks.push(format!("**💭 Thinking**\n\n{}", blockquote(text)));
                    }
                    Event::ToolUse { name, input, .. } => {
                        tool_calls += 1;
                        blocks.push(format!(
                            "**🔧 Tool call: {name}**\n\n{}",
                            fenced("json", &pretty_json(input))
                        ));
                    }
                    Event::ToolResult {
                        name,
                        text,
                        is_error,
                        ..
                    } => {
                        let mut title = "**📥 Tool result".to_string();
                        if let Some(n) = name {
                            title.push_str(&format!(" ({n})"));
                        }
                        if *is_error {
                            title.push_str(" · error");
                        }
                        title.push_str("**");
                        blocks.push(format!("{title}\n\n{}", fenced("", text)));
                    }
                    Event::User { .. } | Event::Command { .. } => unreachable!(),
                }
            }
        }
    }

    let mut md = String::new();
    md.push_str(&format!("# Session Context Export: {session_name}\n\n"));
    md.push_str(&format!("- **Session kind**: {kind}\n"));
    md.push_str(&format!("- **Agent Session ID**: `{agent_session_id}`\n"));
    md.push_str(&format!("- **Transcript file**: `{source_path}`\n"));
    if let Some(t) = exported_at {
        md.push_str(&format!("- **Exported at**: {t}\n"));
    }
    md.push_str(&format!(
        "- **Content stats**: {users} user messages · {turns} assistant turns · {tool_calls} tool calls\n\n"
    ));
    md.push_str(
        "> This file contains the full context of the session's main conversation: \
         user messages, assistant replies with thinking, and the input/result of every tool call.\n\
         > Sub-agent internal messages and system-injected reminder blocks are excluded; \
         for raw data, see the transcript file above.\n\n\
         ---\n\n",
    );
    if blocks.is_empty() {
        md.push_str("(Transcript is empty)\n");
    } else {
        md.push_str(&blocks.join("\n\n"));
        md.push('\n');
    }
    md
}

/// Role-section heading with an optional ISO timestamp copied from the recording.
fn section_header(label: &str, ts: Option<&str>) -> String {
    match ts {
        Some(t) if !t.is_empty() => format!("## {label} · {t}"),
        _ => format!("## {label}"),
    }
}

/// Prefix every line with `> ` to distinguish reasoning from normal prose.
fn blockquote(text: &str) -> String {
    text.lines()
        .map(|l| {
            if l.is_empty() {
                ">".to_string()
            } else {
                format!("> {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Choose a code fence longer than any backtick run in embedded content, with a minimum of three.
fn fence_for(text: &str) -> String {
    let mut max_run = 0usize;
    let mut cur = 0usize;
    for c in text.chars() {
        if c == '`' {
            cur += 1;
            max_run = max_run.max(cur);
        } else {
            cur = 0;
        }
    }
    "`".repeat((max_run + 1).max(3))
}

/// Wrap literal tool input or output in a safe fence so data is not rendered as Markdown.
fn fenced(lang: &str, text: &str) -> String {
    let f = fence_for(text);
    let body = text.trim_end_matches('\n');
    if body.is_empty() {
        format!("{f}{lang}\n{f}")
    } else {
        format!("{f}{lang}\n{body}\n{f}")
    }
}

// ─────────────────────────── Claude parsing ───────────────────────────

/// What the CLI records as the assistant's reply to a command it answered itself, such as `/effort`.
const NO_RESPONSE_PLACEHOLDER: &str = "No response requested.";

/// The text a locally answered slash command printed, or None for a row carrying anything else.
///
/// The CLI records the command's own echo (`<command-name>`) alongside its output, and only the output
/// is worth showing: the echo repeats the line the user just typed.
fn local_command_stdout(content: Option<&str>) -> Option<String> {
    const OPEN: &str = "<local-command-stdout>";
    const CLOSE: &str = "</local-command-stdout>";
    let body = content?.trim().strip_prefix(OPEN)?;
    let text = body.strip_suffix(CLOSE).unwrap_or(body).trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The command line a locally answered slash command was invoked with, such as `/model claude-opus-4-6`.
///
/// The recording keeps the echo apart from the output, so without this a reopened session shows the
/// answer to a command with nothing above it saying what was asked.
fn local_command_echo(content: Option<&str>) -> Option<String> {
    let content = content?;
    let name = between(content, "<command-name>", "</command-name>")?;
    let args = between(content, "<command-args>", "</command-args>").unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    Some(if args.is_empty() { name } else { format!("{name} {args}") })
}

/// The text between two markers, trimmed. None when the opening marker is absent.
fn between(text: &str, open: &str, close: &str) -> Option<String> {
    let rest = text.split_once(open)?.1;
    Some(rest.split_once(close).map_or(rest, |(inner, _)| inner).trim().to_string())
}

/// Parse a complete Claude transcript into events, skipping sidechain, meta, and injected blocks.
pub(crate) fn claude_events(content: &str) -> Vec<Event> {
    // Map tool_use IDs to names for subsequent tool_result display.
    let mut tool_names: HashMap<String, String> = HashMap::new();
    let mut out: Vec<Event> = Vec::new();

    for line in content.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("isSidechain")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || v.get("isMeta").and_then(Value::as_bool).unwrap_or(false)
        {
            continue;
        }
        let ts = v
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_string);
        // A slash command the CLI ran itself is recorded as a system row with no message of its own.
        if v.get("type").and_then(Value::as_str) == Some("system")
            && v.get("subtype").and_then(Value::as_str) == Some("local_command")
        {
            let content = v.get("content").and_then(Value::as_str);
            match local_command_stdout(content) {
                Some(text) => out.push(Event::Command { text, ts }),
                // The echo of the command itself is what the person typed, so it reads as their message.
                None => {
                    if let Some(cmd) = local_command_echo(content) {
                        out.push(Event::User { text: cmd, ts });
                    }
                }
            }
            continue;
        }
        let Some(msg) = v.get("message") else {
            continue;
        };
        match v.get("type").and_then(Value::as_str) {
            Some("user") => match msg.get("content") {
                Some(Value::String(s)) => {
                    push_user_text(&mut out, s, ts);
                }
                Some(Value::Array(items)) => {
                    // A row may mix text, image, and tool_result blocks. Emit each tool result separately
                    // and combine the rest into the user message.
                    let mut parts: Vec<String> = Vec::new();
                    for it in items {
                        match it.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                if let Some(s) = it.get("text").and_then(Value::as_str) {
                                    parts.push(s.to_string());
                                }
                            }
                            Some("image") => parts.push("[image]".to_string()),
                            Some("tool_result") => {
                                let id = it
                                    .get("tool_use_id")
                                    .and_then(Value::as_str)
                                    .map(str::to_string);
                                let name = id.as_deref().and_then(|id| tool_names.get(id).cloned());
                                out.push(Event::ToolResult {
                                    id,
                                    name,
                                    text: tool_result_text(it),
                                    is_error: it
                                        .get("is_error")
                                        .and_then(Value::as_bool)
                                        .unwrap_or(false),
                                });
                            }
                            _ => {}
                        }
                    }
                    push_user_text(&mut out, &parts.join("\n"), ts);
                }
                _ => {}
            },
            Some("assistant") => {
                let Some(items) = msg.get("content").and_then(Value::as_array) else {
                    continue;
                };
                for it in items {
                    match it.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(s) = it.get("text").and_then(Value::as_str) {
                                let s = s.trim();
                                // A slash command the CLI handled locally gets this stand-in reply
                                // written to the recording. It says nothing; showing it would only
                                // litter the conversation with the same line after every `/effort`.
                                if !s.is_empty() && s != NO_RESPONSE_PLACEHOLDER {
                                    out.push(Event::AssistantText {
                                        text: s.to_string(),
                                        ts: ts.clone(),
                                    });
                                }
                            }
                        }
                        Some("thinking") => {
                            if let Some(s) = it.get("thinking").and_then(Value::as_str) {
                                let s = s.trim();
                                if !s.is_empty() {
                                    out.push(Event::Thinking {
                                        text: s.to_string(),
                                        ts: ts.clone(),
                                    });
                                }
                            }
                        }
                        Some("tool_use") => {
                            let name = it
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("(unknown tool)")
                                .to_string();
                            let id = it.get("id").and_then(Value::as_str).map(str::to_string);
                            if let Some(id) = id.as_deref() {
                                tool_names.insert(id.to_string(), name.clone());
                            }
                            let input = it
                                .get("input")
                                .cloned()
                                .unwrap_or_else(|| Value::Object(Default::default()));
                            out.push(Event::ToolUse {
                                id,
                                name,
                                input,
                                ts: ts.clone(),
                            });
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

/// Append a genuine user message, discarding empty text and injected blocks.
fn push_user_text(out: &mut Vec<Event>, text: &str, ts: Option<String>) {
    let text = text.trim();
    if text.is_empty() || is_injected_context(text) {
        return;
    }
    out.push(Event::User {
        text: text.to_string(),
        ts,
    });
}

/// Extract text from tool_result content represented as a string or text/image block array.
fn tool_result_text(block: &Value) -> String {
    match block.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|it| match it.get("type").and_then(Value::as_str) {
                Some("text") => it.get("text").and_then(Value::as_str).map(str::to_string),
                Some("image") => Some("[image]".to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Pretty-print a JSON value, falling back to `to_string` if serialization unexpectedly fails.
fn pretty_json(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

// ─────────────────────────── Codex parsing ───────────────────────────

/// Parse a complete Codex rollout into events, skipping injected environment and instruction blocks.
pub(crate) fn codex_events(content: &str) -> Vec<Event> {
    // Map call IDs to tool names for function_call_output display.
    let mut call_names: HashMap<String, String> = HashMap::new();
    let mut out: Vec<Event> = Vec::new();

    for line in content.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let ts = v
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_string);
        let Some(payload) = v.get("payload") else {
            continue;
        };
        match payload.get("type").and_then(Value::as_str) {
            Some("message") => {
                let Some(role) = payload.get("role").and_then(Value::as_str) else {
                    continue;
                };
                let Some(items) = payload.get("content").and_then(Value::as_array) else {
                    continue;
                };
                let text = items
                    .iter()
                    .filter_map(|it| {
                        matches!(
                            it.get("type").and_then(Value::as_str),
                            Some("input_text") | Some("output_text")
                        )
                        .then(|| it.get("text").and_then(Value::as_str))
                        .flatten()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                match role {
                    "user" => {
                        // Modern app-server recordings identify every bundled content source. A row made
                        // only from plugin recommendations, AGENTS.md, environment context, or other
                        // harness input is not something the person typed. Older rollouts lack that
                        // metadata, so retain the known-tag fallback for them.
                        if codex_user_message_is_injected(payload, text) {
                            continue;
                        }
                        out.push(Event::User {
                            text: text.to_string(),
                            ts,
                        });
                    }
                    "assistant" => out.push(Event::AssistantText {
                        text: text.to_string(),
                        ts,
                    }),
                    _ => {}
                }
            }
            Some("reasoning") => {
                // Join text entries from the reasoning summary array.
                let Some(items) = payload.get("summary").and_then(Value::as_array) else {
                    continue;
                };
                let text = items
                    .iter()
                    .filter_map(|it| it.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let text = text.trim();
                if !text.is_empty() {
                    out.push(Event::Thinking {
                        text: text.to_string(),
                        ts,
                    });
                }
            }
            Some("function_call") | Some("custom_tool_call") => {
                let name = payload
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("(unknown tool)")
                    .to_string();
                let id = payload
                    .get("call_id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some(id) = id.as_deref() {
                    call_names.insert(id.to_string(), name.clone());
                }
                // Arguments arrive as a JSON string; parse it back to a value and keep unparseable text as a string.
                let raw = payload
                    .get("arguments")
                    .or_else(|| payload.get("input"))
                    .map(|a| match a {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                let input =
                    serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| Value::String(raw));
                out.push(Event::ToolUse {
                    id,
                    name,
                    input,
                    ts,
                });
            }
            Some("function_call_output") | Some("custom_tool_call_output") => {
                let id = payload
                    .get("call_id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let name = id.as_deref().and_then(|id| call_names.get(id).cloned());
                let text = match payload.get("output") {
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => pretty_json(other),
                    None => String::new(),
                };
                out.push(Event::ToolResult {
                    id,
                    name,
                    text,
                    is_error: false,
                });
            }
            _ => {}
        }
    }
    out
}

pub(crate) fn codex_user_message_is_injected(payload: &Value, text: &str) -> bool {
    if let Some(kinds) = payload
        .pointer("/internal_chat_message_metadata_passthrough/content_item_kinds")
        .and_then(Value::as_array)
    {
        return !kinds
            .iter()
            .filter_map(Value::as_str)
            .any(|kind| kind.starts_with("user."));
    }
    const INJECTED_PREFIXES: &[&str] = &[
        "<environment_context>",
        "<user_instructions>",
        "<recommended_plugins>",
        "<skills_instructions>",
        "<permissions instructions>",
        "<collaboration_mode>",
        "<apps_instructions>",
        "<plugins_instructions>",
        "<multi_agent_mode>",
        "# AGENTS.md instructions",
    ];
    INJECTED_PREFIXES.iter().any(|prefix| text.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stand-in reply the CLI records for a locally handled command is not part of the conversation.
    #[test]
    fn claude_no_response_placeholder_is_dropped() {
        let lines = [
            r#"{"type":"user","message":{"role":"user","content":"/effort high"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"No response requested."}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"a real answer"}]}}"#,
        ];
        let events = claude_events(&lines.join("\n"));
        let texts: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                Event::AssistantText { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["a real answer"]);
    }

    /// Claude exports include reasoning and paired tool inputs/results, skip sidechain/meta/injected
    /// blocks, and lengthen fences when results contain backticks.
    #[test]
    fn claude_full_export() {
        let lines = [
            r#"{"type":"user","message":{"role":"user","content":"help me fix a bug"},"timestamp":"2026-06-08T09:01:47.010Z"}"#,
            r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"META-PRIVATE injection"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"First find where it fails."},{"type":"text","text":"Let me take a look."}]},"timestamp":"t2"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tu_1","name":"Bash","input":{"command":"ls"}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_1","content":"a.rs ```code``` b.rs"}]}}"#,
            r#"{"type":"user","isSidechain":true,"message":{"role":"user","content":"SIDECHAIN-PRIVATE subagent chatter"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"<system-reminder>REMINDER-PRIVATE</system-reminder>"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"<task-notification>TASK-PRIVATE</task-notification>"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"This session is being continued from a previous conversation that ran out of context. CONTINUATION-PRIVATE"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Fixed it."}]}}"#,
        ];
        let events = claude_events(&lines.join("\n"));
        let md = render(
            &events,
            "test session",
            "claude",
            "sid-1",
            "/tmp/x.jsonl",
            Some("2026-06-10 21:00"),
        );

        // Header metadata and statistics.
        assert!(md.contains("# Session Context Export: test session"));
        assert!(md.contains("- **Exported at**: 2026-06-10 21:00"));
        assert!(md.contains("1 user messages · 1 assistant turns · 1 tool calls"));
        // User messages have timestamped headings; meta, sidechain, and injected blocks are absent.
        // Assertions use unique test-data markers to avoid matching header prose.
        assert!(md.contains("## 👤 User · 2026-06-08T09:01:47.010Z"));
        assert!(!md.contains("META-PRIVATE"));
        assert!(!md.contains("SIDECHAIN-PRIVATE"));
        assert!(!md.contains("REMINDER-PRIVATE"));
        assert!(!md.contains("TASK-PRIVATE"));
        assert!(!md.contains("CONTINUATION-PRIVATE"));
        // Reasoning is quoted; tool calls have pretty input; results pair to Bash by ID.
        assert!(md.contains("**💭 Thinking**\n\n> First find where it fails."));
        assert!(md.contains("**🔧 Tool call: Bash**"));
        assert!(md.contains("\"command\": \"ls\""));
        assert!(md.contains("**📥 Tool result (Bash)**"));
        // A result containing ``` upgrades the fence to four backticks.
        assert!(md.contains("````\na.rs ```code``` b.rs\n````"));
        // Tool results do not split an assistant turn; subsequent text remains under one heading.
        assert_eq!(md.matches("## 🤖 Assistant").count(), 1);
        assert!(md.contains("Fixed it."));
    }

    /// Claude tool_result content as text/image blocks with an is_error marker.
    #[test]
    fn claude_tool_result_block_array_and_error() {
        let lines = [
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tu_9","name":"Read","input":{"file_path":"/a"}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_9","is_error":true,"content":[{"type":"text","text":"file not found"},{"type":"image","source":{}}]}]}}"#,
        ];
        let events = claude_events(&lines.join("\n"));
        let md = render(&events, "s", "claude", "id", "/p", None);
        assert!(md.contains("**📥 Tool result (Read) · error**"));
        assert!(md.contains("file not found\n[image]"));
        // Omit the line when no export time is supplied.
        assert!(!md.contains("Exported at"));
    }

    /// Codex filtering, pretty-printed argument JSON, tool names paired by call_id, and quoted reasoning.
    #[test]
    fn codex_full_export() {
        let lines = [
            r#"{"timestamp":"t1","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>\n…\n</environment_context>"}]}}"#,
            r#"{"timestamp":"t1b","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<recommended_plugins>\nPRIVATE PLUGIN LIST\n</recommended_plugins>"}],"internal_chat_message_metadata_passthrough":{"content_item_kinds":["plugins.recommendations","agents_md.instructions"]}}}"#,
            r#"{"timestamp":"t2","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"check the configuration"}]}}"#,
            r#"{"timestamp":"t3","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"I need to find the config file first."}]}}"#,
            r#"{"timestamp":"t4","type":"response_item","payload":{"type":"function_call","name":"shell","call_id":"c1","arguments":"{\"command\":[\"ls\"]}"}}"#,
            r#"{"timestamp":"t5","type":"response_item","payload":{"type":"function_call_output","call_id":"c1","output":"app.yml"}}"#,
            r#"{"timestamp":"t6","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The configuration lives in app.yml."}]}}"#,
        ];
        let events = codex_events(&lines.join("\n"));
        let md = render(&events, "s", "codex", "id", "/p", None);
        assert!(!md.contains("environment_context"));
        assert!(!md.contains("PRIVATE PLUGIN LIST"));
        assert!(md.contains("## 👤 User · t2"));
        assert!(md.contains("> I need to find the config file first."));
        assert!(md.contains("**🔧 Tool call: shell**"));
        assert!(md.contains("\"command\": ["));
        assert!(md.contains("**📥 Tool result (shell)**"));
        assert!(md.contains("app.yml"));
        assert!(md.contains("The configuration lives in app.yml."));
    }

    /// Metadata wins over tag-shaped text: if the person deliberately types a reserved-looking tag, it
    /// still belongs to the conversation, while a harness-only row is dropped.
    #[test]
    fn codex_content_kinds_distinguish_user_text_from_injected_context() {
        let lines = [
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<recommended_plugins>injected</recommended_plugins>"}],"internal_chat_message_metadata_passthrough":{"content_item_kinds":["plugins.recommendations"]}}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<recommended_plugins>typed deliberately</recommended_plugins>"}],"internal_chat_message_metadata_passthrough":{"content_item_kinds":["user.text"]}}}"#,
        ];
        let events = codex_events(&lines.join("\n"));
        let users = events
            .iter()
            .filter_map(|event| match event {
                Event::User { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(users, ["<recommended_plugins>typed deliberately</recommended_plugins>"]);
    }

    /// Empty recordings retain a complete header and an empty-conversation placeholder.
    #[test]
    fn empty_transcript_renders_placeholder() {
        let md = render(&[], "empty session", "claude", "id", "/p", None);
        assert!(md.contains("# Session Context Export: empty session"));
        assert!(md.contains("(Transcript is empty)"));
    }

    /// Fence length follows the longest backtick run in the content.
    #[test]
    fn fence_grows_past_backtick_runs() {
        assert_eq!(fence_for("no backticks"), "```");
        assert_eq!(fence_for("inline `code` span"), "```");
        assert_eq!(fence_for("``` three in a row"), "````");
        assert_eq!(fence_for("````` five in a row"), "``````");
    }

    /// Read-only local smoke test that exports the latest real Claude transcript to a temporary file and
    /// prints its header for inspection. Ignored by default because it depends on local `~/.claude` data.
    /// Run with `cargo test --lib -- --ignored export_real_claude_transcript_smoke --nocapture`.
    #[test]
    #[ignore = "depends on the real ~/.claude/projects history on this machine"]
    fn export_real_claude_transcript_smoke() {
        let projects = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .expect("HOME")
            .join(".claude/projects");
        let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
        for dir in std::fs::read_dir(&projects).expect("reading projects").flatten() {
            let Ok(entries) = std::fs::read_dir(dir.path()) else {
                continue;
            };
            for f in entries.flatten() {
                let p = f.path();
                if p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    if let Ok(m) = p.metadata().and_then(|m| m.modified()) {
                        files.push((m, p));
                    }
                }
            }
        }
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        let path = &files.first().expect("there should be at least one transcript").1;
        eprintln!("exporting: {}", path.display());
        let content = std::fs::read_to_string(path).expect("reading the transcript");
        let events = claude_events(&content);
        let md = render(
            &events,
            "real-machine smoke test",
            "claude",
            "smoke",
            &path.display().to_string(),
            Some("2026-06-10 21:20"),
        );
        let out = std::env::temp_dir().join("vlx-export-smoke.md");
        std::fs::write(&out, &md).expect("writing the temporary artefact");
        eprintln!(
            "artefact ({} characters) written to: {}",
            md.chars().count(),
            out.display()
        );
        let head: String = md.chars().take(800).collect();
        eprintln!("---- start of the artefact ----\n{head}");
        assert!(md.contains("## 👤 User"));
        assert!(md.contains("## 🤖 Assistant"));
        assert!(md.contains("**🔧 Tool call:"));
        assert!(md.contains("**📥 Tool result"));
    }
}
