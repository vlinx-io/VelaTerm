//! Bounded, persistent split diagnostics. Only structural identifiers are accepted, never PTY text.

use serde::{Deserialize, Serialize};

use crate::host::AppCtx;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Shortcut,
    Menu,
    PaneButton,
    Sidebar,
    Drop,
    Tile,
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

/// Only validated structural metadata reaches the shared asynchronous writer.
pub fn record(app: &AppCtx, client: &str, entry: Entry) -> Result<bool, String> {
    let _ = app;
    entry.validate()?;
    if !valid_id(client) {
        return Err("invalid_split_trace_client".into());
    }
    if !crate::diagnostics::enabled(
        &std::env::var("VLX_SPLIT_LOG_LEVEL").unwrap_or_else(|_| "info".into()),
        "INFO",
    ) {
        return Ok(false);
    }
    if !crate::diagnostics::level_enabled("INFO") {
        return Ok(false);
    }
    let accepted = crate::diagnostics::record(
        "INFO",
        "split",
        serde_json::json!({"clientId":client,"clientAtMs":entry.client_at_ms,"source":entry.source,"sessionIds":entry.session_ids,"parentSessionId":entry.parent_session_id,"tabId":entry.tab_id,"direction":entry.direction,"count":entry.session_ids.len()}),
    );
    if accepted {
        Ok(true)
    } else {
        Err("diagnostic_queue_unavailable".into())
    }
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
    fn accepts_every_frontend_split_source() {
        for source in ["shortcut", "menu", "pane-button", "sidebar", "drop", "tile", "mirror", "unknown"] {
            let entry: Entry = serde_json::from_value(serde_json::json!({
                "source": source, "clientAtMs": 1, "sessionIds": ["session-1"]
            }))
            .unwrap_or_else(|e| panic!("{source}: {e}"));
            assert!(entry.validate().is_ok(), "{source}");
        }
    }
}
