//! Pick an earlier Claude conversation of the session's directory and continue it here, or branch from it.
//!
//! The picker is only a view of the backend's listing: which conversations exist, which one this session
//! holds and which ones another session owns all come from the server, and the server checks the choice
//! again. Once a choice is taken, the history arrives through the session's reset event like any restart,
//! so every open view of the session follows without doing anything here.

import { useEffect, useMemo, useRef, useState } from "react";
import Icons from "../../../components/Icons";
import { Backdrop } from "../../../components/Backdrop";
import { dateLocale, useT, type I18nKey } from "../../../i18n";
import { chatResume, chatResumeList, type ResumableConversation, type ResumeListing, type ResumeMode } from "../../../ipc/chat";
import { fmtBytes } from "../../../format";
import { useTermStore } from "../../../store/termStore";
import "./resume-picker.css";

/**
 * The picker's wording for an error. Stable backend refusals get their own; anything else (an unreadable
 * folder, a failed start) gets a general one for loading or for acting, so no backend text reaches the view.
 */
export function resumeErrorKey(error: unknown, action: "list" | "act"): I18nKey {
  const text = String(error);
  if (text.includes("chat_resume_busy")) return "chat.resume.error.busy";
  if (text.includes("chat_resume_in_use")) return "chat.resume.error.inUse";
  if (text.includes("chat_resume_not_found")) return "chat.resume.error.notFound";
  if (text.includes("chat_resume_unsupported")) return "chat.resume.error.unsupported";
  return action === "list" ? "chat.resume.error.list" : "chat.resume.error.failed";
}

/** How long typing pauses before the search goes to the server. */
const SEARCH_DELAY_MS = 250;

