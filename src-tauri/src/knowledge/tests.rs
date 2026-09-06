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
impl Drop for Fixture { fn drop(&mut self) { let _=std::fs::remove_dir_all(&self.dir); } }

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
    let f=Fixture::new();let index=f.seed();f.fake_runtime("#!/bin/sh\nexit 0\n");let memory=f.memory();
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
    let separate=agent::query(&f.app,&query).unwrap();assert_eq!(separate["code"]["available"],true);assert_eq!(separate["code"]["data"]["total"],1);
    std::fs::write(f.root.join("main.ts"),"export function renamed() { return 2; }\n").unwrap();
    let updated=dispatch(&f.app,"knowledge_search",&json!({"id":index.id,"query":"renamed"})).unwrap();assert_eq!(updated["total"],1);
    assert_eq!(dispatch(&f.app,"knowledge_search",&json!({"id":index.id,"query":"otherBranch"})).unwrap()["total"],0);
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
