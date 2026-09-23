//! Claude catalogue read from the installed CLI itself.
//!
//! Claude Code answers the stream-json control requests `initialize` and `list_models` with the models
//! the installed version and the signed-in account recommend. That list is merged into the website
//! catalogue (or the bundled table), see `claude_models::assemble`: a model appears in VelaTerm as soon as
//! the user's Claude Code knows it, without anyone editing the bundled table or the website catalogue, and
//! the catalogue's other models stay selectable.
//!
//! One probe per binary is cached in app settings together with the CLI version string that produced
//! it. The probe runs in a private, empty directory, so no project's settings shape it. The cache is
//! reused until `claude --version` reports a different version (read at most once per run, or again on an
//! explicit refresh) or until the user presses Refresh. A running conversation's `list_models` answer may
//! reflect that conversation's project settings, so it never replaces the probe's list: it shapes that
//! conversation's own menu, and the identifiers it names are remembered per binary as additions only, so a
//! model one conversation reports appears in every menu. Probing is single-flight per binary and backs off
//! after a failure.
//!
//! Everything here spawns processes, so callers must stay off the main thread; dispatch and the Tauri
//! command layer already run catalogue reads inside blocking workers.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::chat::protocol::{self, Incoming};
use super::remote_model_catalog::{self, Status, EVENT};
use crate::host::AppCtx;

/// App-settings key holding the probe cache: a JSON map from binary hash to `Snapshot`. The suffix is
/// the schema version; a later format change gets `.v2` and ignores this key.
pub const KEY: &str = "model-catalog.claude.cli.v1";
/// Hard limit for one probe. The measured run takes about four seconds; a CLI that has not answered by
/// then is killed and the failure backs off.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(12);
/// Seconds to wait after a failed probe before the same binary is asked again, like the website catalogue.
pub const BACKOFF: u64 = 300;
/// Request ids the probe uses; the parser accepts answers to these two only.
const INIT_ID: &str = "probe-init";
const LIST_ID: &str = "probe-list";
/// The headless launch: no permission mode, no `--bare` (which would ignore the signed-in account and
/// so misreport the list). Hooks, MCP servers and session persistence are switched off, like the memory
/// runner's headless launch: the model list depends on the account, not on hooks or MCP, and a probe must
/// neither run configured commands nor leave a transcript behind.
const PROBE_ARGS: &[&str] = &[
    "-p",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--settings",
    "{\"disableAllHooks\":true}",
    "--strict-mcp-config",
    "--no-session-persistence",
];
/// Directory below the app data dir the probe runs in. Private (0700 on unix) because Claude Code reads
/// the working directory's `.claude/` project settings; a shared directory such as `/tmp` would let
/// another local user plant settings there.
const WORK_DIR: &str = "model-probe";
/// Environment the probe must not inherit: VelaTerm's own session identity, and the markers of a Claude
/// session VelaTerm itself may have been started from. The probe is not a VelaTerm session.
const SCRUBBED_ENV: &[&str] = &[
    "VLX_SESSION_ID",
    "VLX_TOKEN",
    "VLX_SPAWN_URL",
    "VLX_EXE",
    "VLX_BIN_DIR",
    super::inject::CLAUDE_SETTINGS_ENV,
    super::inject::NOTFOUND_URL_ENV,
];

/// One row of the CLI's `models` array, with the fields the catalogue uses. Unknown fields are ignored.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CliModel {
    /// What the CLI accepts as `--model`; short names such as `opus[1m]` included.
    pub value: String,
    /// The identifier the value currently resolves to, when the CLI says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_model: Option<String>,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub supported_effort_levels: Vec<String>,
    #[serde(default)]
    pub supports_fast_mode: bool,
    #[serde(default)]
    pub disabled: bool,
}

/// The cached answer for one binary.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The `claude --version` string of the binary that produced the list; None when it could not be read.
    pub version: Option<String>,
    /// Unix seconds of the probe or live answer.
    pub checked_at: u64,
    /// `probe` for a headless probe; `live` while only running conversations have reported, before any
    /// probe of this binary succeeded.
    pub origin: String,
    /// The neutral probe's rows; empty while only running conversations have reported.
    pub models: Vec<CliModel>,
    /// Rows running conversations reported, remembered as additions: they may add an identifier to the
    /// catalogue, never change or remove an entry. Rows the probe itself lists are dropped from here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live: Vec<CliModel>,
    /// The CLI version whose conversations reported `live`. Additions last until that version changes or
    /// Refresh is pressed, so an identifier a conversation once named cannot stay in every menu forever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_version: Option<String>,
}

/// Whether a catalogue read may start a probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    /// Probe when the cache is missing or belongs to another version, unless a failure is backing off.
    Allow,
    /// Never spawn; answer from the cache only. Used for the public share surface.
    CacheOnly,
    /// Re-read the version and probe regardless of cache and backoff. The Refresh button.
    Force,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeError {
    MissingBinary,
    Spawn(String),
    Timeout,
    Exit(i32),
    Malformed,
    Empty,
    Cli(String),
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbeError::MissingBinary => write!(f, "missingBinary"),
            ProbeError::Spawn(e) => write!(f, "spawnFailed:{e}"),
            ProbeError::Timeout => write!(f, "timeout"),
            ProbeError::Exit(code) => write!(f, "exit:{code}"),
            ProbeError::Malformed => write!(f, "malformed"),
            ProbeError::Empty => write!(f, "empty"),
            ProbeError::Cli(e) => write!(f, "cli:{e}"),
        }
    }
}

#[derive(Default)]
struct Attempt {
    attempted_at: u64,
    error: Option<String>,
}

#[derive(Default)]
struct State {
    /// Data directories whose stored cache has been merged into `snapshots`.
    loaded: HashSet<PathBuf>,
    snapshots: HashMap<String, Snapshot>,
    /// Version strings read this run, keyed by binary hash; a None entry records a failed read.
    versions: HashMap<String, Option<String>>,
    attempts: HashMap<String, Attempt>,
    in_flight: HashSet<String>,
    /// Probes started this run; tests assert on it.
    probes: u64,
}

fn state() -> &'static (Mutex<State>, Condvar) {
    static STATE: OnceLock<(Mutex<State>, Condvar)> = OnceLock::new();
    STATE.get_or_init(|| {
        run_started();
        (Mutex::new(State::default()), Condvar::new())
    })
}

/// Unix seconds at which this run first touched the catalogue. A snapshot checked since then was produced
/// by this run, whatever its version string says.
fn run_started() -> u64 {
    static STARTED_AT: OnceLock<u64> = OnceLock::new();
    *STARTED_AT.get_or_init(now)
}

static STARTED: OnceLock<()> = OnceLock::new();

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// The cache key for one binary. The path itself is never stored: the cache is readable by remote clients
/// as an app setting, and a host path is not theirs to see.
fn bin_hash(bin: &str) -> String {
    format!("{:x}", Sha256::digest(bin.as_bytes()))[..16].to_string()
}

