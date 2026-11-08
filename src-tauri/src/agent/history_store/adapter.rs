//! Provider-specific meaning over a bounded physical-record projection.

use std::collections::{HashMap,HashSet};
use std::io::Read;
use std::sync::Arc;

use serde_json::{json,Value};

use crate::agent::export::{self,Event};
use crate::models::SessionKind;
use super::limits::PREVIEW_BYTES;
use super::projection::{Checkpoint,Projector,Record};
use super::store::{digest,FieldMeta,Rec,RegisteredSource,State};

/// The fields of each record keyed by path: the reference a client uses and where the original is.
pub type Refs=HashMap<String,(String,FieldMeta)>;

/// A record being scanned: its source, the checkpoints of its long strings, and the source bytes when
/// they are held in memory. Reads during a scan come from the bytes just parsed.
pub(super) struct Scanned<'a> {pub source:&'a RegisteredSource,pub checkpoints:&'a [Checkpoint],pub bytes:Option<&'a Arc<Vec<u8>>>}

impl Scanned<'_> {
    fn raw(&self,start:u64,end:u64) -> Result<Vec<u8>,String> {
        if let Some(bytes)=self.bytes {return bytes.get(start as usize..end as usize).map(<[u8]>::to_vec).ok_or_else(||"source_changed: history record was truncated".into());}
        use std::io::{Seek,SeekFrom};
        let mut file=std::fs::File::open(&self.source.segment.path).map_err(|_|"detail_unavailable: history source is unavailable")?;
        file.seek(SeekFrom::Start(start)).map_err(|e|e.to_string())?;
        let mut bytes=vec![0;(end-start) as usize];file.read_exact(&mut bytes).map_err(|_|"source_changed: history source was truncated")?;Ok(bytes)
    }

    /// The decoded text of a string field of the record being scanned, read piece by piece.
    pub(super) fn text(&self,meta:&FieldMeta) -> Result<ScanText<'_>,String> {
        let pieces=self.checkpoints.iter().filter(|piece|piece.path==meta.path).cloned().collect::<Vec<_>>();
        let pieces=if pieces.is_empty() {
            vec![Checkpoint {record_start:meta.record_start,path:meta.path.clone(),raw_start:meta.raw_start,raw_end:meta.raw_end,decoded_start:0,
                decoded_end:meta.decoded_bytes,hash:Vec::new(),sequence:0}]
        } else {pieces};
        Ok(ScanText {scanned:self,pieces,next:0,current:std::io::Cursor::new(Vec::new())})
    }
}

/// Decoded text of a string field of a record being scanned.
pub(super) struct ScanText<'a> {scanned:&'a Scanned<'a>,pieces:Vec<Checkpoint>,next:usize,current:std::io::Cursor<Vec<u8>>}
impl Read for ScanText<'_> {
    fn read(&mut self,output:&mut [u8]) -> std::io::Result<usize> {
        loop {
            let count=self.current.read(output)?;
            if count>0 || output.is_empty() {return Ok(count);}
            let Some(piece)=self.pieces.get(self.next) else {return Ok(0);};self.next+=1;
            let raw=self.scanned.raw(piece.raw_start,piece.raw_end).map_err(std::io::Error::other)?;
            let text=super::projection::decode_checkpoint(&raw).map_err(std::io::Error::other)?;
            self.current=std::io::Cursor::new(text.into_bytes());
        }
    }
}

/// A field reference: 128 bits of the field's identity within this source generation.
pub(super) fn field_ref(identity:&str) -> String {let mut reference=digest(identity.as_bytes());reference.truncate(32);reference}

pub(super) fn meta(record_start:u64,field:&super::projection::Field) -> FieldMeta {
    FieldMeta {record_start,path:field.path.clone(),raw_start:field.start,raw_end:field.end,decoded_bytes:field.decoded_bytes,parent:None,
        encoding:field.encoding.clone(),hash:field.hash.clone()}
}

