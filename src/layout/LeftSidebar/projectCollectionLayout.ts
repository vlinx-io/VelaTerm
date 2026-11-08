//! Flatten collection membership independently of each project's own groups and sessions.

import { collectionsFirst, isVirtualProject, type Project } from "../../types";
import { isSystemCollection, SYSTEM_PROJECTS, systemOrder } from "./systemCollections";

export interface ProjectListEntry {
  project: Project;
  indent: number;
  projectCount: number;
}

/** Missing parents are rendered at the top level. A visited set also keeps a malformed external snapshot usable. */
export function arrangeProjectsInCollections(
  projects: Project[],
  isExpanded: (project: Project) => boolean,
): ProjectListEntry[] {
  const byId = new Map(projects.map(p => [p.id, p]));
  const children = new Map<string, Project[]>();
  const roots: Project[] = [];
  // Only the fixed collections sit at the top level once they exist; anything else found there, such as a
  // project created by an older client, is shown under Projects.
  const fallback = byId.get(SYSTEM_PROJECTS);
  for (const project of projects) {
    let parent = project.collectionId ? byId.get(project.collectionId) : undefined;
    if (!parent && fallback && !isSystemCollection(project.id)) parent = fallback;
    if (isSystemCollection(project.id)) parent = undefined;
    if (parent && isVirtualProject(parent) && parent.id !== project.id) {
      const members = children.get(parent.id) ?? [];
      members.push(project);
      children.set(parent.id, members);
    } else roots.push(project);
  }
  const seen = new Set<string>();
  const out: ProjectListEntry[] = [];
  const visit = (project: Project, indent: number) => {
    if (seen.has(project.id)) return;
    seen.add(project.id);
    const members = children.get(project.id) ?? [];
    out.push({ project, indent, projectCount: members.length });
    if (isExpanded(project)) for (const member of collectionsFirst(members)) visit(member, indent + 1);
  };
  const rootOrder = collectionsFirst(roots).sort((a, b) => systemOrder(a.id) - systemOrder(b.id));
  for (const project of rootOrder) visit(project, 0);
  // Unreachable cycles are shown as loose only; children of intentionally collapsed parents stay hidden.
  for (const project of projects) {
    let current: Project | undefined = project;
    const chain = new Set<string>();
    while (current?.collectionId && !chain.has(current.id)) {
      if (seen.has(current.id)) break;
      chain.add(current.id);
      current = byId.get(current.collectionId);
    }
    if (current && chain.has(current.id)) visit(project, 0);
  }
  return out;
}

/**
 * Entries of one top-level collection, one level shallower. The collection itself stays first with indent -1:
 * it has no row of its own, but its sessions and groups are still listed, at the top level.
 */
export function sectionEntries(entries: ProjectListEntry[], rootId: string): ProjectListEntry[] {
  const start = entries.findIndex(e => e.indent === 0 && e.project.id === rootId);
  if (start < 0) return [];
  let end = start + 1;
  while (end < entries.length && entries[end].indent > 0) end++;
  return entries.slice(start, end).map(e => ({ ...e, indent: e.indent - 1 }));
}
