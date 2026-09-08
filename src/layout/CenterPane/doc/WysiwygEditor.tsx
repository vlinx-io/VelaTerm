//! Thin wrapper around the Milkdown Crepe WYSIWYG Markdown editor: lifecycle, defaultValue,
//! markdownUpdated-to-onEdited forwarding, and getMarkdown access.
//!
//! Content is snapshot-based: defaultValue is supplied once on mount and the editor owns it afterward.
//! DocView changes the component key for reloads or mode switches rather than setting content in place.

import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
} from "react";
import { Crepe } from "@milkdown/crepe";
import "@milkdown/crepe/theme/common/style.css";
import { editorViewCtx } from "@milkdown/kit/core";
import { $prose } from "@milkdown/kit/utils";
import type { EditorView } from "@milkdown/kit/prose/view";
import { t } from "../../../i18n";
import { EMPTY_STATUS, type DocSearchControl } from "./docSearch";
import {
  pmApply,
  pmClear,
  pmNext,
  pmPrev,
  pmReplace,
  pmReplaceAll,
  pmSearchPlugin,
} from "./pmSearch";
import { pmMermaidPlugin } from "./pmMermaid";
import { uploadDocImage } from "../../../ipc/transport";
import { loadFileBlob } from "../../../ipc/info";
import { extOf } from "../../../terminal/imageInput";
import { stripImageRatioAlt } from "./docImage";

/** Map extensions to MIME types so object URLs render formats such as SVG correctly. */
const IMG_MIME: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  bmp: "image/bmp",
  svg: "image/svg+xml",
  tiff: "image/tiff",
};
function mimeFromPath(p: string): string {
  const ext = p.split(".").pop()?.toLowerCase() ?? "";
  return IMG_MIME[ext] ?? "application/octet-stream";
}

/** Reuse object URLs by absolute path to avoid repeated local reads and memory leaks. */
const docImgUrlCache = new Map<string, string>();

/** Save pasted/dropped images under a sibling assets directory and return the Markdown-relative path. */
async function onUploadDocImage(file: File, docPath: string): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  return uploadDocImage(bytes, extOf(file), docPath);
}

/**
 * Convert Markdown image sources into WebView-displayable URLs:
 * - Preserve HTTP, HTTPS, data, and blob sources.
 * - Resolve local relative paths against the document, then use loadFileBlob to build object URLs.
 *   This reuses ImageDocView's chunked I/O and avoids changing Tauri asset-protocol security settings.
 *
 * On read failure, return the original source for Crepe's placeholder/failure handling without throwing.
 */
async function proxyDocImageURL(src: string, docPath: string): Promise<string> {
  if (/^(https?:|data:|blob:)/i.test(src)) return src;
  const docDir = docPath.replace(/[/\\][^/\\]*$/, "");
  const isAbs = src.startsWith("/") || /^[a-zA-Z]:[/\\]/.test(src);
  const abs = isAbs ? src : docDir ? `${docDir}/${src}` : src;
  const cached = docImgUrlCache.get(abs);
  if (cached) return cached;
  try {
    const res = await loadFileBlob(abs);
    if (!res) return src;
    const url = URL.createObjectURL(
      new Blob([res.blob], { type: mimeFromPath(abs) }),
    );
    docImgUrlCache.set(abs, url);
    return url;
  } catch {
    return src;
  }
}

/** Localize Crepe's slash menu, placeholders, and link/image/code widgets. Capture the current locale
 * at mount; DocView rebuilds the editor by key instead of hot-switching it. docPath locates pasted
 * image storage and resolves rendering paths. */
const crepeFeatureConfigs = (docPath: string, textOnly: boolean) => ({
  [Crepe.Feature.Placeholder]: {
    text: t("crepe.placeholder"),
    mode: "block" as const,
  },
  [Crepe.Feature.BlockEdit]: {
    textGroup: {
      label: t("crepe.textGroup"),
      text: { label: t("crepe.paragraph") },
      h1: { label: t("crepe.h1") },
      h2: { label: t("crepe.h2") },
      h3: { label: t("crepe.h3") },
      h4: { label: t("crepe.h4") },
      h5: { label: t("crepe.h5") },
      h6: { label: t("crepe.h6") },
      quote: { label: t("crepe.quote") },
      divider: { label: t("crepe.divider") },
    },
    listGroup: {
      label: t("crepe.listGroup"),
      bulletList: { label: t("crepe.bulletList") },
      orderedList: { label: t("crepe.orderedList") },
      taskList: { label: t("crepe.taskList") },
    },
    advancedGroup: {
      label: t("crepe.advancedGroup"),
      image: textOnly ? null : { label: t("crepe.image") },
      codeBlock: { label: t("crepe.codeBlock") },
      table: { label: t("crepe.table") },
      math: { label: t("crepe.math") },
    },
  },
  [Crepe.Feature.LinkTooltip]: {
    inputPlaceholder: t("crepe.linkPlaceholder"),
  },
  [Crepe.Feature.ImageBlock]: {
    inlineUploadButton: t("crepe.upload"),
    inlineUploadPlaceholderText: t("crepe.orPasteImageLink"),
    blockUploadButton: t("crepe.uploadImage"),
    blockUploadPlaceholderText: t("crepe.orPasteImageLink"),
    blockCaptionPlaceholderText: t("crepe.imageCaption"),
    blockConfirmButton: t("crepe.confirm"),
    // Top-level onUpload/proxyDomURL covers both inline and block images through Crepe's fallback.
    onUpload: (file: File) => textOnly ? Promise.reject(new Error(t("memory.invalid"))) : onUploadDocImage(file, docPath),
    proxyDomURL: (url: string) => textOnly ? "data:image/gif;base64,R0lGODlhAQABAAD/ACwAAAAAAQABAAACADs=" : proxyDocImageURL(url, docPath),
  },
  [Crepe.Feature.CodeMirror]: {
    searchPlaceholder: t("crepe.searchLanguage"),
    noResultText: t("crepe.noResult"),
    copyText: t("common.copy"),
    previewToggleText: (previewOnly: boolean) =>
      previewOnly ? t("crepe.edit") : t("crepe.collapse"),
  },
});

