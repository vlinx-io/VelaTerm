// Main process for the VelaTerm Electron shell (CommonJS, avoiding the ESM complexity of package.json "type":"module").
//
// Responsibilities:
//   1. Enforce a single instance.
//   2. Spawn the Rust sidecar (`velaterm --serve --local-http`) with a dynamic port and random password.
//   3. Run health checks and auto-login. The main process logs in ahead of time and injects the cookie into the
//      session, allowing the unchanged frontend to open directly on the main screen.
//   4. Restart the sidecar after crashes and clean up the child process on exit to prevent zombies.
//   5. Create the BrowserWindow (contextIsolation enabled, nodeIntegration disabled) and application menu,
//      preserving standard operations such as copy and paste.
//   6. Provide IPC for native dialog, shell, badge, and window-focus capabilities, exposed to the frontend
//      platform layer from the preload script through contextBridge.
//
// The Electron shell stays thin: all business logic remains in the shared Rust backend and frontend. This file
// handles only process orchestration specific to the Electron edition.

const { app, BrowserWindow, WebContentsView, Menu, dialog, shell, ipcMain, net, screen } =
  require("electron");
const { spawn, execFile } = require("node:child_process");
const { Readable } = require("node:stream");
const { pipeline } = require("node:stream/promises");
const crypto = require("node:crypto");
const net0 = require("node:net");
const path = require("node:path");
const fs = require("node:fs");
const windowLayout = require("./window-layout.cjs");
const windowPolicy = windowLayout.loadPolicy(app);
const diagnostic = require("./diagnostics.cjs").createDiagnostics(process.env.VLX_LOG_DIR || path.join(app.getPath("userData"), "logs"));

const LOOPBACK = "127.0.0.1";
/** Overall timeout for sidecar health checks and login, in milliseconds. */
const STARTUP_TIMEOUT_MS = 15000;

/** Maximum consecutive crash restarts; exceeding it indicates a persistent fault, shows an error, and stops the retry loop. */
const MAX_RESTARTS = 5;

/** Runtime state. */
const state = {
  /** @type {import('child_process').ChildProcess | null} */
  child: null,
  port: 0,
  password: "",
  /** Bearer token issued by the sidecar after auto-login; handed only to the main window's own origin. */
  token: "",
  /** Last few hundred characters of sidecar stderr, for the startup error dialog only. */
  stderrTail: "",
  /** Whether an intentional shutdown is in progress; do not restart the sidecar during shutdown. */
  quitting: false,
  /** Whether the user confirmed shutdown; subsequent before-quit/close events proceed immediately. */
  quitConfirmed: false,
  /** Whether a quit dialog is already open, preventing duplicates from repeated menu or close-button clicks. */
  quitConfirmationPending: false,
  /** Whether the renderer reported that its quit dialog is on screen; an unacknowledged prompt falls back to the native dialog. */
  quitPromptAcked: false,
  /** Whether the main UI started successfully; after this point, quitting requires confirmation even during sidecar recovery. */
  quitConfirmationEnabled: false,
  /** Whether readiness was reached at least once. Only an unexpected exit after readiness counts as a restartable crash; initial failures are reported by startSidecar. */
  ready: false,
  /** Consecutive crash-restart count, reset after successful readiness. */
  restarts: 0,
  /** @type {BrowserWindow | null} */
  win: null,
  /** Open account Remote windows; they close with the application, so the quit confirmation mentions them. */
  /** @type {Set<BrowserWindow>} */
  remoteWindows: new Set(),
  /** Per-window base titles and pending counts driving window titles and the summed Dock badge. */
  /** @type {Map<BrowserWindow, { baseTitle: string, count: number }>} */
  windowBadges: new Map(),
  /** Canonical absolute path from `vela <path>` that the renderer has not consumed yet. */
  pendingOpenProject: null,
  /** One-shot flag set when the background service crashed and restarted, consumed by the reloaded window. */
  pendingServiceRestartNotice: false,
};

/** Read `--open-project <path>` from Electron argv, resolving relative paths against the launching process's cwd. */
function parseOpenProjectArg(argv, cwd) {
  const i = argv.indexOf("--open-project");
  if (i < 0) return null;
  // Take the next argument only: launchers (the AppImage runtime, Chromium) may add switches of their own.
  if (!argv[i + 1] || argv[i + 1].startsWith("--")) {
    throw new Error("usage: vela <project-path>");
  }
  const resolved = fs.realpathSync(path.resolve(cwd || process.cwd(), argv[i + 1]));
  if (!fs.statSync(resolved).isDirectory()) {
    throw new Error(`Directory does not exist: ${resolved}`);
  }
  return resolved;
}

try {
  state.pendingOpenProject = parseOpenProjectArg(process.argv, process.cwd());
} catch (e) {
  diagnostic("launcher_failed");
  app.exit(2);
}

const VELA_SHIM_MARKER = "VelaTerm managed vela command";

function velaCommandDirs() {
  const home = app.getPath("home");
  const fromPath = (process.env.PATH || "").split(path.delimiter).filter(Boolean);
  const common =
    process.platform === "win32"
      ? [path.join(home, "AppData", "Roaming", "npm"), path.join(home, ".local", "bin")]
      : ["/usr/local/bin", "/opt/homebrew/bin", path.join(home, ".local", "bin"), "/opt/local/bin"];
  return [...new Set([...common, ...fromPath])];
}

function velaCommandPaths(dir) {
  return process.platform === "win32"
    ? ["vela.com", "vela.exe", "vela.bat", "vela.cmd"].map((name) => path.join(dir, name))
    : [path.join(dir, "vela")];
}

function isManagedVelaCommand(dest) {
  try {
    return fs.readFileSync(dest, "utf8").includes(VELA_SHIM_MARKER);
  } catch {
    return false;
  }
}

function velaCommandStatus() {
  for (const dir of velaCommandDirs()) {
    for (const dest of velaCommandPaths(dir)) {
      if (!fs.existsSync(dest)) continue;
      if (isManagedVelaCommand(dest)) return { installed: true, path: dest, conflict: null };
      return { installed: false, path: null, conflict: dest };
    }
  }
  return { installed: false, path: null, conflict: null };
}

