//! 原生 CLI 的短时查询适配：模型目录与离线历史均经过同一 NDJSON 边界。
use std::io::{BufRead,BufReader,Read,Write};
use std::process::Stdio;
use std::sync::{Arc,Mutex,mpsc};
use std::time::Duration;
use serde_json::{json,Value};

/// 仅共享 GUI 的 Personal Provider 文件；账户与会话继续由 CLI 的既有 profile 管理。
pub fn prepare_environment(cmd: &mut std::process::Command, home: Option<std::path::PathBuf>) {
    let configured = |key: &str| {
        cmd.get_envs().find(|(name, _)| {
            if cfg!(windows) { name.to_string_lossy().eq_ignore_ascii_case(key) } else { *name == std::ffi::OsStr::new(key) }
        }).map(|(_, value)| value.map(std::ffi::OsStr::to_os_string))
            .unwrap_or_else(|| std::env::var_os(key))
            .is_some_and(|value| !value.to_string_lossy().trim().is_empty())
    };
    // 显式 CLI profile 必须优先，尤其不能让隔离配置读到个人 GUI 的 Key。
    if configured("ZCODE_PERSONAL_PROVIDER_CONFIG_FILE") || configured("ZCODE_DATA_BASE_DIR") { return; }
    let Some(home) = home else { return; };
    let Some(settings) = std::fs::read_to_string(home.join(".zcode/v2/setting.json")).ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok()) else { return; };
    let Some(base) = settings["dataBaseDir"].as_str().map(str::trim).filter(|path| !path.is_empty()) else { return; };
    let base = std::path::Path::new(base);
    if !base.is_absolute() { return; }
    let personal = base.join(".zcode/v2/provider_config.json");
    if personal.is_file() {
        // GUI 迁移过数据目录时，默认 CLI 仍读 HOME。只复用配置路径，不迁移账户或原生历史。
        cmd.env("ZCODE_PERSONAL_PROVIDER_CONFIG_FILE", personal);
    }
}

pub fn query(bin: &str, cwd: Option<&str>, method: &str, params: Value) -> Result<Value,String> {
    let mut cmd=crate::host::command(bin);
    // 与会话启动统一重建登录环境，避免 USERPROFILE 等路径变量指向另一份历史库。
    crate::login_env::refresh_command(&mut cmd);
    prepare_environment(&mut cmd, crate::host::home_dir());
    query_command(cmd, bin, cwd, method, params)
}

fn query_command(mut cmd: std::process::Command, bin: &str, cwd: Option<&str>, method: &str, params: Value) -> Result<Value,String> {
    super::executable::prepare_command(&mut cmd,bin);
    cmd.args(["app-server","--stdio","--standalone"]);
    if let Some(cwd)=cwd {cmd.current_dir(cwd);}
    #[cfg(unix)] {use std::os::unix::process::CommandExt;cmd.process_group(0);}
    let mut child=cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e|format!("无法查询 Zcode CLI：{e}"))?;
    let stdin=Arc::new(Mutex::new(child.stdin.take().unwrap()));
    let stdout=child.stdout.take().unwrap();
    let stderr=child.stderr.take().unwrap();
    std::thread::spawn(move || {let _=std::io::copy(&mut stderr.take(1024*1024),&mut std::io::sink());});
    let (tx,rx)=mpsc::channel();
    let writer=stdin.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(frame)=serde_json::from_str::<Value>(&line) else {continue};
            if frame["id"]=="vela-query" && frame.get("method").is_none() {
                let result=if frame.get("error").is_some() {Err(frame["error"]["message"].as_str().unwrap_or("Zcode 查询失败").to_string())}else{Ok(frame["result"].clone())};
                let _=tx.send(result);break;
            }
            if frame.get("id").is_some() && frame.get("method").is_some() {
                let answer=if frame["method"]=="session/requestRuntimePreferences" {json!({"id":frame["id"],"result":{"nativeSearchEnhancementsEnabled":true,"memoryEnabled":false,"askUserQuestionAutoResolutionEnabled":false}})}
                    else {json!({"id":frame["id"],"error":{"code":-32601,"message":"只读查询不提供此宿主能力"}})};
                let _=writeln!(writer.lock().unwrap(),"{answer}");
            }
        }
    });
    // 等待回包前释放 stdin；CLI 初始化的反向运行偏好请求也要写入同一标准流。
    let sent={let mut writer=stdin.lock().unwrap();writeln!(writer,"{}",json!({"id":"vela-query","method":method,"params":params})).and_then(|_|writer.flush())};
    let result=match sent {
        Ok(())=>rx.recv_timeout(Duration::from_secs(30)).map_err(|_|"Zcode 查询超时或进程退出".to_string()).and_then(|r|r),
        Err(e)=>Err(format!("无法写入 Zcode 查询：{e}")),
    };
    drop(stdin);
    crate::host::kill_process_tree(&mut child);
    let _=child.wait();
    result
}

