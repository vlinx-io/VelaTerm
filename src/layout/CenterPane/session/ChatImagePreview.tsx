import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { useSuspendNativeViews } from "../../../hooks/nativeViewSuspend";
import { useBackdropDismiss } from "../../../hooks/useBackdropDismiss";
import { useImageMenu } from "./useImageMenu";
import { useT } from "../../../i18n";

/** Display the attachment's original bytes outside the transcript's clipped, virtualized layout. */
export function ChatImagePreview({ src, alt, onClose }: { src: string; alt: string; onClose: () => void }) {
  const t = useT();
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [fit, setFit] = useState(true);
  const [failed, setFailed] = useState(false);
  const { onContextMenu, imageMenu } = useImageMenu(src, () => setFit(false));
  const backdrop = useBackdropDismiss(onClose);
  useSuspendNativeViews();

  useEffect(() => {
    const dialog = dialogRef.current;
    const previousFocus = document.activeElement;
    dialog?.showModal();
    return () => {
      dialog?.close();
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus();
    };
  }, []);

  return createPortal(
    <dialog
      ref={dialogRef}
      className="sv-image-preview"
      aria-label={alt || t("crepe.image")}
      {...backdrop}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onKeyDown={(event) => event.stopPropagation()}
    >
      <div className="sv-image-preview-panel">
        <div className="sv-image-preview-toolbar">
          <button type="button" aria-pressed={fit} onClick={() => setFit(true)}>
            {t("doc.imgFit")}
          </button>
          <button type="button" aria-pressed={!fit} onClick={() => setFit(false)}>
            {t("doc.imgActual")}
          </button>
          <button type="button" className="sv-image-preview-close" onClick={onClose} autoFocus>
            {t("common.close")}
          </button>
        </div>
        <div className={`sv-image-preview-body${fit ? " is-fit" : ""}`}>
          {failed ? (
            <span role="alert">{t("doc.imgDecodeFailed")}</span>
          ) : (
            <img src={src} alt={alt} onContextMenu={onContextMenu} onError={() => setFailed(true)} />
          )}
        </div>
      </div>
      {imageMenu}
    </dialog>,
    document.body,
  );
}
