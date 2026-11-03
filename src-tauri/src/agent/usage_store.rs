//! One account-level usage snapshot for the whole process, refreshed by a single background poller.
//!
//! Usage quotas are per **account**, not per session, so every session used to ask the same question and
//! every extra client multiplied the load on rate-limited endpoints. The snapshot now lives here: one
//! background thread polls each provider whose account is present on this machine, stores the result, and
//! emits [`USAGE_EVENT`]. Sessions, windows, browser clients, and mobile all read that one copy through
//! `usage_snapshot`, and never fetch on their own.
//!
//! Two settings drive the poller, read live from the shared `vlx-settings` block so a change applies
//! without a restart: `usageAutoRefresh` (on by default) and `usageRefreshSec` (300 by default).
//!
//! The last snapshot is persisted in `app_settings`, so a restart shows the previous numbers immediately
//! instead of leaving the panel blank until the first poll returns.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::agent::transcript::CodexUsage;
use crate::agent::usage::{ClaudeUsage, FetchError, GrokUsage, ZcodeUsage};
use crate::host::AppCtx;
use crate::models::SessionKind;

/// Broadcast whenever the stored snapshot changes. Payload is the full [`UsageSnapshot`].
pub const USAGE_EVENT: &str = "usage://changed";

/// `app_settings` key holding the last snapshot as JSON, so a restart can paint before the first poll.
const SNAPSHOT_KEY: &str = "usage.snapshot";

/// Frontend settings block shared across shells; the poller reads its two usage keys.
const VLX_SETTINGS_KEY: &str = "vlx-settings";

/// Defaults, matching `SETTINGS_DEFAULTS` in the frontend: polling on, every five minutes.
const DEFAULT_AUTO: bool = true;
const DEFAULT_INTERVAL_SEC: u64 = 300;
/// Floor for the configured interval. The Claude endpoint is rate-limited hard enough that anything
/// shorter buys nothing; the settings UI offers no smaller value either.
const MIN_INTERVAL_SEC: u64 = 30;

/// How often the poller wakes to compare wall clocks. Short enough that a laptop coming back from sleep
/// refetches within seconds, cheap enough to ignore.
const TICK: Duration = Duration::from_secs(15);

/// Consecutive failures after which a provider backs off. The delay grows to `1 << MAX_BACKOFF_STEPS`
/// times the configured interval, so a machine with no credentials stops retrying every few minutes.
const MAX_BACKOFF_STEPS: u32 = 3;

/// Upper bound on a server's `Retry-After` we are willing to honor. Anthropic has been seen asking for
/// about 45 minutes; anything past an hour is treated as a broken header.
const MAX_RETRY_AFTER_SECS: u64 = 3600;

/// One provider's stored result: the data, why the last attempt failed, and when it was taken.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageEntry<T> {
    /// Last successful reading; kept while later attempts fail so the panel does not blink empty.
    pub data: Option<T>,
    /// Last failure message, cleared by the next success. With `data` present the panel shows it as a
    /// stale marker next to the old numbers; without `data` it replaces the rows.
    pub error: Option<String>,
    /// Unix milliseconds of the last successful reading.
    pub fetched_at: Option<u64>,
    /// Unix milliseconds of the failure in `error`, so the panel can say how long the numbers are stale.
    /// Defaults so snapshots persisted before this field existed still load.
    #[serde(default)]
    pub error_at: Option<u64>,
}

/// Hand-written so an entry defaults to empty for payload types that have no `Default` of their own.
impl<T> Default for UsageEntry<T> {
    fn default() -> Self {
        Self {
            data: None,
            error: None,
            fetched_at: None,
            error_at: None,
        }
    }
}

/// The account-level snapshot shared by every session and client.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub claude: UsageEntry<ClaudeUsage>,
    pub codex: UsageEntry<CodexUsage>,
    pub grok: UsageEntry<GrokUsage>,
    /// Defaults so snapshots persisted before ZCode had a quota still load.
    #[serde(default)]
    pub zcode: UsageEntry<ZcodeUsage>,
    /// True while automatic polling is enabled in settings; the panel hides its countdown when off.
    pub auto: bool,
    /// Effective poll interval in seconds, so the panel can count down to the next refresh.
    pub interval_sec: u64,
}

/// Providers that publish an account-level quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    Claude,
    Codex,
    Grok,
    Zcode,
}

