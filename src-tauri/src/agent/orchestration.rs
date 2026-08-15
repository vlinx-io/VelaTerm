//! Orchestration profiles and resource guardrails for spawned child sessions.
//! Settings are optional and invalid values fall back to defaults.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::{json, Map, Value};

use crate::host::AppCtx;

/// Frontend settings blob holding both `orchestrationProfiles` and `orchestration`.
const VLX_SETTINGS_KEY: &str = "vlx-settings";
/// Agent, model, and effort that replace Claude in the default profiles when Claude is not installed.
const CLAUDE_FALLBACK: (&str, &str, &str) = ("codex", "gpt-5.6-sol", "high");

/// One reusable routing choice. Each field is individually optional; an unset field means the
/// caller's explicit flag or the frontend's per-agent default applies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    pub description: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub worktree: Option<bool>,
    pub permission_mode: Option<String>,
}

impl Profile {
    /// Serialize the profile for `vagent config`.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        if let Some(v) = &self.description {
            map.insert("description".into(), json!(v));
        }
        if let Some(v) = &self.agent {
            map.insert("agent".into(), json!(v));
        }
        if let Some(v) = &self.model {
            map.insert("model".into(), json!(v));
        }
        if let Some(v) = &self.effort {
            map.insert("effort".into(), json!(v));
        }
        if let Some(v) = self.worktree {
            map.insert("worktree".into(), json!(v));
        }
        if let Some(v) = &self.permission_mode {
            map.insert("permissionMode".into(), json!(v));
        }
        Value::Object(map)
    }

    fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        let text = |key: &str| {
            obj.get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        Some(Self {
            description: text("description"),
            agent: text("agent"),
            model: text("model"),
            effort: text("effort"),
            worktree: obj.get("worktree").and_then(Value::as_bool),
            permission_mode: Some(
                text("permissionMode")
                    .filter(|mode| matches!(mode.as_str(), "default" | "skip" | "inherit"))
                    .unwrap_or_else(|| "inherit".to_string()),
            ),
        })
    }
}

/// Resource guardrails applied to one lead session's subtree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub max_descendants: u32,
    pub max_parallel: u32,
    pub max_depth: u32,
    pub require_confirmation_above: u32,
    pub auto_approve: bool,
    /// Whether a plain `retire --confirm` may skip its confirmation card. A retire that also removes
    /// worktrees always shows the card.
    pub auto_approve_retire: bool,
    pub default_timeout_secs: u64,
    pub worktree_copy_patterns: Vec<String>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_descendants: 10,
            max_parallel: 4,
            max_depth: 2,
            require_confirmation_above: 6,
            auto_approve: false,
            auto_approve_retire: false,
            default_timeout_secs: 1800,
            worktree_copy_patterns: vec!["docs/plans/**".to_string()],
        }
    }
}

impl Limits {
    /// Serialize limits for `vagent config`.
    pub fn to_json(&self) -> Value {
        json!({
            "maxDescendants": self.max_descendants,
            "maxParallel": self.max_parallel,
            "maxDepth": self.max_depth,
            "requireConfirmationAbove": self.require_confirmation_above,
            "autoApprove": self.auto_approve,
            "autoApproveRetire": self.auto_approve_retire,
            "defaultTimeoutSecs": self.default_timeout_secs,
            "worktreeCopyPatterns": self.worktree_copy_patterns,
        })
    }

    fn from_json(v: &Value) -> Self {
        let mut limits = Self::default();
        let Some(obj) = v.as_object() else {
            return limits;
        };
        let count = |key: &str| obj.get(key).and_then(Value::as_u64);
        if let Some(n) = count("maxDescendants") {
            limits.max_descendants = n as u32;
        }
        if let Some(n) = count("maxParallel") {
            limits.max_parallel = n as u32;
        }
        if let Some(n) = count("maxDepth") {
            limits.max_depth = n as u32;
        }
        if let Some(n) = count("requireConfirmationAbove") {
            limits.require_confirmation_above = n as u32;
        }
        if let Some(v) = obj.get("autoApprove").and_then(Value::as_bool) {
            limits.auto_approve = v;
        }
        if let Some(v) = obj.get("autoApproveRetire").and_then(Value::as_bool) {
            limits.auto_approve_retire = v;
        }
        if let Some(n) = count("defaultTimeoutSecs") {
            limits.default_timeout_secs = n;
        }
        if let Some(list) = obj.get("worktreeCopyPatterns").and_then(Value::as_array) {
            limits.worktree_copy_patterns = list
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
        }
        limits
    }
}

