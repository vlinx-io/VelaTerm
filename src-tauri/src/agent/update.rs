//! One-click agent upgrades from the version badge.
//!
//! Installation types the recipe into the session's idle shell, but an upgrade is offered while the agent runs
//! in the conversation view, which has no shell. The backend therefore runs the agent's own self-update
//! subcommand as a one-shot process and keeps its output for the dialog to poll. Every supported agent ships
//! such a subcommand and it follows however the agent was installed (native installer, npm, Homebrew), so no
//! installer is guessed here.
//!
//! The executable is the one the session resolves, so the copy whose version the badge shows is the copy
//! that gets updated. Nothing about the command comes from the client.

use std::collections::HashMap;
use std::io::Read;
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use crate::{host::AppCtx, models::{Session, SessionKind}};

/// Output kept for the dialog; older bytes are dropped first.
const OUTPUT_LIMIT: usize = 128 * 1024;
/// Upgrades download a full build; anything still running after this is treated as stuck.
const TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// `idle`, `running`, `succeeded`, or `failed`.
    pub state: &'static str,
    pub output: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    /// Version reported by the executable after the command finished.
    pub version: Option<String>,
}

struct Job {
    output: Mutex<Vec<u8>>,
    result: Mutex<Option<(Option<i32>, bool, Option<String>)>>,
}

impl Job {
    fn status(&self) -> UpdateStatus {
        let output = String::from_utf8_lossy(&self.output.lock().unwrap()).into_owned();
        match self.result.lock().unwrap().clone() {
            None => UpdateStatus { state: "running", output, exit_code: None, timed_out: false, version: None },
            Some((exit_code, timed_out, version)) => UpdateStatus {
                state: if exit_code == Some(0) { "succeeded" } else { "failed" },
                output, exit_code, timed_out, version,
            },
        }
    }

    fn append(&self, bytes: &[u8]) {
        let mut output = self.output.lock().unwrap();
        output.extend_from_slice(bytes);
        if output.len() > OUTPUT_LIMIT {
            let excess = output.len() - OUTPUT_LIMIT;
            output.drain(..excess);
        }
    }
}

/// Jobs by executable, so two sessions of the same agent never upgrade one installation at once.
static JOBS: OnceLock<Mutex<HashMap<String, Arc<Job>>>> = OnceLock::new();
/// The job each session started or found. Resolving the executable again while it is polled is unreliable:
/// an npm or Homebrew upgrade removes the old link before installing the new one, and a lookup in between
/// resolves a different path and loses the running job.
static SESSION_JOBS: OnceLock<Mutex<HashMap<String, Arc<Job>>>> = OnceLock::new();

fn jobs() -> &'static Mutex<HashMap<String, Arc<Job>>> {
    JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn session_jobs() -> &'static Mutex<HashMap<String, Arc<Job>>> {
    SESSION_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// How long to wait for an update the agent was already running on its own.
const CONCURRENT_WAIT: Duration = Duration::from_secs(5 * 60);

/// Agents such as Antigravity update themselves in the background whenever they run, including the version
/// probe behind the badge. A command started meanwhile exits with "update already in progress" while that
/// update succeeds, so this output means: wait for it instead of reporting a failure.
fn concurrent_update(output: &[u8]) -> bool {
    let text = String::from_utf8_lossy(output).to_lowercase();
    text.contains("already in progress") || text.contains("another update")
}

/// Self-update subcommands. Each one detects its own installation method.
fn subcommand(kind: SessionKind) -> Option<&'static [&'static str]> {
    match kind {
        SessionKind::Claude | SessionKind::Codex | SessionKind::Omp | SessionKind::Antigravity => Some(&["update"]),
        SessionKind::Opencode => Some(&["upgrade"]),
        // Without a flag `pi update` may also touch extensions; only the agent itself is meant here.
        SessionKind::Pi => Some(&["update", "--self"]),
        _ => None,
    }
}

/// The OpenCode 2 previews track a separate beta channel that `opencode upgrade` does not follow.
fn supported(kind: SessionKind, version: Option<&str>) -> Option<&'static [&'static str]> {
    if kind == SessionKind::Opencode && super::opencode::is_v2_version(version) { return None; }
    subcommand(kind)
}

/// The command as the user would type it, shown and copied in the dialog.
fn display(bin: &str, args: &[&str]) -> String {
    let program = if !bin.chars().any(char::is_whitespace) { bin.to_string() }
        else if cfg!(windows) { format!("& \"{bin}\"") }
        else { format!("'{}'", bin.replace('\'', "'\\''")) };
    std::iter::once(program).chain(args.iter().map(|arg| arg.to_string())).collect::<Vec<_>>().join(" ")
}

