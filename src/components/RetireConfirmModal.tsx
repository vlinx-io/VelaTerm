//! Retire deletes each listed worktree and branch for good, and deletes nothing until this card answers.

import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { useSuspendNativeViews } from "../hooks/nativeViewSuspend";
import { retireResult } from "../ipc/commands";
import { onRetireCancel, onRetireRequest, type RetireRequest } from "../ipc/events";
import { notify } from "../notify";
import { useTermStore } from "../store/termStore";
import { t as translate } from "../i18n";

/** A retire card blocks its caller for the whole answer timeout, so a hidden window still hears it. */
function announce(req: RetireRequest) {
  const { notifyEnabled, soundEnabled } = useTermStore.getState();
  if (!notifyEnabled) return;
  const action =
    req.action === "cleanup-and-archive"
      ? translate("retire.actionCleanup")
      : translate("retire.actionArchive");
  void notify(req.sessionId, translate("retire.notifyTitle"), `${req.name}: ${action}`, soundEnabled);
}

export function RetireConfirmModal() {
  const t = useT();
  const [queue, setQueue] = useState<RetireRequest[]>([]);

  useEffect(() => {
    let disposed = false;
    const unlisten: (() => void)[] = [];
    const register = (pending: Promise<() => void>) =>
      void pending
        .then((fn) => {
          // The listener resolves after unmount when the card is remounted quickly; drop it instead of leaking.
          if (disposed) fn();
          else unlisten.push(fn);
        })
        .catch(() => {});
    register(
      onRetireRequest((req) => {
        setQueue((q) => [...q, req]);
        announce(req);
      }),
    );
    // An expired request destroys nothing, so its card leaves instead of collecting a dead approval.
    register(
      onRetireCancel((requestId) =>
        setQueue((q) => q.filter((req) => req.requestId !== requestId)),
      ),
    );
    return () => {
      disposed = true;
      for (const fn of unlisten) fn();
    };
  }, []);

  // The queue head is the active request; an empty queue renders no card.
  const req = queue.length > 0 ? queue[0] : null;

  // Suspend native browser views while the card is visible so they cannot cover it.
  useSuspendNativeViews(Boolean(req));

  if (!req) return null;

  const remaining = queue.length - 1;
  const worktrees = req.worktrees ?? [];
  const answer = (approved: boolean) => {
    void retireResult(req.requestId, {
      approved,
      error: approved ? undefined : "the user declined the retire",
    }).catch(() => {});
    setQueue((q) => q.slice(1));
  };

  return (
    <div
      role="dialog"
      aria-label={t("retire.title")}
      style={{
        position: "fixed",
        top: 46, // Leave 8 px below the 38 px title bar.
        right: 16,
        zIndex: 1100,
        width: 400,
        maxWidth: "calc(100vw - 32px)",
        background: "var(--bg-panel)",
        border: "1px solid var(--border-strong)",
        borderRadius: 10,
        padding: 18,
        boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "baseline",
          justifyContent: "space-between",
          gap: 8,
          marginBottom: 14,
        }}
      >
        <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>
          {t("retire.title")}
        </div>
        {remaining > 0 && (
          <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {t("retire.remaining", remaining)}
          </div>
        )}
      </div>

      <div style={{ fontSize: 12.5, color: "var(--text-primary)", marginBottom: 6 }}>
        {t("retire.session")}: {req.name}
      </div>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 12 }}>
        {req.action === "cleanup-and-archive"
          ? t("retire.actionCleanup")
          : t("retire.actionArchive")}
        {req.descendantCount > 0 && ` ${t("retire.descendants", req.descendantCount)}`}
      </div>

      {worktrees.length > 0 && (
        <div
          role="alert"
          style={{
            marginBottom: 12,
            padding: "8px 10px",
            border: "1px solid var(--status-asking)",
            borderRadius: 5,
            color: "var(--text-primary)",
            fontSize: 11,
            lineHeight: 1.45,
          }}
        >
          <div style={{ color: "var(--status-asking)", fontWeight: 600, marginBottom: 3 }}>
            {t("retire.irreversible")}
          </div>
          {t("retire.worktreeCount", worktrees.length)}
        </div>
      )}

      {worktrees.length > 0 && (
        <div
          style={{
            maxHeight: 220,
            overflowY: "auto",
            display: "flex",
            flexDirection: "column",
            gap: 8,
          }}
        >
          {worktrees.map((w) => (
            <div
              key={w.id}
              style={{
                padding: "7px 9px",
                background: "var(--bg-app)",
                border: "1px solid var(--border)",
                borderRadius: 5,
                fontFamily: "var(--font-mono, monospace)",
                fontSize: 11.5,
                lineHeight: 1.5,
                wordBreak: "break-all",
              }}
            >
              <div style={{ color: "var(--text-primary)" }}>{w.name}</div>
              <div style={{ color: "var(--text-muted)" }}>
                {t("retire.pathLabel")}: {w.path}
              </div>
              <div style={{ color: "var(--text-muted)" }}>
                {t("retire.branchLabel")}: {w.branch || t("retire.branchUnknown")}
              </div>
              {w.resumed && (
                <div style={{ color: "var(--text-muted)" }}>{t("retire.resumed")}</div>
              )}
            </div>
          ))}
        </div>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 18 }}>
        <button className="vlx-btn" onClick={() => answer(false)}>
          {t("retire.keep")}
        </button>
        <button className="vlx-btn vlx-btn-primary" onClick={() => answer(true)}>
          {t("retire.approve")}
        </button>
      </div>
    </div>
  );
}
