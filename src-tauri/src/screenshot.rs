//! System-wide screenshots: global hotkey, screen capture, and the annotation overlay window.
//!
//! Flow: a borderless, top-level `screenshot` window is created hidden shortly after startup and
//! reused, so a screenshot never waits for a web view to start and load its page. The global hotkey
//! (or `screenshot_start`) captures the monitor under the cursor on a worker thread, keeps the PNG in
//! memory, moves the hidden window over that monitor, and tells the overlay page (`screenshot.html`)
//! through [`FRAME_EVENT`]. The page pulls the frame through `screenshot_frame`, calls
//! `screenshot_ready` once it has painted, and lets the user select a region and annotate it. The
//! finished PNG comes back through `screenshot_copy` or `screenshot_save`; `screenshot_close`
//! discards everything. Ending a screenshot hides the window again.
//!
//! Capture backends: macOS runs the system `screencapture` tool (it follows Screen Recording
//! permission and every display API change without extra code here), Windows uses xcap, and Linux
//! reports the feature as unsupported.
//!
//! The shortcut is an accelerator string such as `Ctrl+Cmd+S`, persisted in app_settings under
//! [`SHORTCUT_KEY`]; an empty value turns the hotkey off. The backend registers it at startup so the
//! hotkey works before and without the main window's frontend.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::db::{repo, Db};

/// app_settings key holding the accelerator; an empty string disables the hotkey.
pub const SHORTCUT_KEY: &str = "vlx-screenshot-shortcut";
/// Label of the overlay window.
pub const OVERLAY_LABEL: &str = "screenshot";
/// Event sent to the overlay page when a new frame is waiting; the payload is the screenshot id.
const FRAME_EVENT: &str = "screenshot-frame";
/// How long after startup the hidden overlay is created, so it does not compete with the main window.
const WARM_UP_DELAY: Duration = Duration::from_secs(3);
/// How long a captured frame may wait for the overlay to show it before the window is recreated.
const SHOW_TIMEOUT: Duration = Duration::from_secs(5);

/// Default accelerator. macOS follows the Ctrl+Cmd family used by chat apps' screenshot keys
/// (WeChat and QQ use Ctrl+Cmd+A) without taking their key. Windows avoids Alt+A (WeChat),
/// Ctrl+Alt+A (QQ), Win+Shift+S (Snipping Tool), and Ctrl+Alt+S (JetBrains settings).
pub fn default_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "Ctrl+Cmd+S"
    } else {
        "Ctrl+Alt+X"
    }
}

/// Whether this platform can capture the screen.
pub fn supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

/// Captured frame waiting for the overlay page, plus bookkeeping for one screenshot session.
#[derive(Default)]
struct Session {
    /// Nonzero while a screenshot is in progress. The page reports it back with `screenshot_ready`,
    /// so a late signal from an earlier screenshot cannot reveal the window.
    id: u64,
    png: Option<Vec<u8>>,
    /// The overlay has painted this frame and is on screen.
    shown: bool,
    /// macOS: the application that was frontmost before the hotkey, reactivated when the overlay closes.
    #[cfg(target_os = "macos")]
    previous_app_pid: Option<i32>,
}

/// Screenshot state managed by Tauri.
#[derive(Default)]
pub struct ScreenshotState {
    /// The currently registered accelerator (as stored) and its parsed form.
    shortcut: Mutex<Option<(String, Shortcut)>>,
    /// Last registration failure for the stored accelerator, shown in Settings.
    last_error: Mutex<Option<String>>,
    session: Mutex<Session>,
    /// Set while a capture is running so a held or repeated hotkey starts only one.
    capturing: std::sync::atomic::AtomicBool,
    /// Source of screenshot ids.
    last_id: std::sync::atomic::AtomicU64,
    /// Serializes creation of the overlay window: the warm-up and a capture may both need it.
    building: Mutex<()>,
}

/// Shortcut status reported to Settings.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    /// Effective accelerator; empty when the hotkey is off.
    shortcut: String,
    default_shortcut: String,
    supported: bool,
    /// Registration error for `shortcut`, e.g. when another application already holds it.
    error: Option<String>,
}

