//! Cached Antigravity model catalogue and per-model thinking effort levels.
//!
//! `agy acp` takes ~2.7s per session handshake and `agy models` can take several seconds on a cold
//! start, so VelaTerm ships a built-in initial catalogue in code and layers runtime updates in
//! `app_settings` on top. Every session open / launch lookup returns the current catalogue
//! immediately and triggers a background refresh (`agy models` / `agy acp`), broadcasting
//! `antigravity://models-changed` whenever the available list changes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::chat::{acp_protocol, antigravity_protocol};
use super::session_settings::Selection;
use crate::db::repo;
use crate::host::AppCtx;

const CACHE_KEY: &str = "model-catalog.antigravity.v1";
const SCHEMA_VERSION: u32 = 1;
/// Bump whenever `bundled()` changes so older persisted caches are refreshed from the bundled list
/// until a live refresh replaces them.
const BUNDLED_REVISION: u32 = 1;
const REFRESH_TIMEOUT: Duration = Duration::from_secs(30);
const REFRESH_COOLDOWN: Duration = Duration::from_secs(5);
pub const EVENT: &str = "antigravity://models-changed";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityModel {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub effort_levels: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedCatalog {
    schema_version: u32,
    #[serde(default)]
    bundled_revision: u32,
    #[serde(default)]
    models: Vec<AntigravityModel>,
}

#[derive(Default)]
struct RefreshState {
    in_flight: bool,
    last_finished: Option<Instant>,
}

type RefreshKey = (Option<PathBuf>, String);
static REFRESH_STATES: OnceLock<Mutex<HashMap<RefreshKey, RefreshState>>> = OnceLock::new();

fn refresh_states() -> &'static Mutex<HashMap<RefreshKey, RefreshState>> {
    REFRESH_STATES.get_or_init(Default::default)
}

/// Built-in initial Antigravity model catalogue so even a first run has an immediate picker list.
pub fn bundled() -> Vec<AntigravityModel> {
    let entries: &[(&str, &str, &[&str])] = &[
        ("gemini-4-neon", "Gemini 4 Neon", &["low", "medium", "high", "xhigh"]),
        ("gemini-4-carbon", "Gemini 4 Carbon", &["low", "medium", "high"]),
        ("gemini-4-argon", "Gemini 4 Argon", &["low", "medium", "high"]),
        ("gemini-4-hydrogen", "Gemini 4 Hydrogen", &["low", "medium", "high", "xhigh"]),
        ("gemini-pro-orchestrator-neon", "Orchestrator (Neon)", &[]),
        ("gemini-pro-orchestrator", "Orchestrator (Carbon)", &[]),
        ("gemini-pro-orchestrator-argon", "Orchestrator (Argon)", &[]),
        ("gemini-3.8-flash", "Gemini 3.8 Flash", &["low", "medium", "high"]),
        ("claude-opus-5-5", "Claude Opus 5.5", &["low", "medium", "high"]),
        ("claude-sonnet-5-5", "Claude Sonnet 5.5", &["low", "medium", "high"]),
        ("gpt-oss-120b-medium", "GPT-OSS 120B (Medium)", &[]),
    ];
    entries
        .iter()
        .map(|(id, label, efforts)| AntigravityModel {
            id: (*id).to_string(),
            label: (*label).to_string(),
            description: String::new(),
            effort_levels: efforts.iter().map(|level| (*level).to_string()).collect(),
        })
        .collect()
}

/// Return the currently effective Antigravity models (persisted runtime cache or bundled list)
/// without spawning any external process.
pub fn cached(ctx: &AppCtx) -> Vec<AntigravityModel> {
    let conn = ctx.db().conn.lock().unwrap();
    effective_models_from_conn(&conn)
}

/// Return the cached/bundled Antigravity models immediately and schedule a background refresh via
/// `agy models`.
pub fn list(ctx: &AppCtx, bin: &str) -> Result<Vec<AntigravityModel>, String> {
    let models = cached(ctx);
    schedule_refresh(ctx, bin);
    Ok(models)
}

