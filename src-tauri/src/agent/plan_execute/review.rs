//! Optional independent review, persisted fan-out and shared split-workflow review.

use super::*;

pub(super) fn protocol(config: &Config) -> String {
    if config.review_enabled.is_none() {
        return include_str!("../../../../skills/vspawn/references/plan-execute-legacy.md").into();
    }
    format!("{}\n\nIndependent review: {}. This selection is fixed for this workflow.",
        include_str!("../../../../skills/vspawn/references/plan-execute.md"),
        if config.review_enabled == Some(true) { "enabled" } else { "disabled; Plan must not perform technical review" })
}

pub(super) fn has_report(app: &AppCtx, run: &Run) -> Result<bool, String> {
    app.db().conn.lock().unwrap().query_row(
        "SELECT 1 FROM plan_execute_messages WHERE run_id=?1 AND action='report' AND round=?2 AND sender_id=?3 LIMIT 1",
        params![run.id,run.round,run.executor_id], |_|Ok(())).optional().map(|v|v.is_some()).map_err(|e|e.to_string())
}

pub(super) fn ensure_reviewer(app: &AppCtx, run: &mut Run) -> Result<(), String> {
    let root_id = split::parent_id(app,&run.id)?.unwrap_or_else(||run.id.clone());
    let root = get(app,&root_id)?;
    if root.config.review_enabled != Some(true) { return Err("Independent review is not enabled".into()); }
    let planner = session(app,&root.planner_id)?;
    let id = root.reviewer_id.clone().unwrap_or_else(||role_id(&root_id,"reviewer"));
    let reviewer = create_role_bound(app,&id,&planner.project_id,planner.group_id.as_deref(),Some(&planner),
        "Review",&root.config.review,planner.cwd.as_deref(),false,planner.worktree_path.as_deref(),
        planner.worktree_base_ref.as_deref(),|_,_|Ok(()))?;
    session(app,&reviewer.id)?;
    // Also bind an already-created peer after a retry. All tasks share the same persisted reviewer.
    app.db().conn.lock().unwrap().execute(
        "UPDATE plan_execute_runs SET reviewer_id=?2 WHERE id=?1 OR id IN (SELECT run_id FROM plan_execute_tasks WHERE parent_id=?1)",
        params![root_id,reviewer.id]).map_err(|e|e.to_string())?;
    run.reviewer_id = Some(reviewer.id);
    sync_session_states(app);
    Ok(())
}

pub(super) fn save_plan_notice(conn: &rusqlite::Connection, run: &Run, req: &Request, origin: &Value) -> Result<(), String> {
    if !matches!(req.action.as_str(),"report"|"dispatch") { return Ok(()); }
    let id = format!("msg-{}",role_id(&req.message_id,"plan-notice"));
    let text = if req.action == "report" {
        format!("Execution report submitted. Workflow: {}; round: {}; report message: {}; reviewer: {}. Independent review is pending. Collect progress; do not perform technical review or finish before the reviewer passes this round. Full report: use vflow read-report {} --message-id {}.",
            run.id,req.round,req.message_id,run.reviewer_id.as_deref().unwrap_or(""),run.id,req.message_id)
    } else {
        format!("The reviewer dispatched corrections directly to the existing executor. Workflow: {}; round: {}; correction message: {}. Collect progress; do not resend this assignment.\n\n{}",run.id,req.round,req.message_id,req.text)
    };
    let wire = format!("[VelaTerm message {id}]\n{origin}\n\n{text}");
    conn.execute("INSERT INTO plan_execute_messages(id,run_id,sender_id,target_id,action,round,fingerprint,wire,origin) VALUES (?1,?2,?3,?4,'progress',?5,?6,?7,?8)",
        params![id,run.id,req.session_id,run.planner_id,req.round,format!("fanout:{}",req.message_id),wire,origin.to_string()]).map_err(|e|e.to_string())?;
    Ok(())
}

fn receipt(app: &AppCtx, run: &Run, target: &str, id: &str) -> Option<Value> {
    let saved: Option<String> = app.db().conn.lock().unwrap().query_row(
        "SELECT outcome FROM chat_submissions WHERE session_id=?1 AND id=?2",params![target,id],|r|r.get(0)).optional().ok().flatten().flatten();
    let outcome: Result<String,String> = serde_json::from_str(saved.as_deref()?).ok()?;
    let outcome = outcome.ok()?;
    Some(json!({"delivery":outcome,"messageId":id,"targetSessionId":target,"run":brief(run)}))
}

