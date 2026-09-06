//! Regression coverage for worktrees entered by a running agent after the session was created. These sessions
//! have no persisted worktreePath, so the icon must probe the process's live cwd instead of the database cwd.

import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Session } from "../../types";

const mocks = vi.hoisted(() => ({
  getSessionCwd: vi.fn(),
  getGitStatus: vi.fn(),
}));

vi.mock("../../ipc/commands", () => ({ getSessionCwd: mocks.getSessionCwd }));
vi.mock("../../ipc/info", () => ({ getGitStatus: mocks.getGitStatus }));
vi.mock("../../i18n", () => ({
  dateLocale: () => "en-US",
  useT: () => (key: string) => key,
}));
vi.mock("../../store/termStore", () => ({
  useTermStore: (selector: (state: unknown) => unknown) =>
    selector({
      agentPresets: [],
      runtimes: { child: { status: "running", agentState: "working" } },
    }),
}));
vi.mock("../agentPresetIcon", () => ({
  sessionIconEl: () => <span data-testid="kind-icon" />,
}));

import { SessionKindIcon } from "./sessionMeta";

afterEach(cleanup);

describe("SessionKindIcon worktree badge", () => {
  it("detects a worktree from the running process cwd when metadata is absent", async () => {
    const path = "/repo/.claude/worktrees/task";
    mocks.getSessionCwd.mockResolvedValue(path);
    mocks.getGitStatus.mockResolvedValue({
      isRepo: true,
      branch: "task",
      isWorktree: true,
      worktreePath: path,
    });
    const session = {
      id: "child",
      projectId: "collection",
      name: "task",
      kind: "claude",
      parentSessionId: "parent",
      cwd: null,
      worktreePath: null,
    } as unknown as Session;

    const { container } = render(<SessionKindIcon session={session} />);

    await waitFor(() => expect(mocks.getSessionCwd).toHaveBeenCalledWith("child"));
    await waitFor(() => expect(mocks.getGitStatus).toHaveBeenCalledWith(path));
    await waitFor(() => expect(container.querySelector(".wt-badge")).not.toBeNull());
  });
});
