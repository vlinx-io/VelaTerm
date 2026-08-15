//! Data-access layer for project import, group/session CRUD, and tree queries.
//! Every function accepts `&Connection`; callers hold the mutex lock.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use crate::models::{Group, NodeKind, Project, Session, SessionKind, Tree};

/// Converts Windows verbatim paths returned by `std::fs::canonicalize` into the ordinary form used
/// by the UI and persisted session configuration. Drive paths become `D:\dir`; verbatim UNC paths
/// become `\\server\share`. Other device namespaces (for example volume GUID paths) stay intact.
fn normalize_windows_verbatim_path(path: String) -> String {
    let Some(rest) = path.strip_prefix(r"\\?\") else {
        return path;
    };
    if let Some(unc) = rest
        .strip_prefix(r"UNC\")
        .or_else(|| rest.strip_prefix(r"unc\"))
    {
        return format!(r"\\{unc}");
    }
    let bytes = rest.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
    {
        return rest.to_string();
    }
    path
}

fn normalize_optional_windows_verbatim_path(path: Option<String>) -> Option<String> {
    path.map(normalize_windows_verbatim_path)
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Use millisecond timestamps as initial sort_order so new nodes append monotonically.
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn map_project(row: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: normalize_windows_verbatim_path(row.get(2)?),
        color: row.get(3)?,
        sort_order: row.get(4)?,
        collapsed: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        // Append the emoji marker at index 7 without shifting existing fields.
        mark: row.get(7)?,
    })
}

fn map_group(row: &Row) -> rusqlite::Result<Group> {
    Ok(Group {
        id: row.get(0)?,
        project_id: row.get(1)?,
        parent_group_id: row.get(2)?,
        name: row.get(3)?,
        sort_order: row.get(4)?,
        collapsed: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        // Append group worktree columns at indices 7/8 without shifting existing fields.
        worktree_path: normalize_optional_windows_verbatim_path(row.get(7)?),
        worktree_base_ref: row.get(8)?,
        // Append the emoji marker at index 9.
        mark: row.get(9)?,
    })
}

fn map_session(row: &Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get(0)?,
        project_id: row.get(1)?,
        group_id: row.get(2)?,
        name: row.get(3)?,
        shell: row.get(4)?,
        cwd: normalize_optional_windows_verbatim_path(row.get(5)?),
        env_json: row.get(6)?,
        init_cmd: row.get(7)?,
        hotkey: row.get(8)?,
        sort_order: row.get(9)?,
        created_at: row.get(10)?,
        // Append kind at SELECT index 11 without disturbing older indices.
        kind: SessionKind::from_db(&row.get::<_, String>(11)?),
        // Append agent_session_id at index 12.
        agent_session_id: row.get(12)?,
        // Append three session-hierarchy columns at indices 13/14/15.
        parent_session_id: row.get(13)?,
        collapsed: row.get::<_, i64>(14)? != 0,
        worktree_path: normalize_optional_windows_verbatim_path(row.get(15)?),
        // Append archived_at at index 16.
        archived_at: row.get(16)?,
        // Append the browser node's last URL at index 17.
        browser_url: row.get(17)?,
        // Append custom agent arguments at index 18.
        agent_args: row.get(18)?,
        // Append permission mode at index 19.
        permission_mode: row.get(19)?,
        // Append the full worktree baseline ref at index 20.
        worktree_base_ref: row.get(20)?,
        // Append the emoji marker at index 21.
        mark: row.get(21)?,
        // Append model/effort at indices 22/23.
        model: row.get(22)?,
        effort: row.get(23)?,
    })
}

/// Session-column list whose order must match `map_session`; shared by tree/archive queries.
const SESSION_COLUMNS: &str = "id, project_id, group_id, name, shell, cwd, env_json, init_cmd, \
     hotkey, sort_order, created_at, kind, agent_session_id, \
     parent_session_id, collapsed, worktree_path, archived_at, browser_url, agent_args, \
     permission_mode, worktree_base_ref, mark, model, effort";

/// Import an existing directory as a project, using its directory name.
pub fn import_project(conn: &Connection, root_path: &str) -> Result<Project, String> {
    let path = Path::new(root_path);
    if !path.is_dir() {
        return Err(format!("Directory does not exist: {root_path}"));
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve project directory {root_path}: {e}"))?;

    // Legacy roots may be relative, symlinked, or differently separated. Canonicalize each existing
    // root before comparison so the CLI switches to the project instead of importing a duplicate.
    let mut stmt = conn
        .prepare(
            "SELECT id, name, root_path, color, sort_order, collapsed, created_at, mark FROM projects",
        )
        .map_err(|e| format!("Failed to inspect existing projects: {e}"))?;
    let existing = stmt
        .query_map([], map_project)
        .map_err(|e| format!("Failed to inspect existing projects: {e}"))?;
    for row in existing {
        let project = row.map_err(|e| format!("Failed to read existing project: {e}"))?;
        let old = Path::new(&project.root_path);
        if old.canonicalize().ok().as_deref() == Some(canonical.as_path()) {
            return Ok(project);
        }
    }

    let name = canonical
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    let project = Project {
        id: new_id(),
        name,
        root_path: normalize_windows_verbatim_path(canonical.to_string_lossy().into_owned()),
        color: None,
        sort_order: now_millis(),
        collapsed: false,
        mark: None,
        created_at: now_secs(),
    };

    conn.execute(
        "INSERT INTO projects (id, name, root_path, color, sort_order, collapsed, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
        params![
            project.id,
            project.name,
            project.root_path,
            project.color,
            project.sort_order,
            project.created_at,
        ],
    )
    .map_err(|e| format!("Failed to write project: {e}"))?;

    Ok(project)
}

/// Create a group, top-level when parent_group_id is absent. Optional worktree fields establish the
/// group's default session workspace and sidebar tag.
///
/// Production uses this full signature; tests retain the four-argument wrapper with no worktree.
pub fn create_group_full(
    conn: &Connection,
    project_id: &str,
    parent_group_id: Option<&str>,
    name: &str,
    worktree_path: Option<&str>,
    worktree_base_ref: Option<&str>,
) -> Result<Group, String> {
    let group = Group {
        id: new_id(),
        project_id: project_id.to_string(),
        parent_group_id: parent_group_id.map(|s| s.to_string()),
        name: name.to_string(),
        sort_order: now_millis(),
        collapsed: false,
        created_at: now_secs(),
        worktree_path: worktree_path
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        worktree_base_ref: worktree_base_ref
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        mark: None,
    };

    conn.execute(
        "INSERT INTO groups (id, project_id, parent_group_id, name, sort_order, collapsed, worktree_path, worktree_base_ref, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)",
        params![
            group.id,
            group.project_id,
            group.parent_group_id,
            group.name,
            group.sort_order,
            group.worktree_path,
            group.worktree_base_ref,
            group.created_at,
        ],
    )
    .map_err(|e| format!("Failed to write group: {e}"))?;

    Ok(group)
}

/// Test-only compatibility wrapper that creates a group without worktree fields.
#[cfg(test)]
pub fn create_group(
    conn: &Connection,
    project_id: &str,
    parent_group_id: Option<&str>,
    name: &str,
) -> Result<Group, String> {
    create_group_full(conn, project_id, parent_group_id, name, None, None)
}

/// Create session configuration under a group or at project root.
///
/// Test-only legacy wrapper without custom agent arguments; production uses `create_session_full`.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn create_session(
    conn: &Connection,
    project_id: &str,
    group_id: Option<&str>,
    name: &str,
    kind: SessionKind,
    shell: Option<&str>,
    cwd: Option<&str>,
    init_cmd: Option<&str>,
    parent_session_id: Option<&str>,
    worktree_path: Option<&str>,
) -> Result<Session, String> {
    create_session_full(
        conn,
        project_id,
        group_id,
        name,
        kind,
        shell,
        cwd,
        init_cmd,
        parent_session_id,
        worktree_path,
        None,
        None,
        None,
        None,
        None,
    )
}

/// Create full session configuration including custom agent arguments and permission mode. Agent
/// launch splits arguments into command-line words and maps permissions to agent-specific flags.
#[allow(clippy::too_many_arguments)]
pub fn create_session_full(
    conn: &Connection,
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
    worktree_base_ref: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<Session, String> {
    let session = Session {
        id: new_id(),
        project_id: project_id.to_string(),
        group_id: group_id.map(|s| s.to_string()),
        name: name.to_string(),
        kind,
        shell: shell.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        cwd: cwd.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        env_json: None,
        init_cmd: init_cmd.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        agent_args: agent_args.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        permission_mode: permission_mode
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        hotkey: None,
        agent_session_id: None,
        parent_session_id: parent_session_id
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        collapsed: false,
        worktree_path: worktree_path
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        worktree_base_ref: worktree_base_ref
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        archived_at: None,
        browser_url: None,
        mark: None,
        model: model.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        effort: effort.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        sort_order: now_millis(),
        created_at: now_secs(),
    };

    conn.execute(
        "INSERT INTO sessions
           (id, project_id, group_id, name, kind, shell, cwd, env_json, init_cmd, hotkey, parent_session_id, collapsed, worktree_path, sort_order, created_at, agent_args, permission_mode, worktree_base_ref, model, effort)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            session.id,
            session.project_id,
            session.group_id,
            session.name,
            session.kind.as_str(),
            session.shell,
            session.cwd,
            session.env_json,
            session.init_cmd,
            session.hotkey,
            session.parent_session_id,
            session.collapsed as i64,
            session.worktree_path,
            session.sort_order,
            session.created_at,
            session.agent_args,
            session.permission_mode,
            session.worktree_base_ref,
            session.model,
            session.effort,
        ],
    )
    .map_err(|e| format!("Failed to write session: {e}"))?;

    Ok(session)
}

/// Persist an in-memory `eph-` session using its frontend-supplied ID. Preserving that PTY key turns
/// it permanent without restarting or losing context. Store configuration only; runtime state remains
/// in memory.
#[allow(clippy::too_many_arguments)]
pub fn persist_session(
    conn: &Connection,
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
    let session = Session {
        id: id.to_string(),
        project_id: project_id.to_string(),
        group_id: group_id.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        name: name.to_string(),
        kind,
        shell: shell.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        cwd: cwd.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        env_json: None,
        init_cmd: init_cmd.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        agent_args: None,
        permission_mode: None,
        hotkey: None,
        agent_session_id: None,
        parent_session_id: parent_session_id
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        collapsed: false,
        worktree_path: None,
        worktree_base_ref: None,
        archived_at: None,
        browser_url: None,
        mark: None,
        model: None,
        effort: None,
        sort_order: now_millis(),
        created_at: now_secs(),
    };

    conn.execute(
        "INSERT INTO sessions
           (id, project_id, group_id, name, kind, shell, cwd, env_json, init_cmd, hotkey, parent_session_id, collapsed, worktree_path, sort_order, created_at, agent_args, permission_mode)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            session.id,
            session.project_id,
            session.group_id,
            session.name,
            session.kind.as_str(),
            session.shell,
            session.cwd,
            session.env_json,
            session.init_cmd,
            session.hotkey,
            session.parent_session_id,
            session.collapsed as i64,
            session.worktree_path,
            session.sort_order,
            session.created_at,
            session.agent_args,
            session.permission_mode,
        ],
    )
    .map_err(|e| format!("Failed to persist session: {e}"))?;

    Ok(session)
}

