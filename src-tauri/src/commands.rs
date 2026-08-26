//! Slim Tauri command entry points.
//!
//! Most desktop data commands converge on `desktop_call`: transport.ts routes frontend invoke calls
//! into the same `web::dispatch::dispatch` used by browser/remote clients and runs the entire call in
//! `spawn_blocking`, keeping the main thread responsive. Only commands requiring a direct native or
//! specialized channel remain here: hot-path PTY writes/resizes, PTY spawn/kill, binary replay and
//! image persistence, native windows/views, and host management. Other data logic lives in
//! `command_core`, `web/dispatch.rs`, or lower modules and is reached through `desktop_call`.

use std::io::Read;
use std::time::{Duration, UNIX_EPOCH};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::agent::server::HookServer;
use crate::db::{repo, Db};
use crate::files;
use crate::host::AppCtx;
use crate::models::SessionKind;
use crate::pty::manager::{PtyManager, SpawnResult};
use crate::web::{DeviceEntry, PairingInfo};

/// Take and clear an unconsumed `vela <path>` request. The frontend checks after installing its
/// listener; second-instance events use the same wake-only event plus queued payload to avoid races.
#[tauri::command]
pub fn take_open_project_request(pending: State<crate::PendingOpenProject>) -> Option<String> {
    pending.0.lock().unwrap().take()
}

/// Quit-confirmation handshake. The run-loop emits `app://quit-requested` instead of showing a native dialog;
/// the frontend acknowledges immediately, then reports the user's decision. These are pure in-memory flag
/// updates plus the exit call, so a synchronous command is correct here.
///
/// Acknowledge that the frontend dialog is on screen, cancelling the native-dialog watchdog.
#[tauri::command]
pub fn quit_prompt_ack(state: State<crate::QuitState>) {
    state
        .acked
        .store(true, std::sync::atomic::Ordering::SeqCst);
}

/// The user approved the exit. Any workspace snapshot has already been written by the frontend.
#[tauri::command]
pub fn confirm_quit(app: AppHandle, state: State<crate::QuitState>) {
    use std::sync::atomic::Ordering;
    state.pending.store(false, Ordering::SeqCst);
    state.confirmed.store(true, Ordering::SeqCst);
    app.exit(0);
}

/// The user dismissed the dialog. Clearing `pending` lets a later quit request prompt again.
#[tauri::command]
pub fn cancel_quit(state: State<crate::QuitState>) {
    use std::sync::atomic::Ordering;
    state.pending.store(false, Ordering::SeqCst);
    state.acked.store(false, Ordering::SeqCst);
}

/// VS Code-style shell command management. Native macOS settings/menu actions query, install, or
/// uninstall `vela` directly so browser/remote clients cannot modify the host PATH.
#[tauri::command]
pub fn vela_command_status() -> crate::agent::spawn_cli::UserCliStatus {
    crate::agent::spawn_cli::user_cli_status()
}

#[tauri::command]
pub fn install_vela_command() -> Result<crate::agent::spawn_cli::UserCliStatus, String> {
    crate::agent::spawn_cli::install_user_cli().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn uninstall_vela_command() -> Result<crate::agent::spawn_cli::UserCliStatus, String> {
    crate::agent::spawn_cli::uninstall_user_cli().map_err(|e| e.to_string())
}

// ─────────────────────────── Unified desktop dispatch ───────────────────────────

/// Unified desktop entry point for frontend `invoke(cmd, args)`. It shares browser/remote dispatch
/// and moves the full call to the blocking pool, keeping data commands off the UI thread. Hot-path
/// typing, native windows/views, and host management remain direct allowlisted commands; see
/// `DIRECT_DESKTOP_CMDS` in transport.ts.
///
/// Details: AppHandle can move into `spawn_blocking`; AppCtx then retrieves managed state. The source
/// is `DESKTOP_SOURCE` (`"desktop"`). Dispatch returns serialized JSON as `Result<Value, String>`,
/// preserving the browser shape and camelCase argument keys.
#[tauri::command]
pub async fn desktop_call(
    app: AppHandle,
    cmd: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::web::dispatch::dispatch(
            &AppCtx::Tauri(app),
            &cmd,
            &args,
            crate::pty::manager::DESKTOP_SOURCE,
            // The desktop shell is the machine owner's own UI: always a local, fully trusted origin.
            crate::web::dispatch::CallOrigin::Local,
        )
    })
    .await
    .map_err(|e| format!("desktop_call background task failed: {e}"))?
}

// ─────────────────────────── PTY ───────────────────────────

