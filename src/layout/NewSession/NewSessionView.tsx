//! New Session tab: pick where and how an agent session runs, type its first task, and start it. Every
//! choice is resolved again by the backend at creation; this view only offers what the backend reports.
//!
//! A session type switch (task, project, chat) and a setup card (project, agent, model, effort, permission) sit
//! above a chat-style composer. Only project sessions pick a project; task and chat sessions are created in the
//! fixed Tasks and Chat collections, so the tab's location alone says which type it is.
//!
//! Images pasted into the task go with it: a conversation receives them with its first message, and a
//! terminal session gets uploaded copies whose paths are added to its first task, as a terminal paste does.
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { AgentPermissionSelect } from "../../components/AgentPermissionSelect";
import Icons from "../../components/Icons";
import { LaunchField, ModelEffortFields } from "../../components/LaunchFields";
import Select from "../../components/Select";
import { useTaskImages } from "../../components/useTaskImages";
import { genId } from "../../genId";
import { useGitBranchInfo } from "../../hooks/useGitBranch";
import { useT } from "../../i18n";
import { createAgentSession, prepareAgentSession, type AgentSessionContext, type LaunchOption } from "../../ipc/launch";
import { uploadImage } from "../../ipc/transport";
import { setCollapsed } from "../../ipc/tree";
import { defaultEngineFor } from "../../store/settings";
import { extOf } from "../../terminal/imageInput";
import { useTermStore, type NewSessionTab } from "../../store/termStore";
import { hasTerminalInterface, isVirtualProject, supportsChatEngine, type AgentPreset, type SessionEngine, type SessionKind } from "../../types";
import { dataUrl, type Attachment } from "../CenterPane/session/attachments";
import { PresetIcon } from "../agentPresetIcon";
import { agentChoices, recentAgentChoices, recentProjectId, rememberAgentChoice, rememberProject, type AgentChoice } from "../NewAgentSession/choices";
import { kindIconEl } from "../sessionViewers/sessionMeta";
import { Seg } from "../TitleBar/settingsParts";
import {
  isSystemCollection, SESSION_TYPE_COLLECTION, sessionTypeOf, SystemCollectionIcon, systemLabel, type SessionType,
  type SystemSection,
} from "../LeftSidebar/systemCollections";
import "./new-session.css";

interface Prepared { key: string; options: LaunchOption[]; presets: AgentPreset[]; locationNames: string[] }

const SESSION_TYPES: { type: SessionType; section: SystemSection; label: "newSession.typeTask" | "newSession.typeProject" | "newSession.typeChat";
  hint: "newSession.typeTaskHint" | "newSession.typeProjectHint" | "newSession.typeChatHint" }[] = [
  { type: "project", section: "projects", label: "newSession.typeProject", hint: "newSession.typeProjectHint" },
  { type: "task", section: "tasks", label: "newSession.typeTask", hint: "newSession.typeTaskHint" },
  { type: "chat", section: "chat", label: "newSession.typeChat", hint: "newSession.typeChatHint" },
];

