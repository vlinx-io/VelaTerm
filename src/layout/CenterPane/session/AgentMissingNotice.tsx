//! Inline guidance for a conversation whose agent executable cannot be started.
//!
//! The terminal view answers the same condition with its install card, but only a terminal has a shell
//! to run the recipe in. This notice therefore explains the state and hands installation over to the
//! terminal view, where the existing card performs it and takes care of detecting the new path. An agent
//! without a terminal interface cannot go there, so the backend runs its recipe instead and the notice
//! waits for it; if that fails, it shows the command to run by hand.

import { useEffect, useState } from "react";

import { useT } from "../../../i18n";
import { agentInstallRecipe, agentInstallRun, type AgentInstallRecipe } from "../../../ipc/commands";
import { copyText } from "../../../ipc/transport";
import { platform } from "../../../platform";
import type { Session } from "../../../types";
import { AGENT_KIND_LABEL, kindIconEl } from "../../sessionViewers/sessionMeta";

/** Missing-agent notice shown in the conversation when a send failed for lack of an executable. */
export function AgentMissingNotice({
  session,
  onInstall,
  onInstalled,
  onDismiss,
}: {
  session: Session;
  /** One-click installation in the terminal view; absent when the agent has no terminal interface. */
  onInstall?: () => void;
  /** Called once an installation run here has finished; used when there is no terminal view to go to. */
  onInstalled?: () => void;
  /** Absent while the conversation cannot send at all: then there is nothing else to do here. */
  onDismiss?: () => void;
}) {
  const t = useT();
  const [recipe, setRecipe] = useState<AgentInstallRecipe | null>(null);
  const [run, setRun] = useState<"idle" | "running" | "failed">("idle");

  useEffect(() => {
    let alive = true;
    void agentInstallRecipe(session.kind)
      .then((r) => {
        if (alive) setRecipe(r);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [session.kind]);

  const label = recipe?.label ?? AGENT_KIND_LABEL[session.kind] ?? session.kind;
  const hasInstallCommand = !!recipe?.command.trim();
  const installHere = !onInstall && !!onInstalled && hasInstallCommand && run !== "failed";
  const openDocs = () => {
    if (recipe) void platform.opener.openExternal(recipe.docsUrl).catch(() => {});
  };
  const install = () => {
    if (onInstall) return onInstall();
    setRun("running");
    agentInstallRun(session.kind).then(
      () => {
        setRun("idle");
        onInstalled?.();
      },
      () => setRun("failed"),
    );
  };

  return (
    <div className="sv-agent-missing" role="status">
      <div className="sv-agent-missing-head">
        {kindIconEl(session.kind, 14)}
        <span className="sv-agent-missing-title">{t("agentInstall.title", label)}</span>
      </div>
      <div className="sv-agent-missing-desc">{t("agentInstall.desc", label)}</div>
      {installHere && recipe?.needsNode && <div className="sv-agent-missing-desc">{t("agentInstall.needsNode")}</div>}
      {run === "running" && <div className="sv-agent-missing-desc">{t("agentInstall.installingDesc")}</div>}
      {!onInstall && recipe && hasInstallCommand && !installHere && (
        <>
          <div className="sv-agent-missing-desc">
            {t(run === "failed" ? "agentInstall.installFailed" : "agentInstall.runInTerminal")}
            {run !== "failed" && recipe.needsNode && <> {t("agentInstall.needsNode")}</>}
          </div>
          <div className="sv-agent-missing-command">
            <code>{recipe.command}</code>
            <button className="vlx-btn" onClick={() => void copyText(recipe.command, { reportFailure: true })}>
              {t("common.copy")}
            </button>
          </div>
        </>
      )}
      <div className="sv-agent-missing-actions">
        {hasInstallCommand && (onInstall || installHere) && (
          <button className="vlx-btn vlx-btn-primary" onClick={install} disabled={run === "running"}>
            {t(run === "running" ? "agentInstall.installing" : "agentInstall.install")}
          </button>
        )}
        <button className="vlx-btn" onClick={openDocs} disabled={!recipe}>
          {t("agentInstall.docs")}
        </button>
        <span className="sv-agent-missing-spacer" />
        {onDismiss && <button className="sv-agent-missing-link" onClick={onDismiss}>
          {t("agentInstall.dismiss")}
        </button>}
      </div>
    </div>
  );
}
