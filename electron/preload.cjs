// Preload bridge for the VelaTerm Electron shell (CommonJS).
//
// Security: contextIsolation is enabled. Expose **only a narrow, controlled API** here; never hand the full
// ipcRenderer object to the page. The renderer receives two values used by the frontend platform adapter:
//   1. window.__VLX_ELECTRON__: an environment flag used by src/platform/env.ts to derive isElectron;
//   2. window.vlxNative: a controlled wrapper around native capabilities that delegates to individual IPC
//      handlers in the main process (see main.cjs).

const { contextBridge, ipcRenderer } = require("electron");

// Development detection: the main process injects this through webPreferences.additionalArguments as the
// inverse of app.isPackaged. Unpackaged `pnpm electron:dev` runs are development builds; electron-builder
// releases are not. src/platform/env.ts uses this value to derive isDev so the title bar can show its DEV
// badge in Electron development runs as well.
const isElectronDev = process.argv.includes("--vlx-dev=1");

contextBridge.exposeInMainWorld("__VLX_ELECTRON__", true);
contextBridge.exposeInMainWorld("__VLX_ELECTRON_DEV__", isElectronDev);

contextBridge.exposeInMainWorld("vlxNative", {
  /** Open the system Save As dialog; return null when canceled. */
  saveFile: (opts) => ipcRenderer.invoke("vlx:dialog:saveFile", opts),
  /** Open the system directory picker; return null when canceled. */
  pickDirectory: () => ipcRenderer.invoke("vlx:dialog:pickDirectory"),
  /** Open a URL with the system default application (external browser or system-settings scheme). */
  openExternal: (url) => ipcRenderer.invoke("vlx:shell:openExternal", url),
  /** Open a path with the system default application (file manager). */
  openPath: (p) => ipcRenderer.invoke("vlx:shell:openPath", p),
  /** Reveal and select a path in the system file manager. */
  revealPath: (p) => ipcRenderer.invoke("vlx:shell:revealPath", p),
  /** Set the Dock/taskbar badge count; zero clears it. */
  setBadgeCount: (count) => ipcRenderer.invoke("vlx:badge:setCount", count),
  /** Bring the current window to the foreground. */
  setFocus: () => ipcRenderer.invoke("vlx:window:setFocus"),
  /**
   * Subscribe to window focus changes and return an unsubscribe function.
   * The main process sends 'vlx:window:focus' (true/false) when win receives focus/blur events.
   */
  onFocusChanged: (cb) => {
    const handler = (_e, focused) => cb(!!focused);
    ipcRenderer.on("vlx:window:focus", handler);
    return () => ipcRenderer.removeListener("vlx:window:focus", handler);
  },
  /** Read/subscribe to external `vela <path>` launch requests. Events only wake the listener; take atomically consumes the path. */
  takeOpenProjectRequest: () => ipcRenderer.invoke("vlx:project:takeOpenRequest"),
  onOpenProjectRequest: (cb) => {
    const handler = () => cb();
    ipcRenderer.on("vlx:project:openRequest", handler);
    return () => ipcRenderer.removeListener("vlx:project:openRequest", handler);
  },
  /**
   * Application-exit confirmation handshake. The main process sends 'vlx:quit:requested' instead of showing a
   * native dialog, so the renderer can present a localized dialog carrying the "save workspace" checkbox.
   * `ack` stops the main-process watchdog; `confirm`/`cancel` report the user's decision.
   */
  quit: {
    onRequested: (cb) => {
      const handler = () => cb();
      ipcRenderer.on("vlx:quit:requested", handler);
      return () => ipcRenderer.removeListener("vlx:quit:requested", handler);
    },
    ack: () => ipcRenderer.invoke("vlx:quit:ack"),
    confirm: () => ipcRenderer.invoke("vlx:quit:confirm"),
    cancel: () => ipcRenderer.invoke("vlx:quit:cancel"),
  },

  /** Explicitly install/uninstall the `vela` shell command, as in VS Code (shared by macOS settings and the native menu). */
  velaCommand: {
    status: () => ipcRenderer.invoke("vlx:velaCommand:status"),
    install: () => ipcRenderer.invoke("vlx:velaCommand:install"),
    uninstall: () => ipcRenderer.invoke("vlx:velaCommand:uninstall"),
  },

  /**
   * Built-in browser tabs (architecture document §17): delegate to the WebContentsView manager in the main
   * process (see main.cjs). Expose only this controlled method set; never pass ipcRenderer or webContents
   * handles to the page. Callers in platform/electron.ts must normalize each URL through ipc/browserUrl.ts first.
   */
  browser: {
    open: (tabId, url, rect) => ipcRenderer.invoke("vlx:browser:open", { tabId, url, rect }),
    navigate: (tabId, url) => ipcRenderer.invoke("vlx:browser:navigate", { tabId, url }),
    back: (tabId) => ipcRenderer.invoke("vlx:browser:back", { tabId }),
    forward: (tabId) => ipcRenderer.invoke("vlx:browser:forward", { tabId }),
    reload: (tabId) => ipcRenderer.invoke("vlx:browser:reload", { tabId }),
    stop: (tabId) => ipcRenderer.invoke("vlx:browser:stop", { tabId }),
    setBounds: (tabId, rect) => ipcRenderer.invoke("vlx:browser:setBounds", { tabId, rect }),
    setVisible: (tabId, visible) => ipcRenderer.invoke("vlx:browser:setVisible", { tabId, visible }),
    close: (tabId) => ipcRenderer.invoke("vlx:browser:close", { tabId }),
    /** Subscribe to state events from all browser tabs and return an unsubscribe function; callers filter by tabId. */
    onState: (cb) => {
      const handler = (_e, payload) => cb(payload);
      ipcRenderer.on("vlx:browser:state", handler);
      return () => ipcRenderer.removeListener("vlx:browser:state", handler);
    },
  },
});
