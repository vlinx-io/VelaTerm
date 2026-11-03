//! Contract tests use an isolated database and a local fixture endpoint, never personal credentials.
use super::*;

fn fixture() -> (TaskDir, AppCtx, String) {
    let dir=TaskDir(std::env::temp_dir().join(format!("vlx-translation-test-{}",uuid::Uuid::new_v4())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let db=crate::db::Db::open(&dir.0.join("test.db")).unwrap();
    let app=AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(dir.0.clone(),db)));
    let id={let conn=app.db().conn.lock().unwrap();let project=repo::create_virtual_project(&conn,"Translation tests").unwrap();
        let session=repo::create_session(&conn,&project.id,None,"Translation",SessionKind::Claude,None,None,None,None,None).unwrap();
        repo::set_session_engine(&conn,&session.id,"chat").unwrap();session.id};
    (dir,app,id)
}

fn configure(app: &AppCtx,sid: &str,url: &str) {
    let service=config::service_save(app,&json!({"name":"Fixture","baseUrl":url,"model":"fixture","apiKey":"fixture-private-key"})).unwrap();
    config::save(app,&json!({"engine":"api","serviceId":service})).unwrap();config::enable(app,true).unwrap();
    let mut prefs=config::Preferences::default();prefs.outgoing=true;prefs.target="de".into();prefs.reply_target="de".into();
    config::session_save(app,sid,&json!(prefs)).unwrap();
}

#[test]
fn google_configuration_needs_no_service_or_agent_and_survives_reload() {
    let (dir,app,_sid)=fixture();
    assert!(config::settings(&app,true).unwrap()["engines"].as_array().unwrap().contains(&json!("google")));
    config::save(&app,&json!({"engine":"google"})).unwrap();
    let cfg=config::read_config(&app.db().conn.lock().unwrap()).unwrap();
    assert!(engine_ready(&app,&cfg).is_ok());assert!(cfg.agent.is_empty());assert!(cfg.service_id.is_empty());
    let reopened=crate::db::Db::open(&dir.0.join("test.db")).unwrap();
    assert_eq!(config::read_config(&reopened.conn.lock().unwrap()).unwrap().engine,"google");
}

#[test]
fn bing_configuration_needs_no_service_or_agent_and_survives_reload() {
    let (dir,app,_sid)=fixture();
    assert!(config::settings(&app,true).unwrap()["engines"].as_array().unwrap().contains(&json!("bing")));
    config::save(&app,&json!({"engine":"bing"})).unwrap();
    let cfg=config::read_config(&app.db().conn.lock().unwrap()).unwrap();
    assert!(engine_ready(&app,&cfg).is_ok());assert!(cfg.agent.is_empty());assert!(cfg.service_id.is_empty());
    let reopened=crate::db::Db::open(&dir.0.join("test.db")).unwrap();
    assert_eq!(config::read_config(&reopened.conn.lock().unwrap()).unwrap().engine,"bing");
}

#[test]
#[ignore = "Explicit live check of the anonymous Google endpoint; never runs in the default suite."]
fn google_live_translation_preserves_structure_and_reuses_cache() {
    let (_dir,app,_sid)=fixture();config::save(&app,&json!({"engine":"google"})).unwrap();
    let cfg=config::read_config(&app.db().conn.lock().unwrap()).unwrap();
    let original="# 你好世界\n\n请保留 `request_id` 和 https://example.com/a?q=1&x=2。\n```rs\nlet x = 1;\n```\n";
    let first=translate(&app,&cfg,"auto","en",original,"live-google").unwrap();
    assert_eq!(first.source_language,"zh-CN");assert_ne!(first.translated(),original);
    assert_eq!(first.pairs.iter().map(|p|p.original.as_str()).collect::<Vec<_>>().join("\n"),original);
    assert!(first.translated().contains("`request_id`"));assert!(first.translated().contains("https://example.com/a?q=1&x=2"));
    assert!(first.translated().contains("```rs\nlet x = 1;\n```"));
    let second=translate(&app,&cfg,"auto","en",original,"live-google-cache").unwrap();assert_eq!(first.translated(),second.translated());
    let test=translate(&app,&cfg,"en","de","Connection test.","connection-test-google").unwrap();
    assert!(!test.translated().trim().is_empty());assert_ne!(test.translated(),"Connection test.");
    println!("Google live check: {} / {}",first.source_language,test.translated());
}

#[test]
#[ignore = "Explicit live check of the anonymous Bing endpoint; never runs in the default suite."]
fn bing_live_translation_preserves_structure_and_reuses_cache() {
    let (_dir,app,_sid)=fixture();config::save(&app,&json!({"engine":"bing"})).unwrap();
    let cfg=config::read_config(&app.db().conn.lock().unwrap()).unwrap();
    let original="# 你好世界\n\n请保留 `request_id` 和 https://example.com/a?q=1&x=2。\n```rs\nlet x = 1;\n```\n";
    let first=translate(&app,&cfg,"auto","en",original,"live-bing").unwrap();
    assert_eq!(first.source_language,"zh-CN");assert_ne!(first.translated(),original);
    assert_eq!(first.pairs.iter().map(|p|p.original.as_str()).collect::<Vec<_>>().join("\n"),original);
    assert!(first.translated().contains("`request_id`"));assert!(first.translated().contains("https://example.com/a?q=1&x=2"));
    assert!(first.translated().contains("```rs\nlet x = 1;\n```"));
    let second=translate(&app,&cfg,"auto","en",original,"live-bing-cache").unwrap();assert_eq!(first.translated(),second.translated());
    let test=translate(&app,&cfg,"en","de","Connection test.","connection-test-bing").unwrap();
    assert!(!test.translated().trim().is_empty());assert_ne!(test.translated(),"Connection test.");
    println!("Bing live check: {} / {}",first.source_language,test.translated());
}

