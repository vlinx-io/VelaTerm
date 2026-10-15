import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Group, Project } from "../../types";
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
vi.mock("../../hooks/useGitBranch", () => ({ useGitBranch: () => null }));
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
import { PROJECT_DRAG_MIME } from "./projectCollectionDrop";

const collection = (id: string, name: string, sortOrder: number, collapsed = false): Project => ({
  rootPath: "",
  id,
  name,
  sortOrder,
  collapsed,
  createdAt: 0,
});
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
    projects: [
      collection("f-pay", "Payments", 1), collection("f-empty", "Empty", 2),
      project("p-web", "payments-web", 1, "f-pay"),
      project("p-notes", "notes", 2, null),
      project("p-api", "payments-api", 3, "f-pay"),
    ],
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

describe("collection rows", () => {
  it("lists collections with their projects indented, then loose projects", () => {
    const { container } = render(<ProjectTree {...handlers()} />);
    expect(names(container)).toEqual(["Payments", "payments-web", "payments-api", "Empty", "notes"]);
    expect(within(rowOf("Payments")).queryByText("2")).toBeNull();
    expect(rowOf("payments-web").style.paddingLeft).toBe("19px");
    expect(rowOf("notes").style.paddingLeft).toBe("6px");
  });

  it("hides a collapsed collection's projects", () => {
    mocks.state.projects = [...(mocks.state.projects as Project[]).filter(p => p.rootPath), collection("f-pay", "Payments", 1, true), collection("f-empty", "Empty", 2)];
    const { container } = render(<ProjectTree {...handlers()} />);
    expect(names(container)).toEqual(["Payments", "Empty", "notes"]);
  });

  it("opens collections and drops empty ones while filtering", () => {
    mocks.state.projects = [...(mocks.state.projects as Project[]).filter(p => p.rootPath), collection("f-pay", "Payments", 1, true), collection("f-empty", "Empty", 2)];
    const base = handlers();
    const { container } = render(<ProjectTree {...base} view={{ ...base.view, treeFilter: "api" }} />);
    expect(names(container)).toEqual(["Payments", "payments-api"]);
  });

  it("toggles the shared collapse state from the primary pane", () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.click(rowOf("Payments"));
    expect(mocks.state.toggleCollapsed).toHaveBeenCalledWith("project", "f-pay");
  });

  it("keeps a split-off pane's collection collapse to that pane", () => {
    const base = handlers();
    render(
      <ProjectTree {...base} isPrimary={false} view={{ ...base.view, id: "side", collapsedOverrides: {} }} />,
    );
    fireEvent.click(rowOf("Payments"));
    expect(mocks.state.setSidebarTreeViewCollapsed).toHaveBeenCalledWith("side", "f-pay", true);
    expect(mocks.state.toggleCollapsed).not.toHaveBeenCalled();
  });

  it("expands a collapsed collection to reveal the active session", () => {
    mocks.state.projects = [collection("f-pay", "Payments", 1, true), project("p-api", "payments-api", 1, "f-pay", false)];
    mocks.state.sessions = [
      {
        id: "s1",
        projectId: "p-api",
        groupId: null,
        parentSessionId: null,
        name: "shell",
        kind: "terminal",
        collapsed: false,
        sortOrder: 0,
        createdAt: 0,
      },
    ];
    mocks.state.activeSessionId = "s1";
    render(<ProjectTree {...handlers()} />);
    expect(mocks.state.toggleCollapsed).toHaveBeenCalledWith("project", "f-pay");
  });

  it("expands a collapsed collection to reveal a freshly opened project", () => {
    mocks.state.projects = [...(mocks.state.projects as Project[]).filter(p => p.rootPath), collection("f-pay", "Payments", 1, true)];
    mocks.state.revealProjectId = "p-api";
    render(<ProjectTree {...handlers()} />);
    expect(mocks.state.toggleCollapsed).toHaveBeenCalledWith("project", "f-pay");
  });
});

const dropData = (payload: object) => ({
  dataTransfer: { getData: () => JSON.stringify(payload), types: ["text/plain", PROJECT_DRAG_MIME] },
});
const overData = (types: string[]) => ({ dataTransfer: { types, dropEffect: "" } });
const projectPayload = (id: string) => ({ kind: "project", id, projectId: id });

