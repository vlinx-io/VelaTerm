// Regression scenarios use inert chat peers and missing binaries, never a real model.
fn split_fixture() -> AppCtx {
    let (app, _, _) = plan_execute_fixture();
    match &app {
        AppCtx::Headless(host) => host.set_hooks(crate::agent::server::HookServer {
            port: 0,
            token: "fixture".into(),
        }),
        #[cfg(feature = "gui")]
        _ => unreachable!(),
    }
    {
        let conn = app.db().conn.lock().unwrap();
        conn.execute("UPDATE plan_execute_runs SET executor_id=NULL,config=?1 WHERE id='run'",[json!({"splitTasks":true,"plan":{"agent":"claude","effort":"high"},"exec":{"agent":"claude","model":"default-executor","effort":"medium"}}).to_string()]).unwrap();
        conn.execute("INSERT INTO app_settings(key,value,updated_at) VALUES ('vlx-settings',?1,0)",[json!({"agentDefaults":{"claude":{"path":"/nonexistent/velaterm-split-test-agent"}}}).to_string()]).unwrap();
    }
    app
}

fn split_propose(app: &AppCtx) -> crate::agent::plan_execute::Request {
    let req = flow_request(
        "planner",
        "propose",
        0,
        &json!({"tasks":[
            {"name":"Parser","prompt":"Implement parser validation"},
            {"name":"Settings","prompt":"Implement settings validation"},
            {"name":"Removed","prompt":"This task will be removed"}
        ]})
        .to_string(),
    );
    crate::agent::plan_execute::action(app, &req).unwrap();
    req
}

