//! Public links expose agent conversations, never the generic remote terminal interface.
//! AI tools retain the host user's permissions; this policy is not an operating-system sandbox.

use crate::db::repo;
use crate::host::AppCtx;
use crate::models::{Session, SessionKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareScope {
    pub scope: String,
    pub target_id: Option<String>,
    /// Tunnel-authenticated identity, not part of persisted or client-supplied share configuration.
    #[serde(skip)]
    pub push_authority: Option<(String, String)>,
}

impl ShareScope {
    fn contains(&self, session: &Session) -> bool {
        if matches!(session.kind, SessionKind::Terminal | SessionKind::Browser) {
            return false;
        }
        match self.scope.as_str() {
            "machine" => self.target_id.is_none(),
            "project" => self.target_id.as_deref() == Some(session.project_id.as_str()),
            "session" => self.target_id.as_deref() == Some(session.id.as_str()),
            _ => false,
        }
    }

    /// Whether a session exists on the host and falls inside this scope. Used before registering per-session
    /// event forwarding for a share connection, so no session outside the grant can push events to it.
    pub fn covers_session(&self, app: &AppCtx, session_id: &str) -> bool {
        let Ok(conn) = app.db().conn.lock() else {
            return false;
        };
        repo::get_session(&conn, session_id)
            .ok()
            .flatten()
            .is_some_and(|session| self.contains(&session))
    }

    pub fn dispatch(&self, app: &AppCtx, cmd: &str, args: &Value) -> Result<Value, String> {
        if cmd == "shared_sessions" || cmd == "shared_catalog" {
            let conn = app.db().conn.lock().map_err(|_| "Database unavailable")?;
            let sessions = repo::list_all_sessions(&conn)?;
            let projects = repo::list_tree(&conn)?.projects;
            // Do not serialize Session directly: it contains paths, environment and launch arguments.
            let visible: Vec<Value> = sessions.iter().filter(|s| self.contains(s)).map(|s| {
                json!({"id":s.id,"name":s.name,"kind":s.kind,"engine":s.engine,
                    "projectId":s.project_id,"projectName":projects.iter().find(|p|p.id == s.project_id).map(|p|p.name.as_str()),
                    "canChat":s.engine == "chat" && matches!(s.kind, SessionKind::Claude | SessionKind::Codex | SessionKind::Opencode | SessionKind::Pi | SessionKind::Omp)})
            }).collect();
            if cmd == "shared_sessions" {
                return Ok(json!(visible));
            }
            let permitted = projects
                .iter()
                .filter(|p| match self.scope.as_str() {
                    "machine" => self.target_id.is_none(),
                    "project" => self.target_id.as_deref() == Some(p.id.as_str()),
                    "session" => visible.iter().any(|s| s["projectId"] == p.id),
                    _ => false,
                })
                .map(|p| json!({"id":p.id,"name":p.name}))
                .collect::<Vec<_>>();
            return Ok(json!({"projects":permitted,"sessions":visible}));
        }
        if !allowed_command(cmd) {
            return Err("This interface is unavailable through a public share".into());
        }
        let sid = args
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or("Missing session ID")?;
        let session = {
            let conn = app.db().conn.lock().map_err(|_| "Database unavailable")?;
            repo::get_session(&conn, sid)?.ok_or("Session unavailable")?
        };
        if !self.contains(&session) {
            return Err("Session is outside the shared scope".into());
        }
        if cmd != "read_agent_chat" && session.engine != "chat" {
            return Err(
                "The host must switch this session to conversation view before continuing it"
                    .into(),
            );
        }
        // Images can contain filesystem references in some provider formats. New public-share messages
        // accept text; attachment reads use the existing session-owned opaque attachment IDs.
        if cmd == "chat_send"
            && args.get("images").is_some_and(|images| {
                !images.is_null() && images.as_array().is_none_or(|a| !a.is_empty())
            })
        {
            return Err("Image uploads are unavailable through a public share".into());
        }
        // Visitors continue the host's selected agent configuration. Tool approvals answer the
        // pending request without replacing its input or installing persistent permission rules.
        let safe_args = if cmd == "chat_start" {
            json!({"sessionId":sid})
        } else if cmd == "chat_permission" {
            let mut safe = json!({"sessionId":sid,"requestId":args["requestId"],"allow":args["allow"],"message":args["message"]});
            if args["allow"] == true && args["answers"].is_object() {
                let snapshot = app.chat().snapshot(sid);
                let pending = snapshot
                    .permissions
                    .iter()
                    .find(|p| p["id"] == args["requestId"])
                    .ok_or("Permission request is no longer pending")?;
                if pending["tool_name"] != "AskUserQuestion" {
                    return Err("This request does not accept question answers".into());
                }
                let questions = pending["input"]["questions"]
                    .as_array()
                    .ok_or("Invalid question request")?;
                let mut answers = serde_json::Map::new();
                for question in questions {
                    let title = question["question"].as_str().ok_or("Invalid question")?;
                    if let Some(answer) = args["answers"][title].as_str() {
                        if answer.len() > 16_384 {
                            return Err("Answer is too long".into());
                        }
                        answers.insert(title.into(), json!(answer));
                    }
                }
                let mut input = pending["input"].clone();
                input["answers"] = json!(answers);
                safe["updatedInput"] = input;
            }
            safe
        } else {
            args.clone()
        };
        super::dispatch::dispatch(
            app,
            cmd,
            &safe_args,
            "public-share",
            super::dispatch::CallOrigin::Remote,
        )
    }
}

/// Dispatches one command from a share-tunnel WebSocket connection. The scope was resolved from the
/// tunnel-injected grant headers at connection time and cannot be widened here. Commands the shared shell
/// needs but a visitor must not run return neutral values; everything outside the policy is denied.
pub fn dispatch_shared(
    app: &AppCtx,
    scope: &ShareScope,
    cmd: &str,
    args: &Value,
    who: &str,
    origin: super::dispatch::CallOrigin,
) -> Result<Value, String> {
    if cmd == "mobile_push_bind" {
        return crate::mobile_push::bind(app, Some(scope), args);
    }
    match cmd {
        // The tree is rebuilt rather than filtered in place: visitor sessions must never carry host paths,
        // environment, launch arguments, or worktree locations, and only sessions inside the scope appear.
        "list_tree" => return filtered_tree(app, scope),
        "session_states" => return filtered_session_states(app, scope),
        // Locations and host-side UI writes do not belong to a visitor. Neutral values keep the shared shell
        // rendering instead of surfacing errors for data it does not need.
        "get_git_status" | "get_session_cwd" => return Ok(Value::Null),
        "set_app_settings" => return Ok(Value::Null),
        "mirror_get" => {
            return Ok(json!({"enabled":false,"rev":0,"source":Value::Null,"state":Value::Null}))
        }
        "list_agent_presets" | "list_shells" => return Ok(json!([])),
        // Machine panels stay host-only; the shared session view has no use for them.
        "process_stats" | "system_stats" | "usage_refresh" | "model_catalog_refresh" | "list_dir"
        | "search_session_content" => return Err(unavailable()),
        _ => {}
    }
    if !app_command(cmd) {
        return Err(unavailable());
    }
    // Read-only commands that carry no session scope are dispatched as-is.
    if matches!(
        cmd,
        "get_app_settings"
            | "app_version"
            | "diagnostic_event"
            | "usage_snapshot"
            | "agent_permission_catalog"
    ) {
        return super::dispatch::dispatch(app, cmd, args, who, origin);
    }
    let sid = args
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or("Missing session ID")?;
    let session = {
        let conn = app.db().conn.lock().map_err(|_| "Database unavailable")?;
        repo::get_session(&conn, sid)?.ok_or("Session unavailable")?
    };
    if !scope.contains(&session) {
        return Err("Session is outside the shared scope".into());
    }
    if cmd == "mobile_notification_preview" {
        return super::dispatch::dispatch(app, cmd, args, who, origin);
    }
    if session.engine != "chat" {
        return Err(
            "The host must switch this session to conversation view before continuing it".into(),
        );
    }
    // Visitors may add text to a conversation but not upload images, which could reference host files.
    if cmd == "chat_send"
        && args.get("images").is_some_and(|images| {
            !images.is_null() && images.as_array().is_none_or(|a| !a.is_empty())
        })
    {
        return Err("Image uploads are unavailable through a public share".into());
    }
    let safe_args = if cmd == "chat_start" {
        json!({"sessionId":sid})
    } else if cmd == "chat_permission" {
        permission_args(app, sid, args)?
    } else {
        args.clone()
    };
    super::dispatch::dispatch(app, cmd, &safe_args, who, origin)
}

/// Rejects an image upload or a scope escape on a tool permission answer, and rebuilds question answers so
/// only fields the pending request actually declared are accepted.
fn permission_args(app: &AppCtx, sid: &str, args: &Value) -> Result<Value, String> {
    let mut safe = json!({"sessionId":sid,"requestId":args["requestId"],"allow":args["allow"],"message":args["message"]});
    if args["allow"] == true && args["answers"].is_object() {
        let snapshot = app.chat().snapshot(sid);
        let pending = snapshot
            .permissions
            .iter()
            .find(|p| p["id"] == args["requestId"])
            .ok_or("Permission request is no longer pending")?;
        if pending["tool_name"] != "AskUserQuestion" {
            return Err("This request does not accept question answers".into());
        }
        let questions = pending["input"]["questions"]
            .as_array()
            .ok_or("Invalid question request")?;
        let mut answers = serde_json::Map::new();
        for question in questions {
            let title = question["question"].as_str().ok_or("Invalid question")?;
            if let Some(answer) = args["answers"][title].as_str() {
                if answer.len() > 16_384 {
                    return Err("Answer is too long".into());
                }
                answers.insert(title.into(), json!(answer));
            }
        }
        let mut input = pending["input"].clone();
        input["answers"] = json!(answers);
        safe["updatedInput"] = input;
    }
    Ok(safe)
}

/// Commands the shared in-app session view may call. Everything else is denied; the visitor gets exactly the
/// conversation surface the product promises. Interactive commands that would reconfigure or end the host
/// session (rewind, clear, engine switch, MCP, agent auth, model changes) are intentionally absent.
fn app_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "get_app_settings"
            | "app_version"
            | "diagnostic_event"
            | "usage_snapshot"
            | "agent_permission_catalog"
            | "list_tree"
            | "session_states"
            | "session_permission_state"
            | "session_mark_read"
            | "mobile_notification_preview"
            | "chat_snapshot"
            | "chat_attach"
            | "chat_detach"
            | "chat_models"
            | "chat_commands"
            | "chat_row"
            | "chat_attachment"
            | "read_agent_chat"
            | "chat_start"
            | "chat_send"
            | "chat_interrupt"
            | "chat_auto_continue_cancel"
            | "chat_permission"
            | "chat_queue_steer"
            | "chat_queue_remove"
            | "chat_queue_update"
            | "chat_background_tasks"
    )
}

