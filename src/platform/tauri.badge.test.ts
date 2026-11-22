import { beforeEach, describe, expect, it, vi } from "vitest";

const emitMock = vi.fn();
const currentWindowMock = { label: "main" };
const envState = {
  kind: "tauri" as "tauri" | "electron" | "browser",
  isTauri: true,
  isElectron: false,
  isBrowser: false,
  isRemoteWindow: false,
  hasNativeHost: true,
  isDev: false,
  isMac: true,
};

vi.mock("@tauri-apps/api/event", () => ({
  emit: (...args: unknown[]) => emitMock(...args),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => currentWindowMock,
}));

vi.mock("./env", () => ({
  env: envState,
}));

describe("tauriPlatform.badge.setCount", () => {
  beforeEach(() => {
    emitMock.mockReset();
    emitMock.mockResolvedValue(undefined);
    currentWindowMock.label = "main";
    envState.kind = "tauri";
    envState.isTauri = true;
    envState.isBrowser = false;
    envState.isRemoteWindow = false;
    envState.hasNativeHost = true;
  });

  it("emits vlx://window-badge with the current label for the main window", async () => {
    const { tauriPlatform } = await import("./tauri");
    await tauriPlatform.badge.setCount(3);
    expect(emitMock).toHaveBeenCalledWith("vlx://window-badge", {
      windowLabel: "main",
      count: 3,
    });
  });

  it("emits vlx://window-badge for remote desktop windows too", async () => {
    envState.kind = "browser";
    envState.isTauri = false;
    envState.isBrowser = true;
    envState.isRemoteWindow = true;
    envState.hasNativeHost = true;
    currentWindowMock.label = "ssh-1a2b3c4d";

    const { tauriPlatform } = await import("./tauri");
    await tauriPlatform.badge.setCount(2);
    expect(emitMock).toHaveBeenCalledWith("vlx://window-badge", {
      windowLabel: "ssh-1a2b3c4d",
      count: 2,
    });
  });

  it("normalizes undefined, negative, and fractional counts to non-negative integers", async () => {
    const { tauriPlatform } = await import("./tauri");
    await tauriPlatform.badge.setCount(undefined);
    await tauriPlatform.badge.setCount(-4);
    await tauriPlatform.badge.setCount(2.9);

    expect(emitMock).toHaveBeenNthCalledWith(1, "vlx://window-badge", { windowLabel: "main", count: 0 });
    expect(emitMock).toHaveBeenNthCalledWith(2, "vlx://window-badge", { windowLabel: "main", count: 0 });
    expect(emitMock).toHaveBeenNthCalledWith(3, "vlx://window-badge", { windowLabel: "main", count: 2 });
  });

  it("no-ops in plain browsers without a native host", async () => {
    envState.kind = "browser";
    envState.isTauri = false;
    envState.isBrowser = true;
    envState.isRemoteWindow = false;
    envState.hasNativeHost = false;

    const { tauriPlatform } = await import("./tauri");
    await tauriPlatform.badge.setCount(5);
    expect(emitMock).not.toHaveBeenCalled();
  });

  it("silently ignores emit errors on restricted external-origin windows", async () => {
    emitMock.mockRejectedValueOnce(new Error("permission denied"));
    const { tauriPlatform } = await import("./tauri");
    await expect(tauriPlatform.badge.setCount(1)).resolves.toBeUndefined();
  });
});
