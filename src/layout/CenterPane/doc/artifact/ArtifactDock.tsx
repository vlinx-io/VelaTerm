//! The artifact panel beside a conversation: the page the agent is working on, live, with its saved versions.
//!
//! Stepping to an earlier version previews that snapshot without touching the file; restoring writes it back
//! after a confirmation, and the restore itself becomes the newest version through the usual change watch.

import { useEffect, useState } from "react";

import Icons from "../../../../components/Icons";
import { dateLocale, useT } from "../../../../i18n";
import { artifactRestore } from "../../../../ipc/artifact";
import { useTermStore } from "../../../../store/termStore";
import { ArtifactHeaderControls, ArtifactPane } from "./ArtifactParts";
import { useArtifact } from "./useArtifact";
import "./artifact.css";

/** Version chips shown in the header; older ones stay in history but off the bar. */
const VISIBLE_VERSIONS = 6;

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/** "14:22" for today, "Oct 17, 14:22" otherwise, in the interface language. */
function savedAt(ms: number): string {
  const d = new Date(ms);
  const today = new Date().toDateString() === d.toDateString();
  const opts: Intl.DateTimeFormatOptions = today
    ? { hour: "2-digit", minute: "2-digit" }
    : { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" };
  return new Intl.DateTimeFormat(dateLocale(), opts).format(d);
}

export function ArtifactDock({ path, onClose, style }: { path: string; onClose: () => void; style?: React.CSSProperties }) {
  const t = useT();
  const [version, setVersion] = useState<number | null>(null);
  const [confirmRestore, setConfirmRestore] = useState(false);
  const [restoreFailed, setRestoreFailed] = useState(false);
  const [consoleOpen, setConsoleOpen] = useState(false);
  const state = useArtifact(path, { hidden: false, version });

  useEffect(() => {
    setVersion(null);
    setConfirmRestore(false);
    setRestoreFailed(false);
  }, [path]);

  const latest = state.versions.at(-1)?.version ?? null;
  const viewed = version === null ? null : state.versions.find((v) => v.version === version) ?? null;
  const pick = (v: number) => {
    setConfirmRestore(false);
    setRestoreFailed(false);
    setVersion(v === latest ? null : v);
  };

  const openTab = (mode?: "source") => {
    const store = useTermStore.getState();
    store.openDocTab(path);
    if (!mode) return;
    const tab = Object.values(useTermStore.getState().docTabs).find((d) => d.path === path && d.kind !== "diff" && d.kind !== "changes");
    if (tab) store.setDocTabMode(tab.id, mode);
  };

  const restore = async (v: number) => {
    try {
      await artifactRestore(path, v);
      setConfirmRestore(false);
      setVersion(null);
      state.reload();
    } catch {
      setRestoreFailed(true);
    }
  };

  return (
    <div className="artifact-dock" style={style}>
      <div className="artifact-dock-head">
        <span className="artifact-dock-title" title={path}>
          {baseName(path)}
        </span>
        <ArtifactHeaderControls state={state} path={path} consoleOpen={consoleOpen} onToggleConsole={() => setConsoleOpen((o) => !o)} compact />
        <span className="grow" />
        {state.versions.length > 1 && (
          <div className="artifact-versions" role="group">
            {state.versions.slice(-VISIBLE_VERSIONS).map((v) => (
              <button
                key={v.version}
                className={"artifact-version" + ((version ?? latest) === v.version ? " on" : "")}
                aria-pressed={(version ?? latest) === v.version}
                title={t("artifact.versionTitle", v.version, savedAt(v.mtimeMs))}
                onClick={() => pick(v.version)}
              >
                v{v.version}
              </button>
            ))}
          </div>
        )}
        <button className="artifact-icon-btn" title={t("artifact.source")} aria-label={t("artifact.source")} onClick={() => openTab("source")}>
          <Icons.code size={14} />
        </button>
        <button className="artifact-icon-btn" title={t("artifact.openAsTab")} aria-label={t("artifact.openAsTab")} onClick={() => openTab()}>
          <Icons.external size={14} />
        </button>
        <button className="artifact-icon-btn" title={t("artifact.closePanel")} aria-label={t("artifact.closePanel")} onClick={onClose}>
          <Icons.close size={14} />
        </button>
      </div>
      {viewed && (
        <div className="docview-banner" role="status">
          <span style={{ flex: 1 }}>
            {restoreFailed
              ? t("artifact.restoreFailed")
              : confirmRestore
                ? t("artifact.restoreBody", viewed.version)
                : t("artifact.viewingVersion", viewed.version, savedAt(viewed.mtimeMs))}
          </span>
          {confirmRestore ? (
            <>
              <button onClick={() => void restore(viewed.version)}>{t("artifact.restore")}</button>
              <button onClick={() => setConfirmRestore(false)}>{t("common.cancel")}</button>
            </>
          ) : (
            <>
              <button onClick={() => setConfirmRestore(true)}>
                <Icons.undo size={12} /> {t("artifact.restoreVersion")}
              </button>
              <button onClick={() => setVersion(null)}>{t("artifact.backToLatest")}</button>
            </>
          )}
        </div>
      )}
      <ArtifactPane
        state={state}
        path={path}
        title={baseName(path)}
        consoleOpen={consoleOpen}
        onCloseConsole={() => setConsoleOpen(false)}
        onCloseTab={onClose}
      />
    </div>
  );
}
