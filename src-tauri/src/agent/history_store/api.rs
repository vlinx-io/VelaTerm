//! Session-authorized, byte-bounded history pages and immutable field chunks.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json,Value};
use sha2::{Digest,Sha256};

use crate::db::repo;
use crate::host::AppCtx;
use crate::models::SessionKind;
use super::limits::*;
use super::source;
use super::store::{digest,FieldMeta,RegisteredSource,State,Store};

#[derive(Clone)]
pub struct Branch {
    pub store:Arc<Store>,
    pub sources:Vec<RegisteredSource>,
    pub generation:String,
    pub kind:SessionKind,
    pub native_id:String,
    pub error:Option<String>,
    pub scope:String,
    pub(super) parent:Option<Arc<super::children::Parent>>,
}

#[cfg(test)]
impl Branch {
    /// A single-source branch over a test recording, without a session database lookup.
    pub(crate) fn test_branch(store:Arc<Store>,source:RegisteredSource,kind:SessionKind) -> Self {
        Self {store,sources:vec![source],generation:"test".into(),kind,native_id:"test".into(),
            error:None,scope:"test".into(),parent:None}
    }
}

/// Native recording sizes captured before an agent process starts. Rows that process appends belong
/// to its live timeline, so history pages for the running conversation end at these offsets.
#[derive(Clone,Debug)]
pub struct ProcessCut {pub sizes:Vec<(std::path::PathBuf,u64)>}

impl ProcessCut {
    /// Reads file metadata only; no recording byte is read before the agent starts. Database
    /// providers have no byte offset that separates earlier rows from rows the process will add.
    pub fn capture(kind:SessionKind,native_id:&str) -> Option<ProcessCut> {
        if kind.uses_opencode() || kind==SessionKind::Kiro {return None;}
        let resolved=source::resolve(kind,native_id).ok()?;
        let sizes=resolved.segments.into_iter().map(|segment| {
            let size=std::fs::metadata(&segment.path).map(|meta|meta.len()).unwrap_or(0);(segment.path,size)
        }).collect();
        Some(ProcessCut {sizes})
    }
    pub fn has_history(&self) -> bool {self.sizes.iter().any(|(_,size)|*size>0)}
}

pub fn prepare(app:&AppCtx,session_id:&str) -> Result<Branch,String> {
    let session=repo::get_session(&app.db().conn.lock().unwrap(),session_id).map_err(|e|e.to_string())?
        .ok_or("Session not found")?;
    let id=session.agent_session_id.clone().unwrap_or_default();
    let store=Store::at(&app.data_dir()?)?;
    // Between parking a retired harness id and OpenCode writing its first session row there is no
    // current recording to read. The legacy source below is the whole conversation then; a missing or
    // not-yet-written OpenCode store must not hide it behind an error.
    let current_ready = session.kind != SessionKind::Deepseek
        || id.is_empty()
        || crate::agent::opencode_store::session_exists(&id).unwrap_or(false)
        || session.legacy_agent_session_id.is_none();
    let resolved=if !current_ready {source::Resolved {segments:Vec::new(),error:None}} else if id.is_empty() {source::Resolved {segments:Vec::new(),error:None}} else {source::resolve(session.kind,&id)?};
    let mut sources=Vec::new();
    // A DeepSeek conversation first recorded by the retired harness stays in front of the OpenCode one
    // that replaced it, so continuing a conversation never hides what was said before the switch.
    if session.kind==SessionKind::Deepseek {
        if let Some(legacy)=session.legacy_agent_session_id.as_deref().filter(|legacy|*legacy!=id) {
            if let Some(path)=super::deepseek_legacy::log(legacy) {
                let native=format!("{}{legacy}",super::deepseek_legacy::NATIVE_PREFIX);
                sources.push(super::deepseek_legacy::register(&store,session.kind,path,&native)?);
            }
        }
    }
    for segment in resolved.segments {
        sources.push(match session.kind {
            SessionKind::Kiro=>super::kiro::register(&store,segment.path,&segment.native_id)?,
            SessionKind::Deepseek if segment.native_id.starts_with(super::deepseek_legacy::NATIVE_PREFIX)=>super::deepseek_legacy::register(&store,session.kind,segment.path,&segment.native_id)?,
            SessionKind::Opencode|SessionKind::Deepseek=>super::opencode::register(&store,session.kind,segment.path,&segment.native_id)?,
            kind=>store.register(kind,segment)?,
        });
    }
    bind(store,sources,session.kind,id,resolved.error,session_id)
}

