//! Commit message box for the Git panel. It commits what is staged — never what is merely changed —
//! so the button says how many files are going in and stays disabled until both a message and
//! something staged exist. Amend is a separate toggle because it rewrites the previous commit
//! rather than adding one, which is a different act and should not be one stray click away.

import { useState } from "react";
import { useT } from "../../../i18n";

export function CommitBox({
  stagedCount,
  busy,
  onCommit,
}: {
  stagedCount: number;
  busy: boolean;
  /** Resolves to true when the commit succeeded, which is when the message box clears. */
  onCommit: (message: string, amend: boolean) => Promise<boolean>;
}) {
  const t = useT();
  const [message, setMessage] = useState("");
  const [amend, setAmend] = useState(false);

  // Amend rewrites the last commit and needs no new staged file, so only that mode commits with an
  // empty staging area.
  const canCommit = message.trim().length > 0 && (amend || stagedCount > 0) && !busy;

  const submit = async () => {
    if (!canCommit) return;
    const ok = await onCommit(message.trim(), amend);
    if (ok) {
      setMessage("");
      setAmend(false);
    }
  };

  return (
    <div className="git-commit">
      <textarea
        className="git-commit-msg"
        value={message}
        rows={2}
        placeholder={t("git.commitPlaceholder")}
        onChange={(e) => setMessage(e.target.value)}
        onKeyDown={(e) => {
          // Enter inserts a newline for a commit body; the platform's accelerator commits.
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            void submit();
          }
        }}
      />
      <div className="git-commit-bar">
        <label className="git-amend">
          <input type="checkbox" checked={amend} onChange={(e) => setAmend(e.target.checked)} />
          {t("git.amend")}
        </label>
        <button type="button" className="git-commit-btn" disabled={!canCommit} onClick={() => void submit()}>
          {amend ? t("git.amendCommit") : t("git.commitCount", stagedCount)}
        </button>
      </div>
    </div>
  );
}
