//! Persistent global search index, kept in its own derived file outside the business database.
//!
//! One row is one locatable fragment: a block of at most [`CHUNK_BYTES`] of a transcript message, or one
//! visible recording line. Each fragment goes into the trigram table `session_fts` and, under the same
//! rowid, into the word table `session_words` with jieba boundaries marked; the query side keeps the
//! semantics of the earlier in-database index (words first, then trigram, then LIKE for short terms).
//!
//! Transcripts are read straight from their files, and OpenCode and Kiro from their own databases
//! ([`super::extract`]); searching a conversation never builds a history index for it.
//! A session is indexed in one transaction, so a search sees either its previous rows or its new rows.
//! Each session records a fingerprint of its source; an unchanged session costs one file stat, and an
//! append that keeps the history generation re-indexes only from the last indexed message on.
//!
//! The file can be deleted at any time: it is rebuilt from the original recordings.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path,PathBuf};
use std::sync::{Arc,Mutex,OnceLock};
use std::sync::atomic::{AtomicBool,AtomicUsize,Ordering};

use rusqlite::{params,params_from_iter,Connection,OptionalExtension};

use crate::agent::transcript;
use crate::host::AppCtx;
use crate::models::SessionKind;
use super::{tokenize,SearchMatch,SearchScope,SessionSearchHit,limit_chars};

/// Extraction and schema version. Each version builds its own file; until that file completes a
/// background pass, searches read an earlier version's file for the sessions it has not indexed yet,
/// and the pass that completes removes the earlier files.
const VERSION:i64=5;
/// Oldest version whose `session_fts` and `session_words` rows, and `corpus_state` fingerprints, this
/// code reads with the same meaning, message numbering included. Raise it when a version changes them.
/// Version 5 numbers a forked Codex rollout from the start of its parent's history but leaves this at 1:
/// only forks are renumbered, their earlier hits still match, and the first complete pass retires them,
/// whereas raising it would leave every session out of search until that pass ends.
const QUERYABLE_SINCE:i64=1;
/// Bytes of message text per fragment. Phrase and AND matches hold within one fragment.
pub(super) const CHUNK_BYTES:usize=64*1024;
/// Snippets returned per session; further fragments only count toward `match_count`.
pub(super) const MAX_MATCHES:usize=50;
/// Minimum term length for trigram MATCH; shorter terms use LIKE.
const MIN_FTS_TERM_CHARS:usize=3;
/// Bytes of readable context kept on each side of a match in a snippet.
const SNIPPET_CONTEXT:usize=40;

const SCHEMA:&str="
CREATE TABLE IF NOT EXISTS corpus_meta(version INTEGER NOT NULL);
CREATE VIRTUAL TABLE IF NOT EXISTS session_fts USING fts5(
  text, session_id UNINDEXED, source UNINDEXED, message_index UNINDEXED, ordinal UNINDEXED, role UNINDEXED, ts UNINDEXED,
  record_offset UNINDEXED, tokenize = 'trigram remove_diacritics 1');
CREATE VIRTUAL TABLE IF NOT EXISTS session_words USING fts5(
  words, content = '', contentless_delete = 1, tokenize = 'porter unicode61 remove_diacritics 1');
CREATE TABLE IF NOT EXISTS corpus_state(
  session_id TEXT PRIMARY KEY, source TEXT NOT NULL, path TEXT, length INTEGER NOT NULL, modified INTEGER,
  generation TEXT, messages INTEGER NOT NULL, indexed_at INTEGER NOT NULL, resume TEXT);
CREATE TABLE IF NOT EXISTS corpus_built(at INTEGER NOT NULL);
";

fn sql(error:rusqlite::Error) -> String {
    if matches!(error,rusqlite::Error::SqliteFailure(rusqlite::ffi::Error {code:rusqlite::ErrorCode::DiskFull,..},_)) {
        "storage_pressure: the search index has no space left".into()
    } else {format!("Search index operation failed: {error}")}
}
/// Return every free page to the file system. `incremental_vacuum` frees one page per step, so a
/// single `execute_batch` would release just one page and leave the file growing.
pub(crate) fn reclaim_free_pages(conn:&Connection) -> rusqlite::Result<()> {
    let mut statement=conn.prepare("PRAGMA incremental_vacuum")?;let mut rows=statement.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}
fn check(cancel:&AtomicBool) -> Result<(),String> {if cancel.load(Ordering::Relaxed) {Err("read_canceled".into())} else {Ok(())}}
fn now() -> i64 {std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs() as i64).unwrap_or(0)}

pub(crate) fn path(data_dir:&Path) -> PathBuf {file(data_dir,VERSION)}
/// Version 2 kept the unversioned name it was released under; other versions carry theirs.
fn file(data_dir:&Path,version:i64) -> PathBuf {
    data_dir.join("search-index").join(if version==2 {"corpus.sqlite".into()} else {format!("corpus-v{version}.sqlite")})
}

/// `file:` URI for attaching the business database read-only.
fn read_only_uri(path:&str) -> String {
    let mut uri=String::from("file:");
    for ch in path.chars() {match ch {'%'=>uri.push_str("%25"),' '=>uri.push_str("%20"),'?'=>uri.push_str("%3f"),'#'=>uri.push_str("%23"),_=>uri.push(ch)}}
    uri.push_str("?mode=ro");uri
}

/// Open the index with the business database attached as `app`, rebuilding it when its version changed.
pub(super) fn open(app:&AppCtx) -> Result<Connection,String> {
    let file=path(&app.data_dir()?);
    std::fs::create_dir_all(file.parent().unwrap()).map_err(|_|"storage_pressure: the search index directory is unavailable")?;
    let conn=Connection::open(&file).map_err(sql)?;
    conn.busy_timeout(std::time::Duration::from_secs(30)).map_err(sql)?;
    // auto_vacuum must be chosen before the first table exists; it is a no-op on an existing file.
    conn.execute_batch("PRAGMA auto_vacuum=INCREMENTAL; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;").map_err(sql)?;
    let version:Option<i64>=conn.query_row("SELECT version FROM corpus_meta",[],|r|r.get(0)).optional().unwrap_or(None);
    if version!=Some(VERSION) {
        conn.execute_batch("DROP TABLE IF EXISTS session_words; DROP TABLE IF EXISTS session_fts; DROP TABLE IF EXISTS corpus_state;
            DROP TABLE IF EXISTS corpus_meta;").map_err(sql)?;
        conn.execute_batch(SCHEMA).map_err(sql)?;
        conn.execute("INSERT INTO corpus_meta VALUES(?1)",[VERSION]).map_err(sql)?;
        reclaim_free_pages(&conn).map_err(sql)?;
    }
    // Files from before the completion marker gain it here.
    conn.execute_batch("CREATE TABLE IF NOT EXISTS corpus_built(at INTEGER NOT NULL);").map_err(sql)?;
    let main=app.db().conn.lock().unwrap().path().map(str::to_owned).filter(|path|!path.is_empty())
        .ok_or("Search requires a file-backed application database")?;
    conn.execute("ATTACH DATABASE ?1 AS app",[read_only_uri(&main)]).map_err(sql)?;
    Ok(conn)
}

/// Index files of earlier versions, newest first.
fn earlier(data_dir:&Path) -> Vec<(i64,PathBuf)> {
    let current=path(data_dir);
    let Ok(entries)=std::fs::read_dir(data_dir.join("search-index")) else {return Vec::new();};
    let mut found:Vec<(i64,PathBuf)>=entries.flatten().map(|entry|entry.path())
        .filter(|candidate|*candidate!=current && candidate.file_name().and_then(|name|name.to_str())
            .is_some_and(|name|name=="corpus.sqlite" || name.starts_with("corpus-v") && name.ends_with(".sqlite")))
        .filter_map(|candidate| {
            let conn=Connection::open_with_flags(&candidate,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
            let version:i64=conn.query_row("SELECT version FROM corpus_meta",[],|r|r.get(0)).ok()?;
            (version<VERSION).then_some((version,candidate))
        }).collect();
    found.sort_by(|a,b|b.0.cmp(&a.0));found
}

/// Whether this version's file has completed a background pass.
fn built(conn:&Connection) -> Result<bool,String> {conn.query_row("SELECT EXISTS(SELECT 1 FROM main.corpus_built)",[],|r|r.get(0)).map_err(sql)}

/// While this version's file is still being built, attach the newest earlier file it can read as `old`,
/// so sessions not indexed here yet are still found. A file that cannot be attached is simply not used.
pub(super) fn attach_earlier(app:&AppCtx,conn:&Connection) -> Result<(),String> {
    if built(conn)? || previous_attached(conn)? {return Ok(());}
    if let Some((_,earlier))=earlier(&app.data_dir()?).into_iter().find(|(version,_)|*version>=QUERYABLE_SINCE) {
        let _=conn.execute("ATTACH DATABASE ?1 AS old",[read_only_uri(&earlier.to_string_lossy())]);
    }
    Ok(())
}
fn previous_attached(conn:&Connection) -> Result<bool,String> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM pragma_database_list WHERE name='old')",[],|r|r.get(0)).map_err(sql)
}

