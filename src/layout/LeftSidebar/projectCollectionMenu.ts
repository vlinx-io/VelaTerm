//! Keyboard-reachable collection membership commands use the same targets as project drag-and-drop.

import type { MenuItem } from "../../components/ContextMenu";
import type { t as translate } from "../../i18n";
import { isVirtualProject, type Project } from "../../types";
import { resolveProjectDrop } from "./projectCollectionDrop";
import { isSystemCollection, projectDisplayName, SYSTEM_CHAT, SYSTEM_PROJECTS, SYSTEM_TASKS, systemOrder } from "./systemCollections";

export function moveToCollectionItem(t: typeof translate, project: Project | undefined, projects: Project[], onMove: (id: string | null) => void): MenuItem | null {
  if (!project || isSystemCollection(project.id)) return null;
  const collections = projects.filter((p) => isVirtualProject(p) && p.id !== SYSTEM_TASKS && p.id !== SYSTEM_CHAT)
    .sort((a, b) => systemOrder(a.id) - systemOrder(b.id));
  // Once Projects exists it is the top level for projects, so it replaces the Top Level entry.
  const hasProjects = collections.some((p) => p.id === SYSTEM_PROJECTS);
  if (!collections.length && !project.collectionId) return null;
  const current = collections.find(p => p.id === project.collectionId)?.id ?? null;
  return {
    label: t("collection.moveTo"),
    submenu: [
      ...collections.map(p => ({
        label: projectDisplayName(t, p),
        disabled: !resolveProjectDrop({ kind: "project", id: project.id }, p, projects),
        onClick: () => onMove(p.id),
      })),
      ...(hasProjects ? [] : [
        { label: "", separator: true },
        { label: t("collection.none"), disabled: current === null, onClick: () => onMove(null) },
      ]),
    ],
  };
}
