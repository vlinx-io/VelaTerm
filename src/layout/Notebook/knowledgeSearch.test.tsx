// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { KnowledgeNavigation } from "./KnowledgeNavigation";
import { MemoryRoute } from "../Memory/MemoryRoute";

const api = vi.hoisted(() => ({
  overview: vi.fn(),
  tree: vi.fn(),
  search: vi.fn(),
  list: vi.fn(),
  collections: vi.fn(),
  restore: vi.fn(),
  env: { isBrowser: true },
}));
vi.mock("../../ipc/notebook", () => ({
  notebookOverview: api.overview,
  notebookTree: api.tree,
  notebookSearch: api.search,
  notebookRestore: api.restore,
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
vi.mock("../../ipc/memory", () => ({ memoryAttention: () => Promise.resolve(0), memoryList: api.list, memoryGet: vi.fn(), memoryCollections: api.collections }));
vi.mock("../../platform", () => ({ platform: { env: api.env, dialog: { pickDirectory: vi.fn() } } }));
vi.mock("../../remote/ServerFileBrowser", () => ({
  cardStyle: {},
  joinPath: (parent: string, name: string) => `${parent.replace(/\/$/, "")}/${name}`,
  useServerBrowser: () => ({ selectedDir: "/existing/Markdown" }),
  ServerBrowserView: () => <div>Server directory picker</div>,
}));

const vault = { id: "vault-1", name: "Personal", root: "/notes" };
const node = { path: "Projects/Note.md", absolutePath: "/notes/Projects/Note.md", name: "Note.md", kind: "note", size: 10, updatedAt: 1, favorite: false, tags: [], summary: "Body" };
const hit = { vaultId: "vault-1", vaultName: "Personal", path: "Tea/Tea note.md", absolutePath: "/notes/Tea/Tea note.md", name: "Tea note.md", summary: "Brewing oolong, 90°C", line: 3, matches: 2, score: 115, updatedAt: 1700000000000, favorite: false, tags: [], related: [{ path: "Tea/Brewing.md", name: "Brewing.md", absolutePath: "/notes/Tea/Brewing.md" }] };

beforeEach(() => {
  setLang("en");
  api.env.isBrowser = true;
  window.history.replaceState(null, "", "/");
  useTermStore.setState({ docTabs: {}, openTabs: [], activeTabId: null });
  api.list.mockReset().mockResolvedValue({ projects: [], selectedSessionId: null, entries: [], total: 0, pageSize: 40, tags: [] });
  api.collections.mockReset().mockResolvedValue({ projects: [], sessions: [] });
  api.overview.mockReset().mockResolvedValue({ vaults: [vault], defaultRoot: "/Notes", limits: { chunkBytes: 1024, noteBytes: 10000, assetBytes: 10000, files: 100 } });
  api.tree.mockReset().mockResolvedValue({ vault, nodes: [node], entries: [node], tags: [], trash: [], skipped: 0 });
  api.search.mockReset().mockResolvedValue({ entries: [hit], total: 1, hasMore: false, unavailable: [] });
  api.restore.mockReset().mockResolvedValue({ path: "Restored.md" });
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const opened = (path: string) => Object.values(useTermStore.getState().docTabs).some((tab) => tab.path === path);

it("searches the current notebook from the workspace and opens the hit in a document tab", async () => {
  window.history.replaceState(null, "", "/?memory=notebook/vault-1");
  render(<MemoryRoute />);
  const field = await screen.findByRole("textbox", { name: "Search notes…" });
  fireEvent.change(field, { target: { value: "oolong" } });

  const row = await screen.findByRole("option", { name: /Tea note/ }, { timeout: 3000 });
  expect(api.search).toHaveBeenCalledWith("oolong", "vault-1", 100);
  expect(row.textContent).toContain("Personal · Tea/Tea note.md");
  // The snippet keeps the surrounding text and marks the matched words instead of showing a bare line.
  expect(row.textContent).toContain("Brewing oolong, 90°C");
  expect(document.querySelector("mark")?.textContent).toBe("oolong");
  expect(screen.getByText("1 results")).toBeTruthy();
  expect(row.textContent).toContain("Line 3");

  fireEvent.click(row);
  expect(opened("/notes/Tea/Tea note.md")).toBe(true);
});

it("widens the workspace search to every notebook through the scope selector", async () => {
  window.history.replaceState(null, "", "/?memory=notebook/vault-1");
  render(<MemoryRoute />);
  fireEvent.change(await screen.findByRole("textbox", { name: "Search notes…" }), { target: { value: "oolong" } });
  await screen.findByRole("option", { name: /Tea note/ }, { timeout: 3000 });

  fireEvent.click(screen.getByRole("combobox", { name: "Search scope" }));
  fireEvent.click(screen.getByRole("option", { name: "All knowledge bases" }));
  await waitFor(() => expect(api.search).toHaveBeenLastCalledWith("oolong", "", 100));
  expect(new URLSearchParams(location.search).get("memoryScope")).toBe("all");
  expect(screen.getByRole("option", { name: /Tea note/ })).toBeTruthy();
});

it("reports an empty result instead of an empty list", async () => {
  api.search.mockResolvedValue({ entries: [], total: 0, hasMore: false, unavailable: [] });
  window.history.replaceState(null, "", "/?memory=notebook/vault-1");
  render(<MemoryRoute />);
  fireEvent.change(await screen.findByRole("textbox", { name: "Search notes…" }), { target: { value: "nothing-matches" } });
  expect(await screen.findByText("No notes match this search.", {}, { timeout: 3000 })).toBeTruthy();
});

it("replaces the inspector tree with hits and opens the highlighted one on Enter", async () => {
  render(<KnowledgeNavigation />);
  fireEvent.change(await screen.findByRole("textbox", { name: "Search knowledge and notes…" }), { target: { value: "oolong" } });

  const row = await screen.findByRole("option", { name: /Tea note/ }, { timeout: 3000 });
  expect(screen.queryByText("Local knowledge bases")).toBeNull();
  expect(row.getAttribute("aria-selected")).toBe("true");
  fireEvent.keyDown(screen.getByRole("textbox", { name: "Search knowledge and notes…" }), { key: "Enter" });
  expect(opened("/notes/Tea/Tea note.md")).toBe(true);
});

it("opens a related note from inside a hit without treating it as the hit itself", async () => {
  window.history.replaceState(null, "", "/?memory=notebook/vault-1");
  render(<MemoryRoute />);
  fireEvent.change(await screen.findByRole("textbox", { name: "Search notes…" }), { target: { value: "oolong" } });
  const related = await screen.findByRole("button", { name: "Brewing" }, { timeout: 3000 });
  fireEvent.click(related);
  expect(opened("/notes/Tea/Brewing.md")).toBe(true);
  expect(opened("/notes/Tea/Tea note.md")).toBe(false);
});
