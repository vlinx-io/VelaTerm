import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import type { Project } from "../../types";

const mocks = vi.hoisted(() => ({ setProjectCollection: vi.fn(), openDialog: vi.fn() }));

const storeState = vi.hoisted(() => ({
  leftWidth: 280,
  importProject: vi.fn(),
  setCreateProjectModalOpen: vi.fn(),
  setCloneModalOpen: vi.fn(),
  renameNode: vi.fn(),
  addVirtualProject: vi.fn(),
  openSession: vi.fn(),
  treeFilter: "",
  setTreeFilter: vi.fn(),
  selection: [],
  clearSelection: vi.fn(),
  archiveMany: vi.fn(),
  archiveGroup: vi.fn(),
  archiveCollection: vi.fn(),
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
  sessions: [] as { id: string; projectId: string }[],
  runtimes: {},
  statusFilter: null,
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
  groups: [],
  projects: [] as Project[],
  treeLoaded: true,
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
  getLocale: () => "en",
}));
vi.mock("../../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  );
  return { useTermStore };
});
vi.mock("../../store/projectCollections", () => ({ setProjectCollection: mocks.setProjectCollection }));
vi.mock("../../hooks/useGitBranch", () => ({ isWorktreeGone: () => false }));
vi.mock("../../components/Icons", () => ({
  default: new Proxy({}, { get: () => () => null }),
}));
vi.mock("../../components/ContextMenu", () => {
  interface Item {
    label: string;
    separator?: boolean;
    disabled?: boolean;
    onClick?: (event: React.MouseEvent) => void;
    submenu?: Item[];
  }
  const renderItems = (items: Item[], onClose: () => void, prefix: string): ReactNode[] =>
    items.map((item, index) =>
      item.separator ? null : (
        <div key={`${prefix}-${index}`}>
          <button
            type="button"
            disabled={item.disabled}
            onClick={(event) => {
              item.onClick?.(event);
              if (item.onClick) onClose();
            }}
          >
            {item.label}
          </button>
          {item.submenu && renderItems(item.submenu, onClose, `${prefix}-${index}`)}
        </div>
      ),
    );
  return {
    ContextMenu: ({ items, onClose }: { items: Item[]; onClose: () => void }) => (
      <div data-testid="context-menu">{renderItems(items, onClose, "m")}</div>
    ),
  };
});
vi.mock("../sessionMenu", () => ({
  useSessionMenu: () => ({
    newSessionItems: vi.fn(() => []),
    buildSessionItems: vi.fn(() => []),
    buildScratchItems: vi.fn(() => []),
    buildMoveToMany: vi.fn(() => null),
    buildGitItems: vi.fn(() => []),
    buildMarkItem: vi.fn(() => ({ label: "mark.menu", submenu: [] })),
    openDialog: mocks.openDialog,
    dialogs: null,
  }),
}));
vi.mock("./ProjectTree", () => ({
  ProjectTree: ({ onContext, onAddSession }: { onContext: (node: unknown, x: number, y: number) => void; onAddSession: (node: unknown, x: number, y: number) => void }) => <div>
    {storeState.projects.map(p => <div key={p.id}>
      <button onContextMenu={event => {
        event.preventDefault();
        onContext({ kind: "project", id: p.id, name: p.name, projectId: p.id, groupId: null }, 10, 10);
      }}>{p.name}</button>
      <button onClick={() => onAddSession({ kind: "project", id: p.id, name: p.name, projectId: p.id, groupId: null }, 10, 10)}>add-{p.name}</button>
    </div>)}
  </div>,
}));
vi.mock("../GlobalSearch/GlobalSearch", () => ({ GlobalSearch: () => null }));

import { LeftSidebar } from "./LeftSidebar";

const project = (id: string, name: string, rootPath: string, collectionId: string | null = null): Project => ({
  id, name, rootPath, collectionId, collapsed: false, sortOrder: 0, createdAt: 0,
});
beforeEach(() => {
  vi.clearAllMocks();
  window.history.replaceState(null, "", "/");
  mocks.setProjectCollection.mockResolvedValue(undefined);
  storeState.addVirtualProject.mockResolvedValue(undefined);
  storeState.renameNode.mockResolvedValue(undefined);
  storeState.projects = [project("project-1", "payments-web", "/tmp/project"), project("c1", "Payments", "")];
  storeState.sessions = [];
});

