import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Group, Project, Session } from "../../types";
import type { TreeHandlers } from "./ProjectTree";

const mocks = vi.hoisted(() => ({
  state: {} as Record<string, unknown>,
  setProjectCollection: vi.fn(),
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
vi.mock("../../hooks/useGitBranch", () => ({ useGitBranch: () => null, useLinkedWorktrees: () => new Map() }));
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
beforeEach(() => {
  mocks.state.groups = [group("g1", "Feature")];
});
const filtered = (treeFilter: string) => handlers({ view: { ...handlers().view, treeFilter } });

describe("search text filtering", () => {
  it("keeps a matching session visible after it is renamed to something the text no longer matches", () => {
    mocks.state.sessions = [session("api-old", 1), session("other", 2)];
    const { container, rerender } = render(<ProjectTree {...filtered("api")} />);
    expect(names(container)).toEqual(["app", "Feature", "api-old"]);

    mocks.state.sessions = [session("api-old", 1, { name: "renamed" }), session("other", 2)];
    rerender(<ProjectTree {...filtered("api")} />);
    expect(names(container)).toEqual(["app", "Feature", "renamed"]);
  });

  it("keeps a matching group and its contents visible after the group is renamed", () => {
    mocks.state.groups = [group("g1", "Api work")];
    mocks.state.sessions = [session("s1", 1)];
    const { container, rerender } = render(<ProjectTree {...filtered("api")} />);
    expect(names(container)).toEqual(["app", "Api work", "s1"]);

    mocks.state.groups = [group("g1", "Backend")];
    rerender(<ProjectTree {...filtered("api")} />);
    expect(names(container)).toEqual(["app", "Backend", "s1"]);
  });

  it("judges renamed items by their new name once the search text changes", () => {
    mocks.state.sessions = [session("api-old", 1), session("other", 2)];
    const { container, rerender } = render(<ProjectTree {...filtered("api")} />);
    mocks.state.sessions = [session("api-old", 1, { name: "renamed" }), session("other", 2)];
    rerender(<ProjectTree {...filtered("api")} />);

    rerender(<ProjectTree {...filtered("ren")} />);
    expect(names(container)).toEqual(["app", "Feature", "renamed"]);
    rerender(<ProjectTree {...filtered("api")} />);
    expect(container.querySelectorAll(".row .nm")).toHaveLength(0);
  });
});
