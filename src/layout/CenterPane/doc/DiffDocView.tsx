//! Diff document tab: a file's uncommitted changes against HEAD, read-only, in the same viewer the Changes
//! dialog uses. The tab's refresh action and a repeated open bump reloadNonce, which reloads the diff.

import { useState } from "react";
import { FileDiffView, readDiffOptions, writeDiffOptions, type DiffOptions } from "../../../components/diff/FileDiffView";
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
  const setOption = (key: keyof DiffOptions) => setOptionsState((current) => {
    const next = { ...current, [key]: !current[key] };
    writeDiffOptions(next);
    return next;
  });
  const diff = tab.diff;
  return (
    <div className="docview" style={hidden ? { display: "none" } : undefined}>
      <div className="docview-head">
        <span className="title">{tab.title}</span>
        <span className="path" title={tab.path}>{tab.path}</span>
        <span className="changes-comparison docview-diff-comparison">{t("changes.compareAll")}</span>
        <button type="button" className="vlx-btn changes-toggle" aria-pressed={options.ignoreWhitespace}
          onClick={() => setOption("ignoreWhitespace")}>{t("changes.ignoreWhitespace")}</button>
        <button type="button" className="vlx-btn changes-toggle" aria-pressed={options.collapseUnchanged}
          onClick={() => setOption("collapseUnchanged")}>{t("changes.collapseUnchanged")}</button>
        <button type="button" className="vlx-btn" onClick={() => refresh(tab.id)}>{t("changes.refresh")}</button>
        <button type="button" className="docview-save" onClick={() => openDocTab(tab.path)}>
          <Icons.file size={13} />{t("changes.openFile")}
        </button>
      </div>
      <div className="docview-diff-body">
        {diff && <FileDiffView cwd={diff.root} path={diff.path} tick={tab.reloadNonce} commit={null} base={null}
          side="all" options={options} />}
      </div>
    </div>
  );
}
