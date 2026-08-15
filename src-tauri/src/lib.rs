// The minimal --no-default-features server excludes desktop paths, leaving desktop-only helpers unused.
// Suppress dead_code noise only for that build; GUI builds retain normal warnings.
#![cfg_attr(not(feature = "gui"), allow(dead_code))]

mod agent;
// Undoes the AppImage launcher's environment rewrite for child processes on Linux.
pub mod appimage;
// Built-in browser tabs require desktop Tauri child windows and are absent from the minimal server.
#[cfg(feature = "gui")]
mod browser;
mod command_core;
// Tauri command registry is compiled only for GUI builds.
#[cfg(feature = "gui")]
mod commands;
mod db;
mod files;
mod git;
mod gitea;
mod host;
mod models;
mod procstat;
mod pty;
mod search;
// GUI-only vela-server provisioning: R2 download, minisign verification, and cache.
#[cfg(feature = "gui")]
mod server_supply;
// GUI-only client-side SSH orchestration, provisioning, serve, forwarding, and auto-login.
#[cfg(feature = "gui")]
mod ssh_remote;
// GUI-only pure-Rust russh transport, currently used on Windows.
#[cfg(feature = "gui")]
mod ssh_russh;
mod web;
// GUI-only macOS native notifications with session-aware click navigation.
#[cfg(all(feature = "gui", target_os = "macos"))]
mod notify_native;
// GUI-only Windows WinRT toasts with foreground session-aware click navigation.
#[cfg(all(feature = "gui", target_os = "windows"))]
mod notify_win;

use agent::server::HookServer;
use db::Db;
use host::{AppCtx, HeadlessHost};
// lib.rs names PtyManager only in GUI run(); HeadlessHost constructs its own manager.
#[cfg(feature = "gui")]
use pty::manager::PtyManager;
#[cfg(feature = "gui")]
use tauri::{Listener, Manager};
use web::WebServer;

use std::path::{Path, PathBuf};

/// Pending `vela <path>` request shared by first/second instances and taken after frontend listeners exist.
#[cfg(feature = "gui")]
pub(crate) struct PendingOpenProject(pub std::sync::Mutex<Option<String>>);

/// Quit-confirmation handshake state shared by the run-loop and the `confirm_quit`/`cancel_quit` commands.
///
/// The frontend owns the confirmation dialog so it can offer the "save workspace" checkbox and localized copy,
/// neither of which a native message dialog supports. `pending` guards against duplicate prompts, and
/// `confirmed` lets the run-loop distinguish an approved exit from a fresh user request.
#[cfg(feature = "gui")]
#[derive(Default)]
pub(crate) struct QuitState {
    pub confirmed: std::sync::atomic::AtomicBool,
    pub pending: std::sync::atomic::AtomicBool,
    /// Set once the frontend reports that it displayed the dialog. A frozen or crashed webview never acknowledges,
    /// so the watchdog falls back to the native dialog and the application stays quittable.
    pub acked: std::sync::atomic::AtomicBool,
}

/// Brief `vela` CLI help invoked through a hidden shim argument so normal GUI startup stays quiet.
pub fn print_vela_help() {
    println!("usage: vela <project-path>\n\nOpen a project in VelaTerm, or switch to it if it is already open.");
}

/// Parse `--open-project <path>` or a development-friendly positional project path while ignoring
/// unrelated GUI/platform arguments to preserve prior startup behavior.
pub fn open_project_from_args(args: &[String], cwd: &Path) -> Result<Option<PathBuf>, String> {
    let raw = match args.get(1).map(String::as_str) {
        Some("--open-project") => {
            if args.len() != 3 {
                return Err(
                    "expected exactly one project path\nusage: vela <project-path>".to_string(),
                );
            }
            &args[2]
        }
        Some(first) if !first.starts_with('-') && args.len() == 2 => &args[1],
        _ => return Ok(None),
    };
    let input = PathBuf::from(raw);
    let joined = if input.is_absolute() {
        input
    } else {
        cwd.join(input)
    };
    if !joined.is_dir() {
        return Err(format!("directory does not exist: {}", joined.display()));
    }
    joined
        .canonicalize()
        .map(Some)
        .map_err(|e| format!("cannot resolve project path {}: {e}", joined.display()))
}

#[cfg(all(feature = "gui", not(debug_assertions)))]
fn queue_open_project(app: &tauri::AppHandle, args: &[String], cwd: &str) {
    use tauri::Emitter;
    let Ok(Some(path)) = open_project_from_args(args, Path::new(cwd)) else {
        return;
    };
    if let Some(pending) = app.try_state::<PendingOpenProject>() {
        *pending.0.lock().unwrap() = Some(path.to_string_lossy().into_owned());
    }
    let _ = app.emit("open-project://request", ());
}

/// Request from a remote-context window for the local backend to relay a native notification.
#[cfg(feature = "gui")]
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // window_label/session_id are used only by the macOS native channel.
struct RemoteNotify {
    window_label: String,
    session_id: String,
    title: String,
    body: String,
}

/// Hidden Codex notify entry point that forwards event JSON to the local hook service and exits.
/// `--notify` accepts an explicit URL; `--notify-env` reads VLX_CODEX_NOTIFY_URL so static config can
/// use a runtime port/token.
pub fn run_notify(args: &[String]) {
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let (url, body) = match mode {
        "--notify" => (
            args.get(2).cloned().unwrap_or_default(),
            args.get(3).cloned().unwrap_or_default(),
        ),
        "--notify-env" => (
            std::env::var(agent::inject::CODEX_NOTIFY_URL_ENV).unwrap_or_default(),
            args.get(2).cloned().unwrap_or_default(),
        ),
        _ => return,
    };
    if !url.is_empty() {
        agent::server::forward_notify(&url, &body);
    }
}

/// Hidden Codex lifecycle-hook entry that forwards stdin JSON to this session's local hook service:
/// `vlx-term --codex-hook <ready|working|tool|asking|waiting>`.
///
/// `working` marks the start of a turn (UserPromptSubmit) and always applies, while `tool` marks
/// mid-turn activity (PreToolUse) that the hook service may drop when it arrives after the turn ended.
///
/// Build the URL from PTY-injected environment variables rather than hook configuration, avoiding
/// per-session definitions/token leakage. Missing variables or invalid states are silent no-ops;
/// forwarding never emits output or affects Codex.
pub fn run_codex_hook(args: &[String]) {
    use std::io::Read;

    let Some(state) = args.get(2).map(String::as_str) else {
        return;
    };
    if !matches!(state, "ready" | "working" | "tool" | "asking" | "waiting") {
        return;
    }
    let Ok(base) = std::env::var("VLX_SPAWN_URL") else {
        return;
    };
    let Ok(sid) = std::env::var("VLX_SESSION_ID") else {
        return;
    };
    let Ok(token) = std::env::var("VLX_TOKEN") else {
        return;
    };
    if base.is_empty() || sid.is_empty() || token.is_empty() {
        return;
    }
    let mut body = String::new();
    let _ = std::io::stdin().read_to_string(&mut body);
    let url = format!(
        "{}/hook/{}?t={}&e=codex_{}",
        base.trim_end_matches('/'),
        sid,
        token,
        state
    );
    agent::server::forward_notify(&url, &body);
}

