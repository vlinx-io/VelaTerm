//! Persistent data models serialized as camelCase to match frontend TypeScript.

use serde::{Deserialize, Serialize};

/// Session type, which determines launch behavior.
/// - `Terminal`: a plain login shell and the backward-compatible default.
/// - Agent types are launched by VelaTerm with official hooks, notify handlers, plugins, or extensions
///   injected to obtain authoritative lifecycle state. OpenCode uses local plugin events; Copilot uses
///   command hooks under `~/.copilot/hooks/`; Cursor merges hooks into `~/.cursor/hooks.json`; Cline uses
///   scripts under `<data_dir>/cline/hooks/` through `CLINE_HOOKS_DIR` without user-config changes; Pi
///   loads a TypeScript extension through `-e <absolute path>` and supports native `--fork`; Antigravity
///   merges command hooks into global Gemini hooks and resumes with `--conversation=<id>`; Kiro clones the
///   user's default agent into a shadow config at `~/.kiro/agents/vlx-term.json`, merges its hooks in, and
///   launches with `--agent vlx-term`; Crush uses a
///   shadow configuration under `<data_dir>/crush/crush.json` through `CRUSH_GLOBAL_CONFIG`. Crush exposes
///   only PreToolUse, so the frontend detects idle state from the screen and resumes with `--session <id>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionKind {
    #[default]
    Terminal,
    Claude,
    Codex,
    Opencode,
    Copilot,
    Cursor,
    /// Google Antigravity CLI (`agy`), ordered before Cline to match the menu.
    Antigravity,
    Cline,
    Pi,
    /// Moonshot AI Kimi Code CLI (`kimi`), with K3 available by default and official lifecycle hooks.
    Kimi,
    /// Kiro CLI (`kiro-cli chat`), driven by lifecycle hooks in a shadow agent config and resumed with
    /// `--resume-id <id>`. It exposes no permission-request hook, so it has no asking state.
    Kiro,
    /// xAI Grok Build CLI (`grok`), defaulting to Grok 4.5 with caller-controlled model overrides.
    Grok,
    /// Zoo Code headless CLI, currently executable as `roo`, using screen/activity state fallbacks.
    Zoo,
    /// charmbracelet/crush (`crush`), with only a PreToolUse hook injected through shadow configuration.
    Crush,
    /// Built-in browser page with no PTY or agent. Opening creates a center-pane tab at `browser_url`.
    /// Its name and latest URL are persisted, and it is available only on desktop clients.
    Browser,
}

