import ReactDOM from "react-dom/client";
import "@xterm/xterm/css/xterm.css";
// Bundle JetBrains Mono locally for offline use instead of loading it from Google Fonts.
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/600.css";
import "@fontsource/jetbrains-mono/700.css";
import "@fontsource/jetbrains-mono/400-italic.css";
// Embedded symbol fallback (U+23E9–23FA, about 1.5 KB) supplies media glyphs missing from JetBrains Mono.
import "./styles/fonts.css";
import "./styles/index.css";
import App from "./App";
import { initI18n, t } from "./i18n";
import { recordRequestError } from "./ipc/reqLog";
import MobileApp from "./mobile/MobileApp";
import { isMobileView } from "./mobile/detect";
import { LoginGate } from "./remote/LoginGate";
import { ErrorBoundary } from "./components/ErrorBoundary";

// The main UI uses only application context menus. WKWebView otherwise shows native Reload/AutoFill
// items in uncovered areas, mixing unrelated actions and styles. Cancel only the browser default in
// capture phase; propagation continues to terminal, tab, file-tree, and editor React handlers.
document.addEventListener("contextmenu", (event) => event.preventDefault(), { capture: true });

// macOS text services (auto-capitalization, autocorrect, spell-check marking) apply to every
// editable element in WKWebView unless the page opts out per element. Project names, paths, and
// commands must arrive verbatim, so opt out globally on focus instead of repeating the three
// attributes in each component (FormModal had them; CreateProjectModal and others did not).
// focusin fires before the first keystroke and costs nothing on the terminal render hot path,
// unlike a subtree MutationObserver. Attributes already set in JSX win: only absent ones are added.
document.addEventListener("focusin", (event) => {
  const el = event.target;
  if (!(el instanceof HTMLElement)) return;
  const editable =
    el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el.isContentEditable;
  if (!editable) return;
  if (!el.hasAttribute("autocapitalize")) el.setAttribute("autocapitalize", "none");
  if (!el.hasAttribute("autocorrect")) el.setAttribute("autocorrect", "off");
  if (!el.hasAttribute("spellcheck")) el.setAttribute("spellcheck", "false");
});

// Global fallback: render unhandled errors outside React directly into a DOM panel.
installGlobalErrorOverlay();

// ─── Terminal glyph-width correction for desktop WKWebView ─────────────────
// WKWebView canvas measureText may miss the requested font: 13 px Menlo `W` measured 14.30 px while
// the DOM rendered 7.84 px. xterm 5.5 prefers OffscreenCanvas in CharSizeService and treats the
// difference from DOM glyph width as letter spacing, spreading every character. Hiding OffscreenCanvas
// makes xterm fall back to consistent DOM measurement. Chrome remote access does not need this, and
// no project or xterm component other than CharSizeService uses OffscreenCanvas here.
if ("__TAURI_INTERNALS__" in window) {
  (window as unknown as Record<string, unknown>).OffscreenCanvas = undefined;
}

// Dictionaries other than English load on demand, so wait for the active one before the first paint;
// otherwise a non-English user briefly sees English and then a full re-render. `finally` rather than
// `then`: if the dictionary fails to load there is nothing to wait for, and English is a usable UI.
//
// Do not use React.StrictMode: development double mounting would duplicate xterm instances and PTY
// spawns. Select the mobile layout once at startup for non-Tauri narrow touch screens or overrides.
void initI18n().finally(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <ErrorBoundary>
      <LoginGate>{isMobileView() ? <MobileApp /> : <App />}</LoginGate>
    </ErrorBoundary>,
  );
});

