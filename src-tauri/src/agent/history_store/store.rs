//! In-memory derived history over native recordings, with one scan per physical source.
//!
//! Nothing here is written to disk. A recording opened in this process is scanned once into row
//! metadata and bounded previews; field bodies, tool details and prose are read again from the original
//! bytes on demand, verified against the block hashes taken while scanning. An index that no reader
//! has used for a while is dropped and rebuilt from the recording when it is opened again.

use std::collections::{BTreeMap,HashMap,HashSet};
use std::fs::File;
use std::io::{Read,Seek,SeekFrom};
use std::path::{Path,PathBuf};
use std::sync::{Arc,Mutex,OnceLock,RwLock};
use std::sync::atomic::{AtomicBool,Ordering};
use std::time::{Duration,Instant};

use sha2::{Digest,Sha256};

use crate::models::SessionKind;
use super::limits::*;
use super::projection::{Checkpoint,Projector,Record};
use super::source::{self,Segment};

pub(super) fn digest(value:&[u8]) -> String {
    // A scan names every field this way; formatting each byte separately allocated per byte.
    const HEX:&[u8;16]=b"0123456789abcdef";
    let mut text=String::with_capacity(64);
    for byte in Sha256::digest(value) {text.push(HEX[(byte>>4) as usize] as char);text.push(HEX[(byte&15) as usize] as char);}
    text
}

/// How long an index nobody holds stays in memory.
const IDLE:Duration=Duration::from_secs(10*60);
/// Estimated bytes of idle indexes kept beyond [`IDLE`]; the least recently used go first.
const IDLE_BUDGET:usize=512*1024*1024;
/// Verified block hashes become visible to readers after this much progress of a long scan.
const PUBLISH_BYTES:u64=4*1024*1024;
const WORKERS:usize=2;
const QUEUED_SOURCES:usize=64;

/// What a record contributes to branch selection and to the few readers that classify records.
#[derive(Clone,Debug,Default)]
pub(super) struct Rec {
    pub end:u64,
    pub ordinal:Option<u64>,
    pub native:Option<Box<str>>,
    pub parent:Option<Box<str>>,
    /// The parent is Claude's `logicalParentUuid`: a compaction boundary naming what it summarized.
    pub logical:bool,
    pub event:Option<Box<str>>,
    /// Codex `payload.type`, which tells a completed item apart from its response-item copy.
    pub subtype:Option<Box<str>>,
    /// Codex `turn_context` turn identity, which rewind counts.
    pub turn:Option<Box<str>>,
    /// Claude `message.id`, shared by the records of one streamed assistant message.
    pub message:Option<Box<str>>,
    /// A Claude user record whose content is a non-empty array of tool results only.
    pub results:bool,
    /// A Claude `last-prompt` record of a rewind, and the leaf it names.
    pub rewound:bool,
    pub leaf:Option<Box<str>>,
    pub active:bool,
    pub error:Option<Box<str>>,
}

#[derive(Clone,Debug)]
pub(super) struct Row {
    pub order:(u64,u32),
    pub record_start:u64,
    pub record_end:u64,
    pub ordinal:Option<u64>,
    pub kind:Box<str>,
    pub revision:u64,
    /// The part written last; its summary is the row's summary.
    pub latest:u64,
}

#[derive(Clone,Debug)]
pub(super) struct Part {pub record_end:u64,pub ordinal:Option<u64>,pub summary:Box<str>}

#[derive(Clone,Debug)]
pub(super) struct RowField {pub reference:String,pub label:String,pub sequence:u64}

/// A field a client can address: where its original bytes are, and how they are encoded.
#[derive(Clone,Debug,PartialEq)]
pub struct FieldMeta {
    pub record_start:u64,
    pub path:String,
    /// Raw range in the source, or in the decoded parent string when `parent` is set.
    pub raw_start:u64,
    pub raw_end:u64,
    pub decoded_bytes:u64,
    pub parent:Option<Box<FieldMeta>>,
    pub encoding:String,
    /// Hash of exactly the raw range, for a string that fits in one checkpoint or a scalar.
    pub hash:Option<Vec<u8>>,
}

#[derive(Clone,Debug)]
pub(super) struct FileRow {pub path:String,pub change:Option<String>,pub detail:Option<String>,pub kind:Option<String>}

/// The original bytes of a source: a recording file, or the conversation a database holds, laid out
/// once in memory as one JSON record per line.
#[derive(Clone,Debug)]
pub(super) enum Bytes {File,Memory(Arc<Vec<u8>>)}

#[derive(Debug)]
pub(super) struct State {
    pub generation:String,
    pub identity:String,
    pub size:u64,
    pub modified:String,
    pub head_hash:Vec<u8>,
    pub indexed:u64,
    pub status:String,
    pub error:Option<String>,
    /// The size at which a scan last failed; it is not read again until the source changes.
    pub failed_at:Option<u64>,
    pub branch_revision:String,
    pub bytes:Bytes,
    /// SHA-256 of each complete [`INPUT_BYTES`] block of the file read so far, and of the partial block after them.
    pub blocks:Vec<[u8;32]>,
    pub tail_block:Option<(u64,[u8;32])>,
    /// A read found the original changed: the next registration starts a new generation.
    pub stale:bool,
    pub records:BTreeMap<u64,Rec>,
    pub identities:HashMap<Box<str>,Vec<u64>>,
    pub rows:HashMap<Arc<str>,Row>,
    pub order:BTreeMap<(u64,u32),Arc<str>>,
    pub parts:HashMap<Arc<str>,BTreeMap<u64,Part>>,
    pub record_rows:BTreeMap<u64,Vec<Arc<str>>>,
    pub row_fields:HashMap<Arc<str>,Vec<RowField>>,
    pub fields:HashMap<String,FieldMeta>,
    pub field_rows:HashMap<String,Vec<Arc<str>>>,
    pub files:HashMap<Arc<str>,BTreeMap<(u64,u64),FileRow>>,
    pub superseded:HashMap<String,String>,
    pub kv:HashMap<String,String>,
    pub branch_mark:Option<u64>,
    /// Antigravity tool calls awaiting their result step: (record start, call index) → (row, active).
    pub pending:BTreeMap<(u64,u64),(String,bool)>,
    pub append:HashSet<String>,
    pub calls:HashMap<Box<str>,Vec<Arc<str>>>,
    /// Rough size of everything above, for choosing which idle index to drop.
    pub memory:usize,
}