// ─────────────────────────── Parsing ───────────────────────────

/// The usable rows of a `models` array: anything without a string `value` is dropped.
pub fn parse_rows(rows: &[Value]) -> Vec<CliModel> {
    rows.iter()
        .filter(|row| row.get("value").and_then(Value::as_str).is_some_and(|v| !v.trim().is_empty()))
        .filter_map(|row| serde_json::from_value::<CliModel>(row.clone()).ok())
        .collect()
}

/// Collects the two answers a probe waits for while the stream is still being read.
#[derive(Default)]
struct StreamParser {
    init: Option<Vec<CliModel>>,
    list: Option<Vec<CliModel>>,
    error: Option<String>,
}

impl StreamParser {
    /// Feed one stdout line. Hook lines, system lines, non-JSON and anything not addressed to the probe
    /// are ignored, exactly as the chat engine ignores lines it does not know.
    fn feed(&mut self, line: &str) {
        let Incoming::ControlResponse { request_id, response, error } = protocol::parse_line(line) else {
            return;
        };
        if request_id != INIT_ID && request_id != LIST_ID {
            return;
        }
        if let Some(message) = error {
            self.error.get_or_insert(message);
            return;
        }
        let rows = response
            .get("models")
            .and_then(Value::as_array)
            .map(|rows| parse_rows(rows))
            .unwrap_or_default();
        if request_id == LIST_ID {
            self.list = Some(rows);
        } else {
            self.init = Some(rows);
        }
    }

    /// Whether the answer the probe prefers has arrived.
    fn complete(&self) -> bool {
        self.list.is_some()
    }

    fn finish(self) -> Result<Vec<CliModel>, ProbeError> {
        match self.list.or(self.init) {
            Some(rows) if !rows.is_empty() => Ok(rows),
            Some(_) => Err(ProbeError::Empty),
            None => match self.error {
                Some(message) => Err(ProbeError::Cli(message)),
                None => Err(ProbeError::Malformed),
            },
        }
    }
}

/// Parse a complete probe transcript. The `list_models` answer wins; the `initialize` answer, which
/// carries the same array, is the fallback when the CLI ended before answering the second request.
/// The probe itself parses incrementally through the same `StreamParser`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_stream<'a>(lines: impl IntoIterator<Item = &'a str>) -> Result<Vec<CliModel>, ProbeError> {
    let mut parser = StreamParser::default();
    for line in lines {
        parser.feed(line);
    }
    parser.finish()
}

// ─────────────────────────── Probe ───────────────────────────

/// Whether `bin` names something that can be spawned, without spawning it. An absolute path must be an
/// executable file; a bare name must be found on PATH. A missing binary is skipped entirely: no process,
/// and no timeout to wait for.
fn locate(bin: &str) -> Result<(), ProbeError> {
    let path = Path::new(bin);
    let present = if path.is_absolute() || path.components().count() > 1 {
        super::executable::is_executable_file(path)
    } else {
        super::executable::find_on_path(bin).is_some()
    };
    if present { Ok(()) } else { Err(ProbeError::MissingBinary) }
}

/// The private working directory of the probe, created on demand and (re)restricted to the owner.
fn work_dir(app: &AppCtx) -> Result<PathBuf, ProbeError> {
    let unavailable = |_| ProbeError::Spawn("workDir".into());
    let path = app.data_dir().map_err(|_| ProbeError::Spawn("workDir".into()))?.join(WORK_DIR);
    std::fs::create_dir_all(&path).map_err(unavailable)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).map_err(unavailable)?;
    }
    Ok(path)
}

/// The probe's command line and environment, without spawning it.
fn probe_command(bin: &str, cwd: &Path) -> std::process::Command {
    let mut cmd = crate::host::command(bin);
    super::executable::prepare_command(&mut cmd, bin);
    cmd.args(PROBE_ARGS);
    cmd.env("NO_COLOR", "1");
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS.iter().chain(SCRUBBED_ENV) {
        cmd.env_remove(key);
    }
    // A private, empty directory: neither the project settings of whatever directory the backend runs
    // in nor settings another local user could plant in a shared directory may shape the probe.
    cmd.current_dir(cwd);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    // Its own process group, so a timeout can end whatever the CLI started along with it.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd
}

/// End the probe and everything in its process group, then reap it. The same pattern as the memory
/// runner's headless launch.
fn kill_tree(child: &mut std::process::Child) {
    // The crate's one process-tree kill (process group on unix, taskkill /T on Windows), then reap.
    crate::host::kill_process_tree(child);
    let _ = child.wait();
}

/// Ask the binary for its models over the control protocol, killing it once `timeout` elapses.
pub fn probe_with_timeout(bin: &str, cwd: &Path, timeout: Duration) -> Result<Vec<CliModel>, ProbeError> {
    locate(bin)?;
    let mut cmd = probe_command(bin, cwd);
    let mut child = cmd.spawn().map_err(|e| ProbeError::Spawn(e.to_string()))?;
    // Both requests up front, then EOF: the CLI exits on its own once stdin closes.
    if let Some(mut stdin) = child.stdin.take() {
        let requests = [
            protocol::control_request(INIT_ID, protocol::initialize()),
            protocol::control_request(LIST_ID, protocol::list_models()),
        ];
        for request in requests {
            let _ = writeln!(stdin, "{request}");
        }
        let _ = stdin.flush();
    }
    let stdout = child.stdout.take();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    // Drain on a helper thread: a CLI that fills the pipe while the parent waits would deadlock.
    std::thread::spawn(move || {
        if let Some(out) = stdout {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        }
    });
    let deadline = Instant::now() + timeout;
    let mut parser = StreamParser::default();
    let mut eof = false;
    while !parser.complete() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(line) => parser.feed(&line),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                eof = true;
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                kill_tree(&mut child);
                return Err(ProbeError::Timeout);
            }
        }
    }
    // Wait for the CLI to end once it has answered; if it lingers past the deadline, kill it.
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                kill_tree(&mut child);
                break None;
            }
        }
    };
    match parser.finish() {
        Ok(rows) => Ok(rows),
        Err(parse_error) => match status {
            // A CLI that ended without answering explains itself through its exit code.
            Some(status) if eof && !status.success() => Err(ProbeError::Exit(status.code().unwrap_or(-1))),
            _ => Err(parse_error),
        },
    }
}

/// Probe with the standard timeout.
pub fn probe(bin: &str, cwd: &Path) -> Result<Vec<CliModel>, ProbeError> {
    probe_with_timeout(bin, cwd, PROBE_TIMEOUT)
}

// ─────────────────────────── Cache ───────────────────────────

