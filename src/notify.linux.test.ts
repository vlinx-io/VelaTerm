import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const emit = vi.fn();
const sendNotification = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ emit }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ label: "ssh-1234" }) }));
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: async () => true,
  requestPermission: async () => "granted",
  sendNotification,
}));

async function load(transport: { isTauri: boolean; isRemoteWindow: boolean }) {
  vi.resetModules();
  vi.doMock("./ipc/transport", () => transport);
  return import("./notify");
}

describe("Linux native notification channel", () => {
  const realAgent = navigator.userAgent;

  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(navigator, "userAgent", { value: "Mozilla/5.0 (X11; Linux x86_64)", configurable: true });
  });
  afterEach(() => {
    Object.defineProperty(navigator, "userAgent", { value: realAgent, configurable: true });
  });

  it("relays a remote window's notification with its window label so a click returns to it", async () => {
    const { notify } = await load({ isTauri: false, isRemoteWindow: true });
    await notify("session", "Title", "Body", true);
    expect(emit).toHaveBeenCalledWith("vlx://remote-notify", {
      windowLabel: "ssh-1234",
      sessionId: "session",
      title: "Title",
      body: "Body",
      sound: true,
    });
    expect(sendNotification).not.toHaveBeenCalled();
  });

  it("sends a main window notification through the native command", async () => {
    invoke.mockResolvedValue(true);
    const { notify } = await load({ isTauri: true, isRemoteWindow: false });
    await notify("session", "Title", "Body");
    expect(invoke).toHaveBeenCalledWith("native_notify", {
      sessionId: "session",
      title: "Title",
      body: "Body",
      sound: false,
    });
    expect(sendNotification).not.toHaveBeenCalled();
  });

  it("falls back to the plugin when the native command declines", async () => {
    invoke.mockResolvedValue(false);
    const { notify } = await load({ isTauri: true, isRemoteWindow: false });
    await notify("session", "Title", "Body");
    expect(sendNotification).toHaveBeenCalledOnce();
  });
});
