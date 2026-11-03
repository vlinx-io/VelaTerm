//! Resumable session search. A request scans a bounded amount and returns snippets, never full rows.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool,Ordering};
use std::time::{Duration,Instant};

use serde_json::{json,Value};

use crate::host::AppCtx;
use super::api::{self,Branch};
use super::limits::*;

struct Job {
    session:String,
    generation:String,
    query:String,
    branch:Branch,
    complete:bool,
    after:Option<String>,
    pending:VecDeque<(String,Value)>,
    row:Option<(String,Value)>,
    fields:VecDeque<(String,String)>,
    field:Option<String>,
    sequence:u64,
    offset:usize,
    carry:String,
    previous_bytes:usize,
    summary:String,
    summary_offset:Option<usize>,
    inspected:u64,
    rows:u64,
    canceled:Arc<AtomicBool>,
    touched:Instant,
    page_sequence:u64,
    responses:VecDeque<(u64,Value)>,
}

type Jobs=HashMap<String,Arc<Mutex<Job>>>;
static JOBS:OnceLock<Mutex<Jobs>>=OnceLock::new();

/// Simple Unicode case folding, without the multi-character expansions regex search never promised.
fn fold(text:&str) -> (String,Vec<(usize,usize)>) {
    let mut result=String::new();
    let mut offsets=Vec::new();
    for (start,ch) in text.char_indices() {
        let mut upper=ch.to_uppercase();
        let first=upper.next().unwrap();
        let mut lower=first.to_lowercase();
        let folded=lower.next().unwrap();
        let folded=if upper.next().is_none() && lower.next().is_none() {folded} else {ch};
        offsets.push((result.len(),start)); result.push(folded);
    }
    offsets.push((result.len(),text.len()));
    (result,offsets)
}

fn snippets(text:&str,query:&str,offset:usize,skip_end:usize,limit:usize) -> (Vec<(String,usize)>,Option<usize>) {
    let (folded,map)=fold(text);
    let (needle,_)=fold(query);
    let mut hits=Vec::new();
    let mut at=map.iter().find(|(_,original)|*original>=offset).map_or(folded.len(),|(folded,_)|*folded);
    while at<folded.len() {
        let Some(found)=folded[at..].find(&needle).map(|index|at+index) else {break;};
        let start=map.iter().find(|(folded,_)|*folded==found).map_or(0,|(_,original)|*original);
        let end=map.iter().find(|(folded,_)|*folded>=found+needle.len()).map_or(text.len(),|(_,original)|*original);
        if end<=skip_end {at=found+needle.len();continue;}
        let mut left=start.saturating_sub(100); while !text.is_char_boundary(left) {left+=1;}
        let mut right=(end+160).min(text.len()); while !text.is_char_boundary(right) {right-=1;}
        hits.push((text[left..right].into(),start));
        at=found+needle.len();
        if hits.len()>=limit {return (hits,Some(end));}
    }
    (hits,None)
}

pub fn read(app:&AppCtx,session:&str,query:&str,cursor:Option<&str>) -> Result<Value,String> {
    if cursor.is_none() && !query.trim().is_empty() {
        if let Some(page)=indexed(app,session,query)? {return Ok(page);}
    }
    read_with(session,query,cursor,||api::prepare(app,session))
}

/// Full-text search through the global content index, scoped to this session: every whitespace-separated
/// term must match, in any order, whole words first. Returns None when the index has no transcript match,
/// or cannot answer, so the caller falls back to the literal scan, which also covers content newer than
/// the index.
fn indexed(app:&AppCtx,session:&str,query:&str) -> Result<Option<Value>,String> {
    let Ok(result)=crate::search::jobs::blocking(app,query,crate::search::SearchScope::All,Some(session),1,true) else {return Ok(None);};
    let Some(matches)=result["hits"].as_array().and_then(|hits|hits.iter().find(|hit|hit["sessionId"]==session))
        .and_then(|hit|hit["matches"].as_array()).filter(|matches|!matches.is_empty()) else {return Ok(None);};
    let branch=api::prepare(app,session)?;
    let mut prose=None;
    let mut hits=Vec::new();
    for found in matches {
        let mut row=match found["recordOffset"].as_u64() {Some(offset)=>branch.row_at_record(offset)?,None=>None};
        if row.is_none() {
            let Some(index)=found["messageIndex"].as_u64() else {continue;};
            if prose.is_none() {prose=Some(super::prose::Prose::new(branch.clone(),&AtomicBool::new(false))?);}
            row=prose.as_ref().unwrap().row_id(index)?;
        }
        let Some(row)=row else {continue;};
        hits.push(json!({"rowId":row,"snippet":found["snippet"],"offset":0,"matched":found["matched"]}));
    }
    if hits.is_empty() {return Ok(None);}
    Ok(Some(json!({"protocolVersion":2,"historyGeneration":branch.generation,"hits":hits,"nextCursor":null,
        "coverage":"complete","scannedBytes":0,"fullText":true})))
}