#[test]
fn conversation_scope_migrates_validates_and_survives_reload() {
    let (dir,app,sid)=fixture();
    let legacy=json!({"enabled":true,"engine":"agent","agent":"codex","model":"","serviceId":"","version":2,"generation":3,"enabledSince":1});
    app.db().conn.lock().unwrap().execute("INSERT INTO translation_config(id,value) VALUES(1,?1)",[legacy.to_string()]).unwrap();
    let initial=config::settings(&app,false).unwrap();
    assert_eq!(initial["config"]["defaultEnabled"],true);
    let supported=initial["conversationKinds"].as_array().unwrap().iter().map(|kind|kind["id"].clone()).collect::<Vec<_>>();
    assert_eq!(initial["config"]["conversationKinds"],json!(supported));
    assert!(supported.contains(&json!("antigravity")));assert!(!supported.contains(&json!("cline")));
    assert_eq!(session_state(&app,&sid).unwrap()["enabled"],true);
    config::set_conversation_kinds(&app,&json!(["codex","codex"])).unwrap();
    assert_eq!(session_state(&app,&sid).unwrap()["enabled"],false);
    for invalid in [json!(["shell"]),json!(["cline"]),json!(["unknown"]),json!(null),json!("codex")] {
        assert_eq!(config::set_conversation_kinds(&app,&invalid).unwrap_err(),"translation:configuration");
    }
    // Saving provider settings from an older client must not restore excluded conversation types.
    config::save(&app,&json!({"engine":"agent","agent":"codex"})).unwrap();
    let reopened=crate::db::Db::open(&dir.0.join("test.db")).unwrap();
    assert_eq!(config::read_config(&reopened.conn.lock().unwrap()).unwrap().conversation_kinds,vec![SessionKind::Codex]);
    config::set_conversation_kinds(&app,&json!([])).unwrap();
    config::enable(&app,false).unwrap();config::enable(&app,true).unwrap();
    assert!(config::read_config(&app.db().conn.lock().unwrap()).unwrap().conversation_kinds.is_empty());
}

#[test]
fn excluded_conversations_bypass_translation_and_reject_stale_jobs() {
    let (_dir,app,sid)=fixture();configure(&app,&sid,"http://127.0.0.1:23917/v1");
    let mut prefs=config::read_preferences(&app.db().conn.lock().unwrap(),&sid).unwrap();prefs.source="de".into();
    config::session_save(&app,&sid,&json!(prefs)).unwrap();
    let request=json!({"sessionId":sid,"messageId":"scope-test","text":"Hallo","images":[]});
    prepare(&app,&request).unwrap();
    config::set_conversation_kinds(&app,&json!(["codex"])).unwrap();
    assert!(guard_original(&app,&sid,&json!({"text":"Hello"})).is_ok());
    assert_eq!(prepare(&app,&request).unwrap_err(),"translation:disabled");
    assert_eq!(config::session_save(&app,&sid,&json!(prefs)).err().unwrap(),"translation:disabled");
    assert_eq!(schedule_reply(&app,&sid,"excluded-reply","Hello",false,false).unwrap_err(),"translation:disabled");
    observe(&app,&sid,&json!({"type":"rows","rows":[{"id":"excluded-auto","kind":"assistant","text":"Hello","at":now_ms()}]}));
    let excluded=session_state(&app,&sid).unwrap();
    assert_eq!(excluded["enabled"],false);assert_eq!(excluded["pending"][0]["status"],"canceled");
    assert!(excluded["replies"].as_array().unwrap().is_empty());
    config::set_conversation_kinds(&app,&json!(["claude"])).unwrap();
    assert_eq!(session_state(&app,&sid).unwrap()["enabled"],true);
    assert_eq!(send(&app,&sid,"Hallo",vec![],None,"scope-test").unwrap_err(),"translation:canceled");
    assert_eq!(guard_original(&app,&sid,&json!({"text":"Hello"})).unwrap_err(),"translation:required");
}

#[test]
fn effort_is_compatible_with_legacy_config_and_persisted_independently() {
    let (_dir,app,_sid)=fixture();
    let legacy=json!({"enabled":true,"engine":"agent","agent":"codex","model":"old-model","serviceId":"","version":2,"generation":3,"enabledSince":1});
    app.db().conn.lock().unwrap().execute("INSERT INTO translation_config(id,value) VALUES(1,?1)",[legacy.to_string()]).unwrap();
    let initial=config::settings(&app,true).unwrap();assert_eq!(initial["config"]["effort"],"");
    let codex=initial["agents"].as_array().unwrap().iter().find(|a|a["id"]=="codex").unwrap();
    assert_eq!(codex["effortFlag"],"model_reasoning_effort");
    config::save(&app,&json!({"engine":"agent","agent":"codex","model":"chosen-model","effort":"ultra"})).unwrap();
    let saved=config::read_config(&app.db().conn.lock().unwrap()).unwrap();
    assert_eq!(saved.effort,"ultra");assert_eq!(saved.model,"chosen-model");assert!(saved.enabled);
    assert_eq!(saved.version,3);assert_eq!(saved.generation,4);
    assert!(config::save(&app,&json!({"engine":"agent","agent":"codex","effort":"high --unsafe"})).is_err());
    assert_eq!(config::read_config(&app.db().conn.lock().unwrap()).unwrap().effort,"ultra");
    config::save(&app,&json!({"engine":"agent","agent":"codex","model":"chosen-model","effort":""})).unwrap();
    assert_eq!(config::settings(&app,true).unwrap()["config"]["effort"],"");
}

