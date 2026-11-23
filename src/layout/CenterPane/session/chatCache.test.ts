import { expect, it } from "vitest";
import { registerRowPositions, type ChatRow, type ChatSnapshot } from "../../../ipc/chat";
import { cacheChat, cachedChat, clearChatCache, mergeRows, reconcileChat, limitHistoryWindow } from "./chatCache";

const row = (id: string, position: number): ChatRow => {
  const value: ChatRow = { kind: "user", id, text: id };
  registerRowPositions([value], { [id]: position });
  return value;
};

it("keeps authoritative order and ignores unseen updates before the loaded history window", () => {
  const loaded = [row("middle", 90), row("last", 100)];
  const result = mergeRows(loaded, [row("new", 101), row("old", 2), row("last", 100)], true);
  expect(result.map(item => item.id)).toEqual(["middle", "last", "new"]);
  expect(mergeRows([row("older", 89)], result).map(item => item.id)).toEqual(["older", "middle", "last", "new"]);
  // A pending local submission whose replayed timeline shrank below the old window floor stays at the tail.
  const preserved = mergeRows(loaded, [row("msg-pending", 12), row("old", 2)], true, new Set(["msg-pending"]));
  expect(preserved.map(item => item.id)).toEqual(["middle", "last", "msg-pending"]);
});

it("retains recent events over a delayed snapshot without creating gaps in history", () => {
  const baseline = { pageKind: "recent", hasMore: true, startedAt: 100, rowsRevision: 10, queueRevision: 10, rows: [row("middle", 90)], queue: [] } as unknown as ChatSnapshot;
  const result = reconcileChat(baseline, [], [
    { type: "rows", epoch: 100, revision: 11, rows: [row("old", 2), row("new", 91)] },
    { type: "queued", epoch: 100, revision: 12, items: [{ id: "q", text: "queued" }] },
  ]);
  expect(result.rows.map(item => item.id)).toEqual(["middle", "new"]);
  expect(result.queue[0].id).toBe("q");
  expect(result.hasMore).toBe(true);
});

it("merges reconnect deltas and discards events from older processes", () => {
  const baseline = { pageKind: "delta", startedAt: 200, rowsRevision: 20, rows: [row("new", 91)], queue: [] } as unknown as ChatSnapshot;
  const result = reconcileChat(baseline, [row("middle", 90)], [{ type: "replaceRows", epoch: 100, revision: 99, rows: [] }]);
  expect(result.rows.map(item => item.id)).toEqual(["middle", "new"]);
});

it("restores a restarted process atomically while a previous snapshot is in flight", () => {
  const baseline = { startedAt: 100, rowsRevision: 50, rows: [row("old-process", 0)], queue: [] } as unknown as ChatSnapshot;
  const restored = row("restored-history", 90);
  const result = reconcileChat(baseline, [], [
    { type: "reset", epoch: 200, revision: 1, rows: [restored], hasMore: true },
    { type: "rows", epoch: 200, revision: 2, rows: [row("new-message", 91)] },
  ]);
  expect(result.rows.map(item => item.id)).toEqual(["restored-history", "new-message"]);
  expect(result.hasMore).toBe(true);
  expect(result.startedAt).toBe(200);
  expect(result.rowsRevision).toBe(2);
  const newer = { ...result, rowsRevision: 3 };
  expect(reconcileChat(newer, [], [
    { type: "reset", epoch: 200, revision: 1, rows: [restored] },
  ]).rows).toEqual(result.rows);
});


it("keeps a newer authorization event over a late snapshot and clears it on process reset", () => {
  const snapshot = { running: true, rows: [], queue: [], permissions: [], commands: [], configKeys: [], startedAt: 100, auth: { status: "required" as const } };
  const pending = { status: "pending" as const, verificationUrl: "https://auth.openai.com/codex/device", userCode: "ABCD-1234" };
  const updated = reconcileChat(snapshot, [], [{ type: "extras", extras: { auth: pending } }]);
  expect(updated.auth).toEqual(pending);
  expect(reconcileChat(updated, [], [{ type: "reset", epoch: 200, rows: [], revision: 0 }]).auth).toBeUndefined();
});

