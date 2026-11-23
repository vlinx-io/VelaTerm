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

use crate::agent::chat::protocol::ChatImage;
use crate::agent::session_settings::{self, Selection};
use crate::db::repo;
use crate::host::{AppCtx, PRESETS_CHANGED, SETTINGS_CHANGED, TREE_CHANGED};
use crate::models::{AgentPreset, Group, NodeKind, Project, Session, SessionKind, Tree};

// Private helpers.

/// Reads a session's `(kind, captured agent-native session ID)`, returning a specific error if either is absent.
/// Shared by the four transcript/usage commands for consistent lookup and error messages.
fn session_kind_and_agent(ctx: &AppCtx, session_id: &str) -> Result<(SessionKind, String), String> {
    let session = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::get_session(&conn, session_id)?
    }
    .ok_or("Session not found")?;
    // A parked DeepSeek conversation still has its retired harness recording while the new OpenCode
    // conversation has not reported an id yet.
    let agent_id = session.agent_session_id.or(session.legacy_agent_session_id)
        .ok_or("No agent session id captured for this session yet")?;
    Ok((session.kind, agent_id))
}

/// Normalizes empty strings to None so blank values are not persisted.
fn empty_to_none(v: Option<&str>) -> Option<&str> {
    v.filter(|s| !s.trim().is_empty())
}

/// Allocate the same per-project `Claude N` / `Codex N` names as ordinary session creation.
/// Archived sessions count too, so clearing repeatedly never reuses a label that remains in history.
fn next_chat_session_name(
    conn: &rusqlite::Connection,
    project_id: &str,
    kind: SessionKind,
) -> Result<String, String> {
    let label = match kind {
        SessionKind::Claude => "Claude",
        SessionKind::Codex => "Codex",
        SessionKind::Opencode => "OpenCode",
        SessionKind::Antigravity => "Antigravity",
        SessionKind::Pi => "Pi",
        SessionKind::Omp => "OMP",
        SessionKind::Zcode => "ZCode",
        SessionKind::Deepseek => "DeepSeek",
        SessionKind::Copilot => "Copilot",
        SessionKind::Cursor => "Cursor",
        SessionKind::Kimi => "Kimi Code",
        SessionKind::Grok => "Grok Build",
        _ => return Err("Only chat sessions can be cleared".to_string()),
    };
    let prefix = format!("{label} ");
    let max_suffix = repo::list_all_sessions(conn)?
        .into_iter()
        .filter(|session| session.project_id == project_id && session.kind == kind)
        .filter_map(|session| {
            session
                .name
                .strip_prefix(&prefix)
                .and_then(|suffix| suffix.parse::<u64>().ok())
        })
        .max()
        .unwrap_or(0);
    let suffix = max_suffix
        .checked_add(1)
        .ok_or_else(|| format!("No available {label} session number"))?;
    Ok(format!("{label} {suffix}"))
}

// Tree management. Successful writes emit `TREE_CHANGED`; clients debounce tree reloads for cross-client sync.

/// Imports a directory as a project.
pub fn import_project(ctx: &AppCtx, root_path: &str) -> Result<Project, String> {
    import_project_into_collection(ctx, root_path, None)
}

/// Imports a directory and optionally assigns it to a collection in the same database transaction.
pub fn import_project_into_collection(ctx: &AppCtx, root_path: &str, collection_id: Option<&str>) -> Result<Project, String> {
    let project = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::import_project_into_collection(&conn, root_path, collection_id)?
    };
    ctx.emit(TREE_CHANGED, ());
    Ok(project)
}

/// Creates a directory-free sidebar collection, optionally nested under another collection.
pub fn create_virtual_project(ctx: &AppCtx, name: &str, collection_id: Option<&str>) -> Result<Project, String> {
    let project = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::create_virtual_project_in_collection(&conn, name, collection_id)?
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
    // Which engine drives the session from the start. A session created as `chat` opens straight into
    // the conversation view, rather than being switched over from a terminal the user never wanted.
    engine: Option<&str>,
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
            empty_to_none(engine),
        )?;
        let seeded_agent_id = agent_session_id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            // Only the terminal engine can seed a caller-provided id; the chat engine lets the agent assign
            // it (see `SessionKind::accepts_caller_session_id`).
            .or_else(|| kind.accepts_caller_session_id(&session.engine).then(|| session.id.clone()));
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
        let kind = repo::get_session_kind(&conn, id)?;
        let normalized_perm = empty_to_none(permission_mode);
        repo::update_session(
            &conn,
            id,
            name,
            empty_to_none(shell),
            empty_to_none(cwd),
            empty_to_none(init_cmd),
            empty_to_none(agent_args),
            normalized_perm,
        )?;
        if kind == Some(SessionKind::Antigravity) {
            let effective = crate::agent::permission_catalog::effective(
                &conn,
                SessionKind::Antigravity,
                normalized_perm,
            )?
            .unwrap_or_else(|| "default".to_string());
            let acp_mode =
                crate::agent::chat::antigravity_protocol::acp_mode_for_permission(&effective);
            repo::set_collaboration_mode(&conn, id, acp_mode)?;
        }
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
    // The person chose this name; stop an agent-written title from replacing it later.
    if matches!(kind, NodeKind::Session) {
        crate::agent::server::forget_adopted_title(ctx, id);
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

/// Binds an existing group to a worktree so sessions created in it afterwards start there and it shows the
/// sidebar tag. Sessions already in the group are left alone: their working directory was copied in at creation
/// time and changing it under a running PTY is not possible. See `repo::set_group_worktree`.
pub fn set_group_worktree(
    ctx: &AppCtx,
    id: &str,
    worktree_path: Option<&str>,
    worktree_base_ref: Option<&str>,
) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_group_worktree(
            &conn,
            id,
            empty_to_none(worktree_path),
            empty_to_none(worktree_base_ref),
        )?;
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
        let _ = crate::search::corpus::drop_sessions(ctx, &hard_deleted);
        // Drop the authoritative session records too. They deliberately outlive a process exit — an
        // agent that finished and left an unread result behind must stay marked — so removal from the
        // tree is the only thing that clears one. The in-memory status view goes with them, so `vstat`
        // stops listing sessions nobody can open and a coordinator waiting on one is released rather
        // than waiting out its timeout.
        for sid in &hard_deleted {
            crate::session_state::forget(sid);
            crate::agent::status_watch::forget(sid);
        }
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

/// Archives a user collection: archive its sessions, release member projects, and retain the collection as a
/// hidden tombstone that any restored session revives. See `repo::archive_collection`.
pub fn archive_collection(ctx: &AppCtx, id: &str) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::archive_collection(&conn, id)?;
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

/// Persist collection membership through the shared asynchronous data-command dispatch.
pub fn set_project_collection(ctx: &AppCtx, project_id: &str, collection_id: Option<&str>) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_project_collection(&conn, project_id, collection_id)?;
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
    let mut settings = repo::get_app_settings(&conn)?;
    settings
        .entry(crate::pty::completion::MODE_KEY.into())
        .or_insert_with(|| crate::pty::completion::DEFAULT_MODE.into());
    settings
        .entry(crate::pty::completion::TAB_BEHAVIOR_KEY.into())
        .or_insert_with(|| crate::pty::completion::DEFAULT_TAB_BEHAVIOR.into());
    Ok(settings)
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
    repo::get_or_create_app_setting(&conn, INSTALL_ID_KEY, &uuid::Uuid::new_v4().to_string())
}

/// Batch-upserts application preferences by key with last-write-wins semantics. This is the authoritative backend
/// side of frontend local-cache dual writes, used for startup seeding and debounced setting changes.
///
/// Broadcasts `SETTINGS_CHANGED` with the written key names on success, so a preference changed in one
/// shell reaches the others while they run instead of waiting for their next launch. Only names travel,
/// never values; see the constant's documentation for why. The writer receives its own broadcast as well,
/// exactly like `TREE_CHANGED`: client-side reconciliation is idempotent, so the extra pass is harmless.
pub fn set_app_settings(
    ctx: &AppCtx,
    mut entries: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    if let Some(mode) = entries.get(crate::pty::completion::MODE_KEY) {
        if !matches!(mode.as_str(), "auto" | "tab" | "off") {
            return Err("Invalid terminal completion mode".into());
        }
    }
    if let Some(behavior) = entries.get(crate::pty::completion::TAB_BEHAVIOR_KEY) {
        if !matches!(behavior.as_str(), "accept" | "complete") {
            return Err("Invalid terminal Tab behavior".into());
        }
    }
    let region = match entries.get(crate::server_region::SETTING_KEY) {
        Some(value) => Some(crate::server_region::Region::parse(value).ok_or("Invalid server region")?),
        None => None,
    };
    // The remote-access listen settings are validated and stored canonically, whoever writes them; an
    // invalid value rejects the whole batch (error codes are mapped to localized text by the frontend).
    if let Some(bind) = entries.get_mut(REMOTE_BIND_KEY) {
        *bind = crate::web::BindChoice::parse(bind)?.as_setting();
    }
    if let Some(host) = entries.get_mut(REMOTE_PAIRING_HOST_KEY) {
        *host = crate::web::validate_pairing_host(host)?.unwrap_or_default();
    }
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_app_settings(&conn, &entries)?;
    }
    if entries.contains_key("vlx-settings") {
        crate::agent::install::forget_shell_discovery();
    }
    if let Some(region) = region {
        crate::server_region::set(region);
    }
    if let Some(mode) = entries.get(crate::pty::completion::MODE_KEY) {
        // Running shells pick up the new mode at their next prompt.
        if let Err(e) = ctx.data_dir().and_then(|dir| crate::pty::completion::write_mode(&dir, mode)) {
            crate::diagnostic_warn!("failed to share the terminal completion mode: {e}");
        }
    }
    // An empty batch changes nothing; staying quiet avoids waking every client for no reason.
    if !entries.is_empty() {
        let keys: Vec<String> = entries.keys().cloned().collect();
        ctx.emit(SETTINGS_CHANGED, keys);
    }
    Ok(())
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
pub(crate) fn app_identifier(ctx: &AppCtx) -> String {
    ctx.data_dir()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "io.vlinx.vlxterm".to_string())
}

/// Returns the DeepSeek endpoint and whether a key is stored, never the key itself.
pub fn deepseek_get_status(ctx: &AppCtx) -> crate::agent::deepseek_config::Status {
    crate::agent::deepseek_config::status(ctx.db(), &app_identifier(ctx))
}

