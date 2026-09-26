import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { md } from "../../../markdownEngine";
import type { ChatEvent, ChatPermission, ChatSnapshot } from "../../../ipc/chat";
import type { SessionPermissionState } from "../../../hooks/useSessionPermissionState";
import type { PermissionAnswer } from "./permissionCards";

vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(), onTransportReconnect: vi.fn(),
}));
vi.mock("./engineSwitch", () => ({ useEngineSwitch: () => ({ switchTo: vi.fn(), confirm: null }) }));
// Exercise the pane's orchestration; menu layout is covered separately by the controls themselves.
vi.mock("./controls", () => ({
  LevelBar: () => null,
  ControlChip: ({ title, label, value, options, onPick }: {
    title: string; label?: string; value: string; options: { value: string; label: string; tag?: string }[];
    onPick: (value: string, keep: boolean) => void;
  }) => <select aria-label={title} data-label={label} value={value} onChange={(event) => onPick(event.target.value, true)}>
    {options.map((option) => <option key={option.value} value={option.value}>{option.label}{option.tag ? ` — ${option.tag}` : ""}</option>)}
  </select>,
}));
vi.mock("./permissionCards", () => ({
  PermissionCard: ({ request, onAnswer }: {
    request: ChatPermission; onAnswer: (request: ChatPermission, reply: PermissionAnswer) => void;
  }) => <button onClick={() => onAnswer(request, { allow: true, mode: "acceptEdits" })}>Approve plan</button>,
}));

import { invoke, listen, onTransportReconnect } from "../../../ipc/transport";
import { COMPOSER_CHIP_IDS, DEFAULT_COMPOSER_INLINE_CHIPS } from "../../../store/settings";
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
  const reconnectListeners = new Set<() => void>();
  reconnectCallback = () => reconnectListeners.forEach(callback => callback());
  vi.mocked(onTransportReconnect).mockImplementation(callback => {
    reconnectListeners.add(callback); return () => { reconnectListeners.delete(callback); };
  });
  const chatListeners = new Set<(event: ChatEvent) => void>();
  eventCallback = event => chatListeners.forEach(callback => callback(event));
  vi.mocked(listen).mockImplementation((name, callback) => {
    const listener = callback as (event: ChatEvent) => void;
    if (name === "chat://event/s") chatListeners.add(listener);
    return Promise.resolve(() => { chatListeners.delete(listener); });
  });
  // Every chip inline: jsdom reports no widths, so nothing folds into a More row and the assertions below
  // can reach every chip directly.
  useTermStore.setState({ chatModel: "old-model", chatModelByKind: { claude: "old-model" }, chatEffortByModel: {}, runtimes: {}, agentDefaults: {}, composerInlineChips: [...COMPOSER_CHIP_IDS] });
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === "agent_permission_catalog") {
      const agent = (args as { agent: string }).agent;
      return Promise.resolve({ modes: agent === "codex" ? ["read-only", "auto", "full-access"]
        : agent === "opencode" ? ["default", "bypassPermissions"]
        : ["plan", "default", "acceptEdits", "auto", "bypassPermissions"], selected: "default" }) as Promise<never>;
    }
    if (command === "agent_set_default_permission") {
      const { agent, mode } = args as { agent: string; mode: string };
      return Promise.resolve({ [agent]: { permissionMode: mode } }) as Promise<never>;
    }
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

async function mountPane(kind: "claude" | "codex" | "opencode" = "claude", expectedModel: string | null = "old-model", mobile = false) {
  const view = render(<ChatPane session={{ id: "s", projectId: "p", name: "Claude", kind, engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
    area={{}} hidden={false} focused multi={false} mobile={mobile} onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />);
  if (expectedModel !== null) await waitFor(() => expect((screen.getByRole("combobox", { name: "Model" }) as HTMLSelectElement).value).toBe(expectedModel));
  else await waitFor(() => expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_snapshot")).toBe(true));
  return view;
}

/** Serve `session_permission_state` from a value the test can change between events. */
function mockPermissionState(read: () => SessionPermissionState) {
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "session_permission_state"
    ? Promise.resolve(read()) as Promise<never> : previous(command, args));
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

it("recalls session inputs in order while busy and restores the unsent draft", async () => {
  snapshotOverrides = { rows: [
    { kind: "user", id: "old", text: "First input" },
    { kind: "assistant", id: "answer", text: "Do not recall answers", streaming: false },
    { kind: "user", id: "image", text: "" },
    { kind: "user", id: "external", text: "Another session's message", origin: {
      sessionId: "other", name: "Other", agent: "codex", role: "session",
    } },
    { kind: "user", id: "new", text: "Second input" },
  ], queue: [
    { id: "queued", text: "Queued input" },
    { id: "external-queued", text: "Another session's queued message", origin: {
      sessionId: "other", name: "Other", agent: "codex", role: "session",
    } },
  ] };
  useTermStore.setState({ runtimes: { s: { status: "running", agent: "codex", agentState: "working" } } });
  const { container } = await mountPane("codex");
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "Unsent draft" } });
  input.setSelectionRange(0, 0);
  for (const expected of ["Queued input", "Second input", "First input", "First input"]) {
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(input.value).toBe(expected);
    expect(input.selectionStart).toBe(0);
  }
  for (const expected of ["Second input", "Queued input", "Unsent draft"]) {
    input.setSelectionRange(input.value.length, input.value.length);
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input.value).toBe(expected);
  }
  expect(input.selectionStart).toBe(0);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send")).toBe(false);
});

it("leaves multiline caret movement, selection, modified arrows and IME input to the editor", async () => {
  snapshotOverrides = { rows: [{ kind: "user", id: "old", text: "History\nContinuation" }] };
  const { container } = await mountPane();
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "First line\nSecond line" } });
  for (const [start, end, options] of [
    [12, 12, {}], [0, 5, {}], [0, 0, { shiftKey: true }], [0, 0, { ctrlKey: true }],
    [0, 0, { altKey: true }], [0, 0, { metaKey: true }], [0, 0, { isComposing: true }],
    [0, 0, { keyCode: 229 }],
  ] as const) {
    input.setSelectionRange(start, end);
    expect(fireEvent.keyDown(input, { key: "ArrowUp", ...options })).toBe(true);
    expect(input.value).toBe("First line\nSecond line");
  }
  input.setSelectionRange(0, 0);
  fireEvent.keyDown(input, { key: "ArrowUp" });
  expect(input.value).toBe("History\nContinuation");
  expect(fireEvent.keyDown(input, { key: "ArrowDown" })).toBe(true);
  expect(input.value).toBe("History\nContinuation");
});

it("keeps the history position when queued input becomes a row and when newer input arrives", async () => {
  snapshotOverrides = { rows: [{ kind: "user", id: "old", text: "Same input" }],
    queue: [{ id: "queued", text: "Same input" }] };
  const { container } = await mountPane();
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.keyDown(input, { key: "ArrowUp" });
  act(() => {
    eventCallback({ type: "rows", rows: [
      { kind: "user", id: "queued", text: "Same input" },
      { kind: "user", id: "latest", text: "New input" },
    ] });
    eventCallback({ type: "queued", items: [] });
  });
  fireEvent.keyDown(input, { key: "ArrowUp" });
  expect(input.value).toBe("Same input");
  input.setSelectionRange(input.value.length, input.value.length);
  fireEvent.keyDown(input, { key: "ArrowDown" });
  expect(input.value).toBe("Same input");
  expect(input.selectionStart).toBe(input.value.length);
  fireEvent.keyDown(input, { key: "ArrowDown" });
  expect(input.value).toBe("New input");
  fireEvent.keyDown(input, { key: "ArrowDown" });
  expect(input.value).toBe("");
});

it("recalls a submitted edit as the latest input and starts a fresh draft after sending", async () => {
  snapshotOverrides = { rows: [{ kind: "user", id: "old", text: "History" }] };
  const { container } = await mountPane();
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "Original draft" } });
  input.setSelectionRange(0, 0);
  fireEvent.keyDown(input, { key: "ArrowUp" });
  fireEvent.change(input, { target: { value: "Edited history" } });
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(useOutbox.getState().sessions.s?.[0].status).toBe("sent"));
  fireEvent.keyDown(input, { key: "ArrowUp" });
  expect(input.value).toBe("Edited history");
  input.setSelectionRange(input.value.length, input.value.length);
  fireEvent.keyDown(input, { key: "ArrowDown" });
  expect(input.value).toBe("");
});

