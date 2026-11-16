//! Native rewind identities are selected from the current indexed branch, never from display text.

use std::collections::HashSet;
use serde_json::{json,Value};
use crate::{agent::chat::history::RewindTarget,host::AppCtx,models::SessionKind};
use super::{api::{self,Branch},store::{RegisteredSource,State}};

pub struct Selection {pub branch:Branch,pub row:String,pub target:RewindTarget,pub position:String}

impl Selection {
    pub fn prefill(&self) -> Result<String,String> {
        if !self.target.text.is_empty() {return Ok(self.target.text.clone());}
        let mut text=String::new();let mut cursor=None;let mut first=true;
        loop {
            let page=self.branch.manifest_page(&self.row,cursor.as_deref())?;
            for field in page["fields"].as_array().ok_or("index_incomplete: rewind body manifest is unavailable")? {
                if field["label"]!="body" {continue;}
                let reference=field["detailRef"].as_str().ok_or("index_incomplete: rewind body reference is unavailable")?;
                let size=field["sizeBytes"].as_u64().unwrap_or(0);
                let prefix=field["prefix"].as_str().unwrap_or_default();let suffix=field["suffix"].as_str().unwrap_or_default();
                if text.len() as u64+size+prefix.len() as u64+suffix.len() as u64+1>8*1024*1024 {
                    return Err("detail_requires_paging: rewind text exceeds the editable draft budget".into());
                }
                if !first {text.push('\n');}first=false;text.push_str(prefix);
                if let Some(literal)=field["literal"].as_str() {
                    self.branch.write_detail(reference,None,&mut std::io::sink(),&std::sync::atomic::AtomicBool::new(false))?;text.push_str(literal);
                } else {
                    let mut sequence=0;loop {let part=self.branch.detail(reference,None,sequence)?;
                        text.push_str(part["text"].as_str().ok_or("index_incomplete: rewind text chunk is unavailable")?);
                        if text.len()>8*1024*1024 {return Err("detail_requires_paging: rewind text exceeds the editable draft budget".into());}
                        if part["eof"]==true {break;}sequence+=1;
                    }
                }text.push_str(suffix);
            }
            cursor=page["nextCursor"].as_str().map(str::to_owned);if cursor.is_none() {break;}
        }
        if text.is_empty() {
            let page=self.branch.manifest(&self.row,0)?;
            text=page["row"]["text"].as_str().or_else(||page["row"]["command"].as_str()).unwrap_or_default().into();
        }
        if let Some(last)=&self.target.last_message_id {
            let mut found=false;
            for source in &self.branch.sources {
                let start={
                    let state=source.current()?;
                    state.identities.get(&**last).into_iter().flatten().copied().find(|start|api::record_visible(&state,source,*start))
                };
                if let Some(start)=start {
                    let (record,refs)=super::event_stream::read_record(source,start)?;
                    let field=super::shell_text::single(&record,&refs).ok_or("index_incomplete: native shell output has no original field")?;
                    if text.len() as u64+field.decoded_bytes+1>8*1024*1024 {return Err("detail_requires_paging: rewind text exceeds the editable draft budget".into());}
                    text.push('\n');super::fields::walk(source,&field,&std::sync::atomic::AtomicBool::new(false),&mut |part|{text.push_str(part);Ok(())})?;
                    found=true;break;
                }
            }
            if !found {return Err("source_changed: the selected native shell output is no longer visible".into());}
        }
        Ok(text.trim().into())
    }

    pub fn latest(&self) -> Result<Selection,String> {
        ready(&self.branch)?;
        for source in self.branch.sources.iter().rev() {
            let rows={let state=source.current()?;native_rows(&self.branch,&state,source)};
            for (_,id) in rows.into_iter().rev() {
                match select(self.branch.clone(),&id,Some(&self.branch.generation)) {
                    Err(error) if error.starts_with("rewind_not_candidate:")=>continue,
                    result=>return result,
                }
            }
        }
        Err("detail_unavailable: the native branch contains no rewind target".into())
    }

