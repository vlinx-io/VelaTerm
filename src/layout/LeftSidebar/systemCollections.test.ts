import { describe, expect, it } from "vitest";
import type { Project, Session, SessionRuntime } from "../../types";
import { arrangeProjectsInCollections, sectionEntries } from "./projectCollectionLayout";
import { resolveProjectDrop } from "./projectCollectionDrop";
import { moveToCollectionItem } from "./projectCollectionMenu";
import { t } from "../../i18n";
import {
  liveAgentIds, liveTerminalIds, parseRailSection, RAIL_SECTIONS, sectionCollection, sessionTypeOf, SYSTEM_CHAT, SYSTEM_PROJECTS,
  SYSTEM_TASKS,
} from "./systemCollections";

const project = (id: string, collectionId: string | null = null, rootPath = `/tmp/${id}`): Project => ({
  id, name: id, collectionId, rootPath, collapsed: false, sortOrder: 0, createdAt: 0,
});

describe("fixed collections", () => {
  const projects = project(SYSTEM_PROJECTS, null, "");
  const tasks = project(SYSTEM_TASKS, null, "");
  const chat = project(SYSTEM_CHAT, null, "");
  const loose = project("loose");
  const own = project("own", SYSTEM_PROJECTS, "");
  const all = [chat, loose, tasks, own, projects];

  it("keeps only the fixed collections at the top, in their fixed order", () => {
    expect(arrangeProjectsInCollections(all, () => true).map((e) => [e.project.id, e.indent]))
      .toEqual([[SYSTEM_PROJECTS, 0], ["own", 1], ["loose", 1], [SYSTEM_TASKS, 0], [SYSTEM_CHAT, 0]]);
  });

  it("lists one section's contents a level up, keeping the section itself first without a row", () => {
    const entries = arrangeProjectsInCollections(all, () => true);
    expect(sectionEntries(entries, SYSTEM_PROJECTS).map((e) => [e.project.id, e.indent]))
      .toEqual([[SYSTEM_PROJECTS, -1], ["own", 0], ["loose", 0]]);
    expect(sectionEntries(entries, SYSTEM_TASKS).map((e) => [e.project.id, e.indent])).toEqual([[SYSTEM_TASKS, -1]]);
    expect(sectionEntries(entries, "missing")).toEqual([]);
  });

  it("puts Agents inside Terminals instead of giving it an activity bar button", () => {
    expect(RAIL_SECTIONS).not.toContain("agents");
    expect(RAIL_SECTIONS.map(sectionCollection)).toEqual(
      RAIL_SECTIONS.map((s) => (s === "terminals" || s === "archive" || s === "knowledge" ? null : s === "projects" ? SYSTEM_PROJECTS : s === "tasks" ? SYSTEM_TASKS : SYSTEM_CHAT)),
    );
  });

  it("ends the activity bar with Knowledge, which has no collection behind it", () => {
    expect(RAIL_SECTIONS[RAIL_SECTIONS.length - 1]).toBe("knowledge");
    expect(parseRailSection("knowledge")).toBe("knowledge");
    expect(sectionCollection("knowledge")).toBeNull();
  });

  it("never moves a fixed collection or puts a project into Tasks or Chat", () => {
    expect(resolveProjectDrop({ kind: "project", id: SYSTEM_TASKS }, own, all)).toBeNull();
    expect(resolveProjectDrop({ kind: "project", id: "loose" }, tasks, all)).toBeNull();
    expect(resolveProjectDrop({ kind: "project", id: "loose" }, chat, all)).toBeNull();
    expect(resolveProjectDrop({ kind: "project", id: "loose" }, own, all)).toEqual({ projectId: "loose", collectionId: "own" });
    expect(moveToCollectionItem(t, projects, all, () => {})).toBeNull();
    const labels = moveToCollectionItem(t, loose, all, () => {})?.submenu?.map((item) => item.label);
    expect(labels).toEqual([t("systemCollection.projects"), "own"]);
  });

  it("derives the session type from the location", () => {
    expect(sessionTypeOf(SYSTEM_TASKS)).toBe("task");
    expect(sessionTypeOf(SYSTEM_CHAT)).toBe("chat");
    expect(sessionTypeOf("loose")).toBe("project");
  });

  it("lists running terminals and the agents started in them", () => {
    const session = (id: string, kind: Session["kind"] = "terminal") =>
      ({ id, kind, projectId: "loose", name: id }) as Session;
    const runtimes: Record<string, SessionRuntime> = {
      a: { status: "running" }, b: { status: "running", agent: "claude" }, c: { status: "exited" },
      d: { status: "running" }, e: { status: "running", agent: "codex" },
    };
    const terminals = liveTerminalIds([session("a"), session("b"), session("c"), session("d", "claude")],
      { e: session("e") }, runtimes);
    expect(terminals).toEqual(["a", "b", "e"]);
    expect(liveAgentIds(terminals, runtimes)).toEqual(["b", "e"]);
  });
});
