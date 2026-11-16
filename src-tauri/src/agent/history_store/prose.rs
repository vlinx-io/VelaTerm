//! Fixed-snapshot prose messages. Range selection precedes reading any original message body.

use std::io::Write;
use std::sync::atomic::{AtomicBool,Ordering};
use serde_json::{json,Value};
use crate::{agent::transcript,models::SessionKind};
use super::{api::Branch,event_stream::{Cursor,Piece,Text},store::FieldMeta};

#[derive(Clone,Debug)]
pub(super) struct Message {pub role:&'static str,pub timestamp:Option<String>,pub row_id:Option<String>}

/// One fragment of a message body: original fields read in full, a literal verified against the
/// fields it came from, or an OpenCode multipart user message.
#[derive(Clone,Debug)]
pub(super) struct Fragment {
    pub source:usize,
    pub fields:Vec<FieldMeta>,
    pub literal:Option<String>,
    pub native_user:Option<String>,
    pub trim:bool,
    pub separator:&'static str,
    /// What joins this fragment to the one before it.
    pub join:&'static str,
    /// Classification fields of the fragment's record, read again before its body is supplied.
    pub proofs:Vec<FieldMeta>,
}

pub struct Prose {
    pub(super) cursor:Cursor,
    pub total:u64,
    pub(super) messages:Vec<Message>,
    pub(super) fragments:Vec<Vec<Fragment>>,
    pub(super) tools:Vec<Vec<String>>,
}

pub fn supported(kind:SessionKind) -> bool {matches!(kind,SessionKind::Claude|SessionKind::Codex|SessionKind::Pi|SessionKind::Omp|SessionKind::Opencode|SessionKind::Deepseek|SessionKind::Kiro|SessionKind::Grok|SessionKind::Antigravity)}

impl Prose {
    pub fn new(branch:Branch,cancel:&AtomicBool) -> Result<Self,String> {
        let _work=super::work::Work::acquire(cancel)?;
        Self::build(Cursor::new(branch,cancel)?,cancel)
    }

    fn message(&self,number:u64) -> Option<&Message> {self.messages.get(number as usize)}
    fn index(&self,number:u64) -> Option<usize> {(number<self.total).then_some(number as usize)}

