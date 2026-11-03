//! New Session tab: pick where and how an agent session runs, type its first task, and start it. Every
//! choice is resolved again by the backend at creation; this view only offers what the backend reports.
//!
//! The settings sit as chips on the composer's own control row, the same chips the conversation composer
//! uses, so the tab reads as "the composer of a session that does not exist yet".
import { useEffect, useId, useMemo, useRef, useState } from "react";
import Icons from "../../components/Icons";
import { useLaunchModels } from "../../components/LaunchFields";
import { permissionLabelKey } from "../../components/AgentPermissionSelect";
import { genId } from "../../genId";
import { useAgentPermissions } from "../../hooks/useAgentPermissions";
import { useGitBranchInfo } from "../../hooks/useGitBranch";
import { useT } from "../../i18n";
import { createAgentSession, prepareAgentSession, type AgentSessionContext, type LaunchOption } from "../../ipc/launch";
import { setCollapsed } from "../../ipc/tree";
import { defaultEngineFor } from "../../store/settings";
import { useTermStore, type NewSessionTab } from "../../store/termStore";
import { hasTerminalInterface, supportsChatEngine, type AgentPreset, type SessionEngine } from "../../types";
import { PresetIcon } from "../agentPresetIcon";
import { ControlChip, type ChipOption } from "../CenterPane/session/controls";
import { effortChipLabel, effortChipOptions, modelChipOptions } from "../CenterPane/session/modelChips";
import { agentChoices, recentAgentChoices, recentProjectId, rememberAgentChoice, rememberProject, type AgentChoice } from "../NewAgentSession/choices";
import { kindIconEl } from "../sessionViewers/sessionMeta";
import { assistantLabel } from "../sessionViewers/TranscriptViewer";
import { Seg } from "../TitleBar/settingsParts";
import "../CenterPane/session/session-view.css";
import "./new-session.css";

interface Prepared { key: string; options: LaunchOption[]; presets: AgentPreset[]; locationNames: string[] }

const agentGlyph = (choice: AgentChoice | undefined, size: number) =>
  choice?.preset ? <PresetIcon preset={choice.preset} size={size} /> : kindIconEl(choice?.kind ?? "claude", size);

