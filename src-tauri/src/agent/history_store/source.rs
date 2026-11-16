//! Resolve native histories into ordered physical intervals without concatenating ancestor files.

use std::collections::HashSet;
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::agent::resume;
use crate::models::SessionKind;
use super::projection::Projector;

#[derive(Clone, Debug)]
pub struct Segment {
    pub path: PathBuf,
    pub native_id: String,
    pub end_byte: Option<u64>,
    pub end_ordinal: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Stamp {
    pub identity: String,
    pub modified: String,
    pub size: u64,
    pub head_hash: Vec<u8>,
}

pub fn identity(metadata: &Metadata) -> String {
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        format!("{}:{}", metadata.dev(), metadata.ino())
    }
    #[cfg(not(unix))] {
        // Windows creation time distinguishes common replacements; content blocks remain authoritative.
        format!("{:?}", metadata.created().ok())
    }
}

pub fn stamp(file: &mut File) -> Result<Stamp, String> {
    let metadata = file.metadata().map_err(|e| format!("History source is unavailable: {e}"))?;
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut head = vec![0u8; metadata.len().min(64 * 1024) as usize];
    file.read_exact(&mut head).map_err(|e| e.to_string())?;
    Ok(Stamp { identity: identity(&metadata), modified: format!("{:?}", metadata.modified().ok()),
        size: metadata.len(), head_hash: Sha256::digest(&head).to_vec() })
}

pub struct Resolved {
    pub segments: Vec<Segment>,
    pub error: Option<String>,
}

pub fn resolve(kind: SessionKind, id: &str) -> Result<Resolved, String> {
    match kind {
        SessionKind::Codex => {
            resolve_codex(id, &resume::find_codex_rollout)
        }
        SessionKind::Claude => Ok(Resolved {segments:vec![Segment {
            path: resume::find_claude_transcript(id).ok_or("source_missing: Claude transcript file not found")?,
            native_id: id.to_owned(), end_byte: None, end_ordinal: None,
        }],error:None}),
        SessionKind::Grok=>Ok(Resolved {segments:vec![Segment {
            path:resume::find_grok_updates(id).ok_or("source_missing: Grok transcript file not found")?,
            native_id:id.to_owned(),end_byte:None,end_ordinal:None,
        }],error:None}),
        SessionKind::Pi | SessionKind::Omp => Ok(Resolved {segments:vec![Segment {
            path: resume::find_pi_session(kind, id).ok_or("source_missing: The agent's session recording could not be found")?,
            native_id: id.to_owned(), end_byte: None, end_ordinal: None,
        }],error:None}),
        SessionKind::Opencode=>Ok(Resolved {segments:vec![Segment {
            path:crate::agent::opencode_store::db_path().filter(|path|path.is_file()).ok_or("source_missing: OpenCode database is unavailable")?,
            native_id:id.into(),end_byte:None,end_ordinal:None,
        }],error:None}),
        // A DeepSeek conversation recorded before OpenCode took over exists only as a dsh log.
        SessionKind::Deepseek=>match super::deepseek_legacy::log(id) {
            Some(path)=>Ok(Resolved {segments:vec![Segment {
                path,native_id:format!("{}{id}",super::deepseek_legacy::NATIVE_PREFIX),end_byte:None,end_ordinal:None,
            }],error:None}),
            None=>Ok(Resolved {segments:vec![Segment {
                path:crate::agent::opencode_store::db_path().filter(|path|path.is_file()).ok_or("source_missing: OpenCode database is unavailable")?,
                native_id:id.into(),end_byte:None,end_ordinal:None,
            }],error:None}),
        },
        SessionKind::Kiro=>{
            let roots=crate::agent::kiro_store::Roots::native()?;
            if !crate::agent::kiro_store::valid_id(id) {return Err("unsupported_record: Kiro session identity is invalid".into());}
            if roots.sessions.join(format!("{id}.json")).exists() || roots.sessions.join(format!("{id}.jsonl")).exists() {
                return Ok(Resolved {segments:vec![Segment {path:roots.sessions.join(format!("{id}.jsonl")),native_id:id.into(),end_byte:None,end_ordinal:None}],error:None});
            }
            Ok(Resolved {segments:vec![Segment {path:roots.database.filter(|path|path.is_file()).ok_or("source_missing: Kiro database is unavailable")?,
                native_id:id.into(),end_byte:None,end_ordinal:None}],error:None})
        },
        SessionKind::Antigravity => Ok(Resolved {segments:vec![Segment {
            path:crate::agent::antigravity::transcript_path(id).ok_or("source_missing: Antigravity transcript file not found")?,
            native_id:id.to_owned(),end_byte:None,end_ordinal:None,
        }],error:None}),
        other => Err(format!("Bounded history adapter is unavailable for {} sessions", other.as_str())),
    }
}

