//! Platform adapter: Electron implementation.
//!
//! Selected when `env.isElectron` detects preload's `window.__VLX_ELECTRON__` or Electron in the UA.
//!
//! Capability routing:
//! - Transport, notifications, and clipboard reuse the Tauri fallback paths in transport.ts and
//!   notify.ts. The renderer loads the local sidecar over loopback, uses wsClient, Chromium Web
//!   Notifications, and the async Clipboard API (loopback is a secure Chromium context).
//! - Dialogs, external links, badges, and focus use preload's `window.vlxNative` bridge to main.cjs
//!   IPC handlers backed by dialog, shell, app.setBadgeCount, and BrowserWindow.

import { normalizeBrowserUrl } from "../ipc/browserUrl";
import { copyText, invoke as transportInvoke, listen as transportListen } from "../ipc/transport";
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
  BrowserPopupPayload,
  BrowserRect,
  BrowserStatePayload,
  ClipboardCapability,
  DialogCapability,
  FontCatalog,
  FontCapability,
  NotifyCapability,
  OpenerCapability,
  PickFileOptions,
  Platform,
  QuitCapability,
  QuitRequest,
  SaveFileOptions,
  TransportCapability,
  UnlistenFn,
  UpdateDownloadEvent,
  UpdateHandle,
  UpdaterCapability,
  VelaCommandCapability,
  VelaCommandStatus,
  WindowCapability,
} from "./types";

/** Controlled native API exposed as `window.vlxNative` by electron/preload.cjs. */
interface VlxNativeBridge {
  fontCatalog?(): Promise<FontCatalog>;
  saveFile(opts?: SaveFileOptions): Promise<string | null>;
  pickDirectory(): Promise<string | null>;
  pickFile(opts?: PickFileOptions): Promise<string | null>;
  pathForFile?(file: File): string;
  openExternal(url: string): Promise<void>;
  openPath(path: string): Promise<void>;
  revealPath(path: string): Promise<void>;
  setBadgeCount(count: number): Promise<void>;
  setFocus(): Promise<void>;
  onFocusChanged(cb: (focused: boolean) => void): () => void;
  takeOpenProjectRequest(): Promise<string | null>;
  onOpenProjectRequest(cb: () => void): () => void;
  quit?: {
    onRequested(cb: (request?: Partial<QuitRequest>) => void): () => void;
    ack(): Promise<void>;
    confirm(): Promise<void>;
    cancel(): Promise<void>;
  };
  velaCommand?: {
    status(): Promise<VelaCommandStatus>;
    install(): Promise<VelaCommandStatus>;
    uninstall(): Promise<VelaCommandStatus>;
  };
  browser?: VlxBrowserBridge;
  updater?: {
    supported: boolean;
    check(headers?: Record<string, string>): Promise<Omit<UpdateHandle, "downloadAndInstall" | "close"> | null>;
    downloadAndInstall(onEvent?: (event: UpdateDownloadEvent) => void): Promise<void>;
    relaunch(): Promise<void>;
  };
  message?(opts: { title: string; message: string; kind?: "info" | "error" }): Promise<void>;
}

/** Built-in browser bridge exposed by preload and backed by main-process WebContentsView. */
interface VlxBrowserBridge {
  open(tabId: string, url: string, rect: BrowserRect): Promise<void>;
  navigate(tabId: string, url: string): Promise<void>;
  back(tabId: string): Promise<void>;
  forward(tabId: string): Promise<void>;
  reload(tabId: string): Promise<void>;
  stop(tabId: string): Promise<void>;
  setBounds(tabId: string, rect: BrowserRect): Promise<void>;
  setVisible(tabId: string, visible: boolean): Promise<void>;
  close(tabId: string): Promise<void>;
  /** Subscribe to browser-tab status events; callers filter by tabId. Returns an unsubscribe function. */
  onState(cb: (s: BrowserStatePayload & { tabId: string }) => void): () => void;
  /** Subscribe to new-window requests; callers filter by tabId. Returns an unsubscribe function. */
  onPopup(cb: (p: BrowserPopupPayload & { tabId: string }) => void): () => void;
}

/** Return the preload bridge, degrading capabilities safely if unexpectedly unavailable. */
function bridge(): VlxNativeBridge | undefined {
  return (window as unknown as { vlxNative?: VlxNativeBridge }).vlxNative;
}

const transport: TransportCapability = {
  invoke: transportInvoke,
  listen: transportListen,
};

const fonts: FontCapability = {
  async catalog() {
    const native = bridge();
    try {
      if (!native?.fontCatalog) throw new Error("Local font enumeration is unavailable");
      return await native.fontCatalog();
    } catch {
      return transportInvoke("bundled_font_catalog");
    }
  },
};

const dialog: DialogCapability = {
  async saveFile(opts) {
    return (await bridge()?.saveFile(opts)) ?? null;
  },
  async pickDirectory() {
    return (await bridge()?.pickDirectory()) ?? null;
  },
  async pickFile(opts) {
    return (await bridge()?.pickFile(opts)) ?? null;
  },
  async droppedFolders(transfer) {
    const pathForFile = bridge()?.pathForFile;
    if (!pathForFile) return [];
    const paths: string[] = [];
    for (const item of Array.from(transfer.items)) {
      if (item.kind !== "file" || !item.webkitGetAsEntry()?.isDirectory) continue;
      const file = item.getAsFile();
      const path = file ? pathForFile(file) : "";
      if (path) paths.push(path);
    }
    return paths;
  },
};