/// Hidden Grok lifecycle-hook entry. Static `~/.grok/hooks/vlx-term.json` commands call this
/// executable, while all dynamic callback values remain scoped to the managed Grok process.
pub fn run_grok_hook(args: &[String]) {
    use std::io::Read;

    let Some(state) = args.get(2).map(String::as_str) else {
        return;
    };
    if !matches!(state, "boot" | "working" | "asking" | "waiting" | "idle") {
        return;
    }
    let Ok(base) = std::env::var("VLX_SPAWN_URL") else {
        return;
    };
    let Ok(sid) = std::env::var("VLX_SESSION_ID") else {
        return;
    };
    let Ok(token) = std::env::var("VLX_TOKEN") else {
        return;
    };
    if base.is_empty() || sid.is_empty() || token.is_empty() {
        return;
    }
    let mut body = String::new();
    let _ = std::io::stdin().read_to_string(&mut body);
    let url = format!(
        "{}/hook/{}?t={}&e={}",
        base.trim_end_matches('/'),
        sid,
        token,
        state
    );
    agent::server::forward_notify(&url, &body);
}

/// Hidden `--spawn` entry used by the PATH `vspawn` shim. POST injected session data to `/spawn`,
/// return its exit code, and never launch the GUI.
pub fn run_spawn(args: &[String]) -> ! {
    agent::cli_client::run_spawn(args)
}

/// Hidden `--view <file|URL>` entry used by `vopen`; POST `/view` to open a center-pane tab, then exit.
pub fn run_view(args: &[String]) -> ! {
    agent::cli_client::run_view(args)
}

/// Hidden `--agent-ctl` entry used by the PATH `vagent` shim. POST one agent-control operation to
/// `/agent/<op>`, print the JSON response, and never launch the GUI.
pub fn run_agent_ctl(args: &[String]) -> ! {
    agent::ctl_client::run_agent_ctl(args)
}

/// Build-time Git commit, or unknown, used by --version and SSH remote version pinning.
pub const GIT_COMMIT: &str = match option_env!("VLX_GIT_COMMIT") {
    Some(c) => c,
    None => "unknown",
};

/// Binary semver from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Print stable version/commit output for --version/-V and exit without windows or services.
///
/// SSH provisioning parses line one as `velaterm <semver>` and line two as `commit: <hash>` to decide
/// binary reuse. Keep this format stable unless the remote parser changes with it.
pub fn print_version() {
    println!("velaterm {VERSION}");
    println!("commit: {GIT_COMMIT}");
}

/// Send a session-aware native notification and return whether the platform handled it. macOS uses
/// UNUserNotificationCenter, Windows uses WinRT toast, and other/unsupported builds return false for
/// frontend plugin fallback.
#[cfg(feature = "gui")]
#[tauri::command]
fn native_notify(
    window: tauri::WebviewWindow,
    session_id: String,
    title: String,
    body: String,
    sound: bool,
) -> bool {
    #[cfg(target_os = "macos")]
    {
        let _ = sound; // macOS uses the system default notification sound.
        notify_native::send(window.label(), &session_id, &title, &body)
    }
    #[cfg(target_os = "windows")]
    {
        notify_win::send(
            window.app_handle().clone(),
            window.label().to_string(),
            session_id,
            title,
            body,
            sound,
        )
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (window, session_id, title, body, sound);
        false
    }
}

/// Return actual macOS notification authorization status, or unsupported outside a bundled macOS app.
#[cfg(feature = "gui")]
#[tauri::command]
fn native_notify_auth_status() -> String {
    #[cfg(target_os = "macos")]
    {
        notify_native::authorization_status()
    }
    #[cfg(not(target_os = "macos"))]
    {
        "unsupported".to_string()
    }
}

/// Request macOS notification permission and return authorization; non-macOS returns false.
#[cfg(feature = "gui")]
#[tauri::command]
fn native_notify_request_auth() -> bool {
    #[cfg(target_os = "macos")]
    {
        notify_native::request_authorization()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Disable macOS smart quotes/dashes for this app before any WebView exists. Otherwise `--model`
/// can become an em-dash option that the CLI cannot recognize. App-level defaults do not affect the system.
#[cfg(all(feature = "gui", target_os = "macos"))]
fn disable_smart_substitutions() {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::NSString;
    unsafe {
        let defaults: *mut AnyObject = msg_send![class!(NSUserDefaults), standardUserDefaults];
        for key in [
            "NSAutomaticDashSubstitutionEnabled",
            "NSAutomaticQuoteSubstitutionEnabled",
        ] {
            let ns_key = NSString::from_str(key);
            let _: () = msg_send![defaults, setBool: false, forKey: &*ns_key];
        }
    }
}

/// Register the Windows AppUserModelId under HKCU so release builds can display toasts.
///
/// Windows requires registered AppUserModelID identity, normally supplied by an installer shortcut.
/// The no-bundle release copies only a bare executable, so its unregistered toasts were silently dropped;
/// development worked only because an older installer left a registered shortcut.
///
/// A DisplayName under the per-identifier AppUserModelId registry key provides minimal identity without
/// an installer. Registration is idempotent and failure is nonfatal, affecting notifications only.
#[cfg(all(feature = "gui", windows))]
fn register_aumid_for_notifications(identifier: &str, display_name: &str) {
    let path = format!(r"Software\Classes\AppUserModelId\{identifier}");
    let result = windows_registry::CURRENT_USER
        .create(&path)
        .and_then(|key| key.set_string("DisplayName", display_name));
    if let Err(e) = result {
        eprintln!(
            "failed to register AppUserModelId (Windows toast notifications may not show): {e}"
        );
    }
}

/// app_settings/localStorage key for cross-shell automatic pasted-image cleanup, enabled by default.
const CLEAN_PASTED_IMAGES_KEY: &str = "vlx-clean-images";

/// Read pasted-image cleanup: only explicit `0` disables it; missing/unreadable values default enabled.
fn pasted_image_cleanup_enabled(db: &Db) -> bool {
    let Ok(conn) = db.conn.lock() else {
        return true;
    };
    match db::repo::get_app_settings(&conn) {
        Ok(map) => map.get(CLEAN_PASTED_IMAGES_KEY).map(String::as_str) != Some("0"),
        Err(_) => true,
    }
}

/// Desktop GUI entry that builds the Tauri app; absent from the minimal server.
#[cfg(feature = "gui")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(initial_open_project: Option<PathBuf>) {
    // Disable macOS smart punctuation before creating any WebView.
    #[cfg(target_os = "macos")]
    disable_smart_substitutions();

    #[cfg(debug_assertions)]
    let builder = tauri::Builder::default();
    #[cfg(not(debug_assertions))]
    let builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            queue_open_project(app, &args, &cwd);
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.unminimize();
                let _ = win.show();
                let _ = win.set_focus();
            }
        }));
    // Release builds use one GUI instance; development permits parallel worktree instances. Register
    // the single-instance plugin first so argv/cwd forwarding is reliable.
    run_with_builder(builder, initial_open_project)
}

