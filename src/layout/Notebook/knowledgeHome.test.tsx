// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { MemoryRoute } from "../Memory/MemoryRoute";

const api = vi.hoisted(() => ({ overview: vi.fn(), tree: vi.fn(), search: vi.fn(), list: vi.fn(), collections: vi.fn(), get: vi.fn(), env: { isBrowser: true } }));
vi.mock("../../ipc/notebook", () => ({
  notebookOverview: api.overview,
  notebookTree: api.tree,
  notebookSearch: api.search,
  notebookRestore: vi.fn(),
  notebookGet: vi.fn(),
  notebookSave: vi.fn(),
  notebookStat: vi.fn(),
  notebookRegister: vi.fn(),
  notebookCreate: vi.fn(),
  notebookMove: vi.fn(),
  notebookTrash: vi.fn(),
  notebookUnregister: vi.fn(),
  notebookAsset: vi.fn(),
  notebookFavorite: vi.fn(),
  uploadNotebookFiles: vi.fn(),
  notebookDroppedFiles: vi.fn(),
}));
vi.mock("../../ipc/memory", () => ({ memoryAttention: () => Promise.resolve(0), memoryList: api.list, memoryGet: api.get, memoryCollections: api.collections }));
vi.mock("../../platform", () => ({ platform: { env: api.env, dialog: { pickDirectory: vi.fn() } } }));
vi.mock("../../remote/ServerFileBrowser", () => ({
  cardStyle: {},
  joinPath: (parent: string, name: string) => `${parent.replace(/\/$/, "")}/${name}`,
  useServerBrowser: () => ({ selectedDir: "/existing/Markdown" }),
  ServerBrowserView: () => <div>Server directory picker</div>,
}));

const vault = { id: "vault-1", name: "Personal", root: "/notes" };
const noteHit = { vaultId: "vault-1", vaultName: "Personal", path: "Tea/Tea note.md", absolutePath: "/notes/Tea/Tea note.md", name: "Tea note.md", summary: "Brewing oolong, 90°C", line: 3, matches: 2, score: 115, updatedAt: 1700000000000, favorite: false, tags: [], matched: ["oolong"], related: [] };
const entryHit = { id: "entry-1", sessionId: "s", title: "Throttle design", summary: "Output scheduler", snippet: "the throttle window is 12 ms", line: 4, matched: ["throttle"], tags: ["scheduling"], version: 1, updatedAt: 1700000000000, sourceCount: 1, related: [{ id: "entry-2", title: "Scheduler notes" }] };
const projects = [{ id: "p", name: "VelaTerm", kind: "project", count: 1, sessions: [{ id: "s", name: "Design session", kind: "session", count: 1 }] }];

