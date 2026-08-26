//! Runtime host abstraction that decouples the event bus, state container, and data directory from
//! `tauri::AppHandle`.
//!
//! Shared `pty/`, `agent/`, and `web/` code accepts only `AppCtx` rather than AppHandle:
//! - **Desktop GUI**: `AppCtx::Tauri` forwards events, state, and paths through AppHandle, preserving
//!   delivery to the frontend WebView and `app.listen` subscribers.
//! - **Headless `--serve` CLI**: `AppCtx::Headless` owns manually constructed Db, PtyManager, and
//!   HookServer state, an in-process subscriber bus, and an explicit data directory. With no window or
//!   Tauri event loop, events are delivered only to forwarding listeners registered by `web/ws.rs`.
//!
//! Payload semantics match Tauri: emit serializes to a JSON string and listeners receive that exact
//! string, equivalent to `Event::payload()`, so `web/ws.rs` forwards both host variants identically.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

// Tauri imports are GUI-only; the minimal server uses only the Headless branch.
#[cfg(feature = "gui")]
use tauri::{Emitter, Listener, Manager};

use crate::agent::server::HookServer;
use crate::db::Db;
use crate::pty::manager::PtyManager;
use crate::web::WebServer;

/// Global event emitted with an empty payload after any successful tree mutation, including import,
/// create, delete, rename, move, configuration, collapse, or archive. Clients debounce and reload the
/// tree to synchronize sidebars. Both desktop commands and browser WebSocket dispatch must emit it.
pub const TREE_CHANGED: &str = "tree://changed";

/// Global event emitted with an empty payload after any agent-preset mutation. Presets appear in the
/// new-session menu on every client, so each one reloads its list on this signal. Kept separate from
/// `TREE_CHANGED` because presets live outside the session tree and change far less often.
pub const PRESETS_CHANGED: &str = "presets://changed";

/// Global event emitted after any successful `app_settings` write, carrying **only the key names** that
/// were written (`Vec<String>`), never their values. `remoteAccess.*` and `gitea.token` are protected
/// keys that `web/dispatch.rs` strips from a remote client's `get_app_settings` reply; shipping values
/// in a broadcast would walk straight around that filter. Key names let each client re-read through the
/// normal path, so the filter still applies. Clients reconcile their local preference cache on receipt.
pub const SETTINGS_CHANGED: &str = "settings://changed";

/// Cross-platform home directory: `$HOME` on Unix and `%USERPROFILE%` or known-folder APIs on Windows.
///
/// Use this for agent data directories such as `~/.claude`, `~/.codex`, and `~/.cursor` instead of
/// reading HOME directly, which is commonly unset on Windows. Delegates to cross-platform
/// `dirs::home_dir()`.
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Create a cross-platform `Command`, adding `CREATE_NO_WINDOW` on Windows so this GUI-subsystem binary
/// does not flash a conhost window for every console child. Other platforms use plain `Command::new`.
///
/// Route repeatedly spawned runtime processes such as Git probes, worktrees, and OpenCode verification
/// through this helper. Platform-gated `lsof`/`ps` commands and test-only processes are unaffected.
pub fn command<S: AsRef<OsStr>>(program: S) -> Command {
    // `mut` is used only by the Windows creation_flags branch.
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // External tools run under the AppImage's rewritten environment would resolve libraries and
    // interpreters inside the bundle. Restore the system environment; a no-op off Linux.
    crate::appimage::scrub_command(&mut cmd);
    cmd
}

/// Unified runtime host entry point for desktop Tauri and headless operation.
#[derive(Clone)]
pub enum AppCtx {
    /// Desktop GUI forwarding managed state, events, and paths through AppHandle.
    #[cfg(feature = "gui")]
    Tauri(tauri::AppHandle),
    /// Headless CLI backed by the in-process host described in the module documentation.
    Headless(Arc<HeadlessHost>),
}

/// In-process headless equivalents of Tauri's GUI managed state.
pub struct HeadlessHost {
    data_dir: PathBuf,
    db: Db,
    pty: PtyManager,
    /// HookServer::start itself needs AppCtx to emit and write the database, so construct the host first
    /// and fill this after startup. No hook traffic can access it beforehand.
    hooks: OnceLock<HookServer>,
    events: EventBus,
    /// LAN remote-access instance binding `0.0.0.0` with TLS. It is constructed but not started, then
    /// controlled by `web_server_start/stop`. This is separate from `serve_main`'s persistent plaintext
    /// loopback instance, which is essential to the Electron renderer and must never be shared.
    remote_web: WebServer,
}

