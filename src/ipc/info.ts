//! Info-panel IPC for Git status, directory listing, file preview, opening directories, and copying paths, routed through transport.

import { copyText as transportCopyText, invoke, openPath, revealPath as transportRevealPath } from "./transport";
import type { GitStatus } from "../types";

export function getGitStatus(path: string): Promise<GitStatus> {
  return invoke<GitStatus>("get_git_status", { path });
}

/** One direct child of a directory, lazily loaded one level at a time by the right-panel Files tree. */
export interface DirEntry {
  name: string;
  isDir: boolean;
  /** Whether this is hidden (name starts with `.`, such as `.claude`/`.env`); hidden by default with a UI toggle. */
  isHidden: boolean;
  /** Git-change badge (M/A/U/D), or null when unchanged. */
  gitBadge: string | null;
}

/** List a directory's direct children, directories first and sorted by name, including Git badges. */
export function listDir(path: string): Promise<DirEntry[]> {
  return invoke<DirEntry[]>("list_dir", { path });
}

/** Create an empty file from the Files context menu/+; the backend rejects existing paths without overwriting. */
export function createFile(path: string): Promise<void> {
  return invoke<void>("create_file", { path });
}

/** Create a directory from the Files context menu/+; the backend rejects existing paths. */
export function createDir(path: string): Promise<void> {
  return invoke<void>("create_dir", { path });
}

/** Rename/move a file or directory; the backend rejects an existing destination without overwriting. */
export function renamePath(from: string, to: string): Promise<void> {
  return invoke<void>("rename_path", { from, to });
}

/** Delete a file or recursively delete a directory. */
export function deletePath(path: string): Promise<void> {
  return invoke<void>("delete_path", { path });
}

/** File-preview content fragment. */
export interface FilePreview {
  content: string;
  truncated: boolean;
  binary: boolean;
}

/** Read an initial byte range for preview; truncate at the limit and return no content for binary files. */
export function readFilePreview(path: string): Promise<FilePreview> {
  return invoke<FilePreview>("read_file_preview", { path });
}

/** Editor read result for document tabs: content, millisecond mtime for optimistic-save locking, and truncation metadata. */
export interface TextFile {
  content: string;
  mtimeMs: number;
  /** File exceeds the 10 MB limit: only the first 10 MB were read, so the frontend opens read-only and forbids saving. */
  truncated: boolean;
  /** Actual on-disk byte count, used by the "showing first 10 MB of X" truncation notice. */
  fullSize: number;
}

/** Read a strict UTF-8 text file; read only the first 10 MB and mark truncated above the limit, rejecting binary/non-UTF-8 data. */
export function readTextFile(path: string): Promise<TextFile> {
  return invoke<TextFile>("read_text_file", { path });
}

/** Write result: conflict=true means the file changed externally and nothing was written. */
export interface WriteOutcome {
  conflict: boolean;
  mtimeMs: number;
}

/**
 * Write a text file. If non-null `expectedMtimeMs` differs from the current disk mtime, do not write and return
 * conflict=true. Pass null to force overwrite after the user chooses Overwrite in conflict confirmation.
 */
export function writeTextFile(
  path: string,
  content: string,
  expectedMtimeMs: number | null,
): Promise<WriteOutcome> {
  return invoke<WriteOutcome>("write_text_file", { path, content, expectedMtimeMs });
}

/** Lightweight stat used by document tabs to poll for external changes every two seconds. */
export interface FileStat {
  mtimeMs: number;
  size: number;
}

export function statFile(path: string): Promise<FileStat> {
  return invoke<FileStat>("stat_file", { path });
}

/** One `read_file_base64` chunk: Base64 byte segment, total file size, and current mtime. */
export interface FileChunk {
  base64: string;
  size: number;
  mtimeMs: number;
}

/** Read `[offset, offset+maxLen)` and encode it as Base64, capped at 50 MB and used by chunked image viewing. */
export function readFileBase64(
  path: string,
  offset: number,
  maxLen: number,
): Promise<FileChunk> {
  return invoke<FileChunk>("read_file_base64", { path, offset, maxLen });
}

/**
 * Append one upload chunk, truncating the file at offset 0, and resolve to the file's new length.
 *
 * The backend refuses any offset that is not the file's current length, so callers must upload sequentially
 * and must not retry a chunk that already landed.
 */
export function writeFileChunk(
  path: string,
  offset: number,
  bytesB64: string,
): Promise<number> {
  return invoke<number>("write_file_chunk", { path, offset, bytesB64 });
}

