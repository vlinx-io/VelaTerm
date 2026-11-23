import type { ChatEvent, ChatRow, ChatSnapshot } from "../../../ipc/chat";

import { chatRowPosition } from "../../../ipc/chat";
import { t } from "../../../i18n";

const cache = new Map<string, ChatSnapshot>();
const cacheSizes = new Map<string, number>();
const byteSizes = new WeakMap<object, number>();
const CACHE_BYTE_LIMIT = 32 * 1024 * 1024;
const CACHE_ROW_LIMIT = 8 * 240;
let cacheBytes = 0;
let cacheRows = 0;
export const ACTIVE_HISTORY_ROWS = 240;
const ACTIVE_HISTORY_BYTES = 8 * 1024 * 1024;

/** Estimate owned bytes without allocating a second serialized conversation or a large UTF-8 buffer. */
function ownedBytes(value: unknown): number {
  if (typeof value === "string") {
    if (value.length > CACHE_BYTE_LIMIT) return CACHE_BYTE_LIMIT + 1;
    let bytes = 2;
    for (let i = 0; i < value.length; i++) {
      const code = value.charCodeAt(i);
      if (code < 128) bytes += code < 32 || code === 34 || code === 92 ? 6 : 1;
      else if (code < 2048) bytes += 2;
      else if (code >= 0xd800 && code <= 0xdbff && value.charCodeAt(i + 1) >= 0xdc00 && value.charCodeAt(i + 1) <= 0xdfff) { bytes += 4; i++; }
      else bytes += 3;
      if (bytes > CACHE_BYTE_LIMIT) return bytes;
    }
    // Include JavaScript's UTF-16 storage and a retained render/copy representation as well as
    // the escaped transport representation measured above.
    return bytes+value.length*4;
  }
  if (!value || typeof value !== "object") return 16;
  const cached = byteSizes.get(value);
  if (cached !== undefined) return cached;
  let bytes = 64;
  for (const [key, child] of Object.entries(value)) {
    bytes += key.length * 2 + 32 + ownedBytes(child);
    if (bytes > CACHE_BYTE_LIMIT) break;
  }
  byteSizes.set(value, bytes);
  return bytes;
}

function evict(id: string) {
  cacheBytes -= cacheSizes.get(id) ?? 0;
  cacheRows -= cache.get(id)?.rows.length ?? 0;
  cache.delete(id);
  cacheSizes.delete(id);
}

export function cachedChat(id: string) {
  const snapshot = cache.get(id);
  if (snapshot) { cache.delete(id); cache.set(id, snapshot); }
  return snapshot;
}
export function cacheChat(id: string, snapshot: ChatSnapshot) {
  evict(id);
  const bytes = ownedBytes(snapshot);
  // An over-budget window remains in its mounted view and can be read again from the backend. It must
  // not displace every smaller conversation or silently replace its body with a partial cache entry.
  if (bytes > CACHE_BYTE_LIMIT || snapshot.rows.length > CACHE_ROW_LIMIT) return;
  cache.set(id, snapshot);
  cacheSizes.set(id, bytes);
  cacheBytes += bytes;
  cacheRows += snapshot.rows.length;
  while (cache.size > 8 || cacheBytes > CACHE_BYTE_LIMIT || cacheRows > CACHE_ROW_LIMIT) evict(cache.keys().next().value!);
}
export function clearChatCache() { cache.clear(); cacheSizes.clear(); cacheBytes = 0; cacheRows = 0; }

/** Keep a recoverable window, retaining the edge the reader requested rather than trimming message bodies. */
export function limitHistoryWindow(rows: ChatRow[], edge: "older" | "newer") {
  let start = edge === "older" ? 0 : rows.length;
  let end = start;
  let bytes = 0;
  while (end - start < ACTIVE_HISTORY_ROWS && (edge === "older" ? end < rows.length : start > 0)) {
    const index = edge === "older" ? end : start - 1;
    const next = ownedBytes(rows[index]);
    if (bytes + next > ACTIVE_HISTORY_BYTES) break;
    bytes += next;
    if (edge === "older") end++; else start--;
  }
  if (rows.length && start === end) throw new Error("history_window_overflow: "+t("chat.history.windowLimit"));
  return { rows: rows.slice(start, end), droppedBefore: start > 0, droppedAfter: end < rows.length };
}

