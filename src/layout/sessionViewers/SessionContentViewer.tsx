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
//! the transcript. An empty array confirms there is no transcript and selects recording playback;
//! omitting the prop retains internal loading for archive browsing.

import { useEffect, useState } from "react";

import { t } from "../../i18n";
import { readAgentTranscript, type TranscriptMessage } from "../../ipc/commands";
import type { Session } from "../../types";
import { RecordingViewer } from "./RecordingViewer";
import { TranscriptViewer } from "./TranscriptViewer";

/** Whether this session type inherently lacks an agent transcript and should use recording playback. */
function hasNoTranscript(kind: Session["kind"]): boolean {
  return kind === "terminal" || kind === "browser";
}

export function SessionContentViewer({
  session,
  source,
  initialQuery,
  highlightTerms,
  scrollToMessageIndex,
  scrollToOrdinal,
  preloadedMessages,
}: {
  session: Session;
  /** Known global-search match source; "recording" skips transcript detection. */
  source?: "transcript" | "recording";
  /** Browsing-mode prefill for the transcript filter field. */
  initialQuery?: string;
  /** Literals matched by the search index for the active hit; enables locate highlighting. */
  highlightTerms?: string[];
  scrollToMessageIndex?: number;
  scrollToOrdinal?: number;
  /** Externally cached transcript from the search console; providing it skips internal loading. */
  preloadedMessages?: TranscriptMessage[];
}) {
  // Use recording directly for recording matches or session types that have no transcript.
  const recordingOnly = source === "recording" || hasNoTranscript(session.kind);
  const hasPreload = preloadedMessages !== undefined;

  // Use nonempty preloaded messages directly; an empty list confirms fallback to recording.
  const [messages, setMessages] = useState<TranscriptMessage[] | null>(
    hasPreload && preloadedMessages.length > 0 ? preloadedMessages : null,
  );
  const [fallback, setFallback] = useState(
    recordingOnly || (hasPreload && preloadedMessages.length === 0),
  );

  useEffect(() => {
    // Do not load internally when recording is selected or messages were preloaded.
    if (recordingOnly || hasPreload) return;
    let cancelled = false;
    readAgentTranscript(session.id)
      .then((msgs) => {
        if (cancelled) return;
        if (msgs.length > 0) setMessages(msgs);
        else setFallback(true);
      })
      .catch(() => {
        if (!cancelled) setFallback(true);
      });
    return () => {
      cancelled = true;
    };
  }, [session.id, recordingOnly, hasPreload]);

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
  if (fallback)
    return (
      <RecordingViewer
        sessionId={session.id}
        initialQuery={highlightTerms?.[0] ?? initialQuery}
        scrollToOrdinal={scrollToOrdinal}
      />
    );
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
