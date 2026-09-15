//! Pure tab-layout transforms that place existing sessions into split panes.
//!
//! Every pane leaf mounts its session's terminal, and unmounting a terminal kills its process on desktop. A
//! session moving between tabs therefore has to leave its old tree and enter its new one in the same store
//! update: CenterPane renders panes keyed by session ID, so a move inside one update keeps the mounted view and
//! the PTY behind it. The functions here only compute that update; the store commits it with a single `set`.
//!
//! Tabs are keyed by a session ID and the tab bar labels a tab with that session. When a move takes the keying
//! session out of a tab that keeps other panes, the tab is re-keyed to one of its remaining sessions, so the label
//! stays truthful and reopening the departed session can never overwrite the surviving tree.

import {
  buildGrid,
  collectSessionIds,
  findBySession,
  findLeaf,
  firstLeaf,
  GRID_MAX,
  makeLeaf,
  type PaneNode,
  removeSession,
  setLeafSession,
  splitAt,
  swapLeafSessions,
} from "../layout/CenterPane/paneTree";

/** The part of the store a placement reads and rewrites. */
export interface TabLayout {
  openTabs: string[];
  liveTabs: string[];
  pinnedTabs: string[];
  paneTrees: Record<string, PaneNode>;
  activeTabId: string | null;
  lastActiveSessionTabId: string | null;
}

/** A placement result: the rewritten layout plus the pane that receives focus. */
export interface PlacedLayout extends TabLayout {
  activeSessionId: string;
  focusedPaneId: string;
}

/** Copy only the layout fields. Callers pass the whole store state, and a result spread from it would carry stale
 *  copies of every other field into the store's `set`. */
function pickLayout(l: TabLayout): TabLayout {
  return {
    openTabs: l.openTabs,
    liveTabs: l.liveTabs,
    pinnedTabs: l.pinnedTabs,
    paneTrees: l.paneTrees,
    activeTabId: l.activeTabId,
    lastActiveSessionTabId: l.lastActiveSessionTabId,
  };
}

function renameTab(layout: TabLayout, from: string, to: string): TabLayout {
  const swap = (ids: string[]) => ids.map((id) => (id === from ? to : id));
  const paneTrees = { ...layout.paneTrees, [to]: layout.paneTrees[from] };
  delete paneTrees[from];
  return {
    openTabs: swap(layout.openTabs),
    liveTabs: swap(layout.liveTabs),
    pinnedTabs: swap(layout.pinnedTabs),
    paneTrees,
    activeTabId: layout.activeTabId === from ? to : layout.activeTabId,
    lastActiveSessionTabId:
      layout.lastActiveSessionTabId === from ? to : layout.lastActiveSessionTabId,
  };
}

/** Re-key a tab whose keying session is no longer inside its tree. Left unchanged when no remaining session is
 *  free to serve as a key. */
function rekeyIfOrphaned(layout: TabLayout, tabId: string): TabLayout {
  const tree = layout.paneTrees[tabId];
  if (!tree || findBySession(tree, tabId)) return layout;
  const key = collectSessionIds(tree).find((sid) => !layout.paneTrees[sid]);
  return key ? renameTab(layout, tabId, key) : layout;
}

/** Take sessions out of every visible and background tab without ending them. A tab left empty is removed.
 *  The caller must place the sessions again in the same store update, or their terminals unmount. */
export function detachSessions(layout: TabLayout, ids: ReadonlySet<string>): TabLayout {
  let next = pickLayout(layout);
  for (const tabId of [...layout.openTabs, ...layout.liveTabs]) {
    const tree = next.paneTrees[tabId];
    if (!tree) continue;
    let pruned: PaneNode | null = tree;
    for (const sid of collectSessionIds(tree)) {
      if (ids.has(sid) && pruned) pruned = removeSession(pruned, sid);
    }
    if (pruned === tree) continue;
    if (pruned === null) {
      const paneTrees = { ...next.paneTrees };
      delete paneTrees[tabId];
      const drop = (list: string[]) => list.filter((id) => id !== tabId);
      next = {
        openTabs: drop(next.openTabs),
        liveTabs: drop(next.liveTabs),
        pinnedTabs: drop(next.pinnedTabs),
        paneTrees,
        activeTabId: next.activeTabId === tabId ? null : next.activeTabId,
        lastActiveSessionTabId:
          next.lastActiveSessionTabId === tabId ? null : next.lastActiveSessionTabId,
      };
      continue;
    }
    next = rekeyIfOrphaned(
      { ...next, paneTrees: { ...next.paneTrees, [tabId]: pruned } },
      tabId,
    );
  }
  return next;
}

/** The visible tab whose tree holds a pane. */
function tabOfPane(layout: TabLayout, paneId: string): string | null {
  return (
    layout.openTabs.find((tabId) => {
      const tree = layout.paneTrees[tabId];
      return !!tree && !!findLeaf(tree, paneId);
    }) ?? null
  );
}

