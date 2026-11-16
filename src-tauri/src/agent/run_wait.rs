//! `vrun`: start a long-running command and wait for it in one call.
//!
//! An agent learns that work has finished only when a command it issued returns. Starting work with
//! `nohup ... &` returns at once, so the agent must write a second command to wait for it — and that
//! second command is written fresh every time. Forgetting it, attaching it to the previous round, or
//! matching the waiting command itself with `pgrep` all end the same way: the work finishes and nobody
//! is told. This command removes that gap by starting the work and waiting for it in a single call.
//!
//! The work runs under a small supervisor — this binary again, in its own session (`setsid`) with
//! `SIGHUP` ignored — so closing the conversation does not kill a build that has been running for an
//! hour. The supervisor waits on the command as its parent and records the exit code, so the result
//! survives the waiting half leaving early.
//!
//! Every wait is bounded without a timeout: the waiting half exits at fixed times with a progress report
//! (every three minutes unless `-i` says otherwise), so an agent that has promised to report by a certain
//! time is woken to do it, and a wait that has gone wrong costs one interval rather than the rest of the
//! session. `vrun --wait` picks the wait up again on the same schedule; the command itself is never
//! interrupted. An explicit `-t` still ends the waiting for good, returning 124.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use super::runs::RunMeta;

const USAGE: &str = "usage: vrun [-t <seconds>] [-i <seconds>] <label> <command> [arguments...]
       vrun --wait [-t <seconds>] [-i <seconds>] <label>
       vrun --status <label>
Starts the command, waits for it, and exits when it does, so its completion reaches you.
Prints the exit code, the elapsed time and the tail of the log; the command's exit code is passed through.
At each interval vrun exits 75 with a progress report and leaves the command running;
continue with `vrun --wait <label>`, which keeps the same schedule.
    -i <seconds>  report progress at this interval, counted from the start of the command (default 180)
    -t <seconds>  stop waiting after this long and exit 124, leaving the command running
                  (no limit by default; with --wait, counted from now)
    --wait        wait again for a label that is still running
    --status      report on a label started earlier";

const DEFAULT_INTERVAL_SECS: u64 = 180;
const POLL: Duration = Duration::from_millis(500);
const TAIL_BYTES: u64 = 64 * 1024;
const TAIL_LINES: usize = 20;
/// Reaching the wait limit is not the command failing. 124 is what `timeout(1)` returns, so a caller
/// that already knows that convention reads it correctly.
const TIMEOUT_EXIT: i32 = 124;
/// A progress report is not a result either. 75 is `EX_TEMPFAIL` from `sysexits.h`: try again later.
const REPORT_EXIT: i32 = 75;
/// Hidden first argument that turns this process into the supervisor of one run.
const SUPERVISE: &str = "--supervise";

struct Parsed {
    label: String,
    timeout: Option<u64>,
    interval: u64,
    command: Vec<String>,
}

struct Resume {
    label: String,
    timeout: Option<u64>,
    interval: Option<u64>,
}

enum Parse {
    Help,
    Status(String),
    Run(Parsed),
    Wait(Resume),
    Err(String),
}

fn seconds(name: &str, flag: &str, value: Option<&String>) -> Result<u64, String> {
    let Some(value) = value else {
        return Err(format!("{flag} needs a number of seconds"));
    };
    match value.parse::<u64>() {
        Ok(secs) if secs > 0 => Ok(secs),
        _ => Err(format!("Invalid {name} {value}")),
    }
}