it("keeps the turn clock from events newer than a late snapshot", () => {
  const idle = { running: true, rows: [], queue: [], permissions: [], commands: [], configKeys: [], startedAt: 100 };
  expect(reconcileChat(idle, [], [{ type: "turnStarted", startedAt: 500 }]).turnStartedAt).toBe(500);
  const busy = { ...idle, turnStartedAt: 500 };
  expect(reconcileChat(busy, [], [{ type: "turnCompleted" }]).turnStartedAt).toBeUndefined();
  expect(reconcileChat(busy, [], [{ type: "turnCompleted" }, { type: "turnStarted", startedAt: 900 }]).turnStartedAt).toBe(900);
  const exited = reconcileChat(busy, [], [{ type: "exited", code: 0, stderr: "" }]);
  expect(exited.running).toBe(false);
  expect(exited.turnStartedAt).toBeUndefined();
  // The first message starts the process after a snapshot that found none.
  const started = reconcileChat({ ...idle, running: false }, [], [
    { type: "process", pid: 42, startedAt: 100 },
    { type: "turnStarted", startedAt: 600 },
  ]);
  expect(started).toMatchObject({ running: true, pid: 42, turnStartedAt: 600 });
});

it("evicts by owned bytes and keeps smaller snapshots available", () => {
  clearChatCache();
  const snapshot = (text: string) => ({ rows: [{ kind: "user", id: text.length.toString(), text }], queue: [] }) as unknown as ChatSnapshot;
  cacheChat("small", snapshot("small"));
  cacheChat("oversized", snapshot("x".repeat(33 * 1024 * 1024)));
  expect(cachedChat("oversized")).toBeUndefined();
  expect(cachedChat("small")?.rows[0].id).toBe("5");
  clearChatCache();
});

it("limits cached history rows and promotes a reused session before LRU eviction", () => {
  clearChatCache();
  const snapshot = (rows: number) => ({ rows: Array.from({ length: rows }, (_, index) => row(String(index), index)), queue: [] }) as unknown as ChatSnapshot;
  for (let index = 0; index < 8; index++) cacheChat(String(index), snapshot(1));
  cachedChat("0");
  cacheChat("next", snapshot(1));
  expect(cachedChat("0")).toBeDefined();
  expect(cachedChat("1")).toBeUndefined();
  cacheChat("too-many", snapshot(8 * 240 + 1));
  expect(cachedChat("too-many")).toBeUndefined();
  clearChatCache();
});

it("recovers evicted rows in both directions while keeping at most 240 complete summaries", () => {
  const original = Array.from({ length: 300 }, (_, index) => row(`message-${index}`, index));
  const recent = limitHistoryWindow(original, "newer");
  expect(recent.rows).toEqual(original.slice(60));
  expect(recent.droppedBefore).toBe(true);
  const older = limitHistoryWindow(mergeRows(original.slice(0, 60), recent.rows), "older");
  expect(older.rows).toEqual(original.slice(0, 240));
  expect(older.droppedAfter).toBe(true);
  const forward = limitHistoryWindow(mergeRows(older.rows, original.slice(240)), "newer");
  expect(forward.rows).toEqual(recent.rows);
  expect(original[0]).toEqual({ kind: "user", id: "message-0", text: "message-0" });
});

it("limits mounted owned bytes without changing a retained body or dropping control state", () => {
  const body = "中".repeat(30_000);
  const rows: ChatRow[] = Array.from({ length: 200 }, (_, index) => ({ kind: "user", id: String(index), text: body }));
  const window = limitHistoryWindow(rows, "older");
  expect(window.rows.length).toBeLessThan(200);
  expect(window.rows.every(value => value.kind === "user" && value.text === body)).toBe(true);
  expect(window.droppedAfter).toBe(true);
  const snapshot = { protocolVersion: 2, rows: Array.from({ length: 300 }, (_, index) => row(String(index), index)),
    running: true, queue: [{ id: "queued", text: "pending" }], permissions: [{ id: "permission" }], commands: [], configKeys: [] } as ChatSnapshot;
  const result = reconcileChat(snapshot, [], []);
  expect(result.rows).toHaveLength(240);
  expect(result.hasMore).toBe(true);
  expect(result.queue).toEqual(snapshot.queue);
  expect(result.permissions).toEqual(snapshot.permissions);
});
