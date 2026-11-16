//! The DeepSeek routes a conversation can choose.
//!
//! DeepSeek sessions are driven by OpenCode, but they expose only the official DeepSeek provider. The
//! catalogue therefore comes from OpenCode's `/provider` answer and is filtered to [`PROVIDER`], and the
//! saved selection is normalized to OpenCode's `provider/model` spelling.

use serde_json::Value;

use super::opencode_protocol::OpencodeModel;

/// The OpenCode provider that talks to the DeepSeek API.
pub const PROVIDER: &str = "deepseek";

/// The model a DeepSeek conversation starts on when it has no saved model. It is the vision model and
/// matches the route DeepSeek Harness used before the engine moved to OpenCode.
pub const DEFAULT_MODEL: &str = "deepseek-flash";

/// The retired id older DeepSeek conversations may still carry.
const LEGACY_MODEL: &str = "deepseek-v4-flash";

/// The inline OpenCode configuration a DeepSeek server starts with.
///
/// Every model OpenCode chooses by itself — titles, summaries, subagents — must stay on the DeepSeek
/// provider. The per-turn model still travels with each prompt, so a later switch in the composer wins.
/// A configured endpoint and key are injected as provider options; both stay absent when unset, leaving
/// OpenCode's own sign-in and the provider's official address in charge.
pub fn config(model: Option<&str>, base_url: Option<&str>, api_key: Option<&str>) -> String {
    let model = normalize_model(model);
    let mut value = serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "model": model,
        "small_model": format!("{PROVIDER}/{DEFAULT_MODEL}"),
    });
    apply_provider(&mut value, base_url, api_key);
    value.to_string()
}

/// Add the configured endpoint and key to an OpenCode configuration document.
///
/// The one-shot title and translation paths inherit this too, so a key entered in the settings page
/// works for every model call the engine starts on the conversation's behalf. Existing provider
/// settings are preserved and only the DeepSeek entry is touched.
pub fn apply_provider(config: &mut Value, base_url: Option<&str>, api_key: Option<&str>) {
    let mut options = serde_json::Map::new();
    if let Some(url) = base_url.map(str::trim).filter(|url| !url.is_empty()) {
        options.insert("baseURL".into(), Value::String(url.to_string()));
    }
    if let Some(key) = api_key.map(str::trim).filter(|key| !key.is_empty()) {
        options.insert("apiKey".into(), Value::String(key.to_string()));
    }
    if options.is_empty() {
        return;
    }
    let Some(root) = config.as_object_mut() else { return; };
    let provider = root.entry("provider").or_insert_with(|| Value::Object(Default::default()));
    if !provider.is_object() {
        *provider = Value::Object(Default::default());
    }
    let entry = provider
        .as_object_mut()
        .expect("provider was just forced into an object")
        .entry(PROVIDER)
        .or_insert_with(|| Value::Object(Default::default()));
    if !entry.is_object() {
        *entry = Value::Object(Default::default());
    }
    let entry = entry.as_object_mut().expect("provider entry was just forced into an object");
    let existing = entry.entry("options").or_insert_with(|| Value::Object(Default::default()));
    if !existing.is_object() {
        *existing = Value::Object(Default::default());
    }
    let existing = existing.as_object_mut().expect("options were just forced into an object");
    for (name, value) in options {
        existing.insert(name, value);
    }
}

/// Map DeepSeek Harness's reasoning ladder onto OpenCode's model variants.
///
/// The old agent offered `off`, which OpenCode spells by omitting the variant. Unknown values pass
/// through so the provider can report them rather than this function silently choosing another level.
pub fn normalize_effort(effort: Option<&str>) -> Option<String> {
    let effort = effort.map(str::trim).filter(|effort| !effort.is_empty())?;
    match effort.to_ascii_lowercase().as_str() {
        "off" | "default" | "none" => None,
        "minimal" => Some("low".into()),
        other => Some(other.to_string()),
    }
}

