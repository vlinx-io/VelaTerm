//! Isolated behavioral tests; the ignored integration test uses a supplied verified CodeGraph bundle.
use super::*;
use crate::host::HeadlessHost;
use std::time::{Duration,Instant};

struct Fixture { app: AppCtx, dir: PathBuf, root: PathBuf }
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("vlx-knowledge-test-{}",uuid::Uuid::new_v4()));
        let root = dir.join("project"); std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        db.conn.lock().unwrap().execute("INSERT INTO projects(id,name,root_path,sort_order,collapsed,created_at) VALUES('p','CodeGraph',?1,0,0,0)",[root.to_str().unwrap()]).unwrap();
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(dir.clone(),db)));
        Self {app,dir,root}
    }
    fn index(&self) -> Index {
        let value = list(&self.app,"p").unwrap(); serde_json::from_value(value["indexes"][0].clone()).unwrap()
    }
    fn seed(&self) -> Index {
        let index = self.index(); std::fs::create_dir(self.root.join(".codegraph")).unwrap();
        let conn = Connection::open(self.root.join(".codegraph/codegraph.db")).unwrap();
        conn.execute_batch("CREATE TABLE nodes(id TEXT PRIMARY KEY,name TEXT,qualified_name TEXT,kind TEXT,file_path TEXT,start_line INTEGER,end_line INTEGER,signature TEXT,language TEXT); CREATE TABLE edges(id INTEGER,source TEXT,target TEXT,kind TEXT,line INTEGER,provenance TEXT,metadata TEXT); CREATE TABLE files(path TEXT,content_hash TEXT); CREATE TABLE project_metadata(key TEXT,value TEXT); INSERT INTO project_metadata VALUES('index_state','complete'); INSERT INTO nodes VALUES('a','run','module.run','function','main.ts',1,1,NULL,'typescript'),('b','run','other.run','function','other.ts',1,1,NULL,'typescript'); INSERT INTO edges VALUES(1,'a','b','calls',1,NULL,NULL);").unwrap();
        let content = b"export function run() { return 1; }\n";
        for file in ["main.ts","other.ts"] {
            std::fs::write(self.root.join(file),content).unwrap();
            conn.execute("INSERT INTO files VALUES(?1,?2)",params![file,graph::digest(content)]).unwrap();
        }
        self.app.db().conn.lock().unwrap().execute("UPDATE knowledge_indexes SET enabled=1,status='ready' WHERE id=?1",[&index.id]).unwrap();
        get(&self.app,&index.id).unwrap()
    }
    fn memory(&self) -> Value {
        crate::memory::dispatch(&self.app,"memory_save",&json!({"id":null,"version":0,"title":"Run design","summary":"Reasoning","content":"Keep run deterministic.","tags":[],"related":[]})).unwrap()
    }
    fn wait(&self,id:&str) -> Index {
        let started=Instant::now();
        loop {
            let i=get(&self.app,id).unwrap();
            if !["indexing","syncing"].contains(&i.status.as_str()) { return i; }
            assert!(started.elapsed()<Duration::from_secs(60)); std::thread::sleep(Duration::from_millis(50));
        }
    }
    #[cfg(unix)]
    fn fake_runtime(&self, script: &str) {
        use std::os::unix::fs::PermissionsExt;
        let target = if cfg!(target_os="macos") { if cfg!(target_arch="aarch64") {"darwin-arm64"}else{"darwin-x64"} }else{if cfg!(target_arch="aarch64") {"linux-arm64"}else{"linux-x64"}};
        let runtime=self.dir.join("codegraph/1.6.0").join(format!("codegraph-{target}"));
        std::fs::create_dir_all(runtime.join("lib/dist/bin")).unwrap();
        std::fs::write(runtime.join("lib/dist/bin/codegraph.js"),"").unwrap();
        std::fs::write(runtime.join("node"),script).unwrap();
        std::fs::set_permissions(runtime.join("node"),std::fs::Permissions::from_mode(0o700)).unwrap();
    }
}
impl Drop for Fixture { fn drop(&mut self) {
    if let Ok(value)=list(&self.app,"p") { for index in value["indexes"].as_array().unwrap() {worker::stop(&self.app,index["root"].as_str().unwrap());} }
    let _=std::fs::remove_dir_all(&self.dir); } }