it("offers Collection creation only in the header toolbar, keeps its draft in the URL, and submits a trimmed name", async () => {
  render(<LeftSidebar />);
  const creationLinks = screen.getAllByRole("link", { name: "tree.newCollection" });
  expect(creationLinks).toHaveLength(1);
  fireEvent.click(creationLinks[0]);
  const input = screen.getByRole("textbox", { name: /collection.name/ });
  fireEvent.change(input, { target: { value: "  Billing  " } });
  expect(new URLSearchParams(window.location.search).get("collectionName")).toBe("  Billing  ");
  fireEvent.click(screen.getByRole("button", { name: "collection.submit" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(storeState.addVirtualProject).toHaveBeenCalledWith("Billing");
  expect(window.location.search).toBe("");
  expect(screen.queryByText("folder.new")).toBeNull();
});

it("restores a creation form from a URL without creating anything", () => {
  window.history.replaceState(null, "", "/?collectionDialog=create&collectionName=Research");
  render(<LeftSidebar />);
  expect((screen.getByRole("textbox", { name: /collection.name/ }) as HTMLInputElement).value).toBe("Research");
  expect(storeState.addVirtualProject).not.toHaveBeenCalled();
});

it("offers collection, project, import and group creation from a collection's plus menu", () => {
  render(<LeftSidebar />);
  fireEvent.click(screen.getByRole("button", { name: "add-Payments" }));
  for (const name of ["tree.newCollection", "tree.createProject", "tree.importProject", "tree.newGroup"]) {
    expect(within(screen.getByTestId("context-menu")).getByRole("button", { name })).toBeTruthy();
  }
  fireEvent.click(within(screen.getByTestId("context-menu")).getByRole("button", { name: "tree.createProject" }));
  expect(storeState.setCreateProjectModalOpen).toHaveBeenCalledWith(true, "c1");
  fireEvent.click(screen.getByRole("button", { name: "add-Payments" }));
  fireEvent.click(within(screen.getByTestId("context-menu")).getByRole("button", { name: "tree.importProject" }));
  expect(storeState.importProject).toHaveBeenCalledWith("c1");
  fireEvent.click(screen.getByRole("button", { name: "add-Payments" }));
  fireEvent.click(screen.getByRole("button", { name: "tree.newGroup" }));
  expect(mocks.openDialog).toHaveBeenCalledWith({ type: "newGroup", projectId: "c1", parentGroupId: null });
  fireEvent.click(screen.getByRole("button", { name: "add-Payments" }));
  fireEvent.click(screen.getByRole("button", { name: "tree.newCollection" }));
  expect(new URLSearchParams(window.location.search).get("collectionParentId")).toBe("c1");
  expect(screen.getByRole("dialog")).toBeTruthy();
});

it("restores the parent collection from a child-creation URL and submits it to the backend action", async () => {
  window.history.replaceState(null, "", "/?collectionDialog=create&collectionParentId=c1&collectionName=Research");
  render(<LeftSidebar />);
  fireEvent.click(screen.getByRole("button", { name: "collection.submit" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(storeState.addVirtualProject).toHaveBeenCalledWith("Research", "c1");
  expect(window.location.search).toBe("");
});

it("retains the collection rename form and draft on a backend failure", async () => {
  storeState.renameNode.mockRejectedValue(new Error("Node not found"));
  render(<LeftSidebar />);
  fireEvent.contextMenu(screen.getByRole("button", { name: "Payments" }));
  fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
  expect((screen.getByRole("textbox", { name: /collection.name/ }) as HTMLInputElement).value).toBe("Payments");
  fireEvent.change(screen.getByRole("textbox", { name: /collection.name/ }), { target: { value: "Billing" } });
  fireEvent.click(screen.getByRole("button", { name: "common.save" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Node not found");
  expect(screen.getByRole("dialog")).toBeTruthy();
  expect((screen.getByRole("textbox", { name: /collection.name/ }) as HTMLInputElement).value).toBe("Billing");
  expect(new URLSearchParams(window.location.search).get("collectionDialog")).toBe("rename");
});

it("offers Move to Collection and a top-level destination", () => {
  render(<LeftSidebar />);
  fireEvent.contextMenu(screen.getByRole("button", { name: "payments-web" }));
  expect(screen.getByRole("button", { name: "collection.moveTo" })).toBeTruthy();
  fireEvent.click(screen.getAllByRole("button", { name: "Payments" }).at(-1)!);
  expect(mocks.setProjectCollection).toHaveBeenCalledWith("project-1", "c1");
});

it("detaches an existing member using the menu", () => {
  storeState.projects[0].collectionId = "c1";
  render(<LeftSidebar />);
  fireEvent.contextMenu(screen.getByRole("button", { name: "payments-web" }));
  fireEvent.click(screen.getByRole("button", { name: "collection.none" }));
  expect(mocks.setProjectCollection).toHaveBeenCalledWith("project-1", null);
});

it("offers Archive Collection only for a collection that holds sessions", () => {
  render(<LeftSidebar />);
  fireEvent.contextMenu(screen.getByRole("button", { name: "Payments" }));
  expect(screen.queryByRole("button", { name: "tree.archiveCollection" })).toBeNull();
});

it("archives a collection that holds sessions from its menu", () => {
  storeState.sessions = [{ id: "s1", projectId: "c1" }];
  render(<LeftSidebar />);
  fireEvent.contextMenu(screen.getByRole("button", { name: "Payments" }));
  fireEvent.click(screen.getByRole("button", { name: "tree.archiveCollection" }));
  expect(storeState.archiveCollection).toHaveBeenCalledWith("c1");
});