    fn build(mut cursor:Cursor,cancel:&AtomicBool) -> Result<Self,String> {
        if cursor.branch.kind.uses_opencode()||cursor.branch.kind==SessionKind::Kiro {return Self::native(cursor,cancel);}
        let kind=cursor.branch.kind;
        let mut total=0;let mut role="";let mut workflow=false;let mut last_tool=None;
        let mut messages:Vec<Message>=Vec::new();let mut fragments:Vec<Vec<Fragment>>=Vec::new();let mut tools:Vec<Vec<String>>=Vec::new();
        let join=if kind==SessionKind::Grok {""} else {"\n\n"};
        while let Some(indexed)=cursor.next_record(cancel)? {
            let record=&indexed.record;let raw=record.value.to_string();
            let source=cursor.branch.sources.iter().position(|source|source.id==indexed.source.id && source.generation==indexed.source.generation)
                .ok_or("source_changed: prose source is outside the fixed snapshot")?;
            // Rows of this record in the view, and the Antigravity user request it shows.
            let (rows,visible)={
                let state=indexed.source.current()?;
                let rows=state.record_rows.get(&record.start).map(|ids|ids.iter().filter_map(|id| {
                    let summary=state.parts.get(id)?.get(&record.start)?;
                    let value:Value=serde_json::from_str(&summary.summary).ok()?;Some((id.to_string(),value))
                }).collect::<Vec<_>>()).unwrap_or_default();
                let visible=if kind==SessionKind::Antigravity {
                    rows.iter().find(|(_,value)|value["kind"]=="user").map(|(id,value)| {
                        let field=state.row_fields.get(id.as_str()).and_then(|fields|fields.iter().find(|field|field.label=="body"))
                            .and_then(|field|state.fields.get(&field.reference).cloned());
                        (value["text"].as_str().unwrap_or_default().to_owned(),field)
                    })
                } else {None};
                (rows,visible)
            };
            let pieces=match kind {
                SessionKind::Codex=>transcript::parse_codex_line(&raw).into_iter().collect::<Vec<_>>(),
                SessionKind::Claude=>transcript::parse_claude_line(&raw).into_iter().collect::<Vec<_>>(),
                SessionKind::Pi|SessionKind::Omp=>transcript::pi_pieces(&raw),
                SessionKind::Grok=>transcript::grok_pieces(&record.value),
                SessionKind::Antigravity=>{
                    let mut value=record.value.clone();
                    if value["type"]=="USER_INPUT" {value["content"]=json!(visible.as_ref().map(|(text,_)|text.clone()).unwrap_or_default());}
                    transcript::antigravity_pieces(&value.to_string())
                },
                _=>return Err("unsupported_record: streamed prose is unavailable for this provider".into()),
            };
            let mut assistant_text=0;
            // Text from a record the conversation view leaves out (Codex's injected AGENTS.md
            // instructions, for one) has no row through which its fields could be opened.
            let shown=pieces.is_empty() || !rows.is_empty();
            for piece in pieces {
                if !shown && !piece.text.is_empty() {continue;}
                let boundary=kind!=SessionKind::Grok && piece.role=="user" && (workflow || piece.text.starts_with("[VelaTerm message "));
                if role!=piece.role || boundary {
                    total+=1;role=piece.role;workflow=piece.text.starts_with("[VelaTerm message ");last_tool=None;
                    let mut candidates=rows.iter().map(|(id,value)|(value["kind"]!=role,id.clone())).collect::<Vec<_>>();candidates.sort();
                    messages.push(Message {role,timestamp:piece.timestamp.clone(),row_id:candidates.into_iter().next().map(|(_,id)|id)});
                    fragments.push(Vec::new());tools.push(Vec::new());
                }
                if !piece.text.is_empty() {
                    let mut paths=super::adapter::body_paths(record,"body",None,0);
                    // Claude and Pi can place several assistant blocks in one physical row. Select
                    // only communicated text, preserving their existing blank-line separators.
                    if role=="assistant" && matches!(kind,SessionKind::Claude|SessionKind::Pi|SessionKind::Omp) {
                        paths=record.fields.iter().filter(|field|field.encoding=="string" && field.path.starts_with("/message/content/")
                            && field.path.ends_with("/text")).map(|field|field.path.clone()).collect();
                        if matches!(kind,SessionKind::Pi|SessionKind::Omp) {paths=paths.into_iter().nth(assistant_text).into_iter().collect();assistant_text+=1;}
                    }
                    let mut fields=paths.iter().filter_map(|path|indexed.references.get(path).map(|(_,meta)|meta.clone())).collect::<Vec<_>>();
                    let mut long=record.fields.iter().any(|field|paths.contains(&field.path) && field.truncated);
                    if kind==SessionKind::Antigravity {
                        if role=="user" {fields=visible.as_ref().and_then(|(_,field)|field.clone()).into_iter().collect();long=true;}
                        else {fields=indexed.references.get("/content").map(|(_,meta)|meta.clone()).into_iter().collect();paths=vec!["/content".into()];
                            long=record.fields.iter().any(|field|field.path=="/content" && field.truncated);}
                    }
                    let proofs=record.fields.iter().filter(|field|super::projection::identity_path(&field.path))
                        .filter_map(|field|indexed.references.get(&field.path).map(|(_,meta)|meta.clone())).collect::<Vec<_>>();
                    let fragment=if long {
                        if fields.is_empty() {return Err("unsupported_record: original prose fields are unavailable".into());}
                        Fragment {source,fields,literal:None,native_user:None,trim:!matches!(kind,SessionKind::Grok|SessionKind::Antigravity),
                            separator:if kind==SessionKind::Grok {""} else if role=="assistant" && kind!=SessionKind::Codex {"\n\n"} else {"\n"},join,proofs}
                    } else {Fragment {source,fields,literal:Some(piece.text.clone()),native_user:None,trim:false,separator:"\n",join,proofs}};
                    let _=paths;
                    fragments.last_mut().ok_or("index_incomplete: prose fragment precedes its message")?.push(fragment);
                }
                for tool in piece.tools {
                    if last_tool.as_ref()!=Some(&tool) {
                        tools.last_mut().ok_or("index_incomplete: prose tool precedes its message")?.push(tool.clone());last_tool=Some(tool);
                    }
                }
            }
        }
        let mut prose=Self {cursor,total,messages,fragments,tools};
        if kind==SessionKind::Grok {prose.compact_grok(cancel)?;}
        Ok(prose)
    }

