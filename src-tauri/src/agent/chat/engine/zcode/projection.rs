//! V4 只读投影：用量与原生子任务；主会话正文仍由既有协议接收。
use super::{AppCtx, ChatProcess, ChatRow};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct State {
    topics: HashMap<String, Topic>,
    fragments: HashMap<String, Assembly>,
}
#[derive(Default)]
struct Topic {
    path: Vec<String>,
    subscription: String,
    epoch: String,
    seq: Option<u64>,
    rows: BTreeMap<i64, Value>,
    model: Option<String>,
    reported_fallbacks: HashSet<String>,
}
struct Assembly {
    started: u64,
    bytes: usize,
    checksum: String,
    parts: Vec<Option<Vec<u8>>>,
}

pub(super) fn subscribe(proc: &ChatProcess, native: &str) -> Result<(), String> {
    let topic = format!("conversation/{native}");
    proc.zcode
        .lock()
        .unwrap()
        .projection
        .topics
        .entry(topic.clone())
        .or_default();
    proc.request_and_wait("zcode_projection",|id|json!({"id":id,"method":"v4/conversation/subscribe","params":{"connectionId":"velaterm","topic":topic,"clientMode":"desktop-continuous"}}))?;
    Ok(())
}

fn crc32(bytes: &[u8]) -> String {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ ((0u32.wrapping_sub(crc & 1)) & 0xedb88320);
        }
    }
    format!("{:08x}", !crc)
}

fn logical(state: &mut State, wire: &Value) -> Option<Value> {
    use base64::Engine;
    if wire["kind"] == "complete" {
        return wire.get("frame").cloned();
    }
    if wire["kind"] != "fragment" {
        return None;
    }
    let id = format!(
        "{}:{}",
        wire["subscriptionId"].as_str()?,
        wire["logicalFrameId"].as_str()?
    );
    let count = wire["fragmentCount"].as_u64()? as usize;
    let index = wire["fragmentIndex"].as_u64()? as usize;
    let bytes = wire["logicalBytes"].as_u64()? as usize;
    if count == 0 || count > 1024 || index >= count || bytes > 16 * 1024 * 1024 {
        return None;
    }
    let now = super::now_ms();
    state.fragments.retain(|_, a| now.saturating_sub(a.started) < 30_000);
    if !state.fragments.contains_key(&id)
        && (state.fragments.len() >= 32
            || state
                .fragments
                .values()
                .map(|a| a.bytes)
                .sum::<usize>()
                .saturating_add(bytes)
                > 32 * 1024 * 1024)
    {
        return None;
    }
    let checksum = wire["checksum"]["value"].as_str()?;
    let entry = state.fragments.entry(id.clone()).or_insert_with(|| Assembly {
        started: now,
        bytes,
        checksum: checksum.into(),
        parts: vec![None; count],
    });
    if entry.bytes != bytes || entry.parts.len() != count || entry.checksum != checksum {
        state.fragments.remove(&id);
        return None;
    }
    let part = base64::engine::general_purpose::STANDARD
        .decode(wire["dataBase64"].as_str()?)
        .ok()?;
    if let Some(old) = &entry.parts[index] {
        if old == &part {
            return None;
        }
        state.fragments.remove(&id);
        return None;
    }
    if part.len() > 1024 * 1024
        || entry
            .parts
            .iter()
            .flatten()
            .map(Vec::len)
            .sum::<usize>()
            .saturating_add(part.len())
            > bytes
    {
        state.fragments.remove(&id);
        return None;
    }
    entry.parts[index] = Some(part);
    if entry.parts.iter().any(Option::is_none) {
        return None;
    }
    let entry = state.fragments.remove(&id)?;
    let payload: Vec<u8> = entry.parts.into_iter().flatten().flatten().collect();
    if payload.len() != bytes || crc32(&payload) != checksum {
        return None;
    }
    serde_json::from_slice(&payload).ok()
}