/// References of every addressed field of a record; the last occurrence of a duplicated key wins.
pub(super) fn refs(source:&RegisteredSource,record:&Record) -> Refs {
    let mut refs=Refs::new();
    if metadata_only(&record.value) {return refs;}
    for field in record.fields.iter().filter(|field|addressed(&field.path)) {
        let reference=field_ref(&format!("{}:{}:{}:{}",source.id,source.generation,record.start,field.path));
        refs.insert(field.path.clone(),(reference,meta(record.start,field)));
    }
    refs
}

/// Codex tool arguments are a JSON document inside a string. A long one is parsed again from its
/// decoded text so its own fields can be addressed.
pub(super) fn expand_arguments(scanned:&Scanned,record:&mut Record,refs:&mut Refs) -> Result<(),String> {
    let outer=record.fields.iter().find(|field|matches!(field.path.as_str(),"/payload/arguments"|"/payload/input") && field.truncated).cloned();
    let Some(outer)=outer else {return Ok(());};
    if !record.value.pointer(&outer.path).and_then(Value::as_str).is_some_and(|text|matches!(text.trim_start().as_bytes().first(),Some(b'{'|b'['))) {return Ok(());}
    let Some((_,outer_meta))=refs.get(&outer.path).cloned() else {return Ok(());};
    let mut parser=Projector::new(scanned.text(&outer_meta)?,0).long_checkpoints_only();
    let Some(inner)=parser.next(&mut |_|Ok(()))? else {return Ok(());};
    // A tool is allowed to take ordinary string arguments. Keep their original detail reference. A
    // malformed inner object is never retried through an unbounded serde_json allocation.
    if inner.error.is_some() || !inner.value.is_object() && !inner.value.is_array() {return Ok(());}
    let source=scanned.source;
    let prefix=format!("{}/@json",outer.path);
    for mut field in inner.fields {
        field.path=format!("{prefix}{}",field.path);
        let inner_ref=field_ref(&format!("{}:{}:{}:{}",source.id,source.generation,record.start,field.path));
        let mut inner_meta=meta(record.start,&field);inner_meta.parent=Some(Box::new(outer_meta.clone()));
        refs.insert(field.path.clone(),(inner_ref,inner_meta));
        record.fields.push(field);
    }
    if let Some(value)=record.value.pointer_mut(&outer.path) {*value=inner.value;}
    Ok(())
}

pub fn preview(text:&str,bytes:usize) -> String {
    let mut end=text.len().min(bytes);
    while !text.is_char_boundary(end) {end-=1;}
    text[..end].to_owned()
}

/// Subtrees no reader addresses field by field. Codex writes the whole conversation so far into
/// every compaction record; indexing those copies again made a long session's index several times
/// its own size. The record projection still carries the record for classification. A reasoning
/// item's `encrypted_content` is opaque ciphertext no view shows; it can be a third of a long session.
const UNADDRESSED_SUBTREES:[&str;2]=["/payload/replacement_history","/payload/encrypted_content"];
pub(super) fn addressed(path:&str) -> bool {!UNADDRESSED_SUBTREES.iter().any(|subtree|path.starts_with(subtree))}

/// Codex records that only carry usage, settings or lifecycle state. Readers take them from the
/// record projection and never address their strings, so they get no field references.
fn metadata_only(value:&Value) -> bool {
    match value["type"].as_str() {
        Some("token_usage_record"|"world_state"|"turn_context"|"session_meta")=>true,
        Some("event_msg")=>matches!(value.pointer("/payload/type").and_then(Value::as_str),
            Some("token_count"|"thread_settings_applied"|"task_started"|"task_complete")),
        _=>false,
    }
}

fn get<'a>(value:&'a Value,names:&[&str]) -> Option<&'a str> {
    names.iter().find_map(|key|value.get(*key).and_then(Value::as_str))
}

