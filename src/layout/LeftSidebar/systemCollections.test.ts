import { describe, expect, it } from "vitest";
import type { Group, Project, Session, SessionRuntime } from "../../types";
import { arrangeProjectsInCollections, sectionEntries, sectionNodes } from "./projectCollectionLayout";
import { resolveProjectDrop } from "./projectCollectionDrop";
import { moveToCollectionItem } from "./projectCollectionMenu";
import { t } from "../../i18n";
import {
  liveAgentIds, liveTerminalIds, parseRailSection, pendingCountsBySection, RAIL_SECTIONS, sectionCollection, sessionTypeOf, SYSTEM_CHAT, SYSTEM_PROJECTS,
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

  it("ends the built-in activity bar with Knowledge and parses dynamic plugin sections", () => {
    expect(RAIL_SECTIONS.slice(-2)).toEqual(["archive", "knowledge"]);
    expect(parseRailSection("knowledge")).toBe("knowledge");
    expect(parseRailSection("plugin:colab")).toBe("plugin:colab");
    expect(sectionCollection("knowledge")).toBeNull();
    expect(sectionCollection("plugin:colab")).toBeNull();
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

  it("scopes projects, groups, and sessions to the active sidebar section", () => {
    const col = project("col-1", SYSTEM_PROJECTS, "");
    const nested = project("nested-p", "col-1");
    const projectsList = [projects, tasks, chat, col, nested, loose];
    const groupsList: Group[] = [
      { id: "g-proj", projectId: "nested-p", parentGroupId: null, name: "gp", sortOrder: 0, collapsed: false, createdAt: 0 },
      { id: "g-task", projectId: SYSTEM_TASKS, parentGroupId: null, name: "gt", sortOrder: 0, collapsed: false, createdAt: 0 },
      { id: "g-chat", projectId: SYSTEM_CHAT, parentGroupId: null, name: "gc", sortOrder: 0, collapsed: false, createdAt: 0 },
    ];
    const mkSession = (id: string, projectId: string, kind: Session["kind"] = "claude"): Session =>
      ({ id, projectId, groupId: null, name: id, kind, sortOrder: 0, collapsed: false, createdAt: 0 }) as Session;
    const sessionsList = [
      mkSession("s-proj-nested", "nested-p"),
      mkSession("s-proj-loose", "loose", "terminal"),
      mkSession("s-task", SYSTEM_TASKS),
      mkSession("s-chat", SYSTEM_CHAT),
    ];
    const drafts: Record<string, Session> = {
      "s-draft": mkSession("s-draft", "", "terminal"),
    };
    const runtimes: Record<string, SessionRuntime> = {
      "s-proj-loose": { status: "running", agent: "claude", agentState: "working" },
      "s-draft": { status: "running" },
      "s-task": { status: "running", agent: "codex", agentState: "working" },
    };

    const projScope = sectionNodes("projects", projectsList, groupsList, sessionsList, drafts, runtimes);
    expect(projScope.projects.map((p) => p.id)).toEqual(["col-1", "nested-p", "loose"]);
    expect(projScope.groups.map((g) => g.id)).toEqual(["g-proj"]);
    expect(projScope.sessions.map((s) => s.id)).toEqual(["s-proj-nested", "s-proj-loose"]);

    const taskScope = sectionNodes("tasks", projectsList, groupsList, sessionsList, drafts, runtimes);
    expect(taskScope.projects).toEqual([]);
    expect(taskScope.groups.map((g) => g.id)).toEqual(["g-task"]);
    expect(taskScope.sessions.map((s) => s.id)).toEqual(["s-task"]);

    const chatScope = sectionNodes("chat", projectsList, groupsList, sessionsList, drafts, runtimes);
    expect(chatScope.projects).toEqual([]);
    expect(chatScope.groups.map((g) => g.id)).toEqual(["g-chat"]);
    expect(chatScope.sessions.map((s) => s.id)).toEqual(["s-chat"]);

    const termScope = sectionNodes("terminals", projectsList, groupsList, sessionsList, drafts, runtimes);
    expect(termScope.projects).toEqual([]);
    expect(termScope.groups).toEqual([]);
    expect(termScope.sessions.map((s) => s.id)).toEqual(["s-proj-loose", "s-draft"]);

    // Legacy single-tree layout (no SYSTEM_PROJECTS) keeps everything in scope.
    const legacy = sectionNodes("projects", [nested, loose], groupsList, sessionsList, drafts, runtimes);
    expect(legacy.projects.map((p) => p.id)).toEqual(["nested-p", "loose"]);
    expect(legacy.groups.map((g) => g.id)).toEqual(["g-proj", "g-task", "g-chat"]);
    expect(legacy.sessions.map((s) => s.id)).toEqual(["s-proj-nested", "s-proj-loose", "s-task", "s-chat"]);
  });

  it("counts pending (asking or unread) sessions per activity bar section", () => {
    const mk = (
      id: string,
      projectId: string,
      kind: Session["kind"] = "claude",
      archivedAt: number | null = null,
    ) => ({ id, projectId, kind, name: id, archivedAt }) as Session;
    const sessions = [
      mk("p-ask", "loose"),
      mk("p-unread", SYSTEM_PROJECTS),
      mk("p-viewed", "loose"),
      mk("p-work", "loose"),
      mk("p-term", "loose", "terminal"),
      mk("t-ask", SYSTEM_TASKS),
      mk("t-unread", SYSTEM_TASKS),
      mk("t-archived", SYSTEM_TASKS, "claude", 12345),
      mk("c-unread", SYSTEM_CHAT),
    ];
    const runtimes: Record<string, SessionRuntime> = {
      "p-ask": { status: "running", agent: "claude", agentState: "asking" },
      "p-unread": { status: "running", agent: "claude", agentState: "waiting" },
      "p-viewed": { status: "running", agent: "claude", agentState: "waiting" },
      "p-work": { status: "running", agent: "claude", agentState: "working" },
      "p-term": { status: "running", agent: "codex", agentState: "asking" },
      "t-ask": { status: "running", agent: "claude", agentState: "asking" },
      "t-unread": { status: "idle" },
      "t-archived": { status: "running", agent: "claude", agentState: "asking" },
      "c-unread": { status: "running", agent: "claude", agentState: "background" },
    };
    const notifications = {
      "p-unread": 1,
      "t-unread": 2,
      "c-unread": 3,
      "t-archived": 4,
    };

    expect(pendingCountsBySection(sessions, runtimes, notifications)).toEqual({
      projects: 3, // p-ask, p-unread, p-term
      tasks: 2, // t-ask, t-unread (t-archived excluded)
      chat: 1, // c-unread
    });
  });
});
