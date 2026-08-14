/** Session ID (backend UUID). */
export type SessionId = string;

/** Session process lifecycle state. */
export type SessionStatus = "idle" | "running" | "exited" | "error";

/** AI agent kind detected within a session. */
export type AgentKind = "claude" | "codex" | "opencode" | "copilot" | "cursor" | "antigravity" | "cline" | "pi" | "crush" | "kimi" | "kiro" | "grok" | "zoo";

/**
 * Session kind, which determines launch behavior.
 * - terminal: plain terminal.
 * - claude / codex / opencode / copilot / cursor / antigravity / cline / pi: agents launched locally by
 *   vlx-term with authoritative status reporting injected through official Claude/Codex hooks, opencode plugin
 *   events, Copilot CLI hook files, Cursor CLI hook files, Antigravity (agy) command hooks merged into the
 *   user's global hooks.json, Cline CLI scripts injected through CLINE_HOOKS_DIR, Pi `-e` extension events, or a
 *   PreToolUse hook injected through Crush's shadow configuration. Crush exposes only PreToolUse and therefore
 *   reports working only; idle state comes from screen detection.
 */
export type SessionKind = "terminal" | "claude" | "codex" | "opencode" | "copilot" | "cursor" | "antigravity" | "cline" | "pi" | "crush" | "kimi" | "kiro" | "grok" | "zoo" | "browser";

/** Agent kinds with a two-state permission-mode toggle. Selecting Skip passes that agent's flag for bypassing
 *  every permission confirmation at launch. opencode controls permissions through configuration and has no
 *  equivalent CLI flag; Pi has no permission-confirmation mechanism; terminal/browser have no such concept.
 *  Cline's direction is reversed because it is fully automatic by default: both states inject an explicit flag
 *  (`--auto-approve false` for default and `--auto-approve true` for skip; see backend
 *  inject::permission_flag), while presenting the same two-state semantics to users. This is the single source
 *  shared by the Edit Session dialog and bottom status-bar permission toggle, preventing constant drift. */
export const PERMISSION_TOGGLE_KINDS: SessionKind[] = ["claude", "codex", "copilot", "cursor", "antigravity", "cline", "crush", "kimi", "kiro", "grok", "zoo"];
export function supportsPermissionToggle(kind: SessionKind): boolean {
  return PERMISSION_TOGGLE_KINDS.includes(kind);
}

/**
 * Agent activity state:
 * - working: processing with ongoing output.
 * - asking: stopped while the UI asks a question or awaits confirmation.
 * - waiting: stopped without a question; the response is ready for review.
 */
export type AgentState = "working" | "asking" | "waiting";

/** Every value a status indicator may display: lifecycle ∪ agent activity states ∪ unavailable hook state. */
export type DisplayStatus = SessionStatus | AgentState | "unavailable";

/** Node kind used by rename, delete, and move operations. */
export type NodeKind = "project" | "group" | "session";

export interface Project {
  id: string;
  name: string;
  rootPath: string;
  color?: string | null;
  sortOrder: number;
  collapsed: boolean;
  /** Optional emoji marker shown before the name in the sidebar; see `Session.mark`. */
  mark?: string | null;
  createdAt: number;
}

export interface Group {
  id: string;
  projectId: string;
  parentGroupId?: string | null;
  name: string;
  sortOrder: number;
  collapsed: boolean;
  createdAt: number;
  /** Git worktree directory associated with the group, or empty. Groups retain their own worktree metadata so
   *  the sidebar can show a worktree tag and new sessions in the group can use it by default, copying it locally. */
  worktreePath?: string | null;
  /** Full ref of this worktree's base branch. Sessions inherit it as their integration target; empty for selected existing worktrees or legacy data. */
  worktreeBaseRef?: string | null;
  /** Optional emoji marker shown before the name in the sidebar; see `Session.mark`. */
  mark?: string | null;
}

