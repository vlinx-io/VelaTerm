// Optional review regressions use inert peers and explicitly missing provider binaries.
fn review_fixture(enabled: bool) -> AppCtx {
    let (app, _, _) = plan_execute_fixture();
    match &app {
        AppCtx::Headless(host) => host.set_hooks(crate::agent::server::HookServer {port:0,token:"fixture".into()}),
        #[cfg(feature="gui")] _ => unreachable!(),
    }
    {
        let conn = app.db().conn.lock().unwrap();
        conn.execute("UPDATE plan_execute_runs SET config=?1,review_status=?2 WHERE id='run'",rusqlite::params![
            json!({"reviewEnabled":enabled,"plan":{"agent":"claude"},"exec":{"agent":"claude"},"review":{"agent":"claude","model":"review-model","effort":"high"}}).to_string(),
            if enabled {"pending"} else {"not_enabled"}]).unwrap();
        conn.execute("INSERT INTO app_settings(key,value,updated_at) VALUES ('vlx-settings',?1,0)",
            [json!({"agentDefaults":{"claude":{"path":"/nonexistent/velaterm-review-test-agent"}}}).to_string()]).unwrap();
    }
    app
}

fn review_cleanup(app: AppCtx) {
    let ids: Vec<String> = app.chat().sessions.lock().unwrap().keys().cloned().collect();
    for id in ids { app.chat().stop(&app,&id).unwrap(); }
    let dir = app.data_dir().unwrap(); drop(app); std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn optional_review_fans_out_retries_failed_target_and_separates_acceptance_from_finish() {
    let _state_guard = crate::session_state::test_lock();
    use crate::agent::plan_execute as flow;
    let app = review_fixture(true);
    flow::action(&app,&flow_request("planner","dispatch",1,"Implement")).unwrap();
    let report = flow_request("executor","report",1,"Delivered /tmp/result; evidence /tmp/check.log; no remaining work");
    assert!(tell_report(&app,&report).unwrap_err().contains("saved"));
    let status = flow::action(&app,&flow_request("planner","status",0,"")).unwrap();
    let reviewer = status["run"]["reviewerId"].as_str().unwrap().to_owned();
    assert_ne!(reviewer,"planner");
    assert_eq!(status["run"]["state"],"reviewing");
    let saved = crate::db::repo::get_session(&app.db().conn.lock().unwrap(),&reviewer).unwrap().unwrap();
    assert_eq!(saved.parent_session_id.as_deref(),Some("planner"));
    assert_eq!(crate::agent::session_settings::stored(&app.db().conn.lock().unwrap(),&reviewer).unwrap().unwrap().0.model.as_deref(),Some("review-model"));
    let before = app.chat().snapshot("planner").rows.len();
    let notice = serde_json::to_value(app.chat().snapshot("planner")).unwrap();
    assert!(notice["rows"].as_array().unwrap().iter().any(|r|r["text"].as_str().unwrap_or("").contains(&report.message_id)));
    assert!(!notice.to_string().contains("Delivered /tmp/result"));
    menu_peer(&app,&reviewer);
    let delivery = tell_report(&app,&report).unwrap();
    assert_eq!(delivery["deliveries"].as_array().unwrap().len(),2);
    tell_report(&app,&report).unwrap();
    assert_eq!(app.chat().snapshot("planner").rows.len(),before);
    assert_eq!(app.chat().snapshot(&reviewer).rows.len(),1);
    let mut read = flow_request("planner","read-report",1,""); read.message_id = report.message_id.clone();
    assert!(flow::action(&app,&read).unwrap()["wire"].as_str().unwrap().contains("Delivered /tmp/result"));
    assert!(flow::action(&app,&flow_request("planner","accept",1,"Bypass")).is_err());
    assert!(flow::action(&app,&flow_request("planner","finish",1,"Bypass")).is_err());
    assert!(flow::action(&app,&flow_request("executor","accept",1,"Bypass")).is_err());
    let correction = flow_request(&reviewer,"dispatch",2,"Fix validation; check /tmp/result");
    let corrected = flow::action(&app,&correction).unwrap();
    assert_eq!(corrected["run"]["reviewStatus"],"changes_requested");
    assert_eq!(corrected["run"]["executorId"],"executor");
    assert_eq!(corrected["deliveries"].as_array().unwrap().len(),2);
    assert!(tell_report(&app,&report).unwrap_err().contains("earlier"));
    tell_report(&app,&flow_request("executor","report",2,"Corrected; evidence new-check.log")).unwrap();
    assert_eq!(flow::action(&app,&flow_request(&reviewer,"accept",2,"Inspected changes and evidence; passed")).unwrap()["run"]["state"],"summarizing");
    assert!(flow::action(&app,&flow_request("executor","finish",2,"Bypass")).is_err());
    let done = flow::action(&app,&flow_request("planner","finish",2,"Delivered and independently reviewed")).unwrap();
    assert_eq!(done["run"]["state"],"completed");
    assert_eq!(done["run"]["reviewStatus"],"passed");
    assert_eq!(app.db().conn.lock().unwrap().query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),4);
    review_cleanup(app);
}