impl Provider {
    /// All providers, in the order the poller visits them.
    const ALL: [Provider; 4] = [Provider::Claude, Provider::Codex, Provider::Grok, Provider::Zcode];

    /// Parse the frontend's session-kind string; unknown names have no quota source.
    pub fn parse(s: &str) -> Option<Provider> {
        match s.trim().to_ascii_lowercase().as_str() {
            "claude" => Some(Provider::Claude),
            "codex" => Some(Provider::Codex),
            "grok" => Some(Provider::Grok),
            "zcode" => Some(Provider::Zcode),
            _ => None,
        }
    }

    /// Whether this machine holds account material for the provider.
    ///
    /// Deliberately a cheap filesystem check rather than a real credential read: on macOS the Claude
    /// token lives in the Keychain, and probing it from a background thread would raise an access
    /// prompt on machines that never run Claude at all.
    fn account_present(self) -> bool {
        match self {
            Provider::Claude => super::resume::claude_home().is_some_and(|p| p.is_dir()),
            Provider::Codex => super::resume::codex_home().is_some_and(|p| p.is_dir()),
            Provider::Grok => grok_home().is_some_and(|p| p.join("auth.json").is_file()),
            // The store `zcode login` writes; the platform and key inside are read only when fetching.
            Provider::Zcode => crate::host::home_dir()
                .is_some_and(|h| h.join(".zcode").join("v2").join("credentials.json").is_file()),
        }
    }
}

/// Grok root: `GROK_HOME` when set, otherwise `~/.grok`.
fn grok_home() -> Option<std::path::PathBuf> {
    if let Some(h) = std::env::var_os("GROK_HOME") {
        return Some(std::path::PathBuf::from(h));
    }
    crate::host::home_dir().map(|h| h.join(".grok"))
}

/// Poller bookkeeping that never leaves the process: last attempt time, consecutive failures, and any
/// server-imposed wait.
#[derive(Default)]
struct Attempt {
    /// Unix milliseconds of the last attempt, successful or not.
    at_ms: u64,
    fails: u32,
    /// Unix milliseconds before which the server asked us not to retry (`Retry-After` on 429). Polling
    /// through it only extends the ban, so `due` waits for it regardless of the interval.
    not_before_ms: u64,
}

#[derive(Default)]
struct State {
    snapshot: UsageSnapshot,
    attempts: HashMap<&'static str, Attempt>,
    /// Set once the persisted snapshot has been merged in, so a late start cannot overwrite fresh data.
    restored: bool,
}

static CODEX_OPERATION: Mutex<()> = Mutex::new(());

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn key(p: Provider) -> &'static str {
    match p {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
        Provider::Grok => "grok",
        Provider::Zcode => "zcode",
    }
}

/// Read the current snapshot. Pure memory: no network, no subprocess, no file reads.
pub fn snapshot(ctx: &AppCtx) -> UsageSnapshot {
    restore_once(ctx);
    let cfg = config(ctx);
    let mut guard = STATE.lock().unwrap();
    let st = guard.get_or_insert_with(State::default);
    st.snapshot.auto = cfg.0;
    st.snapshot.interval_sec = cfg.1;
    st.snapshot.clone()
}

/// Refresh now and return the updated snapshot.
///
/// `provider` limits the work to one source; None refreshes every provider whose account is present.
/// A manual refresh also clears the failure backoff so the next poll resumes the normal interval.
pub fn refresh(ctx: &AppCtx, provider: Option<Provider>, force: bool) -> UsageSnapshot {
    restore_once(ctx);
    let targets: Vec<Provider> = match provider {
        Some(p) => vec![p],
        None => Provider::ALL.into_iter().collect(),
    };
    let mut changed = false;
    for p in targets {
        if !p.account_present() {
            continue;
        }
        if force {
            reset_backoff(p);
        }
        changed |= fetch_one(ctx, p, force);
    }
    let snap = snapshot(ctx);
    if changed {
        persist(ctx, &snap);
        ctx.emit(USAGE_EVENT, snap.clone());
    }
    snap
}

/// Redeem through the same installed CLI and refresh the account snapshot for every client.
/// A refresh failure does not turn a confirmed redemption into an ambiguous mutation failure.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexResetResult {
    outcome: crate::agent::transcript::CodexResetOutcome,
    usage: UsageSnapshot,
}

