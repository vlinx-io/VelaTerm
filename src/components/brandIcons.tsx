//! Official solid agent brand icons alongside the outline system in Icons.tsx. Most paths come from
//! simple-icons and tree-shake in production. A 24-unit viewBox with currentColor fill follows theme
//! and accent colors. simple-icons excludes OpenAI, so OPENAI_PATH embeds the official Codex mark.
//! Session-tree and Usage source icons share these assets; Claude Usage uses its orange siClaude star
//! while the session tree retains the robot icon.
import { siClaude, siCline, siCursor, siGithubcopilot, siOpencode } from "simple-icons";
import type { SimpleIcon } from "simple-icons";

/**
 * VelaTerm's sail without the app-icon tile. Because the artwork is tall and narrow, callers use a
 * slightly larger SVG box than square agent marks so their visible areas—not merely their boxes—balance.
 */
function VelaSailMark({ size = 14 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="20 12 66 82"
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
    >
      <rect x="29.5" y="20" width="6" height="62" rx="3" opacity="0.72" />
      <rect x="29.5" y="80" width="25" height="6.5" rx="3.25" opacity="0.88" />
      <path d="M 40 24 Q 67 30 76 51 Q 65 70 40 74 Z" />
      <path
        d="M 40 49 Q 58 49 70 53"
        fill="none"
        stroke="var(--bg-0)"
        strokeWidth="2.2"
        strokeLinecap="round"
        opacity="0.5"
      />
    </svg>
  );
}

/** Compact VelaTerm identity mark for places where the full app tile is visually too heavy. */
export function velaSailMarkEl(size = 14) {
  return <VelaSailMark size={size} />;
}

/**
 * Preserve distinctive brand colors such as Claude orange. Near-black/white monochrome marks for
 * OpenCode, Copilot, and Cursor use currentColor so they remain visible in either theme.
 */
function brandFill(hex: string): string {
  const n = Number.parseInt(hex, 16);
  if (Number.isNaN(n)) return "currentColor";
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return lum < 0.22 || lum > 0.85 ? "currentColor" : `#${hex}`;
}

function BrandIcon({ icon, size = 14 }: { icon: SimpleIcon; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={brandFill(icon.hex)}
      role="img"
      aria-label={icon.title}
    >
      <path d={icon.path} />
    </svg>
  );
}

/**
 * Map agent types to official simple-icons marks; callers provide fallbacks for others. Claude is
 * intentionally absent because sessionMeta.kindIconEl uses the preferred robot icon.
 */
const BRAND: Record<string, SimpleIcon> = {
  opencode: siOpencode,
  copilot: siGithubcopilot,
  cursor: siCursor,
  // Cline's near-black #18181B mark falls back to currentColor through brandFill.
  cline: siCline,
};

/** Return an official agent brand icon, or null for the caller's fallback. */
export function brandIconEl(name: string, size = 14) {
  const icon = BRAND[name];
  return icon ? <BrandIcon icon={icon} size={size} /> : null;
}

/**
 * Official OpenAI rosette omitted by simple-icons for brand-policy reasons and embedded manually.
 * Its 24-unit viewBox and currentColor fill follow the caller's theme-aware Codex foreground.
 */
const OPENAI_PATH =
  "M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z";

function OpenAiMark({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" role="img" aria-label="OpenAI">
      <path d={OPENAI_PATH} />
    </svg>
  );
}

/**
 * Codex OpenAI icon using currentColor, shared by Usage and sessionMeta for consistent appearance.
 */
export function openAiMarkEl(size = 14) {
  return <OpenAiMark size={size} />;
}

/**
 * Hand-drawn Pi mark because simple-icons has no Pi coding-agent logo. A bold Greek π SVG text glyph
 * uses currentColor, typically the session tree's amber KIND_COLOR.
 */
function PiMark({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" role="img" aria-label="Pi">
      <text
        x="12"
        y="19"
        textAnchor="middle"
        fontSize="22"
        fontWeight="700"
        fontFamily="Georgia, 'Times New Roman', serif"
      >
        π
      </text>
    </svg>
  );
}

