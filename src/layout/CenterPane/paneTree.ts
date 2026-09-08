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

/** Split targetPaneId, keeping the original leaf as a and placing the new session in b. */
export function splitAt(
  node: PaneNode,
  targetPaneId: string,
  dir: "horizontal" | "vertical",
  newSessionId: string,
): PaneNode {
  if (node.kind === "leaf") {
    if (node.paneId !== targetPaneId) return node;
    return {
      kind: "split",
      paneId: newId(),
      dir,
      sizes: [50, 50],
      a: node,
      b: makeLeaf(newSessionId),
    };
  }
  return {
    ...node,
    a: splitAt(node.a, targetPaneId, dir, newSessionId),
    b: splitAt(node.b, targetPaneId, dir, newSessionId),
  };
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
