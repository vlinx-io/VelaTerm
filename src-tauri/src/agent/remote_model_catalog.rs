//! Website-owned Claude catalogue. One background fetch serves every local and remote view.
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use crate::host::AppCtx;
use super::claude_models::ClaudeModel;

const URL: &str = "https://velaterm.com/api/model-catalog/claude";
const KEY: &str = "model-catalog.claude.v1";
const LIMIT: u64 = 1_048_576;
const INTERVAL: u64 = 6 * 60 * 60;
pub const EVENT: &str = "model-catalog://changed";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    id: String, label: String, description: String,
    context_window: Option<u64>, effort_levels: Vec<String>,
    min_version: Option<String>,
    #[serde(default)] supports_fast_mode: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog { schema_version: u32, revision: u64, models: Vec<Entry> }
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub revision: Option<u64>,
    pub source: String,
    pub checked_at: Option<u64>,
    pub error: Option<String>,
    pub refreshing: bool,
    /// Set when the list comes from the installed CLI (`source: "cli"`): its `--version` string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_version: Option<String>,
}
#[derive(Default)]
struct State { catalog: Option<Catalog>, status: Status, attempted_at: u64 }
static STATE: Mutex<Option<State>> = Mutex::new(None);
static STARTED: OnceLock<()> = OnceLock::new();
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }

fn version(value: &str) -> Option<(u32,u32,u32)> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || p.len() > 6 || !p.chars().all(|c| c.is_ascii_digit())) { return None; }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
}
fn parse(bytes: &[u8]) -> Result<Catalog, String> {
    if bytes.len() as u64 > LIMIT { return Err("invalidCatalog".into()); }
    let catalog: Catalog = serde_json::from_slice(bytes).map_err(|_| "invalidCatalog")?;
    let mut ids = HashSet::new();
    if catalog.schema_version != 1 || catalog.revision == 0 || catalog.models.is_empty() || catalog.models.len() > 1000 { return Err("invalidCatalog".into()); }
    for m in &catalog.models {
        if m.id.is_empty() || m.id.len() > 200 || !m.id.starts_with(|c: char| c.is_ascii_alphanumeric())
            || !m.id.chars().all(|c| c.is_ascii_alphanumeric() || "._:/[]-".contains(c))
            || !ids.insert(&m.id) || m.label.trim().is_empty() || m.label.chars().count() > 200
            || m.description.chars().count() > 2000 || m.label.chars().chain(m.description.chars()).any(|c| c < ' ')
            || m.context_window.is_some_and(|n| n == 0 || n > 100_000_000)
            || m.min_version.as_ref().is_some_and(|v| version(v).is_none())
            || m.effort_levels.len() > 16 || m.effort_levels.iter().collect::<HashSet<_>>().len() != m.effort_levels.len()
            || m.effort_levels.iter().any(|s| s.is_empty() || s.len() > 32 || !s.starts_with(|c: char| c.is_ascii_lowercase()) || !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-".contains(c))) {
            return Err("invalidCatalog".into());
        }
    }
    Ok(catalog)
}

pub fn models(installed: Option<(u32,u32,u32)>) -> Option<Vec<ClaudeModel>> {
    let guard = STATE.lock().unwrap();
    let catalog = guard.as_ref()?.catalog.as_ref()?;
    Some(catalog.models.iter().filter(|m| match (installed, m.min_version.as_deref().and_then(version)) {
        (Some(have), Some(min)) => have >= min, _ => true,
    }).map(|m| ClaudeModel {
        id: m.id.clone(), label: m.label.clone(), description: m.description.clone(),
        context_window: m.context_window, effort_levels: m.effort_levels.clone(), curated: true,
        large_context: m.id.ends_with("[1m]"), supports_fast_mode: m.supports_fast_mode, is_default: false,
    }).collect())
}
pub fn status() -> Status {
    STATE.lock().unwrap().as_ref().map(|s| s.status.clone()).unwrap_or_else(|| Status { source: "bundled".into(), ..Status::default() })
}
fn endpoint() -> String { std::env::var("VLX_MODEL_CATALOG_URL").unwrap_or_else(|_| URL.into()) }
fn fetch(url: &str, previous: Option<&Catalog>) -> Result<Option<Catalog>, String> {
    let parsed = url::Url::parse(url).map_err(|_| "invalidUrl")?;
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))) { return Err("invalidUrl".into()); }
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).redirects(0).build();
    let mut request = agent.get(url).set("Accept", "application/json");
    let etag = previous.map(|c| format!("\"claude-{}\"", c.revision));
    if let Some(tag) = &etag { request = request.set("If-None-Match", tag); }
    let response = request.call().map_err(|_| "downloadFailed")?;
    if response.status() == 304 && previous.is_some() { return Ok(None); }
    if response.status() != 200 { return Err("downloadFailed".into()); }
    let mut bytes = Vec::new();
    response.into_reader().take(LIMIT + 1).read_to_end(&mut bytes).map_err(|_| "downloadFailed")?;
    let next = parse(&bytes)?;
    if let Some(old) = previous {
        if next.revision < old.revision || (next.revision == old.revision && serde_json::to_vec(&next).ok() != serde_json::to_vec(old).ok()) { return Err("invalidRevision".into()); }
    }
    Ok(Some(next))
}

