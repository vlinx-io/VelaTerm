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
  expect(screen.getByRole("status").textContent).toBe("1/4");
  expect(onLocate).toHaveBeenLastCalledWith(0, "[needle]");
  fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
  expect(screen.getByRole("status").textContent).toBe("4/4");
  expect(onLocate).toHaveBeenLastCalledWith(1, "[needle]");
  expect(screen.getByRole("status").title).toBe("nested [needle]");
  fireEvent.keyDown(input, { key: "Enter" });
  expect(screen.getByRole("status").textContent).toBe("1/4");
  fireEvent.change(input, { target: { value: "absent" } });
  expect(screen.getByRole("status").textContent).toBe("No results");
  expect(onLocate).toHaveBeenLastCalledWith(-1, "");
});
it("closes with Escape without propagating an agent interrupt", () => {
  const parent = vi.fn(); const close = vi.fn();
  render(<div onKeyDown={parent}><ChatSearch entries={[]} onLocate={vi.fn()} onClose={close} /></div>);
  fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
  expect(close).toHaveBeenCalledOnce();
  expect(parent).not.toHaveBeenCalled();
});

it("searches deferred tool output and nested details without expanding cards", async () => {
  vi.mocked(loadChatTool).mockResolvedValueOnce({ kind: "tool", id: "deferred", name: "Bash", input: {}, status: "completed", isError: false, output: "remote needle", children: [{ kind: "tool", id: "child", name: "Read", input: {}, status: "completed", isError: false, detailAvailable: true }] }).mockResolvedValueOnce({ kind: "tool", id: "child", name: "Read", input: {}, status: "completed", isError: false, output: "nested needle" });
  render(<ChatSearch entries={[{ kind: "row", id: "deferred", row: { kind: "tool", id: "deferred", name: "Bash", input: {}, status: "completed", isError: false, detailAvailable: true } }]} onLocate={vi.fn()} onClose={vi.fn()} />);
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "needle" } });
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("1/2"));
  expect(loadChatTool).toHaveBeenCalledTimes(2);
});
