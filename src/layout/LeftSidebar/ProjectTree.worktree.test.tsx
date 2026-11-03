import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Group, Project, Session } from "../../types";
import type { TreeHandlers } from "./ProjectTree";

const mocks = vi.hoisted(() => ({
  state: {} as Record<string, unknown>,
  setProjectCollection: vi.fn(),
  cwdWorktrees: new Map<string, string>(),
}));

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count, getItemKey }: { count: number; getItemKey: (index: number) => string }) => ({
    getTotalSize: () => count * 28,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({ index, key: getItemKey(index), start: index * 28 })),
    measureElement: () => {},
    measure: () => {},
    scrollToIndex: () => {},
  }),
}));
vi.mock("../../store/termStore", () => ({
  useTermStore: Object.assign(
    (selector: (state: Record<string, unknown>) => unknown) => selector(mocks.state),
    { getState: () => mocks.state },
  ),
  isVisibleSession: () => false,
}));
vi.mock("../../store/projectCollections", () => ({
  setProjectCollection: mocks.setProjectCollection,
}));
vi.mock("../../i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("../../components/Icons", () => ({ default: new Proxy({}, { get: () => () => null }) }));
vi.mock("../../components/StatusIndicator", () => ({ StatusIndicator: () => null }));
vi.mock("../sessionViewers/sessionMeta", () => ({ SessionKindIcon: () => null }));
vi.mock("../../hooks/useGitBranch", () => ({
  useGitBranch: () => null,
  useLinkedWorktrees: () => mocks.cwdWorktrees,
}));
vi.mock("../../hooks/shortcutRegistry", () => ({
  DEFAULT_BINDINGS: { openProject: "Mod+O" },
  formatCombo: (combo: string) => combo,
}));
vi.mock("../../ipc/shareBase", () => ({ isShareSurface: false }));
vi.mock("../../sharing/sessionNavigation", () => ({
  navigateSharedSession: vi.fn(),
  sharedSessionUrl: () => "",
}));

import { ProjectTree } from "./ProjectTree";

const project = (id: string, name: string, sortOrder: number, collectionId: string | null, collapsed = true): Project => ({
  id,
  name,
  rootPath: `/tmp/${id}`,
  sortOrder,
  collapsed,
  createdAt: 0,
  collectionId,
});
const handlers = (overrides: Partial<TreeHandlers> = {}): TreeHandlers => ({
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
  renameError: null,
  onNewCollection: vi.fn(),
  setRenameVal: vi.fn(),
  commitRename: vi.fn(),
  cancelRename: vi.fn(),
  onAddSession: vi.fn(),
  onAddGroup: vi.fn(),
  ...overrides,
});
const names = (container: HTMLElement) =>
  Array.from(container.querySelectorAll(".row .nm")).map((el) => el.textContent);
const rowOf = (text: string) => screen.getByText(text).closest(".row") as HTMLElement;

beforeEach(() => {
  vi.clearAllMocks();
  mocks.setProjectCollection.mockResolvedValue(undefined);
  mocks.state = {
    projects: [project("p1", "app", 1, null, false)],
    groups: [] as Group[],
    sessions: [],
    ephemeralSessions: {},
    runtimes: {},
    notifications: {},
    treeLoaded: true,
    shortcutOverrides: {},
    activeSessionId: null,
    revealProjectId: null,
    revealSuppressId: null,
    density: "regular",
    navLayout: "regular",
    selection: [],
    selectionAnchor: null,
    setCreateProjectModalOpen: vi.fn(),
    importProject: vi.fn(),
    setCloneModalOpen: vi.fn(),
    toggleCollapsed: vi.fn(),
    openSession: vi.fn(),
    setRevealProject: vi.fn(),
    setRevealSuppress: vi.fn(),
    moveNode: vi.fn(),
    moveMany: vi.fn(),
    selectSingle: vi.fn(),
    toggleSelect: vi.fn(),
    setSelection: vi.fn(),
    setInspectTarget: vi.fn(),
    setSidebarTreeViewCollapsed: vi.fn(),
  };
});

const group = (id: string, name: string, worktreePath: string | null = null): Group => ({
  id, projectId: "p1", parentGroupId: null, name, sortOrder: 1, collapsed: false, worktreePath,
} as Group);
const session = (
  id: string, sortOrder: number, extra: Partial<Session> = {},
): Session => ({
  id, projectId: "p1", groupId: "g1", name: id, kind: "terminal", sortOrder, collapsed: false, ...extra,
} as Session);
const WT_A = "/repo/.vlx-worktrees/app-aaa";
const WT_B = "/repo/.vlx-worktrees/app-bbb";

beforeEach(() => {
  localStorage.clear();
  mocks.cwdWorktrees = new Map();
  mocks.state.groups = [group("g1", "Feature")];
});

describe("worktree rows", () => {
  it("gathers sessions sharing a worktree under one row placed at the first of them", () => {
    mocks.state.sessions = [
      session("plain-1", 1),
      session("wt-a-1", 2, { worktreePath: WT_A }),
      session("wt-b-1", 3, { worktreePath: WT_B }),
      session("plain-2", 4),
      session("wt-a-2", 5, { worktreePath: `${WT_A}/` }),
    ];
    const { container } = render(<ProjectTree {...handlers()} />);
    expect(names(container)).toEqual([
      "app", "Feature", "plain-1", "app-aaa", "wt-a-1", "wt-a-2", "app-bbb", "wt-b-1", "plain-2",
    ]);
    expect(container.querySelectorAll(".row.worktree")).toHaveLength(2);
    expect(rowOf("app-aaa").style.paddingLeft).toBe(rowOf("plain-1").style.paddingLeft);
    expect(rowOf("wt-a-1").style.paddingLeft).toBe("45px");
    expect(rowOf("plain-1").style.paddingLeft).toBe("32px");
  });

  it("shows a bound group under its worktree row and no worktree rows inside it", () => {
    mocks.state.groups = [
      group("g1", "Feature", WT_A),
      { ...group("g2", "Nested"), parentGroupId: "g1" },
      group("g3", "Plain"),
    ];
    mocks.state.sessions = [
      session("wt-a-1", 1, { worktreePath: WT_A }),
      session("wt-b-1", 2, { worktreePath: WT_B }),
      session("nested-b", 3, { groupId: "g2", worktreePath: WT_B }),
      session("root-a", 4, { groupId: null, worktreePath: WT_A }),
    ];
    const { container } = render(<ProjectTree {...handlers()} />);
    expect(names(container)).toEqual([
      "app", "app-aaa", "Feature", "Nested", "nested-b", "wt-a-1", "wt-b-1", "root-a", "Plain",
    ]);
    expect(container.querySelectorAll(".row.worktree")).toHaveLength(1);
    expect(rowOf("app-aaa").style.paddingLeft).toBe(rowOf("Plain").style.paddingLeft);
    expect(rowOf("Feature").style.paddingLeft).toBe("32px");
    expect(rowOf("root-a").style.paddingLeft).toBe("32px");
    expect(rowOf("Nested").style.paddingLeft).toBe("45px");
  });

  it("gathers a session group whose descendants all share a worktree above its parent session", () => {
    mocks.state.sessions = [
      session("root-wt", 1, { groupId: null, worktreePath: WT_A }),
      session("parent", 2),
      session("child-1", 3, { parentSessionId: "parent", worktreePath: WT_B }),
      session("child-2", 4, { parentSessionId: "parent", worktreePath: WT_B }),
      session("grandchild", 5, { parentSessionId: "child-1", worktreePath: WT_B }),
    ];
    const { container } = render(<ProjectTree {...handlers()} />);
    expect(names(container)).toEqual([
      "app", "Feature", "app-bbb", "parent", "child-1", "grandchild", "child-2", "app-aaa", "root-wt",
    ]);
  });

  it("gathers a session started in a worktree directory by the worktree its cwd sits in", () => {
    mocks.cwdWorktrees = new Map([[`${WT_B}/sub`, WT_B]]);
    mocks.state.sessions = [
      session("parent", 1),
      session("child", 2, { parentSessionId: "parent", cwd: `${WT_B}/sub` }),
    ];
    const { container } = render(<ProjectTree {...handlers()} />);
    expect(names(container)).toEqual(["app", "Feature", "app-bbb", "parent", "child"]);
  });

  it("shows no worktree rows under a session in a worktree", () => {
    mocks.state.sessions = [
      session("parent", 1, { worktreePath: WT_A }),
      session("child-a", 2, { parentSessionId: "parent", worktreePath: WT_A }),
      session("child-b", 3, { parentSessionId: "parent", worktreePath: WT_B }),
    ];
    const { container } = render(<ProjectTree {...handlers()} />);
    expect(names(container)).toEqual(["app", "Feature", "app-aaa", "parent", "child-a", "child-b"]);
  });

  it("keeps worktree rows for children of a session group spanning several worktrees", () => {
    mocks.state.sessions = [
      session("parent", 1),
      session("child-a", 2, { parentSessionId: "parent", worktreePath: WT_A }),
      session("child-plain", 3, { parentSessionId: "parent" }),
    ];
    const { container } = render(<ProjectTree {...handlers()} />);
    expect(names(container)).toEqual(["app", "Feature", "parent", "app-aaa", "child-a", "child-plain"]);
  });

  it("collapses a worktree row and remembers it", () => {
    mocks.state.sessions = [session("wt-a-1", 1, { worktreePath: WT_A })];
    const { container, unmount } = render(<ProjectTree {...handlers()} />);
    fireEvent.click(rowOf("app-aaa"));
    expect(names(container)).toEqual(["app", "Feature", "app-aaa"]);
    expect(mocks.state.toggleCollapsed).not.toHaveBeenCalled();
    unmount();
    const again = render(<ProjectTree {...handlers()} />);
    expect(names(again.container)).toEqual(["app", "Feature", "app-aaa"]);
  });

  it("uses the pane's own collapse map in a split-off pane", () => {
    mocks.state.sessions = [session("wt-a-1", 1, { worktreePath: WT_A })];
    const base = handlers({ isPrimary: false });
    render(<ProjectTree {...base} view={{ ...base.view, id: "v2", collapsedOverrides: {} }} />);
    fireEvent.click(rowOf("app-aaa"));
    expect(mocks.state.setSidebarTreeViewCollapsed).toHaveBeenCalledWith(
      "v2", expect.stringMatching(/^wt:g1:/), true,
    );
  });

  it("shows a worktree row while filtering when one of its sessions matches", () => {
    mocks.state.sessions = [
      session("alpha", 1, { worktreePath: WT_A }),
      session("beta", 2, { worktreePath: WT_A }),
      session("gamma", 3),
    ];
    const base = handlers();
    const { container } = render(<ProjectTree {...base} view={{ ...base.view, treeFilter: "beta" }} />);
    expect(names(container)).toEqual(["app", "Feature", "app-aaa", "beta"]);
  });

  it("moves a session dropped on a worktree row into the row's container", () => {
    mocks.state.sessions = [session("wt-a-1", 1, { worktreePath: WT_A }), session("other", 2, { groupId: null })];
    render(<ProjectTree {...handlers()} />);
    const payload = JSON.stringify({ kind: "session", id: "other", projectId: "p1" });
    fireEvent.drop(rowOf("app-aaa"), {
      dataTransfer: { getData: () => payload, types: ["text/plain"] },
    });
    expect(mocks.state.moveNode).toHaveBeenCalledWith("session", "other", "p1", "g1", null, expect.any(Number));
  });
});
