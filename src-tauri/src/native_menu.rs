//! Native menu commands belong to the focused window, including SSH and paired-URL windows.

use tauri::{AppHandle, Emitter, Runtime};

pub fn dispatch<R: Runtime>(
    app: &AppHandle<R>,
    focused_label: Option<&str>,
    action: &str,
) -> tauri::Result<()> {
    // Window::emit also broadcasts globally. Explicitly target the label, and never broadcast when
    // AppKit has no focused window (for example while a native dialog owns focus).
    if let Some(label) = focused_label {
        app.emit_to(label, "menu://action", action)?;
    }
    Ok(())
}

#[cfg(all(test, feature = "native-menu-tests"))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use tauri::{Listener, WebviewWindowBuilder};

    #[test]
    fn menu_commands_reach_only_the_selected_window() {
        let app = tauri::test::mock_app();
        let (tx, rx) = mpsc::channel();
        let labels = ["main", "remote-ssh", "remote-url"];
        let mut windows = Vec::new();
        for label in labels {
            let window = WebviewWindowBuilder::new(&app, label, Default::default())
                .build()
                .unwrap();
            let tx = tx.clone();
            window.listen("menu://action", move |event| {
                tx.send((label, event.payload().to_owned())).unwrap();
            });
            windows.push(window);
        }
        for label in labels {
            for action in ["split-right", "split-down", "settings"] {
                dispatch(app.handle(), Some(label), action).unwrap();
                assert_eq!(
                    rx.try_iter().collect::<Vec<_>>(),
                    vec![(label, serde_json::to_string(action).unwrap())]
                );
            }
        }
        // Missing or stale focus must not deliver to any other window.
        dispatch(app.handle(), None, "split-right").unwrap();
        dispatch(app.handle(), Some("closed-window"), "split-down").unwrap();
        assert!(rx.try_recv().is_err());
    }
}
