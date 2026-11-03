use super::*;
use std::path::PathBuf;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("vela-security-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/app.rs"),
            "fn main() {\n    unsafe_call();\n}\n",
        )
        .unwrap();
        assert!(crate::host::command("git")
            .arg("init")
            .arg(&root)
            .output()
            .unwrap()
            .status
            .success());
        Self(root)
    }
    fn root(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn scope_rejects_traversal_and_tracks_untracked_text() {
    let f = Fixture::new();
    for bad in ["../outside", "/tmp/x", "src/../x", "a\\b", "a\nb"] {
        assert!(!scope::safe_relative(bad));
    }
    let (_, files, excluded) = scope::collect(f.root(), "path", "src").unwrap();
    assert_eq!(files.len(), 1);
    assert!(excluded.is_empty());
    assert_eq!(files[0].path, "src/app.rs");
    assert!(scope::collect(f.root(), "repository", "src").is_err());
}
#[test]
fn non_git_directories_support_static_scopes() {
    let f = Fixture::new();
    std::fs::remove_dir_all(f.0.join(".git")).unwrap();
    let (_, files, _) = scope::collect(f.root(), "path", "src").unwrap();
    assert_eq!(files.len(), 1);
    assert!(scope::collect(f.root(), "working-tree", "").is_err());
}
#[test]
fn working_tree_includes_staged_and_untracked_files_before_first_commit() {
    let f = Fixture::new();
    assert!(crate::host::command("git")
        .arg("-C")
        .arg(&f.0)
        .args(["add", "src/app.rs"])
        .output()
        .unwrap()
        .status
        .success());
    std::fs::write(f.0.join("extra.txt"), "extra\n").unwrap();
    let (_, files, _) = scope::collect(f.root(), "working-tree", "").unwrap();
    assert_eq!(
        files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
        vec!["extra.txt", "src/app.rs"]
    );
}
#[cfg(unix)]
#[test]
fn scope_never_follows_external_symlinks() {
    let f = Fixture::new();
    std::os::unix::fs::symlink("/etc/passwd", f.0.join("src/linked")).unwrap();
    assert!(scope::source(f.root(), "src/linked").is_err());
}
fn example_finding() -> workflow::Finding {
    workflow::Finding {
        id: "candidate-1".into(),
        title: "Example".into(),
        severity: "high".into(),
        path: "src/app.rs".into(),
        line: 2,
        end_line: 2,
        evidence: "    unsafe_call();".into(),
        source: "request".into(),
        sink: "call".into(),
        preconditions: "untrusted input".into(),
        impact: "execution".into(),
        recommendation: "validate input".into(),
        verdict: "candidate".into(),
    }
}
#[test]
fn findings_require_exact_evidence_in_unchanged_scoped_source() {
    let f = Fixture::new();
    let (_, files, _) = scope::collect(f.root(), "repository", "").unwrap();
    let mut finding = example_finding();
    assert!(workflow::check_finding(f.root(), &files, &finding).is_ok());
    finding.evidence = "invented();".into();
    assert!(workflow::check_finding(f.root(), &files, &finding).is_err());
    finding = example_finding();
    std::fs::write(f.0.join("src/app.rs"), "changed\n").unwrap();
    assert_eq!(
        workflow::check_finding(f.root(), &files, &finding).unwrap_err(),
        "security_source_changed"
    );
}
#[test]
fn output_requires_current_stage_and_rejects_unknown_fields() {
    let value =
        json!({"stageId":"current","summary":"ok","reviewedFiles":[],"gaps":[],"findings":[]});
    assert!(workflow::parse(&format!("```json\n{value}\n```"), "current").is_ok());
    assert!(workflow::parse(&value.to_string(), "stale").is_err());
    let mut invalid = value;
    invalid["extra"] = json!(true);
    assert!(workflow::parse(&invalid.to_string(), "current").is_err());
}
#[test]
fn canceled_run_cannot_be_overwritten_and_orphaned_run_is_interrupted() {
    let f = Fixture::new();
    let db = crate::db::Db::open(&f.0.join("test.sqlite3")).unwrap();
    let ctx = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(
        f.0.clone(),
        db,
    )));
    let conn = ctx.db().conn.lock().unwrap();
    // Disable project foreign keys only for this isolated persistence fixture.
    conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    let mut run = Run {
        id: "run".into(),
        project_id: "project".into(),
        session_id: "session".into(),
        agent: "codex".into(),
        agent_label: agent_label("codex"),
        root: f.root().into(),
        scope: "repository".into(),
        path: String::new(),
        status: "running".into(),
        phase: "threat".into(),
        created_at: now(),
        updated_at: now(),
        files: vec![],
        excluded: vec![],
        reviewed: vec![],
        findings: vec![],
        threat_model: String::new(),
        steps: vec![],
        gaps: vec![],
        error: String::new(),
        language: "en".into(),
        model: Some("test-model".into()),
        effort: Some("high".into()),
        upstream: None,
    };
    conn.execute(
        "INSERT INTO security_runs VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            run.id,
            run.project_id,
            run.session_id,
            run.status,
            run.created_at,
            serde_json::to_string(&run).unwrap()
        ],
    )
    .unwrap();
    drop(conn);
    run.status = "canceled".into();
    save(&ctx, &mut run).unwrap();
    run.status = "completed".into();
    assert!(save(&ctx, &mut run).is_err());
    let saved = get(&ctx, "run").unwrap();
    assert_eq!(saved.status, "canceled");
    assert_eq!(saved.model.as_deref(), Some("test-model"));
    assert_eq!(saved.effort.as_deref(), Some("high"));
    let mut legacy = serde_json::to_value(&saved).unwrap();
    legacy.as_object_mut().unwrap().remove("model");
    legacy.as_object_mut().unwrap().remove("effort");
    assert!(serde_json::from_value::<Run>(legacy)
        .unwrap()
        .model
        .is_none());
    run.status = "running".into();
    ctx.db()
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE security_runs SET status='running',data=?1",
            [serde_json::to_string(&run).unwrap()],
        )
        .unwrap();
    assert_eq!(reconcile(&ctx, run).unwrap().status, "interrupted");
}

