//! ZCode sign-in and sign-out through its own CLI.
//!
//! ZCode's protocol server has no account methods, so the account panel runs `zcode login <provider>
//! --no-browser --json` beside the conversation. The CLI prints the provider's authorization page, waits
//! while the user approves it in their own browser (the provider redirects to ZCode's server, never to this
//! host), saves the credentials ZCode and its desktop app share, and exits. Only the printed link reaches the
//! view, and only when it points at the provider. A conversation already running keeps the account it
//! started with, so a finished sign-in or sign-out releases an idle process and the next message starts a
//! fresh one.

use super::{active, emit_extras, finish, now_ms, AuthState, ChatProcess};
use crate::host::AppCtx;
use serde_json::json;
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::{atomic::Ordering, Arc, Mutex};
use std::time::Duration;

/// Longest a sign-in may wait for the user before it is stopped and reported as failed.
const LOGIN_LIMIT: Duration = Duration::from_secs(15 * 60);
/// Longest the CLI may take to print its authorization page.
const LINK_LIMIT: Duration = Duration::from_secs(30);

/// The CLI's account providers: Z.ai, and BigModel for mainland China.
fn provider(id: Option<&str>) -> Result<&'static str, String> {
    match id.unwrap_or("zai") {
        "zai" => Ok("zai"),
        "bigmodel" => Ok("bigmodel"),
        _ => Err("Unknown ZCode account provider".into()),
    }
}

/// A command running the session's ZCode with `args`, in the user's environment.
fn cli(proc: &ChatProcess, args: &[&str]) -> std::process::Command {
    let mut cmd = crate::agent::zcode::command(&proc.bin);
    cmd.args(args);
    if let Some(dir) = proc.cwd.as_deref().filter(|d| !d.trim().is_empty()).map(str::to_string)
        .or_else(|| crate::host::home_dir().map(|h| h.to_string_lossy().into_owned()))
    {
        cmd.current_dir(dir);
    }
    crate::login_env::refresh_command(&mut cmd);
    crate::agent::executable::prepare_command(&mut cmd, &proc.bin);
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS {
        cmd.env_remove(key);
    }
    cmd
}

/// The authorization page the CLI printed, if it is one of the providers' own over HTTPS.
fn authorization_url(line: &str) -> Option<String> {
    let url = url::Url::parse(line.trim()).ok()?;
    let host = url.host_str()?;
    let trusted = ["z.ai", "bigmodel.cn"].iter().any(|root| host == *root || host.ends_with(&format!(".{root}")));
    (url.scheme() == "https" && trusted && url.username().is_empty() && url.password().is_none())
        .then(|| url.to_string())
}

/// No usable model until the user signs in: show the account panel in place of the failed turn.
pub(in crate::agent::chat::engine) fn require(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    {
        let mut extras = proc.extras.lock().unwrap();
        if extras.auth.as_ref().is_some_and(AuthState::active) {
            return;
        }
        extras.auth = Some(AuthState::new("required"));
    }
    emit_extras(app, session_id, proc);
}

