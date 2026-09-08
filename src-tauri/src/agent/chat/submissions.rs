//! Durable submission receipts prevent a lost acknowledgement from sending a prompt twice.

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

pub const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS chat_submissions (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    id TEXT NOT NULL, fingerprint TEXT NOT NULL, outcome TEXT,
    PRIMARY KEY(session_id, id)
);";

pub enum Claim {
    New,
    Complete(Result<String, String>),
}

pub fn claim(conn: &Connection, session: &str, id: &str, payload: &[u8]) -> Result<Claim, String> {
    if !id.starts_with("msg-") || uuid::Uuid::parse_str(&id[4..]).is_err() {
        return Err("Invalid message identifier".into());
    }
    let fingerprint = format!("{:x}", Sha256::digest(payload));
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO chat_submissions(session_id,id,fingerprint) VALUES (?1,?2,?3)",
        params![session, id, fingerprint],
    ).map_err(|e| e.to_string())?;
    if inserted == 1 { return Ok(Claim::New); }
    let (saved, outcome): (String, Option<String>) = conn.query_row(
        "SELECT fingerprint,outcome FROM chat_submissions WHERE session_id=?1 AND id=?2",
        params![session, id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| e.to_string())?;
    if saved != fingerprint { return Err("Message identifier already belongs to another submission".into()); }
    match outcome {
        Some(value) => serde_json::from_str(&value).map(Claim::Complete).map_err(|e| e.to_string()),
        // The process may have stopped after writing to the agent but before saving the receipt.
        // Keep this unresolved instead of guessing that another write would be safe.
        None => Err("chat_submission_pending".into()),
    }
}

pub fn finish(conn: &Connection, session: &str, id: &str, outcome: &Result<String, String>) -> Result<(), String> {
    let value = serde_json::to_string(outcome).map_err(|e| e.to_string())?;
    conn.execute("UPDATE chat_submissions SET outcome=?3 WHERE session_id=?1 AND id=?2",
        params![session, id, value]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY); INSERT INTO sessions VALUES ('s');").unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn
    }

    #[test]
    fn duplicate_claims_never_dispatch_and_receipts_survive_reopening() {
        let path = std::env::temp_dir().join(format!("vlx-submission-{}.db", uuid::Uuid::new_v4()));
        let id = format!("msg-{}", uuid::Uuid::new_v4());
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY); INSERT INTO sessions VALUES ('s');").unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            assert!(matches!(claim(&conn, "s", &id, b"prompt").unwrap(), Claim::New));
            assert!(matches!(claim(&conn, "s", &id, b"prompt"), Err(error) if error == "chat_submission_pending"));
            finish(&conn, "s", &id, &Ok("queued".into())).unwrap();
        }
        let conn = Connection::open(&path).unwrap();
        assert!(matches!(claim(&conn, "s", &id, b"prompt").unwrap(), Claim::Complete(Ok(value)) if value == "queued"));
        assert!(claim(&conn, "s", &id, b"different prompt").is_err());
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejections_are_repeatable_and_invalid_identifiers_are_not_claimed() {
        let conn = database();
        assert!(claim(&conn, "s", "arbitrary", b"prompt").is_err());
        let id = format!("msg-{}", uuid::Uuid::new_v4());
        assert!(matches!(claim(&conn, "s", &id, b"prompt").unwrap(), Claim::New));
        finish(&conn, "s", &id, &Err("No running turn".into())).unwrap();
        assert!(matches!(claim(&conn, "s", &id, b"prompt").unwrap(), Claim::Complete(Err(value)) if value == "No running turn"));
    }
}
