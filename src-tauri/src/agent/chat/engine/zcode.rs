//! Zcode 原生 CLI NDJSON；进程不连接桌面 GUI 或 mailbox。
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, atomic::Ordering};
use serde_json::{json, Value};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use super::{ChatProcess, ChatRow, emit, handle_turn_end, now_ms, start_waiting_message};
mod projection;
use crate::{host::AppCtx, models::SessionKind, pty::AgentState};

#[derive(Default)]
pub(super) struct State {
    projection: projection::State,
    last_seq: Option<u64>,
    current_input: Option<String>,
    usage_request: Option<String>,
    completed: HashSet<String>,
    completed_turns: HashSet<String>,
    pub models: Vec<Value>,
    model_choices: HashMap<String,Value>,
    skill_commands: Vec<Value>,
    pub(super) slash_commands: Vec<Value>,
    pub auth_child: Option<Arc<std::sync::Mutex<std::process::Child>>>,
}

pub(super) fn native_id(proc: &ChatProcess) -> Result<String, String> {
    proc.agent_session_id.lock().unwrap().clone().ok_or("Zcode 原生会话尚未就绪".into())
}

pub(super) fn command(proc: &ChatProcess, kind: &'static str, method: &str, mut params: Value) -> Result<Value, String> {
    params["sessionId"] = json!(native_id(proc)?);
    proc.request_and_wait(kind, |id| json!({"id":id,"method":method,"params":params}))
}

pub(super) fn set_model(proc: &ChatProcess, model: Option<&str>) -> Result<(), String> {
    let model = model.ok_or("请从 Zcode 原生模型列表选择具体模型")?;
    let choice=proc.zcode.lock().unwrap().model_choices.get(model).cloned().ok_or("Zcode 原生目录中没有此模型")?;
    let selection=complete_selection(&choice,proc.effort.lock().unwrap().as_deref());
    let response=command(proc, "set_model", "session/setModel", json!({"model":selection,"persistAsWorkspaceLastUsed":false}))?;
    if let Some(settings)=response.get("settings") { *proc.effort.lock().unwrap()=settings["thoughtLevel"]["current"].as_str().map(str::to_string); }
    Ok(())
}

fn complete_selection(choice: &Value, effort: Option<&str>) -> Value {
    let mut selection=choice["ref"].clone();
    let reasoning=&choice["reasoning"];
    let level=effort.filter(|value|reasoning["levels"].as_array().is_some_and(|levels|levels.iter().any(|v|v["value"].as_str()==Some(*value))))
        .or_else(||reasoning["defaultLevel"].as_str());
    if let Some(level)=level {selection["options"]=json!({"reasoningLevel":level});}
    selection
}

pub(super) fn set_effort(proc: &ChatProcess, effort: Option<&str>) -> Result<(),String> {
    let model=proc.model.lock().unwrap().clone().ok_or("Zcode 尚未选择模型")?;
    let choice=proc.zcode.lock().unwrap().model_choices.get(&model).cloned().ok_or("Zcode 模型目录尚未就绪")?;
    let selected=effort.or_else(||choice["reasoning"]["defaultLevel"].as_str()).ok_or("该 Zcode 模型没有默认思考级别")?;
    let response=command(proc,"set_effort","session/setThoughtLevel",json!({"thoughtLevel":selected,"persistAsWorkspaceLastUsed":false}))?;
    *proc.effort.lock().unwrap()=response["settings"]["thoughtLevel"]["current"].as_str().or(Some(selected)).map(str::to_string);
    Ok(())
}

pub(super) fn bootstrap(app: AppCtx, session_id: String, proc: Arc<ChatProcess>, resume: Option<String>) {
    std::thread::spawn(move || {
        let run = || -> Result<(), String> {
            let cwd = proc.cwd.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned());
            let workspace = json!({"workspacePath":cwd,"workspaceKey":cwd});
            let method = if resume.is_some() { "session/resume" } else { "session/create" };
            let params = if let Some(native) = resume { json!({"sessionId":native,"workspace":workspace}) }
                else { json!({"workspace":workspace,"titleGenerationEnabled":false}) };
            let snapshot = proc.request_and_wait("zcode_open", |id| json!({"id":id,"method":method,"params":params}))?;
            let native = snapshot["session"]["sessionId"].as_str().ok_or("Zcode 未返回原生会话 ID")?;
            if !super::verify_native_identity(&app, &session_id, &proc, native) { return Err("Zcode 原生会话身份不匹配".into()); }
            *proc.agent_session_id.lock().unwrap() = Some(native.into());
            let changed = crate::db::repo::set_agent_session_id(&app.db().conn.lock().unwrap(), &session_id, native, SessionKind::Zcode)?;
            if changed { app.emit(crate::host::TREE_CHANGED, ()); }
            // 与运行中的设置修改串行，启动时捕获的默认值不能覆盖刚确认的新选择。
            let settings_change = proc.settings_change.lock().unwrap();
            let selected_model = proc.model.lock().unwrap().clone();
            let selected_effort = proc.effort.lock().unwrap().clone();
            let selected_mode = proc.mode.lock().unwrap().clone();
            apply_snapshot(&app, &session_id, &proc, &snapshot, true);
            let subscription = command(&proc, "zcode_subscribe", "session/subscribe", json!({"deliveryKind":"desktop-continuous","includeSnapshot":false}))?;
            proc.zcode.lock().unwrap().last_seq = subscription["eventSeq"].as_u64();
            if let Some(model) = selected_model { set_model(&proc, Some(&model))?; *proc.model.lock().unwrap() = Some(model); }
            if let Some(effort) = selected_effort {
                set_effort(&proc,Some(&effort))?;
            }
            command(&proc, "set_permission_mode", "session/setMode", json!({"mode":selected_mode}))?;
            *proc.mode.lock().unwrap() = selected_mode;
            proc.permission_confirmed.store(true, Ordering::Relaxed);
            emit(&app, &session_id, json!({"type":"session","agentSessionId":native,"model":proc.model.lock().unwrap().clone()}));
            proc.ready.store(true, Ordering::Relaxed);
            drop(settings_change);
            start_waiting_message(&app, &session_id, &proc);
            if let Err(error)=projection::subscribe(&proc,native) {
                emit(&app,&session_id,json!({"type":"error","message":format!("Zcode 用量/子任务投影订阅失败：{error}")}));
            }
            // 先完成预选设置并放行会话，再读取会初始化 context 的技能目录。
            // 慢发现不能让启动时捕获的旧模式覆盖用户刚确认的设置。
            if let Ok(commands) = refresh_commands(&proc) {
                emit(&app,&session_id,json!({"type":"commands","commands":commands}));
            }
            Ok(())
        };
        if let Err(error) = run() {
            emit(&app, &session_id, json!({"type":"error","message":format!("Zcode CLI 初始化失败：{error}")}));
            super::release_process(&app, &session_id, &proc);
        }
    });
}