/// Schedule a background `agy models` refresh, coalescing concurrent calls and notifying open views
/// whenever the catalogue changes.
pub fn schedule_refresh(ctx: &AppCtx, bin: &str) {
    let bin = bin.trim();
    if bin.is_empty() {
        return;
    }
    if (bin.contains('/') || bin.contains('\\')) && !Path::new(bin).is_file() {
        return;
    }
    let key: RefreshKey = (ctx.data_dir().ok(), bin.to_string());
    {
        let mut map = refresh_states().lock().unwrap();
        let entry = map.entry(key.clone()).or_default();
        if entry.in_flight
            || entry
                .last_finished
                .is_some_and(|at| at.elapsed() < REFRESH_COOLDOWN)
        {
            return;
        }
        entry.in_flight = true;
    }
    let ctx = ctx.clone();
    let bin = bin.to_string();
    std::thread::spawn(move || {
        let parsed = super::model_catalog::run_list_command_with_timeout(
            "antigravity",
            &bin,
            REFRESH_TIMEOUT,
        )
        .ok()
        .map(|text| parse_cli_output(&text))
        .filter(|models| !models.is_empty());
        if let Some(models) = parsed {
            save_and_notify(&ctx, &models);
        }
        let mut map = refresh_states().lock().unwrap();
        if let Some(entry) = map.get_mut(&key) {
            entry.in_flight = false;
            entry.last_finished = Some(Instant::now());
        }
    });
}

/// Merge newly discovered cached/bundled models into a running ACP session's model list so
/// long-lived sessions also see newly rolled-out models immediately.
pub fn merge_live_with_cached(
    ctx: &AppCtx,
    mut live: Vec<AntigravityModel>,
) -> Vec<AntigravityModel> {
    let extras = cached(ctx);
    let mut seen: HashSet<String> = live.iter().map(|m| m.id.clone()).collect();
    for model in extras {
        if seen.insert(model.id.clone()) {
            live.push(model);
        }
    }
    live
}

/// Convert a live ACP session's catalogue into grouped Antigravity models with per-model effort levels.
pub fn models_from_acp(ctx: &AppCtx, catalog: &acp_protocol::Catalog) -> Vec<AntigravityModel> {
    let previous = {
        let conn = ctx.db().conn.lock().unwrap();
        read_cached_from_conn(&conn)
    };
    models_from_acp_with_previous(catalog, previous.as_ref())
}

/// Persist the live ACP session's models and per-model effort levels to SQLite, and notify open
/// sessions if the catalogue changed.
pub fn remember_acp(ctx: &AppCtx, catalog: &acp_protocol::Catalog) {
    if !catalog.model_switchable() {
        return;
    }
    let previous = {
        let conn = ctx.db().conn.lock().unwrap();
        read_cached_from_conn(&conn)
    };
    let models = models_from_acp_with_previous(catalog, previous.as_ref());
    if models.is_empty() {
        return;
    }
    save_and_notify(ctx, &models);
}

fn save_and_notify(ctx: &AppCtx, models: &[AntigravityModel]) {
    let payload = CachedCatalog {
        schema_version: SCHEMA_VERSION,
        bundled_revision: BUNDLED_REVISION,
        models: models.to_vec(),
    };
    let Ok(json) = serde_json::to_string(&payload) else {
        return;
    };
    let changed = {
        let conn = ctx.db().conn.lock().unwrap();
        let previous_effective = effective_models_from_conn(&conn);
        let previous_stored = read_cached_from_conn(&conn).map(|c| c.models);
        if previous_stored.as_deref() != Some(models) {
            let _ = repo::set_app_settings(&conn, &HashMap::from([(CACHE_KEY.to_string(), json)]));
        }
        previous_effective != models
    };
    if changed {
        ctx.emit(EVENT, ());
        ctx.chat().notify_antigravity_models_changed(ctx);
    }
}

/// Normalize a saved `(model, effort)` pair against the grouped Antigravity model rules.
///
/// Legacy conversations picked from raw `agy models` rows may store a combined slug such as
/// `gemini-4-hydrogen-xhigh`; split those into `(gemini-4-hydrogen, xhigh)` while keeping
/// single-variant ids like `gpt-oss-120b-medium` verbatim, and drop efforts that the resolved
/// model no longer accepts.
pub fn normalize_selection(ctx: &AppCtx, selection: &mut Selection) {
    let conn = ctx.db().conn.lock().unwrap();
    normalize_selection_with_conn(&conn, selection);
}