#[test]
fn graph_preserves_exact_identity_and_rejects_escaping_sources() {
    let f=Fixture::new();f.seed();
    assert_eq!(graph::search(&f.root,"run",0).unwrap()["total"],2);
    let detail=graph::detail(&f.root,"a").unwrap();
    assert_eq!(detail["outgoing"][0]["node"]["id"],"b");assert_eq!(detail["incoming"].as_array().unwrap().len(),0);
    assert_eq!(detail["changedDuringRead"],false);
    assert!(graph::file(&f.root,"../test.db").is_err());
    assert!(graph::file(&f.root,f.dir.join("test.db").to_str().unwrap()).is_err());
    std::fs::write(f.root.join("main.ts"),"changed\n").unwrap();
    assert_eq!(graph::detail(&f.root,"a").unwrap()["changedDuringRead"],true);
}

#[test]
fn links_detect_changes_and_survive_disabled_indexes_without_rewriting_memory() {
    let f=Fixture::new();let index=f.seed();let memory=f.memory();
    let digest=graph::digest(&graph::file(&f.root,"main.ts").unwrap());
    f.app.db().conn.lock().unwrap().execute("INSERT INTO knowledge_links VALUES('l',?1,?2,'main.ts','module.run','function',?3,0)",params![memory["id"].as_str(),index.id,digest]).unwrap();
    assert_eq!(links::for_entry(&f.app,memory["id"].as_str().unwrap()).unwrap()[0]["status"],"current");
    std::fs::write(f.root.join("main.ts"),"export function run() { return 2; }\n").unwrap();
    dispatch(&f.app,"knowledge_disable",&json!({"id":index.id})).unwrap();
    assert_eq!(links::for_entry(&f.app,memory["id"].as_str().unwrap()).unwrap()[0]["status"],"review");
    std::fs::remove_file(f.root.join("main.ts")).unwrap();
    assert_eq!(links::for_entry(&f.app,memory["id"].as_str().unwrap()).unwrap()[0]["status"],"unavailable");
    let saved=crate::memory::dispatch(&f.app,"memory_get",&json!({"id":memory["id"]})).unwrap();
    assert_eq!(saved["entry"],memory);
    // Projects with archived sessions are removed through a tombstone.
    f.app.db().conn.lock().unwrap().execute("UPDATE projects SET deleted_at=1 WHERE id='p'",[]).unwrap();
    assert!(get(&f.app,&index.id).is_err());
    assert!(links::for_entry(&f.app,memory["id"].as_str().unwrap()).unwrap().as_array().unwrap().is_empty());
    assert!(f.root.join(".codegraph/codegraph.db").exists());
    let retained=crate::memory::dispatch(&f.app,"memory_get",&json!({"id":memory["id"]})).unwrap();
    assert_eq!(retained["entry"],memory);
}

#[test]
#[cfg(unix)]
fn indexing_failure_and_cancellation_are_recoverable() {
    let f=Fixture::new();let index=f.seed(); f.fake_runtime("#!/bin/sh\nexit 1\n");
    start(&f.app,&index.id).unwrap(); assert_eq!(f.wait(&index.id).status,"failed");
    f.fake_runtime("#!/bin/sh\nsleep 10\n");start(&f.app,&index.id).unwrap();
    assert_eq!(start(&f.app,&index.id).unwrap_err(),"knowledge_busy");
    dispatch(&f.app,"knowledge_disable",&json!({"id":index.id})).unwrap();
    std::thread::sleep(Duration::from_millis(1300));
    assert_eq!(get(&f.app,&index.id).unwrap().status,"disabled");
    assert!(f.root.join(".codegraph/codegraph.db").exists());
}

