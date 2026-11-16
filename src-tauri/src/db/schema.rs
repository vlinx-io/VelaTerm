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
  collection_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
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
  collaboration_mode TEXT,
  agent_preset_id TEXT,
  agent_path  TEXT,
  hotkey      TEXT,
  agent_session_id TEXT,
  legacy_agent_session_id TEXT,
  parent_session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
  collapsed   INTEGER NOT NULL DEFAULT 0,
  worktree_path TEXT,
  worktree_base_ref TEXT,
  archived_at INTEGER,
  fork_pending INTEGER NOT NULL DEFAULT 0,
  browser_url TEXT,
  engine      TEXT NOT NULL DEFAULT 'tui',
  mark        TEXT,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_groups_project ON groups(project_id);
CREATE INDEX IF NOT EXISTS idx_groups_parent ON groups(parent_group_id);
CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id);
CREATE INDEX IF NOT EXISTS idx_sessions_group ON sessions(group_id);

CREATE TABLE IF NOT EXISTS session_model_settings (
  session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  model TEXT,
  effort TEXT,
  native_state TEXT
);

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
-- command_core. CREATE IF NOT EXISTS is safe for old databases; migrate adds their shared_db and mirror
-- columns.
CREATE TABLE IF NOT EXISTS ssh_hosts (
  target            TEXT PRIMARY KEY,   -- user@host[:port], also used as the keyring account
  label             TEXT,               -- Reserved for a user-defined alias
  last_connected_at INTEGER NOT NULL,   -- Last successful connection time in seconds; sorted descending
  shared_db         INTEGER NOT NULL DEFAULT 0,  -- Previous "reuse remote desktop database" choice
  mirror            INTEGER NOT NULL DEFAULT 0,  -- Previous "mirror layout across devices" choice
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

-- Reusable agent launch configurations. A preset is display and launch data only: `base_kind` says which
-- built-in agent it behaves like, so hook injection, resume and status detection keep using SessionKind
-- unchanged, while the preset supplies the name, icon, executable and default arguments. This is what lets
-- several drop-in replacements of the same CLI run side by side, which a single per-kind executable path
-- in app_settings cannot express.
--
-- Sessions copy a preset's values onto their own row at creation and keep `agent_preset_id` for display
-- only, so editing or deleting a preset never disturbs sessions already created from it.
--
-- `icon` holds a base64 data URL rather than a file path: remote and browser clients cannot read a local
-- file, and an inline value reaches them through the existing tree sync.
CREATE TABLE IF NOT EXISTS agent_presets (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL,
  base_kind       TEXT NOT NULL,              -- SessionKind string; currently always 'claude'
  exec_path       TEXT,                       -- Absolute path; empty falls back to PATH lookup
  agent_args      TEXT,
  permission_mode TEXT,
  icon            TEXT,                       -- base64 data URL, size-capped at write time
  sort_order      INTEGER NOT NULL DEFAULT 0,
  created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chat_codex_settings (
  session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  service_tier TEXT,
  personality TEXT
);
-- Claude conversation-view switches chosen for one conversation. `chrome` is 1 or 0 once the user picked it;
-- a missing row or null follows the `chatChromeDefault` preference in `vlx-settings`.
CREATE TABLE IF NOT EXISTS chat_claude_settings (
  session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  chrome INTEGER
);

-- One `/vorch` orchestration: a parent session asked for several agents at once. Without these two tables an
-- orchestration would leave no trace beyond unrelated-looking sibling sessions, and nothing could report on it
-- afterwards. Rows are kept after the run so its outcome stays inspectable; deleting the parent takes them.
CREATE TABLE IF NOT EXISTS orch_runs (
  id                TEXT PRIMARY KEY,
  parent_session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  title             TEXT NOT NULL,
  created_at        INTEGER NOT NULL
);

-- One agent within a run. `session_id` stays NULL until the frontend has created that session and reported it
-- back, and stays NULL forever when the user removed that entry in the confirmation dialog: the request is the
-- record, and what became of it is filled in later. `status` distinguishes the two NULL cases.
CREATE TABLE IF NOT EXISTS orch_agents (
  orch_id    TEXT NOT NULL REFERENCES orch_runs(id) ON DELETE CASCADE,
  idx        INTEGER NOT NULL,
  name       TEXT NOT NULL,
  session_id TEXT,
  status     TEXT NOT NULL DEFAULT 'pending',
  PRIMARY KEY (orch_id, idx)
);
CREATE INDEX IF NOT EXISTS idx_orch_runs_parent ON orch_runs(parent_session_id);
CREATE INDEX IF NOT EXISTS idx_orch_agents_session ON orch_agents(session_id);
"#;

// Do **not** place idx_sessions_parent here. SCHEMA runs before migrate(), when legacy sessions tables do not yet
// have parent_session_id (migrate adds it via ALTER), so index creation would fail with "no such column". Create
// it at the end of migrate() after the column exists; see db/mod.rs.
