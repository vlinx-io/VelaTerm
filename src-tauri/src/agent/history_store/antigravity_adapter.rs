//! Native step pairing stays in the derived index. User-request ranges exclude runtime annotations.

use std::io::Read;
use serde_json::{Value,json};

use super::adapter::{attach,preview,short_input,write_row,Refs,Scanned};
use super::limits::{INPUT_BYTES,PREVIEW_BYTES};
use super::projection::Record;
use super::store::{FieldMeta,State,digest};

/// The decoded offset of the first `marker` at or after `start` in a string field being scanned.
fn find(scanned:&Scanned,field:&FieldMeta,start:u64,marker:&str) -> Result<Option<u64>,String> {
    let mut reader=scanned.text(field)?;
    let mut bytes=vec![0;INPUT_BYTES];let mut carry:Vec<u8>=Vec::new();let mut at=0u64;
    loop {
        let count=reader.read(&mut bytes).map_err(|error|error.to_string())?;
        if count==0 {return Ok(None);}
        let base=at-carry.len() as u64;carry.extend_from_slice(&bytes[..count]);at+=count as u64;
        let mut offset=0;
        while let Some(found)=carry[offset..].windows(marker.len()).position(|window|window==marker.as_bytes()) {
            let position=base+(offset+found) as u64;
            if position>=start {return Ok(Some(position));}
            offset+=found+1;
        }
        let kept=(marker.len()-1).min(carry.len());carry.drain(..carry.len()-kept);
    }
}

fn visible_user(scanned:&Scanned,record:&Record,refs:&Refs) -> Result<Option<(String,String,u64,FieldMeta)>,String> {
    let Some((_,content))=refs.get("/content").cloned() else {return Ok(None);};
    if content.encoding!="string" || content.decoded_bytes==0 {return Ok(None);}
    let (start,end)=if let Some(open)=find(scanned,&content,0,"<USER_REQUEST>")? {
        let start=open+"<USER_REQUEST>".len() as u64;
        let Some(end)=find(scanned,&content,start,"</USER_REQUEST>")? else {return Ok(None);};
        (start,end)
    } else {(0,content.decoded_bytes)};
    if start==end {return Ok(None);}
    let mut reader=scanned.text(&content)?;
    let mut buffer=vec![0;INPUT_BYTES];let mut at=0u64;let mut first=None;let mut last=start;let mut text=String::new();let mut preview_full=false;
    let mut pending:Vec<u8>=Vec::new();
    loop {
        let count=reader.read(&mut buffer).map_err(|error|error.to_string())?;
        if count==0 {break;}
        pending.extend_from_slice(&buffer[..count]);
        let valid=match std::str::from_utf8(&pending) {Ok(_)=>pending.len(),Err(error)=>error.valid_up_to()};
        let chunk=std::str::from_utf8(&pending[..valid]).unwrap().to_owned();pending.drain(..valid);
        for (offset,ch) in chunk.char_indices() {
            let position=at+offset as u64;
            if position<start || position>=end {continue;}
            if !ch.is_whitespace() {first.get_or_insert(position);last=position+ch.len_utf8() as u64;}
            if first.is_some() && !preview_full {
                if text.len()+ch.len_utf8()<=PREVIEW_BYTES {text.push(ch);} else {preview_full=true;}
            }
        }
        at+=valid as u64;
        if at>=end {break;}
    }
    let Some(first)=first else {return Ok(None);};
    // A range inside the decoded parent string. The original content reference is never attached to a
    // user row, so metadata around the request cannot leak into search or downloads.
    let path="@visible/content";let source=scanned.source;
    let sliced=digest(format!("{}:{}:{}:{path}:{first}:{last}",source.id,source.generation,record.start).as_bytes());
    let meta=FieldMeta {record_start:record.start,path:path.into(),raw_start:first,raw_end:last,decoded_bytes:last-first,parent:Some(Box::new(content)),
        encoding:"slice".into(),hash:None};
    if last-first<=PREVIEW_BYTES as u64 {text.truncate(((last-first) as usize).min(text.len()));}
    Ok(Some((sliced,text,last-first,meta)))
}