export function NewSessionView({ tab, hidden }: { tab: NewSessionTab; hidden: boolean }) {
  const t = useT();
  const id = useId();
  const prompt = useRef<HTMLTextAreaElement>(null);
  const attempt = useRef<{ key: string; requestId: string } | null>(null);
  const projects = useTermStore((s) => s.projects);
  const groups = useTermStore((s) => s.groups);
  const sessions = useTermStore((s) => s.sessions);
  const agentDefaults = useTermStore((s) => s.agentDefaults);
  const project = projects.find((p) => p.id === tab.projectId);
  const group = tab.groupId ? groups.find((g) => g.id === tab.groupId && g.projectId === tab.projectId) : null;
  const parent = tab.parentSessionId
    ? sessions.find((s) => s.id === tab.parentSessionId && !s.archivedAt && s.projectId === tab.projectId) : null;
  const validTarget = !!project && (!tab.groupId || !!group) && (!tab.parentSessionId || !!parent);
  const context = useMemo<AgentSessionContext>(() => ({
    projectId: tab.projectId || null, groupId: tab.groupId, activeSessionId: tab.parentSessionId,
    placement: tab.parentSessionId ? "child" : "sibling",
  }), [tab.projectId, tab.groupId, tab.parentSessionId]);
  const contextKey = JSON.stringify(context);
  const [prepared, setPrepared] = useState<Prepared | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [revision, setRevision] = useState(0);
  const [choiceId, setChoiceId] = useState("");
  const [engine, setEngine] = useState<SessionEngine | null>(null);
  const [model, setModel] = useState("");
  const [effort, setEffort] = useState("");
  const [permission, setPermission] = useState<string | null>(null);
  const [worktree, setWorktree] = useState(false);
  const [worktreeName, setWorktreeName] = useState("");
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  // Agents are listed per location, so a tab opened without one starts in the last project used, or the
  // first project, instead of leaving the agent chip empty until a project is picked.
  useEffect(() => {
    if (tab.projectId || !projects.length) return;
    const recent = recentProjectId();
    const fallback = projects.find((p) => p.id === recent) ?? projects[0];
    useTermStore.getState().retargetNewSessionTab(tab.id, { projectId: fallback.id });
  }, [tab.id, tab.projectId, projects]);

  useEffect(() => {
    let alive = true;
    setPrepared(null); setLoadError(false);
    if (!validTarget) return;
    prepareAgentSession(context).then((result) => {
      if (alive) setPrepared({ key: contextKey, options: result.options, presets: result.presets, locationNames: result.locationNames });
    }).catch(() => { if (alive) setLoadError(true); });
    return () => { alive = false; };
  }, [context, contextKey, validTarget, revision]);
  useEffect(() => {
    if (!hidden) prompt.current?.focus();
  }, [hidden]);

  const ready = !!prepared && prepared.key === contextKey && validTarget;
  const choices = useMemo<AgentChoice[]>(() => prepared ? agentChoices(prepared.options, prepared.presets, recentAgentChoices()) : [],
    [prepared]);
  const choice = choices.find((c) => c.id === choiceId) ?? choices[0];
  const spec = choice ? prepared?.options.find((o) => o.id === choice.kind) : undefined;
  const kind = choice?.kind;
  const canChat = !!kind && supportsChatEngine(kind);
  const canTui = !!kind && hasTerminalInterface(kind);
  // An agent with only one view always opens in it; the switch shows the other one disabled.
  const view: SessionEngine = !canChat ? "tui" : !canTui ? "chat" : engine ?? defaultEngineFor(kind, agentDefaults);
  // The folder the session starts in: its parent's, else the project's. The backend applies group worktrees.
  const cwd = parent?.cwd || project?.rootPath || null;
  const git = useGitBranchInfo(cwd, !hidden);
  const location = ready ? prepared.locationNames : project ? [project.name] : [];

  const models = useLaunchModels(spec?.id, { cwd });
  const catalog = models.catalog;
  const modelEntry = catalog?.models.find((m) => m.id === model);
  const levels = (modelEntry?.effortLevels ?? catalog?.effortLevels ?? []);
  // Until the user picks one, the chip shows the mode the backend will apply: the preset's, else the
  // agent's default from Settings.
  const permissions = useAgentPermissions(kind ?? "claude",
    permission ?? choice?.preset?.permissionMode ?? (kind ? agentDefaults[kind]?.permissionMode : null) ?? null);
  const permissionCatalog = kind ? permissions?.catalog : undefined;

  const pickAgent = (next: string) => {
    if (next === choice?.id) return;
    // Model, effort, permission and view belong to one agent; another agent starts from its own defaults.
    setChoiceId(next); setEngine(null); setModel(""); setEffort(""); setPermission(null); setError("");
  };
  const pickModel = (next: string) => {
    setModel(next);
    // An effort the new model does not offer would be rejected at launch, so it falls back to the default.
    const supported = catalog?.models.find((m) => m.id === next)?.effortLevels ?? catalog?.effortLevels;
    if (effort && supported && !supported.includes(effort)) setEffort("");
  };
  // After a create, import or clone started here, the project it produced is selected in this tab: import
  // and create reveal the project (also when the folder was already a project), clone only adds it to the
  // list. Cancelling the dialog does neither, so the tab keeps its project until the next such action.
  const stopWatching = useRef<(() => void) | null>(null);
  useEffect(() => () => stopWatching.current?.(), []);
  const addProject = (run: () => void) => {
    stopWatching.current?.();
    const known = new Set(useTermStore.getState().projects.map((p) => p.id));
    const unsubscribe = useTermStore.subscribe((state, before) => {
      const revealed = state.revealProjectId && state.revealProjectId !== before.revealProjectId ? state.revealProjectId : null;
      const added = revealed ?? state.projects.find((p) => !known.has(p.id))?.id;
      if (!added || !state.projects.some((p) => p.id === added)) return;
      unsubscribe(); stopWatching.current = null;
      setError(""); setWorktree(false);
      useTermStore.getState().retargetNewSessionTab(tab.id, { projectId: added });
    });
    stopWatching.current = unsubscribe;
    run();
  };
  const retarget = (projectId: string) => {
    if (busy || projectId === tab.projectId) return;
    setError(""); setWorktree(false);
    useTermStore.getState().retargetNewSessionTab(tab.id, { projectId });
  };

  const start = async () => {
    if (!choice || !ready || busy) return;
    const task = text.trim();
    const request = {
      context,
      ...(choice.preset ? { presetId: choice.preset.id } : { kind: choice.kind }),
      ...(canChat && canTui ? { engine: view } : {}),
      ...(model.trim() ? { model: model.trim() } : {}),
      ...(effort.trim() ? { effort: effort.trim() } : {}),
      ...(permission ? { permissionMode: permission } : {}),
      ...(worktree ? { worktree: true, ...(worktreeName.trim() ? { worktreeName: worktreeName.trim() } : {}) } : {}),
      ...(task ? { prompt: task } : {}),
    };
    // Retrying the same submission reuses its request ID, so a reply lost after success cannot create twice.
    const key = JSON.stringify(request);
    if (attempt.current?.key !== key) attempt.current = { key, requestId: genId() };
    setBusy(true); setError("");
    try {
      const session = await createAgentSession({ requestId: attempt.current.requestId, ...request });
      const store = useTermStore.getState();
      useTermStore.setState((state) => ({
        sessions: state.sessions.some((s) => s.id === session.id) ? state.sessions : [...state.sessions, session],
        runtimes: { ...state.runtimes, [session.id]: state.runtimes[session.id] ?? { status: "idle" } },
        // A terminal session receives its first task as a launch argument when its process starts.
        ...(task && session.engine !== "chat" ? { pendingPrompts: { ...state.pendingPrompts, [session.id]: task } } : {}),
      }));
      rememberAgentChoice(choice.id);
      rememberProject(session.projectId);
      store.openSession(session.id);
      useTermStore.getState().closeTab(tab.id);
      void setCollapsed(session.parentSessionId ? "session" : session.groupId ? "group" : "project",
        session.parentSessionId ?? session.groupId ?? session.projectId, false)
        .catch(() => {}).then(() => useTermStore.getState().loadTree());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setBusy(false);
    }
  };

  const disabled = busy || !ready;
  const projectOptions: ChipOption<string>[] = projects.map((p) => ({ value: p.id, label: p.name, glyph: <Icons.folder size={14} /> }));
  const agentOptions: ChipOption<string>[] = choices.map((c) => ({
    value: c.id, label: c.label, glyph: agentGlyph(c, 14), tag: c.recent ? t("agentPicker.recent") : undefined,
    // Most agents have both views; the few with one say which, so the view switch below is no surprise.
    hint: !supportsChatEngine(c.kind) ? t("newSession.terminalOnly") : !hasTerminalInterface(c.kind) ? t("newSession.conversationOnly") : undefined,
  }));
  const defaultModelLabel = t("chat.followModelDefault", kind ? assistantLabel(kind) : "");
  const modelOptions: ChipOption<string>[] = kind ? [
    ...modelChipOptions(t, kind, catalog?.models ?? [], defaultModelLabel),
    // A model the catalogue no longer lists stays visible while it is selected.
    ...(model && !catalog?.models.some((m) => m.id === model) ? [{ value: model, label: model }] : []),
  ] : [];
  // Only the levels the launch catalogue reports are offered: they are what the backend accepts.
  const effortOptions = effortChipOptions(t, Array.from(new Set([...levels, ...(effort ? [effort] : [])])), true);
  const modelStatus = !models.current ? t("spawn.modelLoading")
    : models.current.error || !catalog?.models.length ? t("newSession.modelsUnavailable") : "";

  return (
    <div className="nsv" style={hidden ? { display: "none" } : undefined}>
      <div className="nsv-main">
        <header className="nsv-hero">
          <h1 id={`${id}-title`}>{t("newSession.heading")}</h1>
          {location.length > 0 && <p className="nsv-target">
            <Icons.folder size={13} aria-hidden="true" />
            <span className="nsv-target-path">{location.join(" / ")}</span>
            {git.isRepo && git.branch && <>
              <span aria-hidden="true">·</span>
              <Icons.branch size={12} aria-hidden="true" />
              <span>{git.branch}</span>
            </>}
          </p>}
        </header>

        {!projects.length ? (
          <div className="nsv-empty">
            <p>{t("agentPicker.noProject")}</p>
            <button type="button" className="vlx-btn" onClick={() => void useTermStore.getState().importProject()}>
              {t("tree.openProject")}
            </button>
          </div>
        ) : (
          <form className="nsv-form" aria-labelledby={`${id}-title`} onSubmit={(event) => { event.preventDefault(); void start(); }}>
            <div className={"nsv-composer" + (disabled ? " is-disabled" : "")}>
              <textarea ref={prompt} className="nsv-prompt" rows={3} value={text} disabled={busy}
                aria-label={t("newSession.taskLabel")} placeholder={t("newSession.placeholder")}
                onChange={(event) => { setText(event.target.value); setError(""); }}
                onKeyDown={(event) => {
                  if (event.nativeEvent.isComposing || event.keyCode === 229) return;
                  if (event.key === "Enter" && !event.shiftKey && !event.altKey && !event.metaKey && !event.ctrlKey) {
                    event.preventDefault(); void start();
                  }
                }} />
              <div className="nsv-composer-bar">
                <div className="nsv-chips">
                  <ControlChip placement="auto" glyph={<Icons.folder size={14} />}
                    label={project?.name ?? t("agentPicker.selectProject")} title={t("info.project")}
                    value={project?.id ?? ""} options={projectOptions} disabled={busy} menuWidth={260}
                    filterPlaceholder={t("chat.filterPlaceholder")} onPick={retarget}
                    actions={[
                      { id: "create", label: t("tree.createProject"), glyph: <Icons.projectPlus size={14} />,
                        onSelect: () => addProject(() => useTermStore.getState().setCreateProjectModalOpen(true)) },
                      { id: "import", label: t("tree.importProject"), glyph: <Icons.projectOpen size={14} />,
                        onSelect: () => addProject(() => void useTermStore.getState().importProject()) },
                      { id: "clone", label: t("tree.cloneProject"), glyph: <Icons.projectClone size={14} />,
                        onSelect: () => addProject(() => useTermStore.getState().setCloneModalOpen(true)) },
                    ]} />
                  <ControlChip placement="auto" glyph={choice ? agentGlyph(choice, 14) : <Icons.bot size={14} />}
                    label={choice?.label ?? (validTarget && !loadError ? t("common.loading") : t("orch.agentLabel"))}
                    title={t("orch.agentLabel")} value={choice?.id ?? ""} options={agentOptions}
                    disabled={disabled || !choices.length} menuWidth={260}
                    filterPlaceholder={t("chat.filterPlaceholder")} onPick={pickAgent} />
                  {/* The agent chip already carries the agent's mark, so the model chip uses a neutral one. */}
                  {spec && kind && <ControlChip placement="auto" glyph={<Icons.sparkle size={14} />}
                    label={catalog?.models.find((m) => m.id === model)?.label ?? (model || defaultModelLabel)} title={t("chat.modelTooltip")}
                    value={model} options={modelOptions} disabled={disabled} menuWidth={280}
                    filterPlaceholder={t("chat.filterPlaceholder")} onPick={pickModel}
                    footer={modelStatus ? <div className="sv-model-catalog-status">
                      <div role="status">{modelStatus}</div>
                      {models.current && <button type="button" className="sv-model-catalog-refresh" onClick={models.retry}>{t("common.retry")}</button>}
                    </div> : undefined} />}
                  {spec?.effortFlag && <ControlChip placement="auto" glyph={<Icons.cpu size={14} />}
                    label={effort ? effortChipLabel(t, effort) : t("chat.effortDefault")} title={t("chat.effortTooltip")}
                    value={effort} options={effortOptions} disabled={disabled} onPick={setEffort} />}
                  {permissionCatalog && <ControlChip placement="auto" glyph={<Icons.lock size={14} />}
                    label={t(permissionLabelKey(permissionCatalog.selected))} title={t("chat.modeTooltip")}
                    value={permissionCatalog.selected} disabled={disabled} menuWidth={240}
                    options={permissionCatalog.modes.map((mode) => ({ value: mode, label: t(permissionLabelKey(mode)) }))}
                    onPick={(mode) => setPermission(mode)} />}
                </div>
                <button type="submit" className="nsv-start" disabled={disabled || !choice}
                  aria-label={t(busy ? "newSession.starting" : "newSession.start")} title={t(busy ? "newSession.starting" : "newSession.start")}>
                  {busy ? <span className="nsv-spinner" aria-hidden="true" /> : <Icons.arrowUp size={15} aria-hidden="true" />}
                </button>
              </div>
            </div>
            <div className="nsv-toolbar">
              {kind && <span className="nsv-view" role="group" aria-label={t("newSession.viewLabel")}
                title={!canChat ? t("newSession.noConversationView") : !canTui ? t("newSession.noTerminalView") : undefined}>
                <Seg<SessionEngine> value={view} onChange={(next) => { if (!busy) setEngine(next); }}
                  disabledOptions={[...(canChat ? [] : ["chat" as const]), ...(canTui ? [] : ["tui" as const])]}
                  options={[["chat", t("tree.engineChat")], ["tui", t("tree.engineTui")]]} />
              </span>}
              <label className={"nsv-switch" + (git.isRepo ? "" : " is-disabled")}
                title={git.isRepo ? t("newSession.worktreeHint") : t("newSession.worktreeNotRepo")}>
                <input type="checkbox" role="switch" checked={worktree && git.isRepo} disabled={busy || !git.isRepo}
                  onChange={(event) => setWorktree(event.target.checked)} />
                <span>{t("spawn.worktreeLabel")}</span>
              </label>
              {/* A disabled switch alone looks broken, so the reason is on screen, not only in the tooltip. */}
              {cwd && !git.isRepo && <span className="nsv-switch-note">{t("newSession.worktreeNotRepo")}</span>}
              {worktree && git.isRepo && <input className="nsv-worktree-name" type="text" value={worktreeName} disabled={busy}
                spellCheck={false} autoComplete="off" maxLength={80}
                aria-label={t("newSession.worktreeName")} placeholder={t("newSession.worktreeName")}
                title={t("newSession.worktreeNameHint")}
                onChange={(event) => setWorktreeName(event.target.value)}
                onKeyDown={(event) => {
                  if (event.nativeEvent.isComposing || event.keyCode === 229) return;
                  if (event.key === "Enter") { event.preventDefault(); void start(); }
                }} />}
              <span className="nsv-hint">{t("newSession.enterHint")}</span>
            </div>
            {loadError && <div className="nsv-notice" role="alert">
              <span>{t("agentPicker.loadFailed")}</span>
              <button type="button" className="vlx-btn" onClick={() => setRevision((r) => r + 1)}>{t("common.retry")}</button>
            </div>}
            {project && !validTarget && <p className="nsv-notice" role="status">{t("agentPicker.invalidTarget")}</p>}
            {error && <p className="nsv-error" role="alert">{error}</p>}
          </form>
        )}
      </div>
    </div>
  );
}
