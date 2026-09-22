//! Rendered row order of the sidebar tree with "Sort by activity" off (the manual order, exactly as the store
//! delivers it) and on (most recently active first at every level), plus the drag gate that refuses edge drops
//! while the mode is on. The virtualizer is replaced so every row renders in jsdom.

import { fireEvent, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Group, Project, Session } from "../../types";

const project = (id: string, sortOrder: number): Project =>
  ({ id, name: id, rootPath: `/tmp/${id}`, sortOrder, collapsed: false, createdAt: 0 }) as Project;
const group = (id: string, projectId: string, sortOrder: number): Group =>
  ({ id, projectId, parentGroupId: null, name: id, sortOrder, collapsed: false, createdAt: 0 }) as Group;
const session = (id: string, projectId: string, sortOrder: number, groupId: string | null = null): Session =>
  ({ id, projectId, groupId, name: id, kind: "terminal", sortOrder, collapsed: false, createdAt: 0 }) as Session;

// The store delivers every array in manual sort_order, exactly like list_tree.
const storeState = vi.hoisted(() => ({
  projects: [] as Project[],
  groups: [] as Group[],
  sessions: [] as Session[],
  treeLoaded: true,
  ephemeralSessions: {},
  runtimes: {},
  notifications: {},
  browserTabs: {},
  docTabs: {},
  taskTabs: {},
  openTabs: [],
  paneTrees: {},
  liveTabs: [],
  activeTabId: null,
  activeSessionId: null,
  revealProjectId: null,
  selection: [],
  selectionAnchor: null,
  shortcutOverrides: {},
  density: "regular",
  navLayout: "tree",
  agentPresets: [],
  sortByActivity: false,
  sessionActivity: {} as Record<string, number>,
  toggleCollapsed: vi.fn(),
  openSession: vi.fn(),
  setRevealProject: vi.fn(),
  moveNode: vi.fn(),
  moveMany: vi.fn(),
  selectSingle: vi.fn(),
  toggleSelect: vi.fn(),
  setSelection: vi.fn(),
  setInspectTarget: vi.fn(),
  setSidebarTreeViewCollapsed: vi.fn(),
  setCreateProjectModalOpen: vi.fn(),
  importProject: vi.fn(),
  setCloneModalOpen: vi.fn(),
}));

vi.mock("../../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  );
  return { useTermStore, isVisibleSession: () => false };
});
vi.mock("../../i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("../../components/Icons", () => ({ default: new Proxy({}, { get: () => () => null }) }));
vi.mock("../../components/StatusIndicator", () => ({ StatusIndicator: () => null }));
vi.mock("../sessionViewers/sessionMeta", () => ({ SessionKindIcon: () => null }));
vi.mock("../../hooks/useGitBranch", () => ({ useGitBranch: () => null, isWorktreeGone: () => false }));
vi.mock("../../sharing/sessionNavigation", () => ({ navigateSharedSession: vi.fn(), sharedSessionUrl: () => "" }));
// jsdom has no layout, so the real virtualizer would render nothing; this stand-in renders every row.
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count, getItemKey }: { count: number; getItemKey?: (i: number) => string | number }) => ({
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({ index, key: getItemKey?.(index) ?? index, start: index * 28, size: 28 })),
    getTotalSize: () => count * 28,
    measureElement: () => {},
    measure: () => {},
    scrollToIndex: () => {},
  }),
}));

import { ProjectTree } from "./ProjectTree";

const view = {
  id: "main",
  name: "Main",
  treeFilter: "",
  statusFilter: null,
  statusFilterIds: null,
  markFilter: null,
  collapsedOverrides: null,
};

function renderTree() {
  return render(
    <ProjectTree
      view={view}
      isPrimary
      onContext={vi.fn()}
      contextId={null}
      renamingId={null}
      renameVal=""
      setRenameVal={vi.fn()}
      commitRename={vi.fn()}
      cancelRename={vi.fn()}
      onAddSession={vi.fn()}
      onAddGroup={vi.fn()}
    />,
  );
}

const renderedNames = (container: HTMLElement) =>
  Array.from(container.querySelectorAll(".nm")).map((el) => el.textContent);