/// Plugin whose handler starts a screenshot whenever the registered hotkey is pressed.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                start(app);
            }
        })
        .build()
}

/// Register the stored (or default) accelerator at startup and prepare the hidden overlay window.
/// Registration failures are kept for Settings.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    if !supported() {
        return;
    }
    // The overlay is also needed with the hotkey off: the title bar starts screenshots too.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(WARM_UP_DELAY);
        if let Err(e) = ensure_overlay(&handle) {
            crate::diagnostic_warn!("failed to prepare the screenshot window: {e}");
        }
    });
    let stored = read_stored(app);
    let accel = stored.unwrap_or_else(|| default_shortcut().to_string());
    if accel.is_empty() {
        return;
    }
    let state = app.state::<ScreenshotState>();
    match register(app, &accel) {
        Ok(()) => *state.last_error.lock().unwrap() = None,
        Err(e) => {
            crate::diagnostic_warn!("failed to register the screenshot shortcut: {e}");
            *state.last_error.lock().unwrap() = Some(e);
        }
    }
}

fn read_stored<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    let db = app.try_state::<Db>()?;
    let conn = db.conn.lock().ok()?;
    repo::get_app_settings(&conn).ok()?.remove(SHORTCUT_KEY)
}

fn parse(accel: &str) -> Result<Shortcut, String> {
    accel
        .parse::<Shortcut>()
        .map_err(|_| format!("\"{accel}\" is not a valid shortcut."))
}

/// Replace the registered hotkey with `accel`, restoring the previous one when registration fails.
fn register<R: Runtime>(app: &AppHandle<R>, accel: &str) -> Result<(), String> {
    let state = app.state::<ScreenshotState>();
    let gs = app.global_shortcut();
    let new = if accel.is_empty() { None } else { Some(parse(accel)?) };
    let mut current = state.shortcut.lock().unwrap();
    if let (Some((_, old)), Some(new)) = (current.as_ref(), new.as_ref()) {
        if old == new {
            return Ok(());
        }
    }
    let previous = current.take();
    if let Some((_, old)) = previous.as_ref() {
        let _ = gs.unregister(*old);
    }
    let Some(new) = new else {
        return Ok(());
    };
    match gs.register(new) {
        Ok(()) => {
            *current = Some((accel.to_string(), new));
            Ok(())
        }
        Err(_) => {
            if let Some((prev_accel, old)) = previous {
                if gs.register(old).is_ok() {
                    *current = Some((prev_accel, old));
                }
            }
            Err("This shortcut is already in use by another application.".to_string())
        }
    }
}

fn status<R: Runtime>(app: &AppHandle<R>) -> ShortcutStatus {
    let stored = read_stored(app);
    let error = app.state::<ScreenshotState>().last_error.lock().unwrap().clone();
    ShortcutStatus {
        shortcut: stored.unwrap_or_else(|| default_shortcut().to_string()),
        default_shortcut: default_shortcut().to_string(),
        supported: supported(),
        error,
    }
}

/// Read the screenshot shortcut. Reads SQLite, so it runs on the blocking pool.
#[tauri::command]
pub async fn screenshot_shortcut_get(app: AppHandle) -> Result<ShortcutStatus, String> {
    tauri::async_runtime::spawn_blocking(move || status(&app))
        .await
        .map_err(|e| format!("Failed to read the screenshot shortcut: {e}"))
}

/// Register and persist a new accelerator (empty turns the hotkey off). Nothing is persisted when
/// registration fails, so the stored value always matches a working hotkey.
#[tauri::command]
pub async fn screenshot_shortcut_set(
    app: AppHandle,
    shortcut: String,
) -> Result<ShortcutStatus, String> {
    if !supported() {
        return Err("Screenshots are not supported on this platform.".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let accel = shortcut.trim().to_string();
        register(&app, &accel)?;
        *app.state::<ScreenshotState>().last_error.lock().unwrap() = None;
        let db = app.state::<Db>();
        let conn = db.conn.lock().map_err(|_| "Failed to open settings.".to_string())?;
        let entries = std::collections::HashMap::from([(SHORTCUT_KEY.to_string(), accel)]);
        repo::set_app_settings(&conn, &entries)?;
        drop(conn);
        Ok(status(&app))
    })
    .await
    .map_err(|e| format!("Failed to save the screenshot shortcut: {e}"))?
}

