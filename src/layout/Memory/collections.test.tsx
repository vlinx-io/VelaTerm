// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { setLang } from "../../i18n";
import type { Session } from "../../types";
import { collectionTreeRows, MemoryCollections, MemoryCollectionsDirectory } from "./MemoryCollections";
import { COLLECTIONS_COLLAPSE } from "./navigation";

const api = vi.hoisted(() => ({ collections: vi.fn(), list: vi.fn(), search: vi.fn(),page:vi.fn(),cancel:vi.fn() }));
vi.mock("../../ipc/memory", () => ({ memoryCollections: api.collections, memoryList: api.list, memoryGet: vi.fn() }));
vi.mock("../../ipc/commands", () => ({ startContentSearch: api.search,contentSearchPage:api.page,cancelContentSearch:api.cancel,readAgentTranscript: vi.fn().mockResolvedValue([]) }));
vi.mock("../../exportSession", () => ({ canExportContext: () => false, exportSessionToFile: vi.fn() }));
vi.mock("../sessionViewers/SessionContentViewer", () => ({
  SessionContentViewer: ({ session }: { session: { id: string } }) => <div>content:{session.id}</div>,
}));
const store = vi.hoisted(() => ({
  loadArchived: vi.fn(), restoreSession: vi.fn(), deleteNode: vi.fn(),
  projects: [], groups: [], sessions: [], archivedSessions: [] as unknown[],
}));
vi.mock("../../store/termStore", () => ({
  useTermStore: Object.assign(
    (select: (s: unknown) => unknown) => select(store),
    { getState: () => store },
  ),
}));

const archived: Session = { id: "s1", projectId: "p", name: "Archived one", kind: "claude", collapsed: false, sortOrder: 1, createdAt: 1, archivedAt: 100, agentSessionId: "agent-1" };
const child: Session = { ...archived, id: "s2", name: "Child task", parentSessionId: "s1", agentSessionId: "agent-2" };
const collections = {
  projects: [{ id: "p", name: "Alpha", kind: "project", count: 1, sessions: [
    { id: "s1", name: "Archived one", kind: "claude", count: 1, archivedAt: 100, parentId: null, groupIds: ["g"], groupPath: ["Work"] },
    { id: "s2", name: "Child task", kind: "claude", count: 0, archivedAt: 100, parentId: "s1", groupIds: [], groupPath: [] },
  ] }],
  sessions: [archived, child],
};

beforeEach(() => {
  setLang("en");
  window.history.replaceState(null, "", "/?memory=collections&memoryCollectionView=time");
  api.collections.mockReset().mockResolvedValue(collections);
  api.list.mockReset().mockResolvedValue({ projects: [], selectedSessionId: null, entries: [], total: 0, pageSize: 40, tags: [] });
  api.search.mockReset().mockResolvedValue({jobId:"search-one",status:"pending",scannedSessions:0,scannedBytes:0});
  api.page.mockReset().mockResolvedValue({jobId:"search-one",status:"complete",scannedSessions:1,scannedBytes:1,hits:[],totalSessions:0,totalMatches:0});
  api.cancel.mockReset().mockResolvedValue(undefined);
  store.loadArchived.mockReset();
  store.restoreSession.mockReset().mockResolvedValue(undefined);
  store.deleteNode.mockReset().mockResolvedValue(undefined);
});
afterEach(cleanup);

it("lists every archived session in a table that links to its page", async () => {
  render(<MemoryCollections sessionId="" />);
  const link = await screen.findByRole("link", { name: /Archived one/ });
  expect(link.closest(".kbx-list")!.textContent).toContain("Child task");
  expect(link.textContent).toContain("Alpha");
  const params = new URLSearchParams(link.getAttribute("href")!.split("?")[1]);
  expect(params.get("memory")).toBe("collection/s1");
  expect(params.get("memoryCollectionProject")).toBe("p");
  expect(store.loadArchived).toHaveBeenCalled();
});

it("filters the archive table through the URL", async () => {
  api.collections.mockResolvedValue({ ...collections, projects: [...collections.projects,
    { id: "q", name: "Beta", kind: "project", count: 0, sessions: [{ id: "s3", name: "Beta task", kind: "claude", count: 0, archivedAt: 200, parentId: null, groupIds: [], groupPath: [] }] }] });
  window.history.replaceState(null, "", "/?memory=collections&memoryCollectionProject=q");
  render(<MemoryCollections sessionId="" />);
  await screen.findByRole("link", { name: /Beta task/ });
  expect(screen.queryByRole("link", { name: /Archived one/ })).toBeNull();
  expect(screen.getByRole("heading", { name: "Beta" })).toBeTruthy();
});

