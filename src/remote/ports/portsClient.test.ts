import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const handlers = vi.hoisted(() => ({
  native: null as null | ((payload: unknown) => void),
}));
const transportMock = vi.hoisted(() => (over: Record<string, unknown> = {}) => ({
  isRemoteWindow: true,
  remoteSshSession: "s1",
  remoteConnectionKind: "ssh",
  emitNative: vi.fn().mockResolvedValue(undefined),
  listenNative: vi.fn(async (_event: string, callback: (payload: unknown) => void) => {
    handlers.native = callback;
    return () => {};
  }),
  ...over,
}));
vi.mock("../../ipc/transport", () => transportMock());

type Client = typeof import("./portsClient");
type Transport = typeof import("../../ipc/transport");
let client: Client;
let emitNative: Transport["emitNative"];
let listenNative: Transport["listenNative"];

const snapshot = (over: Record<string, unknown> = {}) => ({
  session: "s1", detected: [3000], forwards: [], detectionAvailable: true, ...over,
});

beforeEach(async () => {
  vi.resetModules();
  vi.clearAllMocks();
  handlers.native = null;
  ({ emitNative, listenNative } = await import("../../ipc/transport"));
  client = await import("./portsClient");
});
afterEach(() => vi.useRealTimers());

it("is supported in SSH remote windows", () => {
  expect(client.portsSupported).toBe(true);
});

it("is not supported outside SSH remote windows", async () => {
  const load = (over: Record<string, unknown>) => {
    vi.resetModules();
    vi.doMock("../../ipc/transport", () => transportMock(over));
    return import("./portsClient");
  };
  expect((await load({ isRemoteWindow: false, remoteSshSession: null })).portsSupported).toBe(false);
  expect((await load({ remoteConnectionKind: "wsl" })).portsSupported).toBe(false);
  vi.doMock("../../ipc/transport", () => transportMock());
});

it("emits requests with the window session and only the given port", async () => {
  await client.requestPorts("list");
  await client.requestPorts("forward", 3000);
  expect(listenNative).toHaveBeenCalledWith("ssh://ports-state", expect.any(Function));
  expect(emitNative).toHaveBeenNthCalledWith(1, "vlx://ports-request", { session: "s1", action: "list" });
  expect(emitNative).toHaveBeenNthCalledWith(2, "vlx://ports-request", { session: "s1", action: "forward", rport: 3000 });
});

it("keeps the last snapshot for this session and ignores other sessions", async () => {
  const { result } = renderHook(() => client.usePorts());
  await act(() => client.requestPorts("list"));
  act(() => handlers.native?.(snapshot({ session: "other", detected: [9999] })));
  expect(result.current.snapshot).toBeNull();
  act(() => handlers.native?.(snapshot()));
  expect(result.current.snapshot?.detected).toEqual([3000]);
});

it("reports unavailable when no snapshot arrives within the timeout", async () => {
  vi.useFakeTimers();
  const { result } = renderHook(() => client.usePorts());
  await act(() => client.requestPorts("list"));
  act(() => vi.advanceTimersByTime(client.UNAVAILABLE_AFTER_MS - 1));
  expect(result.current.unavailable).toBe(false);
  act(() => vi.advanceTimersByTime(1));
  expect(result.current.unavailable).toBe(true);
});

it("does not report unavailable when a snapshot arrives in time", async () => {
  vi.useFakeTimers();
  const { result } = renderHook(() => client.usePorts());
  await act(() => client.requestPorts("list"));
  act(() => handlers.native?.(snapshot()));
  act(() => vi.advanceTimersByTime(client.UNAVAILABLE_AFTER_MS * 2));
  expect(result.current.unavailable).toBe(false);
});

it("keeps an error through later polls until the next user action", async () => {
  const { result } = renderHook(() => client.usePorts());
  await act(() => client.requestPorts("forward", 3000));
  act(() => handlers.native?.(snapshot({ error: "bind failed" })));
  expect(result.current.error).toBe("bind failed");
  await act(() => client.requestPorts("list"));
  act(() => handlers.native?.(snapshot()));
  expect(result.current.error).toBe("bind failed");
  await act(() => client.requestPorts("unforward", 3000));
  expect(result.current.error).toBeNull();
});

it("never rejects when the native bus fails", async () => {
  vi.mocked(emitNative).mockRejectedValueOnce(new Error("no bus"));
  await expect(client.requestPorts("list")).resolves.toBeUndefined();
});
