//! DeepSeek Harness session logs: where they live, how to decode them, and the timeline they replay to.
//!
//! dsh keeps every session as an append-only event log at
//! `$DSH_HOME/sessions/<encoded cwd>/<session id>/session.v<N>.jsonl.zstd` (`$DSH_HOME` defaults to `~/.dsh`).
//! The first line is a header (`type: session`); each later line is one envelope `{type, seq, time, data}`.
//! A session can hold several generations side by side after a format migration; the highest is current.
//!
//! **Why the log and not ACP.** dsh's `session/resume` restores a conversation without replaying it, so the
//! log is the only record of what was said. It is also the richer one: it keeps reasoning, tool results and
//! turn boundaries the protocol never repeats.
//!
//! **Same ids as the live view.** dsh tags each ACP message chunk with the assistant message's own id and
//! each tool call with its call id, and the log keeps both. Rows built here use the ids the live mapper
//! derives from them (`acp-r-<message>`, `acp-a-<message>`, `acp-t-<call>`), so a conversation reopened while
//! its rows are on screen lands on the same rows.
//!
//! Only the conversation is shown: prompts the person wrote (`source.kind == "user"`), the model's
//! reasoning, answers and tool calls, tool results, failed turns and compaction. Runtime context, system
//! prompts, request headers and bookkeeping events are the harness talking to the model and stay hidden.
//!
//! Verified against the logs dsh 0.2.1-rc.2 and rc.3 write (session format v4).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::engine::ChatRow;
use crate::agent::transcript::TranscriptMessage;

/// Decompressed bytes read from one log at most. A log is mostly tool schemas and results; past this a
/// conversation is cut short rather than read without bound.
const MAX_LOG_BYTES: u64 = 256 * 1024 * 1024;

/// Event types the timeline needs; every other line is skipped without being parsed.
const WANTED: &[&str] = &[
    "user/message", "assistant/message", "tool/result", "turn/start", "turn/end",
    "compaction/start", "compaction/summary", "compaction/end",
];

/// The harness home: `$DSH_HOME`, else `~/.dsh`.
pub fn home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DSH_HOME").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    crate::host::home_dir().map(|home| home.join(".dsh"))
}

/// A session id is a directory name under the log root, so anything that could leave it is refused.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && !id.starts_with('.') && id.len() <= 200
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The generation a file name stores, if it is a session log: `session.jsonl` is v0, `session.vN.jsonl` is N.
fn generation(name: &str) -> Option<u32> {
    let stem = name.strip_suffix(".zstd").unwrap_or(name).strip_suffix(".jsonl")?;
    if stem == "session" { return Some(0); }
    stem.strip_prefix("session.v")?.parse().ok()
}

/// The current log in one session directory: the highest generation, compressed or not.
fn current_log(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            generation(&name).map(|generation| (generation, name.ends_with(".zstd"), entry.path()))
        })
        // Within one generation a plain file is the one being written; prefer it over a compressed copy.
        .max_by_key(|(generation, compressed, _)| (*generation, !*compressed))
        .map(|(_, _, path)| path)
}

/// Find a session's log under `root/sessions`. The directory above the session is an encoding of its
/// working directory, which VelaTerm does not reproduce; scanning one level is cheap and exact.
fn find_in(root: &Path, id: &str) -> Option<PathBuf> {
    if !valid_id(id) { return None; }
    for project in std::fs::read_dir(root.join("sessions")).ok()?.flatten() {
        let dir = project.path().join(id);
        if dir.is_dir() {
            if let Some(log) = current_log(&dir) { return Some(log); }
        }
    }
    None
}

pub fn find(id: &str) -> Option<PathBuf> {
    find_in(&home()?, id)
}

/// Whether a conversation is known to be gone: the log root is readable and holds no such session.
pub fn confirmed_missing(id: &str) -> bool {
    let Some(root) = home() else { return false };
    if !root.join("sessions").is_dir() { return false; }
    find_in(&root, id).is_none()
}