it("collapses the archive tree when the sidebar asks for it", async () => {
  window.history.replaceState(null, "", "/?memory=collection/s2&memoryCollectionProject=p");
  render(<MemoryCollectionsDirectory />);
  await screen.findByRole("link", { name: /Child task/ });
  act(() => { window.dispatchEvent(new Event(COLLECTIONS_COLLAPSE)); });
  expect(screen.queryByRole("link", { name: /Child task/ })).toBeNull();
});

it("opens a conversation with the entries generated from it", async () => {
  window.history.replaceState(null, "", "/?memory=collection/s1&memoryCollectionProject=p");
  api.list.mockResolvedValue({
    projects: [], selectedSessionId: "s1", total: 1, pageSize: 40, tags: [],
    entries: [{ id: "e1", title: "Stored topic", summary: "Reusable", tags: [], updatedAt: 0, sourceCount: 1 }],
  });
  render(<MemoryCollections sessionId="s1" />);
  expect(await screen.findByRole("link", { name: /Stored topic/ })).toBeTruthy();
  expect(api.list).toHaveBeenCalledWith(expect.objectContaining({ sessionId: "s1" }));
  fireEvent.click(screen.getByRole("tab", { name: "Conversation" }));
  await waitFor(() => expect(new URLSearchParams(location.search).get("memoryCollectionTab")).toBe("conversation"));
  expect(await screen.findByText("content:s1")).toBeTruthy();
});

it("searches archived content from the query parameter and keeps it in the URL", async () => {
  window.history.replaceState(null, "", "/?memory=collections&memoryCollectionQuery=needle");
  api.page.mockResolvedValue({jobId:"search-one",status:"complete",scannedSessions:1,scannedBytes:1,totalSessions:1,totalMatches:1,
    hits:[{ sessionId: "s1", name: "Archived one", kind: "claude", archived: true, source: "transcript", matchCount: 1, matches: [{ messageIndex: 0, ordinal: 1, snippet: "the needle here", matched: ["needle"] }] }]});
  render(<MemoryCollections sessionId="" />);
  expect(screen.getByRole("textbox", { name: "Search archived content…" })).toHaveProperty("value", "needle");
  expect(await screen.findByText("Archived one")).toBeTruthy();
  await waitFor(() => expect(api.search).toHaveBeenCalledWith("needle", "archived"));
  fireEvent.change(screen.getByRole("textbox", { name: "Search archived content…" }), { target: { value: "tea" } });
  await waitFor(() => expect(new URLSearchParams(location.search).get("memoryCollectionQuery")).toBe("tea"));
});

it("restores and permanently deletes an archived session from its row menu", async () => {
  render(<MemoryCollections sessionId="" />);
  const child = await screen.findByRole("link", { name: /Child task/ });
  // Only the archive root restores; the nested child goes back with it.
  fireEvent.contextMenu(child);
  expect(screen.queryByText("Restore to normal session")).toBeNull();
  fireEvent.click(screen.getByText("Delete permanently (with recording)"));
  await waitFor(() => expect(store.deleteNode).toHaveBeenCalledWith("session", "s2"));
});

it("restores an archive root from its row menu", async () => {
  render(<MemoryCollections sessionId="" />);
  fireEvent.contextMenu(await screen.findByRole("link", { name: /Archived one/ }));
  fireEvent.click(screen.getByText("Restore to normal session"));
  await waitFor(() => expect(store.restoreSession).toHaveBeenCalledWith("s1"));
});

it("shows the collections hierarchy in the knowledge tree", async () => {
  render(<MemoryCollectionsDirectory />);
  const project = await screen.findByRole("link", { name: /Alpha/ });
  expect(new URLSearchParams(project.getAttribute("href")!.split("?")[1]).get("memoryCollectionProject")).toBe("p");
  fireEvent.click(screen.getByRole("button", { name: "Alpha" }));
  expect(screen.queryByRole("link", { name: /Archived one/ })).toBeNull();
  fireEvent.click(screen.getAllByRole("button", { name: "Work" })[0]);
  const session = await screen.findByRole("link", { name: /Archived one/ });
  expect(new URLSearchParams(session.getAttribute("href")!.split("?")[1]).get("memory")).toBe("collection/s1");
  expect(session.getAttribute("title")).toBe("Alpha › Work › Archived one");
  fireEvent.click(screen.getByRole("button", { name: "Archived one" }));
  const nested = await screen.findByRole("link", { name: /Child task/ });
  expect(nested.getAttribute("title")).toBe("Alpha › Work › Archived one › Child task");
});

