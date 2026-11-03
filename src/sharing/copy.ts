import en from "./messages/en.json";

type Messages = Record<string, string>;

/**
 * Loaders for every language except English, which stays in the entry bundle as the fallback. Each is its
 * own chunk: bundling all eleven put about 130 KB of sharing copy into the entry bundle, which the window
 * had to load before its first paint even though a session reads one language.
 */
const LOADERS: Record<string, () => Promise<{ default: Messages }>> = {
  "zh-CN": () => import("./messages/zh-CN.json"),
  "zh-TW": () => import("./messages/zh-TW.json"),
  ja: () => import("./messages/ja.json"),
  ko: () => import("./messages/ko.json"),
  fr: () => import("./messages/fr.json"),
  de: () => import("./messages/de.json"),
  es: () => import("./messages/es.json"),
  "pt-BR": () => import("./messages/pt-BR.json"),
  ru: () => import("./messages/ru.json"),
  vi: () => import("./messages/vi.json"),
};

const loaded = new Map<string, Messages>([["en", en]]);

/**
 * Load `locale`'s sharing copy. The i18n module calls this alongside the interface dictionary, so the copy
 * is ready whenever the interface language is. Never rejects: a failed load leaves English in place.
 */
export function loadSharingText(locale: string): Promise<void> {
  const load = LOADERS[locale];
  if (!load || loaded.has(locale)) return Promise.resolve();
  return load().then(mod => { loaded.set(locale, mod.default); }, () => {});
}

/** Presentation text only; sharing rules and choices come from the host and account service. */
export function sharingText(message: string, requested?: string): string {
  const saved = typeof document === "undefined" ? undefined : document.cookie.split("; ").find(item => item.startsWith("velaterm-site-locale="))?.split("=")[1];
  const language = requested ?? saved ?? (typeof navigator === "undefined" ? "en" : navigator.language);
  const normalized = language.replace("_", "-");
  const locale = normalized === "en" || normalized in LOADERS ? normalized : normalized.startsWith("zh")
    ? (/TW|HK|Hant/i.test(normalized) ? "zh-TW" : "zh-CN")
    : normalized.startsWith("pt") ? "pt-BR" : normalized.split("-")[0];
  const dictionary = loaded.get(locale) ?? en as Messages;
  return dictionary[message] ?? (en as Messages)[message] ?? message;
}
