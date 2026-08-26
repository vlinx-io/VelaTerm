//! Regression coverage for the spawn confirmation claim.
//!
//! The confirmation card is shown on every connected client, so the same request can be answered twice
//! within the same second — one person on a desktop and a phone, or two people. The backend hands the
//! request to exactly one answer; a client that loses must drop its card and do nothing, or the task runs
//! twice with two worktrees and two agents.

import { beforeEach, describe, expect, it, vi } from "vitest";

const resolveSpawn = vi.hoisted(() => vi.fn());

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
  resolveSpawn,
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

const request = { parentSessionId: "parent-1", prompt: "build the thing" };

/** Queue one card and run confirm against a stubbed executeSpawn, returning that stub. */
async function confirmWith(claim: () => Promise<boolean>) {
  const executeSpawn = vi.fn().mockResolvedValue(undefined);
  resolveSpawn.mockImplementation(claim);
  useTermStore.setState({ pendingSpawns: [{ ...request }], executeSpawn });
  await useTermStore.getState().confirmSpawn({ ...request });
  return executeSpawn;
}

describe("spawn confirmation claim", () => {
  beforeEach(() => {
    resolveSpawn.mockReset();
  });

  it("executes the task when this client wins the claim", async () => {
    const executeSpawn = await confirmWith(() => Promise.resolve(true));

    expect(resolveSpawn).toHaveBeenCalledWith("parent-1", "build the thing", true);
    expect(executeSpawn).toHaveBeenCalledTimes(1);
    // The card leaves the queue either way; only the work is conditional.
    expect(useTermStore.getState().pendingSpawns).toHaveLength(0);
  });

  it("does nothing but drop the card when another client answered first", async () => {
    const executeSpawn = await confirmWith(() => Promise.resolve(false));

    expect(executeSpawn).not.toHaveBeenCalled();
    expect(useTermStore.getState().pendingSpawns).toHaveLength(0);
  });

  it("still executes when the backend cannot answer", async () => {
    // An older backend or a transport error must not strand the task: fall back to the previous
    // behavior of executing, which risks a duplicate only in the case that was already possible.
    const executeSpawn = await confirmWith(() => Promise.reject(new Error("unknown command")));

    expect(executeSpawn).toHaveBeenCalledTimes(1);
  });

  it("claims a cancelled card too, so a cancel racing a confirm settles on one answer", () => {
    resolveSpawn.mockResolvedValue(true);
    useTermStore.setState({ pendingSpawns: [{ ...request }] });

    useTermStore.getState().cancelSpawn();

    expect(resolveSpawn).toHaveBeenCalledWith("parent-1", "build the thing", false);
    expect(useTermStore.getState().pendingSpawns).toHaveLength(0);
  });
});

/** Run one request through handleSpawnRequest against a stubbed executeSpawn, returning that stub. */
async function handleWith(
  claim: () => Promise<boolean>,
  patch: Partial<{ spawnConfirm: boolean }> = {},
) {
  const executeSpawn = vi.fn().mockResolvedValue(undefined);
  resolveSpawn.mockImplementation(claim);
  useTermStore.setState({
    spawnConfirm: false,
    notifyEnabled: false,
    pendingSpawns: [],
    executeSpawn,
    ...patch,
  });
  await useTermStore.getState().handleSpawnRequest({ ...request });
  return executeSpawn;
}

describe("spawn claim on the immediate-start path", () => {
  beforeEach(() => {
    resolveSpawn.mockReset();
  });

  it("claims before running when confirmation is off", async () => {
    const executeSpawn = await handleWith(() => Promise.resolve(true));

    expect(resolveSpawn).toHaveBeenCalledWith("parent-1", "build the thing", true);
    expect(executeSpawn).toHaveBeenCalledTimes(1);
  });

  it("does not run when another client already claimed the request", async () => {
    // Two clients with confirmation off both receive the same spawn event. Without a claim each one
    // starts the task, producing two worktrees and two agents for one request.
    const executeSpawn = await handleWith(() => Promise.resolve(false));

    expect(executeSpawn).not.toHaveBeenCalled();
  });

  it("still runs when the backend cannot answer the claim", async () => {
    const executeSpawn = await handleWith(() =>
      Promise.reject(new Error("unknown command")),
    );

    expect(executeSpawn).toHaveBeenCalledTimes(1);
  });

  it("queues a card without claiming when confirmation is on", async () => {
    // The claim belongs to the answer, not to the arrival: claiming here would settle the request
    // before anyone looked at it and dismiss the card on every other client.
    const executeSpawn = await handleWith(() => Promise.resolve(true), {
      spawnConfirm: true,
    });

    expect(resolveSpawn).not.toHaveBeenCalled();
    expect(executeSpawn).not.toHaveBeenCalled();
    expect(useTermStore.getState().pendingSpawns).toHaveLength(1);
  });
});
