//! Recursive pane layout for saved sidebar tree projections. Each top-level tab is the pane group of one
//! activity-bar section and owns an independent binary split tree whose leaves reference sidebar view IDs. The
//! sidebar shows one group at a time; switching sections swaps the whole group and keeps the others intact.

import { genId } from "../../genId";
import { parseRailSection, type RailSection } from "./systemCollections";

export type SidebarSplitDirection = "horizontal" | "vertical";

export interface SidebarViewLeaf {
  kind: "leaf";
  paneId: string;
  viewId: string;
}

export interface SidebarViewSplit {
  kind: "split";
  paneId: string;
  dir: SidebarSplitDirection;
  sizes: [number, number];
  a: SidebarViewPaneNode;
  b: SidebarViewPaneNode;
}

export type SidebarViewPaneNode = SidebarViewLeaf | SidebarViewSplit;

export interface SidebarTreeTab {
  id: string;
  root: SidebarViewPaneNode;
  activeViewId: string;
  /** Activity-bar section shown by every pane of this group. Absent means Projects. */
  section?: RailSection;
}

/** The section a pane group shows; groups saved before sections existed show Projects. */
export const sidebarTabSection = (tab: { section?: RailSection } | undefined): RailSection =>
  tab?.section ?? "projects";

/** The group holding a view, if any. */
export const sidebarTabOfView = <T extends SidebarTreeTab>(tabs: T[], viewId: string): T | undefined =>
  tabs.find((tab) => collectSidebarViewIds(tab.root).includes(viewId));

/**
 * Give every pane group a section. The group holding the primary view is always Projects, because project reveal
 * and the global filter aliases target that view; it is listed first so the activity bar finds it before any other
 * Projects group. Another group keeps its own section, or else the section its first pane showed when sections
 * were stored per pane. Nothing is dropped: the activity bar opens the first group of a section, and any further
 * group of the same section keeps its panes and conditions.
 */
export function assignSidebarTabSections(
  tabs: SidebarTreeTab[],
  primaryViewId: string,
  legacyViewSection: (viewId: string) => unknown = () => undefined,
): SidebarTreeTab[] {
  const primaryTab = sidebarTabOfView(tabs, primaryViewId);
  const ordered = primaryTab ? [primaryTab, ...tabs.filter((tab) => tab !== primaryTab)] : tabs;
  return ordered.map((tab) => ({
    ...tab,
    section: tab === primaryTab
      ? "projects"
      : parseRailSection(tab.section) ??
        parseRailSection(legacyViewSection(firstSidebarViewId(tab.root))) ??
        "projects",
  }));
}

export interface SidebarViewRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

const FULL_RECT: SidebarViewRect = { left: 0, top: 0, width: 100, height: 100 };

export function makeSidebarViewLeaf(viewId: string): SidebarViewLeaf {
  return { kind: "leaf", paneId: genId(), viewId };
}

export function makeSidebarTreeTab(viewId: string, section?: RailSection): SidebarTreeTab {
  return {
    id: `tree-tab-${genId()}`,
    root: makeSidebarViewLeaf(viewId),
    activeViewId: viewId,
    ...(section ? { section } : {}),
  };
}

export function collectSidebarViewIds(node: SidebarViewPaneNode): string[] {
  if (node.kind === "leaf") return [node.viewId];
  return [...collectSidebarViewIds(node.a), ...collectSidebarViewIds(node.b)];
}

export function firstSidebarViewId(node: SidebarViewPaneNode): string {
  return node.kind === "leaf" ? node.viewId : firstSidebarViewId(node.a);
}

/** Split one view leaf, retaining it on the first side and placing the copied view on the second side. */
export function splitSidebarView(
  node: SidebarViewPaneNode,
  targetViewId: string,
  dir: SidebarSplitDirection,
  newViewId: string,
): SidebarViewPaneNode {
  if (node.kind === "leaf") {
    if (node.viewId !== targetViewId) return node;
    return {
      kind: "split",
      paneId: genId(),
      dir,
      sizes: [50, 50],
      a: node,
      b: makeSidebarViewLeaf(newViewId),
    };
  }
  return {
    ...node,
    a: splitSidebarView(node.a, targetViewId, dir, newViewId),
    b: splitSidebarView(node.b, targetViewId, dir, newViewId),
  };
}

