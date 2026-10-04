//! Project folder actions. They live outside termStore.ts to keep that module from growing; each calls the
//! backend and then reloads the tree, like the store's other tree mutations.

import * as tree from "../ipc/tree";
import { useTermStore } from "./termStore";

const reloadTree = () => useTermStore.getState().loadTree();

export async function createProjectFolder(name: string): Promise<void> {
  const trimmed = name.trim();
  if (!trimmed) return;
  try {
    await tree.createProjectFolder(trimmed);
  } finally {
    await reloadTree();
  }
}

export async function renameProjectFolder(id: string, name: string): Promise<void> {
  const trimmed = name.trim();
  if (!trimmed) return;
  try {
    await tree.renameProjectFolder(id, trimmed);
  } finally {
    await reloadTree();
  }
}

export async function deleteProjectFolder(id: string): Promise<void> {
  try {
    await tree.deleteProjectFolder(id);
  } finally {
    await reloadTree();
  }
}

/** Reloads even on failure: the usual cause is a folder deleted from another view, which the reload removes. */
export async function setProjectFolder(projectId: string, folderId: string | null): Promise<void> {
  try {
    await tree.setProjectFolder(projectId, folderId);
  } finally {
    await reloadTree();
  }
}

/** Optimistic, like toggleCollapsed for projects: the chevron flips at once and the backend follows. */
export async function toggleProjectFolderCollapsed(id: string): Promise<void> {
  const current = useTermStore.getState().projectFolders.find((f) => f.id === id);
  if (!current) return;
  const next = !current.collapsed;
  useTermStore.setState((state) => ({
    projectFolders: state.projectFolders.map((f) => (f.id === id ? { ...f, collapsed: next } : f)),
  }));
  await tree.setProjectFolderCollapsed(id, next).catch(() => {});
}