impl HeadlessHost {
    pub fn new(data_dir: PathBuf, db: Db) -> Self {
        Self {
            data_dir,
            db,
            pty: PtyManager::new(),
            hooks: OnceLock::new(),
            events: EventBus::new(),
            remote_web: WebServer::new(),
        }
    }

    /// Store the started HookServer exactly once.
    pub fn set_hooks(&self, hooks: HookServer) {
        if self.hooks.set(hooks).is_err() {
            unreachable!("HookServer should be filled in only once");
        }
    }
}

/// Listener handle returned by listen and passed back to unlisten.
pub enum ListenerId {
    #[cfg(feature = "gui")]
    Tauri(tauri::EventId),
    Headless(u64),
}

impl AppCtx {
    /// Application data directory from Tauri's identifier-based path API or the headless startup path.
    pub fn data_dir(&self) -> Result<PathBuf, String> {
        match self {
            #[cfg(feature = "gui")]
            AppCtx::Tauri(app) => app
                .path()
                .app_data_dir()
                .map_err(|e| format!("Failed to get app data directory: {e}")),
            AppCtx::Headless(host) => Ok(host.data_dir.clone()),
        }
    }

    pub fn db(&self) -> &Db {
        match self {
            #[cfg(feature = "gui")]
            AppCtx::Tauri(app) => app.state::<Db>().inner(),
            AppCtx::Headless(host) => &host.db,
        }
    }

    pub fn pty(&self) -> &PtyManager {
        match self {
            #[cfg(feature = "gui")]
            AppCtx::Tauri(app) => app.state::<PtyManager>().inner(),
            AppCtx::Headless(host) => &host.pty,
        }
    }

    pub fn hooks(&self) -> &HookServer {
        match self {
            #[cfg(feature = "gui")]
            AppCtx::Tauri(app) => app.state::<HookServer>().inner(),
            AppCtx::Headless(host) => host
                .hooks
                .get()
                .expect("HookServer has not been filled in yet (wrong startup order)"),
        }
    }

    /// LAN remote-access instance. In GUI mode this is the Tauri-managed WebServer used by desktop
    /// commands; in headless mode it is the host's on-demand secondary instance, not serve_main's
    /// persistent plaintext loopback server.
    pub fn remote_web(&self) -> &WebServer {
        match self {
            #[cfg(feature = "gui")]
            AppCtx::Tauri(app) => app.state::<WebServer>().inner(),
            AppCtx::Headless(host) => &host.remote_web,
        }
    }

    /// Emit an event to the frontend WebView and app listeners in GUI mode, or to the in-process
    /// subscriber table used by `web/ws.rs` in headless mode.
    pub fn emit<S: serde::Serialize + Clone>(&self, event: &str, payload: S) {
        // Cache the latest pty://status/* value by signal kind in PtyManager so late-attaching browser
        // clients and hot-reloaded desktop clients receive a snapshot during attach. The same pass feeds
        // the authoritative session record: it needs the *previous* state to tell a real transition from
        // a repeat, so read the cache before overwriting it.
        if let Some(sid) = event.strip_prefix("pty://status/") {
            if let Ok(value) = serde_json::to_value(payload.clone()) {
                let previous = match self {
                    #[cfg(feature = "gui")]
                    AppCtx::Tauri(app) => app
                        .try_state::<PtyManager>()
                        .and_then(|pty| pty.cached_state_signal(sid)),
                    AppCtx::Headless(host) => host.pty.cached_state_signal(sid),
                };
                match self {
                    #[cfg(feature = "gui")]
                    AppCtx::Tauri(app) => {
                        if let Some(pty) = app.try_state::<PtyManager>() {
                            pty.cache_status(sid, &value);
                        }
                    }
                    AppCtx::Headless(host) => host.pty.cache_status(sid, &value),
                }
                crate::session_state::observe_status(self, sid, &value, previous.as_ref());
            }
        }
        match self {
            #[cfg(feature = "gui")]
            AppCtx::Tauri(app) => {
                let _ = app.emit(event, payload);
            }
            AppCtx::Headless(host) => {
                let json = match serde_json::to_string(&payload) {
                    Ok(s) => s,
                    Err(_) => return,
                };
                host.events.emit(event, &json);
            }
        }
    }

