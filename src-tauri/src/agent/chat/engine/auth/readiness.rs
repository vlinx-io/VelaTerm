//! What a conversation's agent needs before it can take a message: to be installed, and for its user to be
//! signed in. The view asks when a conversation opens, so nothing is typed into one that could not deliver it.
//!
//! Only a definite answer may hold a message back. An agent can run on an API key, a third-party provider or a
//! profile this check knows nothing about, so a check that fails, times out or cannot tell reports "unknown",
//! and the agent's own refusal handles that case when a message is sent.

use super::super::agent_environment;
use crate::agent::executable::{self, LaunchBinary};
use crate::agent::session_settings;
use crate::agent::zcode_account;
use crate::host::AppCtx;
use crate::models::SessionKind;
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Longest one status check may take before its answer counts as unknown.
const PROBE_LIMIT: Duration = Duration::from_secs(8);
/// The request id of Codex's `account/read`, which marks the answer in its output.
const CODEX_ACCOUNT_REQUEST: i64 = 2;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentReadiness {
    pub installed: bool,
    /// `None` when this agent's sign-in cannot be told from outside it.
    pub signed_in: Option<bool>,
}

/// The session's agent as the chat engine would start it. Runs commands: never call it on the UI thread.
pub(in crate::agent::chat::engine) fn check(app: &AppCtx, session_id: &str) -> Result<AgentReadiness, String> {
    let (session, root) = {
        let conn = app.db().conn.lock().unwrap();
        let session = crate::db::repo::get_session(&conn, session_id)?.ok_or("Session not found")?;
        let root = crate::db::repo::get_project_root(&conn, &session.project_id)?;
        (session, root)
    };
    if session.engine != "chat" || !session.kind.supports_chat() {
        return Ok(AgentReadiness { installed: true, signed_in: None });
    }
    let missing = AgentReadiness { installed: false, signed_in: None };
    let kind = session.kind;
    let cwd = crate::host::spawn_dir(
        session.cwd.as_deref().filter(|c| !c.trim().is_empty()).or(root.as_deref()),
    );
    let prepared = |bin: &str| {
        let mut cmd = if kind == SessionKind::Zcode { crate::agent::zcode::command(bin) } else { crate::host::command(bin) };
        if let Some(cwd) = &cwd {
            cmd.current_dir(cwd);
        }
        agent_environment(app, session_id, &mut cmd);
        executable::prepare_command(&mut cmd, bin);
        cmd
    };
    let bin = match executable::resolve_launch_binary(app, session_id, kind)? {
        LaunchBinary::Broken => return Ok(missing),
        LaunchBinary::Path(path) => {
            if Path::new(&path).is_absolute() && !Path::new(&path).exists() {
                return Ok(missing);
            }
            // Resolution only reads what is already known; this check runs while the conversation opens, so it
            // may wait for the answer and leave it for the first message.
            if crate::agent::health::check(kind, &path) == crate::agent::health::Health::Broken {
                return Ok(missing);
            }
            path
        }
        // Discovery found nothing, but the engine still starts the bare name, so only that attempt can say.
        LaunchBinary::Unresolved => {
            let name = executable::command_name(kind);
            if !starts(prepared(name)) {
                return Ok(missing);
            }
            name.to_string()
        }
    };
    let args = crate::agent::inject::split_extra_args(Some(&session_settings::without_selection_args(kind, session.agent_args.as_deref())));
    let cmd = prepared(&bin);
    let signed_in = match kind {
        SessionKind::Zcode => zcode(&cmd, &bin, cwd.as_deref()),
        SessionKind::Claude => claude(cmd, &args),
        SessionKind::Codex => codex(cmd, &args),
        SessionKind::Cursor => cursor(cmd, &args),
        _ => None,
    };
    Ok(AgentReadiness { installed: true, signed_in })
}

/// Whether the system can start this command at all. The process does no work: it is stopped at once.
fn starts(mut cmd: Command) -> bool {
    cmd.arg("--version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    match cmd.spawn() {
        Ok(mut child) => {
            let _ = child.kill();
            let _ = child.wait();
            true
        }
        Err(error) => error.kind() != ErrorKind::NotFound,
    }
}

/// The command's output up to the line `done` accepts or its end. `None` when it could not start or took too long.
fn output(mut cmd: Command, input: &[Value], done: impl Fn(&str) -> bool + Send + 'static) -> Option<String> {
    cmd.stdin(if input.is_empty() { Stdio::null() } else { Stdio::piped() })
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    // Held open until the answer is in: a server exits when its input ends.
    let mut stdin = child.stdin.take();
    if let Some(stdin) = stdin.as_mut() {
        for line in input {
            let _ = writeln!(stdin, "{line}");
        }
        let _ = stdin.flush();
    }
    let answer = child.stdout.take().and_then(|stdout| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut text = String::new();
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                text.push_str(&line);
                text.push('\n');
                if done(&line) {
                    break;
                }
            }
            let _ = tx.send(text);
        });
        rx.recv_timeout(PROBE_LIMIT).ok()
    });
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    answer
}

