//! Permission choices shared by settings, chat controls, and terminal launches.
use crate::{db::repo, host::AppCtx, models::SessionKind};
use serde::Serialize;
use serde_json::{json, Value};

pub const CLAUDE_MODES: &[&str] = &[
    "plan",
    "default",
    "acceptEdits",
    "auto",
    "bypassPermissions",
];
pub fn modes(kind: SessionKind) -> &'static [&'static str] {
    match kind {
        SessionKind::Claude => CLAUDE_MODES,
        SessionKind::Codex => &["read-only", "auto", "full-access"],
        SessionKind::Zcode => &["plan", "build", "edit", "yolo"],
        SessionKind::Opencode | SessionKind::Antigravity => &["default", "bypassPermissions"],
        // OMP can ask for approval; its bypass maps to `--approval-mode=yolo`. Pi has no approval step at
        // all, so it exposes no modes and the composer shows no permission control for it.
        SessionKind::Omp => &["default", "bypassPermissions"],
        _ => &[],
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    modes: &'static [&'static str],
    selected: String,
}

pub fn catalog(agent: &str, stored: Option<&str>) -> Result<Catalog, String> {
    let kind = kind(agent)?;
    Ok(Catalog {
        modes: modes(kind),
        selected: normalize(kind, stored)?.into(),
    })
}

fn kind(agent: &str) -> Result<SessionKind, String> {
    match agent {
        "claude" => Ok(SessionKind::Claude),
        "codex" => Ok(SessionKind::Codex),
        "zcode" => Ok(SessionKind::Zcode),
        "opencode" => Ok(SessionKind::Opencode),
        "omp" => Ok(SessionKind::Omp),
        "antigravity" => Ok(SessionKind::Antigravity),
        _ => Err(format!("Unsupported permission agent: {agent}")),
    }
}

pub fn normalize(kind: SessionKind, stored: Option<&str>) -> Result<&str, String> {
    let mode = match (kind, stored.map(str::trim).unwrap_or("")) {
        (SessionKind::Zcode, "" | "default") => "build",
        (SessionKind::Codex, "" | "default") => "auto",
        (SessionKind::Codex, "skip") => "full-access",
        (_, "") => "default",
        (_, "skip") => "bypassPermissions",
        (_, value) => value,
    };
    if modes(kind).contains(&mode) {
        Ok(mode)
    } else {
        Err(format!(
            "{} does not support permission mode {mode}",
            kind.as_str()
        ))
    }
}

/// Validate explicit session choices while leaving other agents' existing permission protocols intact.
pub fn validate(kind: SessionKind, mode: Option<&str>) -> Result<(), String> {
    if !modes(kind).is_empty() {
        normalize(kind, mode)?;
    }
    Ok(())
}

/// The permission a session actually uses: its own choice, or the agent kind's global default when it has
/// none. Every launch path and the permission state read through this, so terminal and conversation views
/// never disagree about an unset value. Unreadable settings count as no default, which keeps asking.
pub fn effective(
    conn: &rusqlite::Connection,
    kind: SessionKind,
    stored: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(mode) = stored.map(str::trim).filter(|mode| !mode.is_empty()) {
        return Ok(Some(if kind == SessionKind::Claude && normalize(kind, Some(mode)).is_err() {
            "default".to_string()
        } else { mode.to_string() }));
    }
    let settings: Value = repo::get_app_settings(conn)?
        .get("vlx-settings")
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    Ok(settings["agentDefaults"][kind.as_str()]["permissionMode"]
        .as_str()
        .map(str::trim)
        .filter(|mode| !mode.is_empty())
        .map(|mode| if kind == SessionKind::Claude && normalize(kind, Some(mode)).is_err() {
            "default".to_string()
        } else { mode.to_string() }))
}

