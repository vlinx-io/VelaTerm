import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import type { ProjectFolder } from "../../types";

const mocks = vi.hoisted(() => ({
  createProjectFolder: vi.fn(),
  renameProjectFolder: vi.fn(),
  deleteProjectFolder: vi.fn(),
  setProjectFolder: vi.fn(),
  folder: { id: "f-pay", name: "Payments", sortOrder: 1, collapsed: false, createdAt: 0 } as ProjectFolder,
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
  projects: [{ id: "project-1", name: "payments-web", rootPath: "/tmp/project", folderId: null as string | null }],
  projectFolders: [] as ProjectFolder[],
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
vi.mock("../../store/projectFolders", () => ({
  createProjectFolder: mocks.createProjectFolder,
  renameProjectFolder: mocks.renameProjectFolder,
  deleteProjectFolder: mocks.deleteProjectFolder,
  setProjectFolder: mocks.setProjectFolder,
}));
vi.mock("../../hooks/useGitBranch", () => ({ isWorktreeGone: () => false }));
vi.mock("../../components/Icons", () => ({
  default: new Proxy({}, { get: () => () => null }),
}));
vi.mock("../../components/ContextMenu", () => {
  interface Item {
    label: string;
    separator?: boolean;
    disabled?: boolean;
    onClick?: () => void;
    submenu?: Item[];
  }
  const renderItems = (items: Item[], onClose: () => void, prefix: string): ReactNode[] =>
    items.map((item, index) =>
      item.separator ? null : (
        <div key={`${prefix}-${index}`}>
          <button
            type="button"
            disabled={item.disabled}
            onClick={() => {
              item.onClick?.();
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
vi.mock("../../components/FormModal", async () => {
  const React = await vi.importActual<typeof import("react")>("react");
  return {
    FormModal: ({
      title,
      initial,
      submitLabel,
      onSubmit,
      onCancel,
    }: {
      title: string;
      initial?: Record<string, string>;
      submitLabel?: string;
      onSubmit: (values: Record<string, string>) => void;
      onCancel: () => void;
    }) => {
      const [value, setValue] = React.useState(initial?.name ?? "");
      return (
        <div role="dialog" aria-label={title}>
          <input aria-label="folder.name" value={value} onChange={(e) => setValue(e.target.value)} />
          <button type="button" onClick={() => onSubmit({ name: value })}>
            {submitLabel}
          </button>
          <button type="button" onClick={onCancel}>
            cancel-form
          </button>
        </div>
      );
    },
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
    openDialog: vi.fn(),
    dialogs: null,
  }),
}));
vi.mock("./ProjectTree", () => ({
  ProjectTree: ({
    onContext,
    onFolderContext,
  }: {
    onContext: (node: unknown, x: number, y: number) => void;
    onFolderContext: (folder: ProjectFolder, x: number, y: number) => void;
  }) => (
    <div>
      <button
        type="button"
        onContextMenu={(event) => {
          event.preventDefault();
          onContext(
            { kind: "project", id: "project-1", name: "payments-web", projectId: "project-1", groupId: null },
            10,
            10,
          );
        }}
      >
        Project
      </button>
      <button
        type="button"
        onContextMenu={(event) => {
          event.preventDefault();
          onFolderContext(mocks.folder, 10, 10);
        }}
      >
        Folder
      </button>
    </div>
  ),
}));
vi.mock("../GlobalSearch/GlobalSearch", () => ({ GlobalSearch: () => null }));

import { LeftSidebar } from "./LeftSidebar";

beforeEach(() => {
  vi.clearAllMocks();
  for (const fn of [mocks.createProjectFolder, mocks.renameProjectFolder, mocks.deleteProjectFolder, mocks.setProjectFolder]) {
    fn.mockResolvedValue(undefined);
  }
  storeState.projectFolders = [mocks.folder];
  storeState.projects = [{ id: "project-1", name: "payments-web", rootPath: "/tmp/project", folderId: null }];
});

describe("folder management", () => {
  it("creates a folder from the footer button with a trimmed name", () => {
    render(<LeftSidebar />);
    fireEvent.click(screen.getByRole("button", { name: "folder.new" }));
    fireEvent.change(screen.getByLabelText("folder.name"), { target: { value: "  Billing  " } });
    fireEvent.click(screen.getByRole("button", { name: "folder.create" }));
    expect(mocks.createProjectFolder).toHaveBeenCalledWith("Billing");
  });

  it("renames a folder from its context menu, prefilled with the current name", () => {
    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Folder" }));
    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    const input = screen.getByLabelText("folder.name") as HTMLInputElement;
    expect(input.value).toBe("Payments");
    fireEvent.change(input, { target: { value: "Billing" } });
    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    expect(mocks.renameProjectFolder).toHaveBeenCalledWith("f-pay", "Billing");
  });

  it("ignores a rename to a blank name", () => {
    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Folder" }));
    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    fireEvent.change(screen.getByLabelText("folder.name"), { target: { value: "   " } });
    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    expect(mocks.renameProjectFolder).not.toHaveBeenCalled();
  });

  it("asks before deleting a folder and says the projects are kept", () => {
    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Folder" }));
    fireEvent.click(screen.getByRole("button", { name: "folder.delete" }));
    expect(screen.getByText("folder.deleteTitle")).toBeTruthy();
    expect(screen.getByText("folder.deleteBody")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));
    expect(mocks.deleteProjectFolder).not.toHaveBeenCalled();

    fireEvent.contextMenu(screen.getByRole("button", { name: "Folder" }));
    fireEvent.click(screen.getByRole("button", { name: "folder.delete" }));
    fireEvent.click(screen.getByRole("button", { name: "common.delete" }));
    expect(mocks.deleteProjectFolder).toHaveBeenCalledWith("f-pay");
  });
});

describe("Move to Folder", () => {
  it("moves a loose project into the chosen folder", () => {
    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Project" }));
    expect(screen.getByRole("button", { name: "folder.moveTo" })).toBeTruthy();
    expect((screen.getByRole("button", { name: "folder.none" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Payments" }));
    expect(mocks.setProjectFolder).toHaveBeenCalledWith("project-1", "f-pay");
  });

  it("takes a project out of its folder with No Folder", () => {
    storeState.projects = [{ id: "project-1", name: "payments-web", rootPath: "/tmp/project", folderId: "f-pay" }];
    render(<LeftSidebar />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Project" }));
    expect((screen.getByRole("button", { name: "Payments" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "folder.none" }));
    expect(mocks.setProjectFolder).toHaveBeenCalledWith("project-1", null);
  });
});
