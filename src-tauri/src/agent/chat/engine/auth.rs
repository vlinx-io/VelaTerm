//! Provider-managed authorization. Credentials remain inside the provider process.

pub(super) mod claude;
pub(super) mod zcode;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use super::{codex_protocol, emit_extras, now_ms, ChatProcess};
use crate::host::AppCtx;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_code: Option<String>,
    #[serde(skip)]
    login_id: Option<String>,
    #[serde(skip)]
    attempt: String,
    #[serde(skip)]
    started_at: u64,
    #[serde(skip)]
    callback_url: Option<String>,
    #[serde(skip)]
    oauth_state: Option<String>,
    #[serde(skip)]
    completion_id: Option<String>,
    #[serde(skip)]
    submission_id: Option<String>,
}

impl AuthState {
    fn new(status: &'static str) -> Self {
        Self {
            status,
            verification_url: None,
            user_code: None,
            login_id: None,
            attempt: String::new(),
            started_at: now_ms(),
            callback_url: None,
            oauth_state: None,
            completion_id: None,
            submission_id: None,
        }
    }

    pub(super) fn active(&self) -> bool {
        matches!(self.status, "starting" | "pending" | "submitting" | "canceling" | "signingOut")
    }
}

pub(super) fn active(proc: &ChatProcess) -> bool {
    proc.extras
        .lock()
        .unwrap()
        .auth
        .as_ref()
        .is_some_and(AuthState::active)
}

pub(super) fn blocks_queue(proc: &ChatProcess) -> bool {
    proc.extras.lock().unwrap().auth.as_ref().is_some_and(|s| {
        matches!(s.status, "required" | "signedOut" | "logoutFailed") || s.active()
    })
}

/// A successful turn also confirms credentials refreshed outside this view, for example in the CLI.
pub(super) fn turn_succeeded(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let cleared = {
        let mut extras = proc.extras.lock().unwrap();
        if extras
            .auth
            .as_ref()
            .is_some_and(|s| matches!(s.status, "required" | "signedOut" | "logoutFailed"))
        {
            extras.auth = None;
            true
        } else {
            false
        }
    };
    if cleared {
        emit_extras(app, session_id, proc);
    }
}

pub(super) fn is_auth_error(error: &Value) -> bool {
    let info = &error["codexErrorInfo"];
    if info == "unauthorized" || info.get("unauthorized").is_some() {
        return true;
    }
    let message = error
        .as_str()
        .or_else(|| error["message"].as_str())
        .unwrap_or("")
        .to_lowercase();
    message.contains("refresh token")
        || message.contains("refresh_token")
        || message.contains("unauthorized")
        || message.contains("not logged in")
        || message.contains("authentication required")
}

pub(super) fn require(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, error: &Value) {
    if !is_auth_error(error) {
        return;
    }
    {
        let mut extras = proc.extras.lock().unwrap();
        if extras.auth.as_ref().is_some_and(AuthState::active) {
            return;
        }
        extras.auth = Some(AuthState::new("required"));
    }
    emit_extras(app, session_id, proc);
}

