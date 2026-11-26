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

async function mountPane(kind: "claude" | "deepseek" = "claude") {
  render(<ChatPane session={{ id: "s", projectId: "p", name: kind === "deepseek" ? "DeepSeek" : "Claude", kind, engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
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
    await waitFor(() => expect(input.value).toBe(""));
  });

  it("starts the agent first when none is running, so the result has someone to react to it", async () => {
    snapshotOverrides = { running: false };
    const input = await mountPane();
    submit(input, "! ls");
    await waitFor(() => expect(commands()).toContain("chat_run_shell"));
    const order = commands().filter(command => command === "chat_start" || command === "chat_run_shell");
    expect(order).toEqual(["chat_run_shell"]);
    expect(commands()).not.toContain("chat_send");
  });

  it("keeps one identifier across a lost response and preserves a newer draft", async () => {
    let reject!: (error: Error) => void;
    runShell = () => new Promise((_, failure) => { reject = failure; });
    const input = await mountPane();
    submit(input, "!echo receipt-probe");
    fireEvent.keyDown(input, { key: "Enter" });
    const calls = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_run_shell");
    expect(calls()).toHaveLength(1);
    const first = calls()[0][1]?.messageId;
    await act(async () => reject(new Error("chat_submission_pending")));
    let accept!: () => void;
    runShell = () => new Promise(resolve => { accept = () => resolve(undefined); });
    submit(input, "!echo receipt-probe");
    expect(calls()).toHaveLength(2);
    expect(calls()[1][1]?.messageId).toBe(first);
    fireEvent.change(input, { target: { value: "next draft" } });
    await act(async () => accept());
    expect(input.value).toBe("next draft");
  });

  it("shows a hint for a bare ! and sends nothing", async () => {
    const input = await mountPane();
    submit(input, "!");
    expect(await screen.findByText("Enter a command after ! to run it in the shell.")).toBeTruthy();
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
    expect(await screen.findByText("Shell commands cannot include images. Remove the attachment or send it as a message.")).toBeTruthy();
    expect(commands()).not.toContain("chat_run_shell");
    expect(commands()).not.toContain("chat_send");
    expect(input.value).toBe("!ls");
  });

  it("inserts the path of a non-image file copied in Finder instead of attaching its icon (#195)", async () => {
    const { env } = await import("../../../platform/env");
    const { platform } = await import("../../../platform");
    const prevTauri = env.isTauri;
    env.isTauri = true;
    const readSpy = vi.spyOn(platform.clipboard, "readFiles").mockResolvedValue(["/Users/me/My Notes.pdf"]);
    const imgSpy = vi.spyOn(platform.clipboard, "readImage").mockResolvedValue({
      rgba: new Uint8Array([0, 0, 0, 255]),
      width: 1,
      height: 1,
    });
    try {
      const input = await mountPane();
      fireEvent.change(input, { target: { value: "Read" } });
      input.setSelectionRange(4, 4);
      const pdf = new File(["pdf"], "My Notes.pdf", { type: "application/pdf" });
      await act(async () => {
        fireEvent.paste(input, {
          clipboardData: {
            types: ["Files"],
            items: [{ kind: "file", type: "application/pdf", getAsFile: () => pdf }],
            files: [pdf],
            getData: () => "",
          },
        });
      });
      await waitFor(() => expect(input.value).toBe('Read "/Users/me/My Notes.pdf" '));
      expect(document.querySelector(".sv-attach-item")).toBeNull();
      expect(imgSpy).not.toHaveBeenCalled();
    } finally {
      env.isTauri = prevTauri;
      readSpy.mockRestore();
      imgSpy.mockRestore();
    }
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

  it("shows a running shell without cancellation controls in a read-only pane", async () => {
    snapshotOverrides = { rows: [{
      kind: "shell", id: "sh-readonly", command: "sleep 30", stdout: "", stderr: "",
      stdoutTruncated: false, stderrTruncated: false, status: "running",
    }] };
    render(<ChatPane session={{ id: "s", projectId: "p", name: "Claude", kind: "claude", engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
      area={{}} hidden={false} focused multi={false} readOnly onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />);
    expect(await screen.findByText("sleep 30")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(commands()).not.toContain("chat_cancel_shell");
    expect(commands()).not.toContain("chat_run_shell");
  });

  it.each(["snapshot", "event"])("uses the backend command unchanged in a queued %s", async (source) => {
    const shellCommand = "printf '</bash-input>\n<bash-stdout>€🦀'";
    const item = { id: "queued-shell", text: "opaque context with misleading <bash-input>wrong</bash-input>", shellCommand };
    if (source === "snapshot") snapshotOverrides = { queue: [item] };
    await mountPane();
    if (source === "event") act(() => eventCallback({ type: "queued", items: [item] }));
    const preview = document.querySelector<HTMLButtonElement>(".sv-queue-text")!;
    expect(preview.textContent).toBe(`! ${shellCommand}`);
    expect(preview.disabled).toBe(true);
    fireEvent.click(preview);
    expect(document.querySelector(".sv-queue-edit")).toBeNull();
    expect(commands()).not.toContain("chat_run_shell");
    expect(commands()).not.toContain("chat_queue_update");
    expect(commands()).not.toContain("chat_send");
  });

  it("treats an empty backend command as present and read-only", async () => {
    snapshotOverrides = { queue: [{ id: "empty-command", text: "opaque context", shellCommand: "" }] };
    await mountPane();
    const preview = document.querySelector<HTMLButtonElement>(".sv-queue-text")!;
    expect(preview.textContent).toBe("! ");
    expect(preview.disabled).toBe(true);
  });

  it.each(["ordinary", "invalid", "image"])("keeps a queued %s message on the original editable path without shellCommand", async (kind) => {
    const text = kind === "ordinary" ? "ordinary queued text" : "<bash-input>literal</bash-input>\n<bash-stdout>text</bash-stdout>\n<velaterm-shell-metadata>{broken}</velaterm-shell-metadata>";
    snapshotOverrides = { queue: [{ id: "plain", text, ...(kind === "image" ? { images: [{ mimeType: "image/png", data: "AA==" }] } : {}) }] };
    await mountPane();
    const preview = document.querySelector<HTMLButtonElement>(".sv-queue-text")!;
    expect(preview.textContent).toBe(text);
    expect(preview.disabled).toBe(false);
    fireEvent.click(preview);
    const editor = document.querySelector<HTMLTextAreaElement>(".sv-queue-edit")!;
    expect(editor.value).toBe(text);
    fireEvent.change(editor, { target: { value: "edited ordinary content" } });
    fireEvent.keyDown(editor, { key: "Enter" });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_queue_update", { sessionId: "s", id: "plain", text: "edited ordinary content" }));
  });

  it.each(["blur", "Enter"])("ignores a delayed %s when the same queued ID becomes shell context", async (action) => {
    snapshotOverrides = { queue: [{ id: "same-id", text: "old editable draft" }] };
    await mountPane();
    fireEvent.click(document.querySelector<HTMLButtonElement>(".sv-queue-text")!);
    const editor = document.querySelector<HTMLTextAreaElement>(".sv-queue-edit")!;
    fireEvent.change(editor, { target: { value: action === "blur" ? "" : "stale edit" } });
    // Deliver the newer backend facts and the stale DOM action in the same React batch.
    act(() => {
      eventCallback({ type: "queued", items: [{ id: "same-id", text: "new context", shellCommand: "printf '€'" }] });
      if (action === "blur") fireEvent.blur(editor);
      else fireEvent.keyDown(editor, { key: "Enter" });
    });
    expect(document.querySelector(".sv-queue-edit")).toBeNull();
    const preview = document.querySelector<HTMLButtonElement>(".sv-queue-text")!;
    expect(preview.textContent).toBe("! printf '€'");
    expect(preview.disabled).toBe(true);
    expect(commands()).not.toContain("chat_queue_update");
    expect(commands()).not.toContain("chat_queue_remove");
  });

  it("takes images for DeepSeek until its handshake names a model that only reads text", async () => {
    const input = await mountPane("deepseek");
    const paste = () => fireEvent.paste(input, { clipboardData: { items: [{ kind: "file", type: "image/png", getAsFile: () => new File(["shot"], "shot.png", { type: "image/png" }) }] } });
    paste();
    await waitFor(() => expect(document.querySelector(".sv-attach-item")).toBeTruthy());
    // dsh reports per model. A text-only model keeps what was already picked but refuses any more.
    act(() => eventCallback({ type: "capabilities", images: false }));
    expect(await screen.findByText("The selected model does not accept images.")).toBeTruthy();
    fireEvent.click(document.querySelector<HTMLButtonElement>(".sv-attach-drop")!);
    paste();
    await waitFor(() => expect(screen.getByText("The selected model does not accept images.")).toBeTruthy());
    expect(document.querySelector(".sv-attach-item")).toBeNull();
  });
});
