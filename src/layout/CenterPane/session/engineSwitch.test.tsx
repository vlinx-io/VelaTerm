import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useTermStore } from "../../../store/termStore";
import type { Session } from "../../../types";
import { useEngineSwitch } from "./engineSwitch";

const session = { id: "s", projectId: "p", name: "Agent", kind: "claude", engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 } as Session;
let switchTo: ReturnType<typeof useEngineSwitch>["switchTo"];

function Harness() {
  const engine = useEngineSwitch(session);
  switchTo = engine.switchTo;
  return <>{engine.confirm}</>;
}

const setMode = vi.fn(() => Promise.resolve());
// The engine a fresh tree read finds on the server; null makes the read itself fail.
let serverEngine: Session["engine"] | null;
const loadTree = vi.fn(async () => {
  if (serverEngine === null) throw new Error("offline");
  useTermStore.setState({ sessions: [{ ...session, engine: serverEngine }] });
});
beforeEach(() => {
  setMode.mockReset();
  setMode.mockImplementation(() => Promise.resolve());
  serverEngine = "chat";
  loadTree.mockClear();
  useTermStore.setState({ runtimes: {}, sessions: [session], setSessionEngineMode: setMode, loadTree });
});
afterEach(cleanup);

describe("handing a command to the terminal view", () => {
  it("leaves the command for the terminal, without its line ends, and reports the switch as done", async () => {
    const onDone = vi.fn();
    render(<Harness />);
    await act(async () => switchTo("tui", { prefill: "/status\r\n", onDone }));
    expect(setMode).toHaveBeenCalledWith("s", "tui");
    expect(useTermStore.getState().runtimes.s?.agentPrefill).toBe("/status");
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it("ignores a second switch while one is running, even for a working agent", async () => {
    setMode.mockImplementation(() => new Promise(() => {}));
    render(<Harness />);
    await act(async () => switchTo("tui", { prefill: "/status" }));
    useTermStore.setState({ runtimes: { ...useTermStore.getState().runtimes, s: { ...useTermStore.getState().runtimes.s, status: "running", agent: "claude", agentState: "working" } as never } });
    await act(async () => switchTo("tui", { prefill: "/status" }));
    expect(setMode).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Switch" })).toBeNull();
  });

  it("reports nothing as done while the server has not answered", async () => {
    setMode.mockImplementation(() => new Promise(() => {}));
    const onDone = vi.fn();
    const onFail = vi.fn();
    render(<Harness />);
    await act(async () => switchTo("tui", { prefill: "/status", onDone, onFail }));
    expect(onDone).not.toHaveBeenCalled();
    expect(onFail).not.toHaveBeenCalled();
  });

  it("leaves nothing behind when the switch fails, and tells the caller", async () => {
    setMode.mockImplementation(() => Promise.reject(new Error("no")));
    const onStart = vi.fn();
    const onDone = vi.fn();
    const onFail = vi.fn();
    render(<Harness />);
    await act(async () => switchTo("tui", { prefill: "/status", onStart, onDone, onFail }));
    expect(useTermStore.getState().runtimes.s?.agentPrefill).toBeUndefined();
    expect(onStart).toHaveBeenCalledTimes(1);
    expect(onFail).toHaveBeenCalledTimes(1);
    expect(onDone).not.toHaveBeenCalled();
  });

  it("keeps the command for the terminal when the answer was lost but the server switched", async () => {
    setMode.mockImplementation(() => Promise.reject(new Error("connection closed")));
    serverEngine = "tui";
    const onDone = vi.fn();
    const onFail = vi.fn();
    render(<Harness />);
    await act(async () => switchTo("tui", { prefill: "/add-dir ../lib", onDone, onFail }));
    expect(loadTree).toHaveBeenCalled();
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(useTermStore.getState().runtimes.s?.agentPrefill).toBe("/add-dir ../lib");
    // Nothing failed from the user's point of view: no draft comes back and no error shows.
    expect(onFail).not.toHaveBeenCalled();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps both the command and the draft while the outcome cannot be read, then settles", async () => {
    setMode.mockImplementation(() => Promise.reject(new Error("connection closed")));
    serverEngine = null;
    const onDone = vi.fn();
    const onFail = vi.fn();
    render(<Harness />);
    await act(async () => switchTo("tui", { prefill: "/add-dir ../lib", onDone, onFail }));
    expect(onFail).toHaveBeenCalledTimes(1);
    expect(onDone).not.toHaveBeenCalled();
    expect(useTermStore.getState().runtimes.s?.agentPrefill).toBe("/add-dir ../lib");
    // The next tree (after a reconnect) still shows the conversation view: the command is dropped.
    act(() => useTermStore.setState({ sessions: [{ ...session, engine: "chat" }] }));
    expect(useTermStore.getState().runtimes.s?.agentPrefill).toBeUndefined();
  });

  it("leaves an unread outcome to the terminal when the next tree shows it switched", async () => {
    setMode.mockImplementation(() => Promise.reject(new Error("connection closed")));
    serverEngine = null;
    render(<Harness />);
    await act(async () => switchTo("tui", { prefill: "/add-dir ../lib" }));
    act(() => useTermStore.setState({ sessions: [{ ...session, engine: "tui" }] }));
    expect(useTermStore.getState().runtimes.s?.agentPrefill).toBe("/add-dir ../lib");
  });

  it("starts only after a working agent's question is answered with yes", async () => {
    useTermStore.setState({ runtimes: { s: { status: "running", agent: "claude", agentState: "working" } as never } });
    const onStart = vi.fn();
    const onFail = vi.fn();
    render(<Harness />);
    act(() => switchTo("tui", { prefill: "/status", onStart, onFail }));
    expect(onStart).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Switch" }));
    await act(async () => {});
    expect(onStart).toHaveBeenCalledTimes(1);
    expect(onFail).not.toHaveBeenCalled();
    expect(setMode).toHaveBeenCalledWith("s", "tui");
  });

  it("does not let a held Enter answer the question it opened", async () => {
    useTermStore.setState({ runtimes: { s: { status: "running", agent: "claude", agentState: "working" } as never } });
    render(<Harness />);
    act(() => switchTo("tui", { prefill: "/status" }));
    const confirm = await screen.findByRole("button", { name: "Switch" });
    fireEvent.keyDown(confirm, { key: "Enter", repeat: true });
    expect(setMode).not.toHaveBeenCalled();
    fireEvent.keyDown(confirm, { key: "Enter" });
    await act(async () => {});
    expect(setMode).toHaveBeenCalledWith("s", "tui");
  });

  it("leaves nothing behind when a working agent's question is cancelled", async () => {
    useTermStore.setState({ runtimes: { s: { status: "running", agent: "claude", agentState: "working" } as never } });
    render(<Harness />);
    const onStart = vi.fn();
    const onFail = vi.fn();
    act(() => switchTo("tui", { prefill: "/status", onStart, onFail }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(setMode).not.toHaveBeenCalled();
    // The caller took nothing, so there is nothing to give back.
    expect(onStart).not.toHaveBeenCalled();
    expect(onFail).not.toHaveBeenCalled();
    expect(useTermStore.getState().runtimes.s?.agentPrefill).toBeUndefined();
    // A later plain switch types nothing either.
    await act(async () => switchTo("tui"));
    expect(useTermStore.getState().runtimes.s?.agentPrefill).toBeUndefined();
  });
});
