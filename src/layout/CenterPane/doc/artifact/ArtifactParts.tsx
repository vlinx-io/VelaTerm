//! Building blocks shared by the artifact tab and the panel beside a conversation: header controls, the
//! sandboxed frame on its stage, the console drawer, and the blocked-network and file-gone states.

import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";

import Icons from "../../../../components/Icons";
import { useT } from "../../../../i18n";
import { isTauri } from "../../../../ipc/transport";
import { env, platform } from "../../../../platform";
import { useTermStore } from "../../../../store/termStore";
import type { ArtifactViewport } from "../../../../store/docTab";
import { consoleReport } from "./artifactBridge";
import {
  addComment, artifactReview, clearComments, commentReport, describeTarget, optionReport, removeComment, setDraft,
  setPicking, subscribeArtifactReview,
} from "./artifactReview";
import { copyText } from "../../../../ipc/transport";
import { getTerminal, pasteToTerminal } from "../../../../terminal/registry";
import { frameLayout, VIEWPORT_ORDER, viewportReadout, type ViewportSize } from "./artifactViewport";
import { insertIntoComposer } from "./artifactWriters";
import type { ArtifactState } from "./useArtifact";
import "./artifact.css";

/** Sandbox flags: scripts, forms and dialogs run; same-origin access, popups and top navigation do not. */
const SANDBOX = "allow-scripts allow-forms allow-modals";

const VIEWPORT_ICON = { fit: Icons.fit, desktop: Icons.monitor, tablet: Icons.tablet, phone: Icons.phone } as const;
const VIEWPORT_LABEL = {
  fit: "artifact.viewportFit",
  desktop: "artifact.viewportDesktop",
  tablet: "artifact.viewportTablet",
  phone: "artifact.viewportPhone",
} as const;

/** Chips and buttons for a header: writer, live, errors, network, viewport, reload, open externally. */
export function ArtifactHeaderControls({
  state,
  path,
  viewport,
  onViewport,
  consoleOpen,
  onToggleConsole,
  compact = false,
}: {
  state: ArtifactState;
  path: string;
  viewport?: ArtifactViewport;
  onViewport?: (v: ArtifactViewport) => void;
  consoleOpen: boolean;
  onToggleConsole: () => void;
  /** Beside a conversation: the writer is the conversation itself, and the panel has little room. */
  compact?: boolean;
}) {
  const t = useT();
  const writerName = useTermStore((s) => (state.writer ? s.sessions.find((x) => x.id === state.writer)?.name : undefined));
  const canOpenExternally = (isTauri || env.isElectron) && !compact;
  const review = useSyncExternalStore(subscribeArtifactReview, () => artifactReview(path));
  return (
    <>
      <button
        className={"artifact-chip" + (review.picking ? " picking" : "")}
        title={t("artifact.commentTitle")}
        aria-pressed={review.picking}
        onClick={() => setPicking(path, !review.picking)}
      >
        <Icons.feedback size={12} />
        {t("artifact.comment")}
        {review.comments.length > 0 && <span className="artifact-count">{review.comments.length}</span>}
      </button>
      {!compact && state.writer && writerName && (
        <button
          className="artifact-chip writer"
          title={t("artifact.goToSession")}
          onClick={() => useTermStore.getState().openSession(state.writer!)}
        >
          <Icons.bot size={12} />
          <span>{writerName}</span>
        </button>
      )}
      {state.live && <span className="artifact-chip live" title={t("artifact.liveTitle")}>{t("artifact.live")}</span>}
      {state.errorCount + state.warnCount > 0 && (
        <button className={"artifact-chip" + (state.errorCount ? " err" : "")} aria-pressed={consoleOpen} onClick={onToggleConsole}>
          {state.errorCount ? <Icons.warn size={12} /> : null}
          {state.errorCount ? t("artifact.errorCount", state.errorCount) : t("artifact.warningCount", state.warnCount)}
        </button>
      )}
      <button
        className={"artifact-chip" + (state.net ? " online" : "")}
        title={t(state.net ? "artifact.onlineTitle" : "artifact.offlineTitle")}
        aria-pressed={state.net}
        onClick={() => state.setNet(!state.net)}
      >
        {state.net ? <Icons.globe size={12} /> : <Icons.shield size={12} />}
        {t(state.net ? "artifact.online" : "artifact.offline")}
      </button>
      {viewport && onViewport && (
        <div className="docview-seg icons" role="group">
          {VIEWPORT_ORDER.map((v) => {
            const Icon = VIEWPORT_ICON[v];
            return (
              <button
                key={v}
                className={viewport === v ? "on" : ""}
                aria-pressed={viewport === v}
                title={t(VIEWPORT_LABEL[v])}
                aria-label={t(VIEWPORT_LABEL[v])}
                onClick={() => onViewport(v)}
              >
                <Icon size={13} />
              </button>
            );
          })}
        </div>
      )}
      <button className="artifact-icon-btn" title={t("artifact.reload")} aria-label={t("artifact.reload")} onClick={state.reload}>
        <Icons.restart size={14} />
      </button>
      {canOpenExternally && (
        <button
          className="artifact-icon-btn"
          title={t("artifact.openInBrowser")}
          aria-label={t("artifact.openInBrowser")}
          onClick={() => void platform.opener.openPath(path)}
        >
          <Icons.external size={14} />
        </button>
      )}
    </>
  );
}

