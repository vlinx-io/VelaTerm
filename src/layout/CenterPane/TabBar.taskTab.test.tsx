import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const storeState = vi.hoisted(() => ({
  shortcutOverrides: {},
  projects: [{ id: "project-1", rootPath: "/tmp/project" }],
  groups: [],
  sessions: [
    { id: "sess-1", name: "Session One", kind: "claude", projectId: "project-1", groupId: null },
  ],
  ephemeralSessions: {},
  openTabs: ["sess-1", "task-1", "task-2"],
  activeTabId: "task-2",
  docTabs: {},
  browserTabs: {},
  taskTabs: {
    "task-1": { id: "task-1", sessionId: "sess-1", taskId: "w1", title: "protocol probe", taskType: "local_workflow" },
    "task-2": { id: "task-2", sessionId: "sess-1", taskId: "sh1", title: "npm test", taskType: "local_bash" },
  },
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

vi.mock("../../i18n", () => ({ useT: () => (key: string) => key, dateLocale: () => "en" }));
vi.mock("../../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  );
  return { useTermStore };
});
vi.mock("../../components/Icons", () => ({
  default: new Proxy({}, { get: (_target, name) => () => <i data-icon={String(name)} /> }),
}));
vi.mock("../../components/StatusIndicator", () => ({ StatusIndicator: () => null }));
vi.mock("../../components/ContextMenu", () => ({ ContextMenu: () => null }));
vi.mock("../sessionMenu", () => ({
  useSessionMenu: () => ({
    buildSessionItems: vi.fn(() => []),
    buildScratchItems: vi.fn(() => []),
    shellSwitchItems: vi.fn(() => []),
    dialogs: null,
  }),
}));
vi.mock("../sessionViewers/sessionMeta", () => ({ SessionKindIcon: () => null }));
vi.mock("../../ipc/transport", () => ({ isTauri: false }));
vi.mock("../../platform", () => ({ env: { isElectron: false } }));
vi.mock("../../hooks/shortcutRegistry", () => ({ labelWithCombo: (label: string) => label }));
vi.mock("../../hooks/useKeyboardShortcuts", () => ({ DOC_EXPORT_PDF_EVENT: "doc-export-pdf" }));

import { TabBar } from "./TabBar";

/** Tab root carrying the title. */
function tab(name: string): HTMLElement {
  const el = screen.getByText(name).closest(".tab");
  if (!el) throw new Error(`no tab root for ${name}`);
  return el as HTMLElement;
}

describe("TabBar task tabs", () => {
  beforeEach(() => {
    Element.prototype.scrollIntoView = vi.fn();
    storeState.closeTab.mockClear();
    storeState.setActiveTab.mockClear();
  });

  it("renders every task tab with its title, an icon for its kind, and the active accent", () => {
    render(<TabBar />);
    expect(tab("protocol probe").querySelector("[data-icon=layers]")).toBeTruthy();
    expect(tab("npm test").querySelector("[data-icon=terminal]")).toBeTruthy();
    expect(tab("npm test").classList.contains("on")).toBe(true);
    expect(tab("protocol probe").classList.contains("on")).toBe(false);
    expect(tab("protocol probe").getAttribute("title")).toBe("chat.tasks.tabTooltip");
  });

  it("activates on click and closes through the store on its close action", () => {
    render(<TabBar />);
    fireEvent.click(tab("protocol probe"));
    expect(storeState.setActiveTab).toHaveBeenCalledWith("task-1");

    fireEvent.click(tab("protocol probe").querySelector(".x") as HTMLElement);
    expect(storeState.closeTab).toHaveBeenCalledWith("task-1");
    expect(storeState.setActiveTab).toHaveBeenCalledTimes(1);
  });

  it("closes on middle click like a browser tab", () => {
    render(<TabBar />);
    fireEvent.mouseDown(tab("npm test"), { button: 1 });
    fireEvent.mouseUp(tab("npm test"), { button: 1 });
    expect(storeState.closeTab).toHaveBeenCalledWith("task-2");
  });
});
