import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

import { platform } from "../../../platform";
import { MessageBubble } from "./rows";

vi.mock("../../../platform/env", () => ({ env: { isTauri: false, isElectron: false, isWeb: true } }));

const TEXT = "Saved to `marketing/reddit/post-body.md`, **checked**.";

afterEach(() => {
  window.getSelection()?.removeAllRanges();
  cleanup();
  vi.restoreAllMocks();
});

function renderAndSelect() {
  const { container } = render(<MessageBubble who="Claude" isUser={false} text={TEXT} />);
  const body = container.querySelector(".sv-msg-body")!;
  const range = document.createRange();
  range.selectNodeContents(body);
  window.getSelection()!.addRange(range);
  return body;
}

it("writes displayed text on copy and keeps the rich flavor", () => {
  const body = renderAndSelect();
  const setData = vi.fn();
  fireEvent.copy(body, { clipboardData: { setData } });
  expect(setData).toHaveBeenCalledWith("text/plain", "Saved to marketing/reddit/post-body.md, checked.");
  expect(setData).toHaveBeenCalledWith("text/html", expect.stringContaining("<code>marketing/reddit/post-body.md</code>"));
});

it("offers the Markdown source from the selection's context menu", () => {
  const writeText = vi.spyOn(platform.clipboard, "writeText").mockResolvedValue();
  const body = renderAndSelect();
  fireEvent.contextMenu(body);
  fireEvent.click(screen.getByText("Copy as Markdown"));
  expect(writeText).toHaveBeenCalledWith(TEXT, { reportFailure: true });
  expect(screen.queryByText("Copy as Markdown")).toBeNull();
});

it("copies through the native copy path from the context menu", () => {
  const execCommand = vi.fn(() => true);
  Object.defineProperty(document, "execCommand", { value: execCommand, configurable: true });
  const body = renderAndSelect();
  fireEvent.contextMenu(body);
  fireEvent.click(screen.getByText("Copy"));
  expect(execCommand).toHaveBeenCalledWith("copy");
  Reflect.deleteProperty(document, "execCommand");
});

it("shows no menu without a selection", () => {
  const { container } = render(<MessageBubble who="Claude" isUser={false} text={TEXT} />);
  fireEvent.contextMenu(container.querySelector(".sv-msg-body")!);
  expect(screen.queryByText("Copy as Markdown")).toBeNull();
});
