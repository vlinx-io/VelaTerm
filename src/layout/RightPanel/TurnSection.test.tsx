import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TurnSection, TurnStatsView } from "./TurnSection";
import { agentTurnStats, gitChangedFiles, type AgentTurnStats, type ChangedFile } from "../../ipc/commands";
import { getGitStatus } from "../../ipc/info";
import type { Session } from "../../types";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";
vi.mock("../../ipc/commands", () => ({ agentTurnStats: vi.fn(), gitChangedFiles: vi.fn() }));
vi.mock("../../ipc/info", () => ({ getGitStatus: vi.fn() }));
const stats: AgentTurnStats = {
  model: "gpt-5.2-codex", tokens: 1600, inputTokens: 12000, totalTokens: 13600, cachedTokens: 10560,
  cacheHitPercent: 88, sessionTokens: 320000, toolsUsed: 3, filesTouched: 0, changedFiles: [],
  contextTokens: 73000, contextLimit: 258400, contextPercent: 28.25, generationTokensPerSecond: 32,
};
const session = (id: string, agentSessionId: string | null = "native") => ({ id, kind: "codex", agentSessionId } as Session);
// jsdom has no matchMedia, and switching views persists through the store, which resolves the theme.
beforeAll(() => {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: false, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn(),
  }));
});
afterEach(() => { cleanup(); setLang("en"); vi.resetAllMocks(); useTermStore.setState({ turnFilesView: false }); });
describe("This turn", () => {
  it("separates context, turn totals, cached input and session usage", () => {
    render(<TurnStatsView stats={stats} />);
    const panel = screen.getByRole("region", { name: "This turn" });
    expect(within(panel).getByText("gpt-5.2-codex")).toBeTruthy();
    expect(within(panel).getByRole("progressbar").getAttribute("aria-valuenow")).toBe("28.25");
    expect(within(panel).getByText("13.6k")).toBeTruthy();
    expect(within(panel).getByText("320k")).toBeTruthy();
    expect(within(panel).getByText("88.0%")).toBeTruthy();
    expect(within(panel).getByText("32.0 tok/s")).toBeTruthy();
    expect(within(panel).getByText("average output").closest(".turn-metric")?.getAttribute("title"))
      .toContain("not pure model decoding speed");
    expect(within(panel).getByText("0")).toBeTruthy();
  });
  it("shows absent metrics as unknown and caps only the visual context meter", () => {
    const { rerender } = render(<TurnStatsView stats={null} state="error" />);
    expect(screen.getByRole("status").textContent).toContain("temporarily unavailable");
    expect(screen.queryByText("0")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    rerender(<TurnStatsView stats={{ ...stats, contextPercent: 105, generationTokensPerSecond: null }} />);
    expect(screen.getByText("105%")).toBeTruthy();
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("100");
    expect(screen.queryByText(/tok\/s/)).toBeNull();
  });
  it("shows context used without a meter when the model's limit is unknown", () => {
    render(<TurnStatsView stats={{ ...stats, contextPercent: null, contextLimit: null }} />);
    const panel = screen.getByRole("region", { name: "This turn" });
    expect(within(panel).queryByRole("progressbar")).toBeNull();
    expect(within(panel).getByText("73.0k / —")).toBeTruthy();
  });
  it("updates the output-rate label and explanation when the language changes", async () => {
    render(<TurnStatsView stats={stats} />);
    act(() => setLang("zh-CN"));
    const label = await screen.findByText("平均输出速度");
    expect(label.closest(".turn-metric")?.getAttribute("title")).toContain("并非模型的纯解码速度");
    expect(screen.getByText("32.0 tok/s")).toBeTruthy();
  });
  it("rejects a late response from a previously selected session", async () => {
    let resolveOld!: (value: AgentTurnStats) => void;
    vi.mocked(agentTurnStats).mockImplementation(id => id === "old" ? new Promise(resolve => { resolveOld = resolve; }) : Promise.resolve({ ...stats, totalTokens: 9000 }));
    const { rerender } = render(<TurnSection session={session("old")} />);
    rerender(<TurnSection session={session("new")} />);
    await screen.findByText("9k");
    await act(async () => { resolveOld(stats); });
    expect(screen.queryByText("13.6k")).toBeNull();
    expect(screen.getByText("9k")).toBeTruthy();
  });
  it("clears stale statistics when a read fails and waits for an uncaptured session", async () => {
    vi.mocked(agentTurnStats).mockResolvedValueOnce(stats).mockRejectedValue(new Error("missing"));
    const { rerender } = render(<TurnSection session={session("a")} />);
    await screen.findByText("13.6k");
    rerender(<TurnSection session={session("a")} currentTool="Read" />);
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("temporarily unavailable"));
    expect(screen.queryByText("13.6k")).toBeNull();
    const calls = vi.mocked(agentTurnStats).mock.calls.length;
    rerender(<TurnSection session={session("b", null)} />);
    expect(screen.getByRole("status").textContent).toContain("first turn");
    expect(vi.mocked(agentTurnStats).mock.calls.length).toBe(calls);
  });
  it("swaps the statistics for the changed files and remembers the choice until the user goes back", async () => {
    const openDocTab = vi.fn();
    const openDiffTab = vi.fn();
    useTermStore.setState({ turnFilesView: false, openDocTab, openDiffTab });
    vi.mocked(getGitStatus).mockResolvedValue({ isRepo: true, root: "/repo", ahead: 0, behind: 0, staged: 0, unstaged: 1, untracked: 0 });
    vi.mocked(gitChangedFiles).mockResolvedValue({ files: [{ path: "src/a.ts" } as ChangedFile], truncated: false });
    const changed = { ...stats, filesTouched: 3, changedFiles: [
      { path: "/repo/src/a.ts", additions: 12, deletions: 3 },
      { path: "README.md", additions: 1, deletions: 0 },
      { path: "/elsewhere/b.rs", additions: 4, deletions: 4 },
    ] };
    const { rerender } = render(<TurnStatsView stats={changed} cwd="/repo/" />);
    fireEvent.click(screen.getByRole("button", { name: "Show the files changed in this turn" }));
    expect(useTermStore.getState().turnFilesView).toBe(true);
    expect(screen.queryByText("13.6k")).toBeNull();
    // The header totals every file; each row shows its own lines.
    expect(screen.getByText("3 files").parentElement?.textContent).toBe("3 files+17−7");
    // Paths under the session folder lose that prefix; others keep their own folder.
    expect(screen.getByText("a.ts").nextElementSibling?.textContent).toBe("src");
    expect(screen.getByText("a.ts").closest("button")?.textContent).toBe("a.tssrc+12−3");
    expect(screen.getByText("b.rs").nextElementSibling?.textContent).toBe("/elsewhere");
    // A file with uncommitted changes opens in a diff tab at its repository-relative path.
    fireEvent.click(screen.getByText("a.ts"));
    await waitFor(() => expect(openDiffTab).toHaveBeenCalledWith("/repo/src/a.ts", "/repo", "src/a.ts"));
    expect(vi.mocked(getGitStatus)).toHaveBeenCalledWith("/repo/src");
    // One with nothing left to compare opens in the editor instead.
    fireEvent.click(screen.getByText("README.md"));
    await waitFor(() => expect(openDocTab).toHaveBeenCalledWith("/repo/README.md"));
    expect(openDiffTab).toHaveBeenCalledTimes(1);
    // A later turn opens on the list as well.
    rerender(<TurnStatsView stats={{ ...changed, filesTouched: 0, changedFiles: [] }} cwd="/repo" />);
    expect(screen.getByText("No files were edited in this turn.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Turn stats" }));
    expect(useTermStore.getState().turnFilesView).toBe(false);
    expect(screen.getByText("13.6k")).toBeTruthy();
  });
  it("leaves out the numbers of a file it could not count, and the totals with them", () => {
    useTermStore.setState({ turnFilesView: true });
    render(<TurnStatsView stats={{ ...stats, filesTouched: 2, changedFiles: [
      { path: "/r/a.ts", additions: 2, deletions: 1 },
      { path: "/r/b.ts", additions: null, deletions: null },
    ] }} cwd="/r" />);
    expect(screen.getByText("2 files").parentElement?.textContent).toBe("2 files");
    expect(screen.getByText("b.ts").closest("button")?.textContent).toBe("b.ts");
    expect(screen.getByText("a.ts").closest("button")?.textContent).toBe("a.ts+2−1");
  });
  it("merges a file named both absolutely and relatively, and sorts by the path shown", () => {
    useTermStore.setState({ turnFilesView: true });
    render(<TurnStatsView stats={{ ...stats, filesTouched: 3, changedFiles: [
      { path: "/r/src/b.ts", additions: 2, deletions: 1 },
      { path: "a.ts", additions: 1, deletions: 0 },
      { path: "src/b.ts", additions: 3, deletions: 0 },
    ] }} cwd="/r" />);
    expect(screen.getByText("2 files").parentElement?.textContent).toBe("2 files+6−1");
    expect([...document.querySelectorAll(".turn-file")].map((b) => b.textContent)).toEqual(["a.ts+1−0", "b.tssrc+5−1"]);
  });
  it("falls back to the statistics when the agent records no file list", () => {
    useTermStore.setState({ turnFilesView: true });
    render(<TurnStatsView stats={{ ...stats, filesTouched: null, changedFiles: null }} />);
    expect(screen.getByText("13.6k")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Show the files changed in this turn" })).toBeNull();
  });
});
