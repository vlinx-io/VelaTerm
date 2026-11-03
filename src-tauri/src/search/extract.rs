//! Message text of Claude, Codex, Pi, OMP, Grok and Antigravity transcripts, read straight from the
//! provider's JSONL file.
//!
//! The search index needs only what each message says, so it does not build the history index for a
//! conversation it merely searches. Messages are numbered exactly as the conversation view numbers them
//! ([`crate::agent::history_store::prose::Prose`]): the same line parsers, the same records shown, the
//! same message boundaries and, for the tree recordings of Claude, Pi and OMP, the same active branch. `vsearch` prints those numbers and
//! `vrefer --range` reads them back, so the two must agree.
//!
//! Each message also carries the byte offset of its first record. The conversation view locates a hit
//! by that offset, which stays exact even where numbering could not be confirmed.
//!
//! Only complete lines are read; a line still being written is left for the next refresh. A line over
//! [`LINE_BYTES`] is never held in memory: its branch identity is read by a streaming parse and its text
//! is not indexed.

use std::collections::{HashMap,HashSet};
use std::fs::File;
use std::io::{BufReader,Read,Seek,SeekFrom};
use std::path::{Path,PathBuf};
use std::sync::atomic::{AtomicBool,Ordering};

use serde::{Deserialize,Serialize};
use serde_json::Value;
use sha2::{Digest,Sha256};

use crate::agent::{export,transcript};
use crate::models::SessionKind;

/// Longest line read into memory.
pub(super) const LINE_BYTES:usize=64*1024*1024;
/// Prefix of the file that identifies it on resume.
const HEAD_BYTES:u64=64*1024;
/// The history index refuses a branch deeper than this, and so does this reader.
const MAX_CHAIN:usize=100_000;

pub(super) fn supported(kind:SessionKind) -> bool {
    matches!(kind,SessionKind::Claude|SessionKind::Codex|SessionKind::Pi|SessionKind::Omp|SessionKind::Grok|SessionKind::Antigravity)
}
/// Grok drops messages left without text or tools after reading them all, which renumbers the ones
/// after them, so its messages are always read from the first.
fn resumable(kind:SessionKind) -> bool {kind!=SessionKind::Grok}
/// Recordings whose records form a tree, of which only the active branch is read.
fn tree(kind:SessionKind) -> bool {matches!(kind,SessionKind::Claude|SessionKind::Pi|SessionKind::Omp)}

/// One message of the conversation, in conversation-view numbering. It begins at piece `piece` of the
/// record at `offset`: a Pi record can hold the end of one message and the start of the next.
pub(super) struct Message {pub number:u64,pub role:&'static str,pub timestamp:Option<String>,pub offset:u64,pub piece:u64,pub text:String,pub tools:Vec<String>}

/// Append a piece's tool names in order, an immediate repeat collapsed, as the transcript reader keeps them.
fn add_tools(message:&mut Message,tools:Vec<String>) {for tool in tools {if message.tools.last()!=Some(&tool) {message.tools.push(tool);}}}

/// A whole conversation as `vrefer` prints it, read straight from the provider's records in the numbering
/// `vsearch` reports, so `vrefer --range` reads back exactly the messages a search hit names. None for a
/// provider that is not read directly.
pub(crate) fn conversation(kind:SessionKind,id:&str,cancel:&AtomicBool) -> Result<Option<Vec<transcript::TranscriptMessage>>,String> {
    let convert=|message:Message|transcript::TranscriptMessage {role:message.role.into(),text:message.text,timestamp:message.timestamp,tools:message.tools};
    if matches!(kind,SessionKind::Opencode|SessionKind::Kiro) {
        let (found,_)=native_messages(kind,id,cancel)?.ok_or("conversation recording is unavailable")?;
        return Ok(Some(found.into_iter().map(convert).collect()));
    }
    if !supported(kind) {return Ok(None);}
    let path=transcript::source_path(kind,id).filter(|path|path.is_file()).ok_or("conversation recording is unavailable")?;
    let mut found=Vec::new();
    Transcript::open(kind,&path,cancel)?.messages(None,cancel,|message| {found.push(convert(message));Ok(())})?;
    Ok(Some(found))
}

/// Where the last message began, so a conversation that only grew is read again from there. `head`
/// hashes the bytes in front of it that identify the file; `before` counts the selected records in
/// front of it, which changes when a rewind moves the Claude branch.
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub(super) struct Resume {pub offset:u64,#[serde(default)] pub piece:u64,pub number:u64,pub head:String,pub before:u64}

fn check(cancel:&AtomicBool) -> Result<(),String> {if cancel.load(Ordering::Relaxed) {Err("read_canceled".into())} else {Ok(())}}

/// A complete line, or the start of one too long to hold.
enum Line<'a> {Text(&'a [u8]),Oversized}

/// Visit every complete line from `from`, up to `to` when given, with its starting offset.
fn lines(path:&Path,from:u64,to:Option<u64>,cancel:&AtomicBool,mut each:impl FnMut(u64,Line)->Result<(),String>) -> Result<(),String> {
    let mut file=File::open(path).map_err(|_|"source_missing: search source is unavailable")?;
    file.seek(SeekFrom::Start(from)).map_err(|e|e.to_string())?;
    let mut reader=BufReader::with_capacity(1024*1024,file.take(to.map_or(u64::MAX,|to|to.saturating_sub(from))));
    let mut line=Vec::new();let mut start=from;let mut oversized=false;let mut position=from;
    let mut block=vec![0;256*1024];let mut since_check=0usize;
    loop {
        let count=reader.read(&mut block).map_err(|e|e.to_string())?;
        if count==0 {return Ok(());}
        since_check+=count;if since_check>=4*1024*1024 {check(cancel)?;since_check=0;}
        let mut rest=&block[..count];
        while !rest.is_empty() {
            match rest.iter().position(|byte|*byte==b'\n') {
                Some(at)=> {
                    if !oversized {
                        if line.len()+at>LINE_BYTES {oversized=true;line.clear();} else {line.extend_from_slice(&rest[..at]);}
                    }
                    position+=at as u64+1;
                    each(start,if oversized {Line::Oversized} else {Line::Text(&line)})?;
                    line.clear();oversized=false;start=position;rest=&rest[at+1..];
                },
                None=> {
                    if !oversized {
                        if line.len()+rest.len()>LINE_BYTES {oversized=true;line=Vec::new();} else {line.extend_from_slice(rest);}
                    }
                    position+=rest.len() as u64;rest=&[];
                },
            }
        }
    }
}