fn parse(args: &[String]) -> Parse {
    let mut timeout = None;
    let mut interval = None;
    let mut wait = false;
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        match arg.as_str() {
            "--help" | "-h" => return Parse::Help,
            "--status" => {
                return match args.get(i + 1) {
                    Some(label) => Parse::Status(label.clone()),
                    None => Parse::Err("--status needs a label".into()),
                };
            }
            "--wait" => {
                wait = true;
                i += 1;
            }
            "-t" | "--timeout" => {
                match seconds("timeout", arg, args.get(i + 1)) {
                    Ok(secs) => timeout = Some(secs),
                    Err(message) => return Parse::Err(message),
                }
                i += 2;
            }
            "-i" | "--interval" => {
                match seconds("interval", arg, args.get(i + 1)) {
                    Ok(secs) => interval = Some(secs),
                    Err(message) => return Parse::Err(message),
                }
                i += 2;
            }
            other if other.starts_with('-') => return Parse::Err(format!("Unknown option {other}")),
            // The label ends option parsing: everything after it belongs to the command, including its
            // own flags, so `vrun build cargo test --lib` needs no `--` separator.
            _ => break,
        }
    }
    let Some(label) = args.get(i) else {
        return Parse::Err("Specify a label and a command".into());
    };
    if !super::runs::valid_label(label) {
        return Parse::Err("The label must be a plain name without path separators".into());
    }
    if wait {
        if args.len() > i + 1 {
            return Parse::Err("--wait takes a label and no command".into());
        }
        return Parse::Wait(Resume { label: label.clone(), timeout, interval });
    }
    let command: Vec<String> = args[i + 1..].to_vec();
    if command.is_empty() {
        return Parse::Err("Specify the command to run after the label".into());
    }
    Parse::Run(Parsed { label: label.clone(), timeout, interval: interval.unwrap_or(DEFAULT_INTERVAL_SECS), command })
}

/// Where a label's log and receipts live: beside the injected `bin/` directory, so runs sit with the
/// rest of the session's data. Outside a session, fall back to a temporary directory.
fn run_dir(label: &str) -> PathBuf {
    let base = std::env::var_os("VLX_BIN_DIR")
        .map(PathBuf::from)
        .and_then(|bin| bin.parent().map(|dir| dir.join("runs")))
        .unwrap_or_else(|| std::env::temp_dir().join("velaterm-runs"));
    base.join(label)
}

/// Read the last lines of the log without loading a long build's entire output.
fn tail(path: &Path) -> String {
    tail_lines(path, TAIL_BYTES, TAIL_LINES)
}

fn print_tail(path: &Path) {
    let text = tail(path);
    if !text.is_empty() { println!("{text}"); }
}

/// The last `max_lines` lines within the last `max_bytes` of a file. Shared with the log viewer.
pub(crate) fn tail_lines(path: &Path, max_bytes: u64, max_lines: usize) -> String {
    let Ok(mut file) = std::fs::File::open(path) else { return String::new() };
    let Ok(len) = file.metadata().map(|m| m.len()) else { return String::new() };
    let from = len.saturating_sub(max_bytes);
    if file.seek(SeekFrom::Start(from)).is_err() { return String::new(); }
    let mut buffer = Vec::new();
    if file.read_to_end(&mut buffer).is_err() { return String::new(); }
    let text = String::from_utf8_lossy(&buffer);
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(max_lines)..].join("\n")
}

