//! SQLite persistence layer.

pub mod repo;
pub mod schema;
pub mod system_collections;

use crate::diagnostics::DatabaseMutex as Mutex;

use rusqlite::{Connection, OptionalExtension};

/// Database handle injected as Tauri managed state.
pub struct Db {
    pub conn: Mutex<Connection>,
    pub(crate) memory_worker: std::sync::atomic::AtomicBool,
}

impl Db {
    /// Open or create the database and initialize its schema.
    pub fn open(path: &std::path::Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("Failed to open database: {e}"))?;
        // Restrict the database to the owner: app_settings contains the remote-access Argon2id password
        // verifier (an offline-bruteforce target) and possibly a plaintext gitea.token fallback. SQLite's
        // WAL/SHM side files inherit the main database file's permissions, so 0600 here covers them too.
        // Failure is logged, never fatal: a read-only filesystem must not block startup.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
                crate::diagnostic_warn!("failed to restrict database file permissions: {e}");
            }
        }
        // Connection PRAGMAs are ordered deliberately. busy_timeout waits up to five seconds on locks so
        // multiple processes sharing a development database queue writes rather than immediately returning
        // SQLITE_BUSY. WAL allows concurrent readers and a serialized writer and requires local storage;
        // it persists in the database file. foreign_keys must be enabled per connection for cascades.
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| format!("Failed to set busy_timeout: {e}"))?;
        conn.execute_batch("PRAGMA journal_mode = WAL;\nPRAGMA foreign_keys = ON;")
            .map_err(|e| format!("Failed to set pragmas: {e}"))?;
        conn.execute_batch(schema::SCHEMA)
            .map_err(|e| format!("Failed to initialize schema: {e}"))?;
        migrate(&conn)?;
        conn.execute_batch(crate::agent::chat::submissions::SCHEMA).map_err(|e| e.to_string())?;
        conn.execute_batch(crate::agent::translation::SCHEMA).map_err(|e| e.to_string())?;
        crate::agent::translation::migrate(&conn)?;
        // The saved-message recovery feature was removed; its tables held only that feature's copies.
        conn.execute_batch("DROP TABLE IF EXISTS chat_recovery_items; DROP TABLE IF EXISTS chat_recovery_state; DROP TABLE IF EXISTS chat_recovery_meta;").map_err(|e| e.to_string())?;
        conn.execute_batch(crate::agent::chat::ownership::SCHEMA).map_err(|e| e.to_string())?;
        conn.execute_batch(crate::agent::chat::auto_continue::SCHEMA).map_err(|e| e.to_string())?;
        conn.execute_batch(crate::agent::spawn_requests::SCHEMA).map_err(|e| e.to_string())?;
        conn.execute_batch(crate::agent::plan_execute::SCHEMA).map_err(|e| e.to_string())?;
        crate::agent::plan_execute::migrate(&conn)?;
        conn.execute_batch(crate::agent::tell::SCHEMA).map_err(|e| e.to_string())?;
        crate::mobile_push::init(&conn)?;
        crate::memory::init(&conn)?;
        crate::kb::init(&conn)?;
        crate::knowledge::init(&conn)?;
        crate::security::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            memory_worker: std::sync::atomic::AtomicBool::new(false),
        })
    }
}

/// Whether a regular or virtual table exists in sqlite_master.
pub fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type IN ('table','view') AND name = ?1",
        [table],
        |_| Ok(()),
    )
    .optional()
    .map(|o| o.is_some())
    .unwrap_or(false)
}

