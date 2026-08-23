//! Icon rendering and image intake for agent presets.
//!
//! A preset's icon travels as a base64 data URL rather than a file path, because browser and remote clients
//! cannot read a file on the host. Keeping it inline also means it reaches every client through the ordinary
//! preset broadcast with no extra transport.

import type { AgentPreset, Session } from "../types";
import { kindIconEl } from "./sessionViewers/sessionMeta";

/** Square edge, in pixels, that an uploaded image is reduced to before encoding. */
const ICON_EDGE = 64;

/** Upper bound on the encoded icon, mirroring the backend's limit so the failure is reported in the dialog
 *  rather than as a command error. */
const ICON_MAX_BYTES = 64 * 1024;

/** Render a preset's icon, falling back to the built-in icon of the agent it behaves like. */
export function PresetIcon({ preset, size = 14 }: { preset: AgentPreset; size?: number }) {
  if (!preset.icon) return kindIconEl(preset.baseKind, size);
  return (
    <img
      src={preset.icon}
      alt=""
      style={{ width: size, height: size, borderRadius: 3, flex: "none", objectFit: "cover" }}
    />
  );
}

/** Render the icon for a session: its preset's when it came from one, otherwise the kind's. */
export function sessionIconEl(
  session: Pick<Session, "kind" | "agentPresetId">,
  presets: AgentPreset[],
  size = 14,
) {
  const preset = session.agentPresetId
    ? presets.find((p) => p.id === session.agentPresetId)
    : undefined;
  if (!preset) return kindIconEl(session.kind, size);
  return <PresetIcon preset={preset} size={size} />;
}

/**
 * Load an image file, reduce it to a square icon and return it as a PNG data URL.
 *
 * Downscaling happens here rather than on the backend so an oversized photograph never travels or gets
 * stored: a 64x64 PNG stays comfortably inside the size limit. The image is drawn centred on its shorter
 * edge, so a non-square source is cropped rather than distorted.
 */
export async function fileToIconDataUrl(file: File): Promise<string> {
  if (!file.type.startsWith("image/")) throw new Error("Choose an image file");
  const src = await readAsDataUrl(file);
  const img = await loadImage(src);
  const canvas = document.createElement("canvas");
  canvas.width = ICON_EDGE;
  canvas.height = ICON_EDGE;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Cannot process the image in this browser");
  const edge = Math.min(img.width, img.height);
  if (edge <= 0) throw new Error("The image is empty");
  ctx.drawImage(
    img,
    (img.width - edge) / 2,
    (img.height - edge) / 2,
    edge,
    edge,
    0,
    0,
    ICON_EDGE,
    ICON_EDGE,
  );
  const out = canvas.toDataURL("image/png");
  if (out.length > ICON_MAX_BYTES) throw new Error("The image is too large");
  return out;
}

function readAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("Could not read the image"));
    reader.readAsDataURL(file);
  });
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("Could not decode the image"));
    img.src = src;
  });
}
