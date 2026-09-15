//! Recursive binary-tree model for iTerm/tmux-style splits. Each leaf binds one session; internal
//! nodes represent horizontal or vertical splits.

import { genId } from "../../genId";

export interface LeafNode {
  kind: "leaf";
  paneId: string;
  sessionId: string;
}

export interface SplitNode {
  kind: "split";
  paneId: string;
  dir: "horizontal" | "vertical";
  /** Percentage shares for sides a and b, totaling 100 and updated by divider dragging. */
  sizes: [number, number];
  a: PaneNode;
  b: PaneNode;
}

export type PaneNode = LeafNode | SplitNode;

function newId(): string {
  // Use genId's fallback because plain-HTTP remote access lacks crypto.randomUUID in an insecure context.
  return genId();
}

export function makeLeaf(sessionId: string): LeafNode {
  return { kind: "leaf", paneId: newId(), sessionId };
}

/** Collect every leaf session ID in the tree. */
export function collectSessionIds(node: PaneNode): string[] {
  if (node.kind === "leaf") return [node.sessionId];
  return [...collectSessionIds(node.a), ...collectSessionIds(node.b)];
}

/** Return the first (leftmost/topmost) leaf. */
export function firstLeaf(node: PaneNode): LeafNode {
  return node.kind === "leaf" ? node : firstLeaf(node.a);
}

/** Find a leaf by paneId. */
export function findLeaf(node: PaneNode, paneId: string): LeafNode | null {
  if (node.kind === "leaf") return node.paneId === paneId ? node : null;
  return findLeaf(node.a, paneId) ?? findLeaf(node.b, paneId);
}

/** Find a leaf by sessionId. */
export function findBySession(
  node: PaneNode,
  sessionId: string,
): LeafNode | null {
  if (node.kind === "leaf") return node.sessionId === sessionId ? node : null;
  return findBySession(node.a, sessionId) ?? findBySession(node.b, sessionId);
}

/** Retarget every leaf for one session while preserving pane identity and split geometry. */
export function replaceSession(
  node: PaneNode,
  previousSessionId: string,
  nextSessionId: string,
): PaneNode {
  if (node.kind === "leaf") {
    return node.sessionId === previousSessionId
      ? { ...node, sessionId: nextSessionId }
      : node;
  }
  const a = replaceSession(node.a, previousSessionId, nextSessionId);
  const b = replaceSession(node.b, previousSessionId, nextSessionId);
  return a === node.a && b === node.b ? node : { ...node, a, b };
}

/** Split targetPaneId, keeping the original leaf as a and placing the new session in b. With `before`, the new
 *  session takes side a instead (left or top). */
export function splitAt(
  node: PaneNode,
  targetPaneId: string,
  dir: "horizontal" | "vertical",
  newSessionId: string,
  before = false,
): PaneNode {
  if (node.kind === "leaf") {
    if (node.paneId !== targetPaneId) return node;
    const added = makeLeaf(newSessionId);
    return {
      kind: "split",
      paneId: newId(),
      dir,
      sizes: [50, 50],
      a: before ? added : node,
      b: before ? node : added,
    };
  }
  return {
    ...node,
    a: splitAt(node.a, targetPaneId, dir, newSessionId, before),
    b: splitAt(node.b, targetPaneId, dir, newSessionId, before),
  };
}

/** Bind a different session to one pane while preserving its paneId and the surrounding geometry. */
export function setLeafSession(
  node: PaneNode,
  paneId: string,
  sessionId: string,
): PaneNode {
  if (node.kind === "leaf") {
    return node.paneId === paneId ? { ...node, sessionId } : node;
  }
  const a = setLeafSession(node.a, paneId, sessionId);
  const b = setLeafSession(node.b, paneId, sessionId);
  return a === node.a && b === node.b ? node : { ...node, a, b };
}

/** Exchange the sessions shown by two panes of the same tree; geometry and paneIds stay in place. */
export function swapLeafSessions(
  node: PaneNode,
  paneA: string,
  paneB: string,
): PaneNode {
  const a = findLeaf(node, paneA);
  const b = findLeaf(node, paneB);
  if (!a || !b || a === b) return node;
  return setLeafSession(setLeafSession(node, paneA, b.sessionId), paneB, a.sessionId);
}

/** Maximum number of sessions `buildGrid` lays out. */
export const GRID_MAX = 4;

/** Build an evenly divided tile layout in reading order: two side by side, three as one full-height pane beside
 *  a stacked pair, and four as a 2×2 grid. Extra sessions beyond GRID_MAX are ignored. */