#[test]
#[cfg(unix)]
fn association_checks_displayed_source_and_memory_versions() {
    let f=Fixture::new();let index=f.seed();f.fake_runtime("#!/bin/sh\nwhile IFS= read -r request; do printf '%s\\n' 'VLX_KNOWLEDGE {\"result\":{}}'; done\n");let memory=f.memory();
    let digest=graph::digest(&graph::file(&f.root,"main.ts").unwrap());
    let mut args=json!({"entryId":memory["id"],"version":memory["version"],"indexId":index.id,"nodeId":"a","digest":"old"});
    assert_eq!(links::create(&f.app,&args).unwrap_err(),"knowledge_conflict");
    args["digest"]=json!(digest);links::create(&f.app,&args).unwrap();
    let link=links::for_entry(&f.app,memory["id"].as_str().unwrap()).unwrap()[0].clone();
    let review=json!({"entryId":memory["id"],"id":link["id"],"version":-1,"digest":digest});
    assert_eq!(links::review(&f.app,&review).unwrap_err(),"knowledge_conflict");
    assert_eq!(links::for_node(&f.app,&index,&graph::find(&graph::open(&f.root).unwrap(),"a").unwrap()).unwrap()[0]["entryId"],memory["id"]);
}

#[test]
#[ignore = "Set VLX_TEST_CODEGRAPH_DIR to the extracted verified bundle directory."]
#[cfg(unix)]
fn real_runtime_indexes_syncs_and_queries_a_separate_worktree() {
    let f=Fixture::new();
    let bundle=PathBuf::from(std::env::var("VLX_TEST_CODEGRAPH_DIR").expect("verified bundle directory"));
    let target=f.dir.join("codegraph/1.6.0");std::fs::create_dir_all(&target).unwrap();
    std::os::unix::fs::symlink(&bundle,target.join(bundle.file_name().unwrap())).unwrap();
    let git=|args:&[&str]| {let output=crate::host::command("git").arg("-C").arg(&f.root).args(args).output().unwrap();assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));};
    git(&["init","-q"]);
    std::fs::write(f.root.join("main.ts"),"export function helper() { return 1; }\nexport function run() { return helper(); }\n").unwrap();
    std::fs::write(f.root.join("lib.rs"),"pub fn twice(value: i32) -> i32 { value * 2 }\n").unwrap();
    git(&["add","main.ts"]);git(&["-c","user.name=Test","-c","user.email=test@example.invalid","commit","-qm","fixture"]);
    let index=f.index();start(&f.app,&index.id).unwrap();assert_eq!(f.wait(&index.id).status,"ready");
    assert_eq!(dispatch(&f.app,"knowledge_search",&json!({"id":index.id,"query":"twice"})).unwrap()["total"],1);
    let search=dispatch(&f.app,"knowledge_search",&json!({"id":index.id,"query":"run"})).unwrap();
    let node=&search["nodes"][0];let detail=dispatch(&f.app,"knowledge_node",&json!({"id":index.id,"nodeId":node["id"]})).unwrap();
    assert!(detail["outgoing"].as_array().unwrap().iter().any(|e|e["node"]["name"]=="helper"));
    let overview=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"status"})).unwrap();
    assert!(overview["nodeCount"].as_u64().unwrap()>0);
    let ranked=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"search","query":"kind:function run"})).unwrap();
    assert_eq!(ranked["nodes"][0]["name"],"run");
    let helper=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"search","query":"helper"})).unwrap()["nodes"][0]["id"].clone();
    let path=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"path","nodeId":node["id"],"targetId":helper})).unwrap();
    assert_eq!(path["path"].as_array().unwrap().len(),2);
    let impact=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"impact","nodeId":helper,"depth":2})).unwrap();
    assert!(impact["nodes"].as_array().unwrap().iter().any(|n|n["name"]=="run"));
    let callers=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"callers","nodeId":helper,"depth":1})).unwrap();
    assert!(callers["relations"].as_array().unwrap().iter().any(|n|n["node"]["name"]=="run"));
    let callees=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"callees","nodeId":node["id"],"depth":1})).unwrap();
    assert!(callees["relations"].as_array().unwrap().iter().any(|n|n["node"]["name"]=="helper"));
    let reverse=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"path","nodeId":helper,"targetId":node["id"]})).unwrap();
    assert!(reverse["path"].is_null());
    let explore=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"explore","query":"main.ts run helper"})).unwrap();
    assert!(explore["content"].as_array().unwrap().iter().any(|c|c["text"].as_str().unwrap().contains("helper")));
    let files=dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"files","query":"main"})).unwrap();
    assert_eq!(files["files"][0]["path"],"main.ts");
    assert!(guard_paths(&f.app,"knowledge_query",&json!({"id":index.id}), |_|Err("denied".into())).is_err());
    assert_eq!(dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"invalid"})).unwrap_err(),"knowledge_invalid");
    // Verify native watching itself, without a query-triggered sync.
    std::fs::write(f.root.join("watched.ts"),"export function watchedUpdate() { return 7; }\n").unwrap();
    let wait_started=Instant::now();
    while graph::search(&f.root,"watchedUpdate",0).unwrap()["total"]!=1 {
        assert!(wait_started.elapsed()<Duration::from_secs(15),"native watcher did not update the index");
        std::thread::sleep(Duration::from_millis(100));
    }
    let wt=f.root.join("nested-worktree");git(&["worktree","add","-qb","separate",wt.to_str().unwrap()]);
    std::fs::write(wt.join("main.ts"),"export function otherBranch() { return 3; }\n").unwrap();
    assert_eq!(checkout(&wt).unwrap(),wt.canonicalize().unwrap());
    assert_ne!(checkout(&wt).unwrap(),checkout(&f.root).unwrap());
    let sid = {
        let conn=f.app.db().conn.lock().unwrap();
        crate::db::repo::create_session(&conn,"p",None,"Worktree",crate::models::SessionKind::Terminal,None,Some(wt.to_str().unwrap()),None,None,Some(wt.to_str().unwrap())).unwrap().id
    };
    let query=json!({"sessionId":sid,"cwd":wt,"action":"search","query":"otherBranch"});
    let unavailable=agent::query(&f.app,&query).unwrap();assert_eq!(unavailable["code"]["available"],false);
    let targets=list(&f.app,"p").unwrap();
    let target=targets["indexes"].as_array().unwrap().iter().find(|i|i["root"].as_str()==wt.to_str()).unwrap();
    start(&f.app,target["id"].as_str().unwrap()).unwrap();assert_eq!(f.wait(target["id"].as_str().unwrap()).status,"ready");
    let separate=agent::query(&f.app,&query).unwrap();assert_eq!(separate["code"]["available"],true);assert_eq!(separate["code"]["data"]["nodes"].as_array().unwrap().len(),1);
    std::fs::write(f.root.join("main.ts"),"export function renamed() { return 2; }\n").unwrap();
    let updated=dispatch(&f.app,"knowledge_search",&json!({"id":index.id,"query":"renamed"})).unwrap();assert_eq!(updated["total"],1);
    assert_eq!(dispatch(&f.app,"knowledge_search",&json!({"id":index.id,"query":"otherBranch"})).unwrap()["total"],0);
    dispatch(&f.app,"knowledge_disable",&json!({"id":index.id})).unwrap();
    assert_eq!(dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"status"})).unwrap_err(),"knowledge_disabled");
    start(&f.app,&index.id).unwrap();assert_eq!(f.wait(&index.id).status,"ready");
    assert!(dispatch(&f.app,"knowledge_query",&json!({"id":index.id,"action":"status"})).unwrap()["watching"].as_bool().unwrap());
    assert!(!f.root.join(".git/hooks/post-commit").exists());
}

