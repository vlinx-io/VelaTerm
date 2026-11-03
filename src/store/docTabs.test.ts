//! DocTab state-model tests: open/focus/deduplication, protection from session replacement, survival
//! across tree reconciliation, close-confirmation routing, and active-state fallback after closure.
//!
//! Stub Tauri-touching IPC/notification modules so the store loads under Node/jsdom, as in keepAlive.test.tsx.

import { afterEach, describe, expect, it, vi } from "vitest";

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
// Provide notify's full export surface: platform adapters indirectly imported by termStore reference
// every permission helper, so stubbing notify() alone causes missing exports at load time.
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import type { Session } from "../types";
import { useTermStore } from "./termStore";

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

/** Reset to project p1, named sessions A/B, an empty center pane, and single-tab mode. */
function seed() {
  useTermStore.setState({
    projects: [
      { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
    ],
    groups: [],
    sessions: [mkSession("A"), mkSession("B")],
    runtimes: { A: { status: "idle" }, B: { status: "idle" } },
    epochs: {},
    ephemeralSessions: {},
    pendingPrompts: {},
    openTabs: [],
    activeTabId: null,
    paneTrees: {},
    activeSessionId: null,
    focusedPaneId: null,
    liveTabs: [],
    docTabs: {},
    notifications: {},
    singleTabMode: true,
  });
}

/** Return the only document-tab ID, throwing on multiples to make failures visible. */
function soleDocId(): string {
  const ids = Object.keys(useTermStore.getState().docTabs);
  expect(ids.length).toBe(1);
  return ids[0];
}

afterEach(() => {
  localStorage.clear();
});

describe("openDiffTab", () => {
  it("keeps a file's diff tab apart from its editor tab and refreshes it when opened again", () => {
    seed();
    const store = useTermStore.getState();
    store.openDiffTab("/repo/src/a.ts", "/repo", "src/a.ts");
    store.openDocTab("/repo/src/a.ts");
    const tabs = Object.values(useTermStore.getState().docTabs);
    expect(tabs.map((tab) => tab.kind).sort()).toEqual(["code", "diff"]);
    const diff = tabs.find((tab) => tab.kind === "diff")!;
    expect(diff.diff).toEqual({ root: "/repo", path: "src/a.ts" });
    store.openDiffTab("/repo/src/a.ts", "/repo", "src/a.ts");
    const s = useTermStore.getState();
    expect(Object.keys(s.docTabs)).toHaveLength(2);
    expect(s.activeTabId).toBe(diff.id);
    expect(s.docTabs[diff.id].reloadNonce).toBe(1);
  });
});

describe("openChanges", () => {
  it("keeps one tab per comparison and selects the requested file in it", () => {
    seed();
    const store = useTermStore.getState();
    store.openChanges("/repo", { side: "staged", path: "a.ts" });
    store.openChanges("/repo", { commit: "abc" });
    expect(Object.keys(useTermStore.getState().docTabs)).toHaveLength(2);
    store.openChanges("/repo", { side: "staged", path: "b.ts" });
    const s = useTermStore.getState();
    const staged = Object.values(s.docTabs).filter((tab) => tab.changes?.side === "staged" && !tab.changes.commit);
    expect(staged).toHaveLength(1);
    expect(s.activeTabId).toBe(staged[0].id);
    expect(staged[0].changes).toMatchObject({ cwd: "/repo", path: "b.ts" });
    expect(staged[0].title).toBe("repo");
    // Opening it again without a file keeps the one picked inside the tab.
    store.selectChangesFile(staged[0].id, "c.ts");
    store.openChanges("/repo", { side: "staged" });
    expect(useTermStore.getState().docTabs[staged[0].id].changes?.path).toBe("c.ts");
  });
});

describe("openDocTab: creating, focusing and deduplicating", () => {
  it("a new document tab is appended to openTabs and activated, with activeSessionId null", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/notes.md");

    const s = useTermStore.getState();
    const id = soleDocId();
    expect(id.startsWith("doc-")).toBe(true);
    expect(s.openTabs).toContain(id);
    expect(s.activeTabId).toBe(id);
    expect(s.activeSessionId).toBeNull();
    expect(s.focusedPaneId).toBeNull();
    const tab = s.docTabs[id];
    expect(tab.path).toBe("/tmp/notes.md");
    expect(tab.title).toBe("notes.md");
    expect(tab.mode).toBe("visual"); // Markdown opens in reading mode.
    expect(tab.dirty).toBe(false);
  });

  it("a non-markdown extension opens in source mode", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/config.toml");
    expect(useTermStore.getState().docTabs[soleDocId()].mode).toBe("source");
  });

  it("reopening the same path focuses the existing tab instead of creating one", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/a.md");
    const first = soleDocId();
    // Switch to a session, then view the same file and refocus its original tab.
    useTermStore.getState().openSession("A");
    expect(useTermStore.getState().activeTabId).toBe("A");

    useTermStore.getState().openDocTab("/tmp/a.md");
    const s = useTermStore.getState();
    expect(Object.keys(s.docTabs).length).toBe(1);
    expect(s.activeTabId).toBe(first);
    expect(s.activeSessionId).toBeNull();
  });

  it("reopening the same path increments reloadNonce, forcing DocView to re-read from disk", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/a.md");
    const id = soleDocId();
    expect(useTermStore.getState().docTabs[id].reloadNonce).toBe(0);

    useTermStore.getState().openDocTab("/tmp/a.md");
    expect(useTermStore.getState().docTabs[id].reloadNonce).toBe(1);
    useTermStore.getState().openDocTab("/tmp/a.md");
    expect(useTermStore.getState().docTabs[id].reloadNonce).toBe(2);
  });
});