/// The effective orchestration configuration for one spawn decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestrationConfig {
    pub profiles: HashMap<String, Profile>,
    pub limits: Limits,
}

impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            profiles: default_profiles(),
            limits: Limits::default(),
        }
    }
}

impl OrchestrationConfig {
    /// camelCase JSON object of every profile, keyed by name.
    pub fn profiles_json(&self) -> Value {
        let mut map = Map::new();
        for (name, profile) in &self.profiles {
            map.insert(name.clone(), profile.to_json());
        }
        Value::Object(map)
    }

    /// Return profile names in sorted order.
    pub fn profile_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.profiles.keys().cloned().collect();
        names.sort();
        names
    }
}

fn default_profiles() -> HashMap<String, Profile> {
    let mk = |description: &str, agent: &str, model: &str, effort: &str| Profile {
        description: Some(description.to_string()),
        agent: Some(agent.to_string()),
        model: Some(model.to_string()),
        effort: Some(effort.to_string()),
        worktree: Some(true),
        permission_mode: Some("inherit".to_string()),
    };
    HashMap::from([
        (
            "database".to_string(),
            mk(
                "Use for database schemas, migrations, queries, indexes, persistence, and data access.",
                "claude",
                "opus",
                "high",
            ),
        ),
        (
            "frontend".to_string(),
            mk(
                "Use for UI components, routes, styling, responsive behavior, and browser interactions.",
                "claude",
                "opus",
                "high",
            ),
        ),
        (
            "quick-edits".to_string(),
            mk(
                "Use for simple, well-scoped updates such as find-and-replace changes, small configuration edits, text revisions, and other mechanical changes.",
                "codex",
                "gpt-5.6-luna",
                "xhigh",
            ),
        ),
        (
            "tests".to_string(),
            mk(
                "Use for focused unit, integration, regression, and end-to-end tests.",
                "codex",
                "gpt-5.6-luna",
                "xhigh",
            ),
        ),
    ])
}

/// Default profiles with every Claude entry rewritten to `CLAUDE_FALLBACK` when Claude is absent.
fn default_profiles_for(claude_available: bool) -> HashMap<String, Profile> {
    let mut profiles = default_profiles();
    if claude_available {
        return profiles;
    }
    let (agent, model, effort) = CLAUDE_FALLBACK;
    for profile in profiles.values_mut() {
        if profile.agent.as_deref() == Some("claude") {
            profile.agent = Some(agent.to_string());
            profile.model = Some(model.to_string());
            profile.effort = Some(effort.to_string());
        }
    }
    profiles
}

/// Default profiles resolved against the installed agents, detected once per process.
fn resolved_default_profiles() -> HashMap<String, Profile> {
    static CLAUDE_AVAILABLE: OnceLock<bool> = OnceLock::new();
    let available =
        *CLAUDE_AVAILABLE.get_or_init(|| crate::agent::install::agent_available("claude"));
    default_profiles_for(available)
}

/// Resolved default profiles as the camelCase map the frontend stores in `orchestrationProfiles`.
pub fn resolved_default_profiles_json() -> Value {
    let mut map = Map::new();
    for (name, profile) in resolved_default_profiles() {
        map.insert(name, profile.to_json());
    }
    Value::Object(map)
}

/// Parse stored settings, calling `defaults` only when the blob holds no profile map.
pub fn parse_config_with(
    settings_json: Option<&str>,
    defaults: fn() -> HashMap<String, Profile>,
) -> OrchestrationConfig {
    let Some(parsed) = settings_json.and_then(|s| serde_json::from_str::<Value>(s).ok()) else {
        return OrchestrationConfig {
            profiles: defaults(),
            limits: Limits::default(),
        };
    };
    let profiles = match parsed
        .get("orchestrationProfiles")
        .and_then(Value::as_object)
    {
        Some(obj) => obj
            .iter()
            .filter_map(|(name, v)| Some((name.clone(), Profile::from_json(v)?)))
            .collect(),
        None => defaults(),
    };
    let limits = match parsed.get("orchestration") {
        Some(v) => Limits::from_json(v),
        None => Limits::default(),
    };
    OrchestrationConfig { profiles, limits }
}

/// Read settings from the database, falling back to defaults on read errors.
pub fn load(app: &AppCtx) -> OrchestrationConfig {
    let stored = app
        .db()
        .conn
        .lock()
        .ok()
        .and_then(|conn| crate::db::repo::get_app_settings(&conn).ok())
        .and_then(|mut settings| settings.remove(VLX_SETTINGS_KEY));
    parse_config_with(stored.as_deref(), resolved_default_profiles)
}