/** The Electron and Tauri editions share this marker; overwrite only shims created by VelaTerm itself. */
/** Shim text that launches this installation. Inside an AppImage, execPath lives in a temporary mount that
 *  disappears on exit, so the AppImage file itself is the stable launcher. */
function velaShimContent() {
  const exe = process.env.APPIMAGE || process.execPath;
  return process.platform === "win32"
    ? `@REM ${VELA_SHIM_MARKER}\r\n@IF "%~1"=="-h" GOTO help\r\n@IF "%~1"=="--help" GOTO help\r\n@"${exe}" --open-project %*\r\n@EXIT /B %ERRORLEVEL%\r\n:help\r\n@ECHO usage: vela ^<project-path^>\r\n`
    : `#!/bin/sh\n# ${VELA_SHIM_MARKER}\ncase "\${1:-}" in -h|--help) echo 'usage: vela <project-path>'; exit 0;; esac\nexec '${exe.replaceAll("'", "'\\''")}' --open-project "$@"\n`;
}

function installVelaCommand() {
  if (!app.isPackaged) throw new Error("Install the packaged VelaTerm app before adding its shell command to PATH.");
  const content = velaShimContent();
  const before = velaCommandStatus();
  if (before.installed) {
    // Repoint a shim of ours that launches an older location, such as an AppImage that was moved or the
    // desktop edition this installation replaced.
    try {
      if (fs.readFileSync(before.path, "utf8") !== content) fs.writeFileSync(before.path, content, { mode: 0o755 });
    } catch {
      diagnostic("command_install_failed");
    }
    return before;
  }
  if (before.conflict) throw new Error(`another 'vela' command already exists at ${before.conflict}`);
  const home = app.getPath("home");
  const dirs = velaCommandDirs();
  for (const dir of dirs) {
    const underHome = path.relative(home, dir);
    const leaf = path.basename(dir);
    const safeUnderHome =
      underHome && !underHome.startsWith("..") && !path.isAbsolute(underHome) &&
      ["bin", "npm", "Scripts"].includes(leaf);
    const safeUnix =
      process.platform !== "win32" &&
      ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"].includes(dir);
    if (!safeUnderHome && !safeUnix) continue;
    try {
      fs.mkdirSync(dir, { recursive: true });
      const dest = path.join(dir, process.platform === "win32" ? "vela.cmd" : "vela");
      fs.writeFileSync(dest, content, { mode: 0o755 });
      diagnostic("command_ready");
      return { installed: true, path: dest, conflict: null };
    } catch {
      // This candidate is not writable. Try the next one without elevation or disrupting application startup.
    }
  }
  throw new Error("no writable bin directory is available; add a user-writable bin directory to PATH and try again");
}

function uninstallVelaCommand() {
  for (const dir of velaCommandDirs()) {
    const dest = path.join(dir, process.platform === "win32" ? "vela.cmd" : "vela");
    if (fs.existsSync(dest) && isManagedVelaCommand(dest)) fs.rmSync(dest);
  }
  return velaCommandStatus();
}

// ─────────────────────────── Sidecar discovery and startup ───────────────────────────

