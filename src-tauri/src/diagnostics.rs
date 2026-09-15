//! Bounded local diagnostics. Only approved metadata reaches either output sink.
use serde_json::{json, Map, Value};
use std::{
    cell::RefCell,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        OnceLock,
    },
    time::{Duration, Instant},
};

const CAPACITY: usize = 2048;
const FILE_LIMIT: u64 = 10 * 1024 * 1024;
const DIRECTORY_LIMIT: u64 = 100 * 1024 * 1024;
static LOGGER: OnceLock<Logger> = OnceLock::new();
static DROPPED: AtomicU64 = AtomicU64::new(0);
static FAILURES: AtomicU64 = AtomicU64::new(0);
thread_local! { static CONTEXT: RefCell<String> = const { RefCell::new(String::new()) }; static OPERATION: RefCell<String> = const { RefCell::new(String::new()) }; }
struct Logger {
    tx: SyncSender<Message>,
    level: u8,
    routes: std::collections::HashMap<&'static str, usize>,
}
enum Message {
    Line(usize, String),
    Flush(mpsc::Sender<()>),
}

pub fn timestamp() -> String {
    let t = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        t.year(),
        u8::from(t.month()),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

pub fn enabled(configured: &str, level: &str) -> bool {
    rank(level) >= rank(configured)
}
fn rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "off" => 255,
        "error" => 4,
        "warn" => 3,
        "debug" => 1,
        "trace" => 0,
        _ => 2,
    }
}

