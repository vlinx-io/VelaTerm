import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ChatEvent, ChatPermission, ChatSnapshot } from "../../../ipc/chat";
import type { PermissionAnswer } from "./permissionCards";

vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(), onTransportReconnect: vi.fn(),
}));
vi.mock("./engineSwitch", () => ({ useEngineSwitch: () => ({ switchTo: vi.fn(), confirm: null }) }));
// Exercise the pane's orchestration; menu layout is covered separately by the controls themselves.
vi.mock("./controls", () => ({
  LevelBar: () => null,
  ControlChip: ({ title, value, options, onPick }: {
    title: string; value: string; options: { value: string; label: string }[];
    onPick: (value: string, keep: boolean) => void;
  }) => <select aria-label={title} value={value} onChange={(event) => onPick(event.target.value, true)}>
    {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
  </select>,
}));
vi.mock("./permissionCards", () => ({
  PermissionCard: ({ request, onAnswer }: {
    request: ChatPermission; onAnswer: (request: ChatPermission, reply: PermissionAnswer) => void;
  }) => <button onClick={() => onAnswer(request, { allow: true, mode: "acceptEdits" })}>Approve plan</button>,
}));

import { invoke, listen, onTransportReconnect } from "../../../ipc/transport";
import { useTermStore } from "../../../store/termStore";
import { ChatPane } from "./ChatPane";
import { useOutbox } from "./outbox";
import { clearChatCache } from "./chatCache";

let eventCallback: (event: ChatEvent) => void;
let permissions: ChatPermission[];
let complete: (value?: unknown) => void;
let reject: (reason: Error) => void;
let snapshotOverrides: Partial<ChatSnapshot>;
let reconnectCallback: () => void;

beforeEach(() => {
  clearChatCache();
  useOutbox.setState({ sessions: {} });
  snapshotOverrides = {};
  permissions = [];
  vi.mocked(invoke).mockReset();
  vi.mocked(onTransportReconnect).mockImplementation(callback => { reconnectCallback = callback; return () => {}; });
  vi.mocked(listen).mockImplementation((_name, callback) => {
    eventCallback = callback as (event: ChatEvent) => void;
    return Promise.resolve(() => {});
  });
  useTermStore.setState({ chatModel: "old-model", chatModelByKind: { claude: "old-model" }, chatEffortByModel: {}, runtimes: {} });
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "chat_snapshot") return Promise.resolve({
      submissionReceipts: true, running: true, model: "old-model", mode: "default", rows: [], queue: [], permissions, commands: [], configKeys: [], ...snapshotOverrides,
    }) as Promise<never>;
    if (command === "chat_models") return Promise.resolve([
      { id: "old-model", label: "Old", description: "", effortLevels: ["high"] },
      { id: "new-model", label: "New", description: "", effortLevels: ["high"] },
    ]) as Promise<never>;
    if (command === "chat_set_model" || command === "chat_set_mode") {
      return new Promise<unknown>((resolve, fail) => { complete = resolve; reject = fail; }) as Promise<never>;
    }
    return Promise.resolve(command === "chat_send" ? "sent" : undefined) as Promise<never>;
  });
});
afterEach(cleanup);

it("replays newer events over a late snapshot without losing history or restoring rewound rows", async () => {
  let finishSnapshot!: (value: unknown) => void;
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_snapshot"
    ? new Promise<unknown>(resolve => { finishSnapshot = resolve; }) as Promise<never> : previous(command, args));
  const mounting = mountPane();
  await waitFor(() => expect(finishSnapshot).toBeTypeOf("function"));
  act(() => {
    eventCallback({ type: "rows", revision: 11, epoch: 100, rows: [{ kind: "user", id: "new", text: "New message" }] });
    eventCallback({ type: "queued", revision: 11, epoch: 100, items: [{ id: "q", text: "New queue" }] });
  });
  expect(screen.getByText("New message")).toBeTruthy();
  await act(async () => finishSnapshot({ running: true, startedAt: 100, model: "old-model", rowsRevision: 10, queueRevision: 10,
    rows: [{ kind: "user", id: "old", text: "Old history" }], queue: [], permissions: [], commands: [], configKeys: [] }));
  await mounting;
  expect(screen.getByText("Old history")).toBeTruthy();
  expect(screen.getByText("New message")).toBeTruthy();
  expect(screen.getByText("New queue")).toBeTruthy();
  act(() => {
    eventCallback({ type: "replaceRows", revision: 12, epoch: 100, rows: [] });
    eventCallback({ type: "rows", revision: 11, epoch: 100, rows: [{ kind: "user", id: "new", text: "New message" }] });
    eventCallback({ type: "queued", revision: 10, epoch: 100, items: [] });
  });
  expect(screen.queryByText("New message")).toBeNull();
  expect(screen.getByText("New queue")).toBeTruthy();
});

