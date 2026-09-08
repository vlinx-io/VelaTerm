import { beforeEach, expect, it, vi } from "vitest";
vi.mock("../../../ipc/chat", () => ({ chatSend: vi.fn() }));
import { chatSend } from "../../../ipc/chat";
import { acknowledgeSubmissions, ChatVersions, createSubmission, deliverSubmission, retrySubmission, submissionsFor, useOutbox } from "./outbox";

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

it("retries a confirmed rejection with a fresh identifier and the same behavior", async () => {
  vi.mocked(chatSend).mockRejectedValueOnce(new Error("No running turn")).mockResolvedValueOnce("sent");
  const item = createSubmission("s", "Hello", [], "steer");
  await deliverSubmission("s", item);
  await retrySubmission("s", submissionsFor("s")[0]);
  expect(submissionsFor("s")).toHaveLength(1);
  expect(submissionsFor("s")[0].id).not.toBe(item.id);
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
