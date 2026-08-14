//! Dispatch browser and Electron WebSocket `invoke(cmd, args)` calls to the same logic as Tauri commands.
//!
//! Stateful, side-effecting, and event-emitting commands are orchestrated in `command_core`; this module
//! only extracts arguments, calls core, and returns. `commands.rs` shares that logic. Pure stateless
//! commands such as `git::*` and `files::*` call their underlying modules directly.
//!
//! PTY startup is special because it registers a WebSocket output subscriber, so `ws.rs` handles it.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::Value;

use crate::command_core as core;
use crate::files;
use crate::git;
use crate::host::AppCtx;
use crate::models::{NodeKind, SessionKind};

/// Dispatch one command and return a JSON value ready for frontend serialization.
///
/// `source` identifies the originating WebSocket connection for PTY resize-owner arbitration. Desktop
/// commands always use desktop, and input no longer participates in arbitration.
pub fn dispatch(app: &AppCtx, cmd: &str, args: &Value, source: &str) -> Result<Value, String> {
    match cmd {
        // ── PTY control (`pty_spawn` is handled in ws.rs) ──
        "pty_write" => {
            // Input does not participate in resize-owner arbitration and needs no source identifier.
            app.pty()
                .write(&req_str(args, "sessionId")?, &req_str(args, "data")?)?;
            Ok(Value::Null)
        }
        "pty_resize" => {
            // takeover explicitly claims ownership through Fit This Window; normal resize is arbitrated.
            app.pty().resize(
                app,
                &req_str(args, "sessionId")?,
                req_u16(args, "cols")?,
                req_u16(args, "rows")?,
                source,
                args.get("takeover")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )?;
            Ok(Value::Null)
        }
        "pty_kill" => {
            app.pty().kill(&req_str(args, "sessionId")?)?;
            Ok(Value::Null)
        }
        "pty_redraw" => {
            // Force a full-screen TUI redraw by pulsing SIGWINCH without changing size or ownership.
            app.pty().nudge_redraw(&req_str(args, "sessionId")?);
            Ok(Value::Null)
        }
        "get_session_cwd" => to_value(app.pty().cwd(&req_str(args, "sessionId")?)),
        "list_shells" => {
            let data_dir = app.data_dir().ok();
            to_value(crate::pty::manager::available_shells(data_dir.as_deref()))
        }
        // Git Bash status was previously desktop-only, causing every browser/remote mount probe to return
        // Unknown command. Register it here: query actual status on Windows and report unavailable elsewhere.
        "gitbash_status" => {
            // Do not reuse commands::GitbashStatus because the commands module is GUI-gated and absent
            // from no-default-features server builds. Construct the matching camelCase response directly.
            #[cfg(windows)]
            {
                let dd = app.data_dir().ok();
                let available = dd
                    .as_deref()
                    .map(|d| crate::agent::gitbash::default_bash(d).is_some())
                    .unwrap_or(false);
                let full_installed = dd
                    .as_deref()
                    .map(crate::agent::gitbash::full_installed)
                    .unwrap_or(false);
                Ok(serde_json::json!({ "available": available, "fullInstalled": full_installed }))
            }
            #[cfg(not(windows))]
            {
                Ok(serde_json::json!({ "available": false, "fullInstalled": false }))
            }
        }
        "agent_install_recipe" => to_value(crate::agent::install::install_recipe(&req_str(
            args, "agent",
        )?)),
        // Installation-path discovery performs filesystem I/O and an npm-prefix subprocess. Dispatch
        // already runs in desktop_call's blocking pool or a WebSocket worker, never the main thread.
        "agent_locate_bin" => to_value(crate::agent::install::locate_installed_bin(&req_str(
            args, "agent",
        )?)),

        // ── Tree management orchestrated by command_core ──
        "list_tree" => to_value(core::list_tree(app)?),
        "import_project" => to_value(core::import_project(app, &req_str(args, "rootPath")?)?),
        "clone_project" => {
            // New clients supply operationId for progress filtering and cancellation; generate one for old clients.
            let operation_id =
                opt_str(args, "operationId").unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            to_value(core::clone_project(
                app,
                &req_str(args, "url")?,
                &req_str(args, "parentDir")?,
                opt_str(args, "folderName").as_deref(),
                opt_str(args, "branch").as_deref(),
                &operation_id,
                source,
            )?)
        }
        "cancel_clone_project" => {
            to_value(git::cancel_clone(&req_str(args, "operationId")?, source)?)
        }
        "create_group" => to_value(core::create_group(
            app,
            &req_str(args, "projectId")?,
            opt_str(args, "parentGroupId").as_deref(),
            &req_str(args, "name")?,
            opt_str(args, "worktreePath").as_deref(),
            opt_str(args, "worktreeBaseRef").as_deref(),
        )?),
        "create_session" => to_value(core::create_session(
            app,
            &req_str(args, "projectId")?,
            opt_str(args, "groupId").as_deref(),
            &req_str(args, "name")?,
            req_kind::<SessionKind>(args, "kind")?,
            opt_str(args, "shell").as_deref(),
            opt_str(args, "cwd").as_deref(),
            opt_str(args, "initCmd").as_deref(),
            opt_str(args, "parentSessionId").as_deref(),
            opt_str(args, "worktreePath").as_deref(),
            opt_str(args, "agentArgs").as_deref(),
            opt_str(args, "permissionMode").as_deref(),
            opt_str(args, "agentSessionId").as_deref(),
            opt_str(args, "worktreeBaseRef").as_deref(),
            opt_str(args, "model").as_deref(),
            opt_str(args, "effort").as_deref(),
        )?),
        "persist_session" => to_value(core::persist_session(
            app,
            &req_str(args, "id")?,
            &req_str(args, "projectId")?,
            opt_str(args, "groupId").as_deref(),
            &req_str(args, "name")?,
            req_kind::<SessionKind>(args, "kind")?,
            opt_str(args, "shell").as_deref(),
            opt_str(args, "cwd").as_deref(),
            opt_str(args, "initCmd").as_deref(),
            opt_str(args, "parentSessionId").as_deref(),
        )?),
        "fork_session" => to_value(core::fork_session(app, &req_str(args, "sessionId")?)?),
        // Deliver a spawn outcome to a parked `vagent spawn` request; false when it already timed out.
        "spawn_result" => to_value(crate::agent::ctl::resolve_spawn(
            &req_str(args, "requestId")?,
            crate::agent::ctl::SpawnOutcome {
                session_id: opt_str(args, "sessionId"),
                error: opt_str(args, "error"),
                worktree_error: opt_str(args, "worktreeError"),
                awaiting_confirmation: args
                    .get("awaitingConfirmation")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
        )),
        "update_session" => {
            core::update_session(
                app,
                &req_str(args, "id")?,
                &req_str(args, "name")?,
                opt_str(args, "shell").as_deref(),
                opt_str(args, "cwd").as_deref(),
                opt_str(args, "initCmd").as_deref(),
                opt_str(args, "agentArgs").as_deref(),
                opt_str(args, "permissionMode").as_deref(),
            )?;
            Ok(Value::Null)
        }
        "rename_node" => {
            core::rename_node(
                app,
                req_kind::<NodeKind>(args, "kind")?,
                &req_str(args, "id")?,
                &req_str(args, "name")?,
            )?;
            Ok(Value::Null)
        }
        // Set or clear the sidebar emoji marker. An absent/empty mark clears it.
        "set_node_mark" => {
            core::set_node_mark(
                app,
                req_kind::<NodeKind>(args, "kind")?,
                &req_str(args, "id")?,
                opt_str(args, "mark").as_deref(),
            )?;
            Ok(Value::Null)
        }
        "clear_node_worktree" => {
            core::clear_node_worktree(
                app,
                req_kind::<NodeKind>(args, "kind")?,
                &req_str(args, "id")?,
            )?;
            Ok(Value::Null)
        }
        "delete_node" => {
            core::delete_node(
                app,
                req_kind::<NodeKind>(args, "kind")?,
                &req_str(args, "id")?,
            )?;
            Ok(Value::Null)
        }
        "move_node" => {
            core::move_node(
                app,
                req_kind::<NodeKind>(args, "kind")?,
                &req_str(args, "id")?,
                opt_str(args, "targetProjectId").as_deref(),
                opt_str(args, "targetGroupId").as_deref(),
                opt_str(args, "targetParentSessionId").as_deref(),
                req_i64(args, "sortOrder")?,
            )?;
            Ok(Value::Null)
        }
        "set_collapsed" => {
            core::set_collapsed(
                app,
                req_kind::<NodeKind>(args, "kind")?,
                &req_str(args, "id")?,
                req_bool(args, "collapsed")?,
            )?;
            Ok(Value::Null)
        }
        "set_session_archived" => {
            core::set_session_archived(app, &req_str(args, "id")?, req_bool(args, "archived")?)?;
            Ok(Value::Null)
        }
        "archive_group" => {
            core::archive_group(app, &req_str(args, "id")?)?;
            Ok(Value::Null)
        }
        "list_archived_sessions" => to_value(core::list_archived_sessions(app)?),
        // Persist a browser node's latest URL. Electron renders with main-process WebContentsView, but
        // persistence belongs in the sidecar, so Electron and remote clients dispatch here without a
        // tree://changed broadcast. Tauri uses the matching commands.rs entry into the same core logic.
        "set_browser_url" => {
            core::set_browser_url(app, &req_str(args, "id")?, &req_str(args, "url")?)?;
            Ok(Value::Null)
        }
        // Cross-shell application preferences: Electron reads and writes shared app_settings via sidecar.
        "get_app_settings" => to_value(core::get_app_settings(app)?),
        // Remote and Electron clients compare backend CARGO_PKG_VERSION with __APP_VERSION__ at startup
        // and report frontend/backend build drift. This shares Tauri's app_version source.
        "app_version" => to_value(env!("CARGO_PKG_VERSION")),
        "set_app_settings" => {
            let entries = args
                .get("entries")
                .and_then(|v| {
                    serde_json::from_value::<std::collections::HashMap<String, String>>(v.clone())
                        .ok()
                })
                .unwrap_or_default();
            core::set_app_settings(app, entries)?;
            Ok(Value::Null)
        }
        // Clean Now removes temporary pasted images from vlx-uploads and returns removed/freedBytes.
        // Dispatch keeps desktop_call and remote/Electron behavior aligned; cleanup needs no AppCtx.
        "clean_pasted_images" => Ok(core::clean_pasted_images()),
        // Global session-content search is available remotely. Conversation hits can navigate on all
        // clients, while recording hits show snippets and desktop alone can seek replay.
        "search_session_content" => to_value(core::search_session_content(
            app,
            &req_str(args, "query")?,
            opt_str(args, "scope").as_deref(),
        )?),

        // ── Info panel, files, and Git ──
        "get_git_status" => to_value(git::status(&req_str(args, "path")?)),
        "git_changed_files" => to_value(git::changed_files(&req_str(args, "cwd")?)?),
        "git_file_diff" => to_value(git::file_diff(
            &req_str(args, "cwd")?,
            &req_str(args, "path")?,
        )?),
        "git_recent_commits" => to_value(git::recent_commits(&req_str(args, "cwd")?, 5)),
        "agent_context_info" => {
            to_value(core::agent_context_info(app, &req_str(args, "sessionId")?)?)
        }
        "agent_turn_stats" => to_value(core::agent_turn_stats(app, &req_str(args, "sessionId")?)?),
        "claude_usage" => {
            let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
            to_value(crate::agent::usage::claude_usage(force)?)
        }
        "codex_usage" => {
            let live = args.get("live").and_then(Value::as_bool).unwrap_or(false);
            to_value(core::codex_usage(app, &req_str(args, "sessionId")?, live)?)
        }
        "grok_usage" => {
            let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
            to_value(crate::agent::usage::grok_usage(force)?)
        }
        "read_agent_transcript" => to_value(core::read_agent_transcript(
            app,
            &req_str(args, "sessionId")?,
        )?),
        // Export complete session context as Markdown. Browsers omit destPath and download returned
        // content locally; core returns None after writing to disk or Some(content) for transfer.
        "export_session_context" => to_value(core::export_session_context(
            app,
            &req_str(args, "sessionId")?,
            opt_str(args, "destPath").as_deref(),
            opt_str(args, "exportedAt").as_deref(),
        )?),
        "list_dir" => to_value(files::list_dir(&req_str(args, "path")?)?),
        "read_file_preview" => to_value(files::read_preview(&req_str(args, "path")?)?),
        "create_file" => to_value(files::create_file(&req_str(args, "path")?)?),
        "create_dir" => to_value(files::create_dir(&req_str(args, "path")?)?),
        "rename_path" => to_value(files::rename_path(
            &req_str(args, "from")?,
            &req_str(args, "to")?,
        )?),
        "delete_path" => to_value(files::delete_path(&req_str(args, "path")?)?),
        "read_text_file" => to_value(files::read_text_file(&req_str(args, "path")?)?),
        "write_text_file" => to_value(files::write_text_file(
            &req_str(args, "path")?,
            &req_str(args, "content")?,
            opt_u64(args, "expectedMtimeMs"),
        )?),
        // Binary writes such as PDF export arrive as a numeric array and deserialize to Vec<u8>.
        "write_bytes_file" => {
            files::write_bytes(&req_str(args, "path")?, &req_kind::<Vec<u8>>(args, "data")?)?;
            Ok(Value::Null)
        }
        // Browser/remote image paste sends base64 bytes and an extension, saves to a temporary directory,
        // and returns the absolute path for the terminal. Authenticated WebSocket transport, E2EE under
        // pairing, replaces the old cookie-only `/api/upload` path. Base64 is smaller than number[] and
        // shares `files::save_pasted_image` with the identically named desktop command.
        "save_pasted_image" => to_value(files::save_pasted_image(
            &b64_bytes(args, "bytesB64")?,
            &req_str(args, "ext")?,
        )?),
        // Document-editor images persist beside the document under assets/, returning a relative Markdown
        // path. Electron, browser, and remote windows use this authenticated and optionally encrypted call.
        "save_doc_image" => to_value(files::save_doc_image(
            &req_str(args, "docPath")?,
            &b64_bytes(args, "bytesB64")?,
            &req_str(args, "ext")?,
        )?),
        "stat_file" => to_value(files::stat_file(&req_str(args, "path")?)?),
        "read_file_base64" => to_value(files::read_file_base64(
            &req_str(args, "path")?,
            req_u64(args, "offset")?,
            req_u64(args, "maxLen")?,
        )?),
        "process_stats" => {
            let pid = args
                .get("pid")
                .and_then(|v| v.as_u64())
                .ok_or("Missing pid")? as u32;
            to_value(crate::procstat::subtree_stats(pid))
        }
        "create_worktree" => {
            let repo_root = req_str(args, "repoRoot")?;
            let info = git::worktree_add(&repo_root, &req_str(args, "name")?)?;
            let patterns = crate::agent::orchestration::load(app)
                .limits
                .worktree_copy_patterns;
            let _ = git::copy_into_worktree(&repo_root, &info.path, &patterns);
            to_value(info)
        }
        "list_worktrees" => to_value(git::worktree_list(&req_str(args, "repoRoot")?)?),
        "worktrees_in_subtree" => to_value(core::worktrees_in_subtree(
            app,
            &req_str(args, "sessionId")?,
        )?),
        "remove_worktree" => {
            git::worktree_remove(&req_str(args, "path")?, req_bool(args, "force")?)?;
            Ok(Value::Null)
        }
        "commit_worktree" => {
            git::commit_all(&req_str(args, "wtPath")?, &req_str(args, "message")?)?;
            Ok(Value::Null)
        }
        "delete_branch" => {
            git::branch_delete(&req_str(args, "repo")?, &req_str(args, "branch")?)?;
            Ok(Value::Null)
        }
        "git_branch_list" => to_value(git::branch_list(&req_str(args, "cwd")?)?),
        "git_merge_preview" => to_value(git::merge_branches_preview(
            &req_str(args, "cwd")?,
            &req_str(args, "source")?,
            &req_str(args, "target")?,
        )?),
        "git_merge_apply" => to_value(git::merge_branches_apply(
            &req_str(args, "cwd")?,
            &req_str(args, "source")?,
            &req_str(args, "target")?,
            opt_str(args, "commitMessage").as_deref(),
        )?),
        "gitea_get_status" => to_value(core::gitea_get_status(app)),
        "gitea_set_config" => to_value(core::gitea_set_config(
            app,
            &req_str(args, "baseUrl")?,
            &req_str(args, "token")?,
        )?),
        "gitea_probe" => to_value(core::gitea_probe(
            &req_str(args, "baseUrl")?,
            &req_str(args, "token")?,
        )),
        "land_gitea_pr" => to_value(core::land_gitea_pr(
            app,
            &req_str(args, "sessionId")?,
            &req_str(args, "title")?,
            &req_str(args, "body")?,
        )?),
        // SSH/URL host history and remembered passwords are desktop GUI-only because remote UIs do not
        // initiate nested connections. Their `crate::ssh_remote` dependency and dispatch branches are
        // therefore GUI-gated; the minimal server falls through for these unused commands. Actual connect
        // and password storage use the equally GUI-gated native `ssh_connect` command.
        #[cfg(feature = "gui")]
        "ssh_hosts_list" => to_value(core::ssh_hosts_list(app)?),
        #[cfg(feature = "gui")]
        "ssh_host_forget" => {
            core::ssh_host_forget(app, &req_str(args, "target")?)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "gui")]
        "url_hosts_list" => to_value(core::url_hosts_list(app)?),
        #[cfg(feature = "gui")]
        "url_host_password" => to_value(core::url_host_password(app, &req_str(args, "url")?)),
        #[cfg(feature = "gui")]
        "url_host_record" => {
            core::url_host_record(
                app,
                &req_str(args, "url")?,
                opt_str(args, "password").as_deref(),
                args.get("remember")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )?;
            Ok(Value::Null)
        }
        #[cfg(feature = "gui")]
        "url_host_forget" => {
            core::url_host_forget(app, &req_str(args, "url")?)?;
            Ok(Value::Null)
        }
        // Start browser directory selection at the server machine's home directory.
        "home_dir" => to_value(crate::host::home_dir().map(|h| h.to_string_lossy().to_string())),
        "codex_notify_snippet" => {
            let exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "velaterm".to_string());
            to_value(crate::agent::inject::codex_config_snippet(&exe))
        }

        // ── LAN remote-access service, orchestrated in command_core through ctx.remote_web() ──
        // Electron controls the sidecar's secondary LAN instance through WebSocket. It remains independent
        // from the persistent plaintext loopback instance required by the renderer.
        "web_server_start" => to_value(core::web_server_start(
            app,
            &req_str(args, "password")?,
            opt_u16(args, "port"),
            args.get("lanHttp")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )?),
        "web_server_stop" => {
            core::web_server_stop(app)?;
            Ok(Value::Null)
        }
        "web_server_status" => to_value(core::web_server_status(app)),

        "spawn_skills_installed" => Ok(Value::Bool(crate::agent::spawn_cli::skills_installed())),
        "install_spawn_skills" => {
            crate::agent::spawn_cli::install_skills()
                .map_err(|e| format!("Failed to install spawn skills: {e}"))?;
            Ok(Value::Null)
        }
        "uninstall_spawn_skills" => {
            crate::agent::spawn_cli::uninstall_skills()
                .map_err(|e| format!("Failed to uninstall spawn skills: {e}"))?;
            Ok(Value::Null)
        }

        other => Err(format!("Unknown command: {other}")),
    }
}

