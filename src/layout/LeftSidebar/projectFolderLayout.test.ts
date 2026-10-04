import { describe, expect, it } from "vitest";
import type { Project, ProjectFolder } from "../../types";
import { arrangeProjectsInFolders, type ProjectListEntry } from "./projectFolderLayout";

const folder = (id: string, sortOrder: number, collapsed = false): ProjectFolder => ({
  id,
  name: id,
  sortOrder,
  collapsed,
  createdAt: 0,
});
const project = (id: string, folderId: string | null = null): Project => ({
  id,
  name: id,
  rootPath: `/tmp/${id}`,
  sortOrder: 0,
  collapsed: false,
  createdAt: 0,
  folderId,
});
const expandUnlessCollapsed = (f: ProjectFolder) => !f.collapsed;
const labels = (entries: ProjectListEntry[]) =>
  entries.map((e) =>
    e.kind === "folder" ? `folder:${e.folder.id}:${e.projectCount}` : `${"  ".repeat(e.indent)}${e.project.id}`,
  );

describe("arrangeProjectsInFolders", () => {
  it("lists folders by sort order, each followed by its projects, then loose projects", () => {
    const entries = arrangeProjectsInFolders(
      [project("web", "pay"), project("notes"), project("api", "pay"), project("ops", "infra")],
      [folder("infra", 2), folder("pay", 1)],
      expandUnlessCollapsed,
      false,
    );
    expect(labels(entries)).toEqual(["folder:pay:2", "  web", "  api", "folder:infra:1", "  ops", "notes"]);
  });

  it("keeps a collapsed folder's count but hides its projects", () => {
    const entries = arrangeProjectsInFolders(
      [project("web", "pay"), project("api", "pay"), project("notes")],
      [folder("pay", 1, true)],
      expandUnlessCollapsed,
      false,
    );
    expect(labels(entries)).toEqual(["folder:pay:2", "notes"]);
  });

  it("shows an empty folder so projects can be dropped into it", () => {
    const entries = arrangeProjectsInFolders([project("notes")], [folder("pay", 1)], expandUnlessCollapsed, false);
    expect(labels(entries)).toEqual(["folder:pay:0", "notes"]);
  });

  it("hides folders with no visible project while a filter is active", () => {
    const entries = arrangeProjectsInFolders(
      [project("api", "pay")],
      [folder("pay", 1, true), folder("infra", 2)],
      () => true,
      true,
    );
    expect(labels(entries)).toEqual(["folder:pay:1", "  api"]);
  });

  it("treats a project whose folder no longer exists as loose", () => {
    const entries = arrangeProjectsInFolders([project("web", "gone")], [], expandUnlessCollapsed, false);
    expect(labels(entries)).toEqual(["web"]);
  });
});
