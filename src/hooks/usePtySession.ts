//! Bind an xterm instance to the full lifecycle of a backend PTY session.
//!
//! Flow: create xterm, fit its grid, install exit/status listeners, spawn the PTY with a binary output
//! channel, buffer bytes until spawn resolves and establishes the correct grid, then replay them.
//! Execute the launch command, write onData back, synchronize size through ResizeObserver, and clean
//! up/kill on unmount. Buffer-before-grid ordering is essential for a correct mirrored first screen.
//!
//! ## Size-mode state machine for shared-session mirroring and explicit takeover
//!
//! A PTY has one grid and its byte stream cannot be reflowed independently per client. One owner
//! therefore controls size while every other client mirrors it faithfully:
//!
//! - **fit mode** (owner): fit xterm to its container and send ResizeObserver changes to the PTY.
//! - **mirror mode** (viewer): pin the PTY grid and CSS-scale it to fit, never sending resize; typing remains allowed.
//!
//! New sessions start in fit. Attaching to another owner enters mirror before replay. Resized signals
//! select fit for this owner, mirror for another, or let a visible unowned client claim with a normal
//! resize. “Fit this window” performs takeover; the backend broadcasts ownership to all clients.

import { useCallback, useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import type { WebglAddon } from "@xterm/addon-webgl";
import type { CanvasAddon } from "@xterm/addon-canvas";
import { loadCanvasCtor, loadWebglCtor, peekWebglCtor } from "../term/rendererAddons";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { ClipboardAddon, type IClipboardProvider } from "@xterm/addon-clipboard";
import { ImageAddon } from "@xterm/addon-image";
import type { UnlistenFn } from "../ipc/transport";
import { terminalLinkHandler, webLinkActivate } from "../terminal/openLink";

import { appAltKeyCodes, IS_PLAIN_BROWSER } from "./shortcutRegistry";

import {
  ptyRedraw,
  ptyResize,
  ptySpawn,
  ptyTeardownSession,
  ptyWrite,
  wasKilledLocally,
} from "../ipc/commands";
import {
  copyText,
  getClientSource,
  isMac,
  isRemoteWindow,
  isTauri,
  onPtyReattach,
  onPtyReattachStart,
} from "../ipc/transport";
import { onPtyExit, onPtyKilled, onPtyStatus } from "../ipc/events";
import { env } from "../platform";
import { dlog } from "../debug";
import { t } from "../i18n";
import { useTermStore } from "../store/termStore";
import {
  noteOsc52Copy,
  registerRedraw,
  registerSearch,
  registerTerminal,
  unregisterRedraw,
  unregisterSearch,
  unregisterTerminal,
} from "../terminal/registry";
import { installQueryReplyGuard } from "../terminal/queryReplyGuard";
import {
  discardTerminalOutput,
  flushTerminalOutput,
  noteUserInput,
  writeTerminalOutput,
} from "../terminal/outputScheduler";
import { installWebkitImeFix, isWebkitEngine } from "../terminal/imeWebkitFix";
import { detectAgentScreen, readScreenTail } from "../terminal/screenDetect";
import { remapGrokDayCanvasToWhite } from "../terminal/grokBgRemap";
import { fontStack, resolveTheme, XTERM_THEME } from "../theme";
import type { AgentKind, Session } from "../types";

/** Size mode: fit owns the PTY grid; mirror pins the grid and observes it. */
export type SizeMode = "fit" | "mirror";

// ─── Monospace font readiness gate ───────────────────────────────────
// xterm caches measured glyph widths. Creating it before asynchronous JetBrains Mono loading records
// fallback widths, leaving spaced-out text after the real font appears until a resize. Wait once at
// module scope, with a timeout that permits fallback rendering, and share readiness across terminals.
let fontsReady = false;
const fontsReadyPromise: Promise<void> = (async () => {
  try {
    if (!document.fonts.check('16px "JetBrains Mono"')) {
      await Promise.race([
        document.fonts.load('16px "JetBrains Mono"').then(() => undefined),
        new Promise<void>((resolve) => setTimeout(resolve, 1500)),
      ]);
    }
  } catch {
    /* Font API unavailable/failed: continue with the fallback font. */
  }
  fontsReady = true;
})();

/** Subscribe to monospace-font readiness and rerender once it becomes true. */
function useFontsReady(): boolean {
  const [ready, setReady] = useState(fontsReady);
  useEffect(() => {
    if (ready) return;
    let alive = true;
    void fontsReadyPromise.then(() => {
      if (alive) setReady(true);
    });
    return () => {
      alive = false;
    };
  }, [ready]);
  return ready;
}

// OSC 52 clipboard writer: TUIs such as Claude use OSC 52 for select-to-copy while mouse reporting
// is active. xterm discards it by default, so route writes through the cross-environment copyText.
//
// readText always returns empty to prevent remote programs reading private clipboard data. Per-session
// providers allow writes only and record copied character counts for contextual-menu feedback.
function makeOsc52Provider(sessionId: string): IClipboardProvider {
  return {
    writeText: (_selection, text) => {
      // Record feedback only where clipboard writes are reliable. Plain-HTTP remote browsers reject
      // asynchronous writes outside user gestures, so claiming success there would be misleading.
      if (isTauri || isRemoteWindow || env.isElectron) noteOsc52Copy(sessionId, text.length);
      return copyText(text);
    },
    readText: () => "",
  };
}

export function usePtySession(session: Session, cwd?: string, hidden?: boolean) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  // Expose the terminal so reveal logic can recalculate the viewport scroll area.
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  // Active Canvas/WebGL renderer addon; DOM mode uses null. Dispose on rebuild/unmount.
  const rendererRef = useRef<CanvasAddon | WebglAddon | null>(null);
  // Timestamp display:none entry so reveal can rebuild a long-hidden, reclaimed WebGL context.
  const hiddenSinceRef = useRef<number | null>(null);
  // Keep current hidden state in a ref so output-scheduler closures do not use stale props.
  const hiddenRef = useRef<boolean>(!!hidden);
  // Agent-launch placeholder remains true until Claude begins drawing its first screen.
  const [starting, setStarting] = useState(false);
  const setRuntime = useTermStore((s) => s.setRuntime);
  // Subscribe to terminal font family/size for live application to this xterm.
  const termFontFamily = useTermStore((s) => s.termFontFamily);
  const termFontSize = useTermStore((s) => s.termFontSize);
  // Window focus plus active-session state drives terminal FocusIn/FocusOut forwarding.
  const windowFocused = useTermStore((s) => s.windowFocused);
  // Subscribe only to whether this session is active, avoiding rerenders of every terminal on switches.
  const isActiveSession = useTermStore((s) => s.activeSessionId === session.id);
  /** Last forwarded focus state, used to deduplicate PTY writes. */
  const lastFocusSentRef = useRef<boolean | null>(null);

  // ─── Size-mode state machine ───────────────────────────
  // UI state for the mirror overlay plus cross-effect refs used by reveal logic.
  const [sizeMode, setSizeMode] = useState<SizeMode>("fit");
  const [ptyDims, setPtyDims] = useState<{ cols: number; rows: number } | null>(null);
  const modeRef = useRef<SizeMode>("fit");
  /** Most recently known size owner from spawn/resized; null means unowned. */
  const ownerRef = useRef<string | null>(null);
  /** Mirror-scale recalculation exposed to reveal logic. */
  const applyScaleRef = useRef<(() => void) | null>(null);
  /** Claim an unowned size with fit plus normal first-wins resize. */
  const claimRef = useRef<(() => void) | null>(null);
  /** Explicitly take ownership with fit plus takeover resize. */
  const takeoverRef = useRef<(() => void) | null>(null);
  /** Stable explicit-takeover entry used by the mirror overlay. */
  const takeoverSize = useCallback(() => takeoverRef.current?.(), []);

  // ─── DEC 1004 focus reporting ──────────────────────────────────
  // macOS WKWebView does not blur xterm when the window backgrounds, preventing TUIs from sending
  // notifications. Explicitly forward FocusIn/FocusOut from window focus, active session, and visibility
  // only when focus reporting is enabled, deduplicating writes.
  useEffect(() => {
    const term = termRef.current;
    if (!term || !term.modes.sendFocusMode) return;
    // Only the size owner reports focus. Every client watching a shared session has its own window, so
    // without this gate one TUI receives several contradictory FocusIn/FocusOut streams and blinks between
    // focused and unfocused as the other windows come and go. Same rule, same reason as terminal-query
    // replies being answered by the owner alone (communication doc §5.5). Mirror mode makes several
    // clients watch the same session routinely, which is what turned this from theory into a visible bug.
    if (sizeMode !== "fit") {
      // Forget what was last sent so regaining ownership re-reports the real state instead of deduplicating
      // against a value another window's stream has since overwritten.
      lastFocusSentRef.current = null;
      return;
    }
    const focused = windowFocused && !hidden && isActiveSession;
    if (lastFocusSentRef.current === focused) return;
    lastFocusSentRef.current = focused;
    void ptyWrite(session.id, focused ? "\x1b[I" : "\x1b[O");
  }, [windowFocused, hidden, isActiveSession, session.id, sizeMode]);

  // When a session becomes visible/active, flush its background backlog immediately in chunks. Chunking
  // preserves xterm's frame slicing and lets keyboard events run between parse batches.
  useEffect(() => {
    if (isActiveSession && !hidden) flushTerminalOutput(session.id);
  }, [isActiveSession, hidden, session.id]);

  // Create xterm only after font readiness so its initial glyph measurement is correct.
  const fontsReady = useFontsReady();

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    // Wait for the real font rather than caching fallback widths; this effect reruns when ready.
    if (!fontsReady) return;

    // Terminal typography is independent from UI typography. Read initial settings and build a real
    // font-family string for xterm's canvas/managed DOM; a later effect applies live changes.
    const fontState = useTermStore.getState();

    const term = new Terminal({
      fontFamily: fontStack(fontState.termFontFamily),
      fontSize: fontState.termFontSize,
      lineHeight: 1.2,
      cursorBlink: true,
      scrollback: 5000,
      allowProposedApi: true,
      // In mouse-reporting TUIs, enable macOS Option-drag forced selection like iTerm2; otherwise xterm
      // forwards all dragging to the app and users cannot select/copy text.
      macOptionClickForcesSelection: true,
      // ─── Native xterm 5.5 rendering tuning ───
      // Enforce minimum contrast so ANSI white remains readable on light themes.
      minimumContrastRatio: 4.5,
      // Slightly increase wheel speed and fast-scroll responsiveness.
      scrollSensitivity: 1.15,
      fastScrollSensitivity: 5,
      // Render an unfocused split-pane block cursor as an unobtrusive outline.
      cursorInactiveStyle: "outline",
      // Use bright colors for clearer bold text.
      drawBoldTextInBrightColors: true,
      // Shrink glyphs that overflow their cell instead of letting them bleed into the neighbouring one.
      // This targets Nerd Font / CJK fonts whose icons are wider than the measured cell and smear the
      // next column. IMPORTANT: this option, and xterm's `customGlyphs` (default true, which draws
      // box-drawing characters as vectors rather than trusting the font), are read **only by the
      // canvas and webgl renderers**. Under the DOM renderer — the default, and the only usable one on
      // macOS WKWebView — both are dead settings. Setting it costs nothing and takes effect for users
      // who switch `termRenderer` to canvas/webgl in Settings, which on Windows/WebView2 is a working
      // path; do not read these two lines as "the glyph corruption is handled".
      rescaleOverlappingGlyphs: true,
      // Override OSC 8 links because xterm's confirm/window.open flow fails under Tauri.
      linkHandler: terminalLinkHandler,
      theme: XTERM_THEME[resolveTheme(useTermStore.getState().theme)],
    });
    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);
    term.loadAddon(new WebLinksAddon(webLinkActivate));
    // Enable OSC 52 select-to-copy through the provider above.
    term.loadAddon(new ClipboardAddon(undefined, makeOsc52Provider(session.id)));
    const unicode11 = new Unicode11Addon();
    term.loadAddon(unicode11);
    term.unicode.activeVersion = "11";
    term.open(container);

    // Keep Windows/Linux Ctrl+Alt+letter app shortcuts from reaching xterm as Meta escape sequences,
    // including edge cases where the global capture handler has no active session. Plain-browser
    // clients use the same Ctrl+Alt bindings on every OS (see IS_PLAIN_BROWSER), so block them there
    // too; desktop macOS keeps Cmd bindings and its Ctrl+Alt stays terminal input.
    //
    // The blocked set is derived from the bindings actually in effect, read at keypress time: a user who
    // rebinds an action to another letter would otherwise leak that combo into the terminal as Meta.
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;
      // Legacy terminal input has no encoding for a modified Enter, so xterm emits a bare CR for both
      // Enter and Shift+Enter and CLIs that treat Shift+Enter as "insert newline" (Claude Code, Codex)
      // see a submit instead. Emit ESC+CR, the same sequence iTerm2 sends once its Claude Code key
      // mapping is installed, and swallow the event so xterm does not also write CR. Skip while an IME
      // composition is active, where Enter commits the candidate.
      if (
        e.key === "Enter" &&
        e.shiftKey &&
        !e.ctrlKey &&
        !e.altKey &&
        !e.metaKey &&
        !e.isComposing
      ) {
        e.preventDefault();
        void ptyWrite(session.id, "\x1b\r").catch(() => {});
        return false;
      }
      // On Windows/Linux, let plain Ctrl+V reach native browser paste. xterm otherwise converts it to
      // \x16 and prevents the paste event, breaking both image capture and text paste. Ctrl+Shift+V is
      // already allowed, while macOS uses Cmd+V and keeps Ctrl+V for terminal applications. Use the
      // operator's platform, independent of the remote session host.
      if (isMac) {
        // Desktop macOS binds app shortcuts to Cmd, so only plain-browser clients need the
        // Ctrl+Alt block below; Ctrl+V stays terminal quoted-insert on macOS.
        if (!IS_PLAIN_BROWSER) return true;
      } else {
        // Delegate plain Ctrl+V to native browser paste for image support.
        if (
          e.ctrlKey &&
          !e.shiftKey &&
          !e.altKey &&
          !e.metaKey &&
          (e.code === "KeyV" || e.key === "v" || e.key === "V")
        ) {
          return false;
        }
      }
      // Do not let xterm process app-level Ctrl+Alt+letter shortcuts.
      if (
        e.ctrlKey &&
        e.altKey &&
        !e.metaKey &&
        appAltKeyCodes(useTermStore.getState().shortcutOverrides).has(e.code)
      ) {
        return false;
      }
      return true;
    });
    // SIXEL/iTerm IIP enables inline images. Load after open() creates rendering services but before
    // renderer addons so ImageAddon observes renderer switches; terminal disposal releases it.
    term.loadAddon(new ImageAddon());

    // Renderer setting applies to new terminals: DOM is the default and the only path verified under
    // Tauri (WKWebView), where canvas mismeasures cell width against the configured font; canvas does
    // draw custom glyphs (seamless TUI tables) without GPU-context loss, so it stays opt-in; WebGL is
    // sharp/fast but advanced because hidden Tauri views can corrupt glyph atlases or lose contexts.
    // Failure falls back to DOM.
    // Load after open() and ImageAddon.
    // Non-DOM renderers load their addon on demand (see term/rendererAddons.ts), so attaching is
    // asynchronous. The terminal renders through DOM until the chunk arrives, which is what it would
    // have fallen back to anyway had the addon been unavailable.
    //
    // The `termRef.current === term` guard is what keeps a late arrival from touching a disposed
    // terminal: termRef is assigned further down in this same synchronous block, so it is already set
    // by the time any await resumes, and cleanup clears it.
    const renderer = useTermStore.getState().termRenderer;
    if (renderer === "canvas") {
      void loadCanvasCtor().then((Ctor) => {
        if (!Ctor || termRef.current !== term) return;
        try {
          const canvas = new Ctor();
          term.loadAddon(canvas);
          rendererRef.current = canvas;
          dlog("canvas renderer attached ->", session.id);
        } catch (e) {
          dlog("canvas unavailable, fallback to DOM ->", session.id, e);
        }
      });
    } else if (renderer === "webgl") {
      void loadWebglCtor().then((Ctor) => {
        if (!Ctor || termRef.current !== term) return;
        try {
          const webgl = new Ctor();
          // On WebGL context loss, dispose and clear the addon so xterm falls back to DOM.
          webgl.onContextLoss(() => {
            dlog("webgl context lost ->", session.id);
            try {
              webgl.dispose();
            } catch {
              /* Known to throw during disposal; ignore. */
            }
            rendererRef.current = null;
          });
          term.loadAddon(webgl);
          rendererRef.current = webgl;
          dlog("webgl renderer attached ->", session.id);
        } catch (e) {
          dlog("webgl unavailable, fallback to DOM ->", session.id, e);
        }
      });
    }

    registerTerminal(session.id, term);
    registerSearch(session.id, searchAddon);
    termRef.current = term;
    fitRef.current = fitAddon;

    // Tell output scheduling when the user types so background draining yields. Capture keydown during
    // IME composition, composition events without keydown, and later onData paths; repeated signals only
    // refresh a timestamp.
    const onUserInputSignal = () => noteUserInput();
    container.addEventListener("keydown", onUserInputSignal, true);
    container.addEventListener("compositionstart", onUserInputSignal, true);
    container.addEventListener("compositionupdate", onUserInputSignal, true);

    let disposed = false;
    let unlistenExit: UnlistenFn | undefined;
    let unlistenKilled: UnlistenFn | undefined;
    let unlistenStatus: UnlistenFn | undefined;

    // ─── Query-response arbitration for shared sessions ───
    // Only the fit/owner client answers terminal queries; mirrors stay silent so one query produces one
    // response. During replay, suppress all historical query replies and OSC 52 writes to avoid injecting
    // stale coordinates/properties or overwriting the clipboard. Install after all addons.
    let replaying = false;
    const disposeReplyGuard = installQueryReplyGuard(term, {
      swallowReplies: () => replaying || modeRef.current === "mirror",
      swallowClipboardWrites: () => replaying,
    });

    // Never fit a display:none/zero-width container. xterm silently shrinks to two columns and
    // permanently hard-wraps history rather than throwing.
    const isVisible = () =>
      !!container.offsetParent && container.clientWidth > 0;

    const safeFit = () => {
      if (!isVisible()) return;
      try {
        fitAddon.fit();
      } catch {
        /* Defensive: fit may still throw in extreme cases. */
      }
    };

    // ─── Size mode: fit owner <-> mirror viewer ─────────────
    // Every remount starts in fit; the spawn result selects the actual mode.
    modeRef.current = "fit";
    ownerRef.current = null;
    setSizeMode("fit");
    setPtyDims(null);

    // Mirror scaling compares the pinned grid's natural pixel size with the container and only shrinks.
    const applyMirrorScale = () => {
      const screen = container.querySelector<HTMLElement>(".xterm-screen");
      if (!screen) return;
      const w = screen.offsetWidth;
      const h = screen.offsetHeight;
      const cw = container.clientWidth;
      const ch = container.clientHeight;
      if (!w || !h || !cw || !ch) return;
      const k = Math.min(cw / w, ch / h, 1);
      container.style.transform = `scale(${k})`;
      container.style.transformOrigin = "top left";
    };
    applyScaleRef.current = applyMirrorScale;

    // Hard redraw performs the complete manual-resize recovery. In fit mode, recompute the grid and send
    // a real resize only when drift exists; in mirror mode, recalculate scale without touching the PTY.
    // Then redraw, repair viewport geometry, and jiggle backend size so full-screen TUIs repaint.
    const hardRedraw = () => {
      if (modeRef.current === "mirror") {
        applyMirrorScale();
      } else if (isVisible()) {
        const prevCols = term.cols;
        const prevRows = term.rows;
        safeFit();
        if (term.cols !== prevCols || term.rows !== prevRows) {
          ptyResize(session.id, term.cols, term.rows).catch(() => {});
        }
      }
      try {
        term.refresh(0, term.rows - 1);
        term.scrollLines(-1);
        term.scrollLines(1);
      } catch {
        /* Defensive: refresh/scroll may throw in extreme cases. */
      }
      ptyRedraw(session.id).catch(() => {});
    };
    registerRedraw(session.id, hardRedraw);

    // Enter mirror by setting the PTY grid before replay, then scale now and next frame after layout.
    const enterMirror = (cols: number, rows: number) => {
      modeRef.current = "mirror";
      setSizeMode("mirror");
      setPtyDims({ cols, rows });
      if (term.cols !== cols || term.rows !== rows) {
        try {
          term.resize(cols, rows);
        } catch {
          /* Defend against invalid grid values. */
        }
      }
      applyMirrorScale();
      requestAnimationFrame(applyMirrorScale);
    };

    // Return to fit by clearing scale and measuring the container; the caller chooses takeover.
    const enterFit = () => {
      modeRef.current = "fit";
      setSizeMode("fit");
      setPtyDims(null);
      container.style.transform = "";
      safeFit();
    };

    // Claim an unowned PTY with a normal visible-client resize; first wins and others mirror its broadcast.
    const claimSize = () => {
      if (!isVisible()) return;
      enterFit();
      ptyResize(session.id, term.cols, term.rows).catch(() => {});
    };
    claimRef.current = claimSize;

    // Explicit “Fit this window” takeover replaces any owner and broadcasts others into mirror mode.
    takeoverRef.current = () => {
      enterFit();
      if (isVisible()) {
        ptyResize(session.id, term.cols, term.rows, { takeover: true }).catch(() => {});
      }
    };

    // Resized broadcasts successful changes, owner detachment, and attach snapshots.
    const handleResized = (cols: number, rows: number, owner: string | null) => {
      ownerRef.current = owner;
      // Ignore before spawn resolves; SpawnResult soon provides a more authoritative grid/owner and
      // avoids changing the grid before replay gating is ready.
      if (!gateOpen) return;
      const me = getClientSource();
      if (owner !== null && owner === me) {
        // Acknowledgement of takeover/claim or a normal owner resize echo.
        if (modeRef.current !== "fit") enterFit();
      } else if (owner !== null) {
        enterMirror(cols, rows);
      } else {
        claimSize();
      }
    };

    // ─── Debounced screen detection ───────────────────────────────────────
    // Detect only agent sessions after 250 ms output silence, analogous to content-changed polling.
    const agentKind: AgentKind | null =
      session.kind === "claude"
        ? "claude"
        : session.kind === "codex"
          ? "codex"
          : session.kind === "opencode"
            ? "opencode"
            : session.kind === "copilot"
              ? "copilot"
              : session.kind === "cursor"
                ? "cursor"
                : session.kind === "antigravity"
                  ? "antigravity"
                  : session.kind === "cline"
                    ? "cline"
                    : session.kind === "pi"
                      ? "pi"
                      : session.kind === "crush"
                        ? "crush"
                        : session.kind === "kimi"
                          ? "kimi"
                          : session.kind === "kiro"
                            ? "kiro"
                          : session.kind === "grok"
                            ? "grok"
                          : session.kind === "zoo"
                            ? "zoo"
                        : null;
    // Codex deliberately has no screen detector. Official lifecycle hooks (and legacy turn-complete notify)
    // are its only activity sources; an absent event is displayed as unavailable/running rather than guessed.
    const screenDetectKind = agentKind === "codex" ? null : agentKind;
    let detectTimer: ReturnType<typeof setTimeout> | undefined;
    let recheckTimer: ReturnType<typeof setTimeout> | undefined;
    let lastTail = "";

    // Remove the launch placeholder after enough first-screen output or a five-second fallback.
    let startupTimer: ReturnType<typeof setTimeout> | undefined;
    let startupBytes = 0;
    let startupCleared = false;
    const clearStarting = () => {
      if (startupCleared) return;
      startupCleared = true;
      clearTimeout(startupTimer);
      setStarting(false);
    };

    const runDetect = (force = false) => {
      if (disposed || !screenDetectKind) return;
      // Hook-authoritative agents normally skip screen reads outside working. The remaining screen-enabled
      // fallbacks may use a stable idle screen to recover from an agent-specific missing completion event.
      const rtNow = useTermStore.getState().runtimes[session.id];
      if (rtNow?.authoritative && rtNow.agentState !== "working") return;
      const tail = readScreenTail(term);
      if (!force && tail === lastTail) return; // Skip unchanged content.
      lastTail = tail;

      // Missing agents are reported authoritatively through e=notfound hooks, not screen text, because
      // an echoed guard command itself contains the not-found marker and causes false positives.
      const screen = detectAgentScreen(screenDetectKind, tail);
      useTermStore.getState().applyScreenDetection(session.id, screen);

      // Schedule a forced second look when the screen appears idle but status remains working. Ctrl+C
      // may produce no further output, otherwise preventing the store's stable-idle confirmation.
      if (
        screen.visibleIdle &&
        useTermStore.getState().runtimes[session.id]?.agentState === "working"
      ) {
        clearTimeout(recheckTimer);
        recheckTimer = setTimeout(() => runDetect(true), 2200);
      }
    };

    // Use one trailing timer that checks lastChunkAt and reschedules for remaining silence, avoiding
    // hundreds of clear/set operations during output floods while preserving 250 ms semantics.
    let lastChunkAt = 0;
    const detectWhenQuiet = () => {
      detectTimer = undefined;
      const since = performance.now() - lastChunkAt;
      if (since < 240) {
        detectTimer = setTimeout(detectWhenQuiet, Math.max(20, 250 - since));
        return;
      }
      runDetect();
    };
    const scheduleDetect = () => {
      if (!screenDetectKind) return;
      lastChunkAt = performance.now();
      if (detectTimer === undefined) detectTimer = setTimeout(detectWhenQuiet, 250);
    };

    // ─── Let xterm render output incrementally; do not force term.refresh per chunk ──────
    // Per-chunk full redraws caused severe flood/typing stalls and mostly redrew content immediately
    // replaced by the next chunk. Keep only conditional reveal redraws for hidden-renderer degradation.

    // ─── Replay gate ──────────────────────────────────────────────
    // Buffer attach replay/new-session output until spawn resolves, then establish the grid/mode before
    // ordered flush. This is transport-order-independent and is rearmed when reconnection starts.
    let gateOpen = false;
    const preBuf: Uint8Array[] = [];
    // Every received byte, live or replayed, schedules screen detection and launch-byte accounting.
    const noteChunk = (bytes: Uint8Array) => {
      scheduleDetect(); // Debounce screen detection after output.
      // Remove the launch placeholder once Claude has drawn enough of its first screen.
      if (!startupCleared) {
        startupBytes += bytes.length;
        if (startupBytes > 1200) clearStarting();
      }
    };

    // GrokDay paints its analysis canvas as #f6f6f6; remap to pure white so light mode matches Codex's
    // white analysis pane while slightly darker user-bar grays stay gray.
    const paintBytes = (bytes: Uint8Array) =>
      session.kind === "grok" ? remapGrokDayCanvasToWhite(bytes) : bytes;

    // Replay flush writes directly, bypassing the scheduler because replaying/done ordering is strict.
    // The parse callback controls gate timing only; xterm handles rendering.
    const writeChunk = (bytes: Uint8Array, onParsed?: () => void) => {
      const painted = paintBytes(bytes);
      term.write(painted, onParsed);
      noteChunk(painted);
    };

    // After the gate opens, schedule live output immediately for foreground and coalesced/throttled for
    // background. Determine foreground from current store/hidden refs, not stale closures.
    const writeLive = (bytes: Uint8Array) => {
      const painted = paintBytes(bytes);
      noteChunk(painted);
      const foreground =
        useTermStore.getState().activeSessionId === session.id &&
        !hiddenRef.current;
      writeTerminalOutput(session.id, term, painted, {
        foreground,
      });
    };
    // Flush gate buffers. For attach/reattach history, keep replay suppression until the final chunk's
    // ordered parse callback, preventing historical query replies. Then restore live arbitration and
    // invoke onFlushed for viewport repair.
    const openGate = (suppress: boolean, onFlushed?: () => void) => {
      gateOpen = true;
      const chunks = preBuf.splice(0, preBuf.length);
      const done = () => {
        replaying = false;
        onFlushed?.();
      };
      if (suppress && chunks.length > 0) {
        replaying = true;
        for (let i = 0; i < chunks.length - 1; i++) writeChunk(chunks[i]);
        writeChunk(chunks[chunks.length - 1], done);
      } else {
        for (const chunk of chunks) writeChunk(chunk);
        done();
      }
    };

    const setup = async () => {
      safeFit();
      const cols = term.cols;
      const rows = term.rows;

      // Register independent exit/killed/status listeners in parallel to reduce Tauri IPC latency, but
      // always before spawn to avoid missing events. Natural exit closes the pane; remote kills close the
      // local view except this instance's unmount/restart events; status feeds store/notifications while
      // resized is consumed directly by the terminal size state machine.
      [unlistenExit, unlistenKilled, unlistenStatus] = await Promise.all([
        onPtyExit(session.id, () => {
          if (disposed) return;
          dlog("pty exit -> closeSession", session.id);
          useTermStore.getState().closeSession(session.id);
        }),
        onPtyKilled(session.id, () => {
          if (disposed || wasKilledLocally(session.id)) return;
          dlog("pty killed by another end -> closeSession", session.id);
          useTermStore.getState().closeSession(session.id);
        }),
        onPtyStatus(session.id, (signal) => {
          if (disposed) return;
          if (signal.kind === "resized") {
            handleResized(signal.cols, signal.rows, signal.owner);
            return;
          }
          // An authoritative notfound hook removes the launch placeholder, shows installation guidance,
          // and clears any prior install-in-progress state after this fresh spawn failed again.
          if (signal.kind === "agent_missing") {
            clearStarting();
            useTermStore
              .getState()
              .setRuntime(session.id, { agentMissing: true, agentInstalling: false });
            return;
          }
          useTermStore.getState().applyStatusSignal(session.id, signal);
        }),
      ]);

      if (disposed) return;

      // Take a pending child-task prompt once and pass it to spawn as a positional launch argument.
      // This avoids readiness races from typing later and prevents reinjection on restart/resume.
      const initialPrompt = useTermStore
        .getState()
        .takePendingPrompt(session.id);

      try {
        const result = await ptySpawn(
          {
            sessionId: session.id,
            kind: session.kind,
            shell: session.shell ?? undefined,
            cwd: cwd ?? undefined,
            cols,
            rows,
            initialPrompt: initialPrompt ?? undefined,
            // Inject UI brightness into Claude settings because Windows ConPTY blocks reliable OSC 11 detection.
            theme: resolveTheme(useTermStore.getState().theme),
          },
          // PTY output arrives through the binary Channel without JSON and is gated until the grid is set.
          (bytes) => {
            // Schedule live output only after the gate opens; otherwise buffer for direct openGate replay.
            if (gateOpen) writeLive(bytes);
            else preBuf.push(bytes);
          },
        );
        const { pid, launch, attached } = result;
        // Reset missing/install state on every spawn. A still-missing agent will report notfound again.
        setRuntime(session.id, {
          status: "running",
          pid,
          startedAt: Date.now(),
          agentMissing: false,
          agentInstalling: false,
          agentPathSaved: null,
          // Store a newly generated full launch command for Option-revealed session info. Attach omits it.
          ...(launch ? { launchCmd: launch } : {}),
        });

        // Select size mode before replay: attach to another owner mirrors its grid; an unowned visible
        // attach claims with normal resize; new/self-owned sessions remain fit.
        ownerRef.current = result.owner;
        const me = getClientSource();
        if (attached && result.owner !== null && result.owner !== me) {
          enterMirror(result.cols, result.rows);
        } else if (attached && result.owner === null && isVisible()) {
          ptyResize(session.id, term.cols, term.rows).catch(() => {});
        }
        // Attach buffers are history and suppress responses; new-session buffers are live and may answer queries.
        openGate(attached);

        // Execute backend-generated agent launch or terminal initCmd only for a new PTY. Attach must skip
        // both, otherwise a browser attaching to a running terminal would rerun initCmd.
        const autoCmd = attached ? undefined : (launch ?? session.initCmd?.trim());
        if (autoCmd) {
          // Show the agent-launch placeholder across shell/Claude cold start, with a five-second fallback.
          if (agentKind) {
            setStarting(true);
            startupTimer = setTimeout(clearStarting, 5000);
          }
          await ptyWrite(session.id, `${autoCmd}\r`);
        }
      } catch (err) {
        clearStarting();
        setRuntime(session.id, { status: "error" });
        term.writeln(`\r\n\x1b[31m${t("term.startFailed", String(err))}\x1b[0m`);
      }
    };

    // WebKit IME fallback immediately writes committed full-width punctuation and swallows xterm's later
    // duplicate onData, avoiding delayed/out-of-order delivery.
    const imeFix = isWebkitEngine()
      ? installWebkitImeFix(term, (data) => {
          ptyWrite(session.id, data).catch(() => {});
        })
      : null;

    // Codex inline TUI finalization after Esc can leave xterm 5.5 viewportY behind baseY when parsing
    // partial-region scroll sequences, causing a jump into history even without alternate-screen mode.
    //
    // For a brief window after a standalone Esc while Codex is working, keep parsed output at the live
    // bottom. Any next input cancels this, preserving idle Esc and second-Esc interactions.
    let followCodexInterruptUntil = 0;
    const writeParsedSub = term.onWriteParsed(() => {
      if (performance.now() > followCodexInterruptUntil) return;
      const buffer = term.buffer.active;
      if (buffer.viewportY !== buffer.baseY) term.scrollToBottom();
    });

    const dataSub = term.onData((data) => {
      // onData is the authoritative input moment, covering context-menu paste and other no-keydown paths.
      noteUserInput();
      // Swallow xterm's late duplicate of punctuation already sent by the WebKit fallback.
      if (imeFix?.shouldSwallow(data)) return;
      ptyWrite(session.id, data).catch(() => {});
      // Ctrl+C or standalone Esc immediately ends working status because Claude emits no completion hook
      // after interruption. Ctrl+C may use includes; Esc must equal exactly `\x1b`, since arrows/Alt/function
      // keys all begin with Esc and must not falsely clear status.
      const isInterrupt = data.includes("\x03") || data === "\x1b";
      const cur = agentKind
        ? useTermStore.getState().runtimes[session.id]?.agentState
        : undefined;
      if (agentKind === "codex" && data === "\x1b" && cur === "working") {
        // Return immediately to bottom, then correct parsed batches for up to five seconds.
        followCodexInterruptUntil = performance.now() + 5000;
        term.scrollToBottom();
      } else {
        // Any subsequent interaction, including a second Esc, stops viewport intervention.
        followCodexInterruptUntil = 0;
      }
      // Change only working to waiting for agents that still require an input-interrupt fallback. Codex is
      // hook-only: its Stop lifecycle event owns this transition, and missing events must remain visible.
      if (agentKind && agentKind !== "codex" && isInterrupt) {
        if (cur === "working") {
          useTermStore
            .getState()
            .setRuntime(session.id, { agentState: "waiting" });
        }
      }
    });

    const resizeObserver = new ResizeObserver(() => {
      // Ignore display:none zero-width resize callbacks so fit cannot collapse and corrupt history.
      if (!isVisible()) return;
      // Mirror mode only recalculates CSS scale; never fit or resize the PTY.
      if (modeRef.current === "mirror") {
        applyMirrorScale();
        return;
      }
      safeFit();
      ptyResize(session.id, term.cols, term.rows).catch(() => {});
    });
    resizeObserver.observe(container);

    // Browser reconnection reuses attach gating: before respawn, close the gate and reset because replay
    // redraws mode prelude plus current screen as a unit. FIFO buffers replay before the response. On
    // completion, apply the latest owner/grid, flush under response suppression, and repair viewport.
    const unlistenReattachStart = onPtyReattachStart(session.id, () => {
      if (disposed) return;
      gateOpen = false;
      preBuf.length = 0;
      try {
        term.reset();
      } catch {
        /* Defensive: reset failure must not block reattachment. */
      }
    });
    const unlistenReattach = onPtyReattach(session.id, (res) => {
      if (disposed || !termRef.current) return;
      ownerRef.current = res.owner;
      const me = getClientSource();
      if (res.owner !== null && res.owner !== me) {
        enterMirror(res.cols, res.rows);
      } else if (res.owner === null) {
        claimSize();
      } else if (modeRef.current !== "fit") {
        enterFit();
      }
      openGate(true, () => {
        termRef.current?.scrollLines(-1);
        termRef.current?.scrollLines(1);
      });
    });

    void setup();

    return () => {
      disposed = true;
      // Drop this session's scheduler backlog before disposal.
      discardTerminalOutput(session.id);
      clearTimeout(detectTimer);
      clearTimeout(recheckTimer);
      clearTimeout(startupTimer);
      resizeObserver.disconnect();
      container.removeEventListener("keydown", onUserInputSignal, true);
      container.removeEventListener("compositionstart", onUserInputSignal, true);
      container.removeEventListener("compositionupdate", onUserInputSignal, true);
      imeFix?.dispose();
      writeParsedSub.dispose();
      dataSub.dispose();
      unlistenExit?.();
      unlistenKilled?.();
      unlistenStatus?.();
      unlistenReattach?.();
      unlistenReattachStart?.();
      disposeReplyGuard();
      unregisterSearch(session.id, searchAddon);
      unregisterRedraw(session.id, hardRedraw);
      unregisterTerminal(session.id, term);
      if (termRef.current === term) termRef.current = null;
      if (fitRef.current === fitAddon) fitRef.current = null;
      applyScaleRef.current = null;
      claimRef.current = null;
      takeoverRef.current = null;
      // On unmount, desktop kills its process while browser detaches without killing the shared session.
      ptyTeardownSession(session.id).catch(() => {});
      // Dispose the current renderer addon explicitly before term.dispose. WebGL may have been rebuilt,
      // and its disposal can throw; using the ref plus try/catch prevents teardown from aborting.
      try {
        rendererRef.current?.dispose();
      } catch {
        /* WebGL disposal is known to throw; ignore. */
      }
      rendererRef.current = null;
      term.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session.id, fontsReady]);

  // Apply font changes live. Visible fit mode refits and sends a changed grid; mirror mode only rescales.
  // Hidden tabs defer layout until reveal. Initial equal values cause no redundant fit/resize.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    const fam = fontStack(termFontFamily);
    if (term.options.fontFamily === fam && term.options.fontSize === termFontSize) return;
    term.options.fontFamily = fam;
    term.options.fontSize = termFontSize;
    const container = containerRef.current;
    if (modeRef.current === "mirror") {
      applyScaleRef.current?.();
    } else if (container && container.offsetParent && container.clientWidth > 0) {
      try {
        fitRef.current?.fit();
      } catch {
        /* Defensive: fit may throw in extreme cases. */
      }
      ptyResize(session.id, term.cols, term.rows).catch(() => {});
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [termFontFamily, termFontSize]);

  // Repair scroll geometry when a hidden tab becomes visible.
  //
  // display:none tabs keep receiving output, but xterm computes scroll height with offsetHeight=0 and
  // leaves it one screen short. If dimensions do not change on reveal, no normal path recalculates it.
  //
  // Force recalculation by scrolling one line up and back. Net position is unchanged, while each real
  // scroll invokes syncScrollArea; fit/refresh at equal dimensions would not.
  useEffect(() => {
    // Synchronize current hidden state for foreground output scheduling.
    hiddenRef.current = !!hidden;
    if (hidden) {
      // Record hide time for reveal-duration decisions.
      hiddenSinceRef.current = Date.now();
      return;
    }
    const term = termRef.current;
    const fit = fitRef.current;
    const container = containerRef.current;
    if (!term || !fit || !container) return;

    // A long-hidden WebGL context may be reclaimed without onContextLoss, leaving xterm drawing into a
    // dead context. Rebuild it after the threshold or fall back to DOM.
    const hiddenMs = hiddenSinceRef.current ? Date.now() - hiddenSinceRef.current : 0;
    hiddenSinceRef.current = null;
    const STALE_RENDERER_MS = 30_000;
    // Only WebGL needs context rebuild. Canvas/DOM do not. Full redraw is a separate opt-in advanced
    // setting so normal reveal remains smooth unless artifacts require the fallback.
    const termRenderer = useTermStore.getState().termRenderer;
    const redrawOnReveal = useTermStore.getState().redrawOnReveal;

    let cancelled = false;
    let rafId = 0;
    const recover = (attempt: number) => {
      rafId = requestAnimationFrame(() => {
        if (cancelled) return;
        // Retry for a few frames until layout produces a visible nonzero-width container.
        if (!container.offsetParent || container.clientWidth === 0) {
          if (attempt < 5) recover(attempt + 1);
          return;
        }

        // For long-hidden WebGL, safely dispose the old addon and attach a fresh one. Failure leaves DOM
        // fallback; success requires one full redraw because the new context starts empty.
        let rebuiltWebgl = false;
        if (termRenderer === "webgl" && hiddenMs >= STALE_RENDERER_MS) {
          try {
            rendererRef.current?.dispose();
          } catch {
            /* Known disposal exception; ignore. */
          }
          rendererRef.current = null;
          // Reached only after WebGL attached successfully once, so the constructor is already
          // resolved; a null here means the addon never loaded and DOM stays, matching the catch below.
          const Ctor = peekWebglCtor();
          try {
            if (!Ctor) throw new Error("webgl addon not loaded");
            const next = new Ctor();
            next.onContextLoss(() => {
              try {
                next.dispose();
              } catch {
                /* Known disposal exception; ignore. */
              }
              rendererRef.current = null;
            });
            term.loadAddon(next);
            rendererRef.current = next;
            rebuiltWebgl = true;
          } catch {
            /* WebGL unavailable: keep the DOM renderer. */
          }
        }

        // Redraw the full screen only after size changes, explicit redraw-on-reveal, or WebGL rebuild.
        // DOM rows remain updated while hidden, so ordinary same-size tab switches need only composition.
        let dimsChanged = false;
        if (modeRef.current === "mirror") {
          // In mirror mode, claim if the owner left while hidden; otherwise rescale only. Treat either as
          // potentially requiring redraw without fitting or resizing another owner's PTY.
          if (ownerRef.current === null) {
            claimRef.current?.();
          } else {
            applyScaleRef.current?.();
          }
          dimsChanged = true;
        } else {
          // In fit mode, measure the possibly resized window and use the grid change as the redraw probe.
          const prevCols = term.cols;
          const prevRows = term.rows;
          try {
            fit.fit();
          } catch {
            /* Defensive: fit may still throw. */
          }
          dimsChanged = term.cols !== prevCols || term.rows !== prevRows;
          // Send resize only for a real grid change; redundant SIGWINCH causes expensive agent redraws.
          if (dimsChanged) {
            ptyResize(session.id, term.cols, term.rows).catch(() => {});
          }
        }

        if (dimsChanged || redrawOnReveal || rebuiltWebgl) {
          // Clear degraded Canvas/WebGL glyph atlases and redraw so the renderer rerasterizes everything.
          try {
            rendererRef.current?.clearTextureAtlas();
          } catch {
            /* The addon may already be disposed. */
          }
          term.refresh(0, term.rows - 1);
        }
        // Recalculate viewport scroll area by scrolling one line up and back; cheap and always required.
        term.scrollLines(-1);
        term.scrollLines(1);
      });
    };
    recover(0);

    return () => {
      cancelled = true;
      cancelAnimationFrame(rafId);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hidden]);

  return { containerRef, starting, sizeMode, ptyDims, takeoverSize };
}
