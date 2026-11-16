import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Session } from "../types";

const mocks = vi.hoisted(() => ({ rename: vi.fn(), cancel: vi.fn(), options: vi.fn(), models: vi.fn(), t: (key: string) => key }));
const state = vi.hoisted(() => new Proxy({
  sessions: [] as Session[], projects: [], groups: [], agentPresets: [], agentDefaults: {},
  runtimes: {}, paneTrees: {}, sidebarTreeViews: [], ephemeralSessions: {}, openTabs: [], liveTabs: [],
  sessionTitlePrefs: {},
  sessionTitleAgent: "",
  setSessionTitleAgent: vi.fn(),
}, { get: (target, key) => key in target ? target[key as keyof typeof target] : vi.fn() }));
vi.mock("../store/termStore", () => ({
  useTermStore: Object.assign((select: (s: typeof state) => unknown) => select(state), { getState: () => state }),
}));
vi.mock("../ipc/tree", () => ({ renameSessionWithAgent: mocks.rename, cancelSessionTitle: mocks.cancel, sessionTitleOptions: mocks.options }));
vi.mock("../ipc/launch", async importOriginal => ({ ...await importOriginal<object>(), launchModels: mocks.models }));
vi.mock("../ipc/commands", async importOriginal => ({
  ...await importOriginal<object>(), listShells: vi.fn().mockResolvedValue([]), gitbashStatus: vi.fn().mockResolvedValue(null),
}));
vi.mock("../hooks/useGitBranch", () => ({
  isWorktreeGone: () => false, peekGitBranchInfo: () => null, prefetchGitBranchInfo: vi.fn(), invalidateGitBranch: vi.fn(),
}));
vi.mock("../ipc/events", async importOriginal => ({
  ...await importOriginal<object>(), onGitbashDownloadDone: () => Promise.resolve(() => {}),
}));
vi.mock("../i18n", () => ({ t: mocks.t, useT: () => mocks.t }));

import { useSessionMenu } from "./sessionMenu";
import { SmartRenameRoute } from "./SmartRename/SmartRenameRoute";
import { readSmartRenameRoute } from "./SmartRename/navigation";

function session(kind: Session["kind"] = "claude", agentSessionId: string | null = "native-id"): Session {
  return { id: "session", projectId: "project", groupId: null, name: "Existing title", kind,
    agentSessionId, shell: null, cwd: null, envJson: null, initCmd: null, hotkey: null,
    parentSessionId: null, collapsed: false, worktreePath: null, sortOrder: 0, createdAt: 0 };
}
function titleItem(menu: ReturnType<typeof useSessionMenu>, s: Session) {
  return menu.buildSessionItems({ kind: "session", id: s.id, name: s.name, projectId: s.projectId, groupId: s.groupId ?? null })
    .find(item => item.label.startsWith("sessionTitle."));
}
const spec = { supportsPlanExecute: true, supportsReferSummary: true, acceptsTask: true, effortFlag: "--effort", effortLevels: ["low", "high"], modelSelectable: true };
function options(available = true) {
  return { agent: "claude", sessionName: "Existing title", agents: [
    { ...spec, id: "claude", label: "Claude", available, model: "current-model", effort: "high" },
    { ...spec, id: "codex", label: "Codex", available: true, model: "codex-default", effort: "low" },
  ] };
}
function openRoute(query = "?smartRename=session") { window.history.replaceState(null, "", `/${query}`); render(<SmartRenameRoute />); }
beforeEach(() => {
  state.sessions = [session()]; mocks.rename.mockReset(); mocks.options.mockReset(); mocks.models.mockReset();
  mocks.cancel.mockReset(); mocks.cancel.mockResolvedValue(true);
  state.sessionTitlePrefs = {}; state.sessionTitleAgent = ""; state.setSessionTitleAgent.mockReset();
  mocks.options.mockResolvedValue(options());
  mocks.models.mockResolvedValue({ models: [{ id: "custom-model", label: "Custom", effortLevels: ["low", "high", "ultra"] }], effortLevels: ["low", "high", "ultra"] });
  window.history.replaceState(null, "", "/");
});
afterEach(cleanup);

