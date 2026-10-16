import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../../../ipc/chat", async (original) => ({
  ...await original<typeof import("../../../ipc/chat")>(),
  chatMcpStatus: vi.fn(), chatMcpToggle: vi.fn(), chatMcpReconnect: vi.fn(),
}));

import { chatMcpStatus, chatMcpToggle, type ChatBackgroundTask } from "../../../ipc/chat";
import { setLang } from "../../../i18n";
import { McpChip, TasksChip, UsageMeter } from "./extras";

const servers = { mcpServers: [{ name: "local-tools", status: "connected", tools: [] }] };

beforeEach(() => { vi.resetAllMocks(); setLang("en"); });
afterEach(cleanup);

it("stops loading after an unknown-command failure and recovers by retrying the list", async () => {
  let fail!: (error: Error) => void;
  vi.mocked(chatMcpStatus).mockReturnValueOnce(new Promise((_resolve, reject) => { fail = reject; }));
  render(<McpChip sessionId="session-a" />);
  fireEvent.click(screen.getByRole("button", { name: "MCP" }));
  expect(screen.getByText("Reading the server list…")).toBeTruthy();
  await act(async () => fail(new Error("Unknown command: chat_mcp_status")));
  expect(screen.queryByText("Reading the server list…")).toBeNull();
  expect(screen.queryByText("No MCP servers configured")).toBeNull();
  expect(screen.getByRole("alert").textContent).toContain("Update and restart that backend");
  vi.mocked(chatMcpStatus).mockResolvedValueOnce(servers);
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await screen.findByText("local-tools");
  expect(screen.queryByRole("alert")).toBeNull();
  expect(chatMcpStatus).toHaveBeenCalledTimes(2);
  expect(chatMcpStatus).toHaveBeenLastCalledWith("session-a");
});

it("preserves other request errors and allows an empty successful retry", async () => {
  vi.mocked(chatMcpStatus).mockRejectedValueOnce(new Error("Agent request timed out"));
  render(<McpChip sessionId="session-a" />);
  fireEvent.click(screen.getByRole("button", { name: "MCP" }));
  await screen.findByText("Error: Agent request timed out");
  expect(screen.queryByText("Reading the server list…")).toBeNull();
  vi.mocked(chatMcpStatus).mockResolvedValueOnce({ mcpServers: [] });
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await screen.findByText("No MCP servers configured");
  expect(screen.queryByRole("alert")).toBeNull();
});