/// The two routes OpenCode's DeepSeek provider offers, for a picker that must answer before a server
/// has been started or while OpenCode is not installed yet.
pub fn shipped() -> Vec<OpencodeModel> {
    [("deepseek-flash", "DeepSeek V4.1 Flash", vec!["low", "high", "max"]),
     ("deepseek-v4-pro", "DeepSeek V4 Pro", vec!["high", "max"])]
        .into_iter()
        .map(|(id, label, efforts)| OpencodeModel {
            id: model_value(id),
            label: label.to_string(),
            description: "DeepSeek".to_string(),
            effort_levels: efforts.into_iter().map(str::to_string).collect(),
            context_window: Some(1_048_576),
            curated: true,
            large_context: false,
        })
        .collect()
}

/// Keep only the models of the DeepSeek provider, in the order OpenCode reported them.
#[cfg(test)]
fn filter(models: Vec<OpencodeModel>) -> Vec<OpencodeModel> {
    models
        .into_iter()
        .filter(|model| model.id.strip_prefix(PROVIDER).and_then(|rest| rest.strip_prefix('/')).is_some())
        .collect()
}

/// The OpenCode selection a DeepSeek conversation should use.
///
/// Older conversations saved DeepSeek Harness's opaque `["deepseek-official","deepseek-flash"]` pair;
/// bare model names and the retired `deepseek-v4-flash` are accepted too. Anything naming another
/// provider falls back to [`DEFAULT_MODEL`], because this session type must never call another API.
pub fn normalize_model(model: Option<&str>) -> String {
    let raw = model.map(str::trim).filter(|raw| !raw.is_empty());
    let Some(raw) = raw else {
        return model_value(DEFAULT_MODEL);
    };
    if let Ok(parts) = serde_json::from_str::<Vec<String>>(raw) {
        if parts.len() == 2 && !parts[0].trim().is_empty() && !parts[1].trim().is_empty() {
            return route(&parts[0], &parts[1]);
        }
    }
    match raw.split_once('/') {
        Some((provider, model)) => route(provider, model),
        None => model_value(current_model(raw)),
    }
}

fn route(provider: &str, model: &str) -> String {
    if provider == PROVIDER || provider == "deepseek-official" {
        model_value(current_model(model))
    } else {
        model_value(DEFAULT_MODEL)
    }
}

fn current_model(model: &str) -> &str {
    match model.trim() {
        LEGACY_MODEL => DEFAULT_MODEL,
        other => other,
    }
}

fn model_value(model: &str) -> String {
    format!("{PROVIDER}/{model}")
}