describe("refreshDocTab: the Refresh file action in the tab context menu", () => {
  it("increments reloadNonce and leaves the other metadata alone", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/r.md");
    const id = soleDocId();

    useTermStore.getState().refreshDocTab(id);
    const tab = useTermStore.getState().docTabs[id];
    expect(tab.reloadNonce).toBe(1);
    expect(tab.path).toBe("/tmp/r.md");
    expect(tab.dirty).toBe(false);
  });

  it("a tab id that does not exist is a safe no-op", () => {
    seed();
    useTermStore.getState().refreshDocTab("doc-missing");
    expect(Object.keys(useTermStore.getState().docTabs).length).toBe(0);
  });
});

describe("makeDocTab classifies by extension (kind plus initial mode)", () => {
  const table: Array<[string, string, string]> = [
    ["/tmp/a.md", "markdown", "visual"],
    ["/tmp/b.markdown", "markdown", "visual"],
    ["/tmp/c.mdx", "markdown", "visual"],
    ["/tmp/b.py", "code", "source"],
    ["/tmp/c.rs", "code", "source"],
    ["/tmp/no-extension", "code", "source"],
    ["/tmp/x.unknown", "code", "source"],
    ["/tmp/d.png", "image", "source"],
    ["/tmp/D.PNG", "image", "source"], // Extensions are case-insensitive.
    ["/tmp/e.svg", "image", "source"],
    ["/tmp/f.jpeg", "image", "source"],
    ["/tmp/g.gif", "image", "source"],
  ];
  for (const [path, kind, mode] of table) {
    it(`${path} → ${kind} + ${mode}`, () => {
      seed();
      useTermStore.getState().openDocTab(path);
      const tab = useTermStore.getState().docTabs[soleDocId()];
      expect(tab.kind).toBe(kind);
      expect(tab.mode).toBe(mode);
      expect(tab.dirty).toBe(false);
    });
  }
});

describe("core regression: a session never overwrites a document tab", () => {
  it("openSession in single-tab mode while a document tab is active opens the session in a new tab and leaves the document tab untouched", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/design.md");
    const docId = soleDocId();

    useTermStore.getState().openSession("A");

    const s = useTermStore.getState();
    // The document tab and its metadata remain intact.
    expect(s.openTabs).toContain(docId);
    expect(s.docTabs[docId]).toBeTruthy();
    // The session opens and activates in its own new tab.
    expect(s.openTabs).toContain("A");
    expect(s.activeTabId).toBe("A");
    expect(s.activeSessionId).toBe("A");
    // A document tab is not moved into terminal keep-alive state.
    expect(s.liveTabs).not.toContain(docId);
  });

  it("openSession with an existing session anchor reuses the most recent session tab slot instead of opening a third one", () => {
    seed();
    // Activate A as the most recent session tab, then switch to the document.
    useTermStore.getState().openSession("A");
    useTermStore.getState().openDocTab("/tmp/design.md");
    const docId = soleDocId();
    expect(useTermStore.getState().activeTabId).toBe(docId);

    // Opening B from the document reuses A's slot and keeps A alive instead of creating a third tab.
    useTermStore.getState().openSession("B");

    const s = useTermStore.getState();
    expect(s.activeTabId).toBe("B");
    expect(s.openTabs).toContain("B");
    expect(s.openTabs).toContain(docId); // The document stays in place.
    expect(s.openTabs).not.toContain("A"); // A yields its slot.
    expect(s.liveTabs).toContain("A"); // Named session A remains alive in the background.
    expect(s.openTabs.length).toBe(2); // Only the document and B remain visible.
    expect(s.lastActiveSessionTabId).toBe("B");
  });

  it("switching from a document tab to a session and back keeps both states correct", () => {
    seed();
    useTermStore.getState().openSession("A");
    useTermStore.getState().openDocTab("/tmp/x.md");
    const docId = soleDocId();

    useTermStore.getState().setActiveTab("A");
    expect(useTermStore.getState().activeSessionId).toBe("A");

    useTermStore.getState().setActiveTab(docId);
    const s = useTermStore.getState();
    expect(s.activeTabId).toBe(docId);
    expect(s.activeSessionId).toBeNull();
    expect(s.focusedPaneId).toBeNull();
  });
});

