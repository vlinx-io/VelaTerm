//! Covers runtime propagation of preference changes between shells.
//!
//! Before this, preferences were reconciled once at startup: a change made in the browser stayed
//! invisible on the desktop until its next launch. The backend now broadcasts after every write, and
//! these tests pin the two properties that make following it safe — pending local writes are flushed
//! first, so a broadcast inside the debounce window cannot revert a value this shell just set, and the
//! writer's own echo is a no-op rather than a second round of work.

import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("../ipc/transport", () => ({
  invoke,
  isRemoteWindow: false,
  listen: vi.fn(),
  listenNative: vi.fn(),
}));

const { onSettingsChanged } = vi.hoisted(() => ({ onSettingsChanged: vi.fn() }));
vi.mock("../ipc/events", () => ({ onSettingsChanged }));

const { setLang, loadLangChoice } = vi.hoisted(() => ({
  setLang: vi.fn(),
  loadLangChoice: vi.fn(() => "ja"),
}));
vi.mock("../i18n", () => ({ setLang, loadLangChoice }));

const { hydrateSettingsFromCache } = vi.hoisted(() => ({
  hydrateSettingsFromCache: vi.fn(),
}));
vi.mock("./termStore", () => ({
  useTermStore: { getState: () => ({ hydrateSettingsFromCache }) },
}));

import { CONVERSATION_VIEW_HINT_KEY } from "../ipc/conversationViewHint";
import { pushSetting, reconcileSettings } from "../ipc/settingsSync";
import { refreshSettingsFromBackend, startSettingsWatch } from "./settingsWatch";

/** Enable outbound propagation the way startup does; `pushSetting` is gated on reconciliation. */
async function enableSync() {
  invoke.mockResolvedValueOnce({});
  await reconcileSettings();
  invoke.mockReset();
}

beforeEach(() => {
  invoke.mockReset();
  onSettingsChanged.mockReset();
  setLang.mockReset();
  hydrateSettingsFromCache.mockReset();
  localStorage.clear();
});

describe("refreshSettingsFromBackend", () => {
  it("applies a language chosen in another shell", async () => {
    localStorage.setItem("vlx-lang", "en");
    invoke.mockResolvedValueOnce({ "vlx-lang": "ja" });

    await refreshSettingsFromBackend();

    expect(localStorage.getItem("vlx-lang")).toBe("ja");
    expect(setLang).toHaveBeenCalledWith("ja");
  });

  it("re-reads the store's cached settings when a visual key changed", async () => {
    localStorage.setItem("vlx-theme", "light");
    invoke.mockResolvedValueOnce({ "vlx-theme": "dark" });

    await refreshSettingsFromBackend();

    expect(hydrateSettingsFromCache).toHaveBeenCalledTimes(1);
  });

  it("does nothing when the backend agrees with the local cache", async () => {
    // The writer hears its own broadcast. Reconciliation finds no difference, so this must be inert
    // rather than a second application of settings that are already in effect.
    localStorage.setItem("vlx-theme", "dark");
    invoke.mockResolvedValueOnce({ "vlx-theme": "dark" });

    await refreshSettingsFromBackend();

    expect(setLang).not.toHaveBeenCalled();
    expect(hydrateSettingsFromCache).not.toHaveBeenCalled();
  });

  it("flushes a pending local write before re-reading", async () => {
    // A broadcast arriving inside pushSetting's debounce window would otherwise reconcile against a
    // backend that has not seen this shell's newest value, revert the local cache to the older one,
    // and flip back when the debounce finally fires.
    await enableSync();
    localStorage.setItem("vlx-theme", "dark");
    pushSetting("vlx-theme", "dark");

    const calls: string[] = [];
    invoke.mockImplementation((cmd: string) => {
      calls.push(cmd);
      return Promise.resolve(cmd === "get_app_settings" ? { "vlx-theme": "dark" } : undefined);
    });

    await refreshSettingsFromBackend();

    expect(calls[0]).toBe("set_app_settings"); // the pending write lands first
    expect(calls).toContain("get_app_settings");
    expect(localStorage.getItem("vlx-theme")).toBe("dark");
  });

  it("caches conversation view hint dismissal from the backend without re-hydrating the store", async () => {
    invoke.mockResolvedValueOnce({ [CONVERSATION_VIEW_HINT_KEY]: "1" });

    await refreshSettingsFromBackend();

    expect(localStorage.getItem(CONVERSATION_VIEW_HINT_KEY)).toBe("1");
    expect(hydrateSettingsFromCache).not.toHaveBeenCalled();
  });
});

