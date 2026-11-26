//! Resolve OS file-clipboard entries during paste so copied files and folders insert their paths
//! instead of falling back to Finder's synthesized file-icon bitmap.

import { env } from "../platform/env";
import { chatDropText } from "./dropPaths";
import { imagesFromClipboard } from "./imageInput";

/** Safety cap matching `attachments.ts` so oversized files are not read into memory on paste. */
const MAX_RAW_IMAGE_BYTES = 50 * 1024 * 1024;

const IMAGE_EXT_MIME: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  bmp: "image/bmp",
  svg: "image/svg+xml",
  tiff: "image/tiff",
  tif: "image/tiff",
  avif: "image/avif",
  ico: "image/x-icon",
};

/** Whether this client runs in a local desktop shell whose OS clipboard shares the session filesystem. */
export function isLocalDesktop(): boolean {
  return Boolean(env.isTauri || env.isElectron);
}

/** Synchronously extracted snapshot of `ClipboardEvent.clipboardData` before async IPC runs. */
export interface CapturedPasteData {
  images: File[];
  hasNonImageFiles: boolean;
  hasFileSignal: boolean;
  text: string;
  hasTextType: boolean;
}

/** Capture `DataTransfer` fields synchronously; browsers clear `clipboardData` once the handler returns. */
export function capturePasteData(data: DataTransfer | null): CapturedPasteData {
  const images = imagesFromClipboard(data);
  const items = Array.from(data?.items ?? []);
  const rawFiles = Array.from(data?.files ?? []);
  const types = Array.from(data?.types ?? []);
  const hasNonImageFiles =
    items.some((item) => item.kind === "file" && !item.type.startsWith("image/")) ||
    rawFiles.some((file) => !file.type.startsWith("image/"));
  const text = typeof data?.getData === "function" ? data.getData("text/plain") : "";
  const hasFileSignal = images.length > 0 || hasNonImageFiles || types.includes("Files");
  const hasTextType = Boolean(text) || types.some((t) => t.startsWith("text/"));
  return { images, hasNonImageFiles, hasFileSignal, text, hasTextType };
}

function basenameOfPath(path: string): string {
  const trimmed = path.length > 1 ? path.replace(/[\\/]+$/, "") : path;
  return trimmed.split(/[\\/]/).pop() ?? trimmed;
}

/** Return the image MIME type for a file path based on extension, or null for non-images/trailing slashes. */
export function imageMimeForPath(path: string): string | null {
  if (!path || /[\\/]$/.test(path)) return null;
  const name = basenameOfPath(path);
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return null;
  return IMAGE_EXT_MIME[name.slice(dot + 1).toLowerCase()] ?? null;
}

async function loadImageFileFromPath(path: string, mime: string): Promise<File | null> {
  try {
    const { loadFileBlob, statFile } = await import("../ipc/info");
    const stat = await statFile(path);
    if (!stat || stat.size > MAX_RAW_IMAGE_BYTES) return null;
    const loaded = await loadFileBlob(path);
    if (!loaded) return null;
    return new File([loaded.blob], basenameOfPath(path), { type: mime });
  } catch {
    return null;
  }
}

export interface ResolvedClipboardFiles {
  allPaths: string[];
  imagePaths: string[];
  otherPaths: string[];
  imageFiles: File[];
}

/**
 * Read OS file URLs / `CF_HDROP` from the local system clipboard and partition them into image files vs
 * non-image paths. Synthesized Finder icon bitmaps in `capturedImages` that do not correspond to an
 * actual copied image path are discarded.
 */
export async function resolveClipboardFiles(
  capturedImages: File[] = [],
  options?: { loadMissingImages?: boolean },
): Promise<ResolvedClipboardFiles | null> {
  if (!isLocalDesktop()) return null;
  const mod = await import("../platform");
  const paths = (await mod.platform?.clipboard?.readFiles?.()) ?? [];
  if (!paths.length) return null;

  const byName = new Map<string, File[]>();
  for (const file of capturedImages) {
    const bucket = byName.get(file.name);
    if (bucket) bucket.push(file);
    else byName.set(file.name, [file]);
  }

  const imagePaths: string[] = [];
  const otherPaths: string[] = [];
  const imageFiles: File[] = [];

  for (const path of paths) {
    const mime = imageMimeForPath(path);
    if (!mime) {
      otherPaths.push(path);
      continue;
    }
    const name = basenameOfPath(path);
    const matched = byName.get(name)?.shift();
    if (matched) {
      imagePaths.push(path);
      imageFiles.push(matched);
      continue;
    }
    if (options?.loadMissingImages) {
      const loaded = await loadImageFileFromPath(path, mime);
      if (loaded) {
        imagePaths.push(path);
        imageFiles.push(loaded);
        continue;
      }
      // Unreadable or directory entry ending with an image-like suffix: fall back to inserting its path.
      otherPaths.push(path);
      continue;
    }
    imagePaths.push(path);
  }

  return { allPaths: paths, imagePaths, otherPaths, imageFiles };
}

/** Format file paths for insertion at `[start, end)` in a chat/prompt text field, preserving surrounding spaces. */
export function formatSpacedChatPaths(
  current: string,
  start: number,
  end: number,
  paths: string[],
): { value: string; insert: string; caret: number } {
  const safeStart = Math.max(0, Math.min(start, current.length));
  const safeEnd = Math.max(safeStart, Math.min(end, current.length));
  const before = current.slice(0, safeStart);
  const after = current.slice(safeEnd);
  const insert =
    (before && !/\s$/.test(before) ? " " : "") +
    chatDropText(paths) +
    (/^\s/.test(after) ? "" : " ");
  return {
    value: before + insert + after,
    insert,
    caret: before.length + insert.length,
  };
}

/** Replace the current selection of an input/textarea while keeping browser undo and React `onChange` in sync. */
export function replaceTextFieldSelection(
  field: HTMLTextAreaElement | HTMLInputElement,
  text: string,
): void {
  field.focus({ preventScroll: true });
  if (document.execCommand?.(text ? "insertText" : "delete", false, text)) return;
  const start = field.selectionStart ?? 0;
  const end = field.selectionEnd ?? start;
  field.setRangeText(text, start, end, "end");
  field.dispatchEvent(new Event("input", { bubbles: true }));
}

/** Insert formatted file paths at the current selection of a controlled input/textarea. */
export function insertPathsIntoTextField(
  field: HTMLTextAreaElement | HTMLInputElement,
  paths: string[],
): void {
  if (!paths.length) return;
  const start = field.selectionStart ?? field.value.length;
  const end = field.selectionEnd ?? start;
  const { insert } = formatSpacedChatPaths(field.value, start, end, paths);
  replaceTextFieldSelection(field, insert);
}
