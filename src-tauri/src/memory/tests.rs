//! Behavioral verification uses isolated databases and deterministic local CLI fixtures, never a model service.
use super::*;
use crate::host::{AppCtx, HeadlessHost};
use rusqlite::params;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Fixture {
    app: AppCtx,
    dir: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("vlx-memory-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        Self {
            app: AppCtx::Headless(Arc::new(HeadlessHost::new(dir.clone(), db))),
            dir,
        }
    }
    fn source(&self, id: &str, content: &str) {
        let conn = self.app.db().conn.lock().unwrap();
        conn.execute("INSERT INTO memory_sources(id,session_id,session_name,kind,agent_session_id,digest,content,created_at) VALUES(?1,?1,?1,'claude',?1,?2,?3,?4)",params![id,runner::digest(content),content,now()]).unwrap();
    }
    fn failed_job(&self, id: &str, source: &str, agent: &str) {
        self.app.db().conn.lock().unwrap().execute("INSERT INTO memory_jobs(id,source_id,agent,model,status,stage,owner_pid,created_at,updated_at) VALUES(?1,?2,?3,'','failed','extract',0,?4,?4)",params![id,source,agent,now()]).unwrap();
    }
    fn save(&self, title: &str, content: &str) -> Entry {
        serde_json::from_value(dispatch(&self.app,"memory_save",&json!({"id":null,"version":0,"title":title,"summary":"可复用的知识","content":content,"tags":["技术"],"related":[]})).unwrap()).unwrap()
    }
    fn await_job(&self, id: &str) -> Value {
        let start = Instant::now();
        loop {
            let result = runner::jobs(&self.app, &json!({"id":id})).unwrap();
            let job = &result["jobs"][0];
            if job["status"] != "running" {
                return job.clone();
            }
            assert!(
                start.elapsed() < Duration::from_secs(12),
                "job timed out: {job}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    #[cfg(unix)]
    fn cli(&self, mode: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.dir.join("fake-agent");
        let script = format!(
            "#!/usr/bin/env python3\nMODE = {mode:?}\n{}",
            r#"
import sys,json,re,pathlib,time
if '--version' in sys.argv: print('2.1.252');sys.exit(0)
if 'app-server' in sys.argv:
 for line in sys.stdin:
  request=json.loads(line)
  if request.get('method')=='initialize': print(json.dumps({'id':request['id'],'result':{}}),flush=True)
  if request.get('method')=='model/list':
   print(json.dumps({'id':request['id'],'result':{'data':[{'id':'test-model','displayName':'Test model','supportedReasoningEfforts':[{'reasoningEffort':'low'},{'reasoningEffort':'high'}]}]}}),flush=True)
 sys.exit(0)
prompt=sys.stdin.read()
root=pathlib.Path(__file__).parent
with (root/'calls').open('a') as f: f.write('call\n')
with (root/'args').open('a') as f: f.write(json.dumps(sys.argv[1:])+'\n')
if MODE=='invalid': print('invalid');sys.exit(0)
if MODE=='slow': time.sleep(8)
if 'CATALOG:\n' in prompt:
 catalog=json.loads(prompt.split('CATALOG:\n',1)[1].split('\nSOURCE ',1)[0])
 target=catalog[0]['id'] if catalog else ''
 content=prompt.split('\nSOURCE ',1)[1].split(':\n',1)[1]
 result={'entries':[{'targetId':target,'title':'服务端口规范','summary':'本地服务配置','content':content,'tags':['技术'],'relatedTitles':[]}]}
else:
 target=re.search(r'targetId=([a-z0-9-]+)',prompt).group(1)
 existing=json.loads(prompt.split('EXISTING ENTRY:\n',1)[1].split('\nCONTRIBUTIONS:\n',1)[0])
 topics=json.loads(prompt.split('CONTRIBUTIONS:\n',1)[1])
 content=(existing['content']+'\n' if existing else '')+'\n'.join(t['content'] for t in topics)
 if MODE=='conflict':
  (root/'merge-started').write_text('ready')
  while not (root/'continue').exists(): time.sleep(.03)
 result={'entries':[{'targetId':target,'title':'服务端口规范','summary':'本地服务配置','content':content,'tags':['技术'],'relatedTitles':[]}]}
if len(sys.argv)>1 and sys.argv[1]=='exec':
 print(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':json.dumps(result)}}))
 print(json.dumps({'type':'turn.completed','usage':{'input_tokens':12,'output_tokens':8}}))
else: print(json.dumps({'structured_output':result,'is_error':False,'usage':{'input_tokens':12,'output_tokens':8}}))
"#
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        self.app.db().conn.lock().unwrap().execute("INSERT OR REPLACE INTO app_settings(key,value,updated_at) VALUES('vlx-settings',?1,?2)",params![json!({"agentDefaults":{"claude":{"path":path},"codex":{"path":path}}}).to_string(),now()]).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn independent_search_handles_chinese_short_terms_tags_and_queries() {
    let f = Fixture::new();
    f.save("服务端口规范", "本地端口 24680，支持中文搜索和数据库事务。");
    f.save("其他主题", "独立内容。");
    for query in ["中文", "数据库事务", "24680", "服务 端口"] {
        let list = dispatch(
            &f.app,
            "memory_list",
            &json!({"query":query,"tag":"技术","page":0}),
        )
        .unwrap();
        assert_eq!(list["total"], 1, "{query}");
    }
    assert_eq!(
        dispatch(&f.app, "memory_list", &json!({"query":"' OR 1=1 --"})).unwrap()["total"],
        0
    );
    assert_eq!(
        dispatch(&f.app, "memory_list", &json!({"tag":"不存在"})).unwrap()["total"],
        0
    );
    let conn = f.app.db().conn.lock().unwrap();
    let count: i64 = conn
        .query_row("SELECT count(*) FROM session_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn revisions_reject_stale_writes_and_restore_as_new_version() {
    let f = Fixture::new();
    let first = f.save("事务", "旧内容");
    let edit = json!({"id":first.id,"version":1,"title":"事务","summary":"摘要","content":"新内容","tags":[],"related":[]});
    assert_eq!(
        dispatch(&f.app, "memory_save", &edit).unwrap()["version"],
        2
    );
    assert_eq!(
        dispatch(&f.app, "memory_save", &edit).unwrap_err(),
        "memory_conflict"
    );
    let restored = dispatch(
        &f.app,
        "memory_restore",
        &json!({"id":first.id,"version":2,"targetVersion":1}),
    )
    .unwrap();
    assert_eq!(restored["version"], 3);
    assert_eq!(restored["content"], "旧内容");
    let detail = dispatch(&f.app, "memory_get", &json!({"id":first.id,"version":2})).unwrap();
    assert_eq!(detail["versions"].as_array().unwrap().len(), 3);
    assert_eq!(detail["revision"]["content"], "新内容");
    assert!(
        detail["versions"][0].get("entry").is_none(),
        "history metadata must not load every full revision"
    );
}

#[test]
fn duplicate_titles_and_invalid_links_are_rejected_atomically() {
    let f = Fixture::new();
    f.save("Theme", "original");
    assert_eq!(dispatch(&f.app,"memory_save",&json!({"id":null,"version":0,"title":" theme ","summary":"","content":"x","tags":[],"related":[]})).unwrap_err(),"memory_duplicate_title");
    assert_eq!(dispatch(&f.app,"memory_save",&json!({"id":null,"version":0,"title":"Other","summary":"","content":"x","tags":[],"related":["missing"]})).unwrap_err(),"memory_invalid:related");
    assert_eq!(
        repo::all(&f.app.db().conn.lock().unwrap()).unwrap().len(),
        1
    );
}

#[test]
fn batch_conflict_rolls_back_entries_and_fts() {
    let f = Fixture::new();
    let existing = f.save("Existing", "original");
    let mut conn = f.app.db().conn.lock().unwrap();
    {
        let tx = conn.transaction().unwrap();
        let mut new = existing.clone();
        new.id = "new".into();
        new.title = "New".into();
        repo::put(&tx, new, 0, "test").unwrap();
        assert_eq!(
            repo::put(&tx, existing, 0, "test").unwrap_err(),
            "memory_conflict"
        );
    }
    assert!(repo::get(&conn, "new").unwrap().is_none());
    assert_eq!(
        conn.query_row("SELECT count(*) FROM memory_fts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn links_backlinks_and_sources_survive_origin_deletion() {
    let f = Fixture::new();
    f.source("source", "完整来源");
    let a = f.save("A", "first");
    let mut b = f.save("B", "second");
    b.related = vec![a.id.clone()];
    b.sources = vec!["source".into()];
    {
        let mut conn = f.app.db().conn.lock().unwrap();
        let tx = conn.transaction().unwrap();
        repo::put(&tx, b, 1, "test").unwrap();
        tx.commit().unwrap();
    }
    let detail = dispatch(&f.app, "memory_get", &json!({"id":a.id})).unwrap();
    assert_eq!(detail["backlinks"].as_array().unwrap().len(), 1);
    // No session row exists: provenance is intentionally independent of the session foreign keys.
    assert_eq!(
        dispatch(&f.app, "memory_source", &json!({"id":"source"})).unwrap()["content"],
        "完整来源"
    );
    dispatch(&f.app, "memory_delete", &json!({"id":a.id,"version":1})).unwrap();
    assert!(dispatch(&f.app, "memory_source", &json!({"id":"source"})).is_ok());
}

#[test]
fn chunking_preserves_all_utf8_and_cli_decoders_reject_incomplete_output() {
    let source = "知识🙂\n".repeat(20000);
    let chunks = runner::chunks(&source);
    assert!(chunks.len() > 1);
    assert_eq!(chunks.concat(), source);
    assert!(chunks.iter().all(|c| c.chars().count() <= 24000));
    let valid = json!({"entries":[]});
    assert_eq!(
        runner::process::decode(
            "claude",
            &json!({"structured_output":valid,"is_error":false}).to_string()
        )
        .unwrap()
        .0,
        valid
    );
    assert!(runner::process::decode(
        "claude",
        &json!({"structured_output":valid,"is_error":true}).to_string()
    )
    .is_err());
    assert!(runner::process::decode(
        "codex",
        &json!({"type":"item.completed","item":{"type":"agent_message","text":valid.to_string()}})
            .to_string()
    )
    .is_err());
}

#[test]
fn interrupted_jobs_become_retryable_without_disturbing_live_jobs() {
    let f = Fixture::new();
    f.source("s", "test");
    f.failed_job("job", "s", "claude");
    let conn = f.app.db().conn.lock().unwrap();
    conn.execute(
        "UPDATE memory_jobs SET status='running',updated_at=?1",
        [now() - 130000],
    )
    .unwrap();
    runner::recover(&conn).unwrap();
    assert_eq!(
        conn.query_row("SELECT error FROM memory_jobs", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "memory_interrupted"
    );
    conn.execute(
        "UPDATE memory_jobs SET status='running',updated_at=?1",
        [now()],
    )
    .unwrap();
    runner::recover(&conn).unwrap();
    assert_eq!(
        conn.query_row("SELECT status FROM memory_jobs", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "running"
    );
}

#[cfg(unix)]
#[test]
fn both_agents_merge_two_conversations_with_provenance_and_deduplication() {
    for agent in ["claude", "codex"] {
        let f = Fixture::new();
        f.cli("normal");
        f.source("s1", "会话一：服务端口 24680。");
        f.source("s2", "会话二：增加请求 ID 校验。");
        for (source, old) in [("s1", "old1"), ("s2", "old2")] {
            f.failed_job(old, source, agent);
            let result = runner::retry(&f.app, old).unwrap();
            let job = f.await_job(result["id"].as_str().unwrap());
            assert_eq!(job["status"], "completed", "{agent}: {job}");
        }
        let entries = repo::all(&f.app.db().conn.lock().unwrap()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].content.contains("24680"));
        assert!(entries[0].content.contains("请求 ID"));
        assert_eq!(entries[0].sources.len(), 2);
        assert_eq!(entries[0].version, 2);
        let again = runner::retry(&f.app, "old2").unwrap();
        assert_eq!(again["reused"], true);
        assert_eq!(
            std::fs::read_to_string(f.dir.join("calls"))
                .unwrap()
                .lines()
                .count(),
            4
        );
        let other_agent = if agent == "claude" { "codex" } else { "claude" };
        f.failed_job("other", "s2", other_agent);
        let other = runner::retry(&f.app, "other").unwrap();
        assert_eq!(
            f.await_job(other["id"].as_str().unwrap())["status"],
            "completed"
        );
        let original = runner::retry(&f.app, "old2").unwrap();
        assert_eq!(original["reused"], true);
        assert_eq!(original["id"], again["id"]);
        assert_eq!(
            std::fs::read_to_string(f.dir.join("calls"))
                .unwrap()
                .lines()
                .count(),
            6
        );
    }
}

#[cfg(unix)]
#[test]
fn invalid_agent_output_never_writes_memory() {
    let f = Fixture::new();
    f.cli("invalid");
    f.source("s", "source");
    f.failed_job("old", "s", "claude");
    let result = runner::retry(&f.app, "old").unwrap();
    let job = f.await_job(result["id"].as_str().unwrap());
    assert_eq!(job["status"], "failed");
    assert_eq!(job["error"], "memory_invalid_output");
    assert!(repo::all(&f.app.db().conn.lock().unwrap())
        .unwrap()
        .is_empty());
}

#[cfg(unix)]
#[test]
fn cancelling_a_running_agent_leaves_no_partial_documents() {
    let f = Fixture::new();
    f.cli("slow");
    f.source("s", "source");
    f.failed_job("old", "s", "codex");
    let result = runner::retry(&f.app, "old").unwrap();
    let id = result["id"].as_str().unwrap();
    let start = Instant::now();
    while !f.dir.join("calls").exists() {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
    }
    runner::cancel(&f.app, id).unwrap();
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(f.await_job(id)["status"], "cancelled");
    assert!(repo::all(&f.app.db().conn.lock().unwrap())
        .unwrap()
        .is_empty());
}

#[cfg(unix)]
#[test]
fn human_edit_during_merge_is_preserved_and_job_fails_with_conflict() {
    let f = Fixture::new();
    f.cli("conflict");
    let entry = f.save("服务端口规范", "初始知识");
    f.source("s", "新知识");
    f.failed_job("old", "s", "claude");
    let result = runner::retry(&f.app, "old").unwrap();
    let id = result["id"].as_str().unwrap();
    let start = Instant::now();
    while !f.dir.join("merge-started").exists() {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
    }
    dispatch(&f.app,"memory_save",&json!({"id":entry.id,"version":1,"title":entry.title,"summary":"人工编辑","content":"必须保留人工修改","tags":[],"related":[]})).unwrap();
    std::fs::write(f.dir.join("continue"), "go").unwrap();
    let job = f.await_job(id);
    assert_eq!(job["status"], "failed");
    assert_eq!(job["error"], "memory_conflict");
    assert_eq!(
        repo::get(&f.app.db().conn.lock().unwrap(), &entry.id)
            .unwrap()
            .unwrap()
            .content,
        "必须保留人工修改"
    );
}

#[test]
fn old_memory_jobs_gain_an_empty_effort_without_losing_history() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(&include_str!("schema.sql").replace(" effort TEXT NOT NULL DEFAULT '',\n", "")).unwrap();
    conn.execute("INSERT INTO memory_sources VALUES('s','s','s','claude','s','digest','source',1)", []).unwrap();
    conn.execute("INSERT INTO memory_jobs(id,source_id,agent,model,status,stage,owner_pid,created_at,updated_at) VALUES('j','s','claude','opus','completed','done',0,1,1)", []).unwrap();
    init(&conn).unwrap(); init(&conn).unwrap();
    let saved: (String,String) = conn.query_row("SELECT model,effort FROM memory_jobs WHERE id='j'", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(saved, ("opus".into(), "".into()));
}

#[test]
fn model_selection_rejects_unknown_models_and_unsupported_effort() {
    let options = vec![runner::ModelOption { id: "model".into(), label: "Model".into(), effort_levels: vec!["high".into()] }];
    assert!(runner::validate_model(&options, "model", "high").is_ok());
    assert!(runner::validate_model(&options, "model", "").is_ok());
    assert!(runner::validate_model(&options, "unknown", "high").is_err());
    assert!(runner::validate_model(&options, "model", "ultra").is_err());
    assert!(runner::validate_model(&options, "", "high").is_err());
}

#[cfg(unix)]
#[test]
fn compilation_preserves_effort_on_retry_and_deduplicates_only_matching_settings() {
    for agent in ["claude", "codex"] {
        let f = Fixture::new(); f.cli("normal"); f.source("s", "Durable knowledge");
        let model = if agent == "claude" { "claude-opus-5" } else { "test-model" };
        for effort in ["low", "high"] {
            f.failed_job(effort, "s", agent);
            f.app.db().conn.lock().unwrap().execute("UPDATE memory_jobs SET model=?1,effort=?2 WHERE id=?2", params![model,effort]).unwrap();
            let result = runner::retry(&f.app, effort).unwrap();
            assert_eq!(result["reused"], false);
            let job = f.await_job(result["id"].as_str().unwrap());
            assert_eq!(job["status"], "completed", "{job}");
            assert_eq!(job["model"], model); assert_eq!(job["effort"], effort);
            assert_eq!(runner::retry(&f.app, effort).unwrap()["id"], result["id"]);
        }
        let args = std::fs::read_to_string(f.dir.join("args")).unwrap();
        let calls: Vec<Vec<String>> = args.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(calls.len(), 4);
        for (index, args) in calls.iter().enumerate() {
            let effort = if index < 2 { "low" } else { "high" };
            assert!(args.windows(2).any(|pair| pair == ["--model", model]));
            if agent == "claude" { assert!(args.windows(2).any(|pair| pair == ["--effort", effort])); }
            else { assert!(args.windows(2).any(|pair| pair[0] == "-c" && pair[1] == format!("model_reasoning_effort=\"{effort}\""))); }
        }
    }
}