async function mountPane(kind: "claude" | "codex" | "opencode" = "claude", expectedModel = "old-model") {
  const view = render(<ChatPane session={{ id: "s", projectId: "p", name: "Claude", kind, engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
    area={{}} hidden={false} focused multi={false} onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />);
  await waitFor(() => expect((screen.getByRole("combobox", { name: "Model" }) as HTMLSelectElement).value).toBe(expectedModel));
  return view;
}

it("resynchronizes on reconnection and reuses the uncertain submission identifier", async () => {
  const { container } = await mountPane();
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  const previous = vi.mocked(invoke).getMockImplementation()!;
  const disconnected = new Error("Disconnected");
  disconnected.name = "TransportError";
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_send"
    ? Promise.reject(disconnected) : previous(command, args));
  fireEvent.change(input, { target: { value: "Keep this message" } });
  fireEvent.keyDown(input, { key: "Enter" });
  await screen.findByText("Delivery unconfirmed");
  const original = vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")![1]!;
  vi.mocked(invoke).mockImplementation(previous);
  act(() => reconnectCallback());
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_send")).toHaveLength(2));
  expect(invoke).toHaveBeenLastCalledWith("chat_send", original);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_snapshot")).toHaveLength(2);
  act(() => eventCallback({ type: "rows", rows: [{ kind: "user", id: original.messageId as string, text: "Keep this message" }] }));
  expect(screen.getAllByText("Keep this message")).toHaveLength(1);
});

it("preserves input when an older server cannot deduplicate submissions", async () => {
  snapshotOverrides = { submissionReceipts: undefined };
  const { container } = await mountPane();
  await act(async () => {});
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "Do not lose this" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(input.value).toBe("Do not lose this");
  expect(screen.getByText("Update the server before sending messages from this client.")).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send")).toBe(false);
});

it("blocks native arrow control insertion in the composer without changing text or selection", async () => {
  const { container } = await mountPane();
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "工工工式addd 中文" } });
  for (const position of [0, input.value.length]) {
    input.setSelectionRange(position, position);
    for (const data of ["\u001c", "\u001d", "\u001e", "\u001f", "\u001d\u001d"]) {
      const event = new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "insertText", data });
      expect(input.dispatchEvent(event)).toBe(false);
      expect(input.value).toBe("工工工式addd 中文");
      expect(input.selectionStart).toBe(position);
    }
  }
  for (const [inputType, data] of [
    ["insertText", "中文"], ["insertText", "addd"], ["insertText", "\t"],
    ["insertLineBreak", null], ["insertCompositionText", "zhong"],
    ["insertFromPaste", "中文\n\u001c"], ["deleteContentBackward", null],
  ]) {
    expect(input.dispatchEvent(new InputEvent("beforeinput", {
      bubbles: true, cancelable: true, inputType: inputType!, data,
      isComposing: inputType === "insertCompositionText",
    }))).toBe(true);
  }
  const arrow = new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true, cancelable: true });
  expect(input.dispatchEvent(arrow)).toBe(true);
});

it("guards queue editors mounted after the pane and removes the listener on unmount", async () => {
  const { container, unmount } = await mountPane();
  act(() => eventCallback({ type: "queued", items: [{ id: "guard-q", text: "Queued input" }] }));
  fireEvent.click(screen.getByText("Queued input"));
  const editor = container.querySelector<HTMLTextAreaElement>(".sv-queue-edit")!;
  const root = container.querySelector<HTMLDivElement>(".term-mount")!;
  const insert = () => editor.dispatchEvent(new InputEvent("beforeinput", {
    bubbles: true, cancelable: true, inputType: "insertText", data: "\u001c",
  }));
  expect(insert()).toBe(false);
  unmount();
  root.append(editor);
  expect(insert()).toBe(true);
});

