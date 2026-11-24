//! Antigravity Google sign-in via `agy acp --login` and sign-out via ACP `logout`.
//!
//! `agy acp --login` runs an out-of-band PKCE OAuth flow (`redirect_uri=https://antigravity.google/oauth-callback`),
//! prints the Google authorization link on stdout without requiring a TTY, and reads the pasted authorization
//! code or full callback URL from stdin. That keeps sign-in identical across desktop, SSH, browser remote
//! access, and mobile clients while leaving credential storage entirely inside `agy`.

use super::{active, emit_extras, finish, now_ms, AuthState, ChatProcess};
use crate::agent::chat::{acp_protocol, engine};
use crate::host::AppCtx;
use crate::models::SessionKind;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{atomic::Ordering, Arc, Mutex};
use std::time::{Duration, Instant};

/// Longest the login helper may take to print its authorization page.
const LINK_LIMIT: Duration = Duration::from_secs(30);
/// Longest a sign-in may wait for the user to finish browser authorization and paste the code.
const LOGIN_LIMIT: Duration = Duration::from_secs(15 * 60);
/// Longest token exchange may run after the user submits the code or callback URL.
const SUBMIT_LIMIT: Duration = Duration::from_secs(120);
/// Longest a canceled login child may take to exit after being killed.
const CANCEL_LIMIT: Duration = Duration::from_secs(10);

/// Child handle for an in-flight `agy acp --login` process.
pub(in crate::agent::chat::engine) struct AntigravityLogin {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl AntigravityLogin {
    fn terminate(&mut self) {
        self.stdin = None;
        let _ = self.child.kill();
    }
}

/// Retain `agy acp` storage and endpoint flags that select where credentials live.
pub(in crate::agent::chat::engine) fn settings_args(args: &[String]) -> Vec<String> {
    const VALUE_FLAGS: &[&str] = &[
        "--gemini-dir",
        "--gemini_dir",
        "--app-data-dir",
        "--app_data_dir",
        "--cli_dir_name",
        "--cloud_code_endpoint",
    ];
    const BOOL_FLAGS: &[&str] = &[
        "--disable-telemetry",
        "--disable_telemetry",
        "--is_google3_workspace",
        "--is_google_internal",
        "--use_stubby_auth",
    ];
    let mut selected = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if VALUE_FLAGS.contains(&arg.as_str()) {
            selected.push(arg.clone());
            if let Some(value) = iter.next() {
                selected.push(value.clone());
            }
        } else if BOOL_FLAGS.contains(&arg.as_str())
            || VALUE_FLAGS
                .iter()
                .chain(BOOL_FLAGS.iter())
                .any(|flag| arg.strip_prefix(flag).is_some_and(|rest| rest.starts_with('=')))
        {
            selected.push(arg.clone());
        }
    }
    selected
}

/// The authorization page `agy acp --login` printed, if it points at Google OAuth over HTTPS.
fn authorization_url(line: &str) -> Option<String> {
    let url = url::Url::parse(line.trim()).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str() != Some("accounts.google.com")
        || !url.path().starts_with("/o/oauth2/")
    {
        return None;
    }
    Some(url.to_string())
}

/// Validate the pasted authorization code or callback URL before forwarding it to `agy acp --login`.
fn validate_code(input: &str) -> Result<&str, &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty()
        || trimmed.len() > 8192
        || trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err("antigravity_auth_invalid_code");
    }
    Ok(trimmed)
}

fn idle(proc: &ChatProcess) -> Result<(), String> {
    if !proc.alive.load(Ordering::Relaxed) {
        return Err("Antigravity has stopped. Please try again.".into());
    }
    let expired = proc
        .extras
        .lock()
        .unwrap()
        .auth
        .as_ref()
        .is_some_and(|s| s.status == "required");
    let turn = proc.turn.lock().unwrap();
    if turn.running {
        return Err("Wait for Antigravity's current turn to finish before changing accounts.".into());
    }
    if !expired && (!turn.active_tasks.is_empty() || !turn.background_tasks.is_empty()) {
        let blocking = engine::blocking_task_names(proc, &turn);
        return Err(format!(
            "Wait for background tasks to finish or stop them before changing accounts: {}",
            blocking.join(", ")
        ));
    }
    drop(turn);
    if !proc.permissions.lock().unwrap().is_empty() {
        return Err("Answer the pending permission request before changing accounts.".into());
    }
    Ok(())
}

