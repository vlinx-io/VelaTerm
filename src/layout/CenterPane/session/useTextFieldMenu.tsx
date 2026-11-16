import { useState, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import { ContextMenu } from "../../../components/ContextMenu";
import { IS_MAC } from "../../../hooks/shortcutRegistry";
import { useT } from "../../../i18n";
import { platform } from "../../../platform";
import { env } from "../../../platform/env";
import { imageFromNativeClipboard } from "../../../terminal/imageInput";

type TextField = HTMLTextAreaElement | HTMLInputElement;

// Only these types expose a text selection. Password fields stay out so the menu cannot copy a secret.
const TEXT_FIELD = "textarea, input:not([type]), input[type=text], input[type=search], input[type=url], input[type=tel]";

/** The primary-modifier chord, written the way each platform's own menus write it. */
const chord = (key: string) => IS_MAC ? `⌘ ${key}` : `Ctrl+${key}`;

/** Replace the field's selection through the editing command, which keeps undo and fires React's onChange. */
function replaceSelection(field: TextField, text: string) {
  field.focus({ preventScroll: true });
  if (document.execCommand?.(text ? "insertText" : "delete", false, text)) return;
  field.setRangeText(text, field.selectionStart ?? 0, field.selectionEnd ?? 0, "end");
  field.dispatchEvent(new Event("input", { bubbles: true }));
}

/**
 * Cut, Copy, Paste and Select All for text fields. The application cancels the system context menu
 * everywhere, so without this a field has no pointer route to its clipboard.
 *
 * `onImage` makes Paste accept a clipboard image, for fields that take attachments. An image wins over
 * text, as it does for the terminal's menu, because only that order avoids WKWebView's fallback paste prompt.
 */
export function useTextFieldMenu({ onImage, onCopyFailed }: { onImage?: (file: File) => void; onCopyFailed: (message: string) => void }) {
  const t = useT();
  const [menu, setMenu] = useState<{ x: number; y: number; host: Element; field: TextField; start: number; end: number; selected: string } | null>(null);
  const canReadClipboard = env.hasNativeHost || typeof navigator.clipboard?.readText === "function";
  const write = (text: string) => platform.clipboard.writeText(text, { reportFailure: true });
  const copyFailed = () => onCopyFailed(t("common.copyFailed"));
  const paste = async (field: TextField) => {
    if (onImage && env.isTauri) {
      const image = await imageFromNativeClipboard().catch(() => null);
      if (image) {
        onImage(image);
        return;
      }
    }
    const text = await platform.clipboard.readText();
    if (text && field.isConnected && !field.disabled) replaceSelection(field, text);
  };
  return {
    onContextMenu: (event: MouseEvent<HTMLElement>) => {
      const field = (event.target as Element).closest<TextField>(TEXT_FIELD);
      if (!field) return;
      event.preventDefault();
      // A field with its own menu sits inside the pane-wide one; only the innermost may open.
      event.stopPropagation();
      field.focus({ preventScroll: true });
      const start = field.selectionStart ?? 0;
      const end = field.selectionEnd ?? start;
      setMenu({ x: event.clientX, y: event.clientY, host: field.closest("dialog") || document.body, field, start, end, selected: field.value.slice(start, end) });
    },
    textFieldMenu: menu && createPortal(<ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)} items={[
      { label: t("common.cut"), shortcut: chord("X"), disabled: !menu.selected || menu.field.disabled || menu.field.readOnly, onClick: () => {
        const { field, start, end, selected } = menu;
        void write(selected).then(() => {
          // The write can outlast further typing; never delete text that has since changed.
          if (!field.isConnected || field.value.slice(start, end) !== selected) return;
          field.setSelectionRange(start, end);
          replaceSelection(field, "");
        }).catch(copyFailed);
      } },
      { label: t("common.copy"), shortcut: chord("C"), disabled: !menu.selected, onClick: () => void write(menu.selected).catch(copyFailed) },
      {
        label: t("term.paste"),
        shortcut: chord("V"),
        // Plain HTTP remote access forbids asynchronous clipboard reads; the keyboard shortcut still works there.
        disabled: !canReadClipboard || menu.field.disabled || menu.field.readOnly,
        // A stalled clipboard has already raised the global notice, and anything else reads as empty.
        onClick: () => void paste(menu.field).catch(() => {}),
      },
      { label: t("term.selectAll"), shortcut: chord("A"), disabled: !menu.field.value, onClick: () => { menu.field.focus({ preventScroll: true }); menu.field.select(); } },
    ]} />, menu.host),
  };
}