it("labels OpenCode replies with their own models instead of the current selection", async () => {
  const { container } = await mountPane("opencode");
  act(() => eventCallback({ type: "replaceRows", rows: [
    { kind: "assistant", id: "a", text: "First answer", streaming: false, model: "deepseek-v4-pro" },
    { kind: "assistant", id: "b", text: "Second answer", streaming: true, model: "glm-5.3" },
    { kind: "assistant", id: "c", text: "Legacy answer", streaming: false },
  ] }));
  const authors = () => Array.from(container.querySelectorAll(".sv-msg-who"), (node) => node.textContent);
  await waitFor(() => expect(authors()).toEqual(["deepseek-v4-pro", "glm-5.3", "OpenCode"]));
  act(() => eventCallback({ type: "settingsChanged", model: "new-model" }));
  expect(authors()).toEqual(["deepseek-v4-pro", "glm-5.3", "OpenCode"]);
  act(() => eventCallback({ type: "rows", rows: [
    { kind: "assistant", id: "b", text: "Second answer completed", streaming: false, model: "glm-5.3" },
  ] }));
  expect(authors()).toEqual(["deepseek-v4-pro", "glm-5.3", "OpenCode"]);
});

it.each(["claude", "codex"] as const)("preserves %s reply labels without model metadata", async (kind) => {
  const { container } = await mountPane(kind);
  act(() => eventCallback({ type: "rows", rows: [
    { kind: "assistant", id: "a", text: "Answer", streaming: false },
  ] }));
  await waitFor(() => expect(container.querySelector(".sv-msg-who")?.textContent)
    .toBe(kind === "claude" ? "Claude" : "Codex"));
});

it("keeps the confirmed model and saved default while a change waits or is rejected", async () => {
  await mountPane();
  const select = screen.getByRole("combobox", { name: "Model" }) as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "new-model" } });
  expect(select.value).toBe("old-model");
  expect(useTermStore.getState().chatModelByKind.claude).toBe("old-model");
  await act(async () => reject(new Error("model unavailable")));
  expect(select.value).toBe("old-model");
  expect(screen.getByText("Error: model unavailable")).toBeTruthy();
  expect(useTermStore.getState().chatModelByKind.claude).toBe("old-model");
  fireEvent.change(select, { target: { value: "new-model" } });
  await act(async () => complete());
  expect(select.value).toBe("new-model");
  expect(useTermStore.getState().chatModelByKind.claude).toBe("new-model");
});

it("retains the approval card and does not approve the tool when its required mode change fails", async () => {
  permissions = [{ id: "r", subtype: "can_use_tool", tool_name: "ExitPlanMode", input: {} }];
  await mountPane();
  fireEvent.click(screen.getByRole("button", { name: "Approve plan" }));
  expect(screen.getByRole("button", { name: "Approve plan" })).toBeTruthy();
  await act(async () => reject(new Error("mode unavailable")));
  expect((screen.getByRole("combobox", { name: "Permission mode" }) as HTMLSelectElement).value).toBe("default");
  expect(screen.getByRole("button", { name: "Approve plan" })).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_permission")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Approve plan" }));
  await act(async () => complete());
  expect(screen.queryByRole("button", { name: "Approve plan" })).toBeNull();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_permission")).toBe(true);
});