it("loads earlier pages once, preserves live messages, and reaches the first message", async () => {
  useTermStore.setState({ searchOpen: false });
  snapshotOverrides = { startedAt: 17, rowsRevision: 1, hasMore: true,
    positions: { recent: 1000 }, rows: [{ kind: "user", id: "recent", text: "Recent message" }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  let finishHistory!: (value: unknown) => void;
  const pages: string[] = [];
  vi.mocked(invoke).mockImplementation((command, args) => {
    const before = (args?.window as { before?: string } | undefined)?.before;
    if (command !== "chat_snapshot" || !before) return previous(command, args);
    pages.push(before);
    if (before === "recent") return new Promise<unknown>(resolve => { finishHistory = resolve; }) as Promise<never>;
    return Promise.resolve({ ...snapshotOverrides, pageKind: "history", hasMore: false,
      positions: { first: 0 }, rows: [{ kind: "user", id: "first", text: "First message" }] }) as Promise<never>;
  });
  const { container } = await mountPane();
  fireEvent.click(container.querySelector(".sv-history-more")!);
  fireEvent.click(container.querySelector(".sv-history-more")!);
  expect(pages).toEqual(["recent"]);
  act(() => eventCallback({ type: "rows", epoch: 17, revision: 3, positions: { live: 1001 },
    rows: [{ kind: "user", id: "live", text: "Live message" }] }));
  await act(async () => finishHistory({ ...snapshotOverrides, pageKind: "history", hasMore: true,
    positions: { older: 400 }, rows: [{ kind: "user", id: "older", text: "Older message" }] }));
  fireEvent.click(container.querySelector(".sv-history-more")!);
  await waitFor(() => expect(container.querySelector(".sv-history-more")).toBeNull());
  expect(pages).toEqual(["recent", "older"]);
  expect([...container.querySelectorAll(".sv-item[data-search-id]")].map(item => item.getAttribute("data-search-id")))
    .toEqual(["first", "older", "recent", "live"]);
  expect(screen.getByText("Live message")).toBeTruthy();
});

it("keeps failed history loading retryable beside the earlier-message control", async () => {
  useTermStore.setState({ searchOpen: false });
  snapshotOverrides = { hasMore: true, rows: [{ kind: "user", id: "recent", text: "Recent message" }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  let attempts = 0;
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command !== "chat_snapshot" || !(args?.window as { before?: string } | undefined)?.before) return previous(command, args);
    attempts += 1;
    return (attempts === 1 ? Promise.reject(new Error("History temporarily unavailable")) : Promise.resolve({
      ...snapshotOverrides, pageKind: "history", hasMore: false,
      rows: [{ kind: "user", id: "first", text: "First message" }],
    })) as Promise<never>;
  });
  const { container } = await mountPane();
  fireEvent.click(container.querySelector(".sv-history-more")!);
  await waitFor(() => expect(container.querySelector(".sv-history [role=alert]")?.textContent).toContain("History temporarily unavailable"));
  expect(screen.getByText("Recent message")).toBeTruthy();
  fireEvent.scroll(container.querySelector(".sv-scroll")!);
  expect(attempts).toBe(1);
  fireEvent.click(container.querySelector(".sv-history-more")!);
  await waitFor(() => expect(screen.getByText("First message")).toBeTruthy());
  expect(container.querySelector(".sv-history")).toBeNull();
  expect(attempts).toBe(2);
});

it("loads older pages until an input is found and can recall a slash command without completing it", async () => {
  snapshotOverrides = { hasMore: true, rows: [{ kind: "user", id: "recent", text: "Recent input" }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => {
    const before = (args?.window as { before?: string } | undefined)?.before;
    if (command === "chat_snapshot" && before) return Promise.resolve({
      ...snapshotOverrides, pageKind: "history", hasMore: before === "recent",
      rows: before === "recent"
        ? [{ kind: "assistant", id: "answer", text: "Older answer", streaming: false }]
        : [{ kind: "user", id: "older", text: "/review" }],
    }) as Promise<never>;
    return previous(command, args);
  });
  const { container } = await mountPane("codex");
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.keyDown(input, { key: "ArrowUp" });
  expect(input.value).toBe("Recent input");
  fireEvent.keyDown(input, { key: "ArrowUp" });
  await waitFor(() => expect(input.value).toBe("/review"));
  expect(container.querySelector(".sv-complete")).toBeNull();
  input.setSelectionRange(input.value.length, input.value.length);
  fireEvent.keyDown(input, { key: "ArrowDown" });
  expect(input.value).toBe("Recent input");
});

it.each(["edit", "down"])("does not overwrite the draft when older history finishes after %s", async action => {
  snapshotOverrides = { hasMore: true, rows: [{ kind: "assistant", id: "answer", text: "Answer", streaming: false }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  let finishHistory!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_snapshot" && (args?.window as { before?: string } | undefined)?.before
    ? new Promise<unknown>(resolve => { finishHistory = resolve; }) as Promise<never> : previous(command, args));
  const { container } = await mountPane();
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.keyDown(input, { key: "ArrowUp" });
  await waitFor(() => expect(finishHistory).toBeTypeOf("function"));
  if (action === "edit") fireEvent.change(input, { target: { value: "Keep the new draft" } });
  else fireEvent.keyDown(input, { key: "ArrowDown" });
  await act(async () => finishHistory({ ...snapshotOverrides, pageKind: "history", hasMore: false,
    rows: [{ kind: "user", id: "old", text: "Old input" }] }));
  expect(input.value).toBe(action === "edit" ? "Keep the new draft" : "");
});

it("labels OpenCode replies with their own models instead of the current selection", async () => {
  const { container } = await mountPane("opencode");
  act(() => eventCallback({ type: "replaceRows", rows: [
    { kind: "user", id: "u1", text: "First question" },
    { kind: "assistant", id: "a", text: "First answer", streaming: false, model: "deepseek-v4-pro" },
    { kind: "user", id: "u2", text: "Second question" },
    { kind: "assistant", id: "b", text: "Second answer", streaming: true, model: "glm-5.3" },
    { kind: "user", id: "u3", text: "Third question" },
    { kind: "assistant", id: "c", text: "Legacy answer", streaming: false },
  ] }));
  const authors = () => Array.from(container.querySelectorAll(".sv-msg-who"), (node) => node.textContent);
  await waitFor(() => expect(authors()).toEqual(["You", "deepseek-v4-pro", "You", "glm-5.3", "You", "OpenCode"]));
  act(() => eventCallback({ type: "settingsChanged", model: "new-model" }));
  expect(authors()).toEqual(["You", "deepseek-v4-pro", "You", "glm-5.3", "You", "OpenCode"]);
  act(() => eventCallback({ type: "rows", rows: [
    { kind: "assistant", id: "b", text: "Second answer completed", streaming: false, model: "glm-5.3" },
  ] }));
  expect(authors()).toEqual(["You", "deepseek-v4-pro", "You", "glm-5.3", "You", "OpenCode"]);
});

it("opens the agent's line above its reasoning, not under the prompt", async () => {
  const { container } = await mountPane();
  act(() => eventCallback({ type: "replaceRows", rows: [
    { kind: "user", id: "u1", text: "Question" },
    { kind: "reasoning", id: "r1", text: "Thinking it over", streaming: false },
    { kind: "assistant", id: "a1", text: "Answer", streaming: false, model: "some-model" },
  ] }));
  const reasoning = container.querySelector('[data-search-id="r1"]')!;
  expect(reasoning.querySelector(".sv-turn-head .sv-msg-who")?.textContent).toBe("some-model");
  expect(container.querySelector('[data-search-id="a1"] .sv-msg-head')).toBeNull();
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

it("keeps the saved restart notice after Later without restarting or changing current permissions", async () => {
  await mountPane();
  const select = screen.getByRole("combobox", { name: "Permission mode" }) as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "bypassPermissions" } });
  await act(async () => reject(new Error("CHAT_PERMISSION_RESTART_REQUIRED:123")));
  expect(screen.getByRole("alertdialog")).toBeTruthy();
  expect(select.value).toBe("default");
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_restart_permission_mode")).toBe(false);
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "session_permission_state"
    ? Promise.resolve({ configured: "bypassPermissions", current: "default", launch: "default",
      pending: "bypassPermissions", activation: "restart", running: true }) as Promise<never>
    : previous(command, args));
  fireEvent.click(screen.getByRole("button", { name: "Later" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  act(() => reconnectCallback());
  expect(await screen.findByText("Applies after restarting this session: Bypass")).toBeTruthy();
  expect(select.value).toBe("default");
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_restart_permission_mode")).toBe(false);
});

it("turns a late permission rejection event into the restart confirmation", async () => {
  await mountPane();
  act(() => eventCallback({ type: "error", message: "CHAT_PERMISSION_RESTART_REQUIRED:123" }));
  expect(screen.getByRole("alertdialog")).toBeTruthy();
  expect((screen.getByRole("combobox", { name: "Permission mode" }) as HTMLSelectElement).value).toBe("default");
});

it.each(["success", "rejected", "disconnected"])("handles a confirmed Bypass restart: %s", async outcome => {
  const accepted = outcome === "success";
  await mountPane();
  const select = screen.getByRole("combobox", { name: "Permission mode" }) as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "bypassPermissions" } });
  await act(async () => reject(new Error("CHAT_PERMISSION_RESTART_REQUIRED:123")));
  let finish!: () => void;
  let fail!: (error: Error) => void;
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_restart_permission_mode"
    ? new Promise<void>((resolve, reject) => { finish = resolve; fail = reject; }) as Promise<never>
    : previous(command, args));
  fireEvent.click(screen.getByRole("button", { name: "Restart and apply" }));
  expect(invoke).toHaveBeenCalledWith("chat_restart_permission_mode", { sessionId: "s", pid: 123 });
  expect(select.value).toBe("default");
  expect((screen.getByRole("button", { name: "Restarting…" }) as HTMLButtonElement).disabled).toBe(true);
  const failure = new Error("Policy rejected the permission change");
  if (outcome === "disconnected") failure.name = "TransportError";
  await act(async () => accepted ? finish() : fail(failure));
  expect(select.value).toBe(accepted ? "bypassPermissions" : "default");
  expect(useTermStore.getState().agentDefaults.claude?.permissionMode).toBe(accepted ? "bypassPermissions" : undefined);
  expect(screen.queryByRole("alertdialog")).toBeNull();
  if (outcome === "rejected") expect(screen.getByText(/Permission change failed.*Policy rejected/)).toBeTruthy();
  if (outcome === "disconnected") expect(screen.getByText(/The permission change could not be confirmed/)).toBeTruthy();
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
  let state: SessionPermissionState = { configured: "auto", current: "auto", launch: null,
    pending: null, activation: "applied", running: true };
  mockPermissionState(() => state);
  await mountPane("codex");
  const select = screen.getByRole("combobox", { name: /Permission mode/ }) as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "full-access" } });
  expect(select.value).toBe("auto");
  expect(screen.queryByRole("option", { name: /Next turn/ })).toBeNull();
  await act(async () => {
    state = { configured: "full-access", current: "auto", launch: null,
      pending: "full-access", activation: "nextTurn", running: true };
    eventCallback({ type: "settingsChanged", mode: "full-access", pendingPermissionMode: { current: "auto", next: "full-access" } });
    complete();
  });
  expect(select.value).toBe("full-access");
  // The chip shows the chosen mode; its menu carries the note that the choice waits for the next turn.
  expect((await screen.findByRole("option", { name: /Full Access — Next turn/ })).textContent).toContain("Full Access");
  expect(screen.getByRole("button", { name: "Approve plan" })).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_interrupt" || command === "chat_permission")).toBe(false);
  // A new native turn notification, steering, and unrelated settings are not policy acknowledgements.
  act(() => {
    eventCallback({ type: "steerAccepted" });
    eventCallback({ type: "settingsChanged", model: "new-model" });
    eventCallback({ type: "turnCompleted" });
    eventCallback({ type: "turnStarted", startedAt: Date.now() });
  });
  expect(screen.getByRole("option", { name: /Next turn/ })).toBeTruthy();
  await act(async () => {
    state = { configured: "full-access", current: "full-access", launch: null,
      pending: null, activation: "applied", running: true };
    eventCallback({ type: "settingsChanged", mode: "full-access", pendingPermissionMode: null });
  });
  await waitFor(() => expect(screen.queryByRole("option", { name: /Next turn/ })).toBeNull());
  expect(select.value).toBe("full-access");
});