#[test]
fn actual_agent_command_inherits_effort_and_applies_explicit_overrides() {
    let (dir,app,_sid)=fixture();
    let bin=std::env::current_exe().unwrap().to_string_lossy().into_owned();
    let defaults=json!({"agentDefaults":{"codex":{"args":"--model default-model -c model_reasoning_effort=high"}}});
    app.db().conn.lock().unwrap().execute("INSERT INTO app_settings(key,value,updated_at) VALUES('vlx-settings',?1,0)",[defaults.to_string()]).unwrap();
    let mut cfg=config::Config::default();cfg.agent="codex".into();
    let args=|cfg:&config::Config|agent_command(&app,cfg,&bin,&dir.0).unwrap().0.get_args().map(|arg|arg.to_string_lossy().into_owned()).collect::<Vec<_>>();
    let inherited=args(&cfg);
    assert!(inherited.iter().any(|a|a.starts_with("model_instructions_file=")));
    assert!(inherited.iter().any(|a|a=="developer_instructions=\"\""));
    assert!(inherited.iter().any(|a|a=="skills.max_context_tokens=1"));
    assert!(std::fs::read_to_string(dir.0.join("translation-instructions.md")).unwrap().starts_with("You are a translation engine."));
    assert!(inherited.windows(2).any(|a|a==["--model","default-model"]));
    assert!(inherited.windows(2).any(|a|a==["-c","model_reasoning_effort=high"]));
    cfg.model="chosen-model".into();cfg.effort="ultra".into();let explicit=args(&cfg);
    assert!(explicit.windows(2).any(|a|a==["--model","chosen-model"]));
    assert!(explicit.windows(2).any(|a|a==["-c","model_reasoning_effort=ultra"]));
    assert!(!explicit.iter().any(|a|a=="default-model" || a=="model_reasoning_effort=high"));
    for (agent,flag) in [("claude","--effort"),("opencode","--variant"),("pi","--thinking"),("omp","--thinking"),("grok","--reasoning-effort")] {
        cfg.agent=agent.into();cfg.effort="high".into();
        assert!(args(&cfg).windows(2).any(|a|a==[flag,"high"]),"missing effort flag for {agent}");
    }
    let fallback=json!({"chatModelByKind":{"codex":"fallback-model"},"chatEffortByModel":{"codex:fallback-model":"low"}});
    app.db().conn.lock().unwrap().execute("UPDATE app_settings SET value=?1 WHERE key='vlx-settings'",[fallback.to_string()]).unwrap();
    cfg.agent="codex".into();cfg.model.clear();cfg.effort.clear();let selected=args(&cfg);
    assert!(selected.windows(2).any(|a|a==["--model","fallback-model"]));
    assert!(selected.windows(2).any(|a|a==["-c","model_reasoning_effort=low"]));
}

#[test]
fn defaults_credentials_and_remote_write_policy() {
    let (_dir,app,sid)=fixture();let initial=config::settings(&app,false).unwrap();
    assert_eq!(initial["config"]["enabled"],false);
    assert_eq!(session_state(&app,&sid).unwrap()["preferences"]["outgoing"],true);
    assert_eq!(session_state(&app,&sid).unwrap()["preferences"]["replies"],true);
    configure(&app,&sid,"https://fixture.example/v1");
    let response=config::settings(&app,false).unwrap();
    assert!(!response.to_string().contains("fixture-private-key"));assert_eq!(response["services"][0]["hasKey"],true);
    assert!(config::base_url("https://secret@fixture.example/v1").is_err());
    assert!(config::base_url("https://fixture.example/v1?api_key=secret").is_err());
    let error=crate::web::dispatch::dispatch(&app,"translation_service_save",&json!({}),"test",crate::web::dispatch::CallOrigin::Remote).unwrap_err();
    assert_eq!(error,"remote_cmd_forbidden:translation_service_save");
    assert!(guard_original(&app,&sid,&json!({"text":"Hello"})).is_err());
    assert!(guard_original(&app,&sid,&json!({"text":"/review"})).is_ok());
    config::enable(&app,false).unwrap();assert!(guard_original(&app,&sid,&json!({"text":"Hello"})).is_ok());
}

#[test]
fn service_key_round_trip_is_local_and_settings_never_include_it() {
    use crate::web::dispatch::{dispatch as call,CallOrigin};
    let (_dir,app,_sid)=fixture();
    let mut service=json!({"name":"Credential fixture","baseUrl":"https://fixture.example/v1","model":"fixture","apiKey":"fixture-saved-key"});
    let id=config::service_save(&app,&service).unwrap();service["id"]=json!(id);
    let args=json!({"id":id});
    assert_eq!(call(&app,"translation_service_key",&args,"test",CallOrigin::Local).unwrap(),json!("fixture-saved-key"));
    assert_eq!(call(&app,"translation_service_key",&args,"test",CallOrigin::Remote).unwrap_err(),"remote_cmd_forbidden:translation_service_key");
    assert!(!config::settings(&app,true).unwrap().to_string().contains("fixture-saved-key"));
    service["apiKey"]=json!("fixture-replacement-key");config::service_save(&app,&service).unwrap();
    assert_eq!(call(&app,"translation_service_key",&args,"test",CallOrigin::Local).unwrap(),json!("fixture-replacement-key"));
    service["apiKey"]=json!("");service["clearKey"]=json!(true);config::service_save(&app,&service).unwrap();
    assert_eq!(call(&app,"translation_service_key",&args,"test",CallOrigin::Local).unwrap(),json!(""));
    assert_eq!(config::settings(&app,true).unwrap()["services"][0]["hasKey"],false);
}

#[test]
fn same_language_skips_service_and_cancellation_invalidates_ready_delivery() {
    let (_dir,app,sid)=fixture();configure(&app,&sid,"http://127.0.0.1:23917/v1");
    let mut prefs=config::read_preferences(&app.db().conn.lock().unwrap(),&sid).unwrap();prefs.source="de".into();
    config::session_save(&app,&sid,&json!(prefs)).unwrap();
    let id=format!("msg-{}",uuid::Uuid::new_v4());let request=json!({"sessionId":sid,"messageId":id,"text":"Hallo `x`","behavior":"steer","images":[]});
    prepare(&app,&request).unwrap();prepare(&app,&request).unwrap();
    let ready=session_state(&app,&sid).unwrap();assert_eq!(ready["messages"][0]["accepted"],false);
    assert_eq!(ready["messages"][0]["result"]["pairs"][0]["translation"],"Hallo `x`");
    cancel(&app,&sid,&id).unwrap();assert_eq!(send(&app,&sid,"Hallo `x`",vec![],Some("steer"),&id).unwrap_err(),"translation:canceled");
    assert!(guard_original(&app,&sid,&json!({"text":"Hallo `x`","messageId":id,"originalFallback":true})).is_ok());
    assert!(guard_original(&app,&sid,&json!({"text":"Changed","messageId":id,"originalFallback":true})).is_err());
    prepare(&app,&request).unwrap();config::enable(&app,false).unwrap();
    assert_eq!(send(&app,&sid,"Hallo `x`",vec![],Some("steer"),&id).unwrap_err(),"translation:canceled");
}

