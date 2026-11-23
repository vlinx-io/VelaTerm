//! Validation, confirmation and outcome tests for `vapp` requests.

use super::*;
use std::cmp::Ordering;
use std::sync::Arc;
use crate::db::repo;
use crate::host::HeadlessHost;
use crate::models::SessionKind;

struct Fixture { app: AppCtx, dir: std::path::PathBuf }
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("vlx-assist-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        Fixture { app: AppCtx::Headless(Arc::new(HeadlessHost::new(dir.clone(), db))), dir }
    }
    fn local(&self, cmd: &str, args: Value) -> Result<Value> { call(&self.app, cmd, args, CallOrigin::Local) }
    fn command(&self, cmd: &str, args: Value) -> Result<Value> { dispatch(&self.app, cmd, &args, CallOrigin::Local) }
    fn session(&self) -> String {
        let conn = self.app.db().conn.lock().unwrap();
        // Collection names are unique, and a test may submit from several sessions.
        let project = repo::create_virtual_project(&conn, &format!("Work {}", uuid::Uuid::new_v4())).unwrap();
        repo::create_session(&conn, &project.id, None, "Planner", SessionKind::Codex, None, None, None, None, None).unwrap().id
    }
    /// Submits as a fresh session and returns the stored plan, or the refusal when any action is invalid.
    fn submit(&self, actions: Value) -> Value { self.submit_with(actions, false) }
    fn submit_with(&self, actions: Value, no_confirm: bool) -> Value {
        let session = self.session();
        let submitted = submit(&self.app, &session, "request", &actions, no_confirm).unwrap();
        if submitted["accepted"] == false { return submitted; }
        self.command("assist_get", json!({"id": submitted["id"]})).unwrap()
    }
    /// Confirms every listed action together, as the session view does for a run of host actions.
    fn run_all(&self, plan: &Value, indices: &[usize]) -> Value {
        self.command("assist_run", json!({"id": plan["id"], "indices": indices, "confirmed": true})).unwrap()
    }
    fn name_of(&self, id: &Value) -> Value {
        let tree = self.local("list_tree", json!({})).unwrap();
        tree["projects"].as_array().unwrap().iter().find(|p| p["id"] == *id).map(|p| p["name"].clone()).unwrap_or(Value::Null)
    }
    fn tree(&self) -> crate::models::Tree { repo::list_tree(&self.app.db().conn.lock().unwrap()).unwrap() }
    /// A collection named Course with three loose sessions, in the order Lesson 10, Lesson 2, Notes.
    fn course(&self) -> (String, Vec<String>) {
        let conn = self.app.db().conn.lock().unwrap();
        let project = repo::create_virtual_project(&conn, "Course").unwrap();
        let sessions = ["Lesson 10 make the Taiwan edition", "Lesson 2 make the Taiwan edition", "Notes"].iter()
            .map(|name| repo::create_session(&conn, &project.id, None, name, SessionKind::Claude, None, None, None, None, None).unwrap().id)
            .collect();
        (project.id, sessions)
    }
}
impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); } }

fn reasons(refused: &Value) -> Vec<String> {
    assert_eq!(refused["accepted"], false, "{refused}");
    refused["problems"].as_array().unwrap().iter().map(|s| s["reason"].as_str().unwrap().to_string()).collect()
}

fn statuses(plan: &Value) -> Vec<String> {
    plan["actions"].as_array().unwrap().iter().map(|a| a["status"].as_str().unwrap().to_string()).collect()
}

#[test]
fn every_area_supplies_its_state() {
    let fixture = Fixture::new();
    let ids = contexts(&fixture.app, None).iter().map(|(d, _)| d.id).collect::<Vec<_>>();
    assert_eq!(ids, ["projects", "files", "knowledge", "settings"]);
    let listed = catalog(&fixture.app, Some("settings")).unwrap();
    assert_eq!(listed["domains"].as_array().unwrap().len(), 1);
    assert!(catalog(&fixture.app, Some("databases")).is_err());
}

#[test]
fn names_with_numbers_sort_by_value() {
    assert_eq!(tree::natural_cmp("Lesson 2", "Lesson 10"), Ordering::Less);
    assert_eq!(tree::natural_cmp("人物6. 張衡", "人物10. 祖沖之"), Ordering::Less);
    assert_eq!(tree::natural_cmp("b", "A"), Ordering::Greater);
    assert_eq!(tree::natural_cmp("v02", "v2"), Ordering::Greater);
}

