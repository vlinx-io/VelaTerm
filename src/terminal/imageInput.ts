import { safeError } from "../ipc/diagnosticSafety";
//! Feed pasted or dropped terminal images to an agent by uploading temporary files and writing their
//! paths into terminal input. Claude reads the path directly; Codex receives an `image_path:` prefix
//! so it remains visible instead of collapsing a lone path into `[Image #N]`.
//!
//! The image lives on the user's clipboard while the agent may run across the network. Desktop uses
//! the same upload path into a local temporary file, keeping behavior consistent across clients.

import { ptyWrite } from "../ipc/commands";
import { uploadImage } from "../ipc/transport";
import { platform } from "../platform";
import type { ImagePasteMode } from "../store/settings";
import type { SessionKind } from "../types";

/** Map MIME types to file extensions. */
const MIME_EXT: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/gif": "gif",
  "image/webp": "webp",
  "image/bmp": "bmp",
  "image/svg+xml": "svg",
  "image/tiff": "tiff",
};

export function extOf(file: File): string {
  return MIME_EXT[file.type] ?? file.name.split(".").pop() ?? "png";
}

/** Extract image files from a paste event's clipboardData. */
export function imagesFromClipboard(data: DataTransfer | null): File[] {
  if (!data) return [];
  const out: File[] = [];
  for (const item of Array.from(data.items)) {
    if (item.kind === "file" && item.type.startsWith("image/")) {
      const f = item.getAsFile();
      if (f) out.push(f);
    }
  }
  return out;
}

/** Whether `clipboardData` carries at least one non-image file entry (e.g. a document copied in Finder). */
export function hasNonImageClipboardFiles(data: DataTransfer | null): boolean {
  if (!data) return false;
  const items = Array.from(data.items ?? []);
  if (items.some((item) => item.kind === "file" && !item.type.startsWith("image/"))) {
    return true;
  }
  return Array.from(data.files ?? []).some((file) => !file.type.startsWith("image/"));
}

/**
 * Select the path for one paste event while preserving three invariants:
 * 1. Native agent paste requires explicit user selection and shell support.
 * 2. Path mode must intercept and upload image files.
 * 3. When WKWebView omits images from DataTransfer, local Tauri reads the native clipboard instead
 *    of silently falling through to xterm and producing `[Image #x]` in Codex.
 */
export type ImagePastePlan =
  | { kind: "agent" }
  | { kind: "images"; files: File[] }
  | { kind: "non-image-files" }
  | { kind: "text" }
  | { kind: "native-clipboard" }
  | { kind: "image-unavailable" };

export function planImagePaste(
  data: DataTransfer | null,
  mode: ImagePasteMode,
  nativePasteAvailable: boolean,
  nativeClipboardFallbackAvailable: boolean,
): ImagePastePlan {
  const files = imagesFromClipboard(data);
  const nonImageFiles = hasNonImageClipboardFiles(data);
  const text = typeof data?.getData === "function" ? data.getData("text/plain") : "";
  if (nonImageFiles && files.length === 0) {
    return { kind: "non-image-files" };
  }
  // In local native mode, VelaTerm converts image paste into the Ctrl+V agents recognize. Letting it
  // reach xterm would paste empty text because images lack text/plain, so the agent would never read
  // the system clipboard. Plain text still follows xterm's normal path.
  if (mode === "agent" && nativePasteAvailable) {
    if (files.length > 0 || !text) return { kind: "agent" };
    return { kind: "text" };
  }
  if (files.length > 0) return { kind: "images", files };
  if (text) return { kind: "text" };
  return nativeClipboardFallbackAvailable
    ? { kind: "native-clipboard" }
    : { kind: "image-unavailable" };
}

/** Claude and Codex interactive TUIs use Ctrl+V to read image data from their host clipboard. */
export function supportsNativeImagePaste(kind: SessionKind): boolean {
  return kind === "claude" || kind === "codex";
}

/**
 * Trigger native agent image paste by explicitly writing Ctrl+V byte 0x16 to the PTY, allowing
 * Claude/Codex to read the system clipboard without relying on xterm's empty image-paste behavior.
 */
export function injectNativeImagePaste(sessionId: string): Promise<void> {
  return ptyWrite(sessionId, "\x16");
}

/**
 * Tauri/WKWebView fallback for paste events that omit DataTransfer image files: read RGBA from the
 * native clipboard, encode a PNG File, and reuse the upload path.
 */
export async function imageFromNativeClipboard(): Promise<File> {
  const image = await platform.clipboard.readImage();
  if (!image) throw new Error("Clipboard image is unavailable");
  const { width, height, rgba } = image;
  if (!width || !height || rgba.length !== width * height * 4) {
    throw new Error("Invalid clipboard image data");
  }
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Canvas 2D is unavailable");
  ctx.putImageData(new ImageData(new Uint8ClampedArray(rgba), width, height), 0, 0);
  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (value) => (value ? resolve(value) : reject(new Error("PNG encoding failed"))),
      "image/png",
    );
  });
  return new File([blob], "clipboard.png", { type: "image/png" });
}

/** Extract image files from a drop event's dataTransfer. */
export function imagesFromDrop(data: DataTransfer | null): File[] {
  if (!data) return [];
  return Array.from(data.files).filter((f) => f.type.startsWith("image/"));
}

/** Aggregate image-injection result used to decide whether to show user feedback. */
export interface InjectImagesResult {
  ok: number;
  fail: number;
  /** Last failure reason for the feedback banner. */
  lastError?: string;
}

/**
 * Upload images sequentially and write server paths surrounded by spaces into terminal input. Codex
 * also receives a visible prefix. Skip individual failures without stopping other images, but report
 * their count because console-only failures made remote paste problems silent and difficult to diagnose.
 */
export async function injectImageFiles(
  sessionId: string,
  files: File[],
  kind: SessionKind,
): Promise<InjectImagesResult> {
  const result: InjectImagesResult = { ok: 0, fail: 0 };
  for (const file of files) {
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const path = await uploadImage(bytes, extOf(file));
      // Codex converts a lone valid image path into a collapsed attachment. Prefix it in path mode so
      // the reference stays visible; Claude and other agents retain the bare path.
      const input = kind === "codex" ? ` image_path: ${path} ` : ` ${path} `;
      await ptyWrite(sessionId, input);
      result.ok += 1;
    } catch (e) {
      console.error("image paste/drop failed", safeError(e));
      result.fail += 1;
      result.lastError = e instanceof Error ? e.message : String(e);
    }
  }
  return result;
}
