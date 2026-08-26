//! Tests for file transfer: sequential upload chunking, the part-file rename that keeps a half-written file
//! away from its destination, refusal of an occupied destination, cancellation, and the download handoff.
//!
//! A mocked transport.invoke stands in for the backend, holding an in-memory filesystem, so the assertions are
//! about the call sequence the transfer layer produces rather than about real disk state.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./transport", () => ({
  invoke: vi.fn(),
  copyText: vi.fn(),
  openPath: vi.fn(),
}));

import { invoke } from "./transport";
import {
  cancelTransfer,
  clearFinishedTransfers,
  getTransfers,
  startDownload,
  startUpload,
  subscribeTransfers,
  TRANSFER_CHUNK,
} from "./transfer";

/** Files present on the fake server, keyed by absolute path. */
let disk: Map<string, Uint8Array>;
/** Every command the transfer layer issued, for order assertions. */
let calls: string[];

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function bytesToB64(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

function makeBytes(n: number): Uint8Array {
  const out = new Uint8Array(n);
  for (let i = 0; i < n; i++) out[i] = (i * 13) % 251;
  return out;
}

/** Minimal File stand-in: Node's File lacks nothing needed here, but constructing one per test is noisy. */
function fakeFile(name: string, bytes: Uint8Array): File {
  return new File([bytes as BlobPart], name);
}

/** Back the mocked transport with the fake disk, mirroring the backend's offset and collision rules. */
function mockBackend(): void {
  vi.mocked(invoke).mockImplementation((cmd, args) => {
    calls.push(cmd);
    const a = (args ?? {}) as Record<string, never>;
    const path = a.path as unknown as string;
    switch (cmd) {
      case "list_dir": {
        const prefix = `${path}/`;
        const names = [...disk.keys()]
          .filter((p) => p.startsWith(prefix) && !p.slice(prefix.length).includes("/"))
          .map((p) => ({ name: p.slice(prefix.length), isDir: false, isHidden: false, gitBadge: null }));
        return Promise.resolve(names) as Promise<never>;
      }
      case "write_file_chunk": {
        const offset = a.offset as unknown as number;
        const chunk = b64ToBytes(a.bytesB64 as unknown as string);
        const cur = offset === 0 ? new Uint8Array(0) : (disk.get(path) ?? new Uint8Array(0));
        if (cur.length !== offset) return Promise.reject(new Error("Upload offset mismatch"));
        const next = new Uint8Array(cur.length + chunk.length);
        next.set(cur);
        next.set(chunk, cur.length);
        disk.set(path, next);
        return Promise.resolve(next.length) as Promise<never>;
      }
      case "rename_path": {
        const from = a.from as unknown as string;
        const to = a.to as unknown as string;
        if (disk.has(to)) return Promise.reject(new Error("A file or folder with this name already exists"));
        disk.set(to, disk.get(from)!);
        disk.delete(from);
        return Promise.resolve(null) as Promise<never>;
      }
      case "stat_file": {
        const bytes = disk.get(path);
        if (!bytes) return Promise.reject(new Error("Failed to read file metadata"));
        return Promise.resolve({ mtimeMs: 1000, size: bytes.length }) as Promise<never>;
      }
      case "delete_path":
        disk.delete(path);
        return Promise.resolve(null) as Promise<never>;
      case "create_download_ticket": {
        if (!disk.has(path)) return Promise.reject(new Error("Failed to read file metadata"));
        return Promise.resolve("/api/download?token=faketoken") as Promise<never>;
      }
      case "read_file_base64": {
        const bytes = disk.get(path);
        if (!bytes) return Promise.reject(new Error("Failed to read file metadata"));
        const offset = a.offset as unknown as number;
        const maxLen = a.maxLen as unknown as number;
        return Promise.resolve({
          base64: bytesToB64(bytes.slice(offset, offset + maxLen)),
          size: bytes.length,
          mtimeMs: 1000,
        }) as Promise<never>;
      }
      default:
        return Promise.reject(new Error(`unexpected command ${cmd}`));
    }
  });
}

beforeEach(() => {
  disk = new Map();
  calls = [];
  localStorage.clear();
  vi.mocked(invoke).mockReset();
  mockBackend();
  clearFinishedTransfers();
});

describe("startUpload", () => {
  it("writes sequential chunks into a part file and renames it onto the destination", async () => {
    const bytes = makeBytes(TRANSFER_CHUNK + 1234);
    expect(await startUpload(fakeFile("big.bin", bytes), "/srv/proj")).toBe(true);

    // Two chunks for a file just over one chunk, then the rename; the destination never sees a partial file.
    expect(calls.filter((c) => c === "write_file_chunk")).toHaveLength(2);
    expect(calls.at(-1)).toBe("rename_path");
    expect(disk.has("/srv/proj/big.bin.vlxpart")).toBe(false);
    expect(Array.from(disk.get("/srv/proj/big.bin")!)).toEqual(Array.from(bytes));

    const t = getTransfers().at(-1)!;
    expect(t.state).toBe("done");
    expect(t.transferred).toBe(bytes.length);
  });

  it("uploads an empty file, which still needs one call to create it", async () => {
    expect(await startUpload(fakeFile("empty.txt", new Uint8Array(0)), "/srv/proj")).toBe(true);
    expect(calls.filter((c) => c === "write_file_chunk")).toHaveLength(1);
    expect(disk.get("/srv/proj/empty.txt")).toEqual(new Uint8Array(0));
  });

  it("refuses an occupied destination before transferring anything", async () => {
    disk.set("/srv/proj/taken.bin", makeBytes(10));
    expect(await startUpload(fakeFile("taken.bin", makeBytes(4096)), "/srv/proj")).toBe(false);

    expect(calls).toEqual(["list_dir"]);
    expect(getTransfers().at(-1)!.state).toBe("failed");
    // The existing file is untouched, so a refused upload cannot destroy what is already there.
    expect(disk.get("/srv/proj/taken.bin")!.length).toBe(10);
  });

  it("removes the part file when the transfer is cancelled midway", async () => {
    const bytes = makeBytes(TRANSFER_CHUNK * 3);
    let seen = 0;
    const original = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((cmd, args) => {
      // Cancel after the first chunk lands, the way a click on the queue's cancel button would.
      if (cmd === "write_file_chunk" && seen++ === 0) {
        const p = original(cmd, args);
        cancelTransfer(getTransfers().at(-1)!.id);
        return p;
      }
      return original(cmd, args);
    });

    expect(await startUpload(fakeFile("cancel.bin", bytes), "/srv/proj")).toBe(false);
    expect(calls).toContain("delete_path");
    expect(disk.has("/srv/proj/cancel.bin.vlxpart")).toBe(false);
    expect(disk.has("/srv/proj/cancel.bin")).toBe(false);
    expect(getTransfers().at(-1)!.state).toBe("cancelled");
  });

  it("keeps the part file when it finally gives up, so the bytes can be resumed", async () => {
    vi.useFakeTimers();
    const original = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((cmd, args) =>
      cmd === "write_file_chunk" ? Promise.reject(new Error("disk full")) : original(cmd, args),
    );

    const done = startUpload(fakeFile("fail.bin", makeBytes(64)), "/srv/proj");
    await vi.advanceTimersByTimeAsync(120_000);
    expect(await done).toBe(false);

    const t = getTransfers().at(-1)!;
    expect(t.state).toBe("failed");
    expect(t.error).toBe("disk full");
    // Failure is not cancellation: nothing is deleted, and the resume record survives for the next attempt.
    expect(calls).not.toContain("delete_path");
    expect(localStorage.getItem("vlx-upload-resume")).toContain("fail.bin");
    vi.useRealTimers();
  });

  it("retries a dropped chunk and continues from what actually landed", async () => {
    vi.useFakeTimers();
    const bytes = makeBytes(TRANSFER_CHUNK * 2);
    const original = vi.mocked(invoke).getMockImplementation()!;
    let failures = 0;
    vi.mocked(invoke).mockImplementation(async (cmd, args) => {
      // The first chunk lands and *then* the connection drops, the worst case for naive retrying: the
      // client never learns the write succeeded, so a blind retry would write the same bytes twice.
      if (cmd === "write_file_chunk" && failures === 0) {
        failures++;
        await original(cmd, args);
        throw new Error("connection lost");
      }
      return (await original(cmd, args)) as never;
    });

    const done = startUpload(fakeFile("flaky.bin", bytes), "/srv/proj");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(await done).toBe(true);

    expect(calls).toContain("stat_file");
    expect(Array.from(disk.get("/srv/proj/flaky.bin")!)).toEqual(Array.from(bytes));
    expect(getTransfers().at(-1)!.state).toBe("done");
    vi.useRealTimers();
  });

  it("resumes from the leftover part file when the same file is uploaded again", async () => {
    const bytes = makeBytes(TRANSFER_CHUNK * 2);
    const file = fakeFile("resume.bin", bytes);
    // A previous attempt left one chunk on the server and recorded which file it came from.
    disk.set("/srv/proj/resume.bin.vlxpart", bytes.slice(0, TRANSFER_CHUNK));
    localStorage.setItem(
      "vlx-upload-resume",
      JSON.stringify({
        "/srv/proj/resume.bin": { name: file.name, size: file.size, lastModified: file.lastModified },
      }),
    );

    expect(await startUpload(file, "/srv/proj")).toBe(true);

    // Only the missing half is sent, and the assembled file is still byte-for-byte correct.
    expect(calls.filter((c) => c === "write_file_chunk")).toHaveLength(1);
    expect(getTransfers().at(-1)!.resumedFrom).toBe(TRANSFER_CHUNK);
    expect(Array.from(disk.get("/srv/proj/resume.bin")!)).toEqual(Array.from(bytes));
    expect(localStorage.getItem("vlx-upload-resume")).not.toContain("resume.bin");
  });

  it("restarts from zero when the leftover part belongs to a different file", async () => {
    const bytes = makeBytes(TRANSFER_CHUNK + 10);
    const file = fakeFile("swapped.bin", bytes);
    disk.set("/srv/proj/swapped.bin.vlxpart", makeBytes(TRANSFER_CHUNK));
    // Same name and destination, but the recorded size is not this file's: resuming would splice two files.
    localStorage.setItem(
      "vlx-upload-resume",
      JSON.stringify({
        "/srv/proj/swapped.bin": { name: file.name, size: file.size + 1, lastModified: file.lastModified },
      }),
    );

    expect(await startUpload(file, "/srv/proj")).toBe(true);

    expect(getTransfers().at(-1)!.resumedFrom).toBeUndefined();
    expect(Array.from(disk.get("/srv/proj/swapped.bin")!)).toEqual(Array.from(bytes));
  });

  it("measures throughput once there is more than one sample", async () => {
    vi.useFakeTimers();
    const original = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation(async (cmd, args) => {
      // Let exactly one second of wall clock pass per chunk, making the expected rate one chunk per second.
      if (cmd === "write_file_chunk") vi.setSystemTime(Date.now() + 1000);
      return (await original(cmd, args)) as never;
    });
    // The rate is cleared when the upload finishes, so it has to be observed while chunks are going out.
    const seen: { rate?: number; eta?: number }[] = [];
    const stop = subscribeTransfers(() => {
      const t = getTransfers().at(-1);
      if (t?.bytesPerSec) seen.push({ rate: t.bytesPerSec, eta: t.etaSec });
    });

    const done = startUpload(fakeFile("rate.bin", makeBytes(TRANSFER_CHUNK * 3)), "/srv/proj");
    await vi.advanceTimersByTimeAsync(10_000);
    await done;
    stop();

    expect(seen.length).toBeGreaterThan(0);
    // One 2 MB chunk per simulated second, and a time-left estimate that follows from it.
    expect(seen[0].rate).toBeCloseTo(TRANSFER_CHUNK, -3);
    expect(seen[0].eta).toBeGreaterThan(0);
    vi.useRealTimers();
  });
});

describe("startDownload", () => {
  it("asks for a ticket and hands the URL to the browser, holding no bytes itself", async () => {
    disk.set("/srv/proj/out.bin", makeBytes(TRANSFER_CHUNK * 4));
    const clicks: string[] = [];
    const realCreate = document.createElement.bind(document);
    vi.spyOn(document, "createElement").mockImplementation((tag: string) => {
      const el = realCreate(tag) as HTMLElement;
      if (tag === "a") el.click = () => clicks.push((el as HTMLAnchorElement).href);
      return el;
    });

    await startDownload("/srv/proj/out.bin");

    // One ticket request, and nothing that reads bytes: the transfer belongs to the browser from here on.
    expect(calls).toEqual(["create_download_ticket"]);
    expect(clicks).toHaveLength(1);
    expect(clicks[0]).toContain("/api/download?token=");
    // A download is not a queue entry, because its progress lives in the browser's own download UI.
    expect(getTransfers()).toHaveLength(0);
    vi.mocked(document.createElement).mockRestore();
  });

  it("surfaces the backend error instead of opening a dead link", async () => {
    const original = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((cmd, args) =>
      cmd === "create_download_ticket"
        ? Promise.reject(new Error("Failed to read file metadata"))
        : original(cmd, args),
    );
    await expect(startDownload("/srv/proj/missing.bin")).rejects.toThrow("metadata");
  });
});
