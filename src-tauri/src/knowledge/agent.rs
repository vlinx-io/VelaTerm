//! Session-scoped, read-only knowledge queries over the existing authenticated loopback channel.
use super::*;
use std::io::Read;
use std::time::{Duration, Instant};

const USAGE: &str = "usage: vkb explore <query> | search <query> | node <symbol-id> | callers <id> [depth] | callees <id> [depth] | impact <id> [depth] | path <from-id> <to-id> | files [path] | memories <query> | memory <entry-id> | notes <query> | note <vault-id> <path> | status\nQueries the current checkout and this VelaTerm backend's knowledge base. Code indexes must be enabled in the project's Code Graph page. Results carry availability and review status; no knowledge entry is changed.";

fn is_ephemeral_session(id: &str) -> bool {
    id.strip_prefix("eph-").is_some_and(|rest| !rest.is_empty())
}

fn find_matching_index(items: &Value, cwd: &Path, current_checkout: &Option<PathBuf>) -> Option<Value> {
    items.as_array()?.iter()
        .filter(|i| i["root"].as_str().is_some_and(|r| cwd.starts_with(r) && git_root(Path::new(r)) == *current_checkout))
        .max_by_key(|i| i["root"].as_str().map(str::len).unwrap_or(0))
        .cloned()
}

/// Match `cwd` against the session's bound project first, then fall back to any imported live project
/// whose root or worktree contains `cwd`. Plain terminals (`eph-*`) and collection sessions have no
/// bound project root of their own, and interactive shells may `cd` into another imported repository.
fn resolve_target_index(app: &AppCtx, preferred_project_id: Option<&str>, cwd: &Path) -> Result<(Option<Value>, Value)> {
    let current_checkout = git_root(cwd);
    if let Some(pid) = preferred_project_id {
        if let Ok(targets) = list(app, pid) {
            if let Some(index) = find_matching_index(&targets["indexes"], cwd, &current_checkout) {
                return Ok((Some(index), targets["runtime"].clone()));
            }
        }
    }
    let tree = {
        let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
        crate::db::repo::list_tree(&conn)?
    };
    let may_contain = |raw: &str| -> bool {
        if raw.is_empty() { return false; }
        let Ok(dir) = Path::new(raw).canonicalize() else { return false; };
        cwd.starts_with(&dir) || current_checkout.as_ref().is_some_and(|cc| dir.starts_with(cc))
    };
    let mut best: Option<Value> = None;
    let mut runtime: Option<Value> = None;
    for project in &tree.projects {
        if preferred_project_id == Some(project.id.as_str()) { continue; }
        let candidate = may_contain(&project.root_path)
            || tree.groups.iter().filter(|g| g.project_id == project.id).any(|g| g.worktree_path.as_deref().is_some_and(&may_contain))
            || tree.sessions.iter().filter(|s| s.project_id == project.id).any(|s| {
                s.worktree_path.as_deref().is_some_and(&may_contain) || s.cwd.as_deref().is_some_and(&may_contain)
            });
        if !candidate { continue; }
        let targets = list(app, &project.id)?;
        if runtime.is_none() { runtime = Some(targets["runtime"].clone()); }
        if let Some(index) = find_matching_index(&targets["indexes"], cwd, &current_checkout) {
            let len = index["root"].as_str().map(str::len).unwrap_or(0);
            if best.as_ref().and_then(|b| b["root"].as_str()).map_or(true, |r| len > r.len()) {
                best = Some(index);
            }
        }
    }
    let runtime = match runtime {
        Some(r) => r,
        None => super::runtime::status(app)?,
    };
    Ok((best, runtime))
}