impl State {
    fn new(generation:String,stamp:&source::Stamp,bytes:Bytes) -> Self {
        Self {generation,identity:stamp.identity.clone(),size:stamp.size,modified:stamp.modified.clone(),head_hash:stamp.head_hash.clone(),
            indexed:0,status:"partial".into(),error:None,failed_at:None,branch_revision:String::new(),bytes,blocks:Vec::new(),tail_block:None,stale:false,
            records:BTreeMap::new(),identities:HashMap::new(),rows:HashMap::new(),order:BTreeMap::new(),parts:HashMap::new(),record_rows:BTreeMap::new(),
            row_fields:HashMap::new(),fields:HashMap::new(),field_rows:HashMap::new(),files:HashMap::new(),superseded:HashMap::new(),kv:HashMap::new(),
            branch_mark:None,pending:BTreeMap::new(),append:HashSet::new(),calls:HashMap::new(),memory:0}
    }

    pub(super) fn row_id(&self,id:&str) -> Option<Arc<str>> {self.rows.get_key_value(id).map(|(key,_)|key.clone())}
    fn interned(&self,id:&str) -> Arc<str> {self.row_id(id).unwrap_or_else(||Arc::from(id))}

    /// The summary a row last received.
    pub(super) fn summary(&self,id:&str) -> Option<&str> {
        let row=self.rows.get(id)?;self.parts.get(id)?.get(&row.latest).map(|part|&*part.summary)
    }

    /// Insert or replace a row's part from one record, and the row itself, as that record presents it.
    pub(super) fn write_row(&mut self,record:&Record,local:usize,call:Option<&str>,value:&serde_json::Value) {
        let id=value["id"].as_str().unwrap_or_default();let key=self.interned(id);
        let summary=value.to_string();self.memory+=summary.len()+128;
        let ordinal=record.value["ordinal"].as_u64();
        self.parts.entry(key.clone()).or_default().insert(record.start,Part {record_end:record.end,ordinal,summary:summary.into()});
        let rows=self.record_rows.entry(record.start).or_default();if !rows.contains(&key) {rows.push(key.clone());}
        let order=(record.start,local as u32);
        match self.rows.get_mut(id) {
            Some(row)=> {
                row.latest=record.start;row.revision+=1;
                if record.start<row.record_start {row.record_end=record.end;row.ordinal=ordinal;row.record_start=record.start;}
                if order<row.order {self.order.remove(&row.order);row.order=order;self.order.insert(order,key);}
            },
            None=> {
                self.rows.insert(key.clone(),Row {order,record_start:record.start,record_end:record.end,ordinal,kind:value["kind"].as_str().unwrap_or_default().into(),
                    revision:1,latest:record.start});
                self.order.insert(order,key.clone());
                if let Some(call)=call {self.calls.entry(call.into()).or_default().push(key);}
            },
        }
    }

    /// Place a row at a record boundary other than its first part's.
    pub(super) fn set_order(&mut self,id:&str,order:(u64,u32)) {
        let Some(key)=self.row_id(id) else {return;};let row=self.rows.get_mut(id).unwrap();
        self.order.remove(&row.order);row.order=order;self.order.insert(order,key);
    }

    /// Attach fields to a row under a label, in order after the fields it already has.
    pub(super) fn attach(&mut self,row:&str,label:&str,fields:&[(String,FieldMeta)]) {
        let key=self.interned(row);let list=self.row_fields.entry(key.clone()).or_default();
        let base=list.iter().map(|field|field.sequence+1).max().unwrap_or(0);let mut added=0;
        for (reference,meta) in fields {
            if list.iter().any(|field|&field.reference==reference) {continue;}
            list.push(RowField {reference:reference.clone(),label:label.into(),sequence:base+added});added+=1;
            self.memory+=reference.len()*2+meta.path.len()+96;
            self.fields.insert(reference.clone(),meta.clone());
            let owners=self.field_rows.entry(reference.clone()).or_default();if !owners.contains(&key) {owners.push(key.clone());}
        }
    }

    /// Drop a row's fields under one label; a complete value replaces the pieces it was appended from.
    pub(super) fn detach(&mut self,row:&str,label:&str) {
        let Some(list)=self.row_fields.get_mut(row) else {return;};
        let removed=list.iter().filter(|field|field.label==label).map(|field|field.reference.clone()).collect::<Vec<_>>();
        list.retain(|field|field.label!=label);
        for reference in removed {
            self.append.remove(&reference);
            if let Some(owners)=self.field_rows.get_mut(&reference) {
                owners.retain(|owner|&**owner!=row);
                if owners.is_empty() {self.field_rows.remove(&reference);self.fields.remove(&reference);}
            }
        }
    }

    pub(super) fn record(&mut self,start:u64,rec:Rec) {
        if let Some(native)=&rec.native {
            let starts=self.identities.entry(native.clone()).or_default();if !starts.contains(&start) {starts.push(start);}
        }
        self.memory+=96+rec.native.as_ref().map_or(0,|value|value.len()*2)+rec.parent.as_ref().map_or(0,|value|value.len());
        // A record seen again by a scan resumed from an earlier watermark keeps its branch state: only
        // the scan that reaches the leaf recomputes `active`.
        let active=self.records.get(&start).map_or(rec.active,|previous|previous.active);
        self.records.insert(start,Rec {active,..rec});
    }

    pub(super) fn active(&self,start:u64) -> bool {self.records.get(&start).is_some_and(|record|record.active)}

