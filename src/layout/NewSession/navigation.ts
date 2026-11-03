//! URL for New Session tabs. The tab ID and its location live in the query string, so a refresh or a pasted
//! link reopens the same tab for the same project; the typed draft itself stays in the mounted view.
import { useEffect, useRef } from "react";
import { useTermStore, type NewSessionTab, type NewSessionTarget } from "../../store/termStore";
import { memoryNavigate, useMemoryLocation } from "../Memory/navigation";
import { activeAgentLocation } from "../NewAgentSession/navigation";

const PARAMS = ["newSession", "newSessionProject", "newSessionGroup", "newSessionParent"];
const ID_PREFIX = "new-session-";

export function newSessionUrl(tab?: Pick<NewSessionTab, "id" | "projectId" | "groupId" | "parentSessionId"> | null): string {
  const url = new URL(window.location.href);
  PARAMS.forEach((key) => url.searchParams.delete(key));
  if (tab) {
    url.searchParams.set("newSession", tab.id);
    if (tab.projectId) url.searchParams.set("newSessionProject", tab.projectId);
    if (tab.groupId) url.searchParams.set("newSessionGroup", tab.groupId);
    if (tab.parentSessionId) url.searchParams.set("newSessionParent", tab.parentSessionId);
  }
  return url.href;
}

/** Open a New Session tab for a location; the navigation component then records it in the URL. */
export function openNewSession(target: NewSessionTarget) {
  useTermStore.getState().openNewSessionTab(target);
}

/** Open a New Session tab where the user is working: beside the active session, else in the selected project
 * or group. With nothing to go on, the tab opens without a project and lets the user pick one. */
export function openNewSessionHere() {
  const state = useTermStore.getState();
  const location = activeAgentLocation(state);
  const anchor = location.anchorId ? state.sessions.find((s) => s.id === location.anchorId) : undefined;
  openNewSession({ projectId: location.projectId, groupId: location.groupId, parentSessionId: anchor?.parentSessionId ?? null });
}

/** Link target for menus that offer New Session: a fresh tab ID, so opening it in place of a click still works. */
export function newSessionHref(target: NewSessionTarget): string {
  return newSessionUrl({ id: `${ID_PREFIX}link`, projectId: target.projectId,
    groupId: target.groupId ?? null, parentSessionId: target.parentSessionId ?? null });
}

/** Keeps the URL and the active New Session tab in step, in both directions. */
export function NewSessionNavigation() {
  const search = useMemoryLocation();
  const treeLoaded = useTermStore((state) => state.treeLoaded);
  const restoring = useRef(false);
  useEffect(() => {
    // Restoring the saved layout must finish first, or it would replace the tab opened here.
    if (!treeLoaded) return;
    const params = new URLSearchParams(window.location.search);
    const id = params.get("newSession");
    const state = useTermStore.getState();
    const active = state.activeTabId ? state.newSessionTabs[state.activeTabId] : undefined;
    restoring.current = true;
    if (id && id.startsWith(ID_PREFIX)) {
      if (active?.id !== id) {
        const opened = state.openNewSessionTab({
          projectId: params.get("newSessionProject") ?? "",
          groupId: params.get("newSessionGroup"),
          parentSessionId: params.get("newSessionParent"),
        }, state.newSessionTabs[id] || id !== `${ID_PREFIX}link` ? id : undefined);
        // A menu link carries a placeholder ID; record the real one without adding a history entry.
        if (opened !== id) {
          const tab = useTermStore.getState().newSessionTabs[opened];
          if (tab) memoryNavigate(newSessionUrl(tab), true);
        }
      }
    } else if (active) {
      const fallback = active.returnTabId && state.openTabs.includes(active.returnTabId)
        ? active.returnTabId : state.openTabs.find((tabId) => !state.newSessionTabs[tabId] && !state.taskTabs[tabId]);
      if (fallback) state.setActiveTab(fallback);
      else state.closeTab(active.id);
    }
    restoring.current = false;
  }, [search, treeLoaded]);
  useEffect(() => useTermStore.subscribe((state, before) => {
    if (!state.treeLoaded || !before.treeLoaded || restoring.current) return;
    const active = state.activeTabId ? state.newSessionTabs[state.activeTabId] : undefined;
    const previous = before.activeTabId ? before.newSessionTabs[before.activeTabId] : undefined;
    // Changing the project inside the tab updates its location without a new history entry.
    if (active && previous?.id === active.id) {
      if (active !== previous) {
        const href = newSessionUrl(active);
        if (href !== window.location.href) memoryNavigate(href, true);
      }
      return;
    }
    if (!active && !previous) return;
    const href = newSessionUrl(active);
    if (href !== window.location.href) memoryNavigate(href);
  }), []);
  return null;
}
