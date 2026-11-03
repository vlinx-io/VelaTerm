//! Platform adapter types and interface contracts.
//!
//! Consolidate every frontend call that directly touches a platform (desktop shell or native OS capability) into
//! environment-independent capability interfaces. Application code depends only on `Platform` as defined here;
//! `index.ts` selects the Tauri or Electron implementation at runtime.
//!
//! Rule (see overall plan §2 / §3.1): application code must never import `@tauri-apps/*` or `electron` directly;
//! all access goes through this adapter. ESLint no-restricted-imports enforces the rule, exempting platform/ and
//! other adapter modules.

import type { UnlistenFn } from "../ipc/transport";

export type { UnlistenFn };

/** Runtime shell kind. `browser` means remote browser access without a desktop shell, using WS. */
export type PlatformKind = "tauri" | "electron" | "browser";

/** Native display-host fonts plus text fonts shipped with the frontend. */
export interface FontCatalog {
  families: string[];
  systemFontsAvailable: boolean;
}

export interface FontCapability {
  /** Query the local display device; plain browsers receive only bundled fonts. */
  catalog(): Promise<FontCatalog>;
}

/** Unified environment view; env.ts is the detailed single source of truth. */
export interface PlatformEnv {
  /** Shell kind. */
  kind: PlatformKind;
  /** Whether this runs inside the Tauri WebView (desktop Tauri edition). */
  isTauri: boolean;
  /** Whether this runs inside the Electron shell (desktop Electron edition). */
  isElectron: boolean;
  /** Whether access comes through a browser without a desktop shell, using WS; includes remote-connection windows. */
  isBrowser: boolean;
  /**
   * Whether this is a remote-connection window: a local wry window presents as a browser and uses WS
   * (isTauri=false) while retaining `__TAURI_INTERNALS__` and access to local native capabilities. This is the
   * native-capable subset of isBrowser.
   */
  isRemoteWindow: boolean;
  /** Whether local native capabilities are available (main window, remote-connection window, or Electron). */
  hasNativeHost: boolean;
  /**
   * Whether this is a development run, used by the DEV badge and similar UI. Unified across shells:
   * - Tauri/browser: build-time `import.meta.env.DEV` is true while the Vite development server runs.
   * - Electron: the frontend always loads bundled output (DEV is always false), so the main process injects
   *   `__VLX_ELECTRON_DEV__` (= `!app.isPackaged`) through preload as a runtime signal.
   */
  isDev: boolean;
  /** Whether the OS is macOS; used only for UI text, never key handling, which always uses metaKey||ctrlKey. */
  isMac: boolean;
}

/** Save As dialog options. */
export interface SaveFileOptions {
  /** Default filename or path. */
  defaultPath?: string;
  /** Dialog title, where supported. */
  title?: string;
  /** File-type filters. */
  filters?: { name: string; extensions: string[] }[];
}

/** File picker options. */
export interface PickFileOptions {
  /** Dialog title, where supported. */
  title?: string;
  /** File-type filters. */
  filters?: { name: string; extensions: string[] }[];
}

/** File-dialog capability. */
export interface DialogCapability {
  /** Open the system Save As dialog; return the selected absolute path, or null on cancel/outside desktop. */
  saveFile(opts?: SaveFileOptions): Promise<string | null>;
  /** Open the system directory picker; return null on cancel or in browsers, which use the server-directory modal. */
  pickDirectory(): Promise<string | null>;
  /** Open the system file picker; return the selected absolute path, or null on cancel or without a native picker. */
  pickFile(opts?: PickFileOptions): Promise<string | null>;
  /**
   * Absolute paths of the folders in an external file drop; files are left out. Call synchronously from the
   * `drop` handler, because the DataTransfer is emptied once the handler returns. Returns an empty list where
   * local paths are unavailable, as in browsers and remote-connection windows.
   */
  droppedFolders(transfer: DataTransfer): Promise<string[]>;
}

/** External-opening capability using default applications or the file manager. */
export interface OpenerCapability {
  /** Open a URL with the system default application, such as an external browser or settings scheme. */
  openExternal(url: string): Promise<void>;
  /** Open a path with the system default application/file manager; silently no-op in browsers. */
  openPath(path: string): Promise<void>;
  /** Reveal and select a path in the system file manager; silently no-op in browsers. */
  revealPath(path: string): Promise<void>;
}

/** Dock/taskbar badge capability. */
export interface BadgeCapability {
  /** Set the badge count; zero/undefined clears it. Browsers have no badge, so they silently no-op. */
  setCount(count: number | undefined): Promise<void>;
}

/** Clipboard capability. */
export interface ClipboardCapability {
  /** Write text with layered fallbacks on plaintext HTTP; reportFailure rejects if all paths fail. */
  writeText(text: string, options?: { reportFailure?: boolean }): Promise<void>;
  /** Read clipboard text; return an empty string when unavailable. */
  readText(): Promise<string>;
  /** Read RGBA pixels from a clipboard image; currently supported only in the main Tauri window, otherwise null. */
  readImage(): Promise<{ rgba: Uint8Array; width: number; height: number } | null>;
}