pub fn models(bin: &str) -> Result<Vec<String>,String> {
    let cwd=std::env::current_dir().map_err(|e|e.to_string())?.to_string_lossy().into_owned();
    let snapshot=query(bin,Some(&cwd),"session/create",json!({"workspace":{"workspacePath":cwd,"workspaceKey":cwd},"persistence":"deferred","titleGenerationEnabled":false}))?;
    Ok(snapshot["settings"]["model"]["available"].as_array().into_iter().flatten().filter(|m|m["disabledReason"].is_null()).filter_map(|m|Some(format!("{}/{}",m["ref"]["providerId"].as_str()?,m["ref"]["modelId"].as_str()?))).collect())
}

/// 尚未启动或已停止的会话也必须能选模型；临时目录查询不绑定或恢复用户原生会话。
pub fn session_models(app: &crate::host::AppCtx, session_id: &str, bin: &str, cwd: Option<&str>) -> Result<Vec<Value>,String> {
    let cwd = match cwd {
        Some(cwd) => cwd.to_string(),
        None => std::env::current_dir().map_err(|error| error.to_string())?.to_string_lossy().into_owned(),
    };
    let mut cmd = crate::host::command(bin);
    cmd.current_dir(&cwd);
    crate::agent::chat::engine::agent_environment(app, session_id, &mut cmd);
    prepare_environment(&mut cmd, crate::host::home_dir());
    let snapshot = query_command(cmd, bin, Some(&cwd), "session/create", json!({"workspace":{"workspacePath":cwd,"workspaceKey":cwd},"persistence":"deferred","titleGenerationEnabled":false}))?;
    Ok(model_entries(&snapshot))
}

/// 套餐查询复用该会话环境，不创建或绑定原生历史。
pub fn session_start_plan(app: &crate::host::AppCtx, session_id: &str, bin: &str, cwd: Option<&str>) -> Result<Value,String> {
    let mut cmd = crate::host::command(bin);
    if let Some(cwd) = cwd { cmd.current_dir(cwd); }
    crate::agent::chat::engine::agent_environment(app, session_id, &mut cmd);
    prepare_environment(&mut cmd, crate::host::home_dir());
    query_command(cmd, bin, cwd, "provider/readStartPlan", json!({}))
}

/// 空白会话也从 CLI 的原生发现器读取技能；不扫描或复制另一套配置。
pub fn session_commands(app: &crate::host::AppCtx, session_id: &str, bin: &str, cwd: Option<&str>) -> Result<Vec<Value>,String> {
    let cwd = cwd.map(str::to_owned).unwrap_or_else(||std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned());
    let mut cmd = crate::host::command(bin);
    cmd.current_dir(&cwd);
    crate::agent::chat::engine::agent_environment(app, session_id, &mut cmd);
    prepare_environment(&mut cmd, crate::host::home_dir());
    let result = query_command(cmd, bin, Some(&cwd), "skills/referenceCatalog", json!({"workspace":{"workspacePath":cwd,"workspaceKey":cwd}}))?;
    skill_commands(&result)
}