/// Builds the visitor's tree from the host tree: only sessions inside the scope, keeping exactly the
/// ancestors needed to place them. Sensitive fields (paths, environment, launch arguments, worktrees) are
/// blanked rather than removed because the frontend tree renderer dereferences some of them.
fn filtered_tree(app: &AppCtx, scope: &ShareScope) -> Result<Value, String> {
    let tree = crate::command_core::list_tree(app)?;
    // Only conversation sessions are usable through a share: a terminal-engine agent would open into a
    // terminal view whose PTY commands are denied, so it never appears in the visitor's tree.
    let allowed: Vec<&Session> = tree
        .sessions
        .iter()
        .filter(|s| scope.contains(s) && s.engine == "chat")
        .collect();
    let project_ids: std::collections::HashSet<&str> =
        allowed.iter().map(|s| s.project_id.as_str()).collect();
    let group_by_id: std::collections::HashMap<&str, &crate::models::Group> =
        tree.groups.iter().map(|g| (g.id.as_str(), g)).collect();
    let mut group_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for session in &allowed {
        let mut current = session.group_id.as_deref();
        while let Some(id) = current {
            if !group_ids.insert(id) {
                break;
            }
            current = group_by_id.get(id).and_then(|g| g.parent_group_id.as_deref());
        }
    }
    let mut projects = Vec::new();
    for project in tree.projects.iter().filter(|p| project_ids.contains(p.id.as_str())) {
        let mut value = serde_json::to_value(project).map_err(|_| "Cannot encode shared tree")?;
        // The sidebar renderer trims rootPath; the real host path must not reach the visitor.
        value["rootPath"] = json!("");
        projects.push(value);
    }
    let mut groups = Vec::new();
    for group in tree.groups.iter().filter(|g| group_ids.contains(g.id.as_str())) {
        let mut value = serde_json::to_value(group).map_err(|_| "Cannot encode shared tree")?;
        value["worktreePath"] = Value::Null;
        value["worktreeBaseRef"] = Value::Null;
        groups.push(value);
    }
    let mut sessions = Vec::new();
    for session in &allowed {
        let mut value = serde_json::to_value(session).map_err(|_| "Cannot encode shared tree")?;
        for key in [
            "cwd",
            "envJson",
            "initCmd",
            "agentArgs",
            "worktreePath",
            "worktreeBaseRef",
            "agentPath",
            "shell",
            "browserUrl",
        ] {
            value[key] = Value::Null;
        }
        sessions.push(value);
    }
    Ok(json!({"projects": projects, "groups": groups, "sessions": sessions}))
}

