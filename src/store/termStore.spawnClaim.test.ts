//! A frontend can lose any acknowledgement; only durable backend receipts settle a request.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SpawnRequest } from "../ipc/events";
import type { SpawnReceipt } from "../ipc/commands";
const ipc = vi.hoisted(() => ({ resolveSpawn: vi.fn(), spawnRequests: vi.fn(), spawnRequest: vi.fn(), retrySpawn: vi.fn() }));
vi.mock("../ipc/commands", async original => ({ ...await original<typeof import("../ipc/commands")>(), ...ipc }));
vi.mock("../notify", async original => ({ ...await original<typeof import("../notify")>(), notify: vi.fn() }));
import { useTermStore } from "./termStore";
const req: SpawnRequest = { requestId: "request-1", parentSessionId: "parent", prompt: "same task" };
const receipt = (patch: Partial<SpawnReceipt> = {}): SpawnReceipt => ({
  requestId: req.requestId!, request: req, decision: "pending", state: "pending", sessionId: "child", messageId: "msg-fixed", session: null, error: null, ...patch,
});
const openSession = vi.fn(), loadTree = vi.fn(), addSession = vi.fn();
beforeEach(() => {
  vi.clearAllMocks();
  loadTree.mockResolvedValue(undefined);
  useTermStore.setState({ pendingSpawns: [], spawnReceipts: {}, notifyEnabled: false, loadTree, openSession, addSession });
});