/// Tell the VelaTerm instance hosting this session to look at its runs again.
///
/// Best effort and brief: the service also polls, so a lost announcement only delays what the session
/// shows by a few seconds, and a slow one must never hold up the work or its result.
fn announce() {
    let (Ok(url), Ok(session_id)) = (
        std::env::var("VLX_SPAWN_URL"),
        std::env::var("VLX_SESSION_ID"),
    ) else {
        return;
    };
    let Some(token) = super::session_env::token() else {
        return;
    };
    let body = serde_json::json!({ "sessionId": session_id }).to_string();
    let _ = super::cli_client::post_brief(&format!("{url}/runs?t={token}"), &body);
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Write through a temporary file, so a reader never sees half of an exit code or a record.
fn write_atomic(path: &Path, text: &str) {
    let partial = path.with_extension("partial");
    if std::fs::write(&partial, text).is_ok() && std::fs::rename(&partial, path).is_err() {
        let _ = std::fs::remove_file(&partial);
    }
}

fn read_exit(dir: &Path) -> Option<i32> {
    std::fs::read_to_string(dir.join("exit")).ok().and_then(|code| code.trim().parse().ok())
}

fn read_meta(dir: &Path) -> Option<RunMeta> {
    serde_json::from_str(&std::fs::read_to_string(dir.join(super::runs::META_FILE)).ok()?).ok()
}

fn write_meta(dir: &Path, meta: &RunMeta) {
    if let Ok(text) = serde_json::to_string(meta) {
        write_atomic(&dir.join(super::runs::META_FILE), &text);
    }
}

/// When the next progress report is due: the first multiple of `interval` after `now`, counted from the
/// start. Reports keep to that one schedule however many times the wait is picked up again.
fn next_report(started_at: u64, interval_secs: u64, now: u64) -> u64 {
    let interval = interval_secs.saturating_mul(1000).max(1);
    started_at.saturating_add((now.saturating_sub(started_at) / interval).saturating_add(1).saturating_mul(interval))
}

/// Serialize starts before touching receipts, and leave a running label's files intact.
fn claim_start(dir: &Path) -> Result<std::fs::File, String> {
    let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(dir.join("start.lock")).map_err(|error| error.to_string())?;
    file.try_lock().map_err(|_| "Another command is starting with this label".to_string())?;
    if read_exit(dir).is_none() && read_meta(dir).is_some_and(|meta| super::runs::alive(meta.pid)) {
        return Err("A command is still running with this label; use --wait or choose another label".into());
    }
    Ok(file)
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    // Safety: both calls are async-signal-safe and run between fork and exec.
    unsafe {
        command.pre_exec(|| {
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            // A new session detaches the work from the process group the host may terminate.
            libc::setsid();
            Ok(())
        });
    }
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
}

/// The command stays in the supervisor's process group, so stopping the run signals both. It inherits
/// the ignored `SIGHUP` as it is.
#[cfg(unix)]
fn keep_detached(_command: &mut Command) {}

/// A console program started by the console-less supervisor would otherwise open a console window. It
/// gets a hidden console instead of none: PowerShell started with no console at all (`DETACHED_PROCESS`)
/// exits 0 at once without running its command.
#[cfg(windows)]
fn keep_detached(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// Stopping a run signals its whole group. The supervisor outlives the signal just long enough to
/// record how the command ended. Called after the command is spawned, so the command keeps the default.
#[cfg(unix)]
fn survive_stop() {
    unsafe { libc::signal(libc::SIGTERM, libc::SIG_IGN); }
}

#[cfg(windows)]
fn survive_stop() {}

#[cfg(unix)]
fn exit_code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    // A command killed by a signal reports 128 + the signal, as a shell does.
    status.code().or_else(|| status.signal().map(|signal| 128 + signal)).unwrap_or(1)
}

#[cfg(windows)]
fn exit_code(status: std::process::ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

fn status(label: &str) -> ! {
    let dir = run_dir(label);
    if !dir.exists() {
        eprintln!("vrun: no run recorded for {label}");
        std::process::exit(1);
    }
    let log = dir.join("log");
    match read_exit(&dir) {
        Some(code) => println!("vrun: {label} finished with exit code {code}"),
        None => {
            let pid = std::fs::read_to_string(dir.join("pid")).ok()
                .and_then(|text| text.trim().parse::<u32>().ok());
            match pid {
                Some(pid) if super::runs::alive(pid) => {
                    println!("vrun: {label} is still running as PID {pid}; wait for it with `vrun --wait {label}`")
                }
                Some(pid) => println!("vrun: {label} (PID {pid}) recorded no exit code; it was stopped"),
                None => println!("vrun: {label} has no recorded PID"),
            }
        }
    }
    println!("vrun: log {}", log.display());
    print_tail(&log);
    std::process::exit(0);
}

/// Close any descriptors inherited past stderr before spawning a detached supervisor or waiting on
/// a long-running command, so the task never retains a caller's locks or pipes.
#[cfg(unix)]
fn close_extra_fds() {
    for dir in ["/proc/self/fd", "/dev/fd"] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        // Collect first so the directory stream closes before we close the listed descriptors.
        let fds: Vec<i32> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
            .filter(|&fd| fd >= 3)
            .collect();
        for fd in fds {
            unsafe { libc::close(fd); }
        }
        return;
    }
    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    let max = if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == 0 {
        (limit.rlim_cur as u64).min(65_536) as i32
    } else {
        1024
    };
    for fd in 3..max {
        unsafe { libc::close(fd); }
    }
}

#[cfg(not(unix))]
fn close_extra_fds() {}

pub fn run(args: &[String]) -> ! {
    close_extra_fds();
    let args = &args[args.len().min(2)..];
    if args.first().map(String::as_str) == Some(SUPERVISE) {
        supervise(&args[1..]);
    }
    match parse(args) {
        Parse::Help => {
            println!("{USAGE}");
            std::process::exit(0);
        }
        Parse::Err(message) => {
            eprintln!("vrun: {message}");
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
        Parse::Status(label) => status(&label),
        Parse::Run(parsed) => start(parsed),
        Parse::Wait(resume) => wait_again(resume),
    }
}

/// The detached half: run the command, wait for it as its parent, and record how it ended. Its standard
/// output and error are already the run's log.
fn supervise(args: &[String]) -> ! {
    let (Some(dir), Some(program)) = (args.first(), args.get(1)) else {
        eprintln!("vrun: the supervisor needs a run directory and a command");
        std::process::exit(2);
    };
    let dir = PathBuf::from(dir);
    let mut command = Command::new(program);
    command.args(&args[2..]).stdin(Stdio::null());
    keep_detached(&mut command);
    let code = match command.spawn() {
        Ok(mut child) => {
            survive_stop();
            match child.wait() {
                Ok(status) => exit_code(status),
                Err(error) => {
                    eprintln!("vrun: lost track of {program}: {error}");
                    1
                }
            }
        }
        Err(error) => {
            eprintln!("vrun: cannot start {program}: {error}");
            2
        }
    };
    write_atomic(&dir.join("exit"), &code.to_string());
    announce();
    std::process::exit(code);
}

fn start(parsed: Parsed) -> ! {
    let dir = run_dir(&parsed.label);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("vrun: cannot create {}: {error}", dir.display());
        std::process::exit(2);
    }
    let claim = match claim_start(&dir) {
        Ok(claim) => claim,
        Err(error) => {
            eprintln!("vrun: {}: {error}", parsed.label);
            std::process::exit(2);
        }
    };
    let log_path = dir.join("log");
    // A new run replaces the previous one's receipts, so a stale exit code is never read as this run's,
    // and the previous run's record never names this one's session while it starts.
    let _ = std::fs::remove_file(dir.join(super::runs::META_FILE));
    let _ = std::fs::remove_file(dir.join("exit"));
    let log = match std::fs::File::create(&log_path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("vrun: cannot write {}: {error}", log_path.display());
            std::process::exit(2);
        }
    };
    let errors = match log.try_clone() {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("vrun: cannot open the log for the command's errors: {error}");
            std::process::exit(2);
        }
    };
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            eprintln!("vrun: cannot locate the VelaTerm binary to supervise the command: {error}");
            std::process::exit(2);
        }
    };
    let mut command = Command::new(&exe);
    command.arg("--run").arg(SUPERVISE).arg(&dir).args(&parsed.command)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(errors));
    detach(&mut command);
    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("vrun: cannot start {}: {error}", parsed.command[0]);
            std::process::exit(2);
        }
    };
    // The supervisor's PID: it leads the run's process group, so stopping it stops the command too.
    let pid = child.id();
    let _ = std::fs::write(dir.join("pid"), pid.to_string());
    let started_at = now_ms();
    // The record that lets the session show this run and stay busy until it ends, and lets a later
    // `vrun --wait` keep this call's schedule.
    let meta = RunMeta {
        session_id: std::env::var("VLX_SESSION_ID").unwrap_or_default(),
        command: parsed.command.clone(),
        started_at,
        pid,
        deadline: parsed.timeout.map(|secs| started_at.saturating_add(secs.saturating_mul(1000))),
        interval: Some(parsed.interval),
    };
    write_meta(&dir, &meta);
    drop(claim);
    announce();
    let limit = parsed.timeout.map(|secs| format!(" · timeout {secs}s")).unwrap_or_default();
    println!("vrun: {} · PID {pid} · report every {}s{limit} · log {}", parsed.label, parsed.interval, log_path.display());
    wait(&dir, &parsed.label, &meta, Watched::Child(child))
}