    pub fn turn_count(&self) -> Result<usize,String> {
        ready(&self.branch)?;
        let mut turns:HashSet<Option<String>>=HashSet::new();
        for (sequence,source) in self.branch.sources.iter().enumerate() {
            let rows={
                let state=source.current()?;
                native_rows(&self.branch,&state,source).into_iter().filter(|(order,_)|api::order_key(sequence,*order)>=self.position).map(|(_,id)| {
                    let row=&state.rows[&*id];
                    let turn=state.records.range(..=row.record_start).rev().find(|(_,record)|record.event.as_deref()==Some("turn_context") && record.active)
                        .map(|(start,record)|(*start,record.end,record.turn.as_deref().map(str::to_owned)));
                    (row.record_start,row.record_end,turn)
                }).collect::<Vec<_>>()
            };
            for (from,to,turn) in rows {
                verify(source,from,to)?;
                let (start,end,turn)=turn.ok_or("index_incomplete: a removed user message has no confirmed native turn")?;verify(source,start,end)?;
                turns.insert(turn);
            }
        }
        if turns.contains(&None) {return Err("index_incomplete: a removed user message has no confirmed native turn".into());}
        if turns.is_empty() {return Err("index_incomplete: native rewind turn range is empty".into());}Ok(turns.len())
    }
}

fn ready(branch:&Branch) -> Result<(),String> {
    if let Some(error)=&branch.error {return Err(error.clone());}
    if !(matches!(branch.kind,SessionKind::Claude|SessionKind::Codex) || branch.kind.uses_opencode()) {
        return Err("detail_unavailable: this provider does not support conversation rewind".into());
    }
    for source in &branch.sources {
        if super::deepseek_legacy::is_legacy_source(source) {continue;}
        let state=source.current().map_err(|_|"source_changed: rewind source generation changed".to_string())?;
        if state.status!="complete" {return Err(state.error.clone().unwrap_or_else(||"index_busy: native rewind targets are still being indexed".into()));}
    }Ok(())
}

