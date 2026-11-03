//! Session-content dispatcher shared by archives and global-search location. Agent sessions prefer
//! the readable transcript view. Terminal/browser sessions and unavailable transcripts (missing
//! IDs, deleted files, OpenCode, and similar cases) fall back to read-only recording playback.
//!
//! Optional global-search location anchors:
//! - A known `source` selects the path directly; "recording" skips transcript detection.
//! - Transcript mode forwards `highlightTerms` and `scrollToMessageIndex` to TranscriptViewer.
//! - Recording mode forwards the first of `highlightTerms` as the find query and `scrollToOrdinal` to
//!   RecordingViewer.
//! - `highlightTerms` are the literals the search index matched for the active hit; `initialQuery` is the
//!   archive-browsing prefill and is not used for location.
//!
//! Optional `preloadedMessages` bypasses internal readAgentTranscript and uses the search console's
//! cache. Moving between matches in one session then only scrolls; it neither remounts nor reloads
//! the transcript. An empty array selects recording playback, except for Kiro, where it is re-read to
//! distinguish an empty record from a cached failure. Omitting the prop retains internal loading.

import { lazy, Suspense, useEffect, useState } from "react";

import { t } from "../../i18n";
import { readAgentTranscript, type TranscriptMessage } from "../../ipc/commands";
import type { Session } from "../../types";
import { TranscriptViewer } from "./TranscriptViewer";
import { PagedConversation } from "./PagedConversation";
import { hasPagedHistory } from "./pagedHistory";

// Recording playback embeds its own xterm, which would otherwise ride into the entry bundle with global search.
const RecordingViewer = lazy(() => import("./RecordingViewer").then((m) => ({ default: m.RecordingViewer })));

/** Whether this session type inherently lacks an agent transcript and should use recording playback. */
function hasNoTranscript(kind: Session["kind"]): boolean {
  return kind === "terminal" || kind === "browser";
}

interface SessionContentProps {
  session: Session;
  /** Known global-search match source; "recording" skips transcript detection. */
  source?: "transcript" | "recording";
  initialQuery?: string;
  highlightTerms?: string[];
  scrollToMessageIndex?: number;
  /** Byte offset of the hit's first record, for paged conversations. */
  scrollToRecordOffset?: number;
  scrollToRowId?: string;
  scrollToOrdinal?: number;
  /** Search caches failures as empty arrays; Kiro rechecks them to retain the backend reason. */
  preloadedMessages?: TranscriptMessage[];
}

export function SessionContentViewer(props: SessionContentProps) {
  const { session, source, preloadedMessages } = props;
  if (source === "recording" || hasNoTranscript(session.kind)) return <RecordingContent {...props} />;
  if (hasPagedHistory(session)) return <PagedConversation session={session} messageIndex={props.scrollToRowId ? undefined : props.scrollToMessageIndex}
    recordOffset={props.scrollToRowId ? undefined : props.scrollToRecordOffset} rowId={props.scrollToRowId} />;
  if (preloadedMessages !== undefined) {
    if (preloadedMessages.length > 0) return <TranscriptViewer {...props} messages={preloadedMessages} />;
    if (session.kind !== "kiro") return <RecordingContent {...props} />;
  }
  // Only the transcript reader owns asynchronous state. Source changes unmount it, while moving
  // between hits in the same source keeps the reader and viewer mounted without another read.
  return <LoadedTranscript key={`${session.id}:${session.kind}`} {...props} />;
}

function RecordingContent({ session, highlightTerms, initialQuery, scrollToOrdinal }: SessionContentProps) {
  return (
    <Suspense fallback={null}>
      <RecordingViewer sessionId={session.id} initialQuery={highlightTerms?.[0] ?? initialQuery} scrollToOrdinal={scrollToOrdinal} />
    </Suspense>
  );
}

function LoadedTranscript(props: SessionContentProps) {
  const { session, initialQuery, highlightTerms, scrollToMessageIndex } = props;
  const isKiro = session.kind === "kiro";
  const [messages, setMessages] = useState<TranscriptMessage[] | null>(null);
  const [fallback, setFallback] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    readAgentTranscript(session.id)
      .then((msgs) => {
        if (cancelled) return;
        if (msgs.length > 0 || isKiro) setMessages(msgs);
        else setFallback(true);
      })
      .catch((reason) => {
        if (!cancelled) {
          if (isKiro) setError(String(reason));
          setFallback(true);
        }
      });
    return () => { cancelled = true; };
  }, [session.id, isKiro]);

  if (isKiro && error)
    return (
      <div className="sv" style={{ gap: "var(--sv-pad)" }}>
        <div role="alert" style={{ padding: "var(--sv-pad)", overflowWrap: "anywhere", overflow: "auto", maxHeight: "40%", flexShrink: 0 }}>
          {error}
        </div>
        <div style={{ position: "relative", flex: 1, minHeight: 0 }}>
          <RecordingContent {...props} />
        </div>
      </div>
    );

  if (messages)
    return (
      <TranscriptViewer
        session={session}
        messages={messages}
        initialQuery={initialQuery}
        highlightTerms={highlightTerms}
        scrollToMessageIndex={scrollToMessageIndex}
      />
    );
  if (fallback) return <RecordingContent {...props} />;
  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: "var(--text-muted)",
        fontSize: 13,
      }}
    >
      {t("archive.loadingTranscript")}
    </div>
  );
}
