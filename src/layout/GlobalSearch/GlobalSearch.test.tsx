// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ContentSearchPage } from "../../ipc/commands";
import type { Project } from "../../types";
import { SYSTEM_CHAT, SYSTEM_PROJECTS, SYSTEM_TASKS } from "../LeftSidebar/systemCollections";
import { GlobalSearch } from "./GlobalSearch";

const api = vi.hoisted(() => ({
  start: vi.fn(),
  page: vi.fn(),
  cancel: vi.fn(),
}));

const storeState = vi.hoisted(() => ({
  setGlobalSearchOpen: vi.fn(),
  loadArchived: vi.fn().mockResolvedValue(undefined),
  openSession: vi.fn(),
  projects: [] as Project[],
  groups: [],
  sessions: [],
  archivedSessions: [],
  runtimes: {},
}));

vi.mock("../../ipc/commands", () => ({
  startContentSearch: api.start,
  contentSearchPage: api.page,
  cancelContentSearch: api.cancel,
  readAgentTranscript: vi.fn(),
}));
vi.mock("../../store/termStore", () => ({
  useTermStore: (selector: (s: typeof storeState) => unknown) => selector(storeState),
}));
vi.mock("../../hooks/nativeViewSuspend", () => ({
  useSuspendNativeViews: () => {},
}));
vi.mock("../sessionViewers/SessionContentViewer", () => ({
  SessionContentViewer: () => null,
}));

const makeProject = (id: string, name: string, rootPath: string, collectionId: string | null = null): Project => ({
  id,
  name,
  rootPath,
  collectionId,
  sortOrder: 0,
  collapsed: false,
  createdAt: 0,
});

const completePage = (jobId: string): ContentSearchPage => ({
  jobId,
  status: "complete",
  scannedSessions: 1,
  scannedBytes: 64,
  totalSessions: 1,
  totalMatches: 1,
  nextCursor: null,
  previousCursor: null,
  hits: [
    {
      sessionId: "s1",
      name: "Session 1",
      kind: "claude",
      archived: false,
      source: "transcript",
      matchCount: 1,
      matches: [{ messageIndex: 0, ordinal: 1, snippet: "needle", matched: ["needle"] }],
    },
  ],
});

beforeEach(() => {
  vi.useFakeTimers();
  api.start.mockReset();
  api.page.mockReset();
  api.cancel.mockReset().mockResolvedValue(undefined);
  storeState.projects = [
    makeProject(SYSTEM_PROJECTS, "Projects", "", null),
    makeProject(SYSTEM_TASKS, "Tasks", "", null),
    makeProject(SYSTEM_CHAT, "Chats", "", null),
    makeProject("col-work", "Work", "", SYSTEM_PROJECTS),
    makeProject("proj-a", "Alpha", "/tmp/alpha", "col-work"),
  ];
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

it("filters global search by a selected collection or project and resets to all", async () => {
  api.start
    .mockResolvedValueOnce({ jobId: "j1", status: "pending", scannedSessions: 0, scannedBytes: 0 })
    .mockResolvedValueOnce({ jobId: "j2", status: "pending", scannedSessions: 0, scannedBytes: 0 })
    .mockResolvedValueOnce({ jobId: "j3", status: "pending", scannedSessions: 0, scannedBytes: 0 })
    .mockResolvedValueOnce({ jobId: "j4", status: "pending", scannedSessions: 0, scannedBytes: 0 });
  api.page
    .mockResolvedValueOnce(completePage("j1"))
    .mockResolvedValueOnce(completePage("j2"))
    .mockResolvedValueOnce(completePage("j3"))
    .mockResolvedValueOnce(completePage("j4"));

  render(<GlobalSearch />);

  const input = screen.getByPlaceholderText("Search across all session content…");
  fireEvent.change(input, { target: { value: "needle" } });
  await advance(250);
  expect(api.start).toHaveBeenLastCalledWith("needle", "live", undefined);

  // Pick the Work collection.
  const trigger = screen.getByRole("combobox", { name: "Filter by project or collection" });
  fireEvent.click(trigger);
  fireEvent.click(screen.getByRole("option", { name: /Work/ }));
  expect(screen.getByPlaceholderText("Search in Work…")).toBeTruthy();
  await advance(250);
  expect(api.start).toHaveBeenLastCalledWith("needle", "live", "col-work");

  // Narrow further to project Alpha.
  fireEvent.click(trigger);
  fireEvent.click(screen.getByRole("option", { name: /Alpha/ }));
  expect(screen.getByPlaceholderText("Search in Alpha…")).toBeTruthy();
  await advance(250);
  expect(api.start).toHaveBeenLastCalledWith("needle", "live", "proj-a");

  // Reset back to all projects & collections.
  fireEvent.click(trigger);
  fireEvent.click(screen.getByRole("option", { name: "All projects & collections" }));
  expect(screen.getByPlaceholderText("Search across all session content…")).toBeTruthy();
  await advance(250);
  expect(api.start).toHaveBeenLastCalledWith("needle", "live", undefined);
});
