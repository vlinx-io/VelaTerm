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

/// Trust classification of a dispatch call's origin, independent of the `source` connection ID.
///
/// The threat-model boundary is "who can reach this transport": the Tauri desktop (`desktop_call`) and
/// any client of a `LoopbackHttp` instance (the Electron sidecar; reaching loopback implies local shell
/// access, which the threat model already trusts) are `Local`. Clients of the network-exposed
/// `LanTls`/`LanHttp` instances are `Remote`, even though they are fully authenticated: a paired device
/// gets a shell, but not the management plane that could rotate the pairing token, revoke other devices,
/// or read/rewrite the persisted remote-access credentials.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallOrigin {
    Local,
    Remote,
}

impl CallOrigin {
    /// Map a serving mode to the trust origin of its WebSocket clients. String-matching on the `ws-N`
    /// source ID cannot work here because Electron loopback and remote browsers both connect over WS.
    pub fn for_serve_mode(mode: super::ServeMode) -> Self {
        match mode {
            super::ServeMode::LoopbackHttp => CallOrigin::Local,
            super::ServeMode::LanTls | super::ServeMode::LanHttp => CallOrigin::Remote,
        }
    }
}

/// Management-plane commands gated to local origins: they control the remote-access service itself
/// (start/stop/restart with a new password, pairing-token rotation, device revocation) or leak host
/// topology (ports, URLs, fingerprint, interfaces). No remote UI calls them — the remote-access panel
/// and its status polling are desktop/Electron-only — so gating is regression-free by construction.
const MANAGEMENT_CMDS: &[&str] = &[
    "web_server_start",
    "web_server_stop",
    "web_server_status",
    "web_pairing_create",
    "web_devices_list",
    "web_device_revoke",
    "network_interfaces_list",
    // Mirror mode is host-side service configuration (the checkbox lives in the desktop remote-access
    // panel), so a paired device may follow the shared layout but never switch the feature for everyone.
    "mirror_set_enabled",
];

/// Commands that read or write stored secrets and are therefore gated to local origins — the class
/// behind the protected-settings ACL, swept across every dispatch arm:
/// - `gitea_set_config` writes the Gitea token (keyring, or the plaintext `gitea.token` app-settings
///   fallback that `is_protected_setting` hides from remote clients — this arm bypassed that ACL).
/// - `url_host_password` returns a remembered remote-window password from the keyring in plaintext.
/// - `url_host_record` writes such a password into the keyring.
/// - `ssh_host_forget` / `url_host_forget` DELETE remembered passwords from the keyring — a secret-store
///   mutation (remote wipe of stored credentials), even though they never return a secret.
/// Reviewed but deliberately NOT gated: `gitea_get_status` returns only `has_token`/`configured`
/// booleans, never the token; `gitea_probe` uses a caller-supplied token, not a stored one;
/// `land_gitea_pr` uses the stored token internally without returning it; `ssh_hosts_list` /
/// `url_hosts_list` return host metadata with a `has_password` flag, never the secret itself.
/// No remote UI calls the gated commands (the URL-host flows are desktop GUI-only), so gating is
/// regression-free by construction.
const SECRET_CMDS: &[&str] = &[
    "gitea_set_config",
    "url_host_password",
    "url_host_record",
    "ssh_host_forget",
    "url_host_forget",
];

/// Whether an app-settings key is security-relevant and therefore hidden from and unwritable by remote
/// clients: the `remoteAccess.*` namespace carries the Argon2id password verifier (an offline-bruteforce
/// target) and the autostart port/enabled keys the next restart trusts; `gitea.token` is a plaintext
/// credential fallback when no keyring is available.
fn is_protected_setting(key: &str) -> bool {
    key.starts_with("remoteAccess.") || key == "gitea.token"
}

/// Deny remote clients direct file access inside the app data directory, which holds the secret files
/// behind the remote-access trust model: the SQLite database (Argon2id PHC verifier, plaintext settings
/// fallbacks), `vlx-web-access.json` (pairing token), `vlx-e2ee-key.b64`, and the TLS key. This is a
/// narrow deny-list, not file-API sandboxing: every path outside the data directory stays allowed so
/// legitimate remote features (doc editor, file viewer) keep working; local origins are unrestricted.
///
/// Accepted limits (reviewed, deliberate — not oversights):
/// - Hard links are invisible to path resolution: a hard link to a secret created OUTSIDE the data
///   dir before this check would pass the prefix comparison. Creating one requires local filesystem
///   access the remote client does not have through this API (rename into the data dir is gated).
/// - The resolve-then-operate sequence has an inherent check-then-use window: a path component can
///   be swapped for a symlink between the check and the file operation. Closing either gap would
///   need openat-style descriptor-anchored traversal, which std::fs does not expose; the threat
///   model (a paired remote device already trusted with a shell) accepts both.
fn guard_remote_path(app: &AppCtx, origin: CallOrigin, path: &str) -> Result<(), String> {
    if origin != CallOrigin::Remote {
        return Ok(());
    }
    let data_dir = app.data_dir()?;
    // The data dir exists whenever a server instance runs; canonicalizing both sides makes the prefix
    // comparison immune to symlinked temp roots (`/tmp` vs `/private/tmp` on macOS).
    let data_dir = data_dir.canonicalize().unwrap_or(data_dir);
    match resolve_for_acl(std::path::Path::new(path)) {
        // Path::starts_with compares whole components, so `/data-dir-evil` never matches `/data-dir`.
        Some(resolved) if !resolved.starts_with(&data_dir) => Ok(()),
        // Inside the data dir, or unresolvable (nonexistent parent — the file operation itself would
        // fail anyway): deny by default.
        _ => Err(format!("remote_path_forbidden:{path}")),
    }
}

/// Resolve a path for the ACL check, symlink- and traversal-safe, without requiring the target to
/// exist: an existing target is fully canonicalized; a not-yet-existing target canonicalizes its
/// existing parent directory (which resolves any `..` and symlinks in the directory part) and
/// re-appends the final component; a dangling symlink's target is resolved recursively so a write
/// through it is attributed to its real, canonicalized destination (chains of dangling links
/// included — a raw target path would compare against the canonicalized data dir and fail open on
/// systems with symlinked path components such as macOS `/var` → `/private/var`). Returns None when
/// nothing can be resolved (nonexistent parent, or a symlink chain deeper than the cap) — the caller
/// fails closed on None.
fn resolve_for_acl(path: &std::path::Path) -> Option<std::path::PathBuf> {
    // 8 mirrors typical kernel symlink-resolution limits; deeper chains are denied, never allowed.
    resolve_for_acl_depth(path, 8)
}

fn resolve_for_acl_depth(path: &std::path::Path, depth: u8) -> Option<std::path::PathBuf> {
    if depth == 0 {
        return None;
    }
    if let Ok(resolved) = path.canonicalize() {
        return Some(resolved);
    }
    let parent = path.parent()?;
    let name = path.file_name()?;
    let base = parent.canonicalize().ok()?;
    if let Ok(target) = std::fs::read_link(path) {
        // Dangling symlink: resolve its target relative to the canonicalized parent, then resolve
        // the (by definition nonexistent) target itself recursively — that canonicalizes its parent
        // and re-appends the final component, keeping both sides of the prefix comparison canonical.
        let target = if target.is_absolute() {
            target
        } else {
            base.join(target)
        };
        return resolve_for_acl_depth(&target, depth - 1);
    }
    Some(base.join(name))
}

