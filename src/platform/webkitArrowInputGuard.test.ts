import { afterEach, describe, expect, it, vi } from "vitest";
import { installWebkitArrowInputGuard } from "./webkitArrowInputGuard";

let remove = () => {};

afterEach(() => {
  remove();
  remove = () => {};
  document.body.replaceChildren();
});

function fireBeforeInput(target: EventTarget, init: InputEventInit) {
  const event = new InputEvent("beforeinput", { bubbles: true, cancelable: true, ...init });
  return target.dispatchEvent(event);
}

describe("installWebkitArrowInputGuard", () => {
  it("blocks WKWebView arrow control insertion in textarea and input fields", () => {
    remove = installWebkitArrowInputGuard();
    const textarea = document.createElement("textarea");
    const input = document.createElement("input");
    document.body.append(textarea, input);

    for (const el of [textarea, input]) {
      for (const data of ["\u001c", "\u001d", "\u001e", "\u001f", "\u001d\u001d", "\uf700", "\uf701", "\uf702", "\uf703"]) {
        expect(fireBeforeInput(el, { inputType: "insertText", data })).toBe(false);
      }
    }
  });

  it("blocks arrow control insertion inside contenteditable hosts and stops propagation to editor listeners", () => {
    remove = installWebkitArrowInputGuard();
    const editor = document.createElement("div");
    editor.setAttribute("contenteditable", "true");
    const line = document.createElement("span");
    const readonlyIsland = document.createElement("span");
    readonlyIsland.setAttribute("contenteditable", "false");
    editor.append(line, readonlyIsland);
    document.body.append(editor);

    const editorListener = vi.fn();
    editor.addEventListener("beforeinput", editorListener, true);

    expect(fireBeforeInput(line, { inputType: "insertText", data: "\u001d" })).toBe(false);
    expect(editorListener).not.toHaveBeenCalled();

    // Non-editable islands inside contenteditable are left alone.
    expect(fireBeforeInput(readonlyIsland, { inputType: "insertText", data: "\u001d" })).toBe(true);
  });

  it("leaves xterm helper textareas and non-editable elements untouched", () => {
    remove = installWebkitArrowInputGuard();
    const xtermInput = document.createElement("textarea");
    xtermInput.className = "xterm-helper-textarea";
    const plainDiv = document.createElement("div");
    document.body.append(xtermInput, plainDiv);

    expect(fireBeforeInput(xtermInput, { inputType: "insertText", data: "\u001c" })).toBe(true);
    expect(fireBeforeInput(plainDiv, { inputType: "insertText", data: "\u001c" })).toBe(true);
  });

  it("allows normal typing, composition, line breaks, deletion, paste, and arrow keydown events", () => {
    remove = installWebkitArrowInputGuard();
    const textarea = document.createElement("textarea");
    document.body.append(textarea);

    for (const [inputType, data] of [
      ["insertText", "中文"],
      ["insertText", "ssh -p 12222"],
      ["insertText", "\t"],
      ["insertLineBreak", null],
      ["insertCompositionText", "zuowan"],
      ["insertFromPaste", "中文\n\u001c"],
      ["deleteContentBackward", null],
    ] as const) {
      expect(fireBeforeInput(textarea, {
        inputType,
        data,
        isComposing: inputType === "insertCompositionText",
      })).toBe(true);
    }

    const arrow = new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true });
    expect(textarea.dispatchEvent(arrow)).toBe(true);
  });

  it("stops intercepting after cleanup is called", () => {
    remove = installWebkitArrowInputGuard();
    const textarea = document.createElement("textarea");
    document.body.append(textarea);

    expect(fireBeforeInput(textarea, { inputType: "insertText", data: "\u001d" })).toBe(false);
    remove();
    expect(fireBeforeInput(textarea, { inputType: "insertText", data: "\u001d" })).toBe(true);
  });
});