/// Visible user, command and shell rows of one source in order: the messages a rewind can target.
fn native_rows(branch:&Branch,state:&State,source:&RegisteredSource) -> Vec<((u64,u32),std::sync::Arc<str>)> {
    if super::deepseek_legacy::is_legacy_source(source) {return Vec::new();}
    branch.ordered(state,source,source.segment.end_byte).into_iter().filter(|(_,id)|matches!(&*state.rows[&**id].kind,"user"|"command"|"shell")).collect()
}
fn identity(value:Option<&str>) -> Result<String,String> {
    value.filter(|id|!id.is_empty() && id.len()<=200).map(str::to_owned).ok_or_else(||"index_incomplete: rewind target has no bounded native identity".into())
}
fn verify(source:&RegisteredSource,start:u64,end:u64) -> Result<(),String> {
    let mut at=start;
    while at<end {let next=(at+super::limits::INPUT_BYTES as u64).min(end);super::store::verified_range(source,at,next)?;at=next;}
    Ok(())
}
/// The neighboring active, error-free record before or after `start` within the segment.
fn neighbor(source:&RegisteredSource,start:u64,before:Option<&str>) -> Result<Option<(u64,u64)>,String> {
    let state=source.current()?;
    Ok(match before {
        Some(parent)=>state.identities.get(parent).into_iter().flatten().copied().filter(|candidate|*candidate<start)
            .filter(|candidate|state.records.get(candidate).is_some_and(|record|record.active && record.error.is_none())).max()
            .map(|candidate|(candidate,state.records[&candidate].end)),
        None=>state.records.range(start+1..).find(|(_,record)|record.active && record.error.is_none()
            && source.segment.end_byte.is_none_or(|end|record.end<=end) && source.segment.end_ordinal.is_none_or(|end|record.ordinal.is_none_or(|ordinal|ordinal<end)))
            .map(|(candidate,record)|(*candidate,record.end)),
    })
}
fn claude_target(source:&RegisteredSource,start:u64,end:u64,validate:bool) -> Result<RewindTarget,String> {
    let (record,refs)=super::event_stream::read_record(source,start)?;let value=&record.value;
    let long=record.fields.iter().filter(|field|field.encoding=="string" && (field.path=="/message/content" || field.path.starts_with("/message/content/") && field.path.ends_with("/text")))
        .any(|field|value.pointer(&field.path).and_then(Value::as_str).is_none_or(|text|text.len() as u64!=field.decoded_bytes));
    let raw=value.to_string();let mut own=crate::agent::chat::history::claude_rewind_targets(&raw);
    let mut selected=own.pop().ok_or("rewind_not_candidate: this native record is not a genuine user turn")?;
    if selected.text.starts_with("<bash-stdout>") {
        if let Some(parent)=value["parentUuid"].as_str() {
            if let Some((from,to))=neighbor(source,start,Some(parent))? {
                let (previous,previous_refs)=super::event_stream::read_record(source,from)?;
                if let (Some(input),Some(output))=(super::shell_text::single(&previous,&previous_refs),super::shell_text::single(&record,&refs)) {
                    if super::shell_text::input(source,&input)? && super::shell_text::output(source,&output)?.is_some() {
                        if validate {verify(source,from,to)?;verify(source,start,end)?;}
                        return Err("rewind_not_candidate: native shell output belongs to its input message".into());
                    }
                }
                let pair=crate::agent::chat::history::claude_rewind_targets(&format!("{}\n{raw}",previous.value));
                if pair.len()==1 && pair[0].last_message_id.as_deref()==Some(selected.message_id.as_str()) {
                    if validate {verify(source,from,to)?;}
                    return Err("rewind_not_candidate: native shell output belongs to its input message".into());
                }
            }
        }
    }
    if let Some((from,to))=neighbor(source,start,None)? {
        let (next,next_refs)=super::event_stream::read_record(source,from)?;let next_value=&next.value;
        if selected.text.starts_with("<bash-input>") && next_value["type"]=="user" && next_value["parentUuid"]==selected.message_id
            && next_value["isMeta"]!=true && next_value["isSidechain"]!=true {
            if let (Some(input),Some(output))=(super::shell_text::single(&record,&refs),super::shell_text::single(&next,&next_refs)) {
                if super::shell_text::input(source,&input)? && super::shell_text::output(source,&output)?.is_some() {
                    if validate {verify(source,from,to)?;verify(source,start,end)?;}
                    selected.last_message_id=Some(identity(next_value["uuid"].as_str())?);selected.text.clear();return Ok(selected);
                }
            }
        }
        let pair=crate::agent::chat::history::claude_rewind_targets(&format!("{raw}\n{next_value}"));
        if pair.len()==1 && pair[0].last_message_id.is_some() {
            if validate {verify(source,from,to)?;}
            selected=pair.into_iter().next().unwrap();
        }
    }
    // A projected preview is a full editable draft only when every contributing string is complete.
    if long {selected.text.clear();}
    if validate {verify(source,start,end)?;}Ok(selected)
}
fn target(branch:&Branch,source:&RegisteredSource,start:u64,end:u64,validate:bool) -> Result<RewindTarget,String> {
    if branch.kind==SessionKind::Claude {return claude_target(source,start,end,validate);}
    if validate {verify(source,start,end)?;}
    let value=super::store::reproject(source,start,&mut |_|Ok(()))?.value;
    let (message,turn)=match branch.kind {
        SessionKind::Opencode|SessionKind::Deepseek=>(identity(value.pointer(if value.pointer("/info/role").and_then(Value::as_str)==Some("assistant") {"/info/parentID"} else {"/info/id"}).and_then(Value::as_str))?,None),
        SessionKind::Codex=>{
            let turn={
                let state=source.current()?;
                state.records.range(..=start).rev().find(|(_,record)|record.event.as_deref()==Some("turn_context") && record.active && record.error.is_none())
                    .map(|(from,record)|(*from,record.end,record.turn.as_deref().map(str::to_owned)))
            };
            let (from,to,turn)=turn.ok_or("index_incomplete: native user message has no confirmed turn")?;
            if validate {verify(source,from,to)?;}
            let turn=identity(turn.as_deref())?;
            (value.pointer("/payload/id").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(||turn.clone()),Some(turn))
        },
        _=>unreachable!(),
    };
    Ok(RewindTarget {message_id:message,last_message_id:None,turn_id:turn,text:String::new()})
}

