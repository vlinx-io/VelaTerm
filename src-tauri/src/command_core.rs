//! Transport-independent command core. The actual orchestration for session, tree, and session-content commands—
//! acquiring connections, calling repositories, emitting events, and applying side effects—lives **once** here.
//!
//! Both transports are thin parameter -> core -> response adapters:
//! - Desktop Tauri commands in `commands.rs` use `AppCtx::Tauri(app)`.
//! - Browser/Electron WebSocket dispatch in `web/dispatch.rs` uses the same `AppCtx`.
//!
//! New commands therefore define orchestration once and cannot drift between transports. Previous duplication
//! caused desktop-only recording cleanup in `delete_node` and desktop-only empty-string normalization in
//! `create_session`; centralization keeps behavior consistent.
//!
//! Scope is limited to stateful, side-effecting, or multi-step commands. Stateless one-line forwarding such as
//! `files::*`/`git::*` still calls lower layers directly, and transport-specific streaming commands such as
//! `pty_spawn` and `read_recording` remain outside this module.

use crate::db::repo;
use crate::host::{AppCtx, PRESETS_CHANGED, TREE_CHANGED};
use crate::models::{AgentPreset, Group, NodeKind, Project, Session, SessionKind, Tree};

// Private helpers.

/// Reads a session's `(kind, captured agent-native session ID)`, returning a specific error if either is absent.
/// Shared by the four transcript/usage commands for consistent lookup and error messages.
fn session_kind_and_agent(ctx: &AppCtx, session_id: &str) -> Result<(SessionKind, String), String> {
    let (kind, agent_id) = {
        let conn = ctx.db().conn.lock().unwrap();
        (
            repo::get_session_kind(&conn, session_id)?,
            repo::get_agent_session_id(&conn, session_id)?,
        )
    };
    let kind = kind.ok_or("Session not found")?;
    let agent_id = agent_id.ok_or("No agent session id captured for this session yet")?;
    Ok((kind, agent_id))
}

/// Normalizes empty strings to None so blank values are not persisted.
fn empty_to_none(v: Option<&str>) -> Option<&str> {
    v.filter(|s| !s.trim().is_empty())
}

// Tree management. Successful writes emit `TREE_CHANGED`; clients debounce tree reloads for cross-client sync.

/// Imports a directory as a project.
pub fn import_project(ctx: &AppCtx, root_path: &str) -> Result<Project, String> {
    let project = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::import_project(&conn, root_path)?
    };
    ctx.emit(TREE_CHANGED, ());
    Ok(project)
}

/// Clones a remote repository under `parent_dir` and imports it as a project. Derives the directory name from the
/// repository when `folder_name` is empty and clones a specific `branch` when provided (see `git::clone_to`).
pub fn clone_project(
    ctx: &AppCtx,
    url: &str,
    parent_dir: &str,
    folder_name: Option<&str>,
    branch: Option<&str>,
    operation_id: &str,
    source: &str,
) -> Result<Project, String> {
    let cloned = crate::git::clone_to_with_progress(
        url,
        parent_dir,
        folder_name,
        branch,
        operation_id,
        source,
        |progress| ctx.emit(crate::git::CLONE_PROGRESS_EVENT, progress),
    )?;
    ctx.emit(
        crate::git::CLONE_PROGRESS_EVENT,
        crate::git::CloneProgress {
            operation_id: operation_id.to_string(),
            stage: "importing".to_string(),
            percent: None,
        },
    );
    let project = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::import_project(&conn, &cloned)?
    };
    ctx.emit(TREE_CHANGED, ());
    Ok(project)
}

/// Returns the complete project/group/session tree.
pub fn list_tree(ctx: &AppCtx) -> Result<Tree, String> {
    let conn = ctx.db().conn.lock().unwrap();
    repo::list_tree(&conn)
}

/// Creates a group/subgroup. When worktree path/base ref are present, the group owns that worktree, displays its
/// tag in the sidebar, and supplies it as the default for new sessions within the group.
pub fn create_group(
    ctx: &AppCtx,
    project_id: &str,
    parent_group_id: Option<&str>,
    name: &str,
    worktree_path: Option<&str>,
    worktree_base_ref: Option<&str>,
) -> Result<Group, String> {
    let group = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::create_group_full(
            &conn,
            project_id,
            parent_group_id,
            name,
            worktree_path,
            worktree_base_ref,
        )?
    };
    ctx.emit(TREE_CHANGED, ());
    Ok(group)
}

// Agent presets. Successful writes emit `PRESETS_CHANGED` so every client refreshes its new-session menu.

/// List every agent preset in menu order.
pub fn list_agent_presets(ctx: &AppCtx) -> Result<Vec<AgentPreset>, String> {
    let conn = ctx.db().conn.lock().unwrap();
    repo::list_agent_presets(&conn)
}

/// Create an agent preset from a launch configuration the user chose to keep.
pub fn create_agent_preset(
    ctx: &AppCtx,
    name: &str,
    base_kind: SessionKind,
    exec_path: Option<&str>,
    agent_args: Option<&str>,
    permission_mode: Option<&str>,
    icon: Option<&str>,
) -> Result<AgentPreset, String> {
    let preset = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::create_agent_preset(
            &conn,
            name,
            base_kind,
            empty_to_none(exec_path),
            empty_to_none(agent_args),
            empty_to_none(permission_mode),
            empty_to_none(icon),
        )?
    };
    ctx.emit(PRESETS_CHANGED, ());
    Ok(preset)
}

/// Update an agent preset. Sessions already created from it keep their own copied launch values.
pub fn update_agent_preset(
    ctx: &AppCtx,
    id: &str,
    name: &str,
    exec_path: Option<&str>,
    agent_args: Option<&str>,
    permission_mode: Option<&str>,
    icon: Option<&str>,
) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::update_agent_preset(
            &conn,
            id,
            name,
            empty_to_none(exec_path),
            empty_to_none(agent_args),
            empty_to_none(permission_mode),
            empty_to_none(icon),
        )?;
    }
    ctx.emit(PRESETS_CHANGED, ());
    Ok(())
}

/// Delete an agent preset. Sessions created from it keep launching exactly as before.
pub fn delete_agent_preset(ctx: &AppCtx, id: &str) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::delete_agent_preset(&conn, id)?;
    }
    ctx.emit(PRESETS_CHANGED, ());
    Ok(())
}

