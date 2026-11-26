//! Bind existing chat conversations into a persistent Plan/Execute workflow without creating new ones.

use std::collections::{HashMap, HashSet};

use crate::models::NodeKind;

use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindRequest {
    pub request_id: String,
    pub planner_id: String,
    #[serde(default)]
    pub executor_ids: Vec<String>,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub images: Vec<ChatImage>,
    #[serde(default)]
    pub reparent_executors: bool,
    #[serde(default)]
    pub config: Config,
}

#[derive(Clone)]
struct ExistingActiveRun {
    run: Run,
    task_count: usize,
    has_pending_proposal: bool,
}

impl ExistingActiveRun {
    fn is_pending_empty_single(&self) -> bool {
        !self.run.config.split_tasks
            && self.run.executor_id.is_none()
            && self.run.round == 0
            && matches!(self.run.state.as_str(), "planning" | "blocked")
    }

    fn is_extendable_split(&self) -> bool {
        self.run.config.split_tasks
            && !self.has_pending_proposal
            && self.task_count < split::MAX_TASKS
    }

    fn extendable(&self) -> bool {
        self.is_pending_empty_single() || self.is_extendable_split()
    }

    fn remaining_slots(&self) -> usize {
        if self.is_pending_empty_single() {
            split::MAX_TASKS
        } else if self.is_extendable_split() {
            split::MAX_TASKS.saturating_sub(self.task_count)
        } else {
            0
        }
    }

    fn summary_json(&self) -> Value {
        json!({
            "id": self.run.id,
            "state": self.run.state,
            "round": self.run.round,
            "splitTasks": self.run.config.split_tasks,
            "taskCount": self.task_count,
            "remainingSlots": self.remaining_slots(),
            "reviewEnabled": self.run.config.review_enabled,
            "reviewStatus": self.run.review_status,
            "config": self.run.config,
        })
    }
}

#[derive(Clone)]
struct SessionWorkflowStatus {
    eligible: bool,
    ineligible_reason: Option<&'static str>,
    active_role: Option<&'static str>,
    extendable_planner: bool,
    existing_run: Option<ExistingActiveRun>,
}

fn check_session_chat_eligibility(session: &Session) -> Result<(), &'static str> {
    if !supported(session.kind) {
        return Err("unsupported_agent");
    }
    if session.engine != "chat" {
        return Err("not_chat");
    }
    Ok(())
}

fn eligibility_error(reason: &str) -> String {
    match reason {
        "unsupported_agent" => "Planning and execution require chat-capable agents".into(),
        "not_chat" => "Only conversation-view (chat) sessions can be bound to a workflow".into(),
        _ => "This session cannot be bound to a workflow".into(),
    }
}

fn load_active_root_runs_for_planner(
    conn: &rusqlite::Connection,
    planner_id: &str,
) -> Result<Vec<ExistingActiveRun>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT r.id, r.owner_id, r.planner_id, r.executor_id, r.config, r.task, r.state, r.round, r.summary, r.reviewer_id, r.review_status \
             FROM plan_execute_runs r \
             WHERE r.planner_id = ?1 \
               AND r.state NOT IN ('completed', 'stopped') \
               AND NOT EXISTS (SELECT 1 FROM plan_execute_tasks t WHERE t.run_id = r.id) \
             ORDER BY r.rowid",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([planner_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, u32>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, String>(10)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    let mut result = Vec::with_capacity(rows.len());
    for (id, owner_id, planner_id, executor_id, config_str, task, state, round, summary, reviewer_id, review_status) in rows {
        let config: Config = serde_json::from_str(&config_str).map_err(|e| e.to_string())?;
        let task_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM plan_execute_tasks WHERE parent_id = ?1",
                [&id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let has_pending_proposal: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM plan_execute_proposals WHERE run_id = ?1 AND state = 'pending')",
                [&id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        result.push(ExistingActiveRun {
            run: Run {
                id,
                owner_id,
                planner_id,
                executor_id,
                reviewer_id,
                review_status,
                config,
                task,
                state,
                round,
                summary,
            },
            task_count: task_count.max(0) as usize,
            has_pending_proposal,
        });
    }
    Ok(result)
}

fn inspect_session_status(
    conn: &rusqlite::Connection,
    session: &Session,
) -> Result<SessionWorkflowStatus, String> {
    let (eligible, ineligible_reason) = match check_session_chat_eligibility(session) {
        Ok(()) => (true, None),
        Err(reason) => (false, Some(reason)),
    };

    let is_executor: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM plan_execute_runs WHERE executor_id = ?1 AND state NOT IN ('completed', 'stopped'))",
            [&session.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let is_reviewer: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM plan_execute_runs WHERE reviewer_id = ?1 AND state NOT IN ('completed', 'stopped'))",
            [&session.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let is_planner: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM plan_execute_runs WHERE planner_id = ?1 AND state NOT IN ('completed', 'stopped'))",
            [&session.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let active_role = if is_executor {
        Some("executor")
    } else if is_reviewer {
        Some("reviewer")
    } else if is_planner {
        Some("planner")
    } else {
        None
    };

    let (extendable_planner, existing_run) = if eligible && !is_executor && !is_reviewer && is_planner {
        let roots = load_active_root_runs_for_planner(conn, &session.id)?;
        if roots.len() == 1 {
            let root = roots.into_iter().next().unwrap();
            let ext = root.extendable();
            (ext, Some(root))
        } else {
            (false, None)
        }
    } else {
        (false, None)
    };

    Ok(SessionWorkflowStatus {
        eligible,
        ineligible_reason,
        active_role,
        extendable_planner,
        existing_run,
    })
}

fn is_ancestor(
    parent_map: &HashMap<String, Option<String>>,
    ancestor_id: &str,
    descendant_id: &str,
) -> bool {
    let mut cur = parent_map.get(descendant_id).and_then(|p| p.as_deref());
    let mut hops = 0usize;
    while let Some(id) = cur {
        if id == ancestor_id {
            return true;
        }
        hops += 1;
        if hops > 256 {
            break;
        }
        cur = parent_map.get(id).and_then(|p| p.as_deref());
    }
    false
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "Execute".into();
    }
    trimmed.chars().take(max_chars).collect()
}

fn resolve_session_role(app: &AppCtx, session: &Session) -> Result<RoleConfig, String> {
    let sel = super::super::session_settings::resolve(app, session)?;
    Ok(RoleConfig {
        agent: Some(session.kind),
        model: sel.model,
        effort: sel.effort,
    })
}

