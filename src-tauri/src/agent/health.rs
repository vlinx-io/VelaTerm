//! Whether an agent executable actually starts.
//!
//! A file on disk is not an installation. An installer that stopped partway, or an update that replaced half
//! a package, leaves a launcher that dies on its first line, and the conversation then fails in ways that look
//! unrelated. Running `--version` is the one check that holds for every agent and every way of installing it.
//!
//! Only a definite failure counts. A check that runs out of time says nothing, because a cold start on a slow
//! machine is not a broken installation.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use crate::models::SessionKind;

/// Longest a health check waits for `--version`.
const LIMIT: Duration = Duration::from_secs(10);
/// A working or undecided verdict is kept until the file changes, an install or update forgets it, or the app
/// restarts. A broken one is checked again after this long: a repair made elsewhere may not touch the file.
const BROKEN_FRESH: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Runs,
    Broken,
    Unknown,
}

/// How a `--version` run ended.
pub(crate) enum Outcome {
    /// The system could not start the file at all.
    NotStarted,
    Exited { success: bool, stdout: String },
    TimedOut,
}

/// Run `bin --version` with nothing on its input and at most `limit` to answer.
pub(crate) fn run_version(bin: &str, limit: Duration) -> Outcome {
    // The ZCode desktop app's binary opens its window when run bare; its bundled agent answers instead.
    let mut command = if super::zcode::desktop_resources(Path::new(bin)).is_some() {
        super::zcode::command(bin)
    } else {
        crate::host::command(bin)
    };
    super::executable::prepare_command(&mut command, bin);
    command.arg("--version").env("NO_COLOR", "1")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return Outcome::NotStarted,
    };
    let stdout = child.stdout.take();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(stdout) = stdout {
            let _ = stdout.take(16_384).read_to_end(&mut bytes);
        }
        let _ = tx.send(bytes);
    });
    let deadline = Instant::now() + limit;
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Outcome::TimedOut;
            }
        }
    };
    let stdout = rx.recv_timeout(Duration::from_millis(100)).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default();
    Outcome::Exited { success, stdout }
}

/// Agents whose `--version` was seen to exit cleanly when installed (2026-11-07). For the others a failing
/// exit may only mean the flag is unknown, so it proves nothing.
fn version_exits_cleanly(kind: SessionKind) -> bool {
    matches!(kind,
        SessionKind::Claude | SessionKind::Codex | SessionKind::Opencode | SessionKind::Copilot
        | SessionKind::Cursor | SessionKind::Antigravity | SessionKind::Pi | SessionKind::Omp
        | SessionKind::Kimi | SessionKind::Grok | SessionKind::Zcode | SessionKind::Deepseek)
}

fn judge(kind: SessionKind, outcome: &Outcome) -> Health {
    match outcome {
        Outcome::Exited { success: true, .. } => Health::Runs,
        Outcome::Exited { success: false, .. } if version_exits_cleanly(kind) => Health::Broken,
        // The file is there, yet it cannot be executed: a truncated download, a dangling link, a lost interpreter.
        Outcome::NotStarted => Health::Broken,
        _ => Health::Unknown,
    }
}

/// The file behind `path` as it is now. A changed file is a different installation.
fn stamp(path: &Path) -> Option<(u64, SystemTime)> {
    let meta = std::fs::metadata(std::fs::canonicalize(path).ok()?).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

type Entry = ((u64, SystemTime), Instant, Health);

fn cache() -> &'static Mutex<HashMap<String, Entry>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The current stamp of `bin` when it is an absolute path to an existing file. A missing file is the
/// caller's to report, and a bare name is resolved by the shell that runs it.
fn checkable(bin: &str) -> Option<(u64, SystemTime)> {
    let path = Path::new(bin);
    if !path.is_absolute() || !path.exists() {
        return None;
    }
    stamp(path)
}

/// The cached verdict for `bin` while its file is unchanged and the verdict still fresh.
fn cached(bin: &str, stamp: (u64, SystemTime)) -> Option<Health> {
    let (seen, at, health) = cache().lock().unwrap().get(bin).copied()?;
    (seen == stamp && (health != Health::Broken || at.elapsed() < BROKEN_FRESH)).then_some(health)
}

fn run_and_record(kind: SessionKind, bin: &str, stamp: (u64, SystemTime)) -> Health {
    let health = judge(kind, &run_version(bin, LIMIT));
    cache().lock().unwrap().insert(bin.to_string(), (stamp, Instant::now(), health));
    health
}

/// Whether the executable at `bin` starts, running it if nothing fresh is known. Blocks for up to `LIMIT`, so
/// it belongs only where a wait is already expected: after an installer or update exits, and in the readiness
/// check that runs while a conversation opens.
pub fn check(kind: SessionKind, bin: &str) -> Health {
    let Some(stamp) = checkable(bin) else { return Health::Unknown };
    cached(bin, stamp).unwrap_or_else(|| run_and_record(kind, bin, stamp))
}

/// What is already known about `bin`, without waiting. Launch paths use this: an unknown executable is
/// launched as before while a check runs in the background, so the next launch knows.
pub fn known(kind: SessionKind, bin: &str) -> Health {
    let Some(stamp) = checkable(bin) else { return Health::Unknown };
    if let Some(health) = cached(bin, stamp) {
        return health;
    }
    if in_flight().lock().unwrap().insert(bin.to_string()) {
        let bin = bin.to_string();
        std::thread::spawn(move || {
            run_and_record(kind, &bin, stamp);
            in_flight().lock().unwrap().remove(&bin);
        });
    }
    Health::Unknown
}

fn in_flight() -> &'static Mutex<HashSet<String>> {
    static IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(Default::default)
}

/// Forget what is known about `bin`. An install or update just changed it, possibly without touching the file
/// the cache is keyed on: a package's launcher often stays the same while the code behind it is replaced.
pub fn forget(bin: &str) {
    cache().lock().unwrap().remove(bin);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exited(success: bool) -> Outcome {
        Outcome::Exited { success, stdout: String::new() }
    }

    #[test]
    fn only_a_definite_failure_marks_an_installation_broken() {
        assert_eq!(judge(SessionKind::Codex, &exited(true)), Health::Runs);
        assert_eq!(judge(SessionKind::Codex, &exited(false)), Health::Broken);
        assert_eq!(judge(SessionKind::Codex, &Outcome::NotStarted), Health::Broken);
        assert_eq!(judge(SessionKind::Codex, &Outcome::TimedOut), Health::Unknown);
        // An agent whose `--version` has not been seen working may simply not know the flag.
        assert_eq!(judge(SessionKind::Crush, &exited(false)), Health::Unknown);
        assert_eq!(judge(SessionKind::Crush, &Outcome::NotStarted), Health::Broken);
    }

    #[cfg(unix)]
    #[test]
    fn a_launcher_that_dies_is_broken_and_a_repaired_one_runs() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("vlx-health-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("codex");
        let write = |body: &str| {
            std::fs::write(&bin, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        let path = bin.to_str().unwrap();
        write("echo 'Error: Cannot find module' >&2; exit 1");
        assert_eq!(check(SessionKind::Codex, path), Health::Broken);
        write("echo 'codex-cli 1.2.3'");
        forget(path);
        assert_eq!(check(SessionKind::Codex, path), Health::Runs);
        assert_eq!(check(SessionKind::Codex, dir.join("absent").to_str().unwrap()), Health::Unknown);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
