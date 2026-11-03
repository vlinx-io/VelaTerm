import type { ReactNode } from "react";

import { useT } from "../../../i18n";
import { SessionLink } from "./links";

/** One problem a review command reported, in the shape Claude Code's `ReportFindings` hands to its host. */
export interface CommandFinding {
  file: string;
  line?: number;
  summary: string;
  failureScenario?: string;
  category?: string;
  verdict?: string;
}

/**
 * The findings a slash command printed, or null when its output is anything else.
 *
 * `/code-review` run in the foreground prints its findings as a fenced JSON array instead of prose. The host
 * is meant to draw them as a list; shown as Markdown they read as a wall of escaped JSON. Only an array whose
 * every entry names a file and a summary qualifies, so ordinary command output is never mistaken for it.
 */
export function parseCommandFindings(text: string): CommandFinding[] | null {
  const fenced = /^```(?:json)?\s*\n([\s\S]*?)\n```$/.exec(text.trim());
  const body = (fenced ? fenced[1] : text).trim();
  if (!body.startsWith("[")) return null;
  let value: unknown;
  try { value = JSON.parse(body); } catch { return null; }
  if (!Array.isArray(value)) return null;
  const findings: CommandFinding[] = [];
  for (const item of value) {
    if (!item || typeof item !== "object") return null;
    const entry = item as Record<string, unknown>;
    if (typeof entry.file !== "string" || typeof entry.summary !== "string") return null;
    findings.push({
      file: entry.file,
      line: typeof entry.line === "number" && entry.line > 0 ? entry.line : undefined,
      summary: entry.summary,
      failureScenario: typeof entry.failure_scenario === "string" ? entry.failure_scenario : undefined,
      category: typeof entry.category === "string" ? entry.category : undefined,
      verdict: typeof entry.verdict === "string" ? entry.verdict : undefined,
    });
  }
  // An empty array is a review that found nothing, which is only recognisable when it was fenced as JSON.
  if (!findings.length && !fenced) return null;
  return findings;
}

/** A review's findings, most severe first as reported, each with its location and how it goes wrong. */
export function CommandFindings({ findings }: { findings: CommandFinding[] }): ReactNode {
  const t = useT();
  if (!findings.length) return <div className="sv-findings-none">{t("chat.findings.none")}</div>;
  return (
    <div className="sv-findings">
      <div className="sv-findings-head">{t("chat.findings.count", findings.length)}</div>
      <ol className="sv-findings-list">
        {findings.map((finding, i) => {
          const location = finding.line ? `${finding.file}:${finding.line}` : finding.file;
          return (
            <li key={i} className="sv-finding">
              <div className="sv-finding-meta">
                <SessionLink href={location}><code>{location}</code></SessionLink>
                {finding.category && <span className="sv-finding-tag">{finding.category}</span>}
                {finding.verdict && <span className="sv-finding-tag">{finding.verdict}</span>}
              </div>
              <div className="sv-finding-summary">{finding.summary}</div>
              {finding.failureScenario && <div className="sv-finding-scenario">{finding.failureScenario}</div>}
            </li>
          );
        })}
      </ol>
    </div>
  );
}