export interface WysiwygHandle {
  /** Return the current Markdown serialization, or null before the editor is ready. */
  getMarkdown: () => string | null;
  /** Return current HTML for printing/PDF export, or null before the editor is ready. */
  getHtml: () => string | null;
  /** Shared find/replace controls, effective after the ProseMirror view is ready. */
  search: DocSearchControl;
}

export const WysiwygEditor = forwardRef<
  WysiwygHandle,
  {
    defaultValue: string;
    /** Current absolute document path, empty for drafts; used for sibling assets and relative images. */
    docPath: string;
    /** Called after genuine user edits so DocView can mark dirty and decide persistence. */
    onEdited: () => void;
    /** Prevent document image reads and uploads for text-only knowledge entries. */
    textOnly?: boolean;
  }
>(function WysiwygEditor({ defaultValue, docPath, onEdited, textOnly = false }, ref) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const crepeRef = useRef<Crepe | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  // markdownUpdated before create() completes is parser/normalization initialization noise, not user editing.
  const readyRef = useRef(false);
  const onEditedRef = useRef(onEdited);
  onEditedRef.current = onEdited;

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const crepe = new Crepe({
      root,
      defaultValue,
      featureConfigs: crepeFeatureConfigs(docPath, textOnly),
    });
    // Register find/replace before create(), the stable Milkdown pattern versus mutating state afterward.
    crepe.editor.use($prose(() => pmSearchPlugin()));
    // Mermaid plugin renders decorations below fenced blocks without changing document serialization.
    crepe.editor.use($prose(() => pmMermaidPlugin()));
    crepe.on((l) => {
      l.markdownUpdated((_ctx, md, prevMd) => {
        if (!readyRef.current || md === prevMd) return;
        // Focus guard: Crepe may asynchronously normalize code-block languages or lists after create().
        // Those updates previously marked newly opened documents dirty and blocked external reloads.
        // Genuine user edits necessarily occur while focus is inside the editor.
        if (!root.contains(document.activeElement)) return;
        onEditedRef.current();
      });
    });
    crepeRef.current = crepe;
    let disposed = false;
    void crepe.create().then(() => {
      if (disposed) return;
      readyRef.current = true;
      // Retain the active view for search controls; the plugin was registered before create().
      crepe.editor.action((ctx) => {
        viewRef.current = ctx.get(editorViewCtx);
      });
    });
    return () => {
      disposed = true;
      readyRef.current = false;
      viewRef.current = null;
      crepeRef.current = null;
      // Guard destruction because occasional ProseMirror teardown races must not interrupt React unmount.
      try {
        void crepe.destroy();
      } catch {
        /* Defensive fallback. */
      }
    };
    // Create once per mount; the parent changes key for content or mode changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useImperativeHandle(ref, () => ({
    getMarkdown: () => {
      const crepe = crepeRef.current;
      if (!crepe || !readyRef.current) return null;
      try {
        return stripImageRatioAlt(crepe.getMarkdown());
      } catch {
        return null;
      }
    },
    getHtml: () => {
      if (!viewRef.current || !readyRef.current) return null;
      // The `.ProseMirror` editor node's innerHTML is the rendered HTML.
      return viewRef.current.dom.innerHTML;
    },
    search: {
      apply: (o) =>
        viewRef.current ? pmApply(viewRef.current, o.query, o.caseSensitive) : EMPTY_STATUS,
      next: () => (viewRef.current ? pmNext(viewRef.current) : EMPTY_STATUS),
      prev: () => (viewRef.current ? pmPrev(viewRef.current) : EMPTY_STATUS),
      replace: (txt) => (viewRef.current ? pmReplace(viewRef.current, txt) : EMPTY_STATUS),
      replaceAll: (txt) => (viewRef.current ? pmReplaceAll(viewRef.current, txt) : EMPTY_STATUS),
      clear: () => {
        if (viewRef.current) pmClear(viewRef.current);
      },
    },
  }));

  return <div className="docview-wysiwyg" ref={rootRef} />;
});