fn guide_fallbacks(topic: &mut Topic, queue: &Value, reports: &mut Vec<Value>) {
    if !topic.path.is_empty() { return; }
    for item in queue["items"].as_array().into_iter().flatten() {
        let Some(id)=item["sourceCommandId"].as_str().filter(|id|id.starts_with("velaterm-guide:")) else {continue};
        if item["delivery"]["requested"]=="guide" && item["delivery"]["admitted"]=="queue" && topic.reported_fallbacks.insert(id.into()) {
            reports.push(json!({"type":"steerFallback","reason":item["delivery"]["fallbackReasonCode"]}));
        }
    }
}

pub(super) fn handle(app: &AppCtx, session: &str, proc: &Arc<ChatProcess>, wire: &Value) {
    let Some(topic_name) = wire["topic"].as_str() else {
        return;
    };
    let mut guard = proc.zcode.lock().unwrap();
    let state = &mut guard.projection;
    // 只能读取当前会话，以及已被原生父投影证明归属的子任务。
    if !state.topics.contains_key(topic_name) {
        return;
    }
    let Some(frame) = logical(state, wire) else { return };
    if frame["topic"] != wire["topic"] || frame["subscriptionId"] != wire["subscriptionId"] {
        return;
    }
    let Some(topic) = state.topics.get_mut(topic_name) else {
        return;
    };
    let Some(seq) = frame["toSeq"].as_u64() else { return };
    let epoch = frame["logEpoch"].as_str().unwrap_or("");
    let subscription = frame["subscriptionId"].as_str().unwrap_or("");
    let payload = &frame["payload"];
    let mut usage = None;
    let mut reports = Vec::new();
    if payload["kind"] == "snapshot" {
        if topic.subscription == subscription && topic.epoch == epoch && topic.seq.is_some_and(|old| seq < old) {
            return;
        }
        if !topic.subscription.is_empty() && topic.subscription != subscription {
            return;
        }
        topic.subscription = subscription.into();
        topic.epoch = epoch.into();
        topic.seq = Some(seq);
        topic.rows.clear();
        let snapshot = &payload["snapshot"];
        guide_fallbacks(topic,&snapshot["queue"],&mut reports);
        for row in snapshot["rows"]["window"].as_array().into_iter().flatten() {
            if !topic.path.is_empty() || row["kind"] == "subagent" {
                if let Some(id) = row["rowId"].as_i64() {
                    topic.rows.insert(id, row.clone());
                }
            }
        }
        topic.model = model(&snapshot["config"]).or(topic.model.take());
        usage = snapshot.get("usage").cloned();
    } else if payload["kind"] == "deltas" {
        if topic.subscription != subscription || topic.epoch != epoch || topic.seq.is_some_and(|old| seq <= old) {
            return;
        }
        if frame["fromSeq"].as_u64() != topic.seq {
            let id = proc.request_id("zcode_projection");
            let _=proc.write(&json!({"id":id,"method":"v4/conversation/resync","params":{"connectionId":"velaterm","topic":topic_name,"subscriptionId":subscription}}));
            return;
        }
        topic.seq = Some(seq);
        for delta in payload["deltas"].as_array().into_iter().flatten() {
            match delta["op"].as_str() {
                Some("row.appended" | "row.upserted") => {
                    let row = &delta["row"];
                    if !topic.path.is_empty() || row["kind"] == "subagent" {
                        if let Some(id) = row["rowId"].as_i64() {
                            topic.rows.insert(id, row.clone());
                        }
                    }
                }
                Some("row.removed") => {
                    if let Some(from) = delta["fromRowId"].as_i64() {
                        topic.rows.retain(|id, _| *id < from);
                    }
                }
                Some("row.delta") => {
                    if let Some(row) = delta["rowId"].as_i64().and_then(|id| topic.rows.get_mut(&id)) {
                        if let Some(path) = delta["path"].as_str() {
                            let pointer = format!("/{}", path.replace('.', "/"));
                            if let Some(value) = row.pointer_mut(&pointer) {
                                if let Some(text) = value.as_str() {
                                    *value = json!(format!("{text}{}", delta["append"].as_str().unwrap_or("")));
                                }
                            }
                        }
                    }
                }
                Some("state.updated") => {
                    let patch = &delta["patch"];
                    guide_fallbacks(topic,&patch["queue"],&mut reports);
                    if let Some(config) = patch.get("config") {
                        topic.model = model(config).or(topic.model.take());
                    }
                    if let Some(value) = patch.get("usage") {
                        usage = Some(value.clone());
                    }
                }
                _ => {}
            }
        }
    } else {
        return;
    }
    let path = topic.path.clone();
    let rows = topic.rows.values().cloned().collect::<Vec<_>>();
    let child_model = topic.model.clone();
    let mut discovered = Vec::new();
    for row in &rows {
        if row["kind"] != "subagent" {
            continue;
        }
        let (Some(child), Some(tool)) = (row["childSessionId"].as_str(), row["parentToolCallId"].as_str()) else {
            continue;
        };
        if path.len() >= 8 {
            continue;
        }
        let mut child_path = path.clone();
        child_path.push(if path.is_empty() {
            tool.into()
        } else {
            format!("{topic_name}:{tool}")
        });
        let topic = format!("conversation/{child}");
        if !state.topics.contains_key(&topic) {
            state.topics.insert(
                topic.clone(),
                Topic {
                    path: child_path,
                    ..Topic::default()
                },
            );
            discovered.push(topic);
        }
    }
    drop(guard);
    for report in reports {super::emit(app,session,report);}
    // 释放协议锁后才更新时间线，避免与 tool 事件形成交叉锁顺序。
    publish(app, session, proc, topic_name, &path, &rows, child_model, usage);
    for topic in discovered {
        let id = proc.request_id("zcode_projection");
        let _=proc.write(&json!({"id":id,"method":"v4/conversation/subscribe","params":{"connectionId":"velaterm","topic":topic,"clientMode":"desktop-continuous"}}));
    }
}

