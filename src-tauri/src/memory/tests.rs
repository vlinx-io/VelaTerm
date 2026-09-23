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
            if !["queued", "running", "cancelling"].contains(&job["status"].as_str().unwrap()) {
                return job.clone();
            }
            assert!(
                start.elapsed() < Duration::from_secs(12),
                "job timed out: {job}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    fn status(&self, id: &str) -> String {
        self.app
            .db()
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT status FROM memory_jobs WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
    }
    #[cfg(unix)]
    fn await_cli(&self, id: &str) -> i32 {
        let started = Instant::now();
        loop {
            if let Ok(pid) = std::fs::read_to_string(self.dir.join(format!("started-{id}"))) {
                return pid.parse().unwrap();
            }
            assert!(
                started.elapsed() < Duration::from_secs(8),
                "CLI did not start: {id}"
            );
            std::thread::sleep(Duration::from_millis(20));
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
if MODE=='gated':
 job=pathlib.Path.cwd().name
 (root/('started-'+job)).write_text(str(__import__('os').getpid()))
 while not (root/('release-'+job)).exists(): time.sleep(.02)
if MODE=='invalid': print('invalid');sys.exit(0)
if MODE=='empty': print(json.dumps({'structured_output':{'entries':[]},'is_error':False}));sys.exit(0)
if MODE=='slow': time.sleep(8)
if 'CATALOG:\n' in prompt:
 catalog=json.loads(prompt.split('CATALOG:\n',1)[1].split('\nSOURCE ',1)[0])
 target=catalog[0]['id'] if catalog else ''
 content=prompt.split('\nSOURCE ',1)[1].split(':\n',1)[1]
 result={'entries':[{'targetId':target,'title':'服务端口规范','summary':'本地服务配置','content':content,'tags':['技术'],'relatedTitles':[]}]}
else:
 target=re.search(r'targetId=([a-z0-9-]+)',prompt).group(1)
 topics=json.loads(prompt.split('CONTRIBUTIONS:\n',1)[1])
 content='\n'.join(t['content'] for t in topics)
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
fn search_ranks_relevance_above_recency_and_lists_relations() {
    let f = Fixture::new();
    f.save("无关主题", "别的内容。");
    let body = f.save("实现记录", "这里讨论限流的具体做法。");
    let titled = f.save("限流设计", "输出调度的一些历史。");
    dispatch(
        &f.app,
        "memory_save",
        &json!({"id":body.id,"version":1,"title":"实现记录","summary":"可复用的知识","content":"这里讨论限流的具体做法。","tags":["技术"],"related":[titled.id]}),
    )
    .unwrap();
    // The body-only match is the most recently updated, so relevance has to outrank recency.
    f.app
        .db()
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE memory_entries SET updated_at=?1 WHERE id=?2",
            params![now() + 10_000, body.id],
        )
        .unwrap();
    let list = dispatch(
        &f.app,
        "memory_list",
        &json!({"query":"限流","tag":"","sort":"updated","page":0}),
    )
    .unwrap();
    assert_eq!(list["total"], 2);
    assert_eq!(list["entries"][0]["id"], titled.id, "标题命中排在正文命中之前");
    assert!(list["entries"][0]["related"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == body.id), "反向引用也要列出");
    let body_hit = list["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == body.id)
        .unwrap();
    assert_eq!(body_hit["related"][0]["id"], titled.id);
    let browse = dispatch(
        &f.app,
        "memory_list",
        &json!({"query":"","tag":"","sort":"updated","page":0}),
    )
    .unwrap();
    assert!(
        browse["entries"][0].get("related").is_none(),
        "浏览不返回相关条目"
    );
}

#[test]
fn fuzzy_fallback_matches_typos_and_reports_snippets_and_matches() {
    let f = Fixture::new();
    let titled = f.save("Knowledge Base", "release notes");
    let body = f.save("Implementation record", "throttle design notes");
    f.save("Unrelated topic", "nothing here");

    // The exact search keeps the strict shape and reports snippet, line and literals.
    let exact = dispatch(
        &f.app,
        "memory_list",
        &json!({"query":"throttle","tag":"","sort":"updated","page":0}),
    )
    .unwrap();
    assert_eq!(exact["total"], 1);
    assert_eq!(exact["fuzzy"], false);
    assert_eq!(exact["entries"][0]["id"], body.id);
    assert_eq!(exact["entries"][0]["line"], 1);
    assert!(exact["entries"][0]["snippet"]
        .as_str()
        .unwrap()
        .contains("throttle"));
    assert_eq!(exact["entries"][0]["matched"], json!(["throttle"]));

    // A typo in the title matches the fuzzy fallback and marks the original word.
    let typo_title = dispatch(
        &f.app,
        "memory_list",
        &json!({"query":"knoledge base","tag":"","sort":"updated","page":0}),
    )
    .unwrap();
    assert_eq!(typo_title["fuzzy"], true);
    assert_eq!(typo_title["total"], 1);
    assert_eq!(typo_title["entries"][0]["id"], titled.id);
    assert_eq!(typo_title["entries"][0]["line"], 0);
    assert_eq!(
        typo_title["entries"][0]["matched"],
        json!(["Knowledge", "base"])
    );
    assert_eq!(typo_title["entries"][0]["snippet"], "release notes");

    // A typo in the body shows the word that was actually found.
    let typo_body = dispatch(
        &f.app,
        "memory_list",
        &json!({"query":"throtle","tag":"","sort":"updated","page":0}),
    )
    .unwrap();
    assert_eq!(typo_body["fuzzy"], true);
    assert_eq!(typo_body["total"], 1);
    assert_eq!(typo_body["entries"][0]["id"], body.id);
    assert_eq!(typo_body["entries"][0]["line"], 1);
    assert_eq!(typo_body["entries"][0]["matched"], json!(["throttle"]));
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
fn both_agents_isolate_sessions_and_replace_only_the_requested_collection() {
    for agent in ["claude", "codex"] {
        let f = Fixture::new();
        f.cli("normal");
        f.source("s1", "Session one: port 24680.");
        f.source("s2", "Session two: request IDs.");
        for source in ["s1", "s2"] {
            f.failed_job(source, source, agent);
            let job = runner::retry(&f.app, source).unwrap();
            assert_eq!(
                f.await_job(job["id"].as_str().unwrap())["status"],
                "completed"
            );
        }
        let entries = repo::all(&f.app.db().conn.lock().unwrap()).unwrap();
        assert_eq!(entries.len(), 2);
        let first = entries.iter().find(|e| e.session_id == "s1").unwrap();
        let second = entries.iter().find(|e| e.session_id == "s2").unwrap();
        assert!(!first.content.contains("request IDs"));
        assert!(!second.content.contains("24680"));
        dispatch(&f.app,"memory_save",&json!({"id":first.id,"version":1,"title":first.title,"summary":"manual","content":"manual edit","tags":[],"related":[]})).unwrap();
        let again = runner::retry(&f.app, "s1").unwrap();
        assert_eq!(again["reused"], false);
        assert_eq!(
            f.await_job(again["id"].as_str().unwrap())["status"],
            "completed"
        );
        let conn = f.app.db().conn.lock().unwrap();
        let updated = repo::get(&conn, &first.id).unwrap().unwrap();
        assert_eq!(updated.version, 3);
        assert!(!updated.content.contains("manual edit"));
        assert_eq!(repo::get(&conn, &second.id).unwrap().unwrap().version, 1);
        assert_eq!(repo::all(&conn).unwrap().len(), 2);
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
    f.app
        .db()
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE memory_entries SET session_id='s',title_key=json_array('s',title) WHERE id=?1",
            [&entry.id],
        )
        .unwrap();
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
    conn.execute_batch(
        &include_str!("schema.sql").replace(" effort TEXT NOT NULL DEFAULT '',\n", ""),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memory_sources VALUES('s','s','s','claude','s','digest','source',1)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO memory_jobs(id,source_id,agent,model,status,stage,owner_pid,created_at,updated_at) VALUES('j','s','claude','opus','completed','done',0,1,1)", []).unwrap();
    init(&conn).unwrap();
    init(&conn).unwrap();
    let saved: (String, String) = conn
        .query_row(
            "SELECT model,effort FROM memory_jobs WHERE id='j'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(saved, ("opus".into(), "".into()));
}

#[test]
fn model_selection_rejects_unknown_models_and_unsupported_effort() {
    let options = vec![runner::ModelOption {
        id: "model".into(),
        label: "Model".into(),
        effort_levels: vec!["high".into()],
    }];
    assert!(runner::validate_model(&options, "model", "high").is_ok());
    assert!(runner::validate_model(&options, "model", "").is_ok());
    assert!(runner::validate_model(&options, "unknown", "high").is_err());
    assert!(runner::validate_model(&options, "model", "ultra").is_err());
    assert!(runner::validate_model(&options, "", "high").is_err());
}

#[cfg(unix)]
#[test]
fn compilation_preserves_effort_on_retry_and_replaces_completed_results() {
    for agent in ["claude", "codex"] {
        let f = Fixture::new();
        f.cli("normal");
        f.source("s", "Durable knowledge");
        let model = if agent == "claude" {
            "claude-opus-5"
        } else {
            "test-model"
        };
        for effort in ["low", "high"] {
            f.failed_job(effort, "s", agent);
            f.app
                .db()
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE memory_jobs SET model=?1,effort=?2 WHERE id=?2",
                    params![model, effort],
                )
                .unwrap();
            let result = runner::retry(&f.app, effort).unwrap();
            assert_eq!(result["reused"], false);
            let job = f.await_job(result["id"].as_str().unwrap());
            assert_eq!(job["status"], "completed", "{job}");
            assert_eq!(job["model"], model);
            assert_eq!(job["effort"], effort);
            let repeated = runner::retry(&f.app, effort).unwrap();
            assert_ne!(repeated["id"], result["id"]);
            assert_eq!(
                f.await_job(repeated["id"].as_str().unwrap())["status"],
                "completed"
            );
        }
        let args = std::fs::read_to_string(f.dir.join("args")).unwrap();
        let calls: Vec<Vec<String>> = args
            .lines()
            .map(|line| serde_json::from_str::<Vec<String>>(line).unwrap())
            // The model catalogue probes the Claude fixture once over stream-json before the first job;
            // only the compilation runs are counted here.
            .filter(|args| !args.iter().any(|arg| arg == "--input-format"))
            .collect();
        assert_eq!(calls.len(), 8);
        for (index, args) in calls.iter().enumerate() {
            let effort = if index < 4 { "low" } else { "high" };
            assert!(args.windows(2).any(|pair| pair == ["--model", model]));
            if agent == "claude" {
                assert!(args.windows(2).any(|pair| pair == ["--effort", effort]));
            } else {
                assert!(args.windows(2).any(|pair| pair[0] == "-c"
                    && pair[1] == format!("model_reasoning_effort=\"{effort}\"")));
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn failed_regeneration_keeps_existing_entries_and_empty_success_clears_them() {
    let f = Fixture::new();
    f.cli("normal");
    f.source("s", "Original knowledge");
    f.failed_job("old", "s", "claude");
    let first = runner::retry(&f.app, "old").unwrap();
    assert_eq!(
        f.await_job(first["id"].as_str().unwrap())["status"],
        "completed"
    );
    let before = repo::all(&f.app.db().conn.lock().unwrap()).unwrap();
    f.cli("invalid");
    let failed = runner::retry(&f.app, "old").unwrap();
    assert_eq!(
        f.await_job(failed["id"].as_str().unwrap())["status"],
        "failed"
    );
    assert_eq!(
        json!(before),
        json!(repo::all(&f.app.db().conn.lock().unwrap()).unwrap())
    );
    f.cli("empty");
    let empty = runner::retry(&f.app, "old").unwrap();
    assert_eq!(
        f.await_job(empty["id"].as_str().unwrap())["status"],
        "completed"
    );
    assert!(repo::all(&f.app.db().conn.lock().unwrap())
        .unwrap()
        .is_empty());
}

#[test]
fn migration_preserves_merged_content_and_freezes_project_and_session_names() {
    let f = Fixture::new();
    let conn = f.app.db().conn.lock().unwrap();
    conn.execute("INSERT INTO projects(id,name,root_path,created_at) VALUES('p','Original project','/tmp',1)",[]).unwrap();
    conn.execute("INSERT INTO sessions(id,project_id,name,kind,created_at) VALUES('s','p','Original session','claude',1)",[]).unwrap();
    conn.execute_batch("DROP TABLE memory_sessions; DROP INDEX memory_entries_session; ALTER TABLE memory_entries DROP COLUMN session_id;").unwrap();
    for (id, session) in [("src1", "s"), ("src2", "other")] {
        conn.execute("INSERT INTO memory_sources VALUES(?1,?2,'Original session','claude','','digest','source',1)",params![id,session]).unwrap();
    }
    for (id, sources) in [("single", "[\"src1\"]"), ("merged", "[\"src1\",\"src2\"]")] {
        conn.execute("INSERT INTO memory_entries VALUES(?1,?1,?1,'summary','preserved content','[]','[]',?2,1,1,1)",params![id,sources]).unwrap();
    }
    for (id, title) in [("plain", "foo"), ("json", "[\"\",\"foo\"]")] {
        conn.execute(
            "INSERT INTO memory_entries VALUES(?1,?2,?2,'','content','[]','[]','[]',1,1,1)",
            params![id, title],
        )
        .unwrap();
    }
    init(&conn).unwrap();
    conn.execute("UPDATE projects SET name='Renamed' WHERE id='p'", [])
        .unwrap();
    conn.execute("DELETE FROM sessions WHERE id='s'", [])
        .unwrap();
    init(&conn).unwrap();
    assert_eq!(repo::get(&conn, "single").unwrap().unwrap().session_id, "s");
    let merged = repo::get(&conn, "merged").unwrap().unwrap();
    assert_eq!(merged.session_id, "__legacy__");
    assert_eq!(merged.content, "preserved content");
    let metadata: (String, String) = conn
        .query_row(
            "SELECT name,project_name FROM memory_sessions WHERE id='s'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        metadata,
        ("Original session".into(), "Original project".into())
    );
    drop(conn);
    let list = repo::list(&f.app, &json!({"sessionId":"s"})).unwrap();
    assert_eq!(list["total"], 1);
    assert_eq!(list["projects"].as_array().unwrap().len(), 3);
}

#[test]
fn hierarchy_is_complete_across_pages_and_supports_manual_and_unknown_projects() {
    let f = Fixture::new();
    let manual = f.save("Manual entry", "Independent manual content");
    let mut conn = f.app.db().conn.lock().unwrap();
    super::hierarchy::snapshot(&conn, "s", "Deleted origin session").unwrap();
    let tx = conn.transaction().unwrap();
    for i in 0..45 {
        let mut entry = manual.clone();
        entry.id = format!("generated-{i}");
        entry.title = format!("Topic {i:02}");
        entry.session_id = "s".into();
        repo::put(&tx, entry, 0, "fixture").unwrap();
    }
    tx.commit().unwrap();
    drop(conn);
    let page = repo::list(&f.app, &json!({"sessionId":"s","page":1,"sort":"title"})).unwrap();
    assert_eq!(page["entries"].as_array().unwrap().len(), 5);
    assert_eq!(page["total"], 45);
    assert_eq!(page["projects"].as_array().unwrap().len(), 2);
    assert!(page["projects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == "__unknown__" && p["count"] == 45));
    assert_eq!(
        repo::list(&f.app, &json!({"projectId":"__manual__"})).unwrap()["total"],
        1
    );
    assert_eq!(
        repo::list(&f.app, &json!({"projectId":"__unknown__"})).unwrap()["total"],
        45
    );
    let direct = repo::list(
        &f.app,
        &json!({"selectedId":"generated-1","sessionId":"__manual__"}),
    )
    .unwrap();
    assert_eq!(direct["selectedSessionId"], "s");
    assert_eq!(direct["total"], 45);
    let filtered = repo::list(&f.app, &json!({"query":"Topic 44"})).unwrap();
    assert_eq!(filtered["projects"][0]["count"], 1);
}

#[test]
fn collections_group_archived_roots_by_original_project_and_keep_group_paths() {
    let f = Fixture::new();
    {
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute("INSERT INTO projects(id,name,root_path,created_at) VALUES('p','Alpha','/tmp/vlx-a',1)", []).unwrap();
        conn.execute("INSERT INTO groups(id,project_id,name,created_at) VALUES('g','p','Work',1)", []).unwrap();
        conn.execute("INSERT INTO groups(id,project_id,parent_group_id,name,created_at) VALUES('g2','p','g','Nested',1)", []).unwrap();
        conn.execute("INSERT INTO sessions(id,project_id,group_id,name,kind,archived_at,created_at) VALUES('s1','p','g2','Archived one','claude',100,1)", []).unwrap();
        // Tombstoned containers still name the session's original location, so they must not be filtered.
        conn.execute("UPDATE projects SET deleted_at=1 WHERE id='p'", []).unwrap();
        conn.execute("UPDATE groups SET deleted_at=1 WHERE id='g'", []).unwrap();
        // An archived descendant is not an archive root and gets no row of its own.
        conn.execute("INSERT INTO sessions(id,project_id,name,kind,parent_session_id,archived_at,created_at) VALUES('s2','p','Child','terminal','s1',100,1)", []).unwrap();
    }
    let entry = f.save("Stored topic", "Reusable content");
    {
        let conn = f.app.db().conn.lock().unwrap();
        repo::move_entry(&conn, &entry.id, entry.version, "s1", "manual").unwrap();
    }
    let result = dispatch(&f.app, "memory_collections", &json!({})).unwrap();
    let projects = result["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["id"], "p");
    assert_eq!(projects[0]["name"], "Alpha");
    assert_eq!(projects[0]["count"], 1);
    let sessions = projects[0]["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], "s1");
    assert_eq!(sessions[0]["count"], 1);
    assert_eq!(sessions[0]["groupPath"], json!(["Work", "Nested"]));
    assert_eq!(result["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(result["sessions"][0]["id"], "s1");
    assert_eq!(result["sessions"][0]["name"], "Archived one");
}

#[test]
fn knowledge_tree_rename_move_and_group_delete_keep_live_nodes_in_sync() {
    let f = Fixture::new();
    {
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute("INSERT INTO projects(id,name,root_path,sort_order,created_at) VALUES('p1','First project','/tmp/vlx-a',1,1)", []).unwrap();
        conn.execute("INSERT INTO projects(id,name,root_path,sort_order,created_at) VALUES('p2','Second project','/tmp/vlx-b',2,1)", []).unwrap();
        conn.execute("INSERT INTO sessions(id,project_id,name,kind,created_at) VALUES('s1','p1','Design session','claude',1)", []).unwrap();
        conn.execute("INSERT INTO sessions(id,project_id,name,kind,created_at) VALUES('s2','p1','Other session','claude',1)", []).unwrap();
        super::hierarchy::snapshot(&conn, "s1", "Design session").unwrap();
        super::hierarchy::snapshot(&conn, "s2", "Other session").unwrap();
    }
    let entry = f.save("Knowledge entry", "Reusable content");
    // Manual entries start ungrouped; moving one into a session writes the new grouping and revision.
    let moved = {
        let conn = f.app.db().conn.lock().unwrap();
        repo::move_entry(&conn, &entry.id, entry.version, "s1", "manual").unwrap()
    };
    assert_eq!(moved.session_id, "s1");
    let renamed = {
        let conn = f.app.db().conn.lock().unwrap();
        repo::rename_entry(&conn, &entry.id, moved.version, "Renamed entry", "manual").unwrap()
    };
    assert_eq!(renamed.title, "Renamed entry");
    assert_eq!(renamed.session_id, "s1");
    assert_eq!(renamed.version, moved.version + 1);

    // A title already present in the target group is refused and leaves the entry in the manual group.
    let other = f.save("Renamed entry", "Another content");
    let conflict = {
        let conn = f.app.db().conn.lock().unwrap();
        repo::move_entry(&conn, &other.id, other.version, "s1", "manual").unwrap_err()
    };
    assert_eq!(conflict, "memory_duplicate_title");

    // Group renames reach the live tables and the snapshots that label the knowledge tree.
    dispatch(&f.app, "memory_group_rename", &json!({"kind":"session","id":"s1","name":"Renamed session"})).unwrap();
    dispatch(&f.app, "memory_group_rename", &json!({"kind":"project","id":"p1","name":"Renamed project"})).unwrap();
    {
        let conn = f.app.db().conn.lock().unwrap();
        let live: String = conn.query_row("SELECT name FROM sessions WHERE id='s1'", [], |r| r.get(0)).unwrap();
        let (snap, project): (String, String) = conn.query_row("SELECT name,project_name FROM memory_sessions WHERE id='s1'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((live.as_str(), snap.as_str(), project.as_str()), ("Renamed session", "Renamed session", "Renamed project"));
    }

    // Moving a session between projects moves the live row and re-groups its entries.
    dispatch(&f.app, "memory_group_move", &json!({"sessionId":"s1","targetProjectId":"p2"})).unwrap();
    {
        let conn = f.app.db().conn.lock().unwrap();
        let live: String = conn.query_row("SELECT project_id FROM sessions WHERE id='s1'", [], |r| r.get(0)).unwrap();
        let (project, name): (String, String) = conn.query_row("SELECT project_id,project_name FROM memory_sessions WHERE id='s1'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((live.as_str(), project.as_str(), name.as_str()), ("p2", "p2", "Second project"));
    }

    // Group deletion removes only the entries under the group and keeps the live session.
    let removed = dispatch(&f.app, "memory_group_delete", &json!({"kind":"session","id":"s1"})).unwrap();
    assert_eq!(removed["removed"], 1);
    {
        let conn = f.app.db().conn.lock().unwrap();
        assert!(repo::get(&conn, &entry.id).unwrap().is_none());
        assert!(conn.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE id='s1')", [], |r| r.get::<_, bool>(0)).unwrap());
    }
    let manual = repo::list(&f.app, &json!({"projectId":"__manual__"})).unwrap();
    assert_eq!(manual["total"], 1);
}

#[cfg(unix)]
#[test]
fn independent_sessions_run_concurrently_and_replacement_stops_only_its_own_process() {
    let f = Fixture::new();
    f.cli("gated");
    f.source("one", "First session source.");
    f.source("two", "Second session source.");
    f.failed_job("retry-one", "one", "claude");
    f.failed_job("retry-two", "two", "codex");
    let first = runner::retry(&f.app, "retry-one").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let first_pid = f.await_cli(&first);
    let second = runner::retry(&f.app, "retry-two").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    f.await_cli(&second);
    assert_eq!(f.status(&first), "running");
    assert_eq!(
        f.status(&second),
        "running",
        "an independent session must not wait for the first"
    );

    // Even identical input and options replace an unfinished job instead of reusing it.
    let replacement = runner::retry(&f.app, "retry-one").unwrap();
    assert_eq!(replacement["reused"], false);
    let latest = replacement["id"].as_str().unwrap();
    assert_ne!(latest, first);
    f.await_cli(latest);
    assert_eq!(f.status(&first), "cancelled");
    assert_eq!(
        unsafe { libc::kill(first_pid, 0) },
        -1,
        "the old process must stop before its replacement starts"
    );
    assert_eq!(
        f.status(&second),
        "running",
        "replacement must not cancel another session"
    );
    assert!(repo::all(&f.app.db().conn.lock().unwrap())
        .unwrap()
        .is_empty());

    std::fs::write(f.dir.join(format!("release-{latest}")), "go").unwrap();
    std::fs::write(f.dir.join(format!("release-{second}")), "go").unwrap();
    assert_eq!(f.await_job(latest)["status"], "completed");
    assert_eq!(f.await_job(&second)["status"], "completed");
    let entries = repo::all(&f.app.db().conn.lock().unwrap()).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .any(|e| e.session_id == "one" && e.content.contains("First session")));
    assert!(entries
        .iter()
        .any(|e| e.session_id == "two" && e.content.contains("Second session")));
}

#[cfg(unix)]
#[test]
fn repeated_replacement_keeps_only_latest_snapshot_and_options_then_resumes_after_restart() {
    let f = Fixture::new();
    f.cli("normal");
    for source in ["old", "middle", "latest"] {
        f.source(source, source);
        f.failed_job(source, source, "claude");
    }
    {
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute("UPDATE memory_sources SET session_id='shared'", [])
            .unwrap();
        conn.execute("UPDATE memory_jobs SET status='running' WHERE id='old'", [])
            .unwrap();
        conn.execute("UPDATE memory_jobs SET agent='codex',model='test-model',effort='high' WHERE id='latest'", []).unwrap();
    }
    let middle = runner::retry(&f.app, "middle").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let latest = runner::retry(&f.app, "latest").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(f.status("old"), "cancelling");
    assert_eq!(f.status(&middle), "cancelled");
    assert_eq!(f.status(&latest), "queued");
    assert!(
        !f.dir.join("calls").exists(),
        "a replacement cannot run while the old process still owns the session"
    );

    // A second host opens the same durable queue. Stale cancellation releases its session slot.
    f.app
        .db()
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE memory_jobs SET updated_at=?1 WHERE id='old'",
            [now() - 130_000],
        )
        .unwrap();
    let reopened = AppCtx::Headless(Arc::new(HeadlessHost::new(
        f.dir.clone(),
        crate::db::Db::open(&f.dir.join("test.db")).unwrap(),
    )));
    super::resume(&reopened);
    let result = f.await_job(&latest);
    assert_eq!(result["status"], "completed");
    assert_eq!(result["model"], "test-model");
    assert_eq!(result["effort"], "high");
    assert_eq!(f.status("old"), "cancelled");
    let entries = repo::all(&f.app.db().conn.lock().unwrap()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].content, "latest");
    assert_eq!(
        std::fs::read_to_string(f.dir.join("calls"))
            .unwrap()
            .lines()
            .count(),
        2, // Only the latest task's extraction and merge execute.
    );
}

#[test]
fn claiming_is_session_scoped_across_connections_and_releases_failed_or_cancelled_jobs() {
    let f = Fixture::new();
    for source in ["a", "a-new", "b"] {
        f.source(source, source);
        f.failed_job(source, source, "claude");
    }
    let mut other = rusqlite::Connection::open(f.dir.join("test.db")).unwrap();
    {
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute(
            "UPDATE memory_sources SET session_id='a' WHERE id='a-new'",
            [],
        )
        .unwrap();
        conn.execute("UPDATE memory_jobs SET status='queued'", [])
            .unwrap();
    }
    let mut conn = f.app.db().conn.lock().unwrap();
    assert!(matches!(queue::claim(&mut conn).unwrap(), queue::Next::Job(id) if id == "a"));
    assert!(matches!(queue::claim(&mut other).unwrap(), queue::Next::Job(id) if id == "b"));
    assert!(matches!(
        queue::claim(&mut other).unwrap(),
        queue::Next::Waiting
    ));
    conn.execute("UPDATE memory_jobs SET status='failed' WHERE id='a'", [])
        .unwrap();
    assert!(matches!(queue::claim(&mut other).unwrap(), queue::Next::Job(id) if id == "a-new"));
    conn.execute("UPDATE memory_jobs SET status='queued' WHERE id='a'", [])
        .unwrap();
    drop(conn);
    runner::cancel(&f.app, "a").unwrap();
    assert_eq!(f.status("a"), "cancelled");
    assert!(matches!(
        queue::claim(&mut other).unwrap(),
        queue::Next::Idle
    ));
}

#[test]
fn upgrading_removes_the_global_compiler_limit_without_losing_jobs() {
    let f = Fixture::new();
    for source in ["a", "b"] {
        f.source(source, source);
        f.failed_job(source, source, "claude");
    }
    let conn = f.app.db().conn.lock().unwrap();
    conn.execute_batch("CREATE UNIQUE INDEX memory_one_running ON memory_jobs((1)) WHERE status='running'; UPDATE memory_jobs SET status='running' WHERE id='a';").unwrap();
    super::init(&conn).unwrap();
    super::init(&conn).unwrap();
    conn.execute("UPDATE memory_jobs SET status='running' WHERE id='b'", [])
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM memory_jobs WHERE status='running'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
}