it("restores a pending permission choice from the backend when reopening the pane", async () => {
  snapshotOverrides = { mode: "full-access", pendingPermissionMode: { current: "auto", next: "full-access" } };
  let state: SessionPermissionState = { configured: "full-access", current: "auto", launch: null,
    pending: "full-access", activation: "nextTurn", running: true };
  mockPermissionState(() => state);
  const view = await mountPane("codex");
  expect(await screen.findByRole("option", { name: /Full Access — Next turn/ })).toBeTruthy();
  view.unmount();
  await mountPane("codex");
  expect(await screen.findByRole("option", { name: /Full Access — Next turn/ })).toBeTruthy();
  state = { configured: "read-only", current: "auto", launch: null,
    pending: "read-only", activation: "nextTurn", running: true };
  act(() => eventCallback({ type: "settingsChanged", mode: "read-only", pendingPermissionMode: { current: "auto", next: "read-only" } }));
  expect((screen.getByRole("combobox", { name: /Permission mode/ }) as HTMLSelectElement).value).toBe("read-only");
  expect(await screen.findByRole("option", { name: /Read Only — Next turn/ })).toBeTruthy();
  // Choosing the current permissions again cancels the deferred change.
  state = { configured: "auto", current: "auto", launch: null, pending: null, activation: "applied", running: true };
  act(() => eventCallback({ type: "settingsChanged", mode: "auto", pendingPermissionMode: null }));
  await waitFor(() => expect(screen.queryByRole("option", { name: /Next turn/ })).toBeNull());
  state = { configured: "full-access", current: "auto", launch: null,
    pending: "full-access", activation: "nextTurn", running: true };
  act(() => eventCallback({ type: "settingsChanged", mode: "full-access", pendingPermissionMode: { current: "auto", next: "full-access" } }));
  expect(await screen.findByRole("option", { name: /Full Access — Next turn/ })).toBeTruthy();
  state = { configured: "full-access", current: "full-access", launch: null, pending: null, activation: "applied", running: true };
  act(() => eventCallback({ type: "reset" }));
  await waitFor(() => expect(screen.queryByRole("option", { name: /Next turn/ })).toBeNull());
});

it("retains the confirmed pending choice when saving another permission mode fails", async () => {
  snapshotOverrides = { mode: "full-access", pendingPermissionMode: { current: "auto", next: "full-access" } };
  mockPermissionState(() => ({ configured: "full-access", current: "auto", launch: null,
    pending: "full-access", activation: "nextTurn", running: true }));
  await mountPane("codex");
  const select = screen.getByRole("combobox", { name: /Permission mode/ }) as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "read-only" } });
  await act(async () => reject(new Error("mode unavailable")));
  expect(select.value).toBe("full-access");
  expect((await screen.findByRole("option", { name: /Full Access — Next turn/ })).textContent).toContain("Full Access");
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

it("shows the backend's initialized extra-argument permission mode without relabeling it as Default", async () => {
  snapshotOverrides = { mode: "dontAsk" };
  await mountPane();
  expect(screen.getByRole("combobox", { name: "Permission mode" }).getAttribute("data-label")).toBe("dontAsk");
  act(() => eventCallback({ type: "settingsChanged", mode: "plan" }));
  expect(screen.getByRole("combobox", { name: "Permission mode" }).getAttribute("data-label")).toBe("Plan Mode");
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

it("keeps the composer idle while a background task holds the session on working", async () => {
  await mountPane();
  act(() => eventCallback({ type: "extras", extras: { backgroundTasks: [
    { task_id: "shell", task_type: "local_bash", description: "Wait in background", can_stop: true },
  ] } }));
  act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: "claude", agentState: "working" } } }));
  expect(screen.queryByText(/Stop · Esc/)).toBeNull();
  // The turn Claude opens to answer the task is a real turn again.
  act(() => eventCallback({ type: "turnStarted", startedAt: Date.now() }));
  expect(screen.getByText(/Stop · Esc/)).toBeTruthy();
});

