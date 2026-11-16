import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
vi.mock("../../../ipc/chat", async original => ({ ...await original<typeof import("../../../ipc/chat")>(), loadChatTool: vi.fn() }));
import { loadChatTool } from "../../../ipc/chat";
import { ChatSearch } from "./ChatSearch";

afterEach(cleanup);
it("finds literal matches across history, tool inputs and nested output, and wraps navigation", () => {
  const onLocate = vi.fn();
  render(<ChatSearch entries={[
    { kind: "row", id: "old", row: { kind: "user", id: "old", text: "[Needle] first [needle]" } },
    { kind: "run", id: "tool", running: false, calls: [{ kind: "tool", id: "tool", name: "Read", input: { path: "[needle]" }, status: "completed", isError: false, children: [{ kind: "reasoning", id: "child", text: "nested [needle]", streaming: false }] }] },
  ]} onLocate={onLocate} onClose={vi.fn()} />);
  const input = screen.getByRole("textbox");
  fireEvent.change(input, { target: { value: "[needle]" } });
  expect(screen.getByRole("status").textContent).toBe("4/4");
  expect(onLocate).toHaveBeenLastCalledWith(1, "[needle]");
  expect(screen.getByRole("status").title).toBe("nested [needle]");
  fireEvent.keyDown(input, { key: "Enter" });
  expect(screen.getByRole("status").textContent).toBe("3/4");
  fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
  fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
  expect(screen.getByRole("status").textContent).toBe("1/4");
  expect(onLocate).toHaveBeenLastCalledWith(0, "[needle]");
  fireEvent.change(input, { target: { value: "absent" } });
  expect(screen.getByRole("status").textContent).toBe("No results");
  expect(onLocate).toHaveBeenLastCalledWith(-1, "");
});
it("leaves the Enter that confirms IME composition to the input method", () => {
  render(<ChatSearch entries={[
    { kind: "row", id: "a", row: { kind: "user", id: "a", text: "needle one" } },
    { kind: "row", id: "b", row: { kind: "user", id: "b", text: "needle two" } },
  ]} onLocate={vi.fn()} onClose={vi.fn()} />);
  const input = screen.getByRole("textbox");
  fireEvent.change(input, { target: { value: "needle" } });
  expect(screen.getByRole("status").textContent).toBe("2/2");
  // WebKit reports the confirming Enter with isComposing false and keyCode 229.
  expect(fireEvent.keyDown(input, { key: "Enter", keyCode: 229 })).toBe(true);
  expect(screen.getByRole("status").textContent).toBe("2/2");
  fireEvent.keyDown(input, { key: "Enter" });
  expect(screen.getByRole("status").textContent).toBe("1/2");
});
it("stays on the selected match while older history loads above it", () => {
  const onLocate = vi.fn();
  const recent = [
    { kind: "row" as const, id: "a", row: { kind: "user" as const, id: "a", text: "needle one" } },
    { kind: "row" as const, id: "b", row: { kind: "user" as const, id: "b", text: "needle two" } },
  ];
  const { rerender } = render(<ChatSearch entries={recent} onLocate={onLocate} onClose={vi.fn()} loadingHistory />);
  const input = screen.getByRole("textbox");
  fireEvent.change(input, { target: { value: "needle" } });
  expect(screen.getByText("2/2")).toBeTruthy();
  fireEvent.keyDown(input, { key: "Enter" });
  expect(onLocate).toHaveBeenLastCalledWith(0, "needle");
  const older = { kind: "row" as const, id: "old", row: { kind: "user" as const, id: "old", text: "needle zero" } };
  rerender(<ChatSearch entries={[older, ...recent]} onLocate={onLocate} onClose={vi.fn()} />);
  expect(screen.getByRole("status").textContent).toBe("2/3");
  expect(onLocate).toHaveBeenLastCalledWith(1, "needle");
});
it("closes with Escape without propagating an agent interrupt", () => {
  const parent = vi.fn(); const close = vi.fn();
  render(<div onKeyDown={parent}><ChatSearch entries={[]} onLocate={vi.fn()} onClose={close} /></div>);
  fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
  expect(close).toHaveBeenCalledOnce();
  expect(parent).not.toHaveBeenCalled();
});

it("does not report no results before history is loaded or after a paging failure", () => {
  const props = { entries: [], onLocate: vi.fn(), onClose: vi.fn(), onRetryHistory: vi.fn() };
  const { rerender } = render(<ChatSearch {...props} loadingHistory />);
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "first message" } });
  expect(screen.queryByText("No results")).toBeNull();
  expect(screen.getByText("Loading…")).toBeTruthy();
  rerender(<ChatSearch {...props} historyError="History temporarily unavailable" />);
  expect(screen.queryByText("No results")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(props.onRetryHistory).toHaveBeenCalledOnce();
  rerender(<ChatSearch {...props} />);
  expect(screen.getByText("No results")).toBeTruthy();
});

it("searches deferred tool output and nested details without expanding cards", async () => {
  vi.mocked(loadChatTool).mockResolvedValueOnce({ kind: "tool", id: "deferred", name: "Bash", input: {}, status: "completed", isError: false, output: "remote needle", children: [{ kind: "tool", id: "child", name: "Read", input: {}, status: "completed", isError: false, detailAvailable: true }] }).mockResolvedValueOnce({ kind: "tool", id: "child", name: "Read", input: {}, status: "completed", isError: false, output: "nested needle" });
  render(<ChatSearch entries={[{ kind: "row", id: "deferred", row: { kind: "tool", id: "deferred", name: "Bash", input: {}, status: "completed", isError: false, detailAvailable: true } }]} onLocate={vi.fn()} onClose={vi.fn()} />);
  expect(loadChatTool).not.toHaveBeenCalled();
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "needle" } });
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("2/2"));
  expect(loadChatTool).toHaveBeenCalledTimes(2);
});
