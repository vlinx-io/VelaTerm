import { create } from "zustand";
import { genId } from "../../../genId";
import { chatSend, resolveChatImage, type ChatImage, type ChatImageValue, type SendBehavior, type ChatRow } from "../../../ipc/chat";
import { cancelTranslation, prepareTranslation, sendTranslation } from "../../../ipc/translation";

export interface Submission {
  id: string;
  text: string;
  images: ChatImageValue[];
  behavior: SendBehavior;
  status: "translating" | "translationFailed" | "canceled" | "sending" | "sent" | "queued" | "failed" | "unknown";
  translation?: boolean;
  translationReady?: boolean;
  originalFallback?: boolean;
  error?: string;
  observed?: boolean;
  startedAt?: number | null;
  durationMs?: number | null;
}

const EMPTY: Submission[] = [];
export const useOutbox = create<{ sessions: Record<string, Submission[]> }>(() => ({ sessions: {} }));
export const submissionsFor = (session: string) => useOutbox.getState().sessions[session] ?? EMPTY;
export const emptySubmissions = EMPTY;

function change(session: string, update: (items: Submission[]) => Submission[]) {
  useOutbox.setState(state => ({ sessions: { ...state.sessions, [session]: update(state.sessions[session] ?? EMPTY) } }));
}

/** Drop a submission the backend returned to the composer, so no optimistic row outlives it. */
export function discardSubmission(session: string, id: string) {
  change(session, items => items.filter(item => item.id !== id));
}

export function acknowledgeSubmissions(session: string, ids: string[]) {
  const confirmed = new Set(ids);
  if (submissionsFor(session).some(item => confirmed.has(item.id) && (!item.observed || item.status === "sent" || item.status === "queued"))) {
    change(session, items => items.flatMap(item => !confirmed.has(item.id) ? [item]
      : item.status === "sent" || item.status === "queued" ? [] : [{ ...item, observed: true }]));
  }
}

/** Keep the optimistic and confirmed message in one keyed list; identity is never inferred from text. */
export function unconfirmedSubmissionRows(rows: readonly ChatRow[], queued: readonly { id: string }[], pending: readonly Submission[]): ChatRow[] {
  const known = new Set([...rows, ...queued].map(row => row.id));
  return pending.filter(item => !known.has(item.id)).map(item => ({ kind: "user", id: item.id, text: item.text, images: item.images }));
}

/** Only unconfirmed UI state lives here; mounted panes share it across navigation. */
export function createSubmission(session: string, text: string, images: ChatImage[], behavior: SendBehavior, translation = false): Submission {
  const item: Submission = { id: `msg-${genId()}`, text, images, behavior, status: translation ? "translating" : "sending", translation };
  change(session, items => [...items, item]);
  return item;
}

const active = new Map<string, object>();
export function recoverTranslationSubmissions(session: string, pending: Submission[]) {
  if (!pending.length) return;
  change(session, items => {
    const updated = items.map(item => !active.has(`${session}:${item.id}`) ? pending.find(value => value.id === item.id) ?? item : item);
    return [...updated, ...pending.filter(item => !updated.some(value => value.id === item.id))];
  });
}
export async function deliverSubmission(session: string, item: Submission, start?: () => Promise<void>, beforeTranslatedSend?: () => Promise<void>) {
  const key = `${session}:${item.id}`;
  if (active.has(key)) return;
  const operation = {};
  active.set(key, operation);
  const current = () => active.get(key) === operation;
  let translating = !!item.translation && !item.translationReady;
  change(session, items => items.map(value => value.id === item.id ? { ...value, status: translating ? "translating" : "sending", error: undefined } : value));
  try {
    const images = item.images.some(image => "attachmentId" in image) ? await Promise.all(item.images.map(image=>resolveChatImage(image,true))) : item.images as ChatImage[];
    if (translating) {
      await prepareTranslation(session, item.id, item.text, images, item.behavior);
      if (!current() || submissionsFor(session).find(value => value.id === item.id)?.status === "canceled") return;
      translating = false;
      change(session, items => items.map(value => value.id === item.id ? { ...value, translationReady: true, status: "sending" } : value));
    }
    // Install the host's original/translation mapping before the chat engine can emit its user row.
    if (item.translation && beforeTranslatedSend) await beforeTranslatedSend();
    if (!current()) return;
    if (start) await start();
    if (!current()) return;
    const receipt = item.translation ? await sendTranslation(session, item.id, item.text, images, item.behavior)
      : item.originalFallback ? await chatSend(session, item.text, item.behavior, images.length ? images : undefined, item.id, true)
      : await chatSend(session, item.text, item.behavior, images.length ? images : undefined, item.id);
    if (!current()) return;
    // A steered message is one that went out, whether or not the recipient is free to read it yet; the
    // composer draws it like any other sent message.
    const status = receipt === "steered" || receipt === "blocked" ? "sent" : receipt;
    change(session, items => items.flatMap(value => value.id !== item.id ? [value]
      : status === "command" || value.observed ? [] : [{ ...value, status }]));
    return receipt;
  } catch (error) {
    if (!current()) return;
    const required = String(error).includes("translation:required");
    const unknown = (error instanceof Error && error.name === "TransportError") || String(error).includes("chat_submission_pending");
    change(session, items => items.map(value => value.id === item.id
      ? value.status === "canceled" ? value : { ...value, translation: required ? true : value.translation, status: unknown ? "unknown" : translating || required ? "translationFailed" : "failed", error: unknown ? undefined : String(error) } : value));
  } finally {
    if (current()) active.delete(key);
  }
}

export async function cancelSubmissionTranslation(session: string, item: Submission) {
  await cancelTranslation(session, item.id);
  active.delete(`${session}:${item.id}`);
  change(session, items => items.map(value => value.id === item.id && value.status === "translating" ? { ...value, status: "canceled", error: undefined } : value));
}

export async function sendOriginalSubmission(session: string, item: Submission, start?: () => Promise<void>) {
  if (!["translationFailed", "canceled"].includes(item.status)) return;
  await cancelTranslation(session, item.id);
  change(session, items => items.map(value => value.id === item.id ? { ...value, translation: false, translationReady: false, originalFallback: true } : value));
  return deliverSubmission(session, { ...item, translation: false, translationReady: false, originalFallback: true }, start);
}

export function retrySubmission(session: string, item: Submission, start?: () => Promise<void>, beforeTranslatedSend?: () => Promise<void>) {
  // The backend reclaims only proven rejections; every retry retains the original operation identity.
  return deliverSubmission(session, item, start, beforeTranslatedSend);
}

/** Revisions belong to one agent process and to one independently updated collection. */
export class ChatVersions {
  epoch = 0;
  rows = 0;
  queue = 0;
  accept(collection: "rows" | "queue", revision?: number, epoch?: number) {
    if (epoch !== undefined && epoch < this.epoch) return false;
    if (epoch !== undefined && epoch > this.epoch) {
      this.epoch = epoch;
      this.rows = this.queue = 0;
    }
    if (revision === undefined) return true;
    if (revision < this[collection]) return false;
    this[collection] = revision;
    return true;
  }
}