/// Merge the stored cache of this app's database into memory, once per database.
fn load(app: &AppCtx) {
    let Ok(dir) = app.data_dir() else { return };
    if state().0.lock().unwrap().loaded.contains(&dir) {
        return;
    }
    let stored = {
        let conn = app.db().conn.lock().unwrap();
        crate::db::repo::get_app_settings(&conn).ok().and_then(|mut m| m.remove(KEY))
    };
    let parsed: HashMap<String, Snapshot> = stored
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let mut guard = state().0.lock().unwrap();
    for (hash, snapshot) in parsed {
        // The earlier rule stored a whole session answer (disabled rows included) as a "live" snapshot's
        // models. That value is only as valid as the rule that wrote it, so it is dropped and re-derived by
        // the next probe or conversation instead of being read as a probe list.
        if snapshot.origin == "live" && !snapshot.models.is_empty() {
            continue;
        }
        guard.snapshots.entry(hash).or_insert(snapshot);
    }
    guard.loaded.insert(dir);
}

/// Write the whole in-memory cache. Called with the state lock held so two writers cannot reorder.
fn persist(app: &AppCtx, snapshots: &HashMap<String, Snapshot>) {
    let Ok(json) = serde_json::to_string(snapshots) else { return };
    let mut entries = HashMap::new();
    entries.insert(KEY.to_string(), json);
    if let Ok(conn) = app.db().conn.lock() {
        let _ = crate::db::repo::set_app_settings(&conn, &entries);
    }
}

/// The version string of `bin`, read once per run unless `force`. None when the binary did not answer.
pub fn version_of(bin: &str, force: bool) -> Option<String> {
    let hash = bin_hash(bin);
    if !force {
        if let Some(cached) = state().0.lock().unwrap().versions.get(&hash) {
            return cached.clone();
        }
    }
    let version = super::claude_models::read_version_for_bin(bin);
    state().0.lock().unwrap().versions.insert(hash, version.clone());
    version
}

/// The version string read this run for `bin`, without spawning anything.
pub fn cached_version(bin: &str) -> Option<String> {
    state().0.lock().unwrap().versions.get(&bin_hash(bin)).cloned().flatten()
}

/// The cached snapshot for `bin`, if any.
#[cfg_attr(not(test), allow(dead_code))]
pub fn cached(app: &AppCtx, bin: &str) -> Option<Snapshot> {
    load(app);
    state().0.lock().unwrap().snapshots.get(&bin_hash(bin)).cloned()
}

/// The snapshot to build the catalogue from, probing as `probe` allows.
///
/// A caller that arrives while another probe for the same binary runs waits for that result instead of
/// starting a second process: the first `chat_models` after a restart should answer with the new list
/// rather than showing the old one and jumping.
pub fn ensure(app: &AppCtx, bin: &str, probe: Probe) -> Option<Snapshot> {
    run(app, bin, probe).0
}

fn run(app: &AppCtx, bin: &str, probe: Probe) -> (Option<Snapshot>, Option<ProbeError>) {
    load(app);
    let hash = bin_hash(bin);
    let (lock, ready) = state();
    if probe == Probe::CacheOnly {
        return (lock.lock().unwrap().snapshots.get(&hash).cloned(), None);
    }
    let version = version_of(bin, probe == Probe::Force);
    {
        let mut guard = lock.lock().unwrap();
        let snapshot = guard.snapshots.get(&hash).cloned();
        if probe == Probe::Allow {
            // A matching version proves the cache current. Without a readable version nothing can tell an
            // update apart, so a snapshot this run produced is kept for the rest of the run instead of
            // probing again on every read; the next start probes afresh.
            // Only a probe's list counts: what running conversations reported is no substitute for it.
            let current = snapshot.as_ref().is_some_and(|s| {
                s.origin == "probe"
                    && !s.models.is_empty()
                    && s.version == version
                    && (version.is_some() || s.checked_at >= run_started())
            });
            let backing_off = guard
                .attempts
                .get(&hash)
                .is_some_and(|a| a.error.is_some() && now().saturating_sub(a.attempted_at) < BACKOFF);
            if current || backing_off {
                return (snapshot, None);
            }
        }
        if guard.in_flight.contains(&hash) {
            // Single flight: wait for the running probe of this binary, at most one probe's worth.
            let deadline = Instant::now() + PROBE_TIMEOUT + Duration::from_secs(1);
            while guard.in_flight.contains(&hash) {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                guard = ready.wait_timeout(guard, remaining).unwrap().0;
            }
            return (guard.snapshots.get(&hash).cloned(), None);
        }
        guard.in_flight.insert(hash.clone());
        guard.probes += 1;
        guard.attempts.insert(hash.clone(), Attempt { attempted_at: now(), error: None });
    }
    let started = Instant::now();
    let result = work_dir(app).and_then(|cwd| self::probe(bin, &cwd));
    let (snapshot, error) = {
        let mut guard = lock.lock().unwrap();
        guard.in_flight.remove(&hash);
        let error = match &result {
            Ok(models) => {
                // Additions the probe now lists itself are redundant; the rest are kept while the CLI version
                // that reported them is unchanged. Refresh starts over, so a stale addition can be cleared.
                let previous = guard.snapshots.get(&hash).filter(|s| probe != Probe::Force && s.live_version == version);
                let live = previous
                    .map(|s| s.live.iter().filter(|row| !models.iter().any(|m| m.value == row.value)).cloned().collect())
                    .unwrap_or_default();
                let snapshot = Snapshot {
                    version: version.clone(),
                    checked_at: now(),
                    origin: "probe".into(),
                    models: models.clone(),
                    live,
                    live_version: previous.and_then(|s| s.live_version.clone()),
                };
                guard.snapshots.insert(hash.clone(), snapshot);
                persist(app, &guard.snapshots);
                None
            }
            Err(error) => Some(error.clone()),
        };
        if let Some(attempt) = guard.attempts.get_mut(&hash) {
            attempt.error = error.as_ref().map(|e| format!("probeFailed:{e}"));
        }
        ready.notify_all();
        (guard.snapshots.get(&hash).cloned(), error)
    };
    audit(&version, error.as_ref(), started.elapsed().as_millis());
    app.emit(EVENT, catalog_status(app));
    (snapshot, error)
}