#[cfg(feature = "gui")]
fn run_with_builder(builder: tauri::Builder<tauri::Wry>, initial_open_project: Option<PathBuf>) {
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        // Desktop updater checks latest.json and installs signed packages; process supports relaunch.
        // Endpoint/key live in tauri.conf.json, and release.sh enables artifacts only with a signing key.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(PtyManager::new())
        .manage(WebServer::new())
        .manage(browser::BrowserManager::new())
        .manage(QuitState::default())
        .manage(PendingOpenProject(std::sync::Mutex::new(
            initial_open_project.map(|p| p.to_string_lossy().into_owned()),
        )))
        .setup(|app| {
            // Size the main window to 77% x 81% of the current monitor with caps, then center it.
            // This fits laptops and uses larger displays; fall back to configured dimensions if unavailable.
            if let Some(win) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = win.current_monitor() {
                    let scale = monitor.scale_factor();
                    let sz = monitor.size(); // Physical pixels.
                    let sw = sz.width as f64 / scale;
                    let sh = sz.height as f64 / scale;
                    let w = (sw * 0.77).clamp(900.0, 1620.0);
                    let h = (sh * 0.81).clamp(600.0, 1035.0);
                    let _ = win.set_size(tauri::LogicalSize::new(w, h));
                    // Center within work_area rather than the full display so menu bar/Dock do not skew
                    // placement. Physical coordinates also handle multiple monitors and avoid center() timing.
                    let wa = monitor.work_area();
                    let x = wa.position.x as f64 + (wa.size.width as f64 - w * scale) / 2.0;
                    let y = wa.position.y as f64 + (wa.size.height as f64 - h * scale) / 2.0;
                    let _ =
                        win.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
                }
            }

            // DevTools open only on demand from the development title-bar action.

            // Replace the macOS default menu without native close_window so Cmd+W reaches the WebView's
            // split/tab close handler. Preserve essential copy/paste/quit/minimize/fullscreen actions.
            #[cfg(target_os = "macos")]
            {
                use tauri::menu::{
                    AboutMetadataBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder,
                };
                use tauri::Emitter;
                let app_name = app.package_info().name.clone();
                // Custom update/settings items emit menu://action. Native About displays icon/version and
                // attribution; explicitly supply package version so metadata cannot suppress it.
                let about_meta = AboutMetadataBuilder::new()
                    .version(Some(app.package_info().version.to_string()))
                    .copyright(Some("A product by VLINX"))
                    .build();
                let check_update_item =
                    MenuItemBuilder::with_id("check-update", "Check for Updates…").build(app)?;
                let settings_item = MenuItemBuilder::with_id("settings", "Settings…")
                    .accelerator("CmdOrCtrl+,")
                    .build(app)?;
                let install_vela_item = MenuItemBuilder::with_id(
                    "install-vela-command",
                    "Install 'vela' Command in PATH…",
                )
                .enabled(!cfg!(debug_assertions))
                .build(app)?;
                let uninstall_vela_item = MenuItemBuilder::with_id(
                    "uninstall-vela-command",
                    "Uninstall 'vela' Command from PATH…",
                )
                .enabled(!cfg!(debug_assertions))
                .build(app)?;
                let app_menu = SubmenuBuilder::new(app, &app_name)
                    .about(Some(about_meta))
                    .item(&check_update_item)
                    .separator()
                    .item(&settings_item)
                    .separator()
                    .item(&install_vela_item)
                    .item(&uninstall_vela_item)
                    .separator()
                    // Omit the rarely useful macOS Services submenu to reduce terminal-menu noise.
                    .hide()
                    .hide_others()
                    .separator()
                    .quit()
                    .build()?;
                // Omit native undo/redo so Cmd+Z/Cmd+Shift+Z reach CodeMirror/ProseMirror's JS histories;
                // NSApplication would otherwise consume them while native WebView undo cannot control those
                // editors. Keep cut/copy/paste/select-all because WebKit translates them into DOM events.
                let edit_menu = SubmenuBuilder::new(app, "Edit")
                    .cut()
                    .copy()
                    .paste()
                    .select_all()
                    .build()?;
                // Register terminal split accelerators with AppKit as well as the frontend keydown
                // listener. WKWebView can consume browser-reserved key equivalents such as Cmd+Shift+D
                // before JavaScript sees them; native menu actions make both split directions reliable.
                let split_right_item = MenuItemBuilder::with_id("split-right", "Split Right")
                    .accelerator("CmdOrCtrl+D")
                    .build(app)?;
                let split_down_item = MenuItemBuilder::with_id("split-down", "Split Down")
                    .accelerator("CmdOrCtrl+Shift+D")
                    .build(app)?;
                let terminal_menu = SubmenuBuilder::new(app, "Terminal")
                    .item(&split_right_item)
                    .item(&split_down_item)
                    .build()?;
                // Omit redundant View/Window menus; title-bar controls remain. Help items open the website,
                // feedback page, or emit the shared frontend share action.
                let visit_website_item =
                    MenuItemBuilder::with_id("visit-website", "Visit Website").build(app)?;
                let send_feedback_item =
                    MenuItemBuilder::with_id("send-feedback", "Send Feedback…").build(app)?;
                let share_item =
                    MenuItemBuilder::with_id("share", "❤️ Share us…").build(app)?;
                let help_menu = SubmenuBuilder::new(app, "Help")
                    .item(&visit_website_item)
                    .item(&send_feedback_item)
                    .separator()
                    .item(&share_item)
                    .build()?;
                let menu = MenuBuilder::new(app)
                    .items(&[&app_menu, &edit_menu, &terminal_menu, &help_menu])
                    .build()?;
                app.set_menu(menu)?;
                // Emit custom item IDs to frontend onMenuAction; the system handles predefined items.
                app.on_menu_event(move |app_handle, event| {
                    use tauri_plugin_opener::OpenerExt;
                    let id = event.id().0.as_str();
                    match id {
                        // Forward custom application and terminal commands to the frontend.
                        "settings" | "check-update" | "share" | "split-right" | "split-down" => {
                            let _ = app_handle.emit("menu://action", id);
                        }
                        // Like VS Code, install/remove the PATH shim only on explicit user action and
                        // report its exact destination or conflict in a native dialog.
                        "install-vela-command" | "uninstall-vela-command" => {
                            use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                            let installing = id == "install-vela-command";
                            let result = if installing {
                                agent::spawn_cli::install_user_cli()
                            } else {
                                agent::spawn_cli::uninstall_user_cli()
                            };
                            let (message, kind) = match result {
                                Ok(status) if status.installed => (
                                    format!(
                                        "The 'vela' command is ready at:\n{}\n\nRun: vela <project-path>",
                                        status.path.as_deref().unwrap_or("PATH")
                                    ),
                                    MessageDialogKind::Info,
                                ),
                                Ok(status) if status.conflict.is_some() => (
                                    format!(
                                        "A different 'vela' command remains at:\n{}\n\nVelaTerm did not modify it.",
                                        status.conflict.as_deref().unwrap_or("PATH")
                                    ),
                                    MessageDialogKind::Warning,
                                ),
                                Ok(_) => (
                                    "The VelaTerm-managed 'vela' command was removed from PATH."
                                        .to_string(),
                                    MessageDialogKind::Info,
                                ),
                                Err(e) => (e.to_string(), MessageDialogKind::Error),
                            };
                            app_handle
                                .dialog()
                                .message(message)
                                .title(if installing {
                                    "Install 'vela' Command"
                                } else {
                                    "Uninstall 'vela' Command"
                                })
                                .kind(kind)
                                .show(|_| {});
                        }
                        // Open help URLs directly through the Rust-side opener.
                        "visit-website" => {
                            let _ = app_handle
                                .opener()
                                .open_url("https://velaterm.com", None::<&str>);
                        }
                        "send-feedback" => {
                            let _ = app_handle
                                .opener()
                                .open_url("https://velaterm.com/feedback", None::<&str>);
                        }
                        _ => {}
                    }
                });
            }

            // Register the Windows AppUserModelId so bare no-bundle releases can display toasts.
            #[cfg(windows)]
            register_aumid_for_notifications(&app.config().identifier, &app.package_info().name);

            // Store the database under the application data directory.
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("failed to get app data dir: {e}"))?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| format!("failed to create data dir: {e}"))?;
            // Record data_dir so Windows SSH connections prefer bundled tools over a broken/missing PATH install.
            ssh_remote::set_data_dir(data_dir.clone());
            let db = Db::open(&data_dir.join("vlx-term.db"))?;

            // When cleanup is enabled, remove pasted images older than 24h left by crashes. Normal exit
            // removes this process's files precisely; disabling the setting skips both paths.
            if pasted_image_cleanup_enabled(&db) {
                let swept =
                    files::sweep_stale_pasted_images(std::time::Duration::from_secs(24 * 60 * 60));
                if swept > 0 {
                    eprintln!("startup sweep: removed {swept} stale pasted temp image(s)");
                }
            }
            app.manage(db);

            // Extract bundled terminal commands into data_dir/bin, which PTY sessions prepend to PATH.
            // Failure is nonfatal and only disables those commands.
            if let Err(e) = agent::spawn_cli::install(&data_dir) {
                eprintln!("failed to install built-in command shims (vspawn / vopen etc. will be unavailable): {e}");
            }
            // Release builds auto-install the shell command on Windows/Linux; macOS requires explicit action.
            #[cfg(all(not(debug_assertions), not(target_os = "macos")))]
            match agent::spawn_cli::install_user_cli() {
                Ok(status) => println!(
                    "vela command ready: {}",
                    status.path.as_deref().unwrap_or("PATH")
                ),
                Err(e) => eprintln!("failed to install vela command in PATH: {e}"),
            }
            // Refresh bundled skills in both Claude/Codex user directories on every version.
            agent::spawn_cli::refresh_installed_skills();

            // On Windows, register the bundled minimal Git Bash tree beside the executable and use it
            // in place. Avoid the former ~326 MB copy into data_dir and Defender scan on every variant
            // switch. Failure falls back to PowerShell/system Git Bash.
            #[cfg(windows)]
            {
                match app
                    .path()
                    .resolve("resources/gitbash", tauri::path::BaseDirectory::Resource)
                {
                    Ok(dir) => agent::gitbash::set_bundled_dir(dir),
                    Err(e) => eprintln!("failed to resolve Git Bash resource path (falling back to PowerShell): {e}"),
                }
                // Best-effort background cleanup removes obsolete gitbash-min and staging trees only;
                // never touch the still-used on-demand gitbash-full cache.
                let stale = data_dir.join("gitbash-min");
                let stale_staging = data_dir.join("gitbash-min.staging");
                if stale.exists() || stale_staging.exists() {
                    std::thread::spawn(move || {
                        if stale.exists() {
                            if let Err(e) = std::fs::remove_dir_all(&stale) {
                                eprintln!(
                                    "failed to remove legacy gitbash-min copy ({}): {e}",
                                    stale.display()
                                );
                            }
                        }
                        let _ = std::fs::remove_dir_all(&stale_staging);
                    });
                }
            }

            // Extract the OpenCode status bridge and inject its path at launch. Failure merely falls
            // back to screen-based status detection.
            if let Err(e) = agent::opencode::install(&data_dir) {
                eprintln!("failed to install opencode plugin (opencode status will degrade to screen detection): {e}");
            }

            // Start the random-port/token loopback hook service for agent status callbacks.
            let hooks = HookServer::start(AppCtx::Tauri(app.handle().clone()))?;
            app.manage(hooks);

            // Auto-start LAN remote access when the persisted enabled flag is set, restoring the state
            // from before the last quit (GitHub issue #15). On a thread because start() synchronously
            // preflights the port bind. Failures (e.g. port in use) are recorded on the WebServer and
            // shown in the remote-access panel — never fatal for app startup.
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    match command_core::web_server_autostart(&AppCtx::Tauri(handle)) {
                        Ok(Some(status)) => println!(
                            "remote access auto-started on port {}",
                            status.port.unwrap_or(0)
                        ),
                        Ok(None) => {}
                        Err(e) => eprintln!("remote access auto-start failed: {e}"),
                    }
                });
            }

            // Install macOS session-aware notification click handling; unsupported builds skip it.
            #[cfg(target_os = "macos")]
            notify_native::install(app.handle().clone());

            // Remote contexts cannot call native_notify directly, so relay vlx://remote-notify through
            // the backend with its window/session identity. Native macOS/Windows channels target that
            // window on click; unsupported/failed delivery falls back to the official plugin.
            {
                let handle = app.handle().clone();
                app.listen_any("vlx://remote-notify", move |event| {
                    let Ok(p) = serde_json::from_str::<RemoteNotify>(event.payload()) else {
                        return;
                    };
                    #[cfg(target_os = "macos")]
                    let shown =
                        notify_native::send(&p.window_label, &p.session_id, &p.title, &p.body);
                    // Windows launch data encodes window/session so click handling targets the remote window.
                    #[cfg(target_os = "windows")]
                    let shown = notify_win::send(
                        handle.clone(),
                        p.window_label.clone(),
                        p.session_id.clone(),
                        p.title.clone(),
                        p.body.clone(),
                        false, // RemoteNotify currently has no sound field; default to silent.
                    );
                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                    let shown = false;
                    if !shown {
                        use tauri_plugin_notification::NotificationExt;
                        let _ = handle
                            .notification()
                            .builder()
                            .title(&p.title)
                            .body(&p.body)
                            .show();
                    }
                });
            }

            // Relay remote-window reconnect requests into a manual tunnel-monitor kick. It retries after
            // automatic recovery gives up and ignores kicks while the tunnel is healthy.
            app.listen_any("vlx://ssh-reconnect", move |event| {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(event.payload()) else {
                    return;
                };
                if let Some(session) = v.get("session").and_then(|s| s.as_str()) {
                    ssh_remote::kick_tunnel(session);
                }
            });

            // DevTools are opened on demand rather than automatically in development.

            // Disable WebView2 browser accelerator handling in the main view so Ctrl-based app shortcuts
            // reach frontend keydown. Built-in browser child views retain normal browser accelerators.
            #[cfg(target_os = "windows")]
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.with_webview(|webview| {
                    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
                    use windows::core::Interface;
                    // SAFETY: setup runs on the main thread and wry owns the live WebView2 COM pointer.
                    unsafe {
                        if let Ok(core) = webview.controller().CoreWebView2() {
                            if let Ok(settings) = core.Settings() {
                                if let Ok(s3) = settings.cast::<ICoreWebView2Settings3>() {
                                    let _ = s3.SetAreBrowserAcceleratorKeysEnabled(false);
                                }
                            }
                        }
                    }
                });
            }

            // Set Windows ICON_BIG explicitly. Tauri reliably set only ICON_SMALL, leaving the taskbar
            // with a generic icon. Extract embedded executable icons and apply both through WM_SETICON.
            #[cfg(target_os = "windows")]
            if let Some(win) = app.get_webview_window("main") {
                if let Ok(hwnd) = win.hwnd() {
                    use std::os::windows::ffi::OsStrExt;
                    use windows::Win32::Foundation::{LPARAM, WPARAM};
                    use windows::Win32::UI::Shell::ExtractIconExW;
                    use windows::Win32::UI::WindowsAndMessaging::{
                        SendMessageW, HICON, ICON_BIG, ICON_SMALL, WM_SETICON,
                    };
                    use windows::core::PCWSTR;
                    if let Ok(exe) = std::env::current_exe() {
                        let mut wide: Vec<u16> = exe.as_os_str().encode_wide().collect();
                        wide.push(0);
                        let mut large = HICON::default();
                        let mut small = HICON::default();
                        // SAFETY: main-thread setup, NUL-terminated path, valid output pointers/live hwnd.
                        unsafe {
                            let n = ExtractIconExW(
                                PCWSTR(wide.as_ptr()),
                                0,
                                Some(&mut large),
                                Some(&mut small),
                                1,
                            );
                            if n > 0 {
                                if !large.is_invalid() {
                                    SendMessageW(
                                        hwnd,
                                        WM_SETICON,
                                        Some(WPARAM(ICON_BIG as usize)),
                                        Some(LPARAM(large.0 as isize)),
                                    );
                                }
                                if !small.is_invalid() {
                                    SendMessageW(
                                        hwnd,
                                        WM_SETICON,
                                        Some(WPARAM(ICON_SMALL as usize)),
                                        Some(LPARAM(small.0 as isize)),
                                    );
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Most desktop data commands use this single spawn_blocking dispatch entry; adding a web
            // dispatch command automatically covers desktop too.
            commands::desktop_call,
            commands::take_open_project_request,
            commands::quit_prompt_ack,
            commands::confirm_quit,
            commands::cancel_quit,
            commands::vela_command_status,
            commands::install_vela_command,
            commands::uninstall_vela_command,
            // Commands below require direct native/specialized channels and bypass desktop_call:
            // 1) PTY spawn Channel, hot-path write/resize, and direct teardown kill.
            commands::pty_spawn,
            commands::pty_write,
            commands::pty_resize,
            commands::pty_kill,
            // 2) Binary/persistence channels, Git Bash, and shell detection.
            commands::get_session_cwd,
            commands::list_shells,
            commands::agent_install_recipe,
            commands::gitbash_status,
            commands::download_full_gitbash,
            commands::read_recording,
            commands::save_pasted_image,
            commands::save_doc_image,
            // 3) Remote-access host management that owns live WebServer state.
            commands::web_pairing_create,
            commands::web_devices_list,
            commands::web_device_revoke,
            // 4) Main-thread windows and developer tools.
            commands::open_remote_window,
            commands::probe_remote_fingerprint,
            commands::url_trust_fingerprint,
            commands::open_devtools,
            // Desktop-only SSH connection, provisioning, serve, forwarding, and auto-login.
            commands::ssh_probe_host,
            commands::ssh_trust_host,
            commands::ssh_connect,
            commands::ssh_disconnect,
            // 5) Desktop-only built-in browser tabs.
            browser::browser_open,
            browser::browser_navigate,
            browser::browser_back,
            browser::browser_forward,
            browser::browser_reload,
            browser::browser_stop,
            browser::browser_set_bounds,
            browser::browser_set_visible,
            browser::browser_close,
            // 6) macOS native notifications invoked directly from notify.ts.
            native_notify,
            native_notify_auth_status,
            native_notify_request_auth,
        ])
        .build(tauri_context())
        .expect("error while building tauri application")
        .run({
            use std::sync::atomic::Ordering;
            move |app, event| {
                // Confirm user-triggered exits in the frontend so the dialog can carry localized copy and the
                // "save workspace" checkbox. Programmatic exit/restart with a code already belongs to a confirmed
                // workflow and must not prompt again.
                let request_quit_confirmation = |app: &tauri::AppHandle| {
                    let st = app.state::<QuitState>();
                    if st.confirmed.load(Ordering::SeqCst) || st.pending.swap(true, Ordering::SeqCst)
                    {
                        return;
                    }
                    st.acked.store(false, Ordering::SeqCst);
                    use tauri::Emitter;
                    if app.emit("app://quit-requested", ()).is_err() {
                        native_quit_confirmation(app);
                        return;
                    }
                    // A webview that never acknowledges within the grace period would leave the application
                    // unquittable, so fall back to the native dialog.
                    let app = app.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        let st = app.state::<QuitState>();
                        if st.pending.load(Ordering::SeqCst)
                            && !st.acked.load(Ordering::SeqCst)
                            && !st.confirmed.load(Ordering::SeqCst)
                        {
                            native_quit_confirmation(&app);
                        }
                    });
                };

                match &event {
                    tauri::RunEvent::ExitRequested { code: None, api, .. }
                        if !app.state::<QuitState>().confirmed.load(Ordering::SeqCst) =>
                    {
                        api.prevent_exit();
                        request_quit_confirmation(app);
                    }
                    // On Windows/Linux, intercept final main-window closure before destruction so canceling
                    // does not leave a windowless background process.
                    #[cfg(not(target_os = "macos"))]
                    tauri::RunEvent::WindowEvent {
                        label,
                        event: tauri::WindowEvent::CloseRequested { api, .. },
                        ..
                    } if label == "main"
                        && !app.state::<QuitState>().confirmed.load(Ordering::SeqCst) =>
                    {
                        api.prevent_close();
                        request_quit_confirmation(app);
                    }
                    _ => {}
                }

                // When enabled, remove only this process's pasted-image temp files on exit, preserving
                // images used by other instances.
                if let tauri::RunEvent::Exit = event {
                    let enabled = app
                        .try_state::<Db>()
                        .map(|db| pasted_image_cleanup_enabled(db.inner()))
                        .unwrap_or(true);
                    if enabled {
                        let n = files::cleanup_pasted_images();
                        if n > 0 {
                            eprintln!("exit cleanup: removed {n} temp image(s) pasted this session");
                        }
                    }
                }
            }
        });
}

/// Degraded quit confirmation used when the webview cannot present the frontend dialog. A native message dialog
/// supports neither a checkbox nor translated copy, so it only offers plain quit/cancel and never saves the
/// workspace. Copy stays English to match the other native dialogs.
#[cfg(feature = "gui")]
fn native_quit_confirmation(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let app = app.clone();
    app.clone()
        .dialog()
        .message("Any running terminal and agent sessions will be stopped.")
        .title("Quit VelaTerm?")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Quit".to_string(),
            "Cancel".to_string(),
        ))
        .show(move |should_quit| {
            let st = app.state::<QuitState>();
            st.pending.store(false, Ordering::SeqCst);
            if should_quit {
                st.confirmed.store(true, Ordering::SeqCst);
                app.exit(0);
            }
        });
}

