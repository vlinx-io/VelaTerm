import messages from "./messages.json";

/** Presentation text only; sharing rules and choices come from the host and account service. */
export function sharingText(message: string, requested?: string): string {
  const saved = typeof document === "undefined" ? undefined : document.cookie.split("; ").find(item => item.startsWith("velaterm-site-locale="))?.split("=")[1];
  const language = requested ?? saved ?? (typeof navigator === "undefined" ? "en" : navigator.language);
  const normalized = language.replace("_", "-");
  const locale = normalized in messages ? normalized : normalized.startsWith("zh")
    ? (/TW|HK|Hant/i.test(normalized) ? "zh-TW" : "zh-CN")
    : normalized.startsWith("pt") ? "pt-BR" : normalized.split("-")[0];
  const dictionary = messages[locale as keyof typeof messages] ?? messages.en;
  return (dictionary as Record<string, string>)[message] ?? message;
}
