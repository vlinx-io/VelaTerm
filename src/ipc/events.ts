//! Backend event listeners routed through the transport adapter: Tauri events on desktop, WebSocket relay in browsers.

import {
  isRemoteWindow,
  listen,
  listenNative,
  type UnlistenFn,
} from "./transport";
import type { MirrorSnapshot, RemoteClient } from "./mirror";
import type { SessionStateBatch } from "./sessionState";
import type { KillReason, UsageSnapshot } from "./commands";
import type { AgentKind } from "../types";

// PTY output bypasses the event channel and travels directly through spawnPty's binary Channel / WS binary frames; see transport.ts.

/** Listen for a session's natural process exit, such as user `exit` or program completion. */
export function onPtyExit(
  sessionId: string,
  cb: () => void,
): Promise<UnlistenFn> {
  return listen(`pty://exit/${sessionId}`, () => cb());
}

/** Who ended a session deliberately, and whether it is coming back. */
export interface PtyKilled {
  /** Connection ID of the client that asked: `desktop`, or `ws-N`. Empty when the backend shut down. */
  source: string;
  /** `restart` means a new process for this same session follows; `close` means it is going away. */
  reason: KillReason;
}

/**
 * Listen for explicit session termination initiated by a client, such as closing a desktop tab or restarting a
 * session. This differs from pty://exit (natural exit): the initiating client cleans up through its own unmount
 * path, while **other clients** viewing the same session close their local views. They can reopen it from the
 * sidebar instead of remaining silently frozen.
 *
 * The payload is what makes that decision possible. It used to be empty, so a client could only guess why the
 * session died — and guessing "gone" meant restarting a session closed its tab on every other client. The
 * requester likewise had to recognise its own echo through a three-second timing window; `source` says so
 * outright.
 */
export function onPtyKilled(
  sessionId: string,
  cb: (ev: PtyKilled) => void,
): Promise<UnlistenFn> {
  return listen<Partial<PtyKilled> | null>(
    `pty://killed/${sessionId}`,
    (payload) =>
      cb({
        source: payload?.source ?? "",
        // An unknown reason has to read as `close`: keeping a pane for a session that never comes back
        // leaves a dead terminal on screen, which is worse than closing one the user meant to restart.
        reason: payload?.reason === "restart" ? "restart" : "close",
      }),
  );
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
      agent:
        | "claude"
        | "codex"
        | "opencode"
        | "copilot"
        | "cursor"
        | "antigravity"
        | "cline"
        | "pi"
        | "crush"
        | "kimi"
        | "kiro"
        | "grok"
        | "zoo"
        | null;
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
  kind?: AgentKind | "terminal" | null;
  worktree?: boolean | null;
  /** Model override chosen in the spawn confirmation dialog; null inherits from parent or defaults. */
  model?: string | null;
  /** Effort override chosen in the spawn confirmation dialog; null inherits from parent or defaults. */
  effort?: string | null;
  /** Set by `vspawn --yes`: run immediately with default settings, bypassing the confirmation card. */
  noConfirm?: boolean | null;
}

/** Listen for child-task requests as a global event registered once on mount. */
export function onSpawnRequest(
  cb: (req: SpawnRequest) => void,
): Promise<UnlistenFn> {
  return listen<SpawnRequest>("spawn://request", (payload) => cb(payload));
}

/** Payload broadcast when any client confirms or cancels a spawn confirmation card. */
export interface SpawnResolved {
  source: string;
  parentSessionId: string;
  prompt: string;
  confirmed: boolean;
}