fn login_command(proc: &ChatProcess) -> Command {
    let mut cmd = crate::host::command(&proc.bin);
    cmd.arg("acp").arg("--login").args(&proc.auth_settings_args);
    if let Some(dir) = proc
        .cwd
        .as_deref()
        .filter(|d| !d.trim().is_empty())
        .map(str::to_string)
        .or_else(|| crate::host::home_dir().map(|h| h.to_string_lossy().into_owned()))
    {
        cmd.current_dir(dir);
    }
    crate::login_env::refresh_command(&mut cmd);
    crate::agent::executable::prepare_command(&mut cmd, &proc.bin);
    crate::agent::antigravity::prepare_command_env(&mut cmd);
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS {
        cmd.env_remove(key);
    }
    cmd
}

/// Prompt for sign-in when `agy acp` rejects a session or turn with `-32000 Authentication required`.
pub(in crate::agent::chat::engine) fn require(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
) {
    crate::agent::antigravity::invalidate_readiness_cache();
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
) -> Result<(), String> {
    if active(proc) {
        return Ok(());
    }
    idle(proc)?;
    let mut child = login_command(proc)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Could not start Antigravity sign-in.".to_string())?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let attempt = proc.request_id("antigravity_auth_start");
    let mut state = AuthState::new("starting");
    state.attempt = attempt.clone();
    proc.extras.lock().unwrap().auth = Some(state);
    let login = Arc::new(Mutex::new(AntigravityLogin { child, stdin }));
    proc.acp.lock().unwrap().login = Some(login.clone());
    emit_extras(app, session_id, proc);
    crate::diagnostics::record(
        "INFO",
        "antigravity_account",
        json!({"method":"program","sessionId":session_id,"status":"started"}),
    );

    if let Some(stdout) = stdout {
        let (app, session_id, proc, attempt) = (
            app.clone(),
            session_id.to_string(),
            proc.clone(),
            attempt.clone(),
        );
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Some(url) = authorization_url(&line) else {
                    continue;
                };
                let updated = {
                    let mut extras = proc.extras.lock().unwrap();
                    if let Some(state) = extras
                        .auth
                        .as_mut()
                        .filter(|s| s.attempt == attempt && s.status == "starting")
                    {
                        state.status = "pending";
                        state.started_at = now_ms();
                        state.verification_url = Some(url);
                        true
                    } else {
                        false
                    }
                };
                if updated {
                    emit_extras(&app, &session_id, &proc);
                }
            }
        });
    }

    let (app, session_id, proc) = (app.clone(), session_id.to_string(), proc.clone());
    std::thread::spawn(move || {
        let started = Instant::now();
        let outcome = loop {
            std::thread::sleep(Duration::from_millis(200));
            let state = proc
                .extras
                .lock()
                .unwrap()
                .auth
                .as_ref()
                .filter(|s| s.attempt == attempt)
                .map(|s| (s.status, s.started_at));
            let exited = login.lock().unwrap().child.try_wait();
            match exited {
                Ok(Some(code)) => {
                    break if state.map(|(s, _)| s) == Some("canceling") {
                        "canceled"
                    } else if code.success() {
                        "success"
                    } else {
                        "failed"
                    };
                }
                Ok(None) => {
                    let timed_out = match state {
                        None => true,
                        Some(("starting", _)) => started.elapsed() >= LINK_LIMIT,
                        Some(("pending", phase_at)) => {
                            now_ms().saturating_sub(phase_at) >= LOGIN_LIMIT.as_millis() as u64
                        }
                        Some(("submitting", phase_at)) => {
                            now_ms().saturating_sub(phase_at) >= SUBMIT_LIMIT.as_millis() as u64
                        }
                        Some(("canceling", phase_at)) => {
                            now_ms().saturating_sub(phase_at) >= CANCEL_LIMIT.as_millis() as u64
                        }
                        Some(_) => true,
                    };
                    if timed_out || !proc.alive.load(Ordering::Relaxed) {
                        let mut guard = login.lock().unwrap();
                        guard.terminate();
                        let _ = guard.child.wait();
                        break if state.map(|(s, _)| s) == Some("canceling") {
                            "canceled"
                        } else {
                            "failed"
                        };
                    }
                }
                Err(_) => break "failed",
            }
        };
        {
            let mut acp = proc.acp.lock().unwrap();
            if acp.login.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &login)) {
                acp.login = None;
            }
        }
        let _action = proc.action.lock().unwrap();
        let _callback = proc.callbacks.read().unwrap();
        if !app.chat().owns_process(&session_id, &proc) || proc.released.load(Ordering::Relaxed) {
            return;
        }
        if proc
            .extras
            .lock()
            .unwrap()
            .auth
            .as_ref()
            .is_some_and(|s| s.attempt == attempt)
        {
            finish(&app, &session_id, &proc, outcome);
            if outcome == "success" {
                crate::agent::antigravity::bump_auth_epoch();
                crate::agent::antigravity::invalidate_readiness_cache();
                app.chat().clear_antigravity_401_recovery(&session_id);
                // The running ACP server caches its pre-login auth state; release so the next message starts fresh.
                release(&app, &session_id, &proc);
            } else {
                engine::release_if_idle(&app, &session_id, &proc);
            }
        }
    });
    Ok(())
}