/// OpenCode's provider report as a plain value, for tests.
#[cfg(test)]
fn from_provider(value: &Value) -> Vec<OpencodeModel> {
    filter(super::opencode_protocol::parse_models(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selections_are_normalized_to_the_deepseek_provider() {
        assert_eq!(normalize_model(None), "deepseek/deepseek-flash");
        assert_eq!(normalize_model(Some("")), "deepseek/deepseek-flash");
        assert_eq!(normalize_model(Some("deepseek-v4-pro")), "deepseek/deepseek-v4-pro");
        assert_eq!(normalize_model(Some("deepseek/deepseek-v4-pro")), "deepseek/deepseek-v4-pro");
        assert_eq!(normalize_model(Some("deepseek-official/deepseek-flash")), "deepseek/deepseek-flash");
        // Harness's opaque route pair.
        assert_eq!(normalize_model(Some(r#"["deepseek-official","deepseek-v4-pro"]"#)), "deepseek/deepseek-v4-pro");
        // The retired id moves to its successor.
        assert_eq!(normalize_model(Some(r#"["deepseek-official","deepseek-v4-flash"]"#)), "deepseek/deepseek-flash");
        assert_eq!(normalize_model(Some("deepseek-v4-flash")), "deepseek/deepseek-flash");
        // Another provider can never be selected in a DeepSeek conversation.
        assert_eq!(normalize_model(Some("opencode-go/deepseek-v4-pro")), "deepseek/deepseek-flash");
        assert_eq!(normalize_model(Some("anthropic/claude-opus-5")), "deepseek/deepseek-flash");
    }

    #[test]
    fn harness_effort_levels_map_to_opencode_variants() {
        assert_eq!(normalize_effort(None), None);
        assert_eq!(normalize_effort(Some("off")), None);
        assert_eq!(normalize_effort(Some("default")), None);
        assert_eq!(normalize_effort(Some("minimal")), Some("low".into()));
        assert_eq!(normalize_effort(Some("max")), Some("max".into()));
    }

    #[test]
    fn the_launch_configuration_pins_default_and_small_models() {
        let parsed: Value =
            serde_json::from_str(&config(Some("deepseek-official/deepseek-v4-pro"), None, None)).unwrap();
        assert_eq!(parsed["model"], "deepseek/deepseek-v4-pro");
        assert_eq!(parsed["small_model"], "deepseek/deepseek-flash");
        assert!(parsed.get("provider").is_none());
        let parsed: Value = serde_json::from_str(&config(None, None, None)).unwrap();
        assert_eq!(parsed["model"], "deepseek/deepseek-flash");
    }

    #[test]
    fn configured_endpoint_and_key_become_provider_options() {
        let parsed: Value = serde_json::from_str(&config(
            None,
            Some("https://proxy.example/v1"),
            Some("sk-test"),
        ))
        .unwrap();
        assert_eq!(parsed["provider"]["deepseek"]["options"]["baseURL"], "https://proxy.example/v1");
        assert_eq!(parsed["provider"]["deepseek"]["options"]["apiKey"], "sk-test");
        // Empty values leave the provider block absent rather than blanking OpenCode's own defaults.
        let parsed: Value = serde_json::from_str(&config(None, Some("  "), None)).unwrap();
        assert!(parsed.get("provider").is_none());
    }

    #[test]
    fn provider_merge_keeps_other_opencode_settings() {
        let mut config = json!({
            "permission": {"*": "deny"},
            "provider": {"deepseek": {"options": {"timeout": 5}}, "other": {"name": "Other"}},
        });
        apply_provider(&mut config, Some("https://proxy.example"), Some("sk-test"));
        assert_eq!(config["permission"]["*"], "deny");
        assert_eq!(config["provider"]["other"]["name"], "Other");
        assert_eq!(config["provider"]["deepseek"]["options"]["timeout"], 5);
        assert_eq!(config["provider"]["deepseek"]["options"]["baseURL"], "https://proxy.example");
        assert_eq!(config["provider"]["deepseek"]["options"]["apiKey"], "sk-test");
        // A malformed provider value is replaced rather than crashing the merge.
        let mut config = json!({"provider": "bad"});
        apply_provider(&mut config, Some("https://proxy.example"), None);
        assert_eq!(config["provider"]["deepseek"]["options"]["baseURL"], "https://proxy.example");
    }

    #[test]
    fn the_provider_report_keeps_only_deepseek_models() {
        let models = from_provider(&json!({
            "connected": ["deepseek", "opencode-go"],
            "default": {"deepseek": "deepseek-flash"},
            "all": [
                {"id": "deepseek", "name": "DeepSeek", "models": {
                    "deepseek-flash": {"name": "DeepSeek V4.1 Flash", "variants": {"low": {}, "high": {}, "max": {}}},
                    "deepseek-v4-pro": {"name": "DeepSeek V4 Pro", "variants": {"high": {}, "max": {}}}
                }},
                {"id": "opencode-go", "name": "OpenCode Go", "models": {
                    "deepseek-v4-pro": {"name": "DeepSeek V4 Pro"}
                }}
            ]
        }));
        assert_eq!(models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["deepseek/deepseek-flash", "deepseek/deepseek-v4-pro"]);
        assert_eq!(models[0].effort_levels, ["low", "high", "max"]);
    }
}
