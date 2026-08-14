//! Regression tests for the Git tab's Changes section, which previously rendered a fixed design placeholder
//! and so never reflected the working tree. The list must come from the backend and follow the tree as it
//! changes: an edit, a stage, a commit or a branch switch all land here through the poll.

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { gitChangedFiles, getGitStatus } = vi.hoisted(() => ({
  gitChangedFiles: vi.fn(),
  getGitStatus: vi.fn(),
}));

vi.mock("../../ipc/commands", () => ({ gitChangedFiles }));
vi.mock("../../ipc/info", () => ({ getGitStatus }));

import { GitTab } from "./GitTab";

function changed(path: string, status: string, additions = 0, deletions = 0, binary = false) {
  return { path, status, additions, deletions, binary };
}

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  getGitStatus.mockResolvedValue({ isRepo: true, branch: "main", ahead: 0, behind: 0 });
  gitChangedFiles.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.clearAllMocks();
});

describe("GitTab changes", () => {
  /** The counts and paths must be the backend's, not a constant. A placeholder would show its own files here. */
  it("renders the files and line counts reported by the backend", async () => {
    gitChangedFiles.mockResolvedValue([
      changed("src/main.rs", "modified", 3, 1),
      changed("docs/new.md", "added", 12, 0),
    ]);

    render(<GitTab path="/repo" />);

    expect(await screen.findByText("src/main.rs")).toBeDefined();
    expect(screen.getByText("docs/new.md")).toBeDefined();
    expect(screen.getByText("+3")).toBeDefined();
    expect(screen.getByText("−1")).toBeDefined();
    expect(gitChangedFiles).toHaveBeenCalledWith("/repo");
  });

  /** The reported failure: the panel kept its first render forever. A later poll must replace the list. */
  it("picks up changes made after the first render", async () => {
    gitChangedFiles.mockResolvedValue([changed("src/main.rs", "modified", 3, 1)]);
    render(<GitTab path="/repo" />);
    expect(await screen.findByText("src/main.rs")).toBeDefined();

    // The working tree moves on: the edit is committed and a different file is touched.
    gitChangedFiles.mockResolvedValue([changed("README.md", "modified", 1, 1)]);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });

    expect(await screen.findByText("README.md")).toBeDefined();
    expect(screen.queryByText("src/main.rs")).toBeNull();
  });

  /** A clean tree reports nothing, which must read as "no changes" rather than a stale list. */
  it("shows the empty state for a clean working tree", async () => {
    gitChangedFiles.mockResolvedValue([]);
    render(<GitTab path="/repo" />);
    expect(await screen.findByText("No changes")).toBeDefined();
  });

  /** Line counts do not exist for binary files: numstat reports `-` for both sides. */
  it("omits line counts for binary files", async () => {
    gitChangedFiles.mockResolvedValue([changed("logo.png", "added", 0, 0, true)]);
    render(<GitTab path="/repo" />);
    expect(await screen.findByText("logo.png")).toBeDefined();
    expect(screen.queryByText("+0")).toBeNull();
  });

  /** Outside a repository the command rejects; the panel must stay empty rather than surface an error row. */
  it("stays empty when the directory is not a repository", async () => {
    gitChangedFiles.mockRejectedValue(new Error("Not a git repository: /tmp"));
    render(<GitTab path="/tmp" />);
    expect(await screen.findByText("No changes")).toBeDefined();
  });

  /** With no session there is nothing to query, so the backend must not be called at all. */
  it("does not query the backend without a path", async () => {
    render(<GitTab path={null} />);
    await waitFor(() => expect(gitChangedFiles).not.toHaveBeenCalled());
  });
});