#[test]
fn selected_subdirectory_does_not_expand_to_the_parent_repository() {
    let f=Fixture::new();
    let git=crate::host::command("git").arg("-C").arg(&f.root).args(["init","-q"]).status().unwrap();assert!(git.success());
    let sub=f.root.join("src");std::fs::create_dir(&sub).unwrap();
    f.app.db().conn.lock().unwrap().execute("UPDATE projects SET root_path=?1 WHERE id='p'",[sub.to_str().unwrap()]).unwrap();
    assert_eq!(roots(&f.app,"p").unwrap(),vec![sub.to_string_lossy().into_owned()]);
}

#[test]
fn ephemeral_terminals_and_cross_project_directories_resolve_status_and_knowledge() {
    let f = Fixture::new();
    let sub = f.root.join("nested");
    std::fs::create_dir_all(&sub).unwrap();
    let unindexed = f.dir.join("scratch");
    std::fs::create_dir_all(&unindexed).unwrap();
    let entry = f.memory();

    // Plain terminals use in-memory `eph-*` IDs without a SQLite session row.
    // Outside any imported project (including $HOME), status and search report missing code index instead of 400.
    let scratch_status = agent::query(&f.app, &json!({"sessionId":"eph-plain","cwd":unindexed,"action":"status","query":""})).unwrap();
    assert!(scratch_status["index"].is_null());
    assert_eq!(scratch_status["code"]["available"], false);
    assert_eq!(scratch_status["code"]["reason"], "knowledge_index_missing");
    assert!(scratch_status["runtime"]["version"].is_string());

    if let Some(home) = crate::host::home_dir().filter(|p| p.is_dir()) {
        let home_status = agent::query(&f.app, &json!({"sessionId":"eph-plain","cwd":home,"action":"status","query":""})).unwrap();
        assert!(home_status["index"].is_null());
        assert_eq!(home_status["code"]["reason"], "knowledge_index_missing");
        let home_search = agent::query(&f.app, &json!({"sessionId":"eph-plain","cwd":home,"action":"search","query":"Run"})).unwrap();
        assert_eq!(home_search["code"]["available"], false);
        assert_eq!(home_search["memories"]["entries"][0]["id"], entry["id"]);
    }

    // Moving into an imported project (or its subdirectory) from an `eph-*` terminal resolves that project's index.
    let proj_status = agent::query(&f.app, &json!({"sessionId":"eph-plain","cwd":sub,"action":"status","query":""})).unwrap();
    assert_eq!(proj_status["index"]["projectId"], "p");
    assert_eq!(proj_status["index"]["root"], f.root.to_str().unwrap());
    assert_eq!(proj_status["index"]["status"], "disabled");

    // Non-code actions work from ephemeral sessions as well.
    let memories = agent::query(&f.app, &json!({"sessionId":"eph-plain","action":"memories","query":"Run"})).unwrap();
    assert_eq!(memories["entries"][0]["id"], entry["id"]);

    // A persisted session bound to a collection or another project still resolves the imported project at `cwd`.
    let other_sid = {
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute("INSERT INTO projects(id,name,root_path,sort_order,collapsed,created_at) VALUES('chat-col','Chat','',1,0,0)", []).unwrap();
        crate::db::repo::create_session(&conn, "chat-col", None, "Chat", crate::models::SessionKind::Codex, None, None, None, None, None).unwrap().id
    };
    let cross_status = agent::query(&f.app, &json!({"sessionId":other_sid,"cwd":sub,"action":"status","query":""})).unwrap();
    assert_eq!(cross_status["index"]["projectId"], "p");

    // Unknown non-ephemeral session IDs remain rejected.
    assert_eq!(
        agent::query(&f.app, &json!({"sessionId":"missing-session","cwd":f.root,"action":"status","query":""})).unwrap_err(),
        "knowledge_not_found"
    );
}

