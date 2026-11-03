//! Native CLI execution of the bundled upstream workflow and local workbench.
use super::{get, save, scope, upstream, workflow, Result, Run};
use crate::{agent::chat::engine::{ChatRow, ChatSnapshot}, host::AppCtx};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub fn register(plugin: &Path, state: &Path, run: &mut Run) -> Result<()> {
    let mut args = vec![
        "start-prompt-only-scan".into(),
        "--thread-id".into(),
        run.session_id.clone(),
        "--target-path".into(),
        run.root.clone(),
        "--scope".into(),
        if run.scope == "path" {
            run.path.clone()
        } else {
            ".".into()
        },
        "--mode".into(),
        if run.scope == "working-tree" {
            "diff"
        } else {
            "standard"
        }
        .into(),
        "--scan-root".into(),
        state
            .parent()
            .ok_or("security_upstream_invalid")?
            .join("scans")
            .to_string_lossy()
            .into_owned(),
    ];
    if run.scope == "working-tree" {
        args.extend(["--diff-target-kind".into(), "working_tree".into()]);
    }
    if let Some(model) = &run.model {
        args.extend(["--model".into(), model.clone()]);
    }
    if let Some(effort) = &run.effort {
        args.extend(["--reasoning-effort".into(), effort.clone()]);
    }
    let context = upstream::workbench(plugin, state, &args)?;
    let scan = &context["scan"];
    if scan["scanId"].as_str().is_none() || scan["scanDir"].as_str().is_none() {
        return Err("security_upstream_invalid".into());
    }
    run.upstream = Some(
        json!({"package":"@openai/codex-security","packageVersion":upstream::PACKAGE_VERSION,
        "pluginVersion":upstream::PLUGIN_VERSION,"adapter":upstream::ADAPTER_VERSION,
        "pluginDir":plugin,"stateDir":state,"scan":scan}),
    );
    Ok(())
}

fn binding(run: &Run) -> Result<(PathBuf, PathBuf, String)> {
    let value = run.upstream.as_ref().ok_or("security_upstream_missing")?;
    Ok((
        PathBuf::from(
            value["pluginDir"]
                .as_str()
                .ok_or("security_upstream_invalid")?,
        ),
        PathBuf::from(
            value["stateDir"]
                .as_str()
                .ok_or("security_upstream_invalid")?,
        ),
        value["scan"]["scanId"]
            .as_str()
            .ok_or("security_upstream_invalid")?
            .to_owned(),
    ))
}

pub fn cancel(run: &Run) -> Result<()> {
    if run.upstream.is_none() {
        return Ok(());
    }
    let (plugin, state, id) = binding(run)?;
    upstream::workbench(
        &plugin,
        &state,
        &["cancel-scan".into(), "--scan-id".into(), id],
    )?;
    Ok(())
}

fn sync(app: &AppCtx, run: &mut Run) -> Result<String> {
    let (plugin, state, id) = binding(run)?;
    let context = upstream::workbench(
        &plugin,
        &state,
        &["get-scan".into(), "--scan-id".into(), id],
    )?;
    let scan = context.get("scan").ok_or("security_upstream_invalid")?;
    let phase = scan["progress"]["phase"].as_str().unwrap_or("preflight");
    let changed = run
        .upstream
        .as_ref()
        .is_none_or(|value| value["scan"] != *scan);
    let phase_changed = run.phase != phase || run.steps.is_empty();
    if phase_changed {
        let at = super::now();
        if let Some(previous) = run.steps.last_mut() {
            if let Some(started) = previous["startedAt"].as_i64() {
                previous["durationMs"] = (at - started).max(0).into();
            }
        }
        run.steps
            .push(json!({"id":uuid::Uuid::new_v4().to_string(),"phase":phase,
            "summary":phase,"startedAt":at,"durationMs":0,"findingCount":scan["findingCount"]}));
        run.phase = phase.into();
        workflow::log(
            app,
            run,
            "scan_phase",
            &json!({"step":phase,"method":"upstream_workbench",
            "status":"started","inputCount":scan["progress"]["phaseProgress"]["total"],
            "outputCount":scan["progress"]["phaseProgress"]["completed"],"durationMs":0}),
        );
    }
    run.upstream.as_mut().ok_or("security_upstream_missing")?["scan"] = scan.clone();
    if changed || phase_changed {
        save(app, run)?;
    }
    let status = scan["progress"]["status"].as_str().unwrap_or("running");
    Ok(if status == "complete" {
        "completed"
    } else {
        status
    }
    .to_owned())
}

