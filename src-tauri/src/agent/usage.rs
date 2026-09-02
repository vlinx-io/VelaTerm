//! Account usage for Claude and Grok Build subscription plans.
//!
//! - **Claude**: Anthropic `oauth/usage` (five-hour / weekly windows). Credentials come from the Claude
//!   Code keychain item or `~/.claude/.credentials.json`.
//! - **Grok**: `cli-chat-proxy.grok.com/v1/billing?format=credits` SuperGrok weekly usage pool
//!   (same source as Grok Build `/usage`). Tokens come from `~/.grok/auth.json` (OIDC); expired
//!   access tokens are refreshed via `auth.x.ai/oauth2/token`.
//!
//! Both endpoints are rate-limited or network-bound, so process-wide TTL caches sit under the poller's
//! interval to absorb repeated manual clicks. A failed request is reported as `FetchError`; keeping the
//! last good reading is `usage_store`'s job, so nothing here hides a failure behind cached data.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One usage window, such as five hours or seven days.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    /// Utilization percentage from 0 to 100.
    pub utilization: f64,
    /// Optional ISO 8601 reset timestamp.
    pub resets_at: Option<String>,
}

/// Account-wide Claude usage snapshot, independent of a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUsage {
    /// Five-hour rolling window.
    pub five_hour: Option<UsageWindow>,
    /// Seven-day total rolling window.
    pub seven_day: Option<UsageWindow>,
    /// Optional seven-day Opus-specific quota available on some plans.
    pub seven_day_opus: Option<UsageWindow>,
    /// Per-model weekly quotas from the `limits` array (for example Fable). Defaults to empty so
    /// snapshots persisted before this field existed still deserialize.
    #[serde(default)]
    pub model_weekly: Vec<ModelWindow>,
}

/// A weekly quota scoped to one model, taken from a `limits[]` entry with `kind: "weekly_scoped"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelWindow {
    /// Model display name as the API reports it, such as "Fable 5".
    pub model: String,
    /// Utilization percentage from 0 to 100.
    pub utilization: f64,
    /// Optional ISO 8601 reset timestamp.
    pub resets_at: Option<String>,
}

/// Why a provider request failed, with the server's retry hint when it sent one.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchError {
    /// Human-readable reason; surfaces in the Info panel tooltip.
    pub message: String,
    /// Seconds the server asked us to wait (`Retry-After` on HTTP 429), when present.
    pub retry_after_secs: Option<u64>,
}

impl From<String> for FetchError {
    fn from(message: String) -> Self {
        Self { message, retry_after_secs: None }
    }
}

/// Map a ureq failure to `FetchError`, reading `Retry-After` off a 429 so the poller can honor it.
fn http_error(what: &str, e: ureq::Error) -> FetchError {
    match e {
        ureq::Error::Status(code, resp) => {
            let retry_after_secs = resp
                .header("retry-after")
                .and_then(|v| v.trim().parse::<u64>().ok());
            let message = match (code, retry_after_secs) {
                (429, Some(s)) => format!("{what} rate limited (HTTP 429), retry after {s}s"),
                (429, None) => format!("{what} rate limited (HTTP 429)"),
                (401 | 403, _) => format!("{what} rejected the credentials (HTTP {code})"),
                _ => format!("{what} returned HTTP {code}"),
            };
            FetchError { message, retry_after_secs }
        }
        other => FetchError::from(format!("{what} request failed: {other}")),
    }
}

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA: &str = "oauth-2025-04-20";
/// Cache TTL deduplicates concurrent session requests and repeated clicks. The frontend's usage-refresh
/// interval (five minutes by default) controls normal frequency; both layers protect the endpoint.
const CACHE_TTL: Duration = Duration::from_secs(15);

/// Process-wide last successful result and timestamp, shared by all Claude sessions.
static CACHE: Mutex<Option<(Instant, ClaudeUsage)>> = Mutex::new(None);