/** Pi glyph icon using the caller's currentColor. */
export function piMarkEl(size = 14) {
  return <PiMark size={size} />;
}

/**
 * OMP (oh-my-pi) mark, redrawn from the project's own `assets/icon.svg`: a blocky pi whose shortened right
 * leg ends in a plug, for the extension surface the CLI is built around. Monochrome through currentColor,
 * typically the session tree's orange KIND_COLOR, with the plug's two pins cut out so they read at larger
 * sizes. The square viewBox keeps it the same visual weight as the other 24-unit marks.
 */
function OmpMark({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 120 120" fill="currentColor" role="img" aria-label="OMP">
      <rect x="10" y="23" width="100" height="12" rx="2" />
      <rect x="25" y="35" width="12" height="62" rx="2" />
      <rect x="75" y="35" width="12" height="45" rx="2" />
      <path
        fillRule="evenodd"
        d="M71 70h20v16H71z M76 74h3v8h-3z M82 74h3v8h-3z"
      />
    </svg>
  );
}

/** OMP plug-tipped pi mark using the caller's currentColor. */
export function ompMarkEl(size = 14) {
  return <OmpMark size={size} />;
}

/**
 * Hand-drawn Google Antigravity CLI (`agy`) mark: a double up-chevron evokes upward motion and stays
 * distinct from other logos. It uses currentColor, typically the session tree's blue KIND_COLOR.
 */
function AntigravityMark({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" role="img" aria-label="Antigravity">
      <path d="M12 3 L21 12 L18 12 L12 6 L6 12 L3 12 Z M12 10 L21 19 L18 19 L12 13 L6 19 L3 19 Z" />
    </svg>
  );
}

/** Antigravity upward mark using the caller's currentColor. */
export function antigravityMarkEl(size = 14) {
  return <AntigravityMark size={size} />;
}

/**
 * Hand-drawn solid heart for charmbracelet/crush, reflecting its love theme while remaining distinct
 * from chevrons, π, rosettes, and robots. It uses currentColor, typically pink KIND_COLOR.
 */
function CrushMark({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" role="img" aria-label="Crush">
      <path d="M12 21C12 21 3 14.5 3 8.5C3 5.46 5.24 3.5 7.8 3.5C9.6 3.5 11.1 4.6 12 6C12.9 4.6 14.4 3.5 16.2 3.5C18.76 3.5 21 5.46 21 8.5C21 14.5 12 21 12 21Z" />
    </svg>
  );
}

/** Crush heart mark using the caller's currentColor. */
export function crushMarkEl(size = 14) {
  return <CrushMark size={size} />;
}

/** Kimi Code crescent-and-star mark reflecting the Kimi/Moonshot identity. */
function KimiMark({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" role="img" aria-label="Kimi Code">
      <path d="M15.8 2.7A9.6 9.6 0 1 0 21.3 17 8.15 8.15 0 1 1 15.8 2.7Z" />
      <path d="m18.1 5 .65 1.55L20.3 7.2l-1.55.65-.65 1.55-.65-1.55-1.55-.65 1.55-.65Z" />
    </svg>
  );
}

export function kimiMarkEl(size = 14) {
  return <KimiMark size={size} />;
}

/**
 * Official Grok mark from the current grok.com SVG wordmark. The source SVG labels these two paths
 * `mark` and hides the remaining "Grok" word paths at compact sizes, so use that provided icon form
 * unchanged. Its original currentColor fill follows the official black/light monochrome treatment.
 * Source: https://grok.com (Grok-feb-2025-logo.svg)
 */