/// Start a screenshot from the application UI. Returns immediately; capture runs on a worker thread.
/// Synchronous by design: it only sets a flag and spawns a thread (category 1, no IO here).
#[tauri::command]
pub fn screenshot_start(app: AppHandle) {
    start(&app);
}

/// Begin a screenshot unless one is already capturing or on screen.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    use std::sync::atomic::Ordering;
    if !supported() {
        return;
    }
    let state = app.state::<ScreenshotState>();
    // A screenshot already on screen keeps its work.
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        if win.is_visible().unwrap_or(false) {
            let _ = win.set_focus();
            return;
        }
    }
    if state.capturing.swap(true, Ordering::SeqCst) {
        return;
    }
    // Read before the overlay activates VelaTerm, so focus can return to that app afterwards.
    #[cfg(target_os = "macos")]
    let previous_app_pid = macos::frontmost_other_app_pid();
    #[cfg(not(target_os = "macos"))]
    let previous_app_pid = None;
    let app = app.clone();
    std::thread::spawn(move || {
        let result = capture_and_open(&app, previous_app_pid);
        let state = app.state::<ScreenshotState>();
        state.capturing.store(false, Ordering::SeqCst);
        if let Err(e) = result {
            crate::diagnostic_warn!("screenshot capture failed: {e}");
            *state.session.lock().unwrap() = Session::default();
        }
    });
}

fn capture_and_open<R: Runtime>(app: &AppHandle<R>, previous_app_pid: Option<i32>) -> Result<(), String> {
    let cursor = app.cursor_position().map_err(|e| e.to_string())?;
    let monitor = monitor_containing(app, cursor)?;
    let pos = *monitor.position();
    let size = *monitor.size();
    let scale = monitor.scale_factor();
    // Make sure the overlay exists while the capture runs. Normally the warm-up created it already
    // and this returns at once; right after startup, or after the window was lost, it overlaps the
    // capture instead of following it.
    let overlay = {
        let app = app.clone();
        std::thread::spawn(move || ensure_overlay(&app))
    };
    let png = capture_monitor(app, pos, size, scale);
    let win = overlay
        .join()
        .map_err(|_| "the screenshot window task panicked".to_string())??;
    let png = png?;

    let state = app.state::<ScreenshotState>();
    let id = state.last_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    {
        let mut session = state.session.lock().unwrap();
        *session = Session::default();
        session.id = id;
        session.png = Some(png);
        #[cfg(target_os = "macos")]
        {
            session.previous_app_pid = previous_app_pid;
        }
        #[cfg(not(target_os = "macos"))]
        let _ = previous_app_pid;
    }
    cover_monitor(&win, pos, size);
    // The page loads the frame on this event; a page that is still loading asks with
    // `screenshot_pending` once it is ready, so the event may safely go unheard.
    let _ = app.emit_to(OVERLAY_LABEL, FRAME_EVENT, id);
    if wait_until_shown(app, id) {
        return Ok(());
    }
    // The page never showed the frame, e.g. because WebKit ended its process while it was hidden.
    // A fresh window picks up the waiting frame as soon as its page has loaded.
    crate::diagnostic_warn!("the screenshot window did not respond; recreating it");
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.destroy();
    }
    let win = ensure_overlay(app)?;
    cover_monitor(&win, pos, size);
    if wait_until_shown(app, id) {
        Ok(())
    } else {
        Err("the screenshot window did not appear".into())
    }
}