#[test]
fn plan_execute_rejects_unsupported_agents_before_creating_sessions_or_worktrees() {
    use crate::agent::plan_execute::{self as flow, menu};
    let app = menu_fixture();
    for option in crate::agent::launch_options::catalog().into_iter().filter(|option| !option.supports_plan_execute) {
        for role in ["plan", "exec"] {
            let mut config = json!({"worktreeMode":"each","plan":{"agent":"claude"},"exec":{"agent":"claude"}});
            config[role]["agent"] = json!(option.id);
            let request = json!({"requestId":uuid::Uuid::new_v4().to_string(),"parentSessionId":"parent","prompt":"Task","cwd":"/missing-workflow-repository","worktree":true,"planExecute":config});
            assert!(flow::start(&app, &serde_json::from_value(request).unwrap()).unwrap_err().contains("chat-capable"));
            let request = json!({"requestId":uuid::Uuid::new_v4().to_string(),"context":{"projectId":"p","groupId":"g","parentSessionId":"parent"},"prompt":"Task","cwd":"/missing-workflow-repository","worktree":true,"config":config});
            assert!(menu::start(&app, &serde_json::from_value(request).unwrap()).unwrap_err().contains("chat-capable"));
        }
    }
    let conn = app.db().conn.lock().unwrap();
    assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    assert_eq!(conn.query_row("SELECT count(*) FROM plan_execute_runs", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    drop(conn);
    let dir = app.data_dir().unwrap(); drop(app); std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn split_plan_execute_worktree_modes_cover_planner_executors_and_retries() {
    let _state_guard = crate::session_state::test_lock();
    use crate::agent::plan_execute::{self as flow, menu, split};
    for linked_source in [false, true] {
    for entry in ["menu", "spawn"] {
        for mode in ["none", "shared", "each"] {
            let app = menu_fixture();
            let root = app.data_dir().unwrap().join("repo with spaces 中文");
            std::fs::create_dir(&root).unwrap();
            for args in [vec!["init", "-q"], vec!["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-q", "--allow-empty", "-m", "Fixture"]] {
                assert!(std::process::Command::new("git").args(args).current_dir(&root).status().unwrap().success());
            }
            std::fs::write(root.join("uncommitted.txt"), "preserve").unwrap();
            let source = if linked_source {
                let source = crate::git::worktree_add(root.to_str().unwrap(), "Existing source").unwrap();
                let source = std::path::PathBuf::from(source.path);
                std::fs::write(source.join("source-only.txt"), "committed only on the source branch").unwrap();
                for args in [vec!["add", "source-only.txt"], vec!["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-q", "-m", "Source branch"]] {
                    assert!(std::process::Command::new("git").args(args).current_dir(&source).status().unwrap().success());
                }
                std::fs::write(source.join("uncommitted.txt"), "source dirty").unwrap();
                source
            } else { root.clone() };
            let id = uuid::Uuid::new_v4().to_string();
            let config = json!({"worktreeMode":mode,"splitTasks":true,"plan":{"agent":"claude"},"exec":{"agent":"claude"}});
            // An explicit mode overrides the old boolean, which remains accepted for older clients.
            let input = if entry == "menu" {
                json!({"requestId":id,"context":{"projectId":"p","groupId":"g","parentSessionId":"parent"},"prompt":"Task","cwd":source,"worktree":mode=="none","config":config})
            } else {
                json!({"requestId":id,"parentSessionId":"parent","prompt":"Task","cwd":source,"worktree":mode=="none","planExecute":config,"noConfirm":true})
            };
            let start = || if entry == "menu" {
                menu::start(&app, &serde_json::from_value(input.clone()).unwrap())
            } else {
                flow::start(&app, &serde_json::from_value(input.clone()).unwrap())
            };
            let first = start().unwrap();
            assert_eq!(first["run"]["config"]["worktreeMode"], mode);
            assert_eq!(first["planner"]["engine"], "chat");
            let planner = first["planner"]["id"].as_str().unwrap();
            let cwd = first["planner"]["cwd"].as_str().unwrap();
            assert_eq!(cwd == source.to_str().unwrap(), mode == "none");
            assert_eq!(std::path::Path::new(cwd).join("source-only.txt").exists(), linked_source);
            if mode != "none" { assert_eq!(std::fs::canonicalize(std::path::Path::new(cwd).parent().unwrap()).unwrap(), std::fs::canonicalize(root.join(".vlx-worktrees")).unwrap()); }
            assert_eq!(first["planner"]["worktreePath"].is_null(), mode == "none");
            assert_eq!(std::path::Path::new(cwd).join("uncommitted.txt").exists(), mode == "none");
            assert_eq!(start().unwrap()["planner"]["id"], planner);
            menu_peer(&app, planner);
            start().unwrap();
            // Executors must fork the planner's current commit, not the source or main branch.
            std::fs::write(std::path::Path::new(cwd).join("planner-commit.txt"), "committed by planner").unwrap();
            for args in [vec!["add", "planner-commit.txt"], vec!["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-q", "-m", "Planner commit"]] {
                assert!(std::process::Command::new("git").args(args).current_dir(cwd).status().unwrap().success());
            }
            let mut request = flow_request(planner, "propose", 0, &json!({"tasks":[
                {"name":"Parser","prompt":"Implement parser"}, {"name":"Settings","prompt":"Implement settings"}
            ]}).to_string());
            request.run_id = id.clone();
            flow::action(&app, &request).unwrap();
            let proposal = split::read(&app, &id).unwrap();
            assert_eq!(proposal["run"]["config"]["worktreeMode"], mode);
            assert_eq!(proposal["run"]["state"], "awaiting_confirmation");
            assert_eq!(proposal["state"], "pending");
            assert_eq!(proposal["executionTasks"], json!([]));
            assert_eq!(app.db().conn.lock().unwrap().query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, i64>(0)).unwrap(), 2,
                "even --yes must create only the parent and planner before task confirmation");
            let mut bypass = flow_request(planner, "dispatch", 1, "Skip the final review");
            bypass.run_id = id.clone();
            assert!(flow::action(&app, &bypass).is_err(), "launch confirmation cannot authorize execution");
            let confirmation: split::Confirmation = serde_json::from_value(json!({"runId":id,"proposalId":request.message_id,"tasks":proposal["tasks"]})).unwrap();
            if mode == "each" {
                // A creation failure must not start executors in the planner's directory.
                let missing = root.join("missing");
                app.db().conn.lock().unwrap().execute("UPDATE sessions SET cwd=?2 WHERE id=?1", rusqlite::params![planner, missing.to_str().unwrap()]).unwrap();
                let failed = split::confirm(&app, &confirmation).unwrap();
                assert_eq!(failed["errors"].as_array().unwrap().len(), 2);
                assert!(failed["tasks"].as_array().unwrap().iter().all(|t| t["run"]["executorId"].is_null()));
                app.db().conn.lock().unwrap().execute("UPDATE sessions SET cwd=?2 WHERE id=?1", rusqlite::params![planner, cwd]).unwrap();
            }
            let launched = split::confirm(&app, &confirmation).unwrap();
            let mut directories = std::collections::HashSet::from([cwd.to_owned()]);
            let mut executor_ids = Vec::new();
            for task in launched["tasks"].as_array().unwrap() {
                let executor_id = task["run"]["executorId"].as_str().unwrap();
                executor_ids.push(executor_id.to_owned());
                assert_eq!(task["run"]["config"]["worktreeMode"], mode);
                let executor = crate::db::repo::get_session(&app.db().conn.lock().unwrap(), executor_id).unwrap().unwrap();
                assert_eq!(executor.engine, "chat");
                let execution_cwd = executor.cwd.as_deref().unwrap();
                assert!(std::path::Path::new(execution_cwd).join("planner-commit.txt").exists());
                assert_eq!(execution_cwd == cwd, mode != "each");
                assert_eq!(executor.worktree_path.is_some(), mode == "each");
                assert_eq!(task["executor"]["cwd"], execution_cwd);
                if mode == "each" {
                    assert_eq!(executor.worktree_path.as_deref(), Some(execution_cwd));
                    assert_eq!(std::path::Path::new(execution_cwd).parent(), std::path::Path::new(cwd).parent());
                    assert!(!std::path::Path::new(execution_cwd).join("uncommitted.txt").exists());
                    assert!(!executor.worktree_base_ref.as_deref().unwrap().is_empty());
                }
                directories.insert(execution_cwd.to_owned());
                menu_peer(&app, executor_id);
            }
            assert_eq!(directories.len(), if mode == "each" { 3 } else { 1 });
            let retried = split::confirm(&app, &confirmation).unwrap();
            assert_eq!(retried["errors"], json!([]));
            for (task, executor) in retried["tasks"].as_array().unwrap().iter().zip(&executor_ids) {
                assert_eq!(task["run"]["executorId"], *executor);
            }
            let worktrees = crate::git::worktree_list(root.to_str().unwrap()).unwrap();
            assert_eq!(worktrees.len(), usize::from(linked_source) + match mode { "none" => 1, "shared" => 2, _ => 4 });
            assert_eq!(std::fs::read_to_string(root.join("uncommitted.txt")).unwrap(), "preserve");
            for executor in &executor_ids { app.chat().stop(&app, executor).unwrap(); }
            app.chat().stop(&app, planner).unwrap();
            let dir = app.data_dir().unwrap();
            drop(app);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
    }
}

#[test]
fn split_plan_execute_confirmation_reports_and_independent_corrections() {
    let _state_guard = crate::session_state::test_lock();
    use crate::agent::plan_execute::{self as flow, split};
    let app = split_fixture();
    {
        let conn = app.db().conn.lock().unwrap();
        conn.execute("INSERT INTO plan_execute_messages(id,run_id,sender_id,target_id,action,round,fingerprint,wire,origin) VALUES ('msg-run','run','owner','planner','start',0,'start','Task','{}')", []).unwrap();
        conn.execute(
            "INSERT INTO plan_execute_attachments(message_id,images) VALUES ('msg-run',?1)",
            [json!([{"mimeType":"image/png","data":"AQID"}]).to_string()],
        )
        .unwrap();
    }
    let proposal = split_propose(&app);
    assert_eq!(split::pending(&app).unwrap(), json!(["run"]));
    assert_eq!(split::read(&app, "run").unwrap()["state"], "pending");
    let before: u32 = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, 3);
    assert!(flow::action(
        &app,
        &flow_request("planner", "dispatch", 1, "Bypass confirmation")
    )
    .is_err());
    assert!(flow::action(
        &app,
        &flow_request("planner", "accept", 0, "Premature success")
    )
    .is_err());
    flow::action(&app, &proposal).unwrap();
    let mut changed = flow_request("planner", "propose", 0, "{\"tasks\":[]}");
    changed.message_id = proposal.message_id.clone();
    assert!(flow::action(&app, &changed).is_err());

    let mut request: split::Confirmation = serde_json::from_value(json!({"runId":"run","proposalId":proposal.message_id,"tasks":[
        {"name":"Approved parser","prompt":"User-edited parser task","config":{"agent":"claude","model":"approved-model","effort":"low"}},
        {"name":"Settings","prompt":"User-edited settings task"}
    ]})).unwrap();
    let first = split::confirm(&app, &request).unwrap();
    assert_eq!(first["tasks"].as_array().unwrap().len(), 2);
    assert!(!first["errors"].as_array().unwrap().is_empty());
    let ids: Vec<String> = first["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["run"]["id"].as_str().unwrap().to_owned())
        .collect();
    let peers: Vec<String> = first["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["run"]["executorId"].as_str().unwrap().to_owned())
        .collect();
    assert_ne!(peers[0], peers[1]);
    let original_peers = peers.clone();
    for id in &peers {
        menu_peer(&app, id);
    }
    let retried = split::confirm(&app, &request).unwrap();
    assert_eq!(retried["errors"], json!([]));
    assert_eq!(split::pending(&app).unwrap(), json!([]));
    for (index, id) in ids.iter().enumerate() {
        let mut status = flow_request("planner", "status", 0, "");
        status.run_id = id.clone();
        let status = flow::action(&app, &status).unwrap();
        assert_eq!(status["run"]["round"], 1);
        assert_eq!(status["run"]["plannerId"], "planner");
        assert_eq!(status["run"]["executorId"], peers[index]);
        assert_eq!(status["parentRunId"], "run");
        assert_eq!(status["run"]["state"], "executing");
    }
    for peer in &peers {
        let snapshot = serde_json::to_value(app.chat().snapshot(peer)).unwrap();
        assert_eq!(
            app.chat()
                .attachment(
                    peer,
                    snapshot["rows"][0]["images"][0]["attachmentId"]
                        .as_str()
                        .unwrap()
                )
                .unwrap()
                .data,
            "AQID"
        );
    }
    let first_session = crate::db::repo::get_session(&app.db().conn.lock().unwrap(), &peers[0])
        .unwrap()
        .unwrap();
    assert_eq!(first_session.name, "Approved parser");
    let selection =
        crate::agent::session_settings::stored(&app.db().conn.lock().unwrap(), &peers[0])
            .unwrap()
            .unwrap()
            .0;
    assert_eq!(selection.model.as_deref(), Some("approved-model"));
    assert_eq!(selection.effort.as_deref(), Some("low"));
    request.tasks[0].prompt = "A conflicting confirmation".into();
    assert!(split::confirm(&app, &request).is_err());
    request.tasks[0].prompt = "User-edited parser task".into();

    let report = |index: usize, round| crate::agent::tell::Request {
        session_id: peers[index].clone(),
        target: None,
        report: true,
        steer: false,
        round: Some(round),
        message_id: format!("msg-{}", uuid::Uuid::new_v4()),
        text: format!("Task {index}, round {round}, review evidence"),
    };
    let a = crate::agent::tell::send(&app, &report(0, 1)).unwrap();
    assert_eq!(a["targetSessionId"], "planner");
    assert_eq!(a["run"]["id"], ids[0]);
    finish_turn(app.chat(), &app, &peers[0], "success");
    let mut correction = flow_request("planner", "dispatch", 2, "Correct parser finding F-1");
    correction.run_id = ids[0].clone();
    assert_eq!(
        flow::action(&app, &correction).unwrap()["targetSessionId"],
        peers[0]
    );
    let overall = flow::action(&app, &flow_request("planner", "status", 0, "")).unwrap();
    assert_eq!(overall["tasks"][0]["run"]["round"], 2);
    assert_eq!(overall["tasks"][1]["run"]["round"], 1);
    assert!(crate::agent::tell::send(&app, &report(0, 1)).is_err());
    assert_eq!(
        crate::agent::tell::send(&app, &report(1, 1)).unwrap()["run"]["id"],
        ids[1]
    );
    let mut accept = flow_request("planner", "accept", 1, "Settings verified");
    accept.run_id = ids[1].clone();
    assert_eq!(flow::action(&app, &accept).unwrap()["delivery"], "recorded");
    assert!(flow::action(&app, &flow_request("planner", "accept", 1, "Too soon")).is_err());
    crate::agent::tell::send(&app, &report(0, 2)).unwrap();
    accept = flow_request("planner", "accept", 2, "Parser corrections verified");
    accept.run_id = ids[0].clone();
    flow::action(&app, &accept).unwrap();
    // Replaying confirmation does not resend completed work or replace executors.
    let replay = split::confirm(&app, &request).unwrap();
    for (i, peer) in original_peers.iter().enumerate() {
        assert_eq!(replay["tasks"][i]["run"]["executorId"], *peer);
    }
    assert_eq!(
        flow::action(
            &app,
            &flow_request("planner", "accept", 1, "All original requirements verified")
        )
        .unwrap()["run"]["state"],
        "completed"
    );
    let count: u32 = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, before + 2);
    for id in ["planner", "executor"]
        .into_iter()
        .chain(peers.iter().map(String::as_str))
    {
        app.chat().stop(&app, id).unwrap();
    }
}

#[test]
fn split_plan_execute_cancel_and_stop_preserve_confirmation_boundary() {
    let _state_guard = crate::session_state::test_lock();
    use crate::agent::plan_execute::{self as flow, split};
    let app = split_fixture();
    let proposed = split_propose(&app);
    assert!(split::cancel(&app, "run", "stale-id").is_err());
    split::cancel(&app, "run", &proposed.message_id).unwrap();
    split::cancel(&app, "run", &proposed.message_id).unwrap();
    assert_eq!(split::read(&app, "run").unwrap()["run"]["state"], "blocked");
    let confirm:split::Confirmation=serde_json::from_value(json!({"runId":"run","proposalId":proposed.message_id,"tasks":[{"name":"Task","prompt":"Task"}]})).unwrap();
    assert!(split::confirm(&app, &confirm).is_err());
    let proposed = split_propose(&app);
    flow::action(&app, &flow_request("owner", "stop", 0, "")).unwrap();
    assert_eq!(split::pending(&app).unwrap(), json!([]));
    assert_eq!(split::read(&app, "run").unwrap()["state"], "stopped");
    let mut confirm = confirm;
    confirm.proposal_id = proposed.message_id;
    assert!(split::confirm(&app, &confirm).is_err());
    assert!(flow::action(
        &app,
        &flow_request("planner", "propose", 0, "{\"tasks\":[]}")
    )
    .is_err());
    let count: u32 = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3);
    for id in ["planner", "executor"] {
        app.chat().stop(&app, id).unwrap();
    }
}

#[test]
fn split_plan_execute_missing_report_and_stop_are_isolated_per_task() {
    let _state_guard = crate::session_state::test_lock();
    use crate::agent::plan_execute::{self as flow, split};
    let app = split_fixture();
    let proposal = split_propose(&app);
    let confirmation: split::Confirmation =
        serde_json::from_value(json!({"runId":"run","proposalId":proposal.message_id,
        "tasks":[{"name":"Parser","prompt":"Parser"},{"name":"Settings","prompt":"Settings"}]}))
        .unwrap();
    let first = split::confirm(&app, &confirmation).unwrap();
    let ids: Vec<String> = first["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["run"]["id"].as_str().unwrap().to_owned())
        .collect();
    let peers: Vec<String> = first["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["run"]["executorId"].as_str().unwrap().to_owned())
        .collect();
    for peer in &peers {
        menu_peer(&app, peer);
    }
    split::confirm(&app, &confirmation).unwrap();
    finish_turn(app.chat(), &app, &peers[0], "success");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let status = flow::action(&app, &flow_request("planner", "status", 0, "")).unwrap();
        if status["tasks"][0]["run"]["state"] == "blocked" {
            assert_eq!(status["tasks"][1]["run"]["state"], "executing");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Missing task report must block only that task"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let snapshot = app.chat().snapshot("planner");
    assert!(
        snapshot
            .queue
            .iter()
            .any(|item| item.text.contains(&ids[0]) && item.text.contains("is blocked")),
        "The shared planner must receive the system notice"
    );
    let mut blocked = flow_request(&peers[0], "block", 1, "Need clarification for this task");
    blocked.run_id = ids[0].clone();
    assert_eq!(flow::action(&app, &blocked).unwrap()["delivery"], "queued");
    let mut stop = flow_request("planner", "stop", 0, "");
    stop.run_id = ids[0].clone();
    flow::action(&app, &stop).unwrap();
    assert!(app.chat().turn_in_progress("planner"));
    assert!(app.chat().turn_in_progress(&peers[1]));
    let report = crate::agent::tell::Request {
        session_id: peers[1].clone(),
        target: None,
        report: true,
        steer: false,
        round: Some(1),
        message_id: format!("msg-{}", uuid::Uuid::new_v4()),
        text: "Settings verified".into(),
    };
    crate::agent::tell::send(&app, &report).unwrap();
    let mut accept = flow_request("planner", "accept", 1, "Settings accepted");
    accept.run_id = ids[1].clone();
    flow::action(&app, &accept).unwrap();
    finish_turn(app.chat(), &app, &peers[1], "success");
    crate::command_core::chat_send(
        &app,
        &peers[1],
        "Independent user task after acceptance",
        vec![],
        None,
        None,
    )
    .unwrap();
    flow::action(&app, &flow_request("owner", "stop", 0, "")).unwrap();
    assert!(app.chat().turn_in_progress(&peers[1]), "Stopping the overall workflow must leave an already accepted executor's later user work alone");
    let status = flow::action(&app, &flow_request("owner", "status", 0, "")).unwrap();
    assert_eq!(status["tasks"][0]["run"]["state"], "stopped");
    assert_eq!(status["tasks"][1]["run"]["state"], "completed");
    assert!(split::confirm(&app, &confirmation).is_err());
    for id in ["planner", "executor"]
        .into_iter()
        .chain(peers.iter().map(String::as_str))
    {
        app.chat().stop(&app, id).unwrap();
    }
}