pub(crate) fn short_input(input:&Value) -> Value {
    let mut object=serde_json::Map::new();
    for key in ["command","description","path","file_path","notebook_path","pattern","url","query","skill","name","subagent_type","agentThreadId"] {
        if let Some(value)=input.get(key).and_then(Value::as_str) {
            object.insert(key.into(),Value::String(preview(value,512)));
        }
    }
    Value::Object(object)
}

pub(crate) fn body_paths(record:&Record,role:&str,call:Option<&str>,local:usize) -> Vec<String> {
    if record.value["method"]=="session/update" {return vec!["/params/update/content/text".into()];}
    if record.value["type"]=="kiro_message" {return vec!["/text".into()];}
    if matches!(record.value["type"].as_str(),Some("opencode_message"|"opencode_part")) {return super::opencode::body_paths(record,role);}
    if record.value["type"].as_str()==Some("response_item") {
        return record.fields.iter().filter(|field|field.encoding=="string" || role=="toolInput").filter(|field|match role {
            "toolInput"=>field.path.starts_with("/payload/arguments") || field.path.starts_with("/payload/input"),
            "toolOutput"=>field.path.starts_with("/payload/output"),
            "reasoning"=>field.path.starts_with("/payload/summary/") && field.path.ends_with("/text"),
            _=>field.path.starts_with("/payload/content/") && field.path.ends_with("/text"),
        }).map(|field|field.path.clone()).collect();
    }
    if let Some(call)=call {
        if let Some(blocks)=record.value.pointer("/message/content").and_then(Value::as_array) {
            for (index,block) in blocks.iter().enumerate() {
                if get(block,&["id","tool_use_id","toolCallId"])==Some(call) {
                    let root=format!("/message/content/{index}/{}",if role=="toolInput" {
                        if block.get("input").is_some() {"input"} else {"arguments"}
                    } else {"content"});
                    return record.fields.iter().filter(|field|field.path==root || field.path.starts_with(&(root.clone()+"/")) &&
                        (role=="toolInput" || field.encoding=="string" && field.path.ends_with("/text")))
                        .map(|field|field.path.clone()).collect();
                }
            }
        }
    }
    if (record.value["type"]=="assistant" || record.value["type"]=="message" && record.value.pointer("/message/role").and_then(Value::as_str)==Some("assistant")) && call.is_none() {
        if let Some(blocks)=record.value.pointer("/message/content").and_then(Value::as_array) {
            let selected=blocks.iter().enumerate().filter(|(_,block)|match block["type"].as_str() {
                Some("tool_use"|"toolCall")=>true,
                Some("text")=>block["text"].as_str().is_some_and(|text|!text.trim().is_empty() && text.trim()!=export::NO_RESPONSE_PLACEHOLDER),
                Some("thinking")=>block["thinking"].as_str().is_some_and(|text|!text.trim().is_empty()),
                _=>false,
            }).nth(local).map(|(index,_)|index);
            if let Some(index)=selected {
                let root=format!("/message/content/{index}/{}",if role=="reasoning" {"thinking"} else if role=="toolInput" {"input"} else {"text"});
                return record.fields.iter().filter(|field|field.path==root || (role=="toolInput" && field.path.starts_with(&(root.clone()+"/"))))
                    .map(|field|field.path.clone()).collect();
            }
        }
    }
    record.fields.iter().filter(|field|field.encoding=="string" || role=="toolInput").filter(|field|match role {
        "toolInput"=>field.path.contains("/input/") || field.path.contains("/arguments/"),
        "toolOutput"=>field.path.starts_with("/message/content") || field.path=="/message/output",
        "reasoning"=>field.path.starts_with("/message/content/") && field.path.ends_with("/thinking"),
        _=>field.path=="/message/content" || (field.path.starts_with("/message/content/") && field.path.ends_with("/text")) || field.path=="/content",
    }).map(|field|field.path.clone()).collect()
}

/// Attach the fields at `paths` of the record being scanned to a row.
pub(super) fn attach(state:&mut State,row:&str,label:&str,paths:&[String],refs:&Refs) {
    let fields=paths.iter().filter_map(|path|refs.get(path).cloned()).collect::<Vec<_>>();
    state.attach(row,label,&fields);
}

