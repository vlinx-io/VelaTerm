//! Pure layout snapshot for mirror mode: build it from store state, and validate one that arrived from a peer.
//!
//! Kept free of store and transport imports so it stays unit-testable and so `termStore` can import the type
//! without a runtime cycle. What belongs in here is the *arrangement* — which tabs exist, how panes are split,
//! what is active — and nothing that is either client-specific or already shared by other means:
//!
//! - pixel widths of the three columns stay local, because the two windows are rarely the same size;
//! - scroll position, search boxes, and sidebar filters stay local, because syncing them interrupts the peer;
//! - terminal cols/rows stay out entirely: the PTY has exactly one size, arbitrated by the owner model
//!   (communication doc §6.3), and a second authority over it would only fight that one;
//! - tree collapse state is already global — it lives in SQLite — so republishing it here would be redundant.

import type { PaneNode } from "../layout/CenterPane/paneTree";
import type { InspectorTab } from "../theme";
import type { Session } from "../types";
import type { DocTab } from "./docTab";
import type { BrowserTab, SelNode } from "./termStore";

/** Current snapshot schema. A peer running an older or newer version is ignored rather than half-applied. */
export const MIRROR_LAYOUT_VERSION = 1;

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

/** Sidebar state worth sharing: what is selected and inspected, and whether the column is collapsed. */
export interface MirrorLeft {
  selection: SelNode[];
  inspectTarget: SelNode | null;
  collapsed: boolean;
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
    },
    right: { inspectorTab, collapsed: right.collapsed === true },
  };
}
