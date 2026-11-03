import { act, fireEvent, render, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Session, SessionRuntime } from "../types";
import type { TreeNodeRef } from "./LeftSidebar/ProjectTree";

const testState = vi.hoisted(() => {
  const addSession = vi.fn();
  const openSession = vi.fn();
  const newScratchTab = vi.fn();
  return {
    addSession,
    openSession,
    newScratchTab,
    state: {
      addSession,
      openSession,
      newScratchTab,
      openMerge: vi.fn(),
      openChanges: vi.fn(),
      updateSession: vi.fn(),
      deleteNode: vi.fn(),
      deleteMany: vi.fn(),
      moveNode: vi.fn(),
      moveMany: vi.fn(),
      archiveSession: vi.fn(),
      clearNodeWorktree: vi.fn(),
      forkSession: vi.fn(),
      closeSession: vi.fn(),
      setRuntime: vi.fn(),
      runtimes: {} as Record<string, SessionRuntime>,
      setActiveTab: vi.fn(),
      closeTab: vi.fn(),
      requestCloseDocTab: vi.fn(),
      addGroup: vi.fn(),
      groups: [],
      sessions: [] as Session[],
      ephemeralSessions: {},
      projects: [{ id: "project-1", rootPath: "/tmp/project" }],
      agentDefaults: {},
      agentPresets: [],
    },
  };
});

const platformEnv = vi.hoisted(() => ({ isTauri: true, isElectron: false }));

