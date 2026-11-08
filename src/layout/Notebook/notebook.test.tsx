// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { KnowledgeNavigation } from "./KnowledgeNavigation";
import { KnowledgeVaultDialogs } from "./VaultActions";
import { MemoryRoute } from "../Memory/MemoryRoute";

const api=vi.hoisted(()=>({overview:vi.fn(),tree:vi.fn(),list:vi.fn(),collections:vi.fn(),get:vi.fn(),save:vi.fn(),stat:vi.fn(),register:vi.fn(),create:vi.fn(),restore:vi.fn(),pick:vi.fn(),env:{isBrowser:true},move:vi.fn(),trash:vi.fn(),unregister:vi.fn(),vaultRename:vi.fn(),memoryRename:vi.fn(),memoryDelete:vi.fn(),memoryMove:vi.fn(),memoryGroupRename:vi.fn(),memoryGroupMove:vi.fn(),memoryGroupDelete:vi.fn()}));
vi.mock("../../ipc/notebook",()=>({notebookOverview:api.overview,notebookTree:api.tree,notebookGet:api.get,notebookSave:api.save,notebookStat:api.stat,notebookRegister:api.register,notebookCreate:api.create,notebookRestore:api.restore,notebookMove:api.move,notebookTrash:api.trash,notebookUnregister:api.unregister,notebookVaultRename:api.vaultRename,notebookAsset:vi.fn(),notebookFavorite:vi.fn(),uploadNotebookFiles:vi.fn(),notebookDroppedFiles:vi.fn()}));
vi.mock("../../ipc/memory",()=>({memoryAttention:()=>Promise.resolve(0),memoryList:api.list,memoryGet:vi.fn(),memoryCollections:api.collections,memoryRename:api.memoryRename,memoryDelete:api.memoryDelete,memoryMove:api.memoryMove,memoryGroupRename:api.memoryGroupRename,memoryGroupMove:api.memoryGroupMove,memoryGroupDelete:api.memoryGroupDelete}));
vi.mock("../../platform",()=>({platform:{env:api.env,dialog:{pickDirectory:api.pick}}}));
vi.mock("../../remote/ServerFileBrowser",()=>({cardStyle:{},joinPath:(parent:string,name:string)=>`${parent.replace(/\/$/, "")}/${name}`,useServerBrowser:()=>({selectedDir:"/existing/Markdown"}),ServerBrowserView:()=> <div>Server directory picker</div>}));
const vault={id:"vault-1",name:"Personal",root:"/notes"};
const node={path:"Projects/Note.md",absolutePath:"/notes/Projects/Note.md",name:"Note.md",kind:"note",size:10,updatedAt:1,favorite:false,tags:[],summary:"Body"};
beforeEach(()=>{
  setLang("en");api.env.isBrowser=true;api.pick.mockReset();window.history.replaceState(null,"","/");
  useTermStore.setState({docTabs:{},openTabs:[],activeTabId:null});
  api.list.mockReset().mockResolvedValue({projects:[],selectedSessionId:null,entries:[],total:0,pageSize:40,tags:[]});
  api.collections.mockReset().mockResolvedValue({projects:[],sessions:[]});
  api.overview.mockReset().mockResolvedValue({vaults:[vault],defaultRoot:"/Notes",limits:{chunkBytes:1024,noteBytes:10000,assetBytes:10000,files:100}});
  api.tree.mockReset().mockResolvedValue({vault,nodes:[{...node,path:"Projects",absolutePath:"/notes/Projects",name:"Projects",kind:"folder"},node],entries:[node],tags:[],trash:[],skipped:0});
  api.get.mockReset().mockResolvedValue({});api.stat.mockReset().mockResolvedValue({digest:"old"});api.save.mockReset();api.register.mockReset().mockResolvedValue({id:vault.id});api.create.mockReset().mockResolvedValue({path:"New.md",kind:"note",absolutePath:"/notes/New.md"});api.restore.mockReset().mockResolvedValue({path:"Restored.md"});
});
afterEach(()=>{cleanup();vi.clearAllMocks();});

const opened=(path:string)=>Object.values(useTermStore.getState().docTabs).some(tab=>tab.path===path);

