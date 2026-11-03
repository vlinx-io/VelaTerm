//! A writer lease is an OS lock, never a persisted PID. Old PID records are only probe evidence.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use sysinfo::{Pid, ProcessesToUpdate, ProcessRefreshKind, System};
use crate::host::AppCtx;

pub const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS chat_process_owners (
 session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
 token TEXT NOT NULL, host_pid INTEGER NOT NULL, host_birth INTEGER NOT NULL,
 child_pid INTEGER, child_birth INTEGER
);";

fn birth(pid: u32) -> Option<u64> {
    let mut system = System::new();
    let pid = Pid::from_u32(pid);
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing());
    system.process(pid).map(|p|p.start_time())
}

pub(super) fn process_may_exist(pid: u32) -> bool {
    if birth(pid).is_some() { return true; }
    #[cfg(unix)] {
        if pid>i32::MAX as u32 { return false; }
        // EPERM proves existence too; only ESRCH proves that the old writer is absent.
        return unsafe { libc::kill(pid as i32,0) }==0 || std::io::Error::last_os_error().raw_os_error()!=Some(libc::ESRCH);
    }
    #[cfg(windows)] { windows_process_may_exist(pid) }
    #[cfg(not(any(unix,windows)))] { true }
}

#[cfg(windows)]
fn windows_process_may_exist(pid: u32) -> bool {
    use ::windows::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER};
    use ::windows::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    const STILL_ACTIVE: u32=259;
    if pid==0 { return true; }
    let handle=match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid) } {
        Ok(handle)=>handle,
        // Only an invalid process ID proves absence. Access denial or a query failure is uncertainty.
        Err(error)=>return error.code()!=::windows::core::HRESULT::from_win32(ERROR_INVALID_PARAMETER.0),
    };
    let mut code=0;
    let queried=unsafe { GetExitCodeProcess(handle,&mut code) }.is_ok();
    let _=unsafe { CloseHandle(handle) };
    !queried || code==STILL_ACTIVE
}

fn may_be_same_process(pid: u32, expected: u64) -> bool {
    match birth(pid) {
        Some(actual) => actual==expected || actual==0,
        None => process_may_exist(pid),
    }
}

fn process_group_may_exist(pid: u32) -> bool {
    #[cfg(unix)] {
        if pid==0 || pid>i32::MAX as u32 { return false; }
        // An orphaned descendant can outlive the group leader. Probe only; never kill persisted IDs.
        return unsafe { libc::kill(-(pid as i32),0) }==0 || std::io::Error::last_os_error().raw_os_error()!=Some(libc::ESRCH);
    }
    #[cfg(not(unix))] { let _=pid; false }
}

fn previous_child_may_write(pid: u32, stamp: u64) -> bool {
    match birth(pid) {
        Some(actual) => actual==stamp || actual==0,
        // A process group ID is never reused while the group has members, so a live process with a
        // different birth means the old group is gone. Only an absent leader leaves orphans to probe.
        None => process_may_exist(pid) || process_group_may_exist(pid),
    }
}

fn lock(app: &AppCtx, key: &str, drain_grace: bool) -> Result<File, String> {
    let dir = if key.starts_with("native:") {
        crate::host::home_dir().ok_or("Cannot locate the agent ownership directory")?.join(".config/velaterm/chat-owners")
    } else { app.data_dir()?.join("chat-owners") };
    std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
    let path = dir.join(format!("{:x}.lock",Sha256::digest(key.as_bytes())));
    let file = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path).map_err(|e|e.to_string())?;
    let mut attempts=if drain_grace { 20 } else { 0 };
    loop {
        if file.try_lock().is_ok() { break; }
        if attempts==0 { return Err("Another process still owns this conversation. Wait for it to exit before continuing.".into()); }
        // A killed descendant may retain the inherited descriptor briefly after its root is reaped.
        attempts-=1; std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(file)
}

/// Hold this guard while editing a dormant queue, so another host cannot start using stale contents.
pub fn idle_guard(app: &AppCtx, session: &str) -> Result<Vec<File>, String> {
    idle_guard_with(app,session,false)
}

pub(super) fn handoff_guard(app: &AppCtx, session: &str) -> Result<Vec<File>, String> {
    idle_guard_with(app,session,true)
}