#[test]
fn compatible_api_preserves_fragments_and_reuses_cache() {
    let (_dir,app,sid)=fixture();
    let (server,port)=(0..100).find_map(|_| {let port=10000+(uuid::Uuid::new_v4().as_u128()%39000) as u16;
        tiny_http::Server::http(("127.0.0.1",port)).ok().map(|s|(s,port))}).unwrap();
    configure(&app,&sid,&format!("http://127.0.0.1:{port}/v1"));
    let worker=std::thread::spawn(move || {
        let mut req=server.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();assert_eq!(req.url(),"/v1/chat/completions");
        let mut body=String::new();req.as_reader().read_to_string(&mut body).unwrap();let value:Value=serde_json::from_str(&body).unwrap();
        let prompt=value["messages"][0]["content"].as_str().unwrap();let input:Vec<String>=serde_json::from_str(prompt.split("INPUT JSON:\n").nth(1).unwrap()).unwrap();
        let lines:Vec<_>=input.iter().map(|line|line.replace("Hello","Hallo")).collect();
        let output=json!({"choices":[{"message":{"content":json!({"sourceLanguage":"en","lines":lines}).to_string()},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":10}}).to_string();
        req.respond(tiny_http::Response::from_string(output)).unwrap();
    });
    let cfg=config::read_config(&app.db().conn.lock().unwrap()).unwrap();
    let first=translate(&app,&cfg,"en","de","Hello `request_id`\n","first").unwrap();worker.join().unwrap();
    assert_eq!(first.translated(),"Hallo `request_id`\n");
    let second=translate(&app,&cfg,"en","de","Hello `request_id`\n","second").unwrap();assert_eq!(first.translated(),second.translated());
}

#[test]
fn changed_preferences_invalidate_ready_text_and_fallback_has_no_translation_metadata() {
    let (_dir,app,sid)=fixture();configure(&app,&sid,"http://127.0.0.1:23917/v1");
    let mut prefs=config::read_preferences(&app.db().conn.lock().unwrap(),&sid).unwrap();prefs.source="de".into();
    config::session_save(&app,&sid,&json!(prefs)).unwrap();
    let id=format!("msg-{}",uuid::Uuid::new_v4());let request=json!({"sessionId":sid,"messageId":id,"text":"Hallo","images":[]});
    prepare(&app,&request).unwrap();prefs.target="en".into();config::session_save(&app,&sid,&json!(prefs)).unwrap();
    assert_eq!(send(&app,&sid,"Hallo",vec![],None,&id).unwrap_err(),"translation:canceled");
    cancel(&app,&sid,&id).unwrap();
    assert!(session_state(&app,&sid).unwrap()["messages"].as_array().unwrap().is_empty());
}

#[test]
fn streaming_replies_reuse_text_nodes_and_recover_complete_document_structure() {
    let (_dir,app,sid)=fixture();
    let (server,port)=(0..100).find_map(|_| {let port=10000+(uuid::Uuid::new_v4().as_u128()%39000) as u16;
        tiny_http::Server::http(("127.0.0.1",port)).ok().map(|s|(s,port))}).unwrap();
    configure(&app,&sid,&format!("http://127.0.0.1:{port}/v1"));
    let mut prefs=config::read_preferences(&app.db().conn.lock().unwrap(),&sid).unwrap();prefs.replies=true;
    config::session_save(&app,&sid,&json!(prefs)).unwrap();
    let (sent,received)=std::sync::mpsc::channel();
    let worker=std::thread::spawn(move || { for expected in ["First.","Second."] {
        let mut req=server.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();let mut body=String::new();req.as_reader().read_to_string(&mut body).unwrap();
        let value:Value=serde_json::from_str(&body).unwrap();let prompt=value["messages"][0]["content"].as_str().unwrap();
        let input:Vec<String>=serde_json::from_str(prompt.split("INPUT JSON:\n").nth(1).unwrap()).unwrap();assert_eq!(input[0],expected);
        let output=json!({"choices":[{"message":{"content":json!({"sourceLanguage":"en","lines":input}).to_string()}}]}).to_string();
        std::thread::sleep(Duration::from_millis(60));
        req.respond(tiny_http::Response::from_string(output)).unwrap();sent.send(()).unwrap();
    }});
    schedule_reply(&app,&sid,"row","First.\n\npartial",true,true).unwrap();received.recv_timeout(Duration::from_secs(10)).unwrap();
    for _ in 0..200 {if session_state(&app,&sid).unwrap()["replies"][0]["state"]=="ready"{break;}std::thread::sleep(Duration::from_millis(10));}
    let first=session_state(&app,&sid).unwrap()["replies"][0]["durationMs"].as_u64().unwrap();assert!(first>=60);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(session_state(&app,&sid).unwrap()["replies"][0]["durationMs"],first);
    schedule_reply(&app,&sid,"row","First.\n\nSecond.",false,true).unwrap();received.recv_timeout(Duration::from_secs(10)).unwrap();
    for _ in 0..200 {if session_state(&app,&sid).unwrap()["replies"][0]["state"]=="ready"{break;}std::thread::sleep(Duration::from_millis(10));}
    let reply=session_state(&app,&sid).unwrap();assert_eq!(reply["replies"][0]["result"]["pairs"].as_array().unwrap().len(),3);
    assert_eq!(reply["replies"][0]["result"]["pairs"][2]["original"],"Second.");worker.join().unwrap();
    assert!(reply["replies"][0]["durationMs"].as_u64().unwrap()>=first+60);
    assert!(reply["replies"][0]["startedAt"].is_null());
}

fn another_session(app: &AppCtx,existing: &str) -> String {
    let conn=app.db().conn.lock().unwrap();
    let project:String=conn.query_row("SELECT project_id FROM sessions WHERE id=?1",[existing],|r|r.get(0)).unwrap();
    let session=repo::create_session(&conn,&project,None,"Another translation conversation",SessionKind::Claude,None,None,None,None,None).unwrap();
    repo::set_session_engine(&conn,&session.id,"chat").unwrap();session.id
}

#[test]
fn global_enable_preserves_explicit_switches_and_new_conversations_default_on() {
    let (dir,app,first)=fixture();config::enable(&app,true).unwrap();let second=another_session(&app,&first);
    assert!(config::read_preferences(&app.db().conn.lock().unwrap(),&first).unwrap().outgoing);
    assert!(config::read_preferences(&app.db().conn.lock().unwrap(),&second).unwrap().outgoing);
    let mut prefs=config::Preferences::default();prefs.outgoing=false;prefs.replies=false;prefs.target="en".into();prefs.reply_target="de".into();
    config::session_save(&app,&first,&json!(prefs)).unwrap();
    assert!(!config::read_preferences(&app.db().conn.lock().unwrap(),&first).unwrap().outgoing);
    assert!(config::read_preferences(&app.db().conn.lock().unwrap(),&second).unwrap().outgoing);
    config::enable(&app,false).unwrap();assert!(guard_original(&app,&second,&json!({"text":"Hello"})).is_ok());
    config::enable(&app,true).unwrap();
    let enabled=config::read_preferences(&app.db().conn.lock().unwrap(),&first).unwrap();
    assert!(!enabled.outgoing);assert!(!enabled.replies);assert_eq!(enabled.target,"en");assert_eq!(enabled.reply_target,"de");
    let third=another_session(&app,&first);assert!(session_state(&app,&third).unwrap()["preferences"]["outgoing"].as_bool().unwrap());
    drop(app);let db=crate::db::Db::open(&dir.0.join("test.db")).unwrap();
    assert!(!config::read_preferences(&db.conn.lock().unwrap(),&first).unwrap().outgoing);
    for id in [&second,&third] {assert!(config::read_preferences(&db.conn.lock().unwrap(),id).unwrap().outgoing);}
}

#[test]
fn default_switch_controls_inherited_preferences_and_preserves_explicit_choices_after_restart() {
    use crate::web::dispatch::{dispatch as call,CallOrigin};
    let (dir,app,first)=fixture();config::enable(&app,true).unwrap();
    let mut explicit=config::Preferences::default();explicit.target="en".into();explicit.reply_target="de".into();
    config::session_save(&app,&first,&json!(explicit)).unwrap();
    let inherited=another_session(&app,&first);
    call(&app,"translation_set_default_enabled",&json!({"enabled":false}),"test",CallOrigin::Remote).unwrap();
    let prefs=session_state(&app,&inherited).unwrap();
    assert_eq!(prefs["enabled"],true);assert_eq!(prefs["preferences"]["outgoing"],false);assert_eq!(prefs["preferences"]["replies"],false);
    assert_eq!(prefs["preferences"]["target"],"en");assert_eq!(prefs["preferences"]["replyTarget"],"de");
    assert!(guard_original(&app,&inherited,&json!({"text":"Hello"})).is_ok());
    assert!(guard_original(&app,&first,&json!({"text":"Hello"})).is_err());
    let paused=another_session(&app,&first);let mut explicit=config::read_preferences(&app.db().conn.lock().unwrap(),&paused).unwrap();
    explicit.outgoing=false;explicit.replies=false;config::session_save(&app,&paused,&json!(explicit)).unwrap();
    // An older provider draft cannot overwrite the independently saved default.
    config::save(&app,&json!({"engine":"google","defaultEnabled":true})).unwrap();
    config::enable(&app,false).unwrap();config::enable(&app,true).unwrap();
    assert_eq!(config::settings(&app,false).unwrap()["config"]["defaultEnabled"],false);
    drop(app);let db=crate::db::Db::open(&dir.0.join("test.db")).unwrap();
    let reopened=AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(dir.0.clone(),db)));
    assert_eq!(session_state(&reopened,&inherited).unwrap()["preferences"]["outgoing"],false);
    let new_session=another_session(&reopened,&first);
    assert_eq!(session_state(&reopened,&new_session).unwrap()["preferences"]["replies"],false);
    config::set_default_enabled(&reopened,true).unwrap();
    for id in [&first,&inherited,&new_session] {
        let prefs=session_state(&reopened,id).unwrap()["preferences"].clone();
        assert_eq!(prefs["outgoing"],true);assert_eq!(prefs["replies"],true);
    }
    let prefs=session_state(&reopened,&paused).unwrap()["preferences"].clone();assert_eq!(prefs["outgoing"],false);assert_eq!(prefs["replies"],false);
}

#[test]
fn default_switch_validates_input_and_rolls_back_failed_saves() {
    use crate::web::dispatch::{dispatch as call,CallOrigin};
    let (_dir,app,_sid)=fixture();
    for args in [json!({}),json!({"enabled":null}),json!({"enabled":"false"})] {
        assert_eq!(call(&app,"translation_set_default_enabled",&args,"test",CallOrigin::Local).unwrap_err(),"translation:configuration");
    }
    config::set_default_enabled(&app,true).unwrap();
    app.db().conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_default_switch BEFORE INSERT ON translation_config BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(config::set_default_enabled(&app,false).is_err());
    assert_eq!(config::settings(&app,true).unwrap()["config"]["defaultEnabled"],true);
}

#[test]
fn legacy_off_defaults_migrate_once_without_resetting_later_local_pauses() {
    let (dir,app,sid)=fixture();config::enable(&app,true).unwrap();
    let mut prefs=config::Preferences::default();prefs.outgoing=false;prefs.target="en".into();prefs.reply_target="de".into();
    config::session_save(&app,&sid,&json!(prefs)).unwrap();
    let legacy=json!({"source":"auto","target":"en","replySource":"auto","replyTarget":"de","display":"immersive"});
    app.db().conn.lock().unwrap().execute("UPDATE translation_defaults SET value=?1 WHERE id=1",[legacy.to_string()]).unwrap();
    drop(app);let db=crate::db::Db::open(&dir.0.join("test.db")).unwrap();let conn=db.conn.lock().unwrap();
    let upgraded=config::read_preferences(&conn,&sid).unwrap();assert!(upgraded.outgoing);assert_eq!(upgraded.target,"en");
    conn.execute("UPDATE translation_preferences SET value=?1 WHERE session_id=?2",params![json!(prefs).to_string(),sid]).unwrap();
    config::migrate(&conn).unwrap();assert!(!config::read_preferences(&conn,&sid).unwrap().outgoing);
}

#[test]
fn languages_are_inherited_but_conversation_switches_and_overrides_stay_independent() {
    let (dir,app,first)=fixture();config::enable(&app,true).unwrap();
    let second=another_session(&app,&first);
    let mut p=config::Preferences::default();p.target="en".into();p.reply_target="zh-CN".into();p.outgoing=true;p.replies=true;
    config::session_save(&app,&first,&json!(p)).unwrap();
    let inherited=config::read_preferences(&app.db().conn.lock().unwrap(),&second).unwrap();
    assert_eq!(inherited.target,"en");assert_eq!(inherited.reply_target,"zh-CN");assert!(inherited.outgoing);assert!(inherited.replies);
    let mut p=inherited;p.source="en".into();p.target="de".into();p.display="both".into();
    config::session_save(&app,&second,&json!(p)).unwrap();
    let mut old=config::read_preferences(&app.db().conn.lock().unwrap(),&first).unwrap();
    assert!(old.outgoing);assert!(old.replies);assert_eq!(old.target,"en");old.outgoing=false;
    config::session_save(&app,&first,&json!(old)).unwrap();
    let third=another_session(&app,&first);
    let latest=session_state(&app,&third).unwrap()["preferences"].clone();
    assert_eq!(latest["source"],"en");assert_eq!(latest["target"],"de");assert_eq!(latest["display"],"both");assert_eq!(latest["replyTarget"],"zh-CN");
    drop(app);
    let reopened=crate::db::Db::open(&dir.0.join("test.db")).unwrap();let conn=reopened.conn.lock().unwrap();
    assert_eq!(json!(config::read_preferences(&conn,&third).unwrap()),latest);
    assert_eq!(config::read_preferences(&conn,&first).unwrap().target,"en");
    assert!(!config::read_preferences(&conn,&first).unwrap().outgoing);
    assert_eq!(config::read_preferences(&conn,&second).unwrap().display,"both");
}

#[test]
fn legacy_languages_are_recovered_once_without_overwriting_explicit_preferences() {
    let (dir,app,first)=fixture();let second=another_session(&app,&first);
    let mut old=config::Preferences::default();old.target="en".into();old.reply_target="zh-CN".into();old.outgoing=true;
    {let conn=app.db().conn.lock().unwrap();
        conn.execute("INSERT INTO translation_preferences(session_id,value) VALUES(?1,?2)",params![first,json!(old).to_string()]).unwrap();
        conn.execute("INSERT INTO translation_preferences(session_id,value) VALUES(?1,?2)",params![second,json!(config::Preferences::default()).to_string()]).unwrap();
        conn.execute("DROP TABLE translation_defaults",[]).unwrap();}
    drop(app);
    let db=crate::db::Db::open(&dir.0.join("test.db")).unwrap();let conn=db.conn.lock().unwrap();
    let inherited=config::read_preferences(&conn,&second).unwrap();
    assert_eq!(inherited.target,"en");assert_eq!(inherited.reply_target,"zh-CN");assert!(inherited.outgoing);
    assert!(config::read_preferences(&conn,&first).unwrap().outgoing);
    conn.execute("DELETE FROM translation_preferences",[]).unwrap();config::migrate(&conn).unwrap();
    assert_eq!(config::read_preferences(&conn,&first).unwrap().target,"en");
}

#[test]
fn switching_without_a_target_is_saved_but_cannot_translate_or_bypass_sending() {
    let (_dir,app,sid)=fixture();config::enable(&app,true).unwrap();
    let mut p=config::Preferences::default();p.outgoing=true;p.replies=true;
    config::session_save(&app,&sid,&json!(p)).unwrap();
    assert_eq!(session_state(&app,&sid).unwrap()["preferences"]["outgoing"],true);
    assert_eq!(prepare(&app,&json!({"sessionId":sid,"messageId":"unconfigured","text":"Hello","images":[]})).unwrap_err(),"translation:language");
    assert_eq!(guard_original(&app,&sid,&json!({"text":"Hello"})).unwrap_err(),"translation:required");
    assert_eq!(schedule_reply(&app,&sid,"unconfigured","Hello",false,true).unwrap_err(),"translation:language");
    assert_eq!(app.db().conn.lock().unwrap().query_row("SELECT state FROM translation_jobs WHERE message_id='unconfigured'",[],|r|r.get::<_,String>(0)).unwrap(),"failed");
    assert!(guard_original(&app,&sid,&json!({"text":"Hello","messageId":"unconfigured","originalFallback":true})).is_ok());
    assert!(guard_original(&app,&sid,&json!({"text":"Changed","messageId":"unconfigured","originalFallback":true})).is_err());
    p.outgoing=false;p.replies=false;config::session_save(&app,&sid,&json!(p)).unwrap();
    assert_eq!(session_state(&app,&sid).unwrap()["preferences"]["replies"],false);
    assert!(guard_original(&app,&sid,&json!({"text":"Hello"})).is_ok());
}

#[test]
fn failed_default_write_rolls_back_conversation_preferences() {
    let (_dir,app,sid)=fixture();config::enable(&app,true).unwrap();
    app.db().conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_defaults BEFORE UPDATE ON translation_defaults BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    let mut p=config::Preferences::default();p.target="en".into();p.outgoing=true;
    assert!(config::session_save(&app,&sid,&json!(p)).is_err());
    let stored=config::read_preferences(&app.db().conn.lock().unwrap(),&sid).unwrap();
    assert!(stored.outgoing);assert!(stored.target.is_empty());
}

#[test]
fn native_translation_bindings_survive_restart_and_distinguish_identical_messages() {
    let (dir,app,sid)=fixture();configure(&app,&sid,"https://fixture.example/v1");
    let rows=vec!["client-one".to_string(),"client-two".to_string()];
    bind_native_message(&app,&sid,"native-one",&rows);
    bind_native_message(&app,&sid,"native-one",&rows);
    bind_native_message(&app,&sid,"native-two",&rows);
    drop(app);
    let reopened=crate::db::Db::open(&dir.0.join("test.db")).unwrap();let conn=reopened.conn.lock().unwrap();
    let mut messages=vec![json!({"id":"client-one","original":"One","result":{"pairs":[{"translation":"Same"}]}}),json!({"id":"client-two","original":"Two","result":{"pairs":[{"translation":"Same"}]}})];
    let mut replies=vec![];
    append_replay_translations(&conn,&sid,vec![("h-0".into(),"user".into(),vec!["native-two".into()],"Same".into()),("h-1".into(),"user".into(),vec!["native-one".into()],"Same".into())],&mut messages,&mut replies).unwrap();
    assert_eq!(messages[2]["original"],"Two");assert_eq!(messages[3]["original"],"One");
}

#[test]
fn translation_replay_aliases_follow_native_ids_and_merged_assistant_blocks() {
    let content=[
        json!({"type":"response_item","payload":{"type":"message","role":"user","id":"response-id","content":[{"type":"input_text","text":"Hello"}]}}),
        json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"UserMessage","id":"native-user"}}}),
        json!({"type":"response_item","payload":{"type":"message","role":"assistant","id":"answer-one","content":[{"type":"output_text","text":"Hello"}]}}),
        json!({"type":"response_item","payload":{"type":"message","role":"assistant","id":"answer-two","content":[{"type":"output_text","text":"Hello"}]}}),
    ].iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let aliases=super::super::chat::history::codex_translation_aliases(&content);
    assert_eq!(aliases,vec![("h-0".into(),"user".into(),vec!["native-user".into()],"Hello".into()),("h-1".into(),"assistant".into(),vec!["answer-one".into(),"answer-two".into()],"Hello\n\nHello".into())]);
    let (_dir,app,sid)=fixture();let conn=app.db().conn.lock().unwrap();
    let mut replies=[("answer-one","First"),("answer-two","Second")].iter().map(|(id,translated)|json!({"id":id,"original":"Hello","state":"ready","version":1,"generation":1,"source":"en","target":"de","result":{"sourceLanguage":"en","pairs":[{"original":"Hello","translation":translated}]}})).collect::<Vec<_>>();
    replies[0]["durationMs"]=json!(120);replies[1]["durationMs"]=json!(340);
    append_replay_translations(&conn,&sid,aliases.clone(),&mut vec![],&mut replies).unwrap();
    assert_eq!(replies[2]["id"],"h-1");assert_eq!(replies[2]["result"]["pairs"].as_array().unwrap().len(),3);
    assert_eq!(replies[2]["result"]["pairs"][1]["original"],"");
    assert_eq!(replies[2]["durationMs"],460);
    replies.truncate(2);replies[1]["durationMs"]=Value::Null;
    append_replay_translations(&conn,&sid,aliases,&mut vec![],&mut replies).unwrap();
    assert!(replies[2]["durationMs"].is_null());
}

