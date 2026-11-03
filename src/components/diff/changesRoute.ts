//! URL state for the changes tab, so a diff survives a reload, can be bookmarked or shared, and
//! reopens with the browser's back and forward.
//!
//! The store stays the source of truth while the app runs. Showing a different comparison pushes a
//! history entry; choosing another file in it, or switching to another kind of tab, replaces it.
//! On load and on back/forward the URL opens or focuses the matching tab.

import { useEffect } from "react";
import type { GitChangeSide } from "../../ipc/commands";
import { memoryNavigate } from "../../layout/Memory/navigation";
import { useTermStore } from "../../store/termStore";

const KEYS = ["changes", "changesSide", "changesPath", "changesCommit", "changesBase", "changesBaseLabel"] as const;
const SIDES: GitChangeSide[] = ["all", "staged", "unstaged", "untracked", "conflicted"];

interface Route {
  cwd: string;
  side: GitChangeSide;
  path: string | null;
  commit: string | null;
  base: { from: string; label: string } | null;
}

export function readChangesRoute(search: string): Route | null {
  const params = new URLSearchParams(search);
  const cwd = params.get("changes");
  if (!cwd) return null;
  const side = params.get("changesSide") as GitChangeSide | null;
  const commit = params.get("changesCommit");
  const from = params.get("changesBase");
  return {
    cwd,
    side: side && SIDES.includes(side) ? side : "all",
    path: params.get("changesPath"),
    commit,
    base: commit && from ? { from, label: params.get("changesBaseLabel") ?? from.slice(0, 12) } : null,
  };
}

export function changesUrl(href: string, route: Route | null): string {
  const url = new URL(href);
  for (const key of KEYS) url.searchParams.delete(key);
  if (route) {
    url.searchParams.set("changes", route.cwd);
    if (route.side !== "all") url.searchParams.set("changesSide", route.side);
    if (route.path) url.searchParams.set("changesPath", route.path);
    if (route.commit) url.searchParams.set("changesCommit", route.commit);
    if (route.base) {
      url.searchParams.set("changesBase", route.base.from);
      url.searchParams.set("changesBaseLabel", route.base.label);
    }
  }
  return url.href;
}

/** The comparison on screen: the active tab's, when it is a changes tab. */
function storeRoute(state = useTermStore.getState()): Route | null {
  const target = state.activeTabId ? state.docTabs[state.activeTabId]?.changes : undefined;
  return target
    ? { cwd: target.cwd, side: target.side, path: target.path, commit: target.commit, base: target.base }
    : null;
}

const same = (a: Route | null, b: Route | null) => JSON.stringify(a) === JSON.stringify(b);

/** Keep the active changes tab and the URL in step. Mount once, at the app root. */
export function useChangesRoute(): void {
  useEffect(() => {
    // A changes URL opens its tab, or focuses it if open. Without one there is nothing to undo: leaving a
    // changes tab for another tab is not navigation, so the tabs stay as they are.
    const apply = () => {
      // Restoring the saved workspace replaces the open tabs, so wait for it.
      if (!useTermStore.getState().treeLoaded) return;
      const route = readChangesRoute(window.location.search);
      if (!route || same(route, storeRoute())) return;
      useTermStore.getState().openChanges(route.cwd, {
        path: route.path ?? undefined, side: route.side, commit: route.commit ?? undefined, base: route.base ?? undefined,
      });
    };
    apply();
    window.addEventListener("popstate", apply);
    const unsubscribe = useTermStore.subscribe((state, previous) => {
      if (state.treeLoaded && !previous.treeLoaded) { apply(); return; }
      // Until then the URL is a request still waiting to be applied, not a picture of the tabs: a mirrored
      // layout landing first must not overwrite it.
      if (!state.treeLoaded) return;
      const next = storeRoute(state);
      if (same(next, storeRoute(previous))) return;
      const shown = readChangesRoute(window.location.search);
      if (same(next, shown)) return;
      // Only a different comparison is a new place to go back to; picking a file in it, or switching to
      // another kind of tab, rewrites the current entry.
      const replace = !next || Boolean(shown && next.cwd === shown.cwd && next.commit === shown.commit
        && next.side === shown.side && JSON.stringify(next.base) === JSON.stringify(shown.base));
      // Forced: opening a changes tab never discards a draft, so the unsaved-draft guard has nothing to protect.
      memoryNavigate(changesUrl(window.location.href, next), replace, true);
    });
    return () => {
      window.removeEventListener("popstate", apply);
      unsubscribe();
    };
  }, []);
}

/** Mounts the URL sync at the app root; renders nothing. */
export function ChangesRoute(): null {
  useChangesRoute();
  return null;
}
