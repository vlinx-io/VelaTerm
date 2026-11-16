//! OpenCode conversations, read from OpenCode's own database into in-memory records.
//!
//! A session is laid out once as one JSON record per message and per part, the shape the rows and
//! prose readers address; details read back from that layout. The database is opened read-only and
//! laid out again only when it changed.

use std::path::{Path,PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool,Ordering};
use serde_json::{json,Value};

use crate::models::SessionKind;
use super::adapter::{self,Refs,Scanned};
use super::limits::*;
use super::projection::{Projector,Record};
use super::store::{digest,FieldMeta,RegisteredSource,State,Store};

/// Identity, size and modification time of the database and its WAL: they change with every write.
pub(super) fn stamp(path:&Path) -> Result<String,String> {
    let mut facts=Vec::new();
    for candidate in [path.to_owned(),PathBuf::from(format!("{}-wal",path.to_string_lossy()))] {
        match std::fs::metadata(&candidate) {
            Ok(metadata)=>facts.push(json!([super::source::identity(&metadata),metadata.len(),format!("{:?}",metadata.modified().ok())])),
            Err(error) if error.kind()==std::io::ErrorKind::NotFound=>facts.push(Value::Null),
            Err(_)=>return Err("source_missing: native database metadata is unavailable".into()),
        }
    }Ok(json!(facts).to_string())
}

fn info(value:&Value,id:&str) -> Value {
    let mut result=serde_json::Map::new();for key in ["role","modelID","summary","agent","mode","time","error","parentID"] {
        if let Some(value)=value.get(key) {result.insert(key.into(),value.clone());}
    }result.insert("id".into(),json!(id));Value::Object(result)
}

/// The session as records: each message, then each of its parts, oldest first.
fn layout(path:&Path,native:&str) -> Result<Vec<u8>,String> {
    let messages=crate::agent::opencode_store::messages_in(path,native).map_err(|error| {
        if error.starts_with("OpenCode has no session") {format!("source_missing: {error}")} else {error}
    })?;
    let mut bytes=Vec::new();
    for message in messages {
        let Some(id)=message.info["id"].as_str().filter(|id|!id.is_empty() && id.len()<=200) else {continue;};
        let info=info(&message.info,id);
        serde_json::to_writer(&mut bytes,&json!({"type":"opencode_message","info":info})).map_err(|e|e.to_string())?;bytes.push(b'\n');
        for part in message.parts {
            serde_json::to_writer(&mut bytes,&json!({"type":"opencode_part","info":info,"part":part})).map_err(|e|e.to_string())?;bytes.push(b'\n');
        }
    }
    Ok(bytes)
}

pub(super) fn register(store:&Store,kind:SessionKind,path:PathBuf,native:&str) -> Result<RegisteredSource,String> {
    if native.is_empty() || native.len()>200 {return Err("detail_unavailable: native database session identity is invalid".into());}
    if !path.is_file() {return Err("source_missing: OpenCode database is unavailable".into());}
    let fingerprint=stamp(&path)?;
    let key=format!("sqlite:opencode:{}:{native}",path.to_string_lossy());
    let layout_path=path.clone();
    store.register_virtual(kind,&key,&path,native,fingerprint,||layout(&layout_path,native))
}

pub(super) fn body_paths(record:&Record,role:&str) -> Vec<String> {
    let roots:Vec<&str>=match role {"toolInput"=>vec!["/part/state/input"],"toolOutput"=>vec!["/part/state/output","/part/state/error"],_=>vec!["/part/text"]};
    record.fields.iter().filter(|field|roots.iter().any(|root|field.path==*root || field.path.starts_with(&format!("{root}/"))))
        .map(|field|field.path.clone()).collect()
}

fn row_id(source:&RegisteredSource,message:&str) -> String {
    format!("native:{}",digest(format!("opencode:{}:u:{message}",source.segment.native_id).as_bytes()))
}

/// Record starts grouped by OpenCode identity: a message's own record, its parts, and the parts of
/// assistant messages answering it.
fn group(kind:&str,id:&str) -> Box<str> {format!("opencode:{kind}:{id}").into()}
fn grouped(state:&State,kind:&str,id:&str) -> Vec<u64> {state.identities.get(&group(kind,id)).cloned().unwrap_or_default()}

/// One record of the layout, parsed again from the bytes held in memory.
fn parse(state:&State,bytes:&[u8],start:u64) -> Option<Record> {
    let end=state.records.get(&start)?.end;
    Projector::new(bytes.get(start as usize..end as usize)?,start).long_checkpoints_only().next(&mut |_|Ok(())).ok().flatten()
        .filter(|record|record.error.is_none())
}

