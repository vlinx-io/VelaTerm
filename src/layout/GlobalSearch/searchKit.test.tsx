// @vitest-environment jsdom
import {act,cleanup,render,renderHook} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {ContentSearchControls,useContentSearch,type ContentSearch} from "./searchKit";
import type {ContentSearchPage} from "../../ipc/commands";
const api=vi.hoisted(()=>({start:vi.fn(),page:vi.fn(),cancel:vi.fn()}));
vi.mock("../../ipc/commands",()=>({startContentSearch:api.start,contentSearchPage:api.page,cancelContentSearch:api.cancel,readAgentTranscript:vi.fn()}));
vi.mock("../sessionViewers/SessionContentViewer",()=>({SessionContentViewer:()=>null}));
const ready=(jobId:string,name:string):ContentSearchPage=>({jobId,status:"complete",scannedSessions:12,scannedBytes:100,
  totalSessions:12,totalMatches:72,nextCursor:10,previousCursor:null,
  hits:[{sessionId:name,name,kind:"claude",archived:false,source:"transcript",matchCount:6,matches:[]} ]});
beforeEach(()=>{vi.useFakeTimers();api.start.mockReset();api.page.mockReset();api.cancel.mockReset().mockResolvedValue(undefined);});
afterEach(()=>{cleanup();vi.useRealTimers();});
async function advance(ms:number) {await act(async()=>{await vi.advanceTimersByTimeAsync(ms);});}

it("cancels a start that finishes after its query has already been replaced",async()=>{
  let resolve!: (page:ContentSearchPage)=>void;
  api.start.mockReturnValueOnce(new Promise<ContentSearchPage>(done=>{resolve=done;})).mockResolvedValueOnce({jobId:"new",status:"pending",scannedSessions:0,scannedBytes:0});
  api.page.mockResolvedValue(ready("new","current"));
  const {result}=renderHook(()=>useContentSearch("live"));
  act(()=>result.current.setQuery("old"));await advance(250);
  act(()=>result.current.setQuery("new"));await advance(250);
  await act(async()=>{resolve({jobId:"old",status:"pending",scannedSessions:0,scannedBytes:0});});
  expect(api.cancel).toHaveBeenCalledWith("old");expect(api.page).not.toHaveBeenCalledWith("old");
  expect(result.current.results[0].name).toBe("current");expect(result.current.loading).toBe(false);
});

it("polls progress and replaces each page instead of accumulating the corpus",async()=>{
  api.start.mockResolvedValue({jobId:"one",status:"pending",scannedSessions:0,scannedBytes:0});
  api.page.mockResolvedValueOnce({jobId:"one",status:"pending",scannedSessions:4,scannedBytes:2048})
    .mockResolvedValueOnce(ready("one","first")).mockResolvedValueOnce({...ready("one","second"),nextCursor:null,previousCursor:11})
    .mockResolvedValueOnce(ready("one","first"));
  const {result}=renderHook(()=>useContentSearch("all"));act(()=>result.current.setQuery("needle"));await advance(250);
  expect(result.current.page?.scannedSessions).toBe(4);expect(result.current.loading).toBe(true);await advance(250);
  expect(result.current.page?.totalMatches).toBe(72);expect(result.current.results).toHaveLength(1);
  await act(async()=>result.current.next());expect(api.page).toHaveBeenLastCalledWith("one",10,undefined);
  expect(result.current.results.map(hit=>hit.name)).toEqual(["second"]);
  await act(async()=>result.current.previous());expect(api.page).toHaveBeenLastCalledWith("one",undefined,11);
  expect(result.current.results.map(hit=>hit.name)).toEqual(["first"]);expect(api.start).toHaveBeenCalledTimes(1);
});

it("clearing a query cancels its pending read and ignores a late page",async()=>{
  let resolve!: (page:ContentSearchPage)=>void;api.start.mockResolvedValue({jobId:"one",status:"pending",scannedSessions:0,scannedBytes:0});
  api.page.mockReturnValue(new Promise<ContentSearchPage>(done=>{resolve=done;}));
  const {result}=renderHook(()=>useContentSearch("live"));act(()=>result.current.setQuery("needle"));await advance(250);
  act(()=>result.current.setQuery(""));expect(api.cancel).toHaveBeenCalledWith("one");
  await act(async()=>resolve(ready("one","stale")));expect(result.current.results).toEqual([]);expect(result.current.loading).toBe(false);expect(result.current.page).toBeNull();
});

it("retains a visible failure and retries through a fresh owned task",async()=>{
  api.start.mockResolvedValueOnce({jobId:"failed",status:"pending",scannedSessions:0,scannedBytes:0}).mockResolvedValueOnce({jobId:"retry",status:"pending",scannedSessions:0,scannedBytes:0});
  api.page.mockRejectedValueOnce(new Error("source_changed: original changed")).mockResolvedValueOnce(ready("retry","fresh"));
  const {result}=renderHook(()=>useContentSearch("live"));act(()=>result.current.setQuery("needle"));await advance(250);
  expect(result.current.error).toContain("source_changed:");expect(result.current.loading).toBe(false);
  act(()=>result.current.retry());await advance(250);expect(api.cancel).toHaveBeenCalledWith("failed");expect(result.current.error).toBeNull();expect(result.current.results[0].name).toBe("fresh");
});

it("says when sessions were still being indexed so a short result is not mistaken for the whole",()=>{
  const search=(pendingSessions?:number):ContentSearch=>({query:"needle",setQuery:()=>{},results:[],loading:false,error:null,
    page:{...ready("one","a"),pendingSessions},next:()=>{},previous:()=>{},retry:()=>{},cancel:()=>{}});
  const {getByRole,rerender}=render(<ContentSearchControls search={search(3)} />);
  expect(getByRole("status").textContent).toBe("72 matches · 12 sessions · 3 sessions are still being indexed, so results may be incomplete.");
  rerender(<ContentSearchControls search={search(0)} />);expect(getByRole("status").textContent).toBe("72 matches · 12 sessions");
});

it("forwards project/collection filter changes to startContentSearch and cancels the prior job",async()=>{
  api.start.mockResolvedValueOnce({jobId:"all",status:"pending",scannedSessions:0,scannedBytes:0})
    .mockResolvedValueOnce({jobId:"col",status:"pending",scannedSessions:0,scannedBytes:0});
  api.page.mockResolvedValueOnce(ready("all","first")).mockResolvedValueOnce(ready("col","filtered"));
  const {result,rerender}=renderHook(({projectId}:{projectId?:string})=>useContentSearch("live",undefined,projectId),{initialProps:{projectId:undefined as string|undefined}});
  act(()=>result.current.setQuery("needle"));await advance(250);
  expect(api.start).toHaveBeenLastCalledWith("needle","live",undefined);
  expect(result.current.results[0].name).toBe("first");
  rerender({projectId:"col-work"});await advance(250);
  expect(api.cancel).toHaveBeenCalledWith("all");
  expect(api.start).toHaveBeenLastCalledWith("needle","live","col-work");
  expect(result.current.results[0].name).toBe("filtered");
});
