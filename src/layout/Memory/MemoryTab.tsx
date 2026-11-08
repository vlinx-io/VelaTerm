import { useEffect, useRef, useState } from "react";
import { ContextMenu } from "../../components/ContextMenu";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { MemoryIcon } from "./MemoryIcon";
import { memoryNavigate, memoryUrl, useMemoryLocation } from "./navigation";

/** Keep the opened library tab available when another tab is selected. The tab bar passes the other open
 * tabs' count and a closer so the right-click menu offers the same close actions as every other tab. */
export function MemoryTab({ otherTabCount = 0, onCloseOthers }: { otherTabCount?: number; onCloseOthers?: () => void } = {}) {
  const t = useT();
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const location = useMemoryLocation();
  const route = new URLSearchParams(location).get("memory") ?? "";
  const active = !!route;
  const [savedUrl, setSavedUrl] = useState<string | null>(() => active ? window.location.href : null);
  const pendingClose = useRef(false);
  useEffect(() => {
    const resetClose = () => { pendingClose.current = false; };
    window.addEventListener("memory:beforeNavigate", resetClose);
    return () => window.removeEventListener("memory:beforeNavigate", resetClose);
  }, []);
  useEffect(() => {
    if (active) setSavedUrl(window.location.href);
    else if (pendingClose.current) {
      pendingClose.current = false;
      setSavedUrl(null);
    }
  }, [active, location]);
  if (!active && !savedUrl) return null;
  // Archived sessions keep their own identity in the tab, whichever entry opened them.
  const shown = active ? route : new URL(savedUrl!, window.location.href).searchParams.get("memory") ?? "";
  const archive = shown === "collections" || shown.startsWith("collection/");
  const close = () => {
    if (!active) { setSavedUrl(null); return; }
    memoryNavigate(memoryUrl(""));
    if (!new URLSearchParams(window.location.search).get("memory")) setSavedUrl(null);
    else pendingClose.current = true;
  };
  const hasOthers = otherTabCount > 0 && !!onCloseOthers;
  // The menu renders beside the tab, not inside it, so right-clicking its backdrop cannot bubble into the
  // tab's own handler and reopen it.
  return <><div className={`tab memory-open-tab${active ? " on" : ""}`} onContextMenu={(event) => {
    event.preventDefault();
    setMenu({ x: event.clientX, y: event.clientY });
  }}>
    <a className="memory-tab-link" href={active ? window.location.href : savedUrl!} aria-current={active ? "page" : undefined} onClick={(event) => {
      if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      if (!active && savedUrl) memoryNavigate(savedUrl);
    }}>
      {archive ? <Icons.archive size={16} /> : <MemoryIcon size={16} />}<span className="tnm">{t(archive ? "memory.collections" : "memory.title")}</span>
    </a>
    <button type="button" className="x" onClick={close} title={t("common.close")} aria-label={t("common.close")}><Icons.x size={12} /></button>
  </div>
    {menu && <ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)} items={[
      { label: t("doc.closeTab"), onClick: close },
      { label: t("tab.closeOthers"), disabled: !hasOthers, onClick: () => onCloseOthers?.() },
      // The library tab always sits after the regular tabs, so nothing is ever to its right.
      { label: t("tab.closeRight"), disabled: true, onClick: () => {} },
      { label: t("tab.closeAll"), onClick: () => { onCloseOthers?.(); close(); } },
    ]} />}
  </>;
}