/// Saves the DeepSeek endpoint and key. An empty key preserves the stored one unless `clear_key` is set.
pub fn deepseek_set_config(
    ctx: &AppCtx,
    base_url: &str,
    api_key: &str,
    clear_key: bool,
) -> Result<crate::agent::deepseek_config::Status, String> {
    let status = crate::agent::deepseek_config::set_config(
        ctx.db(),
        &app_identifier(ctx),
        base_url,
        api_key,
        clear_key,
    )?;
    // Running DeepSeek servers hold the configuration they started with, so idle ones are ended here and
    // restart with the new credentials on the next message.
    ctx.chat().refresh_deepseek(ctx);
    Ok(status)
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

/// Whether the Git panel can open a pull request for the repository at `cwd`: Gitea must be
/// configured on the same host as `origin`, hold a token, and answer. `reason` is `ok`,
/// `not_gitea`, `no_token`, or `unreachable` for the panel to explain.
pub fn git_pr_provider(ctx: &AppCtx, cwd: &str) -> serde_json::Value {
    if !crate::gitea::relevant(ctx.db(), cwd) {
        return serde_json::json!({ "provider": null, "available": false, "reason": "not_gitea" });
    }
    let (available, reason) = crate::gitea::land_provider_check(ctx.db(), &app_identifier(ctx));
    serde_json::json!({ "provider": "gitea", "available": available, "reason": reason })
}

/// Push the current branch and open a Gitea pull request into `base`, the branch the Git panel
/// compares against.
pub fn git_open_pull_request(
    ctx: &AppCtx,
    cwd: &str,
    base: &str,
    title: &str,
    body: &str,
) -> Result<crate::gitea::PrOutcome, String> {
    // `status().branch` is None on a detached HEAD, where there is no branch to push.
    let head = crate::git::status(cwd).branch.ok_or("Check out a branch before opening a pull request")?;
    let base_ref = crate::git::compare::full_ref(cwd, base)?;
    crate::gitea::open_pr(ctx.db(), &app_identifier(ctx), cwd, &head, &base_ref, title, body)
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
        .map(|(target, label, last_connected_at, shared_db, mirror)| {
            let has_password = crate::ssh_remote::has_password(&id, "ssh", &target);
            crate::ssh_remote::SshHostInfo {
                target,
                label,
                last_connected_at,
                has_password,
                shared_db,
                mirror,
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
    let scope = crate::search::SearchScope::from_arg(scope.unwrap_or("live"));
    crate::search::jobs::legacy(ctx, query, scope, None)
}

/// Session-content search optionally restricted to one already resolved session ID.
pub fn search_session_content_in(
    ctx: &AppCtx,
    query: &str,
    scope: Option<&str>,
    session_id: Option<&str>,
) -> Result<Vec<crate::search::SessionSearchHit>, String> {
    let scope = crate::search::SearchScope::from_arg(scope.unwrap_or("live"));
    crate::search::jobs::legacy(ctx, query, scope, session_id)
}

/// Reads an agent transcript for the archive-panel viewer.
pub fn read_agent_transcript(
    ctx: &AppCtx,
    session_id: &str,
) -> Result<Vec<crate::agent::transcript::TranscriptMessage>, String> {
    let (kind, agent_id) = session_kind_and_agent(ctx, session_id)?;
    if crate::agent::history_store::prose::supported(kind) {
        let branch=crate::agent::history_store::api::prepare(ctx,session_id)?;
        let mut ready=branch.error.is_none();let mut total=0u64;
        for source in &branch.sources {
            let (water,status)=source.progress()?;
            ready&=status=="complete" || source.segment.end_byte.is_some_and(|end|water>=end);
            total=total.saturating_add(source.segment.end_byte.unwrap_or(source.size));
        }
        if ready {
            let cancel=std::sync::atomic::AtomicBool::new(false);
            let prose=crate::agent::history_store::prose::Prose::new(branch,&cancel)?;
            return prose.collect_bounded(0,prose.total,crate::agent::history_store::limits::PAGE_BYTES,&cancel);
        }
        if total>crate::agent::history_store::limits::PAGE_BYTES as u64 {
            return Err(crate::agent::history_store::compat::UPGRADE.into());
        }
    }
    crate::agent::transcript::read(kind, &agent_id)
}

// ─────────────────────────── Chat engine ───────────────────────────

/// Choose how an agent session is driven, and stop whatever the previous engine had running.
///
/// The conversation itself is untouched — both engines read and write the same recording — so the next
/// launch picks up where the last one stopped.
pub fn set_session_engine(ctx: &AppCtx, session_id: &str, engine: &str) -> Result<(), String> {
    if !matches!(engine, "tui" | "chat") {
        return Err(format!("Unknown session engine: {engine}"));
    }
    let session = session_settings::session(ctx, session_id)?;
    if session.engine == engine {
        return Ok(());
    }
    if !session.kind.supports_chat() {
        return Err("This session does not support switching conversation engines".into());
    }
    if engine == "tui" && !session.kind.has_terminal_interface() {
        return Err(format!("{} has no terminal interface; this conversation stays in the conversation view.",
            crate::agent::launch_options::label(session.kind)));
    }
    // Leaving an engine means leaving its process behind; two agents must never share one conversation.
    if engine == "tui" {
        let snapshot = ctx.chat().snapshot(session_id);
        let selection = if snapshot.running {
            Selection {
                model: snapshot.model,
                effort: snapshot.effort,
            }
        } else {
            session_settings::resolve(ctx, &session)?
        };
        session_settings::persist(ctx, &session, &selection)?;
        if session.kind == SessionKind::Opencode {
            // This opens only the local protocol peer; no prompt or model request is sent.
            chat_start(
                ctx,
                session_id,
                selection.model.as_deref(),
                selection.effort.as_deref(),
                false,
            )?;
            ctx.chat().prepare_terminal(session_id, &selection)?;
        }
    }
    let _transition=ctx.chat().engine_guard();
    let current=session_settings::session(ctx,session_id)?;
    if current.engine==engine { return Ok(()); }
    let _owner=if engine=="tui" {
        ctx.chat().stop_for_handoff(ctx,session_id)?
    } else {
        let owner=crate::agent::chat::ownership::idle_guard(ctx,session_id)?;
        // Restart semantics, not close: the pane stays and the chat engine takes over the same session.
        ctx.pty().kill_for_handoff(session_id)?;
        // Read after the terminal exits so its final settings writes are visible.
        let session = session_settings::session(ctx, session_id)?;
        let selection = session_settings::resolve(ctx, &session)?;
        session_settings::persist(ctx, &session, &selection)?;
        owner
    };
    repo::set_session_engine(&ctx.db().conn.lock().unwrap(), session_id, engine)?;
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Archive one chat conversation and atomically put a fresh, identically configured sibling in its place.
///
/// The process is stopped only after the transaction commits. The returned session lets the initiating
/// client retarget its pane synchronously; `TREE_CHANGED` then reconciles every other connected client.
pub fn chat_clear(ctx: &AppCtx, session_id: &str) -> Result<Session, String> {
    let fresh = {
        let mut conn = ctx.db().conn.lock().unwrap();
        let transaction = conn
            .transaction()
            .map_err(|e| format!("Failed to start chat clear transaction: {e}"))?;
        let source = repo::get_session(&transaction, session_id)?.ok_or("Session not found")?;
        if source.archived_at.is_some() {
            return Err("This chat session is already archived".to_string());
        }
        if !source.kind.supports_chat() || source.engine != "chat"
        {
            return Err("Only chat sessions can be cleared".to_string());
        }
        let name = next_chat_session_name(&transaction, &source.project_id, source.kind)?;
        let fresh = repo::create_fresh_chat_session(&transaction, &source, &name)?;
        repo::set_archived(&transaction, session_id, true)?;
        transaction
            .commit()
            .map_err(|e| format!("Failed to commit chat clear transaction: {e}"))?;
        fresh
    };

    ctx.chat().stop(ctx, session_id)?;
    crate::agent::chat::auto_continue::cancel(ctx, session_id);
    ctx.emit(TREE_CHANGED, ());
    Ok(fresh)
}

/// Start the chat engine for a session, or do nothing if it is already running.
///
/// The conversation continues where it left off: the session's captured agent id becomes `--resume`, and
/// because both engines write the same recording, a conversation started in the terminal picks up here.
pub fn chat_start(
    ctx: &AppCtx,
    session_id: &str,
    model: Option<&str>,
    effort: Option<&str>,
    fast_mode: bool,
) -> Result<(), String> {
    let (mut session, project_root) = {
        let conn = ctx.db().conn.lock().unwrap();
        let session = repo::get_session(&conn, session_id)?.ok_or("Session not found")?;
        let root = repo::get_project_root(&conn, &session.project_id)?;
        (session, root)
    };
    // A DeepSeek conversation recorded by the retired harness cannot be resumed in OpenCode. Park its id
    // in `legacy_agent_session_id` first: that both lets the new conversation claim `agent_session_id` and
    // keeps the old recording visible above it in the history view.
    if session.kind == SessionKind::Deepseek {
        if let Some(legacy) = session
            .agent_session_id
            .clone()
            .filter(|id| {
                crate::agent::history_store::deepseek_legacy::is_legacy(id)
                    // A saved id neither store knows can still block startup through the identity
                    // check; park it the same way so a fresh conversation may begin.
                    || crate::agent::resume::confirmed_missing(SessionKind::Deepseek, id)
            })
        {
            let conn = ctx.db().conn.lock().unwrap();
            repo::park_legacy_agent_session_id(&conn, session_id, &legacy, SessionKind::Deepseek)?;
            session.agent_session_id = None;
            session.legacy_agent_session_id = Some(legacy);
        }
    }
    if session.archived_at.is_some() { return Err("Restore this conversation before restarting its agent.".into()); }
    if session.engine!="chat" { return Err("Open this conversation in the chat view before starting its agent.".into()); }
    if ctx.chat().is_alive(session_id) { return Ok(()); }
    if !session.kind.supports_chat() {
        return Err(format!(
            "The chat engine does not support {} sessions yet",
            session.kind.as_str()
        ));
    }
    // A session created under a project usually has no directory of its own; it runs in the project's.
    // The terminal path applies this fallback in the frontend, so the engine must apply it here or the
    // agent inherits the backend's own working directory and reads the wrong files. With neither, or with a
    // directory that no longer exists, it runs in home exactly as a terminal does; OpenCode v2 cannot open a
    // session without a directory at all.
    let cwd = crate::host::spawn_dir(
        session.cwd.as_deref().filter(|c| !c.trim().is_empty()).or(project_root.as_deref()),
    );
    if crate::agent::executable::resolve_launch_binary(ctx, session_id, session.kind)?
        == crate::agent::executable::LaunchBinary::Broken
    {
        return Err(format!("agent_not_installed:{}", session.kind.as_str()));
    }
    let bin = crate::agent::executable::for_session(ctx, &session);
    let mut selection = session_settings::resolve(ctx, &session)?;
    if model.is_some() {
        selection.model = session_settings::clean(model);
    }
    if effort.is_some() {
        selection.effort = session_settings::clean(effort);
    }
    // DeepSeek is driven by OpenCode but only offers the official DeepSeek provider. Older conversations
    // saved DeepSeek Harness's opaque route pair, so normalize it to OpenCode's `deepseek/<model>` spelling.
    if session.kind == SessionKind::Deepseek {
        selection.model = Some(crate::agent::chat::deepseek_models::normalize_model(selection.model.as_deref()));
        selection.effort = crate::agent::chat::deepseek_models::normalize_effort(selection.effort.as_deref());
    } else if session.kind == SessionKind::Antigravity {
        crate::agent::antigravity_models::normalize_selection(ctx, &mut selection);
    }
    session_settings::persist(ctx, &session, &selection)?;
    let extra_args =
        session_settings::without_selection_args(session.kind, session.agent_args.as_deref());
    let permission_mode = {
        let conn = ctx.db().conn.lock().unwrap();
        crate::agent::permission_catalog::effective(&conn, session.kind, session.permission_mode.as_deref())?
    };
    // A DeepSeek conversation used to be recorded by DeepSeek Harness, so its saved id is not an
    // OpenCode session. Start a new OpenCode conversation instead of failing to resume a foreign id.
    let resume = session.agent_session_id.as_deref().filter(|id| {
        session.kind != SessionKind::Deepseek
            || !crate::agent::resume::confirmed_missing(session.kind, id)
    });
    // A Grok session seeded with its own VelaTerm session id (see `create_session`/`insert_session`) has no
    // agent conversation until first launch. If that id was never created — a row written before chat
    // sessions stopped pre-seeding, or a tui session switched to chat before it ran — the chat engine must
    // not resume it: ACP assigns the id via `session/new`, so `session/load` would only fail and report a
    // missing conversation. Drop it so the first launch starts fresh. The id is stored once the agent opens.
    let resume = resume.filter(|id| {
        !(session.kind == SessionKind::Grok
            && *id == session.id.as_str()
            && crate::agent::resume::confirmed_missing(session.kind, id))
    });
    ctx.chat().start(
        ctx,
        session_id,
        session.kind,
        cwd.as_deref(),
        &bin,
        resume,
        selection.model.as_deref(),
        selection.effort.as_deref(),
        permission_mode.as_deref(),
        session.collaboration_mode.as_deref(),
        &crate::agent::inject::split_extra_args(Some(&extra_args)),
        fast_mode,
    )
}

/// Switch Claude's fast mode on or off for the running conversation.
pub fn chat_set_fast_mode(ctx: &AppCtx, session_id: &str, enabled: bool) -> Result<(), String> {
    ctx.chat().set_fast_mode(ctx, session_id, enabled)
}

/// Attach or detach Claude in Chrome for a Claude conversation: saved with the conversation so every later
/// process gets it too, and applied to the running process at once.
pub fn chat_set_chrome(ctx: &AppCtx, session_id: &str, enabled: bool) -> Result<(), String> {
    if ctx.chat().snapshot(session_id).running {
        ctx.chat().set_chrome(session_id, enabled)?;
    }
    {
        let conn = ctx.db().conn.lock().unwrap();
        session_settings::set_chrome(&conn, session_id, enabled)?;
    }
    ctx.emit(
        &format!("chat://event/{session_id}"),
        serde_json::json!({"type":"chromeChanged","enabled":enabled}),
    );
    Ok(())
}

/// Choose the Codex service tier for subsequent turns; None returns to the thread's own.
pub fn chat_set_service_tier(
    ctx: &AppCtx,
    session_id: &str,
    tier: Option<&str>,
) -> Result<(), String> {
    let tier = tier.map(str::trim).filter(|value| !value.is_empty());
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_codex_chat_setting(&conn, session_id, "service_tier", tier)?;
    }
    if ctx.chat().snapshot(session_id).running {
        ctx.chat().set_service_tier(session_id, tier)?;
    }
    ctx.emit(
        &format!("chat://event/{session_id}"),
        serde_json::json!({"type":"codexSettingsChanged","serviceTier":tier}),
    );
    Ok(())
}

/// Choose the Codex personality for subsequent turns; None returns to the configured one.
pub fn chat_set_personality(
    ctx: &AppCtx,
    session_id: &str,
    personality: Option<&str>,
) -> Result<(), String> {
    let personality = personality.map(str::trim).filter(|value| !value.is_empty());
    if personality.is_some_and(|value| !matches!(value, "none" | "friendly" | "pragmatic")) {
        return Err("Unknown Codex personality".into());
    }
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_codex_chat_setting(&conn, session_id, "personality", personality)?;
    }
    if ctx.chat().snapshot(session_id).running {
        ctx.chat().set_personality(session_id, personality)?;
    }
    ctx.emit(
        &format!("chat://event/{session_id}"),
        serde_json::json!({"type":"codexSettingsChanged","personality":personality}),
    );
    Ok(())
}

/// Whether the conversation's agent is installed and signed in, checked before anything is sent.
pub fn chat_agent_readiness(ctx: &AppCtx, session_id: &str) -> Result<impl serde::Serialize, String> {
    ctx.chat().agent_readiness(ctx, session_id)
}

/// Begin provider-managed authorization in the existing conversation.
pub fn chat_auth_start(ctx: &AppCtx, session_id: &str, provider: Option<&str>) -> Result<(), String> {
    prepare_chat_account(ctx, session_id)?;
    ctx.chat().auth_start(ctx, session_id, provider)
}

pub fn chat_auth_logout(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    prepare_chat_account(ctx, session_id)?;
    ctx.chat().auth_logout(ctx, session_id)
}

fn prepare_chat_account(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    {
        let conn = ctx.db().conn.lock().unwrap();
        let session = repo::get_session(&conn, session_id)?.ok_or("Session not found")?;
        if !matches!(session.kind, SessionKind::Codex | SessionKind::Claude | SessionKind::Zcode | SessionKind::Antigravity)
            || session.engine != "chat"
        {
            return Err(
                "Account operations are available only in Claude, Codex, ZCode or Antigravity chat sessions.".into(),
            );
        }
    }
    chat_start(ctx, session_id, None, None, false)
}

pub fn chat_auth_submit(ctx: &AppCtx, session_id: &str, code: &str) -> Result<(), String> {
    ctx.chat().auth_submit(ctx, session_id, code)
}

pub fn chat_auth_cancel(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    ctx.chat().auth_cancel(ctx, session_id)
}

/// Ask Codex to summarize the conversation now (`/compact`).
pub fn chat_compact(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    ctx.chat().compact(session_id)
}

/// Run Codex's code review as a turn of the conversation (`/review [branch <name> | commit <sha> | <instructions>]`).
pub fn chat_review(ctx: &AppCtx, session_id: &str, args: &str) -> Result<(), String> {
    ctx.chat().review(ctx, session_id, args)
}

/// The MCP servers the running agent knows, with their state and tools.
pub fn chat_mcp_status(ctx: &AppCtx, session_id: &str) -> Result<serde_json::Value, String> {
    ctx.chat().mcp_status(session_id)
}

/// Enable or disable one MCP server, answering with the refreshed server list.
pub fn chat_mcp_toggle(
    ctx: &AppCtx,
    session_id: &str,
    server: &str,
    enabled: bool,
) -> Result<serde_json::Value, String> {
    ctx.chat().mcp_toggle(session_id, server, enabled)
}

/// Reconnect one MCP server, answering with the refreshed server list.
pub fn chat_mcp_reconnect(
    ctx: &AppCtx,
    session_id: &str,
    server: &str,
) -> Result<serde_json::Value, String> {
    ctx.chat().mcp_reconnect(session_id, server)
}

/// Stop one of a conversation's background tasks.
pub fn chat_stop_task(ctx: &AppCtx, session_id: &str, task_id: &str) -> Result<(), String> {
    ctx.chat().stop_task(ctx, session_id, task_id)
}

/// The command and recent output of one background shell task, for its task tab.
pub fn chat_task_output(
    ctx: &AppCtx,
    session_id: &str,
    task_id: &str,
) -> Result<crate::agent::chat::engine::TaskOutput, String> {
    ctx.chat().task_output(session_id, task_id)
}

/// What one of a session's subagents has done so far, read from its own recording.
pub fn chat_subagent_rows(
    ctx: &AppCtx,
    session_id: &str,
    task_id: &str,
) -> Result<Vec<crate::agent::chat::engine::ChatRow>, String> {
    let (kind, agent_id) = session_kind_and_agent(ctx, session_id)?;
    match kind {
        SessionKind::Claude => crate::agent::chat::history::replay_claude_subagent(&agent_id, task_id),
        SessionKind::Antigravity => {
            let content = crate::agent::antigravity::read_transcript(task_id)?;
            Ok(crate::agent::chat::antigravity_protocol::history_rows(&content))
        }
        _ => Err("Subagent conversations are available only for Claude and Antigravity".into()),
    }
}

/// Move every foreground task of the running turn to the background.
pub fn chat_background_tasks(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    ctx.chat().background_tasks(ctx, session_id)
}

/// How many images one message may carry, and how large each may be once decoded.
///
/// Attached images are held in memory for as long as the row is on the timeline and are sent again to
/// every client that opens the conversation, so an unbounded paste would be paid for repeatedly. The
/// limits are generous for what people actually paste — a screenshot is well under a megabyte — and the
/// composer stops at the same numbers, with a message, before anything is uploaded.
pub const MAX_IMAGES_PER_MESSAGE: usize = 20;
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// Send a user turn to a running chat session, with any images attached to it.
///
/// `behavior` says what to do when a turn is already running: `queue` (the default) waits for it to end,
/// `interrupt` stops it and goes first, `steer` writes into it. Answers `"sent"` or `"queued"`.
pub fn chat_send(
    ctx: &AppCtx,
    session_id: &str,
    text: &str,
    images: Vec<ChatImage>,
    behavior: Option<&str>,
    message_id: Option<&str>,
) -> Result<&'static str, String> {
    if crate::security::session_active(session_id) {
        return Err("security_audit_owns_conversation".into());
    }
    check_images(&images)?;
    // A message sent while waiting for a usage limit to reset takes over from the automatic continuation.
    crate::agent::chat::auto_continue::cancel(ctx, session_id);
    // Pasted calls can bypass completion and arrive before the startup catalogue. Resolve aliases on
    // the backend as well, so a first-message `$skill` in Claude is never sent as ordinary prose.
    let session = session_settings::session(ctx, session_id)?;
    if session.archived_at.is_some() { return Err("Restore this conversation before sending another message.".into()); }
    if session.engine!="chat" { return Err("Open this conversation in the chat view before sending another message.".into()); }
    let commands = if crate::agent::chat::skills::needs_alias_lookup(session.kind, text) {
        chat_commands(ctx, session_id)?
    } else {
        Vec::new()
    };
    let original_text = text;
    let text = crate::agent::chat::skills::native_text(text, &commands);
    let Some(message_id) = message_id else {
        return ctx
            .chat()
            .send(ctx, session_id, &text, images, behavior.unwrap_or("queue"));
    };
    use crate::agent::chat::submissions::{self, Claim};
    let payload = serde_json::to_vec(&serde_json::json!([
        original_text,
        images,
        behavior.unwrap_or("queue")
    ]))
    .map_err(|e| e.to_string())?;
    let claim = submissions::claim_retry(
        &ctx.db().conn.lock().unwrap(),
        session_id,
        message_id,
        &payload,
    )?;
    if let Claim::Complete(outcome) = claim {
        return outcome.and_then(|value| match value.as_str() {
            "sent" => Ok("sent"),
            "steered" => Ok("steered"),
            "blocked" => Ok("blocked"),
            "queued" => Ok("queued"),
            "command" => Ok("command"),
            _ => Err("Invalid submission receipt".into()),
        });
    }
    let outcome = ctx.chat().send_identified_checked(
        ctx,
        session_id,
        &text,
        images,
        behavior.unwrap_or("queue"),
        Some(message_id),
    );
    use crate::agent::chat::engine::SubmissionFailure;
    if let Err(SubmissionFailure::Rejected(error)) = &outcome {
        submissions::finish_rejected(&ctx.db().conn.lock().unwrap(), session_id, message_id, error)?;
        return Err(error.clone());
    }
    let outcome = outcome.map_err(|_| "chat_submission_pending".to_string());
    submissions::finish(
        &ctx.db().conn.lock().unwrap(),
        session_id,
        message_id,
        &outcome.clone().map(str::to_owned),
    )
    .map_err(|_| "chat_submission_pending".to_string())?;
    outcome
}

/// Run a shell command typed with `!` in the conversation view.
///
/// The whole text after `!` is the command, handed to the session's shell as one string. It runs in the
/// agent's directory with the agent's environment; the agent is started first when none is alive, the way
/// a normal send does, because the result is handed to it as a user message when the command ends.
pub fn chat_run_shell(
    ctx: &AppCtx,
    session_id: &str,
    command: &str,
    message_id: &str,
) -> Result<(), String> {
    if crate::security::session_active(session_id) {
        return Err("security_audit_owns_conversation".into());
    }
    // Shell mode is unavailable on Windows hosts; recorded commands still replay as rows.
    if cfg!(windows) {
        return Err(crate::agent::chat::shell::UNSUPPORTED.into());
    }
    if command.trim().is_empty() {
        return Err("chat_shell_empty".into());
    }
    if message_id.trim().is_empty() {
        return Err("A shell command needs a message id".into());
    }
    let receipt_id = crate::agent::chat::shell::submission_id(message_id);
    {
        let conn = ctx.db().conn.lock().unwrap();
        match crate::agent::chat::submissions::claim_retry(&conn, session_id, &receipt_id, command.as_bytes())? {
            crate::agent::chat::submissions::Claim::New => {},
            crate::agent::chat::submissions::Claim::Complete(result) => return result.map(|_| ()),
        }
    }
    use crate::agent::chat::engine::SubmissionFailure;
    let outcome = (|| {
        if !ctx.chat().is_alive(session_id) {
            chat_start(ctx, session_id, None, None, false).map_err(SubmissionFailure::Rejected)?;
        }
        ctx.chat().run_shell_checked(ctx, session_id, command, message_id)?;
        Ok("accepted".to_string())
    })();
    if let Err(SubmissionFailure::Rejected(error)) = &outcome {
        crate::agent::chat::submissions::finish_rejected(&ctx.db().conn.lock().unwrap(), session_id, &receipt_id, error)?;
        return Err(error.clone());
    }
    let outcome = outcome.map_err(|_: SubmissionFailure| "chat_submission_pending".to_string());
    crate::agent::chat::submissions::finish(&ctx.db().conn.lock().unwrap(), session_id, &receipt_id, &outcome)?;
    outcome.map(|_| ())
}

/// Stop the shell command `message_id` names, together with everything it started.
pub fn chat_cancel_shell(ctx: &AppCtx, session_id: &str, message_id: &str) -> Result<(), String> {
    if crate::security::session_active(session_id) {
        return Err("security_audit_owns_conversation".into());
    }
    ctx.chat().cancel_shell(session_id, message_id)
}

/// Refuse a message whose attachments are past the limits. The last line of defence rather than the
/// first: the composer says the same thing in the user's own language before the bytes are ever sent.
pub(crate) fn check_images(images: &[ChatImage]) -> Result<(), String> {
    if images.len() > MAX_IMAGES_PER_MESSAGE {
        return Err(format!(
            "At most {MAX_IMAGES_PER_MESSAGE} images can be sent with one message"
        ));
    }
    for image in images {
        if !image.mime_type.starts_with("image/") {
            return Err(format!("{} is not an image", image.mime_type));
        }
        // Base64 carries three bytes in every four characters; padding makes the estimate slightly high,
        // which only ever rejects a file already at the very edge of the limit.
        if image.data.len() / 4 * 3 > MAX_IMAGE_BYTES {
            return Err(format!(
                "Each image must be under {} MB",
                MAX_IMAGE_BYTES / (1024 * 1024)
            ));
        }
    }
    Ok(())
}

/// Add a queued message to the current turn after native acceptance.
pub fn chat_queue_steer(ctx: &AppCtx, session_id: &str, id: &str) -> Result<(), String> {
    ctx.chat().queue_steer(ctx, session_id, id)
}

/// Drop a message that is still waiting behind the running turn.
pub fn chat_queue_remove(ctx: &AppCtx, session_id: &str, id: &str) -> Result<(), String> {
    ctx.chat().queue_remove(ctx, session_id, id)
}

/// Rewrite a message that is still waiting behind the running turn.
pub fn chat_queue_update(
    ctx: &AppCtx,
    session_id: &str,
    id: &str,
    text: &str,
) -> Result<(), String> {
    ctx.chat().queue_update(ctx, session_id, id, text)
}

/// Stop the turn in progress, leaving the conversation open.
pub fn chat_interrupt(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    ctx.chat().interrupt(ctx, session_id)
}

/// Answer a permission question raised by a tool the agent wants to run.
///
/// `updated_permissions` is the standing rule the answer adopts, taken from the question's own
/// suggestions — what "allow, and stop asking me about this" sends.
#[allow(clippy::too_many_arguments)]
pub fn chat_permission(
    ctx: &AppCtx,
    session_id: &str,
    request_id: &str,
    allow: bool,
    updated_input: Option<serde_json::Value>,
    message: Option<&str>,
    updated_permissions: Option<serde_json::Value>,
) -> Result<(), String> {
    ctx.chat().respond_permission(
        ctx,
        session_id,
        request_id,
        allow,
        updated_input,
        message,
        updated_permissions,
    )
}

/// Save the permission choice. Codex adopts it on the next turn without interrupting the active one.
///
/// Remembering matters because this is the only permission control a chat session shows: the status bar's
/// two-state toggle applies at launch and is hidden for these sessions, so if the choice were not stored,
/// every restart would silently go back to asking.
pub fn chat_set_mode(ctx: &AppCtx, session_id: &str, mode: &str) -> Result<(), String> {
    let session = session_settings::session(ctx, session_id)?;
    let mode = crate::agent::permission_catalog::normalize(session.kind, Some(mode))?;
    // The agent starts with the first message, so a mode chosen before that has nobody to tell yet; the
    // stored value below is what that launch reads.
    let restart_required = if ctx.chat().is_alive(session_id) {
        match ctx.chat().set_mode(ctx, session_id, mode) {
            Ok(()) => None,
            Err(error) if error.starts_with("CHAT_PERMISSION_RESTART_REQUIRED:") => Some(error),
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    // Terminal sessions have historically stored the unrestricted choice as `skip`; keep that spelling
    // at rest so switching engines does not silently turn confirmations back on.
    let stored = if matches!(mode, "bypassPermissions" | "full-access") {
        "skip"
    } else {
        mode
    };
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_permission_mode(&conn, session_id, stored)?;
        if session.kind == SessionKind::Antigravity {
            let acp_mode = crate::agent::chat::antigravity_protocol::acp_mode_for_permission(mode);
            repo::set_collaboration_mode(&conn, session_id, acp_mode)?;
        }
    }
    ctx.emit(TREE_CHANGED, ());
    // Keep a deferred choice in the database without claiming the running process adopted it.
    // Returning the restart requirement opens the confirmation; dismissing it keeps the choice pending.
    if let Some(error) = restart_required {
        return Err(error);
    }
    ctx.emit(
        &crate::agent::chat::engine::event_name(session_id),
        serde_json::json!({
            "type":"settingsChanged",
            "mode":mode,
            "pendingPermissionMode":ctx.chat().pending_permission_mode(session_id),
        }),
    );
    Ok(())
}

/// Change and persist the collaboration style used by subsequent Codex turns.
pub fn chat_set_collaboration_mode(
    ctx: &AppCtx,
    session_id: &str,
    mode: &str,
) -> Result<(), String> {
    let session = session_settings::session(ctx, session_id)?;
    if session.kind == SessionKind::Antigravity {
        return Err("antigravity does not support collaboration modes".into());
    }
    if ctx.chat().is_alive(session_id) {
        ctx.chat().set_collaboration_mode(ctx, session_id, mode)?;
    }
    {
        let conn = ctx.db().conn.lock().unwrap();
        repo::set_collaboration_mode(&conn, session_id, mode)?;
    }
    ctx.emit(TREE_CHANGED, ());
    Ok(())
}

/// Change the model of the running conversation; an empty value restores the default.
pub fn chat_set_model(ctx: &AppCtx, session_id: &str, model: Option<&str>) -> Result<(), String> {
    let session = session_settings::session(ctx, session_id)?;
    let mut selection = session_settings::resolve(ctx, &session)?;
    selection.model = session_settings::clean(model);
    if session.kind == SessionKind::Deepseek {
        selection.model = crate::agent::chat::deepseek_models::normalize_model(selection.model.as_deref()).into();
        selection.effort = crate::agent::chat::deepseek_models::normalize_effort(selection.effort.as_deref());
    } else if session.kind == SessionKind::Antigravity {
        crate::agent::antigravity_models::normalize_selection(ctx, &mut selection);
    }
    // Only Copilot passes `--model` exclusively at ACP launch; other ACP agents switch live over
    // the protocol once their handshake settles.
    let launch_only = session.kind == SessionKind::Copilot && ctx.chat().is_alive(session_id)
        && !ctx.chat().acp_switchable(session_id).0;
    if launch_only {
        return ctx.chat().update_launch_settings(ctx, session_id, || {
            session_settings::persist(ctx, &session, &selection)?;
            ctx.emit(&crate::agent::chat::engine::event_name(session_id), serde_json::json!({
                "type":"settingsChanged","model":selection.model,"images":ctx.chat().image_support(session_id)
            }));
            Ok(())
        });
    }
    if ctx.chat().is_alive(session_id) {
        ctx.chat()
            .set_model_with_app(ctx, session_id, selection.model.as_deref())?;
        if ctx.chat().is_alive(session_id) {
            let snap = ctx.chat().snapshot(session_id);
            selection.model = snap.model;
            if session.kind == SessionKind::Antigravity {
                ctx.chat().sync_antigravity_catalog(ctx, session_id);
                if snap.running {
                    selection.effort = snap.effort;
                }
                crate::agent::antigravity_models::normalize_selection(ctx, &mut selection);
            }
        } else if session.kind == SessionKind::Antigravity {
            crate::agent::antigravity_models::normalize_selection(ctx, &mut selection);
        }
    }
    session_settings::persist(ctx, &session, &selection)?;
    let payload = if session.kind == SessionKind::Antigravity {
        serde_json::json!({
            "type":"settingsChanged",
            "model":selection.model,
            "effort":selection.effort,
            "images":ctx.chat().image_support(session_id)
        })
    } else {
        serde_json::json!({
            "type":"settingsChanged",
            "model":selection.model,
            "images":ctx.chat().image_support(session_id)
        })
    };
    ctx.emit(&crate::agent::chat::engine::event_name(session_id), payload);
    Ok(())
}

/// Change reasoning effort. Codex applies it as a turn option; Claude answers through `/effort`.
pub fn chat_set_effort(ctx: &AppCtx, session_id: &str, effort: Option<&str>) -> Result<(), String> {
    let session = session_settings::session(ctx, session_id)?;
    let mut selection = session_settings::resolve(ctx, &session)?;
    selection.effort = session_settings::clean(effort);
    if session.kind == SessionKind::Deepseek {
        selection.effort = crate::agent::chat::deepseek_models::normalize_effort(selection.effort.as_deref());
    } else if session.kind == SessionKind::Antigravity {
        crate::agent::antigravity_models::normalize_selection(ctx, &mut selection);
    }
    let launch_only = session.kind == SessionKind::Copilot && ctx.chat().is_alive(session_id)
        && !ctx.chat().acp_switchable(session_id).1;
    if launch_only {
        return ctx.chat().update_launch_settings(ctx, session_id, || {
            session_settings::persist(ctx, &session, &selection)?;
            ctx.emit(&crate::agent::chat::engine::event_name(session_id), serde_json::json!({"type":"settingsChanged","effort":selection.effort}));
            Ok(())
        });
    }
    if ctx.chat().is_alive(session_id) {
        ctx.chat()
            .set_effort(ctx, session_id, selection.effort.as_deref())?;
    }
    session_settings::persist(ctx, &session, &selection)?;
    ctx.emit(
        &crate::agent::chat::engine::event_name(session_id),
        serde_json::json!({"type":"settingsChanged","effort":selection.effort}),
    );
    Ok(())
}

/// Grok's model picker: what the running session offers first, then every model the CLI names, then any
/// model only the config file describes. Each entry carries that model's own reasoning ladder.
fn grok_chat_models(ctx: &AppCtx, session_id: &str, bin: &str) -> serde_json::Value {
    let live = ctx.chat().acp_models(ctx, session_id).unwrap_or_default();
    let ids = grok_cli_models(bin);
    let config = crate::agent::grok_models::load();
    serde_json::Value::Array(assemble_grok_catalog(&live, &ids, &config))
}

/// Merge the live session's choices with the CLI listing and Grok's config file.
///
/// A running Grok session reports only the model it started with, while `grok models` names every model
/// the account can switch to and `config.toml` says which reasoning levels each one offers. A live
/// ladder wins over the config, since it describes the session that is actually running.
fn assemble_grok_catalog(
    live: &[serde_json::Value],
    ids: &[String],
    config: &std::collections::HashMap<String, crate::agent::grok_models::ConfigModel>,
) -> Vec<serde_json::Value> {
    let mut order: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for model in live {
        if let Some(id) = model.get("id").and_then(serde_json::Value::as_str) {
            if !id.is_empty() && seen.insert(id.to_string()) { order.push(id.to_string()); }
        }
    }
    for id in ids {
        if !id.is_empty() && seen.insert(id.clone()) { order.push(id.clone()); }
    }
    // Config-only models go last, in a stable order so the menu does not shuffle between reads.
    let mut config_only: Vec<&String> = config.keys().filter(|id| !seen.contains(*id)).collect();
    config_only.sort();
    order.extend(config_only.into_iter().cloned());
    let live_by: std::collections::HashMap<&str, &serde_json::Value> = live.iter()
        .filter_map(|model| model.get("id").and_then(serde_json::Value::as_str).map(|id| (id, model)))
        .collect();
    order.into_iter().map(|id| {
        let live_model = live_by.get(id.as_str()).copied();
        let configured = config.get(&id);
        let live_levels = levels_of(live_model, "effortLevels");
        let effort_levels = if !live_levels.is_empty() {
            live_levels
        } else {
            configured.map(|model| model.effort_levels.clone()).unwrap_or_default()
        };
        let label = text_field(live_model, "label")
            .or_else(|| configured.map(|model| model.label.as_str()))
            .unwrap_or(id.as_str());
        let description = text_field(live_model, "description").unwrap_or_default();
        serde_json::json!({"id":id,"label":label,"description":description,"effortLevels":effort_levels})
    }).collect()
}

/// A non-empty string field of one catalogue entry.
fn text_field<'a>(model: Option<&'a serde_json::Value>, key: &str) -> Option<&'a str> {
    model
        .and_then(|model| model.get(key))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
}

/// A catalogue entry's string array field, such as `effortLevels`.
fn levels_of(model: Option<&serde_json::Value>, key: &str) -> Vec<String> {
    model
        .and_then(|model| model.get(key))
        .and_then(serde_json::Value::as_array)
        .map(|values| values.iter().filter_map(|value| value.as_str()).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Asking `grok models` starts a CLI that answers on its own timeout, and the picker re-reads the
/// catalogue after every model or reasoning change. Keep the last answer briefly so one session does not
/// pay for the same listing on every refresh.
fn grok_cli_models(bin: &str) -> Vec<String> {
    const TTL: std::time::Duration = std::time::Duration::from_secs(300);
    type Cached = (String, std::time::Instant, Vec<String>);
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<Cached>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(cache) = cache.lock() {
        if let Some((cached_bin, at, models)) = cache.as_ref() {
            if cached_bin == bin && at.elapsed() < TTL { return models.clone(); }
        }
    }
    let models = crate::agent::model_catalog::list_models_at("grok", bin).unwrap_or_default();
    if let Ok(mut cache) = cache.lock() {
        *cache = Some((bin.to_string(), std::time::Instant::now(), models.clone()));
    }
    models
}

pub fn chat_models(ctx: &AppCtx, session_id: &str) -> Result<serde_json::Value, String> {
    let session = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::get_session(&conn, session_id)?.ok_or("Session not found")?
    };
    match session.kind {
        // Live capabilities enrich the complete catalogue; the CLI picker only lists a subset. What a
        // running conversation reports is kept, so a session that is not running still offers the models
        // only the installed CLI knows about.
        SessionKind::Claude => {
            let bin = crate::agent::executable::for_session(ctx, &session);
            let models = match ctx.chat().live_claude_models(session_id) {
                Some(live) => {
                    crate::agent::claude_models::remember_reported(&bin, &live);
                    live
                }
                None => crate::agent::claude_models::list_for_bin(&bin),
            };
            serde_json::to_value(models)
        }
        SessionKind::Codex => {
            if let Some(models) = ctx.chat().live_codex_models(session_id) {
                return serde_json::to_value(models?).map_err(|e| e.to_string());
            }
            let bin = crate::agent::executable::for_session(ctx, &session);
            let args = crate::agent::inject::split_extra_args(session.agent_args.as_deref());
            let root = {
                let conn = ctx.db().conn.lock().unwrap();
                repo::get_project_root(&conn, &session.project_id)?
            };
            let cwd = session.cwd.as_deref().filter(|c| !c.trim().is_empty()).or(root.as_deref());
            serde_json::to_value(crate::agent::codex_models::list_in_dir(&bin, &args, cwd)?)
        }
        SessionKind::Opencode | SessionKind::Deepseek => {
            // The running server already knows its providers; without one, a short-lived server answers.
            let bin = crate::agent::executable::for_session(ctx, &session);
            let root = {
                let conn = ctx.db().conn.lock().unwrap();
                repo::get_project_root(&conn, &session.project_id)?
            };
            let cwd = session
                .cwd
                .as_deref()
                .filter(|c| !c.trim().is_empty())
                .map(str::to_string)
                .or(root);
            let running = ctx.chat().opencode_server(session_id);
            let provider = (session.kind == SessionKind::Deepseek)
                .then_some(crate::agent::chat::deepseek_models::PROVIDER);
            let models = match crate::agent::opencode_models::list_provider(
                ctx,
                session_id,
                &bin,
                cwd.as_deref(),
                running,
                provider,
            ) {
                Ok(models) if !models.is_empty() => models,
                Ok(_) if session.kind == SessionKind::Deepseek => crate::agent::chat::deepseek_models::shipped(),
                Err(_) if session.kind == SessionKind::Deepseek => crate::agent::chat::deepseek_models::shipped(),
                Ok(models) => models,
                Err(error) => return Err(error),
            };
            serde_json::to_value(models)
        }
        SessionKind::Zcode => {
            if let Some(models) = ctx.chat().zcode_models(session_id) {
                return serde_json::to_value(models).map_err(|e| format!("Failed to serialize model catalogue: {e}"));
            }
            let bin = crate::agent::executable::for_session(ctx, &session);
            let root = {
                let conn = ctx.db().conn.lock().unwrap();
                repo::get_project_root(&conn, &session.project_id)?
            };
            // The directory the conversation itself would start in. A session in Chats has neither its own
            // directory nor a project root, and runs in home.
            let cwd = crate::host::spawn_dir(session.cwd.as_deref().filter(|c| !c.trim().is_empty()).or(root.as_deref()))
                .ok_or("ZCode needs a working directory to list its models")?;
            serde_json::to_value(crate::agent::zcode::list_models(&bin, &cwd)?)
        }
        // A running ACP session lists the models it can switch to. Without one, the CLI's own catalogue is
        // what its next launch can be given.
        kind if crate::agent::chat::acp_protocol::supported(kind) => {
            // Grok's picker merges three sources, so it answers as one list rather than the live one.
            if kind == SessionKind::Grok {
                let bin = crate::agent::executable::for_session(ctx, &session);
                return Ok(grok_chat_models(ctx, session_id, &bin));
            }
            if kind == SessionKind::Antigravity {
                let bin = crate::agent::executable::for_session(ctx, &session);
                crate::agent::antigravity_models::schedule_refresh(ctx, &bin);
                if let Some(models) = ctx.chat().acp_models(ctx, session_id) {
                    return Ok(serde_json::Value::Array(models));
                }
                return serde_json::to_value(crate::agent::antigravity_models::cached(ctx))
                    .map_err(|e| format!("Failed to serialize model catalogue: {e}"));
            }
            if let Some(models) = ctx.chat().acp_models(ctx, session_id) {
                return Ok(serde_json::Value::Array(models));
            }
            let efforts: &[&str] = if kind == SessionKind::Copilot {
                crate::agent::chat::acp_protocol::COPILOT_EFFORTS
            } else {
                &[]
            };
            let models = if kind == SessionKind::Kimi {
                crate::agent::model_catalog::list_models(ctx, kind.as_str())?
            } else {
                let bin = crate::agent::executable::for_session(ctx, &session);
                crate::agent::model_catalog::list_models_at(kind.as_str(), &bin)?
            };
            serde_json::to_value(models.into_iter().map(|id| serde_json::json!({
                "label":id,"id":id,"description":"","effortLevels":efforts,
            })).collect::<Vec<_>>())
        }
        SessionKind::Pi | SessionKind::Omp => {
            if let Some(models) = ctx.chat().pi_models(session_id) {
                return serde_json::to_value(models)
                    .map_err(|e| format!("Failed to serialize model catalogue: {e}"));
            }
            let bin = crate::agent::executable::for_session(ctx, &session);
            let root = {
                let conn = ctx.db().conn.lock().unwrap();
                repo::get_project_root(&conn, &session.project_id)?
            };
            let cwd = session
                .cwd
                .as_deref()
                .filter(|c| !c.trim().is_empty())
                .map(str::to_string)
                .or(root);
            let args = session_settings::without_selection_args(
                session.kind,
                session.agent_args.as_deref(),
            );
            serde_json::to_value(crate::agent::pi_models::list(
                session.kind,
                &bin,
                cwd.as_deref(),
                &crate::agent::inject::split_extra_args(Some(&args)),
            )?)
        }
        _ => {
            return Err(format!(
                "The chat engine does not support {} sessions",
                session.kind.as_str()
            ))
        }
    }
    .map_err(|e| format!("Failed to serialize model catalogue: {e}"))
}

/// Read the provider catalogue even before the first message, without starting a conversation.
pub fn chat_commands(ctx: &AppCtx, session_id: &str) -> Result<Vec<serde_json::Value>, String> {
    if let Some(commands) = ctx
        .chat()
        .live_commands(session_id)
        .filter(|commands| !commands.is_empty())
    {
        return Ok(commands);
    }
    let session = session_settings::session(ctx, session_id)?;
    if !matches!(session.kind, SessionKind::Codex | SessionKind::Claude | SessionKind::Pi | SessionKind::Omp) {
        return Ok(Vec::new());
    }
    let root = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::get_project_root(&conn, &session.project_id)?
    };
    let cwd = session
        .cwd
        .as_deref()
        .filter(|c| !c.trim().is_empty())
        .or(root.as_deref());
    let bin = crate::agent::executable::for_session(ctx, &session);
    let args =
        session_settings::without_selection_args(session.kind, session.agent_args.as_deref());
    crate::agent::chat::skills::lookup(
        session.kind,
        &bin,
        cwd,
        &crate::agent::inject::split_extra_args(Some(&args)),
    )
}

/// The fresh host declares dormant chat facts before returning a reconnect snapshot.
pub fn session_states(ctx: &AppCtx) -> Result<std::collections::HashMap<String,crate::session_state::SessionState>,String> {
    let known=crate::session_state::snapshot();
    let sessions=repo::list_all_sessions(&ctx.db().conn.lock().unwrap())?;
    for session in sessions.into_iter().filter(|s|s.engine=="chat" && s.archived_at.is_none() && !known.contains_key(&s.id)) {
        if !ctx.chat().is_alive(&session.id) && crate::agent::chat::ownership::idle_guard(ctx,&session.id).is_ok() {
            crate::session_state::restore_stopped_chat(ctx,&session.id,session.kind.as_str());
        }
    }
    Ok(crate::session_state::snapshot())
}

/// Everything a client needs to draw the conversation from scratch, including one that just connected.
pub fn chat_snapshot(
    ctx: &AppCtx,
    session_id: &str,
) -> Result<crate::agent::chat::engine::ChatSnapshot, String> {
    chat_snapshot_window(ctx, session_id, None)
}

pub fn chat_snapshot_control(
    ctx: &AppCtx,
    session_id: &str,
    window: Option<&crate::agent::chat::engine::ChatWindow>,
) -> Result<crate::agent::chat::engine::ChatSnapshot, String> {
    let mut snapshot = ctx.chat().snapshot_window(session_id, window);
    let mut session_kind = None;
    if !snapshot.running {
        let session = session_settings::session(ctx, session_id)?;
        session_kind = Some(session.kind);
        let selection = session_settings::resolve(ctx, &session)?;
        snapshot.model = selection.model.clone();
        snapshot.effort = selection.effort.clone();
        snapshot.selection = Some(selection);
    }
    {
        let conn = ctx.db().conn.lock().unwrap();
        let (tier, personality) = repo::codex_chat_settings(&conn, session_id)?;
        snapshot.service_tier = tier;
        snapshot.personality = personality;
        let kind = match session_kind {
            Some(kind) => Some(kind),
            None => repo::get_session_kind(&conn, session_id)?,
        };
        if kind == Some(SessionKind::Claude) {
            snapshot.chrome = Some(session_settings::chrome(&conn, session_id)?);
        }
    }
    Ok(snapshot)
}

pub fn chat_snapshot_window(
    ctx: &AppCtx,
    session_id: &str,
    window: Option<&crate::agent::chat::engine::ChatWindow>,
) -> Result<crate::agent::chat::engine::ChatSnapshot, String> {
    let mut snapshot = chat_snapshot_control(ctx, session_id, window)?;
    // No process has run this conversation since the backend started, so the engine holds no timeline for
    // it. The agent's own recording still does. Reading it here means a reopened session shows what was
    // said before anything else happens — and keeps showing it when the agent cannot be started at all,
    // instead of an empty pane under an error.
    if snapshot.pid.is_none() {
        let session = {
            let conn = ctx.db().conn.lock().unwrap();
            repo::get_session(&conn, session_id)?
        };
        if let Some((kind, id)) = session.and_then(|s| {
            s.agent_session_id.or(s.legacy_agent_session_id).map(|id| (s.kind, id))
        }) {
            let replayed = if crate::agent::chat::acp_protocol::supported(kind) {
                crate::agent::chat::acp_protocol::replay(ctx.data_dir().ok().as_deref(), kind, &id)
            } else {
                crate::agent::chat::history::replay(kind, &id)
            };
            match replayed {
                Ok(rows) => snapshot.rows = rows,
                Err(e) if e.starts_with("detail_requires_paging:") => return Err(e),
                // A recording that is gone, or a thread Codex never wrote, is the same empty pane as before.
                Err(e) => crate::diagnostic_warn!("chat: no history replayed for {id}: {e}"),
            }
            snapshot.agent_session_id = Some(id);
        }
    }
    if snapshot.pid.is_none() {
        snapshot.user_messages = Some(crate::agent::chat::user_messages::outline(&snapshot.rows));
        snapshot.total_rows = snapshot.rows.len();
        snapshot.positions = snapshot
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| (row.id().to_owned(), index))
            .collect();
        if let Some(window) = window {
            let end = window
                .before
                .as_ref()
                .and_then(|id| snapshot.rows.iter().position(|row| row.id() == id))
                .unwrap_or(snapshot.rows.len());
            let start = end.saturating_sub(crate::agent::chat::engine::SNAPSHOT_PAGE_ROWS);
            snapshot.page_kind = if window.before.is_some() && end < snapshot.rows.len() {
                "history"
            } else {
                "recent"
            };
            snapshot.has_more = start > 0;
            if snapshot.page_kind == "history" { snapshot.user_messages = None; }
            snapshot.rows = snapshot.rows[start..end].to_vec();
            snapshot
                .positions
                .retain(|_, index| *index >= start && *index < end);
        }
    }
    Ok(snapshot)
}