fn apply_snapshot(app: &AppCtx, session: &str, proc: &Arc<ChatProcess>, snapshot: &Value, history: bool) {
    if history { apply_persisted_usage(app,session,proc,&snapshot["messages"]); }
    let settings = &snapshot["settings"];
    let current = &settings["model"]["current"];
    if let (Some(provider), Some(model)) = (current["providerId"].as_str(), current["modelId"].as_str()) {
        *proc.model.lock().unwrap() = Some(format!("{provider}/{model}"));
    }
    apply_models(proc, snapshot);
    if let Some(mode) = settings["mode"]["current"].as_str() { *proc.mode.lock().unwrap() = mode.into(); }
    *proc.effort.lock().unwrap() = settings["thoughtLevel"]["current"].as_str().map(str::to_string);
    let commands = {
        let mut state=proc.zcode.lock().unwrap();
        state.slash_commands=snapshot["slashCommands"].as_array().into_iter().flatten().filter(|v|v["name"].as_str().is_some()).map(|v|json!({"name":v["name"],"description":v["description"].as_str().unwrap_or_default(),"invocation":"/"})).collect();
        combined_commands(&state)
    };
    *proc.commands.lock().unwrap() = commands.clone();
    emit(app,session,json!({"type":"commands","commands":commands}));
    if history {
        for message in snapshot["messages"].as_array().into_iter().flatten() {
            let info = &message["info"];
            if info["role"] == "user" {
                let text = message["parts"].as_array().into_iter().flatten().filter(|p|p["type"]=="text").filter_map(|p|p["text"].as_str()).collect::<String>();
                let images=message["parts"].as_array().into_iter().flatten().filter_map(|part| {
                    if part["type"]!="file" { return None; }
                    let (mime,data)=part["url"].as_str()?.strip_prefix("data:")?.split_once(";base64,")?;
                    if !mime.starts_with("image/") || data.is_empty() { return None; }
                    Some(super::ChatImage{mime_type:mime.into(),data:data.into()})
                }).collect();
                if let Some(id) = info["messageId"].as_str() { proc.timeline.lock().unwrap().upsert(ChatRow::User{id:id.into(),text,images,at:info["time"]["created"].as_i64()}); }
            } else {
                for part in message["parts"].as_array().into_iter().flatten() { apply_part(proc, part, false); }
            }
        }
    }
}

// 使用原生持久化 assistant token 记录恢复原始累计；V4 的冷投影累计可能从零开始。
fn apply_persisted_usage(app: &AppCtx, session: &str, proc: &Arc<ChatProcess>, messages: &Value) {
    let mut input=0u64;let mut output=0u64;let mut cached=0u64;let mut written=0u64;let mut reasoning=0u64;
    let mut seen=HashSet::new();
    for message in messages.as_array().into_iter().flatten() {
        let info=&message["info"];
        if info["role"]!="assistant" {continue;}
        if let Some(id)=info["messageId"].as_str().or_else(||info["id"].as_str()) {if !seen.insert(id) {continue;}}
        let tokens=&info["tokens"];
        input=input.saturating_add(tokens["input"].as_u64().unwrap_or(0));
        output=output.saturating_add(tokens["output"].as_u64().unwrap_or(0));
        cached=cached.saturating_add(tokens["cache"]["read"].as_u64().unwrap_or(0));
        written=written.saturating_add(tokens["cache"]["write"].as_u64().unwrap_or(0));
        reasoning=reasoning.saturating_add(tokens["reasoning"].as_u64().unwrap_or(0));
    }
    proc.extras.lock().unwrap().native_usage=Some(json!({"scope":"session","inputTokens":input,"outputTokens":output,"cachedInputTokens":cached,"cacheWriteTokens":written,"reasoningOutputTokens":reasoning}));
    super::emit_extras(app,session,proc);
}

