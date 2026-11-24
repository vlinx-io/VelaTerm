//! Diff document tab: a file's uncommitted changes against HEAD, read-only, in the same viewer the Changes
//! tab uses, including inline review comments shared with the repository's working-tree review scope.

import { useMemo, useState } from "react";
import {
  FileDiffView,
  readDiffOptions,
  writeDiffOptions,
  type DiffDraftRequest,
  type DiffOptions,
  type DiffSelectionRange,
} from "../../../components/diff/FileDiffView";
import { DiffReviewBar } from "../../../components/diff/DiffReviewBar";
import { formatLineRange, reviewScopeKey } from "../../../components/diff/diffReviewStore";
import Icons from "../../../components/Icons";
import { useT } from "../../../i18n";
import { useTermStore, type DocTab } from "../../../store/termStore";
import "../../../components/diff/changes-modal.css";
import "./docTheme.css";

export function DiffDocView({ tab, hidden }: { tab: DocTab; hidden: boolean }) {
  const t = useT();
  const openDocTab = useTermStore((s) => s.openDocTab);
  const refresh = useTermStore((s) => s.refreshDocTab);
  const [options, setOptionsState] = useState<DiffOptions>(readDiffOptions);
  const [lineSelection, setLineSelection] = useState<DiffSelectionRange | null>(null);
  const [draftRequest, setDraftRequest] = useState<DiffDraftRequest | null>(null);

  const setOption = (key: keyof DiffOptions) => setOptionsState((current) => {
    const next = { ...current, [key]: !current[key] };
    writeDiffOptions(next);
    return next;
  });
  const diff = tab.diff;
  const scopeKey = useMemo(
    () => (diff ? reviewScopeKey({ cwd: diff.root, side: "all", commit: null, base: null }) : undefined),
    [diff],
  );

  const triggerComment = () => {
    const sel = lineSelection ?? { side: "modified", startLine: 1, endLine: 1 };
    setDraftRequest((prev) => ({
      nonce: (prev?.nonce ?? 0) + 1,
      side: sel.side,
      startLine: sel.startLine,
      endLine: sel.endLine,
    }));
  };

  return (
    <div className="docview" style={hidden ? { display: "none" } : undefined}>
      <div className="docview-head">
        <span className="title">{tab.title}</span>
        <span className="path" title={tab.path}>{tab.path}</span>
        <span className="changes-comparison docview-diff-comparison">{t("changes.compareAll")}</span>
        <button type="button" className="vlx-btn changes-add-comment-btn" onClick={triggerComment}>
          <Icons.plus size={12} aria-hidden="true" />
          <span>
            {lineSelection
              ? t("changes.review.addLine", formatLineRange(lineSelection.startLine, lineSelection.endLine))
              : t("changes.review.addFile")}
          </span>
        </button>
        <button type="button" className="vlx-btn changes-toggle" aria-pressed={options.ignoreWhitespace}
          onClick={() => setOption("ignoreWhitespace")}>{t("changes.ignoreWhitespace")}</button>
        <button type="button" className="vlx-btn changes-toggle" aria-pressed={options.collapseUnchanged}
          onClick={() => setOption("collapseUnchanged")}>{t("changes.collapseUnchanged")}</button>
        <button type="button" className="vlx-btn" onClick={() => refresh(tab.id)}>{t("changes.refresh")}</button>
        <button type="button" className="docview-save" onClick={() => openDocTab(tab.path)}>
          <Icons.file size={13} />{t("changes.openFile")}
        </button>
      </div>
      <div className="changes-stack">
        {diff && scopeKey && <DiffReviewBar scopeKey={scopeKey} cwd={diff.root} />}
        <div className="docview-diff-body" style={{ flex: 1, minHeight: 0 }}>
          {diff && (
            <FileDiffView
              cwd={diff.root}
              path={diff.path}
              tick={tab.reloadNonce}
              commit={null}
              base={null}
              side="all"
              options={options}
              reviewScope={scopeKey}
              draftRequest={draftRequest}
              onSelectionChange={setLineSelection}
            />
          )}
        </div>
      </div>
    </div>
  );
}
