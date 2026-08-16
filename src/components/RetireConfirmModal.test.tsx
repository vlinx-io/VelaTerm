//! Approve and decline each answer their own request id, after the card lists every worktree it deletes.

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { RetireRequest } from "../ipc/events";

const { commands, events, notify, store } = vi.hoisted(() => ({
  commands: { retireResult: vi.fn().mockResolvedValue(true) },
  events: { onRetireRequest: vi.fn(), onRetireCancel: vi.fn(), onRetireResolved: vi.fn() },
  notify: { notify: vi.fn().mockResolvedValue(undefined) },
  store: { notifyEnabled: true, soundEnabled: false },
}));

vi.mock("../i18n", () => ({
  useT: () => (key: string) => key,
  t: (key: string) => key,
}));
vi.mock("../hooks/nativeViewSuspend", () => ({ useSuspendNativeViews: () => {} }));
vi.mock("../ipc/commands", () => commands);
vi.mock("../ipc/events", () => events);
vi.mock("../notify", () => notify);
vi.mock("../store/termStore", () => ({
  useTermStore: { getState: () => store },
}));

import { RetireConfirmModal } from "./RetireConfirmModal";

/** Deliver a backend request to the listener the card registered on mount. */
let emit: (req: RetireRequest) => void;
/** Withdraw one request id, as the backend does when its handler times out. */
let expire: (requestId: string) => void;
/** Report one request id answered by another client, as the resolved broadcast does. */
let resolve: (requestId: string) => void;

const CLEANUP: RetireRequest = {
  requestId: "req-1",
  sessionId: "sess-1",
  name: "worker one",
  action: "cleanup-and-archive",
  descendantCount: 2,
  worktrees: [
    {
      id: "sess-1",
      name: "worker one",
      path: "/repo/.vlx-worktrees/worker-one",
      branch: "vlx/worker-one",
      targetCommit: "abc123",
    },
    {
      id: "sess-2",
      name: "worker two",
      path: "/repo/.vlx-worktrees/worker-two",
      branch: "vlx/worker-two",
      resumed: true,
    },
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  commands.retireResult.mockResolvedValue(true);
  events.onRetireRequest.mockImplementation((cb: (req: RetireRequest) => void) => {
    emit = cb;
    return Promise.resolve(() => {});
  });
  events.onRetireCancel.mockImplementation((cb: (requestId: string) => void) => {
    expire = cb;
    return Promise.resolve(() => {});
  });
  events.onRetireResolved.mockImplementation((cb: (requestId: string) => void) => {
    resolve = cb;
    return Promise.resolve(() => {});
  });
});

afterEach(cleanup);

/** Render and deliver one request so the card is on screen. */
async function open(req: RetireRequest = CLEANUP) {
  render(<RetireConfirmModal />);
  await waitFor(() => expect(events.onRetireRequest).toHaveBeenCalled());
  act(() => emit(req));
  await screen.findByRole("dialog");
}

describe("RetireConfirmModal", () => {
  it("stays hidden until the backend asks", async () => {
    render(<RetireConfirmModal />);
    await waitFor(() => expect(events.onRetireRequest).toHaveBeenCalled());
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("lists every worktree path and branch before the user can approve", async () => {
    await open();
    for (const worktree of CLEANUP.worktrees) {
      expect(screen.getByText(new RegExp(worktree.path))).toBeTruthy();
      expect(screen.getByText(new RegExp(worktree.branch as string))).toBeTruthy();
    }
    expect(screen.getByRole("alert")).toBeTruthy();
    expect(commands.retireResult).not.toHaveBeenCalled();
  });

  it("approves the request and closes the card", async () => {
    await open();
    fireEvent.click(screen.getByText("retire.approve"));
    expect(commands.retireResult).toHaveBeenCalledWith("req-1", {
      approved: true,
      error: undefined,
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("declines with a reason so the parked request fails instead of waiting out its timeout", async () => {
    await open();
    fireEvent.click(screen.getByText("retire.keep"));
    expect(commands.retireResult).toHaveBeenCalledWith("req-1", {
      approved: false,
      error: "the user declined the retire",
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("shows an archive-only plan without the deletion warning", async () => {
    await open({
      requestId: "req-2",
      sessionId: "sess-9",
      name: "settled worker",
      action: "archive",
      descendantCount: 0,
      worktrees: [],
    });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText("retire.actionArchive")).toBeTruthy();
  });

  it("answers each queued request with its own id", async () => {
    await open();
    act(() =>
      emit({
        requestId: "req-3",
        sessionId: "sess-3",
        name: "worker three",
        action: "archive",
        descendantCount: 0,
        worktrees: [],
      }),
    );
    fireEvent.click(screen.getByText("retire.approve"));
    await screen.findByText(/worker three/);
    fireEvent.click(screen.getByText("retire.keep"));

    expect(commands.retireResult.mock.calls[0][0]).toBe("req-1");
    expect(commands.retireResult.mock.calls[1][0]).toBe("req-3");
    expect(commands.retireResult.mock.calls[1][1].approved).toBe(false);
  });

  it("raises a notification because the card blocks its caller in the background", async () => {
    await open();
    expect(notify.notify).toHaveBeenCalledWith(
      "sess-1",
      "retire.notifyTitle",
      expect.stringContaining("worker one"),
      false,
    );
  });

  it("stays silent when notifications are off", async () => {
    store.notifyEnabled = false;
    await open();
    expect(notify.notify).not.toHaveBeenCalled();
    store.notifyEnabled = true;
  });

  it("withdraws an expired card so its approval cannot silently do nothing", async () => {
    await open();
    act(() => expire("req-1"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(commands.retireResult).not.toHaveBeenCalled();
  });

  it("closes a card another client already answered without sending a dead answer", async () => {
    await open();
    act(() => resolve("req-1"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(commands.retireResult).not.toHaveBeenCalled();
  });

  it("keeps a queued card that a different request withdraws", async () => {
    await open();
    act(() => expire("req-9"));
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("closes the card even when the answer call rejects", async () => {
    commands.retireResult.mockRejectedValue(new Error("waiter timed out"));
    await open();
    fireEvent.click(screen.getByText("retire.approve"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});