describe("AI session renaming", () => {
  it("appears for recorded agent sessions in the shared sidebar/tab menu", () => {
    const { result } = renderHook(useSessionMenu);
    expect(titleItem(result.current, state.sessions[0])?.disabled).toBe(false);
    expect(titleItem(result.current, state.sessions[0])?.href).toContain("smartRename=session");
    state.sessions = [session("claude", null)];
    const empty = renderHook(useSessionMenu);
    expect(titleItem(empty.result.current, state.sessions[0])?.disabled).toBe(true);
    for (const kind of ["terminal", "browser"] as const) {
      state.sessions = [session(kind)];
      const excluded = renderHook(useSessionMenu);
      expect(titleItem(excluded.result.current, state.sessions[0])).toBeUndefined();
    }
  });

  it("always opens confirmation before generating, including repeated menu clicks", () => {
    const { result } = renderHook(useSessionMenu);
    const item = titleItem(result.current, state.sessions[0])!;
    act(() => { item.onClick?.({ preventDefault: vi.fn() } as unknown as React.MouseEvent); item.onClick?.({ preventDefault: vi.fn() } as unknown as React.MouseEvent); });
    expect(readSmartRenameRoute()).toEqual({ sessionId: "session", agent: null, model: null, effort: null });
    expect(mocks.rename).not.toHaveBeenCalled();
  });

  it("loads options inside the same rename dialog with submission disabled", async () => {
    mocks.options.mockReturnValue(new Promise(() => {}));
    openRoute();
    const dialog = screen.getByRole("dialog", { name: "sessionTitle.rename" });
    expect(dialog.textContent).toContain("sessionTitle.confirmHint");
    expect(screen.getByRole("status").textContent).toContain("common.loading");
    expect(screen.getByRole("button", { name: "common.rename" })).toHaveProperty("disabled", true);
  });

  it("shows backend defaults and the full-conversation notice, and cancellation never generates", async () => {
    openRoute();
    expect(await screen.findByRole("combobox", { name: "orch.agentLabel" })).toHaveProperty("textContent", expect.stringContaining("Claude"));
    expect(screen.getByText("sessionTitle.confirmHint")).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "spawn.modelLabel" }).textContent).toContain("current-model");
    expect(screen.getByRole("combobox", { name: "spawn.effortLabel" }).textContent).toContain("chat.effort.high");
    expect(mocks.rename).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));
    await waitFor(() => expect(readSmartRenameRoute()).toBeNull());
    expect(mocks.rename).not.toHaveBeenCalled();
  });

  it("hides model and effort for an agent whose CLI cannot take a per-run selection", async () => {
    mocks.options.mockResolvedValue({ agent: "zcode", sessionName: "Existing title", agents: [
      { ...spec, id: "zcode", label: "ZCode", available: true, model: "provider/model", effort: "max", modelSelectable: false },
    ] });
    openRoute();
    expect(await screen.findByRole("combobox", { name: "orch.agentLabel" })).toHaveProperty("textContent", expect.stringContaining("ZCode"));
    expect(screen.getByText("sessionTitle.confirmHint")).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "spawn.modelLabel" })).toBeNull();
    expect(screen.queryByRole("combobox", { name: "spawn.effortLabel" })).toBeNull();
    expect(screen.getByRole("button", { name: "common.rename" })).toHaveProperty("disabled", false);
  });

  it("restores the agent and its model and effort chosen last time, even without a session default", async () => {
    state.sessionTitleAgent = "codex";
    state.sessionTitlePrefs = { codex: { model: "custom-model", effort: "ultra" } };
    mocks.options.mockResolvedValue({ ...options(), agent: "", agents: options().agents });
    openRoute();
    expect(await screen.findByRole("combobox", { name: "orch.agentLabel" })).toHaveProperty("textContent", expect.stringContaining("Codex"));
    expect(screen.getByRole("combobox", { name: "spawn.modelLabel" }).textContent).toContain("custom-model");
    expect(screen.getByRole("combobox", { name: "spawn.effortLabel" }).textContent).toContain("chat.effort.ultra");
    expect(screen.getByRole("button", { name: "common.rename" })).toHaveProperty("disabled", false);
  });

  it("remembers an explicitly chosen agent for the next dialog", async () => {
    openRoute();
    const agent = await screen.findByRole("combobox", { name: "orch.agentLabel" });
    fireEvent.keyDown(agent, { key: "Enter" }); fireEvent.keyDown(agent, { key: "ArrowDown" }); fireEvent.keyDown(agent, { key: "Enter" });
    expect(state.setSessionTitleAgent).toHaveBeenCalledWith("codex");
  });

  it("falls back to the session's own agent when the remembered one is unavailable", async () => {
    state.sessionTitleAgent = "codex";
    mocks.options.mockResolvedValue({ ...options(), agents: options().agents.map(item => item.id === "codex" ? { ...item, available: false } : item) });
    openRoute();
    expect(await screen.findByRole("combobox", { name: "orch.agentLabel" })).toHaveProperty("textContent", expect.stringContaining("Claude"));
  });

  it("submits the chosen agent/model/effort only on confirmation and suppresses duplicate submission", async () => {
    let finish!: (value: { title: string; agent: string }) => void;
    mocks.rename.mockReturnValue(new Promise(resolve => { finish = resolve; }));
    openRoute();
    const agent = await screen.findByRole("combobox", { name: "orch.agentLabel" });
    fireEvent.keyDown(agent, { key: "Enter" }); fireEvent.keyDown(agent, { key: "ArrowDown" }); fireEvent.keyDown(agent, { key: "Enter" });
    expect(screen.getByRole("combobox", { name: "spawn.modelLabel" }).textContent).toContain("codex-default");
    expect(screen.getByRole("combobox", { name: "spawn.effortLabel" }).textContent).toContain("chat.effort.low");
    const model = screen.getByRole("combobox", { name: "spawn.modelLabel" });
    const effort = screen.getByRole("combobox", { name: "spawn.effortLabel" });
    await waitFor(() => expect(mocks.models).toHaveBeenCalledWith("codex", { parentSessionId: "session", cwd: null, inheritArgs: true }));
    fireEvent.click(model);
    fireEvent.click(await screen.findByRole("option", { name: /Custom/ }));
    fireEvent.click(effort);
    fireEvent.click(await screen.findByRole("option", { name: "chat.effort.ultra" }));
    expect(readSmartRenameRoute()).toEqual({ sessionId: "session", agent: "codex", model: "custom-model", effort: "ultra" });
    expect(mocks.rename).not.toHaveBeenCalled();
    const submit = screen.getByRole("button", { name: "common.rename" });
    fireEvent.click(submit); fireEvent.click(submit);
    expect(mocks.rename).toHaveBeenCalledTimes(1);
    expect(mocks.rename).toHaveBeenCalledWith("session", "codex", "custom-model", "ultra", expect.any(String));
    expect(screen.getByRole("status").textContent).toContain("sessionTitle.generating");
    expect(screen.getByRole("combobox", { name: "orch.agentLabel" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("combobox", { name: "spawn.modelLabel" })).toHaveProperty("disabled", true);
    await act(async () => { finish({ title: "Generated", agent: "codex" }); });
    await waitFor(() => expect(readSmartRenameRoute()).toBeNull());
    expect(state.sessions[0].name).toBe("Existing title");
  });

  it.each(["button", "Escape"])("cancels a running task via %s and permits a fresh retry", async action => {
    let fail!: (cause: Error) => void;
    mocks.rename.mockReturnValueOnce(new Promise((_resolve, reject) => { fail = reject; }))
      .mockResolvedValueOnce({ title: "Generated", agent: "claude" });
    openRoute();
    await screen.findByRole("combobox", { name: "spawn.modelLabel" });
    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    const operationId = mocks.rename.mock.calls[0][4];
    expect(screen.getByRole("button", { name: "common.cancel" })).toHaveProperty("disabled", false);
    if (action === "button") fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));
    else fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(mocks.cancel).toHaveBeenCalledWith("session", operationId);
    expect(readSmartRenameRoute()).not.toBeNull();
    await act(async () => { fail(new Error("session_title:cancelled")); });
    await waitFor(() => expect(readSmartRenameRoute()).toBeNull());
    expect(screen.queryByRole("alert")).toBeNull();
    expect(state.sessions[0].name).toBe("Existing title");
    act(() => { window.history.pushState(null, "", "/?smartRename=session"); window.dispatchEvent(new PopStateEvent("popstate")); });
    await screen.findByRole("combobox", { name: "spawn.modelLabel" });
    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    await waitFor(() => expect(readSmartRenameRoute()).toBeNull());
    expect(mocks.rename.mock.calls[1][4]).not.toBe(operationId);
  });

  it("cancels on navigation and a late response cannot close a reopened dialog", async () => {
    let finish!: (value: { title: string; agent: string }) => void;
    mocks.rename.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
    openRoute();
    await screen.findByRole("combobox", { name: "spawn.modelLabel" });
    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    act(() => { window.history.pushState(null, "", "/"); window.dispatchEvent(new PopStateEvent("popstate")); });
    expect(mocks.cancel).toHaveBeenCalledWith("session", mocks.rename.mock.calls[0][4]);
    act(() => { window.history.pushState(null, "", "/?smartRename=session"); window.dispatchEvent(new PopStateEvent("popstate")); });
    await screen.findByRole("combobox", { name: "spawn.modelLabel" });
    await act(async () => { finish({ title: "Late result", agent: "claude" }); });
    expect(readSmartRenameRoute()?.sessionId).toBe("session");
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("retains the selections and original title after an error so the user can retry", async () => {
    mocks.rename.mockRejectedValueOnce(new Error("session_title:changed")).mockResolvedValueOnce({ title: "Generated", agent: "claude" });
    openRoute();
    await screen.findByRole("combobox", { name: "spawn.modelLabel" });
    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    expect((await screen.findByRole("alert")).textContent).toContain("sessionTitle.changed");
    expect(screen.getByRole("combobox", { name: "spawn.modelLabel" }).textContent).toContain("current-model");
    expect(readSmartRenameRoute()?.sessionId).toBe("session");
    expect(state.sessions[0].name).toBe("Existing title");
    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    await waitFor(() => expect(readSmartRenameRoute()).toBeNull());
    expect(mocks.rename).toHaveBeenCalledTimes(2);
  });

  it("restores URL selections and explicit native defaults without generating", async () => {
    openRoute("?smartRename=session&smartRenameAgent=codex&smartRenameModel=&smartRenameEffort=");
    expect(await screen.findByRole("combobox", { name: "orch.agentLabel" })).toHaveProperty("textContent", expect.stringContaining("Codex"));
    expect(screen.getByRole("combobox", { name: "spawn.modelLabel" }).textContent).toContain("chat.followModelDefault");
    expect(screen.getByRole("combobox", { name: "spawn.effortLabel" }).textContent).toContain("chat.effort.auto");
    expect(mocks.rename).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(readSmartRenameRoute()).toBeNull();
    expect(new URL(window.location.href).searchParams.size).toBe(0);
  });

  it("requires the user to select an available alternative when the current agent is unavailable", async () => {
    mocks.options.mockResolvedValue(options(false)); openRoute();
    const agent = await screen.findByRole("combobox", { name: "orch.agentLabel" });
    expect(screen.getByRole("button", { name: "common.rename" })).toHaveProperty("disabled", true);
    fireEvent.keyDown(agent, { key: "Enter" });
    expect(screen.getByRole("option", { name: /Claude/ }).getAttribute("aria-disabled")).toBe("true");
    fireEvent.keyDown(agent, { key: "ArrowDown" }); fireEvent.keyDown(agent, { key: "Enter" });
    expect(readSmartRenameRoute()?.agent).toBe("codex");
    fireEvent.keyDown(agent, { key: "Enter" }); fireEvent.keyDown(agent, { key: "Escape" });
    expect(readSmartRenameRoute()?.sessionId).toBe("session");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(mocks.rename).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "common.rename" })).toHaveProperty("disabled", false);
  });
});
