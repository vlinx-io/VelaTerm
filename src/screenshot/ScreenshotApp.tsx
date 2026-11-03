//! Screenshot overlay: region selection plus annotation, in the style of chat-app screenshot tools.
//!
//! The overlay window covers one monitor and shows the captured frame at 1:1. Before a region exists,
//! dragging selects one and a click selects the whole screen. With a region, its edges and corners
//! resize it, dragging inside moves it (when no tool is active), and the toolbar offers shapes, pen,
//! mosaic, text, undo, save, cancel, and Done (copy to the clipboard). Esc finishes typing, otherwise
//! cancels; a right click clears the region, or cancels when there is none. Enter or a double click inside the region
//! finishes. Losing focus cancels, so the overlay can never be left covering the screen.
//!
//! The window itself is created hidden ahead of time and reused (see src-tauri/src/screenshot.rs):
//! `ScreenshotOverlay` mounts a fresh editor for each screenshot and unmounts it when it ends.
//!
//! Outside the desktop shell (a plain browser during development) a generated test frame stands in
//! for the capture and results are kept on `window.__vlxScreenshotLast`.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
import Icons from "../components/Icons";
import {
  onScreenshotFocusChanged,
  onScreenshotFrame,
  screenshotClose,
  screenshotFinish,
  screenshotFrame,
  screenshotPending,
  screenshotReady,
} from "../ipc/screenshot";
import { reloadLangChoice, useT, type I18nKey } from "../i18n";
import {
  COLORS,
  MOSAIC_WIDTHS,
  STROKE_WIDTHS,
  TEXT_LINE_HEIGHT,
  TEXT_SIZES,
  boxFrom,
  clampMove,
  contains,
  drawShapes,
  exportPng,
  handleCursor,
  handlePoint,
  hitHandle,
  isMeaningful,
  makeMosaicSource,
  resizeBox,
  textFont,
  toolbarPosition,
  HANDLES,
  type Box,
  type Handle,
  type MosaicSource,
  type Pt,
  type Shape,
  type Tool,
} from "./scene";

const IS_TAURI = "__TAURI_INTERNALS__" in window;
const IS_MAC = /mac/i.test(navigator.platform || navigator.userAgent);
const MOD = IS_MAC ? "⌘" : "Ctrl+";


/** Test frame for development in a plain browser: a gradient with a grid and some text. */
async function demoFrame(): Promise<Blob> {
  const dpr = window.devicePixelRatio || 1;
  const c = document.createElement("canvas");
  c.width = Math.round(window.innerWidth * dpr);
  c.height = Math.round(window.innerHeight * dpr);
  const ctx = c.getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, c.width, c.height);
  g.addColorStop(0, "#1e3a5f");
  g.addColorStop(1, "#5f1e4a");
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, c.width, c.height);
  ctx.strokeStyle = "rgba(255,255,255,0.12)";
  for (let x = 0; x < c.width; x += 40 * dpr) ctx.strokeRect(x, 0, 0, c.height);
  for (let y = 0; y < c.height; y += 40 * dpr) ctx.strokeRect(0, y, c.width, 0);
  ctx.fillStyle = "#fff";
  ctx.font = `${32 * dpr}px sans-serif`;
  ctx.fillText("VelaTerm screenshot test frame", 80 * dpr, 120 * dpr);
  return new Promise((resolve) => c.toBlob((b) => resolve(b!), "image/png"));
}

async function loadFrame(): Promise<Blob> {
  if (!IS_TAURI) return demoFrame();
  const buf = await screenshotFrame();
  return new Blob([buf], { type: "image/png" });
}

type Drag =
  | { mode: "create"; start: Pt }
  | { mode: "move"; start: Pt; orig: Box }
  | { mode: "resize"; handle: Handle }
  | { mode: "draw" };

interface TextDraft {
  at: Pt;
  value: string;
}