pub fn query(app: &AppCtx, args: &Value) -> Result<Value> {
    let sid = required(args, "sessionId")?;
    let session = {
        let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
        crate::db::repo::get_session(&conn, sid)?
    };
    // Ephemeral plain terminals (`eph-*`) live only in client memory and are intentionally absent from SQLite.
    if session.is_none() && !is_ephemeral_session(sid) {
        return Err("knowledge_not_found".into());
    }
    let action = required(args, "action")?;
    let text = args.get("query").and_then(Value::as_str).unwrap_or("");
    if text.len() > 4096 { return Err("knowledge_invalid".into()); }
    if action == "notes" { return crate::kb::dispatch(app, "kb_search", &json!({"query":text})); }
    if action == "note" {
        let (vault_id, path) = text.split_once(' ').filter(|(v, p)| !v.is_empty() && !p.trim().is_empty()).ok_or("knowledge_invalid")?;
        return crate::kb::dispatch(app, "kb_get", &json!({"vaultId":vault_id,"path":path.trim_start()}));
    }
    if action == "memory" {
        if text.trim().is_empty() { return Err("knowledge_invalid".into()); }
        let mut value = crate::memory::dispatch(app, "memory_get", &json!({"id":text.trim()}))?;
        value["codeReferences"] = links::for_entry(app, text.trim())?;
        return Ok(value);
    }
    if action == "memories" {
        return crate::memory::dispatch(app, "memory_list", &json!({"query":text,"tag":"","sort":"updated","page":0}));
    }
    if !["search","status","node","explore","files","impact","path","callers","callees"].contains(&action) {
        return Err("knowledge_invalid".into());
    }
    let raw_cwd = required(args, "cwd")?;
    let cwd = Path::new(raw_cwd).canonicalize().map_err(|_| "knowledge_missing_directory")?;
    if !cwd.is_dir() { return Err("knowledge_invalid_directory".into()); }
    let indexable_cwd = cwd.parent().is_some()
        && crate::host::home_dir().as_deref() != Some(cwd.as_path());
    let (index, runtime) = if indexable_cwd {
        resolve_target_index(app, session.as_ref().map(|s| s.project_id.as_str()), &cwd)?
    } else {
        (None, super::runtime::status(app)?)
    };
    if action == "status" {
        return match index {
            Some(index) => Ok(json!({"index":index,"runtime":runtime})),
            None => Ok(json!({"index":Value::Null,"code":{"available":false,"reason":"knowledge_index_missing"},"runtime":runtime,"cwd":cwd})),
        };
    }
    let memories = crate::memory::dispatch(app, "memory_list", &json!({"query":text,"tag":"","sort":"updated","page":0}))?;
    let notes = if action == "search" { crate::kb::dispatch(app, "kb_search", &json!({"query":text}))? } else { json!(null) };
    let Some(index) = index else {
        return Ok(json!({"code":{"available":false,"reason":"knowledge_index_missing"},"memories":memories,"notes":notes,"cwd":cwd}));
    };
    let id = index["id"].as_str().ok_or("knowledge_invalid")?;
    let result = if action == "node" { dispatch(app, "knowledge_node", &json!({"id":id,"nodeId":text})) }
        else if action == "search" { dispatch(app, "knowledge_query", &json!({"id":id,"action":"search","query":text})) }
        else {
            let mut request = json!({"id":id,"action":action,"query":text});
            if ["impact","callers","callees","path"].contains(&action) {
                let parts: Vec<_> = text.split_whitespace().collect();
                request["nodeId"] = json!(parts.first().copied().unwrap_or(""));
                if action == "path" { request["targetId"] = json!(parts.get(1).copied().unwrap_or("")); }
                else if let Some(depth) = parts.get(1) { request["depth"] = json!(depth.parse::<u64>().map_err(|_| "knowledge_invalid")?); }
            }
            dispatch(app, "knowledge_query", &request)
        };
    let code = match result { Ok(data) => json!({"available":true,"data":data}), Err(error) => json!({"available":false,"reason":error}) };
    Ok(json!({"code":code,"memories":memories,"notes":notes,"indexId":id,"root":cwd}))
}