/// Initialize once, before the database and services. No user content is captured from stdout/stderr.
pub fn init(data_dir: &Path) {
    LOGGER.get_or_init(|| {
        let dir = std::env::var_os("VLX_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| data_dir.join("logs"));
        let mut writers = vec![FileWriter::new(dir)];
        let mut routes = std::collections::HashMap::new();
        if std::env::var_os("VLX_LOG_DIR").is_none() {
            for (event, key) in [
                ("split", "VLX_SPLIT_LOG_DIR"),
                ("memory", "VLX_MEMORY_LOG_DIR"),
                ("knowledge", "VLX_KNOWLEDGE_LOG_DIR"),
                ("security", "VLX_SECURITY_LOG_DIR"),
            ] {
                if let Some(dir) = std::env::var_os(key) {
                    routes.insert(event, writers.len());
                    writers.push(FileWriter::new(PathBuf::from(dir)));
                }
            }
        }
        let level = rank(&std::env::var("VLX_LOG_LEVEL").unwrap_or_else(|_| "info".into()));
        let (tx, rx) = mpsc::sync_channel::<Message>(CAPACITY);
        let spawn = std::thread::Builder::new()
            .name("diagnostic-writer".into())
            .spawn(move || {
                let mut last_notice = Instant::now() - Duration::from_secs(60);
                while let Ok(message) = rx.recv() {
                    match message {
                        Message::Line(route, line) => {
                            // Both sinks receive the exact same already-filtered line.
                            let _ = writeln!(io::stderr().lock(), "{line}");
                            if writers[route].append(&line, FILE_LIMIT).is_err() {
                                FAILURES.fetch_add(1, Ordering::Relaxed);
                                if last_notice.elapsed() >= Duration::from_secs(60) {
                                    let _ = writeln!(
                                        io::stderr().lock(),
                                        "{} [WARN ] [system] event=diagnostic_write status=failed",
                                        timestamp()
                                    );
                                    last_notice = Instant::now();
                                }
                            }
                        }
                        Message::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            });
        if spawn.is_err() {
            FAILURES.fetch_add(1, Ordering::Relaxed);
        }
        Logger { tx, level, routes }
    });
    static PANIC_HOOK: OnceLock<()> = OnceLock::new();
    PANIC_HOOK.get_or_init(|| {
        // A panic payload may contain a user document or credential-bearing error.
        std::panic::set_hook(Box::new(|info| {
            record(
                "ERROR",
                "runtime_panic",
                json!({"line":info.location().map(|p|p.line())}),
            );
            flush();
        }));
    });
    record(
        "INFO",
        "runtime_started",
        json!({"pid":std::process::id(),"version":crate::VERSION,"platform":std::env::consts::OS}),
    );
}

pub fn health() -> Value {
    json!({"droppedCount":DROPPED.load(Ordering::Relaxed),"writeFailures":FAILURES.load(Ordering::Relaxed),"initialized":LOGGER.get().is_some()})
}

/// Authenticated clients submit metadata only; this endpoint cannot read logs or change configuration.
pub fn client_event(source: &str, args: &Value) -> Result<Value, String> {
    use std::{collections::HashMap, sync::Mutex};
    static RATES: OnceLock<Mutex<HashMap<String, (Instant, u32)>>> = OnceLock::new();
    let event = match args.get("event").and_then(Value::as_str) {
        Some("request") => "client_request",
        Some("shell_switch") => "client_shell_switch",
        Some("restart") => "client_restart",
        Some("pty_output") => "client_pty_output",
        Some("ws_state") => "client_ws_state",
        Some("pty_spawn") => "client_pty_spawn",
        _ => return Ok(json!(false)),
    };
    let Ok(mut rates) = RATES.get_or_init(|| Mutex::new(HashMap::new())).lock() else {
        return Ok(json!(false));
    };
    rates.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(60));
    if rates.len() >= 512 && !rates.contains_key(source) {
        return Ok(json!(false));
    }
    let (_, count) = rates
        .entry(source.to_string())
        .or_insert((Instant::now(), 0));
    if *count >= 120 {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return Ok(json!(false));
    }
    *count += 1;
    drop(rates);
    let _context = Context::enter(args.get("requestId").and_then(Value::as_str));
    let permitted: Map<String, Value> = args
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| {
            matches!(
                k.as_str(),
                "requestId"
                    | "operationId"
                    | "sessionId"
                    | "status"
                    | "step"
                    | "errorCode"
                    | "command"
                    | "clientDurationMs"
                    | "bytes"
                    | "attached"
                    | "retryCount"
                    | "generation"
            )
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut fields = safe_fields(&Value::Object(permitted));
    fields["source"] = json!("frontend");
    record(
        if args.get("status").and_then(Value::as_str) == Some("failed") {
            "WARN"
        } else {
            "INFO"
        },
        event,
        fields,
    );
    Ok(json!(true))
}

/// Bounded exit barrier; an unavailable disk or writer cannot prevent application shutdown.
pub fn flush() {
    if let Some(logger) = LOGGER.get() {
        let (tx, rx) = mpsc::channel();
        if logger.tx.try_send(Message::Flush(tx)).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(2));
        }
    }
}

fn uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok() && value.len() == 36
}
fn identity(value: &str) -> bool {
    uuid(value) || value.strip_prefix("eph-").is_some_and(uuid)
}

