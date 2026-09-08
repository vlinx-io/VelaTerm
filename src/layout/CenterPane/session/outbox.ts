import { create } from "zustand";
import { chatSend, type ChatImage, type SendBehavior } from "../../../ipc/chat";

export interface Submission {
  id: string;
  text: string;
  images: ChatImage[];
  behavior: SendBehavior;
  status: "sending" | "sent" | "queued" | "failed" | "unknown";
  error?: string;
  observed?: boolean;
}

const EMPTY: Submission[] = [];
export const useOutbox = create<{ sessions: Record<string, Submission[]> }>(() => ({ sessions: {} }));
export const submissionsFor = (session: string) => useOutbox.getState().sessions[session] ?? EMPTY;
export const emptySubmissions = EMPTY;

function change(session: string, update: (items: Submission[]) => Submission[]) {
  useOutbox.setState(state => ({ sessions: { ...state.sessions, [session]: update(state.sessions[session] ?? EMPTY) } }));
}

export function acknowledgeSubmissions(session: string, ids: string[]) {
  const confirmed = new Set(ids);
  if (submissionsFor(session).some(item => confirmed.has(item.id))) {
    change(session, items => items.flatMap(item => !confirmed.has(item.id) ? [item]
      : item.status === "sent" || item.status === "queued" ? [] : [{ ...item, observed: true }]));
  }
}

/** Only unconfirmed UI state lives here; mounted panes share it across navigation. */
export function createSubmission(session: string, text: string, images: ChatImage[], behavior: SendBehavior): Submission {
  const item: Submission = { id: `msg-${crypto.randomUUID()}`, text, images, behavior, status: "sending" };
  change(session, items => [...items, item]);
  return item;
}

const active = new Set<string>();
export async function deliverSubmission(session: string, item: Submission, start?: () => Promise<void>) {
  const key = `${session}:${item.id}`;
  if (active.has(key)) return;
  active.add(key);
  change(session, items => items.map(value => value.id === item.id ? { ...value, status: "sending", error: undefined } : value));
  try {
    if (start) await start();
    const status = await chatSend(session, item.text, item.behavior, item.images.length ? item.images : undefined, item.id);
    change(session, items => items.flatMap(value => value.id !== item.id ? [value]
      : status === "command" || value.observed ? [] : [{ ...value, status }]));
  } catch (error) {
    const unknown = (error instanceof Error && error.name === "TransportError") || String(error).includes("chat_submission_pending");
    change(session, items => items.map(value => value.id === item.id
      ? { ...value, status: unknown ? "unknown" : "failed", error: unknown ? undefined : String(error) } : value));
  } finally {
    active.delete(key);
  }
}

export function retrySubmission(session: string, item: Submission, start?: () => Promise<void>) {
  // A confirmed rejection can be corrected by a new attempt. Uncertain delivery must
  // keep its original identifier so the backend can return the durable receipt.
  if (item.status === "failed") {
    change(session, items => items.filter(value => value.id !== item.id));
    item = createSubmission(session, item.text, item.images, item.behavior);
  }
  return deliverSubmission(session, item, start);
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