    fn compact_grok(&mut self,cancel:&AtomicBool) -> Result<(),String> {
        struct Meaningful(bool);
        impl Write for Meaningful {
            fn write(&mut self,bytes:&[u8]) -> std::io::Result<usize> {
                self.0|=std::str::from_utf8(bytes).map_err(std::io::Error::other)?.chars().any(|ch|!ch.is_whitespace());Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {Ok(())}
        }
        let mut keep=Vec::new();
        for number in 0..self.total {
            let mut text=Meaningful(false);self.body(number,cancel,&mut text)?;
            keep.push(text.0 || !self.tools[number as usize].is_empty());
        }
        let mut index=0;
        self.messages.retain(|_|{let kept=keep[index];index+=1;kept});index=0;
        self.fragments.retain(|_|{let kept=keep[index];index+=1;kept});index=0;
        self.tools.retain(|_|{let kept=keep[index];index+=1;kept});
        self.total=self.messages.len() as u64;Ok(())
    }

    fn native(mut cursor:Cursor,cancel:&AtomicBool) -> Result<Self,String> {
        use crate::agent::export::Event;
        let kind=cursor.branch.kind;
        let mut role="";let mut workflow=false;let mut last_tool=None;
        let mut messages:Vec<Message>=Vec::new();let mut fragments:Vec<Vec<Fragment>>=Vec::new();let mut tools:Vec<Vec<String>>=Vec::new();
        while let Some(item)=cursor.next(cancel)? {
            let (next,text,timestamp,tool)=match &item.event {
                Event::User {text,ts}=>("user",Some(text),ts,None),
                Event::AssistantText {text,ts}=>("assistant",Some(text),ts,None),
                Event::ToolUse {name,ts,..}=>("assistant",None,ts,Some(name)),
                _=>continue,
            };
            let workflow_body=text.is_some_and(|text|text.starts_with("[VelaTerm message "));
            if role!=next || kind==SessionKind::Kiro && text.is_some() || next=="user" && (workflow || workflow_body) {
                role=next;workflow=workflow_body;last_tool=None;
                messages.push(Message {role,timestamp:timestamp.clone(),row_id:item.row_id.clone()});fragments.push(Vec::new());tools.push(Vec::new());
            }
            if text.is_some_and(|text|!text.is_empty()) || kind==SessionKind::Kiro && text.is_some()
                || item.body.pieces.iter().any(|piece|matches!(piece,Piece::NativeUser(..))) {
                let mut source=0;let mut fields=Vec::new();let mut literal=None;let mut native_user=None;
                for piece in &item.body.pieces {
                    let owner=match piece {Piece::Field(owner,_)|Piece::NativeUser(owner,_)|Piece::VerifiedLiteral(owner,..)=>Some(owner),_=>None};
                    if let Some(owner)=owner {source=cursor.branch.sources.iter().position(|candidate|candidate.id==owner.id && candidate.generation==owner.generation)
                        .ok_or("source_changed: native prose source is unavailable")?;}
                    match piece {
                        Piece::Field(_,field)=>fields.push(field.clone()),
                        Piece::NativeUser(_,message)=>native_user=Some(message.clone()),
                        Piece::Literal(text)=>literal=Some(text.clone()),
                        Piece::VerifiedLiteral(_,verified,text)=>{fields.extend(verified.iter().cloned());literal=Some(text.clone());},
                    }
                }
                fragments.last_mut().ok_or("index_incomplete: prose fragment precedes its message")?.push(Fragment {source,fields,literal,native_user,trim:false,
                    separator:item.body.separator,join:"\n\n",proofs:Vec::new()});
            }
            if let Some(tool)=tool {if last_tool.as_ref()!=Some(tool) {
                tools.last_mut().ok_or("index_incomplete: prose tool precedes its message")?.push(tool.clone());last_tool=Some(tool.clone());
            }}
        }
        let total=messages.len() as u64;
        Ok(Self {cursor,total,messages,fragments,tools})
    }

    pub fn row_id(&self,number:u64) -> Result<Option<String>,String> {Ok(self.message(number).and_then(|message|message.row_id.clone()))}

    pub fn metadata(&self,number:u64) -> Result<Option<Value>,String> {
        Ok(self.message(number).map(|message|json!({"messageIndex":number,"role":message.role,"timestamp":message.timestamp,"rowId":message.row_id})))
    }

    pub(super) fn tools_of(&self,number:u64) -> &[String] {self.index(number).map_or(&[],|index|&self.tools[index])}
    pub(super) fn fragments_of(&self,number:u64) -> &[Fragment] {self.index(number).map_or(&[],|index|&self.fragments[index])}

    pub fn memory_source(&self,max_chars:usize,cancel:&AtomicBool) -> Result<Option<String>,String> {
        let mut writer=Characters {text:String::new(),chars:0,limit:max_chars};let mut meaningful=false;
        for number in 0..self.total {
            let meta=self.metadata(number)?.ok_or("source_changed: message metadata changed")?;
            if number>0 {writer.write_all(b"\n").map_err(|e|e.to_string())?;}
            write!(writer,"## Message {} · {} · {}\n\n",number+1,meta["role"].as_str().unwrap_or_default(),meta["timestamp"].as_str().unwrap_or_default()).map_err(|e|e.to_string())?;
            let before=writer.text.len();self.body(number,cancel,&mut writer)?;
            meaningful|=!writer.text[before..].trim().is_empty();writer.write_all(b"\n").map_err(|e|e.to_string())?;
        }
        Ok(meaningful.then_some(writer.text))
    }

    pub(crate) fn body(&self,number:u64,cancel:&AtomicBool,writer:&mut impl Write) -> Result<(),String> {
        let _work=super::work::Work::acquire(cancel)?;
        let mut first=true;
        for fragment in self.fragments_of(number) {
            if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
            if !first {writer.write_all(fragment.join.as_bytes()).map_err(|e|e.to_string())?;}first=false;
            let source=self.cursor.branch.sources.get(fragment.source).ok_or("Invalid prose source")?;
            for proof in &fragment.proofs {super::fields::walk(source,proof,cancel,&mut |_|Ok(()))?;}
            if let Some(message)=&fragment.native_user {
                super::opencode::walk_user(source,message,cancel,&mut |text|writer.write_all(text.as_bytes()).map_err(|e|e.to_string()))?;continue;
            }
            let text=Text {pieces:fragment.fields.iter().map(|field|Piece::Field(source.clone(),field.clone())).collect(),quote_json:false,separator:fragment.separator};
            if let Some(literal)=&fragment.literal {
                // Short transformed text is cached, but its contributing bytes are still verified
                // against the actual source before it is supplied as original conversation evidence.
                text.walk(&self.cursor.branch,cancel,&mut |_|Ok(()))?;
                writer.write_all(literal.as_bytes()).map_err(|e|e.to_string())?;continue;
            }
            text.write(&self.cursor.branch,cancel,writer,fragment.trim)?;
        }
        Ok(())
    }

    pub fn write_messages(&self,start:u64,end:u64,cancel:&AtomicBool,writer:&mut impl Write) -> Result<(),String> {
        writer.write_all(b"[").map_err(|e|e.to_string())?;
        for number in start.min(self.total)..end.min(self.total).max(start.min(self.total)) {
            if cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
            if number>start {writer.write_all(b",").map_err(|e|e.to_string())?;}
            let message=self.message(number).ok_or("source_changed: message metadata changed")?;
            writer.write_all(b"{\"role\":").map_err(|e|e.to_string())?;serde_json::to_writer(&mut *writer,&message.role).map_err(|e|e.to_string())?;
            writer.write_all(b",\"timestamp\":").map_err(|e|e.to_string())?;serde_json::to_writer(&mut *writer,&message.timestamp).map_err(|e|e.to_string())?;
            writer.write_all(b",\"tools\":").map_err(|e|e.to_string())?;serde_json::to_writer(&mut *writer,self.tools_of(number)).map_err(|e|e.to_string())?;
            writer.write_all(b",\"text\":\"").map_err(|e|e.to_string())?;
            self.body(number,cancel,&mut JsonString {writer})?;
            writer.write_all(b"\"}").map_err(|e|e.to_string())?;
        }
        writer.write_all(b"]").map_err(|e|e.to_string())?;Ok(())
    }

    pub fn collect_bounded(&self,start:u64,end:u64,max_bytes:usize,cancel:&AtomicBool) -> Result<Vec<transcript::TranscriptMessage>,String> {
        let mut writer=Budget {bytes:Vec::new(),limit:max_bytes};self.write_messages(start,end,cancel,&mut writer)?;
        serde_json::from_slice(&writer.bytes).map_err(|e|e.to_string())
    }
}

struct JsonString<'a,W:Write> {writer:&'a mut W}
impl<W:Write> Write for JsonString<'_,W> {
    fn write(&mut self,bytes:&[u8]) -> std::io::Result<usize> {
        let text=std::str::from_utf8(bytes).map_err(std::io::Error::other)?;
        let value=serde_json::to_string(text).map_err(std::io::Error::other)?;
        self.writer.write_all(value[1..value.len()-1].as_bytes())?;Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {self.writer.flush()}
}

struct Budget {bytes:Vec<u8>,limit:usize}
impl Write for Budget {
    fn write(&mut self,bytes:&[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len())>self.limit {return Err(std::io::Error::other(super::compat::UPGRADE));}
        self.bytes.extend_from_slice(bytes);Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {Ok(())}
}

struct Characters {text:String,chars:usize,limit:usize}
impl Write for Characters {
    fn write(&mut self,bytes:&[u8]) -> std::io::Result<usize> {
        let text=std::str::from_utf8(bytes).map_err(std::io::Error::other)?;let count=text.chars().count();
        if count>self.limit.saturating_sub(self.chars) {return Err(std::io::Error::other("memory_source_too_large"));}
        self.text.push_str(text);self.chars+=count;Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {Ok(())}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grok_chunks_and_antigravity_visible_requests_preserve_original_prose_semantics() {
        let cancel=AtomicBool::new(false);let temp=super::super::tests::Temp::new();
        let records=[json!({"method":"session/update","timestamp":1,"params":{"update":{"sessionUpdate":"user_message_chunk","content":{"text":"  "}}}}),
            json!({"method":"session/update","timestamp":2,"params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"text":"first"}}}}),
            json!({"method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"text":" second\n"}}}}),
            json!({"method":"session/update","params":{"update":{"sessionUpdate":"tool_call","_meta":{"x.ai/tool":{"name":"Read"}}}}}),
            json!({"method":"session/update","params":{"update":{"sessionUpdate":"user_message_chunk","content":{"text":"[VelaTerm message one]"}}}}),
            json!({"method":"session/update","params":{"update":{"sessionUpdate":"user_message_chunk","content":{"text":"[VelaTerm message two]"}}}})];
        let raw=records.iter().map(|record|record.to_string()+"\n").collect::<String>();let branch=super::super::tests::indexed_kind(&temp,&raw,SessionKind::Grok);
        let expected=transcript::read_at(SessionKind::Grok,&branch.sources[0].segment.path).unwrap();let prose=Prose::new(branch,&cancel).unwrap();
        assert_eq!(prose.collect_bounded(0,prose.total,512*1024,&cancel).unwrap(),expected);assert_eq!(prose.total,2);
        let temp=super::super::tests::Temp::new();let request="中文 original\n".repeat(5000);let answer=" \n".to_owned()+&"response\n".repeat(5000)+" \t";
        let raw=[json!({"type":"USER_INPUT","step_index":0,"content":format!("{}<USER_REQUEST> {request} </USER_REQUEST>private tail","private head ".repeat(1000))}),
            json!({"type":"PLANNER_RESPONSE","step_index":1,"content":answer,"thinking":"private reasoning","tool_calls":[{"name":"view_file","args":{}}]})]
            .iter().map(|record|record.to_string()+"\n").collect::<String>();
        let branch=super::super::tests::indexed_kind(&temp,&raw,SessionKind::Antigravity);let expected=transcript::read_at(SessionKind::Antigravity,&branch.sources[0].segment.path).unwrap();
        let prose=Prose::new(branch,&cancel).unwrap();assert_eq!(prose.collect_bounded(0,prose.total,512*1024,&cancel).unwrap(),expected);
    }
    #[test]
    fn exhausted_previews_keep_late_text_and_injected_context_visibility() {
        for kind in [SessionKind::Claude,SessionKind::Codex,SessionKind::Pi] {
            let temp=super::super::tests::Temp::new();let noise=json!((0..20).map(|_|json!({"text":"noise".repeat(2000)})).collect::<Vec<_>>()).to_string();
            let body=" ".repeat(12_000)+"Late communicated original";
            let value=match kind {
                SessionKind::Claude=>json!({"type":"assistant","uuid":"one","message":{"content":[{"type":"text","text":body}]}}),
                SessionKind::Pi=>json!({"type":"message","id":"one","message":{"role":"assistant","content":[{"type":"text","text":body}]}}),
                _=>json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":body}]}}),
            };
            let raw=format!("{{\"noise\":{noise},{}\n",&value.to_string()[1..]);
            let branch=super::super::tests::indexed_kind(&temp,&raw,kind);
            assert_eq!(branch.page(None,None,None).unwrap()["rows"].as_array().unwrap().iter().filter(|row|row["kind"]=="assistant").count(),1,"{kind:?}");
            let prose=Prose::new(branch,&AtomicBool::new(false)).unwrap();assert_eq!(prose.total,1,"{kind:?}");
            assert_eq!(prose.collect_bounded(0,1,64*1024,&AtomicBool::new(false)).unwrap()[0].text,"Late communicated original","{kind:?}");
            if kind!=SessionKind::Pi {
                let temp=super::super::tests::Temp::new();let mut value=value;
                let content=if kind==SessionKind::Claude {value["type"]=json!("user");&mut value["message"]["content"][0]} else {value["payload"]["role"]=json!("user");&mut value["payload"]["content"][0]};
                content["text"]=json!(" ".repeat(12_000)+if kind==SessionKind::Claude {"<system-reminder>injected</system-reminder>"} else {"<environment_context>injected</environment_context>"});
                let raw=format!("{{\"noise\":{noise},{}\n",&value.to_string()[1..]);let branch=super::super::tests::indexed_kind(&temp,&raw,kind);
                assert_eq!(Prose::new(branch,&AtomicBool::new(false)).unwrap().total,0,"{kind:?}");
            }
        }
    }
    #[test]
    fn codex_instructions_hidden_from_the_conversation_stay_out_of_prose() {
        let temp=super::super::tests::Temp::new();
        let message=|role:&str,text:&str| json!({"type":"response_item","payload":{"type":"message","role":role,
            "content":[{"type":if role=="user" {"input_text"} else {"output_text"},"text":text}]}}).to_string()+"\n";
        let raw=message("user","# AGENTS.md instructions\n\n<INSTRUCTIONS>rules</INSTRUCTIONS>")+&message("user","Where is the config?")+&message("assistant","In settings.");
        let branch=super::super::tests::indexed(&temp,&raw);
        assert_eq!(branch.page(None,None,None).unwrap()["rows"].as_array().unwrap().len(),2);
        let cancel=AtomicBool::new(false);let prose=Prose::new(branch,&cancel).unwrap();assert_eq!(prose.total,2);
        let texts=prose.collect_bounded(0,2,64*1024,&cancel).unwrap().into_iter().map(|message|message.text).collect::<Vec<_>>();
        assert_eq!(texts,["Where is the config?","In settings."]);
    }
    #[test]
    fn memory_material_preserves_the_existing_character_limit_and_full_original_text() {
        let temp=super::super::tests::Temp::new();let text="雪😀".repeat(100_000);
        let raw=json!({"type":"response_item","timestamp":"t1","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":text}]}}).to_string()+"\n";
        let branch=super::super::tests::indexed(&temp,&raw);let path=branch.sources[0].segment.path.clone();let cancel=AtomicBool::new(false);
        assert_eq!(super::super::compat::read_path(&path).unwrap_err(),super::super::compat::UPGRADE);
        let prose=Prose::new(branch,&cancel).unwrap();
        let expected=format!("## Message 1 · user · t1\n\n{text}\n");
        assert_eq!(prose.memory_source(expected.chars().count(),&cancel).unwrap().unwrap(),expected);
        assert_eq!(prose.memory_source(expected.chars().count()-1,&cancel).unwrap_err(),"memory_source_too_large");
    }
    #[test]
    fn prose_matches_existing_merge_and_selects_range_before_long_body_reads() {
        let temp=super::super::tests::Temp::new();let long="雪😀\n".repeat(20_000);let mut raw=String::new();
        for value in [json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"First"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":long}]}}),
            json!({"type":"response_item","payload":{"type":"function_call","name":"Read","call_id":"one","arguments":"{}"}}),
            json!({"type":"response_item","payload":{"type":"function_call","name":"Read","call_id":"two","arguments":"{}"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Last"}]}})] {raw.push_str(&value.to_string());raw.push('\n');}
        let branch=super::super::tests::indexed(&temp,&raw);let path=branch.sources[0].segment.path.clone();let cancel=AtomicBool::new(false);
        let prose=Prose::new(branch,&cancel).unwrap();assert_eq!(prose.total,3);
        let all=prose.collect_bounded(0,3,512*1024,&cancel).unwrap();assert_eq!(all,transcript::read_at(SessionKind::Codex,&path).unwrap());
        assert_eq!(all[1].text,long.trim_end());assert_eq!(all[1].tools,["Read"]);
        // Break an earlier original after metadata selection. Reading the last message must never
        // touch the unrelated giant body, while requesting that body detects the changed bytes.
        let original=std::fs::read_to_string(&path).unwrap();std::fs::write(&path,original.replace("雪","雲")).unwrap();
        assert_eq!(prose.collect_bounded(2,3,1024,&cancel).unwrap()[0].text,"Last");
        assert!(prose.collect_bounded(1,2,512*1024,&cancel).unwrap_err().contains("source_changed"));
    }
}