fn model(config: &Value) -> Option<String> {
    let selection = &config["modelSelection"];
    if let (Some(provider), Some(model)) = (selection["providerId"].as_str(), selection["modelId"].as_str()) {
        return Some(format!("{provider}/{model}"));
    }
    let (provider, model) = (config["provider"].as_str()?, config["model"].as_str()?);
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    Some(format!("{provider}/{model}"))
}

fn publish(
    app: &AppCtx,
    session: &str,
    proc: &Arc<ChatProcess>,
    namespace: &str,
    path: &[String],
    rows: &[Value],
    model: Option<String>,
    usage: Option<Value>,
) {
    if path.is_empty() {
        if let Some(usage) = usage {
            let mut extras = proc.extras.lock().unwrap();
            let context = &usage["contextWindow"];
            extras.context_tokens = context["usedTokens"].as_u64();
            extras.context_window = context["maxTokens"].as_u64().filter(|v| *v > 0);
            drop(extras);
            super::super::emit_extras(app, session, proc);
        }
    } else {
        let mut children = rows
            .iter()
            .filter_map(|row| map_row(row, namespace, model.as_deref()))
            .collect::<Vec<_>>();
        let mut timeline = proc.timeline.lock().unwrap();
        for child in &mut children {
            if let ChatRow::Tool {
                id, subagent, children, ..
            } = child
            {
                if let Some(ChatRow::Tool {
                    subagent: old_info,
                    children: old_children,
                    ..
                }) = super::super::find_row(&timeline.rows, id)
                {
                    *subagent = old_info.clone();
                    *children = old_children.clone();
                }
            }
        }
        // 子快照可能先于父工具事件到达，先建立同一路径的占位卡，随后由原生工具更新补齐。
        for depth in 0..path.len() {
            if super::super::find_row(&timeline.rows, &path[depth]).is_none() {
                timeline.upsert_at(
                    &path[..depth],
                    ChatRow::Tool {
                        id: path[depth].clone(),
                        name: "Agent".into(),
                        input: Value::Null,
                        output: None,
                        is_error: false,
                        status: "running",
                        subagent: None,
                        children: vec![],
                    },
                );
            }
        }
        let parent_path = &path[..path.len() - 1];
        let tool = &path[path.len() - 1];
        let previous = super::super::find_row(&timeline.rows, tool).cloned();
        if let Some(ChatRow::Tool {
            id,
            name,
            input,
            output,
            is_error,
            mut status,
            mut subagent,
            ..
        }) = previous
        {
            if let Some(header) = rows.iter().rev().find(|row| row["kind"] == "turnHeader") {
                status = match header["state"].as_str() {
                    Some("running") => "running",
                    Some("completedSuccess") => "completed",
                    Some("completedInterrupted") => "cancelled",
                    Some("failed") => "failed",
                    _ => status,
                };
            }
            let info = subagent.get_or_insert_with(Default::default);
            info.model = model;
            info.tool_uses = Some(children.iter().filter(|r| matches!(r, ChatRow::Tool { .. })).count() as u64);
            if let Some(usage) = usage {
                info.total_tokens = usage["cumulative"]["inputTokens"]
                    .as_u64()
                    .zip(usage["cumulative"]["outputTokens"].as_u64())
                    .map(|(i, o)| i.saturating_add(o));
            }
            timeline.upsert_at(
                parent_path,
                ChatRow::Tool {
                    id,
                    name,
                    input,
                    output,
                    is_error,
                    status,
                    subagent,
                    children,
                },
            );
        }
    }
    let mut timeline = proc.timeline.lock().unwrap();
    for row in rows.iter().filter(|r| r["kind"] == "subagent") {
        let Some(native_tool) = row["parentToolCallId"].as_str() else {
            continue;
        };
        let tool = if path.is_empty() {
            native_tool.to_string()
        } else {
            format!("{namespace}:{native_tool}")
        };
        let Some(ChatRow::Tool {
            id,
            name,
            input,
            output,
            is_error,
            status,
            subagent,
            children,
        }) = super::super::find_row(&timeline.rows, &tool).cloned()
        else {
            continue;
        };
        let mut info = subagent.unwrap_or_default();
        info.title = row["subagentType"].as_str().map(str::to_string);
        info.description = input["description"].as_str().map(str::to_string);
        info.duration_ms = row["endedAt"]
            .as_f64()
            .zip(row["startedAt"].as_f64())
            .map(|(end, start)| (end - start).max(0.0) as u64);
        let status = match row["status"].as_str() {
            Some("running") => "running",
            Some("success") => "completed",
            Some("failed" | "lost") => "failed",
            Some("cancelled") => "cancelled",
            _ => status,
        };
        timeline.upsert_at(
            path,
            ChatRow::Tool {
                id,
                name,
                input,
                output,
                is_error: is_error || status == "failed",
                status,
                subagent: Some(info),
                children,
            },
        );
    }
}

