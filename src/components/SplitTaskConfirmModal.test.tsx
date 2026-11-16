import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SplitProposal } from "../ipc/launch";

const mocks = vi.hoisted(() => ({
  pending: vi.fn(), read: vi.fn(), confirm: vi.fn(), cancel: vi.fn(), options: vi.fn(), models: vi.fn(),
  event: null as null | ((value: {runId: string; resolved?: boolean}) => void),
  store: { loadTree: vi.fn(), openSession: vi.fn(), pendingSpawns: [] as unknown[] },
}));
vi.mock("../i18n", () => ({ useT: () => (key: string, n?: number) => n == null ? key : `${key}:${n}` }));
vi.mock("../hooks/nativeViewSuspend", () => ({ useSuspendNativeViews: () => {} }));
vi.mock("../ipc/transport", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../ipc/transport")>()),
  isTauri: true,
  listen: (_name: string, callback: typeof mocks.event) => { mocks.event = callback; return Promise.resolve(() => {}); },
}));
vi.mock("../ipc/wsClient", () => ({ wsClient: {} }));
vi.mock("../store/termStore", () => ({ useTermStore: Object.assign((selector: (state: typeof mocks.store) => unknown) => selector(mocks.store), { getState: () => mocks.store }) }));
vi.mock("../ipc/launch", () => ({
  pendingSplitProposals: mocks.pending, readSplitProposal: mocks.read,
  confirmSplitProposal: mocks.confirm, cancelSplitProposal: mocks.cancel,
  launchOptions: mocks.options, launchModels: mocks.models,
}));
import { SplitTaskConfirmModal } from "./SplitTaskConfirmModal";

function proposal(): SplitProposal {
  return { runId: "run", proposalId: "proposal", state: "pending", plannerId: "planner", plannerName: "Planner", cwd: "/repo", task: "Original task",
    tasks: [
      { name: "Parser", prompt: "Parser assignment", config: { agent: "claude", model: "opus", effort: "high" } },
      { name: "Settings", prompt: "Settings assignment", config: { agent: "codex", model: "gpt-5.6-sol", effort: "medium" } },
    ], run: { id: "run", state: "awaiting_confirmation", summary: "" }, executionTasks: [] };
}
beforeEach(() => {
  vi.clearAllMocks();
  mocks.store.pendingSpawns = [];
  mocks.pending.mockResolvedValue(["run"]); mocks.read.mockResolvedValue(proposal());
  mocks.confirm.mockResolvedValue({ tasks: [], errors: [] }); mocks.cancel.mockResolvedValue({});
  mocks.store.loadTree.mockResolvedValue(undefined);
  mocks.options.mockResolvedValue(["claude", "codex"].map(id => ({ id, label: id, supportsPlanExecute: true, acceptsTask: true, effortFlag: "--effort", effortLevels: ["low", "medium", "high"] })));
  mocks.models.mockResolvedValue({ models: [], effortLevels: ["low", "medium", "high"] });
  window.history.replaceState(null, "", "/?splitReview=run");
});
afterEach(() => { cleanup(); window.history.replaceState(null, "", "/"); });

it.each(["none", "shared", "each"] as const)("restores a %s proposal and keeps task edits while navigating before confirmation", async mode => {
  const saved = proposal();
  saved.run.config = { worktreeMode: mode, plan: {}, exec: {} };
  mocks.read.mockResolvedValue(saved);
  render(<SplitTaskConfirmModal />);
  await screen.findByLabelText("orch.promptLabel");
  expect(screen.getByText(mode === "each" ? "launch.workflowDirectoryEachHint" : "launch.splitSharedDirectory")).toBeTruthy();
  expect(mocks.confirm).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("orch.promptLabel"), { target: { value: "Edited parser assignment" } });
  fireEvent.click(screen.getByLabelText("spawn.modelLabel"));
  fireEvent.change(screen.getByPlaceholderText("chat.filterPlaceholder"), { target: { value: "approved-model" } });
  fireEvent.click(screen.getAllByRole("option")[0]);
  fireEvent.click(screen.getByRole("link", { name: "2. Settings" }));
  expect(new URLSearchParams(window.location.search).get("splitTask")).toBe("1");
  fireEvent.change(screen.getByLabelText("orch.promptLabel"), { target: { value: "Edited settings assignment" } });
  fireEvent.click(screen.getByRole("link", { name: "1. Parser" }));
  expect((screen.getByLabelText("orch.promptLabel") as HTMLTextAreaElement).value).toBe("Edited parser assignment");
  expect(screen.getByLabelText("spawn.modelLabel").textContent).toContain("approved-model");
  fireEvent.click(screen.getByRole("button", { name: "orch.launch:2" }));
  await waitFor(() => expect(mocks.confirm).toHaveBeenCalledOnce());
  expect(mocks.confirm.mock.calls[0][0]).toEqual({ runId: "run", proposalId: "proposal", tasks: [
    { name: "Parser", prompt: "Edited parser assignment", config: { agent: "claude", model: "approved-model", effort: "high" } },
    { name: "Settings", prompt: "Edited settings assignment", config: { agent: "codex", model: "gpt-5.6-sol", effort: "medium" } },
  ] });
  await waitFor(() => expect(window.location.search).toBe(""));
});