/// Expanded tool details remain available when an inactive conversation is replayed from disk.
pub fn chat_row(
    ctx: &AppCtx,
    session_id: &str,
    row_id: &str,
    epoch: Option<u64>,
) -> Result<serde_json::Value, String> {
    if ctx
        .chat()
        .snapshot_window(session_id, Some(&crate::agent::chat::engine::ChatWindow {control_only:true,..Default::default()}))
        .pid
        .is_some()
    {
        return ctx.chat().row_detail(session_id, row_id, epoch);
    }
    if epoch.is_some() {
        return Err("Chat history changed; synchronize the conversation again".into());
    }
    let snapshot = chat_snapshot(ctx, session_id)?;
    fn find(rows: &[crate::agent::chat::engine::ChatRow], id: &str) -> Option<serde_json::Value> {
        for row in rows {
            if row.id() == id {
                return serde_json::to_value(row).ok();
            }
            if let crate::agent::chat::engine::ChatRow::Tool { children, .. } = row {
                if let Some(value) = find(children, id) {
                    return Some(value);
                }
            }
        }
        None
    }
    find(&snapshot.rows, row_id).ok_or_else(|| "Chat row is no longer available".into())
}

/// Resolve image bytes referenced by a slim chat snapshot.
pub fn chat_attachment(
    ctx: &AppCtx,
    session_id: &str,
    attachment_id: &str,
) -> Result<ChatImage, String> {
    ctx.chat().attachment(session_id, attachment_id)
}