impl SessionKind {
    /// Lowercase string used for persistence.
    pub fn as_str(self) -> &'static str {
        match self {
            SessionKind::Terminal => "terminal",
            SessionKind::Claude => "claude",
            SessionKind::Codex => "codex",
            SessionKind::Opencode => "opencode",
            SessionKind::Copilot => "copilot",
            SessionKind::Cursor => "cursor",
            SessionKind::Antigravity => "antigravity",
            SessionKind::Cline => "cline",
            SessionKind::Pi => "pi",
            SessionKind::Kimi => "kimi",
            SessionKind::Kiro => "kiro",
            SessionKind::Grok => "grok",
            SessionKind::Zoo => "zoo",
            SessionKind::Crush => "crush",
            SessionKind::Browser => "browser",
        }
    }

    /// Restore from a database string, falling back to Terminal for forward compatibility.
    pub fn from_db(s: &str) -> Self {
        match s {
            "claude" => SessionKind::Claude,
            "codex" => SessionKind::Codex,
            "opencode" => SessionKind::Opencode,
            "copilot" => SessionKind::Copilot,
            "cursor" => SessionKind::Cursor,
            "antigravity" => SessionKind::Antigravity,
            "cline" => SessionKind::Cline,
            "pi" => SessionKind::Pi,
            "kimi" => SessionKind::Kimi,
            "kiro" => SessionKind::Kiro,
            "grok" => SessionKind::Grok,
            "zoo" => SessionKind::Zoo,
            "crush" => SessionKind::Crush,
            "browser" => SessionKind::Browser,
            _ => SessionKind::Terminal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub color: Option<String>,
    pub sort_order: i64,
    pub collapsed: bool,
    /// Optional emoji marker shown before the name in the sidebar; see [`Session::mark`].
    pub mark: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub project_id: String,
    /// None indicates a top-level group under the project root.
    pub parent_group_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub collapsed: bool,
    pub created_at: i64,
    /// Git worktree associated with this group. The sidebar displays its tag and new sessions in the group
    /// copy it as their default worktree.
    pub worktree_path: Option<String>,
    /// Full baseline branch ref recorded when creating the worktree, inherited by sessions as their landing
    /// target. Empty for selected existing worktrees and older databases.
    pub worktree_base_ref: Option<String>,
    /// Optional emoji marker shown before the name in the sidebar; see [`Session::mark`].
    pub mark: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub project_id: String,
    /// None indicates placement directly under the project root.
    pub group_id: Option<String>,
    pub name: String,
    /// Session type whose string value matches [`SessionKind::as_str`].
    pub kind: SessionKind,
    pub shell: Option<String>,
    pub cwd: Option<String>,
    pub env_json: Option<String>,
    pub init_cmd: Option<String>,
    /// User-defined agent launch arguments such as `--model opus`. Local agent sessions split and append
    /// them after built-in flags and before the initial prompt. Persisted across launches/resumes and
    /// trusted at the same level as user-provided init_cmd shell input.
    pub agent_args: Option<String>,
    /// Per-agent-session permission mode: None/`default` uses staged approval and `skip` bypasses all
    /// confirmations. At launch, `inject::permission_flag` maps this to agent-specific flags such as
    /// Claude `--dangerously-skip-permissions`, Codex
    /// `--dangerously-bypass-approvals-and-sandbox`、copilot `--allow-all-tools`、cursor
    /// `--force`; OpenCode/Pi have no flag, while Cline injects `--auto-approve true/false` in both modes.
    /// Persisted independently of `agent_args`.
    pub permission_mode: Option<String>,
    /// Structured model selection such as `fable`. Translated to agent-specific flags at launch,
    /// before `agent_args` so explicit user arguments win. None uses the agent default.
    pub model: Option<String>,
    /// Structured reasoning-effort selection such as `high`; handled like [`Session::model`].
    pub effort: Option<String>,
    pub hotkey: Option<String>,
    /// Last recorded native agent session ID, or None before a resumable conversation exists. Reopening
    /// passes it unchanged to Claude `--resume`, Codex `resume`, or the corresponding agent mechanism.
    pub agent_session_id: Option<String>,
    /// Parent session ID. None means top-level; a child belongs through this field rather than `group_id`.
    pub parent_session_id: Option<String>,
    /// Collapsed state for expanding or hiding child sessions in the sidebar.
    pub collapsed: bool,
    /// Associated Git worktree directory, used to offer cleanup when deleting the session.
    pub worktree_path: Option<String>,
    /// Full baseline branch ref recorded when creating the worktree. Landing or opening a pull request
    /// targets this independently of session hierarchy. Only independent worktrees have a value; older
    /// records fall back to the primary worktree's current branch.
    pub worktree_base_ref: Option<String>,
    /// Archive timestamp in seconds. Archived sessions are reversibly hidden and available read-only.
    pub archived_at: Option<i64>,
    /// Last URL for browser nodes, empty for other types. The frontend debounces navigation updates through
    /// `set_browser_url` and reloads this value next time.
    pub browser_url: Option<String>,
    /// Optional user-chosen emoji marker, such as `🔥`, displayed before the node name in the sidebar and
    /// usable as a sidebar filter. The stored value is the emoji itself rather than an enumerated code, so
    /// markers written by a newer build survive round-trips here. None or empty means unmarked.
    pub mark: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
}

/// Complete tree snapshot returned for frontend hierarchy assembly.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tree {
    pub projects: Vec<Project>,
    pub groups: Vec<Group>,
    pub sessions: Vec<Session>,
}

/// Node kind used by rename, delete, and move to select the target table.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Project,
    Group,
    Session,
}