/// Return the build-time Tauri context from one macro expansion shared by GUI/headless, avoiding
/// duplicate embedded resources and preserving release identifier overrides for default data paths.
#[cfg(feature = "gui")]
fn tauri_context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

/// Resolve the serve identifier for default data path and production checks. GUI uses embedded config;
/// the minimal server uses the independent `io.vlinx.vlxterm.server` identity, preventing accidental
/// use of desktop databases and retaining production plaintext protections. SSH normally supplies data-dir.
#[cfg(feature = "gui")]
fn serve_identifier() -> String {
    tauri_context().config().identifier.clone()
}
#[cfg(not(feature = "gui"))]
fn serve_identifier() -> String {
    "io.vlinx.vlxterm.server".to_string()
}

// ─────────────────────────── Headless (--serve) mode ───────────────────────────

/// Parsed `--serve` arguments.
#[derive(Debug, PartialEq)]
struct ServeArgs {
    port: u16,
    password: String,
    data_dir: Option<String>,
    /// `--local-http`: plaintext loopback HTTP/WebSocket for the Electron sidecar, avoiding TLS latency.
    local_http: bool,
    /// `--lan-http`: plaintext 0.0.0.0 mode for native mobile-shell testing where self-signed WebView
    /// certificates cannot work. It is CLI-only, rejected by production identifiers, and loses to
    /// `--local-http` when both are supplied.
    lan_http: bool,
    /// `--print-pairing`: print the full pairing URL including the long-lived `#pair` token fragment even
    /// when stdout is not a terminal. Without it, non-TTY runs (systemd/journald) get only the token-less
    /// base URL so the credential does not persist in service logs.
    print_pairing: bool,
}

