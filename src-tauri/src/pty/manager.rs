//! PTY manager for all running sessions, keyed by session ID.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use super::monitor::{OutputScanner, ScanEvent};
use super::session::{
    OutputCoalescer, OutputSink, OutputStream, PtySession, Recorder, INPUT_QUEUE_CAP,
};
use super::{AgentState, StatusSignal};
use crate::agent::inject;
use crate::agent::server::HookEndpoint;
use crate::host::AppCtx;
use crate::models::SessionKind;

/// Result of `pty_spawn`: the child PID and an optional automatic launch command. The backend generates
/// injected commands for typed agent sessions, and the frontend writes them to the PTY using `initCmd`
/// timing. Plain Terminal sessions return no launch command and use the user's custom `initCmd`.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnResult {
    pub pid: u32,
    pub launch: Option<String>,
    /// Subscriber ID registered by this call, used by WebSocket clients to detach their own output stream.
    pub sub_id: u64,
    /// Whether this call attached to a running session rather than starting one. Attached clients must skip
    /// the `initCmd` fallback, which would otherwise execute again in an already-running shell.
    pub attached: bool,
    /// Current PTY columns: requested on launch, or the session's current value on attach. This is the only
    /// reliable initial size for mirror mode because it is independent of event/replay ordering.
    pub cols: u16,
    /// Current PTY rows, with the same launch/attach semantics as `cols`.
    pub rows: u16,
    /// Current size owner: the caller on launch, or the existing owner on attach. Clients enter mirror mode
    /// when this does not match their source ID.
    pub owner: Option<String>,
}

/// Monitor-thread polling interval.
const POLL_INTERVAL: Duration = Duration::from_millis(200);
/// Window used to classify a poll cycle as active, slightly longer than the interval to avoid edge misses.
const ACTIVE_WINDOW: Duration = Duration::from_millis(260);
/// Consecutive active cycles required to enter the working state (about 600 ms).
///
/// This hysteresis threshold accepts the sustained spinner redraws produced while agents work, but rejects
/// isolated idle redraws such as cursor blinking or status-bar refreshes, preventing state oscillation.
const WORK_ENTER_TICKS: u32 = 3;
/// Consecutive inactive cycles required to leave the working state (about 800 ms of silence).
const IDLE_LEAVE_TICKS: u32 = 4;

/// Polling interval of the silence heal.
const HEAL_POLL: Duration = Duration::from_millis(500);
/// Silence required before a hook-driven session still displaying working is healed to waiting.
///
/// While a turn runs, the Claude and Codex TUIs repaint their elapsed-time line about once per second, so
/// six seconds of complete silence means no turn is running. The margin is wide enough that a slow model
/// response cannot be mistaken for an ended turn.
const QUIET_HEAL: Duration = Duration::from_secs(6);

/// Output replay limit. Roughly 512 KB covers several full TUI redraws and recent history for new subscribers.
const SCROLLBACK_CAP: usize = 512 * 1024;

/// Per-session recording limit. At 50 MB, recording stops and marks the output as truncated. Only unusually
/// long, redraw-heavy agent sessions should reach this safety cap.
const RECORDING_CAP: u64 = 50 * 1024 * 1024;

/// Key for the frontend's main JSON settings block in `app_settings`, shared across shells. The backend reads
/// only `agentDefaults.<kind>.path`; see `PersistedSettings` and [`agent_bin_path`].
const VLX_SETTINGS_KEY: &str = "vlx-settings";

/// Shared `app_settings` key for session recording. Recording is off unless the user explicitly stores `"1"`.
const RECORD_SESSIONS_KEY: &str = "vlx-record-sessions";

/// Desktop source ID. Browser clients use their WebSocket connection IDs; spawn and resize use it to track ownership.
pub const DESKTOP_SOURCE: &str = "desktop";

/// Poll interval and timeout while another client is spawning the same session. PTY creation normally takes
/// milliseconds; timeout returns an error so the frontend can retry.
const SPAWN_WAIT_POLL: Duration = Duration::from_millis(50);
const SPAWN_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

/// Global subscriber ID used to detach one browser output stream without affecting others or killing the session.
static NEXT_SUB_ID: AtomicU64 = AtomicU64::new(1);

/// Global PTY manager injected as Tauri managed state.
///
/// `sessions` is shared with background readers so they can remove sessions at EOF and promptly release
/// master PTY handles, preventing descriptor leaks.
pub struct PtyManager {
    sessions: Arc<Mutex<HashMap<String, PtySession>>>,
    /// Session state snapshots: session ID → signal kind → latest JSON payload.
    ///
    /// Status events are ephemeral broadcasts, so late attachments and hot reloads can miss earlier agent
    /// markers. `AppCtx::emit` caches the latest `pty://status/*` payload by kind and attach replays it in
    /// `SNAPSHOT_KINDS` order.
    status_cache: Arc<Mutex<HashMap<String, HashMap<String, serde_json::Value>>>>,
    /// Reservation set for sessions that passed the absence check but have not yet entered `sessions`.
    ///
    /// PTY creation separates the absence check from insertion, so concurrent clients could launch duplicate
    /// processes. The reservation winner launches; others wait and attach once insertion completes.
    /// `SpawnSlot` releases reservations on every return path.
    spawning: Mutex<HashSet<String>>,
}

/// RAII guard that releases a concurrent-spawn reservation on every exit path.
struct SpawnSlot<'a> {
    set: &'a Mutex<HashSet<String>>,
    id: String,
}

impl Drop for SpawnSlot<'_> {
    fn drop(&mut self) {
        self.set.lock().unwrap().remove(&self.id);
    }
}

