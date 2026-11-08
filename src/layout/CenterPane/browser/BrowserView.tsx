import { safeError } from "../../../ipc/diagnosticSafety";
//! Browser-tab body with a React navigation toolbar, quick-access bar, and native child-WebView placeholder.
//!
//! Web content is not in the DOM. Rust attaches a native WKWebView to the main window and overlays it
//! using the placeholder's getBoundingClientRect(): open with the first rectangle, close on unmount,
//! update bounds through ResizeObserver/window resize while visible, and toggle visibility while
//! keeping the hidden page process and media alive.
//!
//! The address bar uses local edit state while focused: Enter navigates, Escape restores, and blur
//! follows the store URL updated by browser state events.

import { useEffect, useRef, useState } from "react";
import Icons from "../../../components/Icons";
import { useT } from "../../../i18n";
import {
  browserBack,
  browserClose,
  browserForward,
  browserNavigate,
  browserOpen,
  browserReload,
  browserSetBounds,
  browserSetVisible,
  browserStop,
  onBrowserPopup,
  onBrowserState,
} from "../../../ipc/browser";
import { isClaudeArtifactUrl } from "../../../ipc/browserUrl";
import { platform } from "../../../platform";
import { useTermStore, type BrowserTab } from "../../../store/termStore";
import { useNativeViewSuspended } from "../../../hooks/nativeViewSuspend";
import { BrowserQuickAccess } from "./BrowserQuickAccess";

/** Toolbar icon button using the same inline hover handling as the TabBar dropdown. */
function ToolBtn({
  title,
  onClick,
  children,
}: {
  title: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      title={title}
      onClick={onClick}
      style={{
        display: "grid",
        placeItems: "center",
        width: 26,
        height: 26,
        flex: "none",
        border: "none",
        borderRadius: 6,
        background: "transparent",
        color: "var(--text-dim)",
        cursor: "pointer",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = "var(--bg-3, rgba(128,128,128,0.12))";
        e.currentTarget.style.color = "var(--text)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "transparent";
        e.currentTarget.style.color = "var(--text-dim)";
      }}
    >
      {children}
    </button>
  );
}