#[test]
fn claude_translation_binding_is_persisted_before_send_and_reused_after_restart() {
    let (dir,app,sid)=fixture();configure(&app,&sid,"https://fixture.example/v1");
    assert!(claude_message_uuid(&app,&sid,"plain-message").unwrap().is_none());
    let mut prefs=config::read_preferences(&app.db().conn.lock().unwrap(),&sid).unwrap();prefs.source="de".into();
    config::session_save(&app,&sid,&json!(prefs)).unwrap();
    prepare(&app,&json!({"sessionId":sid,"messageId":"translated","text":"Hallo","images":[]})).unwrap();
    let native=claude_message_uuid(&app,&sid,"translated").unwrap().unwrap();
    assert!(uuid::Uuid::parse_str(&native).is_ok());
    assert_eq!(claude_message_uuid(&app,&sid,"translated").unwrap(),Some(native.clone()));
    drop(app);
    let reopened=crate::db::Db::open(&dir.0.join("test.db")).unwrap();let conn=reopened.conn.lock().unwrap();
    let mut messages=vec![json!({"id":"translated","original":"Hallo","result":{"pairs":[{"original":"Hallo","translation":"Hallo"}]}})];
    append_replay_translations(&conn,&sid,vec![("h-0".into(),"user".into(),vec![native],"Hallo".into())],&mut messages,&mut vec![]).unwrap();
    assert_eq!(messages[1]["id"],"h-0");assert_eq!(messages[1]["original"],"Hallo");
}

