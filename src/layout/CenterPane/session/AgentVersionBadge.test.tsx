import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Session } from "../../../types";
import type { AgentVersion } from "../../../ipc/agentVersion";

const state = vi.hoisted(() => ({
  agentDefaults: {} as Record<string, { path?: string }>,
  runtimes: {} as Record<string, { startedAt?: number }>,
}));
vi.mock("../../../store/termStore", () => ({ useTermStore: (select: (value: typeof state) => unknown) => select(state) }));
vi.mock("../../../ipc/agentVersion", () => ({ agentVersion: vi.fn(), agentUpdateStart: vi.fn(), agentUpdateStatus: vi.fn() }));
vi.mock("../../../ipc/chat", () => ({ chatDetach: vi.fn() }));
vi.mock("../../../ipc/info", () => ({ copyText: vi.fn() }));
vi.mock("../../../platform", () => ({ platform: { opener: { openExternal: vi.fn() } } }));
vi.mock("../../../ipc/transport", () => ({ onTransportReconnect: () => () => {} }));
import { agentUpdateStart, agentUpdateStatus, agentVersion, type AgentUpdateStatus } from "../../../ipc/agentVersion";
import { chatDetach } from "../../../ipc/chat";
import { setLang } from "../../../i18n";
import { AgentVersionBadge } from "./AgentVersionBadge";

const session = { id: "one", kind: "codex", engine: "chat" } as Session;
const installed: AgentVersion = {
  version: "1.9.0", running: false, latestVersion: null, updateAvailable: false,
  checkStatus: "unchecked", docsUrl: "https://developers.openai.com/codex/cli", updateCommand: null,
};
const idle: AgentUpdateStatus = { state: "idle", output: "", exitCode: null, timedOut: false, version: null };

beforeEach(() => {
  vi.resetAllMocks();
  state.agentDefaults = {};
  state.runtimes = {};
  setLang("en");
  vi.mocked(agentVersion).mockResolvedValue(installed);
});
afterEach(cleanup);

it("shows the selected CLI version and a visible update link to official instructions", async () => {
  let release!: (value: AgentVersion) => void;
  vi.mocked(agentVersion).mockResolvedValueOnce(installed)
    .mockImplementationOnce(() => new Promise(resolve => { release = resolve; }));
  render(<AgentVersionBadge session={session} hidden={false} />);
  await screen.findByText("v1.9.0");
  expect(screen.getByTitle("Installed version: 1.9.0")).toBeTruthy();
  expect(screen.queryByRole("link")).toBeNull();
  await act(async () => release({ ...installed, latestVersion: "1.10.0", updateAvailable: true, checkStatus: "checked" }));
  const link = screen.getByRole("link", { name: "Update: v1.10.0" });
  expect(link.getAttribute("href")).toBe(installed.docsUrl);
  expect(link.getAttribute("title")).toBe("Version 1.10.0 is available. Open the official upgrade instructions.");
  expect(screen.getByTitle("Installed version: 1.9.0")).toBeTruthy();
  expect(screen.getByRole("status").textContent).toBe("v1.9.0Update: v1.10.0");
  expect(agentVersion).toHaveBeenNthCalledWith(2, "one", true);
});

it("names the running version and offers no update when the server says it is current", async () => {
  vi.mocked(agentVersion).mockResolvedValue({ ...installed, running: true, latestVersion: "1.9.0", checkStatus: "checked" });
  render(<AgentVersionBadge session={session} hidden={false} />);
  await screen.findByTitle("Running version: 1.9.0");
  expect(screen.queryByRole("link")).toBeNull();
});

it("keeps the current version and reports a failed release check", async () => {
  vi.mocked(agentVersion).mockResolvedValueOnce(installed).mockResolvedValueOnce({ ...installed, checkStatus: "failed" });
  render(<AgentVersionBadge session={session} hidden={false} />);
  await screen.findByTitle("Installed version: 1.9.0\nUnable to check for updates.", { normalizer: value => value });
  expect(screen.getByRole("status").textContent).toBe("v1.9.0");
  expect(screen.queryByRole("link")).toBeNull();
});

it("still shows the update when no official instructions are known", async () => {
  vi.mocked(agentVersion).mockResolvedValue({ ...installed, docsUrl: null, latestVersion: "1.10.0", updateAvailable: true, checkStatus: "checked" });
  render(<AgentVersionBadge session={session} hidden={false} />);
  await screen.findByText("Update: v1.10.0");
  expect(screen.queryByRole("link")).toBeNull();
});

it("reports an unavailable executable without inventing a version or an update", async () => {
  vi.mocked(agentVersion).mockResolvedValue({ ...installed, version: null });
  render(<AgentVersionBadge session={session} hidden={false} />);
  await screen.findByText("Version unavailable");
  expect(screen.queryByRole("link")).toBeNull();
});

