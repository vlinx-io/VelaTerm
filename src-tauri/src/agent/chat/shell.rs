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
//! The tag format is an informed approximation of what the CLI binary emits; the exact template is not
//! extractable from it. Builder and parser are consistent with each other, which is what the view needs.

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

/// First line of a stream whose head was cut, in the message the agent receives.
pub const TRUNCATED_NOTE: &str = "[VelaTerm: earlier output truncated]";
const EXIT_PREFIX: &str = "Exit code ";
const CANCELLED_LINE: &str = "Command cancelled by the user";

/// How often a running command's row is refreshed while output keeps arriving.
const UPSERT_INTERVAL: Duration = Duration::from_millis(100);
/// How often the wait thread checks whether the command has ended.
const WAIT_POLL: Duration = Duration::from_millis(50);

pub const STATUS_RUNNING: &str = "running";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_CANCELLED: &str = "cancelled";

/// Everything one finished command tells the agent, and everything the row shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellContext {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    /// `None` when the command was cancelled: it never reported a code of its own.
    pub exit_code: Option<i32>,
    pub cancelled: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
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

// ─────────────────────────── Tag format ───────────────────────────

/// The one user message the agent receives when a command ends.
pub fn build_context(ctx: &ShellContext) -> String {
    let mut stdout = String::new();
    if ctx.stdout_truncated {
        stdout.push_str(TRUNCATED_NOTE);
        stdout.push('\n');
    }
    stdout.push_str(&ctx.stdout);
    let mut stderr = String::new();
    if ctx.stderr_truncated {
        stderr.push_str(TRUNCATED_NOTE);
        stderr.push('\n');
    }
    stderr.push_str(&ctx.stderr);
    let trailer = if ctx.cancelled {
        Some(CANCELLED_LINE.to_string())
    } else {
        match ctx.exit_code {
            Some(0) | None => None,
            Some(code) => Some(format!("{EXIT_PREFIX}{code}")),
        }
    };
    if let Some(line) = trailer {
        if !stderr.is_empty() && !stderr.ends_with('\n') {
            stderr.push('\n');
        }
        stderr.push_str(&line);
    }
    format!("<bash-input>{}</bash-input>\n<bash-stdout>{stdout}</bash-stdout><bash-stderr>{stderr}</bash-stderr>", ctx.command)
}

/// Whether a message is a shell-mode context message rather than prose.
pub fn is_tagged(text: &str) -> bool {
    text.trim_start().starts_with("<bash-input>")
}

/// Read a context message back. `None` for anything that is not one.
///
/// The tags are in-band, so command output may itself contain them (grepping this repository does).
/// The builder always writes `</bash-input>` followed by a newline and `<bash-stdout>`, one
/// `</bash-stdout><bash-stderr>` seam, and `</bash-stderr>` as the very end of the message; the parser
/// anchors on exactly those: the first input seam, the LAST stdout/stderr seam and the closing tag at
/// the end. Only output whose stderr contains the seam itself is misread, the same limit the terminal
/// UI has, and not worth an escaping scheme the CLI does not use either.
pub fn parse_context(text: &str) -> Option<ShellContext> {
    let rest = text.trim_start().strip_prefix("<bash-input>")?;
    let (command, rest) = rest.split_once("</bash-input>\n<bash-stdout>")
        .or_else(|| rest.split_once("</bash-input><bash-stdout>"))?;
    let rest = rest.trim_end();
    let (stdout, stderr) = match rest.strip_suffix("</bash-stderr>") {
        Some(body) => body.rsplit_once("</bash-stdout><bash-stderr>")?,
        // A message without a stderr part: the stdout tag closes the message.
        None => (rest.strip_suffix("</bash-stdout>")?, ""),
    };
    let (stdout, stdout_truncated) = strip_note(stdout);
    let (stderr, stderr_truncated) = strip_note(stderr);
    let mut ctx = ShellContext {
        command: command.to_string(),
        stdout: stdout.to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        cancelled: false,
        stdout_truncated,
        stderr_truncated,
    };
    let (body, last) = match stderr.rsplit_once('\n') {
        Some((body, last)) => (body, last),
        None => ("", stderr),
    };
    if last == CANCELLED_LINE {
        ctx.cancelled = true;
        ctx.exit_code = None;
        ctx.stderr = body.to_string();
    } else if let Some(code) = last.strip_prefix(EXIT_PREFIX).and_then(|n| n.parse::<i32>().ok()) {
        ctx.exit_code = Some(code);
        ctx.stderr = body.to_string();
    } else {
        ctx.stderr = stderr.to_string();
    }
    Some(ctx)
}

fn strip_note(stream: &str) -> (&str, bool) {
    match stream.strip_prefix(TRUNCATED_NOTE) {
        Some(rest) => (rest.strip_prefix('\n').unwrap_or(rest), true),
        None => (stream, false),
    }
}

/// The timeline row for a command, live or finished.
pub fn row(id: String, at: Option<i64>, ctx: &ShellContext, status: &'static str) -> ChatRow {
    ChatRow::Shell {
        id,
        command: ctx.command.clone(),
        stdout: ctx.stdout.clone(),
        stderr: ctx.stderr.clone(),
        stdout_truncated: ctx.stdout_truncated,
        stderr_truncated: ctx.stderr_truncated,
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
        *slot = row(std::mem::take(id), *at, &ctx, ctx.status());
    }
}

// ─────────────────────────── Runner ───────────────────────────