const opener: OpenerCapability = {
  async openExternal(url) {
    const b = bridge();
    if (b) return b.openExternal(url);
    window.open(url, "_blank", "noopener"); // Fallback.
  },
  async openPath(path) {
    await bridge()?.openPath(path);
  },
  async revealPath(path) {
    await bridge()?.revealPath(path);
  },
};

const badge: BadgeCapability = {
  async setCount(count) {
    await bridge()?.setBadgeCount(count || 0);
  },
};

const clipboard: ClipboardCapability = {
  writeText: copyText,
  async readText() {
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
    return null;
  },
};

const windowCap: WindowCapability = {
  async setFocus() {
    const b = bridge();
    if (b) return b.setFocus();
    window.focus();
  },
  async onFocusChanged(cb): Promise<UnlistenFn> {
    const b = bridge();
    if (b) return b.onFocusChanged(cb);
    // Fall back to DOM focus events.
    const onFocus = () => cb(true);
    const onBlur = () => cb(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  },
  // notify.ts handles Electron notification clicks through Web Notification onclick.
  async onNotificationClick() {
    return null;
  },
  async takeOpenProjectRequest() {
    return (await bridge()?.takeOpenProjectRequest()) ?? null;
  },
  async onOpenProjectRequest(cb) {
    return bridge()?.onOpenProjectRequest(cb) ?? (() => {});
  },
};

const quit: QuitCapability = {
  async onRequested(cb): Promise<UnlistenFn | null> {
    return (
      bridge()?.quit?.onRequested((request) =>
        cb({ remoteWindows: request?.remoteWindows ?? 0, closeWindow: request?.closeWindow ?? false }),
      ) ?? null
    );
  },
  async ack() {
    await bridge()?.quit?.ack();
  },
  async confirm() {
    await bridge()?.quit?.confirm();
  },
  async cancel() {
    await bridge()?.quit?.cancel();
  },
};

const velaCommand: VelaCommandCapability = {
  async status() {
    return (await bridge()?.velaCommand?.status()) ?? { installed: false, path: null, conflict: null };
  },
  async install() {
    const cli = bridge()?.velaCommand;
    if (!cli) throw new Error("The vela command can only be installed from the desktop app.");
    return cli.install();
  },
  async uninstall() {
    const cli = bridge()?.velaCommand;
    if (!cli) throw new Error("The vela command can only be removed from the desktop app.");
    return cli.uninstall();
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
 * Built-in browser backed by preload and a main-process WebContentsView. Normalize addresses with
 * shared `normalizeBrowserUrl` before sending them, matching Tauri's Rust rules. Callers catch invalid
 * input and log address-bar navigation failures only.
 */
const browser: BrowserCapability = {
  async open(tabId, url, rect) {
    await bridge()?.browser?.open(tabId, normalizeBrowserUrl(url), rect);
  },
  async navigate(tabId, input) {
    await bridge()?.browser?.navigate(tabId, normalizeBrowserUrl(input));
  },
  async back(tabId) {
    await bridge()?.browser?.back(tabId);
  },
  async forward(tabId) {
    await bridge()?.browser?.forward(tabId);
  },
  async reload(tabId) {
    await bridge()?.browser?.reload(tabId);
  },
  async stop(tabId) {
    await bridge()?.browser?.stop(tabId);
  },
  async setBounds(tabId, rect) {
    await bridge()?.browser?.setBounds(tabId, rect);
  },
  async setVisible(tabId, visible) {
    await bridge()?.browser?.setVisible(tabId, visible);
  },
  async close(tabId) {
    await bridge()?.browser?.close(tabId);
  },
  async onState(tabId, cb): Promise<UnlistenFn> {
    const b = bridge()?.browser;
    if (!b) return () => {};
    return b.onState((s) => {
      if (s.tabId === tabId) cb({ url: s.url, title: s.title, loading: s.loading });
    });
  },
  async onPopup(tabId, cb): Promise<UnlistenFn> {
    const b = bridge()?.browser;
    if (!b) return () => {};
    return b.onPopup((p) => {
      if (p.tabId === tabId) cb({ url: p.url });
    });
  },
};

/** AppImage self-update through the main process; other Electron installations report unsupported. */
const updater: UpdaterCapability = {
  supported: !!bridge()?.updater?.supported,
  async check(headers) {
    const native = bridge()?.updater;
    if (!native?.supported) return null;
    const found = await native.check(headers);
    if (!found) return null;
    return {
      ...found,
      downloadAndInstall: (onEvent) => native.downloadAndInstall(onEvent),
      // The main process keeps only the latest manifest, so there is nothing to release.
      close: async () => {},
    };
  },
  async relaunch() {
    await bridge()?.updater?.relaunch();
  },
  async message(text, opts) {
    const native = bridge();
    if (native?.message) await native.message({ title: opts.title, message: text, kind: opts.kind });
    else window.alert(text);
  },
};

/** Electron platform implementation. */
export const electronPlatform: Platform = {
  fonts,
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
  updater,
};
