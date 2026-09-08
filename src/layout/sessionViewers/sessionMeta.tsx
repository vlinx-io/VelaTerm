//! Session metadata helpers: list-row icons, archive time formatting, and session breadcrumb paths.
//! Shared by the archive and global search panels to keep both implementations consistent.

import { useEffect, useState } from "react";
import {
  antigravityMarkEl,
  brandIconEl,
  CODEX_BRAND_COLOR,
  crushMarkEl,
  GROK_BRAND_COLOR,
  grokMarkEl,
  kimiMarkEl,
  kiroMarkEl,
  openAiMarkEl,
  ompMarkEl,
  piMarkEl,
  zooMarkEl,
} from "../../components/brandIcons";
import Icons from "../../components/Icons";
import { useGitBranchInfo } from "../../hooks/useGitBranch";
import { getSessionCwd } from "../../ipc/commands";
import { dateLocale, useT } from "../../i18n";
import type { Session, SessionKind } from "../../types";
import { useTermStore } from "../../store/termStore";
import { sessionIconEl } from "../agentPresetIcon";

/**
 * Distinguishing colors for each agent. Claude orange is Anthropic's official color. The other
 * official logos are monochrome, so these project-selected brand-inspired colors simply make
 * agents easier to distinguish in the left tree. Grok instead keeps its official monochrome treatment
 * through a theme-aware token so it remains legible on both light and dark backgrounds.
 */
const KIND_COLOR: Partial<Record<SessionKind, string>> = {
  claude: "#D97757", // Orange (Anthropic's official color)
  codex: CODEX_BRAND_COLOR, // Theme-aware color shared with Usage (see brandIcons)
  opencode: "#3B82F6", // Blue
  copilot: "#8957E5", // Purple
  cursor: "#18B5C9", // Cyan
  // Cline's official logo is also near-black (simple-icons hex #18181B), which disappears into dark
  // backgrounds. As with the other brands, use a theme-safe accent; green is the only unused family.
  cline: "#22C55E", // Green
  pi: "#F59E0B", // Amber, distinct from the blue, purple, green, and cyan families
  // OMP's own accent orange. It sits near Claude's terracotta and Pi's amber, so the plug-tipped pi mark,
  // not the colour, is what tells it apart at a glance.
  omp: "#F97316",
  antigravity: "#4285F4", // Google blue; the chevron mark distinguishes it from OpenCode's blue flower
  crush: "#EC4899", // Pink, echoing Crush's love theme and remaining clear in both themes
  kimi: "#7C6CF2", // Moon purple, distinct from the Codex blue and Copilot purple
  grok: GROK_BRAND_COLOR, // Official black mark in light mode, reversed to white in dark mode
  zoo: "#16A085", // Teal, distinct from the existing colors and paired with the Z-shaped mark
  kiro: "#A855F7", // Violet, one step brighter than the Copilot and Kimi purples so the ghost mark reads apart
};

// Sidebar and tab icons can mount for the same session together. Share only the in-flight cwd lookup so they do
// not launch duplicate `lsof` processes on macOS; do not cache the result because an agent can change directory.
const runtimeCwdInflight = new Map<string, Promise<string | null>>();
function inspectRuntimeCwd(sessionId: string): Promise<string | null> {
  let request = runtimeCwdInflight.get(sessionId);
  if (!request) {
    request = getSessionCwd(sessionId).catch(() => null);
    runtimeCwdInflight.set(sessionId, request);
    void request.finally(() => runtimeCwdInflight.delete(sessionId));
  }
  return request;
}

/** Session type to uncolored icon element. */
function rawKindIcon(kind: SessionKind, size: number) {
  switch (kind) {
    case "terminal":
      return <Icons.terminal size={size} />;
    case "browser":
      return <Icons.globe size={size} />;
    // Claude uses the generic outlined robot icon.
    case "claude":
      return <Icons.bot size={size} />;
    // These agents use their official logos, falling back to the robot when brandIconEl lacks one.
    case "opencode":
      return brandIconEl("opencode", size) ?? <Icons.bot size={size} />;
    case "copilot":
      return brandIconEl("copilot", size) ?? <Icons.bot size={size} />;
    case "cursor":
      return brandIconEl("cursor", size) ?? <Icons.bot size={size} />;
    case "cline":
      return brandIconEl("cline", size) ?? <Icons.bot size={size} />;
    // Antigravity uses a custom rising-chevron mark because simple-icons does not include it.
    case "antigravity":
      return antigravityMarkEl(size);
    // Pi uses a custom pi-shaped mark because simple-icons does not include it.
    case "pi":
      return piMarkEl(size);
    // OMP redraws its own icon: a blocky pi with a plug on the short leg.
    case "omp":
      return ompMarkEl(size);
    // Crush uses a custom heart mark because simple-icons does not include it.
    case "crush":
      return crushMarkEl(size);
    case "kimi":
      return kimiMarkEl(size);
    case "grok":
      return grokMarkEl(size);
    case "zoo":
      return zooMarkEl(size);
    // Kiro uses a custom ghost mark because simple-icons does not include it.
    case "kiro":
      return kiroMarkEl(size);
    // Codex uses the official OpenAI logo embedded in brandIcons, matching the Usage panel.
    case "codex":
      return openAiMarkEl(size);
    default:
      return <Icons.bot size={size} />;
  }
}

