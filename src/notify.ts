//! System notification wrapper that requests permission on first use, then sends directly:
//! - Main Tauri window: call local `native_notify` for clickable native notifications (macOS
//!   UNUserNotificationCenter, Windows WinRT toasts), falling back to the official plugin.
//! - Remote window: unable to call application commands from its remote context, so emit
//!   `vlx://remote-notify` for the local backend to relay; use the official plugin for permission checks.
//! - Browser/mobile: use the Web Notification API.
//! Native local commands use bare invoke from `@tauri-apps/api/core`, never transport. In remote windows,
//! transport forwards over WebSocket to the remote host and cannot reach the local notification service.

import { invoke as nativeInvoke } from "@tauri-apps/api/core";
import { isTauri, isRemoteWindow } from "./ipc/transport";

/**
 * Whether the local native notification plugin is available: either a main Tauri window or a remote
 * window, which uses WebSocket with `isTauri=false` but retains `__TAURI_INTERNALS__` for local plugin
 * invocation. Remote wry/WKWebView windows do not support Web Notifications; browser/mobile clients do.
 */
const useNativeNotify = isTauri || isRemoteWindow;

/** Coarse host platform from the user agent, matching detectOs in settingsBehaviorFields. */
const UA =
  (typeof navigator !== "undefined" &&
    (navigator.userAgent || (navigator as unknown as { platform?: string }).platform)) ||
  "";
const isMac = /Mac|iPhone|iPad/i.test(UA);
const isWindows = !isMac && /Win/i.test(UA);

/**
 * Whether a clickable native notification channel exists: macOS UNUserNotificationCenter, or Windows
 * WinRT toasts. Both carry the window label and session ID so a click navigates to that session, which
 * the official plugin cannot do. Linux has no native channel and always uses the plugin.
 */
const hasNativeChannel = isMac || isWindows;

let granted: boolean | null = null;

/** Ensure notification permission, caching the result to avoid repeated OS queries. */
async function ensurePermission(): Promise<boolean> {
  if (granted !== null) return granted;
  try {
    if (useNativeNotify) {
      const { isPermissionGranted, requestPermission } = await import(
        "@tauri-apps/plugin-notification"
      );
      granted = await isPermissionGranted();
      if (!granted) {
        granted = (await requestPermission()) === "granted";
      }
    } else if (typeof Notification !== "undefined") {
      granted =
        Notification.permission === "granted" ||
        (await Notification.requestPermission()) === "granted";
    } else {
      granted = false;
    }
  } catch {
    granted = false;
  }
  return granted;
}

/** System notification permission: granted, denied, not yet requested, or unsupported. */
export type NotifyPermission = "granted" | "denied" | "default" | "unsupported";

/**
 * Read notification permission without prompting. The native plugin exposes only a granted boolean,
 * so ungranted maps to `default`; callers distinguish denial after an explicit request. Browsers
 * expose all three states through `Notification.permission`.
 */
export async function getNotifyPermission(): Promise<NotifyPermission> {
  try {
    if (useNativeNotify) {
      const { isPermissionGranted } = await import(
        "@tauri-apps/plugin-notification"
      );
      return (await isPermissionGranted()) ? "granted" : "default";
    }
    if (typeof Notification !== "undefined") {
      return Notification.permission as NotifyPermission;
    }
    return "unsupported";
  } catch {
    return "unsupported";
  }
}

/**
 * Request system notification permission, return the resulting state, and refresh this module's cache.
 * Native clients return immediately if already granted; otherwise they request OS authorization.
 * Previously denied macOS requests return `denied` without another prompt so callers can show guidance.
 */
export async function requestNotifyPermission(): Promise<NotifyPermission> {
  granted = null; // Clear the cache and repopulate it from this request.
  try {
    if (useNativeNotify) {
      const { isPermissionGranted, requestPermission } = await import(
        "@tauri-apps/plugin-notification"
      );
      if (await isPermissionGranted()) {
        granted = true;
        return "granted";
      }
      const res = await requestPermission();
      granted = res === "granted";
      return res === "granted" ? "granted" : "denied";
    }
    if (typeof Notification !== "undefined") {
      const res = await Notification.requestPermission();
      granted = res === "granted";
      return res as NotifyPermission;
    }
    return "unsupported";
  } catch {
    return "unsupported";
  }
}

/**
 * Query permission through macOS UNUserNotificationCenter first, which is more accurate than the plugin
 * under ad-hoc signing. Fall back to `getNotifyPermission` when the command returns `unsupported`, as in
 * non-macOS, development, or unbundled environments. Used to decide whether to show settings guidance.
 */