pub(crate) fn normalize_selection_with_conn(conn: &rusqlite::Connection, selection: &mut Selection) {
    let models = effective_models_from_conn(conn);
    if let Some(raw) = selection.model.clone() {
        let already_known = models.iter().any(|m| m.id == raw);
        if !already_known {
            if let Some((base, level, _)) = split_effort_variant(&raw, &raw) {
                let split_known = models
                    .iter()
                    .any(|m| m.id == base && !m.effort_levels.is_empty());
                if split_known {
                    selection.model = Some(base.to_string());
                    if selection.effort.is_none() {
                        selection.effort = Some(level.to_string());
                    }
                }
            }
        }
    }
    if let Some(model_id) = selection.model.as_deref() {
        if let Some(entry) = models.iter().find(|m| m.id == model_id) {
            if selection
                .effort
                .as_ref()
                .is_some_and(|eff| !entry.effort_levels.iter().any(|l| l == eff))
            {
                selection.effort = None;
            }
        }
    }
}

fn models_from_acp_with_previous(
    catalog: &acp_protocol::Catalog,
    previous: Option<&CachedCatalog>,
) -> Vec<AntigravityModel> {
    if catalog.models_from_state {
        let pairs: Vec<(String, String)> = catalog
            .models
            .iter()
            .map(|c| (c.id.clone(), c.name.clone()))
            .collect();
        let mut folded = fold_pairs(&pairs);
        for model in &mut folded {
            if let Some(live) = catalog.model_efforts.get(&model.id) {
                model.effort_levels = canonical_efforts(live);
            } else if model.effort_levels.is_empty() {
                if let Some(prev) = previous.and_then(|p| p.models.iter().find(|m| m.id == model.id)) {
                    model.effort_levels = prev.effort_levels.clone();
                }
            }
        }
        return folded;
    }

    let grouped_ids: HashSet<&str> = catalog.models.iter().map(|c| c.id.as_str()).collect();
    let mut derived: HashMap<String, Vec<String>> = HashMap::new();
    for choice in &catalog.state_models {
        if grouped_ids.contains(choice.id.as_str()) {
            derived.entry(choice.id.clone()).or_default();
        } else if let Some((base, level, _)) = split_effort_variant(&choice.id, &choice.name) {
            if grouped_ids.contains(base) {
                derived.entry(base.to_string()).or_default().push(level.to_string());
            }
        }
    }
    for levels in derived.values_mut() {
        *levels = canonical_efforts(levels);
    }
    for (id, levels) in &catalog.model_efforts {
        derived.insert(id.clone(), canonical_efforts(levels));
    }

    catalog
        .models
        .iter()
        .map(|choice| {
            let effort_levels = if let Some(levels) = derived.get(&choice.id) {
                levels.clone()
            } else if catalog.state_models.is_empty() {
                previous
                    .and_then(|p| p.models.iter().find(|m| m.id == choice.id))
                    .map(|m| m.effort_levels.clone())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            AntigravityModel {
                id: choice.id.clone(),
                label: choice.name.clone(),
                description: choice.description.clone().unwrap_or_default(),
                effort_levels,
            }
        })
        .collect()
}

/// Parse `agy models` stdout (`<slug>   <Display Name>`) and fold effort variants into base models.
pub fn parse_cli_output(text: &str) -> Vec<AntigravityModel> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some(first_ws) = line.find(char::is_whitespace) else {
            let id = line;
            if super::model_catalog::looks_like_id(id)
                && !id.ends_with(':')
                && seen.insert(id.to_string())
            {
                pairs.push((id.to_string(), id.to_string()));
            }
            continue;
        };
        let id = &line[..first_ws];
        if !super::model_catalog::looks_like_id(id) || id.ends_with(':') {
            continue;
        }
        if !seen.insert(id.to_string()) {
            continue;
        }
        let label = line[first_ws..].trim();
        let display = if label.is_empty() { id } else { label };
        pairs.push((id.to_string(), display.to_string()));
    }
    fold_pairs(&pairs)
}

