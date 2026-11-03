//! Screenshot commands: hotkey settings and the overlay window (desktop shell only; see
//! src-tauri/src/screenshot.rs).
//!
//! Calls the native commands directly rather than through transport: a rejected shortcut is an
//! expected outcome shown next to the setting, not a request failure for the global error banner.

import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export interface ScreenshotShortcutStatus {
  /** Effective accelerator such as "Ctrl+Cmd+S"; empty when the hotkey is off. */
  shortcut: string;
  defaultShortcut: string;
  supported: boolean;
  /** Set when the stored accelerator could not be registered at startup. */
  error: string | null;
}

export const screenshotShortcutGet = () => invoke<ScreenshotShortcutStatus>("screenshot_shortcut_get");

/** Register and persist an accelerator; an empty string turns the hotkey off. Rejects when taken. */
export const screenshotShortcutSet = (shortcut: string) =>
  invoke<ScreenshotShortcutStatus>("screenshot_shortcut_set", { shortcut });

/** Start screen capture through the same native entry point as the global hotkey. */
export const screenshotStart = () => invoke<void>("screenshot_start");

// ── Overlay window (screenshot.html) ──

/** Subscribe to new screenshots. The overlay window is reused, so each one arrives as an event. */
export const onScreenshotFrame = (cb: (id: number) => void) =>
  getCurrentWebviewWindow().listen<number>("screenshot-frame", ({ payload }) => cb(payload));

/** Id of the screenshot whose frame is already waiting, for a page that loaded after the event. */
export const screenshotPending = () => invoke<number | null>("screenshot_pending");

/** PNG of the captured monitor. */
export const screenshotFrame = () => invoke<ArrayBuffer>("screenshot_frame");

/** The frame of screenshot `id` has painted: show the overlay and focus it. */
export const screenshotReady = (id: number) => invoke<void>("screenshot_ready", { id });

/** Discard the screenshot and close the overlay. */
export const screenshotClose = () => invoke<void>("screenshot_close");

/** Hand the edited PNG back as the raw request body: copy it, or ask where to save it. */
export const screenshotFinish = (action: "copy" | "save", png: Uint8Array) =>
  invoke<unknown>(action === "copy" ? "screenshot_copy" : "screenshot_save", png);

/** Subscribe to focus changes of the overlay window. */
export const onScreenshotFocusChanged = (cb: (focused: boolean) => void) =>
  getCurrentWindow().onFocusChanged(({ payload }) => cb(payload));