/**
 * Maps session types to icons. Each agent has a dedicated icon and accent color; plain terminals use
 * a terminal and browser pages use a globe. The left tree, center tabs, creation menu, archive, and
 * search panels share this mapping. KIND_COLOR colors both outlined icons and monochrome official
 * logos (brandFill maps black to currentColor); Grok's monochrome color follows the active theme,
 * while terminal and browser icons inherit foreground color.
 */
export function kindIconEl(kind: SessionKind, size = 14) {
  const icon = rawKindIcon(kind, size);
  const color = KIND_COLOR[kind];
  return color ? <span style={{ color, display: "inline-flex" }}>{icon}</span> : icon;
}

/** Icon for a session row in the archive and global search lists. */
export function KindIcon({ session, size = 14 }: { session: Session; size?: number }) {
  return kindIconEl(session.kind, size);
}

/**
 * Session-type icon with a branch badge in the lower-right when inside a Git worktree. Shared by
 * session rows in the left sidebar and tabs in the center pane.
 *
 * Worktree membership is determined in three ways:
 * 1. Use a persisted `worktreePath` for worktrees created by VelaTerm, such as `vspawn --worktree`.
 * 2. For a running spawned child, inspect the process cwd so agent-managed directory changes are reflected.
 * 3. Otherwise probe Git at the session directory, falling back from a missing `cwd` to `rootPath`.
 *    This also covers worktrees opened directly as projects and therefore not recorded by VelaTerm.
 *
 * Only linked worktrees—not the primary working tree—receive a badge. `useGitBranchInfo` caches and
 * deduplicates by path, while sidebar virtualization limits probes to visible rows. The parent must
 * use `position: relative` to anchor the badge (see `.wt-badge` in vlinx.css).
 */
export function SessionKindIcon({
  session,
  size = 14,
  rootPath,
}: {
  session: Session;
  size?: number;
  rootPath?: string | null;
}) {
  const t = useT();
  // A session created from a preset shows the preset's icon; a deleted preset falls back to the kind's.
  const presets = useTermStore((st) => st.agentPresets);
  // Agent tools can move a running process into a worktree after launch. Subscribe to coarse runtime state
  // changes and inspect its real cwd then; this avoids polling while still refreshing after a working turn.
  const runtimeKey = useTermStore((st) => {
    const runtime = st.runtimes[session.id];
    return `${runtime?.status ?? ""}:${runtime?.agentState ?? ""}`;
  });
  const [runtimeCwd, setRuntimeCwd] = useState<string | null>(null);
  useEffect(() => {
    // This fallback exists for spawned children that enter an agent-managed worktree after launch. Persisted
    // worktrees and top-level sessions already have a reliable path and avoid the native cwd lookup entirely.
    if (
      session.worktreePath ||
      !session.parentSessionId ||
      !runtimeKey.startsWith("running:")
    ) {
      return;
    }
    let cancelled = false;
    void inspectRuntimeCwd(session.id).then((cwd) => {
      if (!cancelled) setRuntimeCwd(cwd);
    });
    return () => {
      cancelled = true;
    };
  }, [session.id, session.worktreePath, runtimeKey]);
  // A stored worktreePath is conclusive, so pass null to short-circuit the hook and avoid a Git probe.
  const probePath = session.worktreePath ? null : runtimeCwd || session.cwd || rootPath || null;
  const git = useGitBranchInfo(probePath);
  const inWorktree = !!session.worktreePath || git.isWorktree;
  const wtPath = session.worktreePath || git.worktreePath;
  const wtDir = wtPath ? (wtPath.split("/").filter(Boolean).pop() ?? wtPath) : null;
  return (
    <>
      {sessionIconEl(session, presets, size)}
      {inWorktree && (
        <span className="wt-badge" title={`${t("spawn.worktreeLabel")}${wtDir ? `: ${wtDir}` : ""}`}>
          <Icons.branch size={8} sw={2} />
        </span>
      )}
    </>
  );
}

/** Format an archive timestamp in seconds as local time, returning an empty string for no value. */
export function fmtArchivedAt(ts?: number | null): string {
  if (!ts) return "";
  return new Date(ts * 1000).toLocaleString(dateLocale());
}

/**
 * Format an ISO timestamp as a compact localized month/day and hour:minute value for search hits.
 * Return an empty string when parsing fails.
 */
export function fmtTsShort(ts?: string | null): string {
  if (!ts) return "";
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString(dateLocale(), {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * Build a session breadcrumb: "project › group chain › parent session". Omit unresolved segments.
 * Cascading deletion keeps projects and groups for archived sessions intact; this also applies to
 * live global-search results, whose parent session is resolved from `liveSessions`.
 */
export function locationOf(
  s: Session,
  projects: { id: string; name: string }[],
  groups: { id: string; name: string; parentGroupId?: string | null }[],
  liveSessions: Session[],
): string {
  const parts: string[] = [];
  const proj = projects.find((p) => p.id === s.projectId);
  if (proj) parts.push(proj.name);
  // Walk from the containing group up to the top-level group.
  const chain: string[] = [];
  let gid = s.groupId ?? null;
  while (gid) {
    const g = groups.find((x) => x.id === gid);
    if (!g) break;
    chain.unshift(g.name);
    gid = g.parentGroupId ?? null;
  }
  parts.push(...chain);
  // Include the parent session when it is still present in the session list.
  if (s.parentSessionId) {
    const parent = liveSessions.find((x) => x.id === s.parentSessionId);
    if (parent) parts.push(parent.name);
  }
  return parts.join(" › ");
}