pub(crate) fn bind(store:Arc<Store>,sources:Vec<RegisteredSource>,kind:SessionKind,id:String,error:Option<String>,scope:&str) -> Result<Branch,String> {
    let signature=sources.iter().map(|source| {
        let revision=source.index.read().branch_revision.clone();
        json!([source.id,source.generation,revision,source.segment.end_byte,source.segment.end_ordinal])
    }).collect::<Vec<_>>();
    let generation=digest(serde_json::to_string(&(signature,&error)).unwrap().as_bytes());
    for source in &sources {
        if kind==SessionKind::Codex && source.indexed==0 && source.size>TAIL_BYTES && source.index.read().records.is_empty() {
            // The newest rows of a long rollout are shown before the whole file has been read.
            store.scan(kind,source,&std::sync::atomic::AtomicBool::new(false),true)?;
        }
        store.schedule(kind,source.clone());
    }
    Ok(Branch {store,sources,generation,kind,native_id:id,error,scope:scope.into(),parent:None})
}

fn field_view(reference:&str,label:&str,path:&str,size:u64,encoding:&str,append:bool,revision:&str) -> Value {
    let mut field=json!({"detailRef":reference,"label":label,"path":path,"sizeBytes":size,
        "kind":if encoding=="json" {"json"} else if path.ends_with("diff") {"diff"} else {"text"},"append":append,"contentRevision":revision});
    if let Some(label)=label.strip_prefix("attachment:") {field["kind"]=json!("image");field["label"]=json!("attachment");field["mimeType"]=json!(label.rsplit_once(':').map(|(mime,_)|mime));}
    else if let Some(rest)=label.strip_prefix("attachment_file:") {
        let (mime,len)=rest.rsplit_once(':').unwrap_or((rest,""));
        field["kind"]=json!("image");field["label"]=json!("attachment");field["mimeType"]=json!(mime);
        if let Ok(file_len)=len.parse::<u64>() {field["sizeBytes"]=json!(file_len);}
    }
    match label {
        "body_shell"=>{field["label"]=json!("body");field["prefix"]=json!("!");},
        "body_file"=>{field["label"]=json!("body");field["prefix"]=json!("[file: ");field["suffix"]=json!("]");},
        "body_image"=>{field["label"]=json!("body");field["literal"]=json!("[image]");},
        _=>{},
    }field
}

/// Whether a record of `source` is on the selected branch and inside the segment's limits.
pub(super) fn record_visible(state:&State,source:&RegisteredSource,start:u64) -> bool {
    let Some(record)=state.records.get(&start) else {return false;};
    record.active && source.segment.end_byte.is_none_or(|end|record.end<=end)
        && source.segment.end_ordinal.is_none_or(|end|record.ordinal.is_none_or(|ordinal|ordinal<end))
}

impl Branch {
    /// Whether a row of `source` belongs to the conversation view, its record ending at `end` at most.
    pub(super) fn visible(&self,state:&State,source:&RegisteredSource,id:&str,end:Option<u64>) -> bool {
        let Some(row)=state.rows.get(id) else {return false;};
        if end.is_some_and(|end|row.record_end>end) || source.segment.end_ordinal.is_some_and(|limit|row.ordinal.is_some_and(|ordinal|ordinal>=limit)) {return false;}
        let Some(record)=state.records.get(&row.record_start) else {return false;};
        if !record.active || state.superseded.contains_key(id) {return false;}
        // A Codex child thread presents only its completed items, not the context it inherited.
        if self.kind==SessionKind::Codex && self.scope.starts_with("child:") && !matches!(record.subtype.as_deref(),Some("item_completed"|"itemCompleted")) {return false;}
        true
    }

