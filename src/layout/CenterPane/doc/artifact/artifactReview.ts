//! Comments the user leaves on an artifact page, and whether the page is in pick mode, kept per file.
//!
//! In memory only, like the writer map: comments are a draft for the next message to the agent, and once sent
//! they live in the conversation. Both previews of one file (the tab and the panel) share the same draft.

import type { PickedTarget } from "./artifactBridge";

export interface ArtifactComment extends PickedTarget {
  id: number;
  note: string;
}

export interface ArtifactReview {
  /** Clicking in the page picks an element instead of using the page. */
  picking: boolean;
  /** The element picked but not yet commented on. */
  draft: PickedTarget | null;
  comments: ArtifactComment[];
}

const EMPTY: ArtifactReview = { picking: false, draft: null, comments: [] };
const reviews = new Map<string, ArtifactReview>();
const listeners = new Set<() => void>();
let nextId = 1;

export function artifactReview(path: string): ArtifactReview {
  return reviews.get(path) ?? EMPTY;
}

export function subscribeArtifactReview(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function update(path: string, change: (r: ArtifactReview) => ArtifactReview): void {
  reviews.set(path, change(artifactReview(path)));
  for (const l of listeners) l();
}

export function setPicking(path: string, picking: boolean): void {
  if (artifactReview(path).picking === picking) return;
  update(path, (r) => ({ ...r, picking }));
}

/** A pick ends pick mode, so the next click uses the page again until the user asks for another. */
export function setDraft(path: string, draft: PickedTarget | null): void {
  update(path, (r) => ({ ...r, draft, picking: false }));
}

export function addComment(path: string, note: string): void {
  const text = note.trim();
  update(path, (r) => r.draft && text
    ? { ...r, draft: null, comments: [...r.comments, { ...r.draft, id: nextId++, note: text }] }
    : r);
}

export function removeComment(path: string, id: number): void {
  update(path, (r) => ({ ...r, comments: r.comments.filter((c) => c.id !== id) }));
}

export function clearComments(path: string): void {
  update(path, (r) => ({ ...r, draft: null, comments: [] }));
}

/** Pins the page draws: each comment by number, and the element being commented on. */
export function reviewMarks(review: ArtifactReview): { n: number; selector: string }[] {
  const marks = review.comments.map((c, i) => ({ n: i + 1, selector: c.selector }));
  return review.draft ? [...marks, { n: 0, selector: review.draft.selector }] : marks;
}

/** How a comment names its element for the agent: the tag, its text, and where to find it. */
export function describeTarget(target: PickedTarget): string {
  const text = target.text ? ` “${target.text.length > 60 ? `${target.text.slice(0, 60)}…` : target.text}”` : "";
  return `<${target.tag}>${text}`;
}

/** Message for the agent: the page, then each comment with the element it is about. */
export function commentReport(path: string, comments: ArtifactComment[]): string {
  const lines = comments.map((c, i) => {
    const where = [`selector \`${c.selector}\``, c.option ? `in option ${c.option}` : ""].filter(Boolean).join(", ");
    return `${i + 1}. ${describeTarget(c)} (${where}): ${c.note}`;
  });
  return `Review comments on the page ${path}:\n${lines.join("\n")}\nPlease update the page to address them.`;
}

/** Message for the agent when the user settles on one of the page's design options. */
export function optionReport(path: string, id: string, label: string): string {
  const name = label ? `${id} (${label})` : id;
  return `In ${path}, go with option ${name}. Keep that design, apply it where it belongs, and remove the other options from the page.`;
}
