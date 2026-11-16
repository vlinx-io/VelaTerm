//! Absolute paths of files dropped from the OS file manager onto the main window.
//!
//! The main window keeps Tauri's native drag/drop handler disabled because that handler swallows every HTML5
//! drag, which would break sidebar reordering and pane splitting. The WebView therefore receives external
//! drops as ordinary HTML5 events, whose `File` objects never expose a path. Each platform recovers the paths
//! of the current drop through its own native channel, and the frontend asks for them from its `drop` handler:
//!
//! - macOS reads the system drag pasteboard, which still holds the drop's file URLs when the event fires.
//! - Linux records the URI list WebKitGTK receives through GTK's `drag-data-received` before the DOM drop.
//! - Windows receives the dropped `File` objects from `chrome.webview.postMessageWithAdditionalObjects`,
//!   which WebView2 hands to the host with their paths.
//!
//! Each path comes back with whether it is a directory: the sidebar adds dropped folders as projects, the
//! file tree copies files and folders into the project, and terminals and the chat composer insert the paths.

use std::path::PathBuf;

/// One path of the drop currently being handled by the frontend.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedPath {
    pub path: String,
    pub is_dir: bool,
}

/// Returns the paths of the drop currently being handled by the frontend.
///
/// Async per the IO rule: the pasteboard read, the Windows hand-off wait, and the `is_dir` checks all
/// run on the blocking pool.
#[tauri::command]
pub async fn take_dropped_paths() -> Result<Vec<DroppedPath>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        platform::take()
            .into_iter()
            .filter(|p| p.exists())
            .map(|p| DroppedPath { is_dir: p.is_dir(), path: p.to_string_lossy().into_owned() })
            .collect()
    })
    .await
    .map_err(|e| format!("Failed to read dropped paths: {e}"))
}

/// Refuses every `file://` navigation in every webview.
///
/// With Tauri's drag/drop handler disabled, a file dropped where the page does not cancel the drop gets the
/// WebView's default action: the window navigates to that file and the interface is replaced by it. The
/// frontend cancels unhandled file drops itself; this is the backstop for any path that slips past it, such
/// as a drop landing before the page has loaded. No app page, artifact, or browser tab loads `file://`.
pub fn nav_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("file-nav-guard")
        .on_navigation(|_webview, url| url.scheme() != "file")
        .build()
}

/// Attaches the native listener that captures dropped paths, where the platform needs one.
pub fn install(win: &tauri::WebviewWindow) {
    platform::install(win);
}

/// Converts a `file://` URI into a local path; other schemes are ignored.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn path_from_file_uri(uri: &str) -> Option<PathBuf> {
    url::Url::parse(uri.trim()).ok().filter(|u| u.scheme() == "file")?.to_file_path().ok()
}

