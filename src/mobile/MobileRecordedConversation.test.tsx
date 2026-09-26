import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Session } from "../types";
import type { ChatRow } from "../ipc/chat";
const rpc = vi.hoisted(() => ({readAgentChat:vi.fn(),ptyWrite:vi.fn(),shellRow:vi.fn()}));
vi.mock("../ipc/commands", () => rpc);
vi.mock("../i18n", () => ({useT:()=>(key:string)=>key}));
vi.mock("../layout/sessionViewers/TranscriptViewer", () => ({assistantLabel:()=>"Agent"}));
vi.mock("../layout/CenterPane/session/rows",()=>({MessageBubble:({text}:{text:string})=><p>{text}</p>,ReasoningRow:()=>null,ToolCard:()=>null,
 ShellRow:(props:{row:Extract<ChatRow,{kind:"shell"}>;onCancel?:(id:string)=>void})=>{rpc.shellRow(props);return <p>{props.row.command}</p>},
}));
import {MobileRecordedConversation} from "./MobileRecordedConversation";
const session={id:"remote",kind:"codex",engine:"tui"} as Session;
afterEach(()=>{cleanup();vi.resetAllMocks();vi.useRealTimers()});
describe("手机读取终端会话",()=>{
 it("uses a read-only empty state for Kiro without prompting for input",async()=>{
  rpc.readAgentChat.mockResolvedValue([]);
  render(<MobileRecordedConversation session={{...session,kind:"kiro"}}/>);
  expect(await screen.findByText("archive.emptyTranscript")).toBeTruthy();
  expect(screen.queryByText("chat.empty")).toBeNull();
  expect(screen.queryByRole("textbox")).toBeNull();
  expect(rpc.ptyWrite).not.toHaveBeenCalled();
 });
 it("passes backend shell history to the shared row without a cancellation callback",async()=>{
  const shell:Extract<ChatRow,{kind:"shell"}>={kind:"shell",id:"h-0",command:"printf recorded",stdout:"recorded",stderr:"",stdoutTruncated:false,stderrTruncated:false,status:"running"};
  rpc.readAgentChat.mockResolvedValue([{index:0,kind:"user",text:"Raw shell context",shell}]);
  render(<MobileRecordedConversation session={session}/>);
  expect(await screen.findByText("printf recorded")).toBeTruthy();
  expect(screen.queryByText("Raw shell context")).toBeNull();
  expect(rpc.shellRow).toHaveBeenCalledWith({row:shell});
  expect(rpc.ptyWrite).not.toHaveBeenCalled();
 });
 it("leaves command-like user text unchanged when the backend provides no shell row",async()=>{
  rpc.readAgentChat.mockResolvedValue([{index:0,kind:"user",text:"!printf ordinary message"}]);
  render(<MobileRecordedConversation session={session}/>);
  expect(await screen.findByText("!printf ordinary message")).toBeTruthy();
  expect(rpc.shellRow).not.toHaveBeenCalled();
  expect(rpc.ptyWrite).not.toHaveBeenCalled();
 });
 it("keeps Kiro history read-only without a PTY submission control",async()=>{
  rpc.readAgentChat.mockResolvedValue([{index:0,kind:"assistant",text:"Recorded Kiro answer"}]);
  render(<MobileRecordedConversation session={{...session,kind:"kiro"}}/>);
  expect(await screen.findByText("Recorded Kiro answer")).toBeTruthy();
  expect(rpc.readAgentChat).toHaveBeenCalledWith("remote");
  expect(screen.queryByRole("textbox")).toBeNull();
  expect(screen.queryByRole("button",{name:"session.send"})).toBeNull();
  expect(rpc.ptyWrite).not.toHaveBeenCalled();
 });
 it("加载完成后显示记录，发送到原 PTY",async()=>{
  rpc.readAgentChat.mockResolvedValue([{index:0,kind:"assistant",text:"远端继续运行"}]);rpc.ptyWrite.mockResolvedValue(undefined);
  render(<MobileRecordedConversation session={session}/>);
  expect(await screen.findByText("远端继续运行")).toBeTruthy();
  expect(screen.queryByRole("button",{name:"mobile.inputOptions"})).toBeNull();
  fireEvent.change(screen.getByRole("textbox"),{target:{value:"第一行\n第二行"}});
  fireEvent.click(screen.getByRole("button",{name:"session.send"}));
  await waitFor(()=>expect(rpc.ptyWrite).toHaveBeenCalledTimes(2));
  expect(rpc.ptyWrite.mock.calls).toEqual([["remote","\x1b[200~第一行\n第二行\x1b[201~"],["remote","\r"]]);
  await waitFor(()=>expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(""));
 });
 it("发送失败保留输入，不自动重发",async()=>{
  rpc.readAgentChat.mockResolvedValue([]);rpc.ptyWrite.mockRejectedValue(new Error("offline"));
  render(<MobileRecordedConversation session={session}/>);await screen.findByText("chat.empty");
  fireEvent.change(screen.getByRole("textbox"),{target:{value:"保留输入"}});
  fireEvent.click(screen.getByRole("button",{name:"session.send"}));
  expect(await screen.findByRole("alert")).toBeTruthy();expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("保留输入");
  expect(rpc.ptyWrite).toHaveBeenCalledTimes(1);
 });
 it("卸载后不再安排刷新",async()=>{
  vi.useFakeTimers();let resolve!:(value:[])=>void;rpc.readAgentChat.mockReturnValue(new Promise(r=>{resolve=r}));
  const view=render(<MobileRecordedConversation session={session}/>);view.unmount();resolve([]);
  await vi.advanceTimersByTimeAsync(5000);expect(rpc.readAgentChat).toHaveBeenCalledTimes(1);
 });
});
