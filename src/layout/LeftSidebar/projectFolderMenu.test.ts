import { describe, expect, it, vi } from "vitest";
import type { t as translate } from "../../i18n";
import type { Project, ProjectFolder } from "../../types";
import { moveToFolderItem } from "./projectFolderMenu";

const t = ((key: string) => key) as unknown as typeof translate;
const folder = (id: string, sortOrder: number): ProjectFolder => ({
  id,
  name: id,
  sortOrder,
  collapsed: false,
  createdAt: 0,
});
const project = (folderId: string | null): Project => ({
  id: "p1",
  name: "p1",
  rootPath: "/tmp/p1",
  sortOrder: 0,
  collapsed: false,
  createdAt: 0,
  folderId,
});

describe("moveToFolderItem", () => {
  it("is omitted when there are no folders to move into", () => {
    expect(moveToFolderItem(t, project(null), [], vi.fn())).toBeNull();
    expect(moveToFolderItem(t, undefined, [folder("pay", 1)], vi.fn())).toBeNull();
  });

  it("lists folders in order, then No Folder, and disables the current location", () => {
    const item = moveToFolderItem(t, project("pay"), [folder("infra", 2), folder("pay", 1)], vi.fn());
    const entries = item?.submenu?.map((e) => (e.separator ? "---" : `${e.label}${e.disabled ? " (current)" : ""}`));
    expect(item?.label).toBe("folder.moveTo");
    expect(entries).toEqual(["pay (current)", "infra", "---", "folder.none"]);
  });

  it("treats a folder that no longer exists as No Folder", () => {
    const item = moveToFolderItem(t, project("gone"), [folder("pay", 1)], vi.fn());
    expect(item?.submenu?.find((e) => e.label === "folder.none")?.disabled).toBe(true);
  });

  it("reports the chosen folder, or null for No Folder", () => {
    const onMove = vi.fn();
    const item = moveToFolderItem(t, project("pay"), [folder("pay", 1), folder("infra", 2)], onMove);
    item?.submenu?.find((e) => e.label === "infra")?.onClick?.({} as React.MouseEvent);
    item?.submenu?.find((e) => e.label === "folder.none")?.onClick?.({} as React.MouseEvent);
    expect(onMove.mock.calls).toEqual([["infra"], [null]]);
  });
});