/// One command running for a conversation.
pub struct ShellRun {
    pub id: String,
    pub command: String,
    pub started_at: i64,
    child: Mutex<Child>,
    stdout: Arc<Mutex<TailBuffer>>,
    stderr: Arc<Mutex<TailBuffer>>,
    cancelled: AtomicBool,
    /// Set when the conversation was stopped underneath the command: the result is not reported to an
    /// agent that has been let go, which would start it again only to tell it.
    abandoned: AtomicBool,
    last_upsert: Mutex<Instant>,
}

impl ShellRun {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        crate::host::kill_process_tree(&mut self.child.lock().unwrap());
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
        ShellContext {
            command: self.command.clone(),
            stdout: stdout.text().to_string(),
            stderr: stderr.text().to_string(),
            exit_code,
            cancelled: self.cancelled.load(Ordering::Relaxed),
            stdout_truncated: stdout.truncated(),
            stderr_truncated: stderr.truncated(),
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
    let shell = session_shell(app, kind, persisted_shell);
    let mut cmd = crate::host::command(&shell);
    cmd.args(shell_args(&shell, command));
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    super::engine::agent_environment(app, session_id, &mut cmd);
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
    let stdout = child.stdout.take().ok_or("The shell has no output stream")?;
    let stderr = child.stderr.take().ok_or("The shell has no error stream")?;
    let run = Arc::new(ShellRun {
        id: message_id.to_string(),
        command: command.to_string(),
        started_at: now_ms(),
        child: Mutex::new(child),
        stdout: Arc::new(Mutex::new(TailBuffer::default())),
        stderr: Arc::new(Mutex::new(TailBuffer::default())),
        cancelled: AtomicBool::new(false),
        abandoned: AtomicBool::new(false),
        last_upsert: Mutex::new(Instant::now()),
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
        for reader in readers {
            let _ = reader.join();
        }
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
    mut pipe: impl Read + Send + 'static,
    run: Arc<ShellRun>,
    buffer: Arc<Mutex<TailBuffer>>,
    on_update: Arc<impl Fn(&ShellRun) + Send + Sync + 'static>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut bytes = [0u8; 4096];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            let n = match pipe.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
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
        }
    }

    #[test]
    fn a_clean_exit_round_trips_without_a_trailer() {
        let original = ctx("ls -la", "total 0\n", "", Some(0), false);
        let text = build_context(&original);
        assert_eq!(text, "<bash-input>ls -la</bash-input>\n<bash-stdout>total 0\n</bash-stdout><bash-stderr></bash-stderr>");
        assert_eq!(parse_context(&text), Some(original));
    }

    #[test]
    fn a_failing_exit_adds_its_code_as_the_last_stderr_line() {
        let original = ctx("false", "", "warn", Some(3), false);
        let text = build_context(&original);
        assert!(text.ends_with("<bash-stderr>warn\nExit code 3</bash-stderr>"), "{text}");
        assert_eq!(parse_context(&text), Some(original));
        // An empty stderr carries the code alone.
        let alone = build_context(&ctx("exit 3", "", "", Some(3), false));
        assert!(alone.ends_with("<bash-stderr>Exit code 3</bash-stderr>"));
        assert_eq!(parse_context(&alone).unwrap().exit_code, Some(3));
    }

    #[test]
    fn a_cancelled_command_says_so_and_has_no_exit_code() {
        let original = ctx("sleep 30", "partial", "", None, true);
        let text = build_context(&original);
        assert!(text.ends_with("<bash-stderr>Command cancelled by the user</bash-stderr>"), "{text}");
        let parsed = parse_context(&text).unwrap();
        assert!(parsed.cancelled);
        assert_eq!(parsed.exit_code, None);
        assert_eq!(parsed, original);
    }

    #[test]
    fn truncation_is_a_note_at_the_head_of_the_stream_and_a_flag() {
        let mut original = ctx("yes", "tail", "err", Some(0), false);
        original.stdout_truncated = true;
        original.stderr_truncated = true;
        let text = build_context(&original);
        assert!(text.contains("<bash-stdout>[VelaTerm: earlier output truncated]\ntail</bash-stdout>"), "{text}");
        assert_eq!(parse_context(&text), Some(original));
    }

    #[test]
    fn prose_and_a_missing_stderr_block_are_handled() {
        assert_eq!(parse_context("hello"), None);
        assert_eq!(parse_context("<bash-input>x</bash-input>"), None);
        let parsed = parse_context("<bash-input>echo</bash-input>\n<bash-stdout>out</bash-stdout>").unwrap();
        assert_eq!(parsed.stderr, "");
        assert_eq!(parsed.exit_code, Some(0));
        assert!(is_tagged("  <bash-input>x</bash-input>"));
        assert!(!is_tagged("! not a tag"));
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
            let _run = spawn_run(&app("cwd"), "s-env", SessionKind::Claude, Some("/bin/sh"), Some(dir.to_str().unwrap()), "sh-1", "pwd; echo \"$VLX_SESSION_ID\"", |_| {}, move |_, ctx| {
                let _ = tx.send(ctx);
            })
            .unwrap();
            let ctx = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(ctx.exit_code, Some(0), "{ctx:?}");
            assert!(ctx.stdout.ends_with(&format!("{}\ns-env\n", expected.display())), "{ctx:?}");
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn unix_shell_arguments_run_the_command_as_one_login_string() {
            assert_eq!(shell_args("/bin/zsh", "echo 'a b' | wc"), vec!["-l", "-c", "echo 'a b' | wc"]);
        }
    }
}