#[test]
fn optional_review_disabled_reports_and_supplements_go_to_plan_without_review_role() {
    let _state_guard = crate::session_state::test_lock();
    use crate::agent::plan_execute as flow;
    let app = review_fixture(false);
    flow::action(&app,&flow_request("planner","dispatch",1,"Implement")).unwrap();
    let first = tell_report(&app,&flow_request("executor","report",1,"Partial: remaining delivery step")).unwrap();
    assert_eq!(first["targetSessionId"],"planner");
    assert_eq!(first["run"]["state"],"summarizing");
    assert_eq!(first["run"]["reviewStatus"],"not_enabled");
    tell_report(&app,&flow_request("executor","report",1,"Supplemented missing evidence path")).unwrap();
    flow::action(&app,&flow_request("planner","dispatch",2,"Complete reported delivery step")).unwrap();
    assert!(flow::action(&app,&flow_request("planner","finish",2,"No report yet")).is_err());
    assert!(flow::action(&app,&flow_request("planner","accept",2,"Must not review")).is_err());
    flow::action(&app,&flow_request("executor","block",2,"Missing input")).unwrap();
    tell_report(&app,&flow_request("executor","report",2,"Delivered; verification evidence /tmp/output.log")).unwrap();
    let done = flow::action(&app,&flow_request("planner","finish",2,"Collected reported deliverables")).unwrap();
    assert!(done["run"]["summary"].as_str().unwrap().contains("No independent review"));
    assert!(done["run"]["reviewerId"].is_null());
    assert_eq!(app.db().conn.lock().unwrap().query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),3);
    review_cleanup(app);
}

#[test]
fn optional_review_split_shares_reviewer_and_requires_integrated_delivery_review() {
    let _state_guard = crate::session_state::test_lock();
    use crate::agent::plan_execute::{self as flow,split};
    for enabled in [false,true] {
        let app = review_fixture(enabled);
        app.db().conn.lock().unwrap().execute("UPDATE plan_execute_runs SET executor_id=NULL,config=json_set(config,'$.splitTasks',json('true')) WHERE id='run'",[]).unwrap();
        let proposal = split_propose(&app);
        let tasks = split::read(&app,"run").unwrap()["tasks"].as_array().unwrap()[..2].to_vec();
        let confirmation = serde_json::from_value(json!({"runId":"run","proposalId":proposal.message_id,"tasks":tasks})).unwrap();
        let launch = split::confirm(&app,&confirmation).unwrap();
        for task in launch["tasks"].as_array().unwrap() {
            menu_peer(&app,task["run"]["executorId"].as_str().unwrap());
        }
        split::confirm(&app,&confirmation).unwrap();
        let mut reviewer = None;
        for task in launch["tasks"].as_array().unwrap() {
            let id = task["run"]["id"].as_str().unwrap();
            let exec = task["run"]["executorId"].as_str().unwrap();
            let mut req = flow_request(exec,"report",1,"Delivered task, evidence and integration output"); req.run_id = id.into();
            let report = tell_report(&app,&req);
            if enabled {
                if reviewer.is_none() {
                    assert!(report.is_err());
                    let status = flow::action(&app,&flow_request("planner","status",0,"")).unwrap();
                    reviewer = Some(status["run"]["reviewerId"].as_str().unwrap().to_owned());
                    menu_peer(&app,reviewer.as_deref().unwrap());
                    tell_report(&app,&req).unwrap();
                } else { report.unwrap(); }
                let mut accept = flow_request(reviewer.as_deref().unwrap(),"accept",1,"Task reviewed"); accept.run_id = id.into();
                flow::action(&app,&accept).unwrap();
            } else { report.unwrap(); }
            let mut finish = flow_request("planner","finish",1,"Task summary"); finish.run_id = id.into();
            flow::action(&app,&finish).unwrap();
        }
        let status = flow::action(&app,&flow_request("planner","status",0,"")).unwrap();
        assert_eq!(status["run"]["state"],if enabled {"reviewing"} else {"summarizing"});
        if enabled {
            assert!(flow::action(&app,&flow_request("planner","finish",1,"Too soon")).is_err());
            for task in status["tasks"].as_array().unwrap() { assert_eq!(task["run"]["reviewerId"],reviewer.as_deref().unwrap()); }
            flow::action(&app,&flow_request(reviewer.as_deref().unwrap(),"accept",1,"Integrated delivery reviewed")).unwrap();
            // New work invalidates both task and overall acceptance and reuses the assigned peers.
            let id = status["tasks"][0]["run"]["id"].as_str().unwrap();
            let exec = status["tasks"][0]["run"]["executorId"].as_str().unwrap();
            let mut fix = flow_request(reviewer.as_deref().unwrap(),"dispatch",2,"Integrated defect: fix interface"); fix.run_id = id.into();
            flow::action(&app,&fix).unwrap();
            assert!(flow::action(&app,&flow_request("planner","finish",1,"Stale pass")).is_err());
            let mut report = flow_request(exec,"report",2,"Fixed interface; new integration evidence"); report.run_id = id.into(); tell_report(&app,&report).unwrap();
            let mut pass = flow_request(reviewer.as_deref().unwrap(),"accept",2,"Interface reviewed"); pass.run_id = id.into(); flow::action(&app,&pass).unwrap();
            let mut finish = flow_request("planner","finish",2,"Corrected task summary"); finish.run_id = id.into(); flow::action(&app,&finish).unwrap();
            flow::action(&app,&flow_request(reviewer.as_deref().unwrap(),"accept",1,"New integrated delivery reviewed")).unwrap();
        }
        let done = flow::action(&app,&flow_request("planner","finish",1,"Complete original request collected")).unwrap();
        assert_eq!(done["run"]["state"],"completed");
        review_cleanup(app);
    }
}

