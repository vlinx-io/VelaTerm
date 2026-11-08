//! Clickable Linux notifications with click-through navigation to the originating window and session.
//!
//! The official plugin does not report clicks on the desktop, so this posts through the freedesktop
//! notification service directly. It covers the main window and remote-connection windows, and hands a
//! notice to the official plugin when the service cannot take it.
//!
//! Raising a window is subject to the desktop's focus policy. Servers that send an activation token let
//! Wayland compositors accept the request; without one the desktop may only mark the window as ready.

#![cfg(target_os = "linux")]

mod freedesktop;

use std::sync::OnceLock;

use gtk::prelude::GtkWindowExt;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use freedesktop::{Client, Notice, Origin};

/// Click event shared with the macOS and Windows channels and observed by frontend onNotificationClick.
const NOTIFY_CLICK_EVENT: &str = "notification://click";

static CLIENT: OnceLock<Client> = OnceLock::new();

/// Queue a notification for this window and session. Returns false when the worker is gone so the caller can fall back.
pub fn send(
    app: &AppHandle,
    window_label: String,
    session_id: String,
    title: String,
    body: String,
    sound: bool,
) -> bool {
    let client = CLIENT.get_or_init(|| start_client(app));
    client.send(Notice {
        origin: Origin { window_label, session_id },
        title,
        body,
        sound,
    })
}

fn start_client(app: &AppHandle) -> Client {
    let click_app = app.clone();
    let fallback_app = app.clone();
    let icon = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
        .unwrap_or_default();
    Client::start(
        app.package_info().name.clone(),
        icon,
        move |origin, token| activate(&click_app, origin, token),
        move |notice| {
            let _ = fallback_app
                .notification()
                .builder()
                .title(&notice.title)
                .body(&notice.body)
                .show();
        },
    )
}

/// Raise the source window and tell only that window to open the session. GTK calls need the main thread.
fn activate(app: &AppHandle, origin: Origin, token: Option<String>) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Origin { window_label, session_id } = origin;
        let Some(win) = handle.get_webview_window(&window_label) else {
            // The source window has closed. Its session belongs to that window, so only raise the main window.
            if let Some(main) = handle.get_webview_window("main") {
                let _ = main.unminimize();
                let _ = main.show();
                let _ = main.set_focus();
            }
            return;
        };
        if let (Some(token), Ok(gtk_window)) = (token, win.gtk_window()) {
            gtk_window.set_startup_id(&token);
        }
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        let _ = handle.emit_to(window_label.as_str(), NOTIFY_CLICK_EVENT, session_id);
    });
}
