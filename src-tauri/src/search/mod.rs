//! Global session-content search.
//!
//! The persistent index in [`corpus`] lives in its own derived file. `session_fts` (trigram) answers
//! substring/CJK queries without any dictionary; `session_words` (jieba boundaries + unicode61 + porter)
//! holds the same fragments split into words, so a query matches whole words first: `和服` finds the word
//! `和服` but not the run `总和服务`, and `spawn` finds `spawned` through stemming while leaving `vspawn`
//! alone. Only when the word index finds nothing does the trigram index (or LIKE for terms under three
//! characters) take over, so a partial identifier such as `worktre` still resolves.
//!
//! Every match carries `matched`, the literal strings the index actually matched. The frontend highlights
//! those literals rather than the raw query, so what is marked on screen is exactly what was matched.
//!
//! Search jobs live in [`jobs`]; segmentation in [`tokenize`]; ANSI stripping in [`ansi`]; fuzzy text
//! matching shared with the knowledge searches in [`fuzzy`].
//!
//! Navigation anchors: transcripts use `message_index`, which the frontend scrolls to, and, when the
//! message was read from its file directly, `record_offset`, which locates it exactly; recordings use
//! `ordinal`, the number of `findNext` operations to perform, counted among the matching lines.

pub mod ansi;
pub mod corpus;
mod extract;
pub mod fuzzy;
pub mod jobs;
mod legacy;
pub mod tokenize;

/// Conversations read straight from the provider's records, numbered as search numbers them.
pub(crate) use extract::conversation;

use serde::Serialize;

use crate::db::repo;
use crate::host::AppCtx;
use crate::models::SessionKind;

/// Maximum characters per snippet, preventing an unusually long line from overwhelming the panel.
const SNIPPET_MAX_CHARS: usize = 200;

/// Search scope. Archive state comes from `sessions.archived_at` (null means live).
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
}

/// One matching snippet, with fields and serialization preserved from the previous implementation.
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role:Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub row_id:Option<String>,
    /// Byte offset of the message's first record in its transcript file; the conversation view locates
    /// the message by it.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub record_offset:Option<u64>,
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
    /// Matching snippets, capped at [`corpus::MAX_MATCHES`] per session.
    pub matches: Vec<SearchMatch>,
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

/// Bring one session's search rows up to date after it stops, in the background.
pub fn reindex_session_quiet(app: &AppCtx, session_id: &str) {
    let app=app.clone();let id=session_id.to_owned();
    std::thread::spawn(move || {
        let session=corpus::sessions(&app,None).ok().and_then(|_|{
            let conn=app.db().conn.lock().unwrap();
            repo::get_session(&conn,&id).ok().flatten().map(|session|corpus::Session {id:session.id,project_id:session.project_id,name:session.name,kind:session.kind,
                native:session.agent_session_id.or(session.legacy_agent_session_id),archived:session.archived_at.is_some()})
        });
        let Some(session)=session else {return;};
        let cancel=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        if let Ok(conn)=corpus::open(&app) {let _=corpus::refresh_session(&app,&conn,&session,&cancel);}
    });
}

/// After startup: remove the business database's old search tables once, remove earlier index files the
/// current one has replaced, then fill the search index in the background so the first search does not
/// have to read every conversation.
pub fn warm_index(app: AppCtx) {
    std::thread::spawn(move || {
        legacy::remove(&app);
        corpus::remove_earlier(&app);
        // History is indexed in memory now; the derived index and protocol spool earlier versions kept
        // on disk are removed.
        if let Ok(dir)=app.data_dir() {crate::agent::history_store::store::retire_persisted(&dir);}
        if let Some(background)=corpus::Background::begin(&app) {catch_up(&app,&background);}
    });
}

