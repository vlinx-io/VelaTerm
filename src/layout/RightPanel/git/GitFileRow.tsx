//! One file row in the Git panel: status badge, path, line counts, and the actions for that file.
//!
//! The path is split so the directory truncates from its end while the file name always stays whole
//! — the same treatment the Changes modal uses, and for the same reason: the name is the part that
//! identifies the row. Do not reach for `direction: rtl` to ellipsize from the left; under WebKit it
//! eats the first character of a purely LTR name (see the note in ChangesModal).

import Icons from "../../../components/Icons";
import { statusLetter } from "../../../components/diff/changeStatus";
import type { ChangedFile } from "../../../ipc/commands";

/** One hover action on a file row. */
export interface RowAction {
  key: string;
  title: string;
  icon: React.ReactNode;
  onClick: () => void;
  danger?: boolean;
}

export function GitFileRow({
  file,
  additions,
  deletions,
  binary,
  onOpen,
  actions,
  confirming,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancelConfirm,
}: {
  file: ChangedFile;
  additions: number;
  deletions: number;
  binary: boolean;
  /** Opens this file's diff; omitted for rows with nothing to show, such as an untracked directory. */
  onOpen?: () => void;
  actions: RowAction[];
  /** When true the row replaces its actions with an inline confirmation. */
  confirming?: boolean;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm?: () => void;
  onCancelConfirm?: () => void;
}) {
  const letter = statusLetter(file.status);
  const slash = file.path.lastIndexOf("/");
  const dir = slash >= 0 ? file.path.slice(0, slash + 1) : "";
  const base = slash >= 0 ? file.path.slice(slash + 1) : file.path;

  return (
    <div className="git-row" title={file.path}>
      <button type="button" className="git-row-main" onClick={onOpen} disabled={!onOpen}>
        <span className={"gb gb-" + letter}>{letter}</span>
        <span className="nm">
          {dir && <span className="dir">{dir}</span>}
          <span className="base">{base}</span>
        </span>
        {!binary && (additions > 0 || deletions > 0) && (
          <span className="n">
            <span className="plus">+{additions}</span> <span className="minus">−{deletions}</span>
          </span>
        )}
        {binary && <span className="n bin">bin</span>}
      </button>
      {confirming ? (
        <span className="git-row-confirm">
          <button type="button" className="danger" onClick={onConfirm}>
            {confirmLabel}
          </button>
          <button type="button" onClick={onCancelConfirm} title={cancelLabel} aria-label={cancelLabel}>
            <Icons.x size={12} />
          </button>
        </span>
      ) : (
        <span className="git-row-acts">
          {actions.map((a) => (
            <button
              type="button"
              key={a.key}
              title={a.title}
              aria-label={a.title}
              className={a.danger ? "danger" : undefined}
              onClick={a.onClick}
            >
              {a.icon}
            </button>
          ))}
        </span>
      )}
    </div>
  );
}