/** The frame on its stage, sized for the viewport; also the banner and console that belong under a header. */
export function ArtifactPane({
  state,
  path,
  title,
  viewport = "fit",
  consoleOpen,
  onCloseConsole,
  onCloseTab,
}: {
  state: ArtifactState;
  path: string;
  title: string;
  viewport?: ArtifactViewport;
  consoleOpen: boolean;
  onCloseConsole: () => void;
  onCloseTab?: () => void;
}) {
  const t = useT();
  if (state.missing) {
    return (
      <div className="artifact-pane">
        <div className="artifact-empty">
          <Icons.file size={20} />
          <span className="artifact-empty-title">{t("artifact.fileGone")}</span>
          <span className="artifact-empty-body">{t("artifact.fileGoneBody", title)}</span>
          {onCloseTab && (
            <button className="vlx-btn" onClick={onCloseTab}>
              {t("doc.closeTab")}
            </button>
          )}
        </div>
      </div>
    );
  }
  return (
    <div className="artifact-pane">
      {!state.net && state.blocked > 0 && (
        <div className="docview-banner artifact-banner-warn" role="status">
          <Icons.shield size={13} />
          <span style={{ flex: 1 }}>{t("artifact.netBlocked", state.blocked)}</span>
          <button onClick={() => state.setNet(true)}>{t("artifact.allowForFile")}</button>
        </div>
      )}
      {state.options.length > 0 && <ArtifactOptions state={state} path={path} />}
      <ArtifactStage state={state} viewport={viewport} title={title} />
      <ArtifactReviewDrawer state={state} path={path} />
      {consoleOpen && <ArtifactConsole state={state} path={path} onClose={onCloseConsole} />}
    </div>
  );
}

