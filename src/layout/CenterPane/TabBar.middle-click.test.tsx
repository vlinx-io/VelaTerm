import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const storeState = vi.hoisted(() => ({
  shortcutOverrides: {},
  projects: [{ id: "project-1", rootPath: "/tmp/project" }],
  groups: [],
  sessions: [
    { id: "sess-1", name: "Session One", kind: "terminal", projectId: "project-1", groupId: null },
    { id: "sess-2", name: "Session Two", kind: "terminal", projectId: "project-1", groupId: null },
  ],
  ephemeralSessions: {},
  openTabs: ["sess-1", "sess-2", "doc-1"],
  activeTabId: "sess-1",
  docTabs: {
    "doc-1": { path: "/tmp/project/notes.md", title: "notes.md", kind: "text", dirty: true },
  } as Record<string, unknown>,
  browserTabs: {},
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

vi.mock("../../i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("../../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  );
  return { useTermStore };
});
vi.mock("../../components/Icons", () => ({
  default: new Proxy({}, { get: () => () => null }),
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

const MIDDLE = { button: 1 };

/** Tab root carrying the title, which is the element the middle-click handlers are attached to. */
function tab(name: string): HTMLElement {
  const el = screen.getByText(name).closest(".tab");
  if (!el) throw new Error(`no tab root for ${name}`);
  return el as HTMLElement;
}

describe("TabBar middle-click close", () => {
  beforeEach(() => {
    // jsdom has no layout, so the tab bar's scroll-into-view of the active tab needs a stub.
    Element.prototype.scrollIntoView = vi.fn();
    storeState.closeTab.mockClear();
    storeState.requestCloseDocTab.mockClear();
    storeState.setActiveTab.mockClear();
  });

  it("closes a session tab when the middle button goes down and up on it", () => {
    render(<TabBar />);
    const target = tab("Session Two");
    fireEvent.mouseDown(target, MIDDLE);
    fireEvent.mouseUp(target, MIDDLE);

    expect(storeState.closeTab).toHaveBeenCalledWith("sess-2");
    expect(storeState.setActiveTab).not.toHaveBeenCalled();
  });

  it("routes a dirty document tab through its unsaved-changes confirmation", () => {
    render(<TabBar />);
    const target = tab("notes.md");
    fireEvent.mouseDown(target, MIDDLE);
    fireEvent.mouseUp(target, MIDDLE);

    expect(storeState.requestCloseDocTab).toHaveBeenCalledWith("doc-1");
    expect(storeState.closeTab).not.toHaveBeenCalled();
  });

  it("swallows the press so the platform default never runs", () => {
    render(<TabBar />);
    const prevented = !fireEvent.mouseDown(tab("Session One"), MIDDLE);
    expect(prevented).toBe(true);
  });

  it("cancels when the button is released on a different tab", () => {
    render(<TabBar />);
    fireEvent.mouseDown(tab("Session One"), MIDDLE);
    fireEvent.mouseUp(tab("Session Two"), MIDDLE);

    expect(storeState.closeTab).not.toHaveBeenCalled();
  });

  it("cancels when the pointer leaves the bar before release", () => {
    const { container } = render(<TabBar />);
    const target = tab("Session One");
    fireEvent.mouseDown(target, MIDDLE);
    fireEvent.mouseLeave(container.querySelector(".tabbar") as HTMLElement);
    fireEvent.mouseUp(target, MIDDLE);

    expect(storeState.closeTab).not.toHaveBeenCalled();
  });

  it("leaves other buttons alone", () => {
    render(<TabBar />);
    const target = tab("Session Two");
    fireEvent.mouseDown(target, { button: 0 });
    fireEvent.mouseUp(target, { button: 0 });

    expect(storeState.closeTab).not.toHaveBeenCalled();
  });
});