/// Signed out with an empty model catalogue is the state in which ZCode refuses a message. A plan or a personal
/// provider can fill the catalogue without a sign-in, and then the message goes through.
fn zcode(cmd: &Command, bin: &str, cwd: Option<&str>) -> Option<bool> {
    // No readable account config counts as signed out here, exactly as it does when a message is sent.
    if zcode_account::Account::for_command(cmd).and_then(|account| account.signed_in()).is_some() {
        return Some(true);
    }
    let dir = cwd.map(str::to_string).or_else(|| crate::host::home_dir().map(|home| home.to_string_lossy().into_owned()))?;
    crate::agent::zcode::list_models(bin, &dir).ok().map(|models| !models.is_empty())
}

/// `claude auth status` already counts API keys, tokens and third-party providers as signed in.
fn claude(mut cmd: Command, args: &[String]) -> Option<bool> {
    cmd.args(super::claude::settings_args(args)).args(["auth", "status", "--json"]);
    claude_signed_in(&output(cmd, &[], |_| false)?)
}

fn claude_signed_in(output: &str) -> Option<bool> {
    serde_json::from_str::<Value>(output).ok()?.get("loggedIn")?.as_bool()
}

/// Asked of its server rather than `codex login status`, which reports "not logged in" for a configured
/// provider that needs no OpenAI account. The server does not count a key in the environment, so one is unknown.
fn codex(mut cmd: Command, args: &[String]) -> Option<bool> {
    if ["OPENAI_API_KEY", "CODEX_API_KEY"].iter().any(|key| zcode_account::env_of(&cmd, key).is_some()) {
        return None;
    }
    cmd.arg("app-server").arg("--stdio").args(crate::agent::codex_models::app_server_args(args));
    let input = [
        json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"velaterm","version":env!("CARGO_PKG_VERSION")}}}),
        json!({"method":"initialized"}),
        json!({"id":CODEX_ACCOUNT_REQUEST,"method":"account/read","params":{"refreshToken":false}}),
    ];
    let text = output(cmd, &input, |line| is_codex_account_answer(line))?;
    codex_signed_in(&text)
}

fn is_codex_account_answer(line: &str) -> bool {
    serde_json::from_str::<Value>(line).is_ok_and(|value| value.get("id").and_then(Value::as_i64) == Some(CODEX_ACCOUNT_REQUEST))
}

fn codex_signed_in(output: &str) -> Option<bool> {
    let line = output.lines().find(|line| is_codex_account_answer(line))?;
    let answer: Value = serde_json::from_str(line).ok()?;
    let result = answer.get("result")?;
    if !result.get("account").is_none_or(Value::is_null) {
        return Some(true);
    }
    Some(!result.get("requiresOpenaiAuth")?.as_bool()?)
}

/// A key passed to Cursor directly stands in for a sign-in that `status` would not report.
fn cursor(mut cmd: Command, args: &[String]) -> Option<bool> {
    let keyed = ["CURSOR_API_KEY", "CURSOR_AUTH_TOKEN"].iter().any(|key| zcode_account::env_of(&cmd, key).is_some())
        || args.iter().any(|arg| arg == "-a" || arg.starts_with("--api-key"));
    if keyed {
        return None;
    }
    cmd.arg("status");
    cursor_signed_in(&output(cmd, &[], |_| false)?)
}

fn cursor_signed_in(output: &str) -> Option<bool> {
    let text = output.to_lowercase();
    if text.contains("not logged in") {
        Some(false)
    } else if text.contains("logged in") {
        Some(true)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_answer_decides_only_when_it_says_so() {
        assert_eq!(claude_signed_in(r#"{"loggedIn":false,"authMethod":"none"}"#), Some(false));
        assert_eq!(claude_signed_in(r#"{"loggedIn":true,"authMethod":"third_party"}"#), Some(true));
        assert_eq!(claude_signed_in("error: unknown command 'auth'"), None);
    }

    #[test]
    fn codex_needs_a_sign_in_only_when_its_provider_requires_one() {
        let answer = |result: Value| format!("{}\n{}\n", json!({"id":1,"result":{}}), json!({"id":2,"result":result}));
        assert_eq!(codex_signed_in(&answer(json!({"account":null,"requiresOpenaiAuth":true}))), Some(false));
        assert_eq!(codex_signed_in(&answer(json!({"account":null,"requiresOpenaiAuth":false}))), Some(true));
        assert_eq!(codex_signed_in(&answer(json!({"account":{"type":"chatgpt"},"requiresOpenaiAuth":true}))), Some(true));
        assert_eq!(codex_signed_in(&answer(json!({"account":null}))), None);
        assert_eq!(codex_signed_in(&json!({"id":2,"error":{"message":"unknown method"}}).to_string()), None);
    }

    #[test]
    fn cursor_status_text_is_read_without_guessing() {
        assert_eq!(cursor_signed_in("Not logged in\n"), Some(false));
        assert_eq!(cursor_signed_in("Logged in as someone\n"), Some(true));
        assert_eq!(cursor_signed_in("Checking...\n"), None);
    }
}
