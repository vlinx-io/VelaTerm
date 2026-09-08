//! Clipboard serialization for selections inside one rendered chat message.
//!
//! Browser copy derives plain text from the layout DOM. That makes harmless presentation wrappers such as
//! `.sv-msg-body > .sv-p` leak block separators into the clipboard. Instead, rendered Markdown nodes carry
//! their source semantics as data attributes; this module clones only the selected range, restores those
//! semantics, and serializes the result. The clipboard therefore follows the message content rather than
//! whichever `div` structure the current design happens to use.

import TurndownService from "turndown";
import { gfm } from "turndown-plugin-gfm";

export const COPY_TAG_ATTRIBUTE = "data-sv-markdown-tag";
export const COPY_IGNORE_ATTRIBUTE = "data-sv-markdown-ignore";
export const COPY_LIST_MARKER_ATTRIBUTE = "data-sv-markdown-list-marker";
export const COPY_LIST_START_ATTRIBUTE = "data-sv-markdown-list-start";
export const COPY_LANGUAGE_ATTRIBUTE = "data-sv-markdown-language";
export const COPY_ALIGN_ATTRIBUTE = "data-sv-markdown-align";
export const COPY_TASK_ATTRIBUTE = "data-sv-markdown-task";

export const copyAttributes = {
  blockquote: { [COPY_TAG_ATTRIBUTE]: "blockquote" },
  br: { [COPY_TAG_ATTRIBUTE]: "br" },
  code: { [COPY_TAG_ATTRIBUTE]: "code" },
  em: { [COPY_TAG_ATTRIBUTE]: "em" },
  h1: { [COPY_TAG_ATTRIBUTE]: "h1" },
  h2: { [COPY_TAG_ATTRIBUTE]: "h2" },
  h3: { [COPY_TAG_ATTRIBUTE]: "h3" },
  h4: { [COPY_TAG_ATTRIBUTE]: "h4" },
  h5: { [COPY_TAG_ATTRIBUTE]: "h5" },
  h6: { [COPY_TAG_ATTRIBUTE]: "h6" },
  hr: { [COPY_TAG_ATTRIBUTE]: "hr" },
  ignore: { [COPY_IGNORE_ATTRIBUTE]: "true" },
  li: { [COPY_TAG_ATTRIBUTE]: "li" },
  listMarker: { [COPY_IGNORE_ATTRIBUTE]: "true", [COPY_LIST_MARKER_ATTRIBUTE]: "true" },
  ol: { [COPY_TAG_ATTRIBUTE]: "ol" },
  p: { [COPY_TAG_ATTRIBUTE]: "p" },
  pre: { [COPY_TAG_ATTRIBUTE]: "pre" },
  s: { [COPY_TAG_ATTRIBUTE]: "s" },
  strong: { [COPY_TAG_ATTRIBUTE]: "strong" },
  table: { [COPY_TAG_ATTRIBUTE]: "table" },
  tbody: { [COPY_TAG_ATTRIBUTE]: "tbody" },
  td: { [COPY_TAG_ATTRIBUTE]: "td" },
  th: { [COPY_TAG_ATTRIBUTE]: "th" },
  thead: { [COPY_TAG_ATTRIBUTE]: "thead" },
  tr: { [COPY_TAG_ATTRIBUTE]: "tr" },
  ul: { [COPY_TAG_ATTRIBUTE]: "ul" },
} as const;

export function orderedListCopyAttributes(start: string | number | undefined) {
  return { ...copyAttributes.ol, [COPY_LIST_START_ATTRIBUTE]: String(start ?? 1) } as const;
}

export function codeBlockCopyAttributes(language: string | undefined) {
  const fenceLanguage = language?.trim().split(/\s+/)[0];
  return {
    ...copyAttributes.pre,
    ...(fenceLanguage ? { [COPY_LANGUAGE_ATTRIBUTE]: fenceLanguage } : {}),
  } as const;
}

export function tableCellCopyAttributes(tag: "td" | "th", align: string | null | undefined) {
  return {
    ...copyAttributes[tag],
    ...(align ? { [COPY_ALIGN_ATTRIBUTE]: align } : {}),
  } as const;
}

