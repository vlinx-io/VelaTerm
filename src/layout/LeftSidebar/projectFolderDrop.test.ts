import { describe, expect, it } from "vitest";
import type { Project, ProjectFolder } from "../../types";
import { hasProjectDrag, PROJECT_DRAG_MIME, resolveProjectDrop } from "./projectFolderDrop";

const project = (id: string, folderId: string | null = null): Project => ({
  id,
  name: id,
  rootPath: `/tmp/${id}`,
  sortOrder: 0,
  collapsed: false,
  createdAt: 0,
  folderId,
});
const folder = (id: string): ProjectFolder => ({ id, name: id, sortOrder: 0, collapsed: false, createdAt: 0 });

const projects = [project("web", "pay"), project("api", "pay"), project("notes"), project("stale", "gone")];
const folders = [folder("pay"), folder("infra")];
const drag = (id: string) => ({ kind: "project", id });
const onProject = (id: string) => ({ kind: "project" as const, project: projects.find((p) => p.id === id)! });
const onFolder = (folderId: string) => ({ kind: "folder" as const, folderId });

describe("resolveProjectDrop", () => {
  it("moves a project dropped on a folder row into that folder", () => {
    expect(resolveProjectDrop(drag("notes"), onFolder("pay"), projects, folders)).toEqual({
      projectId: "notes",
      folderId: "pay",
    });
  });

  it("moves a project dropped on a project inside a folder into that folder", () => {
    expect(resolveProjectDrop(drag("notes"), onProject("api"), projects, folders)).toEqual({
      projectId: "notes",
      folderId: "pay",
    });
  });

  it("takes a project out of its folder when dropped on a loose project", () => {
    expect(resolveProjectDrop(drag("web"), onProject("notes"), projects, folders)).toEqual({
      projectId: "web",
      folderId: null,
    });
    expect(resolveProjectDrop(drag("web"), onProject("stale"), projects, folders)).toEqual({
      projectId: "web",
      folderId: null,
    });
  });

  it("ignores drops that would change nothing", () => {
    expect(resolveProjectDrop(drag("web"), onFolder("pay"), projects, folders)).toBeNull();
    expect(resolveProjectDrop(drag("web"), onProject("api"), projects, folders)).toBeNull();
    expect(resolveProjectDrop(drag("web"), onProject("web"), projects, folders)).toBeNull();
    expect(resolveProjectDrop(drag("notes"), onProject("notes"), projects, folders)).toBeNull();
    expect(resolveProjectDrop(drag("stale"), onProject("notes"), projects, folders)).toBeNull();
  });

  it("ignores a folder or project that no longer exists", () => {
    expect(resolveProjectDrop(drag("notes"), onFolder("gone"), projects, folders)).toBeNull();
    expect(resolveProjectDrop(drag("missing"), onFolder("pay"), projects, folders)).toBeNull();
  });

  it("leaves session and group payloads to the existing handlers", () => {
    expect(resolveProjectDrop({ kind: "session", id: "s1" }, onProject("api"), projects, folders)).toBeNull();
    expect(resolveProjectDrop({ kind: "group", id: "g1" }, onFolder("pay"), projects, folders)).toBeNull();
  });
});

describe("hasProjectDrag", () => {
  it("recognizes a project drag from its data types alone", () => {
    expect(hasProjectDrag(["text/plain", PROJECT_DRAG_MIME])).toBe(true);
    expect(hasProjectDrag(["text/plain"])).toBe(false);
  });
});
