//! Model selectors and their reasoning levels for a launch draft, before a child session exists.

use crate::{db::repo, host::AppCtx, models::SessionKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub parent_session_id: Option<String>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub inherit_args: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub effort_levels: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub models: Vec<Model>,
    /// Native levels offered before the user chooses a specific model.
    pub effort_levels: Vec<String>,
}

pub fn list(app: &AppCtx, kind: SessionKind, context: &Context) -> Result<Catalog, String> {
    let (parent, root, settings) = {
        let conn = app.db().conn.lock().unwrap();
        let parent = context
            .parent_session_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .map(|id| {
                repo::get_session(&conn, id)?
                    .ok_or_else(|| "The parent session no longer exists".to_string())
            })
            .transpose()?;
        let root = parent
            .as_ref()
            .map(|p| repo::get_project_root(&conn, &p.project_id))
            .transpose()?
            .flatten();
        let settings = repo::get_app_settings(&conn)?
            .remove("vlx-settings")
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or_default();
        (parent, root, settings)
    };
    let same_agent = parent.as_ref().filter(|p| p.kind == kind);
    let bin =
        super::executable::resolve(app, kind, same_agent.and_then(|p| p.agent_path.as_deref()))
            .unwrap_or_else(|| super::executable::command_name(kind).to_string());
    let cwd = context
        .cwd
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| parent.as_ref().and_then(|p| p.cwd.as_deref()))
        .or(root.as_deref());
    let args = if context.inherit_args {
        same_agent
            .and_then(|p| p.agent_args.as_deref())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| settings["agentDefaults"][kind.as_str()]["args"].as_str())
    } else {
        None
    };
    let extra = super::inject::split_extra_args(Some(
        &super::session_settings::without_selection_args(kind, args),
    ));
    let value = match kind {
        SessionKind::Claude => serde_json::to_value(super::claude_models::list_for_bin(app, &bin)),
        SessionKind::Codex => {
            serde_json::to_value(super::codex_models::list_in_dir(&bin, &extra, cwd)?)
        }
        SessionKind::Opencode => {
            // A stable scope reuses the backend's reserved port. Serialize discovery so simultaneous
            // clients do not treat another lookup's listener as a port conflict.
            static LOOKUP: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _guard = LOOKUP.lock().unwrap();
            let scope = format!(
                "launch-{:x}",
                Sha256::digest(format!("{bin}\0{}", cwd.unwrap_or("")))
            );
            serde_json::to_value(super::opencode_models::list(app, &scope, &bin, cwd, None)?)
        }
        SessionKind::Pi | SessionKind::Omp => {
            serde_json::to_value(super::pi_models::list_for_launch(kind, &bin, cwd, &extra)?)
        }
        _ => {
            let levels = super::launch_options::catalog()
                .into_iter()
                .find(|s| s.id == kind)
                .map(|s| {
                    s.effort_levels
                        .into_iter()
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let models = super::model_catalog::list_models(app, kind.as_str())?
                .into_iter()
                .map(|id| Model {
                    label: id.clone(),
                    id,
                    effort_levels: levels.clone(),
                })
                .collect();
            return Ok(Catalog {
                models,
                effort_levels: levels,
            });
        }
    }
    .map_err(|e| format!("Failed to serialize launch models: {e}"))?;
    let models: Vec<Model> =
        serde_json::from_value(value).map_err(|e| format!("Invalid model catalogue: {e}"))?;
    let mut effort_levels = Vec::new();
    for model in &models {
        for level in &model.effort_levels {
            if !effort_levels.contains(level) {
                effort_levels.push(level.clone());
            }
        }
    }
    Ok(Catalog {
        models,
        effort_levels,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> AppCtx {
        let dir = std::env::temp_dir().join(format!("vlx-launch-models-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let app = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(
            dir.clone(),
            db,
        )));
        let mut defaults = serde_json::Map::new();
        for kind in ["claude", "codex", "opencode", "pi", "omp"] {
            let path = dir.join(format!("{kind}.py"));
            std::fs::write(&path, include_str!("testdata/launch_catalog.py")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            defaults.insert(kind.into(), serde_json::json!({"path":path,"args":"--model inherited --thinking high --fixture-config retained"}));
        }
        repo::set_app_settings(
            &app.db().conn.lock().unwrap(),
            &std::collections::HashMap::from([(
                "vlx-settings".into(),
                serde_json::json!({"agentDefaults":defaults}).to_string(),
            )]),
        )
        .unwrap();
        app
    }

    #[test]
    fn launch_catalogues_preserve_native_efforts_provider_selectors_and_directory() {
        let app = fixture();
        let dir = app.data_dir().unwrap();
        let context = Context {
            cwd: Some(dir.to_string_lossy().into()),
            ..Default::default()
        };
        let codex = list(&app, SessionKind::Codex, &context).unwrap();
        assert_eq!(codex.models[0].id, "gpt-5.6-sol");
        assert_eq!(
            codex.models[0].effort_levels,
            ["low", "medium", "high", "xhigh", "max", "ultra"]
        );
        assert_eq!(codex.models[1].effort_levels, ["low"]);
        let claude = list(&app, SessionKind::Claude, &context).unwrap();
        assert!(claude.models.iter().any(|m| !m.effort_levels.is_empty()));
        // The fixture answers the CLI probe; the identifiers only it knows are appended to the catalogue.
        let ids: Vec<&str> = claude.models.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"claude-opus-4-8"), "the catalogue stays the base");
        let at = ids.iter().position(|id| *id == "fixture-claude-model").unwrap();
        assert_eq!(ids[at..at + 2], ["fixture-claude-model", "fixture-claude-model-b"]);
        assert_eq!(claude.models[at].label, "Fixture Claude");
        for kind in [SessionKind::Pi, SessionKind::Omp] {
            let result = list(&app, kind, &context).unwrap();
            assert_eq!(
                result
                    .models
                    .iter()
                    .map(|m| m.id.as_str())
                    .collect::<Vec<_>>(),
                ["provider-a/shared", "provider-b/shared"]
            );
            assert!(result.models[0].effort_levels.contains(
                &if kind == SessionKind::Pi {
                    "xhigh"
                } else {
                    "max"
                }
                .to_string()
            ));
            assert_eq!(result.models[1].effort_levels, ["off"]);
            let inherited = Context {
                inherit_args: true,
                cwd: context.cwd.clone(),
                ..Default::default()
            };
            list(&app, kind, &inherited).unwrap();
        }
        for _ in 0..2 {
            let opencode = list(&app, SessionKind::Opencode, &context).unwrap();
            assert_eq!(opencode.models.len(), 2);
            assert_eq!(opencode.models[0].id, "fixture/deep");
            assert_eq!(opencode.models[0].effort_levels, ["low", "high", "custom"]);
            assert!(opencode.models[1].effort_levels.is_empty());
        }
        let calls: Vec<Value> = std::fs::read_to_string(dir.join("calls.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let codex_call = calls.iter().find(|row| row["kind"] == "codex").unwrap();
        assert_eq!(
            std::path::Path::new(codex_call["cwd"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            dir.canonicalize().unwrap()
        );
        for kind in ["pi", "omp"] {
            let rows: Vec<_> = calls.iter().filter(|row| row["kind"] == kind).collect();
            assert_eq!(rows.len(), 2);
            assert!(rows
                .iter()
                .all(|row| std::path::Path::new(row["cwd"].as_str().unwrap())
                    .canonicalize()
                    .unwrap()
                    == dir.canonicalize().unwrap()));
            assert!(!rows[0]["args"].to_string().contains("fixture-config"));
            assert!(rows[1]["args"].to_string().contains("fixture-config"));
            assert!(!rows[1]["args"].to_string().contains("inherited"));
        }
        let ports: Vec<_> = calls
            .iter()
            .filter(|row| row["kind"] == "opencode")
            .map(|row| {
                let args = row["args"].as_array().unwrap();
                args[args.iter().position(|arg| arg == "--port").unwrap() + 1]
                    .as_str()
                    .unwrap()
                    .parse::<u16>()
                    .unwrap()
            })
            .collect();
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0], ports[1]);
        assert!((10000..=49151).contains(&ports[0]));
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
