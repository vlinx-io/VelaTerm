//! Native transport for agent status hooks. Hook configurations invoke this executable directly, so VelaTerm
//! never writes or launches a PowerShell script to report agent state. Short-lived: never initialize
//! application or GUI state here.
//!
//! Every handler forwards the agent's JSON payload unchanged to the injected loopback hook server and always
//! exits successfully, so a hook failure can never disrupt the agent. What an agent expects on stdout differs
//! per agent and is part of its [`Spec`].

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

const MAX_PAYLOAD: u64 = 1024 * 1024;
const STDIN_TIMEOUT: Duration = Duration::from_secs(1);
// Wait briefly for EOF once a complete JSON object arrives so invalid trailing bytes like `{}{}`
// still fail when written back-to-back, while a leaked pipe handle does not stall or drop the hook.
const EOF_GRACE: Duration = Duration::from_millis(25);
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const HTTP_TIMEOUT: Duration = Duration::from_secs(2);
const STOP_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const STOP_HTTP_TIMEOUT: Duration = Duration::from_secs(86400);
const STOP_ALLOW: &[u8] = b"{\"decision\":\"allow\"}\n";

pub(crate) const CHAT_ENGINE_ENV: &str = "VLX_ENGINE";
pub(crate) const CHAT_ENGINE_VALUE: &str = "chat";

/// Whether the process runs inside a chat-engine session whose lifecycle is driven by the protocol.
pub(crate) fn is_chat_session() -> bool {
    std::env::var(CHAT_ENGINE_ENV).is_ok_and(|value| value == CHAT_ENGINE_VALUE)
}

/// Contract of one agent served by `--agent-hook <agent> <event>`. Each `events` list matches the `e=` values
/// that agent's Unix forwarding script posts, so the hook server sees the same events on every platform.
struct Spec {
    /// event arguments the agent's hook configuration may pass.
    events: &'static [&'static str],
    /// Exact stdout the agent expects from an observe-only hook.
    response: &'static [u8],
}

fn spec(agent: &str) -> Option<Spec> {
    Some(match agent {
        // Empty stdout means "no opinion"; `{"decision":"allow"}` would bypass Crush's own permission prompt.
        "crush" => Spec { events: &["working"], response: b"" },
        // Kiro adds a successful hook's stdout to the agent context, so the bridge stays silent.
        "kiro" => Spec { events: &["boot", "kiro_working", "kiro_waiting"], response: b"" },
        // PreInvocation and PostInvocation require `{}` for protojson parsing; `stop` handles `/agy-stop` separately.
        "antigravity" => Spec { events: &["working", "waiting", "stop"], response: b"{}\n" },
        "copilot" => Spec { events: &["boot", "working", "asking", "waiting"], response: b"" },
        // Kimi's forwarding script never wrote to stdout; keep that contract.
        "kimi" => Spec { events: &["boot", "kimi_working", "kimi_asking", "kimi_waiting", "kimi_idle"], response: b"" },
        _ => return None,
    })
}

fn validated_base_url(base: &str, sid: &str, token: &str) -> Option<url::Url> {
    if sid.is_empty() || token.is_empty() {
        return None;
    }
    let url = url::Url::parse(base).ok()?;
    // The injected hook server uses IPv4 loopback. Avoid DNS, redirects, and inherited proxies.
    if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") || url.port().is_none()
        || !url.username().is_empty() || url.password().is_some() || url.path() != "/"
        || url.query().is_some() || url.fragment().is_some()
    {
        return None;
    }
    Some(url)
}

fn callback_url(base: &str, sid: &str, token: &str, event: &str) -> Option<url::Url> {
    let mut url = validated_base_url(base, sid, token)?;
    url.path_segments_mut().ok()?.extend(["hook", sid]);
    url.query_pairs_mut().append_pair("t", token).append_pair("e", event);
    Some(url)
}

fn agy_stop_url(base: &str, sid: &str, token: &str) -> Option<url::Url> {
    let mut url = validated_base_url(base, sid, token)?;
    url.path_segments_mut().ok()?.extend(["agy-stop", sid]);
    url.query_pairs_mut().append_pair("t", token);
    Some(url)
}