    /// Visible rows of one source in order, ending at `end`.
    pub(super) fn ordered(&self,state:&State,source:&RegisteredSource,end:Option<u64>) -> Vec<((u64,u32),Arc<str>)> {
        state.order.iter().filter(|(_,id)|self.visible(state,source,id,end)).map(|(order,id)|(*order,id.clone())).collect()
    }

    pub(super) fn resolve_row(&self,id:&str) -> String {
        // A superseded row is located through the row that stands for it in the view.
        for source in &self.sources {
            if let Some(replacement)=source.index.read().superseded.get(id) {return replacement.clone();}
        }
        id.into()
    }

    pub fn detail_facts(&self,reference:&str,expected:Option<&str>) -> Result<(String,u64),String> {
        let (source,field)=self.authorize(reference)?;
        let revision=format!("{}:{reference}",source.generation);
        if expected.is_some_and(|value|value!=revision) {return Err("source_changed: detail revision changed".into());}
        Ok((revision,field.decoded_bytes))
    }

    pub fn detail_sequence_at(&self,reference:&str,offset:u64) -> Result<u64,String> {
        let (source,field)=self.authorize(reference)?;
        if offset>=field.decoded_bytes && field.decoded_bytes>0 {return Err("cursor_expired: detail range is outside its field".into());}
        super::fields::sequence_at(&source,&field,offset)
    }

    pub fn write_detail(&self,reference:&str,expected:Option<&str>,writer:&mut impl std::io::Write,cancel:&std::sync::atomic::AtomicBool) -> Result<u64,String> {
        let (revision,size)=self.detail_facts(reference,expected)?;
        let mut written=0;let mut sequence=0;
        while written<size {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {return Err("read_canceled".into());}
            let value=self.detail(reference,Some(&revision),sequence)?;
            let text=value["text"].as_str().ok_or("detail_unavailable: detail chunk is unavailable")?;
            writer.write_all(text.as_bytes()).map_err(|_|"storage_pressure: detail file could not be written")?;
            written+=text.len() as u64;sequence+=1;
            if value["eof"]==true {break;}
        }
        if written!=size {return Err("source_changed: detail size changed during the transfer".into());}
        Ok(written)
    }

    pub(super) fn cursor(&self,operation:&str,owner:&str,sequence:u64) -> String {
        let bound=serde_json::to_vec(&json!([2,self.scope,self.generation,operation,owner,sequence])).unwrap();
        let mut hash=Sha256::new();hash.update(self.store.cursor_key.as_bytes());hash.update(bound);
        let signature=hash.finalize().iter().map(|b|format!("{b:02x}")).collect::<String>();
        format!("v2.{sequence}.{signature}")
    }

    pub(super) fn cursor_sequence(&self,operation:&str,owner:&str,cursor:Option<&str>) -> Result<u64,String> {
        let Some(cursor)=cursor else {return Ok(0);};
        if cursor.len()>96 {return Err("cursor_expired: invalid history cursor".into());}
        let mut parts=cursor.split('.');
        if parts.next()!=Some("v2") {return Err("cursor_expired: unsupported history cursor".into());}
        let sequence=parts.next().and_then(|p|p.parse::<u64>().ok()).ok_or("cursor_expired: invalid history cursor")?;
        if self.cursor(operation,owner,sequence)!=cursor {return Err("cursor_expired: history cursor belongs to another scope or revision".into());}
        Ok(sequence)
    }

    pub fn detail_page(&self,reference:&str,expected:Option<&str>,cursor:Option<&str>) -> Result<Value,String> {
        let sequence=self.cursor_sequence("detail",reference,cursor)?;
        let mut value=self.detail(reference,expected,sequence)?;
        value["nextCursor"]=if value["eof"]==true {Value::Null} else {json!(self.cursor("detail",reference,sequence+1))};
        value["previousCursor"]=if sequence==0 {Value::Null} else {json!(self.cursor("detail",reference,sequence-1))};
        if serde_json::to_vec(&value).map_err(|e|e.to_string())?.len()>DETAIL_BYTES {return Err("History detail exceeds its wire byte budget".into());}
        Ok(value)
    }

