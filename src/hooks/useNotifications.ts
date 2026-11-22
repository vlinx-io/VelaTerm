//! Notification lifecycle: focus tracking, stale-marker cleanup on foreground return without
//! navigation, delayed read clearing, and Dock badges.

import { useEffect } from "react";
import { platform } from "../platform";
import { getNotifyPermission, requestNotifyPermission } from "../notify";
import { useTermStore } from "../store/termStore";

/**
 * Delay before clearing a viewed notification. It also controls the transition from waiting-for-reply
 * to viewed: retain the former for two seconds after opening to avoid an instantaneous status change.
 */
const CLEAR_DELAY_MS = 2000;

export function useNotifications(): void {
  const activeSessionId = useTermStore((s) => s.activeSessionId);
  const notifications = useTermStore((s) => s.notifications);
  const windowFocused = useTermStore((s) => s.windowFocused);
  const setWindowFocused = useTermStore((s) => s.setWindowFocused);
  const focusReturned = useTermStore((s) => s.focusReturned);
  const clearNotification = useTermStore((s) => s.clearNotification);

  // Request system permission once at startup only when notifications are enabled and the decision
  // is still default. Desktop may show a system prompt; browsers may ignore gestureless requests,
  // which notify.ts handles without error.
  useEffect(() => {
    if (!useTermStore.getState().notifyEnabled) return;
    let cancelled = false;
    void getNotifyPermission().then((p) => {
      if (!cancelled && p === "default") void requestNotifyPermission();
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // Native notification clicks emit a session ID, focus its session, and raise the window. Desktop
  // and remote windows receive this event; browsers use Web Notification onclick in notify.ts.
  useEffect(() => {
    if (!platform.env.hasNativeHost) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void platform.window
      .onNotificationClick((id) => {
        if (!id) return;
        useTermStore.getState().openSession(id);
        // Some windows may lack setFocus ACL permission; the platform layer handles that safely.
        void platform.window.setFocus();
      })
      .then((u) => {
        if (!u) return;
        if (disposed) u();
        else unlisten = u;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Track focus to suppress notifications while visible and clear stale markers on foreground return.
  // Do not jump to the newest notification, which made focus jitter switch active work unexpectedly.
  // Desktop and remote windows use Tauri focus events; browsers/mobile fall back to DOM focus/blur.
  // Remote windows retain `__TAURI_INTERNALS__` despite WS transport, making onFocusChanged more
  // reliable than document.hasFocus() in macOS WKWebView. open_remote_window grants core:event.
  useEffect(() => {
    if (!platform.env.hasNativeHost) {
      const onFocus = () => {
        setWindowFocused(true);
        focusReturned();
      };
      const onBlur = () => setWindowFocused(false);
      window.addEventListener("focus", onFocus);
      window.addEventListener("blur", onBlur);
      setWindowFocused(document.hasFocus());
      return () => {
        window.removeEventListener("focus", onFocus);
        window.removeEventListener("blur", onBlur);
      };
    }
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void platform.window
      .onFocusChanged((focused) => {
        setWindowFocused(focused);
        if (focused) focusReturned();
      })
      .then((u) => {
        if (disposed) u();
        else unlisten = u;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [setWindowFocused, focusReturned]);

  // A Pending mark set on the active session holds only while it stays active; leaving it releases the hold,
  // so coming back later reads it as usual.
  const unreadHold = useTermStore((s) => s.unreadHold);
  useEffect(() => {
    if (unreadHold && unreadHold !== activeSessionId) useTermStore.setState({ unreadHold: null });
  }, [unreadHold, activeSessionId]);

  // Clear the active session's marker after it remains focused and visible for the delay.
  useEffect(() => {
    if (!windowFocused || !activeSessionId) return;
    if (!(activeSessionId in notifications)) return;
    if (unreadHold === activeSessionId) return;
    const t = setTimeout(() => clearNotification(activeSessionId), CLEAR_DELAY_MS);
    return () => clearTimeout(t);
  }, [windowFocused, activeSessionId, notifications, clearNotification, unreadHold]);

  // Per-window pending count equals unread sessions plus pending spawn confirmations. Desktop
  // hosts show it on each window's title and sum across open windows for the Dock badge; plain
  // browsers ignore it.
  const pendingSpawnCount = useTermStore((s) => s.pendingSpawns.length);
  const badgeCount = Object.keys(notifications).length + pendingSpawnCount;
  useEffect(() => {
    void platform.badge.setCount(badgeCount);
  }, [badgeCount]);
}