/// Preview the Claude file checkpoint associated with one visible user message.
pub fn chat_rewind_preview(
    ctx: &AppCtx,
    session_id: &str,
    row_id: &str,
    history_generation:Option<&str>,
) -> Result<serde_json::Value, String> {
    if let Some(generation)=history_generation {ctx.chat().rewind_indexed_preview(ctx,session_id,row_id,generation)}
    else {ctx.chat().rewind_preview(session_id, row_id)}
}

/// Rewind conversation state, Claude file checkpoints, or both from one visible user message.
pub fn chat_rewind(
    ctx: &AppCtx,
    session_id: &str,
    row_id: &str,
    scope: &str,
    history_generation:Option<&str>,
) -> Result<serde_json::Value, String> {
    if crate::security::session_active(session_id) {
        return Err("security_audit_owns_conversation".into());
    }
    // A scheduled continuation would send a message while the rewind stops and restarts the agent.
    crate::agent::chat::auto_continue::cancel(ctx, session_id);
    let result = if let Some(generation)=history_generation {ctx.chat().rewind_indexed(ctx,session_id,row_id,scope,generation)?}
        else {ctx.chat().rewind(ctx, session_id, row_id, scope)?};
    Ok(result)
}

/// Stop waiting for a usage limit to reset; the conversation will not continue on its own.
pub fn chat_auto_continue_cancel(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    crate::agent::chat::auto_continue::cancel(ctx, session_id);
    Ok(())
}