/// Update session name, shell, cwd, launch command, custom agent arguments, and permission mode.
#[allow(clippy::too_many_arguments)]
pub fn update_session(
    conn: &Connection,
    id: &str,
    name: &str,
    shell: Option<&str>,
    cwd: Option<&str>,
    init_cmd: Option<&str>,
    agent_args: Option<&str>,
    permission_mode: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET name = ?1, shell = ?2, cwd = ?3, init_cmd = ?4, agent_args = ?5, permission_mode = ?6 WHERE id = ?7",
        params![name, shell, cwd, init_cmd, agent_args, permission_mode, id],
    )
    .map_err(|e| format!("Failed to update session: {e}"))?;
    Ok(())
}

/// Read custom launch arguments, returning Ok(None) when absent or unset.
pub fn get_agent_args(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT agent_args FROM sessions WHERE id = ?1",
        params![id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(|opt| opt.flatten())
    .map_err(|e| format!("Failed to read agent args: {e}"))
}

/// Model/effort pair read together at launch.
pub type ModelEffort = (Option<String>, Option<String>);

/// Read model/effort for launch translation; (None, None) when the session or values are absent.
pub fn get_model_effort(conn: &Connection, id: &str) -> Result<ModelEffort, String> {
    conn.query_row(
        "SELECT model, effort FROM sessions WHERE id = ?1",
        params![id],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        },
    )
    .optional()
    .map(|opt| opt.unwrap_or((None, None)))
    .map_err(|e| format!("Failed to read model/effort: {e}"))
}

/// Read permission mode for inject::permission_flag, returning Ok(None) when absent or unset.
pub fn get_permission_mode(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT permission_mode FROM sessions WHERE id = ?1",
        params![id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(|opt| opt.flatten())
    .map_err(|e| format!("Failed to read permission mode: {e}"))
}

/// Persist a browser node's last URL from debounced navigation events, guarded by kind.
pub fn set_browser_url(conn: &Connection, id: &str, url: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET browser_url = ?1 WHERE id = ?2 AND kind = 'browser'",
        params![url, id],
    )
    .map_err(|e| format!("Failed to update browser url: {e}"))?;
    Ok(())
}

// ─────────────────────────── Application preferences shared across shells ───────────────────────────

/// Read all app preferences as key/value pairs using frontend localStorage-compatible keys.
pub fn get_app_settings(conn: &Connection) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM app_settings")
        .map_err(|e| format!("Failed to prepare app_settings query: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Failed to query app_settings: {e}"))?;
    let mut map = HashMap::new();
    for r in rows {
        let (k, v) = r.map_err(|e| format!("Failed to read app_settings row: {e}"))?;
        map.insert(k, v);
    }
    Ok(map)
}

/// Delete an app preference. Used to remove the plaintext Gitea fallback after keyring storage succeeds.
pub fn delete_app_setting(conn: &Connection, key: &str) -> Result<(), String> {
    conn.execute("DELETE FROM app_settings WHERE key = ?1", params![key])
        .map_err(|e| format!("Failed to delete app setting {key}: {e}"))?;
    Ok(())
}

/// Batch-upsert preferences with last-write-wins and current-second updated_at.
pub fn set_app_settings(
    conn: &Connection,
    entries: &HashMap<String, String>,
) -> Result<(), String> {
    let now = now_secs();
    for (k, v) in entries {
        conn.execute(
            "INSERT INTO app_settings(key, value, updated_at) VALUES(?1, ?2, ?3) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![k, v, now],
        )
        .map_err(|e| format!("Failed to upsert app setting {k}: {e}"))?;
    }
    Ok(())
}

// ─────────────────────────── SSH connection history ───────────────────────────

/// List SSH hosts most-recent-first with target, label, timestamp, and prior shared-db choice.
/// The upper layer augments password-memory state from the keyring; this layer never accesses it.
pub fn list_ssh_hosts(
    conn: &Connection,
) -> Result<Vec<(String, Option<String>, i64, bool)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT target, label, last_connected_at, shared_db FROM ssh_hosts ORDER BY last_connected_at DESC",
        )
        .map_err(|e| format!("Failed to prepare ssh_hosts query: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, bool>(3)?,
            ))
        })
        .map_err(|e| format!("Failed to query ssh_hosts: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read ssh_hosts row: {e}"))?);
    }
    Ok(out)
}

/// Upsert a successful connection by target, updating timestamp/shared-db and setting created_at initially.
pub fn upsert_ssh_host(
    conn: &Connection,
    target: &str,
    now: i64,
    shared_db: bool,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO ssh_hosts(target, label, last_connected_at, shared_db, created_at) VALUES(?1, NULL, ?2, ?3, ?2) \
         ON CONFLICT(target) DO UPDATE SET last_connected_at = excluded.last_connected_at, shared_db = excluded.shared_db",
        params![target, now, shared_db],
    )
    .map_err(|e| format!("Failed to upsert ssh host: {e}"))?;
    Ok(())
}

/// Forget a host from history; the upper layer separately removes its keyring password.
pub fn delete_ssh_host(conn: &Connection, target: &str) -> Result<(), String> {
    conn.execute("DELETE FROM ssh_hosts WHERE target = ?1", params![target])
        .map_err(|e| format!("Failed to delete ssh host: {e}"))?;
    Ok(())
}

// ─────────────────────────── Remote URL/pairing-link history ───────────────────────────

/// List opened pairing links most-recent-first; passwords stay in the keyring, never this table.
pub fn list_url_hosts(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT url FROM url_hosts ORDER BY last_connected_at DESC")
        .map_err(|e| format!("Failed to prepare url_hosts query: {e}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query url_hosts: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read url_hosts row: {e}"))?);
    }
    Ok(out)
}

/// Upsert an opened URL, updating last_connected_at and initially setting created_at.
pub fn upsert_url_host(conn: &Connection, url: &str, now: i64) -> Result<(), String> {
    conn.execute(
        "INSERT INTO url_hosts(url, label, last_connected_at, created_at) VALUES(?1, NULL, ?2, ?2) \
         ON CONFLICT(url) DO UPDATE SET last_connected_at = excluded.last_connected_at",
        params![url, now],
    )
    .map_err(|e| format!("Failed to upsert url host: {e}"))?;
    Ok(())
}

/// Forget a pairing link from history.
pub fn delete_url_host(conn: &Connection, url: &str) -> Result<(), String> {
    conn.execute("DELETE FROM url_hosts WHERE url = ?1", params![url])
        .map_err(|e| format!("Failed to delete url host: {e}"))?;
    Ok(())
}

/// Read a host:port TOFU certificate fingerprint, or None for a new endpoint.
pub fn get_url_host_key(conn: &Connection, host_port: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT fingerprint FROM url_host_keys WHERE host_port = ?1",
        params![host_port],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Failed to query url host key: {e}"))
}

/// Store or replace a user-confirmed host:port TLS fingerprint.
pub fn upsert_url_host_key(
    conn: &Connection,
    host_port: &str,
    fingerprint: &str,
    now: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO url_host_keys(host_port, fingerprint, confirmed_at) VALUES(?1, ?2, ?3) \
         ON CONFLICT(host_port) DO UPDATE SET fingerprint = excluded.fingerprint, \
         confirmed_at = excluded.confirmed_at",
        params![host_port, fingerprint, now],
    )
    .map_err(|e| format!("Failed to upsert url host key: {e}"))?;
    Ok(())
}

/// Persist an agent's own session ID for exact automatic resume.
///
/// Guard by kind so one agent cannot update another's row. Repeated capture is idempotent.
///
/// Also clear fork_pending once a forked node captures **its own new conversation ID**, returning
/// subsequent launches to normal exact resume.
///
/// Return whether data actually changed. Skip identical writes so callers broadcast tree://changed
/// only on first capture/change rather than for every hook.
pub fn set_agent_session_id(
    conn: &Connection,
    id: &str,
    agent_session_id: &str,
    kind: SessionKind,
) -> Result<bool, String> {
    let n = conn
        .execute(
            "UPDATE sessions SET agent_session_id = ?1, fork_pending = 0
             WHERE id = ?2 AND kind = ?3
               AND (agent_session_id IS NULL OR agent_session_id <> ?1 OR fork_pending <> 0)",
            params![agent_session_id, id, kind.as_str()],
        )
        .map_err(|e| format!("Failed to store agent session id: {e}"))?;
    Ok(n > 0)
}

/// Atomically claim a newly created agent session ID for a PTY session.
///
/// Disk scanning narrows only by start time/cwd, so concurrent agents may see the same newest file.
/// One UPDATE verifies the target is unbound/pending fork and the ID is unclaimed by the same kind,
/// preventing two VelaTerm sessions from claiming one conversation.
pub fn claim_agent_session_id(
    conn: &Connection,
    id: &str,
    agent_session_id: &str,
    kind: SessionKind,
) -> Result<bool, String> {
    let n = conn
        .execute(
            "UPDATE sessions SET agent_session_id = ?1, fork_pending = 0
             WHERE id = ?2 AND kind = ?3
               AND (agent_session_id IS NULL OR agent_session_id = '' OR fork_pending <> 0)
               AND NOT EXISTS (
                 SELECT 1 FROM sessions AS owner
                  WHERE owner.id <> ?2
                    AND owner.kind = ?3
                    AND owner.agent_session_id = ?1
                    AND owner.fork_pending = 0
               )",
            params![agent_session_id, id, kind.as_str()],
        )
        .map_err(|e| format!("Failed to claim agent session id: {e}"))?;
    Ok(n > 0)
}

/// Atomically claim a newly created Codex rollout ID for a PTY session.
pub fn claim_codex_session_id(
    conn: &Connection,
    id: &str,
    agent_session_id: &str,
) -> Result<bool, String> {
    claim_agent_session_id(conn, id, agent_session_id, SessionKind::Codex)
}

/// Read fork_pending for launch argument selection; missing sessions return false.
pub fn get_fork_pending(conn: &Connection, id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT fork_pending FROM sessions WHERE id = ?1",
        params![id],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map(|opt| opt.unwrap_or(0) != 0)
    .map_err(|e| format!("Failed to read fork flag: {e}"))
}