export interface Session {
  id: string;
  projectId: string;
  groupId?: string | null;
  name: string;
  /** Session kind: terminal, Claude, Codex, and others. */
  kind: SessionKind;
  shell?: string | null;
  cwd?: string | null;
  envJson?: string | null;
  initCmd?: string | null;
  /** User-defined agent launch arguments such as `--model opus`; appended verbatim at startup for agent sessions only. */
  agentArgs?: string | null;
  /** Two-state permission mode: empty/`"default"` asks incrementally (default); `"skip"` bypasses every permission
   *  confirmation. Agent sessions only; the backend maps it to the appropriate launch flag by kind (none for opencode). */
  permissionMode?: string | null;
  /** Structured model selection such as `fable`; translated to agent flags at launch, before
   *  `agentArgs` so explicit user arguments win. */
  model?: string | null;
  /** Structured reasoning-effort selection such as `high`; handled like `model`. */
  effort?: string | null;
  hotkey?: string | null;
  /** Last remembered native agent session ID for automatic Claude/Codex resume; the frontend only passes/displays it. */
  agentSessionId?: string | null;
  /** Parent session ID; empty for a top-level session, otherwise this session is nested below that parent. */
  parentSessionId?: string | null;
  /** Collapse state, meaningful only for sessions with children. */
  collapsed: boolean;
  /** Associated Git worktree directory, or empty; used to offer cleanup when deleting the session. */
  worktreePath?: string | null;
  /** Full base-branch ref recorded when the worktree was created, such as `refs/heads/dev-electron`; integration
   *  through merge/PR targets it. Legacy worktrees are empty and fall back to the main worktree's current branch. */
  worktreeBaseRef?: string | null;
  /** Archive timestamp in seconds, or empty when active. Archiving hides the session from the normal tree for read-only replay and is reversible. */
  archivedAt?: number | null;
  /** Last URL visited by a browser node (kind=browser); empty for other kinds (architecture document §17). */
  browserUrl?: string | null;
  /** Optional user-chosen emoji marker such as `🔥`, rendered before the node name in the sidebar and usable as a
   *  sidebar filter. The stored value is the emoji itself, so markers unknown to this build still round-trip.
   *  Empty or absent means unmarked. */
  mark?: string | null;
  sortOrder: number;
  createdAt: number;
}

/** Complete tree snapshot. */
export interface Tree {
  projects: Project[];
  groups: Group[];
  sessions: Session[];
}

/** In-memory session runtime state. */
export interface SessionRuntime {
  status: SessionStatus;
  pid?: number;
  /** Process start time in milliseconds, recorded after successful spawn and used by Info for started/uptime. */
  startedAt?: number;
  exitCode?: number;
  /** Agent detected in the session; null/undefined for non-agent sessions. */
  agent?: AgentKind | null;
  /** Agent activity state; meaningless when agent is absent. */
  agentState?: AgentState | null;
  /**
   * Codex activity source selected by the backend at launch. Modern Codex is `hooks` and must never accept
   * terminal-screen or output-activity guesses; `legacy` may receive turn-complete notify but likewise does not
   * fabricate precise state. Other agents leave this unset.
   */
  agentStateSource?: "hooks" | "legacy";
  /** Whether modern Codex has completed its SessionStart hook callback. False means hook health is unproven. */
  agentHookReady?: boolean;
  /**
   * Whether a full authoritative lifecycle event has been received from official hooks or an agent plugin.
   * For Codex, only lifecycle hooks qualify; the legacy turn-complete-only notify does not. Once true, ignore
   * heuristic fallback signals such as busy/bell when determining activity to prevent interference.
   */
  authoritative?: boolean;
  /** Backend activity indicating sustained output; used only to infer state for fallback agents run manually in Terminal sessions. */
  busy?: boolean;
  /** Most recent OSC terminal title, for information display. */
  title?: string | null;
  /**
   * Name of the tool currently being invoked, reported live by lifecycle hooks and cleared at Stop.
   * Currently populated for Claude and Grok sessions and displayed in the Info panel.
   */
  currentTool?: string | null;
  /**
   * Whether this session has ever entered working, used by screen detection. Screen idle maps to `waiting`
   * (response ready for review) only after everWorked=true; otherwise, a newly launched empty prompt would
   * falsely appear as waiting. See plan §3.6.
   */
  everWorked?: boolean;
  /**
   * Start time in milliseconds of continuous screen-detected idle, used by arbitration to require stable idle
   * for more than two seconds. Reset to zero for any non-idle result; record the current time on first idle.
   */
  lastIdleAt?: number;
  // Note: the "most recent working transition" used by the 1200 ms working→idle debounce has moved out of this
  // type. It changes on every working signal and would defeat applyStatusSignal's value-level guard (React-side
  // deduplication). It now lives in termStore's module-level workingPulseAt Map for internal decisions only.
  /**
   * Agent executable is missing. When the interactive-shell guard cannot find the agent on PATH at launch, it
   * reports through the hook (`e=notfound`), and the backend emits `agent_missing` (see inject::launch_cmd and
   * server.rs). onPtyStatus sets this to true, causing TerminalView to show AgentInstallCard. Using an authoritative
   * hook instead of screen scraping avoids mistaking echoed guard commands for failures. Reset to false after each
   * successful (re)spawn; if still absent, the guard reports again and restores true. Relevant only to agent sessions.
   */
  agentMissing?: boolean;
  /**
   * One-click installation is in progress. Set true after writing the install command to the session shell. No
   * persistent UI overlays the installation because the terminal already streams its output. TerminalView's
   * destination polling (useAgentInstallLocator) checks the install location every three seconds. Clear on
   * detection (which opens the Installation Complete dialog), timeout, or the next spawn result (found/still
   * missing). Runtime storage survives tab switches and pane remounts. Relevant only to agent sessions.
   */
  agentInstalling?: boolean;
  /**
   * Executable path discovered after one-click installation and written into settings automatically. When
   * non-empty, AgentInstallCard shows the Installation Complete dialog with the path and Restart Now/Later.
   * Clear after user action or the next successful spawn. This belongs in runtime rather than component state:
   * restart remounts the whole pane by advancing epoch and would clear component state, while runtime survives
   * remounts and tab changes. Relevant only to agent sessions.
   */
  agentPathSaved?: string | null;
  /**
   * Complete launch command assembled by the backend and actually written to the PTY for this spawn, including
   * agent flags, resume/permission/custom arguments, and the shell guard. Record after a successful agent-session
   * spawn; plain terminals (launch=None) do not. Displayed only in Session Info opened while holding Option and
   * never used for logic. Attaching to a running session without launch does not overwrite the existing value.
   */
  launchCmd?: string;
}