/// A view started showing this conversation. Cancels a release a closed view asked for; starts nothing.
pub fn chat_attach(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    ctx.chat().attach(session_id);
    // Release the DB lock before resolving the executable, because `executable::for_session`
    // locks `ctx.db().conn` again; in Rust 2021 an inline `if let` scrutinee holds the guard
    // across the whole block and self-deadlocks the global SQLite connection.
    let session = {
        let conn = ctx.db().conn.lock().unwrap();
        repo::get_session(&conn, session_id).ok().flatten()
    };
    if let Some(session) = session {
        if session.kind == SessionKind::Antigravity {
            let bin = crate::agent::executable::for_session(ctx, &session);
            crate::agent::antigravity_models::schedule_refresh(ctx, &bin);
        }
    }
    Ok(())
}

/// A view stopped showing this conversation. Its process goes as soon as it is idle; the conversation
/// stays, and the next message sent restarts the agent where it left off.
pub fn chat_detach(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    ctx.chat().detach(ctx, session_id);
    Ok(())
}

/// End the conversation and let its process go.
pub fn chat_stop(ctx: &AppCtx, session_id: &str) -> Result<(), String> {
    crate::security::cancel_session(ctx, session_id)?;
    ctx.chat().stop(ctx, session_id)
}

/// Reads an agent conversation for the session view — the chat-style second view of a live session.
///
/// This is the same recording `read_agent_transcript` reads, parsed more completely: reasoning and tool
/// calls are kept, and each tool call carries its raw input so the view can draw a card per tool.
/// A session that has not run a turn yet has no agent-native id and therefore no recording. That is an empty
/// conversation, not a failure: reading it returns no rows, so the view says there is nothing here yet rather
/// than printing a backend error over a session that is simply new. The other three commands sharing
/// `session_kind_and_agent` keep the error — asking for usage or an export of a session that never ran is a
/// question with no answer, while asking for its conversation has one.
pub fn read_agent_chat(
    ctx: &AppCtx,
    session_id: &str,
) -> Result<Vec<crate::agent::chat::ChatEvent>, String> {
    let (kind, agent_id) = {
        let conn = ctx.db().conn.lock().unwrap();
        let session = repo::get_session(&conn, session_id)?;
        let kind = session.as_ref().map(|session| session.kind);
        let agent_id = session.and_then(|session| session.agent_session_id.or(session.legacy_agent_session_id));
        (kind, agent_id)
    };
    let kind = kind.ok_or("Session not found")?;
    let Some(agent_id) = agent_id else {
        return Ok(Vec::new());
    };
    crate::agent::chat::read(kind, &agent_id)
}