pub fn refresh(ctx: &AppCtx) -> Status {
    let started = std::time::Instant::now();
    let previous = {
        let mut guard = STATE.lock().unwrap();
        let state = guard.get_or_insert_with(State::default);
        if state.status.refreshing { return state.status.clone(); }
        state.status.refreshing = true;
        state.attempted_at = now();
        state.catalog.clone()
    };
    ctx.emit(EVENT, status());
    let result = fetch(&endpoint(), previous.as_ref()).and_then(|new| {
        if let Some(catalog) = &new {
            let json = serde_json::to_string(catalog).map_err(|_| "cacheFailed")?;
            let mut entries = HashMap::new(); entries.insert(KEY.to_string(), json);
            let conn = ctx.db().conn.lock().map_err(|_| "cacheFailed")?;
            crate::db::repo::set_app_settings(&conn, &entries).map_err(|_| "cacheFailed")?;
        }
        Ok(new)
    });
    let snapshot = {
        let mut guard = STATE.lock().unwrap(); let state = guard.as_mut().unwrap();
        state.status.refreshing = false;
        match result {
            Ok(new) => {
                if let Some(catalog) = new { state.catalog = Some(catalog); }
                state.status.revision = state.catalog.as_ref().map(|c| c.revision);
                state.status.source = "website".into(); state.status.checked_at = Some(now()); state.status.error = None;
            }
            Err(error) => { state.status.error = Some(error); state.status.source = if state.catalog.is_some() { "cache" } else { "bundled" }.into(); }
        }
        state.status.clone()
    };
    audit(ctx, &snapshot, started.elapsed().as_millis());
    ctx.emit(EVENT, snapshot.clone()); snapshot
}

fn audit(ctx: &AppCtx, status: &Status, duration: u128) {
    let _=ctx;
    let level=if status.error.is_some(){"WARN"}else{"INFO"};
    if !crate::diagnostics::enabled(&std::env::var("VLX_MODEL_CATALOG_LOG_LEVEL").unwrap_or_else(|_|"info".into()),level){return;}
    crate::diagnostics::record(level,"model_catalog_sync",serde_json::json!({"status":if status.error.is_some(){"failed"}else{"success"},"revision":status.revision,"durationMs":duration as u64}));
}

pub fn start(ctx: AppCtx) {
    if STARTED.set(()).is_err() { return; }
    let stored = { let conn = ctx.db().conn.lock().unwrap(); crate::db::repo::get_app_settings(&conn).ok().and_then(|mut m| m.remove(KEY)) };
    let catalog = stored.as_ref().and_then(|s| parse(s.as_bytes()).ok());
    let state = State { status: Status { revision: catalog.as_ref().map(|c| c.revision), source: if catalog.is_some() { "cache" } else { "bundled" }.into(), ..Status::default() }, catalog, attempted_at: 0 };
    *STATE.lock().unwrap() = Some(state);
    std::thread::spawn(move || loop {
        let due = STATE.lock().unwrap().as_ref().is_some_and(|s| !s.status.refreshing && now().saturating_sub(s.attempted_at) >= if s.status.error.is_some() { 300 } else { INTERVAL });
        if due { refresh(&ctx); }
        std::thread::sleep(Duration::from_secs(15));
    });
}

/// Replace the in-memory website catalogue; `None` clears it. Callers serialize through
/// `cli_model_catalog::TEST_LOCK`.
#[cfg(test)]
pub(crate) fn set_for_tests(json: Option<&[u8]>) {
    let catalog = json.map(|bytes| parse(bytes).expect("valid test catalogue"));
    let mut guard = STATE.lock().unwrap();
    let state = guard.get_or_insert_with(State::default);
    state.status.source = if catalog.is_some() { "cache" } else { "bundled" }.into();
    state.catalog = catalog;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example(revision: u64, id: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"revision":revision,"models":[{
            "id":id,"label":"Test model","description":"Test catalogue entry", "contextWindow":200000,
            "effortLevels":["low","high"],"minVersion":"2.1.0"
        }]})).unwrap()
    }
    #[test]
    fn accepts_future_models_and_rejects_invalid_catalogues() {
        let catalog = parse(&example(12, "test-future-model")).unwrap();
        assert_eq!(catalog.models[0].id, "test-future-model");
        assert_eq!(catalog.revision, 12);
        let mut json: serde_json::Value = serde_json::from_slice(&example(1, "test-model")).unwrap();
        json["models"][0]["minVersion"] = "latest".into();
        assert!(parse(&serde_json::to_vec(&json).unwrap()).is_err());
        assert!(parse(&example(0, "test-model")).is_err());
        assert!(parse(&example(1, "--bad\n")).is_err());
        assert!(parse(&vec![b' '; LIMIT as usize + 1]).is_err());
        assert!(version("2.1.9").unwrap() < version("2.1.10").unwrap());
    }
    #[test]
    fn cache_round_trip_keeps_every_model_field() {
        let original = parse(&example(3, "test-new-model[1m]")).unwrap();
        let restored = parse(&serde_json::to_vec(&original).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(original).unwrap(), serde_json::to_value(restored).unwrap());
    }
    #[test]
    fn unsafe_endpoint_is_rejected_before_network_access() {
        assert_eq!(fetch("http://example.com/catalog", None).unwrap_err(), "invalidUrl");
    }
}
