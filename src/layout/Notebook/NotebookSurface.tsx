import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { ContextMenu } from "../../components/ContextMenu";
import Icons from "../../components/Icons";
import Select from "../../components/Select";
import { useT } from "../../i18n";
import { notebookOverview, notebookRestore, notebookTree, type NotebookOverview, type NotebookNode } from "../../ipc/notebook";
import { MemoryLink, memoryNavigate, memoryUrl, useMemoryLocation } from "../Memory/navigation";
import { shortDate } from "../Memory/shared";
import { CopyKnowledgeDialog, NotebookDialog } from "./NotebookDialogs";
import { KnowledgeSearchResults, useKnowledgeSearch, useSearchKeys } from "./KnowledgeSearch";
import { homePath, noteFolder, noteTitle, NotebookError, notebookError, notebookRoute, openVaultFile, useNotebookLoad } from "./shared";
import { KnowledgeHome } from "./KnowledgeHome";
import { knowledgeSelection } from "./KnowledgeTreeRow";
import { ImportRecords } from "./ImportRecords";
import { KnowledgeBackLink } from "./KnowledgeBack";
import { useNotebookImports, type NotebookImportOutcome } from "./importManager";
import "./notebook.css";
import "../Memory/kbx.css";

export function NotebookSurface({route}:{route:string}) {
  const {data,error,reload}=useNotebookLoad(notebookOverview,[]);const [page,id="",action=""]=route.split('/');
  useEffect(()=>{window.addEventListener("notebook:vaultsChanged",reload);window.addEventListener("focus",reload);return()=>{window.removeEventListener("notebook:vaultsChanged",reload);window.removeEventListener("focus",reload);};},[reload]);
  if(!data)return <NotebookError error={error} retry={reload}/>;
  if(page==="notebook")return <NotebookWorkspace key={id} id={id} action={action} overview={data} onVaultsChanged={reload}/>;
  if(id==="copy")return <CopyKnowledgeDialog overview={data} onChanged={reload}/>;
  return <>{error&&<NotebookError error={error} retry={reload}/>}<KnowledgeHome overview={data}/></>;
}

const NOTE_LAYOUT_KEY="vlx-notes-layout";

/** The note list's layout: the URL when it says, otherwise the one last chosen, otherwise cards. */
function noteLayout(fromUrl:string|null):"cards"|"list"{
  let saved:string|null=null;
  try{saved=localStorage.getItem(NOTE_LAYOUT_KEY);}catch{/* Storage can be unavailable; the default still works. */}
  return (fromUrl??saved)==="list"?"list":"cards";
}
function rememberNoteLayout(layout:string){
  try{localStorage.setItem(NOTE_LAYOUT_KEY,layout);}catch{/* The choice still applies through the URL. */}
}