fn request_usage(proc: &ChatProcess) {
    let id=proc.request_id("zcode_usage_read");
    proc.zcode.lock().unwrap().usage_request=Some(id.clone());
    let _=proc.write(&json!({"id":id,"method":"session/messages","params":{"sessionId":native_id(proc).unwrap_or_default()}}));
}

fn workspace(proc: &ChatProcess) -> Value {
    let cwd=proc.cwd.clone().unwrap_or_else(||std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned());
    json!({"workspacePath":cwd,"workspaceKey":cwd})
}

pub(super) fn refresh_commands(proc: &ChatProcess) -> Result<Vec<Value>,String> {
    let result=command(proc,"zcode_skills","skills/referenceCatalog",json!({"workspace":workspace(proc)}))?;
    let skills=crate::agent::zcode_cli::skill_commands(&result)?;
    let mut state=proc.zcode.lock().unwrap();
    state.skill_commands=skills;
    let commands=combined_commands(&state);
    *proc.commands.lock().unwrap()=commands.clone();
    Ok(commands)
}

fn combined_commands(state: &State) -> Vec<Value> {
    let mut names=HashSet::new();
    state.slash_commands.iter().chain(&state.skill_commands).filter(|command|names.insert(command["name"].as_str().unwrap_or_default())).cloned().collect()
}

fn apply_models(proc: &Arc<ChatProcess>, snapshot: &Value) {
    let mut state = proc.zcode.lock().unwrap();
    state.models = crate::agent::zcode_cli::model_entries(snapshot);
    state.model_choices.clear();
    for choice in snapshot["settings"]["model"]["available"].as_array().into_iter().flatten() {
        if let (Some(provider),Some(model))=(choice["ref"]["providerId"].as_str(),choice["ref"]["modelId"].as_str()) {
            state.model_choices.insert(format!("{provider}/{model}"),choice.clone());
        }
    }
}

pub(super) fn start_plan(proc: &Arc<ChatProcess>) -> Result<Value, String> {
    let result = command(proc, "zcode_start_plan", "provider/readStartPlan", json!({}))?;
    if result.get("settings").is_some() { apply_models(proc, &result); }
    Ok(json!({"plans":result["plans"]}))
}

pub(super) fn dispatch(proc: &Arc<ChatProcess>, input: &str, text: &str, images: &[super::ChatImage]) -> Result<(), String> {
    let native = native_id(proc)?;
    proc.zcode.lock().unwrap().current_input = Some(input.into());
    let id = proc.request_id("zcode_send");
    let attachments=images.iter().enumerate().map(|(index,image)|json!({"kind":"image","filename":format!("image-{}.{}",index+1,image.mime_type.split('/').nth(1).unwrap_or("png")),"mimeType":image.mime_type,"dataBase64":image.data})).collect::<Vec<_>>();
    proc.write(&json!({"id":id,"method":"session/send","params":{"sessionId":native,"inputId":input,"content":text,"attachments":attachments},"trace":{"traceId":input}}))
}

/// 使用原生 guide 在当前 turn 的安全检查点注入，不能再发一次 session/send。
pub(super) fn steer(app: &AppCtx, session: &str, proc: &ChatProcess, input: &str, text: &str, images: &[super::ChatImage]) -> Result<(),String> {
    let turn=proc.current_turn.lock().unwrap().clone().ok_or("Zcode 尚未开始回合，请稍后追加指令")?;
    if !images.is_empty() {return Err("Zcode 当前不支持在即时追加中携带图片，请使用排队发送；图片没有发送".into());}
    if !proc.permissions.lock().unwrap().is_empty() {return Err("请先处理当前的确认请求，再即时追加指令".into());}
    emit(app,session,json!({"type":"steerPending"}));
    let result=command(proc,"zcode_steer","v4/command",json!({"commandId":format!("velaterm-guide:{input}"),"clientId":"velaterm","type":"sendText","issuedAt":now_ms(),"payload":{"text":text,"requestedDelivery":"guide"}}))?;
    match result["status"].as_str() {
        Some("accepted"|"duplicate")=>{
            if proc.current_turn.lock().unwrap().as_deref()!=Some(turn.as_str()) {
                emit(app,session,json!({"type":"steerPending","turnEnded":true}));
            }
            Ok(())
        },
        _=>Err(result["message"].as_str().or_else(||result["reasonCode"].as_str()).unwrap_or("Zcode 拒绝了追加指令").into()),
    }
}

fn text_row(proc: &ChatProcess, id: &str, text: String, reasoning: bool, streaming: bool) {
    let row = if reasoning { ChatRow::Reasoning{id:id.into(),text,streaming} }
        else { ChatRow::Assistant{id:id.into(),text,streaming,model:proc.model.lock().unwrap().clone(),at:Some(now_ms() as i64),duration_ms:None} };
    proc.timeline.lock().unwrap().upsert(row);
}

