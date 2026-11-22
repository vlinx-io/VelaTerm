//! Shell mode for the conversation view: `!` followed by a command runs it in the session's shell.
//!
//! The terminal UI of Claude Code does the same with a leading `!`: the command runs without the model,
//! and command plus output are then handed to the model as one user message wrapped in `<bash-input>`,
//! `<bash-stdout>` and `<bash-stderr>` tags, so it can react to the result. This module owns three parts
//! of that in VelaTerm:
//!
//! - the tag format: `build_context` writes the message the agent receives and `parse_context` reads it
//!   back, so the same message is drawn as a command row live, when the agent echoes it, and on replay,
//!   and never as a user bubble full of tags;
//! - the runner: `spawn_run` starts the command through the session's shell in the agent's directory
//!   and environment, streams both pipes into bounded tail buffers, and reports the exit;
//! - the replay mapping: `map_replayed_rows` turns a recorded tagged user message back into a row.
//!
//! VelaTerm sends one combined context message. Claude Code 2.1.278 native shell mode instead records
//! adjacent input/output messages; history joins those observed records while preserving their identities.

use std::io::Read;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::engine::ChatRow;
use crate::host::AppCtx;
use crate::models::SessionKind;

/// How much of each stream the row and the agent keep. The tail is kept, the head is cut.
pub const OUTPUT_CAP: usize = 200 * 1024;

const METADATA_START: &str = "\n<velaterm-shell-metadata>";
const METADATA_END: &str = "</velaterm-shell-metadata>";

/// How often a running command's row is refreshed while output keeps arriving.
const UPSERT_INTERVAL: Duration = Duration::from_millis(250);
const LIVE_OUTPUT_CAP: usize = 8 * 1024;
const PIPE_DRAIN_GRACE: Duration = Duration::from_millis(500);
/// How often the wait thread checks whether the command has ended.
const WAIT_POLL: Duration = Duration::from_millis(50);
/// Stable refusal on Windows hosts. Running an arbitrary command there needs `powershell -EncodedCommand`
/// or a comparable launcher, which antivirus products such as Huorong block as a suspicious script.
pub const UNSUPPORTED: &str = "chat_shell_unsupported";

pub const STATUS_RUNNING: &str = "running";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_CANCELLED: &str = "cancelled";

/// Everything one finished command tells the agent, and everything the row shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellContext {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    /// `None` when no exit code was reported, including native or ambiguous legacy recordings.
    pub exit_code: Option<i32>,
    pub cancelled: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub output_incomplete: bool,
}

impl ShellContext {
    pub fn status(&self) -> &'static str {
        if self.cancelled { STATUS_CANCELLED } else { STATUS_COMPLETED }
    }
}

/// The last `OUTPUT_CAP` bytes of a stream, cut at a character boundary.
#[derive(Debug, Default)]
pub struct TailBuffer {
    text: String,
    truncated: bool,
}

