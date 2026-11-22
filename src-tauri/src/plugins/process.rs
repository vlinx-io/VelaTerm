//! Out-of-process plugin sidecar transport using NDJSON over stdin/stdout.
//!
//! One sidecar process multiplexes:
//! - Request/response JSON-RPC (`rpc.call`, `cli.exec`, `pty.open`)
//! - Fire-and-forget notifications (`pty.input`, `pty.resize`, `pty.close`, `shutdown`)
//! - Server-pushed events (`plugin.emit`, `pty.output`, `pty.exit`)

use super::manifest::PluginManifest;
use super::{PLUGINS_CHANGED, PLUGIN_EVENT};
use crate::host::AppCtx;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

/// Callback bundle registered by `PtyManager` for an active plugin-backed virtual PTY stream.
pub struct PluginPtySink {
    pub on_output: Box<dyn FnMut(&[u8]) + Send + 'static>,
    pub on_exit: Box<dyn FnOnce(i32, Option<String>) + Send + 'static>,
}

/// Callback invoked when the sidecar process unexpectedly exits.
pub type OnProcessExit = Box<dyn FnOnce(Option<String>) + Send + 'static>;

/// Live handle to one running external plugin sidecar process.
pub struct PluginProcess {
    plugin_id: String,
    next_id: AtomicU64,
    alive: Arc<AtomicBool>,
    shutting_down: Arc<AtomicBool>,
    writer: Mutex<Option<BufWriter<ChildStdin>>>,
    child: Mutex<Option<Child>>,
    pending: Arc<Mutex<HashMap<u64, mpsc::SyncSender<Result<Value, String>>>>>,
    pty_sinks: Arc<Mutex<HashMap<String, PluginPtySink>>>,
}