    pub fn manifest_page(&self,row_id:&str,cursor:Option<&str>) -> Result<Value,String> {
        let from=self.cursor_sequence("manifest",row_id,cursor)?;
        let mut value=self.manifest(row_id,from)?;
        value["nextCursor"]=value["nextCursor"].as_u64().map(|next|json!(self.cursor("manifest",row_id,next))).unwrap_or(Value::Null);
        value["previousCursor"]=if from==0 {Value::Null} else {json!(self.cursor("manifest",row_id,from.saturating_sub(50)))};
        Ok(value)
    }

    /// A copied locator can mint fresh revision-bound cursors after a process or transport reconnect.
    /// The client supplies a logical chunk number, never a source path or byte offset.
    pub fn locate_detail(&self,row_id:&str,reference:&str,sequence:u64) -> Result<Value,String> {
        let (source,field)=self.authorize(reference)?;
        let row_id=self.resolve_row(row_id);
        let state=source.current()?;
        let label=state.row_fields.get(row_id.as_str()).and_then(|fields|fields.iter().find(|candidate|candidate.reference==reference)).map(|field|field.label.clone())
            .ok_or("detail_unavailable: field belongs to another history row")?;
        let revision=format!("{}:{reference}",source.generation);
        Ok(json!({"field":field_view(reference,&label,&field.path,field.decoded_bytes,&field.encoding,false,&revision),
            "cursor":self.cursor("detail",reference,sequence),"historyGeneration":self.generation}))
    }

    pub(super) fn render_row(&self,state:&State,source:&RegisteredSource,id:&str) -> Result<Value,String> {
        let summary=state.summary(id).ok_or("detail_unavailable: history row is unavailable")?;
        let row:Value=serde_json::from_str(summary).map_err(|e|e.to_string())?;
        if self.kind.uses_opencode() {return Ok(super::children::public_row(super::attachments::decorate(state,source,row)));}
        let mut result:Option<Value>=None;
        for (start,part) in state.parts.get(id).into_iter().flatten() {
            if !state.active(*start) || source.segment.end_byte.is_some_and(|end|part.record_end>end)
                || source.segment.end_ordinal.is_some_and(|end|part.ordinal.is_some_and(|ordinal|ordinal>=end)) {continue;}
            let mut value:Value=serde_json::from_str(&part.summary).map_err(|e|e.to_string())?;
            if let Some(old)=result.take() {
                super::children::merge(&old,&mut value);
                if value["kind"]=="tool" && value.get("output").is_some() {
                    value["name"]=old["name"].clone();value["input"]=old["input"].clone();
                    if value.get("_child").is_none() {if let Some(child)=old.get("_child") {value["_child"]=child.clone();}}
                } else if value["kind"]=="tool" && old.get("output").is_some() {
                    value["output"]=old["output"].clone();value["status"]=old["status"].clone();value["isError"]=old["isError"].clone();
                }
            }
            result=Some(value);
        }
        Ok(super::children::public_row(super::attachments::decorate(state,source,result.unwrap_or(row))))
    }

    /// Visible rows in order after `after`, at most one page; each with its position key.
    pub fn forward(&self,after:Option<&str>) -> Result<Vec<(String,Value)>,String> {
        let mut result=Vec::new();
        for (sequence,source) in self.sources.iter().enumerate() {
            let state=source.current()?;
            for (order,id) in self.ordered(&state,source,source.segment.end_byte) {
                let key=order_key(sequence,order);
                if after.is_some_and(|after|key.as_str()<=after) {continue;}
                result.push((key,self.render_row(&state,source,&id)?));
                if result.len()>=PAGE_ROWS {return Ok(result);}
            }
        }
        Ok(result)
    }