/// Inspect the selected session(s) and their project candidates to seed the binding dialog.
/// When `mode == Some("executors")`, every eligible selected session is pre-assigned as an executor
/// and the command session is inferred from remaining candidates (common ancestor or unique extendable
/// active command session), or left empty for the user to pick.
pub fn prepare(app: &AppCtx, session_ids: &[String], mode: Option<&str>) -> Result<Value, String> {
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for id in session_ids {
        let trimmed = id.trim();
        if !trimmed.is_empty() && seen.insert(trimmed.to_owned()) {
            deduped.push(trimmed.to_owned());
        }
    }
    if deduped.is_empty() {
        return Err("Select at least one session".into());
    }

    let (selected_sessions, all_project_sessions, status_map) = {
        let conn = app.db().conn.lock().unwrap();
        let mut selected = Vec::with_capacity(deduped.len());
        let mut project_ids = Vec::new();
        let mut seen_projects = HashSet::new();
        for id in &deduped {
            let s = repo::get_session(&conn, id)?
                .filter(|s| s.archived_at.is_none())
                .ok_or_else(|| "The workflow session is missing or archived".to_owned())?;
            if seen_projects.insert(s.project_id.clone()) {
                project_ids.push(s.project_id.clone());
            }
            selected.push(s);
        }

        let tree = repo::list_tree(&conn)?;
        let project_set: HashSet<&str> = project_ids.iter().map(String::as_str).collect();
        let mut candidates_raw: Vec<Session> = tree
            .sessions
            .into_iter()
            .filter(|s| s.archived_at.is_none() && project_set.contains(s.project_id.as_str()))
            .collect();
        // Order sessions by project appearance order, then existing tree sort order.
        let project_order: HashMap<&str, usize> = project_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        candidates_raw.sort_by_key(|s| {
            (
                project_order.get(s.project_id.as_str()).copied().unwrap_or(usize::MAX),
                s.sort_order,
                s.created_at,
            )
        });

        let mut statuses = HashMap::with_capacity(candidates_raw.len());
        for s in &candidates_raw {
            statuses.insert(s.id.clone(), inspect_session_status(&conn, s)?);
        }
        (selected, candidates_raw, statuses)
    };

    let parent_map: HashMap<String, Option<String>> = all_project_sessions
        .iter()
        .map(|s| (s.id.clone(), s.parent_session_id.clone()))
        .collect();

    let can_be_plan = |id: &str| {
        status_map
            .get(id)
            .is_some_and(|st| st.eligible && (st.active_role.is_none() || st.extendable_planner))
    };
    let can_be_exec = |id: &str| {
        status_map
            .get(id)
            .is_some_and(|st| st.eligible && st.active_role.is_none())
    };

    let (planner_id, executor_ids) = if mode == Some("executors") {
        let selected_set: HashSet<&str> = selected_sessions.iter().map(|s| s.id.as_str()).collect();
        let mut execs = Vec::new();
        for s in &selected_sessions {
            if execs.len() >= split::MAX_TASKS {
                break;
            }
            if can_be_exec(&s.id) {
                execs.push(s.id.clone());
            }
        }
        let other_plan_capable: Vec<&Session> = all_project_sessions
            .iter()
            .filter(|s| !selected_set.contains(s.id.as_str()) && can_be_plan(&s.id))
            .collect();
        let ancestor_matches: Vec<&Session> = if !execs.is_empty() {
            other_plan_capable
                .iter()
                .copied()
                .filter(|cand| execs.iter().all(|eid| is_ancestor(&parent_map, &cand.id, eid)))
                .collect()
        } else {
            vec![]
        };
        let extendable_matches: Vec<&Session> = other_plan_capable
            .iter()
            .copied()
            .filter(|s| status_map[&s.id].extendable_planner)
            .collect();

        let chosen_planner = if ancestor_matches.len() == 1 {
            ancestor_matches[0].id.clone()
        } else if extendable_matches.len() == 1 {
            extendable_matches[0].id.clone()
        } else {
            String::new()
        };
        (chosen_planner, execs)
    } else if selected_sessions.len() == 1 {
        let p = &selected_sessions[0];
        let p_status = &status_map[&p.id];
        let mut execs = Vec::new();
        if p_status.eligible && (p_status.active_role.is_none() || p_status.extendable_planner) {
            let cap = p_status
                .existing_run
                .as_ref()
                .map(|r| r.remaining_slots())
                .unwrap_or(split::MAX_TASKS);
            for child in &all_project_sessions {
                if execs.len() >= cap {
                    break;
                }
                if child.parent_session_id.as_deref() == Some(p.id.as_str()) && can_be_exec(&child.id) {
                    execs.push(child.id.clone());
                }
            }
        }
        (p.id.clone(), execs)
    } else {
        let plan_capable: Vec<&Session> = selected_sessions
            .iter()
            .filter(|s| can_be_plan(&s.id))
            .collect();
        let eligible_selected: Vec<&Session> = selected_sessions
            .iter()
            .filter(|s| status_map.get(&s.id).is_some_and(|st| st.eligible))
            .collect();

        // 1. Unique extendable active planner among selected sessions.
        let extendable_matches: Vec<&Session> = plan_capable
            .iter()
            .copied()
            .filter(|s| status_map[&s.id].extendable_planner)
            .collect();

        // 2. Unique ancestor of all other eligible selected sessions.
        let ancestor_matches: Vec<&Session> = if eligible_selected.len() >= 2 {
            plan_capable
                .iter()
                .copied()
                .filter(|cand| {
                    eligible_selected
                        .iter()
                        .all(|other| other.id == cand.id || is_ancestor(&parent_map, &cand.id, &other.id))
                })
                .collect()
        } else {
            vec![]
        };

        let chosen_planner = if extendable_matches.len() == 1 {
            extendable_matches[0].id.clone()
        } else if ancestor_matches.len() == 1 {
            ancestor_matches[0].id.clone()
        } else if let Some(first) = plan_capable.first() {
            first.id.clone()
        } else {
            selected_sessions[0].id.clone()
        };

        let cap = status_map
            .get(&chosen_planner)
            .and_then(|st| st.existing_run.as_ref())
            .map(|r| r.remaining_slots())
            .unwrap_or(split::MAX_TASKS);

        let mut execs = Vec::new();
        for s in &selected_sessions {
            if execs.len() >= cap {
                break;
            }
            if s.id != chosen_planner && can_be_exec(&s.id) {
                execs.push(s.id.clone());
            }
        }
        (chosen_planner, execs)
    };

    let planner_status = status_map.get(&planner_id);
    let default_config = if let Some(existing) = planner_status.and_then(|st| st.existing_run.as_ref()) {
        json!(existing.run.config)
    } else if planner_status.is_some_and(|st| st.eligible) {
        super::defaults(
            app,
            &planner_id,
            &Config {
                worktree_mode: Some(WorktreeMode::None),
                ..Default::default()
            },
        )?
    } else {
        let fallback_agent = selected_sessions
            .iter()
            .find(|s| supported(s.kind))
            .map(|s| s.kind)
            .unwrap_or(SessionKind::Claude);
        json!(Config {
            review_enabled: Some(false),
            worktree_mode: Some(WorktreeMode::None),
            plan: RoleConfig {
                agent: Some(fallback_agent),
                model: None,
                effort: None,
            },
            exec: RoleConfig {
                agent: Some(fallback_agent),
                model: None,
                effort: None,
            },
            review: RoleConfig {
                agent: Some(fallback_agent),
                model: None,
                effort: None,
            },
            split_tasks: false,
        })
    };

    let candidates: Vec<Value> = all_project_sessions
        .iter()
        .map(|s| {
            let st = &status_map[&s.id];
            json!({
                "id": s.id,
                "projectId": s.project_id,
                "groupId": s.group_id,
                "parentSessionId": s.parent_session_id,
                "name": s.name,
                "kind": s.kind,
                "engine": s.engine,
                "cwd": s.cwd,
                "worktreePath": s.worktree_path,
                "eligible": st.eligible,
                "ineligibleReason": st.ineligible_reason,
                "activeRole": st.active_role,
                "extendablePlanner": st.extendable_planner,
                "existingRun": st.existing_run.as_ref().map(ExistingActiveRun::summary_json),
            })
        })
        .collect();

    Ok(json!({
        "plannerId": planner_id,
        "executorIds": executor_ids,
        "candidates": candidates,
        "defaultConfig": default_config,
    }))
}

fn existing_receipt(app: &AppCtx, run: &Run, target: &str, msg_id: &str) -> Option<Value> {
    let saved: Option<String> = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT outcome FROM chat_submissions WHERE session_id = ?1 AND id = ?2",
            params![target, msg_id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten()
        .flatten();
    let outcome: Result<String, String> = serde_json::from_str(saved.as_deref()?).ok()?;
    let outcome = outcome.ok()?;
    Some(json!({
        "delivery": outcome,
        "messageId": msg_id,
        "targetSessionId": target,
        "run": brief(run),
    }))
}

fn deliver_binding_message(
    app: &AppCtx,
    run_id: &str,
    msg_id: &str,
    restore_state_on_success: &str,
) -> Result<Value, String> {
    let run = get(app, run_id)?;
    if let Some(saved) = existing_receipt(app, &run, &run.planner_id, msg_id) {
        if saved["delivery"] == "sent" || app.chat().is_alive(&run.planner_id) {
            return Ok(saved);
        }
    }
    let wire: String = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT wire FROM plan_execute_messages WHERE id = ?1",
            [msg_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    match deliver(app, &run, &run.planner_id, &wire, msg_id) {
        Ok(delivery) => {
            if run.state == "blocked" {
                app.db()
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE plan_execute_runs SET state = ?2, summary = '' WHERE id = ?1 AND state = 'blocked'",
                        params![run.id, restore_state_on_success],
                    )
                    .map_err(|e| e.to_string())?;
                if run.config.split_tasks {
                    split::refresh_parent(app, &run.id)?;
                }
            }
            sync_session_states(app);
            Ok(delivery)
        }
        Err(error) => {
            app.db()
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE plan_execute_runs SET state = 'blocked', summary = ?2 WHERE id = ?1",
                    params![run.id, error],
                )
                .map_err(|e| e.to_string())?;
            sync_session_states(app);
            Ok(json!({
                "delivery": "blocked",
                "messageId": msg_id,
                "targetSessionId": run.planner_id,
                "error": error,
            }))
        }
    }
}

