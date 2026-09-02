import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const menuMocks = vi.hoisted(() => ({
  newSessionItems: vi.fn(
    (
      _projectId: string,
      _groupId: string | null,
      _parentSessionId: string | null,
      opts?: { withTerminal?: boolean; withBrowser?: boolean },
    ) => (opts?.withTerminal ? [{ label: "tree.newTerminalSession", onClick: vi.fn() }] : []),
  ),
}));

const storeState = vi.hoisted(() => ({
  leftWidth: 280,
  importProject: vi.fn(),
  setCloneModalOpen: vi.fn(),
  renameNode: vi.fn(),
  openSession: vi.fn(),
  treeFilter: "",
  setTreeFilter: vi.fn(),
  selection: [],
  clearSelection: vi.fn(),
  archiveMany: vi.fn(),
  archiveGroup: vi.fn(),
  clearNodeWorktree: vi.fn(),
  archiveOpen: false,
  setArchiveOpen: vi.fn(),
  globalSearchOpen: false,
  setGlobalSearchOpen: vi.fn(),
  notifications: {},
  pendingSpawns: [] as unknown[],
  clearAllNotifications: vi.fn(),
  clearAllBadges: vi.fn(),
  ephemeralSessions: {},
  browserTabs: {},
  docTabs: {},
  renameScratch: vi.fn(),
  sessions: [],
  runtimes: {},
  statusFilter: null as ("working" | "asking" | "waiting")[] | null,
  dynamicStatusFilter: true,
  setStatusFilter: vi.fn(),
  appendSidebarTreeViewStatusMatches: vi.fn(),
  refreshSidebarTreeViewStatusMatches: vi.fn(),
  setSidebarTreeViewStatusFilter: vi.fn(),
  markFilter: null,
  setMarkFilter: vi.fn(),
  setSidebarTreeViewMarkFilter: vi.fn(),
  splitSidebarTreeView: vi.fn(),
  deleteSidebarTreeView: vi.fn(),
  setActiveSidebarTreeView: vi.fn(),
  resizeSidebarTreeSplit: vi.fn(),
  groups: [{ id: "group-1", projectId: "project-1", worktreePath: null }],
  projects: [{ id: "project-1", rootPath: "/tmp/project" }],
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));
vi.mock("../../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  );
  return { useTermStore };
});
vi.mock("../../hooks/useGitBranch", () => ({ isWorktreeGone: () => false }));
vi.mock("../../components/Icons", () => ({
  default: new Proxy(
    {},
    { get: () => () => null },
  ),
}));
vi.mock("../../components/ContextMenu", () => ({
  ContextMenu: ({ items, onClose }: { items: { label: string }[]; onClose: () => void }) => (
    <div>
      {items.map((item, index) => <span key={`${item.label}-${index}`}>{item.label}</span>)}
      <button type="button" onClick={onClose}>close-context-menu</button>
    </div>
  ),
}));
vi.mock("../sessionMenu", () => ({
  useSessionMenu: () => ({
    newSessionItems: menuMocks.newSessionItems,
    buildSessionItems: vi.fn(() => []),
    buildScratchItems: vi.fn(() => []),
    buildMoveToMany: vi.fn(() => null),
    buildGitItems: vi.fn(() => []),
    buildMarkItem: vi.fn((_kind: string, _id: string) => ({ label: "mark.menu", submenu: [] })),
    openDialog: vi.fn(),
    dialogs: null,
  }),
}));
vi.mock("./ProjectTree", () => ({
  ProjectTree: ({
    onContext,
    contextId,
  }: {
    onContext: (node: unknown, x: number, y: number) => void;
    contextId: string | null;
  }) => (
    <div data-testid="project-tree" data-context-id={contextId ?? ""}>
      <button
        type="button"
        onContextMenu={(event) => {
          event.preventDefault();
          onContext(
            { kind: "group", id: "group-1", name: "Group", projectId: "project-1" },
            event.clientX,
            event.clientY,
          );
        }}
      >
        Group
      </button>
    </div>
  ),
}));
vi.mock("./ArchivePanel", () => ({ ArchivePanel: () => null }));
vi.mock("../GlobalSearch/GlobalSearch", () => ({ GlobalSearch: () => null }));

import { LeftSidebar } from "./LeftSidebar";

describe("LeftSidebar", () => {
  beforeEach(() => {
    menuMocks.newSessionItems.mockClear();
    storeState.statusFilter = null;
    storeState.dynamicStatusFilter = true;
    storeState.appendSidebarTreeViewStatusMatches.mockClear();
  });

  it("adds dynamic matches and exposes only the requested downward split control", () => {
    storeState.statusFilter = ["working"];
    render(<LeftSidebar />);

    expect(storeState.appendSidebarTreeViewStatusMatches).toHaveBeenCalledWith("main");
    expect(
      screen.getByRole("button", { name: "tree.refreshStatusFilter" }).hasAttribute("disabled"),
    ).toBe(false);
    expect(screen.queryByRole("button", { name: "tree.viewMainName" })).toBeNull();
    expect(screen.queryByRole("button", { name: "tree.viewSplitRight" })).toBeNull();
    expect(screen.getByRole("button", { name: "tree.viewSplitDown" })).toBeTruthy();
  });

  it("does not append matches while dynamic status filtering is disabled", () => {
    storeState.statusFilter = ["working"];
    storeState.dynamicStatusFilter = false;
    render(<LeftSidebar />);

    expect(storeState.appendSidebarTreeViewStatusMatches).not.toHaveBeenCalled();
  });

  it("includes the terminal-session entry scoped to the clicked group", () => {
    render(<LeftSidebar />);

    fireEvent.contextMenu(screen.getByRole("button", { name: "Group" }), {
      clientX: 120,
      clientY: 80,
    });

    expect(menuMocks.newSessionItems).toHaveBeenCalledWith(
      "project-1",
      "group-1",
      null,
      { withBrowser: true, withTerminal: true },
    );
    expect(screen.getByText("tree.newTerminalSession")).toBeTruthy();
  });

  it("offers Move to Worktree on a group that has no worktree yet", () => {
    render(<LeftSidebar />);

    fireEvent.contextMenu(screen.getByRole("button", { name: "Group" }));
    expect(screen.getByText("tree.moveGroupToWorktree")).toBeTruthy();
  });

  it("exposes only a temporary context target while the menu is open", () => {
    render(<LeftSidebar />);
    const tree = screen.getByTestId("project-tree");
    expect(tree.getAttribute("data-context-id")).toBe("");

    fireEvent.contextMenu(screen.getByRole("button", { name: "Group" }));
    expect(tree.getAttribute("data-context-id")).toBe("group-1");

    fireEvent.click(screen.getByRole("button", { name: "close-context-menu" }));
    expect(tree.getAttribute("data-context-id")).toBe("");
  });

  it("does not style an active-only session as selected", async () => {
    const { sessionRowClassName } = await vi.importActual<typeof import("./ProjectTree")>(
      "./ProjectTree",
    );

    expect(sessionRowClassName(false, true, false, false)).toBe("row session active");
    expect(sessionRowClassName(true, false, false, false)).toBe("row session sel");
    expect(sessionRowClassName(true, true, false, false)).toBe("row session sel active");
  });
});
