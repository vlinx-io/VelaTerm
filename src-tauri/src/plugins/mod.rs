//! Out-of-process dynamic plugin host: discovery, lifecycle, RPC forwarding, asset reading,
//! virtual PTY provider lookup, and CLI/skill artifact management.

pub mod cli_bridge;
pub mod cli_skills;
pub mod manifest;
pub mod process;
pub mod registry;

use crate::host::AppCtx;
use crate::web::dispatch::CallOrigin;
use manifest::{is_valid_plugin_id, validate_rel_path, PluginManifest};
use process::PluginProcess;
use registry::{plugin_data_dir, PluginRecord, PluginSource};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Unified event channel carrying all plugin-emitted events across Tauri IPC and remote WebSocket.
pub const PLUGIN_EVENT: &str = "plugin://event";

/// Broadcast whenever the installed plugin catalog or runtime status changes.
pub const PLUGINS_CHANGED: &str = "plugins://changed";

/// Runtime switch for the out-of-process plugin host. Off by default until the plugin UI ships;
/// set `VLX_ENABLE_PLUGINS=1` to activate plugin loading and management commands.
pub fn host_enabled() -> bool {
    std::env::var("VLX_ENABLE_PLUGINS")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Runtime state of a registered plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginRuntimeStatus {
    Disabled,
    Starting,
    Running,
    Stopped,
    Error,
}

/// Sanitized plugin entry returned to the frontend PluginHost and Settings panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: PluginSource,
    pub path: String,
    pub enabled: bool,
    pub status: PluginRuntimeStatus,
    pub last_error: Option<String>,
    pub revision: u64,
    pub has_backend: bool,
    pub ui: Option<manifest::UiManifest>,
    pub commands: Vec<String>,
    pub secret_commands: Vec<String>,
    pub pty_schemes: Vec<String>,
    pub cli: Vec<String>,
    pub skills: Vec<String>,
    pub installed_at: i64,
}

/// Text asset returned by `plugin_read_asset` for dynamic ESM/CSS/locale loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAsset {
    pub content: String,
    pub mime_type: String,
    pub revision: u64,
}

struct LoadedPlugin {
    record: PluginRecord,
    manifest: Option<PluginManifest>,
    process: Option<Arc<PluginProcess>>,
    status: PluginRuntimeStatus,
    last_error: Option<String>,
    revision: u64,
}

struct ManagerInner {
    plugins: BTreeMap<String, LoadedPlugin>,
    next_rev: u64,
}

