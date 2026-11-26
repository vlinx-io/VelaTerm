//! Image-paste regression coverage for the source/text editor: Markdown inserts image syntax, text never enters
//! the upload path, and unsaved drafts persist through embedded data URLs rather than temporary images deleted on exit.

import { createRef } from "react";
import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  uploadDocImage: vi.fn(),
  env: { isTauri: false },
  readFiles: vi.fn<() => Promise<string[]>>(),
  imageFromNativeClipboard: vi.fn(),
}));

vi.mock("../../../ipc/transport", () => ({
  uploadDocImage: mocks.uploadDocImage,
}));
vi.mock("../../../platform/env", () => ({
  env: mocks.env,
}));
vi.mock("../../../platform", () => ({
  env: mocks.env,
  platform: { clipboard: { readFiles: mocks.readFiles } },
}));
vi.mock("../../../terminal/imageInput", () => ({
  extOf: () => "png",
  imageFromNativeClipboard: mocks.imageFromNativeClipboard,
  imagesFromClipboard: (data: DataTransfer | null) =>
    ((data as DataTransfer & { imageFiles?: File[] } | null)?.imageFiles ?? []),
}));

import { SourceEditor, type SourceHandle } from "./SourceEditor";

function imageFile(): File {
  const file = new File([new Uint8Array([1, 2, 3])], "shot.png", { type: "image/png" });
  // jsdom's File implementation does not always provide arrayBuffer; add it explicitly for saved-document upload paths.
  Object.defineProperty(file, "arrayBuffer", {
    value: async () => new Uint8Array([1, 2, 3]).buffer,
  });
  return file;
}

function dispatchPaste(container: HTMLElement, data: DataTransfer): Event {
  const target = container.querySelector<HTMLElement>(".cm-content");
  if (!target) throw new Error("CodeMirror content did not mount");
  const event = new Event("paste", { bubbles: true, cancelable: true });
  Object.defineProperty(event, "clipboardData", { value: data });
  act(() => target.dispatchEvent(event));
  return event;
}

describe("SourceEditor image paste", () => {
  beforeEach(() => {
    mocks.uploadDocImage.mockReset();
    mocks.env.isTauri = false;
    mocks.readFiles.mockReset().mockResolvedValue([]);
    mocks.imageFromNativeClipboard.mockReset();
  });

  it("a saved Markdown file persists the image and inserts standard image syntax", async () => {
    mocks.uploadDocImage.mockResolvedValue("assets/shot.png");
    const ref = createRef<SourceHandle>();
    const { container } = render(
      <SourceEditor
        ref={ref}
        defaultValue="body text"
        path="/tmp/note.md"
        kind="markdown"
        onEdited={() => {}}
      />,
    );
    await waitFor(() => expect(ref.current?.getText()).toBe("body text"));

    const file = imageFile();
    const event = dispatchPaste(container, {
      imageFiles: [file],
      types: ["Files"],
    } as unknown as DataTransfer);

    expect(event.defaultPrevented).toBe(true);
    await waitFor(() => expect(ref.current?.getText()).toBe("![](assets/shot.png)body text"));
    expect(mocks.uploadDocImage).toHaveBeenCalledWith(
      new Uint8Array([1, 2, 3]),
      "png",
      "/tmp/note.md",
    );
  });

  it("a plain-text clipboard does not trigger an image upload", async () => {
    const ref = createRef<SourceHandle>();
    const { container } = render(
      <SourceEditor
        ref={ref}
        defaultValue=""
        path="/tmp/note.md"
        kind="markdown"
        onEdited={() => {}}
      />,
    );
    await waitFor(() => expect(ref.current?.getText()).toBe(""));

    dispatchPaste(container, {
      imageFiles: [],
      types: ["text/plain"],
      getData: () => "hello",
    } as unknown as DataTransfer);

    expect(mocks.uploadDocImage).not.toHaveBeenCalled();
  });

  it("an unsaved draft embeds the image data instead of creating a temporary attachment", async () => {
    const ref = createRef<SourceHandle>();
    const { container } = render(
      <SourceEditor
        ref={ref}
        defaultValue=""
        path=""
        kind="code"
        onEdited={() => {}}
      />,
    );
    await waitFor(() => expect(ref.current?.getText()).toBe(""));

    dispatchPaste(container, {
      imageFiles: [imageFile()],
      types: ["Files"],
    } as unknown as DataTransfer);

    await waitFor(() =>
      expect(ref.current?.getText()).toMatch(/^!\[\]\(data:image\/png;base64,/),
    );
    expect(mocks.uploadDocImage).not.toHaveBeenCalled();
  });

  it("inserts the path of a non-image file copied in Finder instead of reading the file-icon bitmap (#195)", async () => {
    mocks.env.isTauri = true;
    mocks.readFiles.mockResolvedValue(["/Users/me/Docs/spec.pdf"]);
    const ref = createRef<SourceHandle>();
    const { container } = render(
      <SourceEditor
        ref={ref}
        defaultValue=""
        path="/tmp/note.md"
        kind="markdown"
        onEdited={() => {}}
      />,
    );
    await waitFor(() => expect(ref.current?.getText()).toBe(""));

    const pdf = new File(["pdf"], "spec.pdf", { type: "application/pdf" });
    const event = dispatchPaste(container, {
      imageFiles: [],
      items: [{ kind: "file", type: "application/pdf", getAsFile: () => pdf }],
      files: [pdf],
      types: ["Files"],
      getData: () => "",
    } as unknown as DataTransfer);

    expect(event.defaultPrevented).toBe(true);
    await waitFor(() => expect(ref.current?.getText()).toBe("/Users/me/Docs/spec.pdf"));
    expect(mocks.imageFromNativeClipboard).not.toHaveBeenCalled();
    expect(mocks.uploadDocImage).not.toHaveBeenCalled();
  });
});

describe("SourceEditor companion updates", () => {
  it("applies visual edits without feedback, while source replacements still report edits", async () => {
    const ref = createRef<SourceHandle>();
    const onEdited = vi.fn();
    const { container } = render(
      <SourceEditor ref={ref} defaultValue="original" path="/tmp/compare.md" kind="markdown" onEdited={onEdited} />,
    );
    await waitFor(() => expect(ref.current?.getText()).toBe("original"));
    const editor = container.querySelector(".cm-editor");
    act(() => ref.current?.setText("# updated\n\nExact spacing  "));
    expect(ref.current?.getText()).toBe("# updated\n\nExact spacing  ");
    expect(onEdited).not.toHaveBeenCalled();
    expect(container.querySelector(".cm-editor")).toBe(editor);
    act(() => {
      ref.current?.search.apply({ query: "updated", caseSensitive: true });
      ref.current?.search.replace("source edit");
    });
    expect(ref.current?.getText()).toBe("# source edit\n\nExact spacing  ");
    expect(onEdited).toHaveBeenCalledOnce();
  });
});