    pub fn authorize(&self,reference:&str) -> Result<(RegisteredSource,FieldMeta),String> {
        super::children::validate_parent(self)?;
        for source in &self.sources {
            let state=source.index.read();
            let Some(field)=state.fields.get(reference) else {continue;};
            if state.generation!=source.generation || state.stale {return Err("source_changed: detail belongs to an expired history generation".into());}
            if !record_visible(&state,source,field.record_start) {return Err("detail_unavailable: detail is outside the selected history branch".into());}
            let owned=state.field_rows.get(reference).into_iter().flatten().any(|row| {
                state.rows.get(row).is_some_and(|row|state.active(row.record_start) && source.segment.end_byte.is_none_or(|end|row.record_end<=end)
                    && source.segment.end_ordinal.is_none_or(|end|row.ordinal.is_none_or(|ordinal|ordinal<end)))
            });
            if !owned {return Err("detail_unavailable: detail is outside the selected history branch".into());}
            return Ok((source.clone(),field.clone()));
        }
        Err("detail_unavailable: history detail is unavailable".into())
    }

    pub fn detail(&self,reference:&str,expected:Option<&str>,sequence:u64) -> Result<Value,String> {
        let (source,field)=self.authorize(reference)?;
        let revision=format!("{}:{reference}",source.generation);
        if expected.is_some_and(|value|value!=revision) {return Err("source_changed: detail revision changed".into());}
        let (text,decoded_start,decoded_end,size)=super::fields::read_piece(&source,&field,sequence,0)?;
        let eof=decoded_end>=size;
        let result=json!({"contentRevision":revision,"chunkSequence":sequence,"text":text,"nextCursor":if eof {None} else {Some(sequence+1)},
            "eof":eof,"decodedStart":decoded_start,"decodedEnd":decoded_end});
        if serde_json::to_vec(&result).map_err(|e|e.to_string())?.len()>DETAIL_BYTES {return Err("History detail exceeds its wire byte budget".into());}
        Ok(result)
    }

    /// The same branch ending where the running process began: native sources stop at the sizes
    /// captured before launch. The generation stays the full branch's, so details read from these
    /// rows authorize against the full branch.
    pub fn before_process(&self,cut:&ProcessCut) -> Result<Branch,String> {
        let sources=self.sources.iter().map(|source| {
            let mut source=source.clone();
            // A recording that did not exist at launch was written entirely by this process.
            let size=cut.sizes.iter().find(|(path,_)|*path==source.segment.path).map_or(0,|(_,size)|*size);
            source.segment.end_byte=Some(source.segment.end_byte.map_or(size,|end|end.min(size)));source
        }).collect();
        Ok(Branch {sources,..self.clone()})
    }

    /// The row of a message that global search located by the offset of its first record in the native
    /// recording: that record's message row, or the next selected row when the record has none.
    pub fn row_at_record(&self,offset:u64) -> Result<Option<String>,String> {
        // Search reads the conversation's own recording; a Codex fork's inherited parent intervals are
        // other files, where the same offset means something else.
        let own=self.sources.iter().filter(|source|source.segment.native_id==self.native_id).collect::<Vec<_>>();
        let sources=if own.is_empty() {self.sources.iter().collect()} else {own};
        for source in sources {
            let state=source.current()?;
            for (start,ids) in state.record_rows.range(offset..) {
                if !state.active(*start) {continue;}
                let mut candidates=Vec::new();
                for id in ids {
                    let Some(part)=state.parts.get(&**id).and_then(|parts|parts.get(start)) else {continue;};
                    if source.segment.end_byte.is_some_and(|end|part.record_end>end) {continue;}
                    let kind=serde_json::from_str::<Value>(&part.summary).ok().and_then(|value|value["kind"].as_str().map(str::to_owned)).unwrap_or_default();
                    candidates.push((!matches!(kind.as_str(),"user"|"assistant"),id.clone()));
                }
                candidates.sort();
                if let Some((_,id))=candidates.into_iter().next() {return Ok(Some(id.to_string()));}
            }
        }
        Ok(None)
    }

    pub fn page(&self,before:Option<&str>,after:Option<&str>,anchor:Option<&str>) -> Result<Value,String> {
        self.page_with(before,after,anchor,false)
    }