/// The overlay window, created hidden when it does not exist yet. Must not run on the main thread:
/// building a window from a main-thread callback deadlocks on Windows.
fn ensure_overlay<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>, String> {
    let state = app.state::<ScreenshotState>();
    let _building = state.building.lock().unwrap();
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        return Ok(win);
    }
    // Hidden until the page reports `screenshot_ready`, so the user never sees an empty frame.
    // Position and size are set for each screenshot by `cover_monitor`.
    WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("screenshot.html".into()))
        .title("Screenshot")
        .decorations(false)
        .resizable(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .visible(false)
        .background_color(tauri::window::Color(0, 0, 0, 255))
        .build()
        .map_err(|e| format!("failed to open the screenshot window: {e}"))
}

/// Place the hidden overlay exactly over one monitor. Physical units on purpose: logical placement
/// can round differently on mixed-DPI setups.
fn cover_monitor<R: Runtime>(
    win: &WebviewWindow<R>,
    pos: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
) {
    let _ = win.set_position(pos);
    let _ = win.set_size(size);
}

/// Wait until screenshot `id` is on screen or has ended; false when the overlay never showed it.
fn wait_until_shown<R: Runtime>(app: &AppHandle<R>, id: u64) -> bool {
    let state = app.state::<ScreenshotState>();
    let deadline = Instant::now() + SHOW_TIMEOUT;
    loop {
        {
            let session = state.session.lock().unwrap();
            if session.id != id || session.shown {
                return true;
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The monitor whose physical frame contains `point`, falling back to the primary monitor.
/// `monitor_from_point` is not used: on macOS it expects logical points while `cursor_position`
/// reports physical pixels, so it misses every monitor and the fallback always won.
fn monitor_containing<R: Runtime>(
    app: &AppHandle<R>,
    point: tauri::PhysicalPosition<f64>,
) -> Result<tauri::Monitor, String> {
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let inside = |m: &&tauri::Monitor| {
        let (p, s) = (m.position(), m.size());
        point.x >= p.x as f64
            && point.x < p.x as f64 + s.width as f64
            && point.y >= p.y as f64
            && point.y < p.y as f64 + s.height as f64
    };
    if let Some(m) = monitors.iter().find(inside) {
        return Ok(m.clone());
    }
    app.primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.into_iter().next())
        .ok_or_else(|| "no monitor found".to_string())
}

#[cfg(target_os = "macos")]
fn capture_monitor<R: Runtime>(
    app: &AppHandle<R>,
    pos: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    scale: f64,
) -> Result<Vec<u8>, String> {
    if !macos::screen_capture_allowed() {
        macos::explain_missing_permission(app);
        return Err("screen recording permission missing".into());
    }
    macos::capture(app, pos, size, scale)
}

#[cfg(target_os = "windows")]
fn capture_monitor<R: Runtime>(
    _app: &AppHandle<R>,
    pos: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    _scale: f64,
) -> Result<Vec<u8>, String> {
    win_capture::capture(pos, size)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn capture_monitor<R: Runtime>(
    _app: &AppHandle<R>,
    _pos: tauri::PhysicalPosition<i32>,
    _size: tauri::PhysicalSize<u32>,
    _scale: f64,
) -> Result<Vec<u8>, String> {
    Err("unsupported platform".into())
}

/// PNG of the captured monitor for the overlay page.
#[tauri::command]
pub fn screenshot_frame(state: tauri::State<'_, ScreenshotState>) -> Result<tauri::ipc::Response, String> {
    // Category 1: hands over a buffer already in memory.
    let session = state.session.lock().unwrap();
    let png = session.png.clone().ok_or("No screenshot is in progress.")?;
    Ok(tauri::ipc::Response::new(png))
}

/// Id of the screenshot whose frame is waiting for the overlay, for a page that loaded after the
/// frame event was sent.
#[tauri::command]
pub fn screenshot_pending(state: tauri::State<'_, ScreenshotState>) -> Option<u64> {
    // Category 1: reads in-memory state.
    let session = state.session.lock().unwrap();
    session.png.as_ref().map(|_| session.id)
}

/// The overlay has painted the frame of screenshot `id`: show it above everything and give it
/// keyboard focus.
#[tauri::command]
pub fn screenshot_ready(app: AppHandle, id: u64) {
    // Category 2: window operations that must reach the main thread; no IO.
    {
        let state = app.state::<ScreenshotState>();
        let mut session = state.session.lock().unwrap();
        if id == 0 || session.id != id {
            return;
        }
        session.shown = true;
    }
    let Some(win) = app.get_webview_window(OVERLAY_LABEL) else {
        return;
    };
    #[cfg(target_os = "macos")]
    macos::raise_above_menu_bar(&win);
    let _ = win.show();
    let _ = win.set_focus();
}

/// Discard the screenshot and hide the overlay.
#[tauri::command]
pub fn screenshot_close(app: AppHandle) {
    // Category 2: destroys a window; no IO.
    close_overlay(&app);
}

fn close_overlay<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<ScreenshotState>();
    let session = std::mem::take(&mut *state.session.lock().unwrap());
    // Hidden rather than destroyed: the next screenshot reuses the loaded page.
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.hide();
    }
    #[cfg(target_os = "macos")]
    if let Some(pid) = session.previous_app_pid {
        macos::activate_app(pid);
    }
    #[cfg(not(target_os = "macos"))]
    drop(session);
}

/// Read the PNG sent as the raw request body.
fn request_png(request: &tauri::ipc::Request<'_>) -> Result<Vec<u8>, String> {
    match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) if !bytes.is_empty() => Ok(bytes.clone()),
        _ => Err("The screenshot image is missing.".to_string()),
    }
}

