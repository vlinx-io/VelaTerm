import { useEffect, useState, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import { ContextMenu } from "../../../components/ContextMenu";
import { useT } from "../../../i18n";

/** Keep the menu in the preview's top layer when an image is inside a modal dialog. */
export function useImageMenu(src: string, onPreview?: () => void) {
  const t = useT();
  const [menu, setMenu] = useState<{ x: number; y: number; host: Element } | null>(null);
  const [error, setError] = useState(false);
  useEffect(() => { setMenu(null); setError(false); }, [src]);
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
  const run = (action: () => Promise<void>) => {
    setError(false);
    void action().catch(() => setError(true));
  };
  return {
    onContextMenu: (event: MouseEvent<HTMLImageElement>) => {
      event.preventDefault();
      event.stopPropagation();
      setMenu({ x: event.clientX, y: event.clientY, host: event.currentTarget.closest("dialog") || document.body });
    },
    imageMenu: <>
      {menu && createPortal(<ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)} items={[
        ...(onPreview ? [{ label: t("chat.imageViewOriginal"), onClick: onPreview }] : []),
        { label: t("chat.imageCopy"), onClick: () => run(async () => (await import("../../../platform/imageActions")).copyImage(src)) },
        { label: t("chat.imageSave"), onClick: () => run(async () => (await import("../../../platform/imageActions")).saveImage(src)) },
      ]} />, menu.host)}
      {error && <span role="alert">{t("chat.imageActionFailed")}</span>}
    </>,
  };
}
