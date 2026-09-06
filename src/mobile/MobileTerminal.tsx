//! Lightweight mobile replacement for TerminalView. It retains usePtySession integration
//! (spawn or attach, scrollback replay, reconnect recovery, and size mirroring) plus pasted or
//! dropped image injection, while omitting pane headers, context menus, split callbacks, and Git status.
//!
//! Unlike desktop, mobile mounts only the current session. Returning to the list detaches without
//! killing the shared browser-side session; reopening attaches again and replays its output.
//!
//! When attaching to a desktop-owned session, usePtySession defaults to mirror mode: grid dimensions
//! stay fixed to the PTY and CSS scales the terminal. Input does not claim sizing; the mirror badge
//! lets the user explicitly take ownership using this device's dimensions.

import { useRef } from "react";
import { useT } from "../i18n";
import { usePtySession } from "../hooks/usePtySession";
import { imagesFromClipboard, imagesFromDrop } from "../terminal/imageInput";
import type { Session } from "../types";
import { useTouchTerminal } from "./useTouchTerminal";

export function MobileTerminal({
  session,
  cwd,
  onImages,
  imgError,
}: {
  session: Session;
  cwd?: string;
  /** Send pasted or dropped images upward for upload, path injection, and shared error handling. */
  onImages: (files: File[]) => void;
  /** Image-upload error managed by the parent and cleared after five seconds; empty means hidden. */
  imgError?: string | null;
}) {
  const t = useT();
  const { containerRef, starting, sizeMode, ptyDims, takeoverSize } = usePtySession(
    session,
    cwd,
    false,
  );

  // Touch layer: swipe to scroll, tap for the keyboard, long-press to select, and suppress synthetic mouse events.
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const { overlay, copy, cancel } = useTouchTerminal(wrapRef, session.id);

  return (
    <div
      ref={wrapRef}
      className="m-term"
      // Use the capture phase (`onPasteCapture`): xterm's textarea paste listener always calls
      // stopPropagation, so a bubbling onPaste never fires. For images, also stop propagation so
      // xterm does not perform an empty text paste. Leave plain text to xterm's normal paste path.
      onPasteCapture={(e) => {
        const imgs = imagesFromClipboard(e.clipboardData);
        if (imgs.length) {
          e.preventDefault();
          e.stopPropagation();
          onImages(imgs);
        }
      }}
      onDragOver={(e) => {
        if (e.dataTransfer?.types?.includes("Files")) e.preventDefault();
      }}
      onDrop={(e) => {
        const imgs = imagesFromDrop(e.dataTransfer);
        if (imgs.length) {
          e.preventDefault();
          onImages(imgs);
        }
      }}
    >
      <div ref={containerRef} className="m-term-host" />

      {/* Touch-selection overlay: highlight rectangles, two draggable handles, and a copy/cancel bar.
          useTouchTerminal derives text and coordinates through the public buffer API. The overlay
          and handles ignore pointer events; the wrapper's touch listener detects handle drags by
          coordinates. `data-ctl` lets toolbar buttons receive clicks without stealing focus. */}
      {overlay && (
        <>
          {overlay.rects.map((r, i) => (
            <div
              key={i}
              className="m-sel-rect"
              style={{ left: r.x, top: r.y, width: r.w, height: r.h }}
            />
          ))}
          {overlay.start && (
            <div
              className="m-sel-handle"
              style={{ left: overlay.start.x, top: overlay.start.y }}
            />
          )}
          {overlay.end && (
            <div
              className="m-sel-handle"
              style={{ left: overlay.end.x, top: overlay.end.y }}
            />
          )}
          <div className="m-sel-bar" data-ctl>
            <button type="button" className="m-sel-btn" data-ctl onClick={copy}>
              {t("mobile.selCopy")}
            </button>
            <button type="button" className="m-sel-btn" data-ctl onClick={cancel}>
              {t("mobile.selCancel")}
            </button>
          </div>
        </>
      )}
      {/* Mirror badge: this client scales the terminal to a PTY size owned elsewhere. Clicking takes
          ownership, resizes the PTY to this window, and moves other clients into mirror mode. It is
          hidden in fit mode. Preventing pointerdown's default preserves xterm focus and keyboard state. */}
      {sizeMode === "mirror" && (
        <button
          type="button"
          className="m-mirror-badge"
          // `data-ctl` lets the touch layer pass this control through without focusing or starting selection.
          data-ctl
          onPointerDown={(e) => e.preventDefault()}
          onClick={takeoverSize}
        >
          {t("term.mirrorBadgeMobile", ptyDims ? ` ${ptyDims.cols}×${ptyDims.rows}` : "")}
        </button>
      )}
      {/* Image-upload error banner, cleared by the parent after five seconds. Console-only errors were too easy to miss. */}
      {imgError && <div className="m-img-error">{imgError}</div>}
      {starting && (
        <div className="m-starting">
          {t(
            "term.starting",
            session.kind === "codex"
              ? "Codex"
              : session.kind === "opencode"
                ? "OpenCode"
                : session.kind === "copilot"
                  ? "Copilot"
                  : session.kind === "cursor"
                    ? "Cursor"
                    : session.kind === "antigravity"
                      ? "Antigravity"
                      : session.kind === "cline"
                        ? "Cline"
                        : session.kind === "pi"
                          ? "Pi"
                          : session.kind === "omp"
                          ? "OMP"
                          : session.kind === "crush"
                            ? "Crush"
                            : session.kind === "kimi"
                              ? "Kimi Code (K3)"
                              : session.kind === "kiro"
                                ? "Kiro"
                              : session.kind === "grok"
                                ? "Grok Build (Grok 4.5)"
                              : session.kind === "zoo"
                                ? "Zoo Code"
                            : "Claude",
          )}
        </div>
      )}
    </div>
  );
}
