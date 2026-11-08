//! Turn a native select-all inside CodeMirror into a whole-document selection.
//!
//! CodeMirror renders only the lines near the screen. On macOS the app menu's predefined Select All
//! owns Cmd+A, so WKWebView runs its native `selectAll:` before CodeMirror's keymap sees the key, and
//! the context menu's `execCommand("selectAll")` does the same. Either way the DOM selection covers only
//! the rendered lines, and copying it silently drops the rest of the file. CodeMirror repairs this
//! itself only on Chrome and iOS, and only from an empty selection; this extension covers WebKit too.

import { EditorSelection, EditorState, type Extension, type Transaction } from "@codemirror/state";
import { ViewPlugin } from "@codemirror/view";

export interface Range {
  from: number;
  to: number;
}

/** True when `sel` is what a native select-all over the rendered `viewport` reads back as. The DOM
 *  edges sit next to the gap elements, so either end may land one position off, or already at the
 *  document boundary. A viewport that already spans the whole document needs no repair. */
export function isViewportSelectAll(sel: Range, viewport: Range, docLength: number): boolean {
  if (viewport.from === 0 && viewport.to === docLength) return false;
  if (sel.from === sel.to || (sel.from === 0 && sel.to === docLength)) return false;
  const startOk = sel.from === 0 || sel.from === viewport.from || sel.from === viewport.from - 1;
  const endOk = sel.to === docLength || sel.to === viewport.to || sel.to === viewport.to + 1;
  return startOk && endOk;
}

/** Expand a selection read back from a native select-all to the whole document. */
export function nativeSelectAll(): Extension {
  // The viewport at the time the DOM selection was read; it only changes through view updates.
  let viewport: Range | null = null;
  const track = ViewPlugin.define((view) => {
    viewport = view.viewport;
    return {
      update(u) {
        viewport = u.view.viewport;
      },
    };
  });
  const expand = EditorState.transactionFilter.of((tr: Transaction) => {
    // Only selection-only changes read from the DOM; pointer drags and edits keep their own range.
    if (!tr.selection || tr.docChanged || tr.selection.ranges.length !== 1) return tr;
    if (!tr.isUserEvent("select") || tr.isUserEvent("select.pointer")) return tr;
    const docLength = tr.newDoc.length;
    if (!viewport || !isViewportSelectAll(tr.selection.main, viewport, docLength)) return tr;
    return [tr, { selection: EditorSelection.single(0, docLength), sequential: true }];
  });
  return [track, expand];
}
