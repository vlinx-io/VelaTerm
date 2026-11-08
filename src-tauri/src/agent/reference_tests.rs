//! Reference pipeline tests with local CLI stand-ins; no model or user transcript is accessed.

use super::*;
use crate::{agent::transcript::TranscriptMessage, db::repo, models::SessionKind};
use std::{os::unix::fs::PermissionsExt, path::PathBuf};

struct Fixture {
    app: AppCtx,
    dir: PathBuf,
    session: crate::models::Session,
}

impl Fixture {
    fn new(summary: bool) -> Self {
        let dir = std::env::temp_dir().join(format!("vlx-reference-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let session = {
            let conn = db.conn.lock().unwrap();
            let project = repo::create_virtual_project(&conn, "Reference tests").unwrap();
            repo::create_session(
                &conn,
                &project.id,
                None,
                "Source",
                SessionKind::Codex,
                None,
                dir.to_str(),
                None,
                None,
                None,
            )
            .unwrap()
        };
        let fixture = Self {
            app: AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(
                dir.clone(),
                db,
            ))),
            dir,
            session,
        };
        fixture.script("claude", "printf 'compressed fact\\n'");
        fixture.script("codex", "printf 'final answer\\n'");
        let settings = serde_json::json!({
            "referSummary": {"enabled": summary, "agent":"claude", "model":"summary-model", "effort":"high"},
            "agentDefaults": {
                "claude": {"path": fixture.dir.join("claude")},
                "codex": {"path": fixture.dir.join("codex")}
            }
        });
        repo::set_app_settings(
            &fixture.app.db().conn.lock().unwrap(),
            &std::collections::HashMap::from([("vlx-settings".to_string(), settings.to_string())]),
        )
        .unwrap();
        fixture
    }

    fn script(&self, name: &str, result: &str) {
        let path = self.dir.join(name);
        std::fs::write(&path, format!(
            "#!/bin/sh\n[ \"$1\" = --version ] && {{ echo 1.0.0; exit 0; }}\nprintf '%s\\n' \"$@\" >> \"$0.args\"\nprintf '\\nCALL\\n' >> \"$0.prompts\"\ncat >> \"$0.prompts\"\n{result}\n"
        )).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn ask(&self, text: &str) -> Result<Answered, String> {
        let req: ReferRequest = serde_json::from_value(serde_json::json!({
            "sessionId":self.session.id, "target":self.session.id,
            "ask":"why?", "with":"codex", "timeout":10
        }))
        .unwrap();
        answer_question(
            &self.app,
            &req,
            "why?",
            &self.session,
            &[TranscriptMessage {
                role: "user".into(),
                text: text.into(),
                timestamp: None,
                tools: Vec::new(),
            }],
        )
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn reference_summary_chunks_keep_model_and_effort_separate_from_the_answerer() {
    let fixture = Fixture::new(true);
    let source = format!(
        "SOURCE-START{}SOURCE-END",
        "正文".repeat(SUMMARY_CHUNK_BYTES / 3)
    );
    let answer = fixture.ask(&source).unwrap();
    assert_eq!(answer.answer, "final answer");
    assert_eq!(answer.kind, SessionKind::Codex);
    let summary = answer.pre_summary.unwrap();
    assert_eq!(summary.selection.agent, SessionKind::Claude);
    assert_eq!(summary.selection.model, "summary-model");
    assert_eq!(summary.selection.effort, "high");
    let prompts = fixture.read("claude.prompts");
    assert!(prompts.contains("SOURCE-START") && prompts.contains("SOURCE-END"));
    assert_eq!(prompts.matches("\nCALL\n").count(), 3);
    assert_eq!(
        fixture
            .read("claude.args")
            .matches("--model\nsummary-model\n--effort\nhigh")
            .count(),
        3
    );
    let final_prompt = fixture.read("codex.prompts");
    assert!(final_prompt.contains("COMPRESSED SESSION SUMMARY:"));
    assert_eq!(final_prompt.matches("compressed fact").count(), 3);
    assert!(!final_prompt.contains("SOURCE-START"));
    assert!(!fixture.read("codex.args").contains("summary-model"));
}

#[test]
fn reference_full_mode_skips_pre_summary_and_preserves_the_whole_source() {
    let fixture = Fixture::new(false);
    let source = format!("SOURCE-START{}SOURCE-END", "原文".repeat(30_000));
    let answer = fixture.ask(&source).unwrap();
    assert!(answer.pre_summary.is_none());
    assert!(fixture.read("codex.prompts").contains(&source));
    assert!(!fixture.dir.join("claude.prompts").exists());
}

#[test]
fn reference_summary_failure_stops_before_answering_and_keeps_its_reason() {
    let fixture = Fixture::new(true);
    fixture.script("claude", "printf 'summary failed\\n' >&2; exit 3");
    let error = fixture
        .ask("source evidence")
        .err()
        .expect("summary must fail");
    assert!(error.contains("summary failed"), "{error}");
    assert!(!fixture.dir.join("codex.prompts").exists());
}

#[test]
fn reference_handler_returns_the_requested_transcript_window_when_summary_fails() {
    let fixture = Fixture::new(true);
    fixture.script("claude", "printf 'summary failed\\n' >&2; exit 3");
    let req = serde_json::from_value(serde_json::json!({
        "sessionId":fixture.session.id, "target":"Source",
        "ask":"why?", "with":"codex", "last":1
    }))
    .unwrap();
    let (status, body) = handle_refer_with_reader(&fixture.app, req, |_, id| {
        assert_eq!(id, fixture.session.id);
        Ok(["first evidence", "last evidence"]
            .into_iter()
            .map(|text| TranscriptMessage {
                role: "user".into(),
                text: text.into(),
                timestamp: None,
                tools: Vec::new(),
            })
            .collect())
    });
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(status, 200);
    assert_eq!(body["sessionId"], fixture.session.id);
    assert_eq!(body["total"], 2);
    assert_eq!(body["range"], serde_json::json!([1, 2]));
    assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    assert_eq!(body["messages"][0]["text"], "last evidence");
    assert!(body["askFailed"]
        .as_str()
        .unwrap()
        .contains("summary failed"));
    assert!(body.get("answer").is_none());
    assert!(!fixture.dir.join("codex.prompts").exists());
}

#[test]
fn reference_original_excerpts_are_deduplicated_and_scoped_to_the_target() {
    let fixture = Fixture::new(true);
    let messages={
        let conn = fixture.app.db().conn.lock().unwrap();
        let other = repo::create_session(
            &conn,
            &fixture.session.project_id,
            None,
            "Other",
            SessionKind::Codex,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        vec![(fixture.session.id.clone(),"cache timeout evidence"),(other.id,"cache timeout unrelated")]
    };
    let store=crate::agent::history_store::store::Store::at(&fixture.dir).unwrap();
    for (id,text) in messages {
        let native=format!("fixture-{id}");let path=fixture.dir.join(format!("{id}.jsonl"));
        std::fs::write(&path,serde_json::json!({"type":"response_item","payload":{"type":"message","id":"user","role":"user",
            "content":[{"type":"input_text","text":text}]}}).to_string()+"\n").unwrap();
        repo::set_agent_session_id(&fixture.app.db().conn.lock().unwrap(),&id,&native,SessionKind::Codex).unwrap();
        let source=store.register(SessionKind::Codex,crate::agent::history_store::source::Segment {path,native_id:native.clone(),end_byte:None,end_ordinal:None}).unwrap();
        crate::agent::history_store::api::bind(store.clone(),vec![source],SessionKind::Codex,native,None,&id).unwrap();
    }
    let excerpts =
        relevant_original_excerpts(&fixture.app, &fixture.session.id, "cache timeout").unwrap();
    assert_eq!(excerpts.len(), 1);
    assert_eq!(excerpts[0].message_index, 0);
    assert_eq!(excerpts[0].score, 2);
    assert!(excerpts[0].snippet.contains("evidence"));
    assert!(!excerpts[0].snippet.contains("unrelated"));
    crate::agent::history_store::store::release_test_directory(&fixture.dir);
}
