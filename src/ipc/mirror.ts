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

/**
 * One remote client attached to this service, matching Rust `presence::ClientInfo`.
 *
 * Everything but `ip` is self-reported by the client during the encrypted handshake, and the plaintext
 * paths report nothing at all. It names who is attached; it decides nothing.
 */
export interface RemoteClient {
  /** Connection ID (`ws-N`), the same ID mirror snapshots carry as their publisher. */
  source: string;
  /** Device name the client reported, such as `macOS · Chrome`, or null on a plaintext connection. */
  name: string | null;
  /** Device ID from the paired-devices registry, or null on a plaintext connection. */
  deviceId: string | null;
  /** Address the server saw the connection come from. */
  ip: string;
  /** Unix seconds at which the connection was accepted. */
  since: number;
}

/** Snapshot plus the current on/off state, returned by `mirror_get`. */
export interface MirrorStatus extends MirrorSnapshot {
  enabled: boolean;
  /** Remote clients attached right now. Carried here because `clients://changed` only fires on the next
   *  connect or disconnect, which leaves a client that just aligned with nothing to show until then. */
  clients: number;
  /** Who those clients are, for the host badge. Rides along for the same reason as the count. */
  clientList?: RemoteClient[];
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