it("sends normally when Alt+Enter arrives with no running turn", async () => {
  const { container } = await mountPane();
  const input = container.querySelector(".sv-composer textarea") ?? screen.getByRole("textbox");
  fireEvent.change(input, { target: { value: "Hello" } });
  fireEvent.keyDown(input, { key: "Enter", altKey: true });
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({ behavior: "queue", text: "Hello" })));
});

it("keeps the mobile send button beside the input while the options are collapsed", async () => {
  const { container } = await mountPane("claude", "old-model", true);
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "Hello" } });
  expect(container.querySelector(".sv-composer")!.getAttribute("data-options-expanded")).toBe("false");
  const sends = container.querySelectorAll<HTMLButtonElement>(".sv-send");
  expect(sends).toHaveLength(1);
  expect(sends[0].closest(".sv-quick-actions")).not.toBeNull();
  expect(sends[0].closest(".sv-controls")).toBeNull();
  fireEvent.click(sends[0]);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({ text: "Hello" })));
});

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
  expect(screen.getByRole("status", { name: "1 of 1" }).textContent).toBe("1/1");
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
  await screen.findByRole("status", { name: "" });
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
    const resize = () => act(async () => {
      for (const [observer, record] of [...observers]) {
        if (record.targets.has(scroll)) record.callback([], observer);
      }
      await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
    });
    await resize();
    expect(top).toBe(800);
    // A layout-induced scroll can arrive before ResizeObserver.
    height = 1800;
    fireEvent.scroll(scroll);
    expect(top).toBe(1400);
    expect(container.querySelector(".sv-to-end")).toBeNull();
    viewport = 250;
    await resize();
    expect(top).toBe(1550);
    height = 2100;
    await resize();
    expect(top).toBe(1850);
    // Actual upward scrolling parks history, even as later content grows.
    scroll.scrollTop = 900;
    fireEvent.scroll(scroll);
    expect(container.querySelector(".sv-to-end")).toBeTruthy();
    height = 2500;
    await resize();
    expect(top).toBe(900);
    const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
    fireEvent.change(input, { target: { value: "Continue" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(input.value).toBe(""));
    expect(top).toBe(2250);
    height = 2800;
    await resize();
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

it("shows the irreversible warning for an OpenCode rewind without a file preview", async () => {
  snapshotOverrides = { rewindScopes: ["both"], rows: [{ kind: "user", id: "u-opencode", text: "Edit the fixture" }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_rewind_preview"
    ? Promise.resolve({ canRewind: true }) as Promise<never> : previous(command, args));
  await mountPane("opencode");
  fireEvent.click(await screen.findByRole("button", { name: "Rewind from here" }));
  fireEvent.click(screen.getByRole("button", { name: "Rewind conversation and restore files" }));
  await waitFor(() => expect(screen.getByText("This cannot be undone.")).toBeTruthy());
  expect(screen.queryByText(/files? will change/)).toBeNull();
});

it("opens Codex rewind on the first Enter and can reopen it before confirming", async () => {
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: "u-command", text: "Command target" }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_rewind"
    ? Promise.resolve({ prefillText: "Command target" }) as Promise<never> : previous(command, args));
  const { container } = await mountPane("codex");
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  const scroll = container.querySelector<HTMLDivElement>(".sv-scroll")!;
  Object.defineProperties(scroll, { scrollHeight: { value: 2000 }, clientHeight: { value: 500 } });
  const message = screen.getByText("Command target").closest<HTMLDivElement>(".sv-msg")!;
  // jsdom has no layout or scrollIntoView. Simulate the browser locating the user message.
  message.scrollIntoView = vi.fn(() => { scroll.scrollTop = 25; });
  for (let attempt = 0; attempt < 2; attempt += 1) {
    fireEvent.change(input, { target: { value: "/rewind" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await screen.findByRole("button", { name: "Rewind conversation" });
    await waitFor(() => expect(message.scrollIntoView).toHaveBeenCalledTimes(attempt + 1));
    fireEvent.click(screen.getByRole("button", { name: "Rewind conversation" }));
    expect(scroll.scrollTop).toBe(25);
    expect(input.value).toBe("");
    if (attempt === 0) fireEvent.click(screen.getByRole("button", { name: "Keep as is" }));
  }
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send" || command === "chat_rewind")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Rewind" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_rewind", {
    sessionId: "s", rowId: "u-command", scope: "conversation",
  }));
  await waitFor(() => expect(input.value).toBe("Command target"));
});

it.each([
  ["claude", "conversation", "Look at this", false],
  ["claude", "both", "", true],
  ["codex", "conversation", "", false],
  ["codex", "conversation", "Look at this", true],
  ["opencode", "both", "Look at this", false],
  ["opencode", "both", "", true],
] as const)("restores %s %s images to the composer (text=%s, reference=%s) and resends their bytes", async (kind, scope, text, reference) => {
  const image = { mimeType: "image/png", data: "UE5H" };
  const rowId = `restore-${kind}-${scope}-${reference}`;
  snapshotOverrides = { rewindScopes: [scope], rows: [
    { kind: "user", id: "keep", text: "Keep this earlier message" },
    { kind: "user", id: rowId, text, images: [reference ? { mimeType: image.mimeType, attachmentId: `row:${rowId}:0` } : image] },
    { kind: "assistant", id: "answer", text: "Obsolete answer", streaming: false },
  ] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  let resolveImage!: (value: typeof image) => void;
  const imageReady = new Promise<typeof image>(resolve => { resolveImage = resolve; });
  let rewound = false;
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === "chat_attachment") return (rewound ? Promise.reject(new Error("Attachment removed")) : imageReady) as Promise<never>;
    if (command === "chat_rewind_preview") return Promise.resolve({ canRewind: true, filesChanged: [] }) as Promise<never>;
    if (command === "chat_rewind") {
      rewound = true;
      eventCallback({ type: "replaceRows", rows: [snapshotOverrides.rows![0]] });
      return Promise.resolve({ prefillText: text }) as Promise<never>;
    }
    return previous(command, args);
  });
  const { container } = await mountPane(kind);
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.click(screen.getAllByRole("button", { name: "Rewind from here" })[1]);
  fireEvent.click(screen.getByRole("button", { name: scope === "both" ? "Rewind conversation and restore files" : "Rewind conversation" }));
  await waitFor(() => expect((screen.getByRole("button", { name: "Rewind" }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "Rewind" }));
  if (reference) {
    expect(rewound).toBe(false);
    await act(async () => resolveImage(image));
  }
  await waitFor(() => expect(container.querySelector<HTMLImageElement>(".sv-attach-thumb")?.src).toBe("data:image/png;base64,UE5H"));
  expect(input.value).toBe(text);
  expect(screen.getByText("Keep this earlier message")).toBeTruthy();
  expect(screen.queryByText("Obsolete answer")).toBeNull();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send")).toBe(false);
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")?.[1]).toMatchObject({ text, images: [image] }));
  expect(container.querySelector(".sv-attach-thumb")).toBeNull();
});

it("keeps unsent text and images when restoring an earlier image, and allows removing the restored attachment", async () => {
  const image = { mimeType: "image/png", data: "UE5H" };
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: "restore-draft", text: "Original", images: [image] }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_rewind"
    ? Promise.resolve({ prefillText: "Original" }) as Promise<never> : previous(command, args));
  const { container } = await mountPane("codex");
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "Unsent draft" } });
  fireEvent.paste(input, { clipboardData: { items: [{ kind: "file", type: "image/png", getAsFile: () => new File(["draft"], "draft.png", { type: "image/png" }) }] } });
  await waitFor(() => expect(container.querySelectorAll(".sv-attach-thumb")).toHaveLength(1));
  fireEvent.click(screen.getByRole("button", { name: "Rewind from here" }));
  fireEvent.click(screen.getByRole("button", { name: "Rewind conversation" }));
  fireEvent.click(screen.getByRole("button", { name: "Rewind" }));
  await waitFor(() => expect(container.querySelectorAll(".sv-attach-thumb")).toHaveLength(2));
  expect(input.value).toBe("Unsent draft");
  fireEvent.click(container.querySelector<HTMLButtonElement>(".sv-attach-drop")!);
  expect(container.querySelectorAll(".sv-attach-thumb")).toHaveLength(1);
  expect(container.querySelector<HTMLImageElement>(".sv-attach-thumb")?.alt).toBe("draft.png");
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")?.[1]).toMatchObject({
    text: "Unsent draft", images: [{ mimeType: "image/png", data: "ZHJhZnQ=" }],
  }));
});

