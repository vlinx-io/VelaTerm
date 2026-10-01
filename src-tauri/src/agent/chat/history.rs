//! Structured conversation events for the session view — the chat-style second view of an agent session.
//!
//! The Markdown export in `export.rs` and this module read the same recordings through the same parser;
//! they differ only in what they produce. Export flattens everything into one Markdown document, while the
//! session view needs addressable rows: one bubble per message, one card per tool call, each carrying the
//! fields the card renders (a Bash `command`, an Edit `old_string`/`new_string`) rather than a pre-rendered
//! block of text.
//!
//! Two transformations happen here and nowhere else:
//! - a tool call and the result answering it are folded into a single row, paired by tool id, because the UI
//!   draws them as one card that starts pending and later fills in its output;
//! - directly adjacent assistant/thinking rows of the same kind are joined, since one logical response often
//!   arrives as several content blocks. User rows stay separate because each is a rewind boundary.
//!
//! Two readers take these rows. The read-only session view renders `ChatEvent`s as they are. The chat engine
//! goes through `replay`, which converts them into the engine's own timeline rows, so a conversation the
//! agent is resuming opens with everything that was already said instead of a blank page.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::models::SessionKind;

use crate::agent::chat::engine::{ChatRow, SubagentInfo};
use crate::agent::export::{
    claude_events, codex_events, codex_user_message_is_injected, pi_events, Event,
};
use crate::agent::resume;
use crate::agent::transcript::is_injected_context;

/// Provider-native identity of one genuine user turn, used by rewind without exposing recording paths.
#[derive(Clone, Debug, PartialEq)]
pub struct RewindTarget {
    pub message_id: String,
    /// The final native message of a shell command stored as separate input and output records.
    pub last_message_id: Option<String>,
    pub turn_id: Option<String>,
    /// Visible user text, used to align provider ids with replay rows while skipping local command output.
    pub text: String,
}

/// One row of the session view, in file order.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    /// Position in the returned list. Stable for a given transcript length, so the frontend uses it as the
    /// React key and as the anchor for "scroll to this message" from global search.
    pub index: usize,
    /// `user`, `assistant`, `thinking`, `command`, or `tool`.
    pub kind: &'static str,
    /// Message body. Empty for a tool row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// ISO timestamp copied from the recording, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Tool name, for a tool row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Raw tool input, so each card can pick out the fields it shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    /// Tool output text. Absent while the call is still pending.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// The tool reported a failure.
    pub is_error: bool,
    /// The call has no result yet — either it is still running, or the recording ends mid-call.
    pub pending: bool,
    /// Backend-parsed shell context; read-only clients render this without command controls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<ChatRow>,
    /// Provider id used only while rebuilding richer live rows; it is not part of the IPC contract.
    #[serde(skip)]
    native_id: Option<String>,
}

impl ChatEvent {
    fn message(kind: &'static str, text: String, timestamp: Option<String>) -> Self {
        ChatEvent {
            index: 0,
            kind,
            text: Some(text),
            timestamp,
            tool: None,
            input: None,
            output: None,
            is_error: false,
            pending: false,
            native_id: None,
            shell: None,
        }
    }
}

/// Read a session's recording and return the session-view rows.
///
/// Errors carry the reason the view cannot be shown — deleted recording, or an agent whose history is not a
/// flat parseable file — so the frontend can offer the terminal view instead. Error strings are English
/// because they surface directly in the UI.
pub fn read(kind: SessionKind, agent_session_id: &str) -> Result<Vec<ChatEvent>, String> {
    if kind == SessionKind::Kiro {
        return Ok(crate::agent::kiro_store::read(agent_session_id)?.messages?
            .into_iter().enumerate().map(|(index, message)| {
                let mut event = ChatEvent::message(message.role, message.text, message.timestamp);
                event.index = index;
                event.native_id = message.native_id;
                event
            }).collect());
    }
    let events = match kind {
        SessionKind::Claude => claude_events(&resume::read_claude_transcript(agent_session_id)?),
        SessionKind::Codex => {
            let (_, content) = resume::read_codex_rollout_chain(agent_session_id)?;
            codex_events(&content)
        }
        SessionKind::Opencode => {
            let messages = crate::agent::opencode_store::messages(agent_session_id)?;
            super::opencode_timeline::events(&messages)
        }
        // Pi and OMP record an append-only tree of entries; the parser keeps the active branch.
        SessionKind::Pi | SessionKind::Omp => {
            pi_events(&resume::read_pi_transcript(kind, agent_session_id)?)
        }
        // Other agents keep their history in SQLite blobs or formats we do not parse. Use a fallback arm so a
        // newly added session kind compiles as unsupported rather than silently rendering an empty view.
        other => {
            return Err(format!(
                "Session view is not supported for {} sessions yet",
                other.as_str()
            ))
        }
    };
    Ok(if kind == SessionKind::Claude { fold_claude(events) } else { fold(events) })
}

/// Read provider-native user-turn identities in transcript order.
///
/// The live timeline deliberately uses its own stable row IDs. Rewind resolves those rows by ordinal from
/// the tail, then sends only the provider-native ID back to the already-running protocol peer.
pub fn rewind_targets(
    kind: SessionKind,
    agent_session_id: &str,
) -> Result<Vec<RewindTarget>, String> {
    let content = match kind {
        SessionKind::Claude => resume::read_claude_transcript(agent_session_id)?,
        SessionKind::Codex => resume::read_codex_rollout_chain(agent_session_id)?.1,
        SessionKind::Opencode => {
            let messages = crate::agent::opencode_store::messages(agent_session_id)?;
            return Ok(super::opencode_timeline::user_turns(&messages));
        }
        other => {
            return Err(format!(
                "Conversation rewind is not supported for {} sessions",
                other.as_str()
            ))
        }
    };
    Ok(match kind {
        SessionKind::Claude => claude_rewind_targets(&content),
        SessionKind::Codex => codex_rewind_targets(&content),
        _ => unreachable!(),
    })
}