pub(super) fn write_row(state:&mut State,scanned:&Scanned,record:&Record,local:usize,call:Option<&str>,value:&Value,refs:&Refs) {
    state.write_row(record,local,call,value);
    if value["kind"]=="user" {super::attachments::attach(state,scanned.source,record,value["id"].as_str().unwrap(),refs);}
}

/// The branch facts of a record that readers need after its projection is gone.
fn facts(kind:SessionKind,record:&Record) -> Rec {
    let value=&record.value;
    let native=match kind {
        SessionKind::Claude=>get(value,&["uuid"]),
        SessionKind::Pi|SessionKind::Omp=>get(value,&["id"]),
        SessionKind::Opencode|SessionKind::Kiro=>None,
        _=>value.pointer("/payload/id").and_then(Value::as_str),
    };
    let results=value["type"]=="user" && value.pointer("/message/content").and_then(Value::as_array)
        .is_some_and(|content|!content.is_empty() && content.iter().all(|block|block["type"]=="tool_result"));
    let rewound=value["type"]=="last-prompt" && value["rewound"]==true;
    let parent=get(value,&["parentUuid","parentId"]);
    let logical=parent.is_none().then(||get(value,&["logicalParentUuid"])).flatten();
    Rec {end:record.end,ordinal:value["ordinal"].as_u64(),native:native.map(Into::into),parent:parent.or(logical).map(Into::into),logical:logical.is_some(),
        event:value["type"].as_str().map(Into::into),subtype:value.pointer("/payload/type").and_then(Value::as_str).map(Into::into),
        turn:(value["type"]=="turn_context").then(||value.pointer("/payload/turn_id").or_else(||value.pointer("/payload/turnId")).and_then(Value::as_str).map(Into::into)).flatten(),
        message:value.pointer("/message/id").and_then(Value::as_str).map(Into::into),results,rewound,
        leaf:rewound.then(||value["leafUuid"].as_str().map(Into::into)).flatten(),
        active:true,error:record.error.as_deref().map(Into::into)}
}