fn reparent_in_tx(
    tx: &rusqlite::Transaction<'_>,
    planner: &Session,
    executors: &[Session],
) -> Result<bool, String> {
    let same_project: Vec<&Session> = executors
        .iter()
        .filter(|e| e.project_id == planner.project_id)
        .collect();
    if same_project.is_empty() {
        return Ok(false);
    }

    let target_ids: HashSet<&str> = same_project.iter().map(|e| e.id.as_str()).collect();
    let mut changed = false;

    // Walk up from planner to detect whether any selected executor is currently an ancestor of planner.
    // Lifting planner to the highest selected ancestor's position prevents cycles cleanly.
    let mut highest_selected_ancestor: Option<Session> = None;
    let mut cur_parent = planner.parent_session_id.clone();
    let mut hops = 0usize;
    while let Some(pid) = cur_parent {
        hops += 1;
        if hops > 256 {
            break;
        }
        let Some(anc) = repo::get_session(tx, &pid)? else {
            break;
        };
        if target_ids.contains(anc.id.as_str()) {
            highest_selected_ancestor = Some(anc.clone());
        }
        cur_parent = anc.parent_session_id;
    }

    let mut planner_group = planner.group_id.clone();
    if let Some(top) = highest_selected_ancestor {
        repo::move_node(
            tx,
            NodeKind::Session,
            &planner.id,
            Some(&planner.project_id),
            top.group_id.as_deref(),
            top.parent_session_id.as_deref(),
            top.sort_order,
        )?;
        planner_group = top.group_id;
        changed = true;
    }

    let max_order: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) FROM sessions WHERE project_id = ?1 AND parent_session_id = ?2",
            params![planner.project_id, planner.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let mut next_order = max_order.saturating_add(1);

    for exec in same_project {
        let current = repo::get_session(tx, &exec.id)?
            .ok_or_else(|| "The workflow session is missing or archived".to_owned())?;
        if current.parent_session_id.as_deref() == Some(planner.id.as_str())
            && current.group_id == planner_group
        {
            continue;
        }
        repo::move_node(
            tx,
            NodeKind::Session,
            &exec.id,
            Some(&planner.project_id),
            planner_group.as_deref(),
            Some(&planner.id),
            next_order,
        )?;
        next_order = next_order.saturating_add(1);
        changed = true;
    }

    Ok(changed)
}

