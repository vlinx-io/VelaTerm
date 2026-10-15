import { describe, expect, it } from "vitest";
import type { Project } from "../../types";
import { hiddenSearchMatches } from "./hiddenSearchMatches";

const project = (id: string, name: string, collectionId: string | null = null): Project => ({
  id,
  name,
  rootPath: `/tmp/${id}`,
  sortOrder: 0,
  collapsed: false,
  createdAt: 0,
  collectionId,
});
const collection = (id: string, name: string): Project => ({
  id,
  name,
  rootPath: "",
  sortOrder: 0,
  collapsed: false,
  createdAt: 0,
});

const ids = (projects: Project[]) => projects.map((p) => p.id);

describe("hiddenSearchMatches", () => {
  const projects = [
    project("p-api", "Payments-API"),
    project("p-web", "payments-web", "f-pay"),
    project("p-notes", "notes"),
    collection("f-pay", "Payments"),
  ];

  it("returns nothing when no status filter is active", () => {
    expect(hiddenSearchMatches(projects, "pay", new Set(), false)).toEqual([]);
  });

  it("returns nothing when the search text is empty", () => {
    expect(hiddenSearchMatches(projects, "  ", new Set(), true)).toEqual([]);
  });

  it("includes matching projects that the filter hid, such as ones with no sessions", () => {
    expect(ids(hiddenSearchMatches(projects, "notes", new Set(), true))).toEqual(["p-notes"]);
  });

  it("includes matching projects whose sessions are in another state", () => {
    expect(ids(hiddenSearchMatches(projects, "api", new Set(["p-notes"]), true))).toEqual(["p-api"]);
  });

  it("includes projects inside a folder but never the folder itself", () => {
    expect(ids(hiddenSearchMatches(projects, "web", new Set(), true))).toEqual(["p-web"]);
    expect(ids(hiddenSearchMatches(projects, "payments", new Set(), true))).not.toContain("f-pay");
  });

  it("excludes projects already visible in the filtered results", () => {
    expect(ids(hiddenSearchMatches(projects, "pay", new Set(["p-api"]), true))).toEqual(["p-web"]);
  });

  it("matches case-insensitively and sorts alphabetically", () => {
    expect(ids(hiddenSearchMatches(projects, "PAYMENTS", new Set(), true))).toEqual(["p-api", "p-web"]);
    expect(ids(hiddenSearchMatches([project("b", "beta"), project("a", "Alpha")], "a", new Set(), true)))
      .toEqual(["a", "b"]);
  });
});
