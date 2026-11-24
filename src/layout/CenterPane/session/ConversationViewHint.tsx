import { useId, useLayoutEffect, useRef, useSyncExternalStore } from "react";
import Icons from "../../../components/Icons";
import { useT } from "../../../i18n";
import {
  dismissConversationViewHint,
  isConversationViewHintDismissed,
  subscribeConversationViewHint,
} from "../../../ipc/conversationViewHint";
import {
  isInitialSettingsPending,
  subscribeInitialSettingsPending,
} from "../../../store/settingsWatch";
import "./conversation-view-hint.css";

/** One-time guidance anchored to the view switch, without taking focus or moving the conversation. */
export function ConversationViewHint({ enabled, onSwitch }: { enabled: boolean; onSwitch: () => void }) {
  const t = useT();
  const hintId = useId();
  const anchorRef = useRef<HTMLSpanElement>(null);
  const hintRef = useRef<HTMLSpanElement>(null);
  const dismissed = useSyncExternalStore(
    subscribeConversationViewHint,
    isConversationViewHintDismissed,
    () => true,
  );
  const initialSettingsPending = useSyncExternalStore(
    subscribeInitialSettingsPending,
    isInitialSettingsPending,
    () => false,
  );
  const visible = enabled && !dismissed && !initialSettingsPending;

  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    const hint = hintRef.current;
    const pane = anchor?.closest(".pane");
    if (!visible || !anchor || !hint || !pane) return;
    const position = () => {
      const bounds = pane.getBoundingClientRect();
      const target = anchor.getBoundingClientRect();
      hint.style.maxWidth = `${Math.max(0, Math.min(320, bounds.width - 24))}px`;
      const width = hint.getBoundingClientRect().width;
      const center = target.left + target.width / 2;
      const left = Math.max(bounds.left + 12, Math.min(center - width / 2, bounds.right - width - 12));
      hint.style.left = `${left - target.left}px`;
      hint.style.setProperty("--hint-arrow-left", `${center - left - 5}px`);
    };
    position();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(position);
    observer?.observe(pane);
    observer?.observe(hint);
    window.addEventListener("resize", position);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", position);
    };
  }, [visible]);

  return (
    <span className="sv-view-switch" ref={anchorRef}>
      <button type="button" title={t("session.showTerminal")} aria-label={t("session.showTerminal")}
        aria-describedby={visible ? hintId : undefined}
        onClick={() => { dismissConversationViewHint(); onSwitch(); }}>
        <Icons.terminal size={14} />
      </button>
      {visible && <span className="sv-view-hint" ref={hintRef}>
        <span id={hintId} role="note">{t("session.terminalViewHint")}</span>
        <button type="button" className="sv-view-hint-close" title={t("common.close")}
          aria-label={t("common.close")} onClick={dismissConversationViewHint}>
          <Icons.close size={12} />
        </button>
      </span>}
    </span>
  );
}
