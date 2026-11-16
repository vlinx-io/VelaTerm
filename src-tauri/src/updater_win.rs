//! Windows self-update installed from VelaTerm's own data folder.
//!
//! Tauri's updater plugin writes the downloaded installer to `%TEMP%\<app>-<version>-updater-<random>\`, keeps
//! that folder forever, starts the installer and exits, so every update leaves its full installer behind. A
//! program that drops an executable into Temp and runs it is also what antivirus behavior engines watch for
//! (issue #144). VelaTerm therefore downloads through the plugin, which still verifies the minisign signature,
//! writes the installer to `<app local data>\updates\`, starts it with the same arguments the plugin would use,
//! and removes old installers from that folder and the plugin's leftover Temp folders at startup.
//!
//! The rest of the update flow (check, release notes, progress UI) stays on the plugin's JS API; only the
//! final download-and-install step on Windows is routed here.

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, ResourceId, Webview};

/// Folder under the app's local data directory that holds a pending installer.
const UPDATES_DIR: &str = "updates";
/// Leftovers younger than this may belong to an installer that is still finishing, so cleanup skips them.
#[cfg_attr(not(windows), allow(dead_code))]
const MIN_AGE: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Progress events in the plugin's wire format, so the frontend's existing handler reads them unchanged.
#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started { content_length: Option<u64> },
    #[serde(rename_all = "camelCase")]
    Progress { chunk_length: usize },
    Finished,
}

fn updates_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("failed to resolve the local data directory: {e}"))?;
    Ok(base.join(UPDATES_DIR))
}

/// Whether this copy runs from the portable zip rather than an NSIS installation. The installer always writes
/// `uninstall.exe` beside `velaterm.exe`; the zip (and a bare release build) has none. Running the installer
/// from such a copy would install a second VelaTerm elsewhere instead of updating it, so the frontend offers
/// the download page instead. Async because it touches the file system.
#[tauri::command]
pub async fn updater_windows_portable() -> bool {
    if !cfg!(windows) {
        return false;
    }
    tauri::async_runtime::spawn_blocking(|| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| !dir.join("uninstall.exe").is_file()))
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

/// Download the update behind the plugin resource `rid`, verify it, install it from VelaTerm's own folder and
/// exit. Only returns on failure. Async with blocking file IO moved off the runtime: the installer is ~150 MB.
#[tauri::command]
pub async fn updater_install_windows(
    app: AppHandle,
    webview: Webview,
    rid: ResourceId,
    on_event: Channel<DownloadEvent>,
) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("This update path is only used on Windows".to_string());
    }
    let update = webview
        .resources_table()
        .get::<tauri_plugin_updater::Update>(rid)
        .map_err(|e| format!("Update is no longer available: {e}"))?;

    let mut first_chunk = true;
    let bytes = update
        .download(
            |chunk_length, content_length| {
                if first_chunk {
                    first_chunk = false;
                    let _ = on_event.send(DownloadEvent::Started { content_length });
                }
                let _ = on_event.send(DownloadEvent::Progress { chunk_length });
            },
            || {
                let _ = on_event.send(DownloadEvent::Finished);
            },
        )
        .await
        .map_err(|e| format!("Update download failed: {e}"))?;
    // Windows releases ship an NSIS installer, which is a PE executable.
    if !bytes.starts_with(b"MZ") {
        return Err("Update package is not a Windows installer".to_string());
    }

    let dir = updates_dir(&app)?;
    let file_name = format!("{}-{}-setup.exe", app.package_info().name, update.version);
    let installer = tauri::async_runtime::spawn_blocking(move || -> Result<std::path::PathBuf, String> {
        // Only one installer is ever pending; anything older is a leftover.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create the update folder: {e}"))?;
        let path = dir.join(file_name);
        std::fs::write(&path, &bytes).map_err(|e| format!("Failed to save the update: {e}"))?;
        Ok(path)
    })
    .await
    .map_err(|e| format!("Failed to save the update: {e}"))??;

    launch_and_exit(&app, &installer)
}

/// Start the NSIS installer exactly as the plugin does for `installMode: "passive"` (tauri.conf.json): passive UI
/// and restart (`/P /R`), update mode, and this process's arguments after `/ARGS` so the relaunch keeps them.
#[cfg(windows)]
fn launch_and_exit(app: &AppHandle, installer: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

    let mut parameters = vec!["/P".to_string(), "/R".to_string(), "/UPDATE".to_string(), "/ARGS".to_string()];
    parameters.extend(std::env::args_os().skip(1).map(|arg| escape_nsis_arg(&arg.to_string_lossy())));
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let file = wide(installer.as_os_str());
    let params = wide(std::ffi::OsStr::new(&parameters.join(" ")));
    let verb = wide(std::ffi::OsStr::new("open"));

    // Same order as the plugin: let Tauri run its exit cleanup, then hand over to the installer and exit.
    app.cleanup_before_exit();
    // SAFETY: every pointer refers to a NUL-terminated buffer that outlives the call.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(params.as_ptr()),
            PCWSTR::null(),
            SW_SHOW,
        )
    };
    // ShellExecuteW reports success with a value greater than 32.
    if result.0 as isize <= 32 {
        return Err(format!("Failed to start the installer (code {})", result.0 as isize));
    }
    std::process::exit(0);
}

