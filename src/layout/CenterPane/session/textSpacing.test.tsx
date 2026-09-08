import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";

import { Markdown } from "./markdown";
import { createMessageSelectionClipboardContent } from "./selectionCopy";
import { ToolBody } from "./toolCards";

afterEach(() => {
  window.getSelection()?.removeAllRanges();
  cleanup();
});

it("preserves mixed text and emoji with default character spacing", () => {
  const text = "中文 English，路径 /tmp/中文.png；👩‍💻 𠮷。";
  const { container } = render(<div>{text}</div>);
  expect(container.textContent).toBe(text);
  expect(container.querySelector(".sv-cjk")).toBeNull();
});

it("updates streamed text and newly completed Markdown without leaving stale spans", () => {
  const { container, rerender } = render(<Markdown text="等待" />);
  rerender(<Markdown text={"等待确认。\n\n| 内容 | 状态 |\n| --- | --- |\n| 中文正文 | 正常 |\n\n`/tmp/中文.png`"} />);
  expect(container.querySelector(".sv-p")?.textContent).toBe("等待确认。");
  expect(container.querySelector("td")?.textContent).toBe("中文正文");
  expect(container.querySelector(".sv-codespan")?.textContent).toBe("/tmp/中文.png");
  expect(container.querySelector(".sv-cjk")).toBeNull();
});

it("copies the original Markdown without adding spacing characters", () => {
  const text = "中文 English，保持间距。\n\n**重点中文**";
  const { container } = render(<div className="sv-msg-body"><Markdown text={text} /></div>);
  const body = container.firstElementChild!;
  const range = document.createRange();
  range.selectNodeContents(body);
  const selection = window.getSelection()!;
  selection.addRange(range);
  expect(createMessageSelectionClipboardContent(selection, body)?.plainText).toBe(text);
});

it("preserves highlighted code and copies its exact text", () => {
  const code = '// 检查中文\nconst path = "/tmp/中文.png";';
  const { container } = render(<div className="sv-msg-body"><Markdown text={`\`\`\`ts\n${code}\n\`\`\``} /></div>);
  const pre = container.querySelector("pre")!;
  expect(pre.textContent).toBe(code);
  expect(pre.querySelector(".c-com")?.textContent).toBe("// 检查中文");
  expect(pre.querySelector(".c-key")?.textContent).toBe("const");
  const range = document.createRange();
  range.selectNodeContents(pre);
  const selection = window.getSelection()!;
  selection.addRange(range);
  expect(createMessageSelectionClipboardContent(selection, container.firstElementChild!)?.plainText).toBe(code);
});

it("preserves tool input, output, and diff text with default spacing", () => {
  const { container, rerender } = render(<ToolBody tool="Bash" input={{ command: "echo 中文" }} output="执行成功。" isError={false} />);
  expect(container.querySelector(".sv-code pre")?.textContent).toBe("echo 中文");
  expect(container.querySelector(".sv-out pre")?.textContent).toBe("执行成功。");
  rerender(<ToolBody tool="Edit" input={{ old_string: "旧内容", new_string: "新内容" }} isError={false} />);
  expect(container.querySelector(".sv-diff-del .sv-diff-text")?.textContent).toBe("旧内容");
  expect(container.querySelector(".sv-diff-add .sv-diff-text")?.textContent).toBe("新内容");
});

it("keeps long plain-text fallback content grouped rather than creating an element per character", () => {
  const text = "中文".repeat(100_001);
  const { container } = render(<Markdown text={text} />);
  expect(container.textContent).toBe(text);
  expect(container.querySelectorAll(".sv-cjk")).toHaveLength(0);
});