#[test]
fn selection_accepts_uncatalogued_models_and_rejects_unsupported_efforts() {
    use crate::agent::launch_models::{Catalog, Model};
    let catalog = Catalog {
        models: vec![Model {
            id: "test-model".into(), label: "Test".into(), effort_levels: vec!["low".into(), "high".into()],
            description: String::new(), large_context: false,
        }],
        effort_levels: vec!["low".into(), "medium".into()],
    };
    assert!(validate_selection(Some("test-model"), Some("high"), &catalog).is_ok());
    assert!(validate_selection(Some("test-model"), Some("max"), &catalog).is_err());
    // The detailed rules live in launch_models::check_choice; this only checks the audit's error codes.
    assert!(validate_selection(Some("newer-model"), None, &catalog).is_ok());
    assert_eq!(validate_selection(None, Some("high"), &catalog).unwrap_err(), "security_invalid:effort");
    assert_eq!(validate_selection(Some("two words"), None, &catalog).unwrap_err(), "security_invalid:model");
    assert!(validate_selection(Some(""), Some(""), &catalog).is_ok());
    assert_eq!(validate_selection(Some("bad\nmodel"), None, &catalog).unwrap_err(), "security_invalid:selection");
    let request: Request =
        serde_json::from_value(json!({"projectId":"p","agent":"codex","scope":"repository"}))
            .unwrap();
    assert!(request.model.is_none());
    assert!(request.effort.is_none());
}

#[test]
fn audit_agents_come_from_the_shared_launch_catalogue() {
    for agent in AUDIT_AGENTS {
        let kind = audit_kind(agent).unwrap();
        assert_eq!(kind.as_str(), *agent);
        // The label has to come from the catalogue, or the picker and the run header disagree about
        // what the same agent is called.
        assert_eq!(agent_label(agent), crate::agent::launch_options::label(kind));
        assert!(!agent_label(agent).is_empty());
    }
    // The audit attaches its MCP server through per-CLI arguments, so an agent without that wiring
    // must be refused rather than started with no way to report findings.
    for agent in ["opencode", "pi", "grok", "terminal", ""] {
        assert!(audit_kind(agent).is_err(), "{agent}");
        assert!(super::upstream::launch_args(
            agent,
            std::path::Path::new("/tmp/plugin"),
            std::path::Path::new("/tmp/state"),
            ""
        )
        .is_err(), "{agent}");
    }
}

#[test]
fn a_stored_run_reports_its_agent_label_after_an_agent_is_renamed() {
    // The label is not read back from the stored row, so an older run shows the current name.
    let stored = json!({
        "id":"r","projectId":"p","sessionId":"s","agent":"claude","root":"/tmp","scope":"repository",
        "path":"","status":"completed","phase":"report","createdAt":0,"updatedAt":0,"files":[],
        "excluded":[],"reviewed":[],"findings":[],"threatModel":"","steps":[],"gaps":[],"error":""
    });
    let mut run: Run = serde_json::from_value(stored).unwrap();
    assert_eq!(run.agent_label, "");
    run.agent_label = agent_label(&run.agent);
    assert_eq!(run.agent_label, crate::agent::launch_options::label(SessionKind::Claude));
}