/// Fetch Claude account usage.
///
/// With `force=false`, return a fresh cache entry without contacting the endpoint. Expiration or
/// `force=true` makes a real request. A failure is returned as is: the caller keeps its own last good
/// reading, and returning cached data here would make a 429 look like a fresh success.
pub fn claude_usage(force: bool) -> Result<ClaudeUsage, FetchError> {
    if !force {
        if let Some((t, cached)) = CACHE.lock().unwrap().as_ref() {
            if t.elapsed() < CACHE_TTL {
                return Ok(cached.clone());
            }
        }
    }
    let u = fetch()?;
    *CACHE.lock().unwrap() = Some((Instant::now(), u.clone()));
    Ok(u)
}

/// Read the token, call the endpoint, and parse the response.
fn fetch() -> Result<ClaudeUsage, FetchError> {
    let token = read_oauth_token()?;
    let body = ureq::get(USAGE_URL)
        .set("Authorization", &format!("Bearer {token}"))
        .set("anthropic-beta", OAUTH_BETA)
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(8))
        .call()
        .map_err(|e| http_error("usage endpoint", e))?
        .into_string()
        .map_err(|e| format!("failed to read usage response: {e}"))?;
    let v: Value =
        serde_json::from_str(&body).map_err(|e| format!("failed to parse usage JSON: {e}"))?;
    Ok(parse_claude_usage(&v))
}

/// Build the snapshot from the `oauth/usage` JSON body.
fn parse_claude_usage(v: &Value) -> ClaudeUsage {
    ClaudeUsage {
        five_hour: parse_window(v.get("five_hour")),
        seven_day: parse_window(v.get("seven_day")),
        seven_day_opus: parse_window(v.get("seven_day_opus")),
        model_weekly: parse_model_weekly(v.get("limits")),
    }
}

/// Collect per-model weekly quotas from `limits[]`.
///
/// Claude Code's own `/usage` screen reads the same array: entries shaped like
/// `{kind: "weekly_scoped", scope: {model: {display_name}}, percent, resets_at}` become
/// "Current week (<model>)" rows. Entries of other kinds or without a model scope are skipped.
fn parse_model_weekly(v: Option<&Value>) -> Vec<ModelWindow> {
    let Some(items) = v.and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if item.get("kind").and_then(Value::as_str) != Some("weekly_scoped") {
                return None;
            }
            let model = item
                .get("scope")?
                .get("model")?
                .get("display_name")?
                .as_str()?
                .trim();
            if model.is_empty() {
                return None;
            }
            let utilization = item
                .get("percent")
                .or_else(|| item.get("utilization"))
                .and_then(Value::as_f64)?;
            let resets_at = parse_reset_time(item.get("resets_at"));
            Some(ModelWindow {
                model: model.to_string(),
                utilization,
                resets_at,
            })
        })
        .collect()
}

/// Read a `resets_at` field that the server sends either as an ISO string or as Unix seconds.
///
/// Claude Code's own usage screen accepts both shapes for `limits[]` entries, so keep the number
/// path here too; a numeric value would otherwise silently drop the reset time from the row.
fn parse_reset_time(v: Option<&Value>) -> Option<String> {
    let v = v?;
    if let Some(s) = v.as_str() {
        return Some(s.to_string());
    }
    v.as_u64().map(unix_to_rfc3339_utc)
}

/// Parse `{utilization: f64, resets_at: "ISO"}`, returning None for missing/null fields.
fn parse_window(v: Option<&Value>) -> Option<UsageWindow> {
    let v = v?;
    if v.is_null() {
        return None;
    }
    let utilization = v.get("utilization").and_then(Value::as_f64)?;
    let resets_at = v
        .get("resets_at")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(UsageWindow {
        utilization,
        resets_at,
    })
}