    /// `live` marks a branch from [`Branch::before_process`]: the running agent keeps appending past
    /// the cut, and a tree provider's earlier rows stay addressable once the scan has passed the cut.
    pub fn page_with(&self,before:Option<&str>,after:Option<&str>,anchor:Option<&str>,live:bool) -> Result<Value,String> {
        super::children::validate_parent(self)?;
        let mut candidates:Vec<(String,Value,u64)>=Vec::new();
        let mut indexed=0u64;let mut bytes=0u64;let mut known=0u64;
        let mut complete=self.error.is_none();let mut indexing=false;let mut error=self.error.clone();
        let boundary=before.or(after).or(anchor);
        let mut located=None;let mut canonical_anchor=None;
        if let Some(id)=boundary {
            let id=self.resolve_row(id);
            if anchor.is_some() {canonical_anchor=Some(id.clone());}
            for (sequence,source) in self.sources.iter().enumerate() {
                let state=source.current()?;
                if self.visible(&state,source,&id,source.segment.end_byte) {located=Some(order_key(sequence,state.rows[id.as_str()].order));break;}
            }
            if located.is_none() {
                let mut pending=self.page_with(None,None,None,live)?;
                if pending["indexing"]==true {
                    let mut location=serde_json::Map::new();
                    for (key,value) in [("before",before),("after",after),("anchor",anchor)] {
                        if let Some(value)=value {location.insert(key.into(),json!(value));}
                    }
                    pending["pendingLocation"]=Value::Object(location);
                    pending["rows"]=json!([]);pending["positions"]=json!({});pending["hasMore"]=json!(false);pending["hasNewer"]=json!(false);
                    return Ok(pending);
                }
                return Err("cursor_expired: history row is outside the current branch".into());
            }
        }
        for (sequence,source) in self.sources.iter().enumerate() {
            let state=source.current()?;
            let water=state.indexed;let limit=source.segment.end_byte.unwrap_or(source.size);
            indexed+=water.min(limit);bytes+=limit;
            complete&=state.status=="complete" || source.segment.end_byte.is_some_and(|end|water>=end);
            indexing|=state.status=="partial" && water<limit;
            error=error.or(state.error.clone()).or_else(||(state.status=="error").then(||"History contains unsupported or malformed records".into()));
            // Tree providers cannot claim an active branch before its leaf has been confirmed. While an
            // appended tail is read, the branch confirmed by the last complete scan stays visible.
            let mut end=source.segment.end_byte;
            if matches!(self.kind,SessionKind::Claude|SessionKind::Pi|SessionKind::Omp) && state.status!="complete"
                && !(live && source.segment.end_byte.is_some_and(|end|water>=end)) {
                match state.branch_mark.filter(|mark|*mark>0) {Some(mark)=>end=Some(end.map_or(mark,|end|end.min(mark))),None=>continue}
            }
            let ordered=self.ordered(&state,source,end);
            let count=ordered.len() as u64;
            let window:Vec<usize>=match (&located,after.is_some(),anchor.is_some()) {
                (Some(at),true,_)=>ordered.iter().enumerate().filter(|(_,(order,_))|order_key(sequence,*order).as_str()>at.as_str()).map(|(index,_)|index).take(PAGE_ROWS+1).collect(),
                (Some(at),false,true)=> {
                    let found=ordered.iter().enumerate().filter(|(_,(order,_))|order_key(sequence,*order).as_str()<=at.as_str()).map(|(index,_)|index).collect::<Vec<_>>();
                    found[found.len().saturating_sub(PAGE_ROWS+1)..].to_vec()
                },
                (Some(at),false,false)=> {
                    let found=ordered.iter().enumerate().filter(|(_,(order,_))|order_key(sequence,*order).as_str()<at.as_str()).map(|(index,_)|index).collect::<Vec<_>>();
                    found[found.len().saturating_sub(PAGE_ROWS+1)..].to_vec()
                },
                (None,_,_)=>(ordered.len().saturating_sub(PAGE_ROWS+1)..ordered.len()).collect(),
            };
            for index in window {
                let (order,id)=&ordered[index];
                let value=self.render_row(&state,source,id)?;
                candidates.push((order_key(sequence,*order),value,known+index as u64));
            }
            known+=count;
        }
        candidates.sort_by(|a,b|a.0.cmp(&b.0));
        if after.is_some() {candidates.truncate(PAGE_ROWS);}
        else if candidates.len()>PAGE_ROWS {candidates.drain(..candidates.len()-PAGE_ROWS);}
        let mut rows=Vec::new();
        let mut positions=HashMap::new();
        for (_,row,position) in candidates {
            positions.insert(row["id"].as_str().unwrap_or_default().to_owned(),position);
            rows.push(row);
        }
        let coverage=if error.is_some() {"error"} else if complete {"complete"} else {"partial"};
        let mut result=json!({"protocolVersion":2,"historyGeneration":self.generation,"processEpoch":null,
            "snapshotRevision":indexed.to_string(),"coverage":coverage,"indexing":indexing,"indexedBytes":indexed,"sourceBytes":bytes,"knownRows":known,
            "totalRows":if complete {Some(known)} else {None},"rowsRevision":indexed,"positions":positions,"rows":rows,
            "pageKind":if before.is_some() {"history"} else {"recent"},"hasMore":false,"historyError":error,"agentSessionId":self.native_id});
        if let Some(id)=canonical_anchor {result["locatedRowId"]=json!(id);}
        // The message rail covers the whole conversation, not only the rows of this page.
        if before.is_none() && after.is_none() {result["userMessages"]=self.outline(live)?;}
        while serde_json::to_vec(&result).map_err(|e|e.to_string())?.len()>PAGE_BYTES-16*1024 {
            let rows=result["rows"].as_array_mut().unwrap();
            if rows.len()<=1 {return Err("History row exceeds its page byte budget".into());}
            let removed=rows.remove(if after.is_some() {rows.len()-1} else {0});
            result["positions"].as_object_mut().unwrap().remove(removed["id"].as_str().unwrap());
        }
        let first=result["rows"].as_array().and_then(|r|r.first()).and_then(|r|r["id"].as_str());
        let last=result["rows"].as_array().and_then(|r|r.last()).and_then(|r|r["id"].as_str());
        let has_more=first.and_then(|id|result["positions"][id].as_u64()).is_some_and(|position|position>0);
        let has_newer=last.and_then(|id|result["positions"][id].as_u64()).is_some_and(|position|position+1<known);
        result["hasMore"]=json!(has_more);
        result["hasNewer"]=json!(has_newer);
        Ok(result)
    }

