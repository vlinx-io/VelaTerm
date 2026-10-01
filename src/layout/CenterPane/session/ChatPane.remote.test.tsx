//! Remote subscription behaviour of the conversation pane: over a WebSocket a pane follows its conversation
//! only while it can be seen (or waits for a receipt), catches up incrementally when shown, and reloads in
//! full at a resync barrier. The desktop keeps every mounted pane subscribed.

import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ChatEvent, ChatSnapshot } from "../../../ipc/chat";

const mode = vi.hoisted(() => ({ desktop: false }));
vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(), onTransportReconnect: vi.fn(),
  get isTauri() { return mode.desktop; },
}));
vi.mock("./engineSwitch", () => ({ useEngineSwitch: () => ({ switchTo: vi.fn(), confirm: null }) }));
// The permission badge follows the same channel only while the pane is visible; counted here is the pane's own subscription.
vi.mock("../../../hooks/useSessionPermissionState", () => ({ useSessionPermissionState: () => undefined }));

import { invoke, listen, onTransportReconnect } from "../../../ipc/transport";
import { useTermStore } from "../../../store/termStore";
import { ChatPane } from "./ChatPane";
import { useOutbox } from "./outbox";
import { cachedChat, clearChatCache } from "./chatCache";

const chatListeners = new Set<(event: ChatEvent) => void>();
let unlistened = 0;
let snapshots: Array<Partial<ChatSnapshot>>;
let reconnect: () => void;

const emit = (event: ChatEvent) => act(() => chatListeners.forEach(listener => listener(event)));
const snapshotWindows = () => vi.mocked(invoke).mock.calls
  .filter(([command]) => command === "chat_snapshot")
  .map(([, args]) => (args as { window?: Record<string, unknown> }).window);
const chatSubscriptions = () => vi.mocked(listen).mock.calls.filter(([name]) => name === "chat://event/s").length;

beforeEach(() => {
  mode.desktop = false;
  clearChatCache();
  useOutbox.setState({ sessions: {} });
  useTermStore.setState({ chatModel: "m", chatModelByKind: { claude: "m" }, chatEffortByModel: {}, runtimes: {}, agentDefaults: {} });
  chatListeners.clear();
  unlistened = 0;
  snapshots = [];
  vi.mocked(listen).mockReset();
  vi.mocked(listen).mockImplementation((name, callback) => {
    const listener = callback as (event: ChatEvent) => void;
    if (name === "chat://event/s") chatListeners.add(listener);
    return Promise.resolve(() => { if (chatListeners.delete(listener)) unlistened++; });
  });
  const reconnectListeners = new Set<() => void>();
  reconnect = () => reconnectListeners.forEach(callback => callback());
  vi.mocked(onTransportReconnect).mockImplementation(callback => {
    reconnectListeners.add(callback); return () => { reconnectListeners.delete(callback); };
  });
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "chat_snapshot") {
      const next = snapshots.shift() ?? {};
      return Promise.resolve({
        submissionReceipts: true, running: true, startedAt: 7, rowsRevision: 3, model: "m", mode: "default",
        rows: [], queue: [], permissions: [], commands: [], configKeys: [], ...next,
      }) as Promise<never>;
    }
    if (command === "agent_permission_catalog") return Promise.resolve({ modes: ["default"], selected: "default" }) as Promise<never>;
    if (command === "chat_models") return Promise.resolve([]) as Promise<never>;
    return Promise.resolve(undefined) as Promise<never>;
  });
});
afterEach(cleanup);

function pane(hidden: boolean) {
  return <ChatPane session={{ id: "s", projectId: "p", name: "Claude", kind: "claude", engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
    area={{}} hidden={hidden} focused multi={false} onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />;
}

it("a hidden remote pane neither subscribes nor reads a snapshot until it is shown", async () => {
  const view = render(pane(true));
  await act(async () => {});
  expect(chatSubscriptions()).toBe(0);
  expect(snapshotWindows()).toEqual([]);
  act(() => reconnect());
  await act(async () => {});
  expect(snapshotWindows(), "a reconnect does not wake a hidden pane").toEqual([]);

  view.rerender(pane(false));
  await waitFor(() => expect(snapshotWindows()).toHaveLength(1));
  expect(chatSubscriptions()).toBe(1);
  expect(snapshotWindows()[0]).toEqual({ compactTasks: true });
});

it("hiding unsubscribes and showing again catches up with a tagged delta snapshot", async () => {
  snapshots = [{ metaTag: "meta-1", historyTag: "1:00000000000000ab", commands: [{ name: "review", description: "Review" }], rows: [{ kind: "user", id: "u1", text: "hello" }] }];
  const view = render(pane(false));
  await waitFor(() => expect(cachedChat("s")?.metaTag).toBe("meta-1"));
  view.rerender(pane(true));
  await act(async () => {});
  expect(unlistened).toBe(1);
  expect(chatListeners.size).toBe(0);

  snapshots = [{ metaUnchanged: true, metaTag: "meta-1", pageKind: "delta", rowsRevision: 9, commands: [], configKeys: [] }];
  view.rerender(pane(false));
  await waitFor(() => expect(snapshotWindows()).toHaveLength(2));
  expect(snapshotWindows()[1]).toEqual({ since: 3, epoch: 7, from: "u1", metaTag: "meta-1", historyTag: "1:00000000000000ab", compactTasks: true });
  await waitFor(() => expect(cachedChat("s")?.rowsRevision).toBe(9));
  expect(cachedChat("s")?.commands).toEqual([{ name: "review", description: "Review" }]);
  expect(cachedChat("s")?.rows.map(row => row.id), "the delta keeps the rows the pane held").toEqual(["u1"]);
  expect(chatSubscriptions()).toBe(2);
});

it("a message waiting for its receipt keeps a hidden pane subscribed", async () => {
  const view = render(pane(false));
  await waitFor(() => expect(snapshotWindows()).toHaveLength(1));
  act(() => useOutbox.setState({ sessions: { s: [{ id: "msg-1", text: "hi", images: [], behavior: "queue", status: "sending" }] } }));
  view.rerender(pane(true));
  await act(async () => {});
  expect(unlistened).toBe(0);
  act(() => useOutbox.setState({ sessions: { s: [] } }));
  await act(async () => {});
  expect(unlistened, "once confirmed, the hidden pane lets go").toBe(1);
});

it("a resync barrier reloads the conversation in full", async () => {
  render(pane(false));
  await waitFor(() => expect(snapshotWindows()).toHaveLength(1));
  await waitFor(() => expect(cachedChat("s")).toBeDefined());
  emit({ type: "resync" });
  await waitFor(() => expect(snapshotWindows()).toHaveLength(2));
  expect(snapshotWindows()[1], "no since: a full snapshot").toEqual({ compactTasks: true });
});

it("keeps hidden panes subscribed on the desktop, as before", async () => {
  mode.desktop = true;
  const view = render(pane(true));
  await waitFor(() => expect(snapshotWindows()).toHaveLength(1));
  expect(chatSubscriptions()).toBe(1);
  view.rerender(pane(false));
  view.rerender(pane(true));
  await act(async () => {});
  expect(unlistened).toBe(0);
  expect(snapshotWindows()).toHaveLength(1);
});