/// Paths captured by a native listener and not yet taken by the frontend.
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod stash {
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// A drop is taken within milliseconds; anything older belongs to a drag the frontend never consumed.
    const MAX_AGE: Duration = Duration::from_secs(10);

    static LATEST: Mutex<Option<(Instant, Vec<PathBuf>)>> = Mutex::new(None);

    /// Replaces the stash with the paths of the latest drag.
    pub fn put(paths: Vec<PathBuf>) {
        *LATEST.lock().unwrap() = Some((Instant::now(), paths));
    }

    /// Takes the stashed paths if they are recent enough, leaving the stash empty.
    pub fn take() -> Option<Vec<PathBuf>> {
        let (at, paths) = LATEST.lock().unwrap().take()?;
        (at.elapsed() <= MAX_AGE).then_some(paths)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::PathBuf;

    use objc2::msg_send;
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {
        static NSPasteboardNameDrag: &'static NSString;
    }

    /// The drag pasteboard needs no listener: it is read directly when the frontend asks.
    pub fn install(_win: &tauri::WebviewWindow) {}

    /// Reads the file URLs of the current drag from the system drag pasteboard.
    ///
    /// Finder writes file reference URLs (`file:///.file/id=…`), so each URL is resolved through NSURL's
    /// `filePathURL` rather than parsed as a plain path.
    pub fn take() -> Vec<PathBuf> {
        autoreleasepool(|_| unsafe {
            let mut paths = Vec::new();
            let pasteboard: *mut AnyObject =
                msg_send![objc2::class!(NSPasteboard), pasteboardWithName: NSPasteboardNameDrag];
            if pasteboard.is_null() {
                return paths;
            }
            let items: *mut AnyObject = msg_send![pasteboard, pasteboardItems];
            if items.is_null() {
                return paths;
            }
            let file_url_type = NSString::from_str("public.file-url");
            let count: usize = msg_send![items, count];
            for index in 0..count {
                let item: *mut AnyObject = msg_send![items, objectAtIndex: index];
                let url: Option<Retained<NSString>> = msg_send![item, stringForType: &*file_url_type];
                let Some(url) = url else { continue };
                let url: *mut AnyObject = msg_send![objc2::class!(NSURL), URLWithString: &*url];
                if url.is_null() {
                    continue;
                }
                let file_url: *mut AnyObject = msg_send![url, filePathURL];
                if file_url.is_null() {
                    continue;
                }
                let path: Option<Retained<NSString>> = msg_send![file_url, path];
                if let Some(path) = path {
                    paths.push(PathBuf::from(path.to_string()));
                }
            }
            paths
        })
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::path::PathBuf;

    use gtk::prelude::WidgetExt;

    use super::{path_from_file_uri, stash};

    /// Records the URI list of every drag WebKitGTK receives. The handler runs before WebKit's own class
    /// handler and does not stop emission, so HTML5 drag and drop keeps working unchanged.
    pub fn install(win: &tauri::WebviewWindow) {
        let _ = win.with_webview(|webview| {
            webview.inner().connect_drag_data_received(|_, _, _, _, data, _, _| {
                let paths: Vec<PathBuf> =
                    data.uris().iter().filter_map(|uri| path_from_file_uri(uri)).collect();
                if !paths.is_empty() {
                    stash::put(paths);
                }
            });
        });
    }

    pub fn take() -> Vec<PathBuf> {
        stash::take().unwrap_or_default()
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2File, ICoreWebView2WebMessageReceivedEventArgs2,
    };
    use webview2_com::{take_pwstr, WebMessageReceivedEventHandler};
    use windows::core::{Interface, PWSTR};

    use super::stash;

    /// Marker the frontend puts in the message so unrelated web messages are ignored.
    const MESSAGE_MARKER: &str = "vlxDroppedFiles";

    /// How long `take` waits for the web message posted just before the command.
    const HANDOFF_TIMEOUT: Duration = Duration::from_millis(1500);

    /// Listens for the frontend's `postMessageWithAdditionalObjects` call carrying the dropped files.
    ///
    /// The frontend posts an object rather than a string. Tauri's own IPC listener reads messages with
    /// `TryGetWebMessageAsString`, which fails for objects, so this message never reaches Tauri's IPC.
    pub fn install(win: &tauri::WebviewWindow) {
        let _ = win.with_webview(|webview| {
            // SAFETY: with_webview runs on the main thread and wry owns the live WebView2 COM pointers.
            unsafe {
                let Ok(core) = webview.controller().CoreWebView2() else { return };
                let handler = WebMessageReceivedEventHandler::create(Box::new(|_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut json = PWSTR::null();
                    args.WebMessageAsJson(&mut json)?;
                    if !take_pwstr(json).contains(MESSAGE_MARKER) {
                        return Ok(());
                    }
                    let objects = args.cast::<ICoreWebView2WebMessageReceivedEventArgs2>()?.AdditionalObjects()?;
                    let mut count = 0u32;
                    objects.Count(&mut count)?;
                    let mut paths = Vec::new();
                    for index in 0..count {
                        let Ok(file) = objects.GetValueAtIndex(index).and_then(|o| o.cast::<ICoreWebView2File>())
                        else {
                            continue;
                        };
                        let mut path = PWSTR::null();
                        if file.Path(&mut path).is_ok() {
                            let path = take_pwstr(path);
                            if !path.is_empty() {
                                paths.push(PathBuf::from(path));
                            }
                        }
                    }
                    stash::put(paths);
                    Ok(())
                }));
                let mut token = 0i64;
                let _ = core.add_WebMessageReceived(&handler, &mut token);
            }
        });
    }

    /// Waits briefly for the web message: it travels on a different channel from the command, so the
    /// command can arrive first.
    pub fn take() -> Vec<PathBuf> {
        let deadline = Instant::now() + HANDOFF_TIMEOUT;
        loop {
            if let Some(paths) = stash::take() {
                return paths;
            }
            if Instant::now() >= deadline {
                return Vec::new();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod platform {
    use std::path::PathBuf;

    pub fn install(_win: &tauri::WebviewWindow) {}

    pub fn take() -> Vec<PathBuf> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_become_local_paths() {
        #[cfg(unix)]
        {
            assert_eq!(path_from_file_uri("file:///home/me/My%20Project"), Some(PathBuf::from("/home/me/My Project")));
            assert_eq!(path_from_file_uri("file:///tmp/a\r\n"), Some(PathBuf::from("/tmp/a")));
        }
        assert_eq!(path_from_file_uri("https://example.com/a"), None);
        assert_eq!(path_from_file_uri("not a uri"), None);
    }
}
