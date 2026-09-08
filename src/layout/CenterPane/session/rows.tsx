//! The pieces a conversation is drawn from, shared by both readings of a session.
//!
//! A session recorded by a terminal-driven agent and a session driven live by the chat engine arrive
//! through different paths but describe the same things: someone said something, the agent answered, a tool
//! ran. These components are that shared vocabulary, so both views look identical and only one of them has
//! to be styled.

import { useEffect, useMemo, useRef, useState, type ClipboardEvent, type ReactNode } from "react";

import { velaSailMarkEl } from "../../../components/brandIcons";
import Icons from "../../../components/Icons";
import { fmtTokens } from "../../../format";
import { dateLocale, useT } from "../../../i18n";
import {
  resolveChatImage,
  loadChatTool,
  chatToolContextKey,
  type ChatRow,
  type ChatImage,
  type ChatImageValue,
  type ChatRewindScope,
  type SubagentInfo,
} from "../../../ipc/chat";
import { dataUrl } from "./attachments";
import { useImageMenu } from "./useImageMenu";
import { ChatImagePreview } from "./ChatImagePreview";
import { Markdown } from "./markdown";
import { parseAnsweredQuestions, type AnsweredQuestion } from "./questionForm";
import { ToolBody, toolSummary } from "./toolCards";
import { runSummaryText, type ToolRow } from "./toolRuns";
import { createMessageSelectionClipboardContent } from "./selectionCopy";

/**
 * What to show for the instant a message was written.
 *
 * The date is dropped for anything said today, which is nearly everything anyone is reading: a
 * conversation held this morning does not need every line stamped with the year. Older messages keep the
 * day, and the full date and time stay in the tooltip either way.
 */
function messageTime(at: string | number | undefined): { short: string; full: string } | null {
  if (at === undefined || at === null || at === "") return null;
  const d = new Date(at);
  if (Number.isNaN(d.getTime())) return null;
  const locale = dateLocale();
  const now = new Date();
  const today =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  const time = d.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
  const day = d.toLocaleDateString(locale, { month: "numeric", day: "numeric" });
  return { short: today ? time : `${day} ${time}`, full: d.toLocaleString(locale) };
}

/** Compact elapsed time used beside a completed answer and by the live working indicator. */
export function formatTurnDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(durationMs / 1_000));
  const seconds = totalSeconds % 60;
  const totalMinutes = Math.floor(totalSeconds / 60);
  const minutes = totalMinutes % 60;
  const hours = Math.floor(totalMinutes / 60);
  if (hours > 0) return `${hours}h ${minutes}m ${seconds}s`;
  if (totalMinutes > 0) return `${totalMinutes}m ${seconds}s`;
  return `${seconds}s`;
}

function LiveTurnDuration({ startedAt }: { startedAt: number }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, [startedAt]);
  return <span className="sv-working-time">{formatTurnDuration(now - startedAt)}</span>;
}

/** Copy message content from its Markdown semantics instead of the layout DOM's block separators. */
function handleMessageCopy(event: ClipboardEvent<HTMLDivElement>) {
  const content = createMessageSelectionClipboardContent(window.getSelection(), event.currentTarget);
  if (!content) return;
  event.preventDefault();
  event.clipboardData.setData("text/plain", content.plainText);
  event.clipboardData.setData("text/html", content.html);
}

/** Load snapshot-backed bytes only when the row or queue item containing this image is actually mounted. */
export function ChatImageView({
  image,
  className,
  alt,
}: {
  image: ChatImageValue;
  className: string;
  alt: string;
}) {
  const t = useT();
  const [preview, setPreview] = useState(false);
  const [resolved, setResolved] = useState<ChatImage | null>(() => ("data" in image ? image : null));
  useEffect(() => {
    setPreview(false);
    let disposed = false;
    if ("data" in image) {
      setResolved(image);
      return () => {
        disposed = true;
      };
    }
    setResolved(null);
    void resolveChatImage(image)
      .then((value) => {
        if (!disposed) setResolved(value);
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, [image]);
  const src = resolved ? dataUrl(resolved) : "";
  const { onContextMenu, imageMenu } = useImageMenu(src, () => setPreview(true));
  if (!resolved) return null;
  return (
    <>
      <img
        className={`${className} sv-image-trigger`}
        src={src}
        alt={alt}
        loading="lazy"
        role="button"
        tabIndex={0}
        aria-label={alt || t("crepe.image")}
        aria-haspopup="dialog"
        onClick={() => setPreview(true)}
        onContextMenu={onContextMenu}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            event.stopPropagation();
            setPreview(true);
          }
        }}
      />
      {imageMenu}
      {preview && <ChatImagePreview src={src} alt={alt} onClose={() => setPreview(false)} />}
    </>
  );
}

