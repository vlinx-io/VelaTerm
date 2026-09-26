import { useEffect, useRef, useState } from "react";
import Icons from "../components/Icons";
import { useT } from "../i18n";
import { ptyWrite, readAgentChat, type ChatEvent } from "../ipc/commands";
import { MessageBubble, ReasoningRow, ShellRow, ToolCard } from "../layout/CenterPane/session/rows";
import { assistantLabel } from "../layout/sessionViewers/TranscriptViewer";
import type { Session } from "../types";
import "../layout/CenterPane/session/session-view.css";

/** Reads the running terminal's recording without acquiring its PTY or starting a chat engine. */
export function MobileRecordedConversation({ session, cwd }: { session: Session; cwd?: string }) {
  const t = useT();
  const readOnly = session.kind === "kiro";
  const [rows, setRows] = useState<ChatEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const scroll = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const sendingRef = useRef(false);
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const refresh = async () => {
      try {
        const next = await readAgentChat(session.id);
        if (!disposed) { setRows(next); setError(null); }
      } catch (e) { if (!disposed) setError(String(e)); }
      finally {
        if (!disposed) { setLoading(false); timer = setTimeout(refresh, 1500); }
      }
    };
    void refresh();
    return () => { disposed = true; clearTimeout(timer); };
  }, [session.id, retry]);
  useEffect(() => {
    if (follow.current && scroll.current) scroll.current.scrollTop = scroll.current.scrollHeight;
  }, [rows]);
  const send = async () => {
    if (readOnly || !draft.trim() || sendingRef.current) return;
    const text = draft;
    sendingRef.current = true; setSending(true); setSendError(null);
    try {
      // Bracketed paste keeps multiline messages as text; Enter submits to the existing terminal.
      await ptyWrite(session.id, `\x1b[200~${text.replace(/[\x00-\x08\x0b-\x1f\x7f]/g, "")}\x1b[201~`);
      await ptyWrite(session.id, "\r");
      setDraft(current => current === text ? "" : current);
    } catch (e) { setSendError(String(e)); }
    finally { sendingRef.current = false; setSending(false); }
  };
  return <div className="m-recorded sv">
    <div className="m-recorded-scroll" ref={scroll} onScroll={() => {
      const el = scroll.current;
      if (el) follow.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
    }}>
      {loading && <p role="status">{t("chat.sync.loading")}</p>}
      {error && <div className="m-load-error" role="alert"><p>{t("chat.sync.failed")}</p><p>{error}</p><button type="button" onClick={() => {setLoading(true); setError(null); setRetry(x => x + 1);}}>{t("common.retry")}</button></div>}
      {!loading && !error && rows.length === 0 && <p>{t(readOnly ? "archive.emptyTranscript" : "chat.empty")}</p>}
      {rows.map(row => row.kind === "tool" ? <ToolCard key={row.index} name={row.tool} input={row.input} output={row.output} isError={row.isError} running={row.pending} cwd={cwd} />
        : row.kind === "thinking" ? <ReasoningRow key={row.index} text={row.text ?? ""} />
        : row.kind === "user" && row.shell ? <ShellRow key={row.index} row={row.shell} />
        : <MessageBubble key={row.index} who={row.kind === "user" ? t("archive.you") : assistantLabel(session.kind)} isUser={row.kind === "user"} text={row.text ?? ""} at={row.timestamp ?? undefined} />)}
    </div>
    {!readOnly && <form className="m-recorded-composer" onSubmit={e => { e.preventDefault(); void send(); }}>
      {sendError && <p role="alert">{t("chat.submission.failed")}: {sendError}</p>}
      <div className="m-recorded-input">
      <textarea value={draft} onChange={e => setDraft(e.target.value)} aria-label={t("session.send")} placeholder={t("chat.empty")} />
      {/* Keeps the keyboard up after sending, the same way the options toggle does. */}
      <button type="submit" className="sv-send" disabled={sending || !draft.trim() || loading || !!error}
        aria-label={sending ? t("chat.submission.sending") : t("session.send")}
        title={sending ? t("chat.submission.sending") : t("session.send")}
        onPointerDown={e => e.preventDefault()}>
        <Icons.arrowRight size={20} />
      </button>
      </div>
    </form>}
  </div>;
}