pub(super) fn store_record(state:&mut State,kind:SessionKind,scanned:&Scanned,record:&Record,refs:&Refs) -> Result<(),String> {
    let source=scanned.source;
    state.record(record.start,facts(kind,record));
    if let Some(error)=&record.error {
        let row=json!({"kind":"notice","id":format!("gap:{}:{}:{}",source.id,source.generation,record.start),"message":error});
        state.write_row(record,0,None,&row);return Ok(());
    }
    match kind {
        SessionKind::Antigravity=>return super::antigravity_adapter::store(state,scanned,record,refs),
        SessionKind::Opencode=>return super::opencode::store_record(state,scanned,record,refs),
        SessionKind::Kiro=>return super::kiro::store_record(state,scanned,record,refs),
        _=>{},
    }
    let value=&record.value;
    let native=match kind {
        SessionKind::Claude=>get(value,&["uuid"]),
        SessionKind::Pi|SessionKind::Omp=>get(value,&["id"]),
        _=>value.pointer("/payload/id").and_then(Value::as_str),
    };
    let mut projected=value.clone();
    if super::children::is_claude_child(source) {projected["isSidechain"]=json!(false);}
    let raw=projected.to_string();
    let events=match kind {
        SessionKind::Codex=>export::codex_events(&raw),
        SessionKind::Claude=>export::claude_events(&raw),
        SessionKind::Pi|SessionKind::Omp=>export::pi_events(&raw),
        SessionKind::Grok=>crate::agent::transcript::grok_pieces(&projected).into_iter().map(|piece| {
            if let Some(name)=piece.tools.first() {Event::ToolUse {id:None,name:name.clone(),input:json!({}),ts:piece.timestamp}}
            else if piece.role=="user" {Event::User {text:piece.text,ts:piece.timestamp}}
            else {Event::AssistantText {text:piece.text,ts:piece.timestamp}}
        }).collect(),
        _=>return Err("Bounded history projection has no provider adapter".into()),
    };
    for (local,event) in events.into_iter().enumerate() {
        let physical=format!("{}:{}:{}:{}:{local}",kind.as_str(),source.id,source.generation,record.start);
        let mut call_id=None;
        let mut at=None;
        let mut label="body";
        let mut role="body";
        let mut row=match event {
            Event::User {text,ts}=>{at=ts;json!({"kind":"user","text":preview(&text,PREVIEW_BYTES)})}
            Event::AssistantText {text,ts}=>{at=ts;json!({"kind":"assistant","text":preview(&text,PREVIEW_BYTES),"streaming":false})}
            Event::Thinking {text,..}=>{role="reasoning";json!({"kind":"reasoning","text":preview(&text,PREVIEW_BYTES),"streaming":false})}
            Event::Command {text,ts}=>{at=ts;json!({"kind":"command","text":preview(&text,PREVIEW_BYTES)})}
            Event::Wake {summary,ts}=>{at=ts;json!({"kind":"wake","summary":preview(&summary,PREVIEW_BYTES)})}
            Event::Shell {command,output,exit_code,cancelled,truncated,ts,..}=>{at=ts;json!({"kind":"shell","command":preview(&command,PREVIEW_BYTES),
                "stdout":preview(&output,PREVIEW_BYTES),"stderr":"","stdoutTruncated":truncated,"stderrTruncated":false,
                "status":if cancelled {"cancelled"} else {"completed"},"exitCode":exit_code})}
            Event::ToolUse {id,name,input,ts}=>{
                call_id=id;at=ts;label="input";role="toolInput";
                json!({"kind":"tool","name":name,"input":short_input(&input),"isError":false,"status":"running","detailAvailable":true})
            }
            Event::ToolResult {id,name,text,is_error}=>{
                call_id=id;label="output";role="toolOutput";
                json!({"kind":"tool","name":name.unwrap_or_else(||"tool".into()),"input":{},"output":preview(&text,512),
                    "isError":is_error,"status":if is_error {"failed"} else {"completed"},"detailAvailable":true})
            }
        };
        let id=if let Some(call)=&call_id {format!("call:{}",digest(format!("{}:{}:{call}",kind.as_str(),source.segment.native_id).as_bytes()))}
            else if let Some(native)=native {format!("native:{}",digest(format!("{}:{}:{native}:{local}",kind.as_str(),source.segment.native_id).as_bytes()))}
            else {format!("record:{}",digest(physical.as_bytes()))};
        row["id"]=Value::String(id.clone());
        if let Some(at)=crate::agent::chat::history::parsed_at(at.as_deref()) {row["at"]=json!(at);}
        let paths=body_paths(record,role,call_id.as_deref(),local);
        let long=paths.iter().any(|path|record.fields.iter().any(|field|&field.path==path && field.truncated));
        if long && row["kind"]!="tool" {row["bodyAvailable"]=json!(true);}
        super::children::annotate(kind,record,&mut row)?;
        write_row(state,scanned,record,local,call_id.as_deref(),&row,refs);
        attach(state,&id,label,&paths,refs);
        if kind==SessionKind::Codex {
            if row["kind"]=="user" {state.kv.insert(CODEX_PENDING_USER.into(),id.clone());}
            if let Some(call)=&call_id {
                let open=format!("{call}\n{id}");
                if role=="toolInput" {state.kv.insert(CODEX_OPEN_CALL.into(),open);}
                else if state.kv.get(CODEX_OPEN_CALL)==Some(&open) {
                    // The bracket closed; a later completed item belongs to another call.
                    state.kv.insert(CODEX_OPEN_CALL.into(),String::new());
                }
            }
        }
    }
    // Completed native items carry historical diffs that response_item does not include.
    if kind==SessionKind::Codex && matches!(value.pointer("/payload/type").and_then(Value::as_str),Some("item_completed"|"itemCompleted")) {
        if let Some(item)=value.pointer("/payload/item") {
            supersede_codex_response(state,source,item);
            super::children::codex_record(state,source,record,item)?;
            if item["type"].as_str().is_some_and(|kind|kind.replace('_',"").eq_ignore_ascii_case("userMessage")) {
                codex_user(state,scanned,record,refs,"/payload/item")?;
            }
            if let Some(mut row)=crate::agent::chat::history::recorded_codex_item_row(item,value["timestamp"].as_str())
                .and_then(|row|serde_json::to_value(row).ok()) {
                let native=row["id"].as_str().unwrap_or_default().to_owned();
                let id=if row["kind"]=="assistant" || row["kind"]=="reasoning" {
                    format!("native:{}",digest(format!("{}:{}:{native}:0",kind.as_str(),source.segment.native_id).as_bytes()))
                } else {format!("item:{}",digest(format!("{}:{}:{native}",kind.as_str(),source.segment.native_id).as_bytes()))};
                row["id"]=json!(id);
                row["detailAvailable"]=json!(true);
                if row["kind"]=="tool" {
                    row["input"]=short_input(&row["input"]);
                    if let Some(output)=row["output"].as_str() {row["output"]=json!(preview(output,512));}
                }
                let category=item["type"].as_str().unwrap_or_default().replace('_',"").to_ascii_lowercase();
                let roots:Vec<&str>=match category.as_str() {
                    "agentmessage"=>vec!["content"],
                    "reasoning"=>["summary_text","summaryText","summary","raw_content"].into_iter().find(|key|item.get(key).is_some()).into_iter().collect(),
                    "commandexecution"=>vec!["command","cwd"],
                    "filechange"=>vec!["changes"],
                    "mcptoolcall"|"dynamictoolcall"=>vec!["arguments"],
                    "websearch"=>vec!["query"],
                    _=>Vec::new(),
                };
                let paths:Vec<_>=record.fields.iter().filter(|field|roots.iter().any(|root| {
                    let prefix=format!("/payload/item/{root}");field.path==prefix || field.path.starts_with(&(prefix+"/"))
                }) && (row["kind"]=="tool" || field.encoding=="string" && (field.path.ends_with("/text") || !field.path.contains("/content/"))))
                    .map(|field|field.path.clone()).collect();
                if row["kind"]!="tool" && record.fields.iter().any(|field|paths.contains(&field.path) && field.truncated) {
                    row["bodyAvailable"]=json!(true);
                }
                write_row(state,scanned,record,0,Some(&native),&row,refs);
                attach(state,&id,if row["kind"]=="tool" {"input"} else {"body"},&paths,refs);
                let outputs:Vec<_>=match category.as_str() {
                    "commandexecution"=>vec!["aggregated_output","aggregatedOutput","formatted_output","stdout","stderr"],
                    "mcptoolcall"|"dynamictoolcall"=>vec!["result","error"],
                    "websearch"=>vec!["results"],
                    _=>Vec::new(),
                };
                let paths:Vec<_>=record.fields.iter().filter(|field|outputs.iter().any(|root| {
                    let prefix=format!("/payload/item/{root}");field.path==prefix || field.path.starts_with(&(prefix+"/"))
                })).map(|field|field.path.clone()).collect();
                attach(state,&id,"output",&paths,refs);
                store_files(state,&id,record.start,item,refs);
            }
        }
    }
    Ok(())
}

