//! Server region: the international service (velaterm.com, downloads on dl.velaterm.com) or the mainland China
//! mirror (velaterm.cn, downloads on dl.velaterm.cn). Both serve the same content; the China mirror is faster
//! and more reliable from inside mainland China.
//!
//! The choice is the synchronized preference `vlx-server-region`. The backend reads the same app_settings key
//! for update checks, update downloads, vela-server downloads and the model catalog (`server_region.rs`); this
//! module covers the website links the page opens itself.
//!
//! Account features (sign-in, Remote, public sharing) stay on velaterm.com in both regions, because their
//! sessions and relay tunnels are bound to that origin.

import { useSyncExternalStore } from "react";

import { flushNow, pushSetting } from "./settingsSync";

export type ServerRegion = "global" | "cn";

export const SERVER_REGION_KEY = "vlx-server-region";

const ORIGINS: Record<ServerRegion, string> = {
  global: "https://velaterm.com",
  cn: "https://velaterm.cn",
};

/** Read the saved region; anything unrecognized, or unavailable storage, means the international service. */
export function loadServerRegion(): ServerRegion {
  try {
    return localStorage.getItem(SERVER_REGION_KEY) === "cn" ? "cn" : "global";
  } catch {
    return "global";
  }
}

const listeners = new Set<() => void>();

/** Save the region locally and to the backend, which switches its own requests on the next one. */
export function saveServerRegion(region: ServerRegion): void {
  try {
    localStorage.setItem(SERVER_REGION_KEY, region);
  } catch {
    /* The backend copy still applies; this shell falls back to the default on its next launch. */
  }
  pushSetting(SERVER_REGION_KEY, region);
  // Write through now instead of after the debounce, so an update check right after switching already
  // goes to the chosen server. Bounded: a half-open socket must not leave this pending forever.
  void flushNow(2000);
  notifyServerRegionChanged();
}

/** Re-render subscribers after the value changed, including when another shell's choice was reconciled in. */
export function notifyServerRegionChanged(): void {
  for (const fn of listeners) fn();
}

export function useServerRegion(): ServerRegion {
  return useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    loadServerRegion,
    () => "global",
  );
}

/** Website origin for the selected region, without a trailing slash. */
export function siteOrigin(region: ServerRegion = loadServerRegion()): string {
  return ORIGINS[region];
}

/** Absolute website URL for `path` (starting with "/") on the selected region. */
export function siteUrl(path: string, region: ServerRegion = loadServerRegion()): string {
  return `${ORIGINS[region]}${path}`;
}
