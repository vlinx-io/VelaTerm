//! Full-text index extraction, persistence, and refresh.
//!
//! Two extraction tracks follow archive viewing and legacy search:
//! - Claude/Codex with a captured agentSessionId and readable transcript uses the **transcript track**,
//!   one parsed message per FTS row with `message_index` as the navigation anchor.
//! - Other sessions use the **recording track**, one ANSI-stripped log line per row with an `ordinal`.
//!
//! PTY reader threads never touch SQLite. Indexing occurs only when a session stops or just before search.
//! Expensive file reads, ANSI stripping, and transcript parsing happen outside the lock; short transactions
//! batch writes inside it so the shared `Mutex<Connection>` does not delay tree operations.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;

use crate::agent::transcript;
use crate::db::{repo, table_exists, Db};
use crate::models::Session;

/// Transcript-track source stored in session_fts and search_index_state.
const SRC_TRANSCRIPT: &str = "transcript";
/// Recording-track source identifier.
const SRC_RECORDING: &str = "recording";

/// One FTS fragment constructed outside the lock and batch-inserted inside it.
struct FtsRow {
    text: String,
    source: &'static str,
    /// Transcript message index/navigation anchor; None for recordings.
    message_index: Option<i64>,
    /// Fragment order within this session/track, used for sorting and recording hit counts.
    ordinal: i64,
    role: Option<String>,
    ts: Option<String>,
}

/// Complete extracted track for one session, built outside the lock and persisted in one batch.
struct BuildResult {
    source: &'static str,
    /// Indexed position: transcript byte length or recording byte offset/file length after a full rebuild.
    indexed_len: u64,
    /// Source-file mtime in seconds.
    indexed_mtime: Option<i64>,
    /// Resolved transcript path, persisted so later refreshes stat it directly; None for recordings.
    source_path: Option<String>,
    rows: Vec<FtsRow>,
}

/// Whether both index tables exist; missing FTS5/trigram support degrades search as a whole.
fn fts_ready(conn: &rusqlite::Connection) -> bool {
    table_exists(conn, "session_fts") && table_exists(conn, "session_words")
}

/// Remove one session's rows from both index tables; `source` limits it to one track, None clears all.
fn delete_rows(
    tx: &rusqlite::Transaction,
    session_id: &str,
    source: Option<&str>,
) -> Result<(), String> {
    match source {
        Some(src) => {
            tx.execute(
                "DELETE FROM session_words WHERE rowid IN \
                 (SELECT rowid FROM session_fts WHERE session_id = ?1 AND source = ?2)",
                params![session_id, src],
            )
            .map_err(|e| format!("Failed to clear word rows: {e}"))?;
            tx.execute(
                "DELETE FROM session_fts WHERE session_id = ?1 AND source = ?2",
                params![session_id, src],
            )
            .map_err(|e| format!("Failed to clear fts rows: {e}"))?;
        }
        None => {
            tx.execute(
                "DELETE FROM session_words WHERE session_id = ?1",
                params![session_id],
            )
            .map_err(|e| format!("Failed to clear word rows: {e}"))?;
            tx.execute(
                "DELETE FROM session_fts WHERE session_id = ?1",
                params![session_id],
            )
            .map_err(|e| format!("Failed to clear fts rows: {e}"))?;
        }
    }
    Ok(())
}

/// Current Unix time in seconds.
fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Return file byte length and mtime seconds, or zero/None when metadata is unreadable.
fn stat_len_mtime(path: &Path) -> (u64, Option<i64>) {
    match std::fs::metadata(path) {
        Ok(m) => {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
            (m.len(), mtime)
        }
        Err(_) => (0, None),
    }
}

/// Trim and return a nonempty agent_session_id.
fn nonempty_agent_id(s: &Session) -> Option<&str> {
    s.agent_session_id
        .as_deref()
        .map(str::trim)
        .filter(|x| !x.is_empty())
}