/// Unknown strings and fields are omitted, including exception messages and arbitrary nested objects.
pub fn safe_fields(data: &Value) -> Value {
    let mut out = Map::new();
    let Some(fields) = data.as_object() else {
        return Value::Object(out);
    };
    for (key, value) in fields.iter().take(64) {
        let numeric = matches!(
            key.as_str(),
            "durationMs"
                | "queueMs"
                | "lockWaitMs"
                | "pid"
                | "clientAtMs"
                | "cols"
                | "rows"
                | "bytes"
                | "inputCount"
                | "outputCount"
                | "responseBytes"
                | "stderrBytes"
                | "originalChars"
                | "sentChars"
                | "limit"
                | "imageCount"
                | "revision"
                | "retryCount"
                | "generation"
                | "exitCode"
                | "statusCode"
                | "droppedCount"
                | "writeFailures"
                | "prepareMs"
                | "encodeMs"
                | "elapsedMs"
                | "clientDurationMs"
                | "count"
                | "line"
        );
        if numeric && (value.is_u64() || value.is_i64() || value.is_null()) {
            out.insert(key.clone(), value.clone());
            continue;
        }
        if matches!(
            key.as_str(),
            "attached" | "truncated" | "initialized" | "success"
        ) && value.is_boolean()
        {
            out.insert(key.clone(), value.clone());
            continue;
        }
        if matches!(key.as_str(), "usage" | "counters") {
            if let Some(map) = value.as_object() {
                let safe: Map<String, Value> = map
                    .iter()
                    .filter(|(k, v)| {
                        matches!(
                            k.as_str(),
                            "input_tokens"
                                | "output_tokens"
                                | "cached_input_tokens"
                                | "cache_creation_input_tokens"
                                | "cache_read_input_tokens"
                                | "total_tokens"
                        ) && v.is_u64()
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                out.insert(key.clone(), Value::Object(safe));
            }
            continue;
        }
        if key == "sessionIds" {
            if let Some(ids) = value.as_array() {
                out.insert(
                    key.clone(),
                    Value::Array(
                        ids.iter()
                            .take(200)
                            .filter(|v| v.as_str().is_some_and(identity))
                            .cloned()
                            .collect(),
                    ),
                );
            }
            continue;
        }
        let Some(s) = value.as_str() else {
            continue;
        };
        if key == "model" {
            if s == "configured_default" {
                out.insert(key.clone(), json!(s));
            } else {
                out.insert(key.clone(), json!("redacted"));
                use std::{collections::HashMap, sync::Mutex};
                static MODELS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
                if s.len() <= 200 {
                    if let Ok(mut models) = MODELS.get_or_init(|| Mutex::new(HashMap::new())).lock()
                    {
                        if models.len() < 128 || models.contains_key(s) {
                            let id = models
                                .entry(s.to_owned())
                                .or_insert_with(|| uuid::Uuid::new_v4().to_string());
                            out.insert("modelRef".into(), json!(id));
                        }
                    }
                }
            }
            continue;
        }
        let allowed = match key.as_str() {
            "requestId" | "operationId" | "sessionId" | "jobId" | "entryId" | "parentSessionId" | "vaultId"
            | "tabId" => identity(s),
            "sha256" | "stderrSha256" => s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()),
            "status" => matches!(
                s,
                "started"
                    | "running"
                    | "success"
                    | "completed"
                    | "partial"
                    | "failed"
                    | "cancelled"
                    | "interrupted"
                    | "ok"
                    | "invalid"
                    | "validated"
                    | "received"
                    | "slow"
                    | "unknown"
            ),
            "shell" => matches!(
                s,
                "posix" | "powershell" | "cmd" | "fish" | "wsl" | "unknown"
            ),
            "method" => matches!(
                s,
                "program"
                    | "AI"
                    | "git_cli"
                    | "snapshot"
                    | "upstream_workbench"
                    | "GET"
                    | "POST"
                    | "PUT"
                    | "PATCH"
                    | "DELETE"
                    | "HEAD"
                    | "OPTIONS"
            ),
            "interface" => matches!(
                s,
                "claude CLI" | "codex CLI" | "claude local CLI" | "codex local CLI"
            ),
            "goal" => matches!(s, "compile_thematic_memory" | "codex_security_scan"),
            "inputType" => matches!(s, "text" | "image" | "mixed"),
            "schema" => matches!(s, "memory-wiki-v1" | "codex-security/1.0"),
            "preview" => matches!(
                s,
                "[source and memory content redacted]"
                    | "[candidate content redacted]"
                    | "[audit context redacted]"
            ),
            "entityType" => matches!(s, "memory_entry" | "security_finding"),
            "effort" => matches!(
                s,
                "configured_default" | "low" | "medium" | "high" | "xhigh" | "max"
            ),
            "clientId" => {
                matches!(s, "main" | "desktop")
                    || s.strip_prefix("ws-").is_some_and(|v| {
                        !v.is_empty() && v.len() <= 12 && v.bytes().all(|b| b.is_ascii_digit())
                    })
            }
            "platform" => matches!(s, "windows" | "macos" | "linux" | "android" | "ios"),
            "version" => {
                s.len() <= 32
                    && s.split('.').count() == 3
                    && s.split('.').all(|part| part.parse::<u32>().is_ok())
            }
            "direction" => matches!(s, "horizontal" | "vertical"),
            "source" => matches!(
                s,
                "shortcut"
                    | "menu"
                    | "pane-button"
                    | "sidebar"
                    | "drop"
                    | "tile"
                    | "mirror"
                    | "unknown"
                    | "desktop"
                    | "remote"
                    | "frontend"
                    | "backend"
                    | "cache"
                    | "bundled"
                    | "configured"
                    | "system"
            ),
            "errorCode" => matches!(
                s,
                "timeout"
                    | "permission_denied"
                    | "not_found"
                    | "cancelled"
                    | "unavailable"
                    | "operation_failed"
                    | "invalid_launch_identifier"
                    | "kb_conflict" | "kb_exists" | "kb_invalid_path" | "kb_invalid" | "kb_io"
                    | "kb_too_large" | "kb_not_found" | "kb_unavailable" | "kb_encoding"
                    | "kb_empty_import" | "kb_incomplete_import" | "kb_move_incomplete"
            ),
            "step" => matches!(
                s,
                "discover"
                    | "import"
                    | "connect"
                    | "probe"
                    | "supply"
                    | "upload"
                    | "verify"
                    | "start"
                    | "forward"
                    | "kill"
                    | "lock"
                    | "openpty"
                    | "shell"
                    | "spawn"
                    | "attach"
                    | "first_output"
                    | "read"
                    | "write"
                    | "slot"
                    | "prepare"
                    | "dispatch"
                    | "encode"
                    | "normalize"
                    | "extract"
                    | "save"
                    | "finish"
                    | "ai_request"
                    | "ai_received"
                    | "ai_failed"
                    | "ai_response"
                    | "job_completed"
                    | "job_failed"
                    | "job_cancelled"
                    | "scan_phase"
                    | "mount"
                    | "unmount"
                    | "request"
                    | "render"
                    | "switch"
                    | "restart"
                    | "fallback"
                    | "exit"
                    | "handshake"
            ),
            "command" => COMMANDS.contains(&s),
            "path" => matches!(
                s,
                "/knowledge"
                    | "/spawn"
                    | "/view"
                    | "/refer"
                    | "/search"
                    | "/orch"
                    | "/stat"
                    | "/hook/:id"
                    | "/api/login"
                    | "/api/me"
                    | "/api/mode"
                    | "/api/logout"
                    | "/api/download"
                    | "/ws"
                    | "static"
                    | "unknown"
            ),
            _ => false,
        };
        if allowed {
            out.insert(key.clone(), value.clone());
        }
    }
    Value::Object(out)
}

pub fn error_code(error: &str) -> &'static str {
    let e = error
        .chars()
        .take(4096)
        .collect::<String>()
        .to_ascii_lowercase();
    if e.contains("model and effort must be identifiers") {
        "invalid_launch_identifier"
    } else if e.contains("timeout") || e.contains("timed out") {
        "timeout"
    } else if e.contains("permission") || e.contains("denied") || e.contains("unauthorized") {
        "permission_denied"
    } else if e.contains("not found") || e.contains("no such file") {
        "not_found"
    } else if e.contains("cancel") {
        "cancelled"
    } else {
        "operation_failed"
    }
}

