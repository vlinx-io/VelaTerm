//! Images waiting in the composer, from the moment they are pasted or dropped until they are sent.
//!
//! The chat engine takes an image as base64 inside the message itself, so nothing is written to disk on the
//! way — unlike the terminal path in `terminal/imageInput.ts`, which uploads a file and types its path,
//! because a terminal can only carry text. What the two share is how an image is fished out of a paste or a
//! drop, and that part is reused rather than written twice.

import { genId } from "../../../genId";
import type { ChatImage } from "../../../ipc/chat";

/**
 * How much can ride along with one message.
 *
 * The same numbers are enforced in `command_core::chat_send`. They are here as well so that a file too
 * large is refused before it is read, and refused in words the reader can act on rather than as a failure
 * coming back from the backend.
 */
export const MAX_IMAGES = 4;
export const MAX_IMAGE_BYTES = 5 * 1024 * 1024;

/** One image held in the composer, not yet sent. */
export interface Attachment extends ChatImage {
  /** Identifies the thumbnail while it is on screen; never leaves the frontend. */
  id: string;
  /** The file's own name, shown as the thumbnail's tooltip. */
  name: string;
  /** Size in bytes of the file itself, before base64. */
  bytes: number;
}

/** Why a file was left out, in the terms the composer explains it in. */
export type Rejection =
  | { reason: "tooMany" }
  | { reason: "tooLarge"; name: string }
  | { reason: "unreadable"; name: string };

export interface AttachResult {
  /** The attachments the composer should hold now: what it had, plus whatever was accepted. */
  attachments: Attachment[];
  /** Everything left out, in the order it was offered. Empty when all of it was taken. */
  rejected: Rejection[];
}

/** What an `<img>` needs to draw one of these. */
export function dataUrl(image: ChatImage): string {
  return `data:${image.mimeType};base64,${image.data}`;
}

/**
 * Read files into attachments, refusing what does not fit.
 *
 * A file that is too large, or one file too many, is left out while the rest are taken: dropping a folder
 * of screenshots should attach what it can and say what it could not, rather than fail as a whole.
 */
export async function attachImages(current: Attachment[], files: File[]): Promise<AttachResult> {
  const attachments = current.slice();
  const rejected: Rejection[] = [];
  for (const file of files) {
    if (attachments.length >= MAX_IMAGES) {
      rejected.push({ reason: "tooMany" });
      continue;
    }
    if (file.size > MAX_IMAGE_BYTES) {
      rejected.push({ reason: "tooLarge", name: file.name });
      continue;
    }
    try {
      attachments.push({
        id: genId(),
        name: file.name,
        mimeType: file.type || "image/png",
        data: await readBase64(file),
        bytes: file.size,
      });
    } catch {
      rejected.push({ reason: "unreadable", name: file.name });
    }
  }
  return { attachments, rejected };
}

/**
 * The file's bytes as base64, without the `data:` prefix the reader puts in front of them.
 *
 * `FileReader` rather than `arrayBuffer()` and a hand-rolled encoder: the browser already does this, and
 * doing it by hand means walking a multi-megabyte array in JavaScript for no gain.
 */
function readBase64(file: File): Promise<string> {
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
