import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const platformEnv = vi.hoisted(() => ({ isElectron: false, isRemoteWindow: false }));

const storeState = vi.hoisted(() => ({
  shortcutOverrides: {},
  projects: [
    { id: "A", name: "Alpha", rootPath: "/tmp/a" },
    { id: "B", name: "Beta", rootPath: "/tmp/b" },
  ],
  groups: [],
  sessions: [
    { id: "a1", name: "Alpha shell", kind: "terminal", projectId: "A", groupId: null },
    { id: "b1", name: "Beta shell", kind: "terminal", projectId: "B", groupId: null },
  ],
  ephemeralSessions: {
    g1: { id: "g1", name: "Anywhere", kind: "terminal", projectId: "", groupId: null, global: true },
  } as Record<string, unknown>,
  openTabs: ["a1", "b1", "g1"],
  paneTrees: {},
  activeTabId: "a1",
  scopeTabsToProject: true,
  activeProjectId: "A" as string | null,
  docTabs: {},
  browserTabs: {},
  taskTabs: {},
  runtimes: {},
  notifications: {},
  setActiveTab: vi.fn(),
  closeTab: vi.fn(),
  requestCloseDocTab: vi.fn(),
  refreshDocTab: vi.fn(),
  newScratchTab: vi.fn(),
  newDocTab: vi.fn(),
  moveTabToBackground: vi.fn(),
  reorderTab: vi.fn(),
  renameScratch: vi.fn(),
  openBrowserTab: vi.fn(),
}));

vi.mock("../../i18n", () => ({ useT: () => (key: string) => key, getLocale: () => "en" }));
vi.mock("../../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  );
  return { useTermStore };
});
vi.mock("../../components/Icons", () => ({
  default: new Proxy({}, { get: (_t, name) => () => <span data-icon={String(name)} /> }),
}));
vi.mock("../../components/StatusIndicator", () => ({ StatusIndicator: () => null }));
vi.mock("../../components/ContextMenu", () => ({
  ContextMenu: ({ items }: { items: { label: string; onClick?: () => void }[] }) => (
    <div>
      {items.map((item, i) => (
        <button key={`${item.label}-${i}`} onClick={() => item.onClick?.()}>
          {item.label}
        </button>
      ))}
    </div>
  ),
}));
vi.mock("../sessionMenu", () => ({
  useSessionMenu: () => ({
    buildSessionItems: vi.fn(() => []),
    buildScratchItems: vi.fn(() => []),
    shellSwitchItems: vi.fn(() => []),
    dialogs: null,
  }),
}));
vi.mock("../sessionViewers/sessionMeta", () => ({ SessionKindIcon: () => null }));
vi.mock("../../ipc/transport", () => ({ isTauri: false, isElectronShell: false, isRemoteWindow: false, isMac: false }));
vi.mock("../../platform", () => ({ env: platformEnv }));
vi.mock("../../hooks/shortcutRegistry", () => ({ labelWithCombo: (label: string) => label }));
vi.mock("../../hooks/useKeyboardShortcuts", () => ({ DOC_EXPORT_PDF_EVENT: "doc-export-pdf" }));

import { TabBar } from "./TabBar";

const tabOf = (name: string) => screen.getByText(name).closest(".tab") as HTMLElement;

describe("TabBar project scoping", () => {
  beforeEach(() => {
    Element.prototype.scrollIntoView = vi.fn();
    storeState.scopeTabsToProject = true;
    storeState.activeProjectId = "A";
    platformEnv.isRemoteWindow = false;
    storeState.closeTab.mockClear();
    storeState.newScratchTab.mockClear();
  });

  it("shows only the active project's tabs plus global terminals", () => {
    render(<TabBar />);
    expect(screen.getByText("Alpha shell")).toBeTruthy();
    expect(screen.getByText("Anywhere")).toBeTruthy();
    expect(screen.queryByText("Beta shell")).toBeNull();
  });

  it("shows every tab when the setting is off", () => {
    storeState.scopeTabsToProject = false;
    render(<TabBar />);
    expect(screen.getByText("Beta shell")).toBeTruthy();
  });

  it("marks a global terminal with its tooltip and globe icon", () => {
    render(<TabBar />);
    const tab = tabOf("Anywhere");
    expect(tab.getAttribute("title")).toBe("tab.globalTerminal");
    expect(tab.querySelector('[data-icon="globe"]')).not.toBeNull();
    expect(tabOf("Alpha shell").querySelector('[data-icon="globe"]')).toBeNull();
  });

  it("offers the global terminal button only while tabs are scoped", () => {
    const { unmount } = render(<TabBar />);
    fireEvent.click(screen.getByTitle("tab.newGlobalTerminal"));
    expect(storeState.newScratchTab).toHaveBeenCalledWith({ global: true });
    unmount();
    storeState.scopeTabsToProject = false;
    render(<TabBar />);
    expect(screen.queryByTitle("tab.newGlobalTerminal")).toBeNull();
  });

  it("hides the global terminal button in a remote window", () => {
    platformEnv.isRemoteWindow = true;
    render(<TabBar />);
    expect(screen.queryByTitle("tab.newGlobalTerminal")).toBeNull();
  });

  it("closes only visible tabs from the context menu", () => {
    render(<TabBar />);
    fireEvent.contextMenu(tabOf("Alpha shell"));
    fireEvent.click(screen.getByText("tab.closeAll"));
    expect(storeState.closeTab.mock.calls.map(([id]) => id)).toEqual(["a1", "g1"]);
  });

  it("closes every tab from the context menu when the setting is off", () => {
    storeState.scopeTabsToProject = false;
    render(<TabBar />);
    fireEvent.contextMenu(tabOf("Alpha shell"));
    fireEvent.click(screen.getByText("tab.closeAll"));
    expect(storeState.closeTab.mock.calls.map(([id]) => id)).toEqual(["a1", "b1", "g1"]);
  });
});
