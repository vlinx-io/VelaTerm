import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";

import { Markdown } from "./markdown";
import { createMessageSelectionClipboardContent } from "./selectionCopy";

afterEach(() => {
  window.getSelection()?.removeAllRanges();
  cleanup();
});

function copyAll(text: string) {
  const { container } = render(<div className="sv-msg-body"><Markdown text={text} /></div>);
  const body = container.firstElementChild!;
  const range = document.createRange();
  range.selectNodeContents(body);
  const selection = window.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range);
  return createMessageSelectionClipboardContent(selection, body);
}

it("copies inline code, emphasis, and links as displayed text", () => {
  const content = copyAll("Saved to `marketing/reddit/post-body.md`, **checked** in _one_ [place](https://example.com/docs).");
  expect(content?.plainText).toBe("Saved to marketing/reddit/post-body.md, checked in one place.");
  expect(content?.markdown).toBe("Saved to `marketing/reddit/post-body.md`, **checked** in _one_ [place](https://example.com/docs).");
});

it("does not escape characters that Markdown would treat as syntax", () => {
  const content = copyAll("Run `rm -rf build_*` then edit snake_case [1] and 2 * 3.");
  expect(content?.plainText).toBe("Run rm -rf build_* then edit snake_case [1] and 2 * 3.");
});

it("keeps a selection that starts in prose and ends inside inline code free of backticks", () => {
  const { container } = render(<div className="sv-msg-body"><Markdown text="Open `docs/readme.md` now." /></div>);
  const body = container.firstElementChild!;
  const code = body.querySelector("code")!;
  const range = document.createRange();
  range.setStart(body.querySelector(".sv-p")!.firstChild!.firstChild!, 0);
  range.setEnd(code.firstChild!, 4);
  window.getSelection()!.addRange(range);
  expect(createMessageSelectionClipboardContent(window.getSelection(), body)?.plainText).toBe("Open docs");
});

it("copies headings, quotes, and code blocks without their Markdown markers", () => {
  const content = copyAll("## Steps\n\n> Run this first.\n\n```sh\n  pnpm install\npnpm test\n```\n\nDone.");
  expect(content?.plainText).toBe("Steps\n\nRun this first.\n\n  pnpm install\npnpm test\n\nDone.");
  expect(content?.markdown).toContain("> Run this first.\n\n```sh\n  pnpm install\npnpm test\n```");
});

it("copies lists with the markers the view displays", () => {
  expect(copyAll("- first\n- second\n  - nested")?.plainText).toBe("• first\n• second\n  • nested");
  expect(copyAll("3. third\n4. fourth")?.plainText).toBe("3. third\n4. fourth");
  const tasks = copyAll("- [x] done\n- [ ] open");
  expect(tasks?.plainText).toBe("☑ done\n☐ open");
  expect(tasks?.markdown).toBe("- [x] done\n- [ ] open");
});

it("copies tables as tab-separated rows", () => {
  const content = copyAll("| Name | Port |\n| --- | --- |\n| web | `18733` |\n| api | 18732 |");
  expect(content?.plainText).toBe("Name\tPort\nweb\t18733\napi\t18732");
  expect(content?.markdown).toBe("| Name | Port |\n| --- | --- |\n| web | `18733` |\n| api | 18732 |");
});
