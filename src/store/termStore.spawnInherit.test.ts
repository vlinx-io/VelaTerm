//! Regression coverage for orchestration spawns: a child created through `spawn://request` must launch with
//! the same permission mode and launch arguments a hand-created session would get. Before this, `executeSpawn`
//! passed neither, so every spawned child persisted `permission_mode = NULL` and came up asking for
//! confirmations even when the parent ran with them skipped.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ({
  listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }),
}));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { useTermStore } from "./termStore";
import type { Session } from "../types";

const parent = {
  id: "parent-1",
  projectId: "proj-1",
  groupId: null,
  name: "lead",
  kind: "claude",
  cwd: "/repo",
  permissionMode: "skip",
  agentArgs: "--model opus",
} as unknown as Session;

/** Returns the input `executeSpawn` handed to `addSession`, with worktree creation left out of the picture. */
async function spawnInput(
  overrides: Partial<Session>,
  agentDefaults: Record<string, { args?: string; permissionMode?: string }>,
  kind?: "claude" | "codex",
) {
  const addSession = vi.fn().mockResolvedValue(null);
  useTermStore.setState({
    sessions: [{ ...parent, ...overrides } as Session],
    projects: [],
    agentDefaults,
    addSession,
  } as never);
  await useTermStore.getState().executeSpawn({
    parentSessionId: "parent-1",
    prompt: "investigate the failing test",
    kind: kind ?? null,
    worktree: false,
  });
  expect(addSession).toHaveBeenCalledTimes(1);
  return addSession.mock.calls[0][0];
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("spawned child launch configuration", () => {
  it("inherits the parent's permission mode and arguments when it runs the same agent", async () => {
    const input = await spawnInput({}, {});
    expect(input.permissionMode).toBe("skip");
    expect(input.agentArgs).toBe("--model opus");
  });

  it("falls back to the agent kind's global defaults when the parent carries none", async () => {
    const input = await spawnInput(
      { permissionMode: null, agentArgs: null },
      { claude: { permissionMode: "skip", args: "--model sonnet" } },
    );
    expect(input.permissionMode).toBe("skip");
    expect(input.agentArgs).toBe("--model sonnet");
  });

  it("does not carry the parent's arguments into a child running a different agent", async () => {
    const input = await spawnInput({}, { codex: { args: "--full-auto" } }, "codex");
    // Arguments are agent-specific, so only the kind's own defaults apply.
    expect(input.agentArgs).toBe("--full-auto");
    // The permission mode is a user intent rather than an agent flag, so it still follows the parent.
    expect(input.permissionMode).toBe("skip");
  });

  it("leaves both unset when neither the parent nor the defaults specify anything", async () => {
    const input = await spawnInput({ permissionMode: null, agentArgs: null }, {});
    expect(input.permissionMode).toBeNull();
    expect(input.agentArgs).toBeNull();
  });
});
