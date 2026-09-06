//! Global session-content search backed by two FTS5 indexes.
//!
//! `session_fts` (trigram) indexes history from every session, including archived ones, and answers
//! substring/CJK queries without any dictionary. `session_words` (jieba boundaries + unicode61 + porter) holds
//! the same fragments split into words, so a query matches whole words first: `和服` finds the word `和服`
//! but not the run `总和服务`, and `spawn` finds `spawned` through stemming while leaving `vspawn` alone.
//! Search runs the word index first and ranks its BM25 hits; only when it finds nothing does the trigram
//! index (or the LIKE fallback for terms under three characters) take over, so a partial identifier such as
//! `worktre` still resolves. Which path answered is not surfaced; both return the same hit shape.
//!
//! Every match carries `matched`, the literal strings the index actually matched (the stemmed word as it
//! appears in the text, or the query terms for substring hits). The frontend highlights those literals
//! rather than the raw query, so what is marked on screen is exactly what the backend matched.
//!
//! Dual content sources, index freshness, and extraction live in [`index`]; segmentation lives in
//! [`tokenize`]; ANSI stripping lives in [`ansi`].
//!
//! Navigation anchors: transcripts use `message_index`, which the frontend scrolls to; recordings use
//! `ordinal`, the number of `findNext` operations to perform. A recording ordinal is query-dependent (the
//! matching line's position among matches in document order), so it is calculated at search time; the
//! index's `ordinal` column stores only a stable line order for sorting. `findNext` counts occurrences of
//! the first matched literal, which equals the line position only when each matching line contains it
//! once, so recording navigation remains best-effort as documented in the design's section 6.

pub mod ansi;
pub mod index;
pub mod tokenize;

use std::path::Path;

use rusqlite::params_from_iter;
use serde::Serialize;

use crate::db::repo;
use crate::db::{table_exists, Db};
use crate::host::AppCtx;
use crate::models::SessionKind;

/// Maximum snippets returned per session; additional hits count toward `match_count` without allocating snippets.
const MAX_MATCHES_PER_SESSION: usize = 50;
/// Maximum characters per snippet, preventing an unusually long line from overwhelming the panel.
const SNIPPET_MAX_CHARS: usize = 200;
/// Approximate byte count of readable context retained on each side of a match.
const SNIPPET_CONTEXT: usize = 40;
/// Minimum term length for trigram MATCH; shorter terms use the LIKE fallback in [`search_sessions`].
const MIN_FTS_TERM_CHARS: usize = 3;

/// Search scope. Archive state comes from `sessions.archived_at` (null means live), so the SQL JOIN filters
/// sessions before results are produced rather than retrieving everything and discarding out-of-scope rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    /// Live (unarchived) sessions only; the default for global search.
    Live,
    /// Live and archived sessions when global search includes archives.
    All,
    /// Archived sessions only, used by search within the archive panel.
    Archived,
}

impl SearchScope {
    /// Parses a frontend string, conservatively falling back to `Live` for unknown values. This is a regular
    /// associated function rather than `FromStr` because parsing has no error state.
    pub fn from_arg(s: &str) -> Self {
        match s {
            "all" => Self::All,
            "archived" => Self::Archived,
            _ => Self::Live,
        }
    }

    /// Additional WHERE clause for this scope, applied to joined `sessions` alias `s`. These are fixed
    /// literals, never user input, and are therefore safe to interpolate into SQL.
    fn sql_filter(self) -> &'static str {
        match self {
            Self::All => "",
            Self::Live => " AND s.archived_at IS NULL",
            Self::Archived => " AND s.archived_at IS NOT NULL",
        }
    }
}

/// One matching snippet, with fields and serialization preserved from the previous implementation.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    /// Transcript message index used as the frontend scroll anchor; None for recordings.
    pub message_index: Option<u32>,
    /// One-based match number: display order within a transcript session, or the matching line's position in
    /// recording document order, which tells the frontend how many times to call findNext.
    pub ordinal: u32,
    /// Readable contextual snippet that preserves matched text for frontend highlighting.
    pub snippet: String,
    /// Literal strings the index matched in this fragment, in document order without duplicates. The
    /// frontend highlights these and uses the first one for recording `findNext`.
    pub matched: Vec<String>,
}

