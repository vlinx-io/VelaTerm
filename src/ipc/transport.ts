//! Transport adapter exposing backend calls, event listeners, and PTY output through environment-neutral APIs.
//!
//! Automatically selects one implementation:
//! - **Desktop Tauri WebView** uses `@tauri-apps/api` invoke/Channel/listen and plugins.
//! - **Browser remote access** uses a single WebSocket (see `wsClient.ts`).
//!
//! Higher layers depend only on this module, allowing the same React code to run on desktop and browser.

import { Channel, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { t } from "../i18n";
import { genId } from "../genId";
import { recordRequestError } from "./reqLog";
import { safeCommand, safeError, diagnosticOperation } from "./diagnosticSafety";
import { apiUrl } from "./shareBase";
import { wsClient, bytesToB64 } from "./wsClient";
import { acceptsHistoryProtocol, historyProtocolError, requiresHistoryProtocol } from "./historyCapability";
import { mapBackendError } from "./backendError";
import type { SessionKind } from "../types";

/** Whether this is a Tauri WebView rather than browser remote access. Remote-connection windows are physically
 *  Tauri WebViews but must use WebSocket transport, so their init script sets __VLX_FORCE_BROWSER__. */
export const isTauri =
  typeof window !== "undefined" &&
  "__TAURI_INTERNALS__" in window &&
  !(window as any).__VLX_FORCE_BROWSER__;

/** Whether this is the Electron shell's own window, marked by its preload. It is a desktop host like Tauri but
 *  reaches its local backend over WebSocket. platform/env.ts derives `env.isElectron` from this. */
export const isElectronShell =
  typeof window !== "undefined" && !!(window as any).__VLX_ELECTRON__;

/** Whether the current platform is macOS, used only for UI labels. Shortcut logic uses metaKey || ctrlKey. */
export const isMac =
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.userAgent || (navigator as any).platform || "");

/** Whether this is a remote-connection window: a local wry window using WS as a browser while retaining
 *  __TAURI_INTERNALS__ for local native commands such as clipboard writes. */
export const isRemoteWindow =
  typeof window !== "undefined" &&
  !!(window as any).__VLX_REMOTE__ &&
  "__TAURI_INTERNALS__" in window;

/** SSH connection session ID injected through __VLX_REMOTE__ for SSH remote windows; null for paired URLs and plain
 *  browsers. The disconnect banner uses it to filter tunnel-state events and request tunnel reconstruction. */
export const remoteSshSession: string | null =
  (typeof window !== "undefined" &&
    (window as any).__VLX_REMOTE__?.session) ||
  null;

/** WSL windows use the same remote session envelope with their own lifecycle events. */
export const remoteConnectionKind: "ssh" | "wsl" =
  typeof window !== "undefined" && (window as any).__VLX_REMOTE__?.transport === "wsl" ? "wsl" : "ssh";

// PTY launch arguments and results.

export interface PtySpawnArgs {
  /** Diagnostic correlation only; never persisted as session configuration. */
  diagnosticRequestId?: string;
  diagnosticOperationId?: string;
  sessionId: string;
  kind: SessionKind;
  shell?: string;
  cwd?: string;
  cols: number;
  rows: number;
  /** Initial task prompt for a spawned child session, injected only on first launch. */
  initialPrompt?: string;
  /**
   * Resolved UI brightness for Claude --settings theme. Windows ConPTY answers Claude's OSC 11 query using its
   * own dark default, so automatic detection is always dark; the frontend sends the real light/dark value.
   */
  theme?: "light" | "dark";
  /**
   * Current theme brightness written to COLORFGBG at spawn for TUIs such as OpenCode that do not query OSC 11.
   */
  dark?: boolean;
}

export interface PtySpawnResult {
  pid: number;
  launch: string | null;
  /**
   * Whether this attached to a running session rather than starting one. Attach must skip initCmd fallback because
   * the shell already ran its launch command and must not execute it again.
   */
  attached: boolean;
  /** Current PTY columns: launch input for new sessions or current value when attached. Reliable initial mirror size. */
  cols: number;
  /** Current PTY rows, with the same semantics as columns. */
  rows: number;
  /**
   * Current sizing owner source (`desktop` or `ws-N`), null when unowned. An attached client whose source differs
   * enters mirror mode.
   */
  owner: string | null;
}

