//! Drop geometry for sessions dragged from the sidebar onto the center pane. A drop near a pane's edge splits that
//! pane toward the edge; a drop in its middle shows the session in that pane instead.

import type { Rect } from "./paneTree";

/** Sidebar session drags carry this type next to their text/plain payload. Unlike the payload, a type is readable
 *  during dragover, which is what lets the center pane accept the drag before the drop. */
export const SESSION_DRAG_MIME = "application/x-vlx-session";

/** Present alongside SESSION_DRAG_MIME when several sessions are dragged, so dragover can preview a tiled tab
 *  across the whole stage instead of a single pane. */
export const SESSION_MULTI_DRAG_MIME = "application/x-vlx-session-multi";

/** Share of a pane's width or height, measured from each edge, that counts as an edge drop. */
const EDGE = 0.25;

export type DropZone = "left" | "right" | "top" | "bottom" | "center";

/** Classify a point given as fractions (0..1) of a pane's width and height. The nearest edge wins inside the edge
 *  band; everything else is the center. */
export function dropZone(x: number, y: number): DropZone {
  const edges: Array<[DropZone, number]> = [
    ["left", x],
    ["right", 1 - x],
    ["top", y],
    ["bottom", 1 - y],
  ];
  const [zone, distance] = edges.reduce((best, cur) => (cur[1] < best[1] ? cur : best));
  return distance <= EDGE ? zone : "center";
}

/** The split a drop zone asks for, or null for the center. `before` places the new pane left of or above. */
export function zoneSplit(
  zone: DropZone,
): { dir: "horizontal" | "vertical"; before: boolean } | null {
  switch (zone) {
    case "left":
      return { dir: "horizontal", before: true };
    case "right":
      return { dir: "horizontal", before: false };
    case "top":
      return { dir: "vertical", before: true };
    case "bottom":
      return { dir: "vertical", before: false };
    default:
      return null;
  }
}

/** The area a drop would occupy, in the same percentage space as the pane rectangle. */
export function zonePreview(rect: Rect, zone: DropZone): Rect {
  const halfW = rect.width / 2;
  const halfH = rect.height / 2;
  switch (zone) {
    case "left":
      return { ...rect, width: halfW };
    case "right":
      return { ...rect, left: rect.left + halfW, width: halfW };
    case "top":
      return { ...rect, height: halfH };
    case "bottom":
      return { ...rect, top: rect.top + halfH, height: halfH };
    default:
      return rect;
  }
}
