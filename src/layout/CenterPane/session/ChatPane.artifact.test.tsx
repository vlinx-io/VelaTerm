//! The conversation side of artifact previews: `vartifact page.html` from this session opens the side panel,
//! and "Send to session" puts text in the composer without sending it.

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(), invoke: vi.fn(), listen: vi.fn(), onTransportReconnect: vi.fn(),
}));
vi.mock("./engineSwitch", () => ({ useEngineSwitch: () => ({ switchTo: vi.fn(), confirm: null }) }));
vi.mock("./controls", () => ({ LevelBar: () => null, ControlChip: () => null }));

import { invoke, listen, onTransportReconnect } from "../../../ipc/transport";
import { useTermStore } from "../../../store/termStore";
import { insertIntoComposer, openBesideSession } from "../doc/artifact/artifactWriters";
import { ChatPane } from "./ChatPane";
import { useOutbox } from "./outbox";
import { clearChatCache } from "./chatCache";

const PAGE = "/p/site/index.html";

beforeEach(() => {
  clearChatCache();
  useOutbox.setState({ sessions: {} });
  vi.mocked(invoke).mockReset();
  vi.mocked(onTransportReconnect).mockImplementation(() => () => {});
  vi.mocked(listen).mockImplementation(() => Promise.resolve(() => {}));
  useTermStore.setState({ chatModel: "m", chatModelByKind: { claude: "m" }, chatEffortByModel: {}, runtimes: {}, agentDefaults: {} });
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "chat_snapshot") return Promise.resolve({
      submissionReceipts: true, running: true, model: "m", mode: "default", rows: [], queue: [], permissions: [], commands: [], configKeys: [],
    }) as Promise<never>;
    if (command === "chat_models") return Promise.resolve([{ id: "m", label: "M", description: "", effortLevels: ["high"] }]) as Promise<never>;
    if (command === "artifact_grant") return Promise.resolve({ grantId: "g1", entry: "index.html", root: "/p/site" }) as Promise<never>;
    if (command === "artifact_versions" || command === "artifact_snapshot") return Promise.resolve([]) as Promise<never>;
    if (command === "fs_watch") return Promise.resolve({ root: "/p/site", supported: true }) as Promise<never>;
    return Promise.resolve(undefined) as Promise<never>;
  });
});
afterEach(cleanup);

async function mountPane(hidden = false) {
  render(<ChatPane session={{ id: "s", projectId: "p", name: "Claude", kind: "claude", engine: "chat", collapsed: false, sortOrder: 0, createdAt: 0 }}
    area={{}} hidden={hidden} focused multi={false} onActivate={() => {}} onSplit={() => {}} onClose={() => {}} />);
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "chat_snapshot")).toBe(true));
  await act(async () => {});
}

describe("artifact panel beside a conversation", () => {
  it("opens when this session asks, with a sandboxed frame on the grant", async () => {
    await mountPane();
    let taken = false;
    await act(async () => { taken = openBesideSession("s", PAGE); });
    expect(taken).toBe(true);
    const frame = await waitFor(() => {
      const el = document.querySelector(".artifact-dock iframe") as HTMLIFrameElement | null;
      expect(el).toBeTruthy();
      return el!;
    });
    expect(frame.getAttribute("sandbox")).toBe("allow-scripts allow-forms allow-modals");
    expect(frame.src).toContain("/g1/index.html");
    expect(vi.mocked(invoke).mock.calls.find(([command]) => command === "artifact_grant")?.[1]).toMatchObject({ path: PAGE, net: false });
  });

  it("opens other files beside it in the document view, and closing the panel drops the document", async () => {
    await mountPane();
    let taken = false;
    await act(async () => { taken = openBesideSession("s", "/p/site/notes.md"); });
    expect(taken).toBe(true);
    await waitFor(() => expect(document.querySelector(".dock-doc-body[data-dock-doc-id]")).toBeTruthy());
    const docs = Object.values(useTermStore.getState().dockDocs);
    expect(docs).toHaveLength(1);
    expect(docs[0]).toMatchObject({ path: "/p/site/notes.md", kind: "markdown" });
    expect(useTermStore.getState().openTabs).not.toContain(docs[0].id);
    await act(async () => screen.getByLabelText("Close preview").click());
    await waitFor(() => expect(document.querySelector(".artifact-dock")).toBeNull());
    expect(useTermStore.getState().dockDocs).toEqual({});
  });

  it("leaves other sessions' pages and hidden panes to open as tabs", async () => {
    await mountPane();
    expect(openBesideSession("other", PAGE)).toBe(false);
    cleanup();
    await mountPane(true);
    expect(openBesideSession("s", PAGE)).toBe(false);
    expect(document.querySelector(".artifact-dock")).toBeNull();
  });

  it("puts a console report in the composer without sending it", async () => {
    await mountPane();
    const input = screen.getByRole("textbox") as HTMLTextAreaElement;
    await act(async () => insertIntoComposer("s", "Please fix the errors."));
    expect(input.value).toBe("Please fix the errors.");
    await act(async () => insertIntoComposer("other", "not for this pane"));
    expect(input.value).toBe("Please fix the errors.");
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).not.toContain("chat_send");
  });
});