#[test]
fn an_invalid_action_refuses_the_whole_request() {
    let fixture = Fixture::new();
    let collection = fixture.local("create_virtual_project", json!({"name": "Clients"})).unwrap();
    let refused = fixture.submit(json!([
        {"domain": "projects", "kind": "rename", "id": collection["id"], "name": "Office"},
        {"domain": "projects", "kind": "rename", "id": "missing", "name": "x"},
        {"domain": "terminal", "kind": "run", "command": "rm -rf /"},
        {"domain": "settings", "kind": "set", "setting": "theme", "value": "DARK"},
        {"domain": "settings", "kind": "set", "setting": "theme", "value": "purple"},
        {"kind": "launchRocket"},
    ]));
    assert_eq!(refused["problems"], json!([
        {"index": 1, "domain": "projects", "kind": "rename", "field": "id", "reason": "unknownItem"},
        {"index": 2, "domain": "terminal", "kind": "run", "field": "domain", "reason": "unsupported"},
        {"index": 4, "domain": "settings", "kind": "set", "reason": "invalid"},
        {"index": 5, "domain": "projects", "kind": "launchRocket", "field": "kind", "reason": "unsupported"},
    ]));
    // Nothing was stored, so nothing waits for the user and nothing changed.
    assert_eq!(fixture.command("assist_pending", json!({"sessionId": fixture.session()})).unwrap(), json!([]));
    assert_eq!(fixture.name_of(&collection["id"]), "Clients");
}

#[test]
fn valid_actions_are_normalized_with_their_owner() {
    let fixture = Fixture::new();
    let collection = fixture.local("create_virtual_project", json!({"name": "Clients"})).unwrap();
    let plan = fixture.submit(json!([
        {"kind": "rename", "id": collection["id"], "name": "Office"},
        {"domain": "settings", "kind": "set", "setting": "theme", "value": "DARK"},
    ]));
    assert_eq!(plan["actions"][0]["domain"], "projects");
    assert_eq!(plan["actions"][0]["currentName"], "Clients");
    assert_eq!(plan["actions"][0]["backend"], true);
    assert_eq!(plan["actions"][1]["value"], "dark");
    assert_eq!(plan["actions"][1]["backend"], false);
}

#[test]
fn built_in_collections_cannot_be_changed_or_hold_projects_elsewhere() {
    let fixture = Fixture::new();
    crate::db::system_collections::ensure(&fixture.app.db().conn.lock().unwrap()).unwrap();
    let folder = fixture.dir.join("demo-project");
    std::fs::create_dir_all(&folder).unwrap();
    let refused = fixture.submit(json!([
        {"domain": "projects", "kind": "rename", "id": crate::db::system_collections::PROJECTS, "name": "Mine"},
        {"domain": "projects", "kind": "deleteNode", "id": crate::db::system_collections::CHAT},
        {"domain": "projects", "kind": "importProject", "path": folder, "collectionId": crate::db::system_collections::TASKS},
    ]));
    assert_eq!(reasons(&refused), ["builtIn", "builtIn", "invalid"]);
    let plan = fixture.submit(json!([{"domain": "projects", "kind": "importProject", "path": folder, "collectionId": crate::db::system_collections::PROJECTS}]));
    assert_eq!(plan["actions"][0]["collectionId"], crate::db::system_collections::PROJECTS);
}

#[test]
fn host_actions_run_once_and_window_actions_are_only_marked() {
    let fixture = Fixture::new();
    let collection = fixture.local("create_virtual_project", json!({"name": "Clients"})).unwrap();
    let plan = fixture.submit(json!([
        {"domain": "projects", "kind": "rename", "id": collection["id"], "name": "Office"},
        {"domain": "settings", "kind": "openSettings", "page": "terminal"},
    ]));
    let id = plan["id"].clone();
    assert!(fixture.command("assist_run", json!({"id": id, "index": 0})).unwrap_err().contains("Confirm"));
    let done = fixture.command("assist_run", json!({"id": id, "index": 0, "confirmed": true})).unwrap();
    assert_eq!(done["actions"][0]["status"], "done");
    assert_eq!(fixture.name_of(&collection["id"]), "Office");
    assert!(fixture.command("assist_run", json!({"id": id, "index": 0, "confirmed": true})).unwrap_err().contains("already"));
    assert!(fixture.command("assist_run", json!({"id": id, "index": 1, "confirmed": true})).is_err());
    assert!(fixture.command("assist_mark", json!({"id": id, "index": 0, "status": "done"})).is_err());
    let marked = fixture.command("assist_mark", json!({"id": id, "index": 1, "status": "done"})).unwrap();
    assert_eq!(marked["actions"][1]["status"], "done");
}