fn claude_rewind_targets(content: &str) -> Vec<RewindTarget> {
    let targets: Vec<RewindTarget> = content
        .lines()
        .filter_map(|line| {
            let value = serde_json::from_str::<Value>(line).ok()?;
            if value.get("isSidechain").and_then(Value::as_bool).unwrap_or(false)
                || value.get("isMeta").and_then(Value::as_bool).unwrap_or(false)
            {
                return None;
            }
            let uuid = value.get("uuid").and_then(Value::as_str)?.to_string();
            if value.get("type").and_then(Value::as_str) != Some("user") {
                return None;
            }
            let content = value.pointer("/message/content")?;
            let (mut text, has_image) = match content {
                Value::String(text) => (text.trim().to_string(), false),
                Value::Array(items) => (
                    items
                        .iter()
                        .filter_map(|item| match item.get("type").and_then(Value::as_str) {
                            Some("text") => item.get("text").and_then(Value::as_str).map(str::to_string),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .to_string(),
                    items.iter().any(|item| item.get("type").and_then(Value::as_str) == Some("image")),
                ),
                _ => return None,
            };
            if text.starts_with("<command-name>") {
                let name = between(&text, "<command-name>", "</command-name>").unwrap_or_default();
                let args = between(&text, "<command-args>", "</command-args>").unwrap_or_default();
                text = [name.trim(), args.trim()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
            }
            ((!text.is_empty() || has_image) && !is_injected_context(&text)).then_some(RewindTarget {
                message_id: uuid,
                last_message_id: None, turn_id: None,
                text,
            })
        })
        .collect();
    let parents: HashMap<String, String> = content.lines().filter_map(|line| {
        let value = serde_json::from_str::<Value>(line).ok()?;
        Some((value["uuid"].as_str()?.into(), value["parentUuid"].as_str()?.into()))
    }).collect();
    let mut merged: Vec<RewindTarget> = Vec::new();
    for target in targets {
        if let Some(input) = merged.last_mut() {
            if parents.get(&target.message_id) == Some(&input.message_id) {
                if let Some((text, _)) = native_shell_pair(&input.text, &target.text) {
                    input.text = text;
                    input.last_message_id = Some(target.message_id);
                    continue;
                }
            }
        }
        merged.push(target);
    }
    merged
}

fn between<'a>(value: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = value.find(start)? + start.len();
    let to = value[from..].find(end)? + from;
    Some(&value[from..to])
}

fn codex_rewind_targets(content: &str) -> Vec<RewindTarget> {
    let mut turn_id: Option<String> = None;
    let mut targets = Vec::new();
    for line in content.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        if value.get("type").and_then(Value::as_str) == Some("turn_context") {
            turn_id = value
                .pointer("/payload/turn_id")
                .or_else(|| value.pointer("/payload/turnId"))
                .and_then(Value::as_str)
                .map(str::to_string);
            continue;
        }
        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = value.get("payload") else { continue };
        if payload.get("type").and_then(Value::as_str) != Some("message")
            || payload.get("role").and_then(Value::as_str) != Some("user")
        {
            continue;
        }
        let content = payload.get("content").and_then(Value::as_array);
        let text = content
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        let has_image = content
            .is_some_and(|items| items.iter().any(|item| matches!(item.get("type").and_then(Value::as_str), Some("input_image" | "image"))));
        if (!has_image && text.is_empty()) || codex_user_message_is_injected(payload, &text) {
            continue;
        }
        let Some(native_turn) = turn_id.clone() else { continue };
        targets.push(RewindTarget {
            message_id: payload
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(&native_turn)
                .to_string(),
            last_message_id: None, turn_id: Some(native_turn),
            text,
        });
    }
    targets
}

/// Fold parsed events into rows: pair each tool result with its call, and join adjacent same-kind messages.
fn fold(events: Vec<Event>) -> Vec<ChatEvent> {
    let mut out: Vec<ChatEvent> = Vec::new();
    // Tool id -> index in `out` of the pending card waiting for that result.
    let mut pending: HashMap<String, usize> = HashMap::new();

    for ev in events {
        match ev {
            Event::User { text, ts } => push_message(&mut out, "user", text, ts),
            Event::Shell { id, command, output, exit_code, cancelled, truncated, ts } => {
                let mut event = ChatEvent::message("user", format!("!{command}"), ts);
                let context = super::shell::ShellContext {
                    command, stdout: output, exit_code, cancelled, stdout_truncated: truncated,
                    ..Default::default()
                };
                event.native_id = id;
                event.shell = Some(super::shell::row(replay_id(out.len()), parsed_at(event.timestamp.as_deref()),
                    &context, context.status()));
                out.push(event);
            }
            Event::AssistantText { text, ts } => push_message(&mut out, "assistant", text, ts),
            Event::Thinking { text, ts } => push_message(&mut out, "thinking", text, ts),
            // Each command answers a separate line someone typed, so two of them stay two rows even when
            // they arrive back to back.
            Event::Command { text, ts } => out.push(ChatEvent::message("command", text, ts)),
            Event::ToolUse {
                id,
                name,
                input,
                ts,
            } => {
                if let Some(id) = id.clone() {
                    pending.insert(id, out.len());
                }
                out.push(ChatEvent {
                    index: 0,
                    kind: "tool",
                    text: None,
                    timestamp: ts,
                    tool: Some(name),
                    input: Some(input),
                    output: None,
                    is_error: false,
                    pending: true,
                    native_id: id,
                    shell: None,
                });
            }
            Event::ToolResult {
                id,
                name,
                text,
                is_error,
            } => {
                // Normal case: the result answers a call we already have a card for.
                let slot = id.as_deref().and_then(|id| pending.remove(id));
                if let Some(i) = slot {
                    out[i].output = Some(text);
                    out[i].is_error = is_error;
                    out[i].pending = false;
                    continue;
                }
                // The call is missing — a truncated recording, or a result carried over from a resumed
                // session whose earlier half lives in another file. Show the result on its own.
                out.push(ChatEvent {
                    index: 0,
                    kind: "tool",
                    text: None,
                    timestamp: None,
                    tool: name,
                    input: None,
                    output: Some(text),
                    is_error,
                    pending: false,
                    native_id: id,
                    shell: None,
                });
            }
        }
    }

    for (i, ev) in out.iter_mut().enumerate() {
        ev.index = i;
        if ev.kind == "user" && ev.shell.is_none() {
            ev.shell = ev.text.as_deref().and_then(super::shell::parse_context).map(|context| {
                let at = parsed_at(ev.timestamp.as_deref());
                super::shell::row(replay_id(i), at, &context, context.status())
            });
        }
    }
    out
}

/// Claude 2.1.278 records native shell input and output as two adjacent user messages. Neither record
/// contains execution metadata; status-looking output must remain intact and its exit code unknown.
fn native_shell_pair(input: &str, output: &str) -> Option<(String, super::shell::ShellContext)> {
    let command = input.strip_prefix("<bash-input>")?.strip_suffix("</bash-input>")?;
    let context = super::shell::legacy_context(command, output)?;
    Some((format!("{input}\n{output}"), context))
}

fn fold_claude(events: Vec<Event>) -> Vec<ChatEvent> {
    let mut out: Vec<ChatEvent> = Vec::new();
    for event in fold(events) {
        if let Some(previous) = out.last_mut() {
            if previous.kind == "user" && event.kind == "user" {
                if let Some((text, context)) = native_shell_pair(
                    previous.text.as_deref().unwrap_or(""), event.text.as_deref().unwrap_or(""),
                ) {
                    let mut shell = super::shell::row(replay_id(previous.index),
                        parsed_at(previous.timestamp.as_deref()), &context, super::shell::STATUS_COMPLETED);
                    if let ChatRow::Shell { source_text, .. } = &mut shell { *source_text = text.clone(); }
                    previous.text = Some(text);
                    previous.shell = Some(shell);
                    continue;
                }
            }
        }
        out.push(event);
    }
    // Keep original indexes: removing an output-only row must not shift later search anchors.
    out
}

/// Append a message, joining it to the previous row when that row is the same kind of message.
///
/// One turn commonly arrives as several content blocks; drawing each as its own bubble would break a single
/// answer into fragments. A tool row in between ends the run, because text before and after a tool call are
/// genuinely separate parts of the answer.
fn push_message(out: &mut Vec<ChatEvent>, kind: &'static str, text: String, ts: Option<String>) {
    if let Some(last) = out.last_mut() {
        if kind != "user" && last.kind == kind {
            if let Some(prev) = last.text.as_mut() {
                prev.push_str("\n\n");
                prev.push_str(&text);
                return;
            }
        }
    }
    out.push(ChatEvent::message(kind, text, ts));
}

// ─────────────────────────── Replaying into a live conversation ───────────────────────────

#[derive(Clone, Debug)]
struct CodexSubagentReplay {
    thread_id: String,
    description: Option<String>,
    status: &'static str,
    model: Option<String>,
    total_tokens: Option<u64>,
    children: Vec<ChatRow>,
}

/// Id of a replayed row.
///
/// The prefix keeps these apart from the ids the live conversation uses — a message id with a block index,
/// a tool id, `u-<n>` for a turn the view echoed itself — so a replayed row can never be overwritten by a
/// live one, or the other way round. The index is the row's position in the whole recording, not in the
/// displayed page, so the ids do not shift when a client loads earlier history.
fn replay_id(index: usize) -> String {
    format!("h-{index}")
}

/// Read a recording back as timeline rows, for a conversation the agent is resuming.
///
/// The agent remembers the conversation across a restart, but the timeline is in memory and starts empty,
/// so without this the view would be blank in front of an agent that knows exactly what was said.
/// `directory` is where the agent runs; for Claude it names the one recording the agent continues.
pub fn replay(kind: SessionKind, agent_session_id: &str, directory: Option<&Path>) -> Result<Vec<ChatRow>, String> {
    let mut rows = replay_recorded(kind, agent_session_id, directory)?;
    // The recording holds the message shell mode sent the agent as a user message; the view shows it as
    // the command row it was. One pass here covers every provider.
    super::shell::map_replayed_rows(&mut rows);
    Ok(rows)
}

/// The Claude recording to replay, where it lies, and its checked handle to read it and its children from.
///
/// With an absolute working directory it is the recording in that directory's key folder, checked the way
/// the resume picker checks it before binding, so no same-named file or link in another folder can take
/// its place. Without one, the recording every other reader of the id uses (`open_recording`).
fn claude_recording(root: Option<&Path>, agent_session_id: &str, directory: Option<&Path>) -> Result<(PathBuf, std::fs::File), String> {
    const MISSING: &str = "Claude transcript file not found";
    let root = root.ok_or(MISSING)?;
    match directory.filter(|directory| directory.is_absolute()) {
        Some(directory) => super::conversations::open_claude(root, directory, agent_session_id),
        None => super::conversations::open_recording(root, agent_session_id),
    }
}

fn replay_recorded(kind: SessionKind, agent_session_id: &str, directory: Option<&Path>) -> Result<Vec<ChatRow>, String> {
    if kind == SessionKind::Opencode {
        // OpenCode's store already holds addressable parts, and its subagents are whole sessions of their
        // own; the row ids come from the store so a later live event can update the same rows.
        let messages = crate::agent::opencode_store::messages(agent_session_id)?;
        let children = |id: &str| crate::agent::opencode_store::messages(id).ok();
        let mut rows = super::opencode_timeline::rows(&messages, &children);
        attach_recorded_turn_durations(&mut rows);
        return Ok(rows);
    }
    if kind == SessionKind::Claude {
        let root = super::conversations::projects_root();
        let (path, mut file) = claude_recording(root.as_deref(), agent_session_id, directory)?;
        return claude_checked_replay(&path, &mut file, super::conversations::CHILDREN_LIMIT);
    }
    if kind != SessionKind::Codex {
        return Ok(to_rows(read(kind, agent_session_id)?));
    }
    let (_, content) = resume::read_codex_rollout_chain(agent_session_id)?;
    let events = fold(codex_events(&content))
        .into_iter()
        .filter(codex_replay_event_visible)
        .collect();
    let mut read_thread = |id: &str| resume::read_codex_rollout_chain(id).map(|(_, value)| value);
    let subagents = codex_subagent_replays(&content, &mut read_thread);
    Ok(to_rows_with_subagents(events, &subagents))
}

/// The replay of a checked Claude recording. Its children are read through the recording's own folder,
/// all of them together within `children_limit`.
fn claude_checked_replay(path: &Path, file: &mut std::fs::File, children_limit: u64) -> Result<Vec<ChatRow>, String> {
    let content = resume::claude_active_branch(&super::conversations::read_whole(file)?);
    let mut remaining = children_limit;
    Ok(claude_replay(&content, &mut |id| {
        super::conversations::read_child(path, file, id, &mut remaining)
    }, &mut HashSet::new(), 0))
}

/// Rebuild only children linked by native tool results. Unrelated sidechain files stay out of the view.
fn claude_replay(
    content: &str,
    read_child: &mut impl FnMut(&str) -> Result<String, String>,
    ancestors: &mut HashSet<String>,
    depth: usize,
) -> Vec<ChatRow> {
    let records: Vec<Value> = content.lines().filter_map(|line| serde_json::from_str(line).ok()).collect();
    let mut links = HashMap::new();
    for record in &records {
        if record.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let Some(result) = record.get("toolUseResult").filter(|r| r.get("agentId").and_then(Value::as_str).is_some()) else {
            continue;
        };
        if let Some(blocks) = record.pointer("/message/content").and_then(Value::as_array) {
            for block in blocks {
                if block.get("type").and_then(Value::as_str) == Some("tool_result") {
                    if let Some(id) = block.get("tool_use_id").and_then(Value::as_str) {
                        links.insert(id.to_string(), result.clone());
                    }
                }
            }
        }
    }
    let events = fold_claude(claude_events(content));
    let mut rows = Vec::with_capacity(events.len());
    for event in events {
        let result = event.native_id.as_ref().and_then(|id| links.get(id));
        let mut row = to_row(event, None);
        if let (Some(result), ChatRow::Tool { name, input, children, subagent, status, .. }) = (result, &mut row) {
            if matches!(name.as_str(), "Task" | "Agent") {
                let id = result["agentId"].as_str().unwrap();
                *subagent = Some(SubagentInfo {
                    title: input.get("name").and_then(Value::as_str).map(str::to_string),
                    description: result.get("description").or_else(|| input.get("description")).and_then(Value::as_str).map(str::to_string),
                    model: result.get("resolvedModel").and_then(Value::as_str).map(str::to_string),
                    total_tokens: result.get("totalTokens").and_then(Value::as_u64),
                    tool_uses: result.get("totalToolUseCount").and_then(Value::as_u64),
                    duration_ms: result.get("totalDurationMs").and_then(Value::as_u64),
                });
                let loaded = if depth >= 16 || !ancestors.insert(id.to_string()) {
                    Err("Nested subagent history exceeds the replay limit".to_string())
                } else {
                    let loaded = read_child(id).map(|text| {
                        // A child file marks its own messages as sidechain. They become the local main
                        // chain only inside its card; the ordinary root/export parser is unchanged.
                        let branch = resume::claude_active_branch(&text);
                        let normalized = branch.lines().filter_map(|line| {
                            let mut value: Value = serde_json::from_str(line).ok()?;
                            value["isSidechain"] = Value::Bool(false);
                            Some(value.to_string())
                        }).collect::<Vec<_>>().join("\n");
                        let last = normalized.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok())
                            .filter(|v| v.get("type").and_then(Value::as_str) == Some("assistant")).last();
                        if let Some(last) = last {
                            if let Some(facts) = subagent {
                                if facts.model.is_none() {
                                    facts.model = normalized.lines().rev()
                                        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                                        .find_map(|value| value.pointer("/message/model").and_then(Value::as_str).map(str::to_string));
                                }
                            }
                            if result.get("isAsync").and_then(Value::as_bool) == Some(true) {
                                *status = if last.pointer("/message/stop_reason").and_then(Value::as_str) == Some("end_turn") { "completed" } else { "running" };
                            }
                        }
                        let mut rows = claude_replay(&normalized, read_child, ancestors, depth + 1);
                        scope_child_rows(&mut rows, id);
                        rows
                    });
                    ancestors.remove(id);
                    loaded
                };
                *children = loaded.unwrap_or_else(|message| vec![ChatRow::Notice {
                    id: format!("h-child-unavailable-{id}"), message,
                }]);
            }
        }
        rows.push(row);
    }
    attach_recorded_turn_durations(&mut rows);
    rows
}