it.each(["attachment", "rewind"] as const)("keeps history and the composer unchanged when %s fails during image rewind", async failure => {
  const image = { mimeType: "image/png", data: "UE5H" };
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: `failed-${failure}`, text: "Original with image",
    images: [failure === "attachment" ? { mimeType: image.mimeType, attachmentId: "row:failed-attachment:0" } : image] }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === `chat_${failure}`
    ? Promise.reject(new Error("Image rewind rejected")) : previous(command, args));
  const { container } = await mountPane("codex");
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "Keep this draft" } });
  fireEvent.click(screen.getByRole("button", { name: "Rewind from here" }));
  fireEvent.click(screen.getByRole("button", { name: "Rewind conversation" }));
  fireEvent.click(screen.getByRole("button", { name: "Rewind" }));
  await screen.findByText("Error: Image rewind rejected");
  expect(input.value).toBe("Keep this draft");
  expect(container.querySelector(".sv-attach-thumb")).toBeNull();
  expect(container.querySelector('[data-search-id="failed-' + failure + '"]')).toBeTruthy();
  expect(document.querySelector(".sv-rewind-target")?.textContent).toContain("Original with image");
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send")).toBe(false);
  if (failure === "attachment") expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_rewind")).toBe(false);
});

it("preserves all images when rewind exceeds the attachment limit and waits for excess images to be removed", async () => {
  const images = Array.from({ length: 4 }, () => ({ mimeType: "image/png", data: "UE5H" }));
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: "restore-limit", text: "Original", images }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_rewind"
    ? Promise.resolve({ prefillText: "Original" }) as Promise<never> : previous(command, args));
  const { container } = await mountPane("codex");
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.paste(input, { clipboardData: { items: [{ kind: "file", type: "image/png", getAsFile: () => new File(["draft"], "draft.png", { type: "image/png" }) }] } });
  await waitFor(() => expect(container.querySelectorAll(".sv-attach-thumb")).toHaveLength(1));
  fireEvent.click(screen.getByRole("button", { name: "Rewind from here" }));
  fireEvent.click(screen.getByRole("button", { name: "Rewind conversation" }));
  fireEvent.click(screen.getByRole("button", { name: "Rewind" }));
  await waitFor(() => expect(container.querySelectorAll(".sv-attach-thumb")).toHaveLength(5));
  fireEvent.keyDown(input, { key: "Enter" });
  expect(screen.getByText("A message can include up to 4 images")).toBeTruthy();
  expect(input.value).toBe("Original");
  expect(container.querySelectorAll(".sv-attach-thumb")).toHaveLength(5);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send")).toBe(false);
  fireEvent.click([...container.querySelectorAll<HTMLButtonElement>(".sv-attach-drop")].at(-1)!);
  expect(container.querySelector(".sv-attach-note")).toBeNull();
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")?.[1]).toMatchObject({ text: "Original", images }));
});

it("leaves message images in history and the composer unchanged for file-only rewind", async () => {
  snapshotOverrides = { rewindScopes: ["files"], rows: [{ kind: "user", id: "files-only", text: "Original with image", images: [{ mimeType: "image/png", data: "UE5H" }] }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === "chat_rewind_preview") return Promise.resolve({ canRewind: true, filesChanged: [] }) as Promise<never>;
    if (command === "chat_rewind") return Promise.resolve({ prefillText: null }) as Promise<never>;
    return previous(command, args);
  });
  const { container } = await mountPane();
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "Unsent draft" } });
  fireEvent.click(screen.getByRole("button", { name: "Rewind from here" }));
  fireEvent.click(screen.getByRole("button", { name: "Restore files" }));
  await waitFor(() => expect((screen.getByRole("button", { name: "Rewind" }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "Rewind" }));
  await waitFor(() => expect(container.querySelector(".sv-rewind-confirm")).toBeNull());
  expect(input.value).toBe("Unsent draft");
  expect(container.querySelector(".sv-attach-thumb")).toBeNull();
  expect(container.querySelector('[data-search-id="files-only"] .sv-msg-image')).toBeTruthy();
});

it("receives native rewind scopes when Codex starts after the initial snapshot", async () => {
  snapshotOverrides = { running: false, rewindScopes: [], rows: [{ kind: "user", id: "u-native", text: "An earlier request" }] };
  await mountPane("codex");
  expect((screen.getByRole("button", { name: "Rewind from here" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Edit" }) as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByRole("button", { name: "Rewind from here" }).title).toContain("not running");
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

it("keeps restored Codex history visible when a new process resets the timeline", async () => {
  snapshotOverrides = { startedAt: 100, rowsRevision: 10,
    rows: [{ kind: "user", id: "previous-id", text: "Earlier question" }] };
  const { container } = await mountPane("codex");
  const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(input, { target: { value: "Continue here" } });
  fireEvent.keyDown(input, { key: "Enter" });
  act(() => eventCallback({ type: "reset", epoch: 200, revision: 1, hasMore: true,
    rows: [{ kind: "user", id: "replayed-id", text: "Earlier question" }] }));
  expect(screen.getByText("Earlier question")).toBeTruthy();
  expect(screen.getByText("Continue here")).toBeTruthy();
  expect(container.querySelector('[data-search-id="previous-id"]')).toBeNull();
  expect(container.querySelector('[data-search-id="replayed-id"]')).toBeTruthy();
  expect(container.querySelector(".sv-history-more")).toBeTruthy();
  await act(async () => {});
});

it("confirms the target before removing history and resends the edit with its images", async () => {
  const image = { mimeType: "image/png", data: "UE5H" };
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [
    { kind: "user", id: "keep", text: "Keep this" },
    { kind: "user", id: "edit", text: "Original", images: [image] },
    { kind: "assistant", id: "answer", text: "Old answer", streaming: false },
  ] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_rewind"
    ? Promise.resolve({ prefillText: "Original" }) as Promise<never> : previous(command, args));
  await mountPane("codex");
  fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[1]);
  fireEvent.change(screen.getByRole("textbox", { name: "Edit" }), { target: { value: "Replacement" } });
  fireEvent.click(screen.getByRole("button", { name: "Review and resend" }));
  expect(document.querySelector(".sv-rewind-target")?.textContent).toContain("Original");
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_rewind")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Delete and resend" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_rewind", { sessionId: "s", rowId: "edit", scope: "conversation" }));
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send")).toBe(true));
  const calls = vi.mocked(invoke).mock.calls;
  expect(calls.findIndex(([command]) => command === "chat_rewind")).toBeLessThan(calls.findIndex(([command]) => command === "chat_send"));
  expect(calls.find(([command]) => command === "chat_send")?.[1]).toMatchObject({ text: "Replacement", images: [image] });
  expect(screen.getByText("Keep this")).toBeTruthy();
  expect(screen.queryByText("Old answer")).toBeNull();
});

it("keeps the original conversation and edited text when rewind fails", async () => {
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: "edit", text: "Original" }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_rewind"
    ? Promise.reject(new Error("Rewind rejected")) : previous(command, args));
  await mountPane("codex");
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Edit" }), { target: { value: "Replacement" } });
  fireEvent.click(screen.getByRole("button", { name: "Review and resend" }));
  fireEvent.click(screen.getByRole("button", { name: "Delete and resend" }));
  await screen.findByText("Error: Rewind rejected");
  expect((screen.getByRole("textbox", { name: "Edit" }) as HTMLTextAreaElement).value).toBe("Replacement");
  expect(document.querySelector(".sv-rewind-target")?.textContent).toContain("Original");
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send")).toBe(false);
});

it.each(["claude", "codex", "opencode"] as const)("resends the edited image selection for %s and preserves the composer draft", async kind => {
  const original = { mimeType: "image/png", data: "T0xE" };
  const kept = { mimeType: "image/png", data: "S0VFUA==" };
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: `edit-images-${kind}`, text: "Original", images: [original, { mimeType: "image/png", attachmentId: `row:edit-images-${kind}:1` }] }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_attachment"
    ? Promise.resolve(kept) as Promise<never> : command === "chat_rewind"
    ? Promise.resolve({ prefillText: "Original" }) as Promise<never> : previous(command, args));
  const { container } = await mountPane(kind);
  const composer = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
  fireEvent.change(composer, { target: { value: "Unsent draft" } });
  fireEvent.paste(composer, { clipboardData: { items: [{ kind: "file", type: "image/png", getAsFile: () => new File(["DRAFT"], "draft.png", { type: "image/png" }) }] } });
  await waitFor(() => expect(container.querySelectorAll(".sv-composer .sv-attach-item")).toHaveLength(1));
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  const editor = screen.getByRole("textbox", { name: "Edit" });
  fireEvent.change(editor, { target: { value: " " } });
  fireEvent.click(container.querySelector<HTMLButtonElement>(".sv-message-editor .sv-attach-drop")!);
  fireEvent.paste(editor, { clipboardData: { items: [{ kind: "file", type: "image/png", getAsFile: () => new File(["NEW"], "new.png", { type: "image/png" }) }] } });
  await waitFor(() => expect(container.querySelectorAll(".sv-message-editor .sv-attach-item")).toHaveLength(2));
  fireEvent.click(screen.getByRole("button", { name: "Review and resend" }));
  const preview = container.querySelectorAll(".sv-rewind-target")[1];
  await waitFor(() => expect(Array.from(preview.querySelectorAll("img"), image => image.src)).toEqual([
    "data:image/png;base64,S0VFUA==", "data:image/png;base64,TkVX",
  ]));
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_rewind")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Delete and resend" }));
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")?.[1]).toMatchObject({ text: "", images: [kept, { mimeType: "image/png", data: "TkVX" }] }));
  expect(composer.value).toBe("Unsent draft");
  expect(container.querySelector<HTMLImageElement>(".sv-composer .sv-attach-thumb")?.src).toBe("data:image/png;base64,RFJBRlQ=");
});