/// Start a PTY session and return its child PID plus any typed-session launch command.
///
/// If a remembered agent session ID still has a transcript, inject exact resume arguments; otherwise
/// fall back to a new session.
///
/// This async command moves SQLite queries, resume validation, openpty/fork, and spawn-slot polling
/// off the main thread. The frontend already awaits it, and per-session spawn serialization preserves
/// ordering. pty_write/resize/kill deliberately remain synchronous because IPC arrival order protects
/// keystroke ordering and prevents a delayed kill from terminating a replacement session.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub fn pty_spawn(
    app: AppHandle,
    state: State<PtyManager>,
    hooks: State<HookServer>,
    db: State<Db>,
    session_id: String,
    kind: SessionKind,
    shell: Option<String>,
    cwd: Option<String>,
    // Current UI brightness sets COLORFGBG for TUIs that do not query OSC 11.
    dark: Option<bool>,
    cols: u16,
    rows: u16,
    // Initial child-task prompt, supplied only on first launch as a positional agent argument.
    initial_prompt: Option<String>,
    // Inject the light/dark theme into Claude settings because ConPTY cannot detect the real background.
    theme: Option<String>,
    on_output: Channel<InvokeResponseBody>,
) -> Result<SpawnResult, String> {
    // Read the resume ID and pending-fork flag under a short lock, then verify the transcript exists.
    let (in_db, mut resume_id, fork, agent_args, perm, created_at) = {
        let conn = db.conn.lock().unwrap();
        let session = repo::get_session(&conn, &session_id)?;
        let in_db = session.is_some();
        let resume = session.as_ref().and_then(|s| s.agent_session_id.clone());
        let created_at = session.as_ref().map(|s| s.created_at).unwrap_or(0);
        let fork = repo::get_fork_pending(&conn, &session_id)?;
        // Append configured custom arguments unchanged on every launch and resume.
        let args = repo::get_agent_args(&conn, &session_id)?;
        // Map permission mode to agent-specific flags and prepend them to custom arguments.
        let perm = repo::get_permission_mode(&conn, &session_id)?;
        let args =
            crate::agent::inject::merge_permission_flag(kind, perm.as_deref(), args.as_deref());
        (in_db, resume, fork, args, perm, created_at)
    };
    // Reject stale sessions deleted by another client to avoid orphan shells. Ephemeral split-pane
    // sessions with the eph- prefix are intentionally absent from the database and remain allowed.
    if !in_db && !session_id.starts_with("eph-") {
        return Err("Session has been deleted".to_string());
    }
    resume_id = resume_id.filter(|id| !crate::agent::resume::confirmed_missing(kind, id));
    if kind == SessionKind::Pi && resume_id.is_none() && !fork && in_db {
        if let Some(cwd_for_repair) = cwd.as_deref() {
            let created = created_at.max(0) as u64;
            let since = UNIX_EPOCH + Duration::from_secs(created.saturating_sub(10));
            for agent_id in crate::agent::resume::capture_pi_session_candidates_oldest_first_since(
                Some(cwd_for_repair),
                since,
            ) {
                let changed = {
                    let conn = db.conn.lock().unwrap();
                    repo::claim_agent_session_id(&conn, &session_id, &agent_id, SessionKind::Pi)?
                };
                if changed {
                    let _ = app.emit(crate::host::TREE_CHANGED, ());
                    resume_id = Some(agent_id);
                    break;
                }
            }
        }
    }

    // Wrap the Tauri binary Channel as a transport-neutral OutputSink so desktop and WebSocket clients
    // share subscription fan-out/replay. A failed send returns false and removes the stale subscriber.
    let sink: crate::pty::session::OutputSink = Box::new(move |bytes: &[u8]| {
        on_output
            .send(InvokeResponseBody::Raw(bytes.to_vec()))
            .is_ok()
    });

    state.spawn(
        crate::host::AppCtx::Tauri(app),
        session_id,
        kind,
        shell,
        cwd,
        dark,
        cols,
        rows,
        hooks.endpoint(),
        resume_id,
        fork,
        initial_prompt,
        agent_args,
        perm,
        theme,
        crate::pty::manager::DESKTOP_SOURCE,
        false, // Desktop has no reconnect reattach mode; always perform normal spawn-or-attach.
        sink,
    )
}

/// Write keystroke data to a session. Input does not participate in size ownership.
#[tauri::command]
pub fn pty_write(state: State<PtyManager>, session_id: String, data: String) -> Result<(), String> {
    state.write(&session_id, &data)
}