/// Dispatch one command and return a JSON value ready for frontend serialization.
///
/// `source` identifies the originating WebSocket connection for PTY resize-owner arbitration. Desktop
/// commands always use desktop, and input no longer participates in arbitration. `origin` classifies the
/// caller's trust (see [`CallOrigin`]) and gates the management plane and protected settings keys.
pub fn dispatch(
    app: &AppCtx,
    cmd: &str,
    args: &Value,
    source: &str,
    origin: CallOrigin,
) -> Result<Value, String> {
    // Gate the management plane and secret-bearing commands before any argument parsing: remote paired
    // devices are trusted with a shell (threat model), but not with rotating/revoking the credentials
    // that admit other devices, nor with reading/writing stored secrets. The machine-readable
    // `code:detail` error format is mapped to a localized message in the frontend (wsClient.ts).
    if origin == CallOrigin::Remote && (MANAGEMENT_CMDS.contains(&cmd) || SECRET_CMDS.contains(&cmd))
    {
        return Err(format!("remote_cmd_forbidden:{cmd}"));
    }
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
            // Cloning creates a directory tree at a caller-chosen location; planting files inside
            // the data dir is the same class as rename_path's `to` direction, so gate the target.
            // folderName needs no separate gate: git.rs rejects separators in it and joins it under
            // this gated parentDir, so the composed target cannot escape the checked prefix.
            let parent_dir = req_str(args, "parentDir")?;
            guard_remote_path(app, origin, &parent_dir)?;
            to_value(core::clone_project(
                app,
                &req_str(args, "url")?,
                &parent_dir,
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
        "list_agent_presets" => to_value(core::list_agent_presets(app)?),
        // Agent presets carry `execPath`, which names a program rather than reading a file. It is gated
        // by the same reasoning as create_session's `agentPath`: a paired device already holds a shell.
        "create_agent_preset" => to_value(core::create_agent_preset(
            app,
            &req_str(args, "name")?,
            req_kind::<SessionKind>(args, "baseKind")?,
            opt_str(args, "execPath").as_deref(),
            opt_str(args, "agentArgs").as_deref(),
            opt_str(args, "permissionMode").as_deref(),
            opt_str(args, "icon").as_deref(),
        )?),
        "update_agent_preset" => to_value(core::update_agent_preset(
            app,
            &req_str(args, "id")?,
            &req_str(args, "name")?,
            opt_str(args, "execPath").as_deref(),
            opt_str(args, "agentArgs").as_deref(),
            opt_str(args, "permissionMode").as_deref(),
            opt_str(args, "icon").as_deref(),
        )?),
        "delete_agent_preset" => to_value(core::delete_agent_preset(app, &req_str(args, "id")?)?),
        "reorder_agent_presets" => {
            to_value(core::reorder_agent_presets(app, &req_str_vec(args, "ids")?)?)
        }
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
            opt_str(args, "agentPresetId").as_deref(),
            opt_str(args, "agentPath").as_deref(),
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
        // Remote clients receive the map without security-relevant keys (password verifier, autostart
        // config, plaintext token fallback); local desktop/Electron callers keep the full map.
        "get_app_settings" => {
            let mut settings = core::get_app_settings(app)?;
            if origin == CallOrigin::Remote {
                settings.retain(|k, _| !is_protected_setting(k));
            }
            to_value(settings)
        }
        // Remote and Electron clients compare backend CARGO_PKG_VERSION with __APP_VERSION__ at startup
        // and report frontend/backend build drift. This shares Tauri's app_version source.
        "app_version" => to_value(env!("CARGO_PKG_VERSION")),
        // Anonymous per-installation identifier sent as a header with desktop update checks, so the
        // update server can count installations rather than IP addresses. Created on first call.
        "install_id" => to_value(core::install_id(app)?),
        "set_app_settings" => {
            let entries = args
                .get("entries")
                .and_then(|v| {
                    serde_json::from_value::<std::collections::HashMap<String, String>>(v.clone())
                        .ok()
                })
                .unwrap_or_default();
            // Reject the whole batch instead of silently dropping protected keys: a silent drop would
            // fake success while the write never happened, hiding the ACL from legitimate tooling.
            if origin == CallOrigin::Remote {
                if let Some(key) = entries.keys().find(|k| is_protected_setting(k)) {
                    return Err(format!("remote_setting_forbidden:{key}"));
                }
            }
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
        "git_file_diff" => {
            // file_diff reads the worktree side with a raw fs::read of repo_top(cwd).join(path):
            // an absolute `path` replaces the base entirely and repo_top falls back to `cwd` outside
            // a repository, so the effective target is caller-chosen file content — gate the path
            // actually read, exactly like the files::* content arms.
            let cwd = req_str(args, "cwd")?;
            let path = req_str(args, "path")?;
            if origin == CallOrigin::Remote {
                // Compute the effective worktree path ONCE, gate it, and hand the SAME value through
                // to the read: checked path and used path cannot diverge, and the repo_top git
                // subprocess runs once instead of twice.
                let target = git::file_diff_worktree_path(&cwd, &path);
                guard_remote_path(app, origin, &target.to_string_lossy())?;
                to_value(git::file_diff_at(&cwd, &path, &target)?)
            } else {
                // Local lane: no ACL applies, so skip the extra repo_top subprocess entirely.
                to_value(git::file_diff(&cwd, &path)?)
            }
        }
        "git_recent_commits" => to_value(git::recent_commits(&req_str(args, "cwd")?, 5)),
        // Git panel history: a page of commits, one commit's files, and one file's diff inside it.
        "git_commit_count" => to_value(git::commit_count(&req_str(args, "cwd")?)),
        "git_log_page" => to_value(git::log_page(
            &req_str(args, "cwd")?,
            opt_u64(args, "limit").unwrap_or(30) as usize,
            opt_u64(args, "offset").unwrap_or(0) as usize,
        )),
        "git_commit_files" => to_value(git::commit_files(
            &req_str(args, "cwd")?,
            &req_str(args, "hash")?,
        )?),
        "git_commit_file_diff" => {
            // Unlike git_file_diff this never touches the worktree: both sides come out of the
            // object database through `git show`, so a path here can only name repository content
            // and the data-dir ACL has nothing to gate.
            to_value(git::commit_file_diff(
                &req_str(args, "cwd")?,
                &req_str(args, "hash")?,
                &req_str(args, "path")?,
            )?)
        }
        // Git panel mutations. Each is git-mediated and refused outside a repository; `paths` are
        // repository-relative and validated in git.rs before any command runs.
        "git_stage" => {
            git::stage(&req_str(args, "cwd")?, &req_str_vec(args, "paths")?)?;
            Ok(Value::Null)
        }
        "git_unstage" => {
            git::unstage(&req_str(args, "cwd")?, &req_str_vec(args, "paths")?)?;
            Ok(Value::Null)
        }
        "git_discard" => {
            git::discard(&req_str(args, "cwd")?, &req_str_vec(args, "paths")?)?;
            Ok(Value::Null)
        }
        "git_commit" => to_value(git::commit(
            &req_str(args, "cwd")?,
            &req_str(args, "message")?,
            args.get("amend").and_then(Value::as_bool).unwrap_or(false),
        )?),
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
        "export_session_context" => {
            // destPath is a caller-chosen write target on disk; same write class as write_text_file.
            let dest = opt_str(args, "destPath");
            if let Some(dest) = dest.as_deref() {
                guard_remote_path(app, origin, dest)?;
            }
            to_value(core::export_session_context(
                app,
                &req_str(args, "sessionId")?,
                dest.as_deref(),
                opt_str(args, "exportedAt").as_deref(),
            )?)
        }
        // Every remote-dispatchable arm that returns file CONTENT or MUTATES the filesystem at a
        // caller-chosen path carries `guard_remote_path` (the data-dir ACL). This is no longer a
        // hand-maintained claim: `remote_path_acl_has_no_ungated_path_arm` below mechanically
        // enumerates every dispatch arm extracting a path-like argument and fails on any arm that is
        // neither gated nor listed as a justified exception. Deliberately NOT gated:
        // - `list_dir` / `stat_file`: metadata only (names, sizes, mtimes) — no content, no mutation.
        // - `get_git_status` / `git_changed_files` / `git_recent_commits` / `git_branch_list` /
        //   `git_merge_*` / `create_worktree` / `list_worktrees` / `commit_worktree` /
        //   `delete_branch`: repo-scoped through git itself — they return derived metadata
        //   (statuses, name lists, commit info, merge summaries), never raw bytes of a caller-named
        //   file, and their mutations go through git, which refuses the non-repo data dir.
        //   `git_file_diff` is the exception — it reads a caller-chosen file raw from the
        //   filesystem, independent of repo status — and is therefore gated above.
        // - `save_pasted_image`: writes only into a temp directory it picks itself — no caller path.
        "list_dir" => to_value(files::list_dir(&req_str(args, "path")?)?),
        "read_file_preview" => {
            // Returns up to 64 KB of file content — the pairing token, E2EE key, and TLS key are all
            // NUL-free text and would round-trip through this arm in full.
            let path = req_str(args, "path")?;
            guard_remote_path(app, origin, &path)?;
            to_value(files::read_preview(&path)?)
        }
        "create_file" => {
            let path = req_str(args, "path")?;
            guard_remote_path(app, origin, &path)?;
            to_value(files::create_file(&path)?)
        }
        "create_dir" => {
            let path = req_str(args, "path")?;
            guard_remote_path(app, origin, &path)?;
            to_value(files::create_dir(&path)?)
        }
        "rename_path" => {
            // Both ends are gated: renaming a secret OUT of the data dir is a two-step exfiltration
            // (move, then read outside), renaming INTO it plants files next to the secrets.
            let from = req_str(args, "from")?;
            let to = req_str(args, "to")?;
            guard_remote_path(app, origin, &from)?;
            guard_remote_path(app, origin, &to)?;
            to_value(files::rename_path(&from, &to)?)
        }
        "delete_path" => {
            let path = req_str(args, "path")?;
            guard_remote_path(app, origin, &path)?;
            to_value(files::delete_path(&path)?)
        }
        // The generic full-content read/write arms are the ones that could hand a remote client the
        // secret files in the data directory (DB with the PHC verifier, pairing/E2EE/TLS keys), so they
        // carry the data-dir ACL; all other paths behave exactly as before.
        "read_text_file" => {
            let path = req_str(args, "path")?;
            guard_remote_path(app, origin, &path)?;
            to_value(files::read_text_file(&path)?)
        }
        "write_text_file" => {
            let path = req_str(args, "path")?;
            guard_remote_path(app, origin, &path)?;
            to_value(files::write_text_file(
                &path,
                &req_str(args, "content")?,
                opt_u64(args, "expectedMtimeMs"),
            )?)
        }
        // Binary writes such as PDF export arrive as a numeric array and deserialize to Vec<u8>. Same
        // write class as write_text_file, so it carries the same remote data-dir ACL.
        "write_bytes_file" => {
            let path = req_str(args, "path")?;
            guard_remote_path(app, origin, &path)?;
            files::write_bytes(&path, &req_kind::<Vec<u8>>(args, "data")?)?;
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
        "save_doc_image" => {
            // Writes into `<docPath parent>/assets/` — a caller-chosen location, same write class.
            // Gate BOTH the document path and the EFFECTIVE assets directory it writes into: with a
            // docPath just outside the data dir, an `assets` entry symlinked into it would pass the
            // docPath check alone while the actual write lands inside the data dir (check/use split).
            let doc_path = req_str(args, "docPath")?;
            guard_remote_path(app, origin, &doc_path)?;
            let assets = files::doc_assets_dir(&doc_path)?;
            guard_remote_path(app, origin, &assets.to_string_lossy())?;
            to_value(files::save_doc_image(
                &doc_path,
                &b64_bytes(args, "bytesB64")?,
                &req_str(args, "ext")?,
            )?)
        }
        "stat_file" => to_value(files::stat_file(&req_str(args, "path")?)?),
        "read_file_base64" => {
            let path = req_str(args, "path")?;
            guard_remote_path(app, origin, &path)?;
            to_value(files::read_file_base64(
                &path,
                req_u64(args, "offset")?,
                req_u64(args, "maxLen")?,
            )?)
        }
        "process_stats" => {
            let pid = args
                .get("pid")
                .and_then(|v| v.as_u64())
                .ok_or("Missing pid")? as u32;
            to_value(crate::procstat::subtree_stats(pid))
        }
        "create_worktree" => to_value(git::worktree_add(
            &req_str(args, "repoRoot")?,
            &req_str(args, "name")?,
        )?),
        "list_worktrees" => to_value(git::worktree_list(&req_str(args, "repoRoot")?)?),
        "worktrees_in_subtree" => to_value(core::worktrees_in_subtree(
            app,
            &req_str(args, "sessionId")?,
        )?),
        "remove_worktree" => {
            // Deletes a directory tree at a caller-chosen path; gated like the other mutating arms.
            let path = req_str(args, "path")?;
            guard_remote_path(app, origin, &path)?;
            git::worktree_remove(&path, req_bool(args, "force")?)?;
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
        // UI mirror: every client publishes its layout here and follows what the others publish. `source`
        // comes from the transport, never from the arguments, so the echo filter cannot be spoofed.
        "mirror_get" => Ok(core::mirror_get(app)),
        "mirror_push" => Ok(core::mirror_push(
            app,
            source,
            args.get("state").cloned().unwrap_or(Value::Null),
        )),
        "mirror_set_enabled" => {
            core::mirror_set_enabled(app, req_bool(args, "enabled")?)?;
            Ok(Value::Null)
        }
        // Stateless interface enumeration for the panel's IP selector; the module is called directly
        // because no AppCtx is involved, and desktop_call routes the desktop shell through here too.
        "network_interfaces_list" => to_value(crate::web::network_interfaces_list()),
        // Pairing management mirrors the Tauri commands so the Electron/browser remote-access panel
        // works over WebSocket exactly like the desktop panel does over Tauri IPC.
        "web_pairing_create" => to_value(core::web_pairing_create(
            app,
            opt_str(args, "address"),
            args.get("rotate").and_then(Value::as_bool).unwrap_or(false),
        )?),
        "web_devices_list" => to_value(core::web_devices_list(app)),
        "web_device_revoke" => Ok(Value::Bool(core::web_device_revoke(
            app,
            &req_str(args, "deviceId")?,
        )?)),

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

        // Answers true only for the client that answered first; a later caller must drop its card
        // without spawning anything.
        "resolve_spawn" => Ok(Value::Bool(core::resolve_spawn(
            app,
            source,
            &req_str(args, "parentSessionId")?,
            &req_str(args, "prompt")?,
            req_bool(args, "confirmed")?,
        ))),

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

/// Read a required array-of-strings argument, rejecting a non-array or any non-string element.
fn req_str_vec(args: &Value, key: &str) -> Result<Vec<String>, String> {
    let arr = args
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Missing array parameter {key}"))?;
    arr.iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("Parameter {key} must contain only strings"))
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::{dispatch, CallOrigin, MANAGEMENT_CMDS, SECRET_CMDS};
    use crate::host::{AppCtx, HeadlessHost};
    use crate::pty::manager::DESKTOP_SOURCE;
    use serde_json::{json, Value};
    use std::sync::Arc;

    /// Side-effect-free headless context with an isolated temporary database; the remote-access
    /// WebServer is constructed but not started, matching the panel's initial state.
    fn test_ctx() -> AppCtx {
        let data_dir =
            std::env::temp_dir().join(format!("vlx-dispatch-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).expect("failed to create the temporary directory");
        let db = crate::db::Db::open(&data_dir.join("test.db"))
            .expect("failed to open the test database");
        AppCtx::Headless(Arc::new(HeadlessHost::new(data_dir, db)))
    }

    /// Regression for the Electron/browser pairing panel: the three pairing commands must be routed
    /// by WebSocket dispatch instead of falling through to "Unknown command" (they were Tauri-only).
    #[test]
    fn pairing_commands_are_dispatched() {
        let app = test_ctx();

        // Stopped server: pairing creation fails with the WebServer's own error, proving the call
        // reached core rather than the unknown-command fallback.
        let err = dispatch(
            &app,
            "web_pairing_create",
            &json!({ "address": null, "rotate": false }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .unwrap_err();
        assert_eq!(err, "Web server not started");

        // The arm tolerates omitted keys (address defaults to None, rotate to false); a bare WS
        // client sending an empty args object must reach core the same way.
        let err = dispatch(&app, "web_pairing_create", &json!({}), DESKTOP_SOURCE, CallOrigin::Local).unwrap_err();
        assert_eq!(err, "Web server not started");

        // Device listing returns an empty list while stopped, like the Tauri command does.
        let devices = dispatch(&app, "web_devices_list", &json!({}), DESKTOP_SOURCE, CallOrigin::Local).unwrap();
        assert_eq!(devices, json!([]));

        // Revocation returns false while stopped; the camelCase key matches webServer.ts.
        let revoked = dispatch(
            &app,
            "web_device_revoke",
            &json!({ "deviceId": "dev-a" }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .unwrap();
        assert_eq!(revoked, Value::Bool(false));
    }

    /// Success path on a running loopback server (the Electron mode this ticket targets): pairing
    /// creation over WebSocket dispatch returns a real pairing URL, and rotate issues a new token.
    #[test]
    fn pairing_create_works_on_running_loopback_server() {
        let app = test_ctx();

        // Find a free loopback port and start the real server there in plaintext mode. The probed
        // port can be stolen between drop and the server's own preflight bind while other tests
        // bind ports in parallel, so retry with a fresh ephemeral port instead of flaking.
        let mut port = 0;
        let mut started = Err("never attempted".to_string());
        for _ in 0..5 {
            port = std::net::TcpListener::bind(("127.0.0.1", 0))
                .expect("failed to bind an ephemeral port")
                .local_addr()
                .expect("failed to read the ephemeral port")
                .port();
            started = app.remote_web().start(
                app.clone(),
                crate::web::StartAuth::Password("test-pw".into()),
                Some(port),
                crate::web::ServeMode::LoopbackHttp,
            );
            if started.is_ok() {
                break;
            }
        }
        started.expect("failed to start the loopback web server after retries");

        let first = dispatch(
            &app,
            "web_pairing_create",
            &json!({ "address": "127.0.0.1", "rotate": false }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .expect("pairing creation failed on a running server");
        let url = first["url"].as_str().expect("url missing from PairingInfo");
        assert!(
            url.starts_with(&format!("http://127.0.0.1:{port}/#pair=")),
            "unexpected pairing URL: {url}"
        );
        let first_token = first["deviceToken"]
            .as_str()
            .expect("deviceToken missing from PairingInfo");
        assert!(!first_token.is_empty(), "device token must not be empty");

        // Rotation must invalidate the previous shared token by issuing a different one.
        let rotated = dispatch(
            &app,
            "web_pairing_create",
            &json!({ "address": "127.0.0.1", "rotate": true }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .expect("pairing rotation failed on a running server");
        let rotated_token = rotated["deviceToken"]
            .as_str()
            .expect("deviceToken missing after rotation");
        assert_ne!(rotated_token, first_token, "rotate must issue a new token");

        // Stop the server so the test leaves no background thread behind.
        app.remote_web().stop();
    }

    /// The IP-selector command is routed by dispatch (covering both transports via desktop_call) and
    /// returns an array of {name, ip, vpn} objects. Content is machine-dependent, so only the shape
    /// and the machine-independent ordering contract are asserted; an empty list is valid.
    #[test]
    fn network_interfaces_list_is_dispatched_with_shape() {
        let app = test_ctx();
        let result = dispatch(&app, "network_interfaces_list", &json!({}), DESKTOP_SOURCE, CallOrigin::Local)
            .expect("network_interfaces_list must be routed, not an unknown command");
        let list = result.as_array().expect("expected a JSON array");
        // Ordering contract of iface_candidates: broadcast-capable LAN interfaces come first, so once
        // a vpn=true entry has appeared, no vpn=false entry may follow.
        let mut seen_vpn = false;
        for entry in list {
            assert!(entry["name"].is_string(), "name must be a string: {entry}");
            assert!(entry["ip"].is_string(), "ip must be a string: {entry}");
            let vpn = entry["vpn"]
                .as_bool()
                .unwrap_or_else(|| panic!("vpn must be a boolean: {entry}"));
            assert!(
                !(seen_vpn && !vpn),
                "LAN entries must precede VPN entries, but {entry} follows a VPN entry"
            );
            seen_vpn = seen_vpn || vpn;
        }
    }

    /// Documents the contract the IP selector builds on: the pairing link host is exactly the address
    /// argument, verbatim, even for a CGNAT (Tailscale) address the server is not bound to. The server
    /// runs on loopback only; the address is a pure URL string, no network access to 100.x occurs.
    #[test]
    fn pairing_create_uses_selected_cgnat_address_as_host() {
        let app = test_ctx();

        // Same retry pattern as above: the probed ephemeral port can be stolen before the server binds.
        let mut port = 0;
        let mut started = Err("never attempted".to_string());
        for _ in 0..5 {
            port = std::net::TcpListener::bind(("127.0.0.1", 0))
                .expect("failed to bind an ephemeral port")
                .local_addr()
                .expect("failed to read the ephemeral port")
                .port();
            started = app.remote_web().start(
                app.clone(),
                crate::web::StartAuth::Password("test-pw".into()),
                Some(port),
                crate::web::ServeMode::LoopbackHttp,
            );
            if started.is_ok() {
                break;
            }
        }
        started.expect("failed to start the loopback web server after retries");

        let info = dispatch(
            &app,
            "web_pairing_create",
            &json!({ "address": "100.100.5.5", "rotate": false }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .expect("pairing creation failed on a running server");
        let url = info["url"].as_str().expect("url missing from PairingInfo");
        assert!(
            url.starts_with(&format!("http://100.100.5.5:{port}/#pair=")),
            "pairing URL must carry the selected address as host: {url}"
        );

        // Stop the server so the test leaves no background thread behind.
        app.remote_web().stop();
    }

    /// The deviceId argument is required; a missing key must fail argument extraction, not panic.
    #[test]
    fn device_revoke_requires_device_id() {
        let app = test_ctx();
        let err = dispatch(&app, "web_device_revoke", &json!({}), DESKTOP_SOURCE, CallOrigin::Local).unwrap_err();
        assert_eq!(err, "Missing string parameter deviceId");
    }

    /// Every management-plane command is rejected for remote origins before any argument parsing or core
    /// work — even with valid arguments — while the same calls keep working for local origins (see the
    /// pairing tests above, which all run with CallOrigin::Local and cover the desktop_call path).
    #[test]
    fn management_commands_are_gated_for_remote_origin() {
        let app = test_ctx();
        // Valid-looking args per command prove the gate fires before extraction, not on missing params.
        let args = json!({
            "password": "pw", "deviceId": "dev-a", "address": "127.0.0.1", "rotate": true
        });
        // Pin the gated set literally: iterating over the production constant alone would stay green if a
        // command were removed from MANAGEMENT_CMDS, silently un-gating it. The acceptance criterion names
        // these commands, so shrinking the set must fail HERE first.
        const EXPECTED_GATED: &[&str] = &[
            "web_server_start",
            "web_server_stop",
            "web_server_status",
            "web_pairing_create",
            "web_devices_list",
            "web_device_revoke",
            "network_interfaces_list",
            // Added with mirror mode: the switch belongs to the host running the service, not to the
            // devices connecting to it.
            "mirror_set_enabled",
        ];
        assert_eq!(
            MANAGEMENT_CMDS, EXPECTED_GATED,
            "MANAGEMENT_CMDS changed: removing a command un-gates it for remote clients — update this \
             pinned list only together with a deliberate security review"
        );
        for cmd in EXPECTED_GATED {
            let err = dispatch(&app, cmd, &args, "ws-1", CallOrigin::Remote).unwrap_err();
            assert_eq!(
                err,
                format!("remote_cmd_forbidden:{cmd}"),
                "{cmd} must be gated for remote origins"
            );
        }
    }

    /// Secret-bearing commands (the class behind the settings ACL) are rejected for remote origins
    /// before any argument parsing, while local origins keep them working. The set is pinned literally
    /// for the same reason as EXPECTED_GATED above: shrinking SECRET_CMDS must fail here first.
    #[test]
    fn secret_commands_are_gated_for_remote_origin() {
        let app = test_ctx();
        const EXPECTED_SECRET: &[&str] = &[
            "gitea_set_config",
            "url_host_password",
            "url_host_record",
            "ssh_host_forget",
            "url_host_forget",
        ];
        assert_eq!(
            SECRET_CMDS, EXPECTED_SECRET,
            "SECRET_CMDS changed: removing a command lets remote clients read or write stored secrets — \
             update this pinned list only together with a deliberate security review"
        );
        let args = json!({
            "baseUrl": "http://gitea.local", "token": "secret", "url": "http://h:1",
            "remember": false, "target": "user@host"
        });
        for cmd in EXPECTED_SECRET {
            let err = dispatch(&app, cmd, &args, "ws-1", CallOrigin::Remote).unwrap_err();
            assert_eq!(
                err,
                format!("remote_cmd_forbidden:{cmd}"),
                "{cmd} must be gated for remote origins"
            );
        }
    }

    /// The local lane keeps working for every newly gated secret command: gitea_set_config reaches core
    /// (an empty token skips keyring writes and only persists the base URL — tests must not touch the
    /// real keyring), and the GUI-only URL-host commands reach core as well.
    #[test]
    fn secret_commands_keep_working_for_local_origin() {
        let app = test_ctx();
        let status = dispatch(
            &app,
            "gitea_set_config",
            &json!({ "baseUrl": "http://gitea.local", "token": "" }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .expect("local gitea_set_config must reach core");
        assert_eq!(status["hasToken"], json!(false));

        #[cfg(feature = "gui")]
        {
            // No password is stored for this URL, so core returns None (Null) — but the call reaches core.
            let pw = dispatch(
                &app,
                "url_host_password",
                &json!({ "url": "http://127.0.0.1:9999/#pair=x" }),
                DESKTOP_SOURCE,
                CallOrigin::Local,
            )
            .expect("local url_host_password must reach core");
            assert_eq!(pw, Value::Null);

            // remember=false records the host without touching stored passwords beyond a no-op delete.
            dispatch(
                &app,
                "url_host_record",
                &json!({ "url": "http://127.0.0.1:9999/#pair=x", "remember": false }),
                DESKTOP_SOURCE,
                CallOrigin::Local,
            )
            .expect("local url_host_record must reach core");

            // The forget commands stay usable locally: deleting an unknown host is a no-op delete in
            // the keyring plus a DB row delete, so the calls reach core without side effects.
            dispatch(
                &app,
                "ssh_host_forget",
                &json!({ "target": "nobody@nowhere.example" }),
                DESKTOP_SOURCE,
                CallOrigin::Local,
            )
            .expect("local ssh_host_forget must reach core");
            dispatch(
                &app,
                "url_host_forget",
                &json!({ "url": "http://127.0.0.1:9999/#pair=x" }),
                DESKTOP_SOURCE,
                CallOrigin::Local,
            )
            .expect("local url_host_forget must reach core");
        }
    }

    /// FIX for the named residual risk: remote clients must not read or write files inside the data
    /// directory (DB with PHC verifier, vlx-web-access.json, vlx-e2ee-key.b64, TLS key) through the
    /// generic file arms — while paths outside the data dir stay fully usable remotely (doc editor,
    /// file viewer) and local origins stay unrestricted even inside the data dir.
    #[test]
    fn remote_file_access_to_data_dir_is_denied() {
        let app = test_ctx();
        let data_dir = app.data_dir().unwrap();
        let secret = data_dir.join("vlx-web-access.json");
        std::fs::write(&secret, b"{\"pairing_token\":\"top-secret\"}").unwrap();
        let secret_str = secret.to_string_lossy().to_string();

        // Remote read of a data-dir file is denied with the stable code, for both read arms.
        for cmd in ["read_text_file", "read_file_base64"] {
            let err = dispatch(
                &app,
                cmd,
                &json!({ "path": secret_str, "offset": 0, "maxLen": 64 }),
                "ws-1",
                CallOrigin::Remote,
            )
            .unwrap_err();
            assert_eq!(err, format!("remote_path_forbidden:{secret_str}"), "{cmd} must deny data-dir reads");
        }

        // Remote writes into the data dir are denied — including a not-yet-existing target (the parent
        // directory resolves) and the binary write arm.
        let new_target = data_dir.join("planted.txt");
        let new_target_str = new_target.to_string_lossy().to_string();
        let err = dispatch(
            &app,
            "write_text_file",
            &json!({ "path": new_target_str, "content": "x" }),
            "ws-1",
            CallOrigin::Remote,
        )
        .unwrap_err();
        assert_eq!(err, format!("remote_path_forbidden:{new_target_str}"));
        let err = dispatch(
            &app,
            "write_bytes_file",
            &json!({ "path": secret_str, "data": [1, 2, 3] }),
            "ws-1",
            CallOrigin::Remote,
        )
        .unwrap_err();
        assert_eq!(err, format!("remote_path_forbidden:{secret_str}"));
        assert!(!new_target.exists(), "a denied write must not create the file");

        // Traversal into the data dir is resolved before the check: a path that leaves and re-enters
        // through `..` is still denied.
        let dodged = format!(
            "{}/../{}/vlx-web-access.json",
            data_dir.to_string_lossy(),
            data_dir.file_name().unwrap().to_string_lossy()
        );
        let err = dispatch(
            &app,
            "read_text_file",
            &json!({ "path": dodged }),
            "ws-1",
            CallOrigin::Remote,
        )
        .unwrap_err();
        assert_eq!(err, format!("remote_path_forbidden:{dodged}"));

        // A symlink outside the data dir pointing into it is resolved and denied (unix only).
        #[cfg(unix)]
        {
            let outside = std::env::temp_dir().join(format!("vlx-acl-link-{}", uuid::Uuid::new_v4()));
            std::os::unix::fs::symlink(&secret, &outside).unwrap();
            let outside_str = outside.to_string_lossy().to_string();
            let err = dispatch(
                &app,
                "read_text_file",
                &json!({ "path": outside_str }),
                "ws-1",
                CallOrigin::Remote,
            )
            .unwrap_err();
            assert_eq!(err, format!("remote_path_forbidden:{outside_str}"));
            let _ = std::fs::remove_file(&outside);
        }

        // A normal path outside the data dir stays readable and writable for remote clients.
        let project = std::env::temp_dir().join(format!("vlx-acl-proj-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project).unwrap();
        let doc = project.join("notes.md");
        std::fs::write(&doc, b"hello remote").unwrap();
        let doc_str = doc.to_string_lossy().to_string();
        let read = dispatch(
            &app,
            "read_text_file",
            &json!({ "path": doc_str }),
            "ws-1",
            CallOrigin::Remote,
        )
        .expect("remote read outside the data dir must keep working");
        assert_eq!(read["content"], json!("hello remote"));
        dispatch(
            &app,
            "write_text_file",
            &json!({ "path": doc_str, "content": "edited remotely" }),
            "ws-1",
            CallOrigin::Remote,
        )
        .expect("remote write outside the data dir must keep working");
        assert_eq!(std::fs::read_to_string(&doc).unwrap(), "edited remotely");

        // Local origins (desktop and Electron loopback) stay unrestricted inside the data dir.
        let read = dispatch(
            &app,
            "read_text_file",
            &json!({ "path": secret_str }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .expect("local read of data-dir files must stay allowed");
        assert_eq!(read["content"], json!("{\"pairing_token\":\"top-secret\"}"));

        let _ = std::fs::remove_dir_all(&project);
    }

    /// Hardening for the two `resolve_for_acl` branches the main ACL test does not reach:
    /// (1) the `read_link` branch — a dangling symlink outside the data dir pointing at a
    /// not-yet-existing target inside it must be attributed to its real target and denied, otherwise
    /// a remote client plants the link and writes the secret file through it; (2) the deny-by-default
    /// branch — a path whose parent does not exist resolves to None and is denied for remote origins
    /// even though it lies outside the data dir (the file operation would fail anyway; failing closed
    /// costs no legitimate feature).
    #[test]
    fn remote_path_acl_resolves_dangling_symlinks_and_denies_unresolvable_paths() {
        let app = test_ctx();
        let data_dir = app.data_dir().unwrap();

        #[cfg(unix)]
        {
            // Dangling symlink: target inside the data dir does NOT exist, so canonicalize() fails on
            // both the link and the target — only the read_link branch can catch this.
            let target = data_dir.join("planted-via-dangling-link.json");
            assert!(!target.exists());
            let link = std::env::temp_dir().join(format!("vlx-acl-dangling-{}", uuid::Uuid::new_v4()));
            std::os::unix::fs::symlink(&target, &link).unwrap();
            let link_str = link.to_string_lossy().to_string();
            let err = dispatch(
                &app,
                "write_text_file",
                &json!({ "path": link_str, "content": "planted" }),
                "ws-1",
                CallOrigin::Remote,
            )
            .unwrap_err();
            assert_eq!(
                err,
                format!("remote_path_forbidden:{link_str}"),
                "a dangling symlink into the data dir must be denied for remote writes"
            );
            assert!(!target.exists(), "the denied write must not create the target");
            let _ = std::fs::remove_file(&link);
        }

        // Deny-by-default: nonexistent parent directory → resolve_for_acl returns None → denied for
        // remote even outside the data dir.
        let unresolvable = std::env::temp_dir()
            .join(format!("vlx-acl-missing-parent-{}", uuid::Uuid::new_v4()))
            .join("file.txt");
        let unresolvable_str = unresolvable.to_string_lossy().to_string();
        let err = dispatch(
            &app,
            "write_text_file",
            &json!({ "path": unresolvable_str, "content": "x" }),
            "ws-1",
            CallOrigin::Remote,
        )
        .unwrap_err();
        assert_eq!(
            err,
            format!("remote_path_forbidden:{unresolvable_str}"),
            "an unresolvable path (missing parent) must fail closed for remote origins"
        );

        // Chain of dangling symlinks (link → link → data-dir target): read_link only unwraps one
        // level, so only the recursive resolution attributes the write to the data dir.
        #[cfg(unix)]
        {
            let target = data_dir.join("planted-via-chain.json");
            assert!(!target.exists());
            let inner = std::env::temp_dir().join(format!("vlx-acl-chain-b-{}", uuid::Uuid::new_v4()));
            let outer = std::env::temp_dir().join(format!("vlx-acl-chain-a-{}", uuid::Uuid::new_v4()));
            std::os::unix::fs::symlink(&target, &inner).unwrap();
            std::os::unix::fs::symlink(&inner, &outer).unwrap();
            let outer_str = outer.to_string_lossy().to_string();
            let err = dispatch(
                &app,
                "write_text_file",
                &json!({ "path": outer_str, "content": "planted" }),
                "ws-1",
                CallOrigin::Remote,
            )
            .unwrap_err();
            assert_eq!(
                err,
                format!("remote_path_forbidden:{outer_str}"),
                "a chain of dangling symlinks into the data dir must be denied"
            );
            assert!(!target.exists(), "the denied write must not create the target");
            let _ = std::fs::remove_file(&outer);
            let _ = std::fs::remove_file(&inner);
        }
    }

    /// Class regression for the FIX-5 sweep: EVERY remote-dispatchable arm that returns file content
    /// or mutates the filesystem at a caller-chosen path is denied inside the data dir, keeps working
    /// on normal paths remotely, and stays unrestricted for local origins. `list_dir`/`stat_file`
    /// stay ungated by design (metadata only) — asserted here so a future content-returning change
    /// to them shows up as a conscious decision, not an accident.
    #[test]
    fn remote_data_dir_acl_covers_all_content_and_mutation_arms() {
        let app = test_ctx();
        let data_dir = app.data_dir().unwrap();
        let secret = data_dir.join("vlx-web-access.json");
        std::fs::write(&secret, b"{\"pairing_token\":\"top-secret\"}").unwrap();
        let secret_str = secret.to_string_lossy().to_string();
        let dd = data_dir.to_string_lossy().to_string();

        // (cmd, args) pairs that must be denied for Remote with the stable code on the offending path.
        let denied: Vec<(&str, Value, String)> = vec![
            ("read_file_preview", json!({ "path": secret_str }), secret_str.clone()),
            ("create_file", json!({ "path": format!("{dd}/planted.txt") }), format!("{dd}/planted.txt")),
            ("create_dir", json!({ "path": format!("{dd}/planted-dir") }), format!("{dd}/planted-dir")),
            // Exfil direction: move the secret out of the data dir, then read it outside.
            ("rename_path", json!({ "from": secret_str, "to": format!("{}/exfil.json", std::env::temp_dir().to_string_lossy()) }), secret_str.clone()),
            // Planting direction: move a file into the data dir (`from` is outside and passes; the
            // guard on `to` denies).
            ("rename_path", json!({ "from": format!("{}/nonexistent-src.txt", std::env::temp_dir().to_string_lossy()), "to": format!("{dd}/planted.txt") }), format!("{dd}/planted.txt")),
            ("delete_path", json!({ "path": secret_str }), secret_str.clone()),
            ("save_doc_image", json!({ "docPath": format!("{dd}/doc.md"), "bytesB64": "aGk=", "ext": "png" }), format!("{dd}/doc.md")),
            // Planting direction through clone: a remote client must not clone a fresh repository
            // into the data dir; the gate fires on parentDir before any git/network work starts.
            ("clone_project", json!({ "url": "https://example.invalid/repo.git", "parentDir": dd.clone() }), dd.clone()),
            ("remove_worktree", json!({ "path": dd.clone(), "force": true }), dd.clone()),
            ("export_session_context", json!({ "sessionId": "no-such-session", "destPath": format!("{dd}/export.md") }), format!("{dd}/export.md")),
            // git_file_diff reads the worktree side with a raw fs::read of repo_top(cwd).join(path):
            // an absolute path replaces the base and repo_top falls back to cwd outside a repo, so
            // {cwd:"/", path:<secret>} would return the secret in `modified` without the gate.
            ("git_file_diff", json!({ "cwd": "/", "path": secret_str }), secret_str.clone()),
        ];
        for (cmd, args, path) in &denied {
            let err = dispatch(&app, cmd, args, "ws-1", CallOrigin::Remote).unwrap_err();
            assert_eq!(
                err,
                format!("remote_path_forbidden:{path}"),
                "{cmd} must deny remote access inside the data dir"
            );
        }
        assert!(secret.exists(), "the secret must survive the denied mutations");
        assert!(!data_dir.join("planted.txt").exists());
        assert!(!data_dir.join("planted-dir").exists());

        // Deliberate pinning of the metadata-only exceptions: list_dir and stat_file stay usable
        // remotely even INSIDE the data dir — they return names/sizes/mtimes, never content. If one
        // of them ever starts returning content, gate it and update this assertion consciously.
        dispatch(&app, "list_dir", &json!({ "path": dd.clone() }), "ws-1", CallOrigin::Remote)
            .expect("list_dir must stay ungated for remote origins (metadata only)");
        dispatch(&app, "stat_file", &json!({ "path": secret_str }), "ws-1", CallOrigin::Remote)
            .expect("stat_file must stay ungated for remote origins (metadata only)");

        // Outside the data dir the same arms keep working remotely (doc editor / file viewer).
        let project = std::env::temp_dir().join(format!("vlx-acl-arms-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project).unwrap();
        let doc = project.join("notes.md");
        std::fs::write(&doc, b"hello").unwrap();
        let doc_str = doc.to_string_lossy().to_string();
        let preview = dispatch(
            &app,
            "read_file_preview",
            &json!({ "path": doc_str }),
            "ws-1",
            CallOrigin::Remote,
        )
        .expect("remote preview outside the data dir must keep working");
        assert_eq!(preview["content"], json!("hello"));
        let renamed = project.join("renamed.md");
        dispatch(
            &app,
            "rename_path",
            &json!({ "from": doc_str, "to": renamed.to_string_lossy() }),
            "ws-1",
            CallOrigin::Remote,
        )
        .expect("remote rename outside the data dir must keep working");
        assert!(renamed.exists());

        // Local origin stays unrestricted inside the data dir (desktop and Electron loopback).
        let preview = dispatch(
            &app,
            "read_file_preview",
            &json!({ "path": secret_str }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .expect("local preview of data-dir files must stay allowed");
        assert_eq!(preview["content"], json!("{\"pairing_token\":\"top-secret\"}"));

        let _ = std::fs::remove_dir_all(&project);
    }

    /// FIX-5 regression for the git lane: git_file_diff is gated on the effective worktree path it
    /// reads (repo_top(cwd).join(path)), so a remote client cannot pull data-dir secrets through the
    /// diff viewer — while diffs on normal paths keep working remotely and local origins stay
    /// unrestricted even inside the data dir.
    #[test]
    fn git_file_diff_is_gated_on_its_effective_path() {
        let app = test_ctx();
        let data_dir = app.data_dir().unwrap();
        let secret = data_dir.join("vlx-web-access.json");
        std::fs::write(&secret, b"{\"pairing_token\":\"top-secret\"}").unwrap();
        let secret_str = secret.to_string_lossy().to_string();

        // Remote + absolute secret path (Path::join replaces the base; "/" is not a repository, so
        // repo_top falls back to cwd): denied with the stable code on the effective target.
        let err = dispatch(
            &app,
            "git_file_diff",
            &json!({ "cwd": "/", "path": secret_str }),
            "ws-1",
            CallOrigin::Remote,
        )
        .unwrap_err();
        assert_eq!(err, format!("remote_path_forbidden:{secret_str}"));

        // Remote + relative path with cwd inside the data dir (the repo_top fallback lane): denied.
        let err = dispatch(
            &app,
            "git_file_diff",
            &json!({ "cwd": data_dir.to_string_lossy(), "path": "vlx-web-access.json" }),
            "ws-1",
            CallOrigin::Remote,
        )
        .unwrap_err();
        assert!(
            err.starts_with("remote_path_forbidden:"),
            "relative data-dir diff must be denied, got: {err}"
        );

        // Remote + normal project path stays usable (the ChangesModal diff viewer): outside a repo
        // file_diff still returns the worktree side, which is exactly the behavior to preserve.
        let project = std::env::temp_dir().join(format!("vlx-acl-diff-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("notes.md"), b"hello diff").unwrap();
        let diff = dispatch(
            &app,
            "git_file_diff",
            &json!({ "cwd": project.to_string_lossy(), "path": "notes.md" }),
            "ws-1",
            CallOrigin::Remote,
        )
        .expect("remote git_file_diff outside the data dir must keep working");
        assert_eq!(diff["modified"], json!("hello diff"));

        // Local origin stays unrestricted inside the data dir.
        let diff = dispatch(
            &app,
            "git_file_diff",
            &json!({ "cwd": data_dir.to_string_lossy(), "path": "vlx-web-access.json" }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .expect("local git_file_diff inside the data dir must stay allowed");
        assert_eq!(diff["modified"], json!("{\"pairing_token\":\"top-secret\"}"));

        let _ = std::fs::remove_dir_all(&project);
    }

    /// Argument keys that carry a caller-chosen filesystem path anywhere in this dispatch match.
    /// The trailing `)` distinguishes e.g. `args, "to")` from `args, "token")`.
    const PATH_KEYS: &[&str] = &[
        "args, \"path\")",
        "args, \"cwd\")",
        "args, \"from\")",
        "args, \"to\")",
        "args, \"destPath\")",
        "args, \"docPath\")",
        // The Git panel's batch arms take repository-relative paths under this key.
        "args, \"paths\")",
        "args, \"repoRoot\")",
        "args, \"wtPath\")",
        "args, \"repo\")",
        "args, \"parentDir\")",
        "args, \"worktreePath\")",
        "args, \"rootPath\")",
        // folderName is a path *component*: clone_project joins it under the (gated) parentDir and
        // rejects separators, so the gate on parentDir covers the composed target — but the key is
        // tracked here so a future arm consuming folderName without such a gate turns red.
        "args, \"folderName\")",
        // agentPath names the executable a session's PTY runs. Tracked so a future arm taking it
        // without a gate turns red; create_session itself is justified below.
        "args, \"agentPath\")",
        // execPath is the preset-level counterpart of agentPath, same reasoning.
        "args, \"execPath\")",
    ];
    /// Deliberately ungated path-taking arms, each with its standing justification (mirrors the
    /// sweep comment at the file arms): metadata-only, or repo-scoped through git itself — never
    /// raw bytes of a caller-named file, mutations refused by git on the non-repo data dir.
    const UNGATED_JUSTIFIED: &[&str] = &[
        "list_dir",        // metadata only (names, sizes, mtimes)
        "stat_file",       // metadata only
        "get_git_status",  // derived repo status, no file content
        "git_changed_files",
        "git_recent_commits",
        "git_branch_list",
        "git_merge_preview", // diff_stat summary, no raw file bytes
        "git_merge_apply",   // git-mediated mutation, refused outside a repository
        // Git panel arms. History reads come out of the object database (`git log`, `diff-tree`,
        // `git show <hash>:<path>`), never from a caller-named file on disk, so repository content
        // is all they can return. The mutations run through git with a `--` separator after
        // git.rs rejects absolute paths and `..` components, and git itself refuses a pathspec
        // outside the repository — the one raw filesystem write, deleting an untracked entry,
        // re-resolves the repository root and requires the target's real parent to sit under it.
        "git_commit_count",
        "git_log_page",
        "git_commit_files",
        "git_commit_file_diff",
        "git_stage",
        "git_unstage",
        "git_discard",
        "git_commit",
        "create_worktree",   // git-mediated, refused outside a repository
        "list_worktrees",
        "commit_worktree",   // git-mediated mutation, refused outside a repository
        "delete_branch",
        // Session-tree metadata arms: `cwd`/`worktreePath` only choose where a PTY spawns and
        // `agentPath` which program it spawns; the arms read and write no file content, and a
        // remote paired device already holds a full shell through `pty_spawn` by the threat model,
        // so naming a directory or a binary for it grants nothing further.
        "create_group",
        "create_session",
        "persist_session",
        "update_session",
        // Preset arms store `execPath` as the program a future session will run; they open no file.
        "create_agent_preset",
        "update_agent_preset",
        // Pure DB insert: rootPath is stored as the project root and never read or written on disk
        // by this arm; the sessions it enables later run through the shell trust above.
        "import_project",
    ];

    /// The command names of one arm-start line, or None when the line is no arm start.
    fn arm_names(line: &str) -> Option<Vec<String>> {
        let t = line.trim_start();
        if !t.starts_with('"') {
            return None;
        }
        let (patterns, _) = t.split_once("=>")?;
        let mut names = Vec::new();
        for part in patterns.split('|') {
            let name = part.trim().strip_prefix('"')?.strip_suffix('"')?;
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return None;
            }
            names.push(name.to_string());
        }
        Some(names)
    }

    /// Non-comment lines that LOOK like a match arm (start with `"` and contain `=>`) but are not
    /// recognized by [`arm_names`] — e.g. a future guard arm (`"cmd" if cond =>`) or a pattern
    /// wrapped across lines. Such a line would silently fold its body into the predecessor arm,
    /// exactly the parser-blindness class removed for multi-pattern arms; the chokepoint asserts
    /// this list is empty so any new arm shape fails LOUD instead of hiding a coverage gap.
    fn unrecognized_arm_like_lines(src: &str) -> Vec<String> {
        src.lines()
            .map(str::trim_start)
            .filter(|t| !t.starts_with("//"))
            .filter(|t| t.starts_with('"') && t.contains("=>") && arm_names(t).is_none())
            .map(str::to_string)
            .collect()
    }

    /// Parse `match` arms out of dispatch source text into (pattern names, body) pairs. An arm
    /// starts at a line whose trimmed form is `"name" => ...` or the multi-pattern
    /// `"a" | "b" => ...`; everything up to the next such line belongs to its body, attributed to
    /// EVERY name of the pattern. Comment lines are skipped so prose mentioning
    /// guard_remote_path can never satisfy the check.
    fn parse_dispatch_arms(src: &str) -> Vec<(Vec<String>, String)> {
        let mut arms: Vec<(Vec<String>, String)> = Vec::new();
        for line in src.lines() {
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            if let Some(names) = arm_names(line) {
                arms.push((names, String::new()));
            }
            if let Some((_, body)) = arms.last_mut() {
                body.push_str(line);
                body.push('\n');
            }
        }
        arms
    }

    /// Negative test of the arm parser itself: a multi-pattern arm (`"a" | "b" =>`) must open a NEW
    /// arm attributed to both names — the old parser missed it and silently folded such a body into
    /// the predecessor arm, letting an ungated multi-pattern arm inherit the predecessor's gate.
    /// Commented-out arms must stay invisible.
    #[test]
    fn dispatch_arm_parser_recognizes_multi_pattern_arms() {
        let src = r#"
        match cmd {
            "single" => {
                let path = req_str(args, "path")?;
                guard_remote_path(app, origin, &path)?;
            }
            "multi_a" | "multi_b" => {
                let path = req_str(args, "path")?;
            }
            // "commented_out" => req_str(args, "path")?,
            other => Err(()),
        }
        "#;
        let arms = parse_dispatch_arms(src);
        let names: Vec<&Vec<String>> = arms.iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec![&vec!["single".to_string()], &vec![
            "multi_a".to_string(),
            "multi_b".to_string()
        ]]);
        // The multi-pattern body belongs to its own arm, NOT to the predecessor: it extracts a path
        // and carries no gate, while `single` keeps its gate.
        assert!(arms[0].1.contains("guard_remote_path"));
        assert!(arms[1].1.contains("args, \"path\")"));
        assert!(
            !arms[1].1.contains("guard_remote_path"),
            "the multi-pattern arm must not inherit the predecessor's gate"
        );

        // Pin the chosen behavior for arm shapes the parser does NOT understand (e.g. a future
        // guard arm): they are not silently folded away — the unrecognized-line scan reports them,
        // and the chokepoint turns red until the parser is extended.
        let guarded = r#"
        match cmd {
            "plain" => Ok(()),
            "guarded" if cond => {
                let path = req_str(args, "path")?;
            }
            other => Err(()),
        }
        "#;
        let unrecognized = unrecognized_arm_like_lines(guarded);
        assert_eq!(
            unrecognized,
            vec![r#""guarded" if cond => {"#.to_string()],
            "a guard arm must be surfaced as unrecognized, never silently folded"
        );
        assert!(unrecognized_arm_like_lines(src).is_empty());
    }

    /// FIX for the save_doc_image check/use divergence: the arm gates the EFFECTIVE write target
    /// `<docPath parent>/assets`, not only docPath — an `assets` entry symlinked into the data dir
    /// beside an innocent-looking document would otherwise route the write into the data dir. Normal
    /// documents keep working remotely, and local origins stay unrestricted.
    #[test]
    fn save_doc_image_gates_the_effective_assets_path() {
        let app = test_ctx();
        let data_dir = app.data_dir().unwrap();

        let project = std::env::temp_dir().join(format!("vlx-acl-docimg-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project).unwrap();
        let doc = project.join("note.md");
        std::fs::write(&doc, b"# hi").unwrap();
        let doc_str = doc.to_string_lossy().to_string();

        // Normal remote save outside the data dir keeps working (doc editor image paste).
        let rel = dispatch(
            &app,
            "save_doc_image",
            &json!({ "docPath": doc_str, "bytesB64": "aGk=", "ext": "png" }),
            "ws-1",
            CallOrigin::Remote,
        )
        .expect("remote doc-image save outside the data dir must keep working");
        assert!(rel.as_str().unwrap().starts_with("assets/"));

        // Attack shape (unix): replace the assets dir with a symlink into the data dir. The docPath
        // gate alone passes (the document lies outside), but the effective-assets gate denies.
        #[cfg(unix)]
        {
            let assets = project.join("assets");
            std::fs::remove_dir_all(&assets).unwrap();
            let inside = data_dir.join("planted-assets");
            std::os::unix::fs::symlink(&inside, &assets).unwrap();
            let err = dispatch(
                &app,
                "save_doc_image",
                &json!({ "docPath": doc_str, "bytesB64": "aGk=", "ext": "png" }),
                "ws-1",
                CallOrigin::Remote,
            )
            .unwrap_err();
            assert_eq!(
                err,
                format!("remote_path_forbidden:{}", assets.to_string_lossy()),
                "the effective assets path must be gated, not just docPath"
            );
            assert!(!inside.exists(), "the denied write must not create the directory");

            // Local origins stay unrestricted even through the symlinked assets dir. Pre-create the
            // link target: create_dir_all cannot materialize a directory through a dangling symlink,
            // and this assertion is about the ACL, not filesystem creation semantics.
            std::fs::create_dir_all(&inside).unwrap();
            dispatch(
                &app,
                "save_doc_image",
                &json!({ "docPath": doc_str, "bytesB64": "aGk=", "ext": "png" }),
                DESKTOP_SOURCE,
                CallOrigin::Local,
            )
            .expect("local save through the symlinked assets dir must stay allowed");
        }

        let _ = std::fs::remove_dir_all(&project);
    }

    /// Structural chokepoint for the FIX-5 class (the sweep completeness was claimed twice by prose
    /// and twice wrong: read_file_preview in round 0, git_file_diff in round 1). This test parses the
    /// REAL dispatch source: every match arm that extracts a caller-chosen path-like argument must
    /// either call `guard_remote_path` in its own body or be listed here as a justified exception.
    /// Adding a new path-taking arm without a gate — or gating an arm the exception list still
    /// claims is ungated — fails this test, so completeness is enforced mechanically, not asserted
    /// in comments. Limits: it covers arms in THIS file keyed by the known path-argument names below;
    /// a path smuggled through a differently named argument or a command dispatched outside this
    /// match (pty_spawn in ws.rs) is out of its reach and stays on the reviewer.
    #[test]
    fn remote_path_acl_has_no_ungated_path_arm() {
        const SRC: &str = include_str!("dispatch.rs");
        // Only the production half of the file: everything before the test module.
        let prod = SRC.split("#[cfg(test)]").next().unwrap();

        // Fail loud on any arm-shaped line the parser cannot attribute (guard arms, wrapped
        // patterns): its body would silently inherit the predecessor's gate status otherwise.
        let unrecognized = unrecognized_arm_like_lines(prod);
        assert!(
            unrecognized.is_empty(),
            "dispatch contains arm-like lines the ACL parser does not recognize — extend the \
             parser (arm_names) before relying on this chokepoint: {unrecognized:?}"
        );

        let arms = parse_dispatch_arms(prod);
        assert!(
            arms.len() > 50,
            "the arm parser no longer sees the dispatch match (found {} arms) — fix the parser, \
             do not delete this chokepoint",
            arms.len()
        );

        for (names, body) in &arms {
            if !PATH_KEYS.iter().any(|k| body.contains(k)) {
                continue;
            }
            let gated = body.contains("guard_remote_path");
            // Every pattern of a (possibly multi-pattern) arm must individually be justified when
            // the shared body is ungated; a gated body may not keep stale exception entries.
            for name in names {
                let excepted = UNGATED_JUSTIFIED.contains(&name.as_str());
                assert!(
                    gated || excepted,
                    "dispatch arm `{name}` extracts a caller-chosen path but neither calls \
                     guard_remote_path nor is a justified exception — gate it or add it to \
                     UNGATED_JUSTIFIED with a reviewed reason"
                );
                assert!(
                    !(gated && excepted),
                    "dispatch arm `{name}` is gated but still listed in UNGATED_JUSTIFIED — remove \
                     the stale exception so the list stays accurate"
                );
            }
        }
        // Every exception must still exist as a path-taking arm, so the list cannot rot.
        for name in UNGATED_JUSTIFIED {
            let arm = arms.iter().find(|(names, _)| names.iter().any(|n| n == name));
            let takes_path = arm
                .map(|(_, body)| PATH_KEYS.iter().any(|k| body.contains(k)))
                .unwrap_or(false);
            assert!(
                takes_path,
                "UNGATED_JUSTIFIED entry `{name}` is not a path-taking dispatch arm (anymore) — \
                 remove or fix the entry"
            );
        }
    }

    /// The serve-mode → origin mapping: Electron's loopback sidecar stays Local (reaching 127.0.0.1
    /// implies local shell access), while both network-exposed modes are Remote. Together with the
    /// gating/ACL tests this covers the Electron-loopback lane compositionally.
    #[test]
    fn call_origin_for_serve_mode() {
        use crate::web::ServeMode;
        assert_eq!(CallOrigin::for_serve_mode(ServeMode::LoopbackHttp), CallOrigin::Local);
        assert_eq!(CallOrigin::for_serve_mode(ServeMode::LanTls), CallOrigin::Remote);
        assert_eq!(CallOrigin::for_serve_mode(ServeMode::LanHttp), CallOrigin::Remote);
    }

    /// Mirror mode: a remote client follows and publishes the shared layout, but only a local one may
    /// switch the feature on or off — the checkbox is host-side service configuration.
    #[test]
    fn mirror_commands_gate_only_the_switch() {
        let app = test_ctx();
        // The layout hub is a process-wide singleton; hold the shared test lock for the whole body.
        let _hub = crate::web::mirror::test_guard();

        let pushed = dispatch(
            &app,
            "mirror_push",
            &json!({ "state": { "v": 1, "center": { "openTabs": ["A"] } } }),
            "ws-7",
            CallOrigin::Remote,
        )
        .expect("a remote client must be able to publish its layout");
        // The publisher is stamped from the connection, never from the arguments, so the echo filter on
        // the receiving side cannot be spoofed by a caller claiming to be someone else.
        assert_eq!(pushed["source"], "ws-7");
        assert_eq!(pushed["rev"], 1);

        let got = dispatch(&app, "mirror_get", &json!({}), "ws-7", CallOrigin::Remote)
            .expect("a remote client must be able to read the shared layout");
        assert_eq!(got["state"]["center"]["openTabs"][0], "A");
        assert_eq!(got["enabled"], true, "mirror mode is on unless the host turned it off");

        let denied = dispatch(
            &app,
            "mirror_set_enabled",
            &json!({ "enabled": false }),
            "ws-7",
            CallOrigin::Remote,
        )
        .expect_err("switching mirror mode is management-plane, not available to remote clients");
        assert!(denied.starts_with("remote_cmd_forbidden"), "unexpected error: {denied}");

        dispatch(
            &app,
            "mirror_set_enabled",
            &json!({ "enabled": false }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .expect("the host may switch it");
        let after = dispatch(&app, "mirror_get", &json!({}), DESKTOP_SOURCE, CallOrigin::Local).unwrap();
        assert_eq!(after["enabled"], false);
        // Switching off drops the published layout, so switching back on starts from whoever publishes
        // first rather than restoring an arrangement from hours ago. The revision keeps counting, or a
        // client that stayed connected would dismiss every later push as a stale frame.
        assert_eq!(after["state"], serde_json::Value::Null);
        assert_eq!(after["rev"], 1);
    }

    /// get_app_settings hides the security-relevant keys (remoteAccess.* verifier/autostart config and
    /// the plaintext gitea.token fallback) from remote clients while local callers keep the full map.
    #[test]
    fn get_app_settings_filters_protected_keys_for_remote() {
        let app = test_ctx();
        crate::command_core::set_app_settings(
            &app,
            std::collections::HashMap::from([
                ("remoteAccess.passwordHash".to_string(), "$argon2id$fake".to_string()),
                ("remoteAccess.port".to_string(), "9123".to_string()),
                ("gitea.token".to_string(), "secret-token".to_string()),
                ("vlx-theme".to_string(), "dark".to_string()),
            ]),
        )
        .unwrap();

        let remote = dispatch(&app, "get_app_settings", &json!({}), "ws-1", CallOrigin::Remote)
            .expect("remote settings read must succeed, just filtered");
        assert_eq!(remote["vlx-theme"], "dark");
        assert!(remote.get("remoteAccess.passwordHash").is_none());
        assert!(remote.get("remoteAccess.port").is_none());
        assert!(remote.get("gitea.token").is_none());

        let local = dispatch(&app, "get_app_settings", &json!({}), DESKTOP_SOURCE, CallOrigin::Local)
            .expect("local settings read must stay unfiltered");
        assert_eq!(local["remoteAccess.passwordHash"], "$argon2id$fake");
        assert_eq!(local["gitea.token"], "secret-token");
        assert_eq!(local["vlx-theme"], "dark");
    }

    /// set_app_settings rejects a remote batch containing any protected key — and the database stays
    /// untouched, including the unprotected keys of the same batch — while plain remote writes and local
    /// writes of protected keys keep working.
    #[test]
    fn set_app_settings_rejects_protected_keys_for_remote() {
        let app = test_ctx();

        // A remote batch mixing a protected key is rejected as a whole.
        let err = dispatch(
            &app,
            "set_app_settings",
            &json!({ "entries": { "remoteAccess.port": "1", "vlx-theme": "dark" } }),
            "ws-1",
            CallOrigin::Remote,
        )
        .unwrap_err();
        assert_eq!(err, "remote_setting_forbidden:remoteAccess.port");
        let settings = crate::command_core::get_app_settings(&app).unwrap();
        assert!(settings.get("remoteAccess.port").is_none(), "rejected write must not persist");
        assert!(settings.get("vlx-theme").is_none(), "a rejected batch must persist nothing");

        // gitea.token is protected by exact key.
        let err = dispatch(
            &app,
            "set_app_settings",
            &json!({ "entries": { "gitea.token": "x" } }),
            "ws-1",
            CallOrigin::Remote,
        )
        .unwrap_err();
        assert_eq!(err, "remote_setting_forbidden:gitea.token");

        // Unprotected remote writes still work.
        dispatch(
            &app,
            "set_app_settings",
            &json!({ "entries": { "vlx-theme": "light" } }),
            "ws-1",
            CallOrigin::Remote,
        )
        .expect("remote write of unprotected keys must succeed");
        // Local callers may write protected keys (the desktop panel persists the remote-access config).
        dispatch(
            &app,
            "set_app_settings",
            &json!({ "entries": { "remoteAccess.port": "9123" } }),
            DESKTOP_SOURCE,
            CallOrigin::Local,
        )
        .expect("local write of protected keys must succeed");
        let settings = crate::command_core::get_app_settings(&app).unwrap();
        assert_eq!(settings.get("vlx-theme").map(String::as_str), Some("light"));
        assert_eq!(settings.get("remoteAccess.port").map(String::as_str), Some("9123"));
    }
}
