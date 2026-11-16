import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { PermissionSeg } from "./StatusBar";
import { useTermStore } from "../../store/termStore";
import { invoke } from "../../ipc/transport";
import { PERMISSION_TOGGLE_KINDS, type Session } from "../../types";
import type { SessionPermissionState } from "../../hooks/useSessionPermissionState";

vi.mock("../../ipc/transport", async original => ({
  ...await original<typeof import("../../ipc/transport")>(), invoke: vi.fn(),
  listen: vi.fn().mockResolvedValue(() => {}),
  onTransportReconnect: () => () => {}, onTransportDisconnect: () => () => {},
}));

let state: SessionPermissionState;
const update = vi.fn();
const restart = vi.fn();
beforeEach(() => {
  vi.mocked(invoke).mockReset();
  update.mockReset(); restart.mockReset();
  state = { configured: "default", current: null, launch: "default", pending: null,
    activation: "unconfirmed", running: true };
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === "session_permission_state" ? { ...state }
    : { modes: ["default", "bypassPermissions", "full-access"], selected: state.configured }) as Promise<never>);
  update.mockImplementation(async (id, patch) => {
    if (patch.permissionMode !== state.launch) state = { ...state, configured: patch.permissionMode,
      pending: patch.permissionMode, activation: "restart" };
    useTermStore.setState(s => ({ sessions: s.sessions.map(session => session.id === id ? { ...session, ...patch } : session) }));
  });
  restart.mockImplementation(async () => {
    state = { ...state, launch: state.configured, pending: null, activation: "unconfirmed" };
    useTermStore.setState({ runtimes: { s: { status: "exited" } } });
  });
  useTermStore.setState({ activeSessionId: "s", runtimes: { s: { status: "running" } },
    updateSession: update, restartSession: restart });
});
afterEach(cleanup);

it.each(PERMISSION_TOGGLE_KINDS)("%s prompts, retains the deferred notice, and clears it after restart", async kind => {
  useTermStore.setState({ sessions: [{ id: "s", kind, engine: "tui", name: kind, permissionMode: "default" } as Session] });
  const { container } = render(<PermissionSeg />);
  // The launch mode names the button until the agent confirms a permission.
  fireEvent.click(await screen.findByText("Always Ask"));
  expect(container.querySelector(".seg.on")).toBeNull();
  const name = kind === "codex" ? "Full Access" : ["claude", "opencode", "antigravity"].includes(kind) ? "Bypass" : "Skip all permission confirmations";
  fireEvent.click(await screen.findByRole("button", { name }));
  expect(await screen.findByRole("button", { name: "Restart now" })).toBeTruthy();
  expect(restart).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Later" }));
  expect(screen.queryByRole("button", { name: "Restart now" })).toBeNull();
  expect(screen.getByText(/Applies after restarting this session:/)).toBeTruthy();
  expect(restart).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Always Ask"));
  fireEvent.click(await screen.findByRole("button", { name }));
  fireEvent.click(await screen.findByRole("button", { name: "Restart now" }));
  await waitFor(() => expect(restart).toHaveBeenCalledWith("s"));
  await waitFor(() => expect(screen.queryByText(/Applies after restarting this session:/)).toBeNull());
  expect(container.querySelector(".seg.on")).toBeTruthy();
});

it("does not ask to restart when the backend reports the choice is already in use", async () => {
  useTermStore.setState({ sessions: [{ id: "s", kind: "copilot", engine: "tui", permissionMode: "default" } as Session] });
  render(<PermissionSeg />);
  fireEvent.click(await screen.findByText("Always Ask"));
  update.mockImplementation(async () => {});
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Ask each time (default)" })));
  expect(screen.queryByRole("button", { name: "Restart now" })).toBeNull();
  expect(restart).not.toHaveBeenCalled();
});

it("shows an unset session under the agent default and stores asking explicitly", async () => {
  useTermStore.setState({ agentDefaults: { cursor: { permissionMode: "skip" } },
    sessions: [{ id: "s", kind: "cursor", engine: "tui", permissionMode: null } as Session] });
  render(<PermissionSeg />);
  fireEvent.click(await screen.findByText("Always Ask"));
  const ask = screen.getByRole("button", { name: "Ask each time (default)" });
  expect(screen.getByRole("button", { name: "Skip all permission confirmations" }).querySelector("svg")).toBeTruthy();
  expect(ask.querySelector("svg")).toBeNull();
  update.mockImplementation(async () => {});
  await act(async () => fireEvent.click(ask));
  // Null would fall back to the skipping default again, so asking has to be written out.
  expect(update).toHaveBeenCalledWith("s", expect.objectContaining({ permissionMode: "default" }));
});
