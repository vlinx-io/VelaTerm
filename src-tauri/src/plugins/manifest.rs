//! Plugin manifest schema (v1), validation, and cross-platform entry resolution.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// Reserved plugin identifiers that cannot be claimed by external plugins.
const RESERVED_PLUGIN_IDS: &[&str] = &["core", "host", "system", "all", "builtin", "plugins"];

/// Built-in CLI command names that plugins must not shadow.
const BUILTIN_CLI_NAMES: &[&str] = &[
    "vela",
    "vspawn",
    "vspawn-tree",
    "vopen",
    "vartifact",
    "vrefer",
    "vsearch",
    "vstat",
    "vself",
    "vkb",
    "vflow",
    "vtell",
    "vrun",
    "vapp",
];

/// Reserved URI schemes that plugins must not claim as virtual PTY providers.
const RESERVED_PTY_SCHEMES: &[&str] = &["file", "http", "https", "ws", "wss", "ssh", "tauri"];

/// Platform-specific or single backend executable path specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BackendEntrySpec {
    Single(String),
    ByPlatform(BTreeMap<String, String>),
}

impl BackendEntrySpec {
    /// Resolve the relative executable path for the current OS and CPU architecture.
    pub fn resolve_current(&self) -> Option<&str> {
        match self {
            BackendEntrySpec::Single(p) => Some(p.as_str()),
            BackendEntrySpec::ByPlatform(map) => {
                let key = current_platform_key();
                map.get(key)
                    .or_else(|| map.get("default"))
                    .map(String::as_str)
            }
        }
    }
}

/// Backend sidecar declaration inside `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendManifest {
    pub entry: BackendEntrySpec,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub secret_commands: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub pty_schemes: Vec<String>,
}

/// Sidebar activity-rail contribution metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiRailManifest {
    pub label_key: String,
    #[serde(default = "default_route")]
    pub default_route: String,
    #[serde(default)]
    pub icon_svg: Option<String>,
    #[serde(default)]
    pub separator_before: bool,
}

fn default_route() -> String {
    "overview".to_string()
}

/// CenterPane overlay/tab contribution metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiCenterPageManifest {
    #[serde(default)]
    pub hides_right_panel: bool,
    pub tab_title_key: String,
}

/// Frontend UI bundle and contribution points declared in `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiManifest {
    pub entry: String,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub locales_dir: Option<String>,
    #[serde(default)]
    pub rail: Option<UiRailManifest>,
    #[serde(default)]
    pub center_page: Option<UiCenterPageManifest>,
    #[serde(default)]
    pub diagnostic_redacted_commands: Vec<String>,
}

/// CLI command shim declared by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliContribution {
    pub name: String,
}

/// Agent skill markdown file contributed by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillContribution {
    pub name: String,
    pub path: String,
}

/// Top-level `manifest.json` structure for external VelaTerm plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_api_version")]
    pub api_version: String,
    #[serde(default)]
    pub backend: Option<BackendManifest>,
    #[serde(default)]
    pub ui: Option<UiManifest>,
    #[serde(default)]
    pub cli: Vec<CliContribution>,
    #[serde(default)]
    pub skills: Vec<SkillContribution>,
}

fn default_api_version() -> String {
    "1".to_string()
}

/// Platform key matching `BackendEntrySpec::ByPlatform` entries.
pub fn current_platform_key() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    { "macos-aarch64" }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    { "macos-x86_64" }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    { "linux-x86_64" }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    { "linux-aarch64" }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    { "windows-x86_64" }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    { "windows-aarch64" }
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
    )))]
    { "default" }
}

/// Validate that a plugin ID is safe for filesystem paths, URLs, and event namespaces.
pub fn is_valid_plugin_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 64 || RESERVED_PLUGIN_IDS.contains(&id) {
        return false;
    }
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Validate a relative path inside a plugin directory, rejecting traversal or absolute paths.
pub fn validate_rel_path(rel: &str) -> Result<PathBuf, String> {
    if rel.is_empty() || rel.contains('\0') || rel.starts_with('/') || rel.starts_with('\\') {
        return Err(format!("Invalid plugin relative path: {rel}"));
    }
    let path = Path::new(rel);
    let mut clean = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("Unsafe plugin path traversal: {rel}"));
            }
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(format!("Empty plugin relative path: {rel}"));
    }
    Ok(clean)
}

fn is_valid_cli_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 48 || BUILTIN_CLI_NAMES.contains(&name) {
        return false;
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_valid_pty_scheme(scheme: &str) -> bool {
    if scheme.is_empty() || scheme.len() > 32 || RESERVED_PTY_SCHEMES.contains(&scheme) {
        return false;
    }
    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))
}

