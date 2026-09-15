import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../../../ipc/chat", () => ({
  chatMcpStatus: vi.fn(), chatMcpToggle: vi.fn(), chatMcpReconnect: vi.fn(),
}));

import { chatMcpStatus, chatMcpToggle } from "../../../ipc/chat";
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

it("keeps the Tasks chip in the row with an empty task list and says so when opened", () => {
  render(<TasksChip tasks={[]} busy={false} onStop={() => {}} onBackgroundAll={() => {}} />);
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
    <TasksChip tasks={[]} busy={false} disabled onStop={() => {}} onBackgroundAll={() => {}} />
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
    <TasksChip tasks={[]} busy={false} onStop={() => {}} onBackgroundAll={() => {}} />
  </>);
  expect(mcp.hasAttribute("disabled")).toBe(false);
  expect(mcp.getAttribute("title")).toBe("MCP servers");
  fireEvent.click(mcp);
  expect(chatMcpStatus).toHaveBeenCalledWith("session-a");
});

it("closes an open popover when the chip becomes disabled", () => {
  const { rerender } = render(<TasksChip tasks={[]} busy={false} onStop={() => {}} onBackgroundAll={() => {}} />);
  fireEvent.click(screen.getByRole("button", { name: "Tasks" }));
  expect(screen.getByText("No background tasks")).toBeTruthy();
  rerender(<TasksChip tasks={[]} busy={false} disabled onStop={() => {}} onBackgroundAll={() => {}} />);
  expect(screen.queryByText("No background tasks")).toBeNull();
  expect(screen.getByRole("button", { name: "Tasks" }).getAttribute("aria-expanded")).toBe("false");
});
