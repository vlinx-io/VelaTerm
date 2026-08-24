//! Quit-dialog tests: the handshake with the shell, and the rule that the workspace is written only when the
//! user ticks the box — and always before the exit is approved, since the process dies as soon as it is.

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { flushNow, quit, store } = vi.hoisted(() => ({
  flushNow: vi.fn().mockResolvedValue(undefined),
  quit: {
    onRequested: vi.fn(),
    ack: vi.fn().mockResolvedValue(undefined),
    confirm: vi.fn().mockResolvedValue(undefined),
    cancel: vi.fn().mockResolvedValue(undefined),
  },
  store: {
    saveWorkspaceOnQuit: false,
    setSaveWorkspaceOnQuit: vi.fn(),
    saveWorkspaceSnapshot: vi.fn(),
  },
}));

vi.mock("../i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("../platform", () => ({ platform: { quit } }));
vi.mock("../ipc/settingsSync", () => ({ flushNow }));
vi.mock("../hooks/nativeViewSuspend", () => ({ useSuspendNativeViews: () => {} }));
vi.mock("../store/termStore", () => {
  const useTermStore = (selector: (s: typeof store) => unknown) => selector(store);
  useTermStore.getState = () => store;
  return { useTermStore };
});

import { QuitConfirmModal } from "./QuitConfirmModal";

/** Fire the shell's exit request captured by the component's listener. */
let fireQuitRequest: () => void;

beforeEach(() => {
  vi.clearAllMocks();
  store.saveWorkspaceOnQuit = false;
  quit.onRequested.mockImplementation((cb: () => void) => {
    fireQuitRequest = cb;
    return Promise.resolve(() => {});
  });
});

afterEach(cleanup);

/** Approve the exit and let confirm()'s awaited settings flush settle before asserting. */
async function confirmExit() {
  fireEvent.click(screen.getByText("quit.confirm"));
  await waitFor(() => expect(quit.confirm).toHaveBeenCalled());
}

/** Render and drive the shell request so the dialog is on screen. */
async function open() {
  render(<QuitConfirmModal />);
  await waitFor(() => expect(quit.onRequested).toHaveBeenCalled());
  act(() => fireQuitRequest());
  await screen.findByText("quit.title");
}

describe("QuitConfirmModal", () => {
  it("stays hidden until the shell asks", async () => {
    render(<QuitConfirmModal />);
    await waitFor(() => expect(quit.onRequested).toHaveBeenCalled());
    expect(screen.queryByText("quit.title")).toBeNull();
  });

  it("acknowledges immediately so the shell does not fall back to its native dialog", async () => {
    await open();
    expect(quit.ack).toHaveBeenCalled();
    expect(quit.confirm).not.toHaveBeenCalled();
  });

  it("cancels without saving or exiting", async () => {
    await open();
    fireEvent.click(screen.getByText("common.cancel"));
    expect(quit.cancel).toHaveBeenCalled();
    expect(quit.confirm).not.toHaveBeenCalled();
    expect(store.saveWorkspaceSnapshot).not.toHaveBeenCalled();
  });

  it("exits without saving when the box is unticked", async () => {
    await open();
    await confirmExit();
    expect(store.saveWorkspaceSnapshot).not.toHaveBeenCalled();
    expect(quit.confirm).toHaveBeenCalled();
  });

  it("writes the snapshot before approving the exit when ticked", async () => {
    await open();
    fireEvent.click(screen.getByRole("checkbox"));
    await confirmExit();

    expect(store.saveWorkspaceSnapshot).toHaveBeenCalled();
    expect(quit.confirm).toHaveBeenCalled();
    // The process dies as soon as the exit is approved, so ordering is the whole point of this test.
    expect(store.saveWorkspaceSnapshot.mock.invocationCallOrder[0]).toBeLessThan(
      quit.confirm.mock.invocationCallOrder[0],
    );
  });

  it("remembers the choice as the next exit's default", async () => {
    await open();
    fireEvent.click(screen.getByRole("checkbox"));
    await confirmExit();
    expect(store.setSaveWorkspaceOnQuit).toHaveBeenCalledWith(true);
  });

  // Settings writes are debounced by 400 ms and the process dies well inside that window. Startup
  // reconciliation then treats the backend as authoritative, so an unflushed answer is not merely lost:
  // it overwrites the value already in localStorage, and the box comes back unticked every launch.
  it("flushes the remembered choice to the backend before approving the exit", async () => {
    await open();
    fireEvent.click(screen.getByRole("checkbox"));
    await confirmExit();

    expect(flushNow).toHaveBeenCalled();
    expect(flushNow.mock.invocationCallOrder[0]).toBeGreaterThan(
      store.setSaveWorkspaceOnQuit.mock.invocationCallOrder[0],
    );
    expect(flushNow.mock.invocationCallOrder[0]).toBeLessThan(
      quit.confirm.mock.invocationCallOrder[0],
    );
  });

  // Call order alone does not prove the fix: a fire-and-forget `flushNow()` satisfies every assertion above
  // while still racing termination, which is the whole bug. Hold the flush unresolved and require that the
  // exit stays unapproved until it lands, so dropping the await fails here.
  it("does not approve the exit until the flush has landed", async () => {
    let landFlush: () => void = () => {};
    flushNow.mockImplementationOnce(
      () => new Promise<void>((resolve) => { landFlush = resolve; }),
    );
    await open();
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(screen.getByText("quit.confirm"));

    await waitFor(() => expect(flushNow).toHaveBeenCalled());
    expect(quit.confirm).not.toHaveBeenCalled();

    await act(async () => landFlush());
    await waitFor(() => expect(quit.confirm).toHaveBeenCalled());
  });

  it("preticks the box from the remembered preference", async () => {
    store.saveWorkspaceOnQuit = true;
    await open();
    expect((screen.getByRole("checkbox") as HTMLInputElement).checked).toBe(true);
  });
});
