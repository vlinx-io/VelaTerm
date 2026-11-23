import { beforeEach, expect, it, vi } from "vitest";
vi.mock("../../../ipc/chat", () => ({ chatSend: vi.fn(), resolveChatImage: vi.fn() }));
import { chatSend } from "../../../ipc/chat";
import { acknowledgeSubmissions, ChatVersions, createSubmission, deliverSubmission, reconcileConfirmedSubmissions, retrySubmission, submissionsFor, useOutbox, unconfirmedSubmissionRows } from "./outbox";

beforeEach(() => {
  useOutbox.setState({ sessions: {} });
  vi.mocked(chatSend).mockReset();
});

it("keeps one optimistic message until both its row and receipt arrive, in either order", async () => {
  for (const eventFirst of [true, false]) {
    let resolve!: (status: "sent") => void;
    vi.mocked(chatSend).mockImplementation(() => new Promise(done => { resolve = done; }));
    const item = createSubmission("s", "Hello", [], "queue");
    const sending = deliverSubmission("s", item);
    await deliverSubmission("s", item);
    if (eventFirst) acknowledgeSubmissions("s", [item.id]);
    resolve("sent");
    await sending;
    if (!eventFirst) {
      expect(submissionsFor("s")[0].status).toBe("sent");
      acknowledgeSubmissions("s", [item.id]);
    }
    expect(submissionsFor("s")).toEqual([]);
  }
  expect(chatSend).toHaveBeenCalledTimes(2);
});

it("rechecks uncertain delivery with the same identifier and retains its attachments", async () => {
  const error = new Error("Disconnected");
  error.name = "TransportError";
  vi.mocked(chatSend).mockRejectedValueOnce(error).mockResolvedValueOnce("queued");
  const images = [{ mimeType: "image/png", data: "abc" }];
  const item = createSubmission("s", "Hello", images, "interrupt");
  await deliverSubmission("s", item);
  expect(submissionsFor("s")[0].status).toBe("unknown");
  await retrySubmission("s", submissionsFor("s")[0]);
  expect(chatSend).toHaveBeenNthCalledWith(2, "s", "Hello", "interrupt", images, item.id);
  expect(submissionsFor("s")[0].status).toBe("queued");
});

it("retries a confirmed rejection with its original identifier and the same behavior", async () => {
  vi.mocked(chatSend).mockRejectedValueOnce(new Error("No running turn")).mockResolvedValueOnce("sent");
  const item = createSubmission("s", "Hello", [], "steer");
  await deliverSubmission("s", item);
  await retrySubmission("s", submissionsFor("s")[0]);
  expect(submissionsFor("s")).toHaveLength(1);
  expect(submissionsFor("s")[0].id).toBe(item.id);
  expect(submissionsFor("s")[0].behavior).toBe("steer");
});

it("rejects old collection versions and old processes after a restart", () => {
  const versions = new ChatVersions();
  expect(versions.accept("rows", 5, 100)).toBe(true);
  expect(versions.accept("rows", 4, 100)).toBe(false);
  expect(versions.accept("queue", 2, 100)).toBe(true);
  expect(versions.accept("queue", 1, 100)).toBe(false);
  expect(versions.accept("rows", 1, 200)).toBe(true);
  expect(versions.accept("rows", 999, 100)).toBe(false);
});


it("reconciles optimistic rows by identifier without hiding repeated text", () => {
  const first = createSubmission("s", "Hello", [], "queue", true);
  const second = createSubmission("s", "Hello", [], "queue", true);
  const third = createSubmission("s", "Hello", [], "queue", true);
  expect(unconfirmedSubmissionRows([{ kind: "user", id: first.id, text: "Translated" }], [{ id: second.id }], [first, second, third])).toEqual([
    { kind: "user", id: third.id, text: "Hello", images: [] },
  ]);
});


it("does not republish an already observed submission while its receipt is pending", async () => {
  let resolve!: (status: "sent") => void;
  vi.mocked(chatSend).mockImplementation(() => new Promise(done => { resolve = done; }));
  const item = createSubmission("s", "Hello", [], "queue");
  const pending = deliverSubmission("s", item);
  acknowledgeSubmissions("s", [item.id]);
  const observed = submissionsFor("s");
  acknowledgeSubmissions("s", [item.id]);
  expect(submissionsFor("s")).toBe(observed);
  resolve("sent"); await pending;
  expect(submissionsFor("s")).toEqual([]);
});

it("retires confirmed submissions once a post-receipt tail snapshot has moved on", async () => {
  vi.mocked(chatSend).mockResolvedValue("sent");
  const item = createSubmission("s", "Hello", [], "queue");
  await deliverSubmission("s", item);
  const confirmedAt = submissionsFor("s")[0].confirmedAt!;
  expect(confirmedAt).toBeTypeOf("number");
  // An earlier snapshot cannot retire a submission confirmed after it started.
  reconcileConfirmedSubmissions("s", confirmedAt - 1, [], true);
  expect(submissionsFor("s")).toHaveLength(1);
  // A non-authoritative empty snapshot marks the submission reconciled once without dropping it.
  reconcileConfirmedSubmissions("s", confirmedAt, [], false);
  expect(submissionsFor("s")[0].reconciled).toBe(true);
  // An authoritative tail snapshot started at or after confirmation clears the stale optimistic ghost.
  reconcileConfirmedSubmissions("s", confirmedAt, ["later-row"], true);
  expect(submissionsFor("s")).toEqual([]);
});
