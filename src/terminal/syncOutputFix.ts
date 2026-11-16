//! Workaround for @xterm/xterm 6.0.0 full-viewport redraw on DEC mode 2026 exit.
//!
//! ## Problem
//! Modern CLI TUIs (such as `agy` via BubbleTea v2) query synchronized output with `DECRQM`
//! (`CSI ? 2026 $ p`) and wrap every keystroke echo and prompt redraw in `CSI ? 2026 h ... CSI ? 2026 l`.
//! In `@xterm/xterm` 6.0.0, `InputHandler.resetModePrivate` resets `decPrivateModes.synchronizedOutput`
//! to `false` on `CSI ? 2026 l` and fires `onRequestRefreshRows(undefined)`, which `CoreBrowserTerminal`
//! translates to `this.refresh(0, this.rows - 1)`. Because `RenderService.refreshRows` merges `[start, end]`
//! with `_syncOutputHandler.flush()`, passing `0 .. rows - 1` discards the tracked dirty-row range and
//! forces `DomRenderer.renderRows(0, rows - 1)` to replace DOM children across every row on every
//! keystroke, producing visible flicker on populated terminals.
//!
//! ## Approach
//! Intercept `CSI ? Pm l` through the public parser API when `2026` is present: clear
//! `synchronizedOutput`, flush `_syncOutputHandler` directly so only dirty rows buffered across earlier
//! chunks are refreshed, and let `InputHandler.parse` flush the current chunk's `_dirtyRowTracker` range
//! normally. Any companion mode numbers in compound `DECRST` sequences delegate to `resetModePrivate`.
//!
//! `CoreService.reset()` replaces `decPrivateModes` with a fresh clone on `term.reset()`, `RIS`
//! (`ESC c`), and `DECSTR` (`CSI ! p`), so the handler resolves internal service fields dynamically on
//! each sequence rather than caching references at installation time.
//!
//! The workaround is self-contained and falls back cleanly if a future `@xterm/xterm` version changes
//! internal field names.

import type { Terminal } from "@xterm/xterm";

interface XtermDecPrivateModes {
  synchronizedOutput: boolean;
}

interface XtermSyncOutputHandler {
  flush(): { start: number; end: number } | undefined;
}

interface XtermRenderService {
  _syncOutputHandler?: XtermSyncOutputHandler;
  refreshRows(start: number, end: number): void;
}

interface XtermInputHandler {
  resetModePrivate(params: { length: number; params: number[] }): boolean;
}

interface XtermCore {
  coreService?: { decPrivateModes?: XtermDecPrivateModes };
  _inputHandler?: XtermInputHandler;
  _renderService?: XtermRenderService;
}

function readXtermInternals(term: Terminal): {
  core: XtermCore;
  modes: XtermDecPrivateModes;
  inputHandler: XtermInputHandler;
} | null {
  const core = (term as unknown as { _core?: XtermCore })._core;
  const modes = core?.coreService?.decPrivateModes;
  const inputHandler = core?._inputHandler;
  if (
    !core ||
    !modes ||
    typeof modes.synchronizedOutput !== "boolean" ||
    typeof inputHandler?.resetModePrivate !== "function"
  ) {
    return null;
  }
  return { core, modes, inputHandler };
}

/** Flush buffered synchronized-output rows and clear DEC mode 2026 without a full-screen refresh. */
function exitSynchronizedOutput(core: XtermCore, modes: XtermDecPrivateModes): void {
  modes.synchronizedOutput = false;
  // Resolve _renderService at call time so installation works both before and after term.open(),
  // and remains safe for headless Terminal instances where _renderService is undefined.
  const renderService = core._renderService;
  const buffered = renderService?._syncOutputHandler?.flush();
  if (buffered && renderService) {
    renderService.refreshRows(buffered.start, buffered.end);
  }
}

/**
 * Installs the DEC private mode 2026 incremental-refresh workaround on an xterm instance and returns
 * a cleanup callback.
 */
export function installSyncOutputFix(term: Terminal): () => void {
  if (!readXtermInternals(term)) {
    return () => {};
  }

  const sub = term.parser.registerCsiHandler({ prefix: "?", final: "l" }, (params) => {
    if (!params.includes(2026)) return false;

    // Resolve live core references on each call because CoreService.reset() replaces decPrivateModes.
    const live = readXtermInternals(term);
    if (!live) return false;

    for (const p of params) {
      if (p === 2026) {
        exitSynchronizedOutput(live.core, live.modes);
      } else if (typeof p === "number") {
        // Ignore number[] subparameter arrays produced by Params.toArray(), matching InputHandler.resetModePrivate.
        live.inputHandler.resetModePrivate({ length: 1, params: [p] });
      }
    }
    return true;
  });

  return () => sub.dispose();
}
