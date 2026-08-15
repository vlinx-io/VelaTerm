//! Transport adapter exposing backend calls, event listeners, and PTY output through environment-neutral APIs.
//!
//! Automatically selects one implementation:
//! - **Desktop Tauri WebView** uses `@tauri-apps/api` invoke/Channel/listen and plugins.
//! - **Browser remote access** uses a single WebSocket (see `wsClient.ts`).
//!
//! Higher layers depend only on this module, allowing the same React code to run on desktop and browser.

import { Channel, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";

import { t } from "../i18n";
import { recordRequestError } from "./reqLog";
import { wsClient, bytesToB64 } from "./wsClient";
import type { SessionKind } from "../types";

/** Whether this is a Tauri WebView rather than browser remote access. Remote-connection windows are physically
 *  Tauri WebViews but must use WebSocket transport, so their init script sets __VLX_FORCE_BROWSER__. */
export const isTauri =
  typeof window !== "undefined" &&
  "__TAURI_INTERNALS__" in window &&
  !(window as any).__VLX_FORCE_BROWSER__;

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

// PTY launch arguments and results.

export interface PtySpawnArgs {
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
  "probe_remote_fingerprint",
  "url_trust_fingerprint",
  "open_devtools",
  "ssh_probe_host",
  "ssh_trust_host",
  "ssh_connect",
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
  return invokeInner<T>(cmd, args).catch((err) => {
    // User-canceled clone is handled control flow, not a global error; cleanup failures return another error.
    const message = String(err);
    if (!(cmd === "clone_project" && (message === "CLONE_CANCELLED" || message === "Error: CLONE_CANCELLED"))) {
      recordRequestError(cmd, err);
    }
    throw err;
  });
}

function invokeInner<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Browser/remote continues through WebSocket and web/dispatch.rs.
  if (!isTauri) return wsClient.invoke<T>(cmd, args);
  // Desktop invokes whitelisted native commands directly and routes everything else through desktop_call.
  return DIRECT_DESKTOP_CMDS.has(cmd)
    ? tauriInvoke<T>(cmd, args)
    : tauriInvoke<T>("desktop_call", { cmd, args: args ?? {} });
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
  if (isTauri) {
    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (msg) => onBytes(new Uint8Array(msg));
    return tauriInvoke<PtySpawnResult>("pty_spawn", { ...args, onOutput: channel });
  }
  return wsClient.spawnPty(args, onBytes);
}

/**
 * Streams a terminal recording in chunks through `onBytes` and resolves at EOF. Desktop uses the same binary
 * Channel as PTY output. Browser playback is currently unsupported and rejects immediately.
 */
export function readRecordingStream(
  sessionId: string,
  onBytes: (bytes: Uint8Array) => void,
): Promise<void> {
  if (isTauri) {
    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (msg) => onBytes(new Uint8Array(msg));
    return tauriInvoke("read_recording", { sessionId, onChunk: channel });
  }
  return Promise.reject(new Error(t("transport.noReplayInBrowser")));
}

/**
 * Releases this client's session use when `usePtySession` unmounts. Desktop preserves the unmount-means-kill
 * semantics; browser detaches only this client and leaves the shared process for others.
 */
export function ptyTeardown(sessionId: string): Promise<void> {
  if (isTauri) return tauriInvoke("pty_kill", { sessionId });
  wsClient.teardownPty(sessionId);
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
export async function copyText(text: string): Promise<void> {
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
  if (copyViaEvent(text)) return;
  try {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.focus();
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
  } catch {
    /* Silently ignore when no clipboard path is available. */
  }
}

/** Writes through a one-shot copy listener, the most reliable insecure-context path, and reports success. */
function copyViaEvent(text: string): boolean {
  let wrote = false;
  const handler = (e: ClipboardEvent) => {
    e.preventDefault();
    e.stopImmediatePropagation();
    try {
      e.clipboardData?.setData("text/plain", text);
      wrote = true;
    } catch {
      /* Leave failure to the next fallback. */
    }
  };
  document.addEventListener("copy", handler, true);
  try {
    document.execCommand("copy");
  } catch {
    /* Treat an execCommand exception as failure. */
  } finally {
    document.removeEventListener("copy", handler, true);
  }
  return wrote;
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
