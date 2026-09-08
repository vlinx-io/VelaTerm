import type { ChatEvent, ChatRow, ChatSnapshot } from "../../../ipc/chat";

import { chatRowPosition } from "../../../ipc/chat";

const cache = new Map<string, ChatSnapshot>();
export function cachedChat(id: string) { return cache.get(id); }
export function cacheChat(id: string, snapshot: ChatSnapshot) {
  cache.delete(id);
  cache.set(id, snapshot);
  while (cache.size > 8) cache.delete(cache.keys().next().value!);
}
export function clearChatCache() { cache.clear(); }

export function mergeRows(base: ChatRow[], incoming: ChatRow[], retainWindow = false): ChatRow[] {
  const result = [...base];
  const indices = new Map(result.map((row, index) => [row.id, index]));
  const minimum = Math.min(...base.map(row => chatRowPosition(row) ?? Infinity));
  for (const row of incoming) {
    if (retainWindow && Number.isFinite(minimum) && !indices.has(row.id) && (chatRowPosition(row) ?? Infinity) < minimum) continue;
    const index = indices.get(row.id);
    if (index === undefined) { indices.set(row.id, result.length); result.push(row); }
    else result[index] = row;
  }
  return result.sort((a, b) => {
    const left = chatRowPosition(a), right = chatRowPosition(b);
    return left !== undefined && right !== undefined ? left - right : 0;
  });
}

/** Events are already visible. Replay their newer state over the late baseline in one commit. */
export function reconcileChat(snapshot: ChatSnapshot, current: ChatRow[], events: ChatEvent[]): ChatSnapshot {
  let result = { ...snapshot, rows: snapshot.pageKind === "delta" ? mergeRows(current, snapshot.rows) : snapshot.rows };
  for (const event of events) {
    if (event.type !== "rows" && event.type !== "replaceRows" && event.type !== "queued" && event.type !== "reset") continue;
    const epoch = event.epoch ?? result.startedAt;
    if (epoch !== undefined && result.startedAt !== undefined && epoch < result.startedAt) continue;
    if (epoch !== undefined && epoch !== result.startedAt) {
      result = { ...result, startedAt: epoch, rows: [], queue: [], rowsRevision: 0, queueRevision: 0, hasMore: false };
    }
    if (event.type === "reset") continue;
    if (event.type === "queued") {
      if (event.revision === undefined || event.revision > (result.queueRevision ?? -1)) {
        result = { ...result, queue: event.items, queueRevision: event.revision };
      }
    } else if (event.revision === undefined || event.revision > (result.rowsRevision ?? -1)) {
      result = { ...result, rows: event.type === "rows" ? mergeRows(result.rows, event.rows, result.hasMore ?? false) : event.rows, rowsRevision: event.revision, hasMore: event.type === "replaceRows" ? (event.hasMore ?? false) : result.hasMore };
    }
  }
  return result;
}

/** Numbers only; diagnostics never retain message text or attachments. */
export const chatSyncMetrics: { requestMs: number; commitMs: number; rows: number; mode: string }[] = [];
