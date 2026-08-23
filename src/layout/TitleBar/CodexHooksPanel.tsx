import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import {
  codexHooksList,
  codexHookUpdate,
  type CodexHook,
  type CodexHooksResponse,
} from "../../ipc/commands";
import { SectionTitle } from "./settingsParts";

interface CodexHooksPanelProps {
  cwds: string[];
  codexPath: string;
}

function eventLabel(eventName: string): string {
  return eventName
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/^./, (character) => character.toUpperCase());
}

function uniqueHooks(response: CodexHooksResponse | null): CodexHook[] {
  const hooks = new Map<string, CodexHook>();
  for (const entry of response?.data ?? []) {
    for (const hook of entry.hooks) {
      if (!hooks.has(hook.key)) hooks.set(hook.key, hook);
    }
  }
  return Array.from(hooks.values()).sort((left, right) => {
    const source = left.sourcePath.localeCompare(right.sourcePath);
    return source || left.displayOrder - right.displayOrder;
  });
}

function sourceLabel(source: CodexHook["source"], t: ReturnType<typeof useT>): string {
  if (source === "velaterm") return "VelaTerm";
  if (source === "user") return t("settings.codexHooksUser");
  if (source === "project") return t("settings.codexHooksProject");
  if (source === "plugin") return t("settings.codexHooksPlugin");
  if (source === "sessionFlags") return t("settings.codexHooksSession");
  return t("settings.codexHooksManagedSource");
}

function trustLabel(hook: CodexHook, t: ReturnType<typeof useT>): string {
  if (hook.isManaged || hook.trustStatus === "managed") return t("settings.codexHooksManaged");
  if (hook.trustStatus === "trusted") return t("settings.codexHooksTrusted");
  if (hook.trustStatus === "modified") return t("settings.codexHooksChanged");
  return t("settings.codexHooksReview");
}

function trustColor(hook: CodexHook): string {
  if (!hook.enabled) return "var(--text-dim)";
  if (hook.isManaged || hook.trustStatus === "trusted") return "var(--accent)";
  if (hook.trustStatus === "modified") return "var(--danger, #e05252)";
  return "var(--warning, #d99a36)";
}

function hookStatusLabel(hook: CodexHook, t: ReturnType<typeof useT>): string {
  if (!hook.enabled) return t("settings.codexHooksDisabled");
  return trustLabel(hook, t);
}

