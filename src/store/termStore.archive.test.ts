//! archiveSession and archiveMany must report a backend refusal that protects a worker worktree.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ({
  setSessionArchived: vi.fn().mockResolvedValue(undefined),
  setCollapsed: vi.fn().mockResolvedValue(undefined),
  listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }),
}));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { listTree, setSessionArchived } from "../ipc/tree";
import { useTermStore } from "./termStore";

const REFUSAL = '"worker" still holds a worktree: worktree has no verified landing.';

beforeEach(() => {
  vi.mocked(setSessionArchived).mockReset();
  vi.mocked(setSessionArchived).mockResolvedValue(undefined);
  vi.mocked(listTree).mockClear();
  useTermStore.setState({ selection: [], selectionAnchor: null });
});

describe("archiveSession", () => {
  it("rejects with the backend reason instead of reporting a silent archive", async () => {
    vi.mocked(setSessionArchived).mockRejectedValueOnce(new Error(REFUSAL));

    await expect(useTermStore.getState().archiveSession("s1")).rejects.toThrow(REFUSAL);
    expect(listTree).not.toHaveBeenCalled();
  });

  it("archives and reloads the tree when the backend accepts", async () => {
    await useTermStore.getState().archiveSession("s1");

    expect(setSessionArchived).toHaveBeenCalledWith("s1", true);
    expect(listTree).toHaveBeenCalledTimes(1);
  });
});

describe("archiveMany", () => {
  it("archives every accepted session and rejects with each refusal", async () => {
    vi.mocked(setSessionArchived).mockImplementation(async (id: string) => {
      if (id !== "s2") throw new Error(`${id}: ${REFUSAL}`);
    });

    const archiving = useTermStore.getState().archiveMany(["s1", "s2", "s3"]);

    await expect(archiving).rejects.toThrow(/s1: .*\n.*s3: /s);
    expect(setSessionArchived).toHaveBeenCalledTimes(3);
    // The accepted sessions are archived, so the tree still reloads and the selection clears.
    expect(listTree).toHaveBeenCalledTimes(1);
    expect(useTermStore.getState().selection).toEqual([]);
  });

  it("resolves when no session is refused", async () => {
    await expect(
      useTermStore.getState().archiveMany(["s1", "s2"]),
    ).resolves.toBeUndefined();
    expect(setSessionArchived).toHaveBeenCalledTimes(2);
  });
});