/**
 * This client's source ID: always `"desktop"` locally, or the browser WS ID from the first server hello frame.
 * Compare with Resized.owner/SpawnResult.owner to determine sizing ownership.
 */
export function getClientSource(): string | null {
  return isTauri ? "desktop" : wsClient.getSource();
}

// Generic invoke and listen.

/**
 * Desktop whitelist for native commands that must bypass unified desktop_call dispatch:
 * - Typing hot paths (pty_write/pty_resize) avoid spawn_blocking thread hops on every keystroke.
 * - Main-thread window/native child-view operations (open_remote_window, open_devtools, browser_*) must execute
 *   on the window host thread.
 * - Remote-host management commands (web_pairing_create/web_devices_list/web_device_revoke) stay native
 *   direct commands on the desktop; matching dispatch arms exist for the Electron loopback sidecar, and
 *   remote WS clients are rejected there by the backend's origin gate.
 * - Remote fingerprint probe/trust commands pair with open_remote_window and exist only as native commands.
 * Everything else uses desktop_call, whose single backend entry moves blocking data operations off the UI thread.
 */
const DIRECT_DESKTOP_CMDS = new Set([
  "pty_write",
  "pty_resize",
  "open_remote_window",
  "open_account_remote_window",
  "probe_remote_fingerprint",
  "url_trust_fingerprint",
  "open_devtools",
  // Dropped-file paths come from the local window's own drag channel, which only the desktop process can read.
  "take_dropped_paths",
  // Native chrome tinting touches the window/appearance on every platform, so it must stay a native command.
  "set_native_theme",
  "ssh_probe_host",
  "ssh_trust_host",
  "ssh_connect",
  "wsl_connect",
  "connect_cancel",
  "ssh_disconnect",
  "browser_open",
  "browser_navigate",
  "browser_back",
  "browser_forward",
  "browser_reload",
  "browser_stop",
  "browser_set_bounds",
  "browser_set_visible",
  "browser_close",
  "web_pairing_create",
  "web_devices_list",
  "web_device_revoke",
]);

/** Invokes a backend command.
 *
 *  Before rejection reaches the caller, [`recordRequestError`] records and broadcasts every failure through the
 *  console, ring buffer, and UI banner. It never retries; callers own retry policy. Errors remain visible even
 *  when a caller intentionally swallows rejection, such as `void loadTree()` during startup. */
export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const requestId = genId();
  const started = performance.now();
  const tracked = !["pty_write", "pty_resize", "diagnostic_event", "diagnostic_health"].includes(cmd);
  return invokeInner<T>(cmd, args, tracked ? { requestId, operationId: diagnosticOperation(args?.sessionId ?? args?.id) } : undefined).then((result) => {
    if (tracked) diagnosticEvent("request", { requestId, command: safeCommand(cmd), status: "success", clientDurationMs: Math.round(performance.now() - started) });
    return result;
  }).catch((err) => {
    const raw = err instanceof Error ? err.message : typeof err === "string" ? err : undefined;
    const localized = raw === undefined ? undefined : mapBackendError(raw);
    if (raw !== localized) err = err instanceof Error ? new Error(localized) : localized;
    if (tracked) diagnosticEvent("request", { requestId, command: safeCommand(cmd), status: "failed", errorCode: safeError(err), clientDurationMs: Math.round(performance.now() - started) });
    // User-canceled clone is handled control flow, not a global error; cleanup failures return another error.
    const message = String(err);
    if (!(cmd === "clone_project" && (message === "CLONE_CANCELLED" || message === "Error: CLONE_CANCELLED"))) {
      recordRequestError(cmd, err);
    }
    throw err;
  });
}

let pendingDiagnostics = 0;

