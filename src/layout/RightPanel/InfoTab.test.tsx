import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { InfoTab } from "./InfoTab";
import { agentTurnStats } from "../../ipc/commands";
import { useTermStore } from "../../store/termStore";
import type { Session } from "../../types";

vi.mock("../../ipc/commands", () => ({
  agentContextInfo: vi.fn().mockResolvedValue(null),
  agentTurnStats: vi.fn().mockResolvedValue({ model: "Kiro model", contextLimit: 200000, contextPercent: 37 }),
  usageRefresh: vi.fn().mockResolvedValue(null),
}));
vi.mock("../../ipc/info", () => ({
  processStats: vi.fn().mockResolvedValue(null),
  systemStats: vi.fn().mockResolvedValue(null),
}));
vi.mock("../../hooks/useGitBranch", () => ({ useGitBranch: () => "main" }));

beforeAll(() => {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: false,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
});
afterEach(() => {
  cleanup();
  localStorage.clear();
  useTermStore.setState({ infoCollapsed: {}, usageAntigravityShowOther: false, usage: null });
});

const session = {
  id: "s1",
  kind: "terminal",
  name: "shell",
  agentSessionId: null,
  projectId: "p1",
} as unknown as Session;

describe("InfoTab sections", () => {
  it("displays Kiro's reported model and percentage without inventing token usage", async () => {
    render(<InfoTab session={{ ...session, kind: "kiro", agentSessionId: "native-id" }} cwd="/tmp" />);
    expect(await screen.findByText("Kiro model")).toBeTruthy();
    expect(screen.getByText("37%")).toBeTruthy();
    expect(screen.getByText("— / 200.0k")).toBeTruthy();
    expect(screen.queryByText("turn tokens")).toBeNull();
    expect(screen.queryByText("session total")).toBeNull();
  });
  it("shows ZCode's plan windows and its turn statistics", async () => {
    const entry = { error: null, fetchedAt: Date.now(), errorAt: null };
    useTermStore.setState({
      usage: {
        claude: { data: null, ...entry }, codex: { data: null, ...entry }, grok: { data: null, ...entry },
        zcode: {
          data: { platform: "bigmodel", level: "pro", windows: [
            { label: "5h", usedPercent: 7.1, resetsAt: null }, { label: "7d", usedPercent: 1.4, resetsAt: null },
          ] },
          ...entry,
        },
        auto: true, intervalSec: 300,
      },
    });
    render(<InfoTab session={{ ...session, kind: "zcode", agentSessionId: "sess_1" }} cwd="/tmp" />);
    expect(screen.getByText("ZCode")).toBeTruthy();
    expect(screen.getByText("5h")).toBeTruthy();
    expect(screen.getByText("7%")).toBeTruthy();
    expect(screen.getByText("1%")).toBeTruthy();
    expect(await screen.findByText("This turn")).toBeTruthy();
    useTermStore.setState({ usage: null });
  });
  it("shows DeepSeek's turn statistics from its session log", async () => {
    vi.mocked(agentTurnStats).mockResolvedValueOnce({
      model: "deepseek-v4-flash", tokens: 28100, inputTokens: 4200000, totalTokens: 4228100,
      cachedTokens: 4100000, cacheHitPercent: 97.8, contextTokens: 86100, contextLimit: 1048576,
      contextPercent: 8.2, sessionTokens: 169000000, toolsUsed: 4, filesTouched: 0, changedFiles: [],
      generationTokensPerSecond: 42,
    });
    render(<InfoTab session={{ ...session, kind: "deepseek", agentSessionId: "dsh-1" }} cwd="/tmp" />);
    expect(await screen.findByText("This turn")).toBeTruthy();
    expect(await screen.findByText("deepseek-v4-flash")).toBeTruthy();
    expect(screen.getAllByText("4.2M")).toHaveLength(2);
    expect(screen.getByText("169M")).toBeTruthy();
    expect(screen.getByText("86.1k / 1M")).toBeTruthy();
  });
  it("shows Antigravity's turn statistics from its native transcript", async () => {
    vi.mocked(agentTurnStats).mockResolvedValueOnce({
      model: "Gemini 3.8 Flash (High)", tokens: 162, inputTokens: 8733, totalTokens: 8895,
      cachedTokens: null, cacheHitPercent: null, contextTokens: 73500, contextLimit: null,
      contextPercent: null, sessionTokens: 120000, toolsUsed: 1, filesTouched: 0, changedFiles: [],
      generationTokensPerSecond: null,
    });
    render(<InfoTab session={{ ...session, kind: "antigravity", agentSessionId: "agy-1" }} cwd="/tmp" />);
    expect(await screen.findByText("This turn")).toBeTruthy();
    expect(await screen.findByText("Gemini 3.8 Flash (High)")).toBeTruthy();
    expect(screen.getByText("73.5k / —")).toBeTruthy();
  });
  it("shows Antigravity's quota buckets with window rail and inline reset times", () => {
    const entry = { error: null, fetchedAt: Date.now(), errorAt: null };
    useTermStore.setState({
      usage: {
        claude: { data: null, ...entry }, codex: { data: null, ...entry }, grok: { data: null, ...entry },
        antigravity: {
          data: { windows: [
            { label: "5h · gemini", usedPercent: 9, resetsAt: "2026-11-10T09:24:09Z" },
            { label: "7d · claude/gpt", usedPercent: 0, resetsAt: null },
          ] },
          ...entry,
        },
        auto: true, intervalSec: 300,
      },
    });
    const { container } = render(
      <InfoTab session={{ ...session, kind: "antigravity", agentSessionId: "conv_1" }} cwd="/tmp" />,
    );
    expect(screen.getByText("Antigravity")).toBeTruthy();
    expect(screen.queryByText("5h quota")).toBeNull();
    expect(screen.queryByText("7d quota")).toBeNull();
    expect(screen.getByText("5h")).toBeTruthy();
    expect(screen.getByText("gemini")).toBeTruthy();
    expect(screen.getByText("9%")).toBeTruthy();
    expect(screen.queryByText("claude/gpt")).toBeNull();
    expect(container.querySelectorAll(".usg-reset").length).toBe(1);

    fireEvent.click(screen.getByRole("button", { name: "Show claude/gpt" }));
    expect(screen.getByText("7d")).toBeTruthy();
    expect(screen.getByText("claude/gpt")).toBeTruthy();
    expect(screen.getByText("0%")).toBeTruthy();
    useTermStore.setState({ usage: null });
  });

  it("deduplicates window rail labels across multi-bucket groups", () => {
    const entry = { error: null, fetchedAt: Date.now(), errorAt: null };
    useTermStore.setState({
      usageAntigravityShowOther: true,
      usage: {
        claude: { data: null, ...entry }, codex: { data: null, ...entry }, grok: { data: null, ...entry },
        antigravity: {
          data: { windows: [
            { label: "5h · gemini", usedPercent: 47, resetsAt: "2026-11-10T08:43:00Z" },
            { label: "5h · claude/gpt", usedPercent: 0, resetsAt: "2026-11-10T12:04:00Z" },
            { label: "7d · gemini", usedPercent: 12, resetsAt: "2026-11-22T03:43:00Z" },
          ] },
          ...entry,
        },
        auto: true, intervalSec: 300,
      },
    });
    const { container } = render(
      <InfoTab session={{ ...session, kind: "antigravity", agentSessionId: "conv_1" }} cwd="/tmp" />,
    );
    expect(container.querySelectorAll(".usg-group").length).toBe(2);
    const winCells = Array.from(container.querySelectorAll(".usg-win")).map((el) => el.textContent);
    expect(winCells).toEqual(["5h", "", "7d"]);
    expect(container.querySelectorAll(".usg-reset").length).toBe(3);
    useTermStore.setState({ usage: null });
  });

  it("hides claude/gpt by default and reveals or hides it when the disclosure button is clicked", () => {
    const entry = { error: null, fetchedAt: Date.now(), errorAt: null };
    useTermStore.setState({
      usage: {
        claude: { data: null, ...entry }, codex: { data: null, ...entry }, grok: { data: null, ...entry },
        antigravity: {
          data: { windows: [
            { label: "5h · gemini", usedPercent: 52, resetsAt: "2026-11-10T13:18:00Z" },
            { label: "5h · claude/gpt", usedPercent: 0, resetsAt: "2026-11-10T15:56:00Z" },
            { label: "7d · gemini", usedPercent: 64, resetsAt: "2026-11-22T11:43:00Z" },
            { label: "7d · claude/gpt", usedPercent: 0, resetsAt: "2026-11-23T10:56:00Z" },
            { label: "7d · gemini 4 argon", usedPercent: 10, resetsAt: null },
          ] },
          ...entry,
        },
        auto: true, intervalSec: 300,
      },
    });
    render(<InfoTab session={{ ...session, kind: "antigravity", agentSessionId: "conv_1" }} cwd="/tmp" />);
    const showBtn = screen.getByRole("button", { name: "Show claude/gpt" });
    expect(showBtn.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByText("claude/gpt")).toBeNull();
    expect(screen.getAllByText("gemini")).toHaveLength(2);
    expect(screen.getByText("gemini 4 argon")).toBeTruthy();

    fireEvent.click(showBtn);
    expect(useTermStore.getState().usageAntigravityShowOther).toBe(true);
    const hideBtn = screen.getByRole("button", { name: "Hide claude/gpt" });
    expect(hideBtn.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getAllByText("claude/gpt")).toHaveLength(2);

    fireEvent.click(hideBtn);
    expect(useTermStore.getState().usageAntigravityShowOther).toBe(false);
    expect(screen.queryByText("claude/gpt")).toBeNull();
  });

  it("omits the disclosure toggle when only Gemini buckets or only non-Gemini buckets exist, and on other providers", () => {
    const entry = { error: null, fetchedAt: Date.now(), errorAt: null };
    useTermStore.setState({
      usage: {
        claude: {
          data: {
            fiveHour: { utilization: 20, resetsAt: "2026-11-10T10:10:00Z" },
            sevenDay: { utilization: 10, resetsAt: "2026-11-20T00:00:00Z" },
            sevenDayOpus: { utilization: 0, resetsAt: null },
            modelWeekly: [],
          },
          ...entry,
        },
        codex: { data: null, ...entry },
        grok: { data: null, ...entry },
        antigravity: {
          data: { windows: [
            { label: "5h · gemini", usedPercent: 52, resetsAt: null },
            { label: "7d · gemini", usedPercent: 64, resetsAt: null },
          ] },
          ...entry,
        },
        auto: true,
        intervalSec: 300,
      },
    });
    const { rerender } = render(
      <InfoTab session={{ ...session, kind: "antigravity", agentSessionId: "conv_1" }} cwd="/tmp" />,
    );
    expect(screen.queryByRole("button", { name: /Show |Hide / })).toBeNull();

    // An Antigravity snapshot with only claude/gpt renders it rather than collapsing to an empty list.
    useTermStore.setState({
      usage: {
        claude: { data: null, ...entry }, codex: { data: null, ...entry }, grok: { data: null, ...entry },
        antigravity: {
          data: { windows: [{ label: "5h · claude/gpt", usedPercent: 12, resetsAt: null }] },
          ...entry,
        },
        auto: true, intervalSec: 300,
      },
    });
    rerender(<InfoTab session={{ ...session, kind: "antigravity", agentSessionId: "conv_1" }} cwd="/tmp" />);
    expect(screen.getByText("claude/gpt")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Show |Hide / })).toBeNull();

    rerender(<InfoTab session={{ ...session, kind: "claude", agentSessionId: "conv_2" }} cwd="/tmp" />);
    expect(screen.queryByRole("button", { name: /Show |Hide / })).toBeNull();
  });

  it("applies warn and crit tones for Claude multi-bucket usage and renders stale vs countdown", () => {
    const now = Date.now();
    const claudeData = {
      fiveHour: { utilization: 78, resetsAt: "2026-11-10T10:10:00Z" },
      sevenDay: { utilization: 94, resetsAt: "2026-11-20T00:00:00Z" },
      sevenDayOpus: { utilization: 0, resetsAt: null },
      modelWeekly: [],
    };
    const empty = { data: null, error: null, fetchedAt: now, errorAt: null };
    useTermStore.setState({
      usage: {
        claude: { data: claudeData, error: null, fetchedAt: now, errorAt: null },
        codex: empty,
        grok: empty,
        auto: true,
        intervalSec: 300,
      },
    });
    const { container, rerender } = render(
      <InfoTab session={{ ...session, kind: "claude", agentSessionId: "conv_2" }} cwd="/tmp" />,
    );
    expect(container.querySelector(".usg-row.warn")?.textContent).toContain("78%");
    expect(container.querySelector(".usg-row.crit")?.textContent).toContain("94%");
    expect(container.querySelector(".usg-row.zero")?.textContent).toContain("0%");
    const countdown = container.querySelector(".usg-countdown")?.textContent ?? "";
    expect(countdown).toMatch(/^\d+:\d{2}$/);
    expect(countdown).not.toContain("↻");
    expect(container.querySelector(".usg-stale")).toBeNull();

    useTermStore.setState({
      usage: {
        claude: { data: claudeData, error: "timeout", fetchedAt: now - 600_000, errorAt: now },
        codex: empty,
        grok: empty,
        auto: true,
        intervalSec: 300,
      },
    });
    rerender(<InfoTab session={{ ...session, kind: "claude", agentSessionId: "conv_2" }} cwd="/tmp" />);
    expect(container.querySelector(".usg-stale")?.textContent).toBe("stale");
    expect(container.querySelector(".usg-countdown")).toBeNull();
    useTermStore.setState({ usage: null });
  });
  it("gives every section a collapse button and folds its body away", () => {
    render(<InfoTab session={session} cwd="/tmp" />);
    expect(screen.getByText("Process")).toBeTruthy();
    expect(screen.getByText("Resources")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Collapse section" })).toHaveLength(2);

    // The collapse control belongs to its own header, so folding Process leaves Resources intact.
    const process = screen.getByText("Process").closest(".insp-section") as HTMLElement;
    fireEvent.click(within(process).getByRole("button", { name: "Collapse section" }));
    expect(within(process).queryByText("shell")).toBeNull();
    expect(screen.getByText("Resources")).toBeTruthy();
    expect(useTermStore.getState().infoCollapsed).toEqual({ agent: true });
  });
});
