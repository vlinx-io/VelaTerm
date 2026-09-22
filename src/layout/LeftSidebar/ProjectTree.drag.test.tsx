// Drag-and-drop coverage for the sidebar tree: projects reorder among themselves through move_node, while sessions
// and groups dropped on a project header still move into that project's root.
import { cleanup, fireEvent, render, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const shareMock = vi.hoisted(() => ({ isShareSurface: false, shareBasePath: "" }));

const storeState = vi.hoisted(() => ({
  projects: [
    // Deliberately not in sortOrder order: the drop handler must sort before computing the neighbours.
    { id: "pc", name: "Gamma", rootPath: "/tmp/c", sortOrder: 3000, collapsed: false, createdAt: 0 },
    { id: "pa", name: "Alpha", rootPath: "/tmp/a", sortOrder: 1000, collapsed: true, createdAt: 0 },
    { id: "pb", name: "Beta", rootPath: "/tmp/b", sortOrder: 2000, collapsed: true, createdAt: 0 },
  ],
  groups: [
    { id: "g1", projectId: "pc", parentGroupId: null, name: "Tools", sortOrder: 0, collapsed: true, createdAt: 0 },
  ],
  sessions: [
    {
      id: "s1",
      projectId: "pc",
      groupId: null,
      parentSessionId: null,
      name: "Shell",
      kind: "terminal",
      sortOrder: 0,
      collapsed: false,
      createdAt: 0,
    },
  ],
  treeLoaded: true,
  ephemeralSessions: {},
  notifications: {},
  runtimes: {},
  selection: [] as unknown[],
  selectionAnchor: null,
  activeSessionId: null,
  revealProjectId: null,
  revealSuppressId: null,
  density: "regular",
  navLayout: "full",
  shortcutOverrides: {},
  moveNode: vi.fn(() => Promise.resolve()),
  moveMany: vi.fn(() => Promise.resolve()),
  toggleCollapsed: vi.fn(),
  openSession: vi.fn(),
  setRevealProject: vi.fn(),
  setRevealSuppress: vi.fn(),
  setCreateProjectModalOpen: vi.fn(),
  importProject: vi.fn(),
  setCloneModalOpen: vi.fn(),
  selectSingle: vi.fn(),
  toggleSelect: vi.fn(),
  setSelection: vi.fn(),
  setInspectTarget: vi.fn(),
  setSidebarTreeViewCollapsed: vi.fn(),
}));

vi.mock("../../i18n", () => ({ t: (key: string) => key, useT: () => (key: string) => key }));
vi.mock("../../ipc/shareBase", () => shareMock);
vi.mock("../../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  );
  return { useTermStore, isVisibleSession: () => false };
});
vi.mock("../../hooks/useGitBranch", () => ({
  useGitBranch: () => null,
  isWorktreeGone: () => false,
  peekGitBranchInfo: () => ({ isRepo: false }),
  prefetchGitBranchInfo: vi.fn(),
}));
vi.mock("../../components/Icons", () => ({
  default: new Proxy({}, { get: () => () => null }),
}));
vi.mock("../../components/StatusIndicator", () => ({ StatusIndicator: () => null }));
vi.mock("../sessionViewers/sessionMeta", () => ({ SessionKindIcon: () => null }));
// jsdom has no layout, so render every row instead of the virtualized window.
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({ key: index, index, start: index * 28 })),
    getTotalSize: () => count * 28,
    measureElement: () => {},
    measure: () => {},
    scrollToIndex: () => {},
  }),
}));

import { ProjectTree, type TreeHandlers } from "./ProjectTree";

const handlers: TreeHandlers = {
  view: {
    id: "main",
    name: "Main",
    treeFilter: "",
    statusFilter: null,
    statusFilterIds: null,
    markFilter: null,
    collapsedOverrides: null,
  },
  isPrimary: true,
  onContext: vi.fn(),
  contextId: null,
  renamingId: null,
  renameVal: "",
  setRenameVal: vi.fn(),
  commitRename: vi.fn(),
  cancelRename: vi.fn(),
  onAddSession: vi.fn(),
  onAddGroup: vi.fn(),
};

/** Minimal DataTransfer stand-in: jsdom ships none, and the tree reads text/plain back at drop time. */
function makeDataTransfer() {
  const data = new Map<string, string>();
  return {
    effectAllowed: "uninitialized",
    dropEffect: "none",
    setData: (type: string, value: string) => void data.set(type, value),
    getData: (type: string) => data.get(type) ?? "",
    setDragImage: () => {},
  };
}

const ROW_HEIGHT = 28;

// jsdom ships no DragEvent, and testing-library then falls back to a plain Event that drops clientY. Give it a
// MouseEvent-based stand-in so drag events carry pointer coordinates for the zone calculation.
class FakeDragEvent extends MouseEvent {
  dataTransfer: unknown;
  constructor(type: string, init?: MouseEventInit & { dataTransfer?: unknown }) {
    super(type, init);
    this.dataTransfer = init?.dataTransfer ?? null;
  }
}
for (const target of [globalThis, document.defaultView] as unknown as Record<string, unknown>[]) {
  if (target && !("DragEvent" in target)) target.DragEvent = FakeDragEvent;
}

