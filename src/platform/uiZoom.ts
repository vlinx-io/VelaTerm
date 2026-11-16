//! Whole-interface zoom (issue #113).
//!
//! Uses each shell's native page zoom rather than CSS `zoom` on the document: Tauri's `Webview.setZoom`
//! (WKWebView pageZoom / WebView2 ZoomFactor) and Electron's `webFrame.setZoomFactor`. Native zoom behaves
//! like a browser's Cmd +: every CSS pixel is scaled and redrawn at the new ratio, so pointer coordinates,
//! `getBoundingClientRect` and xterm's cell measurements stay in one consistent space. CSS `zoom` mixes
//! zoomed and unzoomed coordinates differently in WebKit and Chromium, which offsets terminal selection.
//!
//! The level is a per-device preference kept in localStorage only and never mirrored to the backend: the
//! right size depends on this machine's displays, not on the server a client connects to. Plain browsers
//! have their own zoom, so the control is offered only where a native host exists.

import { useSyncExternalStore } from "react";
// The unified environment view rather than named exports: test doubles provide the `env` object, and a
// missing extra export must not break module loading for pages that never zoom.
import { env } from "./env";

const STORAGE_KEY = "vlx-ui-zoom";

/** Zoom steps offered by the status-bar control, ascending. 1 is the default. */
export const UI_ZOOM_STEPS = [0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2] as const;
export const UI_ZOOM_MIN = UI_ZOOM_STEPS[0];
export const UI_ZOOM_MAX = UI_ZOOM_STEPS[UI_ZOOM_STEPS.length - 1];

interface ElectronZoomBridge {
  setZoomFactor?(factor: number): void;
}

function electronBridge(): ElectronZoomBridge | undefined {
  return (window as unknown as { vlxNative?: ElectronZoomBridge }).vlxNative;
}

/** Whether this window can apply native zoom: the local Tauri main window or the Electron shell. */
export const uiZoomSupported: boolean = env.isElectron || (env.isTauri && !env.isRemoteWindow);

/** Clamp to the supported range and round to two decimals; anything unreadable becomes 1. */
export function normalizeUiZoom(value: unknown): number {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return 1;
  return Math.round(Math.min(UI_ZOOM_MAX, Math.max(UI_ZOOM_MIN, n)) * 100) / 100;
}

/** Parse a typed zoom percentage: a whole number inside the supported range, with or without the
 *  trailing `%` sign and spacing before it. Returns the factor, or null when the text is not a valid
 *  level, so the caller can keep the current one. */
export function parseUiZoomPercent(text: string): number | null {
  const raw = text.normalize("NFKC").trim().replace(/\s*%$/, "").trim();
  if (!/^\d+$/.test(raw)) return null;
  const percent = Number(raw);
  if (!Number.isFinite(percent)) return null;
  if (percent < UI_ZOOM_MIN * 100 || percent > UI_ZOOM_MAX * 100) return null;
  return normalizeUiZoom(percent / 100);
}

function load(): number {
  if (!uiZoomSupported) return 1;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw == null ? 1 : normalizeUiZoom(raw);
  } catch {
    return 1;
  }
}

let current = load();
const listeners = new Set<() => void>();

/** Current zoom factor; 1 where native zoom is unsupported. */
export function getUiZoom(): number {
  return current;
}

async function applyNative(factor: number): Promise<void> {
  if (env.isElectron) {
    electronBridge()?.setZoomFactor?.(factor);
    return;
  }
  if (env.isTauri && !env.isRemoteWindow) {
    const { getCurrentWebview } = await import("@tauri-apps/api/webview");
    await getCurrentWebview().setZoom(factor);
  }
}

/** Store and apply a new level. Listeners update before the native call so the label never lags. */
export function setUiZoom(value: number): void {
  if (!uiZoomSupported) return;
  const next = normalizeUiZoom(value);
  if (next === current) return;
  current = next;
  try {
    if (next === 1) localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, String(next));
  } catch {
    /* Ignore unavailable localStorage; the level still applies for this run. */
  }
  for (const fn of listeners) fn();
  void applyNative(next).catch((e) => console.warn("[ui-zoom] apply failed", e));
}

/** Next step above (dir 1) or below (dir -1) the current level, or the current level at either end. */
export function stepUiZoom(dir: 1 | -1): number {
  if (dir > 0) return UI_ZOOM_STEPS.find((s) => s > current + 0.001) ?? current;
  return [...UI_ZOOM_STEPS].reverse().find((s) => s < current - 0.001) ?? current;
}

/** Re-apply the saved level at startup. The native zoom does not survive a relaunch on every shell. */
export function applyStoredUiZoom(): void {
  if (!uiZoomSupported) return;
  void applyNative(current).catch((e) => console.warn("[ui-zoom] apply failed", e));
}

function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** React binding for the current zoom factor. */
export function useUiZoom(): number {
  return useSyncExternalStore(subscribe, getUiZoom, getUiZoom);
}