pub fn candidates(branch:&Branch,cursor:Option<&str>) -> Result<Value,String> {
    ready(branch)?;
    let from=branch.cursor_sequence("rewind","native",cursor)?;let mut skip=from;let mut examined=0u64;let mut rows=Vec::new();let mut more=false;
    for source in &branch.sources {
        let listed={
            let state=source.current()?;
            native_rows(branch,&state,source).into_iter().map(|(_,id)| {
                let row=&state.rows[&*id];(id.to_string(),row.record_start,row.record_end,state.summary(&id).unwrap_or_default().to_owned())
            }).collect::<Vec<_>>()
        };
        let count=listed.len() as u64;
        if skip>=count {skip-=count;continue;}
        for (id,start,end,summary) in listed.into_iter().skip(skip as usize) {
            if examined==60 {more=true;break;}
            examined+=1;
            match target(branch,source,start,end,false) {
                Ok(_)=>{
                    let row:Value=serde_json::from_str(&summary).map_err(|e|e.to_string())?;
                    let value=super::store::reproject(source,start,&mut |_|Ok(()))?.value;
                    let text=row["text"].as_str().or_else(||row["command"].as_str()).unwrap_or_default().chars().take(240).collect::<String>();
                    rows.push(json!({"rowId":id,"text":text,"at":row["at"],"hasImages":value.pointer("/message/content").or_else(||value.pointer("/payload/content")).and_then(Value::as_array)
                        .is_some_and(|items|items.iter().any(|item|matches!(item["type"].as_str(),Some("image"|"input_image"))))}));
                },
                Err(error) if error.starts_with("index_incomplete:") || error.starts_with("rewind_not_candidate:")=>{},
                Err(error)=>return Err(error),
            }
        }
        skip=0;
        if more {break;}
    }
    let value=json!({"protocolVersion":2,"historyGeneration":branch.generation,"rows":rows,
        "nextCursor":more.then(||branch.cursor("rewind","native",from+examined))});
    if serde_json::to_vec(&value).map_err(|e|e.to_string())?.len()>super::limits::PAGE_BYTES {return Err("unsupported_record: rewind page exceeds its byte budget".into());}
    Ok(value)
}

pub fn select(branch:Branch,row:&str,expected:Option<&str>) -> Result<Selection,String> {
    if expected.is_some_and(|generation|generation!=branch.generation) {return Err("cursor_expired: rewind branch changed".into());}
    ready(&branch)?;let row=branch.resolve_row(row);
    for (sequence,source) in branch.sources.iter().enumerate() {
        let found={
            let state=source.current()?;
            native_rows(&branch,&state,source).into_iter().find(|(_,id)|**id==*row)
                .map(|(order,id)|{let record=&state.rows[&*id];(order,record.record_start,record.record_end)})
        };
        if let Some((order,start,end))=found {
            let target=target(&branch,source,start,end,true)?;
            return Ok(Selection {branch,row,target,position:api::order_key(sequence,order)});
        }
    }
    Err("detail_unavailable: user message has no confirmed native rewind identity".into())
}

