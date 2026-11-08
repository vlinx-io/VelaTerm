//! Activity bar: a narrow column with one button per fixed sidebar section. Every section owns a pane group (its
//! splits and each pane's conditions); picking a section swaps the whole group in, and picking the section already
//! shown hides or shows the sidebar. Settings, account, feedback, share, and game center sit at the bottom of the bar.

import { Fragment, useEffect, useState } from "react";
import { GameCenterLink } from "../../components/GameCenterLink";
import Icons from "../../components/Icons";
import { getLocale, useT } from "../../i18n";
import { isShareSurface } from "../../ipc/shareBase";
import { invoke } from "../../ipc/transport";
import { env, platform } from "../../platform";
import { sharingText } from "../../sharing/copy";
import { SharingLink } from "../../sharing/navigation";
import { navigateTranslation, translationUrl } from "../../translation/navigation";
import { useTermStore } from "../../store/termStore";
import { memoryNavigate, memoryUrl } from "../Memory/navigation";
import { knowledgeSelection } from "../Notebook/knowledgeSelection";
import { sidebarTabOfView, sidebarTabSection } from "./sidebarTreeLayout";
import {
  liveTerminalIds,
  RAIL_SECTIONS,
  SYSTEM_PROJECTS,
  SystemCollectionIcon,
  systemLabel,
} from "./systemCollections";

export function SidebarRail() {
  const t = useT();
  const section = useTermStore((s) =>
    sidebarTabSection(
      sidebarTabOfView(s.sidebarTreeTabs, s.activeSidebarTreeViewId) ??
        sidebarTabOfView(s.sidebarTreeTabs, s.primarySidebarTreeViewId),
    ));
  const collapsed = useTermStore((s) => s.leftCollapsed);
  const pick = useTermStore((s) => s.pickSidebarSection);
  // A backend without the fixed collections shows its old single tree, which has no sections to pick.
  const fixedLayout = useTermStore((s) => s.projects.some((p) => p.id === SYSTEM_PROJECTS));
  // A number, so runtime updates that keep the same count do not re-render the bar.
  const terminals = useTermStore((s) => liveTerminalIds(s.sessions, s.ephemeralSessions, s.runtimes).length);

  return (
    <nav className="sidebar-rail" aria-label={t("sidebarSection.nav")}>
      {fixedLayout && RAIL_SECTIONS.map((id) => {
        const active = section === id && !collapsed;
        const count = id === "terminals" ? terminals : 0;
        const label = systemLabel(t, id);
        return (
          <Fragment key={id}>
          {id === "archive" && <span className="sidebar-rail-sep" aria-hidden="true" />}
          <button
            type="button"
            className={"sidebar-rail-btn" + (active ? " active" : "")}
            title={label}
            aria-label={count > 0 ? `${label} (${count})` : label}
            aria-pressed={active}
            onClick={() => {
              pick(id);
              // Archive and Knowledge are pages as well as sidebar sections; showing one opens its page.
              if (active) return;
              const route = new URLSearchParams(window.location.search).get("memory") ?? "";
              const onArchive = route === "collections" || route.startsWith("collection/");
              if (id === "archive" && !onArchive) memoryNavigate(memoryUrl("collections", knowledgeSelection));
              if (id === "knowledge" && (!route || onArchive)) memoryNavigate(memoryUrl("notebooks", knowledgeSelection));
            }}
          >
            <SystemCollectionIcon section={id} size={16} />
            {count > 0 && <span className="sidebar-rail-count" aria-hidden="true">{count > 99 ? "99+" : count}</span>}
          </button>
          </Fragment>
        );
      })}
      <span className="sidebar-rail-sp" />
      <RailFooter />
    </nav>
  );
}

const FEEDBACK_URL = "https://velaterm.com/feedback";

/** Settings, account, feedback, share, and game center entries pinned to the bottom of the activity bar. */
function RailFooter() {
  const t = useT();
  // Store-level settings visibility is shared with the macOS native Settings menu; the modal itself is
  // mounted by the title bar, which follows the route.
  const settingsOpen = useTermStore((s) => s.settingsOpen);
  const setSettingsOpen = useTermStore((s) => s.setSettingsOpen);
  // Share-dialog visibility is shared with the macOS native Share menu; the title bar mounts the dialog.
  const shareOpen = useTermStore((s) => s.shareOpen);
  const setShareOpen = useTermStore((s) => s.setShareOpen);
  const accountLinked = usePublicAccountLinked();
  const accountLabel = `${sharingText("Account", getLocale())} · ${t("common.experimental")}`;

  return (
    <>
      {!isShareSurface && (
        <button
          type="button"
          className={"sidebar-rail-btn sidebar-rail-tool" + (settingsOpen ? " on" : "")}
          title={t("settings.title")}
          aria-label={t("settings.title")}
          onClick={(e) => {
            // Option/Alt reveals the translation category for this opening only.
            if (settingsOpen && !e.altKey) {
              setSettingsOpen(false);
              navigateTranslation(translationUrl({ settings: null, settingsSection: null, settingsTranslation: null, translationService: null }));
            } else {
              navigateTranslation(translationUrl({ settings: "appearance", settingsSection: null, settingsTranslation: e.altKey ? "1" : null, translationService: null }));
            }
          }}
        >
          <Icons.gear size={16} />
        </button>
      )}

      {!env.isBrowser && (
        <SharingLink
          className={"sidebar-rail-btn sidebar-rail-tool" + (accountLinked ? " account-on" : "")}
          values={{ publicAccount: "1" }}
          title={accountLabel}
          aria-label={accountLabel}
        >
          <Icons.account size={16} />
        </SharingLink>
      )}

      {/* Feedback opens the feedback page in the system browser on every platform and surface. */}
      <button
        type="button"
        className="sidebar-rail-btn sidebar-rail-tool"
        title={t("titlebar.feedback")}
        aria-label={t("titlebar.feedback")}
        onClick={() => {
          void platform.opener.openExternal(FEEDBACK_URL).catch(() => {});
        }}
      >
        <Icons.feedback size={16} />
      </button>

      {/* Public share windows serve a single grant, so sharing and settings stay hidden there. */}
      {!isShareSurface && (
        <button
          type="button"
          className={"sidebar-rail-btn sidebar-rail-tool" + (shareOpen ? " on" : "")}
          title={t("titlebar.share")}
          aria-label={t("titlebar.share")}
          onClick={() => setShareOpen(!shareOpen)}
        >
          <Icons.share size={16} />
        </button>
      )}

      {!isShareSurface && <GameCenterLink className="sidebar-rail-btn sidebar-rail-tool" iconSize={16} />}
    </>
  );
}

/**
 * Account link state lights the account icon. The relay is authoritative; query it once and refresh when
 * the account panel reports a link change in this window.
 */
function usePublicAccountLinked(): boolean {
  const [linked, setLinked] = useState(false);
  useEffect(() => {
    // Browser clients never show the account entry, and the relay status is not part of their session.
    if (env.isBrowser) return;
    let alive = true;
    const refresh = () => {
      invoke<{ linked: boolean }>("public_account_status")
        .then((status) => {
          if (alive) setLinked(status.linked);
        })
        .catch(() => {
          // Keep the last known state while the account service is unreachable; the icon is decoration.
        });
    };
    refresh();
    window.addEventListener("public-account-changed", refresh);
    return () => {
      alive = false;
      window.removeEventListener("public-account-changed", refresh);
    };
  }, []);
  return linked;
}
