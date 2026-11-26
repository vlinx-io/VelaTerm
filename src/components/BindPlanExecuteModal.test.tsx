import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlanExecuteBindPreparation } from "../ipc/launch";

const mocks = vi.hoisted(() => ({
  preparePlanExecuteBind: vi.fn(),
  bindPlanExecute: vi.fn(),
  planExecuteDefaults: vi.fn(),
  applyLaunchArgs: vi.fn(),
  launchOptions: vi.fn(),
  launchModels: vi.fn(),
  loadTree: vi.fn(),
  openSession: vi.fn(),
}));

vi.mock("../i18n", () => ({
  t: (key: string, ...args: unknown[]) => (args.length ? `${key}:${args.join(",")}` : key),
  useT: () => (key: string, ...args: unknown[]) =>
    args.length ? `${key}:${args.join(",")}` : key,
}));

vi.mock("../ipc/launch", () => ({
  preparePlanExecuteBind: mocks.preparePlanExecuteBind,
  bindPlanExecute: mocks.bindPlanExecute,
  planExecuteDefaults: mocks.planExecuteDefaults,
  applyLaunchArgs: mocks.applyLaunchArgs,
  launchOptions: mocks.launchOptions,
  launchModels: mocks.launchModels,
}));

vi.mock("../ipc/events", () => ({
  onAntigravityModelsChanged: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("../hooks/nativeViewSuspend", () => ({
  useSuspendNativeViews: () => {},
}));

vi.mock("../store/termStore", () => {
  const state = {
    groups: [{ id: "g1", projectId: "p1", name: "Group A" }],
    projects: [{ id: "p1", name: "Project 1", rootPath: "/tmp/p1" }],
    chatImageMaxMb: 10,
    loadTree: mocks.loadTree,
    openSession: mocks.openSession,
  };
  const useTermStore = Object.assign(
    (selector: (s: typeof state) => unknown) => selector(state),
    { getState: () => state },
  );
  return { useTermStore };
});

import { BindPlanExecuteModal } from "./BindPlanExecuteModal";

const DEFAULT_OPTIONS = [
  {
    id: "claude",
    label: "Claude",
    supportsPlanExecute: true,
    supportsReferSummary: true,
    acceptsTask: true,
    effortFlag: "--effort",
    effortLevels: ["low", "medium", "high"],
  },
  {
    id: "codex",
    label: "Codex",
    supportsPlanExecute: true,
    supportsReferSummary: true,
    acceptsTask: true,
    effortFlag: "--effort",
    effortLevels: ["low", "medium", "high"],
  },
];

function openBindRoute(search: string) {
  const url = new URL(window.location.href);
  url.search = search;
  window.history.replaceState(null, "", url.href);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

describe("BindPlanExecuteModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.launchOptions.mockResolvedValue(DEFAULT_OPTIONS);
    mocks.launchModels.mockResolvedValue({ models: [], effortLevels: ["low", "medium", "high"] });
    mocks.applyLaunchArgs.mockResolvedValue(null);
    mocks.loadTree.mockResolvedValue(undefined);
    mocks.planExecuteDefaults.mockResolvedValue({
      reviewEnabled: false,
      worktreeMode: "none",
      splitTasks: false,
      plan: { agent: "claude" },
      exec: { agent: "claude" },
      review: { agent: "claude" },
    });
    openBindRoute("");
  });

  afterEach(() => {
    openBindRoute("");
  });

  it("preselects direct child executors from single-session entry and submits cleanly", async () => {
    const prep: PlanExecuteBindPreparation = {
      plannerId: "s-plan",
      executorIds: ["s-child"],
      candidates: [
        {
          id: "s-plan",
          projectId: "p1",
          groupId: null,
          parentSessionId: null,
          name: "Plan Session",
          kind: "claude",
          engine: "chat",
          cwd: "/tmp/p1",
          worktreePath: null,
          eligible: true,
          ineligibleReason: null,
          activeRole: null,
          extendablePlanner: false,
          existingRun: null,
        },
        {
          id: "s-child",
          projectId: "p1",
          groupId: null,
          parentSessionId: "s-plan",
          name: "Child Worker",
          kind: "codex",
          engine: "chat",
          cwd: "/tmp/p1",
          worktreePath: null,
          eligible: true,
          ineligibleReason: null,
          activeRole: null,
          extendablePlanner: false,
          existingRun: null,
        },
      ],
      defaultConfig: {
        reviewEnabled: false,
        worktreeMode: "none",
        splitTasks: false,
        plan: { agent: "claude" },
        exec: { agent: "claude" },
        review: { agent: "claude" },
      },
    };
    mocks.preparePlanExecuteBind.mockResolvedValueOnce(prep);
    mocks.bindPlanExecute.mockResolvedValueOnce({
      planner: { id: "s-plan" },
      run: { id: "req-100", state: "planning", summary: "" },
      tasks: [],
      delivery: { delivery: "sent" },
    });

    openBindRoute("?bindPlanExecute=req-100&bindPlan=s-plan");
    render(<BindPlanExecuteModal />);

    await waitFor(() => {
      expect(mocks.preparePlanExecuteBind).toHaveBeenCalledWith(["s-plan"], undefined);
    });

    // Single child is already a direct child of s-plan, so reparent checkbox is hidden.
    expect(screen.queryByText("bindPlanExecute.reparentLabel")).toBeNull();
    // Single executor bound -> dynamic exec config & splitTasks checkbox are hidden.
    expect(screen.queryByText("launch.splitTasks")).toBeNull();

    const submitBtn = await screen.findByRole("button", { name: "bindPlanExecute.submit" });
    await waitFor(() => expect((submitBtn as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(submitBtn);

    await waitFor(() => {
      expect(mocks.bindPlanExecute).toHaveBeenCalledWith(
        expect.objectContaining({
          requestId: "req-100",
          plannerId: "s-plan",
          executorIds: ["s-child"],
          reparentExecutors: false,
        }),
      );
    });
    expect(mocks.loadTree).toHaveBeenCalled();
    expect(mocks.openSession).toHaveBeenCalledWith("s-plan");
    expect(new URL(window.location.href).searchParams.has("bindPlanExecute")).toBe(false);
  });

  it("shows dynamic exec & worktree config when 0 executors are selected and handles blocked delivery retry", async () => {
    const prep: PlanExecuteBindPreparation = {
      plannerId: "s-plan",
      executorIds: [],
      candidates: [
        {
          id: "s-plan",
          projectId: "p1",
          groupId: null,
          parentSessionId: null,
          name: "Solo Plan",
          kind: "claude",
          engine: "chat",
          cwd: "/tmp/p1",
          worktreePath: null,
          eligible: true,
          ineligibleReason: null,
          activeRole: null,
          extendablePlanner: false,
          existingRun: null,
        },
      ],
      defaultConfig: {
        reviewEnabled: false,
        worktreeMode: "none",
        splitTasks: false,
        plan: { agent: "claude" },
        exec: { agent: "claude" },
        review: { agent: "claude" },
      },
    };
    mocks.preparePlanExecuteBind.mockResolvedValueOnce(prep);
    mocks.bindPlanExecute.mockResolvedValueOnce({
      planner: { id: "s-plan" },
      run: { id: "req-200", state: "blocked", summary: "Agent process unavailable" },
      tasks: [],
      delivery: { delivery: "blocked", error: "Agent process unavailable" },
    });

    openBindRoute("?bindPlanExecute=req-200&bindPlan=s-plan");
    render(<BindPlanExecuteModal />);

    await screen.findByText("launch.splitTasks");
    // Shared worktree is excluded when binding an existing planner.
    expect(screen.queryByText("orch.worktreeShared")).toBeNull();
    expect(screen.getByText("orch.worktreeEach")).toBeTruthy();

    const submitBtn = screen.getByRole("button", { name: "bindPlanExecute.submit" });
    await waitFor(() => expect((submitBtn as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(submitBtn);

    await screen.findByText("Agent process unavailable");
    // Route stays open with the same requestId for safe idempotent retry.
    expect(new URL(window.location.href).searchParams.get("bindPlanExecute")).toBe("req-200");
    expect(mocks.openSession).not.toHaveBeenCalled();
  });

  it("shows reparent option for sibling executors in multi-select and locks review config in extend mode", async () => {
    const prep: PlanExecuteBindPreparation = {
      plannerId: "s-active",
      executorIds: ["s-sibling"],
      candidates: [
        {
          id: "s-active",
          projectId: "p1",
          groupId: null,
          parentSessionId: null,
          name: "Active Plan",
          kind: "claude",
          engine: "chat",
          cwd: "/tmp/p1",
          worktreePath: null,
          eligible: true,
          ineligibleReason: null,
          activeRole: "planner",
          extendablePlanner: true,
          existingRun: {
            id: "run-existing",
            state: "executing",
            round: 1,
            splitTasks: true,
            taskCount: 2,
            remainingSlots: 10,
            reviewEnabled: true,
            reviewStatus: "pending",
            config: {
              reviewEnabled: true,
              worktreeMode: "none",
              splitTasks: true,
              plan: { agent: "claude" },
              exec: { agent: "claude" },
              review: { agent: "claude" },
            },
          },
        },
        {
          id: "s-sibling",
          projectId: "p1",
          groupId: null,
          parentSessionId: null,
          name: "Sibling Exec",
          kind: "codex",
          engine: "chat",
          cwd: "/tmp/p1",
          worktreePath: null,
          eligible: true,
          ineligibleReason: null,
          activeRole: null,
          extendablePlanner: false,
          existingRun: null,
        },
      ],
      defaultConfig: {
        reviewEnabled: true,
        worktreeMode: "none",
        splitTasks: true,
        plan: { agent: "claude" },
        exec: { agent: "claude" },
        review: { agent: "claude" },
      },
    };
    mocks.preparePlanExecuteBind.mockResolvedValueOnce(prep);
    mocks.bindPlanExecute.mockResolvedValueOnce({
      planner: { id: "s-active" },
      run: { id: "run-existing", state: "executing", summary: "" },
      tasks: [],
      delivery: { delivery: "queued" },
    });

    openBindRoute("?bindPlanExecute=req-300&bindExec=s-active,s-sibling");
    render(<BindPlanExecuteModal />);

    await screen.findByText("bindPlanExecute.activeWorkflowNotice:2,10");
    expect(screen.getByText("bindPlanExecute.lockedReviewEnabled")).toBeTruthy();
    expect(screen.getByText("bindPlanExecute.reparentLabel")).toBeTruthy();

    const extendBtn = screen.getByRole("button", { name: "bindPlanExecute.submitExtend" });
    await waitFor(() => expect((extendBtn as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(extendBtn);

    await waitFor(() => {
      expect(mocks.bindPlanExecute).toHaveBeenCalledWith(
        expect.objectContaining({
          requestId: "req-300",
          plannerId: "s-active",
          executorIds: ["s-sibling"],
          reparentExecutors: true,
        }),
      );
    });
  });

  it("supports batch-setting selected sessions as execution sessions and picking a command session", async () => {
    const prep: PlanExecuteBindPreparation = {
      plannerId: "",
      executorIds: ["s-e1", "s-e2"],
      candidates: [
        {
          id: "s-cmd",
          projectId: "p1",
          groupId: null,
          parentSessionId: null,
          name: "Commander",
          kind: "antigravity",
          engine: "chat",
          cwd: "/tmp/p1",
          worktreePath: null,
          eligible: true,
          ineligibleReason: null,
          activeRole: null,
          extendablePlanner: false,
          existingRun: null,
        },
        {
          id: "s-e1",
          projectId: "p1",
          groupId: null,
          parentSessionId: null,
          name: "Exec 1",
          kind: "claude",
          engine: "chat",
          cwd: "/tmp/p1",
          worktreePath: null,
          eligible: true,
          ineligibleReason: null,
          activeRole: null,
          extendablePlanner: false,
          existingRun: null,
        },
        {
          id: "s-e2",
          projectId: "p1",
          groupId: null,
          parentSessionId: null,
          name: "Exec 2",
          kind: "codex",
          engine: "chat",
          cwd: "/tmp/p1",
          worktreePath: null,
          eligible: true,
          ineligibleReason: null,
          activeRole: null,
          extendablePlanner: false,
          existingRun: null,
        },
      ],
      defaultConfig: {
        reviewEnabled: false,
        worktreeMode: "none",
        splitTasks: false,
        plan: { agent: "claude" },
        exec: { agent: "claude" },
        review: { agent: "claude" },
      },
    };
    mocks.preparePlanExecuteBind.mockResolvedValueOnce(prep);
    mocks.bindPlanExecute.mockResolvedValueOnce({
      planner: { id: "s-cmd" },
      run: { id: "req-400", state: "executing", summary: "" },
      tasks: [],
      delivery: { delivery: "sent" },
    });

    openBindRoute("?bindPlanExecute=req-400&bindRole=exec&bindExec=s-e1,s-e2");
    render(<BindPlanExecuteModal />);

    await waitFor(() => {
      expect(mocks.preparePlanExecuteBind).toHaveBeenCalledWith(["s-e1", "s-e2"], "executors");
    });

    // Both selected sessions are prefilled as executors; submit stays disabled until a command session is picked.
    expect(await screen.findByText("bindPlanExecute.execSummary:2")).toBeTruthy();
    const submitBtn = screen.getByRole("button", { name: "bindPlanExecute.submit" }) as HTMLButtonElement;
    expect(submitBtn.disabled).toBe(true);

    // Pick Commander in the Command session Select dropdown.
    const planCombo = screen.getByRole("combobox", { name: "bindPlanExecute.planSessionLabel" });
    fireEvent.click(planCombo);
    const cmdOption = await screen.findByRole("option", { name: /Commander/ });
    fireEvent.click(cmdOption);

    await waitFor(() => expect(submitBtn.disabled).toBe(false));
    fireEvent.click(submitBtn);

    await waitFor(() => {
      expect(mocks.bindPlanExecute).toHaveBeenCalledWith(
        expect.objectContaining({
          requestId: "req-400",
          plannerId: "s-cmd",
          executorIds: ["s-e1", "s-e2"],
          reparentExecutors: true,
        }),
      );
    });
  });
});
