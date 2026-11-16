//! OpenCode's background work, published as the composer's task list.
//!
//! A turn can end while subagents or shell commands keep running. OpenCode v2 reports those through its
//! background-job list; v1 only detaches synchronous subagents. Both are folded into the same
//! `extras.backgroundTasks` the Claude engine fills, so the composer, the task tabs and the session's
//! background status need no agent-specific branch.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::{root_session, server};
use super::super::{
    cap_finished_tasks, emit_extras, now_ms, release_if_idle, settle_background_state,
    BackgroundTask, ChatProcess, ChatRow, TaskOutput,
};
use crate::agent::chat::opencode_timeline;
use crate::host::AppCtx;

/// How often the poller asks the server what is still running.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Give jobs created by a just-finished turn a moment to appear before the first poll.
const FIRST_POLL: Duration = Duration::from_millis(400);
/// How many consecutive job lists may omit a running task before it is considered gone.
const JOB_MISSES: u8 = 3;
/// A child session that never appears active within this long after being promoted is already over.
const START_GRACE_MS: u64 = 5_000;
/// The most shell output one task-page request returns.
const OUTPUT_TAIL: usize = 64 * 1024;

struct Candidate {
    task_id: String,
    task_type: &'static str,
    description: String,
    started_at: u64,
    tool_use_id: String,
}

/// Remember everything a tool part contributes to task tracking. Called for live parts and for history
/// rebuilt from the server, so the same tool can be promoted in either path.
pub(super) fn observe_tool(state: &mut super::OpencodeState, part: &Value) {
    if part.get("type").and_then(Value::as_str) != Some("tool") {
        return;
    }
    let Some(id) = part.get("id").and_then(Value::as_str) else { return };
    let started = part
        .pointer("/state/time/start")
        .or_else(|| part.pointer("/time/start"))
        .and_then(Value::as_u64);
    if let Some(started) = started {
        state.tool_started.entry(id.to_string()).or_insert(started);
    }
    if let Some(child) = opencode_timeline::child_session_of(part) {
        state.child_parts.entry(id.to_string()).or_insert(child);
    }
}

/// Register the work that will outlive the turn that just ended, then follow it until it finishes.
pub(super) fn after_turn(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let found = candidates(proc, false);
    if !found.is_empty() {
        promote(app, session_id, proc, found);
    }
    if pending(proc) {
        ensure_poller(app, session_id, proc);
    }
}

/// Move the foreground work of the running turn into the background.
pub(super) fn background(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) -> Result<(), String> {
    let server = server(proc)?;
    let native = root_session(proc)?;
    // Snapshot the running tools before asking: detaching may complete their call rows while the child
    // sessions keep going.
    let found = candidates(proc, true);
    server.background(&native)?;
    if !found.is_empty() {
        promote(app, session_id, proc, found);
    }
    if pending(proc) {
        ensure_poller(app, session_id, proc);
    }
    Ok(())
}

/// Stop one task: interrupt its child session, or terminate its shell command.
pub(super) fn stop_task(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, task_id: &str) -> Result<(), String> {
    let (task_type, shell_id) = {
        let extras = proc.extras.lock().unwrap();
        let task_type = extras
            .background_tasks
            .iter()
            .find(|task| task.task_id == task_id)
            .ok_or("Background task is no longer available")?
            .task_type
            .clone();
        drop(extras);
        let shell = proc.opencode.lock().unwrap().shell_ids.get(task_id).cloned();
        (task_type, shell)
    };
    let server = server(proc)?;
    match task_type.as_str() {
        "local_agent" => {
            if let Err(error) = server.interrupt_session(task_id) {
                if !crate::agent::chat::opencode_protocol::is_not_found(&error) {
                    return Err(error);
                }
            }
        }
        "local_bash" => {
            let Some(shell) = shell_id else {
                return Err("This background command cannot be stopped from here".into());
            };
            if let Err(error) = server.delete_shell(&shell) {
                if !crate::agent::chat::opencode_protocol::is_not_found(&error) {
                    return Err(error);
                }
            }
        }
        _ => return Err("This task cannot be stopped".into()),
    }
    {
        let mut extras = proc.extras.lock().unwrap();
        if let Some(task) = extras.background_tasks.iter_mut().find(|task| task.task_id == task_id) {
            task.set_status("canceled", now_ms());
            task.in_inventory = false;
        }
    }
    sync_turn(proc);
    emit_extras(app, session_id, proc);
    after_pending_change(app, session_id, proc);
    Ok(())
}

