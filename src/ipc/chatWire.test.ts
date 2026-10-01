import { describe, expect, it } from "vitest";

import fixture from "./chatWire.fixture.json";
import { CHAT_WIRE_VERSION, ChatWireDecoder, ChatWireGap, applyObject } from "./chatWire";

// The fixture is written by the Rust encoder test (`web::chat_wire::tests`), so decoding it here pins the
// TypeScript decoder to exactly what the server sends.
describe("chat wire decoder", () => {
  it("decodes every frame the Rust encoder produced back to the engine's payload", () => {
    expect(fixture.version).toBe(CHAT_WIRE_VERSION);
    const decoder = new ChatWireDecoder();
    for (const frame of fixture.frames) {
      const sid = frame.name.slice("chat://event/".length);
      expect(decoder.decode(sid, frame.payload)).toEqual(frame.decoded);
    }
  });

  it("never mutates what it handed out before", () => {
    const decoder = new ChatWireDecoder();
    const first = decoder.decode("s", { type: "rows", epoch: 1, retain: ["r"], rows: [{ kind: "assistant", id: "r", text: "a", streaming: true, meta: { n: 1 } }] }) as { rows: object[] };
    const frozen = JSON.parse(JSON.stringify(first));
    const second = decoder.decode("s", { type: "rows", epoch: 1, retain: ["r"], rows: [{ id: "r", patch: { app: { text: "b" }, len: { text: 1 }, sub: { meta: { set: { n: 2 } } } } }] }) as { rows: Array<Record<string, unknown>> };
    expect(first).toEqual(frozen);
    expect(second.rows[0]).not.toBe(first.rows[0]);
    expect(second.rows[0]).toEqual({ kind: "assistant", id: "r", text: "ab", streaming: true, meta: { n: 2 } });
    expect(second).not.toHaveProperty("retain");
  });

  it("keeps exactly the retained rows as bases", () => {
    const decoder = new ChatWireDecoder();
    decoder.decode("s", { type: "rows", epoch: 1, rows: [{ kind: "tool", id: "t", status: "completed" }] });
    expect(() => decoder.decode("s", { type: "rows", epoch: 1, rows: [{ id: "t", patch: { set: { status: "failed" } } }] })).toThrow(ChatWireGap);
    decoder.decode("s", { type: "rows", epoch: 1, retain: ["t"], rows: [{ kind: "tool", id: "t", status: "running" }] });
    // A new epoch drops every base.
    expect(() => decoder.decode("s", { type: "rows", epoch: 2, rows: [{ id: "t", patch: { set: { status: "failed" } } }] })).toThrow(ChatWireGap);
  });

  it("treats a tampered append length or a missing base as a gap", () => {
    expect(() => applyObject({ text: "héllo" }, { app: { text: "!" }, len: { text: 4 } })).toThrow(ChatWireGap);
    expect(applyObject({ text: "😀" }, { app: { text: "!" }, len: { text: 2 } })).toEqual({ text: "😀!" });
    const decoder = new ChatWireDecoder();
    expect(() => decoder.decode("s", { type: "extras", patch: { set: { fastMode: true } } })).toThrow(ChatWireGap);
    expect(() => applyObject({ list: [{ id: "a" }] }, { arr: { list: { key: "id", order: ["a", "b"] } } })).toThrow(ChatWireGap);
  });

  // Keys are agent data. The legacy path (JSON.parse of a full frame) and the Rust reference decoder only know own
  // keys, so `__proto__` must stay a key and never become the decoded object's prototype.
  it("keeps agent-controlled keys like __proto__ as own keys, as JSON.parse does", () => {
    const base = JSON.parse('{"input":{"description":"list"}}');
    const set = applyObject(base, JSON.parse('{"sub":{"input":{"set":{"__proto__":{"command":"rm -rf ~"}}}}}'));
    const input = set.input as Record<string, unknown>;
    expect(Object.getPrototypeOf(input)).toBe(Object.prototype);
    expect(input.command).toBeUndefined();
    expect(Object.keys(input)).toEqual(["description", "__proto__"]);
    expect(JSON.stringify(set)).toBe('{"input":{"description":"list","__proto__":{"command":"rm -rf ~"}}}');
    const own = applyObject(JSON.parse('{"__proto__":"ab"}'), JSON.parse('{"app":{"__proto__":"c"},"len":{"__proto__":2}}'));
    expect(JSON.stringify(own)).toBe('{"__proto__":"abc"}');
    // A nested or list patch of a key the base does not own has no base, whatever the prototype chain offers.
    expect(() => applyObject({}, JSON.parse('{"sub":{"__proto__":{"set":{"polluted":true}}}}'))).toThrow(ChatWireGap);
    expect(() => applyObject({}, JSON.parse('{"sub":{"toString":{"set":{"x":1}}}}'))).toThrow(ChatWireGap);
    expect(() => applyObject({}, JSON.parse('{"arr":{"__proto__":{"key":"id","order":[]}}}'))).toThrow(ChatWireGap);
    expect(() => applyObject({ list: [] }, JSON.parse('{"arr":{"list":{"key":"id","order":["toString"]}}}'))).toThrow(ChatWireGap);
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
  });

  it("passes the full frames of an older server through unchanged", () => {
    const decoder = new ChatWireDecoder();
    const rows = { type: "rows", epoch: 1, revision: 2, rows: [{ kind: "user", id: "u", text: "hi" }] };
    const extras = { type: "extras", extras: { fastMode: false, backgroundTasks: [{ task_id: "t", workflow_progress: [] }] } };
    const queued = { type: "queued", items: [] };
    expect(decoder.decode("s", rows)).toEqual(rows);
    expect(decoder.decode("s", extras)).toBe(extras);
    expect(decoder.decode("s", queued)).toBe(queued);
  });
});