const TOOLS: { tool: Tool; icon: keyof typeof Icons; label: I18nKey }[] = [
  { tool: "rect", icon: "shotRect", label: "screenshot.rect" },
  { tool: "ellipse", icon: "shotEllipse", label: "screenshot.ellipse" },
  { tool: "arrow", icon: "shotArrow", label: "screenshot.arrow" },
  { tool: "pen", icon: "shotPen", label: "screenshot.pen" },
  { tool: "mosaic", icon: "shotMosaic", label: "screenshot.mosaic" },
  { tool: "text", icon: "shotText", label: "screenshot.text" },
];

const SIZE_LABELS: I18nKey[] = ["screenshot.small", "screenshot.medium", "screenshot.large"];

let measureCtx: CanvasRenderingContext2D | null = null;
function textWidth(text: string, size: number): number {
  measureCtx ??= document.createElement("canvas").getContext("2d");
  if (!measureCtx) return text.length * size;
  measureCtx.font = textFont(size);
  return Math.max(...text.split("\n").map((l) => measureCtx!.measureText(l).width));
}

/** Resolve once the current content has been painted, or shortly after when nothing is painting. */
function afterPaint(): Promise<void> {
  return new Promise((resolve) => {
    const timer = window.setTimeout(resolve, 100);
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        window.clearTimeout(timer);
        resolve();
      }),
    );
  });
}

/**
 * Root of the reused overlay window. Each screenshot gets its own editor, keyed by the screenshot id,
 * so no selection or annotation carries over from the previous one.
 */