it("does not restore removed images when resending only text", async () => {
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: "remove-images", text: "Original",
    images: [{ mimeType: "image/png", data: "T0xE" }] }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_rewind"
    ? Promise.resolve({ prefillText: "Original" }) as Promise<never> : previous(command, args));
  const { container } = await mountPane("codex");
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  fireEvent.click(container.querySelector<HTMLButtonElement>(".sv-message-editor .sv-attach-drop")!);
  fireEvent.click(screen.getByRole("button", { name: "Review and resend" }));
  fireEvent.click(screen.getByRole("button", { name: "Delete and resend" }));
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")?.[1]).toMatchObject({ text: "Original", images: undefined }));
});

it("keeps edited images when a retained history attachment cannot be resolved", async () => {
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: "edit-missing-image", text: "Original",
    images: [{ mimeType: "image/png", attachmentId: "row:edit-missing-image:0" }] }] };
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_attachment"
    ? Promise.reject(new Error("History image unavailable")) : previous(command, args));
  const { container } = await mountPane("codex");
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  fireEvent.paste(screen.getByRole("textbox", { name: "Edit" }), { clipboardData: {
    items: [{ kind: "file", type: "image/png", getAsFile: () => new File(["NEW"], "new.png", { type: "image/png" }) }],
  } });
  await waitFor(() => expect(container.querySelectorAll(".sv-message-editor .sv-attach-item")).toHaveLength(2));
  fireEvent.click(screen.getByRole("button", { name: "Review and resend" }));
  fireEvent.click(screen.getByRole("button", { name: "Delete and resend" }));
  await screen.findByText("Error: History image unavailable");
  expect(container.querySelectorAll(".sv-message-editor .sv-attach-item")).toHaveLength(2);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_rewind" || command === "chat_send")).toBe(false);
  fireEvent.click(container.querySelector<HTMLButtonElement>(".sv-rewind-confirm .sv-deny")!);
  fireEvent.click(container.querySelector<HTMLButtonElement>(".sv-message-editor .sv-attach-drop")!);
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_rewind"
    ? Promise.resolve({ prefillText: "Original" }) as Promise<never> : previous(command, args));
  fireEvent.click(screen.getByRole("button", { name: "Review and resend" }));
  fireEvent.click(screen.getByRole("button", { name: "Delete and resend" }));
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")?.[1]).toMatchObject({ text: "Original", images: [{ mimeType: "image/png", data: "TkVX" }] }));
});


it.each(["claude", "opencode"] as const)("saves %s permission defaults only after the mode is accepted", async kind => {
  await mountPane(kind);
  const select = screen.getByRole("combobox", { name: "Permission mode" });
  fireEvent.change(select, { target: { value: "bypassPermissions" } });
  expect(invoke).not.toHaveBeenCalledWith("agent_set_default_permission", expect.anything());
  await act(async () => reject(new Error("Permission rejected")));
  expect(useTermStore.getState().agentDefaults).toEqual({});
  fireEvent.change(select, { target: { value: "bypassPermissions" } });
  await act(async () => complete());
  expect(invoke).toHaveBeenCalledWith("agent_set_default_permission", { agent: kind, mode: "bypassPermissions" });
  expect(useTermStore.getState().agentDefaults[kind]?.permissionMode).toBe("bypassPermissions");
});

it("keeps confirmed session permissions when saving the global default fails", async () => {
  await mountPane("opencode");
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "agent_set_default_permission"
    ? Promise.reject(new Error("Default save failed")) : previous(command, args));
  const select = screen.getByRole("combobox", { name: "Permission mode" }) as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "bypassPermissions" } });
  await act(async () => complete());
  expect(select.value).toBe("bypassPermissions");
  expect(useTermStore.getState().agentDefaults).toEqual({});
  expect(screen.getByText("Error: Default save failed")).toBeTruthy();
});


it("keeps message actions visible while working and enables them again when idle", async () => {
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: "u", text: "Earlier request" }] };
  await mountPane("codex");
  const rewind = screen.getByRole("button", { name: "Rewind from here" }) as HTMLButtonElement;
  const edit = screen.getByRole("button", { name: "Edit" }) as HTMLButtonElement;
  expect(rewind.disabled).toBe(false);
  act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: "codex", agentState: "working" } } }));
  expect(screen.getByRole("button", { name: "Rewind from here" })).toBe(rewind);
  expect(screen.getByRole("button", { name: "Edit" })).toBe(edit);
  expect(rewind.disabled).toBe(true);
  expect(edit.disabled).toBe(true);
  expect(rewind.title).toContain("no active turn");
  fireEvent.click(rewind);
  fireEvent.click(edit);
  expect(screen.queryByRole("button", { name: "Rewind conversation" })).toBeNull();
  expect(screen.queryByRole("textbox", { name: "Edit" })).toBeNull();
  act(() => useTermStore.setState({ runtimes: {} }));
  expect(rewind.disabled).toBe(false);
  expect(edit.disabled).toBe(false);
  fireEvent.click(edit);
  expect((screen.getByRole("textbox", { name: "Edit" }) as HTMLTextAreaElement).value).toBe("Earlier request");
});

