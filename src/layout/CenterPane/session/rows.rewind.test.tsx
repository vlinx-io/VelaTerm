import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(),
  invoke: vi.fn(),
  listen: vi.fn(),
}));

import { chatSnapshot } from "../../../ipc/chat";
import { invoke } from "../../../ipc/transport";
import { env } from "../../../platform/env";
import { imageFromNativeClipboard } from "../../../terminal/imageInput";
import { MAX_IMAGE_BYTES, MAX_IMAGES } from "./attachments";
import { formatTurnDuration, MessageBubble, WorkingRow } from "./rows";

vi.mock("../../../platform/env", () => ({ env: { isTauri: false, isElectron: false, isWeb: true } }));
vi.mock("../../../terminal/imageInput", async original => ({
  ...await original<typeof import("../../../terminal/imageInput")>(), imageFromNativeClipboard: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  env.isTauri = false;
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

describe("message rewind guide", () => {
  it("explains terminal rewind on both buttons, opens the guide, runs the action, and closes on Escape", () => {
    const onAction = vi.fn();
    const guide = {
      title: "Rewind in terminal view",
      tooltip: "Switch to terminal view and run /rewind.",
      description: "Use /rewind in terminal view to rewind the conversation or restore files.",
      actionLabel: "Switch to terminal view",
      onAction,
    };
    render(<MessageBubble who="You" isUser text="Previous turn" rewindGuide={guide} />);

    const rewindBtn = screen.getByRole("button", { name: "Rewind from here" }) as HTMLButtonElement;
    const editBtn = screen.getByRole("button", { name: "Edit" }) as HTMLButtonElement;
    expect(rewindBtn.disabled).toBe(false);
    expect(editBtn.disabled).toBe(false);
    expect(rewindBtn.title).toBe(guide.tooltip);
    expect(editBtn.title).toBe(guide.tooltip);

    fireEvent.click(rewindBtn);
    expect(screen.getByRole("dialog", { name: guide.title })).toBeTruthy();
    expect(rewindBtn.getAttribute("aria-expanded")).toBe("true");
    expect(editBtn.getAttribute("aria-expanded")).toBe("false");

    fireEvent.click(editBtn);
    expect(screen.getByRole("dialog", { name: guide.title })).toBeTruthy();
    expect(rewindBtn.getAttribute("aria-expanded")).toBe("false");
    expect(editBtn.getAttribute("aria-expanded")).toBe("true");

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: guide.title })).toBeNull();

    fireEvent.click(editBtn);
    fireEvent.click(screen.getByRole("button", { name: guide.actionLabel }));
    expect(onAction).toHaveBeenCalledOnce();
    expect(screen.queryByRole("dialog", { name: guide.title })).toBeNull();
  });
});

describe("editing a previous message", () => {
  it("warns before requesting confirmation and keeps editing separate from rewind", () => {
    const onRewind = vi.fn();
    const onEditSend = vi.fn();
    render(<MessageBubble who="You" isUser text="Original request" onRewind={onRewind} onEditSend={onEditSend} rewindScopes={["conversation"]} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByText(/original message and all later messages will be permanently deleted/)).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Updated request" } });
    expect(onEditSend).not.toHaveBeenCalled();
    expect(onRewind).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Review and resend" }));
    expect(onEditSend).toHaveBeenCalledWith("Updated request", []);
    expect(onRewind).not.toHaveBeenCalled();
  });

  it("discards a cancelled edit and prevents an empty text-only resend", () => {
    const onEditSend = vi.fn();
    render(<MessageBubble who="You" isUser text="Original request" onEditSend={onEditSend} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "  " } });
    expect((screen.getByRole("button", { name: "Review and resend" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Keep as is" }));
    expect(screen.queryByRole("textbox")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("Original request");
    expect(onEditSend).not.toHaveBeenCalled();
  });

  const paste = (files: File[]) => fireEvent.paste(screen.getByRole("textbox"), {
    clipboardData: { items: files.map(file => ({ kind: "file", type: file.type, getAsFile: () => file })), getData: () => "" },
  });
  const png = (name: string) => new File([name], name, { type: "image/png" });

  it("keeps consecutive pastes, removes an original image, and sends images without text", async () => {
    const onEditSend = vi.fn();
    const { container } = render(<MessageBubble who="You" isUser text="Original" images={[{ mimeType: "image/png", data: "T0xE" }]} onEditSend={onEditSend} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: " " } });
    paste([png("first")]);
    paste([png("second")]);
    expect((screen.getByRole("button", { name: "Review and resend" }) as HTMLButtonElement).disabled).toBe(true);
    await waitFor(() => expect(container.querySelectorAll(".sv-attach-item")).toHaveLength(3));
    fireEvent.click(screen.getAllByRole("button", { name: "Remove this image" })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Review and resend" }));
    expect(onEditSend).toHaveBeenCalledWith("", [expect.objectContaining({ data: btoa("first") }), expect.objectContaining({ data: btoa("second") })]);
    fireEvent.click(screen.getAllByRole("button", { name: "Remove this image" })[0]);
    fireEvent.click(screen.getAllByRole("button", { name: "Remove this image" })[0]);
    expect((screen.getByRole("button", { name: "Review and resend" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("counts retained images toward the limit and reports oversized or unreadable pastes", async () => {
    const { container } = render(<MessageBubble who="You" isUser text="Original" images={[{ mimeType: "image/png", data: "T0xE" }]} onEditSend={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const large = png("large.png");
    Object.defineProperty(large, "size", { value: MAX_IMAGE_BYTES + 1 });
    paste([large]);
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("large.png"));
    const read = vi.spyOn(FileReader.prototype, "readAsDataURL").mockImplementation(() => { throw new Error("Unreadable"); });
    paste([png("broken.png")]);
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("broken.png"));
    read.mockRestore();
    paste(Array.from({ length: MAX_IMAGES }, (_, i) => png(`s${i}`)));
    await waitFor(() => expect(container.querySelectorAll(".sv-attach-item")).toHaveLength(MAX_IMAGES));
    expect(screen.getByRole("status").textContent).toContain(String(MAX_IMAGES));
  });

  it("does not add an unfinished paste to a reopened editor", async () => {
    const { container } = render(<MessageBubble who="You" isUser text="Original" onEditSend={vi.fn()} />);
    let reader!: FileReader;
    vi.spyOn(FileReader.prototype, "readAsDataURL").mockImplementation(function(this: FileReader) { reader = this; });
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    paste([png("late")]);
    await waitFor(() => expect(reader).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "Keep as is" }));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    Object.defineProperty(reader, "result", { value: "data:image/png;base64,TEFURQ==" });
    await act(async () => { reader.dispatchEvent(new ProgressEvent("load")); });
    expect(container.querySelector(".sv-attach-item")).toBeNull();
    expect((screen.getByRole("button", { name: "Review and resend" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("reads the native clipboard when Tauri omits image data and leaves text paste alone", async () => {
    env.isTauri = true;
    vi.mocked(imageFromNativeClipboard).mockResolvedValue(png("native"));
    const { container } = render(<MessageBubble who="You" isUser text="Original" onEditSend={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    paste([]);
    await waitFor(() => expect(container.querySelectorAll(".sv-attach-item")).toHaveLength(1));
    expect(imageFromNativeClipboard).toHaveBeenCalledOnce();
    expect(fireEvent.paste(screen.getByRole("textbox"), { clipboardData: { items: [], getData: () => "plain text" } })).toBe(true);
    expect(imageFromNativeClipboard).toHaveBeenCalledOnce();
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