impl TailBuffer {
    pub fn push(&mut self, chunk: &str) {
        self.text.push_str(chunk);
        if self.text.len() > OUTPUT_CAP {
            let mut cut = self.text.len() - OUTPUT_CAP;
            while !self.text.is_char_boundary(cut) {
                cut += 1;
            }
            self.text.drain(..cut);
            self.truncated = true;
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Shell execution has its own receipt namespace, so its eventual context message can use the row id
/// without colliding with an ordinary chat submission. Only a digest is stored in the existing table.
pub fn submission_id(message_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("shell-execution\0{message_id}"));
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    format!("msg-{}", uuid::Uuid::from_bytes(bytes))
}

// ─────────────────────────── Tag format ───────────────────────────

/// VelaTerm-owned metadata in an ordinary provider user message. Byte lengths make the raw streams
/// unambiguous without escaping their display or interpreting output as execution facts.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextMetadata {
    version: u8,
    command_bytes: usize,
    stdout_bytes: usize,
    stderr_bytes: usize,
    exit_code: Option<i32>,
    cancelled: bool,
    stdout_truncated: bool,
    stderr_truncated: bool,
    output_incomplete: bool,
}

/// The one user message the agent receives when a command ends.
pub fn build_context(ctx: &ShellContext) -> String {
    let metadata = ContextMetadata {
        version: 1, command_bytes: ctx.command.len(), stdout_bytes: ctx.stdout.len(), stderr_bytes: ctx.stderr.len(),
        exit_code: ctx.exit_code, cancelled: ctx.cancelled, stdout_truncated: ctx.stdout_truncated,
        stderr_truncated: ctx.stderr_truncated, output_incomplete: ctx.output_incomplete,
    };
    let metadata = serde_json::to_string(&metadata).expect("shell metadata contains only integers and booleans");
    format!("<bash-input>{}</bash-input>\n<bash-stdout>{}</bash-stdout><bash-stderr>{}</bash-stderr>{METADATA_START}{metadata}{METADATA_END}",
        ctx.command, ctx.stdout, ctx.stderr)
}

/// Whether a message is a shell-mode context message rather than prose.
pub fn is_tagged(text: &str) -> bool {
    text.trim_start().starts_with("<bash-input>")
}

/// Read VelaTerm's length-delimited context, or an unambiguous legacy context with unknown facts.
/// Malformed or future metadata stays raw; it must not silently fall back to a guessed legacy result.
pub fn parse_context(text: &str) -> Option<ShellContext> {
    let text = text.trim_start();
    if let Some(framed) = text.strip_suffix(METADATA_END) {
        let (body, metadata) = framed.rsplit_once(METADATA_START)?;
        if metadata.len() > 1024 { return None; }
        let metadata: ContextMetadata = serde_json::from_str(metadata).ok()?;
        if metadata.version != 1 { return None; }
        let rest = body.strip_prefix("<bash-input>")?;
        let (command, rest) = take_bytes(rest, metadata.command_bytes)?;
        let rest = rest.strip_prefix("</bash-input>\n<bash-stdout>")?;
        let (stdout, rest) = take_bytes(rest, metadata.stdout_bytes)?;
        let rest = rest.strip_prefix("</bash-stdout><bash-stderr>")?;
        let (stderr, rest) = take_bytes(rest, metadata.stderr_bytes)?;
        if rest != "</bash-stderr>" { return None; }
        return Some(ShellContext {
            command: command.into(), stdout: stdout.into(), stderr: stderr.into(),
            exit_code: metadata.exit_code, cancelled: metadata.cancelled,
            stdout_truncated: metadata.stdout_truncated, stderr_truncated: metadata.stderr_truncated,
            output_incomplete: metadata.output_incomplete,
        });
    }
    let rest = text.strip_prefix("<bash-input>")?;
    let (command, rest) = rest.split_once("</bash-input>")?;
    if contains_shell_tag(command) { return None; }
    let output = rest.strip_prefix('\n').unwrap_or(rest);
    legacy_context(command, output.trim_end())
}

fn take_bytes(text: &str, count: usize) -> Option<(&str, &str)> {
    // str::get rejects out-of-range lengths and offsets inside a UTF-8 character without panicking.
    Some((text.get(..count)?, text.get(count..)?))
}

fn contains_shell_tag(text: &str) -> bool {
    ["<bash-input>", "</bash-input>", "<bash-stdout>", "</bash-stdout>", "<bash-stderr>", "</bash-stderr>"]
        .iter().any(|tag| text.contains(tag))
}

/// The native input event supplies its own command boundary. Output tags without lengths are only
/// usable if their streams contain no competing tags. Old status-looking lines remain literal data.
pub(super) fn legacy_context(command: &str, output: &str) -> Option<ShellContext> {
    let rest = output.strip_prefix("<bash-stdout>")?;
    let (stdout, stderr) = match rest.strip_suffix("</bash-stderr>") {
        Some(body) => body.split_once("</bash-stdout><bash-stderr>")?,
        None => (rest.strip_suffix("</bash-stdout>")?, ""),
    };
    if contains_shell_tag(stdout) || contains_shell_tag(stderr) { return None; }
    Some(ShellContext {
        command: command.into(), stdout: stdout.into(), stderr: stderr.into(), ..Default::default()
    })
}

/// The timeline row for a command, live or finished.
pub fn row(id: String, at: Option<i64>, ctx: &ShellContext, status: &'static str) -> ChatRow {
    ChatRow::Shell {
        id,
        command: ctx.command.clone(),
        stdout: ctx.stdout.clone(),
        stderr: ctx.stderr.clone(),
        source_text: build_context(ctx),
        stdout_truncated: ctx.stdout_truncated,
        stderr_truncated: ctx.stderr_truncated,
        output_incomplete: ctx.output_incomplete,
        status,
        exit_code: if status == STATUS_RUNNING { None } else { ctx.exit_code },
        at,
    }
}

/// Turn every replayed top-level user message that is a context message back into its command row.
///
/// The agent's own recording stores what it was sent, which is the tagged text; without this a reopened
/// conversation would show the tags as something the user typed.
pub fn map_replayed_rows(rows: &mut [ChatRow]) {
    for slot in rows.iter_mut() {
        let ChatRow::User { id, text, images, at } = slot else { continue };
        if !images.is_empty() {
            continue;
        }
        let Some(ctx) = parse_context(text) else { continue };
        let original = std::mem::take(text);
        *slot = row(std::mem::take(id), *at, &ctx, ctx.status());
        if let ChatRow::Shell { source_text, .. } = slot { *source_text = original; }
    }
}

// ─────────────────────────── Runner ───────────────────────────

/// One command running for a conversation.
pub struct ShellRun {
    pub id: String,
    pub command: String,
    pub started_at: i64,
    child: Mutex<Child>,
    output_incomplete: AtomicBool,
    stdout: Arc<Mutex<TailBuffer>>,
    stderr: Arc<Mutex<TailBuffer>>,
    cancelled: AtomicBool,
    /// Set when the conversation was stopped underneath the command: the result is not reported to an
    /// agent that has been let go, which would start it again only to tell it.
    abandoned: AtomicBool,
    last_upsert: Mutex<Instant>,
    drain_deadline: Mutex<Option<Instant>>,
}

impl ShellRun {
    pub fn cancel(&self) {
        let mut child = self.child.lock().unwrap();
        // A late click cannot relabel an already completed command or target a reused process id.
        if matches!(child.try_wait(), Ok(Some(_))) { return; }
        self.cancelled.store(true, Ordering::Relaxed);
        #[cfg(unix)]
        crate::host::kill_process_tree(&mut child);
        #[cfg(not(unix))]
        let _ = child.kill();
    }

