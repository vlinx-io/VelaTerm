//! Decoder for the chat codec a WebSocket connection negotiates with `{t:"caps",chat:1}`.
//!
//! The server (`src-tauri/src/web/chat_wire.rs`, which documents the patch grammar) sends each chat event as a
//! patch against what exactly this connection received last. This module rebuilds the engine's full payload
//! before any listener sees it, so the views never learn that a codec exists. It is pure: every decoded object
//! is new, and nothing handed out earlier is ever mutated (views key per-row facts on object identity).
//!
//! Full frames pass through unchanged, which is all an older server ever sends. A patch whose base is missing
//! or whose append length does not match is a gap: `decode` throws `ChatWireGap`, and the caller resolves it
//! with an explicit resync (see `wsClient.ts`).

/** The codec version this client decodes, announced in `caps`. */
export const CHAT_WIRE_VERSION = 1;

type Obj = Record<string, unknown>;

/** A patch could not be applied: its base is missing or differs from what the server encoded against. */
export class ChatWireGap extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ChatWireGap";
  }
}

const isObj = (value: unknown): value is Obj => typeof value === "object" && value !== null && !Array.isArray(value);

/** An own property only. Keys come from agent-controlled data, so `__proto__` or `toString` must never resolve
 *  through the prototype chain; JSON.parse and the Rust reference decoder only know own keys. */
const has = (obj: Obj, key: string): boolean => Object.prototype.hasOwnProperty.call(obj, key);
const own = (obj: Obj, key: string): unknown => (has(obj, key) ? obj[key] : undefined);

/** Write `key` as an own data property, as JSON.parse does. A plain assignment of `__proto__` would replace the
 *  object's prototype instead of creating the key. */
function put(obj: Obj, key: string, value: unknown): void {
  Object.defineProperty(obj, key, { value, writable: true, enumerable: true, configurable: true });
}

function applyArray(base: unknown, patch: Obj): unknown[] {
  if (!Array.isArray(base)) throw new ChatWireGap("list patch without a list base");
  const key = String(patch.key);
  const bases = new Map<string, unknown>();
  for (const item of base) if (isObj(item) && typeof own(item, key) === "string") bases.set(item[key] as string, item);
  const order = Array.isArray(patch.order) ? (patch.order as string[]) : [...bases.keys()];
  const items = isObj(patch.items) ? patch.items : {};
  return order.map(id => {
    const item = own(items, id);
    if (isObj(item) && has(item, "new")) return item.new;
    if (!bases.has(id)) throw new ChatWireGap(`list entry ${id} without a base`);
    return isObj(item) ? applyObject(bases.get(id), item) : bases.get(id);
  });
}

/** Apply one object patch to `base`, returning a new object. */
export function applyObject(base: unknown, patch: Obj): Obj {
  if (!isObj(base)) throw new ChatWireGap("object patch without an object base");
  const out: Obj = { ...base };
  if (Array.isArray(patch.del)) for (const key of patch.del) delete out[String(key)];
  if (isObj(patch.set)) for (const [key, value] of Object.entries(patch.set)) put(out, key, value);
  if (isObj(patch.app)) {
    const lengths = isObj(patch.len) ? patch.len : {};
    for (const [key, suffix] of Object.entries(patch.app)) {
      const current = own(out, key);
      if (typeof current !== "string" || current.length !== own(lengths, key)) {
        throw new ChatWireGap(`append base of ${key} does not match`);
      }
      put(out, key, current + String(suffix));
    }
  }
  if (isObj(patch.sub)) for (const [key, inner] of Object.entries(patch.sub)) {
    if (!has(out, key)) throw new ChatWireGap(`nested patch of ${key} without a base`);
    put(out, key, applyObject(out[key], inner as Obj));
  }
  if (isObj(patch.arr)) for (const [key, list] of Object.entries(patch.arr)) {
    put(out, key, applyArray(own(out, key), list as Obj));
  }
  return out;
}

interface SessionBases {
  epoch?: unknown;
  /** Live rows the server keeps as patch bases, by id. */
  rows: Map<string, Obj>;
  extras?: Obj;
}

/** Per-connection decoder state for every chat session it has seen. */
export class ChatWireDecoder {
  private readonly sessions = new Map<string, SessionBases>();

  /** A new connection starts from nothing. */
  reset(): void {
    this.sessions.clear();
  }

  /** Forget one session, after a gap or at its resync barrier. */
  forget(sessionId: string): void {
    this.sessions.delete(sessionId);
  }

  /** Rebuild the engine's payload for one `chat://event/{sessionId}` event. Throws `ChatWireGap`. */
  decode(sessionId: string, payload: unknown): unknown {
    if (!isObj(payload)) return payload;
    let state = this.sessions.get(sessionId);
    if (!state) {
      state = { rows: new Map() };
      this.sessions.set(sessionId, state);
    }
    switch (payload.type) {
      case "rows": {
        if (payload.epoch !== state.epoch) {
          state.rows.clear();
          state.epoch = payload.epoch;
        }
        const working = new Map(state.rows);
        const rows = (Array.isArray(payload.rows) ? payload.rows : []).map(entry => {
          let row = entry as Obj;
          if (isObj(entry) && !("kind" in entry) && isObj(entry.patch)) {
            const base = working.get(String(entry.id));
            if (!base) throw new ChatWireGap(`row ${String(entry.id)} patch without a base`);
            row = applyObject(base, entry.patch);
          }
          if (isObj(row)) working.set(String(row.id), row);
          return row;
        });
        const retain = Array.isArray(payload.retain) ? (payload.retain as string[]) : [];
        state.rows = new Map(retain.filter(id => working.has(id)).map(id => [id, working.get(id)!]));
        const { retain: _retain, ...rest } = payload;
        return { ...rest, rows };
      }
      case "replaceRows":
      case "reset":
        state.rows.clear();
        state.epoch = payload.epoch ?? state.epoch;
        return payload;
      case "extras": {
        if (isObj(payload.patch)) {
          if (!state.extras) throw new ChatWireGap("extras patch without a base");
          const { patch, ...rest } = payload;
          const decoded = { ...rest, extras: applyObject(state.extras, patch as Obj) };
          state.extras = decoded.extras;
          return decoded;
        }
        state.extras = isObj(payload.extras) ? payload.extras : undefined;
        return payload;
      }
      default:
        return payload;
    }
  }
}
