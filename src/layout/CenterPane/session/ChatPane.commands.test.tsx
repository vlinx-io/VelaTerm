import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatCommand, ChatEvent, ChatSnapshot } from "../../../ipc/chat";
import type { Session } from "../../../types";

const engine = vi.hoisted(() => ({ switchTo: vi.fn() }));
vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(), onTransportReconnect: vi.fn(),
}));
vi.mock("./engineSwitch", () => ({ useEngineSwitch: () => ({ switchTo: engine.switchTo, confirm: null }) }));
vi.mock("./controls", () => ({ LevelBar: () => null, ControlChip: () => null }));

import { invoke, listen, onTransportReconnect } from "../../../ipc/transport";
import { useTermStore } from "../../../store/termStore";
import { ChatPane } from "./ChatPane";
import { useOutbox } from "./outbox";
import { clearChatCache } from "./chatCache";

let snapshotOverrides: Partial<ChatSnapshot>;
let chatListeners: Set<(event: ChatEvent) => void>;
let liveCommands: ChatCommand[];
const clearChatSession = vi.fn(() => Promise.resolve({} as Session));
const forkSession = vi.fn(() => Promise.resolve());

beforeEach(() => {
  clearChatCache();
  useOutbox.setState({ sessions: {} });
  snapshotOverrides = {};
  liveCommands = [];
  engine.switchTo.mockReset();
  // By default the switch begins at once and succeeds, as for an idle agent.
  engine.switchTo.mockImplementation((_engine: string, opts?: { onStart?: () => void; onDone?: () => void }) => {
    opts?.onStart?.();
    opts?.onDone?.();
  });
  clearChatSession.mockClear();
  forkSession.mockClear();
  vi.mocked(invoke).mockReset();
  vi.mocked(onTransportReconnect).mockImplementation(() => () => {});
  chatListeners = new Set();
  vi.mocked(listen).mockImplementation((name, callback) => {
    const listener = callback as (event: ChatEvent) => void;
    if (name === "chat://event/s") chatListeners.add(listener);
    return Promise.resolve(() => { chatListeners.delete(listener); });
  });
  useTermStore.setState({
    chatModel: "m", chatModelByKind: { claude: "m", codex: "m" }, chatEffortByModel: {}, runtimes: {}, agentDefaults: {},
    clearChatSession, forkSession,
  });
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "agent_permission_catalog") {
      return Promise.resolve({ modes: ["plan", "default", "acceptEdits", "auto", "bypassPermissions"], selected: "default" }) as Promise<never>;
    }
    if (command === "chat_snapshot") return Promise.resolve({
      submissionReceipts: true, running: false, model: "m", mode: "default", rows: [], queue: [], permissions: [], commands: [], configKeys: [], ...snapshotOverrides,
    }) as Promise<never>;
    if (command === "chat_models") return Promise.resolve([{ id: "m", label: "M", description: "", effortLevels: ["high"] }]) as Promise<never>;
    if (command === "chat_commands") return Promise.resolve(liveCommands) as Promise<never>;
    return Promise.resolve(command === "chat_send" ? "sent" : undefined) as Promise<never>;
  });
});
afterEach(cleanup);

async function mountPane(kind: Session["kind"] = "claude", extra: Partial<Session> = {}) {
  render(<ChatPane session={{ id: "s", projectId: "p", name: "Agent", kind, engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0, ...extra }}
    area={{}} hidden={false} focused multi={false} onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />);
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_snapshot")).toBe(true));
  await act(async () => {});
  return screen.getByRole("textbox") as HTMLTextAreaElement;
}

async function submit(input: HTMLTextAreaElement, text: string) {
  fireEvent.change(input, { target: { value: text } });
  // Let the catalogue read finish; Enter waits for it otherwise.
  await act(async () => {});
  fireEvent.keyDown(input, { key: "Enter" });
  await act(async () => {});
}

