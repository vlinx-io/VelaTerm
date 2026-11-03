//! Shared CodeMirror colors, highlighting, and language loading for the diff views.
//! The theme uses Vlinx CSS variables and follows light/dark mode automatically; the highlight map uses the
//! five-color palette plus text-hierarchy variables. SourceEditor.tsx currently carries an equivalent definition
//! that can later be consolidated into this module.

import {
  HighlightStyle,
  LanguageDescription,
  syntaxHighlighting,
} from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { tags } from "@lezer/highlight";

/** Highlight map shared by Markdown and code, using Vlinx's five colors and text hierarchy with automatic light/dark switching. */
export const vlxHighlight = HighlightStyle.define([
  // ── Markdown ──
  { tag: tags.heading, color: "var(--accent)", fontWeight: "600" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.monospace, color: "var(--mag)" },
  { tag: tags.link, color: "var(--cyan)" },
  { tag: tags.url, color: "var(--cyan)", textDecoration: "underline" },
  { tag: tags.quote, color: "var(--text-mid)", fontStyle: "italic" },
  { tag: tags.contentSeparator, color: "var(--text-dim)" },
  // ── General code ──
  { tag: tags.comment, color: "var(--text-dim)", fontStyle: "italic" },
  { tag: tags.meta, color: "var(--text-dim)" },
  { tag: tags.processingInstruction, color: "var(--text-dim)" },
  { tag: tags.keyword, color: "var(--mag)" },
  { tag: tags.string, color: "var(--green)" },
  { tag: tags.number, color: "var(--yellow)" },
  { tag: tags.typeName, color: "var(--yellow)" },
  { tag: tags.className, color: "var(--yellow)" },
  { tag: tags.bool, color: "var(--yellow)" },
  { tag: tags.atom, color: "var(--yellow)" },
  { tag: tags.null, color: "var(--yellow)" },
  { tag: tags.attributeName, color: "var(--yellow)" },
  { tag: tags.function(tags.variableName), color: "var(--cyan)" },
  { tag: tags.function(tags.propertyName), color: "var(--cyan)" },
  { tag: tags.tagName, color: "var(--red)" },
  { tag: tags.regexp, color: "var(--red)" },
  { tag: tags.escape, color: "var(--red)" },
  { tag: tags.definition(tags.variableName), color: "var(--text)" },
  { tag: tags.operator, color: "var(--text-mid)" },
  { tag: tags.punctuation, color: "var(--text-mid)" },
  { tag: tags.bracket, color: "var(--text-mid)" },
]);

/** Base CodeMirror theme using Vlinx variables, a transparent background, and automatic light/dark mode. */
export const vlxCmTheme = EditorView.theme({
  "&": {
    fontSize: "12.5px",
    backgroundColor: "transparent",
    color: "var(--text)",
  },
  ".cm-content": { fontFamily: "var(--font-mono)" },
  ".cm-gutters": {
    backgroundColor: "transparent",
    color: "var(--text-faint)",
    border: "none",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { lineHeight: "1.5" },
  // MergeView's built-in light palette does not follow the application's CSS-variable theme.
  ".cm-collapsedLines": {
    background: "var(--bg-hover)",
    color: "var(--text-dim)",
    borderBlock: "1px solid var(--border)",
    fontFamily: "var(--font-prose)",
    textAlign: "center",
    padding: "4px 10px",
  },
  ".cm-collapsedLines:hover": {
    background: "var(--bg-active)",
    color: "var(--text)",
  },
  "&.cm-merge-a .cm-changedLine": {
    backgroundColor: "color-mix(in srgb, var(--red) 8%, transparent)",
  },
  "&.cm-merge-b .cm-changedLine": {
    backgroundColor: "color-mix(in srgb, var(--green) 8%, transparent)",
  },
  "&.cm-merge-a .cm-changedText": {
    background: "color-mix(in srgb, var(--red) 18%, transparent)",
  },
  "&.cm-merge-b .cm-changedText": {
    background: "color-mix(in srgb, var(--green) 18%, transparent)",
  },
  "&.cm-merge-a .cm-changedLineGutter": { background: "var(--red)" },
  "&.cm-merge-b .cm-changedLineGutter": { background: "var(--green)" },
});

/** Highlight and theme extensions shared by both sides of a diff. */
export function vlxCmHighlighting(): Extension {
  return [syntaxHighlighting(vlxHighlight), vlxCmTheme];
}

/** Match a language by filename and load its extension asynchronously; return null for plain text on no match or load failure. */
export async function languageExtensionFor(
  path: string,
): Promise<Extension | null> {
  const basename = path.split("/").pop() || path;
  const desc = LanguageDescription.matchFilename(languages, basename);
  if (!desc) return null;
  try {
    return await desc.load();
  } catch {
    return null;
  }
}