fn diagnostic_code(error: &str) -> &'static str {
    match error {
        "knowledge_unauthorized" => "knowledge_unauthorized",
        "knowledge_not_found" => "knowledge_not_found",
        "knowledge_invalid" => "knowledge_invalid",
        "knowledge_missing_directory" => "knowledge_missing_directory",
        "knowledge_invalid_directory" => "knowledge_invalid_directory",
        "knowledge_directory_changed" => "knowledge_directory_changed",
        "knowledge_disabled" => "knowledge_disabled",
        "knowledge_runtime_missing" => "knowledge_runtime_missing",
        "knowledge_busy" => "knowledge_busy",
        "knowledge_timeout" => "knowledge_timeout",
        "knowledge_partial" => "knowledge_partial",
        "knowledge_interrupted" => "knowledge_interrupted",
        "knowledge_query_failed" | "knowledge_process_failed" => "knowledge_query_failed",
        "knowledge_result_large" => "knowledge_result_large",
        "knowledge_symbol_missing" | "knowledge_source_missing" => "knowledge_symbol_missing",
        "memory_not_found" => "memory_not_found",
        "kb_conflict" => "kb_conflict",
        "kb_exists" => "kb_exists",
        "kb_invalid_path" => "kb_invalid_path",
        "kb_invalid" => "kb_invalid",
        "kb_io" => "kb_io",
        "kb_too_large" => "kb_too_large",
        "kb_not_found" => "kb_not_found",
        "kb_unavailable" => "kb_unavailable",
        "kb_encoding" => "kb_encoding",
        _ => crate::diagnostics::error_code(error),
    }
}

pub(crate) fn format_cli_error(error: &str) -> String {
    match error {
        "knowledge_unauthorized" => "Session credentials were rejected by VelaTerm. Open a new VelaTerm session and retry.".into(),
        "knowledge_not_found" => "The calling session or requested knowledge item was not found in VelaTerm.".into(),
        "knowledge_invalid" => "Invalid vkb arguments. Run `vkb --help` for usage.".into(),
        "knowledge_missing_directory" => "The current working directory does not exist or cannot be accessed.".into(),
        "knowledge_invalid_directory" => "The current working directory cannot be indexed. Change into a project directory and retry.".into(),
        "knowledge_directory_changed" => "The project directory moved or changed. Check the project path in VelaTerm and retry.".into(),
        "knowledge_disabled" => "Code indexing is disabled for this directory. Enable it on the project's Code Graph page.".into(),
        "knowledge_runtime_missing" => "CodeGraph is not installed. Download it on the project's Code Graph page.".into(),
        "knowledge_busy" => "An index task is currently running for this directory. Wait for it to finish and retry.".into(),
        "knowledge_timeout" => "The knowledge query timed out.".into(),
        "knowledge_partial" | "knowledge_interrupted" => "The code index is incomplete. Synchronize it on the Code Graph page and retry.".into(),
        "knowledge_query_failed" | "knowledge_process_failed" => "The code graph query failed. Synchronize the index and retry.".into(),
        "knowledge_result_large" => "The query result is too large. Narrow the query or reduce the traversal depth.".into(),
        "knowledge_symbol_missing" | "knowledge_source_missing" => "The symbol or source file is no longer in the index. Synchronize and search again.".into(),
        "memory_not_found" => "The requested knowledge entry was not found.".into(),
        "kb_not_found" => "The requested note or local knowledge base was not found.".into(),
        "kb_invalid_path" | "kb_invalid" => "Invalid note path or local knowledge base query.".into(),
        "kb_too_large" => "The requested note is too large to read.".into(),
        "kb_unavailable" | "kb_io" | "kb_encoding" => "The requested note could not be read.".into(),
        code if code.starts_with("knowledge_") || code.starts_with("kb_") || code.starts_with("memory_") => {
            "The knowledge query failed. Check the Knowledge Base and Code Graph status in VelaTerm.".into()
        }
        other => other.to_string(),
    }
}

