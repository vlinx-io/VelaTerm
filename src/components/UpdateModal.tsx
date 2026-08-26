//! Update details dialog with release notes, download progress, and update/later/skip actions.
//!
//! Automatic checks only light up UpdateSeg in the status bar; the dialog opens when the user
//! clicks it. A manual "Check for Updates" action opens it directly. Closing during download is
//! harmless because ipc/updater.ts owns progress and continues in the background status segment.
//!
//! Release notes are selected Markdown changelog sections since the installed version. `marked`
//! tokenizes them and each token maps to a React element without dangerouslySetInnerHTML, ensuring
//! even a compromised update endpoint cannot execute supplied HTML.

import { marked, type Token, type Tokens } from "marked";
import type { ReactNode } from "react";

import { getLocale, useT } from "../i18n";
import {
  closeUpdateModal,
  DOWNLOAD_PAGE_URL,
  dismissUpdate,
  restartApp,
  startInstall,
  useUpdateState,
} from "../ipc/updater";
import { platform } from "../platform";
import { useTermStore } from "../store/termStore";
import { Backdrop } from "./Backdrop";

/** On Windows the plugin exits after download so NSIS can take over; warn the user beforehand. */
const isWindows = () => /Win/i.test(navigator.userAgent);

// ── Markdown -> React (a controlled subset sufficient for changelogs) ─────────

function renderInline(tokens: Token[] | undefined, kp: string): ReactNode[] {
  if (!tokens) return [];
  return tokens.map((tk, i): ReactNode => {
    const key = `${kp}.${i}`;
    switch (tk.type) {
      case "text": {
        const sub = (tk as Tokens.Text).tokens;
        return sub ? <span key={key}>{renderInline(sub, key)}</span> : (tk as Tokens.Text).text;
      }
      case "escape":
        return (tk as Tokens.Escape).text;
      case "strong":
        return (
          <strong key={key} style={{ fontWeight: 600, color: "var(--text-primary)" }}>
            {renderInline((tk as Tokens.Strong).tokens, key)}
          </strong>
        );
      case "em":
        return <em key={key}>{renderInline((tk as Tokens.Em).tokens, key)}</em>;
      case "del":
        return <del key={key}>{renderInline((tk as Tokens.Del).tokens, key)}</del>;
      case "codespan":
        return (
          <code
            key={key}
            style={{
              fontFamily: "var(--font-mono, monospace)",
              fontSize: "0.92em",
              padding: "1px 4px",
              borderRadius: 3,
              background: "var(--bg-elevated)",
            }}
          >
            {(tk as Tokens.Codespan).text}
          </code>
        );
      case "link": {
        const href = (tk as Tokens.Link).href;
        return (
          <a
            key={key}
            href={href}
            onClick={(e) => {
              // Open links in the system browser instead of navigating the WebView.
              e.preventDefault();
              void platform.opener.openExternal(href).catch(() => {});
            }}
            style={{ color: "var(--accent)", cursor: "pointer" }}
          >
            {renderInline((tk as Tokens.Link).tokens, key)}
          </a>
        );
      }
      case "br":
        return <br key={key} />;
      case "html":
        // Discard HTML outright; release notes must never execute it.
        return null;
      default: {
        const txt = (tk as { text?: string }).text;
        return txt ?? null;
      }
    }
  });
}

