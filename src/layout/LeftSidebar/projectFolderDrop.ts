//! Where a dragged project lands. Drag and drop only moves projects between folders; it never reorders them.

import type { Project, ProjectFolder } from "../../types";

/** Set alongside the text/plain payload, because `dragover` can read data types but not data. */
export const PROJECT_DRAG_MIME = "application/x-vlx-project";

export function hasProjectDrag(types: readonly string[]): boolean {
  return types.includes(PROJECT_DRAG_MIME);
}

export type ProjectDropTarget =
  | { kind: "folder"; folderId: string }
  | { kind: "project"; project: Project };

export interface ProjectFolderMove {
  projectId: string;
  folderId: string | null;
}

/** A drop on a folder row joins that folder; a drop on a project row joins that project's folder, or the top
 *  level for a loose project. Returns null for non-project payloads and for drops that change nothing. */
export function resolveProjectDrop(
  payload: { kind: string; id: string },
  target: ProjectDropTarget,
  projects: Project[],
  folders: ProjectFolder[],
): ProjectFolderMove | null {
  if (payload.kind !== "project") return null;
  const dragged = projects.find((p) => p.id === payload.id);
  if (!dragged) return null;
  const known = new Set(folders.map((f) => f.id));
  const folderOf = (p: Project) => (p.folderId && known.has(p.folderId) ? p.folderId : null);
  let folderId: string | null;
  if (target.kind === "folder") {
    if (!known.has(target.folderId)) return null;
    folderId = target.folderId;
  } else {
    if (target.project.id === dragged.id) return null;
    folderId = folderOf(target.project);
  }
  if (folderId === folderOf(dragged)) return null;
  return { projectId: dragged.id, folderId };
}
