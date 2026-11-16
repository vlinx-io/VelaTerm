//! Model catalogue reported by an OpenCode server.
//!
//! OpenCode knows which providers have credentials and which models each offers, so the list is asked of
//! a server rather than kept here. A session that is already running lends its server; otherwise a
//! short-lived one is started for the question and stopped straight after, the way the Codex catalogue
//! is read from a throwaway app-server.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::agent::chat::opencode_protocol::{self, OpencodeModel, Server};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// The models OpenCode offers, optionally restricted to one provider. A DeepSeek conversation uses the
/// filter so the picker cannot offer another provider's models.
pub fn list_provider(app: &crate::host::AppCtx, session_id: &str, bin: &str, cwd: Option<&str>, running: Option<Arc<Server>>, provider: Option<&str>) -> Result<Vec<OpencodeModel>, String> {
    if let Some(server) = running {
        return fetch(&server, provider);
    }
    let port = opencode_protocol::configured_port(app, &format!("catalog.{session_id}"))?;
    let password = opencode_protocol::random_password();
    let mut command = crate::host::command(bin);
    command
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--hostname")
        .arg("127.0.0.1")
        .env("OPENCODE_SERVER_PASSWORD", &password)
        .env("OPENCODE_SERVER_USERNAME", "opencode")
        .env_remove(crate::agent::inject::OPENCODE_CONFIG_ENV)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    crate::login_env::refresh_command(&mut command);
    crate::agent::executable::prepare_command(&mut command, bin);
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS {
        command.env_remove(key);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start OpenCode: {e}"))?;
    let server = Server::new(port, &password, cwd);
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let result = loop {
        if server.healthy() {
            break fetch(&server, provider);
        }
        if let Ok(Some(status)) = child.try_wait() {
            break Err(format!("OpenCode exited before answering ({status})"));
        }
        if Instant::now() > deadline {
            break Err("OpenCode's server did not start in time".to_string());
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn fetch(server: &Server, provider: Option<&str>) -> Result<Vec<OpencodeModel>, String> {
    let providers = server.get("/provider")?;
    let models = opencode_protocol::parse_models(&providers);
    Ok(match provider {
        Some(provider) => models.into_iter()
            .filter(|model| model.id.split_once('/').is_some_and(|(id, _)| id == provider))
            .collect(),
        None => models,
    })
}

/// Context windows by provider, then model, read from one version of the catalogue file.
type Windows = HashMap<String, HashMap<String, u64>>;

/// The parsed catalogue and the file version it came from. The Info panel asks every second or so, and the
/// file runs to megabytes, so it is read again only when it changes.
static WINDOWS: Mutex<Option<(PathBuf, SystemTime, u64, Arc<Windows>)>> = Mutex::new(None);

/// Where OpenCode caches the models.dev catalogue: `OPENCODE_MODELS_PATH` when set, otherwise
/// `$XDG_CACHE_HOME/opencode/models.json`, falling back to `~/.cache` on every platform as OpenCode does.
fn catalogue_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("OPENCODE_MODELS_PATH").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| crate::host::home_dir().map(|home| home.join(".cache")))?;
    Some(cache.join("opencode").join("models.json"))
}

/// The context window OpenCode's own catalogue gives a model.
///
/// OpenCode records the provider and model on every reply but not the window, so the Info panel looks it
/// up in the catalogue the CLI cached on disk, the way Pi's limit is found. The named provider wins; other
/// providers are scanned only when it does not list the model. A model the catalogue does not know, such
/// as one from a custom provider, leaves the limit unknown.
pub fn cached_context_window(provider: Option<&str>, model: &str) -> Option<u64> {
    let path = catalogue_path()?;
    let meta = std::fs::metadata(&path).ok()?;
    let (modified, len) = (meta.modified().ok()?, meta.len());
    let windows = {
        let mut cached = WINDOWS.lock().unwrap();
        match cached.as_ref() {
            Some((p, m, l, windows)) if *p == path && *m == modified && *l == len => windows.clone(),
            _ => {
                let parsed: Value = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
                let windows = Arc::new(windows_from_catalogue(&parsed));
                *cached = Some((path, modified, len, windows.clone()));
                windows
            }
        }
    };
    window_for(&windows, provider, model)
}

/// Keep only what the lookup needs: `{provider: {models: {model: {limit: {context}}}}}`.
fn windows_from_catalogue(catalogue: &Value) -> Windows {
    let Some(providers) = catalogue.as_object() else { return Windows::new(); };
    providers.iter().map(|(provider, entry)| {
        let models = entry.get("models").and_then(Value::as_object).map(|models| models.iter()
            .filter_map(|(id, model)| model.pointer("/limit/context").and_then(Value::as_u64)
                .filter(|window| *window > 0).map(|window| (id.clone(), window)))
            .collect()).unwrap_or_default();
        (provider.clone(), models)
    }).collect()
}

fn window_for(windows: &Windows, provider: Option<&str>, model: &str) -> Option<u64> {
    if let Some(window) = provider.and_then(|p| windows.get(p)).and_then(|models| models.get(model)) {
        return Some(*window);
    }
    windows.values().find_map(|models| models.get(model).copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catalogue_supplies_the_named_providers_window_first() {
        let windows = windows_from_catalogue(&json!({
            "deepseek": {"models": {"deepseek-flash": {"limit": {"context": 1_000_000, "output": 393_216}}}},
            "openrouter": {"models": {
                "deepseek-flash": {"limit": {"context": 128_000}},
                "unlimited": {"limit": {"context": 0}}
            }},
            "broken": {"models": []}
        }));
        assert_eq!(window_for(&windows, Some("deepseek"), "deepseek-flash"), Some(1_000_000));
        assert_eq!(window_for(&windows, Some("openrouter"), "deepseek-flash"), Some(128_000));
        // A provider the catalogue spells differently still finds the model elsewhere.
        assert!(window_for(&windows, Some("custom"), "deepseek-flash").is_some());
        assert_eq!(window_for(&windows, Some("openrouter"), "unlimited"), None);
        assert_eq!(window_for(&windows, None, "missing"), None);
        assert!(windows_from_catalogue(&json!([])).is_empty());
    }
}