/// Resize a PTY only for takeover, no owner, or the current owner; otherwise ignore it. Successful
/// changes broadcast `Resized`. `takeover` is an explicit “fit this window” action and defaults false.
#[tauri::command]
pub fn pty_resize(
    app: AppHandle,
    state: State<PtyManager>,
    session_id: String,
    cols: u16,
    rows: u16,
    takeover: Option<bool>,
) -> Result<(), String> {
    state.resize(
        &crate::host::AppCtx::Tauri(app),
        &session_id,
        cols,
        rows,
        crate::pty::manager::DESKTOP_SOURCE,
        takeover.unwrap_or(false),
    )
}

/// Terminate a session.
///
/// `reason` says whether a new process for this same session follows (`restart`) or the session is going
/// away (`close`); it travels out on `pty://killed/{id}` so other clients keep or close their pane. An
/// absent reason reads as `close`.
#[tauri::command]
pub fn pty_kill(
    state: State<PtyManager>,
    session_id: String,
    reason: Option<String>,
) -> Result<(), String> {
    state.kill(
        &session_id,
        crate::pty::manager::DESKTOP_SOURCE,
        crate::pty::session::KillReason::parse(reason.as_deref()),
    )
}

// ─────────────────────────── Replay, Git Bash, and tree management ───────────────────────────
/// Return a session's runtime working directory for split-pane inheritance.
#[tauri::command]
pub fn get_session_cwd(state: State<PtyManager>, session_id: String) -> Option<String> {
    state.cwd(&session_id)
}

/// List shells available for terminal sessions (cmd, PowerShell, Git Bash, WSL, etc. on Windows).
#[tauri::command]
pub async fn list_shells(app: AppHandle) -> Vec<crate::pty::manager::ShellOption> {
    // The data directory locates bundled Git Bash; failure merely omits that optional entry.
    let data_dir = app.path().app_data_dir().ok();
    // WSL detection launches `wsl.exe --list --quiet`, so run it in the blocking pool. Other systems
    // immediately return an empty list.
    tauri::async_runtime::spawn_blocking(move || {
        crate::pty::manager::available_shells(data_dir.as_deref())
    })
    .await
    .unwrap_or_default()
}

/// Return host-specific installation guidance for a missing agent, or None when unsupported.
#[tauri::command]
pub fn agent_install_recipe(agent: String) -> Option<crate::agent::install::InstallRecipe> {
    crate::agent::install::install_recipe(&agent)
}

/// Full Git Bash installation state used by the shell menu.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitbashStatus {
    /// Whether any bundled-minimal or downloaded-full Git Bash is available.
    pub available: bool,
    /// Whether the full edition with commands such as git/ssh is installed.
    pub full_installed: bool,
}

/// Query Git Bash state; non-Windows platforms always return false.
#[tauri::command]
pub fn gitbash_status(app: AppHandle) -> GitbashStatus {
    #[cfg(windows)]
    {
        let dd = app.path().app_data_dir().ok();
        let available = dd
            .as_deref()
            .map(|d| crate::agent::gitbash::default_bash(d).is_some())
            .unwrap_or(false);
        let full_installed = dd
            .as_deref()
            .map(crate::agent::gitbash::full_installed)
            .unwrap_or(false);
        GitbashStatus {
            available,
            full_installed,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        GitbashStatus {
            available: false,
            full_installed: false,
        }
    }
}

/// Download/install full Git Bash in the background and report progress through events:
/// `gitbash://download`（{phase:"download",received,total} / {phase:"extract"}）、
/// `gitbash://download-done` and `gitbash://download-error`. Only Windows downloads anything.
#[tauri::command]
pub fn download_full_gitbash(app: AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        use tauri::Emitter;
        let dd = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("data dir unavailable: {e}"))?;
        // Report completion immediately when already installed, preserving idempotence.
        if crate::agent::gitbash::full_installed(&dd) {
            let _ = app.emit("gitbash://download-done", ());
            return Ok(());
        }
        let app2 = app.clone();
        std::thread::spawn(move || {
            use crate::agent::gitbash::FullProgress;
            // Emit progress about every 512 KB rather than flooding IPC with 64 KB chunks.
            let last = std::cell::Cell::new(0u64);
            let cb = |p: FullProgress| match p {
                FullProgress::Downloading { received, total } => {
                    let step = received.saturating_sub(last.get()) >= 512 * 1024;
                    if step || (total != 0 && received >= total) {
                        last.set(received);
                        let _ = app2.emit(
                            "gitbash://download",
                            serde_json::json!({"phase":"download","received":received,"total":total}),
                        );
                    }
                }
                FullProgress::Extracting => {
                    let _ = app2.emit("gitbash://download", serde_json::json!({"phase":"extract"}));
                }
            };
            match crate::agent::gitbash::download_full(&dd, &cb) {
                Ok(()) => {
                    let _ = app2.emit("gitbash://download-done", ());
                }
                Err(e) => {
                    let _ = app2.emit("gitbash://download-error", e);
                }
            }
        });
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("downloading Git Bash is only supported on Windows".to_string())
    }
}

