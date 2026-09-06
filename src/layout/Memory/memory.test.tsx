// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { setLang } from "../../i18n";
import { MemoryEditor } from "./MemoryDocument";
import { MemoryCompile } from "./MemoryTasks";
import { MemoryMarkdown } from "./shared";
import { memoryNavigate, memoryUrl } from "./navigation";

const api = vi.hoisted(() => ({ get: vi.fn(), options: vi.fn(), save: vi.fn(), start: vi.fn() }));
vi.mock("../../ipc/memory", () => ({
  memoryGet: api.get, memoryOptions: api.options, memorySave: api.save, memoryStart: api.start,
  memoryDelete: vi.fn(), memoryRestore: vi.fn(), memorySource: vi.fn(), memoryJobs: vi.fn(), memoryCancel: vi.fn(), memoryRetry: vi.fn(),
}));
vi.mock("../../store/termStore", () => ({ useTermStore: (select: (s: unknown) => unknown) => select({ sessions: [{ id: "session", name: "Source conversation" }], archivedSessions: [] }) }));

beforeEach(() => {
  setLang("en"); window.history.replaceState(null, "", "/?memory=library");
  api.get.mockReset(); api.options.mockReset(); api.save.mockReset(); api.start.mockReset();
  api.options.mockResolvedValue({ agents: [{ id: "claude", label: "Claude", available: true }, { id: "codex", label: "Codex", available: true }], defaultAgent: "codex", catalog: [] });
});
afterEach(cleanup);

describe("Global Memory interactions", () => {
  it("uses the backend agent default and submits the user's selected model", async () => {
    api.start.mockResolvedValue({ id: "job-1", reused: false });
    render(<MemoryCompile sessionId="session" />);
    const select = await screen.findByLabelText("Agent") as HTMLSelectElement;
    await waitFor(() => expect(select.value).toBe("codex"));
    fireEvent.change(select, { target: { value: "claude" } });
    fireEvent.change(screen.getByLabelText("Model (optional)"), { target: { value: "chosen-model" } });
    fireEvent.click(screen.getByRole("button", { name: "Compile and save" }));
    await waitFor(() => expect(api.start).toHaveBeenCalledWith("session", "claude", "chosen-model"));
    await waitFor(() => expect(new URLSearchParams(location.search).get("memory")).toBe("job/job-1"));
  });

  it("submits edits with the backend version and preserves the form after a conflict", async () => {
    api.get.mockResolvedValue({ entry: { id: "entry", version: 7, title: "Original", summary: "Summary", content: "Body", tags: ["Tag"], related: [], sources: [] }, catalog: [] });
    api.save.mockRejectedValue(new Error("memory_conflict"));
    render(<MemoryEditor id="entry" />);
    fireEvent.change(await screen.findByLabelText("Title"), { target: { value: "Edited" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(api.save).toHaveBeenCalledWith({ id: "entry", version: 7, title: "Edited", summary: "Summary", content: "Body", tags: ["Tag"], related: [] }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", expect.stringContaining("changed during"));
    expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe("Edited");
  });

  it("requires an explicit discard before leaving an edited form", async () => {
    window.history.replaceState(null, "", "/?memory=new");
    render(<MemoryEditor />);
    fireEvent.change(await screen.findByLabelText("Title"), { target: { value: "Unsaved" } });
    fireEvent.click(screen.getByRole("link", { name: "Cancel" }));
    expect(await screen.findByRole("alertdialog")).toHaveProperty("textContent", expect.stringContaining("Discard unsaved changes?"));
    expect(new URLSearchParams(location.search).get("memory")).toBe("new");
    fireEvent.click(screen.getByRole("button", { name: "OK" }));
    expect(new URLSearchParams(location.search).get("memory")).toBe("library");
  });

  it("renders Markdown without executable HTML or remote images", () => {
    const { container } = render(<MemoryMarkdown content={'# Knowledge\n\n**Safe**\n\n<script>alert(1)</script><img src="https://example.com/tracker" onerror="alert(1)"><a href="javascript:alert(1)">unsafe</a>'} />);
    expect(container.querySelector("h1")?.textContent).toBe("Knowledge");
    expect(container.querySelector("strong")?.textContent).toBe("Safe");
    expect(container.querySelector("script,img")).toBeNull();
    expect(container.querySelector("a")?.getAttribute("href")).toBeNull();
  });

  it("uses stable URLs for library filters and source routes", () => {
    const href = memoryUrl("source/source-id", { memoryQuery: "中文搜索", memoryTag: "技术", memoryPage: 2 });
    memoryNavigate(href);
    const params = new URLSearchParams(location.search);
    expect(params.get("memory")).toBe("source/source-id");expect(params.get("memoryQuery")).toBe("中文搜索");
    expect(params.get("memoryTag")).toBe("技术");expect(params.get("memoryPage")).toBe("2");
  });
});