/// Fork a **sibling** node from a source template, anchored to its conversation with fork_pending=1.
/// First launch branches from source history without modifying it; capture of the new ID clears the flag.
///
/// Available only to supported agents with an existing ID. Worktrees/hotkeys are not copied.
pub fn fork_session(conn: &Connection, source_id: &str) -> Result<Session, String> {
    let source = conn
        .query_row(
            &format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE id = ?1"),
            params![source_id],
            map_session,
        )
        .optional()
        .map_err(|e| format!("Failed to read source session: {e}"))?
        .ok_or_else(|| "Source session not found".to_string())?;

    if !matches!(
        source.kind,
        SessionKind::Claude | SessionKind::Codex | SessionKind::Pi
    ) {
        return Err("Only claude / codex / pi sessions support fork".to_string());
    }
    let anchor = source
        .agent_session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "Source session has no conversation to fork yet (never started)".to_string()
        })?
        .to_string();

    let session = Session {
        id: new_id(),
        project_id: source.project_id.clone(),
        group_id: source.group_id.clone(),
        name: format!("{} (fork)", source.name),
        kind: source.kind,
        shell: source.shell.clone(),
        cwd: source.cwd.clone(),
        env_json: source.env_json.clone(),
        init_cmd: source.init_cmd.clone(),
        // A fork is a parallel conversation in the same lineage, so copy the launch configuration.
        agent_args: source.agent_args.clone(),
        permission_mode: source.permission_mode.clone(),
        model: source.model.clone(),
        effort: source.effort.clone(),
        hotkey: None,
        agent_session_id: Some(anchor.clone()),
        parent_session_id: source.parent_session_id.clone(),
        collapsed: false,
        worktree_path: None,
        worktree_base_ref: None,
        archived_at: None,
        browser_url: None,
        // A fork continues the same line of work, so it keeps the source's marker.
        mark: source.mark.clone(),
        sort_order: now_millis(),
        created_at: now_secs(),
    };

    conn.execute(
        "INSERT INTO sessions
           (id, project_id, group_id, name, kind, shell, cwd, env_json, init_cmd, hotkey,
            agent_session_id, parent_session_id, collapsed, worktree_path,
            fork_pending, sort_order, created_at, agent_args, permission_mode, mark, model, effort)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, ?13, 1, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            session.id,
            session.project_id,
            session.group_id,
            session.name,
            session.kind.as_str(),
            session.shell,
            session.cwd,
            session.env_json,
            session.init_cmd,
            session.hotkey,
            anchor,
            session.parent_session_id,
            session.worktree_path,
            session.sort_order,
            session.created_at,
            session.agent_args,
            session.permission_mode,
            session.mark,
            session.model,
            session.effort,
        ],
    )
    .map_err(|e| format!("Failed to write forked session: {e}"))?;

    Ok(session)
}

/// Live (non-archived) descendants of a session, breadth-first, for agent-control scope checks and
/// name resolution. The root itself is not included.
pub fn live_descendants(conn: &Connection, root_id: &str) -> Result<Vec<Session>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SESSION_COLUMNS} FROM sessions \
             WHERE archived_at IS NULL AND parent_session_id IS NOT NULL"
        ))
        .map_err(|e| format!("Failed to query descendants: {e}"))?;
    let all: Vec<Session> = stmt
        .query_map([], map_session)
        .map_err(|e| format!("Failed to query descendants: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read descendants: {e}"))?;

    let mut result = Vec::new();
    let mut frontier = std::collections::VecDeque::from([root_id.to_string()]);
    let mut remaining = all;
    while let Some(parent) = frontier.pop_front() {
        let (children, rest): (Vec<_>, Vec<_>) = remaining
            .into_iter()
            .partition(|s| s.parent_session_id.as_deref() == Some(parent.as_str()));
        remaining = rest;
        for child in children {
            frontier.push_back(child.id.clone());
            result.push(child);
        }
    }
    Ok(result)
}

/// Return a session's generation and stop safely if its parent chain contains a cycle.
pub fn session_depth(conn: &Connection, id: &str) -> Result<u32, String> {
    let mut seen = std::collections::HashSet::new();
    let mut cur = id.to_string();
    let mut depth = 0u32;
    while seen.insert(cur.clone()) {
        let parent: Option<String> = conn
            .query_row(
                "SELECT parent_session_id FROM sessions WHERE id = ?1",
                params![cur],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|e| format!("Failed to read session depth: {e}"))?
            .flatten();
        match parent {
            Some(p) => {
                cur = p;
                depth += 1;
            }
            None => return Ok(depth),
        }
    }
    Err(format!("session ancestry of {id} contains a cycle"))
}

/// Read a remembered agent session ID for resume, returning Ok(None) when absent/unset.
pub fn get_agent_session_id(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT agent_session_id FROM sessions WHERE id = ?1",
        params![id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(|opt| opt.flatten())
    .map_err(|e| format!("Failed to read agent session id: {e}"))
}

/// Read a session kind so hooks persist agent IDs only into matching rows; missing returns Ok(None).
pub fn get_session_kind(conn: &Connection, id: &str) -> Result<Option<SessionKind>, String> {
    conn.query_row(
        "SELECT kind FROM sessions WHERE id = ?1",
        params![id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|opt| opt.map(|s| SessionKind::from_db(&s)))
    .map_err(|e| format!("Failed to read session kind: {e}"))
}

/// Read only a session name for automatic-placeholder checks; missing returns Ok(None).
pub fn get_session_name(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT name FROM sessions WHERE id = ?1",
        params![id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Failed to read session name: {e}"))
}

/// Return nonempty worktree paths for a session and all descendants for deletion cleanup.
pub fn worktree_paths_in_subtree(conn: &Connection, id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE descendants(id) AS (
               SELECT ?1
               UNION ALL
               SELECT s.id FROM sessions s
                 JOIN descendants d ON s.parent_session_id = d.id
             )
             SELECT worktree_path FROM sessions
             WHERE id IN (SELECT id FROM descendants)
               AND worktree_path IS NOT NULL AND worktree_path <> ''",
        )
        .map_err(|e| format!("Failed to query worktrees: {e}"))?;
    let rows = stmt
        .query_map(params![id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query worktrees: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read worktree: {e}"))?);
    }
    Ok(out)
}

/// Rename a node.
pub fn rename_node(conn: &Connection, kind: NodeKind, id: &str, name: &str) -> Result<(), String> {
    let table = match kind {
        NodeKind::Project => "projects",
        NodeKind::Group => "groups",
        NodeKind::Session => "sessions",
    };
    let sql = format!("UPDATE {table} SET name = ?1 WHERE id = ?2");
    conn.execute(&sql, params![name, id])
        .map_err(|e| format!("Failed to rename: {e}"))?;
    Ok(())
}

/// Set or clear a node's emoji marker. Passing None or an empty string clears it, so the sidebar's
/// "no marker" entry and the marker entries share one command. Any node kind is accepted because
/// projects, groups, and sessions all carry the column.
pub fn set_node_mark(
    conn: &Connection,
    kind: NodeKind,
    id: &str,
    mark: Option<&str>,
) -> Result<(), String> {
    let table = match kind {
        NodeKind::Project => "projects",
        NodeKind::Group => "groups",
        NodeKind::Session => "sessions",
    };
    let value = mark.map(str::trim).filter(|s| !s.is_empty());
    let sql = format!("UPDATE {table} SET mark = ?1 WHERE id = ?2");
    conn.execute(&sql, params![value, id])
        .map_err(|e| format!("Failed to update mark: {e}"))?;
    Ok(())
}

/// Clear a node's worktree binding after its directory is removed, converting it to an ordinary
/// session/group. Sessions also clear cwd so launch falls back to project root. Projects are ignored.
pub fn clear_node_worktree(conn: &Connection, kind: NodeKind, id: &str) -> Result<(), String> {
    let sql = match kind {
        NodeKind::Session => {
            "UPDATE sessions SET worktree_path = NULL, worktree_base_ref = NULL, cwd = NULL WHERE id = ?1"
        }
        NodeKind::Group => {
            "UPDATE groups SET worktree_path = NULL, worktree_base_ref = NULL WHERE id = ?1"
        }
        NodeKind::Project => return Ok(()),
    };
    conn.execute(sql, params![id])
        .map_err(|e| format!("Failed to clear worktree binding: {e}"))?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AgentLanding {
    pub session_id: String,
    pub parent_session_id: String,
    pub source_branch: String,
    pub source_head: String,
    pub source_tree: String,
    pub diff_fingerprint: String,
    pub target_branch: String,
    pub target_before: String,
    pub result_tree: Option<String>,
    pub target_commit: Option<String>,
    pub commit_message: String,
}

pub fn get_agent_landing(conn: &Connection, session_id: &str) -> Result<Option<AgentLanding>, String> {
    conn.query_row(
        "SELECT session_id, parent_session_id, source_branch, source_head, source_tree, \
                diff_fingerprint, target_branch, target_before, result_tree, target_commit, \
                commit_message FROM agent_landings WHERE session_id = ?1",
        params![session_id],
        |row| {
            Ok(AgentLanding {
                session_id: row.get(0)?,
                parent_session_id: row.get(1)?,
                source_branch: row.get(2)?,
                source_head: row.get(3)?,
                source_tree: row.get(4)?,
                diff_fingerprint: row.get(5)?,
                target_branch: row.get(6)?,
                target_before: row.get(7)?,
                result_tree: row.get(8)?,
                target_commit: row.get(9)?,
                commit_message: row.get(10)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("Failed to read agent landing: {e}"))
}

pub fn begin_agent_landing(conn: &Connection, landing: &AgentLanding) -> Result<(), String> {
    conn.execute(
        "INSERT INTO agent_landings (
           session_id, parent_session_id, source_branch, source_head, source_tree,
           diff_fingerprint, target_branch, target_before, result_tree, target_commit,
           commit_message, landed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, NULL)
         ON CONFLICT(session_id) DO UPDATE SET
           parent_session_id = excluded.parent_session_id,
           source_branch = excluded.source_branch,
           source_head = excluded.source_head,
           source_tree = excluded.source_tree,
           diff_fingerprint = excluded.diff_fingerprint,
           target_branch = excluded.target_branch,
           target_before = excluded.target_before,
           result_tree = NULL,
           target_commit = NULL,
           commit_message = excluded.commit_message,
           landed_at = NULL",
        params![
            landing.session_id,
            landing.parent_session_id,
            landing.source_branch,
            landing.source_head,
            landing.source_tree,
            landing.diff_fingerprint,
            landing.target_branch,
            landing.target_before,
            landing.commit_message,
        ],
    )
    .map_err(|e| format!("Failed to record pending agent landing: {e}"))?;
    Ok(())
}

pub fn set_agent_landing_result_tree(
    conn: &Connection,
    session_id: &str,
    result_tree: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE agent_landings SET result_tree = ?1 WHERE session_id = ?2",
        params![result_tree, session_id],
    )
    .map_err(|e| format!("Failed to record agent landing tree: {e}"))?;
    Ok(())
}

pub fn complete_agent_landing(
    conn: &Connection,
    session_id: &str,
    target_commit: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE agent_landings SET target_commit = ?1, landed_at = ?2 WHERE session_id = ?3",
        params![target_commit, now_secs(), session_id],
    )
    .map_err(|e| format!("Failed to complete agent landing record: {e}"))?;
    Ok(())
}

pub fn delete_agent_landing(conn: &Connection, session_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM agent_landings WHERE session_id = ?1",
        params![session_id],
    )
    .map_err(|e| format!("Failed to clear agent landing record: {e}"))?;
    Ok(())
}

/// Minimal session row loaded for archive-preserving deletion decisions.
#[derive(Clone)]
struct SessRow {
    id: String,
    parent: Option<String>,
    group_id: Option<String>,
    archived: bool,
}

fn map_sess_row(row: &Row) -> rusqlite::Result<SessRow> {
    Ok(SessRow {
        id: row.get(0)?,
        parent: row.get(1)?,
        group_id: row.get(2)?,
        archived: row.get::<_, Option<i64>>(3)?.is_some(),
    })
}

/// Delete a node while **preserving archived sessions**:
/// - Groups/projects containing archives become hidden deleted_at tombstones; live sessions are deleted,
///   while archives restore their containers later.
/// - Deleting a live parent detaches archived children to its nearest live group, then removes the parent.
/// - Deleting an archived session from the archive panel physically removes its subtree and empty tombstones.
///
/// Return only physically deleted session IDs for recording/index cleanup; preserved archives are excluded.
pub fn delete_node(conn: &Connection, kind: NodeKind, id: &str) -> Result<Vec<String>, String> {
    match kind {
        NodeKind::Session => delete_session_node(conn, id),
        NodeKind::Group => delete_group_node(conn, id),
        NodeKind::Project => delete_project_node(conn, id),
    }
}

/// Physically delete sessions by generated IN placeholders; empty input is a no-op.
fn delete_sessions_by_id(conn: &Connection, ids: &[String]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!("DELETE FROM sessions WHERE id IN ({placeholders})");
    let p: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    conn.execute(&sql, p.as_slice())
        .map_err(|e| format!("Failed to delete sessions: {e}"))?;
    Ok(())
}

/// Whether a session is archived; missing counts as false.
fn is_session_archived(conn: &Connection, id: &str) -> Result<bool, String> {
    let v: Option<Option<i64>> = conn
        .query_row(
            "SELECT archived_at FROM sessions WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to read archived_at: {e}"))?;
    Ok(matches!(v, Some(Some(_))))
}

/// Return a session's project/group IDs, or None when missing.
fn session_project_and_group(
    conn: &Connection,
    id: &str,
) -> Result<Option<(String, Option<String>)>, String> {
    conn.query_row(
        "SELECT project_id, group_id FROM sessions WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(|e| format!("Failed to read session container: {e}"))
}

/// Follow parent_session_id to the top ancestor and return its group/project, used to rehome archived
/// children when deleting a live parent.
fn top_session_group_project(
    conn: &Connection,
    id: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let mut cur = id.to_string();
    // Defensive bound prevents infinite loops on corrupt cycles.
    for _ in 0..10_000 {
        let (parent, group, project): (Option<String>, Option<String>, String) = conn
            .query_row(
                "SELECT parent_session_id, group_id, project_id FROM sessions WHERE id = ?1",
                params![cur],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|e| format!("Failed to walk session ancestry: {e}"))?;
        match parent {
            Some(p) => cur = p,
            None => return Ok((group, Some(project))),
        }
    }
    Ok((None, None))
}

/// Group IDs in a group's subtree, including itself.
fn descendant_group_ids(conn: &Connection, group_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE gsub(gid) AS (
               SELECT ?1
               UNION ALL
               SELECT g.id FROM groups g JOIN gsub ON g.parent_group_id = gsub.gid
             )
             SELECT gid FROM gsub",
        )
        .map_err(|e| format!("Failed to query group subtree: {e}"))?;
    let rows = stmt
        .query_map(params![group_id], |r| r.get::<_, String>(0))
        .map_err(|e| format!("Failed to query group subtree: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read group id: {e}"))?);
    }
    Ok(out)
}