    /// Listen to an exact event name and receive its JSON payload string, matching Tauri Event payloads.
    pub fn listen(
        &self,
        event: &str,
        handler: impl Fn(&str) + Send + Sync + 'static,
    ) -> ListenerId {
        match self {
            #[cfg(feature = "gui")]
            AppCtx::Tauri(app) => {
                ListenerId::Tauri(app.listen(event.to_string(), move |ev| handler(ev.payload())))
            }
            AppCtx::Headless(host) => {
                ListenerId::Headless(host.events.listen(event, Arc::new(handler)))
            }
        }
    }

    pub fn unlisten(&self, id: ListenerId) {
        match (self, id) {
            #[cfg(feature = "gui")]
            (AppCtx::Tauri(app), ListenerId::Tauri(eid)) => app.unlisten(eid),
            (AppCtx::Headless(host), ListenerId::Headless(hid)) => host.events.unlisten(hid),
            // Ignore an impossible host/handle mismatch. Only GUI builds need this multi-variant fallback;
            // the minimal server has only Headless, where a wildcard would trigger an unreachable warning.
            #[cfg(feature = "gui")]
            _ => {}
        }
    }
}

type Handler = Arc<dyn Fn(&str) + Send + Sync>;

/// In-process event bus keyed by event name, equivalent to app.emit/listen for headless mode.
struct EventBus {
    next_id: AtomicU64,
    subs: Mutex<HashMap<String, Vec<(u64, Handler)>>>,
}

impl EventBus {
    fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            subs: Mutex::new(HashMap::new()),
        }
    }

    fn listen(&self, event: &str, handler: Handler) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.subs
            .lock()
            .unwrap()
            .entry(event.to_string())
            .or_default()
            .push((id, handler));
        id
    }

    fn unlisten(&self, id: u64) {
        let mut subs = self.subs.lock().unwrap();
        for handlers in subs.values_mut() {
            handlers.retain(|(hid, _)| *hid != id);
        }
        subs.retain(|_, v| !v.is_empty());
    }

    /// Clone matching handlers before invoking them so callbacks run unlocked and may emit or listen.
    fn emit(&self, event: &str, payload_json: &str) {
        let handlers: Vec<Handler> = {
            let subs = self.subs.lock().unwrap();
            match subs.get(event) {
                Some(v) => v.iter().map(|(_, h)| Arc::clone(h)).collect(),
                None => return,
            }
        };
        for h in handlers {
            h(payload_json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn event_bus_listen_emit_unlisten() {
        let bus = EventBus::new();
        let hits = Arc::new(Mutex::new(Vec::<String>::new()));

        let hits_a = Arc::clone(&hits);
        let id = bus.listen(
            "pty://status/s1",
            Arc::new(move |p: &str| hits_a.lock().unwrap().push(p.to_string())),
        );

        // A matching listener receives the original JSON payload.
        bus.emit("pty://status/s1", r#"{"type":"bell"}"#);
        assert_eq!(hits.lock().unwrap().as_slice(), [r#"{"type":"bell"}"#]);

        // Distinct event names remain isolated.
        bus.emit("pty://status/s2", "{}");
        assert_eq!(hits.lock().unwrap().len(), 1);

        // Delivery stops after unlisten.
        bus.unlisten(id);
        bus.emit("pty://status/s1", "{}");
        assert_eq!(hits.lock().unwrap().len(), 1);
    }

    #[test]
    fn event_bus_multiple_listeners_same_event() {
        let bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));

        let c1 = Arc::clone(&count);
        let id1 = bus.listen(
            "e",
            Arc::new(move |_: &str| {
                c1.fetch_add(1, Ordering::SeqCst);
            }),
        );
        let c2 = Arc::clone(&count);
        let _id2 = bus.listen(
            "e",
            Arc::new(move |_: &str| {
                c2.fetch_add(1, Ordering::SeqCst);
            }),
        );

        bus.emit("e", "null");
        assert_eq!(count.load(Ordering::SeqCst), 2);

        // Removing one listener leaves the other active.
        bus.unlisten(id1);
        bus.emit("e", "null");
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }
}
