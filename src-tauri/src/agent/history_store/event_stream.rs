//! Fixed-watermark event iteration for full-content tasks. Bodies remain verified field streams.

use std::collections::{BTreeSet,HashMap,HashSet,VecDeque};
use std::io::Write;
use std::sync::atomic::{AtomicBool,Ordering};

use serde_json::Value;

use crate::{agent::export::{self,Event},host::AppCtx,models::SessionKind};
use super::{adapter::Refs,api::{self,Branch},projection::Record,store::{FieldMeta,RegisteredSource,State}};

pub fn complete(app:&AppCtx,session:&str,cancel:&AtomicBool) -> Result<Branch,String> {
    wait(api::prepare(app,session)?,cancel)
}

/// Wait until every source of `branch` is indexed up to its size at the time of the call. An append
/// never silently extends an export.
pub fn wait(mut branch:Branch,cancel:&AtomicBool) -> Result<Branch,String> {
    let session=branch.scope.clone();
    if let Some(error)=&branch.error {return Err(error.clone());}
    for source in &mut branch.sources {source.segment.end_byte=Some(source.segment.end_byte.unwrap_or(source.size));}
    let mut reader=branch.store.subscribe(&session,&branch.sources,None)?;
    let mut observed=HashSet::new();
    let mut refreshed=std::time::Instant::now();
    let result=(|| {
        loop {
            if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
            if refreshed.elapsed()>=std::time::Duration::from_secs(60) {
                reader=branch.store.subscribe(&session,&branch.sources,Some(&reader))?;refreshed=std::time::Instant::now();
            }
            let mut done=true;
            for index in 0..branch.sources.len() {
                let source=branch.sources[index].clone();
                let (water,status,error,scanning)={
                    let state=source.index.read();
                    if state.generation!=source.generation || state.stale {return Err("source_changed: history generation changed while preparing the export".into());}
                    (state.indexed,state.status.clone(),state.error.clone(),source.index.scanning())
                };
                if status=="error" {return Err(error.unwrap_or_else(||"index_incomplete: conversation contains unreadable records".into()));}
                if water<source.segment.end_byte.unwrap() {
                    done=false;
                    if scanning {observed.insert(source.id.clone());continue;}
                    if observed.contains(&source.id) {
                        // A durable watermark excludes the unfinished tail and stops at a native ordinal
                        // cutoff. Neither is a malformed middle-record gap.
                        if source.segment.end_ordinal.is_none() {
                            return Err("index_incomplete: conversation ends with an unfinished record; retry after it has been saved".into());
                        }
                        branch.sources[index].segment.end_byte=Some(water);continue;
                    }
                    branch.store.schedule(branch.kind,source.clone());
                    if source.index.scanning() {observed.insert(source.id.clone());}
                } else if scanning {
                    // The watermark reaches the end before the scan selects the active branch.
                    done=false;
                }
            }
            if done {return Ok(branch.clone());}
            std::thread::park_timeout(std::time::Duration::from_millis(50));
        }
    })();
    branch.store.cancel_reader(&session,&reader)?;result
}

#[derive(Clone)]
pub enum Piece {Literal(String),Field(RegisteredSource,FieldMeta),NativeUser(RegisteredSource,String),VerifiedLiteral(RegisteredSource,Vec<FieldMeta>,String)}
#[derive(Clone)]
pub struct Text {pub pieces:Vec<Piece>,pub quote_json:bool,pub separator:&'static str}
impl Text {
    pub(crate) fn literal(value:String) -> Self {Self {pieces:vec![Piece::Literal(value)],quote_json:false,separator:"\n"}}

    pub fn walk(&self,_branch:&Branch,cancel:&AtomicBool,sink:&mut impl FnMut(&str)->Result<(),String>) -> Result<(),String> {
        if self.quote_json {sink("\"")?;}
        for (index,piece) in self.pieces.iter().enumerate() {
            if index>0 {sink(self.separator)?;}
            let mut emit=|text:&str| {
                if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
                if self.quote_json {let encoded=serde_json::to_string(text).map_err(|e|e.to_string())?;sink(&encoded[1..encoded.len()-1])} else {sink(text)}
            };
            match piece {
                Piece::Literal(text)=>emit(text)?,
                Piece::NativeUser(source,message)=>super::opencode::walk_user(source,message,cancel,&mut emit)?,
                Piece::VerifiedLiteral(source,fields,text)=> {
                    // Short transformed text is cached, but the bytes it came from are still verified.
                    for field in fields {super::fields::walk(source,field,cancel,&mut |_|Ok(()))?;}
                    emit(text)?;
                },
                Piece::Field(source,field)=>super::fields::walk(source,field,cancel,&mut emit)?,
            }
        }
        if self.quote_json {sink("\"")?;}Ok(())
    }