/** Listen for spawn-resolved events so other clients can dismiss their matching confirmation card. */
export function onSpawnResolved(
  cb: (ev: SpawnResolved) => void,
): Promise<UnlistenFn> {
  return listen<SpawnResolved>("spawn://resolved", (payload) => cb(payload));
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

/**
 * Listen for agent-preset changes broadcast after any client creates, edits, deletes or reorders one.
 * Presets appear in every client's new-session menu, so each reloads its list. Kept separate from the tree
 * event because presets live outside the session tree and change far less often.
 */
export function onPresetsChanged(cb: () => void): Promise<UnlistenFn> {
  return listen("presets://changed", () => cb());
}

/**
 * Listen for the account-usage snapshot the backend keeps for this machine. One background poller writes
 * it, so every window, browser client, and session shows the same numbers without querying providers.
 */
export function onUsageChanged(
  cb: (snap: UsageSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<UsageSnapshot>("usage://changed", (payload) => cb(payload));
}

/**
 * Listen for application preferences written by any client. The payload lists the key names that were
 * written and deliberately carries **no values**: `remoteAccess.*` and `gitea.token` are stripped from a
 * remote client's `get_app_settings` reply, and a broadcast carrying values would bypass that filter.
 * Receivers re-read through `reconcileSettings()`, so the filter keeps applying.
 */
export function onSettingsChanged(
  cb: (keys: string[]) => void,
): Promise<UnlistenFn> {
  return listen<string[]>("settings://changed", (payload) =>
    cb(Array.isArray(payload) ? payload : []),
  );
}

/**
 * Listen for authoritative session records the backend has just changed.
 *
 * This is a **connection-level** subscription, registered when the socket opens rather than when a
 * session is attached. Per-session status events only reach a client that has opened that session, which
 * is why a freshly connected browser used to show a dot on nothing but its own tabs.
 *
 * The payload carries only the records that changed, so merge it — a session missing from a batch has
 * not been reset, it simply has no news.
 */
export function onSessionState(
  cb: (batch: SessionStateBatch) => void,
): Promise<UnlistenFn> {
  return listen<SessionStateBatch>("session://state", (payload) =>
    cb(payload && typeof payload === "object" ? payload : {}),
  );
}

/**
 * Listen for a layout published by any client in mirror mode. The payload carries the publisher's connection ID,
 * which the receiver compares against its own `getClientSource()` to drop its own echo.
 */
export function onMirrorLayout(
  cb: (snap: MirrorSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<MirrorSnapshot>("mirror://layout", cb);
}

/**
 * Listen for a remote client connecting or disconnecting.
 *
 * The host never counts itself — it uses IPC, not a WebSocket — so on a host this is the number of other
 * windows attached to its service.
 */
export function onRemoteClients(
  cb: (count: number, clients: RemoteClient[]) => void,
): Promise<UnlistenFn> {
  return listen<{ count: number; clients?: RemoteClient[] }>(
    "clients://changed",
    (payload) =>
      cb(
        typeof payload?.count === "number" ? payload.count : 0,
        Array.isArray(payload?.clients) ? payload.clients : [],
      ),
  );
}

/** Listen for the host switching mirror mode on or off. */
export function onMirrorMode(
  cb: (enabled: boolean) => void,
): Promise<UnlistenFn> {
  return listen<{ enabled: boolean }>("mirror://mode", (payload) =>
    cb(payload.enabled),
  );
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
 * `menu://action` with the action ID as payload. Only desktop Tauri has a native menu; plain browsers do
 * not receive this event. Remote windows (SSH / paired URL) are local Tauri windows and do receive it,
 * but their regular `listen` goes over the WebSocket relay to the remote server, so they must listen
 * natively. Terminal split actions are registered here because WKWebView may consume their macOS key
 * equivalents before dispatching a JavaScript keydown event.
 */
export type MenuAction =
  | "settings"
  | "check-update"
  | "share"
  | "clear-badges"
  | "split-right"
  | "split-down";

/** Listen for native-menu actions as a global event registered once on mount. */
export function onMenuAction(
  cb: (action: MenuAction) => void,
): Promise<UnlistenFn> {
  if (isRemoteWindow) {
    return listenNative<MenuAction>("menu://action", (payload) => cb(payload));
  }
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