/// Thread-safe plugin host managed by `AppCtx` (both Tauri GUI and headless `--serve`).
pub struct PluginManager {
    inner: Arc<Mutex<ManagerInner>>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ManagerInner {
                plugins: BTreeMap::new(),
                next_rev: 1,
            })),
        }
    }

    /// Load `registry.json`, synchronize shims/skills, and spawn enabled plugin sidecars asynchronously.
    pub fn start(&self, app: AppCtx) {
        if !host_enabled() {
            return;
        }
        let Ok(data_dir) = app.data_dir() else {
            return;
        };
        let reg = match registry::load_registry(&data_dir) {
            Ok(r) => r,
            Err(err) => {
                crate::diagnostics::record("WARN", "plugins_registry_load_failed", json!({ "error": err }));
                return;
            }
        };

        let mut to_start: Vec<(String, PathBuf, PluginManifest)> = Vec::new();
        {
            let mut inner = self.inner.lock().unwrap();
            inner.plugins.clear();
            for (id, rec) in reg.plugins {
                let rev = next_revision(&mut inner);
                let root = PathBuf::from(&rec.path);
                match PluginManifest::load_from_dir(&root) {
                    Ok(manifest) => {
                        let has_backend = manifest.backend.is_some();
                        let status = if !rec.enabled {
                            PluginRuntimeStatus::Disabled
                        } else if has_backend {
                            PluginRuntimeStatus::Starting
                        } else {
                            PluginRuntimeStatus::Running
                        };
                        if rec.enabled && has_backend {
                            to_start.push((id.clone(), root, manifest.clone()));
                        }
                        inner.plugins.insert(
                            id,
                            LoadedPlugin {
                                record: rec,
                                manifest: Some(manifest),
                                process: None,
                                status,
                                last_error: None,
                                revision: rev,
                            },
                        );
                    }
                    Err(err) => {
                        inner.plugins.insert(
                            id,
                            LoadedPlugin {
                                record: rec,
                                manifest: None,
                                process: None,
                                status: PluginRuntimeStatus::Error,
                                last_error: Some(err),
                                revision: rev,
                            },
                        );
                    }
                }
            }
        }

        self.sync_cli_and_skills(&data_dir);

        for (id, root, manifest) in to_start {
            let app_clone = app.clone();
            let inner_ref = Arc::clone(&self.inner);
            let pdata = plugin_data_dir(&data_dir, &id);
            let _ = std::thread::Builder::new()
                .name(format!("plugin-boot-{id}"))
                .spawn(move || {
                    spawn_into_state(app_clone, inner_ref, id, root, pdata, manifest);
                });
        }
    }

    /// Re-sync plugin CLI shims and agent skills (called after toggling agent skill settings).
    pub fn sync_artifacts_for(&self, data_dir: &Path) {
        self.sync_cli_and_skills(data_dir);
    }

    /// Return all registered plugins and their current runtime status.
    pub fn list(&self) -> Vec<PluginSummary> {
        let inner = self.inner.lock().unwrap();
        inner.plugins.values().map(summarize).collect()
    }

    /// Install a plugin from a `.zip` or `.vlxplugin` archive and start it immediately if enabled.
    pub fn install_archive(&self, app: &AppCtx, archive_path: &Path) -> Result<PluginSummary, String> {
        let data_dir = app.data_dir()?;
        let (record, manifest) = registry::install_archive(&data_dir, archive_path)?;
        self.activate_entry(app, record, manifest)
    }

    /// Link a local developer plugin directory and start it immediately.
    pub fn link_dev(&self, app: &AppCtx, dir_path: &Path) -> Result<PluginSummary, String> {
        let data_dir = app.data_dir()?;
        let (record, manifest) = registry::link_dev_dir(&data_dir, dir_path)?;
        self.activate_entry(app, record, manifest)
    }

    /// Enable or disable an installed plugin, starting or shutting down its sidecar immediately.
    pub fn set_enabled(&self, app: &AppCtx, plugin_id: &str, enabled: bool) -> Result<PluginSummary, String> {
        let data_dir = app.data_dir()?;
        let record = registry::set_enabled(&data_dir, plugin_id, enabled)?;
        let prev_proc = {
            let mut inner = self.inner.lock().unwrap();
            inner
                .plugins
                .get_mut(plugin_id)
                .and_then(|p| p.process.take())
        };
        if let Some(proc_ref) = prev_proc {
            proc_ref.shutdown();
        }

        let root = PathBuf::from(&record.path);
        let manifest = PluginManifest::load_from_dir(&root)?;
        self.activate_entry(app, record, manifest)
    }

    /// Re-read `manifest.json`, restart the backend sidecar, and bump the UI asset revision.
    pub fn reload(&self, app: &AppCtx, plugin_id: &str) -> Result<PluginSummary, String> {
        if !is_valid_plugin_id(plugin_id) {
            return Err(format!("Invalid plugin id: '{plugin_id}'"));
        }
        let data_dir = app.data_dir()?;
        let reg = registry::load_registry(&data_dir)?;
        let record = reg
            .plugins
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| format!("Plugin '{plugin_id}' is not installed."))?;
        let prev_proc = {
            let mut inner = self.inner.lock().unwrap();
            inner
                .plugins
                .get_mut(plugin_id)
                .and_then(|p| p.process.take())
        };
        if let Some(proc_ref) = prev_proc {
            proc_ref.shutdown();
        }
        let root = PathBuf::from(&record.path);
        let manifest = PluginManifest::load_from_dir(&root)?;
        self.activate_entry(app, record, manifest)
    }

    /// Stop and remove an installed plugin (and optionally delete its isolated data directory).
    pub fn uninstall(&self, app: &AppCtx, plugin_id: &str, remove_data: bool) -> Result<(), String> {
        let prev_proc = {
            let mut inner = self.inner.lock().unwrap();
            inner
                .plugins
                .remove(plugin_id)
                .and_then(|p| p.process)
        };
        if let Some(proc_ref) = prev_proc {
            proc_ref.shutdown();
        }
        let data_dir = app.data_dir()?;
        registry::uninstall(&data_dir, plugin_id, remove_data)?;
        self.sync_cli_and_skills(&data_dir);
        app.emit(PLUGINS_CHANGED, ());
        Ok(())
    }

    /// Read a static UI or locale text asset from an enabled plugin directory (path-traversal safe).
    pub fn read_asset(&self, plugin_id: &str, rel_path: &str) -> Result<PluginAsset, String> {
        if !is_valid_plugin_id(plugin_id) {
            return Err(format!("Invalid plugin id: '{plugin_id}'"));
        }
        let clean = validate_rel_path(rel_path)?;
        let allowed_top = clean
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .is_some_and(|top| top == "ui" || top == "locales");
        if !allowed_top {
            return Err("Plugin asset reads are restricted to ui/ and locales/.".to_string());
        }

        let (root, revision) = {
            let inner = self.inner.lock().unwrap();
            let entry = inner
                .plugins
                .get(plugin_id)
                .ok_or_else(|| format!("Plugin '{plugin_id}' not found."))?;
            (PathBuf::from(&entry.record.path), entry.revision)
        };
        let full = root.join(&clean);
        let canonical_root = std::fs::canonicalize(&root).map_err(|e| e.to_string())?;
        let canonical_full = std::fs::canonicalize(&full)
            .map_err(|_| format!("Plugin asset not found: {rel_path}"))?;
        if !canonical_full.starts_with(&canonical_root) {
            return Err("Plugin asset escapes plugin root.".to_string());
        }
        let meta = std::fs::metadata(&canonical_full).map_err(|e| e.to_string())?;
        if meta.len() > 10 * 1024 * 1024 {
            return Err("Plugin asset exceeds 10 MB limit.".to_string());
        }
        let content = std::fs::read_to_string(&canonical_full)
            .map_err(|e| format!("Failed reading plugin asset {rel_path}: {e}"))?;
        let mime_type = match canonical_full.extension().and_then(|e| e.to_str()) {
            Some("js" | "mjs") => "text/javascript; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            Some("json") => "application/json; charset=utf-8",
            Some("svg") => "image/svg+xml; charset=utf-8",
            _ => "text/plain; charset=utf-8",
        }
        .to_string();
        Ok(PluginAsset {
            content,
            mime_type,
            revision,
        })
    }

    /// Invoke a backend RPC command on a specific plugin, enforcing remote secret restrictions.
    pub fn invoke_plugin(
        &self,
        origin: CallOrigin,
        plugin_id: &str,
        cmd: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let proc_ref = {
            let inner = self.inner.lock().unwrap();
            let entry = inner
                .plugins
                .get(plugin_id)
                .ok_or_else(|| format!("Plugin '{plugin_id}' is not installed."))?;
            if !entry.record.enabled {
                return Err(format!("Plugin '{plugin_id}' is disabled."));
            }
            if origin == CallOrigin::Remote
                && entry
                    .manifest
                    .as_ref()
                    .is_some_and(|m| m.is_secret_command(cmd))
            {
                return Err(format!(
                    "Command '{cmd}' on plugin '{plugin_id}' is not allowed from remote clients."
                ));
            }
            entry.process.clone().ok_or_else(|| {
                entry
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("Plugin '{plugin_id}' backend is not running."))
            })?
        };
        proc_ref.call(
            "rpc.call",
            json!({
                "command": cmd,
                "args": args,
                "origin": match origin {
                    CallOrigin::Local => "local",
                    CallOrigin::Remote => "remote",
                },
            }),
            Duration::from_secs(120),
        )
    }

    /// Execute a CLI command declared by a plugin via the loopback hook server.
    pub fn exec_cli(
        &self,
        plugin_id: &str,
        cli: &str,
        args: &[String],
        cwd: Option<&str>,
    ) -> Result<Value, String> {
        let proc_ref = {
            let inner = self.inner.lock().unwrap();
            let entry = inner
                .plugins
                .get(plugin_id)
                .ok_or_else(|| format!("Plugin '{plugin_id}' is not installed."))?;
            if !entry.record.enabled {
                return Err(format!("Plugin '{plugin_id}' is disabled."));
            }
            entry
                .process
                .clone()
                .ok_or_else(|| format!("Plugin '{plugin_id}' backend is not running."))?
        };
        proc_ref.call(
            "cli.exec",
            json!({
                "cli": cli,
                "args": args,
                "cwd": cwd,
            }),
            Duration::from_secs(600),
        )
    }

    /// Find the running plugin that owns a virtual PTY URI scheme (e.g., `"colab"` for `colab://...`).
    pub fn find_pty_provider(&self, scheme: &str) -> Result<Option<(String, Arc<PluginProcess>)>, String> {
        let inner = self.inner.lock().unwrap();
        for (id, entry) in &inner.plugins {
            if !entry.record.enabled {
                continue;
            }
            if entry
                .manifest
                .as_ref()
                .is_some_and(|m| m.handles_pty_scheme(scheme))
            {
                let proc_ref = entry.process.clone().ok_or_else(|| {
                    entry
                        .last_error
                        .clone()
                        .unwrap_or_else(|| format!("Plugin '{id}' backend is not running."))
                })?;
                return Ok(Some((id.clone(), proc_ref)));
            }
        }
        Ok(None)
    }

    /// Shut down every running plugin sidecar on application exit.
    pub fn shutdown_all(&self) {
        let procs: Vec<Arc<PluginProcess>> = {
            let mut inner = self.inner.lock().unwrap();
            inner
                .plugins
                .values_mut()
                .filter_map(|p| {
                    p.status = PluginRuntimeStatus::Stopped;
                    p.process.take()
                })
                .collect()
        };
        for proc_ref in procs {
            proc_ref.shutdown();
        }
    }

    fn activate_entry(
        &self,
        app: &AppCtx,
        record: PluginRecord,
        manifest: PluginManifest,
    ) -> Result<PluginSummary, String> {
        let data_dir = app.data_dir()?;
        let id = record.id.clone();
        let root = PathBuf::from(&record.path);
        let has_backend = manifest.backend.is_some();

        let prev_proc = {
            let mut inner = self.inner.lock().unwrap();
            let rev = next_revision(&mut inner);
            let old = inner.plugins.get_mut(&id).and_then(|p| p.process.take());
            let status = if !record.enabled {
                PluginRuntimeStatus::Disabled
            } else if has_backend {
                PluginRuntimeStatus::Starting
            } else {
                PluginRuntimeStatus::Running
            };
            inner.plugins.insert(
                id.clone(),
                LoadedPlugin {
                    record: record.clone(),
                    manifest: Some(manifest.clone()),
                    process: None,
                    status,
                    last_error: None,
                    revision: rev,
                },
            );
            old
        };
        if let Some(proc_ref) = prev_proc {
            proc_ref.shutdown();
        }

        if record.enabled && has_backend {
            let pdata = plugin_data_dir(&data_dir, &id);
            spawn_into_state(
                app.clone(),
                Arc::clone(&self.inner),
                id.clone(),
                root,
                pdata,
                manifest,
            );
        }

        self.sync_cli_and_skills(&data_dir);
        app.emit(PLUGINS_CHANGED, ());

        let inner = self.inner.lock().unwrap();
        let entry = inner
            .plugins
            .get(&id)
            .ok_or_else(|| format!("Plugin '{id}' missing after activation."))?;
        Ok(summarize(entry))
    }

    fn sync_cli_and_skills(&self, data_dir: &Path) {
        let enabled_pairs: Vec<(PathBuf, PluginManifest)> = {
            let inner = self.inner.lock().unwrap();
            inner
                .plugins
                .values()
                .filter(|p| p.record.enabled)
                .filter_map(|p| {
                    p.manifest
                        .as_ref()
                        .map(|m| (PathBuf::from(&p.record.path), m.clone()))
                })
                .collect()
        };
        let refs: Vec<(&Path, &PluginManifest)> = enabled_pairs
            .iter()
            .map(|(p, m)| (p.as_path(), m))
            .collect();
        cli_skills::sync_artifacts(data_dir, &refs);
    }
}

