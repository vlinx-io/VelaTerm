import Icons from "./Icons";
import { getLocale, useT } from "../i18n";
import { isTauri } from "../ipc/transport";
import { env } from "../platform";
import { useTermStore } from "../store/termStore";

/** A real public link, opened in the existing browser pane for desktop clients. */
export function GameCenterLink({ className = "tab-add", iconSize = 14 }: { className?: string; iconSize?: number }) {
  const t = useT();
  const locale = getLocale();
  const href = `https://velaterm.com${locale === "en" ? "" : `/${locale}`}/game-center`;
  return <a className={className} href={href} target="_blank" rel="noopener noreferrer"
    title={t("titlebar.gameCenter")} aria-label={t("titlebar.gameCenter")}
    onClick={(event) => {
      if (!(isTauri || env.isElectron) || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      // Hide the browser chrome so the center and the games it launches fill the pane as a standalone page.
      useTermStore.getState().openBrowserTab(href, { chromeHidden: true });
    }}>
    <Icons.gamepad size={iconSize} aria-hidden="true" />
  </a>;
}