it("keeps message actions visible but disabled while messages are queued", async () => {
  snapshotOverrides = { rewindScopes: ["conversation"], rows: [{ kind: "user", id: "u", text: "Earlier request" }], queue: [{ id: "q", text: "Queued" }] };
  await mountPane("codex");
  expect((screen.getByRole("button", { name: "Rewind from here" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Edit" }) as HTMLButtonElement).disabled).toBe(true);
  act(() => eventCallback({ type: "queued", items: [] }));
  expect((screen.getByRole("button", { name: "Rewind from here" }) as HTMLButtonElement).disabled).toBe(false);
});


it("does not reparse unchanged history while typing or moving the caret, but updates streamed Markdown", async () => {
  snapshotOverrides = { rows: [
    { kind: "user", id: "prompt", text: "Long audit instruction. ".repeat(2_000) },
    { kind: "assistant", id: "reply", text: "Initial answer", streaming: true },
  ] };
  const { container } = await mountPane();
  const lexer = vi.spyOn(md, "lexer");
  try {
    const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
    fireEvent.change(input, { target: { value: "Follow-up question" } });
    input.setSelectionRange(3, 3);
    fireEvent.select(input);
    expect(input.value).toBe("Follow-up question");
    expect(lexer).not.toHaveBeenCalled();
    act(() => eventCallback({ type: "rows", rows: [
      { kind: "assistant", id: "reply", text: "Updated **answer**", streaming: false },
    ] }));
    expect(container.querySelector(".sv-msg-body strong")?.textContent).toBe("answer");
    expect(lexer).toHaveBeenCalledTimes(1);
    expect(lexer).toHaveBeenCalledWith("Updated **answer**");
  } finally { lexer.mockRestore(); }
});

it("retains scroll observers during input and observes newly appended rows", async () => {
  const instances: { observe: ReturnType<typeof vi.fn>; unobserve: ReturnType<typeof vi.fn>; disconnect: ReturnType<typeof vi.fn> }[] = [];
  vi.stubGlobal("ResizeObserver", class {
    observe = vi.fn(); unobserve = vi.fn(); disconnect = vi.fn();
    constructor() { instances.push(this); }
  });
  try {
    const { container, unmount } = await mountPane();
    const scroll = container.querySelector(".sv-scroll")!;
    const observer = instances.find(instance => instance.observe.mock.calls.some(([element]) => element === scroll))!;
    expect(observer).toBeTruthy();
    const input = container.querySelector<HTMLTextAreaElement>(".sv-box textarea")!;
    fireEvent.change(input, { target: { value: "Typing must not rebuild observers" } });
    expect(observer.disconnect).not.toHaveBeenCalled();
    act(() => eventCallback({ type: "rows", rows: [{ kind: "user", id: "appended", text: "New row" }] }));
    await waitFor(() => expect(observer.observe).toHaveBeenCalledWith(container.querySelector('[data-search-id="appended"]')));
    act(() => eventCallback({ type: "replaceRows", rows: [] }));
    await waitFor(() => expect(observer.unobserve).toHaveBeenCalled());
    unmount();
    expect(observer.disconnect).toHaveBeenCalledTimes(1);
  } finally { cleanup(); vi.unstubAllGlobals(); }
});


it("restores Codex authorization from the backend and retains the conversation after sign-in", async () => {
  snapshotOverrides = { auth: { status: "required" }, rows: [{ kind: "user", id: "original", text: "Original conversation" }] };
  const view = await mountPane("codex");
  const begin = screen.getByRole("button", { name: "Sign in again" });
  fireEvent.click(begin);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_auth_start", { sessionId: "s" }));
  const pending = { status: "pending" as const, userCode: "ABCD-1234", verificationUrl: "https://auth.openai.com/codex/device" };
  act(() => eventCallback({ type: "extras", extras: { auth: pending } }));
  expect(screen.getByText("ABCD-1234")).toBeTruthy();
  expect(screen.getByRole("link", { name: "Open authorization page" }).getAttribute("href")).toBe(pending.verificationUrl);
  view.unmount();
  snapshotOverrides.auth = pending;
  await mountPane("codex");
  expect(screen.getByText("ABCD-1234")).toBeTruthy();
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "success" } } }));
  expect(screen.queryByText("ABCD-1234")).toBeNull();
  expect(screen.getByText("Original conversation")).toBeTruthy();
  expect(screen.queryByRole("region", { name: "Codex account" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Sign in again" })).toBeNull();
  expect(screen.getByRole("button", { name: "Codex account" }).getAttribute("aria-expanded")).toBe("false");
});

it("keeps authentication commands out of read-only conversation views", async () => {
  snapshotOverrides = { auth: { status: "required" } };
  render(<ChatPane session={{ id: "s", projectId: "p", name: "Codex", kind: "codex", engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
    area={{}} hidden={false} focused multi={false} readOnly onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />);
  await act(async () => {});
  expect(screen.queryByRole("button", { name: "Sign in again" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Codex account" })).toBeNull();
});


it("offers manual sign-out without an auth error and requires confirmation before invoking it", async () => {
  snapshotOverrides = { auth: undefined, rows: [{ kind: "user", id: "history", text: "Keep this conversation" }] };
  await mountPane("codex");
  expect(screen.queryByRole("button", { name: "Sign in again" })).toBeNull();
  expect(screen.queryByRole("region", { name: "Codex account" })).toBeNull();
  const account = screen.getByRole("button", { name: "Codex account" });
  fireEvent.click(account);
  expect(screen.getByRole("button", { name: "Sign in again" })).toBeTruthy();
  fireEvent.keyDown(account, { key: "Escape" });
  expect(screen.queryByRole("button", { name: "Sign in again" })).toBeNull();
  fireEvent.click(account);
  fireEvent.mouseDown(document.body);
  expect(screen.queryByRole("button", { name: "Sign out" })).toBeNull();
  fireEvent.click(account);
  fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
  expect(screen.getByText(/This clears the shared account credentials/)).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "chat_auth_logout")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("button", { name: "Confirm sign-out" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
  const confirm = screen.getByRole("button", { name: "Confirm sign-out" });
  fireEvent.click(confirm);
  fireEvent.click(confirm);
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "chat_auth_logout")).toHaveLength(1));
  expect(invoke).toHaveBeenCalledWith("chat_auth_logout", { sessionId: "s" });
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "signedOut" } } }));
  expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Sign out" })).toBeNull();
  expect(screen.getByText("Keep this conversation")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_auth_start", { sessionId: "s" }));
});

it("keeps the stop shortcut available after closing a settings menu with the keyboard", async () => {
  const { container } = await mountPane("codex");
  act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: "codex", agentState: "working" } } }));
  const account = screen.getByRole("button", { name: "Codex account" });
  fireEvent.click(account);
  expect(screen.getByRole("button", { name: "Sign out" })).toBeTruthy();
  // The open popover takes the first Escape; no More toggle exists while every chip fits inline.
  fireEvent.keyDown(account, { key: "Escape" });
  expect(screen.queryByRole("button", { name: "Sign out" })).toBeNull();
  expect(screen.queryByRole("button", { name: "More" })).toBeNull();
  expect(invoke).not.toHaveBeenCalledWith("chat_interrupt", { sessionId: "s" });
  fireEvent.keyDown(container.querySelector(".sv-box textarea")!, { key: "Escape" });
  expect(invoke).toHaveBeenCalledWith("chat_interrupt", { sessionId: "s" });
});

it("disables account changes during work and shows a failed sign-out without claiming success", async () => {
  snapshotOverrides = { turnStartedAt: Date.now() };
  await mountPane("codex");
  fireEvent.click(screen.getByRole("button", { name: "Codex account" }));
  expect((screen.getByRole("button", { name: "Sign out" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Sign in again" }) as HTMLButtonElement).disabled).toBe(true);
  act(() => useTermStore.setState({ runtimes: { s: { status: "running", agent: "codex", agentState: "working" } } }));
  fireEvent.keyDown(screen.getByRole("button", { name: "Codex account" }), { key: "Escape" });
  expect(screen.queryByRole("button", { name: "Sign out" })).toBeNull();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_interrupt")).toBe(false);
  act(() => eventCallback({ type: "turnCompleted" }));
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "signingOut" } } }));
  expect((screen.getByRole("button", { name: "Sign out" }) as HTMLButtonElement).disabled).toBe(true);
  expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "logoutFailed" } } }));
  expect(screen.getByText("Could not confirm sign-out. Please try again.")).toBeTruthy();
  expect(screen.queryByText("Signed out of Codex. Sign in to continue this conversation.")).toBeNull();
  expect((screen.getByRole("button", { name: "Sign out" }) as HTMLButtonElement).disabled).toBe(false);
});

it("folds canceled authorization back into the account menu, including after a reload", async () => {
  snapshotOverrides = { auth: { status: "pending", userCode: "ABCD-1234", verificationUrl: "https://auth.openai.com/codex/device" } };
  const view = await mountPane("codex");
  expect(screen.getByText("ABCD-1234")).toBeTruthy();
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "canceled" } } }));
  expect(screen.queryByRole("region", { name: "Codex account" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Sign in again" })).toBeNull();
  view.unmount();
  snapshotOverrides.auth = { status: "canceled" };
  await mountPane("codex");
  expect(screen.queryByRole("region", { name: "Codex account" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Codex account" }));
  expect(screen.getByRole("button", { name: "Sign in again" })).toBeTruthy();
  expect(screen.queryByText("Sign-in canceled. You can try again at any time.")).toBeNull();
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "starting" } } }));
  // The chip stays in the row but disabled while the inline panel owns the sign-in; its menu is closed.
  expect(screen.getByRole("button", { name: "Codex account" }).hasAttribute("disabled")).toBe(true);
  expect(screen.getAllByRole("region", { name: "Codex account" })).toHaveLength(1);
});