    /// The identity the record at `start` continues from, or `None` at a root. A recording is only
    /// appended to, so a parent is always written before its child, and following only such links
    /// can never loop. Claude breaks this when a compaction keeps messages that arrive while it runs:
    /// they are written after the boundary, and the boundary's `logicalParentUuid` names the last of
    /// them. The conversation the boundary summarized then ends at the record written just before it.
    /// Claude also names a parent it never writes: when a reply fails to produce a valid tool call,
    /// the reply is dropped but the retry prompt after it still names it. Such a record continues
    /// from the record written just before it, and is a root when nothing was written before it.
    pub(super) fn parent_of(&self,start:u64,record:&Rec) -> Option<Box<str>> {
        let parent=record.parent.as_ref()?;
        let earlier=||self.records.range(..start).rev().find_map(|(_,earlier)|earlier.native.clone());
        match self.identities.get(parent) {
            Some(starts) if starts.iter().any(|at|*at<start) =>Some(parent.clone()),
            Some(_) =>if record.logical {earlier()} else {None},
            None=>earlier(),
        }
    }
}

/// One physical source and its index. Registration replaces the state when the source changed.
pub struct Index {
    pub id:String,
    pub key:String,
    kind:SessionKind,
    /// The recording file and conversation identity, for a file source.
    file:Option<(PathBuf,String)>,
    pub(super) state:RwLock<State>,
    /// The queued or running scan's cancel flag.
    job:Mutex<Option<Arc<AtomicBool>>>,
    /// Held while a scan reads the source; a source never has two scans at once.
    scanning_gate:Mutex<()>,
    touched:Mutex<Instant>,
}
impl std::fmt::Debug for Index {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>) -> std::fmt::Result {f.debug_struct("Index").field("id",&self.id).field("key",&self.key).finish()}
}
impl Index {
    pub(super) fn read(&self) -> std::sync::RwLockReadGuard<'_,State> {self.state.read().unwrap_or_else(|poisoned|poisoned.into_inner())}
    pub(super) fn write(&self) -> std::sync::RwLockWriteGuard<'_,State> {self.state.write().unwrap_or_else(|poisoned|poisoned.into_inner())}
    fn touch(&self) {*self.touched.lock().unwrap()=Instant::now();}
    fn idle(&self) -> Duration {self.touched.lock().unwrap().elapsed()}
    pub(super) fn scanning(&self) -> bool {self.job.lock().unwrap().is_some()}
    fn cancel(&self) {if let Some(cancel)=self.job.lock().unwrap().as_ref() {cancel.store(true,Ordering::Relaxed);}}
}

#[derive(Clone)]
pub struct RegisteredSource {
    pub id:String,
    pub generation:String,
    pub segment:Segment,
    pub size:u64,
    pub indexed:u64,
    pub(super) index:Arc<Index>,
}
impl std::fmt::Debug for RegisteredSource {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredSource").field("id",&self.id).field("generation",&self.generation).field("segment",&self.segment).field("size",&self.size).finish()
    }
}
impl RegisteredSource {
    pub(super) fn virtual_bytes(&self) -> Option<Arc<Vec<u8>>> {match &self.index.read().bytes {Bytes::Memory(bytes)=>Some(bytes.clone()),Bytes::File=>None}}
    pub(super) fn is_virtual(&self) -> bool {matches!(self.index.read().bytes,Bytes::Memory(_))}
    /// The state of this generation, or `source_changed` when the source was registered again since.
    pub(super) fn current(&self) -> Result<std::sync::RwLockReadGuard<'_,State>,String> {
        let state=self.index.read();
        if state.generation!=self.generation || state.stale {return Err("source_changed: detail belongs to an expired history generation".into());}
        Ok(state)
    }
    /// How far this generation is indexed, and its status.
    pub fn progress(&self) -> Result<(u64,String),String> {let state=self.current()?;Ok((state.indexed,state.status.clone()))}
}

struct Registry {indexes:HashMap<String,Arc<Index>>}
static REGISTRY:OnceLock<Mutex<Registry>>=OnceLock::new();
fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| {
        let _=std::thread::Builder::new().name("history-janitor".into()).spawn(||loop {
            std::thread::park_timeout(Duration::from_secs(30));
            evict_idle();
        });
        Mutex::new(Registry {indexes:HashMap::new()})
    })
}

/// Drop indexes nobody holds once they have been idle long enough, or sooner when idle indexes
/// together exceed [`IDLE_BUDGET`].
fn evict_idle() {
    let mut registry=registry().lock().unwrap();
    registry.indexes.retain(|_,index|Arc::strong_count(index)>1 || index.scanning() || index.idle()<IDLE);
    let mut idle=registry.indexes.iter().filter(|(_,index)|Arc::strong_count(index)==1 && !index.scanning())
        .map(|(key,index)|(index.idle(),index.read().memory,key.clone())).collect::<Vec<_>>();
    let mut total=idle.iter().map(|(_,bytes,_)|*bytes).sum::<usize>();
    idle.sort_by(|a,b|b.0.cmp(&a.0));
    for (_,bytes,key) in idle {if total<=IDLE_BUDGET {break;}registry.indexes.remove(&key);total-=bytes;}
}

/// Readers that keep a source's scan going: a reader canceling its own interest stops the scan only
/// when no other reader still waits on it. A reader also keeps its indexes in memory.
struct Reader {scope:String,sources:Vec<Arc<Index>>,touched:Instant}
static READERS:OnceLock<Mutex<HashMap<String,Reader>>>=OnceLock::new();
fn readers() -> &'static Mutex<HashMap<String,Reader>> {READERS.get_or_init(||Mutex::new(HashMap::new()))}

/// A handle on the in-memory history of one data directory. Every handle shares the same indexes.
pub struct Store {pub data_dir:PathBuf,pub(super) cursor_key:String}

