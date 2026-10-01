//! Confirmed Claude permission changes that require a new protocol process.

use super::*;
use crate::agent::session_settings;
use crate::db::repo;

const REQUIRED: &str = "CHAT_PERMISSION_RESTART_REQUIRED:";

/// The signal the UI turns into the restart confirmation for this exact process.
pub(super) fn restart_required(proc: &ChatProcess) -> String {
    format!("{REQUIRED}{}", proc.pid)
}

/// Whether this process was launched with Claude's bypass capability.
///
/// The CLI only permits a runtime switch to `bypassPermissions` when the process started with
/// `--dangerously-skip-permissions` or `--allow-dangerously-skip-permissions`. Either can come from the
/// normalized permission mode or from the session's own launch arguments, so both are checked.
pub(super) fn claude_bypass_capable(permission_mode: Option<&str>, extra_args: &[String]) -> bool {
    protocol::cli_permission_mode(permission_mode) == Some("bypassPermissions")
        || extra_args.iter().any(|arg| {
            arg == "--dangerously-skip-permissions" || arg == "--allow-dangerously-skip-permissions"
        })
}

/// Whether a control-request failure is Claude refusing a runtime switch to `bypassPermissions`.
fn bypass_rejection(error: &str) -> bool {
    error.contains("Cannot set permission mode")
        && error.contains("was not launched with --dangerously-skip-permissions")
}

pub(super) fn mode_error(proc: &ChatProcess, mode: &str, error: String) -> String {
    if mode == "bypassPermissions" && bypass_rejection(&error) {
        restart_required(proc)
    } else {
        error
    }
}

impl ChatManager {
    pub(super) fn check_permission_restart(&self, session_id: &str) -> Result<(), String> {
        if self.permission_restarts.lock().unwrap().contains(session_id) {
            Err("CHAT_PERMISSION_RESTART_BUSY".into())
        } else {
            Ok(())
        }
    }

    /// The caller explicitly confirmed interruption of this exact process. No prompt is sent.
    pub fn restart_claude_bypass(&self, app: &AppCtx, session_id: &str, pid: u32) -> Result<(), String> {
        if !self.permission_restarts.lock().unwrap().insert(session_id.to_string()) {
            return Err("CHAT_PERMISSION_RESTART_BUSY".into());
        }
        let result = self.restart_claude_bypass_inner(app, session_id, pid);
        self.permission_restarts.lock().unwrap().remove(session_id);
        result
    }

