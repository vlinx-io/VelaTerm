//! Platform adapter: Tauri implementation.
//!
//! Platform implementation for Tauri desktop and browser remote access. ipc/transport.ts routes
//! invoke/listen/PTY between desktop Tauri and browser WebSocket paths, while native capabilities
//! degrade according to env. Electron uses electron.ts when env.isElectron is true.
//!
//! This is one of the few adapter files allowed to import `@tauri-apps/*` directly. It reuses
//! transport.ts and notify.ts where possible and supplies Tauri implementations only for missing
//! saveFile, openExternal, badge, clipboard.readText, and window capabilities.

import {
  copyText,
  invoke as transportInvoke,
  listen as transportListen,
  openPath as transportOpenPath,
  pickDirectory as transportPickDirectory,
  revealPath as transportRevealPath,
} from "../ipc/transport";
import {
  getEffectiveNotifyPermission,
  getNotifyPermission,
  notify as notifySend,
  requestEffectiveNotifyPermission,
  requestNotifyPermission,
} from "../notify";
import { env } from "./env";
import type {
  BadgeCapability,
  BrowserCapability,
  BrowserRect,
  BrowserStatePayload,
  ClipboardCapability,
  DialogCapability,
  NotifyCapability,
  OpenerCapability,
  Platform,
  QuitCapability,
  TransportCapability,
  UnlistenFn,
  VelaCommandCapability,
  WindowCapability,
} from "./types";

const transport: TransportCapability = {
  invoke: transportInvoke,
  listen: transportListen,
};

const dialog: DialogCapability = {
  async saveFile(opts) {
    // Browsers have no native save dialog; return null so callers can fall back to Blob download.
    if (!env.isTauri) return null;
    const { save } = await import("@tauri-apps/plugin-dialog");
    const dest = await save({
      defaultPath: opts?.defaultPath,
      title: opts?.title,
      filters: opts?.filters,
    });
    return typeof dest === "string" ? dest : null;
  },
  pickDirectory: transportPickDirectory,
};

const opener: OpenerCapability = {
  async openExternal(url) {
    if (!env.hasNativeHost) {
      // Plain browser fallback opens a new tab; system-settings schemes are not meaningful remotely.
      window.open(url, "_blank", "noopener");
      return;
    }
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  },
  openPath: transportOpenPath,
  revealPath: transportRevealPath,
};

const badge: BadgeCapability = {
  async setCount(count) {
    // Only Tauri desktop has Dock badges; remote windows and browsers skip them.
    if (!env.isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setBadgeCount(count || undefined);
  },
};

const clipboard: ClipboardCapability = {
  writeText: copyText,
  async readText() {
    // Native main and remote windows read through the local plugin, symmetric with copyText. On
    // Windows this avoids WebView2's clipboard-read permission prompt because plugin IPC bypasses web
    // permissions. The main capability and runtime remote-window capability must allow read-text.
    if (env.hasNativeHost) {
      try {
        const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
        return await readText();
      } catch {
        /* Fall through to the browser path. */
      }
    }
    try {
      if (navigator.clipboard && window.isSecureContext) {
        return await navigator.clipboard.readText();
      }
    } catch {
      /* Return an empty string when unavailable. */
    }
    return "";
  },
  async readImage() {
    if (!env.isTauri) return null;
    const { readImage } = await import("@tauri-apps/plugin-clipboard-manager");
    const image = await readImage();
    try {
      const [{ width, height }, rgba] = await Promise.all([image.size(), image.rgba()]);
      return { rgba, width, height };
    } finally {
      await image.close();
    }
  },
};

const windowCap: WindowCapability = {
  async setFocus() {
    if (!env.hasNativeHost) {
      window.focus();
      return;
    }
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().setFocus();
    } catch {
      /* Some windows lack ACL permission; fail silently. */
    }
  },
  async onFocusChanged(cb) {
    // Native hosts only; plain browsers use DOM focus/blur in the caller.
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow().onFocusChanged(({ payload }) => cb(payload));
  },
  async onNotificationClick(cb): Promise<UnlistenFn | null> {
    if (!env.hasNativeHost) return null;
    // Local backend events must use the local Tauri channel, not transport. A remote window's
    // transport sends listeners to the remote WebSocket and cannot reach its local backend.
    const { listen } = await import("@tauri-apps/api/event");
    return listen<string>("notification://click", (e) => {
      if (e.payload) cb(e.payload);
    });
  },
  async takeOpenProjectRequest() {
    if (!env.isTauri) return null;
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<string | null>("take_open_project_request");
  },
  async onOpenProjectRequest(cb) {
    if (!env.isTauri) return () => {};
    const { listen } = await import("@tauri-apps/api/event");
    return listen("open-project://request", () => cb());
  },
};