/// Persist a new menu order from the full list of preset IDs.
pub fn reorder_agent_presets(ctx: &AppCtx, ids: &[String]) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::reorder_agent_presets(&conn, ids)?;
    }
    ctx.emit(PRESETS_CHANGED, ());
    Ok(())
}

/// Creates a session, optionally prepopulating an agent-session resume anchor.
#[allow(clippy::too_many_arguments)]
pub fn create_session(
    ctx: &AppCtx,
    project_id: &str,
    group_id: Option<&str>,
    name: &str,
    kind: SessionKind,
    shell: Option<&str>,
    cwd: Option<&str>,
    init_cmd: Option<&str>,
    parent_session_id: Option<&str>,
    worktree_path: Option<&str>,
    agent_args: Option<&str>,
    permission_mode: Option<&str>,
    agent_session_id: Option<&str>,
    worktree_base_ref: Option<&str>,
    agent_preset_id: Option<&str>,
    agent_path: Option<&str>,
) -> Result<Session, String> {
    // Normalize empty strings consistently across transports.
    let agent_args = empty_to_none(agent_args);
    let permission_mode = empty_to_none(permission_mode);
    let worktree_base_ref = empty_to_none(worktree_base_ref);
    let agent_preset_id = empty_to_none(agent_preset_id);
    let agent_path = empty_to_none(agent_path);
    let session = {
        let conn = ctx.db().conn.lock().unwrap();
        let mut session = repo::create_session_full(
            &conn,
            project_id,
            group_id,
            name,
            kind,
            shell,
            cwd,
            init_cmd,
            parent_session_id,
            worktree_path,
            agent_args,
            permission_mode,
            worktree_base_ref,
            agent_preset_id,
            agent_path,
        )?;
        let seeded_agent_id = agent_session_id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            // Zoo and Grok accept caller-provided UUIDs. Reuse the VelaTerm session ID so parallel sessions in
            // one directory never attach to each other's most recent conversation; first launch creates it.
            .or_else(|| {
                matches!(kind, SessionKind::Zoo | SessionKind::Grok).then(|| session.id.clone())
            });
        if let Some(aid) = seeded_agent_id {
            // Reuse the kind-guarded update so only the newly created matching session is persisted.
            repo::set_agent_session_id(&conn, &session.id, &aid, kind)?;
            session.agent_session_id = Some(aid);
        }
        session
    };
    ctx.emit(TREE_CHANGED, ());
    Ok(session)
}

/// Converts to a permanent session using the frontend-provided ID. Keeping the PTY-indexed ID avoids restarting
/// the process or losing context (architecture section 7).
#[allow(clippy::too_many_arguments)]
pub fn persist_session(
    ctx: &AppCtx,
    id: &str,
    project_id: &str,
    group_id: Option<&str>,
    name: &str,
    kind: SessionKind,
    shell: Option<&str>,
    cwd: Option<&str>,
    init_cmd: Option<&str>,
    parent_session_id: Option<&str>,
) -> Result<Session, String> {
    let session = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::persist_session(
            &conn,
            id,
            project_id,
            group_id,
            name,
            kind,
            shell,
            cwd,
            init_cmd,
            parent_session_id,
        )?
    };
    ctx.emit(TREE_CHANGED, ());
    Ok(session)
}

/// Forks a new conversation from current source history, supported for Claude/Codex with an existing conversation ID.
pub fn fork_session(ctx: &AppCtx, session_id: &str) -> Result<Session, String> {
    let session = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::fork_session(&conn, session_id)?
    };
    ctx.emit(TREE_CHANGED, ());
    Ok(session)
}