vi.mock("../i18n", () => ({
  t: (key: string) => key,
  useT: () => (key: string) => key,
}));
vi.mock("../platform", () => ({ env: platformEnv }));
vi.mock("../components/Backdrop", () => ({ Backdrop: ({ children }: { children: React.ReactNode }) => <>{children}</> }));
vi.mock("../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof testState.state) => unknown) => selector(testState.state),
    { getState: () => testState.state },
  );
  return { useTermStore };
});
vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  downloadFullGitbash: vi.fn(),
  gitbashStatus: vi.fn(() => Promise.resolve(null)),
  listShells: vi.fn(() => Promise.resolve([])),
  ptyKill: vi.fn(() => Promise.resolve()),
  removeWorktree: vi.fn(),
}));
vi.mock("../ipc/info", () => ({ copyText: vi.fn(), openDir: vi.fn() }));
vi.mock("../ipc/chat", () => ({ chatStop: vi.fn(() => Promise.resolve()) }));
vi.mock("../ipc/events", () => ({
  onGitbashDownloadDone: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("../hooks/useGitBranch", () => ({
  invalidateGitBranch: vi.fn(),
  isWorktreeGone: () => false,
  peekGitBranchInfo: () => ({ isRepo: false }),
  prefetchGitBranchInfo: vi.fn(),
}));
vi.mock("./sessionMenuDialogs", () => ({
  ConfirmDelete: () => null,
  DeleteWorktree: () => null,
  GroupInfo: () => null,
  NewAgentSession: () => null,
  NewGroup: () => null,
  ResumeSession: () => null,
  SessionInfo: () => null,
}));

import { useSessionMenu } from "./sessionMenu";
import { chatStop } from "../ipc/chat";
import { ptyKill } from "../ipc/commands";

describe("ending a session process", () => {
  const node = { id: "kill-session", kind: "session", name: "Agent", projectId: "project-1", groupId: null } as TreeNodeRef;
  beforeEach(() => {
    vi.mocked(chatStop).mockReset().mockResolvedValue(undefined);
    vi.mocked(ptyKill).mockReset().mockResolvedValue(undefined);
    testState.state.closeSession.mockClear();
    testState.state.setRuntime.mockClear();
    testState.state.runtimes = { [node.id]: { status: "running" } };
  });

  it.each(["chat", "tui"] as const)("stops the %s engine before closing its view", async (engine) => {
    testState.state.sessions = [{ ...node, kind: "codex", engine } as unknown as Session];
    let finish!: () => void;
    const stop = engine === "chat" ? vi.mocked(chatStop) : vi.mocked(ptyKill);
    stop.mockReturnValueOnce(new Promise<void>((resolve) => { finish = resolve; }));
    const { result } = renderHook(() => useSessionMenu());
    const kill = result.current.buildSessionItems(node).find(item => item.label === "tree.killProcess")!;
    act(() => { kill.onClick?.({} as React.MouseEvent); });
    expect(stop).not.toHaveBeenCalled();
    const view = render(result.current.dialogs);
    fireEvent.click(view.getByRole("button", { name: "tree.killProcess" }));
    view.rerender(result.current.dialogs);
    expect((view.getByRole("button", { name: "tree.killProcess" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(view.getByRole("button", { name: "tree.killProcess" }));
    expect(stop).toHaveBeenCalledTimes(1);
    expect(stop).toHaveBeenCalledWith(node.id);
    expect(engine === "chat" ? ptyKill : chatStop).not.toHaveBeenCalled();
    expect(testState.state.closeSession).not.toHaveBeenCalled();
    await act(async () => { finish(); });
    expect(testState.state.setRuntime).toHaveBeenCalledWith(node.id, { status: "exited", pid: undefined });
    expect(testState.state.closeSession).toHaveBeenCalledWith(node.id);
    view.rerender(result.current.dialogs);
    expect(view.queryByRole("alertdialog")).toBeNull();
    view.unmount();
  });

  it.each([
    [{ status: "idle", alive: true }, true],
    [{ status: "running", alive: false }, false],
    [{ status: "running" }, true],
    [{ status: "idle" }, false],
  ] as const)("offers Kill Process for runtime %o: %s", (runtime, offered) => {
    testState.state.runtimes = { [node.id]: runtime };
    const { result } = renderHook(() => useSessionMenu());
    const labels = result.current.buildSessionItems(node).map(item => item.label);
    expect(labels.includes("tree.killProcess")).toBe(offered);
  });

  it.each(["cancel", "escape"])("does not stop the process after %s", (method) => {
    const { result } = renderHook(() => useSessionMenu());
    act(() => { result.current.buildSessionItems(node).find(item => item.label === "tree.killProcess")?.onClick?.({} as React.MouseEvent); });
    const view = render(result.current.dialogs);
    if (method === "cancel") fireEvent.click(view.getByRole("button", { name: "common.cancel" }));
    else fireEvent.keyDown(view.getByRole("alertdialog"), { key: "Escape" });
    view.rerender(result.current.dialogs);
    expect(view.queryByRole("alertdialog")).toBeNull();
    expect(chatStop).not.toHaveBeenCalled();
    expect(ptyKill).not.toHaveBeenCalled();
    expect(testState.state.closeSession).not.toHaveBeenCalled();
    view.unmount();
  });

  it("keeps the view and reports a stop failure", async () => {
    testState.state.sessions = [{ ...node, kind: "codex", engine: "chat" } as unknown as Session];
    vi.mocked(chatStop).mockRejectedValueOnce(new Error("Unable to stop agent"));
    const { result } = renderHook(() => useSessionMenu());
    act(() => { result.current.buildSessionItems(node).find(item => item.label === "tree.killProcess")?.onClick?.({} as React.MouseEvent); });
    const view = render(result.current.dialogs);
    await act(async () => { fireEvent.click(view.getByRole("button", { name: "tree.killProcess" })); });
    expect(testState.state.closeSession).not.toHaveBeenCalled();
    expect(testState.state.setRuntime).not.toHaveBeenCalled();
    view.rerender(result.current.dialogs);
    expect(view.getByRole("alert").textContent).toBe("Error: Unable to stop agent");
    view.unmount();
  });
});

describe("browser page entry in the tree context menu", () => {
  beforeEach(() => {
    testState.addSession.mockReset();
    testState.openSession.mockReset();
    testState.addSession.mockResolvedValue({ id: "browser-node-1" });
    platformEnv.isTauri = true;
    platformEnv.isElectron = false;
  });

  it("creates and opens a permanent browser node under the selected group", async () => {
    const { result } = renderHook(() => useSessionMenu());
    const items = result.current.newSessionItems("project-1", "group-1", null, {
      withBrowser: true,
    });
    const browserItem = items.find((item) => item.label === "tree.newBrowserPage");

    expect(browserItem).toBeTruthy();
    act(() => browserItem?.onClick?.({} as React.MouseEvent));

    await waitFor(() => {
      expect(testState.addSession).toHaveBeenCalledWith(
        expect.objectContaining({
          projectId: "project-1",
          groupId: "group-1",
          parentSessionId: null,
          kind: "browser",
        }),
      );
      expect(testState.openSession).toHaveBeenCalledWith("browser-node-1");
    });
  });

  it("does not leak the browser entry into child-session menus or browser-only clients", () => {
    const { result } = renderHook(() => useSessionMenu());

    expect(
      result.current
        .newSessionItems("project-1", null, "session-1")
        .some((item) => item.label === "tree.newBrowserPage"),
    ).toBe(false);

    platformEnv.isTauri = false;
    expect(
      result.current
        .newSessionItems("project-1", null, null, { withBrowser: true })
        .some((item) => item.label === "tree.newBrowserPage"),
    ).toBe(false);
  });
});

describe("agent session in a nested group", () => {
  beforeEach(() => {
    testState.addSession.mockReset();
    testState.addSession.mockResolvedValue({ id: "agent-node-1" });
    Object.assign(testState.state, {
      groups: [
        { id: "outer", projectId: "project-1", parentGroupId: null, worktreePath: "/repo/.vlx-worktrees/outer", worktreeBaseRef: "refs/heads/main" },
        { id: "nested", projectId: "project-1", parentGroupId: "outer", worktreePath: null },
      ],
    });
  });

  it("starts in the worktree bound on an ancestor group", async () => {
    const { result } = renderHook(() => useSessionMenu());
    const agents = result.current.newSessionItems("project-1", "nested").find((item) => item.label === "tree.newAgentSessionGroup");
    const claude = agents?.submenu?.find((item) => item.label === "tree.newAgentSession");
    act(() => claude?.onClick?.({} as React.MouseEvent));

    await waitFor(() => {
      expect(testState.addSession).toHaveBeenCalledWith(
        expect.objectContaining({
          groupId: "nested",
          kind: "claude",
          cwd: "/repo/.vlx-worktrees/outer",
          worktreePath: "/repo/.vlx-worktrees/outer",
          worktreeBaseRef: "refs/heads/main",
        }),
      );
    });
  });
});

describe("terminal entry in the group context menu", () => {
  beforeEach(() => testState.newScratchTab.mockReset());

  it("opens a scratch terminal scoped to the selected group", () => {
    const { result } = renderHook(() => useSessionMenu());
    const items = result.current.newSessionItems("project-1", "group-1", null, {
      withBrowser: true,
      withTerminal: true,
    });
    const terminalItem = items.find((item) => item.label === "tree.newTerminalSession");

    expect(terminalItem).toBeTruthy();
    act(() => terminalItem?.onClick?.({} as React.MouseEvent));

    expect(testState.newScratchTab).toHaveBeenCalledWith({
      target: { projectId: "project-1", groupId: "group-1", sessionId: null },
    });
  });
});


describe("session knowledge base entry", () => {
  it("keeps the knowledge-base action at the top level without an experimental wrapper", () => {
    testState.state.sessions = [{ id: "kb-session", kind: "claude", name: "Agent", projectId: "project-1", groupId: null } as unknown as Session];
    const { result } = renderHook(() => useSessionMenu());
    const node = { id: "kb-session", kind: "session", name: "Agent", projectId: "project-1", groupId: null } as TreeNodeRef;
    const items = result.current.buildSessionItems(node);

    expect(items.find(item => item.label === "memory.add")).toBeTruthy();
    expect(items.some(item => item.label === "common.experimental")).toBe(false);
  });
});

describe("planning workflow creation entry", () => {
  it.each([[null, null], ["group-1", null], ["group-1", "parent-1"]])("links to the dialog at group %s and parent %s without creating a session", (groupId, parentId) => {
    testState.addSession.mockClear();
    window.history.replaceState(null, "", "/?session=existing");
    const { result, unmount } = renderHook(() => useSessionMenu());
    const entry = result.current.newSessionItems("project-1", groupId, parentId).find(item => item.label === "tree.newPlanExecuteSession")!;
    const url = new URL(entry.href!);
    expect(url.searchParams.get("planProject")).toBe("project-1");
    expect(url.searchParams.get("planGroup")).toBe(groupId);
    expect(url.searchParams.get("planParent")).toBe(parentId);
    expect(url.searchParams.get("session")).toBe("existing");
    expect(url.searchParams.get("planExecute")).toBeTruthy();
    act(() => { entry.onClick?.({} as React.MouseEvent); });
    expect(window.location.href).toBe(entry.href);
    expect(testState.addSession).not.toHaveBeenCalled();
    unmount();
    window.history.replaceState(null, "", "/");
  });
});
