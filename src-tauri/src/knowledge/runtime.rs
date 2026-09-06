//! Pinned standalone runtime. Downloads are explicit; ordinary indexing performs no network requests.
use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

pub const VERSION: &str = "1.6.0";
fn asset() -> Result<(&'static str, &'static str)> {
    match (std::env::consts::OS,std::env::consts::ARCH) {
        ("macos","aarch64") => Ok(("darwin-arm64","1c73033512d55f67be04717e81532e8beaf7be6fb8531f51a179fa23064ad480")),
        ("macos","x86_64") => Ok(("darwin-x64","cb86a2b62ee676b62a56bf8423600e7d867e752e57f323cdc98c0f6236efd908")),
        ("linux","aarch64") => Ok(("linux-arm64","6dc935a7b8f1a61e688a578b98ea34680eb2e36d7b91db079d64f4011f1a668f")),
        ("linux","x86_64") => Ok(("linux-x64","de3391f79ed42622d937e6cd5b7642a7ea8bb7d1473607e80b879ba73ef216b0")),
        ("windows","aarch64") => Ok(("win32-arm64","3ca980010bd718a6b5e75be1145806ae6491afb1a59a2cec6cee4bf5c39f1b3a")),
        ("windows","x86_64") => Ok(("win32-x64","cd76c3c3391f2d40abef12b142151950b6d77abc2d8429e648f89eaa90f5b68a")),
        _ => Err("knowledge_platform".into()),
    }
}
fn home(app: &AppCtx) -> Result<PathBuf> { Ok(app.data_dir()?.join("codegraph").join(VERSION)) }
pub fn executable(app: &AppCtx) -> Result<(PathBuf,PathBuf)> {
    let root = home(app)?.join(format!("codegraph-{}",asset()?.0));
    let node = root.join(if cfg!(windows) {"node.exe"} else {"node"});
    let script = root.join("lib/dist/bin/codegraph.js");
    if !node.is_file() || !script.is_file() { return Err("knowledge_runtime_missing".into()); }
    Ok((node,script))
}
fn installations() -> &'static Mutex<HashMap<PathBuf, (String,String)>> {
    static STATE: OnceLock<Mutex<HashMap<PathBuf,(String,String)>>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}
