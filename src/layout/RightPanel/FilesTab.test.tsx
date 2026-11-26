import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FilesTab } from "./FilesTab";
import { resetFileClipboardForTest } from "./fileClipboard";

type Change = { root: string; dirs: string[] | null; git: boolean };
const mocks = vi.hoisted(() => ({
  list: vi.fn(),
  search: vi.fn(),
  watch: vi.fn(),
  createFile: vi.fn(),
  deletePath: vi.fn(),
  copyText: vi.fn(),
  clipboardFileState: vi.fn(),
  clipboardWriteFiles: vi.fn(),
  clipboardReadFiles: vi.fn(),
  clipboardClearFiles: vi.fn(),
  pasteFilesCheck: vi.fn(),
  pasteFiles: vi.fn(),
  envState: {
    isBrowser: true,
    isTauri: false,
    isElectron: false,
    isMac: false,
    isLinux: true,
    isWindows: false,
  },
  changed: null as null | ((event: Change) => void),
}));
vi.mock("../../ipc/info", () => ({
  listDir: mocks.list,
  searchFiles: mocks.search,
  fsWatch: mocks.watch,
  copyIntoDir: vi.fn(),
  copyText: mocks.copyText,
  createDir: vi.fn(),
  createFile: mocks.createFile,
  deletePath: mocks.deletePath,
  readFilePreview: vi.fn(),
  renamePath: vi.fn(),
  revealPath: vi.fn(),
  clipboardFileState: mocks.clipboardFileState,
  clipboardWriteFiles: mocks.clipboardWriteFiles,
  clipboardReadFiles: mocks.clipboardReadFiles,
  clipboardClearFiles: mocks.clipboardClearFiles,
  pasteFilesCheck: mocks.pasteFilesCheck,
  pasteFiles: mocks.pasteFiles,
}));
vi.mock("../../platform", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../platform")>();
  return {
    ...actual,
    env: mocks.envState,
  };
});
vi.mock("../../ipc/events", () => ({
  onFsChanged: (cb: (event: Change) => void) => { mocks.changed = cb; return Promise.resolve(() => { mocks.changed = null; }); },
}));
vi.mock("../../i18n", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../i18n")>(), useT: () => (key: string, ...args: unknown[]) =>
    args.length ? `${key}:${args.join(",")}` : key,
}));
vi.mock("../../ipc/transfer", () => {
  const transfers: never[] = [];
  return { getTransfers: () => transfers, subscribeTransfers: () => () => {}, cancelTransfer: vi.fn(),
    clearFinishedTransfers: vi.fn(), startDownload: vi.fn(), startUpload: vi.fn() };
});

