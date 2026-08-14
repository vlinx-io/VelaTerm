
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn().mockResolvedValue(undefined),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  spawnResult: vi.fn().mockResolvedValue(true),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ({
  createSession: vi.fn(),
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

import { SETTINGS_KEY, loadSettings, saveSettings } from "./settings";
import { useTermStore } from "./termStore";

if (!window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

const DEFAULT_LIMITS = {
  maxChildren: 10,
  maxParallel: 4,
  maxDepth: 2,
  requireConfirmationAbove: 6,
  autoApprove: false,
  defaultTimeoutSecs: 1800,
  worktreeCopyPatterns: ["docs/plans/**"],
};

beforeEach(() => {
  localStorage.clear();
});

describe("orchestration settings persistence", () => {
  it("preserves profiles and limits across a save and reload", () => {
    const saved = loadSettings();
    saved.orchestrationProfiles = {
      reviewer: {
        description: "Use for focused code review and defect analysis.",
        agent: "codex",
        model: "gpt-5.6-luna",
        effort: "xhigh",
        worktree: false,
        permissionMode: "skip",
      },
    };
    saved.orchestration = {
      maxChildren: 3,
      maxParallel: 2,
      maxDepth: 1,
      requireConfirmationAbove: 2,
      autoApprove: false,
      defaultTimeoutSecs: 600,
      worktreeCopyPatterns: ["docs/plans/**", ".env.local"],
    };
    saveSettings(saved);

    const reloaded = loadSettings();
    expect(reloaded.orchestrationProfiles).toEqual(saved.orchestrationProfiles);
    expect(reloaded.orchestration).toEqual(saved.orchestration);
  });

  it("loads the full defaults from a blob written before orchestration existed", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ accent: "auto", termFontSize: 15 }));

    const loaded = loadSettings();
    expect(loaded.termFontSize).toBe(15);
    expect(loaded.orchestration).toEqual(DEFAULT_LIMITS);
    expect(Object.keys(loaded.orchestrationProfiles).sort()).toEqual([
      "database",
      "frontend",
      "quick-edits",
      "tests",
    ]);
    expect(loaded.orchestrationProfiles.database).toEqual({
      description:
        "Use for database schemas, migrations, queries, indexes, persistence, and data access.",
      agent: "claude",
      model: "fable",
      effort: "high",
      worktree: true,
      permissionMode: "default",
    });
    expect(loaded.orchestrationProfiles.frontend.description).toBe(
      "Use for UI components, routes, styling, responsive behavior, and browser interactions.",
    );
    expect(loaded.orchestrationProfiles.frontend.model).toBe("opus");
    expect(loaded.orchestrationProfiles.tests.description).toBe(
      "Use for focused unit, integration, regression, and end-to-end tests.",
    );
    expect(loaded.orchestrationProfiles["quick-edits"]).toEqual({
      description:
        "Use for simple, well-scoped updates such as find-and-replace changes, small configuration edits, text revisions, and other mechanical changes.",
      agent: "codex",
      model: "gpt-5.6-luna",
      effort: "xhigh",
      worktree: true,
      permissionMode: "default",
    });
    expect(
      Object.values(loaded.orchestrationProfiles).every(
        (profile) => profile.permissionMode === "default",
      ),
    ).toBe(true);
  });

  it("fills the missing numeric fields of a partial orchestration object", () => {
    localStorage.setItem(
      SETTINGS_KEY,
      JSON.stringify({ orchestration: { maxChildren: 25, maxDepth: 4 } }),
    );

    const loaded = loadSettings();
    expect(loaded.orchestration).toEqual({
      ...DEFAULT_LIMITS,
      maxChildren: 25,
      maxDepth: 4,
    });
    for (const value of Object.values(loaded.orchestration)) {
      expect(value).toBeDefined();
    }
  });

  it("falls back to the default profiles when the stored map is not an object", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ orchestrationProfiles: null }));
    expect(Object.keys(loadSettings().orchestrationProfiles)).toHaveLength(4);
  });

  it("normalizes missing and invalid profile permission modes to default", () => {
    localStorage.setItem(
      SETTINGS_KEY,
      JSON.stringify({
        orchestrationProfiles: {
          missing: { agent: "claude" },
          invalid: { agent: "codex", permissionMode: "yolo" },
          skipped: { agent: "codex", permissionMode: "skip" },
        },
      }),
    );

    expect(loadSettings().orchestrationProfiles).toEqual({
      missing: { agent: "claude", permissionMode: "default" },
      invalid: { agent: "codex", permissionMode: "default" },
      skipped: { agent: "codex", permissionMode: "skip" },
    });
  });
});

