//! Spawn card tests for the model and effort a caller names on the command line: `vspawn --model/--effort`
//! must arrive preselected in the card, without overriding what the chosen agent can actually take.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SpawnRequest } from "../ipc/events";

const { agentListModels, store } = vi.hoisted(() => ({
  agentListModels: vi.fn().mockResolvedValue([] as string[]),
  store: {
    pendingSpawns: [] as SpawnRequest[],
    sessions: [] as { id: string; name: string; kind: string; agentArgs: string | null }[],
    agentDefaults: {} as Record<string, { args?: string }>,
    confirmSpawn: vi.fn(),
    cancelSpawn: vi.fn(),
  },
}));

vi.mock("../i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("../hooks/nativeViewSuspend", () => ({ useSuspendNativeViews: () => {} }));
vi.mock("../ipc/commands", () => ({ agentListModels }));
vi.mock("../store/termStore", () => {
  const useTermStore = (selector: (s: typeof store) => unknown) => selector(store);
  useTermStore.getState = () => store;
  return { useTermStore };
});

import { SpawnConfirmModal } from "./SpawnConfirmModal";

/** Queue one request from a Claude parent whose own launch arguments are `args`. */
function open(req: Partial<SpawnRequest>, args: string | null = null) {
  store.sessions = [{ id: "p1", name: "parent", kind: "claude", agentArgs: args }];
  store.pendingSpawns = [
    { parentSessionId: "p1", prompt: "do the thing", ...req } as SpawnRequest,
  ];
  render(<SpawnConfirmModal />);
}

/** The model box, identified by the placeholder the mocked translator returns. */
const modelBox = () =>
  screen.getByPlaceholderText("spawn.modelDefault") as HTMLInputElement;

/** Press Launch and return the request the card handed to the store. */
function launch(): SpawnRequest {
  fireEvent.click(screen.getByText("spawn.launch"));
  expect(store.confirmSpawn).toHaveBeenCalledTimes(1);
  return store.confirmSpawn.mock.calls[0][0] as SpawnRequest;
}

beforeEach(() => {
  vi.clearAllMocks();
  store.agentDefaults = {};
});

afterEach(cleanup);

describe("model and effort from the command line", () => {
  it("preserves the hidden invocation directory when launching", () => {
    open({ cwd: "/repo" });
    expect(launch().cwd).toBe("/repo");
  });

  it("prefills the card and launches with them", () => {
    open({ model: "opus[1m]", effort: "high" });
    expect(modelBox().value).toBe("opus[1m]");
    const sent = launch();
    expect(sent.model).toBe("opus[1m]");
    expect(sent.effort).toBe("high");
  });

  it("wins over the model inherited from the parent", () => {
    open({ model: "haiku" }, "--model sonnet --permission-mode plan");
    expect(modelBox().value).toBe("haiku");
    expect(launch().model).toBe("haiku");
  });

  it("still inherits the parent's model when the request names none", () => {
    open({}, "--model sonnet");
    expect(modelBox().value).toBe("sonnet");
    expect(launch().model).toBe("sonnet");
  });

  it("ignores an effort level this agent does not document", () => {
    // The selector holds only the levels the agent's CLI accepts, so an unknown one has nowhere to
    // show; the inherited level stands instead of the card silently clearing it.
    open({ effort: "turbo" }, "--effort low");
    expect(launch().effort).toBe("low");
  });

  it("drops an effort meant for an agent whose CLI has none", () => {
    open({ kind: "codex", model: "gpt-5.5", effort: "high" });
    const sent = launch();
    expect(sent.model).toBe("gpt-5.5");
    expect(sent.effort).toBeNull();
  });
});