const CODEX_PENDING_USER:&str="codex:pendingUser";
const CODEX_OPEN_CALL:&str="codex:openCall";

pub(super) fn item_id(kind:SessionKind,source:&RegisteredSource,id:&str,prose:bool) -> String {
    if prose {format!("native:{}",digest(format!("{}:{}:{id}:0",kind.as_str(),source.segment.native_id).as_bytes()))}
    else {format!("item:{}",digest(format!("{}:{}:{id}",kind.as_str(),source.segment.native_id).as_bytes()))}
}

/// Newer Codex recordings write each turn twice: response items for the model and completed items
/// for the client. The view shows the completed items, so their response-item copies are superseded:
/// the user message just before a completed user message, and a generic call whose bracket holds a
/// completed tool item. Superseded rows stay addressable, because transcript prose, exports and
/// search still read the response items and authorize their fields through these rows.
fn supersede_codex_response(state:&mut State,source:&RegisteredSource,item:&Value) {
    let category=item["type"].as_str().unwrap_or_default().replace('_',"").to_ascii_lowercase();
    let Some(native)=item["id"].as_str() else {return;};
    if category=="usermessage" {
        if let Some(id)=state.kv.get(CODEX_PENDING_USER).filter(|id|!id.is_empty()).cloned() {
            state.superseded.insert(id,item_id(SessionKind::Codex,source,native,true));state.kv.insert(CODEX_PENDING_USER.into(),String::new());
        }
    }
    if matches!(category.as_str(),"filechange"|"commandexecution"|"mcptoolcall"|"dynamictoolcall"|"websearch") {
        if let Some((_,id))=state.kv.get(CODEX_OPEN_CALL).and_then(|value|value.split_once('\n')).map(|(call,id)|(call.to_owned(),id.to_owned())) {
            state.superseded.insert(id,item_id(SessionKind::Codex,source,native,false));
        }
    }
}

