//! SQLite persistence layer.

pub mod repo;
pub mod schema;

use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension};

/// Database handle injected as Tauri managed state.
pub struct Db {
    pub conn: Mutex<Connection>,
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
                eprintln!("failed to restrict database file permissions: {e}");
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
        // Create FTS5/trigram separately so an unavailable extension disables search without blocking startup.
        init_search_index(&conn);
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

/// Create the `session_fts` FTS5 virtual table. Some SQLite builds omit FTS5/trigram, so failure is logged
/// and swallowed. [`table_exists`] then reports search unavailable without preventing application startup.
fn init_search_index(conn: &Connection) {
    if let Err(e) = conn.execute_batch(schema::SESSION_FTS_DDL) {
        eprintln!(
            "[VelaTerm] Search index unavailable: failed to create FTS5 table ({e}). \
             Full-text search will be disabled. This SQLite build may lack FTS5/trigram support."
        );
    }
}

/// Whether a regular or virtual table exists in sqlite_master, used to detect session_fts availability.
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
    // Structured launch settings, separate from agent_args so orchestration never parses flag text;
    // inject::model_effort_flags translates them at launch.
    if !column_exists(conn, "sessions", "model") {
        conn.execute("ALTER TABLE sessions ADD COLUMN model TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.model: {e}"))?;
    }
    if !column_exists(conn, "sessions", "effort") {
        conn.execute("ALTER TABLE sessions ADD COLUMN effort TEXT", [])
            .map_err(|e| format!("Failed to migrate sessions.effort: {e}"))?;
    }
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
    // Create the parent index only after migration adds parent_session_id. Putting it in SCHEMA would fail
    // on old databases before ALTER runs; IF NOT EXISTS remains safe and idempotent for new databases.
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_parent ON sessions(parent_session_id)",
        [],
    )
    .map_err(|e| format!("Failed to create session parent index: {e}"))?;
    // Rebuild agent_landings without the parent_session_id foreign key: its cascade let a parent
    // deletion erase a surviving worker's landing record, the only proof the worker's branch is
    // safe to delete. SQLite cannot drop one foreign key in place, so the table is rebuilt.
    let landing_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'agent_landings'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to inspect agent_landings: {e}"))?;
    let parent_fk = landing_sql
        .as_deref()
        .is_some_and(|sql| sql.contains("parent_session_id TEXT NOT NULL REFERENCES"));
    if parent_fk {
        conn.execute_batch(
            "CREATE TABLE agent_landings_rebuilt (
               session_id       TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
               parent_session_id TEXT NOT NULL,
               source_branch    TEXT NOT NULL,
               source_head      TEXT NOT NULL,
               source_tree      TEXT NOT NULL,
               diff_fingerprint TEXT NOT NULL,
               target_branch    TEXT NOT NULL,
               target_before    TEXT NOT NULL,
               result_tree      TEXT,
               target_commit    TEXT,
               commit_message   TEXT NOT NULL,
               landed_at        INTEGER
             );
             INSERT INTO agent_landings_rebuilt SELECT * FROM agent_landings;
             DROP TABLE agent_landings;
             ALTER TABLE agent_landings_rebuilt RENAME TO agent_landings;",
        )
        .map_err(|e| format!("Failed to rebuild agent_landings: {e}"))?;
    }
    Ok(())
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

    /// An old agent_landings table cascaded on parent_session_id, so deleting a parent erased a
    /// surviving worker's landing record. Migration rebuilds the table without that foreign key
    /// and keeps every row.
    #[test]
    fn migrate_rebuilds_agent_landings_without_the_parent_cascade() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn.execute_batch(
            "CREATE TABLE agent_landings (
               session_id       TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
               parent_session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
               source_branch    TEXT NOT NULL,
               source_head      TEXT NOT NULL,
               source_tree      TEXT NOT NULL,
               diff_fingerprint TEXT NOT NULL,
               target_branch    TEXT NOT NULL,
               target_before    TEXT NOT NULL,
               result_tree      TEXT,
               target_commit    TEXT,
               commit_message   TEXT NOT NULL,
               landed_at        INTEGER
             );",
        )
        .unwrap();
        conn.execute_batch(schema::SCHEMA).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, name, root_path, sort_order, collapsed, created_at)
               VALUES ('p', 'p', '/tmp', 0, 0, 0);
             INSERT INTO sessions (id, project_id, name, sort_order, created_at)
               VALUES ('parent', 'p', 'parent', 0, 0);
             INSERT INTO sessions (id, project_id, name, sort_order, created_at)
               VALUES ('worker', 'p', 'worker', 0, 0);
             INSERT INTO agent_landings VALUES
               ('worker', 'parent', 'vlx/w', 'h', 't', 'fp', 'main', 'b', NULL, 'c', 'feat: x', 1);",
        )
        .unwrap();

        migrate(&conn).unwrap();

        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='agent_landings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            !sql.contains("parent_session_id TEXT NOT NULL REFERENCES"),
            "the parent cascade must be gone: {sql}"
        );
        conn.execute("DELETE FROM sessions WHERE id = 'parent'", [])
            .unwrap();
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM agent_landings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "the worker's landing row must survive parent deletion");

        // A repeated migration remains error-free and leaves the rebuilt table alone.
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
