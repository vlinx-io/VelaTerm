//! Query routes work on Tauri, Electron and the existing web entry without another server route.
import { useSyncExternalStore, type AnchorHTMLAttributes } from "react";
const subscribe = (fn: () => void) => { window.addEventListener("popstate", fn); return () => window.removeEventListener("popstate", fn); };
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
  return `${url.pathname}${url.search}${url.hash}`;
}
export function memoryNavigate(url: string, replace = false, force = false) {
  if (!force && !window.dispatchEvent(new CustomEvent("memory:beforeNavigate", { cancelable: true, detail: { url, replace } }))) return;
  window.history[replace ? "replaceState" : "pushState"](null, "", url);
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