pub(in crate::agent::chat::engine) fn submit(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    input: &str,
) -> Result<(), String> {
    let trimmed = validate_code(input).map_err(str::to_string)?;
    let state = proc
        .extras
        .lock()
        .unwrap()
        .auth
        .clone()
        .filter(|s| s.status == "pending")
        .ok_or("Antigravity is not waiting for an authorization code.")?;
    let login = proc
        .acp
        .lock()
        .unwrap()
        .login
        .clone()
        .ok_or("Antigravity sign-in is no longer active.")?;
    let mut stdin = login
        .lock()
        .unwrap()
        .stdin
        .take()
        .ok_or("Antigravity is not waiting for an authorization code.")?;
    let mut next = state;
    next.status = "submitting";
    next.started_at = now_ms();
    next.verification_url = None;
    proc.extras.lock().unwrap().auth = Some(next);
    emit_extras(app, session_id, proc);
    let write_res = writeln!(stdin, "{trimmed}").and_then(|()| stdin.flush());
    drop(stdin);
    if write_res.is_err() {
        login.lock().unwrap().terminate();
        finish(app, session_id, proc, "failed");
        return Err("Could not submit the Antigravity authorization code.".into());
    }
    Ok(())
}

pub(in crate::agent::chat::engine) fn cancel(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
) -> Result<(), String> {
    let mut state = match proc.extras.lock().unwrap().auth.clone() {
        Some(state) if matches!(state.status, "starting" | "pending") => state,
        Some(state) if state.status == "submitting" => {
            return Err("Wait for Antigravity to finish verifying the authorization code.".into());
        }
        _ => return Ok(()),
    };
    state.status = "canceling";
    state.started_at = now_ms();
    proc.extras.lock().unwrap().auth = Some(state);
    emit_extras(app, session_id, proc);
    if let Some(login) = proc.acp.lock().unwrap().login.clone() {
        login.lock().unwrap().terminate();
    }
    Ok(())
}

/// Sign out through ACP's native `logout` method, clearing shared credentials without dropping history.
pub(in crate::agent::chat::engine) fn logout(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
) -> Result<(), String> {
    idle(proc)?;
    if active(proc) {
        return Err("Finish or cancel the current account operation before signing out.".into());
    }
    if proc
        .extras
        .lock()
        .unwrap()
        .auth
        .as_ref()
        .is_some_and(|s| s.status == "signedOut")
    {
        return Ok(());
    }
    let started = now_ms();
    proc.extras.lock().unwrap().auth = Some(AuthState::new("signingOut"));
    emit_extras(app, session_id, proc);
    crate::diagnostics::record(
        "INFO",
        "antigravity_logout",
        json!({"method":"program","sessionId":session_id,"status":"started"}),
    );
    let supported = proc.acp.lock().unwrap().auth_logout_supported;
    let result = if supported {
        engine::acp::release_parked_stop(proc, "allow");
        proc.request_and_wait("acp_auth_logout", acp_protocol::logout)
    } else {
        Err("Antigravity ACP server does not advertise logout support".into())
    };
    let status = if result.is_ok() {
        "signedOut"
    } else {
        "logoutFailed"
    };
    proc.extras.lock().unwrap().auth = Some(AuthState::new(status));
    crate::diagnostics::record(
        "INFO",
        "antigravity_logout",
        json!({
            "method":"program",
            "sessionId":session_id,
            "status": if result.is_ok() { "success" } else { "failed" },
            "durationMs": now_ms().saturating_sub(started)
        }),
    );
    emit_extras(app, session_id, proc);
    crate::agent::antigravity::bump_auth_epoch();
    crate::agent::antigravity::invalidate_readiness_cache();
    release(app, session_id, proc);
    result
        .map(|_| ())
        .map_err(|_| "antigravity_auth_logout_failed".into())
}