impl Store {
    pub fn at(data_dir:&Path) -> Result<Arc<Self>,String> {
        // Cursors are signed with a key of this process; a restart expires them, as it drops the index.
        static KEY:OnceLock<String>=OnceLock::new();
        let key=KEY.get_or_init(||uuid::Uuid::new_v4().to_string()).clone();
        Ok(Arc::new(Self {data_dir:data_dir.to_owned(),cursor_key:key}))
    }

    /// The index registered under `key`, created when this process has not opened the source yet.
    fn index(&self,kind:SessionKind,key:&str,file:Option<(&Path,&str)>) -> Arc<Index> {
        let mut registry=registry().lock().unwrap();
        let index=registry.indexes.entry(format!("{}\n{key}",self.data_dir.display())).or_insert_with(|| Arc::new(Index {
            id:digest(format!("{}:{key}",kind.as_str()).as_bytes())[..32].to_owned(),key:key.into(),kind,
            file:file.map(|(path,native)|(path.to_owned(),native.to_owned())),
            state:RwLock::new(State::new(String::new(),&source::Stamp {identity:String::new(),modified:String::new(),size:0,head_hash:Vec::new()},Bytes::File)),
            job:Mutex::new(None),scanning_gate:Mutex::new(()),touched:Mutex::new(Instant::now())})).clone();
        index.touch();index
    }

    pub fn register(&self,kind:SessionKind,segment:Segment) -> Result<RegisteredSource,String> {
        let mut file=File::open(&segment.path).map_err(|e|format!("History source is unavailable: {e}"))?;
        let stamp=source::stamp(&mut file)?;
        // Rows are named after the conversation identity, so another identity never shares the index.
        let index=self.index(kind,&format!("{}\n{}",segment.path.to_string_lossy(),segment.native_id),Some((&segment.path,&segment.native_id)));
        let (generation,indexed)={
            let mut state=index.write();
            let same=!state.generation.is_empty() && !state.stale && state.identity==stamp.identity && stamp.size>=state.size
                && (stamp.size>state.size || state.modified==stamp.modified) && {
                    // The bytes that identified the file before must still be its first bytes.
                    let mut head=vec![0;state.size.min(64*1024) as usize];
                    file.seek(SeekFrom::Start(0)).is_ok() && file.read_exact(&mut head).is_ok() && Sha256::digest(&head).as_slice()==state.head_hash
                };
            if !same {
                index.cancel();
                *state=State::new(uuid::Uuid::new_v4().to_string(),&stamp,Bytes::File);
            } else if stamp.size>state.size {
                state.size=stamp.size;state.modified=stamp.modified.clone();
                if state.status=="complete" {state.status="partial".into();}
            }
            (state.generation.clone(),state.indexed)
        };
        Ok(RegisteredSource {id:index.id.clone(),generation,segment,size:stamp.size,indexed,index})
    }

    /// Register a source whose records are built in memory from a database. `fingerprint` is cheap to
    /// compute and changes whenever the database might; `build` reads the conversation into one JSON
    /// record per line and runs only when the fingerprint changed. The generation stays while the built
    /// records only grew.
    pub(super) fn register_virtual(&self,kind:SessionKind,key:&str,path:&Path,native:&str,fingerprint:String,
        build:impl FnOnce()->Result<Vec<u8>,String>) -> Result<RegisteredSource,String> {
        let index=self.index(kind,key,None);
        let segment=Segment {path:path.into(),native_id:native.into(),end_byte:None,end_ordinal:None};
        {
            let state=index.read();
            if !state.generation.is_empty() && !state.stale && state.modified==fingerprint {
                return Ok(RegisteredSource {id:index.id.clone(),generation:state.generation.clone(),segment,size:state.size,indexed:state.indexed,index:index.clone()});
            }
        }
        let bytes=Arc::new(build()?);
        let stamp=source::Stamp {identity:"memory".into(),modified:fingerprint,size:bytes.len() as u64,head_hash:Sha256::digest(&bytes[..bytes.len().min(64*1024)]).to_vec()};
        let (generation,indexed)={
            let mut state=index.write();
            let previous=match &state.bytes {Bytes::Memory(old)=>Some(old.clone()),Bytes::File=>None};
            let grown=!state.generation.is_empty() && !state.stale && previous.is_some_and(|old|bytes.len()>=old.len() && bytes[..old.len()]==old[..]);
            if grown {
                if bytes.len() as u64>state.size && state.status=="complete" {state.status="partial".into();}
                state.size=stamp.size;state.modified=stamp.modified.clone();state.bytes=Bytes::Memory(bytes);
            } else {
                index.cancel();
                *state=State::new(uuid::Uuid::new_v4().to_string(),&stamp,Bytes::Memory(bytes));
            }
            (state.generation.clone(),state.indexed)
        };
        Ok(RegisteredSource {id:index.id.clone(),generation,segment,size:stamp.size,indexed,index})
    }

    /// Queue a scan of the part of `source` not indexed yet. Two scans never read one source at once.
    pub fn schedule(self:&Arc<Self>,kind:SessionKind,source:RegisteredSource) {
        {
            let state=source.index.read();
            if state.generation!=source.generation || state.stale || state.failed_at==Some(state.size) {return;}
            let limit=source.segment.end_byte.unwrap_or(state.size).min(state.size);
            if state.indexed>=limit && state.status!="partial" {return;}
            if state.indexed>=limit && source.segment.end_byte.is_some() {return;}
        }
        let cancel={
            let mut job=source.index.job.lock().unwrap();
            if job.is_some() {return;}
            let cancel=Arc::new(AtomicBool::new(false));*job=Some(cancel.clone());cancel
        };
        if !queue(self.clone(),kind,source.clone(),cancel) {*source.index.job.lock().unwrap()=None;}
    }

