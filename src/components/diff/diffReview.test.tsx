import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import en from "../../i18n/locales/en";
import type { ChangesTarget, DocTab } from "../../store/docTab";
import type { Project, Session, SessionRuntime } from "../../types";

const mocks = vi.hoisted(() => ({
  files: vi.fn(),
  diff: vi.fn(),
  select: vi.fn(),
  refresh: vi.fn(),
  openSession: vi.fn(),
  chatStart: vi.fn(),
  chatSend: vi.fn(),
  ptyWrite: vi.fn(),
  copyText: vi.fn(),
  storeState: {
    sessions: [] as Session[],
    projects: [] as Project[],
    runtimes: {} as Record<string, SessionRuntime>,
    activeSessionId: null as string | null,
  },
}));

vi.mock("../../ipc/commands", () => ({
  gitChangedFiles: mocks.files,
  gitFileDiff: mocks.diff,
  gitCommitFiles: mocks.files,
  gitCommitFileDiff: mocks.diff,
  gitRangeFiles: mocks.files,
  gitRangeFileDiff: mocks.diff,
  ptyWrite: mocks.ptyWrite,
}));
vi.mock("../../ipc/chat", () => ({
  chatStart: mocks.chatStart,
  chatSend: mocks.chatSend,
  resolveChatImage: vi.fn(),
}));
vi.mock("../../ipc/info", () => ({
  copyText: mocks.copyText,
}));
vi.mock("../../hooks/nativeViewSuspend", () => ({ useSuspendNativeViews: vi.fn() }));
vi.mock("../../store/termStore", () => {
  const getState = () => ({
    ...mocks.storeState,
    selectChangesFile: mocks.select,
    refreshDocTab: mocks.refresh,
    openSession: mocks.openSession,
  });
  const hook = (selector: (s: ReturnType<typeof getState>) => unknown) => selector(getState());
  hook.getState = getState;
  hook.subscribe = vi.fn(() => () => {});
  return { useTermStore: hook };
});

import { ChangesView } from "./ChangesView";
import {
  extractLineSnippet,
  formatReviewPrompt,
  resolveReviewCandidates,
  reviewScopeKey,
  useDiffReviewStore,
} from "./diffReviewStore";
import { t } from "../../i18n";

function makeSession(overrides: Partial<Session> & { id: string }): Session {
  return {
    projectId: "proj-1",
    groupId: null,
    parentSessionId: null,
    name: "Implement auth",
    kind: "claude",
    engine: "chat",
    cwd: "/repo",
    worktreePath: null,
    worktreeBaseRef: null,
    order: 0,
    collapsed: false,
    archivedAt: null,
    ...overrides,
  } as Session;
}

const tab = (changes: Partial<ChangesTarget> = {}): DocTab =>
  ({
    id: "doc-rev",
    path: "/repo",
    title: "repo",
    kind: "changes",
    mode: "source",
    dirty: false,
    pendingClose: false,
    reloadNonce: 0,
    changes: { cwd: "/repo", side: "all", commit: null, base: null, path: "src/a.ts", ...changes },
  }) as DocTab;

afterEach(() => {
  cleanup();
  useDiffReviewStore.setState({ scopes: {}, lastActiveSessionId: null });
});

beforeEach(() => {
  vi.clearAllMocks();
  mocks.storeState.projects = [{ id: "proj-1", name: "Repo", rootPath: "/repo", sortOrder: 0, createdAt: 0, collapsed: false } as unknown as Project];
  mocks.storeState.sessions = [makeSession({ id: "sess-claude", kind: "claude", engine: "chat", cwd: "/repo" })];
  mocks.storeState.runtimes = {};
  mocks.storeState.activeSessionId = null;
  mocks.chatStart.mockResolvedValue(undefined);
  mocks.chatSend.mockResolvedValue("sent");
  mocks.copyText.mockResolvedValue(undefined);
});