    pub fn write(&self,branch:&Branch,cancel:&AtomicBool,writer:&mut impl Write,trim:bool) -> Result<(),String> {
        let mut begin=0u64;let mut end=u64::MAX;
        if trim {
            let mut offset=0u64;let mut first=None;let mut last=0;
            self.walk(branch,cancel,&mut |text| {
                for (at,ch) in text.char_indices() {if !ch.is_whitespace() {first.get_or_insert(offset+at as u64);last=offset+(at+ch.len_utf8()) as u64;}}
                offset+=text.len() as u64;Ok(())
            })?;
            begin=first.unwrap_or(0);end=last;
        }
        self.write_range(branch,cancel,writer,begin,end)
    }

    pub fn write_range(&self,branch:&Branch,cancel:&AtomicBool,writer:&mut impl Write,begin:u64,end:u64) -> Result<(),String> {
        let mut offset=0u64;
        self.walk(branch,cancel,&mut |text| {
            let from=begin.saturating_sub(offset).min(text.len() as u64) as usize;
            let to=end.saturating_sub(offset).min(text.len() as u64) as usize;
            if to>from {writer.write_all(&text.as_bytes()[from..to]).map_err(|e|e.to_string())?;}
            offset+=text.len() as u64;Ok(())
        })
    }
}

pub struct StreamEvent {pub event:Event,pub body:Text,pub(crate) row_id:Option<String>}
pub(crate) struct IndexedRecord {pub source:RegisteredSource,pub record:Record,pub references:Refs}

/// Read a record again with the fields the scan gave it, nested argument documents included.
pub(crate) fn read_record(source:&RegisteredSource,start:u64) -> Result<(Record,Refs),String> {
    let mut checkpoints=Vec::new();
    let mut record=super::store::reproject(source,start,&mut |piece| {if super::adapter::addressed(&piece.path) {checkpoints.push(piece);}Ok(())})?;
    let bytes=source.virtual_bytes();
    let scanned=super::adapter::Scanned {source,checkpoints:&checkpoints,bytes:bytes.as_ref()};
    let mut refs=super::adapter::refs(source,&record);
    if record.error.is_none() {super::adapter::expand_arguments(&scanned,&mut record,&mut refs)?;}
    Ok((record,refs))
}

/// The records of one source a full read covers: the active branch of a tree recording as of the
/// segment's end, or the active records of any other.
fn selection(kind:SessionKind,state:&State,source:&RegisteredSource) -> BTreeSet<u64> {
    let limit=source.segment.end_byte.unwrap_or(source.size);
    let within=|record:&super::store::Rec|record.end<=limit;
    if !matches!(kind,SessionKind::Claude|SessionKind::Pi|SessionKind::Omp) {
        return state.records.iter().filter(|(_,record)|within(record) && record.active).map(|(start,_)|*start).collect();
    }
    let leaf=state.records.values().rev().filter(|record|within(record)).find(|record|record.native.is_some() || record.rewound)
        .map(|record|if record.rewound {record.leaf.clone()} else {record.native.clone()});
    let mut chain:HashSet<Box<str>>=HashSet::new();
    if let Some(Some(leaf))=leaf {
        let mut pending=vec![leaf];
        while let Some(id)=pending.pop() {
            if chain.len()>100_000 || !chain.insert(id.clone()) {continue;}
            for start in state.identities.get(&id).into_iter().flatten() {
                if let Some(record)=state.records.get(start).filter(|record|within(record)) {
                    if let Some(parent)=state.parent_of(*start,record) {pending.push(parent);}
                }
            }
        }
    }
    state.records.iter().filter(|(_,record)|within(record) && record.native.as_ref().is_none_or(|native|chain.contains(native)))
        .map(|(start,_)|*start).collect()
}

pub struct Cursor {pub branch:Branch,pub(crate) selected:Vec<BTreeSet<u64>>,source:usize,after:Option<u64>,pending:VecDeque<StreamEvent>}
impl Cursor {
    pub fn new(branch:Branch,_cancel:&AtomicBool) -> Result<Self,String> {
        super::children::validate_parent(&branch)?;
        let mut selected=Vec::new();
        for source in &branch.sources {
            let state=source.current()?;selected.push(selection(branch.kind,&state,source));
        }
        Ok(Self {branch,selected,source:0,after:None,pending:VecDeque::new()})
    }