/// The command a background shell task runs and the tail of its output.
pub(super) fn task_output(proc: &Arc<ChatProcess>, task_id: &str) -> Result<TaskOutput, String> {
    let (task_type, tool_use_id, description, shell_id) = {
        let extras = proc.extras.lock().unwrap();
        let task = extras
            .background_tasks
            .iter()
            .find(|task| task.task_id == task_id)
            .ok_or("Background task is no longer available")?;
        let fields = (task.task_type.clone(), task.tool_use_id.clone(), task.description.clone());
        drop(extras);
        let shell = proc.opencode.lock().unwrap().shell_ids.get(task_id).cloned();
        (fields.0, fields.1, fields.2, shell)
    };
    let mut command = None;
    let mut text = None;
    if let Some(tool_use_id) = tool_use_id {
        let timeline = proc.timeline.lock().unwrap();
        if let Some(ChatRow::Tool { input, output, .. }) = super::super::find_row(&timeline.rows, &tool_use_id) {
            command = input.get("command").and_then(Value::as_str).map(str::to_string);
            text = output.clone();
        }
    }
    if task_type == "local_bash" {
        command.get_or_insert(description);
        if text.as_deref().is_none_or(str::is_empty) {
            if let Some(shell) = shell_id {
                if let Ok(server) = server(proc) {
                    if let Ok(output) = server.shell_output(&shell) {
                        if !output.is_empty() {
                            text = Some(output);
                        }
                    }
                }
            }
        }
    }
    let (output, truncated) = match text {
        Some(text) => {
            let (tail, truncated) = tail(&text);
            (Some(tail), truncated)
        }
        None => (None, false),
    };
    Ok(TaskOutput { command, output, truncated })
}

fn candidates(proc: &Arc<ChatProcess>, backgrounded: bool) -> Vec<Candidate> {
    let (children, started) = {
        let state = proc.opencode.lock().unwrap();
        (state.child_parts.clone(), state.tool_started.clone())
    };
    let now = now_ms();
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let timeline = proc.timeline.lock().unwrap();
    collect_rows(&timeline.rows, &children, &started, backgrounded, now, &mut found, &mut seen);
    found
}