#[test]
fn a_skipped_host_action_never_runs() {
    let fixture = Fixture::new();
    let collection = fixture.local("create_virtual_project", json!({"name": "Clients"})).unwrap();
    let plan = fixture.submit(json!([{"domain": "projects", "kind": "rename", "id": collection["id"], "name": "Office"}]));
    let marked = fixture.command("assist_mark", json!({"id": plan["id"], "index": 0, "status": "dismissed"})).unwrap();
    assert_eq!(marked["actions"][0]["status"], "dismissed");
    assert!(fixture.command("assist_run", json!({"id": plan["id"], "index": 0, "confirmed": true})).is_err());
    assert_eq!(fixture.name_of(&collection["id"]), "Clients");
}

#[test]
fn import_offers_only_listed_folders_and_refuses_imported_ones() {
    let fixture = Fixture::new();
    let folder = fixture.dir.join("demo-project");
    std::fs::create_dir_all(&folder).unwrap();
    let plan = fixture.submit(json!([{"domain": "projects", "kind": "importProject", "path": folder}]));
    let id = plan["id"].clone();
    assert_eq!(plan["actions"][0]["candidates"], json!([folder]));
    assert!(fixture.command("assist_run", json!({"id": id, "index": 0, "confirmed": true, "choice": "/etc"})).is_err());
    // A refused choice leaves the action failed rather than retryable.
    let plan = fixture.command("assist_get", json!({"id": id})).unwrap();
    assert_eq!(plan["actions"][0]["status"], "failed");
    // A single folder needs no choice.
    let again = fixture.submit(json!([{"domain": "projects", "kind": "importProject", "path": folder, "as": "demo"}]));
    let done = fixture.run_all(&again, &[0]);
    assert_eq!(done["actions"][0]["status"], "done", "{done}");
    assert!(done["actions"][0]["createdId"].is_string());
    assert_eq!(reasons(&fixture.submit(json!([{"domain": "projects", "kind": "importProject", "path": folder}]))), ["exists"]);
}

#[test]
fn file_and_note_paths_stay_within_their_bounds() {
    let fixture = Fixture::new();
    let file = fixture.dir.join("notes.md");
    std::fs::write(&file, "# Notes").unwrap();
    let refused = fixture.submit(json!([
        {"domain": "files", "kind": "openFile", "path": "relative/path"},
        {"domain": "files", "kind": "openFile", "path": fixture.dir},
        {"domain": "files", "kind": "openFile", "path": file},
        {"domain": "files", "kind": "reveal", "path": fixture.dir},
        {"domain": "knowledge", "kind": "openNote", "vaultId": "missing", "path": "../secret.md"},
    ]));
    assert_eq!(reasons(&refused), ["invalidPath", "notFile", "unknownItem"]);
    assert_eq!(knowledge::inside(&json!("../x.md"), false), Err("invalidPath"));
    assert_eq!(knowledge::inside(&json!("/notes/a.md"), false), Ok(Some("notes/a.md".into())));
    assert!(text(&json!("a\nb"), 10).is_err());
}

#[test]
fn large_state_is_shortened() {
    let fixture = Fixture::new();
    let mut contexts = contexts(&fixture.app, None);
    let sessions = (0..2000).map(|i| json!({"id": format!("session-{i}"), "name": "x".repeat(40)})).collect::<Vec<_>>();
    contexts[0].1["sessions"] = json!(sessions);
    fit(&mut contexts, STATE_BUDGET);
    let size = contexts.iter().map(|(_, c)| c.to_string().len()).sum::<usize>();
    assert!(size <= STATE_BUDGET);
    assert_eq!(contexts[0].1["sessions"].as_array().unwrap().last().unwrap(), "…more items omitted");
}

