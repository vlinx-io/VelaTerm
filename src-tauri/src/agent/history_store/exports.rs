//! Session-scoped, cancellable file generation. Completion returns metadata and a download ticket.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc,Mutex,OnceLock};
use std::sync::atomic::{AtomicBool,Ordering};
use std::time::{Duration,Instant};

use serde_json::{json,Value};
use crate::{db::repo,host::AppCtx};

enum State {Pending,Ready {path:PathBuf,bytes:u64},Stream {branch:super::api::Branch,name:String,at:Option<String>},Failed(String)}
struct Job {session:String,cancel:AtomicBool,state:Mutex<State>,touched:Mutex<Instant>}
struct Task {app:AppCtx,job:Arc<Job>,destination:Option<PathBuf>,name:String,at:Option<String>}
type Jobs=Mutex<HashMap<String,Arc<Job>>>;
static JOBS:OnceLock<Jobs>=OnceLock::new();
static QUEUE:OnceLock<std::sync::mpsc::SyncSender<Task>>=OnceLock::new();

fn queue() -> &'static std::sync::mpsc::SyncSender<Task> {
    QUEUE.get_or_init(|| {
        let (sender,receiver)=std::sync::mpsc::sync_channel::<Task>(32);let receiver=Arc::new(Mutex::new(receiver));
        for _ in 0..2 {
            let receiver=receiver.clone();std::thread::spawn(move||loop {
                let task=match receiver.lock().unwrap().recv() {Ok(task)=>task,Err(_)=>return};
                let result=generate(&task);
                *task.job.state.lock().unwrap()=match result {Ok(state)=>state,Err(error)=>State::Failed(error)};
            });
        }
        sender
    })
}

pub fn start(app:&AppCtx,session:&str,destination:Option<&str>,at:Option<&str>) -> Result<Value,String> {
    let native=repo::get_session(&app.db().conn.lock().unwrap(),session)?.ok_or("Session not found")?;
    if native.agent_session_id.is_none() && native.legacy_agent_session_id.is_none() {return Err("No agent session id captured for this session yet; nothing to export".into());}
    if !matches!(native.kind,crate::models::SessionKind::Claude|crate::models::SessionKind::Codex|crate::models::SessionKind::Pi|crate::models::SessionKind::Omp|crate::models::SessionKind::Opencode|crate::models::SessionKind::Deepseek) {
        return Err("Streamed context export is unavailable for this provider".into());
    }
    if at.is_some_and(|at|at.len()>128) {return Err("Export timestamp exceeds its byte budget".into());}
    let id=format!("export:{}",uuid::Uuid::new_v4());
    let destination=destination.map(PathBuf::from);
    let job=Arc::new(Job {session:session.into(),cancel:AtomicBool::new(false),state:Mutex::new(State::Pending),touched:Mutex::new(Instant::now())});
    let mut jobs=JOBS.get_or_init(||Mutex::new(HashMap::new())).lock().unwrap();
    jobs.retain(|_,job| {
        let expired=job.touched.lock().unwrap().elapsed()>Duration::from_secs(1800);
        let state=job.state.lock().unwrap();
        if expired && !matches!(*state,State::Pending) {
            false
        } else {true}
    });
    if jobs.len()>=64 {return Err("Too many active context exports".into());}
    queue().try_send(Task {app:app.clone(),job:job.clone(),destination,name:native.name,at:at.map(str::to_owned)})
        .map_err(|_|"Context export queue is full")?;
    jobs.insert(id.clone(),job);Ok(json!({"jobId":id,"status":"pending"}))
}

fn find(session:&str,id:&str) -> Result<Arc<Job>,String> {
    let job=JOBS.get_or_init(||Mutex::new(HashMap::new())).lock().unwrap().get(id).cloned().ok_or("cursor_expired: context export expired")?;
    if job.session!=session {return Err("detail_unavailable: context export belongs to another session".into());}
    *job.touched.lock().unwrap()=Instant::now();Ok(job)
}

pub fn status(session:&str,id:&str) -> Result<Value,String> {
    let job=find(session,id)?;let state=job.state.lock().unwrap();
    match &*state {
        State::Pending=>Ok(json!({"jobId":id,"status":"pending"})),
        State::Failed(error)=>Err(error.clone()),
        State::Ready {path,bytes}=>Ok(json!({"jobId":id,"status":"complete","sizeBytes":bytes,"destPath":path,"download":null})),
        State::Stream {branch,name,at}=>Ok(json!({"jobId":id,"status":"complete",
            "download":crate::web::download::issue_markdown(branch.clone(),name,at.as_deref())?})),
    }
}

pub fn cancel(session:&str,id:&str) -> Result<(),String> {
    let job=find(session,id)?;job.cancel.store(true,Ordering::Relaxed);
    let mut state=job.state.lock().unwrap();if matches!(*state,State::Stream {..}) {*state=State::Failed("read_canceled".into());}Ok(())
}

fn generate(task:&Task) -> Result<State,String> {
    let branch=super::event_stream::complete(&task.app,&task.job.session,&task.job.cancel)?;
    let Some(destination)=&task.destination else {
        // Validate every selected record before issuing a download. The HTTP body later generates the
        // document with backpressure; no complete export is staged in the reconstructible cache.
        let mut cursor=super::event_stream::Cursor::new(branch.clone(),&task.job.cancel)?;
        while cursor.next(&task.job.cancel)?.is_some() {}
        if task.job.cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
        return Ok(State::Stream {branch,name:task.name.clone(),at:task.at.clone()});
    };
    let temporary=destination.with_file_name(format!(".vlx-export-{}.tmp",uuid::Uuid::new_v4()));
    let result=(|| {
        let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
        #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let file=options.open(&temporary).map_err(|e|format!("Cannot create context export: {e}"))?;
        let mut writer=std::io::BufWriter::with_capacity(64*1024,file);
        super::markdown::write_markdown(branch,&task.name,task.at.as_deref(),&task.job.cancel,&mut writer)?;
        writer.flush().map_err(|e|e.to_string())?;writer.get_ref().sync_all().map_err(|e|e.to_string())?;
        let bytes=writer.get_ref().metadata().map_err(|e|e.to_string())?.len();drop(writer);
        if task.job.cancel.load(Ordering::Relaxed) {return Err("read_canceled".into());}
        std::fs::rename(&temporary,destination).map_err(|e|format!("Cannot save context export: {e}"))?;Ok(State::Ready {path:destination.clone(),bytes})
    })();
    if result.is_err() {let _=std::fs::remove_file(temporary);}result
}