it("marks a saved Codex permission choice for the next turn without stopping or answering approvals", async () => {
  snapshotOverrides = { mode: "auto" };
  permissions = [{ id: "r", subtype: "can_use_tool", tool_name: "Bash", input: {} }];
  await mountPane("codex");
  const select = screen.getByRole("combobox", { name: "Permission mode" }) as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "full-access" } });
  expect(select.value).toBe("auto");
  expect(screen.queryByText("Next turn")).toBeNull();
  await act(async () => {
    eventCallback({ type: "settingsChanged", mode: "full-access", pendingPermissionMode: { current: "auto", next: "full-access" } });
    complete();
  });
  expect(select.value).toBe("full-access");
  expect(screen.getByText("Next turn").getAttribute("title")).toContain("Full Access will apply to the next turn");
  expect(screen.getByRole("button", { name: "Approve plan" })).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_interrupt" || command === "chat_permission")).toBe(false);
  // A new native turn notification, steering, and unrelated settings are not policy acknowledgements.
  act(() => {
    eventCallback({ type: "steerAccepted" });
    eventCallback({ type: "settingsChanged", model: "new-model" });
    eventCallback({ type: "turnCompleted" });
    eventCallback({ type: "turnStarted", startedAt: Date.now() });
  });
  expect(screen.getByText("Next turn")).toBeTruthy();
  act(() => eventCallback({ type: "settingsChanged", mode: "full-access", pendingPermissionMode: null }));
  expect(select.value).toBe("full-access");
  expect(screen.queryByText("Next turn")).toBeNull();
});

it("restores a pending permission choice from the backend when reopening the pane", async () => {
  snapshotOverrides = { mode: "full-access", pendingPermissionMode: { current: "auto", next: "full-access" } };
  const view = await mountPane("codex");
  expect(screen.getByText("Next turn")).toBeTruthy();
  view.unmount();
  await mountPane("codex");
  expect(screen.getByText("Next turn")).toBeTruthy();
  act(() => eventCallback({ type: "settingsChanged", mode: "read-only", pendingPermissionMode: { current: "auto", next: "read-only" } }));
  expect((screen.getByRole("combobox", { name: "Permission mode" }) as HTMLSelectElement).value).toBe("read-only");
  expect(screen.getByText("Next turn").getAttribute("title")).toContain("Read Only");
  // Choosing the current permissions again cancels the deferred change.
  act(() => eventCallback({ type: "settingsChanged", mode: "auto", pendingPermissionMode: null }));
  expect(screen.queryByText("Next turn")).toBeNull();
  act(() => eventCallback({ type: "settingsChanged", mode: "full-access", pendingPermissionMode: { current: "auto", next: "full-access" } }));
  act(() => eventCallback({ type: "reset" }));
  expect(screen.queryByText("Next turn")).toBeNull();
});

it("retains the confirmed pending choice when saving another permission mode fails", async () => {
  snapshotOverrides = { mode: "full-access", pendingPermissionMode: { current: "auto", next: "full-access" } };
  await mountPane("codex");
  const select = screen.getByRole("combobox", { name: "Permission mode" }) as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "read-only" } });
  await act(async () => reject(new Error("mode unavailable")));
  expect(select.value).toBe("full-access");
  expect(screen.getByText("Next turn").getAttribute("title")).toContain("Full Access");
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_interrupt" || command === "chat_permission")).toBe(false);
});

it("clears the Codex effort override when switching to a model without a saved level", async () => {
  await mountPane("codex");
  act(() => eventCallback({ type: "session", agentSessionId: "thread-1", model: "old-model", effort: "high" }));
  fireEvent.change(screen.getByRole("combobox", { name: "Model" }), { target: { value: "new-model" } });
  await act(async () => complete());
  expect(invoke).toHaveBeenCalledWith("chat_set_effort", { sessionId: "s", effort: undefined });
  expect((screen.getByRole("combobox", { name: "Model" }) as HTMLSelectElement).value).toBe("new-model");
  expect(useTermStore.getState().chatModelByKind.codex).toBe("new-model");
});

it("follows settings confirmed on another view, including resetting the model to default", async () => {
  await mountPane();
  act(() => eventCallback({ type: "settingsChanged", model: "new-model", mode: "plan" }));
  expect((screen.getByRole("combobox", { name: "Model" }) as HTMLSelectElement).value).toBe("new-model");
  expect((screen.getByRole("combobox", { name: "Permission mode" }) as HTMLSelectElement).value).toBe("plan");
  act(() => eventCallback({ type: "settingsChanged", model: null }));
  expect((screen.getByRole("combobox", { name: "Model" }) as HTMLSelectElement).value).toBe("");
});

