//! User-created planning workflows without an extra initiating conversation.

use super::*;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub project_id: String,
    pub group_id: Option<String>,
    pub parent_session_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub request_id: String,
    pub context: Context,
    pub prompt: String,
    #[serde(default)]
    pub images: Vec<ChatImage>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub worktree: bool,
    pub config: Config,
}

struct Placement {
    parent: Option<Session>,
    cwd: Option<String>,
    group_id: Option<String>,
    inherited_worktree: Option<String>,
    inherited_base: Option<String>,
    names: Vec<String>,
}

fn placement(app: &AppCtx, context: &Context) -> Result<Placement, String> {
    let tree = repo::list_tree(&app.db().conn.lock().unwrap())?;
    let project = tree
        .projects
        .iter()
        .find(|p| p.id == context.project_id)
        .ok_or("The project no longer exists")?;
    let parent = context
        .parent_session_id
        .as_deref()
        .map(|id| session(app, id))
        .transpose()?;
    if parent
        .as_ref()
        .is_some_and(|p| p.project_id != context.project_id || p.group_id != context.group_id)
    {
        return Err(
            "The parent session no longer belongs to the selected project and group".into(),
        );
    }
    let group = context
        .group_id
        .as_deref()
        .map(|id| {
            tree.groups
                .iter()
                .find(|g| g.id == id && g.project_id == context.project_id)
                .ok_or("The selected group no longer exists in this project")
        })
        .transpose()?;
    let mut names = vec![project.name.clone()];
    if let Some(group) = group {
        names.push(group.name.clone());
    }
    if let Some(parent) = &parent {
        names.push(parent.name.clone());
    }
    // A nested group runs in the worktree bound on it or its nearest ancestor group.
    let bound = tree.worktree_group(context.group_id.as_deref());
    let inherited_worktree = parent
        .as_ref()
        .and_then(|p| p.worktree_path.clone())
        .or_else(|| bound.and_then(|g| g.worktree_path.clone()));
    let inherited_base = parent
        .as_ref()
        .and_then(|p| p.worktree_base_ref.clone())
        .or_else(|| bound.and_then(|g| g.worktree_base_ref.clone()));
    let cwd = parent
        .as_ref()
        .and_then(|p| p.cwd.clone())
        .or(inherited_worktree.clone())
        .or_else(|| (!project.root_path.trim().is_empty()).then(|| project.root_path.clone()));
    Ok(Placement {
        parent,
        cwd,
        group_id: context.group_id.clone(),
        inherited_worktree,
        inherited_base,
        names,
    })
}

fn defaults(app: &AppCtx, config: &Config, parent: Option<&Session>) -> Result<Config, String> {
    if let Some(parent) = parent {
        return serde_json::from_value(super::defaults(app, &parent.id, config)?)
            .map_err(|e| e.to_string());
    }
    let settings = repo::get_app_settings(&app.db().conn.lock().unwrap())?
        .remove("vlx-settings")
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or_default();
    let role = |input: &RoleConfig, fallback: SessionKind| -> Result<RoleConfig, String> {
        let kind = input.agent.unwrap_or(fallback);
        if !supported(kind) {
            return Err("Planning and execution require chat-capable agents".into());
        }
        let key = kind.as_str();
        let selection = super::super::session_settings::from_args(
            kind,
            settings["agentDefaults"][key]["args"].as_str(),
        );
        let model = input.model.clone().or(selection.model).or_else(|| {
            settings["chatModelByKind"][key]
                .as_str()
                .or_else(|| {
                    (kind == SessionKind::Claude)
                        .then(|| settings["chatModel"].as_str())
                        .flatten()
                })
                .map(str::to_owned)
        });
        let model_key = model.as_deref().unwrap_or("");
        let effort = input.effort.clone().or(selection.effort).or_else(|| {
            settings["chatEffortByModel"][format!("{key}:{model_key}")]
                .as_str()
                .or_else(|| {
                    (kind == SessionKind::Claude)
                        .then(|| settings["chatEffortByModel"][model_key].as_str())
                        .flatten()
                })
                .map(str::to_owned)
        });
        Ok(RoleConfig {
            agent: Some(kind),
            model,
            effort,
        })
    };
    let plan = role(&config.plan, SessionKind::Claude)?;
    let exec = role(&config.exec, plan.agent.unwrap())?;
    let review = if config.review_enabled != Some(true) && config.review.agent.is_some_and(|a| !supported(a)) { config.review.clone() } else { role(&config.review,plan.agent.unwrap())? };
    Ok(Config { plan, exec, review, review_enabled:Some(config.review_enabled.unwrap_or(false)), ..config.clone() })
}

