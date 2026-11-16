//! Turn and context accounting for Google Antigravity (`agy`) conversations.
//!
//! agy appends one JSON line per step to
//! `~/.gemini/antigravity-cli/brain/<conversation>/.system_generated/logs/transcript_full.jsonl`.
//! `PLANNER_RESPONSE` rows carry the provider's usage: `input_tokens` (uncached input),
//! `cache_read_tokens` and `output_tokens`. The latest `USER_INPUT` row starts the current turn.
//! A tool call names the file it edits in `Path`, and the next success or error row reports its
//! outcome, so a failed edit never appears in the turn's changed files.
//!
//! The transcript records only when a step was created; the conversation database beside it records
//! when each response finished, so output speed is estimated from both. The transcript does not record
//! the model's context window either, but that database records the window the provider applied for
//! each request, so the limit is read from there and stays unknown when the record is missing. The
//! model's display name is read from the settings block agy inserts into each user message.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::agent::transcript::{AgentContextInfo, ChangedFile, TurnStats};

/// Antigravity tools whose `Path` argument names the file they change.
const EDIT_TOOLS: &[&str] = &["write_file", "edit_file"];

/// Step types that deliver a tool result, and so resolve the oldest pending call.
const RESULT_TYPES: &[&str] = &["GENERIC", "LIST_DIRECTORY", "VIEW_FILE", "CODE_ACTION"];

/// One provider-reported usage record. agy writes all fields together; a value missing from an
/// imported or older log stays unknown instead of turning into a zero count.
#[derive(Clone, Copy, Debug, Default)]
struct Usage {
    input: u64,
    cache_read: u64,
    output: u64,
}

impl Usage {
    fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            input: value.get("input_tokens")?.as_u64()?,
            output: value.get("output_tokens")?.as_u64()?,
            cache_read: value.get("cache_read_tokens").and_then(Value::as_u64).unwrap_or(0),
        })
    }

    /// Input as the panel defines it for every agent: uncached input plus cache reads.
    fn input_total(self) -> u64 {
        self.input.saturating_add(self.cache_read)
    }

    fn total(self) -> u64 {
        self.input_total().saturating_add(self.output)
    }

    fn add(&mut self, other: Self) {
        self.input = self.input.saturating_add(other.input);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.output = self.output.saturating_add(other.output);
    }
}

/// One model request in the current turn: the transcript step index, which is also the step store's
/// row id, and its reported output tokens.
#[derive(Clone, Copy, Debug)]
struct Request {
    step: u64,
    output: u64,
}

#[derive(Clone, Debug, Default)]
struct Summary {
    model: Option<String>,
    session: Option<Usage>,
    turn: Option<Usage>,
    /// Current-turn model requests, or None when one cannot be linked to its step-store row.
    requests: Option<Vec<Request>>,
    context_tokens: Option<u64>,
    tools: u32,
    files: Vec<String>,
}