// ─────────────────────────── Recording replay (binary Channel) ───────────────────────────

/// Stream a terminal recording in chunks to a read-only xterm. Return an empty replay, not an error,
/// when the session has never produced a recording.
#[tauri::command]
pub fn read_recording(
    app: AppHandle,
    session_id: String,
    on_chunk: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get data directory: {e}"))?;
    let path = data_dir
        .join("recordings")
        .join(format!("{session_id}.log"));
    if !path.exists() {
        return Ok(());
    }
    let mut file =
        std::fs::File::open(&path).map_err(|e| format!("Failed to open recording: {e}"))?;
    let mut buf = [0u8; 65536];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                on_chunk
                    .send(InvokeResponseBody::Raw(buf[..n].to_vec()))
                    .map_err(|e| format!("Failed to send replay chunk: {e}"))?;
            }
            Err(e) => return Err(format!("Failed to read recording: {e}")),
        }
    }
    Ok(())
}

// ─────────────────────────── Image persistence (direct frontend number[] bytes) ────────────────

/// Save a pasted/dropped image to a temporary file and return its absolute path for an agent. Desktop
/// calls this directly; browser/remote invokes the same files implementation over WebSocket.
#[tauri::command]
pub fn save_pasted_image(bytes: Vec<u8>, ext: String) -> Result<String, String> {
    files::save_pasted_image(&bytes, &ext)
}

/// Persist a Markdown image under a sibling `assets/` directory and return its document-relative path,
/// unlike temporary absolute-path agent images.
#[tauri::command]
pub fn save_doc_image(doc_path: String, bytes: Vec<u8>, ext: String) -> Result<String, String> {
    files::save_doc_image(&doc_path, &bytes, &ext)
}

// ─────────────────────────── Browser remote-access host management ───────────────────────────

/// Generate a browser pairing link with shared token and server public key in the URL fragment.
/// `address` chooses the interface IP; rotate replaces the token, invalidates old links, and clears devices.
#[tauri::command]
pub fn web_pairing_create(
    app: AppHandle,
    address: Option<String>,
    rotate: Option<bool>,
) -> Result<PairingInfo, String> {
    crate::command_core::web_pairing_create(&AppCtx::Tauri(app), address, rotate.unwrap_or(false))
}

/// List paired devices that have actually connected for the management panel.
#[tauri::command]
pub fn web_devices_list(app: AppHandle) -> Vec<DeviceEntry> {
    crate::command_core::web_devices_list(&AppCtx::Tauri(app))
}

/// Remove a device registration display entry. Shared links can still reconnect; rotate to revoke all.
/// Errors when the revocation cannot be persisted (it would silently return after the next restart).
#[tauri::command]
pub fn web_device_revoke(app: AppHandle, device_id: String) -> Result<bool, String> {
    crate::command_core::web_device_revoke(&AppCtx::Tauri(app), &device_id)
}

// ─────────────────────────── Remote connection window ───────────────────────────