function focusOn(layout: TabLayout, tabId: string, sessionId: string): PlacedLayout | null {
  const leaf = findBySession(layout.paneTrees[tabId], sessionId);
  if (!leaf) return null;
  return {
    ...layout,
    activeTabId: tabId,
    lastActiveSessionTabId: tabId,
    activeSessionId: sessionId,
    focusedPaneId: leaf.paneId,
  };
}

/** Split a visible pane and show an existing session in the new half, moving it out of wherever it was shown.
 *  Returns null when the pane is not visible or already shows that session. */
export function placeInSplit(
  layout: TabLayout,
  sessionId: string,
  targetPaneId: string,
  dir: "horizontal" | "vertical",
  before: boolean,
): PlacedLayout | null {
  const start = tabOfPane(layout, targetPaneId);
  if (!start) return null;
  if (findLeaf(layout.paneTrees[start], targetPaneId)?.sessionId === sessionId) return null;
  const detached = detachSessions(layout, new Set([sessionId]));
  // Detaching may re-key the target's tab, but pane IDs survive removal, so look the tab up again.
  const tabId = tabOfPane(detached, targetPaneId);
  if (!tabId) return null;
  const tree = splitAt(detached.paneTrees[tabId], targetPaneId, dir, sessionId, before);
  return focusOn({ ...detached, paneTrees: { ...detached.paneTrees, [tabId]: tree } }, tabId, sessionId);
}

/** Show an existing session in a visible pane. A session already in the same tab trades places with the pane's
 *  current one. Otherwise the displaced session moves to its own background tab when `keepDisplaced` allows it,
 *  and is dropped from the layout when not. Returns null for a no-op, or when the displaced session would have
 *  to be dropped against `keepDisplaced` because no free tab key exists. */
export function placeInPane(
  layout: TabLayout,
  sessionId: string,
  targetPaneId: string,
  keepDisplaced: (sessionId: string) => boolean,
): (PlacedLayout & { backgrounded: string | null }) | null {
  const tabId = tabOfPane(layout, targetPaneId);
  if (!tabId) return null;
  const tree = layout.paneTrees[tabId];
  const target = findLeaf(tree, targetPaneId);
  if (!target || target.sessionId === sessionId) return null;

  const sibling = findBySession(tree, sessionId);
  if (sibling) {
    const swapped = swapLeafSessions(tree, targetPaneId, sibling.paneId);
    const base = pickLayout(layout);
    const placed = focusOn({ ...base, paneTrees: { ...base.paneTrees, [tabId]: swapped } }, tabId, sessionId);
    return placed && { ...placed, backgrounded: null };
  }

  const displaced = target.sessionId;
  const detached = detachSessions(layout, new Set([sessionId]));
  let next: TabLayout = {
    ...detached,
    paneTrees: {
      ...detached.paneTrees,
      [tabId]: setLeafSession(detached.paneTrees[tabId], targetPaneId, sessionId),
    },
  };
  next = rekeyIfOrphaned(next, tabId);
  const shownTab = tabOfPane(next, targetPaneId);
  if (!shownTab) return null;

  let backgrounded: string | null = null;
  if (keepDisplaced(displaced)) {
    if (next.paneTrees[displaced]) return null;
    next = {
      ...next,
      paneTrees: { ...next.paneTrees, [displaced]: makeLeaf(displaced) },
      liveTabs: [...next.liveTabs.filter((id) => id !== displaced), displaced],
    };
    backgrounded = displaced;
  }
  const placed = focusOn(next, shownTab, sessionId);
  return placed && { ...placed, backgrounded };
}

/** Open up to GRID_MAX existing sessions as one evenly tiled tab, moving each out of wherever it was shown. The
 *  new tab is pinned and inserted after the active tab. Returns null with fewer than two sessions. */
export function placeInGrid(layout: TabLayout, sessionIds: string[]): PlacedLayout | null {
  const ids = [...new Set(sessionIds)].slice(0, GRID_MAX);
  if (ids.length < 2) return null;
  const detached = detachSessions(layout, new Set(ids));
  const grid = buildGrid(ids);
  const key = ids.find((id) => !detached.paneTrees[id]);
  if (!grid || !key) return null;

  // Detaching may have re-keyed or removed the active tab; an emptied active tab appends the grid at the end.
  const anchor = detached.activeTabId ? detached.openTabs.indexOf(detached.activeTabId) : -1;
  const openTabs = [...detached.openTabs];
  openTabs.splice(anchor >= 0 ? anchor + 1 : openTabs.length, 0, key);
  const next: TabLayout = {
    ...detached,
    openTabs,
    pinnedTabs: [...detached.pinnedTabs, key],
    paneTrees: { ...detached.paneTrees, [key]: grid },
  };
  return focusOn(next, key, firstLeaf(grid).sessionId);
}