export function listItemCopyAttributes(task: boolean, checked: boolean | undefined) {
  return {
    ...copyAttributes.li,
    ...(task ? { [COPY_TASK_ATTRIBUTE]: checked ? "checked" : "unchecked" } : {}),
  } as const;
}

export interface MessageClipboardContent {
  plainText: string;
  html: string;
}

const CODE_BLOCK_SELECTOR = `[${COPY_TAG_ATTRIBUTE}="pre"]`;
const CODE_REGION_SELECTOR = `${CODE_BLOCK_SELECTOR}, [${COPY_TAG_ATTRIBUTE}="code"]`;
const TRAILING_CODE_LINE_BREAKS = /(\r?\n[ \t]*)+$/;

const turndown = new TurndownService({
  bulletListMarker: "-",
  codeBlockStyle: "fenced",
  emDelimiter: "_",
  strongDelimiter: "**",
});
turndown.use(gfm);
turndown.addRule("escapedGfmTableCell", {
  filter: ["th", "td"],
  replacement: (content, node) => {
    const index = node.parentNode ? Array.from(node.parentNode.childNodes).indexOf(node) : 0;
    return `${index === 0 ? "| " : " "}${content.replace(/\|/g, "\\|")} |`;
  },
});
turndown.addRule("gfmStrikethrough", {
  filter: ["del", "s"],
  replacement: (content) => `~~${content}~~`,
});
turndown.addRule("compactListItem", {
  filter: "li",
  replacement: (content, node, options) => {
    const item = content
      .replace(/^\n+/, "")
      // This renderer wraps even tight-list text in a paragraph-like div. Turndown turns that wrapper into
      // terminal newlines; remove them before the list rule adds its single item separator.
      .replace(/\n+$/, "")
      .replace(/\n(?=\S)/g, "\n    ");
    const parent = node.parentElement;
    if (parent?.nodeName !== "OL") return `${options.bulletListMarker} ${item}\n`;
    const start = Number(parent.getAttribute("start") ?? 1);
    const index = Array.from(parent.children).indexOf(node);
    return `${start + index}. ${item}\n`;
  },
});

/** Build stable plain-text Markdown and rich HTML for a non-empty selection inside one message body. */
export function createMessageSelectionClipboardContent(
  selection: Selection | null,
  messageBody: Element,
): MessageClipboardContent | null {
  if (!selection || selection.rangeCount !== 1 || selection.isCollapsed) return null;

  const range = rangeInsideMessage(selection.getRangeAt(0), messageBody);
  if (!range) return null;

  const codeContent = createSelectedCodeContent(range, messageBody);
  if (codeContent) return codeContent;

  const container = document.createElement("div");
  container.append(cloneMarkdownSelection(range, messageBody));
  restoreMarkdownElements(container);

  // At this point whitespace comes from semantic Markdown boundaries, not layout divs. Canonicalizing the
  // outer boundary removes serializer padding while keeping all meaningful spacing inside the selection.
  const markdown = turndown.turndown(container.innerHTML).trim();
  if (!markdown) return null;
  return { plainText: markdown, html: `<meta charset="utf-8">${container.innerHTML}` };
}

function containsBoundary(root: Element, node: Node): boolean {
  return node === root || root.contains(node);
}

/**
 * Chromium's word/paragraph selection may put an otherwise empty boundary at the start of the next row.
 * The two visual block separators that native copy invents for that boundary are the reported `\n\n`.
 * Clamp only such content-free overshoot; a selection containing real text or an image outside this body
 * remains a native cross-message selection.
 */
