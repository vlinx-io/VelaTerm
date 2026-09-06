//! Bounded, persistent split diagnostics. Only structural identifiers are accepted, never PTY text.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::host::AppCtx;

const MAX_BYTES: u64 = 4 * 1024 * 1024;
static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Shortcut,
    Menu,
    PaneButton,
    Mirror,
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub source: Source,
    pub client_at_ms: u64,
    pub session_ids: Vec<String>,
    pub parent_session_id: Option<String>,
    pub tab_id: Option<String>,
    pub direction: Option<Direction>,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

impl Entry {
    fn validate(&self) -> Result<(), String> {
        if self.session_ids.is_empty()
            || self.session_ids.len() > 200
            || !self.session_ids.iter().all(|id| valid_id(id))
            || self
                .parent_session_id
                .as_deref()
                .is_some_and(|id| !valid_id(id))
            || self.tab_id.as_deref().is_some_and(|id| !valid_id(id))
        {
            return Err("invalid_split_trace_identifiers".into());
        }
        Ok(())
    }
}

fn timestamp() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

fn append(dir: &Path, line: &str, limit: u64) -> io::Result<()> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("split log lock poisoned"))?;
    fs::create_dir_all(dir)?;
    let path = dir.join("split.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() + line.len() as u64 + 1 > limit) {
        let previous = dir.join("split.previous.log");
        match fs::remove_file(&previous) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        fs::rename(&path, previous)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    writeln!(options.open(path)?, "{line}")
}

/// `client` is supplied by the native window or WebSocket connection, never by the payload.
pub fn record(app: &AppCtx, client: &str, entry: Entry) -> Result<bool, String> {
    entry.validate()?;
    if !valid_id(client) {
        return Err("invalid_split_trace_client".into());
    }
    let level = std::env::var("VLX_SPLIT_LOG_LEVEL")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(level.as_str(), "off" | "error" | "warn") {
        return Ok(false);
    }
    let dir = std::env::var_os("VLX_SPLIT_LOG_DIR")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(|| app.data_dir().map(|dir| dir.join("logs")))?;
    let payload = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    let line = format!(
        "{} [INFO ] [system] event=split clientId={client} data={payload}",
        timestamp()
    );
    if let Err(error) = append(&dir, &line, MAX_BYTES) {
        eprintln!(
            "{} [WARN ] [system] event=split_log status=failed error={error}",
            timestamp()
        );
        return Err("split_trace_write_failed".into());
    }
    eprintln!("{line}");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_log_injection_and_unbounded_records() {
        let mut entry: Entry = serde_json::from_value(serde_json::json!({
            "source": "menu", "clientAtMs": 1, "sessionIds": ["eph-123"],
            "parentSessionId": "session-1", "tabId": "tab-1", "direction": "horizontal"
        }))
        .unwrap();
        assert!(entry.validate().is_ok());
        entry.session_ids = vec!["bad\n[INFO]".into()];
        assert!(entry.validate().is_err());
        entry.session_ids = vec!["eph-123".into(); 201];
        assert!(entry.validate().is_err());
        entry.session_ids = vec!["eph-123".into()];
        entry.tab_id = Some("x".repeat(129));
        assert!(entry.validate().is_err());
        assert!(serde_json::from_value::<Entry>(serde_json::json!({
            "source": "menu", "clientAtMs": 1, "sessionIds": ["eph-123"], "terminalText": "private"
        }))
        .is_err());
    }

    #[test]
    fn log_survives_reopen_and_keeps_only_one_rotated_file() {
        let dir = std::env::temp_dir().join(format!("vlx-split-log-{}", uuid::Uuid::new_v4()));
        append(&dir, "first", 13).unwrap();
        append(&dir, "second", 13).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("split.log")).unwrap(),
            "first\nsecond\n"
        );
        append(&dir, "third", 13).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("split.previous.log")).unwrap(),
            "first\nsecond\n"
        );
        append(&dir, "fourth", 13).unwrap();
        append(&dir, "fifth", 13).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("split.previous.log")).unwrap(),
            "third\nfourth\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("split.log")).unwrap(),
            "fifth\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(dir.join("split.log"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn browser_dispatch_persists_connection_identity_and_structural_fields() {
        use crate::host::HeadlessHost;
        use crate::web::dispatch::{dispatch, CallOrigin};
        use std::sync::Arc;

        let dir = std::env::temp_dir().join(format!("vlx-split-dispatch-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(dir.clone(), db)));
        let args = serde_json::json!({ "entry": {
            "source": "shortcut", "clientAtMs": 1234,
            "sessionIds": ["eph-123"], "parentSessionId": "session-1",
            "tabId": "tab-1", "direction": "vertical"
        }});
        dispatch(
            &app,
            "record_split_trace",
            &args,
            "ws-7",
            CallOrigin::Remote,
        )
        .unwrap();
        let log = fs::read_to_string(dir.join("logs/split.log")).unwrap();
        assert_eq!(log.lines().count(), 1);
        assert!(log.contains("[INFO ] [system] event=split clientId=ws-7 data="));
        let payload: serde_json::Value =
            serde_json::from_str(log.split_once(" data=").unwrap().1).unwrap();
        assert_eq!(payload, args["entry"]);
        drop(app);
        fs::remove_dir_all(dir).unwrap();
    }
}
