//! Select the mobile layout only outside Tauri and when requested via `?view=` or on a narrow touch device.
//!
//! This decision is made once at startup where bootstrap.tsx branches. Resizing does not switch layouts:
//! desktop keeps every terminal mounted, while mobile mounts a single terminal at a time, so a live
//! switch would corrupt xterm ownership. Screen rotation therefore does not change the selected view.

export function isMobileView(): boolean {
  if (typeof window === "undefined") return false;
  // Desktop shells never use the mobile view. Tauri (`__TAURI_INTERNALS__`, including remote wry
  // WebViews) and Electron (`__VLX_ELECTRON__`, injected by preload) are always desktop contexts.
  if ("__TAURI_INTERNALS__" in window || "__VLX_ELECTRON__" in window) return false;
  // Manual override: the `?view=desktop|mobile` query parameter takes precedence over detection.
  // Earlier builds persisted a `vlx-view-mode` choice in localStorage; it is deliberately ignored
  // now, because the mobile view no longer offers a switch and a stored "desktop" had no way back.
  const param = new URLSearchParams(window.location.search).get("view");
  if (param === "mobile") return true;
  if (param === "desktop") return false;
  // Auto-detect a coarse pointer and a viewport shorter side below 768px. Tablets can opt in with `?view=mobile`.
  const coarse = window.matchMedia?.("(pointer: coarse)").matches ?? false;
  const shortSide = Math.min(window.innerWidth, window.innerHeight);
  return coarse && shortSide < 768;
}
