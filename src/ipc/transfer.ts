//! File transfer between the browser and the machine the server runs on, for remote access and remote-connection
//! windows where the two are different machines.
//!
//! The two directions are deliberately asymmetric.
//!
//! **Download** is a plain HTTP GET, and none of it happens here beyond asking for a URL. Pulling bytes through
//! the command channel would mean JavaScript holds the whole file before it can offer it — no streaming to
//! disk, no speed readout, no pause or resume, and a size limit set by the tab's memory. A real HTTP response
//! hands all of that to the browser's own download manager. The credential problem that made this look
//! impossible (this server has no cookies, and a navigating browser sends no Authorization header) is solved
//! the same way `/ws` solves it: a ticket in the URL, minted over the authenticated socket for one path and
//! valid for minutes.
//!
//! **Upload** does go through the command channel, in `TRANSFER_CHUNK` slices, because it is the browser that
//! has the bytes and `File.slice` already reads them lazily — nothing is held in memory that a chunked POST
//! would avoid. What a download gets from the browser for free, an upload has to provide here: the queue below
//! measures throughput over a moving window, retries a failed chunk instead of abandoning the transfer, and
//! keeps its partial file so the same file dropped again continues rather than restarts.
//!
//! Uploads outlive the panel that started them: the queue is a module-level store rather than component state,
//! so switching right-panel tabs or collapsing the panel does not abort one in flight.

import { deletePath, listDir, renamePath, statFile, writeFileChunk } from "./info";
import { invoke } from "./transport";

/** One queued upload. */
export interface Transfer {
  id: string;
  /** Kept for the queue's icon, and because a future direction would land in the same list. */
  direction: "upload";
  /** File name shown in the queue. */
  name: string;
  /** Absolute destination path on the server. */
  path: string;
  transferred: number;
  /** Total bytes. */
  total: number;
  /** "stalled" means a chunk failed and the upload is waiting to retry, not that it is over. */
  state: "active" | "stalled" | "done" | "failed" | "cancelled";
  /** Backend message when `state` is "failed". */
  error?: string;
  /** Recent throughput, absent until two samples exist. */
  bytesPerSec?: number;
  /** Seconds left at the current rate, absent while throughput is unknown. */
  etaSec?: number;
  /** Which retry is pending while `state` is "stalled". */
  retry?: number;
  /** Bytes already on the server when this upload started, so a resumed one reports honest progress. */
  resumedFrom?: number;
}

/**
 * Bytes per upload chunk.
 *
 * Base64 inflates this to ~2.7 MB per message, well under the backend's 8 MB per-call ceiling, and small
 * enough that terminal output still interleaves between chunks on the same connection.
 */
export const TRANSFER_CHUNK = 2 * 1024 * 1024;

/** Suffix for the partial file an upload writes into before renaming it onto the destination. */
const PART_SUFFIX = ".vlxpart";

let queue: Transfer[] = [];
const listeners = new Set<() => void>();
/** Ids the user cancelled; the upload loop checks this between chunks. */
const cancelled = new Set<string>();

function emit(): void {
  queue = [...queue];
  for (const l of listeners) l();
}

