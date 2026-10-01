import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ResumeListing } from "../../../ipc/chat";

vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(),
}));

import { invoke } from "../../../ipc/transport";
import { useTermStore } from "../../../store/termStore";
import { resumeErrorKey, ResumePicker } from "./ResumePicker";

const listing = (): ResumeListing => ({
  directory: "/work/project",
  truncated: false,
  nextOffset: 3,
  conversations: [
    { id: "free", title: "Fix the build", firstPrompt: "The build fails on CI", updatedAt: 3_000, sizeBytes: 2048, gitBranch: "main", current: false },
    { id: "mine", title: "Current work", updatedAt: 2_000, sizeBytes: 10, current: true },
    { id: "owned", title: "Owned elsewhere", updatedAt: 1_000, sizeBytes: 10, current: false,
      owner: { sessionId: "other", name: "Claude 7", archived: true } },
  ],
});

type ListArgs = { sessionId: string; query?: string; offset?: number };

/** What the server answers: the matches among the recordings from `offset` on, at most `limit` of them. */
function page(all: ResumeListing, { query = "", offset = 0 }: ListArgs, limit = 500): ResumeListing {
  const needle = query.toLowerCase();
  const hits = [];
  let nextOffset = Math.min(offset, all.conversations.length);
  for (const c of all.conversations.slice(offset)) {
    if (hits.length === limit) break;
    nextOffset += 1;
    if (!needle || [c.title, c.firstPrompt, c.id, c.gitBranch].some((field) => field?.toLowerCase().includes(needle))) hits.push(c);
  }
  return { ...all, conversations: hits, truncated: nextOffset < all.conversations.length, nextOffset };
}

let resume: (args: unknown) => Promise<unknown>;
let list: (args: ListArgs) => Promise<unknown>;

beforeEach(() => {
  resume = () => Promise.resolve({ id: "s" });
  list = (args) => Promise.resolve(page(listing(), args));
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === "chat_resume_list") return list(args as ListArgs) as Promise<never>;
    if (command === "chat_resume") return resume(args) as Promise<never>;
    return Promise.resolve(undefined) as Promise<never>;
  });
});
afterEach(cleanup);

async function mount(initialQuery = "") {
  const onClose = vi.fn();
  render(<ResumePicker sessionId="s" initialQuery={initialQuery} onClose={onClose} />);
  await screen.findByText("Fix the build");
  return onClose;
}

const row = (title: string) => screen.getByText(title).closest(".resume-picker-row") as HTMLElement;
const resumeCalls = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_resume");
const listCalls = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_resume_list").map(([, args]) => args);