for (const kind of ["claude", "codex", "opencode"] as const) {
  it(`${kind}: exposes steering and keeps rejected input without queueing`, async () => {
    const { container } = await mountPane(kind);
    act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: kind, agentState: "working" } } }));
    expect(screen.getByText(/Stop · Esc/)).toBeTruthy();
    const input = container.querySelector(".sv-composer textarea") ?? screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "Use the smaller change" } });
    const previous = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((command, args) => command === "chat_send"
      ? Promise.reject(new Error("Steering refused")) : previous(command, args));
    fireEvent.click(screen.getByRole("button", { name: "Steer" }));
    await screen.findByText("Error: Steering refused");
    expect((input as HTMLTextAreaElement).value).toBe("");
    expect(screen.getByText("Use the smaller change")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({ behavior: "steer", text: "Use the smaller change" }));
    vi.mocked(invoke).mockImplementation(previous);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect((input as HTMLTextAreaElement).value).toBe(""));
    act(() => eventCallback({ type: "steerAccepted" }));
    expect(screen.getByText("Steering message sent")).toBeTruthy();
  });
}

it("shows stopping immediately on one Escape and completion only after confirmation", async () => {
  const { container } = await mountPane();
  act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: "claude", agentState: "working" } } }));
  fireEvent.keyDown(container.querySelector(".term-mount")!, { key: "Escape" });
  expect(screen.getByText("Stopping the current turn…")).toBeTruthy();
  expect(screen.queryByText("Current turn stopped")).toBeNull();
  fireEvent.keyDown(container.querySelector(".term-mount")!, { key: "Escape" });
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_interrupt")).toHaveLength(1);
  act(() => eventCallback({ type: "turnInterrupted" }));
  expect(screen.getByText("Current turn stopped")).toBeTruthy();
});

it("surfaces a stop failure and allows retry", async () => {
  const { container } = await mountPane();
  act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: "claude", agentState: "working" } } }));
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_interrupt"
    ? Promise.reject(new Error("Stop failed")) : previous(command, args));
  fireEvent.keyDown(container.querySelector(".term-mount")!, { key: "Escape" });
  await screen.findByText("Error: Stop failed");
  expect(screen.queryByText("Stopping the current turn…")).toBeNull();
  vi.mocked(invoke).mockImplementation(previous);
  fireEvent.click(container.querySelector(".sv-stop")!);
  expect(screen.getByText("Stopping the current turn…")).toBeTruthy();
});

it.each(["claude", "codex", "opencode"] as const)("%s: clears submitted text before acknowledgement and preserves the next draft", async (kind) => {
  const { container } = await mountPane(kind);
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  let accept!: () => void;
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_send"
    ? new Promise<string>((resolve) => { accept = () => resolve("queued"); }) as Promise<never> : previous(command, args));
  fireEvent.change(input, { target: { value: "Submitted message" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(input.value).toBe("");
  expect(input.readOnly).toBe(false);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_send", expect.anything()));
  const messageId = vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")![1]!.messageId as string;
  act(() => eventCallback({ type: "queued", items: [{ id: messageId, text: "Submitted message" }] }));
  expect(screen.getByText("Submitted message")).toBeTruthy();
  expect(input.value).toBe("");
  fireEvent.keyDown(input, { key: "Enter" });
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_send")).toHaveLength(1);
  fireEvent.change(input, { target: { value: "Next draft" } });
  await act(async () => accept());
  expect(input.readOnly).toBe(false);
  expect(input.value).toBe("Next draft");
});

it.each(["chat_start", "chat_send"])("keeps the failed message without overwriting new input when %s fails", async (failedCommand) => {
  snapshotOverrides = { running: failedCommand !== "chat_start" };
  const { container } = await mountPane();
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  let refuse!: (error: Error) => void;
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === failedCommand
    ? new Promise<void>((_, reject) => { refuse = reject; }) as Promise<never> : previous(command, args));
  fireEvent.change(input, { target: { value: "  Original draft\n" } });
  input.setSelectionRange(5, 5);
  fireEvent.keyDown(input, { key: "Enter" });
  expect(input.value).toBe("");
  fireEvent.change(input, { target: { value: "Next draft" } });
  await waitFor(() => expect(refuse).toBeTypeOf("function"));
  await act(async () => refuse(new Error("Submission failed")));
  expect(input.value).toBe("Next draft");
  expect(screen.getByText("Original draft")).toBeTruthy();
  expect(input.readOnly).toBe(false);
  expect(screen.getByText("Error: Submission failed")).toBeTruthy();
});

it("restores steering attachments on failure and clears them on retry", async () => {
  const { container } = await mountPane("codex");
  act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: "codex", agentState: "working" } } }));
  fireEvent.drop(container.querySelector(".sv-composer")!, { dataTransfer: { files: [new File(["png"], "example.png", { type: "image/png" })] } });
  await screen.findByAltText("example.png");
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_send"
    ? Promise.reject(new Error("Image rejected")) : previous(command, args));
  fireEvent.click(screen.getByRole("button", { name: "Steer" }));
  await screen.findByText("Error: Image rejected");
  expect(container.querySelector(".sv-submission img")).toBeTruthy();
  expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({ behavior: "steer", images: [{ mimeType: "image/png", data: "cG5n" }] }));
  vi.mocked(invoke).mockImplementation(previous);
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(screen.getByText("Sent")).toBeTruthy());
  expect(screen.queryByAltText("example.png")).toBeNull();
});