/** Submit only fixed diagnostic events. Delivery failure never re-enters request error logging. */
export function diagnosticEvent(event: "request" | "shell_switch" | "restart" | "pty_output" | "ws_state" | "pty_spawn" | "input_latency" | "startup_ui", fields: Record<string, unknown>): void {
  if (pendingDiagnostics >= 24 || (!isTauri && !wsClient.isDiagnosticReady())) return;
  pendingDiagnostics++;
  void invokeInner("diagnostic_event", { operationId: diagnosticOperation(fields.sessionId), ...fields, event })
    .catch(() => {}).finally(() => { pendingDiagnostics--; });
}

let desktopHistoryProtocol: Promise<void> | undefined;

function requireDesktopHistoryProtocol(): Promise<void> {
  // A new command is safe to probe: an old dispatcher rejects its name without reading history.
  // Never probe chat_snapshot with a new flag that an older backend could silently ignore.
  return desktopHistoryProtocol ??= tauriInvoke<{ historyProtocol?: number }>("desktop_call", { cmd: "chat_capabilities", args: {} })
    .then(result => { if (!acceptsHistoryProtocol(result?.historyProtocol)) throw historyProtocolError(); })
    .catch(error => {
      if (/Unknown command: chat_capabilities|history_protocol_unsupported:/.test(String(error))) throw historyProtocolError();
      desktopHistoryProtocol = undefined;
      throw error;
    });
}

function invokeInner<T>(cmd: string, args?: Record<string, unknown>, diagnostic?: { requestId: string; operationId?: string }): Promise<T> {
  if (cmd === "open_account_remote_window" && (window as unknown as {__VLX_ELECTRON__?:boolean}).__VLX_ELECTRON__) {
    // Electron cannot use the Tauri command: ask the sidecar for the relay URL over the existing
    // authenticated WebSocket, then open it in a dedicated window through the preload bridge.
    return (async () => {
      const result = await wsClient.invoke<{ url: string }>("public_account_remote_url", {
        deviceId: String(args?.deviceId ?? ""),
        grantId: args?.grantId ?? null,
      });
      await (
        window as unknown as { vlxNative: { openAccountRemoteWindow: (url: string) => Promise<void> } }
      ).vlxNative.openAccountRemoteWindow(result.url);
      return undefined as T;
    })();
  }
  // Browser/remote continues through WebSocket and web/dispatch.rs.
  if (!isTauri) return wsClient.invoke<T>(cmd, args, diagnostic);
  if (requiresHistoryProtocol(cmd)) {
    return requireDesktopHistoryProtocol().then(() => tauriInvoke<T>("desktop_call", {
      cmd, args: args ?? {}, diagnosticRequestId: diagnostic?.requestId, diagnosticOperationId: diagnostic?.operationId,
    }));
  }
  // Desktop invokes whitelisted native commands directly and routes everything else through desktop_call.
  return DIRECT_DESKTOP_CMDS.has(cmd)
    ? tauriInvoke<T>(cmd, diagnostic ? { ...args, diagnosticRequestId: diagnostic.requestId, diagnosticOperationId: diagnostic.operationId } : args)
    : tauriInvoke<T>("desktop_call", { cmd, args: args ?? {}, diagnosticRequestId: diagnostic?.requestId, diagnosticOperationId: diagnostic?.operationId });
}

/** Listens for a backend event and returns an unsubscribe function. */
export function listen<T>(
  name: string,
  cb: (payload: T) => void,
): Promise<UnlistenFn> {
  return isTauri
    ? tauriListen<T>(name, (e) => cb(e.payload))
    : wsClient.listen<T>(name, cb);
}

/**
 * Emits on the native Tauri event bus regardless of transport mode.
 *
 * The counterpart to `listenNative`, for the events a remote window exchanges with the desktop process
 * that hosts it — its SSH tunnel, for instance — rather than with the server it displays.
 */
export async function emitNative(name: string, payload?: unknown): Promise<void> {
  const { emit } = await import("@tauri-apps/api/event");
  await emit(name, payload);
}

/**
 * Invokes a native command on the local desktop process regardless of transport mode.
 *
 * The counterpart to `listenNative`, for commands that must reach the process hosting this window rather than
 * the server it displays — split diagnostics, for instance, which belong to the local window's own log file.
 * Falls back to the regular transport when there is no local Tauri host, as in a plain browser.
 */