/// Pick up the wait for a run started earlier, keeping its schedule unless told otherwise.
fn wait_again(resume: Resume) -> ! {
    let dir = run_dir(&resume.label);
    let Some(mut meta) = read_meta(&dir) else {
        eprintln!("vrun: no run recorded for {}", resume.label);
        std::process::exit(2);
    };
    if let Some(code) = read_exit(&dir) {
        finished(&resume.label, meta.started_at, &dir.join("log"), code);
    }
    // A limit that has already been reached was answered by the timeout that reported it. Waiting again
    // without a new one returns to the default: reports only.
    let expired = resume.timeout.is_none() && meta.deadline.is_some_and(|deadline| now_ms() >= deadline);
    if expired { meta.deadline = None; }
    if expired || resume.timeout.is_some() || resume.interval.is_some() {
        if let Some(secs) = resume.timeout { meta.deadline = Some(now_ms().saturating_add(secs.saturating_mul(1000))); }
        if let Some(secs) = resume.interval { meta.interval = Some(secs); }
        write_meta(&dir, &meta);
    }
    let limit = meta.deadline
        .map(|deadline| format!(" · gives up in {}s", deadline.saturating_sub(now_ms()).div_ceil(1000)))
        .unwrap_or_default();
    println!(
        "vrun: waiting again for {} · PID {} · {}s elapsed · report every {}s{limit}",
        resume.label,
        meta.pid,
        elapsed_secs(meta.started_at),
        meta.interval.unwrap_or(DEFAULT_INTERVAL_SECS),
    );
    let pid = meta.pid;
    wait(&dir, &resume.label, &meta, Watched::Pid(pid))
}