#[cfg(not(windows))]
fn launch_and_exit(_app: &AppHandle, _installer: &std::path::Path) -> Result<(), String> {
    Err("This update path is only used on Windows".to_string())
}

/// Quote one argument for the installer's `/ARGS` list. Same rules as the updater plugin's
/// `escape_nsis_current_exe_arg`: standard Windows quoting, and `/` also forces quotes so NSIS does not read
/// the argument as one of its own switches.
#[cfg_attr(not(windows), allow(dead_code))]
fn escape_nsis_arg(arg: &str) -> String {
    let quote = arg.is_empty() || arg.chars().any(|c| c == ' ' || c == '\t' || c == '/');
    let mut out = String::new();
    if quote {
        out.push('"');
    }
    let mut backslashes = 0usize;
    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
        } else {
            if c == '"' {
                // n backslashes before a quote become 2n + 1.
                out.extend(std::iter::repeat_n('\\', backslashes + 1));
            }
            backslashes = 0;
        }
        out.push(c);
    }
    if quote {
        // n trailing backslashes before the closing quote become 2n.
        out.extend(std::iter::repeat_n('\\', backslashes));
        out.push('"');
    }
    out
}

/// Remove installers left by earlier updates: VelaTerm's own `updates` folder and the plugin's Temp folders
/// (`<app>-<version>-updater-*`) from versions that still used it. Runs on a background thread at startup;
/// failures, such as a file an antivirus still holds, are only logged and retried on the next start.
pub fn cleanup_leftovers(app: &AppHandle) {
    if !cfg!(windows) {
        return;
    }
    let own = updates_dir(app).ok();
    let prefix = format!("{}-", app.package_info().name);
    std::thread::Builder::new()
        .name("updater-cleanup".into())
        .spawn(move || {
            if let Some(dir) = own {
                remove_children_older_than(&dir, |_| true);
            }
            remove_children_older_than(&std::env::temp_dir(), |name| {
                name.starts_with(&prefix) && name.contains("-updater-")
            });
        })
        .ok();
}

/// Whether an entry is old enough to be a finished leftover. An unknown age counts as fresh.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_stale(age: Option<std::time::Duration>) -> bool {
    age.is_some_and(|age| age >= MIN_AGE)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn remove_children_older_than(dir: &std::path::Path, matches: impl Fn(&str) -> bool) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !matches(&name) {
            continue;
        }
        let age = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
        if !is_stale(age) {
            continue;
        }
        let path = entry.path();
        let removed = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
        if let Err(e) = removed {
            crate::diagnostic_warn!("failed to remove a leftover update installer: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nsis_args_are_quoted_like_the_updater_plugin() {
        assert_eq!(escape_nsis_arg("plain"), "plain");
        assert_eq!(escape_nsis_arg(""), "\"\"");
        assert_eq!(escape_nsis_arg("C:\\Program Files\\x"), "\"C:\\Program Files\\x\"");
        // `/` forces quotes so NSIS does not take it for a switch.
        assert_eq!(escape_nsis_arg("/S"), "\"/S\"");
        assert_eq!(escape_nsis_arg("a\"b"), "a\\\"b");
        assert_eq!(escape_nsis_arg("dir with space\\"), "\"dir with space\\\\\"");
    }

    #[test]
    fn only_entries_past_the_guard_are_stale() {
        assert!(!is_stale(None));
        assert!(!is_stale(Some(std::time::Duration::from_secs(5))));
        assert!(!is_stale(Some(MIN_AGE - std::time::Duration::from_secs(1))));
        assert!(is_stale(Some(MIN_AGE)));
    }

    #[test]
    fn cleanup_keeps_fresh_and_unrelated_entries() {
        let root = std::env::temp_dir().join(format!("vlx-updater-cleanup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("VelaTerm-0.2.8-updater-abc")).unwrap();
        std::fs::create_dir_all(root.join("unrelated")).unwrap();
        // Everything was just created, so an installer that may still be finishing is left alone.
        remove_children_older_than(&root, |name| name.starts_with("VelaTerm-") && name.contains("-updater-"));
        assert!(root.join("VelaTerm-0.2.8-updater-abc").exists());
        assert!(root.join("unrelated").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