fn render(level: &str, event: &'static str, mut data: Value) -> String {
    let operation = OPERATION.with(|c| c.borrow().clone());
    if !operation.is_empty() && data.get("operationId").is_none_or(Value::is_null) {
        data["operationId"] = json!(operation);
    }
    let request = CONTEXT.with(|c| c.borrow().clone());
    format!(
        "{} [{:<5}] [{}] event={} {}",
        timestamp(),
        level,
        if request.is_empty() {
            "system"
        } else {
            &request
        },
        event,
        safe_fields(&data)
    )
}

pub fn record(level: &str, event: &'static str, data: Value) -> bool {
    let Some(logger) = LOGGER.get() else {
        return false;
    };
    if rank(level) < logger.level {
        return false;
    }
    if logger
        .tx
        .try_send(Message::Line(
            *logger.routes.get(event).unwrap_or(&0),
            render(level, event, data),
        ))
        .is_err()
    {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    true
}
pub fn level_enabled(level: &str) -> bool {
    LOGGER
        .get()
        .is_some_and(|logger| rank(level) >= logger.level)
}

/// Preserve standard Mutex semantics while exposing meaningful database contention.
pub struct DatabaseMutex<T>(std::sync::Mutex<T>);
impl<T> DatabaseMutex<T> {
    pub fn new(value: T) -> Self {
        Self(std::sync::Mutex::new(value))
    }
    pub fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, T>> {
        let started = Instant::now();
        let result = self.0.lock();
        if started.elapsed() >= Duration::from_millis(100) || result.is_err() {
            record(
                "WARN",
                "database_lock",
                json!({"lockWaitMs":started.elapsed().as_millis() as u64,"status":if result.is_ok(){"slow"}else{"failed"}}),
            );
        }
        result
    }
}

/// Preserve a compiled diagnostic template without formatting its potentially sensitive arguments.
pub fn notice(level: &str, template: &'static str, site: &'static str, line: u32) {
    let template: String = template
        .chars()
        .filter(|c| !c.is_control())
        .take(600)
        .collect();
    let output = format!(
        "{} site={} template={}",
        render(level, "runtime_notice", json!({"line":line})),
        site,
        serde_json::to_string(&template).unwrap_or_default()
    );
    let Some(logger) = LOGGER.get() else {
        let _ = writeln!(io::stderr().lock(), "{output}");
        return;
    };
    if rank(level) < logger.level {
        return;
    }
    if logger.tx.try_send(Message::Line(0, output)).is_err() {
        DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

#[macro_export]
macro_rules! diagnostic_warn {
    ($template:literal $(, $args:expr)* $(,)?) => {{
        let _ = format_args!($template $(, $args)*);
        $crate::diagnostics::notice("WARN", $template, file!(), line!());
    }};
}

/// Thread-local context is deliberately installed only inside synchronous worker execution.
pub fn current_context() -> (String, String) {
    (
        CONTEXT.with(|c| c.borrow().clone()),
        OPERATION.with(|c| c.borrow().clone()),
    )
}
pub struct Operation(String);
impl Operation {
    pub fn enter(id: Option<&str>) -> Self {
        let id = id.filter(|s| uuid(s)).unwrap_or_default().to_owned();
        Self(OPERATION.with(|c| c.replace(id)))
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        OPERATION.with(|c| {
            c.replace(self.0.clone());
        });
    }
}

pub struct Context(String);
impl Context {
    pub fn enter(id: Option<&str>) -> Self {
        let id = id
            .filter(|s| uuid(s))
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        Self(CONTEXT.with(|c| c.replace(id)))
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        CONTEXT.with(|c| {
            c.replace(self.0.clone());
        });
    }
}

pub struct Span {
    event: &'static str,
    fields: Value,
    start: Instant,
    phase: Instant,
    status: &'static str,
    step: Option<&'static str>,
}
impl Span {
    pub fn new(event: &'static str, fields: Value) -> Self {
        let mut s = Self {
            event,
            fields,
            start: Instant::now(),
            phase: Instant::now(),
            status: "failed",
            step: None,
        };
        s.emit("INFO", "started", None);
        s
    }
    fn emit(&mut self, level: &str, status: &str, step: Option<&str>) {
        let mut fields = self.fields.clone();
        fields["status"] = json!(status);
        fields["durationMs"] = json!(self.phase.elapsed().as_millis() as u64);
        fields["elapsedMs"] = json!(self.start.elapsed().as_millis() as u64);
        if let Some(step) = step {
            fields["step"] = json!(step);
        }
        record(level, self.event, fields);
    }
    pub fn step(&mut self, step: &'static str) {
        if let Some(previous) = self.step {
            self.emit("INFO", "completed", Some(previous));
        }
        self.phase = Instant::now();
        self.step = Some(step);
        self.emit("INFO", "started", Some(step));
    }
    pub fn finish<T, E>(&mut self, result: &Result<T, E>) {
        self.status = if result.is_ok() { "success" } else { "failed" };
    }
    pub fn success(&mut self) {
        self.status = "success";
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        self.phase = self.start;
        self.emit(
            if self.status == "success" {
                "INFO"
            } else {
                "WARN"
            },
            self.status,
            self.step,
        );
    }
}

struct FileWriter {
    dir: PathBuf,
    stem: String,
    last_cleanup: Option<Instant>,
}
impl FileWriter {
    fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            stem: format!("runtime-{}-{}", std::process::id(), uuid::Uuid::new_v4()),
            last_cleanup: None,
        }
    }
    fn append(&mut self, line: &str, limit: u64) -> io::Result<()> {
        if fs::symlink_metadata(&self.dir).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(io::Error::other("invalid log directory"));
        }
        fs::create_dir_all(&self.dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700))?;
        }
        let path = self.dir.join(format!("{}.log", self.stem));
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(io::Error::other("invalid log target"));
        }
        if fs::metadata(&path).is_ok_and(|m| m.len() + line.len() as u64 + 1 > limit) {
            let _ = fs::remove_file(self.dir.join(format!("{}.4.log", self.stem)));
            for n in (1..4).rev() {
                let from = self.dir.join(format!("{}.{n}.log", self.stem));
                if from.exists() {
                    fs::rename(from, self.dir.join(format!("{}.{}.log", self.stem, n + 1)))?;
                }
            }
            fs::rename(&path, self.dir.join(format!("{}.1.log", self.stem)))?;
        }
        let mut opts = OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = opts.open(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        writeln!(file, "{line}")?;
        if self
            .last_cleanup
            .is_none_or(|t| t.elapsed() > Duration::from_secs(60))
        {
            self.cleanup();
            self.last_cleanup = Some(Instant::now());
        }
        Ok(())
    }
    fn cleanup(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<_> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_owned();
                let rest = name.strip_prefix("runtime-")?;
                let (pid, suffix) = rest.split_once('-')?;
                if pid.parse::<u32>().is_err() {
                    return None;
                }
                if !suffix.is_ascii()
                    || suffix.len() < 40
                    || !uuid(&suffix[..36])
                    || !matches!(
                        &suffix[36..],
                        ".log" | ".1.log" | ".2.log" | ".3.log" | ".4.log"
                    )
                {
                    return None;
                }
                let m = fs::symlink_metadata(e.path()).ok()?;
                if !m.is_file() {
                    return None;
                }
                Some((m.modified().ok()?, m.len(), e.path()))
            })
            .collect();
        files.sort_by_key(|f| f.0);
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        for (time, len, path) in files {
            if (time
                .elapsed()
                .is_ok_and(|age| age > Duration::from_secs(7 * 86400))
                || total > DIRECTORY_LIMIT)
                && fs::remove_file(path).is_ok()
            {
                total = total.saturating_sub(len);
            }
        }
    }
}