pub fn consume_codex_reset(
    ctx: &AppCtx,
    idempotency_key: &str,
) -> Result<CodexResetResult, String> {
    restore_once(ctx);
    let _guard = CODEX_OPERATION
        .try_lock()
        .map_err(|_| "Codex account request in progress; retry the same request".to_string())?;
    let bin_path = crate::agent::executable::resolve(ctx, SessionKind::Codex, None);
    let outcome =
        crate::agent::transcript::consume_codex_reset_credit(bin_path.as_deref(), idempotency_key)?;
    // Never substitute an old rollout after redemption: it cannot confirm the new balance.
    let reading = crate::agent::transcript::live_codex_rate_limits(bin_path.as_deref());
    {
        let mut guard = STATE.lock().unwrap();
        let st = guard.get_or_insert_with(State::default);
        match reading {
            Ok(data) => mark_ok(&mut st.snapshot.codex, data, now_ms()),
            Err(error) => {
                if let Some(data) = &mut st.snapshot.codex.data {
                    data.reset_credits = None;
                }
                mark_err(&mut st.snapshot.codex, FetchError::from(error), now_ms());
            }
        }
    }
    let usage = snapshot(ctx);
    persist(ctx, &usage);
    ctx.emit(USAGE_EVENT, usage.clone());
    Ok(CodexResetResult { outcome, usage })
}

/// Start the single background poller. Safe to call once per process entry point (GUI and headless).
pub fn start(ctx: AppCtx) {
    std::thread::spawn(move || {
        restore_once(&ctx);
        loop {
            let (auto, interval_sec) = config(&ctx);
            if auto {
                let interval_ms = interval_sec.saturating_mul(1000);
                let mut changed = false;
                for p in Provider::ALL {
                    if !p.account_present() || !due(p, interval_ms) {
                        continue;
                    }
                    changed |= fetch_one(&ctx, p, false);
                }
                if changed {
                    let snap = snapshot(&ctx);
                    persist(&ctx, &snap);
                    ctx.emit(USAGE_EVENT, snap);
                }
            }
            std::thread::sleep(TICK);
        }
    });
}

/// Whether a provider is due, judged on the **wall clock**: a laptop that slept through three intervals
/// must refetch as soon as it wakes, which a monotonic timer would not do.
fn due(p: Provider, interval_ms: u64) -> bool {
    let mut guard = STATE.lock().unwrap();
    let st = guard.get_or_insert_with(State::default);
    let a = st.attempts.entry(key(p)).or_default();
    if a.at_ms == 0 {
        return true;
    }
    let backoff = 1u64 << a.fails.min(MAX_BACKOFF_STEPS);
    let now = now_ms();
    // A clock moved backwards (timezone fix, NTP step) would otherwise freeze polling until it catches up.
    if now < a.at_ms {
        return true;
    }
    now >= a.not_before_ms && now.saturating_sub(a.at_ms) >= interval_ms.saturating_mul(backoff)
}

fn reset_backoff(p: Provider) {
    let mut guard = STATE.lock().unwrap();
    let st = guard.get_or_insert_with(State::default);
    let a = st.attempts.entry(key(p)).or_default();
    a.fails = 0;
    a.not_before_ms = 0;
}

