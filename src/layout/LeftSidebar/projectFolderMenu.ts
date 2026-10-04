//! "Move to Folder" submenu for the project context menu, the keyboard-reachable alternative to dragging.

import type { MenuItem } from "../../components/ContextMenu";
import type { t as translate } from "../../i18n";
import type { Project, ProjectFolder } from "../../types";

/** Null when there is no folder to offer. The project's current location is shown disabled; a folder id that is
 *  no longer in `folders` counts as No Folder, matching how the tree places that project. */
export function moveToFolderItem(
  t: typeof translate,
  project: Project | undefined,
  folders: ProjectFolder[],
  onMove: (folderId: string | null) => void,
): MenuItem | null {
  if (!project || folders.length === 0) return null;
  const current =
    project.folderId && folders.some((f) => f.id === project.folderId) ? project.folderId : null;
  const ordered = [...folders].sort((a, b) => a.sortOrder - b.sortOrder || a.createdAt - b.createdAt);
  return {
    label: t("folder.moveTo"),
    submenu: [
      ...ordered.map((f) => ({
        label: f.name,
        disabled: f.id === current,
        onClick: () => onMove(f.id),
      })),
      { label: "", separator: true },
      { label: t("folder.none"), disabled: current === null, onClick: () => onMove(null) },
    ],
  };
}