describe("dragging projects", () => {
  it("starts a project drag that dragover can recognize", async () => {
    render(<ProjectTree {...handlers()} />);
    const setData = vi.fn();
    fireEvent.dragStart(rowOf("notes"), {
      dataTransfer: { setData, setDragImage: vi.fn(), effectAllowed: "" },
    });
    expect(setData).toHaveBeenCalledWith("text/plain", JSON.stringify(projectPayload("p-notes")));
    expect(setData).toHaveBeenCalledWith(PROJECT_DRAG_MIME, "p-notes");
    // The drag-image clone is removed on the next macrotask; let it go so it does not leak into later tests.
    await new Promise((resolve) => setTimeout(resolve, 0));
  });

  it("moves a project dropped on a collection into that collection", () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.drop(rowOf("Empty"), dropData(projectPayload("p-notes")));
    expect(mocks.setProjectCollection).toHaveBeenCalledWith("p-notes", "f-empty");
    expect(mocks.state.moveNode).not.toHaveBeenCalled();
  });

  it("moves a project dropped on a project inside a collection into that collection", () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.drop(rowOf("payments-api"), dropData(projectPayload("p-notes")));
    expect(mocks.setProjectCollection).toHaveBeenCalledWith("p-notes", "f-pay");
  });

  it("takes a project out of its collection when dropped on a loose project", () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.drop(rowOf("notes"), dropData(projectPayload("p-web")));
    expect(mocks.setProjectCollection).toHaveBeenCalledWith("p-web", null);
  });

  it("does nothing when a project is dropped on itself or its own collection", () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.drop(rowOf("payments-web"), dropData(projectPayload("p-web")));
    fireEvent.drop(rowOf("Payments"), dropData(projectPayload("p-web")));
    fireEvent.drop(rowOf("payments-api"), dropData(projectPayload("p-web")));
    expect(mocks.setProjectCollection).not.toHaveBeenCalled();
  });

  it("keeps session and group drops on a project inside a collection unchanged", () => {
    render(<ProjectTree {...handlers()} />);
    fireEvent.drop(rowOf("payments-web"), dropData({ kind: "session", id: "s1", projectId: "p-notes" }));
    expect(mocks.state.moveNode).toHaveBeenCalledWith("session", "s1", "p-web", null, null, expect.any(Number));
    fireEvent.drop(rowOf("payments-web"), dropData({ kind: "group", id: "g1", projectId: "p-web" }));
    expect(mocks.state.moveNode).toHaveBeenCalledWith("group", "g1", "p-web", null, null, expect.any(Number));
    expect(mocks.setProjectCollection).not.toHaveBeenCalled();
  });

  it("never treats a project dropped on a group row as a group move", () => {
    mocks.state.projects = [project("p-web", "payments-web", 1, "f-pay", false), project("p-notes", "notes", 2, null)];
    mocks.state.groups = [
      { id: "g1", projectId: "p-web", parentGroupId: null, name: "backend", sortOrder: 0, collapsed: true, createdAt: 0 },
    ];
    render(<ProjectTree {...handlers()} />);
    expect(fireEvent.dragOver(rowOf("backend"), overData(["text/plain", PROJECT_DRAG_MIME]))).toBe(true);
    fireEvent.drop(rowOf("backend"), dropData(projectPayload("p-notes")));
    fireEvent.drop(rowOf("backend"), dropData(projectPayload("p-web")));
    expect(mocks.state.moveNode).not.toHaveBeenCalled();
    expect(mocks.setProjectCollection).not.toHaveBeenCalled();
  });

  it("keeps collection rows available to both session and project drags", () => {
    render(<ProjectTree {...handlers()} />);
    expect(fireEvent.dragOver(rowOf("Payments"), overData(["text/plain"]))).toBe(false);
    expect(fireEvent.dragOver(rowOf("Payments"), overData(["text/plain", PROJECT_DRAG_MIME]))).toBe(false);
  });
});

describe("other projects section", () => {
  const filtered = (treeFilter: string, statusFilter: boolean) => {
    const base = handlers();
    return {
      ...base,
      view: {
        ...base.view,
        treeFilter,
        statusFilter: statusFilter ? (["working"] as ["working"]) : null,
        statusFilterIds: statusFilter ? { s1: true as const } : null,
      },
    };
  };

  beforeEach(() => {
    mocks.state.sessions = [
      {
        id: "s1",
        projectId: "p-api",
        groupId: null,
        parentSessionId: null,
        name: "pay-shell",
        kind: "terminal",
        collapsed: false,
        sortOrder: 0,
        createdAt: 0,
      },
    ];
  });

  it("lists filter-hidden projects matching the search below the results", () => {
    const { container } = render(<ProjectTree {...filtered("pay", true)} />);
    expect(screen.getByText("tree.otherProjects")).toBeTruthy();
    expect(names(container)).toEqual(["Payments", "payments-api", "pay-shell", "payments-web"]);
    expect(rowOf("payments-web").style.paddingLeft).toBe("6px");
  });

  it("stays hidden without a status filter", () => {
    render(<ProjectTree {...filtered("pay", false)} />);
    expect(screen.queryByText("tree.otherProjects")).toBeNull();
  });

  it("shows name matches in the main tree with only a mark filter", () => {
    const base = handlers();
    const { container } = render(
      <ProjectTree {...base} view={{ ...base.view, treeFilter: "notes", markFilter: "⭐" }} />,
    );
    expect(screen.queryByText("tree.otherProjects")).toBeNull();
    expect(names(container)).toEqual(["notes"]);
  });

  it("replaces the empty state when only hidden projects match", () => {
    const { container } = render(<ProjectTree {...filtered("notes", true)} />);
    expect(screen.queryByText("tree.noMatch")).toBeNull();
    expect(screen.getByText("tree.otherProjects")).toBeTruthy();
    expect(names(container)).toEqual(["notes"]);
  });

  it("opens a new session from a hidden project's add button", () => {
    const props = filtered("notes", true);
    render(<ProjectTree {...props} />);
    fireEvent.click(within(rowOf("notes")).getByTitle("tree.newSession"));
    expect(props.onAddSession).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "project", id: "p-notes", projectId: "p-notes" }),
      expect.any(Number),
      expect.any(Number),
    );
  });
});
