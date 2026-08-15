//! executeSpawn must deliver model/effort/name to session creation and fall back to the per-agent
//! default launch arguments when the request has none.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn().mockRejectedValue(new Error("not a repo")),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  spawnResult: vi.fn().mockResolvedValue(true),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ({
  createSession: vi.fn(),
  setCollapsed: vi.fn().mockResolvedValue(undefined),
  listTree: vi
    .fn()
    .mockResolvedValue({ projects: [], groups: [], sessions: [] }),
}));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { spawnResult } from "../ipc/commands";
import { createSession } from "../ipc/tree";
import { useTermStore } from "./termStore";
import type { Session } from "../types";

const parent: Session = {
  id: "parent-1",
  projectId: "proj-1",
  groupId: null,
  name: "lead",
  kind: "claude",
  cwd: "/repo",
  collapsed: false,
  sortOrder: 0,
  createdAt: 0,
};

beforeEach(() => {
  vi.mocked(createSession).mockClear();
  vi.mocked(createSession).mockImplementation(async (input) => ({
    ...parent,
    id: "child-1",
    name: input.name,
    kind: input.kind ?? "terminal",
    parentSessionId: input.parentSessionId,
  }));
  useTermStore.setState({
    projects: [
      {
        id: "proj-1",
        name: "p",
        rootPath: "/repo",
        sortOrder: 0,
        collapsed: false,
        createdAt: 0,
      },
    ],
    groups: [],
    sessions: [parent],
    runtimes: {},
    pendingPrompts: {},
    agentDefaults: { claude: { args: "--verbose", permissionMode: "skip" } },
    // Isolate the creation behavior from tab/pane side effects.
    openSession: vi.fn(),
  });
});

describe("executeSpawn launch configuration", () => {
  it("passes model, effort, name, and explicit agent args to session creation", async () => {
    await useTermStore.getState().executeSpawn({
      parentSessionId: "parent-1",
      prompt: "implement auth",
      kind: "claude",
      worktree: false,
      model: "fable",
      effort: "high",
      name: "critical-auth",
      agentArgs: "--foo bar",
    });
    expect(createSession).toHaveBeenCalledWith(
      expect.objectContaining({
        parentSessionId: "parent-1",
        name: "critical-auth",
        kind: "claude",
        model: "fable",
        effort: "high",
        agentArgs: "--foo bar",
      }),
    );
    // The prompt is queued for injection at first PTY launch.
    expect(useTermStore.getState().pendingPrompts["child-1"]).toBe(
      "implement auth",
    );
  });

  it("falls back to per-agent default args and permission mode when the request has none", async () => {
    await useTermStore.getState().executeSpawn({
      parentSessionId: "parent-1",
      prompt: "small task",
      kind: "claude",
      worktree: false,
    });
    expect(createSession).toHaveBeenCalledWith(
      expect.objectContaining({
        agentArgs: "--verbose",
        permissionMode: "skip",
        model: null,
        effort: null,
      }),
    );
    // Without an explicit name, the child name derives from the prompt.
    expect(vi.mocked(createSession).mock.calls[0][0].name).toBe("small task");
  });

  it("inherits the parent's permission mode over the per-agent default", async () => {
    useTermStore.setState({
      sessions: [{ ...parent, permissionMode: "skip" }],
      agentDefaults: { claude: { args: "", permissionMode: "default" } },
    });
    await useTermStore.getState().executeSpawn({
      parentSessionId: "parent-1",
      prompt: "task",
      kind: "claude",
      worktree: false,
    });
    expect(createSession).toHaveBeenCalledWith(
      expect.objectContaining({ permissionMode: "skip" }),
    );
  });

  it("prefers an explicitly requested permission mode over inheritance", async () => {
    useTermStore.setState({
      sessions: [{ ...parent, permissionMode: "default" }],
      agentDefaults: { claude: { args: "", permissionMode: "default" } },
    });
    await useTermStore.getState().executeSpawn({
      parentSessionId: "parent-1",
      prompt: "task",
      kind: "claude",
      worktree: false,
      permissionMode: "skip",
    });
    expect(createSession).toHaveBeenCalledWith(
      expect.objectContaining({ permissionMode: "skip" }),
    );
  });

  it("maps inherit from a Claude parent to a different child agent", async () => {
    useTermStore.setState({
      sessions: [{ ...parent, permissionMode: "skip" }],
      agentDefaults: { codex: { permissionMode: "default" } },
    });
    await useTermStore.getState().executeSpawn({
      parentSessionId: "parent-1",
      prompt: "task",
      kind: "codex",
      worktree: false,
      permissionMode: "inherit",
    });
    expect(createSession).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "codex", permissionMode: "skip" }),
    );
  });

  it("maps inherit from a Codex parent to a Claude child", async () => {
    useTermStore.setState({
      sessions: [{ ...parent, kind: "codex", permissionMode: "skip" }],
      agentDefaults: { claude: { permissionMode: "default" } },
    });
    await useTermStore.getState().executeSpawn({
      parentSessionId: "parent-1",
      prompt: "task",
      kind: "claude",
      worktree: false,
      permissionMode: "inherit",
    });
    expect(createSession).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "claude", permissionMode: "skip" }),
    );
  });

  it("releases a parked vagent spawn caller as soon as the card is queued", async () => {
    useTermStore.setState({ spawnConfirm: true, notifyEnabled: false });
    await useTermStore.getState().handleSpawnRequest({
      parentSessionId: "parent-1",
      prompt: "task",
      requestId: "req-45",
    });
    expect(useTermStore.getState().pendingSpawns).toHaveLength(1);
    expect(createSession).not.toHaveBeenCalled();
    expect(spawnResult).toHaveBeenCalledWith("req-45", {
      awaitingConfirmation: true,
    });
  });

  it("reports a correlated spawn back with the created session id and worktree error", async () => {
    await useTermStore.getState().executeSpawn({
      parentSessionId: "parent-1",
      prompt: "task",
      kind: "claude",
      // Worktree requested but the mock rejects, so the error is reported alongside success.
      worktree: true,
      requestId: "req-42",
    });
    expect(spawnResult).toHaveBeenCalledWith("req-42", {
      sessionId: "child-1",
      worktreeError: "not a repo",
    });

    // A missing parent reports an error instead of leaving the caller to time out.
    vi.mocked(spawnResult).mockClear();
    await useTermStore.getState().executeSpawn({
      parentSessionId: "gone",
      prompt: "task",
      requestId: "req-43",
    });
    expect(spawnResult).toHaveBeenCalledWith("req-43", {
      error: "parent session no longer exists",
    });
  });

  it("reports cancellation for the queue head", () => {
    useTermStore.setState({
      pendingSpawns: [
        { parentSessionId: "parent-1", prompt: "x", requestId: "req-44" },
      ],
    });
    useTermStore.getState().cancelSpawn();
    expect(spawnResult).toHaveBeenCalledWith("req-44", {
      error: "cancelled by user",
    });
    expect(useTermStore.getState().pendingSpawns).toHaveLength(0);
  });
});
