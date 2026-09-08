//! Chat IPC contract tests: snapshots keep image bytes out of the main payload, then resolve stable
//! references through the session-owned attachment endpoint only when a mounted image asks for them.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./transport", () => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

import { chatRewind, chatRewindPreview, chatSnapshot, resolveChatImage } from "./chat";
import { invoke } from "./transport";

describe("chat snapshot attachments", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("keeps row and queue references deferred until each image is requested", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "chat_snapshot") {
        return Promise.resolve({
          running: true,
          rows: [
            {
              kind: "user",
              id: "u-1",
              text: "look",
              images: [{ mimeType: "image/png", attachmentId: "row:u-1:0" }],
            },
          ],
          queue: [
            {
              id: "q-1",
              text: "next",
              images: [{ mimeType: "image/jpeg", attachmentId: "queue:q-1:0" }],
            },
          ],
          permissions: [],
          commands: [],
          configKeys: [],
        }) as Promise<never>;
      }
      const request = args as { attachmentId?: string } | undefined;
      const jpeg = request?.attachmentId?.startsWith("queue:");
      return Promise.resolve({
        mimeType: jpeg ? "image/jpeg" : "image/png",
        data: jpeg ? "SlBFRw==" : "UE5H",
      }) as Promise<never>;
    });

    const snapshot = await chatSnapshot("session-1");
    expect(snapshot.rows[0]).toMatchObject({
      images: [{ mimeType: "image/png", attachmentId: "row:u-1:0" }],
    });
    expect(snapshot.queue[0]).toMatchObject({
      images: [{ mimeType: "image/jpeg", attachmentId: "queue:q-1:0" }],
    });
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual(["chat_snapshot"]);

    const row = snapshot.rows[0];
    const queued = snapshot.queue[0];
    if (row?.kind !== "user" || !row.images?.[0] || !queued?.images?.[0]) {
      throw new Error("Snapshot fixture did not contain its image references");
    }
    expect(await resolveChatImage(row.images[0])).toEqual({ mimeType: "image/png", data: "UE5H" });
    expect(await resolveChatImage(queued.images[0])).toEqual({ mimeType: "image/jpeg", data: "SlBFRw==" });
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
      "chat_snapshot",
      "chat_attachment",
      "chat_attachment",
    ]);
  });
});

describe("chat rewind transport", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("keeps the session-owned row id and explicit scope in the shared dispatch contract", async () => {
    vi.mocked(invoke).mockResolvedValue({ prefillText: "try again" } as never);

    await chatRewindPreview("session-1", "u-2");
    await chatRewind("session-1", "u-2", "both");

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["chat_rewind_preview", { sessionId: "session-1", rowId: "u-2" }],
      ["chat_rewind", { sessionId: "session-1", rowId: "u-2", scope: "both" }],
    ]);
  });
});