export function ScreenshotOverlay() {
  const [session, setSession] = useState<number | null>(IS_TAURI ? null : 1);

  useEffect(() => {
    if (!IS_TAURI) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    // The language may have changed in the main window since this page loaded.
    const begin = (id: number) =>
      void reloadLangChoice().then(() => {
        if (!disposed) setSession(id);
      });
    void onScreenshotFrame(begin).then((fn) => {
      if (disposed) return fn();
      unlisten = fn;
      // A frame captured while this page was still loading announced itself before anyone listened.
      void screenshotPending().then((id) => {
        if (id != null) begin(id);
      });
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Clear the editor and let that paint before the window hides, so the next screenshot cannot
  // briefly show this one's frame when the window reappears.
  const end = useCallback(async () => {
    flushSync(() => setSession(null));
    await afterPaint();
  }, []);

  return session === null ? null : <ScreenshotApp key={session} session={session} onEnd={end} />;
}

function ScreenshotApp({ session, onEnd }: { session: number; onEnd: () => Promise<void> }) {
  const t = useT();
  const [img, setImg] = useState<HTMLImageElement | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [view, setView] = useState({ w: window.innerWidth, h: window.innerHeight });
  const [sel, setSel] = useState<Box | null>(null);
  const [tool, setTool] = useState<Tool | null>(null);
  const [color, setColor] = useState(COLORS[0]);
  const [sizeIdx, setSizeIdx] = useState(1);
  const [shapes, setShapes] = useState<Shape[]>([]);
  const [draft, setDraftState] = useState<Shape | null>(null);
  const [text, setText] = useState<TextDraft | null>(null);
  const [pointer, setPointer] = useState<Pt | null>(null);
  const [dragMode, setDragMode] = useState<Drag["mode"] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tbSize, setTbSize] = useState({ w: 0, h: 0 });

  const drag = useRef<Drag | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const textRef = useRef<HTMLTextAreaElement>(null);
  const mosaic = useRef<MosaicSource | null>(null);
  // Mirrors `draft` so pointer-up can read the finished shape without a side effect in a state updater.
  const draftRef = useRef<Shape | null>(null);
  const setDraft = (next: Shape | null | ((s: Shape | null) => Shape | null)) => {
    draftRef.current = typeof next === "function" ? next(draftRef.current) : next;
    setDraftState(draftRef.current);
  };

  const textSize = TEXT_SIZES[sizeIdx];

  // ── Frame loading and window lifecycle ──

  useEffect(() => {
    let objectUrl: string | null = null;
    let cancelled = false;
    void loadFrame()
      .then((blob) => {
        if (cancelled) return;
        objectUrl = URL.createObjectURL(blob);
        const image = new Image();
        image.src = objectUrl;
        setUrl(objectUrl);
        // decode() rather than onload: the frame must be fully decoded before the window is revealed.
        void image.decode().then(() => {
          if (!cancelled) setImg(image);
        });
      })
      .catch(() => void close());
    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Reveal the window once the decoded frame is in the DOM. Neither requestAnimationFrame nor a timer:
  // WebKit runs no animation frames for a hidden window and may throttle timers on a hidden page, which
  // this page is until the call. A message-channel task is not throttled.
  useEffect(() => {
    if (!img || !IS_TAURI) return;
    const channel = new MessageChannel();
    channel.port1.onmessage = () => void screenshotReady(session);
    channel.port2.postMessage(null);
    return () => channel.port1.close();
  }, [img, session]);

  // The pixelated frame behind mosaic strokes costs two full-frame canvas draws, so it is built when
  // first needed rather than before the window appears, and again after a resize.
  const ensureMosaic = useCallback((): MosaicSource | null => {
    if (!img) return null;
    const m = mosaic.current;
    if (!m || m.cssWidth !== view.w || m.cssHeight !== view.h) {
      mosaic.current = makeMosaicSource(img, view.w, view.h);
    }
    return mosaic.current;
  }, [img, view.w, view.h]);
  const mosaicFor = useCallback(
    (list: Shape[]) => (list.some((s) => s.kind === "mosaic") ? ensureMosaic() : null),
    [ensureMosaic],
  );
  // Picking the tool prepares it, so the first stroke does not stall.
  useEffect(() => {
    if (tool === "mosaic") ensureMosaic();
  }, [tool, ensureMosaic]);

  useEffect(() => {
    const onResize = () => setView({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  // Cancel when focus moves to another window, e.g. after Cmd+Tab. A short grace period after the
  // first focus ignores the focus churn of showing and activating the window.
  useEffect(() => {
    if (!IS_TAURI) return;
    let focusedAt = 0;
    let unlisten: (() => void) | undefined;
    void onScreenshotFocusChanged((focused) => {
      if (focused) {
        if (!focusedAt) focusedAt = performance.now();
      } else if (focusedAt && performance.now() - focusedAt > 500) {
        void close();
      }
    }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const close = useCallback(async () => {
    if (IS_TAURI) {
      await onEnd();
      await screenshotClose();
    } else {
      setSel(null);
      setShapes([]);
      setTool(null);
      setText(null);
    }
  }, [onEnd]);

  // ── Shapes ──

  const textShape = useCallback(
    (d: TextDraft | null): Shape | null =>
      d && d.value.trim() ? { kind: "text", at: d.at, text: d.value.replace(/\s+$/, ""), color, size: textSize } : null,
    [color, textSize],
  );

  const commitText = useCallback(() => {
    const s = textShape(text);
    if (s) setShapes((prev) => [...prev, s]);
    setText(null);
  }, [text, textShape]);

  const undo = useCallback(() => {
    if (text) {
      setText(null);
      return;
    }
    setShapes((prev) => prev.slice(0, -1));
  }, [text]);

  const finish = useCallback(
    async (action: "copy" | "save") => {
      if (!img || !sel || busy) return;
      const pending = textShape(text);
      const all = pending ? [...shapes, pending] : shapes;
      setBusy(true);
      setError(null);
      try {
        const png = await exportPng(img, sel, all, mosaicFor(all), view.w);
        if (IS_TAURI) {
          await onEnd();
          // The editor is gone from here on, so a failure cannot be shown; the overlay must not stay up.
          await screenshotFinish(action, png).catch(() => screenshotClose());
        } else {
          (window as unknown as Record<string, unknown>).__vlxScreenshotLast = { action, bytes: png.length };
          setBusy(false);
          await close();
        }
      } catch (e) {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [img, sel, busy, text, textShape, shapes, view.w, close, mosaicFor, onEnd],
  );

  // ── Pointer handling ──

  const pointOf = (e: React.PointerEvent | React.MouseEvent): Pt => ({ x: e.clientX, y: e.clientY });

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || !img || busy) return;
    const p = pointOf(e);
    if (text) {
      // A click outside the text box finishes typing; the next click starts something new.
      commitText();
      return;
    }
    let next: Drag | null = null;
    if (!sel) {
      next = { mode: "create", start: p };
    } else {
      const handle = hitHandle(sel, p);
      if (handle) {
        next = { mode: "resize", handle };
      } else if (contains(sel, p)) {
        if (tool === "text") {
          const lineHeight = textSize * TEXT_LINE_HEIGHT;
          setText({ at: { x: p.x, y: p.y - lineHeight / 2 }, value: "" });
          return;
        }
        if (tool) {
          const width = tool === "mosaic" ? MOSAIC_WIDTHS[sizeIdx] : STROKE_WIDTHS[sizeIdx];
          setDraft(
            tool === "pen"
              ? { kind: "pen", points: [p], color, width }
              : tool === "mosaic"
                ? { kind: "mosaic", points: [p], width }
                : { kind: tool, a: p, b: p, color, width },
          );
          next = { mode: "draw" };
        } else {
          next = { mode: "move", start: p, orig: sel };
        }
      } else if (shapes.length === 0 && !tool) {
        // Without annotations, dragging outside starts over with a new region.
        next = { mode: "create", start: p };
      }
    }
    if (!next) return;
    drag.current = next;
    setDragMode(next.mode);
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const p = pointOf(e);
    setPointer(p);
    const d = drag.current;
    if (!d) return;
    const bounds = { w: view.w, h: view.h };
    const clamped = { x: Math.min(Math.max(0, p.x), view.w), y: Math.min(Math.max(0, p.y), view.h) };
    switch (d.mode) {
      case "create":
        setSel(boxFrom(d.start, clamped));
        break;
      case "move":
        setSel(clampMove({ ...d.orig, x: d.orig.x + p.x - d.start.x, y: d.orig.y + p.y - d.start.y }, bounds));
        break;
      case "resize":
        setSel((s) => (s ? resizeBox(s, d.handle, p, bounds) : s));
        break;
      case "draw":
        setDraft((s) => {
          if (!s) return s;
          if (s.kind === "pen" || s.kind === "mosaic") return { ...s, points: [...s.points, p] };
          if (s.kind === "text") return s;
          let b = p;
          // Shift keeps rectangles square and ellipses circular.
          if (e.shiftKey && (s.kind === "rect" || s.kind === "ellipse")) {
            const side = Math.max(Math.abs(p.x - s.a.x), Math.abs(p.y - s.a.y));
            b = { x: s.a.x + Math.sign(p.x - s.a.x || 1) * side, y: s.a.y + Math.sign(p.y - s.a.y || 1) * side };
          }
          return { ...s, b };
        });
        break;
    }
  };

  const onPointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    setDragMode(null);
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    if (d.mode === "create") {
      // A click without a drag takes the whole screen.
      setSel((s) => (!s || s.w < 4 || s.h < 4 ? { x: 0, y: 0, w: view.w, h: view.h } : s));
    } else if (d.mode === "resize") {
      setSel((s) => (s && (s.w < 1 || s.h < 1) ? { ...s, w: Math.max(1, s.w), h: Math.max(1, s.h) } : s));
    } else if (d.mode === "draw") {
      const done = draftRef.current;
      if (done && isMeaningful(done)) setShapes((prev) => [...prev, done]);
      setDraft(null);
    }
  };

  const onDoubleClick = (e: React.MouseEvent) => {
    if (sel && !tool && !text && contains(sel, pointOf(e))) void finish("copy");
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    if (text) setText(null);
    else if (sel) {
      setSel(null);
      setShapes([]);
      setTool(null);
    } else void close();
  };

  // ── Keyboard ──

  const keyState = useRef({ sel, text, finish, undo, close, commitText });
  keyState.current = { sel, text, finish, undo, close, commitText };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = keyState.current;
      const mod = IS_MAC ? e.metaKey : e.ctrlKey;
      if (e.key === "Escape") {
        e.preventDefault();
        if (s.text) keyState.current.commitText();
        else void s.close();
        return;
      }
      if (s.text) {
        if (mod && e.key === "Enter") {
          e.preventDefault();
          s.commitText();
        }
        return;
      }
      if (e.key === "Enter" && s.sel) {
        e.preventDefault();
        void s.finish("copy");
      } else if (mod && e.code === "KeyZ") {
        e.preventDefault();
        s.undo();
      } else if (mod && e.code === "KeyC" && s.sel) {
        e.preventDefault();
        void s.finish("copy");
      } else if (mod && e.code === "KeyS" && s.sel) {
        e.preventDefault();
        void s.finish("save");
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  useEffect(() => {
    if (text) textRef.current?.focus();
  }, [text?.at]);

  // ── Rendering ──

  useLayoutEffect(() => {
    const c = canvasRef.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.round(view.w * dpr);
    const h = Math.round(view.h * dpr);
    if (c.width !== w || c.height !== h) {
      c.width = w;
      c.height = h;
    }
    const ctx = c.getContext("2d")!;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, w, h);
    if (!sel) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const all = draft ? [...shapes, draft] : shapes;
    drawShapes(ctx, all, sel, mosaicFor(all));
  }, [shapes, draft, sel, view, img, mosaicFor]);

  const showToolbar = !!sel && !!img && (dragMode === null || dragMode === "draw");
  useLayoutEffect(() => {
    const el = toolbarRef.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    if (r.width !== tbSize.w || r.height !== tbSize.h) setTbSize({ w: r.width, h: r.height });
  });

  const cursor = useMemo(() => {
    if (busy) return "progress";
    const d = drag.current;
    if (d?.mode === "resize") return handleCursor(d.handle);
    if (d?.mode === "move") return "move";
    if (d) return "crosshair";
    if (!sel || !pointer) return "crosshair";
    const h = hitHandle(sel, pointer);
    if (h) return handleCursor(h);
    if (contains(sel, pointer)) return tool === "text" ? "text" : tool ? "crosshair" : "move";
    return "default";
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sel, pointer, tool, busy, dragMode]);

  const k = img ? img.naturalWidth / view.w : 1;
  const showLoupe = !!img && !!pointer && (!sel || dragMode === "create" || dragMode === "resize");
  const tb = sel ? toolbarPosition(sel, tbSize, view) : { x: 0, y: 0 };

  return (
    <div
      className="shot-root"
      style={{ cursor }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
    >
      {url && <img className="shot-frame" src={url} alt="" draggable={false} />}
      <canvas ref={canvasRef} className="shot-canvas" />
      {img && !sel && <div className="shot-dim" />}
      {img && !sel && <div className="shot-hint">{t("screenshot.hint")}</div>}
      {sel && (
        <div className="shot-sel" style={{ left: sel.x, top: sel.y, width: sel.w, height: sel.h }}>
          {HANDLES.map((h) => {
            const c = handlePoint(sel, h);
            return <span key={h} className="shot-handle" style={{ left: c.x - sel.x, top: c.y - sel.y }} />;
          })}
        </div>
      )}
      {sel && (
        <div className="shot-size" style={{ left: sel.x, top: sel.y >= 26 ? sel.y - 26 : sel.y + 4 }}>
          {Math.round(sel.w * k)} × {Math.round(sel.h * k)}
        </div>
      )}
      {text && (
        <textarea
          ref={textRef}
          className="shot-text"
          value={text.value}
          rows={Math.max(1, text.value.split("\n").length)}
          onChange={(e) => setText({ ...text, value: e.target.value })}
          onPointerDown={(e) => e.stopPropagation()}
          style={{
            left: text.at.x,
            top: text.at.y,
            color,
            font: textFont(textSize),
            lineHeight: TEXT_LINE_HEIGHT,
            width: textWidth(text.value || " ", textSize) + textSize,
          }}
        />
      )}
      {showToolbar && (
        <div
          ref={toolbarRef}
          className="shot-toolbar"
          style={{ left: tb.x, top: tb.y }}
          onPointerDown={(e) => e.stopPropagation()}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          <div className="shot-bar">
            {TOOLS.map((item) => {
              const Icon = Icons[item.icon];
              return (
                <button
                  key={item.tool}
                  className={`shot-btn${tool === item.tool ? " active" : ""}`}
                  title={t(item.label)}
                  onClick={() => {
                    if (text) commitText();
                    setTool(tool === item.tool ? null : item.tool);
                  }}
                >
                  <Icon size={17} />
                </button>
              );
            })}
            <span className="shot-sep" />
            <button className="shot-btn" title={`${t("screenshot.undo")} (${MOD}Z)`} disabled={!shapes.length && !text} onClick={undo}>
              <Icons.undo size={17} />
            </button>
            <button className="shot-btn" title={`${t("screenshot.save")} (${MOD}S)`} disabled={busy} onClick={() => void finish("save")}>
              <Icons.download size={17} />
            </button>
            <span className="shot-sep" />
            <button className="shot-btn" title={`${t("screenshot.cancel")} (Esc)`} onClick={() => void close()}>
              <Icons.close size={17} />
            </button>
            <button className="shot-done" title={t("screenshot.doneTip")} disabled={busy} onClick={() => void finish("copy")}>
              <Icons.check size={15} sw={2} />
              {t("screenshot.done")}
            </button>
          </div>
          {tool && (
            <div className="shot-bar shot-options">
              {SIZE_LABELS.map((label, i) => (
                <button
                  key={label}
                  className={`shot-btn shot-size-btn${sizeIdx === i ? " active" : ""}`}
                  title={t(label)}
                  onClick={() => setSizeIdx(i)}
                >
                  {tool === "text" ? (
                    <span style={{ fontSize: 10 + i * 3, fontWeight: 600 }}>A</span>
                  ) : (
                    <span className="shot-dot" style={{ width: 4 + i * 3, height: 4 + i * 3 }} />
                  )}
                </button>
              ))}
              {tool !== "mosaic" && <span className="shot-sep" />}
              {tool !== "mosaic" &&
                COLORS.map((c) => (
                  <button
                    key={c}
                    className={`shot-swatch${color === c ? " active" : ""}`}
                    style={{ background: c }}
                    aria-label={c}
                    onClick={() => setColor(c)}
                  />
                ))}
            </div>
          )}
          {error && <div className="shot-error">{t("screenshot.failed", error)}</div>}
        </div>
      )}
      {showLoupe && img && pointer && <Loupe img={img} at={pointer} scale={k} view={view} />}
    </div>
  );
}

/** Magnified pixels around the pointer, for precise selection edges. */
function Loupe({ img, at, scale, view }: { img: HTMLImageElement; at: Pt; scale: number; view: { w: number; h: number } }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const W = 120;
  const H = 84;
  const ZOOM = 6;
  useLayoutEffect(() => {
    const c = ref.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = W * dpr;
    c.height = H * dpr;
    const ctx = c.getContext("2d")!;
    ctx.imageSmoothingEnabled = false;
    const sw = (W / ZOOM) * scale;
    const sh = (H / ZOOM) * scale;
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, c.width, c.height);
    ctx.drawImage(img, at.x * scale - sw / 2, at.y * scale - sh / 2, sw, sh, 0, 0, c.width, c.height);
    ctx.strokeStyle = "rgba(45, 189, 110, 0.8)";
    ctx.lineWidth = dpr;
    ctx.beginPath();
    ctx.moveTo(c.width / 2, 0);
    ctx.lineTo(c.width / 2, c.height);
    ctx.moveTo(0, c.height / 2);
    ctx.lineTo(c.width, c.height / 2);
    ctx.stroke();
  }, [img, at, scale]);
  const left = at.x + 20 + W > view.w ? at.x - 20 - W : at.x + 20;
  const top = at.y + 20 + H + 22 > view.h ? at.y - 20 - H - 22 : at.y + 20;
  return (
    <div className="shot-loupe" style={{ left, top }}>
      <canvas ref={ref} style={{ width: W, height: H }} />
      <div className="shot-loupe-info">
        {Math.round(at.x * scale)}, {Math.round(at.y * scale)}
      </div>
    </div>
  );
}