    pub fn rewind(&mut self) {self.source=0;self.after=None;self.pending.clear();}
    /// Whether the record of `source` at `start` is part of this read.
    pub(crate) fn contains(&self,source:usize,start:u64) -> bool {self.selected.get(source).is_some_and(|records|records.contains(&start))}
    pub(crate) fn next_record(&mut self,cancel:&AtomicBool) -> Result<Option<IndexedRecord>,String> {
        loop {
            if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
            let Some(source)=self.branch.sources.get(self.source).cloned() else {return Ok(None);};
            let from=self.after.map_or(0,|start|start+1);
            let next=self.selected[self.source].range(from..).next().copied();
            let Some(start)=next else {self.source+=1;self.after=None;continue;};
            self.after=Some(start);
            let (error,ordinal)={let state=source.current()?;let record=&state.records[&start];(record.error.clone(),record.ordinal)};
            if source.segment.end_ordinal.is_some_and(|end|ordinal.is_some_and(|ordinal|ordinal>=end)) {continue;}
            if let Some(error)=error {return Err(format!("index_incomplete: {error}"));}
            let (record,references)=read_record(&source,start)?;
            return Ok(Some(IndexedRecord {source,record,references}));
        }
    }

    /// The tool a result answers: the earliest selected call row with that id that names its tool.
    fn tool_name(&self,call:&str) -> Option<String> {
        for (index,candidate) in self.branch.sources[..=self.source.min(self.branch.sources.len()-1)].iter().enumerate().rev() {
            let state=candidate.index.read();
            let mut found:Option<(u64,String)>=None;
            for row in state.calls.get(call).into_iter().flatten() {
                for (start,part) in state.parts.get(row).into_iter().flatten() {
                    if !self.contains(index,*start) || candidate.segment.end_ordinal.is_some_and(|end|part.ordinal.is_some_and(|ordinal|ordinal>=end)) {continue;}
                    let Some(name)=serde_json::from_str::<Value>(&part.summary).ok().and_then(|value|value["name"].as_str().map(str::to_owned)) else {continue;};
                    if name=="tool" {continue;}
                    if found.as_ref().is_none_or(|(at,_)|*start<*at) {found=Some((*start,name));}
                }
            }
            if let Some((_,name))=found {return Some(name);}
        }
        None
    }