fn apply_part(proc: &ChatProcess, part: &Value, streaming: bool) {
    let Some(id) = part["partId"].as_str() else { return; };
    match part["type"].as_str() {
        Some("text" | "reasoning") => {
            let base=part["messageId"].as_str().unwrap_or(id);
            let row_id=if part["type"]=="reasoning" {format!("{base}:reasoning")}else{base.into()};
            let text = part["text"].as_str().unwrap_or("").to_string();
            proc.buffers.lock().unwrap().insert(row_id.clone(),text.clone());
            text_row(proc,&row_id,text,part["type"]=="reasoning",streaming);
        }
        Some("tool") => {
            let state = &part["state"];
            let error = state["status"] == "error";
            let tool_id=part["callId"].as_str().unwrap_or(id);
            let mut timeline=proc.timeline.lock().unwrap();
            let (subagent,children,old_status)=match super::find_row(&timeline.rows,tool_id) {Some(ChatRow::Tool{subagent,children,status,..})=>(subagent.clone(),children.clone(),subagent.as_ref().map(|_|*status)),_=>(None,vec![],None)};
            timeline.upsert(ChatRow::Tool{id:part["callId"].as_str().unwrap_or(id).into(),name:part["tool"].as_str().unwrap_or("Tool").into(),input:state["input"].clone(),output:state[if error {"error"}else{"output"}].as_str().map(str::to_string),is_error:error,status:if error {"failed"}else{old_status.unwrap_or(if state["status"]=="completed" {"completed"}else{"running"})},subagent,children});
        }
        _ => {}
    }
}

fn apply_tool(proc: &ChatProcess, payload: &Value) {
    if payload["source"]=="subagent" {return;}
    let Some(id)=payload["toolCallId"].as_str() else {return};
    let mut timeline=proc.timeline.lock().unwrap();
    let previous=timeline.rows.iter().find(|row|row.id()==id).cloned();
    let (old_name,old_input,old_output,subagent,children,old_status)=match previous {
        Some(ChatRow::Tool{name,input,output,subagent,children,status,..})=>(name,input,output,subagent,children,status),
        _=>("Tool".into(),Value::Null,None,None,vec![],"running"),
    };
    let kind=payload["kind"].as_str().unwrap_or("");
    let error=kind=="error" || payload["result"]["isError"]==true;
    let output=if error {payload["error"]["message"].as_str().map(str::to_string)}
        else if kind=="result" {let result=&payload["result"]; result["content"].as_str().or_else(||result["output"].as_str()).map(str::to_string).or_else(||result["content"].as_array().map(|parts|parts.iter().filter_map(|p|p["text"].as_str()).collect::<String>()))}
        else {payload["stdoutTail"].as_str().map(str::to_string)};
    timeline.upsert(ChatRow::Tool{id:id.into(),name:payload["toolName"].as_str().map(str::to_string).unwrap_or(old_name),input:payload.get("input").cloned().unwrap_or(old_input),output:output.or(old_output),is_error:error,status:if error {"failed"}else if subagent.is_some(){old_status}else if kind=="result" {"completed"}else{"running"},subagent,children});
}

pub(super) fn respond(app: &AppCtx, session: &str, proc: &Arc<ChatProcess>, id: &str, allow: bool, input: Option<Value>, message: Option<&str>) -> Result<(), String> {
    // 与终态清理共用卡片锁；拒绝迟到/重复回答，并使用重发后最新的传输 ID。
    // 锁保持至 Working 发布，保证终态清理及 Waiting 不会被迟到回答反向覆盖。
    let mut permissions = proc.permissions.lock().unwrap();
    let payload = permissions.get(id).cloned().ok_or("That permission request is no longer waiting for an answer")?;
    if !proc.alive.load(Ordering::Relaxed) { return Err("Zcode 进程已停止，请重新打开会话".into()); }
    let result = if payload["zcodeMethod"] == "interaction/requestUserInput" {
        json!({"action":if allow{"accept"}else{"cancel"},"content":input.unwrap_or_else(||json!({})),"reason":message.unwrap_or("")})
    } else { json!({"decision":if allow{"allow"}else{"deny"},"reason":message.unwrap_or("")}) };
    proc.write(&json!({"id":payload["nativeRequestId"],"result":result}))?;
    permissions.remove(id);
    emit(app,session,json!({"type":"permissionResolved","id":id}));
    super::emit_state(app,session,AgentState::Working);
    Ok(())
}

pub(super) fn clear_interactions(app: &AppCtx, session: &str, proc: &ChatProcess) {
    let ids: Vec<_> = proc.permissions.lock().unwrap().drain().map(|(id, _)| id).collect();
    for id in ids { emit(app,session,json!({"type":"permissionResolved","id":id})); }
}

fn clear_foreground_interactions(app: &AppCtx, session: &str, proc: &ChatProcess) {
    let mut permissions=proc.permissions.lock().unwrap();
    let ids=permissions.iter().filter(|(_,value)|value["zcodeChildSessionId"].as_str().is_none()).map(|(id,_)|id.clone()).collect::<Vec<_>>();
    for id in &ids {permissions.remove(id);}
    drop(permissions);
    for id in ids {emit(app,session,json!({"type":"permissionResolved","id":id}));}
}

fn apply_mode(app: &AppCtx, session: &str, proc: &ChatProcess, settings: &Value) {
    let mode = if settings["planEnabled"] == true { Some("plan") }
        else { settings["mode"].as_str().or_else(||settings["mode"]["current"].as_str()) };
    if let Some(mode) = mode {
        *proc.mode.lock().unwrap() = mode.into();
        emit(app,session,json!({"type":"settingsChanged","mode":mode}));
    }
}