/// Open a window connected to a remote vlx-term server, with initialization for auto-login/identity.
///
/// Do **not** load remote HTTPS directly: wry 0.55 WKWebView cannot handle the self-signed certificate
/// challenge and renders blank. Use the local TLS-terminating tunnel and load its loopback HTTP URL,
/// forwarding HTTP/WebSocket bytes to the remote endpoint.
///
/// This **must remain async**. A synchronous Tauri command blocks Windows' main event loop while
/// WebView2 controller creation itself needs that loop, deadlocking the entire app with a blank window.
/// Async execution leaves the loop free to complete `build()`. Do not convert it back to synchronous.
#[tauri::command]
pub async fn open_remote_window(
    app: AppHandle,
    pairing_url: String,
    password: Option<String>,
) -> Result<(), String> {
    // Parse host/port from the pairing URL for the tunnel and preserve #pair=... so wsClient performs
    // the same end-to-end encrypted handshake as a browser.
    let remote: url::Url = pairing_url
        .trim()
        .parse()
        .map_err(|e| format!("Invalid pairing link: {e}"))?;
    let host = remote
        .host_str()
        .ok_or("Pairing link is missing a hostname")?
        .to_string();
    let port = remote
        .port_or_known_default()
        .ok_or("Pairing link is missing a port")?;
    let fragment = remote.fragment().unwrap_or("");
    if !fragment.starts_with("pair=") {
        return Err("Pairing link is missing the #pair=… part".to_string());
    }
    let display_addr = format!("{host}:{port}");

    // Reuse a local tunnel for the remote endpoint and load its loopback URL with the pairing fragment.
    let local_port = crate::web::tunnel::ensure(host, port)?;
    let parsed: url::Url = format!("http://127.0.0.1:{local_port}/#{fragment}")
        .parse()
        .map_err(|e| format!("Failed to build tunnel address: {e}"))?;

    let label = format!("remote-{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let addr_js = serde_json::to_string(&display_addr).unwrap_or_else(|_| "\"\"".into());

    // Optionally inject the panel password as __VLX_AUTOLOGIN__ for LoginGate/E2EE. If absent, show
    // the normal password page. It appears only in this window's initialization script, never argv/logs.
    let autologin_js = match password.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(pw) => format!(
            "window.__VLX_AUTOLOGIN__={{password:{}}};",
            serde_json::to_string(pw).unwrap_or_else(|_| "\"\"".into())
        ),
        None => String::new(),
    };

    // Preserve __TAURI_INTERNALS__; __VLX_FORCE_BROWSER__ alone selects WebSocket transport, while
    // internals are needed to write OSC 52 selection copies to the local clipboard through the plugin.
    let init_script = format!(
        r#"(function(){{
  window.__VLX_FORCE_BROWSER__=true;
  if(typeof window.OffscreenCanvas!=='undefined')window.OffscreenCanvas=undefined;
  window.__VLX_REMOTE__={{address:{addr_js}}};
  {autologin_js}
}})();"#
    );

    tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::External(parsed))
        .title(format!("VelaTerm · Remote: {display_addr}"))
        .inner_size(1280.0, 820.0)
        .min_inner_size(900.0, 600.0)
        // Open with the chrome the user already chose instead of the OS default; see `set_native_theme`.
        .theme(crate::native_theme(&app))
        .initialization_script(&init_script)
        // Disable native drag/drop interception so macOS WKWebView delivers HTML5 image drops.
        .disable_drag_drop_handler()
        .build()
        .map_err(|e| format!("Failed to create remote window: {e}"))?;

    // Runtime-created loopback remote contexts are not covered by configured capabilities. Grant a
    // minimal capability scoped to this window/URL: clipboard writes for OSC 52; native notifications;
    // event listen/unlisten for reliable Tauri focus and notification clicks; event emit to relay
    // remote notification requests to the local backend; set-focus after notification activation;
    // and both opener command/default-URL permissions so links open in the operator's local browser.
    // Both opener permissions are required because command access and URL scope are separate checks.
    use tauri::ipc::CapabilityBuilder;
    app.add_capability(
        CapabilityBuilder::new(format!("remote-caps-{label}"))
            .window(label.clone())
            .remote("http://127.0.0.1:*".to_string())
            .permission("clipboard-manager:allow-write-text")
            .permission("notification:default")
            .permission("opener:allow-open-url")
            .permission("opener:allow-default-urls")
            .permission("core:event:allow-listen")
            .permission("core:event:allow-unlisten")
            .permission("core:event:allow-emit")
            .permission("core:window:allow-set-focus"),
    )
    .map_err(|e| format!("Failed to grant remote window capability: {e}"))?;

    Ok(())
}

/// Parse `(host, port)` from a pairing link; fingerprint trust is keyed by endpoint, not rotating token.
fn pairing_host_port(pairing_url: &str) -> Result<(String, u16), String> {
    let remote: url::Url = pairing_url
        .trim()
        .parse()
        .map_err(|e| format!("Invalid pairing link: {e}"))?;
    let host = remote
        .host_str()
        .ok_or("Pairing link is missing a hostname")?
        .to_string();
    let port = remote
        .port_or_known_default()
        .ok_or("Pairing link is missing a port")?;
    Ok((host, port))
}

/// Remote URL fingerprint probe result, shaped like SSH `HostKeyProbe` for frontend review.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlKeyProbe {
    /// Comparison state: known connects silently, new prompts once, and changed warns.
    pub status: crate::ssh_remote::HostKeyStatus,
    /// Uppercase colon-separated SHA-256 certificate fingerprint for user verification.
    pub fingerprint: String,
}