/// Updates session configuration, normalizing empty fields to None.
#[allow(clippy::too_many_arguments)]
pub fn update_session(
    ctx: &AppCtx,
    id: &str,
    name: &str,
    shell: Option<&str>,
    cwd: Option<&str>,
    init_cmd: Option<&str>,
    agent_args: Option<&str>,
    permission_mode: Option<&str>,
) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::update_session(
            &conn,
            id,
            name,
            empty_to_none(shell),
            empty_to_none(cwd),
            empty_to_none(init_cmd),
            empty_to_none(agent_args),
            empty_to_none(permission_mode),
        )?;
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Renames a node.
pub fn rename_node(ctx: &AppCtx, kind: NodeKind, id: &str, name: &str) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::rename_node(&conn, kind, id, name)?;
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Sets or clears a node's emoji marker. An empty marker clears it; the tree broadcast lets every attached client
/// redraw the sidebar row.
pub fn set_node_mark(
    ctx: &AppCtx,
    kind: NodeKind,
    id: &str,
    mark: Option<&str>,
) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_node_mark(&conn, kind, id, mark)?;
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Converts a node to a normal session/group after its worktree directory is deleted by clearing worktree bindings.
/// Sessions also clear cwd so they start from project root. See `repo::clear_node_worktree`.
pub fn clear_node_worktree(ctx: &AppCtx, kind: NodeKind, id: &str) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::clear_node_worktree(&conn, kind, id)?;
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Deletes a node while **preserving archived sessions**. Deleting a group/project leaves archived descendants
/// behind a soft-deleted parent tombstone; deleting a live parent session reparents archived children to the nearest
/// group. `repo::delete_node` returns only physically deleted session IDs, which are used to remove search-index
/// entries and recordings. Preserved archives and their recordings remain intact. See `repo::delete_node`.
pub fn delete_node(ctx: &AppCtx, kind: NodeKind, id: &str) -> Result<(), String> {
    let hard_deleted = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::delete_node(&conn, kind, id)?
    };
    if !hard_deleted.is_empty() {
        // Clear the shared full-text index regardless of which client initiated deletion.
        let _ = crate::search::index::drop_sessions(ctx.db(), &hard_deleted);
        // Remove recordings consistently, including Electron over WS, so orphaned .log files do not remain.
        if let Ok(data_dir) = ctx.data_dir() {
            let dir = data_dir.join("recordings");
            for sid in &hard_deleted {
                let _ = std::fs::remove_file(dir.join(format!("{sid}.log")));
            }
        }
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Archives or restores a session through soft hiding without deleting data.
pub fn set_session_archived(ctx: &AppCtx, id: &str, archived: bool) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_archived(&conn, id, archived)?;
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Archives an entire group: archive its sessions and retain the group as a hidden soft-deletion tombstone. Restoring
/// any session restores the group. See `repo::archive_group`.
pub fn archive_group(ctx: &AppCtx, id: &str) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::archive_group(&conn, id)?;
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Lists all archived sessions newest-first for the archive browser.
pub fn list_archived_sessions(ctx: &AppCtx) -> Result<Vec<Session>, String> {
    let conn = ctx.db().conn.lock().unwrap();
    repo::list_archived(&conn)
}

/// Moves a node to a new parent and assigns its order.
pub fn move_node(
    ctx: &AppCtx,
    kind: NodeKind,
    id: &str,
    target_project_id: Option<&str>,
    target_group_id: Option<&str>,
    target_parent_session_id: Option<&str>,
    sort_order: i64,
) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::move_node(
            &conn,
            kind,
            id,
            target_project_id,
            target_group_id,
            target_parent_session_id,
            sort_order,
        )?;
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Persists collapsed/expanded state.
pub fn set_collapsed(
    ctx: &AppCtx,
    kind: NodeKind,
    id: &str,
    collapsed: bool,
) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_collapsed(&conn, kind, id, collapsed)?;
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Remembers the last URL visited by a browser node (architecture section 17). Frontend navigation writes back
/// with debounce and deliberately does **not** broadcast tree://changed because updates are frequent and do not
/// change structure; other clients see the value on their next loadTree. Shared by Tauri and WS/Electron/remote.
pub fn set_browser_url(ctx: &AppCtx, id: &str, url: &str) -> Result<(), String> {
    let conn = ctx.db().conn.lock().unwrap();
    repo::set_browser_url(&conn, id, url)
}

/// Reads all application preferences (theme, language, appearance, shortcuts, sound, etc.) shared by Tauri and
/// Electron. Both Tauri and sidecar WS dispatch use this implementation; see the app_settings schema comment.
pub fn get_app_settings(ctx: &AppCtx) -> Result<std::collections::HashMap<String, String>, String> {
    let conn = ctx.db().conn.lock().unwrap();
    repo::get_app_settings(&conn)
}

/// app_settings key holding this installation's anonymous identifier.
const INSTALL_ID_KEY: &str = "install_id";

/// Return this installation's anonymous identifier, generating and persisting one on first call.
///
/// A random UUID with no link to the machine, the user, or any account: it exists so update-check
/// telemetry can count distinct installations instead of distinct IP addresses, which both merges
/// everyone behind one NAT and splits a single user across a changing home address. It lives in
/// `app_settings`, so it is per data directory — development and release builds have separate
/// databases and therefore separate identifiers, which is what we want.
pub fn install_id(ctx: &AppCtx) -> Result<String, String> {
    let conn = ctx.db().conn.lock().unwrap();
    repo::get_or_create_app_setting(
        &conn,
        INSTALL_ID_KEY,
        &uuid::Uuid::new_v4().to_string(),
    )
}

/// Batch-upserts application preferences by key with last-write-wins semantics. This is the authoritative backend
/// side of frontend local-cache dual writes, used for startup seeding and debounced setting changes.
pub fn set_app_settings(
    ctx: &AppCtx,
    entries: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let conn = ctx.db().conn.lock().unwrap();
    repo::set_app_settings(&conn, &entries)
}

/// Immediately clears temporary pasted images and returns `{removed, freedBytes}` for display. Although independent
/// of context, it goes through dispatch for consistent desktop, remote, and Electron behavior.
pub fn clean_pasted_images() -> serde_json::Value {
    let (removed, freed) = crate::files::purge_pasted_images();
    serde_json::json!({ "removed": removed, "freedBytes": freed })
}

/// Lists worktree paths associated with a session and all descendants for deletion confirmation.
pub fn worktrees_in_subtree(ctx: &AppCtx, session_id: &str) -> Result<Vec<String>, String> {
    let conn = ctx.db().conn.lock().unwrap();
    repo::worktree_paths_in_subtree(&conn, session_id)
}

/// Reads a session's (base ref, worktree directory), returning an error when absent. Shared by landing commands.
fn session_worktree(ctx: &AppCtx, session_id: &str) -> Result<(Option<String>, String), String> {
    let conn = ctx.db().conn.lock().unwrap();
    let s = repo::get_session(&conn, session_id)?.ok_or("Session not found")?;
    let wt_path = s
        .worktree_path
        .filter(|p| !p.trim().is_empty())
        .ok_or("Session has no worktree")?;
    Ok((s.worktree_base_ref, wt_path))
}

/// Current shell identifier, also the data-directory suffix. It isolates development and release Gitea Keychain
/// services. A fixed fallback keeps the service name stable if detection fails.
fn app_identifier(ctx: &AppCtx) -> String {
    ctx.data_dir()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "io.vlinx.vlxterm".to_string())
}

/// Returns Gitea integration status for settings without exposing the token.
pub fn gitea_get_status(ctx: &AppCtx) -> crate::gitea::GiteaStatus {
    crate::gitea::status(ctx.db(), &app_identifier(ctx))
}

/// Saves Gitea base URL in app_settings and token in Keychain/plaintext fallback, then returns updated status.
pub fn gitea_set_config(
    ctx: &AppCtx,
    base_url: &str,
    token: &str,
) -> Result<crate::gitea::GiteaStatus, String> {
    crate::gitea::set_config(ctx.db(), &app_identifier(ctx), base_url, token)?;
    Ok(crate::gitea::status(ctx.db(), &app_identifier(ctx)))
}

/// Tests `/api/v1/version` for platform identity and `/api/v1/user` for token validity without side effects.
pub fn gitea_probe(base_url: &str, token: &str) -> crate::gitea::GiteaProbe {
    crate::gitea::probe(base_url, token)
}

/// Lands through Gitea by pushing the worktree branch to origin and opening a PR against the short base name.
pub fn land_gitea_pr(
    ctx: &AppCtx,
    session_id: &str,
    title: &str,
    body: &str,
) -> Result<crate::gitea::PrOutcome, String> {
    let (base_ref, wt_path) = session_worktree(ctx, session_id)?;
    // Reuse local landing's base resolution, including fallback to the main worktree branch for legacy records.
    let targets = crate::git::land_targets(base_ref.as_deref(), &wt_path)?;
    crate::gitea::open_pr(
        ctx.db(),
        &app_identifier(ctx),
        &wt_path,
        &targets.branch,
        &targets.base_ref,
        title,
        body,
    )
}