export function mergeRows(base: ChatRow[], incoming: ChatRow[], retainWindow = false, preserveIds?: ReadonlySet<string>): ChatRow[] {
  const result = [...base];
  const indices = new Map(result.map((row, index) => [row.id, index]));
  const minimum = Math.min(...base.map(row => chatRowPosition(row) ?? Infinity));
  const preservedTail = new Set<string>();
  for (const row of incoming) {
    const position = chatRowPosition(row) ?? Infinity;
    if (retainWindow && Number.isFinite(minimum) && !indices.has(row.id) && position < minimum) {
      if (!preserveIds?.has(row.id)) continue;
      preservedTail.add(row.id);
    }
    const index = indices.get(row.id);
    if (index === undefined) { indices.set(row.id, result.length); result.push(row); }
    else result[index] = row;
  }
  if (preservedTail.size === 0) {
    return result.sort((a, b) => {
      const left = chatRowPosition(a), right = chatRowPosition(b);
      return left !== undefined && right !== undefined ? left - right : 0;
    });
  }
  const order = new Map(result.map((row, index) => [row, index]));
  const maxPos = result.reduce((max, row) => {
    if (preservedTail.has(row.id)) return max;
    const pos = chatRowPosition(row);
    return pos !== undefined && pos > max ? pos : max;
  }, 0);
  const sortKey = (row: ChatRow) => preservedTail.has(row.id) ? maxPos + 1 + (order.get(row) ?? 0) : chatRowPosition(row);
  return result.sort((a, b) => {
    const left = sortKey(a), right = sortKey(b);
    return left !== undefined && right !== undefined ? left - right : 0;
  });
}

/** Events are already visible. Replay their newer state over the late baseline in one commit. */
export function reconcileChat(snapshot: ChatSnapshot, current: ChatRow[], events: ChatEvent[], preserveIds?: ReadonlySet<string>): ChatSnapshot {
  let result = { ...snapshot, rows: snapshot.pageKind === "delta" ? mergeRows(current, snapshot.rows, false, preserveIds) : snapshot.rows };
  for (const event of events) {
    if (event.type === "extras") { result = { ...result, auth: event.extras.auth }; continue; }
    // A snapshot read just before a turn started or ended would otherwise put back the old turn clock:
    // the pane then shows no working row for a turn still running, or keeps one for a turn that ended.
    if (event.type === "turnStarted") { result = { ...result, turnStartedAt: event.startedAt }; continue; }
    if (event.type === "turnCompleted") { result = { ...result, turnStartedAt: undefined }; continue; }
    if (event.type === "process") { result = { ...result, running: true, pid: event.pid }; continue; }
    if (event.type === "exited") { result = { ...result, running: false, turnStartedAt: undefined }; continue; }
    if (event.type !== "rows" && event.type !== "replaceRows" && event.type !== "queued" && event.type !== "reset") continue;
    const epoch = event.epoch ?? result.startedAt;
    if (epoch !== undefined && result.startedAt !== undefined && epoch < result.startedAt) continue;
    if (epoch !== undefined && epoch !== result.startedAt) {
      result = { ...result, userMessages: [], auth: undefined, startedAt: epoch, rows: [], queue: [], rowsRevision: 0, queueRevision: 0, hasMore: false, hasNewer: false };
    }
    if (event.type === "reset") {
      result = { ...result, auth: undefined, turnStartedAt: undefined };
      if (event.rows && (event.revision === undefined || event.revision > (result.rowsRevision ?? -1))) {
        result = { ...result, userMessages: event.userMessages ?? [], rows: event.rows, rowsRevision: event.revision, hasMore: event.hasMore ?? false, hasNewer: false };
      }
      continue;
    }
    if (event.type === "queued") {
      if (event.revision === undefined || event.revision > (result.queueRevision ?? -1)) {
        result = { ...result, queue: event.items, queueRevision: event.revision };
      }
    } else if (event.revision === undefined || event.revision > (result.rowsRevision ?? -1)) {
      if (event.type === "replaceRows") result = { ...result, userMessages: event.userMessages ?? [] };
      result = {
        ...result,
        rows: event.type === "rows" ? mergeRows(result.rows, event.rows, result.hasMore ?? false, preserveIds) : event.rows,
        rowsRevision: event.revision,
        hasMore: event.type === "replaceRows" ? (event.hasMore ?? false) : result.hasMore,
        hasNewer: event.type === "replaceRows" ? false : result.hasNewer,
      };
    }
  }
  if (result.protocolVersion === 2) {
    const window = limitHistoryWindow(result.rows, "newer");
    result = { ...result, rows: window.rows, hasMore: result.hasMore || window.droppedBefore, hasNewer: result.hasNewer || window.droppedAfter };
  }
  return result;
}

/** Numbers only; diagnostics never retain message text or attachments. */
export const chatSyncMetrics: { requestMs: number; commitMs: number; rows: number; mode: string }[] = [];