pub(super) fn start(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) -> Result<(), String> {
    if active(proc) {
        return Ok(());
    }
    if !proc.alive.load(Ordering::Relaxed) || !proc.codex_initialized.load(Ordering::Relaxed) {
        return Err("Codex is still starting or has stopped. Please try again.".into());
    }
    if proc.turn.lock().unwrap().running {
        return Err("Wait for the current turn to finish before signing in.".into());
    }
    let id = proc.request_id("auth_start");
    let mut state = AuthState::new("starting");
    state.attempt = id.clone();
    proc.extras.lock().unwrap().auth = Some(state);
    emit_extras(app, session_id, proc);
    crate::diagnostics::record(
        "INFO",
        if proc.kind == crate::models::SessionKind::Claude { "claude_account" } else { "codex_device_login" },
        json!({"method":"program","sessionId":session_id,"status":"started"}),
    );
    // A successful managed login replaces the saved credentials; cancellation leaves them untouched.
    if let Err(error) = proc.write(&codex_protocol::request(
        &id,
        "account/login/start",
        json!({"type":"chatgptDeviceCode"}),
    )) {
        finish(app, session_id, proc, "failed");
        return Err(error);
    }
    let (app, session_id, proc) = (app.clone(), session_id.to_string(), Arc::clone(proc));
    std::thread::spawn(move || {
        let started = now_ms();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let state = proc.extras.lock().unwrap().auth.clone();
            let Some(state) = state.filter(|s| s.attempt == id && s.active()) else {
                break;
            };
            let limit = if state.status == "starting" {
                30_000
            } else {
                15 * 60_000
            };
            if !proc.alive.load(Ordering::Relaxed) || now_ms().saturating_sub(started) >= limit {
                let _action = proc.action.lock().unwrap();
                if proc
                    .extras
                    .lock()
                    .unwrap()
                    .auth
                    .as_ref()
                    .is_some_and(|s| s.attempt == id && s.active())
                {
                    cancel_native(&proc, state.login_id.as_deref());
                    finish(&app, &session_id, &proc, "failed");
                    super::release_if_idle(&app, &session_id, &proc);
                }
                break;
            }
        }
    });
    Ok(())
}

fn finish(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, status: &'static str) {
    let duration = {
        let mut extras = proc.extras.lock().unwrap();
        let duration = extras
            .auth
            .as_ref()
            .map(|s| now_ms().saturating_sub(s.started_at))
            .unwrap_or(0);
        extras.auth = Some(AuthState::new(status));
        duration
    };
    crate::diagnostics::record(
        "INFO",
        if proc.kind == crate::models::SessionKind::Claude { "claude_account" } else { "codex_device_login" },
        json!({"method":"program","sessionId":session_id,"status":if status == "canceled" { "cancelled" } else { status },"durationMs":duration}),
    );
    emit_extras(app, session_id, proc);
}

fn cancel_native(proc: &ChatProcess, login_id: Option<&str>) {
    if let Some(login_id) = login_id {
        let id = proc.request_id("auth_cancel");
        let _ = proc.write(&codex_protocol::request(
            &id,
            "account/login/cancel",
            json!({"loginId":login_id}),
        ));
    }
}

pub(super) fn cancel(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
) -> Result<(), String> {
    let state = proc.extras.lock().unwrap().auth.clone();
    if let Some(state) = state.filter(|s| matches!(s.status, "starting" | "pending")) {
        if let Some(login_id) = &state.login_id {
            let result = proc.request_and_wait("auth_cancel_user", |id| {
                codex_protocol::request(id, "account/login/cancel", json!({"loginId":login_id}))
            })?;
            // The completion notification may win the race with cancellation. Preserve that outcome.
            if proc
                .extras
                .lock()
                .unwrap()
                .auth
                .as_ref()
                .is_some_and(|s| s.status == "success")
            {
                return Ok(());
            }
            if result["status"] != "canceled" {
                if !active(proc) {
                    return Ok(());
                }
                return Err("Codex could not confirm cancellation. Wait for the sign-in result or try again.".into());
            }
        }
        finish(app, session_id, proc, "canceled");
        super::release_if_idle(app, session_id, proc);
    }
    Ok(())
}

/// Clear the provider's saved account without clearing or replacing the conversation.
pub(super) fn logout(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
) -> Result<(), String> {
    if active(proc) {
        return Err("Finish or cancel the current account operation before signing out.".into());
    }
    if !proc.alive.load(Ordering::Relaxed) || !proc.codex_initialized.load(Ordering::Relaxed) {
        return Err("Codex is still starting or has stopped. Please try again.".into());
    }
    if proc.turn.lock().unwrap().running {
        return Err("Wait for the current turn to finish before signing out.".into());
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
        "codex_logout",
        json!({"method":"program","sessionId":session_id,"status":"started"}),
    );
    let result = proc.request_and_wait("auth_logout", |id| {
        codex_protocol::request(id, "account/logout", json!({}))
    });
    let status = if result.is_ok() {
        "signedOut"
    } else {
        "logoutFailed"
    };
    proc.extras.lock().unwrap().auth = Some(AuthState::new(status));
    crate::diagnostics::record(
        "INFO",
        "codex_logout",
        json!({"method":"program","sessionId":session_id,"status":if result.is_ok() { "success" } else { "failed" },"durationMs":now_ms().saturating_sub(started)}),
    );
    emit_extras(app, session_id, proc);
    super::release_if_idle(app, session_id, proc);
    result
        .map(|_| ())
        .map_err(|_| "codex_auth_logout_failed".into())
}

