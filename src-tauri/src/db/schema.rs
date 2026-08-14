//! Database schema and initialization.

/// Table-creation statements using IF NOT EXISTS, safe to rerun as M2's simple migration strategy.
///
/// `projects.deleted_at` / `groups.deleted_at` are soft-deletion tombstones. Deleting a group/project that still
/// contains archived sessions does not remove its row; it sets this timestamp as a hidden tombstone. `list_tree`
/// excludes it with `deleted_at IS NULL`, making it appear deleted while preserving archived sessions and allowing
/// their group/project to be restored with them (see archive document §2 and retention in `repo::delete_node`).
/// The tombstone is physically removed only after its final archived session is restored or permanently deleted.
///
/// `projects.mark` / `groups.mark` / `sessions.mark` hold an optional user-chosen emoji marker shown before the node
/// name in the sidebar and usable as a sidebar filter. The value is the emoji itself rather than an enumerated code,
/// so unknown values from a newer build round-trip unchanged; NULL or an empty string means unmarked.
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  root_path   TEXT NOT NULL,
  color       TEXT,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  collapsed   INTEGER NOT NULL DEFAULT 0,
  deleted_at  INTEGER,
  mark        TEXT,
  created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS groups (
  id              TEXT PRIMARY KEY,
  project_id      TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  parent_group_id TEXT REFERENCES groups(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  sort_order      INTEGER NOT NULL DEFAULT 0,
  collapsed       INTEGER NOT NULL DEFAULT 0,
  deleted_at      INTEGER,
  worktree_path   TEXT,
  worktree_base_ref TEXT,
  mark            TEXT,
  created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
  id          TEXT PRIMARY KEY,
  project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  group_id    TEXT REFERENCES groups(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'terminal',
  shell       TEXT,
  cwd         TEXT,
  env_json    TEXT,
  init_cmd    TEXT,
  agent_args  TEXT,
  permission_mode TEXT,
  model       TEXT,
  effort      TEXT,
  hotkey      TEXT,
  agent_session_id TEXT,
  parent_session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
  collapsed   INTEGER NOT NULL DEFAULT 0,
  worktree_path TEXT,
  worktree_base_ref TEXT,
  archived_at INTEGER,
  fork_pending INTEGER NOT NULL DEFAULT 0,
  browser_url TEXT,
  mark        TEXT,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_groups_project ON groups(project_id);
CREATE INDEX IF NOT EXISTS idx_groups_parent ON groups(parent_group_id);
CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id);
CREATE INDEX IF NOT EXISTS idx_sessions_group ON sessions(group_id);

-- Application preferences shared across shells: theme, language, appearance, shortcuts, sound, and more.
-- Keys match frontend localStorage (`vlx-theme`, `vlx-lang`, `vlx-sound`, `vlx-notify`, `vlx-settings`),
-- and values store the original localStorage strings. The frontend writes both its local cache and this
-- authoritative backend table. At startup, backend values override the cache while missing keys are seeded
-- from local values; see src/ipc/settingsSync.ts. This lets Tauri and Electron share preferences through one
-- database even though each shell has independent localStorage. CREATE IF NOT EXISTS keeps old databases safe.
CREATE TABLE IF NOT EXISTS app_settings (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

-- Previously connected SSH hosts: one row per user@host[:port], providing connection history and the
-- anchor for Remember Password. Passwords never enter the database; they live only in the system keyring
-- under service `{identifier}.ssh` and account `target`. This table stores the host, last connection time,
-- and previous data-mode selection. Password presence is queried from the keyring; see ssh_remote and
-- command_core. CREATE IF NOT EXISTS is safe for old databases; migrate adds their shared_db column.
CREATE TABLE IF NOT EXISTS ssh_hosts (
  target            TEXT PRIMARY KEY,   -- user@host[:port], also used as the keyring account
  label             TEXT,               -- Reserved for a user-defined alias
  last_connected_at INTEGER NOT NULL,   -- Last successful connection time in seconds; sorted descending
  shared_db         INTEGER NOT NULL DEFAULT 0,  -- Previous "reuse remote desktop database" choice
  created_at        INTEGER NOT NULL
);

-- Previously connected URL remotes: one row per pairing link for one-click reconnection. As with SSH,
-- passwords live only in the system keyring (service `{identifier}.url`, account = link), never here.
-- Each link carries the current pairing token and becomes invalid after the peer rotates it. CREATE IF NOT
-- EXISTS keeps both new and existing databases safe.
CREATE TABLE IF NOT EXISTS url_hosts (
  url               TEXT PRIMARY KEY,   -- Pairing link, including its #pair fragment
  label             TEXT,               -- Reserved for a user-defined alias
  last_connected_at INTEGER NOT NULL,   -- Last-opened timestamp in seconds; sorted descending
  created_at        INTEGER NOT NULL
);

-- Trust anchors for confirmed URL-remote TLS fingerprints. Each row stores one host:port fingerprint for
-- trust on first use and subsequent verification. This is intentionally separate from url_hosts: pairing
-- tokens rotate, while a server identity remains anchored to stable host:port. Semantics mirror SSH
-- known_hosts: a match connects directly, no record prompts once, and a mismatch raises a warning.
-- CREATE IF NOT EXISTS is safe for existing databases and requires no migration.
CREATE TABLE IF NOT EXISTS url_host_keys (
  host_port    TEXT PRIMARY KEY,   -- "host:port" TLS endpoint and fingerprint trust anchor
  fingerprint  TEXT NOT NULL,      -- Uppercase, colon-separated SHA-256 fingerprint from probe_fingerprint
  confirmed_at INTEGER NOT NULL    -- Time in seconds when the user last confirmed this fingerprint
);

-- Freshness bookkeeping for the global search index. Each session/track records how far indexing has
-- progressed and when it last ran for incremental updates and staleness checks; see the search module.
-- init_search_index creates the FTS virtual table separately with fallback, because missing FTS5/trigram
-- support must not prevent application startup.
CREATE TABLE IF NOT EXISTS search_index_state (
  session_id    TEXT    NOT NULL,
  source        TEXT    NOT NULL,            -- 'transcript' | 'recording'
  indexed_len   INTEGER NOT NULL DEFAULT 0,  -- Recording byte offset or transcript file length at indexing
  indexed_mtime INTEGER,                     -- Source-file mtime in seconds for change detection
  indexed_at    INTEGER NOT NULL,
  PRIMARY KEY (session_id, source)
);
CREATE INDEX IF NOT EXISTS idx_search_state_session ON search_index_state(session_id);
"#;

/// Creation statement for the `session_fts` virtual full-text index using a trigram tokenizer for substring/CJK
/// search with case- and diacritic-insensitive matching.
///
/// One FTS row represents one locatable content fragment: a transcript message with `message_index` anchor or a
/// recording line with `ordinal`. `text` is the only indexed column; all others are stored UNINDEXED and returned
/// with matches for navigation and display.
///
/// Created separately from `SCHEMA` by [`crate::db::init_search_index`] with graceful fallback. trigram requires
/// SQLite ≥3.34 and bundled rusqlite (≈3.46) normally satisfies it. If a build lacks support, table creation logs
/// one English error and disables search without affecting the main application.
pub const SESSION_FTS_DDL: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS session_fts USING fts5(
  text,
  session_id    UNINDEXED,
  source        UNINDEXED,
  message_index UNINDEXED,
  ordinal       UNINDEXED,
  role          UNINDEXED,
  ts            UNINDEXED,
  tokenize = 'trigram remove_diacritics 1'
);
"#;
// Do **not** place idx_sessions_parent here. SCHEMA runs before migrate(), when legacy sessions tables do not yet
// have parent_session_id (migrate adds it via ALTER), so index creation would fail with "no such column". Create
// it at the end of migrate() after the column exists; see db/mod.rs.
