import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import { platform } from "../../../platform";
import { useTextFieldMenu } from "./useTextFieldMenu";

const hoisted = vi.hoisted(() => ({
  env: { isTauri: false, isElectron: false, isWeb: true, hasNativeHost: false },
  imageFromNativeClipboard: vi.fn(),
  imagesFromClipboard: vi.fn(() => []),
}));
vi.mock("../../../platform/env", () => ({ env: hoisted.env }));
vi.mock("../../../terminal/imageInput", () => ({
  imageFromNativeClipboard: hoisted.imageFromNativeClipboard,
  imagesFromClipboard: hoisted.imagesFromClipboard,
}));

function Field({ initial = "hello world", onImage, onCopyFailed = () => {}, type }: {
  initial?: string; onImage?: (file: File) => void; onCopyFailed?: (message: string) => void; type?: string;
}) {
  const [value, setValue] = useState(initial);
  const { onContextMenu, textFieldMenu } = useTextFieldMenu({ onImage, onCopyFailed });
  return <div onContextMenu={onContextMenu}>
    {textFieldMenu}
    {type ? <input aria-label="field" type={type} value={value} onChange={e => setValue(e.target.value)} />
      : <textarea aria-label="field" value={value} onChange={e => setValue(e.target.value)} />}
    <span>outside</span>
  </div>;
}

function openMenu(start = 0, end = 5) {
  const field = screen.getByLabelText("field") as HTMLTextAreaElement;
  field.setSelectionRange(start, end);
  fireEvent.contextMenu(field);
  return field;
}
const item = (name: string) => screen.getByRole("menuitem", { name });

beforeEach(() => {
  hoisted.env.isTauri = false;
  hoisted.imageFromNativeClipboard.mockReset();
  Object.defineProperty(navigator, "clipboard", { value: { readText: vi.fn() }, configurable: true });
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  Reflect.deleteProperty(navigator, "clipboard");
});

it("copies the selection without changing the field", async () => {
  const writeText = vi.spyOn(platform.clipboard, "writeText").mockResolvedValue();
  render(<Field />);
  const field = openMenu();
  fireEvent.click(item("Copy"));
  expect(writeText).toHaveBeenCalledWith("hello", { reportFailure: true });
  expect(field.value).toBe("hello world");
});

it("removes cut text once the clipboard has it", async () => {
  vi.spyOn(platform.clipboard, "writeText").mockResolvedValue();
  render(<Field />);
  const field = openMenu();
  fireEvent.click(item("Cut"));
  await waitFor(() => expect(field.value).toBe(" world"));
});

it("keeps the text and reports the failure when a cut cannot reach the clipboard", async () => {
  vi.spyOn(platform.clipboard, "writeText").mockRejectedValue(new Error("denied"));
  const onCopyFailed = vi.fn();
  render(<Field onCopyFailed={onCopyFailed} />);
  const field = openMenu();
  fireEvent.click(item("Cut"));
  await waitFor(() => expect(onCopyFailed).toHaveBeenCalledWith("Could not copy. Please try again."));
  expect(field.value).toBe("hello world");
});

it("does not delete text that changed while the cut was writing", async () => {
  let finish = () => {};
  vi.spyOn(platform.clipboard, "writeText").mockReturnValue(new Promise<void>(resolve => { finish = resolve; }));
  render(<Field />);
  const field = openMenu();
  fireEvent.click(item("Cut"));
  fireEvent.change(field, { target: { value: "other text" } });
  finish();
  await Promise.resolve();
  await Promise.resolve();
  expect(field.value).toBe("other text");
});

it("pastes over the selection and updates the controlled value", async () => {
  vi.spyOn(platform.clipboard, "readText").mockResolvedValue("goodbye");
  render(<Field />);
  const field = openMenu();
  fireEvent.click(item("Paste"));
  await waitFor(() => expect(field.value).toBe("goodbye world"));
});

it("hands a clipboard image to fields that accept attachments", async () => {
  hoisted.env.isTauri = true;
  const image = new File(["x"], "shot.png", { type: "image/png" });
  hoisted.imageFromNativeClipboard.mockResolvedValue(image);
  const readText = vi.spyOn(platform.clipboard, "readText").mockResolvedValue("ignored");
  const onImage = vi.fn();
  render(<Field onImage={onImage} />);
  const field = openMenu();
  fireEvent.click(item("Paste"));
  await waitFor(() => expect(onImage).toHaveBeenCalledWith(image));
  expect(readText).not.toHaveBeenCalled();
  expect(field.value).toBe("hello world");
});

it("pastes copied OS file paths instead of Finder's file-icon bitmap (#195)", async () => {
  hoisted.env.isTauri = true;
  vi.spyOn(platform.clipboard, "readFiles").mockResolvedValue(["/Users/me/My Report.pdf"]);
  const onImage = vi.fn();
  render(<Field onImage={onImage} />);
  const field = openMenu();
  fireEvent.click(item("Paste"));
  await waitFor(() => expect(field.value).toBe('"/Users/me/My Report.pdf" world'));
  expect(hoisted.imageFromNativeClipboard).not.toHaveBeenCalled();
  expect(onImage).not.toHaveBeenCalled();
});

it("disables Paste but keeps its shortcut visible when the page cannot read the clipboard", () => {
  Reflect.deleteProperty(navigator, "clipboard");
  render(<Field />);
  openMenu();
  expect(item("Paste").getAttribute("aria-disabled")).toBe("true");
  expect(item("Paste").textContent).toMatch(/(⌘ |Ctrl\+)V$/);
});

it("shows each action's keyboard shortcut", () => {
  render(<Field />);
  openMenu();
  for (const [name, key] of [["Cut", "X"], ["Copy", "C"], ["Paste", "V"], ["Select All", "A"]]) {
    expect(item(name).textContent).toMatch(new RegExp(`^${name}(⌘ |Ctrl\\+)${key}$`));
  }
});

it("selects everything", () => {
  render(<Field />);
  const field = openMenu(0, 0);
  fireEvent.click(item("Select All"));
  expect([field.selectionStart, field.selectionEnd]).toEqual([0, 11]);
});

it("disables Cut and Copy without a selection", () => {
  render(<Field />);
  openMenu(3, 3);
  expect(item("Cut").getAttribute("aria-disabled")).toBe("true");
  expect(item("Copy").getAttribute("aria-disabled")).toBe("true");
});

it("opens only for text fields", () => {
  render(<Field type="password" />);
  fireEvent.contextMenu(screen.getByLabelText("field"));
  expect(screen.queryByRole("menu")).toBeNull();
  fireEvent.contextMenu(screen.getByText("outside"));
  expect(screen.queryByRole("menu")).toBeNull();
});