/// Copy the edited screenshot (raw PNG body) to the clipboard and close the overlay.
#[tauri::command]
pub async fn screenshot_copy(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let png = request_png(&request)?;
    close_overlay(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let image = tauri::image::Image::from_bytes(&png)
            .map_err(|e| format!("Failed to decode the screenshot: {e}"))?;
        app.clipboard()
            .write_image(&image)
            .map_err(|e| format!("Failed to copy the screenshot: {e}"))
    })
    .await
    .map_err(|e| format!("Failed to copy the screenshot: {e}"))?
}

/// Close the overlay, ask where to save the edited screenshot (raw PNG body), and write it.
/// Returns the saved path, or None when the user cancels the dialog.
#[tauri::command]
pub async fn screenshot_save(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let png = request_png(&request)?;
    // The overlay sits above normal windows, so it must be gone before the save panel appears.
    close_overlay(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let name = format!("Screenshot {}.png", file_timestamp());
        let mut dialog = app
            .dialog()
            .file()
            .set_file_name(name)
            .add_filter("PNG", &["png"]);
        if let Some(dir) = dirs::desktop_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(target) = dialog.blocking_save_file() else {
            return Ok(None);
        };
        let path = target
            .into_path()
            .map_err(|e| format!("Failed to save the screenshot: {e}"))?;
        std::fs::write(&path, &png).map_err(|e| format!("Failed to save the screenshot: {e}"))?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| format!("Failed to save the screenshot: {e}"))?
}

