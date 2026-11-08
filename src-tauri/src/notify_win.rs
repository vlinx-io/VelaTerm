//! Native Windows WinRT toast notifications carrying session IDs for click-through navigation.
//!
//! The official `tauri-plugin-notification` does not deliver desktop click callbacks, matching the
//! macOS limitation. This module creates WinRT ToastNotifications, listens for Activated, and emits
//! the session ID stored in `launch` through `notification://click`. useNotifications then opens the
//! session and raises the window using the same frontend path as macOS.
//!
//! Scope is foreground activation while the application is already running, covering normal agent
//! completion clicks. Cold launch from Action Center requires a registered COM activator and is omitted.
//!
//! Delivery also requires registered AUMID identity (see lib.rs::register_aumid_for_notifications)
//! and remains subject to Windows Do Not Disturb/Focus Assist.
//!
//! Create toasts and handlers on Tauri's main STA/winit thread via run_on_main_thread so activation is
//! delivered. A global table keeps notifications alive until activation or dismissal, then removes them.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use windows::core::{IInspectable, Interface, HSTRING};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{
    ToastActivatedEventArgs, ToastDismissedEventArgs, ToastNotification, ToastNotificationManager,
};

/// Click event shared with the native macOS channel and observed by frontend onNotificationClick.
const NOTIFY_CLICK_EVENT: &str = "notification://click";
/// Separator for `{window label}<SEP>{session ID}` parsed by handle_click. It must be XML-safe because
/// it enters the toast launch attribute. XML 1.0 rejects the `\x1f` used by macOS identifiers, making
/// LoadXml fail silently. `|` is unambiguous because labels and UUID session IDs never contain it.
const SEP: char = '|';

/// Live toasts keyed by incrementing IDs, retained until activation/dismissal and accessed only on the main thread.
static LIVE: Mutex<Option<HashMap<u64, ToastNotification>>> = Mutex::new(None);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn live_insert(id: u64, toast: ToastNotification) {
    LIVE.lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(id, toast);
}
fn live_remove(id: u64) {
    if let Some(map) = LIVE.lock().unwrap().as_mut() {
        map.remove(&id);
    }
}

/// Escape XML metacharacters in title/body/launch and discard C0 controls except tab/newline/carriage
/// return. XML 1.0 rejects any such interpolated character and LoadXml otherwise drops the toast silently.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // XML 1.0 forbids C0 controls other than tab, newline, and carriage return.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// Send a native Windows toast and emit `notification://click` with its session ID on activation.
/// Return true when handled; return false on main-thread dispatch failure so the frontend can fall back.
pub fn send(
    app: AppHandle,
    window_label: String,
    session_id: String,
    title: String,
    body: String,
    sound: bool,
) -> bool {
    let aumid = app.config().identifier.clone();
    // Encode window label and session ID in launch for window-targeted emission after activation.
    let launch = format!("{window_label}{SEP}{session_id}");
    let audio = if sound {
        ""
    } else {
        r#"<audio silent="true"/>"#
    };
    let xml = format!(
        r#"<toast launch="{}" activationType="foreground"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>{}</toast>"#,
        xml_escape(&launch),
        xml_escape(&title),
        xml_escape(&body),
        audio,
    );

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let app_for_main = app.clone();
    // Create and show on the main STA/message-pump thread so activation callbacks are delivered.
    app.run_on_main_thread(move || {
        if let Err(e) = show_on_main(&app_for_main, id, &aumid, &xml) {
            crate::diagnostic_warn!("failed to send Windows toast: {e:?}");
            live_remove(id);
        }
    })
    .is_ok()
}

fn show_on_main(app: &AppHandle, id: u64, aumid: &str, xml: &str) -> windows::core::Result<()> {
    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(xml))?;
    let toast = ToastNotification::CreateToastNotification(&doc)?;

    // On click, parse launch, emit to the originating window, and raise it.
    let app_click = app.clone();
    let on_activated =
        TypedEventHandler::<ToastNotification, IInspectable>::new(move |_sender, args| {
            if let Some(args) = args.as_ref() {
                if let Ok(a) = args.cast::<ToastActivatedEventArgs>() {
                    if let Ok(arg) = a.Arguments() {
                        handle_click(&app_click, &arg.to_string());
                    }
                }
            }
            live_remove(id);
            Ok(())
        });
    toast.Activated(&on_activated)?;

    // Remove dismissed or expired toasts from the live table.
    let on_dismissed =
        TypedEventHandler::<ToastNotification, ToastDismissedEventArgs>::new(move |_s, _a| {
            live_remove(id);
            Ok(())
        });
    toast.Dismissed(&on_dismissed)?;

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(aumid))?;
    notifier.Show(&toast)?;
    live_insert(id, toast);
    Ok(())
}

/// Parse launch, raise the source window, and emit notification://click to it. Address the event with
/// emit_to: `win.emit` is a global broadcast despite its receiver, so the main window would also take it
/// and raise itself over the remote window.
fn handle_click(app: &AppHandle, launch: &str) {
    let (label, session_id) = match launch.split_once(SEP) {
        Some((l, s)) => (l, s.to_string()),
        None => ("", launch.to_string()),
    };
    if let Some(win) = app.get_webview_window(label) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        let _ = app.emit_to(label, NOTIFY_CLICK_EVENT, session_id);
        return;
    }
    // A launch without a label has no addressee, so broadcast it. A closed source window's session belongs
    // to that window, so in that case only raise the main window.
    if label.is_empty() {
        let _ = app.emit(NOTIFY_CLICK_EVENT, session_id);
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}