function installGlobalErrorOverlay() {
  let overlayEl: HTMLElement | null = null;

  function show(title: string, detail: string) {
    if (overlayEl) {
      // Append to the existing panel.
      const pre = overlayEl.querySelector("pre");
      if (pre) pre.textContent += "\n\n" + title + "\n" + detail;
      return;
    }
    overlayEl = document.createElement("div");
    Object.assign(overlayEl.style, {
      position: "fixed",
      inset: "0",
      zIndex: "99998",
      background: "rgba(0,0,0,0.85)",
      color: "#e0e0e0",
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      padding: "24px",
    });
    // Assign text through DOM properties rather than interpolated HTML; error messages use textContent.
    overlayEl.innerHTML = `
      <div style="max-width:560px;width:100%;background:#242424;border:1px solid #333;border-radius:12px;padding:28px">
        <div data-overlay-title style="font-size:15px;font-weight:700;margin-bottom:6px"></div>
        <div data-overlay-desc style="font-size:12px;color:#999;margin-bottom:14px"></div>
        <pre style="margin:0 0 16px;padding:10px 12px;background:#1e1e1e;border:1px solid #333;border-radius:6px;font-size:11px;line-height:1.5;color:#ff8080;overflow:auto;max-height:240px;white-space:pre-wrap;word-break:break-all"></pre>
        <button style="padding:8px 20px;border:none;border-radius:6px;background:#4a9eff;color:#fff;font-size:13px;font-weight:600;cursor:pointer"></button>
      </div>`;
    overlayEl.querySelector("[data-overlay-title]")!.textContent = t("err.uncaughtTitle");
    overlayEl.querySelector("[data-overlay-desc]")!.textContent = t("err.uncaughtDesc");
    overlayEl.querySelector("button")!.textContent = t("err.reload");
    const pre = overlayEl.querySelector("pre")!;
    pre.textContent = title + "\n" + detail;
    overlayEl.querySelector("button")!.onclick = () => location.reload();
    document.body.appendChild(overlayEl);
  }

  window.addEventListener("error", (e) => {
    const message = e.error?.message ?? e.message ?? "Unknown error";
    const detail = e.error?.stack ?? `at ${e.filename}:${e.lineno}:${e.colno}`;
    if (isBenignDisposedCallbackError(message, detail)) {
      recordRequestError("uncaught:benign", `${message}\n${detail}`);
      return;
    }
    show(message, detail);
  });

  window.addEventListener("unhandledrejection", (e) => {
    const err = e.reason;
    // Connection, disconnect, and handshake-auth failures are handled by banners, reconnect, or login.
    // Suppress the global fallback and the browser's default unhandled-rejection report.
    if (err && typeof err === "object" && (err as { expected?: boolean }).expected) {
      e.preventDefault();
      return;
    }
    if (err instanceof Error) {
      if (isBenignDisposedCallbackError(err.message, err.stack ?? "")) {
        recordRequestError("uncaught:benign", `${err.message}\n${err.stack ?? ""}`);
        return;
      }
      show(err.message, err.stack ?? "");
    } else {
      show("Unhandled Promise Rejection", String(err));
    }
  });
}

/** Known-harmless failures from callbacks that outlive the object they belong to.
 *
 * xterm's Viewport constructor schedules `setTimeout(() => this.syncScrollArea())` and `reset()` schedules
 * the same call through requestAnimationFrame, and neither is cancelled on dispose. A terminal opened and
 * disposed within one macrotask (session tree rebuilds during remote connect do exactly that) therefore
 * still runs the callback, which reads `RenderService.dimensions` after the MutableDisposable holding the
 * renderer was cleared. The throw comes from a timer, so no try/catch or error boundary can reach it, and
 * nothing is actually broken: the terminal it belonged to is already gone.
 *
 * Match narrowly rather than suppressing uncaught errors in general, so real crashes still reach the
 * overlay. Engines word the message differently (WebKit prints the failing expression, Chromium prints the
 * missing property), hence the check against both message and stack. */
function isBenignDisposedCallbackError(message: string, stack: string): boolean {
  if (!/syncScrollArea/.test(stack) && !/syncScrollArea/.test(message)) return false;
  return /_renderer|dimensions/.test(message);
}