    /// Read `source` from where its index ends, or only its last [`TAIL_BYTES`] when `tail` is set, so
    /// the newest rows of a long recording can be shown before the whole recording has been read.
    pub fn scan(&self,kind:SessionKind,source:&RegisteredSource,cancel:&AtomicBool,tail:bool) -> Result<(),String> {
        let _work=super::work::Work::acquire(cancel)?;
        let _gate=source.index.scanning_gate.lock().unwrap_or_else(|poisoned|poisoned.into_inner());
        if source.is_virtual() {return scan_memory(kind,source,cancel);}
        let mut file=File::open(&source.segment.path).map_err(|e|format!("History source is unavailable: {e}"))?;
        let stamp=source::stamp(&mut file)?;
        let (limit,mut start)={
            let state=source.current()?;
            if state.identity!=stamp.identity {return Err("source_changed: history source was replaced before scanning".into());}
            if stamp.size<state.size {return Err("source_changed: history source was truncated".into());}
            let limit=source.segment.end_byte.unwrap_or(state.size).min(state.size);
            (limit,if tail {limit.saturating_sub(TAIL_BYTES)} else {state.indexed.min(limit)})
        };
        if tail && start>0 {
            // Discard only the first partial physical line, without retaining its bytes.
            file.seek(SeekFrom::Start(start)).map_err(|e|e.to_string())?;
            let mut bytes=vec![0;INPUT_BYTES];
            while start<limit {
                let count=file.read(&mut bytes[..(limit-start).min(INPUT_BYTES as u64) as usize]).map_err(|e|e.to_string())?;
                if count==0 {break;}
                if let Some(at)=bytes[..count].iter().position(|byte|*byte==b'\n') {start+=at as u64+1;break;}
                start+=count as u64;
            }
        }
        // Blocks are hashed as the scan passes them; a resumed scan first hashes its partial block again.
        let hasher=if tail {None} else {Some(std::rc::Rc::new(std::cell::RefCell::new(BlockHasher::resume(&source.index,&mut file,start)?)))};
        file.seek(SeekFrom::Start(start)).map_err(|e|e.to_string())?;
        let validation=file.try_clone().map_err(|e|e.to_string())?;
        let reader=Hashing {file:&mut file,hasher:hasher.clone(),cancel};
        let mut parser=Projector::new(reader.take(limit-start),start).long_checkpoints_only();
        let mut gaps=false;let mut checkpoints=Vec::new();let mut published=start;
        loop {
            if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
            checkpoints.clear();
            // Parsing reads the file without holding the index, so readers are never kept waiting by a long record.
            let record=parser.next(&mut |checkpoint| {
                if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
                if super::adapter::addressed(&checkpoint.path) {checkpoints.push(checkpoint);}Ok(())
            })?;
            let Some(mut record)=record else {break;};
            if !record.complete {break;}
            if source.segment.end_ordinal.is_some_and(|end|record.value["ordinal"].as_u64().is_some_and(|ordinal|ordinal>=end)) {break;}
            source::opened_source_unchanged(&validation,&source.segment.path,&stamp)?;
            gaps|=record.error.is_some();
            let scanned=super::adapter::Scanned {source,checkpoints:&checkpoints,bytes:None};
            let mut refs=super::adapter::refs(source,&record);
            if record.error.is_none() {super::adapter::expand_arguments(&scanned,&mut record,&mut refs)?;}
            let mut state=source.index.write();
            if state.generation!=source.generation {return Err("source_changed: history generation changed".into());}
            super::adapter::store_record(&mut state,kind,&scanned,&record,&refs)?;
            if !tail {state.indexed=record.end;}
            if let Some(hasher)=hasher.as_ref().filter(|_|record.end-published>=PUBLISH_BYTES) {
                // Only blocks entirely before what was parsed are final.
                hasher.borrow().publish_complete(&mut state);published=record.end;
            }
        }
        // The parser may have read the start of a line still being written; hash only what is final.
        drop(parser);
        source::opened_source_unchanged(&file,&source.segment.path,&stamp)?;
        let mut state=source.index.write();
        if state.generation!=source.generation {return Err("source_changed: history generation changed".into());}
        if let Some(hasher)=&hasher {hasher.borrow().publish(&mut state);}
        if !tail {
            let errors=gaps || state.records.values().any(|record|record.error.is_some());
            let status=if errors {"error"} else if state.indexed>=state.size {"complete"} else {"partial"};
            state.status=status.into();
            super::adapter::select_branch(&mut state,kind,source)?;
            if status=="complete" {state.branch_mark=Some(state.indexed);}
        }
        Ok(())
    }

    pub fn subscribe(&self,scope:&str,sources:&[RegisteredSource],existing:Option<&str>) -> Result<String,String> {
        let indexes=sources.iter().map(|source|source.index.clone()).collect::<Vec<_>>();
        let mut readers=readers().lock().unwrap();
        readers.retain(|_,reader|reader.touched.elapsed()<IDLE);
        if let Some(id)=existing {
            let reader=readers.get_mut(id).ok_or("cursor_expired: index reader expired")?;
            if reader.scope!=scope {return Err("cursor_expired: index reader belongs to another session".into());}
            reader.sources=indexes;reader.touched=Instant::now();return Ok(id.to_owned());
        }
        if readers.len()>=128 {return Err("index_busy: too many active history readers".into());}
        let id=format!("index:{}",uuid::Uuid::new_v4());
        readers.insert(id.clone(),Reader {scope:scope.into(),sources:indexes,touched:Instant::now()});Ok(id)
    }

    pub fn cancel_reader(&self,scope:&str,id:&str) -> Result<bool,String> {cancel_one(scope,id)}
}

fn cancel_one(scope:&str,id:&str) -> Result<bool,String> {
    let mut readers=readers().lock().unwrap();
    let Some(reader)=readers.get(id) else {return Ok(false);};
    if reader.scope!=scope {return Err("cursor_expired: index reader belongs to another session".into());}
    let reader=readers.remove(id).unwrap();
    readers.retain(|_,reader|reader.touched.elapsed()<IDLE);
    for index in reader.sources {
        if !readers.values().any(|other|other.sources.iter().any(|source|Arc::ptr_eq(source,&index))) {index.cancel();}
    }
    Ok(true)
}

