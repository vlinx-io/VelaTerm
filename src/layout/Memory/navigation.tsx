//! Query routes work on Tauri, Electron and the existing web entry without another server route.
import { useSyncExternalStore, type AnchorHTMLAttributes } from "react";
const subscribers = new Set<() => void>();
let acceptedUrl = "";
let acceptedState: Record<string, unknown> | null;
let restoringHistory = false;
const historyIndex = (state: unknown): number | null => {
  const index = (state as { memoryHistoryIndex?: unknown } | null)?.memoryHistoryIndex;
  return typeof index === "number" && Number.isSafeInteger(index) ? index : null;
};
function locationChanged(event: PopStateEvent) {
  // Native history traversal must ask the same draft guard as in-app links before unmounting the editor.
  if (restoringHistory) {
    restoringHistory = false;
  } else if (event.isTrusted && acceptedUrl && window.location.href !== acceptedUrl) {
    const allowed = window.dispatchEvent(new CustomEvent("memory:beforeNavigate", {
      cancelable: true, detail: { url: window.location.href, replace: false },
    }));
    if (!allowed) {
      const from = historyIndex(acceptedState);
      const to = historyIndex(window.history.state);
      if (from != null && to != null && from !== to) {
        restoringHistory = true;
        window.history.go(from - to);
      } else {
        // Entries created outside memory navigation may have no traversal index.
        window.history.pushState(acceptedState, "", acceptedUrl);
      }
      event.stopImmediatePropagation();
      return;
    }
  }
  acceptedUrl = window.location.href;
  acceptedState = window.history.state;
  subscribers.forEach((notify) => notify());
}
const subscribe = (fn: () => void) => {
  if (!subscribers.size) {
    if (historyIndex(window.history.state) == null) {
      window.history.replaceState({ ...window.history.state, memoryHistoryIndex: 0 }, "");
    }
    acceptedUrl = window.location.href;
    acceptedState = window.history.state;
    window.addEventListener("popstate", locationChanged, true);
  }
  subscribers.add(fn);
  return () => {
    subscribers.delete(fn);
    if (!subscribers.size) window.removeEventListener("popstate", locationChanged, true);
  };
};
export function useMemoryLocation() { return useSyncExternalStore(subscribe, () => window.location.search); }
export function memoryUrl(route = "library", values: Record<string, string | number | null> = {}) {
  const url = new URL(window.location.href);
  if (route) [...url.searchParams.keys()].filter((key) => key.startsWith("knowledge")).forEach((key) => url.searchParams.delete(key));
  if (route) url.searchParams.set("memory", route); else {
    [...url.searchParams.keys()].filter((key) => key.startsWith("memory")).forEach((key) => url.searchParams.delete(key));
  }
  for (const [key, value] of Object.entries(values)) {
    if (value == null || value === "") url.searchParams.delete(key); else url.searchParams.set(key, String(value));
  }
  // Tauri can open at tauri://localhost with no pathname. An empty relative
  // target keeps the current query in pushState, so closing must use a full URL.
  return url.href;
}
export function memoryNavigate(url: string, replace = false, force = false) {
  if (!force && !window.dispatchEvent(new CustomEvent("memory:beforeNavigate", { cancelable: true, detail: { url, replace } }))) return;
  const index = historyIndex(window.history.state) ?? 0;
  window.history[replace ? "replaceState" : "pushState"]({ ...window.history.state, memoryHistoryIndex: index + (replace ? 0 : 1) }, "", url);
  window.dispatchEvent(new PopStateEvent("popstate"));
}
export function MemoryLink({ route, values, ...props }: AnchorHTMLAttributes<HTMLAnchorElement> & { route: string; values?: Record<string, string | number | null> }) {
  const href = memoryUrl(route, values);
  return <a {...props} href={href} onClick={(e) => {
    props.onClick?.(e);
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0) return;
    e.preventDefault(); memoryNavigate(href);
  }} />;
}