include!("diagnostic_commands.rs");

/// Audit the native hook/CLI HTTP service without inspecting its credential-bearing URL or body.
pub struct HttpRequest {
    inner: Option<tiny_http::Request>,
    id: String,
    method: String,
    path: &'static str,
    started: Instant,
}
impl HttpRequest {
    pub fn new(request: tiny_http::Request) -> Self {
        let id = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("X-Request-Id"))
            .map(|h| h.value.as_str())
            .filter(|v| uuid(v))
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let method = request.method().as_str().to_owned();
        let path = match request.url().split('?').next().unwrap_or("") {
            "/knowledge" => "/knowledge",
            "/tell" => "/tell",
            "/plan-execute" => "/plan-execute",
            "/spawn" => "/spawn",
            "/view" => "/view",
            "/refer" => "/refer",
            "/search" => "/search",
            "/orch" => "/orch",
            "/stat" => "/stat",
            p if p.starts_with("/hook/") => "/hook/:id",
            _ => "unknown",
        };
        record(
            "INFO",
            "native_http",
            json!({"requestId":id,"method":method,"path":path,"status":"started"}),
        );
        Self {
            inner: Some(request),
            id,
            method,
            path,
            started: Instant::now(),
        }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn respond<R: io::Read>(mut self, response: tiny_http::Response<R>) -> io::Result<()> {
        let status = response.status_code().0;
        let response = response.with_header(
            tiny_http::Header::from_bytes("X-Request-Id", self.id.as_str())
                .expect("validated request ID"),
        );
        let result = self
            .inner
            .take()
            .expect("request not yet answered")
            .respond(response);
        record(
            if result.is_ok() { "INFO" } else { "WARN" },
            "native_http",
            json!({"requestId":self.id,"method":self.method,"path":self.path,"statusCode":status,"success":result.is_ok(),"durationMs":self.started.elapsed().as_millis() as u64}),
        );
        result
    }
}
impl std::ops::Deref for HttpRequest {
    type Target = tiny_http::Request;
    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("request not yet answered")
    }
}
impl std::ops::DerefMut for HttpRequest {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("request not yet answered")
    }
}
impl Drop for HttpRequest {
    fn drop(&mut self) {
        if let Some(request) = self.inner.take() {
            let response = tiny_http::Response::empty(500).with_header(
                tiny_http::Header::from_bytes("X-Request-Id", self.id.as_str())
                    .expect("validated request ID"),
            );
            let _ = request.respond(response);
            record(
                "WARN",
                "native_http",
                json!({"requestId":self.id,"method":self.method,"path":self.path,"statusCode":500,"status":"failed","durationMs":self.started.elapsed().as_millis() as u64}),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asynchronous_delivery_filters_both_sinks() {
        if std::env::var_os("VLX_DIAGNOSTIC_TEST_CHILD").is_some() {
            let dir = PathBuf::from(std::env::var_os("VLX_LOG_DIR").unwrap());
            init(&dir);
            let secret = "synthetic-private-value@example.test";
            record(
                "INFO",
                "delivery_test",
                json!({"durationMs":17,"message":secret}),
            );
            crate::diagnostic_warn!("delivery test failure: {}", secret);
            assert!(
                std::panic::catch_unwind(|| panic!("synthetic-private-value@example.test"))
                    .is_err()
            );
            flush();
            assert_eq!(health()["writeFailures"], 0);
            return;
        }
        let dir = std::env::temp_dir().join(format!("vlx-log-test-{}", uuid::Uuid::new_v4()));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "diagnostics::tests::asynchronous_delivery_filters_both_sinks",
                "--nocapture",
            ])
            .env("VLX_DIAGNOSTIC_TEST_CHILD", "1")
            .env("VLX_LOG_DIR", &dir)
            .env("VLX_LOG_LEVEL", "INFO")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated diagnostic writer test failed"
        );
        let console = String::from_utf8_lossy(&output.stderr);
        let log = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| fs::read_to_string(e.path()).unwrap())
            .collect::<String>();
        assert!(console.contains("delivery_test"));
        assert!(log.contains("delivery_test"));
        assert!(log.contains("delivery test failure: {}"));
        assert!(log.contains("runtime_panic"));
        for text in [&*console, log.as_str()] {
            assert!(!text.contains("synthetic-private-value"));
            assert!(!text.contains("example.test"));
        }
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_content_and_untrusted_metadata() {
        let safe = safe_fields(
            &json!({"message":"Bearer synthetic-secret","path":"/private/user/file","command":"secret_command","status":"password","sessionId":"user@example.test","usage":{"input_tokens":7,"raw":"secret"},"durationMs":42,"sha256":"not-a-digest"}),
        );
        assert_eq!(safe, json!({"usage":{"input_tokens":7},"durationMs":42}));
        assert!(
            safe_fields(&json!({"status":"ok\nERROR","requestId":"a".repeat(500)}))
                .as_object()
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn levels_are_consistent() {
        assert!(!enabled("OFF", "ERROR"));
        assert!(!enabled("warn", "INFO"));
        assert!(enabled("Warn", "ERROR"));
        assert!(enabled("debug", "INFO"));
    }
    #[test]
    fn rotation_is_bounded_and_private() {
        let dir = std::env::temp_dir().join(format!("vlx-log-test-{}", uuid::Uuid::new_v4()));
        let mut writer = FileWriter::new(dir.clone());
        for _ in 0..20 {
            writer.append("safe metadata", 20).unwrap();
        }
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 5);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for e in fs::read_dir(&dir).unwrap() {
                assert_eq!(
                    e.unwrap().metadata().unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn context_is_validated_and_restored() {
        let id = uuid::Uuid::new_v4().to_string();
        {
            let _c = Context::enter(Some(&id));
            assert!(render("INFO", "test", json!({})).contains(&id));
        }
        assert!(render("INFO", "test", json!({})).contains("[system]"));
        let _c = Context::enter(Some("secret\nforged"));
        assert!(!render("INFO", "test", json!({})).contains("secret"));
    }
    #[test]
    fn workflow_launch_validation_keeps_safe_diagnostic_context() {
        let code = error_code("Executor: Model and effort must be identifiers without spaces or shell operators");
        assert_eq!(code, "invalid_launch_identifier");
        for command in ["plan_execute_defaults", "plan_execute_start", "plan_execute_prepare", "plan_execute_create"] {
            let line = render("WARN", "rpc_failure", json!({
                "command": command, "errorCode": code, "prompt": "synthetic-secret", "images": ["private-image"]
            }));
            assert!(line.contains(command));
            assert!(line.contains(code));
            assert!(!line.contains("synthetic-secret"));
            assert!(!line.contains("private-image"));
        }
    }
    #[test]
    fn filtered_metadata_survives_disk_roundtrip() {
        let dir = std::env::temp_dir().join(format!("vlx-log-test-{}", uuid::Uuid::new_v4()));
        let line = render(
            "WARN",
            "rpc_failure",
            json!({"errorCode":"timeout","message":"synthetic-secret","url":"https://example.test/#pair=synthetic-secret","durationMs":1200}),
        );
        let mut writer = FileWriter::new(dir.clone());
        writer.append(&line, FILE_LIMIT).unwrap();
        let written = fs::read_to_string(dir.join(format!("{}.log", writer.stem))).unwrap();
        assert!(written.contains("timeout"));
        assert!(!written.contains("synthetic-secret"));
        assert!(!written.contains("example.test"));
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlink_targets_without_touching_the_destination() {
        let dir = std::env::temp_dir().join(format!("vlx-log-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("private");
        fs::write(&target, "unchanged").unwrap();
        let mut writer = FileWriter::new(dir.clone());
        std::os::unix::fs::symlink(&target, dir.join(format!("{}.log", writer.stem))).unwrap();
        assert!(writer.append("safe metadata", FILE_LIMIT).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "unchanged");
        fs::remove_dir_all(dir).unwrap();
    }
}
