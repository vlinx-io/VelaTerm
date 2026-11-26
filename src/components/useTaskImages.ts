//! Clipboard attachments for a launch draft, using the existing chat image format and paste handling.
import { useEffect, useRef, useState, type ClipboardEvent } from "react";
import { useT } from "../i18n";
import type { ChatImage } from "../ipc/chat";
import { env } from "../platform/env";
import {
  capturePasteData,
  insertPathsIntoTextField,
  isLocalDesktop,
  resolveClipboardFiles,
} from "../terminal/clipboardFiles";
import { imageFromNativeClipboard } from "../terminal/imageInput";
import { useTermStore } from "../store/termStore";
import { attachImages, attachmentRejectionNote, restoreAttachment, type Attachment } from "../layout/CenterPane/session/attachments";

export function useTaskImages(request: object | undefined, initial: ChatImage[] = []) {
  const t = useT();
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const current = useRef<Attachment[]>([]);
  const generation = useRef(0);
  const queue = useRef(Promise.resolve());
  const pending = useRef(0);
  useEffect(() => {
    generation.current += 1;
    current.current = initial.map(restoreAttachment);
    setAttachments(current.current); setNote(null); setBusy(false); pending.current = 0;
    return () => { generation.current += 1; };
    // A request owns its attachments. Background changes and role edits must not reset them.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request]);
  const enqueue = (load: () => Promise<File[]>) => {
    const epoch = generation.current;
    pending.current += 1; setBusy(true);
    queue.current = queue.current.then(async () => {
      if (epoch !== generation.current) return;
      const files = await load();
      if (epoch !== generation.current) return;
      const maxMb = useTermStore.getState().chatImageMaxMb;
      const result = await attachImages(current.current, files, { maxBytes: maxMb * 1024 * 1024 });
      if (epoch !== generation.current) return;
      current.current = result.attachments; setAttachments(result.attachments);
      setNote(attachmentRejectionNote(result.rejected, maxMb, t));
    }).catch(() => { if (epoch === generation.current) setNote(t("chat.attach.unreadable", "clipboard")); })
      .finally(() => { if (epoch === generation.current) { pending.current -= 1; setBusy(pending.current > 0); } });
  };
  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const field = event.currentTarget;
    const captured = capturePasteData(event.clipboardData);
    const local = isLocalDesktop();
    if (!captured.hasFileSignal && (captured.text || !env.isTauri)) return;
    if (!local && !captured.images.length) return;
    event.preventDefault();
    if (local) {
      enqueue(async () => {
        const resolved = await resolveClipboardFiles(captured.images, { loadMissingImages: true }).catch(() => null);
        if (resolved) {
          if (resolved.otherPaths.length && field.isConnected && !field.disabled) {
            insertPathsIntoTextField(field, resolved.otherPaths);
          }
          return resolved.imageFiles;
        }
        if (captured.images.length) return captured.images;
        if (!env.isTauri || captured.text) return [];
        return imageFromNativeClipboard().then(file => [file]).catch(() => []);
      });
    } else if (captured.images.length) {
      enqueue(() => Promise.resolve(captured.images));
    }
  };
  const remove = (id: string) => {
    if (busy) return;
    current.current = current.current.filter(image => image.id !== id);
    setAttachments(current.current); setNote(null);
  };
  return { attachments, note, busy, onPaste, remove };
}