/// The user-visible parts of a user message, each with the field holding its body.
pub(super) fn user_parts(state:&State,source:&RegisteredSource,bytes:&[u8],message:&str,mut sink:impl FnMut(&Value,Option<(String,FieldMeta)>)->Result<(),String>) -> Result<bool,String> {
    let mut shell=false;
    for start in grouped(state,"p",message) {
        if source.segment.end_byte.is_some_and(|end|state.records.get(&start).is_none_or(|record|record.end>end)) {continue;}
        let Some(record)=parse(state,bytes,start) else {continue;};
        let part=&record.value["part"];
        shell|=part["type"]=="text" && part["synthetic"]==true && part["text"]=="The following tool was executed by the user";
        if part["type"]=="text" && (part["synthetic"]==true || part["ignored"]==true) {continue;}
        let path=if part["type"]=="file" {
            if part["mime"].as_str().is_some_and(|mime|mime.starts_with("image/")) || part.get("filename").is_none() {"/part/mime"} else {"/part/filename"}
        } else {"/part/text"};
        let field=adapter::refs(source,&record).remove(path);
        let text=crate::agent::chat::opencode_timeline::user_text(std::slice::from_ref(part));
        // An empty prefix of a long field cannot prove that the entire original is whitespace.
        let long=field.as_ref().is_some_and(|(_,meta)|meta.decoded_bytes>PREVIEW_BYTES as u64);
        if !text.is_empty() || long {sink(part,field)?;}
    }Ok(shell)
}

/// The command of the shell a user ran in place of a message: the first bash call answering it.
pub(super) fn shell_command(state:&State,source:&RegisteredSource,bytes:&[u8],message:&str) -> Option<(Value,(String,FieldMeta))> {
    grouped(state,"c",message).into_iter().filter(|start|source.segment.end_byte.is_none_or(|end|state.records.get(start).is_some_and(|record|record.end<=end)))
        .find_map(|start| {
            let record=parse(state,bytes,start)?;
            if record.value.pointer("/part/type").and_then(Value::as_str)!=Some("tool") || record.value.pointer("/part/tool").and_then(Value::as_str)!=Some("bash") {return None;}
            let field=adapter::refs(source,&record).remove("/part/state/input/command")?;
            Some((record.value,field))
        })
}

pub(super) fn user_preview(state:&State,source:&RegisteredSource,bytes:&[u8],message:&str) -> Result<Option<String>,String> {
    let mut text=String::new();let mut any=false;
    let shell=user_parts(state,source,bytes,message,|part,_| {
        if any && text.len()<PREVIEW_BYTES {text.push('\n');}any=true;
        let part=crate::agent::chat::opencode_timeline::user_text(std::slice::from_ref(part));
        text.push_str(&adapter::preview(&part,PREVIEW_BYTES.saturating_sub(text.len())));Ok(())
    })?;
    if any {return Ok(Some(text));}
    if shell {if let Some((part,_))=shell_command(state,source,bytes,message) {
        return Ok(Some(format!("!{}",adapter::preview(part.pointer("/part/state/input/command").and_then(Value::as_str).unwrap_or_default(),PREVIEW_BYTES-1))));
    }}Ok(None)
}

/// The full text of a multipart user message: its parts joined by line breaks, files and images by
/// their names, or the shell command it ran.
pub(super) fn walk_user(source:&RegisteredSource,message:&str,cancel:&AtomicBool,sink:&mut impl FnMut(&str)->Result<(),String>) -> Result<(),String> {
    let bytes=source.virtual_bytes().ok_or("unsupported_record: OpenCode history bytes are unavailable")?;
    let (parts,shell)={
        let state=source.current()?;let mut parts=Vec::new();
        let shell=user_parts(&state,source,&bytes,message,|part,field|{parts.push((part.clone(),field));Ok(())})?;
        let shell=if parts.is_empty() && shell {shell_command(&state,source,&bytes,message).map(|(_,field)|field)} else {None};
        (parts,shell)
    };
    let mut any=false;
    for (part,field) in parts {
        if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
        if any {sink("\n")?;}any=true;
        if part["type"]=="text" {
            let (_,meta)=field.ok_or("index_incomplete: original user text is unavailable")?;super::fields::walk(source,&meta,cancel,sink)?;
        } else {
            let image=part["mime"].as_str().is_some_and(|mime|mime.starts_with("image/"));
            if !image {sink("[file: ")?;}
            if let Some((_,meta))=field {super::fields::walk(source,&meta,cancel,&mut |text|if image {Ok(())} else {sink(text)})?;}
            sink(if image {"[image]"} else {"]"})?;
        }
    }
    if let Some((_,meta))=shell {sink("!")?;super::fields::walk(source,&meta,cancel,sink)?;}
    Ok(())
}