/// Fetch one provider and store the outcome. Returns true when the stored entry actually changed.
fn fetch_one(ctx: &AppCtx, p: Provider, force: bool) -> bool {
    // Keep a pre-redemption poll from overwriting the post-redemption snapshot.
    let _codex_guard = (p == Provider::Codex).then(|| CODEX_OPERATION.lock().unwrap());
    let result: Result<StoredValue, FetchError> = match p {
        Provider::Claude => crate::agent::usage::claude_usage(force).map(StoredValue::Claude),
        Provider::Codex => codex_account_usage(ctx)
            .map(StoredValue::Codex)
            .map_err(FetchError::from),
        Provider::Grok => crate::agent::usage::grok_usage(force).map(StoredValue::Grok),
        Provider::Zcode => zcode_account_usage(ctx).map(StoredValue::Zcode),
    };

    let mut guard = STATE.lock().unwrap();
    let st = guard.get_or_insert_with(State::default);
    let before = st.snapshot.clone();
    let now = now_ms();
    {
        let a = st.attempts.entry(key(p)).or_default();
        a.at_ms = now;
        match &result {
            Ok(_) => {
                a.fails = 0;
                a.not_before_ms = 0;
            }
            Err(e) => {
                a.fails = a.fails.saturating_add(1);
                // Honor the server's wait, capped so a bogus header cannot park the poller for a day.
                if let Some(secs) = e.retry_after_secs {
                    a.not_before_ms = now + secs.min(MAX_RETRY_AFTER_SECS) * 1000;
                }
            }
        }
    }
    match result {
        Ok(StoredValue::Claude(u)) => mark_ok(&mut st.snapshot.claude, u, now),
        Ok(StoredValue::Codex(u)) => mark_ok(&mut st.snapshot.codex, u, now),
        Ok(StoredValue::Grok(u)) => mark_ok(&mut st.snapshot.grok, u, now),
        Ok(StoredValue::Zcode(u)) => mark_ok(&mut st.snapshot.zcode, u, now),
        // Keep the previous reading on failure: a stale number with a stale marker beats an empty panel.
        Err(e) => match p {
            Provider::Claude => mark_err(&mut st.snapshot.claude, e, now),
            Provider::Codex => mark_err(&mut st.snapshot.codex, e, now),
            Provider::Grok => mark_err(&mut st.snapshot.grok, e, now),
            Provider::Zcode => mark_err(&mut st.snapshot.zcode, e, now),
        },
    }
    st.snapshot != before
}

/// Store a fresh reading and clear any earlier failure.
fn mark_ok<T>(entry: &mut UsageEntry<T>, data: T, now: u64) {
    entry.data = Some(data);
    entry.error = None;
    entry.error_at = None;
    entry.fetched_at = Some(now);
}

/// Record a failure while leaving the last good reading and its timestamp untouched.
fn mark_err<T>(entry: &mut UsageEntry<T>, e: FetchError, now: u64) {
    entry.error = Some(e.message);
    entry.error_at = Some(now);
}

/// One provider's successful reading, kept as an enum so `fetch_one` has a single storage path.
enum StoredValue {
    Claude(ClaudeUsage),
    Codex(CodexUsage),
    Grok(GrokUsage),
    Zcode(ZcodeUsage),
}

/// The ZCode plan quota, with the key of the plan the signed-in account holds. The key is read the way a
/// conversation reads it, through the provider config the installed agent ships with.
fn zcode_account_usage(ctx: &AppCtx) -> Result<ZcodeUsage, FetchError> {
    let bin = crate::agent::executable::resolve(ctx, SessionKind::Zcode, None)
        .ok_or_else(|| FetchError::from("ZCode is not installed".to_string()))?;
    let account = crate::agent::zcode_account::Account::for_command(&crate::agent::zcode::command(&bin))
        .ok_or_else(|| FetchError::from("ZCode's provider config could not be found".to_string()))?;
    let (platform, key) = account
        .plan_api_key()
        .ok_or_else(|| FetchError::from("Sign in to ZCode with a GLM Coding Plan to see its quota".to_string()))?;
    crate::agent::usage::zcode_usage(&platform, &key)
}

/// Account-level Codex limits without any session context.
///
/// The live `codex app-server` call is authoritative and reflects usage from other machines and other
/// front ends; the newest local rollout is the offline fallback for older CLIs and for a machine with no
/// network.
fn codex_account_usage(ctx: &AppCtx) -> Result<CodexUsage, String> {
    let bin_path = crate::agent::executable::resolve(ctx, SessionKind::Codex, None);
    if let Ok(u) = crate::agent::transcript::live_codex_rate_limits(bin_path.as_deref()) {
        return Ok(u);
    }
    crate::agent::transcript::latest_codex_rate_limits()
}

/// Read `usageAutoRefresh` / `usageRefreshSec` from the shared settings block, falling back to the
/// frontend defaults when the block is missing, unreadable, or holds an out-of-range interval.
fn config(ctx: &AppCtx) -> (bool, u64) {
    let json = {
        let Ok(conn) = ctx.db().conn.lock() else {
            return (DEFAULT_AUTO, DEFAULT_INTERVAL_SEC);
        };
        crate::db::repo::get_app_settings(&conn)
            .ok()
            .and_then(|mut m| m.remove(VLX_SETTINGS_KEY))
    };
    let Some(v) = json.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()) else {
        return (DEFAULT_AUTO, DEFAULT_INTERVAL_SEC);
    };
    let auto = v
        .get("usageAutoRefresh")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(DEFAULT_AUTO);
    let interval = v
        .get("usageRefreshSec")
        .and_then(serde_json::Value::as_u64)
        .filter(|n| *n >= MIN_INTERVAL_SEC)
        .unwrap_or(DEFAULT_INTERVAL_SEC);
    (auto, interval)
}