/** Communication capability (invoke/event listening); see ipc/transport.ts for transport routing. */
export interface TransportCapability {
  /** Invoke a backend command. */
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  /** Listen for a backend event and return an unsubscribe function. */
  listen<T>(name: string, cb: (payload: T) => void): Promise<UnlistenFn>;
}

/** Current-window capability. */
export interface WindowCapability {
  /** Bring the current window to the foreground; fail silently because some windows may lack ACL permission. */
  setFocus(): Promise<void>;
  /**
   * Listen for foreground/background focus changes and return an unsubscribe function. Native hosts only;
   * plain browser/mobile callers use DOM focus/blur instead.
   */
  onFocusChanged(cb: (focused: boolean) => void): Promise<UnlistenFn>;
  /**
   * Listen for the local backend's system-notification-click event carrying a session ID. Native hosts return an
   * unsubscribe function; plain browsers use Web Notification onclick and return null. This must use the local
   * event channel rather than transport, because a remote window's transport sends to remote WS, not the local host.
   */
  onNotificationClick(cb: (sessionId: string) => void): Promise<UnlistenFn | null>;
  /** Take and clear an unconsumed `vela <path>` launch request; return null outside desktop environments. */
  takeOpenProjectRequest(): Promise<string | null>;
  /** Fire when a second `vela` process wakes this window; read the payload through takeOpenProjectRequest. */
  onOpenProjectRequest(cb: () => void): Promise<UnlistenFn>;
}

/** Details the shell attaches to an exit request. */
export interface QuitRequest {
  /** Open windows connected to another machine or workspace; they close together with the application. */
  remoteWindows: number;
  /**
   * Only the main window closes and the application keeps running, as in VS Code: on macOS it stays in the
   * Dock, elsewhere remote windows are still open. Local sessions stop either way.
   */
  closeWindow: boolean;
}

/**
 * Application-exit confirmation capability.
 *
 * Both desktop shells intercept a user-triggered exit and hand the decision to the frontend, because a native
 * message dialog supports neither the "save workspace" checkbox nor translated copy. The flow is: shell fires
 * `onRequested` → the frontend calls `ack()` right away so the shell's watchdog stops → the user decides →
 * the frontend calls `confirm()` or `cancel()`. Browsers have no application exit, so `onRequested` returns null
 * and the remaining methods no-op.
 */
export interface QuitCapability {
  /** Subscribe to exit requests; returns an unsubscribe function, or null where there is no application exit. */
  onRequested(cb: (request: QuitRequest) => void): Promise<UnlistenFn | null>;
  /** Report that the dialog is on screen, so the shell does not fall back to its native dialog. */
  ack(): Promise<void>;
  /** Approve the exit. Callers must finish persisting workspace state before calling this. */
  confirm(): Promise<void>;
  /** Dismiss the request, leaving the application running and able to prompt again later. */
  cancel(): Promise<void>;
}

/** Resolution status of the `vela` command on PATH. */
export interface VelaCommandStatus {
  installed: boolean;
  path: string | null;
  /** Existing same-named command on PATH not managed by VelaTerm; installation will not overwrite it. */
  conflict: string | null;
}

/** VS Code-style explicit shell-command installation capability. */
export interface VelaCommandCapability {
  status(): Promise<VelaCommandStatus>;
  install(): Promise<VelaCommandStatus>;
  uninstall(): Promise<VelaCommandStatus>;
}

/** System notification permission state, matching notify.ts. */
export type NotifyPermission = "granted" | "denied" | "default" | "unsupported";

/** Built-in browser-tab placeholder rectangle in CSS pixels, originating at the main UI viewport's top-left. */
export interface BrowserRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** `browser://state` event payload: a partial patch carrying URL, document title, and loading state. */
export interface BrowserStatePayload {
  url?: string;
  title?: string;
  loading?: boolean;
}

/** `browser://popup` event payload: a new-window request that becomes a new app browser tab. */
export interface BrowserPopupPayload {
  url: string;
}

/**
 * Built-in browser-tab capability (architecture document §17).
 *
 * The built-in browser is a native child view created by the window host process and overlaid on BrowserView's
 * placeholder div; it is **not an iframe in the page**. Each shell implements it separately:
 * - **Tauri**: Rust creates a WKWebView through `Window::add_child`; this capability delegates to
 *   `invoke("browser_*")` + `listen("browser://state/{tabId}")`, adding the macOS title-bar Y offset (see browser.rs).
 * - **Electron**: the main process creates a `WebContentsView` in `electron/main.cjs`; this capability delegates
 *   to preload bridge `window.vlxNative.browser`, whose coordinates naturally align to the content area's top-left.
 * - **Browser/remote**: no built-in browser. UI gating hides the entry point, so methods are never called.
 *
 * Persisting a node's last URL through `set_browser_url` is **outside this capability**. That database write goes
 * through transport/WS (sidecar); see `setBrowserUrl` in `ipc/browser.ts`.
 */
