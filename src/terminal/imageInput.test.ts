//! Regression coverage for image-paste routing: path mode must never silently fall back to native agent paste,
//! and WKWebView must use the host-native clipboard fallback when File is unavailable.

import { beforeEach, describe, expect, it, vi } from "vitest";

const { ptyWrite, uploadImage } = vi.hoisted(() => ({
  ptyWrite: vi.fn(),
  uploadImage: vi.fn(),
}));

vi.mock("../ipc/commands", () => ({ ptyWrite }));
vi.mock("../ipc/transport", () => ({ uploadImage }));
vi.mock("../platform", () => ({
  platform: { clipboard: { readImage: vi.fn() } },
}));

import {
  injectImageFiles,
  injectNativeImagePaste,
  planImagePaste,
} from "./imageInput";

function clipboardData(opts: { image?: File; nonImage?: File; text?: string } = {}): DataTransfer {
  const items: Array<{ kind: string; type: string; getAsFile: () => File }> = [];
  if (opts.image) {
    items.push({ kind: "file", type: opts.image.type, getAsFile: () => opts.image! });
  }
  if (opts.nonImage) {
    items.push({ kind: "file", type: opts.nonImage.type, getAsFile: () => opts.nonImage! });
  }
  return {
    items: items as unknown as DataTransferItemList,
    getData: (type: string) => (type === "text/plain" ? (opts.text ?? "") : ""),
  } as DataTransfer;
}

describe("routing of pasted images", () => {
  const image = new File([new Uint8Array([1, 2, 3])], "shot.png", {
    type: "image/png",
  });

  beforeEach(() => {
    ptyWrite.mockReset().mockResolvedValue(undefined);
    uploadImage.mockReset().mockResolvedValue("/tmp/vlx-uploads/shot.png");
  });

  it("hands the paste to the agent only when native mode is explicitly chosen locally", () => {
    expect(planImagePaste(clipboardData({ image }), "agent", true, true)).toEqual({
      kind: "agent",
    });
  });

  it("path mode intercepts an image file and passes it to the upload path", () => {
    const plan = planImagePaste(clipboardData({ image }), "upload", true, true);
    expect(plan.kind).toBe("images");
    if (plan.kind === "images") expect(plan.files).toEqual([image]);
  });

  it("a plain-text paste still goes to xterm and does not read the native image clipboard by mistake", () => {
    expect(planImagePaste(clipboardData({ text: "hello" }), "upload", true, true)).toEqual({
      kind: "text",
    });
  });

  it("falls back to the native Tauri clipboard when WKWebView exposes no image File", () => {
    expect(planImagePaste(clipboardData(), "upload", true, true)).toEqual({
      kind: "native-clipboard",
    });
  });

  it("a remote client is forced into path mode even when the stored value is still agent, and never passes silently", () => {
    expect(planImagePaste(clipboardData({ image }), "agent", false, false).kind).toBe("images");
    expect(planImagePaste(clipboardData(), "agent", false, false)).toEqual({
      kind: "image-unavailable",
    });
  });

  it("Codex path mode injects visible image_path text so it is not collapsed into an attachment placeholder", async () => {
    await injectImageFiles("codex-session", [image], "codex");
    expect(ptyWrite).toHaveBeenCalledWith(
      "codex-session",
      " image_path: /tmp/vlx-uploads/shot.png ",
    );
  });

  it("Claude path mode keeps the existing bare-path input", async () => {
    await injectImageFiles("claude-session", [image], "claude");
    expect(ptyWrite).toHaveBeenCalledWith(
      "claude-session",
      " /tmp/vlx-uploads/shot.png ",
    );
  });

  it("native image mode sends a real Ctrl+V to the PTY", async () => {
    await injectNativeImagePaste("codex-session");
    expect(ptyWrite).toHaveBeenCalledWith("codex-session", "\x16");
  });

  it("routes non-image Finder file pastes away from the native clipboard image fallback (#195)", () => {
    const markdown = new File(["# Doc"], "README.md", { type: "text/markdown" });
    expect(planImagePaste(clipboardData({ nonImage: markdown }), "upload", true, true)).toEqual({
      kind: "non-image-files",
    });
    expect(
      planImagePaste(clipboardData({ nonImage: markdown, text: "README.md" }), "agent", true, true),
    ).toEqual({
      kind: "non-image-files",
    });
  });
});