/// Snapshot signal kinds in attach replay order. `agent` must precede `state` and `busy`, while `resized`
/// comes last. Transient `bell` events are not cached; prompt-based renaming now belongs to the backend hook service.
const SNAPSHOT_KINDS: [&str; 7] = [
    "agent",
    "hook_ready",
    "title",
    "tool",
    "state",
    "busy",
    "resized",
];

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            status_cache: Arc::new(Mutex::new(HashMap::new())),
            spawning: Mutex::new(HashSet::new()),
        }
    }

    /// Caches the latest status signal for a session when `AppCtx::emit` sends it.
    pub fn cache_status(&self, sid: &str, payload: &serde_json::Value) {
        let Some(kind) = payload.get("kind").and_then(serde_json::Value::as_str) else {
            return;
        };
        if !SNAPSHOT_KINDS.contains(&kind) {
            return;
        }
        self.status_cache
            .lock()
            .unwrap()
            .entry(sid.to_string())
            .or_default()
            .insert(kind.to_string(), payload.clone());
    }

    /// Returns a session's cached status snapshot in replay order.
    pub fn status_snapshot(&self, sid: &str) -> Vec<serde_json::Value> {
        let cache = self.status_cache.lock().unwrap();
        let Some(kinds) = cache.get(sid) else {
            return Vec::new();
        };
        SNAPSHOT_KINDS
            .iter()
            .filter_map(|k| kinds.get(*k).cloned())
            .collect()
    }

    /// Returns the agent state last emitted for a session, read from the status snapshot cache.
    ///
    /// The cache holds exactly what clients last received, so background helpers can decide whether a
    /// correction is needed without keeping their own copy of the state machine.
    pub fn cached_agent_state(&self, sid: &str) -> Option<String> {
        let cache = self.status_cache.lock().unwrap();
        Some(
            cache
                .get(sid)?
                .get("state")?
                .get("state")?
                .as_str()?
                .to_string(),
        )
    }

    /// Returns whether the manager still owns a PTY session.
    ///
    /// Background helpers use this as their lifecycle condition and stop once tab closure or process exit
    /// removes the session.
    pub fn is_running(&self, sid: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(sid)
    }

    /// Starts or attaches to a PTY session and returns its PID and optional typed-session launch command.
    ///
    /// A background reader forwards raw PTY bytes through the binary `on_output` channel without encoding,
    /// then emits `pty://exit/{id}` at EOF.
    ///
    /// `kind` controls injection. Agent sessions receive in-memory environment values and an automatic
    /// launch command, relying on authoritative hook events. Terminal sessions retain heuristic monitoring.
    ///
    /// `source` identifies the caller. It becomes size owner for a new session; attaching never changes ownership.
    ///
    /// `attach_only` supports browser reconnection without launching. It errors if the session is absent,
    /// but still waits when another client is currently starting it.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &self,
        app: AppCtx,
        id: String,
        kind: SessionKind,
        shell: Option<String>,
        cwd: Option<String>,
        // UI brightness sets `COLORFGBG` for TUIs that do not query OSC 11. `None` leaves detection unchanged.
        dark: Option<bool>,
        cols: u16,
        rows: u16,
        hook: HookEndpoint,
        resume_id: Option<String>,
        fork: bool,
        init_prompt: Option<String>,
        // Custom agent launch arguments, read from session configuration and appended verbatim.
        extra_args: Option<String>,
        // Permission mode. OpenCode uses `OPENCODE_CONFIG_CONTENT`; other agents already merge their CLI flag upstream.
        permission_mode: Option<String>,
        // Resolved `light`/`dark` theme for Claude settings, bypassing unreliable OSC 11 replies through ConPTY.
        theme: Option<String>,
        source: &str,
        attach_only: bool,
        first_sink: OutputSink,
    ) -> Result<SpawnResult, String> {
        // Spawn-or-attach: attach subscribers to an existing session, replay its screen, and return no launch
        // command so a second client cannot restart the agent.
        //
        // Serialize concurrent launches with a reservation. The winner creates the PTY; other clients poll
        // until they can attach. A timeout indicates that the competing launch failed and lets the frontend retry.
        let mut first_sink = Some(first_sink);
        let wait_start = Instant::now();
        let _spawn_slot = loop {
            {
                let map = self.sessions.lock().unwrap();
                if let Some(session) = map.get(&id) {
                    let pid = session.pid;
                    let sub_id = attach_sink(session, first_sink.take().unwrap());
                    // Return current dimensions and ownership before replay so mirrored full-screen TUIs use the right width.
                    let (cur_cols, cur_rows) = *session.size.lock().unwrap();
                    let owner = session.size_owner.lock().unwrap().clone();
                    drop(map);
                    // Nudge the row count to trigger SIGWINCH and force a complete redraw. Replay may not contain
                    // the alternate screen's current frame, leaving new clients blank until the next redraw.
                    // Call after dropping `map` because `nudge_winch` locks `sessions` itself.
                    nudge_winch(&self.sessions, &id);
                    // Replay the status snapshot for late clients. Existing clients receive identical idempotent
                    // values. Mark cached state as silent because replay is not a new transition and must not notify.
                    let event = StatusSignal::event_name(&id);
                    for mut payload in self.status_snapshot(&id) {
                        if payload.get("kind").and_then(serde_json::Value::as_str) == Some("state")
                        {
                            if let Some(obj) = payload.as_object_mut() {
                                obj.insert("silent".to_string(), serde_json::Value::Bool(true));
                            }
                        }
                        app.emit(&event, payload);
                    }
                    return Ok(SpawnResult {
                        pid,
                        launch: None,
                        sub_id,
                        attached: true,
                        cols: cur_cols,
                        rows: cur_rows,
                        owner,
                    });
                }
            }
            {
                let mut spawning = self.spawning.lock().unwrap();
                if attach_only {
                    // Attach-only mode never reserves or launches. Error if absent, unless another client is starting it.
                    if !spawning.contains(&id) {
                        return Err(format!("Session {id} is not running"));
                    }
                } else if spawning.insert(id.clone()) {
                    break SpawnSlot {
                        set: &self.spawning,
                        id: id.clone(),
                    };
                }
            }
            if wait_start.elapsed() >= SPAWN_WAIT_TIMEOUT {
                return Err(format!(
                    "Session {id} is being started by another client; please retry shortly"
                ));
            }
            std::thread::sleep(SPAWN_WAIT_POLL);
        };
        let first_sink = first_sink.take().expect("the sink has not been consumed since the slot was reserved");

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {e}"))?;

        // Default-shell resolution needs the data directory to locate bundled Git Bash on Windows.
        let data_dir_for_shell = app.data_dir().ok();
        // Heal stale persisted absolute shell paths, especially legacy bundled Git Bash locations removed by
        // upgrades. Fall back to an available shell, preferring the current Git Bash for Windows terminals.
        let shell = match shell {
            Some(s) if shell_path_usable(&s) => s,
            Some(stale) => {
                let fb = resolve_fallback_shell(kind, data_dir_for_shell.as_deref());
                eprintln!("persisted shell path not found, falling back: {stale:?} -> {fb}");
                fb
            }
            None => default_shell(kind, data_dir_for_shell.as_deref()),
        };
        // WSL is persisted as `wsl://<distribution>`, not an executable path. Convert it to
        // `wsl.exe --distribution <name>` and let WSL start the distribution's default interactive shell.
        let wsl_distro = wsl_distro_from_shell(&shell);
        if wsl_distro.is_some() && !cfg!(windows) {
            return Err("WSL shells are only available on Windows".to_string());
        }
        if wsl_distro.is_some() && kind != SessionKind::Terminal {
            return Err(
                "WSL shells are supported for terminal sessions only; agent sessions use the Windows host shell"
                    .to_string(),
            );
        }
        let mut cmd = if let Some(distro) = wsl_distro {
            let mut command = CommandBuilder::new("wsl.exe");
            command.arg("--distribution");
            command.arg(distro);
            command
        } else {
            CommandBuilder::new(&shell)
        };
        // Start Unix shells as login shells so profiles and `path_helper` populate `PATH`, matching common terminals.
        #[cfg(unix)]
        cmd.arg("-l");
        // Add Windows launch arguments by shell family.
        #[cfg(windows)]
        {
            // WSL starts the distribution shell itself; never append a shell `-l` option to `wsl.exe`.
            if wsl_distro.is_none() {
                match inject::shell_kind(&shell) {
                    // Start Git Bash as a login shell so `/etc/profile` configures utilities and UTF-8 locale.
                    inject::ShellKind::Posix => {
                        cmd.arg("-l");
                    }
                    // Agent sessions start PowerShell with a process-local execution-policy bypass so npm `.ps1`
                    // shims and interactive profiles can load. Plain terminals preserve the native policy.
                    // Group Policy can still override the command-line setting.
                    inject::ShellKind::PowerShell if kind != SessionKind::Terminal => {
                        cmd.arg("-ExecutionPolicy");
                        cmd.arg("Bypass");
                    }
                    // cmd.exe, fish, and user-selected plain PowerShell terminals need no extra arguments.
                    _ => {}
                }
            }
        }
        // Normalize the working directory. Empty means unspecified; a stale path falls back to the home directory
        // to avoid process-creation failures. Codex rollout capture also uses the result for cwd matching.
        let spawn_cwd = match cwd.filter(|d| !d.is_empty()) {
            Some(d) if std::path::Path::new(&d).is_dir() => Some(d),
            Some(stale) => {
                let fb = home_dir().filter(|h| std::path::Path::new(h).is_dir());
                eprintln!("persisted cwd not found, falling back: {stale:?} -> {fb:?}");
                fb
            }
            None => None,
        };
        if let Some(dir) = &spawn_cwd {
            cmd.cwd(dir);
        }
        // A PTY inherits this process's environment, which on Linux still carries the AppImage launcher's
        // rewrite of `PYTHONHOME`, `LD_LIBRARY_PATH`, `PATH`, and friends. Undo it before anything else so
        // the user's shell sees the system environment and later overrides here still win. No-op elsewhere.
        crate::appimage::scrub_pty(&mut cmd);
        // Drop inherited color suppressors before advertising color, so an instance launched from an IDE
        // or an agent harness does not hand every session a monochrome TUI.
        scrub_color_suppressors(&mut cmd);
        // Advertise a color-capable terminal.
        cmd.env("TERM", "xterm-256color");
        // Explicitly advertise true color so applications do not conservatively fall back to 256 colors.
        cmd.env("COLORTERM", "truecolor");
        // Route notifications through `TERM_PROGRAM`. Managed agent sessions identify as `vlx-term`, which
        // agents treat as unknown, suppressing OSC notifications in favor of authoritative hooks/plugins.
        // Plain terminals identify as `ghostty`, enabling OSC 9 for agents launched manually, including over SSH.
        // Ghostty is recognized while emitting fewer private sequences than iTerm2, reducing xterm.js interference.
        // Browser sessions should not spawn PTYs; their non-agent classification is defensive.
        let is_agent_session = matches!(
            kind,
            SessionKind::Claude
                | SessionKind::Codex
                | SessionKind::Opencode
                | SessionKind::Copilot
                | SessionKind::Cursor
                | SessionKind::Antigravity
                | SessionKind::Cline
                | SessionKind::Pi
                | SessionKind::Crush
                | SessionKind::Kimi
                | SessionKind::Kiro
                | SessionKind::Grok
                | SessionKind::Zoo
        );
        if is_agent_session {
            cmd.env("TERM_PROGRAM", "vlx-term");
            cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        } else {
            cmd.env("TERM_PROGRAM", "ghostty");
            cmd.env("TERM_PROGRAM_VERSION", "1.2.0");
        }
        // `COLORFGBG` communicates foreground/background brightness to programs that do not query OSC 11.
        // Background 0 is dark and 15 is light; leave it unset when unknown.
        match dark {
            Some(true) => {
                cmd.env("COLORFGBG", "15;0");
            }
            Some(false) => {
                cmd.env("COLORFGBG", "0;15");
            }
            None => {}
        }
        // Configure UTF-8 locale by platform. Locale-dependent shells and tools corrupt multibyte text when
        // encoding falls back to C/ASCII. Values must be recognized by the platform, and an invalid `LC_CTYPE`
        // overrides a valid `LANG`; bare `UTF-8` works on macOS/BSD but not glibc or Git for Windows.
        #[cfg(target_os = "macos")]
        {
            // Finder/Spotlight launches may provide an empty or C locale. Match Terminal.app with UTF-8 defaults.
            let lang = std::env::var("LANG").unwrap_or_default();
            if !lang.to_ascii_uppercase().contains("UTF-8")
                && !lang.to_ascii_uppercase().contains("UTF8")
            {
                cmd.env("LANG", "en_US.UTF-8");
            }
            cmd.env("LC_CTYPE", "UTF-8");
        }
        // Windows intentionally receives no locale override. Git Bash's login profile derives a valid locale,
        // while cmd.exe and PowerShell tools already use UTF-8 when locale variables are absent. Invalid MSYS
        // overrides would force the C locale and prevent the profile from repairing it.
        #[cfg(target_os = "linux")]
        {
            // GUI launches on Linux may also receive C locale. Use glibc's built-in `C.UTF-8` only when no
            // locale variable advertises UTF-8, and do not override the user's `LC_CTYPE`.
            let has_utf8 = ["LC_ALL", "LC_CTYPE", "LANG"].iter().any(|k| {
                std::env::var(k)
                    .map(|v| {
                        let u = v.to_ascii_uppercase();
                        u.contains("UTF-8") || u.contains("UTF8")
                    })
                    .unwrap_or(false)
            });
            if !has_utf8 {
                cmd.env("LANG", "C.UTF-8");
            }
        }

        // Inject variables used by `vspawn` to create a child under the current session through the local
        // `/spawn` endpoint. All sessions receive them, including terminals where users launch agents manually.
        cmd.env("VLX_SESSION_ID", &id);
        cmd.env("VLX_TOKEN", &hook.token);
        cmd.env("VLX_SPAWN_URL", format!("http://127.0.0.1:{}", hook.port));

        // The executable path is shared by Codex notify configuration and built-in command shims.
        let exe_path = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // Prepend `<data_dir>/bin` so bundled `vspawn`, `vopen`, and related commands work without installation.
        // Cross-platform shims call hidden executable subcommands through `VLX_EXE`. Their `v`-prefixed names
        // avoid system-command collisions, so no shell startup wrapper is needed.
        if let Ok(data_dir) = app.data_dir() {
            let bin = crate::agent::spawn_cli::bin_dir(&data_dir);
            // Read the scrubbed `PATH`; the raw one still leads with the AppImage's bundle directories.
            let existing = crate::appimage::clean_var("PATH").unwrap_or_default();
            // Use the platform-specific `PATH` separator.
            let sep = if cfg!(windows) { ';' } else { ':' };
            cmd.env("PATH", format!("{}{sep}{existing}", bin.display()));
            cmd.env("VLX_BIN_DIR", bin.as_os_str());
        }
        // Hooks and shims call this absolute path. Keep it outside data-directory handling so Codex lifecycle
        // hooks remain available even if the data directory cannot be resolved.
        cmd.env("VLX_EXE", &exe_path);

        // Build in-memory injection and the launch command for typed sessions. Read the configured executable
        // path at spawn time so settings changes apply to the next launch.
        let bin_path = session_agent_path(&app, &id).or_else(|| agent_bin_path(&app, kind));
        // Lazily install Pi's state-bridge extension under `<data_dir>/pi/` and pass its absolute path for
        // loading through `-e "$VLX_PI_EXT"`. The static extension reads the session's injected `VLX_*` values.
        // It reports without persisting the port or token. If installation fails, log it and let Pi launch
        // without authoritative state, matching OpenCode's screen-detection fallback.
        let pi_ext_path: Option<String> = if kind == SessionKind::Pi {
            match app.data_dir() {
                Ok(data_dir) => {
                    match crate::agent::pi::install(&data_dir) {
                        Ok(p) => Some(p.to_string_lossy().to_string()),
                        Err(e) => {
                            eprintln!("failed to install pi extension (pi status reporting unavailable): {e}");
                            None
                        }
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        };
        // Refresh the Kiro shadow agent before building the launch command. It re-clones the user's default
        // agent every time so their settings stay current, and a failure simply omits `--agent`.
        let kiro_agent: Option<String> = if kind == SessionKind::Kiro {
            let started = std::time::Instant::now();
            crate::agent::kiro::audit_install("started", 0);
            match crate::agent::kiro::install() {
                Ok(name) => {
                    crate::agent::kiro::audit_install("success", started.elapsed().as_millis());
                    Some(name)
                }
                Err(e) => {
                    crate::agent::kiro::audit_install("failed", started.elapsed().as_millis());
                    eprintln!(
                        "failed to install kiro shadow agent (kiro status reporting unavailable): {e}"
                    );
                    None
                }
            }
        } else {
            None
        };
        let codex_hooks_supported =
            kind != SessionKind::Codex || codex_lifecycle_hooks_supported(bin_path.as_deref());
        // Codex prompts through `/hooks` for any hook whose hash is not already in `hooks.state`, so this
        // must complete before the launch below. Failure degrades status reporting only.
        if kind == SessionKind::Codex && codex_hooks_supported {
            if let Err(e) = crate::agent::codex_hooks::ensure_session_hook_trust(bin_path.as_deref())
            {
                eprintln!(
                    "failed to record Codex hook trust (Codex may prompt to trust VelaTerm hooks): {e}"
                );
            }
        }
        let should_capture_agent_id = resume_id.is_none() || fork;
        // Only the Windows theme-injection branch mutates this value.
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut agent_spawn = inject::prepare_with_args_capabilities(
            kind,
            &shell,
            &exe_path,
            &hook,
            &id,
            resume_id.as_deref(),
            fork,
            init_prompt.as_deref(),
            extra_args.as_deref(),
            bin_path.as_deref(),
            pi_ext_path.as_deref(),
            kiro_agent.as_deref(),
            codex_hooks_supported,
        );
        // Windows only: set Claude's theme explicitly in `--settings`. ConPTY answers OSC 11 with its own
        // always-dark background, making automatic detection incorrect. Unix PTYs handle OSC 11 and live
        // theme changes correctly, so they retain automatic behavior. `CLAUDE_SETTINGS_ENV` carries the setting.
        #[cfg(windows)]
        if let Some(theme) = theme.as_deref() {
            if kind == SessionKind::Claude {
                for (k, v) in agent_spawn.env.iter_mut() {
                    if k == inject::CLAUDE_SETTINGS_ENV {
                        if let Ok(mut j) = serde_json::from_str::<serde_json::Value>(v) {
                            j["theme"] = serde_json::Value::String(theme.to_string());
                            *v = j.to_string();
                        }
                    }
                }
            }
        }
        // Non-Windows platforms preserve automatic live theme switching.
        #[cfg(not(windows))]
        let _ = &theme;
        for (k, v) in &agent_spawn.env {
            cmd.env(k, v);
        }

        // OpenCode receives inline configuration pointing to the installed state-bridge plugin. It reads
        // injected `VLX_*` values and reports session, idle, and permission events. Installation failure
        // leaves OpenCode functional with screen detection as a fallback.
        if kind == SessionKind::Opencode {
            if let Ok(data_dir) = app.data_dir() {
                let plugin = crate::agent::opencode::plugin_path(&data_dir);
                if plugin.exists() {
                    cmd.env(
                        inject::OPENCODE_CONFIG_ENV,
                        inject::build_opencode_config_content(
                            &plugin.to_string_lossy(),
                            permission_mode.as_deref(),
                        ),
                    );
                }
            }
        }

        // Lazily install Copilot's static state-bridge hooks under `~/.copilot/hooks/vlx-term.json`.
        // Installation is idempotent and dynamic values remain in `VLX_*`; failures only disable authoritative state.
        if kind == SessionKind::Copilot {
            if let Err(e) = crate::agent::copilot::install() {
                eprintln!(
                    "failed to install copilot hooks (copilot status reporting unavailable): {e}"
                );
            }
        }

        // Lazily install Cursor's static hook script and merge it into `~/.cursor/hooks.json`, preserving
        // user entries and refusing malformed structures. Dynamic values stay in `VLX_*`; failures only
        // disable authoritative state reporting.
        if kind == SessionKind::Cursor {
            if let Err(e) = crate::agent::cursor::install() {
                eprintln!(
                    "failed to install cursor hooks (cursor status reporting unavailable): {e}"
                );
            }
        }

        // Install Cline's hook scripts under `<data_dir>/cline/hooks/` and expose them only to this process
        // through `CLINE_HOOKS_DIR`. Scripts read dynamic `VLX_*` values; failures fall back to screen detection.
        if kind == SessionKind::Cline {
            match app.data_dir() {
                Ok(data_dir) => match crate::agent::cline::install(&data_dir) {
                    Ok(hooks_dir) => {
                        cmd.env("CLINE_HOOKS_DIR", &hooks_dir);
                    }
                    Err(e) => {
                        eprintln!("failed to install cline hooks (cline status reporting unavailable): {e}");
                    }
                },
                Err(e) => {
                    eprintln!("failed to resolve data dir for cline hooks (cline status reporting unavailable): {e}");
                }
            }
        }

        // Install Antigravity's static bridge at `~/.gemini/vlx-term/hook.sh` and merge the dedicated
        // `vlx-term-status` group into global hooks, preserving all user groups. Failures retain normal launch.
        if kind == SessionKind::Antigravity {
            if let Err(e) = crate::agent::antigravity::install() {
                eprintln!("failed to install antigravity hooks (antigravity status reporting unavailable): {e}");
            }
        }

        // Build an idempotent Crush shadow configuration under `<data_dir>/crush/` by cloning user settings
        // and adding the `PreToolUse` bridge. `CRUSH_GLOBAL_CONFIG` applies it only to this session without
        // touching user configuration or data. Failures retain Crush's own configuration and screen fallback.
        if kind == SessionKind::Crush {
            match app.data_dir() {
                Ok(data_dir) => match crate::agent::crush::install(&data_dir) {
                    Ok(config_dir) => {
                        cmd.env("CRUSH_GLOBAL_CONFIG", &config_dir);
                    }
                    Err(e) => {
                        eprintln!("failed to install crush shadow config (crush status reporting unavailable): {e}");
                    }
                },
                Err(e) => {
                    eprintln!("failed to resolve data dir for crush shadow config (crush status reporting unavailable): {e}");
                }
            }
        }

        // Lazily install Kimi Code lifecycle hooks. Static configuration contains no credentials and reports
        // only when the managed process inherits `VLX_*`; installation failure does not block Kimi.
        if kind == SessionKind::Kimi {
            let started = std::time::Instant::now();
            crate::agent::kimi::audit_install("started", 0);
            if crate::agent::kimi::install().is_err() {
                crate::agent::kimi::audit_install("failed", started.elapsed().as_millis());
            } else {
                crate::agent::kimi::audit_install("success", started.elapsed().as_millis());
            }
        }

        // Grok loads hooks only from fixed user-level files. Install one namespaced JSON file that invokes
        // VelaTerm's hidden bridge; it reads dynamic VLX_* values and no-ops outside managed sessions.
        // PTY activity remains enabled below as a pre-hook/failure fallback and is ignored after an authoritative event.
        if kind == SessionKind::Grok {
            let started = std::time::Instant::now();
            crate::agent::grok::audit_install("started", 0);
            if crate::agent::grok::install(std::path::Path::new(&exe_path)).is_err() {
                crate::agent::grok::audit_install("failed", started.elapsed().as_millis());
            } else {
                crate::agent::grok::audit_install("success", started.elapsed().as_millis());
            }
        }

        let launch = agent_spawn.launch.clone();

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to start shell: {e}"))?;
        let pid = child.process_id().unwrap_or(0);
        // Clone a termination handle into the session, then move the child into a dedicated waiter. This
        // permits termination elsewhere while one thread exclusively waits for exit.
        let killer = child.clone_killer();

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to get PTY reader: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to get PTY writer: {e}"))?;

        // Drop the slave handle after the child takes ownership to avoid descriptor leaks.
        drop(pair.slave);

        // A dedicated per-session writer keeps PTY I/O off the global session lock and UI/IPC thread. A
        // bounded FIFO queue makes writes return quickly and confines blocking to one session. Keep `pty_write`
        // synchronous so IPC arrival order preserves keystroke order. Dropping the session sender ends the thread.
        let (input_tx, input_rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(INPUT_QUEUE_CAP);
        std::thread::spawn(move || {
            let mut writer = writer;
            while let Ok(data) = input_rx.recv() {
                if writer.write_all(&data).is_err() {
                    break; // The PTY is closed; discard meaningless residual input and exit quietly.
                }
                let _ = writer.flush();
            }
        });

        // Intentional-termination flag shared with the reader to suppress natural-exit events after `kill()`.
        let intentional = Arc::new(AtomicBool::new(false));
        let intentional_reader = Arc::clone(&intentional);

        // Shared output state combines replay, mode tracking, and subscribers under one lock. The spawning
        // caller is the first subscriber, and the reader ingests through the same shared object.
        let first_sub_id = NEXT_SUB_ID.fetch_add(1, Ordering::SeqCst);
        let stream = Arc::new(Mutex::new(OutputStream::new(SCROLLBACK_CAP)));
        stream.lock().unwrap().attach(first_sub_id, first_sink);

        self.sessions.lock().unwrap().insert(
            id.clone(),
            PtySession {
                master: pair.master,
                input: input_tx,
                killer,
                pid,
                intentional,
                stream: Arc::clone(&stream),
                size: Mutex::new((cols, rows)),
                size_owner: Mutex::new(Some(source.to_string())),
            },
        );

        // Monitor state tracks the last output for hysteretic busy/idle detection and an `alive` flag that
        // stops monitoring after EOF or termination.
        let last_activity = Arc::new(Mutex::new(Instant::now()));
        let alive = Arc::new(AtomicBool::new(true));
        let status_event = StatusSignal::event_name(&id);

        // Clone application handles before moving the original into the reader.
        let app_extra = app.clone();
        let status_event_extra = status_event.clone();

        // Record only non-Terminal sessions when explicitly enabled. Failure to resolve or open the recording
        // path silently disables recording without affecting the session.
        let mut recorder = if kind != SessionKind::Terminal && recording_enabled(&app) {
            open_recorder(&app, &id)
        } else {
            None
        };

        // Coalesce bursty output into 12 ms or 64 KB frames while forwarding sparse echoes immediately.
        // This caps bridge/WebSocket message volume without delaying recording or Bell/Title scanning.
        // The coalescer lock preserves total ingest order across reader and flush threads.
        let coalescer = Arc::new(Mutex::new(OutputCoalescer::new()));
        // The timed flush thread ingests expired pending data, parks while idle, and flushes before session exit.
        let flusher_thread = {
            let coalescer = Arc::clone(&coalescer);
            let stream = Arc::clone(&stream);
            let alive = Arc::clone(&alive);
            std::thread::spawn(move || loop {
                if !alive.load(Ordering::SeqCst) {
                    // Final safeguard for a race between the reader's last chunk and clearing `alive`.
                    let mut co = coalescer.lock().unwrap();
                    let bytes = co.take(Instant::now());
                    if !bytes.is_empty() {
                        stream.lock().unwrap().ingest(&bytes);
                    }
                    break;
                }
                let wait = {
                    let mut co = coalescer.lock().unwrap();
                    match co.deadline() {
                        None => None,
                        Some(deadline) => {
                            let now = Instant::now();
                            if now >= deadline {
                                let bytes = co.take(now);
                                if !bytes.is_empty() {
                                    stream.lock().unwrap().ingest(&bytes);
                                }
                                None
                            } else {
                                Some(deadline - now)
                            }
                        }
                    }
                };
                match wait {
                    Some(d) => std::thread::park_timeout(d),
                    None => std::thread::park_timeout(Duration::from_millis(500)),
                }
            })
            .thread()
            .clone()
        };

        // Detached background reader; keep the original handle available for optional Terminal monitoring.
        let last_for_read = Arc::clone(&last_activity);
        let alive_for_read = Arc::clone(&alive);
        let coalescer_for_read = Arc::clone(&coalescer);
        let flusher_for_read = flusher_thread.clone();
        // The reader removes its session at EOF to release the master handle.
        let sessions_for_read = Arc::clone(&self.sessions);
        // Clear cached state at session end so a reused ID cannot receive stale values.
        let status_cache_for_read = Arc::clone(&self.status_cache);
        // The reader shares output state for replay buffering, mode tracking, and fan-out.
        let stream_for_read = Arc::clone(&stream);
        let id_for_read = id.clone();
        let exit_event = format!("pty://exit/{id}");
        let killed_event = format!("pty://killed/{id}");
        std::thread::spawn(move || {
            let mut scanner = OutputScanner::default();
            let mut buf = [0u8; 8192];
            // Deduplicate repeated OSC titles before they trigger main-thread evaluation and frontend updates.
            let mut last_title: Option<String> = None;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF: the child process exited.
                    Ok(n) => {
                        let chunk = &buf[..n];
                        // Append this chunk to the session recording used for archival replay.
                        if let Some(rec) = recorder.as_mut() {
                            rec.write(chunk);
                        }
                        // Coalesced ingest still updates replay, mode tracking, and subscribers atomically
                        // against attach, so new subscribers receive each chunk exactly once.
                        {
                            let mut co = coalescer_for_read.lock().unwrap();
                            let was_armed = co.deadline().is_some();
                            if co.push(chunk, Instant::now()) {
                                let bytes = co.take(Instant::now());
                                stream_for_read.lock().unwrap().ingest(&bytes);
                            } else if !was_armed {
                                flusher_for_read.unpark();
                            }
                        }

                        // Update activity time; the monitor applies busy/idle hysteresis.
                        *last_for_read.lock().unwrap() = Instant::now();

                        // Scan for bells, terminal titles, and OSC notifications.
                        scanner.feed(&buf[..n], |ev| match ev {
                            ScanEvent::Bell => {
                                app.emit(&status_event, StatusSignal::Bell);
                            }
                            ScanEvent::Title(title) => {
                                // Do not resend a title identical to the previous one.
                                if last_title.as_deref() != Some(title.as_str()) {
                                    last_title = Some(title.clone());
                                    app.emit(&status_event, StatusSignal::Title { title });
                                }
                            }
                            // Forward OSC 9/777 notifications and let the frontend suppress them for sessions
                            // with authoritative sources.
                            ScanEvent::Notify { title, body } => {
                                app.emit(&status_event, StatusSignal::Notify { title, body });
                            }
                        });
                    }
                    Err(_) => break,
                }
            }
            // Flush pending coalesced bytes so final exit output is not lost.
            {
                let mut co = coalescer_for_read.lock().unwrap();
                let bytes = co.take(Instant::now());
                if !bytes.is_empty() {
                    stream_for_read.lock().unwrap().ingest(&bytes);
                }
            }
            // Reader exit ends the session and stops monitor and flush threads.
            alive_for_read.store(false, Ordering::SeqCst);
            flusher_for_read.unpark();
            // Remove the session and drop master, writer, and child handles. `kill` may have removed it first;
            // natural exit requires this path to prevent backend handle leaks.
            sessions_for_read.lock().unwrap().remove(&id_for_read);
            status_cache_for_read.lock().unwrap().remove(&id_for_read);
            // Emit `pty://exit` for natural exit and `pty://killed` for intentional termination. Other attached
            // clients use the latter to close their views instead of leaving a frozen terminal.
            if !intentional_reader.load(Ordering::SeqCst) {
                app.emit(&exit_event, ());
            } else {
                app.emit(&killed_event, ());
            }
            // Rebuild the stopped session's search index in the background. This is best-effort; stale refresh
            // before search preserves correctness.
            {
                let app_for_index = app.clone();
                let sid_for_index = id_for_read.clone();
                std::thread::spawn(move || {
                    crate::search::reindex_session_quiet(&app_for_index, &sid_for_index);
                });
            }
        });

        // A dedicated exit waiter fixes ConPTY retaining its output pipe after the child exits. `wait()` then
        // removes the session, drops the master, calls ClosePseudoConsole, and unblocks the reader with EOF.
        // On Unix the reader usually removes first; the waiter still reaps the child and avoids zombies.
        {
            let sessions_for_wait = Arc::clone(&self.sessions);
            let id_for_wait = id.clone();
            std::thread::spawn(move || {
                let mut child = child;
                let _ = child.wait();
                // Removing a dead process drops the master and forces ConPTY's reader to EOF; prior removal is harmless.
                sessions_for_wait.lock().unwrap().remove(&id_for_wait);
            });
        }

        match kind {
            // Browser nodes should never spawn PTYs; defensively attach no monitor or agent marker.
            SessionKind::Browser => {}
            // Terminal retains activity-based busy/idle only. Process-tree agent detection was too expensive.
            SessionKind::Terminal => {
                spawn_status_monitor(
                    app_extra,
                    status_event_extra,
                    Arc::clone(&last_activity),
                    Arc::clone(&alive),
                );
            }
            // Claude is marked immediately and relies entirely on precise HTTP-hook state. An interrupted
            // turn sends no Stop, and screen detection stops once the first hook establishes authority, so
            // the silence heal replaces the roughly one-minute wait for Claude's idle_prompt notification.
            SessionKind::Claude => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("claude".to_string()),
                        state_source: None,
                    },
                );
                spawn_idle_heal(
                    app_extra,
                    id.clone(),
                    status_event_extra,
                    Arc::clone(&last_activity),
                    Arc::clone(&alive),
                );
            }
            // OpenCode is marked immediately and relies on plugin events for working, waiting, and asking.
            // The hook body also persists its session ID for later `--session` resume.
            SessionKind::Opencode => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("opencode".to_string()),
                        state_source: None,
                    },
                );
            }
            // Copilot is marked immediately and relies on command-hook lifecycle events. The forwarded hook
            // body persists its session ID for later `--resume=<id>` without file scanning.
            SessionKind::Copilot => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("copilot".to_string()),
                        state_source: None,
                    },
                );
            }
            // Cursor is marked immediately and relies on merged command hooks. The forwarded `session_id`
            // is persisted for later `--resume=<id>` without file scanning.
            SessionKind::Cursor => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("cursor".to_string()),
                        state_source: None,
                    },
                );
            }
            // Antigravity is marked immediately and relies on Pre/PostInvocation hooks. Its forwarded session
            // ID is persisted for later `--conversation=<id>` without file scanning.
            SessionKind::Antigravity => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("antigravity".to_string()),
                        state_source: None,
                    },
                );
            }
            // Cline is marked immediately and relies on installed hook scripts for lifecycle state. Its
            // forwarded `taskId` is persisted for later `--id <id>` resume without file scanning.
            SessionKind::Cline => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("cline".to_string()),
                        state_source: None,
                    },
                );
            }
            // Pi is marked immediately and relies on extension lifecycle events. The extension extracts and
            // reports its session ID for later `--session <id>` resume without file scanning.
            SessionKind::Pi => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("pi".to_string()),
                        state_source: None,
                    },
                );
                crate::agent::resume::spawn_pi_capture(
                    app_extra.clone(),
                    id.clone(),
                    spawn_cwd.clone(),
                    pid,
                    should_capture_agent_id,
                );
            }
            // Crush is marked immediately. Its sole PreToolUse hook reports working and persists `session_id`,
            // but provides no authoritative waiting state. Screen detection and activity hysteresis therefore
            // supply ready/idle fallback without file scanning.
            SessionKind::Crush => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("crush".to_string()),
                        state_source: None,
                    },
                );
                spawn_status_monitor(
                    app_extra,
                    status_event_extra,
                    Arc::clone(&last_activity),
                    Arc::clone(&alive),
                );
            }
            // Kimi Code receives complete lifecycle state from official hooks and persists `session_id` from payloads.
            SessionKind::Kimi => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("kimi".to_string()),
                        state_source: None,
                    },
                );
            }
            // Kiro is marked immediately and relies on its global hooks file for lifecycle state, persisting
            // `session_id` from the payload for later `--resume-id <id>`. It emits Stop only when a turn ends
            // normally, so an interrupt leaves no event; the silence heal corrects that stuck working state.
            SessionKind::Kiro => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("kiro".to_string()),
                        state_source: None,
                    },
                );
                spawn_idle_heal(
                    app_extra,
                    id.clone(),
                    status_event_extra,
                    Arc::clone(&last_activity),
                    Arc::clone(&alive),
                );
            }
            // Grok's official lifecycle hooks provide authoritative state after their first callback. Retain
            // PTY activity until then (and when hook installation/loading fails); the frontend permanently
            // ignores fallback signals once a structured lifecycle event establishes authority.
            SessionKind::Grok => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("grok".to_string()),
                        state_source: None,
                    },
                );
                spawn_status_monitor(
                    app_extra,
                    status_event_extra,
                    Arc::clone(&last_activity),
                    Arc::clone(&alive),
                );
            }
            // Zoo Code has no injectable lifecycle hooks yet; mark its type and combine TUI detection with
            // PTY activity monitoring for long-running tools.
            SessionKind::Zoo => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("zoo".to_string()),
                        state_source: None,
                    },
                );
                spawn_status_monitor(
                    app_extra,
                    status_event_extra,
                    Arc::clone(&last_activity),
                    Arc::clone(&alive),
                );
            }
            // Codex reports its state source immediately, before the first lifecycle event. Modern versions are
            // hook-only from this point onward; legacy versions may still report turn completion through notify,
            // but neither version uses PTY activity or screen content to invent work state.
            SessionKind::Codex => {
                app_extra.emit(
                    &status_event_extra,
                    StatusSignal::Agent {
                        agent: Some("codex".to_string()),
                        state_source: Some(
                            if codex_hooks_supported {
                                "hooks"
                            } else {
                                "legacy"
                            }
                            .to_string(),
                        ),
                    },
                );
                // Lifecycle hooks report the exact Codex session ID. Do not concurrently guess rollouts by
                // cwd/mtime, which is unsafe for parallel sessions in one directory. Only older Codex versions
                // retain scanning as an ID fallback; notify later corrects the ID and title precisely.
                if should_scan_codex_rollout(codex_hooks_supported) {
                    crate::agent::resume::spawn_codex_capture(
                        app_extra.clone(),
                        id.clone(),
                        spawn_cwd.clone(),
                        pid,
                    );
                }
                spawn_idle_heal(
                    app_extra,
                    id.clone(),
                    status_event_extra,
                    Arc::clone(&last_activity),
                    Arc::clone(&alive),
                );
            }
        }

        Ok(SpawnResult {
            pid,
            launch,
            sub_id: first_sub_id,
            attached: false,
            cols,
            rows,
            owner: Some(source.to_string()),
        })
    }

    /// Writes keyboard data to a session's PTY. Input is global across viewers and does not participate in
    /// size ownership, so no source ID is required.
    ///
    /// Briefly lock to clone the sender, then enqueue and return. A dedicated FIFO writer touches the PTY
    /// descriptor without holding the global session lock. A full queue returns an error instead of blocking.
    pub fn write(&self, id: &str, data: &str) -> Result<(), String> {
        let tx = {
            let map = self.sessions.lock().unwrap();
            let session = map
                .get(id)
                .ok_or_else(|| format!("Session {id} not found"))?;
            session.input.clone()
        };
        tx.try_send(data.as_bytes().to_vec()).map_err(|e| match e {
            std::sync::mpsc::TrySendError::Full(_) => {
                format!("Session {id} input backlogged (PTY not consuming stdin)")
            }
            std::sync::mpsc::TrySendError::Disconnected(_) => {
                format!("Session {id} is shutting down")
            }
        })
    }

    /// Resizes a PTY under a single-owner model; all other clients mirror that size.
    ///
    /// PTY size is session-wide. Without arbitration, clients with different viewport widths would continually
    /// overwrite one another. Resize is allowed for explicit takeover, an unowned session, or the current owner;
    /// all other requests are ignored successfully as a race-safe fallback.
    ///
    /// Broadcasts `StatusSignal::Resized` only when size or ownership changes, allowing clients to switch fit/mirror mode.
    ///
    /// A resize after session closure is also a successful no-op, matching the normal ResizeObserver teardown race.
    pub fn resize(
        &self,
        app: &AppCtx,
        id: &str,
        cols: u16,
        rows: u16,
        source: &str,
        takeover: bool,
    ) -> Result<(), String> {
        let changed = {
            let map = self.sessions.lock().unwrap();
            // Resize after closure is a normal teardown race; treat it as a quiet successful no-op.
            let Some(session) = map.get(id) else {
                return Ok(());
            };
            let mut owner = session.size_owner.lock().unwrap();
            if !resize_allowed(owner.as_deref(), source, takeover) {
                return Ok(());
            }
            session
                .master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| format!("Failed to resize PTY: {e}"))?;
            let mut size = session.size.lock().unwrap();
            let changed = *size != (cols, rows) || owner.as_deref() != Some(source);
            *size = (cols, rows);
            *owner = Some(source.to_string());
            changed
        };
        // Broadcast outside the lock and only for real size or ownership changes.
        if changed {
            app.emit(
                &StatusSignal::event_name(id),
                StatusSignal::Resized {
                    cols,
                    rows,
                    owner: Some(source.to_string()),
                },
            );
        }
        Ok(())
    }

    /// Forces a full-screen TUI redraw by nudging SIGWINCH through `nudge_winch`. Local xterm refresh only
    /// repaints its existing buffer and cannot repair content rendered at the wrong dimensions. This simulates
    /// a temporary resize without changing authoritative size or ownership. Missing sessions are ignored.
    pub fn nudge_redraw(&self, id: &str) {
        nudge_winch(&self.sessions, id);
    }

    /// Queries a session's current working directory from its shell PID so split panes can inherit it.
    pub fn cwd(&self, id: &str) -> Option<String> {
        let pid = {
            let map = self.sessions.lock().unwrap();
            map.get(id)?.pid
        };
        if pid == 0 {
            return None;
        }
        process_cwd(pid)
    }

    /// Terminates and removes a session. Marks it intentional before EOF so the reader suppresses natural-exit events.
    pub fn kill(&self, id: &str) -> Result<(), String> {
        if let Some(mut session) = self.sessions.lock().unwrap().remove(id) {
            session.intentional.store(true, Ordering::SeqCst);
            let _ = session.killer.kill();
        }
        self.status_cache.lock().unwrap().remove(id);
        Ok(())
    }

    /// Terminates all sessions during headless shutdown, marking each intentional before killing its child.
    pub fn kill_all(&self) {
        let mut map = self.sessions.lock().unwrap();
        for (_, mut session) in map.drain() {
            session.intentional.store(true, Ordering::SeqCst);
            let _ = session.killer.kill();
        }
        self.status_cache.lock().unwrap().clear();
    }

    /// Detaches one output subscriber without killing the session. Spawn-or-attach is unified in `spawn`.
    ///
    /// If the departing subscriber owns size, release ownership and broadcast the current size with no owner.
    /// Remaining clients can then claim it instead of mirroring a vanished client. Desktop teardown kills instead.
    pub fn detach(&self, app: &AppCtx, id: &str, sub_id: u64, source: &str) {
        let released = {
            let map = self.sessions.lock().unwrap();
            let Some(session) = map.get(id) else { return };
            session.stream.lock().unwrap().detach(sub_id);
            let mut owner = session.size_owner.lock().unwrap();
            if owner.as_deref() == Some(source) {
                *owner = None;
                Some(*session.size.lock().unwrap())
            } else {
                None
            }
        };
        if let Some((cols, rows)) = released {
            app.emit(
                &StatusSignal::event_name(id),
                StatusSignal::Resized {
                    cols,
                    rows,
                    owner: None,
                },
            );
        }
    }
}

