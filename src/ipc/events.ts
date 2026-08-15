//! Backend event listeners routed through the transport adapter: Tauri events on desktop, WebSocket relay in browsers.

import { listen, type UnlistenFn } from "./transport";

// PTY output bypasses the event channel and travels directly through spawnPty's binary Channel / WS binary frames; see transport.ts.

/** Listen for a session's natural process exit, such as user `exit` or program completion. */
export function onPtyExit(
  sessionId: string,
  cb: () => void,
): Promise<UnlistenFn> {
  return listen(`pty://exit/${sessionId}`, () => cb());
}

/**
 * Listen for explicit session termination initiated by a client, such as closing a desktop tab or restarting a
 * session. This differs from pty://exit (natural exit): the initiating client cleans up through its own unmount
 * path and ignores the event via its disposed guard, while **other clients** viewing the same session close their
 * local views. They can reopen it from the sidebar instead of remaining silently frozen.
 */
export function onPtyKilled(
  sessionId: string,
  cb: () => void,
): Promise<UnlistenFn> {
  return listen(`pty://killed/${sessionId}`, () => cb());
}

/**
 * Session status signal reported by the backend, matching Rust `StatusSignal` one-to-one.
 *
 * - `state` is structured. Full hooks/plugins establish authoritative state; legacy Codex notify reports only
 *   completion and does not establish authority.
 * - Codex's `agent.stateSource=hooks` selects hook-only mode before the first lifecycle event; neither screen nor
 *   busy signals may participate in its state.
 * - `busy`/`bell`/`title`/`notify` are fallback signals for agents that explicitly retain fallback semantics.
 */
export type StatusSignal =
  // silent=true corrects state without a system notification (Claude idle_prompt or an old snapshot replayed on attach).
  | {
      kind: "state";
      state: "working" | "asking" | "waiting";
      silent?: boolean;
      /** Authoritative full-lifecycle source; once received, screen/busy fallbacks may not override it. */
      authoritative?: boolean;
    }
  | { kind: "busy"; busy: boolean }
  | { kind: "bell" }
  | { kind: "title"; title: string }
  | {
      kind: "agent";
      agent: "claude" | "codex" | "opencode" | "copilot" | "cursor" | "antigravity" | "cline" | "pi" | "crush" | "kimi" | "kiro" | "grok" | "zoo" | null;
      stateSource?: "hooks" | "legacy";
    }
  | { kind: "hook_ready" }
  | { kind: "tool"; tool: string | null }
  // Agent executable missing from PATH: the launch guard reports through the hook (e=notfound), prompting the
  // frontend install card. This one-shot signal is consumed directly by usePtySession (setting agentMissing), not
  // by the agent state machine.
  | { kind: "agent_missing" }
  // OSC 9 / OSC 777 terminal notification fallback: sessions without authoritative hooks capture notifications
  // from the byte stream, and the frontend shows them subject to visibility guards. Sessions with authoritative
  // events ignore these to avoid duplicates. title may be absent or null.
  | { kind: "notify"; title?: string | null; body: string }
  // PTY size/ownership change, broadcast after a successful resize or owner detach and retained in the status
  // snapshot. Consumed by usePtySession's fit/mirror-mode state machine, not the store's agent state machine.
  | { kind: "resized"; cols: number; rows: number; owner: string | null };

/** Listen for status signals from one session. */
export function onPtyStatus(
  sessionId: string,
  cb: (signal: StatusSignal) => void,
): Promise<UnlistenFn> {
  return listen<StatusSignal>(`pty://status/${sessionId}`, (payload) =>
    cb(payload),
  );
}

/**
 * Child-session request initiated inside a session through the user's `vspawn` command or Claude's `/vspawn`
 * skill and relayed by backend `/spawn`. Matches Rust `SpawnRequest` one-to-one in camelCase.
 */
export interface SpawnRequest {
  parentSessionId: string;
  prompt: string;
  kind?: "claude" | "codex" | "opencode" | "copilot" | "cursor" | "antigravity" | "cline" | "pi" | "crush" | "kiro" | "grok" | "zoo" | "terminal" | null;
  worktree?: boolean | null;
  /** Structured model selection; empty inherits the agent default. */
  model?: string | null;
  /** Structured effort selection; handled like `model`. */
  effort?: string | null;
  /** Child session name; empty derives one from the prompt. */
  name?: string | null;
  /** Raw launch arguments; empty applies the per-agent defaults. */
  agentArgs?: string | null;
  permissionMode?: "default" | "skip" | "inherit" | null;
  /** Whether `/orch` may launch this child without the confirmation card. */
  autoApprove?: boolean | null;
  /** Correlation id from `vagent spawn`; when set, the store reports the outcome via spawn_result. */
  requestId?: string | null;
  forceConfirm?: boolean | null;
  /** Advisory warnings for model or effort values outside this build's curated lists. */
  launchWarnings?: string[];
}

