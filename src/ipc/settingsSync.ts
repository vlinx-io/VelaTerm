//! Cross-shell synchronization for application preferences using a local cache plus an authoritative backend.
//!
//! Theme, language, appearance, shortcuts, sound, and similar preferences originally lived only in each shell
//! WebView's localStorage. Tauri uses WKWebView and Electron uses Chromium, whose separate stores cannot be unified
//! by sharing a data directory. These keys therefore also live in backend SQLite table app_settings (see
//! `src-tauri/src/db/schema.rs`), allowing shells that share a database to share preferences.
//!
//! Confirmed model:
//! - **localStorage is a synchronous cache** available on the first frame with **no flash**; the **backend is authoritative**.
//! - Startup `reconcileSettings()` compares them. If the backend **has** a key, overwrite local storage and add
//!   differing values to a change set for callers to reapply, causing **at most one** flash. If the backend
//!   **lacks** it, seed the backend from local storage without changing the local value or flashing.
//! - Changes within one shell call `pushSetting()` to write to the backend in parallel with debounce. Both remain
//!   aligned, so that shell never flashes. Only the first startup after another shell changed a still-stale cache
//!   can flash once; afterward the refreshed cache remains aligned.
//!
//! Race gate: never let `pushSetting` propagate before `reconcileSettings()` finishes. Otherwise, persistence
//! triggered by startup `applyAppearance` could push this shell's **stale** value before reconciliation and
//! overwrite the authoritative backend. That startup write only repeats the local cache, so dropping it is safe;
//! reconciliation handles the value consistently.

import { invoke } from "./transport";

/** localStorage keys synchronized across shells, each matching its module constant.
 *  vlx-theme=color scheme / vlx-lang=language / vlx-sound=notification sound /
 *  vlx-notify=master system-notification switch / vlx-clean-images=automatic pasted-image cleanup /
 *  vlx-record-sessions=session recording switch used by backend spawn / vlx-settings=main settings block for
 *  appearance, terminal, behavior, advanced options, and shortcuts. */
export const SYNCED_KEYS = [
  "vlx-theme",
  "vlx-lang",
  "vlx-sound",
  "vlx-notify",
  "vlx-clean-images",
  "vlx-record-sessions",
  "vlx-settings",
] as const;
const SYNCED = new Set<string>(SYNCED_KEYS);

/** Do not propagate before reconciliation completes; see the module's race-gate note. */
let syncEnabled = false;
/** Pending debounced propagation buffer (key → value, last write wins). */
let pending: Record<string, string> = {};
let timer: ReturnType<typeof setTimeout> | null = null;
const PUSH_DEBOUNCE_MS = 400;

/** Called when a user changes a preference to write it to the backend with debounce. Ignore calls before
 *  reconciliation or for unsynchronized keys. Call alongside localStorage.setItem: localStorage remains the
 *  local cache and the backend remains authoritative. */
export function pushSetting(key: string, value: string): void {
  if (!syncEnabled || !SYNCED.has(key)) return;
  pending[key] = value;
  if (timer) clearTimeout(timer);
  timer = setTimeout(flush, PUSH_DEBOUNCE_MS);
}

function flush(): void {
  void flushNow();
}

/** Flush pending writes to the backend immediately and await completion, bypassing debounce. Use when a setting
 *  is needed immediately after writing—for example, when an install card fills an agent path and restarts the
 *  session. Backend spawn reads the path from app_settings at that moment and would see the old value if it were
 *  still waiting in the 400 ms debounce window. */
export async function flushNow(): Promise<void> {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  const entries = pending;
  pending = {};
  if (Object.keys(entries).length === 0) return;
  await invoke("set_app_settings", { entries }).catch(() => {
    /* Fail silently when the backend is unavailable; localStorage still serves this shell as its local cache. */
  });
}

/** `backendReached: false` leaves the cache unverified, so a caller must not write shared settings from it. */
export interface ReconcileResult {
  changed: Set<string>;
  backendReached: boolean;
}

/** Startup reconciliation against the authoritative backend, returning the keys a caller must reapply.
 *  An unavailable backend leaves the local cache unchanged and still enables propagation. */
export async function reconcileSettings(): Promise<ReconcileResult> {
  const changed = new Set<string>();
  let backendReached = false;
  try {
    const backend = await invoke<Record<string, string>>("get_app_settings");
    backendReached = true;
    const seed: Record<string, string> = {};
    for (const key of SYNCED_KEYS) {
      const local = localStorage.getItem(key);
      const remote = backend[key];
      if (remote !== undefined) {
        if (remote !== local) {
          localStorage.setItem(key, remote);
          changed.add(key);
        }
      } else if (local !== null) {
        // The backend lacks this key: seed it from local storage. During initial migration, the first shell started becomes authoritative.
        seed[key] = local;
      }
    }
    if (Object.keys(seed).length > 0) {
      await invoke("set_app_settings", { entries: seed }).catch(() => {});
    }
  } catch {
    /* If the backend is unavailable, use the local cache without changing it. */
  } finally {
    syncEnabled = true;
  }
  return { changed, backendReached };
}