impl PluginProcess {
    /// Spawn a plugin backend binary and wire its NDJSON stdin/stdout/stderr pumps.
    pub fn spawn(
        app: AppCtx,
        plugin_root: &Path,
        plugin_data_dir: &Path,
        manifest: &PluginManifest,
        on_exit: OnProcessExit,
    ) -> Result<Arc<Self>, String> {
        let backend = manifest
            .backend
            .as_ref()
            .ok_or_else(|| format!("Plugin '{}' declares no backend entry.", manifest.id))?;
        let rel = backend.entry.resolve_current().ok_or_else(|| {
            format!(
                "Plugin '{}' has no backend executable for this platform.",
                manifest.id
            )
        })?;
        let bin_path = plugin_root.join(rel);
        if !bin_path.is_file() {
            return Err(format!(
                "Plugin '{}' backend executable not found: {}",
                manifest.id,
                bin_path.display()
            ));
        }

        std::fs::create_dir_all(plugin_data_dir)
            .map_err(|e| format!("Failed creating plugin data dir: {e}"))?;

        let mut cmd = crate::host::command(&bin_path);
        cmd.args(&backend.args)
            .current_dir(plugin_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("VLX_PLUGIN_ID", &manifest.id)
            .env("VLX_PLUGIN_ROOT", plugin_root)
            .env("VLX_PLUGIN_DATA_DIR", plugin_data_dir)
            .env("VLX_APP_IDENTIFIER", crate::command_core::app_identifier(&app));

        if let Ok(data_dir) = app.data_dir() {
            cmd.env("VLX_LEGACY_DB_PATH", data_dir.join("vlx-term.db"));
            cmd.env("VLX_HOST_DATA_DIR", data_dir);
        }
        cmd.env("VLX_HOOK_PORT", app.hooks().port.to_string());
        cmd.env("VLX_HOOK_TOKEN", &app.hooks().token);

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        let mut child = cmd.spawn().map_err(|e| {
            format!(
                "Failed spawning plugin '{}' ({}): {e}",
                manifest.id,
                bin_path.display()
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("Plugin '{}' missing stdin pipe", manifest.id))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("Plugin '{}' missing stdout pipe", manifest.id))?;
        let stderr = child.stderr.take();

        let alive = Arc::new(AtomicBool::new(true));
        let shutting_down = Arc::new(AtomicBool::new(false));
        let pending: Arc<Mutex<HashMap<u64, mpsc::SyncSender<Result<Value, String>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pty_sinks: Arc<Mutex<HashMap<String, PluginPtySink>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let proc_arc = Arc::new(Self {
            plugin_id: manifest.id.clone(),
            next_id: AtomicU64::new(1),
            alive: Arc::clone(&alive),
            shutting_down: Arc::clone(&shutting_down),
            writer: Mutex::new(Some(BufWriter::new(stdin))),
            child: Mutex::new(Some(child)),
            pending: Arc::clone(&pending),
            pty_sinks: Arc::clone(&pty_sinks),
        });

        // Stderr pump -> diagnostics log.
        if let Some(err_pipe) = stderr {
            let pid = manifest.id.clone();
            let _ = std::thread::Builder::new()
                .name(format!("plugin-stderr-{pid}"))
                .spawn(move || {
                    let reader = BufReader::new(err_pipe);
                    for line in reader.lines().map_while(Result::ok) {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            crate::diagnostics::record(
                                "INFO",
                                "plugin_stderr",
                                json!({ "pluginId": pid, "line": trimmed }),
                            );
                        }
                    }
                });
        }

        // Stdout NDJSON reader pump.
        {
            let pid = manifest.id.clone();
            let app_ref = app.clone();
            let alive_ref = Arc::clone(&alive);
            let shutting_ref = Arc::clone(&shutting_down);
            let pending_ref = Arc::clone(&pending);
            let sinks_ref = Arc::clone(&pty_sinks);
            std::thread::Builder::new()
                .name(format!("plugin-stdout-{pid}"))
                .spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let Ok(msg) = serde_json::from_str::<Value>(trimmed) else {
                            crate::diagnostics::record(
                                "WARN",
                                "plugin_bad_json",
                                json!({ "pluginId": pid, "raw": trimmed.chars().take(240).collect::<String>() }),
                            );
                            continue;
                        };
                        if let Some(id) = msg.get("id").and_then(Value::as_u64) {
                            let res = if let Some(err) = msg.get("error").filter(|v| !v.is_null()) {
                                let text = err
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .or_else(|| err.as_str())
                                    .unwrap_or("Plugin RPC error")
                                    .to_string();
                                Err(text)
                            } else {
                                Ok(msg.get("result").cloned().unwrap_or(Value::Null))
                            };
                            if let Some(tx) = pending_ref.lock().unwrap().remove(&id) {
                                let _ = tx.send(res);
                            }
                            continue;
                        }
                        if let Some(ev) = msg.get("event").and_then(Value::as_str) {
                            let params = msg.get("params").unwrap_or(&Value::Null);
                            handle_event(&app_ref, &pid, ev, params, &sinks_ref);
                        }
                    }

                    // Stream ended: tear down pending requests and open PTYs.
                    alive_ref.store(false, Ordering::SeqCst);
                    let was_intentional = shutting_ref.load(Ordering::SeqCst);
                    let pending_all: Vec<_> = pending_ref.lock().unwrap().drain().collect();
                    for (_, tx) in pending_all {
                        let _ = tx.send(Err(format!("Plugin '{pid}' process stopped.")));
                    }
                    let sinks_all: Vec<_> = sinks_ref.lock().unwrap().drain().collect();
                    for (_, sink) in sinks_all {
                        (sink.on_exit)(1, Some("Plugin backend exited.".to_string()));
                    }
                    let err_msg = if was_intentional {
                        None
                    } else {
                        Some(format!("Plugin '{pid}' backend process exited unexpectedly."))
                    };
                    on_exit(err_msg);
                    if !was_intentional {
                        app_ref.emit(PLUGINS_CHANGED, ());
                    }
                })
                .map_err(|e| format!("Failed starting plugin stdout reader: {e}"))?;
        }

        // Verify handshake readiness so broken binaries fail fast on start.
        proc_arc.call("plugin.ping", json!({}), Duration::from_secs(10))?;
        Ok(proc_arc)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Send a JSON-RPC request and block until the sidecar responds or `timeout` elapses.
    pub fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        if !self.is_alive() {
            return Err(format!("Plugin '{}' is not running.", self.plugin_id));
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::sync_channel(1);
        self.pending.lock().unwrap().insert(id, tx);

        if let Err(err) = self.write_line(&json!({
            "id": id,
            "method": method,
            "params": params,
        })) {
            self.pending.lock().unwrap().remove(&id);
            return Err(err);
        }

        match rx.recv_timeout(timeout) {
            Ok(res) => res,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!(
                    "Plugin '{}' timed out handling '{method}'.",
                    self.plugin_id
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
                "Plugin '{}' disconnected while handling '{method}'.",
                self.plugin_id
            )),
        }
    }

    /// Send a fire-and-forget notification frame to the sidecar.
    pub fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        if !self.is_alive() {
            return Err(format!("Plugin '{}' is not running.", self.plugin_id));
        }
        self.write_line(&json!({
            "method": method,
            "params": params,
        }))
    }

    /// Register a virtual PTY stream and ask the plugin sidecar to open it.
    pub fn open_pty(
        &self,
        pty_id: &str,
        scheme: &str,
        target: &str,
        cols: u16,
        rows: u16,
        sink: PluginPtySink,
    ) -> Result<(), String> {
        self.pty_sinks.lock().unwrap().insert(pty_id.to_string(), sink);
        let res = self.call(
            "pty.open",
            json!({
                "ptyId": pty_id,
                "scheme": scheme,
                "target": target,
                "cols": cols,
                "rows": rows,
            }),
            Duration::from_secs(25),
        );
        if let Err(err) = res {
            self.pty_sinks.lock().unwrap().remove(pty_id);
            return Err(err);
        }
        Ok(())
    }

    pub fn write_pty(&self, pty_id: &str, bytes: &[u8]) -> Result<(), String> {
        self.notify(
            "pty.input",
            json!({
                "ptyId": pty_id,
                "dataB64": B64.encode(bytes),
            }),
        )
    }

    pub fn resize_pty(&self, pty_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        self.notify(
            "pty.resize",
            json!({
                "ptyId": pty_id,
                "cols": cols,
                "rows": rows,
            }),
        )
    }

    pub fn close_pty(&self, pty_id: &str) {
        self.pty_sinks.lock().unwrap().remove(pty_id);
        let _ = self.notify("pty.close", json!({ "ptyId": pty_id }));
    }

    /// Gracefully ask the sidecar to exit, then terminate its process tree if it does not stop promptly.
    pub fn shutdown(&self) {
        if self.shutting_down.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = self.write_line(&json!({ "method": "shutdown", "params": {} }));
        // Drop stdin to signal EOF.
        let _ = self.writer.lock().unwrap().take();

        if let Some(mut child) = self.child.lock().unwrap().take() {
            let deadline = std::time::Instant::now() + Duration::from_millis(1200);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    _ => {
                        crate::host::kill_process_tree(&mut child);
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
        self.alive.store(false, Ordering::SeqCst);
    }

    fn write_line(&self, value: &Value) -> Result<(), String> {
        let mut guard = self.writer.lock().unwrap();
        let Some(writer) = guard.as_mut() else {
            return Err(format!("Plugin '{}' stdin closed.", self.plugin_id));
        };
        let encoded = serde_json::to_string(value).map_err(|e| e.to_string())?;
        writer
            .write_all(encoded.as_bytes())
            .and_then(|_| writer.write_all(b"\n"))
            .and_then(|_| writer.flush())
            .map_err(|e| format!("Failed writing to plugin '{}': {e}", self.plugin_id))
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn handle_event(
    app: &AppCtx,
    plugin_id: &str,
    event_kind: &str,
    params: &Value,
    pty_sinks: &Arc<Mutex<HashMap<String, PluginPtySink>>>,
) {
    match event_kind {
        "plugin.emit" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return;
            };
            let payload = params.get("payload").cloned().unwrap_or(Value::Null);
            app.emit(
                PLUGIN_EVENT,
                json!({
                    "pluginId": plugin_id,
                    "event": name,
                    "payload": payload,
                }),
            );
        }
        "pty.output" => {
            let Some(pty_id) = params.get("ptyId").and_then(Value::as_str) else {
                return;
            };
            let Some(b64) = params.get("dataB64").and_then(Value::as_str) else {
                return;
            };
            let Ok(bytes) = B64.decode(b64) else {
                return;
            };
            if let Some(sink) = pty_sinks.lock().unwrap().get_mut(pty_id) {
                (sink.on_output)(&bytes);
            }
        }
        "pty.exit" => {
            let Some(pty_id) = params.get("ptyId").and_then(Value::as_str) else {
                return;
            };
            let code = params.get("code").and_then(Value::as_i64).unwrap_or(0) as i32;
            let reason = params.get("reason").and_then(Value::as_str).map(str::to_string);
            if let Some(sink) = pty_sinks.lock().unwrap().remove(pty_id) {
                (sink.on_exit)(code, reason);
            }
        }
        _ => {}
    }
}