it("offers Claude account actions in the folded menu and keeps sign-out explicit", async () => {
  snapshotOverrides = { rows: [{ kind: "user", id: "history", text: "Claude history" }] };
  await mountPane("claude");
  expect(screen.queryByRole("button", { name: "Sign in again" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Claude account" }));
  expect(screen.getByText(/Sign-in updates the Claude account/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
  expect(invoke).not.toHaveBeenCalledWith("chat_auth_logout", { sessionId: "s" });
  fireEvent.click(screen.getByRole("button", { name: "Confirm sign-out" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_auth_logout", { sessionId: "s" }));
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "signedOut" } } }));
  expect(screen.getByText("Signed out of Claude. Sign in to continue this conversation.")).toBeTruthy();
  expect(screen.getByText("Claude history")).toBeTruthy();
});

it("restores Claude authorization after reload and submits the full code only to the account API", async () => {
  snapshotOverrides = { auth: { status: "pending", verificationUrl: "https://claude.ai/oauth/authorize?state=test" } };
  await mountPane("claude");
  expect(screen.getByRole("link", { name: "Open authorization page" }).getAttribute("href")).toBe("https://claude.ai/oauth/authorize?state=test");
  const input = screen.getByLabelText("Authorization code") as HTMLInputElement;
  expect(input.type).toBe("password");
  fireEvent.change(input, { target: { value: "test-code#test" } });
  fireEvent.click(screen.getByRole("button", { name: "Submit code" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_auth_submit", { sessionId: "s", code: "test-code#test" }));
  expect(input.value).toBe("");
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_send")).toBe(false);
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "submitting" } } }));
  expect((screen.getByRole("button", { name: "Cancel" }) as HTMLButtonElement).disabled).toBe(true);
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "success" } } }));
  expect(screen.queryByRole("region", { name: "Claude account" })).toBeNull();
  expect(screen.getByRole("button", { name: "Claude account" }).getAttribute("aria-expanded")).toBe("false");
});

it("shows Claude-specific login failures and folds a confirmed cancellation", async () => {
  snapshotOverrides = { auth: { status: "failed" } };
  await mountPane("claude");
  expect(screen.getByText(/your Claude CLI supports account authorization/)).toBeTruthy();
  expect(screen.queryByText(/Device code authentication/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Sign in again" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_auth_start", { sessionId: "s" }));
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "starting" } } }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_auth_cancel", { sessionId: "s" }));
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "canceling" } } }));
  expect(screen.getByText("Canceling sign-in…")).toBeTruthy();
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "canceled" } } }));
  expect(screen.queryByRole("region", { name: "Claude account" })).toBeNull();
  expect(screen.getByRole("button", { name: "Claude account" })).toBeTruthy();
});

it("keeps hidden chips in More and follows the inline preference at runtime", async () => {
  // Inline preferences control placement; hidden available chips remain reachable through More.
  useTermStore.setState({ composerInlineChips: [...DEFAULT_COMPOSER_INLINE_CHIPS] });
  const { container } = await mountPane("codex");
  expect(screen.getByRole("combobox", { name: "Model" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "Thinking effort" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "Permission mode" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Codex account" })).toBeNull();
  expect(screen.getByRole("button", { name: "More" })).toBeTruthy();
  expect(container.querySelector(".sv-controls-secondary")?.getAttribute("aria-hidden")).toBe("true");
  const inline = [...container.querySelectorAll(".sv-controls-primary .sv-chip-slot")].map((el) => (el as HTMLElement).dataset.chip);
  expect(inline).toEqual(["model", "effort", "permission"]);
  fireEvent.click(screen.getByRole("button", { name: "More" }));
  expect(screen.getByRole("button", { name: "Codex account" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "More" }));
  // Changing the preference reorders and reveals chips without a remount.
  act(() => useTermStore.setState({ composerInlineChips: ["account", "permission"] }));
  expect(screen.getByRole("button", { name: "Codex account" })).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "Model" })).toBeNull();
  expect([...container.querySelectorAll(".sv-controls-primary .sv-chip-slot")].map((el) => (el as HTMLElement).dataset.chip)).toEqual(["account", "permission"]);
  expect(screen.getByRole("button", { name: "More" })).toBeTruthy();
});

it("keeps every offered chip reachable when all inline chips are off", async () => {
  useTermStore.setState({ composerInlineChips: [] });
  snapshotOverrides = { running: false };
  const { container } = await mountPane("codex", null);
  expect(container.querySelectorAll(".sv-controls-primary .sv-chip-slot")).toHaveLength(0);
  expect(container.querySelector(".sv-controls-secondary")?.hasAttribute("inert")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "More" }));
  expect(await screen.findByRole("combobox", { name: "Model" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Codex account" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "MCP" }).hasAttribute("disabled")).toBe(true);
  expect(vi.mocked(invoke).mock.calls.some(call => call[0] === "chat_mcp_status")).toBe(false);
});

it("keeps MCP and Tasks chips in the row while no process runs, disabled and silent, and wakes them in place", async () => {
  snapshotOverrides = { running: false };
  const { container } = await mountPane();
  const mcp = screen.getByRole("button", { name: "MCP" });
  const tasks = screen.getByRole("button", { name: "Tasks" });
  for (const chip of [mcp, tasks]) {
    expect(chip.hasAttribute("disabled")).toBe(true);
    expect(chip.getAttribute("aria-disabled")).toBe("true");
    expect(chip.getAttribute("title")).toBe("The agent process is not running. Send a message to start it.");
    fireEvent.click(chip);
    expect(chip.getAttribute("aria-expanded")).toBe("false");
  }
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_mcp_status")).toBe(false);
  // The account chip is offered too; it is only disabled while a sign-in is unresolved.
  expect(screen.getByRole("button", { name: "Claude account" }).hasAttribute("disabled")).toBe(false);
  expect([...container.querySelectorAll(".sv-controls-primary .sv-chip-slot")].map((el) => (el as HTMLElement).dataset.chip))
    .toEqual(["model", "effort", "permission", "mcp", "tasks", "account"]);
  act(() => eventCallback({ type: "process", pid: 123, startedAt: 100 }));
  expect(screen.getByRole("button", { name: "MCP" })).toBe(mcp);
  expect(mcp.hasAttribute("disabled")).toBe(false);
  expect(tasks.hasAttribute("disabled")).toBe(false);
  expect(tasks.getAttribute("title")).toBe("Background tasks");
  fireEvent.click(tasks);
  expect(screen.getByText("No background tasks")).toBeTruthy();
  fireEvent.click(mcp);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("chat_mcp_status", { sessionId: "s" }));
  act(() => eventCallback({ type: "exited", code: 0, stderr: "", released: true }));
  expect(mcp.hasAttribute("disabled")).toBe(true);
  expect(mcp.getAttribute("aria-expanded")).toBe("false");
});

it("brings the Tasks chip back at the end of the row when it is switched off and on with no tasks", async () => {
  snapshotOverrides = { running: false };
  useTermStore.setState({ composerInlineChips: ["model", "tasks", "effort"] });
  const { container } = await mountPane();
  const inline = () => [...container.querySelectorAll(".sv-controls-primary .sv-chip-slot")].map((el) => (el as HTMLElement).dataset.chip);
  expect(inline()).toEqual(["model", "tasks", "effort"]);
  act(() => useTermStore.setState({ composerInlineChips: ["model", "effort"] }));
  expect(screen.queryByRole("button", { name: "Tasks" })).toBeNull();
  expect(inline()).toEqual(["model", "effort"]);
  act(() => useTermStore.setState({ composerInlineChips: ["model", "effort", "tasks"] }));
  expect(inline()).toEqual(["model", "effort", "tasks"]);
  expect(screen.getByRole("button", { name: "Tasks" }).hasAttribute("disabled")).toBe(true);
});

it("keeps the account chip in the row but disabled while a sign-in is unresolved", async () => {
  snapshotOverrides = { auth: { status: "required" } };
  await mountPane("codex");
  const account = screen.getByRole("button", { name: "Codex account" });
  expect(account.hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("region", { name: "Codex account" })).toBeTruthy();
  act(() => eventCallback({ type: "extras", extras: { auth: { status: "success" } } }));
  expect(screen.getByRole("button", { name: "Codex account" }).hasAttribute("disabled")).toBe(false);
  expect(screen.queryByRole("region", { name: "Codex account" })).toBeNull();
});