beforeEach(() => {
  setLang("en");
  api.env.isBrowser = true;
  window.history.replaceState(null, "", "/?memory=notebooks");
  useTermStore.setState({ docTabs: {}, openTabs: [], activeTabId: null });
  api.overview.mockReset().mockResolvedValue({ vaults: [vault], defaultRoot: "/Notes", limits: { chunkBytes: 1024, noteBytes: 10000, assetBytes: 10000, files: 100 } });
  api.tree.mockReset().mockResolvedValue({ vault, nodes: [], entries: [], tags: [], trash: [], skipped: 0 });
  api.search.mockReset().mockResolvedValue({ entries: [noteHit], total: 1, hasMore: false, unavailable: [] });
  api.list.mockReset().mockResolvedValue({ projects, selectedSessionId: null, entries: [entryHit], total: 1, pageSize: 40, tags: [] });
  api.collections.mockReset().mockResolvedValue({ projects: [], sessions: [] });
  api.get.mockReset().mockResolvedValue({ entry: { id: "entry-1", version: 1, title: "Throttle design", summary: "Output scheduler", content: "Body", tags: [], related: [], sources: [] }, catalog: [] });
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const opened = (path: string) => Object.values(useTermStore.getState().docTabs).some((tab) => tab.path === path);
const field = () => screen.findByRole("textbox", { name: "Search session knowledge and local notes…" });

it("searches both sources from the home page and groups the hits", async () => {
  render(<MemoryRoute />);
  fireEvent.change(await field(), { target: { value: "throttle" } });

  const entry = await screen.findByRole("option", { name: /Throttle design/ }, { timeout: 3000 });
  expect(api.search).toHaveBeenCalledWith("throttle", "", 50);
  expect(api.list).toHaveBeenCalledWith({ query: "throttle", tag: "", sort: "updated", page: 0 });
  expect(screen.getByRole("heading", { name: /Session Knowledge Base/ })).toBeTruthy();
  expect(screen.getByRole("heading", { name: /Local knowledge bases/ })).toBeTruthy();
  expect(entry.textContent).toContain("VelaTerm / Design session");
  expect(entry.textContent).toContain("the throttle window is 12 ms");
  expect([...document.querySelectorAll("mark")].some((mark) => mark.textContent === "throttle")).toBe(true);
  expect(screen.getByText("2 results")).toBeTruthy();

  fireEvent.click(entry);
  expect(new URLSearchParams(location.search).get("memory")).toBe("entry/entry-1");
});

it("opens the active note hit in a document tab", async () => {
  render(<MemoryRoute />);
  const search = await field();
  fireEvent.change(search, { target: { value: "oolong" } });
  await screen.findByRole("option", { name: /Tea note/ }, { timeout: 3000 });

  fireEvent.keyDown(search, { key: "ArrowDown" });
  fireEvent.keyDown(search, { key: "Enter" });
  expect(opened("/notes/Tea/Tea note.md")).toBe(true);
  expect(new URLSearchParams(location.search).has("memory")).toBe(false);
});

it("clears the search on Escape and reports an empty result otherwise", async () => {
  api.search.mockResolvedValue({ entries: [], total: 0, hasMore: false, unavailable: [] });
  api.list.mockResolvedValue({ projects, selectedSessionId: null, entries: [], total: 0, pageSize: 40, tags: [] });
  render(<MemoryRoute />);
  const search = await field();
  fireEvent.change(search, { target: { value: "nothing-matches" } });
  expect(await screen.findByText("Nothing matches this search.", {}, { timeout: 3000 })).toBeTruthy();

  fireEvent.keyDown(search, { key: "Escape" });
  await waitFor(() => expect(screen.queryByText("Nothing matches this search.")).toBeNull());
  expect((search as HTMLInputElement).value).toBe("");
  expect(screen.getByRole("heading", { name: "Session Knowledge Base" })).toBeTruthy();
});

it("says when only approximate results were found", async () => {
  api.search.mockResolvedValue({ entries: [], total: 0, hasMore: false, unavailable: [], fuzzy: true });
  api.list.mockResolvedValue({ projects, selectedSessionId: null, entries: [entryHit], total: 1, pageSize: 40, tags: [], fuzzy: true });
  render(<MemoryRoute />);
  fireEvent.change(await field(), { target: { value: "throtle" } });
  expect(await screen.findByText("No exact matches. Showing approximate results.", {}, { timeout: 3000 })).toBeTruthy();
  expect(await screen.findByRole("option", { name: /Throttle design/ })).toBeTruthy();
});

it("walks back up from a session group to the knowledge-base home", async () => {
  window.history.replaceState(null, "", "/?memory=library&memoryProject=p&memorySession=s");
  render(<MemoryRoute />);
  const up = () => screen.findByRole("link", { name: "Up one level" });
  const route = () => Object.fromEntries(new URLSearchParams(location.search));

  fireEvent.click(await up());
  await waitFor(() => expect(route()).toEqual({ memory: "library", memoryProject: "p" }));
  fireEvent.click(await up());
  await waitFor(() => expect(route()).toEqual({ memory: "library" }));
  fireEvent.click(await up());
  await waitFor(() => expect(route()).toEqual({ memory: "notebooks" }));
  expect(screen.queryByRole("link", { name: "Up one level" })).toBeNull();
});

it("leaves a nested folder for its parent, then the vault, then the home page", async () => {
  window.history.replaceState(null, "", "/?memory=notebook/vault-1&memoryFolder=Tea/Green");
  render(<MemoryRoute />);
  const up = () => screen.findByRole("link", { name: "Up one level" });
  const route = () => Object.fromEntries(new URLSearchParams(location.search));

  fireEvent.click(await up());
  await waitFor(() => expect(route()).toEqual({ memory: "notebook/vault-1", memoryFolder: "Tea" }));
  fireEvent.click(await up());
  await waitFor(() => expect(route()).toEqual({ memory: "notebook/vault-1" }));
  fireEvent.click(await up());
  await waitFor(() => expect(route()).toEqual({ memory: "notebooks" }));
});

it("explains a source only while it has nothing to show", async () => {
  render(<MemoryRoute />);
  await screen.findAllByText("Throttle design");
  await waitFor(() => expect(screen.getByText(/Markdown folders on this computer/)).toBeTruthy());
  expect(screen.queryByText(/Knowledge organized from your sessions/)).toBeNull();
  expect(screen.getByText(/Finished conversations/)).toBeTruthy();
});