    pub fn next(&mut self,cancel:&AtomicBool) -> Result<Option<StreamEvent>,String> {
        loop {
            if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
            if let Some(event)=self.pending.pop_front() {return Ok(Some(event));}
            let Some(IndexedRecord {source,record,references})=self.next_record(cancel)? else {return Ok(None);};
            let raw=record.value.to_string();
            if self.branch.kind.uses_opencode() && record.value["info"]["role"]=="user" {
                if record.value["type"]!="opencode_message" {continue;}
                let message=record.value["info"]["id"].as_str().ok_or("unsupported_record: native user identity is unavailable")?;
                let bytes=super::opencode::bytes(&source)?;
                let text={let state=source.current()?;super::opencode::user_preview(&state,&source,&bytes,message)?};
                if let Some(text)=text {
                    let at=record.value.pointer("/info/time/created").and_then(Value::as_i64).and_then(crate::agent::chat::opencode_timeline::iso_from_ms);
                    let row_id=Some(format!("native:{}",super::store::digest(format!("opencode:{}:u:{message}",source.segment.native_id).as_bytes())));
                    return Ok(Some(StreamEvent {event:Event::User {text,ts:at},body:Text {pieces:vec![Piece::NativeUser(source,message.into())],quote_json:false,separator:"\n"},row_id}));
                }continue;
            }
            let events=if record.value["type"].as_str()==Some("dsh_row") {
                super::deepseek_legacy::events(&record.value)
            } else {match self.branch.kind {
                SessionKind::Codex=>export::codex_events(&raw),SessionKind::Claude=>export::claude_events(&raw),
                SessionKind::Pi|SessionKind::Omp=>export::pi_events(&raw),
                SessionKind::Kiro=>if record.value["type"]=="kiro_message" {
                    let text=record.value["text"].as_str().unwrap_or_default().into();let ts=record.value["timestamp"].as_str().map(str::to_owned);
                    vec![if record.value["role"]=="user" {Event::User {text,ts}} else {Event::AssistantText {text,ts}}]
                } else {Vec::new()},
                kind if kind.uses_opencode()=> {
                    let message=crate::agent::opencode_store::OpencodeMessage {info:record.value["info"].clone(),
                        parts:if record.value["type"]=="opencode_part" {vec![record.value["part"].clone()]} else {vec![]}};
                    crate::agent::chat::opencode_timeline::events(&[message])
                },
                other=>return Err(format!("Streamed event adapter is unavailable for {} sessions",other.as_str())),
            }};
            let refs:HashMap<&str,&FieldMeta>=references.iter().map(|(path,(_,meta))|(path.as_str(),meta)).collect();
            for (local,mut event) in events.into_iter().enumerate() {
                let (role,call,fallback,quote)=match &event {
                    Event::User {text,..}|Event::AssistantText {text,..}|Event::Command {text,..}=>("body",None,text.clone(),false),
                    Event::Thinking {text,..}=>("reasoning",None,text.clone(),false),
                    Event::Wake {summary,..}=>("body",None,summary.clone(),false),
                    Event::ToolUse {id,input,..}=>("toolInput",id.as_deref(),serde_json::to_string_pretty(input).map_err(|e|e.to_string())?,input.is_string()),
                    Event::ToolResult {id,text,..}=>("toolOutput",id.as_deref(),text.clone(),false),
                    Event::Shell {output,..}=>("toolOutput",None,output.clone(),false),
                };
                let paths=super::adapter::body_paths(&record,role,call,local);
                let roots:Vec<String>=paths.iter().filter(|path|!paths.iter().any(|parent|*path!=parent && path.starts_with(&(parent.clone()+"/"))))
                    .cloned().collect();
                let long=record.fields.iter().any(|field|roots.contains(&field.path) && field.truncated);
                let fields=roots.iter().filter_map(|path|refs.get(path.as_str()).map(|meta|(*meta).clone())).collect::<Vec<_>>();
                let mut body=Text {pieces:fields.iter().map(|meta|Piece::Field(source.clone(),meta.clone())).collect(),quote_json:quote,
                    separator:if self.branch.kind==SessionKind::Codex && role=="reasoning" {"\n\n"} else {"\n"}};
                if record.value["type"]=="kiro_message" {body.separator="";}
                if body.pieces.is_empty() {body=Text::literal(fallback.clone());}
                else if !long {body=Text {pieces:vec![Piece::VerifiedLiteral(source.clone(),fields,fallback.clone())],quote_json:false,separator:"\n"};}
                // Local command echoes and shell wrappers are semantic transformations of short fields.
                // Never substitute an untransformed original for one of those displayed values.
                if matches!(event,Event::Command {..}|Event::Shell {..}) {
                    if long {return Err("unsupported_record: this transformed shell or command body requires a streaming adapter".into());}
                    body=Text::literal(fallback.clone());
                }
                if let Event::ToolResult {id:Some(id),name,..}=&mut event {*name=self.tool_name(id);}
                let row_id=if self.branch.kind==SessionKind::Kiro {
                    record.value["id"].as_str().map(|id|format!("native:{}",super::store::digest(format!("kiro:{}:{id}:{}",source.segment.native_id,
                        if record.value["role"]=="user" {"user"} else {"assistant"}).as_bytes())))
                } else if record.value["type"]=="dsh_row" {
                    record.value.pointer("/row/id").and_then(Value::as_str).map(str::to_owned)
                } else if self.branch.kind.uses_opencode() {
                    record.value.pointer("/part/id").and_then(Value::as_str).map(|id|format!("native:{}",super::store::digest(format!("opencode:{}:{id}",source.segment.native_id).as_bytes())))
                } else {None};
                self.pending.push_back(StreamEvent {event,body,row_id});
            }
        }
    }
}
