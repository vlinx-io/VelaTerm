import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const handlers = vi.hoisted(() => ({
  native: null as null | ((payload: unknown) => void),
}));
vi.mock("../../ipc/transport", async (original) => ({
  ...(await original<typeof import("../../ipc/transport")>()),
  isRemoteWindow: true,
  remoteSshSession: "s1",
  remoteConnectionKind: "ssh",
  emitNative: vi.fn().mockResolvedValue(undefined),
  listenNative: vi.fn(async (_event: string, callback: (payload: unknown) => void) => {
    handlers.native = callback;
    return () => {};
  }),
}));

import { emitNative } from "../../ipc/transport";

let PortsTab: typeof import("./PortsTab").PortsTab;
let POLL_MS: number;
let UNAVAILABLE_AFTER_MS: number;

const push = (over: Record<string, unknown> = {}) =>
  act(() => handlers.native?.({
    session: "s1", detected: [3000], forwards: [], detectionAvailable: true, ...over,
  }));

const requests = () => vi.mocked(emitNative).mock.calls.map(([, payload]) => payload);

beforeEach(async () => {
  vi.resetModules();
  vi.clearAllMocks();
  handlers.native = null;
  ({ PortsTab, POLL_MS } = await import("./PortsTab"));
  ({ UNAVAILABLE_AFTER_MS } = await import("./portsClient"));
  (await import("../../i18n")).setLang("en");
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

it("lists on mount and forwards a detected port", async () => {
  render(<PortsTab />);
  await act(async () => {});
  expect(requests()).toEqual([{ session: "s1", action: "list" }]);
  push();
  fireEvent.click(screen.getByRole("button", { name: "Forward 3000" }));
  await act(async () => {});
  expect(requests().at(-1)).toEqual({ session: "s1", action: "forward", rport: 3000 });
});

it("shows active forwards, highlights a changed local port, and opens or stops them", async () => {
  render(<PortsTab />);
  await act(async () => {});
  push({ detected: [], forwards: [
    { rport: 3000, lport: 3000, source: "detected" },
    { rport: 8080, lport: 51234, source: "manual" },
  ] });
  expect(screen.getByText("localhost:3000").className).not.toContain("accent");
  const moved = screen.getByText("localhost:51234");
  expect(moved.className).toContain("accent");
  expect(moved.getAttribute("title")).toBe("Local port 8080 was busy, so another local port is used");
  fireEvent.click(screen.getByRole("button", { name: "Open in browser 8080" }));
  fireEvent.click(screen.getByRole("button", { name: "Stop 3000" }));
  await act(async () => {});
  expect(requests().slice(-2)).toEqual([
    { session: "s1", action: "open", rport: 8080 },
    { session: "s1", action: "unforward", rport: 3000 },
  ]);
});

it("forwards a typed port and rejects invalid input", async () => {
  render(<PortsTab />);
  await act(async () => {});
  push({ detected: [] });
  const input = screen.getByRole("textbox", { name: "Remote port" });
  const submit = screen.getByRole("button", { name: "Forward" });
  for (const bad of ["", "0", "70000", "80a", "-1"]) {
    fireEvent.change(input, { target: { value: bad } });
    expect((submit as HTMLButtonElement).disabled).toBe(true);
  }
  fireEvent.change(input, { target: { value: "5432" } });
  fireEvent.click(submit);
  await act(async () => {});
  expect(requests().at(-1)).toEqual({ session: "s1", action: "forward", rport: 5432 });
  expect((input as HTMLInputElement).value).toBe("");
});

it("keeps manual forwarding when detection is unavailable and shows request errors", async () => {
  render(<PortsTab />);
  await act(async () => {});
  push({ detected: [], detectionAvailable: false, error: "port 3000 is not forwarded" });
  expect(screen.getByText("Port detection is not available on this host")).toBeTruthy();
  expect(screen.getByRole("alert").textContent).toBe("port 3000 is not forwarded");
  expect(screen.getByRole("textbox", { name: "Remote port" })).toBeTruthy();
});

it("polls while mounted and stops after unmount", async () => {
  vi.useFakeTimers();
  const view = render(<PortsTab />);
  await act(async () => {});
  push();
  await act(async () => { vi.advanceTimersByTime(POLL_MS); });
  expect(requests().filter((r) => (r as { action: string }).action === "list")).toHaveLength(2);
  view.unmount();
  await act(async () => { vi.advanceTimersByTime(POLL_MS * 3); });
  expect(requests().filter((r) => (r as { action: string }).action === "list")).toHaveLength(2);
});

it("tells the user to update when the local app never answers", async () => {
  vi.useFakeTimers();
  render(<PortsTab />);
  await act(async () => {});
  await act(async () => { vi.advanceTimersByTime(UNAVAILABLE_AFTER_MS); });
  expect(screen.getByText("Unavailable — update VelaTerm")).toBeTruthy();
});