fn collect_rows(
    rows: &[ChatRow],
    children: &HashMap<String, String>,
    started: &HashMap<String, u64>,
    backgrounded: bool,
    now: u64,
    found: &mut Vec<Candidate>,
    seen: &mut HashSet<String>,
) {
    for row in rows {
        let ChatRow::Tool { id, name, input, subagent, status, children: cards, .. } = row else { continue };
        if let Some(child) = children.get(id) {
            if *status == "running" && seen.insert(child.clone()) {
                let description = subagent
                    .as_ref()
                    .and_then(|facts| facts.description.clone())
                    .or_else(|| input.get("description").and_then(Value::as_str).map(str::to_string))
                    .or_else(|| input.get("subagent_type").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_else(|| "Subagent".to_string());
                found.push(Candidate {
                    task_id: child.clone(),
                    task_type: "local_agent",
                    description,
                    started_at: started.get(id).copied().unwrap_or(now),
                    tool_use_id: id.clone(),
                });
            }
        } else if name == "Bash" && *status == "running" {
            let background = input.get("background").and_then(Value::as_bool) == Some(true);
            if (backgrounded || background) && seen.insert(id.clone()) {
                found.push(Candidate {
                    task_id: id.clone(),
                    task_type: "local_bash",
                    description: input.get("command").and_then(Value::as_str).unwrap_or("Shell command").to_string(),
                    started_at: started.get(id).copied().unwrap_or(now),
                    tool_use_id: id.clone(),
                });
            }
        }
        collect_rows(cards, children, started, backgrounded, now, found, seen);
    }
}

fn promote(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, found: Vec<Candidate>) {
    let now = now_ms();
    let changed = {
        let mut extras = proc.extras.lock().unwrap();
        let before = signature(&extras.background_tasks);
        for candidate in found {
            if let Some(task) = extras.background_tasks.iter_mut().find(|task| task.task_id == candidate.task_id) {
                // A finished task stays finished: stopping it is the user's decision, not a hint to start
                // it again. A task the provider really restarted reappears in a job list and revives there.
                if task.finished() { continue; }
                if task.tool_use_id.is_none() { task.tool_use_id = Some(candidate.tool_use_id); }
                if task.task_type.is_empty() { task.task_type = candidate.task_type.to_string(); }
                if task.description.is_empty() { task.description = candidate.description; }
                if task.started_at.is_none() { task.started_at = Some(candidate.started_at); }
                task.listed = true;
                continue;
            }
            let mut task = BackgroundTask {
                task_id: candidate.task_id,
                task_type: candidate.task_type.to_string(),
                description: candidate.description,
                listed: true,
                // A promoted shell has no command id until a job list names it, so it cannot be stopped
                // yet. Subagents are addressed by their child session and can be stopped at once.
                in_inventory: candidate.task_type != "local_bash",
                started_at: Some(candidate.started_at),
                tool_use_id: Some(candidate.tool_use_id),
                ..BackgroundTask::default()
            };
            task.set_status("running", now);
            extras.background_tasks.push(task);
        }
        cap_finished_tasks(&mut extras.background_tasks);
        let after = signature(&extras.background_tasks);
        before != after
    };
    sync_turn(proc);
    if changed {
        emit_extras(app, session_id, proc);
    }
}

fn ensure_poller(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    if !pending(proc) {
        return;
    }
    {
        let mut state = proc.opencode.lock().unwrap();
        if state.polling { return; }
        state.polling = true;
    }
    let (app, session_id, proc) = (app.clone(), session_id.to_string(), proc.clone());
    std::thread::spawn(move || {
        let mut first = true;
        loop {
            std::thread::sleep(if first { FIRST_POLL } else { POLL_INTERVAL });
            first = false;
            if proc.released.load(Ordering::Relaxed) || !proc.alive.load(Ordering::Relaxed) {
                break;
            }
            if !app.chat().owns_process(&session_id, &proc) {
                break;
            }
            if !refresh(&app, &session_id, &proc) {
                break;
            }
        }
        let restart = {
            proc.opencode.lock().unwrap().polling = false;
            pending(&proc)
        };
        // A task promoted while the old poller was shutting down still needs someone to follow it.
        if restart {
            ensure_poller(&app, &session_id, &proc);
        }
    });
}

/// Ask the server what is still running. Returns whether any task remains live.
fn refresh(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) -> bool {
    let (Ok(server), Ok(root)) = (server(proc), root_session(proc)) else {
        return pending(proc);
    };
    let supported = proc.opencode.lock().unwrap().jobs;
    if server.is_v2() && supported != Some(false) {
        match server.jobs(&root) {
            Ok(jobs) => {
                proc.opencode.lock().unwrap().jobs = Some(true);
                apply_jobs(app, session_id, proc, &jobs);
            }
            Err(error) if unsupported_jobs(&error) => {
                proc.opencode.lock().unwrap().jobs = Some(false);
                follow_child_sessions(app, session_id, proc, &server);
            }
            Err(_) => follow_child_sessions(app, session_id, proc, &server),
        }
    } else {
        follow_child_sessions(app, session_id, proc, &server);
    }
    let more = pending(proc);
    sync_turn(proc);
    if !more {
        if !proc.turn.lock().unwrap().running {
            settle_background_state(app, session_id, proc);
            release_if_idle(app, session_id, proc);
        }
    }
    more
}

fn unsupported_jobs(error: &str) -> bool {
    error.contains("HTTP 404")
        || error.contains("HTTP 405")
        || error.contains("HTTP 501")
        || error.contains("non-JSON")
        || error.contains("does not report background jobs")
}

fn apply_jobs(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, jobs: &[Value]) {
    let now = now_ms();
    let mut misses = std::mem::take(&mut proc.opencode.lock().unwrap().job_misses);
    let mut shells = HashMap::new();
    let mut parsed = Vec::new();
    for job in jobs {
        if let Some((task, shell)) = job_task(job) {
            if let Some((id, shell)) = shell {
                shells.insert(id, shell);
            }
            parsed.push(task);
        }
    }
    let changed = {
        let mut extras = proc.extras.lock().unwrap();
        let before = signature(&extras.background_tasks);
        let names: HashSet<String> = parsed.iter().map(|task| task.task_id.clone()).collect();

        // A shell task promoted from its tool call is the same command the job list now reports under a
        // job id. Adopt its tool row instead of showing the command twice.
        for job in &mut parsed {
            let previous = if job.task_type == "local_bash" {
                extras.background_tasks.iter().position(|task| {
                    task.listed
                        && task.task_type == "local_bash"
                        && task.tool_use_id.is_some()
                        && !shells.contains_key(&task.task_id)
                        && task.description == job.description
                })
            } else {
                extras.background_tasks.iter().position(|task| task.task_id == job.task_id)
            };
            let Some(index) = previous else { continue };
            let old = &extras.background_tasks[index];
            if job.tool_use_id.is_none() { job.tool_use_id = old.tool_use_id.clone(); }
            if job.started_at.is_none() { job.started_at = old.started_at; }
            if job.task_id != old.task_id && job.task_type == "local_bash" {
                extras.background_tasks.remove(index);
            }
        }

        for job in parsed {
            if let Some(task) = extras.background_tasks.iter_mut().find(|task| task.task_id == job.task_id) {
                merge_job(task, job, now);
            } else {
                extras.background_tasks.push(job);
            }
        }

        for task in &mut extras.background_tasks {
            if !task.listed || task.finished() { continue; }
            if names.contains(&task.task_id) {
                misses.remove(&task.task_id);
                continue;
            }
            let count = misses.entry(task.task_id.clone()).or_insert(0);
            *count += 1;
            if *count >= JOB_MISSES {
                task.set_status("ended", now);
                task.in_inventory = false;
                misses.remove(&task.task_id);
            }
        }
        cap_finished_tasks(&mut extras.background_tasks);
        let names: HashSet<&str> = extras.background_tasks.iter().map(|task| task.task_id.as_str()).collect();
        misses.retain(|id, _| names.contains(id.as_str()));
        let after = signature(&extras.background_tasks);
        before != after
    };
    {
        let mut state = proc.opencode.lock().unwrap();
        state.job_misses = misses;
        state.shell_ids.extend(shells);
    }
    if changed {
        emit_extras(app, session_id, proc);
    }
}

fn merge_job(task: &mut BackgroundTask, job: BackgroundTask, now: u64) {
    let status = job.status.clone();
    let stoppable = job.in_inventory;
    if task.status != status {
        task.set_status(&status, now);
    }
    task.task_type = job.task_type;
    task.description = job.description;
    if job.summary.is_some() { task.summary = job.summary; }
    if job.started_at.is_some() { task.started_at = job.started_at; }
    if job.ended_at.is_some() { task.ended_at = job.ended_at; }
    if task.tool_use_id.is_none() { task.tool_use_id = job.tool_use_id; }
    task.listed = true;
    task.in_inventory = stoppable && !task.finished();
}

/// v1 has no job list: follow the child sessions themselves and the tool rows of promoted shells.
fn follow_child_sessions(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, server: &crate::agent::chat::opencode_protocol::Server) {
    let running = {
        let extras = proc.extras.lock().unwrap();
        extras.background_tasks.iter()
            .filter(|task| task.listed && !task.finished())
            .map(|task| (task.task_id.clone(), task.task_type.clone(), task.started_at, task.tool_use_id.clone()))
            .collect::<Vec<_>>()
    };
    if running.is_empty() { return; }
    let active = if running.iter().any(|(_, kind, _, _)| kind == "local_agent") {
        match server.active_sessions() {
            Ok(active) => active,
            Err(_) => return,
        }
    } else {
        HashSet::new()
    };
    let now = now_ms();
    let tool_status = {
        let timeline = proc.timeline.lock().unwrap();
        running.iter().filter_map(|(_, kind, _, tool)| {
            if kind != "local_bash" { return None; }
            let id = tool.as_ref()?;
            match super::super::find_row(&timeline.rows, id) {
                Some(ChatRow::Tool { status, .. }) => Some((id.clone(), *status)),
                _ => None,
            }
        }).collect::<HashMap<_, _>>()
    };
    let changed = {
        let mut extras = proc.extras.lock().unwrap();
        let before = signature(&extras.background_tasks);
        for task in &mut extras.background_tasks {
            if !task.listed || task.finished() { continue; }
            match task.task_type.as_str() {
                "local_agent" => {
                    if active.contains(&task.task_id) { continue; }
                    let old_enough = task.started_at.is_some_and(|start| now.saturating_sub(start) >= START_GRACE_MS);
                    if old_enough {
                        task.set_status("completed", now);
                        task.in_inventory = false;
                    }
                }
                "local_bash" => {
                    let Some(status) = task.tool_use_id.as_ref().and_then(|id| tool_status.get(id)) else { continue; };
                    match *status {
                        "completed" => { task.set_status("completed", now); task.in_inventory = false; }
                        "failed" => { task.set_status("failed", now); task.in_inventory = false; }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        cap_finished_tasks(&mut extras.background_tasks);
        let after = signature(&extras.background_tasks);
        before != after
    };
    if changed {
        emit_extras(app, session_id, proc);
    }
}

fn job_task(job: &Value) -> Option<(BackgroundTask, Option<(String, String)>)> {
    let kind = job.get("kind").and_then(Value::as_str)?;
    let status = match job.get("status").and_then(Value::as_str) {
        Some("running") => "running",
        Some("completed") => "completed",
        Some("error") => "failed",
        Some("cancelled") | Some("canceled") => "canceled",
        _ => return None,
    };
    let started = job.pointer("/time/started").and_then(Value::as_u64);
    let ended = job.pointer("/time/completed").and_then(Value::as_u64);
    let text = |key: &str| job.get(key).and_then(Value::as_str).filter(|value| !value.trim().is_empty()).map(str::to_string);
    let task = |task_id: String, task_type: &str, description: String| {
        BackgroundTask {
            task_id,
            task_type: task_type.to_string(),
            description,
            status: status.to_string(),
            listed: true,
            in_inventory: status == "running",
            started_at: started,
            ended_at: ended,
            ..BackgroundTask::default()
        }
    };
    match kind {
        "subagent" => {
            let child = job.get("childSessionID").or_else(|| job.get("childSessionId")).and_then(Value::as_str)?;
            let description = text("description").or_else(|| text("title")).or_else(|| text("agent"))
                .unwrap_or_else(|| "Subagent".to_string());
            Some((task(child.to_string(), "local_agent", description), None))
        }
        "shell" => {
            let id = text("id")?;
            let shell = text("shellID").or_else(|| text("shellId"));
            let command = text("command").or_else(|| text("title")).unwrap_or_else(|| "Shell command".to_string());
            let mut task = task(id.clone(), "local_bash", command);
            if shell.is_none() { task.in_inventory = false; }
            Some((task, shell.map(|shell| (id, shell))))
        }
        _ => None,
    }
}

fn pending(proc: &Arc<ChatProcess>) -> bool {
    proc.extras.lock().unwrap().background_tasks.iter().any(|task| task.listed && !task.finished())
}

/// Keep the turn queue's background set in step: release, rewind and session state all read it.
fn sync_turn(proc: &Arc<ChatProcess>) {
    let running: HashSet<String> = {
        let extras = proc.extras.lock().unwrap();
        extras.background_tasks.iter()
            .filter(|task| task.listed && !task.finished())
            .map(|task| task.task_id.clone())
            .collect()
    };
    proc.turn.lock().unwrap().background_tasks = running;
}

fn after_pending_change(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    if pending(proc) {
        ensure_poller(app, session_id, proc);
    } else if !proc.turn.lock().unwrap().running {
        settle_background_state(app, session_id, proc);
        release_if_idle(app, session_id, proc);
    }
}

fn signature(tasks: &[BackgroundTask]) -> String {
    let mut out = String::new();
    for task in tasks {
        out.push_str(&format!(
            "{}|{}|{}|{}|{:?}|{:?}|{:?}|{}|{}|{}\n",
            task.task_id,
            task.task_type,
            task.status,
            task.description,
            task.summary,
            task.started_at,
            task.ended_at,
            task.tool_use_id.as_deref().unwrap_or(""),
            task.listed,
            task.in_inventory,
        ));
    }
    out
}

/// The end of a command's output, starting on a whole line when the beginning was cut off.
fn tail(text: &str) -> (String, bool) {
    if text.len() <= OUTPUT_TAIL {
        return (text.to_string(), false);
    }
    let mut start = text.len() - OUTPUT_TAIL;
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    let start = text[start..].find('\n').map(|offset| start + offset + 1).unwrap_or(start);
    (text[start..].to_string(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::chat::engine::tests::{ctx, inert_process};
    use crate::models::SessionKind;

    #[test]
    fn a_job_maps_to_the_task_shape_the_composer_reads() {
        let (task, shell) = job_task(&serde_json::json!({
            "id":"job-1","kind":"shell","status":"running","sessionID":"ses_root",
            "shellID":"sh_1","command":"npm test","time":{"started":1000}
        })).unwrap();
        assert_eq!(task.task_id, "job-1");
        assert_eq!(task.task_type, "local_bash");
        assert_eq!(task.description, "npm test");
        assert_eq!(task.started_at, Some(1000));
        assert_eq!(shell, Some(("job-1".to_string(), "sh_1".to_string())));

        let (task, shell) = job_task(&serde_json::json!({
            "id":"job-2","kind":"subagent","status":"completed","sessionID":"ses_root",
            "childSessionID":"ses_child","agent":"explore","description":"Map the parser",
            "time":{"started":1000,"completed":2500}
        })).unwrap();
        assert_eq!(task.task_id, "ses_child");
        assert_eq!(task.task_type, "local_agent");
        assert_eq!(task.description, "Map the parser");
        assert_eq!(task.ended_at, Some(2500));
        assert!(task.finished());
        assert!(shell.is_none());
    }

    #[test]
    fn running_tools_become_tasks_and_completed_ones_do_not() {
        let proc = inert_process(SessionKind::Opencode);
        {
            let mut state = proc.opencode.lock().unwrap();
            state.child_parts.insert("part-child".into(), "ses_child".into());
            state.child_parts.insert("part-done".into(), "ses_done".into());
            state.tool_started.insert("part-child".into(), 1000);
            state.tool_started.insert("part-shell".into(), 2000);
            state.tool_started.insert("part-foreground".into(), 3000);
        }
        {
            let mut timeline = proc.timeline.lock().unwrap();
            timeline.upsert(ChatRow::Tool {
                id: "part-child".into(), name: "Task".into(), input: serde_json::json!({"subagent_type":"explore"}),
                output: None, is_error: false, status: "running".into(),
                subagent: Some(crate::agent::chat::engine::SubagentInfo {
                    title: Some("explore".into()), description: Some("Map it".into()),
                    model: None, total_tokens: None, tool_uses: None, duration_ms: None,
                }),
                children: Vec::new(),
            });
            timeline.upsert(ChatRow::Tool {
                id: "part-done".into(), name: "Task".into(), input: serde_json::json!({}),
                output: None, is_error: false, status: "completed".into(), subagent: None, children: Vec::new(),
            });
            timeline.upsert(ChatRow::Tool {
                id: "part-shell".into(), name: "Bash".into(),
                input: serde_json::json!({"command":"sleep 30","background":true}),
                output: None, is_error: false, status: "running".into(), subagent: None, children: Vec::new(),
            });
            timeline.upsert(ChatRow::Tool {
                id: "part-foreground".into(), name: "Bash".into(),
                input: serde_json::json!({"command":"cargo test"}),
                output: None, is_error: false, status: "running".into(), subagent: None, children: Vec::new(),
            });
        }
        let found = candidates(&proc, false);
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|task| task.task_id == "ses_child" && task.task_type == "local_agent"));
        assert!(found.iter().any(|task| task.task_id == "part-shell" && task.task_type == "local_bash"));
        assert!(!found.iter().any(|task| task.task_id == "part-foreground"));
        let forced = candidates(&proc, true);
        assert!(forced.iter().any(|task| task.task_id == "part-foreground"));
    }

    #[test]
    fn a_job_that_disappears_ends_only_after_missing_three_times() {
        let app = ctx("opencode-jobs");
        let proc = inert_process(SessionKind::Opencode);
        apply_jobs(&app, "s", &proc, &[serde_json::json!({
            "id":"job-1","kind":"shell","status":"running","shellID":"sh_1","command":"npm test",
            "time":{"started":1000}
        })]);
        assert!(pending(&proc));
        apply_jobs(&app, "s", &proc, &[]);
        assert!(pending(&proc), "one empty list is not proof the job is gone");
        apply_jobs(&app, "s", &proc, &[]);
        assert!(pending(&proc), "two empty lists still leave room for a slow registration");
        apply_jobs(&app, "s", &proc, &[]);
        assert!(!pending(&proc));
        let extras = proc.extras.lock().unwrap();
        assert_eq!(extras.background_tasks[0].status, "ended");
    }

    #[test]
    fn a_finished_task_keeps_its_place_until_the_next_turn() {
        let app = ctx("opencode-finished");
        let proc = inert_process(SessionKind::Opencode);
        apply_jobs(&app, "s", &proc, &[serde_json::json!({
            "id":"job-1","kind":"shell","status":"running","shellID":"sh_1","command":"npm test",
            "time":{"started":1000}
        })]);
        apply_jobs(&app, "s", &proc, &[serde_json::json!({
            "id":"job-1","kind":"shell","status":"completed","shellID":"sh_1","command":"npm test",
            "time":{"started":1000,"completed":2000}
        })]);
        let extras = proc.extras.lock().unwrap();
        assert_eq!(extras.background_tasks.len(), 1);
        assert!(extras.background_tasks[0].finished());
        assert_eq!(extras.background_tasks[0].ended_at, Some(2000));
    }

    #[test]
    fn output_tail_starts_on_a_line() {
        let text = (0..20_000).map(|index| format!("line {index}\n")).collect::<String>();
        let (tail, truncated) = tail(&text);
        assert!(truncated);
        assert!(tail.starts_with("line "));
        assert!(tail.len() <= OUTPUT_TAIL);
    }
}