fn read_artifact(directory: &Path, name: &str, limit: u64) -> Result<Vec<u8>> {
    let path = directory.join(name);
    let metadata =
        std::fs::symlink_metadata(&path).map_err(|_| "security_upstream_artifact_missing")?;
    if !metadata.is_file()
        || metadata.len() > limit
        || !path
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(directory.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("security_upstream_invalid".into());
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

fn import_completed(run: &mut Run) -> Result<()> {
    import_bundle(run, "completed")
}

pub fn recover_terminal_artifacts(run: &mut Run) -> Result<bool> {
    let Some(value) = run.upstream.as_ref() else {
        return Ok(false);
    };
    if value["report"].is_string() {
        return Ok(false);
    }
    let (plugin, state, id) = binding(run)?;
    let context = upstream::workbench(
        &plugin,
        &state,
        &["get-scan".into(), "--scan-id".into(), id],
    )?;
    let status = context["scan"]["progress"]["status"].as_str().unwrap_or("");
    if status == "complete" && run.status == "failed" && run.error == "security_evidence_mismatch" {
        run.upstream.as_mut().ok_or("security_upstream_missing")?["scan"] = context["scan"].clone();
        let error = run.error.clone();
        import_completed(run)?;
        // Historical validation failures stay failed even if the source is later changed.
        run.status = "failed".into();
        run.error = error.clone();
        run.upstream.as_mut().ok_or("security_upstream_missing")?["evidenceError"] = error.into();
        for finding in &mut run.findings {
            finding.verdict = "candidate".into();
        }
        return Ok(true);
    }
    if !matches!(status, "canceled" | "failed") || context["scan"]["reportAvailable"] != true {
        return Ok(false);
    }
    let terminal = run.status.clone();
    if terminal != status && !(terminal == "interrupted" && status == "failed") {
        return Ok(false);
    }
    run.upstream.as_mut().ok_or("security_upstream_missing")?["scan"] = context["scan"].clone();
    import_bundle(run, status)?;
    run.status = terminal;
    Ok(true)
}

fn import_bundle(run: &mut Run, expected_status: &str) -> Result<()> {
    let value = run.upstream.as_ref().ok_or("security_upstream_missing")?;
    let directory = PathBuf::from(
        value["scan"]["scanDir"]
            .as_str()
            .ok_or("security_upstream_invalid")?,
    );
    let (plugin, state, id) = binding(run)?;
    // The completed branch checks the workbench's pinned manifest digest and target binding,
    // validates the seal, and regenerates the readable projection without changing canonical facts.
    let verified = if expected_status == "completed" {
        upstream::workbench(
            &plugin,
            &state,
            &["complete-scan".into(), "--scan-id".into(), id.clone()],
        )?
    } else {
        let python = std::env::var("PYTHON")
            .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
        let checked = crate::host::command(python)
            .arg(plugin.join("scripts/validate_scan_contract.py"))
            .arg("--scan-dir")
            .arg(&directory)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output()
            .map_err(|_| "security_python_unavailable")?;
        if !checked.status.success() {
            return Err("security_upstream_invalid_seal".into());
        }
        json!({"scan":run.upstream.as_ref().ok_or("security_upstream_missing")?["scan"]})
    };
    let warnings = verified["scan"]["warnings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let parse = |name, limit| -> Result<Value> {
        serde_json::from_slice(&read_artifact(&directory, name, limit)?)
            .map_err(|_| "security_upstream_invalid".into())
    };
    let manifest = parse("scan-manifest.json", 16 * 1024 * 1024)?;
    let findings = parse("findings.json", 128 * 1024 * 1024)?;
    let coverage = review_coverage(&directory, &manifest)?;
    if manifest["scan"]["id"] != id || manifest["scan"]["status"] != expected_status {
        return Err("security_upstream_not_completed".into());
    }
    let evidence_error = if expected_status == "completed" {
        validate_source_evidence(run, &findings).err()
    } else {
        None
    };
    let report = String::from_utf8(read_artifact(&directory, "report.md", 32 * 1024 * 1024)?)
        .map_err(|_| "security_upstream_invalid")?;
    run.threat_model = threat_model_text(&manifest["scan"]["threatModel"]);
    run.gaps = coverage["deferred"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v["reason"].as_str().map(str::to_owned))
        .collect();
    run.findings.clear();
    for finding in findings["findings"]
        .as_array()
        .ok_or("security_upstream_invalid")?
    {
        let locations = finding["locations"]
            .as_array()
            .ok_or("security_upstream_invalid")?;
        let location = locations
            .iter()
            .find(|l| l["role"] == "root_control")
            .or_else(|| locations.first())
            .ok_or("security_upstream_invalid")?;
        let text = |v: &Value| {
            v.as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| v.to_string())
        };
        let path = location["path"]
            .as_str()
            .ok_or("security_upstream_invalid")?;
        let line = location["startLine"]
            .as_u64()
            .ok_or("security_upstream_invalid")? as usize;
        let end = location["endLine"].as_u64().unwrap_or(line as u64) as usize;
        let evidence = finding["codeEvidence"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|e| e["path"] == path && e["startLine"].as_u64() == Some(line as u64))
            .and_then(|e| e["code"].as_str())
            .unwrap_or("");
        run.findings.push(workflow::Finding {
            id: text(&finding["findingId"]),
            title: text(&finding["title"]),
            severity: text(&finding["severity"]["level"]),
            path: path.into(),
            line,
            end_line: end,
            evidence: evidence.into(),
            source: text(&finding["attackPath"]["dataflow"]),
            sink: text(&finding["rootCause"]),
            preconditions: text(&finding["attackPath"]["reachability"]),
            impact: text(&finding["summary"]),
            recommendation: text(&finding["remediation"]),
            verdict: if evidence_error.is_some() { "candidate" } else { "validated" }.into(),
        });
    }
    run.status = if evidence_error.is_some() {
        "failed"
    } else if expected_status != "completed" {
        expected_status
    } else if coverage_complete(&coverage) && warnings.is_empty() {
        "completed"
    } else {
        "partial"
    }
    .into();
    if let Some(error) = &evidence_error {
        run.error = error.clone();
    }
    let upstream = run.upstream.as_mut().ok_or("security_upstream_missing")?;
    upstream["evidenceError"] = evidence_error.into();
    upstream["coverageComplete"] = coverage_complete(&coverage).into();
    upstream["scan"] = verified["scan"].clone();
    upstream["manifest"] = manifest;
    upstream["findings"] = findings;
    upstream["coverage"] = coverage;
    upstream["report"] = report.into();
    Ok(())
}

/// What the scan reviewed and left open.
///
/// Releases before 0.8 seal it as `coverage.json`. Since 0.8 upstream keeps this bookkeeping out of the
/// published result: the manifest carries only a summary, and the full review state stays with the latest
/// checkpoint. Legacy detection follows upstream's own `uses_legacy_review`.
fn review_coverage(directory: &Path, manifest: &Value) -> Result<Value> {
    let parse = |name: &str, limit| -> Result<Value> {
        serde_json::from_slice(&read_artifact(directory, name, limit)?)
            .map_err(|_| "security_upstream_invalid".into())
    };
    let scan = &manifest["scan"];
    if scan.get("coverageRef").is_some() || (scan.get("reviewState").is_none() && scan.get("review").is_none()) {
        return parse("coverage.json", 32 * 1024 * 1024);
    }
    if let Some(state) = scan.get("reviewState") {
        return Ok(state.clone());
    }
    let head = parse("checkpoint-head.json", 64 * 1024)?;
    let name = head["checkpoint"]
        .as_str()
        .filter(|name| scope::safe_relative(name) && !name.contains(['/', '\\']))
        .ok_or("security_upstream_invalid")?;
    let checkpoint = parse(&format!("checkpoints/{name}"), 128 * 1024 * 1024)?;
    let mut state = checkpoint["reviewState"].clone();
    if !state.is_object() {
        return Err("security_upstream_invalid".into());
    }
    for key in ["surfaces", "deferred"] {
        if state[key].is_null() {
            state[key] = json!([]);
        }
    }
    Ok(state)
}

/// The threat model's summary, or since 0.8 its Markdown form when the scan retained one.
fn threat_model_text(model: &Value) -> String {
    model["summary"]
        .as_str()
        .or_else(|| model["content"].as_str())
        .unwrap_or("")
        .into()
}

/// Whether the sealed coverage leaves nothing open.
///
/// Upstream 0.8 no longer publishes `completeness`, so the claim is derived from what is still open: deferred
/// work and surfaces that need follow-up. Bundles sealed by older releases keep their explicit field.
fn coverage_complete(coverage: &Value) -> bool {
    if let Some(completeness) = coverage.get("completeness") {
        return completeness == "complete";
    }
    let open = |key: &str| coverage[key].as_array().is_some_and(|items| !items.is_empty());
    let follow_up = coverage["surfaces"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|surface| surface["disposition"] == "needs_follow_up");
    !open("deferred") && !follow_up
}

fn evidence_source(root_path: &str, base_revision: Option<&str>, path: &str) -> Result<String> {
    if !scope::safe_relative(path) {
        return Err("security_outside_scope".into());
    }
    let root = Path::new(root_path)
        .canonicalize()
        .map_err(|_| "security_source_missing")?;
    let file = root.join(path);
    if let Ok(metadata) = std::fs::symlink_metadata(&file) {
        if !metadata.is_file()
            || !file
                .canonicalize()
                .map_err(|_| "security_source_missing")?
                .starts_with(&root)
        {
            return Err("security_outside_scope".into());
        }
        return std::fs::read_to_string(file).map_err(|_| "security_source_missing".into());
    }
    // Deleted changed files retain their exact baseline location; do not silently drop them.
    if let Some(base_revision) = base_revision {
        let base = Some(base_revision)
            .filter(|s| matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or("security_source_missing")?;
        let output = crate::host::command("git")
            .args([
                "--no-optional-locks",
                "-C",
                root_path,
                "show",
                "--no-ext-diff",
                "--no-textconv",
                &format!("{base}:{path}"),
            ])
            .output()
            .map_err(|_| "security_source_missing")?;
        if output.status.success() {
            return String::from_utf8(output.stdout).map_err(|_| "security_binary".into());
        }
    }
    Err("security_source_missing".into())
}

fn validate_source_evidence(run: &Run, findings: &Value) -> Result<()> {
    let base = (run.scope == "working-tree")
        .then(|| run.upstream.as_ref()?.get("scan")?.get("diffTarget")?.get("baseRevision")?.as_str())
        .flatten();
    validate_evidence(&run.root, base, findings)
}

fn validate_evidence(root: &str, base: Option<&str>, findings: &Value) -> Result<()> {
    for finding in findings["findings"]
        .as_array()
        .ok_or("security_upstream_invalid")?
    {
        for location in finding["locations"]
            .as_array()
            .ok_or("security_upstream_invalid")?
        {
            let path = location["path"]
                .as_str()
                .ok_or("security_upstream_invalid")?;
            let source = evidence_source(root, base, path)?;
            let start = location["startLine"]
                .as_u64()
                .ok_or("security_upstream_invalid")?;
            let end = location["endLine"].as_u64().unwrap_or(start);
            if start == 0 || end < start || end > source.lines().count() as u64 {
                return Err("security_evidence_mismatch".into());
            }
        }
        for evidence in finding["codeEvidence"].as_array().into_iter().flatten() {
            let path = evidence["path"]
                .as_str()
                .ok_or("security_upstream_invalid")?;
            let start = evidence["startLine"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or("security_upstream_invalid")?;
            let code = evidence["code"]
                .as_str()
                .ok_or("security_upstream_invalid")?;
            let source = evidence_source(root, base, path)?;
            let count = code.lines().count() as u64;
            let last = start
                .checked_add(count.saturating_sub(1))
                .ok_or("security_evidence_mismatch")?;
            if count == 0
                || last > source.lines().count() as u64
                || evidence["endLine"].as_u64().is_some_and(|end| end != last)
            {
                return Err("security_evidence_mismatch".into());
            }
            let exact = source
                .lines()
                .skip(start as usize - 1)
                .take(code.lines().count())
                .collect::<Vec<_>>()
                .join("\n");
            let expected = code.lines().collect::<Vec<_>>().join("\n");
            if exact != expected {
                return Err("security_evidence_mismatch".into());
            }
        }
    }
    Ok(())
}

/// Check the original, still-unsealed draft before the MCP finalizer receives it.
/// The workbench owns the target and paths; callers cannot supply a replacement source root.
pub(super) fn verify_draft(plugin: &Path, state: &Path, id: &str) -> Result<Value> {
    let context = upstream::workbench(plugin, state, &["get-scan".into(), "--scan-id".into(), id.into()])
        .map_err(|_| "security_upstream_invalid")?;
    let scan = &context["scan"];
    if matches!(scan["progress"]["status"].as_str(), Some("complete" | "completed" | "failed" | "canceled")) {
        // Preserve upstream terminal/idempotent behavior.
        return Ok(json!({"ok":true}));
    }
    let root = scan["targetPath"].as_str().ok_or("security_upstream_invalid")?;
    let directory = Path::new(scan["scanDir"].as_str().ok_or("security_upstream_invalid")?);
    let findings: Value = serde_json::from_slice(&read_artifact(directory, "findings.json", 32 * 1024 * 1024)?)
        .map_err(|_| "security_upstream_invalid")?;
    let base = scan["diffTarget"]["baseRevision"].as_str();
    let mut issues = Vec::new();
    for (finding_index, finding) in findings["findings"].as_array().ok_or("security_upstream_invalid")?.iter().enumerate() {
        if finding["locations"].as_array().is_none() {
            issues.push(json!({"findingIndex":finding_index,"error":"security_upstream_invalid"}));
        }
        for field in ["locations", "codeEvidence"] {
            for (index, item) in finding[field].as_array().into_iter().flatten().enumerate() {
                let mut one = json!({"locations":[],"codeEvidence":[]});
                one[field] = json!([item]);
                if let Err(error) = validate_evidence(root, base, &json!({"findings":[one]})) {
                    let mut issue = json!({"findingIndex":finding_index,"field":field,"index":index,
                        "jsonPointer":format!("/findings/{finding_index}/{field}/{index}"),
                        "path":item["path"],"startLine":item["startLine"],"endLine":item["endLine"],"error":error});
                    if field == "codeEvidence" {
                        if let (Some(path), Some(start), Some(code)) =
                            (item["path"].as_str(), item["startLine"].as_u64().filter(|n| *n > 0), item["code"].as_str())
                        {
                            if let Ok(source) = evidence_source(root, base, path) {
                                let count = code.lines().count();
                                let matches = count > 0 && source.lines().skip(start as usize - 1).take(count).collect::<Vec<_>>()
                                    == code.lines().collect::<Vec<_>>();
                                issue["excerptLineCount"] = count.into();
                                issue["sourceLinesMatchAtStart"] = matches.into();
                                if matches {
                                    if let Some(end) = start.checked_add(count as u64 - 1) {
                                        issue["expectedEndLine"] = end.into();
                                    }
                                }
                            }
                        }
                    }
                    issues.push(issue);
                }
                if issues.len() >= 100 { break; }
            }
            if issues.len() >= 100 { break; }
        }
        if issues.len() >= 100 { break; }
    }
    Ok(json!({"ok":issues.is_empty(),"issues":issues}))
}

pub fn run(app: &AppCtx, run: &mut Run) -> Result<()> {
    let started = Instant::now();
    let result = execute(app, run);
    let snapshot = app.chat().snapshot(&run.session_id);
    let text = snapshot
        .rows
        .iter()
        .filter_map(|row| match row {
            ChatRow::Assistant { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    workflow::log(
        app,
        run,
        "ai_response",
        &json!({"step":"upstream_scan","method":"AI",
        "model":snapshot.model,"responseBytes":text.len(),"sha256":scope::digest(text.as_bytes()),
        "usage":snapshot.extras.native_usage,"usageSource":"native_protocol_cumulative",
        "entityType":"security_finding","inputCount":1,"outputCount":run.findings.len(),
        "status":if result.is_ok(){run.status.as_str()}else{"failed"},"durationMs":started.elapsed().as_millis()}),
    );
    if let Err(error) = &result {
        let detail = snapshot.rows.iter().rev().find_map(|row| match row {
            ChatRow::Error { message, .. } => Some(message.as_str()),
            ChatRow::Assistant { text, .. } if text.starts_with("API Error:") => {
                Some(text.as_str())
            }
            _ => None,
        });
        if let (Some(value), Some(detail)) = (run.upstream.as_mut(), detail) {
            value["failureDetail"] = detail
                .chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .take(4000)
                .collect::<String>()
                .into();
        }
        if error != "security_canceled" {
            if let Ok((plugin, state, id)) = binding(run) {
                // Upstream refuses to replace completed or canceled bundles and preserves checkpoints.
                let _ = upstream::workbench(
                    &plugin,
                    &state,
                    &[
                        "fail-scan".into(),
                        "--scan-id".into(),
                        id,
                        "--message".into(),
                        format!("Native audit host stopped: {error}"),
                    ],
                );
            }
        }
    }
    result
}

pub fn reconcile_stopped(app: &AppCtx, run: &mut Run) -> Result<bool> {
    if run.upstream.is_none() {
        return Ok(false);
    }
    match sync(app, run)?.as_str() {
        "completed" => {
            import_completed(run)?;
            save(app, run)?;
            Ok(true)
        }
        "canceled" => {
            run.status = "canceled".into();
            save(app, run)?;
            Ok(true)
        }
        _ => {
            let (plugin, state, id) = binding(run)?;
            let _ = upstream::workbench(
                &plugin,
                &state,
                &[
                    "fail-scan".into(),
                    "--scan-id".into(),
                    id,
                    "--message".into(),
                    "The native audit host was interrupted. Retained results are incomplete."
                        .into(),
                ],
            );
            Ok(false)
        }
    }
}

#[derive(Default)]
struct ContinuationProgress {
    previous: Option<Value>,
    unchanged_turns: usize,
}

impl ContinuationProgress {
    fn observe(&mut self, checkpoint: Value) -> bool {
        if self.previous.as_ref() == Some(&checkpoint) {
            self.unchanged_turns += 1;
        } else {
            self.previous = Some(checkpoint);
            self.unchanged_turns = 0;
        }
        self.unchanged_turns < 3
    }
}

fn continuation_checkpoint(run: &Run) -> Result<Value> {
    let upstream = run.upstream.as_ref().ok_or("security_upstream_missing")?;
    let mut progress = upstream["scan"]["progress"].clone();
    if let Some(object) = progress.as_object_mut() {
        object.remove("updatedAt");
    }
    let directory = Path::new(upstream["scan"]["scanDir"].as_str().ok_or("security_upstream_invalid")?);
    let mut checkpoint = json!({"progress":progress});
    // Since 0.8 checkpoints move the head file instead of rewriting coverage.json.
    for name in ["findings.json", "coverage.json", "checkpoint-head.json"] {
        if directory.join(name).exists() {
            checkpoint[name] = serde_json::from_slice(&read_artifact(directory, name, 32 * 1024 * 1024)?)
                .map_err(|_| "security_upstream_invalid")?;
        }
    }
    Ok(checkpoint)
}

/// Whether the audit should prompt the agent to continue.
///
/// A turn that ends while the agent's background workers are still running is not idle: Claude Code wakes
/// the conversation itself when a worker reports, so prompting it again would only produce turns without
/// progress and end the audit while its workers are mid-review.
fn continuation_due(snapshot: &ChatSnapshot, message_id: &str, upstream_status: &str, background_pending: bool) -> Result<bool> {
    if upstream_status != "running" {
        return Ok(false);
    }
    if !snapshot.running {
        return Err("security_agent_stopped".into());
    }
    let Some(index) = snapshot.rows.iter().position(|r| matches!(r,ChatRow::User{id,..} if id==message_id)) else {
        return Ok(false);
    };
    let rows = &snapshot.rows[index + 1..];
    if rows.iter().any(|r| matches!(r, ChatRow::User { .. })) {
        return Err("security_conversation_changed".into());
    }
    if rows.iter().any(|r| matches!(r, ChatRow::Error { .. })) {
        return Err("security_agent_failed".into());
    }
    Ok(snapshot.turn_started_at.is_none() && snapshot.queue.is_empty() && snapshot.permissions.is_empty() && !background_pending)
}

fn execute(app: &AppCtx, run: &mut Run) -> Result<()> {
    let (plugin, _, id) = binding(run)?;
    let skill = plugin.join(upstream::entry_skill(run.scope == "working-tree"));
    let prompt = format!(
        "Execute the installed Codex Security workflow at {}. Read that SKILL.md and its referenced instructions.\n\
        This is VlxTerm's native {} host with the upstream local MCP server named {server}.\n\
        Continue existing scanId {} using get_codex_security_scan_context. Never create another scan.\n\
        The registered target and scope are authoritative. Use the upstream MCP tools for context, progress,\n\
        draft checkpoints (complete:false before validation), the final draft, completion and report retrieval.\n\
        Discover your actual delegation tools and report their availability honestly during preflight.\n\
        Apply the upstream baseline, architecture investigation, counterevidence and coverage rules.\n\
        Preserve the complete semantic result required by core-scan.md and finding-detail-fields.md.\n\
        Every reported finding must retain source-backed rootCause.summary, validation.summary,\n\
        attackPath.dataflow.summary and attackPath.reachability.summary, together with their supporting\n\
        facts, prerequisites, effective controls, counterevidence, impact and severity/confidence rationale.\n\
        Carry forward the actual threatModel and investigator evidence. Do not replace rich results\n\
        with a minimal title, one-line summary and code tuple merely because the schema permits it.\n\
        For a Standard scan, core-scan.md explicitly requires an independent baseline worker when\n\
        delegation is available. A worker-capacity warning does not disable delegation: use the actual\n\
        available allowance, launch that baseline, and reuse workers for coherent remaining file groups.\n\
        Use the registered scan model and reasoningEffort for every audit worker as well as the parent.\n\
        Complete the core-scan.md source-coverage reconciliation before finalizing: union only fully\n\
        audited paths, intersect with the authorized inventory, and finish every remaining in-scope file.\n\
        This request has no user-imposed time or file-count limit. Unfinished review is work to continue,\n\
        not a reason to finalize. Partial completion requires an actual user limit or unavailable source,\n\
        with the precise blocker and remaining paths preserved as required by the upstream workflow.\n\
        Keep stable candidate IDs and surface IDs across checkpoints. To resolve a deferred candidate as\n\
        reported, set finding.provenance.candidateId (or finding.extensions.candidateId) to that exact ID.\n\
        Matching identity.anchor, a title or a surface id alone does NOT resolve a deferred candidate.\n\
        A new source-backed candidate discovered later is still eligible: checkpoint it with a stable ID\n\
        and validate it normally. Never reject a finding merely because its ID was absent from an earlier\n\
        checkpoint. Rejection requires source-backed counterevidence under the upstream core rules.\n\
        For rejected or not-applicable candidates, record the exact candidateId and disposition on the\n\
        corresponding reviewState surface. Read back the reviewState returned by the final semantic draft call:\n\
        draft_written is acceptance of a draft, not proof that retained candidates have been reconciled.\n\
        If retained work is genuinely unresolved, keep it deferred and explain it; never force closure.\n\
        Routine upcoming stages (for example\n\
        final report assembly) belong in phase progress, not reviewState.deferred. Use needs_follow_up surfaces\n\
        for review still in progress and replace their dispositions under the same IDs when reviewed.\n\
        Preserve actual unresolved candidates and coverage gaps in deferred. Before complete_codex_security_scan,\n\
        read back the accepted final draft and reconcile any retained item claimed to be resolved through\n\
        the upstream semantic tool with its exact candidate identity and source-backed disposition.\n\
        Never edit sealed files or silently discard unresolved evidence to obtain complete coverage.\n\
        Every codeEvidence.code must contain exact contiguous source lines, including indentation, at\n\
        startLine through endLine. Do not shorten code with ellipses, omit indentation, splice expressions,\n\
        or translate source. Put interpretation in explanation. Verify excerpts before sealing.\n\
        Before the complete:true draft, check every excerpt against local source with a deterministic\n\
        comparison, including that endLine equals startLine plus the excerpt's line count minus one.\n\
        Correct any rejected excerpt from actual source before completion, never by editing sealed files.\n\
        Do not count truncated tool output as a complete source read: revisit missing ranges in bounded reads.\n\
        Preserve the user's native configuration and permissions. Do not modify persistent agent settings.\n\
        Source is read-only; put analysis artifacts in the registered scanDir. Do not publish, create external\n\
        resources, send messages or execute remediation. Do not disclose credential values in tool output\n\
        or reports. Review credential-bearing files through redacted structural inspection; containing\n\
        credentials alone does not exclude a file from the authorized audit. Use exact non-secret\n\
        consumer or control lines as report evidence, retaining precise locations for sensitive values.\n\
        Write human-readable analysis, finding titles, summaries, threat-model text and reviewState notes in {} while retaining canonical field names, identifiers, source evidence and enums.\n\
        Finish with a concise explanation linking the generated report. A checkpoint, empty findings or\n\
        a finished chat turn is not scan completion; complete the upstream workflow before returning.",
        skill.display(), run.agent,id,run.language, server = upstream::mcp_server_name(&run.agent));
    crate::command_core::chat_start(
        app,
        &run.session_id,
        Some(run.model.as_deref().unwrap_or("")),
        Some(run.effort.as_deref().unwrap_or("")),
        false,
    )?;
    app.chat().attach(&run.session_id);
    let mut message_id = uuid::Uuid::new_v4().to_string();
    workflow::log(
        app,
        run,
        "ai_request",
        &json!({"step":"upstream_scan","method":"AI",
        "interface":format!("{} local CLI",run.agent),"model":run.model,"effort":run.effort,
        "goal":"codex_security_scan","inputType":"text","originalChars":prompt.chars().count(),
        "sentChars":prompt.chars().count(),"truncated":false,"limit":null,"imageCount":0,
        "schema":"codex-security/1.0","preview":"[audit context redacted]","sha256":scope::digest(prompt.as_bytes()),
        "inputCount":1,"outputCount":0,"status":"started","durationMs":0}),
    );
    app.chat().send_identified(
        app,
        &run.session_id,
        &prompt,
        vec![],
        "queue",
        Some(&message_id),
    )?;
    let started = Instant::now();
    let mut last_sync = Instant::now() - Duration::from_secs(3);
    let mut upstream_status = "running".to_string();
    let mut continuation_progress = ContinuationProgress::default();
    continuation_progress.observe(continuation_checkpoint(run)?);
    let mut continuations = 0;
    loop {
        if get(app, &run.id)?.status == "canceled" {
            return Err("security_canceled".into());
        }
        if last_sync.elapsed() >= Duration::from_secs(3) {
            let status = sync(app, run)?;
            if status == "completed" {
                import_completed(run)?;
                return save(app, run);
            }
            if status == "canceled" {
                return Err("security_canceled".into());
            }
            if matches!(status.as_str(), "failed" | "interrupted") {
                return Err(format!("security_upstream_{status}"));
            }
            upstream_status = status;
            last_sync = Instant::now();
        }
        let snapshot = app.chat().snapshot(&run.session_id);
        let status = if snapshot.permissions.is_empty() {
            "running"
        } else {
            "waiting"
        };
        if run.status != status {
            run.status = status.into();
            save(app, run)?;
        }
        if continuation_due(&snapshot, &message_id, &upstream_status, app.chat().background_pending(&run.session_id))? {
            upstream_status = sync(app, run)?;
            if upstream_status == "completed" {
                import_completed(run)?;
                return save(app, run);
            }
            if upstream_status != "running" || get(app, &run.id)?.status == "canceled" {
                continue;
            }
            if !continuation_progress.observe(continuation_checkpoint(run)?) {
                if let Some(upstream) = run.upstream.as_mut() {
                    upstream["failureDetail"] = "The agent ended three consecutive turns without advancing the upstream review progress or saved findings and coverage.".into();
                }
                return Err("security_upstream_not_completed".into());
            }
            let continuation = format!(
                "Continue the same Codex Security scan {id} and native conversation. The last chat turn ended, \
                but the upstream scan remains running. Read its current context and saved reviewState, then finish \
                the remaining authorized review and candidate dispositions using the already loaded upstream workflow. \
                Reuse existing worker results; do not restart the scan or repeat completed phases. Preserve candidate IDs \
                and genuine evidence, and save source-backed progress after meaningful review batches. \
                A pending candidate or unfinished report assembly is work to continue, not a reason to end the task. \
                If an actual user restriction or unavailable source prevents further review, preserve the exact blocker \
                and remaining paths and follow the upstream partial-completion contract honestly. \
                Verify exact source excerpts before finalization. Keep the same model, effort, language and permissions. \
                Return only after upstream completion, explicit cancellation or a concrete unrecoverable blocker."
            );
            continuations += 1;
            if let Some(upstream) = run.upstream.as_mut() {
                upstream["continuations"] = continuations.into();
            }
            save(app, run)?;
            workflow::log(app, run, "ai_request", &json!({
                "step":"upstream_continuation","method":"AI","interface":format!("{} local CLI",run.agent),
                "model":run.model,"effort":run.effort,"goal":"continue_incomplete_security_scan",
                "inputType":"text","originalChars":continuation.chars().count(),"sentChars":continuation.chars().count(),
                "truncated":false,"limit":null,"imageCount":0,"schema":"codex-security/1.0",
                "preview":"[audit continuation context redacted]","sha256":scope::digest(continuation.as_bytes()),
                "inputCount":1,"outputCount":0,"status":"started","durationMs":0,"continuation":continuations
            }));
            message_id = uuid::Uuid::new_v4().to_string();
            app.chat().send_identified(app, &run.session_id, &continuation, vec![], "queue", Some(&message_id))?;
            last_sync = Instant::now();
        }
        // There is no file-batch timeout: long scans retain their upstream checkpoints.
        if started.elapsed() > Duration::from_secs(24 * 60 * 60) {
            return Err("security_timeout".into());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn coverage_is_complete_only_when_nothing_remains_open() {
        use serde_json::json;
        assert!(super::coverage_complete(&json!({"deferred":[],"surfaces":[{"disposition":"no_issue_found"},{"disposition":"reported"}]})));
        assert!(!super::coverage_complete(&json!({"deferred":[{"id":"d","reason":"later"}],"surfaces":[]})));
        assert!(!super::coverage_complete(&json!({"deferred":[],"surfaces":[{"disposition":"needs_follow_up"}]})));
        assert!(super::coverage_complete(&json!({"completeness":"complete","deferred":[]})));
        assert!(!super::coverage_complete(&json!({"completeness":"partial","deferred":[]})));
    }

    #[test]
    fn threat_model_reads_summary_or_markdown_content() {
        use serde_json::json;
        assert_eq!(super::threat_model_text(&json!({"summary":"Assets"})), "Assets");
        assert_eq!(super::threat_model_text(&json!({"format":"markdown","content":"# Model"})), "# Model");
        assert_eq!(super::threat_model_text(&serde_json::Value::Null), "");
    }

    use super::*;
    #[test]
    fn incomplete_audit_continues_only_after_a_clean_finished_turn() {
        let manager = crate::agent::chat::engine::ChatManager::default();
        let mut snapshot = manager.snapshot("continuation-test");
        snapshot.running = true;
        snapshot.rows.push(ChatRow::User { id:"audit".into(), text:"audit".into(), images:vec![], at:None });
        assert!(continuation_due(&snapshot, "audit", "running", false).unwrap());
        // Background workers will wake the agent themselves; prompting now would only waste a turn.
        assert!(!continuation_due(&snapshot, "audit", "running", true).unwrap());
        for terminal in ["completed", "canceled", "failed", "interrupted"] {
            assert!(!continuation_due(&snapshot, "audit", terminal, false).unwrap());
        }
        snapshot.turn_started_at = Some(1);
        assert!(!continuation_due(&snapshot, "audit", "running", false).unwrap());
        snapshot.turn_started_at = None;
        snapshot.permissions.push(json!({"id":"pending"}));
        assert!(!continuation_due(&snapshot, "audit", "running", false).unwrap());
        snapshot.permissions.clear();
        snapshot.rows.push(ChatRow::Error { id:"failure".into(), message:"quota exhausted".into() });
        assert_eq!(continuation_due(&snapshot, "audit", "running", false).unwrap_err(), "security_agent_failed");
        snapshot.rows.pop();
        snapshot.rows.push(ChatRow::User { id:"next".into(), text:"continue".into(), images:vec![], at:None });
        assert_eq!(continuation_due(&snapshot, "audit", "running", false).unwrap_err(), "security_conversation_changed");
        assert!(continuation_due(&snapshot, "next", "running", false).unwrap());
        snapshot.running = false;
        assert_eq!(continuation_due(&snapshot, "next", "running", false).unwrap_err(), "security_agent_stopped");
    }

    #[test]
    fn continuation_requires_saved_progress_and_does_not_count_clock_updates() {
        let directory = std::env::temp_dir().join(format!("vlx-audit-continuation-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut run:Run = serde_json::from_value(json!({
            "id":"continuation-test","projectId":"test-project","sessionId":"test-session","agent":"codex",
            "root":directory,"scope":"repository","path":"","status":"running","phase":"discovery",
            "createdAt":0,"updatedAt":0,"files":[],"excluded":[],"reviewed":[],"findings":[],
            "threatModel":"","steps":[],"gaps":[],"error":"",
            "upstream":{"scan":{"scanDir":directory,"progress":{"coverage":{"closedRows":1},"updatedAt":"first"}}}
        })).unwrap();
        let initial = continuation_checkpoint(&run).unwrap();
        run.upstream.as_mut().unwrap()["scan"]["progress"]["updatedAt"] = "later".into();
        assert_eq!(initial, continuation_checkpoint(&run).unwrap());
        let mut progress = ContinuationProgress::default();
        assert!(progress.observe(initial.clone()));
        assert!(progress.observe(initial.clone()));
        assert!(progress.observe(initial.clone()));
        assert!(!progress.observe(initial.clone()));
        std::fs::write(directory.join("coverage.json"), r#"{"deferred":[{"candidateId":"candidate","reason":"awaiting validation"}]}"#).unwrap();
        let saved = continuation_checkpoint(&run).unwrap();
        assert_ne!(initial, saved);
        assert!(progress.observe(saved));
        assert_eq!(progress.unchanged_turns, 0);
        run.upstream.as_mut().unwrap()["scan"]["progress"]["coverage"]["closedRows"] = 2.into();
        assert!(progress.observe(continuation_checkpoint(&run).unwrap()));
        assert_eq!(progress.unchanged_turns, 0);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn native_evidence_accepts_line_endings_but_rejects_changed_code() {
        let directory =
            std::env::temp_dir().join(format!("vlx-source-evidence-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("main.py"), "value = 1\r\n\r\n").unwrap();
        let run: Run=serde_json::from_value(json!({
            "id":"evidence-test","projectId":"test-project","sessionId":"test-session","agent":"codex",
            "root":directory,"scope":"repository","path":"","status":"running","phase":"validation",
            "createdAt":0,"updatedAt":0,"files":[],"excluded":[],"reviewed":[],"findings":[],
            "threatModel":"","steps":[],"gaps":[],"error":""
        })).unwrap();
        let mut document = json!({"findings":[{"locations":[{"path":"main.py","startLine":1,"endLine":2}],
            "codeEvidence":[{"path":"main.py","startLine":1,"code":"value = 1\r\n\r\n"}]}]});
        assert!(validate_source_evidence(&run, &document).is_ok());
        document["findings"][0]["codeEvidence"][0]["code"] = "value = 1\n\n".into();
        assert!(validate_source_evidence(&run, &document).is_ok());
        document["findings"][0]["codeEvidence"][0]["endLine"] = 3.into();
        assert!(validate_source_evidence(&run, &document).is_err());
        document["findings"][0]["codeEvidence"][0]["endLine"] = 2.into();
        document["findings"][0]["codeEvidence"][0]["code"] = "value = ...\n\n".into();
        assert_eq!(validate_source_evidence(&run, &document).unwrap_err(), "security_evidence_mismatch");
        document["findings"][0]["codeEvidence"][0]["code"] = "value = 1\n\n".into();
        std::fs::write(directory.join("main.py"), "value = 2\r\n\r\n").unwrap();
        assert_eq!(
            validate_source_evidence(&run, &document).unwrap_err(),
            "security_evidence_mismatch"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn canceled_checkpoint_import_preserves_status_and_rejects_tampering() {
        let directory =
            std::env::temp_dir().join(format!("vlx-stopped-artifacts-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(directory.join("source")).unwrap();
        std::fs::write(directory.join("source/main.py"), "value = 1\n").unwrap();
        let plugin = upstream::materialize(&directory).unwrap();
        let state = directory.join("state");
        let mut run: Run = serde_json::from_value(json!({
            "id":"test-run","projectId":"test-project","sessionId":"test-session","agent":"codex",
            "root":directory.join("source"),"scope":"repository","path":"","status":"running","phase":"preflight",
            "createdAt":0,"updatedAt":0,"files":[],"excluded":[],"reviewed":[],"findings":[],
            "threatModel":"","steps":[],"gaps":[],"error":""
        })).unwrap();
        register(&plugin, &state, &mut run).unwrap();
        let id = run.upstream.as_ref().unwrap()["scan"]["scanId"]
            .as_str()
            .unwrap()
            .to_owned();
        let draft = directory.join("draft.json");
        std::fs::write(&draft,serde_json::to_vec(&json!({
            "scanId":id,"complete":false,"threatModel":{"summary":"Transport test only; no security review performed."},
            "findings":[],"reviewState":{"surfaces":[],"explicitExclusions":[],
            "deferred":[{"id":"transport-only","reason":"No scan performed; verify checkpoint retention only."},
                {"id":"resolved-candidate","candidateId":"resolved-candidate","reason":"Transport candidate awaiting an explicit disposition."}]}
        })).unwrap()).unwrap();
        // Exercise the semantic MCP draft interface; write-scan-draft accepts only host-normalized documents.
        let script = r#"
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {readFileSync} from 'node:fs';
const child=spawn('node',[process.argv[1]+'/mcp/server.mjs','--stdio'],{
  env:{...process.env,CODEX_SECURITY_STATE_DIR:process.argv[2],PYTHONDONTWRITEBYTECODE:'1'},stdio:['pipe','pipe','inherit']});
let sequence=0;const pending=new Map();
createInterface({input:child.stdout}).on('line',line=>{try{const v=JSON.parse(line);if(v.id&&pending.has(v.id)){pending.get(v.id)(v);pending.delete(v.id);}}catch{}});
const send=(method,params)=>new Promise(resolve=>{const id=++sequence;pending.set(id,resolve);child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});
const timeout=setTimeout(()=>{child.kill();process.exit(2);},30000);
try {
 const init=await send('initialize',{protocolVersion:'2024-11-05',capabilities:{},clientInfo:{name:'vlx-lifecycle-test',version:'1'}});
 if(init.error)throw Error(JSON.stringify(init.error));
 child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
 const initial=JSON.parse(readFileSync(process.argv[3],'utf8'));
 const result=await send('tools/call',{name:'record_codex_security_scan_draft',arguments:initial});
 if(result.error||result.result?.isError)throw Error(JSON.stringify(result));
 const resolved={...initial,reviewState:{...initial.reviewState,
   deferred:initial.reviewState.deferred.filter(candidate=>candidate.id!=='resolved-candidate'),
   surfaces:[{id:'resolved-surface',label:'Transport fixture',candidateId:'resolved-candidate',disposition:'rejected',notes:'This transport fixture is not a security finding.'}]}};
 const updated=await send('tools/call',{name:'record_codex_security_scan_draft',arguments:resolved});
 if(updated.error||updated.result?.isError)throw Error(JSON.stringify(updated));
 process.stdout.write(JSON.stringify(updated));
} catch(error){process.stderr.write(String(error));process.exitCode=1;} finally{clearTimeout(timeout);child.kill();}
"#;
        let result = crate::host::command("node")
            .args(["--input-type=module", "-e", script])
            .arg(&plugin)
            .arg(&state)
            .arg(&draft)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        cancel(&run).unwrap();
        run.status = "canceled".into();
        assert!(recover_terminal_artifacts(&mut run).unwrap());
        assert_eq!(run.status, "canceled");
        let value = run.upstream.as_mut().unwrap();
        assert_eq!(value["manifest"]["scan"]["status"], "canceled");
        assert!(value["report"]
            .as_str()
            .is_some_and(|text| !text.is_empty()));
        let deferred = value["coverage"]["deferred"].as_array().unwrap();
        assert!(deferred
            .iter()
            .any(|candidate| candidate["id"] == "transport-only"));
        assert!(!deferred
            .iter()
            .any(|candidate| candidate["id"] == "resolved-candidate"));
        let scan_dir = PathBuf::from(value["scan"]["scanDir"].as_str().unwrap());
        value.as_object_mut().unwrap().remove("report");
        // Since 0.8 review state is not a sealed file; tamper with a sealed one instead.
        std::fs::write(scan_dir.join("findings.json"), "{}").unwrap();
        assert!(recover_terminal_artifacts(&mut run).is_err());
        assert_eq!(run.status, "canceled");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
