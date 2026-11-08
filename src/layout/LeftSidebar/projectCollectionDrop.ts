//! Project drags change collection membership, leaving session ownership and directories intact.

import { isVirtualProject, type Project } from "../../types";
import { isSystemCollection, SYSTEM_CHAT, SYSTEM_TASKS } from "./systemCollections";

export const PROJECT_DRAG_MIME = "application/x-vlx-project";
export const hasProjectDrag = (types: readonly string[]) => types.includes(PROJECT_DRAG_MIME);

/** A collection receives the project; another project receives it beside itself. Reject self-drops and cycles. */
export function resolveProjectDrop(payload: { kind: string; id: string }, target: Project, projects: Project[]) {
  if (payload.kind !== "project" || payload.id === target.id) return null;
  // Fixed collections stay where they are, and Tasks and Chat hold sessions only.
  if (isSystemCollection(payload.id) || target.id === SYSTEM_TASKS || target.id === SYSTEM_CHAT) return null;
  const dragged = projects.find(p => p.id === payload.id);
  if (!dragged || !projects.some(p => p.id === target.id)) return null;
  const parentOf = (p: Project) => projects.find(c => c.id === p.collectionId && isVirtualProject(c))?.id ?? null;
  const collectionId = isVirtualProject(target) ? target.id : parentOf(target);
  if (parentOf(dragged) === collectionId) return null;
  let ancestor = collectionId;
  const seen = new Set<string>();
  while (ancestor) {
    if (ancestor === dragged.id || seen.has(ancestor)) return null;
    seen.add(ancestor);
    const parent = projects.find(p => p.id === ancestor);
    ancestor = parent ? parentOf(parent) : null;
  }
  return { projectId: dragged.id, collectionId };
}
