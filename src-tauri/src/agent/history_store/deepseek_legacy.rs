//! DeepSeek conversations the retired DeepSeek Harness recorded, shown beside OpenCode ones.
//!
//! DeepSeek is now driven by OpenCode and its conversations live in OpenCode's database. A conversation
//! recorded before that switch exists only as a dsh log under `$DSH_HOME/sessions`; OpenCode has never
//! heard of its id. The log is laid out once, in memory, as the same row records the other providers
//! produce, so the bounded history reader, search, prose and export serve it without a second view.
//!
//! dsh writes compressed, append-only events, not addressable records: every message, call and result is
//! replayed into the timeline rows a live session would have drawn, then each row is one record. The
//! body, tool input and tool output stay in the record for detail reads while the row carries only the
//! preview. Rows are read-only: the process that wrote the log no longer exists, so rewind and resume
//! stay with the new OpenCode conversation.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::agent::chat::deepseek_history;
use crate::agent::export::Event;
use crate::models::SessionKind;

use super::adapter::{self, Refs, Scanned};
use super::limits::PREVIEW_BYTES;
use super::projection::Record;
use super::store::{RegisteredSource, State, Store};

/// Native identity prefix of a legacy source, so rewind never mistakes its rows for OpenCode ones.
pub const NATIVE_PREFIX: &str = "dsh-legacy:";

/// Whether `id` names a conversation only the retired harness recorded.
///
/// The log has to exist, and OpenCode has to have no session under that id. A database that cannot be
/// opened is treated as absent only when it has never been created; a locked or damaged store must not
/// silently turn a real OpenCode conversation into a read-only one.
pub fn is_legacy(id: &str) -> bool {
    if deepseek_history::find(id).is_none() {
        return false;
    }
    match crate::agent::opencode_store::session_exists(id) {
        Ok(exists) => !exists,
        Err(_) => crate::agent::opencode_store::db_path().is_none_or(|path| !path.is_file()),
    }
}

/// The log of a legacy conversation, when it is still there.
pub fn log(id: &str) -> Option<PathBuf> {
    deepseek_history::find(id).filter(|_| is_legacy(id))
}

pub(super) fn is_legacy_source(source: &RegisteredSource) -> bool {
    source.segment.native_id.starts_with(NATIVE_PREFIX)
}

fn fingerprint(path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|_| "source_missing: The DeepSeek conversation log could not be found")?;
    Ok(json!([
        super::source::identity(&metadata),
        metadata.len(),
        format!("{:?}", metadata.modified().ok())
    ])
    .to_string())
}

pub(super) fn register(
    store: &Store,
    kind: SessionKind,
    path: PathBuf,
    native: &str,
) -> Result<RegisteredSource, String> {
    let stamp = fingerprint(&path)?;
    let key = format!("dsh:{}:{}", path.to_string_lossy(), native);
    let layout_path = path.clone();
    store.register_virtual(kind, &key, &path, native, stamp, || layout(&layout_path))
}

fn preview(text: &str) -> String {
    adapter::preview(text, PREVIEW_BYTES)
}

/// The whole conversation as one row record per line. Bodies stay in the record; the row previews them.
fn layout(path: &Path) -> Result<Vec<u8>, String> {
    let events = deepseek_history::read_events(path)?;
    let attachments = deepseek_history::attachment_root();
    let rows = deepseek_history::rows(&events, attachments.as_deref());
    let mut bytes = Vec::new();
    for (ordinal, row) in rows.into_iter().enumerate() {
        let mut row = serde_json::to_value(&row).map_err(|e| e.to_string())?;
        let mut record = Map::new();
        record.insert("type".into(), json!("dsh_row"));
        record.insert("ordinal".into(), json!(ordinal));
        match row["kind"].as_str().unwrap_or_default() {
            "user" => body(&mut record, &mut row, "text", true),
            "assistant" | "reasoning" | "command" => body(&mut record, &mut row, "text", false),
            "error" | "notice" => body(&mut record, &mut row, "message", false),
            "wake" => body(&mut record, &mut row, "summary", false),
            "tool" => tool(&mut record, &mut row),
            "shell" => shell(&mut record, &mut row),
            _ => {}
        }
        record.insert("row".into(), row);
        serde_json::to_writer(&mut bytes, &Value::Object(record)).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
    }
    Ok(bytes)
}

