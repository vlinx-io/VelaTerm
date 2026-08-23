//! Lightweight dependency-free i18n module:
//! - The English dictionary is the sole key source; `I18nKey` derives from it and other dictionaries
//!   must satisfy the same type, making missing keys compile errors.
//! - Values are strings or `(params) => string` functions for language-specific interpolation/plurals.
//! - The persisted `vlx-lang` value is a Locale or "auto", which follows the system language.
//! - navigator.languages is matched exactly, then regionally (Traditional/Simplified Chinese,
//!   Brazilian Portuguese, or primary language), finally falling back to English.
//! - `t()` is synchronous for any module; `useT()` subscribes through useSyncExternalStore.

import { useSyncExternalStore } from "react";
import en from "./locales/en";

/** Supported locales. */
export const LOCALES = [
  "en",
  "zh-CN",
  "zh-TW",
  "ja",
  "ko",
  "fr",
  "de",
  "es",
  "pt-BR",
  "ru",
  "vi",
] as const;

export type Locale = (typeof LOCALES)[number];
/** User locale choice, or "auto" to follow the system. */
export type LangChoice = Locale | "auto";

/** Native locale names displayed in the settings dropdown. */
export const LOCALE_NAMES: Record<Locale, string> = {
  en: "English",
  "zh-CN": "简体中文",
  "zh-TW": "繁體中文",
  ja: "日本語",
  ko: "한국어",
  fr: "Français",
  de: "Deutsch",
  es: "Español",
  "pt-BR": "Português (Brasil)",
  ru: "Русский",
  vi: "Tiếng Việt",
};

/** English defines the key set and each value's string or function signature. */
export type Dict = typeof en;
export type I18nKey = keyof Dict;

/**
 * Dictionary loaders for every locale except English, which stays in the entry bundle because it is
 * both the key source and the fallback used whenever another dictionary is missing or still loading.
 *
 * These are dynamic imports so each dictionary becomes its own chunk: the eleven dictionaries used to
 * account for 450 KB of the entry bundle (about a fifth of it) even though a session only ever reads
 * one of them.
 *
 * The `Promise<{ default: Dict }>` annotation is load-bearing, not decoration. It is what makes the
 * compiler check every dictionary against English's key set, which the old `Record<Locale, Dict>` map
 * used to do — a translation missing a key still fails the build.
 */
const LOADERS: Record<Exclude<Locale, "en">, () => Promise<{ default: Dict }>> = {
  "zh-CN": () => import("./locales/zh-CN"),
  "zh-TW": () => import("./locales/zh-TW"),
  ja: () => import("./locales/ja"),
  ko: () => import("./locales/ko"),
  fr: () => import("./locales/fr"),
  de: () => import("./locales/de"),
  es: () => import("./locales/es"),
  "pt-BR": () => import("./locales/pt-BR"),
  ru: () => import("./locales/ru"),
  vi: () => import("./locales/vi"),
};

/** Dictionaries available synchronously to `t()`. English is present from the start. */
const loaded = new Map<Locale, Dict>([["en", en]]);
/** In-flight loads, so concurrent callers share one request per locale. */
const loading = new Map<Locale, Promise<void>>();

/**
 * Ensure `locale`'s dictionary is in `loaded`, resolving immediately when it already is. A failed
 * load resolves rather than rejects: the caller keeps rendering, English simply stays in place.
 */
function ensureDict(locale: Locale): Promise<void> {
  if (loaded.has(locale)) return Promise.resolve();
  const existing = loading.get(locale);
  if (existing) return existing;

  const task = LOADERS[locale as Exclude<Locale, "en">]()
    .then((mod) => {
      loaded.set(locale, mod.default);
    })
    .catch(() => {
      /* Keep falling back to English; a retry happens on the next locale change. */
    })
    .finally(() => {
      loading.delete(locale);
    });
  loading.set(locale, task);
  return task;
}

/**
 * Load the active locale before the first render. `main.tsx` awaits this so the UI does not paint
 * English and then swap to the user's language a moment later.
 */
export function initI18n(): Promise<void> {
  return ensureDict(currentLocale);
}

const STORAGE_KEY = "vlx-lang";

