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

use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

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
    data_dir().map(|dir| dir.join("opencode.db"))
}

fn open() -> Result<Connection, String> {
    let path = db_path().ok_or("OpenCode's data directory could not be located")?;
    if !path.exists() {
        return Err("OpenCode has not recorded any conversation on this machine".to_string());
    }
    let conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Failed to open OpenCode's store: {e}"))?;
    // OpenCode writes to the same file while it runs; wait briefly rather than fail on a busy lock.
    conn.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| format!("Failed to open OpenCode's store: {e}"))?;
    Ok(conn)
}

fn parse_json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

/// The native model and variant used when OpenCode resumes its terminal interface.
pub fn session_model(session_id: &str) -> Result<Option<Value>, String> {
    use rusqlite::OptionalExtension;
    let conn = open()?;
    let raw: Option<Option<String>> = conn.query_row(
        "SELECT model FROM session WHERE id = ?1", [session_id], |row| row.get(0),
    ).optional().map_err(|e| format!("Failed to read OpenCode session model: {e}"))?;
    Ok(raw.flatten().map(|text| parse_json(&text)))
}

/// Every message of a session with its parts, oldest first.
///
/// Messages that OpenCode has reverted are left out, the way its own client hides them: a revert is
/// staged on the session and cleaned up at the next prompt, and until then the rows still exist.
pub fn messages(session_id: &str) -> Result<Vec<OpencodeMessage>, String> {
    let conn = open()?;
    let revert = {
        let mut statement = conn
            .prepare("SELECT revert FROM session WHERE id = ?1")
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        let mut rows = statement
            .query([session_id])
            .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?;
        match rows.next().map_err(|e| format!("Failed to read OpenCode's store: {e}"))? {
            Some(row) => row
                .get::<_, Option<String>>(0)
                .map_err(|e| format!("Failed to read OpenCode's store: {e}"))?
                .map(|text| parse_json(&text)),
            None => return Err(format!("OpenCode has no session {session_id}")),
        }
    };
    let mut out = read_messages(&conn, session_id)?;
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

fn read_messages(conn: &Connection, session_id: &str) -> Result<Vec<OpencodeMessage>, String> {
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
    Ok(messages)
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
}