/// Collapse `<base>-<effort>` variants into a single grouped model whenever a base has >= 2
/// effort variants, leaving single-variant slugs (such as `gpt-oss-120b-medium`) untouched.
pub fn fold_pairs(pairs: &[(String, String)]) -> Vec<AntigravityModel> {
    let mut variant_efforts: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (id, name) in pairs {
        if let Some((base, level, _)) = split_effort_variant(id, name) {
            variant_efforts.entry(base).or_default().insert(level);
        }
    }

    let mut out: Vec<AntigravityModel> = Vec::new();
    let mut index_by_id: HashMap<String, usize> = HashMap::new();
    for (id, name) in pairs {
        if let Some((base, level, base_label)) = split_effort_variant(id, name) {
            if variant_efforts.get(base).is_some_and(|set| set.len() >= 2) {
                if let Some(&idx) = index_by_id.get(base) {
                    if !out[idx].effort_levels.iter().any(|l| l == level) {
                        out[idx].effort_levels.push(level.to_string());
                    }
                } else {
                    index_by_id.insert(base.to_string(), out.len());
                    out.push(AntigravityModel {
                        id: base.to_string(),
                        label: base_label,
                        description: String::new(),
                        effort_levels: vec![level.to_string()],
                    });
                }
                continue;
            }
        }
        if !index_by_id.contains_key(id) {
            index_by_id.insert(id.clone(), out.len());
            out.push(AntigravityModel {
                id: id.clone(),
                label: if name.trim().is_empty() {
                    id.clone()
                } else {
                    name.trim().to_string()
                },
                description: String::new(),
                effort_levels: Vec::new(),
            });
        }
    }
    for model in &mut out {
        model.effort_levels = canonical_efforts(&model.effort_levels);
    }
    out
}

fn split_effort_variant<'a>(slug: &'a str, name: &str) -> Option<(&'a str, &'static str, String)> {
    for &level in antigravity_protocol::EFFORTS {
        if let Some(without_level) = slug.strip_suffix(level) {
            if let Some(base) = without_level.strip_suffix('-') {
                if !base.is_empty() {
                    let stripped = strip_effort_suffix(name);
                    let label = if stripped.is_empty() || stripped == slug {
                        base.to_string()
                    } else {
                        stripped.to_string()
                    };
                    return Some((base, level, label));
                }
            }
        }
    }
    None
}

fn strip_effort_suffix(name: &str) -> &str {
    let trimmed = name.trim();
    let lower = trimmed.to_ascii_lowercase();
    for suffix in [
        " (low)",
        " (medium)",
        " (high)",
        " (xhigh)",
        " (extra)",
        " (max)",
    ] {
        if lower.ends_with(suffix) {
            return trimmed[..trimmed.len() - suffix.len()].trim_end();
        }
    }
    trimmed
}

fn canonical_efforts(levels: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for &known in antigravity_protocol::EFFORTS {
        if levels.iter().any(|l| l == known) {
            out.push(known.to_string());
        }
    }
    for level in levels {
        if !out.iter().any(|existing| existing == level) {
            out.push(level.clone());
        }
    }
    out
}

fn read_cached_from_conn(conn: &rusqlite::Connection) -> Option<CachedCatalog> {
    let raw = repo::get_app_settings(conn).ok()?.remove(CACHE_KEY)?;
    let parsed: CachedCatalog = serde_json::from_str(&raw).ok()?;
    (parsed.schema_version == SCHEMA_VERSION
        && parsed.bundled_revision >= BUNDLED_REVISION
        && !parsed.models.is_empty())
    .then_some(parsed)
}

