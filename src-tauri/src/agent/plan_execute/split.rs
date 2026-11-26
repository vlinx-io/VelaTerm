//! Persisted task proposals and user-confirmed execution under one planning conversation.

use super::*;

pub(super) const MAX_TASKS: usize = 12;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Task {
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub config: RoleConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Confirmation {
    pub run_id: String,
    pub proposal_id: String,
    pub tasks: Vec<Task>,
}

pub fn parent_id(app: &AppCtx, id: &str) -> Result<Option<String>, String> {
    app.db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT parent_id FROM plan_execute_tasks WHERE run_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())
}

pub fn task_name(app: &AppCtx, id: &str) -> Result<Option<String>, String> {
    app.db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT name FROM plan_execute_tasks WHERE run_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())
}

pub fn task_ids(app: &AppCtx, id: &str) -> Result<Vec<String>, String> {
    let conn = app.db().conn.lock().unwrap();
    let mut q = conn
        .prepare("SELECT run_id FROM plan_execute_tasks WHERE parent_id=?1 ORDER BY position")
        .map_err(|e| e.to_string())?;
    let rows = q.query_map([id], |r| r.get(0)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn tasks(app: &AppCtx, id: &str) -> Result<Vec<Value>, String> {
    let rows: Vec<(String, String)> = {
        let conn = app.db().conn.lock().unwrap();
        let mut q = conn
            .prepare(
                "SELECT run_id,name FROM plan_execute_tasks WHERE parent_id=?1 ORDER BY position",
            )
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    rows.into_iter()
        .map(|(id, name)| {
            let run = get(app, &id)?;
            Ok(json!({"name":name,"prompt":run.task,"run":brief(&run),
            "executor":run.executor_id.as_deref().map(|id|health(app,id))}))
        })
        .collect()
}

fn normalized(run: &Run, tasks: &[Task]) -> Result<Vec<Task>, String> {
    if tasks.is_empty() || tasks.len() > MAX_TASKS {
        return Err(format!("Submit between 1 and {MAX_TASKS} tasks"));
    }
    tasks
        .iter()
        .map(|task| {
            if task.name.trim().is_empty()
                || task.name.chars().count() > 80
                || task.prompt.trim().is_empty()
            {
                return Err(
                    "Every task needs a name of at most 80 characters and a nonempty prompt".into(),
                );
            }
            let same_agent =
                task.config.agent.is_none() || task.config.agent == run.config.exec.agent;
            let config = RoleConfig {
                agent: task.config.agent.or(run.config.exec.agent),
                model: task
                    .config
                    .model
                    .clone()
                    .or_else(|| same_agent.then(|| run.config.exec.model.clone()).flatten()),
                effort: task
                    .config
                    .effort
                    .clone()
                    .or_else(|| same_agent.then(|| run.config.exec.effort.clone()).flatten()),
            };
            validate_config(&Config {
                plan: run.config.plan.clone(),
                exec: config.clone(),
                split_tasks: false,
                ..run.config.clone()
            })?;
            Ok(Task {
                name: task.name.trim().to_owned(),
                prompt: task.prompt.clone(),
                config,
            })
        })
        .collect()
}

/// Called by the planner. This records a proposal only; confirmation is a separate UI command.
pub fn propose(app: &AppCtx, run: &Run, req: &Request) -> Result<Value, String> {
    if req.session_id != run.planner_id || !run.config.split_tasks {
        return Err("Only the planner of a task-splitting workflow can propose tasks".into());
    }
    super::super::tell::validate_message(&req.message_id, &req.text)?;
    let hash = fingerprint(req);
    let previous: Option<(String, String, String)> = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT message_id,fingerprint,state FROM plan_execute_proposals WHERE run_id=?1",
            [&run.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some((id, old_hash, state)) = previous {
        if id == req.message_id {
            if hash != old_hash {
                return Err("This proposal ID already has different content".into());
            }
            if state == "pending" {
                app.emit("plan-execute://proposal", json!({"runId":run.id}));
            }
            return Ok(json!({"proposalId":id,"state":state,"run":brief(run)}));
        }
        if state != "cancelled" {
            return Err("The existing proposal must be resolved before submitting another".into());
        }
    }
    if !matches!(run.state.as_str(), "planning" | "blocked") || !task_ids(app, &run.id)?.is_empty()
    {
        return Err("Tasks can only be proposed before execution begins".into());
    }
    let input: Proposal =
        serde_json::from_str(&req.text).map_err(|e| format!("Invalid task proposal: {e}"))?;
    let tasks = normalized(run, &input.tasks)?;
    {
        let mut conn = app.db().conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO plan_execute_proposals(run_id,message_id,fingerprint,tasks,state,notice_id) VALUES (?1,?2,?3,?4,'pending',?5) ON CONFLICT(run_id) DO UPDATE SET message_id=excluded.message_id,fingerprint=excluded.fingerprint,tasks=excluded.tasks,state='pending',confirmation=NULL,notice_id=excluded.notice_id",
            params![run.id, req.message_id, hash, serde_json::to_string(&tasks).unwrap(),format!("msg-{}",uuid::Uuid::new_v4())]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE plan_execute_runs SET state='awaiting_confirmation',summary='Task proposal awaiting user confirmation' WHERE id=?1",[&run.id]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    sync_session_states(app);
    crate::diagnostics::record(
        "INFO",
        "plan_execute_proposal",
        json!({"runId":run.id,"taskCount":tasks.len(),"status":"pending"}),
    );
    app.emit("plan-execute://proposal", json!({"runId":run.id}));
    Ok(json!({"proposalId":req.message_id,"state":"pending","run":brief(&get(app,&run.id)?)}))
}

pub fn pending(app: &AppCtx) -> Result<Value, String> {
    let conn = app.db().conn.lock().unwrap();
    let mut q = conn.prepare("SELECT p.run_id FROM plan_execute_proposals p JOIN plan_execute_runs r ON r.id=p.run_id JOIN sessions s ON s.id=r.planner_id WHERE p.state='pending' AND r.state='awaiting_confirmation' AND s.archived_at IS NULL ORDER BY p.rowid").map_err(|e|e.to_string())?;
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    Ok(json!(rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?))
}

pub fn read(app: &AppCtx, id: &str) -> Result<Value, String> {
    let run = get(app, id)?;
    let planner = session(app, &run.planner_id)?;
    let (proposal_id, state, raw): (String, String, String) = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT message_id,state,tasks FROM plan_execute_proposals WHERE run_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| "Task proposal not found".to_owned())?;
    Ok(
        json!({"runId":id,"proposalId":proposal_id,"state":state,"tasks":serde_json::from_str::<Vec<Task>>(&raw).map_err(|e|e.to_string())?,
        "plannerId":planner.id,"plannerName":planner.name,"cwd":planner.cwd,"task":run.task,"run":brief(&run),"executionTasks":tasks(app,id)?}),
    )
}

/// User edits are validated and saved before any executor is created. Replays launch only missing work.
pub fn confirm(app: &AppCtx, req: &Confirmation) -> Result<Value, String> {
    let _guard = operation_lock().lock().unwrap();
    let run = get(app, &req.run_id)?;
    if matches!(run.state.as_str(), "stopped" | "completed") {
        return Err("This workflow has already ended".into());
    }
    let edited = normalized(&run, &req.tasks)?;
    let serialized = serde_json::to_string(&edited).map_err(|e| e.to_string())?;
    if serialized.len() > 65536 {
        return Err("The task proposal exceeds 65536 bytes".into());
    }
    let (id, state, previous, notice_id): (String, String, Option<String>, String) = app.db().conn.lock().unwrap().query_row(
        "SELECT message_id,state,confirmation,notice_id FROM plan_execute_proposals WHERE run_id=?1",[&run.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).map_err(|_|"Task proposal not found".to_owned())?;
    if req.proposal_id != id {
        return Err("The proposal has changed; reload it before confirming".into());
    }
    if state == "confirmed" {
        if previous.as_deref() != Some(&serialized) {
            return Err("This proposal was already confirmed with different tasks".into());
        }
    } else {
        if state != "pending" || run.state != "awaiting_confirmation" {
            return Err("This proposal is no longer awaiting confirmation".into());
        }
        let images = message_images(app, &format!("msg-{}", run.id))?;
        let mut conn = app.db().conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for (position, task) in edited.iter().enumerate() {
            let task_id = uuid::Uuid::new_v4().to_string();
            let dispatch_id = format!("msg-{}", uuid::Uuid::new_v4());
            let config = Config {
                plan: run.config.plan.clone(),
                exec: task.config.clone(),
                split_tasks: false,
                ..run.config.clone()
            };
            tx.execute("INSERT INTO plan_execute_runs(id,owner_id,planner_id,config,task,state,review_status) VALUES (?1,?2,?2,?3,?4,'planning',?5)",
                params![task_id,run.planner_id,serde_json::to_string(&config).unwrap(),task.prompt,config.initial_review_status()]).map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO plan_execute_tasks(parent_id,run_id,position,name,dispatch_id) VALUES (?1,?2,?3,?4,?5)",
                params![run.id,task_id,position,task.name,dispatch_id]).map_err(|e|e.to_string())?;
            let start_id = format!("msg-{task_id}");
            tx.execute("INSERT INTO plan_execute_messages(id,run_id,sender_id,target_id,action,round,fingerprint,wire,origin) VALUES (?1,?2,?3,?3,'start',0,'user',?4,'{\"role\":\"user\"}')",
                params![start_id,task_id,run.planner_id,task.prompt]).map_err(|e|e.to_string())?;
            save_images(&tx, &start_id, &images)?;
        }
        tx.execute("UPDATE plan_execute_proposals SET state='confirmed',tasks=?2,confirmation=?2 WHERE run_id=?1",params![run.id,serialized]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE plan_execute_runs SET state='executing',round=1,review_status=?2,summary='User confirmed the execution tasks' WHERE id=?1",params![run.id,run.config.initial_review_status()]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    let mut errors = Vec::new();
    for id in task_ids(app, &run.id)? {
        let task = get(app, &id)?;
        let pending: bool = app
            .db()
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT launch_pending FROM plan_execute_tasks WHERE run_id=?1",
                [&id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !pending {
            continue;
        }
        // A report or a correction round must never be replayed by a delayed confirmation response.
        if task.round > 1 || matches!(task.state.as_str(), "completed" | "stopped" | "reviewing" | "summarizing") {
            continue;
        }
        let dispatch_id: String = app
            .db()
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT dispatch_id FROM plan_execute_tasks WHERE run_id=?1",
                [&id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let request = Request {
            session_id: run.planner_id.clone(),
            run_id: id.clone(),
            action: "dispatch".into(),
            message_id: dispatch_id,
            text: task.task.clone(),
            round: 1,
        };
        if let Err(error) = apply_action(app, &request) {
            app.db()
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE plan_execute_runs SET state='blocked',summary=?2 WHERE id=?1",
                    params![id, error],
                )
                .map_err(|e| e.to_string())?;
            errors.push(json!({"runId":id,"error":error}));
        } else {
            let mut conn = app.db().conn.lock().unwrap();
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            tx.execute(
                "UPDATE plan_execute_tasks SET launch_pending=0 WHERE run_id=?1",
                [&id],
            )
            .map_err(|e| e.to_string())?;
            tx.execute("UPDATE plan_execute_runs SET state='executing' WHERE id=?1 AND state='blocked' AND round=1",[&id]).map_err(|e|e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        }
    }
    refresh_parent(app, &run.id)?;
    // Persist a single notice and stable delivery ID so retries cannot prompt the planner twice.
    let wire = {
        let conn = app.db().conn.lock().unwrap();
        conn.query_row(
            "SELECT wire FROM plan_execute_messages WHERE id=?1",
            [&notice_id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    };
    let wire = if let Some(wire) = wire {
        wire
    } else {
        let origin =
            json!({"name":"VelaTerm","agent":"terminal","role":"system","runId":run.id,"round":1});
        let instructions = if run.config.review_enabled.is_none() { "Review each task with dispatch and accept; perform the final audit before overall acceptance." } else { "Plan only collects progress and summarizes. Use finish for each ready task, then finish the overall workflow. When review is enabled, the shared reviewer must pass each task and integrated delivery first. Do not perform technical review." };
        let text = format!("{instructions}\nThe user confirmed these tasks. The approved prompts and execution settings below replace your proposal. Each task has its own workflow ID and report/correction round, all addressed to this planning conversation. Use vflow status with the task workflow IDs. Do not finish the overall workflow until every task is complete and the full delivery requirements are met. Inspect blocked tasks before retrying. End your turn while executors are working; their reports will arrive here.\n{}",serde_json::to_string_pretty(&tasks(app,&run.id)?).unwrap());
        let wire = format!("[VelaTerm message {notice_id}]\n{origin}\n\n{text}");
        app.db().conn.lock().unwrap().execute("INSERT INTO plan_execute_messages(id,run_id,sender_id,target_id,action,round,fingerprint,wire,origin) VALUES (?1,?2,?3,?3,'confirmed',1,'user',?4,?5)",params![notice_id,run.id,run.planner_id,wire,origin.to_string()]).map_err(|e|e.to_string())?;
        wire
    };
    if let Err(error) = deliver(app, &get(app, &run.id)?, &run.planner_id, &wire, &notice_id) {
        errors.push(json!({"runId":run.id,"error":error}));
    }
    crate::diagnostics::record(
        "INFO",
        "plan_execute_confirm",
        json!({"runId":run.id,"taskCount":edited.len(),"failedCount":errors.len()}),
    );
    app.emit(
        "plan-execute://proposal",
        json!({"runId":run.id,"resolved":true}),
    );
    Ok(json!({"run":brief(&get(app,&run.id)?),"tasks":tasks(app,&run.id)?,"errors":errors}))
}

pub fn cancel(app: &AppCtx, id: &str, proposal_id: &str) -> Result<Value, String> {
    let _guard = operation_lock().lock().unwrap();
    let mut conn = app.db().conn.lock().unwrap();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let (actual, state): (String, String) = tx
        .query_row(
            "SELECT message_id,state FROM plan_execute_proposals WHERE run_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    if actual != proposal_id || !matches!(state.as_str(), "pending" | "cancelled") {
        return Err("This proposal is no longer awaiting confirmation".into());
    }
    tx.execute(
        "UPDATE plan_execute_proposals SET state='cancelled' WHERE run_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute("UPDATE plan_execute_runs SET state='blocked',summary='The user cancelled the task proposal. Wait for new instructions before proposing again.' WHERE id=?1 AND state='awaiting_confirmation'",[id]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    drop(conn);
    sync_session_states(app);
    app.emit(
        "plan-execute://proposal",
        json!({"runId":id,"resolved":true}),
    );
    read(app, id)
}

pub fn refresh_parent(app: &AppCtx, id: &str) -> Result<(), String> {
    let ids = task_ids(app, id)?;
    if ids.is_empty() {
        return Ok(());
    }
    let all_done = ids
        .iter()
        .all(|id| get(app, id).is_ok_and(|r| r.state == "completed"));
    let mut run = get(app,id)?;
    if matches!(run.state.as_str(),"completed"|"stopped") { return Ok(()); }
    if all_done && run.config.review_enabled == Some(true) {
        review::integrated(app,&mut run)?;
    } else {
        let state = if all_done { if run.config.review_enabled == Some(false) { "summarizing" } else { "reviewing" } } else { "executing" };
        app.db().conn.lock().unwrap().execute("UPDATE plan_execute_runs SET state=?2,review_status=CASE WHEN ?3 THEN 'pending' ELSE review_status END WHERE id=?1 AND state!='blocked'",
            params![id,state,!all_done && run.config.review_enabled == Some(true)]).map_err(|e|e.to_string())?;
    }
    sync_session_states(app);
    Ok(())
}

pub fn check_action(app: &AppCtx, run: &Run, action: &str) -> Result<(), String> {
    if let Some(parent) = parent_id(app, &run.id)? {
        if matches!(get(app, &parent)?.state.as_str(), "completed" | "stopped")
            && action != "status"
        {
            return Err("The overall workflow has already ended".into());
        }
    }
    if run.config.split_tasks {
        if action == "dispatch" {
            return Err("Use vflow propose and wait for user confirmation; dispatch corrections to a task workflow ID".into());
        }
        if matches!(action,"accept"|"finish") {
            let ids = task_ids(app, &run.id)?;
            if ids.is_empty()
                || !ids
                    .iter()
                    .all(|id| get(app, id).is_ok_and(|r| r.state == "completed"))
            {
                return Err(
                    "Complete every execution task before completing the overall workflow".into(),
                );
            }
        }
    }
    Ok(())
}