pub fn skill_commands(result: &Value) -> Result<Vec<Value>,String> {
    let skills = result["skills"].as_array().ok_or("Zcode 未返回有效的技能目录")?;
    let mut names = std::collections::HashSet::new();
    Ok(skills.iter().filter(|skill|skill["enabled"]==true).filter_map(|skill| {
        let name=skill["name"].as_str().filter(|name|!name.is_empty())?;
        if !names.insert(name) { return None; }
        Some(json!({"name":name,"description":skill["description"].as_str().unwrap_or_default(),"invocation":"/"}))
    }).collect())
}

/// MCP 探测独占短时进程，慢握手不能堵住 resident 会话的串行请求队列。
pub fn session_mcp(app: &crate::host::AppCtx, session_id: &str, bin: &str, cwd: Option<&str>) -> Result<Value,String> {
    let cwd=cwd.map(str::to_owned).unwrap_or_else(||std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned());
    let mut cmd=crate::host::command(bin);
    cmd.current_dir(&cwd);
    crate::agent::chat::engine::agent_environment(app,session_id,&mut cmd);
    prepare_environment(&mut cmd,crate::host::home_dir());
    let result=query_command(cmd,bin,Some(&cwd),"mcp/list",json!({"workspace":{"workspacePath":cwd,"workspaceKey":cwd},"mode":"connect"}))?;
    let statuses=result["statuses"].as_object().ok_or("Zcode 未返回有效的 MCP 状态")?;
    Ok(json!({"mcpServers":statuses.iter().map(|(name,status)|json!({
        "name":name,"status":if status["status"]=="connecting" {json!("pending")}else{status["status"].clone()},
        "canToggle":false,"toolCount":status["toolCount"],"error":status["error"].as_str(),
    })).collect::<Vec<_>>()}))
}

/// 冷查询和运行中会话使用同一投影，保持来源名称、思考级别及字符串说明契约。
pub fn model_entries(snapshot: &Value) -> Vec<Value> {
    snapshot["settings"]["model"]["available"].as_array().into_iter().flatten()
        .filter(|item| item["disabledReason"].is_null()).filter_map(|item| {
            let description = item["description"].as_str().unwrap_or_default();
            let provider = item["providerLabel"].as_str().unwrap_or_default();
            let hint = if provider.is_empty() { description.to_string() }
                else if description.is_empty() { provider.to_string() }
                else { format!("{provider} · {description}") };
            Some(json!({"id":format!("{}/{}",item["ref"]["providerId"].as_str()?,item["ref"]["modelId"].as_str()?),
                "label":item["label"],"description":hint,"effortLevels":item["reasoning"]["levels"].as_array().into_iter().flatten().filter_map(|v|v["value"].as_str()).collect::<Vec<_>>()}))
        }).collect()
}