function rangeInsideMessage(source: Range, messageBody: Element): Range | null {
  const startsInside = containsBoundary(messageBody, source.startContainer);
  const endsInside = containsBoundary(messageBody, source.endContainer);
  if (startsInside && endsInside) return source;

  const range = source.cloneRange();
  if (startsInside) {
    const overshoot = document.createRange();
    overshoot.setStartAfter(messageBody);
    overshoot.setEnd(source.endContainer, source.endOffset);
    if (hasCopiedDomContent(overshoot.cloneContents())) return null;
    range.setEnd(messageBody, messageBody.childNodes.length);
    return range;
  }
  if (endsInside) {
    const overshoot = document.createRange();
    overshoot.setStart(source.startContainer, source.startOffset);
    overshoot.setEndBefore(messageBody);
    if (hasCopiedDomContent(overshoot.cloneContents())) return null;
    range.setStart(messageBody, 0);
    return range;
  }
  return null;
}

function hasCopiedDomContent(fragment: DocumentFragment): boolean {
  return Boolean(fragment.textContent || fragment.querySelector("img, br, hr"));
}

/** Selections wholly inside code bypass Markdown conversion so indentation and line breaks stay exact. */
function createSelectedCodeContent(range: Range, messageBody: Element): MessageClipboardContent | null {
  const region = closestCodeRegion(range.commonAncestorContainer, messageBody);
  if (!region) return null;

  const fragment = range.cloneContents();
  for (const ignored of fragment.querySelectorAll(`[${COPY_IGNORE_ATTRIBUTE}]`)) ignored.remove();
  const code = (fragment.textContent ?? "").replace(TRAILING_CODE_LINE_BREAKS, "");
  if (!code) return null;

  const fence = region.closest(CODE_BLOCK_SELECTOR);
  const escaped = escapeHtml(code);
  const language = fence?.getAttribute(COPY_LANGUAGE_ATTRIBUTE)?.trim();
  const className = language ? ` class="language-${escapeHtml(language)}"` : "";
  const html =
    fence && code.includes("\n")
      ? `<meta charset="utf-8"><pre><code${className}>${escaped}</code></pre>`
      : `<meta charset="utf-8">${escaped}`;
  return { plainText: code, html };
}

function closestCodeRegion(node: Node, messageBody: Element): Element | null {
  const element = node instanceof Element ? node : node.parentElement;
  const region = element?.closest(CODE_REGION_SELECTOR) ?? null;
  if (!region || !messageBody.contains(region)) return null;
  return element?.closest(`[${COPY_IGNORE_ATTRIBUTE}]`) ? null : region;
}

function escapeHtml(value: string): string {
  const element = document.createElement("span");
  element.textContent = value;
  return element.innerHTML;
}

function cloneMarkdownSelection(range: Range, messageBody: Element): Node {
  const clonedBody = messageBody.cloneNode(true) as Element;
  const clonedRange = cloneRangeInto(range, messageBody, clonedBody);
  removeUnselectedSemantics(range, messageBody, clonedBody);

  let selected: Node = clonedRange.cloneContents();
  let ancestor =
    range.commonAncestorContainer instanceof Element
      ? range.commonAncestorContainer
      : range.commonAncestorContainer.parentElement;
  while (ancestor && ancestor !== messageBody) {
    if (shouldPreserveSemanticElement(range, ancestor)) {
      const wrapper = ancestor.cloneNode(false) as Element;
      normalizeOrderedListStart(wrapper, ancestor, range);
      wrapper.append(selected);
      selected = wrapper;
    }
    ancestor = ancestor.parentElement;
  }
  return selected;
}

function cloneRangeInto(range: Range, source: Element, clone: Element): Range {
  const clonedRange = document.createRange();
  clonedRange.setStart(nodeAtPath(clone, nodePath(source, range.startContainer)), range.startOffset);
  clonedRange.setEnd(nodeAtPath(clone, nodePath(source, range.endContainer)), range.endOffset);
  return clonedRange;
}

function nodePath(root: Node, node: Node): number[] {
  const path: number[] = [];
  let current = node;
  while (current !== root) {
    const parent = current.parentNode;
    if (!parent) throw new Error("Selection boundary is outside its message body");
    path.unshift(Array.from(parent.childNodes).findIndex((child) => child === current));
    current = parent;
  }
  return path;
}

function nodeAtPath(root: Node, path: number[]): Node {
  let current = root;
  for (const index of path) {
    const child = current.childNodes[index];
    if (!child) throw new Error("Cloned message body does not match the selection source");
    current = child;
  }
  return current;
}

