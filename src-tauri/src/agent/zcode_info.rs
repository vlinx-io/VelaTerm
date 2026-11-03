//! ZCode turn accounting, read from the usage tables ZCode keeps in its own session store.
//!
//! ZCode records one `model_usage` row per model request (tokens, duration, time to first token), one
//! `turn_usage` row per user turn and one `tool_usage` row per tool call, so the Info panel's numbers are
//! available whether or not the conversation's process is running. Nothing here writes: ZCode owns the file.
//!
//! Figures follow the panel's definitions shared by every agent, not ZCode's own totals: input includes
//! cached input, and the session total sums each request's input and output. ZCode's `session/usage`
//! instead counts only the input each request added, which would make the same session read far smaller
//! here than it does for Claude.

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use super::transcript::TurnStats;

pub fn turn_stats(id: &str) -> Result<TurnStats, String> {
    let db = super::zcode::session_db_path().ok_or("ZCode's session store could not be located")?;
    if !db.is_file() {
        return Err("ZCode has not recorded any conversation on this machine".to_string());
    }
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| format!("Failed to open ZCode's store: {e}"))?;
    // ZCode writes the same WAL database while it runs; wait briefly rather than fail on a busy lock.
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    stats_from(&conn, id).map_err(|e| format!("Failed to read ZCode's store: {e}"))
}

/// One model request as `model_usage` records it.
struct Request {
    turn_id: Option<String>,
    model: String,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    total: Option<u64>,
    duration_ms: Option<u64>,
    first_token_ms: Option<u64>,
}

impl Request {
    /// Input including cached input. Providers disagree on whether `input_tokens` already contains the
    /// cache traffic; the reported total settles it, the same test ZCode applies to its own figures.
    fn input(&self) -> u64 {
        let cache = self.cache_read + self.cache_write;
        if self.input == 0 {
            return cache;
        }
        if cache == 0 {
            return self.input;
        }
        match self.total {
            Some(total) if total.abs_diff(self.input + cache + self.output) < total.abs_diff(self.input + self.output) => {
                self.input + cache
            }
            _ => self.input,
        }
    }

    /// Streaming time after the first token, the window ZCode's own tokens-per-second figure uses. A
    /// response made only of tool calls records no first token and is left out rather than timed from the
    /// request, which would count the server's latency as generation.
    fn generation_ms(&self) -> Option<u64> {
        let (duration, first) = (self.duration_ms?, self.first_token_ms?);
        (duration > first).then(|| duration - first)
    }
}