pub(super) fn handle_line(app: &AppCtx, session: &str, proc: &Arc<ChatProcess>, line: &str) {
    let Ok(frame) = serde_json::from_str::<Value>(line) else { return; };
    if frame.get("id").is_some() && frame.get("method").is_none() {
        let key = frame["id"].as_str().map(str::to_owned).unwrap_or_else(||frame["id"].to_string());
        let kind = proc.pending.lock().unwrap().remove(&key);
        let error = frame.get("error").map(|e|e["message"].as_str().unwrap_or("Zcode 协议错误").to_string());
        if kind==Some("zcode_usage_read") {
            if proc.zcode.lock().unwrap().usage_request.as_deref()==Some(&key) {
                if error.is_none() {apply_persisted_usage(app,session,proc,&frame["result"]["messages"]);}
            }
            return;
        }
        if let Some(waiter) = proc.waiters.lock().unwrap().remove(&key) {
            let _ = waiter.send(match error {Some(e)=>Err(e),None=>Ok(frame["result"].clone())});
        } else if let Some(error) = error {
            emit(app,session,json!({"type":"error","message":error}));
            if kind == Some("zcode_send") { handle_turn_end(app,session,proc,"zcode_failed",None); }
        }
        return;
    }
    let method = frame["method"].as_str().unwrap_or("");
    let params = &frame["params"];
    if frame.get("id").is_some() {
        if method == "session/requestRuntimePreferences" {
            let _ = proc.write(&json!({"id":frame["id"],"result":{"nativeSearchEnhancementsEnabled":true,"memoryEnabled":false,"askUserQuestionAutoResolutionEnabled":false}}));
        } else if matches!(method,"interaction/requestPermission"|"interaction/requestUserInput") {
            let native = proc.agent_session_id.lock().unwrap().clone();
            if native.as_deref() != params["sessionId"].as_str() {
                let _ = proc.write(&json!({"id":frame["id"],"error":{"code":-32602,"message":"会话不匹配"}})); return;
            }
            // 重发使用新的传输帧 ID；卡片身份必须跟随稳定的业务 requestId。
            let key = params["requestId"].as_str().map(str::to_owned)
                .unwrap_or_else(||frame["id"].as_str().map(str::to_owned).unwrap_or_else(||frame["id"].to_string()));
            let mut input = params["input"].clone();
            let name = if method=="interaction/requestUserInput" {
                input = json!({"questions":params["questions"].as_array().into_iter().flatten().map(|q|json!({"question":q["question"],"header":q["header"],"options":q["options"].as_array().into_iter().flatten().map(|o|json!({"label":o["label"],"description":o["description"],"value":o["value"]})).collect::<Vec<_>>(),"multiSelect":q["multiSelect"]==true})).collect::<Vec<_>>()});
                "AskUserQuestion"
            } else { params["toolName"].as_str().unwrap_or("Tool") };
            let payload = json!({"id":key,"tool_name":name,"input":input,"reason":params["reason"],"nativeRequestId":frame["id"],"zcodeMethod":method,"zcodeQuestions":params["questions"],"zcodeChildSessionId":params["origin"]["childSessionId"]});
            proc.permissions.lock().unwrap().insert(key,payload.clone());
            emit(app,session,json!({"type":"permission","request":payload}));
            super::emit_state(app,session,AgentState::Asking);
        } else {
            let _ = proc.write(&json!({"id":frame["id"],"error":{"code":-32601,"message":format!("VelaTerm 尚不支持 Zcode 宿主请求 {method}")}}));
        }
        return;
    }
    if method=="v4/conversation/frame" {projection::handle(app,session,proc,params);return;}
    if proc.agent_session_id.lock().unwrap().as_deref() != params["sessionId"].as_str() { return; }
    if method == "state.updated" {
        apply_mode(app,session,proc,&params["patch"]);
        return;
    }
    if method != "session/event" { return; }
    {
        let mut state = proc.zcode.lock().unwrap();
        if let Some(seq) = params["seq"].as_u64() {
            if state.last_seq.is_some_and(|old|seq<=old) { return; }
            state.last_seq = Some(seq);
        }
    }
    let payload = &params["payload"];
    match params["type"].as_str() {
        Some("turn.steerDrained") => {
            if payload["drainedInputs"].as_array().into_iter().flatten().any(|item|
                item["delivery"]=="guide" && item["intent"]["sourceCommandId"].as_str().is_some_and(|id|id.starts_with("velaterm-guide:"))) {
                emit(app,session,json!({"type":"steerAccepted"}));
            }
        }
        Some("model.streaming") => {
            // 原生 part 事件是持久化投影；同一流只消费 model.streaming，避免双计 delta。
            let kind = payload["kind"].as_str().unwrap_or("");
            // CLI 在流中交付完整参数后，会从 scheduled 事件省略 input；在同一卡片保留该输入。
            if kind == "tool_call" { apply_tool(proc, payload); }
            if matches!(kind,"text_delta"|"reasoning_delta") {
                let base = payload["partId"].as_str().or_else(||payload["assistantMessageId"].as_str()).unwrap_or("zcode-answer");
                let id = if kind=="reasoning_delta" {format!("{base}:reasoning")}else{base.into()};
                let mut buffers = proc.buffers.lock().unwrap();
                let text = buffers.entry(id.clone()).or_default(); text.push_str(payload["delta"].as_str().unwrap_or(""));
                let text = text.clone(); drop(buffers);
                text_row(proc,&id,text,kind=="reasoning_delta",true);
            }
        }
        Some("part.upserted") => apply_part(proc,&payload["part"],true),
        Some("tool.updated") => apply_tool(proc,payload),
        Some("turn.started") => {
            let turn = params["turnId"].as_str();
            if turn.is_some_and(|id| proc.zcode.lock().unwrap().completed_turns.contains(id)) { return; }
            *proc.current_turn.lock().unwrap()=turn.map(str::to_string);
            proc.extras.lock().unwrap().turn_usage=None;
            super::emit_extras(app,session,proc);
        }
        Some("session.updated") => {
            if let Some(snapshot) = payload.get("snapshot") { apply_snapshot(app,session,proc,snapshot,false); }
            // 工具驱动的模式变化只携带原生 mode/planEnabled，不附带 snapshot。
            apply_mode(app,session,proc,payload);

        }
        Some("permission.resolved") => {
            let id = payload["requestId"].as_str().or_else(||payload["toolCallId"].as_str());
            if let Some(id) = id {
                proc.permissions.lock().unwrap().remove(id);
                emit(app,session,json!({"type":"permissionResolved","id":id}));
            }
        }
        Some("turn.completed"|"turn.failed") => {
            let failed = params["type"]=="turn.failed";
            let turn = params["turnId"].as_str();
            let current_turn = proc.current_turn.lock().unwrap().clone();
            // inputId 在原生终态中可省略；先验证 turn 归属，不能把旧终态归给后来派发的输入。
            if turn.zip(current_turn.as_deref()).is_some_and(|(incoming, current)| incoming != current) { return; }
            let input = {
                let mut state = proc.zcode.lock().unwrap();
                if turn.is_some_and(|id| state.completed_turns.contains(id)) { return; }
                let input = payload["inputId"].as_str().map(str::to_string).or_else(||state.current_input.clone());
                if input.as_ref().zip(state.current_input.as_ref()).is_some_and(|(incoming, current)| incoming != current) { return; }
                if let Some(ref input) = input {
                    if !state.completed.insert(input.clone()) { return; }
                }
                if let Some(turn) = turn { state.completed_turns.insert(turn.to_string()); }
                state.current_input = None;
                input
            };
            *proc.current_turn.lock().unwrap() = None;
            clear_foreground_interactions(app,session,proc);
            let completed_rows=proc.timeline.lock().unwrap().rows.iter().filter_map(|row| {
                let mut row=row.clone();
                match &mut row {ChatRow::Assistant{streaming,..}|ChatRow::Reasoning{streaming,..} if *streaming=>{*streaming=false;Some(row)},_=>None}
            }).collect::<Vec<_>>();
            let had_stream=completed_rows.iter().any(|row|matches!(row,ChatRow::Assistant{..}));
            for row in completed_rows {proc.timeline.lock().unwrap().upsert(row);}
            if failed { emit(app,session,json!({"type":"error","message":payload["error"]["message"]})); }
            else {
                // 无流输出的原生结果仍然可见；已有流由终态统一结束。
                let response = payload["response"].as_str().unwrap_or("");
                if !response.is_empty() && !had_stream {
                    text_row(proc,&format!("zcode:{}",input.as_deref().unwrap_or("result")),response.into(),false,false);
                }
                if payload["resultType"]=="success" {
                    super::auth::turn_succeeded(app,session,proc);
                    if let Some(input) = input { report(app,session,&input,response); }
                }
            }
            let status = if !failed && matches!(payload["resultType"].as_str(),Some("success"|"cancelled")) {"success"}else{"zcode_failed"};
            if payload["usage"].is_object() { proc.extras.lock().unwrap().turn_usage=Some(payload["usage"].clone()); }
            handle_turn_end(app,session,proc,status,payload["duration"].as_u64());
            request_usage(proc);
        }
        _ => {}
    }
}