describe("ProjectTree activity order", () => {
  beforeEach(() => {
    storeState.projects = [project("p1", 1), project("p2", 2)];
    storeState.groups = [group("g1", "p1", 1), group("g2", "p1", 2)];
    storeState.sessions = [
      session("s1", "p1", 1),
      session("s2", "p1", 2, "g1"),
      session("s3", "p1", 3),
      session("s4", "p2", 1),
      session("s5", "p1", 5, "g2"),
    ];
    storeState.sortByActivity = false;
    storeState.sessionActivity = {};
  });

  // Groups render before root sessions inside a project, as the flattening always did.
  const manualOrder = ["p1", "g1", "s2", "g2", "s5", "s1", "s3", "p2", "s4"];

  it("renders the manual order when the mode is off, even with activity recorded", () => {
    storeState.sessionActivity = { s4: 300, s3: 200, s5: 250 };
    const { container } = renderTree();
    expect(renderedNames(container)).toEqual(manualOrder);
  });

  it("renders the most recently active projects, groups and sessions first when the mode is on", () => {
    storeState.sortByActivity = true;
    storeState.sessionActivity = { s4: 300, s3: 200, s5: 250 };
    const { container } = renderTree();
    // p2 (300) before p1 (250); inside p1 the group holding s5 (250) before the inactive g1; root sessions s3
    // (200) before s1 (none). Nothing without activity changes its relative manual position.
    expect(renderedNames(container)).toEqual(["p2", "s4", "p1", "g2", "s5", "g1", "s2", "s3", "s1"]);
  });

  it("renders the manual order when the mode is on but nothing has activity yet", () => {
    storeState.sortByActivity = true;
    const { container } = renderTree();
    expect(renderedNames(container)).toEqual(manualOrder);
  });
});

describe("ProjectTree drag gate", () => {
  beforeEach(() => {
    storeState.projects = [project("p1", 1)];
    storeState.groups = [];
    storeState.sessions = [session("s1", "p1", 1), session("s2", "p1", 2)];
    storeState.sessionActivity = {};
    // jsdom has no DragEvent; testing-library would fall back to a plain Event and drop clientY. A MouseEvent
    // carries the pointer position, and testing-library still attaches the given dataTransfer.
    (window as unknown as { DragEvent: unknown }).DragEvent = window.MouseEvent;
    // Rows are 28 px tall at the top of the viewport, so clientY picks the zone.
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      top: 0, left: 0, bottom: 28, right: 200, width: 200, height: 28, x: 0, y: 0, toJSON: () => ({}),
    } as DOMRect);
  });

  const rowOf = (container: HTMLElement, name: string) =>
    Array.from(container.querySelectorAll(".row.session")).find((el) => el.textContent?.includes(name)) as HTMLElement;
  const dragOver = (row: HTMLElement, clientY: number) => {
    const dataTransfer = { dropEffect: "none", types: ["application/x-vlx-session"] };
    // fireEvent returns false when the handler called preventDefault, which is what lets the drop happen.
    const notPrevented = fireEvent.dragOver(row, { clientY, dataTransfer });
    return { allowed: !notPrevented, dropEffect: dataTransfer.dropEffect };
  };

  it("accepts edge drops in the manual order", () => {
    storeState.sortByActivity = false;
    const { container } = renderTree();
    expect(dragOver(rowOf(container, "s2"), 2).allowed).toBe(true);
    expect(dragOver(rowOf(container, "s2"), 14).allowed).toBe(true);
  });

  it("refuses edge drops but keeps center drops while the mode is on", () => {
    storeState.sortByActivity = true;
    const { container } = renderTree();
    const top = dragOver(rowOf(container, "s2"), 2);
    expect(top.allowed).toBe(false);
    expect(top.dropEffect).toBe("none");
    const bottom = dragOver(rowOf(container, "s2"), 26);
    expect(bottom.allowed).toBe(false);
    const center = dragOver(rowOf(container, "s2"), 14);
    expect(center.allowed).toBe(true);
    expect(center.dropEffect).toBe("move");
  });
});