/// Spawn options after profile resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSpawn {
    pub kind: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub worktree: bool,
    pub permission_mode: Option<String>,
}

/// Merge explicit CLI values over a profile.
pub fn resolve_spawn(
    profile: Option<&Profile>,
    kind: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    worktree: Option<bool>,
    permission_mode: Option<String>,
) -> ResolvedSpawn {
    ResolvedSpawn {
        kind: kind.or_else(|| profile.and_then(|p| p.agent.clone())),
        model: model.or_else(|| profile.and_then(|p| p.model.clone())),
        effort: effort.or_else(|| profile.and_then(|p| p.effort.clone())),
        worktree: worktree
            .or_else(|| profile.and_then(|p| p.worktree))
            .unwrap_or(false),
        permission_mode: permission_mode
            .or_else(|| profile.and_then(|p| p.permission_mode.clone())),
    }
}

/// A spawn rejected by a resource limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitRejection {
    pub message: String,
    pub limit: &'static str,
    pub value: u32,
    pub current: u32,
}

impl LimitRejection {
    pub fn to_json(&self) -> Value {
        json!({
            "error": self.message,
            "limit": self.limit,
            "limitValue": self.value,
            "current": self.current,
        })
    }
}

/// Check one prospective spawn against the depth, descendant, and parallel limits.
pub fn check_limits(
    limits: &Limits,
    parent_depth: u32,
    descendants: u32,
    active: u32,
) -> Option<LimitRejection> {
    let new_depth = parent_depth + 1;
    if new_depth > limits.max_depth {
        return Some(LimitRejection {
            message: format!(
                "max_depth limit reached (spawn would be depth {new_depth} of {})",
                limits.max_depth
            ),
            limit: "max_depth",
            value: limits.max_depth,
            current: new_depth,
        });
    }
    if descendants >= limits.max_descendants {
        return Some(LimitRejection {
            message: format!(
                "max_descendants limit reached ({descendants} of {} retained descendants). Waiting does not free a slot; retire a settled child session.",
                limits.max_descendants
            ),
            limit: "max_descendants",
            value: limits.max_descendants,
            current: descendants,
        });
    }
    if active >= limits.max_parallel {
        return Some(LimitRejection {
            message: format!(
                "max_parallel limit reached ({active} of {} active children)",
                limits.max_parallel
            ),
            limit: "max_parallel",
            value: limits.max_parallel,
            current: active,
        });
    }
    None
}