fn report(app: &AppCtx, session: &str, input: &str, text: &str) {
    let round = app.db().conn.lock().unwrap().query_row("SELECT m.round FROM plan_execute_messages m JOIN plan_execute_runs r ON r.id=m.run_id WHERE m.id=?1 AND m.target_id=?2 AND r.executor_id=?2 AND m.action='dispatch'",rusqlite::params![input,session],|row|row.get::<_,u32>(0)).optional();
    if let Ok(Some(round)) = round {
        let hash=Sha256::digest(format!("zcode-report:{session}:{input}"));
        let request=crate::agent::tell::Request{session_id:session.into(),target:None,report:true,steer:false,round:Some(round),message_id:format!("msg-{}",uuid::Uuid::from_bytes(hash[..16].try_into().unwrap())),text:text.into()};
        if let Err(error)=crate::agent::plan_execute::report(app,&request,None) { emit(app,session,json!({"type":"error","message":format!("Zcode 工作流回传失败：{error}")})); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ended_interaction_rejects_late_response_before_touching_protocol() {
        let app = super::super::tests::ctx("zcode-late-interaction");
        let proc = super::super::tests::inert_process(SessionKind::Zcode);
        let stale = json!({"nativeRequestId":"server-1","zcodeMethod":"interaction/requestPermission"});
        proc.permissions.lock().unwrap().insert("permission-1".into(), stale.clone());
        clear_interactions(&app,"s",&proc);
        let error = respond(&app,"s",&proc,"permission-1",true,None,None).unwrap_err();
        assert_eq!(error,"That permission request is no longer waiting for an answer");
    }

    #[test]
    fn interaction_answer_uses_latest_transport_and_rejects_replay() {
        use std::process::Stdio;
        let app = super::super::tests::ctx("zcode-interaction-replay");
        let proc = super::super::tests::inert_process(SessionKind::Zcode);
        let mut command = crate::host::command(if cfg!(windows) {"cmd.exe"} else {"cat"});
        if cfg!(windows) { command.args(["/D","/C","more"]); }
        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        *proc.stdin.lock().unwrap() = child.stdin.take();
        proc.permissions.lock().unwrap().insert("permission-1".into(),json!({"nativeRequestId":"server-2","zcodeMethod":"interaction/requestPermission"}));
        let first = respond(&app,"s",&proc,"permission-1",true,None,None);
        let repeated = respond(&app,"s",&proc,"permission-1",false,None,None);
        proc.stdin.lock().unwrap().take();
        let output = child.wait_with_output().unwrap();
        first.unwrap();
        assert!(repeated.is_err(),"同一卡片只能回答一次");
        let frames: Vec<Value> = String::from_utf8(output.stdout).unwrap().lines()
            .filter(|line| !line.trim().is_empty()).map(|line|serde_json::from_str(line).unwrap()).collect();
        assert_eq!(frames,vec![json!({"id":"server-2","result":{"decision":"allow","reason":""}})]);
    }
    #[test]
    fn native_interaction_reannouncement_and_terminal_keep_one_live_card() {
        let app = super::super::tests::ctx("zcode-interaction-owner");
        let proc = super::super::tests::inert_process(SessionKind::Zcode);
        *proc.agent_session_id.lock().unwrap() = Some("sess-native".into());
        proc.turn.lock().unwrap().running = true;
        for id in ["server-1", "server-2"] {
            handle_line(&app,"s",&proc,&json!({"id":id,"method":"interaction/requestPermission","params":{
                "sessionId":"sess-native","requestId":"permission-1","toolName":"Write","input":{"content":"test"}
            }}).to_string());
        }
        assert_eq!(proc.permissions.lock().unwrap().len(), 1, "重宣同一交互不能生成多张卡片");
        assert_eq!(proc.permissions.lock().unwrap()["permission-1"]["nativeRequestId"], "server-2");
        handle_line(&app,"s",&proc,&json!({"method":"session/event","params":{
            "sessionId":"sess-native","seq":1,"turnId":"turn-1","type":"turn.completed","payload":{"resultType":"cancelled"}
        }}).to_string());
        assert!(proc.permissions.lock().unwrap().is_empty(), "终态必须撤销不再有效的交互");
    }

    #[test]
    fn native_mode_events_project_tool_driven_plan_transitions() {
        let app = super::super::tests::ctx("zcode-mode-owner");
        let proc = super::super::tests::inert_process(SessionKind::Zcode);
        *proc.agent_session_id.lock().unwrap() = Some("sess-native".into());
        for (seq, planned, expected) in [(1,true,"plan"),(2,false,"build")] {
            handle_line(&app,"s",&proc,&json!({"method":"session/event","params":{
                "sessionId":"sess-native","seq":seq,"type":"session.updated","payload":{"mode":"build","planEnabled":planned,"source":"tool"}
            }}).to_string());
            assert_eq!(*proc.mode.lock().unwrap(), expected);
        }
    }
    #[test]
    fn repeated_old_terminal_cannot_finish_the_next_input() {
        let app = super::super::tests::ctx("zcode-terminal-owner");
        let proc = super::super::tests::inert_process(SessionKind::Zcode);
        *proc.agent_session_id.lock().unwrap() = Some("sess-native".into());
        let event = |seq, turn, kind: &str| json!({"method":"session/event","params":{
            "sessionId":"sess-native", "seq":seq, "turnId":turn, "type":kind,
            "payload":{"response":"RESULT", "resultType":"success"}
        }}).to_string();
        proc.turn.lock().unwrap().running = true;
        proc.zcode.lock().unwrap().current_input = Some("input-a".into());
        handle_line(&app,"s",&proc,&event(1,"turn-a","turn.started"));
        handle_line(&app,"s",&proc,&event(2,"turn-a","turn.completed"));
        assert!(!proc.turn.lock().unwrap().running);
        // 第二次派发已拥有新的 input 与原生 turn，旧终态仍可能以新 seq 到达。
        proc.turn.lock().unwrap().running = true;
        proc.zcode.lock().unwrap().current_input = Some("input-b".into());
        handle_line(&app,"s",&proc,&event(3,"turn-b","turn.started"));
        handle_line(&app,"s",&proc,&event(4,"turn-a","turn.completed"));
        assert!(proc.turn.lock().unwrap().running, "旧回合不能误结束下一任务");
        assert!(!proc.zcode.lock().unwrap().completed.contains("input-b"));
        handle_line(&app,"s",&proc,&event(5,"turn-b","turn.completed"));
        assert!(!proc.turn.lock().unwrap().running);
        assert!(proc.zcode.lock().unwrap().completed.contains("input-b"));
    }
    #[test]
    fn native_plan_review_keeps_option_values() {
        let app = super::super::tests::ctx("zcode-plan");
        let proc = super::super::tests::inert_process(SessionKind::Zcode);
        *proc.agent_session_id.lock().unwrap() = Some("sess-native".into());
        handle_line(&app, "s", &proc, &json!({"id":"review", "method":"interaction/requestUserInput", "params":{
            "sessionId":"sess-native", "questions":[{"header":"Plan", "question":"Review this implementation plan.", "options":[{"label":"Approve", "value":"approve"}]}]
        }}).to_string());
        let permissions = proc.permissions.lock().unwrap();
        assert_eq!(permissions["review"]["input"]["questions"][0]["options"][0]["value"], "approve");
    }
    #[test]
    fn model_selection_preserves_native_reasoning_default() {
        let choice=json!({"ref":{"providerId":"local","modelId":"coding"},"reasoning":{"defaultLevel":"medium","levels":[{"value":"medium"},{"value":"high"}]}});
        assert_eq!(complete_selection(&choice,Some("unsupported"))["options"]["reasoningLevel"],"medium");
        assert_eq!(complete_selection(&choice,Some("high"))["options"]["reasoningLevel"],"high");
    }
    #[test]
    fn native_tool_lifecycle_keeps_arguments_and_finishes_one_card() {
        let app=super::super::tests::ctx("zcode-tool");
        let proc=super::super::tests::inert_process(SessionKind::Zcode);
        *proc.agent_session_id.lock().unwrap()=Some("sess-native".into());
        for (seq,(event,payload)) in [
            ("model.streaming",json!({"kind":"tool_call","toolCallId":"call-1","toolName":"Bash","input":{"command":"echo CLI"}})),
            ("tool.updated",json!({"kind":"scheduled","toolCallId":"call-1","toolName":"Bash","inputOmitted":true,"inputRef":"model_stream"})),
            ("tool.updated",json!({"kind":"started","toolCallId":"call-1"})),
            ("tool.updated",json!({"kind":"result","toolCallId":"call-1","result":{"content":"CLI","isError":false}})),
        ].into_iter().enumerate() {
            handle_line(&app,"s",&proc,&json!({"method":"session/event","params":{"sessionId":"sess-native","seq":seq+1,"type":event,"payload":payload}}).to_string());
        }
        let rows=proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(),1);
        assert!(matches!(&rows[0],ChatRow::Tool{name,input,output,status,..} if name=="Bash" && input["command"]=="echo CLI" && output.as_deref()==Some("CLI") && *status=="completed"));
    }
    #[test]
    fn native_model_catalogue_has_string_descriptions_when_optional_metadata_is_absent() {
        let app = super::super::tests::ctx("zcode-model-description");
        let proc = super::super::tests::inert_process(SessionKind::Zcode);
        apply_snapshot(&app, "s", &proc, &json!({"settings":{"model":{"available":[
            {"ref":{"providerId":"account:bigmodel","modelId":"GLM-5.3"},"label":"GLM-5.3","providerLabel":"BigModel"},
            {"ref":{"providerId":"account:bigmodel","modelId":"GLM-5.2"},"label":"GLM-5.2","description":null},
            {"ref":{"providerId":"account:bigmodel","modelId":"GLM-5"},"label":"GLM-5","providerLabel":"Other Provider","description":"Native description"}
        ]}}}), false);
        let state = proc.zcode.lock().unwrap();
        assert_eq!(state.models.len(), 3);
        assert_eq!(state.models[0]["description"], "BigModel");
        assert_eq!(state.models[1]["description"], "");
        assert_eq!(state.models[2]["description"], "Other Provider · Native description");
        assert_eq!(state.models[0]["id"], "account:bigmodel/GLM-5.3");
    }

    #[test]
    fn native_stream_is_routed_to_its_session_and_terminal_result_finishes_turn() {
        let app=super::super::tests::ctx("zcode-protocol");
        let proc=super::super::tests::inert_process(SessionKind::Zcode);
        *proc.agent_session_id.lock().unwrap()=Some("sess-native".into());
        proc.turn.lock().unwrap().running=true;
        for native in ["sess-foreign","sess-native"] {
            handle_line(&app,"s",&proc,&json!({"method":"session/event","params":{"sessionId":native,"seq":1,"type":"model.streaming","payload":{"kind":"text_delta","assistantMessageId":"answer","delta":"CLI_OK"}}}).to_string());
        }
        let rows=proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(),1);
        assert!(matches!(&rows[0],ChatRow::Assistant{text,streaming:true,..} if text=="CLI_OK"));
        handle_line(&app,"s",&proc,&json!({"method":"session/event","params":{"sessionId":"sess-native","seq":2,"type":"turn.completed","payload":{"response":"CLI_OK","duration":9,"resultType":"success"}}}).to_string());
        assert!(!proc.turn.lock().unwrap().running);
        assert_eq!(proc.timeline.lock().unwrap().rows.len(),1);
        assert!(matches!(&proc.timeline.lock().unwrap().rows[0],ChatRow::Assistant{streaming:false,..}));
    }
    #[test]
    fn foreground_terminal_keeps_child_permission_until_the_process_stops() {
        let app=super::super::tests::ctx("zcode-background-permission");
        let proc=super::super::tests::inert_process(SessionKind::Zcode);
        *proc.agent_session_id.lock().unwrap()=Some("root".into());
        for (id,origin) in [("parent",json!(null)),("child",json!({"kind":"subagent","childSessionId":"native-child","parentSessionId":"root"}))] {
            handle_line(&app,"s",&proc,&json!({"id":id,"method":"interaction/requestPermission","params":{"sessionId":"root","requestId":id,"toolName":"Write","input":{},"origin":origin}}).to_string());
        }
        handle_line(&app,"s",&proc,&json!({"method":"session/event","params":{"sessionId":"root","seq":1,"type":"turn.completed","payload":{"resultType":"success","response":"done"}}}).to_string());
        assert_eq!(proc.permissions.lock().unwrap().len(),1);
        assert!(proc.permissions.lock().unwrap().contains_key("child"));
        clear_interactions(&app,"s",&proc);
        assert!(proc.permissions.lock().unwrap().is_empty());
    }

}
