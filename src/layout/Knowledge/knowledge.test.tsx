// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { setLang } from "../../i18n";
import { KnowledgeRoute } from "./KnowledgeRoute";
import { knowledgeUrl } from "./navigation";

vi.mock("../../ipc/transport", () => ({
  invoke: vi.fn().mockResolvedValue({ indexes: [], runtime: { available: true } }),
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("../../store/termStore", () => ({
  useTermStore: (select: (state: unknown) => unknown) => select({ projects: [{ id: "project", name: "Project" }] }),
}));

beforeEach(() => {
  setLang("en");
  window.history.replaceState(null, "", "/?knowledge=project&knowledgeIndex=index&knowledgeQuery=test&session=keep#terminal");
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

describe("代码图谱关闭导航", () => {
  it.each(["tauri://localhost", "tauri://localhost/", "http://localhost:23147/"])("为 %s 生成完整关闭地址", (base) => {
    vi.stubGlobal("window", { location: { href: `${base}?knowledge=project&knowledgeIndex=index&knowledgeQuery=test` } });
    expect(knowledgeUrl("")).toBe(base);
    expect(new URL(knowledgeUrl("")).search).toBe("");
  });

  it.each(["关闭链接", "Escape", "遮罩"])("通过%s关闭窗口并保留其他页面状态", async (method) => {
    render(<KnowledgeRoute />);
    const dialog = screen.getByRole("dialog");
    if (method === "关闭链接") fireEvent.click(screen.getByRole("link", { name: "Close" }));
    else if (method === "Escape") fireEvent.keyDown(dialog, { key: "Escape" });
    else {
      fireEvent.mouseDown(dialog.parentElement!);
      fireEvent.click(dialog.parentElement!);
    }
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(location.search).toBe("?session=keep");
    expect(location.hash).toBe("#terminal");
  });

  it("浏览器后退恢复图谱，前进再次关闭", async () => {
    render(<KnowledgeRoute />);
    fireEvent.click(screen.getByRole("link", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    window.history.back();
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeNull());
    expect(new URLSearchParams(location.search).get("knowledge")).toBe("project");
    window.history.forward();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(location.search).toBe("?session=keep");
  });
});