describe("a tree refresh (loadTree reconciliation) does not lose document tabs", () => {
  it("a later loadTree keeps document tabs, and deleting a session does not affect them", async () => {
    seed();
    // The first loadTree follows startup restoration; empty localStorage only flips the restored flag.
    await useTermStore.getState().loadTree();

    useTermStore.getState().openDocTab("/tmp/keep.md");
    const docId = soleDocId();
    useTermStore.getState().openSession("A");

    // Simulate a tree-change refresh while mocked listTree still returns A/B.
    await useTermStore.getState().loadTree();
    let s = useTermStore.getState();
    expect(s.openTabs).toContain(docId);
    expect(s.docTabs[docId]).toBeTruthy();

    // Simulate A being deleted elsewhere; reconciliation removes A but preserves the document tab.
    const { listTree } = await import("../ipc/tree");
    vi.mocked(listTree).mockResolvedValueOnce({
      projects: [
        { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
      ],
      groups: [],
      sessions: [mkSession("B")],
    });
    await useTermStore.getState().loadTree();
    s = useTermStore.getState();
    expect(s.openTabs).not.toContain("A");
    expect(s.openTabs).toContain(docId);
    expect(s.docTabs[docId]).toBeTruthy();
  });
});

describe("requestCloseDocTab: routing the close confirmation", () => {
  it("a clean tab closes straight away", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/c.md");
    const docId = soleDocId();

    useTermStore.getState().requestCloseDocTab(docId);
    const s = useTermStore.getState();
    expect(s.openTabs).not.toContain(docId);
    expect(s.docTabs[docId]).toBeUndefined();
  });

  it("a dirty tab sets pendingClose without closing, and cancel restores it", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/d.md");
    const docId = soleDocId();
    useTermStore.getState().setDocTabDirty(docId, true);

    useTermStore.getState().requestCloseDocTab(docId);
    let s = useTermStore.getState();
    expect(s.openTabs).toContain(docId); // Still open.
    expect(s.docTabs[docId].pendingClose).toBe(true);

    useTermStore.getState().cancelCloseDocTab(docId);
    s = useTermStore.getState();
    expect(s.docTabs[docId].pendingClose).toBe(false);
    expect(s.openTabs).toContain(docId);
  });

  it("requesting a close after saving, once the dirty flag is cleared, closes straight away", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/e.md");
    const docId = soleDocId();
    useTermStore.getState().setDocTabDirty(docId, true);
    useTermStore.getState().setDocTabDirty(docId, false); // Simulate a successful save.

    useTermStore.getState().requestCloseDocTab(docId);
    expect(useTermStore.getState().openTabs).not.toContain(docId);
  });
});

describe("which tab becomes active after closeTab closes a document tab", () => {
  it("falling back to an adjacent terminal tab restores the active session", () => {
    seed();
    useTermStore.getState().openSession("A");
    useTermStore.getState().openDocTab("/tmp/f.md");
    const docId = soleDocId();
    expect(useTermStore.getState().activeTabId).toBe(docId);

    useTermStore.getState().closeTab(docId);
    const s = useTermStore.getState();
    expect(s.activeTabId).toBe("A");
    expect(s.activeSessionId).toBe("A");
    expect(s.docTabs[docId]).toBeUndefined();
  });

  it("falling back to an adjacent document tab keeps activeSessionId null", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/g1.md");
    useTermStore.getState().openDocTab("/tmp/g2.md");
    const s0 = useTermStore.getState();
    const ids = s0.openTabs.filter((t) => s0.docTabs[t]);
    expect(ids.length).toBe(2);
    const [first, second] = ids;
    expect(s0.activeTabId).toBe(second);

    useTermStore.getState().closeTab(second);
    const s = useTermStore.getState();
    expect(s.activeTabId).toBe(first);
    expect(s.activeSessionId).toBeNull();
    expect(s.focusedPaneId).toBeNull();
    expect(s.docTabs[first]).toBeTruthy();
  });

  it("closing the last tab returns to the empty state", () => {
    seed();
    useTermStore.getState().openDocTab("/tmp/h.md");
    const docId = soleDocId();
    useTermStore.getState().closeTab(docId);
    const s = useTermStore.getState();
    expect(s.openTabs.length).toBe(0);
    expect(s.activeTabId).toBeNull();
    expect(s.activeSessionId).toBeNull();
    expect(Object.keys(s.docTabs).length).toBe(0);
  });
});