/// The log's events the timeline uses, in order.
///
/// A log being written by a running dsh ends in a frame that is not finished yet; reading stops at the
/// last complete line, which is what dsh itself has made durable.
pub fn read_events(path: &Path) -> Result<Vec<Value>, String> {
    let mut events = Vec::new();
    each_line(path, |line| {
        if unwanted(line) { return true; }
        let Ok(event) = serde_json::from_slice::<Value>(line) else { return true };
        if event.get("type").and_then(Value::as_str).is_some_and(|kind| WANTED.contains(&kind)) { events.push(event); }
        true
    })?;
    Ok(events)
}

/// Feed each complete line of a log, decoded, to `visit` until it returns false or the log ends.
fn each_line(path: &Path, mut visit: impl FnMut(&[u8]) -> bool) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|_| "source_missing: The DeepSeek conversation log could not be found".to_string())?;
    let reader: Box<dyn Read> = if path.extension().is_some_and(|ext| ext == "zstd") {
        Box::new(zstd::stream::read::Decoder::new(file).map_err(|error| format!("The DeepSeek conversation log could not be read: {error}"))?)
    } else {
        Box::new(file)
    };
    let mut reader = BufReader::new(reader.take(MAX_LOG_BYTES));
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(_) => {}
            // A truncated tail of an open log; what came before it is complete.
            Err(_) => break,
        }
        if line.last() != Some(&b'\n') { break; }
        if !visit(&line) { break; }
    }
    Ok(())
}

/// A conversation in the log root, as the session import lists it.
pub struct Listed {
    pub id: String,
    pub cwd: String,
    /// The latest `session/title`, else the first prompt the person wrote; empty when neither exists.
    pub title: String,
    /// The current log's last write, in milliseconds.
    pub updated_at: i64,
}

/// Every conversation under `root/sessions` whose header `cwd` satisfies `belongs`, with the number of logs that
/// could not be read. Subagent runs (`origin: subagent` or a nonzero `delegationDepth`) belong to the
/// conversation that delegated them and are left out.
pub fn list_in(root: &Path, belongs: impl Fn(&str) -> bool) -> Result<(Vec<Listed>, usize), String> {
    let projects = match std::fs::read_dir(root.join("sessions")) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(e) => return Err(format!("Cannot read history directory: {e}")),
    };
    let mut found = Vec::new();
    let mut failures = 0;
    for project in projects.flatten() {
        let Ok(sessions) = std::fs::read_dir(project.path()) else { continue };
        for session in sessions.flatten() {
            let id = session.file_name().to_string_lossy().into_owned();
            if !valid_id(&id) { continue; }
            let Some(log) = current_log(&session.path()) else { continue };
            match summarize(&log, &belongs) {
                Ok(Some(mut listed)) if listed.id == id => {
                    listed.updated_at = std::fs::metadata(&log).ok().and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64).unwrap_or(0);
                    found.push(listed);
                }
                Ok(_) => {}
                Err(_) => failures += 1,
            }
        }
    }
    Ok((found, failures))
}