const commands = () => vi.mocked(invoke).mock.calls.map(([command]) => command);
const sentText = () => (vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")?.[1] as { text?: string } | undefined)?.text;

describe("Claude commands in the conversation view", () => {
  it("hands a terminal-only command to the terminal view, typed and not sent", async () => {
    const input = await mountPane();
    await submit(input, "/status");
    expect(engine.switchTo).toHaveBeenCalledWith("tui", expect.objectContaining({ prefill: "/status" }));
    expect(commands()).not.toContain("chat_send");
    expect(commands()).not.toContain("chat_start");
    expect(input.value).toBe("");
  });

  it("hands an alias over as typed, arguments included", async () => {
    const input = await mountPane();
    await submit(input, "/allowed-tools add Bash");
    expect(engine.switchTo).toHaveBeenCalledWith("tui", expect.objectContaining({ prefill: "/allowed-tools add Bash" }));
    expect(commands()).not.toContain("chat_send");
  });

  it("keeps the draft when the switch fails at once", async () => {
    engine.switchTo.mockImplementation((_engine: string, opts?: { onStart?: () => void; onFail?: () => void }) => {
      opts?.onStart?.();
      opts?.onFail?.();
    });
    const input = await mountPane();
    await submit(input, "/add-dir ../lib");
    expect(input.value).toBe("/add-dir ../lib");
    expect(screen.queryByText("Opening /add-dir in the terminal view…")).toBeNull();
  });

  it("keeps the draft when the switch is cancelled before it begins", async () => {
    // A working agent is asked first; cancelling calls neither callback.
    engine.switchTo.mockImplementation(() => {});
    const input = await mountPane();
    await submit(input, "/btw what is left to do?");
    expect(engine.switchTo).toHaveBeenCalledTimes(1);
    expect(input.value).toBe("/btw what is left to do?");
    expect(commands()).not.toContain("chat_send");
  });

  it("keeps the draft when the switch fails later", async () => {
    engine.switchTo.mockImplementation((_engine: string, opts?: { onStart?: () => void; onFail?: () => void }) => {
      opts?.onStart?.();
      // The failure arrives after the emptied composer has rendered.
      setTimeout(() => opts?.onFail?.(), 0);
    });
    const input = await mountPane();
    await submit(input, "/add-dir ../lib");
    await waitFor(() => expect(input.value).toBe("/add-dir ../lib"));
    await waitFor(() => expect(screen.queryByText("Opening /add-dir in the terminal view…")).toBeNull());
    expect(commands()).not.toContain("chat_send");
  });

  it("keeps the draft and says a switch is running until the server has switched", async () => {
    // The answer never comes, as over a half-open connection.
    engine.switchTo.mockImplementation((_engine: string, opts?: { onStart?: () => void }) => opts?.onStart?.());
    const input = await mountPane();
    await submit(input, "/status");
    expect(await screen.findByText("Opening /status in the terminal view…")).toBeTruthy();
    expect(input.value).toBe("/status");
    expect(commands()).not.toContain("chat_send");
  });

  it("keeps what was typed while a slow switch was running", async () => {
    let done: (() => void) | undefined;
    engine.switchTo.mockImplementation((_engine: string, opts?: { onStart?: () => void; onDone?: () => void }) => {
      opts?.onStart?.();
      done = opts?.onDone;
    });
    const input = await mountPane();
    await submit(input, "/status");
    // The command has not left the composer while the switch runs.
    expect(input.value).toBe("/status");
    fireEvent.change(input, { target: { value: "something new" } });
    act(() => done?.());
    expect(input.value).toBe("something new");
  });

  it("keeps what was typed when a slow switch then fails", async () => {
    let fail: (() => void) | undefined;
    engine.switchTo.mockImplementation((_engine: string, opts?: { onStart?: () => void; onFail?: () => void }) => {
      opts?.onStart?.();
      fail = opts?.onFail;
    });
    const input = await mountPane();
    await submit(input, "/status");
    expect(input.value).toBe("/status");
    fireEvent.change(input, { target: { value: "/status and more" } });
    act(() => fail?.());
    expect(input.value).toBe("/status and more");
    expect(screen.queryByText("Opening /status in the terminal view…")).toBeNull();
  });

  it("explains a command that means nothing here and keeps the draft", async () => {
    const input = await mountPane();
    await submit(input, "/theme");
    expect(await screen.findByText("/theme only changes the agent's terminal interface and has no effect in the conversation view.")).toBeTruthy();
    expect(commands()).not.toContain("chat_send");
    expect(engine.switchTo).not.toHaveBeenCalled();
    expect(input.value).toBe("/theme");
  });

  it("clears the conversation for Claude's /reset alias", async () => {
    const input = await mountPane();
    await submit(input, "/reset");
    expect(clearChatSession).toHaveBeenCalledWith("s", "m", undefined);
    expect(commands()).not.toContain("chat_send");
  });

  it("refuses arguments to a VelaTerm action instead of sending them", async () => {
    const input = await mountPane("claude", { agentSessionId: "abc" });
    await submit(input, "/fork do X");
    expect(await screen.findByText("/fork takes no arguments in the conversation view.")).toBeTruthy();
    expect(forkSession).not.toHaveBeenCalled();
    expect(commands()).not.toContain("chat_send");
  });

  it.each(["/clear old work", "/rewind x", "/new foo"])("sends Claude's %s as a message, as before", async (text) => {
    const input = await mountPane();
    await submit(input, text);
    await waitFor(() => expect(sentText()).toBe(text));
    expect(clearChatSession).not.toHaveBeenCalled();
  });

  it("sends /resume as a message while steering, as before", async () => {
    const input = await mountPane();
    act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: "claude", agentState: "working" } as never } }));
    fireEvent.change(input, { target: { value: "/resume" } });
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Steer" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({ behavior: "steer", text: "/resume" })));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("sends /resume with an image as a message, as before", async () => {
    const input = await mountPane();
    fireEvent.drop(input.closest(".sv-composer")!, { dataTransfer: { files: [new File(["png"], "example.png", { type: "image/png" })] } });
    await screen.findByAltText("example.png");
    fireEvent.change(input, { target: { value: "/resume" } });
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({
      text: "/resume", images: [{ mimeType: "image/png", data: "cG5n" }],
    })));
  });

  it("forks through VelaTerm's session action", async () => {
    const input = await mountPane("claude", { agentSessionId: "abc" });
    await submit(input, "/fork");
    expect(forkSession).toHaveBeenCalledWith("s");
    expect(commands()).not.toContain("chat_send");
  });

  it("puts /fork back when the fork fails", async () => {
    forkSession.mockImplementationOnce(() => Promise.reject(new Error("no")));
    const input = await mountPane("claude", { agentSessionId: "abc" });
    await submit(input, "/fork");
    await waitFor(() => expect(input.value).toBe("/fork"));
  });

  it("says so when there is no conversation to fork yet", async () => {
    const input = await mountPane();
    await submit(input, "/fork");
    expect(await screen.findByText("/fork is not available for this session here.")).toBeTruthy();
    expect(forkSession).not.toHaveBeenCalled();
  });

  it("sends a command Claude answers itself as typed", async () => {
    const input = await mountPane();
    await submit(input, "/model x");
    await waitFor(() => expect(sentText()).toBe("/model x"));
    expect(engine.switchTo).not.toHaveBeenCalled();
  });

  it("tags each row of the slash menu with what sending it does", async () => {
    const input = await mountPane();
    fireEvent.change(input, { target: { value: "/stat" } });
    await act(async () => {});
    const row = (await screen.findByText("/status")).closest("button")!;
    expect(row.querySelector(".sv-complete-tag")?.textContent).toBe("Terminal");
    fireEvent.change(input, { target: { value: "/cle" } });
    await act(async () => {});
    expect((await screen.findByText("/clear")).closest("button")!.querySelector(".sv-complete-tag")?.textContent).toBe("VelaTerm");
  });
  it("follows a command list the running agent replaces mid-session", async () => {
    snapshotOverrides = { running: true };
    liveCommands = [{ name: "old-skill", description: "Old", invocation: "/" }];
    const input = await mountPane();
    fireEvent.change(input, { target: { value: "/old-" } });
    await act(async () => {});
    expect(await screen.findByText("/old-skill")).toBeTruthy();
    expect(chatListeners.size).toBeGreaterThan(0);
    // The menu stays open, so nothing reads the catalogue again: only the pushed event can change it.
    act(() => chatListeners.forEach((listener) => listener({ type: "commands", commands: [{ name: "new-skill", description: "New", invocation: "/" }] })));
    fireEvent.change(input, { target: { value: "/new-" } });
    await act(async () => {});
    const row = (await screen.findByText("/new-skill")).closest("button")!;
    expect(row.querySelector(".sv-complete-tag")?.textContent).toBe("Claude Code");
    fireEvent.change(input, { target: { value: "/old-" } });
    await act(async () => {});
    expect(screen.queryByText("/old-skill")).toBeNull();
    // The manifest layer stays on top of the replaced live list.
    fireEvent.change(input, { target: { value: "/stat" } });
    await act(async () => {});
    expect(await screen.findByText("/status")).toBeTruthy();
  });
});

describe("other agents keep their commands", () => {
  it("compacts Codex through its own request", async () => {
    snapshotOverrides = { running: true };
    const input = await mountPane("codex");
    await submit(input, "/compact");
    await waitFor(() => expect(commands()).toContain("chat_compact"));
    expect(commands()).not.toContain("chat_send");
  });

  it("sends a Claude-only name as a message in Codex", async () => {
    const input = await mountPane("codex");
    await submit(input, "/status");
    await waitFor(() => expect(sentText()).toBe("/status"));
    expect(engine.switchTo).not.toHaveBeenCalled();
  });

  it("sends Codex /clear with arguments as a message, as before", async () => {
    const input = await mountPane("codex");
    await submit(input, "/clear foo");
    await waitFor(() => expect(sentText()).toBe("/clear foo"));
    expect(clearChatSession).not.toHaveBeenCalled();
  });

  it("offers no Claude command in a Codex menu", async () => {
    const input = await mountPane("codex");
    fireEvent.change(input, { target: { value: "/stat" } });
    await act(async () => {});
    expect(screen.queryByText("/status")).toBeNull();
  });
});
