//! Native macOS notifications through UNUserNotificationCenter.
//!
//! Send notifications carrying session IDs and emit `notification://click` on activation. This works
//! only in a properly signed `.app` bundle. Development/unbundled builds skip install/send so callers
//! can fall back to the official plugin through notify.ts.
//!
//! The official plugin does not expose desktop click callbacks, so this integrates the native framework.
//! Bundling and valid signing are mandatory: a bare binary crashes and ad-hoc signing cannot obtain permission.

#![cfg(target_os = "macos")]
// objc2 0.6 makes most methods safe; retain unsafe blocks for version compatibility and suppress warnings.
#![allow(unused_unsafe)]

use std::ptr::NonNull;
use std::sync::OnceLock;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread};
use objc2_foundation::{NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSettings, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};
use tauri::{AppHandle, Emitter, Manager};

/// Frontend event for session navigation; payload is the originating session ID.
pub const NOTIFY_CLICK_EVENT: &str = "notification://click";

/// Store the app handle globally because the Objective-C delegate cannot access setup state.
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Waits between checks that the clicked notification's window holds focus, about 1.2 s in total. The system
/// activates the app and orders its main window forward at a time of its own choosing, which can follow any
/// single raise, so a window that lost focus during this period is raised again.
const RAISE_CHECK_WAITS_MS: [u64; 5] = [150, 200, 250, 300, 300];

fn raise(win: &tauri::WebviewWindow) {
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
}

/// Route notification clicks using identifier `{window label}\x1f{session ID}`. Raise and emit only
/// to the source window so multiple main/remote windows do not all navigate. Legacy identifiers with
/// only a session ID fall back to a global broadcast.
fn emit_click(identifier: &str) {
    let Some(app) = APP_HANDLE.get() else {
        return;
    };
    match identifier.split_once('\u{1f}') {
        Some((label, session_id)) => {
            // Emit navigation to the source window first so the frontend opens the session.
            let _ = app.emit_to(label, NOTIFY_CLICK_EVENT, session_id);
            if app.get_webview_window(label).is_some() {
                let app = app.clone();
                let label = label.to_string();
                std::thread::spawn(move || {
                    for (check, wait) in RAISE_CHECK_WAITS_MS.into_iter().enumerate() {
                        std::thread::sleep(std::time::Duration::from_millis(wait));
                        let app_main = app.clone();
                        let label = label.clone();
                        let _ = app.run_on_main_thread(move || {
                            let Some(win) = app_main.get_webview_window(&label) else { return };
                            if check == 0 {
                                raise(&win);
                            } else if !win.is_focused().unwrap_or(true) {
                                crate::diagnostic_warn!("notification click: window {label} lost focus during activation, raising it again");
                                raise(&win);
                            }
                        });
                    }
                });
            }
        }
        None => {
            let _ = app.emit(NOTIFY_CLICK_EVENT, identifier);
        }
    }
}

define_class!(
    #[unsafe(super(objc2::runtime::NSObject))]
    #[thread_kind = AllocAnyThread]
    #[name = "VlxNotificationDelegate"]
    struct ClickDelegate;

    unsafe impl NSObjectProtocol for ClickDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for ClickDelegate {
        // Request Banner+Sound because macOS suppresses foreground notifications by default.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::Sound,));
        }

        // On activation, extract the identifier and emit navigation. completion must be a DynBlock
        // matching the native trait signature or the method will not register.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion: &block2::DynBlock<dyn Fn()>,
        ) {
            let id = unsafe { response.notification().request().identifier() }.to_string();
            emit_click(&id);
            // Completion is mandatory or the system considers handling unfinished.
            completion.call(());
        }
    }
);

impl ClickDelegate {
    fn new() -> Retained<Self> {
        unsafe { msg_send![Self::alloc(), init] }
    }
}

/// Check for an `.app` bundle before touching UNUserNotificationCenter. A development bare binary
/// would throw NSException and terminate, so perform this Rust-side preflight.
fn has_app_bundle() -> bool {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

/// At startup, store the app handle, install the click delegate, and request permission. Skip
/// unbundled development builds so send can signal fallback without crashing.
pub fn install(app: AppHandle) {
    if !has_app_bundle() {
        return;
    }
    let _ = APP_HANDLE.set(app);
    // Catch Objective-C exceptions so native edge cases cannot crash startup.
    let _ = unsafe {
        objc2::exception::catch(|| {
            let center = UNUserNotificationCenter::currentNotificationCenter();
            let delegate = ClickDelegate::new();
            let proto: &ProtocolObject<dyn UNUserNotificationCenterDelegate> =
                ProtocolObject::from_ref(&*delegate);
            center.setDelegate(Some(proto));
            // The delegate property is weak, so intentionally retain this single object for process
            // lifetime. Its fixed allocation cannot grow.
            std::mem::forget(delegate);

            let options = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound;
            let handler = RcBlock::new(|_granted: Bool, _err: *mut NSError| {});
            center.requestAuthorizationWithOptions_completionHandler(options, &handler);
        })
    };
}

/// Send a native notification identified by `{window_label}\x1f{session_id}` for targeted window
/// activation and session navigation. Return whether the native channel handled it; false tells the
/// caller to use the official plugin fallback.
pub fn send(window_label: &str, session_id: &str, title: &str, body: &str) -> bool {
    if !has_app_bundle() {
        return false;
    }
    let r = unsafe {
        objc2::exception::catch(|| {
            let center = UNUserNotificationCenter::currentNotificationCenter();
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(title));
            content.setBody(&NSString::from_str(body));

            let identifier = NSString::from_str(&format!("{window_label}\u{1f}{session_id}"));
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &identifier,
                &content,
                None,
            );
            let handler = RcBlock::new(|_err: *mut NSError| {});
            center.addNotificationRequest_withCompletionHandler(&request, Some(&handler));
        })
    };
    r.is_ok()
}

/// Read native macOS authorizationStatus and return authorized, denied, notDetermined, provisional,
/// ephemeral, or unsupported. Synchronize the async callback through a channel with a two-second timeout.
pub fn authorization_status() -> String {
    if !has_app_bundle() {
        return "unsupported".to_string();
    }
    let (tx, rx) = std::sync::mpsc::channel::<isize>();
    let r = unsafe {
        objc2::exception::catch(|| {
            let center = UNUserNotificationCenter::currentNotificationCenter();
            let handler = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
                let status = unsafe { settings.as_ref().authorizationStatus() };
                let _ = tx.send(status.0);
            });
            center.getNotificationSettingsWithCompletionHandler(&handler);
        })
    };
    if r.is_err() {
        return "unsupported".to_string();
    }
    match rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(0) => "notDetermined",
        Ok(1) => "denied",
        Ok(2) => "authorized",
        Ok(3) => "provisional",
        Ok(4) => "ephemeral",
        _ => "unsupported",
    }
    .to_string()
}

/// Request native macOS notification permission. The system prompts only for notDetermined and returns
/// established states immediately. Wait up to 60 seconds for user interaction and return authorization.
pub fn request_authorization() -> bool {
    if !has_app_bundle() {
        return false;
    }
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let r = unsafe {
        objc2::exception::catch(|| {
            let center = UNUserNotificationCenter::currentNotificationCenter();
            let options = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound;
            let handler = RcBlock::new(move |granted: Bool, _err: *mut NSError| {
                let _ = tx.send(granted.as_bool());
            });
            center.requestAuthorizationWithOptions_completionHandler(options, &handler);
        })
    };
    if r.is_err() {
        return false;
    }
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .unwrap_or(false)
}