/** Obtain a free port by letting the kernel allocate and immediately release it; the sidecar performs its own bind preflight. */
function getFreePort() {
  return new Promise((resolve, reject) => {
    const srv = net0.createServer();
    srv.unref();
    srv.on("error", reject);
    srv.listen(0, LOOPBACK, () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
  });
}

/** Locate the velaterm binary. Packaged builds use resources/. Development prefers debug, matching the cargo
 *  build run by `electron:dev` and avoiding stale release binaries left by an earlier `pnpm release`; fall back
 *  to release only when debug is unavailable. */
function locateBinary() {
  const exe = process.platform === "win32" ? "velaterm.exe" : "velaterm";
  const candidates = app.isPackaged
    ? [path.join(process.resourcesPath, exe)]
    : [
        path.join(__dirname, "..", "src-tauri", "target", "debug", exe),
        path.join(__dirname, "..", "src-tauri", "target", "release", exe),
      ];
  const found = candidates.find((p) => fs.existsSync(p));
  if (!found) {
    throw new Error(
      `velaterm binary not found. In development, build the backend first: cd src-tauri && cargo build. Searched paths:\n${candidates.join("\n")}`,
    );
  }
  return found;
}

/** Window/taskbar icon. Without an explicit icon, unpackaged runs (`pnpm electron` / `electron:dev`) show the
 *  default Electron icon in the Windows taskbar, so read it from the source icons/ directory. Packaged builds
 *  read resources/icons (see extraResources in electron-builder.yml). The .app's icns controls the macOS Dock
 *  icon and macOS ignores BrowserWindow.icon, so set this only on Windows/Linux. Return undefined when no file
 *  is found, allowing Electron to fall back silently to the executable resource icon embedded by rcedit. */
function windowIcon() {
  if (process.platform === "darwin") return undefined;
  const name = process.platform === "win32" ? "icon.ico" : "128x128.png";
  const candidates = app.isPackaged
    ? [path.join(process.resourcesPath, "icons", name)]
    : [path.join(__dirname, "..", "src-tauri", "icons", name)];
  return candidates.find((p) => fs.existsSync(p));
}

/** Start a sidecar process bound to loopback with plaintext HTTP/WS and a random password.
 *
 *  **Deliberately omit `--data-dir`** so the sidecar falls back to its default "platform data directory /
 *  identifier compiled into the binary" rule (see `lib.rs::serve_data_dir`). This makes it share a data
 *  directory with the Tauri edition that has the **same identifier**: the SQLite database
 *  (projects/groups/sessions tree and agent_session_id resume anchors), recordings, archives, search index,
 *  TLS certificates, bin shims, and opencode plugins are all shared and may overwrite each other.
 *  The binary determines the identifier: the development debug binary uses io.vlinx.vlxterm and shares the
 *  Tauri development database; the release binary overridden with `--config` in release.sh uses
 *  io.vlinx.vlxterm.release and shares the Tauri release database. The sidecar creates the data directory via
 *  `create_dir_all`, so no preparation is needed here.
 *  Note: UI preferences such as theme, language, appearance, and shortcuts live in each shell WebView's
 *  localStorage. WKWebView and Chromium have separate stores, so these preferences are **not shared through the
 *  data directory**. Do not run this edition alongside the Tauri edition: the two processes will contend for
 *  the same SQLite file and trigger "database is locked," while their active PTY sessions remain independent. */
function spawnSidecar() {
  const bin = locateBinary();
  const child = spawn(
    bin,
    ["--serve", "--local-http", "--port", String(state.port)],
    {
      // Pass the password through the environment so it does not appear in the process argument list.
      // VLX_EXIT_WITH_PARENT makes the sidecar exit if this process dies without stopping it.
      env: { ...process.env, VELA_SERVE_PASSWORD: state.password, VLX_EXIT_WITH_PARENT: "1" },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  child.stdout.on("data", () => {}); // Drain CLI output; the backend persists its own safe diagnostics.
  // Never relay arbitrary child-process error text into logs. Keep only a short tail in memory, shown in the
  // startup error dialog so a failure such as a missing system library names its cause.
  state.stderrTail = "";
  child.stderr.on("data", (chunk) => {
    state.stderrTail = (state.stderrTail + chunk.toString()).slice(-600);
  });
  child.on("exit", (code, signal) => {
    diagnostic("sidecar_exit", { exitCode: code });
    state.child = null;
    // Only an unexpected exit after successful readiness counts as a crash and triggers an automatic restart.
    // Initial startup failures are reported by startSidecar's health-check timeout, preventing competing restart loops.
    if (state.ready && !state.quitting) scheduleRestart();
  });
  state.child = child;
}

/** Poll for sidecar readiness. Any connection succeeds; an unauthenticated 401 from /api/me also indicates readiness. */
async function waitForHealth(deadline) {
  const url = `http://${LOOPBACK}:${state.port}/api/me`;
  while (Date.now() < deadline) {
    try {
      const res = await net.fetch(url, { method: "GET" });
      // Any HTTP response, including 401, proves that the service is listening.
      if (res.status > 0) return true;
    } catch {
      /* Not ready yet; keep polling. */
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  return false;
}

/** Log in from the main process and keep the issued bearer token. The server no longer sets cookies; preload
 *  hands the token to the main window before any page script runs (see `vlx:session:token`), so the window
 *  loads authenticated without showing the login page. */
async function autoLogin() {
  const res = await net.fetch(`http://${LOOPBACK}:${state.port}/api/login`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ password: state.password }),
  });
  if (!res.ok) throw new Error(`Auto-login failed: HTTP ${res.status}`);
  const body = await res.json().catch(() => null);
  if (!body || typeof body.token !== "string" || !body.token) {
    throw new Error("Auto-login failed: no session token in the response");
  }
  state.token = body.token;
}

/** Complete sidecar startup: allocate a port, launch the process, await readiness, and auto-login; mark it ready on success. */
async function startSidecar() {
  state.ready = false;
  state.port = await getFreePort();
  state.password = crypto.randomBytes(24).toString("hex");
  state.token = "";
  spawnSidecar();
  const ok = await waitForHealth(Date.now() + STARTUP_TIMEOUT_MS);
  if (!ok) {
    killSidecar(); // Do not leave behind a process that failed to start.
    const detail = state.stderrTail.trim();
    throw new Error(`The local service did not start.${detail ? `\n\n${detail}` : ""}`);
  }
  await autoLogin();
  state.ready = true;
  state.restarts = 0; // Reset the crash count after successful readiness.
}

/** Restart the sidecar with backoff after a crash and reload the window; stop with an error after too many consecutive failures. */
function scheduleRestart() {
  if (state.quitting) return;
  state.ready = false;
  state.restarts += 1;
  if (state.restarts > MAX_RESTARTS) {
    dialog.showErrorBox(
      "VelaTerm sidecar keeps crashing",
      `The local service failed to start ${MAX_RESTARTS} times in a row; retries have stopped. Please check the logs and restart the app.`,
    );
    return;
  }
  const delay = Math.min(500 * state.restarts, 3000); // Linear backoff capped at 3 seconds.
  diagnostic("sidecar_restart", { retryCount: state.restarts, durationMs: delay });
  setTimeout(async () => {
    try {
      await startSidecar();
      state.pendingServiceRestartNotice = true;
      if (state.win && !state.win.isDestroyed()) {
        await state.win.loadURL(`http://${LOOPBACK}:${state.port}/`);
      }
    } catch (e) {
      diagnostic("sidecar_restart_failed");
      scheduleRestart(); // Continue retrying with backoff, bounded by MAX_RESTARTS.
    }
  }, delay);
}

/** Kill the sidecar child during shutdown to prevent zombie processes. */
function killSidecar() {
  if (state.child) {
    try {
      state.child.kill("SIGTERM");
    } catch {
      /* Already exited. */
    }
    state.child = null;
  }
}

/**
 * Centralized application-exit confirmation. The window close button, the application menu and Cmd+Q all
 * converge here, on every platform.
 */
async function requestQuitConfirmation() {
  if (state.quitConfirmed || state.quitConfirmationPending || !state.quitConfirmationEnabled) {
    return;
  }
  state.quitConfirmationPending = true;
  state.quitPromptAcked = false;
  // The renderer owns the dialog so it can offer the "save workspace" checkbox and localized copy, neither of
  // which showMessageBox supports.
  if (state.win && !state.win.isDestroyed()) {
    state.win.webContents.send("vlx:quit:requested", { remoteWindows: state.remoteWindows.size });
    // A frozen or crashed renderer never acknowledges, which would leave the application unquittable.
    setTimeout(() => {
      if (state.quitConfirmationPending && !state.quitPromptAcked && !state.quitConfirmed) {
        void nativeQuitConfirmation();
      }
    }, 5000);
    return;
  }
  await nativeQuitConfirmation();
}

/**
 * Degraded confirmation used when the renderer is unavailable. A native message box supports neither a checkbox
 * nor translated copy, so it only offers plain quit/cancel and never saves the workspace.
 */
async function nativeQuitConfirmation() {
  try {
    const result = await dialog.showMessageBox(
      state.win && !state.win.isDestroyed() ? state.win : undefined,
      {
        type: "question",
        title: "Quit VelaTerm?",
        message: "Quit VelaTerm?",
        detail:
          "Any running terminal and agent sessions will be stopped." +
          (state.remoteWindows.size > 0 ? "\nOpen remote windows will also be closed." : ""),
        buttons: ["Cancel", "Quit"],
        defaultId: 0,
        cancelId: 0,
        noLink: true,
      },
    );
    if (result.response === 1) {
      state.quitConfirmed = true;
      app.quit();
    }
  } finally {
    state.quitConfirmationPending = false;
  }
}

// ─────────────────── Built-in browser tabs (WebContentsView, architecture document §17) ───────────────────
//
// Equivalent to `src-tauri/src/browser.rs` in the Tauri edition: each browser tab is a native WebContentsView.
// `win.contentView.addChildView` overlays it on the main UI, positions it using the frontend placeholder div's
// rectangle, shows/hides it when tabs change, and destroys it when the tab closes. The window host (main process)
// must create child views, so this path bypasses the sidecar just as the Tauri path bypasses WS.
//
// Security model (preserve an equivalent for every item; compare the browser.rs module header / plan §6):
//   1. Child views use **no app preload**, enable sandbox and contextIsolation, and disable nodeIntegration,
//      preventing third-party pages from accessing vlxNative or any internal object.
//   2. The independent persistent session partition `persist:vlx-browser` isolates cookies from the main UI,
//      shares them between browser tabs, and retains them across restarts. Chromium provides this storage
//      directly, without Tauri's macOS 14+ requirement.
//   3. A complete Safari UA avoids Google's `disallowed_useragent` response for embedded WebViews.
//   4. `setWindowOpenHandler` denies native popups and forwards them to the renderer, which opens them as
//      app browser tabs matching the Tauri `on_new_window` path.
//   5. A scheme allowlist in `will-navigate` provides a final guard; the frontend already normalizes address-bar
//      input in `ipc/browserUrl.ts`.
//
// Coordinates: `WebContentsView.setBounds` uses the top-left of the BrowserWindow content area (below the native
// title bar) as its DIP origin. Frontend `getBoundingClientRect()` returns CSS pixels from the same origin, so
// the two **align naturally without Tauri's macOS title-bar Y offset** (plan §5).

/** Independent persistent session partition shared by browser tabs, equivalent to Tauri's data_store_identifier. */
const BROWSER_PARTITION = "persist:vlx-browser";

/** Complete Safari UA, matching SAFARI_UA in browser.rs. */
const BROWSER_UA =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";

/** Navigation-scheme allowlist: http, https, and about, matching browser.rs::scheme_allowed. */
function browserSchemeAllowed(rawUrl) {
  try {
    const p = new URL(rawUrl).protocol.replace(/:$/, "").toLowerCase();
    return p === "http" || p === "https" || p === "about";
  } catch {
    return false;
  }
}

/** tabId → { view, url, title, loading }. Commands look up tabId and silently no-op when absent, matching BrowserManager. */
const browserViews = new Map();

/** Send a tab's current state to the renderer, equivalent to `browser://state/{tabId}`. */
function sendBrowserState(tabId) {
  const rec = browserViews.get(tabId);
  if (!rec || !state.win || state.win.isDestroyed()) return;
  state.win.webContents.send("vlx:browser:state", {
    tabId,
    url: rec.url,
    title: rec.title,
    loading: rec.loading,
  });
}

/** Round values before calling setBounds, which requires integer pixels. */
function applyBrowserBounds(rec, rect) {
  if (!rec || !rect) return;
  rec.view.setBounds({
    x: Math.round(rect.x),
    y: Math.round(rect.y),
    width: Math.round(rect.w),
    height: Math.round(rect.h),
  });
}

/** Create a child view and attach it to the main window. Reusing a tabId only updates bounds; the frontend already normalized the URL. */
function openBrowserView(tabId, url, rect) {
  if (!state.win || state.win.isDestroyed()) return;
  const existing = browserViews.get(tabId);
  if (existing) {
    applyBrowserBounds(existing, rect);
    return;
  }
  const view = new WebContentsView({
    webPreferences: {
      // Security 1: never attach the app preload; third-party pages cannot access vlxNative or internal objects.
      partition: BROWSER_PARTITION, // Security 2: independent persistent partition.
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  const wc = view.webContents;
  wc.setUserAgent(BROWSER_UA); // Security 3.
  const rec = { view, url, title: "", loading: false };
  browserViews.set(tabId, rec);

  // Security 4: deny native popups; the renderer opens each request as an app browser tab.
  wc.setWindowOpenHandler(({ url: popupUrl }) => {
    if (browserSchemeAllowed(popupUrl) && state.win && !state.win.isDestroyed()) {
      state.win.webContents.send("vlx:browser:popup", { tabId, url: popupUrl });
    }
    return { action: "deny" };
  });
  // Security 5: enforce the scheme allowlist for in-page navigation; the frontend already normalizes address-bar input.
  wc.on("will-navigate", (e, navUrl) => {
    if (!browserSchemeAllowed(navUrl)) e.preventDefault();
  });

  // Convert state events into {url,title,loading} updates for the toolbar, equivalent to emitting browser://state.
  const syncUrl = () => {
    rec.url = wc.getURL() || rec.url;
    sendBrowserState(tabId);
  };
  wc.on("did-navigate", syncUrl);
  wc.on("did-navigate-in-page", syncUrl);
  wc.on("page-title-updated", (_e, title) => {
    // Electron provides the actual page title, which is more accurate than Tauri v1's host-based substitute.
    rec.title = title || "";
    sendBrowserState(tabId);
  });
  wc.on("did-start-loading", () => {
    rec.loading = true;
    sendBrowserState(tabId);
  });
  wc.on("did-stop-loading", () => {
    rec.loading = false;
    sendBrowserState(tabId);
  });

  state.win.contentView.addChildView(view);
  applyBrowserBounds(rec, rect);
  if (browserSchemeAllowed(url)) void wc.loadURL(url);
}

/** Address-bar navigation. The frontend already normalized the URL; the scheme guard provides another boundary. */
function navigateBrowserView(tabId, url) {
  const rec = browserViews.get(tabId);
  if (!rec) return;
  if (browserSchemeAllowed(url)) void rec.view.webContents.loadURL(url);
}

/** Back/forward navigation via navigationHistory, which replaces deprecated webContents.goBack/goForward in Electron 30+. */
function browserGoBack(tabId) {
  const rec = browserViews.get(tabId);
  if (rec && rec.view.webContents.navigationHistory.canGoBack()) {
    rec.view.webContents.navigationHistory.goBack();
  }
}
function browserGoForward(tabId) {
  const rec = browserViews.get(tabId);
  if (rec && rec.view.webContents.navigationHistory.canGoForward()) {
    rec.view.webContents.navigationHistory.goForward();
  }
}

/** Show/hide a view when switching tabs; hidden pages remain alive and continue audio/video playback. */
function setBrowserViewVisible(tabId, visible) {
  const rec = browserViews.get(tabId);
  if (rec) rec.view.setVisible(!!visible);
}

/** Close and destroy a child view, removing it before closing webContents to avoid a lingering white rectangle. */
function closeBrowserView(tabId) {
  const rec = browserViews.get(tabId);
  if (!rec) return;
  browserViews.delete(tabId);
  try {
    rec.view.setVisible(false);
    if (state.win && !state.win.isDestroyed()) {
      state.win.contentView.removeChildView(rec.view);
    }
    rec.view.webContents.close();
  } catch (e) {
    diagnostic("browser_close_failed");
  }
}

/** Remove all browser child views when the window is destroyed to prevent orphaned webContents. */
function closeAllBrowserViews() {
  for (const tabId of [...browserViews.keys()]) closeBrowserView(tabId);
}

/** Register built-in-browser IPC handlers that back vlxNative.browser in the preload bridge. */
function registerBrowserIpc() {
  ipcMain.handle("vlx:browser:open", (_e, { tabId, url, rect }) => openBrowserView(tabId, url, rect));
  ipcMain.handle("vlx:browser:navigate", (_e, { tabId, url }) => navigateBrowserView(tabId, url));
  ipcMain.handle("vlx:browser:back", (_e, { tabId }) => browserGoBack(tabId));
  ipcMain.handle("vlx:browser:forward", (_e, { tabId }) => browserGoForward(tabId));
  ipcMain.handle("vlx:browser:reload", (_e, { tabId }) => {
    browserViews.get(tabId)?.view.webContents.reload();
  });
  ipcMain.handle("vlx:browser:stop", (_e, { tabId }) => {
    browserViews.get(tabId)?.view.webContents.stop();
  });
  ipcMain.handle("vlx:browser:setBounds", (_e, { tabId, rect }) => {
    applyBrowserBounds(browserViews.get(tabId), rect);
  });
  ipcMain.handle("vlx:browser:setVisible", (_e, { tabId, visible }) =>
    setBrowserViewVisible(tabId, visible),
  );
  ipcMain.handle("vlx:browser:close", (_e, { tabId }) => closeBrowserView(tabId));
}

// ─────────────────────────── Window and menus ───────────────────────────

function formatWindowTitle(baseTitle, count) {
  return count > 0 ? `${baseTitle} (${count})` : baseTitle;
}

function refreshDockBadge() {
  if (typeof app.setBadgeCount !== "function") return;
  let total = 0;
  for (const [win, entry] of state.windowBadges) {
    if (win.isDestroyed()) {
      state.windowBadges.delete(win);
      continue;
    }
    total += entry.count;
  }
  app.setBadgeCount(total);
}

function createWindow() {
  // Use the same outer-frame policy as Tauri, excluding system bars and accounting for display scaling.
  const layout = windowLayout.windowOptions(windowPolicy, screen.getPrimaryDisplay().workArea);
  const win = new BrowserWindow({
    ...layout,
    title: "VelaTerm",
    backgroundColor: "#1e1e1e",
    icon: windowIcon(),
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      // Development signal: unpackaged runs (`pnpm electron:dev`) are dev; electron-builder releases are not.
      // Pass it to the preload script through additionalArguments. The frontend always loads bundled output,
      // making build-time import.meta.env.DEV consistently false and unable to distinguish Electron development
      // from release. This runtime signal supplies the distinction.
      additionalArguments: [`--vlx-dev=${app.isPackaged ? "0" : "1"}`, `--vlx-updater=${updaterSupported() ? "1" : "0"}`],
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false, // The preload needs require('electron'); contextIsolation still isolates the application renderer.
    },
  });
  state.win = win;
  state.windowBadges.set(win, { baseTitle: "VelaTerm", count: 0 });
  state.quitConfirmationEnabled = true;

  // Keep the native title controlled by windowBadges so index.html <title> does not overwrite it.
  win.on("page-title-updated", (event) => {
    event.preventDefault();
  });

  // Forward window focus changes to the renderer for platform.window.onFocusChanged.
  win.on("focus", () => win.webContents.send("vlx:window:focus", true));
  win.on("blur", () => win.webContents.send("vlx:window:focus", false));
  // Every platform, macOS included: hold the window open until the user confirms. Destroying it first would
  // take the renderer that owns the confirmation dialog with it, leaving only the degraded native fallback --
  // no "save workspace" checkbox, no translated copy -- and on macOS a windowless process behind it.
  win.on("close", (event) => {
    if (state.quitConfirmationEnabled && !state.quitConfirmed) {
      event.preventDefault();
      void requestQuitConfirmation();
    }
  });
  win.on("closed", () => {
    // Browser child views are destroyed with the window; remove stale Map references to prevent leaks.
    closeAllBrowserViews();
    if (state.windowBadges.delete(win)) refreshDockBadge();
    if (state.win === win) state.win = null;
  });

  guardAppNavigation(win.webContents, true);

  void win.loadURL(`http://${LOOPBACK}:${state.port}/`);
  return win;
}

/** Open a link from app content in the system browser; other schemes are dropped. */
function openLinkExternally(url) {
  try {
    const { protocol } = new URL(url);
    if (protocol === "http:" || protocol === "https:" || protocol === "mailto:") void shell.openExternal(url);
  } catch {
    /* Not a URL. */
  }
}

/**
 * Keep an application window on its own content. New-window requests (`target=_blank`, `window.open`) go to
 * the system browser instead of an Electron window that would inherit the app preload. For the main window,
 * leaving the sidecar origin is blocked too, including a file dropped outside a drop zone, which Chromium
 * would otherwise open in place of the app.
 */
function guardAppNavigation(contents, sidecarOnly) {
  contents.setWindowOpenHandler(({ url }) => {
    openLinkExternally(url);
    return { action: "deny" };
  });
  if (!sidecarOnly) return;
  contents.on("will-navigate", (event, url) => {
    let sameOrigin = false;
    try {
      sameOrigin = new URL(url).origin === `http://${LOOPBACK}:${state.port}`;
    } catch {
      /* Unparsable: treat as foreign. */
    }
    if (sameOrigin) return;
    event.preventDefault();
    openLinkExternally(url);
  });
}

/** Application menu, primarily to expose standard renderer shortcuts for copy/paste/select-all, developer tools, and window management. */
function buildMenu() {
  const isMac = process.platform === "darwin";
  const showVelaResult = async (installing) => {
    try {
      const status = installing ? installVelaCommand() : uninstallVelaCommand();
      const detail = status.installed
        ? `The 'vela' command is ready at:\n${status.path}\n\nRun: vela <project-path>`
        : status.conflict
          ? `A different 'vela' command remains at:\n${status.conflict}\n\nVelaTerm did not modify it.`
          : "The VelaTerm-managed 'vela' command was removed from PATH.";
      await dialog.showMessageBox(state.win ?? undefined, {
        type: status.conflict ? "warning" : "info",
        title: `${installing ? "Install" : "Uninstall"} 'vela' Command`,
        message: detail,
      });
    } catch (e) {
      dialog.showErrorBox(
        `${installing ? "Install" : "Uninstall"} 'vela' Command`,
        String(e && e.message ? e.message : e),
      );
    }
  };
  /** @type {import('electron').MenuItemConstructorOptions[]} */
  const template = [
    ...(isMac
      ? [{
          label: app.name,
          submenu: [
            { role: "about" },
            { type: "separator" },
            {
              label: "Install 'vela' Command in PATH…",
              enabled: app.isPackaged,
              click: () => void showVelaResult(true),
            },
            {
              label: "Uninstall 'vela' Command from PATH…",
              enabled: app.isPackaged,
              click: () => void showVelaResult(false),
            },
            { type: "separator" },
            { role: "services" },
            { type: "separator" },
            { role: "hide" },
            { role: "hideOthers" },
            { role: "unhide" },
            { type: "separator" },
            { role: "quit" },
          ],
        }]
      : []),
    { role: "fileMenu" },
    { role: "editMenu" },
    { role: "viewMenu" },
    { role: "windowMenu" },
  ];
  // Windows and Linux show the Alt-triggered menu bar drawn by the page, and Chromium handles the standard
  // editing shortcuts without a native menu, which would otherwise claim Alt and Ctrl+W/R for itself.
  Menu.setApplicationMenu(isMac ? Menu.buildFromTemplate(template) : null);
}

// ─────────────────────────── Automatic updates (Linux AppImage) ───────────────────────────
//
// Uses the desktop updater service and signing key, so the manifest, the AppImage and its minisign signature
// are the same ones the desktop edition has always installed. Only an AppImage can update itself: the
// download replaces the file the app was launched from, then the app restarts from it. Other installations
// (unpacked builds, macOS, Windows) report updates as unsupported, and the page hides the feature.

// Update service and release mirror per server region (see src/ipc/serverRegion.ts). The China mirror serves
// the same manifest and the same signed files under the same keys; only the hosts differ.
const UPDATE_ENDPOINTS = {
  global: "https://velaterm.com/api/updater",
  cn: "https://velaterm.cn/api/updater",
};
const GLOBAL_DL = "https://dl.velaterm.com/";
const CHINA_DL = "https://dl.velaterm.cn/";

/** Manifest of the release found by the last check; the renderer never supplies the URL or signature. */
let pendingUpdate = null;
/** Whether a download is running, so a second click cannot start a competing one. */
let updateDownloading = false;

function updaterSupported() {
  return process.platform === "linux" && app.isPackaged && !!process.env.APPIMAGE;
}

/** Numeric comparison of dotted versions; pre-release suffixes are ignored. */
function compareVersions(a, b) {
  const pa = String(a).split("-")[0].split(".").map((n) => Number(n) || 0);
  const pb = String(b).split("-")[0].split(".").map((n) => Number(n) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** Ask the update service for a newer release. Returns what the page shows, or null when up to date. */
async function checkForUpdate(headers, region) {
  if (!updaterSupported()) throw new Error("Automatic updates are not available for this installation.");
  const china = region === "cn";
  const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
  const current = app.getVersion();
  const endpoint = china ? UPDATE_ENDPOINTS.cn : UPDATE_ENDPOINTS.global;
  const url = `${endpoint}/linux/${arch}?current_version=${encodeURIComponent(current)}`;
  // Forward only update telemetry headers and identify this shell's updater to the service.
  const installId = headers && typeof headers["X-Install-Id"] === "string" ? headers["X-Install-Id"] : "";
  const language = headers && typeof headers["X-App-Language"] === "string" ? headers["X-App-Language"] : "";
  const requestHeaders = { "User-Agent": `velaterm-electron-updater/${current}` };
  if (installId) requestHeaders["X-Install-Id"] = installId;
  if (language) requestHeaders["X-App-Language"] = language;
  const res = await net.fetch(url, { headers: requestHeaders });
  if (res.status === 204) {
    pendingUpdate = null;
    return null;
  }
  if (!res.ok) throw new Error(`Update check failed: HTTP ${res.status}`);
  const manifest = await res.json();
  if (
    typeof manifest.version !== "string" ||
    typeof manifest.url !== "string" ||
    typeof manifest.signature !== "string" ||
    !manifest.url.startsWith("https://")
  ) {
    throw new Error("Update check failed: invalid update manifest");
  }
  if (compareVersions(manifest.version, current) <= 0) {
    pendingUpdate = null;
    return null;
  }
  // The manifest lists dl.velaterm.com; download the same signed file from the selected region's mirror.
  if (china && manifest.url.startsWith(GLOBAL_DL)) manifest.url = CHINA_DL + manifest.url.slice(GLOBAL_DL.length);
  pendingUpdate = manifest;
  return { version: manifest.version, currentVersion: current, body: manifest.notes ?? "", rawJson: manifest };
}

/** Check a downloaded file against the release signing key with the bundled backend binary. */
function verifyUpdate(file, signature) {
  return new Promise((resolve, reject) => {
    execFile(locateBinary(), ["--verify-update", file, signature], { timeout: 120000, windowsHide: true }, (error, _stdout, stderr) => {
      if (error) reject(new Error(`The update signature is not valid. ${String(stderr || "").trim()}`.trim()));
      else resolve();
    });
  });
}

/** Download the pending release next to the running AppImage, verify it, then swap it in place. */
async function downloadAndInstallUpdate(sender) {
  const manifest = pendingUpdate;
  if (!manifest) throw new Error("No update is pending.");
  if (updateDownloading) throw new Error("The update is already downloading.");
  updateDownloading = true;
  const target = process.env.APPIMAGE;
  // Same directory, so the final rename stays on one file system and replaces the file atomically.
  const temp = `${target}.download`;
  const send = (ev) => {
    if (!sender.isDestroyed()) sender.send("vlx:updater:progress", ev);
  };
  try {
    const res = await net.fetch(manifest.url);
    if (!res.ok || !res.body) throw new Error(`Download failed: HTTP ${res.status}`);
    send({ event: "Started", data: { contentLength: Number(res.headers.get("content-length")) || undefined } });
    const body = Readable.fromWeb(res.body);
    // Report progress a few times a second rather than once per network chunk.
    let unreported = 0;
    let lastReport = 0;
    body.on("data", (chunk) => {
      unreported += chunk.length;
      if (Date.now() - lastReport < 200) return;
      lastReport = Date.now();
      send({ event: "Progress", data: { chunkLength: unreported } });
      unreported = 0;
    });
    await pipeline(body, fs.createWriteStream(temp, { mode: 0o755 }));
    if (unreported) send({ event: "Progress", data: { chunkLength: unreported } });
    send({ event: "Finished" });
    await verifyUpdate(temp, manifest.signature);
    fs.chmodSync(temp, 0o755);
    // A running AppImage keeps its mounted image open, so replacing the file does not disturb this process.
    fs.renameSync(temp, target);
    diagnostic("update_installed");
  } catch (e) {
    fs.rmSync(temp, { force: true });
    diagnostic("update_failed");
    if (e && e.code === "EACCES") {
      throw new Error(`VelaTerm cannot replace ${target} because its folder is not writable.`);
    }
    throw e;
  } finally {
    updateDownloading = false;
  }
}

function registerUpdaterIpc() {
  ipcMain.handle("vlx:updater:check", (_e, headers, region) => checkForUpdate(headers, region));
  ipcMain.handle("vlx:updater:downloadAndInstall", (event) => downloadAndInstallUpdate(event.sender));
  ipcMain.handle("vlx:updater:relaunch", () => {
    // The user asked for the restart, so it skips the quit confirmation.
    state.quitConfirmed = true;
    app.relaunch(updaterSupported() ? { execPath: process.env.APPIMAGE, args: [] } : undefined);
    app.quit();
  });
  ipcMain.handle("vlx:dialog:message", async (_e, opts) => {
    await dialog.showMessageBox(state.win && !state.win.isDestroyed() ? state.win : undefined, {
      type: opts?.kind === "error" ? "error" : "info",
      title: String(opts?.title ?? ""),
      message: String(opts?.message ?? ""),
    });
  });
}

// ─────────────────────────── Native-capability IPC (preload bridge backend) ───────────────────────────

function registerNativeIpc() {
  // Synchronous so preload can seed the token before the page's scripts run. Only the main window, while it
  // shows the sidecar's own origin, receives it; the account Remote window shares the preload but must not.
  ipcMain.on("vlx:session:token", (event) => {
    let sameOrigin = false;
    try {
      sameOrigin = new URL(event.senderFrame?.url ?? "").origin === `http://${LOOPBACK}:${state.port}`;
    } catch {
      /* Unparsable URL: treat as a foreign origin. */
    }
    event.returnValue = event.sender === state.win?.webContents && sameOrigin ? state.token : "";
  });
  // Quit-confirmation handshake; see requestQuitConfirmation.
  ipcMain.handle("vlx:quit:ack", () => {
    state.quitPromptAcked = true;
  });
  ipcMain.handle("vlx:quit:confirm", () => {
    state.quitConfirmationPending = false;
    state.quitConfirmed = true;
    app.quit();
  });
  ipcMain.handle("vlx:quit:cancel", () => {
    state.quitConfirmationPending = false;
    state.quitPromptAcked = false;
  });
  ipcMain.handle("vlx:velaCommand:status", () => velaCommandStatus());
  ipcMain.handle("vlx:fonts:catalog", () => new Promise((resolve, reject) => {
    execFile(locateBinary(), ["--font-catalog"], { timeout: 15000, maxBuffer: 4 * 1024 * 1024, windowsHide: true }, (error, stdout) => {
      if (error) return reject(new Error("Local font enumeration failed"));
      try { resolve(JSON.parse(stdout)); }
      catch { reject(new Error("Invalid local font catalog")); }
    });
  }));
  ipcMain.handle("vlx:velaCommand:install", () => installVelaCommand());
  ipcMain.handle("vlx:velaCommand:uninstall", () => uninstallVelaCommand());
  ipcMain.handle("vlx:project:takeOpenRequest", () => {
    const value = state.pendingOpenProject;
    state.pendingOpenProject = null;
    return value;
  });
  ipcMain.handle("vlx:service:takeRestartNotice", (event) => {
    if (event.sender !== state.win?.webContents) return false;
    const pending = state.pendingServiceRestartNotice;
    state.pendingServiceRestartNotice = false;
    return pending;
  });
  ipcMain.handle("vlx:account:remote", async (event, url) => {
    if (event.sender !== state.win?.webContents) throw new Error("Invalid Remote window request");
    let target;
    try { target = new URL(String(url)); } catch { throw new Error("Invalid Remote URL"); }
    // The URL comes from this app's own authenticated sidecar; only the scheme is constrained here.
    if (target.protocol !== "https:" && target.protocol !== "http:") throw new Error("Invalid Remote URL");
    // The page comes from another machine, so it runs as a plain browser: no app preload (and with it no
    // vlxNative), a sandbox, and a user agent without the Electron token so the frontend does not take it
    // for this app's own window.
    const display = screen.getDisplayMatching(state.win.getBounds());
    const layout = windowLayout.windowOptions(windowPolicy, display.workArea);
    const win = new BrowserWindow({...layout,icon:windowIcon(),
      title:"VelaTerm · Remote", webPreferences:{contextIsolation:true,nodeIntegration:false,sandbox:true}});
    state.remoteWindows.add(win);
    win.on("closed", () => state.remoteWindows.delete(win));
    win.setMenuBarVisibility(false);
    win.webContents.setUserAgent(win.webContents.getUserAgent().replace(/\s*Electron\/\S+/, ""));
    guardAppNavigation(win.webContents, false);
    await win.loadURL(target.href);
  });
  ipcMain.handle("vlx:dialog:saveFile", async (_e, opts) => {
    const res = await dialog.showSaveDialog(state.win ?? undefined, {
      defaultPath: opts?.defaultPath,
      title: opts?.title,
      filters: opts?.filters,
    });
    return res.canceled ? null : (res.filePath ?? null);
  });
  ipcMain.handle("vlx:dialog:pickDirectory", async () => {
    const res = await dialog.showOpenDialog(state.win ?? undefined, {
      properties: ["openDirectory"],
    });
    return res.canceled || res.filePaths.length === 0 ? null : res.filePaths[0];
  });
  ipcMain.handle("vlx:dialog:pickFile", async (_e, opts) => {
    const res = await dialog.showOpenDialog(state.win ?? undefined, {
      properties: ["openFile"],
      title: opts?.title,
      filters: opts?.filters,
    });
    return res.canceled || res.filePaths.length === 0 ? null : res.filePaths[0];
  });
  ipcMain.handle("vlx:shell:openExternal", async (_e, url) => {
    await shell.openExternal(String(url));
  });
  ipcMain.handle("vlx:shell:openPath", async (_e, p) => {
    await shell.openPath(String(p));
  });
  ipcMain.handle("vlx:shell:revealPath", async (_e, p) => {
    shell.showItemInFolder(String(p));
  });
  ipcMain.handle("vlx:badge:setCount", (event, count) => {
    const normalized = Math.max(0, Math.floor(Number(count) || 0));
    const win = BrowserWindow.fromWebContents(event.sender);
    if (win && !win.isDestroyed()) {
      const entry = state.windowBadges.get(win) ?? { baseTitle: win.getTitle() || "VelaTerm", count: 0 };
      entry.count = normalized;
      state.windowBadges.set(win, entry);
      win.setTitle(formatWindowTitle(entry.baseTitle, normalized));
    }
    refreshDockBadge();
  });
  ipcMain.handle("vlx:window:setFocus", () => {
    if (state.win && !state.win.isDestroyed()) {
      if (state.win.isMinimized()) state.win.restore();
      state.win.show();
      state.win.focus();
    }
  });
}

// ─────────────────────────── Lifecycle ───────────────────────────

// Single-instance lock: a second instance exits immediately and brings the existing window forward.
if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on("second-instance", (_event, commandLine, workingDirectory) => {
    try {
      const requested = parseOpenProjectArg(commandLine, workingDirectory);
      if (requested) {
        state.pendingOpenProject = requested;
        state.win?.webContents.send("vlx:project:openRequest");
      }
    } catch (e) {
      dialog.showErrorBox("Unable to open project", String(e && e.message ? e.message : e));
    }
    if (state.win) {
      if (state.win.isMinimized()) state.win.restore();
      state.win.show();
      state.win.focus();
    }
  });

  // Windows taskbar identity: without this, taskbar icons and notifications belong to the generic electron.exe
  // identity. Match appId in electron-builder.yml so grouping, pinning, and icons work correctly. This is a no-op
  // on other platforms.
  app.setAppUserModelId("io.vlinx.vlxterm.electron");

  app.whenReady().then(async () => {
    // Like VS Code, macOS installs only after an explicit menu/settings action. Packaged Windows/Linux builds
    // retain the installer/system-package style automatic setup.
    if (process.platform !== "darwin" && app.isPackaged) {
      try {
        installVelaCommand();
      } catch (e) {
        diagnostic("command_install_failed");
      }
    }
    registerNativeIpc();
    registerBrowserIpc();
    registerUpdaterIpc();
    buildMenu();
    try {
      await startSidecar();
    } catch (e) {
      diagnostic("startup_failed");
      dialog.showErrorBox("VelaTerm failed to start", String(e && e.message ? e.message : e));
      app.quit();
      return;
    }
    createWindow();

    app.on("activate", () => {
      if (BrowserWindow.getAllWindows().length === 0) createWindow();
    });
  });

  app.on("window-all-closed", () => {
    if (process.platform !== "darwin") app.quit();
  });

  app.on("before-quit", (event) => {
    // Internal exits before the main UI exists, such as a second instance or startup failure, do not prompt the
    // user. Every normal exit requires confirmation.
    if (state.quitConfirmationEnabled && !state.quitConfirmed) {
      event.preventDefault();
      void requestQuitConfirmation();
      return;
    }
    state.quitting = true;
    killSidecar();
  });
  // Last-resort process cleanup: attempt to terminate the sidecar even after a crash or forced termination.
  process.on("exit", killSidecar);
}