    pub fn abandon(&self) {
        self.abandoned.store(true, Ordering::Relaxed);
        self.cancel();
    }

    pub fn abandoned(&self) -> bool {
        self.abandoned.load(Ordering::Relaxed)
    }

    pub fn pid(&self) -> u32 {
        self.child.lock().unwrap().id()
    }

    fn snapshot(&self, exit_code: Option<i32>) -> ShellContext {
        let stdout = self.stdout.lock().unwrap();
        let stderr = self.stderr.lock().unwrap();
        let cap = if exit_code.is_none() { LIVE_OUTPUT_CAP } else { OUTPUT_CAP };
        let tail = |buffer: &TailBuffer| {
            let text = buffer.text();
            let mut start = text.len().saturating_sub(cap);
            while !text.is_char_boundary(start) { start += 1; }
            (text[start..].to_string(), buffer.truncated() || start > 0)
        };
        let (stdout, stdout_truncated) = tail(&stdout);
        let (stderr, stderr_truncated) = tail(&stderr);
        ShellContext {
            command: self.command.clone(),
            stdout,
            stderr,
            exit_code,
            cancelled: self.cancelled.load(Ordering::Relaxed),
            stdout_truncated,
            stderr_truncated,
            output_incomplete: self.output_incomplete.load(Ordering::Relaxed),
        }
    }

    /// The row while the command runs.
    pub fn running_row(&self) -> ChatRow {
        row(self.id.clone(), Some(self.started_at), &self.snapshot(None), STATUS_RUNNING)
    }
}

/// The shell arguments that run `command` as one string, never split or re-quoted here.
///
/// Unix shells get login semantics like the PTY does, so profiles fill `PATH` and tools such as `az`
/// resolve. Windows chooses by shell family.
fn shell_args(shell: &str, command: &str) -> Vec<String> {
    use crate::agent::inject::{shell_kind, ShellKind};
    if cfg!(windows) {
        match shell_kind(shell) {
            ShellKind::PowerShell | ShellKind::Pwsh => {
                vec!["-NoLogo".into(), "-Command".into(), command.to_string()]
            }
            ShellKind::Cmd => vec!["/C".into(), command.to_string()],
            ShellKind::Posix | ShellKind::Fish => vec!["-l".into(), "-c".into(), command.to_string()],
        }
    } else {
        vec!["-l".into(), "-c".into(), command.to_string()]
    }
}

/// The shell this session's commands run in: the PTY's resolution, minus WSL, which is a distribution
/// rather than an executable and is refused for agent sessions there as well.
fn session_shell(app: &AppCtx, kind: SessionKind, persisted: Option<&str>) -> String {
    let data_dir = app.data_dir().ok();
    let (shell, _) = crate::pty::manager::resolve_shell(kind, persisted.map(str::to_string), data_dir.as_deref());
    if shell.starts_with(crate::pty::manager::WSL_SHELL_PREFIX) {
        return crate::pty::manager::default_shell(kind, data_dir.as_deref());
    }
    shell
}

/// Start `command` for a conversation and report back through `on_update` and `on_finish`.
///
/// Returns as soon as the process is running; two reader threads and one wait thread carry on. `kind` and
/// `persisted_shell` are the session's, `cwd` is the agent's directory, and the environment is the one
/// the agent itself gets.
#[allow(clippy::too_many_arguments)]
pub fn spawn_run(
    app: &AppCtx,
    session_id: &str,
    kind: SessionKind,
    persisted_shell: Option<&str>,
    cwd: Option<&str>,
    message_id: &str,
    command: &str,
    on_update: impl Fn(&ShellRun) + Send + Sync + 'static,
    on_finish: impl FnOnce(&ShellRun, ShellContext) + Send + 'static,
) -> Result<Arc<ShellRun>, String> {
    // Windows has no shell mode; see `chat_run_shell`.
    if cfg!(windows) { return Err(UNSUPPORTED.into()); }
    let shell = session_shell(app, kind, persisted_shell);
    let mut cmd = crate::host::command(&shell);
    configure_shell_command(&mut cmd, &shell, command);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    super::engine::agent_environment_in_shell(app, session_id, &mut cmd, Some(&shell));
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        // Its own process group, so cancel takes down the helpers the command starts, not only the shell.
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start the shell \"{shell}\": {e}"))?;
    let stdout = child.stdout.take().ok_or("chat_shell_start_uncertain: The shell has no output stream")?;
    let stderr = child.stderr.take().ok_or("chat_shell_start_uncertain: The shell has no error stream")?;
    let run = Arc::new(ShellRun {
        id: message_id.to_string(),
        command: command.to_string(),
        started_at: now_ms(),
        child: Mutex::new(child),
        output_incomplete: AtomicBool::new(false),
        stdout: Arc::new(Mutex::new(TailBuffer::default())),
        stderr: Arc::new(Mutex::new(TailBuffer::default())),
        cancelled: AtomicBool::new(false),
        abandoned: AtomicBool::new(false),
        last_upsert: Mutex::new(Instant::now()),
        drain_deadline: Mutex::new(None),
    });
    let on_update = Arc::new(on_update);
    let readers = [
        spawn_pump(stdout, run.clone(), run.stdout.clone(), on_update.clone()),
        spawn_pump(stderr, run.clone(), run.stderr.clone(), on_update.clone()),
    ];
    let waited = run.clone();
    std::thread::spawn(move || {
        let status = loop {
            match waited.child.lock().unwrap().try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {}
                Err(_) => break None,
            }
            std::thread::sleep(WAIT_POLL);
        };
        *waited.drain_deadline.lock().unwrap() = Some(Instant::now() + PIPE_DRAIN_GRACE);
        // The parent may exit while a helper still owns the pipe. Readers poll this deadline and close
        // their own handles, so even descendants outside our process group cannot retain these threads.
        for reader in readers {
            let _ = reader.join();
        }
        #[cfg(unix)]
        crate::host::kill_process_tree(&mut waited.child.lock().unwrap());
        let exit_code = status.and_then(|s| s.code()).unwrap_or(-1);
        let mut ctx = waited.snapshot(Some(exit_code));
        if ctx.cancelled {
            ctx.exit_code = None;
        }
        on_finish(&waited, ctx);
    });
    Ok(run)
}

