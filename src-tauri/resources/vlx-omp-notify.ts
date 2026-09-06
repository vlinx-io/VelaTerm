/**
 * vlx-term ↔ OMP (oh-my-pi) status bridge extension, injected automatically by vlx-term.
 *
 * Sends OMP coding-agent lifecycle events to vlx-term's loopback hook service, providing authoritative
 * working, asking and waiting states like Claude instead of relying on screen inference.
 *
 * Lifecycle mapping:
 *   - input -> working; the first input includes its prompt so the server can derive a meaningful title.
 *   - agent_start -> working as a fallback; deduplication absorbs overlap with input.
 *   - tool_approval_requested -> asking; OMP prompts before a tool runs unless approval is bypassed.
 *   - tool_approval_resolved -> working, resuming the turn once the user answers.
 *   - agent_end -> waiting, including error termination.
 *   - session_start -> boot, carrying OMP's session ID as a resume anchor without changing work state.
 *
 * OMP is a fork of Pi but its extension contract differs: a module registers handlers through
 * `pi.on(...)` rather than returning a handler map, so `resources/vlx-pi-notify.ts` cannot be reused.
 * Verified against omp v18.1.10; the returned-map shape loads without error and never fires.
 *
 * Enabled only with VLX_SESSION_ID, VLX_TOKEN, and VLX_SPAWN_URL in a vlx-managed session. Running OMP
 * directly in a normal terminal performs no work or outbound requests.
 *
 * vlx-term passes `-e <absolute path>` when launching OMP, which loads the TypeScript for that launch
 * only, without touching ~/.omp or a global extension directory. Hook credentials arrive through the
 * process environment and are never persisted.
 */

export default (pi: any) => {
  const base = process.env.VLX_SPAWN_URL; // Loopback hook service shared with spawn.
  const token = process.env.VLX_TOKEN; // One-time token for this process.
  const sid = process.env.VLX_SESSION_ID; // Session ID on the vlx-term side.

  // Stay disabled outside vlx-managed sessions.
  if (!base || !token || !sid) return;

  let lastEvent: string | null = null; // Deduplicate consecutive identical states.
  let lastOmpId: string | null = null; // Resend OMP's session ID only when it changes.
  let promptSent = false; // Include prompt text only with the first input.

  // OMP's own session ID, which is exactly the `--resume` anchor. Fall back to the trailing UUID of the
  // session path `<timestamp>_<UUID>.jsonl` when the manager cannot answer.
  const currentOmpId = (ctx: any): string | null => {
    try {
      const id: string = ctx?.sessionManager?.getSessionId?.();
      if (id) return id;
      const file: string = ctx?.sessionManager?.getSessionFile?.();
      if (!file) return null;
      const name = file.split(/[\\/]/).pop() || "";
      const stem = name.replace(/\.jsonl$/i, "");
      const uuid = stem.slice(stem.lastIndexOf("_") + 1);
      return uuid || null;
    } catch {
      return null;
    }
  };

  // Fire-and-forget POST; failures remain silent and never block OMP.
  const post = (event: string, body?: string) => {
    const url = `${base}/hook/${encodeURIComponent(sid)}?t=${encodeURIComponent(
      token,
    )}&e=${event}`;
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

  // Send only on state changes or new/changed OMP session IDs. extra may carry the first prompt for renaming.
  const signal = (ctx: any, event: string, extra?: Record<string, unknown>) => {
    const ompId = currentOmpId(ctx);
    const idChanged = Boolean(ompId) && ompId !== lastOmpId;
    if (event === lastEvent && !idChanged && !extra) return;
    lastEvent = event;
    if (ompId) lastOmpId = ompId;
    // Match the Claude hook body: session_id is the resume anchor; event name/prompt support renaming.
    const payload: Record<string, unknown> = {};
    if (ompId) payload.session_id = ompId;
    if (extra) Object.assign(payload, extra);
    post(event, Object.keys(payload).length ? JSON.stringify(payload) : undefined);
  };

  // A throwing handler is reported through the extension error channel rather than killing the session,
  // but keep every handler total anyway so a status report can never disturb the agent.
  const on = (event: string, handler: (e: any, ctx: any) => void) => {
    try {
      pi.on(event, async (e: any, ctx: any) => {
        try {
          handler(e, ctx);
        } catch {
          /* Status reporting must never surface as an agent error. */
        }
      });
    } catch {
      /* An unknown event name on an older build simply goes unreported. */
    }
  };

  // On session start/resume/fork, capture only the resume anchor without changing work state.
  on("session_start", (_e, ctx) => signal(ctx, "boot"));

  // User input sets working and includes the first prompt for automatic renaming.
  on("input", (e, ctx) => {
    const text: string =
      typeof e === "string" ? e : (e?.text ?? e?.prompt ?? "");
    const extra =
      !promptSent && text
        ? { hook_event_name: "UserPromptSubmit", prompt: text }
        : undefined;
    if (extra) promptSent = true;
    signal(ctx, "working", extra);
  });

  // Agent start reinforces working; deduplication absorbs overlap with input.
  on("agent_start", (_e, ctx) => signal(ctx, "working"));

  // A tool waiting on the approval prompt is the asking state; answering it returns to working.
  on("tool_approval_requested", (_e, ctx) => signal(ctx, "asking"));
  on("tool_approval_resolved", (_e, ctx) => signal(ctx, "working"));

  // Agent end, including errors, sets waiting.
  on("agent_end", (_e, ctx) => signal(ctx, "waiting"));
};