/// The fields that place a record in its conversation tree: Claude names a record by `uuid`, Pi and
/// OMP by `id`.
#[derive(Deserialize,Default)]
struct Head {
    uuid:Option<Value>,
    id:Option<Value>,
    #[serde(rename="parentUuid")] parent_uuid:Option<Value>,
    #[serde(rename="logicalParentUuid")] logical_parent:Option<Value>,
    #[serde(rename="parentId")] parent_id:Option<Value>,
    #[serde(rename="type")] kind:Option<Value>,
    rewound:Option<Value>,
    #[serde(rename="leafUuid")] leaf:Option<Value>,
}
impl Head {
    /// The parent the history index records, and whether it is Claude's `logicalParentUuid`, which
    /// a compaction boundary uses only when it has no `parentUuid`.
    fn parent(&self) -> (Option<String>,bool) {
        let text=|value:&Option<Value>|value.as_ref().and_then(Value::as_str).map(str::to_owned);
        match text(&self.parent_uuid).or_else(||text(&self.parent_id)) {
            Some(parent)=>(Some(parent),false),
            None=>{let logical=text(&self.logical_parent);let is=logical.is_some();(logical,is)},
        }
    }
    fn rewind(&self) -> bool {self.kind.as_ref().and_then(Value::as_str)==Some("last-prompt") && self.rewound.as_ref().and_then(Value::as_bool)==Some(true)}
}

/// Read only the tree fields of a line too long to hold, streaming through the rest.
fn streamed_head(path:&Path,offset:u64) -> Head {
    let Ok(mut file)=File::open(path) else {return Head::default();};
    if file.seek(SeekFrom::Start(offset)).is_err() {return Head::default();}
    let mut stream=serde_json::Deserializer::from_reader(BufReader::new(file)).into_iter::<Head>();
    stream.next().and_then(Result::ok).unwrap_or_default()
}

/// Which records of a conversation are on its active branch. Codex has no branches.
enum Selection {All,Records(HashSet<u64>)}
impl Selection {
    fn contains(&self,offset:u64) -> bool {match self {Self::All=>true,Self::Records(records)=>records.contains(&offset)}}
    fn before(&self,offset:u64) -> Option<u64> {match self {Self::All=>None,Self::Records(records)=>Some(records.iter().filter(|start|**start<offset).count() as u64)}}
}

/// The active branch as the history index selects it: from the last record with an id, or the leaf a
/// rewind names, up through the parents. Records without an id are always selected.
fn tree_selection(kind:SessionKind,path:&Path,cancel:&AtomicBool) -> Result<Selection,String> {
    let mut records:Vec<(u64,Option<String>)>=Vec::new();
    // Each record carrying an id: where it starts, its parent, and whether that parent is logical.
    let mut parents:HashMap<String,Vec<(u64,Option<String>,bool)>>=HashMap::new();
    let mut latest:Option<Option<String>>=None;
    lines(path,0,None,cancel,|offset,line| {
        let head=match line {
            Line::Text(bytes)=>serde_json::from_slice::<Head>(bytes).unwrap_or_default(),
            Line::Oversized=>streamed_head(path,offset),
        };
        let uuid=if kind==SessionKind::Claude {&head.uuid} else {&head.id}.as_ref().and_then(Value::as_str).map(str::to_owned);
        if let Some(id)=&uuid {let (parent,logical)=head.parent();parents.entry(id.clone()).or_default().push((offset,parent,logical));}
        // A rewind names its leaf even when the record carries an id of its own.
        if head.rewind() {latest=Some(head.leaf.as_ref().and_then(Value::as_str).map(str::to_owned));}
        else if uuid.is_some() {latest=Some(uuid.clone());}
        records.push((offset,uuid));Ok(())
    })?;
    // Nothing to choose between: every record stays selected, as in the history index.
    let Some(leaf)=latest else {return Ok(Selection::All);};
    let mut chain=HashSet::new();
    if let Some(leaf)=leaf {
        let mut pending=vec![leaf];let mut root=false;
        while let Some(id)=pending.pop() {
            if !chain.insert(id.clone()) {continue;}
            if chain.len()>MAX_CHAIN {return Err("index_incomplete: native history branch is cyclic, too deep, or missing an ancestor".into());}
            let Some(found)=parents.get(&id) else {return Err("index_incomplete: native history branch is cyclic, too deep, or missing an ancestor".into());};
            for (offset,parent,logical) in found {
                // Only a parent written earlier is followed, as the history index does, so the walk
                // cannot loop; a compaction boundary whose summarized history is named by a later
                // record, and a record whose parent was never written (Claude drops a reply that
                // failed to produce a valid tool call but keeps the retry prompt naming it), continue
                // from the record written just before it.
                let earlier=||records[..records.partition_point(|(at,_)|at<offset)].iter().rev().find_map(|(_,uuid)|uuid.clone());
                let parent=match parent {
                    Some(parent)=>match parents.get(parent) {
                        Some(starts) if starts.iter().any(|(at,..)|at<offset) =>Some(parent.clone()),
                        Some(_) =>if *logical {earlier()} else {None},
                        None=>earlier(),
                    },
                    None=>None,
                };
                match parent {Some(parent)=>pending.push(parent),None=>root=true}
            }
        }
        if !root {return Err("index_incomplete: native history branch is cyclic, too deep, or missing an ancestor".into());}
    }
    Ok(Selection::Records(records.into_iter().filter(|(_,uuid)|uuid.as_ref().is_none_or(|id|chain.contains(id))).map(|(offset,_)|offset).collect()))
}

/// The first bytes of the file, up to `limit`.
fn head(path:&Path,limit:u64) -> Result<String,String> {
    let mut file=File::open(path).map_err(|_|"source_missing: search source is unavailable")?;
    let mut bytes=Vec::new();(&mut file).take(limit.min(HEAD_BYTES)).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    Ok(Sha256::digest(&bytes).iter().map(|byte|format!("{byte:02x}")).collect())
}

/// Messages of a provider that keeps its conversations in a database rather than a file: OpenCode and
/// Kiro. They carry no record offset; a hit is located by its message number. Returns the messages and a
/// revision that changes when any of them does.
pub(super) fn native_messages(kind:SessionKind,id:&str,cancel:&AtomicBool) -> Result<Option<(Vec<Message>,String)>,String> {
    let pieces:Vec<transcript::Piece>=match kind {
        SessionKind::Opencode=> {
            let Some(path)=crate::agent::opencode_store::db_path().filter(|path|path.is_file()) else {return Ok(None);};
            let messages=match crate::agent::opencode_store::messages_in(&path,id) {
                Ok(messages)=>messages,
                Err(error) if error.starts_with("OpenCode has no session") || error.contains("OpenCode's store")=>return Ok(None),
                Err(error)=>return Err(error),
            };
            check(cancel)?;transcript::opencode_pieces(&messages)
        },
        SessionKind::Kiro=> {
            let session=match crate::agent::kiro_store::read(id) {Ok(session)=>session,Err(_)=>return Ok(None)};
            let messages=session.messages.map_err(|error|format!("unsupported_record: {error}"))?;
            messages.into_iter().map(|message|transcript::Piece {role:message.role,text:message.text,timestamp:message.timestamp,tools:Vec::new()}).collect()
        },
        _=>return Err("unsupported_record: this provider is read from its recording file".into()),
    };
    check(cancel)?;
    let found=native_grouped(kind,pieces);let mut revision=Sha256::new();
    for message in &found {
        for part in [message.role,message.timestamp.as_deref().unwrap_or(""),&message.text] {revision.update((part.len() as u64).to_le_bytes());revision.update(part.as_bytes());}
    }
    Ok(Some((found,revision.finalize().iter().map(|byte|format!("{byte:02x}")).collect())))
}

