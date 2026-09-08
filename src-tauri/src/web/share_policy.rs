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

    pub fn dispatch(&self, app: &AppCtx, cmd: &str, args: &Value) -> Result<Value, String> {
        if cmd == "shared_sessions" {
            let conn = app.db().conn.lock().map_err(|_| "Database unavailable")?;
            let sessions = repo::list_all_sessions(&conn)?;
            // Do not serialize Session directly: it contains paths, environment and launch arguments.
            return Ok(Value::Array(sessions.iter().filter(|s| self.contains(s)).map(|s| {
                json!({"id":s.id,"name":s.name,"kind":s.kind,"engine":s.engine,
                    "canChat":s.engine == "chat" && matches!(s.kind, SessionKind::Claude | SessionKind::Codex | SessionKind::Opencode)})
            }).collect()));
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
        };
        let visible = scope.dispatch(&app, "shared_sessions", &json!({})).unwrap();
        assert_eq!(visible.as_array().unwrap().len(), 1);
        assert_eq!(visible[0]["id"], session.id);
        assert!(visible[0].get("envJson").is_none());
        assert!(visible[0].get("agentArgs").is_none());
        assert!(visible[0].get("cwd").is_none());
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
        assert!(allowed_command("chat_send"));
        assert!(allowed_command("chat_snapshot"));
    }
}