export function buildGrid(sessionIds: string[]): PaneNode | null {
  const ids = sessionIds.slice(0, GRID_MAX);
  const [s0, s1, s2, s3] = ids;
  const split = (dir: "horizontal" | "vertical", a: PaneNode, b: PaneNode): PaneNode => ({
    kind: "split",
    paneId: newId(),
    dir,
    sizes: [50, 50],
    a,
    b,
  });
  switch (ids.length) {
    case 0:
      return null;
    case 1:
      return makeLeaf(s0);
    case 2:
      return split("horizontal", makeLeaf(s0), makeLeaf(s1));
    case 3:
      return split("horizontal", makeLeaf(s0), split("vertical", makeLeaf(s1), makeLeaf(s2)));
    default:
      return split(
        "horizontal",
        split("vertical", makeLeaf(s0), makeLeaf(s2)),
        split("vertical", makeLeaf(s1), makeLeaf(s3)),
      );
  }
}

/** Remove a leaf and promote its sibling over the parent split; return null when the tree empties. */
export function removeLeaf(
  node: PaneNode,
  paneId: string,
): PaneNode | null {
  if (node.kind === "leaf") return node.paneId === paneId ? null : node;
  const a = removeLeaf(node.a, paneId);
  const b = removeLeaf(node.b, paneId);
  if (a === null) return b;
  if (b === null) return a;
  return { ...node, a, b };
}

/** Remove the leaf containing a session ID. */
export function removeSession(
  node: PaneNode,
  sessionId: string,
): PaneNode | null {
  if (node.kind === "leaf") return node.sessionId === sessionId ? null : node;
  const a = removeSession(node.a, sessionId);
  const b = removeSession(node.b, sessionId);
  if (a === null) return b;
  if (b === null) return a;
  return { ...node, a, b };
}

export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

const FULL: Rect = { left: 0, top: 0, width: 100, height: 100 };

/** Compute percentage rectangles for every leaf from split sizes. */
export function computeLayout(
  node: PaneNode,
  rect: Rect = FULL,
): Array<{ leaf: LeafNode; rect: Rect }> {
  if (node.kind === "leaf") return [{ leaf: node, rect }];
  const [s0] = node.sizes;
  if (node.dir === "horizontal") {
    const w0 = (rect.width * s0) / 100;
    return [
      ...computeLayout(node.a, { ...rect, width: w0 }),
      ...computeLayout(node.b, {
        ...rect,
        left: rect.left + w0,
        width: rect.width - w0,
      }),
    ];
  }
  const h0 = (rect.height * s0) / 100;
  return [
    ...computeLayout(node.a, { ...rect, height: h0 }),
    ...computeLayout(node.b, {
      ...rect,
      top: rect.top + h0,
      height: rect.height - h0,
    }),
  ];
}

/** Update a split's side percentages while dragging its divider. */
export function setSizes(
  node: PaneNode,
  splitPaneId: string,
  sizes: [number, number],
): PaneNode {
  if (node.kind === "leaf") return node;
  if (node.paneId === splitPaneId) return { ...node, sizes };
  return {
    ...node,
    a: setSizes(node.a, splitPaneId, sizes),
    b: setSizes(node.b, splitPaneId, sizes),
  };
}

/** Draggable divider description using percentages for absolute positioning. */
export interface DividerInfo {
  /** Target split node whose sizes change during dragging. */
  paneId: string;
  dir: "horizontal" | "vertical";
  /** Current side percentages at the start of the drag. */
  sizes: [number, number];
  /** Horizontal: left is boundary x; vertical: left is the block's left edge. */
  leftPct: number;
  /** Horizontal: top is the block's top edge; vertical: top is boundary y. */
  topPct: number;
  /** Divider length perpendicular to dragging: height for horizontal, width for vertical. */
  lengthPct: number;
  /** Full split rectangle used to convert pixel movement into percentages. */
  parentRect: Rect;
}

/** Collect all split dividers for the active tab's draggable overlay. */
export function computeDividers(
  node: PaneNode,
  rect: Rect = FULL,
): DividerInfo[] {
  if (node.kind === "leaf") return [];
  const [s0] = node.sizes;
  if (node.dir === "horizontal") {
    const w0 = (rect.width * s0) / 100;
    const boundaryX = rect.left + w0;
    return [
      {
        paneId: node.paneId,
        dir: "horizontal",
        sizes: node.sizes,
        leftPct: boundaryX,
        topPct: rect.top,
        lengthPct: rect.height,
        parentRect: rect,
      },
      ...computeDividers(node.a, { ...rect, width: w0 }),
      ...computeDividers(node.b, {
        ...rect,
        left: boundaryX,
        width: rect.width - w0,
      }),
    ];
  }
  const h0 = (rect.height * s0) / 100;
  const boundaryY = rect.top + h0;
  return [
    {
      paneId: node.paneId,
      dir: "vertical",
      sizes: node.sizes,
      leftPct: rect.left,
      topPct: boundaryY,
      lengthPct: rect.width,
      parentRect: rect,
    },
    ...computeDividers(node.a, { ...rect, height: h0 }),
    ...computeDividers(node.b, {
      ...rect,
      top: boundaryY,
      height: rect.height - h0,
    }),
  ];
}
