import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ChangesView } from "./ChangesView";
import type { ChangesTarget, DocTab } from "../../store/docTab";

const mocks = vi.hoisted(() => ({ files: vi.fn(), diff: vi.fn(), select: vi.fn(), refresh: vi.fn() }));
vi.mock("../../ipc/commands", () => ({ gitChangedFiles: mocks.files, gitFileDiff: mocks.diff,
  gitCommitFiles: mocks.files, gitCommitFileDiff: mocks.diff }));
vi.mock("../../i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("../../store/termStore", () => ({ useTermStore: (select: (s: unknown) => unknown) =>
  select({ selectChangesFile: mocks.select, refreshDocTab: mocks.refresh }) }));

const staged = { path: "中文文件.txt", status: "modified", index: "modified", worktree: "modified",
  additions: 3, deletions: 0, stagedAdditions: 1, stagedDeletions: 0, unstagedAdditions: 2, unstagedDeletions: 0 };
const tab = (changes: Partial<ChangesTarget> = {}) => ({ id: "doc-1", path: "/repo", title: "repo", kind: "changes",
  mode: "source", dirty: false, pendingClose: false, reloadNonce: 0,
  changes: { cwd: "/repo", side: "staged", commit: null, base: null, path: null, ...changes } } as DocTab);
afterEach(cleanup);
beforeEach(() => {
  vi.clearAllMocks();
  mocks.files.mockResolvedValue({ files: [staged, { ...staged, path: "untracked.txt", index: "", worktree: "untracked" }], truncated: false });
  mocks.diff.mockResolvedValue({ path: staged.path, original: "", modified: "", binary: true });
});

it("shows only the requested area, selects its first file, and sends its comparison to the backend", async () => {
  const { container } = render(<ChangesView tab={tab()} hidden={false} />);
  await waitFor(() => expect(mocks.diff).toHaveBeenCalledWith("/repo", staged.path, "staged"));
  expect(mocks.select).toHaveBeenCalledWith("doc-1", staged.path);
  expect(screen.queryByText("untracked.txt")).toBeNull();
  expect(screen.getByText("changes.compareStaged")).toBeTruthy();
  expect(screen.getByText("+1")).toBeTruthy();
  expect(container.querySelector(".changes-file-path")?.textContent).toBe(staged.path);
});

it("records a picked file on the tab and shows the file the tab names", async () => {
  const { rerender } = render(<ChangesView tab={tab({ side: "all", path: staged.path })} hidden={false} />);
  await screen.findByText("untracked.txt");
  expect(mocks.select).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("untracked.txt").closest("button")!);
  expect(mocks.select).toHaveBeenCalledWith("doc-1", "untracked.txt");
  rerender(<ChangesView tab={tab({ side: "all", path: "untracked.txt" })} hidden={false} />);
  await waitFor(() => expect(mocks.diff).toHaveBeenLastCalledWith("/repo", "untracked.txt", "all"));
  fireEvent.click(screen.getByText("changes.refresh"));
  expect(mocks.refresh).toHaveBeenCalledWith("doc-1");
});

it("explains the text size limit without calling the file binary", async () => {
  mocks.diff.mockResolvedValue({ path: staged.path, original: "", modified: "", binary: false, tooLarge: true });
  render(<ChangesView tab={tab({ path: staged.path })} hidden={false} />);
  expect(await screen.findByText("changes.tooLarge")).toBeTruthy();
  expect(screen.queryByText("changes.binary")).toBeNull();
});