/// Queries model/context usage for the Info panel (Claude, Codex, Grok, OpenCode, Pi, OMP, DeepSeek).
pub fn agent_context_info(
    ctx: &AppCtx,
    session_id: &str,
) -> Result<crate::agent::transcript::AgentContextInfo, String> {
    // A running chat process reports its own usage after every turn, which is both fresher and more
    // exact than what the recording's tail can be made to say.
    if let Some(info) = ctx.chat().context_info(session_id) {
        return Ok(info);
    }
    let (kind, agent_id) = session_kind_and_agent(ctx, session_id)?;
    if kind==SessionKind::Kiro {
        let context=crate::agent::history_store::context::kiro(ctx,session_id)?;
        return Ok(crate::agent::transcript::AgentContextInfo {model:context.model,context_tokens:None,context_limit:context.limit.unwrap_or(0),current_tool:None});
    }
    let mut info = crate::agent::transcript::context_info(kind, &agent_id)?;
    // Pi and OMP leave the limit to their catalogue; the running process answers with the catalogue the
    // CLI actually uses, which is fresher than the copy it cached on disk.
    if info.context_limit == 0 {
        if let Some(model) = info.model.as_deref() {
            info.context_limit = ctx
                .chat()
                .model_context_window(session_id, model)
                .unwrap_or(0);
        }
    }
    Ok(info)
}

/// Provider-reported turn/session usage and recorded file activity for the Info panel.
pub fn agent_turn_stats(
    ctx: &AppCtx,
    session_id: &str,
) -> Result<crate::agent::transcript::TurnStats, String> {
    let (kind, agent_id) = session_kind_and_agent(ctx, session_id)?;
    if kind==SessionKind::Kiro {return Ok(crate::agent::kiro_info::from_context(crate::agent::history_store::context::kiro(ctx,session_id)?));}
    let mut stats = crate::agent::transcript::current_turn_stats(kind, &agent_id)?;
    if let Ok(context) = agent_context_info(ctx, session_id) {
        stats.with_context(&context);
    }
    ctx.chat().enrich_turn_stats(session_id, &mut stats);
    Ok(stats)
}

/// Exports complete session context as Markdown. With desktop `dest_path`, writes to disk and returns None; without
/// it on browser clients, returns Some(content) for a local frontend download.
pub fn export_session_context(
    ctx: &AppCtx,
    session_id: &str,
    dest_path: Option<&str>,
    exported_at: Option<&str>,
) -> Result<Option<String>, String> {
    let (kind, agent_id, legacy_id, name) = {
        let conn = ctx.db().conn.lock().unwrap();
        (
            repo::get_session_kind(&conn, session_id)?,
            repo::get_agent_session_id(&conn, session_id)?,
            repo::get_session(&conn, session_id)?.and_then(|session| session.legacy_agent_session_id),
            repo::get_session_name(&conn, session_id)?,
        )
    };
    let kind = kind.ok_or("Session not found")?;
    let name = name.unwrap_or_else(|| "Session".to_string());
    // DeepSeek can have two native recordings: the retired harness log and the OpenCode session that
    // followed it. The bounded markdown writer reads the whole branch, so the export is one document
    // instead of whichever half a single native id names.
    let md = if kind == SessionKind::Deepseek {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let branch = crate::agent::history_store::event_stream::complete(ctx, session_id, &cancel)?;
        let mut bytes = Vec::new();
        crate::agent::history_store::markdown::write_markdown(branch, &name, exported_at, &cancel, &mut bytes)?;
        String::from_utf8(bytes).map_err(|e| format!("Failed to build the context export: {e}"))?
    } else {
        let agent_id = agent_id.or(legacy_id)
            .ok_or("No agent session id captured for this session yet; nothing to export")?;
        crate::agent::export::export_markdown(kind, &agent_id, &name, exported_at)?
    };
    match dest_path {
        Some(p) => {
            std::fs::write(p, md).map_err(|e| format!("Failed to write export file: {e}"))?;
            Ok(None)
        }
        None => Ok(Some(md)),
    }
}

// Remote-access web service. WS/headless dispatch toggles LAN remote mode through `ctx.remote_web()`. Desktop
// Tauri commands access the same managed WebServer directly. LAN remote mode uses self-signed TLS on every
// interface by default, or on the one address (or loopback) the owner chose, independently of Electron
// sidecar's persistent plaintext loopback instance.
//
// The run state persists in app_settings so a restart can restore it (GitHub issue #15): enabled flag, port,
// mode, listen address, and an Argon2id PHC hash of the password, never the plaintext. Persistence lives only
// here in the core; the transport adapters (commands.rs / dispatch.rs) stay thin.

/// app_settings key: "1" while remote access should auto-start on launch, "0" after a manual stop.
const REMOTE_ENABLED_KEY: &str = "remoteAccess.enabled";
/// app_settings key: last successfully used listening port (decimal string).
const REMOTE_PORT_KEY: &str = "remoteAccess.port";
/// app_settings key: "1" when the last start used plaintext LAN HTTP mode (dev only), otherwise "0".
const REMOTE_LAN_HTTP_KEY: &str = "remoteAccess.lanHttp";
/// app_settings key: Argon2id PHC verifier of the access password; the only password-derived value on disk.
const REMOTE_PASSWORD_HASH_KEY: &str = "remoteAccess.passwordHash";

/// app_settings key: where remote access listens, canonical `all` (absent means all), `loopback`, or an IPv4.
const REMOTE_BIND_KEY: &str = "remoteAccess.bind";
/// app_settings key: host name or IP used in pairing links when listening on loopback behind a tunnel.
const REMOTE_PAIRING_HOST_KEY: &str = "remoteAccess.pairingHost";

/// Pure: the listen configuration from app settings. `bind_override` (the `--bind` flag of the headless CLI)
/// replaces the stored bind choice, which is then not parsed at all. Invalid stored values are errors, never a
/// silent fallback to every interface.
fn listen_config_from(
    settings: &std::collections::HashMap<String, String>,
    bind_override: Option<crate::web::BindChoice>,
) -> Result<crate::web::ListenConfig, String> {
    let bind = match bind_override {
        Some(bind) => bind,
        None => crate::web::BindChoice::parse(
            settings.get(REMOTE_BIND_KEY).map(String::as_str).unwrap_or(""),
        )?,
    };
    let pairing_host = crate::web::validate_pairing_host(
        settings
            .get(REMOTE_PAIRING_HOST_KEY)
            .map(String::as_str)
            .unwrap_or(""),
    )?;
    Ok(crate::web::ListenConfig { bind, pairing_host })
}

/// The persisted remote-access listen configuration, optionally with the bind choice overridden for this
/// process (see [`listen_config_from`]). Used by the desktop start paths and the headless CLI instance.
pub fn remote_listen_config(
    ctx: &AppCtx,
    bind_override: Option<crate::web::BindChoice>,
) -> Result<crate::web::ListenConfig, String> {
    listen_config_from(&get_app_settings(ctx)?, bind_override)
}

