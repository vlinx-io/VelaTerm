//! Public links expose agent conversations, never the generic remote terminal interface. The host's own
//! account may additionally open the live terminal of a terminal-view agent inside its share, as it can on
//! the host; see `owner_terminal_session`. Plain shells and browser tabs stay out of every share.
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

    /// Whether a session inside the scope is offered to the visitor. Conversation sessions are, and so are
    /// terminal-view agents whose kind has a conversation view: the visitor reads their record and may be
    /// asked to switch them. Other agents have no record to read, so only the owner, who can open their
    /// terminal, sees them.
    fn lists(&self, session: &Session, owner: bool) -> bool {
        self.contains(session) && (session.engine == "chat" || session.kind.supports_chat() || owner)
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
            let owner = super::public_relay::visitor_is_owner(app, self);
            let visible: Vec<Value> = sessions.iter().filter(|s| self.lists(s, owner)).map(|s| {
                json!({"id":s.id,"name":s.name,"kind":s.kind,"engine":s.engine,
                    "projectId":s.project_id,"projectName":projects.iter().find(|p|p.id == s.project_id).map(|p|p.name.as_str()),
                    "canChat":s.engine == "chat" && s.kind.supports_chat()})
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
        if !readonly_history(cmd,args) && session.engine != "chat" {
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
        "get_git_status" | "get_git_branch_info" | "get_session_cwd" => return Ok(Value::Null),
        // Collapsing a row is a view choice: the page keeps it locally and the host's sidebar stays as it was.
        "set_app_settings" | "set_collapsed" => return Ok(Value::Null),
        "mirror_get" => {
            return Ok(json!({"enabled":false,"rev":0,"source":Value::Null,"state":Value::Null}))
        }
        "list_agent_presets" | "list_shells" => return Ok(json!([])),
        // Lets the shared shell decide whether to offer the view switch and the live terminal; both
        // re-check it on every call.
        "share_access" => {
            let owner = super::public_relay::visitor_is_owner(app, scope);
            return Ok(json!({"canSwitchEngine": owner, "canUseTerminal": owner}))
        }
        // Machine panels stay host-only; the shared session view has no use for them.
        "process_stats" | "system_stats" | "usage_refresh" | "model_catalog_refresh" | "list_dir"
        | "search_session_content" => return Err(unavailable()),
        _ => {}
    }
    // Keystrokes, size changes and redraws for a terminal the owner opened; see `owner_terminal_session`.
    if matches!(cmd, "pty_write" | "pty_resize" | "pty_redraw") {
        let sid = args
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or("Missing session ID")?;
        owner_terminal_session(app, scope, sid)?;
        return super::dispatch::dispatch(app, cmd, args, who, origin);
    }
    if !app_command(cmd) {
        return Err(unavailable());
    }
    // Read-only commands that carry no session scope are dispatched as-is.
    if matches!(
        cmd,
        "get_app_settings"
            | "app_version"
            | "chat_capabilities"
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
    if cmd == "mobile_notification_preview"
        || (cmd == "session_mark_read" && scope.lists(&session, super::public_relay::visitor_is_owner(app, scope)))
    {
        return super::dispatch::dispatch(app, cmd, args, who, origin);
    }
    // Only the host's own account may move a terminal-view agent into the conversation view, and only in
    // that direction. The switch stops the terminal agent and changes the view on the host as well.
    if cmd == "set_session_engine" {
        if !super::public_relay::visitor_is_owner(app, scope) || !session.kind.supports_chat() {
            return Err(unavailable());
        }
        if args.get("engine").and_then(Value::as_str) != Some("chat") {
            return Err(unavailable());
        }
        return super::dispatch::dispatch(app, cmd, &json!({"sessionId":sid,"engine":"chat"}), who, origin);
    }
    if session.engine != "chat" && !readonly_history(cmd,args) {
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

/// The session whose live terminal a share connection may open, attach to and type into. Only the host's
/// own account qualifies, and only for terminal-view agents inside the scope; `contains` already keeps plain
/// shells and browser tabs out. The session is returned so the caller launches it from host records rather
/// than from client-supplied paths or commands.
pub fn owner_terminal_session(
    app: &AppCtx,
    scope: &ShareScope,
    session_id: &str,
) -> Result<Session, String> {
    if !super::public_relay::visitor_is_owner(app, scope) {
        return Err(unavailable());
    }
    let session = {
        let conn = app.db().conn.lock().map_err(|_| "Database unavailable")?;
        repo::get_session(&conn, session_id)?.ok_or("Session unavailable")?
    };
    if !scope.contains(&session) || session.engine == "chat" {
        return Err(unavailable());
    }
    Ok(session)
}

fn readonly_history(cmd:&str,args:&Value) -> bool {
    matches!(cmd,"read_agent_chat"|"chat_detail_read"|"chat_detail_download"|"chat_locate"|"chat_search"|"chat_read_cancel")
        || (cmd=="chat_snapshot" && args["historyOnly"]==true)
        || (cmd=="chat_row" && args["protocolVersion"]==2)
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
/// session (rewind, clear, MCP, agent auth, model changes) are intentionally absent. The engine switch is
/// listed only so `dispatch_shared` can admit the owner's one-way move into the conversation view.
fn app_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "get_app_settings"
            | "app_version"
            | "diagnostic_event"
            | "usage_snapshot"
            | "agent_permission_catalog"
            | "list_tree"
            | "set_session_engine"
            | "session_states"
            | "session_permission_state"
            | "session_mark_read"
            | "mobile_notification_preview"
            | "chat_snapshot"
            | "chat_capabilities"
            | "chat_attach"
            | "chat_detach"
            | "chat_models"
            | "agent_version"
            | "chat_commands"
            | "chat_row"
            | "chat_detail_read"
            | "chat_detail_download"
            | "chat_locate"
            | "chat_search"
            | "chat_read_cancel"
            | "chat_attachment"
            | "read_agent_chat"
            | "chat_start"
            | "chat_send"
            | "chat_interrupt"
            | "chat_auto_continue_cancel"
            | "chat_permission"
            | "chat_queue_steer"
            | "chat_queue_read"
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
    // Terminal-view agents appear only when they have a conversation view; see `ShareScope::lists`.
    let owner = super::public_relay::visitor_is_owner(app, scope);
    let allowed: Vec<&Session> = tree.sessions.iter().filter(|s| scope.lists(s, owner)).collect();
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
        value["collectionId"] = Value::Null;
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
    let owner = super::public_relay::visitor_is_owner(app, scope);
    let allowed: std::collections::HashSet<&str> = tree
        .sessions
        .iter()
        .filter(|s| scope.lists(s, owner))
        .map(|s| s.id.as_str())
        .collect();
    let states: std::collections::HashMap<String, crate::session_state::SessionState> =
        crate::command_core::session_states(app)?;
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
            | "chat_detail_read"
            | "chat_detail_download"
            | "chat_locate"
            | "chat_search"
            | "chat_read_cancel"
            | "chat_start"
            | "chat_send"
            | "chat_interrupt"
            | "chat_permission"
            | "chat_queue_steer"
            | "chat_queue_read"
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
        assert_eq!(dispatch_shared(&app,&scope,"chat_queue_remove",&json!({"sessionId":other.id,"id":"msg-outside"}),"queue-test",origin).unwrap_err(),"Session is outside the shared scope");
        assert_eq!(scope.dispatch(&app,"chat_queue_remove",&json!({"sessionId":other.id,"id":"msg-outside"})).unwrap_err(),"Session is outside the shared scope");
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
        let collapsed = |app: &AppCtx| {
            let conn = app.db().conn.lock().unwrap();
            repo::list_tree(&conn).unwrap().sessions.iter().find(|s| s.id == session.id).unwrap().collapsed
        };
        let before = collapsed(&app);
        assert_eq!(
            dispatch_shared(&app, &scope, "set_collapsed", &json!({"kind": "session", "id": session.id, "collapsed": !before}), "ws-test", origin).unwrap(),
            Value::Null
        );
        assert_eq!(collapsed(&app), before);
        assert_eq!(
            dispatch_shared(&app, &scope, "get_session_cwd", &json!({"sessionId": session.id}), "ws-test", origin).unwrap(),
            Value::Null
        );
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Terminal-view agents that have a conversation view are listed with their state and readable record,
    /// but stay read-only; agents without a conversation view stay hidden. A visitor who is not the host's
    /// own account cannot switch the view.
    #[test]
    fn terminal_view_agents_are_listed_read_only() {
        use crate::host::HeadlessHost;
        let dir = std::env::temp_dir().join(format!("share-tui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let (project, tui, other) = {
            let conn = db.conn.lock().unwrap();
            let project = repo::create_virtual_project(&conn, "Shared").unwrap();
            let tui = repo::create_session(&conn, &project.id, None, "Claude TUI", SessionKind::Claude, None, None, None, None, None).unwrap();
            repo::set_session_engine(&conn, &tui.id, "tui").unwrap();
            let other = repo::create_session(&conn, &project.id, None, "Kiro", SessionKind::Kiro, None, None, None, None, None).unwrap();
            (project, tui, other)
        };
        let app = AppCtx::Headless(std::sync::Arc::new(HeadlessHost::new(dir.clone(), db)));
        let scope = ShareScope {
            scope: "project".into(),
            target_id: Some(project.id),
            push_authority: None,
        };
        let origin = crate::web::dispatch::CallOrigin::Remote;
        let tree = dispatch_shared(&app, &scope, "list_tree", &json!({}), "ws-test", origin).unwrap();
        let ids: Vec<&Value> = tree["sessions"].as_array().unwrap().iter().map(|s| &s["id"]).collect();
        assert_eq!(ids, vec![&json!(tui.id)]);
        let picker = scope.dispatch(&app, "shared_sessions", &json!({})).unwrap();
        assert_eq!(picker.as_array().unwrap().len(), 1);
        assert_eq!(picker[0]["id"], tui.id);
        assert!(dispatch_shared(&app, &scope, "read_agent_chat", &json!({"sessionId": tui.id}), "ws-test", origin).is_ok());
        assert!(dispatch_shared(&app, &scope, "session_mark_read", &json!({"sessionId": tui.id}), "ws-test", origin).is_ok());
        assert!(dispatch_shared(&app, &scope, "session_mark_read", &json!({"sessionId": other.id}), "ws-test", origin).is_err());
        assert!(dispatch_shared(&app, &scope, "chat_send", &json!({"sessionId": tui.id, "text": "hi"}), "ws-test", origin).is_err());
        assert_eq!(
            dispatch_shared(&app, &scope, "share_access", &json!({}), "ws-test", origin).unwrap(),
            json!({"canSwitchEngine": false, "canUseTerminal": false})
        );
        assert_eq!(
            dispatch_shared(&app, &scope, "set_session_engine", &json!({"sessionId": tui.id, "engine": "chat"}), "ws-test", origin).unwrap_err(),
            unavailable()
        );
        let conn = app.db().conn.lock().unwrap();
        assert_eq!(repo::get_session(&conn, &tui.id).unwrap().unwrap().engine, "tui");
        drop(conn);
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The host's own account may open the live terminal of a terminal-view agent through its share and sees
    /// agents that have no conversation view; an invited account still reads records only. Plain shells and
    /// conversation-view sessions never get a terminal, even for the owner.
    #[test]
    fn only_the_owner_opens_terminal_view_agents() {
        use crate::host::HeadlessHost;
        let dir = std::env::temp_dir().join(format!("share-owner-tui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let (project, tui, kiro, chat, shell) = {
            let conn = db.conn.lock().unwrap();
            let project = repo::create_virtual_project(&conn, "Shared").unwrap();
            let tui = repo::create_session(&conn, &project.id, None, "Claude TUI", SessionKind::Claude, None, None, None, None, None).unwrap();
            repo::set_session_engine(&conn, &tui.id, "tui").unwrap();
            let kiro = repo::create_session(&conn, &project.id, None, "Kiro", SessionKind::Kiro, None, None, None, None, None).unwrap();
            let chat = repo::create_session(&conn, &project.id, None, "Chat", SessionKind::Claude, None, None, None, None, None).unwrap();
            repo::set_session_engine(&conn, &chat.id, "chat").unwrap();
            let shell = repo::create_session(&conn, &project.id, None, "Shell", SessionKind::Terminal, None, None, None, None, None).unwrap();
            (project, tui, kiro, chat, shell)
        };
        let config = json!({"origin":"https://example.invalid","deviceId":"d","token":"t","shares":{"g":{
            "scope":"project","targetId":project.id,"secret":"s","passwordHash":null,
            "allowedAccounts":["owner","guest"]}}});
        std::fs::write(dir.join("vlx-public-sharing.json"), config.to_string()).unwrap();
        let app = AppCtx::Headless(std::sync::Arc::new(HeadlessHost::new(dir.clone(), db)));
        let visitor = |account: &str| ShareScope {
            scope: "project".into(),
            target_id: Some(project.id.clone()),
            push_authority: Some(("g".into(), account.into())),
        };
        let (owner, guest) = (visitor("owner"), visitor("guest"));
        let origin = crate::web::dispatch::CallOrigin::Remote;

        assert_eq!(
            dispatch_shared(&app, &owner, "share_access", &json!({}), "ws-test", origin).unwrap(),
            json!({"canSwitchEngine": true, "canUseTerminal": true})
        );
        assert!(owner_terminal_session(&app, &owner, &tui.id).is_ok());
        assert!(owner_terminal_session(&app, &owner, &kiro.id).is_ok());
        assert_eq!(owner_terminal_session(&app, &owner, &chat.id).unwrap_err(), unavailable());
        assert_eq!(owner_terminal_session(&app, &owner, &shell.id).unwrap_err(), unavailable());
        // Keystrokes pass the policy for the owner; the session is not running here, so the PTY layer answers.
        assert_ne!(
            dispatch_shared(&app, &owner, "pty_write", &json!({"sessionId": tui.id, "data": "x"}), "ws-test", origin).unwrap_err(),
            unavailable()
        );
        let tree = dispatch_shared(&app, &owner, "list_tree", &json!({}), "ws-test", origin).unwrap();
        let mut ids: Vec<String> = tree["sessions"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_string()).collect();
        ids.sort();
        let mut expected = vec![tui.id.clone(), kiro.id.clone(), chat.id.clone()];
        expected.sort();
        assert_eq!(ids, expected);

        assert_eq!(
            dispatch_shared(&app, &guest, "share_access", &json!({}), "ws-test", origin).unwrap(),
            json!({"canSwitchEngine": false, "canUseTerminal": false})
        );
        assert_eq!(owner_terminal_session(&app, &guest, &tui.id).unwrap_err(), unavailable());
        for cmd in ["pty_write", "pty_resize", "pty_redraw"] {
            assert_eq!(
                dispatch_shared(&app, &guest, cmd, &json!({"sessionId": tui.id, "data": "x", "cols": 80, "rows": 24}), "ws-test", origin).unwrap_err(),
                unavailable(),
                "{cmd} must stay closed to an invited account"
            );
        }
        let tree = dispatch_shared(&app, &guest, "list_tree", &json!({}), "ws-test", origin).unwrap();
        assert!(!tree["sessions"].as_array().unwrap().iter().any(|s| s["id"] == kiro.id));
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A share grant does not expose names of unshared parent collections or broaden access to their members.
    #[test]
    fn shared_tree_omits_unshared_collection_membership() {
        use crate::host::HeadlessHost;
        let dir = std::env::temp_dir().join(format!("share-collections-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let project = {
            let conn = db.conn.lock().unwrap();
            let project = repo::create_virtual_project(&conn, "Shared").unwrap();
            let collection = repo::create_virtual_project(&conn, "Client work").unwrap();
            repo::set_project_collection(&conn, &project.id, Some(collection.id.as_str())).unwrap();
            let session = repo::create_session(
                &conn,
                &project.id,
                None,
                "Conversation",
                SessionKind::Claude,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            repo::set_session_engine(&conn, &session.id, "chat").unwrap();
            project
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
        assert_eq!(tree["projects"][0]["name"], "Shared");
        assert_eq!(tree.get("folders"), None);
        assert!(tree["projects"][0]["collectionId"].is_null());
        assert!(dispatch_shared(
            &app,
            &scope,
            "set_project_collection",
            &json!({ "name": "x" }),
            "ws-test",
            origin
        )
        .is_err());
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
