// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { loadSettings, SETTINGS_KEY } from "../../store/settings";
import { RightPanel } from "../RightPanel/RightPanel";
import { memoryNavigate, memoryUrl } from "../Memory/navigation";

vi.mock("../RightPanel/FilesTab",()=>({FilesTab:()=> <div>File explorer</div>}));
vi.mock("../RightPanel/InfoTab",()=>({InfoTab:()=> <div>Session info</div>}));
vi.mock("../RightPanel/git/GitTab",()=>({GitTab:()=> <div>Git changes</div>}));
vi.mock("../../ipc/notebook",()=>({notebookOverview:async()=>({vaults:[{id:"local",name:"Personal",root:"/notes"}],limits:{},defaultRoot:"/Notes"}),notebookTree:async()=>({vault:{id:"local",name:"Personal",root:"/notes"},nodes:[{kind:"note",path:"Hello.md",absolutePath:"/notes/Hello.md",name:"Hello.md"}],entries:[],trash:[]})}));
const original=useTermStore.getState().setInspectorTab;
beforeEach(()=>{
  setLang("en");window.history.replaceState(null,"","/");
  useTermStore.setState({inspectorTab:"info",setInspectorTab:tab=>useTermStore.setState({inspectorTab:tab})});
});
afterEach(()=>{cleanup();useTermStore.setState({setInspectorTab:original});});

it("keeps Info as the default and exposes three labeled icons without a knowledge tab",()=>{
  localStorage.removeItem(SETTINGS_KEY);expect(loadSettings().inspectorTab).toBe("info");render(<RightPanel/>);
  for(const name of ["Files","Info","Git"]){const link=screen.getByRole("link",{name});expect(link.textContent).toBe("");expect(link.getAttribute("title")).toBe(name);}
  expect(screen.queryByRole("link",{name:"Knowledge Base"})).toBeNull();
  expect(document.querySelectorAll(".insp-tab")).toHaveLength(3);
  expect(screen.getByRole("link",{name:"Info"}).getAttribute("aria-current")).toBe("page");
});

it("opens Info for a saved knowledge tab from an older version",()=>{
  localStorage.setItem(SETTINGS_KEY,JSON.stringify({inspectorTab:"knowledge"}));
  try { expect(loadSettings().inspectorTab).toBe("info"); } finally { localStorage.removeItem(SETTINGS_KEY); }
});

it("ignores an old knowledge inspector URL",()=>{
  window.history.replaceState(null,"","/?inspector=knowledge");render(<RightPanel/>);
  expect(useTermStore.getState().inspectorTab).toBe("info");
  expect(screen.getByRole("link",{name:"Info"}).getAttribute("aria-current")).toBe("page");
});

it("lets a tab be reselected after settings sync switched away from the URL's tab",async()=>{
  window.history.replaceState(null,"","/?inspector=info");render(<RightPanel/>);
  act(()=>useTermStore.setState({inspectorTab:"git"}));
  await waitFor(()=>expect(new URLSearchParams(location.search).get("inspector")).toBe("git"));
  fireEvent.click(screen.getByRole("link",{name:"Info"}));
  await waitFor(()=>expect(useTermStore.getState().inspectorTab).toBe("info"));
});

it("keeps an externally selected tab when an unrelated navigation follows",()=>{
  render(<RightPanel/>);
  act(()=>useTermStore.setState({inspectorTab:"git"}));
  expect(new URLSearchParams(location.search).get("inspector")).toBe("git");
  act(()=>memoryNavigate(memoryUrl("notebooks")));
  expect(useTermStore.getState().inspectorTab).toBe("git");
});

it("switches tabs while an unsaved-draft guard blocks page navigation",()=>{
  window.history.replaceState(null,"","/?inspector=info&memory=entry%2Fm1");render(<RightPanel/>);
  const guard=(event:Event)=>event.preventDefault();window.addEventListener("memory:beforeNavigate",guard);
  try {
    fireEvent.click(screen.getByRole("link",{name:"Git"}));
    expect(useTermStore.getState().inspectorTab).toBe("git");
    expect(new URLSearchParams(location.search).get("memory")).toBe("entry/m1");
  } finally { window.removeEventListener("memory:beforeNavigate",guard); }
});