function removeUnselectedSemantics(range: Range, source: Element, clone: Element): void {
  const selector = `[${COPY_TAG_ATTRIBUTE}], a`;
  const originals = Array.from(source.querySelectorAll(selector));
  const copies = Array.from(clone.querySelectorAll(selector));
  originals.forEach((original, index) => {
    const copy = copies[index];
    if (!copy || shouldPreserveSemanticElement(range, original)) {
      if (copy) normalizeOrderedListStart(copy, original, range);
      return;
    }
    const tag = original.getAttribute(COPY_TAG_ATTRIBUTE);
    if (tag && isBlockBoundary(tag)) {
      copy.setAttribute(COPY_TAG_ATTRIBUTE, "p");
      return;
    }
    copy.removeAttribute(COPY_TAG_ATTRIBUTE);
    if (copy.tagName === "A") copy.setAttribute("data-sv-markdown-unwrap", "true");
  });
}

function shouldPreserveSemanticElement(range: Range, element: Element): boolean {
  if (!range.intersectsNode(element)) return false;
  const tag = element.getAttribute(COPY_TAG_ATTRIBUTE);
  if (tag === "p" || isTableStructure(tag)) return true;
  const selectableSemantic = tag !== null && tag !== "li" && tag !== "ol" && tag !== "ul";
  if ((selectableSemantic || element.tagName === "A") && selectionStaysInsideElement(range, element)) {
    return false;
  }
  if (tag === "ol" || tag === "ul") {
    return selectedListItems(element, range).some(
      (item) => hasSelectedListMarker(range, item) || hasSelectedAllContents(range, item, true),
    );
  }
  if (tag === "li" && hasSelectedListMarker(range, element)) return true;
  return hasSelectedAllContents(range, element, tag === "li");
}

function selectionStaysInsideElement(range: Range, element: Element): boolean {
  return (
    range.startContainer !== element &&
    range.endContainer !== element &&
    element.contains(range.startContainer) &&
    element.contains(range.endContainer)
  );
}

function selectedListItems(list: Element, range: Range): Element[] {
  return Array.from(list.children).filter(
    (child) =>
      child.getAttribute(COPY_TAG_ATTRIBUTE) === "li" &&
      (child.contains(range.startContainer) ||
        child.contains(range.endContainer) ||
        hasSelectedAllContents(range, child, true)),
  );
}

function hasSelectedListMarker(range: Range, item: Element): boolean {
  const marker = item.querySelector(`:scope > [${COPY_LIST_MARKER_ATTRIBUTE}]`);
  if (!marker) return false;
  const markerContents = document.createRange();
  markerContents.selectNodeContents(marker);
  if (
    range.compareBoundaryPoints(Range.END_TO_START, markerContents) >= 0 ||
    range.compareBoundaryPoints(Range.START_TO_END, markerContents) <= 0
  ) {
    return false;
  }
  if (range.compareBoundaryPoints(Range.START_TO_START, markerContents) > 0) {
    markerContents.setStart(range.startContainer, range.startOffset);
  }
  if (range.compareBoundaryPoints(Range.END_TO_END, markerContents) < 0) {
    markerContents.setEnd(range.endContainer, range.endOffset);
  }
  return hasMarkdownContent(markerContents.cloneContents(), true);
}

function normalizeOrderedListStart(copy: Element, original: Element, range: Range): void {
  if (original.getAttribute(COPY_TAG_ATTRIBUTE) !== "ol") return;
  const firstSelected = selectedListItems(original, range)[0];
  if (!firstSelected) return;
  const items = Array.from(original.children).filter(
    (child) => child.getAttribute(COPY_TAG_ATTRIBUTE) === "li",
  );
  const originalStart = Number(original.getAttribute(COPY_LIST_START_ATTRIBUTE) ?? 1);
  copy.setAttribute(COPY_LIST_START_ATTRIBUTE, String(originalStart + items.indexOf(firstSelected)));
}