function renderBlocks(tokens: Token[] | undefined, kp: string): ReactNode[] {
  if (!tokens) return [];
  const out: ReactNode[] = [];
  tokens.forEach((tk, i) => {
    const key = `${kp}.${i}`;
    switch (tk.type) {
      case "heading": {
        const h = tk as Tokens.Heading;
        // Render version headings (##) as titles and deeper headings such as `### Fixed` as group labels.
        const isVersion = h.depth <= 2;
        out.push(
          <div
            key={key}
            style={
              isVersion
                ? {
                    fontSize: 13,
                    fontWeight: 600,
                    color: "var(--text-primary)",
                    margin: "18px 0 8px",
                  }
                : {
                    fontSize: 11,
                    fontWeight: 600,
                    letterSpacing: 0.6,
                    textTransform: "uppercase",
                    color: "var(--text-dim)",
                    margin: "14px 0 6px",
                  }
            }
          >
            {renderInline(h.tokens, key)}
          </div>,
        );
        break;
      }
      case "paragraph":
        out.push(
          <p key={key} style={{ margin: "0 0 8px" }}>
            {renderInline((tk as Tokens.Paragraph).tokens, key)}
          </p>,
        );
        break;
      case "text": {
        const sub = (tk as Tokens.Text).tokens;
        out.push(
          <span key={key}>{sub ? renderInline(sub, key) : (tk as Tokens.Text).text}</span>,
        );
        break;
      }
      case "list": {
        const list = tk as Tokens.List;
        const items = list.items.map((item, j) => (
          <li key={`${key}.${j}`} style={{ margin: "0 0 6px" }}>
            {renderBlocks(item.tokens, `${key}.${j}`)}
          </li>
        ));
        out.push(
          list.ordered ? (
            <ol key={key} style={{ margin: "0 0 8px", paddingLeft: 20 }}>
              {items}
            </ol>
          ) : (
            <ul key={key} style={{ margin: "0 0 8px", paddingLeft: 20 }}>
              {items}
            </ul>
          ),
        );
        break;
      }
      case "code":
        out.push(
          <pre
            key={key}
            style={{
              margin: "0 0 8px",
              padding: 8,
              borderRadius: 6,
              overflowX: "auto",
              background: "var(--bg-elevated)",
              fontFamily: "var(--font-mono, monospace)",
              fontSize: 11.5,
            }}
          >
            {(tk as Tokens.Code).text}
          </pre>,
        );
        break;
      case "blockquote":
        out.push(
          <blockquote
            key={key}
            style={{
              margin: "0 0 8px",
              paddingLeft: 10,
              borderLeft: "2px solid var(--border)",
              color: "var(--text-dim)",
            }}
          >
            {renderBlocks((tk as Tokens.Blockquote).tokens, key)}
          </blockquote>,
        );
        break;
      case "hr":
        out.push(
          <hr
            key={key}
            style={{ border: 0, borderTop: "1px solid var(--border)", margin: "14px 0" }}
          />,
        );
        break;
      case "space":
      case "html":
        break;
      default: {
        const sub = (tk as { tokens?: Token[] }).tokens;
        if (sub) out.push(<div key={key}>{renderBlocks(sub, key)}</div>);
        break;
      }
    }
  });
  return out;
}

function ReleaseNotes({ markdown }: { markdown: string }) {
  const t = useT();
  if (!markdown.trim()) {
    return (
      <div style={{ fontSize: 12.5, color: "var(--text-dim)", fontStyle: "italic" }}>
        {t("updater.noNotes")}
      </div>
    );
  }
  return <>{renderBlocks(marked.lexer(markdown), "n")}</>;
}

// ── Dialog ────────────────────────────────────────────────────────────────

const BTN_BASE = {
  padding: "6px 14px",
  fontSize: 12,
  borderRadius: 6,
  cursor: "pointer",
} as const;

