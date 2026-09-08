import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(),
  invoke: vi.fn(),
  listen: vi.fn(),
}));

import { chatSnapshot } from "../../../ipc/chat";
import { invoke } from "../../../ipc/transport";
import { formatTurnDuration, MessageBubble, WorkingRow } from "./rows";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});
beforeEach(() => vi.mocked(invoke).mockReset());

describe("message rewind menu", () => {
  it("shows every Claude rewind scope and reports the selected one", () => {
    const onRewind = vi.fn();
    render(
      <MessageBubble
        who="You"
        isUser
        text="Change this request"
        onRewind={onRewind}
        rewindScopes={["conversation", "files", "both"]}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Rewind from here" }));
    fireEvent.click(screen.getByRole("button", { name: "Rewind conversation and restore files" }));

    expect(onRewind).toHaveBeenCalledOnce();
    expect(onRewind).toHaveBeenCalledWith("both");
  });

  it("does not offer file rollback when only conversation rewind is supported", () => {
    render(
      <MessageBubble
        who="You"
        isUser
        text="Change this request"
        onRewind={() => {}}
        rewindScopes={["conversation"]}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Rewind from here" }));
    expect(screen.getByRole("button", { name: "Rewind conversation" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Restore files" })).toBeNull();
  });
});

describe("snapshot-backed message images", () => {
  it("fetches image bytes only after the bubble containing the reference mounts", async () => {
    vi.mocked(invoke).mockImplementation((command) => {
      if (command === "chat_snapshot") {
        return Promise.resolve({
          running: true,
          startedAt: 42,
          rows: [{
            kind: "user",
            id: "u-image",
            text: "look",
            images: [{ mimeType: "image/png", attachmentId: "row:u-image:0" }],
          }],
          queue: [],
          permissions: [],
          commands: [],
          configKeys: [],
        }) as Promise<never>;
      }
      return Promise.resolve({ mimeType: "image/png", data: "UE5H" }) as Promise<never>;
    });

    const snapshot = await chatSnapshot("mount-session");
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual(["chat_snapshot"]);
    const row = snapshot.rows[0];
    if (row?.kind !== "user") throw new Error("Snapshot fixture did not produce a user row");

    const { container } = render(
      <MessageBubble who="You" isUser text={row.text} images={row.images} />,
    );
    await waitFor(() => {
      expect(container.querySelector<HTMLImageElement>(".sv-msg-image")?.src).toBe(
        "data:image/png;base64,UE5H",
      );
    });
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
      "chat_snapshot",
      "chat_attachment",
    ]);
  });
});

describe("turn duration", () => {
  it("shows a completed duration only on an assistant answer", () => {
    const { container, rerender } = render(
      <MessageBubble who="Claude" isUser={false} text="Done" durationMs={78_123} />,
    );
    expect(screen.getByText("· 1m 18s")).toBeTruthy();

    rerender(<MessageBubble who="You" isUser text="Question" durationMs={78_123} />);
    expect(container.querySelector(".sv-msg-duration")).toBeNull();
  });

  it("updates the active turn clock once per second and stops at whole seconds", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(100_000));
    render(<WorkingRow startedAt={95_000} />);
    expect(screen.getByText("5s")).toBeTruthy();

    act(() => vi.advanceTimersByTime(2_000));
    expect(screen.getByText("7s")).toBeTruthy();
    expect(formatTurnDuration(3_661_999)).toBe("1h 1m 1s");
  });

  it("keeps the existing working indicator when no start time is available", () => {
    const { container } = render(<WorkingRow />);
    expect(container.querySelector(".sv-working-time")).toBeNull();
  });
});