/// Probe the remote TLS fingerprint and compare it with host:port trust state, returning
/// known/new/changed. The pairing-link public key remains the primary MITM trust anchor; this is an
/// additional change warning that avoids repeated prompts.
///
/// TLS handshake is blocking network I/O up to eight seconds, so run it entirely in spawn_blocking.
/// The in-memory SQLite comparison is a quick read outside that task.
#[tauri::command]
pub async fn probe_remote_fingerprint(
    db: State<'_, Db>,
    pairing_url: String,
) -> Result<UrlKeyProbe, String> {
    let (host, port) = pairing_host_port(&pairing_url)?;
    let host_bg = host.clone();
    let fingerprint = tauri::async_runtime::spawn_blocking(move || {
        crate::web::tunnel::probe_fingerprint(&host_bg, port)
    })
    .await
    .map_err(|e| format!("fingerprint probe task failed: {e}"))??;
    let host_port = format!("{host}:{port}");
    let status = {
        let conn = db.conn.lock().unwrap();
        match repo::get_url_host_key(&conn, &host_port)? {
            None => crate::ssh_remote::HostKeyStatus::New,
            Some(fp) if fp == fingerprint => crate::ssh_remote::HostKeyStatus::Known,
            Some(_) => crate::ssh_remote::HostKeyStatus::Changed,
        }
    };
    Ok(UrlKeyProbe {
        status,
        fingerprint,
    })
}

/// After user confirmation, store this host:port fingerprint as TOFU trust. Unchanged fingerprints
/// become known; only changes warn. This is an in-memory SQLite write with no network I/O.
#[tauri::command]
pub fn url_trust_fingerprint(
    db: State<'_, Db>,
    pairing_url: String,
    fingerprint: String,
) -> Result<(), String> {
    let (host, port) = pairing_host_port(&pairing_url)?;
    let host_port = format!("{host}:{port}");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let conn = db.conn.lock().unwrap();
    repo::upsert_url_host_key(&conn, &host_port, &fingerprint, now)
}

// ─────────────────────────── Developer tools ───────────────────────────

/// Open DevTools for the current window from the development-only frontend action.
///
/// Tauri compiles open_devtools only for debug/devtools builds, so cfg guards it and release builds
/// perform no action. The frontend likewise shows the button only under import.meta.env.DEV.
#[tauri::command]
pub fn open_devtools(window: tauri::WebviewWindow) {
    #[cfg(debug_assertions)]
    {
        window.open_devtools();
    }
    // open_devtools does not exist in release builds; intentionally consume the parameter.
    #[cfg(not(debug_assertions))]
    {
        let _ = window;
    }
}

/// Match native window chrome to the frontend's light/dark mode. Windows only.
///
/// The app keeps the system title bar, and Windows paints it light until `set_theme` turns on DWM's
/// immersive dark mode, leaving a white strip above a dark UI (issue #26 section 5). `mode` mirrors the
/// frontend's `ThemeMode`: `dark`/`light` pin the chrome, anything else (`system`) restores follow-the-OS
/// so a later OS change still reaches the title bar.
///
/// Deliberately a no-op elsewhere. On macOS and Linux the window theme is an app-wide appearance override,
/// so applying it would also repaint native menus and dialogs and flip the WebView's `prefers-color-scheme`
/// — a behaviour change nobody asked for on platforms whose chrome already tracks the app correctly.
///
/// Remembering the value matters as much as applying it: windows opened later (remote/SSH) read it at build
/// time so they never flash the wrong chrome. `set_theme` only posts a message to the event loop and touches
/// no I/O, so a synchronous command is correct here.
#[tauri::command]
pub fn set_native_theme(app: AppHandle, mode: String) {
    #[cfg(windows)]
    {
        let theme = match mode.as_str() {
            "dark" => Some(tauri::Theme::Dark),
            "light" => Some(tauri::Theme::Light),
            _ => None,
        };
        if let Some(state) = app.try_state::<crate::NativeTheme>() {
            *state.0.lock().unwrap() = theme;
        }
        for (_, win) in app.webview_windows() {
            let _ = win.set_theme(theme);
        }
    }
    #[cfg(not(windows))]
    let _ = (app, mode);
}

// ─────────────────────────── SSH remote connections ───────────────────────────

// SSH commands are async plus spawn_blocking because network/SSH/scp I/O must never occupy the UI thread.

/// Probe a remote host key and compare known_hosts so the frontend can confirm new/changed hosts.
#[tauri::command]
pub async fn ssh_probe_host(host: String) -> Result<crate::ssh_remote::HostKeyProbe, String> {
    tauri::async_runtime::spawn_blocking(move || crate::ssh_remote::probe_host_key(&host))
        .await
        .map_err(|e| format!("fingerprint probe task failed: {e}"))?
}

/// Store a user-confirmed fingerprint in known_hosts, replacing the old entry when changed.
#[tauri::command]
pub async fn ssh_trust_host(host: String, was_changed: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::ssh_remote::trust_host(&host, was_changed))
        .await
        .map_err(|e| format!("trust-write task failed: {e}"))?
}

