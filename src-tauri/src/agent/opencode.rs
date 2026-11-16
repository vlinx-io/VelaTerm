//! Extraction and discovery of the opencode status-bridge plugin.
//!
//! vlx-term bundles an opencode plugin via compile-time `include_str!` and extracts it to `opencode/` under the
//! application data directory at startup. When launching an opencode session, it injects
//! `OPENCODE_CONFIG_CONTENT={"plugin":["<absolute plugin path>"]}` (see `agent/inject.rs` and `pty/manager.rs`),
//! causing opencode to load the plugin by its local absolute path.
//!
//! The plugin POSTs events such as `session.status`, `session.idle`, and `permission.updated` to the local hook
//! service using injected `VLX_SESSION_ID`/`VLX_TOKEN`/`VLX_SPAWN_URL`, giving opencode sessions authoritative
//! activity state. See `resources/vlx-opencode-notify.js` for the source.

use std::path::{Path, PathBuf};

/// Plugin source embedded at compile time and distributed with the binary, requiring no external file.
const PLUGIN: &str = include_str!("../../resources/vlx-opencode-notify.js");
/// Extracted filename.
const PLUGIN_NAME: &str = "vlx-opencode-notify.js";

/// Absolute extracted path: `<data_dir>/opencode/vlx-opencode-notify.js`.
pub fn plugin_path(data_dir: &Path) -> PathBuf {
    data_dir.join("opencode").join(PLUGIN_NAME)
}

/// Extract the opencode plugin to `<data_dir>/opencode/` and return its path.
///
/// Overwrite on every startup so the plugin tracks the application version.
pub fn install(data_dir: &Path) -> std::io::Result<PathBuf> {
    let path = plugin_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, PLUGIN)?;
    Ok(path)
}

/// The preview identifies itself as 0.0.0-beta; stable v2 uses major version 2.
pub fn is_v2(bin: &str) -> bool {
    if Path::new(bin).file_stem().and_then(|v|v.to_str())==Some("opencode2") {return true;}
    is_v2_version(super::version::installed(bin).as_deref())
}

pub fn is_v2_version(version: Option<&str>) -> bool {
    version.and_then(|v|semver::Version::parse(v).ok())
        .is_some_and(|v|v.major>=2 || (v.major==0 && v.pre.as_str().starts_with("beta")))
}

/// Flags passed after `run` for one-shot `opencode run` invocations across v1 and v2.
///
/// v1 isolates with `--pure`; v2 removed `--pure` and isolates with `--standalone`.
pub fn run_flags(v2: bool) -> &'static [&'static str] {
    if v2 {
        &["--standalone", "--format", "json"]
    } else {
        &["--format", "json", "--pure"]
    }
}

/// Complete subcommand and flags for one-shot `opencode run` across v1 and v2.
pub fn run_args(v2: bool) -> &'static [&'static str] {
    if v2 {
        &["run", "--standalone", "--format", "json"]
    } else {
        &["run", "--format", "json", "--pure"]
    }
}

/// Format `--model` and `--variant` arguments for one-shot `opencode run`.
///
/// OpenCode v2 removed `--variant` and appends the variant as `<model>#<effort>` inside `--model`.
pub fn run_selection_args(v2: bool, model: Option<&str>, effort: Option<&str>) -> Vec<String> {
    let model = model.map(str::trim).filter(|m| !m.is_empty());
    let effort = effort.map(str::trim).filter(|e| !e.is_empty());
    let Some(model) = model else {
        return if !v2 {
            effort.map(|e| vec!["--variant".into(), e.to_string()]).unwrap_or_default()
        } else {
            Vec::new()
        };
    };
    if v2 {
        let base = model.split('#').next().unwrap_or(model);
        let value = match effort {
            Some(effort) if !base.is_empty() => format!("{base}#{effort}"),
            _ => model.to_string(),
        };
        vec!["--model".into(), value]
    } else {
        let mut args = vec!["--model".into(), model.to_string()];
        if let Some(effort) = effort {
            args.push("--variant".into());
            args.push(effort.to_string());
        }
        args
    }
}

/// Validate and format model and variant flags for callers expecting shell-tokenized strings.
pub fn run_selection_flags(
    kind: crate::models::SessionKind,
    v2: bool,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<Option<String>, String> {
    if !v2 {
        return super::launch_options::apply(kind, None, model, effort);
    }
    super::launch_options::apply(kind, None, model, effort)?;
    let args = run_selection_args(true, model, effort);
    if args.is_empty() {
        return Ok(None);
    }
    let mut parts = Vec::with_capacity(args.len());
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        if let Some(value) = iter.next() {
            parts.push(format!("{flag} '{value}'"));
        } else {
            parts.push(flag);
        }
    }
    Ok(Some(parts.join(" ")))
}