/// Whether a `FORCE_COLOR` value disables color. Anything else (`1`, `2`, `3`, `true`) was set to turn
/// color on and is left alone.
fn force_color_disables(value: &str) -> bool {
    let v = value.trim();
    v.is_empty() || v.eq_ignore_ascii_case("0") || v.eq_ignore_ascii_case("false")
}

/// Remove the inherited environment variables that make color-aware CLIs go monochrome.
///
/// A PTY inherits this process's environment. When the app is started from an IDE, an agent harness or
/// any CI-like shell that exports `NO_COLOR`, `FORCE_COLOR=0` or `CI`, chalk/Ink emit no SGR at all and
/// an agent TUI renders in the default foreground, even though `TERM` and `COLORTERM` advertise color
/// right after this call. Only the launcher's leftovers are dropped: a user who exports these in a shell
/// profile still gets them, because the profile runs inside the PTY. `TERM=dumb` needs no handling here
/// since `TERM` is overwritten unconditionally.
fn scrub_color_suppressors(cmd: &mut portable_pty::CommandBuilder) {
    cmd.env_remove("NO_COLOR");
    cmd.env_remove("CI");
    let disables = cmd
        .get_env("FORCE_COLOR")
        .and_then(|v| v.to_str())
        .is_some_and(force_color_disables);
    if disables {
        cmd.env_remove("FORCE_COLOR");
    }
}