// SSH host history and remembered passwords. These commands are **desktop-only** because Connect Remote is hidden
// from the remote web UI. Gate them and `crate::ssh_remote` behind the gui feature so headless builds omit them.

/// Lists previously connected SSH hosts newest-first, including whether each password is stored in Keychain.
#[cfg(feature = "gui")]
pub fn ssh_hosts_list(ctx: &AppCtx) -> Result<Vec<crate::ssh_remote::SshHostInfo>, String> {
    let id = app_identifier(ctx);
    // Release the database lock before potentially slow per-host Keychain reads.
    let rows = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::list_ssh_hosts(&conn)?
    };
    Ok(rows
        .into_iter()
        .map(|(target, label, last_connected_at, shared_db)| {
            let has_password = crate::ssh_remote::has_password(&id, "ssh", &target);
            crate::ssh_remote::SshHostInfo {
                target,
                label,
                last_connected_at,
                has_password,
                shared_db,
            }
        })
        .collect())
}

/// Forgets a host by deleting its history row and remembered Keychain password.
#[cfg(feature = "gui")]
pub fn ssh_host_forget(ctx: &AppCtx, target: &str) -> Result<(), String> {
    crate::ssh_remote::delete_password(&app_identifier(ctx), "ssh", target);
    let conn = ctx.db().conn.lock().unwrap();
    repo::delete_ssh_host(&conn, target)
}

// Paired-URL history and remembered login passwords, also desktop-only because remote web UI cannot open remotes.

/// Recent paired-URL entry returned to the frontend, including whether its login password is in Keychain.
#[cfg(feature = "gui")]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlHostInfo {
    /// Pairing URL including its #pair fragment, also used as the Keychain account name.
    pub url: String,
    /// Whether the system Keychain stores a login password for this URL.
    pub has_password: bool,
}

/// Lists opened pairing URLs newest-first with remembered-password status.
#[cfg(feature = "gui")]
pub fn url_hosts_list(ctx: &AppCtx) -> Result<Vec<UrlHostInfo>, String> {
    let id = app_identifier(ctx);
    let urls = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::list_url_hosts(&conn)?
    };
    Ok(urls
        .into_iter()
        .map(|url| {
            let has_password = crate::ssh_remote::has_password(&id, "url", &url);
            UrlHostInfo { url, has_password }
        })
        .collect())
}

/// Reads a pairing URL's remembered password for automatic login, returning None when absent.
#[cfg(feature = "gui")]
pub fn url_host_password(ctx: &AppCtx, url: &str) -> Option<String> {
    crate::ssh_remote::load_password(&app_identifier(ctx), "url", url)
}

/// Records a successfully opened pairing URL. If `remember` and a password are provided, store it only in the
/// `{identifier}.url` Keychain service, never the database. A successful connection with `remember=false` removes
/// any existing password, matching the frontend's restored checkbox semantics.
#[cfg(feature = "gui")]
pub fn url_host_record(
    ctx: &AppCtx,
    url: &str,
    password: Option<&str>,
    remember: bool,
) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::upsert_url_host(&conn, url, now)?;
    }
    if remember {
        if let Some(pw) = password.filter(|p| !p.is_empty()) {
            crate::ssh_remote::store_password(&app_identifier(ctx), "url", url, pw);
        }
    } else {
        crate::ssh_remote::delete_password(&app_identifier(ctx), "url", url);
    }
    Ok(())
}

/// Forgets a pairing URL by deleting its history and remembered Keychain password.
#[cfg(feature = "gui")]
pub fn url_host_forget(ctx: &AppCtx, url: &str) -> Result<(), String> {
    crate::ssh_remote::delete_password(&app_identifier(ctx), "url", url);
    let conn = ctx.db().conn.lock().unwrap();
    repo::delete_url_host(&conn, url)
}

// Session content: search, transcripts, and usage.

/// Global session-content search; omitted `scope` defaults to `live` and excludes archives.
pub fn search_session_content(
    ctx: &AppCtx,
    query: &str,
    scope: Option<&str>,
) -> Result<Vec<crate::search::SessionSearchHit>, String> {
    let recordings_dir = ctx.data_dir()?.join("recordings");
    let scope = crate::search::SearchScope::from_arg(scope.unwrap_or("live"));
    // Pass &Db rather than a locked connection; search refreshes outside the lock and writes in short transactions.
    crate::search::search_sessions(ctx.db(), &recordings_dir, query, scope)
}

/// Reads an agent transcript for the archive-panel viewer.
pub fn read_agent_transcript(
    ctx: &AppCtx,
    session_id: &str,
) -> Result<Vec<crate::agent::transcript::TranscriptMessage>, String> {
    let (kind, agent_id) = session_kind_and_agent(ctx, session_id)?;
    crate::agent::transcript::read(kind, &agent_id)
}

/// Queries model/context usage for the Info panel (Claude, Codex, Grok).
pub fn agent_context_info(
    ctx: &AppCtx,
    session_id: &str,
) -> Result<crate::agent::transcript::AgentContextInfo, String> {
    let (kind, agent_id) = session_kind_and_agent(ctx, session_id)?;
    crate::agent::transcript::context_info(kind, &agent_id)
}

/// Queries current-turn tokens, tool calls, and changed files for the Info panel, Claude only.
pub fn agent_turn_stats(
    ctx: &AppCtx,
    session_id: &str,
) -> Result<crate::agent::transcript::TurnStats, String> {
    let (kind, agent_id) = session_kind_and_agent(ctx, session_id)?;
    crate::agent::transcript::current_turn_stats(kind, &agent_id)
}