    fn restart_claude_bypass_inner(&self, app: &AppCtx, session_id: &str, pid: u32) -> Result<(), String> {
        let previous = self.sessions.lock().unwrap().get(session_id).cloned()
            .ok_or("CHAT_PERMISSION_RESTART_STALE")?;
        let _action = previous.action.lock().unwrap();
        let _settings = previous.settings_change.lock().unwrap();
        if previous.kind != SessionKind::Claude || previous.pid != pid
            || !previous.alive.load(Ordering::Relaxed)
        {
            return Err("CHAT_PERMISSION_RESTART_STALE".into());
        }
        {
            let turn = previous.turn.lock().unwrap();
            // Queued prompts and background work need their own explicit disposition. Never discard them.
            if previous.shell_running.load(Ordering::Relaxed) || !turn.waiting.is_empty() || !turn.active_tasks.is_empty() || !turn.background_tasks.is_empty() {
                return Err("CHAT_PERMISSION_RESTART_TASKS".into());
            }
        }
        let (session, root) = {
            let conn = app.db().conn.lock().unwrap();
            let session = repo::get_session(&conn, session_id)?.ok_or("Session not found")?;
            let root = repo::get_project_root(&conn, &session.project_id)?;
            (session, root)
        };
        let confirmed = previous.agent_session_id.lock().unwrap().clone();
        let resume = confirmed.or(session.agent_session_id.clone());
        // A fork that has not recorded an id of its own is still only a branch point: the launch below forks
        // again (start_inner applies this same rule) and holds no id until its first turn.
        let fork = resume.is_some() && {
            let conn = app.db().conn.lock().unwrap();
            repo::get_fork_pending(&conn, session_id)?
        };
        if resume.is_none() && !previous.timeline.lock().unwrap().rows.is_empty() {
            return Err("CHAT_PERMISSION_RESTART_NO_HISTORY".into());
        }
        let bin = crate::agent::executable::for_session(app, &session);
        let cwd = previous.cwd.clone().or(root);
        let model = previous.model.lock().unwrap().clone();
        let effort = previous.effort.lock().unwrap().clone();
        let old_mode = previous.mode.lock().unwrap().clone();
        let fast_mode = previous.extras.lock().unwrap().fast_mode;
        let args = session_settings::without_selection_args(SessionKind::Claude, session.agent_args.as_deref());
        let mut extra_args = crate::agent::inject::split_extra_args(Some(&args));
        // Grant the capability only after confirmation. Start in the old mode until the CLI acknowledges
        // the new mode; conflicting custom flags and managed policies still produce a real rejection.
        extra_args.push("--allow-dangerously-skip-permissions".into());
        extra_args.push("--permission-mode".into());
        extra_args.push(old_mode.clone());

        previous.released.store(true, Ordering::Relaxed);
        {
            let mut child = previous.child.lock().unwrap();
            if let Err(error) = child.kill() {
                if child.try_wait().ok().flatten().is_none() {
                    previous.released.store(false, Ordering::Relaxed);
                    return Err(format!("Failed to stop the agent: {error}"));
                }
            }
            child.wait().map_err(|error| format!("Failed to wait for the agent: {error}"))?;
        }
        *previous.stdin.lock().unwrap() = None;
        previous.alive.store(false, Ordering::Relaxed);
        {
            let mut turn = previous.turn.lock().unwrap();
            turn.running = false;
            turn.started_at = None;
        }
        previous.permissions.lock().unwrap().clear();
        let mut rows = previous.timeline.lock().unwrap().rows.clone();
        settle_rows(&mut rows);
        previous.timeline.lock().unwrap().replace_all(rows.clone());
        emit(app, session_id, json!({"type":"turnCompleted"}));
        emit_state(app, session_id, AgentState::Waiting);

        let result = (|| {
            self.start_inner(app, session_id, SessionKind::Claude, cwd.as_deref(), &bin,
                resume.as_deref(), model.as_deref(), effort.as_deref(), Some(&old_mode),
                None, &extra_args, fast_mode)?;
            let next = self.sessions.lock().unwrap().get(session_id).cloned().ok_or("Agent did not start")?;
            next.timeline.lock().unwrap().replace_all(rows.clone());
            *next.user_targets.lock().unwrap() = previous.user_targets.lock().unwrap().clone();
            // Claude does not publish its native id until a turn starts; preserve it while idle.
            *next.agent_session_id.lock().unwrap() = if fork { None } else { resume };
            emit(app, session_id, reset_event(&next));
            next.request_and_wait("set_permission_mode", |id| {
                protocol::control_request(id, protocol::set_permission_mode("bypassPermissions"))
            })?;
            {
                let conn = app.db().conn.lock().unwrap();
                repo::set_permission_mode(&conn, session_id, "skip")?;
            }
            *next.mode.lock().unwrap() = "bypassPermissions".into();
            next.permission_confirmed.store(true, Ordering::Relaxed);
            app.emit(crate::host::TREE_CHANGED, ());
            emit(app, session_id, json!({"type":"settingsChanged","mode":"bypassPermissions"}));
            Ok(())
        })();
        if result.is_err() {
            // An ambiguous acknowledgement or failed database write must never leave bypass active.
            let retained = self.sessions.lock().unwrap().get(session_id).cloned().unwrap_or(previous.clone());
            retained.released.store(true, Ordering::Relaxed);
            retained.alive.store(false, Ordering::Relaxed);
            {
                let mut child = retained.child.lock().unwrap();
                let _ = child.kill();
                let _ = child.wait();
            }
            *retained.mode.lock().unwrap() = old_mode.clone();
            retained.timeline.lock().unwrap().replace_all(rows);
            // Keep the newest epoch: clients correctly reject resets from an older process.
            crate::session_state::set_alive(app, session_id, false);
            emit(app, session_id, reset_event(&retained));
            emit(app, session_id, json!({"type":"settingsChanged","mode":old_mode}));
            emit(app, session_id, json!({"type":"exited","code":0,"stderr":"","released":true}));
        }
        result
    }
}

