//! Session tree keyboard cursor against the mouse, and where Enter / Escape send focus. Clicking a row has to
//! move the cursor, or Enter after a click acts on the row the arrows last visited instead of the clicked one.
//! Focus leaves the tree for the session's own input: its terminal, or the composer of a chat-engine session.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Group, Project, Session } from "../../types";
import type { TreeHandlers } from "./ProjectTree";

const mocks = vi.hoisted(() => ({
  state: {} as Record<string, unknown>,
  focusTerminal: vi.fn(),
  focusComposer: vi.fn(),
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
vi.mock("../../store/projectCollections", () => ({ setProjectCollection: vi.fn() }));
vi.mock("../../i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("../../components/Icons", () => ({ default: new Proxy({}, { get: () => () => null }) }));
vi.mock("../../components/StatusIndicator", () => ({ StatusIndicator: () => null }));
vi.mock("../sessionViewers/sessionMeta", () => ({ SessionKindIcon: () => null }));
vi.mock("../../hooks/useGitBranch", () => ({ useGitBranch: () => null }));
vi.mock("../../hooks/shortcutRegistry", () => ({
  DEFAULT_BINDINGS: { openProject: "Mod+O" },
  formatCombo: (combo: string) => combo,
}));
vi.mock("../../ipc/shareBase", () => ({ isShareSurface: false }));
vi.mock("../../terminal/registry", () => ({ focusTerminal: mocks.focusTerminal }));
vi.mock("../CenterPane/session/composerFocus", () => ({ focusComposer: mocks.focusComposer }));
vi.mock("../../sharing/sessionNavigation", () => ({
  navigateSharedSession: vi.fn(),
  sharedSessionUrl: () => "",
}));

import { ProjectTree } from "./ProjectTree";

const project = (id: string, name: string, sortOrder: number): Project => ({
  id,
  name,
  rootPath: `/tmp/${id}`,
  sortOrder,
  collapsed: false,
  createdAt: 0,
});
const session = (id: string, projectId: string, name: string, engine: Session["engine"]): Session => ({
  id,
  projectId,
  groupId: null,
  parentSessionId: null,
  name,
  kind: engine === "chat" ? "claude" : "terminal",
  engine,
  collapsed: false,
  sortOrder: 0,
  createdAt: 0,
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
const rowOf = (text: string) => screen.getByText(text).closest(".row") as HTMLElement;
const tree = () => screen.getByRole("tree");
const key = (k: string) => fireEvent.keyDown(tree(), { key: k });
const lastSelected = () => (mocks.state.selectSingle as ReturnType<typeof vi.fn>).mock.lastCall?.[0];

beforeEach(() => {
  vi.clearAllMocks();
  mocks.state = {
    projects: [project("p-a", "alpha", 1), project("p-b", "beta", 2)],
    groups: [] as Group[],
    sessions: [session("s-term", "p-a", "shell", "tui"), session("s-chat", "p-b", "agent", "chat")],
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

describe("clicking moves the keyboard cursor", () => {
  // Rows: alpha, shell, beta, agent. Focusing with no active session starts the cursor on alpha.
  it("makes Enter act on the clicked row, not the one the arrows reached", () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.focus(tree());
    key("ArrowDown");
    fireEvent.click(rowOf("beta"));
    key("Enter");
    expect(lastSelected()).toEqual({ id: "p-b", kind: "project" });
  });

  it("follows a click on the expand chevron too", () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.focus(tree());
    fireEvent.click(rowOf("beta").querySelector(".tw")!);
    expect(mocks.state.selectSingle).not.toHaveBeenCalled();
    key("Enter");
    expect(lastSelected()).toEqual({ id: "p-b", kind: "project" });
  });

  it("keeps modifier clicks as multi-select gestures", () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.focus(tree());
    fireEvent.click(rowOf("beta"), { ctrlKey: true });
    expect(mocks.state.toggleSelect).toHaveBeenCalledWith({ id: "p-b", kind: "project" });
    expect(mocks.state.selectSingle).not.toHaveBeenCalled();
    expect(tree().getAttribute("aria-activedescendant")).toBe("vlx-row-main-p-b");
  });
});

describe("focus leaving the tree", () => {
  it("hands Enter on a terminal session to its terminal", async () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.focus(tree());
    fireEvent.click(rowOf("shell"));
    key("Enter");
    await vi.waitFor(() => expect(mocks.focusTerminal).toHaveBeenCalledWith("s-term"));
    expect(mocks.focusComposer).not.toHaveBeenCalled();
  });

  it("hands Enter on a chat-engine session to its composer", async () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.focus(tree());
    fireEvent.click(rowOf("agent"));
    key("Enter");
    await vi.waitFor(() => expect(mocks.focusComposer).toHaveBeenCalledWith("s-chat"));
    expect(mocks.focusTerminal).not.toHaveBeenCalled();
  });

  it("returns Escape to the active chat session's composer", async () => {
    mocks.state.activeSessionId = "s-chat";
    render(<ProjectTree {...handlers()} />);
    fireEvent.focus(tree());
    key("Escape");
    await vi.waitFor(() => expect(mocks.focusComposer).toHaveBeenCalledWith("s-chat"));
    expect(mocks.focusTerminal).not.toHaveBeenCalled();
  });
});