/// Merge the persisted snapshot into memory once per process, before the first poll returns.
fn restore_once(ctx: &AppCtx) {
    {
        let mut guard = STATE.lock().unwrap();
        let st = guard.get_or_insert_with(State::default);
        if st.restored {
            return;
        }
        st.restored = true;
    }
    let stored = {
        let Ok(conn) = ctx.db().conn.lock() else {
            return;
        };
        crate::db::repo::get_app_settings(&conn)
            .ok()
            .and_then(|mut m| m.remove(SNAPSHOT_KEY))
    };
    let Some(snap) = stored.and_then(|s| serde_json::from_str::<UsageSnapshot>(&s).ok()) else {
        return;
    };
    let mut guard = STATE.lock().unwrap();
    let st = guard.get_or_insert_with(State::default);
    // Only fill gaps: a poll that finished while this was loading must win.
    if st.snapshot.claude.data.is_none() {
        st.snapshot.claude = snap.claude;
    }
    if st.snapshot.codex.data.is_none() {
        st.snapshot.codex = snap.codex;
    }
    if st.snapshot.grok.data.is_none() {
        st.snapshot.grok = snap.grok;
    }
    if st.snapshot.zcode.data.is_none() {
        st.snapshot.zcode = snap.zcode;
    }
}

/// Persist the snapshot so the next launch paints immediately. Failures are logged, never fatal.
fn persist(ctx: &AppCtx, snap: &UsageSnapshot) {
    let Ok(json) = serde_json::to_string(snap) else {
        return;
    };
    let mut entries = HashMap::new();
    entries.insert(SNAPSHOT_KEY.to_string(), json);
    let Ok(conn) = ctx.db().conn.lock() else {
        return;
    };
    if let Err(e) = crate::db::repo::set_app_settings(&conn, &entries) {
        crate::diagnostic_warn!("failed to persist usage snapshot: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_parses_session_kinds() {
        assert_eq!(Provider::parse("Claude"), Some(Provider::Claude));
        assert_eq!(Provider::parse(" codex "), Some(Provider::Codex));
        assert_eq!(Provider::parse("grok"), Some(Provider::Grok));
        assert_eq!(Provider::parse("zcode"), Some(Provider::Zcode));
        assert_eq!(Provider::parse("terminal"), None);
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let snap = UsageSnapshot {
            claude: UsageEntry {
                data: Some(ClaudeUsage {
                    five_hour: Some(crate::agent::usage::UsageWindow {
                        utilization: 12.5,
                        resets_at: Some("2026-08-30T00:29:00Z".to_string()),
                    }),
                    seven_day: None,
                    seven_day_opus: None,
                    model_weekly: Vec::new(),
                }),
                error: None,
                fetched_at: Some(1_700_000_000_000),
                error_at: None,
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("fiveHour"), "camelCase is the wire format");
        let back: UsageSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, snap);
    }

    #[test]
    fn snapshot_persisted_without_error_at_still_loads() {
        let json = r#"{"claude":{"data":null,"error":"x","fetchedAt":null},"codex":{"data":null,"error":null,"fetchedAt":null},"grok":{"data":null,"error":null,"fetchedAt":null},"auto":true,"intervalSec":300}"#;
        let back: UsageSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(back.claude.error.as_deref(), Some("x"));
        assert_eq!(back.claude.error_at, None);
    }

    #[test]
    fn failure_keeps_reading_and_marks_stale() {
        let mut entry: UsageEntry<u32> = UsageEntry::default();
        mark_ok(&mut entry, 7, 1_000);
        mark_err(&mut entry, FetchError::from("boom".to_string()), 2_000);
        assert_eq!(entry.data, Some(7), "last good reading survives a failure");
        assert_eq!(entry.fetched_at, Some(1_000), "success time is not bumped by a failure");
        assert_eq!(entry.error.as_deref(), Some("boom"));
        assert_eq!(entry.error_at, Some(2_000));
        mark_ok(&mut entry, 8, 3_000);
        assert_eq!((entry.error, entry.error_at, entry.fetched_at), (None, None, Some(3_000)));
    }
}