it("groups the session knowledge base and local knowledge bases with command buttons above them",async()=>{
  render(<KnowledgeNavigation/>);
  const link=await screen.findByRole("link",{name:"Personal"});
  expect(new URL(link.getAttribute("href")!).searchParams.get("memory")).toBe("notebook/vault-1");
  expect(screen.getByText("Local knowledge bases").closest(".kbx-lab")).toBeTruthy();
  expect(screen.getByRole("link",{name:"Session Knowledge Base"}).closest(".kbx-lab")).toBeTruthy();
  expect(screen.getByRole("button",{name:"Open knowledge base"})).toBeTruthy();
  expect(screen.getByRole("button",{name:"Create knowledge base"})).toBeTruthy();
  expect(screen.queryByRole("link",{name:"Open knowledge base"})).toBeNull();
});


it("expands multiple root folders inline without navigating away from the current content",async()=>{
  api.list.mockResolvedValue({projects:[{id:"p",name:"Conversation project",kind:"project",count:1,sessions:[{id:"s",name:"Design session",kind:"session",count:1}]}],selectedSessionId:null,entries:[],total:0,pageSize:40,tags:[]});
  render(<KnowledgeNavigation/>);
  fireEvent.click(await screen.findByRole("button",{name:"Personal"}));
  await screen.findByRole("link",{name:"Projects"});
  expect(await screen.findByRole("link",{name:/Conversation project/})).toBeTruthy();
  expect(new URLSearchParams(location.search).get("memory")).toBeNull();
  expect(screen.getByRole("button",{name:"Personal"}).getAttribute("aria-expanded")).toBe("true");
  expect(document.querySelector(".nb-navigation-content")).toBeNull();
  expect(screen.queryByText("Projects and sessions")).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Personal"}));
  expect(screen.queryByRole("link",{name:"Projects"})).toBeNull();
  expect(screen.getByRole("link",{name:/Conversation project/})).toBeTruthy();
});