fn settle_rows(rows: &mut [ChatRow]) {
    for row in rows {
        match row {
            ChatRow::Assistant { streaming, .. } | ChatRow::Reasoning { streaming, .. } => *streaming = false,
            ChatRow::Tool { status, children, .. } => {
                if *status == "running" { *status = "canceled"; }
                settle_rows(children);
            }
            _ => {}
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture(tag: &str) -> (AppCtx, std::path::PathBuf) {
        fixture_with(tag, &[])
    }

    fn fixture_with(tag: &str, extra_args: &[String]) -> (AppCtx, std::path::PathBuf) {
        let app = super::super::tests::ctx(tag);
        match &app {
          AppCtx::Headless(host) => {
            // The fixture never calls hooks; this endpoint does not open a listener.
            host.set_hooks(crate::agent::server::HookServer { port: 19191, token: "unused-fixture-token".into() });
          }
          #[cfg(feature = "gui")]
          _ => unreachable!(),
        }
        let dir = std::env::temp_dir().join(format!("vlx-permission-{tag}-{}-{}", std::process::id(), now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("claude-fixture");
        std::fs::write(&bin, r#"#!/usr/bin/env python3
import sys, json
for line in sys.stdin:
    request = json.loads(line)
    if request.get('type') != 'control_request':
        continue
    mode = request.get('request', {}).get('mode')
    error = None
    if mode == 'bypassPermissions':
        if '--allow-dangerously-skip-permissions' not in sys.argv:
            error = 'Cannot set permission mode to bypassPermissions: session was not launched with --dangerously-skip-permissions'
        elif '--reject-switch' in sys.argv:
            error = 'Policy rejected the permission change'
        elif '--resume' not in sys.argv or sys.argv[sys.argv.index('--resume') + 1] != 'native-history':
            error = 'Native history was not resumed'
    response = {'subtype': 'error' if error else 'success', 'request_id': request['request_id'], 'response': {'commands': []}}
    if error:
        response['error'] = error
    print(json.dumps({'type': 'control_response', 'response': response}), flush=True)
"#).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
        {
            let conn = app.db().conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,name,root_path,created_at) VALUES ('p','test',?1,0)", [dir.to_str().unwrap()]).unwrap();
            conn.execute("INSERT INTO sessions(id,project_id,name,kind,engine,permission_mode,agent_path,agent_session_id,created_at) VALUES ('s','p','test','claude','chat','default',?1,'native-history',0)", [bin.to_str().unwrap()]).unwrap();
        }
        app.chat().start(&app, "s", SessionKind::Claude, dir.to_str(), bin.to_str().unwrap(),
            Some("native-history"), None, None, Some("default"), None, extra_args, false).unwrap();
        let proc = app.chat().get("s").unwrap();
        proc.timeline.lock().unwrap().upsert(ChatRow::User {
            id: "preserved".into(), text: "Keep this conversation".into(), images: vec![], at: None,
        });
        (app, dir)
    }

    #[test]
    fn confirmed_restart_preserves_history_and_commits_only_after_acknowledgement() {
        let (app, dir) = fixture("restart-success");
        let previous = app.chat().get("s").unwrap();
        assert_eq!(crate::command_core::chat_set_mode(&app, "s", "bypassPermissions").unwrap_err(),
            format!("CHAT_PERMISSION_RESTART_REQUIRED:{}", previous.pid));
        assert_eq!(*previous.mode.lock().unwrap(), "default");
        let pending = serde_json::to_value(crate::agent::permission_state::read(&app, "s").unwrap()).unwrap();
        assert_eq!(pending["activation"], "restart");
        assert_eq!(pending["pending"], "bypassPermissions");
        assert_eq!(repo::get_session(&app.db().conn.lock().unwrap(), "s").unwrap().unwrap().permission_mode.as_deref(), Some("skip"));
        assert!(previous.alive.load(Ordering::Relaxed));
        {
            let mut turn = previous.turn.lock().unwrap();
            turn.running = true;
            turn.started_at = Some(now_ms());
        }
        previous.timeline.lock().unwrap().upsert(ChatRow::Assistant {
            id: "partial".into(), text: "Partial answer".into(), streaming: true,
            model: None, at: None, duration_ms: None,
        });
        app.chat().restart_claude_bypass(&app, "s", previous.pid).unwrap();
        let next = app.chat().get("s").unwrap();
        assert_ne!(next.pid, previous.pid);
        assert_eq!(*next.mode.lock().unwrap(), "bypassPermissions");
        let applied = serde_json::to_value(crate::agent::permission_state::read(&app, "s").unwrap()).unwrap();
        assert_eq!(applied["activation"], "applied");
        assert!(applied["pending"].is_null());
        assert_eq!(next.agent_session_id.lock().unwrap().as_deref(), Some("native-history"));
        assert!(next.timeline.lock().unwrap().get("preserved").is_some());
        assert!(matches!(next.timeline.lock().unwrap().get("partial"), Some(ChatRow::Assistant { streaming: false, text, .. }) if text == "Partial answer"));
        assert!(!next.turn.lock().unwrap().running);
        assert_eq!(repo::get_session(&app.db().conn.lock().unwrap(), "s").unwrap().unwrap().permission_mode.as_deref(), Some("skip"));
        app.chat().stop_for_handoff(&app, "s").unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A fork that has not recorded an id of its own restarts as a fork: the relaunch branches from the source
    /// again and the process still holds no id, so the source conversation is never continued in place.
    #[test]
    fn a_pending_fork_restarts_as_a_fork_without_an_id() {
        let (app, dir) = fixture("restart-fork");
        app.db().conn.lock().unwrap().execute("UPDATE sessions SET fork_pending=1 WHERE id='s'", []).unwrap();
        let previous = app.chat().get("s").unwrap();
        *previous.agent_session_id.lock().unwrap() = None;
        app.chat().restart_claude_bypass(&app, "s", previous.pid).unwrap();
        let next = app.chat().get("s").unwrap();
        assert_ne!(next.pid, previous.pid);
        assert_eq!(*next.mode.lock().unwrap(), "bypassPermissions");
        assert_eq!(next.agent_session_id.lock().unwrap().as_deref(), None, "a pending fork holds no id of its own");
        assert!(repo::get_fork_pending(&app.db().conn.lock().unwrap(), "s").unwrap(), "the fork is still pending");
        app.chat().stop_for_handoff(&app, "s").unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejected_restart_retains_history_and_old_permission_without_a_bypass_process() {
        let (app, dir) = fixture("restart-rejection");
        app.db().conn.lock().unwrap().execute("UPDATE sessions SET agent_args='--reject-switch' WHERE id='s'", []).unwrap();
        let previous = app.chat().get("s").unwrap();
        let error = app.chat().restart_claude_bypass(&app, "s", previous.pid).unwrap_err();
        assert!(error.contains("Policy rejected"), "{error}");
        assert!(!app.chat().is_alive("s"));
        let retained = app.chat().get("s").unwrap();
        assert_eq!(*retained.mode.lock().unwrap(), "default");
        assert!(retained.timeline.lock().unwrap().get("preserved").is_some());
        assert_eq!(repo::get_session(&app.db().conn.lock().unwrap(), "s").unwrap().unwrap().permission_mode.as_deref(), Some("default"));
        app.chat().stop(&app, "s").unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stale_confirmation_and_background_work_do_not_stop_the_process() {
        let (app, dir) = fixture("restart-guard");
        let previous = app.chat().get("s").unwrap();
        assert_eq!(app.chat().restart_claude_bypass(&app, "s", previous.pid + 1).unwrap_err(), "CHAT_PERMISSION_RESTART_STALE");
        previous.turn.lock().unwrap().background_tasks.insert("task".into());
        assert_eq!(app.chat().restart_claude_bypass(&app, "s", previous.pid).unwrap_err(), "CHAT_PERMISSION_RESTART_TASKS");
        assert!(app.chat().is_alive("s"));
        assert_eq!(app.chat().get("s").unwrap().pid, previous.pid);
        app.chat().stop_for_handoff(&app, "s").unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bypass_capability_comes_from_the_mode_or_the_launch_arguments() {
        assert!(claude_bypass_capable(Some("bypassPermissions"), &[]));
        assert!(claude_bypass_capable(Some("skip"), &[]));
        assert!(claude_bypass_capable(None, &["--dangerously-skip-permissions".into()]));
        assert!(claude_bypass_capable(
            Some("plan"),
            &["--allow-dangerously-skip-permissions".into()]
        ));
        assert!(!claude_bypass_capable(Some("default"), &[]));
        assert!(!claude_bypass_capable(
            Some("auto"),
            &["--dangerously-bypass-approvals-and-sandbox".into()]
        ));
    }

    #[test]
    fn a_process_launched_with_the_bypass_flag_switches_without_restart() {
        let (app, dir) = fixture_with("bypass-capable", &["--allow-dangerously-skip-permissions".into()]);
        let proc = app.chat().get("s").unwrap();
        assert!(proc.bypass_capable);
        crate::command_core::chat_set_mode(&app, "s", "bypassPermissions").unwrap();
        assert_eq!(*proc.mode.lock().unwrap(), "bypassPermissions");
        // The same process adopted the mode; no restart was offered or needed.
        assert_eq!(app.chat().get("s").unwrap().pid, proc.pid);
        app.chat().stop(&app, "s").unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