/** Subscribe to queue changes; pairs with `getTransfers` for useSyncExternalStore. */
export function subscribeTransfers(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Current queue snapshot; identity changes only when something actually changed. */
export function getTransfers(): Transfer[] {
  return queue;
}

function update(id: string, patch: Partial<Transfer>): void {
  const t = queue.find((x) => x.id === id);
  if (!t) return;
  Object.assign(t, patch);
  emit();
}

/** Mark a transfer for cancellation; its loop stops before the next chunk and cleans up after itself. */
export function cancelTransfer(id: string): void {
  cancelled.add(id);
}

/** Drop finished entries (done, failed, or cancelled) from the queue, leaving active ones running. */
export function clearFinishedTransfers(): void {
  queue = queue.filter((t) => t.state === "active" || t.state === "stalled");
  emit();
}

let seq = 0;

function add(name: string, path: string, total: number): Transfer {
  const t: Transfer = { id: `tr${++seq}`, direction: "upload", name, path, transferred: 0, total, state: "active" };
  queue = [...queue, t];
  emit();
  return t;
}

/** Encode bytes as base64 in slices, because one apply() over a multi-megabyte array overflows the call stack. */
function bytesToBase64(bytes: Uint8Array): string {
  const STEP = 0x8000;
  let bin = "";
  for (let i = 0; i < bytes.length; i += STEP) {
    bin += String.fromCharCode(...bytes.subarray(i, i + STEP));
  }
  return btoa(bin);
}

/**
 * Hand a server file to the browser's download manager.
 *
 * The ticket is requested over the authenticated socket, where the remote data-directory ACL applies and a
 * missing file is reported as an error rather than as a dead link. Everything after that — streaming to disk,
 * progress, speed, pause and resume — belongs to the browser, which is the point of doing it this way.
 *
 * The anchor never navigates the page away: the response carries `Content-Disposition: attachment`, which is
 * defined to start a download instead of a navigation.
 */
export async function startDownload(path: string): Promise<void> {
  const url = await invoke<string>("create_download_ticket", { path });
  const a = document.createElement("a");
  a.href = url;
  document.body.appendChild(a);
  a.click();
  a.remove();
}

/** Remove an abandoned part file, best-effort: a stray `.vlxpart` is worse than a failed cleanup. */
async function discardPart(part: string, started: boolean): Promise<void> {
  if (!started) return;
  await deletePath(part).catch(() => {});
}

// ── Throughput ──────────────────────────────────────────────────────────────────────────────────────

/** Recent (timestamp, bytes) samples per transfer, used for the rate shown in the queue. */
const samples = new Map<string, { t: number; bytes: number }[]>();

/** Seconds of history the rate is averaged over: long enough to ride out one slow chunk, short enough to react. */
const RATE_WINDOW_MS = 5000;

/**
 * Fold one progress point into a transfer's rate estimate.
 *
 * Averaging over a window rather than dividing total bytes by total elapsed time matters here: chunks are
 * megabytes apart, a stall of a few seconds is normal, and a whole-transfer average would keep reporting the
 * speed of a minute ago long after the link changed.
 */
function trackRate(t: Transfer, transferred: number): Pick<Transfer, "bytesPerSec" | "etaSec"> {
  const now = Date.now();
  const list = samples.get(t.id) ?? [];
  list.push({ t: now, bytes: transferred });
  while (list.length > 2 && now - list[0].t > RATE_WINDOW_MS) list.shift();
  samples.set(t.id, list);
  const first = list[0];
  const span = now - first.t;
  if (list.length < 2 || span <= 0) return {};
  const bytesPerSec = ((transferred - first.bytes) * 1000) / span;
  if (bytesPerSec <= 0) return {};
  return { bytesPerSec, etaSec: Math.max(0, (t.total - transferred) / bytesPerSec) };
}

// ── Resume ──────────────────────────────────────────────────────────────────────────────────────────

/** localStorage key holding, per destination path, which local file its `.vlxpart` was being written from. */
const RESUME_KEY = "vlx-upload-resume";

/** Enough of a local file's identity to be confident a leftover part file is a prefix of *this* file. */
interface ResumeRecord {
  name: string;
  size: number;
  lastModified: number;
}

function readResumeMap(): Record<string, ResumeRecord> {
  try {
    return JSON.parse(localStorage.getItem(RESUME_KEY) || "{}") as Record<string, ResumeRecord>;
  } catch {
    return {};
  }
}

/** Remember, or with a null record forget, which file a destination's part file belongs to. */
function writeResumeRecord(dest: string, rec: ResumeRecord | null): void {
  try {
    const map = readResumeMap();
    if (rec) map[dest] = rec;
    else delete map[dest];
    localStorage.setItem(RESUME_KEY, JSON.stringify(map));
  } catch {
    // Private mode or a full quota only costs the ability to resume across a reload; the upload still runs.
  }
}

/**
 * Decide where an upload should pick up, given a part file that already exists on the server.
 *
 * Resuming into someone else's leftovers would splice two files together, so the local file's identity has to
 * match what was recorded when that part file was being written. Name, size and modification time together are
 * what the browser can offer without reading the file, and editing a file changes at least one of them.
 * Anything that does not match starts from zero, which is safe because offset 0 truncates.
 */
async function resumeOffset(dest: string, part: string, file: File): Promise<number> {
  const rec = readResumeMap()[dest];
  if (!rec || rec.name !== file.name || rec.size !== file.size || rec.lastModified !== file.lastModified) {
    return 0;
  }
  const size = await statFile(part).then(
    (st) => st.size,
    () => 0, // No part file after all, or it is unreadable: start over.
  );
  // A part at or past the full size is not a prefix of anything; treat it as junk and rewrite from the start.
  return size > 0 && size < file.size ? size : 0;
}

/** Delay between retries, backing off but capped so a long outage still gets checked regularly. */
function retryDelayMs(attempt: number): number {
  return Math.min(15000, 1000 * 2 ** (attempt - 1));
}

/** How many consecutive failures on one chunk before the upload gives up. Roughly a minute of retrying. */
const MAX_ATTEMPTS = 6;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/**
 * Upload a local file into a server directory, resolving true once it lands at its final path.
 *
 * Chunks go into a `.vlxpart` sibling that is renamed onto the destination at the end, so an interrupted
 * transfer never leaves a truncated file where a complete one is expected. An existing destination is refused
 * up front rather than after moving every byte.
 *
 * A failed chunk does not end the upload. The connection dropping mid-transfer is the ordinary case, not an
 * exceptional one, so a failure backs off and retries, and each retry asks the server how long the part file
 * actually is — which settles the question of whether the chunk landed before the connection went away.
 *
 * Giving up keeps the part file and the resume record on purpose. Dropping the same file on the same folder
 * again continues from where it stopped; only cancelling discards the bytes already sent.
 */
export async function startUpload(file: File, destDir: string): Promise<boolean> {
  const dest = `${destDir.replace(/[\\/]+$/, "")}/${file.name}`;
  const part = `${dest}${PART_SUFFIX}`;
  const t = add(file.name, dest, file.size);
  // Whether a chunk write was attempted, and therefore whether a part file may exist to clean up. Deleting
  // unconditionally would raise the global request-error banner for a path that was never created.
  let started = false;
  try {
    const siblings = await listDir(destDir);
    if (siblings.some((e) => e.name === file.name)) {
      update(t.id, { state: "failed", error: "A file or folder with this name already exists" });
      return false;
    }
    const partName = `${file.name}${PART_SUFFIX}`;
    let offset = siblings.some((e) => e.name === partName) ? await resumeOffset(dest, part, file) : 0;
    if (offset > 0) {
      started = true;
      update(t.id, { transferred: offset, resumedFrom: offset });
    }
    writeResumeRecord(dest, { name: file.name, size: file.size, lastModified: file.lastModified });

    let attempt = 0;
    // An empty file still needs one call, to create the part file that the rename then moves into place.
    do {
      if (cancelled.has(t.id)) {
        await discardPart(part, started);
        writeResumeRecord(dest, null);
        update(t.id, { state: "cancelled" });
        return false;
      }
      const slice = file.slice(offset, offset + TRANSFER_CHUNK);
      const bytes = new Uint8Array(await slice.arrayBuffer());
      try {
        started = true;
        offset = await writeFileChunk(part, offset, bytesToBase64(bytes));
        attempt = 0;
        update(t.id, { state: "active", retry: undefined, transferred: offset, ...trackRate(t, offset) });
      } catch (e) {
        if (++attempt > MAX_ATTEMPTS) throw e;
        update(t.id, { state: "stalled", retry: attempt, bytesPerSec: undefined, etaSec: undefined });
        await sleep(retryDelayMs(attempt));
        // Re-read the part length rather than assuming: the chunk may well have landed before the connection
        // dropped, and rewriting it would be refused by the offset check anyway. A failed stat leaves the
        // offset alone, so the next attempt simply tries the same chunk again.
        offset = await statFile(part).then((st) => st.size, () => offset);
        samples.delete(t.id); // The gap is not throughput; start the rate estimate fresh.
      }
    } while (offset < file.size);

    await renamePath(part, dest);
    writeResumeRecord(dest, null);
    update(t.id, { state: "done", transferred: file.size, bytesPerSec: undefined, etaSec: undefined });
    return true;
  } catch (e) {
    // The part file and its resume record survive a failure on purpose: dropping the same file again picks up
    // where this left off. Only cancelling throws the bytes away.
    update(t.id, {
      state: "failed",
      error: e instanceof Error ? e.message : String(e),
      bytesPerSec: undefined,
      etaSec: undefined,
    });
    return false;
  } finally {
    cancelled.delete(t.id);
    samples.delete(t.id);
  }
}