/// The model agy says this conversation runs on, from the settings block it prepends to a user
/// message: "The user changed setting `Model Selection` from X to Y. No need to comment…". The
/// terminator is used instead of a period because display names contain one ("Gemini 3.8 Flash").
fn model_from(content: &str) -> Option<String> {
    let rest = content.split_once("`Model Selection` from ")?.1;
    let after = rest.split_once(" to ")?.1;
    let end = after.find(". No need to comment")?;
    let name = after[..end].trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// The file a write/edit call changes, when the call names one.
fn call_path(name: &str, args: Option<&Value>) -> Option<String> {
    if !EDIT_TOOLS.contains(&name) {
        return None;
    }
    let path = args?.get("Path")?.as_str()?.trim();
    (!path.is_empty()).then(|| path.to_string())
}

/// Account for one complete transcript. Rows are independent JSON lines, so an unreadable line is
/// skipped rather than failing the panel.
fn parse(text: &str) -> Summary {
    let mut summary = Summary::default();
    let mut pending: VecDeque<Option<String>> = VecDeque::new();
    let mut files: HashSet<String> = HashSet::new();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        match value.get("type").and_then(Value::as_str) {
            Some("USER_INPUT") => {
                summary.turn = None;
                summary.requests = Some(Vec::new());
                summary.tools = 0;
                pending.clear();
                files.clear();
                if let Some(model) = value.get("content").and_then(Value::as_str).and_then(model_from) {
                    summary.model = Some(model);
                }
            }
            Some("PLANNER_RESPONSE") => {
                if let Some(usage) = Usage::parse(&value) {
                    summary.session.get_or_insert_with(Usage::default).add(usage);
                    summary.turn.get_or_insert_with(Usage::default).add(usage);
                    summary.context_tokens = Some(usage.input_total());
                    // A request without its step index cannot be timed, so the whole turn's rate is
                    // withheld rather than computed from a subset.
                    match (summary.requests.as_mut(), value.get("step_index").and_then(Value::as_u64)) {
                        (Some(requests), Some(step)) => requests.push(Request { step, output: usage.output }),
                        (Some(_), None) => summary.requests = None,
                        (None, _) => {}
                    }
                }
                for call in value.get("tool_calls").and_then(Value::as_array).into_iter().flatten() {
                    summary.tools = summary.tools.saturating_add(1);
                    let name = call.get("name").and_then(Value::as_str).unwrap_or("");
                    pending.push_back(call_path(name, call.get("args")));
                }
            }
            Some("ERROR_MESSAGE") => {
                // The failed call produced no file.
                pending.pop_front();
            }
            Some(kind) if RESULT_TYPES.contains(&kind) => {
                if let Some(Some(path)) = pending.pop_front() {
                    files.insert(path);
                }
            }
            _ => {}
        }
    }
    let mut list: Vec<String> = files.into_iter().collect();
    list.sort();
    summary.files = list;
    summary
}

fn stats_from(summary: &Summary) -> TurnStats {
    let turn = summary.turn;
    // A just-submitted prompt has no numbers yet; hide the grid rather than printing dashes.
    let has_turn = turn.is_some() || summary.tools > 0;
    TurnStats {
        model: summary.model.clone(),
        tokens: turn.map(|usage| usage.output),
        input_tokens: turn.map(Usage::input_total),
        total_tokens: turn.map(Usage::total),
        cached_tokens: turn.filter(|usage| usage.cache_read > 0).map(|usage| usage.cache_read),
        cache_hit_percent: turn
            .filter(|usage| usage.input_total() > 0)
            .map(|usage| usage.cache_read as f64 * 100.0 / usage.input_total() as f64),
        session_tokens: summary.session.map(Usage::total),
        tools_used: has_turn.then_some(summary.tools),
        files_touched: has_turn.then_some(summary.files.len() as u32),
        changed_files: has_turn.then(|| {
            summary
                .files
                .iter()
                .map(|path| ChangedFile { path: path.clone(), additions: None, deletions: None })
                .collect()
        }),
        context_tokens: summary.context_tokens,
        // The transcript has no window; `turn_stats` reads it from the conversation database. An
        // unknown limit still hides the meter instead of showing a guessed percentage.
        context_limit: None,
        ..TurnStats::default()
    }
}

/// A response's start and completion as the step store records them.
#[derive(Clone, Copy, Debug)]
struct Timing {
    start: f64,
    end: f64,
}

/// Estimated output throughput over the current turn's completed responses.
///
/// The transcript records only each step's creation time; the conversation database records its
/// completion as well. The span between them matched the provider's own time-to-first-token plus
/// streaming time across every recorded request on this machine. Its metadata is an undocumented
/// protobuf, so only those two timestamps are read, and any unexpected shape yields no figure instead
/// of a wrong one. Tool time falls outside both stamps, matching the panel's definition for every agent.
fn turn_rate(id: &str, requests: &[Request]) -> Option<f64> {
    if requests.is_empty() {
        return None;
    }
    let path = crate::agent::antigravity::conversation_db_path(id)?;
    let conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    // Antigravity may be writing the same database while its session runs.
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    rate_from(&conn, requests)
}

