/**
 * vlx-term ↔ OpenCode status bridge, injected automatically by vlx-term.
 *
 * Sends OpenCode session events to vlx-term's loopback hook service, providing authoritative working,
 * asking, and waiting states like Claude instead of relying on screen inference.
 *
 * Event mapping from @opencode-ai/sdk Event types:
 *   - session.status with status.type === "busy" -> working
 *   - session.idle                               -> waiting
 *   - permission.updated                         -> asking
 *   - permission.replied                         -> working
 *   - session.error                              -> waiting
 *
 * Enabled only in vlx-term-managed sessions with VLX_SESSION_ID, VLX_TOKEN, and VLX_SPAWN_URL.
 * Running OpenCode directly in a normal terminal performs no work or outbound requests.
 *
 * Injection: vlx-term sets this environment value when starting OpenCode:
 *   OPENCODE_CONFIG_CONTENT = {"plugin":["<absolute path to this file>"]}
 * OpenCode loads the plugin from its absolute local path without modifying ~/.config/opencode.
 * Hook address, token, and session ID arrive through process environment and are never persisted.
 */

export const VlxNotify = async () => {
  const base = process.env.VLX_SPAWN_URL; // Loopback hook service shared with spawn.
  const token = process.env.VLX_TOKEN; // One-time token for this process.
  const sid = process.env.VLX_SESSION_ID; // Session ID on the vlx-term side.

  // Stay disabled outside vlx-managed sessions.
  if (!base || !token || !sid) return {};

  let lastEvent = null; // Deduplicate consecutive identical states.
  let captured = false; // Capture OpenCode's own session ID only with the first event.
  let promptSent = false; // Send the first user message only once; the backend names the session from it.

  // Fire-and-forget POST; failures remain silent and never block OpenCode.
  const post = (event, ocSessionId, extra = {}) => {
    const url = `${base}/hook/${encodeURIComponent(sid)}?t=${encodeURIComponent(
      token,
    )}&e=${event}`;
    const hasBody = Boolean(ocSessionId) || Object.keys(extra).length > 0;
    const body = hasBody
      ? JSON.stringify({ ...(ocSessionId ? { session_id: ocSessionId } : {}), ...extra })
      : undefined;
    try {
      fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body,
      }).catch(() => {});
    } catch {
      /* Ignore edge cases such as unavailable fetch. */
    }
  };

  // Send only on state changes or the first OpenCode session-ID capture.
  const signal = (event, ocSessionId) => {
    const needCapture = Boolean(ocSessionId) && !captured;
    if (event === lastEvent && !needCapture) return;
    lastEvent = event;
    if (ocSessionId) captured = true;
    post(event, needCapture ? ocSessionId : undefined);
  };

  return {
    // The first user message names the vlx-term session. The backend parses
    // UserPromptSubmit-shaped bodies and renames auto-numbered placeholders.
    "chat.message": async ({ sessionID }, { message, parts }) => {
      if (promptSent || message?.role !== "user") return;
      const text = (parts ?? [])
        .filter((p) => p?.type === "text")
        .map((p) => p.text?.trim() ?? "")
        .filter(Boolean)
        .join("\n")
        .trim();
      if (!text) return;
      promptSent = true;
      if (sessionID) captured = true;
      // Record the state here as well: the session.status busy event that follows this message would
      // otherwise fail the dedup check and post a second, identical working signal.
      lastEvent = "working";
      post("working", sessionID, {
        hook_event_name: "UserPromptSubmit",
        prompt: text,
      });
    },
    event: async ({ event }) => {
      const type = event?.type;
      const props = event?.properties ?? {};
      switch (type) {
        case "session.status": {
          // Treat only busy as processing. session.idle handles idle consistently, avoiding duplicates
          // and false idle reports during startup.
          if (props.status?.type === "busy") signal("working", props.sessionID);
          break;
        }
        case "session.idle":
          signal("waiting", props.sessionID);
          break;
        case "permission.updated":
          signal("asking", props.sessionID);
          break;
        case "permission.replied":
          signal("working", props.sessionID);
          break;
        case "session.error":
          signal("waiting", props.sessionID);
          break;
        default:
          break;
      }
    },
  };
};