#[test]
fn claude_saved_replies_reappear_under_replayed_ids_without_retranslation() {
    let (_dir,app,sid)=fixture();let conn=app.db().conn.lock().unwrap();
    let content=[
        json!({"type":"assistant","message":{"id":"answer","content":[{"type":"thinking","thinking":""}]}}),
        json!({"type":"assistant","message":{"id":"answer","content":[{"type":"text","text":" First\n"}]}}),
        json!({"type":"assistant","message":{"id":"answer","content":[{"type":"text","text":"Second"}]}}),
    ].iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let aliases=super::super::chat::history::claude_translation_aliases(&content);
    let mut replies=vec![
        json!({"id":"answer:1","original":" First\n","state":"ready","source":"en","target":"de","version":2,"generation":5,"durationMs":123,"result":{"formatVersion":2,"pairs":[{"original":" First","translation":" Erste"},{"original":"","translation":""}]}}),
        json!({"id":"answer:2","original":"Second","state":"ready","source":"en","target":"de","version":2,"generation":5,"durationMs":456,"result":{"formatVersion":2,"pairs":[{"original":"Second","translation":"Zweite"}]}}),
    ];
    append_replay_translations(&conn,&sid,aliases,&mut vec![],&mut replies).unwrap();
    let restored=replies.iter().find(|r|r["id"]=="h-0").unwrap();
    assert_eq!(restored["original"],"First\n\nSecond");assert_eq!(restored["durationMs"],579);
    assert_eq!(restored["result"]["pairs"],json!([{"original":"First","translation":"Erste"},{"original":"","translation":""},{"original":"Second","translation":"Zweite"}]));
}

