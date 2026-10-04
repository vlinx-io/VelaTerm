//! Folder actions call the backend and then reload the tree, so every view resyncs even when a folder was
//! changed or removed elsewhere in the meantime.

import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({
  listTree: vi.fn(),
  setCollapsed: vi.fn(),
  createProjectFolder: vi.fn(),
  renameProjectFolder: vi.fn(),
  deleteProjectFolder: vi.fn(),
  setProjectFolder: vi.fn(),
  setProjectFolderCollapsed: vi.fn(),
}));
vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ipc);
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { useTermStore } from "./termStore";
import {
  createProjectFolder,
  deleteProjectFolder,
  renameProjectFolder,
  setProjectFolder,
  toggleProjectFolderCollapsed,
} from "./projectFolders";
import type { ProjectFolder, Tree } from "../types";

const folder = (id: string, collapsed = false): ProjectFolder => ({
  id,
  name: id,
  sortOrder: 1,
  collapsed,
  createdAt: 0,
});
const treeWith = (folders: ProjectFolder[]): Tree => ({ projects: [], groups: [], sessions: [], folders });

beforeEach(() => {
  for (const fn of Object.values(ipc)) fn.mockReset();
  ipc.listTree.mockResolvedValue(treeWith([]));
  ipc.setCollapsed.mockResolvedValue(undefined);
  ipc.createProjectFolder.mockResolvedValue(folder("f1"));
  ipc.renameProjectFolder.mockResolvedValue(undefined);
  ipc.deleteProjectFolder.mockResolvedValue(undefined);
  ipc.setProjectFolder.mockResolvedValue(undefined);
  ipc.setProjectFolderCollapsed.mockResolvedValue(undefined);
  useTermStore.setState({ projectFolders: [] });
});

describe("loading folders", () => {
  it("keeps the folders from the tree snapshot", async () => {
    ipc.listTree.mockResolvedValue(treeWith([folder("f1"), folder("f2")]));
    await useTermStore.getState().loadTree();
    expect(useTermStore.getState().projectFolders.map((f) => f.id)).toEqual(["f1", "f2"]);
  });

  it("treats a host that predates folders as having none", async () => {
    ipc.listTree.mockResolvedValue({ projects: [], groups: [], sessions: [] });
    useTermStore.setState({ projectFolders: [folder("old")] });
    await useTermStore.getState().loadTree();
    expect(useTermStore.getState().projectFolders).toEqual([]);
  });
});

describe("folder actions", () => {
  it("creates a folder with a trimmed name and reloads", async () => {
    await createProjectFolder("  Payments  ");
    expect(ipc.createProjectFolder).toHaveBeenCalledWith("Payments");
    expect(ipc.listTree).toHaveBeenCalled();
  });

  it("never sends a blank name", async () => {
    await createProjectFolder("   ");
    await renameProjectFolder("f1", "\t ");
    expect(ipc.createProjectFolder).not.toHaveBeenCalled();
    expect(ipc.renameProjectFolder).not.toHaveBeenCalled();
  });

  it("renames and deletes, reloading after each", async () => {
    await renameProjectFolder("f1", " Billing ");
    expect(ipc.renameProjectFolder).toHaveBeenCalledWith("f1", "Billing");
    await deleteProjectFolder("f1");
    expect(ipc.deleteProjectFolder).toHaveBeenCalledWith("f1");
    expect(ipc.listTree).toHaveBeenCalledTimes(2);
  });

  it("reloads the tree even when the target folder is already gone", async () => {
    ipc.setProjectFolder.mockRejectedValue(new Error("Folder not found: f9"));
    await expect(setProjectFolder("p1", "f9")).rejects.toThrow("Folder not found");
    expect(ipc.listTree).toHaveBeenCalled();
  });

  it("reloads the tree and rethrows when a rename or delete fails", async () => {
    ipc.renameProjectFolder.mockRejectedValue(new Error("Folder not found: f9"));
    ipc.deleteProjectFolder.mockRejectedValue(new Error("Folder not found: f9"));
    ipc.createProjectFolder.mockRejectedValue(new Error("boom"));
    await expect(renameProjectFolder("f9", "X")).rejects.toThrow("Folder not found");
    expect(ipc.listTree).toHaveBeenCalledTimes(1);
    await expect(deleteProjectFolder("f9")).rejects.toThrow("Folder not found");
    expect(ipc.listTree).toHaveBeenCalledTimes(2);
    await expect(createProjectFolder("X")).rejects.toThrow("boom");
    expect(ipc.listTree).toHaveBeenCalledTimes(3);
  });

  it("toggles collapse optimistically and persists it", async () => {
    useTermStore.setState({ projectFolders: [folder("f1")] });
    await toggleProjectFolderCollapsed("f1");
    expect(useTermStore.getState().projectFolders[0].collapsed).toBe(true);
    expect(ipc.setProjectFolderCollapsed).toHaveBeenCalledWith("f1", true);
  });

  it("ignores a collapse toggle for an unknown folder", async () => {
    await toggleProjectFolderCollapsed("missing");
    expect(ipc.setProjectFolderCollapsed).not.toHaveBeenCalled();
  });
});