pub(in crate::agent::chat::engine) fn start(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    provider_id: Option<&str>,
) -> Result<(), String> {
    let provider = provider(provider_id)?;
    if active(proc) {
        return Ok(());
    }
    if proc.turn.lock().unwrap().running {
        return Err("Wait for the current turn to finish before signing in.".into());
    }
    let mut child = cli(proc, &["login", provider, "--no-browser", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Could not start ZCode sign-in.".to_string())?;
    let stderr = child.stderr.take();
    let attempt = proc.request_id("zcode_auth_start");
    let mut state = AuthState::new("starting");
    state.attempt = attempt.clone();
    proc.extras.lock().unwrap().auth = Some(state);
    let child = Arc::new(Mutex::new(child));
    proc.zcode.lock().unwrap().login = Some(child.clone());
    emit_extras(app, session_id, proc);
    crate::diagnostics::record("INFO", "zcode_account", json!({"method":"program","sessionId":session_id,"status":"started"}));

    // The link is the first line that parses as a provider page; everything else the CLI says stays here.
    if let Some(stderr) = stderr {
        let (app, session_id, proc, attempt) = (app.clone(), session_id.to_string(), proc.clone(), attempt.clone());
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let Some(url) = authorization_url(&line) else { continue };
                {
                    let mut extras = proc.extras.lock().unwrap();
                    let Some(state) = extras.auth.as_mut().filter(|s| s.attempt == attempt && s.status == "starting") else { return };
                    state.status = "pending";
                    state.verification_url = Some(url);
                }
                emit_extras(&app, &session_id, &proc);
            }
        });
    }

    let (app, session_id, proc) = (app.clone(), session_id.to_string(), proc.clone());
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let outcome = loop {
            std::thread::sleep(Duration::from_millis(250));
            let status = proc.extras.lock().unwrap().auth.as_ref().filter(|s| s.attempt == attempt).map(|s| s.status);
            let exited = child.lock().unwrap().try_wait();
            match exited {
                Ok(Some(code)) => break if status == Some("canceling") { "canceled" } else if code.success() { "success" } else { "failed" },
                Ok(None) => {
                    let limit = if status == Some("starting") { LINK_LIMIT } else { LOGIN_LIMIT };
                    if status.is_none() || started.elapsed() >= limit {
                        let mut child = child.lock().unwrap();
                        let _ = child.kill();
                        let _ = child.wait();
                        break if status == Some("canceling") { "canceled" } else { "failed" };
                    }
                }
                Err(_) => break "failed",
            }
        };
        proc.zcode.lock().unwrap().login = None;
        let _action = proc.action.lock().unwrap();
        if proc.extras.lock().unwrap().auth.as_ref().is_some_and(|s| s.attempt == attempt) {
            finish(&app, &session_id, &proc, outcome);
        }
        if outcome == "success" {
            restart_when_idle(&app, &session_id, &proc);
        }
    });
    Ok(())
}

pub(in crate::agent::chat::engine) fn cancel(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) -> Result<(), String> {
    let login = proc.zcode.lock().unwrap().login.clone();
    {
        let mut extras = proc.extras.lock().unwrap();
        let Some(state) = extras.auth.as_mut().filter(|s| matches!(s.status, "starting" | "pending")) else {
            return Ok(());
        };
        state.status = "canceling";
    }
    emit_extras(app, session_id, proc);
    if let Some(login) = login {
        let _ = login.lock().unwrap().kill();
    }
    Ok(())
}

/// Sign out through `zcode logout`, which clears the account ZCode and its desktop app share.
pub(in crate::agent::chat::engine) fn logout(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) -> Result<(), String> {
    if active(proc) {
        return Err("Finish or cancel the current account operation before signing out.".into());
    }
    if proc.turn.lock().unwrap().running {
        return Err("Wait for the current turn to finish before signing out.".into());
    }
    let started = now_ms();
    proc.extras.lock().unwrap().auth = Some(AuthState::new("signingOut"));
    emit_extras(app, session_id, proc);
    let signed_out = cli(proc, &["logout"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    proc.extras.lock().unwrap().auth = Some(AuthState::new(if signed_out { "signedOut" } else { "logoutFailed" }));
    crate::diagnostics::record("INFO", "zcode_logout", json!({"method":"program","sessionId":session_id,"status":if signed_out { "success" } else { "failed" },"durationMs":now_ms().saturating_sub(started)}));
    emit_extras(app, session_id, proc);
    if signed_out {
        restart_when_idle(app, session_id, proc);
        Ok(())
    } else {
        Err("zcode_auth_logout_failed".into())
    }
}

/// The running server holds the account it started with; let the next message start one with the new one.
fn restart_when_idle(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    if proc.alive.load(Ordering::Relaxed) && !proc.turn.lock().unwrap().running {
        super::super::release_process(app, session_id, proc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_provider_pages_are_shown() {
        let zai = "https://chat.z.ai/api/oauth/authorize?client_id=c&redirect_uri=https://zcode.z.ai/cb&state=s";
        assert_eq!(authorization_url(&format!("  {zai}\n")).as_deref(), Some(zai));
        assert!(authorization_url("https://bigmodel.cn/login?appId=zcode&state=s").is_some());
        for line in [
            "Open this URL to sign in:",
            "http://chat.z.ai/api/oauth/authorize",
            "https://evil.example/z.ai",
            "https://z.ai.evil.example/login",
            "https://user:pass@chat.z.ai/login",
        ] {
            assert_eq!(authorization_url(line), None, "{line}");
        }
    }

    #[test]
    fn providers_are_zai_or_bigmodel() {
        assert_eq!(provider(None), Ok("zai"));
        assert_eq!(provider(Some("bigmodel")), Ok("bigmodel"));
        assert!(provider(Some("--help")).is_err());
    }
}
