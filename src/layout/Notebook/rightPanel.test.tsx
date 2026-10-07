// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { loadSettings, SETTINGS_KEY } from "../../store/settings";
import { RightPanel } from "../RightPanel/RightPanel";

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

it("keeps Info as the default and exposes four labeled icons",()=>{
  localStorage.removeItem(SETTINGS_KEY);expect(loadSettings().inspectorTab).toBe("info");render(<RightPanel/>);
  for(const name of ["Files","Info","Git","Knowledge Base"]){const link=screen.getByRole("link",{name});expect(link.textContent).toBe("");expect(link.getAttribute("title")).toBe(name);}
  expect(screen.getByRole("link",{name:"Info"}).getAttribute("aria-current")).toBe("page");
});

it("selects a tab whose URL is already current, as a mirrored layout leaves it",()=>{
  // A peer's published layout writes inspectorTab without touching the URL, so the address can already
  // name the tab the panel is not showing. The click must still switch the panel.
  window.history.replaceState(null,"","/?inspector=files");
  render(<RightPanel/>);
  // A peer's frame lands here: the store changes while the address bar still names the old tab.
  useTermStore.setState({inspectorTab:"git"});
  fireEvent.click(screen.getByRole("link",{name:"Files"}));
  expect(useTermStore.getState().inspectorTab).toBe("files");
  expect(document.querySelector(".col-right .insp-body")?.textContent).toBe("File explorer");
  expect(window.location.href).toBe(`${location.origin}/?inspector=files`);
});

it("opens knowledge through its right tab URL and keeps the directory in that panel",async()=>{
  render(<RightPanel/>);fireEvent.click(screen.getByRole("link",{name:"Knowledge Base"}));
  await screen.findByRole("link",{name:"Personal"});expect(new URLSearchParams(location.search).get("inspector")).toBe("knowledge");
  expect(new URLSearchParams(location.search).get("memory")).toBe("notebooks");
  expect(screen.getByRole("link",{name:"Session Knowledge Base"})).toBeTruthy();
  expect(screen.getByRole("button",{name:"Open knowledge base"})).toBeTruthy();
  fireEvent.click(screen.getByRole("link",{name:"Personal"}));
  await screen.findByRole("button",{name:"Hello"});expect(document.querySelector(".col-right .nb-file-tree")).not.toBeNull();
  fireEvent.click(screen.getByRole("link",{name:"Info"}));
  await waitFor(()=>expect(useTermStore.getState().inspectorTab).toBe("info"));
  expect(new URLSearchParams(location.search).get("inspector")).toBe("info");
  expect(new URLSearchParams(location.search).get("memory")).toBe("notebook/local");
});
