//! Covers cross-shell persistence and startup hydration for the conversation view switch hint.

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("../../../ipc/transport", () => ({
  invoke,
  isRemoteWindow: false,
  listen: vi.fn(),
  listenNative: vi.fn(),
}));
vi.mock("../../../ipc/events", () => ({
  onSettingsChanged: vi.fn(() => Promise.resolve(vi.fn())),
}));
vi.mock("../../../store/termStore", () => ({
  useTermStore: { getState: () => ({ hydrateSettingsFromCache: vi.fn() }) },
}));

async function loadModules() {
  vi.resetModules();
  const [hintStore, watch, comp] = await Promise.all([
    import("../../../ipc/conversationViewHint"),
    import("../../../store/settingsWatch"),
    import("./ConversationViewHint"),
  ]);
  return { ...hintStore, ...watch, ...comp };
}

beforeEach(() => {
  invoke.mockReset();
  localStorage.clear();
});

afterEach(() => {
  cleanup();
});

describe("ConversationViewHint", () => {
  it("persists dismissal to backend app_settings as well as localStorage", async () => {
    const { CONVERSATION_VIEW_HINT_KEY, ConversationViewHint, refreshSettingsFromBackend } = await loadModules();
    invoke.mockResolvedValueOnce({});
    await refreshSettingsFromBackend();
    invoke.mockResolvedValueOnce(undefined);

    render(<div className="pane"><ConversationViewHint enabled onSwitch={vi.fn()} /></div>);
    expect(screen.getByRole("note")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Close" }));

    expect(screen.queryByRole("note")).toBeNull();
    expect(localStorage.getItem(CONVERSATION_VIEW_HINT_KEY)).toBe("1");
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("set_app_settings", {
        entries: { [CONVERSATION_VIEW_HINT_KEY]: "1" },
      }),
    );
  });

  it("waits for startup reconciliation on a fresh origin so an already-dismissed hint never flashes", async () => {
    const { CONVERSATION_VIEW_HINT_KEY, ConversationViewHint, refreshSettingsFromBackend } = await loadModules();
    let answer: (settings: Record<string, string>) => void = () => {};
    invoke.mockReturnValueOnce(new Promise<Record<string, string>>((resolve) => { answer = resolve; }));

    const startup = refreshSettingsFromBackend();
    render(<div className="pane"><ConversationViewHint enabled onSwitch={vi.fn()} /></div>);

    // Electron launches on a new dynamic port with empty localStorage; stay hidden while fetching app_settings.
    expect(screen.queryByRole("note")).toBeNull();

    await act(async () => {
      answer({ [CONVERSATION_VIEW_HINT_KEY]: "1" });
      await startup;
    });

    expect(screen.queryByRole("note")).toBeNull();
    expect(localStorage.getItem(CONVERSATION_VIEW_HINT_KEY)).toBe("1");
  });

  it("appears after startup reconciliation when neither localStorage nor backend recorded dismissal", async () => {
    const { ConversationViewHint, refreshSettingsFromBackend } = await loadModules();
    let answer: (settings: Record<string, string>) => void = () => {};
    invoke.mockReturnValueOnce(new Promise<Record<string, string>>((resolve) => { answer = resolve; }));

    const startup = refreshSettingsFromBackend();
    render(<div className="pane"><ConversationViewHint enabled onSwitch={vi.fn()} /></div>);
    expect(screen.queryByRole("note")).toBeNull();

    await act(async () => {
      answer({});
      await startup;
    });

    expect(screen.getByRole("note")).toBeTruthy();
  });

  it("seeds backend app_settings from a pre-existing localStorage dismissal on startup", async () => {
    const { CONVERSATION_VIEW_HINT_KEY, ConversationViewHint, refreshSettingsFromBackend } = await loadModules();
    localStorage.setItem(CONVERSATION_VIEW_HINT_KEY, "1");
    invoke.mockResolvedValueOnce({}).mockResolvedValueOnce(undefined);

    await refreshSettingsFromBackend();
    render(<div className="pane"><ConversationViewHint enabled onSwitch={vi.fn()} /></div>);

    expect(screen.queryByRole("note")).toBeNull();
    expect(invoke).toHaveBeenLastCalledWith("set_app_settings", {
      entries: { [CONVERSATION_VIEW_HINT_KEY]: "1" },
    });
  });

  it("hides immediately when another shell dismisses the hint during the run", async () => {
    const { CONVERSATION_VIEW_HINT_KEY, ConversationViewHint, refreshSettingsFromBackend } = await loadModules();
    invoke.mockResolvedValueOnce({});
    await refreshSettingsFromBackend();

    render(<div className="pane"><ConversationViewHint enabled onSwitch={vi.fn()} /></div>);
    expect(screen.getByRole("note")).toBeTruthy();

    invoke.mockResolvedValueOnce({ [CONVERSATION_VIEW_HINT_KEY]: "1" });
    await act(async () => {
      await refreshSettingsFromBackend();
    });

    expect(screen.queryByRole("note")).toBeNull();
  });
});