/** Remove a view leaf and promote its sibling. A null result means the tab itself is now empty. */
export function removeSidebarView(
  node: SidebarViewPaneNode,
  viewId: string,
): SidebarViewPaneNode | null {
  if (node.kind === "leaf") return node.viewId === viewId ? null : node;
  const a = removeSidebarView(node.a, viewId);
  const b = removeSidebarView(node.b, viewId);
  if (a === null) return b;
  if (b === null) return a;
  return { ...node, a, b };
}

export function setSidebarSplitSizes(
  node: SidebarViewPaneNode,
  splitPaneId: string,
  sizes: [number, number],
): SidebarViewPaneNode {
  if (node.kind === "leaf") return node;
  if (node.paneId === splitPaneId) return { ...node, sizes };
  return {
    ...node,
    a: setSidebarSplitSizes(node.a, splitPaneId, sizes),
    b: setSidebarSplitSizes(node.b, splitPaneId, sizes),
  };
}

export function computeSidebarViewLayout(
  node: SidebarViewPaneNode,
  rect: SidebarViewRect = FULL_RECT,
): Array<{ leaf: SidebarViewLeaf; rect: SidebarViewRect }> {
  if (node.kind === "leaf") return [{ leaf: node, rect }];
  const [firstSize] = node.sizes;
  if (node.dir === "horizontal") {
    const firstWidth = (rect.width * firstSize) / 100;
    return [
      ...computeSidebarViewLayout(node.a, { ...rect, width: firstWidth }),
      ...computeSidebarViewLayout(node.b, {
        ...rect,
        left: rect.left + firstWidth,
        width: rect.width - firstWidth,
      }),
    ];
  }
  const firstHeight = (rect.height * firstSize) / 100;
  return [
    ...computeSidebarViewLayout(node.a, { ...rect, height: firstHeight }),
    ...computeSidebarViewLayout(node.b, {
      ...rect,
      top: rect.top + firstHeight,
      height: rect.height - firstHeight,
    }),
  ];
}

export interface SidebarDividerInfo {
  paneId: string;
  dir: SidebarSplitDirection;
  sizes: [number, number];
  leftPct: number;
  topPct: number;
  lengthPct: number;
  parentRect: SidebarViewRect;
}

export function computeSidebarDividers(
  node: SidebarViewPaneNode,
  rect: SidebarViewRect = FULL_RECT,
): SidebarDividerInfo[] {
  if (node.kind === "leaf") return [];
  const [firstSize] = node.sizes;
  if (node.dir === "horizontal") {
    const firstWidth = (rect.width * firstSize) / 100;
    const boundary = rect.left + firstWidth;
    return [
      {
        paneId: node.paneId,
        dir: node.dir,
        sizes: node.sizes,
        leftPct: boundary,
        topPct: rect.top,
        lengthPct: rect.height,
        parentRect: rect,
      },
      ...computeSidebarDividers(node.a, { ...rect, width: firstWidth }),
      ...computeSidebarDividers(node.b, {
        ...rect,
        left: boundary,
        width: rect.width - firstWidth,
      }),
    ];
  }
  const firstHeight = (rect.height * firstSize) / 100;
  const boundary = rect.top + firstHeight;
  return [
    {
      paneId: node.paneId,
      dir: node.dir,
      sizes: node.sizes,
      leftPct: rect.left,
      topPct: boundary,
      lengthPct: rect.width,
      parentRect: rect,
    },
    ...computeSidebarDividers(node.a, { ...rect, height: firstHeight }),
    ...computeSidebarDividers(node.b, {
      ...rect,
      top: boundary,
      height: rect.height - firstHeight,
    }),
  ];
}

/** Convert the old global layout mode without losing the user's visible grouping. */
export function migrateLegacySidebarTabs(
  viewIds: string[],
  layout: "tabs" | "stack",
): SidebarTreeTab[] {
  if (layout === "tabs") return viewIds.map((viewId) => makeSidebarTreeTab(viewId));
  const [firstId, ...rest] = viewIds;
  if (!firstId) return [];
  let root: SidebarViewPaneNode = makeSidebarViewLeaf(firstId);
  for (const [index, viewId] of rest.entries()) {
    const totalLeaves = index + 2;
    const previousShare = ((totalLeaves - 1) / totalLeaves) * 100;
    root = {
      kind: "split",
      paneId: genId(),
      dir: "vertical",
      sizes: [previousShare, 100 - previousShare],
      a: root,
      b: makeSidebarViewLeaf(viewId),
    };
  }
  return [{ id: `tree-tab-${genId()}`, root, activeViewId: firstId }];
}