it("hides stale controls after a mutation fails and retries only the read", async () => {
  vi.mocked(chatMcpStatus).mockResolvedValue(servers);
  vi.mocked(chatMcpToggle).mockRejectedValueOnce("Unknown command: chat_mcp_toggle");
  render(<McpChip sessionId="session-a" />);
  fireEvent.click(screen.getByRole("button", { name: "MCP" }));
  fireEvent.click(await screen.findByRole("button", { name: "Disable" }));
  await screen.findByRole("alert");
  expect(screen.queryByRole("button", { name: "Disable" })).toBeNull();
  expect(screen.queryByText("Reading the server list…")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await screen.findByRole("button", { name: "Disable" });
  expect(chatMcpToggle).toHaveBeenCalledTimes(1);
  expect(chatMcpStatus).toHaveBeenCalledTimes(2);
});

it("keeps the Tasks chip in the row with an empty task list and says so when opened", () => {
  render(<TasksChip sessionId="s" tasks={[]} busy={false} onStop={() => {}} onOpen={() => {}} onBackgroundAll={() => {}} />);
  const chip = screen.getByRole("button", { name: "Tasks" });
  expect(chip.hasAttribute("disabled")).toBe(false);
  expect(chip.querySelector(".sv-chip-badge")).toBeNull();
  fireEvent.click(chip);
  expect(screen.getByText("No background tasks")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Move the running work to the background" })).toBeNull();
});

it("draws disabled MCP and Tasks chips that open nothing and ask the backend nothing without a process", () => {
  const { rerender } = render(<>
    <McpChip sessionId="session-a" disabled />
    <TasksChip sessionId="s" tasks={[]} busy={false} disabled onStop={() => {}} onOpen={() => {}} onBackgroundAll={() => {}} />
  </>);
  const mcp = screen.getByRole("button", { name: "MCP" });
  const tasks = screen.getByRole("button", { name: "Tasks" });
  for (const chip of [mcp, tasks]) {
    expect(chip.hasAttribute("disabled")).toBe(true);
    expect(chip.getAttribute("aria-disabled")).toBe("true");
    expect(chip.getAttribute("title")).toBe("The agent process is not running. Send a message to start it.");
    fireEvent.click(chip);
    expect(chip.getAttribute("aria-expanded")).toBe("false");
  }
  expect(screen.queryByText("No background tasks")).toBeNull();
  expect(chatMcpStatus).not.toHaveBeenCalled();
  // The same chip becomes active in place once the process is there.
  vi.mocked(chatMcpStatus).mockResolvedValueOnce({ mcpServers: [] });
  rerender(<>
    <McpChip sessionId="session-a" />
    <TasksChip sessionId="s" tasks={[]} busy={false} onStop={() => {}} onOpen={() => {}} onBackgroundAll={() => {}} />
  </>);
  expect(mcp.hasAttribute("disabled")).toBe(false);
  expect(mcp.getAttribute("title")).toBe("MCP servers");
  fireEvent.click(mcp);
  expect(chatMcpStatus).toHaveBeenCalledWith("session-a");
});

it("closes an open popover when the chip becomes disabled", () => {
  const { rerender } = render(<TasksChip sessionId="s" tasks={[]} busy={false} onStop={() => {}} onOpen={() => {}} onBackgroundAll={() => {}} />);
  fireEvent.click(screen.getByRole("button", { name: "Tasks" }));
  expect(screen.getByText("No background tasks")).toBeTruthy();
  rerender(<TasksChip sessionId="s" tasks={[]} busy={false} disabled onStop={() => {}} onOpen={() => {}} onBackgroundAll={() => {}} />);
  expect(screen.queryByText("No background tasks")).toBeNull();
  expect(screen.getByRole("button", { name: "Tasks" }).getAttribute("aria-expanded")).toBe("false");
});

const tasks: ChatBackgroundTask[] = [
  { task_id: "w1", task_type: "local_workflow", description: "Beta: gamma-worker", summary: "protocol probe", status: "running", can_stop: true, finished: false },
  { task_id: "sh1", task_type: "local_bash", description: "npm test", status: "completed", can_stop: false, finished: true },
];

function renderTasks(onOpen = vi.fn(), onStop = vi.fn()) {
  render(<TasksChip sessionId="s" tasks={tasks} busy={false} onStop={onStop} onOpen={onOpen} onBackgroundAll={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: /Tasks/ }));
  return { onOpen, onStop };
}

it("lists the live description of each task and counts only the running ones", () => {
  renderTasks();
  expect(screen.getByText("Beta: gamma-worker")).toBeTruthy();
  expect(screen.getByText("npm test")).toBeTruthy();
  expect(screen.getByText("local bash · Completed")).toBeTruthy();
  expect(screen.getByText("1", { selector: ".sv-chip-badge" })).toBeTruthy();
});

it("opens the task behind a row, and Stop stops without opening", () => {
  const { onOpen, onStop } = renderTasks();
  fireEvent.click(screen.getByText("Beta: gamma-worker"));
  expect(onOpen).toHaveBeenCalledWith(tasks[0]);

  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  expect(onStop).toHaveBeenCalledWith("w1");
  expect(onOpen).toHaveBeenCalledTimes(1);
});

it("opens through a task link and stops through a separate command button", () => {
  const { onOpen, onStop } = renderTasks();
  const [open] = screen.getAllByTitle("Open task");
  const stop = screen.getByRole("button", { name: "Stop" });
  // Native buttons: the browser itself activates them on Enter and Space, which jsdom does not emulate,
  // so the element kind and the focus order are what is asserted here.
  expect(open.tagName).toBe("A");
  expect(open.getAttribute("href")).toContain("taskSession=s&taskId=w1");
  expect(stop.tagName).toBe("BUTTON");
  expect(open.contains(stop)).toBe(false);
  expect(stop.contains(open)).toBe(false);
  expect(open.parentElement).toBe(stop.parentElement);
  expect(open.parentElement!.getAttribute("role")).toBeNull();
  expect(open.querySelector("[role=button], button")).toBeNull();
  expect(stop.querySelector("[role=button], button")).toBeNull();

  open.focus();
  expect(document.activeElement).toBe(open);
  fireEvent.click(open);
  expect(onOpen).toHaveBeenCalledWith(tasks[0]);
  stop.focus();
  expect(document.activeElement).toBe(stop);
  fireEvent.click(stop);
  expect(onStop).toHaveBeenCalledWith("w1");
  expect(onOpen).toHaveBeenCalledTimes(1);
});

it("offers no Stop for a finished task, only its status", () => {
  renderTasks();
  expect(screen.getAllByRole("button", { name: "Stop" }).length).toBe(1);
  fireEvent.click(screen.getByText("npm test"));
  expect(screen.getByText("local bash · Completed")).toBeTruthy();
});


it("shows native session and turn usage even when context capacity is unknown", () => {
  render(<UsageMeter detailed extras={{nativeUsage:{scope:"session",inputTokens:120,outputTokens:12,cachedInputTokens:30},turnUsage:{inputTokens:80,outputTokens:8}}} />);
  fireEvent.click(screen.getByRole("button", {name:"Usage"}));
  expect(screen.getByText("Session total")).toBeTruthy();
  expect(screen.getByText("Current turn")).toBeTruthy();
  expect(screen.getByText("120")).toBeTruthy();
  expect(screen.getByText("30")).toBeTruthy();
  expect(screen.getByText("Unknown")).toBeTruthy();
  expect(screen.queryByText(/\$|200.?000/)).toBeNull();
});