/// Recommended headless password environment variable, avoiding exposure in process arguments.
/// The legacy VLX_SERVE_PASSWORD remains a compatibility fallback after the VelaTerm rename.
const SERVE_PASSWORD_ENV: &str = "VELA_SERVE_PASSWORD";
/// Legacy pre-rename password variable for backward compatibility only.
const SERVE_PASSWORD_ENV_LEGACY: &str = "VLX_SERVE_PASSWORD";

/// Parse arguments after `--serve`; explicit password overrides env_password. Errors are user-facing.
fn parse_serve_args(args: &[String], env_password: Option<String>) -> Result<ServeArgs, String> {
    let mut port: u16 = 8799;
    let mut password: Option<String> = None;
    let mut data_dir: Option<String> = None;
    let mut local_http = false;
    let mut lan_http = false;
    let mut print_pairing = false;

    let mut it = args.iter().skip(2); // Skip executable name and --serve.
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--port" => {
                let v = it.next().ok_or("--port requires a value")?;
                port = v
                    .parse()
                    .map_err(|_| format!("--port has invalid value: {v}"))?;
            }
            "--password" => {
                password = Some(it.next().ok_or("--password requires a value")?.clone());
            }
            "--data-dir" => {
                data_dir = Some(it.next().ok_or("--data-dir requires a value")?.clone());
            }
            "--local-http" => {
                local_http = true;
            }
            "--lan-http" => {
                lan_http = true;
            }
            "--print-pairing" => {
                print_pairing = true;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let password = password
        .or(env_password)
        .filter(|p| !p.trim().is_empty())
        .ok_or(format!(
            "missing access password: use --password <password>, or set the {SERVE_PASSWORD_ENV} env var (recommended, keeps it out of the process list)"
        ))?;

    Ok(ServeArgs {
        port,
        password,
        data_dir,
        local_http,
        lan_http,
        print_pairing,
    })
}

