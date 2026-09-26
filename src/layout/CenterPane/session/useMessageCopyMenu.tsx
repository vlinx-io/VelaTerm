import { useEffect, useState, type ClipboardEvent, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import { ContextMenu } from "../../../components/ContextMenu";
import { useT } from "../../../i18n";
import { platform } from "../../../platform";
import { copyAttributes, createMessageSelectionClipboardContent, type MessageClipboardContent } from "./selectionCopy";

/** Copy message content from its Markdown semantics instead of the layout DOM's block separators. */
export function handleMessageCopy(event: ClipboardEvent<HTMLElement>) {
  const content = createMessageSelectionClipboardContent(window.getSelection(), event.currentTarget);
  if (!content) return;
  event.preventDefault();
  event.clipboardData.setData("text/plain", content.plainText);
  event.clipboardData.setData("text/html", content.html);
}

/** Right-click menu for a selection inside one message body: copy as displayed, or as Markdown source. */
export function useMessageCopyMenu() {
  const t = useT();
  const [menu, setMenu] = useState<{ x: number; y: number; host: Element; content: MessageClipboardContent } | null>(null);
  const [error, setError] = useState(false);
  useEffect(() => {
    if (!menu) return;
    const dismiss = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        setMenu(null);
      }
    };
    document.addEventListener("keydown", dismiss, true);
    return () => document.removeEventListener("keydown", dismiss, true);
  }, [menu]);
  const write = (text: string) => {
    setError(false);
    void platform.clipboard.writeText(text, { reportFailure: true }).catch(() => setError(true));
  };
  return {
    onContextMenu: (event: MouseEvent<HTMLElement>) => {
      // Text fields keep their own editing behavior.
      if ((event.target as Element).closest("textarea, input")) return;
      const content = createMessageSelectionClipboardContent(window.getSelection(), event.currentTarget);
      if (!content) return;
      event.preventDefault();
      setMenu({ x: event.clientX, y: event.clientY, host: event.currentTarget.closest("dialog") || document.body, content });
    },
    copyMenu: <>
      {menu && createPortal(<ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)} items={[
        // The menu keeps the DOM selection, so a native copy reaches `handleMessageCopy` with both flavors.
        { label: t("common.copy"), onClick: () => {
          setError(false);
          if (!document.execCommand("copy")) write(menu.content.plainText);
        } },
        { label: t("chat.copyAsMarkdown"), onClick: () => write(menu.content.markdown) },
      ]} />, menu.host)}
      {error && <span role="alert" {...copyAttributes.ignore}>{t("common.copyFailed")}</span>}
    </>,
  };
}