pub fn cancel_reader(scope:&str,id:&str) -> Result<(),String> {
    // Cancellation is idempotent after expiry or completion, including transport retries.
    cancel_one(scope,id).map(|_|())
}

/// Scan a source built in memory from a database: its records are complete and its bytes immutable.
fn scan_memory(kind:SessionKind,source:&RegisteredSource,cancel:&AtomicBool) -> Result<(),String> {
    let bytes=source.virtual_bytes().ok_or("unsupported_record: database history bytes are unavailable")?;
    let start=source.current()?.indexed.min(bytes.len() as u64);
    let mut parser=Projector::new(&bytes[start as usize..],start).long_checkpoints_only();
    let mut checkpoints=Vec::new();let mut gaps=false;
    loop {
        if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
        checkpoints.clear();
        let Some(mut record)=parser.next(&mut |checkpoint| {checkpoints.push(checkpoint);Ok(())})? else {break;};
        if !record.complete {break;}
        gaps|=record.error.is_some();
        let scanned=super::adapter::Scanned {source,checkpoints:&checkpoints,bytes:Some(&bytes)};
        let mut refs=super::adapter::refs(source,&record);
        if record.error.is_none() {super::adapter::expand_arguments(&scanned,&mut record,&mut refs)?;}
        let mut state=source.index.write();
        if state.generation!=source.generation {return Err("source_changed: history generation changed".into());}
        super::adapter::store_record(&mut state,kind,&scanned,&record,&refs)?;
        state.indexed=record.end;
    }
    let mut state=source.index.write();
    if state.generation!=source.generation {return Err("source_changed: history generation changed".into());}
    let errors=gaps || state.records.values().any(|record|record.error.is_some());
    state.status=if errors {"error"} else if state.indexed>=state.size {"complete"} else {"partial"}.into();
    super::adapter::select_branch(&mut state,kind,source)?;
    if state.status=="complete" {state.branch_mark=Some(state.indexed);}
    Ok(())
}

/// Hashes [`INPUT_BYTES`] blocks of the file in order as the scan reads it.
struct BlockHasher {blocks:Vec<[u8;32]>,from:usize,current:Sha256,filled:u64}
impl BlockHasher {
    /// Continue from the last complete block before `start`, hashing the bytes between it and `start` again.
    fn resume(index:&Index,file:&mut File,start:u64) -> Result<Self,String> {
        let complete=(index.read().blocks.len() as u64).min(start/INPUT_BYTES as u64);
        let mut hasher=Self {blocks:Vec::new(),from:complete as usize,current:Sha256::new(),filled:0};
        let from=complete*INPUT_BYTES as u64;
        file.seek(SeekFrom::Start(from)).map_err(|e|e.to_string())?;
        let mut bytes=vec![0;(start-from) as usize];file.read_exact(&mut bytes).map_err(|_|"source_changed: history source was truncated")?;
        hasher.update(&bytes);Ok(hasher)
    }
    fn update(&mut self,mut bytes:&[u8]) {
        while !bytes.is_empty() {
            let take=((INPUT_BYTES as u64-self.filled) as usize).min(bytes.len());
            self.current.update(&bytes[..take]);self.filled+=take as u64;bytes=&bytes[take..];
            if self.filled==INPUT_BYTES as u64 {
                self.blocks.push(std::mem::replace(&mut self.current,Sha256::new()).finalize().into());self.filled=0;
            }
        }
    }
    fn publish_complete(&self,state:&mut State) {state.blocks.truncate(self.from);state.blocks.extend_from_slice(&self.blocks);}
    fn publish(&self,state:&mut State) {
        self.publish_complete(state);
        state.tail_block=(self.filled>0).then(||(self.filled,self.current.clone().finalize().into()));
    }
}

struct Hashing<'a> {file:&'a mut File,hasher:Option<std::rc::Rc<std::cell::RefCell<BlockHasher>>>,cancel:&'a AtomicBool}
impl Read for Hashing<'_> {
    fn read(&mut self,bytes:&mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {return Err(std::io::Error::other("read_canceled"));}
        let count=self.file.read(bytes)?;
        if let Some(hasher)=&self.hasher {hasher.borrow_mut().update(&bytes[..count]);}
        Ok(count)
    }
}

/// Mark a source changed: its readers fail and the next registration reads it again.
pub(super) fn changed(source:&RegisteredSource) -> String {
    let mut state=source.index.write();if state.generation==source.generation {state.stale=true;}
    "source_changed: history content changed".into()
}

