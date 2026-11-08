import { useT } from "../../i18n";
import { useCompletionTip } from "./tip";

/** Names the keys for the dimmed history hint in the pane header, only while that hint is on screen. */
export function CompletionTipLabel({ sessionId }: { sessionId: string }) {
  const t = useT();
  const tip = useCompletionTip(sessionId);
  if (!tip) return null;
  return (
    <span className="completion-tip">
      {tip === "accept-arrow" && <>
        <kbd className="hint-kbd">→</kbd>
        <span>{t("term.completionTipAccept")}</span>
        <kbd className="hint-kbd">Tab</kbd>
        <span>{t("term.completionTipComplete")}</span>
      </>}
      {tip === "accept-tab" && <>
        <kbd className="hint-kbd">Tab</kbd>
        <span>{t("term.completionTipAccept")}</span>
      </>}
      {tip === "undo" && <>
        <kbd className="hint-kbd">Tab</kbd>
        <span>{t("term.completionTipUndo")}</span>
      </>}
    </span>
  );
}