pub fn prepare(app:&AppCtx,session:&str,cursor:Option<&str>) -> Result<Value,String> {candidates(&api::prepare(app,session)?,cursor)}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_prompts_keep_distinct_native_turns_and_pages_are_scope_bound() {
        let temp=super::super::tests::Temp::new();let mut raw=String::new();
        for number in 0..161 {raw.push_str(&format!("{}\n{}\n",json!({"type":"turn_context","payload":{"turn_id":format!("turn-{number}")}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Identical prompt"}]}})));}
        let branch=super::super::tests::indexed(&temp,&raw);let first=candidates(&branch,None).unwrap();assert_eq!(first["rows"].as_array().unwrap().len(),60);
        let cursor=first["nextCursor"].as_str().unwrap();let next=candidates(&branch,Some(cursor)).unwrap();assert_eq!(next["rows"].as_array().unwrap().len(),60);
        let id=next["rows"][0]["rowId"].as_str().unwrap();assert_eq!(select(branch.clone(),id,Some(&branch.generation)).unwrap().target.turn_id.as_deref(),Some("turn-60"));
        let foreign=Branch {scope:"foreign".into(),..branch.clone()};assert!(candidates(&foreign,Some(cursor)).unwrap_err().starts_with("cursor_expired:"));
        assert!(select(branch.clone(),id,Some("old-generation")).err().unwrap().starts_with("cursor_expired:"));
        assert!(select(branch.clone(),"turn-60",None).is_err());
        let source=&branch.sources[0];let changed=raw.replacen("turn-60","turn-99",1);std::fs::write(&source.segment.path,changed).unwrap();
        assert!(select(branch,id,None).err().unwrap().starts_with("source_changed:"));
    }

    #[test]
    fn full_prefill_uses_original_fields_and_rollback_counts_distinct_native_turns() {
        let temp=super::super::tests::Temp::new();let text="Complete editable original 雪😀\n".repeat(20_000);
        let mut raw=format!("{}\n",json!({"type":"turn_context","payload":{"turn_id":"same-turn"}}));
        for body in [&text,"Steered input"] {raw.push_str(&format!("{}\n",json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":body}]}})));}
        raw.push_str(&format!("{}\n{}\n",json!({"type":"turn_context","payload":{"turn_id":"next-turn"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Next prompt"}]}})));
        let branch=super::super::tests::indexed(&temp,&raw);let page=candidates(&branch,None).unwrap();let id=page["rows"][0]["rowId"].as_str().unwrap();
        let selected=select(branch,id,None).unwrap();assert_eq!(selected.prefill().unwrap(),text.trim());assert_eq!(selected.turn_count().unwrap(),2);
        assert_eq!(selected.latest().unwrap().target.turn_id.as_deref(),Some("next-turn"));
    }

    #[test]
    fn claude_shell_pair_keeps_its_input_and_final_native_boundary() {
        let temp=super::super::tests::Temp::new();let path=temp.0.join("claude.jsonl");
        let input=json!({"type":"user","uuid":"input","message":{"content":"<bash-input>printf text</bash-input>"}});
        let output=json!({"type":"user","uuid":"output","parentUuid":"input","message":{"content":"<bash-stdout>Literal exit_code=7</bash-stdout>"}});
        std::fs::write(&path,format!("{input}\n{output}\n")).unwrap();let store=super::super::store::Store::at(&temp.0).unwrap();
        let source=store.register(SessionKind::Claude,super::super::source::Segment {path,native_id:"thread".into(),end_byte:None,end_ordinal:None}).unwrap();
        store.scan(SessionKind::Claude,&source,&std::sync::atomic::AtomicBool::new(false),false).unwrap();
        let branch=api::bind(store,vec![source],SessionKind::Claude,"thread".into(),None,"scope").unwrap();let page=candidates(&branch,None).unwrap();
        assert_eq!(page["rows"].as_array().unwrap().len(),1);let selected=select(branch,page["rows"][0]["rowId"].as_str().unwrap(),None).unwrap();
        assert_eq!(selected.target.message_id,"input");assert_eq!(selected.target.last_message_id.as_deref(),Some("output"));
        assert_eq!(selected.latest().unwrap().target.message_id,"input");assert!(selected.prefill().unwrap().contains("Literal exit_code=7"));
    }

    #[test]
    fn long_native_shell_output_is_streamed_and_ambiguous_tags_keep_separate_turns() {
        for ambiguous in [false,true] {
            let temp=super::super::tests::Temp::new();let path=temp.0.join("claude.jsonl");let long="Complete literal output 雪😀\n".repeat(20_000);
            let input=json!({"type":"user","uuid":"input","message":{"content":"<bash-input>printf text</bash-input>"}});
            let output=json!({"type":"user","uuid":"output","parentUuid":"input","message":{"content":format!("<bash-stdout>{long}{}</bash-stdout>",if ambiguous {"<bash-input>literal tag"} else {""})}});
            let raw=format!("{input}\n{output}\n");std::fs::write(&path,&raw).unwrap();let store=super::super::store::Store::at(&temp.0).unwrap();
            let source=store.register(SessionKind::Claude,super::super::source::Segment {path:path.clone(),native_id:"thread".into(),end_byte:None,end_ordinal:None}).unwrap();
            store.scan(SessionKind::Claude,&source,&std::sync::atomic::AtomicBool::new(false),false).unwrap();
            let branch=api::bind(store,vec![source],SessionKind::Claude,"thread".into(),None,"scope").unwrap();let page=candidates(&branch,None).unwrap();
            assert_eq!(page["rows"].as_array().unwrap().len(),if ambiguous {2} else {1});
            let selected=select(branch,page["rows"][0]["rowId"].as_str().unwrap(),None).unwrap();
            assert_eq!(selected.target.last_message_id.is_some(),!ambiguous);
            if !ambiguous {assert_eq!(selected.prefill().unwrap(),format!("<bash-input>printf text</bash-input>\n<bash-stdout>{long}</bash-stdout>"));}
            assert_eq!(std::fs::read_to_string(&path).unwrap(),raw);
        }
    }
}