fn next_revision(inner: &mut ManagerInner) -> u64 {
    let r = inner.next_rev;
    inner.next_rev = inner.next_rev.saturating_add(1);
    r
}

fn spawn_into_state(
    app: AppCtx,
    inner_ref: Arc<Mutex<ManagerInner>>,
    id: String,
    root: PathBuf,
    pdata: PathBuf,
    manifest: PluginManifest,
) {
    let inner_exit = Arc::clone(&inner_ref);
    let id_exit = id.clone();
    let res = PluginProcess::spawn(
        app.clone(),
        &root,
        &pdata,
        &manifest,
        Box::new(move |err_opt| {
            let mut inner = inner_exit.lock().unwrap();
            if let Some(entry) = inner.plugins.get_mut(&id_exit) {
                entry.process = None;
                if let Some(err) = err_opt {
                    entry.status = PluginRuntimeStatus::Error;
                    entry.last_error = Some(err);
                } else if entry.record.enabled {
                    entry.status = PluginRuntimeStatus::Stopped;
                }
            }
        }),
    );
    {
        let mut inner = inner_ref.lock().unwrap();
        if let Some(entry) = inner.plugins.get_mut(&id) {
            match res {
                Ok(proc_arc) => {
                    entry.process = Some(proc_arc);
                    entry.status = PluginRuntimeStatus::Running;
                    entry.last_error = None;
                }
                Err(err) => {
                    entry.process = None;
                    entry.status = PluginRuntimeStatus::Error;
                    entry.last_error = Some(err);
                }
            }
        }
    }
    app.emit(PLUGINS_CHANGED, ());
}

