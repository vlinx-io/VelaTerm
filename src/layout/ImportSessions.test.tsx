// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { discoverAgentSessions, importAgentSessions } from "../ipc/tree";
import { setLang } from "../i18n";
import { ImportSessionsRoute, importSessionsUrl } from "./ImportSessions";

vi.mock("../ipc/tree", () => ({
  discoverAgentSessions: vi.fn().mockResolvedValue({ directory: "/project", sessions: [], warnings: [] }),
  importAgentSessions: vi.fn(),
}));
vi.mock("../store/termStore", () => ({ useTermStore: { getState: () => ({ loadTree: vi.fn() }) } }));

beforeEach(() => {
  vi.mocked(discoverAgentSessions).mockResolvedValue({ directory: "/project", sessions: [], warnings: [] });
  vi.mocked(importAgentSessions).mockReset();
  setLang("en");
  window.history.replaceState(null, "", "/?importSessions=project&importSearch=test&session=keep#terminal");
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

describe("导入会话关闭导航", () => {
  it.each(["tauri://localhost", "tauri://localhost/", "http://localhost:23147/"])("为 %s 生成完整关闭地址", (base) => {
    vi.stubGlobal("window", { location: { href: `${base}?importSessions=project&importSearch=test` } });
    expect(importSessionsUrl("")).toBe(base);
  });

  it.each(["右上角关闭", "Escape", "遮罩"])("通过%s关闭并保留其他页面状态", async (method) => {
    render(<ImportSessionsRoute />);
    const dialog = screen.getByRole("dialog");
    await screen.findByText("No matching sessions found.");
    if (method === "右上角关闭") fireEvent.click(screen.getByRole("button", { name: "Close" }));
    else if (method === "Escape") fireEvent.keyDown(dialog, { key: "Escape" });
    else {
      fireEvent.mouseDown(dialog.parentElement!);
      fireEvent.click(dialog.parentElement!);
    }
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(location.search).toBe("?session=keep");
    expect(location.hash).toBe("#terminal");
  });

  it("后退恢复导入界面，前进再次关闭", async () => {
    render(<ImportSessionsRoute />);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    window.history.back();
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeNull());
    expect(new URLSearchParams(location.search).get("importSearch")).toBe("test");
    window.history.forward();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});


describe("导入会话选择与反馈", () => {
  const sessions = [
    { kind: "codex" as const, agentSessionId: "a", title: "Alpha", cwd: "/project", updatedAt: 0, imported: false },
    { kind: "claude" as const, agentSessionId: "b", title: "Beta", cwd: "/project", updatedAt: 0, imported: false },
    { kind: "codex" as const, agentSessionId: "c", title: "Previous", cwd: "/project", updatedAt: 0, imported: true },
  ];
  beforeEach(() => {
    window.history.replaceState(null, "", "/?importSessions=project");
    vi.mocked(discoverAgentSessions).mockResolvedValue({ directory: "/project", sessions, warnings: [] });
  });
  it("搜索保留已选项，全选显示部分选中，清空选择覆盖隐藏项", async () => {
    render(<ImportSessionsRoute />);
    const alpha = await screen.findByRole("checkbox", { name: "Codex: Alpha" });
    fireEvent.click(alpha);
    expect((screen.getByRole("checkbox", { name: "Select All" }) as HTMLInputElement).indeterminate).toBe(true);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Beta" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Select All" }));
    expect(screen.getByText("2 selected")).toBeTruthy();
    expect(location.search).toContain("importSearch=Beta");
    fireEvent.click(screen.getByRole("button", { name: "Clear selection" }));
    expect(screen.getByText("0 selected")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Import (0)" }) as HTMLButtonElement).disabled).toBe(true);
  });
  it("失败保留选择以便重试，成功显示结果并禁用已导入记录", async () => {
    vi.mocked(importAgentSessions).mockRejectedValueOnce(new Error("Import failed")).mockResolvedValueOnce([]);
    render(<ImportSessionsRoute />);
    fireEvent.click(await screen.findByRole("checkbox", { name: "Codex: Alpha" }));
    fireEvent.click(screen.getByRole("button", { name: "Import (1)" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Error: Import failed");
    vi.mocked(discoverAgentSessions).mockResolvedValue({
      directory: "/project", sessions: sessions.map((s) => s.agentSessionId === "a" ? { ...s, imported: true } : s), warnings: [],
    });
    fireEvent.click(screen.getByRole("button", { name: "Import (1)" }));
    await screen.findByText("Added 0 sessions to the project.");
    expect((screen.getByRole("checkbox", { name: "Codex: Alpha" }) as HTMLInputElement).disabled).toBe(true);
    expect(vi.mocked(importAgentSessions)).toHaveBeenLastCalledWith("project", [{ kind: "codex", agentSessionId: "a" }]);
  });
  it("无搜索结果时可清除搜索，已导入记录不参与全选", async () => {
    render(<ImportSessionsRoute />);
    await screen.findByText("Alpha");
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "missing" } });
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Select All" }));
    expect(screen.getByText("2 selected")).toBeTruthy();
    expect((screen.getByRole("checkbox", { name: "Codex: Previous" }) as HTMLInputElement).checked).toBe(false);
    expect(location.search).toBe("?importSessions=project");
  });
  it("Tab 焦点停留在弹窗内", async () => {
    render(<ImportSessionsRoute />);
    await screen.findByText("Alpha");
    fireEvent.click(screen.getByRole("checkbox", { name: "Codex: Alpha" }));
    const closeButton = screen.getByRole("button", { name: "Close" });
    const importButton = screen.getByRole("button", { name: "Import (1)" });
    closeButton.focus();
    fireEvent.keyDown(closeButton, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(importButton);
    fireEvent.keyDown(importButton, { key: "Tab" });
    expect(document.activeElement).toBe(closeButton);
  });
});


describe("导入会话加载状态", () => {
  it("读取失败后可刷新恢复，并显示来源警告与空历史提示", async () => {
    window.history.replaceState(null, "", "/?importSessions=project");
    vi.mocked(discoverAgentSessions).mockRejectedValueOnce(new Error("Read failed"))
      .mockResolvedValueOnce({ directory: "/project", sessions: [], warnings: ["Source unavailable"] });
    render(<ImportSessionsRoute />);
    await screen.findByText("Error: Read failed");
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await screen.findByText("No session history was found for this project directory.");
    expect(screen.getByRole("alert").textContent).toBe("Source unavailable");
    expect((screen.getByRole("button", { name: "Import (0)" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("加载过程中仍可关闭，迟到的响应不会重新打开弹窗", async () => {
    let resolve!: (value: Awaited<ReturnType<typeof discoverAgentSessions>>) => void;
    vi.mocked(discoverAgentSessions).mockReturnValueOnce(new Promise((done) => { resolve = done; }));
    render(<ImportSessionsRoute />);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    resolve({ directory: "/project", sessions: [], warnings: [] });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});