#[test]
fn cli_expands_real_errors_without_leaking_templates_or_internal_codes() {
    for code in [
        "knowledge_unauthorized", "knowledge_not_found", "knowledge_invalid",
        "knowledge_missing_directory", "knowledge_invalid_directory", "knowledge_disabled",
        "knowledge_runtime_missing", "memory_not_found", "kb_not_found", "knowledge_unknown_future",
    ] {
        let rendered = agent::format_cli_error(code);
        assert!(!rendered.contains("knowledge_"), "{code} leaked raw code: {rendered}");
        assert!(!rendered.contains("kb_"), "{code} leaked raw code: {rendered}");
        assert!(!rendered.contains("memory_"), "{code} leaked raw code: {rendered}");
        assert!(!rendered.contains("{error}"), "{code} left unexpanded placeholder: {rendered}");
    }

    let args = vec!["velaterm".into(), "--knowledge".into(), "status".into()];
    assert_eq!(
        agent::run_with_env(&args, None, Some("eph-1".into()), Some("tok".into()), Ok(PathBuf::from("."))).unwrap_err(),
        "Run vkb inside a VelaTerm session."
    );
    assert_eq!(
        agent::run_with_env(&args, Some("http://127.0.0.1:9".into()), None, Some("tok".into()), Ok(PathBuf::from("."))).unwrap_err(),
        "Missing VelaTerm session."
    );
    assert_eq!(
        agent::run_with_env(&args, Some("http://127.0.0.1:9".into()), Some("eph-1".into()), None, Ok(PathBuf::from("."))).unwrap_err(),
        "Missing VelaTerm session credentials."
    );
    let unreachable = agent::run_with_env(
        &args,
        Some("http://127.0.0.1:9".into()),
        Some("eph-1".into()),
        Some("tok".into()),
        Ok(std::env::temp_dir()),
    ).unwrap_err();
    assert!(unreachable.contains("http://127.0.0.1:9"), "unexpected transport error: {unreachable}");
    assert!(unreachable.contains("sandbox"), "transport error should mention sandbox networking: {unreachable}");

    let ok_decoded = agent::decode_response(r#"{"result":{"index":{"projectId":"p"}}}"#).unwrap();
    let parsed_decoded: Value = serde_json::from_str(&ok_decoded).unwrap();
    assert_eq!(parsed_decoded["index"]["projectId"], "p");
    assert_eq!(
        agent::decode_response(r#"{"error":"knowledge_invalid"}"#).unwrap_err(),
        "Invalid vkb arguments. Run `vkb --help` for usage."
    );
    assert_eq!(
        agent::decode_response(r#"{"error":"knowledge_unauthorized"}"#).unwrap_err(),
        "Session credentials were rejected by VelaTerm. Open a new VelaTerm session and retry."
    );

    let loopback_allowed = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| std::net::TcpStream::connect(listener.local_addr()?))
        .is_ok();
    if loopback_allowed {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let f = Fixture::new();
        let port = server.server_addr().to_ip().unwrap().port();
        let base = format!("http://127.0.0.1:{port}");
        let app = f.app.clone();
        let worker = std::thread::spawn(move || {
            for _ in 0..3 {
                let req = server.recv().unwrap();
                agent::handle(app.clone(), crate::diagnostics::HttpRequest::new(req), "secret-token".into());
            }
        });
        let ok_json = agent::run_with_env(
            &args,
            Some(base.clone()),
            Some("eph-http".into()),
            Some("secret-token".into()),
            Ok(f.root.clone()),
        ).unwrap();
        let parsed: Value = serde_json::from_str(&ok_json).unwrap();
        assert_eq!(parsed["index"]["projectId"], "p");

        let bad_action = vec!["velaterm".into(), "--knowledge".into(), "bogus".into()];
        let err_400 = agent::run_with_env(
            &bad_action,
            Some(base.clone()),
            Some("eph-http".into()),
            Some("secret-token".into()),
            Ok(f.root.clone()),
        ).unwrap_err();
        assert_eq!(err_400, "Invalid vkb arguments. Run `vkb --help` for usage.");

        let err_401 = agent::run_with_env(
            &args,
            Some(base),
            Some("eph-http".into()),
            Some("wrong-token".into()),
            Ok(f.root.clone()),
        ).unwrap_err();
        assert_eq!(err_401, "Session credentials were rejected by VelaTerm. Open a new VelaTerm session and retry.");
        worker.join().unwrap();
    }
}
