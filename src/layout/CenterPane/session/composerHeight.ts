import { useEffect, useState, useSyncExternalStore } from "react";

// The height the composer's text box was dragged to. One value for every chat pane, kept across restarts;
// null means the box keeps its stylesheet height.
const STORAGE_KEY = "vlx-composer-input-height";
export const COMPOSER_MIN_HEIGHT = 34;
/** What the running-turn hint takes from the text box (22px hint + 8px gap), so the composer keeps its size. */
export const COMPOSER_FEEDBACK_HEIGHT = 30;

/** Whether the text box can give up that height and still show a line; otherwise the feedback must not sit in the flow. */
export function composerHasFeedbackRoom(draggedHeight: number | null, shortPane: boolean) {
  return draggedHeight !== null ? draggedHeight >= COMPOSER_MIN_HEIGHT + COMPOSER_FEEDBACK_HEIGHT : !shortPane;
}
const listeners = new Set<() => void>();

function read(): number | null {
  try {
    const value = Number(localStorage.getItem(STORAGE_KEY));
    return Number.isFinite(value) && value >= COMPOSER_MIN_HEIGHT ? value : null;
  } catch {
    return null;
  }
}

let current = read();

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

export function useComposerHeight() {
  return useSyncExternalStore(subscribe, () => current);
}

export function setComposerHeight(height: number | null) {
  current = height === null ? null : Math.round(height);
  try {
    if (current === null) localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, String(current));
  } catch {
    // Storage can be unavailable; the height still applies for this run.
  }
  listeners.forEach((listener) => listener());
}

/**
 * Drag the top edge of the composer: moving up makes the text box taller, since the composer is anchored
 * to the bottom of the pane. The box never grows past most of the pane, so some transcript stays visible.
 */
export function startComposerResize(e: React.MouseEvent<HTMLElement>, input: HTMLTextAreaElement | null, heightOffset = 0) {
  if (!input || e.button !== 0) return;
  e.preventDefault();
  const startY = e.clientY;
  const startHeight = input.getBoundingClientRect().height + heightOffset;
  const pane = input.closest(".sv-composer")?.parentElement;
  const max = Math.max(COMPOSER_MIN_HEIGHT, Math.min(window.innerHeight * 0.7, (pane?.clientHeight ?? window.innerHeight) * 0.6));
  const previousCursor = document.body.style.cursor;
  const previousUserSelect = document.body.style.userSelect;
  const move = (ev: MouseEvent) => {
    setComposerHeight(Math.max(COMPOSER_MIN_HEIGHT, Math.min(max, startHeight + startY - ev.clientY)));
  };
  const stop = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", stop);
    window.removeEventListener("blur", stop);
    document.body.style.cursor = previousCursor;
    document.body.style.userSelect = previousUserSelect;
  };
  document.body.style.cursor = "row-resize";
  document.body.style.userSelect = "none";
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", stop);
  window.addEventListener("blur", stop);
}

/** Below this pane height the composer starts at one line, so a very short pane keeps room for the conversation. */
const SHORT_PANE_HEIGHT = 320;

/** Tracks whether a session pane is short. Attach the returned ref to the pane's root element. */
export function useShortPane(): [(element: HTMLElement | null) => void, boolean] {
  const [element, setElement] = useState<HTMLElement | null>(null);
  const [short, setShort] = useState(false);
  useEffect(() => {
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      // A hidden pane measures zero; it keeps its state until it is shown again.
      const height = element.clientHeight;
      if (height > 0) setShort(height < SHORT_PANE_HEIGHT);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [element]);
  return [setElement, short];
}
