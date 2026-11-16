//! App settings. Values are applied in the client through the same setters as the settings page.

use serde_json::{json, Value};
use super::{Check, one_of, text, AppCtx, CallOrigin, Domain, Result};

pub(super) const DOMAIN: Domain = Domain {
    id: "settings",
    guide: GUIDE,
    context,
    check: Check::Each(validate),
    backend: &[],
    run,
};

const GUIDE: &str = "App settings.
Actions:
- {\"kind\":\"openSettings\",\"page\":\"one of pages\"}
- {\"kind\":\"set\",\"setting\":\"theme|language|accent|density|sound|notifications\",\"value\":\"one of the listed values; true or false for sound and notifications\"}
- {\"kind\":\"setFontSize\",\"target\":\"terminal|interface|conversation\",\"size\":13}
- {\"kind\":\"setFontFamily\",\"target\":\"terminal|interface|conversation\",\"family\":\"font family name, or empty for the default\"}";

const PAGES: &[&str] = &["general", "appearance", "terminal", "conversation", "translation", "behavior", "advanced", "agents", "shortcuts"];
const TARGETS: &[&str] = &["terminal", "interface", "conversation"];
const FAMILY_TARGETS: &[&str] = &["terminal", "interface", "conversation"];
const CHOICES: &[(&str, &[&str])] = &[
    ("theme", &["system", "dark", "light"]),
    ("language", &["auto", "en", "zh-CN", "zh-TW", "ja", "ko", "fr", "de", "es", "pt-BR", "ru", "vi"]),
    ("accent", &["auto", "green", "blue", "amber", "violet"]),
    ("density", &["compact", "regular", "comfy"]),
];
const SWITCHES: &[&str] = &["sound", "notifications"];

fn context(_app: &AppCtx, _origin: CallOrigin) -> Result<Value> {
    let choices = CHOICES.iter().map(|(name, values)| (name.to_string(), json!(values))).collect::<serde_json::Map<_, _>>();
    Ok(json!({"pages": PAGES, "values": choices, "fontSize": {"min": 8, "max": 40}}))
}

fn validate(_app: &AppCtx, raw: &Value, _context: &Value, _origin: CallOrigin) -> std::result::Result<Value, &'static str> {
    match raw["kind"].as_str().unwrap_or_default() {
        "openSettings" => Ok(json!({"page": one_of(&raw["page"], PAGES, Some("general"))?})),
        "set" => {
            let setting = raw["setting"].as_str().unwrap_or_default();
            if SWITCHES.contains(&setting) {
                let on = match &raw["value"] { Value::Bool(b) => *b, Value::String(s) if s == "true" || s == "on" => true, Value::String(s) if s == "false" || s == "off" => false, _ => return Err("invalid") };
                return Ok(json!({"setting": setting, "value": on}));
            }
            let (name, values) = CHOICES.iter().find(|(name, _)| *name == setting).ok_or("unsupported")?;
            Ok(json!({"setting": name, "value": one_of(&raw["value"], values, None)?}))
        }
        "setFontSize" => {
            let size = raw["size"].as_f64().or_else(|| raw["size"].as_str().and_then(|s| s.trim().parse().ok())).ok_or("invalid")?;
            if !(8.0..=40.0).contains(&size) { return Err("invalid"); }
            Ok(json!({"target": one_of(&raw["target"], TARGETS, Some("terminal"))?, "size": size.round() as u64}))
        }
        "setFontFamily" => Ok(json!({"target": one_of(&raw["target"], FAMILY_TARGETS, Some("terminal"))?, "family": text(&raw["family"], 200)?})),
        _ => Err("unsupported"),
    }
}

fn run(_app: &AppCtx, _action: &Value, _args: &Value, _origin: CallOrigin) -> Result<Value> { Err("Settings are applied in the app window".into()) }
