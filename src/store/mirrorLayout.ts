//! Pure layout snapshot for mirror mode: build it from store state, and validate one that arrived from a peer.
//!
//! Kept free of store and transport imports so it stays unit-testable and so `termStore` can import the type
//! without a runtime cycle. What belongs in here is the *state* two mirrored windows must agree on — which tabs
//! exist, how panes are split, what is active, and how the sidebar tree is projected and filtered. What stays
//! out is either client-specific or already shared by other means:
//!
//! - pixel widths of the three columns stay local, because the two windows are rarely the same size;
//! - scroll position stays local, because it follows from a viewport height the peer does not have;
//! - terminal cols/rows stay out entirely: the PTY has exactly one size, arbitrated by the owner model
//!   (communication doc §6.3), and a second authority over it would only fight that one;
//! - the shared tree's collapse state is already global — it lives in SQLite — so republishing it here would be
//!   redundant. The per-projection collapse overrides are not global, so those do travel.
//!
//! Sidebar search text and status/marker filters travel with the rest. Mirror mode means the two windows hold
//! the same state, not that they replay each other's keystrokes: a filter that is on here is on there.

import type { PaneNode } from "../layout/CenterPane/paneTree";
import {
  collectSidebarViewIds,
  firstSidebarViewId,
  makeSidebarTreeTab,
  type SidebarTreeTab,
  type SidebarViewPaneNode,
} from "../layout/LeftSidebar/sidebarTreeLayout";
import type { InspectorTab } from "../theme";
import type { AgentState, Session } from "../types";
import type { DocTab } from "./docTab";
import type { BrowserTab, SelNode, SidebarTreeView } from "./termStore";

/** Current snapshot schema. A peer running an older or newer version is ignored rather than half-applied. */
export const MIRROR_LAYOUT_VERSION = 2;

/** Upper bound on a published per-view ID map, so a corrupt payload cannot grow without limit. */
const VIEW_MAP_LIMIT = 20000;

/** The session states a status filter may name. */
const AGENT_STATES: AgentState[] = ["working", "asking", "waiting"];

/** Center-pane arrangement: the tabs, their pane trees, and what is active. */
export interface MirrorCenter {
  openTabs: string[];
  liveTabs: string[];
  pinnedTabs: string[];
  activeTabId: string | null;
  lastActiveSessionTabId: string | null;
  activeSessionId: string | null;
  focusedPaneId: string | null;
  /** Pane tree per tab, pruned to tabs the snapshot actually carries. */
  paneTrees: Record<string, PaneNode>;
  /** Metadata for `eph-` split sessions, so a peer can render a split it never created itself. */
  ephemeralSessions: Record<string, Session>;
  /** Metadata for `doc-` tabs; the files live on the server, so a remote client opens the same document. */
  docTabs: Record<string, DocTab>;
  /**
   * Metadata for `browser-` tabs. Carried even by clients that cannot render one (the content lives in a
   * native child WebView, desktop only) so following the layout never deletes the peer's browser tabs.
   */
  browserTabs: Record<string, BrowserTab>;
}

/**
 * Sidebar state: what is selected and inspected, whether the column is collapsed, and the whole set of tree
 * projections with the split layout that arranges them.
 *
 * An empty `views` means the peer published nothing usable; a client that receives that keeps its own sidebar
 * rather than emptying it.
 */
export interface MirrorLeft {
  selection: SelNode[];
  inspectTarget: SelNode | null;
  collapsed: boolean;
  /** Every saved projection, with its name, search text, status/marker filters and collapse overrides. */
  views: SidebarTreeView[];
  /** Sidebar tabs, each owning a binary split tree whose leaves reference projection IDs. */
  tabs: SidebarTreeTab[];
  /** The projection that follows the shared tree state; always one of `views`. */
  primaryViewId: string;
  /** The projection receiving pane-local commands; always one of `views`. */
  activeViewId: string;
}

/** Right panel: which of the three tabs is showing, and whether the column is collapsed. */
export interface MirrorRight {
  inspectorTab: InspectorTab;
  collapsed: boolean;
}

/** One published arrangement. */
export interface MirrorLayout {
  v: number;
  center: MirrorCenter;
  left: MirrorLeft;
  right: MirrorRight;
}

/** The store fields a snapshot reads. `TermStore` satisfies this structurally. */
export interface MirrorLayoutSource {
  openTabs: string[];
  liveTabs: string[];
  pinnedTabs: string[];
  activeTabId: string | null;
  lastActiveSessionTabId: string | null;
  activeSessionId: string | null;
  focusedPaneId: string | null;
  paneTrees: Record<string, PaneNode>;
  ephemeralSessions: Record<string, Session>;
  docTabs: Record<string, DocTab>;
  browserTabs: Record<string, BrowserTab>;
  selection: SelNode[];
  inspectTarget: SelNode | null;
  leftCollapsed: boolean;
  rightCollapsed: boolean;
  inspectorTab: InspectorTab;
  sidebarTreeViews: SidebarTreeView[];
  sidebarTreeTabs: SidebarTreeTab[];
  primarySidebarTreeViewId: string;
  activeSidebarTreeViewId: string;
}

