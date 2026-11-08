//! Keeps a stalled clipboard source from freezing the app on macOS.
//!
//! Applications may publish clipboard data lazily, supplying it only when someone reads it. A remote
//! desktop client does this and then waits on the remote machine, so a read can block for as long as that
//! machine takes. WebKit performs paste reads synchronously on the main thread, which freezes every window
//! until the source answers.
//!
//! Cmd+V and the Edit menu's Paste therefore run a short-lived helper process first. Reading in the helper
//! makes the system clipboard service fetch and keep the data, so WebKit's own read then returns at once and
//! every existing paste handler keeps working unchanged. The read stays out of this process because
//! CoreFoundation serializes access to a pasteboard: a stuck read on a background thread would still block a
//! later main-thread copy or paste. Killing the helper also ends the wait cleanly.

#[cfg(target_os = "macos")]
pub use platform::{request_paste, run_probe};

/// Waits until the clipboard can be read without blocking. Returns false when its source is not responding.
///
/// Async per the IO rule: the wait runs on the blocking pool.
#[tauri::command]
pub async fn clipboard_wait_ready() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    return tauri::async_runtime::spawn_blocking(platform::probe_is_ready)
        .await
        .map_err(|e| e.to_string());
    #[cfg(not(target_os = "macos"))]
    Ok(true)
}

#[cfg(target_os = "macos")]
mod platform {
    use std::process::{Command, Stdio};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use objc2::msg_send;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;
    use serde_json::json;
    use tauri::{AppHandle, Runtime};

    /// How long a paste may wait for the clipboard source before it is abandoned.
    const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
    /// A source that just timed out is not asked again this soon, so a context-menu paste reading image and
    /// text in turn reports one failure instead of waiting twice.
    const FAILURE_MEMORY: Duration = Duration::from_secs(2);
    const POLL_INTERVAL: Duration = Duration::from_millis(5);
    /// The representations WebKit and the paste handlers read. Other types are left alone because resolving
    /// them can trigger expensive conversions or downloads that this paste would never use.
    const PASTE_TYPES: &[&str] = &[
        "public.utf8-plain-text",
        "public.utf16-external-plain-text",
        "public.utf16-plain-text",
        "public.html",
        "public.rtf",
        "public.png",
        "public.tiff",
        "public.jpeg",
        "com.compuserve.gif",
        "public.url",
        "public.file-url",
        "com.apple.webarchive",
        "com.apple.WebKit.custom-pasteboard-data",
    ];

    static LAST_TIMEOUT: Mutex<Option<Instant>> = Mutex::new(None);

    #[derive(Default)]
    struct PendingPaste {
        presses: usize,
        running: bool,
        /// Window that receives the notice if this paste is abandoned.
        window: Option<String>,
    }

    static PENDING: Mutex<PendingPaste> = Mutex::new(PendingPaste {
        presses: 0,
        running: false,
        window: None,
    });

    /// Entry point of the helper process. Blocks for as long as the clipboard source takes to answer.
    pub fn run_probe() {
        autoreleasepool(|_| unsafe {
            let pasteboard: *mut AnyObject =
                msg_send![objc2::class!(NSPasteboard), generalPasteboard];
            if pasteboard.is_null() {
                return;
            }
            let items: *mut AnyObject = msg_send![pasteboard, pasteboardItems];
            if items.is_null() {
                return;
            }
            let count: usize = msg_send![items, count];
            for index in 0..count {
                let item: *mut AnyObject = msg_send![items, objectAtIndex: index];
                let types: *mut AnyObject = msg_send![item, types];
                if types.is_null() {
                    continue;
                }
                for name in PASTE_TYPES {
                    let name = NSString::from_str(name);
                    let offered: bool = msg_send![types, containsObject: &*name];
                    if offered {
                        let _: *mut AnyObject = msg_send![item, dataForType: &*name];
                    }
                }
            }
        })
    }

    /// Runs the helper under the timeout. Only a timeout counts as not ready: if the helper cannot be run
    /// at all, paste proceeds as it would without this gate rather than losing the feature.
    pub fn probe_is_ready() -> bool {
        if LAST_TIMEOUT
            .lock()
            .unwrap()
            .is_some_and(|at| at.elapsed() < FAILURE_MEMORY)
        {
            return false;
        }
        let Ok(exe) = std::env::current_exe() else {
            return true;
        };
        let Ok(mut helper) = Command::new(exe)
            .arg("--clipboard-probe")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return true;
        };
        let started = Instant::now();
        loop {
            match helper.try_wait() {
                Ok(None) if started.elapsed() < PROBE_TIMEOUT => {
                    std::thread::sleep(POLL_INTERVAL)
                }
                Ok(None) => {
                    let _ = helper.kill();
                    let _ = helper.wait();
                    *LAST_TIMEOUT.lock().unwrap() = Some(Instant::now());
                    crate::diagnostics::record(
                        "WARN",
                        "clipboard_source_unresponsive",
                        json!({}),
                    );
                    return false;
                }
                _ => return true,
            }
        }
    }

    /// Handles a paste command from the native menu. Called on the main thread and never blocks it. Presses
    /// arriving during a probe are counted so none is lost; they are abandoned together if the source stalls.
    pub fn request_paste<R: Runtime>(app: &AppHandle<R>, window: Option<String>) {
        {
            let mut pending = PENDING.lock().unwrap();
            pending.presses += 1;
            pending.window = window;
            if pending.running {
                return;
            }
            pending.running = true;
        }
        let app = app.clone();
        std::thread::spawn(move || loop {
            let (presses, window) = {
                let mut pending = PENDING.lock().unwrap();
                if pending.presses == 0 {
                    pending.running = false;
                    return;
                }
                (
                    std::mem::take(&mut pending.presses),
                    pending.window.clone(),
                )
            };
            if probe_is_ready() {
                let _ = app.run_on_main_thread(move || {
                    for _ in 0..presses {
                        send_paste_action();
                    }
                });
            } else {
                let _ = crate::native_menu::dispatch(
                    &app,
                    window.as_deref(),
                    "clipboard-unresponsive",
                );
            }
        });
    }

    /// Sends `paste:` down the responder chain, exactly what the predefined menu item does, so the focused
    /// control receives it whichever window or embedded browser view holds focus.
    fn send_paste_action() {
        unsafe {
            let app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
            let nil: *mut AnyObject = std::ptr::null_mut();
            let _: bool = msg_send![app, sendAction: objc2::sel!(paste:), to: nil, from: nil];
        }
    }
}