/// Perform the complete connection flow: detect the system, provision locally, probe/copy remotely,
/// start serve plus `-L` forwarding, and open an auto-login window. Return the disconnect session ID.
///
/// Provisioning may block for tens of seconds, so run it in spawn_blocking. Only the quick window
/// creation is scheduled on the main thread.
///
/// When remember=true and password auth succeeds, store the password in the system keyring; never
/// fall back to plaintext. If key auth fails, retry once with a remembered password before returning
/// AUTH_REQUIRED. Every successful connection upserts the host history.
#[tauri::command]
pub async fn ssh_connect(
    app: AppHandle,
    db: State<'_, Db>,
    host: String,
    password: Option<String>,
    remember: Option<bool>,
    shared_db: Option<bool>,
    mirror: Option<bool>,
) -> Result<String, String> {
    // Data mode defaults to an isolated database; true reuses the remote desktop release database.
    let shared_db = shared_db.unwrap_or(false);
    // Mirror mode defaults off for SSH: the switch is hidden behind Option/Alt and starts unchecked.
    let mirror = mirror.unwrap_or(false);
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to get app data dir: {e}"))?;
    // Keyring service identifier matches the data-directory suffix and preserves dev/release isolation.
    let identifier = data_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "io.vlinx.vlxterm".to_string());
    let remember = remember.unwrap_or(false);
    // Clone the keyring identifier for tunnel monitoring before moving the original into the closure.
    let identifier_watch = identifier.clone();
    let host_bg = host.clone();
    let app_ev = app.clone();
    let r = tauri::async_runtime::spawn_blocking(move || {
        // Emit stage code and percentage through ssh://progress for localized frontend display.
        let progress = move |stage: &str, pct: Option<u8>| {
            let _ = app_ev.emit(
                "ssh://progress",
                serde_json::json!({ "stage": stage, "percent": pct }),
            );
        };
        // Use agent/key auth when no password is supplied; otherwise use the user's password.
        let mut used: Option<String> = password.clone();
        let auth = match password.as_deref() {
            Some(pw) => crate::ssh_remote::SshAuth::Password(pw.to_string()),
            None => crate::ssh_remote::SshAuth::Auto,
        };
        let mut res =
            crate::ssh_remote::connect(&data_dir, &host_bg, auth, shared_db, mirror, &progress);
        // If key auth fails without a supplied password, retry once from the keyring. Otherwise
        // preserve AUTH_REQUIRED so the frontend can prompt manually.
        if let Err(e) = &res {
            if password.is_none() && e.starts_with(crate::ssh_remote::AUTH_REQUIRED_TAG) {
                if let Some(pw) = crate::ssh_remote::load_password(&identifier, "ssh", &host_bg) {
                    // Adopt the remembered password only after success. On failure, keep AUTH_REQUIRED
                    // so a newly entered and remembered password can replace the stale one.
                    let retry = crate::ssh_remote::connect(
                        &data_dir,
                        &host_bg,
                        crate::ssh_remote::SshAuth::Password(pw.clone()),
                        shared_db,
                        mirror,
                        &progress,
                    );
                    if let Ok(ok) = retry {
                        used = Some(pw);
                        res = Ok(ok);
                    }
                }
            }
        }
        let result = res?;
        // Store only a password actually used with remember enabled. Keyring failure has no plaintext fallback.
        if remember {
            if let Some(pw) = &used {
                crate::ssh_remote::store_password(&identifier, "ssh", &host_bg, pw);
            }
        }
        Ok::<_, String>(result)
    })
    .await
    .map_err(|e| format!("connect task failed: {e}"))??;

    // Record the successful host in most-recent-first connection history.
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let conn = db.conn.lock().unwrap();
        let _ = repo::upsert_ssh_host(&conn, &host, now, shared_db, mirror);
    }

    let session = r.session.clone();
    // Monitor the `ssh -N -L` child and rebuild on its original port after network/sleep/VPN loss so
    // WebSocket reconnection recovers transparently. After repeated failure, emit down for the banner
    // and wait for a vlx://ssh-reconnect user kick.
    {
        let app_tun = app.clone();
        let session_tun = session.clone();
        crate::ssh_remote::watch_tunnel(
            host.clone(),
            session.clone(),
            identifier_watch,
            r.local_port,
            move |state| {
                let _ = app_tun.emit(
                    "ssh://tunnel-state",
                    serde_json::json!({ "session": session_tun, "state": state }),
                );
            },
        );
    }
    let app2 = app.clone();
    app.run_on_main_thread(move || {
        if let Err(e) = open_login_window(&app2, &host, &r.session, r.local_port, &r.password) {
            eprintln!("open ssh login window failed: {e}");
        }
    })
    .map_err(|e| format!("failed to schedule window open on main thread: {e}"))?;
    Ok(session)
}