it("restores the home page with all knowledge bases and direct dialog actions",async()=>{
  window.history.replaceState(null,"","/?memory=notebooks");render(<MemoryRoute/>);
  await screen.findByRole("heading",{name:"Knowledge Base"});
  const home=document.querySelector(".kbx-home") as HTMLElement;
  expect(within(home).getByRole("heading",{name:"Session Knowledge Base"})).toBeTruthy();
  expect(within(home).getByRole("heading",{name:"Local knowledge bases"})).toBeTruthy();
  expect(within(home).getByRole("link",{name:/Session Knowledge Base/})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Open knowledge base"}));
  await screen.findByRole("dialog",{name:"Open knowledge base"});
  expect(new URLSearchParams(location.search).get("memory")).toBe("notebooks");
  fireEvent.keyDown(screen.getByRole("dialog"),{key:"Escape"});
  fireEvent.click(within(home).getByRole("link",{name:/Local knowledge bases/}));
  await screen.findByRole("link",{name:"Recent notes"});
  expect(new URLSearchParams(location.search).get("memory")).toBe("notebook/vault-1");
  expect(screen.queryByRole("link",{name:"Quick open"})).toBeNull();
  expect(screen.getByRole("link",{name:"Import"})).toBeTruthy();
});

it("opens a vault file from the directory tree as a document tab",async()=>{
  render(<KnowledgeNavigation/>);
  fireEvent.click(await screen.findByRole("button",{name:"Personal"}));
  fireEvent.click(await screen.findByRole("link",{name:"Projects"}));
  fireEvent.click(await screen.findByRole("button",{name:"Note"}));
  await waitFor(()=>expect(opened(node.absolutePath)).toBe(true));
});

it("opens a note from the list as a document tab and closes the knowledge surface",async()=>{
  window.history.replaceState(null,"","/?memory=notebook/vault-1");render(<MemoryRoute/>);
  await screen.findByRole("link",{name:"Recent notes"});
  fireEvent.click(screen.getByRole("button",{name:/Note/}));
  await waitFor(()=>expect(opened(node.absolutePath)).toBe(true));
  expect(new URLSearchParams(location.search).has("memory")).toBe(false);
});

it("redirects a former note URL to a document tab",async()=>{
  window.history.replaceState(null,"",`/?memory=notebook/vault-1&memoryPath=${encodeURIComponent(node.path)}`);render(<MemoryRoute/>);
  await waitFor(()=>expect(opened(node.absolutePath)).toBe(true));
  expect(new URLSearchParams(location.search).has("memory")).toBe(false);
});

it("keeps quick open reachable from the keyboard and opens the selected note",async()=>{
  window.history.replaceState(null,"","/?memory=notebook/vault-1");render(<MemoryRoute/>);
  await screen.findByRole("link",{name:"Recent notes"});
  expect(screen.queryByRole("link",{name:"Quick open"})).toBeNull();
  fireEvent.keyDown(window,{key:"o",metaKey:true});
  await screen.findByRole("heading",{name:"Quick open"});
  expect(new URLSearchParams(location.search).get("memory")).toBe("notebook/vault-1/quick");
  fireEvent.keyDown(screen.getByRole("textbox",{name:"Quick open"}),{key:"Enter"});
  await waitFor(()=>expect(opened(node.absolutePath)).toBe(true));
});

it("registers the selected existing folder directly without replacing the center with a form",async()=>{
  render(<><KnowledgeNavigation/><KnowledgeVaultDialogs/></>);
  fireEvent.click(screen.getByRole("button",{name:"Open knowledge base"}));
  expect(new URLSearchParams(location.search).get("memory")).toBeNull();
  expect(new URLSearchParams(location.search).get("kbAction")).toBe("open");
  fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button",{name:"Open"}));
  await waitFor(()=>expect(api.register).toHaveBeenCalledWith("/existing/Markdown",false));
  expect(new URLSearchParams(location.search).get("memory")).toBe("notebook/vault-1");
  expect(new URLSearchParams(location.search).get("kbAction")).toBeNull();
});

it("opens the desktop directory picker once and registers its result immediately",async()=>{
  api.env.isBrowser=false;api.pick.mockResolvedValue("/native/Notes");
  window.history.replaceState(null,"","/?kbAction=open");render(<KnowledgeVaultDialogs/>);
  await waitFor(()=>expect(api.register).toHaveBeenCalledWith("/native/Notes",false));
  expect(api.pick).toHaveBeenCalledTimes(1);
  expect(new URLSearchParams(location.search).get("memory")).toBe("notebook/vault-1");
});

it("does not register a directory when the system picker is cancelled",async()=>{
  api.env.isBrowser=false;api.pick.mockResolvedValue(null);
  window.history.replaceState(null,"","/?memory=notebook/vault-1&memoryPath=Projects%2FNote.md&kbAction=open");
  render(<KnowledgeVaultDialogs/>);
  await waitFor(()=>expect(new URLSearchParams(location.search).has("kbAction")).toBe(false));
  expect(api.register).not.toHaveBeenCalled();
  expect(new URLSearchParams(location.search).get("memoryPath")).toBe(node.path);
});

it("creates a knowledge base in a dialog, retains backend errors and opens the returned ID",async()=>{
  window.history.replaceState(null,"","/?kbAction=new");
  api.register.mockRejectedValueOnce(new Error("kb_exists")).mockResolvedValueOnce({id:"created"});
  render(<KnowledgeVaultDialogs/>);
  fireEvent.change(await screen.findByLabelText("Name"),{target:{value:"Journal"}});
  fireEvent.change(screen.getByLabelText("Folder"),{target:{value:"/chosen"}});
  fireEvent.click(screen.getByRole("button",{name:"Create knowledge base"}));
  await screen.findByRole("alert");
  expect(api.register).toHaveBeenCalledWith("/chosen/Journal",true);
  expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe("Journal");
  fireEvent.click(screen.getByRole("button",{name:"Create knowledge base"}));
  await waitFor(()=>expect(new URLSearchParams(location.search).get("memory")).toBe("notebook/created"));
});

it("redirects former open-page URLs to the directory picker",async()=>{
  window.history.replaceState(null,"","/?memory=notebooks/open");render(<MemoryRoute/>);
  await screen.findByRole("dialog",{name:"Open knowledge base"});
  expect(new URLSearchParams(location.search).get("memory")).toBe("notebooks");
  await screen.findByRole("heading",{name:"Knowledge Base"});
});

it("creates a note and opens it as a document tab",async()=>{
  window.history.replaceState(null,"","/?memory=notebook/vault-1/new");render(<MemoryRoute/>);
  fireEvent.change(await screen.findByLabelText("Name"),{target:{value:"New"}});
  fireEvent.click(screen.getByRole("button",{name:"OK"}));
  await waitFor(()=>expect(api.create).toHaveBeenCalledWith("vault-1","","New","note",undefined));
  await waitFor(()=>expect(opened("/notes/New.md")).toBe(true));
  expect(new URLSearchParams(location.search).has("memory")).toBe(false);
});

const entry={id:"e1",title:"Entry one",summary:"",tags:[],version:3,updatedAt:1,sourceCount:0,sessionId:"s1"};
const memoryTree=()=>api.list.mockImplementation((args:{projectId?:string})=>Promise.resolve(args.projectId
  ? {projects:[],selectedSessionId:null,entries:[entry],total:1,pageSize:40,tags:[]}
  : {projects:[{id:"p1",name:"Conversation project",kind:"project",count:1,sessions:[{id:"s1",name:"Design session",kind:"session",count:1},{id:"s2",name:"Other session",kind:"session",count:0}]}],selectedSessionId:null,entries:[],total:0,pageSize:40,tags:[]}));
const expandMemoryEntry=async()=>{
  fireEvent.click(await screen.findByRole("button",{name:/Conversation project/}));
  fireEvent.click(await screen.findByRole("button",{name:"Design session"}));
  await screen.findByText("Entry one");
};
const rowOf=async(name:string)=>(await screen.findByText(name)).closest(".nb-tree-row") as HTMLElement;

it("renames a knowledge entry inline from the context menu",async()=>{
  memoryTree();render(<KnowledgeNavigation/>);
  await expandMemoryEntry();
  fireEvent.contextMenu(await rowOf("Entry one"));
  fireEvent.click(await screen.findByText("Rename"));
  const input=screen.getByRole("textbox",{name:"Rename"}) as HTMLInputElement;
  expect(input.value).toBe("Entry one");
  fireEvent.change(input,{target:{value:"Renamed entry"}});
  fireEvent.keyDown(input,{key:"Enter"});
  await waitFor(()=>expect(api.memoryRename).toHaveBeenCalledWith("e1",3,"Renamed entry"));
});

it("deletes a knowledge entry only after the confirmation",async()=>{
  memoryTree();render(<KnowledgeNavigation/>);
  await expandMemoryEntry();
  fireEvent.contextMenu(await rowOf("Entry one"));
  fireEvent.click(await screen.findByText("Delete"));
  const dialog=await screen.findByRole("alertdialog");
  expect(within(dialog).getByText("Entry one")).toBeTruthy();
  expect(api.memoryDelete).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button",{name:"OK"}));
  await waitFor(()=>expect(api.memoryDelete).toHaveBeenCalledWith("e1",3));
});

it("moves a knowledge entry onto another session by dragging",async()=>{
  memoryTree();render(<KnowledgeNavigation/>);
  await expandMemoryEntry();
  const source=await rowOf("Entry one");
  const target=(await screen.findByText("Other session")).closest(".nb-tree-row") as HTMLElement;
  fireEvent.dragStart(source,{dataTransfer:{setData:()=>{},effectAllowed:""}});
  fireEvent.dragOver(target,{dataTransfer:{dropEffect:""}});
  fireEvent.drop(target,{dataTransfer:{}});
  await waitFor(()=>expect(api.memoryMove).toHaveBeenCalledWith("e1",3,"s2"));
});

it("renames a local knowledge base folder and closes it from the context menu",async()=>{
  render(<KnowledgeNavigation/>);
  const row=await rowOf("Personal");
  fireEvent.contextMenu(row);
  fireEvent.click(await screen.findByText("Rename"));
  const input=screen.getByRole("textbox",{name:"Rename"}) as HTMLInputElement;
  expect(input.value).toBe("Personal");
  fireEvent.change(input,{target:{value:"Journal"}});
  fireEvent.keyDown(input,{key:"Enter"});
  await waitFor(()=>expect(api.vaultRename).toHaveBeenCalledWith("vault-1","Journal"));

  fireEvent.contextMenu(await rowOf("Personal"));
  fireEvent.click(await screen.findByText("Close knowledge base"));
  const dialog=await screen.findByRole("alertdialog");
  fireEvent.click(within(dialog).getByRole("button",{name:"Close knowledge base"}));
  await waitFor(()=>expect(api.unregister).toHaveBeenCalledWith("vault-1"));
});

it("renames and trashes a note from the directory tree",async()=>{
  render(<KnowledgeNavigation/>);
  fireEvent.click(await screen.findByRole("button",{name:"Personal"}));
  fireEvent.click(await screen.findByRole("button",{name:"Projects"}));
  fireEvent.contextMenu(await rowOf("Note"));
  fireEvent.click(await screen.findByText("Rename"));
  const input=screen.getByRole("textbox",{name:"Rename"}) as HTMLInputElement;
  expect(input.value).toBe("Note");
  fireEvent.change(input,{target:{value:"Renamed"}});
  fireEvent.keyDown(input,{key:"Enter"});
  await waitFor(()=>expect(api.move).toHaveBeenCalledWith("vault-1","Projects/Note.md","Projects/Renamed.md"));

  fireEvent.contextMenu(await rowOf("Note"));
  fireEvent.click(await screen.findByText("Delete"));
  const dialog=await screen.findByRole("alertdialog");
  fireEvent.click(within(dialog).getByRole("button",{name:"OK"}));
  await waitFor(()=>expect(api.trash).toHaveBeenCalledWith("vault-1","Projects/Note.md"));
});

it("reports a rejected rename without leaving the tree in the rename state",async()=>{
  memoryTree();api.memoryRename.mockRejectedValueOnce(new Error("memory_duplicate_title"));
  render(<KnowledgeNavigation/>);
  await expandMemoryEntry();
  fireEvent.contextMenu(await rowOf("Entry one"));
  fireEvent.click(await screen.findByText("Rename"));
  const input=screen.getByRole("textbox",{name:"Rename"}) as HTMLInputElement;
  fireEvent.change(input,{target:{value:"Taken title"}});
  fireEvent.keyDown(input,{key:"Enter"});
  const alert=await screen.findByRole("alert");
  expect(alert.textContent).toContain("already exists");
  expect(screen.queryByRole("textbox",{name:"Rename"})).toBeNull();
});

it("deletes a whole session group after confirmation",async()=>{
  memoryTree();render(<KnowledgeNavigation/>);
  await expandMemoryEntry();
  fireEvent.contextMenu(await rowOf("Design session"));
  fireEvent.click(await screen.findByText("Delete"));
  const dialog=await screen.findByRole("alertdialog");
  expect(within(dialog).getByText("Delete all 1 knowledge entries in this group? The project or session itself is kept.")).toBeTruthy();
  fireEvent.click(within(dialog).getByRole("button",{name:"OK"}));
  await waitFor(()=>expect(api.memoryGroupDelete).toHaveBeenCalledWith("session","s1"));
});

it("switches the note list between cards and a list and remembers the choice",async()=>{
  localStorage.removeItem("vlx-notes-layout");
  window.history.replaceState(null,"","/?memory=notebook/vault-1&memoryFolder=Projects");
  const first=render(<MemoryRoute/>);
  const layout=await screen.findByRole("combobox",{name:"Layout"});
  expect(layout.textContent).toContain("Cards");
  expect(document.querySelector(".kbx-note-cards")).toBeTruthy();
  fireEvent.click(layout);
  fireEvent.click(screen.getByRole("option",{name:"List"}));
  await waitFor(()=>expect(document.querySelector(".kbx-note-list")).toBeTruthy());
  expect(document.querySelector(".kbx-note-cards")).toBeNull();
  expect(new URLSearchParams(location.search).get("memoryLayout")).toBe("list");
  expect(new URLSearchParams(location.search).get("memoryFolder")).toBe("Projects");
  first.unmount();
  window.history.replaceState(null,"","/?memory=notebook/vault-1");
  render(<MemoryRoute/>);
  await waitFor(()=>expect(document.querySelector(".kbx-note-list")).toBeTruthy());
  localStorage.removeItem("vlx-notes-layout");
});