/// A matching session and its snippets, preserving the previous fields and serialization.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchHit {
    pub session_id: String,
    pub name: String,
    pub kind: SessionKind,
    /// Whether the session is archived, determining read-only viewing and live-session availability.
    pub archived: bool,
    /// Matching content source: `"transcript"` for conversations or `"recording"` for terminal recordings.
    pub source: &'static str,
    /// Total hit count, even when returned snippets are capped.
    pub match_count: u32,
    /// Matching snippets, capped at `MAX_MATCHES_PER_SESSION` per session.
    pub matches: Vec<SearchMatch>,
}

/// Raw row from session_fts before aggregation.
struct RawHit {
    session_id: String,
    source: String,
    message_index: Option<i64>,
    ordinal: i64,
    text: String,
    /// Byte spans in `text` marked by the word index; empty for substring hits.
    spans: Vec<(usize, usize)>,
}

/// Which index answered the query, which decides snippet placement, matched literals, and ordering.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Answered {
    /// Word index; rows arrive in BM25 order and carry highlight spans.
    Words,
    /// Trigram MATCH; rows arrive in BM25 order.
    Trigram,
    /// LIKE scan; rows carry no rank, so sessions sort by hit count.
    Like,
}

/// Searches every in-scope session for terms. `recordings_dir` is `<data_dir>/recordings`.
///
/// First, `refresh_stale` incrementally updates changed sessions so newly visible content is immediately
/// searchable; then the word index is queried, and the trigram index only when the word index has no hit.
/// A query empty after trimming and tokenization returns no results. If SQLite lacks FTS5/trigram support,
/// search gracefully returns no results after logging at startup.
pub fn search_sessions(
    db: &Db,
    recordings_dir: &Path,
    query: &str,
    scope: SearchScope,
) -> Result<Vec<SessionSearchHit>, String> {
    let keys: Vec<String> = query.split_whitespace().map(str::to_string).collect();
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    // Refresh the index before searching; expensive work stays outside the lock and transactions remain short.
    index::refresh_stale(db, recordings_dir)?;

    let conn = db.conn.lock().unwrap();
    if !table_exists(&conn, "session_fts") || !table_exists(&conn, "session_words") {
        return Ok(Vec::new()); // Return no results when the index is unavailable.
    }

    // Word index first. Every term must segment into at least one word; pure punctuation goes straight to
    // the substring path.
    let phrases: Option<Vec<String>> = keys.iter().map(|k| tokenize::query_phrase(k)).collect();
    let mut answered = Answered::Words;
    let mut raw = match phrases {
        Some(p) => query_words(&conn, &p, scope)?,
        None => Vec::new(),
    };
    if raw.is_empty() {
        // Substring fallback. If any term is shorter than three characters, use LIKE for the whole query;
        // trigram MATCH cannot handle it.
        let use_fts = keys.iter().all(|k| k.chars().count() >= MIN_FTS_TERM_CHARS);
        answered = if use_fts { Answered::Trigram } else { Answered::Like };
        raw = if use_fts {
            query_fts(&conn, &keys, scope)?
        } else {
            query_like(&conn, &keys, scope)?
        };
    }
    if raw.is_empty() {
        return Ok(Vec::new());
    }

    // Load session metadata once into an ID map and skip orphaned index rows without a corresponding session.
    let sessions = repo::list_all_sessions(&conn)?;
    let meta: std::collections::HashMap<&str, &crate::models::Session> =
        sessions.iter().map(|s| (s.id.as_str(), s)).collect();

    // Aggregate by session ID while preserving first-seen order, which is BM25 relevance order for FTS.
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<RawHit>> =
        std::collections::HashMap::new();
    for hit in raw {
        if !groups.contains_key(&hit.session_id) {
            order.push(hit.session_id.clone());
        }
        groups.entry(hit.session_id.clone()).or_default().push(hit);
    }

    let mut hits: Vec<SessionSearchHit> = Vec::new();
    for sid in &order {
        let Some(sess) = meta.get(sid.as_str()) else {
            continue; // Ignore orphaned rows for deleted sessions.
        };
        let mut rows = groups.remove(sid).unwrap_or_default();
        // Restore document order using the stored line/message ordinal.
        rows.sort_by_key(|r| r.ordinal);
        let is_transcript = rows
            .first()
            .map(|r| r.source == "transcript")
            .unwrap_or(false);
        let match_count = rows.len() as u32;

        let mut matches = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            if matches.len() >= MAX_MATCHES_PER_SESSION {
                break;
            }
            let (snippet, matched) = if answered == Answered::Words {
                let snippet = match (is_transcript, r.spans.first()) {
                    (true, Some(&(at, end))) => snippet_around(&r.text, at, end - at),
                    _ => clip_line(&r.text),
                };
                (snippet, matched_literals(&r.text, &r.spans))
            } else {
                let snippet = if is_transcript {
                    snippet_for(&r.text, &keys)
                } else {
                    clip_line(&r.text)
                };
                (snippet, keys.clone())
            };
            matches.push(SearchMatch {
                // Recording ordinal is the match's position in document order and equals the findNext count.
                // Transcript ordinal is display-only; messageIndex is the navigation anchor.
                message_index: r.message_index.map(|v| v as u32),
                ordinal: (i + 1) as u32,
                snippet,
                matched,
            });
        }

        hits.push(SessionSearchHit {
            session_id: sid.clone(),
            name: sess.name.clone(),
            kind: sess.kind,
            archived: sess.archived_at.is_some(),
            source: if is_transcript {
                "transcript"
            } else {
                "recording"
            },
            match_count,
            matches,
        });
    }

    // LIKE has no BM25 rank, so sort stably by descending hit count. Preserve rank order for FTS results.
    if answered == Answered::Like {
        hits.sort_by(|a, b| b.match_count.cmp(&a.match_count));
    }
    Ok(hits)
}