fn rate_from(conn: &Connection, requests: &[Request]) -> Option<f64> {
    let mut statement = conn.prepare("SELECT metadata FROM steps WHERE idx = ?1").ok()?;
    let mut tokens = 0u64;
    let mut seconds = 0f64;
    for request in requests {
        let step = i64::try_from(request.step).ok()?;
        let metadata: Option<Vec<u8>> = statement.query_row([step], |row| row.get(0)).ok()?;
        let timing = step_timing(metadata.as_deref()?)?;
        if timing.end <= timing.start {
            return None;
        }
        tokens = tokens.saturating_add(request.output);
        seconds += timing.end - timing.start;
    }
    (tokens > 0 && seconds > 0.0).then(|| tokens as f64 / seconds)
}

/// Read a step's creation (field 1) and completion (field 7) timestamps.
fn step_timing(metadata: &[u8]) -> Option<Timing> {
    Some(Timing { start: timestamp(metadata, 1)?, end: timestamp(metadata, 7)? })
}

/// Read one length-delimited `google.protobuf.Timestamp`-shaped field from a message.
fn timestamp(message: &[u8], wanted: u64) -> Option<f64> {
    let mut cursor = 0;
    while cursor < message.len() {
        let key = varint(message, &mut cursor)?;
        match key & 7 {
            0 => { varint(message, &mut cursor)?; }
            1 => skip(message, &mut cursor, 8)?,
            2 => {
                let len = varint(message, &mut cursor)? as usize;
                let end = cursor.checked_add(len)?;
                let body = message.get(cursor..end)?;
                cursor = end;
                if key >> 3 == wanted {
                    return timestamp_value(body);
                }
            }
            5 => skip(message, &mut cursor, 4)?,
            _ => return None,
        }
    }
    None
}

fn timestamp_value(message: &[u8]) -> Option<f64> {
    let mut cursor = 0;
    let (mut seconds, mut nanos) = (0i64, 0i64);
    while cursor < message.len() {
        let key = varint(message, &mut cursor)?;
        match key & 7 {
            0 => {
                let value = varint(message, &mut cursor)? as i64;
                match key >> 3 {
                    1 => seconds = value,
                    2 => nanos = value,
                    _ => {}
                }
            }
            1 => skip(message, &mut cursor, 8)?,
            2 => {
                let len = varint(message, &mut cursor)? as usize;
                cursor = cursor.checked_add(len)?;
                if cursor > message.len() {
                    return None;
                }
            }
            5 => skip(message, &mut cursor, 4)?,
            _ => return None,
        }
    }
    Some(seconds as f64 + nanos as f64 / 1_000_000_000.0)
}

fn skip(message: &[u8], cursor: &mut usize, len: usize) -> Option<()> {
    *cursor = cursor.checked_add(len)?;
    (*cursor <= message.len()).then_some(())
}

fn varint(message: &[u8], cursor: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    let mut shift = 0;
    while shift < 64 {
        let byte = *message.get(*cursor)?;
        *cursor += 1;
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
    }
    None
}

/// The last length-delimited field with this number, for records that repeat it per request.
fn last_bytes<'a>(message: &'a [u8], wanted: u64) -> Option<&'a [u8]> {
    let mut cursor = 0;
    let mut found = None;
    while cursor < message.len() {
        let key = varint(message, &mut cursor)?;
        match key & 7 {
            0 => { varint(message, &mut cursor)?; }
            1 => skip(message, &mut cursor, 8)?,
            2 => {
                let len = varint(message, &mut cursor)? as usize;
                let end = cursor.checked_add(len)?;
                let body = message.get(cursor..end)?;
                cursor = end;
                if key >> 3 == wanted {
                    found = Some(body);
                }
            }
            5 => skip(message, &mut cursor, 4)?,
            _ => return None,
        }
    }
    found
}

