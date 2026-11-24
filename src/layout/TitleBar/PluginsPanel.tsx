//! Settings > Plugins panel: install from archive, link local dev directory, enable/disable,
//! hot-reload, and uninstall external plugins.

import { useEffect, useState } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import {
  pluginCapabilities,
  pluginInstallArchive,
  pluginLinkDev,
  pluginReload,
  pluginSetEnabled,
  pluginUninstall,
  type PluginSummary,
} from "../../ipc/plugins";
import { env, platform } from "../../platform";
import { PluginIcon } from "../../plugins/PluginViews";
import { refreshPlugins, usePlugins } from "../../plugins/host";
import { SectionTitle } from "./settingsParts";
import "./plugins-panel.css";

type ActionMode = null | "archive" | "dev";

export function PluginsPanel() {
  const t = useT();
  const plugins = usePlugins();
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<ActionMode>(null);
  const [pathInput, setPathInput] = useState("");
  const [confirmUninstall, setConfirmUninstall] = useState<PluginSummary | null>(null);
  const [removeData, setRemoveData] = useState(false);
  const [canManage, setCanManage] = useState(!env.isBrowser || env.isElectron);

  useEffect(() => {
    void pluginCapabilities()
      .then((c) => setCanManage(Boolean(c.editable)))
      .catch(() => {});
  }, []);

  const runAction = async (id: string, fn: () => Promise<unknown>) => {
    setBusyId(id);
    setError(null);
    try {
      await fn();
      await refreshPlugins();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyId(null);
    }
  };

  const pickArchive = async () => {
    setError(null);
    if (env.hasNativeHost) {
      try {
        const picked = await platform.dialog.pickFile({
          title: t("plugins.installArchive"),
          filters: [{ name: "VelaTerm Plugin", extensions: ["vlxplugin", "zip"] }],
        });
        if (picked) {
          await runAction("__install__", () => pluginInstallArchive(picked));
        }
        return;
      } catch {
        // Fall through to manual path input if native dialog is unavailable.
      }
    }
    setMode("archive");
    setPathInput("");
  };

  const pickDevDir = async () => {
    setError(null);
    if (env.hasNativeHost) {
      try {
        const picked = await platform.dialog.pickDirectory();
        if (picked) {
          await runAction("__link__", () => pluginLinkDev(picked));
        }
        return;
      } catch {
        // Fall through to manual path input.
      }
    }
    setMode("dev");
    setPathInput("");
  };

  const submitManualPath = async () => {
    const trimmed = pathInput.trim();
    if (!trimmed || !mode) return;
    if (mode === "archive") {
      await runAction("__install__", () => pluginInstallArchive(trimmed));
    } else {
      await runAction("__link__", () => pluginLinkDev(trimmed));
    }
    setMode(null);
    setPathInput("");
  };

  const statusLabel = (p: PluginSummary): string => {
    switch (p.status) {
      case "running":
        return t("plugins.statusRunning");
      case "starting":
        return t("plugins.statusStarting");
      case "disabled":
        return t("plugins.statusDisabled");
      case "error":
        return t("plugins.statusError");
      default:
        return t("plugins.statusStopped");
    }
  };

  const statusColor = (p: PluginSummary): string => {
    switch (p.status) {
      case "running":
        return "var(--green)";
      case "starting":
        return "var(--yellow)";
      case "error":
        return "var(--red)";
      default:
        return "var(--text-dim)";
    }
  };

  return (
    <div className="plugins-panel">
      <SectionTitle>{t("plugins.title")}</SectionTitle>
      <p style={{ margin: 0, fontSize: 12, color: "var(--text-dim)", lineHeight: 1.5 }}>
        {t("plugins.subtitle")}
      </p>

      {canManage ? (
        <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
          <button
            type="button"
            className="vlx-btn vlx-btn-primary"
            disabled={busyId !== null}
            onClick={() => void pickArchive()}
          >
            {t("plugins.installArchive")}
          </button>
          <button
            type="button"
            className="vlx-btn"
            disabled={busyId !== null}
            onClick={() => void pickDevDir()}
          >
            {t("plugins.linkDev")}
          </button>
          <button
            type="button"
            className="vlx-btn"
            disabled={busyId !== null}
            onClick={() => void runAction("__refresh__", () => refreshPlugins())}
          >
            {t("common.refresh")}
          </button>
        </div>
      ) : (
        <div
          style={{
            padding: "8px 10px",
            borderRadius: 6,
            border: "1px solid var(--border)",
            background: "var(--bg-hover)",
            fontSize: 11.5,
            color: "var(--text-dim)",
          }}
        >
          {t("plugins.localOnlyHint")}
        </div>
      )}

      {mode && (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 8,
            padding: 10,
            borderRadius: 6,
            border: "1px solid var(--border)",
            background: "var(--bg-hover)",
          }}
        >
          <span style={{ fontSize: 11.5, fontWeight: 500, color: "var(--text)" }}>
            {mode === "archive" ? t("plugins.archivePathPrompt") : t("plugins.devPathPrompt")}
          </span>
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <input
              type="text"
              className="vlx-input plugins-panel-path-input"
              value={pathInput}
              onChange={(e) => setPathInput(e.target.value)}
              placeholder={mode === "archive" ? "/path/to/plugin.vlxplugin" : "/path/to/plugin-dir"}
              onKeyDown={(e) => {
                if (e.key === "Enter") void submitManualPath();
                if (e.key === "Escape") setMode(null);
              }}
            />
            <button
              type="button"
              className="vlx-btn vlx-btn-primary"
              disabled={!pathInput.trim() || busyId !== null}
              onClick={() => void submitManualPath()}
            >
              {t("common.confirm")}
            </button>
            <button
              type="button"
              className="vlx-btn"
              onClick={() => setMode(null)}
            >
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}

      {error && (
        <div
          style={{
            padding: "8px 10px",
            borderRadius: 6,
            border: "1px solid color-mix(in srgb, var(--red) 45%, transparent)",
            background: "color-mix(in srgb, var(--red) 10%, transparent)",
            color: "var(--red)",
            fontSize: 11.5,
            lineHeight: 1.45,
            overflowWrap: "anywhere",
          }}
        >
          {error}
        </div>
      )}

      {plugins.length === 0 ? (
        <div
          style={{
            padding: "24px 16px",
            textAlign: "center",
            borderRadius: 8,
            border: "1px dashed var(--border)",
            color: "var(--text-dim)",
            fontSize: 12,
          }}
        >
          {t("plugins.empty")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          {plugins.map((p) => {
            const busy = busyId === p.id;
            return (
              <div
                key={p.id}
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: 8,
                  padding: 12,
                  borderRadius: 8,
                  border: "1px solid var(--border)",
                  background: "var(--bg-2)",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <PluginIcon plugin={p} size={16} />
                  <span style={{ fontWeight: 600, fontSize: 13, color: "var(--text)" }}>
                    {p.name}
                  </span>
                  <span style={{ fontSize: 11, color: "var(--text-dim)" }}>v{p.version}</span>
                  <span
                    style={{
                      fontSize: 10.5,
                      padding: "1px 6px",
                      borderRadius: 4,
                      border: "1px solid var(--border)",
                      color: "var(--text-dim)",
                    }}
                  >
                    {p.source === "dev" ? t("plugins.sourceDev") : t("plugins.sourceInstalled")}
                  </span>
                  <span
                    style={{
                      marginLeft: "auto",
                      fontSize: 11,
                      fontWeight: 500,
                      color: statusColor(p),
                    }}
                  >
                    {statusLabel(p)}
                  </span>
                </div>

                {p.description && (
                  <div style={{ fontSize: 11.5, color: "var(--text-dim)", lineHeight: 1.45 }}>
                    {p.description}
                  </div>
                )}

                <div
                  style={{
                    fontSize: 10.5,
                    color: "var(--text-dim)",
                    fontFamily: "var(--font-mono)",
                    overflowWrap: "anywhere",
                  }}
                >
                  {p.path}
                </div>

                {(p.cli.length > 0 || p.ptySchemes.length > 0 || p.skills.length > 0) && (
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                    {p.cli.map((c) => (
                      <span
                        key={`cli-${c}`}
                        style={{
                          fontSize: 10.5,
                          padding: "1px 6px",
                          borderRadius: 4,
                          background: "var(--bg-2)",
                          border: "1px solid var(--border)",
                          color: "var(--text-dim)",
                        }}
                      >
                        CLI: {c}
                      </span>
                    ))}
                    {p.ptySchemes.map((s) => (
                      <span
                        key={`pty-${s}`}
                        style={{
                          fontSize: 10.5,
                          padding: "1px 6px",
                          borderRadius: 4,
                          background: "var(--bg-2)",
                          border: "1px solid var(--border)",
                          color: "var(--text-dim)",
                        }}
                      >
                        PTY: {s}://
                      </span>
                    ))}
                    {p.skills.map((sk) => (
                      <span
                        key={`skill-${sk}`}
                        style={{
                          fontSize: 10.5,
                          padding: "1px 6px",
                          borderRadius: 4,
                          background: "var(--bg-2)",
                          border: "1px solid var(--border)",
                          color: "var(--text-dim)",
                        }}
                      >
                        Skill: {sk}
                      </span>
                    ))}
                  </div>
                )}

                {p.lastError && (
                  <div style={{ fontSize: 11, color: "var(--red)", overflowWrap: "anywhere" }}>
                    {p.lastError}
                  </div>
                )}

                {canManage && (
                  <div style={{ display: "flex", alignItems: "center", gap: 6, marginTop: 2 }}>
                    <button
                      type="button"
                      className={p.enabled ? "vlx-btn" : "vlx-btn vlx-btn-primary"}
                      disabled={busy}
                      onClick={() =>
                        void runAction(p.id, () => pluginSetEnabled(p.id, !p.enabled))
                      }
                    >
                      {p.enabled ? t("plugins.disable") : t("plugins.enable")}
                    </button>
                    {p.enabled && (
                      <button
                        type="button"
                        className="vlx-btn"
                        disabled={busy}
                        onClick={() => void runAction(p.id, () => pluginReload(p.id))}
                      >
                        <Icons.restart size={12} /> {t("plugins.reload")}
                      </button>
                    )}
                    <button
                      type="button"
                      className="vlx-btn plugins-panel-btn-danger"
                      disabled={busy}
                      onClick={() => {
                        setConfirmUninstall(p);
                        setRemoveData(false);
                      }}
                    >
                      {t("plugins.uninstall")}
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {confirmUninstall && (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 8,
            padding: 12,
            borderRadius: 8,
            border: "1px solid color-mix(in srgb, var(--red) 45%, transparent)",
            background: "var(--bg-hover)",
          }}
        >
          <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text)" }}>
            {t("plugins.uninstallConfirm", confirmUninstall.name)}
          </span>
          <label
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              fontSize: 11.5,
              color: "var(--text-dim)",
              cursor: "pointer",
            }}
          >
            <input
              type="checkbox"
              checked={removeData}
              onChange={(e) => setRemoveData(e.target.checked)}
            />
            {t("plugins.removeData")}
          </label>
          <div style={{ display: "flex", gap: 6 }}>
            <button
              type="button"
              className="vlx-btn plugins-panel-btn-danger-solid"
              disabled={busyId !== null}
              onClick={async () => {
                const target = confirmUninstall;
                setConfirmUninstall(null);
                await runAction(target.id, () => pluginUninstall(target.id, removeData));
              }}
            >
              {t("plugins.uninstall")}
            </button>
            <button
              type="button"
              className="vlx-btn"
              onClick={() => setConfirmUninstall(null)}
            >
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