/// Read one pipe into its tail buffer, refreshing the row at most every `UPSERT_INTERVAL`.
///
/// Chunks rather than lines: progress output without a newline would otherwise never show, and a
/// multibyte character split across two reads is carried over instead of being replaced.
fn spawn_pump(
    mut pipe: impl ShellPipe,
    run: Arc<ShellRun>,
    buffer: Arc<Mutex<TailBuffer>>,
    on_update: Arc<impl Fn(&ShellRun) + Send + Sync + 'static>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut bytes = [0u8; 4096];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            if run.drain_deadline.lock().unwrap().is_some_and(|deadline| Instant::now() >= deadline) {
                run.output_incomplete.store(true, Ordering::Relaxed);
                break;
            }
            let available = match pipe_ready(&pipe, bytes.len()) {
                Ok(0) => {
                    #[cfg(not(unix))]
                    std::thread::sleep(WAIT_POLL);
                    continue;
                }
                Ok(available) => available,
                Err(error) => {
                    if error.raw_os_error() != Some(109) { run.output_incomplete.store(true, Ordering::Relaxed); }
                    break;
                }
            };
            let n = match pipe.read(&mut bytes[..available]) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => { run.output_incomplete.store(true, Ordering::Relaxed); break; }
            };
            pending.extend_from_slice(&bytes[..n]);
            let valid = match std::str::from_utf8(&pending) {
                Ok(_) => pending.len(),
                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                Err(_) => pending.len(),
            };
            let chunk = String::from_utf8_lossy(&pending[..valid]).into_owned();
            pending.drain(..valid);
            buffer.lock().unwrap().push(&chunk);
            let due = {
                let mut last = run.last_upsert.lock().unwrap();
                if last.elapsed() >= UPSERT_INTERVAL {
                    *last = Instant::now();
                    true
                } else {
                    false
                }
            };
            if due {
                on_update(&run);
            }
        }
        if !pending.is_empty() {
            buffer.lock().unwrap().push(&String::from_utf8_lossy(&pending));
        }
    })
}

#[cfg(unix)]
trait ShellPipe: Read + std::os::fd::AsRawFd + Send + 'static {}
#[cfg(unix)]
impl<T: Read + std::os::fd::AsRawFd + Send + 'static> ShellPipe for T {}
#[cfg(windows)]
trait ShellPipe: Read + std::os::windows::io::AsRawHandle + Send + 'static {}
#[cfg(windows)]
impl<T: Read + std::os::windows::io::AsRawHandle + Send + 'static> ShellPipe for T {}