function GrokMark({ size = 14 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 34 33"
      fill="currentColor"
      role="img"
      aria-label="Grok"
    >
      <path
        d="M13.2371 21.0407L24.3186 12.8506C24.8619 12.4491 25.6384 12.6057 25.8973 13.2294C27.2597 16.5185 26.651 20.4712 23.9403 23.1851C21.2297 25.8989 17.4581 26.4941 14.0108 25.1386L10.2449 26.8843C15.6463 30.5806 22.2053 29.6665 26.304 25.5601C29.5551 22.3051 30.562 17.8683 29.6205 13.8673L29.629 13.8758C28.2637 7.99809 29.9647 5.64871 33.449 0.844576C33.5314 0.730667 33.6139 0.616757 33.6964 0.5L29.1113 5.09055V5.07631L13.2343 21.0436"
      />
      <path d="M10.9503 23.0313C7.07343 19.3235 7.74185 13.5853 11.0498 10.2763C13.4959 7.82722 17.5036 6.82767 21.0021 8.2971L24.7595 6.55998C24.0826 6.07017 23.215 5.54334 22.2195 5.17313C17.7198 3.31926 12.3326 4.24192 8.67479 7.90126C5.15635 11.4239 4.0499 16.8403 5.94992 21.4622C7.36924 24.9165 5.04257 27.3598 2.69884 29.826C1.86829 30.7002 1.0349 31.5745 0.36364 32.5L10.9474 23.0341" />
    </svg>
  );
}

export function grokMarkEl(size = 14) {
  return <GrokMark size={size} />;
}

/** Zoo Code mark using a bold Z and two nodes instead of the discontinued Roo artwork. */
function ZooMark({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" role="img" aria-label="Zoo Code">
      <path d="M5 4h14v3L10.2 17H19v3H5v-3L13.8 7H5V4Z" />
      <circle cx="5" cy="4" r="2" />
      <circle cx="19" cy="20" r="2" />
    </svg>
  );
}

export function zooMarkEl(size = 14) {
  return <ZooMark size={size} />;
}

/**
 * Kiro mark drawn as a simple ghost silhouette, matching the spirit Kiro uses as its identity. simple-icons
 * carries no Kiro entry, so this is an original shape rather than a reproduction of the official artwork.
 */
function KiroMark({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" role="img" aria-label="Kiro">
      <path d="M12 2a8 8 0 0 0-8 8v11.1c0 .7.8 1.1 1.4.7l2.1-1.6 2.1 1.6c.4.3.9.3 1.2 0l2.1-1.6 2.1 1.6c.6.4 1.4 0 1.4-.7V10a8 8 0 0 0-8-8Zm-2.8 8.4a1.3 1.3 0 1 1 0 2.6 1.3 1.3 0 0 1 0-2.6Zm5.6 0a1.3 1.3 0 1 1 0 2.6 1.3 1.3 0 0 1 0-2.6Z" />
    </svg>
  );
}

export function kiroMarkEl(size = 14) {
  return <KiroMark size={size} />;
}

/**
 * Codex/OpenAI identity color shared across the application: soft gray in dark mode, periwinkle in light mode.
 */
export const CODEX_BRAND_COLOR = "var(--codex-brand-color, #6B78EB)";

/**
 * Grok's official mark is monochrome. The CSS token is black in light mode and reverses to white in
 * dark mode so the mark keeps its brand treatment without disappearing into the application chrome.
 */
export const GROK_BRAND_COLOR = "var(--grok-brand-color)";

/**
 * Usage source icons: Claude uses the official orange siClaude star and Codex uses the OpenAI mark in
 * CODEX_BRAND_COLOR, matching the session tree. Grok uses its theme-aware official monochrome color.
 * Claude intentionally differs from its tree robot. Return null for other agents so callers can fall back.
 */
export function usageBrandIconEl(kind: string, size = 14) {
  if (kind === "claude") return <BrandIcon icon={siClaude} size={size} />;
  if (kind === "codex")
    return (
      <span style={{ color: CODEX_BRAND_COLOR, display: "inline-flex" }}>
        <OpenAiMark size={size} />
      </span>
    );
  if (kind === "grok")
    return (
      <span style={{ color: GROK_BRAND_COLOR, display: "inline-flex" }}>
        <GrokMark size={size} />
      </span>
    );
  return null;
}

// Earlier custom orange Claude robots (pixel and rounded) were unsatisfactory, so sessionMeta uses the
// generic outline Icons.bot. A future colored robot can be added here as a separate component.