/// Move a text body out of the row: the row keeps a preview, the record keeps the whole value.
fn body(record: &mut Map<String, Value>, row: &mut Value, key: &str, images: bool) {
    let full = row.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    row[key] = json!(preview(&full));
    row["bodySizeBytes"] = json!(full.len() as u64);
    if full.len() > PREVIEW_BYTES {
        row["bodyAvailable"] = json!(true);
    }
    record.insert("body".into(), json!(full));
    if !images {
        return;
    }
    let Some(list) = row.get_mut("images").and_then(Value::as_array_mut) else { return };
    let mut content = Vec::new();
    for image in std::mem::take(list) {
        let (Some(mime), Some(data)) = (
            image.get("mimeType").and_then(Value::as_str),
            image.get("data").and_then(Value::as_str),
        ) else {
            continue;
        };
        if !mime.starts_with("image/") || data.len() > crate::command_core::MAX_IMAGE_BYTES * 2 {
            continue;
        }
        // `attachments.rs` decodes `/message/content/*/data` as raw base64 (`prefix = 0`).
        let raw_data = data.strip_prefix("data:")
            .and_then(|rest| rest.split_once(";base64,"))
            .map_or(data, |(_, payload)| payload);
        content.push(json!({
            "type": "image",
            "mimeType": mime,
            "data": raw_data
        }));
    }
    if !content.is_empty() {
        record.insert("message".into(), json!({ "content": content }));
    }
}

/// A tool call keeps its whole input and output in the record and previews them in the row.
fn tool(record: &mut Map<String, Value>, row: &mut Value) {
    let input = row.get("input").cloned().unwrap_or_else(|| json!({}));
    row["input"] = adapter::short_input(&input);
    row["detailAvailable"] = json!(true);
    record.insert("input".into(), input);
    if let Some(output) = row.get("output").and_then(Value::as_str).map(str::to_owned) {
        row["output"] = json!(adapter::preview(&output, 512));
        record.insert("fullOutput".into(), json!(output));
    }
}

/// A `!` command's output is what the row shows; the record keeps the whole streams for detail reads.
fn shell(record: &mut Map<String, Value>, row: &mut Value) {
    for (key, field) in [("stdout", "fullStdout"), ("stderr", "fullStderr")] {
        let full = row.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        row[key] = json!(preview(&full));
        record.insert(field.into(), json!(full));
    }
}

pub(super) fn store_record(
    state: &mut State,
    scanned: &Scanned<'_>,
    record: &Record,
    refs: &Refs,
) -> Result<(), String> {
    let row = record.value["row"].clone();
    let Some(id) = row["id"].as_str().filter(|id| !id.is_empty()).map(str::to_owned) else {
        return Ok(());
    };
    let kind = row["kind"].as_str().unwrap_or_default().to_owned();
    let (label, paths) = match kind.as_str() {
        "tool" => (
            "input",
            record
                .fields
                .iter()
                .filter(|field| field.path == "/input" || field.path.starts_with("/input/"))
                .map(|field| field.path.clone())
                .collect::<Vec<_>>(),
        ),
        "shell" => (
            "output",
            vec!["/fullStdout".to_owned(), "/fullStderr".to_owned()],
        ),
        _ => ("body", vec!["/body".to_owned()]),
    };
    adapter::write_row(state, scanned, record, 0, None, &row, refs);
    adapter::attach(state, &id, label, &paths, refs);
    if kind == "tool" {
        adapter::attach(state, &id, "output", &["/fullOutput".into()], refs);
    }
    if kind == "user" {
        super::attachments::attach(state, scanned.source, record, &id, refs);
    }
    Ok(())
}

