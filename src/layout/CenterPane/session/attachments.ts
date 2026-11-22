//! Images waiting in the composer, from the moment they are pasted or dropped until they are sent.
//!
//! The chat engine takes an image as base64 inside the message itself, so nothing is written to disk on the
//! way — unlike the terminal path in `terminal/imageInput.ts`, which uploads a file and types its path,
//! because a terminal can only carry text. What the two share is how an image is fished out of a paste or a
//! drop, and that part is reused rather than written twice.
//!
//! An image wider or taller than `MAX_IMAGE_EDGE` or larger than the configured size cap is redrawn
//! and compressed before it is sent. See `MAX_IMAGE_EDGE` and `DEFAULT_IMAGE_BYTES` for why.

import { genId } from "../../../genId";
import type { ChatImage, ChatImageValue } from "../../../ipc/chat";
import { DEFAULT_CHAT_IMAGE_MAX_MB } from "../../../store/settings";

/**
 * How much can ride along with one message.
 *
 * `MAX_IMAGE_BYTES` is the hard ceiling enforced in `command_core::chat_send` (5 MB), while
 * `DEFAULT_IMAGE_BYTES` is the default target that images are compressed down to on attachment.
 */
export const MAX_IMAGES = 20;
export const MAX_IMAGE_BYTES = 5 * 1024 * 1024;
export const DEFAULT_IMAGE_BYTES = DEFAULT_CHAT_IMAGE_MAX_MB * 1024 * 1024;
/** Safety cap on raw input size before decoding so giant images cannot stall or exhaust WebView memory. */
export const MAX_RAW_IMAGE_BYTES = 50 * 1024 * 1024;

const JPEG_INITIAL_QUALITIES = [0.92, 0.84, 0.74, 0.62, 0.5] as const;
const JPEG_SMALL_EDGES = [1280, 1024, 768, 512, 384] as const;
const JPEG_SMALL_QUALITIES = [0.82, 0.68, 0.54] as const;

/**
 * The longest edge an image is sent at.
 *
 * Two reasons for redrawing anything larger. An image costs tokens by its pixel count alone, and Anthropic
 * scales everything past this edge down to it anyway, so those pixels are paid for and then discarded. And
 * once a request carries more than twenty images, the API refuses any image measuring 2000px or more —
 * which is exactly where Claude Code's own resizing leaves a large screenshot, so a long conversation full
 * of screenshots eventually fails as a whole. Arriving under that figure keeps both problems away.
 */
export const MAX_IMAGE_EDGE = 1568;

/** One image held in the composer, not yet sent. */
export interface Attachment extends ChatImage {
  /** Identifies the thumbnail while it is on screen; never leaves the frontend. */
  id: string;
  /** The file's own name, shown as the thumbnail's tooltip. */
  name: string;
  /** Size in bytes of what is sent, before base64 — smaller than the file when the image was redrawn. */
  bytes: number;
}

/** Why a file was left out, in the terms the composer explains it in. */
export type Rejection =
  | { reason: "tooMany" }
  | { reason: "tooLarge"; name: string }
  | { reason: "unreadable"; name: string };

export interface AttachResult<T extends ChatImageValue = Attachment> {
  /** The attachments the composer should hold now: what it had, plus whatever was accepted. */
  attachments: (T | Attachment)[];
  /** Everything left out, in the order it was offered. Empty when all of it was taken. */
  rejected: Rejection[];
}

/** What an `<img>` needs to draw one of these. */
export function dataUrl(image: ChatImage): string {
  return `data:${image.mimeType};base64,${image.data}`;
}

/** Restore a sent image to the composer. History retains its bytes and type, but not its filename. */
export function restoreAttachment(image: ChatImage, index: number): Attachment {
  return {
    ...image,
    id: genId(),
    name: `image-${index + 1}.${image.mimeType.split("/")[1] || "png"}`,
    bytes: decodedBytes(image.data),
  };
}

/** How many bytes a base64 string stands for, counted from its length rather than by decoding it. */
function decodedBytes(data: string): number {
  const padding = data.endsWith("==") ? 2 : data.endsWith("=") ? 1 : 0;
  return Math.floor(data.length * 3 / 4) - padding;
}

/**
 * The size an image is redrawn at, or `null` when it already fits within `MAX_IMAGE_EDGE`.
 *
 * Separate from the drawing so the arithmetic can be read and tested without a canvas.
 */
export function fitInside(width: number, height: number, edge = MAX_IMAGE_EDGE): { width: number; height: number } | null {
  const longest = Math.max(width, height);
  if (!Number.isFinite(longest) || longest <= edge) return null;
  const scale = edge / longest;
  return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)) };
}

/** Draw `bitmap` at `(width, height)`, optionally filling white underneath for opaque JPEG output. */
function drawBitmap(
  canvas: HTMLCanvasElement,
  bitmap: ImageBitmap,
  width: number,
  height: number,
  opaqueBackground: boolean,
): boolean {
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return false;
  if (opaqueBackground && typeof context.fillRect === "function") {
    context.fillStyle = "#ffffff";
    context.fillRect(0, 0, width, height);
  }
  context.drawImage(bitmap, 0, 0, width, height);
  return true;
}

function canvasToBlob(canvas: HTMLCanvasElement, mimeType: string, quality?: number): Promise<Blob | null> {
  return new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, mimeType, quality));
}