export function invokeNative<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const native = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  return native ? tauriInvoke<T>(cmd, args).catch((error) => { recordRequestError(cmd, error); throw error; }) : invoke<T>(cmd, args);
}

/**
 * Listens through the native Tauri event bus regardless of transport mode. Remote windows
 * (`__VLX_FORCE_BROWSER__`) route `listen` over the WebSocket relay to the remote server, which never
 * carries events emitted by the local desktop process — such as `menu://action`. Use this for those
 * local-shell events; the window keeps `__TAURI_INTERNALS__` precisely so native listening still works.
 */
export function listenNative<T>(
  name: string,
  cb: (payload: T) => void,
): Promise<UnlistenFn> {
  return tauriListen<T>(name, (e) => cb(e.payload));
}

/**
 * Like `listenNative`, but only for events the desktop process addresses to this window with
 * `emit_to(label, …)`. A bare Tauri `listen` defaults to target Any and also receives events addressed to
 * every other window, so one menu action or notification click would act in the main window and in every
 * remote window at once. Events broadcast to all windows with `emit` still arrive.
 */
export function listenOwnWindow<T>(
  name: string,
  cb: (payload: T) => void,
): Promise<UnlistenFn> {
  return tauriListen<T>(name, (e) => cb(e.payload), { target: getCurrentWindow().label });
}

// PTY output streams.

/**
 * Spawns or attaches to a PTY and streams output through `onBytes`. Desktop uses a zero-JSON Tauri binary Channel;
 * browser uses WebSocket binary frames. An already running session is attached and replayed without restart, and
 * `launch` returns null.
 */
export function spawnPty(
  args: PtySpawnArgs,
  onBytes: (bytes: Uint8Array) => void,
): Promise<PtySpawnResult> {
  const requestId = genId();
  const started = performance.now();
  let first = true;
  const receive = (bytes: Uint8Array) => {
    if (first && bytes.length) {
      first = false;
      diagnosticEvent("pty_output", { requestId, sessionId: args.sessionId, step: "first_output", bytes: bytes.length, clientDurationMs: Math.round(performance.now() - started) });
    }
    onBytes(bytes);
  };
  diagnosticEvent("pty_spawn", { requestId, sessionId: args.sessionId, status: "started" });
  const traced = { ...args, diagnosticRequestId: requestId, diagnosticOperationId: diagnosticOperation(args.sessionId) };
  let result: Promise<PtySpawnResult>;
  if (isTauri) {
    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (msg) => receive(new Uint8Array(msg));
    result = tauriInvoke<PtySpawnResult>("pty_spawn", { ...traced, onOutput: channel });
  } else {
    result = wsClient.spawnPty(traced, receive);
  }
  return result.then((value) => {
    diagnosticEvent("pty_spawn", { requestId, sessionId: args.sessionId, status: "success", attached: value.attached, clientDurationMs: Math.round(performance.now() - started) });
    return value;
  }).catch((error) => {
    recordRequestError("pty_spawn", error);
    diagnosticEvent("pty_spawn", { requestId, sessionId: args.sessionId, status: "failed", errorCode: safeError(error), clientDurationMs: Math.round(performance.now() - started) });
    throw error;
  });
}

/**
 * Streams a terminal recording in chunks through `onBytes` and resolves at EOF. Desktop uses the same binary
 * Channel as PTY output. Other clients fetch the file through a short-lived download ticket, which the backend
 * issues only to the machine's own windows (the Electron shell); remote browsers are refused.
 */