pub(super) fn pending_progress(app: &AppCtx, run: &Run, id: &str) -> Result<bool,String> {
    let conn = app.db().conn.lock().unwrap();
    conn.query_row("SELECT EXISTS(SELECT 1 FROM plan_execute_messages m LEFT JOIN chat_submissions s ON s.session_id=m.target_id AND s.id=m.id WHERE m.run_id=?1 AND m.fingerprint=?2 AND (s.outcome IS NULL OR json_extract(s.outcome,'$.Ok') IS NULL))",
        params![run.id,format!("fanout:{id}")],|r|r.get(0)).map_err(|e|e.to_string())
}

fn send_pending(app: &AppCtx, run: &Run, target: &str, wire: &str, id: &str) -> Result<Value,String> {
    if let Some(saved) = receipt(app,run,target,id) {
        if saved["delivery"] == "sent" || app.chat().is_alive(target) { return Ok(saved); }
    }
    deliver(app,run,target,wire,id)
}

pub(super) fn deliver_handoff(app: &AppCtx, run: &Run, target: &str, wire: &str, id: &str) -> Result<Value, String> {
    let round: u32 = app.db().conn.lock().unwrap().query_row("SELECT round FROM plan_execute_messages WHERE id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
    let primary = if round < run.round {
        receipt(app,run,target,id).ok_or_else(||"An earlier-round delivery is unresolved; inspect the original receipt before resuming".to_owned())
    } else { send_pending(app,run,target,wire,id) };
    let notices: Vec<(String,String,String)> = {
        let conn = app.db().conn.lock().unwrap();
        let mut q = conn.prepare("SELECT id,target_id,wire FROM plan_execute_messages WHERE run_id=?1 AND fingerprint=?2").map_err(|e|e.to_string())?;
        let rows = q.query_map(params![run.id,format!("fanout:{id}")],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
        rows.collect::<Result<_,_>>().map_err(|e|e.to_string())?
    };
    let mut receipts = Vec::new();
    let mut failures = Vec::new();
    match &primary {
        Ok(value) => receipts.push(value.clone()),
        Err(error) => failures.push(format!("{target}: {error}")),
    }
    // Attempt every persisted target, including when another target failed. Submission IDs deduplicate retries.
    for (id,target,wire) in notices {
        match send_pending(app,run,&target,&wire,&id) {
            Ok(value) => receipts.push(value),
            Err(error) => failures.push(format!("{target}: {error}")),
        }
    }
    if !failures.is_empty() { return Err(format!("Workflow handoff was saved; retry with the same message ID. {}",failures.join("; "))); }
    let mut result = primary?;
    result["deliveries"] = json!(receipts);
    Ok(result)
}

pub(super) fn integrated(app: &AppCtx, run: &mut Run) -> Result<(), String> {
    ensure_reviewer(app,run)?;
    let id = format!("msg-{}",role_id(&run.id,&format!("integration-review:{}",serde_json::to_string(&split::tasks(app,&run.id)?.iter().map(|t|json!([t["run"]["id"],t["run"]["round"]])).collect::<Vec<_>>()).unwrap())));
    let previous: Option<String> = app.db().conn.lock().unwrap().query_row(
        "SELECT wire FROM plan_execute_messages WHERE id=?1",[&id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
    let wire = if let Some(wire) = previous { wire } else {
        let origin = json!({"name":"VelaTerm","agent":"terminal","role":"system","runId":run.id,"round":run.round});
        let task_list = split::tasks(app,&run.id)?;
        let text = format!("{}\n\nWorkflow ID: {}\nRole: reviewer\nRound: {}\nAll task summaries are complete. Review the complete original request, integration evidence and actual delivery directories. Task approval alone does not prove integrated delivery. Use vflow accept for this overall workflow only after full delivery passes; otherwise vflow block and notify Plan of the precise remaining work. Plan can coordinate a correction by dispatching the appropriate task workflow. Do not edit implementation files.\n\nOriginal task:\n{}\n\nTasks:\n{}",
            protocol(&run.config),run.id,run.round,run.task,serde_json::to_string_pretty(&task_list).unwrap());
        let wire = format!("[VelaTerm message {id}]\n{origin}\n\n{text}");
        let mut conn = app.db().conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e|e.to_string())?;
        tx.execute("UPDATE plan_execute_runs SET state='reviewing',review_status='pending' WHERE id=?1",[&run.id]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO plan_execute_messages(id,run_id,sender_id,target_id,action,round,fingerprint,wire,origin) VALUES (?1,?2,?3,?4,'integration',?5,'integration',?6,?7)",
            params![id,run.id,run.planner_id,run.reviewer_id,run.round,wire,origin.to_string()]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
        wire
    };
    sync_session_states(app);
    if run.review_status != "passed" {
        deliver(app,&get(app,&run.id)?,run.reviewer_id.as_deref().unwrap(),&wire,&id)?;
    }
    Ok(())
}
