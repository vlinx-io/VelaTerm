import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  env: { isTauri: false, isElectron: false },
  readFiles: vi.fn<() => Promise<string[]>>(),
  statFile: vi.fn(),
  loadFileBlob: vi.fn(),
}));

vi.mock("../platform/env", () => ({ env: hoisted.env }));
vi.mock("../platform", () => ({
  env: hoisted.env,
  platform: { clipboard: { readFiles: hoisted.readFiles } },
}));
vi.mock("../ipc/info", () => ({
  statFile: hoisted.statFile,
  loadFileBlob: hoisted.loadFileBlob,
}));

import {
  capturePasteData,
  formatSpacedChatPaths,
  imageMimeForPath,
  insertPathsIntoTextField,
  resolveClipboardFiles,
} from "./clipboardFiles";

describe("clipboard file resolution for paste (#195)", () => {
  beforeEach(() => {
    hoisted.env.isTauri = true;
    hoisted.env.isElectron = false;
    hoisted.readFiles.mockReset().mockResolvedValue([]);
    hoisted.statFile.mockReset();
    hoisted.loadFileBlob.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("captures WKWebView file-paste payloads whose text/plain is empty", () => {
    const doc = new File(["# Title"], "README.md", { type: "text/markdown" });
    const data = {
      types: ["Files"],
      items: [{ kind: "file", type: "text/markdown", getAsFile: () => doc }],
      files: [doc],
      getData: () => "",
    } as unknown as DataTransfer;
    const captured = capturePasteData(data);
    expect(captured.images).toEqual([]);
    expect(captured.hasNonImageFiles).toBe(true);
    expect(captured.hasFileSignal).toBe(true);
    expect(captured.text).toBe("");
    expect(captured.hasTextType).toBe(false);
  });

  it("returns non-image paths and discards synthesized Finder icon bitmaps", async () => {
    hoisted.readFiles.mockResolvedValue(["/Users/me/Docs/My Report.pdf", "/Users/me/src"]);
    const fakeIcon = new File(["icon"], "image.png", { type: "image/png" });
    const resolved = await resolveClipboardFiles([fakeIcon], { loadMissingImages: true });
    expect(resolved).toEqual({
      allPaths: ["/Users/me/Docs/My Report.pdf", "/Users/me/src"],
      imagePaths: [],
      otherPaths: ["/Users/me/Docs/My Report.pdf", "/Users/me/src"],
      imageFiles: [],
    });
    expect(hoisted.statFile).not.toHaveBeenCalled();
  });

  it("matches copied image files from clipboardData or loads missing ones from disk", async () => {
    const inlinePng = new File(["png"], "shot.png", { type: "image/png" });
    hoisted.readFiles.mockResolvedValue([
      "/Users/me/shot.png",
      "/Users/me/disk.jpg",
      "/Users/me/folder.png",
      "/Users/me/notes.md",
    ]);
    hoisted.statFile.mockImplementation((path: string) =>
      path === "/Users/me/disk.jpg"
        ? Promise.resolve({ size: 4, mtimeMs: 1 })
        : Promise.reject(new Error("is a directory")),
    );
    hoisted.loadFileBlob.mockResolvedValue({
      blob: new Blob(["jpeg"], { type: "image/jpeg" }),
      mtimeMs: 1,
    });

    const resolved = await resolveClipboardFiles([inlinePng], { loadMissingImages: true });
    expect(resolved?.imagePaths).toEqual(["/Users/me/shot.png", "/Users/me/disk.jpg"]);
    expect(resolved?.imageFiles.map((f) => f.name)).toEqual(["shot.png", "disk.jpg"]);
    // A directory or unreadable entry ending in .png falls back to path insertion instead of vanishing.
    expect(resolved?.otherPaths).toEqual(["/Users/me/folder.png", "/Users/me/notes.md"]);
  });

  it("skips OS clipboard file reads outside desktop shells", async () => {
    hoisted.env.isTauri = false;
    hoisted.env.isElectron = false;
    expect(await resolveClipboardFiles()).toBeNull();
    expect(hoisted.readFiles).not.toHaveBeenCalled();
  });

  it("recognizes image extensions and rejects directories or extensionless names", () => {
    expect(imageMimeForPath("/a/b/c.PNG")).toBe("image/png");
    expect(imageMimeForPath("C:\\pics\\photo.jpeg")).toBe("image/jpeg");
    expect(imageMimeForPath("/a/b/dir.png/")).toBeNull();
    expect(imageMimeForPath("/a/b/README.md")).toBeNull();
    expect(imageMimeForPath("/a/b/.png")).toBeNull();
  });

  it("formats and inserts spaced quoted paths into controlled text fields", () => {
    expect(formatSpacedChatPaths("check", 5, 5, ["/a b/c.pdf", "/d.txt"])).toEqual({
      value: 'check "/a b/c.pdf" /d.txt ',
      insert: ' "/a b/c.pdf" /d.txt ',
      caret: 26,
    });
    const textarea = document.createElement("textarea");
    textarea.value = "see world";
    textarea.setSelectionRange(4, 9);
    let observed = "";
    textarea.addEventListener("input", () => {
      observed = textarea.value;
    });
    insertPathsIntoTextField(textarea, ["/Users/me/file.pdf"]);
    expect(observed).toBe("see /Users/me/file.pdf ");
  });
});
