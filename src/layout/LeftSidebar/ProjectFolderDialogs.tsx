//! Name prompt for creating or renaming a project folder, and the delete confirmation. LeftSidebar owns the
//! open dialog because the footer button and the folder context menu have no node to hang a dialog on.

import { FormModal } from "../../components/FormModal";
import { useT } from "../../i18n";
import {
  createProjectFolder,
  deleteProjectFolder,
  renameProjectFolder,
} from "../../store/projectFolders";
import type { ProjectFolder } from "../../types";
import { ConfirmDelete } from "../sessionMenuDialogs";

export type FolderDialog =
  | { type: "create" }
  | { type: "rename"; folder: ProjectFolder }
  | { type: "delete"; folder: ProjectFolder };

export function ProjectFolderDialogs({
  dialog,
  onClose,
}: {
  dialog: FolderDialog | null;
  onClose: () => void;
}) {
  const t = useT();
  if (!dialog) return null;
  if (dialog.type === "delete") {
    const { id, name } = dialog.folder;
    return (
      <ConfirmDelete
        name={name}
        title={t("folder.deleteTitle")}
        body={t("folder.deleteBody", name)}
        onCancel={onClose}
        onConfirm={() => {
          onClose();
          void deleteProjectFolder(id).catch(() => {});
        }}
      />
    );
  }
  const renaming = dialog.type === "rename" ? dialog.folder : null;
  return (
    <FormModal
      title={renaming ? t("folder.renameTitle") : t("folder.createTitle")}
      fields={[
        {
          key: "name",
          label: t("folder.name"),
          placeholder: t("folder.namePlaceholder"),
          required: true,
          autoFocus: true,
        },
      ]}
      initial={renaming ? { name: renaming.name } : undefined}
      submitLabel={renaming ? t("common.rename") : t("folder.create")}
      onCancel={onClose}
      onSubmit={(values) => {
        onClose();
        const name = values.name.trim();
        if (!name) return;
        if (renaming) void renameProjectFolder(renaming.id, name).catch(() => {});
        else void createProjectFolder(name).catch(() => {});
      }}
    />
  );
}
