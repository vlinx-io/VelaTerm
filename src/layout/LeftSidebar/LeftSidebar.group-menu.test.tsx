import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  buildKillProcessMany: vi.fn(() => null as { label: string; onClick: () => void } | null),
}));

const storeState = vi.hoisted(() => ({
  leftWidth: 280,
  importProject: vi.fn(),
  addVirtualProject: vi.fn(),
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
  sidebarTreeViews: [] as unknown[],
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
  useT: () => (key: string, ...args: unknown[]) => (args.length ? `${key}:${args.join(",")}` : key),
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
vi.mock("../../components/ContextMenu", () => {
  type MockMenuItem = {
    label: string;
    disabled?: boolean;
    href?: string;
    onClick?: (e: React.MouseEvent) => void;
    submenu?: MockMenuItem[];
  };
  const renderItems = (items: MockMenuItem[], prefix = ""): React.ReactNode =>
    items.map((item, index) => (
      <div key={`${prefix}${item.label}-${index}`}>
        <button
          type="button"
          disabled={item.disabled}
          data-href={item.href ?? ""}
          onClick={(e) => item.onClick?.(e)}
        >
          {item.label}
        </button>
        {item.submenu ? renderItems(item.submenu, `${prefix}${index}-`) : null}
      </div>
    ));
  return {
    ContextMenu: ({ items, onClose }: { items: MockMenuItem[]; onClose: () => void }) => (
      <div>
        {renderItems(items)}
        <button type="button" onClick={onClose}>close-context-menu</button>
      </div>
    ),
  };
});
vi.mock("../sessionMenu", () => ({
  sessionStatusItem: vi.fn(() => null),
  useSessionMenu: () => ({
    newSessionItems: menuMocks.newSessionItems,
    buildSessionItems: vi.fn(() => []),
    buildScratchItems: vi.fn(() => []),
    buildMoveToMany: vi.fn(() => null),
    buildKillProcessMany: menuMocks.buildKillProcessMany,
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
      <button
        type="button"
        onContextMenu={(event) => {
          event.preventDefault();
          onContext(
            { kind: "session", id: "s1", name: "Session 1", projectId: "project-1", groupId: "group-1" },
            event.clientX,
            event.clientY,
          );
        }}
      >
        Session 1
      </button>
    </div>
  ),
}));
vi.mock("../GlobalSearch/GlobalSearch", () => ({ GlobalSearch: () => null }));

import { LeftSidebar } from "./LeftSidebar";

describe("LeftSidebar", () => {
  beforeEach(() => {
    menuMocks.newSessionItems.mockClear();
    menuMocks.buildKillProcessMany.mockReset().mockReturnValue(null);
    storeState.addVirtualProject.mockReset();
    storeState.statusFilter = null;
    storeState.dynamicStatusFilter = true;
    storeState.appendSidebarTreeViewStatusMatches.mockClear();
    storeState.selection = [];
    storeState.sessions = [];
    storeState.groups = [{ id: "group-1", projectId: "project-1", worktreePath: null }];
    window.history.replaceState(null, "", "/");
  });

  it("keeps failed collection creation open and closes only after a successful retry", async () => {
    storeState.addVirtualProject
      .mockRejectedValueOnce(new Error("A collection with this name already exists"))
      .mockResolvedValueOnce(undefined);
    render(<LeftSidebar />);
    fireEvent.click(screen.getByRole("link", { name: "tree.newCollection" }));
    const input = screen.getByRole("textbox", { name: /collection.name/ }) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "  Research  " } });
    fireEvent.click(screen.getByRole("button", { name: "collection.submit" }));

    expect((await screen.findByRole("alert")).textContent).toBe("collection.duplicateName");
    expect(input.value).toBe("  Research  ");
    expect(storeState.addVirtualProject).toHaveBeenCalledWith("Research");
    fireEvent.click(screen.getByRole("button", { name: "collection.submit" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(storeState.addVirtualProject).toHaveBeenCalledTimes(2);
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
    // Other panes of the active tab get the quieter marker; the focused pane never does.
    expect(sessionRowClassName(false, false, false, false, true)).toBe("row session in-view");
    expect(sessionRowClassName(false, true, false, false, true)).toBe("row session active");
  });

  it("offers batch Rename with AI in multiselection menu for renameable agent sessions", () => {
    storeState.sessions = [
      { id: "s1", projectId: "project-1", groupId: "group-1", name: "One", kind: "claude", agentSessionId: "a1", engine: "tui" },
      { id: "s2", projectId: "project-1", groupId: "group-1", name: "Two", kind: "codex", agentSessionId: null, engine: "chat" },
      { id: "s3", projectId: "project-1", groupId: "group-1", name: "Term", kind: "terminal", agentSessionId: null, engine: "tui" },
    ] as never;
    storeState.selection = [
      { kind: "session", id: "s1" },
      { kind: "session", id: "s2" },
      { kind: "session", id: "s3" },
    ] as never;

    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Session 1" }));
    const item = screen.getByRole("button", { name: "sessionTitle.menuSelected:2" });
    expect(item.hasAttribute("disabled")).toBe(false);
    expect(item.getAttribute("data-href")).toContain("smartRename=s1%2Cs2");
    fireEvent.click(item);
    expect(window.location.search).toContain("smartRename=s1%2Cs2");
  });

  it("offers batch Rename with AI on a group containing at least two renameable agent sessions", () => {
    storeState.groups = [
      { id: "group-1", projectId: "project-1", parentGroupId: null, worktreePath: null },
      { id: "group-2", projectId: "project-1", parentGroupId: "group-1", worktreePath: null },
    ] as never;
    storeState.sessions = [
      { id: "s1", projectId: "project-1", groupId: "group-1", name: "One", kind: "claude", agentSessionId: "a1", engine: "tui" },
      { id: "s2", projectId: "project-1", groupId: "group-2", name: "Two", kind: "codex", agentSessionId: "a2", engine: "tui" },
    ] as never;

    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Group" }));
    const item = screen.getByRole("button", { name: "sessionTitle.menuGroup:2" });
    expect(item.getAttribute("data-href")).toContain("smartRename=s1%2Cs2");
  });

  it("disables batch Rename with AI when selected agent sessions have no conversation yet", () => {
    storeState.sessions = [
      { id: "s1", projectId: "project-1", groupId: "group-1", name: "One", kind: "claude", agentSessionId: null, engine: "tui" },
      { id: "s2", projectId: "project-1", groupId: "group-1", name: "Two", kind: "codex", agentSessionId: null, engine: "tui" },
    ] as never;
    storeState.selection = [
      { kind: "session", id: "s1" },
      { kind: "session", id: "s2" },
    ] as never;

    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Session 1" }));
    const item = screen.getByRole("button", { name: "sessionTitle.menu" });
    expect(item.hasAttribute("disabled")).toBe(true);
  });

  it("offers binding selected sessions as a Plan/Execute workflow in multiselection menu", () => {
    window.history.replaceState(null, "", "/");
    storeState.sessions = [
      { id: "s1", projectId: "project-1", groupId: "group-1", name: "One", kind: "antigravity", engine: "chat" },
      { id: "s2", projectId: "project-1", groupId: "group-1", name: "Two", kind: "codex", engine: "chat" },
    ] as never;
    storeState.selection = [
      { kind: "session", id: "s1" },
      { kind: "session", id: "s2" },
    ] as never;

    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Session 1" }));
    const orchMenu = screen.getByRole("button", { name: "tree.orchestrationMenu" });
    expect(orchMenu.hasAttribute("disabled")).toBe(false);
    const item = screen.getByRole("button", { name: "tree.bindPlanExecuteSelected" });
    expect(item.hasAttribute("disabled")).toBe(false);
    expect(item.getAttribute("data-href")).toContain("bindPlanExecute=");
    expect(item.getAttribute("data-href")).toContain("bindExec=s1%2Cs2");
    const execItem = screen.getByRole("button", { name: "tree.bindAsExecutorsSelected" });
    expect(execItem.hasAttribute("disabled")).toBe(false);
    expect(execItem.getAttribute("data-href")).toContain("bindRole=exec");
    expect(execItem.getAttribute("data-href")).toContain("bindExec=s1%2Cs2");
    fireEvent.click(execItem);
    expect(window.location.search).toContain("bindPlanExecute=");
    expect(window.location.search).toContain("bindRole=exec");
    expect(window.location.search).toContain("bindExec=s1%2Cs2");
    window.history.replaceState(null, "", "/");
  });

  it("disables binding selected sessions when none use the chat engine with a supported agent", () => {
    storeState.sessions = [
      { id: "s1", projectId: "project-1", groupId: "group-1", name: "One", kind: "claude", engine: "tui" },
      { id: "s2", projectId: "project-1", groupId: "group-1", name: "Two", kind: "codex", engine: "tui" },
    ] as never;
    storeState.selection = [
      { kind: "session", id: "s1" },
      { kind: "session", id: "s2" },
    ] as never;

    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Session 1" }));
    const orchMenu = screen.getByRole("button", { name: "tree.orchestrationMenu" });
    expect(orchMenu.hasAttribute("disabled")).toBe(true);
    const item = screen.getByRole("button", { name: "tree.bindPlanExecuteSelected" });
    expect(item.hasAttribute("disabled")).toBe(true);
    const execItem = screen.getByRole("button", { name: "tree.bindAsExecutorsSelected" });
    expect(execItem.hasAttribute("disabled")).toBe(true);
  });

  it("offers Kill Process in the multiselection menu for selected sessions and ignores non-session nodes", () => {
    const onKillClick = vi.fn();
    menuMocks.buildKillProcessMany.mockReturnValue({
      label: "tree.killProcessSelected:2",
      onClick: onKillClick,
    });
    storeState.selection = [
      { kind: "session", id: "s1" },
      { kind: "group", id: "group-1" },
      { kind: "session", id: "s2" },
    ] as never;

    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Session 1" }));

    expect(menuMocks.buildKillProcessMany).toHaveBeenCalledWith(
      ["s1", "s2"],
      { onDone: storeState.clearSelection },
    );
    const killBtn = screen.getByRole("button", { name: "tree.killProcessSelected:2" });
    fireEvent.click(killBtn);
    expect(onKillClick).toHaveBeenCalledTimes(1);
  });

  it("omits Kill Process from the multiselection menu when no selected session can be killed", () => {
    menuMocks.buildKillProcessMany.mockReturnValue(null);
    storeState.selection = [
      { kind: "session", id: "s1" },
      { kind: "session", id: "s2" },
    ] as never;

    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Session 1" }));

    expect(menuMocks.buildKillProcessMany).toHaveBeenCalledWith(
      ["s1", "s2"],
      { onDone: storeState.clearSelection },
    );
    expect(screen.queryByRole("button", { name: /tree\.killProcess/ })).toBeNull();
  });
});