/// A completed Codex user message: its text parts form one user row whose complete value replaces
/// the pieces it may have been appended from.
fn codex_user(state:&mut State,scanned:&Scanned,record:&Record,refs:&Refs,root:&str) -> Result<(),String> {
    let item=record.value.pointer(root).ok_or("unsupported_record: native user item is missing")?;
    let native=item["id"].as_str().filter(|id|!id.is_empty() && id.len()<=200).ok_or("unsupported_record: native user item has no bounded identity")?;
    let content=item["content"].as_array().ok_or("unsupported_record: native user content is unavailable")?;
    let selected=content.iter().enumerate().filter(|(_,part)|matches!(part["type"].as_str(),Some("text"|"input_text")))
        .map(|(index,_)|format!("{root}/content/{index}/text")).collect::<Vec<_>>();
    let mut text=String::new();let mut size=0u64;let mut parts=0;
    for path in &selected {if let Some(field)=record.fields.iter().find(|field|field.path==*path) {
        if parts>0 {if text.len()<PREVIEW_BYTES {text.push('\n');}size+=1;}parts+=1;
        size+=field.decoded_bytes;
        let part=record.value.pointer(path).and_then(Value::as_str).ok_or("unsupported_record: native user text is invalid")?;
        let remaining=PREVIEW_BYTES.saturating_sub(text.len());text.push_str(&preview(part,remaining));
    }}
    let id=item_id(SessionKind::Codex,scanned.source,native,true);
    let row=json!({"id":id,"kind":"user","text":text,"images":[],"bodySizeBytes":size,"bodyAvailable":size>text.len() as u64});
    write_row(state,scanned,record,0,Some(native),&row,refs);
    state.detach(&id,"body");attach(state,&id,"body",&selected,refs);
    Ok(())
}

pub(super) fn store_files(state:&mut State,row:&str,record_start:u64,item:&Value,refs:&Refs) {
    store_files_at(state,row,record_start,item,refs,"/payload/item/changes")
}

pub(super) fn store_files_at(state:&mut State,row:&str,record_start:u64,item:&Value,refs:&Refs,root:&str) {
    let changes=match item.get("changes") {
        Some(Value::Array(array))=>array.iter().enumerate().map(|(index,value)|(index.to_string(),value)).collect::<Vec<_>>(),
        Some(Value::Object(object))=>object.iter().map(|(key,value)|(key.clone(),value)).collect::<Vec<_>>(),
        _=>return,
    };
    let key=state.row_id(row).unwrap_or_else(||Arc::from(row));
    for (index,(name,change)) in changes.into_iter().enumerate() {
        let path=get(change,&["path","file_path"]).unwrap_or(&name);
        let root=format!("{root}/{}",name.replace('~',"~0").replace('/',"~1"));
        let detail=["unified_diff","unifiedDiff","diff","content"].into_iter().find_map(|field| {
            refs.get(&format!("{root}/{field}")).map(|(reference,_)|(reference.clone(),field))
        });
        let change_type=get(change,&["type","kind"]).or_else(||change.pointer("/kind/type").and_then(Value::as_str));
        state.files.entry(key.clone()).or_default().insert((record_start,index as u64),super::store::FileRow {path:preview(path,1024),
            change:change_type.map(str::to_owned),kind:detail.as_ref().map(|d|if d.1=="content" {"text"} else {"diff"}.to_owned()),detail:detail.map(|d|d.0)});
    }
}