#[test]
fn sessions_submit_validated_actions_and_read_their_outcome() {
    let fixture = Fixture::new();
    let session = fixture.session();
    assert!(submit(&fixture.app, "", "x", &json!([{"domain": "settings", "kind": "openSettings"}]), false).is_err());
    assert!(submit(&fixture.app, &session, "x", &json!([]), false).is_err());
    // A draft terminal has no stored row; its request is still accepted, without a name.
    let draft = submit(&fixture.app, "eph-draft", "x", &json!([{"domain": "settings", "kind": "openSettings"}]), false).unwrap();
    let saved = fixture.command("assist_get", json!({"id": draft["id"]})).unwrap();
    assert_eq!(saved["source"], json!({"sessionId": "eph-draft", "sessionName": "", "agent": null}));
    let submitted = submit(&fixture.app, &session, "Open the terminal settings", &json!([{"domain": "settings", "kind": "openSettings", "page": "terminal"}]), false).unwrap();
    assert_eq!(submitted["accepted"], true);
    assert_eq!(submitted["settled"], false);
    let id = submitted["id"].as_str().unwrap();
    let saved = fixture.command("assist_get", json!({"id": id})).unwrap();
    assert_eq!(saved["source"]["sessionName"], "Planner");
    assert_eq!(saved["summary"], "Open the terminal settings");
    // The session view lists only its own requests that still wait for the user.
    let waiting = |session: &str| fixture.command("assist_pending", json!({"sessionId": session})).unwrap()
        .as_array().unwrap().iter().map(|plan| plan["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(waiting(&session), vec![id.to_string()]);
    assert_eq!(waiting("eph-draft"), vec![draft["id"].as_str().unwrap().to_string()]);
    fixture.command("assist_mark", json!({"id": id, "index": 0, "status": "dismissed"})).unwrap();
    assert!(waiting(&session).is_empty());
    let done = outcome(&fixture.app, id).unwrap();
    assert_eq!(done["settled"], true);
    assert_eq!(done["actions"][0]["status"], "dismissed");
}

#[test]
fn one_confirmation_creates_renames_moves_and_sorts() {
    let fixture = Fixture::new();
    let (project, sessions) = fixture.course();
    let plan = fixture.submit(json!([
        {"kind": "createGroup", "projectId": "Course", "name": "Lessons", "as": "lessons"},
        {"kind": "rename", "id": sessions[0], "name": "Lesson 10"},
        {"kind": "rename", "id": sessions[1], "name": "Lesson 2"},
        {"kind": "move", "id": "Lesson 10", "groupId": "$lessons"},
        {"kind": "move", "id": &sessions[1][..8], "groupId": "$lessons"},
        {"kind": "sort", "groupId": "$lessons", "by": "name"},
    ]));
    assert_eq!(plan["actions"][3]["to"], "Course / Lessons");
    assert!(plan["actions"].as_array().unwrap().iter().all(|a| a["backend"] == true));
    let applied = fixture.run_all(&plan, &[0, 1, 2, 3, 4, 5]);
    assert_eq!(statuses(&applied), ["done"; 6], "{applied}");
    let tree = fixture.tree();
    let group = tree.groups.iter().find(|g| g.name == "Lessons").unwrap();
    assert_eq!(group.project_id, project);
    assert_eq!(applied["actions"][0]["createdId"], group.id.as_str());
    // The stored plan names the real group once it exists.
    assert_eq!(applied["actions"][5]["groupId"], group.id.as_str());
    let mut inside = tree.sessions.iter().filter(|s| s.group_id.as_deref() == Some(group.id.as_str())).collect::<Vec<_>>();
    inside.sort_by_key(|s| s.sort_order);
    assert_eq!(inside.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Lesson 2", "Lesson 10"]);
    let status = outcome(&fixture.app, applied["id"].as_str().unwrap()).unwrap();
    assert_eq!(status["settled"], true);
    assert_eq!(status["actions"][0]["as"], "lessons");
    assert!(fixture.command("assist_run", json!({"id": plan["id"], "indices": [0], "confirmed": true})).is_err(), "an action runs only once");
}

#[test]
fn references_wait_for_their_creator_and_fail_with_it() {
    let fixture = Fixture::new();
    let (_, sessions) = fixture.course();
    let plan = fixture.submit(json!([
        {"kind": "createGroup", "projectId": "Course", "name": "Lessons", "as": "lessons"},
        {"kind": "move", "id": sessions[0], "groupId": "$lessons"},
        {"domain": "projects", "kind": "newSession", "projectId": "Course", "groupId": "$lessons"},
    ]));
    // A change confirmed before the item it needs exists is refused and stays pending.
    assert!(fixture.command("assist_run", json!({"id": plan["id"], "index": 1, "confirmed": true})).unwrap_err().contains("earlier"));
    assert_eq!(statuses(&fixture.command("assist_get", json!({"id": plan["id"]})).unwrap()), ["pending", "pending", "pending"]);
    // Skipping the group strands everything that needed it.
    let skipped = fixture.command("assist_mark", json!({"id": plan["id"], "index": 0, "status": "dismissed"})).unwrap();
    assert_eq!(statuses(&skipped), ["dismissed", "failed", "failed"]);
    assert!(skipped["actions"][1]["error"].as_str().unwrap().contains("depends on"));
    assert!(fixture.tree().groups.is_empty());

    // Once the group exists, a window action reads its real ID from the plan.
    let plan = fixture.submit(json!([
        {"kind": "createGroup", "projectId": "Course", "name": "Lessons", "as": "lessons"},
        {"domain": "projects", "kind": "newSession", "projectId": "Course", "groupId": "$lessons"},
    ]));
    let done = fixture.run_all(&plan, &[0]);
    assert_eq!(done["actions"][1]["groupId"], done["actions"][0]["createdId"]);
    assert_eq!(done["actions"][1]["status"], "pending");
}

#[test]
fn ambiguous_names_and_cycles_are_reported() {
    let fixture = Fixture::new();
    let (project, sessions) = fixture.course();
    repo::create_session(&fixture.app.db().conn.lock().unwrap(), &project, None, "Notes", SessionKind::Terminal, None, None, None, None, None).unwrap();
    let refused = fixture.submit(json!([
        {"kind": "rename", "id": "Notes", "name": "x"},
        {"kind": "move", "id": sessions[0], "parentSessionId": sessions[1]},
        {"kind": "move", "id": sessions[1], "parentSessionId": sessions[0]},
    ]));
    assert_eq!(refused["problems"], json!([
        {"index": 0, "domain": "projects", "kind": "rename", "field": "id", "reason": "ambiguous"},
        {"index": 2, "domain": "projects", "kind": "move", "field": "parentSessionId", "reason": "cycle"},
    ]));
}

#[test]
fn sorting_by_given_order_keeps_unlisted_items_after() {
    let fixture = Fixture::new();
    let (project, sessions) = fixture.course();
    let plan = fixture.submit(json!([{"kind": "sort", "target": project, "order": [sessions[2]]}]));
    fixture.run_all(&plan, &[0]);
    let mut root = fixture.tree().sessions.into_iter().filter(|s| s.project_id == project).collect::<Vec<_>>();
    root.sort_by_key(|s| s.sort_order);
    assert_eq!(root.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), [sessions[2].as_str(), &sessions[0], &sessions[1]]);
}

#[test]
fn skipping_a_batch_runs_none_of_it() {
    let fixture = Fixture::new();
    let (project, sessions) = fixture.course();
    let plan = fixture.submit(json!([{"kind": "archiveSession", "id": sessions[2]}, {"kind": "rename", "id": sessions[0], "name": "x"}]));
    let skipped = fixture.command("assist_mark", json!({"id": plan["id"], "indices": [0, 1], "status": "dismissed"})).unwrap();
    assert_eq!(statuses(&skipped), ["dismissed", "dismissed"]);
    assert!(fixture.command("assist_run", json!({"id": plan["id"], "indices": [0, 1], "confirmed": true})).is_err());
    assert!(fixture.command("assist_mark", json!({"id": plan["id"], "indices": [0, 1], "status": "done"})).is_err(), "window actions are recorded one at a time");
    assert_eq!(fixture.tree().sessions.iter().filter(|s| s.project_id == project && s.name != "x").count(), 3);
}

#[test]
fn list_shows_the_callers_project() {
    let fixture = Fixture::new();
    let (project, sessions) = fixture.course();
    let listed = list(&fixture.app, None, &sessions[0]).unwrap();
    assert_eq!(listed["project"]["id"], project.as_str());
    assert_eq!(listed["sessions"].as_array().unwrap().len(), 3);
    assert_eq!(listed["sessions"][0]["kind"], "claude");
    assert!(list(&fixture.app, Some("Nowhere"), "").is_err());
}

#[test]
fn no_confirm_runs_host_actions_immediately_with_references() {
    let fixture = Fixture::new();
    let (project, sessions) = fixture.course();
    let caller = fixture.session();
    let submitted = submit(&fixture.app, &caller, "Group lessons", &json!([
        {"kind": "createGroup", "projectId": "Course", "name": "Lessons", "as": "lessons"},
        {"kind": "rename", "id": sessions[0], "name": "Lesson 10"},
        {"kind": "rename", "id": sessions[1], "name": "Lesson 2"},
        {"kind": "move", "id": "Lesson 10", "groupId": "$lessons"},
        {"kind": "move", "id": &sessions[1][..8], "groupId": "$lessons"},
        {"kind": "sort", "groupId": "$lessons", "by": "name"},
    ]), true).unwrap();
    assert_eq!(submitted["accepted"], true);
    assert_eq!(submitted["settled"], true);
    assert!(submitted["actions"].as_array().unwrap().iter().all(|a| a["status"] == "done"));
    assert!(submitted["actions"][0]["createdId"].is_string());
    assert_eq!(fixture.command("assist_pending", json!({"sessionId": caller})).unwrap(), json!([]));
    let tree = fixture.tree();
    let group = tree.groups.iter().find(|g| g.name == "Lessons").unwrap();
    assert_eq!(group.project_id, project);
    let mut inside = tree.sessions.iter().filter(|s| s.group_id.as_deref() == Some(group.id.as_str())).collect::<Vec<_>>();
    inside.sort_by_key(|s| s.sort_order);
    assert_eq!(inside.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Lesson 2", "Lesson 10"]);
}

#[test]
fn no_confirm_still_refuses_invalid_plans_without_side_effects() {
    let fixture = Fixture::new();
    let collection = fixture.local("create_virtual_project", json!({"name": "Clients"})).unwrap();
    let refused = fixture.submit_with(json!([
        {"kind": "rename", "id": collection["id"], "name": "Office"},
        {"kind": "rename", "id": "missing", "name": "x"},
    ]), true);
    assert_eq!(reasons(&refused), ["unknownItem"]);
    assert_eq!(fixture.name_of(&collection["id"]), "Clients");
}

#[test]
fn no_confirm_leaves_multi_choice_import_and_dependents_pending() {
    let fixture = Fixture::new();
    let parent_a = fixture.dir.join("a");
    let parent_b = fixture.dir.join("b");
    let anchor_a = parent_a.join("anchor-a");
    let anchor_b = parent_b.join("anchor-b");
    let dup_a = parent_a.join("shared-proj");
    let dup_b = parent_b.join("shared-proj");
    for d in [&anchor_a, &anchor_b, &dup_a, &dup_b] { std::fs::create_dir_all(d).unwrap(); }
    // Import sibling anchor projects so `search` discovers both parent_a and parent_b.
    fixture.local("import_project", json!({"rootPath": anchor_a})).unwrap();
    fixture.local("import_project", json!({"rootPath": anchor_b})).unwrap();
    let plan = fixture.submit_with(json!([
        {"kind": "importProject", "name": "shared-proj", "as": "shared"},
        {"kind": "createGroup", "projectId": "$shared", "name": "Docs"},
    ]), true);
    assert_eq!(plan["noConfirm"], true);
    assert_eq!(statuses(&plan), ["pending", "pending"]);
    assert_eq!(plan["actions"][0]["candidates"].as_array().unwrap().len(), 2);
}

#[test]
fn no_confirm_window_actions_claim_once_and_mark_done() {
    let fixture = Fixture::new();
    let (_, sessions) = fixture.course();
    let caller = fixture.session();
    let submitted = submit(&fixture.app, &caller, "Group and open", &json!([
        {"kind": "createGroup", "projectId": "Course", "name": "Lessons", "as": "lessons"},
        {"domain": "projects", "kind": "openSession", "sessionId": sessions[0]},
    ]), true).unwrap();
    assert_eq!(submitted["accepted"], true);
    assert_eq!(submitted["settled"], false);
    assert_eq!(submitted["actions"][0]["status"], "done");
    assert_eq!(submitted["actions"][1]["status"], "pending");
    let id = submitted["id"].as_str().unwrap();

    // First client claims the window action; a second client sees it already claimed.
    let first = fixture.command("assist_claim", json!({"id": id, "index": 1})).unwrap();
    assert_eq!(first["claimed"], true);
    assert_eq!(first["plan"]["actions"][1]["status"], "running");
    let second = fixture.command("assist_claim", json!({"id": id, "index": 1})).unwrap();
    assert_eq!(second["claimed"], false);

    // Marking a running window action done settles the plan.
    let marked = fixture.command("assist_mark", json!({"id": id, "index": 1, "status": "done"})).unwrap();
    assert_eq!(marked["actions"][1]["status"], "done");
    assert_eq!(outcome(&fixture.app, id).unwrap()["settled"], true);

    // Regular plans without noConfirm cannot be claimed.
    let normal = fixture.submit(json!([{"domain": "settings", "kind": "openSettings", "page": "terminal"}]));
    assert!(fixture.command("assist_claim", json!({"id": normal["id"], "index": 0})).unwrap_err().contains("Confirm"));
}
