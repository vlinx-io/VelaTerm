//! 登录由原生 CLI 执行，宿主只显示授权 URL 与完成状态。
use std::io::{BufRead, BufReader, Read};
use std::process::Stdio;
use std::sync::{Arc, Mutex, atomic::Ordering};
use std::time::{Duration, Instant};
use super::{AuthState, ChatProcess, AppCtx, emit_extras};

pub(in super::super) fn run(app: &AppCtx, session: &str, proc: &Arc<ChatProcess>, login: bool, family: &str) -> Result<(),String> {
    let start_plan = family.ends_with("-start-plan");
    let family = family.strip_suffix("-start-plan").unwrap_or(family);
    if !matches!(family,"zai"|"bigmodel") { return Err("Zcode 登录账户应为 zai 或 bigmodel".into()); }
    if !proc.alive.load(Ordering::Relaxed) { return Err("Zcode 进程已停止，请重新打开会话".into()); }
    if super::active(proc) { return Err("Zcode 账户操作正在进行".into()); }
    if proc.turn.lock().unwrap().running { return Err("请先等待 Zcode 当前任务结束".into()); }
    let mut cmd=crate::host::command(&proc.bin);
    crate::agent::executable::prepare_command(&mut cmd,&proc.bin);
    if let Some(cwd)=&proc.cwd { cmd.current_dir(cwd); }
    super::super::agent_environment(app, session, &mut cmd);
    crate::agent::zcode_cli::prepare_environment(&mut cmd, crate::host::home_dir());
    if login {
        cmd.args(["login", family]);
        if start_plan { cmd.arg("start-plan"); }
        cmd.args(["--no-browser","--json"]);
    }
    else { cmd.args(["logout","--json"]); }
    #[cfg(unix)] {use std::os::unix::process::CommandExt;cmd.process_group(0);}
    let mut child=cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e|format!("无法启动 Zcode 原生登录：{e}"))?;
    let stdout=child.stdout.take().unwrap();
    let stderr=child.stderr.take().unwrap();
    let child=Arc::new(Mutex::new(child));
    proc.zcode.lock().unwrap().auth_child=Some(child.clone());
    let attempt = uuid::Uuid::new_v4().to_string();
    let mut state = AuthState::new(if login { "starting" } else { "signingOut" });
    state.attempt = attempt.clone();
    proc.extras.lock().unwrap().auth=Some(state);
    emit_extras(app,session,proc);
    // CLI 的成功 JSON 可能带账户资料，只排空 stdout，绝不写入聊天或诊断。
    std::thread::spawn(move || { let _=std::io::copy(&mut stdout.take(1024*1024),&mut std::io::sink()); });
    let (app2,session2,proc2)=(app.clone(),session.to_string(),proc.clone());
    let url_attempt = attempt.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if line.starts_with("https://") && line.len()<8192 {
                let mut extras=proc2.extras.lock().unwrap();
                if let Some(state)=extras.auth.as_mut().filter(|s|s.attempt == url_attempt && s.active()) { state.status="pending"; state.verification_url=Some(line); }
                drop(extras); emit_extras(&app2,&session2,&proc2);
            }
        }
    });
    let (app,session,proc)=(app.clone(),session.to_string(),proc.clone());
    std::thread::spawn(move || {
        let started=Instant::now();
        let success=loop {
            if !proc.alive.load(Ordering::Relaxed) || !current_attempt(&proc, &attempt) || started.elapsed()>Duration::from_secs(900) {
                super::super::terminate_owned(&mut child.lock().unwrap()); break false;
            }
            match child.lock().unwrap().try_wait() { Ok(Some(code))=>break code.success(), Err(_)=>break false, _=>{} }
            std::thread::sleep(Duration::from_millis(200));
        };
        // 与取消/重新登录共用操作锁；旧回调不能清空新 helper 或写入其账户状态。
        let action = proc.action.lock().unwrap();
        if !current_attempt(&proc, &attempt) { return; }
        proc.zcode.lock().unwrap().auth_child=None;
        proc.extras.lock().unwrap().auth=Some(AuthState::new(if success {if login{"success"}else{"signedOut"}}else{"failed"}));
        emit_extras(&app,&session,&proc);
        if success && login && proc.alive.load(Ordering::Relaxed) {
            // 账户注册表由 CLI 启动装配，重新打开同一原生会话以读取新账户，不创建新上下文。
            super::super::release_process(&app,&session,&proc);
            drop(action);
            if let Err(error)=crate::command_core::chat_start(&app,&session,None,None,false) {
                super::super::emit(&app,&session,serde_json::json!({"type":"error","message":format!("Zcode 登录已完成，恢复会话失败：{error}")}));
            }
        }
    });
    Ok(())
}

fn current_attempt(proc: &ChatProcess, attempt: &str) -> bool {
    proc.extras.lock().unwrap().auth.as_ref().is_some_and(|state| state.attempt == attempt && state.active())
}

pub(in super::super) fn stop(proc: &ChatProcess) {
    if let Some(child)=proc.zcode.lock().unwrap().auth_child.take() {
        super::super::terminate_owned(&mut child.lock().unwrap());
    }
}

pub(in super::super) fn cancel(app: &AppCtx, session: &str, proc: &Arc<ChatProcess>) -> Result<(),String> {
    stop(proc);
    proc.extras.lock().unwrap().auth=Some(AuthState::new("canceled"));
    emit_extras(app,session,proc);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canceled_login_cannot_complete_a_replacement_login() {
        let app = super::super::super::tests::ctx("zcode-auth-attempt");
        if let AppCtx::Headless(host) = &app {
            host.set_hooks(crate::agent::server::HookServer { port: 0, token: "fixture".into() });
        }
        let mut proc = super::super::super::tests::inert_process(crate::models::SessionKind::Zcode);
        let dir = std::env::temp_dir().join(format!("vlx-zcode-auth-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join(if cfg!(windows) { "login.cmd" } else { "login" });
        let contents = if cfg!(windows) {
            "@echo off\r\npowershell.exe -NoProfile -NonInteractive -Command \"Start-Sleep -Seconds 60\"\r\n"
        } else { "#!/bin/sh\nsleep 60\n" };
        std::fs::write(&script, contents).unwrap();
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        Arc::get_mut(&mut proc).unwrap().bin = script.to_string_lossy().into_owned();
        run(&app, "s", &proc, true, "bigmodel").unwrap();
        let replacement;
        {
            // 固定取消和重新登录已经完成、旧完成回调尚未拿到状态锁的交错顺序。
            let mut extras = proc.extras.lock().unwrap();
            stop(&proc);
            let child = crate::host::command(&proc.bin).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
            replacement = Arc::new(Mutex::new(child));
            proc.zcode.lock().unwrap().auth_child = Some(replacement.clone());
            let mut state = AuthState::new("starting");
            state.attempt = "replacement-login".into();
            extras.auth = Some(state);
        }
        std::thread::sleep(Duration::from_millis(500));
        let still_owned = proc.zcode.lock().unwrap().auth_child.as_ref().is_some_and(|child| Arc::ptr_eq(child, &replacement));
        let still_active = super::super::active(&proc);
        stop(&proc);
        super::super::super::terminate_owned(&mut replacement.lock().unwrap());
        std::fs::remove_file(script).unwrap();
        std::fs::remove_dir(dir).unwrap();
        assert!(still_owned, "旧登录完成线程不能清空新进程的所有权");
        assert!(still_active, "旧登录完成线程不能覆盖新登录的状态");
    }
}
