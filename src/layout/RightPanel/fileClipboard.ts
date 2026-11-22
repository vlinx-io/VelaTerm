//! File clipboard store and path selection helpers for the right-panel Files tree.
//!
//! Desktop environments (Windows CF_HDROP, macOS NSPasteboard) synchronize file copies and cuts
//! with the operating system clipboard so users can copy/paste between VelaTerm and File Explorer
//! or Finder. Browser remote access and platforms without a native file clipboard fall back to an
//! in-app clipboard kept at module scope so cut dimming survives switching inspector tabs.

import {
  clipboardClearFiles,
  clipboardFileState,
  clipboardReadFiles,
  clipboardWriteFiles,
  type FileClipboardContent,
  type FileClipboardMode,
} from "../../ipc/info";
import { env } from "../../platform";
import { detectFlavor, parentPath, samePath, splitPath } from "../../remote/serverPath";

export interface FileClipboardSnapshot {
  cutPaths: string[];
  canPaste: boolean;
}

interface StoreState {
  cutPaths: string[];
  cutSequence: number | null;
  fallback: { paths: string[]; mode: FileClipboardMode } | null;
  nativeSupported: boolean | null;
  nativeHasFiles: boolean;
}

const listeners = new Set<() => void>();
let state: StoreState = {
  cutPaths: [],
  cutSequence: null,
  fallback: null,
  nativeSupported: null,
  nativeHasFiles: false,
};
let snapshot: FileClipboardSnapshot = { cutPaths: [], canPaste: false };

function emit(): void {
  const canPaste = state.nativeSupported === true
    ? state.nativeHasFiles
    : !!(state.fallback && state.fallback.paths.length > 0);
  snapshot = { cutPaths: state.cutPaths, canPaste };
  listeners.forEach((cb) => cb());
}