/// Convert transcript messages to FTS rows, skipping tool-only or bodyless entries. Preserve true parsed
/// indexes so `message_index` aligns with frontend readAgentTranscript navigation.
fn transcript_rows(messages: &[transcript::TranscriptMessage]) -> Vec<FtsRow> {
    let mut rows = Vec::new();
    for (idx, m) in messages.iter().enumerate() {
        if m.text.trim().is_empty() {
            continue; // Skip tool-only placeholders but retain real indexes for navigable rows.
        }
        rows.push(FtsRow {
            text: m.text.clone(),
            source: SRC_TRANSCRIPT,
            message_index: Some(idx as i64),
            ordinal: idx as i64,
            role: Some(m.role.clone()),
            ts: m.timestamp.clone(),
        });
    }
    rows
}

/// Convert visible recording text to one FTS row per nonempty line with contiguous zero-based ordinals.
fn recording_rows(lines: &[String], start_seq: i64) -> Vec<FtsRow> {
    let mut rows = Vec::new();
    let mut seq = start_seq;
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            continue; // Empty lines never match or count in xterm findNext, preserving ordinal alignment.
        }
        rows.push(FtsRow {
            text: t.to_string(),
            source: SRC_RECORDING,
            message_index: None,
            ordinal: seq,
            role: None,
            ts: None,
        });
        seq += 1;
    }
    rows
}

/// Extract an entire track outside the lock, selecting the source and building all rows. None means no
/// readable transcript or recording content.
fn build_full(session: &Session, recordings_dir: &Path) -> Option<BuildResult> {
    // Prefer a transcript when agentSessionId resolves to a readable, parseable file.
    if let Some(agent_id) = nonempty_agent_id(session) {
        if let Some(path) = transcript::source_path(session.kind, agent_id) {
            if let Ok(messages) = transcript::read_at(session.kind, &path) {
                let (indexed_len, indexed_mtime) = stat_len_mtime(&path);
                return Some(BuildResult {
                    source: SRC_TRANSCRIPT,
                    indexed_len,
                    indexed_mtime,
                    source_path: Some(path.to_string_lossy().into_owned()),
                    rows: transcript_rows(&messages),
                });
            }
        }
        // Fall back to recording when a remote/local transcript is absent or deleted.
    }
    // Recording track strips ANSI from the complete file, including its final partial line.
    let rec = recordings_dir.join(format!("{}.log", session.id));
    if rec.exists() {
        let bytes = std::fs::read(&rec).ok()?;
        let (indexed_len, indexed_mtime) = stat_len_mtime(&rec);
        let lines = super::ansi::strip_to_lines(&bytes);
        return Some(BuildResult {
            source: SRC_RECORDING,
            indexed_len,
            indexed_mtime,
            source_path: None,
            rows: recording_rows(&lines, 0),
        });
    }
    None
}

/// Batch-insert FTS rows inside the lock as part of a caller-managed short transaction. Each fragment goes
/// into the trigram table and, under the same rowid, into the word table with jieba boundaries marked.
fn insert_rows(
    tx: &rusqlite::Transaction,
    session_id: &str,
    rows: &[FtsRow],
) -> Result<(), String> {
    let mut stmt = tx
        .prepare(
            "INSERT INTO session_fts(text, session_id, source, message_index, ordinal, role, ts) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .map_err(|e| format!("Failed to prepare fts insert: {e}"))?;
    let mut words_stmt = tx
        .prepare("INSERT INTO session_words(rowid, words, session_id) VALUES (?1, ?2, ?3)")
        .map_err(|e| format!("Failed to prepare word insert: {e}"))?;
    for r in rows {
        stmt.execute(params![
            r.text,
            session_id,
            r.source,
            r.message_index,
            r.ordinal,
            r.role,
            r.ts,
        ])
        .map_err(|e| format!("Failed to insert fts row: {e}"))?;
        let rowid = tx.last_insert_rowid();
        words_stmt
            .execute(params![
                rowid,
                super::tokenize::boundary_marked(&r.text),
                session_id
            ])
            .map_err(|e| format!("Failed to insert word row: {e}"))?;
    }
    Ok(())
}

/// Upsert one refresh checkpoint inside the lock. `source_path` is the resolved transcript file for the
/// transcript track and None for recordings.
fn upsert_state(
    tx: &rusqlite::Transaction,
    session_id: &str,
    source: &str,
    indexed_len: u64,
    indexed_mtime: Option<i64>,
    source_path: Option<&str>,
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO search_index_state(session_id, source, indexed_len, indexed_mtime, indexed_at, source_path) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(session_id, source) DO UPDATE SET \
           indexed_len = excluded.indexed_len, \
           indexed_mtime = excluded.indexed_mtime, \
           indexed_at = excluded.indexed_at, \
           source_path = excluded.source_path",
        params![
            session_id,
            source,
            indexed_len as i64,
            indexed_mtime,
            now_secs(),
            source_path,
        ],
    )
    .map_err(|e| format!("Failed to upsert search state: {e}"))?;
    Ok(())
}