fn codex_replay_event_visible(event: &ChatEvent) -> bool {
    if event.kind != "tool" {
        return true;
    }
    !matches!(
        event.tool.as_deref(),
        Some("wait_agent" | "wait" | "send_message" | "followup_task" | "interrupt_agent" | "list_agents")
    )
}

/// Deferred details are addressed by row id across the whole timeline, including nested cards.
/// Each Claude recording starts at h-0, so its child rows need their recording's namespace.
fn scope_child_rows(rows: &mut [ChatRow], agent_id: &str) {
    for row in rows {
        let id = match row {
            ChatRow::User { id, .. } | ChatRow::Assistant { id, .. } | ChatRow::Reasoning { id, .. }
            | ChatRow::Tool { id, .. } | ChatRow::Error { id, .. } | ChatRow::Command { id, .. }
            | ChatRow::Shell { id, .. } | ChatRow::Notice { id, .. } | ChatRow::Compaction { id, .. } => id,
        };
        *id = format!("child-{agent_id}-{id}");
        if let ChatRow::Tool { children, .. } = row { scope_child_rows(children, agent_id); }
    }
}

/// Restore every parsed row. Snapshot windows bound transport; replay must retain the source for paging.
fn to_rows(events: Vec<ChatEvent>) -> Vec<ChatRow> {
    to_rows_with_subagents(events, &HashMap::new())
}

fn to_rows_with_subagents(
    events: Vec<ChatEvent>,
    subagents: &HashMap<String, CodexSubagentReplay>,
) -> Vec<ChatRow> {
    let mut rows: Vec<ChatRow> = Vec::with_capacity(events.len());
    for ev in events {
        let subagent = ev.native_id.as_ref().and_then(|id| subagents.get(id)).cloned();
        rows.push(to_row(ev, subagent));
    }
    attach_recorded_turn_durations(&mut rows);
    rows
}

/// Recover completed-turn durations from provider timestamps when reopening a conversation.
///
/// Recordings do not all carry an explicit duration, but both Claude and Codex timestamp their genuine
/// user messages and assistant output. The last assistant row before the next user boundary is the final
/// answer for that turn. Missing, malformed, or backwards timestamps intentionally produce no value.
fn attach_recorded_turn_durations(rows: &mut [ChatRow]) {
    let mut started_at = None;
    let mut last_answer = None;
    let mut durations = Vec::new();

    for (index, row) in rows.iter().enumerate() {
        match row {
            ChatRow::User { at, .. } => {
                if let (Some(started), Some((answer_index, completed))) = (started_at, last_answer) {
                    if completed >= started {
                        durations.push((answer_index, (completed - started) as u64));
                    }
                }
                started_at = *at;
                last_answer = None;
            }
            ChatRow::Assistant { at: Some(completed), .. }
                if started_at.is_some_and(|started| *completed >= started) =>
            {
                last_answer = Some((index, *completed));
            }
            _ => {}
        }
    }
    if let (Some(started), Some((answer_index, completed))) = (started_at, last_answer) {
        if completed >= started {
            durations.push((answer_index, (completed - started) as u64));
        }
    }
    for (index, duration) in durations {
        if let ChatRow::Assistant { duration_ms, .. } = &mut rows[index] {
            *duration_ms = Some(duration);
        }
    }
}

/// Read the child thread named by every recorded collaboration call.
///
/// Root and child rollouts are separate files. The root keeps only the call id and child thread id, so
/// replay has to follow that relation explicitly; ordinary conversation parsing intentionally never mixes
/// the child's inherited prompt and private harness context into the visible root conversation.
fn codex_subagent_replays<F>(
    content: &str,
    read_thread: &mut F,
) -> HashMap<String, CodexSubagentReplay>
where
    F: FnMut(&str) -> Result<String, String>,
{
    let mut seen = HashSet::new();
    collect_codex_subagent_replays(content, read_thread, &mut seen, 0)
}

fn collect_codex_subagent_replays<F>(
    content: &str,
    read_thread: &mut F,
    seen: &mut HashSet<String>,
    depth: usize,
) -> HashMap<String, CodexSubagentReplay>
where
    F: FnMut(&str) -> Result<String, String>,
{
    let mut cards = HashMap::<String, CodexSubagentReplay>::new();
    let mut by_thread = HashMap::<String, String>::new();
    for line in content.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        let Some(item) = recorded_completed_item(&value) else { continue };
        let item_type = normalized_item_type(item);
        if item_type == "subagentactivity" {
            let Some(thread_id) = value_string(item, &["agent_thread_id", "agentThreadId"]) else {
                continue;
            };
            let kind = value_string(item, &["kind"]).unwrap_or("started");
            if kind == "started" {
                let Some(call_id) = value_string(item, &["id"]) else { continue };
                by_thread.insert(thread_id.to_string(), call_id.to_string());
                cards.entry(call_id.to_string()).or_insert_with(|| CodexSubagentReplay {
                    thread_id: thread_id.to_string(),
                    description: value_string(item, &["agent_path", "agentPath"]).map(str::to_string),
                    status: "running",
                    model: None,
                    total_tokens: None,
                    children: Vec::new(),
                });
            } else if let Some(call_id) = by_thread.get(thread_id) {
                if let Some(card) = cards.get_mut(call_id) {
                    card.status = recorded_subagent_status(kind);
                    if card.description.is_none() {
                        card.description = value_string(item, &["agent_path", "agentPath"])
                            .map(str::to_string);
                    }
                }
            }
        } else if item_type == "collabagenttoolcall" {
            // Some app-server versions record the creation as a collaboration item without a separate
            // SubAgentActivity start. Empty receiver lists are wait/send operations, not new Task cards.
            let Some(thread_id) = value_array(item, &["receiver_thread_ids", "receiverThreadIds"])
                .and_then(|ids| ids.iter().find_map(Value::as_str))
            else {
                continue;
            };
            let Some(call_id) = value_string(item, &["id"]) else { continue };
            by_thread.entry(thread_id.to_string()).or_insert_with(|| call_id.to_string());
            cards.entry(call_id.to_string()).or_insert_with(|| CodexSubagentReplay {
                thread_id: thread_id.to_string(),
                description: value_string(item, &["prompt", "tool"]).map(str::to_string),
                status: recorded_tool_status(item),
                model: value_string(item, &["model"]).map(str::to_string),
                total_tokens: None,
                children: Vec::new(),
            });
        }
    }

    if depth >= 16 {
        return cards;
    }
    for card in cards.values_mut() {
        let Ok((children, total_tokens, model)) = recorded_codex_thread_rows(
            &card.thread_id,
            read_thread,
            seen,
            depth + 1,
        ) else {
            continue;
        };
        card.children = children;
        card.total_tokens = total_tokens;
        if card.model.is_none() {
            card.model = model;
        }
    }
    cards
}

