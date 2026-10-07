//! Vlinx-style right panel with Files, Info, and Git tabs following the current session directory.
//! Current implementation:
//! - Files tree/preview, session basics, and Git branch/ahead/behind values use real data.
//! - Info resource/turn details and per-file Git changes retain design placeholders until their
//!   backend IPC is available; their DOM structure already matches the design.

import { useEffect, useRef } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { projectRoot, type Group, type Project, type Session } from "../../types";
import { FilesTab } from "./FilesTab";
import { GitTab } from "./git/GitTab";
import { InfoTab } from "./InfoTab";
import { KV, Section } from "./parts";
import { KnowledgeNavigation } from "../Notebook/KnowledgeNavigation";
import { MemoryIcon } from "../Memory/shared";
import { memoryNavigate, useMemoryLocation } from "../Memory/navigation";

const INSPECTOR_TABS = [
  { id: "files" as const, label: "Files", Icon: Icons.file },
  { id: "info" as const, label: "Info", Icon: Icons.info },
  { id: "git" as const, label: "Git", Icon: Icons.git },
  { id: "knowledge" as const, label: "", Icon: MemoryIcon },
];


/* ===================== Info: real basics and placeholder resources ===================== */



/** Show project/group basics when the selection is not a session; Files and Git use cwd directly. */
function ScopeInfo({ project, group }: { project: Project; group?: Group }) {
  return (
    <Section id="scope" title={group ? "Group" : "Project"} className="scope-section">
      {group && <KV k="group" v={group.name} />}
      <KV k="project" v={project.name} />
      {/* Collections have no root directory, so the path row is omitted rather than shown empty. */}
      {projectRoot(project) && <KV k="path" v={project.rootPath} accent />}
    </Section>
  );
}


/* ===================== Container ===================== */

export function RightPanel() {
  const t = useT();
  const width = useTermStore((s) => s.rightWidth);
  const inspectorTab = useTermStore((s) => s.inspectorTab);
  const setInspectorTab = useTermStore((s) => s.setInspectorTab);
  const search=useMemoryLocation();
  const initialInspectorTab = useRef(inspectorTab);
  useEffect(()=>{const tab=new URLSearchParams(search).get("inspector") ?? initialInspectorTab.current;if((tab==="files"||tab==="info"||tab==="git"||tab==="knowledge")&&useTermStore.getState().inspectorTab!==tab)setInspectorTab(tab);},[search,setInspectorTab]);
  const tabUrl=(tab:string)=>{const url=new URL(window.location.href);url.searchParams.set("inspector",tab);if(tab==="knowledge"&&!url.searchParams.has("memory"))url.searchParams.set("memory","notebooks");return url.href;};
  const sessions = useTermStore((s) => s.sessions);
  const ephemeralSessions = useTermStore((s) => s.ephemeralSessions);
  const groups = useTermStore((s) => s.groups);
  const projects = useTermStore((s) => s.projects);
  const inspectTarget = useTermStore((s) => s.inspectTarget);
  const activeSessionId = useTermStore((s) => s.activeSessionId);

  // inspectTarget only overrides inspection for projects/groups. Any active-session change clears it
  // through termStore's activeSessionId subscription, even while this panel is unmounted, so no local
  // cleanup effect is needed.

  // Remember the last valid inspection target. Document/browser tabs clear activeSessionId, so this
  // fallback prevents Files, Info, and Git from going blank.
  const lastTargetRef = useRef<{ id: string; kind: string } | null>(null);

  // Resolve the session, owning project, and group by target type. Groups and projects use the project root.
  const resolve = (t: { id: string; kind: string } | null) => {
    let session: Session | undefined;
    let project: Project | undefined;
    let group: Group | undefined;
    if (t?.kind === "session") {
      session = sessions.find((s) => s.id === t.id) ?? ephemeralSessions[t.id];
      project = session ? projects.find((p) => p.id === session!.projectId) : undefined;
    } else if (t?.kind === "group") {
      group = groups.find((g) => g.id === t.id);
      project = group ? projects.find((p) => p.id === group!.projectId) : undefined;
    } else if (t?.kind === "project") {
      project = projects.find((p) => p.id === t.id);
    }
    return { session, project, group };
  };

  // Prefer a valid project/group override; otherwise fall back to the active session.
  const activeTarget = activeSessionId ? { id: activeSessionId, kind: "session" } : null;
  let resolvedFrom: { id: string; kind: string } | null = inspectTarget;
  let { session, project, group } = resolve(inspectTarget);
  if (!session && !project) {
    resolvedFrom = activeTarget;
    ({ session, project, group } = resolve(activeTarget));
  }
  // Document/browser tabs have no active session. Re-resolve the last valid target so the panel keeps
  // showing the recent workspace; deleted nodes naturally resolve to nothing. Store only valid results.
  if (session || project) {
    lastTargetRef.current = resolvedFrom;
  } else {
    ({ session, project, group } = resolve(lastTargetRef.current));
  }
  const cwd = session?.cwd || project?.rootPath || null;

  return (
    <aside className="col col-right" style={{ width, borderLeft: "none" }}>
      <nav className="insp-tabs">
        {INSPECTOR_TABS.map(({ id, label, Icon }) => (
          <a
            href={tabUrl(id)}
            title={id==="knowledge"?t("memory.title"):label}
            aria-label={id==="knowledge"?t("memory.title"):label}
            aria-current={inspectorTab===id?"page":undefined}
            key={id}
            className={"insp-tab" + (inspectorTab === id ? " on" : "")}
            // Apply the tab directly, then record it in the URL. The URL is not the source of truth here:
            // a peer's mirrored layout (or a stale `inspector=` value) can leave it naming a tab this panel
            // is not showing, and then navigating to that same URL would render no change at all — the
            // click appears dead. `memoryNavigate` is skipped when the URL already matches, so a click
            // that only needs the panel to catch up does not add a history entry.
            onClick={event=>{if(event.metaKey||event.ctrlKey||event.shiftKey||event.altKey)return;event.preventDefault();setInspectorTab(id);const url=tabUrl(id);if(url!==window.location.href)memoryNavigate(url);}}
          >
            <Icon size={13} />
          </a>
        ))}
      </nav>
      <div className="insp-body" style={{ display: "flex", flexDirection: "column" }}>
        {inspectorTab === "knowledge" && <KnowledgeNavigation/>}
        {inspectorTab === "files" && <FilesTab rootPath={cwd} rootName={project?.name ?? null} />}
        {inspectorTab === "info" &&
          (session ? (
            <InfoTab key={session.id} session={session} cwd={cwd} />
          ) : project ? (
            <ScopeInfo project={project} group={group} />
          ) : (
            <div className="insp-section" style={{ color: "var(--text-faint)" }}>
              {t("panel.noSession")}
            </div>
          ))}
        {inspectorTab === "git" && <GitTab path={cwd} />}
      </div>
    </aside>
  );
}