describe("the resume picker", () => {
  it("lists the session's conversations with the actions each one allows", async () => {
    await mount();
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("chat_resume_list", { sessionId: "s", query: "", offset: 0 });
    expect(screen.getByText("/work/project")).toBeTruthy();
    const free = within(row("Fix the build"));
    expect(free.getByText("The build fails on CI")).toBeTruthy();
    expect(free.getByText("2 KB")).toBeTruthy();
    expect(free.getByText("main")).toBeTruthy();
    expect(free.getByRole("button", { name: "Resume" })).toBeTruthy();
    expect(free.getByRole("button", { name: "Fork" })).toBeTruthy();
    // The conversation this session already holds offers only a branch.
    const mine = within(row("Current work"));
    expect(mine.getByText("Current")).toBeTruthy();
    expect(mine.queryByRole("button", { name: "Resume" })).toBeNull();
    expect(mine.getByRole("button", { name: "Fork" })).toBeTruthy();
    // One another session continues is opened there, never continued a second time here.
    const owned = within(row("Owned elsewhere"));
    expect(owned.getByText("In Claude 7")).toBeTruthy();
    expect(owned.getByText("Archived")).toBeTruthy();
    expect(owned.queryByRole("button", { name: "Resume" })).toBeNull();
    expect(owned.getByRole("button", { name: "Open session" })).toBeTruthy();
    expect(owned.getByRole("button", { name: "Fork" })).toBeTruthy();
  });

  it("lets the server search, starting from the typed search", async () => {
    await mount("fails on ci");
    expect(screen.getByRole("textbox", { name: "Search conversations" })).toHaveProperty("value", "fails on ci");
    expect(listCalls()).toEqual([{ sessionId: "s", query: "fails on ci", offset: 0 }]);
    expect(screen.queryByText("Current work")).toBeNull();
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "nothing like this" } });
    expect(await screen.findByText("No conversation matches your search.")).toBeTruthy();
    expect(listCalls().at(-1)).toEqual({ sessionId: "s", query: "nothing like this", offset: 0 });
    // Clearing the search asks for everything again.
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(await screen.findByText("Current work")).toBeTruthy();
    expect(listCalls().at(-1)).toEqual({ sessionId: "s", query: "", offset: 0 });
  });

  it("reaches every conversation beyond the first page", async () => {
    list = (args) => Promise.resolve(page(listing(), args, 2));
    await mount();
    expect(screen.queryByText("Owned elsewhere")).toBeNull();
    expect(screen.getByText("Showing the 2 most recent conversations.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Show older conversations" }));
    expect(await screen.findByText("Owned elsewhere")).toBeTruthy();
    expect(listCalls().at(-1)).toEqual({ sessionId: "s", query: "", offset: 2 });
    // All three are listed once, in order, and nothing is left to load.
    expect(screen.getAllByRole("option").map((option) => option.querySelector(".resume-picker-title")?.textContent))
      .toEqual(["Fix the build", "Current work", "Owned elsewhere"]);
    expect(screen.queryByRole("button", { name: "Show older conversations" })).toBeNull();
  });

  it("resumes in place or forks, then closes", async () => {
    const onClose = await mount();
    fireEvent.click(within(row("Fix the build")).getByRole("button", { name: "Resume" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(resumeCalls()[0][1]).toEqual({ sessionId: "s", agentSessionId: "free", mode: "resume" });
    cleanup();
    const again = await mount();
    fireEvent.click(within(row("Owned elsewhere")).getByRole("button", { name: "Fork" }));
    await waitFor(() => expect(again).toHaveBeenCalled());
    expect(resumeCalls()[1][1]).toEqual({ sessionId: "s", agentSessionId: "owned", mode: "fork" });
  });

  it("shows a refusal in the user's words and stays open", async () => {
    resume = () => Promise.reject(new Error("chat_resume_busy"));
    const onClose = await mount();
    fireEvent.click(within(row("Fix the build")).getByRole("button", { name: "Resume" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent",
      "Wait until the agent is idle: no running turn, queued messages, shell command, background task or open permission request.");
    expect(onClose).not.toHaveBeenCalled();
    expect(resumeErrorKey("Error: chat_resume_in_use", "act")).toBe("chat.resume.error.inUse");
    expect(resumeErrorKey("chat_resume_not_found", "act")).toBe("chat.resume.error.notFound");
    expect(resumeErrorKey("chat_resume_unsupported", "list")).toBe("chat.resume.error.unsupported");
    expect(resumeErrorKey("disk on fire", "act")).toBe("chat.resume.error.failed");
    expect(resumeErrorKey("disk on fire", "list")).toBe("chat.resume.error.list");
  });

  it("never shows a backend message, whether loading or acting fails", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    list = () => Promise.reject(new Error("Cannot read Claude conversations: Permission denied (os error 13)"));
    render(<ResumePicker sessionId="s" onClose={() => {}} />);
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "The conversations of this directory could not be read.");
    expect(screen.queryByText(/Permission denied/)).toBeNull();
    // A failed listing is not still loading.
    expect(screen.queryByRole("status")).toBeNull();
    cleanup();
    list = (args) => Promise.resolve(page(listing(), args));
    resume = () => Promise.reject(new Error("Failed to start claude: No such file or directory"));
    const onClose = await mount();
    fireEvent.click(within(row("Fix the build")).getByRole("button", { name: "Resume" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent",
      "The conversation could not be opened here. Try again, or resume it in the terminal view.");
    expect(screen.queryByText(/No such file/)).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    warn.mockRestore();
  });

  it("never acts on a listing that a newer search replaces", async () => {
    const onClose = await mount();
    const search = screen.getByRole("textbox");
    // Typed, and Enter pressed before the pause runs out: the old, unfiltered rows are gone and nothing is taken.
    fireEvent.change(search, { target: { value: "current" } });
    expect(screen.queryByText("Fix the build")).toBeNull();
    fireEvent.keyDown(search, { key: "Enter" });
    // The server is still scanning: nothing is taken either.
    let answer: (listing: ResumeListing) => void = () => {};
    list = (args) => new Promise((resolve) => { answer = () => resolve(page(listing(), args)); });
    await waitFor(() => expect(listCalls().at(-1)).toMatchObject({ query: "current" }));
    fireEvent.keyDown(search, { key: "Enter" });
    expect(screen.queryByText("Fix the build")).toBeNull();
    expect(resumeCalls()).toHaveLength(0);
    expect(onClose).not.toHaveBeenCalled();
    // Once the answer is in, Enter acts on the row the search found: the current conversation just closes.
    await act(async () => answer(listing()));
    expect(await screen.findByText("Current work")).toBeTruthy();
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onClose).toHaveBeenCalled();
    expect(resumeCalls()).toHaveLength(0);
  });

  it("never shows or acts on the previous result in the render that starts a new search", async () => {
    // Outside act, React commits the debounced search before its effect marks the listing as loading; a
    // listener that presses Enter on every DOM change catches any render that still shows the old rows.
    list = (args) => args.query ? new Promise(() => {}) : Promise.resolve(page(listing(), args));
    await mount();
    const environment = globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean };
    const before = environment.IS_REACT_ACT_ENVIRONMENT;
    environment.IS_REACT_ACT_ENVIRONMENT = false;
    const input = screen.getByRole("textbox") as HTMLInputElement;
    const stale: string[][] = [];
    const observer = new MutationObserver(() => {
      const rows = Array.from(document.querySelectorAll(".resume-picker-row .resume-picker-title")).map((e) => e.textContent ?? "");
      if (input.value === "current" && rows.length) {
        stale.push(rows);
        input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
      }
    });
    try {
      observer.observe(document.body, { childList: true, subtree: true, characterData: true });
      // A native change, like typing, so nothing flushes the effects early.
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "current");
      input.dispatchEvent(new Event("input", { bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 600));
    } finally {
      observer.disconnect();
      environment.IS_REACT_ACT_ENVIRONMENT = before;
    }
    expect(listCalls().at(-1)).toMatchObject({ query: "current" });
    expect(stale).toEqual([]);
    expect(resumeCalls()).toHaveLength(0);
  });

  it("opens the owning session, restoring it first when it is archived", async () => {
    const restoreSession = vi.fn(() => Promise.resolve());
    const openSession = vi.fn();
    useTermStore.setState({ restoreSession, openSession });
    const onClose = await mount();
    fireEvent.click(within(row("Owned elsewhere")).getByRole("button", { name: "Open session" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(restoreSession).toHaveBeenCalledWith("other");
    expect(openSession).toHaveBeenCalledWith("other");
    expect(resumeCalls()).toHaveLength(0);
  });

  it("is driven from the keyboard: arrows choose, Enter takes the row's first action, Escape closes", async () => {
    const onClose = await mount();
    const search = screen.getByRole("textbox");
    fireEvent.keyDown(search, { key: "ArrowDown" });
    fireEvent.keyDown(search, { key: "ArrowDown" });
    expect(row("Owned elsewhere").getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(search, { key: "ArrowUp" });
    fireEvent.keyDown(search, { key: "ArrowUp" });
    expect(row("Fix the build").getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() => expect(resumeCalls()).toHaveLength(1));
    expect(resumeCalls()[0][1]).toMatchObject({ agentSessionId: "free", mode: "resume" });
    cleanup();
    const closed = await mount();
    fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
    expect(closed).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("says when nothing was recorded or the listing was cut", async () => {
    list = () => Promise.resolve({ directory: "/work/empty", truncated: false, nextOffset: 0, conversations: [] });
    render(<ResumePicker sessionId="s" onClose={() => {}} />);
    expect(await screen.findByText("No earlier conversations were recorded in this directory.")).toBeTruthy();
    cleanup();
    list = () => Promise.resolve({ ...listing(), truncated: true });
    render(<ResumePicker sessionId="s" onClose={() => {}} />);
    expect(await screen.findByText("Showing the 3 most recent conversations.")).toBeTruthy();
    cleanup();
    // A page that ended before it found anything offers to search on from where it stopped.
    list = (args) => Promise.resolve(args.offset
      ? page(listing(), { ...args, offset: 0 })
      : { directory: "/work/project", truncated: true, nextOffset: 2000, conversations: [] });
    render(<ResumePicker sessionId="s" onClose={() => {}} />);
    expect(await screen.findByText("Older conversations have not been searched yet.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Show older conversations" }));
    expect(await screen.findByText("Fix the build")).toBeTruthy();
    expect(listCalls().at(-1)).toMatchObject({ offset: 2000 });
    cleanup();
    vi.spyOn(console, "warn").mockImplementationOnce(() => {});
    list = () => Promise.reject(new Error("chat_resume_unsupported"));
    render(<ResumePicker sessionId="s" onClose={() => {}} />);
    expect(await screen.findByText("Only Claude sessions in the conversation view can resume an earlier conversation.")).toBeTruthy();
    await act(async () => {});
  });
});