/** Keep only the map entries whose key appears in `ids`. */
function pick<T>(map: Record<string, T>, ids: Iterable<string>): Record<string, T> {
  const out: Record<string, T> = {};
  for (const id of ids) {
    const v = map[id];
    if (v !== undefined) out[id] = v;
  }
  return out;
}

/** Every session referenced by the pane trees a layout carries. */
export function layoutSessionIds(layout: MirrorLayout): Set<string> {
  const out = new Set<string>();
  for (const tree of Object.values(layout.center.paneTrees)) sessionIdsOf(tree, out);
  return out;
}

/** Collect the session IDs referenced by a pane tree. */
function sessionIdsOf(node: PaneNode, into: Set<string>): void {
  if (node.kind === "leaf") {
    into.add(node.sessionId);
    return;
  }
  sessionIdsOf(node.a, into);
  sessionIdsOf(node.b, into);
}

/**
 * Build the snapshot this client would publish.
 *
 * Field order is fixed and every map is rebuilt in tab order, so two consecutive builds of an unchanged
 * arrangement serialize identically — that byte equality is what stops the sync loop from feeding itself.
 */
export function buildMirrorLayout(s: MirrorLayoutSource): MirrorLayout {
  const tabIds = [...s.openTabs, ...s.liveTabs.filter((id) => !s.openTabs.includes(id))];
  const carried = tabIds.filter((id) => s.paneTrees[id]);
  const paneTrees = pick(s.paneTrees, carried);
  const sessionIds = new Set<string>();
  for (const id of carried) sessionIdsOf(paneTrees[id], sessionIds);
  return {
    v: MIRROR_LAYOUT_VERSION,
    center: {
      openTabs: [...s.openTabs],
      liveTabs: [...s.liveTabs],
      pinnedTabs: [...s.pinnedTabs],
      activeTabId: s.activeTabId,
      lastActiveSessionTabId: s.lastActiveSessionTabId,
      activeSessionId: s.activeSessionId,
      focusedPaneId: s.focusedPaneId,
      paneTrees,
      ephemeralSessions: pick(s.ephemeralSessions, sessionIds),
      docTabs: pick(s.docTabs, tabIds),
      browserTabs: pick(s.browserTabs, tabIds),
    },
    left: {
      selection: [...s.selection],
      inspectTarget: s.inspectTarget,
      collapsed: s.leftCollapsed,
      views: s.sidebarTreeViews,
      tabs: s.sidebarTreeTabs,
      primaryViewId: s.primarySidebarTreeViewId,
      activeViewId: s.activeSidebarTreeViewId,
    },
    right: {
      inspectorTab: s.inspectorTab,
      collapsed: s.rightCollapsed,
    },
  };
}

const isObj = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

const strArray = (v: unknown): string[] =>
  Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];

const strOrNull = (v: unknown): string | null => (typeof v === "string" ? v : null);

/** Recursively validate a pane tree; a malformed branch invalidates the whole tree rather than half of it. */
function paneNode(v: unknown): PaneNode | null {
  if (!isObj(v)) return null;
  if (v.kind === "leaf") {
    return typeof v.paneId === "string" && typeof v.sessionId === "string"
      ? { kind: "leaf", paneId: v.paneId, sessionId: v.sessionId }
      : null;
  }
  if (v.kind !== "split") return null;
  const a = paneNode(v.a);
  const b = paneNode(v.b);
  const sizes = Array.isArray(v.sizes) ? v.sizes : null;
  if (
    !a ||
    !b ||
    typeof v.paneId !== "string" ||
    (v.dir !== "horizontal" && v.dir !== "vertical") ||
    !sizes ||
    sizes.length !== 2 ||
    sizes.some((n) => typeof n !== "number" || !Number.isFinite(n))
  )
    return null;
  return {
    kind: "split",
    paneId: v.paneId,
    dir: v.dir,
    sizes: [sizes[0] as number, sizes[1] as number],
    a,
    b,
  };
}

/** Keep only well-formed pane trees, dropping tabs whose tree failed validation. */
function paneTreeMap(v: unknown): Record<string, PaneNode> {
  const out: Record<string, PaneNode> = {};
  if (!isObj(v)) return out;
  for (const [k, raw] of Object.entries(v)) {
    const node = paneNode(raw);
    if (node) out[k] = node;
  }
  return out;
}

