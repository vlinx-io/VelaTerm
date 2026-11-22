//! Persistent plugin registry, archive installation (`.zip` / `.vlxplugin`), and dev-mode directory links.

use super::manifest::{is_valid_plugin_id, PluginManifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

/// Origin of a registered plugin on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginSource {
    /// Copied into `<data_dir>/plugins/installed/<id>/` from an archive.
    Installed,
    /// Linked directly to a local developer directory (never deleted on uninstall).
    Dev,
}

/// One persisted entry in `<data_dir>/plugins/registry.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRecord {
    pub id: String,
    pub source: PluginSource,
    pub path: String,
    pub enabled: bool,
    pub installed_at: i64,
}

/// Persisted registry file stored under `<data_dir>/plugins/registry.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegistryFile {
    pub schema_version: u32,
    pub plugins: BTreeMap<String, PluginRecord>,
}

pub fn plugins_root_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("plugins")
}

pub fn installed_plugins_dir(data_dir: &Path) -> PathBuf {
    plugins_root_dir(data_dir).join("installed")
}

pub fn plugin_data_dir(data_dir: &Path, plugin_id: &str) -> PathBuf {
    data_dir.join("plugins-data").join(plugin_id)
}

fn registry_path(data_dir: &Path) -> PathBuf {
    plugins_root_dir(data_dir).join("registry.json")
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn load_registry(data_dir: &Path) -> Result<PluginRegistryFile, String> {
    let path = registry_path(data_dir);
    if !path.exists() {
        return Ok(PluginRegistryFile {
            schema_version: 1,
            plugins: BTreeMap::new(),
        });
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Failed reading plugin registry {}: {e}", path.display()))?;
    let reg: PluginRegistryFile = serde_json::from_str(&raw)
        .map_err(|e| format!("Invalid plugin registry {}: {e}", path.display()))?;
    Ok(reg)
}

pub fn save_registry(data_dir: &Path, reg: &PluginRegistryFile) -> Result<(), String> {
    let root = plugins_root_dir(data_dir);
    fs::create_dir_all(&root)
        .map_err(|e| format!("Failed creating plugin directory {}: {e}", root.display()))?;
    let path = registry_path(data_dir);
    let tmp = root.join(format!("registry.json.tmp.{}", uuid::Uuid::new_v4().simple()));
    let body = serde_json::to_string_pretty(reg).map_err(|e| e.to_string())?;
    fs::write(&tmp, body).map_err(|e| format!("Failed writing plugin registry: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Failed committing plugin registry: {e}"))?;
    Ok(())
}

/// Extract a `.zip` / `.vlxplugin` archive safely and install or upgrade the plugin under `installed/<id>`.
pub fn install_archive(data_dir: &Path, archive_path: &Path) -> Result<(PluginRecord, PluginManifest), String> {
    if !archive_path.is_file() {
        return Err(format!("Plugin archive not found: {}", archive_path.display()));
    }
    let staging = plugins_root_dir(data_dir)
        .join("staging")
        .join(uuid::Uuid::new_v4().simple().to_string());
    fs::create_dir_all(&staging)
        .map_err(|e| format!("Failed creating plugin staging directory: {e}"))?;

    let extract_res = extract_zip_safely(archive_path, &staging);
    if let Err(err) = extract_res {
        let _ = fs::remove_dir_all(&staging);
        return Err(err);
    }

    let plugin_src = match locate_plugin_root(&staging) {
        Ok(p) => p,
        Err(err) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(err);
        }
    };
    let manifest = match PluginManifest::load_from_dir(&plugin_src) {
        Ok(m) => m,
        Err(err) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(err);
        }
    };

    let installed_root = installed_plugins_dir(data_dir);
    fs::create_dir_all(&installed_root)
        .map_err(|e| format!("Failed creating installed plugins dir: {e}"))?;
    let dest = installed_root.join(&manifest.id);
    if dest.exists() {
        fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed replacing existing plugin {}: {e}", manifest.id))?;
    }
    // Move extracted plugin root into `installed/<id>`.
    if let Err(err) = fs::rename(&plugin_src, &dest) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("Failed installing plugin {}: {err}", manifest.id));
    }
    let _ = fs::remove_dir_all(&staging);

    // Ensure Unix executable permission on resolved backend binary when present.
    #[cfg(unix)]
    ensure_backend_executable(&dest, &manifest);

    let mut reg = load_registry(data_dir)?;
    let prev_enabled = reg.plugins.get(&manifest.id).map(|r| r.enabled).unwrap_or(true);
    let record = PluginRecord {
        id: manifest.id.clone(),
        source: PluginSource::Installed,
        path: dest.to_string_lossy().into_owned(),
        enabled: prev_enabled,
        installed_at: now_unix(),
    };
    reg.schema_version = 1;
    reg.plugins.insert(manifest.id.clone(), record.clone());
    save_registry(data_dir, &reg)?;
    Ok((record, manifest))
}

