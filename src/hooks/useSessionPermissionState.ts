import { useEffect, useState } from "react";
import { invoke, listen, onTransportDisconnect, onTransportReconnect, type UnlistenFn } from "../ipc/transport";

export interface SessionPermissionState {
  configured: string;
  current: string | null;
  launch: string | null;
  pending: string | null;
  activation: "applied" | "nextTurn" | "restart" | "nextStart" | "unconfirmed";
  running: boolean;
}

/** Every displayed runtime fact comes from the backend; reconnects invalidate prior evidence. */
export function useSessionPermissionState(sessionId?: string, revision?: string) {
  const [state, setState] = useState<{ id?: string; value?: SessionPermissionState; error?: string }>();
  useEffect(() => {
    if (!sessionId) return;
    let active = true;
    let generation = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unlisteners: UnlistenFn[] = [];
    const refresh = () => {
      const request = ++generation;
      void invoke<SessionPermissionState>("session_permission_state", { sessionId }).then(value => {
        if (active && request === generation) {
          setState(value && typeof value.running === "boolean" ? { id: sessionId, value }
            : { id: sessionId, error: "Permission state is unavailable" });
        }
      }).catch(error => {
        if (active && request === generation) setState({ id: sessionId, error: String(error) });
      });
    };
    const schedule = () => {
      if (!active) return;
      clearTimeout(timer);
      timer = setTimeout(refresh, 40);
    };
    setState({ id: sessionId });
    refresh();
    const subscribe = (name: string, callback: (event: { type?: string; kind?: string }) => void) => {
      void listen(name, callback).then(stop => active ? unlisteners.push(stop) : stop())
        .catch(error => { if (active) setState({ id: sessionId, error: String(error) }); });
    };
    subscribe("tree://changed", schedule);
    subscribe(`pty://status/${sessionId}`, event => {
      if (["agent", "hook_ready", "agent_missing"].includes(event.kind ?? "")) schedule();
    });
    subscribe(`pty://exit/${sessionId}`, schedule);
    // `resync` is the barrier after a gap: the connection dropped this session's events, so any of the others
    // may have been among them.
    subscribe(`chat://event/${sessionId}`, event => {
      if (["settingsChanged", "process", "reset", "exited", "resync"].includes(event.type ?? "")) schedule();
    });
    unlisteners.push(onTransportReconnect(() => { setState({ id: sessionId }); refresh(); }));
    unlisteners.push(onTransportDisconnect(() => {
      ++generation;
      clearTimeout(timer);
      setState({ id: sessionId, error: "Disconnected" });
    }));
    return () => { active = false; clearTimeout(timer); unlisteners.forEach(stop => stop()); };
  }, [sessionId, revision]);
  return state?.id === sessionId ? state : undefined;
}