/// Whether `--serve` may print the full pairing URL including the long-lived `#pair` token fragment:
/// explicitly requested via `--print-pairing`, or stdout is an interactive terminal (a human who needs
/// the link). Non-TTY stdout without the flag — systemd, pipes — must not receive the credential, since
/// journald and log files would persist it.
fn should_print_pairing(flag: bool, is_tty: bool) -> bool {
    flag || is_tty
}

/// Headless data directory: explicit --data-dir wins; otherwise use platform data directory plus
/// identifier, matching GUI SQLite/recording/certificate layout.
fn serve_data_dir(args: &ServeArgs, identifier: &str) -> Result<std::path::PathBuf, String> {
    if let Some(dir) = &args.data_dir {
        return Ok(std::path::PathBuf::from(dir));
    }
    dirs::data_dir().map(|d| d.join(identifier)).ok_or(
        "cannot resolve platform data dir, specify it explicitly with --data-dir".to_string(),
    )
}

/// Pure-CLI headless entry point for `vlx-term --serve`.
///
/// Build no Tauri app/window/display dependency. Construct GUI-equivalent Db/PtyManager/HookServer
/// state under AppCtx::Headless and reuse the HTTPS/login/WebSocket/PTY service. Ctrl+C/SIGTERM shuts
/// down axum and active PTYs gracefully.
pub fn run_serve(args: &[String]) {
    // Prefer the new variable and fall back to the legacy name.
    let env_password = std::env::var(SERVE_PASSWORD_ENV)
        .ok()
        .or_else(|| std::env::var(SERVE_PASSWORD_ENV_LEGACY).ok());
    let parsed = match parse_serve_args(args, env_password) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("vlx-term --serve failed to start: {e}");
            eprintln!(
                "usage: vlx-term --serve [--port 8799] [--password <password>] [--data-dir <dir>] [--local-http] [--lan-http] [--print-pairing]"
            );
            std::process::exit(1);
        }
    };

    if let Err(e) = serve_main(&parsed) {
        eprintln!("vlx-term --serve failed to start: {e}");
        std::process::exit(1);
    }
}