/// Disconnect SSH. With kill_remote, stop the persistent remote service and all its sessions before
/// closing forwarding/control; otherwise leave it running for reuse.
#[tauri::command]
pub async fn ssh_disconnect(host: String, session: String, kill_remote: bool) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        crate::ssh_remote::disconnect(&host, &session, kill_remote);
    })
    .await;
}

/// Open a window on the local HTTP forwarding port and inject its random password for LoginGate.
/// Unlike URL remote windows, SSH already encrypts transport, so no TLS-stripping tunnel or E2EE
/// pairing fragment is needed.
fn open_login_window(
    app: &AppHandle,
    host: &str,
    session: &str,
    local_port: u16,
    password: &str,
) -> Result<(), String> {
    let parsed: url::Url = format!("http://127.0.0.1:{local_port}/")
        .parse()
        .map_err(|e| format!("failed to build local forward address: {e}"))?;
    let label = format!("ssh-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let addr_js = serde_json::to_string(host).unwrap_or_else(|_| "\"\"".into());
    let pw_js = serde_json::to_string(password).unwrap_or_else(|_| "\"\"".into());
    let session_js = serde_json::to_string(session).unwrap_or_else(|_| "\"\"".into());
    // Force WebSocket mode, provide auto-login, and identify the SSH session so its banner filters
    // tunnel events and emits vlx://ssh-reconnect for the correct tunnel.
    let init_script = format!(
        r#"(function(){{
  window.__VLX_FORCE_BROWSER__=true;
  if(typeof window.OffscreenCanvas!=='undefined')window.OffscreenCanvas=undefined;
  window.__VLX_REMOTE__={{address:{addr_js},session:{session_js}}};
  window.__VLX_AUTOLOGIN__={{password:{pw_js}}};
}})();"#
    );

    let win = tauri::WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::External(parsed))
        .title(format!("VelaTerm · SSH: {host}"))
        .inner_size(1280.0, 820.0)
        .min_inner_size(900.0, 600.0)
        // Open with the chrome the user already chose instead of the OS default; see `set_native_theme`.
        .theme(crate::native_theme(app))
        .initialization_script(&init_script)
        .disable_drag_drop_handler()
        .build()
        .map_err(|e| format!("failed to create SSH remote window: {e}"))?;

    // On SSH-window close, ask whether to stop or preserve the detached remote service for run.json reuse.
    let host_owned = host.to_string();
    let session_owned = session.to_string();
    let app_for_close = app.clone();
    let label_for_close = label.clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            // Prevent closing until the user's choice has been handled.
            api.prevent_close();
            let host = host_owned.clone();
            let session = session_owned.clone();
            let app2 = app_for_close.clone();
            let label2 = label_for_close.clone();
            use tauri_plugin_dialog::{
                DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
            };
            // Native custom button labels also identify the callback result; keep dialog copy in English.
            const BTN_STOP: &str = "Stop server";
            const BTN_KEEP: &str = "Keep running";
            const BTN_CANCEL: &str = "Cancel";
            app_for_close
                .dialog()
                .message(
                    "\"Stop server\" shuts down vela-server on the remote (ending all its sessions).\n\
                     \"Keep running\" leaves it running so the next connection reuses it — sessions are preserved.\n\
                     \"Cancel\" keeps this window open.",
                )
                .title("Disconnect remote")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::YesNoCancelCustom(
                    BTN_STOP.into(),
                    BTN_KEEP.into(),
                    BTN_CANCEL.into(),
                ))
                .show_with_result(move |result| {
                    // Cancel, Escape, or closing the dialog all cancel window closure and preserve the service.
                    let close_service = match result {
                        MessageDialogResult::Custom(s) if s == BTN_STOP => true,
                        MessageDialogResult::Custom(s) if s == BTN_KEEP => false,
                        _ => return,
                    };
                    // Perform blocking SSH disconnect/shutdown in the background, then destroy the window.
                    std::thread::spawn(move || {
                        crate::ssh_remote::disconnect(&host, &session, close_service);
                        if let Some(w) = app2.get_webview_window(&label2) {
                            let _ = w.destroy();
                        }
                    });
                });
        }
    });

    // Grant the same minimal runtime capability as open_remote_window, scoped to this loopback window.
    use tauri::ipc::CapabilityBuilder;
    app.add_capability(
        CapabilityBuilder::new(format!("ssh-caps-{label}"))
            .window(label.clone())
            .remote("http://127.0.0.1:*".to_string())
            .permission("clipboard-manager:allow-write-text")
            .permission("notification:default")
            .permission("core:event:allow-listen")
            .permission("core:event:allow-unlisten")
            .permission("core:event:allow-emit"),
    )
    .map_err(|e| format!("failed to grant SSH window capability: {e}"))?;
    Ok(())
}