const quit: QuitCapability = {
  async onRequested(cb): Promise<UnlistenFn | null> {
    // Only the main Tauri window owns the application lifetime. Remote windows and browsers exit with their tab.
    if (!env.isTauri) return null;
    const { listen } = await import("@tauri-apps/api/event");
    return listen("app://quit-requested", () => cb());
  },
  async ack() {
    if (!env.isTauri) return;
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("quit_prompt_ack");
  },
  async confirm() {
    if (!env.isTauri) return;
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("confirm_quit");
  },
  async cancel() {
    if (!env.isTauri) return;
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("cancel_quit");
  },
};

const velaCommand: VelaCommandCapability = {
  async status() {
    if (!env.isTauri) return { installed: false, path: null, conflict: null };
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke("vela_command_status");
  },
  async install() {
    if (!env.isTauri) throw new Error("The vela command can only be installed from the desktop app.");
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke("install_vela_command");
  },
  async uninstall() {
    if (!env.isTauri) throw new Error("The vela command can only be removed from the desktop app.");
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke("uninstall_vela_command");
  },
};

const notify: NotifyCapability = {
  send: notifySend,
  getPermission: getNotifyPermission,
  requestPermission: requestNotifyPermission,
  getEffectivePermission: getEffectiveNotifyPermission,
  requestEffectivePermission: requestEffectiveNotifyPermission,
};

/**
 * Built-in browser using `invoke("browser_*")` and `listen("browser://state/{tabId}")`, moved from
 * ipc/browser.ts without behavior changes. Each call includes `cssViewportH = window.innerHeight`
 * so Rust can adjust for the macOS title bar. Rust `normalize_url` handles addresses.
 *
 * Plain browsers and remote clients never call this because the UI requires isTauri || isElectron.
 */
const withViewport = (rect: BrowserRect) => ({
  x: rect.x,
  y: rect.y,
  w: rect.w,
  h: rect.h,
  cssViewportH: window.innerHeight,
});
const browser: BrowserCapability = {
  open: (tabId, url, rect) => transportInvoke("browser_open", { tabId, url, ...withViewport(rect) }),
  navigate: (tabId, input) => transportInvoke("browser_navigate", { tabId, input }),
  back: (tabId) => transportInvoke("browser_back", { tabId }),
  forward: (tabId) => transportInvoke("browser_forward", { tabId }),
  reload: (tabId) => transportInvoke("browser_reload", { tabId }),
  stop: (tabId) => transportInvoke("browser_stop", { tabId }),
  setBounds: (tabId, rect) => transportInvoke("browser_set_bounds", { tabId, ...withViewport(rect) }),
  setVisible: (tabId, visible) => transportInvoke("browser_set_visible", { tabId, visible }),
  close: (tabId) => transportInvoke("browser_close", { tabId }),
  onState: (tabId, cb) =>
    transportListen<BrowserStatePayload>(`browser://state/${tabId}`, (payload) => cb(payload)),
};

/** Tauri platform implementation, including browser remote access fallbacks. */
export const tauriPlatform: Platform = {
  env,
  transport,
  dialog,
  opener,
  badge,
  clipboard,
  window: windowCap,
  quit,
  velaCommand,
  notify,
  browser,
};