/**
 * Resolve the state displayed by a status indicator. Process exit/error takes precedence; otherwise agent
 * sessions use agent activity, and all other cases fall back to lifecycle state (idle/running).
 */
export function effectiveStatus(rt: SessionRuntime | undefined): DisplayStatus {
  if (!rt) return "idle";
  if (rt.status === "exited" || rt.status === "error") return rt.status;
  if (rt.agent && rt.agentState) return rt.agentState;
  if (
    rt.agent === "codex" &&
    rt.agentStateSource === "hooks" &&
    !rt.agentHookReady
  ) {
    return "unavailable";
  }
  return rt.status;
}

/**
 * Whether one session matches a state category. Shared by tree indicator colors, filtered snapshots, and counts,
 * ensuring colors, numbers, and filter results correspond exactly:
 * - working (green): the agent is active.
 * - asking (yellow): the UI is asking/awaiting confirmation, or an unread notification needs attention.
 * - waiting (magenta): a response has arrived and has already been viewed (not unread).
 */
export function matchesAgentState(
  category: AgentState,
  st: DisplayStatus,
  unread: boolean,
): boolean {
  if (category === "asking") return st === "asking" || unread;
  if (category === "waiting") return st === "waiting" && !unread;
  return st === category;
}

/**
 * Session counts for the three agent states, shared by the sidebar state-filter menu and bottom status bar so
 * both show identical numbers. Unread depends solely on whether the notification marker remains. Opening a
 * session no longer clears it immediately; useNotifications waits two seconds of viewing. During that interval,
 * the active session remains unread/awaiting response instead of jumping instantly to viewed.
 */
export function countByAgentState(
  sessions: readonly { id: string }[],
  runtimes: Record<string, SessionRuntime>,
  notifications: Record<string, unknown>,
): Record<AgentState, number> {
  const c: Record<AgentState, number> = { working: 0, asking: 0, waiting: 0 };
  for (const s of sessions) {
    const st = effectiveStatus(runtimes[s.id]);
    const unread = s.id in notifications;
    for (const k of ["working", "asking", "waiting"] as const) {
      if (matchesAgentState(k, st, unread)) c[k]++;
    }
  }
  return c;
}

/** Git status for a directory. */
export interface GitStatus {
  isRepo: boolean;
  branch?: string | null;
  ahead: number;
  behind: number;
  staged: number;
  unstaged: number;
  untracked: number;
  /** Whether the current directory is a linked Git worktree rather than the main worktree. */
  isWorktree?: boolean;
  /** Top-level working directory for this worktree, present only when isWorktree is true. */
  worktreePath?: string | null;
}
