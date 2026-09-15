import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatEvent, ChatSnapshot } from "../../../ipc/chat";

vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(), onTransportReconnect: vi.fn(),
}));
vi.mock("./engineSwitch", () => ({ useEngineSwitch: () => ({ switchTo: vi.fn(), confirm: null }) }));
vi.mock("./controls", () => ({ LevelBar: () => null, ControlChip: () => null }));

import { invoke, listen, onTransportReconnect } from "../../../ipc/transport";
import { useTermStore } from "../../../store/termStore";
import { ChatPane } from "./ChatPane";
import { useOutbox } from "./outbox";
import { clearChatCache } from "./chatCache";

let eventCallback: (event: ChatEvent) => void;
let snapshotOverrides: Partial<ChatSnapshot>;
let runShell: (args: unknown) => Promise<unknown>;

beforeEach(() => {
  clearChatCache();
  useOutbox.setState({ sessions: {} });
  snapshotOverrides = {};
  runShell = () => Promise.resolve(undefined);
  vi.mocked(invoke).mockReset();
  vi.mocked(onTransportReconnect).mockImplementation(() => () => {});
  const chatListeners = new Set<(event: ChatEvent) => void>();
  eventCallback = event => chatListeners.forEach(callback => callback(event));
  vi.mocked(listen).mockImplementation((name, callback) => {
    const listener = callback as (event: ChatEvent) => void;
    if (name === "chat://event/s") chatListeners.add(listener);
    return Promise.resolve(() => { chatListeners.delete(listener); });
  });
  useTermStore.setState({ chatModel: "m", chatModelByKind: { claude: "m" }, chatEffortByModel: {}, runtimes: {}, agentDefaults: {} });
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === "agent_permission_catalog") {
      return Promise.resolve({ modes: ["plan", "default", "acceptEdits", "auto", "bypassPermissions"], selected: "default" }) as Promise<never>;
    }
    if (command === "chat_snapshot") return Promise.resolve({
      submissionReceipts: true, running: true, model: "m", mode: "default", rows: [], queue: [], permissions: [], commands: [], configKeys: [], ...snapshotOverrides,
    }) as Promise<never>;
    if (command === "chat_models") return Promise.resolve([{ id: "m", label: "M", description: "", effortLevels: ["high"] }]) as Promise<never>;
    if (command === "chat_run_shell") return runShell(args) as Promise<never>;
    return Promise.resolve(command === "chat_send" ? "sent" : undefined) as Promise<never>;
  });
});
afterEach(cleanup);

async function mountPane() {
  render(<ChatPane session={{ id: "s", projectId: "p", name: "Claude", kind: "claude", engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
    area={{}} hidden={false} focused multi={false} onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />);
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_snapshot")).toBe(true));
  await act(async () => {});
  return screen.getByRole("textbox") as HTMLTextAreaElement;
}

function submit(input: HTMLTextAreaElement, text: string) {
  fireEvent.change(input, { target: { value: text } });
  fireEvent.keyDown(input, { key: "Enter" });
}

const commands = () => vi.mocked(invoke).mock.calls.map(([command]) => command);

describe("shell mode in the composer", () => {
  it("runs a ! command through chat_run_shell and never through chat_send", async () => {
    const input = await mountPane();
    submit(input, "!echo hi");
    await waitFor(() => expect(commands()).toContain("chat_run_shell"));
    const [, args] = vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_run_shell")!;
    expect(args).toMatchObject({ sessionId: "s", command: "echo hi" });
    expect((args as { messageId: string }).messageId).toMatch(/^sh-/);
    expect(commands()).not.toContain("chat_send");
    expect(commands()).not.toContain("chat_start");
    expect(input.value).toBe("");
  });

  it("starts the agent first when none is running, so the result has someone to react to it", async () => {
    snapshotOverrides = { running: false };
    const input = await mountPane();
    submit(input, "! ls");
    await waitFor(() => expect(commands()).toContain("chat_run_shell"));
    const order = commands().filter(command => command === "chat_start" || command === "chat_run_shell");
    expect(order).toEqual(["chat_start", "chat_run_shell"]);
    expect(commands()).not.toContain("chat_send");
  });

  it("shows a hint for a bare ! and sends nothing", async () => {
    const input = await mountPane();
    submit(input, "!");
    expect(await screen.findByText("Type a command after ! to run it in the shell")).toBeTruthy();
    expect(commands()).not.toContain("chat_run_shell");
    expect(commands()).not.toContain("chat_send");
  });

  it("refuses images in shell mode and keeps the draft", async () => {
    const input = await mountPane();
    const file = new File([new Uint8Array([137, 80, 78, 71])], "shot.png", { type: "image/png" });
    await act(async () => {
      fireEvent.paste(input, { clipboardData: { items: [{ kind: "file", type: "image/png", getAsFile: () => file }], files: [file], getData: () => "" } });
    });
    await waitFor(() => expect(document.querySelector(".sv-attach-item")).toBeTruthy());
    submit(input, "!ls");
    expect(await screen.findByText("Shell commands cannot carry images. Remove the attachment or send it as a message.")).toBeTruthy();
    expect(commands()).not.toContain("chat_run_shell");
    expect(commands()).not.toContain("chat_send");
    expect(input.value).toBe("!ls");
  });

  it("tells the user when a command is already running", async () => {
    runShell = () => Promise.reject(new Error("chat_shell_running"));
    const input = await mountPane();
    submit(input, "!sleep 5");
    expect(await screen.findByText("A shell command is still running in this conversation. Cancel it or wait for it to finish.")).toBeTruthy();
    // A refused command stays in the composer for a retry instead of being thrown away.
    expect(input.value).toBe("!sleep 5");
  });

  it("renders a shell row from the event channel with its Cancel wired to chat_cancel_shell", async () => {
    await mountPane();
    act(() => {
      eventCallback({ type: "rows", revision: 1, epoch: 1, rows: [{
        kind: "shell", id: "sh-9", command: "az login", stdout: "Opening…", stderr: "",
        stdoutTruncated: false, stderrTruncated: false, status: "running",
      }] });
    });
    expect(screen.getByText("az login")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(commands()).toContain("chat_cancel_shell"));
    const [, args] = vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_cancel_shell")!;
    expect(args).toEqual({ sessionId: "s", messageId: "sh-9" });
  });
});