/// Nudges PTY rows to trigger SIGWINCH and force a full-screen TUI redraw. It operates directly on the master
/// without changing authoritative size, ownership, or resize broadcasts. Rows minimize text reflow; failures are ignored.
///
/// Hold the intermediate size for 50 ms because SIGWINCH delivery is asynchronous. Restoring immediately lets
/// Node-based TUIs observe no size change and skip redraw. The delay produces one redraw at the temporary size
/// and another at the authoritative size, like a manual resize and restore.
///
/// Restore on a separate thread without sleeping under the session lock. Re-read authoritative size before
/// restoring so a real resize during the delay is never overwritten. Missing sessions are ignored.
fn nudge_winch(sessions: &Arc<Mutex<HashMap<String, PtySession>>>, id: &str) {
    // Step one: briefly lock and apply a one-row nudge.
    {
        let map = sessions.lock().unwrap();
        let Some(session) = map.get(id) else { return };
        let (cols, rows) = *session.size.lock().unwrap();
        let bump = if rows < u16::MAX {
            rows + 1
        } else {
            rows.saturating_sub(1)
        };
        if bump == rows {
            return;
        }
        let _ = session.master.resize(PtySize {
            rows: bump,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }
    // Step two: restore authoritative size after 50 ms. If a real resize already restored it, this is a harmless no-op.
    let sessions = Arc::clone(sessions);
    let id = id.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        let map = sessions.lock().unwrap();
        let Some(session) = map.get(&id) else { return };
        let (cols, rows) = *session.size.lock().unwrap();
        let _ = session.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    });
}

