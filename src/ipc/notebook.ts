//! File-backed knowledge base protocol; the backend owns paths, limits, content and mutation rules.
import { invoke } from "./transport";
export interface NotebookVault { id:string; name:string; root:string }
export interface NotebookNode { path:string; absolutePath:string; name:string; kind:"folder"|"note"|"asset"|"large"; size:number; updatedAt:number; favorite:boolean; tags:string[]; summary:string }
export interface NotebookOverview { vaults:NotebookVault[]; defaultRoot:string|null; home?:string|null; limits:{noteBytes:number;assetBytes:number;files:number;chunkBytes:number} }
export interface NotebookTree { vault:NotebookVault; nodes:NotebookNode[]; entries:NotebookNode[]; tags:string[]; skipped:number; trash:{id:string;path:string;deletedAt:number}[] }
export interface NotebookDocument { vaultId:string; path:string; absolutePath:string; content:string; digest:string; favorite:boolean; tags:string[]; outline:{text:string;level:number;line:number}[]; links:{target:string;label:string;embed:boolean;path:string|null}[]; backlinks:{path:string}[] }
export const notebookOverview=()=>invoke<NotebookOverview>("kb_overview");
export const notebookRegister=(root:string,create:boolean)=>invoke<{id:string}>("kb_register",{root,create});
export const notebookUnregister=(vaultId:string)=>invoke<void>("kb_unregister",{vaultId});
/** Rename the vault's root folder on disk and follow it in the registration. */
export const notebookVaultRename=(vaultId:string,name:string)=>invoke<NotebookVault>("kb_vault_rename",{vaultId,name});
export interface NotebookSearchRelated { path:string; name:string; absolutePath:string }
export interface NotebookSearchHit { vaultId:string; vaultName:string; path:string; absolutePath:string; name:string; summary:string; line:number; matches:number; score:number; updatedAt:number; favorite:boolean; tags:string[]; matched?:string[]; related:NotebookSearchRelated[] }
/** `fuzzy` marks a page answered by the loosened pass, so the host can say exact matches were empty. */
export interface NotebookSearchResult { entries:NotebookSearchHit[]; total:number; hasMore:boolean; unavailable:{vaultId:string;vaultName:string}[]; fuzzy?:boolean }
export const notebookTree=(vaultId:string,filter="",tag="",sort="updated")=>invoke<NotebookTree>("kb_tree",{vaultId,filter,tag,sort});
/** Content lookup across notes. An empty vaultId searches every registered notebook. */
export const notebookSearch=(query:string,vaultId="",limit=100)=>invoke<NotebookSearchResult>("kb_search",{query,limit,...(vaultId?{vaultId}:{})});
export const notebookGet=(vaultId:string,path:string)=>invoke<NotebookDocument>("kb_get",{vaultId,path});
export const notebookStat=(vaultId:string,path:string)=>invoke<{digest:string}>("kb_stat",{vaultId,path});
export const notebookSave=(vaultId:string,path:string,digest:string,content:string)=>invoke<NotebookDocument>("kb_save",{vaultId,path,digest,content});
export const notebookCreate=(vaultId:string,parent:string,name:string,kind:"note"|"folder",content?:string)=>invoke<{path:string;kind:string;absolutePath:string}>("kb_create",{vaultId,parent,name,kind,content});
export const notebookMove=(vaultId:string,from:string,to:string)=>invoke<{path:string;updatedLinks:number}>("kb_move",{vaultId,from,to});
export const notebookTrash=(vaultId:string,path:string)=>invoke<{id:string}>("kb_trash",{vaultId,path});
export const notebookRestore=(vaultId:string,id:string)=>invoke<{path:string}>("kb_restore",{vaultId,id});
export const notebookFavorite=(vaultId:string,path:string,favorite:boolean)=>invoke<void>("kb_favorite",{vaultId,path,favorite});
export interface ImportPlan {importId:string;indices:number[];skipped:number;bytes:number}
export interface ImportResult {imported:number;skipped:number;paths:string[];absolutePaths:string[]}
export const notebookImportBegin=(vaultId:string,destination:string,files:{path:string;size:number}[])=>invoke<ImportPlan>("kb_import_begin",{vaultId,destination,files});
export const notebookImportChunk=(vaultId:string,importId:string,index:number,offset:number,base64:string)=>invoke<{offset:number}>("kb_import_chunk",{vaultId,importId,index,offset,base64});
export const notebookImportCommit=(vaultId:string,importId:string)=>invoke<ImportResult>("kb_import_commit",{vaultId,importId});
export const notebookImportAbort=(vaultId:string,importId:string,error?:string)=>invoke<void>("kb_import_abort",{vaultId,importId,...(error?{error}:{})});
export interface NotebookImportRecord { id:string; destination:string; status:"running"|"completed"|"failed"|"cancelled"|"interrupted"; error:string; files:number; imported:number; skipped:number; bytes:number; createdAt:number; updatedAt:number; finishedAt:number|null }
export interface NotebookImportFile { path:string; size:number; status:"imported"|"pending"|"skipped"; reason:string }
export const notebookImports=(vaultId:string,page=0)=>invoke<{imports:NotebookImportRecord[];total:number;pageSize:number}>("kb_imports",{vaultId,page});
export const notebookImportDetail=(vaultId:string,importId:string)=>invoke<{files:NotebookImportFile[]}>("kb_import_detail",{vaultId,importId});
export const notebookImportDelete=(vaultId:string,importId:string)=>invoke<void>("kb_import_delete",{vaultId,importId});
export async function notebookAsset(vaultId:string,path:string):Promise<Blob> {
  const parts:Uint8Array[]=[];let offset=0;let size=0;let mime="";let stamp:string|undefined;
  do {
    const chunk=await invoke<{base64:string;size:number;mime:string;offset:number;stamp:string}>("kb_asset",{vaultId,path,offset,stamp});
    if (size && size!==chunk.size || chunk.offset<=offset && chunk.size>offset) throw new Error("kb_conflict");
    const bytes=Uint8Array.from(atob(chunk.base64),c=>c.charCodeAt(0));parts.push(bytes);
    size=chunk.size;mime=chunk.mime;offset=chunk.offset;stamp=chunk.stamp;
  } while(offset<size);
  return new Blob(parts as BlobPart[],{type:mime});
}
/** Browser folder drops expose entries rather than the complete FileList supplied by a folder picker. */
export async function notebookDroppedFiles(transfer:DataTransfer):Promise<File[]> {
  const entries=Array.from(transfer.items??[]).filter(item=>item.kind==="file").map(item=>item.webkitGetAsEntry?.()).filter((entry):entry is FileSystemEntry=>!!entry);
  if(!entries.length)return Array.from(transfer.files);
  const files:File[]=[];
  const visit=async(entry:FileSystemEntry,parent:string)=>{
    const path=parent?`${parent}/${entry.name}`:entry.name;
    if(entry.isFile){const file=await new Promise<File>((resolve,reject)=>(entry as FileSystemFileEntry).file(resolve,reject));Object.defineProperty(file,"webkitRelativePath",{value:path});files.push(file);}
    else if(entry.isDirectory){const reader=(entry as FileSystemDirectoryEntry).createReader();for(;;){const batch=await new Promise<FileSystemEntry[]>((resolve,reject)=>reader.readEntries(resolve,reject));if(!batch.length)break;for(const child of batch)await visit(child,path);}}
  };
  for(const entry of entries)await visit(entry,"");return files;
}