/**
 * A message from the person or from the agent, with whatever was attached to it.
 *
 * The two sides are drawn differently because they are different kinds of thing. The agent's answer is a
 * document: an author line — its mark, its name, when it wrote — and then prose down the page. What you
 * said is an instruction, usually one line, and it needs to be findable when you scroll back through a long
 * session. So it sits on a quiet prompt surface under the same author-and-time line used by the answer.
 *
 * Both start their text on the same left edge, so an answer and the question above it read as one exchange.
 */
export function MessageBubble({
  who,
  isUser,
  icon,
  at,
  text,
  images,
  durationMs,
  onRewind,
  rewindScopes,
  openRewindToken,
  onRewindMenuOpened,
}: {
  who: string;
  isUser: boolean;
  /** The agent's mark, shown in the margin beside its answers. */
  icon?: ReactNode;
  /** When it was written: epoch milliseconds from the live engine, an ISO string from a recording. */
  at?: string | number;
  text: string;
  images?: ChatImageValue[];
  /** Total wall-clock time for a completed assistant turn. */
  durationMs?: number;
  /** Present only for top-level user turns while the conversation is safe to rewind. */
  onRewind?: (scope: ChatRewindScope) => void;
  rewindScopes?: ChatRewindScope[];
  /** A new token asks this already-rendered message to open its normal rewind scope menu. */
  openRewindToken?: number;
  /** Acknowledge the one-shot request after the target message has opened its menu. */
  onRewindMenuOpened?: (token: number) => void;
}) {
  const t = useT();
  const [rewindMenu, setRewindMenu] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (openRewindToken === undefined) return;
    setRewindMenu(true);
    window.requestAnimationFrame(() => rootRef.current?.scrollIntoView({ block: "center" }));
    onRewindMenuOpened?.(openRewindToken);
  }, [openRewindToken, onRewindMenuOpened]);
  const when = messageTime(at);
  const time = when ? (
    <time className="sv-msg-time" title={when.full}>
      {when.short}
    </time>
  ) : null;
  const duration =
    !isUser && durationMs !== undefined && Number.isFinite(durationMs) ? (
      <span className="sv-msg-duration">· {formatTurnDuration(durationMs)}</span>
    ) : null;
  // Pictures come before the words, the way they sit above the input while being written.
  const pictures =
    images && images.length > 0 ? (
      <div className="sv-msg-images">
        {images.map((image, i) => (
          <ChatImageView
            key={"attachmentId" in image ? image.attachmentId : i}
            image={image}
            className="sv-msg-image"
            alt=""
          />
        ))}
      </div>
    ) : null;
  const body = text ? <Markdown text={text} /> : null;

  if (isUser) {
    const rewind =
      onRewind && rewindScopes && rewindScopes.length > 0 ? (
        <span className="sv-rewind">
          <button
            className="sv-rewind-button"
            title={t("chat.rewind.title")}
            aria-label={t("chat.rewind.title")}
            aria-expanded={rewindMenu}
            onClick={() => setRewindMenu((open) => !open)}
          >
            <Icons.restart size={12} />
          </button>
          {rewindMenu ? (
            <span className="sv-rewind-menu">
              <span className="sv-rewind-warning">{t("chat.rewind.warning")}</span>
              {rewindScopes.map((scope) => (
                <button
                  key={scope}
                  className="sv-rewind-choice"
                  onClick={() => {
                    setRewindMenu(false);
                    onRewind(scope);
                  }}
                >
                  {t(`chat.rewind.${scope}` as "chat.rewind.conversation")}
                </button>
              ))}
            </span>
          ) : null}
        </span>
      ) : null;
    return (
      <div className="sv-msg sv-msg-user" ref={rootRef}>
        <div className="sv-msg-turn">
          <div className="sv-msg-head">
            <span className="sv-msg-author">
              <span className="sv-msg-mark" aria-hidden>
                <span className="sv-user-sail">{velaSailMarkEl(14)}</span>
              </span>
              <span className="sv-msg-who">{who}</span>
            </span>
            {time}
            {rewind}
          </div>
          <div className="sv-msg-body" onCopy={handleMessageCopy}>
            {pictures}
            {body}
          </div>
        </div>
      </div>
    );
  }
  return (
    <div className="sv-msg sv-msg-assistant">
      <div className="sv-msg-head">
        <span className="sv-msg-author">
          <span className="sv-msg-mark" aria-hidden>
            {icon}
          </span>
          <span className="sv-msg-who">{who}</span>
        </span>
        {time}
        {duration}
      </div>
      {pictures}
      {body ? (
        <div className="sv-msg-body" onCopy={handleMessageCopy}>
          {body}
        </div>
      ) : null}
    </div>
  );
}