/** Try descending JPEG quality levels on the current canvas until the output fits within `maxBytes`. */
async function fitJpeg(
  canvas: HTMLCanvasElement,
  qualities: readonly number[],
  maxBytes: number,
): Promise<ChatImage | null> {
  for (const quality of qualities) {
    const blob = await canvasToBlob(canvas, "image/jpeg", quality);
    if (blob && blob.size <= maxBytes) {
      return { mimeType: "image/jpeg", data: await readBase64(blob) };
    }
  }
  return null;
}

/**
 * Resize and compress an oversized image to fit inside `MAX_IMAGE_EDGE` and `maxBytes`, or return
 * `null` when the original already fits or the browser cannot decode/draw it (SVG, damaged file).
 */
async function shrink(file: File, maxBytes: number): Promise<ChatImage | null> {
  if (typeof createImageBitmap !== "function") return null;
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    return null;
  }
  try {
    const initial = fitInside(bitmap.width, bitmap.height) ?? { width: bitmap.width, height: bitmap.height };
    const resized = initial.width !== bitmap.width || initial.height !== bitmap.height;
    if (!resized && file.size <= maxBytes) return null;

    const canvas = document.createElement("canvas");
    const isJpeg = file.type === "image/jpeg";
    if (!drawBitmap(canvas, bitmap, initial.width, initial.height, false)) return null;

    if (!isJpeg) {
      // Keep PNG for non-JPEG inputs when it fits within `maxBytes` so screenshots stay crisp.
      const png = await canvasToBlob(canvas, "image/png");
      if (png && png.size <= maxBytes) {
        return { mimeType: "image/png", data: await readBase64(png) };
      }
      // Transparent pixels turn black when encoded as JPEG unless painted over a solid background first.
      if (!drawBitmap(canvas, bitmap, initial.width, initial.height, true)) return null;
    }

    const jpeg = await fitJpeg(canvas, JPEG_INITIAL_QUALITIES, maxBytes);
    if (jpeg) return jpeg;

    const longest = Math.max(initial.width, initial.height);
    for (const edge of JPEG_SMALL_EDGES) {
      if (edge >= longest) continue;
      const smaller = fitInside(bitmap.width, bitmap.height, edge);
      if (!smaller || !drawBitmap(canvas, bitmap, smaller.width, smaller.height, !isJpeg)) continue;
      const candidate = await fitJpeg(canvas, JPEG_SMALL_QUALITIES, maxBytes);
      if (candidate) return candidate;
    }
    return null;
  } catch {
    return null;
  } finally {
    bitmap.close();
  }
}

export interface AttachOptions {
  /** Target maximum byte size per attachment before base64 encoding; capped at `MAX_IMAGE_BYTES`. */
  maxBytes?: number;
}

/**
 * Read files into attachments, resizing and compressing images to fit the size limit and refusing
 * anything that still does not fit.
 *
 * A file that is too large, or one file too many, is left out while the rest are taken: dropping a folder
 * of screenshots should attach what it can and say what it could not, rather than fail as a whole.
 * Retained history references keep their object identity so their snapshot context remains available.
 */
export async function attachImages<T extends ChatImageValue = Attachment>(
  current: T[],
  files: File[],
  options?: AttachOptions,
): Promise<AttachResult<T>> {
  const maxBytes = Math.max(1, Math.min(options?.maxBytes ?? DEFAULT_IMAGE_BYTES, MAX_IMAGE_BYTES));
  const attachments: (T | Attachment)[] = current.slice();
  const rejected: Rejection[] = [];
  for (const file of files) {
    if (attachments.length >= MAX_IMAGES) {
      rejected.push({ reason: "tooMany" });
      continue;
    }
    if (file.size > MAX_RAW_IMAGE_BYTES) {
      rejected.push({ reason: "tooLarge", name: file.name });
      continue;
    }
    try {
      const shrunk = await shrink(file, maxBytes);
      if (!shrunk && file.size > maxBytes) {
        rejected.push({ reason: "tooLarge", name: file.name });
        continue;
      }
      const image = shrunk ?? { mimeType: file.type || "image/png", data: await readBase64(file) };
      const bytes = decodedBytes(image.data);
      if (bytes > maxBytes) {
        rejected.push({ reason: "tooLarge", name: file.name });
        continue;
      }
      attachments.push({ id: genId(), name: file.name, ...image, bytes });
    } catch {
      rejected.push({ reason: "unreadable", name: file.name });
    }
  }
  return { attachments, rejected };
}

/** Format the first attachment rejection for display beside the composer. */
export function attachmentRejectionNote(
  rejected: Rejection[],
  maxMb: number,
  translate: typeof import("../../../i18n").t,
): string | null {
  const first = rejected[0];
  if (!first) return null;
  if (first.reason === "tooMany") return translate("chat.attach.tooMany", MAX_IMAGES);
  if (first.reason === "tooLarge") return translate("chat.attach.tooLarge", first.name, maxMb);
  return translate("chat.attach.unreadable", first.name);
}

/**
 * The bytes as base64, without the `data:` prefix the reader puts in front of them.
 *
 * `FileReader` rather than `arrayBuffer()` and a hand-rolled encoder: the browser already does this, and
 * doing it by hand means walking a multi-megabyte array in JavaScript for no gain.
 */
function readBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("Failed to read the image"));
    reader.onload = () => {
      const result = typeof reader.result === "string" ? reader.result : "";
      const comma = result.indexOf(",");
      if (comma < 0) {
        reject(new Error("Failed to read the image"));
        return;
      }
      resolve(result.slice(comma + 1));
    };
    reader.readAsDataURL(file);
  });
}