it("gives Escape to queue editing and open menus before stopping", async () => {
  const { container } = await mountPane();
  act(() => {
    useTermStore.setState({ runtimes: { s: { status: "running", agent: "claude", agentState: "working" } } });
    eventCallback({ type: "queued", items: [{ id: "q", text: "Later" }] });
  });
  fireEvent.click(screen.getByText("Later"));
  fireEvent.keyDown(container.querySelector(".sv-queue-edit")!, { key: "Escape" });
  expect(container.querySelector(".sv-queue-edit")).toBeNull();
  expect(invoke).not.toHaveBeenCalledWith("chat_interrupt", expect.anything());
  const option = document.createElement("div");
  option.setAttribute("role", "option");
  container.querySelector(".term-mount")!.append(option);
  fireEvent.keyDown(container.querySelector(".term-mount")!, { key: "Escape" });
  expect(invoke).not.toHaveBeenCalledWith("chat_interrupt", expect.anything());
  option.remove();
  fireEvent.keyDown(container.querySelector(".term-mount")!, { key: "Escape" });
  expect(invoke).toHaveBeenCalledWith("chat_interrupt", { sessionId: "s" });
});

it.each(["claude", "codex", "opencode"] as const)("restores %s conversation settings instead of global defaults before its process starts", async (kind) => {
  snapshotOverrides = { running: false, selection: { model: "new-model", effort: "high" } };
  await mountPane(kind, "new-model");
  const effort = screen.getByRole("combobox", { name: "Thinking effort" }) as HTMLSelectElement;
  expect(effort.value).toBe("high");
  fireEvent.change(effort, { target: { value: "" } });
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_set_effort", { sessionId: "s", effort: undefined }));
  expect(effort.value).toBe("");
  fireEvent.change(screen.getByRole("combobox", { name: "Model" }), { target: { value: "old-model" } });
  await act(async () => complete());
  expect(invoke).toHaveBeenCalledWith("chat_set_model", { sessionId: "s", model: "old-model" });
});

it.each(["claude", "codex", "opencode"] as const)("keeps explicit automatic %s settings after reopening", async (kind) => {
  useTermStore.setState({ chatModelByKind: { [kind]: "old-model" }, chatEffortByModel: { [`${kind}:old-model`]: "high" } });
  snapshotOverrides = { running: false, selection: { model: null, effort: null } };
  await mountPane(kind, "");
  expect((screen.getByRole("combobox", { name: "Thinking effort" }) as HTMLSelectElement).value).toBe("");
});

