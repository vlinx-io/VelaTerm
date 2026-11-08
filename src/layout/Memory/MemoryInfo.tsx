//! What the right panel's Info tab shows for knowledge-base pages: an entry's provenance, or an archived session's details.
import { useT } from "../../i18n";
import { memoryCollections, memoryGet } from "../../ipc/memory";
import { kindLabel } from "../sessionMenuShared";
import type { SessionKind } from "../../types";
import { collectionPlace } from "./MemoryCollections";
import { EntryFacts } from "./MemoryDocument";
import { LoadState, detailTime, useMemoryLoad } from "./shared";
import "./kbx.css";

/** `route` is the value of the memory query parameter, already known to be an entry, a revision or an archived session. */
export function MemoryInfo({ route }: { route: string }) {
  const [page, id, version] = route.split("/");
  return page === "collection" ? <ArchivedInfo id={id} /> : <EntryInfo key={route} id={id} version={page === "history" ? Number(version) : undefined} />;
}

function EntryInfo({ id, version }: { id: string; version?: number }) {
  const { data, error, reload } = useMemoryLoad(() => memoryGet(id, version), [id, version]);
  const entry = data ? (version == null ? data.entry : data.revision) : null;
  if (!data || !entry) return <LoadState error={error} reload={reload} />;
  return <div className="kbx-info"><EntryFacts id={id} version={version} data={data} entry={entry} /></div>;
}

function ArchivedInfo({ id }: { id: string }) {
  const t = useT();
  const { data, error, reload } = useMemoryLoad(memoryCollections, []);
  if (!data) return <LoadState error={error} reload={reload} />;
  const project = data.projects.find((item) => item.sessions.some((session) => session.id === id));
  const session = project?.sessions.find((item) => item.id === id);
  if (!project || !session) return null;
  const rows = new Map(project.sessions.map((item) => [item.id, item]));
  return <div className="kbx-info"><section className="kbx-rsec">
    <div className="kbx-kv">
      <span className="k">{t("archive.colProject")}</span><span className="v">{collectionPlace(project.name || t("memory.unknownProject"), rows, id)}</span>
      <span className="k">{t("archive.colAgent")}</span><span className="v">{kindLabel(session.kind as SessionKind)}</span>
      {session.archivedAt && <><span className="k">{t("archive.colArchived")}</span><span className="v">{detailTime(session.archivedAt * 1000)}</span></>}
      <span className="k">{t("archive.colKnowledge")}</span><span className="v">{session.count ? t("kb.entriesCount", String(session.count)) : t("archive.withoutKnowledge")}</span>
    </div>
  </section></div>;
}
