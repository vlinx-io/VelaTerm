//! Read-only access to OpenCode's own conversation store.
//!
//! OpenCode keeps sessions, messages, and message parts in one SQLite database rather than in flat files.
//! The chat engine reads it to replay a conversation before its server is up, the read-only session view
//! reads it for a terminal-driven session, and the Markdown export reads it for a finished conversation.
//! Nothing here writes: OpenCode owns the file, and this module only mirrors what it recorded.
//!
//! Every row stores its JSON body without the identifying columns, so the columns are folded back into the
//! value on the way out. Callers then see the same `Session`, `Message`, and `Part` shapes the HTTP API
//! returns, which is what lets one converter serve both the store and the live server.

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};

/// One message with its parts, in recording order.
#[derive(Clone, Debug)]
pub struct OpencodeMessage {
    pub info: Value,
    pub parts: Vec<Value>,
}

/// Where OpenCode keeps its data: `$XDG_DATA_HOME/opencode`, else `~/.local/share/opencode` — the same
/// answer on every platform, because OpenCode's own path resolution falls back to that directory when no
/// XDG variable is set.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        let base = PathBuf::from(xdg);
        if !base.as_os_str().is_empty() {
            return Some(base.join("opencode"));
        }
    }
    crate::host::home_dir().map(|home| home.join(".local").join("share").join("opencode"))
}

pub fn db_path() -> Option<PathBuf> {
    if let Some(path)=std::env::var_os("OPENCODE_DB").filter(|p|!p.is_empty()) { return Some(PathBuf::from(path)); }
    data_dir().map(|dir| dir.join("opencode.db"))
}

fn open() -> Result<Connection, String> {
    open_at(&db_path().ok_or("OpenCode's data directory could not be located")?)
}

fn open_at(path: &std::path::Path) -> Result<Connection, String> {
    if !path.exists() {
        return Err("OpenCode has not recorded any conversation on this machine".to_string());
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Failed to open OpenCode's store: {e}"))?;
    // OpenCode writes to the same file while it runs; wait briefly rather than fail on a busy lock.
    conn.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| format!("Failed to open OpenCode's store: {e}"))?;
    conn.execute_batch("PRAGMA cache_size=-8192; PRAGMA query_only=ON; BEGIN;").map_err(|e|e.to_string())?;
    Ok(conn)
}

fn parse_json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

pub(crate) fn has_table(conn: &Connection, name: &str) -> bool {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",[name],|row|row.get(0)).unwrap_or(false)
}

fn session_table(conn: &Connection, id: &str) -> Result<&'static str, String> {
    for table in ["session_v2","session"] {
        if !has_table(conn,table) {continue;}
        let exists: bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),[id],|r|r.get(0))
            .map_err(|e|format!("Failed to read OpenCode's store: {e}"))?;
        if exists {return Ok(table);}
    }
    Err(format!("OpenCode has no session {id}"))
}

/// A native-store lookup covers both generations without invoking a different installed CLI.
pub fn session_exists(id: &str) -> Result<bool, String> {
    let conn=open()?;
    match session_table(&conn,id) {
        Ok(_)=>Ok(true),
        Err(error) if error.starts_with("OpenCode has no session ")=>Ok(false),
        Err(error)=>Err(error),
    }
}