/// Handled before ordinary response errors: login failures belong to the authentication panel.
pub(super) fn response(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    kind: Option<&str>,
    request_id: &str,
    result: &Value,
    error: Option<&str>,
) -> bool {
    if kind == Some("auth_cancel") {
        return true;
    }
    if kind != Some("auth_start") {
        return false;
    }
    let mut extras = proc.extras.lock().unwrap();
    let Some(state) = extras
        .auth
        .as_mut()
        .filter(|s| s.status == "starting" && s.attempt == request_id)
    else {
        drop(extras);
        cancel_native(proc, result["loginId"].as_str());
        return true;
    };
    let url = result["verificationUrl"].as_str();
    let code = result["userCode"].as_str();
    let login_id = result["loginId"].as_str();
    // Device authorization has a fixed provider-owned destination, never a caller-supplied URL.
    if error.is_some()
        || result["type"] != "chatgptDeviceCode"
        || url != Some("https://auth.openai.com/codex/device")
        || !code.is_some_and(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        || !login_id.is_some_and(|s| !s.is_empty() && s.len() <= 128)
    {
        let duration = now_ms().saturating_sub(state.started_at);
        *state = AuthState::new("failed");
        drop(extras);
        crate::diagnostics::record(
            "INFO",
            "codex_device_login",
            json!({"method":"program","sessionId":session_id,"status":"failed","durationMs":duration}),
        );
        cancel_native(proc, login_id);
    } else {
        state.status = "pending";
        state.verification_url = url.map(str::to_string);
        state.user_code = code.map(str::to_string);
        state.login_id = login_id.map(str::to_string);
        drop(extras);
    }
    emit_extras(app, session_id, proc);
    if !active(proc) {
        super::release_if_idle(app, session_id, proc);
    }
    true
}

pub(super) fn notification(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    method: &str,
    params: &Value,
) -> bool {
    if method == "account/updated" {
        let Some(mode) = params.get("authMode") else {
            return true;
        };
        {
            let mut extras = proc.extras.lock().unwrap();
            if extras.auth.as_ref().is_some_and(AuthState::active) {
                return true;
            }
            if mode.is_null() {
                extras.auth = Some(AuthState::new("signedOut"));
            } else if extras
                .auth
                .as_ref()
                .is_some_and(|s| s.status == "signedOut")
            {
                extras.auth = Some(AuthState::new("success"));
            } else {
                return true;
            }
        }
        emit_extras(app, session_id, proc);
        return true;
    }
    if method != "account/login/completed" {
        return false;
    }
    let success = params["success"] == true;
    {
        let mut extras = proc.extras.lock().unwrap();
        let Some(state) = extras.auth.as_mut().filter(|s| {
            s.status == "pending" && s.login_id.as_deref() == params["loginId"].as_str()
        }) else {
            return true;
        };
        let duration = now_ms().saturating_sub(state.started_at);
        *state = AuthState::new(if success { "success" } else { "failed" });
        crate::diagnostics::record(
            "INFO",
            "codex_device_login",
            json!({"method":"program","sessionId":session_id,"status":state.status,"durationMs":duration}),
        );
    }
    emit_extras(app, session_id, proc);
    if success && !proc.ready.load(Ordering::Relaxed) {
        super::open_codex_thread(app, session_id, proc);
    }
    super::release_if_idle(app, session_id, proc);
    true
}