/// On macOS, use `security` to read Claude Code's JSON OAuth item from Keychain and extract accessToken.
/// Because vlx-term is a different application, first access may prompt for Keychain permission. Common
/// failures are API-key login, no login, or denied access.
#[cfg(target_os = "macos")]
fn read_oauth_token() -> Result<String, String> {
    let out = std::process::Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .map_err(|e| format!("failed to run security to read Keychain: {e}"))?;
    if !out.status.success() {
        let code = out.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stderr = stderr.trim();
        // Exit 44 is errSecItemNotFound, usually API-key login or no subscription installation. Other
        // failures commonly indicate denied Keychain access.
        return Err(if stderr.is_empty() {
            format!("cannot read claude subscription credentials (security exit {code})")
        } else {
            format!("cannot read claude subscription credentials (security exit {code}): {stderr}")
        });
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    extract_access_token(raw.trim())
}

/// Outside macOS, currently read ~/.claude/.credentials.json, common on Linux. Windows DPAPI credentials
/// are unreadable through this path, producing Err until dedicated support is added.
#[cfg(not(target_os = "macos"))]
fn read_oauth_token() -> Result<String, String> {
    let home = crate::host::home_dir().ok_or("home directory not found")?;
    let p = home.join(".claude").join(".credentials.json");
    let raw = std::fs::read_to_string(&p).map_err(|_| {
        "claude credentials file not found (non-macOS only supports ~/.claude/.credentials.json)"
            .to_string()
    })?;
    extract_access_token(&raw)
}

/// Extract accessToken from either `{accessToken,...}` or `{claudeAiOauth:{accessToken,...}}`.
fn extract_access_token(raw: &str) -> Result<String, String> {
    let v: Value =
        serde_json::from_str(raw).map_err(|e| format!("failed to parse credentials JSON: {e}"))?;
    let obj = v.get("claudeAiOauth").unwrap_or(&v);
    obj.get("accessToken")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "no accessToken in credentials JSON".to_string())
}

// ─── Grok Build monthly credits ───────────────────────────────────────────────

/// Grok SuperGrok shared usage pool for the Info panel USAGE section.
///
/// Prefer the unified weekly credits view (`/v1/billing?format=credits`), which matches Grok Build's
/// own `/usage` screen. Fall back to the legacy monthly dollars payload when credits format is absent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrokUsage {
    /// Used percentage of the current pool, 0–100.
    pub used_percent: f64,
    /// Window label for the frontend row, such as `7d` or `mo`.
    pub window_label: String,
    /// ISO 8601 period end (when the pool resets).
    pub period_end: Option<String>,
    /// ISO 8601 period start.
    pub period_start: Option<String>,
    /// Optional product split: Grok Build share of the same weekly pool, 0–100.
    pub build_percent: Option<f64>,
    /// Optional pay-as-you-go cap when configured (0 means disabled).
    pub on_demand_cap: Option<f64>,
}

/// Credits format returns SuperGrok's shared weekly pool (same source as the in-TUI `/usage` screen).
const GROK_BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";
const GROK_TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
const GROK_CACHE_TTL: Duration = Duration::from_secs(30);

static GROK_CACHE: Mutex<Option<(Instant, GrokUsage)>> = Mutex::new(None);

/// Fetch Grok Build account credit usage.
///
/// Cached like Claude: `force` bypasses TTL for manual refresh. A failure is returned as is so the store
/// can mark the reading stale instead of mistaking cached data for a fresh success.
pub fn grok_usage(force: bool) -> Result<GrokUsage, FetchError> {
    if !force {
        if let Some((t, cached)) = GROK_CACHE.lock().unwrap().as_ref() {
            if t.elapsed() < GROK_CACHE_TTL {
                return Ok(cached.clone());
            }
        }
    }
    let u = fetch_grok_billing()?;
    *GROK_CACHE.lock().unwrap() = Some((Instant::now(), u.clone()));
    Ok(u)
}