describe("orchestration store setters", () => {
  beforeEach(() => {
    useTermStore.setState({
      orchestrationProfiles: {
        database: {
          description: "Use for database work.",
          agent: "claude",
          model: "fable",
          effort: "high",
          worktree: true,
          permissionMode: "skip",
        },
      },
      orchestration: { ...DEFAULT_LIMITS, worktreeCopyPatterns: ["docs/plans/**"] },
    });
  });

  it("merges a patch into an existing profile without dropping its other fields", () => {
    useTermStore.getState().setOrchestrationProfile("database", { model: "opus" });
    expect(useTermStore.getState().orchestrationProfiles.database).toEqual({
      description: "Use for database work.",
      agent: "claude",
      model: "opus",
      effort: "high",
      worktree: true,
      permissionMode: "skip",
    });
  });

  it("adds a new profile and deletes one with a null patch", () => {
    useTermStore.getState().setOrchestrationProfile("docs", { agent: "codex" });
    expect(useTermStore.getState().orchestrationProfiles.docs).toEqual({
      agent: "codex",
      permissionMode: "default",
    });

    useTermStore.getState().setOrchestrationProfile("database", null);
    expect(useTermStore.getState().orchestrationProfiles.database).toBeUndefined();
    expect(Object.keys(useTermStore.getState().orchestrationProfiles)).toEqual(["docs"]);
  });

  it("merges a partial limits patch and persists it", () => {
    useTermStore.getState().setOrchestrationLimits({ maxParallel: 8 });
    expect(useTermStore.getState().orchestration).toEqual({
      ...DEFAULT_LIMITS,
      maxParallel: 8,
    });
    expect(loadSettings().orchestration.maxParallel).toBe(8);
  });
});

describe("forced spawn confirmation", () => {
  const executeSpawn = vi.fn().mockResolvedValue(undefined);

  beforeEach(() => {
    executeSpawn.mockClear();
    useTermStore.setState({
      spawnConfirm: false,
      notifyEnabled: false,
      pendingSpawns: [],
      sessions: [],
      executeSpawn,
    });
  });

  it("queues a request that crossed the backend threshold even with confirmation off", async () => {
    await useTermStore.getState().handleSpawnRequest({
      parentSessionId: "parent-1",
      prompt: "seventh child",
      forceConfirm: true,
    });
    expect(executeSpawn).not.toHaveBeenCalled();
    expect(useTermStore.getState().pendingSpawns).toHaveLength(1);
  });

  it("still executes immediately without the flag", async () => {
    await useTermStore.getState().handleSpawnRequest({
      parentSessionId: "parent-1",
      prompt: "second child",
    });
    expect(executeSpawn).toHaveBeenCalledTimes(1);
    expect(useTermStore.getState().pendingSpawns).toHaveLength(0);
  });

  it("auto-executes an orchestrated request when its setting is enabled", async () => {
    useTermStore.setState({ spawnConfirm: true });
    await useTermStore.getState().handleSpawnRequest({
      parentSessionId: "parent-1",
      prompt: "profiled child",
      autoApprove: true,
    });
    expect(executeSpawn).toHaveBeenCalledTimes(1);
    expect(useTermStore.getState().pendingSpawns).toHaveLength(0);
  });

  it("keeps the confirmation card when the orchestration threshold forces it", async () => {
    await useTermStore.getState().handleSpawnRequest({
      parentSessionId: "parent-1",
      prompt: "threshold child",
      autoApprove: true,
      forceConfirm: true,
    });
    expect(executeSpawn).not.toHaveBeenCalled();
    expect(useTermStore.getState().pendingSpawns).toHaveLength(1);
  });
});
