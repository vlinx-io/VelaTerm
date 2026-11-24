import { useSyncExternalStore } from "react";
import {
  emitNative,
  isRemoteWindow,
  listenNative,
  remoteConnectionKind,
  remoteSshSession,
} from "../../ipc/transport";

export type ForwardSource = "manual" | "detected";

export interface PortForward {
  rport: number;
  lport: number;
  source: ForwardSource;
}

export interface PortsSnapshot {
  session: string;
  detected: number[];
  forwards: PortForward[];
  detectionAvailable: boolean;
  error?: string;
}

export type PortsAction = "list" | "forward" | "unforward" | "open";

export interface PortsState {
  snapshot: PortsSnapshot | null;
  unavailable: boolean;
  error: string | null;
}

export const portsSupported = isRemoteWindow && !!remoteSshSession && remoteConnectionKind === "ssh";

export const UNAVAILABLE_AFTER_MS = 3000;

let state: PortsState = { snapshot: null, unavailable: false, error: null };
const subscribers = new Set<() => void>();
let listening: Promise<void> | null = null;
let unavailableTimer: ReturnType<typeof setTimeout> | null = null;

function setState(next: PortsState) {
  state = next;
  subscribers.forEach((notify) => notify());
}

function ensureListening(): Promise<void> {
  listening ??= listenNative<PortsSnapshot>("ssh://ports-state", (payload) => {
    if (payload.session !== remoteSshSession) return;
    if (unavailableTimer) {
      clearTimeout(unavailableTimer);
      unavailableTimer = null;
    }
    // A poll snapshot has no error; keep the last one until the next user action clears it.
    setState({ snapshot: payload, unavailable: false, error: payload.error ?? state.error });
  }).then(
    () => undefined,
    () => undefined,
  );
  return listening;
}

export async function requestPorts(action: PortsAction, rport?: number): Promise<void> {
  if (!portsSupported) return;
  await ensureListening();
  if (action !== "list" && state.error !== null) setState({ ...state, error: null });
  // No snapshot after the first request means the local app predates port forwarding.
  if (!state.snapshot && !state.unavailable && !unavailableTimer) {
    unavailableTimer = setTimeout(() => {
      unavailableTimer = null;
      if (!state.snapshot) setState({ ...state, unavailable: true });
    }, UNAVAILABLE_AFTER_MS);
  }
  try {
    await emitNative("vlx://ports-request", {
      session: remoteSshSession,
      action,
      ...(rport === undefined ? {} : { rport }),
    });
  } catch {
    // The unavailable timer reports a missing local listener.
  }
}

function subscribe(notify: () => void) {
  subscribers.add(notify);
  return () => {
    subscribers.delete(notify);
  };
}

export function usePorts(): PortsState {
  return useSyncExternalStore(subscribe, () => state);
}