function NotebookWorkspace({id,action,overview,onVaultsChanged}:{id:string;action:string;overview:NotebookOverview;onVaultsChanged:()=>void}) {
  const t=useT();const search=useMemoryLocation();const params=new URLSearchParams(search);const legacyPath=params.get("memoryPath")??"";
  const query=params.get("memoryQuery")??"";const filter=params.get("memoryFilter")??"";const tag=params.get("memoryTag")??"";const folder=params.get("memoryFolder")??"";const sort=params.get("memorySort")??"updated";const layout=noteLayout(params.get("memoryLayout"));
  const {data,error,reload}=useNotebookLoad(()=>notebookTree(id,filter,tag,sort),[id,filter,tag,sort]);
  const [input,setInput]=useState(query);const [failure,setFailure]=useState("");const [menu,setMenu]=useState<{x:number;y:number}|null>(null);
  const running=useNotebookImports().get(id);const [notice,setNotice]=useState<NotebookImportOutcome|null>(null);
  useEffect(()=>{
    let timer:ReturnType<typeof setTimeout>|undefined;
    const finished=(event:Event)=>{const outcome=(event as CustomEvent<NotebookImportOutcome>).detail;if(outcome?.vaultId!==id)return;setNotice(outcome);reload();clearTimeout(timer);timer=setTimeout(()=>setNotice(null),10000);};
    window.addEventListener("notebook:importFinished",finished);return()=>{window.removeEventListener("notebook:importFinished",finished);clearTimeout(timer);};
  },[id,reload]);
  // "all" widens the lookup to every registered notebook; an empty scope keeps it inside this one.
  const scope=params.get("memoryScope")??"";const searching=query.trim().length>0;
  const lookup=useKnowledgeSearch(query,scope==="all"?"":id);
  const hits=lookup.result?.entries??[];
  const openHit=(index:number)=>{const hit=hits[index];if(hit)openVaultFile(hit.absolutePath);};
  const keys=useSearchKeys(hits.length,openHit,()=>{setInput("");memoryNavigate(memoryUrl(notebookRoute(id),{memoryQuery:null,memoryScope:null}),true);});
  const searchInput=useRef<HTMLInputElement>(null);
  useEffect(()=>setInput(query),[query]);
  useEffect(()=>{if(input===query)return;const timer=setTimeout(()=>memoryNavigate(memoryUrl(notebookRoute(id),{memoryQuery:input,memoryFolder:null}),true),250);return()=>clearTimeout(timer);},[input,query,id]);
  useEffect(()=>{const focus=()=>{if(!document.hidden)reload();};const timer=setInterval(focus,5000);window.addEventListener("focus",focus);return()=>{clearInterval(timer);window.removeEventListener("focus",focus);};},[reload]);
  // Notes opened before the knowledge base used document tabs still arrive with a memoryPath URL.
  useEffect(()=>{
    if(!legacyPath||!data)return;
    const node=data.nodes.find(n=>n.path===legacyPath);
    if(node&&node.kind!=="folder")openVaultFile(node.absolutePath);
  },[legacyPath,data]);
  useEffect(()=>{
    const keys=(event:KeyboardEvent)=>{
      if(!event.metaKey&&!event.ctrlKey||action||new URLSearchParams(location.search).has("kbAction"))return;
      if(event.key.toLowerCase()==="n"){event.preventDefault();event.stopImmediatePropagation();memoryNavigate(memoryUrl(notebookRoute(id,"new")));}
      if(event.key.toLowerCase()==="o"){event.preventDefault();event.stopImmediatePropagation();memoryNavigate(memoryUrl(notebookRoute(id,"quick")));}
      if(event.shiftKey&&event.key.toLowerCase()==="f"){event.preventDefault();searchInput.current?.focus();}
    };window.addEventListener("keydown",keys,true);return()=>window.removeEventListener("keydown",keys,true);
  },[id,action]);
  if(!data)return <NotebookError error={error} retry={reload}/>;
  const navigateList=(values:Record<string,string|null>)=>memoryNavigate(memoryUrl(notebookRoute(id),{memoryPath:null,memoryQuery:null,memoryFolder:null,memoryTag:null,...values}));
  const entries=folder?data.entries.filter(n=>n.path.startsWith(`${folder}/`)):data.entries;
  // A query replaces the note list with hits; the surrounding vault bar and view switch stay in place.
  const searchPane=<div className="kbx-list nb-search-library">
    <div className="nb-search-head"><strong role="status">{lookup.result?t("nb.searchCount",String(lookup.result.total)):t("common.loading")}</strong></div>
    <KnowledgeSearchResults query={query} result={lookup.result} error={lookup.error} active={keys.active} onActive={keys.setActive} onOpen={openHit} onOpenRelated={(related)=>openVaultFile(related.absolutePath)}/>
  </div>;
  const openMenu=(event:ReactMouseEvent<HTMLButtonElement>)=>{const rect=event.currentTarget.getBoundingClientRect();setMenu({x:rect.right-180,y:rect.bottom+4});};
  const viewTab=filter==="favorites"||filter==="trash"?filter:action==="imports"?"imports":"recent";
  return <div className="nb-workspace kbx-page kbx-narrow">
    <div className="kbx-bar">
      <KnowledgeBackLink className="kbx-ibtn"/>
      <nav className="kbx-crumbs" aria-label={t("nb.vaults")}><MemoryLink route="notebooks" values={knowledgeSelection}>{t("nb.localVaults")}</MemoryLink><Icons.chevR size={12} aria-hidden="true"/><MemoryLink className={folder?"":"cur"} route={notebookRoute(id)} values={{memoryPath:null,memoryFolder:null}}>{data.vault.name}</MemoryLink>{folder&&<><Icons.chevR size={12} aria-hidden="true"/><b>{folder}</b></>}</nav>
      <div className="kbx-acts">
        {running&&<MemoryLink className="nb-import-chip" route={notebookRoute(id,"imports")} title={running.current}><Icons.upload size={12}/><span>{t("nb.importProgress",String(running.done),String(running.files))}</span></MemoryLink>}
        <MemoryLink className="kbx-ibtn" title={t("nb.newFolder")} aria-label={t("nb.newFolder")} route={notebookRoute(id,"folder")}><Icons.folderPlus size={16}/></MemoryLink>
        <MemoryLink className="kbx-ibtn" title={t("nb.import")} aria-label={t("nb.import")} route={notebookRoute(id,"import")}><Icons.upload size={16}/></MemoryLink>
        <button type="button" className="kbx-ibtn" title={t("kb.moreActions")} aria-label={t("kb.moreActions")} onClick={openMenu}><Icons.more size={16} aria-hidden="true"/></button>
        <MemoryLink className="kbx-btn pri" route={notebookRoute(id,"new")}><Icons.plus size={14}/>{t("nb.newNote")}</MemoryLink>
      </div>
    </div>
    {menu&&<ContextMenu x={menu.x} y={menu.y} onClose={()=>setMenu(null)} items={[
      ...(folder?[{label:t("nb.move"),onClick:()=>memoryNavigate(memoryUrl(notebookRoute(id,"move"),{memoryPath:folder}))},{label:t("common.delete"),danger:true,onClick:()=>memoryNavigate(memoryUrl(notebookRoute(id,"delete"),{memoryPath:folder}))}]:[]),
      {label:t("nb.closeVault"),onClick:()=>memoryNavigate(memoryUrl(notebookRoute(id,"close")))},
    ]}/>}
    <div className="kbx-head"><div className="t">
      <div className="t1"><Icons.notebook size={16} aria-hidden="true"/><h1>{data.vault.name}</h1></div>
      <div className="kbx-meta"><span title={data.vault.root}>{homePath(data.vault.root,overview.home)}</span><span><b>{data.entries.length.toLocaleString()}</b> {t("nb.notesWord",String(data.entries.length))}</span>{!!data.entries.length&&<span>{t("kb.updatedOn",shortDate(Math.max(...data.entries.map(note=>note.updatedAt))))}</span>}</div>
    </div></div>
    <nav className="kbx-tabs" aria-label={t("nb.vaults")}>
      <MemoryLink route={notebookRoute(id)} values={knowledgeSelection} className={viewTab==="recent"?"on":""} aria-current={viewTab==="recent"?"page":undefined}>{t("nb.recent")}</MemoryLink>
      <MemoryLink route={notebookRoute(id)} values={{...knowledgeSelection,memoryFilter:"favorites"}} className={viewTab==="favorites"?"on":""} aria-current={viewTab==="favorites"?"page":undefined}>{t("nb.favorites")}</MemoryLink>
      <MemoryLink route={notebookRoute(id)} values={{...knowledgeSelection,memoryFilter:"trash"}} className={viewTab==="trash"?"on":""} aria-current={viewTab==="trash"?"page":undefined}>{t("nb.trash")}</MemoryLink>
      <MemoryLink route={notebookRoute(id,"imports")} values={knowledgeSelection} className={viewTab==="imports"?"on":""} aria-current={viewTab==="imports"?"page":undefined}>{t("nb.imports")}</MemoryLink>
    </nav>
    {notice&&<div className={`nb-banner nb-import-notice ${notice.status}`} role="status"><span>{notice.status==="completed"?`${t("nb.imported")}: ${notice.imported} · ${t("nb.skipped")}: ${notice.skipped}`:notebookError(notice.error)}</span><MemoryLink route={notebookRoute(id,"imports")}>{t("nb.imports")}</MemoryLink><button type="button" className="kbx-ibtn" title={t("common.close")} aria-label={t("common.close")} onClick={()=>setNotice(null)}><Icons.x size={14}/></button></div>}
    {(error||failure)&&<NotebookError error={error||failure} retry={()=>{setFailure("");reload();}}/>}
    {action==="imports"?<div className="kbx-list kbx-pad"><ImportRecords id={id}/></div>:action&&action!=="quick"?<div className="kbx-scroll"><NotebookDialog key={`${action}:${folder}`} action={action} tree={data} overview={overview} onChanged={()=>{reload();onVaultsChanged();}}/></div>:action==="quick"?<div className="kbx-scroll"><QuickOpen id={id} entries={data.nodes.filter(n=>n.kind==="note")}/></div>:filter==="trash"?<div className="kbx-list kbx-border-top">{data.trash.length?<div>{data.trash.map(item=><div className="kbx-it" key={item.id}><Icons.archive size={14} aria-hidden="true"/><b>{item.path}</b><span className="w kbx-trash-time">{shortDate(item.deletedAt)}</span><button className="kbx-btn" onClick={()=>{void notebookRestore(id,item.id).then(()=>reload()).catch(e=>setFailure(notebookError(e)));}}>{t("nb.restore")}</button></div>)}</div>:<p className="kbx-empty">{t("nb.emptyTrash")}</p>}</div>:<>
      <div className="kbx-filter">
        <label className="kbx-input"><Icons.search size={12} aria-hidden="true"/><input ref={searchInput} aria-label={t("nb.search")} placeholder={t("nb.search")} value={input} onKeyDown={keys.onKeyDown} onChange={e=>setInput(e.target.value)}/></label>
        <Select size="sm" width="auto" menuWidth={200} value={scope} ariaLabel={t("nb.searchScope")} onChange={value=>memoryNavigate(memoryUrl(notebookRoute(id),{memoryQuery:query||null,memoryFolder:null,memoryScope:value||null}),true)} options={[{ value: "", label: t("nb.searchThisVault") }, { value: "all", label: t("nb.searchAllVaults") }]} />
        <span className="g"/>
        <Select size="sm" width="auto" menuWidth={180} value={tag} ariaLabel={t("nb.tags")} onChange={value=>navigateList({memoryTag:value,memoryFilter:filter})} options={[{ value: "", label: t("memory.allTags") }, ...data.tags.map(value=>({ value, label: value }))]} />
        <Select size="sm" width="auto" menuWidth={180} value={sort} ariaLabel={t("memory.updated")} onChange={value=>navigateList({memorySort:value,memoryFilter:filter})} options={[{ value: "updated", label: t("memory.updated") }, { value: "title", label: t("memory.titleSort") }]} />
        <Select size="sm" width="auto" menuWidth={140} value={layout} ariaLabel={t("nb.layout")} onChange={value=>{rememberNoteLayout(value);memoryNavigate(memoryUrl(notebookRoute(id),{memoryLayout:value}),true);}} options={[{ value: "cards", label: t("nb.layoutCards") }, { value: "list", label: t("nb.layoutList") }]} />
      </div>
      {searching?searchPane:<div className="kbx-note-wrap">
        {entries.length?layout==="list"?<div className="kbx-note-list">{entries.map(note=><button type="button" key={note.path} className="kbx-nr" onClick={()=>openVaultFile(note.absolutePath)}><span className="a"><span className="tt" title={note.name}>{noteTitle(note.name)}</span><span className="sn">{note.summary}</span></span><span className="m" title={note.path}>{noteFolder(note.path)}</span><span className="d">{shortDate(note.updatedAt)}</span></button>)}</div>:<div className="kbx-note-cards">{entries.map(note=><button type="button" key={note.path} className="kbx-nc" onClick={()=>openVaultFile(note.absolutePath)}><span className="tt" title={note.name}>{noteTitle(note.name)}</span><div className="sn">{note.summary}</div><div className="ft"><span title={note.path}>{noteFolder(note.path)}</span><span>{shortDate(note.updatedAt)}</span></div></button>)}</div>:<div className="kbx-empty"><p>{t("nb.emptyNotes")}</p><MemoryLink className="kbx-btn pri" route={notebookRoute(id,"new")}><Icons.plus size={14}/>{t("nb.newNote")}</MemoryLink></div>}
        {data.skipped>0&&<p className="kbx-empty">{t("nb.skipped")}: {data.skipped}</p>}
      </div>}
    </>}
  </div>;
}

