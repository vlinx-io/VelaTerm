//! Orders the sidebar's top level: folders by sort order, each followed by its projects unless collapsed, then
//! projects without a folder. Project order inside each bucket is the incoming order (already by sortOrder).

import type { Project, ProjectFolder } from "../../types";

export type ProjectListEntry =
  | { kind: "folder"; folder: ProjectFolder; projectCount: number; expanded: boolean }
  | { kind: "project"; project: Project; indent: 0 | 1 };

/** `hideEmptyFolders` is set while a filter is active; unfiltered, an empty folder stays visible as a drop target.
 *  A project pointing at a folder that is not in `folders` (deleted elsewhere) is listed as loose. */
export function arrangeProjectsInFolders(
  projects: Project[],
  folders: ProjectFolder[],
  isExpanded: (folder: ProjectFolder) => boolean,
  hideEmptyFolders: boolean,
): ProjectListEntry[] {
  const known = new Set(folders.map((f) => f.id));
  const members = new Map<string, Project[]>();
  const loose: Project[] = [];
  for (const project of projects) {
    const folderId = project.folderId;
    if (folderId && known.has(folderId)) {
      const list = members.get(folderId);
      if (list) list.push(project);
      else members.set(folderId, [project]);
    } else {
      loose.push(project);
    }
  }
  const ordered = [...folders].sort((a, b) => a.sortOrder - b.sortOrder || a.createdAt - b.createdAt);
  const out: ProjectListEntry[] = [];
  for (const folder of ordered) {
    const inside = members.get(folder.id) ?? [];
    if (hideEmptyFolders && inside.length === 0) continue;
    const expanded = isExpanded(folder);
    out.push({ kind: "folder", folder, projectCount: inside.length, expanded });
    if (expanded) for (const project of inside) out.push({ kind: "project", project, indent: 1 });
  }
  for (const project of loose) out.push({ kind: "project", project, indent: 0 });
  return out;
}
