import { isVirtualProject, type Project } from "../../types";

/** Projects whose name matches the search text but that a status filter removed from the tree, so a search
 *  can still reach them. Collections are skipped because only directory projects can host new sessions. */
export function hiddenSearchMatches(
  projects: readonly Project[],
  searchText: string,
  visibleProjectIds: ReadonlySet<string> | null,
  statusFiltering: boolean,
): Project[] {
  const needle = searchText.trim().toLowerCase();
  if (!statusFiltering || !needle) return [];
  return projects
    .filter(
      (p) =>
        !isVirtualProject(p) &&
        !visibleProjectIds?.has(p.id) &&
        p.name.toLowerCase().includes(needle),
    )
    .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
}