/// Pure resize-ownership arbitration: allow explicit takeover, an unowned session, or the current owner;
/// reject all other sources as a fallback for mirror-client races.
fn resize_allowed(owner: Option<&str>, source: &str, takeover: bool) -> bool {
    takeover || owner.is_none() || owner == Some(source)
}

/// Attaches a subscriber atomically under the output lock: send mode prelude, replay current output, then
/// register it. Mutual exclusion with ingest guarantees the snapshot plus every later chunk exactly once.
fn attach_sink(session: &PtySession, sink: OutputSink) -> u64 {
    let sub_id = NEXT_SUB_ID.fetch_add(1, Ordering::SeqCst);
    session.stream.lock().unwrap().attach(sub_id, sink);
    sub_id
}

/// Opens or creates a session recording and returns an append-only `Recorder`.
///
/// Recordings live at `<app_data_dir>/recordings/<session-id>.log`, naturally separated by application
/// identifier. Respawn appends a launch separator to the same file. Path or open failures return `None`.
fn open_recorder(app: &AppCtx, id: &str) -> Option<Recorder> {
    let data_dir = app.data_dir().ok()?;
    let dir = data_dir.join("recordings");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{id}.log"));
    let existing_len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = file
        .write_all(format!("\r\n--- [VelaTerm] session started/resumed @ {ts} ---\r\n").as_bytes());
    Some(Recorder::new(file, existing_len, RECORDING_CAP))
}

/// Reads the session-recording setting. Only an explicit `"1"` enables it; missing or unreadable settings
/// remain off. Callers independently exclude plain Terminal sessions.
/// Reads an agent's global executable path from `vlx-settings.agentDefaults.<kind>.path`. A nonempty path
/// launches directly; missing or invalid settings preserve command-name lookup. Expands common `~/` prefixes.
pub(crate) fn agent_bin_path(app: &AppCtx, kind: SessionKind) -> Option<String> {
    let key = match kind {
        SessionKind::Claude => "claude",
        SessionKind::Codex => "codex",
        SessionKind::Opencode => "opencode",
        SessionKind::Copilot => "copilot",
        SessionKind::Cursor => "cursor",
        SessionKind::Antigravity => "antigravity",
        SessionKind::Cline => "cline",
        SessionKind::Pi => "pi",
        SessionKind::Crush => "crush",
        SessionKind::Kimi => "kimi",
        SessionKind::Kiro => "kiro",
        SessionKind::Grok => "grok",
        SessionKind::Zoo => "zoo",
        _ => return None,
    };
    let json = {
        let conn = app.db().conn.lock().ok()?;
        crate::db::repo::get_app_settings(&conn)
            .ok()?
            .remove(VLX_SETTINGS_KEY)?
    };
    let v: serde_json::Value = serde_json::from_str(&json).ok()?;
    let p = v
        .get("agentDefaults")?
        .get(key)?
        .get("path")?
        .as_str()?
        .trim();
    if p.is_empty() {
        return None;
    }
    Some(expand_home_prefix(p))
}

/// Expand a leading `~/`, or the Windows `~\` people type by hand, against the home directory. Every other
/// path is returned unchanged, including one whose home directory cannot be resolved.
fn expand_home_prefix(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\")) {
        if let Some(home) = crate::host::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    p.to_string()
}