it("searches the focused session and closes without interrupting a running turn", async () => {
  snapshotOverrides = { rows: [{ kind: "user", id: "search-message", text: "Searchable history" }] };
  useTermStore.setState({ searchOpen: false });
  const { container } = await mountPane();
  fireEvent.click(screen.getByTitle("Search…"));
  const input = screen.getByRole("textbox", { name: "Search transcript…" });
  fireEvent.change(input, { target: { value: "history" } });
  expect(screen.getByRole("status").textContent).toBe("1/1");
  expect(container.querySelector(".sv-search-match")?.textContent).toContain("Searchable history");
  vi.mocked(invoke).mockClear();
  fireEvent.keyDown(input, { key: "Escape" });
  expect(screen.queryByRole("search")).toBeNull();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_interrupt")).toBe(false);
});


it.each([
  ["codex", "/vsp", "$vspawn "], ["codex", "$vsp", "$vspawn "],
  ["claude", "/vsp", "/vspawn "], ["claude", "$vsp", "/vspawn "],
] as const)("loads skills before the first message: %s %s", async (kind, draft, native) => {
  snapshotOverrides = { running: false, commands: [] };
  const fallback = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_commands"
    ? Promise.resolve([{ name: "vspawn", description: "Create a child session", invocation: kind === "codex" ? "$" : "/" }]) as Promise<never>
    : fallback(command, args));
  const { container } = await mountPane(kind);
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: draft, selectionStart: draft.length } });
  await screen.findByRole("button", { name: /vspawn/ });
  expect(invoke).toHaveBeenCalledWith("chat_commands", { sessionId: "s" });
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === "chat_start" || name === "chat_send")).toBe(false);
  fireEvent.keyDown(input, { key: "Tab" });
  expect(input.value).toBe(native);
  fireEvent.change(input, { target: { value: native + "inspect the project" } });
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({ text: native + "inspect the project" })));
});


it("waits for the skill catalogue and retries a failed lookup without sending the partial name", async () => {
  snapshotOverrides = { running: false, commands: [] };
  const fallback = vi.mocked(invoke).getMockImplementation()!;
  let failLookup: (error: Error) => void = () => {};
  let retry = false;
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command !== "chat_commands") return fallback(command, args);
    if (retry) return Promise.resolve([{ name: "vspawn", invocation: "$" }]) as Promise<never>;
    return new Promise((_, reject) => { failLookup = reject; }) as Promise<never>;
  });
  const { container } = await mountPane("codex");
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "$vsp", selectionStart: 4 } });
  await screen.findByRole("status");
  fireEvent.keyDown(input, { key: "Enter" });
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === "chat_send")).toBe(false);
  await act(async () => failLookup(new Error("Skill lookup failed")));
  await screen.findByText(/Skill lookup failed/);
  retry = true;
  fireEvent.change(input, { target: { value: "" } });
  fireEvent.change(input, { target: { value: "$vsp", selectionStart: 4 } });
  await screen.findByRole("button", { name: "$vspawn" });
  expect(screen.queryByText(/Skill lookup failed/)).toBeNull();
});


