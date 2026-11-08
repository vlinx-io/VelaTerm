import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const h = vi.hoisted(() => ({
  navigate: vi.fn(() => Promise.resolve()),
  close: vi.fn(() => Promise.resolve()),
  setVisible: vi.fn(() => Promise.resolve()),
  back: vi.fn(),
  forward: vi.fn(),
  /** Callback registered by the last onBrowserPopup subscription, used to simulate a page popup. */
  popup: null as ((p: { url: string }) => void) | null,
  store: {
    applyBrowserState: vi.fn(),
    closeTab: vi.fn(),
    openBrowserTab: vi.fn(),
    setActiveTab: vi.fn(),
    browserTabs: {} as Record<string, unknown>,
    openTabs: [] as string[],
  },
}));

vi.mock("../../../i18n", () => ({
  useT: () => (key: string) => key,
}));
vi.mock("../../../ipc/browser", () => ({
  browserBack: h.back,
  browserClose: h.close,
  browserForward: h.forward,
  browserNavigate: h.navigate,
  browserOpen: vi.fn(() => Promise.resolve()),
  browserReload: vi.fn(),
  browserSetBounds: vi.fn(),
  browserSetVisible: h.setVisible,
  browserStop: vi.fn(),
  onBrowserState: vi.fn(() => Promise.resolve(() => {})),
  onBrowserPopup: vi.fn((_tabId: string, cb: (p: { url: string }) => void) => {
    h.popup = cb;
    return Promise.resolve(() => {});
  }),
}));
vi.mock("../../../platform", () => ({
  platform: { opener: { openPath: vi.fn(), revealPath: vi.fn(), openExternal: vi.fn(() => Promise.resolve()) } },
}));
vi.mock("../../../store/termStore", () => ({
  useTermStore: {
    getState: () => h.store,
  },
}));
vi.mock("../../../hooks/nativeViewSuspend", () => ({
  useNativeViewSuspended: () => false,
}));

import { platform } from "../../../platform";
import { BrowserView } from "./BrowserView";

/** Center-page metadata shared by tests: the standalone root that opened the game. */
const CENTER = {
  id: "browser-center",
  url: "https://velaterm.com/zh-CN/game-center",
  title: "Game Center",
  loading: false,
  chromeHidden: true,
};

const GAME = {
  id: "browser-game",
  url: "https://velaterm.com/games/shooter/index.html",
  title: "PIXEL WING",
  loading: false,
  chromeHidden: true,
  openerTabId: CENTER.id,
  openerUrl: CENTER.url,
};

describe("BrowserView quick access integration", () => {
  beforeEach(() => {
    // jsdom has no ResizeObserver; a visible BrowserView observes its placeholder for native-view bounds.
    vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
    h.popup = null;
    h.store.browserTabs = { [CENTER.id]: CENTER, [GAME.id]: GAME };
    h.store.openTabs = [CENTER.id, GAME.id];
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    h.navigate.mockClear();
    h.close.mockClear();
    h.setVisible.mockClear();
    h.back.mockClear();
    h.forward.mockClear();
    h.store.openBrowserTab.mockClear();
    h.store.closeTab.mockClear();
    h.store.setActiveTab.mockClear();
  });

  it("routes a shortcut through browserNavigate for the current tab", async () => {
    render(
      <BrowserView
        hidden
        tab={{ id: "browser-test", url: "about:blank", title: "New Tab", loading: false, chromeHidden: false }}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "ChatGPT", hidden: true }));

    await waitFor(() => {
      expect(h.navigate).toHaveBeenCalledWith("browser-test", "https://chatgpt.com/");
    });
  });

  it("renders a compact toolbar without an address bar or quick access on a standalone page", () => {
    render(<BrowserView hidden={false} tab={CENTER} />);

    expect(screen.queryByPlaceholderText("browser.addressPlaceholder")).toBeNull();
    expect(screen.queryByRole("button", { name: "ChatGPT" })).toBeNull();
    expect(screen.getByTitle("browser.back")).toBeTruthy();
    expect(screen.getByTitle("browser.forward")).toBeTruthy();
    expect(screen.getByTitle("browser.reload")).toBeTruthy();
  });

  it("falls back to page history when a standalone page has no opener", () => {
    render(<BrowserView hidden={false} tab={CENTER} />);

    fireEvent.click(screen.getByTitle("browser.back"));

    expect(h.store.setActiveTab).not.toHaveBeenCalled();
    expect(h.navigate).not.toHaveBeenCalled();
    expect(h.back).toHaveBeenCalledWith(CENTER.id);
  });

  it("returns to the opener and closes the standalone child tab with the back control", () => {
    render(<BrowserView hidden={false} tab={GAME} />);

    fireEvent.click(screen.getByTitle("browser.back"));

    expect(h.store.setActiveTab).toHaveBeenCalledWith(CENTER.id);
    expect(h.store.closeTab).toHaveBeenCalledWith(GAME.id);
    expect(h.navigate).not.toHaveBeenCalled();
  });

  it("navigates to the opener URL when the opener tab is already closed", () => {
    h.store.openTabs = [GAME.id];
    render(<BrowserView hidden={false} tab={GAME} />);

    fireEvent.click(screen.getByTitle("browser.back"));

    expect(h.navigate).toHaveBeenCalledWith(GAME.id, CENTER.url);
    expect(h.store.closeTab).not.toHaveBeenCalled();
    expect(h.store.setActiveTab).not.toHaveBeenCalled();
  });

  it("opens a game in the center tab and keeps back and forward in that tab", () => {
    const { rerender } = render(<BrowserView hidden={false} tab={CENTER} />);
    expect(h.popup).not.toBeNull();

    h.popup!({ url: GAME.url });

    expect(h.navigate).toHaveBeenCalledWith(CENTER.id, GAME.url);
    expect(h.store.openBrowserTab).not.toHaveBeenCalled();

    rerender(<BrowserView hidden={false} tab={{ ...CENTER, url: GAME.url, title: GAME.title }} />);
    fireEvent.click(screen.getByTitle("browser.back"));
    fireEvent.click(screen.getByTitle("browser.forward"));

    expect(h.back).toHaveBeenCalledWith(CENTER.id);
    expect(h.forward).toHaveBeenCalledWith(CENTER.id);
    expect(h.store.closeTab).not.toHaveBeenCalled();
    expect(h.store.setActiveTab).not.toHaveBeenCalled();
  });

  it("opens Claude artifact popups in the system browser", () => {
    render(<BrowserView tab={CENTER} hidden={false} />);

    h.popup!({ url: "https://claude.ai/artifact/MFRHaqC6HDcutPKorGdXR2" });

    expect(platform.opener.openExternal).toHaveBeenCalledWith("https://claude.ai/artifact/MFRHaqC6HDcutPKorGdXR2");
    expect(h.store.openBrowserTab).not.toHaveBeenCalled();
    expect(h.navigate).not.toHaveBeenCalled();
  });

  it("keeps ordinary browser popups in new tabs linked to their opener", () => {
    const parent = { ...CENTER, chromeHidden: false };
    h.store.browserTabs[CENTER.id] = parent;
    render(<BrowserView hidden={false} tab={parent} />);

    h.popup!({ url: GAME.url });

    expect(h.store.openBrowserTab).toHaveBeenCalledWith(GAME.url, {
      chromeHidden: false,
      openerTabId: CENTER.id,
      openerUrl: CENTER.url,
    });
    expect(h.navigate).not.toHaveBeenCalled();
  });
});