/// The header and title of one log. Only the header, title events and the first prompt are parsed.
fn summarize(log: &Path, belongs: &impl Fn(&str) -> bool) -> Result<Option<Listed>, String> {
    let mut header: Option<Value> = None;
    let mut named = String::new();
    let mut prompt = String::new();
    each_line(log, |line| {
        // The first line is the header; a log for another directory or a subagent stops right there.
        if header.is_none() {
            let Ok(first) = serde_json::from_slice::<Value>(line) else { return false };
            let cwd = first.get("cwd").and_then(Value::as_str).unwrap_or_default();
            let delegated = first.get("origin").and_then(Value::as_str) == Some("subagent")
                || first.get("delegationDepth").and_then(Value::as_u64).unwrap_or(0) > 0;
            if first.get("type").and_then(Value::as_str) != Some("session") || delegated || cwd.is_empty() || !belongs(cwd) {
                return false;
            }
            header = Some(first);
            return true;
        }
        // dsh writes `type` first, so most lines are skipped unparsed; a line written otherwise is parsed and checked.
        let tagged = |kind: &[u8]| line.strip_prefix(br#"{"type":""#).and_then(|rest| rest.strip_prefix(kind))
            .is_some_and(|rest| rest.first() == Some(&b'"'));
        let wanted = tagged(b"session/title") || (prompt.is_empty() && tagged(b"user/message"));
        if line.starts_with(br#"{"type":""#) && !wanted { return true; }
        let Ok(event) = serde_json::from_slice::<Value>(line) else { return true };
        let data = event.get("data");
        match event.get("type").and_then(Value::as_str) {
            Some("session/title") => {
                named = data.and_then(|d| d.get("title")).and_then(Value::as_str).unwrap_or_default().trim().to_string();
            }
            Some("user/message") if prompt.is_empty()
                && data.and_then(|d| d.pointer("/source/kind")).and_then(Value::as_str) == Some("user") => {
                prompt = text_of(data.and_then(|d| d.get("content"))).trim().to_string();
            }
            _ => {}
        }
        true
    })?;
    let Some(header) = header else { return Ok(None) };
    Ok(Some(Listed {
        id: header.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
        cwd: header.get("cwd").and_then(Value::as_str).unwrap_or_default().to_string(),
        title: if named.is_empty() { prompt } else { named },
        updated_at: 0,
    }))
}

/// Whether a raw line is known to be an event the timeline skips, judged from a leading `"type"` field
/// without parsing the line. dsh writes `type` first; a line written otherwise is parsed and checked.
fn unwanted(line: &[u8]) -> bool {
    let head = &line[..line.len().min(48)];
    let Some(rest) = head.strip_prefix(br#"{"type":""#) else { return false };
    !WANTED.iter().any(|kind| rest.starts_with(kind.as_bytes()) && rest.get(kind.len()) == Some(&b'"'))
}

// ─────────────────────────── Tool cards ───────────────────────────

/// A dsh tool call as one of the view's known tool cards.
///
/// dsh's file and shell tools take the arguments the view's cards already read (`file_path`, `old_string`,
/// `command`…), so most calls only need the card's name. Shared with the live ACP mapper, which receives
/// the same tool names and arguments, so a call looks the same live and replayed.
pub fn tool_card(name: &str, args: &Value) -> (String, Value) {
    let mut input = if args.is_object() { args.clone() } else { json!({"input": args}) };
    let card = match name {
        "bash" | "pwsh" => "Bash",
        "read" => "Read",
        "write" => "Write",
        "edit" => "Edit",
        "grep" => "Grep",
        "glob" => "Glob",
        "web_fetch" => "WebFetch",
        "web_search" => {
            if let Some(queries) = args.get("queries").and_then(Value::as_array) {
                let text: Vec<&str> = queries.iter().filter_map(Value::as_str).collect();
                input["query"] = json!(text.join(" · "));
            }
            "WebSearch"
        }
        "todo_write" => "TodoWrite",
        "subagent" => "Task",
        other => return (other.to_string(), input),
    };
    (card.to_string(), input)
}

/// A call's arguments as the model wrote them: a JSON string, which a cut-off stream can leave unparsable.
pub fn parse_arguments(arguments: &Value) -> Value {
    match arguments {
        Value::String(text) => serde_json::from_str(text).unwrap_or_else(|_| json!({"input": text})),
        Value::Null => json!({}),
        other => other.clone(),
    }
}

// ─────────────────────────── Timeline ───────────────────────────

pub fn reasoning_id(message: &str) -> String { format!("acp-r-{message}") }
pub fn answer_id(message: &str) -> String { format!("acp-a-{message}") }
pub fn tool_id(call: &str) -> String { format!("acp-t-{call}") }

/// The text of a list of content blocks, text blocks only.
fn text_of(content: Option<&Value>) -> String {
    let mut out = String::new();
    for block in content.and_then(Value::as_array).into_iter().flatten() {
        if block.get("type").and_then(Value::as_str) != Some("text") { continue; }
        if let Some(text) = block.get("text").and_then(Value::as_str) { out.push_str(text); }
    }
    out
}

/// Rows in timeline order, addressed by id so a later event can update an earlier row.
#[derive(Default)]
struct Rows {
    rows: Vec<ChatRow>,
    at: HashMap<String, usize>,
}

impl Rows {
    fn place(&mut self, row: ChatRow) {
        match self.at.get(row.id()) {
            Some(&index) => self.rows[index] = row,
            None => { self.at.insert(row.id().to_string(), self.rows.len()); self.rows.push(row); }
        }
    }

    fn get_mut(&mut self, id: &str) -> Option<&mut ChatRow> {
        let index = *self.at.get(id)?;
        self.rows.get_mut(index)
    }
}

/// The timeline a sequence of log events replays to.
pub fn rows(events: &[Value]) -> Vec<ChatRow> {
    let mut out = Rows::default();
    // Tool calls of the open turn still waiting for a result.
    let mut running: Vec<String> = Vec::new();
    let mut turn_started: Option<i64> = None;
    let mut compaction: Option<String> = None;
    for event in events {
        let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
        let time = event.get("time").and_then(Value::as_i64);
        let seq = event.get("seq").and_then(Value::as_i64).unwrap_or(0);
        let data = event.get("data").cloned().unwrap_or(Value::Null);
        match kind {
            "user/message" => {
                if data.pointer("/source/kind").and_then(Value::as_str) != Some("user") { continue; }
                let text = text_of(data.get("content"));
                if text.is_empty() { continue; }
                let id = data.get("id").and_then(Value::as_str).map(|id| format!("dsh-u-{id}")).unwrap_or_else(|| format!("dsh-u{seq}"));
                out.place(ChatRow::User { id, text, images: Vec::new(), at: time });
            }
            "turn/start" => {
                turn_started = time;
            }
            "assistant/message" => {
                let message = data.get("message").cloned().unwrap_or(Value::Null);
                let message_id = message.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("seq{seq}"));
                let model = message.pointer("/source/model").and_then(Value::as_str).map(str::to_string);
                let (mut reasoning, mut answer) = (String::new(), String::new());
                let mut calls = Vec::new();
                for block in message.get("content").and_then(Value::as_array).into_iter().flatten() {
                    match block.get("type").and_then(Value::as_str) {
                        Some("reasoning") => reasoning.push_str(block.get("text").and_then(Value::as_str).unwrap_or("")),
                        Some("text") => answer.push_str(block.get("text").and_then(Value::as_str).unwrap_or("")),
                        Some("tool-call") => calls.push(block.clone()),
                        _ => {}
                    }
                }
                if !reasoning.trim().is_empty() {
                    out.place(ChatRow::Reasoning { id: reasoning_id(&message_id), text: reasoning, streaming: false });
                }
                if !answer.is_empty() {
                    out.place(ChatRow::Assistant { id: answer_id(&message_id), text: answer, streaming: false, model, at: time, duration_ms: None });
                }
                for call in calls {
                    let Some(call_id) = call.get("id").and_then(Value::as_str) else { continue };
                    let name = call.get("name").and_then(Value::as_str).unwrap_or("tool");
                    let (name, input) = tool_card(name, &parse_arguments(call.get("arguments").unwrap_or(&Value::Null)));
                    let id = tool_id(call_id);
                    running.push(id.clone());
                    out.place(ChatRow::Tool { id, name, input, output: None, is_error: false, status: "running", subagent: None, children: Vec::new() });
                }
            }
            "tool/result" => {
                let message = data.get("message").cloned().unwrap_or(Value::Null);
                let Some(call_id) = message.get("toolCallId").and_then(Value::as_str) else { continue };
                let id = tool_id(call_id);
                running.retain(|open| *open != id);
                let failed = message.get("isError").and_then(Value::as_bool).unwrap_or(false);
                let text = text_of(message.get("content"));
                let reason = data.pointer("/error/reason").and_then(Value::as_str).map(str::to_string);
                if let Some(ChatRow::Tool { output, is_error, status, .. }) = out.get_mut(&id) {
                    *output = Some(if text.is_empty() { reason.unwrap_or_default() } else { text });
                    *is_error = failed;
                    *status = if failed { "failed" } else { "completed" };
                }
            }
            "turn/end" => {
                let reason = data.pointer("/reason/kind").and_then(Value::as_str).unwrap_or("completed");
                let settle = if reason == "completed" { "completed" } else { "canceled" };
                for id in running.drain(..) {
                    if let Some(ChatRow::Tool { status, .. }) = out.get_mut(&id) { *status = settle; }
                }
                if let Some(message) = turn_failure(&data) {
                    out.place(ChatRow::Error { id: format!("dsh-e{seq}"), message });
                }
                if let (Some(start), Some(end)) = (turn_started.take(), time) {
                    if end >= start { super::acp_protocol::attach_duration(&mut out.rows, (end - start) as u64); }
                }
            }
            "compaction/start" => {
                let id = format!("dsh-c{seq}");
                let trigger = if data.get("sourceCommandId").is_some() { "manual" } else { "auto" };
                out.place(ChatRow::Compaction { id: id.clone(), status: "loading", trigger: Some(trigger.into()), pre_tokens: None });
                compaction = Some(id);
            }
            "compaction/summary" => {
                let tokens = data.get("shadowedTokenCount").and_then(Value::as_u64);
                if let Some(ChatRow::Compaction { pre_tokens, .. }) = compaction.clone().and_then(|id| out.get_mut(&id)) { *pre_tokens = tokens; }
            }
            "compaction/end" => {
                if let Some(id) = compaction.take() {
                    if data.get("error").is_some() {
                        // A failed compaction left the conversation as it was; the marker would claim otherwise.
                        if let Some(index) = out.at.remove(&id) { out.rows.remove(index); reindex(&mut out); }
                    } else if let Some(ChatRow::Compaction { status, .. }) = out.get_mut(&id) {
                        *status = "completed";
                    }
                }
            }
            _ => {}
        }
    }
    // A turn still open belongs to a process that is gone, or one still running; either way its calls are
    // not waiting on this view.
    for id in running {
        if let Some(ChatRow::Tool { status, .. }) = out.get_mut(&id) { *status = "canceled"; }
    }
    out.rows
}

fn reindex(rows: &mut Rows) {
    rows.at = rows.rows.iter().enumerate().map(|(index, row)| (row.id().to_string(), index)).collect();
}

/// The message a failed turn leaves behind. A stop the person asked for is not a failure.
fn turn_failure(data: &Value) -> Option<String> {
    match data.pointer("/reason/kind").and_then(Value::as_str)? {
        "error" => Some(data.pointer("/reason/error/message").and_then(Value::as_str)
            .map(|message| format!("The turn failed: {message}"))
            .unwrap_or_else(|| "The turn failed.".into())),
        "max-tokens" => Some("The response reached its output token limit.".into()),
        "blocked" => Some("The turn was blocked before it could start.".into()),
        _ => None,
    }
}

/// History rows of a DeepSeek conversation, read from its log.
pub fn replay(id: &str) -> Result<Vec<ChatRow>, String> {
    let path = find(id).ok_or("source_missing: The DeepSeek conversation log could not be found")?;
    let mut rows = rows(&read_events(&path)?);
    super::shell::map_replayed_rows(&mut rows);
    Ok(rows)
}

/// The conversation as plain messages, for transcript views and search.
pub fn transcript(id: &str) -> Result<Vec<TranscriptMessage>, String> {
    let path = find(id).ok_or("source_missing: The DeepSeek conversation log could not be found")?;
    let timestamp = |at: Option<i64>| at.map(|ms| crate::agent::usage::unix_to_rfc3339_utc((ms.max(0) / 1000) as u64));
    let mut out: Vec<TranscriptMessage> = Vec::new();
    for row in rows(&read_events(&path)?) {
        match row {
            ChatRow::User { text, at, .. } => out.push(TranscriptMessage { role: "user".into(), text, timestamp: timestamp(at), tools: Vec::new() }),
            ChatRow::Assistant { text, at, .. } => match out.last_mut() {
                Some(last) if last.role == "assistant" => {
                    if !last.text.is_empty() { last.text.push_str("\n\n"); }
                    last.text.push_str(&text);
                }
                _ => out.push(TranscriptMessage { role: "assistant".into(), text, timestamp: timestamp(at), tools: Vec::new() }),
            },
            ChatRow::Tool { name, .. } => {
                if out.last().is_none_or(|last| last.role != "assistant") {
                    out.push(TranscriptMessage { role: "assistant".into(), text: String::new(), timestamp: None, tools: Vec::new() });
                }
                let last = out.last_mut().expect("an assistant message was just ensured");
                if last.tools.last() != Some(&name) { last.tools.push(name); }
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The events of a real dsh 0.2.1-rc.2 turn (2026-10-25), trimmed to the fields the timeline reads.
    fn turn() -> Vec<Value> {
        vec![
            json!({"type":"permission/preset","seq":0,"time":10,"data":{"preset":"workspace-write"}}),
            json!({"type":"turn/start","seq":4,"time":1_000,"data":{"turn":1}}),
            json!({"type":"user/message","seq":8,"time":1_001,"data":{"content":[{"type":"text","text":"Create hello.txt"}],"source":{"kind":"user"},"role":"user","id":"u-1"}}),
            json!({"type":"user/message","seq":9,"time":1_002,"data":{"content":[{"type":"text","text":"Current runtime context."}],"source":{"kind":"runtime-context"},"role":"user"}}),
            json!({"type":"assistant/message","seq":14,"time":1_500,"data":{"turn":1,"step":1,"message":{"role":"assistant","id":"m1",
                "content":[{"type":"reasoning","text":""},{"type":"text","text":"I'll create it."},
                    {"type":"tool-call","id":"call_1","name":"write","arguments":"{\"file_path\": \"/w/hello.txt\", \"content\": \"hi\"}"}],
                "source":{"kind":"model","provider":"deepseek-official","model":"deepseek-v4-flash"}}}}),
            json!({"type":"tool/call","seq":15,"time":1_501,"data":{"callId":"call_1","name":"write","arguments":"{}"}}),
            json!({"type":"tool/result","seq":16,"time":1_520,"data":{"message":{"role":"tool","toolCallId":"call_1","content":[{"type":"text","text":"Created file"}],"isError":false}}}),
            json!({"type":"assistant/message","seq":20,"time":2_000,"data":{"message":{"role":"assistant","id":"m2",
                "content":[{"type":"reasoning","text":"Done."},{"type":"tool-call","id":"call_2","name":"bash","arguments":"{\"command\": \"ls -la\", \"description\": \"List files\"}"}]}}}),
            json!({"type":"tool/result","seq":24,"time":2_100,"data":{"message":{"role":"tool","toolCallId":"call_2","content":[{"type":"text","text":"denied"}],"isError":true},
                "error":{"name":"E","code":"SANDBOX","reason":"blocked"}}}),
            json!({"type":"assistant/message","seq":28,"time":2_900,"data":{"message":{"role":"assistant","id":"m3",
                "content":[{"type":"reasoning","text":"Answer briefly."},{"type":"text","text":"Created hello.txt."}],"source":{"model":"deepseek-v4-flash"}}}}),
            json!({"type":"turn/end","seq":30,"time":3_000,"data":{"turn":1,"reason":{"kind":"completed"}}}),
        ]
    }

    fn summary(rows: &[ChatRow]) -> Vec<String> {
        rows.iter().map(|row| match row {
            ChatRow::User { id, text, .. } => format!("{id} user {text}"),
            ChatRow::Assistant { id, text, duration_ms, .. } => format!("{id} answer {text} {duration_ms:?}"),
            ChatRow::Reasoning { id, text, .. } => format!("{id} reasoning {text}"),
            ChatRow::Tool { id, name, status, output, .. } => format!("{id} {name} {status} {}", output.clone().unwrap_or_default()),
            ChatRow::Error { id, message } => format!("{id} error {message}"),
            ChatRow::Compaction { id, status, pre_tokens, .. } => format!("{id} compaction {status} {pre_tokens:?}"),
            other => other.id().to_string(),
        }).collect()
    }

    #[test]
    fn a_turn_replays_to_the_rows_the_live_view_draws() {
        assert_eq!(summary(&rows(&turn())), vec![
            "dsh-u-u-1 user Create hello.txt",
            "acp-a-m1 answer I'll create it. None",
            "acp-t-call_1 Write completed Created file",
            "acp-r-m2 reasoning Done.",
            "acp-t-call_2 Bash failed denied",
            "acp-r-m3 reasoning Answer briefly.",
            "acp-a-m3 answer Created hello.txt. Some(2000)",
        ]);
        let replayed = rows(&turn());
        let ChatRow::Tool { input, .. } = &replayed[2] else { panic!() };
        assert_eq!((input["file_path"].as_str(), input["content"].as_str()), (Some("/w/hello.txt"), Some("hi")));
        let ChatRow::Assistant { model, at, .. } = &replayed[6] else { panic!() };
        assert_eq!((model.as_deref(), *at), (Some("deepseek-v4-flash"), Some(2_900)));
    }

    #[test]
    fn a_stopped_turn_cancels_its_open_calls_and_a_failed_one_says_why() {
        let mut events = turn();
        events.truncate(8);
        events.push(json!({"type":"turn/end","seq":25,"time":2_200,"data":{"reason":{"kind":"aborted","reason":{"kind":"user"}}}}));
        let replayed = summary(&rows(&events));
        assert_eq!(replayed.last().unwrap(), "acp-t-call_2 Bash canceled ");
        events.push(json!({"type":"turn/start","seq":26,"time":3_000,"data":{"turn":2}}));
        events.push(json!({"type":"turn/end","seq":27,"time":3_100,"data":{"reason":{"kind":"error","error":{"message":"rate limited","code":"RATE"}}}}));
        assert_eq!(summary(&rows(&events)).last().unwrap(), "dsh-e27 error The turn failed: rate limited");
    }

    #[test]
    fn compaction_is_one_marker_and_a_failed_one_is_dropped() {
        let events = vec![
            json!({"type":"compaction/start","seq":1,"data":{"compactionId":"c","turn":null,"sourceCommandId":"x"}}),
            json!({"type":"compaction/summary","seq":2,"data":{"shadowedTokenCount":9000}}),
            json!({"type":"compaction/end","seq":3,"data":{"compactionId":"c","turn":null}}),
            json!({"type":"compaction/start","seq":4,"data":{"compactionId":"d","turn":2}}),
            json!({"type":"compaction/end","seq":5,"data":{"compactionId":"d","turn":2,"error":"failed"}}),
        ];
        assert_eq!(summary(&rows(&events)), vec!["dsh-c1 compaction completed Some(9000)"]);
    }

    #[test]
    fn tool_names_map_to_the_view_cards() {
        assert_eq!(tool_card("bash", &json!({"command":"ls"})).0, "Bash");
        assert_eq!(tool_card("edit", &json!({"file_path":"/a","old_string":"x","new_string":"y"})).0, "Edit");
        assert_eq!(tool_card("subagent", &json!({"description":"d","prompt":"p"})).0, "Task");
        let (name, input) = tool_card("web_search", &json!({"queries":["rust","zstd"]}));
        assert_eq!((name.as_str(), input["query"].as_str()), ("WebSearch", Some("rust · zstd")));
        assert_eq!(tool_card("create_goal", &json!({"objective":"x"})).0, "create_goal");
        assert_eq!(parse_arguments(&json!("{\"a\": 1")), json!({"input":"{\"a\": 1"}));
    }

    #[test]
    fn logs_are_found_by_generation_and_ids_cannot_leave_the_root() {
        let root = std::env::temp_dir().join(format!("dsh-history-{}", uuid::Uuid::new_v4()));
        let dir = root.join("sessions").join("--w--").join("s-1");
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["session.jsonl.zstd", "session.v4.jsonl.zstd", "session.v12.jsonl.zstd", "session.lock"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        assert_eq!(find_in(&root, "s-1").unwrap(), dir.join("session.v12.jsonl.zstd"));
        assert!(find_in(&root, "missing").is_none());
        for bad in ["", "../s-1", ".hidden", "a/b"] { assert!(find_in(&root, bad).is_none()); }
        assert_eq!((generation("session.jsonl"), generation("session.v3.jsonl"), generation("session.lock")), (Some(0), Some(3), None));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_compressed_log_decodes_up_to_its_last_complete_line() {
        let root = std::env::temp_dir().join(format!("dsh-log-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut text = String::from("{\"type\":\"session\",\"version\":4,\"id\":\"s\"}\n");
        for event in turn() { text.push_str(&event.to_string()); text.push('\n'); }
        text.push_str("{\"type\":\"user/message\",\"seq\":99,\"data\":");
        let path = root.join("session.v4.jsonl.zstd");
        std::fs::write(&path, zstd::encode_all(text.as_bytes(), 3).unwrap()).unwrap();
        let events = read_events(&path).unwrap();
        // The permission preset and tool/call are skipped, and the unfinished last line is not read.
        assert_eq!(events.len(), 9);
        assert!(events.iter().all(|event| event["seq"] != 99));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_line_filter_skips_only_lines_it_can_name_from_the_leading_type_field() {
        assert!(!unwanted(br#"{"type":"turn/end","seq":1}"#));
        assert!(unwanted(br#"{"type":"turn/endless","seq":1}"#));
        assert!(unwanted(br#"{"type":"request/header","seq":1}"#));
        assert!(!unwanted(br#"{"seq":1,"type":"request/header"}"#));
    }

    #[test]
    fn listing_reads_headers_and_titles_and_leaves_out_subagents() {
        let root = std::env::temp_dir().join(format!("dsh-list-{}", uuid::Uuid::new_v4()));
        let log = |id: &str, lines: &[Value]| {
            let dir = root.join("sessions").join("--w--").join(id);
            std::fs::create_dir_all(&dir).unwrap();
            let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
            std::fs::write(dir.join("session.v4.jsonl.zstd"), zstd::encode_all(text.as_bytes(), 3).unwrap()).unwrap();
        };
        let user = |text: &str, kind: &str| json!({"type":"user/message","seq":1,"data":{"content":[{"type":"text","text":text}],"source":{"kind":kind}}});
        log("s-1", &[
            json!({"type":"session","version":4,"id":"s-1","createdAt":1,"cwd":"/w","isSeeded":false,"delegationDepth":0}),
            user("Runtime context", "runtime-context"),
            user("Create hello.txt", "user"),
            json!({"type":"session/title","seq":2,"data":{"title":"First"}}),
            json!({"type":"session/title","seq":3,"data":{"title":"Hello file"}}),
        ]);
        log("s-2", &[
            json!({"type":"session","version":4,"id":"s-2","createdAt":1,"cwd":"/w","isSeeded":false,"delegationDepth":0}),
            user("List files", "user"),
        ]);
        log("s-3", &[json!({"type":"session","version":4,"id":"s-3","createdAt":1,"cwd":"/w","isSeeded":false,"origin":"subagent","delegationDepth":1})]);
        log("s-4", &[json!({"type":"session","version":4,"id":"s-4","createdAt":1,"cwd":"/other","isSeeded":false,"delegationDepth":0})]);
        let (mut found, failures) = list_in(&root, |cwd| cwd == "/w").unwrap();
        found.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(failures, 0);
        assert_eq!(found.iter().map(|s| (s.id.as_str(), s.title.as_str())).collect::<Vec<_>>(),
            [("s-1", "Hello file"), ("s-2", "List files")]);
        assert!(list_in(&root.join("missing"), |_| true).unwrap().0.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