export function UpdateModal() {
  const t = useT();
  const { prompt, stage, modalOpen } = useUpdateState();
  const setShareOpen = useTermStore((s) => s.setShareOpen);
  if (!prompt || !modalOpen) return null;

  const busy = stage.kind === "downloading" || stage.kind === "installing";
  const win = isWindows();
  // useT subscribes to the locale store, so changing language rerenders with the matching changelog.
  const releaseNotes = prompt.localizedNotes[getLocale()] ?? prompt.notes;

  return (
    <Backdrop onClose={closeUpdateModal} zIndex={10000}>
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 560,
          maxWidth: "92vw",
          background: "var(--bg-app)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 20,
          color: "var(--text-primary)",
          boxShadow: "0 12px 40px rgba(0,0,0,0.4)",
        }}
      >
        <div style={{ fontSize: 14, fontWeight: 600 }}>{t("updater.available")}</div>
        <div style={{ fontSize: 12, color: "var(--text-dim)", marginTop: 4 }}>
          {t("updater.versionLine", prompt.version, prompt.currentVersion)}
        </div>

        <div
          style={{
            margin: "14px 0",
            padding: "2px 14px 10px 0",
            maxHeight: 320,
            overflowY: "auto",
            borderTop: "1px solid var(--border)",
            borderBottom: "1px solid var(--border)",
            fontSize: 12.5,
            lineHeight: 1.6,
            color: "var(--text-dim)",
          }}
        >
          <ReleaseNotes markdown={releaseNotes} />
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            flexWrap: "wrap",
            gap: 10,
            marginBottom: 12,
            padding: "10px 12px",
            border: "1px solid var(--accent-line)",
            borderRadius: 8,
            background: "var(--accent-soft)",
          }}
        >
          <div
            style={{
              flex: "1 1 330px",
              fontSize: 12,
              lineHeight: 1.55,
              color: "var(--text-primary)",
            }}
          >
            {t("share.subtitle")}
          </div>
          <button
            onClick={() => {
              closeUpdateModal();
              setShareOpen(true);
            }}
            style={{
              ...BTN_BASE,
              marginLeft: "auto",
              border: "1px solid var(--accent)",
              background: "transparent",
              color: "var(--accent)",
              whiteSpace: "nowrap",
            }}
          >
            {t("share.title")}
          </button>
        </div>

        {stage.kind === "available" && win && (
          <div style={{ fontSize: 11.5, color: "var(--text-dim)", marginBottom: 12 }}>
            {t("updater.windowsNotice")}
          </div>
        )}
        {stage.kind === "downloading" && (
          <DownloadProgress received={stage.received} total={stage.total} />
        )}
        {stage.kind === "installing" && (
          <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 12 }}>
            {win ? t("updater.installingWindows") : t("updater.installing")}
          </div>
        )}
        {stage.kind === "ready" && (
          <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 12 }}>
            {t("updater.installed")}
          </div>
        )}
        {stage.kind === "error" && (
          <div style={{ fontSize: 12, color: "var(--status-asking)", marginBottom: 12 }}>
            {t("updater.downloadFailed", stage.detail)}
          </div>
        )}

        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {stage.kind === "available" && (
            <button
              onClick={() => dismissUpdate({ skip: true })}
              title={t("updater.skipVersionHint")}
              style={{
                ...BTN_BASE,
                border: "1px solid transparent",
                background: "transparent",
                color: "var(--text-dim)",
              }}
            >
              {t("updater.skipVersion")}
            </button>
          )}
          {stage.kind === "error" && (
            <button
              onClick={() => {
                void platform.opener.openExternal(DOWNLOAD_PAGE_URL).catch(() => {});
              }}
              title={t("updater.downloadManuallyHint")}
              style={{
                ...BTN_BASE,
                border: "1px solid var(--border)",
                background: "transparent",
                color: "var(--text-primary)",
              }}
            >
              {t("updater.downloadManually")}
            </button>
          )}
          <div style={{ flex: 1 }} />
          <button
            onClick={closeUpdateModal}
            title={busy ? t("updater.hideHint") : undefined}
            style={{
              ...BTN_BASE,
              border: "1px solid var(--border)",
              background: "transparent",
              color: "var(--text-primary)",
            }}
          >
            {busy ? t("updater.hide") : t("updater.later")}
          </button>
          {(stage.kind === "available" || stage.kind === "error") && (
            <button
              onClick={() => void startInstall()}
              style={{
                ...BTN_BASE,
                border: "1px solid var(--accent)",
                background: "var(--accent)",
                color: "#fff",
              }}
            >
              {stage.kind === "error" ? t("updater.retry") : t("updater.updateNow")}
            </button>
          )}
          {stage.kind === "ready" && (
            <button
              onClick={() => void restartApp()}
              style={{
                ...BTN_BASE,
                border: "1px solid var(--accent)",
                background: "var(--accent)",
                color: "#fff",
              }}
            >
              {t("updater.restartNow")}
            </button>
          )}
        </div>
      </div>
    </Backdrop>
  );
}

/** Download progress; without Content-Length, fall back to an indeterminate downloaded-MB label. */
function DownloadProgress({ received, total }: { received: number; total: number }) {
  const t = useT();
  const pct = total > 0 ? Math.min(100, Math.round((received / total) * 100)) : 0;
  const mb = (n: number) => (n / 1024 / 1024).toFixed(1);
  return (
    <div style={{ marginBottom: 12 }}>
      <div
        style={{
          height: 4,
          borderRadius: 2,
          background: "var(--bg-elevated)",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            height: "100%",
            width: total > 0 ? `${pct}%` : "100%",
            background: "var(--accent)",
            opacity: total > 0 ? 1 : 0.4,
            transition: "width 120ms linear",
          }}
        />
      </div>
      <div style={{ fontSize: 11.5, color: "var(--text-dim)", marginTop: 6 }}>
        {total > 0
          ? t("updater.downloadingPct", pct)
          : t("updater.downloadingBytes", mb(received))}
      </div>
    </div>
  );
}