function ArtifactStage({ state, viewport, title }: { state: ArtifactState; viewport: ArtifactViewport; title: string }) {
  const t = useT();
  const ref = useRef<HTMLDivElement | null>(null);
  const [area, setArea] = useState<ViewportSize>({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setArea({ width: el.clientWidth, height: el.clientHeight });
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const layout = frameLayout(viewport, area);
  const readout = viewportReadout(viewport, layout);
  const frame = state.url && (
    <iframe
      ref={state.frameRef}
      src={state.url}
      sandbox={SANDBOX}
      referrerPolicy="no-referrer"
      title={t("artifact.frameTitle", title)}
      style={viewport === "fit" ? undefined : { width: layout.width, height: layout.height, transform: `scale(${layout.scale})` }}
    />
  );
  return (
    <div ref={ref} className={"artifact-stage" + (viewport === "fit" ? " fit" : "")}>
      {viewport === "fit" ? (
        frame
      ) : (
        <>
          {area.width > 0 && (
            <div className="artifact-device" style={{ width: layout.width * layout.scale, height: layout.height * layout.scale }}>
              {frame}
            </div>
          )}
          {readout && <span className="artifact-readout">{readout}</span>}
        </>
      )}
    </div>
  );
}

function ArtifactConsole({ state, path, onClose }: { state: ArtifactState; path: string; onClose: () => void }) {
  const t = useT();
  const { deliver, copied, sends } = useDeliver(state.writer);
  return (
    <div className="artifact-console">
      <div className="artifact-console-head">
        <span className="grow">{t("artifact.console")}</span>
        {state.errorCount > 0 && (
          <button
            className="vlx-btn"
            title={sends ? t("artifact.sendToSessionTitle") : undefined}
            onClick={() => void deliver(consoleReport(path, state.console))}
          >
            {copied ? t("common.copied") : sends ? t("artifact.sendToSession") : t("common.copy")}
          </button>
        )}
        <button className="vlx-btn" onClick={state.clearConsole}>
          {t("artifact.clearConsole")}
        </button>
        <button className="artifact-icon-btn" title={t("artifact.closeConsole")} aria-label={t("artifact.closeConsole")} onClick={onClose}>
          <Icons.close size={13} />
        </button>
      </div>
      <div className="artifact-console-list">
        {state.console.length === 0 && <div className="artifact-console-empty">{t("artifact.consoleEmpty")}</div>}
        {state.console.map((e) => (
          <div key={e.id} className={"artifact-console-row " + e.level}>
            <Icons.warn size={13} />
            <span className="artifact-console-text">{e.text}</span>
            <span className="artifact-console-line">{e.line ? `:${e.line}` : ""}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

/**
 * Where text for the page's agent goes. A conversation takes it in its message box and an agent's own terminal
 * interface as a paste, both without sending; a plain shell or an unknown writer gets nothing typed into it, so
 * the text is copied instead.
 */
function deliveryTarget(writer: string | undefined): "composer" | "terminal" | "copy" {
  if (!writer) return "copy";
  const state = useTermStore.getState();
  const session = state.sessions.find((s) => s.id === writer) ?? state.ephemeralSessions[writer];
  if (!session || session.kind === "terminal") return "copy";
  if (session.engine === "chat") return "composer";
  return getTerminal(writer) ? "terminal" : "copy";
}

/** Hand text to the session that wrote the page, or copy it when it cannot take it. */
function useDeliver(writer: string | undefined) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1600);
    return () => clearTimeout(timer);
  }, [copied]);
  // Read during render so the button names what will happen; the live check repeats on click.
  const target = useTermStore(() => deliveryTarget(writer));
  const deliver = async (text: string): Promise<boolean> => {
    const now = deliveryTarget(writer);
    if (writer && now === "composer") {
      insertIntoComposer(writer, text);
      useTermStore.getState().openSession(writer);
      return true;
    }
    if (writer && now === "terminal") {
      useTermStore.getState().openSession(writer);
      pasteToTerminal(writer, text);
      return true;
    }
    try {
      await copyText(text);
      setCopied(true);
      return true;
    } catch {
      return false;
    }
  };
  return { deliver, copied, sends: target !== "copy" };
}

/** The page's design options: jump to one, and tell the agent which to keep. */
function ArtifactOptions({ state, path }: { state: ArtifactState; path: string }) {
  const t = useT();
  const [chosen, setChosen] = useState<string | null>(null);
  const { deliver, copied, sends } = useDeliver(state.writer);
  const current = state.options.find((o) => o.id === chosen) ?? null;
  return (
    <div className="artifact-options" role="group" aria-label={t("artifact.options")}>
      <span className="artifact-options-label">{t("artifact.options")}</span>
      <div className="artifact-options-list">
        {state.options.map((o) => (
          <button
            key={o.id}
            className={"artifact-option" + (o.id === chosen ? " on" : "")}
            aria-pressed={o.id === chosen}
            title={o.label || o.id}
            onClick={() => { setChosen(o.id); state.revealOption(o.id); }}
          >
            <span className="artifact-option-id">{o.id}</span>
            {o.label && <span className="artifact-option-label">{o.label}</span>}
          </button>
        ))}
      </div>
      {current && (
        <button
          className="vlx-btn vlx-btn-primary artifact-option-use"
          title={t(sends ? "artifact.useOptionTitle" : "artifact.useOptionCopyTitle")}
          onClick={() => void deliver(optionReport(path, current.id, current.label))}
        >
          {copied ? t("common.copied") : t("artifact.useOption", current.id)}
        </button>
      )}
    </div>
  );
}

/** Pick prompt, the comment box for the picked element, and the comments waiting to be sent. */
function ArtifactReviewDrawer({ state, path }: { state: ArtifactState; path: string }) {
  const t = useT();
  const review = useSyncExternalStore(subscribeArtifactReview, () => artifactReview(path));
  const [note, setNote] = useState("");
  const { deliver, copied, sends } = useDeliver(state.writer);
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    setNote("");
    if (review.draft) input.current?.focus();
  }, [review.draft]);
  // Esc leaves pick mode wherever focus is; inside the page the review script reports it instead.
  useEffect(() => {
    if (!review.picking) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented || e.isComposing) return;
      e.preventDefault();
      setPicking(path, false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [review.picking, path]);
  if (!review.picking && !review.draft && review.comments.length === 0) return null;
  const add = () => { if (note.trim()) addComment(path, note); };
  const send = async () => {
    if (await deliver(commentReport(path, review.comments))) clearComments(path);
  };
  return (
    <div className="artifact-review">
      {review.picking && !review.draft && (
        <div className="artifact-review-hint" role="status">
          <Icons.feedback size={13} />
          <span className="grow">{t("artifact.pickHint")}</span>
          <button className="vlx-btn" onClick={() => setPicking(path, false)}>{t("common.cancel")}</button>
        </div>
      )}
      {review.draft && (
        <div className="artifact-review-draft">
          <div className="artifact-review-target">{t("artifact.commentOn", describeTarget(review.draft))}</div>
          <textarea
            ref={input}
            className="vlx-input artifact-review-input"
            rows={2}
            value={note}
            placeholder={t("artifact.commentPlaceholder")}
            aria-label={t("artifact.commentPlaceholder")}
            onChange={(e) => setNote(e.target.value)}
            onKeyDown={(e) => {
              if (e.nativeEvent.isComposing || e.keyCode === 229) return;
              if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); add(); }
              else if (e.key === "Escape") { e.preventDefault(); setDraft(path, null); }
            }}
          />
          <div className="artifact-review-actions">
            <button className="vlx-btn" onClick={() => setDraft(path, null)}>{t("common.cancel")}</button>
            <button className="vlx-btn vlx-btn-primary" disabled={!note.trim()} onClick={add}>{t("artifact.commentAdd")}</button>
          </div>
        </div>
      )}
      {review.comments.length > 0 && (
        <>
          <div className="artifact-review-list">
            {review.comments.map((c, i) => (
              <div key={c.id} className="artifact-review-row">
                <button className="artifact-review-pin" title={t("artifact.showInPage")} onClick={() => state.revealSelector(c.selector)}>
                  {i + 1}
                </button>
                <div className="artifact-review-body">
                  <span className="artifact-review-el">{describeTarget(c)}{c.option ? ` · ${c.option}` : ""}</span>
                  <span className="artifact-review-note">{c.note}</span>
                </div>
                <button className="artifact-icon-btn" title={t("artifact.commentRemove")} aria-label={t("artifact.commentRemove")}
                  onClick={() => removeComment(path, c.id)}>
                  <Icons.close size={12} />
                </button>
              </div>
            ))}
          </div>
          <div className="artifact-review-foot">
            <button className="vlx-btn" onClick={() => setPicking(path, true)} disabled={review.picking}>{t("artifact.commentMore")}</button>
            <span className="grow" />
            <button className="vlx-btn" onClick={() => clearComments(path)}>{t("artifact.clearConsole")}</button>
            <button className="vlx-btn vlx-btn-primary" title={t(sends ? "artifact.sendCommentsTitle" : "artifact.copyCommentsTitle")}
              onClick={() => void send()}>
              {copied ? t("common.copied") : sends ? t("artifact.sendToSession") : t("common.copy")}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