/** Reasoning, folded away by default: it explains how an answer was reached, which is rarely what a reader wants first. */
export function ReasoningRow({ text }: { text: string }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  return (
    <div className="sv-thinking">
      <button className="sv-thinking-head" onClick={() => setOpen((v) => !v)}>
        <Icons.chevR size={12} className={open ? "sv-caret sv-caret-open" : "sv-caret"} />
        {t("session.thinking")}
      </button>
      {open && (
        <div className="sv-thinking-body">
          <Markdown text={text} />
        </div>
      )}
    </div>
  );
}

/**
 * A tool call: name and one-line summary always visible, details on demand.
 *
 * `steps` is what a subagent did inside this call. It renders inside the card rather than in the
 * conversation, so a Task that runs twenty tools stays one line until someone opens it.
 */
export function ToolCard({
  name,
  input,
  output,
  isError,
  running,
  cwd,
  steps,
  stepCount,
  subagent,
  source,
  renderChildren,
}: {
  source?: Extract<ChatRow, { kind: "tool" }>;
  renderChildren?: (rows: ChatRow[]) => ReactNode;
  name?: string;
  input: unknown;
  output?: string;
  isError: boolean;
  running: boolean;
  cwd?: string;
  steps?: ReactNode;
  /** How many of them there are, shown on the collapsed card so the work is visible without opening it. */
  stepCount?: number;
  /** Provider-announced identity and progress, including background children with no visible steps. */
  subagent?: SubagentInfo;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [details, setDetails] = useState<Extract<ChatRow, { kind: "tool" }> | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const contextKey = source ? chatToolContextKey(source) : undefined;
  useEffect(() => {
    if (!open || !source?.detailAvailable) return;
    let disposed = false;
    setDetails(null);
    setLoadError(null);
    let inFlight = false;
    const load = async () => {
      if (inFlight) return;
      inFlight = true;
      try { const value = await loadChatTool(source); if (!disposed) { setDetails(value); setLoadError(null); } }
      catch (error) { if (!disposed) setLoadError(String(error)); }
      finally { inFlight = false; }
    };
    void load();
    const timer = source.status === "running" ? setInterval(() => void load(), 1000) : undefined;
    return () => { disposed = true; clearInterval(timer); };
  }, [open, contextKey, source?.status, source?.detailAvailable, retry]);
  if (source?.detailAvailable && details) {
    input = details.input;
    output = details.output;
    if (details.children?.length) steps = renderChildren?.(details.children);
  }
  // A question that has been answered is not a tool call anyone wants to unfold; it is part of the
  // conversation, and it gets a card of its own below. Anything else about the call — a question still
  // waiting, one that was refused — falls through to the ordinary card.
  const answered = useMemo(
    () =>
      name === "AskUserQuestion" && !isError && !running
        ? parseAnsweredQuestions(input, output)
        : null,
    [name, isError, running, input, output],
  );
  const summary = subagent?.description ?? toolSummary(name, input, cwd);
  const visibleSteps = Math.max(stepCount ?? 0, subagent?.toolUses ?? 0);
  if (answered) return <QuestionAnswerCard items={answered} />;
  return (
    <div className={"sv-tool" + (isError ? " sv-tool-error" : "") + (open ? " sv-tool-open" : "")}>
      {/* The caret leads the line rather than trailing it: at the far end of a wide pane it sits a screen
          away from the name it opens, and reads as a stray glyph rather than as a disclosure. */}
      <button className="sv-tool-head" onClick={() => setOpen((v) => !v)}>
        <Icons.chevR size={12} className={open ? "sv-caret sv-caret-open" : "sv-caret"} />
        <span className="sv-tool-name">{subagent?.title ?? name ?? t("session.toolUnknown")}</span>
        {summary ? <span className="sv-tool-summary">{summary}</span> : null}
        <span className="sv-tool-spacer" />
        {subagent?.model ? <span className="sv-tool-steps">{subagent.model}</span> : null}
        {subagent?.totalTokens ? (
          <span className="sv-tool-steps">{t("chat.subagent.tokens", fmtTokens(subagent.totalTokens))}</span>
        ) : null}
        {visibleSteps ? (
          <span className="sv-tool-steps">{t("chat.subagent.steps", visibleSteps)}</span>
        ) : null}
        {running ? <span className="sv-tool-pending">{t("session.toolRunning")}</span> : null}
      </button>
      {open && (
        <div className="sv-tool-body">
          {source?.detailAvailable && !details && !loadError && <div role="status">{t("common.loading")}</div>}
          {loadError && <div role="alert">{loadError}<button onClick={() => setRetry(value => value + 1)}>{t("common.retry")}</button></div>}
          {steps}
          <ToolBody tool={name} input={input} output={output} isError={isError} />
        </div>
      )}
    </div>
  );
}

/**
 * A question the agent asked and the answer it was given.
 *
 * Shown open rather than folded away behind a caret, because it is not a tool call the way a file read is:
 * it is a turn of the conversation, and which way a decision went is the part of a session someone scrolls
 * back to find. The options that were not taken are left out — what was offered mattered while the question
 * was open, what was chosen is what the rest of the session follows from.
 */
export function QuestionAnswerCard({ items }: { items: AnsweredQuestion[] }) {
  const t = useT();
  return (
    <div className="sv-qa">
      <div className="sv-qa-head">
        <Icons.info size={12} />
        <span>{t("chat.question.answeredHeading", items.length)}</span>
      </div>
      {items.map((item, i) => (
        <div className="sv-qa-item" key={i}>
          <div className="sv-qa-q">{item.question}</div>
          <div className="sv-qa-a">
            <span className="sv-qa-mark" aria-hidden>
              <Icons.check size={9} />
            </span>
            {item.answer ? (
              <span className="sv-qa-answer">{item.answer}</span>
            ) : (
              <span className="sv-qa-blank">{t("chat.question.blankAnswer")}</span>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

/**
 * A run of tool calls, folded into one line.
 *
 * Whether it is open is decided by the pane rather than by this component: the list under it is
 * virtualized, so a card can be scrolled away and drawn again, and someone who opened a run expects to
 * find it open when they scroll back.
 */
export function ToolRunCard({
  calls,
  running,
  open,
  onToggle,
  cwd,
  renderCall,
}: {
  renderCall?: (row: ToolRow) => ReactNode;
  calls: ToolRow[];
  running: boolean;
  open: boolean;
  onToggle: () => void;
  cwd?: string;
}) {
  const t = useT();
  return (
    <div className="sv-run">
      <button className="sv-run-head" onClick={onToggle} title={t("chat.toolRun.tooltip")}>
        <Icons.chevR size={12} className={open ? "sv-caret sv-caret-open" : "sv-caret"} />
        <span className="sv-run-count">{t("chat.toolRun.count", calls.length)}</span>
        <span className="sv-run-summary">{runSummaryText(calls)}</span>
        <span className="sv-tool-spacer" />
        {running ? <span className="sv-tool-pending">{t("session.toolRunning")}</span> : null}
      </button>
      {open && (
        <div className="sv-run-body">
          {calls.map((call) => renderCall ? <div key={call.id}>{renderCall(call)}</div> : (
            <ToolCard
              key={call.id}
              source={call}
              name={call.name}
              input={call.input}
              output={call.output}
              isError={call.isError}
              running={call.status === "running"}
              cwd={cwd}
              subagent={call.subagent}
            />
          ))}
        </div>
      )}
    </div>
  );
}

/** Something went wrong in the conversation itself, rather than inside a tool. */
export function ErrorRow({ message }: { message: string }): ReactNode {
  return <div className="sv-row-error">{message}</div>;
}

/**
 * What a slash command the agent answered itself printed back, such as `/effort` or `/context`.
 *
 * Set apart from the answers around it because nobody said it: it is the session reporting on itself.
 * The text is Markdown — `/context` replies with a table — so it is rendered rather than shown raw.
 */
export function CommandRow({ text }: { text: string }): ReactNode {
  return (
    <div className="sv-row-command">
      <Markdown text={text} />
    </div>
  );
}

/** A remark about the conversation rather than part of it, such as history left out of a replay. */
export function NoticeRow({ message }: { message: string }): ReactNode {
  return <div className="sv-row-notice">{message}</div>;
}

/**
 * The conversation was summarized to make room in the context window.
 *
 * Drawn as a rule across the timeline rather than a message, because it is a boundary rather than
 * something anyone said: everything above it is what the agent now remembers only as a summary.
 */
export function CompactionRow({
  done,
  trigger,
  preTokens,
}: {
  done: boolean;
  trigger?: string;
  preTokens?: number;
}): ReactNode {
  const t = useT();
  const label = !done
    ? t("chat.compaction.running")
    : trigger === "manual"
      ? t("chat.compaction.manual")
      : t("chat.compaction.auto");
  return (
    <div className={"sv-compaction" + (done ? "" : " sv-compaction-running")}>
      <span className="sv-compaction-line" />
      <span className="sv-compaction-label">
        {label}
        {done && preTokens ? (
          <span className="sv-compaction-detail">
            {t("chat.compaction.from", fmtTokens(preTokens))}
          </span>
        ) : null}
      </span>
      <span className="sv-compaction-line" />
    </div>
  );
}

/** Shown while the agent is working, so an empty pause is distinguishable from a finished turn. */
export function WorkingRow({ startedAt }: { startedAt?: number }) {
  const t = useT();
  return (
    <div className="sv-working">
      <span className="sv-dot" />
      {t("session.working")}
      {startedAt !== undefined ? <LiveTurnDuration startedAt={startedAt} /> : null}
    </div>
  );
}