fn effective_models_from_conn(conn: &rusqlite::Connection) -> Vec<AntigravityModel> {
    read_cached_from_conn(conn)
        .map(|cached| cached.models)
        .unwrap_or_else(bundled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_ctx(name: &str) -> (AppCtx, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("vlx-agy-models-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let app = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(
            dir.clone(),
            db,
        )));
        (app, dir)
    }

    #[test]
    fn folds_cli_models_with_single_variant_preservation_and_effort_order() {
        let raw = "\
gemini-4-argon-medium              Gemini 4 Argon (Medium)
gemini-4-argon-high                Gemini 4 Argon (High)
gemini-4-argon-low                 Gemini 4 Argon (Low)
gemini-4-hydrogen-high             Gemini 4 Hydrogen (High)
gemini-4-hydrogen-xhigh            Gemini 4 Hydrogen (Extra)
gemini-4-hydrogen-medium           Gemini 4 Hydrogen (Medium)
gemini-4-hydrogen-low              Gemini 4 Hydrogen (Low)
gemini-pro-orchestrator            Orchestrator (Carbon)
gemini-pro-orchestrator-argon      Orchestrator (Argon)
gpt-oss-120b-medium                GPT-OSS 120B (Medium)
";
        let models = parse_cli_output(raw);
        assert_eq!(
            models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            [
                "gemini-4-argon",
                "gemini-4-hydrogen",
                "gemini-pro-orchestrator",
                "gemini-pro-orchestrator-argon",
                "gpt-oss-120b-medium",
            ]
        );
        assert_eq!(models[0].label, "Gemini 4 Argon");
        assert_eq!(models[0].effort_levels, ["low", "medium", "high"]);
        assert_eq!(models[1].label, "Gemini 4 Hydrogen");
        assert_eq!(models[1].effort_levels, ["low", "medium", "high", "xhigh"]);
        assert_eq!(models[2].label, "Orchestrator (Carbon)");
        assert!(models[2].effort_levels.is_empty());
        assert_eq!(models[3].label, "Orchestrator (Argon)");
        assert!(models[3].effort_levels.is_empty());
        assert_eq!(models[4].label, "GPT-OSS 120B (Medium)");
        assert!(models[4].effort_levels.is_empty());
    }

    #[test]
    fn acp_catalog_is_cached_with_per_model_efforts_and_served_without_spawning() {
        let (app, dir) = test_ctx("cache");
        let mut catalog = acp_protocol::Catalog::default();
        catalog.absorb_session(&json!({
            "models": {
                "availableModels": [
                    {"modelId": "gemini-4-argon-low", "name": "Gemini 4 Argon (Low)"},
                    {"modelId": "gemini-4-argon-medium", "name": "Gemini 4 Argon (Medium)"},
                    {"modelId": "gemini-4-argon-high", "name": "Gemini 4 Argon (High)"},
                    {"modelId": "gemini-4-hydrogen-low", "name": "Gemini 4 Hydrogen (Low)"},
                    {"modelId": "gemini-4-hydrogen-medium", "name": "Gemini 4 Hydrogen (Medium)"},
                    {"modelId": "gemini-4-hydrogen-high", "name": "Gemini 4 Hydrogen (High)"},
                    {"modelId": "gemini-4-hydrogen-xhigh", "name": "Gemini 4 Hydrogen (Extra)"},
                    {"modelId": "gemini-pro-orchestrator", "name": "Orchestrator (Carbon)"},
                    {"modelId": "gpt-oss-120b-medium", "name": "GPT-OSS 120B (Medium)"}
                ],
                "currentModelId": "gemini-4-argon-medium"
            },
            "modes": {
                "currentModeId": "default",
                "availableModes": [
                    {"id": "default", "name": "Default", "description": "Default mode"},
                    {"id": "plan", "name": "Plan", "description": "Plan mode"}
                ]
            },
            "configOptions": [
                {"type": "select", "id": "model", "category": "model", "currentValue": "gemini-4-argon", "options": [
                    {"value": "gemini-4-argon", "name": "Gemini 4 Argon"},
                    {"value": "gemini-4-hydrogen", "name": "Gemini 4 Hydrogen"},
                    {"value": "gemini-pro-orchestrator", "name": "Orchestrator (Carbon)"},
                    {"value": "gpt-oss-120b-medium", "name": "GPT-OSS 120B (Medium)"}
                ]},
                {"type": "select", "id": "thought_level", "category": "thought_level", "currentValue": "medium", "options": [
                    {"value": "low", "name": "Low"},
                    {"value": "medium", "name": "Medium"},
                    {"value": "high", "name": "High"}
                ]}
            ]
        }));
        remember_acp(&app, &catalog);

        // Passing a nonexistent binary verifies `list` answers from the persistent cache.
        let cached = list(&app, "/nonexistent/agy").unwrap();
        assert_eq!(cached.len(), 4);
        assert_eq!(cached[0].effort_levels, ["low", "medium", "high"]);
        assert_eq!(cached[1].effort_levels, ["low", "medium", "high", "xhigh"]);
        assert!(cached[2].effort_levels.is_empty());
        assert!(cached[3].effort_levels.is_empty());

        // Normalize legacy variant slug and invalid efforts against cached metadata.
        let mut legacy = Selection {
            model: Some("gemini-4-hydrogen-xhigh".into()),
            effort: None,
        };
        normalize_selection(&app, &mut legacy);
        assert_eq!(legacy.model.as_deref(), Some("gemini-4-hydrogen"));
        assert_eq!(legacy.effort.as_deref(), Some("xhigh"));

        let mut single_variant = Selection {
            model: Some("gpt-oss-120b-medium".into()),
            effort: Some("high".into()),
        };
        normalize_selection(&app, &mut single_variant);
        assert_eq!(single_variant.model.as_deref(), Some("gpt-oss-120b-medium"));
        assert!(single_variant.effort.is_none());

        let mut over_argon = Selection {
            model: Some("gemini-4-argon".into()),
            effort: Some("xhigh".into()),
        };
        normalize_selection(&app, &mut over_argon);
        assert!(over_argon.effort.is_none());

        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn selection_normalization_handles_cold_database_safely() {
        let (app, dir) = test_ctx("cold");
        let mut legacy = Selection {
            model: Some("gemini-4-hydrogen-xhigh".into()),
            effort: None,
        };
        normalize_selection(&app, &mut legacy);
        assert_eq!(legacy.model.as_deref(), Some("gemini-4-hydrogen"));
        assert_eq!(legacy.effort.as_deref(), Some("xhigh"));

        let mut gpt_oss = Selection {
            model: Some("gpt-oss-120b-medium".into()),
            effort: None,
        };
        normalize_selection(&app, &mut gpt_oss);
        assert_eq!(gpt_oss.model.as_deref(), Some("gpt-oss-120b-medium"));
        assert!(gpt_oss.effort.is_none());

        let mut synthetic = Selection {
            model: Some("chosen-model".into()),
            effort: Some("high".into()),
        };
        normalize_selection(&app, &mut synthetic);
        assert_eq!(synthetic.model.as_deref(), Some("chosen-model"));
        assert_eq!(synthetic.effort.as_deref(), Some("high"));

        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cold_and_legacy_databases_return_bundled_catalogue_with_neon_immediately() {
        let (app, dir) = test_ctx("bundled");
        let cold = list(&app, "/nonexistent/agy").unwrap();
        assert!(cold.iter().any(|m| m.id == "gemini-4-neon"
            && m.label == "Gemini 4 Neon"
            && m.effort_levels == ["low", "medium", "high", "xhigh"]));
        assert!(cold.iter().any(|m| m.id == "gemini-pro-orchestrator-neon"));

        // A legacy persisted entry lacking `bundledRevision` is superseded by the bundled list.
        {
            let conn = app.db().conn.lock().unwrap();
            let legacy_json = json!({
                "schemaVersion": SCHEMA_VERSION,
                "models": [
                    {"id": "gemini-4-argon", "label": "Gemini 4 Argon", "effortLevels": ["low", "medium", "high"]}
                ]
            })
            .to_string();
            repo::set_app_settings(&conn, &HashMap::from([(CACHE_KEY.to_string(), legacy_json)]))
                .unwrap();
        }
        let upgraded = cached(&app);
        assert!(upgraded.iter().any(|m| m.id == "gemini-4-neon"));

        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn merge_live_with_cached_appends_only_missing_models() {
        let (app, dir) = test_ctx("merge");
        let live = vec![AntigravityModel {
            id: "gemini-4-argon".into(),
            label: "Gemini 4 Argon".into(),
            description: String::new(),
            effort_levels: vec!["low".into(), "medium".into(), "high".into()],
        }];
        let merged = merge_live_with_cached(&app, live);
        assert_eq!(
            merged.iter().filter(|m| m.id == "gemini-4-argon").count(),
            1
        );
        assert!(merged.iter().any(|m| m.id == "gemini-4-neon"));

        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn background_refresh_updates_cache_and_emits_only_when_changed() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let (app, dir) = test_ctx("bg-refresh");
        let script = dir.join("agy");
        std::fs::write(
            &script,
            "#!/bin/sh\necho 'gemini-5-xenon-low\tGemini 5 Xenon (Low)'\necho 'gemini-5-xenon-high\tGemini 5 Xenon (High)'\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();

        let events = Arc::new(AtomicUsize::new(0));
        let seen = events.clone();
        app.listen(EVENT, move |_| {
            seen.fetch_add(1, Ordering::Relaxed);
        });

        let initial = list(&app, script.to_str().unwrap()).unwrap();
        assert!(initial.iter().any(|m| m.id == "gemini-4-neon"));

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while events.load(Ordering::Relaxed) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert_eq!(events.load(Ordering::Relaxed), 1);

        let updated = cached(&app);
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].id, "gemini-5-xenon");
        assert_eq!(updated[0].effort_levels, ["low", "high"]);

        // Saving the identical list again must not re-emit `EVENT`.
        save_and_notify(&app, &updated);
        assert_eq!(events.load(Ordering::Relaxed), 1);

        // Wait for the background thread to clear `in_flight` before removing `dir`.
        let key: RefreshKey = (app.data_dir().ok(), script.to_string_lossy().into_owned());
        while refresh_states()
            .lock()
            .unwrap()
            .get(&key)
            .is_some_and(|s| s.in_flight)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }
}
