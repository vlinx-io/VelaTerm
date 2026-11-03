import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { setLang } from "../../i18n";
import { MemoryCompile, MemoryJobs } from "./MemoryTasks";
import type { MemoryJob } from "../../ipc/memory";

const api = vi.hoisted(() => ({ jobs: vi.fn(), cancel: vi.fn(), start: vi.fn() }));
vi.mock("../../ipc/memory", () => ({
  memoryJobs: api.jobs, memoryCancel: api.cancel, memoryStart: api.start, memoryRetry: vi.fn(),
  memoryOptions: async () => ({
    agents: [
      { id: "claude", label: "Claude", available: false },
      { id: "codex", label: "Codex", available: true },
      { id: "pi", label: "Pi", available: true },
    ],
    defaultAgent: "codex",
  }),
}));
vi.mock("../../ipc/launch", () => ({
  launchOptions: async () => [{ id: "codex", label: "Codex", acceptsTask: true, effortFlag: "--effort", effortLevels: ["high"], supportsPlanExecute: false, supportsReferSummary: false }],
  launchModels: async () => ({ models: [], effortLevels: [] }),
}));
vi.mock("../../store/termStore", () => ({
  useTermStore: Object.assign(
    (select: (s: unknown) => unknown) => select({ sessions: [{ id: "session", name: "Session A" }], archivedSessions: [] }),
    { getState: () => ({ memoryPrefs: {}, setMemoryPrefs: vi.fn() }) },
  ),
}));
const job = (id: string, status: string): MemoryJob => ({
  id, status, sessionName: `Session ${id}`, sourceId: `source-${id}`, agent: "codex", agentLabel: "Codex", model: "", effort: "",
  stage: "extract", progress: 0, total: 1, error: "", entries: [], entryLinks: [], createdAt: 0, updatedAt: 0,
});
beforeEach(() => {
  setLang("en"); window.history.replaceState(null, "", "/?memory=jobs");
  api.jobs.mockReset(); api.cancel.mockReset(); api.start.mockReset();
});
afterEach(cleanup);

it("explains replacement and opens the new job returned by submission", async () => {
  api.start.mockResolvedValue({ id: "replacement", reused: false });
  render(<MemoryCompile sessionId="session" />);
  const agents = await screen.findByRole("combobox", { name: "Agent" });
  await waitFor(() => expect(agents.textContent).toContain("Codex"));
  expect(screen.getByText(/Submitting again cancels any unfinished task/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Organize and save" }));
  await waitFor(() => expect(api.start).toHaveBeenCalledWith("session", "codex", "", ""));
  await waitFor(() => expect(new URLSearchParams(location.search).get("memory")).toBe("job/replacement"));
  expect(screen.queryByRole("alert")).toBeNull();
});

it("shows parallel sessions and lets the user cancel a replacement waiting for its predecessor", async () => {
  const jobs = [job("A", "running"), job("B", "running"), job("old", "cancelling"), job("latest", "queued")];
  api.jobs.mockResolvedValue({ jobs, total: jobs.length, pageSize: 40 });
  api.cancel.mockImplementation(async (id) => {
    api.jobs.mockResolvedValue({ jobs: jobs.map(j => j.id === id ? { ...j, status: "cancelled" } : j), total: jobs.length, pageSize: 40 });
  });
  render(<MemoryJobs />);
  expect(await screen.findAllByText("In progress")).toHaveLength(2);
  const old = (await screen.findByRole("link", { name: "Session old" })).closest("article")!;
  expect(within(old).getByText("Cancelling")).toBeTruthy();
  expect(within(old).queryByRole("button", { name: "Cancel" })).toBeNull();
  const latest = screen.getByRole("link", { name: "Session latest" }).closest("article")!;
  expect(within(latest).getByRole("status").textContent).toContain("once the previous task");
  expect(within(latest).getByRole("link", { name: "Session latest" }).getAttribute("href")).toContain("memory=job%2Flatest");
  fireEvent.click(within(latest).getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(api.cancel).toHaveBeenCalledWith("latest"));
  await screen.findByText("Cancelled");
  expect(screen.getAllByText("In progress")).toHaveLength(2);
});

it("links a finished job to the entries it produced by title", async () => {
  const single = { ...job("one", "completed"), entries: ["e1"], entryLinks: [{ id: "e1", title: "First entry" }] };
  const multiple = { ...job("two", "completed"), entries: ["e2", "e3"], entryLinks: [{ id: "e2", title: "Second" }, { id: "e3", title: "Third" }] };
  api.jobs.mockResolvedValue({ jobs: [single, multiple], total: 2, pageSize: 40 });
  render(<MemoryJobs />);
  // With a single entry the job title opens that entry directly.
  expect((await screen.findByRole("link", { name: "Session one" })).getAttribute("href")).toContain("memory=entry%2Fe1");
  expect(screen.getByRole("link", { name: "Session two" }).getAttribute("href")).toContain("memory=job%2Ftwo");
  expect(screen.getByRole("link", { name: "First entry ↗" }).getAttribute("href")).toContain("memory=entry%2Fe1");
  expect(screen.getByRole("link", { name: "Third ↗" }).getAttribute("href")).toContain("memory=entry%2Fe3");
});

it("offers every organizer agent and starts the job with the one that was picked", async () => {
  api.start.mockResolvedValue({ id: "picked", reused: false });
  render(<MemoryCompile sessionId="session" />);
  const agents = await screen.findByRole("combobox", { name: "Agent" });
  fireEvent.click(agents);
  // An agent whose CLI is missing stays listed and stays unselectable, so its absence is visible.
  const unavailable = screen.getByRole("option", { name: /Claude/ });
  expect(unavailable.getAttribute("aria-disabled")).toBe("true");
  fireEvent.click(screen.getByRole("option", { name: "Pi" }));
  await waitFor(() => expect(agents.textContent).toContain("Pi"));
  fireEvent.click(screen.getByRole("button", { name: "Organize and save" }));
  await waitFor(() => expect(api.start).toHaveBeenCalledWith("session", "pi", "", ""));
});

it("names the agent of a past job from the backend label", async () => {
  const past = { ...job("A", "completed"), agent: "omp", agentLabel: "OMP" };
  api.jobs.mockResolvedValue({ jobs: [past], total: 1, pageSize: 40 });
  render(<MemoryJobs />);
  const article = (await screen.findByRole("link", { name: "Session A" })).closest("article")!;
  expect(within(article).getByText(/OMP/)).toBeTruthy();
});
