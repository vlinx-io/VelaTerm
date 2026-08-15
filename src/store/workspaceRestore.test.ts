//! Saved-workspace tests: writing the exit snapshot, restoring it on the next desktop launch as dormant
//! placeholders, consuming it exactly once, and waking a dormant session.
//!
//! Stub the Tauri-touching IPC/notification modules so the store loads under Node/jsdom, as in docTabs.test.ts.
//! The platform adapter is stubbed as a desktop shell, because the restore path deliberately differs from
//! browser/remote mode: browser sessions survive page closure and reattach, desktop processes do not.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => {
  const s = (id: string) => ({
    id,
    projectId: "p1",
    groupId: null,
    name: id,
    kind: "claude",
    shell: null,
    cwd: "/tmp",
    envJson: null,
    initCmd: null,
    hotkey: null,
    parentSessionId: null,
    collapsed: false,
    worktreePath: null,
    sortOrder: 0,
    createdAt: 0,
  });
  return {
    listTree: vi.fn().mockResolvedValue({
      projects: [
        { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
      ],
      groups: [],
      sessions: [s("A"), s("B")],
    }),
  };
});
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));
// Present as a desktop shell so the restore path treats sessions as processes that died with the app.
const desktopEnv = {
  kind: "tauri" as const,
  isTauri: true,
  isElectron: false,
  isBrowser: false,
  isRemoteWindow: false,
  hasNativeHost: true,
  isDev: true,
  isMac: true,
};
vi.mock("../platform", () => ({
  env: desktopEnv,
  platform: { env: desktopEnv },
}));

import type { Session } from "../types";

const WORKSPACE_KEY = "vlx-workspace";

function mkSession(id: string): Session {
  return {
    id,
    projectId: "p1",
    groupId: null,
    name: id,
    kind: "claude",
    shell: null,
    cwd: "/tmp",
    envJson: null,
    initCmd: null,
    hotkey: null,
    parentSessionId: null,
    collapsed: false,
    worktreePath: null,
    sortOrder: 0,
    createdAt: 0,
  };
}

/**
 * Load a fresh copy of the store module. The first-load-only restore is guarded by module-level state, so each
 * test needs its own module instance to exercise startup behavior.
 */
async function freshStore() {
  vi.resetModules();
  const mod = await import("./termStore");
  return mod.useTermStore;
}

/** Seed two open tabs, A active, so a snapshot has something meaningful to capture. */
function seedTwoTabs(store: Awaited<ReturnType<typeof freshStore>>) {
  store.setState({
    projects: [
      { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
    ],
    groups: [],
    sessions: [mkSession("A"), mkSession("B")],
    runtimes: { A: { status: "running" }, B: { status: "running" } },
    epochs: {},
    ephemeralSessions: {},
    dormantSessions: {},
    openTabs: ["A", "B"],
    activeTabId: "A",
    paneTrees: {
      A: { kind: "leaf" as const, paneId: "pa", sessionId: "A" },
      B: { kind: "leaf" as const, paneId: "pb", sessionId: "B" },
    },
    activeSessionId: "A",
    focusedPaneId: "pa",
    liveTabs: [],
    docTabs: {},
    notifications: {},
  });
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  localStorage.clear();
});

describe("saveWorkspaceSnapshot", () => {
  it("writes tabs, trees and active state under its own key", async () => {
    const store = await freshStore();
    seedTwoTabs(store);

    store.getState().saveWorkspaceSnapshot();

    const raw = localStorage.getItem(WORKSPACE_KEY);
    expect(raw).toBeTruthy();
    const saved = JSON.parse(raw!);
    expect(saved.openTabs).toEqual(["A", "B"]);
    expect(saved.activeTabId).toBe("A");
    expect(saved.activeSessionId).toBe("A");
    expect(Object.keys(saved.paneTrees).sort()).toEqual(["A", "B"]);
  });

  it("keeps ephemeral split leaves, which the routine desktop autosave drops", async () => {
    const store = await freshStore();
    seedTwoTabs(store);
    store.setState({
      ephemeralSessions: { "eph-1": mkSession("eph-1") },
      paneTrees: {
        A: {
          kind: "split" as const,
          paneId: "root",
          dir: "horizontal" as const,
          sizes: [50, 50] as [number, number],
          a: { kind: "leaf" as const, paneId: "pa", sessionId: "A" },
          b: { kind: "leaf" as const, paneId: "pe", sessionId: "eph-1" },
        },
        B: { kind: "leaf" as const, paneId: "pb", sessionId: "B" },
      },
    });

    store.getState().saveWorkspaceSnapshot();

    const saved = JSON.parse(localStorage.getItem(WORKSPACE_KEY)!);
    expect(saved.ephemeralSessions).toHaveProperty("eph-1");
    expect(JSON.stringify(saved.paneTrees.A)).toContain("eph-1");
  });
});

describe("restoring a saved workspace on the next launch", () => {
  it("restores tabs and marks every leaf dormant instead of starting processes", async () => {
    const store = await freshStore();
    seedTwoTabs(store);
    store.getState().saveWorkspaceSnapshot();

    // Simulate a relaunch: a brand-new store instance reading the snapshot left behind.
    const relaunched = await freshStore();
    await relaunched.getState().loadTree();

    const s = relaunched.getState();
    expect(s.openTabs).toEqual(["A", "B"]);
    expect(s.activeTabId).toBe("A");
    expect(Object.keys(s.dormantSessions).sort()).toEqual(["A", "B"]);
  });

  it("consumes the snapshot so a later launch starts clean", async () => {
    const store = await freshStore();
    seedTwoTabs(store);
    store.getState().saveWorkspaceSnapshot();

    const second = await freshStore();
    await second.getState().loadTree();
    expect(localStorage.getItem(WORKSPACE_KEY)).toBeNull();

    const third = await freshStore();
    await third.getState().loadTree();
    expect(third.getState().openTabs).toEqual([]);
    expect(third.getState().dormantSessions).toEqual({});
  });

  it("starts clean when the user did not save a workspace", async () => {
    const store = await freshStore();
    await store.getState().loadTree();
    expect(store.getState().openTabs).toEqual([]);
    expect(store.getState().dormantSessions).toEqual({});
  });

  it("ignores a corrupt snapshot rather than failing to start", async () => {
    localStorage.setItem(WORKSPACE_KEY, "{not json");
    const store = await freshStore();
    await store.getState().loadTree();
    expect(store.getState().openTabs).toEqual([]);
    expect(localStorage.getItem(WORKSPACE_KEY)).toBeNull();
  });
});

describe("waking a dormant session", () => {
  it("clears the mark and bumps the epoch so the terminal remounts", async () => {
    const store = await freshStore();
    seedTwoTabs(store);
    store.setState({ dormantSessions: { A: true, B: true } });

    store.getState().wakeSession("A");

    expect(store.getState().dormantSessions).toEqual({ B: true });
    expect(store.getState().epochs.A).toBe(1);
  });

  it("leaves a session that was never dormant untouched", async () => {
    const store = await freshStore();
    seedTwoTabs(store);

    store.getState().wakeSession("A");

    expect(store.getState().epochs.A).toBeUndefined();
  });

  it("drops a stale dormant mark when the session is opened explicitly", async () => {
    const store = await freshStore();
    seedTwoTabs(store);
    store.setState({ dormantSessions: { A: true } });

    store.getState().openSession("A");

    expect(store.getState().dormantSessions).toEqual({});
  });
});
