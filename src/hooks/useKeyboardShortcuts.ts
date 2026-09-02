//! Global keyboard shortcuts.
//! - Cmd/Ctrl+1–9 focuses the numbered open tab and is fixed to tab positions.
//! - Cmd/Ctrl++/-/0 changes or resets terminal font size and is fixed to those semantics.
//! - Settings may remap temporary-terminal creation, desktop browser tabs, pane/tab closure, both
//!   split directions, terminal/global search, and document save. Defaults live in shortcutRegistry
//!   and overrides in vlx-settings. Remapping changes only triggers, not contextual behavior.
//!
//! Platform defaults use Cmd on macOS and Ctrl+Alt on Windows/Linux (see DEFAULT_BINDINGS). Bare Ctrl
//! letters are critical shell keys, Ctrl+Shift may be consumed by IMEs, and Alt is terminal Meta.
//! usePtySession blocks these Ctrl+Alt combinations from xterm so it cannot emit stray Escape bytes.
//! This listener runs in document capture phase before xterm/editors. The shortcut recorder runs even
//! earlier on window capture and stops propagation while recording.
//! Plain-browser clients (URL remote access) use the Ctrl+Alt defaults on every OS: browser-reserved
//! combos such as ⌘D/⌘T/⌘W/⌘F never reach the page, so the Cmd bindings would be dead keys.

/** Custom document-save event name; detail is the document-tab ID and DocView listens for it. */
export const DOC_SAVE_EVENT = "vlx:doc-save";
/** Custom PDF-export event name; detail is the document-tab ID and DocView listens for it. */
export const DOC_EXPORT_PDF_EVENT = "vlx:doc-export-pdf";

import { useEffect } from "react";
import { isTauri } from "../ipc/transport";
import { env } from "../platform";
import { useTermStore } from "../store/termStore";
import {
  DEFAULT_BINDINGS,
  hasMod,
  matchCombo,
  type ShortcutAction,
} from "./shortcutRegistry";

export function useKeyboardShortcuts() {
  useEffect(() => {
    // Zero-based Cmd+1–9 tab index, with layout/IME-independent e.code fallback; otherwise -1.
    const digitIndex = (e: KeyboardEvent) => {
      if (e.key >= "1" && e.key <= "9") return Number(e.key) - 1;
      if (/^Digit[1-9]$/.test(e.code)) return Number(e.code.slice(5)) - 1;
      return -1;
    };

    const handler = (e: KeyboardEvent) => {
      // Only this client's own mod key counts: on macOS that is Cmd, and Ctrl stays terminal input.
      if (!hasMod(e)) return;

      // ── Fixed shortcut 1: Cmd+1–9 selects the nth tab ──
      const tabIdx = digitIndex(e);
      if (tabIdx >= 0) {
        const { openTabs, setActiveTab } = useTermStore.getState();
        if (tabIdx < openTabs.length) {
          e.preventDefault();
          setActiveTab(openTabs[tabIdx]);
        }
        return;
      }

      // ── Fixed shortcut 2: Cmd++/-/0 changes or resets terminal font size ──
      // Apply only to active session tabs; document/browser tabs retain their own handling.
      {
        const isPlus = e.key === "+" || e.key === "=" || e.code === "Equal";
        const isMinus = e.key === "-" || e.key === "_" || e.code === "Minus";
        const isZero = e.key === "0" || e.code === "Digit0";
        if (isPlus || isMinus || isZero) {
          const { activeSessionId, activeTabId, docTabs, browserTabs, termFontSize, setTermFontSize } =
            useTermStore.getState();
          const onSessionTab =
            !!activeSessionId &&
            !(activeTabId && (docTabs[activeTabId] || browserTabs[activeTabId]));
          if (onSessionTab) {
            e.preventDefault();
            if (isZero) setTermFontSize(13);
            else setTermFontSize(termFontSize + (isPlus ? 1 : -1));
          }
          return;
        }
      }

      // ── Remappable actions: exactly match the user override or default binding ──
      const { shortcutOverrides } = useTermStore.getState();
      const sc = (a: ShortcutAction) => shortcutOverrides[a] || DEFAULT_BINDINGS[a];

      if (matchCombo(e, sc("openProject"))) {
        e.preventDefault();
        void useTermStore.getState().importProject();
        return;
      }

      if (matchCombo(e, sc("newTab"))) {
        e.preventDefault();
        useTermStore.getState().newScratchTab();
        return;
      }

      // New browser tabs are available only in Tauri/Electron desktop shells.
      if (matchCombo(e, sc("newBrowserTab"))) {
        if (isTauri || env.isElectron) {
          e.preventDefault();
          useTermStore.getState().openBrowserTab();
        }
        return;
      }

      if (matchCombo(e, sc("closePane"))) {
        const { activeTabId, docTabs, browserTabs, requestCloseDocTab, closeTab, activeSessionId, closePane } =
          useTermStore.getState();
        // Document tabs close directly when clean or route dirty state to DocView confirmation.
        if (activeTabId && docTabs[activeTabId]) {
          e.preventDefault();
          requestCloseDocTab(activeTabId);
          return;
        }
        // Browser tabs close directly; unmounting destroys the child WebView and no unsaved state exists.
        if (activeTabId && browserTabs[activeTabId]) {
          e.preventDefault();
          closeTab(activeTabId);
          return;
        }
        if (activeSessionId) {
          e.preventDefault();
          // Close the current pane, or the tab when it is the final pane.
          closePane();
        }
        return;
      }

      if (matchCombo(e, sc("saveDoc"))) {
        // Intercept save only for an active document tab.
        const { activeTabId, docTabs } = useTermStore.getState();
        if (activeTabId && docTabs[activeTabId]) {
          e.preventDefault();
          window.dispatchEvent(new CustomEvent(DOC_SAVE_EVENT, { detail: activeTabId }));
        }
        return;
      }

      // Check vertical split before horizontal split because custom bindings may overlap by modifiers.
      if (matchCombo(e, sc("splitDown"))) {
        const { activeSessionId, splitNew } = useTermStore.getState();
        if (activeSessionId) {
          e.preventDefault();
          void splitNew("vertical", "shortcut");
        }
        return;
      }
      if (matchCombo(e, sc("splitRight"))) {
        const { activeSessionId, splitNew } = useTermStore.getState();
        if (activeSessionId) {
          e.preventDefault();
          void splitNew("horizontal", "shortcut");
        }
        return;
      }

      // Check global search before terminal search for the same possible modifier overlap.
      if (matchCombo(e, sc("globalSearch"))) {
        e.preventDefault();
        useTermStore.getState().setGlobalSearchOpen(true);
        return;
      }
      if (matchCombo(e, sc("search"))) {
        const { activeTabId, docTabs, activeSessionId, openSearch } = useTermStore.getState();
        // DocView owns document-tab search, so do not open terminal search here.
        if (activeTabId && docTabs[activeTabId]) return;
        if (activeSessionId) {
          e.preventDefault();
          openSearch();
        }
      }
    };

    document.addEventListener("keydown", handler, true);
    return () => document.removeEventListener("keydown", handler, true);
  }, []);
}
