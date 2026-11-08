//! One inline row shared by every knowledge tree. When a row carries a `target` it also becomes a
//! management surface: right-click opens the shared menu, dragging reparents the item, and an inline
//! input edits its name. Rows without a target stay plain navigation links.

import { createContext, useContext, useEffect, useRef, type ReactNode } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { stripControlChars, useCtrlCharGuard } from "../../hooks/textInputGuards";
import { MemoryLink } from "../Memory/navigation";

export { knowledgeSelection } from "./knowledgeSelection";

/** A data row the tree can rename, move or delete. Synthetic groups (`__manual__`, `__legacy__`,
 *  `__unknown__`) are marked so the actions menu can omit rename. */
export type KnowledgeRowTarget =
  | { key: string; kind: "memoryProject"; id: string; name: string; count: number; special: boolean }
  | { key: string; kind: "memorySession"; id: string; name: string; count: number; special: boolean }
  | { key: string; kind: "memoryEntry"; id: string; title: string; version: number; sessionId: string }
  | { key: string; kind: "vault"; id: string; name: string; root: string }
  | { key: string; kind: "notebookNode"; vaultId: string; path: string; name: string; nodeKind: "folder" | "note" | "asset" | "large" };

/** What a row hands to a compatible drop target. Entries remember their group so a drop back into the
 *  same session is rejected before it reaches the backend. */
export type KnowledgeDragPayload =
  | { kind: "memoryEntry"; id: string; version: number; sessionId: string }
  | { kind: "memorySession"; id: string }
  | { kind: "notebookNode"; vaultId: string; path: string; nodeKind: "folder" | "note" | "asset" | "large" };

export interface KnowledgeTreeActions {
  /** Bumped after every successful mutation so data loaders can refresh. */
  revision: number;
  renameKey: string | null;
  renameValue: string;
  setRenameValue(value: string): void;
  commitRename(): void;
  cancelRename(): void;
  openMenu(target: KnowledgeRowTarget, x: number, y: number): void;
  drag: KnowledgeDragPayload | null;
  dragKey: string | null;
  dropKey: string | null;
  dragPayload(target: KnowledgeRowTarget): KnowledgeDragPayload | null;
  beginDrag(payload: KnowledgeDragPayload): void;
  endDrag(): void;
  hoverDrop(target: KnowledgeRowTarget | null): void;
  canDrop(target: KnowledgeRowTarget): boolean;
  drop(target: KnowledgeRowTarget): void;
}

export const KnowledgeTreeContext = createContext<KnowledgeTreeActions | null>(null);
export const useKnowledgeTree = () => useContext(KnowledgeTreeContext);

/** Stable key constructors so rows, menus and drag state agree on one identity per item. */
export const treeKey = {
  memoryProject: (id: string) => `mp:${id}`,
  memorySession: (id: string) => `ms:${id}`,
  memoryEntry: (id: string) => `me:${id}`,
  vault: (id: string) => `vb:${id}`,
  notebookNode: (vaultId: string, path: string) => `nb:${vaultId}:${path}`,
};

/** Every folder, including each knowledge-base root, uses the same inline tree row. Files with an
 *  onActivate callback open through their owning surface instead of navigating the memory route. */
export function KnowledgeTreeRow({ name, route, values, depth, selected, icon, expanded, onToggle, onOpen, onActivate, children, title, count, knowledge, attention, target, prose }: {
  name: string; route?: string; values?: Record<string, string | number | null>; depth: number;
  selected?: boolean; icon: ReactNode; expanded?: boolean; onToggle?: () => void;
  onOpen?: () => void; onActivate?: () => void; children?: ReactNode; title?: string; count?: number;
  /** Knowledge entries organized from this archived project or session. */
  knowledge?: number;
  /** Failed runs, shown as a warning badge. */
  attention?: number;
  target?: KnowledgeRowTarget;
  /** The name is natural language (a session, an entry, a note) rather than a project or folder. */
  prose?: boolean;
}) {
  const t = useT();
  const actions = useKnowledgeTree();
  const ctrlCharGuard = useCtrlCharGuard();
  const row = useRef<HTMLDivElement>(null);
  useEffect(() => { if (selected) row.current?.scrollIntoView?.({ block: "nearest" }); }, [selected]);
  const renaming = !!target && !!actions && actions.renameKey === target.key;
  const payload = target && actions ? actions.dragPayload(target) : null;
  const dropTarget = !!target && !!actions && actions.dropKey === target.key;
  const label = <>{icon}<span className={"nb-tree-label" + (prose ? " p" : "")}>{name}</span>
    {!!attention && <em className="kbx-fail">{attention}</em>}
    {!!knowledge && <em className="kbx-count"><Icons.layers size={12} aria-hidden="true" />{knowledge}</em>}
    {count != null && <small>{count.toLocaleString()}</small>}</>;
  const renameInput = <input
    ref={ctrlCharGuard}
    className="rename-input"
    autoFocus
    aria-label={t("common.rename")}
    value={actions?.renameValue ?? ""}
    onClick={(event) => event.stopPropagation()}
    onChange={(event) => actions?.setRenameValue(stripControlChars(event.target.value))}
    onKeyDown={(event) => {
      if (event.key === "Enter") actions?.commitRename();
      if (event.key === "Escape") actions?.cancelRename();
    }}
    onBlur={() => actions?.commitRename()}
  />;
  return <li className="nb-tree-item" data-depth={depth}>
    <div
      ref={row}
      className={"nb-tree-row" + (selected ? " active" : "") + (dropTarget ? " drop-over" : "") + (target && actions?.dragKey === target.key ? " dragging" : "")}
      style={{ paddingLeft: 3 + depth * 13 }}
      onContextMenu={target && actions ? (event) => {
        event.preventDefault();
        event.stopPropagation();
        actions.openMenu(target, event.clientX, event.clientY);
      } : undefined}
      {...(payload && actions ? {
        draggable: true,
        onDragStart: (event: React.DragEvent) => {
          actions.beginDrag(payload);
          if (event.dataTransfer) {
            event.dataTransfer.setData("text/plain", target!.key);
            event.dataTransfer.effectAllowed = "move";
          }
        },
        onDragEnd: () => actions.endDrag(),
      } : {})}
      {...(target && actions ? {
        onDragOver: (event: React.DragEvent) => {
          if (!actions.canDrop(target)) return;
          event.preventDefault();
          event.stopPropagation();
          if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
          actions.hoverDrop(target);
        },
        onDragLeave: (event: React.DragEvent) => {
          if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
          actions.hoverDrop(null);
        },
        onDrop: (event: React.DragEvent) => {
          if (!actions.canDrop(target)) return;
          event.preventDefault();
          event.stopPropagation();
          actions.drop(target);
        },
      } : {})}
    >
      {onToggle ? <button type="button" className="nb-tree-chevron" aria-label={name} aria-expanded={expanded} onClick={onToggle}>
        {expanded ? <Icons.chevD size={12} /> : <Icons.chevR size={12} />}
      </button> : <span className="nb-tree-chevron" />}
      {renaming ? renameInput : onActivate
        ? <button type="button" className="nb-tree-open" title={title ?? name} aria-current={selected ? "page" : undefined} onClick={onActivate}>{label}</button>
        : route && <MemoryLink route={route} values={values} title={title ?? name} aria-current={selected ? "page" : undefined} onClick={event => {
            if (!event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey && event.button === 0) onOpen?.();
          }}>{label}</MemoryLink>}
    </div>
    {expanded && children}
  </li>;
}