/// Incrementally add columns missing from older databases; SCHEMA already covers new databases.
fn migrate(conn: &Connection) -> Result<(), String> {
    // Add sessions.kind to old databases with terminal as a nondestructive default.
    if !column_exists(conn, "sessions", "kind") {
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN kind TEXT NOT NULL DEFAULT 'terminal'",
            [],
        )
        .map_err(|e| format!("Failed to migrate sessions.kind: {e}"))?;
    }
    // Add nullable sessions.agent_session_id for the last native agent ID used by automatic resume.
    if !column_exists(conn, "sessions", "agent_session_id") {
        conn.execute("ALTER TABLE sessions ADD COLUMN agent_session_id TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.agent_session_id: {e}"))?;
    }
    // Add nullable parent_session_id for nested sessions. SQLite cannot enforce cascades retroactively on
    // columns added by ALTER, so repo::delete_node recursively deletes children as an application fallback.
    if !column_exists(conn, "sessions", "parent_session_id") {
        conn.execute("ALTER TABLE sessions ADD COLUMN parent_session_id TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.parent_session_id: {e}"))?;
    }
    // Add sessions.collapsed for child-session expansion, matching projects and groups.
    if !column_exists(conn, "sessions", "collapsed") {
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN collapsed INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("Failed to migrate sessions.collapsed: {e}"))?;
    }
    // Add sessions.worktree_path so deletion can offer associated Git worktree cleanup.
    if !column_exists(conn, "sessions", "worktree_path") {
        conn.execute("ALTER TABLE sessions ADD COLUMN worktree_path TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.worktree_path: {e}"))?;
    }
    // Add worktree_base_ref for the full baseline branch recorded at worktree creation. Landing and pull
    // requests target it independently of session hierarchy. Old records fall back to the primary branch.
    if !column_exists(conn, "sessions", "worktree_base_ref") {
        conn.execute("ALTER TABLE sessions ADD COLUMN worktree_base_ref TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.worktree_base_ref: {e}"))?;
    }
    // Add archived_at for reversible soft hiding and read-only playback, nullable when active.
    if !column_exists(conn, "sessions", "archived_at") {
        conn.execute("ALTER TABLE sessions ADD COLUMN archived_at INTEGER", [])
            .map_err(|e| format!("Failed to migrate sessions.archived_at: {e}"))?;
    }
    // Add fork_pending. A value of 1 means agent_session_id still references the source conversation and
    // the first launch must use agent-specific fork arguments. set_agent_session_id clears the flag after
    // capturing the new conversation ID, restoring normal resume behavior.
    if !column_exists(conn, "sessions", "fork_pending") {
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN fork_pending INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("Failed to migrate sessions.fork_pending: {e}"))?;
    }
    // Add browser_url for browser nodes' latest URL; other session types keep it null.
    if !column_exists(conn, "sessions", "browser_url") {
        conn.execute("ALTER TABLE sessions ADD COLUMN browser_url TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.browser_url: {e}"))?;
    }
    // Add nullable agent_args for user-defined launch arguments appended unchanged to agent commands.
    if !column_exists(conn, "sessions", "agent_args") {
        conn.execute("ALTER TABLE sessions ADD COLUMN agent_args TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.agent_args: {e}"))?;
    }
    // Add nullable permission_mode. Null/default uses staged approval; skip bypasses confirmations.
    // inject::permission_flag maps it to agent-specific command-line flags at launch.
    if !column_exists(conn, "sessions", "permission_mode") {
        conn.execute("ALTER TABLE sessions ADD COLUMN permission_mode TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.permission_mode: {e}"))?;
    }
    // Codex collaboration style is independent of approvals/sandboxing. Null retains the native Default
    // behavior for existing databases; only Codex chat sessions write `default` or `plan` here.
    if !column_exists(conn, "sessions", "collaboration_mode") {
        conn.execute("ALTER TABLE sessions ADD COLUMN collaboration_mode TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.collaboration_mode: {e}"))?;
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chat_codex_settings (
            session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
            service_tier TEXT,
            personality TEXT
        );",
    ).map_err(|e| format!("Failed to migrate Codex conversation settings: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chat_claude_settings (
            session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
            chrome INTEGER
        );",
    ).map_err(|e| format!("Failed to migrate Claude conversation settings: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session_model_settings (
            session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
            model TEXT,
            effort TEXT,
            native_state TEXT
        );",
    ).map_err(|e| format!("Failed to migrate session model settings: {e}"))?;
    // Add deleted_at tombstones to projects/groups. Containers holding archived sessions are hidden rather
    // than deleted so restoration can revive the hierarchy. Production SCHEMA always creates both tables;
    // table_exists only supports migration tests containing a sessions table alone.
    if table_exists(conn, "projects") && !column_exists(conn, "projects", "deleted_at") {
        conn.execute("ALTER TABLE projects ADD COLUMN deleted_at INTEGER", [])
            .map_err(|e| format!("Failed to migrate projects.deleted_at: {e}"))?;
    }
    if table_exists(conn, "groups") && !column_exists(conn, "groups", "deleted_at") {
        conn.execute("ALTER TABLE groups ADD COLUMN deleted_at INTEGER", [])
            .map_err(|e| format!("Failed to migrate groups.deleted_at: {e}"))?;
    }
    // Add nullable group worktree path and baseline ref for sidebar tags and new-session defaults.
    if table_exists(conn, "groups") && !column_exists(conn, "groups", "worktree_path") {
        conn.execute("ALTER TABLE groups ADD COLUMN worktree_path TEXT", [])
            .map_err(|e| format!("Failed to migrate groups.worktree_path: {e}"))?;
    }
    if table_exists(conn, "groups") && !column_exists(conn, "groups", "worktree_base_ref") {
        conn.execute("ALTER TABLE groups ADD COLUMN worktree_base_ref TEXT", [])
            .map_err(|e| format!("Failed to migrate groups.worktree_base_ref: {e}"))?;
    }
    // Add how an agent session is driven. Existing sessions keep the terminal engine they were created
    // with; only a session explicitly switched to the chat engine reads anything else.
    if !column_exists(conn, "sessions", "engine") {
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN engine TEXT NOT NULL DEFAULT 'tui'",
            [],
        )
        .map_err(|e| format!("Failed to migrate sessions.engine: {e}"))?;
    }
    // ZCode lost its terminal interface before any release; a development session created with it moves to the
    // conversation view, the only one it can open in. Matches nothing once done.
    conn.execute("UPDATE sessions SET engine = 'chat' WHERE kind = 'zcode' AND engine <> 'chat'", [])
        .map_err(|e| format!("Failed to migrate ZCode sessions to the conversation view: {e}"))?;
    // Inline history hints are no longer the default. Move a saved "auto" back to Tab-only completion once; the marker
    // keeps a later deliberate choice of "auto" from being undone on the next start.
    if table_exists(conn, "app_settings") {
        conn.execute_batch(
            "BEGIN;
             UPDATE app_settings SET value = 'tab', updated_at = CAST(strftime('%s','now') AS INTEGER)
               WHERE key = 'terminal-completion-mode' AND value = 'auto'
                 AND NOT EXISTS (SELECT 1 FROM app_settings WHERE key = 'terminal-completion-tab-default');
             INSERT OR IGNORE INTO app_settings(key, value, updated_at)
               VALUES('terminal-completion-tab-default', '1', CAST(strftime('%s','now') AS INTEGER));
             COMMIT;",
        )
        .map_err(|e| format!("Failed to reset the terminal completion mode: {e}"))?;
    }
    // Add the nullable emoji marker to all three node tables. Unmarked nodes keep NULL, so old databases stay
    // unchanged and the sidebar simply renders no marker for them.
    if !column_exists(conn, "sessions", "mark") {
        conn.execute("ALTER TABLE sessions ADD COLUMN mark TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.mark: {e}"))?;
    }
    if table_exists(conn, "projects") && !column_exists(conn, "projects", "mark") {
        conn.execute("ALTER TABLE projects ADD COLUMN mark TEXT", [])
            .map_err(|e| format!("Failed to migrate projects.mark: {e}"))?;
    }
    if table_exists(conn, "groups") && !column_exists(conn, "groups", "mark") {
        conn.execute("ALTER TABLE groups ADD COLUMN mark TEXT", [])
            .map_err(|e| format!("Failed to migrate groups.mark: {e}"))?;
    }
    // Add ssh_hosts.shared_db to restore the host's last remote-database choice, defaulting to independent.
    if table_exists(conn, "ssh_hosts") && !column_exists(conn, "ssh_hosts", "shared_db") {
        conn.execute(
            "ALTER TABLE ssh_hosts ADD COLUMN shared_db INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("Failed to migrate ssh_hosts.shared_db: {e}"))?;
    }
    // Add ssh_hosts.mirror to restore the host's last mirror-mode choice, defaulting to off.
    if table_exists(conn, "ssh_hosts") && !column_exists(conn, "ssh_hosts", "mirror") {
        conn.execute(
            "ALTER TABLE ssh_hosts ADD COLUMN mirror INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("Failed to migrate ssh_hosts.mirror: {e}"))?;
    }
    // Add the agent-preset link and the per-session executable path. Both are nullable: existing sessions
    // keep resolving their executable through the per-kind default in app_settings, exactly as before.
    if !column_exists(conn, "sessions", "agent_preset_id") {
        conn.execute("ALTER TABLE sessions ADD COLUMN agent_preset_id TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.agent_preset_id: {e}"))?;
    }
    if !column_exists(conn, "sessions", "agent_path") {
        conn.execute("ALTER TABLE sessions ADD COLUMN agent_path TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.agent_path: {e}"))?;
    }
    if table_exists(conn, "projects") && !column_exists(conn, "projects", "collection_id") {
        conn.execute("ALTER TABLE projects ADD COLUMN collection_id TEXT REFERENCES projects(id) ON DELETE SET NULL", [])
            .map_err(|e| format!("Failed to migrate projects.collection_id: {e}"))?;
    }
    migrate_project_folders(conn)?;
    // Create the parent index only after migration adds parent_session_id. Putting it in SCHEMA would fail
    // on old databases before ALTER runs; IF NOT EXISTS remains safe and idempotent for new databases.
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_parent ON sessions(parent_session_id)",
        [],
    )
    .map_err(|e| format!("Failed to create session parent index: {e}"))?;
    Ok(())
}