/// The native model and variant used when OpenCode resumes its terminal interface.
pub fn session_model(session_id: &str) -> Result<Option<Value>, String> {
    use rusqlite::OptionalExtension;
    let conn = open()?;
    let table=session_table(&conn,session_id)?;
    let raw: Option<(Option<u64>,Option<String>)> = conn.query_row(
        &format!("SELECT octet_length(model),CASE WHEN octet_length(model)<=65536 THEN model ELSE NULL END FROM {table} WHERE id = ?1"),
        [session_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|e| format!("Failed to read OpenCode session model: {e}"))?;
    if raw.as_ref().is_some_and(|(size,_)|size.is_some_and(|size|size>65536)) {return Err("unsupported_record: OpenCode model metadata exceeds its byte budget".into());}
    Ok(raw.and_then(|(_,raw)|raw).map(|text| parse_json(&text)))
}

/// Every message of a session with its parts, oldest first.
///
/// Messages that OpenCode has reverted are left out, the way its own client hides them: a revert is
/// staged on the session and cleaned up at the next prompt, and until then the rows still exist.
pub fn messages(session_id: &str) -> Result<Vec<OpencodeMessage>, String> {
    let mut remaining=super::history_store::limits::PAGE_BYTES;
    messages_bounded(session_id,&mut remaining)
}

/// Every message of a session in the database at `path`, without a byte budget: the history reader
/// holds one conversation at a time and reads its bodies back piece by piece.
pub(crate) fn messages_in(path:&std::path::Path,session_id:&str) -> Result<Vec<OpencodeMessage>,String> {
    let conn=open_at(path)?;let mut unbounded=usize::MAX;
    messages_from(&conn,session_id,&mut unbounded)
}

pub(crate) fn messages_bounded(session_id:&str,remaining:&mut usize) -> Result<Vec<OpencodeMessage>,String> {
    // `open` pins one read snapshot before the preflight, so an append cannot grow a selected column
    // between counting its original bytes and materializing a small compatibility response.
    let conn = open()?;
    messages_from(&conn,session_id,remaining)
}

/// What turn accounting reads, without materializing the conversation.
///
/// The Info panel needs token counts, the last user message, and the current turn's tool and file
/// effects. `messages` reads every message and part and refuses a recording past the paging budget,
/// so a long session leaves the panel with nothing; this reader selects only the fields those
/// numbers come from, and only the current turn's tool and patch parts.
pub fn stats_messages(session_id: &str) -> Result<Vec<OpencodeMessage>, String> {
    let conn = open()?;
    stats_messages_from(&conn, session_id)
}

fn stats_messages_from(conn: &Connection, session_id: &str) -> Result<Vec<OpencodeMessage>, String> {
    let table = session_table(conn, session_id)?;
    if has_table(conn, "session_message") && (table == "session_v2" || !has_table(conn, "message")) {
        return stats_v2(conn, session_id, table);
    }
    if !has_table(conn, "message") {
        return Ok(Vec::new());
    }
    stats_v1(conn, session_id)
}

/// One message row with only the columns turn accounting reads.
struct StatsMessageRow {
    id: String,
    /// `time_created` in the legacy store, `seq` in the v2 store: both give the recording order.
    order: i64,
    kind: Option<String>,
    parent: Option<String>,
    tokens_object: Option<String>,
    input: Option<u64>,
    output: Option<u64>,
    reasoning: Option<u64>,
    total: Option<u64>,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    time_created: Option<u64>,
    time_completed: Option<u64>,
}

fn read_stats_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<StatsMessageRow> {
    Ok(StatsMessageRow {
        id: row.get(0)?,
        order: row.get(1)?,
        kind: row.get(2)?,
        parent: row.get(3)?,
        tokens_object: row.get(4)?,
        input: row.get(5)?,
        output: row.get(6)?,
        reasoning: row.get(7)?,
        total: row.get(8)?,
        cache_read: row.get(9)?,
        cache_write: row.get(10)?,
        time_created: row.get(11)?,
        time_completed: row.get(12)?,
    })
}

/// One assistant step before the turn boundary is known.
struct StatsAssistant {
    id: String,
    order: i64,
    parent: Option<String>,
    tokens: Option<Value>,
    time: Option<Value>,
}

/// A tool or patch part reduced to what turn accounting reads.
struct StatsPart {
    message_id: String,
    id: String,
    kind: String,
    call_id: Option<String>,
    tool: Option<String>,
    status: Option<String>,
    file_path: Option<String>,
    content_lines: Option<u64>,
    additions: Option<u64>,
    deletions: Option<u64>,
    exists: Option<bool>,
    files: Option<String>,
}

fn read_stats_part(row: &rusqlite::Row<'_>) -> rusqlite::Result<StatsPart> {
    Ok(StatsPart {
        message_id: row.get(0)?,
        id: row.get(1)?,
        kind: row.get(2)?,
        call_id: row.get(3)?,
        tool: row.get(4)?,
        status: row.get(5)?,
        file_path: row.get(6)?,
        content_lines: row.get(7)?,
        additions: row.get(8)?,
        deletions: row.get(9)?,
        exists: row.get(10)?,
        files: row.get(11)?,
    })
}

/// The part shape `opencode_edit` and the turn loop consume, without any output body.
fn stats_part(part: StatsPart) -> Value {
    let mut value = serde_json::Map::new();
    value.insert("id".into(), json!(part.id));
    value.insert("type".into(), json!(part.kind));
    if let Some(call_id) = part.call_id {
        value.insert("callID".into(), json!(call_id));
    }
    if let Some(tool) = part.tool {
        value.insert("tool".into(), json!(tool));
    }
    if part.kind == "tool" {
        let mut state = serde_json::Map::new();
        if let Some(status) = part.status {
            state.insert("status".into(), json!(status));
        }
        let mut input = serde_json::Map::new();
        if let Some(path) = part.file_path {
            input.insert("filePath".into(), json!(path));
        }
        if let Some(lines) = part.content_lines {
            input.insert("contentLines".into(), json!(lines));
        }
        if !input.is_empty() {
            state.insert("input".into(), Value::Object(input));
        }
        let mut metadata = serde_json::Map::new();
        if part.additions.is_some() || part.deletions.is_some() {
            let mut diff = serde_json::Map::new();
            if let Some(additions) = part.additions {
                diff.insert("additions".into(), json!(additions));
            }
            if let Some(deletions) = part.deletions {
                diff.insert("deletions".into(), json!(deletions));
            }
            metadata.insert("filediff".into(), Value::Object(diff));
        }
        if let Some(exists) = part.exists {
            metadata.insert("exists".into(), json!(exists));
        }
        if !metadata.is_empty() {
            state.insert("metadata".into(), Value::Object(metadata));
        }
        value.insert("state".into(), Value::Object(state));
    }
    if let Some(files) = part.files {
        value.insert("files".into(), parse_json(&files));
    }
    Value::Object(value)
}

/// OpenCode's token object, reduced to the keys turn accounting reads. The v2 store records no
/// total, so the reader supplies the sum its own client computes.
fn stats_tokens(
    object: Option<&str>,
    input: Option<u64>,
    output: Option<u64>,
    reasoning: Option<u64>,
    total: Option<u64>,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    synthesize_total: bool,
) -> Option<Value> {
    if object != Some("object") {
        return None;
    }
    let mut tokens = serde_json::Map::new();
    if let Some(value) = input {
        tokens.insert("input".into(), json!(value));
    }
    if let Some(value) = output {
        tokens.insert("output".into(), json!(value));
    }
    if let Some(value) = reasoning {
        tokens.insert("reasoning".into(), json!(value));
    }
    let mut cache = serde_json::Map::new();
    if let Some(value) = cache_read {
        cache.insert("read".into(), json!(value));
    }
    if let Some(value) = cache_write {
        cache.insert("write".into(), json!(value));
    }
    if !cache.is_empty() {
        tokens.insert("cache".into(), Value::Object(cache));
    }
    let total = if synthesize_total {
        Some(
            input.unwrap_or(0)
                + output.unwrap_or(0)
                + reasoning.unwrap_or(0)
                + cache_read.unwrap_or(0)
                + cache_write.unwrap_or(0),
        )
    } else {
        total
    };
    if let Some(value) = total {
        tokens.insert("total".into(), json!(value));
    }
    Some(Value::Object(tokens))
}

fn stats_time(created: Option<u64>, completed: Option<u64>) -> Option<Value> {
    if created.is_none() && completed.is_none() {
        return None;
    }
    let mut time = serde_json::Map::new();
    if let Some(value) = created {
        time.insert("created".into(), json!(value));
    }
    if let Some(value) = completed {
        time.insert("completed".into(), json!(value));
    }
    Some(Value::Object(time))
}

/// Counts a written file's lines the way Rust's `str::lines` does, without reading the body.
/// Only a completed write that created its file records a countable line count.
fn content_lines(expr: &str, tool: &str) -> String {
    let content = format!("json_extract({expr},'$.state.input.content')");
    format!(
        "CASE WHEN json_extract({expr},'{tool}')='write' \
            AND json_extract({expr},'$.state.metadata.exists')=0 \
            AND json_type({expr},'$.state.input.content')='text' \
         THEN CASE WHEN {content}='' THEN 0 ELSE \
            length({content})-length(replace({content},char(10),'')) \
            +CASE WHEN substr({content},-1)=char(10) THEN 0 ELSE 1 END END END"
    )
}

/// The message a staged revert hides, matching the cut `messages_from` applies.
fn stats_revert(conn: &Connection, table: &str, session_id: &str) -> Result<Option<String>, String> {
    use rusqlite::OptionalExtension;
    let row = conn
        .query_row(
            &format!("SELECT octet_length(revert),CASE WHEN octet_length(revert)<=65536 THEN revert ELSE NULL END FROM {table} WHERE id = ?1"),
            [session_id],
            |row| Ok((row.get::<_, Option<u64>>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?
        .ok_or_else(|| format!("OpenCode has no session {session_id}"))?;
    if row.0.is_some_and(|size| size > 65536) {
        return Err("unsupported_record: OpenCode revert metadata exceeds its byte budget".into());
    }
    let value = row.1.map(|text| parse_json(&text)).unwrap_or(Value::Null);
    Ok(reverted_from(&value).map(str::to_string))
}

fn stats_v1(conn: &Connection, session_id: &str) -> Result<Vec<OpencodeMessage>, String> {
    use rusqlite::OptionalExtension;
    let cut = stats_revert(conn, "session", session_id)?;
    let cut_order = match cut.as_deref() {
        Some(id) => conn
            .query_row(
                "SELECT time_created,id FROM message WHERE session_id = ?1 AND id = ?2",
                [session_id, id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?,
        None => None,
    };
    let filter = if cut_order.is_some() {
        " AND (time_created < ?2 OR (time_created = ?2 AND id < ?3))"
    } else {
        ""
    };
    // Only the last user starts the current turn, and only its identity is read.
    let last_user: Option<(i64, String)> = {
        let mut statement = conn
            .prepare(&format!(
                "SELECT time_created,id FROM message WHERE session_id = ?1 \
                   AND json_extract(data,'$.role')='user'{filter} ORDER BY time_created DESC,id DESC LIMIT 1"
            ))
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let row = match &cut_order {
            Some((time, id)) => statement.query_row(rusqlite::params![session_id, time, id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            }),
            None => statement.query_row([session_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            }),
        };
        row.optional().map_err(|e| format!("Failed to read OpenCode's store: {e}"))?
    };
    // Session totals need every step's tokens, but a step row holds no conversation body.
    let mut statement = conn
        .prepare(&format!(
            "SELECT id,time_created,'assistant',json_extract(data,'$.parentID'),\
             json_type(data,'$.tokens'),json_extract(data,'$.tokens.input'),json_extract(data,'$.tokens.output'),\
             json_extract(data,'$.tokens.reasoning'),json_extract(data,'$.tokens.total'),\
             json_extract(data,'$.tokens.cache.read'),json_extract(data,'$.tokens.cache.write'),\
             json_extract(data,'$.time.created'),json_extract(data,'$.time.completed') \
             FROM message WHERE session_id = ?1 AND json_extract(data,'$.role')='assistant'{filter} \
             ORDER BY time_created,id"
        ))
        .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
    let rows = match &cut_order {
        Some((time, id)) => statement.query_map(rusqlite::params![session_id, time, id], read_stats_message),
        None => statement.query_map([session_id], read_stats_message),
    }
    .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
    let mut assistants: Vec<StatsAssistant> = Vec::new();
    for row in rows {
        let row = row.map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        assistants.push(StatsAssistant {
            id: row.id,
            order: row.order,
            parent: row.parent,
            tokens: stats_tokens(
                row.tokens_object.as_deref(),
                row.input,
                row.output,
                row.reasoning,
                row.total,
                row.cache_read,
                row.cache_write,
                false,
            ),
            time: stats_time(row.time_created, row.time_completed),
        });
    }
    let turn_id = last_user.as_ref().map(|(_, id)| id.clone());
    let mut messages = Vec::new();
    if let Some((_, id)) = &last_user {
        messages.push(OpencodeMessage { info: json!({"id": id, "role": "user"}), parts: Vec::new() });
    }
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut turn: Vec<String> = Vec::new();
    for assistant in assistants {
        // The legacy store links each step to its user message. Where an older row records no
        // parent, the position after the last user message decides, as it does for the v2 store.
        let parent = match &assistant.parent {
            Some(parent) => Some(parent.clone()),
            None => last_user.as_ref().map(|(order, id)| {
                if (assistant.order, assistant.id.as_str()) > (*order, id.as_str()) {
                    id.clone()
                } else {
                    String::new()
                }
            }),
        };
        let in_turn = match &turn_id {
            Some(id) => parent.as_deref() == Some(id.as_str()),
            None => true,
        };
        if in_turn {
            turn.push(assistant.id.clone());
        }
        let mut info = serde_json::Map::new();
        info.insert("id".into(), json!(&assistant.id));
        info.insert("role".into(), json!("assistant"));
        if let Some(parent) = parent {
            info.insert("parentID".into(), json!(parent));
        }
        if let Some(tokens) = assistant.tokens {
            info.insert("tokens".into(), tokens);
        }
        if let Some(time) = assistant.time {
            info.insert("time".into(), time);
        }
        index.insert(assistant.id.clone(), messages.len());
        messages.push(OpencodeMessage { info: Value::Object(info), parts: Vec::new() });
    }
    if turn.is_empty() {
        return Ok(messages);
    }
    let placeholders = std::iter::repeat("?").take(turn.len()).collect::<Vec<_>>().join(",");
    let mut statement = conn
        .prepare(&format!(
            "SELECT message_id,id,json_extract(data,'$.type'),json_extract(data,'$.callID'),json_extract(data,'$.tool'),\
             json_extract(data,'$.state.status'),json_extract(data,'$.state.input.filePath'),{},\
             json_extract(data,'$.state.metadata.filediff.additions'),\
             json_extract(data,'$.state.metadata.filediff.deletions'),\
             json_extract(data,'$.state.metadata.exists'),json_extract(data,'$.files') \
             FROM part WHERE session_id = ?1 AND message_id IN ({placeholders}) \
               AND json_extract(data,'$.type') IN ('tool','patch') ORDER BY time_created,id",
            content_lines("data", "$.tool")
        ))
        .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
    let rows = statement
        .query_map(
            rusqlite::params_from_iter(std::iter::once(session_id).chain(turn.iter().map(String::as_str))),
            read_stats_part,
        )
        .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
    for row in rows {
        let part = row.map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        if let Some(&slot) = index.get(&part.message_id) {
            messages[slot].parts.push(stats_part(part));
        }
    }
    Ok(messages)
}

fn stats_v2(conn: &Connection, session_id: &str, table: &str) -> Result<Vec<OpencodeMessage>, String> {
    use rusqlite::types::Value as SqlValue;
    let cut = stats_revert(conn, table, session_id)?;
    let cut_seq = match cut.as_deref() {
        Some(id) => {
            use rusqlite::OptionalExtension;
            conn.query_row(
                "SELECT seq FROM session_message WHERE session_id = ?1 AND id = ?2",
                [session_id, id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?
        }
        None => None,
    };
    let filter = if cut_seq.is_some() { " AND seq < ?2" } else { "" };
    let cut_params = || match cut_seq {
        Some(seq) => vec![SqlValue::Text(session_id.into()), SqlValue::Integer(seq)],
        None => vec![SqlValue::Text(session_id.into())],
    };
    // Row identities come from columns alone, so a user message's text and attachments stay unread.
    let recorded: Vec<(String, String, i64)> = {
        let mut statement = conn
            .prepare(&format!(
                "SELECT id,type,seq FROM session_message WHERE session_id = ?1 \
                   AND type IN ('user','assistant','shell'){filter} ORDER BY seq,id"
            ))
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(cut_params()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
            })
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let mut recorded = Vec::new();
        for row in rows {
            recorded.push(row.map_err(|e| format!("Failed to read OpenCode's store: {e}"))?);
        }
        recorded
    };
    let mut assistant_info: HashMap<String, Value> = HashMap::new();
    {
        let mut statement = conn
            .prepare(&format!(
                "SELECT id,seq,'assistant',json_extract(data,'$.parentID'),json_type(data,'$.tokens'),\
                 json_extract(data,'$.tokens.input'),json_extract(data,'$.tokens.output'),\
                 json_extract(data,'$.tokens.reasoning'),NULL,json_extract(data,'$.tokens.cache.read'),\
                 json_extract(data,'$.tokens.cache.write'),json_extract(data,'$.time.created'),\
                 json_extract(data,'$.time.completed') \
                 FROM session_message WHERE session_id = ?1 AND type = 'assistant'{filter} ORDER BY seq,id"
            ))
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(cut_params()), read_stats_message)
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        for row in rows {
            let row = row.map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
            let mut info = serde_json::Map::new();
            info.insert("id".into(), json!(&row.id));
            info.insert("role".into(), json!("assistant"));
            if let Some(parent) = row.parent {
                info.insert("parentID".into(), json!(parent));
            }
            if let Some(tokens) = stats_tokens(
                row.tokens_object.as_deref(),
                row.input,
                row.output,
                row.reasoning,
                None,
                row.cache_read,
                row.cache_write,
                true,
            ) {
                info.insert("tokens".into(), tokens);
            }
            if let Some(time) = stats_time(row.time_created, row.time_completed) {
                info.insert("time".into(), time);
            }
            assistant_info.insert(row.id, Value::Object(info));
        }
    }
    // The v2 store keeps no parent on a step; only the position after the last user (or a shell
    // the user ran) decides the turn, so parts are read no earlier than that boundary.
    let boundary = recorded
        .iter()
        .rev()
        .find(|(_, kind, _)| kind == "user" || kind == "shell")
        .map_or(-1, |(_, _, seq)| *seq);
    let mut parts: HashMap<String, Vec<Value>> = HashMap::new();
    if recorded.iter().any(|(_, kind, _)| kind == "assistant") {
        let cut = if cut_seq.is_some() { " AND sm.seq < ?3" } else { "" };
        let mut statement = conn
            .prepare(&format!(
                "SELECT sm.id,'oc2:'||sm.id||':tool:'||coalesce(json_extract(entry.value,'$.id'),''),\
                 'tool',json_extract(entry.value,'$.id'),json_extract(entry.value,'$.name'),\
                 json_extract(entry.value,'$.state.status'),json_extract(entry.value,'$.state.input.filePath'),{},\
                 json_extract(entry.value,'$.state.metadata.filediff.additions'),\
                 json_extract(entry.value,'$.state.metadata.filediff.deletions'),\
                 json_extract(entry.value,'$.state.metadata.exists'),NULL \
                 FROM session_message sm, json_each(sm.data,'$.content') entry \
                 WHERE sm.session_id = ?1 AND sm.type = 'assistant' AND sm.seq > ?2{cut} \
                   AND json_extract(entry.value,'$.type') = 'tool' ORDER BY sm.seq,entry.key",
                content_lines("entry.value", "$.name")
            ))
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let mut params = vec![SqlValue::Text(session_id.into()), SqlValue::Integer(boundary)];
        if let Some(seq) = cut_seq {
            params.push(SqlValue::Integer(seq));
        }
        let rows = statement
            .query_map(rusqlite::params_from_iter(params), read_stats_part)
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        for row in rows {
            let part = row.map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
            parts.entry(part.message_id.clone()).or_default().push(stats_part(part));
        }
    }
    let mut messages = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (id, kind, _) in recorded {
        match kind.as_str() {
            "user" => {
                messages.push(OpencodeMessage { info: json!({"id": id, "role": "user"}), parts: Vec::new() });
            }
            "shell" => {
                messages.push(OpencodeMessage { info: json!({"id": id.clone(), "role": "user"}), parts: Vec::new() });
                let info = json!({"id": format!("{id}:shell"), "role": "assistant", "parentID": id});
                let part = json!({"id": format!("oc2:{id}:shell:0"), "type": "tool", "tool": "shell"});
                messages.push(OpencodeMessage { info, parts: vec![part] });
            }
            "assistant" => {
                let slot = messages.len();
                index.insert(id.clone(), slot);
                let info = assistant_info
                    .remove(&id)
                    .unwrap_or_else(|| json!({"id": id, "role": "assistant"}));
                messages.push(OpencodeMessage { info, parts: Vec::new() });
            }
            _ => {}
        }
    }
    for (message_id, parts) in parts {
        if let Some(&slot) = index.get(&message_id) {
            messages[slot].parts = parts;
        }
    }
    Ok(messages)
}

fn messages_from(conn: &Connection, session_id: &str, remaining: &mut usize) -> Result<Vec<OpencodeMessage>, String> {
    let table=session_table(conn,session_id)?;
    let revert = {
        let mut statement = conn
            .prepare(&format!("SELECT octet_length(revert),CASE WHEN octet_length(revert)<=65536 THEN revert ELSE NULL END FROM {table} WHERE id = ?1"))
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let mut rows = statement
            .query([session_id])
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        match rows.next().map_err(|e| format!("Failed to read OpenCode's store: {e}"))? {
            Some(row) => {
                if row.get::<_,Option<u64>>(0).map_err(|e|e.to_string())?.is_some_and(|size|size>65536) {
                    return Err("unsupported_record: OpenCode revert metadata exceeds its byte budget".into());
                }
                row
                .get::<_, Option<String>>(1)
                .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?
                .map(|text| parse_json(&text))
            },
            None => return Err(format!("OpenCode has no session {session_id}")),
        }
    };
    let mut out = if has_table(conn,"session_message") && (table=="session_v2" || !has_table(conn,"message")) {
        // The same aggregate budget as the v1 tables, counted before any message body is read.
        let bytes:u64=conn.query_row("SELECT coalesce(sum(octet_length(data)+octet_length(id)+octet_length(type)+128),0) FROM session_message WHERE session_id=?1",
            [session_id],|row|row.get(0)).map_err(|e|e.to_string())?;
        if bytes>*remaining as u64 {return Err(super::history_store::compat::UPGRADE.into());}
        let mut statement=conn.prepare("SELECT id,type,data FROM session_message WHERE session_id=?1 ORDER BY seq,id")
            .map_err(|e|format!("Failed to read OpenCode's store: {e}"))?;
        let rows=statement.query_map([session_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?)))
            .map_err(|e|format!("Failed to read OpenCode's store: {e}"))?;
        let mut out=Vec::new();
        for row in rows {
            let (id,kind,data)=row.map_err(|e|format!("Failed to read OpenCode's store: {e}"))?;
            let mut native=parse_json(&data);
            if !native.is_object(){return Err("OpenCode's store contains an invalid message".into());}
            native["id"]=Value::String(id);
            native["type"]=Value::String(kind);
            out.extend(crate::agent::chat::opencode_v2::messages(session_id,&native));
        }
        *remaining-=bytes as usize;
        out
    } else if has_table(conn,"message") {
        let (out,bytes)=read_messages_bounded(conn,session_id,*remaining)?;
        *remaining-=bytes;
        out
    } else {Vec::new()};
    if let Some(cut) = revert.as_ref().and_then(|value| reverted_from(value)) {
        if let Some(index) = out.iter().position(|m| m.info.get("id").and_then(Value::as_str) == Some(cut)) {
            out.truncate(index);
        }
    }
    Ok(out)
}

/// The message a staged revert cut the conversation at, when the whole message was reverted.
///
/// A revert that names a part keeps the message and drops only later parts; OpenCode's own client shows
/// such a message in full, so it is kept here too.
pub fn reverted_from(revert: &Value) -> Option<&str> {
    if revert.get("partID").and_then(Value::as_str).is_some() {
        return None;
    }
    revert.get("messageID").and_then(Value::as_str)
}

#[cfg(test)]
fn read_messages(conn: &Connection, session_id: &str) -> Result<Vec<OpencodeMessage>, String> {
    read_messages_bounded(conn,session_id,super::history_store::limits::PAGE_BYTES).map(|(messages,_)|messages)
}

fn read_messages_bounded(conn:&Connection,session_id:&str,budget:usize) -> Result<(Vec<OpencodeMessage>,usize),String> {
    // SQLite's octet_length(column) inspects the record header without loading the TEXT/BLOB body.
    // This compatibility reader has one aggregate budget, enforced before either SELECT materializes JSON.
    let bytes:u64=conn.query_row("SELECT
        coalesce((SELECT sum(octet_length(data)+octet_length(id)+128) FROM message WHERE session_id=?1),0)+
        coalesce((SELECT sum(octet_length(data)+octet_length(id)+octet_length(message_id)+128) FROM part WHERE session_id=?1),0)",
        [session_id],|row|row.get(0)).map_err(|e|e.to_string())?;
    if bytes>budget as u64 {return Err(super::history_store::compat::UPGRADE.into());}
    let mut statement = conn
        .prepare("SELECT id, data FROM message WHERE session_id = ?1 ORDER BY time_created, id")
        .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
    let mut rows = statement
        .query([session_id])
        .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
    let mut messages: Vec<OpencodeMessage> = Vec::new();
    while let Some(row) = rows.next().map_err(|e| format!("Failed to read OpenCode's store: {e}"))? {
        let id: String = row
            .get(0)
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let data: String = row
            .get(1)
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let mut info = parse_json(&data);
        if let Value::Object(map) = &mut info {
            map.insert("id".into(), Value::String(id));
            map.insert("sessionID".into(), Value::String(session_id.to_string()));
        }
        messages.push(OpencodeMessage { info, parts: Vec::new() });
    }
    let mut statement = conn
        .prepare("SELECT id, message_id, data FROM part WHERE session_id = ?1 ORDER BY time_created, id")
        .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
    let mut rows = statement
        .query([session_id])
        .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
    let index: std::collections::HashMap<String, usize> = messages
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.info.get("id").and_then(Value::as_str).map(|id| (id.to_string(), i)))
        .collect();
    while let Some(row) = rows.next().map_err(|e| format!("Failed to read OpenCode's store: {e}"))? {
        let id: String = row
            .get(0)
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let message_id: String = row
            .get(1)
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let data: String = row
            .get(2)
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let Some(&slot) = index.get(&message_id) else { continue };
        let mut part = parse_json(&data);
        if let Value::Object(map) = &mut part {
            map.insert("id".into(), Value::String(id));
            map.insert("messageID".into(), Value::String(message_id));
            map.insert("sessionID".into(), Value::String(session_id.to_string()));
        }
        messages[slot].parts.push(part);
    }
    Ok((messages,bytes as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_message_revert_names_its_cut() {
        assert_eq!(reverted_from(&serde_json::json!({"messageID":"msg_1"})), Some("msg_1"));
        assert_eq!(
            reverted_from(&serde_json::json!({"messageID":"msg_1","partID":"prt_2"})),
            None
        );
        assert_eq!(reverted_from(&Value::Null), None);
    }

    #[test]
    fn data_dir_follows_xdg_then_home() {
        // Whatever the machine has, the answer ends in OpenCode's own directory name.
        let dir = data_dir();
        assert!(dir.is_none_or(|d| d.ends_with("opencode")));
    }

    #[test]
    fn compatibility_budget_rejects_large_native_parts_before_materializing_them() {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE message(id TEXT,session_id TEXT,time_created INTEGER,data TEXT);
            CREATE TABLE part(id TEXT,message_id TEXT,session_id TEXT,time_created INTEGER,data TEXT);
            INSERT INTO message VALUES('m','selected',1,'{\"role\":\"assistant\"}');
            INSERT INTO part VALUES('p','m','selected',1,CAST(zeroblob(40*1024*1024) AS TEXT));").unwrap();
        assert_eq!(read_messages(&conn,"selected").unwrap_err(),super::super::history_store::compat::UPGRADE);
        assert!(read_messages(&conn,"other").unwrap().is_empty());
    }

    #[test]
    fn v2_store_reads_sequence_and_revert_without_changing_native_rows() {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE session_v2(id TEXT,revert TEXT); CREATE TABLE session_message(id TEXT,session_id TEXT,type TEXT,seq INTEGER,data TEXT);
            INSERT INTO session_v2 VALUES ('ses_native',NULL);").unwrap();
        for (id,kind,seq,data) in [("msg_z","user",1,serde_json::json!({"text":"first"})),
            ("msg_a","assistant",2,serde_json::json!({"content":[{"type":"text","text":"reply"}],"time":{"completed":200}})),
            ("msg_b","user",3,serde_json::json!({"text":"reverted"}))] {
            conn.execute("INSERT INTO session_message VALUES (?1,'ses_native',?2,?3,?4)",rusqlite::params![id,kind,seq,data.to_string()]).unwrap();
        }
        let before:i64=conn.query_row("SELECT total_changes()",[],|r|r.get(0)).unwrap();
        let rows=messages_from(&conn,"ses_native",&mut usize::MAX.clone()).unwrap();
        assert_eq!(rows.iter().map(|m|m.info["id"].as_str().unwrap()).collect::<Vec<_>>(),["msg_z","msg_a","msg_b"]);
        assert_eq!(conn.query_row::<i64,_,_>("SELECT total_changes()",[],|r|r.get(0)).unwrap(),before);
        conn.execute("UPDATE session_v2 SET revert=?1",[serde_json::json!({"messageID":"msg_b"}).to_string()]).unwrap();
        assert_eq!(messages_from(&conn,"ses_native",&mut usize::MAX.clone()).unwrap().len(),2);
        conn.execute_batch("CREATE TABLE session(id TEXT,revert TEXT); CREATE TABLE message(id TEXT,session_id TEXT,time_created INTEGER,data TEXT);
            CREATE TABLE part(id TEXT,message_id TEXT,session_id TEXT,time_created INTEGER,data TEXT);
            INSERT INTO session VALUES ('ses_v1',NULL);
            INSERT INTO session_message VALUES ('msg_setting','ses_v1','model-switched',1,'{}');").unwrap();
        assert!(messages_from(&conn,"ses_v1",&mut usize::MAX.clone()).unwrap().is_empty());
    }

    #[test]
    fn stats_project_fields_without_the_recording_body_or_its_budget() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE session(id TEXT,revert TEXT); CREATE TABLE message(id TEXT,session_id TEXT,time_created INTEGER,data TEXT);
            CREATE TABLE part(id TEXT,message_id TEXT,session_id TEXT,time_created INTEGER,data TEXT);
            INSERT INTO session VALUES ('ses_native',NULL);").unwrap();
        // A legacy summary can hold every changed patch, so the row is larger than the paging budget.
        let user = serde_json::json!({"role":"user","summary":{"diffs":[{"patch":"x".repeat(600*1024)}]}});
        conn.execute("INSERT INTO message VALUES ('u','ses_native',1,?1)",[user.to_string()]).unwrap();
        let assistant = serde_json::json!({"parentID":"u","role":"assistant",
            "tokens":{"total":10,"input":1,"output":2,"reasoning":3,"cache":{"read":4,"write":0}},
            "time":{"created":100,"completed":400}});
        conn.execute("INSERT INTO message VALUES ('a','ses_native',2,?1)",[assistant.to_string()]).unwrap();
        let edit = serde_json::json!({"type":"tool","tool":"edit","callID":"c1","state":{"status":"completed",
            "input":{"filePath":"/r/a.ts"},"metadata":{"filediff":{"additions":4,"deletions":1}},
            "output":"y".repeat(600*1024)}});
        conn.execute("INSERT INTO part VALUES ('p1','a','ses_native',3,?1)",[edit.to_string()]).unwrap();
        let write = serde_json::json!({"type":"tool","tool":"write","callID":"c2","state":{"status":"completed",
            "input":{"filePath":"/r/new.ts","content":"1\n2\n"},"metadata":{"exists":false}}});
        conn.execute("INSERT INTO part VALUES ('p2','a','ses_native',4,?1)",[write.to_string()]).unwrap();
        let patch = serde_json::json!({"type":"patch","files":["/r/new.ts","/r/shell.ts"]});
        conn.execute("INSERT INTO part VALUES ('p3','a','ses_native',5,?1)",[patch.to_string()]).unwrap();
        let messages = stats_messages_from(&conn, "ses_native").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].info["role"], serde_json::json!("user"));
        assert!(messages[0].info.get("summary").is_none());
        assert_eq!(messages[1].info["tokens"]["total"], serde_json::json!(10));
        assert_eq!(messages[1].info["time"]["completed"], serde_json::json!(400));
        assert_eq!(messages[1].parts.len(), 3);
        let write = messages[1].parts.iter().find(|part| part["callID"] == serde_json::json!("c2")).unwrap();
        assert_eq!(write.pointer("/state/input/contentLines"), Some(&serde_json::json!(2)));
        assert!(write.pointer("/state/input/content").is_none());
        assert!(messages[1].parts.iter().all(|part| part.pointer("/state/output").is_none()));
    }

    #[test]
    fn stats_expand_shells_and_read_only_the_last_turns_tools() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE session_v2(id TEXT,revert TEXT); CREATE TABLE session_message(id TEXT,session_id TEXT,type TEXT,seq INTEGER,data TEXT);
            INSERT INTO session_v2 VALUES ('ses_native',NULL);").unwrap();
        let rows = [
            ("u1", "user", 1, serde_json::json!({"text":"first"})),
            ("sh", "shell", 2, serde_json::json!({"command":"ls","status":"completed","exit":0})),
            ("u2", "user", 3, serde_json::json!({"text":"next"})),
            ("a2", "assistant", 4, serde_json::json!({"content":[
                {"type":"text","text":"z".repeat(600*1024)},
                {"type":"tool","id":"call_1","name":"edit","state":{"status":"completed",
                    "input":{"filePath":"/r/a.ts"},"metadata":{"filediff":{"additions":2,"deletions":1}}}}],
                "tokens":{"input":1,"output":2,"reasoning":3,"cache":{"read":4,"write":5}},
                "time":{"created":10,"completed":20}})),
        ];
        for (id, kind, seq, data) in rows {
            conn.execute("INSERT INTO session_message VALUES (?1,'ses_native',?2,?3,?4)",
                rusqlite::params![id, kind, seq, data.to_string()]).unwrap();
        }
        let messages = stats_messages_from(&conn, "ses_native").unwrap();
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[1].info["id"], serde_json::json!("sh"));
        assert_eq!(messages[2].info["id"], serde_json::json!("sh:shell"));
        assert_eq!(messages[2].parts[0]["tool"], serde_json::json!("shell"));
        let last = &messages[4];
        assert_eq!(last.info["tokens"]["total"], serde_json::json!(15));
        assert_eq!(last.parts.len(), 1);
        assert_eq!(last.parts[0]["tool"], serde_json::json!("edit"));
        assert_eq!(last.parts[0].pointer("/state/metadata/filediff/additions"), Some(&serde_json::json!(2)));
    }

    #[test]
    fn stats_read_v2_tools_when_no_user_message_was_recorded() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE session_v2(id TEXT,revert TEXT); CREATE TABLE session_message(id TEXT,session_id TEXT,type TEXT,seq INTEGER,data TEXT);
            INSERT INTO session_v2 VALUES ('ses_native',NULL);").unwrap();
        let data = serde_json::json!({"content":[{"type":"tool","id":"call_1","name":"write","state":{"status":"completed",
            "input":{"filePath":"/r/new.ts","content":"a\nb\n"},"metadata":{"exists":false}}}],
            "tokens":{"input":1,"output":1}});
        conn.execute("INSERT INTO session_message VALUES ('a','ses_native','assistant',1,?1)",[data.to_string()]).unwrap();
        let messages = stats_messages_from(&conn, "ses_native").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].parts[0].pointer("/state/input/contentLines"), Some(&serde_json::json!(2)));
    }

    #[test]
    fn stats_stop_at_a_staged_revert() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE session(id TEXT,revert TEXT); CREATE TABLE message(id TEXT,session_id TEXT,time_created INTEGER,data TEXT);
            CREATE TABLE part(id TEXT,message_id TEXT,session_id TEXT,time_created INTEGER,data TEXT);
            INSERT INTO session VALUES ('ses_native','{\"messageID\":\"u2\"}');").unwrap();
        for (id, time, data) in [
            ("u1", 1, serde_json::json!({"role":"user"})),
            ("a1", 2, serde_json::json!({"role":"assistant","parentID":"u1","tokens":{"total":5,"input":5,"output":0}})),
            ("u2", 3, serde_json::json!({"role":"user"})),
            ("a2", 4, serde_json::json!({"role":"assistant","parentID":"u2","tokens":{"total":7,"input":7,"output":0}})),
        ] {
            conn.execute("INSERT INTO message VALUES (?1,'ses_native',?2,?3)", rusqlite::params![id, time, data.to_string()]).unwrap();
        }
        let messages = stats_messages_from(&conn, "ses_native").unwrap();
        assert_eq!(messages.iter().map(|m| m.info["id"].clone()).collect::<Vec<_>>(),
            [serde_json::json!("u1"), serde_json::json!("a1")]);
    }
}