/** Listen for child-task requests as a global event registered once on mount. */
export function onSpawnRequest(
  cb: (req: SpawnRequest) => void,
): Promise<UnlistenFn> {
  return listen<SpawnRequest>("spawn://request", (payload) => cb(payload));
}

/** One worktree a retire plan deletes; `resumed` rows keep only a branch and may not know its name. */
export interface RetireWorktree {
  id: string;
  name: string;
  path: string;
  branch?: string | null;
  targetCommit?: string | null;
  resumed?: boolean;
}

/**
 * Retire confirmation request from `vagent retire --confirm`, relayed by backend `/retire`. The backend
 * deletes nothing until the `retire_result` command answers this `requestId`.
 */
export interface RetireRequest {
  requestId: string;
  sessionId: string;
  name: string;
  action: "archive" | "cleanup-and-archive";
  descendantCount: number;
  worktrees: RetireWorktree[];
}

/** Listen for retire confirmation requests as a global event registered once on mount. */
export function onRetireRequest(
  cb: (req: RetireRequest) => void,
): Promise<UnlistenFn> {
  return listen<RetireRequest>("retire://request", (payload) => cb(payload));
}

/**
 * Withdrawal of one retire card whose backend request already timed out. Answering a withdrawn card
 * retires nothing, so the card must close itself instead of accepting an approval that does nothing.
 */
export function onRetireCancel(
  cb: (requestId: string) => void,
): Promise<UnlistenFn> {
  return listen<{ requestId: string }>("retire://cancel", (payload) =>
    cb(payload.requestId),
  );
}

/**
 * Open-document-tab request from a session's `view <file>` command, relayed by backend `/view`. Matches Rust
 * `ViewRequest` one-to-one in camelCase. path is already a canonical absolute path; when `isUrl` is true, it is
 * the original HTTP(S) URL for a built-in browser tab and is consumed only on desktop.
 */
export interface ViewRequest {
  sessionId: string;
  path: string;
  isUrl?: boolean;
}

/** Listen for open-document/browser-tab requests as a global event registered once on mount. */
export function onViewRequest(
  cb: (req: ViewRequest) => void,
): Promise<UnlistenFn> {
  return listen<ViewRequest>("view://request", (payload) => cb(payload));
}

/**
 * Listen for the global tree-changed event broadcast after a successful tree write from any client. Debounce a
 * loadTree refresh to keep sidebars synchronized across clients. Local operations trigger it too; loadTree is
 * idempotent, so a duplicate refresh is acceptable.
 */
export function onTreeChanged(cb: () => void): Promise<UnlistenFn> {
  return listen("tree://changed", () => cb());
}

/** Stable `git clone --progress` phase; operationId isolates progress when clones run concurrently. */
export interface CloneProgress {
  operationId: string;
  stage:
    | "starting"
    | "connecting"
    | "preparing"
    | "receiving"
    | "resolving"
    | "checkout"
    | "finalizing"
    | "importing";
  percent: number | null;
}

export function onCloneProgress(
  cb: (progress: CloneProgress) => void,
): Promise<UnlistenFn> {
  return listen<CloneProgress>("git://clone-progress", cb);
}

/**
 * Action from the native macOS application menu. Backend `on_menu_event` emits custom menu clicks as
 * `menu://action` with the action ID as payload. Only desktop Tauri has a native menu; browser/remote clients do
 * not receive this Tauri-channel event. Terminal split actions are registered here because WKWebView may
 * consume their macOS key equivalents before dispatching a JavaScript keydown event.
 */
export type MenuAction =
  | "settings"
  | "check-update"
  | "share"
  | "split-right"
  | "split-down";

/** Listen for native-menu actions as a global event registered once on mount. */
export function onMenuAction(
  cb: (action: MenuAction) => void,
): Promise<UnlistenFn> {
  return listen<MenuAction>("menu://action", (payload) => cb(payload));
}

/** Full Git Bash download progress (Windows only): received/total bytes while downloading, phase only while extracting. */
export interface GitbashDownloadProgress {
  phase: "download" | "extract";
  received?: number;
  total?: number;
}

/** Listen for full Git Bash download-progress events. */
export function onGitbashDownload(
  cb: (p: GitbashDownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<GitbashDownloadProgress>("gitbash://download", (p) => cb(p));
}

/** Listen for full Git Bash download-complete events. */
export function onGitbashDownloadDone(cb: () => void): Promise<UnlistenFn> {
  return listen("gitbash://download-done", () => cb());
}

/** Listen for full Git Bash download-failure events whose payload is the error text. */
export function onGitbashDownloadError(
  cb: (msg: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("gitbash://download-error", (msg) => cb(msg));
}
