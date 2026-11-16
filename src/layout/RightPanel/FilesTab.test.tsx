import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FilesTab } from "./FilesTab";

type Change = { root: string; dirs: string[] | null; git: boolean };
const mocks = vi.hoisted(() => ({ list: vi.fn(), watch: vi.fn(), changed: null as null | ((event: Change) => void) }));
vi.mock("../../ipc/info", () => ({
  listDir: mocks.list, fsWatch: mocks.watch, copyText: vi.fn(), createDir: vi.fn(), createFile: vi.fn(), deletePath: vi.fn(),
  readFilePreview: vi.fn(), renamePath: vi.fn(), revealPath: vi.fn(),
}));
vi.mock("../../ipc/events", () => ({
  onFsChanged: (cb: (event: Change) => void) => { mocks.changed = cb; return Promise.resolve(() => { mocks.changed = null; }); },
}));
vi.mock("../../i18n", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../i18n")>(), useT: () => (key: string) => key,
}));
vi.mock("../../ipc/transfer", () => {
  const transfers: never[] = [];
  return { getTransfers: () => transfers, subscribeTransfers: () => () => {}, cancelTransfer: vi.fn(),
    clearFinishedTransfers: vi.fn(), startDownload: vi.fn(), startUpload: vi.fn() };
});

beforeEach(() => {
  vi.useFakeTimers();
  vi.resetAllMocks();
  // Unwatched by default, so the poll-driven tests below behave as on Linux.
  mocks.watch.mockResolvedValue({ root: "/repo", supported: false });
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