pub(crate) fn resolve_codex(id:&str,lookup:&impl Fn(&str)->Option<PathBuf>) -> Result<Resolved,String> {
    let mut result=Resolved {segments:Vec::new(),error:None};
    if let Err(error)=codex(id,None,None,&mut HashSet::new(),&mut result,lookup) {
        if error=="Codex parent rollout file not found" {return Err("source_missing: Codex rollout file not found".into());}
        return Err(error);
    }
    Ok(result)
}

fn codex(id: &str, end_byte: Option<u64>, end_ordinal: Option<u64>, seen: &mut HashSet<String>, result: &mut Resolved,
    lookup:&impl Fn(&str)->Option<PathBuf>) -> Result<(), String> {
    if seen.len() >= 32 { return Err("Codex rollout fork history is deeper than 32 levels".into()); }
    if !seen.insert(id.to_owned()) { return Err("Codex rollout fork history contains a cycle".into()); }
    let path = lookup(id).ok_or("Codex parent rollout file not found")?;
    let mut file = File::open(&path).map_err(|e| format!("History source is unavailable: {e}"))?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if end_byte.is_some_and(|end| end > size) { return Err("Codex fork history has an invalid byte boundary".into()); }
    // A valid session_meta is decisive even when it has no history_base. Never scan the rollout looking
    // for a second metadata record. Missing metadata is an explicit partial-history condition.
    let mut parser = Projector::new((&mut file).take(super::limits::TAIL_BYTES), 0);
    let mut metadata = None;
    while let Some(record) = parser.next(&mut |_| Ok(()))? {
        if record.error.is_none() && record.value["type"].as_str() == Some("session_meta") {
            metadata = Some(record.value);
            break;
        }
    }
    let value = metadata.ok_or("index_incomplete: Codex session metadata is unavailable within the header budget")?;
    if let Some(native) = value.pointer("/payload/id").and_then(serde_json::Value::as_str) {
        if native != id { return Err("source_changed: Codex source identity does not match the requested thread".into()); }
    }
    if let Some(base) = value.pointer("/payload/history_base").or_else(|| value.pointer("/payload/historyBase")) {
        let parent = base.get("thread_id").or_else(|| base.get("threadId"))
            .and_then(serde_json::Value::as_str).ok_or("Codex fork history has no parent thread identifier")?;
        let bytes = base.get("end_byte_offset").or_else(|| base.get("endByteOffset")).and_then(serde_json::Value::as_u64);
        let ordinal = base.get("end_ordinal_exclusive").or_else(|| base.get("endOrdinalExclusive")).and_then(serde_json::Value::as_u64);
        if bytes.is_none() && ordinal.is_none() { return Err("Codex fork history has no parent cutoff".into()); }
        if let Err(error)=codex(parent,bytes,ordinal,seen,result,lookup) {
            // The verified child remains readable. Missing ancestors never become a complete branch.
            if result.error.is_none() {result.error=Some(format!("index_incomplete: {error}"));}
        }
    }
    result.segments.push(Segment { path, native_id: id.to_owned(), end_byte, end_ordinal });
    Ok(())
}

pub fn opened_source_unchanged(file: &File, path: &Path, expected: &Stamp) -> Result<(), String> {
    let current = file.metadata().map_err(|e| e.to_string())?;
    let at_path = std::fs::metadata(path).map_err(|_| "source_changed: history source was removed")?;
    if identity(&current) != expected.identity || identity(&at_path) != expected.identity || current.len() < expected.size {
        return Err("source_changed: history source was replaced or truncated".into());
    }
    Ok(())
}