describe("diffReviewStore helpers", () => {
  it("shares scope keys for identical working-tree comparisons and escapes nested code fences", () => {
    expect(reviewScopeKey({ cwd: "/repo/", side: "all", commit: null, base: null })).toBe(
      reviewScopeKey({ cwd: "/repo", side: "all", commit: null, base: null }),
    );

    const snippet = extractLineSnippet("one\n```ts\nconst x = 1;\n```\nfive", 2, 4);
    const prompt = formatReviewPrompt(
      [
        {
          id: "c1",
          path: "docs/guide.md",
          side: "modified",
          startLine: 2,
          endLine: 4,
          snippet,
          body: "Update example variable name.",
        },
      ],
      t,
    );
    expect(prompt).toContain(en["changes.review.promptHeader"]);
    expect(prompt).toContain("````md\n```ts\nconst x = 1;\n```\n````");
    expect(prompt).toContain("Comment: Update example variable name.");
  });

  it("prioritizes exact worktree matches and skips stopped TUI sessions", () => {
    const mainChat = makeSession({ id: "s-main", kind: "claude", engine: "chat", cwd: "/repo" });
    const wtChat = makeSession({
      id: "s-wt",
      kind: "codex",
      engine: "chat",
      cwd: "/repo/.vlx-worktrees/feat",
      worktreePath: "/repo/.vlx-worktrees/feat",
    });
    const stoppedTui = makeSession({
      id: "s-tui",
      kind: "opencode",
      engine: "tui",
      cwd: "/repo/.vlx-worktrees/feat",
      worktreePath: "/repo/.vlx-worktrees/feat",
    });
    const candidates = resolveReviewCandidates(
      "/repo/.vlx-worktrees/feat",
      [mainChat, wtChat, stoppedTui],
      mocks.storeState.projects,
      {},
      "s-main",
    );
    expect(candidates.map((c) => c.session.id)).toEqual(["s-wt", "s-main"]);
  });
});

describe("ChangesView inline review comments flow", () => {
  it("guards IME composition on Enter, batches comments across files, and sends them to the session agent", async () => {
    mocks.files.mockResolvedValue({
      files: [
        { path: "src/a.ts", status: "modified", index: "", worktree: "modified", additions: 2, deletions: 1, binary: false },
        { path: "src/b.ts", status: "modified", index: "", worktree: "modified", additions: 1, deletions: 0, binary: false },
      ],
      truncated: false,
    });
    mocks.diff.mockImplementation(async (_cwd: string, path: string) => ({
      path,
      original: "const oldVal = 1;\n",
      modified: path === "src/a.ts" ? "const newVal = 2;\nreturn newVal;\n" : "export const ok = true;\n",
      binary: false,
    }));

    const { rerender } = render(<ChangesView tab={tab({ path: "src/a.ts" })} hidden={false} />);
    await waitFor(() => expect(mocks.diff).toHaveBeenCalledWith("/repo", "src/a.ts", "all"));

    fireEvent.click(await screen.findByRole("button", { name: en["changes.review.addFile"] }));
    const textarea = (await screen.findByLabelText(/Review comment/)) as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: "Rename newVal to count" } });

    // IME composition Enter (keyCode 229) must not submit early.
    fireEvent.keyDown(textarea, { key: "Enter", keyCode: 229, isComposing: true });
    expect(screen.queryByText("1 review comment")).toBeNull();

    // Plain Enter saves the comment.
    fireEvent.keyDown(textarea, { key: "Enter", keyCode: 13 });
    expect(await screen.findByText("1 review comment")).toBeTruthy();

    // Switch to the second file and add a second comment.
    rerender(<ChangesView tab={tab({ path: "src/b.ts" })} hidden={false} />);
    await waitFor(() => expect(mocks.diff).toHaveBeenCalledWith("/repo", "src/b.ts", "all"));
    fireEvent.click(await screen.findByRole("button", { name: en["changes.review.addFile"] }));
    const textareaB = (await screen.findByLabelText(/Review comment/)) as HTMLTextAreaElement;
    fireEvent.change(textareaB, { target: { value: "Add explicit return type" } });
    fireEvent.click(screen.getByRole("button", { name: en["changes.review.save"] }));

    expect(await screen.findByText("2 review comments across 2 files")).toBeTruthy();

    // Send the batched review to Claude.
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Send to Claude" }));
    });

    expect(mocks.openSession).toHaveBeenCalledWith("sess-claude");
    expect(mocks.chatStart).toHaveBeenCalledWith("sess-claude");
    expect(mocks.chatSend).toHaveBeenCalledTimes(1);
    const sentPrompt = mocks.chatSend.mock.calls[0][1] as string;
    expect(sentPrompt).toContain("`src/a.ts` (Modified, L1)");
    expect(sentPrompt).toContain("Rename newVal to count");
    expect(sentPrompt).toContain("`src/b.ts` (Modified, L1)");
    expect(sentPrompt).toContain("Add explicit return type");

    // Scope is cleared once delivered.
    expect(screen.queryByText(/review comment/)).toBeNull();
  });
});
