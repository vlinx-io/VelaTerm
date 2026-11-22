//! Per-window pending counts: updates each window's native title (`<base> (N)`) so the macOS Dock
//! window list, Window menu, and window title bars show per-window pending counts, and sets the
//! application Dock/taskbar badge to the sum across all open windows.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::{Listener, Manager};

/// Event emitted by main and remote windows whenever their pending count changes.
pub(crate) const WINDOW_BADGE_EVENT: &str = "vlx://window-badge";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowBadgePayload {
    window_label: String,
    count: u64,
}

#[derive(Default)]
pub(crate) struct WindowBadgeState(Mutex<WindowBadgeStore>);

#[derive(Default)]
struct WindowBadgeStore {
    entries: HashMap<String, WindowEntry>,
}

struct WindowEntry {
    base_title: String,
    count: u64,
    applied_title: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct UpdatePlan {
    title_change: Option<String>,
    total: u64,
}

/// Format a window's title with its pending count. Zero keeps the plain base title.
pub(crate) fn format_title(base_title: &str, count: u64) -> String {
    if count == 0 {
        base_title.to_string()
    } else {
        format!("{base_title} ({count})")
    }
}

impl WindowBadgeStore {
    fn register(&mut self, label: &str, base_title: String) {
        self.entries.insert(
            label.to_string(),
            WindowEntry {
                applied_title: Some(base_title.clone()),
                base_title,
                count: 0,
            },
        );
    }

    fn set_count(&mut self, label: &str, count: u64, fallback_base_title: Option<&str>) -> UpdatePlan {
        let entry = self.entries.entry(label.to_string()).or_insert_with(|| {
            let base = fallback_base_title
                .filter(|t| !t.is_empty())
                .unwrap_or("VelaTerm")
                .to_string();
            WindowEntry {
                applied_title: Some(base.clone()),
                base_title: base,
                count: 0,
            }
        });
        entry.count = count;
        let desired = format_title(&entry.base_title, count);
        let title_change = if entry.applied_title.as_deref() != Some(desired.as_str()) {
            entry.applied_title = Some(desired.clone());
            Some(desired)
        } else {
            None
        };
        UpdatePlan {
            title_change,
            total: self.total(),
        }
    }

    fn remove(&mut self, label: &str) -> Option<u64> {
        self.entries.remove(label)?;
        Some(self.total())
    }

    fn total(&self) -> u64 {
        self.entries
            .values()
            .fold(0u64, |acc, entry| acc.saturating_add(entry.count))
    }
}

/// Record a newly opened window's base title before any pending suffix is appended.
pub(crate) fn register(app: &tauri::AppHandle, label: &str, base_title: impl Into<String>) {
    if label.is_empty() || label == "screenshot" {
        return;
    }
    let Some(state) = app.try_state::<WindowBadgeState>() else {
        return;
    };
    if let Ok(mut store) = state.0.lock() {
        store.register(label, base_title.into());
    };
}

/// Listen for per-window pending count updates from both the main window and remote windows.
pub(crate) fn install_listener(app: &tauri::AppHandle) {
    let handle = app.clone();
    app.listen_any(WINDOW_BADGE_EVENT, move |event| {
        let Ok(payload) = serde_json::from_str::<WindowBadgePayload>(event.payload()) else {
            return;
        };
        update(&handle, &payload.window_label, payload.count);
    });
}

/// Apply one window's latest pending count, updating its title and the summed Dock badge.
pub(crate) fn update(app: &tauri::AppHandle, label: &str, count: u64) {
    if label.is_empty() || label == "screenshot" {
        return;
    }
    // Ignore stale updates emitted while a window is tearing down so destroyed labels cannot linger.
    let Some(win) = app.get_webview_window(label) else {
        return;
    };
    let Some(state) = app.try_state::<WindowBadgeState>() else {
        return;
    };
    let needs_fallback = state
        .0
        .lock()
        .map(|store| !store.entries.contains_key(label))
        .unwrap_or(false);
    let fallback_title = if needs_fallback {
        win.title().ok()
    } else {
        None
    };
    let plan = match state.0.lock() {
        Ok(mut store) => store.set_count(label, count, fallback_title.as_deref()),
        Err(_) => return,
    };

    let handle = app.clone();
    let label_owned = label.to_string();
    let _ = app.run_on_main_thread(move || {
        if let Some(title) = plan.title_change {
            if let Some(win) = handle.get_webview_window(&label_owned) {
                let _ = win.set_title(&title);
            }
        }
        apply_app_badge(&handle, plan.total);
    });
}

/// Drop a destroyed window's count and refresh the summed Dock badge for the remaining windows.
pub(crate) fn remove(app: &tauri::AppHandle, label: &str) {
    let Some(state) = app.try_state::<WindowBadgeState>() else {
        return;
    };
    let next_total = state
        .0
        .lock()
        .ok()
        .and_then(|mut store| store.remove(label));
    let Some(total) = next_total else {
        return;
    };
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        apply_app_badge(&handle, total);
    });
}