/// Bytes `[start, end)` of the original, checked against the hashes taken while scanning. Bytes a
/// running scan has not hashed yet are read as they are: the recording only grows past them.
pub(super) fn verified_range(source:&RegisteredSource,start:u64,end:u64) -> Result<Vec<u8>,String> {
    if end<start || end-start>2*INPUT_BYTES as u64 {return Err("History range exceeds its byte budget".into());}
    if let Some(bytes)=source.virtual_bytes() {
        drop(source.current()?);
        if end>bytes.len() as u64 {return Err(changed(source));}
        return Ok(bytes[start as usize..end as usize].to_vec());
    }
    let first=(start/INPUT_BYTES as u64) as usize;let last=(end.saturating_sub(1)/INPUT_BYTES as u64) as usize;
    let (identity,hashes)={
        let state=source.current()?;
        let hashes=(first..=last).map(|block|match state.blocks.get(block) {
            Some(hash)=>Some((INPUT_BYTES as u64,*hash)),
            None if block==state.blocks.len()=>state.tail_block,
            None=>None,
        }).collect::<Vec<_>>();
        (state.identity.clone(),hashes)
    };
    let mut file=File::open(&source.segment.path).map_err(|_|"detail_unavailable: history source is unavailable")?;
    if source::identity(&file.metadata().map_err(|e|e.to_string())?)!=identity {return Err(changed(source));}
    let mut result=Vec::with_capacity((end-start) as usize);
    for (offset,hash) in hashes.into_iter().enumerate() {
        let from=(first+offset) as u64*INPUT_BYTES as u64;
        let (length,expected)=match hash {
            Some((length,expected))=>(length,Some(expected)),
            None=>(INPUT_BYTES as u64,None),
        };
        let block_end=from+length;
        let (want_from,want_to)=(start.max(from),end.min(from+INPUT_BYTES as u64));
        if let Some(expected)=expected.filter(|_|want_to<=block_end) {
            let key=(source.id.clone(),source.generation.clone(),first+offset,expected);
            let bytes=match verified_block(&key,&file)? {
                Some(bytes)=>bytes,
                None=> {
                    file.seek(SeekFrom::Start(from)).map_err(|e|e.to_string())?;
                    let mut bytes=vec![0;length as usize];
                    file.read_exact(&mut bytes).map_err(|_|changed(source))?;
                    if Sha256::digest(&bytes).as_slice()!=expected {return Err(changed(source));}
                    remember_block(key,&file,bytes)?
                },
            };
            result.extend_from_slice(&bytes[(want_from-from) as usize..(want_to-from) as usize]);
        } else {
            file.seek(SeekFrom::Start(want_from)).map_err(|e|e.to_string())?;
            let mut bytes=vec![0;(want_to-want_from) as usize];
            file.read_exact(&mut bytes).map_err(|_|changed(source))?;result.extend_from_slice(&bytes);
        }
    }
    if source::identity(&std::fs::metadata(&source.segment.path).map_err(|_|changed(source))?)!=identity {return Err(changed(source));}
    Ok(result)
}

/// Bytes `[start, end)` of the original as they are now; the caller checks them against a hash of
/// exactly this range.
pub(super) fn original_range(source:&RegisteredSource,start:u64,end:u64) -> Result<Vec<u8>,String> {
    if end<start || end-start>2*INPUT_BYTES as u64 {return Err("History range exceeds its byte budget".into());}
    if let Some(bytes)=source.virtual_bytes() {
        drop(source.current()?);
        return bytes.get(start as usize..end as usize).map(<[u8]>::to_vec).ok_or_else(||changed(source));
    }
    let identity=source.current()?.identity.clone();
    let mut file=File::open(&source.segment.path).map_err(|_|"detail_unavailable: history source is unavailable")?;
    if source::identity(&file.metadata().map_err(|e|e.to_string())?)!=identity {return Err(changed(source));}
    file.seek(SeekFrom::Start(start)).map_err(|e|e.to_string())?;
    let mut bytes=vec![0;(end-start) as usize];file.read_exact(&mut bytes).map_err(|_|changed(source))?;
    Ok(bytes)
}

/// Blocks verified recently, with the file's size and modification time when they were read. Reading
/// the records of one block one after another would otherwise hash the same block for each of them;
/// any write to the file changes its stamp and sends the next read back to the disk and the hash.
type BlockKey=(String,String,usize,[u8;32]);
static BLOCKS:OnceLock<Mutex<std::collections::VecDeque<(BlockKey,(u64,Option<std::time::SystemTime>),Arc<Vec<u8>>)>>>=OnceLock::new();
const CACHED_BLOCKS:usize=16;
fn file_stamp(file:&File) -> Result<(u64,Option<std::time::SystemTime>),String> {
    let metadata=file.metadata().map_err(|e|e.to_string())?;Ok((metadata.len(),metadata.modified().ok()))
}
fn verified_block(key:&BlockKey,file:&File) -> Result<Option<Arc<Vec<u8>>>,String> {
    let stamp=file_stamp(file)?;
    let blocks=BLOCKS.get_or_init(Default::default).lock().unwrap();
    Ok(blocks.iter().find(|(candidate,at,_)|candidate==key && *at==stamp && stamp.1.is_some()).map(|(_,_,bytes)|bytes.clone()))
}
fn remember_block(key:BlockKey,file:&File,bytes:Vec<u8>) -> Result<Arc<Vec<u8>>,String> {
    let stamp=file_stamp(file)?;let bytes=Arc::new(bytes);
    let mut blocks=BLOCKS.get_or_init(Default::default).lock().unwrap();
    blocks.retain(|(candidate,_,_)|candidate!=&key);
    if blocks.len()>=CACHED_BLOCKS {blocks.pop_front();}
    blocks.push_back((key,stamp,bytes.clone()));Ok(bytes)
}

/// Read and parse one record of `source` again, verifying its bytes. `sink` receives its checkpoints.
pub(super) fn reproject(source:&RegisteredSource,start:u64,sink:&mut impl FnMut(Checkpoint)->Result<(),String>) -> Result<Record,String> {
    let end=source.current()?.records.get(&start).map(|record|record.end).ok_or("index_incomplete: history record is unavailable")?;
    let reader=RecordReader {source,at:start,end,buffer:Vec::new(),offset:0};
    let mut parser=Projector::new(reader,start).long_checkpoints_only();
    let mut record=parser.next(sink)?.ok_or_else(||changed(source))?;
    if record.end!=end && !(record.end+1==end && !record.complete) {return Err(changed(source));}
    record.end=end;record.complete=true;
    Ok(record)
}

/// Verified bytes of a record, a block at a time.
struct RecordReader<'a> {source:&'a RegisteredSource,at:u64,end:u64,buffer:Vec<u8>,offset:usize}
impl Read for RecordReader<'_> {
    fn read(&mut self,output:&mut [u8]) -> std::io::Result<usize> {
        if self.offset==self.buffer.len() {
            if self.at>=self.end {return Ok(0);}
            let next=((self.at/INPUT_BYTES as u64+1)*INPUT_BYTES as u64).min(self.end);
            self.buffer=verified_range(self.source,self.at,next).map_err(std::io::Error::other)?;self.offset=0;self.at=next;
        }
        let count=output.len().min(self.buffer.len()-self.offset);
        output[..count].copy_from_slice(&self.buffer[self.offset..self.offset+count]);self.offset+=count;Ok(count)
    }
}