/// The supervisor being waited on: this call's own child, or one an earlier call started.
enum Watched {
    Child(std::process::Child),
    Pid(u32),
}

impl Watched {
    /// Our own child is reaped rather than probed: until it is, a finished child still answers signal 0.
    fn ended(&mut self) -> bool {
        match self {
            Watched::Child(child) => !matches!(child.try_wait(), Ok(None)),
            Watched::Pid(pid) => !super::runs::alive(*pid),
        }
    }
}

fn elapsed_secs(started_at: u64) -> u64 {
    now_ms().saturating_sub(started_at) / 1000
}

fn finished(label: &str, started_at: u64, log: &Path, code: i32) -> ! {
    println!("vrun: {label} finished · exit code {code} · {}s", elapsed_secs(started_at));
    print_tail(log);
    std::process::exit(code);
}

fn wait(dir: &Path, label: &str, meta: &RunMeta, mut watched: Watched) -> ! {
    let log = dir.join("log");
    // Records from before the interval existed get the default one.
    let interval = meta.interval.unwrap_or(DEFAULT_INTERVAL_SECS);
    let report_at = next_report(meta.started_at, interval, now_ms());
    loop {
        if let Some(code) = read_exit(dir) {
            finished(label, meta.started_at, &log, code);
        }
        if watched.ended() {
            // The supervisor records the exit code before it exits, so a code written just now counts.
            if let Some(code) = read_exit(dir) {
                finished(label, meta.started_at, &log, code);
            }
            println!("vrun: {label} (PID {}) ended without recording an exit code; it was stopped", meta.pid);
            print_tail(&log);
            std::process::exit(1);
        }
        let now = now_ms();
        if meta.deadline.is_some_and(|deadline| now >= deadline) {
            println!(
                "vrun: {label} has not finished after {}s. PID {} is still running and was not stopped.",
                elapsed_secs(meta.started_at),
                meta.pid,
            );
            print_tail(&log);
            println!("vrun: keep waiting with `vrun --wait {label}`, or check on it with `vrun --status {label}`; the log is {}", log.display());
            std::process::exit(TIMEOUT_EXIT);
        }
        if now >= report_at {
            println!("vrun: {label} is still running · {}s elapsed · PID {}", elapsed_secs(meta.started_at), meta.pid);
            print_tail(&log);
            let next = report_at.saturating_add(interval.saturating_mul(1000)).min(meta.deadline.unwrap_or(u64::MAX));
            let again = next.saturating_sub(now).div_ceil(1000);
            println!("vrun: progress report; the command keeps running. Continue with `vrun --wait {label}`, which reports again in {again}s.");
            std::process::exit(REPORT_EXIT);
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn options_before_the_label_belong_to_vrun() {
        let Parse::Run(parsed) = parse(&args("-t 600 -i 60 build cargo test -i x")) else { panic!() };
        assert_eq!(parsed.label, "build");
        assert_eq!(parsed.timeout, Some(600));
        assert_eq!(parsed.interval, 60);
        assert_eq!(parsed.command, args("cargo test -i x"));

        let Parse::Run(parsed) = parse(&args("build make")) else { panic!() };
        assert_eq!(parsed.timeout, None, "no limit unless asked for");
        assert_eq!(parsed.interval, DEFAULT_INTERVAL_SECS);
    }

    #[test]
    fn wait_takes_a_label_and_optional_limits() {
        let Parse::Wait(resume) = parse(&args("--wait build")) else { panic!() };
        assert_eq!((resume.label.as_str(), resume.timeout, resume.interval), ("build", None, None));
        let Parse::Wait(resume) = parse(&args("--wait -t 900 -i 300 build")) else { panic!() };
        assert_eq!((resume.timeout, resume.interval), (Some(900), Some(300)));
        assert!(matches!(parse(&args("--wait build make")), Parse::Err(_)));
    }

    #[test]
    fn limits_must_be_positive_seconds() {
        for bad in ["-i 0 build make", "-i x build make", "-t -1 build make", "build", "-i", ". make", ".. make"] {
            assert!(matches!(parse(&args(bad)), Parse::Err(_)), "{bad}");
        }
    }

    /// Reports fall on multiples of the interval from the start, wherever the wait picks up.
    #[test]
    fn reports_keep_one_schedule() {
        assert_eq!(next_report(1_000, 60, 1_000), 61_000);
        assert_eq!(next_report(1_000, 60, 30_000), 61_000);
        assert_eq!(next_report(1_000, 60, 61_000), 121_000, "a report just made is not due again");
        assert_eq!(next_report(1_000, 60, 61_500), 121_000);
        assert_eq!(next_report(1_000, 60, 0), 61_000, "a clock behind the start counts as the start");
        assert_eq!(next_report(1_000, u64::MAX, 1_000), u64::MAX);
    }

    #[test]
    fn starts_preserve_live_runs_and_serialize_receipt_changes() {
        let dir = std::env::temp_dir().join(format!("vlx-vrun-start-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let claim = claim_start(&dir).unwrap();
        assert!(claim_start(&dir).is_err());
        let meta = RunMeta { session_id: String::new(), command: vec![], started_at: 1, pid: std::process::id(), deadline: None, interval: None };
        write_meta(&dir, &meta);
        std::fs::write(dir.join("log"), "keep this output").unwrap();
        drop(claim);
        assert!(claim_start(&dir).is_err());
        assert_eq!(std::fs::read_to_string(dir.join("log")).unwrap(), "keep this output");
        assert_eq!(read_meta(&dir).unwrap().pid, meta.pid);
        write_atomic(&dir.join("exit"), "0");
        drop(claim_start(&dir).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "invoked in an isolated process by inherited_descriptors_above_stderr_are_closed_on_entry"]
    fn close_extra_fds_helper() {
        use std::io::{BufRead, Write};
        close_extra_fds();
        println!("VLX_EXTRA_FDS_CLOSED");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        let _ = std::io::stdin().lock().read_line(&mut line);
    }

    #[cfg(unix)]
    #[test]
    fn inherited_descriptors_above_stderr_are_closed_on_entry() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt;

        let dir = std::env::temp_dir().join(format!("vlx-vrun-fds-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let lock_path = dir.join("inherited.lock");
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        locked.try_lock().unwrap();
        let raw_fd = locked.as_raw_fd();

        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "agent::run_wait::tests::close_extra_fds_helper",
            "--ignored",
            "--test-threads=1",
            "--nocapture",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
        unsafe {
            cmd.pre_exec(move || {
                if libc::fcntl(raw_fd, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = cmd.spawn().unwrap();
        drop(locked);

        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        let mut ready = false;
        while reader.read_line(&mut line).unwrap() > 0 {
            if line.contains("VLX_EXTRA_FDS_CLOSED") {
                ready = true;
                break;
            }
            line.clear();
        }
        assert!(ready, "helper did not report extra descriptors closed");
        assert!(child.try_wait().unwrap().is_none(), "helper should still be alive while verifying lock");

        let reopened = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        assert!(reopened.try_lock().is_ok(), "child should not still hold inherited lock descriptor");
        drop(reopened);

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(b"\n");
        }
        assert!(child.wait().unwrap().success());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