/// The events a prose reader takes from one row record.
pub(super) fn events(record: &Value) -> Vec<Event> {
    let row = &record["row"];
    let body = record["body"].as_str().unwrap_or_default();
    let at = row["at"].as_i64().map(|ms| crate::agent::usage::unix_to_rfc3339_utc((ms.max(0) / 1000) as u64));
    match row["kind"].as_str().unwrap_or_default() {
        "user" => vec![Event::User { text: body.into(), ts: at }],
        "assistant" => vec![Event::AssistantText { text: body.into(), ts: at }],
        "reasoning" => vec![Event::Thinking { text: body.into(), ts: at }],
        "tool" => vec![Event::ToolUse {
            id: row["id"].as_str().map(str::to_owned),
            name: row["name"].as_str().unwrap_or("tool").into(),
            input: record["input"].clone(),
            ts: None,
        }],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(path: &Path) {
        let events = [
            json!({"type":"turn/start","seq":1,"time":10,"data":{"turn":1}}),
            json!({"type":"user/message","seq":2,"time":11,"data":{"id":"u1","source":{"kind":"user"},
                "content":[{"type":"text","text":"Create hello.txt"}]}}),
            json!({"type":"assistant/message","seq":3,"time":12,"data":{"message":{"id":"m1","role":"assistant",
                "content":[{"type":"text","text":"Done."}]}}}),
            json!({"type":"turn/end","seq":4,"time":13,"data":{"reason":{"kind":"completed"}}}),
        ];
        let text: String = events.iter().map(|event| format!("{event}\n")).collect();
        std::fs::write(path, zstd::encode_all(text.as_bytes(), 3).unwrap()).unwrap();
    }

    #[test]
    fn a_legacy_log_lays_out_one_complete_record_per_visible_row() {
        let root = std::env::temp_dir().join(format!("dsh-legacy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("session.v4.jsonl.zstd");
        log(&path);
        let layout = layout(&path).unwrap();
        let lines = String::from_utf8(layout).unwrap();
        let records: Vec<Value> = lines.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["row"]["kind"], "user");
        assert_eq!(records[0]["body"], "Create hello.txt");
        assert_eq!(records[1]["row"]["kind"], "assistant");
        assert_eq!(records[1]["body"], "Done.");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_history_branch_pages_a_legacy_conversation() {
        let temp = super::super::tests::Temp::new();
        let path = temp.0.join("session.v4.jsonl.zstd");
        log(&path);
        let store = Store::at(&temp.0).unwrap();
        let source = register(&store, SessionKind::Deepseek, path, "dsh-legacy:s-1").unwrap();
        store
            .scan(SessionKind::Deepseek, &source, &std::sync::atomic::AtomicBool::new(false), false)
            .unwrap();
        let branch = super::super::api::bind(
            store,
            vec![source],
            SessionKind::Deepseek,
            "dsh-legacy:s-1".into(),
            None,
            "legacy",
        )
        .unwrap();
        let page = branch.page(None, None, None).unwrap();
        assert_eq!(page["coverage"], "complete");
        let rows = page["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["kind"], "user");
        assert_eq!(rows[0]["text"], "Create hello.txt");
        assert_eq!(rows[1]["kind"], "assistant");
        assert_eq!(rows[1]["text"], "Done.");
    }

    pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub(crate) struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);

    impl EnvGuard {
        pub(crate) fn set(vars: &[(&'static str, &std::path::Path)]) -> Self {
            let mut prev = Vec::with_capacity(vars.len());
            for &(key, val) in vars {
                prev.push((key, std::env::var_os(key)));
                std::env::set_var(key, val);
            }
            Self(prev)
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, old) in self.0.drain(..).rev() {
                match old {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    fn scan(branch: &crate::agent::history_store::api::Branch) {
        for source in &branch.sources {
            branch.store.scan(SessionKind::Deepseek, source, &std::sync::atomic::AtomicBool::new(false), false).unwrap();
        }
    }

    fn write_zstd_events(dir: &std::path::Path, session_id: &str, events: &[Value]) -> PathBuf {
        let log_dir = dir.join("sessions").join("proj").join(session_id);
        std::fs::create_dir_all(&log_dir).unwrap();
        let path = log_dir.join("session.v4.jsonl.zstd");
        let mut jsonl = String::new();
        for ev in events {
            jsonl.push_str(&ev.to_string());
            jsonl.push('\n');
        }
        let compressed = zstd::stream::encode_all(jsonl.as_bytes(), 3).unwrap();
        std::fs::write(&path, compressed).unwrap();
        path
    }

    #[test]
    fn legacy_prompt_image_resolves_through_attachment_store() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("dsh-legacy-img-{}", uuid::Uuid::new_v4()));
        let _guard = EnvGuard::set(&[("DSH_HOME", &dir)]);
        let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        let hash = "c414cd0e204de974f73753c7e28d7638e7b3691bb8b1a2bab6b25bb7fed7ce77";
        let obj_dir = dir.join("attachments/v1/objects").join(&hash[..2]);
        std::fs::create_dir_all(&obj_dir).unwrap();
        std::fs::write(obj_dir.join(hash), png).unwrap();

        let id = "11111111-1111-4111-8111-111111111111";
        let path = write_zstd_events(
            &dir,
            id,
            &[
                json!({"version":4,"type":"session","seq":0,"time":1,"data":{"sessionId":id,"projectId":"p"}}),
                json!({"version":4,"type":"user/message","seq":1,"time":100,"data":{"id":"u1","source":{"kind":"user"},"content":[
                    {"type":"text","text":"See screenshot"},
                    {"type":"image","attachment":{"attachmentId":format!("sha256:{hash}"),"mediaType":"image/png","width":1,"height":1,"bytes":png.len()}}
                ]}}),
            ],
        );

        let store = Store::at(&dir).unwrap();
        let source = register(&store, SessionKind::Deepseek, path, &format!("{NATIVE_PREFIX}{id}")).unwrap();
        store.scan(SessionKind::Deepseek, &source, &std::sync::atomic::AtomicBool::new(false), false).unwrap();
        let branch = crate::agent::history_store::api::bind(
            store,
            vec![source],
            SessionKind::Deepseek,
            id.to_string(),
            None,
            "test",
        )
        .unwrap();
        let page = branch.page(None, None, None).unwrap();
        let rows = page["rows"].as_array().unwrap();
        let reference = rows[0]["images"][0]["attachmentId"].as_str().unwrap()
            .strip_prefix("history:").expect("history attachment id");

        let facts = crate::agent::history_store::attachments::facts(&branch, reference, None).unwrap();
        assert_eq!(facts.mime, "image/png");
        assert!(facts.encoded > 0);

        let mut decoded = Vec::new();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let written = crate::agent::history_store::attachments::write(&branch, reference, None, &mut decoded, &cancel).unwrap();
        assert_eq!(written, png.len() as u64);
        assert_eq!(decoded, png);

        let resolved = crate::agent::history_store::attachments::resolve(&branch, reference, None, true).unwrap();
        assert_eq!(resolved.mime_type, "image/png");
        use base64::Engine as _;
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(&resolved.data).unwrap(), png);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_prepare_serves_legacy_dsh_when_parked_or_opencode_unwritten() {
        let _lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("dsh-legacy-api-{}", uuid::Uuid::new_v4()));
        let missing_oc = dir.join("missing-opencode.db");
        let _guard = EnvGuard::set(&[("DSH_HOME", &dir), ("OPENCODE_DB", &missing_oc)]);

        let dsh_id = "22222222-2222-4222-8222-222222222222";
        write_zstd_events(
            &dir,
            dsh_id,
            &[
                json!({"version":4,"type":"session","seq":0,"time":1,"data":{"sessionId":dsh_id,"projectId":"p"}}),
                json!({"version":4,"type":"user/message","seq":1,"time":10,"data":{"id":"u1","source":{"kind":"user"},"content":[{"type":"text","text":"Legacy question"}]}}),
                json!({"version":4,"type":"assistant/message","seq":2,"time":20,"data":{"message":{"role":"assistant","id":"a1",
                    "content":[{"type":"text","text":"Legacy answer"}]}}}),
            ],
        );

        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("app.db")).unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO projects (id, name, root_path, sort_order, created_at) VALUES ('p1', 'p', '/tmp', 0, 0)",
                [],
            ).unwrap();
        }
        let session = {
            let conn = db.conn.lock().unwrap();
            crate::db::repo::create_session(&conn, "p1", None, "ds", crate::models::SessionKind::Deepseek, None, None, None, None, None).unwrap()
        };
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(dir.clone(), db));
        let ctx = crate::host::AppCtx::Headless(host);

        // 1) Directly bound legacy id in agent_session_id.
        {
            let conn = ctx.db().conn.lock().unwrap();
            crate::db::repo::set_agent_session_id(&conn, &session.id, dsh_id, SessionKind::Deepseek).unwrap();
        }
        let branch = crate::agent::history_store::api::prepare(&ctx, &session.id).unwrap();
        scan(&branch);
        assert_eq!(branch.error, None);
        assert_eq!(branch.page(None, None, None).unwrap()["rows"].as_array().unwrap().len(), 2);

        // 2) Parked into legacy_agent_session_id (agent_session_id cleared before OpenCode starts).
        {
            let conn = ctx.db().conn.lock().unwrap();
            crate::db::repo::park_legacy_agent_session_id(&conn, &session.id, dsh_id, SessionKind::Deepseek).unwrap();
        }
        let branch = crate::agent::history_store::api::prepare(&ctx, &session.id).unwrap();
        scan(&branch);
        assert_eq!(branch.error, None, "parked legacy session should prepare without error");
        assert_eq!(branch.page(None, None, None).unwrap()["rows"].as_array().unwrap().len(), 2);

        // 3) New OpenCode ses_* assigned before OpenCode DB/session exists on disk.
        {
            let conn = ctx.db().conn.lock().unwrap();
            crate::db::repo::set_agent_session_id(&conn, &session.id, "ses_not_written_yet", SessionKind::Deepseek).unwrap();
        }
        let branch = crate::agent::history_store::api::prepare(&ctx, &session.id).unwrap();
        scan(&branch);
        assert_eq!(branch.error, None, "existing legacy source should not be hidden by missing opencode db");
        assert_eq!(branch.page(None, None, None).unwrap()["rows"].as_array().unwrap().len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