fn fetch_grok_billing() -> Result<GrokUsage, String> {
    let token = grok_access_token(false)?;
    match request_grok_billing(&token) {
        Ok(body) => parse_billing_body(&body),
        Err(status) if status == 401 => {
            // Access token expired: force refresh, then retry once.
            let token = grok_access_token(true)?;
            let body = request_grok_billing(&token).map_err(|s| {
                format!("Grok billing request failed after token refresh (HTTP {s})")
            })?;
            parse_billing_body(&body)
        }
        Err(status) => Err(format!("Grok billing request failed (HTTP {status})")),
    }
}

fn request_grok_billing(token: &str) -> Result<String, u16> {
    match ureq::get(GROK_BILLING_URL)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/json")
        .timeout(Duration::from_secs(10))
        .call()
    {
        Ok(resp) => resp.into_string().map_err(|_| 502u16),
        Err(ureq::Error::Status(code, _)) => Err(code),
        Err(_) => Err(0),
    }
}

fn parse_billing_body(body: &str) -> Result<GrokUsage, String> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| format!("failed to parse Grok billing JSON: {e}"))?;
    parse_grok_billing(&v).ok_or_else(|| "Grok billing response has no usage pool".to_string())
}

/// Parse Grok billing JSON into a usage snapshot.
///
/// Primary shape (`?format=credits`):
/// `{config:{currentPeriod:{type,start,end}, creditUsagePercent, productUsage:[{product,usagePercent}]}}`
///
/// Fallback (legacy dollars):
/// `{config:{monthlyLimit:{val}, used:{val}, billingPeriodStart, billingPeriodEnd}}`
fn parse_grok_billing(v: &Value) -> Option<GrokUsage> {
    let config = v.get("config").unwrap_or(v);

    // Preferred: SuperGrok shared weekly/monthly credits pool.
    if let Some(pct) = config
        .get("creditUsagePercent")
        .and_then(|n| n.as_f64().or_else(|| n.as_i64().map(|i| i as f64)))
    {
        let period = config.get("currentPeriod");
        let period_type = period
            .and_then(|p| p.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let period_start = period
            .and_then(|p| p.get("start"))
            .and_then(Value::as_str)
            .or_else(|| config.get("billingPeriodStart").and_then(Value::as_str))
            .map(str::to_string);
        let period_end = period
            .and_then(|p| p.get("end"))
            .and_then(Value::as_str)
            .or_else(|| config.get("billingPeriodEnd").and_then(Value::as_str))
            .map(str::to_string);
        let build_percent = config
            .get("productUsage")
            .and_then(Value::as_array)
            .and_then(|products| {
                products.iter().find_map(|p| {
                    let name = p.get("product").and_then(Value::as_str)?;
                    if name.eq_ignore_ascii_case("GrokBuild") || name.eq_ignore_ascii_case("Build")
                    {
                        p.get("usagePercent")
                            .and_then(|n| n.as_f64().or_else(|| n.as_i64().map(|i| i as f64)))
                    } else {
                        None
                    }
                })
            });
        return Some(GrokUsage {
            used_percent: pct.clamp(0.0, 100.0),
            window_label: period_window_label(
                period_type,
                period_start.as_deref(),
                period_end.as_deref(),
            ),
            period_end,
            period_start,
            build_percent,
            on_demand_cap: money_val(config.get("onDemandCap")),
        });
    }

    // Legacy monthly dollar pool.
    let limit = money_val(config.get("monthlyLimit"))?;
    if limit <= 0.0 {
        return None;
    }
    let used = money_val(config.get("used")).unwrap_or(0.0).max(0.0);
    let used_percent = ((used / limit) * 100.0).clamp(0.0, 100.0);
    let period_start = config
        .get("billingPeriodStart")
        .and_then(Value::as_str)
        .map(str::to_string);
    let period_end = config
        .get("billingPeriodEnd")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(GrokUsage {
        used_percent,
        window_label: period_window_label(
            "USAGE_PERIOD_TYPE_MONTHLY",
            period_start.as_deref(),
            period_end.as_deref(),
        ),
        period_end,
        period_start,
        build_percent: None,
        on_demand_cap: money_val(config.get("onDemandCap")),
    })
}

/// Map server period type / dates into a compact Codex-style label (`7d`, `mo`, …).
fn period_window_label(period_type: &str, start: Option<&str>, end: Option<&str>) -> String {
    let upper = period_type.to_ascii_uppercase();
    if upper.contains("WEEK") {
        return "7d".into();
    }
    if upper.contains("MONTH") {
        return "mo".into();
    }
    if upper.contains("DAY") || upper.contains("DAILY") {
        return "1d".into();
    }
    // Infer from period length when the type string is missing or unknown.
    if let (Some(s), Some(e)) = (start, end) {
        if let (Some(su), Some(eu)) = (parse_rfc3339_unix(s), parse_rfc3339_unix(e)) {
            let days = eu.saturating_sub(su) as f64 / 86400.0;
            if days <= 1.5 {
                return "1d".into();
            }
            if days <= 8.0 {
                return "7d".into();
            }
            if days <= 35.0 {
                return "mo".into();
            }
            return format!("{}d", days.round() as u64);
        }
    }
    "7d".into()
}

/// Credits are nested as `{val: number}` in the billing payload.
fn money_val(v: Option<&Value>) -> Option<f64> {
    let v = v?;
    if v.is_null() {
        return None;
    }
    v.get("val")
        .and_then(|n| n.as_f64().or_else(|| n.as_i64().map(|i| i as f64)))
        .or_else(|| v.as_f64())
}

fn grok_auth_path() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("GROK_HOME") {
        return Ok(PathBuf::from(home).join("auth.json"));
    }
    let home = crate::host::home_dir().ok_or("home directory not found")?;
    Ok(home.join(".grok").join("auth.json"))
}