function QuickOpen({id,entries}:{id:string;entries:NotebookNode[]}) {
  const t=useT();const location=useMemoryLocation();const query=new URLSearchParams(location).get("memoryQuickQuery")??"";
  const setQuery=(value:string)=>memoryNavigate(memoryUrl(notebookRoute(id,"quick"),{memoryQuickQuery:value}),true);const [selected,setSelected]=useState(0);const results=entries.filter(n=>n.path.toLowerCase().includes(query.toLowerCase())).slice(0,60);
  const open=(node:NotebookNode)=>openVaultFile(node.absolutePath);
  return <section className="nb-dialog nb-quick-dialog"><header><h2>{t("nb.quickOpen")}</h2><MemoryLink route={notebookRoute(id)}>{t("common.cancel")}</MemoryLink></header><input className="input" autoFocus aria-label={t("nb.quickOpen")} value={query} onChange={e=>{setQuery(e.target.value);setSelected(0);}} onKeyDown={e=>{if(e.key==="ArrowDown"){e.preventDefault();setSelected(s=>Math.min(s+1,results.length-1));}else if(e.key==="ArrowUp"){e.preventDefault();setSelected(s=>Math.max(0,s-1));}else if(e.key==="Enter"&&results[selected])open(results[selected]);else if(e.key==="Escape")memoryNavigate(memoryUrl(notebookRoute(id)));}}/><div className="nb-quick-results">{results.map((n,i)=><button key={n.path} type="button" className={i===selected?"active":""} onClick={()=>open(n)}><Icons.docLines/><span>{n.path}</span></button>)}</div></section>;
}