function hasSelectedAllContents(range: Range, element: Element, includeIgnored = false): boolean {
  const contents = document.createRange();
  contents.selectNodeContents(element);
  if (range.compareBoundaryPoints(Range.START_TO_START, contents) > 0) {
    const before = contents.cloneRange();
    before.setEnd(range.startContainer, range.startOffset);
    if (hasMarkdownContent(before.cloneContents(), includeIgnored)) return false;
  }
  if (range.compareBoundaryPoints(Range.END_TO_END, contents) < 0) {
    const after = contents.cloneRange();
    after.setStart(range.endContainer, range.endOffset);
    if (hasMarkdownContent(after.cloneContents(), includeIgnored)) return false;
  }
  return true;
}

function hasMarkdownContent(fragment: DocumentFragment, includeIgnored: boolean): boolean {
  if (!includeIgnored) {
    for (const ignored of fragment.querySelectorAll(`[${COPY_IGNORE_ATTRIBUTE}]`)) ignored.remove();
  }
  if (fragment.textContent) return true;
  return Boolean(
    fragment.querySelector(
      `[${COPY_TAG_ATTRIBUTE}="br"], [${COPY_TAG_ATTRIBUTE}="hr"]`,
    ),
  );
}

function isBlockBoundary(tag: string): boolean {
  return tag === "li" || tag === "blockquote" || tag === "pre" || /^h[1-6]$/.test(tag);
}

function isTableStructure(tag: string | null): boolean {
  return ["table", "thead", "tbody", "tr", "th", "td"].includes(tag ?? "");
}

function restoreMarkdownElements(container: HTMLElement): void {
  for (const ignored of container.querySelectorAll(`[${COPY_IGNORE_ATTRIBUTE}]`)) ignored.remove();

  const marked = Array.from(container.querySelectorAll(`[${COPY_TAG_ATTRIBUTE}]`));
  for (const element of marked.toReversed()) {
    const tag = element.getAttribute(COPY_TAG_ATTRIBUTE);
    if (!tag) continue;
    const semantic = document.createElement(tag);
    if (tag !== "br" && tag !== "hr") semantic.append(...element.childNodes);
    if (tag === "ol") {
      const start = element.getAttribute(COPY_LIST_START_ATTRIBUTE);
      if (start) semantic.setAttribute("start", start);
    }
    if (tag === "li") restoreTaskCheckbox(semantic, element.getAttribute(COPY_TASK_ATTRIBUTE));
    if (tag === "pre") {
      const language = element.getAttribute(COPY_LANGUAGE_ATTRIBUTE);
      const code = semantic.querySelector(":scope > code");
      if (language && code) code.className = `language-${language}`;
    }
    if (tag === "th" || tag === "td") {
      const align = element.getAttribute(COPY_ALIGN_ATTRIBUTE);
      if (align) semantic.setAttribute("align", align);
    }
    element.replaceWith(semantic);
  }

  unwrapIncompleteTables(container);
  for (const link of Array.from(container.querySelectorAll('[data-sv-markdown-unwrap="true"]')).toReversed()) {
    link.replaceWith(...link.childNodes);
  }
  for (const element of Array.from(container.querySelectorAll("div, span")).toReversed()) {
    element.replaceWith(...element.childNodes);
  }
}

function restoreTaskCheckbox(item: HTMLElement, state: string | null): void {
  if (!state) return;
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.disabled = true;
  checkbox.checked = state === "checked";
  item.prepend(checkbox, " ");
}

function unwrapIncompleteTables(container: HTMLElement): void {
  for (const table of container.querySelectorAll("table")) {
    const headerWidth = table.querySelectorAll("thead th").length;
    const usable =
      headerWidth > 0 &&
      Array.from(table.querySelectorAll("tbody tr")).every((row) => row.children.length <= headerWidth);
    if (usable) continue;
    const cells = Array.from(table.querySelectorAll("th, td"));
    const content = document.createDocumentFragment();
    cells.forEach((cell, index) => {
      if (index > 0) content.append("\n");
      content.append(...cell.childNodes);
    });
    table.replaceWith(content);
  }
}