pub fn status(app: &AppCtx) -> Result<Value> {
    let state = installations().lock().map_err(|_| "knowledge_busy")?.get(&home(app)?).cloned();
    let available = executable(app).is_ok();
    Ok(json!({"version":VERSION,"available":available,"supported":asset().is_ok(),
        "status":state.as_ref().map(|s|s.0.as_str()).unwrap_or(if available {"ready"} else {"missing"}),
        "error":state.map(|s|s.1).unwrap_or_default()}))
}
pub fn install_start(app: &AppCtx) -> Result<Value> {
    asset()?;
    let dir = home(app)?;
    if executable(app).is_ok() { return status(app); }
    {
        let mut states = installations().lock().map_err(|_| "knowledge_busy")?;
        if states.get(&dir).is_some_and(|s| s.0 == "installing") { return Err("knowledge_busy".into()); }
        states.insert(dir.clone(),("installing".into(),String::new()));
    }
    let ctx = app.clone();
    std::thread::spawn(move || {
        let started = Instant::now(); audit(&ctx,"system","INFO","runtime_install_started",0);
        let result = install(&ctx);
        audit(&ctx,"system",if result.is_ok(){"INFO"}else{"ERROR"},if result.is_ok(){"runtime_install_completed"}else{"runtime_install_failed"},started.elapsed().as_millis());
        installations().lock().unwrap().insert(dir,match result { Ok(()) => ("ready".into(),String::new()), Err(e) => ("failed".into(),e) });
        ctx.emit("knowledge://changed", ());
    });
    status(app)
}
fn install(app: &AppCtx) -> Result<()> {
    let (target,expected) = asset()?;
    let dir = home(app)?;
    let parent = dir.parent().ok_or("knowledge_install_failed")?;
    std::fs::create_dir_all(parent).map_err(|_| "knowledge_install_failed")?;
    let staging = parent.join(format!("download-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir(&staging).map_err(|_| "knowledge_install_failed")?;
    let result = (|| {
        let suffix = if cfg!(windows) {"zip"} else {"tar.gz"};
        let url = format!("https://github.com/colbymchenry/codegraph/releases/download/v{VERSION}/codegraph-{target}.{suffix}");
        let response = ureq::AgentBuilder::new().timeout(Duration::from_secs(300)).build().get(&url).call().map_err(|_| "knowledge_download_failed")?;
        let mut bytes = Vec::new();
        response.into_reader().take(160 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|_| "knowledge_download_failed")?;
        if bytes.len() > 160 * 1024 * 1024 || format!("{:x}",Sha256::digest(&bytes)) != expected { return Err("knowledge_checksum".into()); }
        if cfg!(windows) {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "knowledge_install_failed")?;
            archive.extract(&staging).map_err(|_| "knowledge_install_failed")?;
        } else {
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(&bytes[..]));
            archive.unpack(&staging).map_err(|_| "knowledge_install_failed")?;
        }
        let extracted = staging.join(format!("codegraph-{target}"));
        if !extracted.join(if cfg!(windows){"node.exe"}else{"node"}).is_file() || !extracted.join("lib/dist/bin/codegraph.js").is_file() { return Err("knowledge_install_failed".into()); }
        // Another app instance may have completed the same installation while we downloaded.
        if executable(app).is_ok() { return Ok(()); }
        let backup = parent.join(format!("replaced-{}",uuid::Uuid::new_v4()));
        let replaced = dir.exists();
        if replaced { std::fs::rename(&dir,&backup).map_err(|_| "knowledge_install_failed")?; }
        if std::fs::rename(&staging,&dir).is_err() {
            if replaced { let _=std::fs::rename(&backup,&dir); }
            return Err("knowledge_install_failed".into());
        }
        if replaced { let _=std::fs::remove_dir_all(&backup); }
        executable(app)?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(staging);
    result
}
fn stop(child: &mut Child) {
    #[cfg(unix)] unsafe { libc::kill(-(child.id() as i32),libc::SIGKILL); }
    #[cfg(windows)] { let _ = crate::host::command("taskkill").args(["/PID",&child.id().to_string(),"/T","/F"]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
    let _ = child.kill(); let _ = child.wait();
}
pub fn run(app: &AppCtx, index: &Index, job: Option<&str>, action: &str) -> Result<()> {
    let (node,script) = executable(app)?;
    let mut cmd = crate::host::command(node);
    // Use the SDK: CLI init may install Git hooks on filesystems without a watcher.
    // Index ownership belongs to VelaTerm and must not change the user's hooks or agent settings.
    const DRIVER: &str = "const {default:CG}=require(process.argv[1]); (async()=>{let cg;try{cg=process.argv[3]==='init'?await CG.init(process.argv[2],{index:true}):await CG.open(process.argv[2]);if(process.argv[3]!=='init'){const r=cg.getIndexState()==='complete'?await cg.sync():await cg.indexAll();if(r.success===false)process.exitCode=1;}}catch(e){process.exitCode=1;}finally{if(cg)cg.destroy();}})();";
    let entry = script.parent().and_then(Path::parent).ok_or("knowledge_runtime_missing")?.join("index.js");
    cmd.args(["--liftoff-only","--disable-warning=ExperimentalWarning","-e",DRIVER]).arg(entry).arg(&index.root).arg(action);
    cmd.current_dir(&index.root).env("CODEGRAPH_TELEMETRY","0").env("DO_NOT_TRACK","1")
        .env("CODEGRAPH_NO_UPDATE_CHECK","1").env("CODEGRAPH_NO_DAEMON","1")
        .env("NO_COLOR","1").env("CI","1")
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)] { use std::os::unix::process::CommandExt; cmd.process_group(0); }
    let mut child = cmd.spawn().map_err(|_| "knowledge_process_failed")?;
    let start = Instant::now(); let mut beat = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return if status.success() {Ok(())} else {Err("knowledge_process_failed".into())},
            Err(_) => { stop(&mut child); return Err("knowledge_process_failed".into()); },
            _ => (),
        }
        if start.elapsed() > Duration::from_secs(1800) { stop(&mut child); return Err("knowledge_timeout".into()); }
        if beat.elapsed() >= Duration::from_secs(1) {
            let current = get(app,&index.id);
            if !current.as_ref().is_ok_and(|i| i.enabled && job.is_none_or(|j| j == i.job_id)) {
                stop(&mut child); return Err("knowledge_disabled".into());
            }
            if let Some(job) = job {
                if let Ok(conn) = app.db().conn.lock() {
                    let _ = conn.execute("UPDATE knowledge_indexes SET updated_at=?1 WHERE id=?2 AND job_id=?3",params![now(),index.id,job]);
                }
            }
            beat = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
pub fn audit(app: &AppCtx, id: &str, level: &str, event: &str, duration: u128) {
    let configured = std::env::var("VLX_KNOWLEDGE_LOG_LEVEL").unwrap_or_default();
    if configured == "off" || (configured == "error" && level != "ERROR") { return; }
    let time = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let request_id = if event.starts_with("http_access") {id} else {"system"};
    let entity = if event.starts_with("sync_") {format!(" indexId={id}")}else{String::new()};
    let status = if level=="ERROR" {"failed"}else if event.ends_with("started") {"running"}else{"ok"};
    let line = format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} [{level:5}] [{request_id}] event={event}{entity} step=codegraph method=program inputCount=1 outputCount={} status={status} durationMs={duration}\n",
        time.year(),u8::from(time.month()),time.day(),time.hour(),time.minute(),time.second(),usize::from(status=="ok"));
    eprint!("{line}");
    let dir = std::env::var_os("VLX_KNOWLEDGE_LOG_DIR").map(PathBuf::from).or_else(|| app.data_dir().ok().map(|p| p.join("logs")));
    if let Some(dir) = dir {
        if std::fs::create_dir_all(&dir).is_ok() {
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("knowledge.log")) { let _ = file.write_all(line.as_bytes()); }
        }
    }
}