it("expands the knowledge tree down to a selected nested session", async () => {
  window.history.replaceState(null, "", "/?memory=collection/s2&memoryCollectionProject=p");
  render(<MemoryCollectionsDirectory />);
  const nested = await screen.findByRole("link", { name: /Child task/ });
  expect(nested.getAttribute("aria-current")).toBe("page");
});

it("shows a nested session with its parent in the breadcrumb", async () => {
  window.history.replaceState(null, "", "/?memory=collection/s2&memoryCollectionProject=p&memoryCollectionTab=conversation");
  render(<MemoryCollections sessionId="s2" />);
  expect(await screen.findByText("content:s2")).toBeTruthy();
  const crumb = screen.getByRole("navigation").querySelector("a[href*='collection%2Fs1'], a[href*='collection/s1']");
  expect(crumb?.textContent).toBe("Archived one");
  expect(screen.queryByRole("button", { name: "Restore to normal session" })).toBeNull();
});

it("names the stored group in the table and shows each date once", async () => {
  render(<MemoryCollections sessionId="" />);
  const first = await screen.findByRole("link", { name: /Archived one/ });
  const second = screen.getByRole("link", { name: /Child task/ });
  expect(first.textContent).toContain("Alpha / Work");
  const dates = [first, second].map((row) => row.querySelector(".c.r")!.textContent);
  expect(dates[0]).toBeTruthy();
  expect(dates[1]).toBe("");
  expect(second.querySelector(".kbx-count")).toBeNull();
  expect(second.textContent).not.toContain("—");
  expect(screen.getByText(/1 in Knowledge Base/)).toBeTruthy();
});

it("shows only the conversation until a session has entries", async () => {
  window.history.replaceState(null, "", "/?memory=collection/s2&memoryCollectionProject=p&memoryCollectionTab=entries");
  render(<MemoryCollections sessionId="s2" />);
  expect(await screen.findByText("content:s2")).toBeTruthy();
  expect(screen.queryByRole("tablist")).toBeNull();
});

it("shows the archive as a tree by default, with projects closed until opened", async () => {
  window.history.replaceState(null, "", "/?memory=collections");
  render(<MemoryCollections sessionId="" />);
  const project = await screen.findByRole("button", { name: /Alpha/ });
  expect(project.getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryByRole("link", { name: /Archived one/ })).toBeNull();
  fireEvent.click(project);
  fireEvent.click(screen.getByRole("button", { name: /Work/ }));
  expect(screen.queryByRole("link", { name: /Child task/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Archived one" }));
  const child = screen.getByRole("link", { name: /Child task/ });
  expect(new URLSearchParams(child.getAttribute("href")!.split("?")[1]).get("memory")).toBe("collection/s2");
  // The opened rows are still open after leaving for a session and coming back.
  cleanup();
  render(<MemoryCollections sessionId="" />);
  expect(await screen.findByRole("link", { name: /Child task/ })).toBeTruthy();
});

it("orders tree rows and keeps what sits above a filtered session", () => {
  const session = (id: string, archivedAt: number, extra: object = {}) => ({ id, name: id, kind: "claude", count: 0, archivedAt, parentId: null, groupIds: [], groupPath: [], ...extra });
  const projects = [
    { id: "b", name: "Beta", kind: "project", count: 0, sessions: [session("old", 1), session("new", 2), session("grouped", 3, { groupIds: ["g"], groupPath: ["Group"], count: 4 })] },
    { id: "a", name: "Alpha", kind: "project", count: 0, sessions: [session("codex", 1, { kind: "codex" })] },
  ] as unknown as Parameters<typeof collectionTreeRows>[0];
  const names = (rows: ReturnType<typeof collectionTreeRows>) => rows.map((row) => row.name);
  const base = { expanded: new Set<string>(), sort: "newest", fallbackName: "Unknown" };
  expect(names(collectionTreeRows(projects, base))).toEqual(["Alpha", "Beta"]);
  const open = collectionTreeRows(projects, { ...base, openAll: true });
  expect(names(open)).toEqual(["Alpha", "codex", "Beta", "Group", "grouped", "new", "old"]);
  expect(open.find((row) => row.name === "Beta")).toMatchObject({ sessions: 3, knowledge: 4 });
  expect(names(collectionTreeRows(projects, { ...base, openAll: true, sort: "oldest" }))).toEqual(["Alpha", "codex", "Beta", "Group", "grouped", "old", "new"]);
  expect(names(collectionTreeRows(projects, { ...base, openAll: true, matches: (row) => row.count > 0 }))).toEqual(["Beta", "Group", "grouped"]);
});