describe("durable spawn decisions", () => {
  it("does not open a confirmation card before a no-confirm receipt arrives", async () => {
    const request = { ...req, noConfirm: true };
    let acknowledge!: (value: SpawnReceipt) => void;
    ipc.spawnRequest.mockReturnValue(new Promise<SpawnReceipt>(resolve => { acknowledge = resolve; }));
    const handling = useTermStore.getState().handleSpawnRequest(request);
    expect(useTermStore.getState().pendingSpawns).toEqual([]);
    acknowledge(receipt({ request, decision: "confirmed", state: "complete" }));
    await handling;
    expect(useTermStore.getState().pendingSpawns).toEqual([]);
  });
  it.each([false, true])("keeps automatic startup out of confirmation while retaining failures for retry (noConfirm=%s)", async noConfirm => {
    const request = { ...req, noConfirm };
    await useTermStore.getState().applySpawnReceipt(receipt({ request, decision: "confirmed", state: "pending" }));
    expect(useTermStore.getState().pendingSpawns).toEqual([]);
    await useTermStore.getState().applySpawnReceipt(receipt({ request, decision: "confirmed", state: "failed", error: "agent unavailable" }));
    expect(useTermStore.getState().pendingSpawns).toEqual([request]);
  });
  it("does not erase a no-confirm startup failure when an older pending receipt arrives", async () => {
    const request = { ...req, noConfirm: true };
    const failed = receipt({ request, decision: "confirmed", state: "failed", error: "agent unavailable" });
    await useTermStore.getState().applySpawnReceipt(failed);
    await useTermStore.getState().applySpawnReceipt(receipt({ request, decision: "confirmed", state: "pending" }));
    expect(useTermStore.getState().pendingSpawns).toEqual([request]);
    expect(useTermStore.getState().spawnReceipts["request-1"]).toEqual(failed);
    ipc.resolveSpawn.mockResolvedValue({ ...failed, state: "complete", error: null, session: { id: "child" } });
    await useTermStore.getState().confirmSpawn(request);
    expect(useTermStore.getState().pendingSpawns).toEqual([]);
    expect(openSession).toHaveBeenCalledWith("child", expect.any(Object));
  });
  it("does not automatically retry a failure after an older recovery list arrives", async () => {
    const request = { ...req, noConfirm: true };
    let acknowledge!: (value: SpawnReceipt[]) => void;
    ipc.spawnRequests.mockReturnValue(new Promise<SpawnReceipt[]>(resolve => { acknowledge = resolve; }));
    const sync = useTermStore.getState().syncSpawnRequests();
    const failed = receipt({ request, decision: "confirmed", state: "failed", error: "agent unavailable" });
    await useTermStore.getState().applySpawnReceipt(failed);
    ipc.retrySpawn.mockResolvedValue({ ...failed, state: "complete", error: null });
    acknowledge([receipt({ request, decision: "confirmed", state: "pending" })]);
    await sync;
    expect(ipc.retrySpawn).not.toHaveBeenCalled();
    expect(useTermStore.getState().spawnReceipts["request-1"]).toEqual(failed);
    expect(useTermStore.getState().pendingSpawns).toEqual([request]);
  });
  it("keeps edited task and images when confirmation has no acknowledgement", async () => {
    const draft = { ...req, prompt: "edited", images: [{ mimeType: "image/png", data: "AQID" }] };
    useTermStore.setState({ pendingSpawns: [req] });
    ipc.resolveSpawn.mockRejectedValue(new Error("disconnected"));
    await expect(useTermStore.getState().confirmSpawn(draft)).rejects.toThrow("disconnected");
    expect(useTermStore.getState().pendingSpawns).toEqual([draft]);
    expect(addSession).not.toHaveBeenCalled();
    expect(ipc.resolveSpawn).toHaveBeenCalledWith("request-1", true, draft);
  });
  it("reads a confirmed result after a lost acknowledgement and opens the same session", async () => {
    const result = receipt({ decision: "confirmed", state: "complete", session: { id: "child" } as never });
    ipc.resolveSpawn.mockRejectedValueOnce(new Error("lost reply")).mockResolvedValue(result);
    useTermStore.setState({ pendingSpawns: [req] });
    await expect(useTermStore.getState().confirmSpawn(req)).rejects.toThrow();
    await useTermStore.getState().confirmSpawn(req);
    expect(ipc.resolveSpawn.mock.calls.every(call => call[0] === "request-1")).toBe(true);
    expect(useTermStore.getState().pendingSpawns).toEqual([]);
    expect(openSession).toHaveBeenCalledWith("child", expect.any(Object)); expect(addSession).not.toHaveBeenCalled();
  });
  it("keeps cancellation visible until the backend acknowledges it", async () => {
    useTermStore.setState({ pendingSpawns: [req] });
    ipc.resolveSpawn.mockRejectedValueOnce(new Error("offline"));
    await expect(useTermStore.getState().cancelSpawn()).rejects.toThrow("offline");
    expect(useTermStore.getState().pendingSpawns).toEqual([req]);
    ipc.resolveSpawn.mockResolvedValue(receipt({ decision: "cancelled", state: "cancelled" }));
    await useTermStore.getState().cancelSpawn(); expect(useTermStore.getState().pendingSpawns).toEqual([]);
  });
  it("uses a recorded cancel when another client wins a confirm/cancel race", async () => {
    useTermStore.setState({ pendingSpawns: [req] });
    ipc.resolveSpawn.mockResolvedValue(receipt({ decision: "cancelled", state: "cancelled" }));
    await useTermStore.getState().confirmSpawn(req);
    expect(openSession).not.toHaveBeenCalled(); expect(addSession).not.toHaveBeenCalled();
    expect(useTermStore.getState().pendingSpawns).toEqual([]);
  });
  it("settles by request ID without removing another request with the same prompt", async () => {
    const second = { ...req, requestId: "request-2" };
    useTermStore.setState({ pendingSpawns: [req, second] });
    await useTermStore.getState().applySpawnReceipt(receipt({ decision: "cancelled", state: "cancelled" }));
    expect(useTermStore.getState().pendingSpawns).toEqual([second]);
  });
  it.each([false, true])("ignores the frontend confirmation preference %s", async spawnConfirm => {
    useTermStore.setState({ spawnConfirm }); ipc.spawnRequest.mockResolvedValue(receipt());
    await useTermStore.getState().handleSpawnRequest({ ...req, noConfirm: true });
    expect(useTermStore.getState().pendingSpawns).toEqual([req]);
    expect(ipc.resolveSpawn).not.toHaveBeenCalled(); expect(addSession).not.toHaveBeenCalled();
  });
  it("retains an event when the receipt read fails", async () => {
    ipc.spawnRequest.mockRejectedValue(new Error("offline"));
    await expect(useTermStore.getState().handleSpawnRequest(req)).rejects.toThrow();
    expect(useTermStore.getState().pendingSpawns).toEqual([req]); expect(addSession).not.toHaveBeenCalled();
  });
  it("does not duplicate a pending card when an event is replayed", async () => {
    ipc.spawnRequest.mockResolvedValue(receipt());
    await useTermStore.getState().handleSpawnRequest(req); await useTermStore.getState().handleSpawnRequest(req);
    expect(useTermStore.getState().pendingSpawns).toHaveLength(1);
  });
  it("restores offline reviews and interrupted creation from backend state", async () => {
    const pending = receipt({ requestId: "review", request: { ...req, requestId: "review" } });
    ipc.spawnRequests.mockResolvedValue([pending, receipt({ decision: "confirmed" })]);
    ipc.retrySpawn.mockResolvedValue(receipt({ decision: "confirmed", state: "ready", session: { id: "child" } as never }));
    await useTermStore.getState().syncSpawnRequests();
    expect(ipc.retrySpawn).toHaveBeenCalledExactlyOnceWith("request-1");
    expect(useTermStore.getState().pendingSpawns).toEqual([pending.request]);
    expect(openSession).toHaveBeenCalledWith("child", expect.any(Object));
  });
  it.each(["failed", "uncertain", "dispatching"] as const)("preserves %s for explicit recovery without an event loop", async state => {
    ipc.spawnRequests.mockResolvedValue([receipt({ decision: "confirmed", state, error: "delivery needs attention" })]);
    await useTermStore.getState().syncSpawnRequests();
    expect(ipc.retrySpawn).not.toHaveBeenCalled(); expect(useTermStore.getState().pendingSpawns).toEqual([req]);
    expect(useTermStore.getState().spawnReceipts["request-1"].error).toBe("delivery needs attention");
  });
  it.each(["none", "shared", "each"] as const)("retains plan-execute %s configuration and images on retry", async worktreeMode => {
    const request = { ...req, images: [{ mimeType: "image/png", data: "AQID" }], planExecute: { splitTasks: true, worktreeMode, plan: {}, exec: {} } };
    const failed = receipt({ request, decision: "confirmed", state: "failed", error: "agent missing" });
    ipc.resolveSpawn.mockResolvedValueOnce(failed).mockResolvedValueOnce({ ...failed, state: "complete", error: null, session: { id: "planner" } });
    useTermStore.setState({ pendingSpawns: [request] });
    await expect(useTermStore.getState().confirmSpawn(request)).rejects.toThrow("agent missing");
    expect(useTermStore.getState().pendingSpawns).toEqual([request]);
    await useTermStore.getState().confirmSpawn(request);
    expect(ipc.resolveSpawn).toHaveBeenNthCalledWith(2, "request-1", true, request);
    expect(openSession).toHaveBeenCalledWith("planner", expect.any(Object)); expect(addSession).not.toHaveBeenCalled();
  });
  it("does not silently dismiss a failed bulk cancellation", async () => {
    ipc.resolveSpawn.mockRejectedValue(new Error("offline")); useTermStore.setState({ pendingSpawns: [req] });
    useTermStore.getState().clearAllBadges(); await Promise.resolve(); await Promise.resolve();
    expect(useTermStore.getState().pendingSpawns).toEqual([req]);
  });
});

it("keeps the selected card in place through confirmed creation before a recoverable failure", async () => {
  const other = { ...req, requestId: "other-request", prompt: "Other review" };
  useTermStore.setState({ pendingSpawns: [req, other] });
  const approved = { ...req, prompt: "Approved by another client" };
  await useTermStore.getState().applySpawnReceipt(receipt({ request: approved, decision: "confirmed", state: "pending" }));
  expect(useTermStore.getState().pendingSpawns).toEqual([approved, other]);
  await useTermStore.getState().applySpawnReceipt(receipt({ request: approved, decision: "confirmed", state: "failed", error: "creation rejected" }));
  expect(useTermStore.getState().pendingSpawns).toEqual([approved, other]);
  expect(openSession).not.toHaveBeenCalled();
});
