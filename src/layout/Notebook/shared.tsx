import { useCallback, useEffect, useState } from "react";
import { t, type I18nKey } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { memoryNavigate, memoryUrl } from "../Memory/navigation";

export const notebookRoute=(vaultId="",action="")=>vaultId?`notebook/${vaultId}${action?`/${action}`:""}`:"notebooks";
/** Vault files open through the shared document tab, closing the knowledge-base surface when visible. */
export function openVaultFile(absolutePath:string) {
  if(new URLSearchParams(window.location.search).has("memory")) memoryNavigate(memoryUrl(""),false,true);
  useTermStore.getState().openDocTab(absolutePath);
}
export function notebookError(error:unknown):string {
  const code=String(error).match(/kb_[a-z_]+/)?.[0];
  const keys:Record<string,I18nKey>={kb_conflict:"nb.conflict",kb_exists:"nb.exists",kb_invalid_path:"nb.invalid",kb_invalid:"nb.invalid",kb_too_large:"nb.tooLarge",kb_encoding:"nb.readOnly",kb_empty_import:"nb.emptyImport",kb_incomplete_import:"nb.incomplete",kb_move_incomplete:"nb.incomplete",kb_interrupted:"nb.importInterruptedHint",kb_import_busy:"nb.importBusy"};
  return t(keys[code??""]??"nb.error");
}
export function useNotebookLoad<T>(loader:()=>Promise<T>,deps:unknown[],formatError:(error:unknown)=>string=notebookError) {
  const [data,setData]=useState<T|null>(null);const [error,setError]=useState("");const [version,setVersion]=useState(0);
  const reload=useCallback(()=>setVersion(v=>v+1),[]);
  useEffect(()=>{let alive=true;setError("");loader().then(v=>{if(alive)setData(v);}).catch(e=>{if(alive)setError(formatError(e));});return()=>{alive=false;};},[...deps,version]);
  return {data,error,reload,setData};
}
export function NotebookError({error,retry}:{error:string;retry?:()=>void}) {
  return <div className="nb-message" role={error?"alert":"status"}>{error||t("common.loading")}{error&&retry&&<button className="btn" onClick={retry}>{t("common.retry")}</button>}</div>;
}
export function sizeLabel(bytes:number) {return bytes<1024?`${bytes} B`:bytes<1024*1024?`${(bytes/1024).toFixed(1)} KB`:`${(bytes/1024/1024).toFixed(1)} MB`;}

/** A note's display title: its file name without the extension or a trailing `_YYYYMMDD_HHmm` stamp. */
export function noteTitle(name: string): string {
  const base = name.replace(/\.md$/i, "");
  return base.replace(/[_ -]\d{8}(?:[_ -]\d{4,6})?$/, "") || base;
}

/** The folder holding a note, relative to its knowledge base; empty at the root. */
export function noteFolder(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash < 0 ? "" : path.slice(0, slash);
}

/** A path with the backend's home directory shortened to `~`. */
export function homePath(path: string, home?: string | null): string {
  if (!home) return path;
  const base = home.replace(/[\\/]+$/, "");
  if (path === base) return "~";
  return path.startsWith(`${base}/`) || path.startsWith(`${base}\\`) ? `~${path.slice(base.length)}` : path;
}
