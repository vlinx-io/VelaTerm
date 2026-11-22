//! Host slot components mounting external plugin Sidebar, CenterPane, TabBar, and Rail icons
//! inside ErrorBoundary guards so a faulty plugin cannot break VelaTerm.

import { useEffect, useRef, useState } from "react";
import { ContextMenu, type MenuItem } from "../components/ContextMenu";
import { ErrorBoundary } from "../components/ErrorBoundary";
import Icons from "../components/Icons";
import { useT } from "../i18n";
import { pluginReload, type PluginSummary } from "../ipc/plugins";
import { translatePluginKey, useLoadedPlugin, usePlugins } from "./host";
import {
  clearPluginSearchParams,
  pluginNavigate,
  pluginUrl,
  readPluginRoute,
  usePluginLocation,
} from "./navigation";

/** Render a plugin's declared SVG icon (safely parsed without scripts) or a fallback icon. */
export function PluginIcon({
  plugin,
  size = 15,
}: {
  plugin?: PluginSummary | null;
  size?: number;
}) {
  const svg = plugin?.ui?.rail?.iconSvg?.trim();
  if (svg && svg.startsWith("<svg") && !/<script|on\w+\s*=/i.test(svg)) {
    return (
      <span
        className="sys-collection-ic is-plugin"
        style={{ width: size, height: size, display: "inline-flex", alignItems: "center", justifyContent: "center" }}
        dangerouslySetInnerHTML={{ __html: svg }}
      />
    );
  }
  return (
    <span className="sys-collection-ic is-plugin">
      <Icons.puzzle size={size} />
    </span>
  );
}

/** TabBar entry representing an open plugin page (`?plugin=<id>`), mirroring MemoryTab behavior. */
export function PluginTab({
  otherTabCount,
  onCloseOthers,
}: {
  otherTabCount: number;
  onCloseOthers: () => void;
}) {
  const t = useT();
  const plugins = usePlugins();
  const search = usePluginLocation();
  const activeRoute = readPluginRoute(search);

  const [pinnedPluginId, setPinnedPluginId] = useState<string | null>(
    () => activeRoute?.pluginId ?? null,
  );
  const lastRouteRef = useRef<Record<string, string>>({});
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    if (activeRoute) {
      lastRouteRef.current[activeRoute.pluginId] = activeRoute.route;
      setPinnedPluginId(activeRoute.pluginId);
    }
  }, [activeRoute?.pluginId, activeRoute?.route]);

  // Drop pinned tab automatically if the plugin was disabled or uninstalled.
  const plugin = pinnedPluginId
    ? plugins.find((p) => p.id === pinnedPluginId && p.enabled && p.ui)
    : null;

  useEffect(() => {
    if (pinnedPluginId && !plugin) {
      setPinnedPluginId(null);
    }
  }, [pinnedPluginId, plugin]);

  if (!plugin) return null;

  const isActive = activeRoute?.pluginId === plugin.id;
  const titleKey = plugin.ui?.centerPage?.tabTitleKey ?? plugin.ui?.rail?.labelKey ?? "";
  const label = titleKey ? translatePluginKey(plugin.id, titleKey) : plugin.name;
  const resolvedLabel = label === titleKey ? plugin.name : label;
  const defaultRoute = plugin.ui?.rail?.defaultRoute || "overview";

  const activate = () => {
    if (isActive) return;
    const saved = lastRouteRef.current[plugin.id] || defaultRoute;
    pluginNavigate(pluginUrl(plugin.id, saved));
  };

  const close = () => {
    setPinnedPluginId(null);
    if (isActive) clearPluginSearchParams(plugin.id);
  };

  const items: MenuItem[] = [
    { label: t("common.close"), onClick: close },
    { label: t("tab.closeOthers"), onClick: onCloseOthers, disabled: otherTabCount === 0 },
    {
      label: t("plugins.reload"),
      onClick: () => {
        void pluginReload(plugin.id);
      },
    },
  ];

  return (
    <>
      <div
        className={`tab${isActive ? " active" : ""}`}
        title={resolvedLabel}
        onClick={activate}
        onContextMenu={(e) => {
          e.preventDefault();
          setMenu({ x: e.clientX, y: e.clientY });
        }}
      >
        <PluginIcon plugin={plugin} size={13} />
        <span className="t-name">{resolvedLabel}</span>
        <span
          className="t-close"
          role="button"
          aria-label={t("common.close")}
          onClick={(e) => {
            e.stopPropagation();
            close();
          }}
        >
          <Icons.close size={11} />
        </span>
      </div>
      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={items} onClose={() => setMenu(null)} />
      )}
    </>
  );
}

/** CenterPane overlay host rendering the active plugin's CenterPage inside an ErrorBoundary. */
export function PluginCenterRoute() {
  const t = useT();
  const search = usePluginLocation();
  const active = readPluginRoute(search);
  const runtime = useLoadedPlugin(active?.pluginId ?? null);

  if (!active) return null;

  if (runtime?.loadError) {
    return (
      <div className="plugin-center-state">
        <div className="plugin-center-error">
          <strong>{t("plugins.loadFailed")}</strong>
          <span>{runtime.loadError}</span>
        </div>
      </div>
    );
  }

  const CenterPage = runtime?.registration?.CenterPage;
  if (!CenterPage) {
    return (
      <div className="plugin-center-state">
        <span>{t("common.loading")}</span>
      </div>
    );
  }

  return (
    <ErrorBoundary>
      <CenterPage route={active.route} />
    </ErrorBoundary>
  );
}

/** LeftSidebar slot rendering a plugin's Sidebar component inside an ErrorBoundary. */
export function PluginSidebarHost({ pluginId }: { pluginId: string }) {
  const t = useT();
  const runtime = useLoadedPlugin(pluginId);

  if (runtime?.loadError) {
    return (
      <div style={{ padding: 12, fontSize: 12, color: "var(--red)" }}>
        {runtime.loadError}
      </div>
    );
  }

  const Sidebar = runtime?.registration?.Sidebar;
  if (!Sidebar) {
    return (
      <div style={{ padding: 12, fontSize: 12, color: "var(--text-dim)" }}>
        {t("common.loading")}
      </div>
    );
  }

  return (
    <ErrorBoundary>
      <Sidebar />
    </ErrorBoundary>
  );
}