#[test]
fn translation_duration_survives_reload_and_cache_hits_measure_their_own_work() {
    let (dir,app,sid)=fixture();
    let (server,port)=(0..100).find_map(|_| {let port=10000+(uuid::Uuid::new_v4().as_u128()%39000) as u16;
        tiny_http::Server::http(("127.0.0.1",port)).ok().map(|s|(s,port))}).unwrap();
    configure(&app,&sid,&format!("http://127.0.0.1:{port}/v1"));
    let worker=std::thread::spawn(move || {
        let req=server.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
        std::thread::sleep(Duration::from_millis(150));
        req.respond(tiny_http::Response::from_string(json!({"choices":[{"message":{"content":"{\"sourceLanguage\":\"en\",\"lines\":[\"Hallo\"]}"}}]}).to_string())).unwrap();
    });
    let request=json!({"sessionId":sid,"messageId":"first","text":"Hello","images":[]});
    prepare(&app,&request).unwrap();worker.join().unwrap();
    let first=session_state(&app,&sid).unwrap()["messages"][0]["durationMs"].as_u64().unwrap();assert!(first>=150);
    prepare(&app,&request).unwrap();assert_eq!(session_state(&app,&sid).unwrap()["messages"][0]["durationMs"],first);
    prepare(&app,&json!({"sessionId":sid,"messageId":"cache","text":"Hello","images":[]})).unwrap();
    let state=session_state(&app,&sid).unwrap();
    let cached=state["messages"].as_array().unwrap().iter().find(|m|m["id"]=="cache").unwrap();
    assert!(cached["durationMs"].as_u64().unwrap()<first);
    drop(app);
    let db=crate::db::Db::open(&dir.0.join("test.db")).unwrap();
    assert_eq!(db.conn.lock().unwrap().query_row("SELECT duration_ms FROM translation_jobs WHERE id='first'",[],|r|r.get::<_,u64>(0)).unwrap(),first);
}