/// Load a usable OIDC access token from `~/.grok/auth.json`.
///
/// When `force_refresh` is true, always exchange the refresh token. Otherwise return the stored
/// access token when it is not near expiry (one-minute skew).
fn grok_access_token(force_refresh: bool) -> Result<String, String> {
    let path = grok_auth_path()?;
    let raw = std::fs::read_to_string(&path)
        .map_err(|_| "Grok auth not found (run `grok login`)".to_string())?;
    let mut root: Value =
        serde_json::from_str(&raw).map_err(|e| format!("failed to parse Grok auth.json: {e}"))?;
    let entry = first_auth_entry_mut(&mut root)
        .ok_or_else(|| "Grok auth.json has no OIDC credentials".to_string())?;

    let access = entry
        .get("key")
        .or_else(|| entry.get("access_token"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "Grok auth.json missing access token".to_string())?;

    if !force_refresh && !token_expiring_soon(entry.get("expires_at").and_then(Value::as_str), 60) {
        return Ok(access);
    }

    let refresh = entry
        .get("refresh_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "Grok access token expired and no refresh_token is available".to_string())?
        .to_string();
    let client_id = entry
        .get("oidc_client_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "Grok auth.json missing oidc_client_id".to_string())?
        .to_string();

    let refreshed = refresh_grok_token(&refresh, &client_id)?;
    // Persist so subsequent Grok CLI/VelaTerm processes reuse the fresh token.
    if let Some(obj) = entry.as_object_mut() {
        obj.insert("key".into(), Value::String(refreshed.access_token.clone()));
        if let Some(exp) = refreshed.expires_at {
            obj.insert("expires_at".into(), Value::String(exp));
        }
        if let Some(rt) = refreshed.refresh_token {
            obj.insert("refresh_token".into(), Value::String(rt));
        }
    }
    if let Ok(serialized) = serde_json::to_string_pretty(&root) {
        let _ = std::fs::write(&path, serialized);
    }
    Ok(refreshed.access_token)
}

fn first_auth_entry_mut(root: &mut Value) -> Option<&mut Value> {
    let obj = root.as_object_mut()?;
    // Prefer entries that look like OIDC (issuer::client).
    let key = obj
        .keys()
        .find(|k| k.contains("auth.x.ai") || k.contains("::"))
        .cloned()
        .or_else(|| obj.keys().next().cloned())?;
    let entry = obj.get_mut(&key)?;
    entry.is_object().then_some(entry)
}

/// Best-effort RFC3339 expiry check. Unparseable values are treated as still valid so the first
/// billing request can decide via HTTP 401.
fn token_expiring_soon(expires_at: Option<&str>, skew_secs: u64) -> bool {
    let Some(raw) = expires_at else {
        return false;
    };
    let Some(exp_unix) = parse_rfc3339_unix(raw) else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    exp_unix <= now.saturating_add(skew_secs)
}

/// Parse a subset of RFC3339 (`2026-07-28T14:08:44.883383Z` / with offset) into Unix seconds.
fn parse_rfc3339_unix(raw: &str) -> Option<u64> {
    // Strip fractional seconds: 2026-07-28T14:08:44.883383Z → 2026-07-28T14:08:44Z
    let s = raw.trim();
    let (main, tz) = if let Some(i) = s.find('.') {
        let rest = &s[i + 1..];
        let tz_at = rest
            .find(['Z', '+', '-'])
            .map(|j| i + 1 + j)
            .unwrap_or(s.len());
        (&s[..i], &s[tz_at..])
    } else if let Some(i) = s.rfind('Z') {
        (&s[..i], "Z")
    } else if let Some(i) = s.rfind('+') {
        (&s[..i], &s[i..])
    } else if let Some(i) = s[11..].rfind('-') {
        // Offset like -07:00 after the date-time body.
        let abs = 11 + i;
        (&s[..abs], &s[abs..])
    } else {
        (s, "Z")
    };
    let body = main.trim_end_matches('Z');
    let parts: Vec<&str> = body.split('T').collect();
    if parts.len() != 2 {
        return None;
    }
    let mut d = parts[0].split('-');
    let year: i32 = d.next()?.parse().ok()?;
    let month: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    let mut t = parts[1].split(':');
    let hour: u32 = t.next()?.parse().ok()?;
    let minute: u32 = t.next()?.parse().ok()?;
    let second: u32 = t.next().unwrap_or("0").parse().ok()?;

    // Days from civil date (Howard Hinnant algorithm) → Unix.
    let mut y = year;
    let m = month as i32;
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u32;
    let days = (era * 146097 + doe as i32 - 719468) as i64;
    let mut secs = days * 86400 + (hour as i64) * 3600 + (minute as i64) * 60 + second as i64;

    // Apply timezone offset when present (+08:00 / -07:00). Z means UTC.
    if tz != "Z" && !tz.is_empty() {
        let sign = if tz.starts_with('-') { -1i64 } else { 1i64 };
        let num = tz.trim_start_matches(['+', '-']);
        let mut p = num.split(':');
        let oh: i64 = p.next().unwrap_or("0").parse().unwrap_or(0);
        let om: i64 = p.next().unwrap_or("0").parse().unwrap_or(0);
        secs -= sign * (oh * 3600 + om * 60);
    }
    u64::try_from(secs).ok()
}

struct RefreshedToken {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<String>,
}

fn refresh_grok_token(refresh_token: &str, client_id: &str) -> Result<RefreshedToken, String> {
    let form = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        urlencoding_form(refresh_token),
        urlencoding_form(client_id)
    );
    let body = ureq::post(GROK_TOKEN_URL)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Accept", "application/json")
        .timeout(Duration::from_secs(10))
        .send_string(&form)
        .map_err(|e| format!("Grok token refresh failed: {e}"))?
        .into_string()
        .map_err(|e| format!("failed to read Grok token response: {e}"))?;
    let v: Value =
        serde_json::from_str(&body).map_err(|e| format!("failed to parse Grok token JSON: {e}"))?;
    let access_token = v
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "Grok token refresh returned no access_token".to_string())?
        .to_string();
    let refresh_token = v
        .get("refresh_token")
        .and_then(Value::as_str)
        .map(str::to_string);
    let expires_at = v.get("expires_in").and_then(Value::as_u64).map(|secs| {
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
            .saturating_add(secs);
        // Format as basic RFC3339 UTC without pulling in time formatting features.
        unix_to_rfc3339_utc(exp)
    });
    Ok(RefreshedToken {
        access_token,
        refresh_token,
        expires_at,
    })
}

fn unix_to_rfc3339_utc(secs: u64) -> String {
    // Civil date from Unix days (Howard Hinnant).
    let z = (secs / 86400) as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    if m <= 2 {
        y += 1;
    }
    let rem = secs % 86400;
    let h = rem / 3600;
    let min = (rem % 3600) / 60;
    let s = rem % 60;
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}Z")
}