/// The update command for a session whose installed version is behind, or None when there is no one-click path.
pub fn command_for(app: &AppCtx, session: &Session, version: Option<&str>) -> Option<String> {
    let args = supported(session.kind, version)?;
    Some(display(&super::executable::for_session(app, session), args))
}

fn session(app: &AppCtx, id: &str) -> Result<Session, String> {
    let conn = app.db().conn.lock().map_err(|_| "Database unavailable")?;
    crate::db::repo::get_session(&conn, id)?.ok_or_else(|| "Session not found".into())
}

/// Report the upgrade of this session's executable, if one ran since VelaTerm started.
pub fn status(app: &AppCtx, id: &str) -> Result<UpdateStatus, String> {
    if let Some(job) = session_jobs().lock().unwrap().get(id).cloned() { return Ok(job.status()); }
    // Another session may be upgrading the same executable; adopt its job so later polls skip resolution.
    let bin = super::executable::for_session(app, &session(app, id)?);
    let Some(job) = jobs().lock().unwrap().get(&bin).cloned() else {
        return Ok(UpdateStatus { state: "idle", output: String::new(), exit_code: None, timed_out: false, version: None });
    };
    session_jobs().lock().unwrap().insert(id.to_string(), job.clone());
    Ok(job.status())
}

/// Start upgrading this session's executable. A run already in progress is returned instead of a second one.
pub fn start(app: &AppCtx, id: &str) -> Result<UpdateStatus, String> {
    let session = session(app, id)?;
    let bin = super::executable::for_session(app, &session);
    let version = super::version::installed(&bin);
    let args = supported(session.kind, version.as_deref()).ok_or("This agent cannot be updated from VelaTerm")?;
    let job = {
        let mut jobs = jobs().lock().unwrap();
        if let Some(job) = jobs.get(&bin).filter(|job| job.result.lock().unwrap().is_none()).cloned() {
            session_jobs().lock().unwrap().insert(id.to_string(), job.clone());
            return Ok(job.status());
        }
        let job = Arc::new(Job { output: Mutex::new(Vec::new()), result: Mutex::new(None) });
        jobs.insert(bin.clone(), job.clone());
        session_jobs().lock().unwrap().insert(id.to_string(), job.clone());
        job
    };
    let mut command = crate::host::command(&bin);
    crate::login_env::refresh_command(&mut command);
    super::executable::prepare_command(&mut command, &bin);
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS { command.env_remove(key); }
    if let Some(home) = crate::host::home_dir() { command.current_dir(home); }
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            job.append(format!("{error}\n").as_bytes());
            *job.result.lock().unwrap() = Some((None, false, version));
            return Ok(job.status());
        }
    };
    let readers = [child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>),
        child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>)]
        .into_iter().flatten().map(|mut stream| {
            let job = job.clone();
            std::thread::spawn(move || {
                let mut buffer = [0u8; 8192];
                while let Ok(n) = stream.read(&mut buffer) {
                    if n == 0 { break; }
                    job.append(&buffer[..n]);
                }
            })
        }).collect::<Vec<_>>();
    let waiter = job.clone();
    let before = version.clone();
    std::thread::spawn(move || {
        let deadline = Instant::now() + TIMEOUT;
        let (code, timed_out) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (status.code(), false),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(200)),
                Ok(None) => { let _ = child.kill(); let _ = child.wait(); break (None, true); }
                Err(_) => { let _ = child.kill(); let _ = child.wait(); break (None, false); }
            }
        };
        // A grandchild that kept the pipes open must not hold the result back.
        let drained = Instant::now() + Duration::from_secs(2);
        for reader in readers {
            while !reader.is_finished() && Instant::now() < drained { std::thread::sleep(Duration::from_millis(20)); }
        }
        let mut code = code;
        let mut version = super::version::installed(&bin);
        if code != Some(0) && !timed_out && concurrent_update(&waiter.output.lock().unwrap()) {
            waiter.append(b"\nWaiting for the update already in progress to finish...\n");
            let deadline = Instant::now() + CONCURRENT_WAIT;
            while version == before && Instant::now() < deadline {
                std::thread::sleep(Duration::from_secs(2));
                version = super::version::installed(&bin);
            }
            if version != before && version.is_some() {
                waiter.append(format!("Updated to {}.\n", version.as_deref().unwrap_or_default()).as_bytes());
                code = Some(0);
            }
        }
        *waiter.result.lock().unwrap() = Some((code, timed_out, version));
    });
    Ok(job.status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_checked_agent_has_a_self_update_command() {
        for kind in [SessionKind::Claude, SessionKind::Codex, SessionKind::Opencode, SessionKind::Pi, SessionKind::Omp, SessionKind::Antigravity] {
            assert!(subcommand(kind).is_some(), "{kind:?}");
        }
        assert!(subcommand(SessionKind::Terminal).is_none());
        assert!(supported(SessionKind::Opencode, Some("0.0.0-beta-19271")).is_none());
        assert_eq!(supported(SessionKind::Pi, Some("1.0.0")), Some(&["update", "--self"][..]));
    }

    #[test]
    fn displayed_commands_are_runnable_as_typed() {
        assert_eq!(display("/usr/local/bin/claude", &["update"]), "/usr/local/bin/claude update");
        assert_eq!(display("pi", &["update", "--self"]), "pi update --self");
        if cfg!(windows) {
            assert_eq!(display("C:\\Program Files\\codex.exe", &["update"]), "& \"C:\\Program Files\\codex.exe\" update");
        } else {
            assert_eq!(display("/Users/a b/claude", &["update"]), "'/Users/a b/claude' update");
        }
    }

    #[test]
    fn recognizes_an_update_already_running() {
        assert!(concurrent_update(b"Error: update already in progress"));
        assert!(!concurrent_update(b"EACCES: permission denied"));
    }

    #[cfg(unix)]
    #[test]
    fn waits_for_an_update_the_agent_was_already_running() {
        use std::os::unix::fs::PermissionsExt;
        use crate::{db::{Db, repo}, host::HeadlessHost};
        let dir = std::env::temp_dir().join(format!("vlx-agent-update-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("agy");
        let marker = dir.join("updated");
        // The background updater finishes a moment after the explicit command gives up.
        std::fs::write(&bin, format!(
            "#!/bin/sh\nif [ \"$1\" = update ]; then (sleep 1; touch '{m}') >/dev/null 2>&1 & echo 'Error: update already in progress' >&2; exit 1; fi\n\
             if [ -f '{m}' ]; then echo 1.4.7; else echo 1.2.14; fi\n",
            m = marker.display())).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
        let db = Db::open(&dir.join("test.db")).unwrap();
        let session = {
            let conn = db.conn.lock().unwrap();
            let project = repo::import_project(&conn, dir.to_str().unwrap()).unwrap();
            repo::create_session_full(&conn, &project.id, None, "Agy", SessionKind::Antigravity,
                None, None, None, None, None, None, None, None, None, bin.to_str(), Some("chat")).unwrap()
        };
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(dir.clone(), db)));
        start(&app, &session.id).unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let done = loop {
            let current = status(&app, &session.id).unwrap();
            if current.state != "running" || Instant::now() > deadline { break current; }
            std::thread::sleep(Duration::from_millis(100));
        };
        assert_eq!(done.state, "succeeded");
        assert_eq!(done.version.as_deref(), Some("1.4.7"));
        assert!(done.output.contains("already in progress") && done.output.contains("Updated to 1.4.7."));
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn runs_the_session_executable_and_reports_the_new_version() {
        use std::os::unix::fs::PermissionsExt;
        use crate::{db::{Db, repo}, host::HeadlessHost};
        let dir = std::env::temp_dir().join(format!("vlx-agent-update-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("custom-agent");
        let marker = dir.join("updated");
        std::fs::write(&bin, format!(
            "#!/bin/sh\nif [ \"$1\" = update ]; then echo fetching; echo warn >&2; touch '{m}'; sleep 1; exit 0; fi\n\
             if [ -f '{m}' ]; then echo 'codex-cli 0.200.0'; else echo 'codex-cli 0.100.0'; fi\n",
            m = marker.display())).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
        let db = Db::open(&dir.join("test.db")).unwrap();
        let session = {
            let conn = db.conn.lock().unwrap();
            let project = repo::import_project(&conn, dir.to_str().unwrap()).unwrap();
            repo::create_session_full(&conn, &project.id, None, "Custom", SessionKind::Codex,
                None, None, None, None, None, None, None, None, None, bin.to_str(), Some("chat")).unwrap()
        };
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(dir.clone(), db)));
        assert_eq!(status(&app, &session.id).unwrap().state, "idle");
        assert_eq!(command_for(&app, &session, Some("0.100.0")).unwrap(), format!("{} update", bin.display()));
        start(&app, &session.id).unwrap();
        // Package managers replace the executable mid-upgrade; polling must not lose the job meanwhile.
        let moved = dir.join("moved-agent");
        std::fs::rename(&bin, &moved).unwrap();
        assert_eq!(status(&app, &session.id).unwrap().state, "running");
        std::fs::rename(&moved, &bin).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let done = loop {
            let current = status(&app, &session.id).unwrap();
            if current.state != "running" || Instant::now() > deadline { break current; }
            std::thread::sleep(Duration::from_millis(50));
        };
        assert_eq!(done.state, "succeeded");
        assert_eq!(done.exit_code, Some(0));
        assert_eq!(done.version.as_deref(), Some("0.200.0"));
        assert!(done.output.contains("fetching") && done.output.contains("warn"));
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