/// Database pieces in conversation-view numbering: every Kiro message stands alone; the others merge
/// one role's consecutive pieces, and a cross-session submission starts a message of its own.
fn native_grouped(kind:SessionKind,pieces:Vec<transcript::Piece>) -> Vec<Message> {
    let mut found:Vec<Message>=Vec::new();let mut role="";let mut workflow=false;
    for piece in pieces {
        let body=piece.text.starts_with("[VelaTerm message ");
        let boundary=kind==SessionKind::Kiro || piece.role=="user" && (workflow || body);
        if role!=piece.role || boundary || found.is_empty() {
            role=piece.role;workflow=body;
            found.push(Message {number:found.len() as u64,role,timestamp:piece.timestamp.clone(),offset:0,piece:0,text:String::new(),tools:Vec::new()});
        }
        let message=found.last_mut().unwrap();
        if !piece.text.is_empty() {
            if !message.text.is_empty() {message.text.push_str("\n\n");}
            message.text.push_str(&piece.text);
        }
        add_tools(message,piece.tools);
    }
    found
}

/// Hand on a finished message. Grok leaves out a message without readable text or a tool call and
/// numbers the rest consecutively, as the conversation view's prose does.
fn finish(done:(Message,bool),grok:&mut Option<u64>,each:&mut impl FnMut(Message)->Result<(),String>) -> Result<(),String> {
    let (mut message,tools)=done;
    let Some(kept)=grok else {return each(message);};
    if !tools && !message.text.chars().any(|ch|!ch.is_whitespace()) {return Ok(());}
    message.number=*kept;*kept+=1;each(message)
}

/// The text Antigravity shows for a user step: what the user typed between `<USER_REQUEST>` tags,
/// or the whole content without them, trimmed. None when nothing visible remains.
fn antigravity_request(content:&str) -> Option<&str> {
    const OPEN:&str="<USER_REQUEST>";const CLOSE:&str="</USER_REQUEST>";
    let request=match content.find(OPEN) {
        Some(at)=>{let rest=&content[at+OPEN.len()..];&rest[..rest.find(CLOSE)?]},
        None=>content,
    };
    let visible=request.trim();(!visible.is_empty()).then_some(visible)
}

/// A record's pieces, and whether the conversation view shows the record at all. Text of a record it
/// leaves out (Codex's injected AGENTS.md instructions, for one) is not part of any message there.
fn pieces(kind:SessionKind,raw:&str) -> (Vec<transcript::Piece>,bool) {
    let pieces:Vec<transcript::Piece>=match kind {
        SessionKind::Claude=>transcript::parse_claude_line(raw).into_iter().collect(),
        SessionKind::Codex=>transcript::parse_codex_line(raw).into_iter().collect(),
        SessionKind::Grok=>return (serde_json::from_str::<Value>(raw).map(|value|transcript::grok_pieces(&value)).unwrap_or_default(),true),
        SessionKind::Antigravity=> {
            let Ok(mut value)=serde_json::from_str::<Value>(raw) else {return (Vec::new(),true);};
            // A user step reads as its visible request; annotations around it are not shown.
            let mut shown=true;
            if value["type"]=="USER_INPUT" {
                let visible=value["content"].as_str().and_then(antigravity_request).map(str::to_owned);
                shown=visible.is_some();value["content"]=Value::String(visible.unwrap_or_default());
            }
            return (transcript::antigravity_pieces(&value.to_string()),shown);
        },
        _=>transcript::pi_pieces(raw),
    };
    let shown=!pieces.iter().any(|piece|!piece.text.is_empty()) || !match kind {
        SessionKind::Claude=>export::claude_events(raw).is_empty(),
        SessionKind::Codex=>export::codex_events(raw).is_empty(),
        _=>export::pi_events(raw).is_empty(),
    };
    (pieces,shown)
}

/// The part of a Codex parent rollout in front of a fork. A `thread/fork` rollout stores only its own
/// turns and names where the parent's history ends in `session_meta.payload.history_base`.
struct Ancestor {path:PathBuf,end_byte:Option<u64>,end_ordinal:Option<u64>}

#[derive(Deserialize)]
struct Ordinal {ordinal:Option<u64>}
#[derive(Deserialize)]
struct Meta {#[serde(rename="type")] kind:Option<String>,payload:Option<MetaPayload>}
#[derive(Deserialize)]
struct MetaPayload {id:Option<String>}

/// The thread id in a Codex rollout's `session_meta`, which leads the file.
fn codex_thread(path:&Path) -> Option<String> {
    let file=File::open(path).ok()?;
    serde_json::Deserializer::from_reader(BufReader::new(file)).into_iter::<Meta>().take(16).map_while(Result::ok)
        .find(|meta|meta.kind.as_deref()==Some("session_meta")).and_then(|meta|meta.payload?.id)
}

/// The parent intervals in front of a forked Codex rollout, oldest first, as the conversation view joins
/// them. A fork whose ancestry cannot be read in full is read from its own file alone.
fn codex_ancestors(path:&Path,lookup:&impl Fn(&str)->Option<PathBuf>) -> Vec<Ancestor> {
    let Some(id)=codex_thread(path) else {return Vec::new();};
    let lookup=|wanted:&str| if wanted==id {Some(path.to_path_buf())} else {lookup(wanted)};
    let Ok(resolved)=crate::agent::history_store::source::resolve_codex(&id,&lookup) else {return Vec::new();};
    if resolved.error.is_some() {return Vec::new();}
    let mut segments=resolved.segments;segments.pop();
    segments.into_iter().map(|segment|Ancestor {path:segment.path,end_byte:segment.end_byte,end_ordinal:segment.end_ordinal}).collect()
}

/// One conversation file, with its active branch resolved and, for a forked Codex rollout, the parent
/// history in front of it.
pub(super) struct Transcript<'a> {kind:SessionKind,path:&'a Path,selection:Selection,ancestors:Vec<Ancestor>}

impl<'a> Transcript<'a> {
    pub(super) fn open(kind:SessionKind,path:&'a Path,cancel:&AtomicBool) -> Result<Self,String> {
        Self::open_with(kind,path,&crate::agent::resume::find_codex_rollout,cancel)
    }

    fn open_with(kind:SessionKind,path:&'a Path,lookup:&impl Fn(&str)->Option<PathBuf>,cancel:&AtomicBool) -> Result<Self,String> {
        if !supported(kind) {return Err("unsupported_record: direct transcript reading is unavailable for this provider".into());}
        let selection=if tree(kind) {tree_selection(kind,path,cancel)?} else {Selection::All};
        let ancestors=if kind==SessionKind::Codex {codex_ancestors(path,lookup)} else {Vec::new()};
        Ok(Self {kind,path,selection,ancestors})
    }