fn paths(record:&Record,root:&str) -> Vec<String> {
    record.fields.iter().filter(|field|field.path==root || field.path.starts_with(&format!("{root}/"))).map(|field|field.path.clone()).collect()
}

pub(super) fn store(state:&mut State,scanned:&Scanned,record:&Record,refs:&Refs) -> Result<(),String> {
    let value=&record.value;let source=scanned.source;
    let Some(step)=value["step_index"].as_u64() else {return Ok(());};
    // A process crash or re-initialization can restart step_index from 0 inside the same transcript.
    // Advance the segment on any strict decrease so later rows append instead of overwriting earlier ones.
    if state.agy_last_step.is_some_and(|prev|step<prev) {
        state.agy_segment+=1;
        for (_,(_,active)) in state.pending.range_mut(..(record.start,0)) {*active=false;}
    }
    state.agy_last_step=Some(step);
    let id=if state.agy_segment==0 {
        format!("native:{}",digest(format!("antigravity:{}:{step}",source.segment.native_id).as_bytes()))
    } else {
        format!("native:{}",digest(format!("antigravity:{}:s{}:{step}",source.segment.native_id,state.agy_segment).as_bytes()))
    };
    let at=crate::agent::chat::history::parsed_at(value["created_at"].as_str());
    match value["type"].as_str() {
        Some("USER_INPUT")=>{
            for (_,(_,active)) in state.pending.range_mut(..(record.start,0)) {*active=false;}
            if let Some((reference,text,size,meta))=visible_user(scanned,record,refs)? {
                let row=json!({"kind":"user","id":id,"text":text,"at":at,"bodyAvailable":size>PREVIEW_BYTES as u64});
                write_row(state,scanned,record,0,None,&row,refs);
                state.attach(&id,"body",&[(reference,meta)]);
            }
        },
        Some("PLANNER_RESPONSE")=>{
            for (local,path,kind) in [(0,"/thinking","reasoning"),(1,"/content","assistant")] {
                if let Some(text)=value.pointer(path).and_then(Value::as_str).filter(|text|!text.is_empty()) {
                    let row_id=if kind=="reasoning" {format!("{id}-thinking")} else {id.clone()};
                    let long=record.fields.iter().any(|field|field.path==path && field.truncated);
                    let row=json!({"kind":kind,"id":row_id,"text":preview(text,PREVIEW_BYTES),"streaming":value["status"]!="DONE","at":at,"bodyAvailable":long});
                    write_row(state,scanned,record,local,None,&row,refs);attach(state,&row_id,"body",&[path.into()],refs);
                }
            }
            for (index,call) in value["tool_calls"].as_array().into_iter().flatten().enumerate() {
                let Some(name)=call["name"].as_str() else {continue;};
                let row_id=format!("{id}-call-{index}");
                let (name,input)=crate::agent::chat::antigravity_protocol::tool_input(name,&call["args"]);
                let row=json!({"kind":"tool","id":row_id,"name":name,"input":short_input(&input),"status":"running","isError":false,"detailAvailable":true});
                write_row(state,scanned,record,index+2,Some(&row_id),&row,refs);
                attach(state,&row_id,"input",&paths(record,&format!("/tool_calls/{index}/args")),refs);
                state.pending.entry((record.start,index as u64)).or_insert((row_id,true));
            }
        },
        Some("ERROR_MESSAGE"|"GENERIC"|"LIST_DIRECTORY"|"VIEW_FILE"|"CODE_ACTION"|"ASK_QUESTION")=>{
            let pending=state.pending.range(..(record.start,0)).find(|(_,(_,active))|*active).map(|(key,(row,_))|(*key,row.clone()));
            if let Some((key,row_id))=pending {
                let error=value["type"]=="ERROR_MESSAGE";
                let content=value["content"].as_str().unwrap_or_default();
                let limit=if value["type"]=="ASK_QUESTION" {2048} else {512};
                let mut row=json!({"kind":"tool","id":row_id,"name":"tool","input":{},"output":preview(content,limit),
                    "isError":error,"status":if error {"failed"} else if value["status"]=="DONE" {"completed"} else {"running"},"detailAvailable":true});
                if !error {
                    if let Some(call)=state.summary(&row_id).and_then(|summary|serde_json::from_str::<Value>(summary).ok()).filter(|value|value["name"]=="Agent") {
                        if let Some(child)=crate::agent::chat::antigravity_protocol::subagent_conversation_id(content).filter(|id|super::children::valid_id(id)) {
                            row["_child"]=json!({"id":child,"facts":{"description":call["input"]["description"]}});
                        }
                    }
                }
                write_row(state,scanned,record,0,Some(&row_id),&row,refs);attach(state,&row_id,"output",&["/content".into()],refs);
                if let Some(entry)=state.pending.get_mut(&key) {entry.1=false;}
            } else if value["type"]=="ERROR_MESSAGE" {
                let row=json!({"kind":"error","id":id,"message":preview(value["content"].as_str().unwrap_or_default(),PREVIEW_BYTES)});
                write_row(state,scanned,record,0,None,&row,refs);attach(state,&id,"body",&["/content".into()],refs);
            }
        },
        _=>{},
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SessionKind;

    fn indexed(temp:&super::super::tests::Temp,records:&[Value]) -> super::super::api::Branch {
        let path=temp.0.join("native.jsonl");
        let raw=records.iter().map(|record|record.to_string()+"\n").collect::<String>();std::fs::write(&path,raw).unwrap();
        let store=super::super::store::Store::at(&temp.0).unwrap();
        let source=store.register(SessionKind::Antigravity,super::super::source::Segment {path,native_id:"native".into(),end_byte:None,end_ordinal:None}).unwrap();
        store.scan(SessionKind::Antigravity,&source,&std::sync::atomic::AtomicBool::new(false),false).unwrap();
        super::super::api::Branch {store,sources:vec![source],generation:"branch".into(),kind:SessionKind::Antigravity,native_id:"native".into(),scope:"scope".into(),error:None,parent:None}
    }

    #[test]
    fn late_wrapped_user_request_excludes_annotations_from_preview_details_and_authorization() {
        let temp=super::super::tests::Temp::new();let request="中文 Ω🚀\n".repeat(15000);
        let content=format!("{}<USER_REQUEST> \n{request}\n </USER_REQUEST><ADDITIONAL_METADATA>private trailing</ADDITIONAL_METADATA>","private prefix ".repeat(5000));
        let branch=indexed(&temp,&[json!({"type":"USER_INPUT","step_index":0,"content":content})]);
        let page=branch.page(None,None,None).unwrap();assert_eq!(page["coverage"],"complete");
        let row=&page["rows"][0];assert_eq!(row["kind"],"user");assert_eq!(row["bodyAvailable"],true);
        assert!(request.starts_with(row["text"].as_str().unwrap()));assert!(!row.to_string().contains("private"));
        let manifest=branch.manifest(row["id"].as_str().unwrap(),0).unwrap();assert_eq!(manifest["fields"].as_array().unwrap().len(),1);
        let reference=manifest["fields"][0]["detailRef"].as_str().unwrap();let mut complete=Vec::new();
        branch.write_detail(reference,None,&mut complete,&std::sync::atomic::AtomicBool::new(false)).unwrap();
        assert_eq!(String::from_utf8(complete).unwrap(),request.trim());
        let (_,refs)=super::super::event_stream::read_record(&branch.sources[0],0).unwrap();let original=refs["/content"].0.clone();
        assert!(branch.detail(&original,None,0).unwrap_err().starts_with("detail_unavailable:"));
        assert!(branch.locate_detail(row["id"].as_str().unwrap(),&original,0).is_err());
    }

    #[test]
    fn native_tool_results_pair_across_records_without_changing_call_identity() {
        let temp=super::super::tests::Temp::new();
        let records=[json!({"type":"USER_INPUT","step_index":0,"content":"<USER_REQUEST>test</USER_REQUEST><ADDITIONAL_METADATA>private</ADDITIONAL_METADATA>"}),
            json!({"type":"PLANNER_RESPONSE","step_index":1,"tool_calls":[{"name":"view_file","args":{"AbsolutePath":"/tmp/test"}}]}),
            json!({"type":"GENERIC","step_index":2,"status":"DONE","content":"file contents"}),
            json!({"type":"PLANNER_RESPONSE","step_index":3,"status":"DONE","content":"answer","thinking":"reason"})];
        let branch=indexed(&temp,&records);let page=branch.page(None,None,None).unwrap();let rows=page["rows"].as_array().unwrap();
        assert_eq!(rows.len(),4);assert_eq!(rows[0]["text"],"test");assert_eq!(rows[1]["name"],"Read");assert_eq!(rows[1]["input"]["file_path"],"/tmp/test");
        assert_eq!(rows[1]["output"],"file contents");assert_eq!(rows[1]["status"],"completed");assert_eq!(rows[2]["text"],"reason");assert_eq!(rows[3]["text"],"answer");
        let manifest=branch.manifest(rows[1]["id"].as_str().unwrap(),0).unwrap();
        assert!(manifest["fields"].as_array().unwrap().iter().any(|field|field["label"]=="input"));
        assert!(manifest["fields"].as_array().unwrap().iter().any(|field|field["label"]=="output"));
        let pending=branch.sources[0].current().unwrap().pending.values().filter(|(_,active)|*active).count();assert_eq!(pending,0);
    }

    #[test]
    fn invoke_subagent_rows_expose_verified_child_association() {
        let temp=super::super::tests::Temp::new();
        let records=[
            json!({"type":"PLANNER_RESPONSE","step_index":0,"tool_calls":[{"name":"invoke_subagent","args":{"Role":"Researcher","TypeName":"research","Prompt":"Inspect engine"}}]}),
            json!({"type":"GENERIC","step_index":1,"status":"DONE","content":"Subagent started: {\"conversationId\":\"child-conv-42\"}"}),
        ];
        let branch=indexed(&temp,&records);
        let page=branch.page(None,None,None).unwrap();
        let rows=page["rows"].as_array().unwrap();
        assert_eq!(rows.len(),1);
        assert_eq!(rows[0]["name"],"Agent");
        assert_eq!(rows[0]["childAvailable"],true);
        assert_eq!(rows[0]["subagent"]["description"],"Researcher");
        assert!(rows[0].get("_child").is_none());
        let row_id=rows[0]["id"].as_str().unwrap();
        let (_,child_id)=super::super::children::linked(&branch,row_id).unwrap();
        assert_eq!(child_id,"child-conv-42");
    }

    #[test]
    fn ask_question_step_pairs_in_bounded_history_and_keeps_questions() {
        let temp=super::super::tests::Temp::new();
        let records=[
            json!({"type":"PLANNER_RESPONSE","step_index":0,"tool_calls":[
                {"name":"ask_question","args":{"questions":[{"question":"Which port?","is_multi_select":false,"options":[{"id":"1","text":"8080"}]}]}},
                {"name":"view_file","args":{"AbsolutePath":"/tmp/cfg"}}
            ]}),
            json!({"type":"ASK_QUESTION","step_index":1,"status":"DONE","content":"Created At: 2026-11-16T10:00:00+08:00\nA1: 8080"}),
            json!({"type":"VIEW_FILE","step_index":2,"status":"DONE","content":"port=8080"}),
        ];
        let branch=indexed(&temp,&records);
        let page=branch.page(None,None,None).unwrap();
        let rows=page["rows"].as_array().unwrap();
        assert_eq!(rows.len(),2);
        assert_eq!(rows[0]["name"],"AskUserQuestion");
        assert_eq!(rows[0]["status"],"completed");
        assert_eq!(rows[0]["input"]["questions"][0]["question"],"Which port?");
        assert_eq!(rows[0]["input"]["questions"][0]["options"][0]["label"],"8080");
        assert!(rows[0]["output"].as_str().unwrap().contains("A1: 8080"));
        assert_eq!(rows[1]["name"],"Read");
        assert_eq!(rows[1]["output"],"port=8080");
    }

    #[test]
    fn step_index_reset_preserves_earlier_rows_and_keeps_same_step_merges() {
        let temp=super::super::tests::Temp::new();
        let path=temp.0.join("native.jsonl");
        let initial=[
            json!({"type":"USER_INPUT","step_index":0,"content":"<USER_REQUEST>before reset</USER_REQUEST>"}),
            json!({"type":"PLANNER_RESPONSE","step_index":1,"status":"RUNNING","content":"working"}),
            json!({"type":"PLANNER_RESPONSE","step_index":1,"status":"DONE","content":"first answer"}),
            json!({"type":"PLANNER_RESPONSE","step_index":2,"tool_calls":[{"name":"view_file","args":{"AbsolutePath":"/tmp/abandoned"}}]}),
        ];
        let raw=initial.iter().map(|record|record.to_string()+"\n").collect::<String>();
        std::fs::write(&path,&raw).unwrap();
        let store=super::super::store::Store::at(&temp.0).unwrap();
        let source=store.register(SessionKind::Antigravity,super::super::source::Segment {path:path.clone(),native_id:"native".into(),end_byte:None,end_ordinal:None}).unwrap();
        store.scan(SessionKind::Antigravity,&source,&std::sync::atomic::AtomicBool::new(false),false).unwrap();

        // Append a reset segment starting back at step_index 0 and verify incremental rescan preserves both halves.
        let after_reset=[
            json!({"type":"USER_INPUT","step_index":0,"content":"<USER_REQUEST>after reset</USER_REQUEST>\n<USER_SETTINGS_CHANGE>Model Selection</USER_SETTINGS_CHANGE>"}),
            json!({"type":"GENERIC","step_index":1,"status":"DONE","content":"unpaired result must not bind pre-reset call"}),
            json!({"type":"PLANNER_RESPONSE","step_index":2,"status":"DONE","content":"second answer"}),
        ];
        let appended=raw+&after_reset.iter().map(|record|record.to_string()+"\n").collect::<String>();
        std::fs::write(&path,appended).unwrap();
        let source=store.register(SessionKind::Antigravity,super::super::source::Segment {path,native_id:"native".into(),end_byte:None,end_ordinal:None}).unwrap();
        store.scan(SessionKind::Antigravity,&source,&std::sync::atomic::AtomicBool::new(false),false).unwrap();

        let branch=super::super::api::Branch {store,sources:vec![source],generation:"branch".into(),kind:SessionKind::Antigravity,native_id:"native".into(),scope:"scope".into(),error:None,parent:None};
        let page=branch.page(None,None,None).unwrap();
        let rows=page["rows"].as_array().unwrap();
        assert_eq!(rows.len(),5,"rows={rows:?}");
        assert_eq!(rows[0]["text"],"before reset");
        assert_eq!(rows[1]["text"],"first answer");
        assert_eq!(rows[2]["name"],"Read");
        assert!(rows[2].get("output").is_none(),"post-reset generic step must not pair with abandoned pre-reset tool call");
        assert_eq!(rows[3]["text"],"after reset");
        assert_eq!(rows[4]["text"],"second answer");
        assert_ne!(rows[0]["id"],rows[3]["id"]);
    }
}