/// The last varint field with this number, for records that repeat it per request.
fn last_varint(message: &[u8], wanted: u64) -> Option<u64> {
    let mut cursor = 0;
    let mut found = None;
    while cursor < message.len() {
        let key = varint(message, &mut cursor)?;
        match key & 7 {
            0 => {
                let value = varint(message, &mut cursor)?;
                if key >> 3 == wanted {
                    found = Some(value);
                }
            }
            1 => skip(message, &mut cursor, 8)?,
            2 => {
                let len = varint(message, &mut cursor)? as usize;
                skip(message, &mut cursor, len)?;
            }
            5 => skip(message, &mut cursor, 4)?,
            _ => return None,
        }
    }
    found
}

/// The model's context window as the conversation database records it for the latest request.
///
/// The transcript lacks the window, but `gen_metadata` beside it stores the provider's context-window
/// metadata for each request, including the window it applied. That record is an undocumented
/// protobuf, so only the fields used here are read: an unexpected shape yields no limit rather than
/// a wrong meter.
fn context_limit(id: &str) -> Option<u64> {
    let path = crate::agent::antigravity::conversation_db_path(id)?;
    let conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    // Antigravity may be writing the same database while its session runs.
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    context_limit_from(&conn)
}

fn context_limit_from(conn: &Connection) -> Option<u64> {
    let data: Option<Vec<u8>> = conn
        .query_row("SELECT data FROM gen_metadata ORDER BY idx DESC LIMIT 1", [], |row| row.get(0))
        .ok()?;
    context_limit_in(data.as_deref()?)
}

/// Read `max_context_tokens` from the latest request's context-window metadata.
///
/// The database nests it as trajectory (1) → chat metadata (9) → context window (10) → limit (4).
/// The same message's estimated usage (1) must be present, so an unrelated field that happens to
/// carry the same number cannot pass for a window.
fn context_limit_in(data: &[u8]) -> Option<u64> {
    let trajectory = last_bytes(data, 1)?;
    let chat = last_bytes(trajectory, 9)?;
    let window = last_bytes(chat, 10)?;
    if last_varint(window, 1).is_none() {
        return None;
    }
    last_varint(window, 4).filter(|&limit| limit >= 1_000)
}

#[derive(Clone)]
struct Cached {
    path: PathBuf,
    modified: Option<SystemTime>,
    len: u64,
    summary: Summary,
}

static CACHE: OnceLock<Mutex<HashMap<String, Cached>>> = OnceLock::new();

fn summary(id: &str) -> Result<Summary, String> {
    let path = crate::agent::antigravity::transcript_path(id)
        .ok_or("Antigravity transcript file not found")?;
    let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
    let modified = metadata.modified().ok();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    // Bound retained accounting when many archived sessions are inspected.
    if cache.len() >= 16 && !cache.contains_key(id) {
        cache.clear();
    }
    if let Some(entry) = cache.get(id) {
        if entry.path == path && entry.modified == modified && entry.len == metadata.len() {
            return Ok(entry.summary.clone());
        }
    }
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let summary = parse(&text);
    cache.insert(
        id.to_string(),
        Cached { path, modified, len: metadata.len(), summary: summary.clone() },
    );
    Ok(summary)
}

/// Provider-reported usage and recorded activity for an Antigravity conversation's current turn.
pub fn turn_stats(id: &str) -> Result<TurnStats, String> {
    let summary = summary(id)?;
    let mut stats = stats_from(&summary);
    stats.generation_tokens_per_second = summary
        .requests
        .as_deref()
        .and_then(|requests| turn_rate(id, requests));
    stats.context_limit = context_limit(id);
    stats.context_percent = stats
        .context_tokens
        .zip(stats.context_limit)
        .map(|(used, limit)| used as f64 * 100.0 / limit as f64);
    Ok(stats)
}

