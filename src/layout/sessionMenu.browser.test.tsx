import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

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
      setActiveTab: vi.fn(),
      closeTab: vi.fn(),
      requestCloseDocTab: vi.fn(),
      addGroup: vi.fn(),
      groups: [],
      sessions: [],
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
  worktreesInSubtree: vi.fn(() => Promise.resolve([])),
}));
vi.mock("../ipc/info", () => ({ copyText: vi.fn(), openDir: vi.fn() }));
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