#[cfg(unix)]
fn pipe_ready(pipe: &impl ShellPipe, capacity: usize) -> std::io::Result<usize> {
    let mut descriptor = libc::pollfd { fd: pipe.as_raw_fd(), events: libc::POLLIN, revents: 0 };
    // Wake immediately when a blocked producer writes, while still checking the drain deadline.
    let ready = unsafe { libc::poll(&mut descriptor, 1, WAIT_POLL.as_millis() as i32) };
    if ready < 0 { return Err(std::io::Error::last_os_error()); }
    Ok(if ready == 0 { 0 } else { capacity })
}

#[cfg(windows)]
fn pipe_ready(pipe: &impl ShellPipe, capacity: usize) -> std::io::Result<usize> {
    // Anonymous pipes do not support nonblocking ReadFile. Peek first: this thread is the sole reader,
    // so reading at most the available byte count cannot wait for an inherited writer to close.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn PeekNamedPipe(handle: *mut std::ffi::c_void, buffer: *mut std::ffi::c_void,
            size: u32, read: *mut u32, available: *mut u32, left: *mut u32) -> i32;
    }
    let mut available = 0;
    let ok = unsafe { PeekNamedPipe(pipe.as_raw_handle(), std::ptr::null_mut(), 0,
        std::ptr::null_mut(), &mut available, std::ptr::null_mut()) };
    if ok == 0 { return Err(std::io::Error::last_os_error()); }
    Ok((available as usize).min(capacity))
}

