//! Shared application-menu commands.
//!
//! Two entry points raise the same commands: the native macOS menu, whose clicks arrive from the
//! backend as `menu://action`, and the Alt-triggered menu bar drawn on Windows and Linux. Keeping the
//! handlers in one place stops the two paths from drifting apart.

import type { MenuAction } from "../../ipc/events";
import { checkForUpdates } from "../../ipc/updater";
import { useTermStore } from "../../store/termStore";

/** Run one application-menu command, whichever menu raised it. */
export function runMenuAction(action: MenuAction): void {
  const s = useTermStore.getState();
  switch (action) {
    case "settings":
      s.setSettingsOpen(true);
      break;
    case "check-update":
      void checkForUpdates({ manual: true });
      break;
    case "share":
      s.setShareOpen(true);
      break;
    case "clear-badges":
      s.clearAllBadges();
      break;
    // Split directly instead of replaying a synthetic key event. The native menu's accelerator is
    // fixed while the shortcut itself is rebindable, so a replayed event can match no binding at all.
    case "split-right":
    case "split-down":
      if (s.activeSessionId) {
        void s.splitNew(
          action === "split-down" ? "vertical" : "horizontal",
          "menu",
        );
      }
      break;
  }
}