fn map_row(row: &Value, namespace: &str, model: Option<&str>) -> Option<ChatRow> {
    let id = format!("{namespace}:v4:{}", row["rowId"].as_i64()?);
    let streaming = row["state"] == "streaming";
    match row["kind"].as_str()? {
        "assistantText" => Some(ChatRow::Assistant {
            id,
            text: row["text"].as_str().unwrap_or("").into(),
            streaming,
            model: model.map(str::to_string),
            at: row["createdAt"].as_f64().map(|v| v as i64),
            duration_ms: None,
        }),
        "reasoning" => Some(ChatRow::Reasoning {
            id,
            text: row["text"].as_str().unwrap_or("").into(),
            streaming,
        }),
        "toolCall" => Some(ChatRow::Tool {
            id: format!("{namespace}:{}", row["toolCallId"].as_str()?),
            name: row["toolName"].as_str().unwrap_or("Tool").into(),
            input: row
                .get("input")
                .filter(|value| !value.is_null())
                .cloned()
                .unwrap_or_else(|| {
                    let text = row["inputText"].as_str().unwrap_or("");
                    serde_json::from_str(text).unwrap_or_else(|_| json!(text))
                }),
            output: row["output"]["text"]
                .as_str()
                .or_else(|| row["error"]["message"].as_str())
                .map(str::to_string),
            is_error: row["status"] == "error",
            status: match row["status"].as_str() {
                Some("success") => "completed",
                Some("error") => "failed",
                Some("cancelled") => "cancelled",
                _ => "running",
            },
            subagent: None,
            children: vec![],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