pub fn handle(app: AppCtx, mut request: crate::diagnostics::HttpRequest, token: String) {
    let started = Instant::now();
    let method = request.method().as_str().to_string();
    let request_id = request.id().to_string();
    let authorized = request.headers().iter().any(|h| h.field.equiv("Authorization") && h.value.as_str() == format!("Bearer {token}"));
    let mut body = String::new();
    let result = if !authorized { Err((401, "knowledge_unauthorized".to_string())) }
        else if request.method() != &tiny_http::Method::Post { Err((405, "knowledge_invalid".to_string())) }
        else if request.as_reader().take(16_385).read_to_string(&mut body).is_err() || body.len() > 16_384 { Err((413, "knowledge_invalid".to_string())) }
        else { serde_json::from_str(&body).map_err(|_| "knowledge_invalid".to_string()).and_then(|args| query(&app, &args)).map_err(|e| (400, e)) };
    let (status, error_code, value) = match result {
        Ok(value) => (200, None, json!({"result":value})),
        Err((status, error)) => (status, Some(diagnostic_code(&error)), json!({"error":error})),
    };
    let response = tiny_http::Response::from_string(value.to_string()).with_status_code(status)
        .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap());
    let _ = request.respond(response);
    let mut fields = json!({"requestId":request_id,"method":method,"statusCode":status,"durationMs":started.elapsed().as_millis() as u64});
    if let Some(code) = error_code { fields["errorCode"] = json!(code); }
    crate::diagnostics::record(if status == 200 { "INFO" } else { "WARN" }, "knowledge_http", fields);
}

pub(crate) fn run_with_env(
    args: &[String],
    base: Option<String>,
    sid: Option<String>,
    token: Option<String>,
    cwd: std::result::Result<PathBuf, String>,
) -> Result<String> {
    let rest = &args[args.len().min(2)..];
    if rest.is_empty() || rest[0] == "--help" || rest[0] == "-h" {
        return Ok(USAGE.to_string());
    }
    let base = base.filter(|s| !s.is_empty()).ok_or("Run vkb inside a VelaTerm session.")?;
    let sid = sid.filter(|s| !s.is_empty()).ok_or("Missing VelaTerm session.")?;
    let token = token.filter(|s| !s.is_empty()).ok_or("Missing VelaTerm session credentials.")?;
    let url = url::Url::parse(&base).map_err(|_| "Invalid VelaTerm endpoint.")?;
    if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "localhost")) {
        return Err("Invalid VelaTerm endpoint.".into());
    }
    let cwd = cwd?;
    let endpoint = format!("{}/knowledge", base.trim_end_matches('/'));
    let body = json!({"sessionId":sid,"cwd":cwd,"action":rest[0],"query":rest[1..].join(" ")});
    let request = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(1900))
        .redirects(0)
        .try_proxy_from_env(false)
        .build()
        .post(&endpoint)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .set("X-Request-Id", &uuid::Uuid::new_v4().to_string())
        .send_string(&body.to_string());
    let response = match request {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(_) => {
            return Err(format!(
                "Cannot reach VelaTerm at {base}. If this session runs in a sandbox without local network access, allow local network access or rerun outside the sandbox."
            ));
        }
    };
    let mut bytes = String::new();
    response.into_reader().take(8 * 1024 * 1024).read_to_string(&mut bytes).map_err(|_| "Cannot read knowledge response.")?;
    decode_response(&bytes)
}

pub(crate) fn decode_response(bytes: &str) -> Result<String> {
    let value: Value = serde_json::from_str(bytes).map_err(|_| "Invalid knowledge response.")?;
    if let Some(error) = value["error"].as_str() {
        return Err(format_cli_error(error));
    }
    serde_json::to_string_pretty(&value["result"]).map_err(|_| "Invalid knowledge response.".into())
}

pub fn run(args: &[String]) -> ! {
    let result = run_with_env(
        args,
        std::env::var("VLX_SPAWN_URL").ok(),
        std::env::var("VLX_SESSION_ID").ok(),
        crate::agent::session_env::token(),
        std::env::current_dir().map_err(|_| "Cannot resolve current directory.".to_string()),
    );
    match result {
        Ok(output) => {
            println!("{output}");
            std::process::exit(0);
        }
        Err(error) => {
            // Print directly to stderr instead of diagnostic_warn!, which intentionally drops all format arguments.
            eprintln!("vkb: {error}");
            std::process::exit(1);
        }
    }
}