/// This session's own agent executable, or None to fall back to the per-kind default. Read at spawn time
/// like the default, so editing it applies to the next launch.
fn session_agent_path(app: &AppCtx, id: &str) -> Option<String> {
    let raw = {
        let conn = app.db().conn.lock().ok()?;
        crate::db::repo::get_agent_path(&conn, id).ok()?
    }?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(expand_home_prefix(trimmed))
}

/// Detects whether the installed Codex is new enough for lifecycle-hook injection.
///
/// `--dangerously-bypass-hook-trust` serves only as the version marker for that capability; VelaTerm records
/// hook trust explicitly instead of passing the flag, which would also suppress the trust check on the user's
/// own hooks. Inspects only public `--help` output without starting a session or using the network. Failure
/// means unsupported and retains notify/screen/busy fallback. Windows `.cmd` wrappers run through cmd.exe;
/// native executables run directly. Probe each launch so an in-place Codex upgrade takes effect immediately.
fn codex_lifecycle_hooks_supported(bin_path: Option<&str>) -> bool {
    let program = bin_path.filter(|p| !p.trim().is_empty()).unwrap_or("codex");
    #[cfg(windows)]
    let output = {
        let lower = program.to_ascii_lowercase();
        if lower.ends_with(".cmd") || lower.ends_with(".bat") {
            crate::host::command("cmd.exe")
                .args(["/D", "/C", program, "--help"])
                .output()
        } else {
            crate::host::command(program).arg("--help").output()
        }
    };
    #[cfg(not(windows))]
    let output = crate::host::command(program).arg("--help").output();

    let Ok(output) = output else {
        return false;
    };
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text.contains("--dangerously-bypass-hook-trust")
}

fn recording_enabled(app: &AppCtx) -> bool {
    let Ok(conn) = app.db().conn.lock() else {
        return false;
    };
    match crate::db::repo::get_app_settings(&conn) {
        Ok(map) => map.get(RECORD_SESSIONS_KEY).map(String::as_str) == Some("1"),
        Err(_) => false,
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Heuristic fallback monitor that reports coarse busy/idle state using output-activity hysteresis.
///
/// Used only when authoritative events are unavailable. It provides busy/idle fallback without expensive
/// process-tree agent detection.
fn spawn_status_monitor(
    app: AppCtx,
    status_event: String,
    last_activity: Arc<Mutex<Instant>>,
    alive: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut working = false; // Whether the session is currently classified as working.
        let mut active_streak: u32 = 0; // Consecutive cycles with output.
        let mut idle_streak: u32 = 0; // Consecutive cycles without output.
        loop {
            std::thread::sleep(POLL_INTERVAL);
            if !alive.load(Ordering::SeqCst) {
                break;
            }

            // A cycle is active when the latest output falls within the activity window.
            let active = last_activity.lock().unwrap().elapsed() < ACTIVE_WINDOW;
            if active {
                active_streak = active_streak.saturating_add(1);
                idle_streak = 0;
            } else {
                idle_streak = idle_streak.saturating_add(1);
                active_streak = 0;
            }

            // Hysteresis requires sustained output to enter working and sustained silence to leave it.
            if !working && active_streak >= WORK_ENTER_TICKS {
                working = true;
                app.emit(&status_event, StatusSignal::Busy { busy: true });
            } else if working && idle_streak >= IDLE_LEAVE_TICKS {
                working = false;
                app.emit(&status_event, StatusSignal::Busy { busy: false });
            }
        }
    });
}

/// One-way self-heal for a hook-driven session whose turn-end event never arrives.
///
/// Claude and Codex both fire `Stop` only when a turn ends normally. Interrupting with Esc, a stream
/// error, or any other aborted turn produces no callback at all. Codex is additionally barred from screen
/// and activity guesses, and Claude disables screen detection as soon as its first hook establishes
/// authority, so either session would keep displaying working until something else happened to it.
/// Claude's idle_prompt notification eventually corrects it, but only after about a minute.
///
/// The heal watches PTY silence instead of terminal content: a running turn repaints the agent's TUI about
/// once per second, so sustained silence means no turn is running. It only ever corrects working into
/// waiting — never the reverse, and never a session waiting on an approval prompt — and stays silent
/// because every situation that reaches here is one the user just caused themselves.
fn spawn_idle_heal(
    app: AppCtx,
    sid: String,
    status_event: String,
    last_activity: Arc<Mutex<Instant>>,
    alive: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        // Emit at most one correction per silent stretch; renewed output arms the next one.
        let mut healed = false;
        loop {
            std::thread::sleep(HEAL_POLL);
            if !alive.load(Ordering::SeqCst) {
                break;
            }
            if last_activity.lock().unwrap().elapsed() < QUIET_HEAL {
                healed = false;
                continue;
            }
            if healed {
                continue;
            }
            // Only a session still displaying working may be corrected. Reading the emitted snapshot
            // keeps this decision on the same state clients see, including states set by hooks.
            if app.pty().cached_agent_state(&sid).as_deref() != Some("working") {
                continue;
            }
            healed = true;
            app.emit(
                &status_event,
                StatusSignal::State {
                    state: AgentState::Waiting,
                    silent: true,
                    authoritative: true,
                },
            );
        }
    });
}

/// Queries a process's current working directory across platforms.
pub(crate) fn process_cwd(pid: u32) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        // macOS has no `/proc`; use `lsof` to inspect the cwd descriptor.
        let output = std::process::Command::new("lsof")
            .args(["-a", "-d", "cwd", "-Fn", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|l| l.strip_prefix('n').map(|s| s.to_string()))
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_link(format!("/proc/{pid}/cwd"))
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}

/// Disables rollout scanning when exact lifecycle hooks are available. This protects session identity:
/// mtime cannot map concurrent rollouts in the same directory to PTYs reliably.
fn should_scan_codex_rollout(codex_hooks_supported: bool) -> bool {
    !codex_hooks_supported
}

/// Prefix for persisted WSL launch specifications. A URI-like value stores the distribution name without
/// conflicting with existing executable paths or requiring a separate arguments field.
const WSL_SHELL_PREFIX: &str = "wsl://";

#[cfg(any(windows, test))]
fn wsl_shell_value(distro: &str) -> String {
    format!("{WSL_SHELL_PREFIX}{distro}")
}

fn wsl_distro_from_shell(shell: &str) -> Option<&str> {
    shell
        .strip_prefix(WSL_SHELL_PREFIX)
        .map(str::trim)
        .filter(|distro| !distro.is_empty())
}

