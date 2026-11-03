//! Translation contracts. Catalogues, defaults, credentials and authoritative task state stay on the host.
import { invoke } from "./transport";
import type { SessionKind } from "../types";
import type { ChatImage, SendBehavior } from "./chat";
import type { LaunchOption } from "./launch";

export type TranslationDisplay = "original" | "translation" | "both" | "immersive";
export type TranslationEngine = "agent" | "api" | "google" | "bing";
export interface TranslationPreferences {
  outgoing: boolean; replies: boolean; source: string; target: string;
  replySource: string; replyTarget: string; display: TranslationDisplay; repliesSince: number;
}
export interface TranslationConfig {
  enabled: boolean; defaultEnabled: boolean; conversationKinds: SessionKind[]; engine: TranslationEngine; agent: SessionKind | ""; model: string; effort: string;
  serviceId: string; version: number; generation: number; enabledSince: number;
}
export interface TranslationService { id: string; name: string; baseUrl: string; model: string; hasKey: boolean }
export interface TranslationSettings {
  config: TranslationConfig; editable: boolean; services: TranslationService[];
  conversationKinds: { id: SessionKind; label: string }[];
  agents: (LaunchOption & { id: SessionKind; available: boolean })[];
  languages: { code: string; name: string }[]; engines: TranslationEngine[];
  protocols: string[]; displays: TranslationDisplay[];
}
export interface TranslationResult { sourceLanguage: string; formatVersion?: number; pairs: { original: string; translation: string }[] }
export interface TranslationTiming { startedAt?: number | null; durationMs?: number | null }
export interface TranslationRecord extends TranslationTiming {
  id: string; original: string; source: string; target: string; version: number; generation: number;
  state: "pending" | "ready" | "failed" | "canceled"; result: TranslationResult | null; error: string | null;
}
export interface TranslatedMessage extends TranslationTiming { id: string; original: string; target: string; result: TranslationResult; accepted: boolean }
export interface TranslationSession {
  serverTime?: number;
  enabled: boolean; version: number; generation: number; preferences: TranslationPreferences;
  replies: TranslationRecord[]; messages: TranslatedMessage[];
  pending: import("../layout/CenterPane/session/outbox").Submission[];
}
export const translationSettings = () => invoke<TranslationSettings>("translation_settings");
export const translationSession = (sessionId: string) => invoke<TranslationSession>("translation_session", { sessionId });
export const saveTranslationPreferences = (sessionId: string, preferences: TranslationPreferences) => invoke<TranslationPreferences>("translation_session_save", { sessionId, preferences });
export const prepareTranslation = (sessionId: string, messageId: string, text: string, images: ChatImage[], behavior: SendBehavior) =>
  invoke<{ id: string }>("translation_prepare", { sessionId, messageId, text, images, behavior });
export const sendTranslation = (sessionId: string, messageId: string, text: string, images: ChatImage[], behavior: SendBehavior) =>
  invoke<"sent" | "queued" | "steered" | "blocked" | "command">("translation_send", { sessionId, messageId, text, images, behavior });
export const cancelTranslation = (sessionId: string, messageId: string) => invoke<void>("translation_cancel", { sessionId, messageId });
export const translateReply = (sessionId: string, rowId: string) => invoke<void>("translation_reply", { sessionId, rowId });
export interface QuestionTranslation extends TranslationTiming {
  input: unknown; originalInput: unknown; version: number; generation: number; source: string; target: string;
}
// `manual` translates a question the user asked for while automatic reply translation is off.
export const translateQuestion = (sessionId: string, requestId: string, manual = false) => invoke<QuestionTranslation>("translation_question", { sessionId, requestId, manual });