/// Whether the spawn must show the confirmation card even when the user disabled confirmation.
/// `active` is the count of descendants that hold a parallel slot before this spawn.
pub fn needs_confirmation(limits: &Limits, active: u32) -> bool {
    active + 1 > limits.require_confirmation_above
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_config(settings_json: Option<&str>) -> OrchestrationConfig {
        parse_config_with(settings_json, default_profiles)
    }

    #[test]
    fn missing_or_invalid_settings_yield_defaults() {
        let default = OrchestrationConfig::default();
        assert_eq!(parse_config(None), default);
        assert_eq!(parse_config(Some("not json")), default);
        assert_eq!(parse_config(Some("{}")), default);
        assert_eq!(parse_config(Some(r#"{"theme":"dark"}"#)), default);

        assert_eq!(default.limits.max_descendants, 10);
        assert_eq!(default.limits.max_parallel, 4);
        assert_eq!(default.limits.max_depth, 2);
        assert_eq!(default.limits.require_confirmation_above, 6);
        assert_eq!(default.limits.default_timeout_secs, 1800);
        assert_eq!(default.limits.worktree_copy_patterns, vec!["docs/plans/**"]);
        assert_eq!(
            default.profile_names(),
            vec!["database", "frontend", "quick-edits", "tests"]
        );
        assert_eq!(
            default.profiles["database"],
            Profile {
                description: Some(
                    "Use for database schemas, migrations, queries, indexes, persistence, and data access."
                        .into()
                ),
                agent: Some("claude".into()),
                model: Some("opus".into()),
                effort: Some("high".into()),
                worktree: Some(true),
                permission_mode: Some("inherit".into()),
            }
        );
        assert_eq!(default.profiles["frontend"].model.as_deref(), Some("opus"));
        assert_eq!(
            default.profiles["quick-edits"],
            Profile {
                description: Some(
                    "Use for simple, well-scoped updates such as find-and-replace changes, small configuration edits, text revisions, and other mechanical changes."
                        .into()
                ),
                agent: Some("codex".into()),
                model: Some("gpt-5.6-luna".into()),
                effort: Some("xhigh".into()),
                worktree: Some(true),
                permission_mode: Some("inherit".into()),
            }
        );
    }

    #[test]
    fn absent_claude_moves_the_claude_profiles_to_codex() {
        let base = default_profiles();
        assert_eq!(default_profiles_for(true), base);

        let fallback = default_profiles_for(false);
        for name in ["database", "frontend"] {
            let profile = &fallback[name];
            assert_eq!(profile.agent.as_deref(), Some("codex"));
            assert_eq!(profile.model.as_deref(), Some("gpt-5.6-sol"));
            assert_eq!(profile.effort.as_deref(), Some("high"));
            assert_eq!(profile.description, base[name].description);
            assert_eq!(profile.worktree, base[name].worktree);
        }
        for name in ["quick-edits", "tests"] {
            assert_eq!(fallback[name], base[name]);
        }
    }

    #[test]
    fn stored_profiles_skip_the_installed_agent_check() {
        let stored = parse_config_with(
            Some(r#"{"orchestrationProfiles":{"deep":{"agent":"claude"}}}"#),
            || panic!("stored profiles must not trigger agent detection"),
        );
        assert_eq!(stored.profile_names(), vec!["deep"]);
    }

    #[test]
    fn limits_fall_back_field_by_field() {
        let c = parse_config(Some(
            r#"{"orchestration":{"maxDescendants":3,"defaultTimeoutSecs":60}}"#,
        ));
        assert_eq!(c.limits.max_descendants, 3);
        assert_eq!(c.limits.default_timeout_secs, 60);
        assert_eq!(c.limits.max_parallel, 4);
        assert_eq!(c.limits.max_depth, 2);
        assert_eq!(c.limits.require_confirmation_above, 6);
        assert!(!c.limits.auto_approve);
        assert_eq!(c.limits.worktree_copy_patterns, vec!["docs/plans/**"]);
        assert_eq!(c.profiles, default_profiles());

        let c = parse_config(Some(r#"{"orchestration":"nope"}"#));
        assert_eq!(c.limits, Limits::default());
        let c = parse_config(Some(
            r#"{"orchestration":{"maxDepth":"deep","maxParallel":9}}"#,
        ));
        assert_eq!(c.limits.max_depth, 2);
        assert_eq!(c.limits.max_parallel, 9);

        let c = parse_config(Some(
            r#"{"orchestration":{"worktreeCopyPatterns":["a/**","b"]}}"#,
        ));
        assert_eq!(c.limits.worktree_copy_patterns, vec!["a/**", "b"]);
    }

    #[test]
    fn stored_profiles_replace_the_defaults() {
        let c = parse_config(Some(
            r#"{"orchestrationProfiles":{"quick":{"description":" Use for quick fixes. ","agent":"claude"},"deep":{"model":"fable","worktree":false}}}"#,
        ));
        assert_eq!(c.profile_names(), vec!["deep", "quick"]);
        assert_eq!(
            c.profiles["quick"],
            Profile {
                description: Some("Use for quick fixes.".into()),
                agent: Some("claude".into()),
                permission_mode: Some("inherit".into()),
                ..Default::default()
            }
        );
        assert_eq!(
            c.profiles["deep"],
            Profile {
                model: Some("fable".into()),
                worktree: Some(false),
                permission_mode: Some("inherit".into()),
                ..Default::default()
            }
        );
        assert_eq!(c.limits, Limits::default());
        let c = parse_config(Some(r#"{"orchestrationProfiles":{"bad":7,"ok":{}}}"#));
        assert_eq!(c.profile_names(), vec!["ok"]);
        assert_eq!(
            c.profiles["ok"],
            Profile {
                permission_mode: Some("inherit".into()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn explicit_values_beat_profile_values() {
        let profile = Profile {
            description: None,
            agent: Some("codex".into()),
            model: Some("gpt-5.6-luna".into()),
            effort: Some("xhigh".into()),
            worktree: Some(true),
            permission_mode: Some("skip".into()),
        };
        let r = resolve_spawn(
            Some(&profile),
            Some("claude".into()),
            Some("fable".into()),
            Some("low".into()),
            Some(false),
            Some("default".into()),
        );
        assert_eq!(
            r,
            ResolvedSpawn {
                kind: Some("claude".into()),
                model: Some("fable".into()),
                effort: Some("low".into()),
                worktree: false,
                permission_mode: Some("default".into()),
            }
        );

        let r = resolve_spawn(Some(&profile), None, None, None, None, None);
        assert_eq!(
            r,
            ResolvedSpawn {
                kind: Some("codex".into()),
                model: Some("gpt-5.6-luna".into()),
                effort: Some("xhigh".into()),
                worktree: true,
                permission_mode: Some("skip".into()),
            }
        );

        let partial = Profile {
            model: Some("opus".into()),
            ..Default::default()
        };
        let r = resolve_spawn(Some(&partial), None, None, Some("high".into()), None, None);
        assert_eq!(
            r,
            ResolvedSpawn {
                kind: None,
                model: Some("opus".into()),
                effort: Some("high".into()),
                worktree: false,
                permission_mode: None,
            }
        );

        let r = resolve_spawn(None, None, None, None, None, None);
        assert_eq!(
            r,
            ResolvedSpawn {
                kind: None,
                model: None,
                effort: None,
                worktree: false,
                permission_mode: None,
            }
        );
        assert!(resolve_spawn(None, None, None, None, Some(true), None).worktree);
    }

    #[test]
    fn permission_mode_resolves_from_explicit_then_profile() {
        let profile = Profile {
            permission_mode: Some("skip".into()),
            ..Default::default()
        };

        let explicit = resolve_spawn(
            Some(&profile),
            None,
            None,
            None,
            None,
            Some("default".into()),
        );
        assert_eq!(explicit.permission_mode.as_deref(), Some("default"));

        let profiled = resolve_spawn(Some(&profile), None, None, None, None, None);
        assert_eq!(profiled.permission_mode.as_deref(), Some("skip"));

        let inherited = resolve_spawn(None, None, None, None, None, None);
        assert_eq!(inherited.permission_mode, None);
    }

    #[test]
    fn inherit_permission_mode_survives_profile_resolution() {
        let config = parse_config(Some(
            r#"{"orchestrationProfiles":{"child":{"permissionMode":"inherit"}}}"#,
        ));
        let profiled = resolve_spawn(
            config.profiles.get("child"),
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(profiled.permission_mode.as_deref(), Some("inherit"));
    }

    #[test]
    fn limits_reject_at_the_boundary_in_order() {
        let limits = Limits::default();
        assert_eq!(check_limits(&limits, 1, 9, 3), None);

        let r = check_limits(&limits, 2, 0, 0).unwrap();
        assert_eq!(r.limit, "max_depth");
        assert_eq!(r.value, 2);
        assert_eq!(r.current, 3);
        assert_eq!(
            r.to_json()["error"],
            "max_depth limit reached (spawn would be depth 3 of 2)"
        );

        let r = check_limits(&limits, 1, 10, 0).unwrap();
        assert_eq!(r.limit, "max_descendants");
        assert_eq!(r.to_json()["limitValue"], 10);
        assert_eq!(r.to_json()["current"], 10);
        assert_eq!(
            r.to_json()["error"],
            "max_descendants limit reached (10 of 10 retained descendants). Waiting does not free a slot; retire a settled child session."
        );

        let r = check_limits(&limits, 1, 5, 4).unwrap();
        assert_eq!(r.limit, "max_parallel");
        assert_eq!(r.to_json()["limitValue"], 4);
        assert_eq!(
            r.to_json()["error"],
            "max_parallel limit reached (4 of 4 active children)"
        );
    }

    #[test]
    fn confirmation_triggers_above_the_threshold() {
        let limits = Limits::default();
        assert!(!needs_confirmation(&limits, 5));
        assert!(needs_confirmation(&limits, 6));

        let always = Limits {
            require_confirmation_above: 0,
            ..Limits::default()
        };
        assert!(needs_confirmation(&always, 0));
    }

    #[test]
    fn config_serializes_as_camel_case() {
        let c = OrchestrationConfig::default();
        let limits = c.limits.to_json();
        assert_eq!(limits["maxDescendants"], 10);
        assert_eq!(limits["requireConfirmationAbove"], 6);
        assert_eq!(limits["autoApprove"], false);
        assert_eq!(limits["defaultTimeoutSecs"], 1800);
        assert_eq!(limits["worktreeCopyPatterns"][0], "docs/plans/**");

        let profiles = c.profiles_json();
        assert_eq!(
            profiles["frontend"]["description"],
            "Use for UI components, routes, styling, responsive behavior, and browser interactions."
        );
        assert_eq!(profiles["tests"]["agent"], "codex");
        assert_eq!(profiles["tests"]["worktree"], true);
        assert_eq!(profiles["tests"]["permissionMode"], "inherit");
        let partial = Profile {
            description: Some("Use for review.".into()),
            agent: Some("claude".into()),
            ..Default::default()
        };
        assert_eq!(
            partial.to_json(),
            json!({"description": "Use for review.", "agent": "claude"})
        );
    }
}