/// Queries live account rate limits for a Codex session, falling back to local rollout data.
pub fn codex_usage(
    ctx: &AppCtx,
    session_id: &str,
    live: bool,
) -> Result<crate::agent::transcript::CodexUsage, String> {
    let session = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::get_session(&conn, session_id)?
    }
    .ok_or("Session not found")?;
    if !matches!(session.kind, SessionKind::Codex) {
        return Err("Only codex sessions support rate limits".to_string());
    }

    // app-server account/rateLimits/read fetches current account limits, while rollout is a historical snapshot.
    // Prefer live data for initial, periodic, and manual refreshes; fall back when older CLIs or networking fail.
    if live {
        // Packaged macOS GUI PATH often omits global npm bins. Prefer user configuration, then installation
        // detection for an absolute path, and only finally let Command resolve `codex` through process PATH.
        let bin_path = crate::pty::manager::agent_bin_path(ctx, SessionKind::Codex)
            .or_else(|| crate::agent::install::locate_installed_bin("codex"));
        if let Ok(usage) = crate::agent::transcript::live_codex_rate_limits(bin_path.as_deref()) {
            return Ok(usage);
        }
    }

    if let Some(agent_id) = session.agent_session_id.as_deref() {
        if let Ok(usage) = crate::agent::transcript::codex_rate_limits(session.kind, agent_id) {
            return Ok(usage);
        }
    }

    // Usage is account-level. If an older Codex session missed the startup capture window,
    // do a conservative cwd-matched repair before falling back to the latest local snapshot.
    if session.agent_session_id.is_none() {
        if let Some(cwd) = session.cwd.as_deref().or(session.worktree_path.as_deref()) {
            let created = session.created_at.max(0) as u64;
            let since =
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(created.saturating_sub(10));
            if let Some(agent_id) =
                crate::agent::resume::capture_codex_session_since(Some(cwd), since)
            {
                let changed = {
                    let conn = ctx.db().conn.lock().unwrap();
                    repo::claim_codex_session_id(&conn, &session.id, &agent_id)?
                };
                if changed {
                    ctx.emit(TREE_CHANGED, ());
                }
                if let Ok(usage) =
                    crate::agent::transcript::codex_rate_limits(session.kind, &agent_id)
                {
                    return Ok(usage);
                }
            }
        }
    }

    crate::agent::transcript::latest_codex_rate_limits()
}

/// Exports complete session context as Markdown. With desktop `dest_path`, writes to disk and returns None; without
/// it on browser clients, returns Some(content) for a local frontend download.
pub fn export_session_context(
    ctx: &AppCtx,
    session_id: &str,
    dest_path: Option<&str>,
    exported_at: Option<&str>,
) -> Result<Option<String>, String> {
    let (kind, agent_id, name) = {
        let conn = ctx.db().conn.lock().unwrap();
        (
            repo::get_session_kind(&conn, session_id)?,
            repo::get_agent_session_id(&conn, session_id)?,
            repo::get_session_name(&conn, session_id)?,
        )
    };
    let kind = kind.ok_or("Session not found")?;
    let agent_id =
        agent_id.ok_or("No agent session id captured for this session yet; nothing to export")?;
    let name = name.unwrap_or_else(|| "Session".to_string());
    let md = crate::agent::export::export_markdown(kind, &agent_id, &name, exported_at)?;
    match dest_path {
        Some(p) => {
            std::fs::write(p, md).map_err(|e| format!("Failed to write export file: {e}"))?;
            Ok(None)
        }
        None => Ok(Some(md)),
    }
}

// Remote-access web service. WS/headless dispatch toggles LAN remote mode through `ctx.remote_web()`. Desktop
// Tauri commands access the same managed WebServer directly. LAN remote mode uses 0.0.0.0 with self-signed TLS,
// independently of Electron sidecar's persistent plaintext loopback instance.
//
// The run state persists in app_settings so a restart can restore it (GitHub issue #15): enabled flag, port,
// mode, and an Argon2id PHC hash of the password — never the plaintext. Persistence lives only here in the
// core; the transport adapters (commands.rs / dispatch.rs) stay thin.

/// app_settings key: "1" while remote access should auto-start on launch, "0" after a manual stop.
const REMOTE_ENABLED_KEY: &str = "remoteAccess.enabled";
/// app_settings key: last successfully used listening port (decimal string).
const REMOTE_PORT_KEY: &str = "remoteAccess.port";
/// app_settings key: "1" when the last start used plaintext LAN HTTP mode (dev only), otherwise "0".
const REMOTE_LAN_HTTP_KEY: &str = "remoteAccess.lanHttp";
/// app_settings key: Argon2id PHC verifier of the access password; the only password-derived value on disk.
const REMOTE_PASSWORD_HASH_KEY: &str = "remoteAccess.passwordHash";

/// Persist remote-access settings; failures are logged and never abort the running service.
fn persist_remote_settings(ctx: &AppCtx, entries: std::collections::HashMap<String, String>) {
    if let Err(e) = set_app_settings(ctx, entries) {
        eprintln!("failed to persist remote-access settings: {e}");
    }
}

/// Shared production guard: plaintext LAN mode exists only for development mobile-device tests.
fn lan_http_guard(ctx: &AppCtx, lan_http: bool) -> Result<(), String> {
    if lan_http && crate::web::is_production_identifier(&app_identifier(ctx)) {
        return Err(
            "LAN plaintext mode is only available in dev builds, not in release builds (use HTTPS with certificate fingerprint pinning on production devices; see architecture §20)."
                .to_string(),
        );
    }
    Ok(())
}

/// Starts LAN remote access with password login and session gating. `lan_http=false` binds 0.0.0.0 with TLS;
/// `lan_http=true` provides plaintext for native mobile shells that cannot bypass self-signed certificates in RN
/// WebView (architecture section 20). Runtime stops any old instance before starting so changes take effect.
/// After a successful start, the enabled flag, port, mode, and password hash persist so the next launch
/// auto-starts the same configuration.
pub fn web_server_start(
    ctx: &AppCtx,
    password: &str,
    port: Option<u16>,
    lan_http: bool,
) -> Result<crate::web::WebServerStatus, String> {
    let mode = if lan_http {
        crate::web::ServeMode::LanHttp
    } else {
        crate::web::ServeMode::LanTls
    };
    // Production forbids plaintext LAN binding. The GUI hides this mode, and this guard blocks programmatic calls.
    lan_http_guard(ctx, lan_http)?;
    if password.trim().is_empty() {
        return Err("Please set an access password first".into());
    }
    // Hash here in the core so the same PHC string both starts the server and gets persisted for auto-start.
    let phc = crate::web::hash_password(password)?;
    let status = ctx.remote_web().start(
        ctx.clone(),
        crate::web::StartAuth::PasswordHash(phc.clone()),
        port,
        mode,
    )?;
    persist_remote_settings(
        ctx,
        std::collections::HashMap::from([
            (REMOTE_ENABLED_KEY.to_string(), "1".to_string()),
            (
                REMOTE_PORT_KEY.to_string(),
                status.port.map(|p| p.to_string()).unwrap_or_default(),
            ),
            (
                REMOTE_LAN_HTTP_KEY.to_string(),
                if lan_http { "1" } else { "0" }.to_string(),
            ),
            (REMOTE_PASSWORD_HASH_KEY.to_string(), phc),
        ]),
    );
    Ok(status)
}