/// All group IDs in a project, including tombstones.
fn group_ids_in_project(conn: &Connection, project_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM groups WHERE project_id = ?1")
        .map_err(|e| format!("Failed to query project groups: {e}"))?;
    let rows = stmt
        .query_map(params![project_id], |r| r.get::<_, String>(0))
        .map_err(|e| format!("Failed to query project groups: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read group id: {e}"))?);
    }
    Ok(out)
}

/// Load all sessions rooted in a group subtree, including descendants of its top-level sessions.
fn load_group_subtree_sessions(conn: &Connection, group_id: &str) -> Result<Vec<SessRow>, String> {
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE
               gsub(gid) AS (
                 SELECT ?1
                 UNION ALL
                 SELECT g.id FROM groups g JOIN gsub ON g.parent_group_id = gsub.gid
               ),
               ssub(sid) AS (
                 SELECT id FROM sessions
                   WHERE parent_session_id IS NULL AND group_id IN (SELECT gid FROM gsub)
                 UNION ALL
                 SELECT s.id FROM sessions s JOIN ssub ON s.parent_session_id = ssub.sid
               )
             SELECT id, parent_session_id, group_id, archived_at FROM sessions
               WHERE id IN (SELECT sid FROM ssub)",
        )
        .map_err(|e| format!("Failed to query group sessions: {e}"))?;
    let rows = stmt
        .query_map(params![group_id], map_sess_row)
        .map_err(|e| format!("Failed to query group sessions: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read session row: {e}"))?);
    }
    Ok(out)
}

/// Load all project sessions; project_id is present on every descendant, so one WHERE covers them.
fn load_project_sessions(conn: &Connection, project_id: &str) -> Result<Vec<SessRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, parent_session_id, group_id, archived_at FROM sessions
             WHERE project_id = ?1",
        )
        .map_err(|e| format!("Failed to query project sessions: {e}"))?;
    let rows = stmt
        .query_map(params![project_id], map_sess_row)
        .map_err(|e| format!("Failed to query project sessions: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read session row: {e}"))?);
    }
    Ok(out)
}

/// Load a session subtree including the root and every descendant.
fn load_session_subtree(conn: &Connection, id: &str) -> Result<Vec<SessRow>, String> {
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE d(id) AS (
               SELECT ?1
               UNION ALL
               SELECT s.id FROM sessions s JOIN d ON s.parent_session_id = d.id
             )
             SELECT id, parent_session_id, group_id, archived_at FROM sessions
               WHERE id IN (SELECT id FROM d)",
        )
        .map_err(|e| format!("Failed to query session subtree: {e}"))?;
    let rows = stmt
        .query_map(params![id], map_sess_row)
        .map_err(|e| format!("Failed to query session subtree: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read session row: {e}"))?);
    }
    Ok(out)
}

/// Whether a group subtree still contains archives, controlling tombstone retention/reclamation.
fn group_subtree_has_archived(conn: &Connection, group_id: &str) -> Result<bool, String> {
    Ok(load_group_subtree_sessions(conn, group_id)?
        .iter()
        .any(|s| s.archived))
}

/// Whether a project still contains archived sessions.
fn project_has_archived(conn: &Connection, project_id: &str) -> Result<bool, String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE project_id = ?1 AND archived_at IS NOT NULL",
            params![project_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("Failed to count archived: {e}"))?;
    Ok(n > 0)
}

/// Find archived roots in supplied subtree rows, detach them from soon-to-be-deleted parents, and
/// attach them to target_group (a tombstoned/deletion-nearest group or project root).
///
/// An archived root is archived while its parent is absent or not in the same archived set. Because
/// archiving marks full subtrees, any nonarchived parent is a live node being deleted.
fn rehome_archive_roots(
    conn: &Connection,
    subtree: &[SessRow],
    target_group: Option<&str>,
) -> Result<(), String> {
    let archived: HashSet<&str> = subtree
        .iter()
        .filter(|s| s.archived)
        .map(|s| s.id.as_str())
        .collect();
    for s in subtree.iter().filter(|s| s.archived) {
        let is_root = match &s.parent {
            None => true,
            Some(p) => !archived.contains(p.as_str()),
        };
        if !is_root {
            continue;
        }
        conn.execute(
            "UPDATE sessions SET parent_session_id = NULL, group_id = ?1 WHERE id = ?2",
            params![target_group, s.id],
        )
        .map_err(|e| format!("Failed to re-home archived session: {e}"))?;
    }
    Ok(())
}

/// Delete a session node according to [`delete_node`] semantics.
fn delete_session_node(conn: &Connection, id: &str) -> Result<Vec<String>, String> {
    // Deleting an archived target from the panel physically removes its subtree and empty tombstones.
    if is_session_archived(conn, id)? {
        let ids = session_ids_in_subtree(conn, id)?;
        let container = session_project_and_group(conn, id)?;
        delete_sessions_by_id(conn, &ids)?;
        if let Some((project, group)) = container {
            gc_empty_tombstones(conn, &project, group.as_deref())?;
        }
        return Ok(ids);
    }
    // Preserve archived children beneath a live target.
    let subtree = load_session_subtree(conn, id)?;
    if !subtree.iter().any(|s| s.archived) {
        // Without archives, recursively delete the session and all descendants.
        let ids: Vec<String> = subtree.iter().map(|s| s.id.clone()).collect();
        delete_sessions_by_id(conn, &ids)?;
        return Ok(ids);
    }
    // Detach archived roots to the nearest live group, then delete the remaining live subtree.
    let (target_group, _project) = top_session_group_project(conn, id)?;
    rehome_archive_roots(conn, &subtree, target_group.as_deref())?;
    let live_ids: Vec<String> = subtree
        .iter()
        .filter(|s| !s.archived)
        .map(|s| s.id.clone())
        .collect();
    delete_sessions_by_id(conn, &live_ids)?;
    Ok(live_ids)
}

/// Delete a group node according to [`delete_node`] semantics.
fn delete_group_node(conn: &Connection, group_id: &str) -> Result<Vec<String>, String> {
    let subtree = load_group_subtree_sessions(conn, group_id)?;
    if !subtree.iter().any(|s| s.archived) {
        // Without archives, rely on foreign-key cascade and return subtree IDs for recording cleanup.
        let ids: Vec<String> = subtree.iter().map(|s| s.id.clone()).collect();
        conn.execute("DELETE FROM groups WHERE id = ?1", params![group_id])
            .map_err(|e| format!("Failed to delete group: {e}"))?;
        return Ok(ids);
    }
    // With archives, tombstone groups that still contain them and hard-delete the rest.
    let ts = now_secs();
    let mut to_delete_groups: Vec<String> = Vec::new();
    for g in descendant_group_ids(conn, group_id)? {
        if group_subtree_has_archived(conn, &g)? {
            conn.execute(
                "UPDATE groups SET deleted_at = ?1 WHERE id = ?2",
                params![ts, g],
            )
            .map_err(|e| format!("Failed to tombstone group: {e}"))?;
        } else {
            to_delete_groups.push(g);
        }
    }
    // Rehome archived roots to their nearest tombstoned group; delete live sessions and empty groups.
    rehome_archive_roots_to_own_group(conn, &subtree)?;
    let live_ids: Vec<String> = subtree
        .iter()
        .filter(|s| !s.archived)
        .map(|s| s.id.clone())
        .collect();
    delete_sessions_by_id(conn, &live_ids)?;
    for g in &to_delete_groups {
        conn.execute("DELETE FROM groups WHERE id = ?1", params![g])
            .map_err(|e| format!("Failed to delete empty group: {e}"))?;
    }
    Ok(live_ids)
}