fn recorded_codex_thread_rows<F>(
    thread_id: &str,
    read_thread: &mut F,
    seen: &mut HashSet<String>,
    depth: usize,
) -> Result<(Vec<ChatRow>, Option<u64>, Option<String>), String>
where
    F: FnMut(&str) -> Result<String, String>,
{
    if depth > 16 || !seen.insert(thread_id.to_string()) {
        return Ok((Vec::new(), None, None));
    }
    let content = read_thread(thread_id)?;
    let nested = collect_codex_subagent_replays(&content, read_thread, seen, depth);
    let mut rows = Vec::<ChatRow>::new();
    let mut nested_by_thread = HashMap::<String, usize>::new();
    let mut total_tokens = None;
    let mut model = None;

    for line in content.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        if model.is_none()
            && value.pointer("/payload/type").and_then(Value::as_str) == Some("thread_settings_applied")
            && value_string(value.pointer("/payload/thread_settings").unwrap_or(&Value::Null), &["model"])
                .is_some()
        {
            model = value.pointer("/payload/thread_settings/model")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        if value.get("type").and_then(Value::as_str) == Some("token_usage_record") {
            total_tokens = [
                "/payload/thread_token_usage/total_tokens",
                "/payload/threadTokenUsage/totalTokens",
                "/payload/usage/total_tokens",
                "/payload/usage/totalTokens",
            ]
            .into_iter()
            .find_map(|pointer| value.pointer(pointer).and_then(Value::as_u64))
            .or(total_tokens);
            continue;
        }
        let Some(item) = recorded_completed_item(&value) else { continue };
        if normalized_item_type(item) == "subagentactivity" {
            let Some(child_thread) = value_string(item, &["agent_thread_id", "agentThreadId"]) else {
                continue;
            };
            let kind = value_string(item, &["kind"]).unwrap_or("started");
            if kind == "started" {
                let Some(id) = value_string(item, &["id"]) else { continue };
                let replay = nested.get(id);
                let description = value_string(item, &["agent_path", "agentPath"])
                    .map(str::to_string)
                    .or_else(|| replay.and_then(|card| card.description.clone()));
                let index = rows.len();
                rows.push(ChatRow::Tool {
                    id: id.to_string(),
                    name: "Task".to_string(),
                    input: serde_json::json!({
                        "description":description,
                        "agentThreadId":child_thread
                    }),
                    output: None,
                    is_error: false,
                    status: replay.map(|card| card.status).unwrap_or("running"),
                    subagent: Some(SubagentInfo {
                        title: Some("Codex subagent".to_string()),
                        description,
                        model: replay.and_then(|card| card.model.clone()),
                        total_tokens: replay.and_then(|card| card.total_tokens),
                        tool_uses: None,
                        duration_ms: None,
                    }),
                    children: replay.map(|card| card.children.clone()).unwrap_or_default(),
                });
                nested_by_thread.insert(child_thread.to_string(), index);
            } else if let Some(index) = nested_by_thread.get(child_thread).copied() {
                if let Some(ChatRow::Tool { status, is_error, .. }) = rows.get_mut(index) {
                    *status = recorded_subagent_status(kind);
                    *is_error = *status == "failed";
                }
            }
            continue;
        }
        if let Some(row) = recorded_codex_item_row(item, value.get("timestamp").and_then(Value::as_str)) {
            rows.push(row);
        }
    }

    Ok((rows, total_tokens, model))
}

fn recorded_completed_item(value: &Value) -> Option<&Value> {
    let event = value.pointer("/payload/type").and_then(Value::as_str)?;
    matches!(event, "item_completed" | "itemCompleted")
        .then(|| value.pointer("/payload/item"))
        .flatten()
}

fn normalized_item_type(item: &Value) -> String {
    value_string(item, &["type"])
        .unwrap_or_default()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn value_string<'a>(value: &'a Value, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|name| value.get(*name).and_then(Value::as_str))
}

fn value_array<'a>(value: &'a Value, names: &[&str]) -> Option<&'a Vec<Value>> {
    names.iter().find_map(|name| value.get(*name).and_then(Value::as_array))
}

fn recorded_subagent_status(kind: &str) -> &'static str {
    match kind {
        "completed" | "success" => "completed",
        "interrupted" | "canceled" | "cancelled" => "canceled",
        "started" | "running" => "running",
        _ => "failed",
    }
}

fn recorded_tool_status(item: &Value) -> &'static str {
    match value_string(item, &["status"]) {
        Some("inProgress" | "in_progress" | "running") => "running",
        Some("completed" | "success") | None => "completed",
        Some(_) => "failed",
    }
}

fn recorded_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.clone()),
                Value::Object(_) => value_string(item, &["text", "summary_text", "summaryText"])
                    .map(str::to_string),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        Some(value) => value.to_string(),
        None => String::new(),
    }
}

fn recorded_codex_item_row(item: &Value, timestamp: Option<&str>) -> Option<ChatRow> {
    let id = value_string(item, &["id"])?.to_string();
    let status = recorded_tool_status(item);
    match normalized_item_type(item).as_str() {
        "agentmessage" => {
            let text = recorded_text(item.get("content"));
            (!text.trim().is_empty()).then_some(ChatRow::Assistant {
                id,
                text,
                streaming: false,
                at: parsed_at(timestamp),
                model: None,
                duration_ms: None,
            })
        }
        "reasoning" => {
            let text = recorded_text(
                item.get("summary_text")
                    .or_else(|| item.get("summaryText"))
                    .or_else(|| item.get("summary"))
                    .or_else(|| item.get("raw_content")),
            );
            (!text.trim().is_empty()).then_some(ChatRow::Reasoning { id, text, streaming: false })
        }
        "commandexecution" => Some(ChatRow::Tool {
            id,
            name: "Bash".to_string(),
            input: serde_json::json!({
                "command":recorded_command(item.get("command")),
                "cwd":item.get("cwd").cloned().unwrap_or(Value::Null)
            }),
            output: value_string(item, &["aggregated_output", "aggregatedOutput", "formatted_output", "stdout"])
                .map(str::to_string),
            is_error: status == "failed",
            status,
            subagent: None,
            children: Vec::new(),
        }),
        "filechange" => Some(ChatRow::Tool {
            id,
            name: "FileChange".to_string(),
            input: serde_json::json!({
                "changes":item.get("changes").cloned().unwrap_or_else(|| serde_json::json!([]))
            }),
            output: None,
            is_error: status == "failed",
            status,
            subagent: None,
            children: Vec::new(),
        }),
        "mcptoolcall" | "dynamictoolcall" => Some(ChatRow::Tool {
            id,
            name: value_string(item, &["tool", "name"]).unwrap_or("tool").to_string(),
            input: item.get("arguments").cloned().unwrap_or(Value::Null),
            output: item.get("result").or_else(|| item.get("error")).map(|value| recorded_text(Some(value))),
            is_error: status == "failed" || item.get("error").is_some_and(|value| !value.is_null()),
            status,
            subagent: None,
            children: Vec::new(),
        }),
        "websearch" => Some(ChatRow::Tool {
            id,
            name: "WebSearch".to_string(),
            input: serde_json::json!({"query":item.get("query").cloned().unwrap_or(Value::Null)}),
            output: item.get("results").map(|value| recorded_text(Some(value))),
            is_error: false,
            status,
            subagent: None,
            children: Vec::new(),
        }),
        "contextcompaction" => Some(ChatRow::Compaction {
            id,
            status: "completed",
            trigger: Some("auto".to_string()),
            pre_tokens: None,
        }),
        // UserMessage is inherited context inside a child rollout, and collaboration calls are represented
        // by SubAgentActivity cards. Neither belongs as a free-standing child step.
        _ => None,
    }
}

fn recorded_command(value: Option<&Value>) -> Value {
    match value {
        Some(Value::Array(parts)) => Value::String(
            parts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "),
        ),
        Some(value) => value.clone(),
        None => Value::Null,
    }
}

/// Milliseconds since the epoch for an ISO timestamp the recording wrote down, or None when it has none
/// or the format is not one this understands. A missing time only costs the row its timestamp line.
pub(crate) fn parsed_at(timestamp: Option<&str>) -> Option<i64> {
    let raw = timestamp?.trim();
    // `2026-09-03T10:45:12.345Z`, the shape Claude writes. Parsed by hand: the crate has no date library,
    // and every other reader of these recordings passes the string straight through to the frontend.
    let (date, rest) = raw.split_once('T')?;
    let mut d = date.split('-');
    let (y, mo, da): (i64, i64, i64) = (
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
    );
    let time = rest.trim_end_matches('Z');
    let (clock, frac) = time.split_once('.').unwrap_or((time, "0"));
    let mut t = clock.split(':');
    let (h, mi, sec): (i64, i64, i64) = (
        t.next()?.parse().ok()?,
        t.next()?.parse().ok()?,
        t.next()?.parse().ok()?,
    );
    let ms: i64 = format!("{frac:0<3}")[..3].parse().unwrap_or(0);
    // Days from the civil calendar to the epoch (Howard Hinnant's algorithm).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + da - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400 + h * 3_600 + mi * 60 + sec) * 1_000) + ms)
}