impl PluginManifest {
    /// Load and validate `<plugin_dir>/manifest.json`.
    pub fn load_from_dir(dir: &Path) -> Result<Self, String> {
        let path = dir.join("manifest.json");
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed reading {}: {e}", path.display()))?;
        let manifest: Self = serde_json::from_str(&raw)
            .map_err(|e| format!("Invalid plugin manifest {}: {e}", path.display()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Check schema version, identifiers, paths, and security invariants.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "Unsupported plugin manifest schemaVersion: {} (expected 1)",
                self.schema_version
            ));
        }
        if !is_valid_plugin_id(&self.id) {
            return Err(format!("Invalid plugin id: '{}'", self.id));
        }
        if self.name.trim().is_empty() {
            return Err("Plugin manifest name must not be empty".to_string());
        }
        if self.version.trim().is_empty() {
            return Err("Plugin manifest version must not be empty".to_string());
        }
        if let Some(backend) = &self.backend {
            match &backend.entry {
                BackendEntrySpec::Single(p) => {
                    validate_rel_path(p)?;
                }
                BackendEntrySpec::ByPlatform(map) => {
                    if map.is_empty() {
                        return Err("Backend entry platform map must not be empty".to_string());
                    }
                    for p in map.values() {
                        validate_rel_path(p)?;
                    }
                }
            }
            for scheme in &backend.pty_schemes {
                if !is_valid_pty_scheme(scheme) {
                    return Err(format!("Invalid or reserved ptyScheme: '{scheme}'"));
                }
            }
        }
        if let Some(ui) = &self.ui {
            validate_rel_path(&ui.entry)?;
            if let Some(style) = &ui.style {
                validate_rel_path(style)?;
            }
            if let Some(locales_dir) = &ui.locales_dir {
                validate_rel_path(locales_dir)?;
            }
        }
        for cli in &self.cli {
            if !is_valid_cli_name(&cli.name) {
                return Err(format!("Invalid or reserved CLI command name: '{}'", cli.name));
            }
        }
        for skill in &self.skills {
            if !is_valid_cli_name(&skill.name) {
                return Err(format!("Invalid or reserved skill name: '{}'", skill.name));
            }
            validate_rel_path(&skill.path)?;
        }
        Ok(())
    }

    /// Check whether a command name matches any declared backend command pattern (exact or `prefix*`).
    pub fn matches_command(&self, cmd: &str) -> bool {
        let Some(backend) = &self.backend else {
            return false;
        };
        backend.commands.iter().any(|pat| {
            if let Some(prefix) = pat.strip_suffix('*') {
                cmd.starts_with(prefix)
            } else {
                cmd == pat
            }
        })
    }

    /// Check whether a command is declared local-only and blocked for remote browser clients.
    pub fn is_secret_command(&self, cmd: &str) -> bool {
        self.backend
            .as_ref()
            .is_some_and(|b| b.secret_commands.iter().any(|s| s == cmd))
    }

    /// Check whether the plugin declares ownership of a virtual PTY URI scheme.
    pub fn handles_pty_scheme(&self, scheme: &str) -> bool {
        self.backend
            .as_ref()
            .is_some_and(|b| b.pty_schemes.iter().any(|s| s == scheme))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_ids_and_paths() {
        assert!(is_valid_plugin_id("colab"));
        assert!(is_valid_plugin_id("my-plugin_2"));
        assert!(!is_valid_plugin_id(""));
        assert!(!is_valid_plugin_id("Core"));
        assert!(!is_valid_plugin_id("core"));
        assert!(!is_valid_plugin_id("../evil"));

        assert!(validate_rel_path("ui/entry.js").is_ok());
        assert!(validate_rel_path("../secret").is_err());
        assert!(validate_rel_path("/etc/passwd").is_err());
        assert!(validate_rel_path("ui/../bin/a").is_err());
    }

    #[test]
    fn matches_command_patterns_and_secrets() {
        let raw = serde_json::json!({
            "schemaVersion": 1,
            "id": "colab",
            "name": "Google Colab",
            "version": "0.1.0",
            "backend": {
                "entry": "bin/vlx-plugin-colab",
                "commands": ["colab_*"],
                "secretCommands": ["colab_login_start"],
                "ptySchemes": ["colab"]
            }
        });
        let m: PluginManifest = serde_json::from_value(raw).unwrap();
        assert!(m.validate().is_ok());
        assert!(m.matches_command("colab_snapshot"));
        assert!(!m.matches_command("other_cmd"));
        assert!(m.is_secret_command("colab_login_start"));
        assert!(!m.is_secret_command("colab_snapshot"));
        assert!(m.handles_pty_scheme("colab"));
    }
}
