import { useEffect, useRef, useState } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { MemoryIcon } from "./MemoryRoute";
import { memoryNavigate, memoryUrl, useMemoryLocation } from "./navigation";

/** Keep the opened library tab available when another tab is selected. */
export function MemoryTab() {
  const t = useT();
  const location = useMemoryLocation();
  const active = !!new URLSearchParams(location).get("memory");
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
  const close = () => {
    if (!active) { setSavedUrl(null); return; }
    memoryNavigate(memoryUrl(""));
    if (!new URLSearchParams(window.location.search).get("memory")) setSavedUrl(null);
    else pendingClose.current = true;
  };
  return <div className={`tab memory-open-tab${active ? " on" : ""}`}>
    <a className="memory-tab-link" href={active ? window.location.href : savedUrl!} aria-current={active ? "page" : undefined} onClick={(event) => {
      if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      if (!active && savedUrl) memoryNavigate(savedUrl);
    }}>
      <MemoryIcon size={16} /><span className="tnm">{t("memory.title")}</span>
    </a>
    <button type="button" className="x" onClick={close} title={t("common.close")} aria-label={t("common.close")}><Icons.x size={12} /></button>
  </div>;
}