fn to_row(mut ev: ChatEvent, subagent: Option<CodexSubagentReplay>) -> ChatRow {
    if let Some(mut shell) = ev.shell.take() {
        if let (ChatRow::Shell { source_text, .. }, Some(text)) = (&mut shell, ev.text.take()) { *source_text = text; }
        return shell;
    }
    let id = replay_id(ev.index);
    let at = parsed_at(ev.timestamp.as_deref());
    match ev.kind {
        // A replayed message keeps no pictures. The recording stores each image as a whole file, and the
        // parser already writes `[image]` into the text where one stood, which is enough to show that
        // something was attached without reading megabytes back off disk for a conversation being reopened.
        "user" => ChatRow::User {
            id,
            text: ev.text.unwrap_or_default(),
            images: Vec::new(),
            at,
        },
        "thinking" => ChatRow::Reasoning {
            id,
            text: ev.text.unwrap_or_default(),
            streaming: false,
        },
        "command" => ChatRow::Command { id, text: ev.text.unwrap_or_default() },
        // A tool call whose result never arrived stays running: the recording ends mid-call, which is
        // exactly what an interrupted session looks like.
        "tool" => match subagent {
            Some(task) => ChatRow::Tool {
                id,
                name: "Task".to_string(),
                input: serde_json::json!({
                    "description":task.description,
                    "agentThreadId":task.thread_id
                }),
                output: None,
                is_error: task.status == "failed",
                status: task.status,
                subagent: Some(SubagentInfo {
                    title: Some("Codex subagent".to_string()),
                    description: task.description,
                    model: task.model,
                    total_tokens: task.total_tokens,
                    tool_uses: None,
                    duration_ms: None,
                }),
                children: task.children,
            },
            None => ChatRow::Tool {
                id,
                name: ev.tool.unwrap_or_else(|| "tool".to_string()),
                input: ev.input.unwrap_or(Value::Null),
                output: ev.output,
                is_error: ev.is_error,
                status: match (ev.pending, ev.is_error) {
                    (true, _) => "running",
                    (false, true) => "failed",
                    (false, false) => "completed",
                },
                subagent: None,
                children: Vec::new(),
            },
        },
        // `assistant`, and anything a future parser adds: prose from the agent is the sane default.
        _ => ChatRow::Assistant {
            at,
            id,
            text: ev.text.unwrap_or_default(),
            streaming: false,
            model: None,
            duration_ms: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claude_replay_in_a_directory_reads_only_that_directory_s_recording() {
        let temp = std::env::temp_dir().join(format!("vlx-replay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).unwrap();
        let temp = temp.canonicalize().unwrap();
        let root = temp.join("projects");
        let project = temp.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let line = |cwd: &Path, text: &str| {
            serde_json::json!({"type":"user","uuid":"u1","cwd":cwd,"message":{"role":"user","content":text}}).to_string()
        };
        let folder = root.join(super::super::conversations::project_key(&project.to_string_lossy()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("conv-1.jsonl"), line(&project, "legit") + "\n").unwrap();
        // A folder that comes first below the root holds the same name with another directory's words.
        std::fs::create_dir_all(root.join("-A")).unwrap();
        std::fs::write(root.join("-A").join("conv-1.jsonl"), line(&temp.join("other"), "FOREIGN") + "\n").unwrap();
        let (path, mut file) = claude_recording(Some(&root), "conv-1", Some(&project)).unwrap();
        let content = super::super::conversations::read_whole(&mut file).unwrap();
        assert_eq!(path, folder.join("conv-1.jsonl"));
        assert!(content.contains("legit") && !content.contains("FOREIGN"));
        // Gone from its own folder, it is gone: no other folder and no missing root stands in.
        std::fs::remove_file(folder.join("conv-1.jsonl")).unwrap();
        assert!(claude_recording(Some(&root), "conv-1", Some(&project)).is_err());
        assert!(claude_recording(None, "conv-1", Some(&project)).is_err());
        let _ = std::fs::remove_dir_all(&temp);
    }

    /// A small recording that links the same subagent many times does not make the replay read it without
    /// bound: once the children's bound is spent, later cards say so instead of reading more.
    #[test]
    fn a_claude_replay_reads_all_its_children_within_one_bound() {
        let temp = std::env::temp_dir().join(format!("vlx-replay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).unwrap();
        let temp = temp.canonicalize().unwrap();
        let root = temp.join("projects");
        let project = temp.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let folder = root.join(super::super::conversations::project_key(&project.to_string_lossy()));
        std::fs::create_dir_all(&folder).unwrap();
        let first = serde_json::json!({"type":"user","uuid":"u0","cwd":project,"message":{"role":"user","content":"start"}}).to_string();
        let tasks = (0..5).map(|n| claude_task_record(&format!("task-{n}"), "c1")).collect::<Vec<_>>().join("\n");
        std::fs::write(folder.join("conv-1.jsonl"), format!("{first}\n{tasks}\n")).unwrap();
        let child = folder.join("conv-1").join("subagents").join("agent-c1.jsonl");
        std::fs::create_dir_all(child.parent().unwrap()).unwrap();
        let text = serde_json::json!({"type":"assistant","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"child says hi"}]}}).to_string();
        std::fs::write(&child, text + "\n").unwrap();
        let size = std::fs::metadata(&child).unwrap().len();

        let (path, mut file) = claude_recording(Some(&root), "conv-1", Some(&project)).unwrap();
        let rows = claude_checked_replay(&path, &mut file, size * 2 + size / 2).unwrap();
        let cards: Vec<&Vec<ChatRow>> = rows.iter().filter_map(|row| match row {
            ChatRow::Tool { children, subagent: Some(_), .. } => Some(children),
            _ => None,
        }).collect();
        assert_eq!(cards.len(), 5);
        let exhausted = |children: &Vec<ChatRow>| matches!(children.as_slice(),
            [ChatRow::Notice { message, .. }] if message == "Subagent history exceeds the replay limit");
        assert_eq!(cards.iter().filter(|children| !children.is_empty() && !exhausted(children)).count(), 2);
        assert_eq!(cards.iter().filter(|children| exhausted(children)).count(), 3);
        let _ = std::fs::remove_dir_all(&temp);
    }

    fn claude(lines: &[&str]) -> Vec<ChatEvent> {
        fold(claude_events(&lines.join("\n")))
    }

    fn claude_task_record(tool: &str, agent: &str) -> String {
        [
            serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":tool,"name":"Agent","input":{"description":"Inspect"}}]}}),
            serde_json::json!({"type":"user","toolUseResult":{"agentId":agent,"totalTokens":42,"totalToolUseCount":1,"totalDurationMs":100},"message":{"content":[{"type":"tool_result","tool_use_id":tool,"content":"summary"}]}}),
        ].iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn claude_replay_keeps_long_root_and_child_histories() {
        let content = (0..1003).map(|i| serde_json::json!({
            "type":"user", "message":{"content":format!("message {i}")}
        }).to_string()).collect::<Vec<_>>().join("\n");
        let mut reader = |_: &str| Ok(content.clone());
        let root = claude_replay(&content, &mut reader, &mut HashSet::new(), 0);
        let parent = claude_replay(&claude_task_record("task", "child"), &mut reader, &mut HashSet::new(), 0);
        let ChatRow::Tool { children, .. } = &parent[0] else { panic!("missing task") };
        assert_ne!(parent[0].id(), children[0].id(), "deferred child details must not resolve to the parent");
        for rows in [&root, children] {
            assert_eq!(rows.len(), 1003);
            assert!(matches!(&rows[0], ChatRow::User { text, .. } if text == "message 0"));
            assert!(matches!(&rows[1002], ChatRow::User { text, .. } if text == "message 1002"));
        }
    }

    #[test]
    fn codex_replay_keeps_every_step_of_a_long_child() {
        let content = (0..503).map(|i| serde_json::json!({
            "type":"event_msg", "payload":{"type":"item_completed", "item":{
                "type":"AgentMessage", "id":format!("step-{i}"),
                "content":[{"type":"Text","text":format!("step {i}")}]
            }}
        }).to_string()).collect::<Vec<_>>().join("\n");
        let (rows, _, _) = recorded_codex_thread_rows(
            "child", &mut |_| Ok(content.clone()), &mut HashSet::new(), 0,
        ).unwrap();
        assert_eq!(rows.len(), 503);
        for (index, row) in rows.iter().enumerate() {
            assert!(matches!(row, ChatRow::Assistant { text, .. } if text == &format!("step {index}")));
        }
    }

    #[test]
    fn claude_replay_restores_nested_children_and_keeps_them_out_of_the_main_timeline() {
        let root = claude_task_record("root-tool", "child");
        let child = [
            r#"{"type":"user","isSidechain":true,"message":{"content":"Inspect files"}}"#.to_string(),
            r#"{"type":"assistant","isSidechain":true,"message":{"model":"child-model","content":[{"type":"thinking","thinking":"reason"},{"type":"tool_use","id":"read","name":"Read","input":{"file_path":"a"}}]}}"#.to_string(),
            r#"{"type":"user","isSidechain":true,"message":{"content":[{"type":"tool_result","tool_use_id":"read","content":"file contents"}]}}"#.to_string(),
            claude_task_record("nested-tool", "nested"),
        ].join("\n");
        let nested = r#"{"type":"assistant","isSidechain":true,"message":{"content":[{"type":"text","text":"nested answer"}]}}"#;
        let mut requested = Vec::new();
        let rows = claude_replay(&root, &mut |id| {
            requested.push(id.to_string());
            match id { "child" => Ok(child.clone()), "nested" => Ok(nested.into()), _ => panic!("unrelated file requested") }
        }, &mut HashSet::new(), 0);
        assert_eq!(requested, ["child", "nested"]);
        assert_eq!(rows.len(), 1);
        let ChatRow::Tool { children, subagent: Some(facts), output, .. } = &rows[0] else { panic!("missing parent card") };
        assert_eq!(output.as_deref(), Some("summary"));
        assert_eq!(facts.total_tokens, Some(42));
        assert_eq!(facts.model.as_deref(), Some("child-model"));
        assert!(children.iter().any(|r| matches!(r, ChatRow::Reasoning { text, .. } if text == "reason")));
        assert!(children.iter().any(|r| matches!(r, ChatRow::Tool { name, output: Some(text), .. } if name == "Read" && text == "file contents")));
        assert!(children.iter().any(|r| matches!(r, ChatRow::Tool { name, children, .. } if name == "Agent" && matches!(&children[0], ChatRow::Assistant { text, .. } if text == "nested answer"))));
    }

    #[test]
    fn claude_replay_handles_missing_and_cyclic_children_without_losing_the_parent() {
        let root = claude_task_record("root", "child");
        for missing in [true, false] {
            let mut count = 0;
            let rows = claude_replay(&root, &mut |_| {
                count += 1;
                if missing { Err("Recording unavailable".into()) } else { Ok(root.clone()) }
            }, &mut HashSet::new(), 0);
            assert_eq!(count, 1, "cyclic references must not reread forever");
            let ChatRow::Tool { children, output, .. } = &rows[0] else { panic!("missing card") };
            assert_eq!(output.as_deref(), Some("summary"));
            if missing { assert!(matches!(&children[0], ChatRow::Notice { .. })); }
            else { assert!(matches!(&children[0], ChatRow::Tool { children, .. } if matches!(&children[0], ChatRow::Notice { .. }))); }
        }
    }

    #[test]
    fn native_pi_and_omp_shell_replay_retains_recorded_results() {
        for content in [include_str!("fixtures/pi-native-shell.jsonl"), include_str!("fixtures/omp-native-shell.jsonl")] {
            let entry = resume::pi_active_branch(content).into_iter()
                .find(|entry| entry.pointer("/message/role").and_then(Value::as_str) == Some("bashExecution")).unwrap();
            let message = &entry["message"];
            let events = fold(pi_events(content));
            assert_eq!(events.iter().map(|event| event.kind).collect::<Vec<_>>(), ["user", "user", "assistant"]);
            assert_eq!(events[0].index, 0);
            assert_eq!(events[1].index, 1);
            assert_eq!(events[0].native_id.as_deref(), entry["id"].as_str());
            let encoded = serde_json::to_value(&events[0]).unwrap();
            assert_eq!(encoded["shell"]["kind"], "shell");
            let rows = to_rows(events);
            let ChatRow::Shell { id, command, stdout, stderr, exit_code, status, source_text, at, .. } = &rows[0] else {
                panic!("native shell result was lost: {:?}", rows[0]);
            };
            assert_eq!(id, "h-0");
            assert_eq!(command, message["command"].as_str().unwrap());
            assert_eq!(stdout, message["output"].as_str().unwrap());
            assert!(stdout.contains("VLX_NATIVE_STDOUT_中文") && stdout.contains("VLX_NATIVE_STDERR_Ω"));
            assert!(stderr.is_empty(), "native recordings expose a combined output stream");
            assert_eq!((*exit_code, *status), (Some(7), "completed"));
            assert_eq!(source_text, &format!("!{command}"));
            assert_eq!(*at, parsed_at(entry["timestamp"].as_str()));
        }
    }

    #[test]
    fn native_pi_shell_flags_and_active_branch_come_from_recorded_fields() {
        for (exit, cancelled, truncated) in [(serde_json::Value::Null, false, false), (serde_json::json!(7), true, true)] {
            let output = "[Command cancelled]\n[Exit code: 0]\n<bash-stderr>literal</bash-stderr>";
            let first = serde_json::json!({"type":"message","id":"kept","parentId":null,
                "message":{"role":"bashExecution","command":"echo kept","output":output,
                    "exitCode":exit,"cancelled":cancelled,"truncated":truncated}});
            let abandoned = serde_json::json!({"type":"message","id":"abandoned","parentId":"kept",
                "message":{"role":"bashExecution","command":"echo discarded","output":"discarded","exitCode":0}});
            let last = serde_json::json!({"type":"message","id":"last","parentId":"kept",
                "message":{"role":"user","content":"ordinary text"}});
            let content = [first, abandoned, last].iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
            let rows = to_rows(fold(pi_events(&content)));
            assert_eq!(rows.len(), 2);
            let ChatRow::Shell { stdout, exit_code, status, stdout_truncated, stderr_truncated, .. } = &rows[0] else {
                panic!("expected recorded shell row");
            };
            assert_eq!(stdout, output);
            assert_eq!(*exit_code, if cancelled { Some(7) } else { None });
            assert_eq!(*status, if cancelled { "cancelled" } else { "completed" });
            assert_eq!((*stdout_truncated, *stderr_truncated), (truncated, false));
            assert!(matches!(&rows[1], ChatRow::User { id, text, .. } if id == "h-1" && text == "ordinary text"));
        }
    }

    /// The recording holds the shell-mode context as a plain user message (it is not on the injected
    /// prefix list, and must not be: the agent did receive it). Replay maps it back to the command row,
    /// for Claude and Codex alike, while the read-only view keeps the user event as it is.
    #[test]
    fn replay_maps_the_shell_context_message_back_to_a_command_row() {
        let tagged = super::super::shell::build_context(&super::super::shell::ShellContext {
            command: "git status".into(), stdout: "clean\n".into(), stderr: "warn".into(),
            exit_code: Some(3), cancelled: false, stdout_truncated: false, stderr_truncated: false, output_incomplete: false,
        });
        let recording = [
            r#"{"type":"user","uuid":"u-1","message":{"content":"hello"}}"#.to_string(),
            serde_json::json!({"type":"user","uuid":"u-2","message":{"content":tagged}}).to_string(),
        ].join("\n");
        let events = claude(&recording.lines().collect::<Vec<_>>());
        assert_eq!(events.iter().map(|ev| ev.kind).collect::<Vec<_>>(), vec!["user", "user"], "read() keeps the user event");
        assert!(matches!(&events[1].shell, Some(ChatRow::Shell { command, .. }) if command == "git status"));
        let encoded = serde_json::to_value(&events[1]).unwrap();
        assert_eq!(encoded["shell"]["kind"], "shell");
        assert_eq!(encoded["index"], 1);
        let mut rows = claude_replay(&recording, &mut |_| Err("none".into()), &mut HashSet::new(), 0);
        assert!(matches!(&rows[1], ChatRow::Shell { source_text, .. } if source_text == &tagged), "replay retains the exact provider source for rewind");
        super::super::shell::map_replayed_rows(&mut rows);
        assert!(matches!(&rows[0], ChatRow::User { text, .. } if text == "hello"));
        match &rows[1] {
            ChatRow::Shell { command, stdout, stderr, exit_code, status, .. } => {
                assert_eq!((command.as_str(), stdout.as_str(), stderr.as_str()), ("git status", "clean\n", "warn"));
                assert_eq!((*exit_code, *status), (Some(3), "completed"));
            }
            other => panic!("expected a shell row, got {other:?}"),
        }

        let codex = [
            r#"{"type":"turn_context","payload":{"turn_id":"turn-1"}}"#.to_string(),
            serde_json::json!({"type":"response_item","payload":{"type":"message","id":"msg-1","role":"user",
                "content":[{"type":"input_text","text":tagged}],
                "internal_chat_message_metadata_passthrough":{"content_item_kinds":["user.text"]}}}).to_string(),
        ].join("\n");
        let events: Vec<ChatEvent> = fold(codex_events(&codex)).into_iter().filter(codex_replay_event_visible).collect();
        let mut rows = to_rows(events);
        assert!(matches!(&rows[0], ChatRow::Shell { source_text, .. } if source_text == &tagged));
        super::super::shell::map_replayed_rows(&mut rows);
        assert!(matches!(&rows[0], ChatRow::Shell { command, exit_code, .. } if command == "git status" && *exit_code == Some(3)));
    }

    #[test]
    fn native_claude_shell_pairs_preserve_streams_indexes_and_both_rewind_ids() {
        // Captured from Claude Code 2.1.278 native ! mode, including its merged stderr and absent exit code.
        let input = "<bash-input>printf 'T4_native_stdout\\n'; printf 'T4_native_stderr\\n' >&2; exit 7</bash-input>";
        let output = "<bash-stdout></bash-stdout><bash-stderr>T4_native_stdout\nT4_native_stderr\n</bash-stderr>";
        let content = [
            serde_json::json!({"type":"user","uuid":"native-input","timestamp":"2026-09-20T08:29:02.146Z","message":{"content":input}}),
            serde_json::json!({"type":"user","uuid":"native-output","parentUuid":"native-input","message":{"content":output}}),
            serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":"Recorded response"}]}}),
        ].iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        let events = fold_claude(claude_events(&content));
        assert_eq!(events.iter().map(|event| event.index).collect::<Vec<_>>(), vec![0, 2]);
        let source = format!("{input}\n{output}");
        let rows = to_rows(events);
        assert!(matches!(&rows[0], ChatRow::Shell { stdout, stderr, exit_code: None, source_text, at: Some(_), .. }
            if stdout.is_empty() && stderr == "T4_native_stdout\nT4_native_stderr\n" && source_text == &source));
        let targets = claude_rewind_targets(&content);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].message_id, "native-input");
        assert_eq!(targets[0].last_message_id.as_deref(), Some("native-output"));
        assert_eq!(targets[0].text, source);
        let unrelated = content.replace("\"parentUuid\":\"native-input\"", "\"parentUuid\":\"someone-else\"");
        assert_eq!(claude_rewind_targets(&unrelated).len(), 2);
    }

    #[test]
    fn native_shell_status_literals_are_not_execution_metadata() {
        let command = "printf '</bash-input>\n<bash-stdout>'";
        let input = format!("<bash-input>{command}</bash-input>");
        for literal in ["Exit code 7", "Command cancelled by the user", "[VelaTerm: earlier output truncated]\ntail",
            "[VelaTerm: output capture ended before all streams closed]\nraw"] {
            let output = format!("<bash-stdout>{literal}</bash-stdout><bash-stderr>{literal}</bash-stderr>");
            let content = [
                serde_json::json!({"type":"user","uuid":"input","message":{"content":input}}),
                serde_json::json!({"type":"user","uuid":"output","parentUuid":"input","message":{"content":output}}),
            ].iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
            let events = fold_claude(claude_events(&content));
            assert_eq!(events.len(), 1);
            let row = events[0].shell.as_ref().unwrap();
            assert!(matches!(row, ChatRow::Shell { command: value, stdout, stderr, exit_code: None, status: "completed",
                stdout_truncated: false, stderr_truncated: false, output_incomplete: false, .. }
                if value == command && stdout == literal && stderr == literal));
            let targets = claude_rewind_targets(&content);
            assert_eq!(targets[0].message_id, "input");
            assert_eq!(targets[0].last_message_id.as_deref(), Some("output"));
            assert_eq!(targets[0].text, format!("{input}\n{output}"));
        }
    }

    #[test]
    fn ambiguous_native_shell_output_keeps_both_original_records_and_ids() {
        let input = "<bash-input>source</bash-input>";
        let output = "<bash-stdout>out</bash-stdout><bash-stderr>code </bash-stdout><bash-stderr> sample</bash-stderr>";
        let content = [
            serde_json::json!({"type":"user","uuid":"input","message":{"content":input}}),
            serde_json::json!({"type":"user","uuid":"output","parentUuid":"input","message":{"content":output}}),
        ].iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        let events = fold_claude(claude_events(&content));
        assert_eq!(events.len(), 2);
        for (event, original) in events.iter().zip([input, output]) {
            assert!(event.shell.is_none());
            assert_eq!(event.text.as_deref(), Some(original));
        }
        let rows = to_rows(events);
        assert!(matches!(&rows[1], ChatRow::User { text, .. } if text == output));
        let targets = claude_rewind_targets(&content);
        assert_eq!(targets.iter().map(|target| target.message_id.as_str()).collect::<Vec<_>>(), ["input", "output"]);
        assert!(targets.iter().all(|target| target.last_message_id.is_none()));
    }

    #[cfg(unix)]
    #[test]
    fn actual_shell_output_survives_saved_provider_records_readonly_replay_and_rewind() {
        use std::sync::{Arc, mpsc};
        use super::super::shell;
        let dir = std::env::temp_dir().join(format!("vlx-shell-record-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let host = Arc::new(crate::host::HeadlessHost::new(dir.clone(), crate::db::Db::open(&dir.join("test.db")).unwrap()));
        host.set_hooks(crate::agent::server::HookServer { port: 0, token: "fixture".into() });
        let app = crate::host::AppCtx::Headless(host);
        let command = "printf '%s' '</bash-input>\n<bash-stdout>€'; printf '%s' 'Exit code 7\nCommand cancelled by the user\n[VelaTerm: earlier output truncated]\n[VelaTerm: output capture ended before all streams closed]\n</bash-stdout><bash-stderr>' >&2";
        let (tx, rx) = mpsc::channel();
        let run = shell::spawn_run(&app, "fixture", SessionKind::Claude, Some("/bin/sh"), Some(dir.to_str().unwrap()),
            "sh-roundtrip", command, |_| {}, move |_, context| { tx.send(context).unwrap(); }).unwrap();
        let context = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(context.exit_code, Some(0));
        assert!(!context.cancelled && !context.output_incomplete);
        assert_eq!(context.command, command);
        assert_eq!(context.stdout, "</bash-input>\n<bash-stdout>€");
        assert_eq!(context.stderr, "Exit code 7\nCommand cancelled by the user\n[VelaTerm: earlier output truncated]\n[VelaTerm: output capture ended before all streams closed]\n</bash-stdout><bash-stderr>");
        let source = shell::build_context(&context);
        for kind in [SessionKind::Claude, SessionKind::Codex] {
            // These isolated files use observed provider record shapes, not a model or a real user's store.
            let record = if kind == SessionKind::Claude {
                serde_json::json!({"type":"user","uuid":"native-message","message":{"content":source}}).to_string()
            } else {
                [serde_json::json!({"type":"turn_context","payload":{"turn_id":"native-turn"}}),
                    serde_json::json!({"type":"response_item","payload":{"type":"message","id":"native-message","role":"user",
                        "content":[{"type":"input_text","text":source}],
                        "internal_chat_message_metadata_passthrough":{"content_item_kinds":["user.text"]}}})]
                    .iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
            };
            let path = dir.join(format!("{}.jsonl", kind.as_str()));
            std::fs::write(&path, record.as_bytes()).unwrap();
            let reopened = std::fs::read_to_string(&path).unwrap();
            let events = if kind == SessionKind::Claude { fold_claude(claude_events(&reopened)) }
                else { fold(codex_events(&reopened)).into_iter().filter(codex_replay_event_visible).collect() };
            assert_eq!(events.len(), 1);
            let readonly = serde_json::to_value(&events[0]).unwrap();
            assert_eq!(readonly["shell"]["command"], command);
            assert_eq!(readonly["shell"]["stdout"], context.stdout);
            assert_eq!(readonly["shell"]["stderr"], context.stderr);
            assert_eq!(readonly["shell"]["exitCode"], 0);
            assert_eq!(readonly["shell"]["status"], "completed");
            let rows = if kind == SessionKind::Claude { claude_replay(&reopened, &mut |_| Err("none".into()), &mut HashSet::new(), 0) }
                else { to_rows(events) };
            assert!(matches!(&rows[0], ChatRow::Shell { source_text, command: value, stdout, stderr, exit_code: Some(0), .. }
                if source_text == &source && value == command && stdout == &context.stdout && stderr == &context.stderr));
            let targets = if kind == SessionKind::Claude { claude_rewind_targets(&reopened) } else { codex_rewind_targets(&reopened) };
            assert_eq!(targets.len(), 1);
            assert_eq!(targets[0].message_id, "native-message");
            assert_eq!(targets[0].text, source);
            assert_eq!(targets[0].turn_id.as_deref(), if kind == SessionKind::Codex { Some("native-turn") } else { None });
            assert_eq!(std::fs::read_to_string(&path).unwrap(), record, "readers must leave the source unchanged");
        }
        drop(run);
        drop(app);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rewind_targets_keep_only_genuine_provider_user_ids() {
        let claude = claude_rewind_targets(
            &[
                r#"{"type":"user","uuid":"u-real","message":{"content":"change it"}}"#,
                r#"{"type":"user","uuid":"u-tool","message":{"content":[{"type":"tool_result","tool_use_id":"t"}]}}"#,
                r#"{"type":"user","uuid":"u-task","message":{"content":"<task-notification>done</task-notification>"}}"#,
            ]
            .join("\n"),
        );
        assert_eq!(claude, vec![RewindTarget {
            message_id: "u-real".into(), last_message_id: None, turn_id: None, text: "change it".into(),
        }]);

        let codex = codex_rewind_targets(
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"injected"}],"internal_chat_message_metadata_passthrough":{"content_item_kinds":["agents_md.instructions"]}}}"#,
                r#"{"type":"turn_context","payload":{"turn_id":"turn-1"}}"#,
                r#"{"type":"response_item","payload":{"type":"message","id":"msg-1","role":"user","content":[{"type":"input_text","text":"real"}],"internal_chat_message_metadata_passthrough":{"content_item_kinds":["user.text"]}}}"#,
            ]
            .join("\n"),
        );
        assert_eq!(
            codex,
            vec![RewindTarget {
                message_id: "msg-1".into(), last_message_id: None, turn_id: Some("turn-1".into()), text: "real".into(),
            }]
        );
    }

    #[test]
    fn rewind_targets_keep_visible_text_for_safe_replay_alignment() {
        let targets = claude_rewind_targets(
            &[
                r#"{"type":"user","uuid":"u-command","message":{"content":"<command-name>/config</command-name>\n<command-args>theme=dark</command-args>"}}"#,
                r#"{"type":"user","uuid":"u-image","message":{"content":[{"type":"text","text":"inspect"},{"type":"image","source":{}}]}}"#,
            ]
            .join("\n"),
        );
        assert_eq!(targets[0].text, "/config theme=dark");
        assert_eq!(targets[1].text, "inspect");
    }

    #[test]
    fn adjacent_user_messages_stay_separate_rewind_boundaries() {
        let rows = claude(&[
            r#"{"type":"user","uuid":"u-1","message":{"content":"first"}}"#,
            r#"{"type":"user","uuid":"u-2","message":{"content":"steer"}}"#,
        ]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].text.as_deref(), Some("first"));
        assert_eq!(rows[1].text.as_deref(), Some("steer"));
    }

    /// What a locally answered slash command printed is kept, and the command's own echo is not.
    ///
    /// The recording writes both as system rows; only the output is worth a row, since the echo repeats
    /// the line already shown as the user's message.
    #[test]
    fn a_command_reads_back_as_what_was_typed_and_what_it_answered() {
        let rows = claude(&[
            r#"{"type":"system","subtype":"local_command","content":"<command-name>/model</command-name>\n<command-message>model</command-message>\n<command-args>opus</command-args>","timestamp":"2026-09-03T08:53:43.737Z"}"#,
            r#"{"type":"system","subtype":"local_command","content":"<local-command-stdout>Set model to opus</local-command-stdout>","timestamp":"2026-09-03T08:53:43.737Z"}"#,
        ]);
        assert_eq!(rows.len(), 2, "the command and its answer are two rows: {rows:?}");
        assert_eq!(rows[0].kind, "user");
        assert_eq!(rows[0].text.as_deref(), Some("/model opus"), "the arguments are kept");
        assert_eq!(rows[1].kind, "command");
        assert_eq!(rows[1].text.as_deref(), Some("Set model to opus"));
    }

    /// A tool call and its result become one card, and the card keeps the raw input fields.
    #[test]
    fn pairs_tool_call_with_result() {
        let rows = claude(&[
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tu_1","name":"Bash","input":{"command":"ls -a"}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_1","content":"a.rs"}]}}"#,
        ]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "tool");
        assert_eq!(rows[0].tool.as_deref(), Some("Bash"));
        assert_eq!(rows[0].output.as_deref(), Some("a.rs"));
        assert!(!rows[0].pending);
        assert_eq!(
            rows[0].input.as_ref().unwrap()["command"].as_str(),
            Some("ls -a")
        );
    }

    /// A call still waiting for its result stays pending so the card can show a spinner.
    #[test]
    fn unanswered_call_stays_pending() {
        let rows = claude(&[
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tu_1","name":"Read","input":{"file_path":"/x"}}]}}"#,
        ]);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].pending);
        assert!(rows[0].output.is_none());
    }

    /// An error result marks the card, and a result without its call still renders on its own.
    #[test]
    fn orphan_and_error_results() {
        let rows = claude(&[
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"gone","content":"boom","is_error":true}]}}"#,
        ]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "tool");
        assert!(rows[0].is_error);
        assert!(rows[0].tool.is_none());
        assert_eq!(rows[0].output.as_deref(), Some("boom"));
    }

    /// A recorded timestamp becomes a real instant; a missing or malformed one costs only the timestamp.
    #[test]
    fn recorded_timestamps_become_epoch_millis() {
        // 2026-09-03T02:45:12.345Z — checked against `date -u -j -f %Y-%m-%dT%H:%M:%S 2026-09-03T02:45:12 +%s`.
        assert_eq!(super::parsed_at(Some("2026-09-03T02:45:12.345Z")), Some(1788403512345));
        // The epoch itself, and a date before it.
        assert_eq!(super::parsed_at(Some("1970-01-01T00:00:00.000Z")), Some(0));
        assert_eq!(super::parsed_at(Some("1969-12-31T23:59:59.000Z")), Some(-1000));
        assert_eq!(super::parsed_at(None), None);
        assert_eq!(super::parsed_at(Some("")), None);
        assert_eq!(super::parsed_at(Some("yesterday")), None);
    }

    #[test]
    fn replayed_duration_uses_the_last_answer_before_each_user_boundary() {
        let mut rows = vec![
            ChatRow::User {
                id: "u1".into(), text: "one".into(), images: Vec::new(), at: Some(1_000),
            },
            ChatRow::Assistant {
                id: "a1".into(), text: "draft".into(), streaming: false, at: Some(2_000), model: None, duration_ms: None,
            },
            ChatRow::Assistant {
                id: "a2".into(), text: "final".into(), streaming: false, at: Some(4_000), model: None, duration_ms: None,
            },
            ChatRow::User {
                id: "u2".into(), text: "two".into(), images: Vec::new(), at: Some(6_000),
            },
            ChatRow::Assistant {
                id: "a3".into(), text: "second final".into(), streaming: false, at: Some(9_500), model: None, duration_ms: None,
            },
            ChatRow::User {
                id: "u3".into(), text: "unknown start".into(), images: Vec::new(), at: None,
            },
            ChatRow::Assistant {
                id: "a4".into(), text: "no duration".into(), streaming: false, at: Some(12_000), model: None, duration_ms: None,
            },
        ];

        attach_recorded_turn_durations(&mut rows);
        assert!(matches!(&rows[1], ChatRow::Assistant { duration_ms: None, .. }));
        assert!(matches!(&rows[2], ChatRow::Assistant { duration_ms: Some(3_000), .. }));
        assert!(matches!(&rows[4], ChatRow::Assistant { duration_ms: Some(3_500), .. }));
        assert!(matches!(&rows[6], ChatRow::Assistant { duration_ms: None, .. }));
    }

    /// Adjacent blocks of one answer join into a single bubble; a tool call in between splits them.
    #[test]
    fn joins_adjacent_messages_but_not_across_tools() {
        let rows = claude(&[
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"first"},{"type":"text","text":"second"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tu_1","name":"Bash","input":{}}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"after"}]}}"#,
        ]);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].text.as_deref(), Some("first\n\nsecond"));
        assert_eq!(rows[2].text.as_deref(), Some("after"));
        assert_eq!(rows[2].index, 2);
    }

    /// Thinking is a separate kind, so the view can style or fold it apart from the answer.
    #[test]
    fn thinking_is_its_own_kind() {
        let rows = claude(&[
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hmm"},{"type":"text","text":"answer"}]}}"#,
        ]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, "thinking");
        assert_eq!(rows[1].kind, "assistant");
    }

    /// Unsupported agents fail with a reason rather than returning an empty conversation.
    #[test]
    fn unsupported_kind_reports_reason() {
        let err = read(SessionKind::Copilot, "sid").unwrap_err();
        assert!(err.contains("copilot"), "unexpected message: {err}");
    }

    // ── Replay into a live timeline ──

    /// Every kind a recording holds becomes the timeline row that draws it, and the ids carry the prefix
    /// that keeps replayed rows apart from live ones.
    #[test]
    fn every_kind_maps_to_its_row() {
        let rows = to_rows(claude(&[
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hmm"},{"type":"text","text":"hello"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tu_1","name":"Bash","input":{"command":"ls"}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_1","content":"a.rs"}]}}"#,
        ]));
        assert_eq!(rows.len(), 4);
        match &rows[0] {
            ChatRow::User { id, text, .. } => {
                assert_eq!(id, "h-0");
                assert_eq!(text, "hi");
            }
            other => panic!("expected a user row, got {other:?}"),
        }
        assert!(matches!(&rows[1], ChatRow::Reasoning { id, streaming, .. } if id == "h-1" && !streaming));
        assert!(matches!(&rows[2], ChatRow::Assistant { id, text, streaming, .. } if id == "h-2" && text == "hello" && !streaming));
        match &rows[3] {
            ChatRow::Tool { id, name, input, output, status, is_error, .. } => {
                assert_eq!(id, "h-3");
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls");
                assert_eq!(output.as_deref(), Some("a.rs"));
                assert_eq!(*status, "completed");
                assert!(!is_error);
            }
            other => panic!("expected a tool row, got {other:?}"),
        }
    }

    /// A call the recording never answers stays running, and a failed one is marked failed.
    #[test]
    fn pending_and_failed_tools_keep_their_status() {
        let rows = to_rows(claude(&[
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tu_1","name":"Bash","input":{}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_1","content":"boom","is_error":true}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tu_2","name":"Read","input":{}}]}}"#,
        ]));
        assert_eq!(rows.len(), 2);
        assert!(matches!(&rows[0], ChatRow::Tool { status, is_error, .. } if *status == "failed" && *is_error));
        assert!(matches!(&rows[1], ChatRow::Tool { status, output, .. } if *status == "running" && output.is_none()));
    }

    /// Restart replay follows the child rollout and preserves a delegated grandchild as a nested Task.
    #[test]
    fn codex_replay_restores_child_details_tokens_and_nesting() {
        let root = [
            r#"{"type":"response_item","payload":{"type":"function_call","name":"spawn_agent","call_id":"spawn-1","arguments":"{}"}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call_output","call_id":"spawn-1","output":"started"}}"#,
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"SubAgentActivity","id":"spawn-1","kind":"started","agent_thread_id":"child-1","agent_path":"/root/reviewer"}}}"#,
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"SubAgentActivity","id":"done-1","kind":"completed","agent_thread_id":"child-1","agent_path":"/root/reviewer"}}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-1","arguments":"{}"}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call_output","call_id":"wait-1","output":"done"}}"#,
        ]
        .join("\n");
        let child = [
            r#"{"type":"event_msg","payload":{"type":"thread_settings_applied","thread_settings":{"model":"gpt-child"}}}"#,
            r#"{"timestamp":"2026-09-04T05:00:00.000Z","type":"event_msg","payload":{"type":"item_completed","item":{"type":"AgentMessage","id":"child-answer","content":[{"type":"Text","text":"reviewing"}]}}}"#,
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"CommandExecution","id":"child-command","command":["pwd"],"cwd":"file:///workspace","status":"completed","aggregated_output":"/workspace\n"}}}"#,
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"SubAgentActivity","id":"nested-task","kind":"started","agent_thread_id":"grandchild-1","agent_path":"/root/reviewer/checker"}}}"#,
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"SubAgentActivity","id":"nested-done","kind":"completed","agent_thread_id":"grandchild-1","agent_path":"/root/reviewer/checker"}}}"#,
            r#"{"type":"token_usage_record","payload":{"thread_token_usage":{"total_tokens":24500}}}"#,
        ]
        .join("\n");
        let grandchild = [
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"CommandExecution","id":"nested-command","command":["git","status"],"status":"completed","aggregated_output":"clean\n"}}}"#,
            r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"AgentMessage","id":"nested-answer","content":[{"type":"Text","text":"all clear"}]}}}"#,
            r#"{"type":"token_usage_record","payload":{"thread_token_usage":{"total_tokens":900}}}"#,
        ]
        .join("\n");
        let mut reader = |id: &str| match id {
            "child-1" => Ok(child.clone()),
            "grandchild-1" => Ok(grandchild.clone()),
            other => Err(format!("unexpected thread {other}")),
        };

        let events = fold(codex_events(&root))
            .into_iter()
            .filter(codex_replay_event_visible)
            .collect();
        let subagents = codex_subagent_replays(&root, &mut reader);
        let rows = to_rows_with_subagents(events, &subagents);
        assert_eq!(rows.len(), 1);
        match &rows[0] {
            ChatRow::Tool {
                name,
                status,
                subagent: Some(facts),
                children,
                ..
            } => {
                assert_eq!(name, "Task");
                assert_eq!(*status, "completed");
                assert_eq!(facts.description.as_deref(), Some("/root/reviewer"));
                assert_eq!(facts.model.as_deref(), Some("gpt-child"));
                assert_eq!(facts.total_tokens, Some(24_500));
                assert!(matches!(
                    &children[0],
                    ChatRow::Assistant { text, streaming: false, .. } if text == "reviewing"
                ));
                assert!(matches!(
                    &children[1],
                    ChatRow::Tool { name, input, output: Some(output), .. }
                        if name == "Bash"
                            && input["command"] == "pwd"
                            && output == "/workspace\n"
                ));
                match &children[2] {
                    ChatRow::Tool {
                        status,
                        subagent: Some(nested_facts),
                        children: nested_children,
                        ..
                    } => {
                        assert_eq!(*status, "completed");
                        assert_eq!(nested_facts.total_tokens, Some(900));
                        assert!(matches!(
                            &nested_children[0],
                            ChatRow::Tool { name, output: Some(output), .. }
                                if name == "Bash" && output == "clean\n"
                        ));
                        assert!(matches!(
                            &nested_children[1],
                            ChatRow::Assistant { text, .. } if text == "all clear"
                        ));
                    }
                    other => panic!("expected a nested Task, got {other:?}"),
                }
            }
            other => panic!("expected a restored Task card, got {other:?}"),
        }
    }

    /// The first row remains available beyond the former 400-row cap, with stable chronological ids.
    #[test]
    fn a_long_recording_keeps_every_row_for_paging() {
        let events: Vec<ChatEvent> = (0..1003)
            .map(|i| {
                let mut ev = ChatEvent::message("user", format!("m{i}"), None);
                ev.index = i;
                ev
            })
            .collect();
        let rows = to_rows(events);
        assert_eq!(rows.len(), 1003);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.id(), format!("h-{index}"));
            assert!(matches!(row, ChatRow::User { text, .. } if text == &format!("m{index}")));
        }
    }

    /// A recording that fits replays whole, with no notice in front of it.
    #[test]
    fn a_short_recording_replays_whole() {
        let rows = to_rows(claude(&[r#"{"type":"user","message":{"role":"user","content":"hi"}}"#]));
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], ChatRow::User { .. }));
    }
}
