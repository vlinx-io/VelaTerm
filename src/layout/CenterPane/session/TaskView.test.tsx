import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ChatBackgroundTask, ChatEvent } from "../../../ipc/chat";
import type { TaskTab } from "../../../store/termStore";

const mode = vi.hoisted(() => ({ desktop: false }));
vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(),
  onTransportReconnect: vi.fn(() => vi.fn()), onTransportDisconnect: vi.fn(() => vi.fn()),
  get isTauri() { return mode.desktop; },
}));

import { invoke, listen, onTransportReconnect, onTransportDisconnect } from "../../../ipc/transport";
import { setLang } from "../../../i18n";
import { useTermStore } from "../../../store/termStore";
import { TaskView, fmtElapsed, plainOutput, taskStatusKey } from "./TaskView";

let eventCallback: ((event: ChatEvent) => void) | undefined;
let unlisten: ReturnType<typeof vi.fn>;
let snapshotTasks: ChatBackgroundTask[] | undefined;

const seed: ChatBackgroundTask = {
  task_id: "worwyzf75",
  task_type: "local_workflow",
  description: "Alpha: alpha-worker",
  summary: "protocol probe",
  status: "running",
  finished: false, can_stop: true, elapsed_ms: 65_000,
  started_at: 1_000_000,
  usage: { total_tokens: 1200, tool_uses: 2, duration_ms: 20 },
  last_tool_name: "alpha-worker",
};

const tab: TaskTab = { id: "task-1", sessionId: "s", taskId: "worwyzf75", title: "protocol probe", taskType: "local_workflow", seed };

const progress: ChatBackgroundTask["workflow_progress"] = [
  { type: "workflow_phase", index: 1, title: "Alpha" },
  { type: "workflow_phase", index: 2, title: "Beta" },
  { type: "workflow_agent", index: 1, label: "alpha-worker", phaseIndex: 1, model: "haiku", state: "done", attempt: 1, durationMs: 1200, tokens: 300, promptPreview: "Reply ALPHA", resultPreview: "ALPHA" },
  { type: "workflow_agent", index: 2, label: "beta-worker", phaseIndex: 1, model: "haiku", state: "done", attempt: 2, resultPreview: "BETA" },
  { type: "workflow_agent", index: 3, label: "gamma-worker", phaseIndex: 2, model: "haiku", state: "start", startedAt: 1_000_500, promptPreview: "Reply GAMMA", resultPreview: "not yet" },
];

beforeEach(() => {
  setLang("en");
  mode.desktop = false;
  eventCallback = undefined;
  // The backend omits an empty list, so an undefined list means "no tasks"; by default the snapshot confirms the seed.
  snapshotTasks = [seed];
  unlisten = vi.fn();
  vi.mocked(listen).mockReset();
  vi.mocked(listen).mockImplementation((name, callback) => {
    // The task-detail name only asks the server for the tree; its listener never receives anything.
    if (name.startsWith("chat://event/")) eventCallback = callback as (event: ChatEvent) => void;
    return Promise.resolve(unlisten as unknown as () => void);
  });
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "chat_snapshot") {
      return Promise.resolve({ running: true, rows: [], queue: [], permissions: [], commands: [], configKeys: [], backgroundTasks: snapshotTasks }) as Promise<never>;
    }
    return Promise.resolve(undefined) as Promise<never>;
  });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

/** Send one `extras` event carrying `tasks` to the mounted view. */
function extras(tasks: ChatBackgroundTask[]) {
  act(() => eventCallback?.({ type: "extras", extras: { backgroundTasks: tasks } }));
}

async function mount(over: Partial<TaskTab> = {}) {
  const view = render(<TaskView tab={{ ...tab, ...over }} hidden={false} />);
  // Let the subscription and the snapshot settle.
  await act(async () => {});
  return view;
}