export async function readRecordingStream(
  sessionId: string,
  onBytes: (bytes: Uint8Array) => void,
): Promise<void> {
  if (isTauri) {
    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (msg) => onBytes(new Uint8Array(msg));
    return tauriInvoke("read_recording", { sessionId, onChunk: channel });
  }
  // Only the Electron shell's own window may replay; remote browsers keep the old refusal without a request.
  if (!isElectronShell) {
    throw new Error(t("transport.noReplayInBrowser"));
  }
  let ticket: string | null;
  try {
    ticket = await invoke<string | null>("recording_download_ticket", { sessionId });
  } catch {
    throw new Error(t("transport.noReplayInBrowser"));
  }
  // No recording file: an empty replay, matching the desktop command.
  if (!ticket) return;
  const res = await fetch(apiUrl(ticket));
  if (!res.ok || !res.body) throw new Error(`Failed to read recording: HTTP ${res.status}`);
  const reader = res.body.getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) return;
    if (value?.length) onBytes(value);
  }
}

/**
 * Releases this client's session use when `usePtySession` unmounts. Desktop preserves the unmount-means-kill
 * semantics; browser detaches only this client and leaves the shared process for others.
 */
export function ptyTeardown(sessionId: string): Promise<void> {
  if (isTauri) return tauriInvoke("pty_kill", { sessionId });
  wsClient.teardownPty(sessionId);
  // The Electron shell is the local desktop, so it keeps the desktop's unmount-means-kill semantics.
  if (isElectronShell) return invoke("pty_kill", { sessionId });
  return Promise.resolve();
}

// Native capabilities with browser fallbacks.

/**
 * Selects a directory. Desktop uses the native picker. Browser lacks one, so higher-level import uses the server
 * directory browser and this returns null as cancellation.
 */
export async function pickDirectory(): Promise<string | null> {
  if (isTauri) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ directory: true, multiple: false });
    return typeof picked === "string" ? picked : null;
  }
  return null;
}

/**
 * Selects a file. Desktop uses the native picker. Outside desktop there is no local file path to offer for a
 * server-side executable, so this returns null as cancellation.
 */
export async function pickFile(opts?: {
  title?: string;
  filters?: { name: string; extensions: string[] }[];
}): Promise<string | null> {
  if (isTauri) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      directory: false,
      multiple: false,
      title: opts?.title,
      filters: opts?.filters,
    });
    return typeof picked === "string" ? picked : null;
  }
  return null;
}

/**
 * Opens a path with the system default application. Desktop uses the opener plugin; browser silently ignores a
 * server-side path that has no meaning on the remote client.
 */
export async function openPath(path: string): Promise<void> {
  if (isTauri) {
    const { openPath: open } = await import("@tauri-apps/plugin-opener");
    return open(path);
  }
  /* No-op in browsers. */
}

/**
 * Reveals and selects a path in the system file manager. Desktop uses revealItemInDir; browser silently ignores
 * a server-side path that is meaningless to the remote client.
 */
export async function revealPath(path: string): Promise<void> {
  if (isTauri) {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    return revealItemInDir(path);
  }
  /* No-op in browsers. */
}

/**
 * Uploads a pasted/dropped image and returns its absolute server path for insertion into the terminal so an agent
 * can read it. Desktop calls `save_pasted_image` with number[] bytes. Browser/remote invokes it over the authenticated
 * terminal WebSocket, inheriting E2EE in paired mode. This replaces cookie-only POST /api/upload, which returned 401
 * for pairing links. Base64 is roughly three times smaller than number[] and stays within the WS frame limit.
 */
export async function uploadImage(
  bytes: Uint8Array,
  ext: string,
): Promise<string> {
  if (isTauri) {
    return tauriInvoke<string>("save_pasted_image", {
      bytes: Array.from(bytes),
      ext,
    });
  }
  return wsClient.invoke<string>("save_pasted_image", {
    bytesB64: bytesToB64(bytes),
    ext,
  });
}

/**
 * Uploads an image pasted into markdown, stores it in a sibling **`assets/` directory**, and returns a relative
 * `assets/xxx.png` path. Unlike temporary absolute-path {@link uploadImage} assets for terminal agents, document
 * images must persist and remain portable through Git or sharing, matching Typora.
 *
 * For saved documents, `save_doc_image` writes on the document host via native Tauri or authenticated WebSocket.
 * Only an explicitly missing docPath falls back to temporary-image handling.
 */
