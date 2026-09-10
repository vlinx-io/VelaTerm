import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ChatBackgroundTask, ChatEvent } from "../../../ipc/chat";
import type { TaskTab } from "../../../store/termStore";

vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(),
}));

import { invoke, listen } from "../../../ipc/transport";
import { setLang } from "../../../i18n";
import { TaskView, fmtElapsed } from "./TaskView";

let eventCallback: ((event: ChatEvent) => void) | undefined;
let unlisten: ReturnType<typeof vi.fn>;
let snapshotTasks: ChatBackgroundTask[] | undefined;

const seed: ChatBackgroundTask = {
  task_id: "worwyzf75",
  task_type: "local_workflow",
  description: "Alpha: alpha-worker",
  summary: "protocol probe",
  status: "running",
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
  eventCallback = undefined;
  // The backend omits an empty list, so an undefined list means "no tasks"; by default the snapshot confirms the seed.
  snapshotTasks = [seed];
  unlisten = vi.fn();
  vi.mocked(listen).mockReset();
  vi.mocked(listen).mockImplementation((_name, callback) => {
    eventCallback = callback as (event: ChatEvent) => void;
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
  // A window, even an empty one, makes the engine return a page: the view reads only the extras.
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "chat_snapshot")).toEqual([["chat_snapshot", { sessionId: "s", window: {} }]]);
});

it("follows extras events for its own task and ignores other tasks", async () => {
  await mount();
  extras([{ ...seed, task_id: "other", description: "Other: worker", usage: { total_tokens: 99 } }]);
  expect(screen.getByText("Alpha: alpha-worker")).toBeTruthy();
  expect(screen.queryByText("Other: worker")).toBeNull();
  // The task missing from a list is a signal that it ended; the last state stays on screen.
  expect(screen.getByText("Ended")).toBeTruthy();

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
  expect(screen.getByText("attempt 2")).toBeTruthy();
  expect(screen.getAllByText("done").length).toBe(2);
  expect(screen.getAllByText("running").length).toBe(1);
  expect(screen.queryByText("This task reports no per-agent progress.")).toBeNull();
});

it("says so when a workflow reports no per-agent progress", async () => {
  await mount();
  expect(screen.getByText("This task reports no per-agent progress.")).toBeTruthy();
});

it("shows the final state once the task ends and drops the Stop action", async () => {
  await mount();
  expect(screen.getByRole("button", { name: "Stop" })).toBeTruthy();
  extras([{ ...seed, status: "completed", ended_at: 1_006_324, summary: 'Dynamic workflow "protocol probe" completed', output_file: "/tmp/tasks/worwyzf75.output", usage: { total_tokens: 64131, tool_uses: 0, duration_ms: 6324 } }]);
  expect(screen.getByText("Completed")).toBeTruthy();
  expect(screen.getByText('Dynamic workflow "protocol probe" completed')).toBeTruthy();
  expect(screen.getByText("/tmp/tasks/worwyzf75.output")).toBeTruthy();
  expect(screen.getByText("0:06")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("shows a running task as ended once its process exits, and stops the clock and the Stop action", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(1_000_000 + 65_000);
  await mount();
  expect(screen.getByText("Running")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Stop" })).toBeTruthy();
  expect(screen.getByText("1:05")).toBeTruthy();

  // The process sends no extras on its way out; the exit itself is the last word about its tasks.
  act(() => eventCallback?.({ type: "exited", code: 1, stderr: "", released: false }));
  expect(screen.getByText("Ended")).toBeTruthy();
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  expect(screen.queryByText("Running")).toBeNull();
  const elapsed = screen.getByText("Elapsed").nextElementSibling!.textContent;
  act(() => { vi.advanceTimersByTime(3000); });
  expect(screen.getByText("Elapsed").nextElementSibling!.textContent).toBe(elapsed);
});

it("shows a running task as ended when a new process takes the session over", async () => {
  await mount();
  act(() => eventCallback?.({ type: "reset", epoch: 2 }));
  expect(screen.getByText("Ended")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("does not let a snapshot that resolves after the exit revive the task", async () => {
  let resolveSnapshot!: (value: unknown) => void;
  const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) =>
    command === "chat_snapshot" ? (new Promise<unknown>((resolve) => { resolveSnapshot = resolve; }) as Promise<never>) : previous(command, args));
  await mount();
  act(() => eventCallback?.({ type: "exited", code: 0, stderr: "", released: true }));
  expect(screen.getByText("Ended")).toBeTruthy();
  await act(async () => resolveSnapshot({ running: false, rows: [], queue: [], permissions: [], commands: [], configKeys: [], backgroundTasks: [seed] }));
  expect(screen.getByText("Ended")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("treats a snapshot without a task list as an empty one: the backend omits empty lists", async () => {
  snapshotTasks = undefined;
  await mount();
  expect(screen.getByText("Ended")).toBeTruthy();
});

it("shows a task the snapshot no longer lists as ended, not as running or empty", async () => {
  snapshotTasks = [];
  await mount();
  expect(screen.getByText("protocol probe", { selector: ".sv-task-title" })).toBeTruthy();
  expect(screen.getByText("Alpha: alpha-worker")).toBeTruthy(); // The last state seen stays on screen.
  expect(screen.getByText("Ended")).toBeTruthy();
  expect(screen.getByText("No longer reported by the agent")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("ticks the elapsed time once a second while the task runs", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(1_000_000 + 65_000);
  await mount();
  expect(screen.getByText("1:05")).toBeTruthy();
  act(() => { vi.advanceTimersByTime(2000); });
  expect(screen.getByText("1:07")).toBeTruthy();
});

it("stays mounted but invisible while hidden, and pauses the ticker until shown again", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(1_000_000 + 65_000);
  const view = render(<TaskView tab={tab} hidden={true} />);
  await act(async () => {});
  const root = view.container.querySelector(".sv-task-view") as HTMLElement;
  expect(root.style.display).toBe("none");
  expect(screen.getByText("1:05")).toBeTruthy(); // Painted while hidden: the state is kept, not rebuilt.
  expect(listen).toHaveBeenCalledTimes(1); // The subscription lives regardless of visibility.
  act(() => { vi.advanceTimersByTime(3000); });
  expect(screen.getByText("1:05")).toBeTruthy(); // No ticking behind a hidden tab.

  view.rerender(<TaskView tab={tab} hidden={false} />);
  expect(root.style.display).toBe("");
  expect(screen.getByText("1:08")).toBeTruthy(); // Showing it catches up at once.
  act(() => { vi.advanceTimersByTime(1000); });
  expect(screen.getByText("1:09")).toBeTruthy();
  expect(listen).toHaveBeenCalledTimes(1); // Toggling visibility does not resubscribe.
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
  expect(unlisten).toHaveBeenCalledTimes(1);
});

it("formats durations as m:ss and h:mm:ss", () => {
  expect(fmtElapsed(0)).toBe("0:00");
  expect(fmtElapsed(65_000)).toBe("1:05");
  expect(fmtElapsed(3_723_000)).toBe("1:02:03");
});