it("keeps following layout growth and viewport changes, but preserves manual history browsing", async () => {
  const observers = new Map<ResizeObserver, { callback: ResizeObserverCallback; targets: Set<Element> }>();
  class Observer {
    constructor(callback: ResizeObserverCallback) { observers.set(this as unknown as ResizeObserver, { callback, targets: new Set() }); }
    observe(target: Element) { observers.get(this as unknown as ResizeObserver)!.targets.add(target); }
    unobserve(target: Element) { observers.get(this as unknown as ResizeObserver)!.targets.delete(target); }
    disconnect() { observers.delete(this as unknown as ResizeObserver); }
  }
  vi.stubGlobal("ResizeObserver", Observer);
  try {
    const { container } = await mountPane();
    const scroll = container.querySelector<HTMLDivElement>(".sv-scroll")!;
    let height = 1200;
    let viewport = 400;
    let top = 0;
    Object.defineProperties(scroll, {
      scrollHeight: { get: () => height },
      clientHeight: { get: () => viewport },
      scrollTop: { get: () => top, set: (value: number) => { top = Math.max(0, Math.min(value, height - viewport)); } },
    });
    const resize = () => act(() => {
      for (const [observer, record] of [...observers]) {
        if (record.targets.has(scroll)) record.callback([], observer);
      }
    });
    resize();
    expect(top).toBe(800);
    // A layout-induced scroll can arrive before ResizeObserver.
    height = 1800;
    fireEvent.scroll(scroll);
    expect(top).toBe(1400);
    expect(container.querySelector(".sv-to-end")).toBeNull();
    viewport = 250;
    resize();
    expect(top).toBe(1550);
    height = 2100;
    resize();
    expect(top).toBe(1850);
    // Actual upward scrolling parks history, even as later content grows.
    scroll.scrollTop = 900;
    fireEvent.scroll(scroll);
    expect(container.querySelector(".sv-to-end")).toBeTruthy();
    height = 2500;
    resize();
    expect(top).toBe(900);
    const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
    fireEvent.change(input, { target: { value: "Continue" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(input.value).toBe(""));
    expect(top).toBe(2250);
    height = 2800;
    resize();
    expect(top).toBe(2550);
    expect(container.querySelector(".sv-to-end")).toBeNull();
  } finally {
    cleanup();
    vi.unstubAllGlobals();
  }
});

it("offers only Codex conversation rewind and sends no file restore request", async () => {
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: "u-native", text: "Edit the fixture" }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === "chat_rewind") return Promise.resolve({ prefillText: "Edit the fixture" }) as Promise<never>;
    return previous(command, args);
  });
  const { container } = await mountPane("codex");
  fireEvent.click(await screen.findByRole("button", { name: "Rewind from here" }));
  expect(screen.queryByRole("button", { name: "Restore files" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Rewind conversation and restore files" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Rewind conversation" }));
  fireEvent.click(screen.getByRole("button", { name: "Rewind" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_rewind", {
    sessionId: "s", rowId: "u-native", scope: "conversation",
  }));
  await waitFor(() => expect(container.querySelector<HTMLTextAreaElement>(".sv-box textarea")?.value).toBe("Edit the fixture"));
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_rewind_preview")).toBe(false);
});

it("receives native rewind scopes when Codex starts after the initial snapshot", async () => {
  snapshotOverrides = { running: false, rewindScopes: [], rows: [{ kind: "user", id: "u-native", text: "An earlier request" }] };
  await mountPane("codex");
  expect(screen.queryByRole("button", { name: "Rewind from here" })).toBeNull();
  act(() => eventCallback({ type: "process", pid: 123, startedAt: 100, rewindScopes: ["conversation"] }));
  fireEvent.click(await screen.findByRole("button", { name: "Rewind from here" }));
  expect(screen.getByRole("button", { name: "Rewind conversation" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Restore files" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Rewind conversation and restore files" })).toBeNull();
});

for (const kind of ["claude", "codex", "opencode"] as const) {
  it(`${kind}: steers a queued message by ID, blocks duplicates, and preserves it on rejection`, async () => {
    snapshotOverrides = { queue: [{ id: "q-steer", text: "Queued correction" }] };
    const { container } = await mountPane(kind);
    act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: kind, agentState: "working" } } }));
    const previous = vi.mocked(invoke).getMockImplementation()!;
    let accept!: () => void;
    let refuse!: (error: Error) => void;
    vi.mocked(invoke).mockImplementation((command, args) => command === "chat_queue_steer"
      ? new Promise<void>((resolve, reject) => { accept = resolve; refuse = reject; }) as Promise<never>
      : previous(command, args));
    const button = container.querySelector<HTMLButtonElement>(".sv-queue-steer")!;
    expect(button.disabled).toBe(false);
    fireEvent.click(button);
    fireEvent.click(button);
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_queue_steer")).toHaveLength(1);
    expect(invoke).toHaveBeenCalledWith("chat_queue_steer", { sessionId: "s", id: "q-steer" });
    expect(screen.getByText("Queued correction")).toBeTruthy();
    await act(async () => refuse(new Error("Native refusal")));
    expect(screen.getByText("Error: Native refusal")).toBeTruthy();
    expect(screen.getByText("Queued correction")).toBeTruthy();
    expect(button.disabled).toBe(false);
    fireEvent.click(button);
    await act(async () => accept());
    expect(container.querySelector(".sv-queue")).toBeNull();
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send" || command === "chat_queue_remove")).toBe(false);
  });
}
