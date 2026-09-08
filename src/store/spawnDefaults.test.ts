//! Coverage for the agent settings a spawned child session starts with.
//!
//! A child created by `vspawn` used to launch with no model and no permission mode at all, ignoring the
//! defaults the user had configured for that agent type. These tests pin the inheritance so the setting
//! cannot be dropped again, and pin the one case that must NOT inherit.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn().mockRejectedValue(new Error("no repo")),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ({
  listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }),
  setCollapsed: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { useTermStore } from "./termStore";
import type { Project, Session } from "../types";

const project: Project = {
  id: "p1",
  name: "p1",
  rootPath: "/tmp/p1",
  collapsed: false,
  sortOrder: 0,
  createdAt: 0,
};

const parent = (kind: Session["kind"]): Session =>
  ({
    id: "parent",
    projectId: "p1",
    groupId: null,
    name: "parent",
    kind,
    cwd: "/tmp/p1",
    parentSessionId: null,
    collapsed: false,
    sortOrder: 0,
    createdAt: 0,
  }) as Session;

/** Capture what executeSpawn asks addSession to create, without touching the backend. */
function captureAddSession() {
  const calls: Record<string, unknown>[] = [];
  useTermStore.setState({
    addSession: (async (input: Record<string, unknown>) => {
      calls.push(input);
      return { ...input, id: `child${calls.length}` } as unknown as Session;
    }) as never,
    openSession: (() => {}) as never,
  });
  return calls;
}

describe("spawned children inherit the agent kind's global defaults", () => {
  beforeEach(() => {
    useTermStore.setState({
      projects: [project],
      sessions: [parent("claude")],
      agentDefaults: {
        claude: { args: "--model opus", permissionMode: "skip" },
        codex: { args: "-m gpt-5.5" },
      },
      pendingPrompts: {},
    });
  });

  it("passes the configured launch args and permission mode through", async () => {
    const calls = captureAddSession();
    await useTermStore
      .getState()
      .executeSpawn({ parentSessionId: "parent", prompt: "fix the login bug" } as never);

    expect(calls).toHaveLength(1);
    expect(calls[0].agentArgs).toBe("--model opus");
    expect(calls[0].permissionMode).toBe("skip");
    expect(calls[0].parentSessionId).toBe("parent");
  });

  it("uses the requested kind's defaults, not the parent's", async () => {
    const calls = captureAddSession();
    await useTermStore
      .getState()
      .executeSpawn({ parentSessionId: "parent", prompt: "t", kind: "codex" } as never);

    // The parent is claude, but the child was asked to be codex, so codex's defaults apply.
    expect(calls[0].kind).toBe("codex");
    expect(calls[0].agentArgs).toBe("-m gpt-5.5");
    expect(calls[0].permissionMode).toBeNull();
  });

  it("sends null rather than an empty string when a kind has no defaults", async () => {
    useTermStore.setState({ agentDefaults: {} });
    const calls = captureAddSession();
    await useTermStore
      .getState()
      .executeSpawn({ parentSessionId: "parent", prompt: "t" } as never);

    // Empty strings would be stored and later tokenized into a stray argument.
    expect(calls[0].agentArgs).toBeNull();
    expect(calls[0].permissionMode).toBeNull();
  });

  it("trims whitespace-only args down to null", async () => {
    useTermStore.setState({ agentDefaults: { claude: { args: "   " } } });
    const calls = captureAddSession();
    await useTermStore
      .getState()
      .executeSpawn({ parentSessionId: "parent", prompt: "t" } as never);

    expect(calls[0].agentArgs).toBeNull();
  });
});
