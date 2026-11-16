//! Native transport for agent status hooks. Hook configurations invoke this executable directly, so VelaTerm
//! never writes or launches a PowerShell script to report agent state. Short-lived: never initialize
//! application or GUI state here.
//!
//! Every handler forwards the agent's JSON payload unchanged to the injected loopback hook server and always
//! exits successfully, so a hook failure can never disrupt the agent. What an agent expects on stdout differs
//! per agent and is part of its [`Spec`].

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

const MAX_PAYLOAD: u64 = 1024 * 1024;
const STDIN_TIMEOUT: Duration = Duration::from_secs(1);
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);
const STOP_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const STOP_HTTP_TIMEOUT: Duration = Duration::from_secs(86400);
const STOP_ALLOW: &[u8] = b"{\"decision\":\"allow\"}\n";

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

fn read_payload(input: impl Read) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    input.take(MAX_PAYLOAD + 1).read_to_end(&mut body).ok()?;
    if body.len() as u64 > MAX_PAYLOAD { return None; }
    // Forward the original bytes, but do not send truncated or malformed hook data.
    let value: serde_json::Value = serde_json::from_slice(&body).ok()?;
    value.is_object().then_some(body)
}

fn bounded_payload(input: impl Read + Send + 'static) -> Option<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new().name("agent-hook-stdin".into()).spawn(move || {
        let _ = tx.send(read_payload(input));
    }).ok()?;
    rx.recv_timeout(STDIN_TIMEOUT).ok().flatten()
}

fn forward(url: &url::Url, body: &[u8]) {
    let agent = ureq::AgentBuilder::new()
        .try_proxy_from_env(false)
        .redirects(0)
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
    let token = super::session_env::token().unwrap_or_default();
    let url = event.filter(|event| events.contains(event)).and_then(|event| callback_url(
        &std::env::var("VLX_SPAWN_URL").unwrap_or_default(),
        &std::env::var("VLX_SESSION_ID").unwrap_or_default(),
        &token, event,
    ));
    if let Some(url) = url {
        if let Some(body) = bounded_payload(std::io::stdin()) { forward(&url, &body); }
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
}