fn validate_json_object(body: &[u8]) -> Option<Vec<u8>> {
    // Forward the original bytes, but do not send truncated or malformed hook data.
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    value.is_object().then(|| body.to_vec())
}

#[cfg(test)]
fn read_payload(input: impl Read) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    input.take(MAX_PAYLOAD + 1).read_to_end(&mut body).ok()?;
    if body.len() as u64 > MAX_PAYLOAD {
        return None;
    }
    validate_json_object(&body)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FramingState {
    Leading,
    Inside,
    Closed,
    Invalid,
}

/// Incremental byte scanner that recognizes when a single top-level JSON object closes so an unclosed
/// pipe holding a complete hook payload does not stall waiting for EOF.
struct JsonObjectFraming {
    state: FramingState,
    depth: usize,
    in_string: bool,
    escaped: bool,
}

impl JsonObjectFraming {
    fn new() -> Self {
        Self {
            state: FramingState::Leading,
            depth: 0,
            in_string: false,
            escaped: false,
        }
    }

    fn push(&mut self, bytes: &[u8]) -> FramingState {
        for &b in bytes {
            match self.state {
                FramingState::Invalid => return FramingState::Invalid,
                FramingState::Leading => {
                    if b.is_ascii_whitespace() {
                        continue;
                    }
                    if b == b'{' {
                        self.state = FramingState::Inside;
                        self.depth = 1;
                    } else {
                        self.state = FramingState::Invalid;
                        return FramingState::Invalid;
                    }
                }
                FramingState::Inside => {
                    if self.in_string {
                        if self.escaped {
                            self.escaped = false;
                        } else if b == b'\\' {
                            self.escaped = true;
                        } else if b == b'"' {
                            self.in_string = false;
                        }
                    } else {
                        match b {
                            b'"' => self.in_string = true,
                            b'{' => self.depth += 1,
                            b'}' => {
                                self.depth -= 1;
                                if self.depth == 0 {
                                    self.state = FramingState::Closed;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                FramingState::Closed => {
                    if !b.is_ascii_whitespace() {
                        self.state = FramingState::Invalid;
                        return FramingState::Invalid;
                    }
                }
            }
        }
        self.state
    }
}

#[derive(Default)]
struct StdinReadState {
    valid_object: Option<Vec<u8>>,
    done: bool,
    final_result: Option<Vec<u8>>,
}

fn finish_stdin(state: &Mutex<StdinReadState>, cv: &Condvar, result: Option<Vec<u8>>) {
    let mut guard = state.lock().unwrap();
    guard.valid_object = None;
    guard.done = true;
    guard.final_result = result;
    cv.notify_all();
}

fn bounded_payload(input: impl Read + Send + 'static) -> Option<Vec<u8>> {
    bounded_payload_with(input, false, STDIN_TIMEOUT, EOF_GRACE)
}

fn bounded_payload_with(
    mut input: impl Read + Send + 'static,
    allow_empty_eof: bool,
    timeout: Duration,
    eof_grace: Duration,
) -> Option<Vec<u8>> {
    let shared = std::sync::Arc::new((Mutex::new(StdinReadState::default()), Condvar::new()));
    let worker_shared = std::sync::Arc::clone(&shared);
    std::thread::Builder::new().name("agent-hook-stdin".into()).spawn(move || {
        let (state, cv) = &*worker_shared;
        let mut buf = Vec::new();
        let mut framing = JsonObjectFraming::new();
        let mut valid_bytes: Option<Vec<u8>> = None;
        let mut chunk = [0u8; 8192];
        loop {
            match input.read(&mut chunk) {
                Ok(0) => {
                    let result = match framing.state {
                        FramingState::Closed => valid_bytes,
                        FramingState::Leading if allow_empty_eof => Some(Vec::new()),
                        _ => None,
                    };
                    finish_stdin(state, cv, result);
                    break;
                }
                Ok(n) => {
                    if buf.len() + n > MAX_PAYLOAD as usize {
                        finish_stdin(state, cv, None);
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let was_closed = framing.state == FramingState::Closed;
                    let next = framing.push(&chunk[..n]);
                    if next == FramingState::Invalid {
                        finish_stdin(state, cv, None);
                        break;
                    }
                    if next == FramingState::Closed && !was_closed {
                        if let Some(valid) = validate_json_object(&buf) {
                            valid_bytes = Some(valid.clone());
                            let mut guard = state.lock().unwrap();
                            guard.valid_object = Some(valid);
                            cv.notify_all();
                        } else {
                            finish_stdin(state, cv, None);
                            break;
                        }
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    finish_stdin(state, cv, None);
                    break;
                }
            }
        }
    }).ok()?;

    let (state, cv) = &*shared;
    let deadline = Instant::now() + timeout;
    let mut grace_deadline: Option<Instant> = None;
    let mut guard = state.lock().unwrap();
    loop {
        if guard.done {
            return guard.final_result.take();
        }
        let now = Instant::now();
        if now >= deadline {
            return guard.valid_object.take();
        }
        if guard.valid_object.is_some() {
            let grace = *grace_deadline.get_or_insert_with(|| (now + eof_grace).min(deadline));
            if now >= grace {
                return guard.valid_object.take();
            }
            let (next, _) = cv.wait_timeout(guard, grace.saturating_duration_since(now)).unwrap();
            guard = next;
        } else {
            grace_deadline = None;
            let (next, _) = cv.wait_timeout(guard, deadline.saturating_duration_since(now)).unwrap();
            guard = next;
        }
    }
}

fn forward(url: &url::Url, body: &[u8]) {
    let agent = ureq::AgentBuilder::new()
        .try_proxy_from_env(false)
        .redirects(0)
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_TIMEOUT)
        .build();
    // The response body is irrelevant. Status, connection, and timeout failures are all fail-open.
    let _ = agent.post(url.as_str()).set("Content-Type", "application/json").send_bytes(body);
}

fn forward_stop(url: &url::Url, body: &[u8], timeout: Duration) -> Vec<u8> {
    let agent = ureq::AgentBuilder::new()
        .try_proxy_from_env(false)
        .redirects(0)
        .timeout_connect(STOP_CONNECT_TIMEOUT)
        .timeout(timeout)
        .build();
    let Ok(response) = agent.post(url.as_str()).set("Content-Type", "application/json").send_bytes(body) else {
        return STOP_ALLOW.to_vec();
    };
    if response.status() != 200 {
        return STOP_ALLOW.to_vec();
    }
    let Ok(text) = response.into_string() else {
        return STOP_ALLOW.to_vec();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return STOP_ALLOW.to_vec();
    };
    if value.get("decision").and_then(serde_json::Value::as_str).is_some() {
        let mut out = serde_json::to_vec(&value).unwrap_or_else(|_| b"{\"decision\":\"allow\"}".to_vec());
        out.push(b'\n');
        return out;
    }
    STOP_ALLOW.to_vec()
}

fn run_antigravity_stop() -> ! {
    let token = super::session_env::token().unwrap_or_default();
    let url = agy_stop_url(
        &std::env::var("VLX_SPAWN_URL").unwrap_or_default(),
        &std::env::var("VLX_SESSION_ID").unwrap_or_default(),
        &token,
    );
    let out = match (url, bounded_payload(std::io::stdin())) {
        (Some(url), Some(body)) => forward_stop(&url, &body, STOP_HTTP_TIMEOUT),
        _ => STOP_ALLOW.to_vec(),
    };
    let _ = std::io::stdout().write_all(&out);
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

/// Forward one event when it is one of `events`, print `response`, and exit successfully, including with
/// missing environment or a pipe whose writer never closes. Explicit process exit also ends a blocked stdin
/// worker without waiting for its inherited handle.
pub fn run(event: Option<&str>, events: &[&str], response: &[u8]) -> ! {
    run_with_policy(event, events, response, false)
}

/// Variant for hooks such as Grok and Codex whose callback remains meaningful when stdin closes empty.
pub fn run_allowing_empty_eof(event: Option<&str>, events: &[&str], response: &[u8]) -> ! {
    run_with_policy(event, events, response, true)
}

fn run_with_policy(event: Option<&str>, events: &[&str], response: &[u8], allow_empty_eof: bool) -> ! {
    if !is_chat_session() {
        let token = super::session_env::token().unwrap_or_default();
        let url = event.filter(|event| events.contains(event)).and_then(|event| callback_url(
            &std::env::var("VLX_SPAWN_URL").unwrap_or_default(),
            &std::env::var("VLX_SESSION_ID").unwrap_or_default(),
            &token, event,
        ));
        if let Some(url) = url {
            if let Some(body) = bounded_payload_with(std::io::stdin(), allow_empty_eof, STDIN_TIMEOUT, EOF_GRACE) {
                forward(&url, &body);
            }
        }
    }
    let _ = std::io::stdout().write_all(response);
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

/// Hidden `--agent-hook <agent> <event>` entry. An unknown agent still exits successfully with no output.
pub fn run_agent(args: &[String]) -> ! {
    let agent = args.get(2).map(String::as_str);
    let event = args.get(3).map(String::as_str);
    if agent == Some("antigravity") && event == Some("stop") {
        run_antigravity_stop();
    }
    let Some(spec) = agent.and_then(spec) else {
        run(None, &[], b"")
    };
    run(event, spec.events, spec.response)
}

/// How an agent runs a hook command string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    /// POSIX shell, including the embedded interpreter Crush uses on every platform.
    Posix,
    /// PowerShell, for configurations with a dedicated PowerShell command field.
    PowerShell,
    /// Undocumented: on Windows the agent may use cmd.exe, PowerShell, or a POSIX shell.
    Any,
}

/// Command line that runs `exe` followed by `args` under `shell`. `args` must be plain words.
pub fn command(exe: &Path, shell: Shell, args: &str) -> String {
    let path = exe.to_string_lossy();
    match shell {
        Shell::Posix => format!("'{}' {args}", path.replace('\'', "'\\''")),
        Shell::PowerShell => format!("& '{}' {args}", path.replace('\'', "''")),
        #[cfg(windows)]
        Shell::Any => format!("{} {args}", any_shell_word(&path, || short_path(exe))),
        #[cfg(not(windows))]
        Shell::Any => command(exe, Shell::Posix, args),
    }
}

/// Characters that cmd.exe, PowerShell, and POSIX shells all read literally inside a bare word.
fn bare(path: &str) -> bool {
    !path.is_empty() && path.chars().all(|c| c.is_alphanumeric() || ":\\/._-~+".contains(c))
}

/// A Windows path every candidate shell reads as the same single word.
///
/// cmd.exe, PowerShell, and POSIX shells all run `C:/dir/app.exe args`, whereas every quoting style breaks
/// at least one of them: `"C:\dir\app.exe" args` is a PowerShell syntax error, and single quotes mean
/// nothing to cmd.exe. Backslashes become forward slashes because a POSIX shell drops unquoted backslashes.
/// A path with spaces or metacharacters falls back to its 8.3 short form; when that is unavailable too,
/// double quotes keep cmd.exe and POSIX shells working.
#[cfg_attr(not(windows), allow(dead_code))]
fn any_shell_word(path: &str, short: impl FnOnce() -> Option<String>) -> String {
    let path = if bare(path) { path.to_string() } else { short().filter(|s| bare(s)).unwrap_or_else(|| path.to_string()) };
    let path = path.replace('\\', "/");
    if bare(&path) { path } else { format!("\"{path}\"") }
}

#[cfg(windows)]
fn short_path(exe: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetShortPathNameW;

    let wide: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut buf = vec![0u16; 1024];
    // SAFETY: `wide` is NUL-terminated and outlives the call; the output slice carries its own length.
    let len = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), Some(&mut buf)) } as usize;
    (len > 0 && len < buf.len()).then(|| String::from_utf16_lossy(&buf[..len]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_requires_the_injected_local_server() {
        for base in ["", "https://127.0.0.1:32109", "http://example.com:32109",
            "http://127.0.0.1", "http://127.0.0.1:32109/other", "http://user@127.0.0.1:32109",
            "http://127.0.0.1:32109/?token=other"]
        {
            assert!(callback_url(base, "s", "t", "working").is_none(), "{base}");
        }
        assert!(callback_url("http://127.0.0.1:32109", "", "t", "boot").is_none());
        let url = callback_url("http://127.0.0.1:32109/", "s/a", "t&x=1", "working").unwrap();
        assert_eq!(url.path(), "/hook/s%2Fa");
        assert_eq!(url.query_pairs().collect::<Vec<_>>(), vec![("t".into(), "t&x=1".into()), ("e".into(), "working".into())]);
    }

    #[test]
    fn payload_preserves_json_bytes_and_rejects_invalid_or_oversized_input() {
        let body = b"{\n \"prompt\": \"hello\", \"conversation_id\": \"123\" }\n";
        assert_eq!(read_payload(&body[..]).unwrap(), body);
        for body in [b"".as_slice(), b"{", b"[]", b"null", b"{}{}", b"{\"x\":\"\xff\"}"] {
            assert!(read_payload(body).is_none());
        }
        let mut large = vec![b' '; MAX_PAYLOAD as usize + 1];
        large[..2].copy_from_slice(b"{}");
        assert!(read_payload(&large[..]).is_none());
        assert!(read_payload(&large[..MAX_PAYLOAD as usize]).is_some());
    }

    #[test]
    fn specs_keep_each_agent_event_set_and_stdout_contract() {
        let crush = spec("crush").unwrap();
        assert_eq!((crush.events, crush.response), (&["working"][..], &b""[..]));
        let kiro = spec("kiro").unwrap();
        assert_eq!((kiro.events, kiro.response), (&["boot", "kiro_working", "kiro_waiting"][..], &b""[..]));
        let antigravity = spec("antigravity").unwrap();
        assert_eq!(antigravity.events, &["working", "waiting", "stop"][..]);
        assert_eq!(antigravity.response, b"{}\n");
        let copilot = spec("copilot").unwrap();
        assert_eq!((copilot.events, copilot.response), (&["boot", "working", "asking", "waiting"][..], &b""[..]));
        let kimi = spec("kimi").unwrap();
        assert_eq!(kimi.events, &["boot", "kimi_working", "kimi_asking", "kimi_waiting", "kimi_idle"][..]);
        assert_eq!(kimi.response, b"");
        assert!(spec("cline").is_none());
    }

    #[test]
    fn commands_quote_for_their_shell() {
        let exe = Path::new("/opt/Vela Term/it's/velaterm");
        assert_eq!(command(exe, Shell::Posix, "--agent-hook crush working"),
            "'/opt/Vela Term/it'\\''s/velaterm' --agent-hook crush working");
        assert_eq!(command(exe, Shell::PowerShell, "--agent-hook copilot boot"),
            "& '/opt/Vela Term/it''s/velaterm' --agent-hook copilot boot");
    }

    #[test]
    fn any_shell_word_is_a_bare_forward_slash_path() {
        let unused = || -> Option<String> { panic!("a bare path needs no short form") };
        assert_eq!(any_shell_word(r"C:\Users\vlinx\AppData\Local\velaterm\velaterm.exe", unused),
            "C:/Users/vlinx/AppData/Local/velaterm/velaterm.exe");
        // Non-ASCII letters are literal in every candidate shell.
        assert_eq!(any_shell_word(r"C:\Users\张三\velaterm.exe", unused), "C:/Users/张三/velaterm.exe");
        // Spaces resolve through the 8.3 short form.
        assert_eq!(any_shell_word(r"C:\Users\John Smith\velaterm.exe", || Some(r"C:\Users\JOHNSM~1\velaterm.exe".into())),
            "C:/Users/JOHNSM~1/velaterm.exe");
        // Without a usable short form, double quotes still serve cmd.exe and POSIX shells.
        assert_eq!(any_shell_word(r"C:\Users\John Smith\velaterm.exe", || None), "\"C:/Users/John Smith/velaterm.exe\"");
        assert_eq!(any_shell_word(r"C:\a&b\velaterm.exe", || Some(r"C:\a&b\velaterm.exe".into())), "\"C:/a&b/velaterm.exe\"");
    }

    #[test]
    fn agy_stop_forwards_server_decision_and_fails_open_on_error() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let handle = std::thread::spawn(move || {
            for idx in 0..2 {
                let req = server.recv().unwrap();
                assert_eq!(req.url(), "/agy-stop/s-1?t=tok");
                let body = if idx == 0 {
                    r#"{"decision":"continue"}"#
                } else {
                    "not json"
                };
                let _ = req.respond(tiny_http::Response::from_string(body).with_status_code(200));
            }
        });
        let url = agy_stop_url(&format!("http://127.0.0.1:{port}"), "s-1", "tok").unwrap();
        assert_eq!(
            forward_stop(&url, b"{}", Duration::from_secs(2)),
            b"{\"decision\":\"continue\"}\n"
        );
        assert_eq!(
            forward_stop(&url, b"{}", Duration::from_secs(2)),
            STOP_ALLOW
        );
        // Unreachable loopback port fails open to allow immediately.
        let closed = agy_stop_url("http://127.0.0.1:1", "s-1", "tok").unwrap();
        assert_eq!(forward_stop(&closed, b"{}", Duration::from_secs(1)), STOP_ALLOW);
        handle.join().unwrap();
    }

    struct ChunkReader {
        rx: std::sync::mpsc::Receiver<Vec<u8>>,
        pending: Vec<u8>,
        offset: usize,
    }

    impl ChunkReader {
        fn new(rx: std::sync::mpsc::Receiver<Vec<u8>>) -> Self {
            Self { rx, pending: Vec::new(), offset: 0 }
        }
    }

    impl Read for ChunkReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            while self.offset >= self.pending.len() {
                match self.rx.recv() {
                    Ok(next) => {
                        self.pending = next;
                        self.offset = 0;
                    }
                    Err(_) => return Ok(0),
                }
            }
            let avail = &self.pending[self.offset..];
            let count = avail.len().min(buf.len());
            buf[..count].copy_from_slice(&avail[..count]);
            self.offset += count;
            Ok(count)
        }
    }

    #[test]
    fn bounded_payload_recovers_complete_json_when_writer_never_closes_stdin() {
        let (tx, rx) = std::sync::mpsc::channel();
        let payload = br#"{"hookEventName":"pre_tool_use","toolName":"echo \"{hi}\""}"#.to_vec();
        tx.send(payload.clone()).unwrap();
        // Hold `tx` open without closing so EOF never arrives; the framing scanner must unblock promptly.
        let started = Instant::now();
        let got = bounded_payload_with(
            ChunkReader::new(rx),
            false,
            Duration::from_secs(2),
            Duration::from_millis(25),
        );
        assert_eq!(got, Some(payload));
        assert!(started.elapsed() < Duration::from_millis(500));
        drop(tx);
    }

    #[test]
    fn bounded_payload_times_out_on_hung_empty_or_incomplete_stdin() {
        let (_tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let started = Instant::now();
        assert!(bounded_payload_with(
            ChunkReader::new(rx),
            true,
            Duration::from_millis(80),
            Duration::from_millis(20),
        )
        .is_none());
        assert!(started.elapsed() >= Duration::from_millis(70));
        assert!(started.elapsed() < Duration::from_secs(1));

        let (tx2, rx2) = std::sync::mpsc::channel();
        tx2.send(br#"{"hookEventName":"pre"#.to_vec()).unwrap();
        assert!(bounded_payload_with(
            ChunkReader::new(rx2),
            true,
            Duration::from_millis(80),
            Duration::from_millis(20),
        )
        .is_none());
        drop(tx2);
    }

    #[test]
    fn bounded_payload_rejects_trailing_junk_and_respects_empty_eof_policy() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(b"{}".to_vec()).unwrap();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            let _ = tx.send(b"{}".to_vec());
        });
        assert!(bounded_payload_with(
            ChunkReader::new(rx),
            false,
            Duration::from_secs(1),
            Duration::from_millis(40),
        )
        .is_none());

        assert!(bounded_payload_with(&b""[..], false, Duration::from_secs(1), Duration::from_millis(25)).is_none());
        assert_eq!(
            bounded_payload_with(&b""[..], true, Duration::from_secs(1), Duration::from_millis(25)),
            Some(Vec::new())
        );
        assert_eq!(
            bounded_payload_with(&b"  \n"[..], true, Duration::from_secs(1), Duration::from_millis(25)),
            Some(Vec::new())
        );

        let mut large = vec![b' '; MAX_PAYLOAD as usize + 1];
        large[..2].copy_from_slice(b"{}");
        assert!(bounded_payload_with(
            std::io::Cursor::new(large),
            false,
            Duration::from_secs(1),
            Duration::from_millis(25),
        )
        .is_none());
    }

    #[test]
    fn chat_session_skips_stdin_and_http_and_exits_immediately() {
        if std::env::var_os("VLX_HOOK_BRIDGE_CHAT_TEST_CHILD").is_some() {
            run_allowing_empty_eof(Some("working"), &["working"], b"");
        }
        let exe = std::env::current_exe().unwrap();
        let started = Instant::now();
        let mut child = std::process::Command::new(exe)
            .args([
                "--exact",
                "agent::hook_bridge::tests::chat_session_skips_stdin_and_http_and_exits_immediately",
                "--nocapture",
            ])
            .env("VLX_HOOK_BRIDGE_CHAT_TEST_CHILD", "1")
            .env(CHAT_ENGINE_ENV, CHAT_ENGINE_VALUE)
            .env("VLX_SPAWN_URL", "http://127.0.0.1:1")
            .env("VLX_SESSION_ID", "s-chat")
            .env("VLX_TOKEN", "tok")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        // Hold stdin open to prove chat mode exits without waiting for EOF.
        let _held_stdin = child.stdin.take().unwrap();
        let status = child.wait().unwrap();
        assert!(status.success());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn hook_forwards_json_and_exits_when_stdin_stays_open() {
        if std::env::var_os("VLX_HOOK_BRIDGE_OPEN_STDIN_CHILD").is_some() {
            run_allowing_empty_eof(Some("working"), &["working"], b"");
        }
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let (tx, rx) = std::sync::mpsc::channel();
        let server_thread = std::thread::spawn(move || {
            let mut req = server.recv().unwrap();
            let url = req.url().to_string();
            let mut body = String::new();
            req.as_reader().read_to_string(&mut body).unwrap();
            let _ = req.respond(tiny_http::Response::empty(200));
            let _ = tx.send((url, body));
        });

        let payload = br#"{"hookEventName":"pre_tool_use","toolName":"bash"}"#;
        let exe = std::env::current_exe().unwrap();
        let started = Instant::now();
        let mut child = std::process::Command::new(exe)
            .args([
                "--exact",
                "agent::hook_bridge::tests::hook_forwards_json_and_exits_when_stdin_stays_open",
                "--nocapture",
            ])
            .env("VLX_HOOK_BRIDGE_OPEN_STDIN_CHILD", "1")
            .env_remove(CHAT_ENGINE_ENV)
            .env("VLX_SPAWN_URL", format!("http://127.0.0.1:{port}"))
            .env("VLX_SESSION_ID", "grok-1")
            .env("VLX_TOKEN", "tok")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut child_stdin = child.stdin.take().unwrap();
        child_stdin.write_all(payload).unwrap();
        child_stdin.flush().unwrap();
        // Keep `child_stdin` open until after `child.wait()` so EOF is never sent.
        let status = child.wait().unwrap();
        drop(child_stdin);
        assert!(status.success());
        assert!(started.elapsed() < Duration::from_secs(2));
        let (url, body) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(url, "/hook/grok-1?t=tok&e=working");
        assert_eq!(body.as_bytes(), payload);
        server_thread.join().unwrap();
    }
}