/// Mark the records on the active branch of a tree recording (Claude, Pi, OMP): from the last record
/// with an id, or the leaf a rewind names, up through the parents. Records without an id stay active.
pub(super) fn select_branch(state:&mut State,kind:SessionKind,source:&RegisteredSource) -> Result<(),String> {
    if !matches!(kind,SessionKind::Claude|SessionKind::Pi|SessionKind::Omp) {return Ok(());}
    let latest=state.records.values().rev().find(|record|record.native.is_some() || record.rewound);
    let Some(latest)=latest else {return Ok(());};
    let leaf:Option<Box<str>>=if latest.rewound {latest.leaf.clone()} else {latest.native.clone()};
    let mut chain:HashSet<Box<str>>=HashSet::new();
    if let Some(leaf)=&leaf {
        let mut pending=vec![leaf.clone()];let mut root=false;let mut missing=false;
        while let Some(id)=pending.pop() {
            if !chain.insert(id.clone()) {continue;}
            if chain.len()>100_000 {break;}
            let Some(starts)=state.identities.get(&id) else {missing=true;continue;};
            for start in starts {
                let Some(record)=state.records.get(start) else {continue;};
                match state.parent_of(*start,record) {Some(parent)=>pending.push(parent),None=>root=true}
            }
        }
        if chain.len()>100_000 || !root || missing {return Err("index_incomplete: native history branch is cyclic, too deep, or missing an ancestor".into());}
    }
    if super::children::is_claude_child(source) {
        // A subagent recording continues parallel tool calls on records whose parent is on the
        // branch: further records of the same streamed message, and the tool results restored after them.
        let mut children:HashMap<&str,Vec<u64>>=HashMap::new();
        for (start,record) in &state.records {if let (Some(parent),Some(_))=(&record.parent,&record.native) {children.entry(parent).or_default().push(*start);}}
        let mut restored:HashMap<Box<str>,bool>=chain.iter().map(|id|(id.clone(),false)).collect();
        let mut pending=restored.iter().map(|(id,flag)|(id.clone(),*flag)).collect::<Vec<_>>();
        while let Some((id,flag))=pending.pop() {
            let parent_message=state.identities.get(&id).and_then(|starts|starts.iter().find_map(|start|state.records.get(start).and_then(|record|record.message.clone())));
            for start in children.get(&*id).into_iter().flatten() {
                let record=&state.records[start];let native=record.native.clone().unwrap();
                let same_message=record.event.as_deref()==Some("assistant") && record.message.is_some() && record.message==parent_message;
                let restore=flag && (record.event.as_deref()==Some("attachment") || record.event.as_deref()==Some("user") && record.results);
                // Every record reached this way is a restored continuation, whichever rule reached it.
                if !(same_message || restore) || restored.get(&native)==Some(&true) {continue;}
                restored.insert(native.clone(),true);pending.push((native,true));
            }
        }
        chain=restored.into_keys().collect();
    }
    let removed=state.records.values().any(|record|record.active && record.native.as_ref().is_some_and(|native|!chain.contains(native)));
    if removed {state.branch_revision=uuid::Uuid::new_v4().to_string();}
    for record in state.records.values_mut() {record.active=record.native.as_ref().is_none_or(|native|chain.contains(native));}
    Ok(())
}