fn stats_from(conn: &Connection, id: &str) -> rusqlite::Result<TurnStats> {
    let mut statement = conn.prepare(
        "SELECT turn_id, model_id, input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens,
                provider_total_tokens, duration_ms, time_to_first_token_ms
         FROM model_usage WHERE session_id = ?1 ORDER BY started_at, id",
    )?;
    let requests: Vec<Request> = statement
        .query_map([id], |row| {
            let count = |index: usize| row.get::<_, Option<i64>>(index).map(|v| v.map(|n| n.max(0) as u64));
            Ok(Request {
                turn_id: row.get(0)?,
                model: row.get(1)?,
                input: count(2)?.unwrap_or(0),
                output: count(3)?.unwrap_or(0),
                cache_read: count(4)?.unwrap_or(0),
                cache_write: count(5)?.unwrap_or(0),
                total: count(6)?,
                duration_ms: count(7)?,
                first_token_ms: count(8)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    // The turn that started last, which may not have reached the model yet; older stores lack the table.
    let latest_turn: Option<String> = if super::opencode_store::has_table(conn, "turn_usage") {
        conn.query_row(
            "SELECT turn_id FROM turn_usage WHERE session_id = ?1 ORDER BY started_at DESC LIMIT 1",
            [id],
            |row| row.get(0),
        )
        .optional()?
    } else {
        None
    };
    let latest_turn = latest_turn.or_else(|| requests.iter().rev().find_map(|r| r.turn_id.clone()));
    let Some(turn_id) = latest_turn else {
        return Ok(TurnStats { model: requests.last().map(|r| r.model.clone()), ..TurnStats::default() });
    };
    let turn: Vec<&Request> = requests.iter().filter(|r| r.turn_id.as_deref() == Some(turn_id.as_str())).collect();
    let input: u64 = turn.iter().map(|r| r.input()).sum();
    let output: u64 = turn.iter().map(|r| r.output).sum();
    let cached: u64 = turn.iter().map(|r| r.cache_read).sum();
    let (generated, generated_ms) = turn
        .iter()
        .filter_map(|r| r.generation_ms().map(|ms| (r.output, ms)))
        .fold((0u64, 0u64), |(tokens, ms), (t, m)| (tokens + t, ms + m));
    let tools = if super::opencode_store::has_table(conn, "tool_usage") {
        Some(conn.query_row(
            "SELECT COUNT(*) FROM tool_usage WHERE session_id = ?1 AND turn_id = ?2",
            [id, turn_id.as_str()],
            |row| row.get::<_, u32>(0),
        )?)
    } else {
        None
    };
    let known = !turn.is_empty();
    Ok(TurnStats {
        model: requests.last().map(|r| r.model.clone()),
        tokens: known.then_some(output),
        input_tokens: known.then_some(input),
        total_tokens: known.then_some(input + output),
        cached_tokens: (cached > 0).then_some(cached),
        cache_hit_percent: (input > 0 && cached <= input).then(|| cached as f64 * 100.0 / input as f64),
        session_tokens: (!requests.is_empty()).then(|| requests.iter().map(|r| r.input() + r.output).sum()),
        tools_used: tools,
        generation_tokens_per_second: (generated > 0 && generated_ms > 0)
            .then(|| generated as f64 * 1000.0 / generated_ms as f64),
        ..TurnStats::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE model_usage (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, turn_id TEXT, model_id TEXT NOT NULL,
                status TEXT NOT NULL, started_at INTEGER NOT NULL, duration_ms INTEGER, time_to_first_token_ms INTEGER,
                input_tokens INTEGER NOT NULL DEFAULT 0, output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_input_tokens INTEGER NOT NULL DEFAULT 0, cache_read_input_tokens INTEGER NOT NULL DEFAULT 0,
                provider_total_tokens INTEGER);
             CREATE TABLE turn_usage (session_id TEXT NOT NULL, turn_id TEXT NOT NULL, started_at INTEGER NOT NULL);
             CREATE TABLE tool_usage (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, turn_id TEXT);",
        )
        .unwrap();
        conn
    }

    fn request(conn: &Connection, id: &str, turn: &str, at: i64, tokens: (i64, i64, i64), timing: (Option<i64>, Option<i64>)) {
        let (input, output, cache_read) = tokens;
        conn.execute(
            "INSERT INTO model_usage (id, session_id, turn_id, model_id, status, started_at, duration_ms, time_to_first_token_ms,
                input_tokens, output_tokens, cache_read_input_tokens, provider_total_tokens)
             VALUES (?1, 's1', ?2, 'GLM-5.3', 'completed', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![id, turn, at, timing.0, timing.1, input, output, cache_read, input + output],
        )
        .unwrap();
    }

    #[test]
    fn latest_turn_rate_and_session_total() {
        let conn = store();
        conn.execute_batch("INSERT INTO turn_usage VALUES ('s1','t1',1),('s1','t2',10);
            INSERT INTO tool_usage VALUES ('a','s1','t1'),('b','s1','t2'),('c','s1','t2');").unwrap();
        request(&conn, "r1", "t1", 1, (1000, 100, 0), (Some(3000), Some(1000)));
        // Values from a recorded GLM turn: input already contains the cache reads.
        request(&conn, "r2", "t2", 10, (53395, 174, 9600), (Some(10558), Some(7891)));
        request(&conn, "r3", "t2", 11, (70760, 2018, 54848), (Some(48690), Some(2533)));
        // A tool-call-only response: no first token, so it adds tokens but no timing.
        request(&conn, "r4", "t2", 12, (71000, 80, 70000), (Some(2441), None));
        let stats = stats_from(&conn, "s1").unwrap();
        assert_eq!(stats.model.as_deref(), Some("GLM-5.3"));
        assert_eq!(stats.input_tokens, Some(53395 + 70760 + 71000));
        assert_eq!(stats.tokens, Some(174 + 2018 + 80));
        assert_eq!(stats.cached_tokens, Some(9600 + 54848 + 70000));
        assert_eq!(stats.tools_used, Some(2));
        assert_eq!(stats.session_tokens, Some(1100 + 53395 + 174 + 70760 + 2018 + 71000 + 80));
        let rate = stats.generation_tokens_per_second.unwrap();
        let expected = (174.0 + 2018.0) * 1000.0 / ((10558.0 - 7891.0) + (48690.0 - 2533.0));
        assert!((rate - expected).abs() < 1e-9);
    }

    #[test]
    fn turn_without_requests_reports_no_turn_figures() {
        let conn = store();
        conn.execute_batch("INSERT INTO turn_usage VALUES ('s1','t1',1),('s1','t2',10);").unwrap();
        request(&conn, "r1", "t1", 1, (1000, 100, 0), (Some(3000), Some(1000)));
        let stats = stats_from(&conn, "s1").unwrap();
        assert_eq!((stats.tokens, stats.input_tokens, stats.generation_tokens_per_second), (None, None, None));
        assert_eq!(stats.tools_used, Some(0));
        assert_eq!(stats.session_tokens, Some(1100));
    }

    #[test]
    fn unknown_session_is_empty() {
        assert_eq!(stats_from(&store(), "missing").unwrap(), TurnStats::default());
    }

    #[test]
    fn cache_outside_input_is_added_when_the_total_says_so() {
        let request = Request {
            turn_id: None, model: String::new(), input: 100, output: 10, cache_read: 900, cache_write: 0,
            total: Some(1010), duration_ms: None, first_token_ms: None,
        };
        assert_eq!(request.input(), 1000);
        assert_eq!(Request { total: Some(110), ..request }.input(), 100);
    }
}