export function subscribeFileClipboard(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

export function getFileClipboardSnapshot(): FileClipboardSnapshot {
  return snapshot;
}

/** Reset module-level clipboard state; intended for unit tests only. */
export function resetFileClipboardForTest(): void {
  state = {
    cutPaths: [],
    cutSequence: null,
    fallback: null,
    nativeSupported: null,
    nativeHasFiles: false,
  };
  emit();
}

/** Parent directory path respecting both POSIX `/` and Windows `\` separators. */
export function parentPathOf(path: string): string {
  return parentPath(path, detectFlavor(path));
}

/** Whether two paths refer to the same filesystem location under the path's OS flavor. */
export function sameFilePath(a: string, b: string): boolean {
  return samePath(a, b, detectFlavor(a));
}

/** Whether `candidate` matches any path in `list`. */
export function isPathInList(list: string[], candidate: string): boolean {
  if (!list.length) return false;
  const flavor = detectFlavor(candidate);
  return list.some((p) => samePath(p, candidate, flavor));
}

function isStrictAncestor(ancestor: string, descendant: string): boolean {
  const flavor = detectFlavor(ancestor);
  const a = splitPath(ancestor, flavor);
  const b = splitPath(descendant, flavor);
  const norm = (s: string) => (flavor === "win" ? s.toLowerCase() : s);
  if (norm(a.root) !== norm(b.root) || a.parts.length >= b.parts.length) return false;
  return a.parts.every((part, idx) => norm(part) === norm(b.parts[idx]));
}

/**
 * Deduplicate paths and remove any entry whose ancestor folder is also selected so bulk copy, cut,
 * or delete does not operate twice on nested items.
 */
export function normalizeSourcePaths(paths: string[]): string[] {
  const unique: string[] = [];
  for (const path of paths) {
    if (!path) continue;
    const flavor = detectFlavor(path);
    if (!unique.some((existing) => samePath(existing, path, flavor))) {
      unique.push(path);
    }
  }
  return unique.filter(
    (candidate) => !unique.some((other) => other !== candidate && isStrictAncestor(other, candidate)),
  );
}

/** Inclusive display-order slice between `anchor` and `target`. */
export function rangeBetweenPaths(order: string[], anchor: string, target: string): string[] {
  const a = order.indexOf(anchor);
  const b = order.indexOf(target);
  if (a < 0 || b < 0) return [target];
  return order.slice(Math.min(a, b), Math.max(a, b) + 1);
}

/** Resolve paste destination directory: the directory itself, or a file's containing directory. */
export function targetPasteDir(node: { path: string; isDir: boolean }): string {
  return node.isDir ? node.path : parentPathOf(node.path);
}

async function detectNativeSupport(): Promise<boolean> {
  if (env.isBrowser || typeof clipboardFileState !== "function") {
    state = { ...state, nativeSupported: false, nativeHasFiles: false };
    emit();
    return false;
  }
  try {
    const status = await clipboardFileState();
    const cutChanged = state.cutSequence !== null && status.sequence !== state.cutSequence;
    state = {
      ...state,
      nativeSupported: status.supported,
      nativeHasFiles: status.supported ? status.hasFiles : false,
      ...(cutChanged ? { cutPaths: [], cutSequence: null } : null),
    };
    emit();
    return status.supported;
  } catch {
    return false;
  }
}

/** Refresh cached OS clipboard availability and clear stale cut dimming if another app overwrote it. */
export async function refreshFileClipboardStatus(): Promise<void> {
  await detectNativeSupport();
}

async function writeFiles(paths: string[], mode: FileClipboardMode): Promise<void> {
  const sources = normalizeSourcePaths(paths);
  if (!sources.length) return;

  const useNative =
    !env.isBrowser &&
    typeof clipboardWriteFiles === "function" &&
    (state.nativeSupported === true || (state.nativeSupported === null && (await detectNativeSupport())));

  if (useNative) {
    const sequence = await clipboardWriteFiles(sources, mode);
    state = {
      ...state,
      nativeSupported: true,
      nativeHasFiles: true,
      fallback: null,
      cutPaths: mode === "cut" ? sources : [],
      cutSequence: mode === "cut" ? sequence : null,
    };
    emit();
    return;
  }

  state = {
    ...state,
    fallback: { paths: sources, mode },
    cutPaths: mode === "cut" ? sources : [],
    cutSequence: null,
  };
  emit();
}

export function copyFilesToClipboard(paths: string[]): Promise<void> {
  return writeFiles(paths, "copy");
}

export function cutFilesToClipboard(paths: string[]): Promise<void> {
  return writeFiles(paths, "cut");
}

/** Read files currently on the OS clipboard (desktop) or in-app fallback clipboard. */
export async function readFilesForPaste(): Promise<
  { paths: string[]; mode: FileClipboardMode; sequence?: number } | null
> {
  const useNative =
    !env.isBrowser &&
    typeof clipboardReadFiles === "function" &&
    (state.nativeSupported === true || (state.nativeSupported === null && (await detectNativeSupport())));

  if (useNative) {
    const content: FileClipboardContent | null = await clipboardReadFiles();
    if (!content || !content.paths.length) {
      state = { ...state, nativeHasFiles: false, cutPaths: [], cutSequence: null };
      emit();
      return null;
    }
    state = { ...state, nativeHasFiles: true };
    emit();
    return content;
  }

  if (!state.fallback || !state.fallback.paths.length) return null;
  return { paths: state.fallback.paths, mode: state.fallback.mode };
}

/** Clear cut dimming (and clear OS clipboard if it still holds the cut we just consumed). */
export async function finishCutPaste(sequence?: number): Promise<void> {
  state = {
    ...state,
    cutPaths: [],
    cutSequence: null,
    fallback: state.fallback?.mode === "cut" ? null : state.fallback,
    ...(sequence !== undefined ? { nativeHasFiles: false } : null),
  };
  emit();
  if (sequence !== undefined && !env.isBrowser && typeof clipboardClearFiles === "function") {
    await clipboardClearFiles(sequence).catch(() => {});
  }
}

/** Clear in-app cut dimming when the user cancels a cut via Escape or copies plain text. */
export async function clearCutClipboard(): Promise<void> {
  const seq = state.cutSequence;
  const hadFallbackCut = state.fallback?.mode === "cut";
  if (!state.cutPaths.length && !hadFallbackCut) return;
  state = {
    ...state,
    cutPaths: [],
    cutSequence: null,
    fallback: hadFallbackCut ? null : state.fallback,
    ...(seq !== null ? { nativeHasFiles: false } : null),
  };
  emit();
  if (seq !== null && !env.isBrowser && typeof clipboardClearFiles === "function") {
    await clipboardClearFiles(seq).catch(() => {});
  }
}

/** Notify that plain text (e.g. Copy Path) overwrote the clipboard. */
export function notifyTextCopied(): void {
  if (!state.cutPaths.length && !state.fallback && !state.nativeHasFiles) return;
  state = {
    ...state,
    cutPaths: [],
    cutSequence: null,
    fallback: null,
    nativeHasFiles: false,
  };
  emit();
}