/// Patch only this agent's permission under the database lock, retaining paths, arguments, and other preferences.
pub fn set_default(app: &AppCtx, agent: &str, mode: &str) -> Result<Value, String> {
    let kind = kind(agent)?;
    let mode = normalize(kind, Some(mode))?;
    let stored = if matches!(mode, "bypassPermissions" | "full-access") {
        "skip"
    } else {
        mode
    };
    let defaults = {
        let conn = app.db().conn.lock().unwrap();
        let settings = repo::get_app_settings(&conn)?;
        let mut value: Value = match settings.get("vlx-settings") {
            Some(raw) => {
                serde_json::from_str(raw).map_err(|e| format!("Invalid saved settings: {e}"))?
            }
            None => json!({}),
        };
        if !value.is_object() {
            return Err("Invalid saved settings object".into());
        }
        if value.get("agentDefaults").is_none() {
            value["agentDefaults"] = json!({});
        }
        let defaults = value["agentDefaults"]
            .as_object_mut()
            .ok_or("Invalid agent defaults")?;
        let config = defaults.entry(agent).or_insert_with(|| json!({}));
        let config = config
            .as_object_mut()
            .ok_or("Invalid agent configuration")?;
        config.insert("permissionMode".into(), json!(stored));
        let defaults = value["agentDefaults"].clone();
        repo::set_app_settings(
            &conn,
            &std::collections::HashMap::from([("vlx-settings".into(), value.to_string())]),
        )?;
        defaults
    };
    app.emit(crate::host::SETTINGS_CHANGED, vec!["vlx-settings"]);
    Ok(defaults)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_catalog_preserves_modes_and_legacy_aliases() {
        for agent in ["claude", "codex", "opencode", "omp"] {
            let kind = kind(agent).unwrap();
            for mode in modes(kind) {
                assert_eq!(catalog(agent, Some(mode)).unwrap().selected, *mode);
            }
            let bypass = if agent == "codex" {
                "full-access"
            } else {
                "bypassPermissions"
            };
            assert_eq!(catalog(agent, Some("skip")).unwrap().selected, bypass);
        }
        assert!(catalog("opencode", Some("plan")).is_err());
        assert!(catalog("claude", Some("read-only")).is_err());
        assert!(catalog("terminal", None).is_err());
    }

    #[test]
    fn permission_defaults_patch_only_the_selected_agent_and_reject_invalid_modes() {
        let dir =
            std::env::temp_dir().join(format!("permission-defaults-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let app = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(
            dir.clone(),
            db,
        )));
        let initial = json!({"theme":"dark", "agentDefaults":{
            "claude":{"path":"/custom/claude", "args":"--model opus", "permissionMode":"skip"},
            "opencode":{"permissionMode":"skip"}
        }});
        repo::set_app_settings(
            &app.db().conn.lock().unwrap(),
            &std::collections::HashMap::from([("vlx-settings".into(), initial.to_string())]),
        )
        .unwrap();
        for mode in CLAUDE_MODES {
            let defaults = set_default(&app, "claude", mode).unwrap();
            let saved = defaults["claude"]["permissionMode"].as_str().unwrap();
            assert_eq!(normalize(SessionKind::Claude, Some(saved)).unwrap(), *mode);
            assert_eq!(defaults["claude"]["path"], "/custom/claude");
            assert_eq!(defaults["claude"]["args"], "--model opus");
            assert_eq!(defaults["opencode"]["permissionMode"], "skip");
        }
        set_default(&app, "opencode", "default").unwrap();
        let before = repo::get_app_settings(&app.db().conn.lock().unwrap()).unwrap();
        assert!(set_default(&app, "opencode", "auto").is_err());
        assert_eq!(
            repo::get_app_settings(&app.db().conn.lock().unwrap()).unwrap(),
            before
        );
        let saved: Value = serde_json::from_str(&before["vlx-settings"]).unwrap();
        assert_eq!(saved["theme"], "dark");
        assert_eq!(
            saved["agentDefaults"]["opencode"]["permissionMode"],
            "default"
        );
        {
            let conn = app.db().conn.lock().unwrap();
            let project = repo::create_virtual_project(&conn, "Permission validation").unwrap();
            let session = repo::create_session(
                &conn,
                &project.id,
                None,
                "OpenCode",
                SessionKind::Opencode,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            repo::update_session(
                &conn,
                &session.id,
                "OpenCode",
                None,
                None,
                None,
                None,
                Some("bypassPermissions"),
            )
            .unwrap();
            assert!(repo::update_session(
                &conn,
                &session.id,
                "OpenCode",
                None,
                None,
                None,
                None,
                Some("plan")
            )
            .is_err());
            assert_eq!(
                repo::get_permission_mode(&conn, &session.id)
                    .unwrap()
                    .as_deref(),
                Some("bypassPermissions")
            );
            assert!(repo::create_session_full(
                &conn,
                &project.id,
                None,
                "Invalid",
                SessionKind::Opencode,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("plan"),
                None,
                None,
                None,
                None
            )
            .is_err());
        }
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn permission_modes_survive_claude_terminal_and_opencode_config() {
        use crate::agent::{chat::protocol, inject};
        for (mode, flag) in [
            ("plan", "--permission-mode plan"),
            ("default", "--permission-mode default"),
            ("acceptEdits", "--permission-mode acceptEdits"),
            ("auto", "--permission-mode auto"),
            ("bypassPermissions", "--dangerously-skip-permissions"),
        ] {
            assert_eq!(
                inject::permission_flag(SessionKind::Claude, Some(mode)),
                Some(flag)
            );
            assert_eq!(protocol::cli_permission_mode(Some(mode)), Some(mode));
            assert_eq!(
                inject::merge_permission_flag(
                    SessionKind::Claude,
                    Some(mode),
                    Some("--model opus")
                )
                .unwrap(),
                format!("{flag} --model opus")
            );
        }
        for mode in ["skip", "bypassPermissions"] {
            let config: Value = serde_json::from_str(&inject::build_opencode_config_content(
                "/plugin",
                Some(mode),
            ))
            .unwrap();
            assert_eq!(config["permission"], "allow");
        }
        let config: Value = serde_json::from_str(&inject::build_opencode_config_content(
            "/plugin",
            Some("default"),
        ))
        .unwrap();
        assert!(config.get("permission").is_none());
    }

    #[test]
    fn legacy_unknown_claude_modes_read_as_default_but_new_writes_are_rejected() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();
        assert_eq!(effective(&conn, SessionKind::Claude, Some("unknown")).unwrap().as_deref(), Some("default"));
        assert!(validate(SessionKind::Claude, Some("unknown")).is_err());
        for mode in ["plan", "default", "acceptEdits", "auto", "skip", "unknown"] {
            repo::set_app_settings(&conn, &std::collections::HashMap::from([("vlx-settings".into(),
                json!({"agentDefaults":{"claude":{"permissionMode":mode}}}).to_string())])).unwrap();
            let expected = if mode == "unknown" { "default" } else { mode };
            for stored in [None, Some(""), Some(" ")] {
                assert_eq!(effective(&conn, SessionKind::Claude, stored).unwrap().as_deref(), Some(expected));
            }
            assert_eq!(effective(&conn, SessionKind::Claude, Some("acceptEdits")).unwrap().as_deref(), Some("acceptEdits"));
        }
    }

    #[test]
    fn unset_session_permissions_follow_the_agent_default() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();
        // Without any saved default an unset session stays unset, and the agent keeps asking.
        assert_eq!(effective(&conn, SessionKind::Claude, None).unwrap(), None);
        repo::set_app_settings(
            &conn,
            &std::collections::HashMap::from([("vlx-settings".into(), json!({"agentDefaults":{
                "claude":{"permissionMode":"skip"}, "cursor":{"permissionMode":"skip"}, "codex":{"permissionMode":" "}
            }}).to_string())]),
        )
        .unwrap();
        for stored in [None, Some(""), Some("  ")] {
            assert_eq!(effective(&conn, SessionKind::Claude, stored).unwrap().as_deref(), Some("skip"));
        }
        // An explicit choice wins, including an explicit "ask" on a checkbox-only agent.
        assert_eq!(effective(&conn, SessionKind::Claude, Some("plan")).unwrap().as_deref(), Some("plan"));
        assert_eq!(effective(&conn, SessionKind::Cursor, Some("default")).unwrap().as_deref(), Some("default"));
        assert_eq!(effective(&conn, SessionKind::Cursor, None).unwrap().as_deref(), Some("skip"));
        assert_eq!(effective(&conn, SessionKind::Codex, None).unwrap(), None);
        assert_eq!(effective(&conn, SessionKind::Opencode, None).unwrap(), None);
        assert_eq!(crate::agent::inject::permission_flag(SessionKind::Cursor, Some("default")), None);
    }
}