/// Delete a project node according to [`delete_node`] semantics.
fn delete_project_node(conn: &Connection, project_id: &str) -> Result<Vec<String>, String> {
    let sessions = load_project_sessions(conn, project_id)?;
    if !sessions.iter().any(|s| s.archived) {
        // Without archives, cascade-delete the whole project.
        let ids: Vec<String> = sessions.iter().map(|s| s.id.clone()).collect();
        conn.execute("DELETE FROM projects WHERE id = ?1", params![project_id])
            .map_err(|e| format!("Failed to delete project: {e}"))?;
        return Ok(ids);
    }
    // With archives, tombstone the project/containing groups, delete others, and rehome archived roots.
    let ts = now_secs();
    conn.execute(
        "UPDATE projects SET deleted_at = ?1 WHERE id = ?2",
        params![ts, project_id],
    )
    .map_err(|e| format!("Failed to tombstone project: {e}"))?;
    let mut to_delete_groups: Vec<String> = Vec::new();
    for g in group_ids_in_project(conn, project_id)? {
        if group_subtree_has_archived(conn, &g)? {
            conn.execute(
                "UPDATE groups SET deleted_at = ?1 WHERE id = ?2",
                params![ts, g],
            )
            .map_err(|e| format!("Failed to tombstone group: {e}"))?;
        } else {
            to_delete_groups.push(g);
        }
    }
    rehome_archive_roots_to_own_group(conn, &sessions)?;
    let live_ids: Vec<String> = sessions
        .iter()
        .filter(|s| !s.archived)
        .map(|s| s.id.clone())
        .collect();
    delete_sessions_by_id(conn, &live_ids)?;
    for g in &to_delete_groups {
        conn.execute("DELETE FROM groups WHERE id = ?1", params![g])
            .map_err(|e| format!("Failed to delete empty group: {e}"))?;
    }
    Ok(live_ids)
}

/// Reattach archived roots to each one's nearest original group by following its top ancestor.
fn rehome_archive_roots_to_own_group(conn: &Connection, subtree: &[SessRow]) -> Result<(), String> {
    let map: HashMap<&str, &SessRow> = subtree.iter().map(|s| (s.id.as_str(), s)).collect();
    let archived: HashSet<&str> = subtree
        .iter()
        .filter(|s| s.archived)
        .map(|s| s.id.as_str())
        .collect();
    for s in subtree.iter().filter(|s| s.archived) {
        let is_root = match &s.parent {
            None => true,
            Some(p) => !archived.contains(p.as_str()),
        };
        if !is_root {
            continue;
        }
        // Follow the in-batch parent chain to the top ancestor and use its group as destination.
        let mut cur = s.id.as_str();
        let target_group = loop {
            let row = map.get(cur).copied();
            match row {
                Some(r) => match &r.parent {
                    Some(p) if map.contains_key(p.as_str()) => cur = p.as_str(),
                    _ => break r.group_id.clone(),
                },
                None => break None,
            }
        };
        conn.execute(
            "UPDATE sessions SET parent_session_id = NULL, group_id = ?1 WHERE id = ?2",
            params![target_group, s.id],
        )
        .map_err(|e| format!("Failed to re-home archived session: {e}"))?;
    }
    Ok(())
}