fn format_bound_executor_lines(entries: &[(Option<String>, &Session)], single_run_id: &str) -> String {
    entries
        .iter()
        .enumerate()
        .map(|(idx, (task_run_id, exec))| {
            let cwd = exec.cwd.as_deref().unwrap_or("");
            if let Some(tid) = task_run_id {
                format!(
                    "{}. {} (sessionId: {}, kind: {}, cwd: {})\n   Task Workflow ID: {}\n   Dispatch round 1: vflow dispatch {} --round 1 --message-id msg-UUID < task.txt",
                    idx + 1,
                    exec.name,
                    exec.id,
                    exec.kind.as_str(),
                    cwd,
                    tid,
                    tid
                )
            } else {
                format!(
                    "{}. {} (sessionId: {}, kind: {}, cwd: {})\n   Dispatch round 1: vflow dispatch {} --round 1 --message-id msg-UUID < task.txt",
                    idx + 1,
                    exec.name,
                    exec.id,
                    exec.kind.as_str(),
                    cwd,
                    single_run_id
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Bind existing sessions into a new workflow (Mode A) or append new executors to an extendable active planner (Mode B).
pub fn apply(app: &AppCtx, req: &BindRequest) -> Result<Value, String> {
    let _guard = operation_lock().lock().unwrap();
    uuid::Uuid::parse_str(&req.request_id).map_err(|_| "Invalid workflow request ID")?;
    core::check_images(&req.images)?;
    if req.prompt.len() > 65536 {
        return Err("Task prompt exceeds 65536 bytes".into());
    }

    let mut executor_ids = Vec::new();
    let mut seen_exec = HashSet::new();
    for id in &req.executor_ids {
        let trimmed = id.trim();
        if !trimmed.is_empty() && seen_exec.insert(trimmed.to_owned()) {
            executor_ids.push(trimmed.to_owned());
        }
    }
    if executor_ids.iter().any(|id| id == &req.planner_id) {
        return Err("The planning session cannot also be selected as an execution session".into());
    }
    if executor_ids.len() > split::MAX_TASKS {
        return Err(format!(
            "Select at most {} execution sessions",
            split::MAX_TASKS
        ));
    }

    let request_snapshot = json!({
        "plannerId": req.planner_id,
        "executorIds": executor_ids,
        "prompt": req.prompt,
        "images": req.images,
        "reparentExecutors": req.reparent_executors,
        "config": req.config,
    });

    // Idempotency replay by request_id.
    let saved_binding: Option<(String, String)> = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT run_id, request FROM plan_execute_bindings WHERE id = ?1",
            [&req.request_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some((saved_run_id, saved_req_str)) = saved_binding {
        if serde_json::from_str::<Value>(&saved_req_str).ok().as_ref() != Some(&request_snapshot) {
            return Err("This request ID already has a different binding configuration".into());
        }
        let existing = get(app, &saved_run_id)?;
        let msg_id = format!("msg-{}", req.request_id);
        let restore_state = if existing.config.split_tasks || existing.round > 0 {
            "executing"
        } else {
            "planning"
        };
        let delivery = deliver_binding_message(app, &existing.id, &msg_id, restore_state)?;
        let updated = get(app, &existing.id)?;
        return Ok(json!({
            "planner": session(app, &updated.planner_id)?,
            "run": brief(&updated),
            "tasks": split::tasks(app, &updated.id)?,
            "delivery": delivery,
        }));
    }

    // Prevent colliding with an existing non-binding workflow ID.
    if get(app, &req.request_id).is_ok() {
        return Err("This request ID already belongs to another workflow".into());
    }

    let planner = session(app, &req.planner_id)?;
    if let Err(reason) = check_session_chat_eligibility(&planner) {
        return Err(format!("Planner: {}", eligibility_error(reason)));
    }

    let mut executors = Vec::with_capacity(executor_ids.len());
    for eid in &executor_ids {
        let exec = session(app, eid)?;
        if let Err(reason) = check_session_chat_eligibility(&exec) {
            return Err(format!("Executor '{}': {}", exec.name, eligibility_error(reason)));
        }
        executors.push(exec);
    }

    // Inspect active workflow roles under the operation lock to guarantee uniqueness.
    let (planner_status, exec_conflicts) = {
        let conn = app.db().conn.lock().unwrap();
        let p_status = inspect_session_status(&conn, &planner)?;
        let mut conflicts = Vec::new();
        for exec in &executors {
            let st = inspect_session_status(&conn, exec)?;
            if let Some(role) = st.active_role {
                conflicts.push((exec.name.clone(), role));
            }
        }
        (p_status, conflicts)
    };

    if let Some((name, role)) = exec_conflicts.first() {
        return Err(format!(
            "Execution session '{name}' already belongs to an active workflow as {role}"
        ));
    }

    if matches!(planner_status.active_role, Some("executor") | Some("reviewer")) {
        return Err(
            "The selected planning session is already an executor or reviewer in an active workflow"
                .into(),
        );
    }

    let mode_b_run: Option<ExistingActiveRun> = if planner_status.active_role == Some("planner") {
        let Some(existing) = planner_status.existing_run.filter(|r| r.extendable()) else {
            return Err(
                "The selected planning session already has an active workflow that cannot accept additional execution sessions"
                    .into(),
            );
        };
        if executors.is_empty() {
            return Err(
                "Select at least one execution session to add to this active workflow".into(),
            );
        }
        if executors.len() > existing.remaining_slots() {
            return Err(format!(
                "At most {} execution sessions can be added to this workflow (limit: {})",
                existing.remaining_slots(),
                split::MAX_TASKS
            ));
        }
        Some(existing)
    } else {
        None
    };

    // Resolve per-executor RoleConfig before entering the SQLite transaction.
    let mut exec_roles = Vec::with_capacity(executors.len());
    for exec in &executors {
        exec_roles.push(resolve_session_role(app, exec)?);
    }

    let prompt_trimmed = req.prompt.trim();
    let task_summary = if prompt_trimmed.is_empty() {
        format!("Bound workflow from session: {}", planner.name)
    } else {
        req.prompt.clone()
    };
    let message_id = format!("msg-{}", req.request_id);

    let (run_id, restore_state, tree_changed) = if let Some(existing_info) = mode_b_run {
        // Mode B: Append new execution sessions to an extendable active planner workflow.
        let mut parent_run = existing_info.run.clone();
        let parent_images = if !req.images.is_empty() {
            req.images.clone()
        } else {
            message_images(app, &format!("msg-{}", parent_run.id))?
        };
        let mut bound_entries: Vec<(Option<String>, &Session)> = Vec::new();
        let mut tree_changed = false;

        {
            let mut conn = app.db().conn.lock().unwrap();
            let tx = conn.transaction().map_err(|e| e.to_string())?;

            if existing_info.is_pending_empty_single() && executors.len() == 1 {
                let exec = &executors[0];
                let mut next_config = parent_run.config.clone();
                next_config.exec = exec_roles[0].clone();
                next_config.worktree_mode = Some(WorktreeMode::None);
                validate_config(&next_config)?;
                tx.execute(
                    "UPDATE plan_execute_runs SET executor_id = ?2, config = ?3 WHERE id = ?1",
                    params![
                        parent_run.id,
                        exec.id,
                        serde_json::to_string(&next_config).map_err(|e| e.to_string())?
                    ],
                )
                .map_err(|e| e.to_string())?;
                parent_run.executor_id = Some(exec.id.clone());
                parent_run.config = next_config;
                bound_entries.push((None, exec));
            } else {
                // Either upgrading an empty single-task run with >= 2 executors, or appending to a split run.
                let mut next_config = parent_run.config.clone();
                next_config.split_tasks = true;
                next_config.worktree_mode = Some(WorktreeMode::None);
                let next_round = parent_run.round.max(1);
                tx.execute(
                    "UPDATE plan_execute_runs SET executor_id = NULL, config = ?2, state = 'executing', round = ?3 WHERE id = ?1",
                    params![
                        parent_run.id,
                        serde_json::to_string(&next_config).map_err(|e| e.to_string())?,
                        next_round
                    ],
                )
                .map_err(|e| e.to_string())?;
                parent_run.config = next_config.clone();
                parent_run.round = next_round;
                parent_run.state = "executing".into();

                let start_pos: i64 = tx
                    .query_row(
                        "SELECT COALESCE(MAX(position) + 1, 0) FROM plan_execute_tasks WHERE parent_id = ?1",
                        [&parent_run.id],
                        |r| r.get(0),
                    )
                    .map_err(|e| e.to_string())?;

                for (idx, (exec, role_cfg)) in executors.iter().zip(exec_roles.iter()).enumerate() {
                    let task_run_id = uuid::Uuid::new_v4().to_string();
                    let dispatch_id = format!("msg-{}", uuid::Uuid::new_v4());
                    let task_name = truncate_chars(&exec.name, 80);
                    let task_config = Config {
                        plan: next_config.plan.clone(),
                        exec: role_cfg.clone(),
                        split_tasks: false,
                        ..next_config.clone()
                    };
                    validate_config(&task_config)?;
                    tx.execute(
                        "INSERT INTO plan_execute_runs(id, owner_id, planner_id, executor_id, reviewer_id, review_status, config, task, state, round) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'planning', 0)",
                        params![
                            task_run_id,
                            parent_run.planner_id,
                            parent_run.planner_id,
                            exec.id,
                            parent_run.reviewer_id,
                            task_config.initial_review_status(),
                            serde_json::to_string(&task_config).map_err(|e| e.to_string())?,
                            task_summary,
                        ],
                    )
                    .map_err(|e| e.to_string())?;
                    tx.execute(
                        "INSERT INTO plan_execute_tasks(parent_id, run_id, position, name, dispatch_id, launch_pending) \
                         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                        params![
                            parent_run.id,
                            task_run_id,
                            start_pos + idx as i64,
                            task_name,
                            dispatch_id,
                        ],
                    )
                    .map_err(|e| e.to_string())?;
                    let child_start_id = format!("msg-{task_run_id}");
                    tx.execute(
                        "INSERT INTO plan_execute_messages(id, run_id, sender_id, target_id, action, round, fingerprint, wire, origin) \
                         VALUES (?1, ?2, ?3, ?3, 'start', 0, 'bind', ?4, '{\"role\":\"user\"}')",
                        params![child_start_id, task_run_id, parent_run.planner_id, task_summary],
                    )
                    .map_err(|e| e.to_string())?;
                    save_images(&tx, &child_start_id, &parent_images)?;
                    bound_entries.push((Some(task_run_id), exec));
                }
            }

            if req.reparent_executors {
                tree_changed = reparent_in_tx(&tx, &planner, &executors)?;
            }

            let list_text = format_bound_executor_lines(&bound_entries, &parent_run.id);
            let extra_prompt = if prompt_trimmed.is_empty() {
                "Prepare assignments based on the current conversation context and run vflow dispatch for each newly bound execution session."
                    .to_owned()
            } else {
                format!("Additional user instructions:\n{}", req.prompt)
            };
            let body = format!(
                "New existing execution session(s) have been bound to this workflow.\nWorkflow ID: {}\nRound: {}\n\nNewly bound execution sessions:\n{}\n\n{}",
                parent_run.id, parent_run.round, list_text, extra_prompt
            );
            let origin = json!({
                "name": "VelaTerm",
                "agent": "terminal",
                "role": "system",
                "runId": parent_run.id,
                "round": parent_run.round,
            });
            let wire = format!("[VelaTerm message {message_id}]\n{origin}\n\n{body}")
                .trim_end()
                .to_owned();
            // Use sender_id = "system" so self-owned planner runs deliver the notice to the planner.
            tx.execute(
                "INSERT INTO plan_execute_messages(id, run_id, sender_id, target_id, action, round, fingerprint, wire, origin) \
                 VALUES (?1, ?2, 'system', ?3, 'notice', ?4, 'bind', ?5, ?6)",
                params![
                    message_id,
                    parent_run.id,
                    parent_run.planner_id,
                    parent_run.round,
                    wire,
                    origin.to_string(),
                ],
            )
            .map_err(|e| e.to_string())?;
            save_images(&tx, &message_id, &req.images)?;
            tx.execute(
                "INSERT INTO plan_execute_bindings(id, run_id, request) VALUES (?1, ?2, ?3)",
                params![req.request_id, parent_run.id, request_snapshot.to_string()],
            )
            .map_err(|e| e.to_string())?;

            tx.commit().map_err(|e| e.to_string())?;
        }

        if parent_run.config.split_tasks {
            split::refresh_parent(app, &parent_run.id)?;
        }
        let restore = if parent_run.config.split_tasks {
            "executing"
        } else {
            "planning"
        };
        (parent_run.id, restore, tree_changed)
    } else {
        // Mode A: Create a new workflow bound to existing planner and 0..=12 existing executors.
        let plan_role = resolve_session_role(app, &planner)?;
        let exec_role = if let Some(first_exec) = exec_roles.first() {
            first_exec.clone()
        } else {
            let mut requested_exec = req.config.exec.clone();
            if requested_exec.agent.is_none() {
                requested_exec.agent = plan_role.agent;
            }
            normalize(app, &requested_exec, &planner)?
        };
        let review_enabled = Some(req.config.review_enabled.unwrap_or(false));
        let review_role = if review_enabled == Some(true) {
            let mut requested_review = req.config.review.clone();
            if requested_review.agent.is_none() {
                requested_review.agent = plan_role.agent;
            }
            normalize(app, &requested_review, &planner)?
        } else {
            RoleConfig::default()
        };

        let (worktree_mode, split_tasks) = if executors.is_empty() {
            if req.config.worktree_mode == Some(WorktreeMode::Shared) {
                return Err(
                    "Shared worktree mode is only available when creating a new planning session"
                        .into(),
                );
            }
            (
                Some(req.config.worktree_mode.unwrap_or(WorktreeMode::None)),
                req.config.split_tasks,
            )
        } else if executors.len() == 1 {
            (Some(WorktreeMode::None), false)
        } else {
            (Some(WorktreeMode::None), true)
        };

        let config = Config {
            review_enabled,
            review: review_role,
            worktree_mode,
            split_tasks,
            plan: plan_role,
            exec: exec_role,
        };
        validate_config(&config)?;

        let run_id = req.request_id.clone();
        let mut tree_changed = false;

        {
            let mut conn = app.db().conn.lock().unwrap();
            let tx = conn.transaction().map_err(|e| e.to_string())?;

            let mut bound_entries: Vec<(Option<String>, &Session)> = Vec::new();
            let (root_executor_id, root_state, root_round) = if executors.len() == 1 {
                bound_entries.push((None, &executors[0]));
                (Some(executors[0].id.clone()), "planning", 0u32)
            } else if executors.len() >= 2 {
                (None, "executing", 1u32)
            } else {
                (None, "planning", 0u32)
            };

            tx.execute(
                "INSERT INTO plan_execute_runs(id, owner_id, planner_id, executor_id, review_status, config, task, state, round) \
                 VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    run_id,
                    planner.id,
                    root_executor_id,
                    config.initial_review_status(),
                    serde_json::to_string(&config).map_err(|e| e.to_string())?,
                    task_summary,
                    root_state,
                    root_round,
                ],
            )
            .map_err(|e| e.to_string())?;

            if executors.len() >= 2 {
                for (idx, (exec, role_cfg)) in executors.iter().zip(exec_roles.iter()).enumerate() {
                    let task_run_id = uuid::Uuid::new_v4().to_string();
                    let dispatch_id = format!("msg-{}", uuid::Uuid::new_v4());
                    let task_name = truncate_chars(&exec.name, 80);
                    let task_config = Config {
                        plan: config.plan.clone(),
                        exec: role_cfg.clone(),
                        split_tasks: false,
                        ..config.clone()
                    };
                    validate_config(&task_config)?;
                    tx.execute(
                        "INSERT INTO plan_execute_runs(id, owner_id, planner_id, executor_id, review_status, config, task, state, round) \
                         VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, 'planning', 0)",
                        params![
                            task_run_id,
                            planner.id,
                            exec.id,
                            task_config.initial_review_status(),
                            serde_json::to_string(&task_config).map_err(|e| e.to_string())?,
                            task_summary,
                        ],
                    )
                    .map_err(|e| e.to_string())?;
                    tx.execute(
                        "INSERT INTO plan_execute_tasks(parent_id, run_id, position, name, dispatch_id, launch_pending) \
                         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                        params![run_id, task_run_id, idx as i64, task_name, dispatch_id],
                    )
                    .map_err(|e| e.to_string())?;
                    let child_start_id = format!("msg-{task_run_id}");
                    tx.execute(
                        "INSERT INTO plan_execute_messages(id, run_id, sender_id, target_id, action, round, fingerprint, wire, origin) \
                         VALUES (?1, ?2, ?3, ?3, 'start', 0, 'bind', ?4, '{\"role\":\"user\"}')",
                        params![child_start_id, task_run_id, planner.id, task_summary],
                    )
                    .map_err(|e| e.to_string())?;
                    save_images(&tx, &child_start_id, &req.images)?;
                    bound_entries.push((Some(task_run_id), exec));
                }
            }

            if req.reparent_executors {
                tree_changed = reparent_in_tx(&tx, &planner, &executors)?;
            }

            let directory = planner.cwd.as_deref().unwrap_or("");
            let bound_section = if bound_entries.is_empty() {
                if config.split_tasks {
                    "No existing execution sessions were bound. Use vflow propose when your task plan is ready so the user can confirm and launch execution sessions."
                        .to_owned()
                } else {
                    "No existing execution sessions were bound. When your plan is ready, run vflow dispatch to create and assign the execution session."
                        .to_owned()
                }
            } else {
                format!(
                    "Bound existing execution sessions:\n{}",
                    format_bound_executor_lines(&bound_entries, &run_id)
                )
            };
            let task_section = if prompt_trimmed.is_empty() {
                "No extra task prompt was entered. Organize the plan from this conversation's existing context and dispatch implementation instructions to the bound execution session(s) using vflow dispatch."
                    .to_owned()
            } else {
                format!("User task:\n{}", req.prompt)
            };
            let task_body = format!(
                "{}\n\nWorkflow ID: {}\nRole: planner\nWorking directory: {}\nThis existing conversation was bound as the planning session. Present the final delivery summary here.\n\n{}\n\n{}",
                review::protocol(&config),
                run_id,
                directory,
                bound_section,
                task_section,
            );
            let origin = json!({"role": "user"});
            let wire = format!("[VelaTerm message {message_id}]\n{origin}\n\n{task_body}")
                .trim_end()
                .to_owned();

            tx.execute(
                "INSERT INTO plan_execute_messages(id, run_id, sender_id, target_id, action, round, fingerprint, wire, origin) \
                 VALUES (?1, ?2, ?3, ?3, 'start', 0, 'bind', ?4, ?5)",
                params![message_id, run_id, planner.id, wire, origin.to_string()],
            )
            .map_err(|e| e.to_string())?;
            save_images(&tx, &message_id, &req.images)?;
            tx.execute(
                "INSERT INTO plan_execute_bindings(id, run_id, request) VALUES (?1, ?2, ?3)",
                params![req.request_id, run_id, request_snapshot.to_string()],
            )
            .map_err(|e| e.to_string())?;

            tx.commit().map_err(|e| e.to_string())?;
        }

        let restore = if executors.len() >= 2 {
            "executing"
        } else {
            "planning"
        };
        (run_id, restore, tree_changed)
    };

    if tree_changed {
        app.emit(crate::host::TREE_CHANGED, ());
    }
    sync_session_states(app);
    let delivery = deliver_binding_message(app, &run_id, &message_id, restore_state)?;
    let updated = get(app, &run_id)?;
    Ok(json!({
        "planner": session(app, &updated.planner_id)?,
        "run": brief(&updated),
        "tasks": split::tasks(app, &updated.id)?,
        "delivery": delivery,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app(tag: &str) -> AppCtx {
        let dir = std::env::temp_dir().join(format!("vlx-pe-bind-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("db.sqlite")).unwrap();
        let app = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(dir, db)));
        match &app {
            AppCtx::Headless(host) => host.set_hooks(crate::agent::server::HookServer {
                port: 0,
                token: "bind-test".into(),
            }),
            #[cfg(feature = "gui")]
            _ => unreachable!(),
        }
        {
            let conn = app.db().conn.lock().unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO projects(id, name, root_path, sort_order, created_at) VALUES ('proj', 'Test Project', '/tmp', 0, 1)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO app_settings(key, value, updated_at) VALUES ('vlx-settings', ?1, 0)",
                [json!({"agentDefaults":{"claude":{"path":"/nonexistent/velaterm-bind-test"}}}).to_string()],
            )
            .unwrap();
        }
        app
    }

    fn cleanup(app: AppCtx) {
        crate::agent::chat::engine::stop_all_test_peers(&app);
        if let Ok(dir) = app.data_dir() {
            drop(app);
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    fn seed(
        app: &AppCtx,
        id: &str,
        name: &str,
        kind: &str,
        engine: &str,
        parent: Option<&str>,
        sort_order: i64,
    ) {
        let conn = app.db().conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions(id, project_id, name, kind, engine, parent_session_id, cwd, permission_mode, sort_order, created_at) \
             VALUES (?1, 'proj', ?2, ?3, ?4, ?5, '/tmp', 'default', ?6, ?6)",
            params![id, name, kind, engine, parent, sort_order],
        )
        .unwrap();
    }

    fn attach_peer(app: &AppCtx, id: &str) {
        crate::agent::chat::engine::attach_inert_peer(app, id);
    }

    fn flow_req(sender: &str, run_id: &str, action: &str, round: u32, text: &str) -> Request {
        Request {
            session_id: sender.into(),
            run_id: run_id.into(),
            action: action.into(),
            round,
            text: text.into(),
            message_id: format!("msg-{}", uuid::Uuid::new_v4()),
        }
    }

    fn send_report(app: &AppCtx, exec: &str, round: u32, text: &str) -> Result<Value, String> {
        crate::agent::tell::send(
            app,
            &crate::agent::tell::Request {
                session_id: exec.into(),
                target: None,
                report: true,
                steer: false,
                round: Some(round),
                message_id: format!("msg-{}", uuid::Uuid::new_v4()),
                text: text.into(),
            },
        )
    }

    #[test]
    fn bind_zero_executors_prepares_and_records_planning_workflow() {
        let _lock = crate::session_state::test_lock();
        let app = test_app("zero");
        seed(&app, "p1", "Main Plan", "claude", "chat", None, 1);
        attach_peer(&app, "p1");

        let prepared = prepare(&app, &["p1".into()], None).unwrap();
        assert_eq!(prepared["plannerId"], "p1");
        assert_eq!(prepared["executorIds"], json!([]));

        let req_id = uuid::Uuid::new_v4().to_string();
        let req = BindRequest {
            request_id: req_id.clone(),
            planner_id: "p1".into(),
            executor_ids: vec![],
            prompt: String::new(),
            images: vec![],
            reparent_executors: false,
            config: Config {
                review_enabled: Some(false),
                worktree_mode: Some(WorktreeMode::Each),
                split_tasks: true,
                exec: RoleConfig {
                    agent: Some(SessionKind::Codex),
                    model: None,
                    effort: None,
                },
                ..Default::default()
            },
        };
        let res = apply(&app, &req).unwrap();
        assert_eq!(res["run"]["id"], req_id);
        assert_eq!(res["run"]["plannerId"], "p1");
        assert!(res["run"]["executorId"].is_null());
        assert_eq!(res["run"]["state"], "planning");
        assert_eq!(res["run"]["round"], 0);
        assert_eq!(res["run"]["config"]["splitTasks"], true);
        assert_eq!(res["run"]["config"]["worktreeMode"], "each");
        assert_eq!(res["delivery"]["delivery"], "sent");

        // Idempotent replay with same request_id succeeds without duplicating rows.
        let replay = apply(&app, &req).unwrap();
        assert_eq!(replay["run"]["id"], req_id);

        // Same request_id with altered payload is rejected.
        let mut altered = req.clone();
        altered.prompt = "mutated".into();
        assert!(apply(&app, &altered).unwrap_err().contains("different binding"));

        cleanup(app);
    }

    #[test]
    fn bind_single_executor_runs_full_dispatch_report_finish_and_waits() {
        let _lock = crate::session_state::test_lock();
        let app = test_app("single");
        seed(&app, "p1", "Planner", "claude", "chat", None, 1);
        seed(&app, "e1", "Child Exec", "codex", "chat", Some("p1"), 2);
        attach_peer(&app, "p1");
        attach_peer(&app, "e1");

        let prepared = prepare(&app, &["p1".into()], None).unwrap();
        assert_eq!(prepared["plannerId"], "p1");
        assert_eq!(prepared["executorIds"], json!(["e1"]));

        let req_id = uuid::Uuid::new_v4().to_string();
        let req = BindRequest {
            request_id: req_id.clone(),
            planner_id: "p1".into(),
            executor_ids: vec!["e1".into()],
            prompt: "Coordinate implementation".into(),
            images: vec![],
            reparent_executors: false,
            config: Config {
                review_enabled: Some(false),
                ..Default::default()
            },
        };
        let bound = apply(&app, &req).unwrap();
        assert_eq!(bound["run"]["executorId"], "e1");
        assert_eq!(bound["run"]["state"], "planning");
        assert_eq!(bound["run"]["round"], 0);
        assert!(bound["run"]["config"].get("splitTasks").is_none());
        assert_eq!(bound["delivery"]["delivery"], "sent");

        // Before dispatch: executor awaits parent, planner is alertable (None).
        let states = crate::session_state::snapshot();
        assert_eq!(states["e1"].workflow_wait.as_deref(), Some("parent"));
        assert_eq!(states.get("p1").and_then(|s| s.workflow_wait.as_deref()), None);

        // Round 1 dispatch reuses existing E1 and injects full executor protocol header.
        let before_sessions: i64 = app
            .db()
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        let dispatched = action(&app, &flow_req("p1", &req_id, "dispatch", 1, "Build module A")).unwrap();
        assert_eq!(dispatched["delivery"], "sent");
        assert_eq!(dispatched["targetSessionId"], "e1");
        assert_eq!(dispatched["run"]["state"], "executing");
        assert_eq!(dispatched["run"]["round"], 1);
        let after_sessions: i64 = app
            .db()
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(before_sessions, after_sessions);

        // Verify executor received the role protocol header on round 1.
        let exec_snap = serde_json::to_value(app.chat().snapshot("e1")).unwrap();
        let exec_msg = exec_snap["rows"][0]["text"].as_str().unwrap();
        assert!(exec_msg.contains(&format!("Workflow ID: {req_id}")));
        assert!(exec_msg.contains("Role: executor"));
        assert!(exec_msg.contains("Round: 1"));
        assert!(exec_msg.contains("Build module A"));

        // While executing: planner awaits children, executor awaits parent.
        let states = crate::session_state::snapshot();
        assert_eq!(states["p1"].workflow_wait.as_deref(), Some("children"));
        assert_eq!(states["e1"].workflow_wait.as_deref(), Some("parent"));

        // Executor submits report via vtell --report (auto-resolving unique workflow).
        let reported = send_report(&app, "e1", 1, "Module A implemented and verified").unwrap();
        assert_eq!(reported["run"]["state"], "summarizing");
        let states = crate::session_state::snapshot();
        assert_eq!(states.get("p1").and_then(|s| s.workflow_wait.as_deref()), None);

        // Planner finishes workflow; waits clear cleanly.
        let finished = action(&app, &flow_req("p1", &req_id, "finish", 1, "All done")).unwrap();
        assert_eq!(finished["run"]["state"], "completed");
        let states = crate::session_state::snapshot();
        assert_eq!(states.get("p1").and_then(|s| s.workflow_wait.as_deref()), None);
        assert_eq!(states.get("e1").and_then(|s| s.workflow_wait.as_deref()), None);

        cleanup(app);
    }

    #[test]
    fn bind_multiple_executors_runs_split_lifecycle_with_optional_review_and_waits() {
        let _lock = crate::session_state::test_lock();
        let app = test_app("multi");
        seed(&app, "p1", "Plan Root", "claude", "chat", None, 1);
        seed(&app, "e1", "Backend Worker", "claude", "chat", Some("p1"), 2);
        seed(&app, "e2", "Frontend Worker", "codex", "chat", Some("p1"), 3);
        attach_peer(&app, "p1");
        attach_peer(&app, "e1");
        attach_peer(&app, "e2");

        let prepared = prepare(&app, &["e2".into(), "p1".into(), "e1".into()], None).unwrap();
        // p1 is the unique ancestor of e1 and e2, so prepare infers p1 as planner.
        assert_eq!(prepared["plannerId"], "p1");
        assert_eq!(prepared["executorIds"], json!(["e2", "e1"]));

        let req_id = uuid::Uuid::new_v4().to_string();
        let bound = apply(
            &app,
            &BindRequest {
                request_id: req_id.clone(),
                planner_id: "p1".into(),
                executor_ids: vec!["e1".into(), "e2".into()],
                prompt: "Split delivery".into(),
                images: vec![],
                reparent_executors: false,
                config: Config {
                    review_enabled: Some(true),
                    review: RoleConfig {
                        agent: Some(SessionKind::Claude),
                        model: None,
                        effort: None,
                    },
                    ..Default::default()
                },
            },
        )
        .unwrap();

        assert_eq!(bound["run"]["state"], "executing");
        assert_eq!(bound["run"]["round"], 1);
        assert_eq!(bound["run"]["config"]["splitTasks"], true);
        let tasks = bound["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);
        let t1_id = tasks[0]["run"]["id"].as_str().unwrap().to_owned();
        let t2_id = tasks[1]["run"]["id"].as_str().unwrap().to_owned();
        assert_eq!(tasks[0]["run"]["executorId"], "e1");
        assert_eq!(tasks[1]["run"]["executorId"], "e2");
        assert_eq!(tasks[0]["run"]["state"], "planning");
        assert_eq!(tasks[1]["run"]["state"], "planning");

        // Immediately after bind: executors await parent, planner remains alertable until it dispatches.
        let states = crate::session_state::snapshot();
        assert_eq!(states["e1"].workflow_wait.as_deref(), Some("parent"));
        assert_eq!(states["e2"].workflow_wait.as_deref(), Some("parent"));
        assert_eq!(states.get("p1").and_then(|s| s.workflow_wait.as_deref()), None);

        // Dispatch both tasks; planner now awaits children.
        action(&app, &flow_req("p1", &t1_id, "dispatch", 1, "Task 1")).unwrap();
        action(&app, &flow_req("p1", &t2_id, "dispatch", 1, "Task 2")).unwrap();
        assert_eq!(
            crate::session_state::snapshot()["p1"].workflow_wait.as_deref(),
            Some("children")
        );

        // E1 reports -> reviewer created -> attach inert peer for reviewer -> retry report delivery.
        let rep1 = flow_req("e1", &t1_id, "report", 1, "Task 1 done");
        assert!(crate::agent::tell::send(
            &app,
            &crate::agent::tell::Request {
                session_id: "e1".into(),
                target: None,
                report: true,
                steer: false,
                round: Some(1),
                message_id: rep1.message_id.clone(),
                text: rep1.text.clone(),
            }
        )
        .unwrap_err()
        .contains("saved"));
        let status1 = action(&app, &flow_req("p1", &t1_id, "status", 0, "")).unwrap();
        let rev_id = status1["run"]["reviewerId"].as_str().unwrap().to_owned();
        attach_peer(&app, &rev_id);
        crate::agent::tell::send(
            &app,
            &crate::agent::tell::Request {
                session_id: "e1".into(),
                target: None,
                report: true,
                steer: false,
                round: Some(1),
                message_id: rep1.message_id.clone(),
                text: rep1.text.clone(),
            },
        )
        .unwrap();
        send_report(&app, "e2", 1, "Task 2 done").unwrap();

        // Reviewer accepts both tasks, planner finishes both tasks -> integrated review triggers.
        action(&app, &flow_req(&rev_id, &t1_id, "accept", 1, "Task 1 passed")).unwrap();
        action(&app, &flow_req(&rev_id, &t2_id, "accept", 1, "Task 2 passed")).unwrap();
        action(&app, &flow_req("p1", &t1_id, "finish", 1, "Task 1 complete")).unwrap();
        action(&app, &flow_req("p1", &t2_id, "finish", 1, "Task 2 complete")).unwrap();

        let parent_status = action(&app, &flow_req("p1", &req_id, "status", 0, "")).unwrap();
        assert_eq!(parent_status["run"]["state"], "reviewing");

        // Reviewer accepts integrated delivery, planner finishes overall workflow.
        action(&app, &flow_req(&rev_id, &req_id, "accept", 1, "Integrated delivery passed")).unwrap();
        let final_res = action(&app, &flow_req("p1", &req_id, "finish", 1, "Overall delivery complete")).unwrap();
        assert_eq!(final_res["run"]["state"], "completed");
        assert_eq!(final_res["run"]["reviewStatus"], "passed");

        cleanup(app);
    }

    #[test]
    fn bind_mode_b_extends_empty_single_and_active_split_planners() {
        let _lock = crate::session_state::test_lock();
        let app = test_app("mode-b");
        seed(&app, "p1", "Plan A", "claude", "chat", None, 1);
        seed(&app, "e1", "Exec 1", "claude", "chat", None, 2);
        seed(&app, "e2", "Exec 2", "codex", "chat", None, 3);
        seed(&app, "e3", "Exec 3", "opencode", "chat", None, 4);
        for id in ["p1", "e1", "e2", "e3"] {
            attach_peer(&app, id);
        }

        // Start Mode A with 0 executors.
        let r1 = uuid::Uuid::new_v4().to_string();
        apply(
            &app,
            &BindRequest {
                request_id: r1.clone(),
                planner_id: "p1".into(),
                executor_ids: vec![],
                prompt: "Initial plan".into(),
                images: vec![],
                reparent_executors: false,
                config: Config {
                    review_enabled: Some(false),
                    ..Default::default()
                },
            },
        )
        .unwrap();

        // Prepare now marks p1 as extendablePlanner.
        let prep = prepare(&app, &["p1".into(), "e1".into(), "e2".into()], None).unwrap();
        assert_eq!(prep["plannerId"], "p1");
        assert_eq!(prep["executorIds"], json!(["e1", "e2"]));
        let p_cand = prep["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "p1")
            .unwrap();
        assert_eq!(p_cand["extendablePlanner"], true);

        // Mode B1b: Append 2 executors to empty single-task planner -> upgrades to split_tasks = true.
        let r2 = uuid::Uuid::new_v4().to_string();
        let ext = apply(
            &app,
            &BindRequest {
                request_id: r2,
                planner_id: "p1".into(),
                executor_ids: vec!["e1".into(), "e2".into()],
                prompt: "Add two workers".into(),
                images: vec![],
                reparent_executors: false,
                config: Config {
                    // Attempting to flip reviewEnabled in Mode B must keep original locked value (false).
                    review_enabled: Some(true),
                    ..Default::default()
                },
            },
        )
        .unwrap();
        assert_eq!(ext["run"]["id"], r1);
        assert_eq!(ext["run"]["config"]["splitTasks"], true);
        assert_eq!(ext["run"]["config"]["reviewEnabled"], false);
        assert_eq!(ext["tasks"].as_array().unwrap().len(), 2);
        // The planner turn started by the initial bind is still running, so the notice queues safely.
        assert_eq!(ext["delivery"]["delivery"], "queued");

        // Mode B2: Append 3rd executor to active split planner.
        let r3 = uuid::Uuid::new_v4().to_string();
        let ext2 = apply(
            &app,
            &BindRequest {
                request_id: r3,
                planner_id: "p1".into(),
                executor_ids: vec!["e3".into()],
                prompt: String::new(),
                images: vec![],
                reparent_executors: false,
                config: Config::default(),
            },
        )
        .unwrap();
        assert_eq!(ext2["run"]["id"], r1);
        assert_eq!(ext2["tasks"].as_array().unwrap().len(), 3);
        assert_eq!(ext2["tasks"][2]["run"]["executorId"], "e3");
        assert_eq!(ext2["delivery"]["delivery"], "queued");

        cleanup(app);
    }

    #[test]
    fn bind_rejects_invalid_sessions_conflicts_and_limits() {
        let _lock = crate::session_state::test_lock();
        let app = test_app("reject");
        seed(&app, "p_chat", "Chat Plan", "claude", "chat", None, 1);
        seed(&app, "p_tui", "TUI Plan", "claude", "tui", None, 2);
        seed(&app, "p_term", "Terminal", "terminal", "tui", None, 3);
        seed(&app, "p_cursor", "Cursor Chat", "cursor", "chat", None, 4);
        seed(&app, "p_arch", "Archived", "claude", "chat", None, 5);
        app.db()
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE sessions SET archived_at = 1234 WHERE id = 'p_arch'", [])
            .unwrap();
        for i in 0..13 {
            seed(&app, &format!("ex_{i}"), &format!("Exec {i}"), "claude", "chat", None, 10 + i);
        }
        attach_peer(&app, "p_chat");
        attach_peer(&app, "ex_0");

        let make_req = |planner: &str, execs: Vec<&str>| BindRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            planner_id: planner.into(),
            executor_ids: execs.into_iter().map(str::to_owned).collect(),
            prompt: String::new(),
            images: vec![],
            reparent_executors: false,
            config: Config::default(),
        };

        // TUI session rejected.
        assert!(apply(&app, &make_req("p_tui", vec![])).unwrap_err().contains("chat"));
        // Unsupported terminal / cursor kind rejected.
        assert!(apply(&app, &make_req("p_term", vec![])).unwrap_err().contains("chat-capable"));
        assert!(apply(&app, &make_req("p_cursor", vec![])).unwrap_err().contains("chat-capable"));
        // Archived session rejected.
        assert!(apply(&app, &make_req("p_arch", vec![])).unwrap_err().contains("missing or archived"));
        // Planner also in executor_ids rejected.
        assert!(apply(&app, &make_req("p_chat", vec!["p_chat"])).unwrap_err().contains("also be selected"));
        // Over 12 executors rejected.
        let thirteen: Vec<String> = (0..13).map(|i| format!("ex_{i}")).collect();
        assert!(apply(
            &app,
            &BindRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                planner_id: "p_chat".into(),
                executor_ids: thirteen,
                prompt: String::new(),
                images: vec![],
                reparent_executors: false,
                config: Config::default(),
            }
        )
        .unwrap_err()
        .contains("at most 12"));

        // Bind p_chat -> ex_0, then verify active uniqueness rejects reusing ex_0 as plan or exec.
        apply(&app, &make_req("p_chat", vec!["ex_0"])).unwrap();
        assert!(apply(&app, &make_req("ex_0", vec![])).unwrap_err().contains("already an executor"));
        assert!(apply(&app, &make_req("ex_1", vec!["ex_0"])).unwrap_err().contains("already belongs to an active workflow"));
        // p_chat already has a bound single executor, so it cannot be extended.
        assert!(apply(&app, &make_req("p_chat", vec!["ex_1"])).unwrap_err().contains("cannot accept additional"));

        cleanup(app);
    }

    #[test]
    fn bind_reparent_executors_handles_normal_and_inverted_ancestry_without_cycles() {
        let _lock = crate::session_state::test_lock();
        let app = test_app("reparent");
        // Tree initially: old_parent -> mid_child -> new_plan, plus sibling_exec
        seed(&app, "old_parent", "Old Parent", "claude", "chat", None, 1);
        seed(&app, "mid_child", "Mid Child", "claude", "chat", Some("old_parent"), 2);
        seed(&app, "new_plan", "New Plan", "claude", "chat", Some("mid_child"), 3);
        seed(&app, "sibling_exec", "Sibling", "claude", "chat", None, 4);
        for id in ["old_parent", "mid_child", "new_plan", "sibling_exec"] {
            attach_peer(&app, id);
        }

        // Bind new_plan as Plan and [old_parent, sibling_exec] as Execute with reparent_executors = true.
        apply(
            &app,
            &BindRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                planner_id: "new_plan".into(),
                executor_ids: vec!["old_parent".into(), "sibling_exec".into()],
                prompt: String::new(),
                images: vec![],
                reparent_executors: true,
                config: Config::default(),
            },
        )
        .unwrap();

        let conn = app.db().conn.lock().unwrap();
        let p = repo::get_session(&conn, "new_plan").unwrap().unwrap();
        let e1 = repo::get_session(&conn, "old_parent").unwrap().unwrap();
        let e2 = repo::get_session(&conn, "sibling_exec").unwrap().unwrap();
        let mid = repo::get_session(&conn, "mid_child").unwrap().unwrap();

        // new_plan lifted to root (old_parent's former parent = None); old_parent and sibling_exec now sit under new_plan.
        assert_eq!(p.parent_session_id, None);
        assert_eq!(e1.parent_session_id.as_deref(), Some("new_plan"));
        assert_eq!(e2.parent_session_id.as_deref(), Some("new_plan"));
        assert_eq!(mid.parent_session_id.as_deref(), Some("old_parent"));
        drop(conn);

        cleanup(app);
    }

    #[test]
    fn bind_supports_antigravity_as_first_class_planner_and_executor_and_ignores_stop_parked_observe() {
        let _lock = crate::session_state::test_lock();
        let app = test_app("antigravity");
        assert!(supported(SessionKind::Antigravity));
        seed(&app, "agy_plan", "Antigravity Plan", "antigravity", "chat", None, 1);
        seed(&app, "agy_exec", "Antigravity Exec", "antigravity", "chat", Some("agy_plan"), 2);
        attach_peer(&app, "agy_plan");
        attach_peer(&app, "agy_exec");

        let prepared = prepare(&app, &["agy_plan".into()], None).unwrap();
        assert_eq!(prepared["plannerId"], "agy_plan");
        assert_eq!(prepared["executorIds"], json!(["agy_exec"]));

        let req_id = uuid::Uuid::new_v4().to_string();
        let bound = apply(
            &app,
            &BindRequest {
                request_id: req_id.clone(),
                planner_id: "agy_plan".into(),
                executor_ids: vec!["agy_exec".into()],
                prompt: "Implement with Antigravity".into(),
                images: vec![],
                reparent_executors: false,
                config: Config {
                    review_enabled: Some(false),
                    ..Default::default()
                },
            },
        )
        .unwrap();
        assert_eq!(bound["run"]["plannerId"], "agy_plan");
        assert_eq!(bound["run"]["executorId"], "agy_exec");
        assert_eq!(bound["run"]["state"], "planning");

        // A stop-parked turnCompleted event (Antigravity awaiting its own background work) must not block the workflow.
        observe(&app, "agy_plan", &json!({"type": "turnCompleted", "stopParked": true}));
        assert_eq!(get(&app, &req_id).unwrap().state, "planning");

        // Full dispatch -> report -> finish works identically to Claude and Codex.
        action(&app, &flow_req("agy_plan", &req_id, "dispatch", 1, "Run Antigravity task")).unwrap();
        send_report(&app, "agy_exec", 1, "Antigravity task completed").unwrap();
        let done = action(&app, &flow_req("agy_plan", &req_id, "finish", 1, "Delivered")).unwrap();
        assert_eq!(done["run"]["state"], "completed");

        cleanup(app);
    }

    #[test]
    fn bind_prepare_executors_mode_keeps_all_selected_as_executors_and_infers_command_session() {
        let _lock = crate::session_state::test_lock();
        let app = test_app("exec-mode");
        seed(&app, "cmd1", "Command One", "claude", "chat", None, 1);
        seed(&app, "cmd2", "Command Two", "antigravity", "chat", None, 2);
        seed(&app, "w1", "Worker 1", "codex", "chat", Some("cmd1"), 3);
        seed(&app, "w2", "Worker 2", "antigravity", "chat", Some("cmd1"), 4);
        seed(&app, "w3", "Worker 3", "claude", "chat", None, 5);
        attach_peer(&app, "cmd2");

        // Case 1: Selected workers share unique common ancestor cmd1 -> cmd1 inferred, all selected stay executors.
        let p_common = prepare(&app, &["w1".into(), "w2".into()], Some("executors")).unwrap();
        assert_eq!(p_common["plannerId"], "cmd1");
        assert_eq!(p_common["executorIds"], json!(["w1", "w2"]));

        // Case 2: No common ancestor and no active extendable command session -> plannerId left empty for manual pick.
        let p_none = prepare(&app, &["w1".into(), "w3".into()], Some("executors")).unwrap();
        assert_eq!(p_none["plannerId"], "");
        assert_eq!(p_none["executorIds"], json!(["w1", "w3"]));

        // Case 3: Once cmd2 is an active extendable command session, selecting w1 + w3 auto-picks cmd2 as command session.
        apply(
            &app,
            &BindRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                planner_id: "cmd2".into(),
                executor_ids: vec![],
                prompt: "Active command".into(),
                images: vec![],
                reparent_executors: false,
                config: Config::default(),
            },
        )
        .unwrap();
        let p_active = prepare(&app, &["w1".into(), "w3".into()], Some("executors")).unwrap();
        assert_eq!(p_active["plannerId"], "cmd2");
        assert_eq!(p_active["executorIds"], json!(["w1", "w3"]));

        cleanup(app);
    }
}