export function BrowserView({ tab, hidden }: { tab: BrowserTab; hidden: boolean }) {
  const t = useT();
  // Hide a visible native browser while DOM context menus/modals are suspended above it; otherwise
  // the native surface covers them. Already hidden tabs are unaffected.
  const suspended = useNativeViewSuspended();
  const placeholderRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  /** Whether the child WebView exists; hidden mounts defer browser_open until a valid visible rectangle. */
  const openedRef = useRef(false);
  /** Local address edit value; null follows the store URL. */
  const [editing, setEditing] = useState<string | null>(null);
  /**
   * Marker for submitted navigation awaiting a new URL. Clearing editing immediately would briefly
   * revert to the old store URL (about:blank on a new tab), making input appear lost. Keep the submitted
   * address until loading ends, then transition smoothly to the actual URL.
   */
  const pendingNavRef = useRef(false);

  /** Current placeholder rectangle in CSS pixels; return null for a hidden zero rectangle. */
  const measure = () => {
    const el = placeholderRef.current;
    if (!el) return null;
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) return null;
    return { x: r.left, y: r.top, w: r.width, h: r.height };
  };

  /** Create the child WebView once, retrying on first visibility if the hidden mount has no valid rectangle. */
  const openOnce = () => {
    if (openedRef.current) return;
    const rect = measure();
    if (!rect) return;
    openedRef.current = true;
    browserOpen(tab.id, tab.url, rect).catch((e) => {
      console.error("browser_open failed:", safeError(e));
    });
  };

  // On mount create the WebView and subscribe to state; on unmount destroy the nonpersistent page.
  useEffect(() => {
    openOnce();
    const unlisten = onBrowserState(tab.id, (s) => {
      useTermStore.getState().applyBrowserState(tab.id, s);
      // Clear local input after loading ends and the URL arrives. Electron did-stop-loading and Tauri
      // PageLoadEvent::Finished both reliably emit loading=false.
      if (pendingNavRef.current && !s.loading) {
        pendingNavRef.current = false;
        // Do not overwrite new input if the user has refocused the address bar.
        if (document.activeElement !== inputRef.current) setEditing(null);
      }
    });
    // Standalone pages keep popup navigation in the current tab; ordinary browser popups open new tabs.
    const unlistenPopup = onBrowserPopup(tab.id, ({ url }) => {
      if (isClaudeArtifactUrl(url)) {
        void platform.opener.openExternal(url).catch((e) => {
          console.warn("browser popup failed to open Claude artifact link:", safeError(e));
        });
        return;
      }
      const parent = useTermStore.getState().browserTabs[tab.id];
      if (parent?.chromeHidden) {
        void browserNavigate(tab.id, url).catch((e) => {
          console.error("browser popup navigation failed:", safeError(e));
        });
        return;
      }
      useTermStore.getState().openBrowserTab(url, {
        chromeHidden: false,
        openerTabId: tab.id,
        openerUrl: parent && parent.url !== "about:blank" ? parent.url : undefined,
      });
    });
    // Focus the address bar on a new blank tab, following browser convention.
    if (tab.url === "about:blank") {
      requestAnimationFrame(() => inputRef.current?.focus());
    }
    return () => {
      void unlisten.then((fn) => fn());
      void unlistenPopup.then((fn) => fn());
      void browserClose(tab.id);
    };
    // Mount/unmount only; subsequent URLs arrive through state events without rebuilding the WebView.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab.id]);

  // While visible, track the placeholder through ResizeObserver plus window resize for position-only changes.
  useEffect(() => {
    if (hidden) return;
    const sync = () => {
      const rect = measure();
      if (rect) void browserSetBounds(tab.id, rect);
    };
    const ro = new ResizeObserver(sync);
    if (placeholderRef.current) ro.observe(placeholderRef.current);
    window.addEventListener("resize", sync);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", sync);
    };
  }, [tab.id, hidden]);

  // Visibility requires an active tab and no suspended DOM overlay. On restoration, refresh bounds
  // because layout may have changed, and create any WebView whose hidden mount lacked a valid rectangle.
  useEffect(() => {
    const shouldShow = !hidden && !suspended;
    void browserSetVisible(tab.id, shouldShow);
    if (shouldShow) {
      requestAnimationFrame(() => {
        openOnce();
        const rect = measure();
        if (rect) void browserSetBounds(tab.id, rect);
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab.id, hidden, suspended]);

  const shownUrl = tab.url === "about:blank" ? "" : tab.url;
  const navigateTo = (rawValue: string) => {
    const value = rawValue.trim();
    if (!value) {
      // Empty input exits edit mode and restores the current URL.
      setEditing(null);
      inputRef.current?.blur();
      return;
    }
    // Quick access has no local input, so show its target immediately instead of the old URL.
    setEditing(value);
    // Keep the submitted address while awaiting state feedback; the loading listener clears it. Do not
    // set editing to null here, which would flash an empty/old address after Enter.
    pendingNavRef.current = true;
    inputRef.current?.blur();
    // If the shell rejects unsafe input such as file://, clear the marker, restore the URL, and log only.
    browserNavigate(tab.id, value).catch((e) => {
      pendingNavRef.current = false;
      setEditing(null);
      console.error("browser_navigate rejected:", safeError(e));
    });
  };

  const navigate = () => navigateTo(editing ?? "");

  /**
   * Back control for standalone pages: return to the tab that opened this one and close the current
   * child tab, fall back to its recorded URL when that tab is gone, and finally use page history.
   */
  const returnToPrevious = () => {
    const state = useTermStore.getState();
    if (tab.openerTabId && state.openTabs.includes(tab.openerTabId)) {
      state.setActiveTab(tab.openerTabId);
      state.closeTab(tab.id);
      return;
    }
    if (tab.openerUrl) {
      void browserNavigate(tab.id, tab.openerUrl);
      return;
    }
    void browserBack(tab.id);
  };

  return (
    <div
      className="browserview"
      style={{
        position: "absolute",
        inset: 0,
        zIndex: 1,
        display: hidden ? "none" : "flex",
        flexDirection: "column",
        background: "var(--bg-1)",
      }}
    >
      {tab.chromeHidden ? (
        // Standalone pages keep a compact toolbar with navigation buttons but no address bar. The bar sits
        // above the native view rectangle, so its buttons are never covered by the overlaid child WebView.
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 4,
            padding: "5px 8px",
            borderBottom: "1px solid var(--border)",
            flex: "none",
          }}
        >
          <ToolBtn title={t("browser.back")} onClick={returnToPrevious}>
            <Icons.arrowLeft size={14} />
          </ToolBtn>
          <ToolBtn title={t("browser.forward")} onClick={() => void browserForward(tab.id)}>
            <Icons.arrowRight size={14} />
          </ToolBtn>
          {tab.loading ? (
            <ToolBtn title={t("browser.stop")} onClick={() => void browserStop(tab.id)}>
              <Icons.x size={14} />
            </ToolBtn>
          ) : (
            <ToolBtn title={t("browser.reload")} onClick={() => void browserReload(tab.id)}>
              <Icons.restart size={14} />
            </ToolBtn>
          )}
        </div>
      ) : (
        <>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              padding: "5px 8px",
              borderBottom: "1px solid var(--border)",
              flex: "none",
            }}
          >
            <ToolBtn title={t("browser.back")} onClick={() => void browserBack(tab.id)}>
              <Icons.arrowLeft size={14} />
            </ToolBtn>
            <ToolBtn title={t("browser.forward")} onClick={() => void browserForward(tab.id)}>
              <Icons.arrowRight size={14} />
            </ToolBtn>
            {tab.loading ? (
              <ToolBtn title={t("browser.stop")} onClick={() => void browserStop(tab.id)}>
                <Icons.x size={14} />
              </ToolBtn>
            ) : (
              <ToolBtn title={t("browser.reload")} onClick={() => void browserReload(tab.id)}>
                <Icons.restart size={14} />
              </ToolBtn>
            )}
            <input
              ref={inputRef}
              value={editing ?? shownUrl}
              placeholder={t("browser.addressPlaceholder")}
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
              onFocus={(e) => {
                setEditing(e.currentTarget.value);
                e.currentTarget.select();
              }}
              onBlur={() => {
                // After navigation blur, retain the submitted address until loading completes.
                if (pendingNavRef.current) return;
                setEditing(null);
              }}
              onChange={(e) => setEditing(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  navigate();
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  setEditing(null);
                  inputRef.current?.blur();
                }
                // Prevent non-global keystrokes from bubbling while normal input editing continues.
                e.stopPropagation();
              }}
              style={{
                flex: 1,
                minWidth: 0,
                height: 26,
                padding: "0 10px",
                fontSize: 12.5,
                color: "var(--text)",
                background: "var(--bg-2)",
                border: "1px solid var(--border-strong)",
                borderRadius: 6,
                outline: "none",
              }}
            />
            {shownUrl.startsWith("http") && (
              <ToolBtn title={t("browser.openExternal")} onClick={() => void platform.opener.openExternal(tab.url)}>
                <Icons.external size={14} />
              </ToolBtn>
            )}
          </div>
          <BrowserQuickAccess label={t("browser.quickAccess")} onNavigate={navigateTo} />
        </>
      )}
      {/* Placeholder rectangle for the overlaid native child WebView; content is outside the DOM. */}
      <div ref={placeholderRef} style={{ flex: 1, minHeight: 0 }} />
    </div>
  );
}