/// Minimal application/x-www-form-urlencoded encode for OAuth token fields.
fn urlencoding_form(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_token_flat_and_nested() {
        // Top-level shape.
        let flat = r#"{"accessToken":"abc","refreshToken":"r"}"#;
        assert_eq!(extract_access_token(flat).unwrap(), "abc");
        // Nested claudeAiOauth shape.
        let nested = r#"{"claudeAiOauth":{"accessToken":"xyz"}}"#;
        assert_eq!(extract_access_token(nested).unwrap(), "xyz");
        // Missing accessToken.
        assert!(extract_access_token(r#"{"foo":1}"#).is_err());
    }

    #[test]
    fn parse_window_handles_null_and_missing() {
        assert!(parse_window(None).is_none());
        assert!(parse_window(Some(&Value::Null)).is_none());
        let v: Value =
            serde_json::from_str(r#"{"utilization":26.0,"resets_at":"2026-06-21T10:00:00+00:00"}"#)
                .unwrap();
        let w = parse_window(Some(&v)).unwrap();
        assert_eq!(w.utilization, 26.0);
        assert_eq!(w.resets_at.as_deref(), Some("2026-06-21T10:00:00+00:00"));
    }

    #[test]
    fn parse_model_weekly_reads_scoped_limits() {
        let v: Value = serde_json::json!({
            "five_hour": {"utilization": 6.0, "resets_at": "2026-09-02T09:39:00+00:00"},
            "seven_day": {"utilization": 8.0, "resets_at": "2026-09-06T10:59:00+00:00"},
            "seven_day_opus": null,
            "limits": [
                {"kind": "weekly_scoped", "scope": {"model": {"display_name": "Fable 5"}},
                 "percent": 42.5, "resets_at": "2026-09-06T10:59:00+00:00"},
                {"kind": "weekly_scoped", "scope": {}, "percent": 1.0},
                {"kind": "session", "percent": 3.0},
                {"kind": "weekly_scoped", "scope": {"model": {"display_name": "  "}}, "percent": 9.0},
                {"kind": "weekly_scoped", "scope": {"model": {"display_name": "Claude Opus 4.8"}},
                 "percent": 12.0, "resets_at": 1_788_000_000}
            ]
        });
        let u = parse_claude_usage(&v);
        assert_eq!(u.five_hour.unwrap().utilization, 6.0);
        assert!(u.seven_day_opus.is_none());
        assert_eq!(u.model_weekly.len(), 2);
        assert_eq!(u.model_weekly[0].model, "Fable 5");
        assert_eq!(u.model_weekly[0].utilization, 42.5);
        assert_eq!(
            u.model_weekly[0].resets_at.as_deref(),
            Some("2026-09-06T10:59:00+00:00")
        );
        // A numeric `resets_at` becomes the same ISO text the string form already carries.
        assert_eq!(u.model_weekly[1].model, "Claude Opus 4.8");
        assert_eq!(
            u.model_weekly[1].resets_at.as_deref(),
            Some("2026-08-29T10:40:00Z")
        );
    }

    #[test]
    fn parse_model_weekly_missing_limits_is_empty_and_old_snapshots_deserialize() {
        let v: Value = serde_json::json!({"five_hour": {"utilization": 1.0}});
        assert!(parse_claude_usage(&v).model_weekly.is_empty());
        // A snapshot persisted before `model_weekly` existed still loads.
        let old = r#"{"fiveHour":{"utilization":1.0,"resetsAt":null},"sevenDay":null,"sevenDayOpus":null}"#;
        let u: ClaudeUsage = serde_json::from_str(old).unwrap();
        assert!(u.model_weekly.is_empty());
    }

    #[test]
    fn parse_grok_billing_weekly_credits_pool() {
        let v: Value = serde_json::from_str(
            r#"{
              "config": {
                "currentPeriod": {
                  "type": "USAGE_PERIOD_TYPE_WEEKLY",
                  "start": "2026-07-22T12:30:19.421585+00:00",
                  "end": "2026-07-29T12:30:19.421585+00:00"
                },
                "creditUsagePercent": 52.0,
                "productUsage": [
                  {"product": "GrokChat", "usagePercent": 48.0},
                  {"product": "GrokBuild", "usagePercent": 3.0}
                ],
                "onDemandCap": {"val": 0},
                "billingPeriodStart": "2026-07-22T12:30:19.421585+00:00",
                "billingPeriodEnd": "2026-07-29T12:30:19.421585+00:00"
              }
            }"#,
        )
        .unwrap();
        let u = parse_grok_billing(&v).unwrap();
        assert_eq!(u.used_percent, 52.0);
        assert_eq!(u.window_label, "7d");
        assert_eq!(u.build_percent, Some(3.0));
        assert_eq!(
            u.period_end.as_deref(),
            Some("2026-07-29T12:30:19.421585+00:00")
        );
    }

    #[test]
    fn parse_grok_billing_monthly_pool_fallback() {
        let v: Value = serde_json::from_str(
            r#"{
              "config": {
                "monthlyLimit": {"val": 15000},
                "used": {"val": 2359},
                "onDemandCap": {"val": 0},
                "billingPeriodStart": "2026-07-01T00:00:00+00:00",
                "billingPeriodEnd": "2026-08-01T00:00:00+00:00"
              }
            }"#,
        )
        .unwrap();
        let u = parse_grok_billing(&v).unwrap();
        assert!((u.used_percent - 15.726).abs() < 0.01);
        assert_eq!(u.window_label, "mo");
        assert_eq!(u.period_end.as_deref(), Some("2026-08-01T00:00:00+00:00"));
        assert_eq!(u.on_demand_cap, Some(0.0));
    }

    #[test]
    fn parse_grok_billing_rejects_zero_limit() {
        let v: Value =
            serde_json::from_str(r#"{"config":{"monthlyLimit":{"val":0},"used":{"val":1}}}"#)
                .unwrap();
        assert!(parse_grok_billing(&v).is_none());
    }

    #[test]
    fn urlencoding_form_encodes_specials() {
        assert_eq!(urlencoding_form("aB9-_.~"), "aB9-_.~");
        assert_eq!(urlencoding_form("a/b c"), "a%2Fb%20c");
    }

    #[test]
    fn parse_rfc3339_unix_basic_z() {
        // 2026-07-28T00:00:00Z
        let u = parse_rfc3339_unix("2026-07-28T00:00:00Z").unwrap();
        assert_eq!(unix_to_rfc3339_utc(u), "2026-07-28T00:00:00Z");
        // Fractional seconds stripped.
        let u2 = parse_rfc3339_unix("2026-07-28T14:08:44.883383Z").unwrap();
        assert_eq!(unix_to_rfc3339_utc(u2), "2026-07-28T14:08:44Z");
    }
}