export async function getEffectiveNotifyPermission(): Promise<NotifyPermission> {
  // Remote windows cannot call native_notify_auth_status. Their local plugin queries the same host
  // permission used by backend native notifications, so use it as the fallback.
  if (useNativeNotify && !isRemoteWindow) {
    try {
      const s = await nativeInvoke<string>("native_notify_auth_status");
      if (s === "authorized" || s === "provisional" || s === "ephemeral")
        return "granted";
      if (s === "denied") return "denied";
      if (s === "notDetermined") return "default";
      // `unsupported` falls through to the official plugin check.
    } catch {
      /* Fall back to the official plugin. */
    }
  }
  return getNotifyPermission();
}

/**
 * Request permission. On macOS, show the native prompt for `notDetermined`, waiting up to 60 seconds,
 * or return an existing decision. Other platforms and fallback paths use requestNotifyPermission.
 */
export async function requestEffectiveNotifyPermission(): Promise<NotifyPermission> {
  // Remote windows cannot call application commands, so request through the local official plugin.
  if (useNativeNotify && !isRemoteWindow) {
    try {
      const s = await nativeInvoke<string>("native_notify_auth_status");
      if (s !== "unsupported") {
        if (s === "notDetermined") {
          const granted = await nativeInvoke<boolean>("native_notify_request_auth");
          return granted ? "granted" : "denied";
        }
        if (s === "authorized" || s === "provisional" || s === "ephemeral")
          return "granted";
        return "denied";
      }
    } catch {
      /* Fall back to the official plugin. */
    }
  }
  return requestNotifyPermission();
}

/**
 * System sound name for plugin-based fallback notifications, per platform.
 *
 * The name is platform-specific and silently dropped when unknown. On Windows the plugin routes
 * through notify-rust, which parses the name with `winrt_notification::Sound::from_str` and falls
 * back to `None` on failure; `None` emits `<audio silent="true"/>`, so a macOS name such as "Ping"
 * produces a visible but mute toast. Only "Default", "IM", "Mail", "Reminder", "SMS" and the
 * loopable "Alarm*"/"Call*" names parse there, so Windows uses "Default".
 */
const FALLBACK_SOUND = isWindows ? "Default" : "Ping";

/**
 * Send a system notification, quietly skipping it without permission or on failure. Callers decide
 * whether a notification is appropriate. `sound` requests the system alert sound.
 */
export async function notify(
  sessionId: string | null,
  title: string,
  body: string,
  sound = false,
): Promise<void> {
  try {
    const ok = await ensurePermission();
    if (!ok) return;
    if (useNativeNotify) {
      if (isRemoteWindow) {
        // Remote contexts cannot call native_notify, so emit `vlx://remote-notify` for the local
        // backend to relay. Include the window label in the notification identifier so clicking
        // activates that window, plus session metadata. Linux has no native relay and uses the plugin.
        if (sessionId && hasNativeChannel) {
          try {
            const { emit } = await import("@tauri-apps/api/event");
            const { getCurrentWindow } = await import("@tauri-apps/api/window");
            await emit("vlx://remote-notify", {
              windowLabel: getCurrentWindow().label,
              sessionId,
              title,
              body,
              sound,
            });
            return;
          } catch {
            /* If relay fails, use the local plugin without click-to-session navigation. */
          }
        }
        const { sendNotification } = await import(
          "@tauri-apps/plugin-notification"
        );
        sendNotification({
          title,
          body,
          ...(sound ? { sound: FALLBACK_SOUND } : {}),
        });
        return;
      }
      // In the main window, try the clickable native channel: UNUserNotificationCenter on macOS,
      // WinRT toasts on Windows. Fall back on false or invocation failure, including unbundled or
      // unsigned development builds. Linux goes directly through the official plugin.
      if (sessionId && hasNativeChannel) {
        try {
          const native = await nativeInvoke<boolean>("native_notify", {
            sessionId,
            title,
            body,
            sound,
          });
          if (native) return;
        } catch {
          /* Fall back to the official plugin. */
        }
      }
      const { sendNotification } = await import(
        "@tauri-apps/plugin-notification"
      );
      sendNotification({
        title,
        body,
        ...(sound ? { sound: FALLBACK_SOUND } : {}),
      });
    } else if (typeof Notification !== "undefined") {
      // Browser notifications focus the window and navigate to the associated session when clicked.
      const n = new Notification(title, { body });
      n.onclick = () => {
        window.focus();
        // Import the store lazily to avoid a top-level notify.ts ↔ termStore cycle.
        if (sessionId) {
          void import("./store/termStore").then((m) =>
            m.useTermStore.getState().openSession(sessionId),
          );
        }
      };
    }
  } catch {
    /* Notification failures must not affect the main flow. */
  }
}
