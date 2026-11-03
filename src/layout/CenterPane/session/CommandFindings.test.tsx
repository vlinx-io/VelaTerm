import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../../../platform", () => ({ platform: { opener: { openExternal: vi.fn() } } }));
vi.mock("../../../store/termStore", () => ({ useTermStore: { getState: () => ({ openDocTab }) } }));
const { openDocTab } = vi.hoisted(() => ({ openDocTab: vi.fn() }));

import { parseCommandFindings } from "./CommandFindings";
import { SessionLinkDirectory } from "./links";
import { CommandRow } from "./rows";

beforeEach(() => vi.clearAllMocks());
afterEach(cleanup);

// What `/code-review` printed in the foreground on 2026-11-01, trimmed to two entries.
const reviewOutput = [
  "```json",
  JSON.stringify([
    {
      file: "tools/hn_comment_checker.py",
      line: 135,
      summary: "The navigate result is ignored.",
      failure_scenario: "A slow load marks the comment as hidden for good.",
    },
    { file: "web/src/NurtureView.tsx", summary: "The colours are undefined.", category: "correctness" },
  ], null, 2),
  "```",
].join("\n");

it("draws review findings as a list with a clickable location instead of raw JSON", () => {
  render(<SessionLinkDirectory.Provider value="/work/project">
    <CommandRow text={reviewOutput} />
  </SessionLinkDirectory.Provider>);
  expect(screen.getByText("2 findings")).toBeTruthy();
  expect(screen.getByText("The navigate result is ignored.")).toBeTruthy();
  expect(screen.getByText("A slow load marks the comment as hidden for good.")).toBeTruthy();
  expect(screen.getByText("correctness")).toBeTruthy();
  expect(screen.queryByText(/"failure_scenario"/)).toBeNull();
  fireEvent.click(screen.getByRole("link", { name: "tools/hn_comment_checker.py:135" }));
  expect(openDocTab).toHaveBeenCalledWith("/work/project/tools/hn_comment_checker.py");
});

it("says so when a review found nothing", () => {
  render(<CommandRow text={"```json\n[]\n```"} />);
  expect(screen.getByText("No findings")).toBeTruthy();
});

it.each([
  ["Set effort level to high"],
  ["[]"],
  ["```json\n{\"file\": \"a.ts\", \"summary\": \"x\"}\n```"],
  ["```json\n[{\"name\": \"a\"}]\n```"],
  ["```json\n[{\"file\": \"a.ts\", \"summary\": \"x\"}, 3]\n```"],
  ["```json\n[not json]\n```"],
])("leaves other command output to Markdown: %s", (text) => {
  expect(parseCommandFindings(text)).toBeNull();
});
