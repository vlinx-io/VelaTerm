import { describe, expect, it } from "vitest";

import { attachImages, dataUrl, MAX_IMAGE_BYTES, MAX_IMAGES } from "./attachments";

/** A file of `size` bytes whose contents are known, so the base64 can be checked against them. */
function png(name: string, size: number): File {
  return new File([new Uint8Array(size).fill(0x41)], name, { type: "image/png" });
}

describe("attachImages", () => {
  it("reads a pasted image into base64 the message can carry", async () => {
    const { attachments, rejected } = await attachImages([], [png("shot.png", 3)]);
    expect(rejected).toEqual([]);
    expect(attachments).toHaveLength(1);
    expect(attachments[0].mimeType).toBe("image/png");
    expect(attachments[0].name).toBe("shot.png");
    expect(attachments[0].bytes).toBe(3);
    // "AAA" — three bytes of 0x41, and no `data:` prefix left on the front.
    expect(attachments[0].data).toBe("QUFB");
  });

  it("adds to what is already attached rather than replacing it", async () => {
    const first = await attachImages([], [png("a.png", 2)]);
    const second = await attachImages(first.attachments, [png("b.png", 2)]);
    expect(second.attachments.map((a) => a.name)).toEqual(["a.png", "b.png"]);
  });

  it("takes what fits and reports the rest, instead of failing as a whole", async () => {
    const files = [png("small.png", 8), png("huge.png", MAX_IMAGE_BYTES + 1), png("also.png", 8)];
    const { attachments, rejected } = await attachImages([], files);
    expect(attachments.map((a) => a.name)).toEqual(["small.png", "also.png"]);
    expect(rejected).toEqual([{ reason: "tooLarge", name: "huge.png" }]);
  });

  it("stops at the limit and says so", async () => {
    const files = Array.from({ length: MAX_IMAGES + 2 }, (_, i) => png(`s${i}.png`, 4));
    const { attachments, rejected } = await attachImages([], files);
    expect(attachments).toHaveLength(MAX_IMAGES);
    expect(rejected).toEqual([{ reason: "tooMany" }, { reason: "tooMany" }]);
  });

  it("gives each attachment its own id, so removing one removes only that one", async () => {
    const { attachments } = await attachImages([], [png("a.png", 2), png("a.png", 2)]);
    expect(attachments[0].id).not.toBe(attachments[1].id);
  });
});

describe("dataUrl", () => {
  it("puts the prefix back for the browser to draw", () => {
    expect(dataUrl({ mimeType: "image/png", data: "QUFB" })).toBe("data:image/png;base64,QUFB");
  });
});
