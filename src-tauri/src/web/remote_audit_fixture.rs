//! Opt-in host for account tunnel verification in vlx-browser. AI work requires an explicit visitor message.

#[test]
#[ignore = "requires VLX_REMOTE_AUDIT_DIR with a local account fixture"]
fn account_remote_browser_fixture() {
    use crate::db::repo;
    use crate::host::{AppCtx, HeadlessHost};
    use crate::models::SessionKind;
    use serde_json::json;
    let dir =
        std::path::PathBuf::from(std::env::var("VLX_REMOTE_AUDIT_DIR").expect("fixture directory"));
    crate::diagnostics::init(&dir, crate::VERSION);
    let seed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("seed.json")).unwrap()).unwrap();
    let origin = seed["origin"].as_str().unwrap();
    assert!(
        origin.starts_with("http://127.0.0.1:")
            || (origin == "https://velaterm.com"
                && std::env::var("VLX_REMOTE_AUDIT_PUBLIC").as_deref() == Ok("1"))
    );
    let port = seed["hostPort"].as_u64().unwrap() as u16;
    assert!((10000..=49151).contains(&port));
    let db = crate::db::Db::open(&dir.join("host.db")).unwrap();
    let (project, visible, other, terminal) = {
        let conn = db.conn.lock().unwrap();
        let project = if seed["chatVerification"].as_bool() == Some(true) {
            let path = dir.join("project");
            std::fs::create_dir_all(&path).unwrap();
            repo::import_project(&conn, path.to_str().unwrap()).unwrap()
        } else {
            repo::create_virtual_project(&conn, "远程验收空间").unwrap()
        };
        let private = repo::create_virtual_project(&conn, "未开放项目").unwrap();
        let visible = repo::create_session(
            &conn,
            &project.id,
            None,
            "共享 AI 会话",
            SessionKind::Claude,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let other = repo::create_session(
            &conn,
            &private.id,
            None,
            "范围外 AI 会话",
            SessionKind::Codex,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let terminal = repo::create_session(
            &conn,
            &project.id,
            None,
            "不可见终端",
            SessionKind::Terminal,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE sessions SET engine='chat' WHERE id IN (?1,?2)",
            [&visible.id, &other.id],
        )
        .unwrap();
        (project, visible, other, terminal)
    };
    let host = std::sync::Arc::new(HeadlessHost::new(dir.clone(), db));
    let app = AppCtx::Headless(host.clone());
    if seed["chatVerification"].as_bool() == Some(true) {
        let hook_port = seed["hookPort"].as_u64().expect("fixed audit hook port") as u16;
        host.set_hooks(
            crate::agent::server::HookServer::start_for_audit(app.clone(), hook_port).unwrap(),
        );
    }
    let grant = seed["grantId"].as_str().unwrap();
    let config = json!({"origin":origin,"deviceId":seed["deviceId"],"token":seed["token"],"shares":{
        grant:{"scope":"machine","targetId":null,"secret":"","passwordHash":null,"allowedAccounts":[seed["accountId"]]}
    }});
    super::write_owner_only(
        &dir.join("vlx-public-sharing.json"),
        &serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    let secret = uuid::Uuid::new_v4().to_string();
    let web = super::WebServer::new();
    web.start(
        app.clone(),
        super::StartAuth::Tunnel {
            secret: secret.clone(),
        },
        Some(port),
        super::ServeMode::ShareTunnel,
    )
    .unwrap();
    super::share_tunnel::start(
        app.clone(),
        origin.into(),
        seed["token"].as_str().unwrap().into(),
        port,
        secret,
    );
    std::fs::write(dir.join("ready.json"), serde_json::to_vec(&json!({"projectId":project.id,"sessionId":visible.id,"otherId":other.id,"terminalId":terminal.id})).unwrap()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1800);
    while !dir.join("stop").exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    if seed["chatVerification"].as_bool() == Some(true) {
        app.chat().stop(&app, &visible.id).unwrap();
    }
    web.stop();
    std::fs::remove_file(dir.join("vlx-public-sharing.json")).unwrap();
}