#[test]
fn optional_review_schema_migration_preserves_legacy_records() {
    use crate::agent::plan_execute as flow;
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE plan_execute_runs(id TEXT PRIMARY KEY,owner_id TEXT,planner_id TEXT,executor_id TEXT,config TEXT,task TEXT,state TEXT,round INTEGER,summary TEXT); INSERT INTO plan_execute_runs VALUES ('legacy','owner','plan','exec','{}','Task','reviewing',2,'Original report');").unwrap();
    flow::migrate(&conn).unwrap(); flow::migrate(&conn).unwrap();
    let saved: (Option<String>,String,String) = conn.query_row("SELECT reviewer_id,review_status,summary FROM plan_execute_runs",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(saved,(None,"legacy".into(),"Original report".into()));
}

#[test]
fn optional_review_retries_only_missing_plan_notices_after_review_advanced_the_round() {
    let _state_guard = crate::session_state::test_lock();
    use crate::agent::plan_execute as flow;
    let app = review_fixture(true);
    flow::action(&app,&flow_request("planner","dispatch",1,"Implement")).unwrap();
    let initial = flow_request("executor","report",1,"Initial result");
    assert!(tell_report(&app,&initial).is_err());
    let status = flow::action(&app,&flow_request("planner","status",0,"")).unwrap();
    let reviewer = status["run"]["reviewerId"].as_str().unwrap().to_owned();
    menu_peer(&app,&reviewer); tell_report(&app,&initial).unwrap();
    app.chat().stop(&app,"planner").unwrap();
    app.db().conn.lock().unwrap().execute("UPDATE sessions SET agent_path='/missing/review-plan-fixture' WHERE id='planner'",[]).unwrap();
    let supplement = flow_request("executor","report",1,"Supplemental current-round evidence");
    assert!(tell_report(&app,&supplement).unwrap_err().contains("saved"));
    let correction = flow_request(&reviewer,"dispatch",2,"Fix the reported problem");
    assert!(flow::action(&app,&correction).is_err());
    assert_eq!(flow::action(&app,&flow_request("executor","status",0,"")).unwrap()["run"]["round"],2);
    let before = serde_json::to_value(app.chat().snapshot(&reviewer)).unwrap();
    menu_peer(&app,"planner");
    let result = tell_report(&app,&supplement).unwrap();
    assert_eq!(result["run"]["round"],2);
    assert_eq!(serde_json::to_value(app.chat().snapshot(&reviewer)).unwrap()["rows"],before["rows"]);
    assert_eq!(serde_json::to_value(app.chat().snapshot(&reviewer)).unwrap()["queue"],before["queue"]);
    assert!(tell_report(&app,&initial).unwrap_err().contains("earlier"));
    assert_eq!(flow::action(&app,&correction).unwrap()["run"]["round"],2);
    review_cleanup(app);
}