/// Convert PR #121's separate folders to ordinary collections exactly once. Existing names and IDs take
/// priority; collisions receive a stable suffix/new ID. Membership, collapse and ordering survive the conversion.
/// Dropping the obsolete column/table in the same transaction prevents a second source of container state.
fn migrate_project_folders(conn: &Connection) -> Result<(), String> {
    if !table_exists(conn, "project_folders") || !table_exists(conn, "projects") { return Ok(()); }
    let tx = conn.unchecked_transaction().map_err(|e| format!("Failed to begin folder migration: {e}"))?;
    let folders = {
        let mut stmt = tx.prepare("SELECT id, name, sort_order, collapsed, created_at FROM project_folders ORDER BY sort_order, created_at")
            .map_err(|e| format!("Failed to read legacy folders: {e}"))?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?, r.get::<_, bool>(3)?, r.get::<_, i64>(4)?)))
            .map_err(|e| format!("Failed to read legacy folders: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| format!("Failed to read legacy folder: {e}"))?
    };
    let mut names = {
        let mut stmt = tx.prepare("SELECT name FROM projects WHERE root_path = '' AND deleted_at IS NULL")
            .map_err(|e| format!("Failed to read collection names: {e}"))?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| format!("Failed to read collection names: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| format!("Failed to read collection name: {e}"))?
            .into_iter().map(|name| name.trim().to_lowercase()).collect::<std::collections::HashSet<_>>()
    };
    for (folder_id, original, order, collapsed, created) in folders {
        let base = if original.trim().is_empty() { "Collection" } else { original.trim() };
        let mut name = base.to_string();
        let mut suffix = 2;
        while !names.insert(name.to_lowercase()) { name = format!("{base} ({suffix})"); suffix += 1; }
        let conflict = tx.query_row("SELECT 1 FROM projects WHERE id = ?1", [&folder_id], |_| Ok(()))
            .optional().map_err(|e| format!("Failed to check collection identity: {e}"))?.is_some();
        let id = if conflict { uuid::Uuid::new_v4().to_string() } else { folder_id.clone() };
        tx.execute("INSERT INTO projects (id, name, root_path, sort_order, collapsed, created_at) VALUES (?1, ?2, '', ?3, ?4, ?5)",
            rusqlite::params![id, name, order, collapsed, created])
            .map_err(|e| format!("Failed to migrate folder to collection: {e}"))?;
        if column_exists(&tx, "projects", "folder_id") {
            tx.execute("UPDATE projects SET collection_id = ?1 WHERE folder_id = ?2", rusqlite::params![id, folder_id])
                .map_err(|e| format!("Failed to migrate project membership: {e}"))?;
        }
    }
    if column_exists(&tx, "projects", "folder_id") {
        tx.execute("ALTER TABLE projects DROP COLUMN folder_id", [])
            .map_err(|e| format!("Failed to remove legacy folder membership: {e}"))?;
    }
    tx.execute("DROP TABLE project_folders", []).map_err(|e| format!("Failed to remove legacy folders: {e}"))?;
    tx.commit().map_err(|e| format!("Failed to commit collection migration: {e}"))
}

/// Whether a table already has a column, based on PRAGMA table_info.
fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    let sql = format!("PRAGMA table_info({table})");
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return false;
    };
    // The second table_info field, index 1, is the column name.
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return false;
    };
    // Consume the iterator in this block so its temporary borrow cannot outlive stmt.
    for name in rows.flatten() {
        if name == column {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Simulate an early sessions table missing hierarchy/worktree columns. Migration adds them and the
    /// parent index idempotently, guarding the regression where a SCHEMA index crashed old databases.
    #[test]
    fn migrate_adds_columns_and_index_on_legacy_db() {
        let conn = Connection::open_in_memory().unwrap();
        // Early sessions table missing three columns.
        conn.execute_batch(
            "CREATE TABLE sessions (
               id TEXT PRIMARY KEY, project_id TEXT NOT NULL, group_id TEXT,
               name TEXT NOT NULL, sort_order INTEGER NOT NULL DEFAULT 0,
               created_at INTEGER NOT NULL
             );",
        )
        .unwrap();

        migrate(&conn).unwrap();

        assert!(column_exists(&conn, "sessions", "parent_session_id"));
        assert!(column_exists(&conn, "sessions", "collapsed"));
        assert!(column_exists(&conn, "sessions", "worktree_path"));
        assert!(column_exists(&conn, "sessions", "mark"));
        assert!(column_exists(&conn, "sessions", "collaboration_mode"));

        let idx: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='idx_sessions_parent'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx, 1, "the idx_sessions_parent index should have been created");

        // A repeated migration remains error-free.
        migrate(&conn).unwrap();
    }

    /// A fresh full SCHEMA also migrates idempotently, skipping existing columns and creating the index.
    #[test]
    fn migrate_is_noop_safe_on_fresh_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(schema::SCHEMA).unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        assert!(column_exists(&conn, "sessions", "parent_session_id"));
    }

    #[test]
    fn migrate_pre_folder_database_keeps_direct_collection_content() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(schema::SCHEMA).unwrap();
        let collection = repo::create_virtual_project(&conn, "Existing collection").unwrap();
        let group = repo::create_group(&conn, &collection.id, None, "Existing group").unwrap();
        let session = repo::create_session(&conn, &collection.id, Some(&group.id), "Existing session",
            crate::models::SessionKind::Terminal, None, Some("/tmp/existing"), None, None, None).unwrap();
        conn.execute("ALTER TABLE projects DROP COLUMN collection_id", []).unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        let tree = repo::list_tree(&conn).unwrap();
        assert_eq!(tree.projects[0].id, collection.id);
        assert!(tree.projects[0].collection_id.is_none());
        assert_eq!(tree.groups[0].id, group.id);
        assert_eq!(tree.sessions[0].id, session.id);
        assert_eq!(tree.sessions[0].cwd.as_deref(), Some("/tmp/existing"));
        assert!(!table_exists(&conn, "project_folders"));
    }

    /// Legacy folders become collections without changing project/session identities, including virtual projects.
    #[test]
    fn migrate_legacy_project_folders_to_collections() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(schema::SCHEMA).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;
            CREATE TABLE project_folders (id TEXT PRIMARY KEY, name TEXT NOT NULL, sort_order INTEGER, collapsed INTEGER, created_at INTEGER);
            ALTER TABLE projects ADD COLUMN folder_id TEXT REFERENCES project_folders(id) ON DELETE SET NULL;
            INSERT INTO project_folders VALUES ('f1', 'Work', 10, 1, 100), ('f2', ' work ', 20, 0, 200), ('collision', 'Notes', 30, 0, 300);
            INSERT INTO projects (id, name, root_path, created_at, folder_id) VALUES
                ('existing', 'WORK', '', 0, NULL), ('real', 'Repo', '/tmp/repo', 0, 'f1'),
                ('virtual', 'Scratch', '', 0, 'f2'), ('collision', 'Other', '/tmp/other', 0, 'collision');").unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        assert!(!table_exists(&conn, "project_folders"));
        assert!(!column_exists(&conn, "projects", "folder_id"));
        let tree = repo::list_tree(&conn).unwrap();
        let project = |id: &str| tree.projects.iter().find(|p| p.id == id).unwrap();
        assert_eq!(project("f1").name, "Work (2)");
        assert_eq!(project("f2").name, "work (3)");
        assert!(project("f1").collapsed);
        assert_eq!(project("f1").sort_order, 10);
        assert_eq!(project("real").root_path, "/tmp/repo");
        assert_eq!(project("real").collection_id.as_deref(), Some("f1"));
        assert_eq!(project("virtual").collection_id.as_deref(), Some("f2"));
        let parent = project("collision").collection_id.as_ref().unwrap();
        assert_ne!(parent, "collision");
        assert_eq!(project(parent).name, "Notes");
        assert_eq!(tree.projects.len(), 7);
    }

    /// The database file is owner-only after open: app_settings holds the remote-access password
    /// verifier (and possibly a plaintext gitea.token fallback), so group/world access is a leak.
    #[cfg(unix)]
    #[test]
    fn opened_database_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "vlx-db-perm-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        let _db = Db::open(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Cross-shell application preferences round-trip exactly and use last-write-wins updates per key.
    #[test]
    fn app_settings_roundtrip_and_upsert() {
        use std::collections::HashMap;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(schema::SCHEMA).unwrap();

        let mut a = HashMap::new();
        a.insert("vlx-theme".to_string(), "dark".to_string());
        a.insert("vlx-lang".to_string(), "zh-CN".to_string());
        repo::set_app_settings(&conn, &a).unwrap();

        let got = repo::get_app_settings(&conn).unwrap();
        assert_eq!(got.get("vlx-theme").map(String::as_str), Some("dark"));
        assert_eq!(got.get("vlx-lang").map(String::as_str), Some("zh-CN"));

        // Upsert replaces the same key without affecting others.
        let mut b = HashMap::new();
        b.insert("vlx-theme".to_string(), "light".to_string());
        repo::set_app_settings(&conn, &b).unwrap();

        let got = repo::get_app_settings(&conn).unwrap();
        assert_eq!(got.get("vlx-theme").map(String::as_str), Some("light"));
        assert_eq!(got.get("vlx-lang").map(String::as_str), Some("zh-CN"));
    }
}