pub fn transcript(bin: &str, cwd: Option<&str>, native: &str) -> Result<Vec<super::transcript::TranscriptMessage>,String> {
    let mut params=json!({"sessionId":native});
    if let Some(cwd)=cwd {params["workspace"]=json!({"workspacePath":cwd,"workspaceKey":cwd});}
    let snapshot=query(bin,cwd,"session/resume",params)?;
    Ok(snapshot["messages"].as_array().into_iter().flatten().filter(|m|matches!(m["info"]["role"].as_str(),Some("user"|"assistant"))).map(|message| {
        let text=message["parts"].as_array().into_iter().flatten().filter(|p|p["type"]=="text").filter_map(|p|p["text"].as_str()).collect::<String>();
        let tools=message["parts"].as_array().into_iter().flatten().filter(|p|p["type"]=="tool").filter_map(|p|p["tool"].as_str().map(str::to_string)).collect();
        super::transcript::TranscriptMessage{role:message["info"]["role"].as_str().unwrap().into(),text,timestamp:None,tools}
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zcode_gui_providers_are_shared_without_moving_cli_login_or_history() {
        let root = std::env::temp_dir().join(format!("vlx-zcode-profile-{}", uuid::Uuid::new_v4()));
        let home = root.join("用户 home");
        let desktop = root.join("GUI 数据");
        std::fs::create_dir_all(home.join(".zcode/v2")).unwrap();
        std::fs::create_dir_all(desktop.join(".zcode/v2")).unwrap();
        let personal = desktop.join(".zcode/v2/provider_config.json");
        std::fs::write(&personal, "GUI 配置应保持原样").unwrap();
        std::fs::write(home.join(".zcode/v2/setting.json"), json!({"dataBaseDir":desktop}).to_string()).unwrap();
        let mut cmd = crate::host::command("zcode");
        cmd.env_remove("ZCODE_PERSONAL_PROVIDER_CONFIG_FILE").env_remove("ZCODE_DATA_BASE_DIR");
        cmd.env("USERPROFILE", &home).env("HOME", &home).env("ZCODE_STORAGE_DIR", root.join("cli-history"));
        prepare_environment(&mut cmd, Some(home.clone()));
        let env: std::collections::HashMap<_, _> = cmd.get_envs().collect();
        assert_eq!(env.get(std::ffi::OsStr::new("ZCODE_PERSONAL_PROVIDER_CONFIG_FILE")), Some(&Some(personal.as_os_str())));
        assert_eq!(env.get(std::ffi::OsStr::new("ZCODE_DATA_BASE_DIR")), Some(&None));
        assert_eq!(env.get(std::ffi::OsStr::new("HOME")), Some(&Some(home.as_os_str())));
        assert_eq!(env.get(std::ffi::OsStr::new("USERPROFILE")), Some(&Some(home.as_os_str())));
        assert_eq!(env.get(std::ffi::OsStr::new("ZCODE_STORAGE_DIR")), Some(&Some(root.join("cli-history").as_os_str())));
        assert_eq!(std::fs::read_to_string(&personal).unwrap(), "GUI 配置应保持原样");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn zcode_explicit_cli_profile_has_priority_over_gui_discovery() {
        for key in ["ZCODE_PERSONAL_PROVIDER_CONFIG_FILE", "ZCODE_DATA_BASE_DIR"] {
            let mut cmd = crate::host::command("zcode");
            cmd.env_remove("ZCODE_PERSONAL_PROVIDER_CONFIG_FILE").env_remove("ZCODE_DATA_BASE_DIR");
            cmd.env(key, "explicit-cli-profile");
            let before: Vec<_> = cmd.get_envs().map(|(k,v)| (k.to_os_string(),v.map(std::ffi::OsStr::to_os_string))).collect();
            prepare_environment(&mut cmd, crate::host::home_dir());
            let after: Vec<_> = cmd.get_envs().map(|(k,v)| (k.to_os_string(),v.map(std::ffi::OsStr::to_os_string))).collect();
            assert_eq!(after, before, "显式配置必须完整保留，不能读取宿主 GUI 配置");
        }
    }

    #[test]
    fn zcode_missing_or_invalid_gui_settings_keep_cli_defaults() {
        let home = std::env::temp_dir().join(format!("vlx-zcode-invalid-profile-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(home.join(".zcode/v2")).unwrap();
        let file = home.join(".zcode/v2/setting.json");
        for settings in [None, Some("bad json".to_string()), Some(json!({"dataBaseDir":42}).to_string()),
            Some(json!({"dataBaseDir":"relative-profile"}).to_string()), Some(json!({"dataBaseDir":home.join("missing")}).to_string())] {
            if let Some(settings) = settings { std::fs::write(&file, settings).unwrap(); }
            let mut cmd = crate::host::command("zcode");
            cmd.env_remove("ZCODE_PERSONAL_PROVIDER_CONFIG_FILE").env_remove("ZCODE_DATA_BASE_DIR");
            prepare_environment(&mut cmd, Some(home.clone()));
            assert!(cmd.get_envs().all(|(_,value)| value.is_none()), "无 GUI 配置时不得更改 CLI 环境");
        }
        std::fs::remove_dir_all(home).unwrap();
    }
}