/** Upload pasted images for a terminal agent and add their paths to its first task, as a terminal paste does. */
async function withImagePaths(task: string, images: Attachment[], kind: SessionKind): Promise<string> {
  const lines = task ? [task] : [];
  for (const image of images) {
    const bytes = Uint8Array.from(atob(image.data), (c) => c.charCodeAt(0));
    const path = await uploadImage(bytes, extOf(new File([], image.name, { type: image.mimeType })));
    // Codex turns a lone image path into a collapsed attachment, so the path keeps a visible prefix.
    lines.push(kind === "codex" ? `image_path: ${path}` : path);
  }
  return lines.join("\n");
}

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
  const sessionType = sessionTypeOf(tab.projectId);
  // Fixed collections are not projects to pick; Tasks and Chat are reached through the type switch.
  // A project session can also be created directly in a user's own collection, listed after the projects.
  const pickable = useMemo(() => {
    const own = projects.filter((p) => !isSystemCollection(p.id));
    return [...own.filter((p) => !isVirtualProject(p)), ...own.filter(isVirtualProject)];
  }, [projects]);
  // A backend without the fixed collections can only create project sessions.
  const typeAvailable = (type: SessionType) =>
    type === "project" || projects.some((p) => p.id === SESSION_TYPE_COLLECTION[type]);
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
  // The tab owns its pasted images; changing the project or agent keeps them.
  const imageScope = useMemo(() => ({ tab: tab.id }), [tab.id]);
  const images = useTaskImages(imageScope);

  // Agents are listed per location, so a tab opened without one starts in the last project used, or the
  // first project, instead of leaving the agent chip empty until a project is picked.
  // The last location used may be Tasks or Chat, which reopens that session type.
  useEffect(() => {
    if (tab.projectId || !projects.length) return;
    const recent = recentProjectId();
    // With no project at all the tab stays empty: it offers project entry points and the other types.
    const fallback = projects.find((p) => p.id === recent && sessionTypeOf(p.id) !== "project")
      ?? pickable.find((p) => p.id === recent) ?? pickable[0];
    if (fallback) useTermStore.getState().retargetNewSessionTab(tab.id, { projectId: fallback.id });
  }, [tab.id, tab.projectId, projects, pickable]);

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
  // A chat session opens in the conversation view, so agents without one are not offered for it.
  const choices = useMemo<AgentChoice[]>(() => prepared
    ? agentChoices(prepared.options, prepared.presets, recentAgentChoices())
      .filter((c) => sessionType !== "chat" || supportsChatEngine(c.kind))
    : [], [prepared, sessionType]);
  const choice = choices.find((c) => c.id === choiceId) ?? choices[0];
  const spec = choice ? prepared?.options.find((o) => o.id === choice.kind) : undefined;
  const kind = choice?.kind;
  const canChat = !!kind && supportsChatEngine(kind);
  const canTui = !!kind && hasTerminalInterface(kind);
  // An agent with only one view always opens in it; the switch shows the other one disabled.
  const view: SessionEngine = sessionType === "chat" ? "chat"
    : !canChat ? "tui" : !canTui ? "chat" : engine ?? defaultEngineFor(kind, agentDefaults);
  // Task and chat sessions have no project folder, so there is nothing to branch a worktree from.
  // A collection has no folder either.
  const worktreeAllowed = sessionType === "project" && !isVirtualProject(project);
  // The folder the session starts in: its parent's, else the project's. The backend applies group worktrees.
  const cwd = parent?.cwd || project?.rootPath || null;
  const git = useGitBranchInfo(cwd, !hidden);
  const location = (ready ? prepared.locationNames : []).join(" / ");

  const pickAgent = (next: string) => {
    if (next === choice?.id) return;
    // Model, effort, permission and view belong to one agent; another agent starts from its own defaults.
    setChoiceId(next); setEngine(null); setModel(""); setEffort(""); setPermission(null); setError("");
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
  // Switching to a project session returns to the last project used, else the first one.
  const pickType = (type: SessionType) => {
    if (busy || type === sessionType || !typeAvailable(type)) return;
    if (type !== "project") { retarget(SESSION_TYPE_COLLECTION[type]); return; }
    const recent = recentProjectId();
    const next = pickable.find((p) => p.id === recent) ?? pickable[0];
    if (next) retarget(next.id);
    else { setError(""); setWorktree(false); useTermStore.getState().retargetNewSessionTab(tab.id, { projectId: "" }); }
  };

  const start = async () => {
    if (!choice || !ready || busy || images.busy) return;
    const task = text.trim();
    const pictures = images.attachments.map(({ mimeType, data }) => ({ mimeType, data }));
    const request = {
      context,
      ...(choice.preset ? { presetId: choice.preset.id } : { kind: choice.kind }),
      ...(canChat && canTui && sessionType !== "chat" ? { engine: view } : {}),
      ...(model.trim() ? { model: model.trim() } : {}),
      ...(effort.trim() ? { effort: effort.trim() } : {}),
      ...(permission ? { permissionMode: permission } : {}),
      ...(worktree && worktreeAllowed ? { worktree: true, ...(worktreeName.trim() ? { worktreeName: worktreeName.trim() } : {}) } : {}),
      ...(task ? { prompt: task } : {}),
      // A terminal session receives its images as file paths in the first task instead.
      ...(view === "chat" && pictures.length ? { images: pictures } : {}),
    };
    // Retrying the same submission reuses its request ID, so a reply lost after success cannot create twice.
    const key = JSON.stringify(request);
    if (attempt.current?.key !== key) attempt.current = { key, requestId: genId() };
    setBusy(true); setError("");
    let launchPrompt = task;
    if (view !== "chat" && images.attachments.length) {
      try { launchPrompt = await withImagePaths(task, images.attachments, choice.kind); }
      catch {
        setError(t("term.imgUploadFailed", images.attachments.length, ""));
        setBusy(false);
        return;
      }
    }
    try {
      const session = await createAgentSession({ requestId: attempt.current.requestId, ...request });
      const store = useTermStore.getState();
      useTermStore.setState((state) => ({
        sessions: state.sessions.some((s) => s.id === session.id) ? state.sessions : [...state.sessions, session],
        runtimes: { ...state.runtimes, [session.id]: state.runtimes[session.id] ?? { status: "idle" } },
        // A terminal session receives its first task as a launch argument when its process starts.
        ...(launchPrompt && session.engine !== "chat" ? { pendingPrompts: { ...state.pendingPrompts, [session.id]: launchPrompt } } : {}),
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

  const targetLabel = !validTarget ? ""
    : sessionType === "task" ? t("agentPicker.targetProject", systemLabel(t, "tasks"))
    : sessionType === "chat" ? t("agentPicker.targetProject", systemLabel(t, "chat"))
    : parent ? t("agentPicker.targetChild", parent.name, location) : t("agentPicker.targetProject", location);
  const disabled = busy || !ready;
  // Until the user picks one, the field shows the mode the backend will apply: the preset's, else the
  // agent's default from Settings.
  const permissionValue = permission ?? choice?.preset?.permissionMode ?? (kind ? agentDefaults[kind]?.permissionMode : null) ?? null;

  return (
    <div className="nsv" style={hidden ? { display: "none" } : undefined}>
      <div className="nsv-main">
        <header className="nsv-hero">
          <span className="nsv-hero-icon" aria-hidden="true">{agentGlyph(choice, 34)}</span>
          <h1 id={`${id}-title`}>{t("newSession.heading")}</h1>
          {targetLabel && <p className="nsv-target">{targetLabel}</p>}
        </header>

        <div className="nsv-types" role="radiogroup" aria-label={t("newSession.typeLabel")}>
          {SESSION_TYPES.map((option) => {
            const active = option.type === sessionType;
            return (
              <button key={option.type} type="button" role="radio" aria-checked={active}
                className={"nsv-type" + (active ? " is-active" : "")}
                disabled={busy || !typeAvailable(option.type)} onClick={() => pickType(option.type)}>
                <span className="nsv-type-head">
                  <SystemCollectionIcon section={option.section} size={14} />
                  <span className="nsv-type-name">{t(option.label)}</span>
                </span>
                <span className="nsv-type-hint">{t(option.hint)}</span>
              </button>
            );
          })}
        </div>

        {sessionType === "project" && !pickable.length ? (
          <div className="nsv-card nsv-empty">
            <p>{t("agentPicker.noProject")}</p>
            <button type="button" className="vlx-btn" onClick={() => void useTermStore.getState().importProject()}>
              {t("tree.openProject")}
            </button>
          </div>
        ) : (
          <section className="nsv-card" aria-labelledby={`${id}-title`}>
            <div className="nsv-grid">
              {sessionType === "project" && <LaunchField label={t("newSession.location")}>
                <Select value={project?.id ?? ""} width="100%" menuPortal disabled={busy}
                  ariaLabel={t("newSession.location")} placeholder={t("agentPicker.selectProject")}
                  searchable searchPlaceholder={t("newSession.locationSearch")} noMatchLabel={t("tree.noMatch")}
                  options={pickable.map((p) => isVirtualProject(p)
                    ? { value: p.id, label: p.name, icon: <Icons.collection size={13} />, hint: t("newSession.collectionOption") }
                    : { value: p.id, label: p.name, icon: <Icons.folder size={13} /> })}
                  onChange={retarget}
                  actions={[
                    { id: "create", label: t("tree.createProject"), icon: <Icons.projectPlus size={13} />,
                      onSelect: () => addProject(() => useTermStore.getState().setCreateProjectModalOpen(true)) },
                    { id: "import", label: t("tree.importProject"), icon: <Icons.projectOpen size={13} />,
                      onSelect: () => addProject(() => void useTermStore.getState().importProject()) },
                    { id: "clone", label: t("tree.cloneProject"), icon: <Icons.projectClone size={13} />,
                      onSelect: () => addProject(() => useTermStore.getState().setCloneModalOpen(true)) },
                  ]} />
              </LaunchField>}
              <LaunchField label={t("orch.agentLabel")}>
                <Select value={choice?.id ?? ""} width="100%" menuPortal disabled={disabled || !choices.length}
                  ariaLabel={t("orch.agentLabel")}
                  placeholder={validTarget && !loadError ? t("common.loading") : "—"}
                  options={choices.map((c) => ({
                    value: c.id, label: c.label, icon: agentGlyph(c, 14),
                    // Most agents have both views; the few with one say which, so the view switch is no surprise.
                    hint: [c.recent ? t("agentPicker.recent") : "",
                      !supportsChatEngine(c.kind) ? t("newSession.terminalOnly") : !hasTerminalInterface(c.kind) ? t("newSession.conversationOnly") : ""]
                      .filter(Boolean).join(" · ") || undefined,
                  }))}
                  onChange={pickAgent} />
              </LaunchField>
              {spec && <ModelEffortFields key={choice?.id} spec={spec} model={model} effort={effort} disabled={disabled}
                context={{ cwd }} onChange={(nextModel, nextEffort) => { setModel(nextModel); setEffort(nextEffort); }} />}
              {kind && <LaunchField label={t("info.permission")}>
                <AgentPermissionSelect key={choice?.id} kind={kind} value={permissionValue}
                  onChange={(mode) => setPermission(mode)} />
              </LaunchField>}
            </div>
            {loadError && <div className="nsv-notice" role="alert">
              <span>{t("agentPicker.loadFailed")}</span>
              <button type="button" className="vlx-btn" onClick={() => setRevision((r) => r + 1)}>{t("common.retry")}</button>
            </div>}
            {project && !validTarget && <p className="nsv-notice" role="status">{t("agentPicker.invalidTarget")}</p>}
          </section>
        )}
      </div>

      <form className="nsv-composer-wrap" onSubmit={(event) => { event.preventDefault(); void start(); }}>
        <div className={"nsv-composer" + (disabled ? " is-disabled" : "")}>
          <textarea ref={prompt} className="nsv-prompt" rows={3} value={text} disabled={busy}
            aria-label={t("newSession.taskLabel")} placeholder={t("newSession.placeholder")}
            onPaste={busy ? undefined : images.onPaste}
            onChange={(event) => { setText(event.target.value); setError(""); }}
            onKeyDown={(event) => {
              if (event.nativeEvent.isComposing || event.keyCode === 229) return;
              if (event.key === "Enter" && !event.shiftKey && !event.altKey && !event.metaKey && !event.ctrlKey) {
                event.preventDefault(); void start();
              }
            }} />
          {images.attachments.length > 0 && <div className="launch-images nsv-images">
            {images.attachments.map((image) => <div className="launch-image" key={image.id} title={image.name}>
              <img src={dataUrl(image)} alt={image.name} />
              <button type="button" className="launch-image-remove" title={t("chat.attach.remove")}
                aria-label={t("chat.attach.remove")} disabled={busy || images.busy} onClick={() => images.remove(image.id)}>
                <Icons.close size={12} />
              </button>
            </div>)}
          </div>}
          <div className="nsv-composer-bar">
            <span className="nsv-hint">{t("newSession.enterHint")}</span>
            <button type="submit" className="vlx-btn vlx-btn-primary nsv-start" disabled={disabled || !choice || images.busy}>
              {t(busy ? "newSession.starting" : "newSession.start")}
              {!busy && <Icons.arrowUp size={13} aria-hidden="true" />}
            </button>
          </div>
        </div>
        <div className="nsv-toolbar">
          {kind && sessionType !== "chat" && <span className="nsv-view" role="group" aria-label={t("newSession.viewLabel")}
            title={!canChat ? t("newSession.noConversationView") : !canTui ? t("newSession.noTerminalView") : undefined}>
            <Seg<SessionEngine> value={view} onChange={(next) => { if (!busy) setEngine(next); }}
              disabledOptions={[...(canChat ? [] : ["chat" as const]), ...(canTui ? [] : ["tui" as const])]}
              options={[["chat", t("tree.engineChat")], ["tui", t("tree.engineTui")]]} />
          </span>}
          {worktreeAllowed && <label className={"nsv-check" + (git.isRepo ? "" : " is-disabled")}
            title={git.isRepo ? t("newSession.worktreeHint") : t("newSession.worktreeNotRepo")}>
            <input type="checkbox" checked={worktree && git.isRepo} disabled={busy || !git.isRepo}
              onChange={(event) => setWorktree(event.target.checked)} />
            <Icons.branch size={13} aria-hidden="true" />
            <span>{t("spawn.worktreeLabel")}</span>
          </label>}
          {/* A disabled checkbox alone looks broken, so the reason is on screen, not only in the tooltip. */}
          {worktreeAllowed && cwd && !git.isRepo && <span className="nsv-check-note">{t("newSession.worktreeNotRepo")}</span>}
          {worktreeAllowed && worktree && git.isRepo && <input className="vlx-input nsv-worktree-name" type="text" value={worktreeName} disabled={busy}
            spellCheck={false} autoComplete="off" maxLength={80}
            aria-label={t("newSession.worktreeName")} placeholder={t("newSession.worktreeName")}
            title={t("newSession.worktreeNameHint")}
            onChange={(event) => setWorktreeName(event.target.value)}
            onKeyDown={(event) => {
              if (event.nativeEvent.isComposing || event.keyCode === 229) return;
              if (event.key === "Enter") { event.preventDefault(); void start(); }
            }} />}
        </div>
        {images.busy && <p className="nsv-hint" role="status">{t("common.loading")}</p>}
        {images.note && <p className="nsv-error" role="alert">{images.note}</p>}
        {error && <p className="nsv-error" role="alert">{error}</p>}
      </form>
    </div>
  );
}