/** Chunk size: 2 MB (~2.7 MB after Base64), keeping remote WS messages bounded and allowing terminal traffic between chunks. */
export const FILE_BLOB_CHUNK = 2 * 1024 * 1024;

/** Error marker thrown when loadFileBlob remains unstable after three rereads of a continuously written file; UI localizes it. */
export const FILE_BEING_WRITTEN = "vlx:file-being-written";

/** loadFileBlob result: complete bytes plus final disk mtime used as the external-change polling baseline. */
export interface FileBlob {
  blob: Blob;
  mtimeMs: number;
}

/** Decode one independently encoded Base64 chunk; concatenating decoded chunks reconstructs the original file. */
function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/**
 * Read a complete file in chunks and assemble a Blob for the image viewer. Sending 50 MB at once would monopolize
 * the remote WS channel; chunks let terminal traffic interleave, and the first chunk returns total size so progress
 * is accurate from the start.
 *
 * - `onProgress(received bytes, total bytes)`: called after each chunk.
 * - `isCancelled`: checked before and after each request so unmount/closed tabs stop transfer; cancellation returns null.
 * - Cross-chunk consistency: if mtime changes between chunks while the file is being written, discard received
 *   data and reread automatically. After three unstable attempts, throw `FILE_BEING_WRITTEN`; the caller's
 *   two-second poll reloads once the file stabilizes.
 * - `chunkLen` is test-only; application code always uses the 2 MB default.
 */
export async function loadFileBlob(
  path: string,
  onProgress?: (received: number, total: number) => void,
  isCancelled?: () => boolean,
  chunkLen: number = FILE_BLOB_CHUNK,
): Promise<FileBlob | null> {
  const MAX_REREADS = 3;
  let rereads = 0;
  for (;;) {
    const chunks: Uint8Array[] = [];
    let received = 0;
    let mtime: number | null = null;
    let torn = false;
    for (;;) {
      if (isCancelled?.()) return null;
      const c = await readFileBase64(path, received, chunkLen);
      if (isCancelled?.()) return null;
      if (mtime != null && c.mtimeMs !== mtime) {
        torn = true;
        break;
      }
      mtime = c.mtimeMs;
      const bytes = b64ToBytes(c.base64);
      if (bytes.length === 0 && received < c.size) {
        // An empty chunk before completion means the file was truncated, possibly within one mtime millisecond; treat it as inconsistency and reread.
        torn = true;
        break;
      }
      chunks.push(bytes);
      received += bytes.length;
      onProgress?.(received, c.size);
      if (received >= c.size) {
        return { blob: new Blob(chunks as BlobPart[]), mtimeMs: mtime };
      }
    }
    if (torn && ++rereads >= MAX_REREADS) {
      throw new Error(FILE_BEING_WRITTEN);
    }
  }
}

/** CPU percentage and memory for the process subtree, including shell, agent, and child commands. */
export interface ProcStats {
  cpu: number;
  rssBytes: number;
}

/** Sample CPU/memory for a session process subtree; return null if the process no longer exists. */
export function processStats(pid: number): Promise<ProcStats | null> {
  return invoke<ProcStats | null>("process_stats", { pid });
}

/** Whole-machine resource usage. For a remote session this describes the remote host, not this laptop. */
export interface SystemStats {
  /** 0-100 across all cores, unlike ProcStats.cpu which sums per-core percentages. */
  cpu: number;
  cores: number;
  memUsed: number;
  memTotal: number;
  swapUsed: number;
  swapTotal: number;
  /** macOS kernel memory pressure level; null on other platforms. */
  pressure: "normal" | "warning" | "critical" | null;
  /** macOS memory pressure as 0-100, higher meaning more pressure; null on other platforms. */
  pressurePct: number | null;
  /** Linux 1/5/15-minute load averages; null on other platforms. */
  load: [number, number, number] | null;
}

/** Sample whole-machine CPU, memory, swap, and the platform's saturation signal (pressure or load). */
export function systemStats(): Promise<SystemStats> {
  return invoke<SystemStats>("system_stats", {});
}

/** Open a directory with the system default application/file manager; silently no-op in browsers. */
export function openDir(path: string): Promise<void> {
  return openPath(path);
}

/** Reveal and select the path in Finder/file manager on desktop; silently no-op in browsers. */
export function revealPath(path: string): Promise<void> {
  return transportRevealPath(path);
}

/** Copy text to the clipboard, automatically falling back to execCommand over plaintext HTTP. */
export function copyText(text: string): Promise<void> {
  return transportCopyText(text);
}