/// Rebuild a session's complete index after it stops: extract outside the lock, then delete, insert, and
/// checkpoint in one short transaction. A finalized session makes full rebuild simplest and most reliable.
pub fn reindex_session(db: &Db, recordings_dir: &Path, session: &Session) -> Result<(), String> {
    {
        // Quietly skip when FTS is unavailable in degraded mode.
        let conn = db.conn.lock().unwrap();
        if !fts_ready(&conn) {
            return Ok(());
        }
    }
    // Read and parse/strip files outside the lock.
    let built = build_full(session, recordings_dir);

    // Batch writes in a short locked transaction.
    let mut conn = db.conn.lock().unwrap();
    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to begin index tx: {e}"))?;
    // Remove both old tracks and checkpoints before writing the newly selected full track.
    delete_rows(&tx, &session.id, None)?;
    tx.execute(
        "DELETE FROM search_index_state WHERE session_id = ?1",
        params![session.id],
    )
    .map_err(|e| format!("Failed to clear search state: {e}"))?;
    if let Some(b) = built {
        insert_rows(&tx, &session.id, &b.rows)?;
        upsert_state(
            &tx,
            &session.id,
            b.source,
            b.indexed_len,
            b.indexed_mtime,
            b.source_path.as_deref(),
        )?;
    }
    tx.commit()
        .map_err(|e| format!("Failed to commit index tx: {e}"))?;
    Ok(())
}