/// Checkpoints of long strings, by source generation, record and path. Reading a long detail chunk
/// by chunk would otherwise parse its whole record again for every chunk.
type PieceKey=(String,String,u64,String);
type Pieces=HashMap<PieceKey,(Arc<Vec<Checkpoint>>,Instant)>;
static PIECES:OnceLock<Mutex<Pieces>>=OnceLock::new();
pub(super) fn cached_pieces(key:PieceKey,compute:impl FnOnce()->Result<Vec<Checkpoint>,String>) -> Result<Arc<Vec<Checkpoint>>,String> {
    let cache=PIECES.get_or_init(||Mutex::new(HashMap::new()));
    if let Some((pieces,touched))=cache.lock().unwrap().get_mut(&key) {*touched=Instant::now();return Ok(pieces.clone());}
    let pieces=Arc::new(compute()?);
    let mut cache=cache.lock().unwrap();
    if cache.len()>=64 {
        if let Some(oldest)=cache.iter().min_by_key(|(_,(_,touched))|*touched).map(|(key,_)|key.clone()) {cache.remove(&oldest);}
    }
    cache.insert(key,(pieces.clone(),Instant::now()));Ok(pieces)
}

struct Job {store:Arc<Store>,kind:SessionKind,source:RegisteredSource,cancel:Arc<AtomicBool>}
struct Queue {jobs:Mutex<std::collections::VecDeque<Job>>,ready:std::sync::Condvar}
static QUEUE:OnceLock<Arc<Queue>>=OnceLock::new();

/// A fixed worker pool keeps queued source scans from creating one operating-system thread per file.
fn queue(store:Arc<Store>,kind:SessionKind,source:RegisteredSource,cancel:Arc<AtomicBool>) -> bool {
    let queue=QUEUE.get_or_init(|| {
        let queue=Arc::new(Queue {jobs:Mutex::new(Default::default()),ready:std::sync::Condvar::new()});
        for worker in 0..WORKERS {
            let queue=queue.clone();
            let _=std::thread::Builder::new().name(format!("history-scan-{worker}")).spawn(move ||loop {
                let job={
                    let mut jobs=queue.jobs.lock().unwrap();
                    while jobs.is_empty() {jobs=queue.ready.wait(jobs).unwrap();}
                    jobs.pop_front().unwrap()
                };
                if !job.cancel.load(Ordering::Relaxed) {
                    if let Err(error)=job.store.scan(job.kind,&job.source,&job.cancel,false) {
                        if error!="read_canceled" && !error.ends_with("read_canceled") {
                            let mut state=job.source.index.write();
                            if state.generation==job.source.generation {
                                // A changed source is read again by its next registration; any other failure
                                // stays until the source changes, so it is not retried in a loop.
                                if error.starts_with("source_changed:") {state.stale=true;}
                                else {state.status="error".into();state.error=Some(error);state.failed_at=Some(state.size);}
                            }
                            // Provider bodies, paths and projections never enter diagnostics.
                            crate::diagnostic_warn!("history index failed source={} generation={}",job.source.id,job.source.generation);
                        }
                    }
                }
                *job.source.index.job.lock().unwrap()=None;
            });
        }
        queue
    });
    let mut jobs=queue.jobs.lock().unwrap();
    if jobs.len()>=QUEUED_SOURCES {return false;}
    jobs.push_back(Job {store,kind,source,cancel});
    queue.ready.notify_one();true
}

/// The recording file this process already read for a conversation, when its provider's own lookup
/// no longer finds it. Only an index held in memory answers; nothing is read or registered.
pub fn known_path(data_dir:&Path,kind:SessionKind,native:&str) -> Option<PathBuf> {
    let prefix=format!("{}\n",data_dir.display());
    registry().lock().unwrap().indexes.iter().filter(|(key,_)|key.starts_with(&prefix)).find_map(|(_,index)| {
        index.file.as_ref().filter(|(_,id)|index.kind==kind && id==native).map(|(path,_)|path.clone())
    })
}

/// Remove what earlier versions persisted: the derived history index and the protocol spool. The
/// providers' own recordings hold everything either was built from.
pub fn retire_persisted(data_dir:&Path) {
    // Summary scratch files of tasks an earlier process did not finish are removed with them.
    for name in ["chat-index","chat-spool","scratch"] {
        let path=data_dir.join(name);
        if std::fs::symlink_metadata(&path).is_ok_and(|metadata|metadata.is_dir()) {
            match std::fs::remove_dir_all(&path) {
                Ok(())=>{crate::diagnostics::record("INFO","history_retired",serde_json::json!({"directory":name}));},
                Err(_)=>{crate::diagnostics::record("WARN","history_retired",serde_json::json!({"directory":name,"status":"failed"}));},
            }
        }
    }
}

/// Wait for scans of a test's directory to stop and forget its indexes.
#[cfg(test)]
pub(crate) fn release_test_directory(directory:&Path) {
    let prefix=format!("{}\n",directory.display());
    let indexes=registry().lock().unwrap().indexes.iter().filter(|(key,_)|key.starts_with(&prefix)).map(|(_,index)|index.clone()).collect::<Vec<_>>();
    for index in &indexes {index.cancel();}
    let deadline=Instant::now()+Duration::from_secs(10);
    for index in &indexes {
        while index.scanning() {
            assert!(Instant::now()<deadline,"The test's owned index workers did not release their directory");
            std::thread::park_timeout(Duration::from_millis(10));
        }
    }
    registry().lock().unwrap().indexes.retain(|key,_|!key.starts_with(&prefix));
}

/// How many sources of a test's directory have an index.
#[cfg(test)]
pub(crate) fn registered(directory:&Path) -> usize {
    let prefix=format!("{}\n",directory.display());
    registry().lock().unwrap().indexes.keys().filter(|key|key.starts_with(&prefix)).count()
}