fn idle_guard_with(app: &AppCtx, session: &str, drain_grace: bool) -> Result<Vec<File>, String> {
    let file=lock(app,&format!("session:{session}"),drain_grace)?;
    let child: Option<(String,Option<u32>,Option<u64>)> = app.db().conn.lock().unwrap().query_row("SELECT token,child_pid,child_birth FROM chat_process_owners WHERE session_id=?1",[session],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|e|e.to_string())?;
    if child.as_ref().is_some_and(|(_,pid,stamp)|pid.zip(*stamp).is_some_and(|(pid,stamp)|previous_child_may_write(pid,stamp))) {
        return Err("The previous agent is still running. Stop that process before continuing this conversation.".into());
    }
    if child.as_ref().is_some_and(|(_,pid,_)|pid.is_none()) && cfg!(not(unix)) {
        return Err("The previous agent's startup could not be verified. Process ownership requires inspection before this conversation can continue.".into());
    }
    // The lease is ours and the recorded writer is proven gone: drop the stale record so later
    // probes cannot match a reused process ID.
    if let Some((token,_,_))=child {
        if app.db().conn.lock().unwrap().execute("DELETE FROM chat_process_owners WHERE session_id=?1 AND token=?2",params![session,token]).is_err() {
            crate::diagnostic_warn!("chat ownership: could not clear stale process evidence");
        }
    }
    let native: Option<(String,Option<String>)> = app.db().conn.lock().unwrap().query_row("SELECT kind,agent_session_id FROM sessions WHERE id=?1",[session],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
    let mut files=vec![file];
    if let Some((kind,Some(id)))=native {
        let native=lock(app,&format!("native:{kind}:{id}"),drain_grace)?;
        if let Some(mut previous)=native_evidence(&native)? {
            if previous_native_may_write(&previous) {
                return Err("The previous agent still owns this native conversation. Wait for that process to exit before continuing.".into());
            }
            if !previous.retired {
                previous.retired=true;
                if record_native(&native,&previous).is_err() { crate::diagnostic_warn!("chat ownership: could not retire stale native process evidence"); }
            }
        }
        files.push(native);
    }
    Ok(files)
}

pub struct Owner {
    pub token: String,
    files: Vec<File>,
    native_key: Option<String>,
    child: Option<(u32,u64)>,
}

#[derive(serde::Serialize,serde::Deserialize)]
struct NativeEvidence {
    token: String, host: u32, host_birth: u64, child: Option<(u32,u64)>, retired: bool,
}

fn previous_native_may_write(previous: &NativeEvidence) -> bool {
    !previous.retired && (previous.child.is_some_and(|(pid,stamp)|previous_child_may_write(pid,stamp))
        || previous.child.is_none() && (may_be_same_process(previous.host,previous.host_birth) && previous.host!=std::process::id() || cfg!(not(unix))))
}

fn native_evidence(file: &File) -> Result<Option<NativeEvidence>, String> {
    let mut file=file;
    file.seek(SeekFrom::Start(0)).map_err(|e|e.to_string())?;
    let mut raw=String::new(); file.read_to_string(&mut raw).map_err(|e|e.to_string())?;
    if raw.is_empty() { return Ok(None); }
    // A torn final record is uncertainty, even if an older owner was already retired.
    serde_json::from_str(raw.trim().lines().last().unwrap_or("")).map(Some)
        .map_err(|_|"Native conversation ownership could not be verified. Inspect its previous process before continuing.".into())
}

fn record_native(file: &File, evidence: &NativeEvidence) -> Result<(), String> {
    let mut file=file;
    file.seek(SeekFrom::End(0)).map_err(|e|e.to_string())?;
    let line=format!("{}\n",serde_json::to_string(evidence).map_err(|e|e.to_string())?);
    file.write_all(line.as_bytes()).and_then(|_|file.sync_data()).map_err(|e|e.to_string())
}

impl Owner {
    pub fn acquire(app: &AppCtx, session: &str, native: Option<String>) -> Result<Self, String> {
        let file = lock(app,&format!("session:{session}"),true)?;
        let previous: Option<(u32,u64,Option<u32>,Option<u64>)> = app.db().conn.lock().unwrap().query_row(
            "SELECT host_pid,host_birth,child_pid,child_birth FROM chat_process_owners WHERE session_id=?1",[session],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|e|e.to_string())?;
        if let Some((host,host_birth,child,child_birth)) = previous {
            if may_be_same_process(host,host_birth) && host!=std::process::id() {
                return Err("Another application instance owns this conversation.".into());
            }
            if child.zip(child_birth).is_some_and(|(pid,stamp)|previous_child_may_write(pid,stamp)) {
                // An unattachable orphan may still be writing the native log. Never start a second writer
                // or kill a PID whose full identity we cannot prove through an owned Child handle.
                return Err("The previous agent is still running. Stop that process before continuing this conversation.".into());
            }
            if child.is_none() && cfg!(not(unix)) {
                return Err("The previous agent's startup could not be verified. Process ownership requires inspection before this conversation can continue.".into());
            }
        }
        let mut owner = Self { token:uuid::Uuid::new_v4().to_string(),files:vec![file],native_key:None,child:None };
        if let Some(key) = native { owner.native(app,&key)?; }
        app.db().conn.lock().unwrap().execute("INSERT INTO chat_process_owners(session_id,token,host_pid,host_birth) VALUES (?1,?2,?3,?4) ON CONFLICT(session_id) DO UPDATE SET token=excluded.token,host_pid=excluded.host_pid,host_birth=excluded.host_birth,child_pid=NULL,child_birth=NULL",
            params![session,owner.token,std::process::id(),birth(std::process::id()).ok_or("Cannot verify application process identity")?]).map_err(|e|e.to_string())?;
        Ok(owner)
    }

    pub fn native(&mut self, app: &AppCtx, key: &str) -> Result<(), String> {
        if self.native_key.as_deref()==Some(key) { return Ok(()) }
        let file=lock(app,&format!("native:{key}"),true)?;
        if let Some(previous)=native_evidence(&file)?.filter(|v|!v.retired && v.token!=self.token) {
            if previous_native_may_write(&previous) { return Err("The previous agent still owns this native conversation. Wait for that process to exit before continuing.".into()); }
        }
        record_native(&file,&self.evidence(false)?)?;
        self.files.push(file);
        self.native_key=Some(key.into());
        Ok(())
    }

    fn evidence(&self, retired: bool) -> Result<NativeEvidence,String> {
        Ok(NativeEvidence { token:self.token.clone(),host:std::process::id(),host_birth:birth(std::process::id()).ok_or("Cannot verify application process identity")?,child:self.child,retired })
    }

    /// Descendants retain the lease if the host is killed between spawn and PID persistence.
    #[cfg(unix)]
    pub fn inherit(&self, command: &mut std::process::Command) {
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt;
        let fds: Vec<_> = self.files.iter().map(AsRawFd::as_raw_fd).collect();
        unsafe { command.pre_exec(move || {
            for fd in &fds {
                if libc::fcntl(*fd,libc::F_SETFD,0) == -1 { return Err(std::io::Error::last_os_error()); }
            }
            Ok(())
        }); }
    }

    pub fn launched(&mut self, app: &AppCtx, session: &str, pid: u32) -> Result<(), String> {
        self.child=Some((pid,birth(pid).ok_or("Cannot verify agent process identity")?));
        app.db().conn.lock().unwrap().execute("UPDATE chat_process_owners SET child_pid=?3,child_birth=?4 WHERE session_id=?1 AND token=?2",
            params![session,self.token,pid,self.child.unwrap().1]).map_err(|e|e.to_string())?;
        for file in self.files.iter().skip(1) { record_native(file,&self.evidence(false)?)?; }
        Ok(())
    }

    pub fn retire(&self, app: &AppCtx, session: &str) {
        let drained=!self.child.is_some_and(|(pid,stamp)|previous_child_may_write(pid,stamp));
        for file in self.files.iter().skip(1) {
            if native_evidence(file).is_ok_and(|v|v.is_some_and(|v|v.token==self.token)) {
                if self.evidence(drained).and_then(|v|record_native(file,&v)).is_err() { crate::diagnostic_warn!("chat ownership: could not retire native process evidence"); }
            }
        }
        if drained && app.db().conn.lock().unwrap().execute("DELETE FROM chat_process_owners WHERE session_id=?1 AND token=?2",params![session,self.token]).is_err() {
            crate::diagnostic_warn!("chat ownership: could not retire process evidence");
        }
    }
}