/// Delete index rows and checkpoints for sessions removed as a subtree by `delete_node`.
pub fn drop_sessions(db: &Db, ids: &[String]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut conn = db.conn.lock().unwrap();
    if !fts_ready(&conn) {
        return Ok(());
    }
    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to begin drop tx: {e}"))?;
    for id in ids {
        delete_rows(&tx, id, None)?;
        tx.execute(
            "DELETE FROM search_index_state WHERE session_id = ?1",
            params![id],
        )
        .map_err(|e| format!("Failed to drop search state: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("Failed to commit drop tx: {e}"))?;
    Ok(())
}

/// Return a session track's checkpointed `(indexed_len, indexed_mtime)`, or None.
struct StateRow {
    indexed_len: u64,
    indexed_mtime: Option<i64>,
    /// Transcript path recorded at the last index; None for recordings or rows written before the column existed.
    source_path: Option<String>,
}

/// Resolve a session's transcript path for refresh. Prefer the path recorded at the last index when the file
/// still exists there, so a refresh costs one stat per session. Fall back to the per-kind directory lookup
/// only when nothing is recorded or the recorded file is gone; that lookup walks the agent's whole session tree
/// (measured at about 1.5 s for 450 Codex sessions) and is what made every search slow.
fn resolve_transcript_path(session: &Session, prev: Option<&StateRow>) -> Option<std::path::PathBuf> {
    let agent_id = nonempty_agent_id(session)?;
    if let Some(p) = prev.and_then(|st| st.source_path.as_deref()) {
        let path = std::path::PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    transcript::source_path(session.kind, agent_id)
}

/// Rows fetched per backfill round; small enough that the lock is released between rounds.
const BACKFILL_BATCH: usize = 500;

/// Serialises backfill rounds so a search and the startup warm-up never segment the same rows twice.
static BACKFILL_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Give every `session_fts` row a `session_words` companion. Normally a no-op; after the word table is first
/// created it segments the existing trigram rows in batches without re-reading any transcript, and it also
/// repairs a row pair that an interrupted write left half-written. Segmentation runs outside the lock; the
/// insert re-checks that the fragment still exists and has no companion yet, so a concurrent reindex or a
/// second backfill cannot produce duplicates.
pub fn backfill_words(db: &Db) -> Result<(), String> {
    let _gate = BACKFILL_GATE.lock().unwrap();
    // Row counts are a cheap scan; the per-row NOT EXISTS probe below costs about 100 ms over 30k rows,
    // too much to pay on every search when nothing is missing. Writes always pair the two tables inside
    // one transaction, so equal counts mean every row has its companion.
    {
        let conn = db.conn.lock().unwrap();
        let count = |table: &str| -> Result<i64, String> {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .map_err(|e| format!("Failed to count {table}: {e}"))
        };
        if count("session_fts")? == count("session_words")? {
            return Ok(());
        }
    }
    let mut done = 0usize;
    loop {
        let batch: Vec<(i64, String, String)> = {
            let conn = db.conn.lock().unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT f.rowid, f.text, f.session_id FROM session_fts f \
                     WHERE NOT EXISTS (SELECT 1 FROM session_words w WHERE w.rowid = f.rowid) \
                     LIMIT ?1",
                )
                .map_err(|e| format!("Failed to prepare word backfill: {e}"))?;
            let rows = stmt
                .query_map(params![BACKFILL_BATCH as i64], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .map_err(|e| format!("Failed to query word backfill: {e}"))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| format!("Failed to read word backfill row: {e}"))?);
            }
            out
        };
        if batch.is_empty() {
            break;
        }
        let marked: Vec<(i64, String, String)> = batch
            .into_iter()
            .map(|(rowid, text, sid)| (rowid, super::tokenize::boundary_marked(&text), sid))
            .collect();

        let mut conn = db.conn.lock().unwrap();
        let tx = conn
            .transaction()
            .map_err(|e| format!("Failed to begin word backfill tx: {e}"))?;
        {
            let mut ins = tx
                .prepare(
                    "INSERT INTO session_words(rowid, words, session_id) \
                     SELECT ?1, ?2, ?3 \
                     WHERE EXISTS (SELECT 1 FROM session_fts WHERE rowid = ?1) \
                       AND NOT EXISTS (SELECT 1 FROM session_words WHERE rowid = ?1)",
                )
                .map_err(|e| format!("Failed to prepare word backfill insert: {e}"))?;
            for (rowid, words, sid) in &marked {
                ins.execute(params![rowid, words, sid])
                    .map_err(|e| format!("Failed to backfill word row: {e}"))?;
            }
        }
        tx.commit()
            .map_err(|e| format!("Failed to commit word backfill tx: {e}"))?;
        done += marked.len();
        if marked.len() < BACKFILL_BATCH {
            break;
        }
    }
    if done > 0 {
        println!("search index: segmented {done} existing fragments into the word index");
    }
    Ok(())
}

/// Refresh sessions whose content is newer than their checkpoint before search. Most are skipped quickly;
/// changed transcripts rebuild fully while recordings append their tail. Also remove orphaned index rows and
/// complete the word index for any trigram row that lacks a companion.
pub fn refresh_stale(db: &Db, recordings_dir: &Path) -> Result<(), String> {
    // Snapshot sessions and checkpoints under the lock, then release it before expensive work.
    let (sessions, states, orphan_ids) = {
        let conn = db.conn.lock().unwrap();
        if !fts_ready(&conn) {
            return Ok(());
        }
        let sessions = repo::list_all_sessions(&conn)?;
        let live: std::collections::HashSet<String> =
            sessions.iter().map(|s| s.id.clone()).collect();

        let mut states: std::collections::HashMap<(String, String), StateRow> =
            std::collections::HashMap::new();
        let mut orphan: std::collections::HashSet<String> = std::collections::HashSet::new();
        {
            let mut stmt = conn
                .prepare(
                    "SELECT session_id, source, indexed_len, indexed_mtime, source_path \
                     FROM search_index_state",
                )
                .map_err(|e| format!("Failed to read search state: {e}"))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                })
                .map_err(|e| format!("Failed to query search state: {e}"))?;
            for r in rows {
                let (sid, source, len, mtime, source_path) =
                    r.map_err(|e| format!("Failed to read state row: {e}"))?;
                if !live.contains(&sid) {
                    orphan.insert(sid.clone());
                }
                states.insert(
                    (sid, source),
                    StateRow {
                        indexed_len: len.max(0) as u64,
                        indexed_mtime: mtime,
                        source_path,
                    },
                );
            }
        }
        (sessions, states, orphan.into_iter().collect::<Vec<_>>())
    };

    // Remove index rows whose sessions no longer exist in the database.
    if !orphan_ids.is_empty() {
        drop_sessions(db, &orphan_ids)?;
    }
    backfill_words(db)?;

    // Checkpoints written before source_path existed get the path recorded without re-indexing, in one
    // transaction at the end, so the first refresh after upgrading costs a directory lookup per session
    // rather than a full re-parse of every transcript.
    let mut path_backfill: Vec<(String, String)> = Vec::new();

    // Per session: check freshness, extract unlocked, then append/rebuild under the lock.
    for s in &sessions {
        // Select the same track as build_full, but reuse the recorded path instead of looking it up again.
        let st = states.get(&(s.id.clone(), SRC_TRANSCRIPT.to_string()));
        if let Some(path) = resolve_transcript_path(s, st) {
            // Rebuild a transcript fully when length or mtime changes.
            let (len, mtime) = stat_len_mtime(&path);
            let stale = match st {
                None => true,
                Some(prev) => prev.indexed_len != len || prev.indexed_mtime != mtime,
            };
            if stale {
                reindex_transcript(db, s, &path)?;
            } else if st.is_some_and(|prev| prev.source_path.is_none()) {
                path_backfill.push((s.id.clone(), path.to_string_lossy().into_owned()));
            }
            continue;
        }

        // Recording track.
        let rec = recordings_dir.join(format!("{}.log", s.id));
        if !rec.exists() {
            continue;
        }
        let (len, _mtime) = stat_len_mtime(&rec);
        let st = states.get(&(s.id.clone(), SRC_RECORDING.to_string()));
        match st {
            // Rebuild when absent from the index or truncated/replaced with a shorter file.
            None => {
                reindex_recording_full(db, &rec, &s.id)?;
            }
            Some(prev) if len < prev.indexed_len => {
                reindex_recording_full(db, &rec, &s.id)?;
            }
            // Append the incremental tail when the file grows.
            Some(prev) if len > prev.indexed_len => {
                reindex_recording_incremental(db, &rec, &s.id, prev.indexed_len)?;
            }
            // Skip unchanged files.
            _ => {}
        }
    }

    if !path_backfill.is_empty() {
        let mut conn = db.conn.lock().unwrap();
        let tx = conn
            .transaction()
            .map_err(|e| format!("Failed to begin path backfill tx: {e}"))?;
        for (sid, path) in &path_backfill {
            tx.execute(
                "UPDATE search_index_state SET source_path = ?1 WHERE session_id = ?2 AND source = ?3",
                params![path, sid, SRC_TRANSCRIPT],
            )
            .map_err(|e| format!("Failed to record transcript path: {e}"))?;
        }
        tx.commit()
            .map_err(|e| format!("Failed to commit path backfill tx: {e}"))?;
    }
    Ok(())
}

