// @vitest-environment jsdom
//! Regression coverage for App hook order when switching between routes that show and hide the right panel.

import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { memoryNavigate, memoryUrl } from "./layout/Memory/navigation";
import { useTermStore } from "./store/termStore";

vi.mock("./components/ClipboardNotice", () => ({ ClipboardNotice: () => null }));
vi.mock("./components/GitbashDownloadBanner", () => ({ GitbashDownloadBanner: () => null }));
vi.mock("./components/ErrorLogModal", () => ({ ErrorLogModal: () => null }));
vi.mock("./components/Splitter", () => ({ Splitter: () => null }));
vi.mock("./components/diff/changesRoute", () => ({ ChangesRoute: () => null }));
vi.mock("./components/NotifyGuideModal", () => ({ NotifyGuideModal: () => null }));
vi.mock("./components/QuitConfirmModal", () => ({ QuitConfirmModal: () => null }));
vi.mock("./components/VelaSkillsModal", () => ({ VelaSkillsModal: () => null }));
vi.mock("./hooks/useKeyboardShortcuts", () => ({ useKeyboardShortcuts: () => {} }));
vi.mock("./hooks/useNotifications", () => ({ useNotifications: () => {} }));
vi.mock("./layout/CenterPane/CenterPane", () => ({ CenterPane: () => <div data-testid="center-pane" /> }));
vi.mock("./layout/LeftSidebar/LeftSidebar", () => ({ LeftSidebar: () => null }));
vi.mock("./layout/LeftSidebar/SidebarRail", () => ({ SidebarRail: () => null }));
vi.mock("./layout/RightPanel/RightPanel", () => ({ RightPanel: () => <div data-testid="right-panel" /> }));
vi.mock("./layout/StatusBar/StatusBar", () => ({ StatusBar: () => null }));
vi.mock("./layout/TitleBar/TitleBar", () => ({ TitleBar: () => null }));
vi.mock("./layout/TitleBar/appMenuActions", () => ({ runMenuAction: () => {} }));
vi.mock("./ipc/commands", () => ({ listShells: vi.fn().mockResolvedValue([]) }));
vi.mock("./ipc/events", () => ({
  onMenuAction: vi.fn().mockResolvedValue(() => {}),
  onPresetsChanged: vi.fn().mockResolvedValue(() => {}),
  onSessionState: vi.fn().mockResolvedValue(() => {}),
  onTreeChanged: vi.fn().mockResolvedValue(() => {}),
  onViewRequest: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("./store/settingsWatch", () => ({
  refreshSettingsFromBackend: vi.fn().mockResolvedValue(undefined),
  startSettingsWatch: () => () => {},
}));
vi.mock("./ipc/transport", () => ({
  diagnosticEvent: vi.fn(),
  isTauri: true,
}));
vi.mock("./ipc/spawnRecovery", () => ({ startSpawnRecovery: () => () => {} }));
vi.mock("./ipc/shareBase", () => ({ isShareSurface: false }));
vi.mock("./sharing/useSharedSessionNavigation", () => ({ useSharedSessionNavigation: () => {} }));
vi.mock("./layout/CenterPane/session/useHistorySessionNavigation", () => ({ useHistorySessionNavigation: () => {} }));
vi.mock("./ExportProgressDialog", () => ({ ExportProgressDialog: () => null }));
vi.mock("./ipc/wsClient", () => ({ wsClient: { onConnState: () => () => {} } }));
vi.mock("./ipc/updater", () => ({
  startUpdateSchedule: () => () => {},
  useUpdateState: () => ({ prompt: null, modalOpen: false }),
}));
vi.mock("./platform", () => ({
  env: { isElectron: false, isTauri: true },
  platform: {
    opener: { openExternal: vi.fn() },
    window: {
      takeOpenProjectRequest: vi.fn().mockResolvedValue(null),
      onOpenProjectRequest: vi.fn().mockResolvedValue(() => {}),
    },
  },
}));
vi.mock("./remote/ConnectionBanner", () => ({ ConnectionBanner: () => null }));
vi.mock("./remote/ProjectDialogRoutes", () => ({ ProjectDialogRoutes: () => null }));
vi.mock("./layout/Assist/AssistRequests", () => ({ AssistRequestListener: () => null }));
vi.mock("./store/mirrorSync", () => ({ startMirrorSync: () => () => {} }));
vi.mock("./store/usageSync", () => ({ startUsageSync: () => () => {} }));
vi.mock("./theme", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./theme")>()),
  watchSystemTheme: () => () => {},
}));
vi.mock("./plugins/host", () => ({
  initPluginHost: vi.fn(),
  pluginRouteHidesRightPanel: () => false,
}));

import App from "./App";

describe("App route hook stability", () => {
  beforeEach(() => {
    window.history.replaceState(null, "", "/");
    useTermStore.setState({
      leftCollapsed: false,
      rightCollapsed: false,
      loadTree: vi.fn().mockResolvedValue(undefined),
      loadAgentPresets: vi.fn().mockResolvedValue(undefined),
      syncSessionStates: vi.fn().mockResolvedValue(undefined),
      applyAppearance: vi.fn(),
    });
  });

  afterEach(() => {
    cleanup();
    window.history.replaceState(null, "", "/");
  });

  it("keeps hook order stable across knowledge routes that toggle the right panel", () => {
    render(<App />);
    expect(screen.getByTestId("right-panel")).toBeTruthy();

    // Navigating from a regular tab to ?memory=notebooks flips memoryRouteHidesRightPanel false -> true.
    act(() => {
      memoryNavigate(memoryUrl("notebooks"));
    });
    expect(screen.queryByTestId("right-panel")).toBeNull();

    // Opening an entry detail flips memoryRouteHidesRightPanel true -> false.
    act(() => {
      memoryNavigate(memoryUrl("entry/e1"));
    });
    expect(screen.getByTestId("right-panel")).toBeTruthy();

    // Returning to the overview flips false -> true again.
    act(() => {
      memoryNavigate(memoryUrl("notebooks"));
    });
    expect(screen.queryByTestId("right-panel")).toBeNull();

    // Closing the knowledge route restores the right panel.
    act(() => {
      memoryNavigate(memoryUrl(""));
    });
    expect(screen.getByTestId("right-panel")).toBeTruthy();
  });
});
