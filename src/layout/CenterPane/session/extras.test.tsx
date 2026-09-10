import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../../../ipc/chat", async (original) => ({
  ...await original<typeof import("../../../ipc/chat")>(),
  chatMcpStatus: vi.fn(), chatMcpToggle: vi.fn(), chatMcpReconnect: vi.fn(),
}));

import { chatMcpStatus, chatMcpToggle, type ChatBackgroundTask } from "../../../ipc/chat";
import { setLang } from "../../../i18n";
import { McpChip, TasksChip } from "./extras";

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

const tasks: ChatBackgroundTask[] = [
  { task_id: "w1", task_type: "local_workflow", description: "Beta: gamma-worker", summary: "protocol probe", status: "running" },
  { task_id: "sh1", task_type: "local_bash", description: "npm test", status: "completed" },
];

function renderTasks(onOpen = vi.fn(), onStop = vi.fn()) {
  render(<TasksChip tasks={tasks} busy={false} onStop={onStop} onOpen={onOpen} onBackgroundAll={vi.fn()} />);
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

it("offers no Stop for a finished task, only its status", () => {
  renderTasks();
  expect(screen.getAllByRole("button", { name: "Stop" }).length).toBe(1);
  fireEvent.click(screen.getByText("npm test"));
  expect(screen.getByText("local bash · Completed")).toBeTruthy();
});