/// Kill any in-flight `agy acp --login` helper when the session stops or is released.
pub(in crate::agent::chat::engine) fn abort_login(proc: &ChatProcess) {
    if let Some(login) = proc.acp.lock().unwrap().login.take() {
        login.lock().unwrap().terminate();
    }
}

fn release(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    proc.auth_restart.store(true, Ordering::Relaxed);
    engine::release_process(app, session_id, proc);
}

/// Carry timeline, queued turns, and auth status across the process restart after sign-in or sign-out.
pub(in crate::agent::chat::engine) fn restore(previous: &ChatProcess, proc: &ChatProcess) -> bool {
    if previous.kind != SessionKind::Antigravity || !previous.auth_restart.load(Ordering::Relaxed) {
        return false;
    }
    let prev_rows = previous.timeline.lock().unwrap().rows.clone();
    if !prev_rows.is_empty() {
        proc.timeline.lock().unwrap().replace_all(prev_rows);
    }
    *proc.extras.lock().unwrap() = previous.extras.lock().unwrap().clone();
    *proc.agent_session_id.lock().unwrap() = previous.agent_session_id.lock().unwrap().clone();
    *proc.user_targets.lock().unwrap() = previous.user_targets.lock().unwrap().clone();
    proc.turn.lock().unwrap().waiting = previous.turn.lock().unwrap().waiting.clone();
    proc.release_when_idle.store(
        previous.release_when_idle.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
    {
        let mut prev_acp = previous.acp.lock().unwrap();
        let mut next_acp = proc.acp.lock().unwrap();
        next_acp.suppress_replay_401 = prev_acp.recovering_401;
        next_acp.pending_401_continuation = prev_acp.pending_401_continuation.take();
        next_acp.pending_context_carryover = prev_acp.pending_context_carryover.take();
    }
    proc.compactions.store(
        previous.compactions.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_url_accepts_only_google_oauth_over_https() {
        let valid = "  https://accounts.google.com/o/oauth2/auth?client_id=123&redirect_uri=https%3A%2F%2Fantigravity.google%2Foauth-callback&state=abc\n";
        assert_eq!(
            authorization_url(valid).as_deref(),
            Some(valid.trim())
        );
        for invalid in [
            "Open this link in a browser to log in with Google:",
            "http://accounts.google.com/o/oauth2/auth?state=abc",
            "https://accounts.google.com.evil.example/o/oauth2/auth?state=abc",
            "https://accounts.google.com/signin/v2",
            "https://user:pass@accounts.google.com/o/oauth2/auth?state=abc",
            "https://evil.example/o/oauth2/auth",
        ] {
            assert_eq!(authorization_url(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn validate_code_accepts_code_or_callback_url_and_rejects_whitespace_or_empty() {
        assert_eq!(validate_code("  4/0AeanS0_test-code  "), Ok("4/0AeanS0_test-code"));
        assert_eq!(
            validate_code("https://antigravity.google/oauth-callback?state=s&code=4%2F0test"),
            Ok("https://antigravity.google/oauth-callback?state=s&code=4%2F0test")
        );
        for invalid in ["", "   ", "code with space", "line1\nline2", "bad\x07ctrl"] {
            assert_eq!(validate_code(invalid), Err("antigravity_auth_invalid_code"), "{invalid:?}");
        }
    }

    #[test]
    fn settings_args_keeps_storage_and_endpoint_flags_only() {
        let args: Vec<String> = [
            "--add-dir",
            "/tmp/extra",
            "--gemini-dir",
            "/tmp/gem",
            "--app-data-dir=/tmp/data",
            "--sandbox",
            "--disable-telemetry",
            "--cloud_code_endpoint=https://example.googleapis.com",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        assert_eq!(
            settings_args(&args),
            vec![
                "--gemini-dir",
                "/tmp/gem",
                "--app-data-dir=/tmp/data",
                "--disable-telemetry",
                "--cloud_code_endpoint=https://example.googleapis.com",
            ]
        );
    }
}