/**
 * Keep the map entries that are objects carrying a string `id`.
 *
 * The check is structural rather than exhaustive on purpose: the publisher is another client of this same
 * app behind the pairing token, so the risk being managed is version skew and corrupt storage, not a hostile
 * payload. Anything that survives here is still only *layout* — it can misplace a tab, not run a command.
 */
function metaMap<T>(v: unknown): Record<string, T> {
  const out: Record<string, T> = {};
  if (!isObj(v)) return out;
  for (const [k, raw] of Object.entries(v)) {
    if (isObj(raw) && typeof raw.id === "string") out[k] = raw as T;
  }
  return out;
}

/** Validate a selected-node reference. */
function selNode(v: unknown): SelNode | null {
  if (!isObj(v) || typeof v.id !== "string") return null;
  if (v.kind !== "project" && v.kind !== "group" && v.kind !== "session") return null;
  return { id: v.id, kind: v.kind };
}

/** Restore a published ID snapshot, keeping only explicit `true` entries. */
function trueMap(v: unknown): Record<string, true> | null {
  if (!isObj(v)) return null;
  const out: Record<string, true> = {};
  let count = 0;
  for (const [id, value] of Object.entries(v)) {
    if (value !== true || !id) continue;
    out[id.slice(0, 100)] = true;
    if (++count >= VIEW_MAP_LIMIT) break;
  }
  // An empty snapshot is still a snapshot: the filter was on and matched nothing, which the peer shows too.
  return out;
}

/** Restore a published collapse map, dropping anything that is not an explicit boolean. */
function boolMap(v: unknown): Record<string, boolean> | null {
  if (!isObj(v)) return null;
  const out: Record<string, boolean> = {};
  let count = 0;
  for (const [id, value] of Object.entries(v)) {
    if (typeof value !== "boolean" || !id) continue;
    out[id.slice(0, 100)] = value;
    if (++count >= VIEW_MAP_LIMIT) break;
  }
  return count > 0 ? out : null;
}

/** Keep the states a status filter may name, or null when it names none. */
function statusStates(v: unknown): AgentState[] | null {
  if (!Array.isArray(v)) return null;
  const selected = AGENT_STATES.filter((state) => v.includes(state));
  return selected.length > 0 ? selected : null;
}

/**
 * Validate one published projection.
 *
 * A status filter means nothing without the ID snapshot taken when it was switched on, so one that arrives
 * without it starts unfiltered rather than hiding rows by a rule this client cannot reproduce.
 */
function sidebarView(v: unknown): SidebarTreeView | null {
  if (!isObj(v) || typeof v.id !== "string") return null;
  const id = v.id.slice(0, 100);
  if (!id) return null;
  const statusFilterIds = trueMap(v.statusFilterIds);
  const statusFilter = statusFilterIds ? statusStates(v.statusFilter) : null;
  const name =
    typeof v.name === "string"
      ? v.name.replace(/[\u0000-\u001F\u007F-\u009F]/g, "").trim().slice(0, 80)
      : "";
  return {
    id,
    // The pane header needs some label; the ID is at least stable and identical on both sides.
    name: name || id,
    treeFilter: typeof v.treeFilter === "string" ? v.treeFilter.slice(0, 500) : "",
    statusFilter,
    statusFilterIds: statusFilter ? statusFilterIds : null,
    markFilter:
      typeof v.markFilter === "string" && v.markFilter.trim()
        ? v.markFilter.trim().slice(0, 16)
        : null,
    collapsedOverrides: boolMap(v.collapsedOverrides),
  };
}

/** Validate one published sidebar split tree, rejecting duplicate or dangling projection references. */
function sidebarPane(
  v: unknown,
  validViewIds: Set<string>,
  usedViewIds: Set<string>,
  usedPaneIds: Set<string>,
): SidebarViewPaneNode | null {
  if (!isObj(v)) return null;
  const paneId = typeof v.paneId === "string" ? v.paneId.slice(0, 100) : "";
  if (!paneId || usedPaneIds.has(paneId)) return null;
  usedPaneIds.add(paneId);
  if (v.kind === "leaf") {
    const viewId = typeof v.viewId === "string" ? v.viewId : "";
    if (!validViewIds.has(viewId) || usedViewIds.has(viewId)) return null;
    usedViewIds.add(viewId);
    return { kind: "leaf", paneId, viewId };
  }
  if (v.kind !== "split" || (v.dir !== "horizontal" && v.dir !== "vertical")) return null;
  const rawFirst = Array.isArray(v.sizes) ? Number(v.sizes[0]) : 50;
  const first = Number.isFinite(rawFirst) ? Math.max(10, Math.min(90, rawFirst)) : 50;
  const a = sidebarPane(v.a, validViewIds, usedViewIds, usedPaneIds);
  const b = sidebarPane(v.b, validViewIds, usedViewIds, usedPaneIds);
  if (!a || !b) return null;
  return { kind: "split", paneId, dir: v.dir, sizes: [first, 100 - first], a, b };
}

