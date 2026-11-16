//! Child-session review preserves launch context and edits across asynchronous capability loading.
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SpawnRequest } from "../ipc/events";
import type { PlanExecuteRolePrefs } from "../store/settings";

const mocks = vi.hoisted(() => ({
  localeSuffix: "",
  launchOptions: vi.fn(),
  launchModels: vi.fn(),
  launchSelection: vi.fn(),
  prepareSpawn: vi.fn(),
  planExecuteDefaults: vi.fn(),
  preparePlanExecute: vi.fn(),
  createPlanExecute: vi.fn(),
  applyLaunchArgs: vi.fn(),
  nativeImage: vi.fn(),
  env: { isTauri: false },
  store: {
    pendingSpawns: [] as SpawnRequest[],
    spawnReceipts: {} as Record<string, import("../ipc/commands").SpawnReceipt>,
    sessions: [] as {
      id: string;
      name: string;
      kind: string;
      agentArgs: string | null;
      cwd?: string;
    }[],
    projects: [],
    agentDefaults: {},
    planExecutePrefs: { plan: {}, exec: {} } as {
      plan: PlanExecuteRolePrefs;
      exec: PlanExecuteRolePrefs;
      review?: PlanExecuteRolePrefs;
    },
    setPlanExecuteRolePrefs: vi.fn(),
    confirmSpawn: vi.fn(),
    cancelSpawn: vi.fn(),
    loadTree: vi.fn(),
    openSession: vi.fn(),
  },
}));
vi.mock("../platform/env", () => ({ env: mocks.env }));
vi.mock("../terminal/imageInput", async (original) => ({
  ...await original<typeof import("../terminal/imageInput")>(),
  imageFromNativeClipboard: mocks.nativeImage,
}));
vi.mock("../i18n", () => ({ useT: () => (key: string) => key + mocks.localeSuffix }));
vi.mock("../hooks/nativeViewSuspend", () => ({
  useSuspendNativeViews: () => {},
}));
vi.mock("../ipc/commands", () => ({
  agentListModels: vi.fn().mockResolvedValue(["opus", "sonnet"]),
  prepareSpawn: mocks.prepareSpawn,
}));
vi.mock("../ipc/launch", () => ({
  launchOptions: mocks.launchOptions,
  launchModels: mocks.launchModels,
  launchSelection: mocks.launchSelection,
  planExecuteDefaults: mocks.planExecuteDefaults,
  preparePlanExecute: mocks.preparePlanExecute,
  createPlanExecute: mocks.createPlanExecute,
  applyLaunchArgs: mocks.applyLaunchArgs,
}));
vi.mock("../store/termStore", () => ({
  useTermStore: Object.assign((selector: (s: typeof mocks.store) => unknown) =>
    selector(mocks.store), { getState: () => mocks.store }),
}));
import { SpawnConfirmModal } from "./SpawnConfirmModal";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.localeSuffix = "";
  mocks.launchModels.mockReset().mockResolvedValue({ models: [
    { id: "opus", label: "opus", effortLevels: ["low", "high"] },
    { id: "sonnet", label: "sonnet", effortLevels: ["low", "high"] },
    { id: "gpt-5.6-sol", label: "gpt-5.6-sol", effortLevels: ["low", "medium", "high", "xhigh", "max", "ultra"] },
  ], effortLevels: ["low", "medium", "high", "xhigh", "max", "ultra"] });
  mocks.env.isTauri = false;
  mocks.nativeImage.mockReset();
  window.history.replaceState(null, "", "/");
  mocks.store.pendingSpawns = [];
  mocks.store.spawnReceipts = {};
  mocks.store.cancelSpawn.mockResolvedValue(undefined);
  mocks.prepareSpawn.mockImplementation(async (requestId: string, kind?: string) => {
    const req = mocks.store.pendingSpawns.find(item => item.requestId === requestId);
    const selected = kind || req?.kind || "claude";
    const values = await mocks.launchSelection(selected, "--model sonnet");
    return { kind: selected, cwd: req?.cwd || "/repo", model: req?.model ?? values.model, effort: req?.effort ?? values.effort };
  });
  mocks.store.loadTree.mockResolvedValue(undefined);
  mocks.launchOptions.mockResolvedValue([
    {
      supportsPlanExecute: true,
      id: "claude",
      label: "Claude",
      acceptsTask: true,
      effortFlag: "--effort",
      effortLevels: ["low", "high"],
    },
    {
      supportsPlanExecute: true,
      id: "codex",
      label: "Codex",
      acceptsTask: true,
      effortFlag: "model_reasoning_effort",
      effortLevels: ["low", "medium", "high", "xhigh", "max", "ultra"],
    },
    {
      supportsPlanExecute: false,
      id: "terminal",
      label: "Terminal",
      acceptsTask: false,
      effortFlag: null,
      effortLevels: [],
    },
  ]);
  mocks.launchSelection.mockResolvedValue({ model: "sonnet", effort: "low" });
  mocks.applyLaunchArgs.mockResolvedValue(null);
  mocks.store.confirmSpawn.mockResolvedValue(undefined);
  mocks.store.agentDefaults = {};
  mocks.store.planExecutePrefs = { plan: {}, exec: {} };
});
afterEach(() => { cleanup(); window.history.replaceState(null, "", "/"); });
async function open(req: Partial<SpawnRequest> = {}, waitForOptions = true) {
  mocks.store.sessions = [
    {
      id: "p1",
      name: "Parent session",
      kind: "claude",
      agentArgs: "--model sonnet",
      cwd: "/repo",
    },
  ];
  mocks.store.pendingSpawns = [
    { requestId: "request-id", parentSessionId: "p1", prompt: "Investigate the parser", ...req },
  ];
  render(<SpawnConfirmModal />);
  if (waitForOptions) await screen.findByLabelText("spawn.modelLabel");
}
function pickModel(field: HTMLElement, value: string) {
  fireEvent.click(field);
  const popup = screen.getByRole("listbox");
  if (value === "") {
    fireEvent.click(within(popup).getAllByRole("option")[0]);
    return;
  }
  fireEvent.change(within(popup).getByRole("textbox"), { target: { value } });
  fireEvent.click(within(popup).getAllByRole("option")[0]);
}
function pickEffort(field: HTMLElement, level: string) {
  fireEvent.click(field);
  const popup = screen.getByRole("listbox");
  fireEvent.click(within(popup).getByRole("option", { name: level ? `chat.effort.${level}` : "chat.effort.auto" }));
}
async function launch() {
  await waitFor(() => expect((screen.getByRole("button", { name: "spawn.launch" }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "spawn.launch" }));
  await waitFor(() => expect(mocks.store.confirmSpawn).toHaveBeenCalledOnce());
  return mocks.store.confirmSpawn.mock.calls[0][0] as SpawnRequest;
}
describe("single child-session review", () => {
  it("preserves task, invocation directory and explicitly requested choices", async () => {
    await open({
      cwd: "/other repo",
      model: "opus",
      effort: "high",
      worktree: false,
    });
    expect(screen.getByText("/other repo")).toBeTruthy();
    expect(screen.getByLabelText("spawn.modelLabel").textContent).toContain("opus");
    expect(await launch()).toMatchObject({
      cwd: "/other repo",
      model: "opus",
      effort: "high",
      worktree: false,
    });
  });
  it("keeps the requested session title when the launch is confirmed", async () => {
    await open({ title: "Parser audit" });
    expect(await launch()).toMatchObject({ title: "Parser audit" });
  });
  it("uses backend-resolved inherited settings and can explicitly clear a model", async () => {
    await open();
    expect(mocks.launchSelection).toHaveBeenCalledWith(
      "claude",
      "--model sonnet",
    );
    await act(async () => { await Promise.resolve(); });
    expect(mocks.prepareSpawn).toHaveBeenCalledTimes(1);
    pickModel(screen.getByLabelText("spawn.modelLabel"), "");
    await act(async () => { await Promise.resolve(); });
    expect(screen.getByLabelText("spawn.modelLabel").textContent).toContain("chat.followModelDefault");
    expect((await launch()).model).toBe("");
  });
  it("supports Codex effort without silently dropping the request", async () => {
    await open({ kind: "codex", model: "gpt-5.5", effort: "xhigh" });
    expect((await launch()).effort).toBe("xhigh");
  });
  it("keeps plain terminal launches available without agent-only controls", async () => {
    await open({ kind: "terminal" }, false);
    await screen.findByText("launch.terminalHint");
    await waitFor(() =>
      expect(
        (
          screen.getByRole("button", {
            name: "spawn.launch",
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(false),
    );
    expect(screen.queryByLabelText("spawn.promptLabel")).toBeNull();
    expect(screen.queryByLabelText("spawn.modelLabel")).toBeNull();
    expect(await launch()).toMatchObject({
      kind: "terminal",
      model: null,
      effort: null,
    });
  });
  it("does not submit an empty task", async () => {
    await open();
    fireEvent.change(screen.getByLabelText("spawn.promptLabel"), {
      target: { value: " \n " },
    });
    expect(
      (
        screen.getByRole("button", {
          name: "spawn.launch",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
  });
  it("keeps the request reviewable when backend validation rejects a choice", async () => {
    await open();
    mocks.applyLaunchArgs.mockRejectedValueOnce(
      new Error("invalid identifier"),
    );
    fireEvent.click(screen.getByRole("button", { name: "spawn.launch" }));
    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      "launch.startError invalid identifier",
    );
    expect(mocks.store.confirmSpawn).not.toHaveBeenCalled();
  });
  it("blocks launch while the capability service is unavailable and allows retry", async () => {
    mocks.launchOptions.mockRejectedValueOnce(new Error("offline"));
    await open({}, false);
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(
      (
        screen.getByRole("button", {
          name: "spawn.launch",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "common.retry" }));
    await screen.findByLabelText("spawn.modelLabel");
    await launch();
  });
});

it.each(["none", "shared", "each"] as const)("keeps both roles editable and submits the %s worktree mode", async mode => {
  const config = {
    plan: { agent: "codex" as const, model: "planner", effort: "high" },
    exec: { agent: "codex" as const, model: "executor", effort: "medium" },
  };
  mocks.planExecuteDefaults.mockResolvedValue(config);
  await open({ planExecute: config }, false);
  const planner = within(await screen.findByRole("region", { name: "launch.planTitle" }));
  const executor = within(screen.getByRole("region", { name: "launch.execTitle" }));
  expect(screen.getAllByRole("radio")).toHaveLength(3);
  const labels = { none: "orch.worktreeNone", shared: "orch.worktreeShared", each: "orch.worktreeEach" };
  expect((screen.getByRole("radio", { name: /orch.worktreeShared/ }) as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByRole("radio", { name: new RegExp(labels[mode]) }));
  expect(planner.getByLabelText("spawn.modelLabel").textContent).toContain("planner");
  expect(executor.getByLabelText("spawn.effortLabel").textContent).toContain("chat.effort.medium");
  pickModel(planner.getByLabelText("spawn.modelLabel"), "final-planner");
  pickEffort(planner.getByLabelText("spawn.effortLabel"), "xhigh");
  pickModel(executor.getByLabelText("spawn.modelLabel"), "final-executor");
  pickEffort(executor.getByLabelText("spawn.effortLabel"), "low");
  const submitted = await launch();
  expect(submitted.worktree).toBe(mode !== "none");
  expect(submitted.planExecute).toEqual({
    worktreeMode: mode,
    plan: { agent: "codex", model: "final-planner", effort: "xhigh" },
    exec: { agent: "codex", model: "final-executor", effort: "low" },
  });
});

it("does not launch a workflow using invented settings when defaults cannot be loaded", async () => {
  mocks.planExecuteDefaults.mockRejectedValue(new Error("offline"));
  await open({ planExecute: { plan: {}, exec: {} } }, false);
  await screen.findByRole("alert");
  expect((screen.getByRole("button", { name: "spawn.launch" }) as HTMLButtonElement).disabled).toBe(true);
  expect(mocks.store.confirmSpawn).not.toHaveBeenCalled();
});

it("ignores obsolete terminal role preferences and lists only workflow-capable agents", async () => {
  mocks.store.planExecutePrefs = { plan: { agent: "terminal", model: "obsolete", effort: "obsolete" }, exec: {} };
  const config = { plan: { agent: "codex" as const, model: "planner" }, exec: { agent: "claude" as const } };
  mocks.planExecuteDefaults.mockResolvedValue(config);
  await open({ planExecute: { plan: {}, exec: {} } }, false);
  const planner = within(await screen.findByRole("region", { name: "launch.planTitle" }));
  fireEvent.click(planner.getByLabelText("spawn.agentLabel"));
  expect(screen.getAllByRole("option").map(option => option.textContent)).toEqual(["Claude", "Codex"]);
  fireEvent.keyDown(planner.getByLabelText("spawn.agentLabel"), { key: "Escape" });
  expect((await launch()).planExecute?.plan).toEqual(config.plan);
});

it("disables workflow launch for a stale unsupported role until the user selects an available agent", async () => {
  const config = { plan: { agent: "terminal" as const }, exec: { agent: "codex" as const } };
  mocks.planExecuteDefaults.mockResolvedValue(config);
  await open({ planExecute: config }, false);
  const planner = within(await screen.findByRole("region", { name: "launch.planTitle" }));
  expect((screen.getByRole("button", { name: "spawn.launch" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(planner.getByLabelText("spawn.agentLabel"));
  fireEvent.click(screen.getByRole("option", { name: "Claude" }));
  expect((await launch()).planExecute?.plan.agent).toBe("claude");
});

it("resets only the selected role when its agent changes", async () => {
  const config = { plan: { agent: "codex" as const, model: "planner", effort: "high" }, exec: { agent: "codex" as const, model: "executor", effort: "medium" } };
  mocks.planExecuteDefaults.mockResolvedValue(config);
  await open({ planExecute: config }, false);
  const planner = within(await screen.findByRole("region", { name: "launch.planTitle" }));
  fireEvent.click(planner.getByLabelText("spawn.agentLabel"));
  fireEvent.click(await screen.findByRole("option", { name: "Claude" }));
  expect((await launch()).planExecute).toEqual({
    worktreeMode: "shared",
    plan: { agent: "claude", model: "", effort: "" },
    exec: config.exec,
  });
  expect(mocks.store.setPlanExecuteRolePrefs).toHaveBeenCalledWith("plan", { agent: "claude", model: null, effort: null });
});

it("opens the workflow on the choices remembered from the previous one", async () => {
  mocks.store.planExecutePrefs = {
    plan: { agent: "claude", model: "opus", effort: "high" },
    exec: {},
  };
  mocks.planExecuteDefaults.mockResolvedValue({
    plan: { agent: "codex", model: "planner", effort: "medium" },
    exec: { agent: "codex", model: "executor", effort: "medium" },
  });
  await open({ planExecute: { plan: {}, exec: {} } }, false);
  const planner = within(await screen.findByRole("region", { name: "launch.planTitle" }));
  const executor = within(screen.getByRole("region", { name: "launch.execTitle" }));
  expect(planner.getByLabelText("spawn.agentLabel").textContent).toContain("Claude");
  expect(planner.getByLabelText("spawn.modelLabel").textContent).toContain("opus");
  expect(planner.getByLabelText("spawn.effortLabel").textContent).toContain("chat.effort.high");
  expect(executor.getByLabelText("spawn.agentLabel").textContent).toContain("Codex");
  expect(executor.getByLabelText("spawn.modelLabel").textContent).toContain("executor");
});

it("lets an explicit request value win over the remembered choice", async () => {
  mocks.store.planExecutePrefs = { plan: { agent: "claude", model: "opus", effort: "high" }, exec: {} };
  const config = { plan: { agent: "codex" as const, model: "requested", effort: "low" }, exec: { agent: "codex" as const, model: "executor", effort: "medium" } };
  mocks.planExecuteDefaults.mockResolvedValue(config);
  await open({ planExecute: config }, false);
  const planner = within(await screen.findByRole("region", { name: "launch.planTitle" }));
  expect(planner.getByLabelText("spawn.agentLabel").textContent).toContain("Codex");
  expect(planner.getByLabelText("spawn.modelLabel").textContent).toContain("requested");
  expect(planner.getByLabelText("spawn.effortLabel").textContent).toContain("chat.effort.low");
});


describe("new planning workflow from the session menu", () => {
  const context = { projectId: "p", groupId: "g", parentSessionId: "parent" };
  const config = { plan: { agent: "codex", model: "planner", effort: "high" }, exec: { agent: "codex", model: "executor", effort: "medium" } };
  const route = "/?planExecute=creation-id&planProject=p&planGroup=g&planParent=parent";
  beforeEach(() => {
    window.history.replaceState(null, "", route);
    mocks.preparePlanExecute.mockResolvedValue({ config, cwd: "/repo", worktree: false, locationNames: ["Project", "Group", "Parent"] });
    mocks.createPlanExecute.mockResolvedValue({ planner: { id: "new-planner" }, run: { state: "planning" } });
  });
  it.each(["none", "shared", "each"] as const)("loads backend defaults and creates both roles with %s worktree mode", async mode => {
    render(<SpawnConfirmModal />);
    const planner = within(await screen.findByRole("region", { name: "launch.planTitle" }));
    const executor = within(screen.getByRole("region", { name: "launch.execTitle" }));
    expect(mocks.preparePlanExecute).toHaveBeenCalledWith(context);
    expect(mocks.planExecuteDefaults).not.toHaveBeenCalled();
    expect(mocks.createPlanExecute).not.toHaveBeenCalled();
    expect(screen.getByText("Project / Group / Parent")).toBeTruthy();
    expect((screen.getByRole("button", { name: "launch.createAndStart" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("spawn.promptLabel"), { target: { value: "Implement the requested feature" } });
    fireEvent.change(screen.getByLabelText("launch.workingDirectory"), { target: { value: "/repo/selected" } });
    pickModel(planner.getByLabelText("spawn.modelLabel"), "review-model");
    pickEffort(executor.getByLabelText("spawn.effortLabel"), "low");
    expect(screen.getAllByRole("radio")).toHaveLength(3);
    const labels = { none: "orch.worktreeNone", shared: "orch.worktreeShared", each: "orch.worktreeEach" };
    expect((screen.getByRole("radio", { name: /orch.worktreeNone/ }) as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByRole("radio", { name: new RegExp(labels[mode]) }));
    fireEvent.click(screen.getByRole("button", { name: "launch.createAndStart" }));
    await waitFor(() => expect(mocks.store.openSession).toHaveBeenCalledWith("new-planner"));
    expect(mocks.createPlanExecute).toHaveBeenCalledWith({ requestId: "creation-id", context, prompt: "Implement the requested feature", cwd: "/repo/selected", worktree: mode !== "none",
      config: { worktreeMode: mode, plan: { ...config.plan, model: "review-model" }, exec: { ...config.exec, effort: "low" } } });
    expect(window.location.search).toBe("");
    expect(mocks.store.confirmSpawn).not.toHaveBeenCalled();
  });
  it("cancels without touching queued requests and responds to browser navigation", async () => {
    render(<SpawnConfirmModal />);
    await screen.findByRole("region", { name: "launch.planTitle" });
    fireEvent.change(screen.getByLabelText("spawn.promptLabel"), { target: { value: "Keep my draft" } });
    act(() => { window.history.replaceState(null, "", route + "&session=another"); window.dispatchEvent(new PopStateEvent("popstate")); });
    expect((screen.getByLabelText("spawn.promptLabel") as HTMLTextAreaElement).value).toBe("Keep my draft");
    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(mocks.store.cancelSpawn).not.toHaveBeenCalled();
    expect(mocks.createPlanExecute).not.toHaveBeenCalled();
    act(() => { window.history.replaceState(null, "", route); window.dispatchEvent(new PopStateEvent("popstate")); });
    await screen.findByRole("region", { name: "launch.planTitle" });
    expect(screen.getByRole("dialog")).toBeTruthy();
  });
  it("retains the same request and editable prompt after a failed launch", async () => {
    mocks.createPlanExecute.mockResolvedValueOnce({ planner: { id: "retained-planner" }, run: { state: "blocked", summary: "Agent unavailable" } });
    render(<SpawnConfirmModal />);
    await screen.findByRole("region", { name: "launch.planTitle" });
    fireEvent.change(screen.getByLabelText("spawn.promptLabel"), { target: { value: "Task" } });
    pasteImage(screen.getByLabelText("spawn.promptLabel"));
    await screen.findByAltText("screenshot.png");
    fireEvent.click(screen.getByRole("button", { name: "launch.createAndStart" }));
    expect((await screen.findByRole("alert")).textContent).toContain("Agent unavailable");
    expect(screen.getByAltText("screenshot.png")).toBeTruthy();
    expect(new URLSearchParams(window.location.search).get("planExecute")).toBe("creation-id");
    fireEvent.click(screen.getByRole("button", { name: "launch.createAndStart" }));
    await waitFor(() => expect(mocks.store.openSession).toHaveBeenCalledWith("new-planner"));
    expect(mocks.createPlanExecute.mock.calls[0]).toEqual(mocks.createPlanExecute.mock.calls[1]);
  });
  it("blocks creation when placement cannot be resolved", async () => {
    mocks.preparePlanExecute.mockRejectedValueOnce(new Error("Project removed"));
    render(<SpawnConfirmModal />);
    await screen.findByRole("alert");
    fireEvent.change(screen.getByLabelText("spawn.promptLabel"), { target: { value: "Task" } });
    expect((screen.getByRole("button", { name: "launch.createAndStart" }) as HTMLButtonElement).disabled).toBe(true);
    expect(mocks.createPlanExecute).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "common.retry" }));
    await screen.findByRole("region", { name: "launch.planTitle" });
    expect((screen.getByRole("button", { name: "launch.createAndStart" }) as HTMLButtonElement).disabled).toBe(false);
  });
});


function pasteImage(input: HTMLElement, name = "screenshot.png") {
  const file = new File([new Uint8Array([1, 2, 3])], name, { type: "image/png" });
  fireEvent.paste(input, { clipboardData: { items: [{ kind: "file", type: file.type, getAsFile: () => file }], getData: () => "" } });
}

it("renders invalid caller model values for correction and gives the executor's validation error", async () => {
  const config = { plan: { agent: "codex" as const, model: "6-astrak", effort: "xhigh" }, exec: { agent: "codex" as const, model: "gpt 5.6 sol", effort: "xhigh" } };
  mocks.planExecuteDefaults.mockResolvedValue(config);
  mocks.applyLaunchArgs.mockImplementation(async (_kind, _args, model) => {
    if (model === "gpt 5.6 sol") throw new Error("Model and effort must be identifiers without spaces or shell operators");
    return null;
  });
  await open({ planExecute: config }, false);
  const executor = within(await screen.findByRole("region", { name: "launch.execTitle" }));
  expect(executor.getByLabelText("spawn.modelLabel").textContent).toContain("gpt 5.6 sol");
  pasteImage(screen.getByLabelText("spawn.promptLabel"));
  await screen.findByAltText("screenshot.png");
  fireEvent.click(screen.getByRole("button", { name: "spawn.launch" }));
  expect((await screen.findByRole("alert")).textContent).toContain("launch.execTitle: Model and effort must be identifiers");
  expect(mocks.store.confirmSpawn).not.toHaveBeenCalled();
  expect(screen.getByAltText("screenshot.png")).toBeTruthy();
  pickModel(executor.getByLabelText("spawn.modelLabel"), "gpt-5.6-sol");
  expect((await launch()).images).toEqual([{ mimeType: "image/png", data: "AQID" }]);
});

it("keeps loading failures specific and preserves the prompt while retrying", async () => {
  mocks.planExecuteDefaults.mockRejectedValueOnce(new Error("The workflow session is missing or archived"));
  mocks.planExecuteDefaults.mockResolvedValue({ plan: { agent: "codex" }, exec: { agent: "codex" } });
  await open({ planExecute: { plan: {}, exec: {} } }, false);
  expect((await screen.findByRole("alert")).textContent).toContain("The workflow session is missing or archived");
  fireEvent.change(screen.getByLabelText("spawn.promptLabel"), { target: { value: "Keep this edited task" } });
  pasteImage(screen.getByLabelText("spawn.promptLabel"));
  await screen.findByAltText("screenshot.png");
  fireEvent.click(screen.getByRole("button", { name: "common.retry" }));
  await screen.findByRole("region", { name: "launch.execTitle" });
  expect((await launch()).prompt).toBe("Keep this edited task");
});

it("serializes consecutive image pastes, supports removal, and carries images into ordinary spawn requests", async () => {
  await open();
  const input = screen.getByLabelText("spawn.promptLabel");
  pasteImage(input, "first.png"); pasteImage(input, "second.png");
  await screen.findByAltText("second.png");
  expect(screen.getByAltText("first.png")).toBeTruthy();
  fireEvent.click(screen.getAllByRole("button", { name: "chat.attach.remove" })[0]);
  expect(screen.queryByAltText("first.png")).toBeNull();
  expect((await launch()).images).toEqual([{ mimeType: "image/png", data: "AQID" }]);
});

it("uses the native image clipboard for an empty WebView paste without intercepting plain text", async () => {
  mocks.env.isTauri = true;
  mocks.nativeImage.mockResolvedValue(new File([new Uint8Array([4, 5, 6])], "native.png", { type: "image/png" }));
  await open();
  const input = screen.getByLabelText("spawn.promptLabel");
  fireEvent.paste(input, { clipboardData: { items: [], getData: () => "plain text" } });
  expect(mocks.nativeImage).not.toHaveBeenCalled();
  fireEvent.paste(input, { clipboardData: { items: [], getData: () => "" } });
  await screen.findByAltText("native.png");
  expect((await launch()).images).toEqual([{ mimeType: "image/png", data: "BAUG" }]);
});

it("does not attach an image whose read finishes after the request was cancelled", async () => {
  mocks.env.isTauri = true;
  let finish!: (file: File) => void;
  mocks.nativeImage.mockReturnValue(new Promise<File>(resolve => { finish = resolve; }));
  await open();
  fireEvent.paste(screen.getByLabelText("spawn.promptLabel"), { clipboardData: { items: [], getData: () => "" } });
  await waitFor(() => expect(mocks.nativeImage).toHaveBeenCalled());
  cleanup();
  await act(async () => { finish(new File(["image"], "cancelled.png", { type: "image/png" })); });
  window.history.replaceState(null, "", "/");
  await open({ requestId: "next-request" });
  expect(screen.queryByAltText("cancelled.png")).toBeNull();
});


it("keeps the error visible when a failed workflow is returned to the queue", async () => {
  const config = { plan: { agent: "codex" as const }, exec: { agent: "codex" as const } };
  const req = { requestId: "retry-visible", parentSessionId: "p1", prompt: "Task", planExecute: config };
  mocks.planExecuteDefaults.mockResolvedValue(config);
  mocks.store.pendingSpawns = [req];
  const view = render(<SpawnConfirmModal />);
  await screen.findByRole("region", { name: "launch.execTitle" });
  pasteImage(screen.getByLabelText("spawn.promptLabel"));
  await screen.findByAltText("screenshot.png");
  mocks.store.confirmSpawn.mockImplementationOnce(async (submitted) => {
    mocks.store.pendingSpawns = [];
    view.rerender(<SpawnConfirmModal />);
    await new Promise(resolve => setTimeout(resolve, 0));
    mocks.store.pendingSpawns = [submitted];
    view.rerender(<SpawnConfirmModal />);
    throw new Error("Agent executable not found");
  });
  fireEvent.click(screen.getByRole("button", { name: "spawn.launch" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Agent executable not found");
  await screen.findByAltText("image-1.png");
});


it.each(["spawn", "menu"])("selects independent Codex effort levels from both %s workflow dropdowns", async entry => {
  const config = {
    plan: { agent: "codex" as const, model: "", effort: "" },
    exec: { agent: "codex" as const, model: "gpt-5.6-sol", effort: "" },
  };
  if (entry === "menu") {
    window.history.replaceState(null, "", "/?planExecute=effort-choice&planProject=p");
    mocks.preparePlanExecute.mockResolvedValue({ config, cwd: "/repo", worktree: false, locationNames: ["Project"] });
    mocks.createPlanExecute.mockResolvedValue({ planner: { id: "planner" }, run: { state: "planning" } });
    render(<SpawnConfirmModal />);
    await screen.findByRole("region", { name: "launch.planTitle" });
    fireEvent.change(screen.getByLabelText("spawn.promptLabel"), { target: { value: "Task" } });
  } else {
    mocks.planExecuteDefaults.mockResolvedValue(config);
    await open({ planExecute: config }, false);
    await screen.findByRole("region", { name: "launch.planTitle" });
  }
  const planner = within(screen.getByRole("region", { name: "launch.planTitle" }));
  const executor = within(screen.getByRole("region", { name: "launch.execTitle" }));
  const choose = async (field: HTMLElement, level: string) => {
    fireEvent.click(field);
    await screen.findByRole("option", { name: "chat.effort.ultra" });
    expect(screen.getAllByRole("option").map(option => option.textContent)).toEqual([
      "chat.effort.auto", "chat.effort.low", "chat.effort.medium", "chat.effort.high", "chat.effort.xhigh", "chat.effort.max", "chat.effort.ultra",
    ]);
    fireEvent.click(screen.getByRole("option", { name: `chat.effort.${level}` }));
    expect(field.textContent).toContain(`chat.effort.${level}`);
  };
  await choose(planner.getByLabelText("spawn.effortLabel"), "xhigh");
  await choose(executor.getByLabelText("spawn.effortLabel"), "high");
  expect(planner.getByLabelText("spawn.modelLabel").textContent).toContain("chat.followModelDefault");
  expect(executor.getByLabelText("spawn.modelLabel").textContent).toContain("gpt-5.6-sol");
  expect(mocks.store.setPlanExecuteRolePrefs).toHaveBeenCalledWith("plan", { effort: "xhigh" });
  expect(mocks.store.setPlanExecuteRolePrefs).toHaveBeenCalledWith("exec", { effort: "high" });
  // Returning one role to its native default must leave the other role's explicit choice intact.
  fireEvent.click(planner.getByLabelText("spawn.effortLabel"));
  fireEvent.click(screen.getByRole("option", { name: "chat.effort.auto" }));
  expect(mocks.store.setPlanExecuteRolePrefs).toHaveBeenCalledWith("plan", { effort: "" });
  const expected = { worktreeMode: entry === "menu" ? "none" : "shared", plan: config.plan, exec: { ...config.exec, effort: "high" } };
  if (entry === "menu") {
    fireEvent.click(screen.getByRole("button", { name: "launch.createAndStart" }));
    await waitFor(() => expect(mocks.createPlanExecute).toHaveBeenCalledWith(expect.objectContaining({ config: expected })));
  } else {
    expect((await launch()).planExecute).toEqual(expected);
  }
});

it("carries automatic splitting from the command request and lets the user turn it off", async () => {
  const config = { splitTasks: true, plan: { agent: "claude" as const }, exec: { agent: "codex" as const } };
  mocks.planExecuteDefaults.mockResolvedValue(config);
  await open({ planExecute: config }, false);
  const toggle = await screen.findByRole("checkbox", { name: /launch.splitTasks/ });
  expect((toggle as HTMLInputElement).checked).toBe(true);
  fireEvent.click(toggle);
  expect((await launch()).planExecute?.splitTasks).toBe(false);
});

it("offers automatic task splitting in a new planning and execution session", async () => {
  window.history.replaceState(null, "", "/?planExecute=create-split&planProject=project");
  mocks.preparePlanExecute.mockResolvedValue({ config: { plan: { agent: "claude" }, exec: { agent: "codex" } }, cwd: "/repo", worktree: false, locationNames: ["Project"] });
  mocks.createPlanExecute.mockResolvedValue({ planner: { id: "planner" }, run: { id: "flow", state: "planning", summary: "" } });
  render(<SpawnConfirmModal />);
  fireEvent.change(await screen.findByLabelText("spawn.promptLabel"), { target: { value: "Split this task" } });
  fireEvent.click(await screen.findByRole("checkbox", { name: /launch.splitTasks/ }));
  fireEvent.click(screen.getByRole("button", { name: "launch.createAndStart" }));
  await waitFor(() => expect(mocks.createPlanExecute).toHaveBeenCalledOnce());
  expect(mocks.createPlanExecute.mock.calls[0][0].config.splitTasks).toBe(true);
});


it("selects the stable request URL even when another review is first in the queue", async () => {
  const second = { requestId: "selected-request", parentSessionId: "p1", prompt: "Selected task" };
  mocks.store.pendingSpawns = [{ requestId: "first", parentSessionId: "p1", prompt: "First task" }, second];
  window.history.replaceState(null, "", "/?spawnRequest=selected-request");
  render(<SpawnConfirmModal />);
  await screen.findByLabelText("spawn.modelLabel");
  expect((screen.getByLabelText("spawn.promptLabel") as HTMLTextAreaElement).value).toBe("Selected task");
  expect((await launch()).requestId).toBe("selected-request");
});

it("keeps a rejected cancellation visible and presents its error", async () => {
  mocks.store.cancelSpawn.mockRejectedValue(new Error("connection lost"));
  await open(); fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));
  expect(await screen.findByText("connection lost")).toBeTruthy();
  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(mocks.store.cancelSpawn).toHaveBeenCalledWith("request-id");
});


it.each(["failed", "uncertain", "dispatching"].flatMap(state => [false, true].map(mobile => ({ state, mobile }))))("keeps confirmed $state launches immutable and opens once (mobile=$mobile)", async ({ state, mobile }) => {
  if (mobile) window.history.replaceState(null, "", "/?view=mobile");
  const request = { requestId: "request-id", parentSessionId: "p1", prompt: "Investigate the parser" };
  mocks.store.spawnReceipts["request-id"] = { requestId: "request-id", request, decision: "confirmed", state,
    sessionId: "created-child", messageId: "msg-request", session: { id: "created-child" }, error: "delivery error" } as import("../ipc/commands").SpawnReceipt;
  await open();
  expect(screen.getByText("spawn.confirmedChoices")).toBeTruthy();
  if (state !== "failed") {
    expect(screen.getByRole("alert").textContent).toBe("spawn.deliveryUncertain");
    expect(screen.queryByText("delivery error")).toBeNull();
  }
  expect((screen.getByLabelText("spawn.promptLabel") as HTMLTextAreaElement).disabled).toBe(true);
  expect((screen.getByLabelText("spawn.modelLabel") as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByLabelText("spawn.agentLabel") as HTMLButtonElement).disabled).toBe(true);
  const retry = screen.getByRole("button", { name: "common.retry" });
  expect((retry as HTMLButtonElement).disabled).toBe(state !== "failed");
  const historyLength = window.history.length;
  fireEvent.click(screen.getByRole("button", { name: "common.open" }));
  expect(window.history.length).toBe(historyLength + 1);
  expect(new URL(window.location.href).searchParams.get("session")).toBe(mobile ? "created-child" : null);
  expect(mocks.store.openSession).toHaveBeenCalledWith("created-child");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(new URL(window.location.href).searchParams.has("spawnRequest")).toBe(false);
  act(() => {
    window.history.replaceState(null, "", "/?spawnRequest=request-id");
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  expect(screen.getByRole("dialog")).toBeTruthy();
  if (state === "failed") {
    fireEvent.click(screen.getByRole("button", { name: "common.retry" }));
    await waitFor(() => expect(mocks.store.confirmSpawn).toHaveBeenCalledWith(request));
  } else { expect(mocks.store.confirmSpawn).not.toHaveBeenCalled(); }
});


it.each(["failed", "uncertain"] as const)("always closes a confirmed %s launch, cancelling only a failed one", async state => {
  const request = { requestId: "request-id", parentSessionId: "p1", prompt: "Investigate the parser", model: "opus 5.5" };
  mocks.store.spawnReceipts["request-id"] = confirmedReceipt(request, state) as import("../ipc/commands").SpawnReceipt;
  mocks.prepareSpawn.mockRejectedValue(new Error("Model and effort must be identifiers without spaces or shell operators"));
  await open(request, false);
  expect((await screen.findAllByText("Model and effort must be identifiers without spaces or shell operators")).length).toBeGreaterThan(0);
  expect((screen.getByRole("button", { name: "common.cancel" }) as HTMLButtonElement).disabled).toBe(state !== "failed");
  fireEvent.click(screen.getByRole("button", { name: "common.close" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(new URL(window.location.href).searchParams.has("spawnRequest")).toBe(false);
  if (state === "failed") expect(mocks.store.cancelSpawn).toHaveBeenCalledWith("request-id");
  else expect(mocks.store.cancelSpawn).not.toHaveBeenCalled();
});

it("closes a pending request immediately even when its cancellation is rejected", async () => {
  mocks.store.cancelSpawn.mockRejectedValue(new Error("connection lost"));
  await open();
  fireEvent.click(screen.getByRole("button", { name: "common.close" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(mocks.store.cancelSpawn).toHaveBeenCalledWith("request-id");
});

it("keeps a missing-identity request disabled and translates its error on locale changes", async () => {
  mocks.store.pendingSpawns = [{ parentSessionId: "p1", kind: "claude", prompt: "Legacy request" }];
  const view = render(<SpawnConfirmModal />);
  expect(await screen.findByText("spawn.requestUnavailable")).toBeTruthy();
  expect((screen.getByRole("button", { name: "spawn.launch" }) as HTMLButtonElement).disabled).toBe(true);
  expect(mocks.prepareSpawn).not.toHaveBeenCalled();
  mocks.localeSuffix = " (changed locale)";
  view.rerender(<SpawnConfirmModal />);
  expect(screen.getByText("spawn.requestUnavailable (changed locale)")).toBeTruthy();
  expect(screen.queryByText("spawn.requestUnavailable")).toBeNull();
  expect(mocks.store.confirmSpawn).not.toHaveBeenCalled();
});


function confirmedReceipt(request: SpawnRequest, state: import("../ipc/commands").SpawnReceipt["state"], resolvedPlanExecute?: SpawnRequest["planExecute"]) {
  return { requestId: request.requestId!, request, decision: "confirmed" as const, state,
    sessionId: "created-child", messageId: "msg-request", session: null, error: "controlled rejection", resolvedPlanExecute };
}

it("preserves pending text, attachment removal and model edits across copied receipts, then displays the approved snapshot", async () => {
  const first: SpawnRequest = { requestId: "request-id", parentSessionId: "p1", prompt: "Original task", worktree: false,
    images: [{ mimeType: "image/png", data: "AQID" }] };
  mocks.store.pendingSpawns = [first];
  const view = render(<SpawnConfirmModal />);
  await screen.findByLabelText("spawn.modelLabel");
  fireEvent.change(screen.getByLabelText("spawn.promptLabel"), { target: { value: "Local unsubmitted task" } });
  pickModel(screen.getByLabelText("spawn.modelLabel"), "local-model");
  fireEvent.click(screen.getByTitle("chat.attach.remove"));
  const calls = mocks.prepareSpawn.mock.calls.length;
  mocks.store.pendingSpawns = [{ ...first, images: [...first.images!] }];
  view.rerender(<SpawnConfirmModal />);
  expect((screen.getByLabelText("spawn.promptLabel") as HTMLTextAreaElement).value).toBe("Local unsubmitted task");
  expect(screen.getByLabelText("spawn.modelLabel").textContent).toContain("local-model");
  expect(screen.queryByAltText("image-1.png")).toBeNull();
  expect(mocks.prepareSpawn).toHaveBeenCalledTimes(calls);
  const approved = { ...first, prompt: "Task confirmed by the other client", worktree: true,
    kind: "codex" as const, model: "approved-model", effort: "xhigh", images: [{ mimeType: "image/png", data: "BAUG" }] };
  mocks.prepareSpawn.mockResolvedValue({ kind: "codex", model: "approved-model", effort: "xhigh", cwd: "/approved/worktree" });
  // The receipt is authoritative even if the queue still contains its previous copy.
  mocks.store.spawnReceipts["request-id"] = confirmedReceipt(approved, "failed");
  view.rerender(<SpawnConfirmModal />);
  await waitFor(() => expect(screen.getByLabelText("spawn.modelLabel").textContent).toContain("approved-model"));
  expect((screen.getByLabelText("spawn.promptLabel") as HTMLTextAreaElement).value).toBe(approved.prompt);
  expect((screen.getByRole("radio", { name: /orch.worktreeNone/ }) as HTMLInputElement).checked).toBe(false);
  expect(screen.getByAltText("image-1.png").getAttribute("src")).toBe("data:image/png;base64,BAUG");
  expect(screen.getByText("/approved/worktree")).toBeTruthy();
  expect((screen.getByTitle("chat.attach.remove") as HTMLButtonElement).disabled).toBe(true);
});

it.each(["pending", "failed", "uncertain", "dispatching"] as const)("locks both workflow roles and splitTasks to the persisted %s snapshot", async state => {
  const approved: SpawnRequest = { requestId: "request-id", parentSessionId: "p1", prompt: "Approved task", worktree: true,
    planExecute: { plan: {}, exec: {}, splitTasks: true, worktreeMode: "each" } };
  const resolved = { ...approved.planExecute!, plan: { agent: "codex" as const, model: "saved-plan", effort: "high" },
    exec: { agent: "claude" as const, model: "saved-exec", effort: "low" } };
  mocks.store.planExecutePrefs = { plan: { agent: "claude", model: "new-memory", effort: "low" }, exec: { agent: "codex", model: "new-memory" } };
  mocks.store.pendingSpawns = [approved];
  mocks.store.spawnReceipts["request-id"] = confirmedReceipt(approved, state, resolved);
  render(<SpawnConfirmModal />);
  await screen.findByRole("region", { name: "launch.legacyPlanTitle" });
  for (const [role, label] of [["plan", "launch.legacyPlanTitle"], ["exec", "launch.execTitle"]] as const) {
    const section = within(screen.getByRole("region", { name: label }));
    for (const name of ["spawn.agentLabel", "spawn.modelLabel", "spawn.effortLabel"]) {
      expect((section.getByLabelText(name) as HTMLButtonElement).disabled).toBe(true);
    }
    expect(section.getByLabelText("spawn.modelLabel").textContent).toContain(resolved[role].model);
    expect(section.getByLabelText("spawn.effortLabel").textContent).toContain(`chat.effort.${resolved[role].effort}`);
    fireEvent.click(section.getByLabelText("spawn.modelLabel"));
    expect(screen.queryByRole("listbox")).toBeNull();
  }
  const split = screen.getByRole("checkbox") as HTMLInputElement;
  expect(split.checked).toBe(true); expect(split.disabled).toBe(true);
  expect(mocks.planExecuteDefaults).not.toHaveBeenCalled();
  expect(mocks.store.setPlanExecuteRolePrefs).not.toHaveBeenCalled();
  if (state === "failed") {
    fireEvent.click(screen.getByRole("button", { name: "common.retry" }));
    await waitFor(() => expect(mocks.store.confirmSpawn).toHaveBeenCalledWith(approved));
  } else {
    expect((screen.getByRole("button", { name: "common.retry" }) as HTMLButtonElement).disabled).toBe(true);
  }
});

it("keeps pending workflow edits, closes an open role portal on confirmation and never remembers its stale choice", async () => {
  const config = { plan: { agent: "codex" as const, model: "initial-plan" }, exec: { agent: "claude" as const, model: "initial-exec" } };
  mocks.planExecuteDefaults.mockResolvedValue(config);
  const request: SpawnRequest = { requestId: "request-id", parentSessionId: "p1", prompt: "Draft task", planExecute: config };
  mocks.store.pendingSpawns = [request];
  const view = render(<SpawnConfirmModal />);
  const planner = within(await screen.findByRole("region", { name: "launch.planTitle" }));
  pickModel(planner.getByLabelText("spawn.modelLabel"), "local-plan");
  fireEvent.click(screen.getByRole("checkbox"));
  mocks.store.pendingSpawns = [{ ...request, planExecute: { ...config } }];
  view.rerender(<SpawnConfirmModal />);
  expect(planner.getByLabelText("spawn.modelLabel").textContent).toContain("local-plan");
  expect((screen.getByRole("checkbox") as HTMLInputElement).checked).toBe(true);
  expect(mocks.planExecuteDefaults).toHaveBeenCalledOnce();
  fireEvent.click(planner.getByLabelText("spawn.agentLabel"));
  const staleOption = screen.getByRole("option", { name: "Claude" });
  const approved = { ...request, planExecute: { ...config, splitTasks: false } };
  mocks.store.spawnReceipts["request-id"] = confirmedReceipt(approved, "failed", approved.planExecute);
  view.rerender(<SpawnConfirmModal />);
  expect(screen.queryByRole("listbox")).toBeNull();
  mocks.store.setPlanExecuteRolePrefs.mockClear();
  fireEvent.click(staleOption);
  expect(mocks.store.setPlanExecuteRolePrefs).not.toHaveBeenCalled();
  expect((screen.getByRole("checkbox") as HTMLInputElement).checked).toBe(false);
  expect(within(screen.getByRole("region", { name: "launch.legacyPlanTitle" })).getByLabelText("spawn.modelLabel").textContent).toContain("initial-plan");
});

it("discards late workflow defaults and accepts only a later persisted snapshot after another client confirms", async () => {
  let defaults!: (value: unknown) => void;
  mocks.planExecuteDefaults.mockReturnValue(new Promise(resolve => { defaults = resolve; }));
  const request: SpawnRequest = { requestId: "request-id", parentSessionId: "p1", prompt: "Draft task", planExecute: { plan: {}, exec: {} } };
  mocks.store.pendingSpawns = [request];
  const view = render(<SpawnConfirmModal />);
  await waitFor(() => expect(mocks.planExecuteDefaults).toHaveBeenCalledOnce());
  const config = { plan: { agent: "codex" as const, model: "frozen-plan", effort: "xhigh" }, exec: { agent: "claude" as const, model: "frozen-exec", effort: "high" } };
  mocks.store.spawnReceipts["request-id"] = confirmedReceipt(request, "failed", config);
  view.rerender(<SpawnConfirmModal />);
  await screen.findByRole("region", { name: "launch.legacyPlanTitle" });
  await act(async () => defaults({ plan: { agent: "claude", model: "late-default" }, exec: { agent: "codex", model: "late-exec" } }));
  expect(within(screen.getByRole("region", { name: "launch.legacyPlanTitle" })).getByLabelText("spawn.modelLabel").textContent).toContain("frozen-plan");
  expect(mocks.store.setPlanExecuteRolePrefs).not.toHaveBeenCalled();
  // A later resolved receipt replaces the explicit, incomplete confirmed request without reading memory.
  mocks.store.spawnReceipts["request-id"] = confirmedReceipt(request, "dispatching", { ...config, worktreeMode: "each", exec: { ...config.exec, model: "persisted-exec" } });
  view.rerender(<SpawnConfirmModal />);
  await waitFor(() => expect(within(screen.getByRole("region", { name: "launch.execTitle" })).getByLabelText("spawn.modelLabel").textContent).toContain("persisted-exec"));
  expect((screen.getAllByRole("radio")[2] as HTMLInputElement).checked).toBe(true);
});


describe("optional independent review", () => {
  const config = {
    reviewEnabled: true,
    plan: { agent: "codex" as const, model: "planner", effort: "xhigh" },
    exec: { agent: "claude" as const, model: "executor", effort: "high" },
    review: { agent: "codex" as const, model: "reviewer", effort: "high" },
  };

  it("shows the backend review default, preserves settings across toggles and submits all three roles", async () => {
    mocks.planExecuteDefaults.mockResolvedValue(config);
    await open({ planExecute: { plan: {}, exec: {} } }, false);
    const reviewer = within(await screen.findByRole("region", { name: "launch.reviewTitle" }));
    expect((screen.getByRole("checkbox", { name: "launch.reviewEnabled" }) as HTMLInputElement).checked).toBe(true);
    pickModel(reviewer.getByLabelText("spawn.modelLabel"), "custom-reviewer");
    fireEvent.click(screen.getByRole("checkbox", { name: "launch.reviewEnabled" }));
    expect(screen.queryByRole("region", { name: "launch.reviewTitle" })).toBeNull();
    expect(screen.getByText("launch.reviewDisabledHint")).toBeTruthy();
    fireEvent.click(screen.getByRole("checkbox", { name: "launch.reviewEnabled" }));
    expect(within(screen.getByRole("region", { name: "launch.reviewTitle" })).getByLabelText("spawn.modelLabel").textContent).toContain("custom-reviewer");
    const req = await launch();
    expect(req.planExecute?.reviewEnabled).toBe(true);
    expect(req.planExecute?.review?.model).toBe("custom-reviewer");
    expect(mocks.applyLaunchArgs).toHaveBeenCalledWith("codex", null, "custom-reviewer", "high");
  });

  it("submits disabled review without validating the unused reviewer", async () => {
    mocks.planExecuteDefaults.mockResolvedValue({ ...config, reviewEnabled: false, review: { agent: "terminal", model: "invalid model" } });
    await open({ planExecute: { reviewEnabled: false, plan: {}, exec: {} } }, false);
    await screen.findByRole("region", { name: "launch.planTitle" });
    expect(screen.queryByRole("region", { name: "launch.reviewTitle" })).toBeNull();
    const req = await launch();
    expect(req.planExecute?.reviewEnabled).toBe(false);
    expect(mocks.applyLaunchArgs).toHaveBeenCalledTimes(2);
  });

  it("locks the review setting and all saved role choices after confirmation", async () => {
    const request: SpawnRequest = { requestId: "request-id", parentSessionId: "p1", prompt: "Task", planExecute: config };
    mocks.store.pendingSpawns = [request];
    mocks.store.spawnReceipts["request-id"] = confirmedReceipt(request, "failed", config);
    mocks.store.planExecutePrefs.review = { agent: "claude", model: "different" };
    render(<SpawnConfirmModal />);
    const reviewer = within(await screen.findByRole("region", { name: "launch.reviewTitle" }));
    const toggle = screen.getByRole("checkbox", { name: "launch.reviewEnabled" }) as HTMLInputElement;
    expect(toggle.disabled).toBe(true);
    fireEvent.click(toggle);
    expect(toggle.checked).toBe(true);
    expect(reviewer.getByLabelText("spawn.modelLabel").textContent).toContain("reviewer");
    expect((reviewer.getByLabelText("spawn.modelLabel") as HTMLButtonElement).disabled).toBe(true);
    expect(mocks.planExecuteDefaults).not.toHaveBeenCalled();
  });
});