/// Remember the rows of a running conversation's `list_models` answer as additions for this binary, so
/// menus of sessions that are not running learn a new model the moment one conversation has reported it
/// (`normalize_id` does not read additions; its pairs come from the neutral probe only). The answer may depend on that conversation's project settings, so it
/// never replaces the probe's list and never removes anything: disabled rows and the `default` row, whose
/// target a project can change, are not remembered, and rows the probe already lists add nothing. The
/// version is read once if this run has not read it yet.
pub fn record_live(app: &AppCtx, bin: &str, rows: &[Value]) {
    let rows: Vec<CliModel> = parse_rows(rows).into_iter().filter(|r| !r.disabled && r.value != "default").collect();
    if rows.is_empty() {
        return;
    }
    load(app);
    let version = version_of(bin, false);
    let changed = {
        let mut guard = state().0.lock().unwrap();
        let snapshot = guard.snapshots.entry(bin_hash(bin)).or_insert_with(|| Snapshot {
            version: version.clone(),
            checked_at: now(),
            origin: "live".into(),
            models: Vec::new(),
            live: Vec::new(),
            live_version: None,
        });
        let mut changed = false;
        // Additions from an older CLI version do not describe this binary any more.
        if snapshot.live_version != version {
            changed |= !snapshot.live.is_empty();
            snapshot.live.clear();
            snapshot.live_version = version.clone();
        }
        for row in rows {
            if snapshot.models.iter().any(|m| m.value == row.value) {
                continue;
            }
            match snapshot.live.iter_mut().find(|m| m.value == row.value) {
                Some(known) if *known == row => {}
                Some(known) => {
                    *known = row;
                    changed = true;
                }
                None => {
                    snapshot.live.push(row);
                    changed = true;
                }
            }
        }
        if changed {
            persist(app, &guard.snapshots);
        }
        changed
    };
    if changed {
        app.emit(EVENT, catalog_status(app));
    }
}

/// Every `(value, resolvedModel)` pair the CLI has reported, newest snapshot first. A row without a
/// `resolvedModel` resolves to its own value, exactly as the menu lists it (`cli_models` takes the same
/// fallback), so an identifier the CLI offers is never rewritten by the static table. Empty until a probe
/// of some binary has succeeded (conversations' answers do not count), which is how `normalize_id` knows
/// to fall back to that table.
pub fn alias_pairs() -> Vec<(String, String)> {
    let guard = state().0.lock().unwrap();
    let mut snapshots: Vec<&Snapshot> = guard.snapshots.values().collect();
    snapshots.sort_by(|a, b| b.checked_at.cmp(&a.checked_at));
    // Only the neutral probe's rows: a conversation's answer depends on its project settings, so its
    // value-to-model pairs must not decide how every other session's launch model is resolved. Its new
    // identifiers still reach every menu as additions.
    pairs_of(snapshots.iter().flat_map(|s| s.models.iter()))
}

/// `alias_pairs` over explicit rows. The `default` row is not a value anyone passes, but its target is
/// listed as a menu row of its own when no other row offers it (`cli_models`), so the target counts as an
/// offered identifier and pairs with itself.
pub(crate) fn pairs_of<'a>(rows: impl Iterator<Item = &'a CliModel>) -> Vec<(String, String)> {
    rows.filter(|m| !m.disabled)
        .filter_map(|m| {
            let resolved = m.resolved_model.clone().unwrap_or_else(|| m.value.clone());
            if m.value == "default" {
                (resolved != "default").then(|| (resolved.clone(), resolved))
            } else {
                Some((m.value.clone(), resolved))
            }
        })
        .collect()
}

fn audit(version: &Option<String>, error: Option<&ProbeError>, duration: u128) {
    let level = if error.is_some() { "WARN" } else { "INFO" };
    if !crate::diagnostics::enabled(
        &std::env::var("VLX_MODEL_CATALOG_LOG_LEVEL").unwrap_or_else(|_| "info".into()),
        level,
    ) {
        return;
    }
    crate::diagnostics::record(
        level,
        "model_catalog_probe",
        serde_json::json!({
            "status": if error.is_some() { "failed" } else { "success" },
            "version": version,
            "durationMs": duration as u64,
            "error": error.map(|e| e.to_string()),
        }),
    );
}

// ─────────────────────────── Status, refresh, start ───────────────────────────

/// The Claude binary the status line and the start-up probe refer to: the configured or discovered one.
fn default_bin(app: &AppCtx) -> String {
    super::executable::resolve(app, crate::models::SessionKind::Claude, None)
        .or_else(|| super::install::locate_installed_bin("claude"))
        .unwrap_or_else(|| "claude".to_string())
}

/// What the status line shows: the CLI snapshot of the default binary when there is one, otherwise the
/// website module's status. A probe failure without any snapshot rides on the fallback status as its
/// error, so the line still says the update failed.
pub fn catalog_status(app: &AppCtx) -> Status {
    let bin = default_bin(app);
    load(app);
    let hash = bin_hash(&bin);
    let guard = state().0.lock().unwrap();
    let attempt = guard.attempts.get(&hash);
    match guard.snapshots.get(&hash) {
        Some(snapshot) => Status {
            revision: None,
            source: "cli".into(),
            checked_at: Some(snapshot.checked_at),
            error: attempt.and_then(|a| a.error.clone()),
            refreshing: guard.in_flight.contains(&hash),
            cli_version: snapshot.version.clone(),
        },
        None => {
            let mut status = remote_model_catalog::status();
            if status.error.is_none() {
                status.error = attempt.and_then(|a| a.error.clone());
            }
            status.refreshing = status.refreshing || guard.in_flight.contains(&hash);
            status
        }
    }
}

/// The Refresh button: probe the CLI first; when that fails, refresh the website catalogue instead.
pub fn catalog_refresh(app: &AppCtx) -> Status {
    let bin = default_bin(app);
    let (_, error) = run(app, &bin, Probe::Force);
    if error.is_some() {
        remote_model_catalog::refresh(app);
    }
    catalog_status(app)
}

/// Load the stored cache and probe the default binary once in the background, so the first menu after
/// a Claude Code update already shows the new list. No interval: the list changes only with the version
/// or the account, which Refresh and running conversations cover.
pub fn start(ctx: AppCtx) {
    if STARTED.set(()).is_err() {
        return;
    }
    std::thread::spawn(move || {
        load(&ctx);
        let bin = default_bin(&ctx);
        ensure(&ctx, &bin, Probe::Allow);
    });
}

#[cfg(test)]
pub(crate) fn probe_count() -> u64 {
    state().0.lock().unwrap().probes
}

/// Forget everything this run learned about `bin` and re-read `app`'s stored cache on the next call.
#[cfg(test)]
pub(crate) fn reset_for_tests(app: &AppCtx, bin: &str) {
    let hash = bin_hash(bin);
    let mut guard = state().0.lock().unwrap();
    guard.snapshots.remove(&hash);
    guard.versions.remove(&hash);
    guard.attempts.remove(&hash);
    if let Ok(dir) = app.data_dir() {
        guard.loaded.remove(&dir);
    }
}

