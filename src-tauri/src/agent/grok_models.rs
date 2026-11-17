//! Grok's model catalogue with each model's reasoning levels, read from its own config file.
//!
//! Grok declares per-model reasoning ladders in `~/.grok/config.toml`:
//!
//! ```toml
//! [model."magpie/auto/deepseek-v4.1-flash"]
//! name = "DeepSeek V4.1 Flash"
//! reasoning_efforts = ["none", "low", "high", "max"]
//! ```
//!
//! Reading that file makes the composer's thinking chip work while the agent is not running. A live ACP
//! session reports the same ladders, but only after it has opened; the config is there immediately and
//! names the same models.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One model as Grok's config declares it.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigModel {
    pub label: String,
    pub effort_levels: Vec<String>,
}

/// Grok root: `GROK_HOME` when set, otherwise `~/.grok`.
fn grok_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("GROK_HOME") {
        return Some(PathBuf::from(home));
    }
    crate::host::home_dir().map(|home| home.join(".grok"))
}

/// Every model `config.toml` gives a reasoning ladder for, keyed by model id.
pub fn load() -> HashMap<String, ConfigModel> {
    grok_home()
        .map(|home| home.join("config.toml"))
        .and_then(|path| read(&path).ok())
        .unwrap_or_default()
}

/// Read the `[model."<id>"]` tables of one Grok config file. A malformed file yields no entries rather
/// than an error: the picker then falls back to what the running session reported.
pub fn read(path: &Path) -> Result<HashMap<String, ConfigModel>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    Ok(parse(&text))
}

/// Parse the config text. Split out so the shape can be checked without touching the user's home.
pub fn parse(text: &str) -> HashMap<String, ConfigModel> {
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else { return HashMap::new() };
    let Some(models) = doc.get("model").and_then(toml_edit::Item::as_table_like) else { return HashMap::new() };
    let mut out = HashMap::new();
    for (id, item) in models.iter() {
        let Some(table) = item.as_table_like() else { continue };
        let effort_levels: Vec<String> = table
            .get("reasoning_efforts")
            .and_then(toml_edit::Item::as_value)
            .and_then(toml_edit::Value::as_array)
            .map(|levels| levels.iter().filter_map(|level| level.as_str()).map(str::to_string).collect())
            .unwrap_or_default();
        let label = table
            .get("name")
            .and_then(toml_edit::Item::as_value)
            .and_then(toml_edit::Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(id)
            .to_string();
        if !effort_levels.is_empty() {
            out.insert(id.to_string(), ConfigModel { label, effort_levels });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_each_models_reasoning_ladder() {
        let config = r#"
[models]
default = "magpie/auto/deepseek-v4.1-flash"
default_reasoning_effort = "high"

[model."magpie/auto/deepseek-v4.1-flash"]
name = "DeepSeek V4.1 Flash"
reasoning_efforts = ["none", "low", "high", "max"]

[model."magpie/group/free"]
name = "free"

[model."magpie/auto/claude-opus-5-5"]
reasoning_efforts = ["low", "medium", "high", "xhigh", "max"]
"#;
        let models = parse(config);
        assert_eq!(models["magpie/auto/deepseek-v4.1-flash"].effort_levels, ["none", "low", "high", "max"]);
        assert_eq!(models["magpie/auto/deepseek-v4.1-flash"].label, "DeepSeek V4.1 Flash");
        // A model without a ladder is left out so the caller can fall back to the live session.
        assert!(!models.contains_key("magpie/group/free"));
        // A missing name falls back to the id.
        assert_eq!(models["magpie/auto/claude-opus-5-5"].label, "magpie/auto/claude-opus-5-5");
    }

    #[test]
    fn a_malformed_file_yields_no_models_instead_of_an_error() {
        assert!(parse("[model.\"x\"").is_empty());
        assert!(parse("").is_empty());
    }
}