/// Local time as `YYYY-MM-DD HH.mm.ss`, the style macOS uses for screenshot file names.
fn file_timestamp() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!(
        "{:04}-{:02}-{:02} {:02}.{:02}.{:02}",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    pub fn screen_capture_allowed() -> bool {
        // SAFETY: argument-free CoreGraphics query, callable from any thread.
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    /// Ask for Screen Recording permission and explain where to grant it. The system prompt appears
    /// only the first time, so a native dialog with a shortcut to System Settings follows it.
    pub fn explain_missing_permission<R: Runtime>(app: &AppHandle<R>) {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
        use tauri_plugin_opener::OpenerExt;
        // SAFETY: argument-free CoreGraphics call; it registers the app in the privacy list.
        unsafe {
            CGRequestScreenCaptureAccess();
        }
        let open = app
            .dialog()
            .message(
                "To take screenshots, allow VelaTerm under Screen & System Audio Recording in \
                 System Settings, then restart VelaTerm.",
            )
            .title("Screen Recording Permission Needed")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Open System Settings".into(),
                "Cancel".into(),
            ))
            .blocking_show();
        if open {
            let _ = app.opener().open_url(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
                None::<&str>,
            );
        }
    }

    /// Capture one display with the system tool. `-R` takes the rectangle in global points and
    /// writes it at the display's native pixel density.
    pub fn capture<R: Runtime>(
        app: &AppHandle<R>,
        pos: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
        scale: f64,
    ) -> Result<Vec<u8>, String> {
        let dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("screenshots");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let file = dir.join(format!("capture-{}.png", uuid::Uuid::new_v4()));
        let rect = format!(
            "{},{},{},{}",
            (pos.x as f64 / scale).round() as i64,
            (pos.y as f64 / scale).round() as i64,
            (size.width as f64 / scale).round() as i64,
            (size.height as f64 / scale).round() as i64,
        );
        let status = std::process::Command::new("/usr/sbin/screencapture")
            .args(["-x", "-t", "png", "-R", &rect])
            .arg(&file)
            .status()
            .map_err(|e| format!("failed to run screencapture: {e}"))?;
        let bytes = std::fs::read(&file);
        let _ = std::fs::remove_file(&file);
        if !status.success() {
            return Err(format!("screencapture exited with {status}"));
        }
        bytes.map_err(|e| format!("failed to read the capture: {e}"))
    }

    /// Lift the overlay above the menu bar and Dock and let it appear over full-screen apps.
    pub fn raise_above_menu_bar<R: Runtime>(win: &WebviewWindow<R>) {
        let target = win.clone();
        let _ = win.run_on_main_thread(move || {
            let Ok(ns_window) = target.ns_window() else {
                return;
            };
            let ns_window = ns_window as *mut AnyObject;
            if ns_window.is_null() {
                return;
            }
            // NSPopUpMenuWindowLevel (101) sits above NSMainMenuWindowLevel (24) and the Dock (20).
            const LEVEL: isize = 101;
            // NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary.
            const BEHAVIOR: usize = (1 << 0) | (1 << 8);
            // SAFETY: a live NSWindow owned by this window, messaged on the main thread.
            unsafe {
                let _: () = msg_send![ns_window, setLevel: LEVEL];
                let _: () = msg_send![ns_window, setCollectionBehavior: BEHAVIOR];
            }
        });
    }

    /// PID of the frontmost application when it is not VelaTerm itself.
    pub fn frontmost_other_app_pid() -> Option<i32> {
        // SAFETY: NSWorkspace class methods and property reads; objects are autoreleased by AppKit.
        unsafe {
            let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
            if workspace.is_null() {
                return None;
            }
            let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
            if app.is_null() {
                return None;
            }
            let pid: i32 = msg_send![app, processIdentifier];
            (pid > 0 && pid as u32 != std::process::id()).then_some(pid)
        }
    }

    /// Hand focus back to the application that was in front before the screenshot.
    pub fn activate_app(pid: i32) {
        // SAFETY: NSRunningApplication lookup by PID; a nil result is checked before use.
        unsafe {
            let app: *mut AnyObject = msg_send![
                class!(NSRunningApplication),
                runningApplicationWithProcessIdentifier: pid
            ];
            if !app.is_null() {
                let _: bool = msg_send![app, activateWithOptions: 0usize];
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod win_capture {
    use tauri::{PhysicalPosition, PhysicalSize};
    use xcap::image::{codecs::png, ExtendedColorType, ImageEncoder};

    /// Capture the monitor whose top-left corner is `pos` and encode it as PNG.
    pub fn capture(pos: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> Result<Vec<u8>, String> {
        let monitor = xcap::Monitor::from_point(pos.x + 1, pos.y + 1).map_err(|e| e.to_string())?;
        let image = monitor.capture_image().map_err(|e| e.to_string())?;
        if image.width() != size.width || image.height() != size.height {
            crate::diagnostic_warn!("screenshot size differs from the monitor size");
        }
        let mut out = Vec::new();
        // Fast compression: the PNG only travels to the overlay page and back.
        png::PngEncoder::new_with_quality(
            &mut out,
            png::CompressionType::Fast,
            png::FilterType::NoFilter,
        )
        .write_image(image.as_raw(), image.width(), image.height(), ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
        Ok(out)
    }
}