/// Reclaim empty tombstone containers upward after permanent archive deletion. Remove tombstoned groups
/// with no archives, then physically delete a tombstoned archive-free project and cascade leftovers.
fn gc_empty_tombstones(
    conn: &Connection,
    project_id: &str,
    group_id: Option<&str>,
) -> Result<(), String> {
    let mut gid = group_id.map(|s| s.to_string());
    while let Some(g) = gid {
        let row: Option<(Option<i64>, Option<String>)> = conn
            .query_row(
                "SELECT deleted_at, parent_group_id FROM groups WHERE id = ?1",
                params![g],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("Failed to read group for gc: {e}"))?;
        let Some((deleted_at, parent)) = row else {
            break; // Group no longer exists.
        };
        if deleted_at.is_none() {
            break; // Live groups are not reclaimed.
        }
        if group_subtree_has_archived(conn, &g)? {
            break; // Preserve groups that still contain archives.
        }
        conn.execute("DELETE FROM groups WHERE id = ?1", params![g])
            .map_err(|e| format!("Failed to gc tombstone group: {e}"))?;
        gid = parent;
    }
    let proj_deleted: Option<Option<i64>> = conn
        .query_row(
            "SELECT deleted_at FROM projects WHERE id = ?1",
            params![project_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to read project for gc: {e}"))?;
    if matches!(proj_deleted, Some(Some(_))) && !project_has_archived(conn, project_id)? {
        conn.execute("DELETE FROM projects WHERE id = ?1", params![project_id])
            .map_err(|e| format!("Failed to gc tombstone project: {e}"))?;
    }
    Ok(())
}

/// Move a node to a new parent and sort position; projects support reordering only.
pub fn move_node(
    conn: &Connection,
    kind: NodeKind,
    id: &str,
    target_project_id: Option<&str>,
    target_group_id: Option<&str>,
    target_parent_session_id: Option<&str>,
    sort_order: i64,
) -> Result<(), String> {
    match kind {
        NodeKind::Group => {
            conn.execute(
                "UPDATE groups SET project_id = ?1, parent_group_id = ?2, sort_order = ?3 WHERE id = ?4",
                params![target_project_id, target_group_id, sort_order, id],
            )
            .map_err(|e| format!("Failed to move group: {e}"))?;
        }
        NodeKind::Session => {
            // Session parent modes are exclusive: group/project-root clears parent_session_id;
            // nesting uses the parent's group and session ID.
            conn.execute(
                "UPDATE sessions SET project_id = ?1, group_id = ?2, parent_session_id = ?3, sort_order = ?4 WHERE id = ?5",
                params![target_project_id, target_group_id, target_parent_session_id, sort_order, id],
            )
            .map_err(|e| format!("Failed to move session: {e}"))?;
        }
        NodeKind::Project => {
            conn.execute(
                "UPDATE projects SET sort_order = ?1 WHERE id = ?2",
                params![sort_order, id],
            )
            .map_err(|e| format!("Failed to move project: {e}"))?;
        }
    }
    Ok(())
}

/// Persist a project/group's expanded or collapsed state.
pub fn set_collapsed(
    conn: &Connection,
    kind: NodeKind,
    id: &str,
    collapsed: bool,
) -> Result<(), String> {
    let table = match kind {
        NodeKind::Project => "projects",
        NodeKind::Group => "groups",
        // Hierarchical sessions also have collapse state, meaningful only when they have children.
        NodeKind::Session => "sessions",
    };
    let sql = format!("UPDATE {table} SET collapsed = ?1 WHERE id = ?2");
    conn.execute(&sql, params![collapsed as i64, id])
        .map_err(|e| format!("Failed to update collapsed state: {e}"))?;
    Ok(())
}

/// Return a complete three-table tree snapshot ordered by sort_order.
pub fn list_tree(conn: &Connection) -> Result<Tree, String> {
    // Exclude hidden deleted_at tombstones retained only because they contain archived sessions.
    let projects = query_all(
        conn,
        "SELECT id, name, root_path, color, sort_order, collapsed, created_at, mark
         FROM projects WHERE deleted_at IS NULL ORDER BY sort_order",
        map_project,
    )?;
    let groups = query_all(
        conn,
        "SELECT id, project_id, parent_group_id, name, sort_order, collapsed, created_at, \
         worktree_path, worktree_base_ref, mark
         FROM groups WHERE deleted_at IS NULL ORDER BY sort_order",
        map_group,
    )?;
    // The normal tree excludes archived sessions, which appear through list_archived.
    let sessions = query_all(
        conn,
        &format!(
            "SELECT {SESSION_COLUMNS} FROM sessions WHERE archived_at IS NULL ORDER BY sort_order"
        ),
        map_session,
    )?;

    Ok(Tree {
        projects,
        groups,
        sessions,
    })
}

/// List archived roots most-recent-first for the archive browser.
///
/// Show only archived sessions whose parent is absent/nonarchived. Archiving marks full subtrees, so
/// descendants do not appear separately; restoring the root restores everything without invisible orphans.
pub fn list_archived(conn: &Connection) -> Result<Vec<Session>, String> {
    query_all(
        conn,
        &format!(
            "SELECT {SESSION_COLUMNS} FROM sessions s
             WHERE s.archived_at IS NOT NULL
               AND (s.parent_session_id IS NULL OR NOT EXISTS (
                     SELECT 1 FROM sessions p
                     WHERE p.id = s.parent_session_id AND p.archived_at IS NOT NULL))
             ORDER BY s.archived_at DESC"
        ),
        map_session,
    )
}

/// List **all** sessions, including archived descendants, for global content search.
pub fn list_all_sessions(conn: &Connection) -> Result<Vec<Session>, String> {
    query_all(
        conn,
        &format!("SELECT {SESSION_COLUMNS} FROM sessions ORDER BY sort_order"),
        map_session,
    )
}

/// Read one session by ID for post-stop index rebuild; missing returns Ok(None).
pub fn get_session(conn: &Connection, id: &str) -> Result<Option<Session>, String> {
    conn.query_row(
        &format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE id = ?1"),
        params![id],
        map_session,
    )
    .optional()
    .map_err(|e| format!("Failed to read session: {e}"))
}

/// Archive or restore an **entire session subtree** by setting or clearing archived_at.
///
/// Subtree semantics prevent children becoming invisible orphans. Only flags change; data and
/// recordings remain, allowing exact agent resume after restoration.
///
/// Restoration also revives tombstoned project/group ancestors so the session has a visible location.
/// Archived children detached from a deleted live parent restore under their nearest live group.
pub fn set_archived(conn: &Connection, id: &str, archived: bool) -> Result<(), String> {
    let ts: Option<i64> = if archived { Some(now_secs()) } else { None };
    conn.execute(
        "UPDATE sessions SET archived_at = ?1 WHERE id IN (
           WITH RECURSIVE descendants(id) AS (
             SELECT ?2
             UNION ALL
             SELECT s.id FROM sessions s
               JOIN descendants d ON s.parent_session_id = d.id
           )
           SELECT id FROM descendants
         )",
        params![ts, id],
    )
    .map_err(|e| format!("Failed to update archived state: {e}"))?;
    if !archived {
        restore_container_chain(conn, id)?;
    }
    Ok(())
}

/// Clear tombstones along a session's project/group chain; live containers are unchanged.
fn restore_container_chain(conn: &Connection, id: &str) -> Result<(), String> {
    let Some((project, group)) = session_project_and_group(conn, id)? else {
        return Ok(());
    };
    conn.execute(
        "UPDATE projects SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![project],
    )
    .map_err(|e| format!("Failed to restore project: {e}"))?;
    // Clear tombstones up the full ancestor group chain.
    let mut gid = group;
    while let Some(g) = gid {
        let parent: Option<Option<String>> = conn
            .query_row(
                "SELECT parent_group_id FROM groups WHERE id = ?1",
                params![g],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("Failed to read group parent: {e}"))?;
        let Some(parent) = parent else {
            break; // Group no longer exists.
        };
        conn.execute(
            "UPDATE groups SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL",
            params![g],
        )
        .map_err(|e| format!("Failed to restore group: {e}"))?;
        gid = parent;
    }
    Ok(())
}

/// Archive a group as a deletion-like operation: archive all subtree sessions, then delete the group.
/// Because it now contains archives, deletion turns it into a hidden tombstone. Its sessions appear as
/// archived roots; restoring any one revives the group. Parent sessions already use subtree archiving.
///
/// Empty groups contain nothing restorable and are deleted permanently.
pub fn archive_group(conn: &Connection, group_id: &str) -> Result<(), String> {
    let ids: Vec<String> = load_group_subtree_sessions(conn, group_id)?
        .into_iter()
        .map(|s| s.id)
        .collect();
    if !ids.is_empty() {
        let ts = now_secs();
        let placeholders = vec!["?"; ids.len()].join(",");
        // Preserve original archive timestamps on sessions already archived.
        let sql = format!(
            "UPDATE sessions SET archived_at = ? WHERE archived_at IS NULL AND id IN ({placeholders})"
        );
        let mut p: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(ids.len() + 1);
        p.push(&ts);
        for id in &ids {
            p.push(id as &dyn rusqlite::ToSql);
        }
        conn.execute(&sql, p.as_slice())
            .map_err(|e| format!("Failed to archive group sessions: {e}"))?;
    }
    // Delete the group into a tombstone when archives exist; an empty group is removed physically.
    delete_group_node(conn, group_id)?;
    Ok(())
}

/// Return a session and all descendant IDs for recording-file cleanup.
pub fn session_ids_in_subtree(conn: &Connection, id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE descendants(id) AS (
               SELECT ?1
               UNION ALL
               SELECT s.id FROM sessions s
                 JOIN descendants d ON s.parent_session_id = d.id
             )
             SELECT id FROM descendants",
        )
        .map_err(|e| format!("Failed to query session subtree: {e}"))?;
    let rows = stmt
        .query_map(params![id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query session subtree: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read session id: {e}"))?);
    }
    Ok(out)
}

fn query_all<T>(
    conn: &Connection,
    sql: &str,
    map: impl Fn(&Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("Failed to prepare query: {e}"))?;
    let rows = stmt
        .query_map([], |row| map(row))
        .map_err(|e| format!("Query failed: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("Failed to read row: {e}"))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::NodeKind;
    use rusqlite::Connection;

    fn mem_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("failed to open the in-memory database");
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA).unwrap();
        conn
    }

    #[test]
    fn windows_verbatim_paths_are_normalized_for_display_and_launch() {
        assert_eq!(
            normalize_windows_verbatim_path(r"\\?\D:\work\vlx-term".to_string()),
            r"D:\work\vlx-term"
        );
        assert_eq!(
            normalize_windows_verbatim_path(r"\\?\UNC\server\share\repo".to_string()),
            r"\\server\share\repo"
        );
        assert_eq!(
            normalize_windows_verbatim_path(r"D:\work\vlx-term".to_string()),
            r"D:\work\vlx-term"
        );
        assert_eq!(
            normalize_windows_verbatim_path(r"\\?\Volume{abc}\repo".to_string()),
            r"\\?\Volume{abc}\repo",
            "device paths with no ordinary Win32 equivalent must not be mangled"
        );
    }

    #[test]
    fn list_tree_normalizes_legacy_windows_paths_from_the_database() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let group = create_group(&conn, &project.id, None, "legacy").unwrap();
        let session = create_session(
            &conn,
            &project.id,
            Some(&group.id),
            "terminal",
            SessionKind::Terminal,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE projects SET root_path = ?1 WHERE id = ?2",
            params![r"\\?\D:\work\vlx-term", project.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE groups SET worktree_path = ?1 WHERE id = ?2",
            params![r"\\?\D:\work\vlx-term\.vlx-worktrees\feature", group.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE sessions SET cwd = ?1, worktree_path = ?2 WHERE id = ?3",
            params![
                r"\\?\D:\work\vlx-term",
                r"\\?\UNC\server\share\worktree",
                session.id
            ],
        )
        .unwrap();

        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.projects[0].root_path, r"D:\work\vlx-term");
        assert_eq!(
            tree.groups[0].worktree_path.as_deref(),
            Some(r"D:\work\vlx-term\.vlx-worktrees\feature")
        );
        assert_eq!(tree.sessions[0].cwd.as_deref(), Some(r"D:\work\vlx-term"));
        assert_eq!(
            tree.sessions[0].worktree_path.as_deref(),
            Some(r"\\server\share\worktree")
        );
    }

    #[test]
    fn import_project_reuses_the_same_canonical_directory() {
        let conn = mem_conn();
        let root = std::env::temp_dir().join(format!("vela-project-dedupe-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let first = import_project(&conn, root.to_str().unwrap()).unwrap();
        let second = import_project(&conn, root.to_str().unwrap()).unwrap();
        assert_eq!(first.id, second.id, "the same path should return the existing project rather than import it twice");
        assert_eq!(list_tree(&conn).unwrap().projects.len(), 1);

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let alias = root.with_extension("alias");
            let _ = std::fs::remove_file(&alias);
            symlink(&root, &alias).unwrap();
            let through_alias = import_project(&conn, alias.to_str().unwrap()).unwrap();
            assert_eq!(first.id, through_alias.id, "a symlink should resolve to the same project");
            let _ = std::fs::remove_file(alias);
        }
        let _ = std::fs::remove_dir_all(root);
    }

    /// Markers round-trip through list_tree for all three node kinds, and an empty marker clears an existing one.
    #[test]
    fn node_marks_roundtrip_and_clear() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let group = create_group(&conn, &project.id, None, "group").unwrap();
        let session = create_session(
            &conn,
            &project.id,
            Some(&group.id),
            "api",
            SessionKind::Terminal,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        // Everything starts unmarked.
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.projects[0].mark, None);
        assert_eq!(tree.groups[0].mark, None);
        assert_eq!(tree.sessions[0].mark, None);

        set_node_mark(&conn, NodeKind::Project, &project.id, Some("🔥")).unwrap();
        set_node_mark(&conn, NodeKind::Group, &group.id, Some("⭐")).unwrap();
        set_node_mark(&conn, NodeKind::Session, &session.id, Some("🐛")).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.projects[0].mark.as_deref(), Some("🔥"));
        assert_eq!(tree.groups[0].mark.as_deref(), Some("⭐"));
        assert_eq!(tree.sessions[0].mark.as_deref(), Some("🐛"));

        // Both an empty string and None mean "clear", so the menu's No Mark entry needs no separate command.
        set_node_mark(&conn, NodeKind::Session, &session.id, Some("  ")).unwrap();
        set_node_mark(&conn, NodeKind::Group, &group.id, None).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.sessions[0].mark, None);
        assert_eq!(tree.groups[0].mark, None);
        assert_eq!(tree.projects[0].mark.as_deref(), Some("🔥"));
    }

    /// Verify hierarchy creation, list_tree, and cascade deletion.
    #[test]
    fn tree_crud_and_cascade_delete() {
        let conn = mem_conn();
        let root = std::env::temp_dir();
        let root_str = root.to_str().unwrap();

        let project = import_project(&conn, root_str).expect("import failed");
        let g1 = create_group(&conn, &project.id, None, "backend services").unwrap();
        let g2 = create_group(&conn, &project.id, Some(&g1.id), "debug").unwrap();
        let s_in_g2 = create_session(
            &conn,
            &project.id,
            Some(&g2.id),
            "api",
            SessionKind::Terminal,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let _s_root = create_session(
            &conn,
            &project.id,
            None,
            "shell",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.projects.len(), 1);
        assert_eq!(tree.groups.len(), 2);
        assert_eq!(tree.sessions.len(), 2);
        // Session kinds round-trip unchanged.
        let root_session = tree.sessions.iter().find(|s| s.name == "shell").unwrap();
        assert_eq!(root_session.kind, SessionKind::Claude);
        let api_session = tree.sessions.iter().find(|s| s.name == "api").unwrap();
        assert_eq!(api_session.kind, SessionKind::Terminal);

        // Deleting g1 cascades through g2 and its session while preserving project-root sessions.
        delete_node(&conn, NodeKind::Group, &g1.id).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.groups.len(), 0, "child groups should be deleted in cascade");
        assert_eq!(tree.sessions.len(), 1, "only the session at the project root remains");
        assert!(tree.sessions.iter().all(|s| s.id != s_in_g2.id));

        // Deleting the project clears everything.
        delete_node(&conn, NodeKind::Project, &project.id).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.projects.len(), 0);
        assert_eq!(tree.sessions.len(), 0);
    }

    /// Fork creates a sibling anchored to the source ID and clears fork_pending after new-ID capture.
    #[test]
    fn fork_session_creates_sibling_with_pending_flag() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let group = create_group(&conn, &project.id, None, "group").unwrap();
        let source = create_session(
            &conn,
            &project.id,
            Some(&group.id),
            "conversation A",
            SessionKind::Claude,
            Some("/bin/zsh"),
            Some("/tmp/proj"),
            None,
            None,
            None,
        )
        .unwrap();

        // A source without a conversation ID cannot be forked.
        assert!(
            fork_session(&conn, &source.id).is_err(),
            "a fork should be refused when there is no agent_session_id"
        );

        set_agent_session_id(&conn, &source.id, "src-conv-id", SessionKind::Claude).unwrap();
        let fork = fork_session(&conn, &source.id).unwrap();

        // The sibling shares project/group/kind and shell/cwd, anchored to the source ID.
        assert_eq!(fork.project_id, project.id);
        assert_eq!(fork.group_id, Some(group.id.clone()));
        assert_eq!(fork.kind, SessionKind::Claude);
        assert_eq!(fork.shell.as_deref(), Some("/bin/zsh"));
        assert_eq!(fork.cwd.as_deref(), Some("/tmp/proj"));
        assert_eq!(fork.agent_session_id.as_deref(), Some("src-conv-id"));
        assert_eq!(fork.name, "conversation A (fork)");
        assert!(
            get_fork_pending(&conn, &fork.id).unwrap(),
            "the new node should carry the fork marker"
        );
        assert!(
            !get_fork_pending(&conn, &source.id).unwrap(),
            "the source session should not carry the marker"
        );

        // Capturing the new conversation ID clears the flag for normal exact resume.
        set_agent_session_id(&conn, &fork.id, "new-conv-id", SessionKind::Claude).unwrap();
        assert!(
            !get_fork_pending(&conn, &fork.id).unwrap(),
            "the marker should be cleared once the new id is captured"
        );
        assert_eq!(
            get_agent_session_id(&conn, &fork.id).unwrap().as_deref(),
            Some("new-conv-id")
        );
        // The source anchor remains unchanged.
        assert_eq!(
            get_agent_session_id(&conn, &source.id).unwrap().as_deref(),
            Some("src-conv-id")
        );
    }

    /// Fork guards reject terminal sessions and missing sources.
    #[test]
    fn fork_session_rejects_non_agent_kinds() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let term = create_session(
            &conn,
            &project.id,
            None,
            "t",
            SessionKind::Terminal,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(
            fork_session(&conn, &term.id).is_err(),
            "a terminal session should refuse to fork"
        );
        assert!(fork_session(&conn, "no-such-id").is_err(), "a missing source should be an error");
    }

    /// agent_session_id round-trips and kind guards prevent cross-agent writes.
    #[test]
    fn agent_session_id_roundtrip_with_kind_guard() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let claude = create_session(
            &conn,
            &project.id,
            None,
            "c",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        // Initially absent.
        assert_eq!(get_agent_session_id(&conn, &claude.id).unwrap(), None);

        // First matching-kind write round-trips and reports a change.
        assert!(set_agent_session_id(&conn, &claude.id, "sess-abc", SessionKind::Claude).unwrap());
        assert_eq!(
            get_agent_session_id(&conn, &claude.id).unwrap().as_deref(),
            Some("sess-abc")
        );
        // Rewriting the same hook-returned value reports no change and triggers no broadcast.
        assert!(!set_agent_session_id(&conn, &claude.id, "sess-abc", SessionKind::Claude).unwrap());
        // A new value reports a change.
        assert!(set_agent_session_id(&conn, &claude.id, "sess-def", SessionKind::Claude).unwrap());
        assert!(set_agent_session_id(&conn, &claude.id, "sess-abc", SessionKind::Claude).unwrap());
        // list_tree includes the field.
        let tree = list_tree(&conn).unwrap();
        let got = tree.sessions.iter().find(|s| s.id == claude.id).unwrap();
        assert_eq!(got.agent_session_id.as_deref(), Some("sess-abc"));

        // A mismatched kind does not update the row.
        set_agent_session_id(&conn, &claude.id, "wrong", SessionKind::Codex).unwrap();
        assert_eq!(
            get_agent_session_id(&conn, &claude.id).unwrap().as_deref(),
            Some("sess-abc")
        );

        // A missing session returns None without error.
        assert_eq!(get_agent_session_id(&conn, "nope").unwrap(), None);
    }

    #[test]
    fn codex_rollout_claim_is_exclusive_and_does_not_overwrite_resume_anchor() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let first = create_session(
            &conn,
            &project.id,
            None,
            "codex-a",
            SessionKind::Codex,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let second = create_session(
            &conn,
            &project.id,
            None,
            "codex-b",
            SessionKind::Codex,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(claim_codex_session_id(&conn, &first.id, "thread-a").unwrap());
        assert!(!claim_codex_session_id(&conn, &second.id, "thread-a").unwrap());
        assert_eq!(get_agent_session_id(&conn, &second.id).unwrap(), None);

        assert!(claim_codex_session_id(&conn, &second.id, "thread-b").unwrap());
        assert!(!claim_codex_session_id(&conn, &second.id, "thread-c").unwrap());
        assert_eq!(
            get_agent_session_id(&conn, &second.id).unwrap().as_deref(),
            Some("thread-b")
        );
    }

    #[test]
    fn generic_agent_session_claim_supports_pi_exclusivity() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let first = create_session(
            &conn,
            &project.id,
            None,
            "pi-a",
            SessionKind::Pi,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let second = create_session(
            &conn,
            &project.id,
            None,
            "pi-b",
            SessionKind::Pi,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(claim_agent_session_id(&conn, &first.id, "pi-thread-a", SessionKind::Pi).unwrap());
        assert!(
            !claim_agent_session_id(&conn, &second.id, "pi-thread-a", SessionKind::Pi).unwrap()
        );
        assert_eq!(get_agent_session_id(&conn, &second.id).unwrap(), None);

        assert!(claim_agent_session_id(&conn, &second.id, "pi-thread-b", SessionKind::Pi).unwrap());
        assert!(
            !claim_agent_session_id(&conn, &second.id, "pi-thread-c", SessionKind::Pi).unwrap()
        );
        assert_eq!(
            get_agent_session_id(&conn, &second.id).unwrap().as_deref(),
            Some("pi-thread-b")
        );
    }

    #[test]
    fn rename_persists() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        rename_node(&conn, NodeKind::Project, &project.id, "new name").unwrap();
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.projects[0].name, "new name");
    }

    /// Hierarchical parent/child/grandchild deletion recurses through the chain while hierarchy fields,
    /// worktree collection, and collapse state round-trip correctly.
    #[test]
    fn session_nesting_and_cascade_delete() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();

        // Top-level parent session.
        let parent = create_session(
            &conn,
            &project.id,
            None,
            "parent",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        // Child session with a worktree.
        let child = create_session(
            &conn,
            &project.id,
            None,
            "child",
            SessionKind::Claude,
            None,
            None,
            None,
            Some(&parent.id),
            Some("/tmp/wt-child"),
        )
        .unwrap();
        // Grandchild under the child.
        let grand = create_session(
            &conn,
            &project.id,
            None,
            "grandchild",
            SessionKind::Terminal,
            None,
            None,
            None,
            Some(&child.id),
            None,
        )
        .unwrap();

        // list_tree returns hierarchy and worktree fields.
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.sessions.len(), 3);
        let got_child = tree.sessions.iter().find(|s| s.id == child.id).unwrap();
        assert_eq!(
            got_child.parent_session_id.as_deref(),
            Some(parent.id.as_str())
        );
        assert_eq!(got_child.worktree_path.as_deref(), Some("/tmp/wt-child"));

        // Parent-subtree collection finds only the child's worktree.
        let wts = worktree_paths_in_subtree(&conn, &parent.id).unwrap();
        assert_eq!(wts, vec!["/tmp/wt-child".to_string()]);

        // Session collapse state is writable.
        set_collapsed(&conn, NodeKind::Session, &parent.id, true).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert!(
            tree.sessions
                .iter()
                .find(|s| s.id == parent.id)
                .unwrap()
                .collapsed
        );

        // Deleting the parent recursively removes child and grandchild.
        delete_node(&conn, NodeKind::Session, &parent.id).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.sessions.len(), 0, "deleting a parent session should recursively delete all of its descendants");
        let _ = grand; // Used only to construct the chain; deleted with the parent.
    }

    /// Archived sessions leave list_tree for list_archived and return after restoration.
    #[test]
    fn archive_hides_from_tree_and_lists_in_archived() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let s = create_session(
            &conn,
            &project.id,
            None,
            "archive me",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        // Attach a child so archive/restore operates on the whole subtree.
        let child = create_session(
            &conn,
            &project.id,
            None,
            "child",
            SessionKind::Terminal,
            None,
            None,
            None,
            Some(&s.id),
            None,
        )
        .unwrap();

        // Initially both are in the normal tree; subtree IDs include root and descendants.
        assert_eq!(list_tree(&conn).unwrap().sessions.len(), 2);
        assert!(list_archived(&conn).unwrap().is_empty());
        assert_eq!(session_ids_in_subtree(&conn, &s.id).unwrap().len(), 2);

        // Archiving the parent hides the subtree and lists only its root.
        set_archived(&conn, &s.id, true).unwrap();
        assert!(list_tree(&conn).unwrap().sessions.is_empty());
        let arch = list_archived(&conn).unwrap();
        assert_eq!(arch.len(), 1, "the archive list shows only roots");
        assert_eq!(arch[0].id, s.id);
        assert!(arch[0].archived_at.is_some());

        // Restoring the root returns the full subtree and clears archive listing.
        set_archived(&conn, &s.id, false).unwrap();
        assert_eq!(list_tree(&conn).unwrap().sessions.len(), 2);
        assert!(list_archived(&conn).unwrap().is_empty());

        // An independently archived child is itself an archived root.
        set_archived(&conn, &child.id, true).unwrap();
        assert_eq!(list_tree(&conn).unwrap().sessions.len(), 1);
        let arch = list_archived(&conn).unwrap();
        assert_eq!(arch.len(), 1);
        assert_eq!(arch[0].id, child.id);
        set_archived(&conn, &child.id, false).unwrap();

        // Deleting a project preserves contained archives and tombstones the project for restoration.
        set_archived(&conn, &child.id, true).unwrap();
        delete_node(&conn, NodeKind::Project, &project.id).unwrap();
        assert_eq!(
            list_archived(&conn).unwrap().len(),
            1,
            "deleting a project should keep the archived sessions beneath it"
        );
        assert!(
            list_tree(&conn).unwrap().projects.is_empty(),
            "a tombstoned project is hidden from the normal tree"
        );
        // Restoring the archive revives its project and normal-tree placement.
        set_archived(&conn, &child.id, false).unwrap();
        assert!(list_archived(&conn).unwrap().is_empty());
        assert_eq!(
            list_tree(&conn).unwrap().projects.len(),
            1,
            "restoring brings the project back as well"
        );
        assert_eq!(list_tree(&conn).unwrap().sessions.len(), 1);
    }

    /// Deleting a group preserves archives, removes live sessions, and tombstones the group; restoration
    /// revives the group and original placement.
    #[test]
    fn delete_group_preserves_archived_and_restore_brings_group_back() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let g = create_group(&conn, &project.id, None, "group A").unwrap();
        let arch = create_session(
            &conn,
            &project.id,
            Some(&g.id),
            "archived",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let live = create_session(
            &conn,
            &project.id,
            Some(&g.id),
            "live",
            SessionKind::Terminal,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        set_archived(&conn, &arch.id, true).unwrap();

        // Only live sessions are physically removed; the group is tombstoned and archives remain.
        let hard = delete_node(&conn, NodeKind::Group, &g.id).unwrap();
        assert_eq!(hard, vec![live.id.clone()], "only live sessions are deleted for real");
        let tree = list_tree(&conn).unwrap();
        assert!(tree.groups.is_empty(), "the group is hidden as a tombstone");
        assert!(tree.sessions.is_empty());
        let archd = list_archived(&conn).unwrap();
        assert_eq!(archd.len(), 1);
        assert_eq!(archd[0].id, arch.id);

        // Restoration revives the group and returns the session to it.
        set_archived(&conn, &arch.id, false).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.groups.len(), 1, "restoring brings the group back as well");
        assert_eq!(tree.groups[0].id, g.id);
        let s = tree.sessions.iter().find(|s| s.id == arch.id).unwrap();
        assert_eq!(s.group_id.as_deref(), Some(g.id.as_str()), "the session lands back in its original group");
        assert!(list_archived(&conn).unwrap().is_empty());
    }

    /// Deleting a live parent preserves archived children by detaching them to its nearest live group.
    #[test]
    fn delete_live_parent_rehomes_archived_subsession_to_nearest_group() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let g = create_group(&conn, &project.id, None, "group").unwrap();
        let parent = create_session(
            &conn,
            &project.id,
            Some(&g.id),
            "parent",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        // A true child uses parent_session_id rather than group_id.
        let child = create_session(
            &conn,
            &project.id,
            None,
            "child",
            SessionKind::Terminal,
            None,
            None,
            None,
            Some(&parent.id),
            None,
        )
        .unwrap();
        set_archived(&conn, &child.id, true).unwrap();

        let hard = delete_node(&conn, NodeKind::Session, &parent.id).unwrap();
        assert_eq!(hard, vec![parent.id.clone()], "only the live parent session is deleted for real");
        let archd = list_archived(&conn).unwrap();
        assert_eq!(archd.len(), 1);
        assert_eq!(archd[0].id, child.id);
        assert!(archd[0].parent_session_id.is_none(), "the archived child has been detached from its parent");
        assert_eq!(
            archd[0].group_id.as_deref(),
            Some(g.id.as_str()),
            "it is reattached to the parent's nearest group"
        );

        // Restoration returns to group g because the parent is gone.
        set_archived(&conn, &child.id, false).unwrap();
        let tree = list_tree(&conn).unwrap();
        let s = tree.sessions.iter().find(|s| s.id == child.id).unwrap();
        assert!(s.parent_session_id.is_none());
        assert_eq!(s.group_id.as_deref(), Some(g.id.as_str()));
    }

    /// Permanently deleting a tombstoned group's last archive physically reclaims the empty group.
    #[test]
    fn deleting_last_archived_gcs_tombstone_group() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let g = create_group(&conn, &project.id, None, "group").unwrap();
        let arch = create_session(
            &conn,
            &project.id,
            Some(&g.id),
            "archived",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        set_archived(&conn, &arch.id, true).unwrap();
        delete_node(&conn, NodeKind::Group, &g.id).unwrap();

        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM groups WHERE id = ?1",
                rusqlite::params![g.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnt, 1, "the tombstoned group stays in the database for now, just hidden");

        // Permanent archive deletion reclaims the tombstoned group.
        delete_node(&conn, NodeKind::Session, &arch.id).unwrap();
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM groups WHERE id = ?1",
                rusqlite::params![g.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnt, 0, "the tombstoned group is reclaimed once the last archive is deleted");
        assert!(list_archived(&conn).unwrap().is_empty());
    }

    /// For nested groups, deletion tombstones only the path to archives and hard-deletes unrelated
    /// siblings; restoration revives the chain and original inner group.
    #[test]
    fn delete_nested_group_tombstones_only_path_to_archived() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let outer = create_group(&conn, &project.id, None, "outer").unwrap();
        let inner_a = create_group(&conn, &project.id, Some(&outer.id), "inner A").unwrap();
        let inner_b = create_group(&conn, &project.id, Some(&outer.id), "inner B").unwrap();
        let arch = create_session(
            &conn,
            &project.id,
            Some(&inner_a.id),
            "archived",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let live_b = create_session(
            &conn,
            &project.id,
            Some(&inner_b.id),
            "live",
            SessionKind::Terminal,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        set_archived(&conn, &arch.id, true).unwrap();

        let hard = delete_node(&conn, NodeKind::Group, &outer.id).unwrap();
        assert_eq!(hard, vec![live_b.id.clone()], "only the live session in inner B is deleted for real");

        let count_group = |id: &str| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM groups WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(count_group(&outer.id), 1, "the outer group is kept as a tombstone");
        assert_eq!(count_group(&inner_a.id), 1, "inner A holds an archive, so it is kept as a tombstone");
        assert_eq!(count_group(&inner_b.id), 0, "inner B holds no archive and is deleted outright");
        assert!(
            list_tree(&conn).unwrap().groups.is_empty(),
            "every tombstoned group is hidden from the normal tree"
        );
        assert_eq!(list_archived(&conn).unwrap().len(), 1);

        // Restoration revives outer/inner A and returns the archive to inner A.
        set_archived(&conn, &arch.id, false).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.groups.len(), 2, "the outer group and inner A are both restored");
        let s = tree.sessions.iter().find(|s| s.id == arch.id).unwrap();
        assert_eq!(
            s.group_id.as_deref(),
            Some(inner_a.id.as_str()),
            "it returns to its original inner group"
        );
    }

    /// Archiving a group archives each session as a root and tombstones the group. Restoring one
    /// revives the group/session while the others remain archived.
    #[test]
    fn archive_group_archives_sessions_and_tombstones_group() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let g = create_group(&conn, &project.id, None, "group").unwrap();
        let s1 = create_session(
            &conn,
            &project.id,
            Some(&g.id),
            "session 1",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let s2 = create_session(
            &conn,
            &project.id,
            Some(&g.id),
            "session 2",
            SessionKind::Terminal,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        archive_group(&conn, &g.id).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert!(tree.groups.is_empty(), "the group is hidden as a tombstone");
        assert!(tree.sessions.is_empty());
        assert_eq!(
            list_archived(&conn).unwrap().len(),
            2,
            "the sessions in the group are flattened into archive roots of their own"
        );
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM groups WHERE id = ?1",
                rusqlite::params![g.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnt, 1, "the group stays in the database as a tombstone");

        // Restoring one revives the group and that session; the other remains archived.
        set_archived(&conn, &s1.id, false).unwrap();
        let tree = list_tree(&conn).unwrap();
        assert_eq!(tree.groups.len(), 1, "the group is restored");
        let r = tree.sessions.iter().find(|x| x.id == s1.id).unwrap();
        assert_eq!(r.group_id.as_deref(), Some(g.id.as_str()), "the session returns to its original group");
        assert_eq!(list_archived(&conn).unwrap().len(), 1, "the other one is still archived");
        let _ = s2;
    }

    /// Archiving an empty group becomes permanent deletion because nothing is restorable.
    #[test]
    fn archive_empty_group_deletes_it() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let g = create_group(&conn, &project.id, None, "empty group").unwrap();
        archive_group(&conn, &g.id).unwrap();
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM groups WHERE id = ?1",
                rusqlite::params![g.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnt, 0, "archiving an empty group degenerates into an outright delete");
    }

    /// Custom arguments/permission mode round-trip through full creation, reads, update, and fork;
    /// the legacy wrapper leaves both absent.
    #[test]
    fn agent_args_roundtrip_create_update_fork() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();

        // Full creation persists fields visible through tree and individual reads.
        let s = create_session_full(
            &conn,
            &project.id,
            None,
            "c",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
            Some("--model opus"),
            Some("skip"),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(s.agent_args.as_deref(), Some("--model opus"));
        assert_eq!(s.permission_mode.as_deref(), Some("skip"));
        let got = list_tree(&conn)
            .unwrap()
            .sessions
            .into_iter()
            .find(|x| x.id == s.id)
            .unwrap();
        assert_eq!(got.agent_args.as_deref(), Some("--model opus"));
        assert_eq!(got.permission_mode.as_deref(), Some("skip"));
        assert_eq!(
            get_agent_args(&conn, &s.id).unwrap().as_deref(),
            Some("--model opus")
        );
        assert_eq!(
            get_permission_mode(&conn, &s.id).unwrap().as_deref(),
            Some("skip")
        );

        // Legacy create_session leaves both fields empty.
        let t = create_session(
            &conn,
            &project.id,
            None,
            "t",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(t.agent_args, None);
        assert_eq!(t.permission_mode, None);
        assert_eq!(get_agent_args(&conn, &t.id).unwrap(), None);
        assert_eq!(get_permission_mode(&conn, &t.id).unwrap(), None);

        // Update arguments and clear permission mode; command-layer blank filtering is outside this test.
        update_session(
            &conn,
            &s.id,
            "c",
            None,
            None,
            None,
            Some("--model sonnet"),
            None,
        )
        .unwrap();
        assert_eq!(
            get_agent_args(&conn, &s.id).unwrap().as_deref(),
            Some("--model sonnet")
        );
        assert_eq!(get_permission_mode(&conn, &s.id).unwrap(), None);

        // Restore skip to verify fork copies permission mode.
        update_session(
            &conn,
            &s.id,
            "c",
            None,
            None,
            None,
            Some("--model sonnet"),
            Some("skip"),
        )
        .unwrap();

        // Fork copies source arguments and permission mode after the source receives a conversation ID.
        set_agent_session_id(&conn, &s.id, "conv-1", SessionKind::Claude).unwrap();
        let f = fork_session(&conn, &s.id).unwrap();
        assert_eq!(f.agent_args.as_deref(), Some("--model sonnet"));
        assert_eq!(f.permission_mode.as_deref(), Some("skip"));
    }

    /// live_descendants returns children and grandchildren of the root only, skipping archived
    /// sessions and unrelated siblings, in breadth-first order.
    #[test]
    fn live_descendants_scopes_and_skips_archived() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let mk = |name: &str, parent: Option<&str>| {
            create_session(
                &conn,
                &project.id,
                None,
                name,
                SessionKind::Claude,
                None,
                None,
                None,
                parent,
                None,
            )
            .unwrap()
        };
        let root = mk("root", None);
        let child_a = mk("child-a", Some(&root.id));
        let child_b = mk("child-b", Some(&root.id));
        let grandchild = mk("grandchild", Some(&child_a.id));
        let archived = mk("archived", Some(&root.id));
        let other = mk("other-root", None);
        let _outside = mk("outside", Some(&other.id));
        set_archived(&conn, &archived.id, true).unwrap();

        let got = live_descendants(&conn, &root.id).unwrap();
        let ids: Vec<&str> = got.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec![&child_a.id, &child_b.id, &grandchild.id]);
    }

    #[test]
    fn session_depth_counts_generations_and_detects_cycles() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        let mk = |name: &str, parent: Option<&str>| {
            create_session(
                &conn,
                &project.id,
                None,
                name,
                SessionKind::Claude,
                None,
                None,
                None,
                parent,
                None,
            )
            .unwrap()
        };
        let root = mk("root", None);
        let child = mk("child", Some(&root.id));
        let grandchild = mk("grandchild", Some(&child.id));

        assert_eq!(session_depth(&conn, &root.id).unwrap(), 0);
        assert_eq!(session_depth(&conn, &child.id).unwrap(), 1);
        assert_eq!(session_depth(&conn, &grandchild.id).unwrap(), 2);
        assert_eq!(session_depth(&conn, "no-such-session").unwrap(), 0);

        conn.execute(
            "UPDATE sessions SET parent_session_id = ?1 WHERE id = ?2",
            params![grandchild.id, root.id],
        )
        .unwrap();
        assert!(session_depth(&conn, &child.id).is_err());
    }

    /// Model/effort round-trip through creation, tree reads, the launch-path reader, and fork;
    /// empty strings normalize to None.
    #[test]
    fn model_effort_roundtrip_create_read_fork() {
        let conn = mem_conn();
        let project = import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();

        let s = create_session_full(
            &conn,
            &project.id,
            None,
            "c",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("fable"),
            Some("high"),
        )
        .unwrap();
        assert_eq!(s.model.as_deref(), Some("fable"));
        assert_eq!(s.effort.as_deref(), Some("high"));

        // The tree read used at startup returns the persisted values.
        let got = list_tree(&conn)
            .unwrap()
            .sessions
            .into_iter()
            .find(|x| x.id == s.id)
            .unwrap();
        assert_eq!(got.model.as_deref(), Some("fable"));
        assert_eq!(got.effort.as_deref(), Some("high"));

        // The launch path reads both values in one query.
        assert_eq!(
            get_model_effort(&conn, &s.id).unwrap(),
            (Some("fable".to_string()), Some("high".to_string()))
        );

        // Fork copies the structured settings with the rest of the launch configuration.
        set_agent_session_id(&conn, &s.id, "conv-2", SessionKind::Claude).unwrap();
        let f = fork_session(&conn, &s.id).unwrap();
        assert_eq!(f.model.as_deref(), Some("fable"));
        assert_eq!(f.effort.as_deref(), Some("high"));

        // Empty strings normalize to None; the legacy wrapper and missing sessions read as unset.
        let e = create_session_full(
            &conn,
            &project.id,
            None,
            "e",
            SessionKind::Codex,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(""),
            Some(""),
        )
        .unwrap();
        assert_eq!(e.model, None);
        assert_eq!(e.effort, None);
        assert_eq!(get_model_effort(&conn, "missing").unwrap(), (None, None));
    }
}