export async function uploadDocImage(
  bytes: Uint8Array,
  ext: string,
  docPath: string,
): Promise<string> {
  if (docPath) {
    if (isTauri) {
      return tauriInvoke<string>("save_doc_image", {
        docPath,
        bytes: Array.from(bytes),
        ext,
      });
    }
    return wsClient.invoke<string>("save_doc_image", {
      docPath,
      bytesB64: bytesToB64(bytes),
      ext,
    });
  }
  return uploadImage(bytes, ext);
}

/** Copies text to clipboard, falling back to execCommand when plaintext HTTP disables navigator.clipboard. */
export async function copyText(text: string, options?: { reportFailure?: boolean }): Promise<void> {
  // In local wry windows, macOS WebView's async clipboard writes are incomplete (including OSC 52 selection-copy),
  // so use a local native command for the local clipboard. Real browsers handle the standard API correctly.
  if (isTauri || isRemoteWindow) {
    try {
      // clipboard-manager write_text is ACL-gated: the main window is statically authorized, while remote windows
      // receive runtime capability in open_remote_window.
      const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
      await writeText(text);
      return;
    } catch {
      /* Fall through to browser paths if native writing fails. */
    }
  }
  // Secure contexts (desktop WebView, HTTPS, localhost) use the standard async clipboard API.
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return;
    }
  } catch {
    /* Fall through to the next fallback. */
  }
  // Plain HTTP remote access disables async clipboard. Prefer intercepting a copy event and setting its data,
  // which works in insecure contexts and does not depend on selection/focus changes. Callers run within a user
  // gesture. Keep textarea+execCommand as the final fallback.
  if (copyViaEvent(text, options?.reportFailure)) return;
  const ta = document.createElement("textarea");
  try {
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.focus();
    ta.select();
    if (document.execCommand("copy")) return;
  } catch {
    /* Preserve the legacy best-effort behavior unless the caller requests failure feedback. */
  } finally {
    ta.remove();
  }
  if (options?.reportFailure) throw new Error(t("common.copyFailed"));
}

/** Writes through a one-shot copy listener, the most reliable insecure-context path, and reports success. */
function copyViaEvent(text: string, reportFailure = false): boolean {
  let wrote = false;
  let succeeded = false;
  const handler = (e: ClipboardEvent) => {
    e.preventDefault();
    e.stopImmediatePropagation();
    try {
      e.clipboardData?.setData("text/plain", text);
      wrote = !reportFailure || e.clipboardData !== null;
    } catch {
      /* Leave failure to the next fallback. */
    }
  };
  document.addEventListener("copy", handler, true);
  try {
    succeeded = document.execCommand("copy");
  } catch {
    /* Treat an execCommand exception as failure. */
  } finally {
    document.removeEventListener("copy", handler, true);
  }
  return wrote && (!reportFailure || succeeded);
}

/**
 * Registers a callback after WS reconnect and replay complete, browser-only. The latest respawn result lets
 * usePtySession correct xterm viewport drift and reconcile fit/mirror mode with ownership or size changes.
 */
export function onPtyReattach(
  sid: string,
  cb: (res: PtySpawnResult) => void,
): (() => void) | null {
  if (!isTauri) return wsClient.onReattach(sid, cb);
  return null;
}

/**
 * Registers a browser-only callback immediately before reattachment after WS reconnect. usePtySession rearms the
 * replay gate and clears the screen before pty-spawn is resent, then onPtyReattach applies current geometry and flushes.
 */
export function onPtyReattachStart(
  sid: string,
  cb: () => void,
): (() => void) | null {
  if (!isTauri) return wsClient.onReattachStart(sid, cb);
  return null;
}

export type { UnlistenFn };

/** Refresh authoritative chat state after the remote transport reconnects. */
export function onTransportReconnect(callback: () => void): () => void {
  if (isTauri) return () => {};
  return wsClient.onConnState(state => { if (state === "online") callback(); });
}

/** Runtime evidence becomes stale as soon as the remote connection is lost. */
export function onTransportDisconnect(callback: () => void): () => void {
  if (isTauri) return () => {};
  return wsClient.onConnState(state => { if (state === "offline") callback(); });
}
