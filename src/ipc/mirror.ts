//! Command wrappers for mirror mode: the shared UI layout every client publishes to and follows.

import { invoke } from "./transport";

/** Opaque-to-the-backend layout blob; its shape is owned by `src/store/mirrorSync.ts`. */
export type MirrorState = Record<string, unknown>;

/** A published layout with its revision and publisher, matching Rust `mirror::Snapshot::to_json`. */
export interface MirrorSnapshot {
  /** Monotonic revision; 0 means nothing has been published yet. */
  rev: number;
  /** Connection ID of the publisher: `desktop`, or `ws-N` for a WebSocket client. */
  source: string;
  /** Layout blob, or null before the first push. */
  state: MirrorState | null;
}

/** Snapshot plus the current on/off state, returned by `mirror_get`. */
export interface MirrorStatus extends MirrorSnapshot {
  enabled: boolean;
}

/** Read mirror mode and the published layout, used once per client to align right after it connects. */
export function mirrorGet(): Promise<MirrorStatus> {
  return invoke<MirrorStatus>("mirror_get");
}

/** Publish this client's layout. The backend stamps the publisher from the connection, not from arguments. */
export function mirrorPush(state: MirrorState): Promise<MirrorSnapshot> {
  return invoke<MirrorSnapshot>("mirror_push", { state });
}

/** Turn mirror mode on or off for every client. Host-side only; remote clients are refused by the dispatcher. */
export function mirrorSetEnabled(enabled: boolean): Promise<void> {
  return invoke<void>("mirror_set_enabled", { enabled });
}