/// Update the OS application badge on the main thread.
#[cfg(target_os = "macos")]
fn apply_app_badge(_app: &tauri::AppHandle, total: u64) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;

    // Set NSDockTile.badgeLabel directly so clearing works even after the last window closes.
    unsafe {
        let ns_app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
        if ns_app.is_null() {
            return;
        }
        let dock_tile: *mut AnyObject = msg_send![ns_app, dockTile];
        if dock_tile.is_null() {
            return;
        }
        let label = if total > 0 {
            Some(NSString::from_str(&total.to_string()))
        } else {
            None
        };
        let label_ptr: *const NSString = label
            .as_deref()
            .map_or(std::ptr::null(), |s| s as *const NSString);
        let _: () = msg_send![dock_tile, setBadgeLabel: label_ptr];
    }
}

#[cfg(not(target_os = "macos"))]
fn apply_app_badge(app: &tauri::AppHandle, total: u64) {
    let badge = (total > 0).then(|| i64::try_from(total).unwrap_or(i64::MAX));
    let target = app.get_webview_window("main").or_else(|| {
        app.webview_windows()
            .into_iter()
            .find(|(label, _)| label != "screenshot")
            .map(|(_, win)| win)
    });
    if let Some(win) = target {
        let _ = win.set_badge_count(badge);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_plain_title_for_zero_and_suffix_for_positive_counts() {
        assert_eq!(format_title("VelaTerm", 0), "VelaTerm");
        assert_eq!(format_title("VelaTerm", 3), "VelaTerm (3)");
        assert_eq!(
            format_title("VelaTerm · SSH mirror: vlinx@10.10.10.66", 2),
            "VelaTerm · SSH mirror: vlinx@10.10.10.66 (2)"
        );
    }

    #[test]
    fn sums_counts_across_windows_and_deduplicates_unchanged_titles() {
        let mut store = WindowBadgeStore::default();
        store.register("main", "VelaTerm".into());
        store.register("ssh-1", "VelaTerm · SSH mirror: vlinx@10.10.10.66".into());

        assert_eq!(
            store.set_count("main", 2, None),
            UpdatePlan {
                title_change: Some("VelaTerm (2)".into()),
                total: 2,
            }
        );
        // Repeating the same count skips redundant set_title while still returning the live total.
        assert_eq!(
            store.set_count("main", 2, None),
            UpdatePlan {
                title_change: None,
                total: 2,
            }
        );
        assert_eq!(
            store.set_count("ssh-1", 4, None),
            UpdatePlan {
                title_change: Some("VelaTerm · SSH mirror: vlinx@10.10.10.66 (4)".into()),
                total: 6,
            }
        );
        // Clearing one window restores its plain base title and leaves the other window's count.
        assert_eq!(
            store.set_count("main", 0, None),
            UpdatePlan {
                title_change: Some("VelaTerm".into()),
                total: 4,
            }
        );
    }

    #[test]
    fn removing_a_window_updates_the_total_and_ignores_untracked_labels() {
        let mut store = WindowBadgeStore::default();
        store.register("main", "VelaTerm".into());
        store.register("ssh-1", "VelaTerm · SSH: vlinx@10.10.10.211".into());
        store.set_count("main", 1, None);
        store.set_count("ssh-1", 3, None);

        assert_eq!(store.remove("screenshot"), None);
        assert_eq!(store.remove("ssh-1"), Some(1));
        assert_eq!(store.remove("main"), Some(0));
        assert_eq!(store.remove("main"), None);
    }

    #[test]
    fn fallback_base_title_is_captured_once_for_unregistered_windows() {
        let mut store = WindowBadgeStore::default();
        assert_eq!(
            store.set_count("remote-1", 2, Some("VelaTerm · Remote: 10.0.0.2:8080")),
            UpdatePlan {
                title_change: Some("VelaTerm · Remote: 10.0.0.2:8080 (2)".into()),
                total: 2,
            }
        );
        // Subsequent updates keep the original base title instead of appending another suffix.
        assert_eq!(
            store.set_count("remote-1", 5, Some("ignored")),
            UpdatePlan {
                title_change: Some("VelaTerm · Remote: 10.0.0.2:8080 (5)".into()),
                total: 5,
            }
        );
    }
}
