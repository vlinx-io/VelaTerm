//! A file shown in the panel beside a conversation (`vartifact` on anything but an HTML page): the same
//! document view `vopen` opens as a tab, hosted in the panel instead.
//!
//! The document lives in the store's `dockDocs` while the panel shows it, so the view's save, mode and close
//! actions work exactly as they do for a tab. Closing a document with unsaved edits asks first, inside the view.

import { lazy, Suspense, useEffect, useRef, useState } from "react";

import Icons from "../../../components/Icons";
import { useT } from "../../../i18n";
import { useTermStore } from "../../../store/termStore";
import "./artifact/artifact.css";

const DocView = lazy(() => import("./DocView").then((m) => ({ default: m.DocView })));

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/** `reloadKey` changes when the same file is opened again, which re-reads it like `vopen` does for a tab. */
export function DockDocView({ path, reloadKey, onClose, style }: {
  path: string;
  reloadKey: number;
  onClose: () => void;
  style?: React.CSSProperties;
}) {
  const t = useT();
  const [id, setId] = useState<string | null>(null);
  const tab = useTermStore((s) => (id ? s.dockDocs[id] : undefined));

  useEffect(() => {
    const store = useTermStore.getState();
    const created = store.openDockDoc(path);
    setId(created);
    return () => useTermStore.getState().closeDockDoc(created);
  }, [path]);

  // Closed from inside the view (its close or discard actions): close the panel with it. Only a document that
  // was shown counts, so the moment between creating it and seeing it in the store closes nothing.
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const shown = useRef<string | null>(null);
  useEffect(() => {
    if (tab) shown.current = tab.id;
    else if (id && shown.current === id) onCloseRef.current();
  }, [id, tab]);

  const firstKey = useRef(reloadKey);
  useEffect(() => {
    if (id && reloadKey !== firstKey.current) useTermStore.getState().refreshDocTab(id);
  }, [id, reloadKey]);

  return (
    <div className="artifact-dock" style={style}>
      <div className="artifact-dock-head">
        <span className="artifact-dock-title" title={path}>
          {baseName(path)}
        </span>
        <span className="grow" />
        <button className="artifact-icon-btn" title={t("artifact.openAsTab")} aria-label={t("artifact.openAsTab")}
          onClick={() => useTermStore.getState().openDocTab(path)}>
          <Icons.external size={14} />
        </button>
        <button className="artifact-icon-btn" title={t("artifact.closePanel")} aria-label={t("artifact.closePanel")}
          onClick={() => (id ? useTermStore.getState().requestCloseDocTab(id) : onClose())}>
          <Icons.close size={14} />
        </button>
      </div>
      <div className="dock-doc-body" data-dock-doc-id={id ?? undefined}>
        {tab && (
          <Suspense fallback={null}>
            <DocView tab={tab} hidden={false} />
          </Suspense>
        )}
      </div>
    </div>
  );
}