pub(super) fn store_record(state:&mut State,kind:SessionKind,scanned:&Scanned,record:&Record,refs:&Refs) -> Result<(),String> {
    let source=scanned.source;
    let value=&record.value;
    if let Some(id)=value.pointer("/info/id").and_then(Value::as_str) {
        let kind=if value["type"]=="opencode_message" {"m"} else {"p"};
        state.identities.entry(group(kind,id)).or_default().push(record.start);
        if kind=="p" && value.pointer("/info/role").and_then(Value::as_str)==Some("assistant") {
            if let Some(parent)=value.pointer("/info/parentID").and_then(Value::as_str) {state.identities.entry(group("c",parent)).or_default().push(record.start);}
        }
    }
    let message=crate::agent::opencode_store::OpencodeMessage {info:value["info"].clone(),parts:if value["type"]=="opencode_part" {vec![value["part"].clone()]} else {vec![]}};
    for (local,row) in crate::agent::chat::opencode_timeline::rows(&[message],&|_|None).into_iter().enumerate() {
        let mut row=serde_json::to_value(row).map_err(|e|e.to_string())?;let native=row["id"].as_str().unwrap_or_default().to_owned();
        let id=format!("native:{}",digest(format!("opencode:{}:{native}",source.segment.native_id).as_bytes()));row["id"]=json!(id);
        let tool=row["kind"]=="tool";
        let mut paths=body_paths(record,if tool {"toolInput"} else {"body"});let mut label=if tool {"input"} else {"body"};
        let file=row["kind"]=="user" && value.pointer("/part/type").and_then(Value::as_str)==Some("file");
        if file {
            let image=value.pointer("/part/mime").and_then(Value::as_str).is_some_and(|mime|mime.starts_with("image/"));
            let path=if image || value.pointer("/part/filename").is_none() {"/part/mime"} else {"/part/filename"};
            paths=vec![path.into()];label=if image {"body_image"} else {"body_file"};
        }
        let mut size=record.fields.iter().filter(|field|paths.contains(&field.path) && field.encoding=="string").map(|field|field.decoded_bytes).sum::<u64>();
        if file {size=if label=="body_image" {7} else {size+8};}
        if tool {row["input"]=adapter::short_input(&row["input"]);row["detailAvailable"]=json!(true);if let Some(output)=row["output"].as_str() {row["output"]=json!(adapter::preview(output,512));}}
        else if let Some(text)=row["text"].as_str() {
            let text=adapter::preview(text,PREVIEW_BYTES);row["text"]=json!(text);
            if row["kind"]=="user" {
                if let Some(old)=state.summary(&id) {
                    let old:Value=serde_json::from_str(old).map_err(|e|e.to_string())?;let mut text=old["text"].as_str().unwrap_or_default().to_owned();
                    if !text.is_empty() {text.push('\n');}text.push_str(row["text"].as_str().unwrap_or_default());row["text"]=json!(adapter::preview(&text,PREVIEW_BYTES));
                    size=size.saturating_add(old["bodySizeBytes"].as_u64().unwrap_or(0)).saturating_add(1);
                }
            }
            row["bodySizeBytes"]=json!(size);if size>PREVIEW_BYTES as u64 {row["bodyAvailable"]=json!(true);}
        }
        super::children::annotate(kind,record,&mut row)?;
        adapter::write_row(state,scanned,record,local,value.pointer("/part/callID").and_then(Value::as_str),&row,refs);
        adapter::attach(state,&id,label,&paths,refs);
        if row["kind"]=="user" {
            for path in &paths {if let Some((reference,_))=refs.get(path) {state.append.insert(reference.clone());}}
        }
        if tool {
            adapter::attach(state,&id,"output",&body_paths(record,"toolOutput"),refs);
            adapter::attach(state,&id,"original",&["/part".into()],refs);
        }
    }
    restore_shell_row(state,scanned,record,refs)
}

/// A shell the user ran stands in OpenCode's history as an assistant bash call; it is shown as the
/// user's `!command` message when that message has no text of its own.
fn restore_shell_row(state:&mut State,scanned:&Scanned,record:&Record,refs:&Refs) -> Result<(),String> {
    let source=scanned.source;
    if record.value.pointer("/part/type").and_then(Value::as_str)!=Some("tool") || record.value.pointer("/part/tool").and_then(Value::as_str)!=Some("bash") {return Ok(());}
    let Some(parent)=record.value.pointer("/info/parentID").and_then(Value::as_str) else {return Ok(());};
    let id=row_id(source,parent);
    if state.rows.contains_key(id.as_str()) {return Ok(());}
    let Some(bytes)=scanned.bytes.cloned() else {return Ok(());};
    let Some(text)=user_preview(state,source,&bytes,parent)? else {return Ok(());};
    let Some((_,command))=refs.get("/part/state/input/command").cloned() else {return Ok(());};
    let Some(start)=grouped(state,"m",parent).first().copied() else {return Ok(());};
    let Some(origin)=parse(state,&bytes,start) else {return Ok(());};
    let size=command.decoded_bytes;
    let row=json!({"id":id,"kind":"user","text":text,"images":[],"at":origin.value.pointer("/info/time/created"),"bodySizeBytes":size+1,"bodyAvailable":size+1>PREVIEW_BYTES as u64});
    adapter::write_row(state,scanned,record,0,None,&row,refs);
    state.set_order(&id,(start,0));
    adapter::attach(state,&id,"body_shell",&["/part/state/input/command".into()],refs);Ok(())
}

/// Every part of a user message in the layout is already scanned when its message is read.
pub(super) fn bytes(source:&RegisteredSource) -> Result<Arc<Vec<u8>>,String> {
    source.virtual_bytes().ok_or_else(||"unsupported_record: OpenCode history bytes are unavailable".into())
}