/// Filters the host state snapshot down to sessions inside the scope.
fn filtered_session_states(app: &AppCtx, scope: &ShareScope) -> Result<Value, String> {
    let tree = crate::command_core::list_tree(app)?;
    let allowed: std::collections::HashSet<&str> = tree
        .sessions
        .iter()
        .filter(|s| scope.contains(s) && s.engine == "chat")
        .map(|s| s.id.as_str())
        .collect();
    let states: std::collections::HashMap<String, crate::session_state::SessionState> =
        crate::session_state::snapshot();
    let filtered: std::collections::HashMap<&str, &crate::session_state::SessionState> = states
        .iter()
        .filter(|(id, _)| allowed.contains(id.as_str()))
        .map(|(id, state)| (id.as_str(), state))
        .collect();
    serde_json::to_value(filtered).map_err(|_| "Cannot encode shared state".into())
}

fn unavailable() -> String {
    "This interface is unavailable through a public share".into()
}

/// Commands the legacy frame-relay catalog and the host-side validation calls are allowed to run. Kept
/// separate from [`app_command`]: that list describes the shared in-app shell, this one the tiny catalog
/// surface the old link protocol used.
fn allowed_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "read_agent_chat"
            | "chat_snapshot"
            | "chat_attachment"
            | "chat_row"
            | "chat_start"
            | "chat_send"
            | "chat_interrupt"
            | "chat_permission"
            | "chat_queue_steer"
            | "chat_queue_remove"
            | "chat_queue_update"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_filters_conversations_and_never_serializes_launch_secrets() {
        use crate::host::HeadlessHost;
        let dir = std::env::temp_dir().join(format!("share-scope-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let (project, session, other) = {
            let conn = db.conn.lock().unwrap();
            let project = repo::create_virtual_project(&conn, "Shared").unwrap();
            let other_project = repo::create_virtual_project(&conn, "Private").unwrap();
            let session = repo::create_session(
                &conn,
                &project.id,
                None,
                "Visible",
                SessionKind::Claude,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let other = repo::create_session(
                &conn,
                &other_project.id,
                None,
                "Hidden",
                SessionKind::Codex,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            repo::create_session(
                &conn,
                &project.id,
                None,
                "Terminal",
                SessionKind::Terminal,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            (project, session, other)
        };
        let app = AppCtx::Headless(std::sync::Arc::new(HeadlessHost::new(dir.clone(), db)));
        let scope = ShareScope {
            scope: "project".into(),
            target_id: Some(project.id),
            push_authority: None,
        };
        let visible = scope.dispatch(&app, "shared_sessions", &json!({})).unwrap();
        assert_eq!(visible.as_array().unwrap().len(), 1);
        assert_eq!(visible[0]["id"], session.id);
        let catalog = scope.dispatch(&app, "shared_catalog", &json!({})).unwrap();
        assert_eq!(catalog["projects"].as_array().unwrap().len(), 1);
        assert_eq!(catalog["sessions"], visible);
        assert_eq!(catalog["projects"][0]["name"], "Shared");
        assert!(catalog["projects"][0].get("rootPath").is_none());
        assert!(visible[0].get("envJson").is_none());
        assert!(visible[0].get("agentArgs").is_none());
        assert!(visible[0].get("cwd").is_none());
        let origin = crate::web::dispatch::CallOrigin::Remote;
        assert!(dispatch_shared(&app, &scope, "mobile_notification_preview", &json!({"sessionId":other.id}), "notification-test", origin).is_err());
        let preview = dispatch_shared(&app, &scope, "mobile_notification_preview", &json!({"sessionId":session.id}), "notification-test", origin).unwrap();
        assert_eq!(preview, json!({"title":"Visible","body":""}));
        assert!(scope
            .dispatch(&app, "read_agent_chat", &json!({"sessionId":other.id}))
            .is_err());
        assert_eq!(
            scope
                .dispatch(&app, "read_agent_chat", &json!({"sessionId":session.id}))
                .unwrap(),
            json!([])
        );
        let single = ShareScope {
            scope: "session".into(),
            target_id: Some(other.id.clone()),
            push_authority: None,
        };
        assert!(!single.contains(&session));
        assert!(single.contains(&other));
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn terminal_files_settings_and_unknown_commands_are_denied() {
        for cmd in [
            "pty_write",
            "pty_resize",
            "pty_kill",
            "pty-spawn",
            "read_text_file",
            "write_text_file",
            "read_file_base64",
            "list_tree",
            "update_session",
            "create_session",
            "web_pairing_create",
            "gitea_set_config",
            "chat_mcp_toggle",
            "future_command",
        ] {
            assert!(!allowed_command(cmd), "Public share must deny {cmd}");
        }
        // Shell mode is the capability pty_spawn grants; a public share visitor has neither, on either
        // surface.
        for cmd in ["chat_run_shell", "chat_cancel_shell"] {
            assert!(!allowed_command(cmd), "Public share must deny {cmd}");
            assert!(!app_command(cmd), "Public share app shell must deny {cmd}");
        }
        assert!(allowed_command("chat_send"));
        assert!(allowed_command("chat_snapshot"));
    }
}

#[cfg(test)]
mod shared_surface_tests {
    use super::*;

    /// The shared application shell gets a filtered tree: only sessions inside the grant, no host paths or
    /// launch material, and no terminal entries. Everything outside the app command list stays denied.
    #[test]
    fn app_tree_is_filtered_sanitized_and_denies_terminal_commands() {
        use crate::host::HeadlessHost;
        let dir = std::env::temp_dir().join(format!("share-tree-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let (project, session, terminal) = {
            let conn = db.conn.lock().unwrap();
            let project = repo::create_virtual_project(&conn, "Shared").unwrap();
            let _private = repo::create_virtual_project(&conn, "Private").unwrap();
            let session = repo::create_session(
                &conn,
                &project.id,
                None,
                "Conversation",
                SessionKind::Claude,
                None,
                Some("/host/secret"),
                Some("rm -rf /"),
                None,
                None,
            )
            .unwrap();
            repo::set_session_engine(&conn, &session.id, "chat").unwrap();
            let terminal = repo::create_session(
                &conn,
                &project.id,
                None,
                "Terminal",
                SessionKind::Terminal,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            (project, session, terminal)
        };
        let app = AppCtx::Headless(std::sync::Arc::new(HeadlessHost::new(dir.clone(), db)));
        let scope = ShareScope {
            scope: "project".into(),
            target_id: Some(project.id),
            push_authority: None,
        };
        let origin = crate::web::dispatch::CallOrigin::Remote;
        let tree = dispatch_shared(&app, &scope, "list_tree", &json!({}), "ws-test", origin).unwrap();
        assert_eq!(tree["projects"].as_array().unwrap().len(), 1);
        assert_eq!(tree["projects"][0]["rootPath"], json!(""));
        let sessions = tree["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["id"], session.id);
        assert!(sessions[0]["cwd"].is_null());
        assert!(sessions[0]["initCmd"].is_null());
        assert!(!sessions.iter().any(|s| s["id"] == terminal.id));
        // Session-scoped commands reject sessions outside the grant before touching the engine.
        assert!(dispatch_shared(&app, &scope, "chat_models", &json!({"sessionId": terminal.id}), "ws-test", origin).is_err());
        assert!(scope.covers_session(&app, &session.id));
        assert!(!scope.covers_session(&app, &terminal.id));
        // Terminal, file, management, and unknown commands never run for a share connection.
        for cmd in [
            "pty_write",
            "pty_resize",
            "pty-spawn",
            "read_text_file",
            "write_file_chunk",
            "create_session",
            "set_session_engine",
            "chat_rewind",
            "chat_clear",
            "web_pairing_create",
            "future_command",
        ] {
            assert!(
                dispatch_shared(&app, &scope, cmd, &json!({"sessionId": session.id}), "ws-test", origin)
                    .is_err(),
                "{cmd} must be denied on the shared surface"
            );
        }
        // Host-side UI writes are neutralized rather than run.
        assert_eq!(
            dispatch_shared(&app, &scope, "set_app_settings", &json!({"key": "x", "value": "y"}), "ws-test", origin).unwrap(),
            Value::Null
        );
        assert_eq!(
            dispatch_shared(&app, &scope, "get_session_cwd", &json!({"sessionId": session.id}), "ws-test", origin).unwrap(),
            Value::Null
        );
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
