import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("./transport", () => ({ invoke }));

import {
  createProjectFolder,
  deleteProjectFolder,
  renameProjectFolder,
  setProjectFolder,
  setProjectFolderCollapsed,
} from "./tree";

describe("project folder IPC", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
  });

  it("sends each folder command with the backend's argument names", async () => {
    await createProjectFolder("Payments");
    expect(invoke).toHaveBeenLastCalledWith("create_project_folder", { name: "Payments" });
    await renameProjectFolder("f1", "Billing");
    expect(invoke).toHaveBeenLastCalledWith("rename_project_folder", { id: "f1", name: "Billing" });
    await deleteProjectFolder("f1");
    expect(invoke).toHaveBeenLastCalledWith("delete_project_folder", { id: "f1" });
    await setProjectFolderCollapsed("f1", true);
    expect(invoke).toHaveBeenLastCalledWith("set_project_folder_collapsed", { id: "f1", collapsed: true });
  });

  it("sends an explicit null to take a project out of its folder", async () => {
    await setProjectFolder("p1", "f1");
    expect(invoke).toHaveBeenLastCalledWith("set_project_folder", { projectId: "p1", folderId: "f1" });
    await setProjectFolder("p1", null);
    expect(invoke).toHaveBeenLastCalledWith("set_project_folder", { projectId: "p1", folderId: null });
  });
});