// ─────────────────────────── Argument extraction helpers ───────────────────────────

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| format!("Failed to serialize result: {e}"))
}

fn req_str(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("Missing string parameter {key}"))
}

/// Decode a base64 string argument to bytes. WebSocket binary payloads such as images use base64,
/// which is roughly three times smaller than number arrays, and are restored here.
fn b64_bytes(args: &Value, key: &str) -> Result<Vec<u8>, String> {
    B64.decode(req_str(args, key)?.trim())
        .map_err(|e| format!("Invalid base64 for parameter {key}: {e}"))
}

/// Optional string: missing and null both become None.
fn opt_str(args: &Value, key: &str) -> Option<String> {
    match args.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Optional unsigned integer: missing and null both become None.
fn opt_u64(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(Value::as_u64)
}

/// Optional u16: missing, null, and out-of-range values become None so ports may use backend defaults.
fn opt_u16(args: &Value, key: &str) -> Option<u16> {
    args.get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())
}

fn req_u64(args: &Value, key: &str) -> Result<u64, String> {
    args.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("Missing numeric parameter {key}"))
}

fn req_u16(args: &Value, key: &str) -> Result<u16, String> {
    args.get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())
        .ok_or_else(|| format!("Missing numeric parameter {key}"))
}

fn req_i64(args: &Value, key: &str) -> Result<i64, String> {
    args.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("Missing numeric parameter {key}"))
}

fn req_bool(args: &Value, key: &str) -> Result<bool, String> {
    args.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("Missing boolean parameter {key}"))
}

/// Deserialize an enum argument such as SessionKind or NodeKind using its serde rename rules.
fn req_kind<T: serde::de::DeserializeOwned>(args: &Value, key: &str) -> Result<T, String> {
    let v = args.get(key).cloned().unwrap_or(Value::Null);
    serde_json::from_value(v).map_err(|e| format!("Invalid value for parameter {key}: {e}"))
}