/// Persist remote-access settings; failures are logged and never abort the running service.
fn persist_remote_settings(ctx: &AppCtx, entries: std::collections::HashMap<String, String>) {
    if let Err(e) = set_app_settings(ctx, entries) {
        crate::diagnostic_warn!("failed to persist remote-access settings: {e}");
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

/// Starts LAN remote access with password login and session gating. `lan_http=false` uses TLS;
/// `lan_http=true` provides plaintext for native mobile shells that cannot bypass self-signed certificates in RN
/// WebView (architecture section 20). The listen address comes from the persisted setting (every interface by
/// default); an invalid or absent address fails the start. Runtime stops any old instance before starting so
/// changes take effect. After a successful start, the enabled flag, port, mode, and password hash persist so
/// the next launch auto-starts the same configuration.
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
    let listen = remote_listen_config(ctx, None)?;
    // Hash here in the core so the same PHC string both starts the server and gets persisted for auto-start.
    let phc = crate::web::hash_password(password)?;
    let status = ctx.remote_web().start_with_listen(
        ctx.clone(),
        crate::web::StartAuth::PasswordHash(phc.clone()),
        port,
        mode,
        listen,
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
    // Return the same merged view as web_server_status and web_server_set_listen (saved bind and pairing
    // host included), so the panel never compares against a status that lacks the saved values.
    Ok(web_server_status(ctx))
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

/// Changes where remote access listens. `bind` is `all`, `loopback`, or an IPv4 address; `pairing_host`
/// None keeps the stored pairing address, `Some("")` clears it. Both are validated first; nothing changes on
/// an invalid value. While the service runs, it is restarted on the new address with the same port, mode, and
/// password verifier (see `WebServer::restart_listen`), so the owner never retypes the password and pairing
/// credentials survive; an absent address fails and leaves the running instance untouched. A change that does
/// not affect the listener (same bind, or a pairing address outside loopback mode) is stored without a
/// restart. The settings are persisted only after success. Returns the merged status for the panel.
pub fn web_server_set_listen(
    ctx: &AppCtx,
    bind: &str,
    pairing_host: Option<&str>,
) -> Result<crate::web::WebServerStatus, String> {
    let bind = crate::web::BindChoice::parse(bind)?;
    let pairing_host = match pairing_host {
        Some(raw) => crate::web::validate_pairing_host(raw)?,
        None => remote_listen_config(ctx, Some(bind.clone()))?.pairing_host,
    };
    let listen = crate::web::ListenConfig { bind, pairing_host };
    let entries = std::collections::HashMap::from([
        (REMOTE_BIND_KEY.to_string(), listen.bind.as_setting()),
        (
            REMOTE_PAIRING_HOST_KEY.to_string(),
            listen.pairing_host.clone().unwrap_or_default(),
        ),
    ]);
    match ctx.remote_web().listen_config() {
        Some(current) => {
            let listener_changes = current.bind != listen.bind
                || (listen.bind == crate::web::BindChoice::Loopback
                    && current.pairing_host != listen.pairing_host);
            if listener_changes {
                ctx.remote_web().restart_listen(ctx.clone(), listen)?;
            }
            persist_remote_settings(ctx, entries);
        }
        // Stopped: persisting is the whole effect, so a failed write is reported.
        None => set_app_settings(ctx, entries)?,
    }
    Ok(web_server_status(ctx))
}

/// app_settings key: "1" (or absent, the default) while mirror mode is on, "0" once the host turns it off.
const MIRROR_ENABLED_KEY: &str = "remoteAccess.mirror";

/// Mirror mode forced by `--serve --mirror <0|1>`, set once at startup and never afterwards.
///
/// A headless service has no panel of its own, so an SSH client states the mode when it starts the service.
/// The choice is deliberately kept in memory instead of written to app_settings: with the shared-database
/// option the service opens the remote machine's own desktop database, and one SSH connection must not
/// silently flip the checkbox that user sees in their own remote-access panel.
static MIRROR_OVERRIDE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Record the startup override. Called before the web service starts; later calls are ignored.
pub fn set_mirror_override(enabled: bool) {
    let _ = MIRROR_OVERRIDE.set(enabled);
}

/// Whether mirror mode is on: the startup override when one was given, otherwise the stored setting.
/// Absent means on: the checkbox in the remote-access panel ships checked, so a database written before
/// this feature existed must read as enabled rather than silently opting out.
fn mirror_enabled(ctx: &AppCtx) -> bool {
    if let Some(forced) = MIRROR_OVERRIDE.get() {
        return *forced;
    }
    get_app_settings(ctx)
        .ok()
        .and_then(|s| s.get(MIRROR_ENABLED_KEY).cloned())
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Current mirror mode plus the published layout, for a client aligning itself right after it connects.
///
/// `clients` and `clientList` ride along because alignment is the one moment a client has no event history:
/// `clients://changed` only fires on the next connect or disconnect, which on a quiet host may be hours away.
pub fn mirror_get(ctx: &AppCtx) -> serde_json::Value {
    let snap = crate::web::mirror::current();
    let mut out = snap.to_json();
    out["enabled"] = serde_json::Value::Bool(mirror_enabled(ctx));
    out["clients"] = serde_json::Value::from(crate::web::presence::count());
    out["clientList"] = serde_json::json!(crate::web::presence::list());
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

/// Persist one request's decision and return its backend-owned execution receipt.
pub fn resolve_spawn(
    ctx: &AppCtx, source: &str, request_id: &str, confirmed: bool,
    request: Option<crate::agent::server::SpawnRequest>,
) -> Result<crate::agent::spawn_requests::Receipt, String> {
    crate::agent::spawn_requests::decide(ctx, source, request_id, confirmed, request)
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
        status.saved_bind = settings.get(REMOTE_BIND_KEY).cloned();
        status.saved_pairing_host = settings.get(REMOTE_PAIRING_HOST_KEY).cloned();
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
pub fn web_server_autostart(ctx: &AppCtx) -> Result<Option<crate::web::WebServerStatus>, String> {
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
    // An invalid persisted listen address is a visible auto-start failure, never a silent skip and never
    // a fallback to every interface.
    let listen = match listen_config_from(&settings, None) {
        Ok(listen) => listen,
        Err(e) => {
            ctx.remote_web().set_autostart_error(Some(e.clone()));
            return Err(e);
        }
    };
    match ctx.remote_web().start_with_listen(
        ctx.clone(),
        crate::web::StartAuth::PasswordHash(phc),
        Some(port),
        mode,
        listen,
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
        assemble_grok_catalog, autostart_config, check_images, get_app_settings, install_id, listen_config_from,
        set_app_settings, web_server_autostart, web_server_set_listen,
        web_server_status, web_server_stop, ChatImage, MAX_IMAGES_PER_MESSAGE, MAX_IMAGE_BYTES,
    };
    use std::collections::HashMap;

    /// A running Grok session reports one model; the picker must still offer the CLI catalogue, and a
    /// model the CLI names without levels falls back to the ladder `config.toml` declares for it.
    #[test]
    fn grok_picker_keeps_live_choices_and_adds_cli_models() {
        let live = vec![serde_json::json!({
            "id":"magpie/workbuddy-ai/deepseek-v4.1-flash-sg","label":"deepseek",
            "description":"","effortLevels":["none","low","high","max"],
        })];
        let config = HashMap::from([("grok-4.6".to_string(), crate::agent::grok_models::ConfigModel {
            label: "Grok 4.6".into(), effort_levels: vec!["xhigh".into(), "high".into()],
        })]);
        let models = assemble_grok_catalog(&live, &["grok-4.6".to_string()], &config);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["id"], "magpie/workbuddy-ai/deepseek-v4.1-flash-sg");
        assert_eq!(models[0]["effortLevels"], serde_json::json!(["none", "low", "high", "max"]));
        assert_eq!(models[1]["id"], "grok-4.6");
        assert_eq!(models[1]["label"], "Grok 4.6");
        assert_eq!(models[1]["effortLevels"], serde_json::json!(["xhigh", "high"]));
    }

    /// Before Grok has ever opened a conversation, the config file's `reasoning_efforts` fill each
    /// model's ladder, which is what makes the thinking chip work on first open.
    #[test]
    fn grok_config_file_fills_the_ladder_before_the_agent_runs() {
        let config = HashMap::from([
            ("magpie/auto/deepseek-v4.1-flash".to_string(), crate::agent::grok_models::ConfigModel {
                label: "DeepSeek V4.1 Flash".into(),
                effort_levels: vec!["none".into(), "low".into(), "high".into(), "max".into()],
            }),
            ("magpie/group/free".to_string(), crate::agent::grok_models::ConfigModel {
                label: "free".into(),
                effort_levels: vec!["high".into()],
            }),
        ]);
        let ids = vec!["magpie/auto/deepseek-v4.1-flash".to_string(), "grok-4.6".to_string()];
        let models = assemble_grok_catalog(&[], &ids, &config);
        assert_eq!(models.len(), 3);
        assert_eq!(models[0]["label"], "DeepSeek V4.1 Flash");
        assert_eq!(models[0]["effortLevels"], serde_json::json!(["none", "low", "high", "max"]));
        // A model with neither a live nor a configured ladder keeps an empty list rather than inventing one.
        assert_eq!(models[1]["id"], "grok-4.6");
        assert_eq!(models[1]["effortLevels"], serde_json::json!([]));
        // Models only the config describes come last, so the CLI order is what people see first.
        assert_eq!(models[2]["id"], "magpie/group/free");
    }

    /// A live ladder wins over the config file, so switching models shows what the running session
    /// actually offers.
    #[test]
    fn live_grok_levels_win_over_config() {
        let live = vec![serde_json::json!({
            "id":"grok-4.6","label":"Grok 4.6","description":"","effortLevels":["xhigh","high","medium","low"],
        })];
        let config = HashMap::from([("grok-4.6".to_string(), crate::agent::grok_models::ConfigModel {
            label: "Config".into(), effort_levels: vec!["low".into()],
        })]);
        let models = assemble_grok_catalog(&live, &[], &config);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["label"], "Grok 4.6");
        assert_eq!(models[0]["effortLevels"], serde_json::json!(["xhigh", "high", "medium", "low"]));
    }

    fn image(bytes: usize) -> ChatImage {
        // Four base64 characters per three bytes, which is what the check reads back.
        ChatImage {
            mime_type: "image/png".into(),
            data: "A".repeat(bytes.div_ceil(3) * 4),
        }
    }

    /// The limits exist so one paste cannot be paid for by every client that later opens the
    /// conversation. What people actually paste passes; a wall of screenshots does not.
    #[test]
    fn attached_images_are_checked_against_the_limits() {
        assert!(check_images(&[]).is_ok());
        assert!(check_images(&vec![image(256 * 1024); MAX_IMAGES_PER_MESSAGE]).is_ok());
        assert!(
            check_images(&vec![image(1024); MAX_IMAGES_PER_MESSAGE + 1]).is_err(),
            "one more than the limit is one too many"
        );
        assert!(check_images(&[image(MAX_IMAGE_BYTES / 2)]).is_ok());
        assert!(check_images(&[image(MAX_IMAGE_BYTES * 2)]).is_err());
        let pdf = ChatImage {
            mime_type: "application/pdf".into(),
            data: "AAAA".into(),
        };
        assert!(check_images(&[pdf]).is_err(), "only images travel this way");
    }

    /// Builds a headless AppCtx over a fresh SQLite db inside `dir` (mirrors web::tests).
    fn headless_ctx(dir: &std::path::Path) -> crate::host::AppCtx {
        std::fs::create_dir_all(dir).unwrap();
        let db = crate::db::Db::open(&dir.join("t.db")).unwrap();
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(dir.to_path_buf(), db));
        crate::host::AppCtx::Headless(host)
    }

    /// The Grok terminal engine accepts `--session-id`, so a new terminal session seeds its own id for a
    /// deterministic first launch. The chat engine (ACP) cannot choose the id — `session/new` assigns it —
    /// so seeding one there makes the first launch try to reopen a conversation that was never created,
    /// which surfaced as "could not reopen that conversation; a new one has been started".
    #[test]
    fn a_new_grok_chat_session_does_not_seed_a_resume_anchor() {
        let dir = std::env::temp_dir().join(format!("vlx-create-seed-{}", std::process::id()));
        let ctx = headless_ctx(&dir);
        ctx.db()
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO projects (id, name, root_path, sort_order, created_at) VALUES ('p','P','/tmp',0,0)",
                [],
            )
            .unwrap();

        let create = |name: &str, engine: &str| {
            super::create_session(
                &ctx, "p", None, name, crate::models::SessionKind::Grok, None, None, None, None,
                None, None, None, None, None, None, None, Some(engine),
            )
            .unwrap()
        };

        let chat = create("grok chat", "chat");
        assert_eq!(
            chat.agent_session_id, None,
            "a new Grok chat session must not pre-seed a resume id"
        );

        let tui = create("grok tui", "tui");
        assert_eq!(
            tui.agent_session_id.as_deref(),
            Some(tui.id.as_str()),
            "a Grok terminal session seeds its own id for --session-id"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Seeding a caller-provided session id is a terminal-engine capability, independent of the specific
    /// session-creation path (`create_session` vs `insert_session`).
    #[test]
    fn caller_id_seeding_is_terminal_only_and_grok_specific() {
        use crate::models::SessionKind::*;
        assert!(Grok.accepts_caller_session_id("tui"));
        assert!(!Grok.accepts_caller_session_id("chat"));
        assert!(Zoo.accepts_caller_session_id("tui"));
        assert!(!Claude.accepts_caller_session_id("tui"));
        assert!(!Claude.accepts_caller_session_id("chat"));
    }

    /// A preference written by one client must reach the others while they run, so the broadcast fires
    /// on every successful write — and it must carry key names only. Values would walk around the
    /// dispatch filter that hides `remoteAccess.*` and `gitea.token` from remote clients.
    #[test]
    fn set_app_settings_broadcasts_key_names_only() {
        let dir =
            std::env::temp_dir().join(format!("vlx-settings-broadcast-{}", std::process::id()));
        let ctx = headless_ctx(&dir);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        ctx.listen(crate::host::SETTINGS_CHANGED, move |payload| {
            sink.lock().unwrap().push(payload.to_string());
        });

        // An empty batch changes nothing, so it must stay quiet rather than wake every client.
        set_app_settings(&ctx, HashMap::new()).unwrap();
        assert!(
            seen.lock().unwrap().is_empty(),
            "an empty write broadcasts nothing"
        );

        set_app_settings(
            &ctx,
            HashMap::from([
                ("vlx-theme".to_string(), "dark".to_string()),
                (
                    "remoteAccess.passwordHash".to_string(),
                    "$argon2id$secret".to_string(),
                ),
            ]),
        )
        .unwrap();

        let payloads = seen.lock().unwrap().clone();
        assert_eq!(payloads.len(), 1, "one write broadcasts once");
        let keys: Vec<String> = serde_json::from_str(&payloads[0]).unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"vlx-theme".to_string()));
        assert!(keys.contains(&"remoteAccess.passwordHash".to_string()));
        assert!(
            !payloads[0].contains("dark") && !payloads[0].contains("$argon2id$secret"),
            "the payload must never carry values: {}",
            payloads[0]
        );
        let _ = std::fs::remove_dir_all(&dir);
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
            get_app_settings(&ctx)
                .unwrap()
                .get("install_id")
                .map(String::as_str),
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
        assert_eq!(
            settings.get("remoteAccess.enabled").map(String::as_str),
            Some("0")
        );
        // Port and hash stay for prefill (doc contract on web_server_stop).
        assert_eq!(
            settings.get("remoteAccess.port").map(String::as_str),
            Some("9123")
        );
        assert_eq!(
            settings
                .get("remoteAccess.passwordHash")
                .map(String::as_str),
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
        assert!(
            err.contains("LAN plaintext mode"),
            "unexpected error: {err}"
        );
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
        assert!(
            result.is_none(),
            "auto-start must skip while an instance runs"
        );
        let status = ctx.remote_web().status();
        assert!(status.running);
        assert_eq!(
            status.port,
            Some(port),
            "the manual instance must keep its port"
        );

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

    #[test]
    fn chat_clear_supports_antigravity_and_increments_name() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-cc-agy-clear-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let ctx = headless_ctx(&tmp);
        let (project_id, session_id) = {
            let conn = ctx.db().conn.lock().unwrap();
            let project = crate::db::repo::import_project(&conn, tmp.to_str().unwrap()).unwrap();
            let session = crate::db::repo::create_session_full(
                &conn,
                &project.id,
                None,
                "Antigravity 1",
                crate::models::SessionKind::Antigravity,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("chat"),
            )
            .unwrap();
            (project.id, session.id)
        };

        let fresh = super::chat_clear(&ctx, &session_id).unwrap();
        assert_eq!(fresh.kind, crate::models::SessionKind::Antigravity);
        assert_eq!(fresh.name, "Antigravity 2");
        assert_eq!(fresh.project_id, project_id);

        let conn = ctx.db().conn.lock().unwrap();
        let archived = crate::db::repo::get_session(&conn, &session_id).unwrap().unwrap();
        assert!(archived.archived_at.is_some());
        drop(conn);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn update_session_syncs_antigravity_collaboration_mode() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-cc-agy-update-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let ctx = headless_ctx(&tmp);
        let session_id = {
            let conn = ctx.db().conn.lock().unwrap();
            let project = crate::db::repo::import_project(&conn, tmp.to_str().unwrap()).unwrap();
            let session = crate::db::repo::create_session_full(
                &conn,
                &project.id,
                None,
                "Antigravity 1",
                crate::models::SessionKind::Antigravity,
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
                Some("chat"),
            )
            .unwrap();
            crate::db::repo::set_collaboration_mode(&conn, &session.id, "plan").unwrap();
            session.id
        };

        super::update_session(
            &ctx,
            &session_id,
            "Antigravity 1",
            None,
            None,
            None,
            None,
            Some("default"),
        )
        .unwrap();
        {
            let conn = ctx.db().conn.lock().unwrap();
            let updated = crate::db::repo::get_session(&conn, &session_id).unwrap().unwrap();
            assert_eq!(updated.permission_mode.as_deref(), Some("default"));
            assert_eq!(updated.collaboration_mode.as_deref(), Some("default"));
        }

        super::update_session(
            &ctx,
            &session_id,
            "Antigravity 1",
            None,
            None,
            None,
            None,
            Some("acceptEdits"),
        )
        .unwrap();
        {
            let conn = ctx.db().conn.lock().unwrap();
            let updated = crate::db::repo::get_session(&conn, &session_id).unwrap().unwrap();
            assert_eq!(updated.permission_mode.as_deref(), Some("acceptEdits"));
            assert_eq!(updated.collaboration_mode.as_deref(), Some("accept-edits"));
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }
    #[test]
    fn listen_config_from_settings() {
        use crate::web::{BindChoice, ListenConfig};
        // Absent keys keep the historical default: every interface, no pairing host.
        assert_eq!(listen_config_from(&HashMap::new(), None), Ok(ListenConfig::default()));
        let got = listen_config_from(
            &settings(&[
                ("remoteAccess.bind", "loopback"),
                ("remoteAccess.pairingHost", "vela.example.ts.net"),
            ]),
            None,
        )
        .unwrap();
        assert_eq!(got.bind, BindChoice::Loopback);
        assert_eq!(got.pairing_host.as_deref(), Some("vela.example.ts.net"));
        // Invalid stored values are errors with their code, never a fallback.
        let err = listen_config_from(&settings(&[("remoteAccess.bind", "bogus")]), None).unwrap_err();
        assert!(err.starts_with("remote_bind_invalid:"), "{err}");
        let err = listen_config_from(&settings(&[("remoteAccess.pairingHost", "a/b")]), None).unwrap_err();
        assert!(err.starts_with("remote_pairing_host_invalid:"), "{err}");
        // The CLI override replaces the stored bind without parsing it.
        let got = listen_config_from(&settings(&[("remoteAccess.bind", "bogus")]), Some(BindChoice::All)).unwrap();
        assert_eq!(got.bind, BindChoice::All);
    }

    /// Writes of the listen keys are validated for every caller and stored canonically; an invalid value
    /// rejects the whole batch.
    #[test]
    fn set_app_settings_validates_listen_keys() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-cc-listen-keys-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let ctx = headless_ctx(&tmp);
        for (key, bad, code) in [
            ("remoteAccess.bind", "host.example", "remote_bind_invalid:"),
            ("remoteAccess.bind", "224.0.0.1", "remote_bind_invalid:"),
            ("remoteAccess.pairingHost", "evil.example/#", "remote_pairing_host_invalid:"),
        ] {
            let err = set_app_settings(
                &ctx,
                HashMap::from([
                    (key.to_string(), bad.to_string()),
                    ("vlx-theme".to_string(), "dark".to_string()),
                ]),
            )
            .unwrap_err();
            assert!(err.starts_with(code), "{key}={bad}: {err}");
            let stored = get_app_settings(&ctx).unwrap();
            assert!(stored.get(key).is_none() && stored.get("vlx-theme").is_none(), "nothing persisted");
        }
        set_app_settings(
            &ctx,
            HashMap::from([
                ("remoteAccess.bind".to_string(), " 127.0.0.1 ".to_string()),
                ("remoteAccess.pairingHost".to_string(), " vela.example.ts.net ".to_string()),
            ]),
        )
        .unwrap();
        let stored = get_app_settings(&ctx).unwrap();
        assert_eq!(stored.get("remoteAccess.bind").map(String::as_str), Some("loopback"));
        assert_eq!(
            stored.get("remoteAccess.pairingHost").map(String::as_str),
            Some("vela.example.ts.net")
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// On a stopped service a listen change persists (canonically) and shows in the status; an invalid
    /// value changes nothing; a None pairing host keeps the stored one and an empty string clears it.
    #[test]
    fn web_server_set_listen_when_stopped_persists_and_validates() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-cc-set-listen-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let ctx = headless_ctx(&tmp);
        let status = web_server_set_listen(&ctx, "loopback", Some("vela.example.ts.net")).unwrap();
        assert!(!status.running);
        assert_eq!(status.saved_bind.as_deref(), Some("loopback"));
        assert_eq!(status.saved_pairing_host.as_deref(), Some("vela.example.ts.net"));

        assert!(web_server_set_listen(&ctx, "bogus", None).unwrap_err().starts_with("remote_bind_invalid:"));
        assert!(web_server_set_listen(&ctx, "all", Some("x:1"))
            .unwrap_err()
            .starts_with("remote_pairing_host_invalid:"));
        let status = web_server_status(&ctx);
        assert_eq!(status.saved_bind.as_deref(), Some("loopback"), "invalid input changes nothing");

        // Switching the bind keeps the stored pairing host; an explicit empty string clears it.
        let status = web_server_set_listen(&ctx, "100.100.83.2", None).unwrap();
        assert_eq!(status.saved_bind.as_deref(), Some("100.100.83.2"));
        assert_eq!(status.saved_pairing_host.as_deref(), Some("vela.example.ts.net"));
        let status = web_server_set_listen(&ctx, "all", Some("")).unwrap();
        assert_eq!(status.saved_bind.as_deref(), Some("all"));
        assert_eq!(status.saved_pairing_host.as_deref(), Some(""));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// An invalid persisted listen address makes auto-start fail visibly: an error, the panel's
    /// autostart_error, and no running server (never a silent skip or a fallback to every interface).
    #[test]
    fn autostart_with_invalid_or_absent_bind_fails_visibly() {
        for (raw, code) in [
            ("bogus", "remote_bind_invalid:bogus"),
            ("192.0.2.10", "remote_bind_unavailable:192.0.2.10"),
        ] {
            let tmp = std::env::temp_dir().join(format!(
                "vlx-cc-autostart-bind-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4().simple()
            ));
            let ctx = headless_ctx(&tmp);
            seed_remote_settings(&ctx, "0");
            // Bypass write validation to simulate a corrupt or foreign database value.
            {
                let conn = ctx.db().conn.lock().unwrap();
                crate::db::repo::set_app_settings(
                    &conn,
                    &HashMap::from([("remoteAccess.bind".to_string(), raw.to_string())]),
                )
                .unwrap();
            }
            let Err(err) = web_server_autostart(&ctx) else {
                panic!("auto-start must fail for bind={raw}");
            };
            assert_eq!(err, code);
            assert!(!ctx.remote_web().status().running, "no server may bind");
            assert_eq!(web_server_status(&ctx).autostart_error.as_deref(), Some(code));
            let _ = std::fs::remove_dir_all(&tmp);
        }
    }

    /// The desktop start path reads the persisted listen address: an absent address fails the start with
    /// its code and binds nothing (no fallback to every interface), and nothing is persisted as enabled.
    #[test]
    fn web_server_start_honours_persisted_bind_without_fallback() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-cc-start-bind-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let ctx = headless_ctx(&tmp);
        web_server_set_listen(&ctx, "192.0.2.10", None).unwrap();
        let err = super::web_server_start(&ctx, "pw", Some(9), false).unwrap_err();
        assert_eq!(err, "remote_bind_unavailable:192.0.2.10");
        assert!(!ctx.remote_web().status().running, "no server may bind");
        assert_ne!(
            get_app_settings(&ctx).unwrap().get("remoteAccess.enabled").map(String::as_str),
            Some("1"),
            "a failed start must not persist the enabled flag"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// A listen change on a running desktop instance (loopback only, so no network address is bound)
    /// restarts it on the same port with the stored verifier, keeps the pairing token, moves the advertised
    /// host, and persists the new setting; an absent address fails, keeps the instance running, and leaves
    /// the persisted setting unchanged.
    #[test]
    fn web_server_set_listen_restarts_running_instance_and_persists_only_on_success() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-cc-set-listen-running-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let ctx = headless_ctx(&tmp);
        web_server_set_listen(&ctx, "loopback", Some("")).unwrap();

        let mut started = Err("never attempted".to_string());
        let mut port = 0;
        for _ in 0..5 {
            port = std::net::TcpListener::bind(("127.0.0.1", 0))
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            started = super::web_server_start(&ctx, "pw", Some(port), false);
            if started.is_ok() {
                break;
            }
        }
        let status = started.expect("failed to start the loopback LanTls server after retries");
        assert_eq!(status.bind.as_deref(), Some("loopback"));
        assert_eq!(status.urls, [format!("https://127.0.0.1:{port}")]);
        // Start returns the merged view like status and set_listen, saved values included, so the panel
        // compares against what is actually stored.
        assert_eq!(status.saved_bind.as_deref(), Some("loopback"), "start returns the saved bind");
        assert_eq!(status.saved_pairing_host.as_deref(), Some(""), "start returns the saved pairing host");
        assert!(status.auto_start, "start returns the persisted enabled flag");
        let token = ctx.remote_web().create_pairing(None, false).unwrap().device_token;

        let status = web_server_set_listen(&ctx, "loopback", Some("vela.example.ts.net")).unwrap();
        assert!(status.running);
        assert_eq!(status.port, Some(port), "the port is kept");
        assert_eq!(status.urls, [format!("https://vela.example.ts.net:{port}")], "the listener was restarted");
        assert_eq!(status.saved_pairing_host.as_deref(), Some("vela.example.ts.net"));
        assert_eq!(
            ctx.remote_web().create_pairing(None, false).unwrap().device_token,
            token,
            "pairing credentials survive the restart"
        );

        let err = web_server_set_listen(&ctx, "192.0.2.10", None).unwrap_err();
        assert_eq!(err, "remote_bind_unavailable:192.0.2.10");
        let status = web_server_status(&ctx);
        assert!(status.running, "the previous instance keeps running");
        assert_eq!(status.bind.as_deref(), Some("loopback"));
        assert_eq!(status.saved_bind.as_deref(), Some("loopback"), "a failed change is not persisted");

        ctx.remote_web().stop();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn chat_attach_antigravity_does_not_deadlock_db() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-cc-chat-attach-agy-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let ctx = headless_ctx(&tmp);
        let project = super::create_virtual_project(&ctx, "P", None).unwrap();
        let session = super::create_session(
            &ctx,
            &project.id,
            None,
            "Antigravity",
            crate::models::SessionKind::Antigravity,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("/nonexistent/agy"),
            Some("chat"),
        )
        .unwrap();

        super::chat_attach(&ctx, &session.id).unwrap();

        // Subsequent DB-backed commands must keep working without blocking.
        let models = super::chat_models(&ctx, &session.id).unwrap();
        assert!(models.as_array().is_some_and(|items| !items.is_empty()));
        super::chat_set_model(&ctx, &session.id, Some("gemini-4-neon")).unwrap();
        let term = super::create_session(
            &ctx,
            &project.id,
            None,
            "Terminal",
            crate::models::SessionKind::Terminal,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(term.kind, crate::models::SessionKind::Terminal);

        let _ = std::fs::remove_dir_all(&tmp);
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