describe("startSettingsWatch", () => {
  it("reconciles on each broadcast and stops listening when torn down", async () => {
    const off = vi.fn();
    let fire: (() => void) | undefined;
    onSettingsChanged.mockImplementation((cb: () => void) => {
      fire = cb;
      return Promise.resolve(off);
    });

    const stop = startSettingsWatch();
    expect(onSettingsChanged).toHaveBeenCalledTimes(1);

    localStorage.setItem("vlx-theme", "light");
    invoke.mockResolvedValueOnce({ "vlx-theme": "dark" });
    fire?.();
    await vi.waitFor(() => expect(hydrateSettingsFromCache).toHaveBeenCalled());

    stop();
    await vi.waitFor(() => expect(off).toHaveBeenCalledTimes(1));
  });

  it("survives a listener that fails to register", async () => {
    // Without runtime propagation the app still works off its startup reconciliation, so a failed
    // registration must not surface as an unhandled rejection.
    onSettingsChanged.mockRejectedValueOnce(new Error("transport down"));

    const stop = startSettingsWatch();
    expect(() => stop()).not.toThrow();
  });
});

describe("theme choices during startup", () => {
  async function freshWatch() {
    vi.resetModules();
    return import("./settingsWatch");
  }

  it("applies a queued choice after hydration and preserves backend preferences", async () => {
    const watch = await freshWatch();
    const sync = await import("../ipc/settingsSync");
    localStorage.setItem("vlx-settings", JSON.stringify({ darkStyle: "soft", accent: "green" }));
    let answer: (settings: Record<string, string>) => void = () => {};
    invoke.mockReturnValueOnce(new Promise<Record<string, string>>((resolve) => { answer = resolve; }));
    const order: string[] = [];
    hydrateSettingsFromCache.mockImplementationOnce(() => { order.push("hydrate"); });
    const refresh = watch.refreshSettingsFromBackend();
    watch.runAfterInitialSettings(() => {
      order.push("choose");
      const settings = JSON.parse(localStorage.getItem("vlx-settings")!);
      const json = JSON.stringify({ ...settings, darkStyle: "classic" });
      localStorage.setItem("vlx-settings", json);
      sync.pushSetting("vlx-settings", json);
    });
    expect(order).toEqual([]);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("get_app_settings"));
    answer({ "vlx-settings": JSON.stringify({ darkStyle: "soft", accent: "blue" }) });
    await refresh;
    invoke.mockResolvedValueOnce(undefined);
    await sync.flushNow();

    expect(order).toEqual(["hydrate", "choose"]);
    expect(JSON.parse(localStorage.getItem("vlx-settings")!)).toEqual({ darkStyle: "classic", accent: "blue" });
    expect(invoke).toHaveBeenLastCalledWith("set_app_settings", {
      entries: { "vlx-settings": JSON.stringify({ darkStyle: "classic", accent: "blue" }) },
    });
  });

  it("waits for overlapping initial reads before applying the latest choice", async () => {
    const watch = await freshWatch();
    const answers: Array<(settings: Record<string, string>) => void> = [];
    invoke.mockImplementation(() => new Promise<Record<string, string>>((resolve) => { answers.push(resolve); }));
    const first = watch.refreshSettingsFromBackend();
    const second = watch.refreshSettingsFromBackend();
    const choose = vi.fn();
    watch.runAfterInitialSettings(() => choose("classic"));
    watch.runAfterInitialSettings(() => choose("soft"));
    await vi.waitFor(() => expect(answers).toHaveLength(2));
    answers[1]({});
    await second;
    expect(choose).not.toHaveBeenCalled();
    answers[0]({});
    await first;

    expect(choose.mock.calls).toEqual([["classic"], ["soft"]]);
  });

  it("applies later choices immediately after startup with an unchanged cache", async () => {
    const watch = await freshWatch();
    invoke.mockResolvedValueOnce({});
    await watch.refreshSettingsFromBackend();
    const choose = vi.fn();
    watch.runAfterInitialSettings(choose);

    expect(choose).toHaveBeenCalledTimes(1);
  });

  it("releases a queued choice when the initial backend read fails", async () => {
    const watch = await freshWatch();
    invoke.mockRejectedValueOnce(new Error("offline"));
    const refresh = watch.refreshSettingsFromBackend();
    const choose = vi.fn();
    watch.runAfterInitialSettings(choose);
    await refresh;

    expect(choose).toHaveBeenCalledTimes(1);
  });
});