/// Rebuild a complete transcript index from an already resolved `path`, with unlocked parsing and a short
/// locked transaction. The path is recorded in the checkpoint so the next refresh can stat it directly.
fn reindex_transcript(db: &Db, session: &Session, path: &Path) -> Result<(), String> {
    let Ok(messages) = transcript::read_at(session.kind, path) else {
        return Ok(()); // Preserve the old index and retry later after a read failure.
    };
    let (indexed_len, indexed_mtime) = stat_len_mtime(path);
    let rows = transcript_rows(&messages);
    let source_path = path.to_string_lossy().into_owned();

    let mut conn = db.conn.lock().unwrap();
    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to begin transcript tx: {e}"))?;
    delete_rows(&tx, &session.id, Some(SRC_TRANSCRIPT))?;
    insert_rows(&tx, &session.id, &rows)?;
    upsert_state(
        &tx,
        &session.id,
        SRC_TRANSCRIPT,
        indexed_len,
        indexed_mtime,
        Some(&source_path),
    )?;
    tx.commit()
        .map_err(|e| format!("Failed to commit transcript tx: {e}"))?;
    Ok(())
}

/// Rebuild a complete recording index after truncation or on first indexing.
fn reindex_recording_full(db: &Db, rec: &Path, session_id: &str) -> Result<(), String> {
    let bytes = std::fs::read(rec).map_err(|e| format!("Failed to read recording: {e}"))?;
    let (indexed_len, indexed_mtime) = stat_len_mtime(rec);
    let lines = super::ansi::strip_to_lines(&bytes);
    let rows = recording_rows(&lines, 0);

    let mut conn = db.conn.lock().unwrap();
    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to begin recording tx: {e}"))?;
    delete_rows(&tx, session_id, Some(SRC_RECORDING))?;
    insert_rows(&tx, session_id, &rows)?;
    upsert_state(&tx, session_id, SRC_RECORDING, indexed_len, indexed_mtime, None)?;
    tx.commit()
        .map_err(|e| format!("Failed to commit recording tx: {e}"))?;
    Ok(())
}

