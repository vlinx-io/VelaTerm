import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const listeners = new Map<string, (payload: unknown) => void>();
const invoke = vi.fn();
vi.mock("../ipc/transport", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
  listen: (name: string, cb: (payload: unknown) => void) => {
    listeners.set(name, cb);
    return Promise.resolve(() => listeners.delete(name));
  },
  onTransportReconnect: () => () => {},
  onTransportDisconnect: () => () => {},
}));

import { useSessionPermissionState } from "./useSessionPermissionState";

const state = (configured: string) => ({ configured, current: null, launch: null, pending: null, activation: "applied", running: true });

beforeEach(() => {
  vi.useFakeTimers();
  listeners.clear();
  invoke.mockReset();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

it("reloads the permission state at a chat resync barrier, whose gap may have dropped a settings change", async () => {
  invoke.mockResolvedValueOnce(state("default")).mockResolvedValueOnce(state("plan"));
  const { result } = renderHook(() => useSessionPermissionState("s"));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(result.current?.value?.configured).toBe("default");
  const chat = listeners.get("chat://event/s");
  expect(chat).toBeDefined();

  await act(async () => {
    chat!({ type: "rows" });
    await vi.advanceTimersByTimeAsync(100);
  });
  expect(invoke, "an ordinary event does not reload").toHaveBeenCalledTimes(1);

  await act(async () => {
    chat!({ type: "resync" });
    await vi.advanceTimersByTimeAsync(100);
  });
  expect(invoke).toHaveBeenCalledTimes(2);
  expect(result.current?.value?.configured).toBe("plan");
});