fn summarize(entry: &LoadedPlugin) -> PluginSummary {
    let m = entry.manifest.as_ref();
    PluginSummary {
        id: entry.record.id.clone(),
        name: m
            .map(|x| x.name.clone())
            .unwrap_or_else(|| entry.record.id.clone()),
        version: m.map(|x| x.version.clone()).unwrap_or_default(),
        description: m.map(|x| x.description.clone()).unwrap_or_default(),
        source: entry.record.source,
        path: entry.record.path.clone(),
        enabled: entry.record.enabled,
        status: entry.status,
        last_error: entry.last_error.clone(),
        revision: entry.revision,
        has_backend: m.and_then(|x| x.backend.as_ref()).is_some(),
        ui: m.and_then(|x| x.ui.clone()),
        commands: m
            .and_then(|x| x.backend.as_ref())
            .map(|b| b.commands.clone())
            .unwrap_or_default(),
        secret_commands: m
            .and_then(|x| x.backend.as_ref())
            .map(|b| b.secret_commands.clone())
            .unwrap_or_default(),
        pty_schemes: m
            .and_then(|x| x.backend.as_ref())
            .map(|b| b.pty_schemes.clone())
            .unwrap_or_default(),
        cli: m
            .map(|x| x.cli.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default(),
        skills: m
            .map(|x| x.skills.iter().map(|s| s.name.clone()).collect())
            .unwrap_or_default(),
        installed_at: entry.record.installed_at,
    }
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}

/// Unified command dispatcher mounted under `web::dispatch::dispatch_inner`.
pub fn dispatch(
    app: &AppCtx,
    origin: CallOrigin,
    cmd: &str,
    args: &Value,
) -> Result<Value, String> {
    if !host_enabled() {
        return match cmd {
            "plugin_list" => Ok(json!([])),
            "plugin_capabilities" => Ok(json!({
                "editable": false,
            })),
            _ => Err("Plugin host is currently disabled.".into()),
        };
    }
    match cmd {
        "plugin_list" => {
            let list = app.plugins().list();
            serde_json::to_value(list).map_err(|e| e.to_string())
        }
        "plugin_capabilities" => Ok(json!({
            "editable": origin != CallOrigin::Remote,
        })),
        "plugin_read_asset" => {
            let plugin_id = str_arg(args, "pluginId")?;
            let path = str_arg(args, "path")?;
            let asset = app.plugins().read_asset(plugin_id, path)?;
            serde_json::to_value(asset).map_err(|e| e.to_string())
        }
        "plugin_invoke" => {
            let plugin_id = str_arg(args, "pluginId")?;
            let command = str_arg(args, "command")?;
            let payload = args.get("args").unwrap_or(&Value::Null);
            app.plugins().invoke_plugin(origin, plugin_id, command, payload)
        }
        "plugin_install_archive" => {
            let path = str_arg(args, "path")?;
            let summary = app.plugins().install_archive(app, Path::new(path))?;
            serde_json::to_value(summary).map_err(|e| e.to_string())
        }
        "plugin_link_dev" => {
            let path = str_arg(args, "path")?;
            let summary = app.plugins().link_dev(app, Path::new(path))?;
            serde_json::to_value(summary).map_err(|e| e.to_string())
        }
        "plugin_set_enabled" => {
            let plugin_id = str_arg(args, "pluginId")?;
            let enabled = args
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or("Missing required boolean parameter: enabled")?;
            let summary = app.plugins().set_enabled(app, plugin_id, enabled)?;
            serde_json::to_value(summary).map_err(|e| e.to_string())
        }
        "plugin_reload" => {
            let plugin_id = str_arg(args, "pluginId")?;
            let summary = app.plugins().reload(app, plugin_id)?;
            serde_json::to_value(summary).map_err(|e| e.to_string())
        }
        "plugin_uninstall" => {
            let plugin_id = str_arg(args, "pluginId")?;
            let remove_data = args
                .get("removeData")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            app.plugins().uninstall(app, plugin_id, remove_data)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(format!("Unknown plugin command: {cmd}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::server::HookServer;
    use crate::db::Db;
    use crate::host::HeadlessHost;
    use std::sync::Arc;

    fn test_ctx() -> (AppCtx, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "vlx_plugins_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir.join("vlx-term.db")).unwrap();
        let host = Arc::new(HeadlessHost::new(dir.clone(), db));
        let ctx = AppCtx::Headless(Arc::clone(&host));
        host.set_hooks(HookServer::start(ctx.clone()).unwrap());
        (ctx, dir)
    }

    #[test]
    fn link_dev_reads_ui_assets_and_blocks_traversal() {
        let (ctx, dir) = test_ctx();
        let plugin_dir = dir.join("sample-plugin");
        std::fs::create_dir_all(plugin_dir.join("ui")).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": 1,
                "id": "demo",
                "name": "Demo Plugin",
                "version": "0.1.0",
                "ui": {
                    "entry": "ui/entry.js",
                    "style": "ui/style.css"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(plugin_dir.join("ui/entry.js"), "export function activate() { return {}; }")
            .unwrap();
        std::fs::write(plugin_dir.join("ui/style.css"), ".demo { color: red; }").unwrap();
        std::fs::write(plugin_dir.join("secret.txt"), "top-secret").unwrap();

        let summary = ctx.plugins().link_dev(&ctx, &plugin_dir).unwrap();
        assert_eq!(summary.id, "demo");
        assert_eq!(summary.status, PluginRuntimeStatus::Running);

        let asset = ctx.plugins().read_asset("demo", "ui/entry.js").unwrap();
        assert!(asset.content.contains("activate"));
        assert!(asset.mime_type.contains("javascript"));

        // Must block traversal and reads outside ui/ or locales/.
        assert!(ctx.plugins().read_asset("demo", "../secret.txt").is_err());
        assert!(ctx.plugins().read_asset("demo", "secret.txt").is_err());
        assert!(ctx.plugins().read_asset("demo", "manifest.json").is_err());

        // Disabling and uninstalling leaves dev source intact.
        let disabled = ctx.plugins().set_enabled(&ctx, "demo", false).unwrap();
        assert!(!disabled.enabled);
        assert_eq!(disabled.status, PluginRuntimeStatus::Disabled);

        ctx.plugins().uninstall(&ctx, "demo", true).unwrap();
        assert!(ctx.plugins().list().is_empty());
        assert!(plugin_dir.join("manifest.json").is_file());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn sidecar_rpc_secret_gate_cli_and_pty_provider_roundtrip() {
        use std::os::unix::fs::PermissionsExt;

        let (ctx, dir) = test_ctx();
        let plugin_dir = dir.join("echo-plugin");
        std::fs::create_dir_all(plugin_dir.join("bin")).unwrap();
        std::fs::create_dir_all(plugin_dir.join("ui")).unwrap();

        let script = r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"method":"plugin.ping"'*)
      printf '{"id":%s,"result":{"ok":true}}\n' "$id"
      ;;
    *'"method":"rpc.call"'*)
      printf '{"event":"plugin.emit","params":{"name":"echo://ping","payload":{"ok":true}}}\n'
      printf '{"id":%s,"result":{"echoed":true}}\n' "$id"
      ;;
    *'"method":"cli.exec"'*)
      printf '{"id":%s,"result":{"stdout":"hello-cli\\n","stderr":"","exitCode":0}}\n' "$id"
      ;;
    *'"method":"shutdown"'*)
      exit 0
      ;;
  esac
done
"#;
        let bin_path = plugin_dir.join("bin/echo-sidecar");
        std::fs::write(&bin_path, script).unwrap();
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(plugin_dir.join("ui/entry.js"), "export function activate() { return {}; }")
            .unwrap();
        std::fs::write(
            plugin_dir.join("manifest.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": 1,
                "id": "echo",
                "name": "Echo Plugin",
                "version": "1.0.0",
                "backend": {
                    "entry": "bin/echo-sidecar",
                    "commands": ["echo_*"],
                    "secretCommands": ["echo_secret"],
                    "ptySchemes": ["echo"]
                },
                "ui": {
                    "entry": "ui/entry.js"
                },
                "cli": [{ "name": "vecho" }]
            }))
            .unwrap(),
        )
        .unwrap();

        let summary = ctx.plugins().link_dev(&ctx, &plugin_dir).unwrap();
        assert_eq!(summary.status, PluginRuntimeStatus::Running);
        assert!(dir.join("bin/vecho").is_file());

        // Local RPC works.
        let res = ctx
            .plugins()
            .invoke_plugin(CallOrigin::Local, "echo", "echo_hello", &json!({}))
            .unwrap();
        assert_eq!(res["echoed"], json!(true));

        // Remote RPC to secretCommand is rejected by host before hitting sidecar.
        let remote_err = ctx
            .plugins()
            .invoke_plugin(CallOrigin::Remote, "echo", "echo_secret", &json!({}))
            .unwrap_err();
        assert!(remote_err.contains("not allowed from remote clients"));

        // CLI execution works.
        let cli_res = ctx
            .plugins()
            .exec_cli("echo", "vecho", &["--version".to_string()], None)
            .unwrap();
        assert_eq!(cli_res["stdout"], json!("hello-cli\n"));
        assert_eq!(cli_res["exitCode"], json!(0));

        // Virtual PTY provider lookup finds the running plugin.
        let provider = ctx.plugins().find_pty_provider("echo").unwrap();
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().0, "echo");

        // Uninstall cleanly shuts down sidecar and removes managed shim.
        ctx.plugins().uninstall(&ctx, "echo", true).unwrap();
        assert!(!dir.join("bin/vecho").exists());

        let _ = std::fs::remove_dir_all(dir);
    }
}