/// Register a local plugin directory directly for development (zero copies, instant reload).
pub fn link_dev_dir(data_dir: &Path, dir_path: &Path) -> Result<(PluginRecord, PluginManifest), String> {
    if !dir_path.is_dir() {
        return Err(format!("Plugin directory not found: {}", dir_path.display()));
    }
    let canonical = fs::canonicalize(dir_path).unwrap_or_else(|_| dir_path.to_path_buf());
    let manifest = PluginManifest::load_from_dir(&canonical)?;
    let mut reg = load_registry(data_dir)?;
    let record = PluginRecord {
        id: manifest.id.clone(),
        source: PluginSource::Dev,
        path: canonical.to_string_lossy().into_owned(),
        enabled: true,
        installed_at: now_unix(),
    };
    reg.schema_version = 1;
    reg.plugins.insert(manifest.id.clone(), record.clone());
    save_registry(data_dir, &reg)?;
    Ok((record, manifest))
}

/// Toggle a registered plugin's enabled state in `registry.json`.
pub fn set_enabled(data_dir: &Path, plugin_id: &str, enabled: bool) -> Result<PluginRecord, String> {
    if !is_valid_plugin_id(plugin_id) {
        return Err(format!("Invalid plugin id: '{plugin_id}'"));
    }
    let mut reg = load_registry(data_dir)?;
    let rec = reg
        .plugins
        .get_mut(plugin_id)
        .ok_or_else(|| format!("Plugin '{plugin_id}' is not installed."))?;
    rec.enabled = enabled;
    let updated = rec.clone();
    save_registry(data_dir, &reg)?;
    Ok(updated)
}

/// Remove a plugin from `registry.json`, delete its installed bundle (never a Dev link), and optionally purge its data.
pub fn uninstall(data_dir: &Path, plugin_id: &str, remove_data: bool) -> Result<PluginRecord, String> {
    if !is_valid_plugin_id(plugin_id) {
        return Err(format!("Invalid plugin id: '{plugin_id}'"));
    }
    let mut reg = load_registry(data_dir)?;
    let removed = reg
        .plugins
        .remove(plugin_id)
        .ok_or_else(|| format!("Plugin '{plugin_id}' is not installed."))?;
    save_registry(data_dir, &reg)?;

    if removed.source == PluginSource::Installed {
        let installed = installed_plugins_dir(data_dir).join(plugin_id);
        if installed.exists() {
            let _ = fs::remove_dir_all(&installed);
        }
    }
    if remove_data {
        let pdata = plugin_data_dir(data_dir, plugin_id);
        if pdata.exists() {
            let _ = fs::remove_dir_all(&pdata);
        }
    }
    Ok(removed)
}

fn locate_plugin_root(staging: &Path) -> Result<PathBuf, String> {
    if staging.join("manifest.json").is_file() {
        return Ok(staging.to_path_buf());
    }
    let mut subdirs = Vec::new();
    for entry in fs::read_dir(staging).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "__MACOSX" || name.starts_with('.') {
            continue;
        }
        if entry.path().is_dir() {
            subdirs.push(entry.path());
        }
    }
    if subdirs.len() == 1 && subdirs[0].join("manifest.json").is_file() {
        return Ok(subdirs.remove(0));
    }
    Err("Plugin archive must contain manifest.json at its root (or inside a single top-level directory).".to_string())
}

fn extract_zip_safely(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    const MAX_ENTRIES: usize = 2048;
    const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

    let file = File::open(archive_path)
        .map_err(|e| format!("Failed opening archive {}: {e}", archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid zip plugin archive: {e}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err("Plugin archive contains too many entries.".to_string());
    }
    let mut total_written: u64 = 0;

    for idx in 0..archive.len() {
        let mut entry = archive.by_index(idx).map_err(|e| e.to_string())?;
        let Some(enclosed) = entry.enclosed_name().map(PathBuf::from) else {
            return Err(format!("Unsafe path in plugin archive: {}", entry.name()));
        };
        if enclosed.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
            return Err(format!("Unsafe path in plugin archive: {}", entry.name()));
        }
        let out_path = dest_dir.join(&enclosed);
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out_file = File::create(&out_path).map_err(|e| e.to_string())?;
        let copied = io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
        total_written = total_written.saturating_add(copied);
        if total_written > MAX_TOTAL_BYTES {
            return Err("Plugin archive exceeds maximum unpacked size (256 MB).".to_string());
        }
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let perm = mode & 0o777;
            if perm != 0 {
                let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(perm));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_backend_executable(root: &Path, manifest: &PluginManifest) {
    use std::os::unix::fs::PermissionsExt;
    if let Some(backend) = &manifest.backend {
        if let Some(rel) = backend.entry.resolve_current() {
            let path = root.join(rel);
            if path.is_file() {
                if let Ok(meta) = fs::metadata(&path) {
                    let mode = meta.permissions().mode();
                    if mode & 0o111 == 0 {
                        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(mode | 0o755));
                    }
                }
            }
        }
    }
}
