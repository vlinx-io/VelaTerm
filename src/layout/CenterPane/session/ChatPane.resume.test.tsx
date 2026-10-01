import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatSnapshot } from "../../../ipc/chat";
import type { Session } from "../../../types";

const share = vi.hoisted(() => ({ surface: false }));
vi.mock("../../../ipc/shareBase", async (original) => {
  const actual = await original<typeof import("../../../ipc/shareBase")>();
  return { ...actual, get isShareSurface() { return share.surface; } };
});
vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(), onTransportReconnect: vi.fn(),
}));
vi.mock("./engineSwitch", () => ({ useEngineSwitch: () => ({ switchTo: vi.fn(), confirm: null }) }));
vi.mock("./controls", () => ({ LevelBar: () => null, ControlChip: () => null }));

import { invoke, listen, onTransportReconnect } from "../../../ipc/transport";
import { useTermStore } from "../../../store/termStore";
import { ChatPane } from "./ChatPane";
import { useOutbox } from "./outbox";
import { clearChatCache } from "./chatCache";

let snapshotOverrides: Partial<ChatSnapshot>;

beforeEach(() => {
  share.surface = false;
  clearChatCache();
  useOutbox.setState({ sessions: {} });
  snapshotOverrides = {};
  vi.mocked(invoke).mockReset();
  vi.mocked(onTransportReconnect).mockImplementation(() => () => {});
  vi.mocked(listen).mockImplementation(() => Promise.resolve(() => {}));
  useTermStore.setState({ chatModel: "m", chatModelByKind: { claude: "m", codex: "m" }, chatEffortByModel: {}, runtimes: {}, agentDefaults: {} });
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "agent_permission_catalog") {
      return Promise.resolve({ modes: ["plan", "default", "acceptEdits", "auto", "bypassPermissions"], selected: "default" }) as Promise<never>;
    }
    if (command === "chat_snapshot") return Promise.resolve({
      submissionReceipts: true, running: false, model: "m", mode: "default", rows: [], queue: [], permissions: [], commands: [], configKeys: [], ...snapshotOverrides,
    }) as Promise<never>;
    if (command === "chat_models") return Promise.resolve([{ id: "m", label: "M", description: "", effortLevels: ["high"] }]) as Promise<never>;
    if (command === "chat_commands") return Promise.resolve([]) as Promise<never>;
    if (command === "chat_resume_list") return Promise.resolve({ directory: "/work", truncated: false, conversations: [] }) as Promise<never>;
    return Promise.resolve(command === "chat_send" ? "sent" : undefined) as Promise<never>;
  });
});
afterEach(cleanup);

async function mountPane(kind: Session["kind"] = "claude") {
  render(<ChatPane session={{ id: "s", projectId: "p", name: "Agent", kind, engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
    area={{}} hidden={false} focused multi={false} onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />);
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_snapshot")).toBe(true));
  await act(async () => {});
  return screen.getByRole("textbox") as HTMLTextAreaElement;
}

function submit(input: HTMLTextAreaElement, text: string) {
  fireEvent.change(input, { target: { value: text } });
  fireEvent.keyDown(input, { key: "Enter" });
}

const commands = () => vi.mocked(invoke).mock.calls.map(([command]) => command);
const EMPTY_ACTION = "Resume an earlier conversation";

describe("resuming an earlier conversation from the conversation view", () => {
  it("opens the picker for a typed /resume without sending anything to the agent", async () => {
    const input = await mountPane();
    submit(input, "/resume");
    expect(await screen.findByRole("dialog", { name: "Resume a conversation" })).toBeTruthy();
    await waitFor(() => expect(commands()).toContain("chat_resume_list"));
    expect(commands()).not.toContain("chat_send");
    expect(commands()).not.toContain("chat_start");
    expect(input.value).toBe("");
  });

  it("carries the text after /resume into the picker's search", async () => {
    const input = await mountPane();
    submit(input, "/resume flaky test");
    const search = await screen.findByRole("textbox", { name: "Search conversations" });
    expect((search as HTMLInputElement).value).toBe("flaky test");
    expect(commands()).not.toContain("chat_send");
  });

  it("offers /resume in the slash menu of a Claude session", async () => {
    const input = await mountPane();
    fireEvent.change(input, { target: { value: "/res" } });
    input.setSelectionRange(4, 4);
    fireEvent.select(input);
    expect(await screen.findByText("Continue an earlier Claude conversation from this directory")).toBeTruthy();
  });

  it("offers the picker in the empty state of a new Claude conversation", async () => {
    await mountPane();
    fireEvent.click(await screen.findByRole("button", { name: EMPTY_ACTION }));
    expect(await screen.findByRole("dialog", { name: "Resume a conversation" })).toBeTruthy();
  });

  it("refuses to open while messages wait to be delivered", async () => {
    snapshotOverrides = { queue: [{ id: "q1", text: "waiting", images: [] }] as unknown as ChatSnapshot["queue"] };
    const input = await mountPane();
    submit(input, "/resume");
    expect(await screen.findByText(
      "Wait until the agent is idle: no running turn, queued messages, shell command, background task or open permission request.",
    )).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(input.value).toBe("/resume");
  });

  it("has no entry for other agents, where /resume stays a message", async () => {
    const input = await mountPane("codex");
    expect(screen.queryByRole("button", { name: EMPTY_ACTION })).toBeNull();
    fireEvent.change(input, { target: { value: "/res" } });
    await act(async () => {});
    expect(screen.queryByText("Continue an earlier Claude conversation from this directory")).toBeNull();
    submit(input, "/resume");
    await waitFor(() => expect(commands()).toContain("chat_send"));
    const sent = vi.mocked(invoke).mock.calls.find(([command]) => command === "chat_send")?.[1] as { text?: string } | undefined;
    expect(sent?.text).toBe("/resume");
    expect(screen.queryByRole("dialog", { name: "Resume a conversation" })).toBeNull();
    expect(commands()).not.toContain("chat_resume_list");
  });

  it("has no entry in a read-only embedding", async () => {
    render(<ChatPane session={{ id: "s", projectId: "p", name: "Agent", kind: "claude", engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
      area={{}} hidden={false} focused multi={false} readOnly onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(commands()).toContain("chat_snapshot"));
    await act(async () => {});
    expect(screen.queryByRole("button", { name: EMPTY_ACTION })).toBeNull();
    expect(screen.queryByRole("dialog", { name: "Resume a conversation" })).toBeNull();
    expect(commands()).not.toContain("chat_resume_list");
  });

  it("has no entry on a share surface", async () => {
    share.surface = true;
    const input = await mountPane();
    expect(screen.queryByRole("button", { name: EMPTY_ACTION })).toBeNull();
    submit(input, "/resume");
    await act(async () => {});
    expect(screen.queryByRole("dialog", { name: "Resume a conversation" })).toBeNull();
    expect(commands()).not.toContain("chat_resume_list");
  });
});