/// Bring every session up to date in the background. A search that runs meanwhile answers from what is
/// indexed and leaves never-indexed sessions to this pass, which starts no new session while a search
/// is refreshing.
fn catch_up(app: &AppCtx,background: &corpus::Background) {
    let cancel=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let started=std::time::Instant::now();let (mut indexed,mut skipped,mut pressure)=(0u64,0u64,0u32);
    let Ok(conn)=corpus::open(app) else {return;};
    let mut after:Option<String>=None;let mut complete=true;
    'sessions: loop {
        let Ok(sessions)=corpus::sessions(app,after.as_deref()) else {complete=false;break;};
        if sessions.is_empty() {break;}
        for session in sessions {
            background.yield_to_searches();
            after=Some(session.id.clone());
            match corpus::refresh_session(app,&conn,&session,&cancel) {
                Ok(corpus::Refreshed::Indexed(_))=>{indexed+=1;pressure=0;},
                Ok(_)=>pressure=0,
                // Storage pressure usually passes once other readers finish; a search later
                // retries the sessions skipped here. Persistent pressure ends the warm-up.
                Err(error) if error.starts_with("storage_pressure:")=> {
                    skipped+=1;pressure+=1;
                    crate::diagnostics::record("WARN","search_session_skipped",serde_json::json!({"sessionId":session.id,"errorCode":"unavailable"}));
                    complete=false;if pressure>=10 {break 'sessions;}
                    std::thread::sleep(std::time::Duration::from_secs(30));
                },
                // A conversation that cannot be read, or no longer exists, stays that way; anything
                // else may pass, so the pass does not count as complete.
                Err(error)=> {
                    skipped+=1;
                    if !["index_incomplete:","unsupported_record:","detail_unavailable:","source_missing:"].iter().any(|code|error.starts_with(code)) {complete=false;}
                },
            }
            // Leave room for interactive work between sessions.
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    let _=corpus::prune(&conn);
    if indexed>0 {let _=corpus::optimize(&conn);}
    // Only a pass that reached every session retires the earlier index files.
    if complete {let _=corpus::mark_built(app,&conn);}
    crate::diagnostics::record("INFO","search_warmup",serde_json::json!({"status":"complete","inputCount":indexed,"retryCount":skipped,
        "durationMs":started.elapsed().as_millis() as u64}));
}

/// Start a background pass for sessions a search deferred; no-op while one is running.
pub(super) fn catch_up_later(app: &AppCtx) {
    let Some(background)=corpus::Background::begin(app) else {return;};
    let app=app.clone();
    std::thread::spawn(move || catch_up(&app,&background));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::sync::Arc;

    /// An application with a file-backed database and an empty search index.
    struct Fixture {app:AppCtx,path:std::path::PathBuf}
    impl Fixture {
        fn new() -> Self {
            let path=std::env::temp_dir().join(format!("vlx-search-mod-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&path).unwrap();
            let db=crate::db::Db::open(&path.join("business.sqlite")).unwrap();
            Self {app:AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(path.clone(),db))),path}
        }
    }
    impl Drop for Fixture {fn drop(&mut self) {crate::agent::history_store::store::release_test_directory(&self.path);let _=std::fs::remove_dir_all(&self.path);}}

    /// Inserts a session row with enough data for metadata lookup.
    fn insert_session(db: &Fixture, id: &str, name: &str, kind: &str) {
        let conn = db.app.db().conn.lock().unwrap();
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

    /// Inserts one index fragment directly to isolate search, aggregation, and ordinal behavior from extraction.
    fn insert_fts(db: &Fixture, sid: &str, source: &str, message_index: Option<i64>, ordinal: i64, text: &str) {
        let conn=corpus::open(&db.app).unwrap();
        corpus::insert(&conn,sid,source,message_index,ordinal,None,None,None,text).unwrap();
    }

    fn search_in_project(db: &Fixture, query: &str, scope: SearchScope, session: Option<&str>, project: Option<&str>) -> Vec<SessionSearchHit> {
        let conn=corpus::open(&db.app).unwrap();
        let (query,ranked)=corpus::rank(&conn,query,scope,session,project).unwrap();
        let Some(query)=query else {return Vec::new();};
        let names=repo::list_all_sessions(&db.app.db().conn.lock().unwrap()).unwrap().into_iter()
            .map(|session|(session.id,(session.name,session.kind,session.archived_at.is_some()))).collect();
        ranked.iter().filter_map(|ranked|corpus::hit(&conn,&query,ranked,&names).unwrap()).collect()
    }
    fn search_in(db: &Fixture, query: &str, scope: SearchScope, session: Option<&str>) -> Vec<SessionSearchHit> {search_in_project(db,query,scope,session,None)}
    fn search(db: &Fixture, query: &str, scope: SearchScope) -> Vec<SessionSearchHit> {search_in(db,query,scope,None)}

    /// The word index answers first: `和服` matches the word but not the run `总和服务`, which the trigram
    /// index would also have accepted, and the matched literal is exactly the word.
    #[test]
    fn word_index_rejects_cross_word_substring() {
        let db = Fixture::new();
        insert_session(&db, "s1", "kimono", "claude");
        insert_session(&db, "s2", "billing", "claude");
        insert_fts(
            &db,
            "s1",
            "transcript",
            Some(0),
            0,
            "我买了一件和服，很好看。",
        );
        insert_fts(
            &db,
            "s2",
            "transcript",
            Some(0),
            0,
            "总和服务的费用还没有结清。",
        );

        let hits =
            search(&db, "和服", SearchScope::Live);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "s1");
        assert_eq!(hits[0].matches[0].matched, vec!["和服".to_string()]);
        assert!(hits[0].matches[0].snippet.contains("和服"));
    }

    /// A partial identifier has no word hit, so the trigram index takes over and matched carries the term.
    #[test]
    fn substring_fallback_when_no_word_matches() {
        let db = Fixture::new();
        insert_session(&db, "s1", "git", "claude");
        insert_fts(
            &db,
            "s1",
            "transcript",
            Some(0),
            0,
            "list every worktree before merging",
        );

        let hits =
            search(&db, "worktre", SearchScope::Live);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matches[0].matched, vec!["worktre".to_string()]);
    }

    #[test]
    fn session_filter_is_applied_before_results_are_built() {
        let db = Fixture::new();
        insert_session(&db, "s1", "first", "claude");
        insert_session(&db, "s2", "second", "codex");
        insert_fts(&db, "s1", "transcript", Some(0), 0, "shared needle");
        insert_fts(&db, "s2", "transcript", Some(4), 4, "another shared needle");

        let hits = search_in(&db, "needle", SearchScope::All, Some("s2"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "s2");
        assert_eq!(hits[0].matches[0].message_index, Some(4));
    }

    /// Porter stemming folds inflections and the matched literal is the word as written; a different
    /// identifier that merely contains the term is not a word hit.
    #[test]
    fn word_index_stems_english_and_reports_literal() {
        let db = Fixture::new();
        insert_session(&db, "s1", "orch", "claude");
        insert_session(&db, "s2", "cli", "claude");
        insert_fts(
            &db,
            "s1",
            "transcript",
            Some(0),
            0,
            "The lead agent spawned two children.",
        );
        insert_fts(
            &db,
            "s2",
            "transcript",
            Some(0),
            0,
            "Run vspawn to create a child session.",
        );

        let hits =
            search(&db, "spawn", SearchScope::Live);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "s1");
        assert_eq!(hits[0].matches[0].matched, vec!["spawned".to_string()]);
    }

    /// A punctuated identifier and a multi-word CJK phrase each match as one phrase and come back as one
    /// literal covering the whole run in the text.
    #[test]
    fn word_index_phrases_report_whole_literals() {
        let db = Fixture::new();
        insert_session(&db, "s1", "app", "claude");
        insert_fts(
            &db,
            "s1",
            "transcript",
            Some(0),
            0,
            "打开终端搜索功能，然后在 vlx-term 里试一下。",
        );

        let hits = search(
            &db,
            "终端搜索",
            SearchScope::Live,
        );
        assert_eq!(hits[0].matches[0].matched, vec!["终端搜索".to_string()]);
        let hits = search(
            &db,
            "vlx-term",
            SearchScope::Live,
        );
        assert_eq!(hits[0].matches[0].matched, vec!["vlx-term".to_string()]);
    }

    /// Recording: single-term matches receive document-order ordinals equal to findNext counts; snippets are full lines.
    #[test]
    fn recording_single_keyword_ordinals_in_doc_order() {
        let db = Fixture::new();
        insert_session(&db, "s1", "term one", "terminal");
        // Three lines, with deploy on lines 0 and 2.
        insert_fts(&db, "s1", "recording", None, 0, "starting deploy step");
        insert_fts(&db, "s1", "recording", None, 1, "nothing here");
        insert_fts(&db, "s1", "recording", None, 2, "another deploy line");

        let hits = search(
            &db,
            "deploy",
            SearchScope::All,
        );
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
        let db = Fixture::new();
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

        let hits = search(
            &db,
            "数据库 配置",
            SearchScope::All,
        );
        assert_eq!(hits.len(), 1);
        let h = &hits[0];
        assert_eq!(h.source, "transcript");
        assert_eq!(
            h.match_count, 1,
            "only a message containing both terms counts as a hit"
        );
        assert_eq!(h.matches[0].message_index, Some(3));
        assert!(h.matches[0].snippet.contains("数据库") || h.matches[0].snippet.contains("配置"));
    }

    /// A two-character Chinese term remains searchable through LIKE because trigram MATCH requires three characters.
    #[test]
    fn short_cjk_keyword_uses_like_fallback() {
        let db = Fixture::new();
        insert_session(&db, "s3", "term cjk", "terminal");
        insert_fts(&db, "s3", "recording", None, 0, "正在读取配置文件");
        insert_fts(&db, "s3", "recording", None, 1, "无关内容");

        let hits = search(
            &db,
            "配置",
            SearchScope::All,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].match_count, 1);
        assert_eq!(hits[0].matches[0].ordinal, 1);
    }

    /// Empty or whitespace-only queries return no results without scanning.
    #[test]
    fn empty_query_returns_empty() {
        let db = Fixture::new();
        assert!(search(
            &db,
            "   ",
            SearchScope::All
        )
        
        .is_empty());
    }

    /// Sessions are ranked by BM25 match density, and orphaned index rows are omitted.
    #[test]
    fn ranks_sessions_and_skips_orphans() {
        let db = Fixture::new();
        insert_session(&db, "a", "few", "terminal");
        insert_session(&db, "b", "many", "terminal");
        // Orphaned row with no corresponding session.
        insert_fts(&db, "ghost", "recording", None, 0, "deploy ghost only");
        insert_fts(&db, "a", "recording", None, 0, "one deploy here");
        insert_fts(&db, "b", "recording", None, 0, "deploy deploy deploy");
        insert_fts(&db, "b", "recording", None, 1, "more deploy again");

        let hits = search(
            &db,
            "deploy",
            SearchScope::All,
        );
        // Return a and b, but not ghost.
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.session_id != "ghost"));
        // b has denser matches and ranks first under BM25.
        assert_eq!(hits[0].session_id, "b");
    }

    /// Scope filtering returns live, archived, or all sessions as requested for both FTS and LIKE paths.
    #[test]
    fn scope_filters_by_archived() {
        let db = Fixture::new();
        insert_session(&db, "live1", "live term", "terminal");
        insert_session(&db, "arch1", "archived term", "terminal");
        // Mark arch1 as archived with a non-null archived_at value.
        {
            let conn = db.app.db().conn.lock().unwrap();
            conn.execute(
                "UPDATE sessions SET archived_at = 123 WHERE id = 'arch1'",
                [],
            )
            .unwrap();
        }
        // FTS path for a term of at least three characters: one hit in each session.
        insert_fts(&db, "live1", "recording", None, 0, "deploy on live");
        insert_fts(&db, "arch1", "recording", None, 0, "deploy on archived");

        let live = search(&db, "deploy", SearchScope::Live);
        assert_eq!(
            live.len(),
            1,
            "Live returns only sessions that are not archived"
        );
        assert_eq!(live[0].session_id, "live1");
        assert!(!live[0].archived);

        let arch = search(&db, "deploy", SearchScope::Archived);
        assert_eq!(arch.len(), 1, "Archived returns only archived sessions");
        assert_eq!(arch[0].session_id, "arch1");
        assert!(arch[0].archived);

        let all = search(&db, "deploy", SearchScope::All);
        assert_eq!(all.len(), 2, "All returns both");

        // LIKE fallback for a two-character Chinese term: scope filtering still applies.
        insert_fts(&db, "live1", "recording", None, 1, "查看配置");
        insert_fts(&db, "arch1", "recording", None, 1, "修改配置");
        let live_like = search(&db, "配置", SearchScope::Live);
        assert_eq!(
            live_like.len(),
            1,
            "the LIKE fallback filters by scope as well"
        );
        assert_eq!(live_like[0].session_id, "live1");
    }

    /// Project and collection filters apply in SQL before word/trigram/LIKE ranking and recursively
    /// expand nested collections, system collections, and archived sessions under tombstoned projects.
    #[test]
    fn project_and_collection_filters_apply_across_all_index_paths() {
        let db = Fixture::new();
        {
            let conn = db.app.db().conn.lock().unwrap();
            crate::db::system_collections::ensure(&conn).unwrap();
            conn.execute_batch("
                INSERT INTO projects(id,name,root_path,sort_order,created_at,collection_id,deleted_at) VALUES
                    ('col-work','Work','',0,0,'vlx-system-projects',NULL),
                    ('col-sub','Sub','',1,0,'col-work',NULL),
                    ('proj-a','Alpha','/a',2,0,'col-work',NULL),
                    ('proj-b','Beta','/b',3,0,'col-sub',NULL),
                    ('proj-tomb','Deleted','/old',4,0,'col-work',100),
                    ('proj-other','Other','/other',5,0,'vlx-system-projects',NULL),
                    ('proj-legacy','Legacy','/legacy',6,0,NULL,NULL);
                INSERT INTO sessions(id,project_id,name,kind,sort_order,created_at,archived_at) VALUES
                    ('s-col','col-work','Direct','claude',0,0,NULL),
                    ('s-a','proj-a','Alpha','claude',0,0,NULL),
                    ('s-b','proj-b','Beta','claude',0,0,NULL),
                    ('s-tomb','proj-tomb','Archived','claude',0,0,200),
                    ('s-other','proj-other','Other','claude',0,0,NULL),
                    ('s-legacy','proj-legacy','Legacy','claude',0,0,NULL),
                    ('s-task','vlx-system-tasks','Task','claude',0,0,NULL),
                    ('s-chat','vlx-system-chat','Chat','claude',0,0,NULL);
            ").unwrap();
        }
        // proj-other has whole-word "work"; proj-a only has substring "worktree", proving project
        // filtering runs before deciding whether to fall back from words to trigram.
        insert_fts(&db, "s-col", "transcript", Some(0), 0, "deploy worktree 配置");
        insert_fts(&db, "s-a", "transcript", Some(0), 0, "deploy worktree 配置");
        insert_fts(&db, "s-b", "transcript", Some(0), 0, "deploy worktree 配置");
        insert_fts(&db, "s-tomb", "transcript", Some(0), 0, "deploy archived 配置");
        insert_fts(&db, "s-other", "transcript", Some(0), 0, "deploy work 配置");
        insert_fts(&db, "s-legacy", "transcript", Some(0), 0, "deploy legacy");
        insert_fts(&db, "s-task", "transcript", Some(0), 0, "deploy task 配置");
        insert_fts(&db, "s-chat", "transcript", Some(0), 0, "deploy chat 配置");

        let ids = |hits: Vec<SessionSearchHit>| {
            let mut out: Vec<_> = hits.into_iter().map(|h| h.session_id).collect();
            out.sort();
            out
        };

        // Single project: words, trigram fallback despite whole-word hit elsewhere, and LIKE fallback.
        assert_eq!(ids(search_in_project(&db, "deploy", SearchScope::Live, None, Some("proj-a"))), ["s-a"]);
        assert_eq!(ids(search_in_project(&db, "work", SearchScope::Live, None, Some("proj-a"))), ["s-a"]);
        assert_eq!(ids(search_in_project(&db, "配置", SearchScope::Live, None, Some("proj-a"))), ["s-a"]);

        // Collection includes direct sessions, child projects, and nested sub-collections; archived
        // sessions under tombstoned child projects appear when scope is All.
        assert_eq!(ids(search_in_project(&db, "deploy", SearchScope::Live, None, Some("col-work"))), ["s-a", "s-b", "s-col"]);
        assert_eq!(ids(search_in_project(&db, "deploy", SearchScope::All, None, Some("col-work"))), ["s-a", "s-b", "s-col", "s-tomb"]);
        assert_eq!(ids(search_in_project(&db, "配置", SearchScope::Live, None, Some("col-sub"))), ["s-b"]);

        // System collections: Projects gathers all non-task/chat projects (including unmigrated NULL),
        // while Tasks and Chat stay isolated.
        assert_eq!(
            ids(search_in_project(&db, "deploy", SearchScope::Live, None, Some(crate::db::system_collections::PROJECTS))),
            ["s-a", "s-b", "s-col", "s-legacy", "s-other"]
        );
        assert_eq!(
            ids(search_in_project(&db, "deploy", SearchScope::Live, None, Some(crate::db::system_collections::TASKS))),
            ["s-task"]
        );
        assert_eq!(
            ids(search_in_project(&db, "deploy", SearchScope::Live, None, Some(crate::db::system_collections::CHAT))),
            ["s-chat"]
        );
        assert!(search_in_project(&db, "deploy", SearchScope::All, None, Some("nonexistent")).is_empty());
    }
}