function rowOf(name: string): HTMLElement {
  // Scope to the tree: dragstart parks a row clone under document.body as the drag image until the next tick.
  const tree = document.querySelector(".tree");
  if (!tree) throw new Error("tree not rendered");
  const row = within(tree as HTMLElement).getByText(name).closest(".row") as HTMLElement | null;
  if (!row) throw new Error(`row ${name} not rendered`);
  // Zones are computed from the row rect, which jsdom reports as all zeros.
  vi.spyOn(row, "getBoundingClientRect").mockReturnValue({
    top: 0, left: 0, bottom: ROW_HEIGHT, right: 200, height: ROW_HEIGHT, width: 200, x: 0, y: 0,
    toJSON: () => ({}),
  });
  return row;
}

/** Drag `source` and drop it on `target` at the given vertical offset inside the row (top, center or bottom zone).
 *  Reports which indicator the target showed while hovering: an edge line (reorder) or the center outline (drop into).
 *  `skipOver` drops without a preceding dragover, so no zone was registered for the target. */
function dragTo(source: HTMLElement, target: HTMLElement, clientY: number, skipOver = false) {
  const dataTransfer = makeDataTransfer();
  fireEvent.dragStart(source, { dataTransfer });
  if (!skipOver) fireEvent.dragOver(target, { dataTransfer, clientY });
  const edge = target.style.borderTop !== "" || target.style.borderBottom !== "";
  const center = target.style.outline !== "";
  fireEvent.drop(target, { dataTransfer, clientY });
  fireEvent.dragEnd(source, { dataTransfer });
  return { edge, center, highlighted: edge || center };
}

describe("ProjectTree drag and drop", () => {
  beforeEach(() => {
    storeState.moveNode.mockClear();
    storeState.moveMany.mockClear();
    shareMock.isShareSurface = false;
  });
  afterEach(cleanup);

  it("makes project rows draggable, but not on the share surface", () => {
    render(<ProjectTree {...handlers} />);
    expect(rowOf("Alpha").getAttribute("draggable")).toBe("true");
    cleanup();

    shareMock.isShareSurface = true;
    render(<ProjectTree {...handlers} />);
    expect(rowOf("Alpha").getAttribute("draggable")).toBe("false");
  });

  it("reorders a project dropped on the top zone of another project", () => {
    render(<ProjectTree {...handlers} />);
    const alpha = rowOf("Alpha");
    const gamma = rowOf("Gamma");

    const { highlighted } = dragTo(alpha, gamma, 2);

    expect(highlighted).toBe(true);
    // Before Gamma: floor((2000 + 3000) / 2), no parent columns for a project.
    expect(storeState.moveNode).toHaveBeenCalledTimes(1);
    expect(storeState.moveNode).toHaveBeenCalledWith("project", "pa", null, null, null, 2500);
  });

  it("drops a project below the last project with a fresh order", () => {
    render(<ProjectTree {...handlers} />);
    const now = 1_700_000_000_000;
    vi.spyOn(Date, "now").mockReturnValue(now);

    dragTo(rowOf("Alpha"), rowOf("Gamma"), ROW_HEIGHT - 2);

    expect(storeState.moveNode).toHaveBeenCalledWith("project", "pa", null, null, null, now);
    vi.restoreAllMocks();
  });

  it("never offers a dragged project a center zone and ignores drops on itself or on other row kinds", () => {
    render(<ProjectTree {...handlers} />);
    const alpha = rowOf("Alpha");
    const gamma = rowOf("Gamma");
    const shell = rowOf("Shell");

    // A project row offers a dragged project no "drop into" zone: mid-row shows an edge line, never the center
    // outline, and the drop lands beside the target instead of inside it.
    const mid = dragTo(alpha, gamma, ROW_HEIGHT / 2);
    expect(mid.center).toBe(false);
    expect(mid.edge).toBe(true);
    expect(storeState.moveNode).toHaveBeenCalledWith("project", "pa", null, null, null, expect.any(Number));
    storeState.moveNode.mockClear();

    // Without a registered zone the drop falls back to "center", which is a no-op for a project payload.
    expect(dragTo(alpha, gamma, ROW_HEIGHT / 2, true).highlighted).toBe(false);
    // Onto itself.
    expect(dragTo(alpha, alpha, 2).highlighted).toBe(false);
    // Onto a session row.
    expect(dragTo(alpha, shell, 2).highlighted).toBe(false);
    // Onto a group row inside the dragged project itself: without the project guard in dropOnGroup the payload
    // would pass the same-project check and be moved as if it were a group.
    expect(dragTo(gamma, rowOf("Tools"), ROW_HEIGHT / 2).highlighted).toBe(false);
    expect(dragTo(gamma, rowOf("Tools"), 2).highlighted).toBe(false);

    expect(storeState.moveNode).not.toHaveBeenCalled();
    expect(storeState.moveMany).not.toHaveBeenCalled();
  });

  it("still moves a session dropped on a project header into that project's root", () => {
    render(<ProjectTree {...handlers} />);
    const shell = rowOf("Shell");
    const alpha = rowOf("Alpha");

    // Center zone: the "drop into" indicator stays for sessions.
    expect(dragTo(shell, alpha, ROW_HEIGHT / 2).center).toBe(true);
    expect(storeState.moveNode).toHaveBeenCalledWith("session", "s1", "pa", null, null, expect.any(Number));

    // Edge zone: also into the root, never a reorder among projects.
    storeState.moveNode.mockClear();
    dragTo(shell, rowOf("Beta"), 2);
    expect(storeState.moveNode).toHaveBeenCalledWith("session", "s1", "pb", null, null, expect.any(Number));
  });
});