/** Map a BCP 47 language tag to a supported Locale, returning null when unknown. */
function mapTag(tag: string): Locale | null {
  const t = tag.toLowerCase();
  // Exact case-insensitive match.
  for (const loc of LOCALES) {
    if (t === loc.toLowerCase()) return loc;
  }
  // Chinese regions: Hant/TW/HK/MO use zh-TW; all others use zh-CN.
  if (t === "zh" || t.startsWith("zh-")) {
    if (
      t.includes("hant") ||
      t.startsWith("zh-tw") ||
      t.startsWith("zh-hk") ||
      t.startsWith("zh-mo")
    ) {
      return "zh-TW";
    }
    return "zh-CN";
  }
  // Map all Portuguese variants to pt-BR.
  if (t === "pt" || t.startsWith("pt-")) return "pt-BR";
  // Match other locales by primary language subtag, such as fr-CA to fr.
  const primary = t.split("-")[0];
  for (const loc of LOCALES) {
    if (primary === loc.toLowerCase()) return loc;
  }
  return null;
}

/** Detect the first supported navigator language, falling back to English. */
function detectSystemLocale(): Locale {
  try {
    const langs =
      typeof navigator !== "undefined" && navigator.languages?.length
        ? navigator.languages
        : [typeof navigator !== "undefined" ? navigator.language : ""];
    for (const tag of langs) {
      if (!tag) continue;
      const loc = mapTag(tag);
      if (loc) return loc;
    }
  } catch {
    /* Fall back to English when the environment does not support detection. */
  }
  return "en";
}

/** Read the persisted locale choice; absent or invalid values become "auto". */
export function loadLangChoice(): LangChoice {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "auto") return "auto";
    if (v && (LOCALES as readonly string[]).includes(v)) return v as Locale;
  } catch {
    /* localStorage is unavailable. */
  }
  return "auto";
}

function resolveChoice(choice: LangChoice): Locale {
  return choice === "auto" ? detectSystemLocale() : choice;
}

let currentChoice: LangChoice = loadLangChoice();
let currentLocale: Locale = resolveChoice(currentChoice);

const listeners = new Set<() => void>();

/**
 * Snapshot counter for `useSyncExternalStore`. The locale alone is not enough: when a dictionary
 * finishes loading, the locale is already set to its final value, so a locale-based snapshot would
 * not change and React would keep the English render on screen.
 */
let revision = 0;

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

function getSnapshot(): number {
  return revision;
}

/** Advance the snapshot and notify subscribers that visible text may have changed. */
function bump() {
  revision++;
  for (const cb of listeners) cb();
}

function syncHtmlLang() {
  try {
    document.documentElement.lang = currentLocale;
  } catch {
    /* Ignore non-browser environments. */
  }
}
syncHtmlLang();

/** Effective locale after resolving auto. */
export function getLocale(): Locale {
  return currentLocale;
}

/** Current locale choice, possibly "auto". */
export function getLangChoice(): LangChoice {
  return currentChoice;
}

/** BCP 47 tag used for date/time localization APIs such as toLocaleString. */
export function dateLocale(): string {
  return currentLocale;
}

/** Set the locale choice, persist it, resolve it, update `<html lang>`, and notify subscribers. */
export function setLang(choice: LangChoice) {
  currentChoice = choice;
  currentLocale = resolveChoice(choice);
  try {
    localStorage.setItem(STORAGE_KEY, choice);
  } catch {
    /* Without localStorage, apply only for this session. */
  }
  // Do not synchronize to the backend here: importing ipc/settingsSync into this low-level module
  // would form a transport -> i18n cycle. SettingsModal, the sole user entry point, calls pushSetting
  // after setLang instead (see ipc/settingsSync.ts).
  syncHtmlLang();
  bump();
  // Fetch the dictionary if this locale has not been used yet; `bump()` again once it lands so the
  // brief English fallback is replaced. Already-loaded locales resolve synchronously enough that no
  // second render is scheduled in practice.
  if (!loaded.has(currentLocale)) {
    const target = currentLocale;
    void ensureDict(target).then(() => {
      if (currentLocale === target) bump();
    });
  }
}

/** Parameter tuple for function entries; string entries use an empty tuple. */
type Params<K extends I18nKey> = Dict[K] extends (...args: infer A) => string
  ? A
  : [];

/**
 * Return text in the current locale for direct use outside React. Fall back to English if a runtime
 * dictionary unexpectedly lacks a key despite compile-time checks.
 */
export function t<K extends I18nKey>(key: K, ...args: Params<K>): string {
  const dict = loaded.get(currentLocale) ?? en;
  const entry = (dict[key] ?? en[key]) as string | ((...a: unknown[]) => string);
  return typeof entry === "function" ? entry(...args) : entry;
}

/** React hook that subscribes to locale changes and returns t. */
export function useT(): typeof t {
  useSyncExternalStore(subscribe, getSnapshot);
  return t;
}