pub fn prepare(app: &AppCtx, context: &Context) -> Result<Value, String> {
    let placement = placement(app, context)?;
    Ok(
        json!({"config":defaults(app,&Config { worktree_mode: Some(WorktreeMode::None), ..Default::default() },placement.parent.as_ref())?,"cwd":placement.cwd,
        "worktree":false,"locationNames":placement.names}),
    )
}

pub fn start(app: &AppCtx, req: &Request) -> Result<Value, String> {
    let _guard = operation_lock().lock().unwrap();
    uuid::Uuid::parse_str(&req.request_id).map_err(|_| "Invalid workflow request ID")?;
    if req.prompt.trim().is_empty() {
        return Err("A workflow needs a task".into());
    }
    let placement = placement(app, &req.context)?;
    let existing = get(app,&req.request_id).ok();
    let mut config = defaults(app, &req.config, placement.parent.as_ref())?;
    if existing.as_ref().is_some_and(|r|r.config.review_enabled.is_none()) && req.config.review_enabled.is_none() {
        config.review_enabled = None; config.review = RoleConfig::default();
    }
    validate_config(&config)?;
    core::check_images(&req.images)?;
    let request = json!({"context":req.context,"prompt":req.prompt,"cwd":req.cwd,"worktree":req.worktree,"config":config});
    if let Ok(existing) = get(app, &req.request_id) {
        let saved: Option<String> = app
            .db()
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT request FROM plan_execute_menu_launches WHERE run_id=?1",
                [&req.request_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if message_images(app, &format!("msg-{}", req.request_id))? != req.images {
            return Err("This request ID already has different task images".into());
        }
        if saved
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .as_ref()
            != Some(&request)
        {
            return Err("This request ID already has a different creation location, task or launch configuration".into());
        }
        if existing.state == "blocked" && existing.round == 0 {
            bootstrap(app, &existing)?;
        }
        return Ok(
            json!({"planner":session(app,&existing.planner_id)?,"run":brief(&get(app,&existing.id)?)}),
        );
    }
    let selected = req
        .cwd
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or(placement.cwd);
    let cwd = selected.ok_or("Select a working directory")?;
    let path = std::path::Path::new(&cwd);
    if !path.is_absolute() || !path.is_dir() {
        return Err("Select an existing absolute working directory".into());
    }
    let id = &req.request_id;
    let inherited = placement.inherited_worktree.as_deref().filter(|w| *w == cwd);
    let planner = create_role_bound(app, &role_id(id, "planner"), &req.context.project_id,
        placement.group_id.as_deref(), placement.parent.as_ref(), "Plan · Execute", &config.plan, Some(&cwd),
        config.worktree_mode(req.worktree) != WorktreeMode::None, inherited,
        inherited.and(placement.inherited_base.as_deref()), |tx, planner| {
            let message_id = format!("msg-{id}");
            let directory = planner.cwd.as_deref().unwrap_or("");
            let task=format!("{}\n\nWorkflow ID: {id}\nRole: planner\nWorking directory: {directory}\nThis workflow was created by the user from the New Session menu. There is no initiating conversation; present the final delivery summary here.\n\nUser task:\n{}",
                review::protocol(&config),req.prompt);
            let origin = json!({"role":"user"});
            let wire = format!("[VelaTerm message {message_id}]\n{origin}\n\n{task}").trim_end().to_owned();
            tx.execute("INSERT INTO plan_execute_runs(id,owner_id,planner_id,config,task,state,review_status) VALUES (?1,?2,?2,?3,?4,'planning',?5)",
                params![id,planner.id,serde_json::to_string(&config).map_err(|e|e.to_string())?,req.prompt,config.initial_review_status()]).map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO plan_execute_menu_launches(run_id,request) VALUES (?1,?2)",
                params![id,request.to_string()]).map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO plan_execute_messages(id,run_id,sender_id,target_id,action,round,fingerprint,wire,origin) VALUES (?1,?2,?3,?3,'start',0,'user',?4,?5)",
                params![message_id,id,planner.id,wire,origin.to_string()]).map_err(|e|e.to_string())?;
            save_images(tx, &message_id, &req.images)
        })?;
    bootstrap(app, &get(app, id)?)?;
    Ok(json!({"planner":planner,"run":brief(&get(app,id)?)}))
}