/// Stops LAN remote access and disables auto-start. Port and password hash stay persisted for prefill;
/// stop-then-start deliberately requires retyping the password (which overwrites the hash).
pub fn web_server_stop(ctx: &AppCtx) -> Result<(), String> {
    ctx.remote_web().stop();
    persist_remote_settings(
        ctx,
        std::collections::HashMap::from([(REMOTE_ENABLED_KEY.to_string(), "0".to_string())]),
    );
    Ok(())
}

/// app_settings key: "1" (or absent, the default) while mirror mode is on, "0" once the host turns it off.
const MIRROR_ENABLED_KEY: &str = "remoteAccess.mirror";

/// Whether mirror mode is on. Absent means on: the checkbox in the remote-access panel ships checked, so a
/// database written before this feature existed must read as enabled rather than silently opting out.
fn mirror_enabled(ctx: &AppCtx) -> bool {
    get_app_settings(ctx)
        .ok()
        .and_then(|s| s.get(MIRROR_ENABLED_KEY).cloned())
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Current mirror mode plus the published layout, for a client aligning itself right after it connects.
pub fn mirror_get(ctx: &AppCtx) -> serde_json::Value {
    let snap = crate::web::mirror::current();
    let mut out = snap.to_json();
    out["enabled"] = serde_json::Value::Bool(mirror_enabled(ctx));
    out
}

/// Publish `state` as the shared layout on behalf of the calling client and broadcast it to every other one.
///
/// `source` is the caller's connection ID, supplied by the transport rather than the caller's arguments, so a
/// client cannot forge someone else's identity and thereby suppress their echo filter. The push is stored even
/// when mirror mode is off — clients stop pushing on their own — so the last arrangement is never half-written.
pub fn mirror_push(ctx: &AppCtx, source: &str, state: serde_json::Value) -> serde_json::Value {
    let snap = crate::web::mirror::push(source, state);
    let payload = snap.to_json();
    ctx.emit(crate::web::mirror::LAYOUT_EVENT, payload.clone());
    payload
}

/// How long an answered spawn request is remembered. Long enough that a client which was offline
/// during the answer cannot revive the card by answering it later, short enough that the table stays
/// small in a session running for days.
const SPAWN_CLAIM_TTL: std::time::Duration = std::time::Duration::from_secs(600);

/// Spawn requests already answered, keyed by parent session and prompt, with the time they were claimed.
fn spawn_claims() -> &'static std::sync::Mutex<
    std::collections::HashMap<String, std::time::Instant>,
> {
    static CLAIMS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>,
    > = std::sync::OnceLock::new();
    CLAIMS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Forget a request's claim, so a fresh card for the same task can be answered again.
///
/// Called when the agent issues the request. An agent that retries a task the user just cancelled sends
/// the identical parent and prompt, and without this the new card would look like an answered one and
/// confirming it would silently do nothing.
pub fn release_spawn_claim(parent_session_id: &str, prompt: &str) {
    spawn_claims()
        .lock()
        .unwrap()
        .remove(&spawn_claim_key(parent_session_id, prompt));
}

/// Claim one spawn request, returning true only for the first caller.
///
/// The confirmation card appears on every connected client, so the same request can be answered twice
/// within the same second — one person on a desktop and a phone, or two people. The broadcast alone
/// cannot prevent the duplicate, because it arrives after the other client has already begun creating
/// a worktree and a child session. Claiming decides a single winner before any work starts.
///
/// Expired entries are pruned on each call, which is enough: claims are rare and few.
fn claim_spawn(key: String) -> bool {
    let mut claims = spawn_claims().lock().unwrap();
    let now = std::time::Instant::now();
    claims.retain(|_, at| now.duration_since(*at) < SPAWN_CLAIM_TTL);
    claims.insert(key, now).is_none()
}

/// Build the claim key. The unit separator cannot appear in a session ID, so no prompt can be crafted
/// to collide with a different request.
fn spawn_claim_key(parent_session_id: &str, prompt: &str) -> String {
    format!("{parent_session_id}\u{1f}{prompt}")
}

/// Answer a spawn request on behalf of the calling client, and report whether this caller won it.
///
/// Returns false when another client already confirmed or cancelled the same request; that caller must
/// then do nothing but drop its card, or the task runs twice. The winner's broadcast carries
/// `parentSessionId` and `prompt` for identification plus a `source` connection ID so the originator can
/// skip its own echo.
pub fn resolve_spawn(
    ctx: &AppCtx,
    source: &str,
    parent_session_id: &str,
    prompt: &str,
    confirmed: bool,
) -> bool {
    if !claim_spawn(spawn_claim_key(parent_session_id, prompt)) {
        return false;
    }
    let payload = serde_json::json!({
        "source": source,
        "parentSessionId": parent_session_id,
        "prompt": prompt,
        "confirmed": confirmed,
    });
    ctx.emit("spawn://resolved", payload);
    true
}

/// Turn mirror mode on or off for every client, persisting the choice for the next launch.
///
/// Switching off clears the published layout so a later switch-on starts from whoever publishes first,
/// instead of every client snapping back to an arrangement from hours ago.
pub fn mirror_set_enabled(ctx: &AppCtx, enabled: bool) -> Result<(), String> {
    set_app_settings(
        ctx,
        std::collections::HashMap::from([(
            MIRROR_ENABLED_KEY.to_string(),
            if enabled { "1" } else { "0" }.to_string(),
        )]),
    )?;
    if !enabled {
        crate::web::mirror::clear();
    }
    ctx.emit(
        crate::web::mirror::MODE_EVENT,
        serde_json::json!({ "enabled": enabled }),
    );
    Ok(())
}

/// Returns LAN remote status, port, access URL, and certificate fingerprint, merged with the persisted
/// saved port and auto-start flag so the panel can prefill and explain itself after a restart.
pub fn web_server_status(ctx: &AppCtx) -> crate::web::WebServerStatus {
    let mut status = ctx.remote_web().status();
    if let Ok(settings) = get_app_settings(ctx) {
        status.saved_port = settings.get(REMOTE_PORT_KEY).and_then(|p| p.parse().ok());
        status.auto_start = settings
            .get(REMOTE_ENABLED_KEY)
            .map(|v| v == "1")
            .unwrap_or(false);
    }
    status
}

/// Pure decision helper for auto-start: Some((port, lan_http, phc)) only when the persisted enabled flag
/// is set, a non-empty password hash exists, and the port parses. A corrupt port aborts auto-start rather
/// than binding an unintended port.
fn autostart_config(
    settings: &std::collections::HashMap<String, String>,
) -> Option<(u16, bool, String)> {
    if settings.get(REMOTE_ENABLED_KEY).map(String::as_str) != Some("1") {
        return None;
    }
    let phc = settings
        .get(REMOTE_PASSWORD_HASH_KEY)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let port: u16 = settings.get(REMOTE_PORT_KEY)?.trim().parse().ok()?;
    if port == 0 {
        return None;
    }
    let lan_http = settings.get(REMOTE_LAN_HTTP_KEY).map(String::as_str) == Some("1");
    Some((port, lan_http, phc))
}

/// Auto-starts LAN remote access on launch when the persisted enabled flag is set. Returns Ok(None) when
/// nothing is configured. Errors (port in use, production guard on a persisted dev mode) are recorded on
/// the WebServer so the panel can display them — callers only log; auto-start is never fatal.
pub fn web_server_autostart(
    ctx: &AppCtx,
) -> Result<Option<crate::web::WebServerStatus>, String> {
    // Never replace a running instance: a very early manual start would otherwise be stopped and
    // re-bound with the persisted (possibly older) configuration by the auto-start thread.
    if ctx.remote_web().status().running {
        return Ok(None);
    }
    // A settings-read failure is an auto-start failure like any other: record it so the panel can show
    // why nothing started, instead of logging it into the void.
    let settings = match get_app_settings(ctx) {
        Ok(s) => s,
        Err(e) => {
            ctx.remote_web().set_autostart_error(Some(e.clone()));
            return Err(e);
        }
    };
    let Some((port, lan_http, phc)) = autostart_config(&settings) else {
        return Ok(None);
    };
    // Re-apply the production guard: a persisted dev-only plaintext mode must never silently bind a
    // release build; it surfaces as a visible auto-start error instead.
    if let Err(e) = lan_http_guard(ctx, lan_http) {
        ctx.remote_web().set_autostart_error(Some(e.clone()));
        return Err(e);
    }
    let mode = if lan_http {
        crate::web::ServeMode::LanHttp
    } else {
        crate::web::ServeMode::LanTls
    };
    match ctx.remote_web().start(
        ctx.clone(),
        crate::web::StartAuth::PasswordHash(phc),
        Some(port),
        mode,
    ) {
        Ok(status) => Ok(Some(status)),
        Err(e) => {
            ctx.remote_web().set_autostart_error(Some(e.clone()));
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        autostart_config, claim_spawn, get_app_settings, install_id, release_spawn_claim,
        set_app_settings, spawn_claim_key, web_server_autostart, web_server_status, web_server_stop,
    };
    use std::collections::HashMap;

    /// Only the first answer to a spawn request wins; a second one — the same card confirmed on a phone
    /// a moment later — must be told it lost, so it cannot create a second worktree and child session.
    #[test]
    fn only_the_first_answer_claims_a_spawn_request() {
        let key = spawn_claim_key("ses-claim-test", "build the thing");
        assert!(claim_spawn(key.clone()), "the first answer wins");
        assert!(!claim_spawn(key.clone()), "a second answer must lose");
        // A different prompt, or the same prompt under a different parent, is a different request.
        assert!(claim_spawn(spawn_claim_key("ses-claim-test", "build something else")));
        assert!(claim_spawn(spawn_claim_key("ses-claim-test2", "build the thing")));
        // An agent retrying a task the user cancelled sends the identical parent and prompt. Issuing the
        // new card releases the old claim, so confirming it works instead of silently doing nothing.
        release_spawn_claim("ses-claim-test", "build the thing");
        assert!(claim_spawn(key), "a re-issued request can be answered again");
    }

    /// Builds a headless AppCtx over a fresh SQLite db inside `dir` (mirrors web::tests).
    fn headless_ctx(dir: &std::path::Path) -> crate::host::AppCtx {
        std::fs::create_dir_all(dir).unwrap();
        let db = crate::db::Db::open(&dir.join("t.db")).unwrap();
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(dir.to_path_buf(), db));
        crate::host::AppCtx::Headless(host)
    }

    fn seed_remote_settings(ctx: &crate::host::AppCtx, lan_http: &str) {
        set_app_settings(
            ctx,
            HashMap::from([
                ("remoteAccess.enabled".to_string(), "1".to_string()),
                ("remoteAccess.port".to_string(), "9123".to_string()),
                ("remoteAccess.lanHttp".to_string(), lan_http.to_string()),
                (
                    "remoteAccess.passwordHash".to_string(),
                    "$argon2id$fake".to_string(),
                ),
            ]),
        )
        .unwrap();
    }

    /// The installation identifier is issued once and then stays put: telemetry counts installations by
    /// it, so a value that changed between calls would inflate every usage figure it feeds.
    #[test]
    fn install_id_is_generated_once_and_reused() {
        let tmp = std::env::temp_dir().join(format!("vlx-cc-install-{}", std::process::id()));
        let ctx = headless_ctx(&tmp);

        let first = install_id(&ctx).unwrap();
        assert_eq!(first.len(), 36, "expected a canonical UUID, got {first}");
        assert_eq!(install_id(&ctx).unwrap(), first);
        // It is an ordinary preference, so a reopened database returns the same value.
        assert_eq!(
            get_app_settings(&ctx).unwrap().get("install_id").map(String::as_str),
            Some(first.as_str())
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Acceptance criterion 5: a manual stop persists enabled=0 while keeping port and password hash
    /// for prefill, and status() merges the persisted values for the panel. No server is ever bound.
    #[test]
    fn web_server_stop_disables_autostart_and_status_merges_settings() {
        let tmp = std::env::temp_dir().join(format!("vlx-cc-stop-{}", std::process::id()));
        let ctx = headless_ctx(&tmp);
        seed_remote_settings(&ctx, "0");

        // status() merges the persisted port and enabled flag even though nothing is running.
        let status = web_server_status(&ctx);
        assert!(!status.running);
        assert_eq!(status.saved_port, Some(9123));
        assert!(status.auto_start);

        // Stop on a non-running server is a no-op for the service but must persist enabled=0.
        web_server_stop(&ctx).unwrap();
        let settings = get_app_settings(&ctx).unwrap();
        assert_eq!(settings.get("remoteAccess.enabled").map(String::as_str), Some("0"));
        // Port and hash stay for prefill (doc contract on web_server_stop).
        assert_eq!(settings.get("remoteAccess.port").map(String::as_str), Some("9123"));
        assert_eq!(
            settings.get("remoteAccess.passwordHash").map(String::as_str),
            Some("$argon2id$fake")
        );
        assert!(!web_server_status(&ctx).auto_start);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// A persisted dev-only plaintext LAN mode must never silently bind in a release build: auto-start
    /// fails, no server runs, and the guard message surfaces via status().autostart_error (panel path).
    #[test]
    fn autostart_reapplies_production_guard_and_surfaces_error() {
        let tmp = std::env::temp_dir()
            .join(format!("vlx-cc-guard-{}", std::process::id()))
            .join("io.vlinx.vlxterm.release");
        let ctx = headless_ctx(&tmp);
        seed_remote_settings(&ctx, "1");

        let Err(err) = web_server_autostart(&ctx) else {
            panic!("production guard must reject lanHttp=1");
        };
        assert!(err.contains("LAN plaintext mode"), "unexpected error: {err}");
        assert!(!ctx.remote_web().status().running, "no server may bind");
        let status = web_server_status(&ctx);
        assert_eq!(status.autostart_error.as_deref(), Some(err.as_str()));
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
    }

    /// A running instance is never replaced by auto-start: a very early manual start (loopback here, so
    /// no 0.0.0.0 bind) survives, auto-start returns Ok(None), and the running config stays untouched.
    #[test]
    fn autostart_skips_when_an_instance_is_already_running() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-cc-skip-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let ctx = headless_ctx(&tmp);
        // Persisted settings that would auto-start on port 9123.
        seed_remote_settings(&ctx, "0");

        // Simulate the early manual start on a free loopback port (retry against port theft).
        let mut port = 0;
        let mut started = Err("never attempted".to_string());
        for _ in 0..5 {
            port = std::net::TcpListener::bind(("127.0.0.1", 0))
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            started = ctx.remote_web().start(
                ctx.clone(),
                crate::web::StartAuth::Password("manual-pw".into()),
                Some(port),
                crate::web::ServeMode::LoopbackHttp,
            );
            if started.is_ok() {
                break;
            }
        }
        started.expect("failed to start the loopback web server after retries");

        let result = web_server_autostart(&ctx).expect("skip must not be an error");
        assert!(result.is_none(), "auto-start must skip while an instance runs");
        let status = ctx.remote_web().status();
        assert!(status.running);
        assert_eq!(status.port, Some(port), "the manual instance must keep its port");

        ctx.remote_web().stop();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// A manual stop retires a stale auto-start error so the panel reflects the current state.
    #[test]
    fn manual_stop_clears_stale_autostart_error() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-cc-clear-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let ctx = headless_ctx(&tmp);
        ctx.remote_web()
            .set_autostart_error(Some("port in use".into()));
        assert_eq!(
            web_server_status(&ctx).autostart_error.as_deref(),
            Some("port in use")
        );
        web_server_stop(&ctx).unwrap();
        assert!(
            web_server_status(&ctx).autostart_error.is_none(),
            "a manual stop must clear the stale autostart error"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    fn settings(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn autostart_config_requires_enabled_hash_and_valid_port() {
        // Complete configuration yields the start parameters.
        let full = settings(&[
            ("remoteAccess.enabled", "1"),
            ("remoteAccess.port", "9123"),
            ("remoteAccess.lanHttp", "0"),
            ("remoteAccess.passwordHash", "$argon2id$fake"),
        ]);
        assert_eq!(
            autostart_config(&full),
            Some((9123, false, "$argon2id$fake".to_string()))
        );

        // Missing keys entirely.
        assert_eq!(autostart_config(&HashMap::new()), None);

        // Disabled flag wins over everything else.
        let mut disabled = full.clone();
        disabled.insert("remoteAccess.enabled".into(), "0".into());
        assert_eq!(autostart_config(&disabled), None);

        // Empty hash means no credential to start with.
        let mut no_hash = full.clone();
        no_hash.insert("remoteAccess.passwordHash".into(), "  ".into());
        assert_eq!(autostart_config(&no_hash), None);

        // Invalid or zero port aborts auto-start instead of binding an unintended port.
        for bad in ["", "abc", "0", "70000"] {
            let mut bad_port = full.clone();
            bad_port.insert("remoteAccess.port".into(), bad.into());
            assert_eq!(autostart_config(&bad_port), None, "port {bad:?}");
        }

        // lanHttp missing defaults to TLS mode; "1" selects plaintext LAN.
        let mut no_mode = full.clone();
        no_mode.remove("remoteAccess.lanHttp");
        assert!(matches!(autostart_config(&no_mode), Some((_, false, _))));
        let mut lan = full;
        lan.insert("remoteAccess.lanHttp".into(), "1".into());
        assert!(matches!(autostart_config(&lan), Some((_, true, _))));
    }
}

/// Generates a browser pairing link with the shared token and server public key in the URL fragment.
/// `address` chooses the interface IP; `rotate` replaces the token, invalidates old links, and clears devices.
pub fn web_pairing_create(
    ctx: &AppCtx,
    address: Option<String>,
    rotate: bool,
) -> Result<crate::web::PairingInfo, String> {
    ctx.remote_web().create_pairing(address, rotate)
}

/// Lists paired devices that have actually connected for the management panel.
pub fn web_devices_list(ctx: &AppCtx) -> Vec<crate::web::DeviceEntry> {
    ctx.remote_web().list_devices()
}

/// Removes a device registration display entry. Shared links can still reconnect; rotate to revoke all.
/// Errors when the revocation cannot be persisted, because it would be undone by the next restart.
pub fn web_device_revoke(ctx: &AppCtx, device_id: &str) -> Result<bool, String> {
    ctx.remote_web().revoke_device(device_id)
}