export function ResumePicker({
  sessionId,
  initialQuery = "",
  onClose,
}: {
  sessionId: string;
  initialQuery?: string;
  onClose: () => void;
}) {
  const t = useT();
  const dialogRef = useRef<HTMLElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [previousFocus] = useState(() => document.activeElement);
  useEffect(() => () => {
    if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus();
  }, [previousFocus]);
  const [listing, setListing] = useState<ResumeListing | null>(null);
  // Which request the listing answers: the search and the refresh it was loaded for. A listing is kept
  // together with this, so a newer search or refresh supersedes it in the same render that starts it.
  const [listedFor, setListedFor] = useState<{ search: string; revision: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState(initialQuery);
  // The search the server last ran; typing reaches it after a short pause.
  const [search, setSearch] = useState(initialQuery.trim());
  const [active, setActive] = useState(0);
  const [revision, setRevision] = useState(0);
  // Only the newest request may fill the list, so a slow answer to an older search never replaces it.
  const request = useRef(0);

  useEffect(() => {
    const next = query.trim();
    if (next === search) return;
    const timer = window.setTimeout(() => setSearch(next), SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [query, search]);

  useEffect(() => {
    const id = ++request.current;
    setLoading(true); setLoadingMore(false); setError("");
    chatResumeList(sessionId, search).then((result) => {
      if (id === request.current) { setListing(result); setListedFor({ search, revision }); setLoading(false); }
    }).catch((e) => {
      if (id === request.current) { setListing(null); setListedFor(null); setError(describe(e, "list")); setLoading(false); }
    });
  }, [sessionId, revision, search]);
  useEffect(() => () => { request.current += 1; }, []);

  const describe = (e: unknown, action: "list" | "act") => {
    console.warn("resume picker:", e);
    return t(resumeErrorKey(e, action));
  };
  // The listing answers the search in the box and the latest refresh, and nothing newer is on its way.
  // Until then it is neither shown nor acted on: the rows, Enter, and every action take their conversation
  // from `shown` only. The check compares what the listing was loaded for, not a loading flag, so the render
  // between a new search and the start of its request cannot show or act on the previous result.
  const settled = !loading && listedFor !== null && listedFor.search === search
    && listedFor.revision === revision && query.trim() === search;
  const shown = settled ? listing : null;
  const visible = useMemo(() => shown?.conversations ?? [], [shown]);
  useEffect(() => setActive(0), [search, revision]);
  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  const close = () => { if (!busy) onClose(); };
  const refresh = () => {
    setListing(null); setRevision((value) => value + 1);
  };
  // The next page of the same search, after the conversations already shown.
  const showOlder = () => {
    if (!shown || loadingMore) return;
    const id = ++request.current;
    const before = shown;
    setLoadingMore(true); setError("");
    chatResumeList(sessionId, search, before.nextOffset).then((page) => {
      if (id !== request.current) return;
      const known = new Set(before.conversations.map((conversation) => conversation.id));
      setListing({
        ...page,
        conversations: [...before.conversations, ...page.conversations.filter((conversation) => !known.has(conversation.id))],
      });
      setLoadingMore(false);
    }).catch((e) => {
      if (id === request.current) { setError(describe(e, "list")); setLoadingMore(false); }
    });
  };
  const take = async (conversation: ResumableConversation, mode: ResumeMode) => {
    if (busy || !settled) return;
    setBusy(true); setError("");
    try {
      await chatResume(sessionId, conversation.id, mode);
      onClose();
    } catch (e) {
      setError(describe(e, "act"));
      setBusy(false);
    }
  };
  const openOwner = async (conversation: ResumableConversation) => {
    const owner = conversation.owner;
    if (!owner || busy || !settled) return;
    setBusy(true); setError("");
    try {
      const store = useTermStore.getState();
      if (owner.archived) await store.restoreSession(owner.sessionId);
      store.openSession(owner.sessionId);
      onClose();
    } catch (e) {
      setError(describe(e, "act"));
      setBusy(false);
    }
  };
  // What Enter does on a row: continue it here, or go where it already continues.
  const primary = (conversation: ResumableConversation) => {
    if (conversation.current) onClose();
    else if (conversation.owner) void openOwner(conversation);
    else void take(conversation, "resume");
  };

  return <Backdrop onClose={close}>
    <section ref={dialogRef} className="resume-picker" role="dialog" aria-modal="true"
      aria-labelledby="resume-picker-title" aria-describedby="resume-picker-description" tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "Escape") { e.stopPropagation(); close(); return; }
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          if (!visible.length) return;
          e.preventDefault();
          setActive((index) => (index + (e.key === "ArrowDown" ? 1 : visible.length - 1)) % visible.length);
          return;
        }
        if (e.key === "Enter" && e.target instanceof HTMLInputElement) {
          const conversation = visible[active];
          if (conversation) { e.preventDefault(); primary(conversation); }
          return;
        }
        if (e.key !== "Tab") return;
        const controls = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>(
          ':is(button, input, [href], [tabindex="0"]):not(:disabled)',
        ) || []);
        const first = controls[0], last = controls[controls.length - 1];
        if (!first) { e.preventDefault(); dialogRef.current?.focus(); }
        else if (e.shiftKey && (document.activeElement === first || document.activeElement === dialogRef.current)) {
          e.preventDefault(); last.focus();
        } else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
      }}>
      <header className="resume-picker-header">
        <div>
          <h2 id="resume-picker-title">{t("chat.resume.title")}</h2>
          <p id="resume-picker-description">{t("chat.resume.description")} {t("chat.resume.forkHint")}</p>
        </div>
        <button type="button" className="vlx-btn resume-picker-icon" aria-label={t("common.close")}
          title={t("common.close")} onClick={close} disabled={busy}><Icons.close size={18} /></button>
      </header>
      <div className="resume-picker-toolbar">
        {listing && <div className="resume-picker-directory" title={listing.directory}>
          <Icons.folder size={15} /><span>{listing.directory}</span>
        </div>}
        <div className="resume-picker-search-row">
          <div className="resume-picker-search">
            <Icons.search size={16} />
            <input className="vlx-input" autoFocus aria-label={t("chat.resume.search")}
              placeholder={t("chat.resume.search")} value={query} disabled={busy}
              onChange={(e) => setQuery(e.target.value)} />
          </div>
          <button type="button" className="vlx-btn" onClick={refresh} disabled={busy || loading || loadingMore}>
            <Icons.restart size={14} />{t("common.refresh")}
          </button>
        </div>
      </div>
      <div className="resume-picker-content" aria-busy={loading || busy}>
        {error && <div className="resume-picker-message resume-picker-error" role="alert">{error}</div>}
        {shown?.truncated && <div className="resume-picker-message resume-picker-more" role="status">
          <span>{shown.conversations.length
            ? t("chat.resume.truncated", { count: shown.conversations.length })
            : t("chat.resume.notSearchedYet")}</span>
          <button type="button" className="vlx-btn" onClick={showOlder} disabled={busy || loadingMore}>
            {t(loadingMore ? "common.loading" : "chat.resume.showOlder")}
          </button>
        </div>}
        {!settled ? !error && <div className="resume-picker-empty" role="status">{t("common.loading")}</div> : shown &&
          (!visible.length ? <div className="resume-picker-empty">
            <Icons.search size={28} />
            <p>{t(search ? "chat.resume.noMatches" : "chat.resume.empty")}</p>
            {query && <button type="button" className="vlx-btn" onClick={() => setQuery("")}>{t("chat.resume.clearSearch")}</button>}
          </div> : <div className="resume-picker-list" role="listbox" aria-label={t("chat.resume.title")} ref={listRef}>
            {visible.map((conversation, index) => <div key={conversation.id} data-index={index} role="option"
              aria-selected={index === active}
              className={`resume-picker-row${index === active ? " is-active" : ""}${conversation.current ? " is-current" : ""}`}
              onMouseEnter={() => setActive(index)}>
              <div className="resume-picker-conversation">
                <span className="resume-picker-title" title={conversation.title}>{conversation.title}</span>
                {conversation.firstPrompt && conversation.firstPrompt !== conversation.title &&
                  <span className="resume-picker-prompt" title={conversation.firstPrompt}>{conversation.firstPrompt}</span>}
                <span className="resume-picker-meta">
                  {conversation.updatedAt > 0 && <time dateTime={new Date(conversation.updatedAt).toISOString()}>
                    {new Date(conversation.updatedAt).toLocaleString(dateLocale())}
                  </time>}
                  <span>{fmtBytes(conversation.sizeBytes)}</span>
                  {conversation.gitBranch && <span className="resume-picker-branch"><Icons.branch size={11} />{conversation.gitBranch}</span>}
                  {conversation.current && <span className="resume-picker-badge">{t("chat.resume.current")}</span>}
                  {conversation.owner && <span className="resume-picker-badge" title={conversation.owner.name}>
                    {t("chat.resume.inSession", { name: conversation.owner.name })}
                  </span>}
                  {conversation.owner?.archived && <span className="resume-picker-badge">{t("chat.resume.archived")}</span>}
                </span>
              </div>
              <div className="resume-picker-actions">
                {conversation.owner
                  ? <button type="button" className="vlx-btn" disabled={busy} onClick={() => void openOwner(conversation)}>
                    {t("chat.resume.openSession")}
                  </button>
                  : !conversation.current && <button type="button" className="vlx-btn vlx-btn-primary" disabled={busy}
                    onClick={() => void take(conversation, "resume")}>{t("chat.resume.resume")}</button>}
                <button type="button" className="vlx-btn" disabled={busy} onClick={() => void take(conversation, "fork")}>
                  {t("chat.resume.fork")}
                </button>
              </div>
            </div>)}
          </div>)}
      </div>
    </section>
  </Backdrop>;
}
