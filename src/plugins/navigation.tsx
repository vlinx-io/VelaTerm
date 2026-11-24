//! Unified query-parameter routing and CenterPane tab lifecycle for all loaded plugins.
//!
//! Uses `?plugin=<pluginId>&pluginRoute=<subRoute>` so every plugin automatically gets:
//! - Mutual exclusion with Memory/Knowledge/Security overlays
//! - TabBar pinning and restoration of the previous session/document tab when closed
//! - Optional right-panel hiding based on the plugin manifest

import { useEffect, useSyncExternalStore, type AnchorHTMLAttributes } from "react";
import { useTermStore } from "../store/termStore";

export const PLUGIN_NAV_EVENT = "plugin:navigate";

export interface ActivePluginRoute {
  pluginId: string;
  route: string;
}

export function pluginUrl(pluginId: string | null, subRoute?: string | null): string {
  const url = new URL(window.location.href);
  if (pluginId) {
    url.searchParams.set("plugin", pluginId);
    if (subRoute) url.searchParams.set("pluginRoute", subRoute);
    else url.searchParams.delete("pluginRoute");
    // Avoid overlapping with built-in CenterPane overlays.
    url.searchParams.delete("memory");
    url.searchParams.delete("knowledge");
    url.searchParams.delete("security");
  } else {
    url.searchParams.delete("plugin");
    url.searchParams.delete("pluginRoute");
  }
  // Tauri can open at tauri://localhost with an empty pathname. Passing "" to
  // pushState/replaceState is a no-op that leaves ?plugin=... stuck in the URL,
  // so always emit a full URL.
  return url.href;
}

export function clearPluginSearchParams(onlyPluginId?: string): void {
  const url = new URL(window.location.href);
  const current = url.searchParams.get("plugin");
  if (!current) return;
  if (onlyPluginId && current !== onlyPluginId) return;
  url.searchParams.delete("plugin");
  url.searchParams.delete("pluginRoute");
  window.history.replaceState(null, "", url.href);
  window.dispatchEvent(new Event(PLUGIN_NAV_EVENT));
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export function readPluginRoute(search: string): ActivePluginRoute | null {
  const params = new URLSearchParams(search);
  const pluginId = params.get("plugin");
  if (!pluginId) return null;
  const route = params.get("pluginRoute") || "overview";
  return { pluginId, route };
}

export function pluginNavigate(href: string): void {
  window.history.pushState(null, "", href);
  window.dispatchEvent(new Event(PLUGIN_NAV_EVENT));
  window.dispatchEvent(new PopStateEvent("popstate"));
}

function subscribe(cb: () => void): () => void {
  window.addEventListener("popstate", cb);
  window.addEventListener(PLUGIN_NAV_EVENT, cb);
  return () => {
    window.removeEventListener("popstate", cb);
    window.removeEventListener(PLUGIN_NAV_EVENT, cb);
  };
}

export function usePluginLocation(): string {
  return useSyncExternalStore(
    subscribe,
    () => window.location.search,
    () => "",
  );
}

/** Coordinates `?plugin=...` tab selection with existing session/document tabs, restoring the prior tab on close. */
export function usePluginTab(): void {
  useEffect(() => {
    let selected = false;
    let updating = false;
    let previous: string | null = null;

    const clearSelection = () => {
      updating = true;
      useTermStore.setState({ activeTabId: null, activeSessionId: null, focusedPaneId: null });
      updating = false;
    };

    const sync = () => {
      const open = !!readPluginRoute(window.location.search);
      if (open && !selected) {
        previous = useTermStore.getState().activeTabId;
        selected = true;
        clearSelection();
      } else if (!open && selected) {
        selected = false;
        const params = new URLSearchParams(window.location.search);
        const otherOverlay = !!params.get("memory") || !!params.get("newSession");
        if (!otherOverlay) {
          const state = useTermStore.getState();
          if (!state.activeTabId) {
            const target =
              previous && state.openTabs.includes(previous) ? previous : state.openTabs[0];
            if (target) state.setActiveTab(target);
          }
        }
      }
    };

    const unsubscribe = useTermStore.subscribe((state, before) => {
      if (updating || !selected || !state.activeTabId || state.activeTabId === before.activeTabId) {
        return;
      }
      const target = state.activeTabId;
      if (state.projects !== before.projects || state.sidebarTreeViews !== before.sidebarTreeViews) {
        clearSelection();
        if (!previous || !state.openTabs.includes(previous)) previous = target;
        return;
      }
      previous = target;
      selected = false;
      clearPluginSearchParams();
    });

    window.addEventListener("popstate", sync);
    window.addEventListener(PLUGIN_NAV_EVENT, sync);
    sync();

    return () => {
      unsubscribe();
      window.removeEventListener("popstate", sync);
      window.removeEventListener(PLUGIN_NAV_EVENT, sync);
    };
  }, []);
}

export function PluginLink({
  pluginId,
  route,
  onClick,
  children,
  ...rest
}: Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> & {
  pluginId: string;
  route?: string;
}) {
  const href = pluginUrl(pluginId, route);
  return (
    <a
      href={href}
      {...rest}
      onClick={(e) => {
        onClick?.(e);
        if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0) {
          return;
        }
        e.preventDefault();
        pluginNavigate(href);
      }}
    >
      {children}
    </a>
  );
}
