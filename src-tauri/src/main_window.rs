//! Local main-window lifecycle while remote windows are open.
//!
//! Someone working entirely remotely has no use for the local main window, yet closing it used to quit the
//! whole application and take every remote window with it. While a remote window is open, closing `main`
//! therefore only hides it: the webview and all local sessions stay alive in the background. The window
//! comes back on a Dock click, before a quit prompt (the prompt lives in its webview), and when the last
//! remote window closes, so the process is never left without a window.

use tauri::{AppHandle, Manager, Runtime};

/// Label of the local main window, defined in tauri.conf.json.
pub const MAIN_LABEL: &str = "main";
/// Label prefix of a paired-URL remote window (`open_remote_window`).
pub const URL_REMOTE_PREFIX: &str = "remote-";
/// Label prefix of an account-linked remote window (`open_account_remote_window`).
pub const ACCOUNT_REMOTE_PREFIX: &str = "account-remote-";
/// Label prefix of an SSH remote window (`open_login_window`).
pub const SSH_REMOTE_PREFIX: &str = "ssh-";

/// The single classification of a window label as a remote window. Window creation builds its labels from
/// the prefixes above, so the two cannot drift apart.
pub fn is_remote_window_label(label: &str) -> bool {
    [URL_REMOTE_PREFIX, ACCOUNT_REMOTE_PREFIX, SSH_REMOTE_PREFIX]
        .iter()
        .any(|prefix| label.starts_with(prefix))
}

/// Whether `labels` contain a remote window other than `except`. `except` names a window whose destruction
/// is being handled: Tauri unregisters it before delivering the event, but an explicit exclusion keeps the
/// decision independent of that ordering.
pub fn remote_window_among<'a>(
    labels: impl IntoIterator<Item = &'a str>,
    except: Option<&str>,
) -> bool {
    labels
        .into_iter()
        .any(|label| is_remote_window_label(label) && Some(label) != except)
}

/// Whether destroying the window `destroyed` leaves the application without any remote window, which is
/// when a hidden main window has to come back.
pub fn last_remote_window_closed<'a>(
    destroyed: &str,
    remaining: impl IntoIterator<Item = &'a str>,
) -> bool {
    is_remote_window_label(destroyed) && !remote_window_among(remaining, Some(destroyed))
}

fn open_labels<R: Runtime>(app: &AppHandle<R>) -> Vec<String> {
    app.webview_windows().into_keys().collect()
}

/// Handle a close request on the main window: while a remote window is open, hide `main` instead of
/// quitting and return true. False means the caller keeps the regular quit confirmation, including the
/// case where hiding fails, so the window never becomes unclosable.
pub fn hide_main_for_remote<R: Runtime>(app: &AppHandle<R>) -> bool {
    let labels = open_labels(app);
    if !remote_window_among(labels.iter().map(String::as_str), None) {
        return false;
    }
    app.get_webview_window(MAIN_LABEL)
        .is_some_and(|win| win.hide().is_ok())
}

/// Show and focus the main window if it is hidden. A visible main window is left alone, so this never
/// steals focus from a remote window the user is working in.
pub fn reveal_hidden_main<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window(MAIN_LABEL) else {
        return;
    };
    if win.is_visible().unwrap_or(true) {
        return;
    }
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
}

/// Bring back a hidden main window once the last remote window is destroyed, so the process never keeps
/// running without any window.
pub fn on_window_destroyed<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let labels = open_labels(app);
    if last_remote_window_closed(label, labels.iter().map(String::as_str)) {
        reveal_hidden_main(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_exactly_the_remote_window_labels() {
        for label in [
            "remote-1a2b3c4d",
            "account-remote-0123456789abcdef0123456789abcdef",
            "ssh-1a2b3c4d",
        ] {
            assert!(is_remote_window_label(label), "{label} is a remote window");
        }
        for label in [
            "main",
            "",
            "remote",
            "ssh",
            "download-remote-test",
            "Remote-1a2b",
        ] {
            assert!(
                !is_remote_window_label(label),
                "{label} is not a remote window"
            );
        }
    }

    #[test]
    fn closing_main_hides_only_while_a_remote_window_is_open() {
        // Only main (or main plus non-remote windows): keep the quit confirmation.
        assert!(!remote_window_among(["main"], None));
        assert!(!remote_window_among(["main", "download-remote-test"], None));
        assert!(!remote_window_among([], None));
        // Any kind of remote window keeps the application running with main hidden.
        assert!(remote_window_among(["main", "remote-1a2b3c4d"], None));
        assert!(remote_window_among(["main", "ssh-1a2b3c4d"], None));
        assert!(remote_window_among(["account-remote-0123", "main"], None));
    }

    #[test]
    fn main_returns_only_after_the_last_remote_window_closes() {
        // Another remote window is still open.
        assert!(!last_remote_window_closed("remote-1", ["main", "ssh-2"]));
        assert!(!last_remote_window_closed(
            "remote-1",
            ["main", "remote-1", "account-remote-3"]
        ));
        // The destroyed window was the last one, whether or not it is still registered.
        assert!(last_remote_window_closed("ssh-2", ["main"]));
        assert!(last_remote_window_closed("ssh-2", ["main", "ssh-2"]));
        // A non-remote window never triggers the reveal, even with no remote window left.
        assert!(!last_remote_window_closed("download-remote-test", ["main"]));
        assert!(!last_remote_window_closed("main", []));
    }
}

// Tauri's mock runtime needs the `native-menu-tests` feature:
// cargo test --manifest-path src-tauri/Cargo.toml --features native-menu-tests main_window
// Its windows report `is_visible() == true` unconditionally, so these tests cover the decisions against real
// registered windows; the hide/show effect itself is not observable there.
#[cfg(all(test, feature = "native-menu-tests"))]
mod behavior_tests {
    use super::*;
    use tauri::WebviewWindowBuilder;

    fn window<R: Runtime>(app: &AppHandle<R>, label: &str) -> tauri::WebviewWindow<R> {
        WebviewWindowBuilder::new(app, label, Default::default())
            .build()
            .unwrap()
    }

    #[test]
    fn close_decision_follows_the_registered_windows() {
        let app = tauri::test::mock_app();
        let _main = window(app.handle(), MAIN_LABEL);
        let _other = window(app.handle(), "download-remote-test");
        assert!(!hide_main_for_remote(app.handle()));

        let ssh = window(app.handle(), "ssh-1a2b3c4d");
        assert!(hide_main_for_remote(app.handle()));
        assert!(!last_remote_window_closed(
            "remote-9",
            open_labels(app.handle()).iter().map(String::as_str)
        ));
        assert!(last_remote_window_closed(
            ssh.label(),
            open_labels(app.handle()).iter().map(String::as_str)
        ));
    }

    #[test]
    fn close_decision_without_a_main_window_falls_back_to_quit() {
        let app = tauri::test::mock_app();
        let _url = window(app.handle(), "remote-1a2b3c4d");
        // Nothing to hide: the caller must keep the quit confirmation.
        assert!(!hide_main_for_remote(app.handle()));
    }
}