/// Record that this version's file holds every session, then remove the earlier files.
pub(super) fn mark_built(app:&AppCtx,conn:&Connection) -> Result<(),String> {
    if !built(conn)? {conn.execute("INSERT INTO corpus_built VALUES(?1)",[now()]).map_err(sql)?;}
    remove_earlier(app);Ok(())
}
/// Remove earlier versions' files once this one is built. A file still open elsewhere (Windows) stays
/// until a later start.
pub(super) fn remove_earlier(app:&AppCtx) {
    let Ok(data_dir)=app.data_dir() else {return;};
    let Ok(conn)=Connection::open_with_flags(path(&data_dir),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else {return;};
    if !built(&conn).unwrap_or(false) {return;}
    for (_,earlier) in earlier(&data_dir) {
        for suffix in ["-wal","-shm",""] {let mut name=earlier.clone().into_os_string();name.push(suffix);let _=std::fs::remove_file(name);}
    }
}

#[derive(Clone)]
pub(super) struct Session {pub id:String,pub name:String,pub kind:SessionKind,pub native:Option<String>,pub archived:bool}

/// Sessions in id order after `after`, 64 at a time, with every text field bounded before it is read.
pub(super) fn sessions(app:&AppCtx,after:Option<&str>) -> Result<Vec<Session>,String> {
    let conn=app.db().conn.lock().unwrap();
    let mut statement=conn.prepare("SELECT substr(id,1,128),substr(name,1,4096),kind,substr(agent_session_id,1,4096),archived_at IS NOT NULL,
        length(CAST(id AS BLOB))>128 OR length(CAST(name AS BLOB))>4096 OR length(CAST(agent_session_id AS BLOB))>4096
        FROM sessions WHERE (?1 IS NULL OR id>?1) ORDER BY id LIMIT 64").map_err(sql)?;
    let rows=statement.query_map([after],|r|Ok((Session {id:r.get(0)?,name:r.get(1)?,kind:SessionKind::from_db(&r.get::<_,String>(2)?),native:r.get(3)?,archived:r.get(4)?},r.get::<_,Option<bool>>(5)?.unwrap_or(false)))).map_err(sql)?;
    rows.map(|row|row.map_err(sql).and_then(|(session,large)|if large {Err("unsupported_record: session metadata exceeds its byte budget".into())} else {Ok(session)})).collect()
}

#[derive(Clone,PartialEq)]
struct State {source:String,path:Option<String>,length:i64,modified:Option<i64>,generation:Option<String>,messages:i64,resume:Option<String>}

fn state(conn:&Connection,session:&str) -> Result<Option<State>,String> {
    conn.query_row("SELECT source,path,length,modified,generation,messages,resume FROM corpus_state WHERE session_id=?1",[session],
        |r|Ok(State {source:r.get(0)?,path:r.get(1)?,length:r.get(2)?,modified:r.get(3)?,generation:r.get(4)?,messages:r.get(5)?,resume:r.get(6)?})).optional().map_err(sql)
}
fn stat(path:&Path) -> Option<(i64,Option<i64>)> {
    let meta=std::fs::metadata(path).ok().filter(|meta|meta.is_file())?;
    let modified=meta.modified().ok().and_then(|time|time.duration_since(std::time::UNIX_EPOCH).ok()).map(|d|d.as_secs() as i64);
    Some((meta.len() as i64,modified))
}

/// Where a session's searchable text comes from, resolved with at most one directory lookup.
enum Track {Transcript(PathBuf),Recording(PathBuf),None}
fn track(app:&AppCtx,session:&Session,previous:Option<&State>) -> Result<Track,String> {
    if let Some(id)=session.native.as_deref().map(str::trim).filter(|id|!id.is_empty()) {
        if session.kind==SessionKind::Kiro {return Ok(Track::Transcript(PathBuf::new()));}
        // The recorded path costs one stat; the per-kind lookup walks the agent's whole session tree.
        let remembered=previous.filter(|state|state.source=="transcript" || state.source=="failed").and_then(|state|state.path.clone()).map(PathBuf::from).filter(|path|path.is_file());
        if let Some(path)=remembered.or_else(||transcript::source_path(session.kind,id).filter(|path|path.is_file())) {return Ok(Track::Transcript(path));}
        // A recording the conversation view opened in this process stays searchable after the provider's
        // directory catalogue changes; only the path held in memory is reused.
        if let Some(path)=crate::agent::history_store::store::known_path(&app.data_dir()?,session.kind,id).filter(|path|path.is_file()) {
            return Ok(Track::Transcript(path));
        }
    }
    let recording=app.data_dir()?.join("recordings").join(format!("{}.log",session.id));
    Ok(if recording.is_file() {Track::Recording(recording)} else {Track::None})
}

fn delete_rows(conn:&Connection,session:&str,from_message:Option<i64>) -> Result<(),String> {
    conn.execute("DELETE FROM session_words WHERE rowid IN (SELECT rowid FROM session_fts WHERE session_id=?1 AND (?2 IS NULL OR message_index>=?2))",
        params![session,from_message]).map_err(sql)?;
    conn.execute("DELETE FROM session_fts WHERE session_id=?1 AND (?2 IS NULL OR message_index>=?2)",params![session,from_message]).map_err(sql)?;Ok(())
}
/// One fragment. `offset` is where its message's first record begins in the transcript file, when the
/// message was read from that file directly.
#[allow(clippy::too_many_arguments)]
pub(super) fn insert(conn:&Connection,session:&str,source:&str,message:Option<i64>,ordinal:i64,role:Option<&str>,ts:Option<&str>,offset:Option<u64>,text:&str) -> Result<(),String> {
    if !text.chars().any(|ch|!ch.is_whitespace()) {return Ok(());}
    conn.execute("INSERT INTO session_fts(text,session_id,source,message_index,ordinal,role,ts,record_offset) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![text,session,source,message,ordinal,role,ts,offset]).map_err(sql)?;
    conn.execute("INSERT INTO session_words(rowid,words) VALUES(?1,?2)",
        params![conn.last_insert_rowid(),tokenize::boundary_marked(text)]).map_err(sql)?;Ok(())
}

/// Read `input` as UTF-8 text in blocks of at most [`CHUNK_BYTES`], ending a full block after its last
/// line break when one is near its end so a block usually ends between sentences.
fn chunks(input:&mut impl Read,cancel:&AtomicBool,mut each:impl FnMut(&str)->Result<(),String>) -> Result<(),String> {
    let mut pending:Vec<u8>=Vec::with_capacity(CHUNK_BYTES);let mut buffer=vec![0;CHUNK_BYTES];let mut done=false;
    loop {
        check(cancel)?;
        while pending.len()<CHUNK_BYTES && !done {
            let count=input.read(&mut buffer[..CHUNK_BYTES-pending.len()]).map_err(|e|e.to_string())?;
            if count==0 {done=true;} else {pending.extend_from_slice(&buffer[..count]);}
        }
        if pending.is_empty() {return Ok(());}
        let complete=match std::str::from_utf8(&pending) {Ok(text)=>text.len(),Err(error) if error.error_len().is_none() && !done=>error.valid_up_to(),
            Err(_)=>return Err("source_changed: search text is not valid UTF-8".into())};
        let mut end=complete;
        if !done || complete<pending.len() {
            let text=std::str::from_utf8(&pending[..complete]).unwrap();
            let from=ceil_char_boundary(text,complete.saturating_sub(4096));
            if let Some(at)=text[from..].rfind('\n').map(|at|from+at+1) {end=at;}
        }
        if end==0 {end=complete.max(1);}
        each(std::str::from_utf8(&pending[..end]).map_err(|_|"source_changed: search text is not valid UTF-8")?)?;
        pending.drain(..end);
        if done && pending.is_empty() {return Ok(());}
    }
}

/// Index every visible line of a terminal recording.
fn index_recording(conn:&Connection,session:&str,path:&Path,cancel:&AtomicBool) -> Result<(),String> {
    delete_rows(conn,session,None)?;
    let mut file=std::fs::File::open(path).map_err(|_|"source_missing: recording is unavailable")?;
    let mut stripper=super::ansi::AnsiStripper::new();let mut ordinal=0i64;let mut failed=None;
    let mut line=|text:&str| {
        if failed.is_some() {return;}
        for piece in text.as_bytes().chunks(CHUNK_BYTES) {
            let piece=String::from_utf8_lossy(piece);
            if let Err(error)=insert(conn,session,"recording",None,ordinal,None,None,None,piece.trim()) {failed=Some(error);return;}
        }
        if text.chars().any(|ch|!ch.is_whitespace()) {ordinal+=1;}
    };
    let mut bytes=vec![0;CHUNK_BYTES];
    loop {
        check(cancel)?;let count=file.read(&mut bytes).map_err(|e|e.to_string())?;if count==0 {break;}
        stripper.feed(&bytes[..count],&mut line);
    }
    stripper.finish(&mut line);
    failed.map_or(Ok(()),Err)
}

/// Refresh coordination for one index file: one refresher at a time, since every refresh writes it;
/// how many searches are bringing sessions up to date; and whether a background pass walks them all.
#[derive(Default)]
struct Activity {gate:Mutex<()>,searching:AtomicUsize,background:AtomicBool}
fn activity(app:&AppCtx) -> Result<Arc<Activity>,String> {
    static ALL:OnceLock<Mutex<HashMap<PathBuf,Arc<Activity>>>>=OnceLock::new();
    let file=path(&app.data_dir()?);
    Ok(ALL.get_or_init(Default::default).lock().unwrap_or_else(|poisoned|poisoned.into_inner()).entry(file).or_default().clone())
}

/// Held by a search for the length of its refresh pass. The background pass starts no new session
/// while one is held, so a search waits at most for the session already being read.
pub(super) struct Searching(Arc<Activity>);
impl Searching {
    pub(super) fn begin(app:&AppCtx) -> Result<Self,String> {let activity=activity(app)?;activity.searching.fetch_add(1,Ordering::SeqCst);Ok(Self(activity))}
}
impl Drop for Searching {fn drop(&mut self) {self.0.searching.fetch_sub(1,Ordering::SeqCst);}}

/// Held by the background pass; None when another pass over the same index is already running.
pub(super) struct Background(Arc<Activity>);
impl Background {
    pub(super) fn begin(app:&AppCtx) -> Option<Self> {
        let activity=activity(app).ok()?;
        activity.background.compare_exchange(false,true,Ordering::SeqCst,Ordering::SeqCst).ok().map(|_|Self(activity))
    }
    /// Wait while a search is refreshing, so a search never queues behind a whole pass.
    pub(super) fn yield_to_searches(&self) {while self.0.searching.load(Ordering::SeqCst)>0 {std::thread::sleep(std::time::Duration::from_millis(20));}}
}
impl Drop for Background {fn drop(&mut self) {self.0.background.store(false,Ordering::SeqCst);}}

/// What a refresh did; `Indexed` carries the source size that was read. `Deferred` sessions were left
/// for the background pass and may be missing from, or behind in, the results.
pub(super) enum Refreshed {Unchanged,Indexed(u64),Removed,Deferred}

/// Bring one session's rows up to date with its source.
pub(super) fn refresh_session(app:&AppCtx,conn:&Connection,session:&Session,cancel:&Arc<AtomicBool>) -> Result<Refreshed,String> {
    let activity=activity(app)?;let _gate=activity.gate.lock().unwrap_or_else(|poisoned|poisoned.into_inner());
    refresh_locked(app,conn,session,cancel)
}

/// A search's refresh. An unchanged session costs its stat without waiting for another refresher. While
/// a background pass runs, a session that was never indexed is left to it. Reading a changed session
/// stops when `budget` trips, which the search sets when its time is up; such a session is deferred and
/// the search answers from what is already indexed.
pub(super) fn refresh_for_search(app:&AppCtx,conn:&Connection,session:&Session,cancel:&Arc<AtomicBool>,budget:&Arc<AtomicBool>) -> Result<Refreshed,String> {
    check(cancel)?;
    let activity=activity(app)?;
    let previous=state(conn,&session.id)?;
    let mut current=None;
    if session.kind!=SessionKind::Kiro {
        let track=track(app,session,previous.as_ref())?;
        current=match &track {Track::Transcript(path)|Track::Recording(path)=>stat(path).map(|(length,modified)|(path.to_string_lossy().into_owned(),length,modified)),Track::None=>None};
        let unchanged=match (&previous,current.clone()) {
            (None,None)=>true,
            (Some(state),Some((path,length,modified)))=>state.path.as_deref()==Some(path.as_str()) && state.length==length && state.modified==modified
                && (state.source=="failed" || state.source==if matches!(track,Track::Recording(_)) {"recording"} else {"transcript"}),
            _=>false,
        };
        if unchanged {return Ok(Refreshed::Unchanged);}
    }
    if previous.is_none() && activity.background.load(Ordering::SeqCst) {
        // An earlier version's file that holds the session as it is now answers for it until the pass gets here.
        let covered=match current {
            Some((path,length,modified)) if previous_attached(conn)? => conn.query_row("SELECT EXISTS(SELECT 1 FROM old.corpus_state WHERE session_id=?1 AND source IN ('transcript','recording')
                AND path=?2 AND length=?3 AND modified IS ?4)",params![session.id,path,length,modified],|r|r.get(0)).unwrap_or(false),
            _=>false,
        };
        return Ok(if covered {Refreshed::Unchanged} else {Refreshed::Deferred});
    }
    let _gate=loop {
        check(cancel)?;
        match activity.gate.try_lock() {
            Ok(guard)=>break guard,
            Err(std::sync::TryLockError::Poisoned(poisoned))=>break poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock)=> {
                if budget.load(Ordering::Relaxed) {return Ok(Refreshed::Deferred);}
                std::thread::sleep(std::time::Duration::from_millis(10));
            },
        }
    };
    match refresh_locked(app,conn,session,budget) {
        Err(error) if error=="read_canceled" && !cancel.load(Ordering::Relaxed)=>Ok(Refreshed::Deferred),
        result=>result,
    }
}

/// Holds the indexer of `app`'s index, as a long refresh would.
#[cfg(test)]
pub(super) fn hold_indexer<T>(app:&AppCtx,during:impl FnOnce()->T) -> T {
    let activity=activity(app).unwrap();let _gate=activity.gate.lock().unwrap();during()
}

fn refresh_locked(app:&AppCtx,conn:&Connection,session:&Session,cancel:&Arc<AtomicBool>) -> Result<Refreshed,String> {
    check(cancel)?;
    let previous=state(conn,&session.id)?;
    let track=track(app,session,previous.as_ref())?;
    let (source,path)=match &track {Track::Transcript(path)=>("transcript",path.clone()),Track::Recording(path)=>("recording",path.clone()),Track::None=> {
        if previous.is_none() {return Ok(Refreshed::Unchanged);}
        conn.execute_batch("BEGIN IMMEDIATE").map_err(sql)?;
        let result=delete_rows(conn,&session.id,None).and_then(|_|conn.execute("DELETE FROM corpus_state WHERE session_id=?1",[&session.id]).map(|_|()).map_err(sql));
        return finish(conn,result).map(|_|Refreshed::Removed);
    }};
    let kiro=session.kind==SessionKind::Kiro;
    let (length,modified)=if kiro {(0,None)} else {stat(&path).ok_or("source_missing: search source is unavailable")?};
    let path_text=(!kiro).then(||path.to_string_lossy().into_owned());
    if !kiro && previous.as_ref().is_some_and(|state|state.source==source && state.path==path_text && state.length==length && state.modified==modified) {
        return Ok(Refreshed::Unchanged);
    }
    if source=="recording" {
        conn.execute_batch("BEGIN IMMEDIATE").map_err(sql)?;
        let result=index_recording(conn,&session.id,&path,cancel).and_then(|_|write_state(conn,&session.id,&State {
            source:source.into(),path:path_text,length,modified,generation:None,messages:0,resume:None}));
        return finish(conn,result).map(|_|Refreshed::Indexed(length as u64));
    }
    // A recording that failed for what it contains fails the same way until it changes.
    if !kiro && previous.as_ref().is_some_and(|state|state.source=="failed" && state.path==path_text && state.length==length && state.modified==modified) {
        return Ok(Refreshed::Unchanged);
    }
    let result=index_transcript(conn,session,&path,previous.as_ref(),(path_text.clone(),length,modified),cancel);
    if let Err(error)=&result {
        if ["index_incomplete:","unsupported_record:","detail_unavailable:"].iter().any(|code|error.starts_with(code)) {
            // The rows already indexed stay, and so does their resume point: a conversation read while
            // its last record was still being written reads on from there once it changes, rather than
            // from the first message.
            let (generation,messages,resume)=previous.filter(|state|state.path==path_text)
                .map_or((None,0,None),|state|(state.generation,state.messages,state.resume));
            write_state(conn,&session.id,&State {source:"failed".into(),path:path_text,length,modified,generation,messages,resume})?;
        }
    }
    result
}

fn index_transcript(conn:&Connection,session:&Session,path:&Path,previous:Option<&State>,(path_text,length,modified):(Option<String>,i64,Option<i64>),
    cancel:&Arc<AtomicBool>) -> Result<Refreshed,String> {
    if super::extract::supported(session.kind) {return index_file(conn,session,path,previous,(path_text,length,modified),cancel);}
    let Some(id)=session.native.as_deref().map(str::trim).filter(|id|!id.is_empty()) else {return Ok(Refreshed::Unchanged);};
    let Some((messages,revision))=super::extract::native_messages(session.kind,id,cancel)? else {return Ok(Refreshed::Unchanged);};
    let state=State {source:"transcript".into(),path:path_text,length,modified,generation:Some(revision),messages:messages.len() as i64,resume:None};
    conn.execute_batch("BEGIN IMMEDIATE").map_err(sql)?;
    // The database changes with every conversation it holds; one whose messages are the same keeps its rows.
    if previous.is_some_and(|previous|previous.source=="transcript" && previous.generation==state.generation) {
        return finish(conn,write_state(conn,&session.id,&state)).map(|_|Refreshed::Unchanged);
    }
    let result=(|| {
        delete_rows(conn,&session.id,None)?;
        for message in &messages {
            let mut part=0i64;let number=message.number as i64;
            chunks(&mut message.text.as_bytes(),cancel,|text| {
                insert(conn,&session.id,"transcript",Some(number),(number<<20)+part,Some(message.role),message.timestamp.as_deref(),None,text)?;part+=1;Ok(())
            })?;
        }
        write_state(conn,&session.id,&state)
    })();
    finish(conn,result).map(|_|Refreshed::Indexed(length.max(0) as u64))
}

/// Index a transcript from its file. A conversation that only grew is read again from
/// where its last message began, when nothing in front of that message changed.
fn index_file(conn:&Connection,session:&Session,path:&Path,previous:Option<&State>,(path_text,length,modified):(Option<String>,i64,Option<i64>),
    cancel:&Arc<AtomicBool>) -> Result<Refreshed,String> {
    if session.native.as_deref().map(str::trim).is_none_or(str::is_empty) {return Ok(Refreshed::Unchanged);}
    let transcript=super::extract::Transcript::open(session.kind,path,cancel)?;
    let grown=previous.filter(|state|(state.source=="transcript" || state.source=="failed") && state.path==path_text && length>=state.length);
    let resume=grown.and_then(|state|state.resume.as_deref()).and_then(|raw|serde_json::from_str::<super::extract::Resume>(raw).ok())
        .filter(|resume|grown.is_some_and(|state|resume.number as i64==state.messages-1));
    let resume=match resume {Some(resume) if transcript.resumes(&resume)? => Some(resume),_=>None};
    conn.execute_batch("BEGIN IMMEDIATE").map_err(sql)?;
    let result=(|| {
        delete_rows(conn,&session.id,resume.as_ref().map(|resume|resume.number as i64).filter(|number|*number>0))?;
        let (total,last)=transcript.messages(resume.as_ref(),cancel,|message| {
            let mut part=0i64;let number=message.number as i64;
            chunks(&mut message.text.as_bytes(),cancel,|text| {
                // A forked rollout's offsets span several files, so its hits are located by message number.
                insert(conn,&session.id,"transcript",Some(number),(number<<20)+part,Some(message.role),message.timestamp.as_deref(),(!transcript.chained()).then_some(message.offset),text)?;
                part+=1;Ok(())
            })
        })?;
        let resume=last.map(|last|serde_json::to_string(&last)).transpose().map_err(|e|e.to_string())?;
        write_state(conn,&session.id,&State {source:"transcript".into(),path:path_text.clone(),length,modified,generation:None,messages:total as i64,resume})
    })();
    finish(conn,result).map(|_|Refreshed::Indexed(length as u64))
}

fn write_state(conn:&Connection,session:&str,state:&State) -> Result<(),String> {
    conn.execute("INSERT OR REPLACE INTO corpus_state VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![session,state.source,state.path,state.length,state.modified,state.generation,state.messages,now(),state.resume]).map_err(sql)?;Ok(())
}
fn finish(conn:&Connection,result:Result<(),String>) -> Result<(),String> {
    match result {
        Ok(())=>{conn.execute_batch("COMMIT").map_err(sql)?;reclaim_free_pages(conn).map_err(sql)},
        Err(error)=>{let _=conn.execute_batch("ROLLBACK");Err(error)},
    }
}

/// Remove rows of sessions that no longer exist in the business database.
pub(super) fn prune(conn:&Connection) -> Result<(),String> {
    conn.execute_batch("BEGIN IMMEDIATE;
        DELETE FROM session_words WHERE rowid IN (SELECT rowid FROM session_fts WHERE session_id NOT IN (SELECT id FROM app.sessions));
        DELETE FROM session_fts WHERE session_id NOT IN (SELECT id FROM app.sessions);
        DELETE FROM corpus_state WHERE session_id NOT IN (SELECT id FROM app.sessions);
        COMMIT;").map_err(sql)?;
    reclaim_free_pages(conn).map_err(sql)
}

/// Delete removed sessions' rows right away rather than at the next prune.
pub fn drop_sessions(app:&AppCtx,ids:&[String]) -> Result<(),String> {
    if ids.is_empty() || !path(&app.data_dir()?).is_file() {return Ok(());}
    let conn=open(app)?;conn.execute_batch("BEGIN IMMEDIATE").map_err(sql)?;
    let result=ids.iter().try_for_each(|id| {
        delete_rows(&conn,id,None)?;conn.execute("DELETE FROM corpus_state WHERE session_id=?1",[id]).map(|_|()).map_err(sql)
    });
    finish(&conn,result)
}

/// Merge fragmented FTS segments after many updates so the file stays compact.
pub(super) fn optimize(conn:&Connection) -> Result<(),String> {
    conn.execute_batch("INSERT INTO session_fts(session_fts) VALUES('optimize'); INSERT INTO session_words(session_words) VALUES('optimize');").map_err(sql)?;
    reclaim_free_pages(conn).map_err(sql)
}

// ─────────────────────────── Query ───────────────────────────

/// Which index answered the query; it decides snippet placement, matched literals and ordering.
#[derive(Clone,Copy,PartialEq,Eq,Debug)]
pub(super) enum Answered {Words,Trigram,Like}
impl Answered {pub(super) fn code(self) -> usize {match self {Self::Words=>0,Self::Trigram=>1,Self::Like=>2}}}

/// A matching session in result order, without its snippets.
#[derive(Clone,Debug)]
pub(super) struct Ranked {pub session:String,pub count:u32,pub source:String}

pub(super) struct Query {pub keys:Vec<String>,pub answered:Answered,expression:String}

fn scope_filter(scope:SearchScope) -> &'static str {
    match scope {SearchScope::All=>"",SearchScope::Live=>" AND s.archived_at IS NULL",SearchScope::Archived=>" AND s.archived_at IS NOT NULL"}
}

fn fts_match_expr(keys:&[String]) -> String {
    keys.iter().map(|k|format!("\"{}\"",k.replace('"',"\"\""))).collect::<Vec<_>>().join(" ")
}
fn like_escape(k:&str) -> String {
    let mut out=String::with_capacity(k.len());
    for c in k.chars() {if c=='\\' || c=='%' || c=='_' {out.push('\\');}out.push(c);}out
}

/// The fragment tables a query reads, each with the condition that limits it: this version's, and
/// while it is being built an earlier file's, for the sessions this one has not indexed yet.
fn arms(conn:&Connection) -> Result<Vec<(&'static str,&'static str)>,String> {
    let mut arms=vec![("main","")];
    if previous_attached(conn)? {arms.push(("old"," AND f.session_id NOT IN (SELECT session_id FROM main.corpus_state)"));}
    Ok(arms)
}
/// The file holding a session's rows.
fn holder(conn:&Connection,session:&str) -> Result<&'static str,String> {
    if !previous_attached(conn)? {return Ok("main");}
    let here:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM main.corpus_state WHERE session_id=?1)",[session],|r|r.get(0)).map_err(sql)?;
    Ok(if here {"main"} else {"old"})
}

/// Matching sessions in relevance order. The word index answers first; the trigram index or, for
/// terms shorter than three characters, LIKE answers only when no word matched.
pub(super) fn rank(conn:&Connection,query:&str,scope:SearchScope,session:Option<&str>) -> Result<(Option<Query>,Vec<Ranked>),String> {
    let keys:Vec<String>=query.split_whitespace().map(str::to_string).collect();
    if keys.is_empty() {return Ok((None,Vec::new()));}
    let arms=arms(conn)?;
    let run=|sql:String,binds:Vec<String>| -> Result<Vec<Ranked>,String> {
        let mut statement=conn.prepare(&sql).map_err(sql_error)?;
        let rows=statement.query_map(params_from_iter(binds.iter()),|r|Ok(Ranked {session:r.get(0)?,count:r.get(1)?,source:r.get(2)?})).map_err(sql_error)?;
        rows.collect::<Result<Vec<_>,_>>().map_err(sql_error)
    };
    // Each arm selects session, source and rank; the sessions table applies the scope once.
    let union=|arm:&dyn Fn(&str,&str)->String,order:&str| format!("SELECT x.session_id,count(*),min(x.source) FROM ({}) x JOIN app.sessions s ON s.id=x.session_id
        WHERE 1=1{} GROUP BY x.session_id ORDER BY {order},x.session_id",arms.iter().map(|(db,limit)|arm(db,limit)).collect::<Vec<_>>().join(" UNION ALL "),scope_filter(scope));
    let one=if session.is_some() {" AND f.session_id=?2"} else {""};
    let with_session=|first:String| -> Vec<String> {let mut binds=vec![first];if let Some(id)=session {binds.push(id.into());}binds};
    if let Some(phrases)=keys.iter().map(|k|tokenize::query_phrase(k)).collect::<Option<Vec<_>>>() {
        let expression=phrases.join(" ");
        let ranked=run(union(&|db,limit|format!("SELECT f.session_id,f.source,w.rank AS r FROM {db}.session_words w JOIN {db}.session_fts f ON f.rowid=w.rowid
            WHERE session_words MATCH ?1{limit}{one}"),"min(x.r)"),with_session(expression.clone()))?;
        if !ranked.is_empty() {return Ok((Some(Query {keys,answered:Answered::Words,expression}),ranked));}
    }
    if keys.iter().all(|k|k.chars().count()>=MIN_FTS_TERM_CHARS) {
        let expression=fts_match_expr(&keys);
        let ranked=run(union(&|db,limit|format!("SELECT f.session_id,f.source,f.rank AS r FROM {db}.session_fts f WHERE session_fts MATCH ?1{limit}{one}"),"min(x.r)"),
            with_session(expression.clone()))?;
        return Ok((Some(Query {keys,answered:Answered::Trigram,expression}),ranked));
    }
    let clause=keys.iter().enumerate().map(|(index,_)|format!("f.text LIKE ?{} ESCAPE '\\'",index+2)).collect::<Vec<_>>().join(" AND ");
    let one=if session.is_some() {" AND f.session_id=?1"} else {""};
    let mut binds=vec![session.unwrap_or_default().to_owned()];binds.extend(keys.iter().map(|k|format!("%{}%",like_escape(k))));
    let mut ranked=run(union(&|db,limit|format!("SELECT f.session_id,f.source,0 AS r FROM {db}.session_fts f WHERE {clause}{limit}{one}"),"count(*) DESC"),binds)?;
    ranked.sort_by(|a,b|b.count.cmp(&a.count));
    Ok((Some(Query {keys,answered:Answered::Like,expression:String::new()}),ranked))
}
fn sql_error(error:rusqlite::Error) -> String {sql(error)}

/// Up to [`MAX_MATCHES`] snippets for one ranked session, in document order.
pub(super) fn matches(conn:&Connection,query:&Query,session:&str) -> Result<Vec<SearchMatch>,String> {
    struct Raw {message:Option<i64>,text:String,spans:Vec<(usize,usize)>,role:Option<String>,ts:Option<String>,transcript:bool,offset:Option<u64>}
    let limit=MAX_MATCHES as i64;let db=holder(conn,session)?;
    // Files before version 3 hold no record offsets.
    let offset=if db=="old" {"NULL"} else {"f.record_offset"};
    let raws:Vec<Raw>=match query.answered {
        Answered::Words=> {
            let mut statement=conn.prepare(&format!("SELECT f.message_index,f.text,f.role,f.ts,f.source,{offset} FROM {db}.session_words w JOIN {db}.session_fts f ON f.rowid=w.rowid
                WHERE session_words MATCH ?1 AND f.session_id=?2 ORDER BY f.ordinal LIMIT ?3")).map_err(sql)?;
            let rows=statement.query_map(params![query.expression,session,limit],|r| {
                Ok(Raw {message:r.get(0)?,text:r.get(1)?,spans:Vec::new(),role:r.get(2)?,ts:r.get(3)?,transcript:r.get::<_,String>(4)?=="transcript",offset:r.get(5)?})
            }).map_err(sql)?;
            let mut raws=rows.collect::<Result<Vec<_>,_>>().map_err(sql)?;
            highlight_words(conn,&query.expression,&mut raws.iter_mut().map(|raw|(&raw.text,&mut raw.spans)).collect::<Vec<_>>())?;
            raws
        },
        Answered::Trigram|Answered::Like=> {
            let (condition,binds)=if query.answered==Answered::Trigram {("session_fts MATCH ?1".to_owned(),vec![query.expression.clone()])} else {
                (query.keys.iter().enumerate().map(|(index,_)|format!("f.text LIKE ?{} ESCAPE '\\'",index+1)).collect::<Vec<_>>().join(" AND "),
                    query.keys.iter().map(|k|format!("%{}%",like_escape(k))).collect())
            };
            let at=binds.len();
            let mut statement=conn.prepare(&format!("SELECT f.message_index,f.text,f.role,f.ts,f.source,{offset} FROM {db}.session_fts f WHERE {condition}
                AND f.session_id=?{} ORDER BY f.ordinal LIMIT ?{}",at+1,at+2)).map_err(sql)?;
            let mut values:Vec<rusqlite::types::Value>=binds.into_iter().map(rusqlite::types::Value::Text).collect();
            values.push(rusqlite::types::Value::Text(session.into()));values.push(rusqlite::types::Value::Integer(limit));
            let rows=statement.query_map(params_from_iter(values.iter()),|r|Ok(Raw {message:r.get(0)?,text:r.get(1)?,spans:Vec::new(),role:r.get(2)?,ts:r.get(3)?,
                transcript:r.get::<_,String>(4)?=="transcript",offset:r.get(5)?})).map_err(sql)?;rows.collect::<Result<_,_>>().map_err(sql)?
        },
    };
    Ok(raws.into_iter().enumerate().map(|(index,raw)| {
        let (snippet,matched)=if query.answered==Answered::Words {
            let snippet=match (raw.transcript,raw.spans.first()) {(true,Some(&(at,end)))=>snippet_around(&raw.text,at,end-at),_=>clip_line(&raw.text)};
            (snippet,matched_literals(&raw.text,&raw.spans))
        } else {(if raw.transcript {snippet_for(&raw.text,&query.keys)} else {clip_line(&raw.text)},query.keys.clone())};
        SearchMatch {message_index:raw.message.map(|v|v as u32),ordinal:(index+1) as u32,snippet,matched,role:raw.role,timestamp:raw.ts,row_id:None,record_offset:raw.offset}
    }).collect())
}

/// Where each text matches a word query. The word index keeps no copy of the text it indexes, so the
/// few fragments of one result page are indexed again in a temporary table of the same tokenizer,
/// whose `highlight` marks exactly the words the stored index matched.
fn highlight_words(conn:&Connection,expression:&str,texts:&mut [(&String,&mut Vec<(usize,usize)>)]) -> Result<(),String> {
    conn.execute_batch("CREATE VIRTUAL TABLE IF NOT EXISTS temp.highlight_words USING fts5(words, tokenize = 'porter unicode61 remove_diacritics 1');
        DELETE FROM temp.highlight_words;").map_err(sql)?;
    for (index,(text,_)) in texts.iter().enumerate() {
        conn.execute("INSERT INTO temp.highlight_words(rowid,words) VALUES(?1,?2)",params![index as i64+1,tokenize::boundary_marked(text)]).map_err(sql)?;
    }
    let mut statement=conn.prepare("SELECT rowid,highlight(highlight_words,0,?2,?3) FROM temp.highlight_words WHERE highlight_words MATCH ?1").map_err(sql)?;
    let rows=statement.query_map(params![expression,tokenize::MARK_OPEN,tokenize::MARK_CLOSE],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).map_err(sql)?;
    for row in rows {
        let (rowid,marked)=row.map_err(sql)?;
        if let Some((text,spans))=texts.get_mut((rowid-1) as usize) {
            let highlighted=tokenize::parse_highlight(&marked);
            // The marked text is the fragment again; its spans index the fragment's own bytes.
            if highlighted.text==**text {**spans=highlighted.spans;}
        }
    }
    conn.execute_batch("DELETE FROM temp.highlight_words;").map_err(sql)?;Ok(())
}

/// The complete hit for one ranked session, with names read from the business database.
pub(super) fn hit(conn:&Connection,query:&Query,ranked:&Ranked,names:&HashMap<String,(String,SessionKind,bool)>) -> Result<Option<SessionSearchHit>,String> {
    let Some((name,kind,archived))=names.get(&ranked.session) else {return Ok(None);};
    Ok(Some(SessionSearchHit {session_id:ranked.session.clone(),name:name.clone(),kind:*kind,archived:*archived,
        source:if ranked.source=="transcript" {"transcript"} else {"recording"},match_count:ranked.count,matches:matches(conn,query,&ranked.session)?}))
}

fn matched_literals(text:&str,spans:&[(usize,usize)]) -> Vec<String> {
    let mut out:Vec<String>=Vec::new();
    for &(a,b) in spans {let literal=&text[a..b];if !out.iter().any(|m|m==literal) {out.push(literal.to_string());}}out
}
fn snippet_for(text:&str,keys:&[String]) -> String {
    let lower=text.to_lowercase();let mut best:Option<(usize,usize)>=None;
    for k in keys {
        let kl=k.to_lowercase();if kl.is_empty() {continue;}
        if let Some(p)=lower.find(&kl) {if best.is_none_or(|(bp,_)|p<bp) {best=Some((p,kl.len()));}}
    }
    // Lowercasing can change byte lengths; snippet_around clamps to character boundaries either way.
    match best {Some((at,len))=>snippet_around(text,at,len),None=>clip_line(text)}
}
fn snippet_around(text:&str,at:usize,hit_len:usize) -> String {
    let len=text.len();let at=at.min(len);
    let start=floor_char_boundary(text,at.saturating_sub(SNIPPET_CONTEXT));
    let end=ceil_char_boundary(text,(at+hit_len+SNIPPET_CONTEXT).min(len));
    let core=text[start..end].replace(['\n','\r','\t']," ");let mut out=String::new();
    if start>0 {out.push('…');}out.push_str(core.trim());if end<len {out.push('…');}
    limit_chars(&out)
}
fn clip_line(line:&str) -> String {limit_chars(line.trim())}
fn floor_char_boundary(s:&str,mut i:usize) -> usize {if i>=s.len() {return s.len();}while i>0 && !s.is_char_boundary(i) {i-=1;}i}
fn ceil_char_boundary(s:&str,mut i:usize) -> usize {if i>=s.len() {return s.len();}while i<s.len() && !s.is_char_boundary(i) {i+=1;}i}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn message(id:&str,role:&str,text:&str) -> String {
        format!("{}\n",serde_json::json!({"type":"response_item","payload":{"type":"message","id":id,"role":role,
            "content":[{"type":if role=="user" {"input_text"} else {"output_text"},"text":text}]}}))
    }

    #[test]
    fn transcript_appends_index_only_new_messages_and_unchanged_sources_cost_one_stat() {
        let root=std::env::temp_dir().join(format!("vlx-search-corpus-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
        let db=crate::db::Db::open(&root.join("business.sqlite")).unwrap();
        db.conn.lock().unwrap().execute_batch("INSERT INTO projects(id,name,root_path,sort_order,created_at) VALUES('p','p','/p',0,0);
            INSERT INTO sessions(id,project_id,name,kind,sort_order,created_at,agent_session_id) VALUES('s','p','codex','codex',0,0,'thread');").unwrap();
        let app=AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(root.clone(),db)));
        let rollout=root.join("rollout-thread.jsonl");
        std::fs::write(&rollout,format!("{}{}",message("u1","user","Where is the alpha config?"),message("a1","assistant","The alpha config lives in settings."))).unwrap();
        let conn=open(&app).unwrap();
        // The provider's directory lookup is not part of this test: start from a remembered path.
        conn.execute("INSERT INTO corpus_state VALUES('s','transcript',?1,0,NULL,NULL,0,0,NULL)",[rollout.to_string_lossy()]).unwrap();
        let session=sessions(&app,None).unwrap().remove(0);let cancel=Arc::new(AtomicBool::new(false));
        let rows=|conn:&Connection| conn.query_row("SELECT count(*),count(DISTINCT message_index) FROM session_fts WHERE session_id='s'",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?))).unwrap();
        assert!(matches!(refresh_session(&app,&conn,&session,&cancel).unwrap(),Refreshed::Indexed(_)));
        assert_eq!(rows(&conn),(2,2));
        assert!(matches!(refresh_session(&app,&conn,&session,&cancel).unwrap(),Refreshed::Unchanged));
        let (query,ranked)=rank(&conn,"alpha",SearchScope::All,None).unwrap();assert_eq!(ranked.len(),1);assert_eq!(ranked[0].count,2);
        let found=matches(&conn,&query.unwrap(),"s").unwrap();assert_eq!(found[0].message_index,Some(0));assert_eq!(found[1].message_index,Some(1));
        // An append keeps earlier rows and adds only the new message.
        std::fs::OpenOptions::new().append(true).open(&rollout).unwrap().write_all(message("u2","user","And the omega one?").as_bytes()).unwrap();
        let later=std::time::SystemTime::now()+std::time::Duration::from_secs(2);std::fs::File::options().write(true).open(&rollout).unwrap().set_modified(later).unwrap();
        assert!(matches!(refresh_session(&app,&conn,&session,&cancel).unwrap(),Refreshed::Indexed(_)));
        assert_eq!(rows(&conn),(3,3));
        assert_eq!(rank(&conn,"omega",SearchScope::All,None).unwrap().1.len(),1);
        // A long message becomes several fragments of the same message.
        std::fs::OpenOptions::new().append(true).open(&rollout).unwrap().write_all(message("a2","assistant",&"long answer line\n".repeat(9000)).as_bytes()).unwrap();
        let later=std::time::SystemTime::now()+std::time::Duration::from_secs(4);std::fs::File::options().write(true).open(&rollout).unwrap().set_modified(later).unwrap();
        refresh_session(&app,&conn,&session,&cancel).unwrap();
        let (fragments,messages)=rows(&conn);assert_eq!(messages,4);assert!(fragments>=5,"{fragments}");
        // Removing the session from the business database prunes its rows.
        app.db().conn.lock().unwrap().execute("DELETE FROM sessions WHERE id='s'",[]).unwrap();prune(&conn).unwrap();assert_eq!(rows(&conn),(0,0));
        drop(conn);drop(app);let _=std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_appended_conversation_resumes_at_its_last_message_and_matches_a_full_rebuild() {
        let root=std::env::temp_dir().join(format!("vlx-search-resume-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
        let db=crate::db::Db::open(&root.join("business.sqlite")).unwrap();
        db.conn.lock().unwrap().execute_batch("INSERT INTO projects(id,name,root_path,sort_order,created_at) VALUES('p','p','/p',0,0);
            INSERT INTO sessions(id,project_id,name,kind,sort_order,created_at,agent_session_id) VALUES('s','p','codex','codex',0,0,'thread');").unwrap();
        let app=AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(root.clone(),db)));
        let rollout=root.join("rollout-thread.jsonl");std::fs::write(&rollout,message("u1","user","First question")).unwrap();
        let conn=open(&app).unwrap();
        conn.execute("INSERT INTO corpus_state VALUES('s','transcript',?1,0,NULL,NULL,0,0,NULL)",[rollout.to_string_lossy()]).unwrap();
        let session=sessions(&app,None).unwrap().remove(0);let cancel=Arc::new(AtomicBool::new(false));
        let rows=|conn:&Connection| {
            let mut statement=conn.prepare("SELECT message_index,group_concat(text,'|') FROM (SELECT message_index,text FROM session_fts WHERE session_id='s' ORDER BY ordinal) GROUP BY message_index ORDER BY message_index").unwrap();
            statement.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap()
        };
        refresh_session(&app,&conn,&session,&cancel).unwrap();
        let appends=[message("a1","assistant","First answer"),message("a2","assistant","continued in the same message"),
            message("u2","user","Second question")+&message("a3","assistant","Second answer")];
        for (index,append) in appends.iter().enumerate() {
            std::fs::OpenOptions::new().append(true).open(&rollout).unwrap().write_all(append.as_bytes()).unwrap();
            let later=std::time::SystemTime::now()+std::time::Duration::from_secs(2*(index as u64+1));
            std::fs::File::options().write(true).open(&rollout).unwrap().set_modified(later).unwrap();
            refresh_session(&app,&conn,&session,&cancel).unwrap();
            let resume:Option<String>=conn.query_row("SELECT resume FROM corpus_state WHERE session_id='s'",[],|r|r.get(0)).unwrap();assert!(resume.is_some());
        }
        // The transcript is read from its file: searching it never builds its history index.
        assert!(!root.join("chat-index").exists());
        std::fs::OpenOptions::new().append(true).open(&rollout).unwrap().write_all(message("a4","assistant","after the rebuild").as_bytes()).unwrap();
        let later=std::time::SystemTime::now()+std::time::Duration::from_secs(20);std::fs::File::options().write(true).open(&rollout).unwrap().set_modified(later).unwrap();
        refresh_session(&app,&conn,&session,&cancel).unwrap();
        let incremental=rows(&conn);
        assert_eq!(incremental.len(),4,"{incremental:?}");assert!(incremental[3].1.contains("after the rebuild"),"{incremental:?}");assert!(incremental[1].1.contains("continued in the same message"),"{incremental:?}");
        // A rebuild from nothing gives the same rows.
        conn.execute("DELETE FROM corpus_state WHERE session_id='s'",[]).unwrap();delete_rows(&conn,"s",None).unwrap();
        conn.execute("INSERT INTO corpus_state VALUES('s','transcript',?1,0,NULL,NULL,0,0,NULL)",[rollout.to_string_lossy()]).unwrap();
        refresh_session(&app,&conn,&session,&cancel).unwrap();assert_eq!(rows(&conn),incremental);
        drop(conn);drop(app);let _=std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_conversation_that_cannot_be_read_is_not_read_again_until_it_changes() {
        let root=std::env::temp_dir().join(format!("vlx-search-failed-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
        let db=crate::db::Db::open(&root.join("business.sqlite")).unwrap();
        db.conn.lock().unwrap().execute_batch("INSERT INTO projects(id,name,root_path,sort_order,created_at) VALUES('p','p','/p',0,0);
            INSERT INTO sessions(id,project_id,name,kind,sort_order,created_at,agent_session_id) VALUES('s','p','claude','claude',0,0,'thread');").unwrap();
        let app=AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(root.clone(),db)));
        // A rewind names a leaf the recording does not contain. A parent that was never written is
        // followed past (Claude drops failed replies), but a missing leaf leaves no branch to read.
        let transcript=root.join("thread.jsonl");
        std::fs::write(&transcript,serde_json::json!({"type":"user","uuid":"b","message":{"role":"user","content":"Orphan"}}).to_string()+"\n"
            +&serde_json::json!({"type":"last-prompt","rewound":true,"leafUuid":"missing"}).to_string()+"\n").unwrap();
        let conn=open(&app).unwrap();
        conn.execute("INSERT INTO corpus_state VALUES('s','transcript',?1,0,NULL,NULL,0,0,NULL)",[transcript.to_string_lossy()]).unwrap();
        let session=sessions(&app,None).unwrap().remove(0);let cancel=Arc::new(AtomicBool::new(false));
        assert!(refresh_session(&app,&conn,&session,&cancel).err().unwrap().starts_with("index_incomplete:"));
        let source=|conn:&Connection| conn.query_row("SELECT source FROM corpus_state WHERE session_id='s'",[],|r|r.get::<_,String>(0)).unwrap();
        assert_eq!(source(&conn),"failed");
        assert!(matches!(refresh_session(&app,&conn,&session,&cancel).unwrap(),Refreshed::Unchanged),"an unchanged failure is skipped");
        // Once the recording changes it is read again.
        std::fs::write(&transcript,serde_json::json!({"type":"user","uuid":"a","message":{"role":"user","content":"Root question"}}).to_string()+"\n").unwrap();
        let later=std::time::SystemTime::now()+std::time::Duration::from_secs(2);std::fs::File::options().write(true).open(&transcript).unwrap().set_modified(later).unwrap();
        assert!(matches!(refresh_session(&app,&conn,&session,&cancel).unwrap(),Refreshed::Indexed(_)));assert_eq!(source(&conn),"transcript");
        drop(conn);drop(app);let _=std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_conversation_caught_mid_write_reads_on_from_its_resume_point_afterwards() {
        let root=std::env::temp_dir().join(format!("vlx-search-midwrite-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
        let db=crate::db::Db::open(&root.join("business.sqlite")).unwrap();
        db.conn.lock().unwrap().execute_batch("INSERT INTO projects(id,name,root_path,sort_order,created_at) VALUES('p','p','/p',0,0);
            INSERT INTO sessions(id,project_id,name,kind,sort_order,created_at,agent_session_id) VALUES('s','p','codex','codex',0,0,'thread');").unwrap();
        let app=AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(root.clone(),db)));
        let rollout=root.join("rollout-thread.jsonl");
        std::fs::write(&rollout,message("u1","user","First question")+&message("a1","assistant","First answer")).unwrap();
        let conn=open(&app).unwrap();
        conn.execute("INSERT INTO corpus_state VALUES('s','transcript',?1,0,NULL,NULL,0,0,NULL)",[rollout.to_string_lossy()]).unwrap();
        let session=sessions(&app,None).unwrap().remove(0);let cancel=Arc::new(AtomicBool::new(false));
        let touch=|seconds:u64| {let later=std::time::SystemTime::now()+std::time::Duration::from_secs(seconds);std::fs::File::options().write(true).open(&rollout).unwrap().set_modified(later).unwrap();};
        refresh_session(&app,&conn,&session,&cancel).unwrap();
        let state=|conn:&Connection| conn.query_row("SELECT source,resume IS NOT NULL FROM corpus_state WHERE session_id='s'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,bool>(1)?))).unwrap();
        assert_eq!(state(&conn),("transcript".into(),true));
        // The next record is only half written when the search reads it.
        let next=message("u2","user","Second question");let (head,tail)=next.split_at(next.len()/2);
        std::fs::OpenOptions::new().append(true).open(&rollout).unwrap().write_all(head.as_bytes()).unwrap();touch(2);
        // The unfinished line is left for the next read; the complete messages stay indexed.
        assert!(matches!(refresh_session(&app,&conn,&session,&cancel).unwrap(),Refreshed::Indexed(_)));
        assert_eq!(state(&conn),("transcript".into(),true));
        let rows=|conn:&Connection| {
            let mut statement=conn.prepare("SELECT message_index,group_concat(text,'|') FROM (SELECT message_index,text FROM session_fts WHERE session_id='s' ORDER BY ordinal) GROUP BY message_index ORDER BY message_index").unwrap();
            statement.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap()
        };
        assert_eq!(rows(&conn).len(),2,"the rows indexed before stay searchable");
        let first=|conn:&Connection| conn.query_row("SELECT rowid FROM session_fts WHERE session_id='s' AND message_index=0",[],|r|r.get::<_,i64>(0)).unwrap();
        let before=first(&conn);
        // Once the record is complete the session reads on and matches a rebuild from nothing.
        std::fs::OpenOptions::new().append(true).open(&rollout).unwrap().write_all(tail.as_bytes()).unwrap();touch(4);
        assert!(matches!(refresh_session(&app,&conn,&session,&cancel).unwrap(),Refreshed::Indexed(_)));
        let incremental=rows(&conn);assert_eq!(incremental.len(),3,"{incremental:?}");assert_eq!(state(&conn).0,"transcript");
        assert_eq!(first(&conn),before,"messages before the resume point are not read again");
        conn.execute("DELETE FROM corpus_state WHERE session_id='s'",[]).unwrap();delete_rows(&conn,"s",None).unwrap();
        conn.execute("INSERT INTO corpus_state VALUES('s','transcript',?1,0,NULL,NULL,0,0,NULL)",[rollout.to_string_lossy()]).unwrap();
        refresh_session(&app,&conn,&session,&cancel).unwrap();assert_eq!(rows(&conn),incremental);
        drop(conn);drop(app);let _=std::fs::remove_dir_all(&root);
    }

    #[test]
    fn blocks_of_multibyte_text_without_line_breaks_split_on_character_boundaries() {
        let text="、".repeat(30_000)+"\n"+&"雪".repeat(30_000);let mut joined=String::new();let mut blocks=0;
        chunks(&mut text.as_bytes(),&AtomicBool::new(false),|block|{assert!(block.len()<=CHUNK_BYTES);joined.push_str(block);blocks+=1;Ok(())}).unwrap();
        assert_eq!(joined,text);assert!(blocks>=3);
    }

    /// Times a cold warm-up as the background pass runs it on a copied data directory, VLX_BENCH_DATA=<dir>,
    /// per provider, and reports whether the history index grew meanwhile.
    #[test]
    #[ignore]
    fn bench_cold_warmup_of_a_real_data_directory() {
        let Ok(dir)=std::env::var("VLX_BENCH_DATA") else {return;};let root=PathBuf::from(dir);
        let app=AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(root.clone(),crate::db::Db::open(&root.join("vlx-term.db")).unwrap())));
        let conn=open(&app).unwrap();let cancel=Arc::new(AtomicBool::new(false));
        let history=|| std::fs::metadata(root.join("chat-index/index.sqlite")).map(|meta|meta.len()).unwrap_or(0);
        let before=history();let began=std::time::Instant::now();
        let mut kinds:std::collections::BTreeMap<String,(u64,u64,u64,f64)>=Default::default();let mut after=None;
        loop {
            let batch=sessions(&app,after.as_deref()).unwrap();if batch.is_empty() {break;}after=batch.last().map(|s|s.id.clone());
            for session in batch {
                let t=std::time::Instant::now();let result=refresh_session(&app,&conn,&session,&cancel);let spent=t.elapsed().as_secs_f64();
                let entry=kinds.entry(format!("{:?}",session.kind)).or_default();entry.3+=spent;
                match result {Ok(Refreshed::Indexed(bytes))=>{entry.0+=1;entry.1+=bytes;},Ok(_)=>{},Err(error)=>{entry.2+=1;eprintln!("skip {:?} {} {error}",session.kind,session.id);}}
                if spent>5.0 {eprintln!("slow {:?} {} {spent:.1}s",session.kind,session.id);}
            }
        }
        for (kind,(count,bytes,failed,spent)) in &kinds {eprintln!("{kind:<12} {count:>5} indexed {:>9.1} MB {failed:>4} failed {spent:>7.1}s",*bytes as f64/1e6);}
        let size=std::fs::metadata(path(&root)).map(|meta|meta.len()).unwrap_or(0);
        eprintln!("total {:.0}s; search index {:.1} MB; history index {:.1} MB -> {:.1} MB",began.elapsed().as_secs_f64(),size as f64/1e6,before as f64/1e6,history() as f64/1e6);
    }

    /// Times one search's refresh pass over an already built index on a copied data directory,
    /// VLX_BENCH_DATA=<dir>, session by session.
    #[test]
    #[ignore]
    fn bench_warm_refresh_of_a_real_data_directory() {
        let Ok(dir)=std::env::var("VLX_BENCH_DATA") else {return;};let root=PathBuf::from(dir);
        let app=AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(root.clone(),crate::db::Db::open(&root.join("vlx-term.db")).unwrap())));
        let conn=open(&app).unwrap();let cancel=Arc::new(AtomicBool::new(false));
        {
            let began=std::time::Instant::now();let mut after=None;
            loop {
                let batch=sessions(&app,after.as_deref()).unwrap();if batch.is_empty() {break;}after=batch.last().map(|s|s.id.clone());
                for session in batch {
                    let previous=state(&conn,&session.id).unwrap().map(|state|state.source);let t=std::time::Instant::now();
                    let result=refresh_session(&app,&conn,&session,&cancel);let spent=t.elapsed().as_secs_f64();
                    if spent>0.2 || matches!(result,Ok(Refreshed::Indexed(_))) {
                        let read=match &result {Ok(Refreshed::Indexed(bytes))=>format!("{:.1} MB",*bytes as f64/1e6),Ok(_)=>"unchanged".into(),Err(e)=>e.clone()};
                        eprintln!("refresh {} {:?} {previous:?} {spent:.2}s {read}",session.id,session.kind);
                    }
                }
            }
            eprintln!("refresh pass {:.1}s",began.elapsed().as_secs_f64());
        }
    }
}