export interface BrowserCapability {
  /** Create a child view over the placeholder; repeated calls for one tabId are idempotent and only update bounds. */
  open(tabId: string, url: string, rect: BrowserRect): Promise<void>;
  /** Navigate from address-bar input after normalization: add HTTPS to domains, search Google, reject invalid schemes. */
  navigate(tabId: string, input: string): Promise<void>;
  back(tabId: string): Promise<void>;
  forward(tabId: string): Promise<void>;
  reload(tabId: string): Promise<void>;
  stop(tabId: string): Promise<void>;
  /** Synchronize placeholder position/size when ResizeObserver or the window reports a change. */
  setBounds(tabId: string, rect: BrowserRect): Promise<void>;
  /** Show/hide on tab switches; hidden page processes remain alive and continue audio/video. */
  setVisible(tabId: string, visible: boolean): Promise<void>;
  /** Close and destroy the child view; silently idempotent when the tab is already closed. */
  close(tabId: string): Promise<void>;
  /** Listen for one browser tab's state events and return an unsubscribe function. */
  onState(tabId: string, cb: (s: BrowserStatePayload) => void): Promise<UnlistenFn>;
  /**
   * Listen for one browser tab's new-window requests (`window.open` / `target=_blank`) and return an
   * unsubscribe function. The native layer denies the popup; the frontend opens it as an app browser tab.
   */
  onPopup(tabId: string, cb: (popup: BrowserPopupPayload) => void): Promise<UnlistenFn>;
}

/** Download progress, in the event shape of the Tauri updater plugin. */
export type UpdateDownloadEvent =
  | { event: "Started"; data: { contentLength?: number } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };

/** A newer release found by `UpdaterCapability.check`. */
export interface UpdateHandle {
  version: string;
  currentVersion: string;
  /** Full changelog Markdown from the manifest. */
  body?: string;
  /** The raw manifest, carrying extensions such as `notes_i18n`. */
  rawJson: Record<string, unknown>;
  /** Download, verify and install the release; the app then needs a restart (Windows exits by itself). */
  downloadAndInstall(onEvent?: (event: UpdateDownloadEvent) => void): Promise<void>;
  /** Release the handle when the prompt is dismissed or replaced. */
  close(): Promise<void>;
}

/**
 * Self-update of the installed desktop app. Tauri uses its updater plugin; the Electron shell updates an
 * AppImage installation from the same update service. Browsers and other Electron installations report
 * `supported: false`, and the update entry points stay inactive.
 */
export interface UpdaterCapability {
  supported: boolean;
  /** Ask the update service for a newer release, sending the given request headers; null when up to date. */
  check(headers?: Record<string, string>): Promise<UpdateHandle | null>;
  /** Restart into the installed update. */
  relaunch(): Promise<void>;
  /** Show a native message box for update results. */
  message(text: string, opts: { title: string; kind?: "info" | "error" }): Promise<void>;
}

/** System-notification capability with local native, remote-window relay, and Web Notification paths; see notify.ts. */
export interface NotifyCapability {
  /** Send a system notification; silently skip without permission or on failure. Callers decide whether it should appear. */
  send(
    sessionId: string | null,
    title: string,
    body: string,
    sound?: boolean,
  ): Promise<void>;
  /** Query current system notification permission without prompting. */
  getPermission(): Promise<NotifyPermission>;
  /** Request system notification permission. */
  requestPermission(): Promise<NotifyPermission>;
  /** Query effective permission, preferring native UNUserNotificationCenter authorization on macOS. */
  getEffectivePermission(): Promise<NotifyPermission>;
  /** Request effective permission through the native authorization dialog on macOS. */
  requestEffectivePermission(): Promise<NotifyPermission>;
}

/** Unified platform adapter; application code depends only on these capabilities. */
export interface Platform {
  fonts: FontCapability;
  /** Environment view. */
  env: PlatformEnv;
  /** Communication (invoke/listen). */
  transport: TransportCapability;
  /** File dialogs. */
  dialog: DialogCapability;
  /** External opening / file manager. */
  opener: OpenerCapability;
  /** Dock/taskbar badge. */
  badge: BadgeCapability;
  /** Clipboard. */
  clipboard: ClipboardCapability;
  /** Current window. */
  window: WindowCapability;
  /** Application-exit confirmation handshake. */
  quit: QuitCapability;
  /** Install/uninstall the `vela` shell command. */
  velaCommand: VelaCommandCapability;
  /** System notifications. */
  notify: NotifyCapability;
  /** Built-in browser tabs, exclusive to desktop shells: Tauri add_child / Electron WebContentsView. */
  browser: BrowserCapability;
  /** Self-update of the installed app. */
  updater: UpdaterCapability;
}