it("paints the seed at once, subscribes to the session, and reads one paged snapshot for it", async () => {
  await mount();
  expect(screen.getByText("protocol probe", { selector: ".sv-task-title" })).toBeTruthy();
  expect(screen.getByText("Alpha: alpha-worker")).toBeTruthy();
  expect(screen.getByText("alpha-worker")).toBeTruthy();
  expect(screen.getByText("1.2k")).toBeTruthy();
  expect(screen.getByText("Running")).toBeTruthy();
  expect(listen).toHaveBeenCalledWith("chat://event/s", expect.any(Function));
  // The view reads only the extras: no rows, and no workflow tree but its own task's.
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_snapshot")).toEqual([
    ["chat_snapshot", { sessionId: "s", window: { limit: 0, compactTasks: true, detailTask: "worwyzf75" } }],
  ]);
  expect(listen).toHaveBeenCalledWith("chat://task/s/worwyzf75", expect.any(Function));
});

it("follows extras events for its own task and ignores other tasks", async () => {
  await mount();
  extras([{ ...seed, task_id: "other", description: "Other: worker", usage: { total_tokens: 99 } }]);
  expect(screen.getByText("Alpha: alpha-worker")).toBeTruthy();
  expect(screen.queryByText("Other: worker")).toBeNull();
  // The task missing from a list is a signal that it ended; the last state stays on screen.
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();

  extras([{ ...seed, description: "Beta: gamma-worker", last_tool_name: "gamma-worker", usage: { total_tokens: 64131, tool_uses: 3, duration_ms: 5000 } }]);
  expect(screen.getByText("Beta: gamma-worker")).toBeTruthy();
  expect(screen.getByText("gamma-worker")).toBeTruthy();
  expect(screen.getByText("64.1k")).toBeTruthy();
  expect(screen.getByText("3")).toBeTruthy();
  expect(screen.getByText("Running")).toBeTruthy();
  expect(screen.queryByText("Ended")).toBeNull();
});

it("renders the workflow's phases and agents, with results only for finished agents", async () => {
  await mount();
  extras([{ ...seed, workflow_progress: progress }]);
  expect(screen.getByText("Alpha")).toBeTruthy();
  expect(screen.getByText("Beta")).toBeTruthy();
  expect(screen.getByText("alpha-worker", { selector: ".sv-task-agent-label" })).toBeTruthy();
  expect(screen.getByText("beta-worker")).toBeTruthy();
  expect(screen.getByText("gamma-worker", { selector: ".sv-task-agent-label" })).toBeTruthy();
  expect(screen.getByText("ALPHA")).toBeTruthy();
  expect(screen.getByText("BETA")).toBeTruthy();
  expect(screen.queryByText("not yet")).toBeNull();
  expect(screen.getByText("Reply GAMMA")).toBeTruthy();
  expect(screen.getByText("Attempt 2")).toBeTruthy();
  expect(screen.getAllByText("Done").length).toBe(2);
  expect(screen.getAllByText("Running", { selector: ".sv-task-agent-state" }).length).toBe(1);
  expect(screen.queryByText("This task reports no per-agent progress.")).toBeNull();
});

it("says so when a workflow reports no per-agent progress", async () => {
  await mount();
  expect(screen.getByText("This task reports no per-agent progress.")).toBeTruthy();
});