it("removes and restores tasks and cancels explicitly without starting execution", async () => {
  render(<SplitTaskConfirmModal />); await screen.findByLabelText("orch.promptLabel");
  fireEvent.click(screen.getByRole("button", { name: "orch.remove" }));
  expect(screen.queryByRole("link", { name: "1. Parser" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "launch.undoRemove" }));
  expect(screen.getByRole("link", { name: "1. Parser" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));
  await waitFor(() => expect(mocks.cancel).toHaveBeenCalledWith("run", "proposal"));
  expect(mocks.confirm).not.toHaveBeenCalled();
});

it("closing leaves the persisted proposal available through a real link", async () => {
  render(<SplitTaskConfirmModal />); await screen.findByLabelText("orch.promptLabel");
  fireEvent.click(screen.getByRole("button", { name: "common.close" }));
  const link = await screen.findByRole("link", { name: "launch.splitReview · 1" });
  expect(link.getAttribute("href")).toContain("splitReview=run");
  expect(mocks.cancel).not.toHaveBeenCalled();
  fireEvent.click(link); await screen.findByLabelText("orch.promptLabel");
  expect(mocks.read).toHaveBeenCalledTimes(2);
});

it("invalid tasks block launch and a confirmation by another client closes editing", async () => {
  render(<SplitTaskConfirmModal />); await screen.findByLabelText("orch.promptLabel");
  fireEvent.change(screen.getByLabelText("orch.promptLabel"), { target: { value: " " } });
  expect((screen.getByRole("button", { name: "orch.launch:2" }) as HTMLButtonElement).disabled).toBe(true);
  const value = proposal(); value.state = "confirmed"; value.run.state = "executing";
  mocks.read.mockResolvedValue(value); mocks.pending.mockResolvedValue([]);
  await act(async () => mocks.event?.({ runId: "run", resolved: true }));
  await screen.findByText("launch.splitConfirmed");
  expect(screen.queryByLabelText("orch.promptLabel")).toBeNull();
  expect(mocks.confirm).not.toHaveBeenCalled();
});

it("excludes unsupported agents and requires repairing an obsolete task selection", async () => {
  const value = proposal(); value.tasks[0].config.agent = "terminal";
  mocks.read.mockResolvedValue(value);
  const options = await mocks.options();
  mocks.options.mockResolvedValue([...options, { id: "terminal", label: "Terminal", supportsPlanExecute: false }]);
  render(<SplitTaskConfirmModal />);
  await screen.findByLabelText("orch.promptLabel");
  expect((screen.getByRole("button", { name: "orch.launch:2" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByLabelText("spawn.agentLabel"));
  expect(screen.getAllByRole("option").map(option => option.textContent)).toEqual(["claude", "codex"]);
  fireEvent.click(screen.getByRole("option", { name: "claude" }));
  fireEvent.click(screen.getByRole("button", { name: "orch.launch:2" }));
  await waitFor(() => expect(mocks.confirm).toHaveBeenCalledOnce());
  expect(mocks.confirm.mock.calls[0][0].tasks[0].config.agent).toBe("claude");
});

it("opens a newly proposed batch and preserves all pending batches", async () => {
  window.history.replaceState(null, "", "/"); mocks.pending.mockResolvedValue([]);
  render(<SplitTaskConfirmModal />); await waitFor(() => expect(mocks.pending).toHaveBeenCalled());
  mocks.pending.mockResolvedValue(["run", "another-run"]);
  await act(async () => mocks.event?.({ runId: "run" }));
  await screen.findByLabelText("orch.promptLabel");
  fireEvent.click(screen.getByRole("button", { name: "common.close" }));
  expect(await screen.findByRole("link", { name: "launch.splitReview · 2" })).toBeTruthy();
});

it.each(["menu", "spawn"])("does not cover an active %s launch review with pending tasks or an automatic proposal popup", async entry => {
  window.history.replaceState(null, "", entry === "menu" ? "/?planExecute=draft&planProject=project" : "/");
  if (entry === "spawn") mocks.store.pendingSpawns = [{}];
  const view = render(<SplitTaskConfirmModal />);
  await waitFor(() => expect(mocks.pending).toHaveBeenCalled());
  await act(async () => mocks.event?.({ runId: "run" }));
  expect(screen.queryByRole("link")).toBeNull();
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(mocks.read).not.toHaveBeenCalled();
  mocks.store.pendingSpawns = [];
  act(() => { window.history.pushState(null, "", "/"); window.dispatchEvent(new PopStateEvent("popstate")); });
  view.rerender(<SplitTaskConfirmModal />);
  expect(await screen.findByRole("link", { name: "launch.splitReview · 1" })).toBeTruthy();
});