    /// Whether messages come from more than one file, so a byte offset alone does not name a record.
    pub(super) fn chained(&self) -> bool {!self.ancestors.is_empty()}

    /// Whether the messages in front of `resume` are still the ones read before.
    pub(super) fn resumes(&self,resume:&Resume) -> Result<bool,String> {
        Ok(self.selection.contains(resume.offset) && self.selection.before(resume.offset).is_none_or(|count|count==resume.before)
            && head(self.path,resume.offset)?==resume.head)
    }

    /// Visit every message from `resume` on (from the first message without one), returning the total
    /// message count and where the last message began.
    pub(super) fn messages(&self,resume:Option<&Resume>,cancel:&AtomicBool,mut each:impl FnMut(Message)->Result<(),String>) -> Result<(u64,Option<Resume>),String> {
        let resume=resume.filter(|_|resumable(self.kind));
        let mut total=resume.map_or(0,|resume|resume.number);
        // A message and whether it called a tool. Grok keeps its own count of the messages it keeps.
        let mut current:Option<(Message,bool)>=None;let mut role="";let mut workflow=false;
        let mut grok=(self.kind==SessionKind::Grok).then_some(0u64);
        // Whether the message in progress began in this file rather than in a parent in front of it.
        let mut own=true;
        // Pieces in front of the resumed message in its first record belong to the message before it.
        let mut skip=resume.map(|resume|(resume.offset,resume.piece));
        let mut visit=|offset:u64,raw:&str,in_file:bool| -> Result<(),String> {
            let (pieces,shown)=pieces(self.kind,raw);
            if pieces.is_empty() {return Ok(());}
            let first=skip.take().filter(|(at,_)|*at==offset).map_or(0,|(_,piece)|piece);
            for (index,piece) in pieces.into_iter().enumerate() {
                if (index as u64)<first {continue;}
                if !shown && !piece.text.is_empty() {continue;}
                let boundary=grok.is_none() && piece.role=="user" && (workflow || piece.text.starts_with("[VelaTerm message "));
                if role!=piece.role || boundary {
                    if let Some(done)=current.take() {finish(done,&mut grok,&mut each)?;}
                    total+=1;role=piece.role;workflow=piece.text.starts_with("[VelaTerm message ");own=in_file;
                    current=Some((Message {number:total-1,role,timestamp:piece.timestamp.clone(),offset,piece:index as u64,text:String::new(),tools:Vec::new()},false));
                }
                let (message,tools)=current.as_mut().unwrap();
                *tools|=!piece.tools.is_empty();
                if !piece.text.is_empty() {
                    // Grok streams a message as chunks of one text; the others join whole blocks.
                    if !message.text.is_empty() && grok.is_none() {message.text.push_str("\n\n");}
                    message.text.push_str(&piece.text);
                }
                add_tools(message,piece.tools);
            }
            Ok(())
        };
        // A resumed read begins in this file; the parent history in front of it was read before.
        if resume.is_none() {
            for ancestor in &self.ancestors {
                let mut ended=false;
                lines(&ancestor.path,0,ancestor.end_byte,cancel,|offset,line| {
                    let Line::Text(bytes)=line else {return Ok(());};
                    let Ok(raw)=std::str::from_utf8(bytes) else {return Ok(());};
                    // The byte cutoff already bounds the read; the ordinal one applies only without it.
                    if let (None,Some(end))=(ancestor.end_byte,ancestor.end_ordinal) {
                        ended|=serde_json::from_str::<Ordinal>(raw).ok().and_then(|record|record.ordinal).is_some_and(|ordinal|ordinal>=end);
                    }
                    if ended {return Ok(());}
                    visit(offset,raw,false)
                })?;
            }
        }
        lines(self.path,resume.map_or(0,|resume|resume.offset),None,cancel,|offset,line| {
            let Line::Text(bytes)=line else {return Ok(());};
            if !self.selection.contains(offset) {return Ok(());}
            let Ok(raw)=std::str::from_utf8(bytes) else {return Ok(());};
            visit(offset,raw,true)
        })?;
        // Resuming starts in this file, so a last message that began in a parent is read again in full.
        let last=match current.as_ref().filter(|_|resumable(self.kind) && own) {
            Some((message,_))=>Some(Resume {offset:message.offset,piece:message.piece,number:message.number,head:head(self.path,message.offset)?,
                before:self.selection.before(message.offset).unwrap_or(0)}),
            None=>None,
        };
        if let Some(done)=current.take() {finish(done,&mut grok,&mut each)?;}
        if let Some(kept)=grok {total=kept;}
        // An unchanged conversation without messages keeps the point it was resumed from.
        Ok((total,last.or_else(||resume.cloned().filter(|_|resumable(self.kind)))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Arc;
    use serde_json::json;
    use crate::agent::history_store::{self as history,prose::Prose,store::Store};

    struct Temp(std::path::PathBuf);
    impl Temp {fn new() -> Self {let path=std::env::temp_dir().join(format!("vlx-search-extract-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&path).unwrap();Self(path)}}
    impl Drop for Temp {fn drop(&mut self) {history::store::release_test_directory(&self.0);let _=std::fs::remove_dir_all(&self.0);}}

    fn lines_of(values:&[Value]) -> String {values.iter().map(|value|value.to_string()+"\n").collect()}

    /// Messages read from the file directly, as (number, role, text, offset).
    fn direct(kind:SessionKind,path:&Path) -> Vec<(u64,String,String,u64)> {
        let cancel=AtomicBool::new(false);let mut found=Vec::new();
        Transcript::open(kind,path,&cancel).unwrap().messages(None,&cancel,|message| {
            found.push((message.number,message.role.to_owned(),message.text,message.offset));Ok(())
        }).unwrap();
        found
    }

    /// The same conversation through the history index, as the conversation view reads it: each message's
    /// number, role and text, and the row its number locates.
    fn indexed(kind:SessionKind,temp:&Temp,path:&Path) -> (history::api::Branch,Vec<(u64,String,String,Option<String>)>) {
        let store=Store::at(&temp.0).unwrap();let cancel=Arc::new(AtomicBool::new(false));
        let source=store.register(kind,history::source::Segment {path:path.into(),native_id:"thread".into(),end_byte:None,end_ordinal:None}).unwrap();
        let branch=history::api::bind(store,vec![source],kind,"thread".into(),None,"test-view").unwrap();
        let branch=history::event_stream::wait(branch,&cancel).unwrap();
        let prose=Prose::new(branch.clone(),&cancel).unwrap();let mut found=Vec::new();
        for number in 0..prose.total {
            let metadata=prose.metadata(number).unwrap().unwrap();let mut text=Vec::new();
            prose.body(number,&cancel,&mut text).unwrap();let text=String::from_utf8(text).unwrap();
            found.push((number,metadata["role"].as_str().unwrap().to_owned(),text,metadata["rowId"].as_str().map(str::to_owned)));
        }
        (branch,found)
    }

    /// Direct reading agrees with the history index on numbering, roles and text, and every message's
    /// offset locates the same row its number does.
    fn agree(kind:SessionKind,raw:&str) -> Vec<(u64,String,String,u64)> {
        let temp=Temp::new();let path=temp.0.join("native.jsonl");std::fs::write(&path,raw).unwrap();
        let read=direct(kind,&path);let (branch,expected)=indexed(kind,&temp,&path);
        assert_eq!(read.iter().map(|m|(m.0,m.1.clone(),m.2.clone())).collect::<Vec<_>>(),expected.iter().map(|m|(m.0,m.1.clone(),m.2.clone())).collect::<Vec<_>>());
        for ((_,_,_,offset),(number,_,_,row)) in read.iter().zip(&expected) {
            if row.is_some() {assert_eq!(&branch.row_at_record(*offset).unwrap(),row,"message {number}");}
        }
        read
    }

    fn claude(uuid:&str,parent:Option<&str>,kind:&str,content:Value) -> Value {
        json!({"parentUuid":parent,"isSidechain":false,"type":kind,"message":{"role":kind,"content":content},"uuid":uuid,"timestamp":"2026-10-19T08:00:00.000Z"})
    }

    #[test]
    fn claude_messages_follow_the_active_branch_and_the_view_numbering() {
        let raw=lines_of(&[
            claude("u1",None,"user",json!("Where is the alpha config?")),
            claude("a1",Some("u1"),"assistant",json!([{"type":"text","text":"Let me look."},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}])),
            claude("r1",Some("a1"),"user",json!([{"type":"tool_result","tool_use_id":"t1","content":"settings.toml"}])),
            claude("a2",Some("r1"),"assistant",json!([{"type":"text","text":"It lives in settings.toml."}])),
            claude("s1",Some("a2"),"user",json!("<system-reminder>injected</system-reminder>")),
            // An abandoned turn: the conversation was rewound to a2 and continued from there.
            claude("u2",Some("s1"),"user",json!("abandoned question")),
            claude("a3",Some("u2"),"assistant",json!([{"type":"text","text":"abandoned answer"}])),
            claude("u3",Some("a2"),"user",json!("[VelaTerm message from planner] first instruction")),
            claude("u4",Some("u3"),"user",json!("[VelaTerm message from planner] second instruction")),
            claude("a4",Some("u4"),"assistant",json!([{"type":"text","text":"Both done."}])),
        ]);
        let read=agree(SessionKind::Claude,&raw);
        assert!(read.iter().all(|m|!m.2.contains("abandoned") && !m.2.contains("injected")));
        assert_eq!(read.iter().map(|m|m.1.as_str()).collect::<Vec<_>>(),["user","assistant","user","user","assistant"]);
        // `vrefer` reads the same messages in the same numbering, with the tools each turn used.
        let temp=Temp::new();let path=temp.0.join("native.jsonl");std::fs::write(&path,&raw).unwrap();
        let cancel=AtomicBool::new(false);let mut referred=Vec::new();
        Transcript::open(SessionKind::Claude,&path,&cancel).unwrap().messages(None,&cancel,|message| {referred.push(message);Ok(())}).unwrap();
        assert_eq!(referred.iter().map(|m|(m.role.to_owned(),m.text.clone())).collect::<Vec<_>>(),read.iter().map(|m|(m.1.clone(),m.2.clone())).collect::<Vec<_>>());
        assert_eq!(referred[1].tools,["Bash"]);assert!(referred[0].tools.is_empty());
    }

    #[test]
    fn claude_rewind_leaf_selects_its_branch() {
        let mut values=vec![
            claude("u1",None,"user",json!("kept question")),
            claude("a1",Some("u1"),"assistant",json!([{"type":"text","text":"kept answer"}])),
            claude("u2",Some("a1"),"user",json!("rewound question")),
            claude("a2",Some("u2"),"assistant",json!([{"type":"text","text":"rewound answer"}])),
        ];
        values.push(json!({"type":"last-prompt","rewound":true,"leafUuid":"a1"}));
        let read=agree(SessionKind::Claude,&lines_of(&values));
        assert_eq!(read.len(),2);assert!(read.iter().all(|m|m.2.starts_with("kept")));
    }

    #[test]
    fn claude_compactions_keep_the_history_they_summarized() {
        let boundary=|uuid:&str,logical:&str| json!({"parentUuid":null,"logicalParentUuid":logical,"isSidechain":false,"type":"system",
            "subtype":"compact_boundary","content":"Conversation compacted","uuid":uuid,"timestamp":"2026-10-19T08:00:00.000Z"});
        let raw=lines_of(&[
            claude("u1",None,"user",json!("before the first compaction")),
            claude("a1",Some("u1"),"assistant",json!([{"type":"text","text":"first answer"}])),
            boundary("c1","a1"),
            claude("u2",Some("c1"),"user",json!("between the compactions")),
            claude("a2",Some("u2"),"assistant",json!([{"type":"text","text":"second answer"}])),
            // A message arrived while the second compaction ran: Claude wrote it after the boundary and
            // named its attachment as the boundary's logical parent, which descends from the boundary itself.
            boundary("c2","t1"),
            claude("u3",Some("c2"),"user",json!("arrived during the compaction")),
            json!({"parentUuid":"u3","isSidechain":false,"type":"attachment","attachment":{"type":"todo"},"uuid":"t1","timestamp":"2026-10-19T08:00:00.000Z"}),
            claude("a3",Some("t1"),"assistant",json!([{"type":"text","text":"after the compaction"}])),
        ]);
        let read=agree(SessionKind::Claude,&raw);
        assert_eq!(read.iter().map(|m|m.2.as_str()).collect::<Vec<_>>(),
            ["before the first compaction","first answer","between the compactions","second answer","arrived during the compaction","after the compaction"]);
    }

    fn pi(id:&str,parent:Option<&str>,role:&str,content:Value) -> Value {
        json!({"type":"message","id":id,"parentId":parent,"timestamp":"2026-10-19T08:00:00.000Z","message":{"role":role,"content":content}})
    }

    #[test]
    fn pi_and_omp_messages_follow_the_active_branch_and_split_records_like_the_view() {
        let raw=lines_of(&[
            json!({"type":"session","id":"s","timestamp":"2026-10-19T08:00:00.000Z"}),
            pi("u1",None,"user",json!([{"type":"text","text":"Where is the alpha config?"}])),
            // One record carries thinking, text, a tool call and more text: several pieces.
            pi("a1",Some("u1"),"assistant",json!([{"type":"thinking","thinking":"pondering"},{"type":"text","text":"Let me look."},
                {"type":"toolCall","id":"c1","name":"bash","arguments":{"command":"ls"}},{"type":"text","text":"Found settings.toml."}])),
            json!({"type":"message","id":"r1","parentId":"a1","message":{"role":"toolResult","toolCallId":"c1","toolName":"bash","content":[{"type":"text","text":"settings.toml"}],"isError":false}}),
            pi("x1",Some("u1"),"assistant",json!([{"type":"text","text":"abandoned answer"}])),
            pi("u2",Some("r1"),"user",json!("[VelaTerm message from planner] first instruction")),
            pi("u3",Some("u2"),"user",json!("[VelaTerm message from planner] second instruction")),
            pi("a2",Some("u3"),"assistant",json!([{"type":"text","text":"Both done."}])),
        ]);
        for kind in [SessionKind::Pi,SessionKind::Omp] {
            let read=agree(kind,&raw);
            assert!(read.iter().all(|m|!m.2.contains("abandoned") && !m.2.contains("pondering")),"{kind:?}");
            assert_eq!(read.iter().map(|m|m.1.as_str()).collect::<Vec<_>>(),["user","assistant","user","user","assistant"],"{kind:?}");
        }
    }

    #[test]
    fn a_pi_message_resumed_mid_record_skips_the_pieces_before_it() {
        let temp=Temp::new();let path=temp.0.join("native.jsonl");let cancel=AtomicBool::new(false);
        std::fs::write(&path,lines_of(&[pi("u1",None,"user",json!("first")),pi("a1",Some("u1"),"assistant",json!([{"type":"text","text":"second"}]))])).unwrap();
        let (_,last)=Transcript::open(SessionKind::Pi,&path,&cancel).unwrap().messages(None,&cancel,|_|Ok(())).unwrap();
        let last=last.unwrap();
        std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(lines_of(&[pi("u2",Some("a1"),"user",json!("third"))]).as_bytes()).unwrap();
        let transcript=Transcript::open(SessionKind::Pi,&path,&cancel).unwrap();assert!(transcript.resumes(&last).unwrap());
        let mut read=Vec::new();
        transcript.messages(Some(&last),&cancel,|message|{read.push((message.number,message.text));Ok(())}).unwrap();
        assert_eq!(read,[(1,"second".to_owned()),(2,"third".to_owned())]);
        // A rewind that abandons the resumed message invalidates the resume point.
        std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(lines_of(&[pi("b1",Some("u1"),"assistant",json!([{"type":"text","text":"other"}]))]).as_bytes()).unwrap();
        assert!(!Transcript::open(SessionKind::Pi,&path,&cancel).unwrap().resumes(&last).unwrap());
    }

    #[test]
    fn database_messages_merge_by_role_except_kiro_and_cross_session_submissions() {
        let piece=|role:&'static str,text:&str,tool:bool| transcript::Piece {role,text:text.into(),timestamp:None,tools:if tool {vec!["bash".into()]} else {Vec::new()}};
        let pieces=||vec![piece("user","Question",false),piece("assistant","Looking.",false),piece("assistant","",true),piece("assistant","Found it.",false),
            piece("user","[VelaTerm message from planner] one",false),piece("user","[VelaTerm message from planner] two",false)];
        let read=native_grouped(SessionKind::Opencode,pieces());
        assert_eq!(read.iter().map(|m|(m.number,m.role,m.text.as_str())).collect::<Vec<_>>(),[(0,"user","Question"),(1,"assistant","Looking.\n\nFound it."),
            (2,"user","[VelaTerm message from planner] one"),(3,"user","[VelaTerm message from planner] two")]);
        assert_eq!(native_grouped(SessionKind::Kiro,pieces()).len(),6);
    }

    fn grok(update:Value) -> Value {json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s","update":update}})}

    #[test]
    fn grok_chunks_join_and_messages_without_text_or_tools_are_left_out() {
        let raw=lines_of(&[
            grok(json!({"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"Where is "}})),
            grok(json!({"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"the alpha config?"}})),
            grok(json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Let me "}})),
            grok(json!({"sessionUpdate":"tool_call","toolCallId":"t1","_meta":{"x.ai/tool":{"name":"bash"}}})),
            grok(json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"look."}})),
            grok(json!({"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"   "}})),
            grok(json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"It is in settings.toml."}})),
        ]);
        let read=agree(SessionKind::Grok,&raw);
        assert_eq!(read.iter().map(|m|m.2.as_str()).collect::<Vec<_>>(),["Where is the alpha config?","Let me look.","It is in settings.toml."]);
        assert_eq!(read.iter().map(|m|m.0).collect::<Vec<_>>(),[0,1,2]);
    }

    #[test]
    fn antigravity_user_steps_read_as_their_visible_request() {
        let raw=lines_of(&[
            json!({"type":"USER_INPUT","step_index":0,"created_at":"2026-10-19T08:00:00Z","content":"<USER_REQUEST>\n Find the omega setting. \n</USER_REQUEST><ADDITIONAL_METADATA>private</ADDITIONAL_METADATA>"}),
            json!({"type":"PLANNER_RESPONSE","step_index":1,"status":"DONE","thinking":"pondering","tool_calls":[{"name":"view_file","args":{"AbsolutePath":"/tmp/a"}}]}),
            json!({"type":"VIEW_FILE","step_index":2,"status":"DONE","content":"omega = 1"}),
            json!({"type":"PLANNER_RESPONSE","step_index":3,"status":"DONE","content":"The omega setting is 1."}),
            json!({"type":"USER_INPUT","step_index":4,"content":"<USER_REQUEST>   </USER_REQUEST>"}),
            json!({"type":"USER_INPUT","step_index":5,"content":"Thanks."}),
            json!({"type":"PLANNER_RESPONSE","step_index":6,"status":"DONE","content":"You are welcome."}),
        ]);
        let read=agree(SessionKind::Antigravity,&raw);
        assert!(read.iter().all(|m|!m.2.contains("private") && !m.2.contains("pondering")));
        assert_eq!(read.iter().map(|m|m.2.as_str()).collect::<Vec<_>>().first(),Some(&"Find the omega setting."));
    }

    fn codex(payload:Value) -> Value {json!({"timestamp":"2026-10-19T08:00:00.000Z","type":"response_item","payload":payload})}
    fn codex_message(role:&str,text:&str) -> Value {
        codex(json!({"type":"message","role":role,"content":[{"type":if role=="user" {"input_text"} else {"output_text"},"text":text}]}))
    }

    #[test]
    fn codex_messages_skip_injected_input_and_match_the_view() {
        let raw=lines_of(&[
            json!({"timestamp":"2026-10-19T08:00:00.000Z","type":"session_meta","payload":{"id":"thread"}}),
            codex_message("user","# AGENTS.md instructions for /repo\n\nBe careful."),
            codex_message("user","<environment_context>cwd</environment_context>"),
            codex_message("user","Find the omega setting."),
            codex(json!({"type":"function_call","name":"shell","call_id":"c1","arguments":"{\"command\":\"rg omega\"}"})),
            codex(json!({"type":"function_call_output","call_id":"c1","output":"omega = 1"})),
            codex_message("assistant","The omega setting is 1."),
            json!({"timestamp":"2026-10-19T08:00:01.000Z","type":"event_msg","payload":{"type":"token_count"}}),
            codex_message("user","Thanks."),
        ]);
        let read=agree(SessionKind::Codex,&raw);
        assert_eq!(read.iter().map(|m|m.2.as_str()).collect::<Vec<_>>(),["Find the omega setting.","The omega setting is 1.","Thanks."]);
    }

    #[test]
    fn a_forked_codex_rollout_reads_its_parent_history_up_to_the_fork_as_the_view_does() {
        let temp=Temp::new();let cancel=Arc::new(AtomicBool::new(false));
        let numbered=|ordinal:u64,mut value:Value| {value["ordinal"]=json!(ordinal);value};
        let head=lines_of(&[
            numbered(0,json!({"timestamp":"2026-10-19T08:00:00.000Z","type":"session_meta","payload":{"id":"parent"}})),
            numbered(1,codex_message("user","Before the fork.")),
            numbered(2,codex_message("assistant","Answered before the fork.")),
        ]);
        let parent=temp.0.join("parent.jsonl");
        std::fs::write(&parent,head.clone()+&lines_of(&[numbered(3,codex_message("user","Only the parent went on."))])).unwrap();
        let child=temp.0.join("child.jsonl");
        std::fs::write(&child,lines_of(&[
            numbered(0,json!({"timestamp":"2026-10-19T08:00:02.000Z","type":"session_meta","payload":{"id":"child",
                "history_base":{"thread_id":"parent","end_ordinal_exclusive":3,"end_byte_offset":head.len()}}})),
            numbered(1,codex_message("user","After the fork.")),
            numbered(2,codex_message("assistant","Answered after the fork.")),
        ])).unwrap();
        let files=[("parent",parent.clone()),("child",child.clone())];
        let lookup=|id:&str|files.iter().find(|(name,_)|*name==id).map(|(_,path)|path.clone());
        let transcript=Transcript::open_with(SessionKind::Codex,&child,&lookup,&cancel).unwrap();assert!(transcript.chained());
        let mut read=Vec::new();
        let (total,last)=transcript.messages(None,&cancel,|message| {read.push((message.number,message.role.to_owned(),message.text));Ok(())}).unwrap();
        assert_eq!(read.iter().map(|m|m.2.as_str()).collect::<Vec<_>>(),["Before the fork.","Answered before the fork.","After the fork.","Answered after the fork."]);
        // The conversation view joins the same intervals and numbers them the same way.
        let store=Store::at(&temp.0).unwrap();
        let sources=history::source::resolve_codex("child",&lookup).unwrap().segments.into_iter().map(|segment|store.register(SessionKind::Codex,segment).unwrap()).collect();
        let branch=history::event_stream::wait(history::api::bind(store,sources,SessionKind::Codex,"child".into(),None,"test-view").unwrap(),&cancel).unwrap();
        let prose=Prose::new(branch,&cancel).unwrap();
        let viewed=(0..prose.total).map(|number| {
            let mut text=Vec::new();prose.body(number,&cancel,&mut text).unwrap();
            (number,prose.metadata(number).unwrap().unwrap()["role"].as_str().unwrap().to_owned(),String::from_utf8(text).unwrap())
        }).collect::<Vec<_>>();
        assert_eq!(read,viewed);
        // A grown fork resumes in its own file and keeps counting after the parent's messages.
        let last=last.unwrap();assert!(transcript.resumes(&last).unwrap());
        std::fs::OpenOptions::new().append(true).open(&child).unwrap().write_all(lines_of(&[numbered(3,codex_message("user","Later."))]).as_bytes()).unwrap();
        let mut grown=Vec::new();
        let (grown_total,_)=transcript.messages(Some(&last),&cancel,|message| {grown.push((message.number,message.text));Ok(())}).unwrap();
        assert_eq!((total,grown_total),(4,5));assert_eq!(grown,[(3,"Answered after the fork.".to_owned()),(4,"Later.".to_owned())]);
        // Without its parent the fork is read from its own file alone.
        let alone=Transcript::open_with(SessionKind::Codex,&child,&|id:&str|(id=="child").then(||child.clone()),&cancel).unwrap();
        assert!(!alone.chained());
    }

    #[test]
    fn a_grown_conversation_resumes_at_its_last_message_and_a_changed_one_does_not() {
        let temp=Temp::new();let path=temp.0.join("native.jsonl");let cancel=AtomicBool::new(false);
        std::fs::write(&path,lines_of(&[codex_message("user","first"),codex_message("assistant","second")])).unwrap();
        let (total,last)=Transcript::open(SessionKind::Codex,&path,&cancel).unwrap().messages(None,&cancel,|_|Ok(())).unwrap();
        let last=last.unwrap();assert_eq!((total,last.number),(2,1));
        std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(lines_of(&[codex_message("assistant","more"),codex_message("user","third")]).as_bytes()).unwrap();
        let transcript=Transcript::open(SessionKind::Codex,&path,&cancel).unwrap();assert!(transcript.resumes(&last).unwrap());
        let mut read=Vec::new();
        let (total,_)=transcript.messages(Some(&last),&cancel,|message|{read.push((message.number,message.text));Ok(())}).unwrap();
        assert_eq!(total,3);assert_eq!(read,[(1,"second\n\nmore".to_owned()),(2,"third".to_owned())]);
        // A line still being written is left for the next read.
        std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{\"type\":\"response_item\"").unwrap();
        assert_eq!(direct(SessionKind::Codex,&path).len(),3);
        // Bytes in front of the resume point changed: the whole conversation is read again.
        std::fs::write(&path,lines_of(&[codex_message("user","FIRST"),codex_message("assistant","second")])).unwrap();
        assert!(!Transcript::open(SessionKind::Codex,&path,&cancel).unwrap().resumes(&last).unwrap());
    }

    #[test]
    fn an_oversized_line_is_streamed_for_its_branch_identity_and_left_out_of_the_text() {
        let temp=Temp::new();let path=temp.0.join("native.jsonl");
        let huge="x".repeat(LINE_BYTES+16);
        let raw=lines_of(&[
            claude("u1",None,"user",json!("small question")),
            claude("a1",Some("u1"),"assistant",json!([{"type":"text","text":huge}])),
            claude("u2",Some("a1"),"user",json!("follow-up")),
        ]);
        std::fs::write(&path,raw).unwrap();
        // The follow-up's parent is found on the active branch; without the long answer the two user
        // turns read as one message.
        let read=direct(SessionKind::Claude,&path);
        assert_eq!(read.iter().map(|m|m.2.as_str()).collect::<Vec<_>>(),["small question\n\nfollow-up"]);
    }
}

/// Compares direct reading with the history index on real recordings, listed one per line as
/// `<claude|codex|pi|omp> <path>` in the file VLX_COMPARE_LIST; the index is built under VLX_COMPARE_DIR.
#[cfg(test)]
mod real {
    use super::*;
    use std::sync::Arc;
    use crate::agent::history_store::{self as history,prose::Prose,store::Store};

    #[test]
    #[ignore]
    fn compare_real_transcripts() {
        let (Ok(list),Ok(dir))=(std::env::var("VLX_COMPARE_LIST"),std::env::var("VLX_COMPARE_DIR")) else {return;};
        let dir=std::path::PathBuf::from(dir);let store=Store::at(&dir).unwrap();let cancel=Arc::new(AtomicBool::new(false));
        let (mut same,mut different,mut failed)=(0,0,0);
        for (index,line) in std::fs::read_to_string(list).unwrap().lines().enumerate() {
            let Some((kind,path))=line.split_once(' ') else {continue;};
            let kind=match kind {"claude"=>SessionKind::Claude,"pi"=>SessionKind::Pi,"omp"=>SessionKind::Omp,_=>SessionKind::Codex};let path=Path::new(path);
            let mut direct=Vec::new();
            let read=Transcript::open(kind,path,&cancel).and_then(|transcript|transcript.messages(None,&cancel,|message| {
                direct.push((message.role.to_owned(),message.text,message.offset));Ok(())
            }));
            let indexed=(|| -> Result<(history::api::Branch,Vec<(String,String,Option<String>)>),String> {
                let source=store.register(kind,history::source::Segment {path:path.into(),native_id:format!("compare-{index}"),end_byte:None,end_ordinal:None})?;
                let branch=history::api::bind(store.clone(),vec![source],kind,format!("compare-{index}"),None,&format!("compare:{index}"))?;
                let branch=history::event_stream::wait(branch,&cancel)?;
                let prose=Prose::new(branch.clone(),&cancel)?;let mut found=Vec::new();
                for number in 0..prose.total {
                    let metadata=prose.metadata(number)?.unwrap();let mut text=Vec::new();
                    prose.body(number,&cancel,&mut text)?;let text=String::from_utf8(text).map_err(|e|e.to_string())?;
                    found.push((metadata["role"].as_str().unwrap().to_owned(),text,metadata["rowId"].as_str().map(str::to_owned)));
                }
                Ok((branch,found))
            })();
            match (read,indexed) {
                (Ok(_),Ok((branch,expected))) => {
                    let first=direct.iter().zip(&expected).position(|(d,e)|d.0!=e.0 || d.1!=e.1);
                    let rows=direct.iter().zip(&expected).filter(|(d,e)|e.2.is_some() && branch.row_at_record(d.2).ok().flatten()!=e.2).count();
                    if direct.len()==expected.len() && first.is_none() && rows==0 {same+=1;} else {
                        different+=1;
                        eprintln!("DIFF {line}: {} vs {} messages, first difference {first:?}, {rows} rows located differently",direct.len(),expected.len());
                        if let Some(at)=first {eprintln!("  direct  {:?} {:?}\n  indexed {:?} {:?}",direct[at].0,direct[at].1.chars().take(120).collect::<String>(),
                            expected[at].0,expected[at].1.chars().take(120).collect::<String>());}
                    }
                },
                (direct,indexed) => {failed+=1;eprintln!("FAIL {line}: direct {:?} indexed {:?}",direct.err(),indexed.err());},
            }
        }
        eprintln!("same {same}, different {different}, failed {failed}");
        assert_eq!(different,0);
    }

    /// Searches a copied data directory, VLX_LOCATE_DATA=<dir>, for VLX_LOCATE_QUERY and opens every
    /// transcript hit the way the conversation view does, through `chat_snapshot` with its record offset.
    /// Each must land on the row holding the match, the same row its message number locates.
    #[test]
    #[ignore]
    fn locate_real_search_hits() {
        let (Ok(dir),Ok(query))=(std::env::var("VLX_LOCATE_DATA"),std::env::var("VLX_LOCATE_QUERY")) else {return;};
        let root=std::path::PathBuf::from(dir);
        let app=crate::host::AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(root.clone(),crate::db::Db::open(&root.join("vlx-term.db")).unwrap())));
        let result=super::super::jobs::blocking(&app,&query,super::super::SearchScope::All,None,50,true).unwrap();
        let snapshot=|sid:&str,location:Value| -> Result<Value,String> {
            let mut args=serde_json::json!({"sessionId":sid,"historyOnly":true,"protocolVersion":2,"preferBounded":true});
            args.as_object_mut().unwrap().extend(location.as_object().unwrap().clone());
            let began=std::time::Instant::now();
            // Like the view, keep one reader while waiting and release it when done: releasing it
            // also stops the scan it waits for.
            loop {
                let page=crate::web::dispatch::dispatch(&app,"chat_snapshot",&args,"test",crate::web::dispatch::CallOrigin::Local);
                let page=match page {Ok(page)=>page,Err(error)=> {
                    if let Some(reader)=args["readJobId"].as_str() {
                        let _=crate::web::dispatch::dispatch(&app,"chat_read_cancel",&serde_json::json!({"sessionId":sid,"jobId":reader}),"test",crate::web::dispatch::CallOrigin::Local);
                    }
                    return Err(error);
                }};
                if let Some(reader)=page["readJobId"].as_str() {args["readJobId"]=serde_json::json!(reader);}
                if page.get("pendingLocation").is_none_or(Value::is_null) || began.elapsed().as_secs()>600 {
                    if let Some(reader)=args["readJobId"].as_str() {
                        let _=crate::web::dispatch::dispatch(&app,"chat_read_cancel",&serde_json::json!({"sessionId":sid,"jobId":reader}),"test",crate::web::dispatch::CallOrigin::Local);
                    }
                    return Ok(page);
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        };
        let (mut located,mut failed)=(0,0);
        for hit in result["hits"].as_array().unwrap() {
            let sid=hit["sessionId"].as_str().unwrap();
            if !matches!(hit["kind"].as_str(),Some("claude"|"codex"|"pi"|"omp")) {continue;}
            for found in hit["matches"].as_array().unwrap() {
                let (Some(offset),Some(index))=(found["recordOffset"].as_u64(),found["messageIndex"].as_u64()) else {continue;};
                // A Codex fork numbers its messages after the inherited ones in the view, but search numbers
                // its own recording only: there the offset alone is comparable.
                let fork=crate::agent::history_store::api::prepare(&app,sid).map(|branch|branch.sources.len()>1).unwrap_or(false);
                let (by_offset,by_index)=(snapshot(sid,serde_json::json!({"recordOffset":offset})),snapshot(sid,serde_json::json!({if fork {"recordOffset"} else {"messageIndex"}:if fork {offset} else {index}})));
                let (by_offset,by_index)=match (by_offset,by_index) {
                    (Ok(a),Ok(b))=>(a,b),
                    (a,b)=> {eprintln!("{} msg {index} offset {offset}: by offset {:?} / by index {:?}",hit["name"],a.err(),b.err());failed+=1;continue;},
                };
                let row=by_offset["locatedRowId"].as_str().unwrap();
                let text=by_offset["rows"].as_array().unwrap().iter().find(|r|r["id"]==row).map(|r|r.to_string()).unwrap_or_default();
                eprintln!("{} msg {index} offset {offset}: row {row} {} index-row-match={}",hit["name"],if text.to_lowercase().contains(&query.to_lowercase()) {"contains query"} else {"preview lacks query"},
                    by_index["locatedRowId"]==by_offset["locatedRowId"]);
                assert_eq!(by_index["locatedRowId"],by_offset["locatedRowId"]);located+=1;
            }
        }
        eprintln!("located {located}, failed {failed}");assert!(located>0);
    }
}