it("shows the final state once the task ends and drops the Stop action", async () => {
  await mount();
  expect(screen.getByRole("button", { name: "Stop" })).toBeTruthy();
  extras([{ ...seed, status: "completed", finished: true, can_stop: false, elapsed_ms: 6324, ended_at: 1_006_324, summary: 'Dynamic workflow "protocol probe" completed', output_file: "/tmp/tasks/worwyzf75.output", usage: { total_tokens: 64131, tool_uses: 0, duration_ms: 6324 } }]);
  expect(screen.getByText("Completed")).toBeTruthy();
  expect(screen.getByText('Dynamic workflow "protocol probe" completed')).toBeTruthy();
  expect(screen.getByText("/tmp/tasks/worwyzf75.output")).toBeTruthy();
  expect(screen.getByText("0:06")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("retains the last reported state but marks it unavailable after exit, and stops the clock and Stop action", async () => {
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout", "Date", "performance"] });
  vi.setSystemTime(1_000_000 + 65_000);
  await mount();
  expect(screen.getByText("Running")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Stop" })).toBeTruthy();
  expect(screen.getByText("1:05")).toBeTruthy();

  // The process sends no extras on its way out; the exit itself is the last word about its tasks.
  act(() => eventCallback?.({ type: "exited", code: 1, stderr: "", released: false }));
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  expect(screen.getByText("Running")).toBeTruthy();
  const elapsed = screen.getByText("Elapsed time").nextElementSibling!.textContent;
  act(() => { vi.advanceTimersByTime(3000); });
  expect(screen.getByText("Elapsed time").nextElementSibling!.textContent).toBe(elapsed);
});

it("marks old task data unavailable when a new process takes the session over", async () => {
  await mount();
  act(() => eventCallback?.({ type: "reset", epoch: 2 }));
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("does not let a snapshot that resolves after the exit revive the task", async () => {
  let resolveSnapshot!: (value: unknown) => void;
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) =>
    command === "chat_snapshot" ? (new Promise<unknown>((resolve) => { resolveSnapshot = resolve; }) as Promise<never>) : previous(command, args));
  await mount();
  act(() => eventCallback?.({ type: "exited", code: 0, stderr: "", released: true }));
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
  await act(async () => resolveSnapshot({ running: false, rows: [], queue: [], permissions: [], commands: [], configKeys: [], backgroundTasks: [seed] }));
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("treats a snapshot without a task list as an empty one: the backend omits empty lists", async () => {
  snapshotTasks = undefined;
  await mount();
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
});

it("retains last task facts with an unavailable label when the snapshot no longer lists it", async () => {
  snapshotTasks = [];
  await mount();
  expect(screen.getByText("protocol probe", { selector: ".sv-task-title" })).toBeTruthy();
  expect(screen.getByText("Alpha: alpha-worker")).toBeTruthy(); // The last state seen stays on screen.
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("ticks the elapsed time once a second while the task runs", async () => {
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout", "Date", "performance"] });
  vi.setSystemTime(1_000_000 + 65_000);
  await mount();
  expect(screen.getByText("1:05")).toBeTruthy();
  act(() => { vi.advanceTimersByTime(2000); });
  expect(screen.getByText("1:07")).toBeTruthy();
});

it("stays mounted but invisible while hidden, follows nothing remotely, and catches up when shown", async () => {
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout", "Date", "performance"] });
  vi.setSystemTime(1_000_000 + 65_000);
  const view = render(<TaskView tab={tab} hidden={true} />);
  await act(async () => {});
  const root = view.container.querySelector(".sv-task-view") as HTMLElement;
  expect(root.style.display).toBe("none");
  expect(screen.getByText("1:05")).toBeTruthy(); // Painted while hidden: the state is kept, not rebuilt.
  // A hidden remote tab neither subscribes nor reads a snapshot, so the server sends it nothing.
  expect(listen).not.toHaveBeenCalled();
  expect(invoke).not.toHaveBeenCalled();
  act(() => { vi.advanceTimersByTime(3000); });
  expect(screen.getByText("1:05")).toBeTruthy(); // No ticking behind a hidden tab.

  snapshotTasks = [{ ...seed, elapsed_ms: 68_000 }];
  view.rerender(<TaskView tab={tab} hidden={false} />);
  await act(async () => {});
  expect(root.style.display).toBe("");
  expect(listen).toHaveBeenCalledWith("chat://event/s", expect.any(Function));
  expect(listen).toHaveBeenCalledWith("chat://task/s/worwyzf75", expect.any(Function));
  expect(screen.getByText("1:08")).toBeTruthy(); // Showing it catches up at once from a fresh snapshot.
  act(() => { vi.advanceTimersByTime(1000); });
  expect(screen.getByText("1:09")).toBeTruthy();

  view.rerender(<TaskView tab={tab} hidden={true} />);
  expect(unlisten).toHaveBeenCalledTimes(2); // Hiding it again leaves both names.
});

it("keeps a hidden tab subscribed on the desktop, as before", async () => {
  mode.desktop = true;
  const view = render(<TaskView tab={tab} hidden={true} />);
  await act(async () => {});
  expect(listen).toHaveBeenCalledWith("chat://event/s", expect.any(Function));
  view.rerender(<TaskView tab={tab} hidden={false} />);
  await act(async () => {});
  expect(vi.mocked(listen).mock.calls.filter(([name]) => name === "chat://event/s")).toHaveLength(1); // No resubscribe.
  expect(unlisten).not.toHaveBeenCalled();
});

it("shows loading, not an empty tree, while the server has left the tree out", async () => {
  await mount();
  extras([{ ...seed, detail_omitted: true }]);
  expect(screen.getByText("Loading…")).toBeTruthy();
  expect(screen.queryByText("This task reports no per-agent progress.")).toBeNull();
  extras([{ ...seed, workflow_progress: progress }]);
  expect(screen.getByText("Reply GAMMA")).toBeTruthy();
  extras([{ ...seed }]);
  expect(screen.getByText("This task reports no per-agent progress.")).toBeTruthy();
});

it("says the tree is unavailable instead of loading forever when the server rejects the detail watch", async () => {
  let detailCallback: ((event: unknown) => void) | undefined;
  vi.mocked(listen).mockImplementation((name, callback) => {
    if (name.startsWith("chat://event/")) eventCallback = callback as (event: ChatEvent) => void;
    if (name.startsWith("chat://task/")) detailCallback = callback as (event: unknown) => void;
    return Promise.resolve(unlisten as unknown as () => void);
  });
  await mount();
  extras([{ ...seed, detail_omitted: true }]);
  expect(screen.getByText("Loading…")).toBeTruthy();
  act(() => detailCallback?.({ type: "watchRejected" }));
  expect(screen.queryByText("Loading…")).toBeNull();
  expect(screen.getByText("The workflow details cannot be shown over this connection right now.")).toBeTruthy();
  // A tree that does arrive (an older server, or the desktop) still wins.
  extras([{ ...seed, workflow_progress: progress }]);
  expect(screen.getByText("Reply GAMMA")).toBeTruthy();
});

it("reads a fresh snapshot at a resync barrier", async () => {
  await mount();
  snapshotTasks = [{ ...seed, description: "After resync" }];
  await act(async () => eventCallback?.({ type: "resync" }));
  expect(screen.getByText("After resync")).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_snapshot")).toHaveLength(2);
});

it("stops exactly this task and reports a refusal", async () => {
  await mount();
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  expect(invoke).toHaveBeenCalledWith("chat_stop_task", { sessionId: "s", taskId: "worwyzf75" });
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === "chat_stop_task" ? Promise.reject(new Error("No such task")) : previous(command, args));
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  await screen.findByRole("alert");
  expect(screen.getByRole("alert").textContent).toContain("No such task");
});

it("unsubscribes on unmount", async () => {
  const view = await mount();
  view.unmount();
  expect(unlisten).toHaveBeenCalledTimes(2); // The session channel and the task-detail name.
});

it("formats durations as m:ss and h:mm:ss", () => {
  expect(fmtElapsed(0)).toBe("0:00");
  expect(fmtElapsed(65_000)).toBe("1:05");
  expect(fmtElapsed(3_723_000)).toBe("1:02:03");
});

it("registers a fresh snapshot on reconnect without starting the agent", async () => {
  await mount();
  const reconnect = vi.mocked(onTransportReconnect).mock.calls.at(-1)![0];
  const disconnect = vi.mocked(onTransportDisconnect).mock.calls.at(-1)![0];
  act(() => disconnect());
  expect(screen.getByRole("alert").textContent).toContain("synchron");
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  snapshotTasks = [{ ...seed, finished: true, can_stop: false, status: "completed", elapsed_ms: 67000 }];
  await act(async () => reconnect());
  expect(screen.getByText("Completed")).toBeTruthy();
  expect(screen.getByText("1:07")).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_snapshot")).toHaveLength(2);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_start")).toBe(false);
});

it("shows snapshot failures and allows a bounded user retry", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("offline"));
  await mount();
  expect(screen.getByRole("alert")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await act(async () => {});
  expect(screen.queryByRole("alert")).toBeNull();
  expect(screen.getByRole("button", { name: "Stop" })).toBeTruthy();
});

it("unknown task states use backend lifecycle facts and labels depend on task type", async () => {
  await mount();
  extras([{ ...seed, task_type: "local_agent", status: "waiting_for_peer", last_tool_name: "Bash" }]);
  expect(screen.getByText("waiting_for_peer")).toBeTruthy();
  expect(screen.getByText("Last reported tool")).toBeTruthy();
  expect(screen.getByText("Bash")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Stop" })).toBeTruthy();
  extras([{ ...seed, task_type: "future_task", last_tool_name: "unclassified" }]);
  expect(screen.queryByText("Last reported tool")).toBeNull();
  expect(screen.queryByText("Last reported agent")).toBeNull();
  expect(screen.queryByText("unclassified")).toBeNull();
});

it("client wall-clock jumps do not change a backend task duration", async () => {
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout", "Date", "performance"] });
  await mount();
  expect(screen.getByText("1:05")).toBeTruthy();
  vi.setSystemTime(Date.now() + 12 * 60 * 60 * 1000);
  act(() => vi.advanceTimersByTime(2000));
  expect(screen.getByText("1:07")).toBeTruthy();
});

it("hydrates a deep-link placeholder without changing focus or inventing an unknown status", async () => {
  const hydrate = vi.spyOn(useTermStore.getState(), "hydrateTaskTab");
  await mount({ taskType: "", seed: undefined });
  expect(hydrate).toHaveBeenCalledWith("s", seed);
  expect(taskStatusKey(undefined)).toBeNull();
  hydrate.mockRestore();
});

it("shows a shell task's command and output, and keeps reading the output while it runs", async () => {
  vi.useFakeTimers();
  const shell: ChatBackgroundTask = { task_id: "b1", task_type: "local_bash", description: "Wait for batch", status: "running", finished: false, can_stop: true, elapsed_ms: 1000 };
  snapshotTasks = [shell];
  let reads = 0;
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === "chat_snapshot") {
      return Promise.resolve({ running: true, rows: [], queue: [], permissions: [], commands: [], configKeys: [], backgroundTasks: snapshotTasks }) as Promise<never>;
    }
    if (command === "chat_task_output") {
      expect(args).toEqual({ sessionId: "s", taskId: "b1" });
      reads++;
      return Promise.resolve({ command: "sleep 60 && echo done", output: `\x1b[32mstep ${reads}\x1b[0m\n`, truncated: reads > 1 }) as Promise<never>;
    }
    return Promise.resolve(undefined) as Promise<never>;
  });
  await mount({ taskId: "b1", taskType: "local_bash", seed: shell, title: "Wait for batch" });
  expect(screen.getByText("sleep 60 && echo done")).toBeTruthy();
  expect(screen.getByText("step 1", { selector: ".sv-task-output" })).toBeTruthy();
  expect(screen.queryByText("Only the most recent output is shown.")).toBeNull();
  await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
  expect(screen.getByText("step 2", { selector: ".sv-task-output" })).toBeTruthy();
  expect(screen.getByText("Only the most recent output is shown.")).toBeTruthy();
});

it("does not read output for tasks that are not shell commands", async () => {
  await mount();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_task_output")).toBe(false);
});

it("renders terminal output as plain text", () => {
  expect(plainOutput("\x1b[1;31mred\x1b[0m\r\n\x1b]0;title\x07ok")).toBe("red\nok");
  expect(plainOutput("10%\r50%\r100%\nnext")).toBe("100%\nnext");
});