    /// Every user message of the branch in order, with a short preview, for the message rail. Previews
    /// shrink, and then the oldest messages are left out, to stay within [`OUTLINE_BYTES`].
    pub(super) fn outline(&self,live:bool) -> Result<Value,String> {
        let mut messages=Vec::new();
        for source in &self.sources {
            let state=source.current()?;
            let mut end=source.segment.end_byte;
            if matches!(self.kind,SessionKind::Claude|SessionKind::Pi|SessionKind::Omp) && state.status!="complete"
                && !(live && source.segment.end_byte.is_some_and(|end|state.indexed>=end)) {
                match state.branch_mark.filter(|mark|*mark>0) {Some(mark)=>end=Some(end.map_or(mark,|end|end.min(mark))),None=>continue}
            }
            for (_,id) in self.ordered(&state,source,end) {
                if &*state.rows[&*id].kind!="user" {continue;}
                let Some(summary)=state.summary(&id).and_then(|summary|serde_json::from_str::<Value>(summary).ok()) else {continue;};
                let text=summary["text"].as_str().unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ");
                let images=state.row_fields.get(&*id).is_some_and(|fields|fields.iter().any(|field|field.label.starts_with("attachment:")||field.label.starts_with("attachment_file:")));
                messages.push((id.to_string(),text,images));
            }
        }
        let render=|chars:usize,from:usize| Value::Array(messages[from..].iter().map(|(id,text,images)|
            json!({"id":id,"text":text.chars().take(chars).collect::<String>(),"hasImages":images})).collect());
        let size=|value:&Value|serde_json::to_vec(value).map(|bytes|bytes.len()).unwrap_or(usize::MAX);
        for chars in [240,80,24] {
            let value=render(chars,0);if size(&value)<=OUTLINE_BYTES {return Ok(value);}
        }
        let mut from=0;let mut value=render(24,0);
        while size(&value)>OUTLINE_BYTES && from<messages.len() {from+=(messages.len()-from)/8+1;value=render(24,from);}
        Ok(value)
    }