export function CodexHooksPanel({ cwds, codexPath }: CodexHooksPanelProps) {
  const t = useT();
  const cwdKey = cwds.filter(Boolean).join("\u0000");
  const stableCwds = useMemo(() => cwdKey.split("\u0000").filter(Boolean), [cwdKey]);
  const [response, setResponse] = useState<CodexHooksResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pendingKey, setPendingKey] = useState<string | null>(null);
  const [expandedSources, setExpandedSources] = useState<Set<string>>(() => new Set());
  // Requests are not cancellable, so a slow earlier reply must not overwrite a later one.
  const latestRequest = useRef(0);

  const load = useCallback(async () => {
    const request = ++latestRequest.current;
    setLoading(true);
    setError(null);
    try {
      const next = await codexHooksList({
        cwds: stableCwds,
        codexPath: codexPath.trim() || undefined,
      });
      if (request === latestRequest.current) setResponse(next);
    } catch (loadError) {
      if (request === latestRequest.current) setError(String(loadError));
    } finally {
      if (request === latestRequest.current) setLoading(false);
    }
  }, [codexPath, stableCwds]);

  useEffect(() => {
    void load();
  }, [load]);

  const update = async (
    hook: CodexHook,
    state: { enabled?: boolean; trustedHash?: string },
  ) => {
    const request = ++latestRequest.current;
    setPendingKey(hook.key);
    setError(null);
    try {
      const next = await codexHookUpdate({
        cwds: stableCwds,
        codexPath: codexPath.trim() || undefined,
        key: hook.key,
        ...state,
      });
      if (request === latestRequest.current) setResponse(next);
    } catch (updateError) {
      if (request === latestRequest.current) setError(String(updateError));
    } finally {
      if (request === latestRequest.current) setPendingKey(null);
    }
  };

  const hooks = uniqueHooks(response);
  const groups = new Map<string, CodexHook[]>();
  for (const hook of hooks) {
    const group = groups.get(hook.sourcePath) ?? [];
    group.push(hook);
    groups.set(hook.sourcePath, group);
  }
  const notices = Array.from(
    new Set(
      (response?.data ?? []).flatMap((entry) => [
        ...entry.warnings,
        ...entry.errors.map((entryError) => `${entryError.path}: ${entryError.message}`),
      ]),
    ),
  );

  return (
    <section style={{ marginTop: 24 }} aria-labelledby="codex-hooks-title">
      <div id="codex-hooks-title">
        <SectionTitle>{t("settings.codexHooks")}</SectionTitle>
      </div>

      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          margin: "-4px 0 12px",
        }}
      >
        <p
          id="codex-hooks-description"
          style={{
            minWidth: 0,
            flex: 1,
            margin: 0,
            color: "var(--text-dim)",
            fontSize: 11,
            lineHeight: 1.5,
          }}
        >
          {t("settings.codexHooksDesc")}
        </p>
        <button
          type="button"
          onClick={() => void load()}
          disabled={loading || pendingKey !== null}
          aria-label={t("common.refresh")}
          aria-describedby="codex-hooks-description"
          title={t("common.refresh")}
          style={{
            width: 28,
            height: 28,
            flex: "0 0 28px",
            display: "grid",
            placeItems: "center",
            border: "1px solid var(--border)",
            borderRadius: 6,
            background: "var(--bg-active)",
            color: "var(--text-dim)",
            cursor: loading || pendingKey !== null ? "default" : "pointer",
            opacity: loading || pendingKey !== null ? 0.55 : 1,
          }}
        >
          <Icons.restart size={13} />
        </button>
      </div>

      <div aria-live="polite">
        {loading && hooks.length === 0 && (
          <div style={{ padding: "16px 0", color: "var(--text-dim)", fontSize: 11.5 }}>
            {t("common.loading")}
          </div>
        )}

        {error && (
          <div
            role="alert"
            style={{
              marginBottom: 10,
              padding: "8px 10px",
              border: "1px solid color-mix(in srgb, var(--danger, #e05252) 45%, transparent)",
              borderRadius: 7,
              color: "var(--danger, #e05252)",
              background: "color-mix(in srgb, var(--danger, #e05252) 8%, transparent)",
              fontSize: 11,
              lineHeight: 1.45,
              overflowWrap: "anywhere",
            }}
          >
            {error}
          </div>
        )}

        {!loading && !error && hooks.length === 0 && (
          <div
            style={{
              padding: "18px 14px",
              border: "1px dashed var(--border-strong)",
              borderRadius: 8,
              color: "var(--text-dim)",
              textAlign: "center",
              fontSize: 11.5,
            }}
          >
            {t("settings.codexHooksEmpty")}
          </div>
        )}

        {Array.from(groups.entries()).map(([sourcePath, sourceHooks], groupIndex) => {
          const source = sourceHooks[0];
          const expanded = expandedSources.has(sourcePath);
          const enabledCount = sourceHooks.filter((hook) => hook.enabled).length;
          const sourceName = sourceLabel(source.source, t);
          const countLabel = t(
            "settings.codexHooksEnabledCount",
            enabledCount,
            sourceHooks.length,
          );
          const contentId = `codex-hook-group-${groupIndex}`;
          return (
            <div
              key={sourcePath}
              style={{
                marginBottom: 6,
                border: "1px solid var(--border)",
                borderRadius: 7,
                background: "color-mix(in srgb, var(--bg-active) 48%, transparent)",
                overflow: "hidden",
              }}
            >
              <button
                type="button"
                aria-expanded={expanded}
                aria-controls={contentId}
                aria-label={`${sourceName}, ${countLabel}`}
                onClick={() =>
                  setExpandedSources((current) => {
                    const next = new Set(current);
                    if (next.has(sourcePath)) next.delete(sourcePath);
                    else next.add(sourcePath);
                    return next;
                  })
                }
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  width: "100%",
                  minHeight: 32,
                  padding: "4px 8px",
                  border: "none",
                  borderBottom: expanded ? "1px solid var(--border)" : "none",
                  background: "transparent",
                  color: "inherit",
                  textAlign: "left",
                  cursor: "pointer",
                }}
              >
                {expanded ? <Icons.chevD size={11} /> : <Icons.chevR size={11} />}
                <span
                  style={{
                    color: "var(--text)",
                    fontSize: 11,
                    fontWeight: 600,
                    flex: "none",
                  }}
                >
                  {sourceName}
                </span>
                <span
                  title={sourcePath}
                  style={{
                    minWidth: 0,
                    flex: 1,
                    color: "var(--text-dim)",
                    fontSize: 9.5,
                    fontFamily: "var(--font-mono, monospace)",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {sourcePath}
                </span>
                <span style={{ color: "var(--text-dim)", fontSize: 9.5, flex: "none" }}>
                  {countLabel}
                </span>
              </button>

              {expanded && (
                <div id={contentId}>
                  {sourceHooks.map((hook, index) => {
                    const busy = pendingKey !== null;
                    const canTrust = !hook.isManaged && hook.trustStatus !== "trusted";
                    const label = eventLabel(hook.eventName);
                    return (
                      <div
                        key={hook.key}
                        aria-disabled={hook.isManaged}
                        style={{
                          borderTop: index === 0 ? "none" : "1px solid var(--border)",
                          opacity: hook.isManaged ? 0.55 : 1,
                        }}
                      >
                        <div
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: 8,
                            padding: "7px 8px 5px",
                          }}
                        >
                          <span
                            style={{
                              minWidth: 0,
                              flex: 1,
                              color: "var(--text)",
                              fontSize: 11.5,
                              fontWeight: 600,
                            }}
                          >
                            {label}
                          </span>
                          <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
                            {canTrust && (
                              <button
                                type="button"
                                onClick={() =>
                                  void update(hook, { trustedHash: hook.currentHash })
                                }
                                disabled={busy}
                                aria-label={t("settings.codexHooksTrust")}
                                style={{
                                  height: 24,
                                  padding: "0 7px",
                                  border: "1px solid var(--border)",
                                  borderRadius: 6,
                                  background: "var(--bg-active)",
                                  color: "var(--text)",
                                  fontSize: 10,
                                  cursor: busy ? "default" : "pointer",
                                  opacity: busy ? 0.55 : 1,
                                }}
                              >
                                {t("settings.codexHooksTrust")}
                              </button>
                            )}
                            {hook.isManaged && (
                              <Icons.lock size={11} style={{ color: "var(--text-dim)" }} />
                            )}
                            <span
                              style={{
                                color: trustColor(hook),
                                fontSize: 9,
                                fontWeight: 600,
                                letterSpacing: "0.02em",
                                lineHeight: 1.4,
                                padding: "1px 5px",
                                border: `1px solid color-mix(in srgb, ${trustColor(hook)} 28%, transparent)`,
                                borderRadius: 4,
                                background: `color-mix(in srgb, ${trustColor(hook)} 12%, transparent)`,
                              }}
                            >
                              {hookStatusLabel(hook, t)}
                            </span>
                            <button
                              type="button"
                              role="switch"
                              aria-label={`${label} hook`}
                              aria-checked={hook.enabled}
                              disabled={hook.isManaged || busy}
                              onClick={() => void update(hook, { enabled: !hook.enabled })}
                              style={{
                                width: 32,
                                height: 24,
                                padding: 0,
                                border: "none",
                                display: "grid",
                                placeItems: "center",
                                background: "transparent",
                                cursor: hook.isManaged || busy ? "default" : "pointer",
                                opacity: hook.isManaged || busy ? 0.55 : 1,
                              }}
                            >
                              <span
                                aria-hidden="true"
                                style={{
                                  display: "flex",
                                  alignItems: "center",
                                  width: 28,
                                  height: 16,
                                  padding: 2,
                                  borderRadius: 999,
                                  background: hook.enabled
                                    ? "var(--accent)"
                                    : "var(--border-strong)",
                                }}
                              >
                                <span
                                  style={{
                                    width: 12,
                                    height: 12,
                                    borderRadius: "50%",
                                    background: "white",
                                    transform: hook.enabled
                                      ? "translateX(12px)"
                                      : "translateX(0)",
                                    boxShadow: "0 1px 2px rgba(0,0,0,0.25)",
                                  }}
                                />
                              </span>
                            </button>
                          </div>
                        </div>

                        <dl
                          style={{
                            display: "grid",
                            gridTemplateColumns: "72px minmax(0, 1fr)",
                            columnGap: 10,
                            rowGap: 4,
                            margin: "0 8px 8px",
                            padding: "6px 8px",
                            border: "1px solid var(--border)",
                            borderRadius: 6,
                            background:
                              "color-mix(in srgb, var(--bg-active) 30%, transparent)",
                          }}
                        >
                          <dt style={{ color: "var(--text-dim)", fontSize: 9.5 }}>
                            {t("settings.codexHooksHandler")}
                          </dt>
                          <dd style={{ margin: 0, color: "var(--text)", fontSize: 10 }}>
                            {eventLabel(hook.handlerType)}
                          </dd>
                          <dt style={{ color: "var(--text-dim)", fontSize: 9.5 }}>
                            {t("settings.codexHooksCommand")}
                          </dt>
                          <dd
                            title={hook.command ?? hook.handlerType}
                            style={{
                              minWidth: 0,
                              margin: 0,
                              color: "var(--text-dim)",
                              fontSize: 10,
                              fontFamily: "var(--font-mono, monospace)",
                              lineHeight: 1.4,
                              overflowWrap: "anywhere",
                            }}
                          >
                            {hook.command ?? hook.handlerType}
                          </dd>
                          <dt style={{ color: "var(--text-dim)", fontSize: 9.5 }}>
                            {t("settings.codexHooksMatcher")}
                          </dt>
                          <dd style={{ margin: 0, color: "var(--text-dim)", fontSize: 10 }}>
                            {hook.matcher || t("settings.codexHooksAll")}
                          </dd>
                          <dt style={{ color: "var(--text-dim)", fontSize: 9.5 }}>
                            {t("settings.codexHooksTimeout")}
                          </dt>
                          <dd style={{ margin: 0, color: "var(--text-dim)", fontSize: 10 }}>
                            {hook.timeoutSec}s
                          </dd>
                          <dt style={{ color: "var(--text-dim)", fontSize: 9.5 }}>
                            {t("settings.codexHooksStatusMessage")}
                          </dt>
                          <dd style={{ margin: 0, color: "var(--text-dim)", fontSize: 10 }}>
                            {hook.statusMessage || t("settings.codexHooksNone")}
                          </dd>
                        </dl>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}

        {notices.length > 0 && (
          <div style={{ marginTop: 8, color: "var(--warning, #d99a36)", fontSize: 10.5, lineHeight: 1.45, whiteSpace: "pre-wrap" }}>
            {notices.join("\n")}
          </div>
        )}
      </div>
    </section>
  );
}