/// Serializes tests that read or set state shared beyond one binary (the website module, alias pairs).
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    pub(crate) const FIXTURE: &str = include_str!("testdata/claude_probe_2.1.280.json");

    /// The recorded `models` array of the fixture (identical in both responses).
    pub(crate) fn fixture_models() -> Vec<Value> {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        fixture["responses"][1]["response"]["response"]["models"].as_array().unwrap().clone()
    }

    /// The two recorded control responses as the CLI printed them, one per line.
    pub(crate) fn fixture_lines() -> Vec<String> {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        fixture["responses"].as_array().unwrap().iter().map(|r| serde_json::to_string(r).unwrap()).collect()
    }

    pub(crate) fn headless(tag: &str) -> (AppCtx, PathBuf) {
        let dir = std::env::temp_dir().join(format!("vlx-cli-catalog-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let app = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(dir.clone(), db)));
        (app, dir)
    }

    /// Write the fake CLI into `dir` with the given CONFIG and return its path.
    #[cfg(unix)]
    pub(crate) fn fake_bin(dir: &Path, config: Value) -> String {
        use std::os::unix::fs::PermissionsExt;
        let script = include_str!("testdata/claude_probe.py").replace("__CONFIG__", &config.to_string());
        let path = dir.join("claude");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// The logged invocations of the fake CLI in `dir`.
    pub(crate) fn calls(dir: &Path) -> Vec<Value> {
        std::fs::read_to_string(dir.join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    /// The headless probes among the logged calls (a chat launch also speaks stream-json, with other args).
    pub(crate) fn probe_calls(dir: &Path) -> Vec<Value> {
        calls(dir).into_iter().filter(|c| c["args"] == serde_json::json!(PROBE_ARGS)).collect()
    }

    /// Make `bin` the configured Claude executable of `app`, so status and refresh refer to it.
    pub(crate) fn configure_bin(app: &AppCtx, bin: &str) {
        let conn = app.db().conn.lock().unwrap();
        let settings = serde_json::json!({"agentDefaults": {"claude": {"path": bin}}}).to_string();
        crate::db::repo::set_app_settings(&conn, &HashMap::from([("vlx-settings".to_string(), settings)])).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn parses_both_recorded_responses_and_ignores_hook_lines() {
        let recorded = fixture_lines();
        let mut lines: Vec<&str> = vec![
            r#"{"type":"system","subtype":"hook_started","hook_name":"SessionStart","hook_id":"h1"}"#,
            "not json at all",
            "",
            &recorded[0],
            r#"{"type":"system","subtype":"hook_response","hook_id":"h1","output":"ok"}"#,
            &recorded[1],
            r#"{"type":"result","subtype":"success"}"#,
        ];
        let models = parse_stream(lines.iter().copied()).unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].value, "default");
        assert_eq!(models[0].resolved_model.as_deref(), Some("claude-opus-5-5[1m]"));
        assert_eq!(models[1].value, "opus[1m]");
        assert_eq!(models[1].resolved_model.as_deref(), Some("claude-opus-5-5[1m]"));
        assert_eq!(models[1].display_name, "Opus (1M context)");
        assert!(models[1].supports_fast_mode);
        assert_eq!(models[1].supported_effort_levels, ["low", "medium", "high", "xhigh", "max"]);
        assert_eq!(models[2].value, "claude-fable-5-1");
        assert!(!models[2].supports_fast_mode, "the fixture row has no supportsFastMode");
        assert!(!models.iter().any(|m| m.disabled));
        // The initialize answer alone carries the same list.
        lines.truncate(4);
        assert_eq!(parse_stream(lines.iter().copied()).unwrap(), models);
    }

    #[test]
    fn malformed_stream_is_an_error_not_a_list() {
        assert_eq!(parse_stream(["garbage", "{\"type\":\"system\"}"]).unwrap_err(), ProbeError::Malformed);
        assert_eq!(parse_stream(Vec::<&str>::new()).unwrap_err(), ProbeError::Malformed);
        let error = r#"{"type":"control_response","response":{"subtype":"error","request_id":"probe-init","error":"Not logged in"}}"#;
        assert_eq!(parse_stream([error]).unwrap_err(), ProbeError::Cli("Not logged in".into()));
        let empty = r#"{"type":"control_response","response":{"subtype":"success","request_id":"probe-list","response":{"models":[]}}}"#;
        assert_eq!(parse_stream([empty]).unwrap_err(), ProbeError::Empty);
        // An answer to somebody else's request is not ours.
        let foreign = r#"{"type":"control_response","response":{"subtype":"success","request_id":"other","response":{"models":[{"value":"x"}]}}}"#;
        assert_eq!(parse_stream([foreign]).unwrap_err(), ProbeError::Malformed);
        // Rows without a value are dropped; unknown fields are tolerated.
        let rows = parse_rows(&[serde_json::json!({"displayName":"no value"}), serde_json::json!({"value":"a","unknownField":1})]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].value, "a");
    }

    #[test]
    fn missing_binary_is_skipped_without_spawn_and_backs_off() {
        let (app, dir) = headless("missing");
        let bin = dir.join("does-not-exist").to_string_lossy().into_owned();
        assert_eq!(probe_with_timeout(&bin, &dir, Duration::from_secs(1)).unwrap_err(), ProbeError::MissingBinary);
        let before = probe_count();
        assert!(ensure(&app, &bin, Probe::Allow).is_none());
        assert_eq!(probe_count(), before + 1);
        let status_error = state().0.lock().unwrap().attempts.get(&bin_hash(&bin)).unwrap().error.clone();
        assert_eq!(status_error.as_deref(), Some("probeFailed:missingBinary"));
        // Inside the backoff window the same binary is not asked again.
        assert!(ensure(&app, &bin, Probe::Allow).is_none());
        assert_eq!(probe_count(), before + 1);
        // Force ignores the backoff.
        assert!(ensure(&app, &bin, Probe::Force).is_none());
        assert_eq!(probe_count(), before + 2);
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn probe_reads_the_fake_binary_without_session_identity() {
        let (app, dir) = headless("probe");
        let bin = fake_bin(&dir, serde_json::json!({"models": fixture_models()}));
        // The backend may itself run inside a VelaTerm or Claude session; the probe must not inherit that.
        let cwd = work_dir(&app).unwrap();
        let removed: Vec<String> = probe_command(&bin, &cwd)
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        for key in ["VLX_SESSION_ID", "VLX_TOKEN", "VLX_SPAWN_URL", "VLX_EXE", "VLX_BIN_DIR", "CLAUDECODE", "CLAUDE_CODE_SESSION_ID", "VLX_CLAUDE_SETTINGS"] {
            assert!(removed.contains(&key.to_string()), "{key} must be removed from the probe environment");
        }
        let models = probe(&bin, &cwd).unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(models[1].resolved_model.as_deref(), Some("claude-opus-5-5[1m]"));
        let calls = probe_calls(&dir);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["args"], serde_json::json!(PROBE_ARGS));
        let env: Vec<&str> = calls[0]["env"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        for forbidden in ["VLX_SESSION_ID", "VLX_TOKEN", "VLX_SPAWN_URL", "CLAUDECODE"] {
            assert!(!env.contains(&forbidden), "{forbidden} leaked into the probe");
        }
        // Hooks, MCP servers and transcripts stay off; the model list does not depend on them.
        let args: Vec<&str> = calls[0]["args"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        let hooks = args.iter().position(|a| *a == "--settings").expect("--settings must be passed");
        assert_eq!(serde_json::from_str::<Value>(args[hooks + 1]).unwrap(), serde_json::json!({"disableAllHooks": true}));
        for flag in ["--strict-mcp-config", "--no-session-persistence"] {
            assert!(args.contains(&flag), "{flag} must be passed");
        }
        // The probe runs in a private directory below the data dir, never in a shared one such as /tmp.
        let ran_in = Path::new(calls[0]["cwd"].as_str().unwrap()).canonicalize().unwrap();
        assert_eq!(ran_in, dir.join(WORK_DIR).canonicalize().unwrap());
        assert_ne!(ran_in, std::env::temp_dir().canonicalize().unwrap());
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&ran_in).unwrap().permissions().mode() & 0o777, 0o700);
            // A directory left wider by an older run is narrowed again.
            std::fs::set_permissions(&ran_in, std::fs::Permissions::from_mode(0o777)).unwrap();
            work_dir(&app).unwrap();
            assert_eq!(std::fs::metadata(&ran_in).unwrap().permissions().mode() & 0o777, 0o700);
        }
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn probe_times_out_and_kills_the_process() {
        let (app, dir) = headless("timeout");
        let bin = fake_bin(&dir, serde_json::json!({"sleep": 30, "models": fixture_models()}));
        let started = Instant::now();
        assert_eq!(probe_with_timeout(&bin, &dir, Duration::from_secs(1)).unwrap_err(), ProbeError::Timeout);
        assert!(started.elapsed() < Duration::from_secs(3));
        let pid = probe_calls(&dir)[0]["pid"].as_u64().unwrap();
        assert!(!alive(pid), "the timed-out CLI must not survive the probe");
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    fn alive(pid: u64) -> bool {
        std::process::Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().unwrap().success()
    }

    /// AC3: the timeout ends the CLI's whole process group, not only the direct child.
    #[cfg(unix)]
    #[test]
    fn probe_timeout_kills_the_whole_process_group() {
        let (app, dir) = headless("group");
        let bin = fake_bin(&dir, serde_json::json!({"sleep": 30, "grandchild": true, "models": fixture_models()}));
        assert_eq!(probe_with_timeout(&bin, &dir, Duration::from_secs(2)).unwrap_err(), ProbeError::Timeout);
        let grandchild: u64 = std::fs::read_to_string(dir.join("grandchild.pid")).expect("the fake CLI started its grandchild").trim().parse().unwrap();
        // The orphaned grandchild is reaped by init once killed; give that a moment.
        let deadline = Instant::now() + Duration::from_secs(3);
        while alive(grandchild) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        let survived = alive(grandchild);
        if survived {
            let _ = std::process::Command::new("kill").args(["-9", &grandchild.to_string()]).status();
        }
        assert!(!survived, "the CLI's grandchild must not survive the probe's timeout");
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn failure_modes_fall_back_without_a_list() {
        let (app, dir) = headless("failures");
        for (mode, expected) in [("garbage", ProbeError::Malformed), ("error", ProbeError::Cli("Not logged in".into())), ("exit3", ProbeError::Exit(3))] {
            let bin = fake_bin(&dir, serde_json::json!({"mode": mode, "models": fixture_models()}));
            assert_eq!(probe(&bin, &dir).unwrap_err(), expected, "mode {mode}");
        }
        let bin = fake_bin(&dir, serde_json::json!({"models": []}));
        assert_eq!(probe(&bin, &dir).unwrap_err(), ProbeError::Empty);
        assert!(ensure(&app, &bin, Probe::Allow).is_none());
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn single_flight_per_binary() {
        let (app, dir) = headless("single-flight");
        let bin = fake_bin(&dir, serde_json::json!({"sleep": 2, "models": fixture_models()}));
        let results: Vec<Option<Snapshot>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2).map(|_| scope.spawn(|| ensure(&app, &bin, Probe::Allow))).collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(results.iter().all(|r| r.is_some()));
        assert_eq!(results[0], results[1]);
        assert_eq!(results[0].as_ref().unwrap().models.len(), 3);
        assert_eq!(probe_calls(&dir).len(), 1, "two concurrent readers must share one probe");
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A CLI whose `--version` cannot be parsed is probed once per run, not on every catalogue read, and
    /// its stored snapshot from an earlier run is not trusted.
    #[cfg(unix)]
    #[test]
    fn unreadable_version_probes_once_per_run() {
        let (app, dir) = headless("no-version");
        let bin = fake_bin(&dir, serde_json::json!({"version": "unknown", "models": fixture_models()}));
        let old = Snapshot { version: None, checked_at: 1, origin: "probe".into(), models: vec![CliModel { value: "claude-opus-5".into(), ..Default::default() }], live: vec![], live_version: None };
        state().0.lock().unwrap().snapshots.insert(bin_hash(&bin), old);
        let first = ensure(&app, &bin, Probe::Allow).unwrap();
        assert!(first.version.is_none());
        assert_eq!(first.models.len(), 3, "a snapshot from an earlier run is re-probed");
        assert_eq!(ensure(&app, &bin, Probe::Allow).unwrap(), first);
        assert_eq!(probe_calls(&dir).len(), 1, "no probe storm without a version");
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// AC9a: a cache written by an older CLI is replaced once `--version` reports a newer one, and the
    /// replacement is reused across a restart (memory cleared, database kept) without a second probe.
    #[cfg(unix)]
    #[test]
    fn cache_is_reused_for_the_same_version_and_replaced_after_an_update() {
        let (app, dir) = headless("cache");
        let bin = fake_bin(&dir, serde_json::json!({"version": "2.1.280", "models": fixture_models()}));
        let stale = Snapshot {
            version: Some("2.1.200".into()),
            checked_at: 1,
            origin: "probe".into(),
            models: vec![CliModel { value: "claude-opus-5".into(), resolved_model: Some("claude-opus-5".into()), ..Default::default() }],
            live: vec![],
            live_version: None,
        };
        {
            let conn = app.db().conn.lock().unwrap();
            let map = HashMap::from([(bin_hash(&bin), stale)]);
            crate::db::repo::set_app_settings(&conn, &HashMap::from([(KEY.to_string(), serde_json::to_string(&map).unwrap())])).unwrap();
        }
        let models = super::super::claude_models::list_for_bin(&app, &bin);
        assert!(models.iter().any(|m| m.id == "claude-opus-5-5" && m.is_default), "the new list must replace the stale cache");
        assert!(!models.iter().any(|m| m.id == "claude-opus-5-5[1m]"), "the CLI's spelling folds onto the catalogue's");
        let stored = {
            let conn = app.db().conn.lock().unwrap();
            crate::db::repo::get_app_settings(&conn).unwrap().remove(KEY).unwrap()
        };
        let stored: HashMap<String, Snapshot> = serde_json::from_str(&stored).unwrap();
        assert_eq!(stored[&bin_hash(&bin)].version.as_deref(), Some("2.1.280"));
        assert_eq!(probe_calls(&dir).len(), 1);
        // A restart: memory is gone, the database is not. The same version reuses the cache.
        reset_for_tests(&app, &bin);
        let again = super::super::claude_models::list_for_bin(&app, &bin);
        assert!(again.iter().any(|m| m.id == "claude-opus-5-5" && m.is_default));
        assert_eq!(probe_calls(&dir).len(), 1, "same version, no second probe");
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// AC4: a running conversation's answer never replaces the probe's list. Its identifiers are
    /// remembered as additions, which feed the alias pairs; disabled and `default` rows are not.
    #[cfg(unix)]
    #[test]
    fn record_live_only_adds_to_the_probe_snapshot() {
        let (app, dir) = headless("live");
        let bin = fake_bin(&dir, serde_json::json!({"version": "2.1.280"}));
        let new_model = serde_json::json!({"value": "claude-opus-6", "resolvedModel": "claude-opus-6", "displayName": "Opus"});
        // Before any probe: the additions stand alone, and the probe is still owed.
        record_live(&app, &bin, &[new_model.clone(), serde_json::json!({"value": "default", "resolvedModel": "claude-sonnet-5"})]);
        let snapshot = cached(&app, &bin).unwrap();
        assert_eq!(snapshot.origin, "live");
        assert!(snapshot.models.is_empty());
        assert_eq!(snapshot.live.iter().map(|m| m.value.as_str()).collect::<Vec<_>>(), ["claude-opus-6"]);
        assert_eq!(snapshot.live_version.as_deref(), Some("2.1.280"));
        // A conversation's answer adds menu entries but never feeds the alias pairs of every session.
        assert!(!alias_pairs().iter().any(|(v, _)| v == "claude-opus-6"));
        // The probe's list arrives and keeps the additions it does not list itself.
        let hash = bin_hash(&bin);
        {
            let mut guard = state().0.lock().unwrap();
            let entry = guard.snapshots.get_mut(&hash).unwrap();
            entry.origin = "probe".into();
            entry.models = parse_rows(&fixture_models());
        }
        // A narrow answer (one row, one disabled row) neither shrinks nor disables anything.
        record_live(&app, &bin, &[
            serde_json::json!({"value": "claude-fable-5-1", "resolvedModel": "claude-fable-5-1"}),
            serde_json::json!({"value": "opus[1m]", "resolvedModel": "claude-opus-5-5[1m]", "disabled": true}),
        ]);
        let snapshot = cached(&app, &bin).unwrap();
        assert_eq!(snapshot.models.len(), 3, "the probe's rows are untouched");
        assert_eq!(snapshot.live.len(), 1, "rows the probe lists and disabled rows are not remembered");
        // The pairs come from the probe's rows.
        assert!(alias_pairs().contains(&("opus[1m]".to_string(), "claude-opus-5-5[1m]".to_string())));
        assert!(!alias_pairs().iter().any(|(v, _)| v == "default"));
        // The engine's chokepoint reads these pairs from the shared state and folds the result.
        assert_eq!(super::super::claude_models::normalize_id("opus[1m]"), "claude-opus-5-5");
        assert_eq!(super::super::claude_models::normalize_id("claude-opus-5-5[1m]"), "claude-opus-5-5");
        // An empty answer changes nothing.
        record_live(&app, &bin, &[]);
        assert_eq!(cached(&app, &bin).unwrap(), snapshot);
        // Additions expire with the CLI version that reported them: a conversation of a newer binary starts
        // over instead of inheriting identifiers an older version once named.
        {
            let mut guard = state().0.lock().unwrap();
            guard.snapshots.get_mut(&hash).unwrap().live_version = Some("2.1.279".into());
        }
        record_live(&app, &bin, &[serde_json::json!({"value": "claude-sonnet-6", "resolvedModel": "claude-sonnet-6"})]);
        let snapshot = cached(&app, &bin).unwrap();
        assert_eq!(snapshot.live.iter().map(|m| m.value.as_str()).collect::<Vec<_>>(), ["claude-sonnet-6"]);
        assert_eq!(snapshot.live_version.as_deref(), Some("2.1.280"));
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A cache written by the earlier rule (a whole session answer stored as a "live" snapshot's models) is
    /// dropped on load and re-derived, not read as a probe list; a current live snapshot survives.
    #[test]
    fn load_drops_snapshots_written_by_the_earlier_live_rule() {
        let (app, dir) = headless("legacy-live");
        let legacy = Snapshot { version: Some("2.1.280".into()), checked_at: 5, origin: "live".into(), models: parse_rows(&fixture_models()), live: vec![], live_version: None };
        let current = Snapshot { version: Some("2.1.280".into()), checked_at: 6, origin: "live".into(), models: vec![], live: vec![CliModel { value: "claude-opus-6".into(), ..Default::default() }], live_version: Some("2.1.280".into()) };
        {
            let conn = app.db().conn.lock().unwrap();
            let stored: HashMap<String, Snapshot> = HashMap::from([("legacy-hash".to_string(), legacy), ("current-hash".to_string(), current.clone())]);
            crate::db::repo::set_app_settings(&conn, &HashMap::from([(KEY.to_string(), serde_json::to_string(&stored).unwrap())])).unwrap();
        }
        load(&app);
        let guard = state().0.lock().unwrap();
        assert!(!guard.snapshots.contains_key("legacy-hash"), "the old-rule snapshot is dropped");
        assert_eq!(guard.snapshots.get("current-hash"), Some(&current));
        drop(guard);
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn status_and_refresh_fall_back_to_the_website_module() {
        let _serial = TEST_LOCK.lock().unwrap();
        let (app, dir) = headless("status");
        let bin = fake_bin(&dir, serde_json::json!({"version": "2.1.280", "models": fixture_models()}));
        configure_bin(&app, &bin);
        let status = catalog_status(&app);
        assert_ne!(status.source, "cli");
        assert!(status.cli_version.is_none());
        // Refresh probes the CLI first.
        std::env::set_var("VLX_MODEL_CATALOG_URL", "not-a-url");
        let status = catalog_refresh(&app);
        assert_eq!(status.source, "cli");
        assert_eq!(status.cli_version.as_deref(), Some("2.1.280"));
        assert!(status.error.is_none());
        assert!(status.checked_at.is_some());
        assert_eq!(probe_calls(&dir).len(), 1);
        // A failing probe keeps the previous list, reports the error and refreshes the website instead.
        let bin = fake_bin(&dir, serde_json::json!({"version": "2.1.280", "mode": "exit3"}));
        let status = catalog_refresh(&app);
        assert_eq!(status.source, "cli");
        assert_eq!(status.error.as_deref(), Some("probeFailed:exit:3"));
        assert_eq!(cached(&app, &bin).unwrap().models.len(), 3);
        assert!(remote_model_catalog::status().error.is_some(), "the website refresh ran and failed offline");
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// AC4 / AC9b: a running conversation's `list_models` answer shapes its own menu, merged into the
    /// catalogue; a narrow answer does not shrink the menu of another session of the same binary, whose
    /// menu comes from the neutral probe; and a new identifier the live answer names still appears there.
    #[cfg(unix)]
    #[test]
    fn live_answer_is_scoped_to_its_session_and_only_adds_elsewhere() {
        let (app, dir) = headless("ac9b");
        if let AppCtx::Headless(host) = &app {
            // The fixture never calls hooks; this endpoint does not open a listener.
            host.set_hooks(crate::agent::server::HookServer { port: 19191, token: "unused-fixture-token".into() });
        }
        // The running session's answer: a new model as its default, and Fable disabled by its project.
        let rows = vec![
            serde_json::json!({"value": "default", "resolvedModel": "test-live-model", "displayName": "Default (recommended)"}),
            serde_json::json!({"value": "test-live-model", "resolvedModel": "test-live-model", "displayName": "Live", "description": "Live model", "supportedEffortLevels": ["low", "high"]}),
            serde_json::json!({"value": "claude-fable-5-1", "resolvedModel": "claude-fable-5-1", "disabled": true}),
        ];
        let bin = fake_bin(&dir, serde_json::json!({"version": "2.1.280", "models": rows}));
        configure_bin(&app, &bin);
        {
            let conn = app.db().conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,name,root_path,created_at) VALUES ('p','test',?1,0)", [dir.to_str().unwrap()]).unwrap();
            for id in ["running", "idle"] {
                conn.execute("INSERT INTO sessions(id,project_id,name,kind,engine,permission_mode,agent_path,created_at) VALUES (?1,'p','test','claude','chat','default',?2,0)", [id, bin.as_str()]).unwrap();
            }
        }
        app.chat()
            .start(&app, "running", crate::models::SessionKind::Claude, dir.to_str(), &bin, None, None, None, Some("default"), None, &[], false)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.chat().live_claude_models("running").is_none() {
            assert!(Instant::now() < deadline, "the fixture never answered list_models");
            std::thread::sleep(Duration::from_millis(50));
        }
        let ids = |value: &Value| value.as_array().unwrap().iter().map(|m| m["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        let running = crate::command_core::chat_models(&app, "running").unwrap();
        let running_ids = ids(&running);
        assert_eq!(running_ids.iter().filter(|id| id.as_str() == "test-live-model").count(), 1);
        assert!(running_ids.contains(&"claude-opus-4-8".to_string()), "the catalogue stays the base");
        assert!(!running_ids.contains(&"claude-fable-5-1".to_string()), "its own disabled row shapes its own menu");
        let live_row = running.as_array().unwrap().iter().find(|m| m["id"] == "test-live-model").unwrap();
        assert_eq!(live_row["isDefault"], true);
        assert!(probe_calls(&dir).is_empty(), "a running session's menu spawns nothing");
        while !cached(&app, &bin).is_some_and(|s| s.live.iter().any(|m| m.value == "test-live-model")) {
            assert!(Instant::now() < deadline, "the live answer never reached the cache");
            std::thread::sleep(Duration::from_millis(50));
        }
        // The resting session: the neutral probe feeds its menu, now with a probe answer that does not
        // know the new model and lists Fable. The live answer adds its model; it does not remove Fable.
        let neutral = vec![serde_json::json!({"value": "claude-fable-5-1", "resolvedModel": "claude-fable-5-1", "displayName": "Fable"})];
        fake_bin(&dir, serde_json::json!({"version": "2.1.280", "models": neutral}));
        let idle = crate::command_core::chat_models(&app, "idle").unwrap();
        let idle_ids = ids(&idle);
        assert!(idle_ids.contains(&"test-live-model".to_string()), "a new model reported by one conversation appears in every menu");
        assert!(idle_ids.contains(&"claude-fable-5-1".to_string()), "one session's narrow answer does not shrink another's menu");
        assert!(idle_ids.contains(&"claude-opus-4-8".to_string()));
        assert!(!idle.as_array().unwrap().iter().any(|m| m["isDefault"] == true && m["id"] == "test-live-model"), "another session's default is not this one's");
        assert_eq!(probe_calls(&dir).len(), 1, "the resting session is served by the neutral probe");
        let status = catalog_status(&app);
        assert_eq!(status.source, "cli");
        assert_eq!(status.cli_version.as_deref(), Some("2.1.280"));
        let _ = app.chat().stop(&app, "running");
        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// AC2, the chip path: a resting Claude session whose stored selection uses another spelling of a
    /// catalogue model shows the catalogue's identifier, so the chip matches the menu row. Other kinds
    /// keep their stored spelling.
    #[test]
    fn resting_session_chip_shows_the_folded_stored_selection() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (app, dir) = headless("chip");
        {
            let conn = app.db().conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,name,root_path,created_at) VALUES ('p','test',?1,0)", [dir.to_str().unwrap()]).unwrap();
            for (id, kind, args) in [
                ("opus", "claude", "--model claude-opus-5-5[1m]"),
                ("haiku", "claude", "--model claude-haiku-4-5-20251001"),
                ("expanded", "claude", "--model claude-opus-4-6[1m]"),
                ("alias", "claude", "--model opus"),
                ("codex", "codex", "--model claude-opus-5-5[1m]"),
            ] {
                conn.execute(
                    "INSERT INTO sessions(id,project_id,name,kind,engine,permission_mode,agent_args,created_at) VALUES (?1,'p','test',?2,'chat','default',?3,0)",
                    [id, kind, args],
                )
                .unwrap();
            }
        }
        for (id, shown) in [
            ("opus", "claude-opus-5-5"),
            ("haiku", "claude-haiku-4-5"),
            ("expanded", "claude-opus-4-6[1m]"),
            // An alias is the user's choice: shown and later persisted as written, never pinned.
            ("alias", "opus"),
            ("codex", "claude-opus-5-5[1m]"),
        ] {
            let snapshot = crate::command_core::chat_snapshot(&app, id).unwrap();
            assert!(!snapshot.running, "{id}");
            assert_eq!(snapshot.model.as_deref(), Some(shown), "{id}");
            assert_eq!(snapshot.selection.as_ref().and_then(|s| s.model.as_deref()), Some(shown), "{id}");
        }
        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Proves parser and probe against the real installation. Hooks are disabled for the probe and no
    /// user message is sent, so it costs no API tokens. Run with
    /// `VLX_LIVE_PROBE=1 cargo test probe_against_installed_cli -- --ignored`.
    #[test]
    #[ignore]
    fn probe_against_installed_cli() {
        if std::env::var("VLX_LIVE_PROBE").is_err() {
            return;
        }
        let bin = super::super::install::locate_installed_bin("claude").unwrap_or_else(|| "claude".into());
        let cwd = std::env::temp_dir().join(format!("vlx-live-probe-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&cwd).unwrap();
        let models = probe(&bin, &cwd).unwrap();
        let _ = std::fs::remove_dir_all(&cwd);
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.value == "default" && m.resolved_model.is_some()));
    }
}