beforeEach(() => {
  vi.useFakeTimers();
  vi.resetAllMocks();
  resetFileClipboardForTest();
  Object.assign(mocks.envState, {
    isBrowser: true,
    isTauri: false,
    isElectron: false,
    isMac: false,
    isLinux: true,
    isWindows: false,
  });
  // Unwatched by default, so the poll-driven tests below behave as on Linux.
  mocks.watch.mockResolvedValue({ root: "/repo", supported: false });
  mocks.search.mockResolvedValue({ entries: [], truncated: false });
  mocks.createFile.mockResolvedValue(undefined);
  mocks.deletePath.mockResolvedValue(undefined);
  mocks.copyText.mockResolvedValue(undefined);
  mocks.clipboardFileState.mockResolvedValue({ supported: false, hasFiles: false, mode: null, sequence: 0 });
  mocks.clipboardWriteFiles.mockResolvedValue(1);
  mocks.clipboardReadFiles.mockResolvedValue(null);
  mocks.clipboardClearFiles.mockResolvedValue(undefined);
  mocks.pasteFilesCheck.mockResolvedValue([]);
  mocks.pasteFiles.mockResolvedValue({ created: [], replaced: [], skipped: [] });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("Files background decorations", () => {
  it("updates nested badges without collapsing directories or losing the selected file", async () => {
    let badge: string | null = "M";
    mocks.list.mockImplementation(async (path: string) => path === "/repo"
      ? [{ name: "src", isDir: true, isHidden: false, gitBadge: badge }]
      : [{ name: "文档.txt", isDir: false, isHidden: false, gitBadge: badge }]);
    const view = render(<FilesTab rootPath="/repo" rootName="Example" />);
    await act(async () => {});
    fireEvent.click(screen.getByText("src"));
    await act(async () => {});
    fireEvent.click(screen.getByText("文档.txt"));
    expect(view.container.querySelectorAll(".gb-M")).toHaveLength(2);
    badge = null;
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(screen.getByText("文档.txt")).toBeTruthy();
    expect(view.container.querySelector(".file-row.sel")?.textContent).toContain("文档.txt");
    expect(view.container.querySelectorAll(".gb-M")).toHaveLength(0);
    mocks.list.mockRejectedValue(new Error("Status unavailable"));
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(screen.getByText("文档.txt")).toBeTruthy();
    expect(view.container.querySelector(".files-err")?.textContent).toContain("Status unavailable");
    mocks.list.mockImplementation(async (path: string) => path === "/repo"
      ? [{ name: "src", isDir: true, isHidden: false, gitBadge: null }]
      : [{ name: "文档.txt", isDir: false, isHidden: false, gitBadge: null }]);
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(view.container.querySelector(".files-err")).toBeNull();
    expect(view.container.querySelector(".file-row.sel")?.textContent).toContain("文档.txt");
  });

  it("keeps an explicit directory-load error until dismissed despite successful background refresh", async () => {
    mocks.list.mockImplementation(async (path: string) => {
      if (path !== "/repo") throw new Error("Directory access denied");
      return [{ name: "src", isDir: true, isHidden: false, gitBadge: null }];
    });
    const view = render(<FilesTab rootPath="/repo" rootName="Example" />);
    await act(async () => {});
    fireEvent.click(screen.getByText("src"));
    await act(async () => {});
    expect(view.container.querySelector(".files-err")?.textContent).toContain("Directory access denied");
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(view.container.querySelector(".files-err")?.textContent).toContain("Directory access denied");
    fireEvent.click(view.container.querySelector(".files-err")!);
    expect(view.container.querySelector(".files-err")).toBeNull();
  });
});

describe("Files quick filter", () => {
  it("clears the filter when the root switches to another project", async () => {
    mocks.list.mockImplementation(async (path: string) => path === "/repo"
      ? [{ name: "a.ts", isDir: false, isHidden: false, gitBadge: null }]
      : [{ name: "b.ts", isDir: false, isHidden: false, gitBadge: null }]);
    const view = render(<FilesTab rootPath="/repo" rootName="A" />);
    await act(async () => {});
    fireEvent.click(screen.getByLabelText("files.filterPlaceholder"));
    fireEvent.change(screen.getByPlaceholderText("files.filterPlaceholder"), { target: { value: "zzz" } });
    expect(screen.queryByText("a.ts")).toBeNull();

    view.rerender(<FilesTab rootPath="/other" rootName="B" />);
    await act(async () => {});
    expect(screen.queryByPlaceholderText("files.filterPlaceholder")).toBeNull();
    expect(screen.getByLabelText("files.filterPlaceholder").getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByText("b.ts")).toBeTruthy();
  });

  it("searches nested directories recursively and lets matching parent folders collapse and re-expand", async () => {
    mocks.list.mockImplementation(async (path: string) => path === "/repo"
      ? [
          { name: "foo", isDir: true, isHidden: false, gitBadge: "M" },
          { name: "src", isDir: true, isHidden: false, gitBadge: null },
          { name: "readme.md", isDir: false, isHidden: false, gitBadge: null },
        ]
      : []);
    mocks.search.mockResolvedValue({
      entries: [
        { path: "foo", isDir: true, isHidden: false, gitBadge: "M" },
        { path: "foo/foo_sub", isDir: true, isHidden: false, gitBadge: null },
        { path: "foo/foo_sub/foo_leaf.ts", isDir: false, isHidden: false, gitBadge: null },
        { path: "src", isDir: true, isHidden: false, gitBadge: null },
        { path: "src/deep", isDir: true, isHidden: false, gitBadge: null },
        { path: "src/deep/foo_helper.ts", isDir: false, isHidden: false, gitBadge: "M" },
      ],
      truncated: true,
    });

    const view = render(<FilesTab rootPath="/repo" rootName="Repo" />);
    await act(async () => {});

    // Neither nested folder is expanded yet in the normal tree.
    expect(screen.queryByText("foo_leaf.ts")).toBeNull();
    expect(screen.queryByText("foo_helper.ts")).toBeNull();

    fireEvent.click(screen.getByLabelText("files.filterPlaceholder"));
    fireEvent.change(screen.getByPlaceholderText("files.filterPlaceholder"), { target: { value: "foo" } });
    await act(async () => { await vi.advanceTimersByTimeAsync(200); });

    expect(mocks.search).toHaveBeenCalledWith("/repo", "foo", false);
    expect(screen.getByText("foo_sub")).toBeTruthy();
    expect(screen.getByText("foo_leaf.ts")).toBeTruthy();
    expect(screen.getByText("foo_helper.ts")).toBeTruthy();
    expect(screen.getByText("files.filterTruncated")).toBeTruthy();

    // Clicking a matching parent folder with matching children collapses it.
    fireEvent.click(screen.getByText("foo"));
    expect(screen.queryByText("foo_sub")).toBeNull();
    expect(screen.queryByText("foo_leaf.ts")).toBeNull();
    expect(screen.getByText("foo_helper.ts")).toBeTruthy();

    // Clicking the parent folder again re-expands it.
    fireEvent.click(screen.getByText("foo"));
    expect(screen.getByText("foo_sub")).toBeTruthy();
    expect(screen.getByText("foo_leaf.ts")).toBeTruthy();

    // Collapse All closes every expanded branch during filtering, and clearing the filter restores normal tree.
    fireEvent.click(screen.getByLabelText("files.collapseAll"));
    expect(screen.queryByText("foo_leaf.ts")).toBeNull();
    expect(screen.queryByText("foo_helper.ts")).toBeNull();
    expect((screen.getByLabelText("files.collapseAll") as HTMLButtonElement).disabled).toBe(true);

    // Clearing filter returns to normal unexpanded `root` state (`src` and `foo` were closed initially).
    fireEvent.click(screen.getByLabelText("common.close"));
    expect(screen.getByText("readme.md")).toBeTruthy();
    expect(screen.queryByText("foo_leaf.ts")).toBeNull();
    expect(view.container.querySelector(".files-filter-truncated")).toBeNull();
  });

  it("supports keyboard collapse/expand and context actions in deep searched folders", async () => {
    mocks.list.mockImplementation(async (path: string) => path === "/repo"
      ? [{ name: "foo", isDir: true, isHidden: false, gitBadge: null }]
      : [{ name: "foo_child.ts", isDir: false, isHidden: false, gitBadge: null }]);
    mocks.search.mockResolvedValue({
      entries: [
        { path: "foo", isDir: true, isHidden: false, gitBadge: null },
        { path: "foo/foo_child.ts", isDir: false, isHidden: false, gitBadge: null },
      ],
      truncated: false,
    });

    const view = render(<FilesTab rootPath="/repo" rootName="Repo" />);
    await act(async () => {});

    fireEvent.click(screen.getByLabelText("files.filterPlaceholder"));
    fireEvent.change(screen.getByPlaceholderText("files.filterPlaceholder"), { target: { value: "foo" } });
    await act(async () => { await vi.advanceTimersByTimeAsync(200); });
    expect(screen.getByText("foo_child.ts")).toBeTruthy();

    // Navigate with keyboard: ArrowDown moves to Repo, then foo; ArrowLeft collapses foo; ArrowRight expands foo.
    const tree = view.container.querySelector(".files-tree")!;
    fireEvent.keyDown(tree, { key: "ArrowDown" });
    fireEvent.keyDown(tree, { key: "ArrowDown" });
    fireEvent.keyDown(tree, { key: "ArrowLeft" });
    expect(screen.queryByText("foo_child.ts")).toBeNull();
    fireEvent.keyDown(tree, { key: "ArrowRight" });
    expect(screen.getByText("foo_child.ts")).toBeTruthy();

    // Creating a new file via context menu on `foo_child.ts` targets `/repo/foo`, not `/repo`.
    fireEvent.contextMenu(screen.getByText("foo_child.ts"));
    fireEvent.click(screen.getByText("files.newFile"));
    const modalInput = screen.getByRole("dialog").querySelector("input") as HTMLInputElement;
    fireEvent.change(modalInput, { target: { value: "foo_new.ts" } });
    await act(async () => {
      fireEvent.click(screen.getByText("common.create"));
    });
    expect(mocks.createFile).toHaveBeenCalledWith("/repo/foo/foo_new.ts");
  });

  it("refreshes active search results on fs://changed while keeping manually collapsed folders closed", async () => {
    mocks.watch.mockResolvedValue({ root: "/repo", supported: true });
    mocks.list.mockResolvedValue([{ name: "foo", isDir: true, isHidden: false, gitBadge: null }]);
    mocks.search.mockResolvedValue({
      entries: [
        { path: "foo", isDir: true, isHidden: false, gitBadge: null },
        { path: "foo/foo_a.ts", isDir: false, isHidden: false, gitBadge: null },
      ],
      truncated: false,
    });

    render(<FilesTab rootPath="/repo" rootName="Repo" />);
    await act(async () => {});
    fireEvent.click(screen.getByLabelText("files.filterPlaceholder"));
    fireEvent.change(screen.getByPlaceholderText("files.filterPlaceholder"), { target: { value: "foo" } });
    await act(async () => { await vi.advanceTimersByTimeAsync(200); });

    // Collapse `foo`, then emit an fs://changed event in `src` adding a new match.
    fireEvent.click(screen.getByText("foo"));
    expect(screen.queryByText("foo_a.ts")).toBeNull();

    mocks.search.mockResolvedValue({
      entries: [
        { path: "foo", isDir: true, isHidden: false, gitBadge: null },
        { path: "foo/foo_a.ts", isDir: false, isHidden: false, gitBadge: null },
        { path: "src", isDir: true, isHidden: false, gitBadge: null },
        { path: "src/foo_b.ts", isDir: false, isHidden: false, gitBadge: null },
      ],
      truncated: false,
    });
    await act(async () => {
      mocks.changed?.({ root: "/repo", dirs: ["src"], git: false });
      await vi.advanceTimersByTimeAsync(200);
    });

    expect(screen.getByText("foo_b.ts")).toBeTruthy();
    expect(screen.queryByText("foo_a.ts")).toBeNull();
  });
});

describe("Files change notification", () => {
  it("re-reads only the shown directories an event names and polls rarely while watched", async () => {
    mocks.watch.mockResolvedValue({ root: "/repo", supported: true });
    mocks.list.mockImplementation(async (path: string) => path === "/repo"
      ? [{ name: "src", isDir: true, isHidden: false, gitBadge: null }, { name: "docs", isDir: true, isHidden: false, gitBadge: null }]
      : [{ name: "a.ts", isDir: false, isHidden: false, gitBadge: null }]);
    render(<FilesTab rootPath="/repo" rootName="Example" />);
    await act(async () => {});
    fireEvent.click(screen.getByText("src"));
    await act(async () => {});
    expect(mocks.watch).toHaveBeenCalledWith("/repo");
    mocks.list.mockClear();

    await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
    expect(mocks.list).not.toHaveBeenCalled();

    // A change in a collapsed folder or another root costs nothing.
    await act(async () => {
      mocks.changed?.({ root: "/repo", dirs: ["docs"], git: false });
      mocks.changed?.({ root: "/other", dirs: [""], git: false });
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(mocks.list).not.toHaveBeenCalled();

    await act(async () => {
      mocks.changed?.({ root: "/repo", dirs: ["src"], git: false });
      mocks.changed?.({ root: "/repo", dirs: ["src"], git: false });
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(mocks.list.mock.calls).toEqual([["/repo/src"]]);

    // A git change moves badges anywhere, so everything shown is re-read.
    mocks.list.mockClear();
    await act(async () => { mocks.changed?.({ root: "/repo", dirs: [], git: true }); await vi.advanceTimersByTimeAsync(200); });
    expect(mocks.list.mock.calls.map((c) => c[0]).sort()).toEqual(["/repo", "/repo/src"]);

    // The safety net still runs every 30 seconds and renews the lease.
    mocks.list.mockClear();
    await act(async () => { await vi.advanceTimersByTimeAsync(30_000); });
    expect(mocks.list).toHaveBeenCalled();
    expect(mocks.watch.mock.calls.length).toBeGreaterThanOrEqual(2);
  });
});

describe("Files selection and clipboard", () => {
  const setupTree = () => {
    mocks.list.mockImplementation(async (path: string) => {
      if (path === "/repo") {
        return [
          { name: "src", isDir: true, isHidden: false, gitBadge: null },
          { name: "a.txt", isDir: false, isHidden: false, gitBadge: null },
          { name: "b.txt", isDir: false, isHidden: false, gitBadge: null },
          { name: "c.txt", isDir: false, isHidden: false, gitBadge: null },
        ];
      }
      if (path === "/repo/src") {
        return [{ name: "nested.ts", isDir: false, isHidden: false, gitBadge: null }];
      }
      return [];
    });
  };

  it("supports Ctrl+click and Shift+click multi-selection, preserves it on right-click, and supports multi-delete", async () => {
    setupTree();
    const view = render(<FilesTab rootPath="/repo" rootName="repo" />);
    await act(async () => {});

    fireEvent.click(screen.getByText("a.txt"));
    expect(view.container.querySelectorAll(".file-row.sel")).toHaveLength(1);

    // Shift+click selects contiguous visible range a.txt..c.txt.
    fireEvent.click(screen.getByText("c.txt"), { shiftKey: true });
    expect(Array.from(view.container.querySelectorAll(".file-row.sel .nm")).map((n) => n.textContent))
      .toEqual(["a.txt", "b.txt", "c.txt"]);

    // Ctrl+click toggles b.txt off without collapsing the rest.
    fireEvent.click(screen.getByText("b.txt"), { ctrlKey: true });
    expect(Array.from(view.container.querySelectorAll(".file-row.sel .nm")).map((n) => n.textContent))
      .toEqual(["a.txt", "c.txt"]);

    // Right-clicking inside selection keeps both selected and opens multi-select menu.
    fireEvent.contextMenu(screen.getByText("a.txt"));
    expect(Array.from(view.container.querySelectorAll(".file-row.sel .nm")).map((n) => n.textContent))
      .toEqual(["a.txt", "c.txt"]);
    expect(screen.queryByText("files.newFile")).toBeNull();
    expect(screen.getByText("common.cut")).toBeTruthy();
    expect(screen.getByText("common.copy")).toBeTruthy();
    expect(screen.getByText("common.delete")).toBeTruthy();

    // Multi-delete asks for confirmation with count and deletes both files.
    fireEvent.click(screen.getByText("common.delete"));
    expect(screen.getByText("files.deleteManyConfirm:2")).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getAllByText("common.delete").at(-1)!);
    });
    expect(mocks.deletePath.mock.calls).toEqual([["/repo/a.txt"], ["/repo/c.txt"]]);

    // Right-clicking outside the selection collapses to the clicked item.
    fireEvent.contextMenu(screen.getByText("b.txt"));
    expect(Array.from(view.container.querySelectorAll(".file-row.sel .nm")).map((n) => n.textContent))
      .toEqual(["b.txt"]);
    expect(screen.getByText("files.newFile")).toBeTruthy();
  });

  it("cuts and copies via context menu with paste target rules, dimming persistence, and Escape/CopyPath reset", async () => {
    setupTree();
    mocks.pasteFilesCheck.mockResolvedValue([
      { source: "/repo/a.txt", name: "a.txt", isDir: false, status: "ready", existingIsDir: null },
    ]);
    mocks.pasteFiles.mockResolvedValue({ created: ["/repo/src/a.txt"], replaced: [], skipped: [] });

    const view = render(<FilesTab rootPath="/repo" rootName="repo" />);
    await act(async () => {});

    // Browser mode falls back to in-app clipboard without invoking native clipboard IPC.
    fireEvent.contextMenu(screen.getByText("a.txt"));
    fireEvent.click(screen.getByText("common.cut"));
    await act(async () => {});
    expect(mocks.clipboardWriteFiles).not.toHaveBeenCalled();
    expect(screen.getByText("a.txt").closest(".file-row")?.classList.contains("cut-entry")).toBe(true);

    // Cut dimming survives unmounting/remounting when switching inspector tabs.
    view.unmount();
    const remounted = render(<FilesTab rootPath="/repo" rootName="repo" />);
    await act(async () => {});
    expect(screen.getByText("a.txt").closest(".file-row")?.classList.contains("cut-entry")).toBe(true);

    // Copy Path clears cut dimming because plain text replaced the clipboard.
    fireEvent.contextMenu(screen.getByText("b.txt"));
    fireEvent.click(screen.getByText("files.copyPath"));
    await act(async () => {});
    expect(mocks.copyText).toHaveBeenCalledWith("/repo/b.txt");
    expect(screen.getByText("a.txt").closest(".file-row")?.classList.contains("cut-entry")).toBe(false);

    // Cut again and cancel with Escape.
    fireEvent.contextMenu(screen.getByText("a.txt"));
    fireEvent.click(screen.getByText("common.cut"));
    await act(async () => {});
    expect(screen.getByText("a.txt").closest(".file-row")?.classList.contains("cut-entry")).toBe(true);
    const tree = remounted.container.querySelector(".files-tree")!;
    fireEvent.keyDown(tree, { key: "Escape" });
    await act(async () => {});
    expect(screen.getByText("a.txt").closest(".file-row")?.classList.contains("cut-entry")).toBe(false);

    // Cut a.txt, paste onto directory 'src' -> targets '/repo/src' and clears cut dimming after completion.
    fireEvent.contextMenu(screen.getByText("a.txt"));
    fireEvent.click(screen.getByText("common.cut"));
    await act(async () => {});
    fireEvent.contextMenu(screen.getByText("src"));
    await act(async () => {
      fireEvent.click(screen.getByText("common.paste"));
    });
    expect(mocks.pasteFilesCheck).toHaveBeenCalledWith(["/repo/a.txt"], "/repo/src", "cut");
    expect(mocks.pasteFiles).toHaveBeenCalledWith(["/repo/a.txt"], "/repo/src", "cut", {});
    expect(screen.getByText("a.txt").closest(".file-row")?.classList.contains("cut-entry")).toBe(false);

    // Copy a.txt and paste onto empty tree space -> targets root '/repo'.
    mocks.pasteFilesCheck.mockClear();
    mocks.pasteFiles.mockClear();
    fireEvent.contextMenu(screen.getByText("a.txt"));
    fireEvent.click(screen.getByText("common.copy"));
    await act(async () => {});
    fireEvent.contextMenu(tree);
    await act(async () => {
      fireEvent.click(screen.getByText("common.paste"));
    });
    expect(mocks.pasteFilesCheck).toHaveBeenCalledWith(["/repo/a.txt"], "/repo", "copy");

    // Paste onto a file ('nested.ts' inside 'src') -> targets parent '/repo/src'.
    mocks.pasteFilesCheck.mockClear();
    await act(async () => {
      fireEvent.contextMenu(screen.getByText("nested.ts"));
    });
    await act(async () => {
      fireEvent.click(screen.getByText("common.paste"));
    });
    expect(mocks.pasteFilesCheck).toHaveBeenCalledWith(["/repo/a.txt"], "/repo/src", "copy");
  });

  it("uses native OS clipboard on desktop and clears stale cut state on external sequence change", async () => {
    Object.assign(mocks.envState, { isBrowser: false, isTauri: true, isMac: false, isLinux: false, isWindows: true });
    setupTree();
    mocks.clipboardFileState.mockResolvedValue({ supported: true, hasFiles: true, mode: "copy", sequence: 10 });
    mocks.clipboardWriteFiles.mockResolvedValue(42);
    mocks.clipboardReadFiles.mockResolvedValue({ paths: ["/repo/a.txt"], mode: "cut", sequence: 42 });
    mocks.pasteFilesCheck.mockResolvedValue([
      { source: "/repo/a.txt", name: "a.txt", isDir: false, status: "ready", existingIsDir: null },
    ]);
    mocks.pasteFiles.mockResolvedValue({ created: ["/repo/src/a.txt"], replaced: [], skipped: [] });

    const view = render(<FilesTab rootPath="/repo" rootName="repo" />);
    await act(async () => {});

    const tree = view.container.querySelector(".files-tree")!;
    fireEvent.click(screen.getByText("a.txt"));
    await act(async () => {
      fireEvent.keyDown(tree, { key: "x", ctrlKey: true });
    });
    expect(mocks.clipboardWriteFiles).toHaveBeenCalledWith(["/repo/a.txt"], "cut");
    expect(screen.getByText("a.txt").closest(".file-row")?.classList.contains("cut-entry")).toBe(true);

    // Focus with a changed OS clipboard sequence drops the stale cut dimming.
    mocks.clipboardFileState.mockResolvedValue({ supported: true, hasFiles: false, mode: null, sequence: 99 });
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(screen.getByText("a.txt").closest(".file-row")?.classList.contains("cut-entry")).toBe(false);

    // Cut again and paste via Ctrl+V into clicked folder 'src'.
    mocks.clipboardFileState.mockResolvedValue({ supported: true, hasFiles: true, mode: "cut", sequence: 42 });
    fireEvent.click(screen.getByText("a.txt"));
    await act(async () => {
      fireEvent.keyDown(tree, { key: "x", ctrlKey: true });
    });
    await act(async () => {
      fireEvent.click(screen.getByText("src"));
    });
    await act(async () => {
      fireEvent.keyDown(tree, { key: "v", ctrlKey: true });
    });
    expect(mocks.clipboardReadFiles).toHaveBeenCalled();
    expect(mocks.pasteFiles).toHaveBeenCalledWith(["/repo/a.txt"], "/repo/src", "cut", {});
    expect(mocks.clipboardClearFiles).toHaveBeenCalledWith(42);
  });

  it("handles paste conflicts via Replace, Skip, Keep Both, Apply-to-all, and Cancel", async () => {
    setupTree();
    mocks.pasteFilesCheck.mockResolvedValue([
      { source: "/repo/a.txt", name: "a.txt", isDir: false, status: "conflict", existingIsDir: false },
      { source: "/repo/b.txt", name: "b.txt", isDir: false, status: "conflict", existingIsDir: false },
    ]);
    mocks.pasteFiles.mockResolvedValue({ created: ["/repo/src/b.txt"], replaced: [], skipped: ["/repo/a.txt"] });

    const view = render(<FilesTab rootPath="/repo" rootName="repo" />);
    await act(async () => {});

    fireEvent.click(screen.getByText("a.txt"));
    fireEvent.click(screen.getByText("b.txt"), { ctrlKey: true });
    const tree = view.container.querySelector(".files-tree")!;
    await act(async () => {
      fireEvent.keyDown(tree, { key: "x", ctrlKey: true });
    });

    // Trigger paste into 'src'; conflict modal opens before any pasteFiles call.
    fireEvent.contextMenu(screen.getByText("src"));
    await act(async () => {
      fireEvent.click(screen.getByText("common.paste"));
    });
    expect(mocks.pasteFiles).not.toHaveBeenCalled();
    expect(screen.getByText("files.pasteConflictProgress:1,2")).toBeTruthy();

    // Cancel leaves filesystem untouched and preserves cut dimming.
    fireEvent.click(screen.getByText("common.cancel"));
    expect(mocks.pasteFiles).not.toHaveBeenCalled();
    expect(screen.getByText("a.txt").closest(".file-row")?.classList.contains("cut-entry")).toBe(true);

    // Re-open conflict prompt: Skip item 1, then Replace item 2.
    fireEvent.contextMenu(screen.getByText("src"));
    await act(async () => {
      fireEvent.click(screen.getByText("common.paste"));
    });
    fireEvent.click(screen.getByText("files.pasteSkip"));
    expect(screen.getByText("files.pasteConflictProgress:2,2")).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getByText("files.pasteReplace"));
    });
    expect(mocks.pasteFiles).toHaveBeenCalledWith(["/repo/a.txt", "/repo/b.txt"], "/repo/src", "cut", {
      "/repo/a.txt": "skip",
      "/repo/b.txt": "replace",
    });

    // Apply-to-all with Keep Both resolves every remaining conflict at once.
    mocks.pasteFiles.mockClear();
    fireEvent.click(screen.getByText("a.txt"));
    fireEvent.click(screen.getByText("b.txt"), { ctrlKey: true });
    await act(async () => {
      fireEvent.keyDown(tree, { key: "c", ctrlKey: true });
    });
    fireEvent.contextMenu(screen.getByText("src"));
    await act(async () => {
      fireEvent.click(screen.getByText("common.paste"));
    });
    const applyAllCheckbox = screen.getByRole("checkbox");
    fireEvent.click(applyAllCheckbox);
    await act(async () => {
      fireEvent.click(screen.getByText("files.pasteKeepBoth"));
    });
    expect(mocks.pasteFiles).toHaveBeenCalledWith(["/repo/a.txt", "/repo/b.txt"], "/repo/src", "copy", {
      "/repo/a.txt": "keepBoth",
      "/repo/b.txt": "keepBoth",
    });
  });
});

describe("Files missing root fallback", () => {
  it("notifies onMissingRoot without showing an error banner when rootPath does not exist", async () => {
    mocks.list.mockRejectedValueOnce(new Error("Failed to read directory: No such file or directory (os error 2)"));
    const onMissingRoot = vi.fn();

    const view = render(
      <FilesTab rootPath="/repo/deleted-sub" rootName="Repo" onMissingRoot={onMissingRoot} />,
    );
    await act(async () => {});

    expect(onMissingRoot).toHaveBeenCalledWith("/repo/deleted-sub");
    expect(view.container.querySelector(".files-err")).toBeNull();
  });

  it("still shows permission errors on rootPath instead of triggering missing-root fallback", async () => {
    mocks.list.mockRejectedValueOnce(new Error("Failed to read directory: Permission denied (os error 13)"));
    const onMissingRoot = vi.fn();

    const view = render(
      <FilesTab rootPath="/repo/restricted" rootName="Repo" onMissingRoot={onMissingRoot} />,
    );
    await act(async () => {});

    expect(onMissingRoot).not.toHaveBeenCalled();
    expect(view.container.querySelector(".files-err")?.textContent).toContain("Permission denied");
  });
});