/**
 * Validate the published sidebar: its projections, the tabs arranging them, and which one is primary or active.
 *
 * A projection the peer's tabs never reference still gets a tab of its own here, because losing one would leave
 * the two sidebars showing different panes — the exact divergence mirroring is for. Returning empty views tells
 * the caller the payload was unusable and the local sidebar should stay as it is.
 */
function sidebarState(left: Record<string, unknown>): {
  views: SidebarTreeView[];
  tabs: SidebarTreeTab[];
  primaryViewId: string;
  activeViewId: string;
} {
  const empty = { views: [], tabs: [], primaryViewId: "", activeViewId: "" };
  if (!Array.isArray(left.views)) return empty;
  const views: SidebarTreeView[] = [];
  const seen = new Set<string>();
  for (const candidate of left.views) {
    const view = sidebarView(candidate);
    if (!view || seen.has(view.id)) continue;
    seen.add(view.id);
    views.push(view);
  }
  if (views.length === 0) return empty;
  const validViewIds = new Set(views.map((view) => view.id));
  let usedViewIds = new Set<string>();
  const seenTabIds = new Set<string>();
  const tabs: SidebarTreeTab[] = [];
  for (const candidate of Array.isArray(left.tabs) ? left.tabs : []) {
    if (!isObj(candidate) || typeof candidate.id !== "string") continue;
    const id = candidate.id.slice(0, 100);
    if (!id || seenTabIds.has(id)) continue;
    // Try the tab against a copy: a rejected tab must not consume the projections it referenced.
    const candidateViewIds = new Set(usedViewIds);
    const root = sidebarPane(candidate.root, validViewIds, candidateViewIds, new Set<string>());
    if (!root) continue;
    seenTabIds.add(id);
    usedViewIds = candidateViewIds;
    const memberIds = collectSidebarViewIds(root);
    const activeViewId =
      typeof candidate.activeViewId === "string" && memberIds.includes(candidate.activeViewId)
        ? candidate.activeViewId
        : firstSidebarViewId(root);
    tabs.push({ id, root, activeViewId });
  }
  for (const view of views) {
    if (!usedViewIds.has(view.id)) tabs.push(makeSidebarTreeTab(view.id));
  }
  const primaryViewId =
    typeof left.primaryViewId === "string" && validViewIds.has(left.primaryViewId)
      ? left.primaryViewId
      : views[0].id;
  const activeViewId =
    typeof left.activeViewId === "string" && validViewIds.has(left.activeViewId)
      ? left.activeViewId
      : primaryViewId;
  return { views, tabs, primaryViewId, activeViewId };
}

/**
 * Validate a snapshot received from a peer, returning null when it is unusable.
 *
 * A version mismatch returns null: applying half of a layout the peer meant differently is worse than
 * staying put, and the two clients converge again as soon as the older one is updated.
 */
export function sanitizeMirrorLayout(raw: unknown): MirrorLayout | null {
  if (!isObj(raw) || raw.v !== MIRROR_LAYOUT_VERSION) return null;
  const center = isObj(raw.center) ? raw.center : {};
  const left = isObj(raw.left) ? raw.left : {};
  const right = isObj(raw.right) ? raw.right : {};
  const paneTrees = paneTreeMap(center.paneTrees);
  const openTabs = strArray(center.openTabs);
  const activeTabId = strOrNull(center.activeTabId);
  const inspectorTab =
    right.inspectorTab === "files" || right.inspectorTab === "info" || right.inspectorTab === "git"
      ? right.inspectorTab
      : "files";
  return {
    v: MIRROR_LAYOUT_VERSION,
    center: {
      openTabs,
      liveTabs: strArray(center.liveTabs),
      pinnedTabs: strArray(center.pinnedTabs),
      // A tab that is not open cannot be the active one; a peer mid-transition could publish that pair.
      activeTabId: activeTabId && openTabs.includes(activeTabId) ? activeTabId : null,
      lastActiveSessionTabId: strOrNull(center.lastActiveSessionTabId),
      activeSessionId: strOrNull(center.activeSessionId),
      focusedPaneId: strOrNull(center.focusedPaneId),
      paneTrees,
      ephemeralSessions: metaMap<Session>(center.ephemeralSessions),
      docTabs: metaMap<DocTab>(center.docTabs),
      browserTabs: metaMap<BrowserTab>(center.browserTabs),
    },
    left: {
      selection: Array.isArray(left.selection)
        ? left.selection.map(selNode).filter((n): n is SelNode => n !== null)
        : [],
      inspectTarget: selNode(left.inspectTarget),
      collapsed: left.collapsed === true,
      ...sidebarState(left),
    },
    right: { inspectorTab, collapsed: right.collapsed === true },
  };
}
