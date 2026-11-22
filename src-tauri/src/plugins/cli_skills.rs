//! Managed CLI shims (`<data_dir>/bin/<cmd>`) and Agent Skills for enabled plugins.

use super::manifest::PluginManifest;
use super::registry::plugins_root_dir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Tracks previously written plugin shims and skill names so disabling or uninstalling a plugin
/// removes only its managed artifacts without touching built-in or user-owned files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedArtifacts {
    shims: BTreeSet<String>,
    skills: BTreeSet<String>,
}

fn state_path(data_dir: &Path) -> PathBuf {
    plugins_root_dir(data_dir).join("managed_artifacts.json")
}

fn load_state(data_dir: &Path) -> ManagedArtifacts {
    let path = state_path(data_dir);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(data_dir: &Path, state: &ManagedArtifacts) {
    let root = plugins_root_dir(data_dir);
    let _ = fs::create_dir_all(&root);
    if let Ok(raw) = serde_json::to_string_pretty(state) {
        let _ = fs::write(state_path(data_dir), raw);
    }
}

/// Synchronize `<data_dir>/bin/` shims and user agent skills to match the currently enabled plugins.
pub fn sync_artifacts(data_dir: &Path, enabled: &[(&Path, &PluginManifest)]) {
    let prev = load_state(data_dir);
    let mut next = ManagedArtifacts::default();

    let bin = crate::agent::spawn_cli::bin_dir(data_dir);
    let _ = fs::create_dir_all(&bin);

    // Write shims for enabled plugins.
    for (_, manifest) in enabled {
        for cli in &manifest.cli {
            let subcmd = format!("--plugin-cli {} {}", manifest.id, cli.name);
            if write_plugin_shim(&bin, &cli.name, &subcmd).is_ok() {
                next.shims.insert(cli.name.clone());
            }
        }
    }

    // Remove stale shims from disabled/uninstalled plugins.
    for old_shim in prev.shims.difference(&next.shims) {
        remove_plugin_shim(&bin, old_shim);
    }

    // Install or remove plugin agent skills only when the user has enabled VelaTerm agent skills.
    let skills_active = crate::agent::spawn_cli::skills_installed();
    let skill_dirs = resolve_skill_dirs();

    if skills_active {
        if let Some((claude_dir, agents_dir)) = &skill_dirs {
            for (root, manifest) in enabled {
                for skill in &manifest.skills {
                    let src = root.join(&skill.path);
                    let Ok(content) = fs::read_to_string(&src) else {
                        continue;
                    };
                    if write_skill_pair(claude_dir, agents_dir, &skill.name, &content).is_ok() {
                        next.skills.insert(skill.name.clone());
                    }
                }
            }
        }
    }

    // Remove skills no longer contributed (or when skills installation is turned off).
    if let Some((claude_dir, agents_dir)) = &skill_dirs {
        for old_skill in prev.skills.difference(&next.skills) {
            let _ = fs::remove_dir_all(claude_dir.join(old_skill));
            let _ = fs::remove_dir_all(agents_dir.join(old_skill));
        }
    }

    save_state(data_dir, &next);
}

/// Remove every plugin-managed skill (called when the user uninstalls VelaTerm agent skills).
pub fn remove_all_plugin_skills(data_dir: &Path) {
    let mut state = load_state(data_dir);
    if let Some((claude_dir, agents_dir)) = resolve_skill_dirs() {
        for skill in &state.skills {
            let _ = fs::remove_dir_all(claude_dir.join(skill));
            let _ = fs::remove_dir_all(agents_dir.join(skill));
        }
    }
    state.skills.clear();
    save_state(data_dir, &state);
}

fn write_plugin_shim(bin: &Path, name: &str, subcmd: &str) -> std::io::Result<()> {
    let sh_shim = format!("#!/bin/sh\nexec \"$VLX_EXE\" {subcmd} \"$@\"\n");
    #[cfg(windows)]
    {
        fs::write(
            bin.join(format!("{name}.cmd")),
            format!("@\"%VLX_EXE%\" {subcmd} %*\r\n"),
        )?;
        fs::write(bin.join(name), sh_shim)?;
    }
    #[cfg(unix)]
    {
        let path = bin.join(name);
        fs::write(&path, sh_shim)?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn remove_plugin_shim(bin: &Path, name: &str) {
    let _ = fs::remove_file(bin.join(name));
    let _ = fs::remove_file(bin.join(format!("{name}.cmd")));
}

fn resolve_skill_dirs() -> Option<(PathBuf, PathBuf)> {
    let home = crate::host::home_dir()?;
    Some((
        home.join(".claude").join("skills"),
        home.join(".agents").join("skills"),
    ))
}

fn agents_skill_content(content: &str) -> String {
    let mut rendered = content
        .lines()
        .filter(|line| {
            !line.starts_with("argument-hint:")
                && !line.starts_with("disable-model-invocation:")
                && !line.starts_with("allowed-tools:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    rendered.push('\n');
    rendered
}

fn write_skill_pair(
    claude_dir: &Path,
    agents_dir: &Path,
    name: &str,
    content: &str,
) -> std::io::Result<()> {
    let c_dest = claude_dir.join(name);
    fs::create_dir_all(&c_dest)?;
    fs::write(c_dest.join("SKILL.md"), content)?;

    let a_dest = agents_dir.join(name);
    fs::create_dir_all(&a_dest)?;
    fs::write(a_dest.join("SKILL.md"), agents_skill_content(content))?;
    Ok(())
}