/// Distinct literal strings under the index's highlight spans, in document order.
fn matched_literals(text: &str, spans: &[(usize, usize)]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for &(a, b) in spans {
        let lit = &text[a..b];
        if !out.iter().any(|m| m == lit) {
            out.push(lit.to_string());
        }
    }
    out
}

/// Word-index query: each term is a phrase of its segmented words, terms are ANDed, rows come back in BM25
/// order together with FTS5 `highlight()` output that marks the matched words in the boundary-marked column.
fn query_words(
    conn: &rusqlite::Connection,
    phrases: &[String],
    scope: SearchScope,
) -> Result<Vec<RawHit>, String> {
    let expr = phrases.join(" ");
    let sql = format!(
        "SELECT f.session_id, f.source, f.message_index, f.ordinal, \
                highlight(session_words, 0, ?2, ?3) \
         FROM session_words w JOIN session_fts f ON f.rowid = w.rowid \
         JOIN sessions s ON s.id = f.session_id \
         WHERE session_words MATCH ?1{} ORDER BY w.rank",
        scope.sql_filter()
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare word query: {e}"))?;
    let rows = stmt
        .query_map(
            rusqlite::params![expr, tokenize::MARK_OPEN, tokenize::MARK_CLOSE],
            |row| {
                let marked: String = row.get(4)?;
                let h = tokenize::parse_highlight(&marked);
                Ok(RawHit {
                    session_id: row.get(0)?,
                    source: row.get(1)?,
                    message_index: row.get(2)?,
                    ordinal: row.get(3)?,
                    text: h.text,
                    spans: h.spans,
                })
            },
        )
        .map_err(|e| format!("Failed to run word query: {e}"))?;
    collect_raw(rows)
}

/// FTS5 multi-term query with implicit AND and BM25 relevance ranking. Joining `sessions` excludes orphaned
/// and out-of-scope rows in SQL while `rank` continues to reflect FTS match density.
fn query_fts(
    conn: &rusqlite::Connection,
    keys: &[String],
    scope: SearchScope,
) -> Result<Vec<RawHit>, String> {
    let expr = fts_match_expr(keys);
    let sql = format!(
        "SELECT f.session_id, f.source, f.message_index, f.ordinal, f.text \
         FROM session_fts f JOIN sessions s ON s.id = f.session_id \
         WHERE f.text MATCH ?1{} ORDER BY rank",
        scope.sql_filter()
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare fts query: {e}"))?;
    let rows = stmt
        .query_map(params_from_iter(std::iter::once(expr)), map_raw_hit)
        .map_err(|e| format!("Failed to run fts query: {e}"))?;
    collect_raw(rows)
}

/// LIKE fallback for short queries, scanning the stored text column with one implicitly ANDed `LIKE '%kw%'`
/// clause per term. The `sessions` JOIN applies the same scope filtering.
fn query_like(
    conn: &rusqlite::Connection,
    keys: &[String],
    scope: SearchScope,
) -> Result<Vec<RawHit>, String> {
    let clause = keys
        .iter()
        .map(|_| "f.text LIKE ? ESCAPE '\\'")
        .collect::<Vec<_>>()
        .join(" AND ");
    let sql = format!(
        "SELECT f.session_id, f.source, f.message_index, f.ordinal, f.text \
         FROM session_fts f JOIN sessions s ON s.id = f.session_id \
         WHERE {clause}{}",
        scope.sql_filter()
    );
    let binds: Vec<String> = keys
        .iter()
        .map(|k| format!("%{}%", like_escape(k)))
        .collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare like query: {e}"))?;
    let rows = stmt
        .query_map(params_from_iter(binds.iter()), map_raw_hit)
        .map_err(|e| format!("Failed to run like query: {e}"))?;
    collect_raw(rows)
}

/// Converts a rusqlite row into a RawHit.
fn map_raw_hit(row: &rusqlite::Row) -> rusqlite::Result<RawHit> {
    Ok(RawHit {
        session_id: row.get(0)?,
        source: row.get(1)?,
        message_index: row.get(2)?,
        ordinal: row.get(3)?,
        text: row.get(4)?,
        spans: Vec::new(),
    })
}

/// Collects query rows and converts rusqlite errors to strings.
fn collect_raw(
    rows: impl Iterator<Item = rusqlite::Result<RawHit>>,
) -> Result<Vec<RawHit>, String> {
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read hit row: {e}"))?);
    }
    Ok(out)
}

/// Builds an FTS5 MATCH expression by quoting each term, doubling embedded quotes, and separating terms with
/// spaces for implicit AND. With trigram tokenization, quoted terms perform substring matching, including CJK,
/// without case or diacritic sensitivity.
fn fts_match_expr(keys: &[String]) -> String {
    keys.iter()
        .map(|k| format!("\"{}\"", k.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escapes LIKE metacharacters (`\ % _`) for use with `ESCAPE '\'`.
fn like_escape(k: &str) -> String {
    let mut out = String::with_capacity(k.len());
    for c in k.chars() {
        if c == '\\' || c == '%' || c == '_' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Builds a transcript snippet around the earliest occurrence of any query term.
fn snippet_for(text: &str, keys: &[String]) -> String {
    let lower = text.to_lowercase();
    let mut best: Option<(usize, usize)> = None; // (byte offset, matched term length in bytes)
    for k in keys {
        let kl = k.to_lowercase();
        if kl.is_empty() {
            continue;
        }
        if let Some(p) = lower.find(&kl) {
            if best.map_or(true, |(bp, _)| p < bp) {
                best = Some((p, kl.len()));
            }
        }
    }
    match best {
        Some((at, len)) => snippet_around(text, at, len),
        None => clip_line(text), // Defensive fallback; a trigram substring should always exist in the source.
    }
}

/// Extracts a readable contextual snippet around byte offset `at` and match length `hit_len`. Newlines and tabs
/// become spaces, ellipses are added where needed, and the result is length-limited.
fn snippet_around(text: &str, at: usize, hit_len: usize) -> String {
    let len = text.len();
    let at = at.min(len);
    let raw_start = at.saturating_sub(SNIPPET_CONTEXT);
    let raw_end = (at + hit_len + SNIPPET_CONTEXT).min(len);
    let start = floor_char_boundary(text, raw_start);
    let end = ceil_char_boundary(text, raw_end);

    let core = text[start..end].replace(['\n', '\r', '\t'], " ");
    let trimmed = core.trim();
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(trimmed);
    if end < len {
        out.push('…');
    }
    limit_chars(&out)
}

/// Trims and limits a full-line snippet; recording entries already contain one visible line each.
fn clip_line(line: &str) -> String {
    limit_chars(line.trim())
}

/// Limits a string to `SNIPPET_MAX_CHARS`, truncating with an ellipsis when necessary.
fn limit_chars(s: &str) -> String {
    if s.chars().count() > SNIPPET_MAX_CHARS {
        let cut: String = s.chars().take(SNIPPET_MAX_CHARS).collect();
        format!("{cut}…")
    } else {
        s.to_string()
    }
}

/// Finds the greatest char boundary at or below `i`; the equivalent std method is still unstable.
fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Finds the smallest char boundary at or above `i`.
fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Best-effort background index rebuild after a session stops, called at EOF by the manager reader thread.
/// Errors are ignored because `refresh_stale` guarantees correctness before search; this only speeds up the
/// first search after shutdown.
pub fn reindex_session_quiet(app: &AppCtx, session_id: &str) {
    let Ok(data_dir) = app.data_dir() else {
        return;
    };
    let recordings_dir = data_dir.join("recordings");
    let db = app.db();
    let session = {
        let conn = db.conn.lock().unwrap();
        repo::get_session(&conn, session_id).ok().flatten()
    };
    let Some(session) = session else {
        return;
    };
    let _ = index::reindex_session(db, &recordings_dir, &session);
}

/// Bring the index up to date on a background thread at startup so the first search does not pay for the
/// sessions that changed since the last run, or for a full rebuild after the index tables change.
pub fn warm_index(app: AppCtx) {
    std::thread::spawn(move || {
        let Ok(data_dir) = app.data_dir() else {
            return;
        };
        let started = std::time::Instant::now();
        match index::refresh_stale(app.db(), &data_dir.join("recordings")) {
            Ok(()) => println!("search index warm-up finished in {:?}", started.elapsed()),
            Err(e) => eprintln!("search index warm-up failed: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    /// Creates a temporary database including the FTS table.
    fn temp_db() -> Db {
        let path = std::env::temp_dir().join(format!(
            "vlx-search-mod-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        Db::open(&path).unwrap()
    }

    /// Inserts a session row with enough data for metadata lookup.
    fn insert_session(db: &Db, id: &str, name: &str, kind: &str) {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO projects(id, name, root_path, sort_order, created_at) VALUES ('p','p','/p',0,0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions(id, project_id, name, kind, sort_order, created_at) VALUES (?1,'p',?2,?3,0,0)",
            params![id, name, kind],
        )
        .unwrap();
    }

    /// Inserts an FTS row directly to isolate search, aggregation, and ordinal behavior from extraction.
    fn insert_fts(
        db: &Db,
        sid: &str,
        source: &str,
        message_index: Option<i64>,
        ordinal: i64,
        text: &str,
    ) {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO session_fts(text, session_id, source, message_index, ordinal, role, ts) \
             VALUES (?1,?2,?3,?4,?5,NULL,NULL)",
            params![text, sid, source, message_index, ordinal],
        )
        .unwrap();
        let rowid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO session_words(rowid, words, session_id) VALUES (?1, ?2, ?3)",
            params![rowid, tokenize::boundary_marked(text), sid],
        )
        .unwrap();
    }

    /// The word index answers first: `和服` matches the word but not the run `总和服务`, which the trigram
    /// index would also have accepted, and the matched literal is exactly the word.
    #[test]
    fn word_index_rejects_cross_word_substring() {
        let db = temp_db();
        insert_session(&db, "s1", "kimono", "claude");
        insert_session(&db, "s2", "billing", "claude");
        insert_fts(&db, "s1", "transcript", Some(0), 0, "我买了一件和服，很好看。");
        insert_fts(&db, "s2", "transcript", Some(0), 0, "总和服务的费用还没有结清。");

        let hits = search_sessions(&db, Path::new("/nonexistent"), "和服", SearchScope::Live).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "s1");
        assert_eq!(hits[0].matches[0].matched, vec!["和服".to_string()]);
        assert!(hits[0].matches[0].snippet.contains("和服"));
    }

    /// A partial identifier has no word hit, so the trigram index takes over and matched carries the term.
    #[test]
    fn substring_fallback_when_no_word_matches() {
        let db = temp_db();
        insert_session(&db, "s1", "git", "claude");
        insert_fts(&db, "s1", "transcript", Some(0), 0, "list every worktree before merging");

        let hits = search_sessions(&db, Path::new("/nonexistent"), "worktre", SearchScope::Live).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matches[0].matched, vec!["worktre".to_string()]);
    }

    /// Porter stemming folds inflections and the matched literal is the word as written; a different
    /// identifier that merely contains the term is not a word hit.
    #[test]
    fn word_index_stems_english_and_reports_literal() {
        let db = temp_db();
        insert_session(&db, "s1", "orch", "claude");
        insert_session(&db, "s2", "cli", "claude");
        insert_fts(&db, "s1", "transcript", Some(0), 0, "The lead agent spawned two children.");
        insert_fts(&db, "s2", "transcript", Some(0), 0, "Run vspawn to create a child session.");

        let hits = search_sessions(&db, Path::new("/nonexistent"), "spawn", SearchScope::Live).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "s1");
        assert_eq!(hits[0].matches[0].matched, vec!["spawned".to_string()]);
    }

    /// A punctuated identifier and a multi-word CJK phrase each match as one phrase and come back as one
    /// literal covering the whole run in the text.
    #[test]
    fn word_index_phrases_report_whole_literals() {
        let db = temp_db();
        insert_session(&db, "s1", "app", "claude");
        insert_fts(&db, "s1", "transcript", Some(0), 0, "打开终端搜索功能，然后在 vlx-term 里试一下。");

        let hits = search_sessions(&db, Path::new("/nonexistent"), "终端搜索", SearchScope::Live).unwrap();
        assert_eq!(hits[0].matches[0].matched, vec!["终端搜索".to_string()]);
        let hits = search_sessions(&db, Path::new("/nonexistent"), "vlx-term", SearchScope::Live).unwrap();
        assert_eq!(hits[0].matches[0].matched, vec!["vlx-term".to_string()]);
    }

    /// Recording: single-term matches receive document-order ordinals equal to findNext counts; snippets are full lines.
    #[test]
    fn recording_single_keyword_ordinals_in_doc_order() {
        let db = temp_db();
        insert_session(&db, "s1", "term one", "terminal");
        // Three lines, with deploy on lines 0 and 2.
        insert_fts(&db, "s1", "recording", None, 0, "starting deploy step");
        insert_fts(&db, "s1", "recording", None, 1, "nothing here");
        insert_fts(&db, "s1", "recording", None, 2, "another deploy line");

        let hits = search_sessions(
            &db,
            std::path::Path::new("/no/such/dir"),
            "deploy",
            SearchScope::All,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        let h = &hits[0];
        assert_eq!(h.source, "recording");
        assert_eq!(h.match_count, 2);
        assert_eq!(h.matches.len(), 2);
        // In document order, line 0 has ordinal 1 and line 2 has ordinal 2.
        assert_eq!(h.matches[0].ordinal, 1);
        assert_eq!(h.matches[0].message_index, None);
        assert_eq!(h.matches[1].ordinal, 2);
        assert!(h.matches[0].snippet.contains("deploy"));
    }

    /// Transcript: messageIndex is the anchor, and multi-term AND requires every term in the same message.
    #[test]
    fn transcript_multi_keyword_and_within_message() {
        let db = temp_db();
        insert_session(&db, "s2", "claude one", "claude");
        // Message 0 contains only one term; message 3 contains both, so only message 3 satisfies AND.
        insert_fts(&db, "s2", "transcript", Some(0), 0, "先看数据库结构");
        insert_fts(
            &db,
            "s2",
            "transcript",
            Some(3),
            3,
            "数据库的配置在 application.yml",
        );

        let hits = search_sessions(
            &db,
            std::path::Path::new("/no/such/dir"),
            "数据库 配置",
            SearchScope::All,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        let h = &hits[0];
        assert_eq!(h.source, "transcript");
        assert_eq!(h.match_count, 1, "only a message containing both terms counts as a hit");
        assert_eq!(h.matches[0].message_index, Some(3));
        assert!(h.matches[0].snippet.contains("数据库") || h.matches[0].snippet.contains("配置"));
    }

    /// A two-character Chinese term remains searchable through LIKE because trigram MATCH requires three characters.
    #[test]
    fn short_cjk_keyword_uses_like_fallback() {
        let db = temp_db();
        insert_session(&db, "s3", "term cjk", "terminal");
        insert_fts(&db, "s3", "recording", None, 0, "正在读取配置文件");
        insert_fts(&db, "s3", "recording", None, 1, "无关内容");

        let hits = search_sessions(
            &db,
            std::path::Path::new("/no/such/dir"),
            "配置",
            SearchScope::All,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].match_count, 1);
        assert_eq!(hits[0].matches[0].ordinal, 1);
    }

    /// Empty or whitespace-only queries return no results without scanning.
    #[test]
    fn empty_query_returns_empty() {
        let db = temp_db();
        assert!(search_sessions(
            &db,
            std::path::Path::new("/no/such/dir"),
            "   ",
            SearchScope::All
        )
        .unwrap()
        .is_empty());
    }

    /// Sessions are ranked by BM25 match density, and orphaned index rows are omitted.
    #[test]
    fn ranks_sessions_and_skips_orphans() {
        let db = temp_db();
        insert_session(&db, "a", "few", "terminal");
        insert_session(&db, "b", "many", "terminal");
        // Orphaned row with no corresponding session.
        insert_fts(&db, "ghost", "recording", None, 0, "deploy ghost only");
        insert_fts(&db, "a", "recording", None, 0, "one deploy here");
        insert_fts(&db, "b", "recording", None, 0, "deploy deploy deploy");
        insert_fts(&db, "b", "recording", None, 1, "more deploy again");

        let hits = search_sessions(
            &db,
            std::path::Path::new("/no/such/dir"),
            "deploy",
            SearchScope::All,
        )
        .unwrap();
        // Return a and b, but not ghost.
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.session_id != "ghost"));
        // b has denser matches and ranks first under BM25.
        assert_eq!(hits[0].session_id, "b");
    }

    /// Scope filtering returns live, archived, or all sessions as requested for both FTS and LIKE paths.
    #[test]
    fn scope_filters_by_archived() {
        let db = temp_db();
        insert_session(&db, "live1", "live term", "terminal");
        insert_session(&db, "arch1", "archived term", "terminal");
        // Mark arch1 as archived with a non-null archived_at value.
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE sessions SET archived_at = 123 WHERE id = 'arch1'",
                [],
            )
            .unwrap();
        }
        // FTS path for a term of at least three characters: one hit in each session.
        insert_fts(&db, "live1", "recording", None, 0, "deploy on live");
        insert_fts(&db, "arch1", "recording", None, 0, "deploy on archived");

        let dir = std::path::Path::new("/no/such/dir");
        let live = search_sessions(&db, dir, "deploy", SearchScope::Live).unwrap();
        assert_eq!(live.len(), 1, "Live returns only sessions that are not archived");
        assert_eq!(live[0].session_id, "live1");
        assert!(!live[0].archived);

        let arch = search_sessions(&db, dir, "deploy", SearchScope::Archived).unwrap();
        assert_eq!(arch.len(), 1, "Archived returns only archived sessions");
        assert_eq!(arch[0].session_id, "arch1");
        assert!(arch[0].archived);

        let all = search_sessions(&db, dir, "deploy", SearchScope::All).unwrap();
        assert_eq!(all.len(), 2, "All returns both");

        // LIKE fallback for a two-character Chinese term: scope filtering still applies.
        insert_fts(&db, "live1", "recording", None, 1, "查看配置");
        insert_fts(&db, "arch1", "recording", None, 1, "修改配置");
        let live_like = search_sessions(&db, dir, "配置", SearchScope::Live).unwrap();
        assert_eq!(live_like.len(), 1, "the LIKE fallback filters by scope as well");
        assert_eq!(live_like[0].session_id, "live1");
    }
}