/// Model and context occupancy for an Antigravity conversation whose process is not running.
pub fn context_info(id: &str) -> Result<AgentContextInfo, String> {
    let summary = summary(id)?;
    Ok(AgentContextInfo {
        model: summary.model,
        context_tokens: summary.context_tokens,
        context_limit: context_limit(id).unwrap_or(0),
        current_tool: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(value: Value) -> String {
        value.to_string()
    }

    fn usage(input: u64, output: u64, read: u64) -> Value {
        json!({"input_tokens":input,"output_tokens":output,"cache_read_tokens":read})
    }

    fn user(text: &str) -> Value {
        json!({"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","content":text})
    }

    fn planner(step: u64, usage_value: Value, calls: Value) -> Value {
        json!({"step_index":step,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE",
            "input_tokens":usage_value["input_tokens"],"output_tokens":usage_value["output_tokens"],
            "cache_read_tokens":usage_value["cache_read_tokens"],"tool_calls":calls})
    }

    fn encoded_varint(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                return out;
            }
        }
    }

    fn encoded_field(field: u64, body: &[u8]) -> Vec<u8> {
        let mut out = encoded_varint(field << 3 | 2);
        out.extend(encoded_varint(body.len() as u64));
        out.extend_from_slice(body);
        out
    }

    fn encoded_timestamp(seconds: u64, nanos: u64) -> Vec<u8> {
        let mut body = encoded_varint(1 << 3);
        body.extend(encoded_varint(seconds));
        body.extend(encoded_varint(2 << 3));
        body.extend(encoded_varint(nanos));
        body
    }

    fn encoded_metadata(start: (u64, u64), end: Option<(u64, u64)>) -> Vec<u8> {
        let mut out = encoded_field(1, &encoded_timestamp(start.0, start.1));
        if let Some((seconds, nanos)) = end {
            out.extend(encoded_field(7, &encoded_timestamp(seconds, nanos)));
        }
        out
    }

    fn step_store() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE steps (idx INTEGER PRIMARY KEY, metadata BLOB)").unwrap();
        conn
    }

    fn store_step(conn: &Connection, step: u64, metadata: Vec<u8>) {
        conn.execute(
            "INSERT INTO steps (idx, metadata) VALUES (?1, ?2)",
            rusqlite::params![step as i64, metadata],
        )
        .unwrap();
    }

    fn encoded_varint_field(field: u64, value: u64) -> Vec<u8> {
        let mut out = encoded_varint(field << 3);
        out.extend(encoded_varint(value));
        out
    }

    /// A request record shaped like `gen_metadata`: usage, then the context window it was given.
    fn encoded_context_record(used: u64, limit: u64) -> Vec<u8> {
        let mut window = encoded_varint_field(1, used);
        window.extend(encoded_field(3, b"\x08\x01"));
        window.extend(encoded_varint_field(4, limit));
        let mut chat = encoded_varint_field(2, 1);
        chat.extend(encoded_field(10, &window));
        encoded_field(1, &encoded_field(9, &chat))
    }

    fn metadata_store() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE gen_metadata (idx INTEGER PRIMARY KEY, data BLOB)").unwrap();
        conn
    }

    fn store_context_record(conn: &Connection, idx: u64, data: Vec<u8>) {
        conn.execute(
            "INSERT INTO gen_metadata (idx, data) VALUES (?1, ?2)",
            rusqlite::params![idx as i64, data],
        )
        .unwrap();
    }

    #[test]
    fn reads_the_context_window_the_database_recorded() {
        assert_eq!(context_limit_in(&encoded_context_record(218_670, 256_000)), Some(256_000));
    }

    #[test]
    fn the_latest_recorded_request_supplies_the_context_window() {
        let conn = metadata_store();
        store_context_record(&conn, 1, encoded_context_record(10, 200_000));
        store_context_record(&conn, 2, encoded_context_record(218_670, 256_000));
        assert_eq!(context_limit_from(&conn), Some(256_000));
    }

    #[test]
    fn an_unexpected_metadata_shape_hides_the_context_window() {
        let without_usage = encoded_field(1, &encoded_field(9, &encoded_field(10, &encoded_varint_field(4, 256_000))));
        assert_eq!(context_limit_in(&without_usage), None);
        assert_eq!(context_limit_in(&encoded_field(1, b"not a window")), None);
        let conn = metadata_store();
        store_context_record(&conn, 1, encoded_field(1, b"not a window"));
        assert_eq!(context_limit_from(&conn), None);
        assert_eq!(context_limit_from(&metadata_store()), None);
    }

    #[test]
    fn reads_the_creation_and_completion_timestamps() {
        let metadata = encoded_metadata((1_700_000_000, 250_000_000), Some((1_700_000_012, 500_000_000)));
        let timing = step_timing(&metadata).unwrap();
        assert!((timing.start - 1_700_000_000.25).abs() < 1e-9);
        assert!((timing.end - 1_700_000_012.5).abs() < 1e-9);
    }

    #[test]
    fn estimates_the_turn_rate_from_step_timestamps() {
        let conn = step_store();
        store_step(&conn, 1, encoded_metadata((100, 0), Some((110, 0))));
        store_step(&conn, 3, encoded_metadata((200, 0), Some((210, 0))));
        let requests = [Request { step: 1, output: 300 }, Request { step: 3, output: 200 }];
        let rate = rate_from(&conn, &requests).unwrap();
        assert!((rate - 25.0).abs() < 1e-9);
    }

    #[test]
    fn a_response_without_a_finished_response_hides_the_rate() {
        let conn = step_store();
        store_step(&conn, 1, encoded_metadata((100, 0), None));
        let requests = [Request { step: 1, output: 300 }];
        assert_eq!(rate_from(&conn, &requests), None);
    }

    #[test]
    fn a_response_with_an_impossible_span_hides_the_rate() {
        let conn = step_store();
        store_step(&conn, 1, encoded_metadata((200, 0), Some((100, 0))));
        let requests = [Request { step: 1, output: 300 }];
        assert_eq!(rate_from(&conn, &requests), None);
    }

    #[test]
    fn a_request_without_its_step_row_hides_the_rate() {
        let conn = step_store();
        store_step(&conn, 1, encoded_metadata((100, 0), Some((110, 0))));
        let requests = [Request { step: 1, output: 300 }, Request { step: 2, output: 100 }];
        assert_eq!(rate_from(&conn, &requests), None);
    }

    #[test]
    fn a_response_without_a_step_index_hides_the_rate() {
        let text = [
            row(user("<USER_REQUEST>\na\n</USER_REQUEST>")),
            row(json!({"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE",
                "input_tokens":10,"output_tokens":5,"cache_read_tokens":0})),
            row(json!({"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE",
                "input_tokens":20,"output_tokens":5,"cache_read_tokens":0})),
        ]
        .join("\n");
        assert!(parse(&text).requests.is_none());
    }

    #[test]
    fn reads_the_latest_turn_and_the_whole_session() {
        let settings = "<USER_REQUEST>\ngo\n</USER_REQUEST>\n<USER_SETTINGS_CHANGE>\nThe user changed setting `Model Selection` from None to Gemini 3.8 Flash (High). No need to comment on this change if the user doesn't ask about it.\n</USER_SETTINGS_CHANGE>";
        let text = [
            row(user(settings)),
            row(planner(1, usage(100, 10, 900), json!([{"name":"read_file","args":{"Path":"/w/a.txt"}}]))),
            row(json!({"step_index":2,"source":"MODEL","type":"GENERIC","status":"DONE","content":"ok"})),
            row(user("<USER_REQUEST>\nagain\n</USER_REQUEST>")),
            row(planner(3, usage(2000, 20, 4000), json!([
                {"name":"write_file","args":{"Path":"/w/new.txt","Content":"hi"}}
            ]))),
            row(json!({"step_index":4,"source":"MODEL","type":"GENERIC","status":"DONE","content":"written"})),
            row(planner(5, usage(300, 30, 4700), json!([
                {"name":"edit_file","args":{"Path":"/w/broken.txt","Target":"a","Replacement":"b"}}
            ]))),
            row(json!({"step_index":6,"source":"SYSTEM","type":"ERROR_MESSAGE","status":"DONE","content":"Error"})),
        ]
        .join("\n");
        let summary = parse(&text);
        assert_eq!(summary.model.as_deref(), Some("Gemini 3.8 Flash (High)"));
        // The current turn is everything after the second user message.
        assert_eq!(summary.turn.map(Usage::total), Some(6_020 + 5_030));
        assert_eq!(summary.session.map(Usage::total), Some(1_010 + 6_020 + 5_030));
        // Context is the last request's full input, cache reads included.
        assert_eq!(summary.context_tokens, Some(5_000));
        assert_eq!(summary.tools, 2);
        // A failed edit is not a changed file.
        assert_eq!(summary.files, vec!["/w/new.txt".to_string()]);
        // Only the current turn's requests are timed.
        let requests = summary.requests.as_ref().unwrap();
        assert_eq!(requests.iter().map(|r| r.step).collect::<Vec<_>>(), vec![3, 5]);
        assert_eq!(requests.iter().map(|r| r.output).sum::<u64>(), 50);
    }

    #[test]
    fn keeps_the_model_when_a_later_message_has_no_settings_block() {
        let text = [
            row(user("<USER_REQUEST>\na\n</USER_REQUEST>\n<USER_SETTINGS_CHANGE>\nThe user changed setting `Model Selection` from None to Claude Opus 5.5 (Medium). No need to comment on this change if the user doesn't ask about it.\n</USER_SETTINGS_CHANGE>")),
            row(planner(1, usage(10, 1, 0), json!([]))),
            row(user("<USER_REQUEST>\nb\n</USER_REQUEST>")),
            row(planner(3, usage(20, 2, 0), json!([]))),
        ]
        .join("\n");
        let summary = parse(&text);
        assert_eq!(summary.model.as_deref(), Some("Claude Opus 5.5 (Medium)"));
        assert_eq!(summary.turn.map(Usage::input_total), Some(20));
    }

    #[test]
    fn a_prompt_without_a_response_hides_the_turn_numbers() {
        let stats = stats_from(&parse(&row(user("<USER_REQUEST>\na\n</USER_REQUEST>"))));
        assert_eq!(stats.tools_used, None);
        assert_eq!(stats.files_touched, None);
        assert_eq!(stats.total_tokens, None);
    }

    #[test]
    fn missing_usage_does_not_become_a_zero_count() {
        let text = [
            row(user("<USER_REQUEST>\na\n</USER_REQUEST>")),
            row(json!({"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"RUNNING","tool_calls":[{"name":"run_command","args":{}}]})),
        ]
        .join("\n");
        let stats = stats_from(&parse(&text));
        assert_eq!(stats.tokens, None);
        assert_eq!(stats.total_tokens, None);
        assert_eq!(stats.session_tokens, None);
        assert_eq!(stats.tools_used, Some(1));
        assert_eq!(stats.files_touched, Some(0));
    }

    #[test]
    fn reads_the_cache_hit_share_and_ignores_an_unreadable_line() {
        let text = [
            row(json!({"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","content":"<USER_REQUEST>\na\n</USER_REQUEST>"})),
            "{not json".to_string(),
            row(planner(1, usage(250, 50, 750), json!([]))),
        ]
        .join("\n");
        let stats = stats_from(&parse(&text));
        assert_eq!(stats.context_tokens, Some(1_000));
        assert_eq!(stats.cache_hit_percent, Some(75.0));
        assert_eq!(stats.total_tokens, Some(1_050));
    }
}