fn read_with(session:&str,query:&str,cursor:Option<&str>,mut prepare:impl FnMut()->Result<Branch,String>) -> Result<Value,String> {
    if query.is_empty() {return Ok(json!({"hits":[],"nextCursor":null,"coverage":"complete","scannedBytes":0}));}
    if query.len()>512 {return Err("Search query exceeds its byte budget".into());}
    let map=JOBS.get_or_init(||Mutex::new(HashMap::new()));
    let (id,job,page_sequence)=if let Some(cursor)=cursor {
        let (id,sequence)=cursor.rsplit_once('.').ok_or("cursor_expired: invalid search cursor")?;
        let sequence=sequence.parse::<u64>().map_err(|_|"cursor_expired: invalid search cursor")?;
        let job=map.lock().unwrap().get(id).cloned().ok_or("cursor_expired: search job expired")?;
        (id.to_owned(),job,sequence)
    } else {
        let mut branch=prepare()?;
        let mut complete=branch.error.is_none();
        for source in &mut branch.sources {
            let (water,status)={let state=source.current()?;(state.indexed,state.status.clone())};
            complete&=status=="complete" || source.segment.end_byte.is_some_and(|end|water>=end);
            source.segment.end_byte=Some(source.segment.end_byte.unwrap_or(water).min(water));
        }
        let generation=branch.generation.clone();
        let id=uuid::Uuid::new_v4().to_string();
        let job=Arc::new(Mutex::new(Job {session:session.into(),generation,query:query.into(),branch,complete,after:None,pending:VecDeque::new(),row:None,
            fields:VecDeque::new(),field:None,sequence:0,offset:0,carry:String::new(),previous_bytes:0,summary:String::new(),summary_offset:None,
            inspected:0,rows:0,canceled:Arc::new(AtomicBool::new(false)),touched:Instant::now(),
            page_sequence:0,responses:VecDeque::new()}));
        let mut jobs=map.lock().unwrap();
        jobs.retain(|_,job|job.try_lock().map_or(true,|job|job.touched.elapsed()<Duration::from_secs(600)));
        if jobs.len()>=64 {return Err("Too many active history searches".into());}
        jobs.insert(id.clone(),job.clone());
        (id,job,0)
    };
    let mut job=job.lock().unwrap();
    if job.session!=session || job.query!=query {return Err("cursor_expired: search cursor belongs to another scope or query".into());}
    if prepare()?.generation!=job.generation {return Err("source_changed: search history changed".into());}
    if job.canceled.load(Ordering::Relaxed) {return Err("read_canceled".into());}
    if page_sequence<job.page_sequence {
        return job.responses.iter().find(|(sequence,_)|*sequence==page_sequence).map(|(_,response)|response.clone())
            .ok_or_else(||"cursor_expired: search response is no longer cached".into());
    }
    if page_sequence>job.page_sequence {return Err("cursor_expired: search cursor was not issued".into());}
    job.touched=Instant::now();
    let began=Instant::now();
    let baseline=job.inspected;
    let mut hits=Vec::new();
    let mut finished=false;
    while hits.len()<50 && job.inspected-baseline<TAIL_BYTES && began.elapsed()<Duration::from_millis(200) {
        if job.canceled.load(Ordering::Relaxed) {return Err("read_canceled".into());}
        if job.row.is_none() {
            if job.pending.is_empty() {job.pending=job.branch.forward(job.after.as_deref())?.into();}
            let Some((order,row))=job.pending.pop_front() else {finished=true;break;};
            job.after=Some(order.clone()); job.rows+=1;
            let row_id=row["id"].as_str().unwrap_or_default().to_owned();
            let mut fields=VecDeque::new();
            for source in &job.branch.sources {
                let state=source.current()?;
                let listed=job.branch.row_fields(&state,source,&row_id);
                for (field,meta) in &listed {
                    if field.label=="original" || !matches!(meta.encoding.as_str(),"string"|"slice"|"json") {continue;}
                    // A document nested in a string is searched through its outer field.
                    if meta.parent.as_ref().is_some_and(|parent|listed.iter().any(|(_,other)|other==&**parent)) {continue;}
                    if field.label!="body_image" && !field.label.starts_with("attachment:") {fields.push_back((field.reference.clone(),field.label.clone()));}
                }
            }
            // Only content searchable in the existing view belongs here; ids, states, and wire keys do
            // not become extra search results. Deferred fields replace, rather than duplicate, previews.
            let mut pieces=Vec::new();
            if row["kind"]=="tool" {
                pieces.push(row["name"].as_str().unwrap_or_default().to_owned());
                if !fields.iter().any(|(_,label)|label=="input") {pieces.push(row["input"].to_string());}
                if !fields.iter().any(|(_,label)|label=="output") {pieces.push(row["output"].as_str().unwrap_or_default().to_owned());}
            } else {
                if fields.is_empty() {
                    if let Some(text)=row.get("text").or_else(||row.get("message")).and_then(Value::as_str) {pieces.push(text.to_owned());}
                } else if row["text"].as_str().is_some_and(|text|text.contains("[image]")) {pieces.push("[image]".into());}
            }
            job.summary=pieces.join("\n");job.summary_offset=Some(0);
            job.fields=fields; job.row=Some((order,row));
        }
        if let Some(offset)=job.summary_offset {
            let (found,next)=snippets(&job.summary,query,offset,0,50-hits.len());
            let row_id=job.row.as_ref().unwrap().1["id"].as_str().unwrap_or_default();
            for (snippet,offset) in found {hits.push(json!({"rowId":row_id,"snippet":snippet,"offset":offset}));}
            job.summary_offset=next;
            if hits.len()>=50 {break;}
        }
        if job.field.is_none() {
            let Some((reference,_))=job.fields.pop_front() else {job.row=None;continue;};
            job.field=Some(reference); job.sequence=0; job.offset=0; job.carry.clear();job.previous_bytes=0;
        }
        let reference=job.field.as_ref().unwrap().clone();
        let detail=job.branch.detail(&reference,None,job.sequence)?;
        let text=detail["text"].as_str().unwrap_or_default();
        let combined=job.carry.clone()+text;
        let (found,next)=snippets(&combined,query,job.offset,job.carry.len(),50-hits.len());
        let row_id=job.row.as_ref().unwrap().1["id"].as_str().unwrap_or_default();
        for (snippet,offset) in found {
            let (sequence,offset)=if offset<job.carry.len() {
                (job.sequence.saturating_sub(1),job.previous_bytes.saturating_sub(job.carry.len())+offset)
            } else {(job.sequence,offset-job.carry.len())};
            hits.push(json!({"rowId":row_id,"detailRef":reference,"chunkSequence":sequence,"snippet":snippet,"offset":offset}));
        }
        job.inspected+=text.len() as u64;
        if let Some(offset)=next {job.offset=offset;}
        else if detail["eof"]==true {job.field=None;job.carry.clear();}
        else {
            let mut left=combined.len().saturating_sub(query.len()*4);
            while !combined.is_char_boundary(left) {left+=1;}
            job.carry=combined[left..].to_owned();job.previous_bytes=text.len(); job.offset=0; job.sequence+=1;
        }
    }
    let next=format!("{id}.{}",job.page_sequence+1);
    let result=json!({"protocolVersion":2,"historyGeneration":job.generation,"hits":hits,"jobId":id,
        "nextCursor":if finished {None} else {Some(next)},"coverage":if finished && job.complete {"complete"} else {"partial"},
        "scannedBytes":job.inspected,"scannedRows":job.rows});
    if serde_json::to_vec(&result).map_err(|e|e.to_string())?.len()>PAGE_BYTES {return Err("Search results exceed their wire byte budget".into());}
    let sequence=job.page_sequence;
    job.responses.push_back((sequence,result.clone()));
    job.page_sequence+=1;
    while job.responses.len()>2 {job.responses.pop_front();}
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_performs_no_source_or_detail_read() {
        let result=read_with("session","",None,||panic!("An empty query must not open history")).unwrap();
        assert_eq!(result["scannedBytes"],0); assert_eq!(result["hits"],json!([]));
    }

    #[test]
    fn unicode_snippets_resume_without_repeating_carry_matches() {
        let (hits,next)=snippets("Σ σ ς 中文 🚀","σ",0,0,2);
        assert_eq!(hits.len(),2);
        let (rest,_)=snippets("Σ σ ς 中文 🚀","σ",next.unwrap(),0,50);
        assert_eq!(rest.len(),1);
        let (hits,_)=snippets("needleneedle","needle",0,6,50);
        assert_eq!(hits.len(),1); assert_eq!(hits[0].1,6);
    }

    #[test]
    fn search_pages_are_repeatable_scoped_and_cancelable() {
        let temp=super::super::tests::Temp::new();
        let text="needle ".repeat(120);
        let raw=json!({"type":"response_item","payload":{"type":"message","id":"message","role":"user",
            "content":[{"type":"input_text","text":text}]}}).to_string()+"\n";
        let branch=super::super::tests::indexed(&temp,&raw);
        let session=uuid::Uuid::new_v4().to_string();
        let first=read_with(&session,"needle",None,||Ok(branch.clone())).unwrap();
        assert_eq!(first["hits"].as_array().unwrap().len(),50);
        let cursor=first["nextCursor"].as_str().unwrap();
        let second=read_with(&session,"needle",Some(cursor),||Ok(branch.clone())).unwrap();
        assert_eq!(second["hits"].as_array().unwrap().len(),50);
        assert_eq!(read_with(&session,"needle",Some(cursor),||Ok(branch.clone())).unwrap(),second);
        assert!(read_with("foreign","needle",Some(cursor),||Ok(branch.clone())).unwrap_err().contains("scope or query"));
        assert!(cancel("foreign",first["jobId"].as_str().unwrap()).is_err());
        cancel(&session,first["jobId"].as_str().unwrap()).unwrap();
        assert_eq!(read_with(&session,"needle",second["nextCursor"].as_str(),||Ok(branch.clone())).unwrap_err(),"read_canceled");
    }

    #[test]
    fn search_finds_a_match_split_across_decode_checkpoints() {
        let temp=super::super::tests::Temp::new();
        let text="x".repeat(CHECKPOINT_BYTES-3)+"needle end";
        let raw=json!({"type":"response_item","payload":{"type":"message","id":"message","role":"user",
            "content":[{"type":"input_text","text":text}]}}).to_string()+"\n";
        let branch=super::super::tests::indexed(&temp,&raw);
        let session=uuid::Uuid::new_v4().to_string();
        let mut cursor=None;
        let mut hits=Vec::new();
        loop {
            let result=read_with(&session,"needle",cursor.as_deref(),||Ok(branch.clone())).unwrap();
            hits.extend(result["hits"].as_array().unwrap().iter().cloned());
            cursor=result["nextCursor"].as_str().map(str::to_owned);
            if cursor.is_none() {assert_eq!(result["coverage"],"complete");break;}
        }
        assert_eq!(hits.len(),1); assert!(hits[0]["snippet"].as_str().unwrap().contains("needle"));
        assert_eq!(hits[0]["chunkSequence"],0);assert_eq!(hits[0]["offset"],CHECKPOINT_BYTES-3);
    }

    #[test]
    fn object_arguments_search_keys_numbers_and_late_content_with_complete_coverage() {
        let temp=super::super::tests::Temp::new();
        let inner=json!({"early":"x".repeat(PREVIEW_BYTES*2),"lateUniqueKey":719,"lateText":"exact needle"}).to_string();
        let raw=json!({"type":"response_item","payload":{"type":"function_call","call_id":"call","name":"Edit","arguments":inner}}).to_string()+"\n";
        let branch=super::super::tests::indexed(&temp,&raw);
        for query in ["lateUniqueKey","719","exact needle"] {
            let session=uuid::Uuid::new_v4().to_string();let mut cursor=None;let mut found=Vec::new();
            loop {
                let page=read_with(&session,query,cursor.as_deref(),||Ok(branch.clone())).unwrap();
                found.extend(page["hits"].as_array().unwrap().iter().cloned());cursor=page["nextCursor"].as_str().map(str::to_owned);
                if cursor.is_none() {assert_eq!(page["coverage"],"complete");break;}
            }
            assert_eq!(found.len(),1,"{query}");
        }
    }

    #[test]
    fn native_message_search_uses_the_displayed_body_without_metadata_duplicates() {
        let temp=super::super::tests::Temp::new();
        let raw=json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"agentMessage","id":"native",
            "text":"needle","metadata":"private metadata","content":[{"type":"text","text":"needle"}]}}}).to_string()+"\n";
        let branch=super::super::tests::indexed(&temp,&raw);let session=uuid::Uuid::new_v4().to_string();
        let page=read_with(&session,"needle",None,||Ok(branch.clone())).unwrap();assert_eq!(page["hits"].as_array().unwrap().len(),1);
        let page=read_with(&session,"private metadata",None,||Ok(branch.clone())).unwrap();assert!(page["hits"].as_array().unwrap().is_empty());
    }
}

pub fn cancel(session:&str,id:&str) -> Result<(),String> {
    let Some(jobs)=JOBS.get() else {return Ok(());};
    let job=jobs.lock().unwrap().get(id).cloned();
    if let Some(job)=job {
        let mut job=job.lock().unwrap();
        if job.session!=session {return Err("Search job is outside the selected session scope".into());}
        job.canceled.store(true,Ordering::Relaxed); job.touched=Instant::now();
    }
    Ok(())
}