/// Decodes `wsl.exe --list --quiet`, which may emit UTF-16LE or UTF-8. Prefer BOM detection, infer UTF-16LE
/// from NUL bytes, and otherwise decode UTF-8 lossily.
#[cfg(any(windows, test))]
fn decode_wsl_list_output(bytes: &[u8]) -> String {
    let (utf16, big_endian) = if bytes.starts_with(&[0xff, 0xfe]) {
        (&bytes[2..], false)
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        (&bytes[2..], true)
    } else if bytes.contains(&0) {
        (bytes, false)
    } else {
        return String::from_utf8_lossy(bytes).into_owned();
    };

    let units = utf16
        .chunks_exact(2)
        .map(|pair| {
            if big_endian {
                u16::from_be_bytes([pair[0], pair[1]])
            } else {
                u16::from_le_bytes([pair[0], pair[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

#[cfg(any(windows, test))]
fn parse_wsl_distros(stdout: &[u8]) -> Vec<String> {
    let mut distros = Vec::<String>::new();
    for line in decode_wsl_list_output(stdout).lines() {
        let name = line.trim_matches(|c: char| c.is_whitespace() || c == '\0' || c == '\u{feff}');
        if name.is_empty() || distros.iter().any(|seen| seen.eq_ignore_ascii_case(name)) {
            continue;
        }
        distros.push(name.to_string());
    }
    distros
}

#[cfg(windows)]
fn wsl_distros() -> Vec<String> {
    use std::os::windows::process::CommandExt;

    // Avoid flashing a console window while probing shells during application startup.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("wsl.exe")
        .args(["--list", "--quiet"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match output {
        Ok(result) if result.status.success() => parse_wsl_distros(&result.stdout),
        _ => Vec::new(),
    }
}

/// One selectable Terminal shell. `path` is persisted in `sessions.shell`: normally an executable path,
/// `wsl://<distribution>` for WSL, or empty for `default_shell`.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellOption {
    /// Stable frontend-only identifier such as `cmd`, `pwsh`, `gitbash`, or `wsl:<distribution>`.
    pub id: String,
    /// Display name such as PowerShell, Git Bash, or WSL: Ubuntu.
    pub label: String,
    /// Executable path, command name, or WSL specification persisted in the session's `shell` field.
    pub path: String,
    /// Whether this is the system-default Terminal shell, used to label sessions with an empty `shell` value.
    pub is_default: bool,
}

/// Lists shells available for Terminal sessions. Windows exposes cmd.exe, PowerShell, Git Bash, and installed
/// WSL distributions. macOS and Linux return an empty list and always use `$SHELL`, allowing the frontend to
/// hide shell-selection controls. Agent sessions use their fixed injection paths.
pub fn available_shells(data_dir: Option<&std::path::Path>) -> Vec<ShellOption> {
    #[cfg(windows)]
    {
        windows_shells(data_dir)
    }
    #[cfg(not(windows))]
    {
        let _ = data_dir;
        Vec::new()
    }
}

/// Finds an executable on `PATH` and returns its absolute path. Used only for Windows shell discovery.
#[cfg(windows)]
fn find_on_path(exe: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(exe);
        if cand.is_file() {
            return Some(cand.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(windows)]
fn windows_shells(data_dir: Option<&std::path::Path>) -> Vec<ShellOption> {
    let mut out = Vec::new();
    // Resolve the default Terminal shell so the matching entry can be marked.
    let default_path = default_shell(SessionKind::Terminal, data_dir);
    let push = |out: &mut Vec<ShellOption>, id: &str, label: &str, path: String| {
        let is_default = path == default_path;
        out.push(ShellOption {
            id: id.to_string(),
            label: label.to_string(),
            is_default,
            path,
        });
    };

    // List bundled or downloaded Git Bash first because it is the preferred Terminal shell.
    if let Some(dd) = data_dir {
        if let Some(bash) = crate::agent::gitbash::default_bash(dd) {
            push(
                &mut out,
                "gitbash",
                "Git Bash",
                bash.to_string_lossy().into_owned(),
            );
        }
    }
    // Windows PowerShell ships with the system and resolves through `PATH`.
    push(
        &mut out,
        "powershell",
        "PowerShell",
        "powershell.exe".into(),
    );
    // List PowerShell 7+ only when `pwsh` is installed.
    if let Some(p) = find_on_path("pwsh.exe") {
        push(&mut out, "pwsh", "PowerShell 7", p);
    }
    // Resolve cmd.exe through `COMSPEC`.
    let cmd = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    push(&mut out, "cmd", "cmd", cmd);
    // List WSL only when installed distributions are returned, not merely when `wsl.exe` exists. Persist each
    // distribution independently so changing the system default cannot silently alter existing sessions.
    for distro in wsl_distros() {
        push(
            &mut out,
            &format!("wsl:{distro}"),
            &format!("WSL: {distro}"),
            wsl_shell_value(&distro),
        );
    }
    // Offer a separately installed Git Bash when it differs from the bundled path.
    if let Some(p) = git_bash_path() {
        if !out.iter().any(|s| s.path == p) {
            push(&mut out, "gitbash-system", "Git Bash (system)", p);
        }
    }
    out
}

/// Detects Git Bash's `bash.exe`, which is commonly absent from `PATH`.
#[cfg(windows)]
fn git_bash_path() -> Option<String> {
    use std::path::Path;
    // 1. Standard installation directories.
    for c in [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ] {
        if Path::new(c).is_file() {
            return Some(c.to_string());
        }
    }
    // 2. Derive the Git root from `<Git>\cmd\git.exe` or `<Git>\bin\git.exe` found on `PATH`.
    if let Some(git) = find_on_path("git.exe") {
        if let Some(root) = Path::new(&git).parent().and_then(Path::parent) {
            let bash = root.join("bin").join("bash.exe");
            if bash.is_file() {
                return Some(bash.to_string_lossy().into_owned());
            }
        }
    }
    // 3. Per-user installation under `LOCALAPPDATA\Programs\Git`.
    if let Ok(la) = std::env::var("LOCALAPPDATA") {
        let bash = Path::new(&la).join(r"Programs\Git\bin\bash.exe");
        if bash.is_file() {
            return Some(bash.to_string_lossy().into_owned());
        }
    }
    None
}

/// Returns the platform's default shell. `data_dir` remains for signature compatibility and shell listing.
///
/// Windows：
/// Agent sessions default to PowerShell because `$env:` injection is safer than cmd.exe `%VAR%` expansion.
/// Terminal sessions default to `COMSPEC`; users can still select Git Bash or PowerShell.
fn default_shell(kind: SessionKind, data_dir: Option<&std::path::Path>) -> String {
    #[cfg(windows)]
    {
        // Default-shell resolution no longer depends on bundled Git Bash.
        let _ = data_dir;
        if kind != SessionKind::Terminal {
            return "powershell.exe".to_string();
        }
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (kind, data_dir);
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
    }
}

/// Checks whether a persisted shell value can launch directly. Empty values use the default; absolute paths
/// must exist, while bare command names remain eligible for `PATH` resolution.
fn shell_path_usable(s: &str) -> bool {
    if s.trim().is_empty() {
        return false;
    }
    if s.starts_with(WSL_SHELL_PREFIX) {
        return wsl_distro_from_shell(s).is_some();
    }
    let p = std::path::Path::new(s);
    if p.is_absolute() {
        p.is_file()
    } else {
        true
    }
}

/// Chooses a fallback for a stale shell path. Windows terminals prefer the current bundled/downloaded Git Bash
/// to preserve the user's intent, then fall back to the system default.
fn resolve_fallback_shell(kind: SessionKind, data_dir: Option<&std::path::Path>) -> String {
    #[cfg(windows)]
    {
        if kind == SessionKind::Terminal {
            if let Some(bash) = data_dir.and_then(|d| crate::agent::gitbash::default_bash(d)) {
                return bash.to_string_lossy().into_owned();
            }
        }
    }
    default_shell(kind, data_dir)
}

/// Resolves the home directory for stale-cwd fallback. Windows uses `USERPROFILE` or `HOMEDRIVE` plus
/// `HOMEPATH`; other platforms use `HOME`. `None` lets the child inherit the process directory.
fn home_dir() -> Option<String> {
    #[cfg(windows)]
    {
        if let Ok(p) = std::env::var("USERPROFILE") {
            if !p.is_empty() {
                return Some(p);
            }
        }
        if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
            let h = format!("{drive}{path}");
            if !h.is_empty() {
                return Some(h);
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").ok().filter(|h| !h.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::Read;

    /// Color suppressors inherited from the launcher are dropped, while a `FORCE_COLOR` that was raised
    /// deliberately survives. Values come from the base environment a `CommandBuilder` starts with.
    #[test]
    fn scrub_color_suppressors_drops_only_the_suppressing_values() {
        let mut cmd = CommandBuilder::new("bash");
        cmd.env("NO_COLOR", "1");
        cmd.env("CI", "true");
        cmd.env("FORCE_COLOR", "0");
        super::scrub_color_suppressors(&mut cmd);
        assert!(cmd.get_env("NO_COLOR").is_none());
        assert!(cmd.get_env("CI").is_none());
        assert!(cmd.get_env("FORCE_COLOR").is_none());

        let mut kept = CommandBuilder::new("bash");
        kept.env("FORCE_COLOR", "3");
        super::scrub_color_suppressors(&mut kept);
        assert_eq!(kept.get_env("FORCE_COLOR").unwrap(), "3");
    }

    /// Only `0`, `false` and an empty value mean "no color"; every other value turns color on.
    #[test]
    fn force_color_disables_matches_only_off_values() {
        assert!(super::force_color_disables("0"));
        assert!(super::force_color_disables(" false "));
        assert!(super::force_color_disables("FALSE"));
        assert!(super::force_color_disables(""));
        assert!(!super::force_color_disables("1"));
        assert!(!super::force_color_disables("3"));
        assert!(!super::force_color_disables("true"));
    }

    /// Persisted shell validation: reject empty values, allow bare command names for `PATH`, and require
    /// absolute paths to exist.
    #[test]
    fn shell_path_usable_gates_missing_absolute_paths() {
        assert!(!super::shell_path_usable(""));
        assert!(!super::shell_path_usable("   "));
        // Bare command names remain eligible for `PATH` resolution.
        assert!(super::shell_path_usable("powershell.exe"));
        assert!(super::shell_path_usable("bash"));
        // Absolute paths are usable only while the file exists.
        let tmp = std::env::temp_dir().join(format!("vlx-shelltest-{}", std::process::id()));
        std::fs::write(&tmp, b"x").unwrap();
        assert!(super::shell_path_usable(&tmp.to_string_lossy()));
        std::fs::remove_file(&tmp).unwrap();
        assert!(!super::shell_path_usable(&tmp.to_string_lossy()));
    }

    /// Resize ownership: allow takeover, first claim when unowned, and the current owner; reject other clients.
    /// Keyboard input does not affect ownership.
    #[test]
    fn resize_ownership_arbitration() {
        use super::resize_allowed;

        // Any client can claim an unowned session with a normal resize.
        assert!(resize_allowed(None, "desktop", false));
        assert!(resize_allowed(None, "ws-1", false));

        // When desktop owns size, browser mirror resizes are rejected.
        assert!(resize_allowed(Some("desktop"), "desktop", false));
        assert!(!resize_allowed(Some("desktop"), "ws-1", false));

        // When a browser owns size, only that connection may resize normally.
        assert!(resize_allowed(Some("ws-2"), "ws-2", false));
        assert!(!resize_allowed(Some("ws-2"), "ws-1", false));
        assert!(!resize_allowed(Some("ws-2"), "desktop", false));

        // Explicit "fit this window" takeover always succeeds.
        assert!(resize_allowed(Some("desktop"), "ws-1", true));
        assert!(resize_allowed(None, "ws-1", true));
    }

    #[test]
    fn wsl_list_parses_utf16le_utf8_and_deduplicates() {
        let text = "\u{feff}Ubuntu-24.04\r\nDebian\r\nubuntu-24.04\r\n\r\n";
        let mut utf16 = vec![0xff, 0xfe];
        utf16.extend(
            text.encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            super::parse_wsl_distros(&utf16),
            vec!["Ubuntu-24.04".to_string(), "Debian".to_string()]
        );
        assert_eq!(
            super::parse_wsl_distros(b"Ubuntu\nDebian\n"),
            vec!["Ubuntu".to_string(), "Debian".to_string()]
        );
        assert!(super::parse_wsl_distros(&[]).is_empty());
    }

    #[test]
    fn wsl_shell_value_round_trips_distribution_name() {
        let value = super::wsl_shell_value("Ubuntu 24.04");
        assert_eq!(value, "wsl://Ubuntu 24.04");
        assert_eq!(super::wsl_distro_from_shell(&value), Some("Ubuntu 24.04"));
        assert_eq!(super::wsl_distro_from_shell("wsl://"), None);
        assert_eq!(super::wsl_distro_from_shell("powershell.exe"), None);
    }

    #[test]
    fn codex_rollout_scan_is_disabled_when_precise_hooks_are_available() {
        assert!(
            !super::should_scan_codex_rollout(true),
            "with concurrent sessions in the same cwd it must rely on the exact hook and stop guessing at rollouts"
        );
        assert!(
            super::should_scan_codex_rollout(false),
            "older Codex versions keep the compatibility fallback that binds the id only and generates no title"
        );
    }

    /// Status snapshots keep the latest value by kind, replay agent before state, ignore transient kinds,
    /// and clear on kill.
    #[test]
    fn status_cache_snapshot_roundtrip() {
        use serde_json::json;

        let mgr = super::PtyManager::new();

        // Bell and prompt signals are not snapshot state.
        mgr.cache_status("s1", &json!({"kind": "bell"}));
        mgr.cache_status("s1", &json!({"kind": "prompt", "text": "the first message"}));
        assert!(mgr.status_snapshot("s1").is_empty());

        // Overwrite state while preserving replay order: agent first, latest state second.
        mgr.cache_status("s1", &json!({"kind": "state", "state": "working"}));
        mgr.cache_status("s1", &json!({"kind": "agent", "agent": "claude"}));
        mgr.cache_status("s1", &json!({"kind": "state", "state": "waiting"}));
        let snap = mgr.status_snapshot("s1");
        assert_eq!(
            snap,
            vec![
                json!({"kind": "agent", "agent": "claude"}),
                json!({"kind": "state", "state": "waiting"}),
            ]
        );

        // Sessions remain isolated.
        assert!(mgr.status_snapshot("s2").is_empty());

        // Kill clears cached state even when no process exists.
        mgr.kill("s1").unwrap();
        assert!(mgr.status_snapshot("s1").is_empty());
    }

    /// Concurrent spawn regression: two clients opening one absent session must share one PID while receiving
    /// independent subscriptions; one launches and the other waits to attach.
    #[cfg(unix)]
    #[test]
    fn concurrent_spawn_same_session_attaches_instead_of_duplicating() {
        use crate::agent::server::HookEndpoint;
        use crate::host::{AppCtx, HeadlessHost};
        use crate::models::SessionKind;
        use std::sync::{Arc, Barrier};

        // Use a temporary data directory and isolated SQLite database for a side-effect-free headless host.
        let data_dir =
            std::env::temp_dir().join(format!("vlx-spawn-race-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).expect("failed to create the temporary directory");
        let db = crate::db::Db::open(&data_dir.join("test.db")).expect("failed to open the test database");
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(data_dir.clone(), db)));

        let mgr = Arc::new(super::PtyManager::new());
        let barrier = Arc::new(Barrier::new(2));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let mgr = Arc::clone(&mgr);
            let app = app.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                barrier.wait(); // Enter spawn simultaneously to maximize the race window.
                mgr.spawn(
                    app,
                    "race-1".to_string(),
                    SessionKind::Terminal,
                    Some("/bin/sh".to_string()),
                    None,
                    None,
                    80,
                    24,
                    HookEndpoint {
                        port: 0,
                        token: "test".to_string(),
                    },
                    None,
                    false,
                    None,
                    None, // extra_args
                    None, // permission_mode
                    None, // theme
                    "desktop",
                    false,
                    Box::new(|_| true),
                )
            }));
        }
        let results: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().expect("the spawn thread panicked"))
            .collect();

        // Both calls succeed with one PID and distinct subscriber IDs.
        let oks: Vec<_> = results
            .into_iter()
            .map(|r| r.expect("concurrent spawns should not fail"))
            .collect();
        assert_eq!(oks[0].pid, oks[1].pid, "both clients should share the same process");
        assert_ne!(oks[0].sub_id, oks[1].sub_id, "each client should have its own subscription");

        // The manager contains one session with both subscribers and no overwritten duplicate process.
        {
            let map = mgr.sessions.lock().unwrap();
            assert_eq!(map.len(), 1);
            let session = map.get("race-1").expect("the session should be running");
            assert_eq!(session.stream.lock().unwrap().subscriber_count(), 2);
        }
        // The reservation guard has cleared the spawn set.
        assert!(mgr.spawning.lock().unwrap().is_empty());

        mgr.kill("race-1").unwrap();
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    /// Attach returns current dimensions and owner so a second client can establish the correct mirror grid before replay.
    #[cfg(unix)]
    #[test]
    fn attach_returns_current_size_and_owner() {
        use crate::agent::server::HookEndpoint;
        use crate::host::{AppCtx, HeadlessHost};
        use crate::models::SessionKind;
        use std::sync::Arc;

        let data_dir =
            std::env::temp_dir().join(format!("vlx-attach-size-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).expect("failed to create the temporary directory");
        let db = crate::db::Db::open(&data_dir.join("test.db")).expect("failed to open the test database");
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(data_dir.clone(), db)));

        let mgr = super::PtyManager::new();
        let hook = HookEndpoint {
            port: 0,
            token: "test".to_string(),
        };

        // Desktop launches at 120×32 and owns that size.
        let first = mgr
            .spawn(
                app.clone(),
                "size-1".to_string(),
                SessionKind::Terminal,
                Some("/bin/sh".to_string()),
                None,
                None,
                120,
                32,
                hook.clone(),
                None,
                false,
                None,
                None, // extra_args
                None, // permission_mode
                None, // theme
                "desktop",
                false,
                Box::new(|_| true),
            )
            .expect("starting the first client failed");
        assert!(!first.attached);
        assert_eq!((first.cols, first.rows), (120, 32));
        assert_eq!(first.owner.as_deref(), Some("desktop"));

        // A browser attaching with a local 80×24 viewport must receive the existing 120×32 desktop-owned size.
        let second = mgr
            .spawn(
                app.clone(),
                "size-1".to_string(),
                SessionKind::Terminal,
                Some("/bin/sh".to_string()),
                None,
                None,
                80,
                24,
                hook,
                None,
                false,
                None,
                None, // extra_args
                None, // permission_mode
                None, // theme
                "ws-1",
                false,
                Box::new(|_| true),
            )
            .expect("attaching failed");
        assert!(second.attached);
        assert_eq!((second.cols, second.rows), (120, 32));
        assert_eq!(second.owner.as_deref(), Some("desktop"));

        mgr.kill("size-1").unwrap();
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    /// Attach-only reconnection attaches to a running session and errors otherwise; it must never resurrect a
    /// session closed by another client during disconnection.
    #[cfg(unix)]
    #[test]
    fn attach_only_never_resurrects_dead_session() {
        use crate::agent::server::HookEndpoint;
        use crate::host::{AppCtx, HeadlessHost};
        use crate::models::SessionKind;
        use std::sync::Arc;

        let data_dir =
            std::env::temp_dir().join(format!("vlx-attach-only-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).expect("failed to create the temporary directory");
        let db = crate::db::Db::open(&data_dir.join("test.db")).expect("failed to open the test database");
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(data_dir.clone(), db)));

        let mgr = super::PtyManager::new();
        let hook = HookEndpoint {
            port: 0,
            token: "test".to_string(),
        };
        let spawn = |attach_only: bool| {
            mgr.spawn(
                app.clone(),
                "ao-1".to_string(),
                SessionKind::Terminal,
                Some("/bin/sh".to_string()),
                None,
                None,
                80,
                24,
                hook.clone(),
                None,
                false,
                None,
                None, // extra_args
                None, // permission_mode
                None, // theme
                "ws-1",
                attach_only,
                Box::new(|_| true),
            )
        };

        // An absent session must error without leaving a session or reservation.
        assert!(spawn(true).is_err());
        assert!(mgr.sessions.lock().unwrap().is_empty());
        assert!(mgr.spawning.lock().unwrap().is_empty());

        // After normal launch, attach-only reconnects to the same PID.
        let first = spawn(false).expect("starting failed");
        let re = spawn(true).expect("reattaching failed");
        assert!(re.attached);
        assert_eq!(re.pid, first.pid);

        mgr.kill("ao-1").unwrap();
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    /// Detaching the owner clears ownership and broadcasts `Resized { owner: None }`; detaching a mirror does neither.
    #[cfg(unix)]
    #[test]
    fn detach_releases_size_owner() {
        use crate::agent::server::HookEndpoint;
        use crate::host::{AppCtx, HeadlessHost};
        use crate::models::SessionKind;
        use std::sync::{Arc, Mutex};

        let data_dir =
            std::env::temp_dir().join(format!("vlx-detach-owner-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).expect("failed to create the temporary directory");
        let db = crate::db::Db::open(&data_dir.join("test.db")).expect("failed to open the test database");
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(data_dir.clone(), db)));

        let mgr = super::PtyManager::new();
        let hook = HookEndpoint {
            port: 0,
            token: "test".to_string(),
        };

        // Browser client `ws-1` launches as owner, then `ws-2` attaches.
        let first = mgr
            .spawn(
                app.clone(),
                "own-1".to_string(),
                SessionKind::Terminal,
                Some("/bin/sh".to_string()),
                None,
                None,
                100,
                30,
                hook.clone(),
                None,
                false,
                None,
                None, // extra_args
                None, // permission_mode
                None, // theme
                "ws-1",
                false,
                Box::new(|_| true),
            )
            .expect("starting the first client failed");
        let second = mgr
            .spawn(
                app.clone(),
                "own-1".to_string(),
                SessionKind::Terminal,
                Some("/bin/sh".to_string()),
                None,
                None,
                80,
                24,
                hook,
                None,
                false,
                None,
                None, // extra_args
                None, // permission_mode
                None, // theme
                "ws-2",
                false,
                Box::new(|_| true),
            )
            .expect("attaching failed");

        // Listen for the resize broadcast caused by owner detachment.
        let events: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let events_cb = Arc::clone(&events);
        let listener = app.listen("pty://status/own-1", move |payload| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                if v.get("kind").and_then(serde_json::Value::as_str) == Some("resized") {
                    events_cb.lock().unwrap().push(v);
                }
            }
        });

        let owner_of = |mgr: &super::PtyManager| {
            mgr.sessions
                .lock()
                .unwrap()
                .get("own-1")
                .and_then(|s| s.size_owner.lock().unwrap().clone())
        };

        // Detaching non-owner `ws-2` leaves ownership unchanged and emits nothing.
        mgr.detach(&app, "own-1", second.sub_id, "ws-2");
        assert_eq!(owner_of(&mgr).as_deref(), Some("ws-1"));
        assert!(events.lock().unwrap().is_empty());

        // Detaching owner `ws-1` clears ownership and broadcasts the current size.
        mgr.detach(&app, "own-1", first.sub_id, "ws-1");
        assert_eq!(owner_of(&mgr), None);
        let got = events.lock().unwrap().clone();
        assert_eq!(got.len(), 1, "the owner leaving should broadcast Resized exactly once");
        assert_eq!(got[0]["cols"], 100);
        assert_eq!(got[0]["rows"], 30);
        assert!(got[0]["owner"].is_null());

        app.unlisten(listener);
        mgr.kill("own-1").unwrap();
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    /// Verifies the core PTY round trip: open, spawn, and read child output from the master.
    #[cfg(unix)]
    #[test]
    fn pty_echo_roundtrip() {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty failed");

        let mut cmd = CommandBuilder::new("/bin/echo");
        cmd.arg("vlx-pty-ok");
        let mut child = pair.slave.spawn_command(cmd).expect("spawn failed");

        let mut reader = pair.master.try_clone_reader().expect("cloning the reader failed");

        // Start reading before dropping the slave or waiting; on macOS, closing the slave can otherwise make
        // the master reach EOF before buffered data is consumed.
        let reader_handle = std::thread::spawn(move || {
            let mut output = String::new();
            let mut buf = [0u8; 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => output.push_str(&String::from_utf8_lossy(&buf[..n])),
                    Err(_) => break,
                }
            }
            output
        });

        // Release the slave so the reader reaches EOF after `echo` exits.
        drop(pair.slave);
        child.wait().expect("wait failed");

        let output = reader_handle.join().expect("joining the reader thread failed");
        assert!(
            output.contains("vlx-pty-ok"),
            "the expected output was not read from the PTY, got: {output:?}"
        );
    }

    /// Runs a real PTY command that prints, rings the bell, and sets a title, verifying `OutputScanner` against actual bytes.
    #[cfg(unix)]
    #[test]
    fn output_scanner_end_to_end() {
        use crate::pty::monitor::{OutputScanner, ScanEvent};
        use std::sync::{Arc, Mutex};

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty failed");

        // `printf` emits BEL and an OSC title sequence.
        let mut cmd = CommandBuilder::new("bash");
        cmd.arg("-c");
        cmd.arg("printf 'thinking...\\a\\033]0;codex-status\\007 done\\n'");
        cmd.env("TERM", "xterm-256color");

        let mut child = pair.slave.spawn_command(cmd).expect("spawn failed");
        let mut reader = pair.master.try_clone_reader().expect("cloning the reader failed");
        drop(pair.slave);

        // Feed real bytes into `OutputScanner` and record detected bells and titles.
        let bells = Arc::new(Mutex::new(0usize));
        let titles = Arc::new(Mutex::new(Vec::<String>::new()));
        let bells_t = Arc::clone(&bells);
        let titles_t = Arc::clone(&titles);
        let reader_handle = std::thread::spawn(move || {
            let mut scanner = OutputScanner::default();
            let mut buf = [0u8; 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => scanner.feed(&buf[..n], |ev| match ev {
                        ScanEvent::Bell => *bells_t.lock().unwrap() += 1,
                        ScanEvent::Title(t) => titles_t.lock().unwrap().push(t),
                        // This case verifies only bells and titles; ignore notification signals.
                        ScanEvent::Notify { .. } => {}
                    }),
                    Err(_) => break,
                }
            }
        });

        // The command exits after printing, allowing the reader thread to finish at EOF.
        let _ = child.wait();
        let _ = reader_handle.join();

        let bell_count = *bells.lock().unwrap();
        let title_list = titles.lock().unwrap().clone();

        assert!(bell_count >= 1, "a bell should be picked up from the real PTY, got {bell_count}");
        assert!(
            title_list.iter().any(|t| t == "codex-status"),
            "the OSC title codex-status should be picked up, got {title_list:?}"
        );
    }
}