    /// Fields of a row a reader may open: the newest value of each, or every piece of an appended one.
    pub(super) fn row_fields(&self,state:&State,source:&RegisteredSource,row:&str) -> Vec<(super::store::RowField,FieldMeta)> {
        let Some(list)=state.row_fields.get(row) else {return Vec::new();};
        let mut fields=list.iter().filter_map(|field|state.fields.get(&field.reference).map(|meta|(field.clone(),meta.clone())))
            .filter(|(_,meta)|record_visible(state,source,meta.record_start)).collect::<Vec<_>>();
        let newest=fields.iter().fold(HashMap::<(String,String),u64>::new(),|mut newest,(field,meta)| {
            let entry=newest.entry((field.label.clone(),meta.path.clone())).or_insert(meta.record_start);*entry=(*entry).max(meta.record_start);newest
        });
        fields.retain(|(field,meta)|state.append.contains(&field.reference) || newest[&(field.label.clone(),meta.path.clone())]==meta.record_start);
        fields.sort_by_key(|(field,_)|field.sequence);fields
    }

    pub fn manifest(&self,row_id:&str,from:u64) -> Result<Value,String> {
        super::children::validate_parent(self)?;
        let row_id=self.resolve_row(row_id);
        for source in &self.sources {
            let state=source.current()?;
            if !self.visible(&state,source,&row_id,source.segment.end_byte) {continue;}
            let row=self.render_row(&state,source,&row_id)?;
            let mut fields=self.row_fields(&state,source,&row_id).into_iter().skip(from as usize).take(51)
                .map(|(field,meta)|field_view(&field.reference,&field.label,&meta.path,meta.decoded_bytes,&meta.encoding,
                    state.append.contains(&field.reference) && !source.is_virtual(),&format!("{}:{}",source.generation,field.reference))).collect::<Vec<_>>();
            let more=fields.len()>50;
            fields.truncate(50);
            let changes=state.files.get(row_id.as_str()).map(|files| {
                let latest=files.keys().filter(|(start,_)|record_visible(&state,source,*start)).map(|(start,_)|*start).max();
                files.iter().filter(|((start,_),_)|Some(*start)==latest).map(|(_,file)|file.clone()).collect::<Vec<_>>()
            }).unwrap_or_default();
            let mut files=changes.into_iter().skip(from as usize).take(51).map(|file|json!({"path":file.path,"changeType":file.change,"detailRef":file.detail,"kind":file.kind,
                "contentRevision":file.detail.as_ref().map(|r|format!("{}:{r}",source.generation))})).collect::<Vec<_>>();
            let more=more||files.len()>50;
            files.truncate(50);
            let value=json!({"protocolVersion":2,"historyGeneration":self.generation,"row":row,"fields":fields,"files":files,"nextCursor":if more {Some(from+50)} else {None}});
            if serde_json::to_vec(&value).map_err(|e|e.to_string())?.len()>PAGE_BYTES {return Err("History manifest exceeds its wire byte budget".into());}
            return Ok(value);
        }
        Err("detail_unavailable: history row is unavailable".into())
    }
}

/// Bytes of the user-message outline carried by a page.
const OUTLINE_BYTES:usize=96*1024;

/// The position key of a row: its source's place in the branch, then its order within that source.
pub(super) fn order_key(sequence:usize,(start,local):(u64,u32)) -> String {format!("{sequence:04}:{start:020}:{local:06}")}