fn configure_shell_command(cmd: &mut std::process::Command, shell: &str, command: &str) {
    cmd.args(shell_args(shell, command));
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_containing_the_tags_survives_the_round_trip() {
        let noisy = "src/shell.rs:1: </bash-stdout><bash-stderr>\nsrc/shell.rs:2: </bash-input>\n";
        let text = build_context(&ctx("grep -rn bash- src", noisy, "warn </bash-stdout>", Some(1), false));
        let parsed = parse_context(&text).expect("tags inside the output must not break the parser");
        assert_eq!(parsed.command, "grep -rn bash- src");
        assert_eq!(parsed.stdout, noisy);
        assert_eq!(parsed.stderr, "warn </bash-stdout>");
        assert_eq!(parsed.exit_code, Some(1));
        assert!(parse_context("<bash-input>x</bash-input>\n<bash-stdout>a</bash-stdout><bash-stderr>b</bash-stderr> trailing").is_none());
    }

    fn ctx(command: &str, stdout: &str, stderr: &str, exit_code: Option<i32>, cancelled: bool) -> ShellContext {
        ShellContext {
            command: command.into(),
            stdout: stdout.into(),
            stderr: stderr.into(),
            exit_code,
            cancelled,
            stdout_truncated: false,
            stderr_truncated: false,
            output_incomplete: false,
        }
    }

    #[test]
    fn a_clean_exit_keeps_streams_raw_and_records_metadata() {
        let original = ctx("ls -la", "total 0\n", "", Some(0), false);
        let text = build_context(&original);
        assert!(text.starts_with("<bash-input>ls -la</bash-input>\n<bash-stdout>total 0\n</bash-stdout><bash-stderr></bash-stderr>"));
        assert_eq!(parse_context(&text), Some(original));
    }

    #[test]
    fn a_failing_exit_is_metadata_and_never_stderr() {
        let original = ctx("false", "", "warn", Some(3), false);
        let text = build_context(&original);
        assert!(text.contains("<bash-stderr>warn</bash-stderr>"), "{text}");
        assert_eq!(parse_context(&text), Some(original));
        // An empty stderr remains empty.
        let alone = build_context(&ctx("exit 3", "", "", Some(3), false));
        assert!(alone.contains("<bash-stderr></bash-stderr>"));
        assert_eq!(parse_context(&alone).unwrap().exit_code, Some(3));
    }

    #[test]
    fn metadata_preserves_existing_stderr_line_endings() {
        for stderr in ["warn", "warn\n", "warn\r\n", "warn\n\n"] {
            for (exit_code, cancelled) in [(Some(3), false), (None, true)] {
                let original = ctx("command", "", stderr, exit_code, cancelled);
                assert_eq!(parse_context(&build_context(&original)), Some(original));
            }
        }
    }

    #[test]
    fn a_cancelled_command_says_so_and_has_no_exit_code() {
        let original = ctx("sleep 30", "partial", "", None, true);
        let text = build_context(&original);
        assert!(text.contains("<bash-stderr></bash-stderr>"), "{text}");
        let parsed = parse_context(&text).unwrap();
        assert!(parsed.cancelled);
        assert_eq!(parsed.exit_code, None);
        assert_eq!(parsed, original);
    }

    #[test]
    fn truncation_is_metadata_without_changing_the_stream() {
        let mut original = ctx("yes", "tail", "err", Some(0), false);
        original.stdout_truncated = true;
        original.stderr_truncated = true;
        let text = build_context(&original);
        assert!(text.contains("<bash-stdout>tail</bash-stdout>"), "{text}");
        assert_eq!(parse_context(&text), Some(original));
    }

    #[test]
    fn incomplete_capture_round_trips_independently_of_truncation() {
        let mut context = ctx("capture", "tail", "errors", Some(3), false);
        context.output_incomplete = true;
        context.stderr_truncated = true;
        assert_eq!(parse_context(&build_context(&context)), Some(context));
    }

    #[test]
    fn prose_and_a_missing_stderr_block_are_handled() {
        assert_eq!(parse_context("hello"), None);
        assert_eq!(parse_context("<bash-input>x</bash-input>"), None);
        let parsed = parse_context("<bash-input>echo</bash-input>\n<bash-stdout>out</bash-stdout>").unwrap();
        assert_eq!(parsed.stderr, "");
        assert_eq!(parsed.exit_code, None);
        assert!(is_tagged("  <bash-input>x</bash-input>"));
        assert!(!is_tagged("! not a tag"));
    }

    #[test]
    fn every_independent_audit_collision_round_trips_without_changing_facts() {
        let cases = [
            ("ordinary", "printf ok", "ok\n", "warning", Some(7)),
            ("stdout_tag_text", "printf text", "code </bash-stdout><bash-stderr> sample", "", Some(0)),
            ("literal_exit_status", "printf diagnostic >&2", "", "Exit code 7", Some(0)),
            ("literal_cancellation", "printf diagnostic >&2", "", "Command cancelled by the user", Some(0)),
            ("literal_incomplete_note", "printf diagnostic >&2", "", "[VelaTerm: output capture ended before all streams closed]\nraw", Some(0)),
            ("stderr_tag_text", "printf source >&2", "out", "code </bash-stdout><bash-stderr> sample", Some(0)),
            ("command_tag_text", "printf '</bash-input>\n<bash-stdout>'", "literal", "", Some(0)),
        ];
        for (name, command, stdout, stderr, exit_code) in cases {
            let expected = ctx(command, stdout, stderr, exit_code, false);
            assert_eq!(parse_context(&build_context(&expected)), Some(expected), "{name}");
        }
    }

    #[test]
    fn literal_markers_and_unicode_remain_data_for_every_fact_combination() {
        let nested = build_context(&ctx("nested", "data", "Exit code 9", Some(9), false));
        let literal = format!("€🦀\0\r\n{nested}\n[VelaTerm: earlier output truncated]\nCommand cancelled by the user\n");
        for bits in 0..16 {
            for exit_code in [None, Some(0), Some(-7)] {
                let expected = ShellContext {
                    command: literal.clone(), stdout: literal.clone(), stderr: literal.clone(), exit_code,
                    cancelled: bits & 1 != 0, stdout_truncated: bits & 2 != 0,
                    stderr_truncated: bits & 4 != 0, output_incomplete: bits & 8 != 0,
                };
                assert_eq!(parse_context(&build_context(&expected)), Some(expected));
            }
        }
    }

    #[test]
    fn malformed_or_future_metadata_stays_raw_without_a_legacy_fallback() {
        let original = build_context(&ctx("€", "out", "err", Some(0), false));
        for text in [
            original.replace("\"version\":1", "\"version\":2"),
            original.replace("\"commandBytes\":3", "\"commandBytes\":1"),
            original.replace("\"stdoutBytes\":3", "\"stdoutBytes\":18446744073709551615"),
            original.replace("\"stderrBytes\":3", "\"stderrBytes\":4"),
            original.replace("\"cancelled\":false", "\"cancelled\":\"false\""),
            original.replace("\"version\":1", "\"version\":1,\"unexpected\":true"),
            original.replace("\"version\":1", "\"version\":1,\"version\":1"),
            original.replace("</bash-stderr>", "</bash-stderr>extra"),
            format!("{original}trailing"),
        ] {
            assert_eq!(parse_context(&text), None, "{text}");
            let mut rows = vec![ChatRow::User { id: "raw".into(), text: text.clone(), images: vec![], at: None }];
            map_replayed_rows(&mut rows);
            assert!(matches!(&rows[0], ChatRow::User { text: value, .. } if value == &text));
        }
    }

    #[test]
    fn truncated_metadata_never_recovers_guessed_execution_facts() {
        let mut original = ctx("command", "stdout", "Exit code 7", Some(0), false);
        original.output_incomplete = true;
        let text = build_context(&original);
        let marker = text.rfind(METADATA_START).unwrap();
        for end in marker..text.len() {
            let truncated = &text[..end];
            if let Some(parsed) = parse_context(truncated) {
                // A cut exactly before the metadata can look like an old message. Its data survives,
                // but none of the absent execution facts may be inferred from that data.
                assert_eq!(parsed, ctx("command", "stdout", "Exit code 7", None, false));
            }
        }
        assert_eq!(parse_context(&text), Some(original));
    }

    #[test]
    fn legacy_status_looking_lines_remain_literal_with_unknown_exit() {
        for literal in ["Exit code 7", "Command cancelled by the user", "[VelaTerm: earlier output truncated]\ntail",
            "[VelaTerm: output capture ended before all streams closed]\nraw"] {
            let text = format!("<bash-input>legacy</bash-input>\n<bash-stdout>{literal}</bash-stdout><bash-stderr>{literal}</bash-stderr>");
            assert_eq!(parse_context(&text), Some(ctx("legacy", literal, literal, None, false)));
        }
    }

    #[test]
    fn ambiguous_legacy_delimiters_preserve_the_original_message() {
        for text in [
            "<bash-input>printf '</bash-input>\n<bash-stdout>'</bash-input>\n<bash-stdout>literal</bash-stdout><bash-stderr></bash-stderr>",
            "<bash-input>printf source</bash-input>\n<bash-stdout>out</bash-stdout><bash-stderr>code </bash-stdout><bash-stderr> sample</bash-stderr>",
        ] {
            assert_eq!(parse_context(text), None);
            let mut rows = vec![ChatRow::User { id: "legacy".into(), text: text.into(), images: vec![], at: Some(12) }];
            map_replayed_rows(&mut rows);
            assert!(matches!(&rows[0], ChatRow::User { id, text: value, at: Some(12), .. } if id == "legacy" && value == text));
        }
    }

    #[test]
    fn the_tail_buffer_keeps_the_last_cap_bytes_on_a_character_boundary() {
        let mut buffer = TailBuffer::default();
        // 300 KiB of three-byte characters: the cut must land between characters, not inside one.
        let chunk: String = std::iter::repeat('€').take(1024).collect();
        for _ in 0..100 {
            buffer.push(&chunk);
        }
        assert!(buffer.truncated());
        assert!(buffer.text().len() <= OUTPUT_CAP);
        assert!(buffer.text().len() > OUTPUT_CAP - 3);
        assert!(buffer.text().chars().all(|c| c == '€'));
        let mut small = TailBuffer::default();
        small.push("abc");
        assert!(!small.truncated());
        assert_eq!(small.text(), "abc");
    }

    #[test]
    fn replayed_tagged_user_rows_become_command_rows() {
        let text = build_context(&ctx("git status", "clean\n", "", Some(0), false));
        let mut rows = vec![
            ChatRow::User { id: "h-0".into(), text: "hello".into(), images: Vec::new(), at: Some(1) },
            ChatRow::User { id: "h-1".into(), text, images: Vec::new(), at: Some(2) },
        ];
        map_replayed_rows(&mut rows);
        assert!(matches!(&rows[0], ChatRow::User { text, .. } if text == "hello"));
        match &rows[1] {
            ChatRow::Shell { id, command, stdout, status, exit_code, at, .. } => {
                assert_eq!(id, "h-1");
                assert_eq!(command, "git status");
                assert_eq!(stdout, "clean\n");
                assert_eq!(*status, STATUS_COMPLETED);
                assert_eq!(*exit_code, Some(0));
                assert_eq!(*at, Some(2));
            }
            other => panic!("expected a shell row, got {other:?}"),
        }
    }

    #[cfg(unix)]
    mod runner {
        use super::*;
        use std::sync::mpsc;

        fn app(tag: &str) -> AppCtx {
            let dir = std::env::temp_dir().join(format!("vlx-chat-shell-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let db = crate::db::Db::open(&dir.join("t.db")).unwrap();
            let host = Arc::new(crate::host::HeadlessHost::new(dir, db));
            // The agent environment names this instance's hook endpoint; no listener is needed here.
            host.set_hooks(crate::agent::server::HookServer { port: 0, token: "fixture".into() });
            AppCtx::Headless(host)
        }

        fn run(tag: &str, command: &str) -> (Arc<ShellRun>, mpsc::Receiver<ShellContext>) {
            let (tx, rx) = mpsc::channel();
            // `/bin/sh` has a quiet login profile on every Unix, so the assertions see the command alone.
            let run = spawn_run(&app(tag), "s", SessionKind::Claude, Some("/bin/sh"), Some("/"), "sh-1", command, |_| {}, move |_, ctx| {
                let _ = tx.send(ctx);
            })
            .unwrap();
            (run, rx)
        }

        #[test]
        fn echo_output_arrives_with_a_clean_exit() {
            let (_run, rx) = run("echo", "echo hello; echo oops >&2");
            let ctx = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            assert!(ctx.stdout.ends_with("hello\n"), "{ctx:?}");
            assert!(ctx.stderr.ends_with("oops\n"), "{ctx:?}");
            assert_eq!(ctx.exit_code, Some(0));
            assert!(!ctx.cancelled);
            assert_eq!(ctx.command, "echo hello; echo oops >&2");
        }

        #[test]
        fn inherited_pipes_have_a_bounded_drain_and_report_incomplete_output() {
            let started = Instant::now();
            let (run, rx) = run("inherited-pipe", "sleep 30 & echo parent-finished");
            let context = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(started.elapsed() < Duration::from_secs(5));
            assert!(context.stdout.contains("parent-finished"));
            assert!(context.output_incomplete);
            assert_eq!(context.exit_code, Some(0));
            run.cancel();
            assert!(!run.cancelled.load(Ordering::Relaxed), "a late cancellation must not relabel completion");
        }

        #[test]
        fn large_output_is_bounded_and_live_previews_are_small() {
            let (tx, rx) = mpsc::channel();
            let sizes = Arc::new(Mutex::new(Vec::new()));
            let updates = sizes.clone();
            let command = "head -c 400000 /dev/zero | tr '\\0' x; sleep 1; printf end";
            let _run = spawn_run(&app("large"), "large", SessionKind::Claude, Some("/bin/sh"), Some("/"), "sh-large", command,
                move |run| { let value = run.snapshot(None); updates.lock().unwrap().push(value.stdout.len()); },
                move |_, value| { let _ = tx.send(value); }).unwrap();
            let context = rx.recv_timeout(Duration::from_secs(10)).unwrap();
            assert!(context.stdout.ends_with("end"));
            assert!(context.stdout.len() <= OUTPUT_CAP);
            assert!(context.stdout_truncated);
            assert!(!context.output_incomplete);
            let sizes = sizes.lock().unwrap();
            assert!(sizes.len() <= 5, "updates are throttled: {sizes:?}");
            assert!(sizes.iter().all(|size| *size <= LIVE_OUTPUT_CAP));
        }

        #[test]
        fn configured_login_shell_reads_its_profile() {
            let directory = std::env::temp_dir().join(format!("vlx-shell-login-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join(".zprofile"), "export VLX_LOGIN_PROBE=profile-loaded\n").unwrap();
            let mut command = std::process::Command::new("/bin/zsh");
            configure_shell_command(&mut command, "/bin/zsh", "printf %s \"$VLX_LOGIN_PROBE\"");
            let output = command.env("ZDOTDIR", &directory).output().unwrap();
            std::fs::remove_dir_all(directory).unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, b"profile-loaded");
        }

        #[test]
        fn a_failing_command_reports_its_exit_code() {
            let (_run, rx) = run("exit", "exit 3");
            let ctx = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(ctx.exit_code, Some(3));
            assert_eq!(ctx.status(), STATUS_COMPLETED);
        }

        #[test]
        fn cancel_kills_the_whole_process_group() {
            let (run, rx) = run("cancel", "sleep 30 & sleep 30; wait");
            std::thread::sleep(Duration::from_millis(300));
            let pgid = run.pid() as i32;
            assert_eq!(unsafe { libc::kill(-pgid, 0) }, 0, "the group must exist while the command runs");
            run.cancel();
            let ctx = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            assert!(ctx.cancelled);
            assert_eq!(ctx.exit_code, None);
            assert_eq!(ctx.status(), STATUS_CANCELLED);
            // The background `sleep` was in the same group; give init a moment to reap it.
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let alive = unsafe { libc::kill(-pgid, 0) } == 0;
                if !alive || Instant::now() > deadline {
                    assert!(!alive, "the process group survived cancel");
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }

        /// AC2: the command runs where the agent runs and sees what the agent sees. `pwd` proves the
        /// directory binding; `VLX_SESSION_ID` is one of the values `agent_environment` sets for the agent.
        #[test]
        fn the_command_runs_in_the_given_directory_with_the_agent_environment() {
            let dir = std::env::temp_dir().join(format!("vlx-chat-shell-cwd-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let expected = std::fs::canonicalize(&dir).unwrap();
            let (tx, rx) = mpsc::channel();
            let _run = spawn_run(&app("cwd"), "s-env", SessionKind::Claude, Some("/bin/sh"), Some(dir.to_str().unwrap()), "sh-1", "pwd; echo \"$VLX_SESSION_ID:$VLX_ENGINE\"", |_| {}, move |_, ctx| {
                let _ = tx.send(ctx);
            })
            .unwrap();
            let ctx = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(ctx.exit_code, Some(0), "{ctx:?}");
            assert!(ctx.stdout.ends_with(&format!("{}\ns-env:chat\n", expected.display())), "{ctx:?}");
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn unix_shell_arguments_run_the_command_as_one_login_string() {
            assert_eq!(shell_args("/bin/zsh", "echo 'a b' | wc"), vec!["-l", "-c", "echo 'a b' | wc"]);
        }
    }
}