/// Fallible `run_serve` implementation extracted for `?` error propagation.
fn serve_main(args: &ServeArgs) -> Result<(), String> {
    let identifier = serve_identifier();
    let data_dir = serve_data_dir(args, &identifier)?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("failed to create data dir: {e}"))?;
    let db = Db::open(&data_dir.join("vlx-term.db"))?;

    // Match GUI cleanup of stale pasted images from abnormal exits when enabled.
    if pasted_image_cleanup_enabled(&db) {
        let swept = files::sweep_stale_pasted_images(std::time::Duration::from_secs(24 * 60 * 60));
        if swept > 0 {
            println!("startup sweep: removed {swept} stale pasted temp image(s)");
        }
    }

    // Match GUI extraction of terminal commands/OpenCode plugin; failures are logged and nonfatal.
    if let Err(e) = agent::spawn_cli::install(&data_dir) {
        eprintln!("failed to install built-in command shims (terminal commands vspawn / vopen etc. will be unavailable): {e}");
    }
    agent::spawn_cli::refresh_installed_skills();
    if let Err(e) = agent::opencode::install(&data_dir) {
        eprintln!("failed to install opencode plugin (opencode status will degrade to screen detection): {e}");
    }

    // Create the in-process host before starting and attaching the AppCtx-dependent hook service.
    let host = std::sync::Arc::new(HeadlessHost::new(data_dir.clone(), db));
    let ctx = AppCtx::Headless(std::sync::Arc::clone(&host));
    host.set_hooks(HookServer::start(ctx.clone())?);

    // Reuse the GUI web service. local-http is loopback plaintext, lan-http is LAN plaintext, and
    // neither selects self-signed LAN TLS; loopback wins if both plaintext flags are present.
    let web = WebServer::new();
    let mode = if args.local_http {
        crate::web::ServeMode::LoopbackHttp
    } else if args.lan_http {
        crate::web::ServeMode::LanHttp
    } else {
        crate::web::ServeMode::LanTls
    };
    // Production rejects LAN plaintext; mobile production must use HTTPS plus certificate pinning.
    // Loopback plaintext never leaves the host and remains permitted.
    if matches!(mode, crate::web::ServeMode::LanHttp)
        && crate::web::is_production_identifier(&identifier)
    {
        return Err(format!(
            "LAN plaintext (--lan-http, binds 0.0.0.0 in plaintext) is only available in dev builds; this is a release build (identifier={identifier}). \
             For production use the default TLS mode, or HTTPS with certificate pinning (see architecture §20)."
        ));
    }
    let status = web.start(
        ctx.clone(),
        crate::web::StartAuth::Password(args.password.clone()),
        Some(args.port),
        mode,
    )?;

    println!("vlx-term headless server started");
    println!("  data dir: {}", data_dir.display());

    // Print a fully usable preferred URL first. LAN TLS requires the complete #pair fragment with
    // token/public key; plaintext modes can use their bare URL. The fragment carries the long-lived
    // pairing token, so it is only printed to an interactive terminal or on explicit --print-pairing —
    // never into journald/service logs, where it would persist (GitHub issue #24).
    let primary = if matches!(mode, crate::web::ServeMode::LanTls) {
        if should_print_pairing(args.print_pairing, std::io::IsTerminal::is_terminal(&std::io::stdout())) {
            match web.create_pairing(None, false) {
                Ok(info) => Some(info.url),
                Err(e) => {
                    eprintln!("  failed to create pairing link: {e}");
                    None
                }
            }
        } else {
            println!(
                "  pairing link withheld (stdout is not a terminal): run with --print-pairing to print it, or create a pairing link from the app"
            );
            status.urls.first().cloned()
        }
    } else {
        status.urls.first().cloned()
    };
    if let Some(url) = &primary {
        println!("  open in browser: {url}");
    }
    // Print other interface URLs without repeating the long pairing fragment; users can append it.
    if status.urls.len() > 1 {
        if matches!(mode, crate::web::ServeMode::LanTls)
            && primary.as_deref().is_some_and(|u| u.contains("#pair="))
        {
            println!("  other addresses (swap host, keep the #pair=… part):");
        } else {
            println!("  other addresses:");
        }
        for url in status.urls.iter().skip(1) {
            println!("    {url}");
        }
    }
    // Only LAN TLS has a certificate fingerprint.
    if let Some(fp) = &status.fingerprint {
        println!("  certificate fingerprint (SHA-256): {fp}");
    }
    // Auto-start the secondary LAN remote instance (ctx.remote_web()) when persisted settings enable it,
    // e.g. remote access turned on from the Electron shell before a sidecar restart. Runs after the
    // primary CLI-configured server so a port conflict deterministically hits this secondary instance,
    // where it is logged and nonfatal — the CLI server semantics stay untouched.
    match command_core::web_server_autostart(&ctx) {
        Ok(Some(remote)) => println!(
            "  remote access auto-started on port {}",
            remote.port.unwrap_or(0)
        ),
        Ok(None) => {}
        Err(e) => eprintln!("  remote access auto-start failed: {e}"),
    }

    println!("  press Ctrl+C (or send SIGTERM) to quit");

    // Wait for cross-platform Ctrl+C and Unix SIGTERM used by systemd stop.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to start signal-wait runtime: {e}"))?;
    rt.block_on(async {
        #[cfg(unix)]
        {
            let mut term =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "failed to register SIGTERM handler (only Ctrl+C will work): {e}"
                        );
                        let _ = tokio::signal::ctrl_c().await;
                        return;
                    }
                };
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    });

    // Gracefully stop local/LAN services and terminate all active PTY sessions.
    println!("received shutdown signal, stopping…");
    web.stop();
    ctx.remote_web().stop();
    ctx.pty().kill_all();
    // Match GUI removal of this process's pasted-image files when cleanup is enabled.
    if pasted_image_cleanup_enabled(ctx.db()) {
        let n = files::cleanup_pasted_images();
        if n > 0 {
            println!("exit cleanup: removed {n} temp image(s) pasted this session");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(rest: &[&str]) -> Vec<String> {
        let mut v = vec!["vlx-term".to_string(), "--serve".to_string()];
        v.extend(rest.iter().map(|s| s.to_string()));
        v
    }

    #[test]
    fn open_project_args_resolve_relative_path_and_reject_bad_arity() {
        let base = std::env::temp_dir().join(format!(
            "vela-open-args-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let project = base.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let args = vec![
            "velaterm".to_string(),
            "--open-project".to_string(),
            "project".to_string(),
        ];
        let got = open_project_from_args(&args, &base).unwrap().unwrap();
        assert_eq!(got, project.canonicalize().unwrap());

        let missing = vec!["velaterm".to_string(), "--open-project".to_string()];
        assert!(open_project_from_args(&missing, &base).is_err());
        let extra = vec![
            "velaterm".to_string(),
            "--open-project".to_string(),
            "project".to_string(),
            "extra".to_string(),
        ];
        assert!(open_project_from_args(&extra, &base).is_err());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn parse_serve_args_full_flags() {
        let got = parse_serve_args(
            &argv(&["--port", "9001", "--password", "pw", "--data-dir", "/tmp/x"]),
            None,
        )
        .expect("parsing should succeed");
        assert_eq!(
            got,
            ServeArgs {
                port: 9001,
                password: "pw".into(),
                data_dir: Some("/tmp/x".into()),
                local_http: false,
                lan_http: false,
                print_pairing: false,
            }
        );
    }

    #[test]
    fn parse_serve_args_print_pairing_flag() {
        // --print-pairing opts in to printing the full pairing URL on non-TTY stdout.
        let got = parse_serve_args(&argv(&["--print-pairing", "--password", "pw"]), None)
            .expect("parsing should succeed");
        assert!(got.print_pairing);
        // Omitting the flag leaves it false.
        let got = parse_serve_args(&argv(&["--password", "pw"]), None).expect("parsing should succeed");
        assert!(!got.print_pairing);
    }

    #[test]
    fn should_print_pairing_truth_table() {
        // The explicit flag always wins; otherwise only an interactive terminal receives the fragment.
        assert!(should_print_pairing(true, true));
        assert!(should_print_pairing(true, false));
        assert!(should_print_pairing(false, true));
        assert!(!should_print_pairing(false, false));
    }

    #[test]
    fn parse_serve_args_local_http_flag() {
        // --local-http is set; other values stay default.
        let got = parse_serve_args(&argv(&["--local-http", "--password", "pw"]), None)
            .expect("parsing should succeed");
        assert!(got.local_http);
        assert!(!got.lan_http);
        // Omitting the flag leaves it false.
        let got = parse_serve_args(&argv(&["--password", "pw"]), None).expect("parsing should succeed");
        assert!(!got.local_http);
        assert!(!got.lan_http);
    }

    #[test]
    fn parse_serve_args_lan_http_flag() {
        // --lan-http independently enables mobile-shell LAN plaintext.
        let got =
            parse_serve_args(&argv(&["--lan-http", "--password", "pw"]), None).expect("parsing should succeed");
        assert!(got.lan_http);
        assert!(!got.local_http);
    }

    #[test]
    fn parse_serve_args_defaults_and_env_password() {
        // Without --password use the environment; port defaults to 8799 and data_dir to None.
        let got = parse_serve_args(&argv(&[]), Some("env-pw".into())).expect("parsing should succeed");
        assert_eq!(got.port, 8799);
        assert_eq!(got.password, "env-pw");
        assert_eq!(got.data_dir, None);
        assert!(!got.local_http);

        // --password overrides the environment.
        let got = parse_serve_args(&argv(&["--password", "flag-pw"]), Some("env-pw".into()))
            .expect("parsing should succeed");
        assert_eq!(got.password, "flag-pw");
    }

    #[test]
    fn parse_serve_args_rejects_bad_input() {
        // Missing or blank passwords from both sources are errors.
        assert!(parse_serve_args(&argv(&[]), None).is_err());
        assert!(parse_serve_args(&argv(&[]), Some("  ".into())).is_err());
        // Invalid ports, missing values, and unknown arguments are errors.
        assert!(parse_serve_args(&argv(&["--port", "abc", "--password", "x"]), None).is_err());
        assert!(parse_serve_args(&argv(&["--password"]), None).is_err());
        assert!(parse_serve_args(&argv(&["--bogus", "--password", "x"]), None).is_err());
    }

    #[test]
    fn serve_data_dir_prefers_explicit() {
        let args = ServeArgs {
            port: 8799,
            password: "x".into(),
            data_dir: Some("/tmp/custom".into()),
            local_http: false,
            lan_http: false,
            print_pairing: false,
        };
        assert_eq!(
            serve_data_dir(&args, "io.vlinx.vlxterm").unwrap(),
            std::path::PathBuf::from("/tmp/custom")
        );

        // Default to the platform data directory under the identifier.
        let args = ServeArgs {
            data_dir: None,
            ..args
        };
        let got = serve_data_dir(&args, "io.vlinx.vlxterm").unwrap();
        assert!(got.ends_with("io.vlinx.vlxterm"), "got {got:?}");
    }
}
