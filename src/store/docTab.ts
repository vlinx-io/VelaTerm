//! Data model and extension classifiers for center-pane document tabs opened by `vopen`. Extracted
//! from termStore because these types and helpers are self-contained and shared by DocView.

import { genId } from "../genId";
import type { GitChangeSide } from "../ipc/commands";

/**
 * Document content type used by DocView dispatch:
 * markdown supports visual, source, and comparison modes; image uses the built-in viewer; code uses the source
 * editor with filename-based language highlighting and plain-text fallback; artifact renders an HTML page in a
 * sandboxed preview with preview, source, and split modes; diff shows the file's uncommitted changes read-only;
 * changes lists a repository comparison's files beside the selected file's diff.
 */
export type DocKind = "markdown" | "code" | "image" | "artifact" | "diff" | "changes";

/** One comparison a changes tab shows, and the file selected in it. */
export interface ChangesTarget {
  /** Repository folder the Git commands run in. */
  cwd: string;
  /** Git area compared when `commit` is null. */
  side: GitChangeSide;
  /** Commit to show instead of the worktree, or null for uncommitted work. */
  commit: string | null;
  /** With `commit`, compare from this commit instead of its parent; `label` names the base. */
  base: { from: string; label: string } | null;
  /** Repository-relative path of the selected file, or null to select the first one. */
  path: string | null;
}

/** Editor modes: visual/source/compare for Markdown, preview/source/split for artifacts, source for the rest. */
export type DocMode = "visual" | "source" | "compare" | "preview" | "split";

/** Viewport an artifact preview renders at; fit fills the available area. */
export type ArtifactViewport = "fit" | "desktop" | "tablet" | "phone";

export interface DocTab {
  /** `"doc-" + genId()`, used as the visible openTabs ID. */
  id: string;
  /** Canonical absolute path suitable for exact deduplication. New drafts use an empty path until
   * their first Save As assigns the real path. */
  path: string;
  /** Basename displayed in the tab bar. */
  title: string;
  /** Content type controlling editor/viewer dispatch; only Markdown exposes a mode switch. */
  kind: DocKind;
  /** Current editor mode; meaningful for Markdown and artifacts, while code/image retain the source placeholder. */
  mode: DocMode;
  /** Artifact preview viewport; undefined means fit. */
  viewport?: ArtifactViewport;
  /** Unsaved-change flag used by the tab indicator and close confirmation; always false for images. */
  dirty: boolean;
  /** Pending close confirmation set for dirty tabs so DocView renders the three-way prompt. */
  pendingClose: boolean;
  /**
   * Refresh generation. Incrementing asks DocView to reread disk after deduplicated `view` focus or
   * the tab refresh action. Unsaved changes trigger the external-change banner instead of being lost.
   */
  reloadNonce: number;
  /** New unsaved draft with an empty path. DocView skips disk I/O and uses Save As. Existing files
   * opened by view leave this undefined; setDocTabPath clears it after persistence. */
  isNew?: boolean;
  /** For kind=diff: the repository's top-level folder and the file's path relative to it. */
  diff?: { root: string; path: string };
  /** For kind=changes: the comparison shown. */
  changes?: ChangesTarget;
}

/** Markdown-family extensions start as kind=markdown in visual mode. */
const MARKDOWN_EXTS = new Set(["md", "markdown", "mdx"]);

/** HTML pages use kind=artifact and open in the sandboxed preview. */
const ARTIFACT_EXTS = new Set(["html", "htm"]);

/** Image extensions use kind=image and the built-in viewer. */
const IMAGE_EXTS = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"]);

/** Classify by extension; all other files, including extensionless paths, are code. */
export function docKindOf(path: string): DocKind {
  const title = path.split("/").pop() || path;
  const ext = title.includes(".") ? title.split(".").pop()!.toLowerCase() : "";
  if (MARKDOWN_EXTS.has(ext)) return "markdown";
  if (IMAGE_EXTS.has(ext)) return "image";
  if (ARTIFACT_EXTS.has(ext)) return "artifact";
  return "code";
}

/** Create document-tab metadata for a path. */
export function makeDocTab(path: string): DocTab {
  const title = path.split("/").pop() || path;
  const kind = docKindOf(path);
  return {
    id: `doc-${genId()}`,
    path,
    title,
    kind,
    mode: kind === "markdown" ? "visual" : kind === "artifact" ? "preview" : "source",
    dirty: false,
    pendingClose: false,
    reloadNonce: 0,
  };
}

/** A read-only tab comparing a file in the working tree with HEAD. */
export function makeDiffTab(path: string, root: string, relative: string): DocTab {
  return { ...makeDocTab(path), kind: "diff", mode: "source", diff: { root, path: relative } };
}

/** Whether two targets show the same comparison, whichever file is selected in it. */
export function sameComparison(a: ChangesTarget, b: ChangesTarget): boolean {
  return a.cwd === b.cwd && a.side === b.side && a.commit === b.commit
    && a.base?.from === b.base?.from && a.base?.label === b.base?.label;
}

/** A read-only tab listing a comparison's changed files beside the selected file's diff. */
export function makeChangesTab(target: ChangesTarget): DocTab {
  const title = target.cwd.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || target.cwd;
  return { ...makeDocTab(target.cwd), title, kind: "changes", mode: "source", changes: target };
}