it("ignores a delayed result from the previously selected session", async () => {
  let first!: (value: AgentVersion) => void;
  vi.mocked(agentVersion).mockImplementationOnce(() => new Promise(resolve => { first = resolve; }));
  const view = render(<AgentVersionBadge session={session} hidden={false} />);
  view.rerender(<AgentVersionBadge session={{ ...session, id: "two" }} hidden={false} />);
  await screen.findByText("v1.9.0");
  await act(async () => first({ ...installed, version: "9.0.0", latestVersion: "10.0.0", updateAvailable: true }));
  expect(screen.queryByText("v9.0.0")).toBeNull();
  expect(screen.queryByRole("link")).toBeNull();
});

it("checks a newly visible pane and refreshes after the process restarts", async () => {
  const view = render(<AgentVersionBadge session={session} hidden />);
  expect(agentVersion).not.toHaveBeenCalled();
  view.rerender(<AgentVersionBadge session={session} hidden={false} />);
  await waitFor(() => expect(agentVersion).toHaveBeenCalledTimes(2));
  state.runtimes.one = { startedAt: 100 };
  view.rerender(<AgentVersionBadge session={session} hidden={false} />);
  await waitFor(() => expect(agentVersion).toHaveBeenCalledTimes(4));
});

it("confirms the self-update command, runs it, and reads the version again", async () => {
  const available = { ...installed, latestVersion: "1.10.0", updateAvailable: true, checkStatus: "checked" as const, updateCommand: "/opt/bin/codex update" };
  vi.mocked(agentVersion).mockResolvedValue(available);
  vi.mocked(agentUpdateStatus).mockResolvedValueOnce(idle)
    .mockResolvedValue({ ...idle, state: "succeeded", output: "Updated\n", exitCode: 0, version: "1.10.0" });
  vi.mocked(agentUpdateStart).mockResolvedValue({ ...idle, state: "running", output: "Downloading\n" });
  render(<AgentVersionBadge session={session} hidden={false} />);
  const button = await screen.findByRole("button", { name: "Update: v1.10.0" });
  expect(button.getAttribute("title")).toBe("Version 1.10.0 is available. Click to update.");
  expect(screen.queryByRole("link")).toBeNull();
  fireEvent.click(button);
  const dialog = await screen.findByRole("dialog", { name: "Update Codex" });
  expect(dialog.textContent).toContain("/opt/bin/codex update");
  expect(dialog.textContent).toContain("v1.9.0 → v1.10.0");
  expect(agentUpdateStart).not.toHaveBeenCalled();
  const calls = vi.mocked(agentVersion).mock.calls.length;
  fireEvent.click(await screen.findByRole("button", { name: "Update" }));
  await screen.findByText(/Downloading/);
  await screen.findByText("Codex has been updated to v1.10.0.", undefined, { timeout: 3000 });
  expect(agentUpdateStart).toHaveBeenCalledWith("one");
  await waitFor(() => expect(vi.mocked(agentVersion).mock.calls.length).toBeGreaterThan(calls));
  expect(screen.queryByRole("button", { name: "Restart agent" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(screen.queryByRole("dialog")).toBeNull();
});

it("offers to restart a running conversation after its agent was updated", async () => {
  vi.mocked(agentVersion).mockResolvedValue({ ...installed, running: true, latestVersion: "1.10.0", updateAvailable: true, checkStatus: "checked", updateCommand: "codex update" });
  vi.mocked(agentUpdateStatus).mockResolvedValue(idle);
  vi.mocked(agentUpdateStart).mockResolvedValue({ ...idle, state: "succeeded", exitCode: 0, version: "1.10.0" });
  vi.mocked(chatDetach).mockResolvedValue();
  render(<AgentVersionBadge session={session} hidden={false} />);
  fireEvent.click(await screen.findByRole("button", { name: "Update: v1.10.0" }));
  fireEvent.click(await screen.findByRole("button", { name: "Update" }));
  fireEvent.click(await screen.findByRole("button", { name: "Restart agent" }));
  expect(chatDetach).toHaveBeenCalledWith("one");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
});

it("reports a failed update with its output", async () => {
  vi.mocked(agentVersion).mockResolvedValue({ ...installed, latestVersion: "1.10.0", updateAvailable: true, checkStatus: "checked", updateCommand: "codex update" });
  vi.mocked(agentUpdateStatus).mockResolvedValue(idle);
  vi.mocked(agentUpdateStart).mockResolvedValue({ ...idle, state: "failed", output: "EACCES: permission denied\n", exitCode: 243, version: "1.9.0" });
  render(<AgentVersionBadge session={session} hidden={false} />);
  fireEvent.click(await screen.findByRole("button", { name: "Update: v1.10.0" }));
  fireEvent.click(await screen.findByRole("button", { name: "Update" }));
  await screen.findByText("The update did not complete. Check the output above, or copy the command and run it in a terminal.");
  expect(screen.getByRole("dialog").textContent).toContain("EACCES: permission denied");
});