#[test]
fn canceled_translation_duration_freezes_and_retry_starts_a_new_measurement() {
    let (_dir,app,sid)=fixture();
    let (server,port)=(0..100).find_map(|_| {let port=10000+(uuid::Uuid::new_v4().as_u128()%39000) as u16;
        tiny_http::Server::http(("127.0.0.1",port)).ok().map(|s|(s,port))}).unwrap();
    configure(&app,&sid,&format!("http://127.0.0.1:{port}/v1"));
    let (seen,received)=std::sync::mpsc::channel();let (release,wait)=std::sync::mpsc::channel();
    let worker=std::thread::spawn(move || {
        let req=server.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();seen.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(10)).unwrap();
        req.respond(tiny_http::Response::from_string(json!({"choices":[{"message":{"content":"{\"sourceLanguage\":\"en\",\"lines\":[\"Hallo\"]}"}}]}).to_string())).unwrap();
    });
    let request=json!({"sessionId":sid,"messageId":"cancel","text":"Hello","images":[]});
    let task_app=app.clone();let task_request=request.clone();let task=std::thread::spawn(move ||prepare(&task_app,&task_request));
    received.recv_timeout(Duration::from_secs(10)).unwrap();
    let active=session_state(&app,&sid).unwrap();assert!(active["pending"][0]["startedAt"].is_u64());assert_eq!(active["pending"][0]["durationMs"],0);
    std::thread::sleep(Duration::from_millis(100));cancel(&app,&sid,"cancel").unwrap();
    let canceled=session_state(&app,&sid).unwrap()["pending"][0].clone();assert!(canceled["startedAt"].is_null());assert!(canceled["durationMs"].as_u64().unwrap()>=100);
    release.send(()).unwrap();worker.join().unwrap();assert_eq!(task.join().unwrap().unwrap_err(),"translation:canceled");
    assert_eq!(session_state(&app,&sid).unwrap()["pending"][0]["durationMs"],canceled["durationMs"]);
    prepare(&app,&request).unwrap();
    assert!(session_state(&app,&sid).unwrap()["messages"][0]["durationMs"].as_u64().unwrap()<canceled["durationMs"].as_u64().unwrap());
}

#[test]
fn duration_migration_keeps_legacy_values_unknown_and_is_idempotent() {
    let (_dir,app,sid)=fixture();configure(&app,&sid,"https://fixture.example/v1");
    let mut prefs=config::read_preferences(&app.db().conn.lock().unwrap(),&sid).unwrap();prefs.source="de".into();
    config::session_save(&app,&sid,&json!(prefs)).unwrap();
    prepare(&app,&json!({"sessionId":sid,"messageId":"legacy","text":"Hallo","images":[]})).unwrap();
    let conn=app.db().conn.lock().unwrap();
    for table in ["translation_jobs","translation_replies"] {for column in ["started_at","duration_ms"] {conn.execute(&format!("ALTER TABLE {table} DROP COLUMN {column}"),[]).unwrap();}}
    config::migrate(&conn).unwrap();config::migrate(&conn).unwrap();drop(conn);
    assert!(session_state(&app,&sid).unwrap()["messages"][0]["durationMs"].is_null());
}