/// v2's full TUI has no model flag or v1 plugin hooks. Prepare its native session through its own
/// authenticated API so even a fresh terminal has a durable identity before it starts.
pub fn prepare_v2_terminal(app: &crate::host::AppCtx, session: &str, bin: &str, cwd: Option<&str>,
    resume: Option<&str>, choice: &super::session_settings::Selection) -> Result<String,String> {
    use crate::agent::chat::opencode_protocol as wire;
    let port=wire::configured_port(app,&format!("terminal.{session}"))?;
    let password=wire::random_password();
    let mut command=crate::host::command(bin);
    command.args(["serve","--hostname","127.0.0.1","--port"]).arg(port.to_string())
        .env("OPENCODE_SERVER_PASSWORD",&password).env("OPENCODE_SERVER_USERNAME","opencode")
        .env_remove(super::inject::OPENCODE_CONFIG_ENV)
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    if let Some(cwd)=cwd {command.current_dir(cwd);}
    crate::login_env::refresh_command(&mut command);
    super::executable::prepare_command(&mut command,bin);
    let mut child=command.spawn().map_err(|e|format!("Failed to prepare OpenCode v2's terminal: {e}"))?;
    let result=(|| {
        let server=wire::Server::new(port,&password,cwd);
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(30);
        while !server.healthy() {
            if child.try_wait().map_err(|e|e.to_string())?.is_some() || std::time::Instant::now()>=deadline {
                return Err("OpenCode v2 did not start its terminal preparation server".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        if !server.is_v2(){return Err("The configured OpenCode executable did not expose the v2 API".into());}
        let native=match resume {Some(id)=>server.get(&format!("/session/{}",wire::encode_query(id)))?,None=>server.post("/session",None)?};
        let id=native.get("id").and_then(serde_json::Value::as_str).ok_or("OpenCode did not return a session id")?;
        if native.get("agent").and_then(serde_json::Value::as_str).is_none() {
            let modes=wire::parse_agents(&server.get("/agent")?);
            let agent=modes.first().ok_or("OpenCode has no primary agent available for the terminal")?;
            server.post(&format!("/session/{id}/agent"),Some(&serde_json::json!({"agent":agent.mode})))?;
        }
        if let Some(model)=choice.model.as_deref() {
            let (provider,model)=wire::split_model(model).ok_or("The OpenCode model must include its provider")?;
            server.post(&format!("/session/{id}/model"),Some(&serde_json::json!({"model":{"id":model,"providerID":provider,
                "variant":choice.effort.as_deref().unwrap_or("default")}})))?;
        }
        crate::db::repo::set_agent_session_id(&app.db().conn.lock().unwrap(),session,id,crate::models::SessionKind::Opencode)?;
        Ok(id.to_string())
    })();
    let _=child.kill();let _=child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_plugin_at_expected_path() {
        let tmp = std::env::temp_dir().join(format!("vlx-oc-plugin-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);

        let path = install(&tmp).expect("writing out the plugin should succeed");
        assert_eq!(path, plugin_path(&tmp));

        let written = std::fs::read_to_string(&path).expect("the plugin should be readable again");
        assert_eq!(written, PLUGIN, "the contents should be the inlined source");
        // Coarsely verify that the plugin exports the event hook and reports as expected, guarding against an accidentally empty source.
        assert!(written.contains("session.idle"), "the plugin should map session.idle");
        assert!(
            written.contains("VLX_SESSION_ID"),
            "the plugin should read VLX_SESSION_ID to decide whether it is enabled"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn run_helpers_adapt_flags_and_model_variant_for_v1_and_v2() {
        assert_eq!(run_args(false), ["run", "--format", "json", "--pure"]);
        assert_eq!(run_args(true), ["run", "--standalone", "--format", "json"]);
        assert_eq!(run_flags(false), ["--format", "json", "--pure"]);
        assert_eq!(run_flags(true), ["--standalone", "--format", "json"]);

        assert_eq!(
            run_selection_args(false, Some("deepseek/deepseek-flash"), Some("high")),
            ["--model", "deepseek/deepseek-flash", "--variant", "high"]
        );
        assert_eq!(
            run_selection_args(true, Some("deepseek/deepseek-flash"), Some("high")),
            ["--model", "deepseek/deepseek-flash#high"]
        );
        assert_eq!(
            run_selection_args(true, Some("deepseek/deepseek-flash"), None),
            ["--model", "deepseek/deepseek-flash"]
        );
        assert!(run_selection_args(true, None, Some("high")).is_empty());

        assert_eq!(
            run_selection_flags(
                crate::models::SessionKind::Deepseek,
                true,
                Some("deepseek/deepseek-flash"),
                Some("max")
            )
            .unwrap()
            .as_deref(),
            Some("--model 'deepseek/deepseek-flash#max'")
        );
        assert!(run_selection_flags(
            crate::models::SessionKind::Deepseek,
            true,
            Some("bad model"),
            None
        )
        .is_err());
    }
}