/// Index complete new recording lines after `indexed_len`. Leave a trailing unterminated line for the next
/// refresh because it may continue growing, avoiding duplicates or misalignment.
fn reindex_recording_incremental(
    db: &Db,
    rec: &Path,
    session_id: &str,
    indexed_len: u64,
) -> Result<(), String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(rec).map_err(|e| format!("Failed to open recording: {e}"))?;
    f.seek(SeekFrom::Start(indexed_len))
        .map_err(|e| format!("Failed to seek recording: {e}"))?;
    let mut tail = Vec::new();
    f.read_to_end(&mut tail)
        .map_err(|e| format!("Failed to read recording tail: {e}"))?;
    if tail.is_empty() {
        return Ok(());
    }
    // Keep only through the final newline and defer the trailing partial line.
    let Some(last_nl) = tail.iter().rposition(|&b| b == b'\n') else {
        return Ok(()); // No complete new line yet.
    };
    let cut = last_nl + 1;
    let complete = &tail[..cut];
    let new_indexed_len = indexed_len + cut as u64;
    let lines = super::ansi::strip_to_lines(complete);
    let mtime = stat_len_mtime(rec).1;

    let mut conn = db.conn.lock().unwrap();
    // Continue after the recording track's current maximum ordinal.
    let next_seq: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(ordinal) + 1, 0) FROM session_fts WHERE session_id = ?1 AND source = ?2",
            params![session_id, SRC_RECORDING],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to read max ordinal: {e}"))?;
    let rows = recording_rows(&lines, next_seq);

    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to begin incremental tx: {e}"))?;
    insert_rows(&tx, session_id, &rows)?;
    upsert_state(&tx, session_id, SRC_RECORDING, new_indexed_len, mtime, None)?;
    tx.commit()
        .map_err(|e| format!("Failed to commit incremental tx: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    /// Create a temporary database including the FTS table.
    fn temp_db() -> Db {
        let path = std::env::temp_dir().join(format!(
            "vlx-search-idx-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        Db::open(&path).unwrap()
    }

    /// Construct a minimal Session containing only index-relevant fields.
    fn make_session(id: &str, kind: crate::models::SessionKind, agent_id: Option<&str>) -> Session {
        Session {
            id: id.to_string(),
            project_id: "p".to_string(),
            group_id: None,
            agent_preset_id: None,
            agent_path: None,
            name: id.to_string(),
            kind,
            shell: None,
            cwd: None,
            env_json: None,
            init_cmd: None,
            agent_args: None,
            permission_mode: None,
            collaboration_mode: None,
            engine: "tui".to_string(),
            hotkey: None,
            agent_session_id: agent_id.map(|s| s.to_string()),
            parent_session_id: None,
            collapsed: false,
            worktree_path: None,
            worktree_base_ref: None,
            archived_at: None,
            browser_url: None,
            mark: None,
            sort_order: 0,
            created_at: 0,
        }
    }

    fn count_rows(db: &Db, session_id: &str) -> i64 {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM session_fts WHERE session_id = ?1",
            params![session_id],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// Full recording rebuild strips ANSI into rows, and deletion clears them.
    #[test]
    fn reindex_recording_full_inserts_and_drop_clears() {
        let db = temp_db();
        let dir = std::env::temp_dir().join(format!("vlx-rec-full-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sid = "sess-rec";
        std::fs::write(
            dir.join(format!("{sid}.log")),
            b"\x1b[32mline one deploy\x1b[0m\r\nline two\r\nthird deploy line\r\n",
        )
        .unwrap();
        let session = make_session(sid, crate::models::SessionKind::Terminal, None);

        reindex_session(&db, &dir, &session).unwrap();
        // Three nonempty lines produce three FTS rows.
        assert_eq!(count_rows(&db, sid), 3);

        // Deletion cleanup.
        drop_sessions(&db, &[sid.to_string()]).unwrap();
        assert_eq!(count_rows(&db, sid), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Incremental recording indexing appends new complete lines and continues ordinals after refresh.
    #[test]
    fn recording_incremental_appends_new_lines() {
        let db = temp_db();
        let dir = std::env::temp_dir().join(format!("vlx-rec-inc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sid = "sess-inc";
        let path = dir.join(format!("{sid}.log"));
        std::fs::write(&path, b"alpha match\r\nbeta\r\n").unwrap();
        let session = make_session(sid, crate::models::SessionKind::Terminal, None);
        // Insert a real sessions row because refresh_stale removes index entries absent from that table.
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO projects(id, name, root_path, sort_order, created_at) VALUES ('p','p','/p',0,0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO sessions(id, project_id, name, kind, sort_order, created_at) VALUES (?1,'p',?1,'terminal',0,0)",
                params![sid],
            )
            .unwrap();
        }

        // Initial full reindex.
        reindex_session(&db, &dir, &session).unwrap();
        assert_eq!(count_rows(&db, sid), 2);

        // Append one complete line and one unterminated partial line.
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            f.write_all(b"gamma match\r\npartial-no-newline").unwrap();
        }
        // Refresh appends only the complete incremental line.
        refresh_stale(&db, &dir).unwrap();
        // One complete gamma line brings the total to three; the partial line is excluded.
        assert_eq!(count_rows(&db, sid), 3);

        // Maximum ordinal is 2 across contiguous 0, 1, and 2.
        {
            let conn = db.conn.lock().unwrap();
            let max: i64 = conn
                .query_row(
                    "SELECT MAX(ordinal) FROM session_fts WHERE session_id = ?1 AND source = 'recording'",
                    params![sid],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(max, 2);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Refresh reads a transcript through the path recorded in search_index_state and records the path it
    /// indexed, so no per-session directory lookup runs. The agent ID here exists nowhere under ~/.claude,
    /// so the only way the rows can appear is through the recorded path.
    #[test]
    fn refresh_uses_recorded_transcript_path() {
        let db = temp_db();
        let dir = std::env::temp_dir().join(format!("vlx-cached-path-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sid = "sess-cached";
        let agent_id = "00000000-cached-path-test-0000";
        let transcript = dir.join(format!("{agent_id}.jsonl"));
        std::fs::write(
            &transcript,
            concat!(
                "{\"type\":\"user\",\"timestamp\":\"t1\",\"message\":{\"content\":\"cached deploy question\"}}\n",
                "{\"type\":\"assistant\",\"timestamp\":\"t2\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"cached deploy answer\"}]}}\n",
            ),
        )
        .unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO projects(id, name, root_path, sort_order, created_at) VALUES ('p','p','/p',0,0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO sessions(id, project_id, name, kind, agent_session_id, sort_order, created_at) \
                 VALUES (?1,'p',?1,'claude',?2,0,0)",
                params![sid, agent_id],
            )
            .unwrap();
            // A checkpoint whose length differs from the file marks the transcript stale on the next refresh.
            conn.execute(
                "INSERT INTO search_index_state(session_id, source, indexed_len, indexed_at, source_path) \
                 VALUES (?1,'transcript',0,0,?2)",
                params![sid, transcript.to_string_lossy()],
            )
            .unwrap();
        }

        refresh_stale(&db, &dir).unwrap();
        assert_eq!(count_rows(&db, sid), 2);
        {
            let conn = db.conn.lock().unwrap();
            let (len, path): (i64, Option<String>) = conn
                .query_row(
                    "SELECT indexed_len, source_path FROM search_index_state WHERE session_id = ?1",
                    params![sid],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(len as u64, std::fs::metadata(&transcript).unwrap().len());
            assert_eq!(path.as_deref(), Some(transcript.to_string_lossy().as_ref()));
        }

        // Once the recorded file is gone, the lookup falls back to the directory scan, finds nothing, and the
        // old rows are preserved rather than dropped.
        std::fs::remove_file(&transcript).unwrap();
        refresh_stale(&db, &dir).unwrap();
        assert_eq!(count_rows(&db, sid), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A trigram row without a word companion (the state of every row when the word table is first created)
    /// gets one on refresh without any transcript being read, and the word index then finds it.
    #[test]
    fn refresh_backfills_word_index_from_existing_rows() {
        let db = temp_db();
        let dir = std::env::temp_dir().join(format!("vlx-backfill-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sid = "sess-backfill";
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO projects(id, name, root_path, sort_order, created_at) VALUES ('p','p','/p',0,0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO sessions(id, project_id, name, kind, sort_order, created_at) VALUES (?1,'p',?1,'terminal',0,0)",
                params![sid],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO session_fts(text, session_id, source, message_index, ordinal) \
                 VALUES ('打开终端搜索功能', ?1, 'recording', NULL, 0)",
                params![sid],
            )
            .unwrap();
        }

        refresh_stale(&db, &dir).unwrap();
        // Running it again must not duplicate the companion.
        refresh_stale(&db, &dir).unwrap();
        let conn = db.conn.lock().unwrap();
        let words: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM session_words WHERE session_id = ?1",
                params![sid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(words, 1);
        let found: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM session_words WHERE session_words MATCH '\"终端 搜索\"'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 1);
        drop(conn);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Refresh removes index rows for sessions absent from the database.
    #[test]
    fn refresh_drops_orphan_rows() {
        let db = temp_db();
        let dir = std::env::temp_dir().join(format!("vlx-orphan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Insert an index row and checkpoint belonging to no session.
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO session_fts(text, session_id, source, ordinal) VALUES ('ghost','ghost-sess','recording',0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO search_index_state(session_id, source, indexed_len, indexed_at) VALUES ('ghost-sess','recording',5,0)",
                [],
            )
            .unwrap();
        }
        // With no sessions rows, ghost-sess is an orphan.
        refresh_stale(&db, &dir).unwrap();
        assert_eq!(count_rows(&db, "ghost-sess"), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
