//! Running agents as protocol peers: one child process per session, and the timeline it produces.
//!
//! Each session owns a Claude stream-json or Codex app-server process, or an OpenCode server. Three threads
//! serve it: one reads stdout and turns every line into timeline rows (for OpenCode, a fourth follows the
//! server's event stream instead, see `opencode`), one drains stderr so a failing launch can say why, and a
//! third flushes rows to the frontend on a fixed cadence.
//!
//! **When the process exists.** It is started by the first message sent, not by opening the view, and it is
//! let go when the view closes — at once if it is idle, otherwise when the running turn and the queue
//! behind it are done (`attach` / `detach`). The conversation outlives the process: the timeline stays in
//! the table, and the next message resumes the agent's own recording. Codex cannot share one app-server
//! between sessions the way its desktop app does, because a thread it has loaded keeps a writer lock on
//! its rollout and the protocol has no request to unload one; the terminal engine's `codex resume` would
//! then fail. See `docs/design/chat引擎进程生命周期_20260905_0755.md`.
//!
//! **Why a flush cadence.** Text arrives token by token. Emitting an event per token would put thousands of
//! messages a minute through the same channel that serves every other client, so rows are marked dirty and
//! swept a few times a second. That is fast enough to read as live typing and slow enough to stay cheap.
//!
//! **Why rows are addressed by id.** The agent describes a message twice: first as fragments while it is
//! being written, then once more, complete, when it is done. Both carry the same message id and block index,
//! so both land on the same row and the complete version simply overwrites what the fragments built. Tool
//! calls work the same way: the call creates the row, and the result — which arrives later, in a separate
//! message — finds it again by tool id.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

mod codex;
mod opencode;

use super::codex_protocol;
use super::skills;
use super::config_schema::{self, ConfigKey};
use super::opencode_protocol;
use super::protocol::{self, ChatImage, Delta, DeltaKind, Incoming};
use crate::host::AppCtx;
use crate::models::SessionKind;
use crate::pty::{AgentState, StatusSignal};

/// Ids of nested Task cards from the root timeline down to the card that owns a child thread.
type RowPath = Vec<String>;

/// How often dirty rows are swept to the frontend. Short enough to read as live typing.
const FLUSH_INTERVAL: Duration = Duration::from_millis(60);
pub(crate) const SNAPSHOT_PAGE_ROWS: usize = 60;

/// How much of stderr to keep. Enough for a stack trace, bounded so a chatty CLI cannot grow without limit.
const STDERR_LIMIT: usize = 8 * 1024;

/// Milliseconds since the epoch, for the process facts the Info panel reports.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Event name carrying everything about one chat session.
pub fn event_name(session_id: &str) -> String {
    format!("chat://event/{session_id}")
}

// ─────────────────────────── Timeline ───────────────────────────

/// One row of the conversation.
///
/// Rows are what the view draws, so the shape is chosen for drawing: a tool call and its result are one row
/// because they are one card, and reasoning is separate from the answer because it is folded away separately.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChatRow {
    #[serde(rename_all = "camelCase")]
    User {
        id: String,
        text: String,
        /// Images sent with the message, kept so the bubble can show what was handed over. Left out of
        /// the wire when there are none, which is almost every row.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        images: Vec<ChatImage>,
        /// When it was said, in milliseconds since the epoch. Replayed rows carry the time the recording
        /// wrote down; live ones the time they arrived. `None` for a recording that never had one.
        #[serde(skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    Assistant {
        id: String,
        text: String,
        streaming: bool,
        /// The model reported for this reply, not the current session selection.
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
        /// Wall-clock time from submitting the turn to its terminal result. Present only after the turn
        /// finishes; the live counter uses `ChatSnapshot::turn_started_at` while work is still running.
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Reasoning { id: String, text: String, streaming: bool },
    #[serde(rename_all = "camelCase")]
    Tool {
        id: String,
        name: String,
        input: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        is_error: bool,
        /// `running` until its result arrives, then `completed` or `failed`.
        status: &'static str,
        /// Provider-owned facts about a subagent represented by this tool call.
        ///
        /// Claude's task protocol is the only reliable source for background children: they may never
        /// emit a sidechain frame. Keeping the facts separate from the tool's raw input preserves both
        /// the protocol payload and a stable presentation contract for the view.
        #[serde(skip_serializing_if = "Option::is_none")]
        subagent: Option<SubagentInfo>,
        /// What a subagent did inside this call, in the order it did it.
        ///
        /// Only a `Task` call ever has any. The agent reports a subagent's messages and tool calls as
        /// ordinary frames tagged with this call's id, so without somewhere of their own to go they would
        /// land in the main conversation and bury the answer the user is waiting for.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        children: Vec<ChatRow>,
    },
    #[serde(rename_all = "camelCase")]
    Error { id: String, message: String },
    /// What a slash command the agent answered itself printed back, such as `/effort` or `/context`.
    ///
    /// Its own row because it is neither side of the conversation: the agent did not say it, and drawing
    /// it as an answer would credit the model with words it never wrote.
    #[serde(rename_all = "camelCase")]
    Command { id: String, text: String },
    /// A remark about the conversation rather than a part of it — currently only that the replayed
    /// recording was cut short. Separate from `Error` because nothing went wrong and it must not read as
    /// a failure.
    #[serde(rename_all = "camelCase")]
    Notice { id: String, message: String },
    /// The conversation was summarized to make room in the context window.
    ///
    /// Worth a row of its own rather than a notice: it explains why the agent may not remember something
    /// said earlier, which is the one thing a reader needs from it. The wording is the view's, so the row
    /// carries only facts — `loading` while it runs, `completed` once the summary is in place.
    #[serde(rename_all = "camelCase")]
    Compaction {
        id: String,
        status: &'static str,
        /// `manual` when someone asked for it, `auto` when the window filled up.
        #[serde(skip_serializing_if = "Option::is_none")]
        trigger: Option<String>,
        /// How large the context was before the summary replaced it.
        #[serde(skip_serializing_if = "Option::is_none")]
        pre_tokens: Option<u64>,
    },
}

/// Compact, provider-neutral presentation facts for a subagent card.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) total_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_uses: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration_ms: Option<u64>,
}

impl ChatRow {
    pub fn id(&self) -> &str {
        match self {
            ChatRow::User { id, .. }
            | ChatRow::Assistant { id, .. }
            | ChatRow::Reasoning { id, .. }
            | ChatRow::Tool { id, .. }
            | ChatRow::Error { id, .. }
            | ChatRow::Command { id, .. }
            | ChatRow::Notice { id, .. }
            | ChatRow::Compaction { id, .. } => id,
        }
    }
}

/// The rows of one conversation, in order, addressable by id.
#[derive(Default)]
struct Timeline {
    revision: u64,
    reset_revision: u64,
    row_versions: HashMap<String, u64>,
    rows: Vec<ChatRow>,
    at: HashMap<String, usize>,
    /// Ids of rows changed since the last flush, in the order they changed.
    dirty: Vec<String>,
    /// A destructive timeline change must replace every client copy instead of being merged as row updates.
    replace_pending: bool,
}

enum TimelineFlush {
    None,
    Rows(Vec<ChatRow>),
    Replace(Vec<ChatRow>),
}

impl Timeline {
    /// Add a row, or replace the one that already carries this id.
    fn upsert(&mut self, row: ChatRow) {
        let id = row.id().to_string();
        match self.at.get(&id) {
            Some(&i) => self.rows[i] = row,
            None => {
                self.at.insert(id.clone(), self.rows.len());
                self.rows.push(row);
            }
        }
        self.revision += 1;
        self.row_versions.insert(id.clone(), self.revision);
        if !self.dirty.contains(&id) {
            self.dirty.push(id);
        }
    }

    /// Attach one turn's total duration to its last top-level answer.
    ///
    /// A turn may contain prose, tools, then more prose. Only the last answer carries the total so a single
    /// duration cannot look like several independent measurements. Child-agent answers stay inside their
    /// Task card and are deliberately excluded.
    fn finish_latest_turn(&mut self, duration_ms: u64) {
        let turn_start = self.rows.iter().rposition(|row| matches!(row, ChatRow::User { .. }));
        let Some(index) = self
            .rows
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, row)| {
                (turn_start.is_none_or(|start| index > start)
                    && matches!(row, ChatRow::Assistant { .. }))
                .then_some(index)
            })
        else {
            return;
        };
        let id = self.rows[index].id().to_string();
        if let ChatRow::Assistant { duration_ms: value, .. } = &mut self.rows[index] {
            *value = Some(duration_ms);
        }
        self.revision += 1;
        self.row_versions.insert(id.clone(), self.revision);
        if !self.dirty.contains(&id) {
            self.dirty.push(id);
        }
    }

    /// Add a row inside the tool call it belongs to, or replace the one already there.
    ///
    /// The parent is marked dirty rather than the child: rows travel whole, so a client redraws the card
    /// and everything under it in one go and never has to reassemble a subagent from separate pieces.
    ///
    /// A child can arrive before the call that owns it — the frames are separate lines and nothing
    /// orders them — so a missing parent is created rather than dropped. The name is a placeholder: the
    /// complete message describing the call follows and replaces it, whatever this CLI version calls
    /// its subagent tool.
    fn upsert_child(&mut self, parent_id: &str, child: ChatRow) {
        if self.at.get(parent_id).is_none() {
            self.upsert(ChatRow::Tool {
                id: parent_id.to_string(),
                name: "Task".to_string(),
                input: Value::Null,
                output: None,
                is_error: false,
                status: "running",
                subagent: None,
                children: Vec::new(),
            });
        }
        let Some(&i) = self.at.get(parent_id) else { return };
        if let ChatRow::Tool { children, .. } = &mut self.rows[i] {
            let id = child.id().to_string();
            match children.iter().position(|c| c.id() == id) {
                Some(at) => children[at] = child,
                None => children.push(child),
            }
        }
        let id = parent_id.to_string();
        self.revision += 1;
        self.row_versions.insert(id.clone(), self.revision);
        if !self.dirty.contains(&id) {
            self.dirty.push(id);
        }
    }

    /// Add or replace a row inside an arbitrarily nested Task card.
    ///
    /// Codex agents may themselves spawn agents. The complete path is retained so a grandchild's work is
    /// attached to its immediate parent rather than flattened into the first Task in the conversation.
    fn upsert_at(&mut self, parent_path: &[String], row: ChatRow) {
        if parent_path.is_empty() {
            self.upsert(row);
            return;
        }
        let root_id = parent_path[0].clone();
        let Some(children) = nested_children_mut(&mut self.rows, parent_path) else {
            return;
        };
        let id = row.id().to_string();
        match children.iter().position(|child| child.id() == id) {
            Some(index) => children[index] = row,
            None => children.push(row),
        }
        self.revision += 1;
        self.row_versions.insert(root_id.clone(), self.revision);
        if !self.dirty.contains(&root_id) {
            self.dirty.push(root_id);
        }
    }

    fn get(&self, id: &str) -> Option<&ChatRow> {
        self.at.get(id).map(|&i| &self.rows[i])
    }

    /// Top-level user messages in display order. Child-agent prompts are deliberately excluded.
    fn user_messages(&self) -> Vec<(String, String)> {
        self.rows
            .iter()
            .filter_map(|row| match row {
                ChatRow::User { id, text, .. } => Some((id.clone(), text.clone())),
                _ => None,
            })
            .collect()
    }

    /// Drop one user message and everything after it, then require a full client-side replacement.
    ///
    /// An unknown id is a hard no-op. Treating lookup failure as index zero would make a stale button look
    /// exactly like total conversation loss.
    fn trim_from_user(&mut self, id: &str) -> bool {
        let Some(index) = self.at.get(id).copied() else { return false };
        if !matches!(self.rows.get(index), Some(ChatRow::User { .. })) {
            return false;
        }
        self.rows.truncate(index);
        self.at.clear();
        for (index, row) in self.rows.iter().enumerate() {
            self.at.insert(row.id().to_string(), index);
        }
        self.dirty.clear();
        self.revision += 1;
        self.reset_revision = self.revision;
        self.row_versions.clear();
        self.replace_pending = true;
        true
    }

    /// Replace every row with a fresh reading of the conversation, and require a full client-side
    /// replacement. Used when the agent's own record is the authority — after a restore, or on resume.
    fn replace_all(&mut self, rows: Vec<ChatRow>) {
        self.rows = rows;
        self.at.clear();
        for (index, row) in self.rows.iter().enumerate() {
            self.at.insert(row.id().to_string(), index);
        }
        self.dirty.clear();
        self.revision += 1;
        self.reset_revision = self.revision;
        self.row_versions.clear();
        self.replace_pending = true;
    }

    /// Find a row where a frame with this parent would have put it: in the conversation, or inside the
    /// tool call named by `parent`. A subagent's calls carry ids of their own, so looking only at the top
    /// level would miss them and answer the result with an empty card.
    fn find(&self, parent: Option<&str>, id: &str) -> Option<&ChatRow> {
        match parent {
            None => self.get(id),
            Some(parent) => match self.get(parent) {
                Some(ChatRow::Tool { children, .. }) => children.iter().find(|c| c.id() == id),
                _ => None,
            },
        }
    }

    fn find_at(&self, parent_path: &[String], id: &str) -> Option<&ChatRow> {
        if parent_path.is_empty() {
            return self.get(id);
        }
        nested_children(&self.rows, parent_path)?.iter().find(|row| row.id() == id)
    }

    /// Update a top-level subagent card without discarding its tool input, result, or child timeline.
    fn update_subagent(
        &mut self,
        id: &str,
        patch: impl FnOnce(&mut SubagentInfo),
        status: Option<&'static str>,
        output: Option<String>,
    ) {
        if self.at.get(id).is_none() {
            self.upsert(ChatRow::Tool {
                id: id.to_string(),
                name: "Task".to_string(),
                input: Value::Null,
                output: None,
                is_error: false,
                status: "running",
                subagent: Some(SubagentInfo::default()),
                children: Vec::new(),
            });
        }
        let Some(&i) = self.at.get(id) else { return };
        if let ChatRow::Tool {
            output: current_output,
            is_error,
            status: current_status,
            subagent,
            ..
        } = &mut self.rows[i]
        {
            let facts = subagent.get_or_insert_with(SubagentInfo::default);
            patch(facts);
            if let Some(next) = status {
                *current_status = next;
                *is_error = next == "failed";
            }
            if output.is_some() {
                *current_output = output;
            }
        }
        let id = id.to_string();
        self.revision += 1;
        self.row_versions.insert(id.clone(), self.revision);
        if !self.dirty.contains(&id) {
            self.dirty.push(id);
        }
    }

    /// Update a Task addressed by its complete nesting path.
    fn update_subagent_at(
        &mut self,
        path: &[String],
        patch: impl FnOnce(&mut SubagentInfo),
        status: Option<&'static str>,
        output: Option<String>,
    ) {
        let Some((id, parent_path)) = path.split_last() else { return };
        if parent_path.is_empty() {
            self.update_subagent(id, patch, status, output);
            return;
        }
        let root_id = parent_path[0].clone();
        let Some(row) = nested_children_mut(&mut self.rows, parent_path)
            .and_then(|children| children.iter_mut().find(|row| row.id() == id))
        else {
            return;
        };
        if let ChatRow::Tool {
            output: current_output,
            is_error,
            status: current_status,
            subagent,
            ..
        } = row
        {
            let facts = subagent.get_or_insert_with(SubagentInfo::default);
            patch(facts);
            if let Some(next) = status {
                *current_status = next;
                *is_error = next == "failed";
            }
            if output.is_some() {
                *current_output = output;
            }
        }
        self.revision += 1;
        self.row_versions.insert(root_id.clone(), self.revision);
        if !self.dirty.contains(&root_id) {
            self.dirty.push(root_id);
        }
    }

    fn take_dirty(&mut self) -> Vec<ChatRow> {
        let ids = std::mem::take(&mut self.dirty);
        ids.iter()
            .filter_map(|id| self.at.get(id).map(|&i| self.rows[i].clone()))
            .collect()
    }

    fn take_flush(&mut self) -> TimelineFlush {
        if self.replace_pending {
            self.replace_pending = false;
            self.dirty.clear();
            return TimelineFlush::Replace(self.rows.clone());
        }
        let rows = self.take_dirty();
        if rows.is_empty() {
            TimelineFlush::None
        } else {
            TimelineFlush::Rows(rows)
        }
    }
}

fn nested_children<'a>(rows: &'a [ChatRow], path: &[String]) -> Option<&'a [ChatRow]> {
    let (id, tail) = path.split_first()?;
    let row = rows.iter().find(|row| row.id() == id)?;
    let ChatRow::Tool { children, .. } = row else { return None };
    if tail.is_empty() {
        Some(children)
    } else {
        nested_children(children, tail)
    }
}

fn nested_children_mut<'a>(
    rows: &'a mut [ChatRow],
    path: &[String],
) -> Option<&'a mut Vec<ChatRow>> {
    let (id, tail) = path.split_first()?;
    let row = rows.iter_mut().find(|row| row.id() == id)?;
    let ChatRow::Tool { children, .. } = row else { return None };
    if tail.is_empty() {
        Some(children)
    } else {
        nested_children_mut(children, tail)
    }
}

/// A message typed while the agent was busy, waiting for the turn in progress to end.
///
/// It is not a timeline row: nothing has been said yet, and it can still be edited or dropped. It becomes
/// a `User` row at the moment it is actually written to the agent.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedMessage {
    pub id: String,
    pub text: String,
    /// Images attached to it, waiting along with the text.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ChatImage>,
}

/// A saved permission choice that differs from the last policy accepted by Codex.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingPermissionMode {
    pub current: String,
    pub next: String,
}

#[derive(Default)]
struct CodexPermissionState {
    applied: Option<String>,
    /// Retain each request's policy: the selection may change again before its acknowledgement.
    requests: HashMap<String, String>,
}

/// Optional bounded synchronization request. Missing options preserve the legacy full snapshot.
#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatWindow {
    pub before: Option<String>,
    pub from: Option<String>,
    pub since: Option<u64>,
    pub epoch: Option<u64>,
}

/// A conversation's full state, as handed to a client that just connected or reopened the view.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSnapshot {
    pub positions: HashMap<String, usize>,
    pub page_kind: &'static str,
    pub has_more: bool,
    pub total_rows: usize,
    pub submission_receipts: bool,
    pub rows_revision: u64,
    pub queue_revision: u64,
    pub rewind_scopes: Vec<&'static str>,
    pub running: bool,
    /// Timeline rows with image bytes replaced by stable attachment references.
    ///
    /// A pane resolves a reference only when it needs the image. This keeps a snapshot proportional to
    /// the conversation text instead of retransmitting several megabytes of Base64 to every observer.
    #[serde(serialize_with = "serialize_snapshot_rows")]
    pub rows: Vec<ChatRow>,
    /// Messages waiting for the turn in progress to finish, in the order they will be sent.
    #[serde(serialize_with = "serialize_snapshot_queue")]
    pub queue: Vec<QueuedMessage>,
    /// Permission questions still waiting for an answer.
    pub permissions: Vec<Value>,
    /// Slash commands and skills this agent offers, as reported at startup.
    pub commands: Vec<Value>,
    /// Settings `/config` accepts, for completing it.
    pub config_keys: Vec<ConfigKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// A saved pair, including explicit automatic values, for a conversation without a running peer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<crate::agent::session_settings::Selection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_permission_mode: Option<PendingPermissionMode>,
    /// Codex collaboration style, independent of its filesystem/approval mode above.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collaboration_mode: Option<String>,
    /// Presets reported by the running Codex app-server. Empty for Claude and unsupported Codex versions.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub collaboration_modes: Vec<codex_protocol::CollaborationModePreset>,
    /// Codex service tier and personality chosen for this conversation, when they differ from the defaults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<String>,
    /// The agent process behind this conversation, and when it started.
    ///
    /// A chat session has no PTY, so nothing else in the product knows there is a process at all: the
    /// Info panel drew "started", "uptime" and this session's CPU from the PTY's record and had none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Milliseconds since the epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    /// When the current turn began, in milliseconds since the epoch. The client uses this authoritative
    /// instant to redraw only the small elapsed-time label once per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_started_at: Option<u64>,
    /// What the Claude process reports about itself beyond the conversation. Empty for other agents.
    #[serde(flatten)]
    pub extras: ClaudeExtras,
}

/// Statuses after which a task reports nothing more. "ended" is synthesized when the inventory drops a task
/// before (or without) a terminal frame naming it.
const FINISHED_TASK_STATUSES: [&str; 8] = ["completed", "failed", "error", "killed", "stopped", "canceled", "cancelled", "ended"];

/// How many finished tasks stay listed until the next turn prunes them.
const FINISHED_TASK_CAP: usize = 20;

/// How long a release waits for the terminal frames of a task the inventory just dropped.
///
/// Claude empties `background_tasks_changed` first and sends `task_updated` and `task_notification` (final
/// status, summary, output file) afterwards. A task tab that outlives its conversation pane shows exactly
/// those, so the process is not killed on the drop itself; a task whose frames never come is let go after
/// this grace so nothing leaks.
const TASK_SETTLE_GRACE: Duration = Duration::from_secs(2);

/// One background task as the composer and the task tab see it: the inventory entry merged with every
/// task-protocol frame that named the same task_id. Wire keys stay Claude's snake_case.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct BackgroundTask {
    pub task_id: String,
    /// "local_workflow", "local_agent", "local_bash"; empty until a frame names it.
    pub task_type: String,
    /// Live: `task_progress.description` ("Phase: agent" for workflows) wins over the inventory's static text.
    pub description: String,
    /// The static description from `task_progress.summary`, or the final summary of `task_notification`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// "running", one of Claude's terminal values, or the synthesized "ended".
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_tool_name: Option<String>,
    /// Stamped here on the first frame; the protocol carries no task-level start time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<u64>,
    /// `{total_tokens, tool_uses, duration_ms}` as last reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Value>,
    /// The workflow's phase/agent tree, kept from the last frame that carried one (about every second frame).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_progress: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file: Option<String>,
    /// Seen in a `background_tasks_changed` inventory; only listed tasks are published.
    #[serde(skip)]
    pub listed: bool,
}

impl BackgroundTask {
    fn finished(&self) -> bool {
        FINISHED_TASK_STATUSES.contains(&self.status.as_str())
    }
}

/// Live facts the Claude protocol reports outside the conversation, shown by the composer.
///
/// Sent whole on every change under one `extras` event: a client that connects late reads the same
/// object from the snapshot, and nobody has to merge partial updates.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeExtras {
    /// Context the last API call carried: input plus cache reads and cache writes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u64>,
    /// The window that context is counted against, as reported for the model that answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// Running total for this process, in US dollars, as the agent itself accounts it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_usd: Option<f64>,
    /// The `get_context_usage` answer after the last turn: categories and the compaction-aware maximum.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_usage: Option<Value>,
    /// The subscription rate-limit windows as last reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<Value>,
    /// Background tasks merged per task_id from the inventory and the task protocol; finished ones stay
    /// listed with their final state until the next turn starts. Foreground subagents are held here too
    /// but only published once an inventory names them (see `published`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub background_tasks: Vec<BackgroundTask>,
    /// Whether fast mode was switched on for this process.
    pub fast_mode: bool,
    /// A failed API call being retried right now; cleared by the next frame that says the call went through.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_retry: Option<Value>,
    /// The model named by the last assistant frame, which is the key into `modelUsage` at turn end.
    #[serde(skip)]
    pub usage_model: Option<String>,
}

impl ClaudeExtras {
    /// The view clients receive: every task the inventory has named, finished or not. Tasks known only
    /// from `task_*` frames are foreground work and stay out until `backgroundAll` lists them.
    fn published(&self) -> ClaudeExtras {
        let mut extras = self.clone();
        extras.background_tasks.retain(|task| task.listed);
        extras
    }
}

// ─────────────────────────── Sessions ───────────────────────────

struct ChatProcess {
    kind: SessionKind,
    cwd: Option<String>,
    action: Mutex<()>,
    stdin: Mutex<Option<ChildStdin>>,
    child: Mutex<Child>,
    timeline: Mutex<Timeline>,
    /// Permission questions waiting for an answer, keyed by the agent's request id.
    permissions: Mutex<HashMap<String, Value>>,
    commands: Mutex<Vec<Value>>,
    /// Settings the agent accepts, so the composer can complete `/config`. Filled in the background.
    config_keys: Mutex<Vec<ConfigKey>>,
    /// Claude task id to the canonical Task tool-use id announced by `task_started`.
    subagent_ids: Mutex<HashMap<String, String>>,
    /// Codex child thread id to the complete Task path that owns its visible activity.
    codex_subagent_ids: Mutex<HashMap<String, RowPath>>,
    /// Child notifications can race ahead of the collab item that names their parent. Keep a small,
    /// bounded backlog until the route is known instead of leaking them into the main conversation.
    ///
    /// OpenCode's child sessions use the same two tables: its subagents run in sessions of their own, and
    /// their events arrive before the `task` call names them just as Codex's do.
    codex_pending_child_events: Mutex<HashMap<String, Vec<(String, Value)>>>,
    /// The OpenCode server behind this conversation and what the engine has learned from it.
    opencode: Mutex<opencode::OpencodeState>,
    /// Codex service tier (`fast`, or another id from its model catalogue) for subsequent turns. None
    /// keeps whatever the thread was opened with.
    service_tier: Mutex<Option<String>>,
    /// Codex personality (`none`, `friendly`, `pragmatic`) for subsequent turns; None keeps the configured one.
    personality: Mutex<Option<String>>,
    /// Why the running Codex turn is about to fail: the `error` notification arrives before the
    /// `turn/completed` that ends the turn, and the reason is shown once, when it does.
    codex_turn_error: Mutex<Option<String>>,
    /// See `ClaudeExtras`.
    extras: Mutex<ClaudeExtras>,
    /// The catalogue Claude answered `list_models` with, as sent. Empty until it answers.
    claude_models: Mutex<Vec<Value>>,
    agent_session_id: Mutex<Option<String>>,
    /// Serialize settings requests so acknowledgements cannot commit out of order.
    settings_change: Mutex<()>,
    model: Mutex<Option<String>>,
    effort: Mutex<Option<String>>,
    mode: Mutex<String>,
    /// Selected permissions are independent of the policy accepted by the active Codex turn.
    codex_permission_state: Mutex<CodexPermissionState>,
    /// `default` or `plan` for Codex. Kept separate from `mode`, which controls approvals and sandboxing.
    collaboration_mode: Mutex<Option<String>>,
    /// Installed app-server's authoritative collaboration preset catalogue.
    collaboration_modes: Mutex<Vec<codex_protocol::CollaborationModePreset>>,
    /// Native Codex turn id. Claude's control protocol does not require one.
    current_turn: Mutex<Option<String>>,
    /// Codex cannot accept turns until initialize and thread start/resume have both answered.
    ready: AtomicBool,
    /// Text accumulated per streaming block, keyed by row id.
    buffers: Mutex<HashMap<String, String>>,
    /// Id of the message currently being written. Fragments do not carry it, and without it the rows they
    /// build would be addressed differently from the complete message — leaving the answer on screen twice.
    current_message: Mutex<Option<String>>,
    /// How many content blocks each message has delivered in complete frames so far.
    ///
    /// The agent does not send a finished message as one frame. It sends one frame per block, as each
    /// block completes — first `[thinking]`, then `[text]` — every frame carrying the same message id and
    /// a `content` array that starts at zero. Numbering each frame's blocks from zero would give the text
    /// the thinking's row id and overwrite it; the true index is how many blocks came before.
    frame_blocks: Mutex<HashMap<String, usize>>,
    /// Ids of our own outstanding control requests, so their answers can be routed.
    pending: Mutex<HashMap<String, &'static str>>,
    /// Blocking command calls waiting for one control/JSON-RPC response from the stdout reader.
    waiters: Mutex<HashMap<String, mpsc::Sender<Result<Value, String>>>>,
    /// View row id to the provider-native user message/turn it represents.
    user_targets: Mutex<HashMap<String, super::history::RewindTarget>>,
    /// Codex user rows sent before `turn/started` reveals their native turn id.
    pending_user_rows: Mutex<Vec<String>>,
    /// The turn in progress and whatever is waiting behind it.
    turn: Mutex<TurnQueue>,
    stderr: Mutex<String>,
    alive: AtomicBool,
    /// A view let go of this conversation while a turn was running: stop as soon as it is idle.
    release_when_idle: AtomicBool,
    /// This process was stopped on purpose. The exit that follows is then reported as asked-for, so no
    /// client draws it as the agent failing.
    released: AtomicBool,
    next_request: AtomicU64,
    /// The agent process, so the Info panel can report the same facts it reports for a terminal session.
    pid: u32,
    /// When it started, in milliseconds since the epoch.
    started_at: u64,
    /// How many times this conversation has been compacted, which is what addresses the marker rows.
    ///
    /// The row is drawn twice — once while the summary is being written, once when it is in place — and
    /// both have to land on the same row, so the count only moves when a compaction finishes.
    compactions: AtomicU64,
}

/// Whether a turn is running, and the messages typed while it was.
///
/// Both live under one lock on purpose. Deciding what to do with a new message means reading the first and
/// writing the second, and if the turn could end in between, a message meant for the queue would sit there
/// with nothing left to wake it.
#[derive(Default)]
struct TurnQueue {
    revision: u64,
    /// Tasks announced through Claude's lifecycle stream, including foreground tasks moved to the background.
    active_tasks: HashSet<String>,
    /// The most recent background-task inventory, also covering tasks without a task_started frame.
    background_tasks: HashSet<String>,
    /// Tasks the inventory dropped whose `task_notification` has not arrived yet, with the instant the
    /// release stops waiting for it (see `TASK_SETTLE_GRACE`).
    settling: HashMap<String, std::time::Instant>,
    /// True from the moment a user turn is written until its `result` line arrives.
    running: bool,
    /// Messages waiting for that turn to end, in the order they will be sent.
    waiting: Vec<QueuedMessage>,
    /// Pause automatic queue draining while native steering acceptance is pending.
    steering: bool,
    /// Whether the running turn was stopped on purpose. The agent reports an interrupted turn as a failed
    /// one, and a failure the user asked for must not be drawn as an error.
    interrupted: bool,
    /// Start of the running logical turn. Steering adds another user row but does not reset this clock.
    started_at: Option<u64>,
}

impl ChatProcess {
    fn permission_modes(&self) -> (String, Option<PendingPermissionMode>) {
        let selected = self.mode.lock().unwrap();
        if self.kind != SessionKind::Codex {
            return (selected.clone(), None);
        }
        let pending = self.codex_permission_state
            .lock()
            .unwrap()
            .applied
            .as_ref()
            .filter(|current| *current != &*selected)
            .map(|current| PendingPermissionMode { current: current.clone(), next: selected.clone() });
        (selected.clone(), pending)
    }

    /// Record the policy before writing so even an immediate acknowledgement uses the sent value.
    fn write_codex_mode_request(&self, id: &str, mode: &str, request: &Value) -> Result<(), String> {
        self.codex_permission_state.lock().unwrap().requests.insert(id.to_string(), mode.to_string());
        let result = self.write(request);
        if result.is_err() {
            self.codex_permission_state.lock().unwrap().requests.remove(id);
        }
        result
    }

    fn write(&self, value: &Value) -> Result<(), String> {
        let mut guard = self.stdin.lock().unwrap();
        let stdin = guard.as_mut().ok_or("The agent is not accepting input")?;
        let line = format!("{value}\n");
        stdin
            .write_all(line.as_bytes())
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("Failed to send to the agent: {e}"))
    }

    fn request_id(&self, kind: &'static str) -> String {
        let n = self.next_request.fetch_add(1, Ordering::Relaxed);
        let id = format!("vlx-{kind}-{n}");
        self.pending.lock().unwrap().insert(id.clone(), kind);
        id
    }

    fn request_and_wait(
        &self,
        kind: &'static str,
        make_request: impl FnOnce(&str) -> Value,
    ) -> Result<Value, String> {
        let id = self.request_id(kind);
        let (tx, rx) = mpsc::channel();
        self.waiters.lock().unwrap().insert(id.clone(), tx);
        if let Err(error) = self.write(&make_request(&id)) {
            self.waiters.lock().unwrap().remove(&id);
            self.pending.lock().unwrap().remove(&id);
            return Err(error);
        }
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(answer) => answer,
            Err(error) => {
                self.waiters.lock().unwrap().remove(&id);
                self.pending.lock().unwrap().remove(&id);
                Err(match error {
                    mpsc::RecvTimeoutError::Timeout => {
                        format!("The agent did not answer the {} request", kind.replace('_', " "))
                    }
                    mpsc::RecvTimeoutError::Disconnected => {
                        format!("The agent exited before answering the {} request", kind.replace('_', " "))
                    }
                })
            }
        }
    }
}

fn initial_mode(kind: SessionKind, stored: Option<&str>) -> String {
    match kind {
        SessionKind::Codex => match stored.map(str::trim) {
            Some("read-only") => "read-only",
            Some("full-access") | Some("skip") => "full-access",
            _ => "auto",
        },
        SessionKind::Opencode => opencode::initial_mode(stored),
        _ => protocol::cli_permission_mode(stored).unwrap_or("default"),
    }
    .to_string()
}

/// All chat-engine sessions, keyed by VelaTerm session id.
#[derive(Default)]
pub struct ChatManager {
    sessions: Mutex<HashMap<String, Arc<ChatProcess>>>,
}

impl ChatManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a live process is behind this conversation right now.
    ///
    /// Settings changed while there is none are stored for the next launch instead of being sent, so a
    /// mode picked before the first message, or after the agent stopped, is not an error.
    pub fn is_alive(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .is_some_and(|proc| proc.alive.load(Ordering::Relaxed))
    }

    fn get(&self, session_id: &str) -> Result<Arc<ChatProcess>, String> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .cloned()
            .ok_or_else(|| "This session has no running agent".to_string())
    }

    /// Start an agent for this session, or do nothing if one is already running.
    ///
    /// `resume` continues an existing conversation — including one recorded by the same session running as a
    /// TUI, since both engines write the same file.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &self,
        app: &AppCtx,
        session_id: &str,
        kind: SessionKind,
        cwd: Option<&str>,
        bin: &str,
        resume: Option<&str>,
        model: Option<&str>,
        effort: Option<&str>,
        permission_mode: Option<&str>,
        collaboration_mode: Option<&str>,
        // The session's own launch arguments, as typed in its settings. Appended last, so a flag written
        // there overrides the one this engine would otherwise pass.
        extra_args: &[String],
        // Claude only: switch fast mode on once the handshake completes.
        fast_mode: bool,
    ) -> Result<(), String> {
        {
            let running = self.sessions.lock().unwrap();
            if running.get(session_id).is_some_and(|p| p.alive.load(Ordering::Relaxed)) {
                return Ok(());
            }
        }

        // Native 1M models such as Opus 5 have no second selectable `[1m]` entry in Paseo, so old saved
        // spellings must follow the same canonical path.
        let model = if kind == SessionKind::Claude {
            model.map(crate::agent::claude_models::normalize_id)
        } else {
            model
        };

        let mut cmd = Command::new(bin);
        // OpenCode is reached over HTTP: the port it listens on and the password that guards it.
        let mut opencode_launch: Option<(u16, String)> = None;
        match kind {
            SessionKind::Claude => {
                cmd.args(protocol::launch_args(
                    resume,
                    model,
                    effort,
                    protocol::cli_permission_mode(permission_mode),
                ));
                cmd.args(extra_args);
            }
            SessionKind::Codex => {
                cmd.arg("app-server").arg("--stdio");
                cmd.args(crate::agent::codex_models::app_server_args(extra_args));
            }
            SessionKind::Opencode => {
                // The session's launch arguments belong to its terminal interface (`--agent`, `-m`); the
                // server accepts none of them, and the same choices reach it as fields of each turn.
                let port = opencode_protocol::configured_port(app, session_id)?;
                let password = opencode_protocol::random_password();
                cmd.arg("serve")
                    .arg("--port")
                    .arg(port.to_string())
                    .arg("--hostname")
                    .arg("127.0.0.1");
                cmd.env("OPENCODE_SERVER_PASSWORD", &password);
                cmd.env("OPENCODE_SERVER_USERNAME", "opencode");
                // A terminal session injects its own status plugin through this variable; it would report
                // that session's identity on top of what this engine reports itself.
                cmd.env_remove(crate::agent::inject::OPENCODE_CONFIG_ENV);
                opencode_launch = Some((port, password));
            }
            _ => return Err(format!("The chat engine does not support {} sessions", kind.as_str())),
        }
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        // The same reasoning as for PTY sessions: an agent we start is a session of its own, never a
        // continuation of whatever harness happened to launch VelaTerm.
        for key in crate::pty::manager::AGENT_HARNESS_MARKERS {
            cmd.env_remove(key);
        }
        // Give the agent this instance's identity, the same way pty/manager.rs does for a terminal. `vspawn`
        // and `vopen` read these to reach the local `/spawn` and `/view` endpoints, and their PATH shims call
        // `$VLX_EXE`. Without this the child inherits whatever the backend was started with: nothing when
        // it was launched from a plain shell, and another instance's session when it was launched from
        // inside a VelaTerm session — the spawn card then appears in that instance, under that session.
        let hook = app.hooks().endpoint();
        cmd.env("VLX_SESSION_ID", session_id);
        cmd.env("VLX_TOKEN", &hook.token);
        cmd.env("VLX_SPAWN_URL", format!("http://127.0.0.1:{}", hook.port));
        if let Ok(exe) = std::env::current_exe() {
            cmd.env("VLX_EXE", exe.as_os_str());
        }
        if let Ok(data_dir) = app.data_dir() {
            let bin = crate::agent::spawn_cli::bin_dir(&data_dir);
            let existing = crate::appimage::clean_var("PATH").unwrap_or_default();
            let sep = if cfg!(windows) { ';' } else { ':' };
            cmd.env("PATH", format!("{}{sep}{existing}", bin.display()));
            cmd.env("VLX_BIN_DIR", bin.as_os_str());
        }
        // Per-session values a terminal session injects for its own hooks. They belong to whichever session
        // the backend was started from, not to this one, and the chat engine does not use them.
        for key in [
            crate::agent::inject::CLAUDE_SETTINGS_ENV,
            crate::agent::inject::NOTFOUND_URL_ENV,
        ] {
            cmd.env_remove(key);
        }
        if kind == SessionKind::Claude {
            // Claude disables file checkpoints outside its TUI unless this SDK-compatible switch is set.
            // Its explicit disable switch still wins, so user policy remains authoritative.
            cmd.env("CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING", "1");
        }
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

        let (service_tier, personality) = {
            let conn = app.db().conn.lock().unwrap();
            crate::db::repo::codex_chat_settings(&conn, session_id)?
        };

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start the agent: {e}"))?;
        let pid = child.id();
        let stdin = child.stdin.take().ok_or("The agent has no input stream")?;
        let stdout = child.stdout.take().ok_or("The agent has no output stream")?;
        let stderr = child.stderr.take();

        let proc = Arc::new(ChatProcess {
            kind,
            action: Mutex::new(()),
            cwd: cwd.map(str::to_string),
            stdin: Mutex::new(Some(stdin)),
            child: Mutex::new(child),
            timeline: Mutex::new(Timeline::default()),
            permissions: Mutex::new(HashMap::new()),
            commands: Mutex::new(Vec::new()),
            config_keys: Mutex::new(Vec::new()),
            subagent_ids: Mutex::new(HashMap::new()),
            codex_subagent_ids: Mutex::new(HashMap::new()),
            codex_pending_child_events: Mutex::new(HashMap::new()),
            opencode: Mutex::new(opencode::OpencodeState::default()),
            service_tier: Mutex::new(service_tier),
            personality: Mutex::new(personality),
            codex_turn_error: Mutex::new(None),
            extras: Mutex::new(ClaudeExtras { fast_mode, ..ClaudeExtras::default() }),
            claude_models: Mutex::new(Vec::new()),
            agent_session_id: Mutex::new(if matches!(kind, SessionKind::Codex | SessionKind::Opencode) {
                resume.map(str::to_string)
            } else {
                None
            }),
            settings_change: Mutex::new(()),
            model: Mutex::new(model.map(str::to_string)),
            effort: Mutex::new(effort.map(str::to_string)),
            mode: Mutex::new(initial_mode(kind, permission_mode)),
            codex_permission_state: Mutex::new(CodexPermissionState::default()),
            collaboration_mode: Mutex::new(match kind {
                SessionKind::Codex => Some(
                    match collaboration_mode.map(str::trim) {
                        Some("plan") => "plan",
                        _ => "default",
                    }
                    .to_string(),
                ),
                // An OpenCode agent name; checked against the server's catalogue once it answers.
                SessionKind::Opencode => collaboration_mode
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_string),
                _ => None,
            }),
            collaboration_modes: Mutex::new(Vec::new()),
            current_turn: Mutex::new(None),
            ready: AtomicBool::new(kind == SessionKind::Claude),
            buffers: Mutex::new(HashMap::new()),
            current_message: Mutex::new(None),
            frame_blocks: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            waiters: Mutex::new(HashMap::new()),
            user_targets: Mutex::new(HashMap::new()),
            pending_user_rows: Mutex::new(Vec::new()),
            turn: Mutex::new(TurnQueue::default()),
            stderr: Mutex::new(String::new()),
            alive: AtomicBool::new(true),
            release_when_idle: AtomicBool::new(false),
            released: AtomicBool::new(false),
            next_request: AtomicU64::new(1),
            pid,
            started_at: now_ms(),
            compactions: AtomicU64::new(0),
        });

        // This timeline is new, so a client still showing the previous process's rows has to drop them.
        // Without that, replayed history lands underneath rows describing the same messages and the
        // conversation reads as if everything happened twice. Emitted before the flusher exists, so no row
        // can arrive ahead of it.
        emit(app, session_id, json!({"type":"reset","epoch":proc.started_at}));

        // A resumed conversation lives only in the agent's memory; this timeline starts empty. Without
        // replaying the recording the view would sit blank in front of an agent that remembers every word
        // of it. Reading happens here, before any client can take a snapshot, so nobody sees the gap.
        if let Some(id) = resume {
            match super::history::replay(kind, id) {
                Ok(rows) => {
                    let mut timeline = proc.timeline.lock().unwrap();
                    timeline.replace_all(rows);
                }
                // A recording that was deleted, or belongs to a conversation held elsewhere, is no reason
                // to refuse the session: the agent still has the context, only the view starts empty.
                Err(e) => eprintln!("chat: no history replayed for {id}: {e}"),
            }
        }

        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.to_string(), proc.clone());

        // Say which process is behind this conversation. The snapshot carries the same two facts, but a
        // pane that opened before the agent started has already read it, and nothing else in the product
        // can see a process that has no PTY.
        emit(
            app,
            session_id,
            json!({"type":"process","pid":proc.pid,"startedAt":proc.started_at,"rewindScopes":rewind_scopes(proc.kind)}),
        );

        // Say which agent is running here, the same way a typed PTY spawn does. Nothing else says it for
        // a chat session, and without it the authoritative record holds no agent — so the work state that
        // follows is never read, and the sidebar dot, the tab marker and this pane's own "working" line
        // all stay silent. A session that had first run in a terminal was carrying the PTY's answer.
        app.emit(
            &StatusSignal::event_name(session_id),
            StatusSignal::Agent { agent: Some(kind.as_str().to_string()), state_source: None },
        );

        // Report the session as running, the same way a PTY does. Everything downstream — the sidebar, and
        // in particular whether a client mounts this session at all — follows that one flag, and a session
        // reported dead has its pane replaced by a placeholder.
        crate::session_state::set_alive(app, session_id, true);

        spawn_stdout_reader(app.clone(), session_id.to_string(), proc.clone(), stdout);
        if let Some(stderr) = stderr {
            spawn_stderr_reader(proc.clone(), stderr);
        }
        spawn_flusher(app.clone(), session_id.to_string(), proc.clone());

        let init_id = proc.request_id("initialize");
        match kind {
            SessionKind::Claude => {
                // Ask for the command catalogue straight away; the composer's completions come from the answer.
                let _ = proc.write(&protocol::control_request(&init_id, protocol::initialize()));
                // Ask a throwaway process which settings this agent accepts. It answers locally, at no token
                // cost, and asking on the live conversation would print usage text into the transcript.
                spawn_config_lookup(
                    app.clone(),
                    session_id.to_string(),
                    proc.clone(),
                    bin.to_string(),
                    cwd.map(str::to_string),
                );
            }
            SessionKind::Codex => {
                if let Err(message) = proc.write(&codex_protocol::initialize(&init_id)) {
                    let _ = self.stop(app, session_id);
                    return Err(message);
                }
            }
            SessionKind::Opencode => {
                let (port, password) = opencode_launch.expect("an OpenCode launch reserves its port");
                opencode::bootstrap(
                    app.clone(),
                    session_id.to_string(),
                    proc.clone(),
                    port,
                    password,
                    resume.map(str::to_string),
                );
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    /// The OpenCode server behind a running session, for callers that need its catalogues.
    pub fn opencode_server(&self, session_id: &str) -> Option<Arc<opencode_protocol::Server>> {
        let proc = self.sessions.lock().unwrap().get(session_id).cloned()?;
        if !proc.alive.load(Ordering::Relaxed) {
            return None;
        }
        let state = proc.opencode.lock().unwrap();
        state.server.clone()
    }

    /// Send a user turn, or decide what to do with it when one is already running.
    ///
    /// Nothing is in flight: the message goes straight out, which is what happens whenever the agent is
    /// idle, whatever behavior was asked for. While a turn is running the caller chooses:
    ///
    /// - `queue` (the default) holds it until the running turn reports its result, then sends it. This is
    ///   what someone who types a follow-up while the agent works almost always means.
    /// - `interrupt` stops the running turn first and sends this message as the next one, so the agent
    ///   drops what it was doing and takes the new instruction.
    /// - `steer` writes it into the running turn. The agent reads it at its next step and answers it
    ///   together with the turn already under way — one result for both, verified against claude 2.1.258.
    ///
    /// `images` ride along with the text as part of the same turn, and wait with it when it is queued.
    ///
    /// Returns `"sent"` when the message went out and `"queued"` when it is waiting.
    pub fn send(
        &self,
        app: &AppCtx,
        session_id: &str,
        text: &str,
        images: Vec<ChatImage>,
        behavior: &str,
    ) -> Result<&'static str, String> {
        self.send_identified(app, session_id, text, images, behavior, None)
    }

    pub fn send_identified(
        &self, app: &AppCtx, session_id: &str, text: &str,
        images: Vec<ChatImage>, behavior: &str, message_id: Option<&str>,
    ) -> Result<&'static str, String> {
        let proc = self.get(session_id)?;
        let _action = proc.action.lock().unwrap();
        // Steering is a prompt, never a local command or a queued replacement.
        if proc.kind == SessionKind::Opencode && behavior != "steer" && proc.ready.load(Ordering::Relaxed) {
            if let Some(handled) = opencode::local_command(app, session_id, &proc, text)? {
                return Ok(if message_id.is_some() { "command" } else { handled });
            }
        }
        let mut turn = proc.turn.lock().unwrap();
        if behavior == "steer" {
            if !proc.ready.load(Ordering::Relaxed) || !turn.running || turn.interrupted {
                return Err("There is no running turn available to steer. Send the message normally or retry when the agent is working.".into());
            }
            drop(turn);
            dispatch_steer(app, session_id, &proc, text, &images, message_id)
                .map_err(|error| if message_id.is_some() { "chat_submission_pending".into() } else { error })?;
            return Ok("sent");
        }
        if !proc.ready.load(Ordering::Relaxed) {
            let item = QueuedMessage {
                id: message_id.map(str::to_owned).unwrap_or_else(|| format!("q-{}", proc.next_request.fetch_add(1, Ordering::Relaxed))),
                text: text.to_string(),
                images,
            };
            turn.waiting.push(item);
            drop(turn);
            emit_queue(app, session_id, &proc);
            return Ok("queued");
        }
        if turn.running {
            let item = QueuedMessage {
                id: message_id.map(str::to_owned).unwrap_or_else(|| format!("q-{}", proc.next_request.fetch_add(1, Ordering::Relaxed))),
                text: text.to_string(),
                images,
            };
            // Interrupting means "do this instead of what you are doing", so this message goes ahead of
            // anything already waiting rather than behind it.
            if behavior == "interrupt" {
                turn.waiting.insert(0, item);
            } else {
                turn.waiting.push(item);
            }
            // Stopping the turn happens under the same lock that queued the message. Letting go first
            // would leave a gap in which the turn ends on its own and sends this very message — and the
            // interrupt, arriving a moment later, would kill the turn it was meant to make room for.
            let stopped = if behavior == "interrupt" {
                interrupt_locked(&proc, &mut turn)
            } else {
                Ok(())
            };
            drop(turn);
            emit_queue(app, session_id, &proc);
            // The prompt is already queued even if stopping the previous turn failed.
            // An identified retry must not enqueue it again under a new identifier.
            stopped.map_err(|error| if message_id.is_some() { "chat_submission_pending".into() } else { error })?;
            return Ok("queued");
        }
        turn.running = true;
        drop(turn);
        // A new turn retires the finished tasks the composer kept showing since the last one.
        if prune_finished_tasks(&mut proc.extras.lock().unwrap()) {
            emit_extras(app, session_id, &proc);
        }
        if let Err(e) = dispatch(app, session_id, &proc, text, &images, message_id) {
            // Nothing was written, so nothing will report a result to clear this again.
            let mut turn = proc.turn.lock().unwrap();
            turn.running = false;
            turn.started_at = None;
            drop(turn);
            // A snapshot may have observed the clock in the brief interval before the failed write.
            emit(app, session_id, json!({"type":"turnCompleted"}));
            return Err(if message_id.is_some() { "chat_submission_pending".into() } else { e });
        }
        Ok("sent")
    }

    /// Promote a queued message into the current turn, preserving it on native refusal.
    pub fn queue_steer(&self, app: &AppCtx, session_id: &str, id: &str) -> Result<(), String> {
        let proc = self.get(session_id)?;
        let _action = proc.action.lock().unwrap();
        let item = {
            let mut turn = proc.turn.lock().unwrap();
            if !proc.ready.load(Ordering::Relaxed) || !turn.running || turn.interrupted {
                return Err("There is no running turn available to steer.".into());
            }
            let item = turn.waiting.iter().find(|item| item.id == id).cloned()
                .ok_or("This message is no longer queued.")?;
            turn.steering = true;
            item
        };
        let result = dispatch_steer(app, session_id, &proc, &item.text, &item.images, Some(&item.id));
        {
            let mut turn = proc.turn.lock().unwrap();
            turn.steering = false;
            if result.is_ok() {
                turn.waiting.retain(|item| item.id != id);
            }
        }
        emit_queue(app, session_id, &proc);
        // A completion received while awaiting acceptance deferred the next queued turn.
        start_waiting_message(app, session_id, &proc);
        release_if_idle(app, session_id, &proc);
        result
    }

    /// Drop a message that is still waiting. Its text is never sent.
    pub fn queue_remove(&self, app: &AppCtx, session_id: &str, id: &str) -> Result<(), String> {
        let proc = self.get(session_id)?;
        let _action = proc.action.lock().unwrap();
        {
            let mut turn = proc.turn.lock().unwrap();
            turn.waiting.retain(|item| item.id != id);
        }
        emit_queue(app, session_id, &proc);
        Ok(())
    }

    /// Rewrite a message that is still waiting. A message already sent is past editing, so an id that is
    /// no longer in the queue simply changes nothing.
    pub fn queue_update(
        &self,
        app: &AppCtx,
        session_id: &str,
        id: &str,
        text: &str,
    ) -> Result<(), String> {
        let proc = self.get(session_id)?;
        let _action = proc.action.lock().unwrap();
        {
            let mut turn = proc.turn.lock().unwrap();
            for item in turn.waiting.iter_mut() {
                if item.id == id {
                    item.text = text.to_string();
                }
            }
        }
        emit_queue(app, session_id, &proc);
        Ok(())
    }

    /// Stop the turn in progress. The conversation stays open.
    pub fn interrupt(&self, session_id: &str) -> Result<(), String> {
        let proc = self.get(session_id)?;
        let mut turn = proc.turn.lock().unwrap();
        interrupt_locked(&proc, &mut turn)
    }

    /// Answer a permission question. Allowing runs the tool as proposed.
    ///
    /// `updated_input` is for the tools whose answer *is* the input: AskUserQuestion carries the user's
    /// picks in its own arguments, so the view sends back an edited copy rather than the proposal. Left
    /// out, the proposal is echoed unchanged, which is how the protocol says "run exactly that".
    ///
    /// `updated_permissions` carries the standing rules the answer adopts — one of the question's own
    /// `permission_suggestions`, handed straight back. It is what turns a one-off yes into "and stop
    /// asking me about this".
    pub fn respond_permission(
        &self,
        app: &AppCtx,
        session_id: &str,
        request_id: &str,
        allow: bool,
        updated_input: Option<Value>,
        message: Option<&str>,
        updated_permissions: Option<Value>,
    ) -> Result<(), String> {
        let proc = self.get(session_id)?;
        let request = proc
            .permissions
            .lock()
            .unwrap()
            .get(request_id)
            .cloned()
            .ok_or("That permission request is no longer waiting for an answer")?;
        if proc.kind == SessionKind::Opencode {
            let remember = updated_permissions
                .as_ref()
                .and_then(Value::as_array)
                .is_some_and(|values| {
                    values.iter().any(|value| {
                        value.get("type").and_then(Value::as_str) == Some("opencodeAlways")
                    })
                });
            return opencode::respond(
                app,
                session_id,
                &proc,
                request_id,
                &request,
                allow,
                updated_input.as_ref(),
                remember,
            );
        }
        proc.permissions.lock().unwrap().remove(request_id);
        if proc.kind == SessionKind::Codex {
            // The rule the card adopted, if any, travels inside `updated_permissions`; the wire module
            // turns it into the matching native decision.
            proc.write(&codex_protocol::permission_response(
                &request,
                allow,
                updated_input.as_ref(),
                updated_permissions.as_ref(),
                message,
            ))?;
            if request.get("_codexMethod").and_then(Value::as_str).is_some_and(|m| m.ends_with("requestUserInput")) {
                let input = updated_input.unwrap_or_else(|| {
                    request.get("input").cloned().unwrap_or(Value::Null)
                });
                proc.timeline.lock().unwrap().upsert(ChatRow::Tool {
                    id: request
                        .get("_codexItemId")
                        .and_then(Value::as_str)
                        .unwrap_or(request_id)
                        .to_string(),
                    name: "AskUserQuestion".to_string(),
                    input,
                    output: Some(if allow { "Answered" } else { "Declined" }.to_string()),
                    is_error: !allow,
                    status: if allow { "completed" } else { "failed" },
                    subagent: None,
                    children: Vec::new(),
                });
            }
            emit(app, session_id, json!({"type":"permissionResolved","id":request_id}));
            emit_state(app, session_id, AgentState::Working);
            return Ok(());
        }
        if request.get("subtype").and_then(Value::as_str) == Some("elicitation") {
            // An MCP server asked the person something. "cancel" is the third answer the protocol knows:
            // neither the form filled in nor a refusal, just closing the dialog.
            let action = if allow {
                "accept"
            } else if message == Some("cancel") {
                "cancel"
            } else {
                "decline"
            };
            proc.write(&protocol::elicitation_response(request_id, action, updated_input))?;
            emit(app, session_id, json!({"type":"permissionResolved","id":request_id}));
            emit_state(app, session_id, AgentState::Working);
            return Ok(());
        }
        let input = updated_input
            .unwrap_or_else(|| request.get("input").cloned().unwrap_or(Value::Null));
        proc.write(&protocol::permission_response(
            request_id,
            allow,
            input,
            message,
            updated_permissions,
        ))?;
        emit(app, session_id, json!({"type":"permissionResolved","id":request_id}));
        emit_state(app, session_id, AgentState::Working);
        Ok(())
    }

    /// Save Codex permissions for its next turn; other agents can apply their modes immediately.
    pub fn set_mode(&self, app: &AppCtx, session_id: &str, mode: &str) -> Result<(), String> {
        let proc = self.get(session_id)?;
        let valid = match proc.kind {
            SessionKind::Codex => matches!(mode, "read-only" | "auto" | "full-access"),
            SessionKind::Opencode => matches!(mode, "default" | "bypassPermissions"),
            _ => matches!(mode, "plan" | "default" | "acceptEdits" | "auto" | "bypassPermissions"),
        };
        if !valid {
            return Err(format!("{} does not support chat mode {mode}", proc.kind.as_str()));
        }
        let _change = proc.settings_change.lock().unwrap();
        if proc.kind == SessionKind::Claude {
            proc.request_and_wait("set_permission_mode", |id| {
                protocol::control_request(id, protocol::set_permission_mode(mode))
            })?;
        }
        *proc.mode.lock().unwrap() = mode.to_string();
        if proc.kind == SessionKind::Codex {
            return Ok(());
        }
        if proc.kind == SessionKind::Opencode {
            // Nothing to tell the server: the engine answers its questions itself in bypass, starting
            // with the ones already waiting.
            if mode == "bypassPermissions" {
                opencode::approve_pending(app, session_id, &proc);
            }
            return Ok(());
        }
        Ok(())
    }

    pub fn pending_permission_mode(&self, session_id: &str) -> Option<PendingPermissionMode> {
        self.sessions.lock().unwrap().get(session_id).and_then(|proc| proc.permission_modes().1)
    }

    /// Change how Codex collaborates on subsequent turns, or which OpenCode agent answers them, without
    /// changing filesystem permissions.
    pub fn set_collaboration_mode(
        &self,
        app: &AppCtx,
        session_id: &str,
        mode: &str,
    ) -> Result<(), String> {
        let proc = self.get(session_id)?;
        if !matches!(proc.kind, SessionKind::Codex | SessionKind::Opencode) {
            return Err(format!(
                "{} does not support collaboration modes",
                proc.kind.as_str()
            ));
        }
        let supported = proc.collaboration_modes.lock().unwrap();
        if !supported.iter().any(|preset| preset.mode == mode) {
            return Err(match proc.kind {
                SessionKind::Opencode => format!("OpenCode has no agent named {mode}"),
                _ => format!("Codex app-server does not support collaboration mode {mode}"),
            });
        }
        drop(supported);
        *proc.collaboration_mode.lock().unwrap() = Some(mode.to_string());
        emit(
            app,
            session_id,
            json!({"type":"collaborationModeChanged","mode":mode}),
        );
        Ok(())
    }

    /// Persist OpenCode's native settings before its terminal interface resumes the conversation.
    pub fn prepare_terminal(&self, session_id: &str, selection: &crate::agent::session_settings::Selection) -> Result<(), String> {
        let proc = self.get(session_id)?;
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while !proc.ready.load(Ordering::Relaxed) {
            if !proc.alive.load(Ordering::Relaxed) || std::time::Instant::now() >= deadline {
                return Err("OpenCode did not become ready to transfer its model settings".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        opencode::prepare_terminal(&proc, selection)
    }

    /// Change the model of the running conversation. `None` restores the default.
    pub fn set_model(&self, session_id: &str, model: Option<&str>) -> Result<(), String> {
        let proc = self.get(session_id)?;
        let model = if proc.kind == SessionKind::Claude {
            model.map(crate::agent::claude_models::normalize_id)
        } else {
            model
        };
        let _change = proc.settings_change.lock().unwrap();
        if proc.kind == SessionKind::Claude {
            proc.request_and_wait("set_model", |id| {
                protocol::control_request(id, protocol::set_model(model))
            })?;
        }
        *proc.model.lock().unwrap() = model.map(str::to_string);
        Ok(())
    }

    /// Change reasoning effort. Codex applies it to the next turn; Claude exposes it as a slash command.
    pub fn set_effort(
        &self,
        app: &AppCtx,
        session_id: &str,
        effort: Option<&str>,
    ) -> Result<(), String> {
        let proc = self.get(session_id)?;
        // Automatic effort removes the override; an empty string is not a provider effort level.
        let effort = effort.map(str::trim).filter(|value| !value.is_empty());
        let previous = std::mem::replace(
            &mut *proc.effort.lock().unwrap(),
            effort.map(str::to_string),
        );
        let was_off = previous.as_deref() == Some(protocol::THINKING_OFF);
        if proc.kind == SessionKind::Claude {
            // "off" is a cap of zero thinking tokens rather than a level; a level chosen after it lifts
            // the cap first, or the level would apply to thinking the model is still forbidden to do.
            if effort == Some(protocol::THINKING_OFF) {
                let id = proc.request_id("set_max_thinking_tokens");
                proc.write(&protocol::control_request(&id, protocol::set_max_thinking_tokens(Some(0))))?;
                return Ok(());
            }
            if was_off {
                let id = proc.request_id("set_max_thinking_tokens");
                proc.write(&protocol::control_request(&id, protocol::set_max_thinking_tokens(None)))?;
            }
            self.send(app, session_id, &format!("/effort {}", effort.unwrap_or("auto")), Vec::new(), "queue")?;
        }
        Ok(())
    }

    /// Choose the Codex service tier — `fast`, or another id its model catalogue lists — for subsequent
    /// turns. None returns to the tier the thread was opened with.
    pub fn set_service_tier(&self, session_id: &str, tier: Option<&str>) -> Result<(), String> {
        let proc = self.codex(session_id)?;
        *proc.service_tier.lock().unwrap() = tier.map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
        Ok(())
    }

    /// Choose how Codex speaks on subsequent turns: `none`, `friendly` or `pragmatic`.
    pub fn set_personality(&self, session_id: &str, personality: Option<&str>) -> Result<(), String> {
        let proc = self.codex(session_id)?;
        let personality = personality.map(str::trim).filter(|p| !p.is_empty());
        if personality.is_some_and(|p| !matches!(p, "none" | "friendly" | "pragmatic")) {
            return Err(format!("Codex does not know the personality {}", personality.unwrap_or("")));
        }
        *proc.personality.lock().unwrap() = personality.map(str::to_string);
        Ok(())
    }

    /// `/compact`: ask Codex to summarize the conversation now.
    pub fn compact(&self, session_id: &str) -> Result<(), String> {
        let proc = self.codex(session_id)?;
        let _action = proc.action.lock().unwrap();
        codex::compact(&proc)
    }

    /// `/review`: run Codex's code review on the working tree, a branch, a commit, or free-form instructions.
    pub fn review(&self, app: &AppCtx, session_id: &str, args: &str) -> Result<(), String> {
        let proc = self.codex(session_id)?;
        codex::review(app, session_id, &proc, args)
    }

    fn codex(&self, session_id: &str) -> Result<Arc<ChatProcess>, String> {
        let proc = self.get(session_id)?;
        if proc.kind != SessionKind::Codex {
            return Err("This action is available only for Codex conversations".to_string());
        }
        Ok(proc)
    }

    /// Switch Claude's fast mode for this process. The catalogue says which models honour it.
    pub fn set_fast_mode(&self, app: &AppCtx, session_id: &str, enabled: bool) -> Result<(), String> {
        let proc = self.get(session_id)?;
        if proc.kind != SessionKind::Claude {
            return Err("Fast mode is available only for Claude conversations".to_string());
        }
        proc.request_and_wait("apply_flag_settings", |id| {
            protocol::control_request(id, protocol::apply_flag_settings(json!({"fastMode":enabled})))
        })?;
        proc.extras.lock().unwrap().fast_mode = enabled;
        emit_extras(app, session_id, &proc);
        Ok(())
    }

    /// The MCP servers this Claude process knows, each with its connection state and tools.
    pub fn mcp_status(&self, session_id: &str) -> Result<Value, String> {
        let proc = self.get(session_id)?;
        if proc.kind == SessionKind::Codex { return codex::mcp_status(&proc); }
        let proc = self.claude(session_id)?;
        proc.request_and_wait("mcp_status", |id| protocol::control_request(id, protocol::mcp_status()))
    }

    pub fn mcp_toggle(&self, session_id: &str, server: &str, enabled: bool) -> Result<Value, String> {
        let proc = self.get(session_id)?;
        if proc.kind == SessionKind::Codex { return codex::mcp_toggle(&proc, server, enabled); }
        let proc = self.claude(session_id)?;
        proc.request_and_wait("mcp_toggle", |id| {
            protocol::control_request(id, protocol::mcp_toggle(server, enabled))
        })?;
        proc.request_and_wait("mcp_status", |id| protocol::control_request(id, protocol::mcp_status()))
    }

    pub fn mcp_reconnect(&self, session_id: &str, server: &str) -> Result<Value, String> {
        let proc = self.get(session_id)?;
        if proc.kind == SessionKind::Codex { return codex::mcp_reconnect(&proc, server); }
        let proc = self.claude(session_id)?;
        proc.request_and_wait("mcp_reconnect", |id| {
            protocol::control_request(id, protocol::mcp_reconnect(server))
        })?;
        proc.request_and_wait("mcp_status", |id| protocol::control_request(id, protocol::mcp_status()))
    }

    /// Stop one background task. The agent reports the new task set on its own afterwards.
    pub fn stop_task(&self, session_id: &str, task_id: &str) -> Result<(), String> {
        let proc = self.claude(session_id)?;
        proc.request_and_wait("stop_task", |id| protocol::control_request(id, protocol::stop_task(task_id)))?;
        Ok(())
    }

    /// Move every foreground task to the background so the turn continues without waiting on them.
    pub fn background_tasks(&self, session_id: &str) -> Result<(), String> {
        let proc = self.claude(session_id)?;
        proc.request_and_wait("background_tasks", |id| {
            protocol::control_request(id, protocol::background_tasks())
        })?;
        Ok(())
    }

    /// The live provider catalogue, or None when no process owns this session.
    pub fn live_commands(&self, session_id: &str) -> Option<Vec<Value>> {
        let proc = self.get(session_id).ok()?;
        if !proc.alive.load(Ordering::Relaxed) { return None; }
        let commands = proc.commands.lock().unwrap().clone();
        Some(commands)
    }

    /// The catalogue the running Claude process reported, or None before it has answered.
    pub fn live_claude_models(&self, session_id: &str) -> Option<Vec<crate::agent::claude_models::ClaudeModel>> {
        let proc = self.sessions.lock().unwrap().get(session_id).cloned()?;
        if proc.kind != SessionKind::Claude {
            return None;
        }
        let list = proc.claude_models.lock().unwrap().clone();
        (!list.is_empty()).then(|| crate::agent::claude_models::from_live(&list))
    }

    /// Model and context usage from the running Claude or Codex process, for the Info panel.
    pub fn context_info(&self, session_id: &str) -> Option<crate::agent::transcript::AgentContextInfo> {
        let proc = self.sessions.lock().unwrap().get(session_id).cloned()?;
        // Codex reports the same two numbers through `thread/tokenUsage/updated`, into the same fields.
        if !matches!(proc.kind, SessionKind::Claude | SessionKind::Codex) {
            return None;
        }
        let extras = proc.extras.lock().unwrap().clone();
        // Codex token-usage notifications omit the model; the session stores it separately.
        let model = extras.usage_model.clone()
            .or_else(|| proc.model.lock().unwrap().clone());
        if let Some(usage) = &extras.context_usage {
            let limit = usage.get("maxTokens").and_then(Value::as_u64)?;
            return Some(crate::agent::transcript::AgentContextInfo {
                model: usage
                    .get("model")
                    .and_then(Value::as_str)
                    .filter(|m| !m.is_empty())
                    .map(str::to_string)
                    .or(model),
                context_tokens: usage.get("totalTokens").and_then(Value::as_u64),
                context_limit: limit,
                current_tool: None,
            });
        }
        let limit = extras.context_window?;
        Some(crate::agent::transcript::AgentContextInfo {
            model,
            context_tokens: extras.context_tokens,
            context_limit: limit,
            current_tool: None,
        })
    }

    fn claude(&self, session_id: &str) -> Result<Arc<ChatProcess>, String> {
        let proc = self.get(session_id)?;
        if proc.kind != SessionKind::Claude {
            return Err("This action is available only for Claude conversations".to_string());
        }
        Ok(proc)
    }

    /// Everything a client needs to render the conversation from scratch.
    pub fn row_detail(&self, session_id: &str, row_id: &str, epoch: Option<u64>) -> Result<Value, String> {
        let proc = self.get(session_id)?;
        if epoch.is_some_and(|value| value != proc.started_at) { return Err("Chat history changed; synchronize the conversation again".into()); }
        let timeline = proc.timeline.lock().unwrap();
        fn find<'a>(rows: &'a [ChatRow], id: &str) -> Option<&'a ChatRow> {
            for row in rows {
                if row.id() == id { return Some(row); }
                if let ChatRow::Tool { children, .. } = row {
                    if let Some(child) = find(children, id) { return Some(child); }
                }
            }
            None
        }
        let row = find(&timeline.rows, row_id).ok_or("Chat row is no longer available")?;
        Ok(detail_row(row))
    }

    pub fn snapshot(&self, session_id: &str) -> ChatSnapshot {
        self.snapshot_window(session_id, None)
    }

    pub fn snapshot_window(&self, session_id: &str, window: Option<&ChatWindow>) -> ChatSnapshot {
        let Some(proc) = self.sessions.lock().unwrap().get(session_id).cloned() else {
            return ChatSnapshot {
                positions: HashMap::new(), page_kind: "full", has_more: false, total_rows: 0,
                submission_receipts: true,
                rows_revision: 0,
                queue_revision: 0,
                rewind_scopes: Vec::new(),
                running: false,
                rows: Vec::new(),
                queue: Vec::new(),
                permissions: Vec::new(),
                commands: Vec::new(),
                config_keys: Vec::new(),
                agent_session_id: None,
                model: None,
                effort: None,
                selection: None,
                mode: None,
                pending_permission_mode: None,
                collaboration_mode: None,
                collaboration_modes: Vec::new(),
                service_tier: None,
                personality: None,
                pid: None,
                started_at: None,
                turn_started_at: None,
                extras: ClaudeExtras::default(),
            };
        };
        // Read every field into a local first: building the struct inline would keep the lock guards alive
        // past the point where `proc` itself is dropped.
        let running = proc.alive.load(Ordering::Relaxed);
        let (rows, positions, rows_revision, page_kind, has_more, total_rows) = {
            let mut timeline = proc.timeline.lock().unwrap();
            timeline.revision += 1;
            let total = timeline.rows.len();
            let mut start = 0;
            let mut end = total;
            let mut kind = "full";
            let mut delta = None;
            if let Some(window) = window {
                kind = "recent";
                if window.epoch == Some(proc.started_at) {
                    if let Some(before) = &window.before {
                        if let Some(index) = timeline.at.get(before) { end = *index; kind = "history"; }
                    } else if let Some(since) = window.since {
                        if since >= timeline.reset_revision && since <= timeline.revision {
                            delta = Some(since); kind = "delta";
                        }
                    }
                }
                start = end.saturating_sub(SNAPSHOT_PAGE_ROWS);
            }
            let selected: Vec<ChatRow> = if let Some(since) = delta {
                start = window.and_then(|w| w.from.as_ref()).and_then(|id| timeline.at.get(id)).copied().unwrap_or(start);
                timeline.rows[start..].iter().filter(|row| timeline.row_versions.get(row.id()).copied().unwrap_or(0) > since).cloned().collect()
            } else { timeline.rows[start..end].to_vec() };
            let positions = selected.iter().filter_map(|row| timeline.at.get(row.id()).map(|index| (row.id().to_owned(), *index))).collect();
            (selected, positions, timeline.revision, kind, start > 0, total)
        };
        let (queue, turn_started_at, queue_revision) = {
            let mut turn = proc.turn.lock().unwrap();
            turn.revision += 1;
            (turn.waiting.clone(), turn.started_at, turn.revision)
        };
        let permissions: Vec<Value> = proc.permissions.lock().unwrap().values().cloned().collect();
        let commands = proc.commands.lock().unwrap().clone();
        let config_keys = proc.config_keys.lock().unwrap().clone();
        let agent_session_id = proc.agent_session_id.lock().unwrap().clone();
        let model = proc.model.lock().unwrap().clone();
        let effort = proc.effort.lock().unwrap().clone();
        let (mode, pending_permission_mode) = proc.permission_modes();
        let collaboration_mode = proc.collaboration_mode.lock().unwrap().clone();
        let collaboration_modes = proc.collaboration_modes.lock().unwrap().clone();
        let service_tier = proc.service_tier.lock().unwrap().clone();
        let personality = proc.personality.lock().unwrap().clone();
        let extras = proc.extras.lock().unwrap().published();
        ChatSnapshot {
            positions, page_kind, has_more, total_rows,
            submission_receipts: true,
            rows_revision,
            queue_revision,
            rewind_scopes: rewind_scopes(proc.kind),
            running,
            rows,
            queue,
            permissions,
            commands,
            config_keys,
            agent_session_id,
            model,
            effort,
            selection: None,
            mode: Some(mode),
            pending_permission_mode,
            collaboration_mode,
            collaboration_modes,
            service_tier,
            personality,
            pid: Some(proc.pid),
            started_at: Some(proc.started_at),
            turn_started_at,
            extras,
        }
    }

    /// Resolve one attachment reference from a slim snapshot.
    ///
    /// The reference names only data already owned by this session; it is not a filesystem path and gives
    /// the caller no way to read arbitrary files. A stale reference fails cleanly after its row is gone.
    pub fn attachment(&self, session_id: &str, attachment_id: &str) -> Result<ChatImage, String> {
        let proc = self.get(session_id)?;
        {
            let timeline = proc.timeline.lock().unwrap();
            for row in &timeline.rows {
                if let Some(image) = row_attachment(row, attachment_id) {
                    return Ok(image.clone());
                }
            }
        }
        let turn = proc.turn.lock().unwrap();
        for item in &turn.waiting {
            for (index, image) in item.images.iter().enumerate() {
                if queue_attachment_id(&item.id, index) == attachment_id {
                    return Ok(image.clone());
                }
            }
        }
        Err("That chat attachment is no longer available".to_string())
    }

    /// Preview file checkpoints without changing either files or conversation state.
    pub fn rewind_preview(&self, session_id: &str, row_id: &str) -> Result<Value, String> {
        let proc = self.get(session_id)?;
        let _action = proc.action.lock().unwrap();
        if proc.kind == SessionKind::Codex {
            return Err("Codex does not expose native file restore through app-server".to_string());
        }
        if proc.kind == SessionKind::Opencode {
            // OpenCode restores the files along with the conversation and has no dry run; the answer is
            // only whether the message can be named.
            ensure_rewind_idle(&proc)?;
            rewind_target(&proc, row_id)?;
            return Ok(json!({"canRewind":true}));
        }
        if proc.kind != SessionKind::Claude {
            return Err("File rewind is supported only for Claude conversations".to_string());
        }
        ensure_rewind_idle(&proc)?;
        let target = rewind_target(&proc, row_id)?;
        proc.request_and_wait("rewind_files", |id| {
            protocol::control_request(id, protocol::rewind_files(&target.message_id, true))
        })
    }

    /// Rewind a conversation, file checkpoints, or both from one visible user row.
    pub fn rewind(
        &self,
        app: &AppCtx,
        session_id: &str,
        row_id: &str,
        scope: &str,
    ) -> Result<Value, String> {
        let proc = self.get(session_id)?;
        let _action = proc.action.lock().unwrap();
        ensure_rewind_idle(&proc)?;
        let conversation = matches!(scope, "conversation" | "both");
        let files = matches!(scope, "files" | "both");
        if !conversation && !files {
            return Err(format!("Unknown rewind scope: {scope}"));
        }
        if proc.kind == SessionKind::Codex && files {
            return Err("Codex does not expose native file restore through app-server".to_string());
        }

        let target = rewind_target(&proc, row_id)?;
        let (drop_rows, prefill_text) = {
            let timeline = proc.timeline.lock().unwrap();
            let users = timeline.user_messages();
            let Some(index) = users.iter().position(|(id, _)| id == row_id) else {
                return Err("That user message is no longer in the conversation".to_string());
            };
            (users[index..].to_vec(), users[index].1.clone())
        };

        // The file checkpoint is still addressable before the conversation branch moves. Reversing this
        // order could drop the native message first and leave its edits on disk with no target left to name.
        let file_result = if files && proc.kind == SessionKind::Claude {
            Some(proc.request_and_wait("rewind_files", |id| {
                protocol::control_request(id, protocol::rewind_files(&target.message_id, false))
            })?)
        } else {
            None
        };

        let mut provider_prefill = None;
        // OpenCode has no file-only restore: asking for the files alone still moves the conversation.
        let conversation = conversation || proc.kind == SessionKind::Opencode;
        if conversation {
            match proc.kind {
                SessionKind::Claude => {
                    // One request drops the target and everything after it. Naming the latest user
                    // message as well is what lets Claude accept an older target; it also makes a
                    // view that missed a turn fail loudly rather than rewind to the wrong depth.
                    let latest = drop_rows.last().map(|(id, _)| id.as_str()).unwrap_or(row_id);
                    let last_seen = rewind_target(&proc, latest)?;
                    let response = proc.request_and_wait("rewind_conversation", |request_id| {
                        protocol::control_request(
                            request_id,
                            protocol::rewind_conversation(&target.message_id, &last_seen.message_id),
                        )
                    })?;
                    if response.get("rewound").and_then(Value::as_bool) != Some(true) {
                        let message = response
                            .get("error")
                            .and_then(Value::as_str)
                            .unwrap_or("Claude refused the conversation rewind");
                        return Err(format!("Claude refused the conversation rewind: {message}"));
                    }
                    if let Some(text) = response.get("prefillText").and_then(Value::as_str) {
                        provider_prefill = Some(text.to_string());
                    }
                }
                SessionKind::Codex => rewind_codex(app, session_id, &proc, &target, &drop_rows)?,
                // One request restores both the conversation and the files; there is no separate
                // file-only rewind, so any scope that reaches here does both.
                SessionKind::Opencode => opencode::rewind(&proc, &target)?,
                other => {
                    return Err(format!(
                        "Conversation rewind is not supported for {} sessions",
                        other.as_str()
                    ))
                }
            }

            if !proc.timeline.lock().unwrap().trim_from_user(row_id) {
                return Err("That user message is no longer in the conversation".to_string());
            }
            emit_state(app, session_id, AgentState::Waiting);
        }

        Ok(json!({
            "prefillText":conversation.then(|| provider_prefill.unwrap_or(prefill_text)),
            "files":file_result,
        }))
    }

    /// End the conversation and let the process go.
    pub fn stop_for_handoff(&self, app: &AppCtx, session_id: &str) -> Result<(), String> {
        let proc = self.sessions.lock().unwrap().get(session_id).cloned();
        self.stop(app, session_id)?;
        if let Some(proc) = proc {
            proc.child.lock().unwrap().wait().map_err(|e| format!("Failed to stop the conversation engine: {e}"))?;
        }
        Ok(())
    }

    /// End the conversation and let the process go.
    pub fn stop(&self, app: &AppCtx, session_id: &str) -> Result<(), String> {
        let Some(proc) = self.sessions.lock().unwrap().remove(session_id) else {
            return Ok(());
        };
        let _action = proc.action.lock().unwrap();
        crate::session_state::set_alive(app, session_id, false);
        proc.alive.store(false, Ordering::Relaxed);
        proc.released.store(true, Ordering::Relaxed);
        {
            let mut turn = proc.turn.lock().unwrap();
            turn.running = false;
            turn.started_at = None;
            turn.waiting.clear();
        }
        emit_queue(app, session_id, &proc);
        // Closing stdin asks the agent to finish; killing is the fallback for one that does not.
        *proc.stdin.lock().unwrap() = None;
        let _ = proc.child.lock().unwrap().kill();
        Ok(())
    }

    /// A view is showing this conversation again, so a release it asked for earlier no longer applies.
    ///
    /// Nothing else happens here: the process, if any, keeps running, and one that already went is started
    /// again only when the next message is sent.
    pub fn attach(&self, session_id: &str) {
        if let Some(proc) = self.sessions.lock().unwrap().get(session_id) {
            let _turn = proc.turn.lock().unwrap();
            proc.release_when_idle.store(false, Ordering::Relaxed);
        }
    }

    /// A view stopped showing this conversation: let the process go once it is idle.
    ///
    /// Idle means no turn running and nothing queued behind it, and then the process ends now. While a
    /// turn is running the release waits for `handle_turn_end`, so an answer that is still being written
    /// is not cut off. Unlike `stop`, the session stays in the table with its timeline, so a pane opened
    /// later — here or on another device — still shows the conversation and restarts the agent lazily.
    pub fn detach(&self, app: &AppCtx, session_id: &str) {
        let Some(proc) = self.sessions.lock().unwrap().get(session_id).cloned() else {
            return;
        };
        if !proc.alive.load(Ordering::Relaxed) {
            return;
        }
        proc.release_when_idle.store(true, Ordering::Relaxed);
        release_if_idle(app, session_id, &proc);
    }
}

/// The turn lock keeps a new send or attach from racing the final idle check and process release.
fn release_if_idle(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let mut turn = proc.turn.lock().unwrap();
    let now = std::time::Instant::now();
    turn.settling.retain(|_, deadline| *deadline > now);
    if proc.release_when_idle.load(Ordering::Relaxed)
        && proc.alive.load(Ordering::Relaxed)
        && !turn.running
        && turn.waiting.is_empty()
        && turn.active_tasks.is_empty()
        && turn.background_tasks.is_empty()
        && turn.settling.is_empty()
        && proc.permissions.lock().unwrap().is_empty()
    {
        release_process(app, session_id, proc);
    }
}

/// Track task liveness independently of Task cards: shell tasks have no subagent card at all.
fn track_claude_task(proc: &Arc<ChatProcess>, subtype: &str, message: &Value) {
    let Some(id) = message.get("task_id").and_then(Value::as_str) else { return };
    let status = message.pointer("/patch/status").or_else(|| message.get("status")).and_then(Value::as_str);
    let finished = matches!(status, Some("completed" | "failed" | "error" | "killed" | "stopped" | "canceled" | "cancelled"))
        || (subtype == "task_notification" && status.is_none());
    let mut turn = proc.turn.lock().unwrap();
    if subtype == "task_notification" {
        turn.settling.remove(id);
    }
    if finished {
        turn.active_tasks.remove(id);
        turn.background_tasks.remove(id);
    } else if matches!(subtype, "task_started" | "task_progress") || matches!(status, Some("pending" | "running" | "paused")) {
        turn.active_tasks.insert(id.to_string());
    }
}

/// Fold one `task_*` frame into the merged task list the composer and the task tabs read.
///
/// Every frame upserts by task_id, so the list also holds foreground subagents the inventory never named;
/// `ClaudeExtras::published` keeps those out of the client view. Fields missing from a frame leave the
/// merged state alone: older CLIs send no `workflow_progress`, and only about every second frame carries it.
fn fold_task_frame(extras: &mut ClaudeExtras, subtype: &str, message: &Value) {
    let Some(task_id) = message.get("task_id").and_then(Value::as_str).filter(|id| !id.is_empty()) else { return };
    let text = |key: &str| message.get(key).and_then(Value::as_str).filter(|value| !value.trim().is_empty()).map(str::to_string);
    let task = upsert_task(&mut extras.background_tasks, task_id);
    match subtype {
        "task_started" => {
            if let Some(task_type) = text("task_type") { task.task_type = task_type; }
            if let Some(description) = text("description") { task.description = description; }
            if let Some(tool_use_id) = text("tool_use_id") { task.tool_use_id = Some(tool_use_id); }
            if let Some(workflow_name) = text("workflow_name") { task.workflow_name = Some(workflow_name); }
            task.status = "running".into();
        }
        "task_progress" => {
            if let Some(description) = text("description") { task.description = description; }
            if let Some(last_tool_name) = text("last_tool_name") { task.last_tool_name = Some(last_tool_name); }
            if let Some(summary) = text("summary") { task.summary = Some(summary); }
            if let Some(usage) = message.get("usage").filter(|usage| usage.is_object()) { task.usage = Some(usage.clone()); }
            if let Some(progress) = message.get("workflow_progress").filter(|progress| progress.is_array()) {
                task.workflow_progress = Some(progress.clone());
            }
        }
        "task_updated" => {
            if let Some(status) = message.pointer("/patch/status").and_then(Value::as_str).filter(|value| !value.is_empty()) {
                task.status = status.into();
            }
            if let Some(end_time) = message.pointer("/patch/end_time").and_then(Value::as_u64) { task.ended_at = Some(end_time); }
        }
        "task_notification" => {
            task.status = text("status").unwrap_or_else(|| "ended".into());
            if let Some(summary) = text("summary") { task.summary = Some(summary); }
            if let Some(usage) = message.get("usage").filter(|usage| usage.is_object()) { task.usage = Some(usage.clone()); }
            if let Some(output_file) = text("output_file") { task.output_file = Some(output_file); }
            if task.ended_at.is_none() { task.ended_at = Some(now_ms()); }
        }
        _ => {}
    }
    cap_finished_tasks(&mut extras.background_tasks);
}

/// Fold a `background_tasks_changed` inventory into the merged list.
///
/// The inventory is authoritative for membership, not for content: it names the tasks that are in the
/// background right now, with a static description. A listed task it no longer names has finished, and
/// its terminal frames arrive only afterwards (if at all), so it is marked "ended" here rather than dropped.
fn fold_task_inventory(extras: &mut ClaudeExtras, tasks: &[Value]) {
    let mut current: HashSet<&str> = HashSet::new();
    for entry in tasks {
        let Some(task_id) = entry.get("task_id").and_then(Value::as_str).filter(|id| !id.is_empty()) else { continue };
        current.insert(task_id);
        let task = upsert_task(&mut extras.background_tasks, task_id);
        task.listed = true;
        if task.task_type.is_empty() {
            if let Some(task_type) = entry.get("task_type").and_then(Value::as_str) { task.task_type = task_type.into(); }
        }
        if task.description.is_empty() {
            if let Some(description) = entry.get("description").and_then(Value::as_str) { task.description = description.into(); }
        }
        if let Some(status) = entry.get("status").and_then(Value::as_str).filter(|value| !value.is_empty()) {
            task.status = status.into();
        }
    }
    let now = now_ms();
    for task in extras.background_tasks.iter_mut() {
        if task.listed && !task.finished() && !current.contains(task.task_id.as_str()) {
            task.status = "ended".into();
            task.ended_at = Some(now);
        }
    }
    cap_finished_tasks(&mut extras.background_tasks);
}

/// The merged entry for `task_id`, created as a running task on first sight.
fn upsert_task<'a>(tasks: &'a mut Vec<BackgroundTask>, task_id: &str) -> &'a mut BackgroundTask {
    if let Some(index) = tasks.iter().position(|task| task.task_id == task_id) {
        return &mut tasks[index];
    }
    tasks.push(BackgroundTask {
        task_id: task_id.to_string(),
        status: "running".into(),
        started_at: Some(now_ms()),
        ..BackgroundTask::default()
    });
    tasks.last_mut().expect("just pushed")
}

/// Keep at most `FINISHED_TASK_CAP` finished tasks, dropping the oldest first.
fn cap_finished_tasks(tasks: &mut Vec<BackgroundTask>) {
    while tasks.iter().filter(|task| task.finished()).count() > FINISHED_TASK_CAP {
        let Some(index) = tasks.iter().position(BackgroundTask::finished) else { break };
        tasks.remove(index);
    }
}

/// Drop finished tasks when a new turn starts; their final state was on screen until now. Returns
/// whether anything changed so the caller can republish `extras`.
fn prune_finished_tasks(extras: &mut ClaudeExtras) -> bool {
    let before = extras.background_tasks.len();
    extras.background_tasks.retain(|task| !task.finished());
    extras.background_tasks.len() != before
}

/// End an idle process that no view is showing any more.
///
/// The process is killed rather than asked to finish: both agents have written every record they hold by
/// the time they are idle, so there is nothing left to flush. The stdout reader sees the pipe close and
/// reports the exit as released, which is what keeps a client that still shows the pane from drawing an
/// error for it.
fn release_process(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    proc.release_when_idle.store(false, Ordering::Relaxed);
    proc.released.store(true, Ordering::Relaxed);
    proc.alive.store(false, Ordering::Relaxed);
    crate::session_state::set_alive(app, session_id, false);
    *proc.stdin.lock().unwrap() = None;
    let _ = proc.child.lock().unwrap().kill();
}

/// Serialize a timeline row for a snapshot without embedding image bytes.
fn snapshot_row(row: &ChatRow) -> Value {
    match row {
        ChatRow::User { id, text, images, at } => json!({
            "kind":"user", "id":id, "text":text, "at":at,
            "images":images.iter().enumerate().map(|(index,image)| json!({"mimeType":image.mime_type,"attachmentId":row_attachment_id(id,index)})).collect::<Vec<_>>()
        }),
        ChatRow::Tool { id, name, input, output, is_error, status, subagent, children } if name != "AskUserQuestion" => {
            let encoded = input.to_string();
            let deferred = encoded.len() > 4096 || output.as_ref().is_some_and(|text| text.len() > 4096) || !children.is_empty();
            let mut value = json!({"kind":"tool", "id":id,"name":name,
                "input":if encoded.len() > 4096 { Value::String(encoded.chars().take(1024).collect()) } else { input.clone() },
                "output":if deferred { None } else { output.as_ref() },
                "isError":is_error,"status":status,"subagent":subagent,
                "detailAvailable":deferred,"childCount":children.len(),"children":[]});
            if deferred || output.is_none() { value.as_object_mut().unwrap().remove("output"); }
            value
        }
        _ => serde_json::to_value(row).unwrap_or(Value::Null),
    }
}

fn detail_row(row: &ChatRow) -> Value {
    let mut value = serde_json::to_value(row).unwrap_or(Value::Null);
    if let ChatRow::Tool { children, .. } = row {
        value["children"] = Value::Array(children.iter().map(snapshot_row).collect());
    }
    value
}

fn serialize_snapshot_rows<S>(rows: &[ChatRow], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let values = rows.iter().map(snapshot_row).collect::<Vec<_>>();
    serde::Serialize::serialize(&values, serializer)
}

fn snapshot_queue_item(item: &QueuedMessage) -> Value {
    let mut value = serde_json::to_value(item).unwrap_or(Value::Null);
    if !item.images.is_empty() {
        value["images"] = Value::Array(
            item.images
                .iter()
                .enumerate()
                .map(|(index, image)| {
                    json!({
                        "mimeType": image.mime_type,
                        "attachmentId": queue_attachment_id(&item.id, index),
                    })
                })
                .collect(),
        );
    }
    value
}

fn serialize_snapshot_queue<S>(queue: &[QueuedMessage], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let values = queue.iter().map(snapshot_queue_item).collect::<Vec<_>>();
    serde::Serialize::serialize(&values, serializer)
}

fn row_attachment_id(row_id: &str, index: usize) -> String {
    format!("row:{row_id}:{index}")
}

fn queue_attachment_id(queue_id: &str, index: usize) -> String {
    format!("queue:{queue_id}:{index}")
}

fn row_attachment<'a>(row: &'a ChatRow, attachment_id: &str) -> Option<&'a ChatImage> {
    match row {
        ChatRow::User { id, images, .. } => images.iter().enumerate().find_map(|(index, image)| {
            (row_attachment_id(id, index) == attachment_id).then_some(image)
        }),
        ChatRow::Tool { children, .. } => children
            .iter()
            .find_map(|child| row_attachment(child, attachment_id)),
        _ => None,
    }
}

/// Only expose restoration scopes implemented by the agent's native protocol.
fn rewind_scopes(kind: SessionKind) -> Vec<&'static str> {
    match kind {
        SessionKind::Claude => vec!["conversation", "files", "both"],
        SessionKind::Codex => vec!["conversation"],
        SessionKind::Opencode => vec!["both"],
        _ => Vec::new(),
    }
}

fn ensure_rewind_idle(proc: &ChatProcess) -> Result<(), String> {
    if !proc.alive.load(Ordering::Relaxed) || !proc.ready.load(Ordering::Relaxed) {
        return Err("Wait for the agent to finish starting before rewinding".to_string());
    }
    let turn = proc.turn.lock().unwrap();
    if turn.running {
        return Err("Wait for the current turn to finish before rewinding".to_string());
    }
    if !turn.waiting.is_empty() {
        return Err("Remove queued messages before rewinding".to_string());
    }
    if !turn.active_tasks.is_empty() || !turn.background_tasks.is_empty() {
        return Err("Wait for background tasks to finish before rewinding".into());
    }
    drop(turn);
    if !proc.permissions.lock().unwrap().is_empty() {
        return Err("Answer the pending permission request before rewinding".to_string());
    }
    Ok(())
}

/// Resolve the view's stable row id to the provider identity stored in its native recording.
fn rewind_target(proc: &ChatProcess, row_id: &str) -> Result<super::history::RewindTarget, String> {
    if let Some(target) = proc.user_targets.lock().unwrap().get(row_id).cloned() {
        return Ok(target);
    }
    if proc.kind == SessionKind::Opencode {
        if let Some(message_id) = row_id.strip_prefix("u:") {
            let text = match proc.timeline.lock().unwrap().get(row_id) {
                Some(ChatRow::User { text, .. }) => text.clone(),
                _ => String::new(),
            };
            return Ok(super::history::RewindTarget {
                message_id: message_id.to_string(),
                turn_id: None,
                text,
            });
        }
    }
    let agent_id = proc
        .agent_session_id
        .lock()
        .unwrap()
        .clone()
        .ok_or("The agent has not recorded this conversation yet")?;
    let targets = super::history::rewind_targets(proc.kind, &agent_id)?;
    let mut known = proc.user_targets.lock().unwrap();
    let row_messages = proc.timeline.lock().unwrap().user_messages();
    let mut cursor = targets.len();
    for (id, text) in row_messages.iter().rev() {
        if known.contains_key(id) {
            continue;
        }
        while cursor > 0 {
            cursor -= 1;
            if targets[cursor].text.trim() == text.trim() {
                known.insert(id.clone(), targets[cursor].clone());
                break;
            }
        }
    }
    known
        .get(row_id)
        .cloned()
        .ok_or_else(|| "The native recording does not contain that user message".to_string())
}

fn rewind_codex(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    target: &super::history::RewindTarget,
    drop_rows: &[(String, String)],
) -> Result<(), String> {
    let thread = fork_codex_for_rewind(proc, target, drop_rows)?;
    activate_rewound_codex(app, session_id, proc, &thread);
    Ok(())
}

fn activate_rewound_codex(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, thread: &str) {
    proc.current_turn.lock().unwrap().take();
    proc.pending_user_rows.lock().unwrap().clear();
    remember_codex_thread(app, session_id, proc, thread, None, None);
}

fn fork_codex_for_rewind(
    proc: &Arc<ChatProcess>, target: &super::history::RewindTarget, drop_rows: &[(String, String)],
) -> Result<String, String> {
    let thread_id = proc
        .agent_session_id
        .lock()
        .unwrap()
        .clone()
        .ok_or("Codex has not opened its thread yet")?;
    let metadata = proc.request_and_wait("thread_read", |id| {
        codex_protocol::thread_read_metadata(id, &thread_id)
    })?;
    let history_mode = metadata
        .pointer("/thread/historyMode")
        .and_then(Value::as_str)
        .unwrap_or("legacy");
    let model = proc.model.lock().unwrap().clone();

    let final_thread_id = match history_mode {
        "paginated" => {
            let turn_id = target
                .turn_id
                .as_deref()
                .ok_or("Codex did not record the turn containing that message")?;
            let forked = proc.request_and_wait("thread_fork", |id| {
                codex_protocol::thread_fork(
                    id,
                    &thread_id,
                    Some(turn_id),
                    proc.cwd.as_deref(),
                    model.as_deref(),
                )
            })?;
            forked
                .pointer("/thread/id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or("Codex thread/fork did not return a thread id")?
        }
        "legacy" => {
            let forked = proc.request_and_wait("thread_fork", |id| {
                codex_protocol::thread_fork(
                    id,
                    &thread_id,
                    None,
                    proc.cwd.as_deref(),
                    model.as_deref(),
                )
            })?;
            let forked_id = forked
                .pointer("/thread/id")
                .and_then(Value::as_str)
                .ok_or("Codex thread/fork did not return a thread id")?
                .to_string();
            let mut turns = Vec::new();
            for (row_id, _) in drop_rows {
                let turn_id = rewind_target(proc, row_id)?
                    .turn_id
                    .ok_or("Codex did not record the turn containing that message")?;
                if !turns.contains(&turn_id) {
                    turns.push(turn_id);
                }
            }
            let rolled_back = proc.request_and_wait("thread_rollback", |id| {
                codex_protocol::thread_rollback(id, &forked_id, turns.len())
            })?;
            rolled_back
                .pointer("/thread/id")
                .and_then(Value::as_str)
                .unwrap_or(&forked_id)
                .to_string()
        }
        other => return Err(format!("Codex returned an unknown thread history mode: {other}")),
    };

    Ok(final_thread_id)
}

// ─────────────────────────── Threads ───────────────────────────

fn spawn_stdout_reader(
    app: AppCtx,
    session_id: String,
    proc: Arc<ChatProcess>,
    stdout: std::process::ChildStdout,
) {
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            handle_line(&app, &session_id, &proc, &line);
        }
        proc.alive.store(false, Ordering::Relaxed);
        for (_, waiter) in proc.waiters.lock().unwrap().drain() {
            let _ = waiter.send(Err("The agent exited before answering the request".to_string()));
        }
        // Nothing is left to send what was waiting, and leaving it on screen would promise a delivery that
        // can no longer happen.
        {
            let mut turn = proc.turn.lock().unwrap();
            turn.running = false;
            turn.started_at = None;
            turn.waiting.clear();
        }
        let code = proc
            .child
            .lock()
            .unwrap()
            .wait()
            .ok()
            .and_then(|s| s.code())
            .unwrap_or(-1);
        // A process that was released or stopped can be replaced before its pipe closes — the pane sends
        // again, and `start` puts a new process in its place. What this one has to say about itself must
        // not then be read as being about the new one: its exit would mark the fresh conversation dead.
        let superseded = app
            .chat()
            .sessions
            .lock()
            .unwrap()
            .get(&session_id)
            .is_none_or(|current| !Arc::ptr_eq(current, &proc));
        if superseded {
            return;
        }
        crate::session_state::set_alive(&app, &session_id, false);
        emit_queue(&app, &session_id, &proc);
        let stderr = proc.stderr.lock().unwrap().clone();
        let released = proc.released.load(Ordering::Relaxed);
        emit(
            &app,
            &session_id,
            json!({"type":"exited","code":code,"stderr":stderr,"released":released}),
        );
        emit_state(&app, &session_id, AgentState::Waiting);
    });
}

fn spawn_config_lookup(
    app: AppCtx,
    session_id: String,
    proc: Arc<ChatProcess>,
    bin: String,
    cwd: Option<String>,
) {
    std::thread::spawn(move || {
        let keys = config_schema::lookup(&bin, cwd.as_deref());
        if keys.is_empty() {
            return;
        }
        *proc.config_keys.lock().unwrap() = keys.clone();
        emit(&app, &session_id, json!({"type":"configKeys","keys":keys}));
    });
}

fn spawn_stderr_reader(proc: Arc<ChatProcess>, stderr: std::process::ChildStderr) {
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            let mut buf = proc.stderr.lock().unwrap();
            if buf.len() < STDERR_LIMIT {
                buf.push_str(&line);
                buf.push('\n');
            }
        }
    });
}

fn spawn_flusher(app: AppCtx, session_id: String, proc: Arc<ChatProcess>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(FLUSH_INTERVAL);
            let (flush, revision, positions) = {
                let mut timeline = proc.timeline.lock().unwrap();
                timeline.revision += 1;
                let flush = timeline.take_flush();
                let rows = match &flush { TimelineFlush::Rows(rows) => rows.as_slice(), TimelineFlush::Replace(rows) => &rows[rows.len().saturating_sub(SNAPSHOT_PAGE_ROWS)..], TimelineFlush::None => &[] };
                let positions: HashMap<_, _> = rows.iter().filter_map(|row| timeline.at.get(row.id()).map(|index| (row.id().to_owned(), *index))).collect();
                (flush, timeline.revision, positions)
            };
            match flush {
                TimelineFlush::None => {}
                TimelineFlush::Rows(rows) => {
                    emit(&app, &session_id, json!({"type":"rows","positions":positions,"rows":rows.iter().map(snapshot_row).collect::<Vec<_>>(),"revision":revision,"epoch":proc.started_at}));
                }
                TimelineFlush::Replace(rows) => {
                    let start = rows.len().saturating_sub(SNAPSHOT_PAGE_ROWS);
                    emit(&app, &session_id, json!({"type":"replaceRows","positions":positions,"rows":rows[start..].iter().map(snapshot_row).collect::<Vec<_>>(),"hasMore":start > 0,"revision":revision,"epoch":proc.started_at}));
                }
            }
            if !proc.alive.load(Ordering::Relaxed) {
                // One last sweep has just run, so nothing written before exit is lost.
                return;
            }
        }
    });
}

// ─────────────────────────── Line handling ───────────────────────────

fn handle_line(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, line: &str) {
    if proc.kind == SessionKind::Opencode {
        return;
    }
    if proc.kind == SessionKind::Codex {
        handle_codex_line(app, session_id, proc, line);
        return;
    }
    match protocol::parse_line(line) {
        Incoming::Init { session_id: agent_id, model } => {
            let model = model
                .map(|value| crate::agent::claude_models::normalize_id(&value).to_string());
            *proc.agent_session_id.lock().unwrap() = Some(agent_id.clone());
            if model.is_some() {
                *proc.model.lock().unwrap() = model.clone();
            }
            // Persist the id so the conversation can be resumed later, by either engine.
            let changed = {
                let conn = app.db().conn.lock().unwrap();
                crate::db::repo::set_agent_session_id(
                    &conn,
                    session_id,
                    &agent_id,
                    SessionKind::Claude,
                )
                .unwrap_or(false)
            };
            // Tell the clients, exactly as the hook server does for a terminal-driven session. The id is
            // how the rest of the product finds the recording: without the broadcast the Info panel keeps
            // showing "—" for model and context until something else happens to reload the tree.
            if changed {
                app.emit(crate::host::TREE_CHANGED, ());
            }
            emit(
                app,
                session_id,
                json!({"type":"session","agentSessionId":agent_id,"model":model}),
            );
        }
        // A subagent's fragments are dropped rather than routed: the complete message that follows says
        // the same thing, and streaming a second conversation into a card nobody has opened buys nothing.
        Incoming::Delta(delta) => {
            if delta.parent.is_none() {
                handle_delta(proc, delta)
            }
        }
        Incoming::Assistant { message, parent } => {
            clear_api_retry(app, session_id, proc);
            if parent.is_none() {
                if let Some(tokens) = frame_context_tokens(&message) {
                    let mut extras = proc.extras.lock().unwrap();
                    extras.context_tokens = Some(tokens);
                    if let Some(model) = message.get("model").and_then(Value::as_str) {
                        if model != SYNTHETIC_MODEL {
                            extras.usage_model = Some(model.to_string());
                        }
                    }
                    drop(extras);
                    emit_extras(app, session_id, proc);
                }
            }
            handle_assistant(proc, &message, parent.as_deref())
        }
        Incoming::Notice { id, message } => {
            proc.timeline.lock().unwrap().upsert(ChatRow::Notice { id, message });
        }
        Incoming::ApiRetry { attempt, max_retries, delay_ms, message } => {
            proc.extras.lock().unwrap().api_retry = Some(json!({
                "attempt": attempt,
                "maxRetries": max_retries,
                "delayMs": delay_ms,
                "message": message,
            }));
            emit_extras(app, session_id, proc);
        }
        Incoming::Notification { text, priority, timeout_ms } => {
            emit(
                app,
                session_id,
                json!({"type":"notification","text":text,"priority":priority,"timeoutMs":timeout_ms}),
            );
        }
        Incoming::RateLimit(info) => {
            proc.extras.lock().unwrap().rate_limit = Some(info);
            emit_extras(app, session_id, proc);
        }
        Incoming::BackgroundTasks(tasks) => {
            {
                let mut turn = proc.turn.lock().unwrap();
                let current: HashSet<String> = tasks.iter()
                    .filter(|task| !matches!(task.get("status").and_then(Value::as_str), Some("completed" | "failed" | "killed" | "stopped" | "canceled" | "cancelled")))
                    .filter_map(|task| task.get("task_id").and_then(Value::as_str).map(str::to_string)).collect();
                let previous = std::mem::replace(&mut turn.background_tasks, current.clone());
                let deadline = std::time::Instant::now() + TASK_SETTLE_GRACE;
                for ended in previous.difference(&current) {
                    turn.active_tasks.remove(ended);
                    turn.settling.insert(ended.clone(), deadline);
                }
                fold_task_inventory(&mut proc.extras.lock().unwrap(), &tasks);
                // A dropped task whose terminal frames never come must still let the process go: nothing
                // else would call the idle check again once the grace has passed.
                if !turn.settling.is_empty() {
                    let (app, session_id, proc) = (app.clone(), session_id.to_string(), proc.clone());
                    std::thread::spawn(move || {
                        std::thread::sleep(TASK_SETTLE_GRACE);
                        release_if_idle(&app, &session_id, &proc);
                    });
                }
            }
            emit_extras(app, session_id, proc);
            release_if_idle(app, session_id, proc);
        }
        Incoming::LocalCommand { id, text } => {
            proc.timeline.lock().unwrap().upsert(ChatRow::Command { id, text });
        }
        Incoming::User { message, parent } => {
            handle_tool_results(proc, &message, parent.as_deref())
        }
        Incoming::CompactionStarted => handle_compaction(proc, false, None, None),
        Incoming::CompactionFinished { trigger, pre_tokens } => {
            handle_compaction(proc, true, trigger, pre_tokens)
        }
        Incoming::Task { subtype, message } => {
            fold_task_frame(&mut proc.extras.lock().unwrap(), &subtype, &message);
            track_claude_task(proc, &subtype, &message);
            handle_claude_task(proc, &subtype, &message);
            emit_extras(app, session_id, proc);
            release_if_idle(app, session_id, proc);
        }
        Incoming::Result { subtype, duration_ms, total_cost_usd, model_usage } => {
            {
                let mut extras = proc.extras.lock().unwrap();
                extras.api_retry = None;
                if total_cost_usd.is_some() {
                    extras.total_cost_usd = total_cost_usd;
                }
                // The window belongs to the model that answered; a turn that fell back to another model
                // reports both, and the frame's own model name says which one counts.
                let window = extras
                    .usage_model
                    .as_deref()
                    .and_then(|model| model_usage.get(model))
                    .or_else(|| model_usage.as_object().and_then(|map| map.values().next()))
                    .and_then(|entry| entry.get("contextWindow"))
                    .and_then(Value::as_u64);
                if window.is_some() {
                    extras.context_window = window;
                }
            }
            emit_extras(app, session_id, proc);
            // The categorised breakdown is what the meter and the Info panel show; asking after every
            // turn keeps it as fresh as the terminal's own `/context` without a person typing it.
            let id = proc.request_id("context_usage");
            let _ = proc.write(&protocol::control_request(&id, protocol::get_context_usage()));
            handle_turn_end(app, session_id, proc, &subtype, duration_ms)
        }
        Incoming::ControlRequest { request_id, request } => {
            handle_control_request(app, session_id, proc, &request_id, request)
        }
        Incoming::ControlResponse { request_id, response, error } => {
            handle_control_response(app, session_id, proc, &request_id, response, error)
        }
        Incoming::Other => {}
    }
}

/// Fold Claude's task-protocol lifecycle into the Task card that owns the subagent.
///
/// Sidechain frames already provide the detailed child timeline for foreground subagents. The task
/// protocol adds the facts those frames cannot provide reliably: identity before the first frame,
/// progress for background children, and a terminal status even when no child frame was emitted.
fn handle_claude_task(proc: &Arc<ChatProcess>, subtype: &str, message: &Value) {
    let task_id = message.get("task_id").and_then(Value::as_str).filter(|id| !id.is_empty());
    if subtype == "task_started" {
        let task_type = message.get("task_type").and_then(Value::as_str);
        let is_subagent = matches!(task_type, Some("local_agent" | "local_workflow"))
            || (task_type.is_none()
                && message.get("subagent_type").and_then(Value::as_str).is_some());
        if !is_subagent || message.get("skip_transcript").and_then(Value::as_bool) == Some(true) {
            return;
        }
        let (Some(task_id), Some(tool_id)) = (
            task_id,
            message.get("tool_use_id").and_then(Value::as_str).filter(|id| !id.is_empty()),
        ) else {
            return;
        };
        let canonical = {
            let mut ids = proc.subagent_ids.lock().unwrap();
            ids.entry(task_id.to_string()).or_insert_with(|| tool_id.to_string()).clone()
        };
        let explicit_name = {
            let timeline = proc.timeline.lock().unwrap();
            match timeline.get(&canonical) {
                Some(ChatRow::Tool { input, .. }) => input
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .map(str::to_string),
                _ => None,
            }
        };
        let title = if task_type == Some("local_workflow") {
            Some("Workflow".to_string())
        } else {
            explicit_name.or_else(|| {
                message
                    .get("subagent_type")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(str::to_string)
            })
        };
        let description = message
            .get("description")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let prompt = if task_type == Some("local_workflow") {
            description.clone()
        } else {
            message
                .get("prompt")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
        };
        let mut timeline = proc.timeline.lock().unwrap();
        timeline.update_subagent(
            &canonical,
            |facts| {
                if title.is_some() {
                    facts.title = title;
                }
                if description.is_some() {
                    facts.description = description;
                }
            },
            Some("running"),
            None,
        );
        if let Some(prompt) = prompt {
            timeline.upsert_child(
                &canonical,
                ChatRow::User {
                    id: format!("subagent-{task_id}-prompt"),
                    text: prompt,
                    images: Vec::new(),
                    at: None,
                },
            );
        }
        return;
    }

    let Some(task_id) = task_id else { return };
    let Some(tool_id) = proc.subagent_ids.lock().unwrap().get(task_id).cloned() else { return };
    let usage = message.get("usage");
    let status = match if subtype == "task_updated" {
        message.get("patch").and_then(|patch| patch.get("status"))
    } else {
        message.get("status")
    }
    .and_then(Value::as_str)
    {
        Some("pending" | "running" | "paused") => Some("running"),
        Some("completed") => Some("completed"),
        Some("failed" | "error") => Some("failed"),
        Some("killed" | "stopped" | "canceled") => Some("canceled"),
        _ => None,
    };
    let output = (subtype == "task_notification")
        .then(|| {
            message
                .get("summary")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
        })
        .flatten();
    let mut timeline = proc.timeline.lock().unwrap();
    timeline.update_subagent(
        &tool_id,
        |facts| {
            if let Some(usage) = usage {
                if let Some(value) = usage.get("total_tokens").and_then(Value::as_u64) {
                    facts.total_tokens = Some(value);
                }
                if let Some(value) = usage.get("tool_uses").and_then(Value::as_u64) {
                    facts.tool_uses = Some(value);
                }
                if let Some(value) = usage.get("duration_ms").and_then(Value::as_u64) {
                    facts.duration_ms = Some(value);
                }
            }
        },
        status,
        output,
    );
}

fn handle_codex_line(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, line: &str) {
    match codex_protocol::parse_line(line) {
        codex_protocol::Incoming::Response { request_id, result, error } => {
            handle_codex_response(app, session_id, proc, &request_id, result, error)
        }
        codex_protocol::Incoming::Request { request_id, method, params } => {
            handle_codex_request(app, session_id, proc, request_id, &method, params)
        }
        codex_protocol::Incoming::Notification { method, params } => {
            handle_codex_notification(app, session_id, proc, &method, params)
        }
        codex_protocol::Incoming::Other => {}
    }
}

/// Open or resume the thread after the optional capability probes finish.
fn open_codex_thread(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let resume = proc.agent_session_id.lock().unwrap().clone();
    let model = proc.model.lock().unwrap().clone();
    let mode = proc.mode.lock().unwrap().clone();
    let open_kind = if resume.is_some() { "thread_resume" } else { "thread_start" };
    let id = proc.request_id(open_kind);
    let service_tier = proc.service_tier.lock().unwrap().clone();
    let personality = proc.personality.lock().unwrap().clone();
    if let Err(message) = proc.write_codex_mode_request(&id, &mode, &codex_protocol::open_thread(
        &id,
        resume.as_deref(),
        proc.cwd.as_deref(),
        model.as_deref(),
        &mode,
        service_tier.as_deref(),
        personality.as_deref(),
    )) {
        emit(app, session_id, json!({"type":"error","message":message}));
        fail_codex_start(app, session_id, proc);
    }
}

fn handle_codex_response(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    request_id: &str,
    result: Value,
    error: Option<String>,
) {
    let kind = proc.pending.lock().unwrap().remove(request_id);
    let sent_mode = proc.codex_permission_state.lock().unwrap().requests.remove(request_id);
    if let Some(waiter) = proc.waiters.lock().unwrap().remove(request_id) {
        let answer = match error {
            Some(message) => Err(message),
            None => Ok(result),
        };
        let _ = waiter.send(answer);
        return;
    }
    if let Some(message) = error {
        // Collaboration modes are experimental. An older app-server may not expose their catalogue; the
        // conversation still works in its native Default mode. Clear the selection as well as the
        // catalogue so later turns do not send an experimental field that this server has just rejected.
        if kind == Some("collaboration_mode_list") {
            *proc.collaboration_mode.lock().unwrap() = None;
            emit(
                app,
                session_id,
                json!({"type":"collaborationModes","modes":[],"mode":"default"}),
            );
            open_codex_thread(app, session_id, proc);
            return;
        }
        // Codex writes a thread's rollout at its first turn, not when the thread is opened. A conversation
        // that was opened and then closed before any message went out therefore left a thread id behind
        // that Codex cannot resume, and without this every later open of that session would die the same
        // way. There was nothing said in that thread, so a fresh one loses nothing. The stored id is
        // replaced when the new thread's start response is remembered.
        if kind == Some("thread_resume") && message.to_lowercase().contains("no rollout") {
            let abandoned = proc.agent_session_id.lock().unwrap().take();
            eprintln!(
                "chat: Codex could not resume thread {}; starting a new one",
                abandoned.as_deref().unwrap_or("?")
            );
            open_codex_thread(app, session_id, proc);
            return;
        }
        if kind == Some("interrupt") { proc.turn.lock().unwrap().interrupted = false; }
        if kind == Some("skills_list") {
            proc.commands.lock().unwrap().clear();
            emit(app, session_id, json!({"type":"commands","commands":[]}));
        }
        emit(app, session_id, json!({"type":"error","message":message,"request":kind}));
        if matches!(kind, Some("initialize" | "thread_start" | "thread_resume")) {
            fail_codex_start(app, session_id, proc);
        } else if matches!(kind, Some("turn_start" | "review_start")) {
            proc.current_turn.lock().unwrap().take();
            handle_turn_end(app, session_id, proc, "request_failed", None);
        }
        return;
    }
    if let Some(mode) = sent_mode {
        proc.codex_permission_state.lock().unwrap().applied = Some(mode);
        let (mode, pending_permission_mode) = proc.permission_modes();
        emit(
            app,
            session_id,
            json!({
                "type":"settingsChanged",
                "mode":mode,
                "pendingPermissionMode":pending_permission_mode,
            }),
        );
    }
    match kind {
        Some("initialize") => {
            if let Err(message) = proc.write(&codex_protocol::initialized()) {
                emit(app, session_id, json!({"type":"error","message":message}));
                fail_codex_start(app, session_id, proc);
                return;
            }
            request_codex_skills(app, session_id, proc);
            // Query first so a restored Plan selection is validated against the installed app-server
            // before the first queued turn can start.
            let id = proc.request_id("collaboration_mode_list");
            if let Err(message) = proc.write(&codex_protocol::collaboration_mode_list(&id)) {
                emit(app, session_id, json!({"type":"error","message":message}));
                fail_codex_start(app, session_id, proc);
            }
        }
        Some("collaboration_mode_list") => {
            let modes = codex_protocol::parse_collaboration_modes(&result);
            if modes.is_empty() {
                *proc.collaboration_mode.lock().unwrap() = None;
                *proc.collaboration_modes.lock().unwrap() = Vec::new();
                emit(
                    app,
                    session_id,
                    json!({"type":"collaborationModes","modes":[],"mode":"default"}),
                );
                open_codex_thread(app, session_id, proc);
                return;
            }
            let mut selected = proc
                .collaboration_mode
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "default".to_string());
            if !modes.iter().any(|preset| preset.mode == selected) {
                selected = modes
                    .iter()
                    .find(|preset| preset.mode == "default")
                    .or_else(|| modes.first())
                    .map(|preset| preset.mode.clone())
                    .unwrap_or_else(|| "default".to_string());
                *proc.collaboration_mode.lock().unwrap() = Some(selected.clone());
            }
            *proc.collaboration_modes.lock().unwrap() = modes.clone();
            emit(
                app,
                session_id,
                json!({"type":"collaborationModes","modes":modes,"mode":selected}),
            );
            open_codex_thread(app, session_id, proc);
        }
        Some("skills_list") => {
            match skills::codex_commands(&result) {
                Ok(commands) => {
                    *proc.commands.lock().unwrap() = commands.clone();
                    emit(app, session_id, json!({"type":"commands","commands":commands}));
                }
                Err(message) => {
                    proc.commands.lock().unwrap().clear();
                    emit(app, session_id, json!({"type":"commands","commands":[]}));
                    emit(app, session_id, json!({"type":"error","message":message}));
                }
            }
        }
        Some("thread_start" | "thread_resume") => {
            let thread = result.get("thread").unwrap_or(&result);
            let thread_id = thread
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| proc.agent_session_id.lock().unwrap().clone());
            let Some(thread_id) = thread_id else {
                emit(
                    app,
                    session_id,
                    json!({"type":"error","message":"Codex app-server did not return a thread id"}),
                );
                fail_codex_start(app, session_id, proc);
                return;
            };
            let model = result
                .get("model")
                .or_else(|| thread.get("model"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let effort = result
                .get("reasoningEffort")
                .or_else(|| thread.get("reasoningEffort"))
                .and_then(Value::as_str)
                .map(str::to_string);
            remember_codex_thread(app, session_id, proc, &thread_id, model, effort);
            proc.ready.store(true, Ordering::Relaxed);
            start_waiting_message(app, session_id, proc);
        }
        Some("turn_start" | "turn_steer" | "interrupt") | None => {}
        Some(_) => {}
    }
}

/// A failed initialize/start/resume handshake cannot recover on the same app-server process. Mark it
/// unavailable and close it so the next send can start a fresh process instead of joining a queue that no
/// thread will ever drain. The stdout reader owns the final `exited` event once the child actually ends.
fn fail_codex_start(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    proc.ready.store(false, Ordering::Relaxed);
    proc.alive.store(false, Ordering::Relaxed);
    crate::session_state::set_alive(app, session_id, false);
    {
        let mut turn = proc.turn.lock().unwrap();
        turn.running = false;
        turn.started_at = None;
        turn.waiting.clear();
    }
    emit_queue(app, session_id, &proc);
    *proc.stdin.lock().unwrap() = None;
    let _ = proc.child.lock().unwrap().kill();
    emit_state(app, session_id, AgentState::Waiting);
}

fn remember_codex_thread(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    thread_id: &str,
    model: Option<String>,
    effort: Option<String>,
) {
    *proc.agent_session_id.lock().unwrap() = Some(thread_id.to_string());
    // A start/resume response describes the thread's stored defaults. Explicit controls supplied by this
    // pane are turn overrides and remain authoritative; otherwise a restarted process would visibly jump
    // back to the old effort even though the next `turn/start` still carries the newly selected value.
    let keep_auto = crate::agent::session_settings::keeps_automatic_effort(app, session_id);
    {
        let mut current = proc.model.lock().unwrap();
        if current.is_none() {
            *current = model;
        }
    }
    {
        let mut current = proc.effort.lock().unwrap();
        if current.is_none() && !keep_auto {
            *current = effort;
        }
    }
    let model = proc.model.lock().unwrap().clone();
    let effort = proc.effort.lock().unwrap().clone();
    let collaboration_mode = proc.collaboration_mode.lock().unwrap().clone();
    let changed = {
        let conn = app.db().conn.lock().unwrap();
        crate::db::repo::set_agent_session_id(&conn, session_id, thread_id, SessionKind::Codex)
            .unwrap_or(false)
    };
    if changed {
        app.emit(crate::host::TREE_CHANGED, ());
    }
    emit(
        app,
        session_id,
        json!({
            "type":"session",
            "agentSessionId":thread_id,
            "model":model,
            "effort":effort,
            "collaborationMode":collaboration_mode
        }),
    );
}

fn start_waiting_message(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let next = {
        let mut turn = proc.turn.lock().unwrap();
        if turn.running || turn.steering || turn.waiting.is_empty() {
            return;
        }
        turn.running = true;
        turn.waiting.remove(0)
    };
    if prune_finished_tasks(&mut proc.extras.lock().unwrap()) {
        emit_extras(app, session_id, proc);
    }
    emit_queue(app, session_id, &proc);
    if let Err(message) = dispatch(app, session_id, proc, &next.text, &next.images, Some(&next.id)) {
        let mut turn = proc.turn.lock().unwrap();
        turn.running = false;
        turn.started_at = None;
        drop(turn);
        emit(app, session_id, json!({"type":"error","message":message}));
        emit(app, session_id, json!({"type":"turnCompleted"}));
        emit_state(app, session_id, AgentState::Waiting);
    }
}

const MAX_PENDING_CODEX_CHILD_THREADS: usize = 32;
const MAX_PENDING_CODEX_CHILD_EVENTS: usize = 128;

fn codex_notification_thread_id(params: &Value) -> Option<&str> {
    params
        .get("threadId")
        .and_then(Value::as_str)
        .or_else(|| params.pointer("/thread/id").and_then(Value::as_str))
}

fn buffer_codex_child_notification(
    proc: &Arc<ChatProcess>,
    thread_id: &str,
    method: &str,
    params: Value,
) {
    let mut pending = proc.codex_pending_child_events.lock().unwrap();
    if !pending.contains_key(thread_id) && pending.len() >= MAX_PENDING_CODEX_CHILD_THREADS {
        return;
    }
    let events = pending.entry(thread_id.to_string()).or_default();
    if events.len() < MAX_PENDING_CODEX_CHILD_EVENTS {
        events.push((method.to_string(), params));
    }
}

fn codex_child_status(params: &Value) -> &'static str {
    match params.pointer("/turn/status").and_then(Value::as_str) {
        Some("completed" | "success") => "completed",
        Some("interrupted" | "canceled" | "cancelled") => "canceled",
        Some(_) => "failed",
        None => "completed",
    }
}

fn codex_child_total_tokens(params: &Value) -> Option<u64> {
    [
        "/tokenUsage/last/totalTokens",
        "/tokenUsage/last/total_tokens",
        "/tokenUsage/total/totalTokens",
        "/tokenUsage/total/total_tokens",
    ]
    .into_iter()
    .find_map(|pointer| params.pointer(pointer).and_then(Value::as_u64))
}

/// Route one Codex child thread into the Task card that started it.
///
/// Child lifecycle never touches the root turn queue or current turn id. That separation is what lets a
/// child finish, fail, or compact without making the main composer look idle or ending the user's turn.
fn handle_codex_child_notification(
    proc: &Arc<ChatProcess>,
    parent_path: &[String],
    method: &str,
    params: &Value,
) {
    match method {
        "turn/started" => proc.timeline.lock().unwrap().update_subagent_at(
            parent_path,
            |_| {},
            Some("running"),
            None,
        ),
        "turn/completed" => proc.timeline.lock().unwrap().update_subagent_at(
            parent_path,
            |_| {},
            Some(codex_child_status(params)),
            None,
        ),
        "item/started" => {
            if let Some(item) = params.get("item") {
                upsert_codex_item_at(proc, item, true, Some(parent_path));
            }
        }
        "item/completed" => {
            if let Some(item) = params.get("item") {
                upsert_codex_item_at(proc, item, false, Some(parent_path));
            }
        }
        "item/agentMessage/delta" => codex_text_delta_at(proc, params, false, Some(parent_path)),
        "item/reasoning/textDelta" | "item/reasoning/summaryTextDelta" => {
            codex_text_delta_at(proc, params, true, Some(parent_path))
        }
        "item/commandExecution/outputDelta" => {
            codex_tool_delta_at(proc, params, true, Some(parent_path))
        }
        "item/fileChange/outputDelta" => {
            codex_tool_delta_at(proc, params, false, Some(parent_path))
        }
        "turn/plan/updated" => upsert_codex_plan_at(proc, params, Some(parent_path)),
        "thread/tokenUsage/updated" => {
            if let Some(total) = codex_child_total_tokens(params) {
                proc.timeline.lock().unwrap().update_subagent_at(
                    parent_path,
                    |facts| facts.total_tokens = Some(total),
                    None,
                    None,
                );
            }
        }
        _ => {}
    }
}

fn register_codex_subagent_threads(proc: &Arc<ChatProcess>, task_path: RowPath, item: &Value) {
    let root = proc.agent_session_id.lock().unwrap().clone();
    let mut thread_ids = item
        .get("receiverThreadIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if let Some(id) = item
        .get("agentThreadId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    {
        thread_ids.push(id.to_string());
    }
    thread_ids.sort();
    thread_ids.dedup();
    thread_ids.retain(|id| root.as_deref() != Some(id));

    for thread_id in thread_ids {
        proc.codex_subagent_ids
            .lock()
            .unwrap()
            .insert(thread_id.clone(), task_path.clone());
        let events = proc
            .codex_pending_child_events
            .lock()
            .unwrap()
            .remove(&thread_id)
            .unwrap_or_default();
        for (method, params) in events {
            handle_codex_child_notification(proc, &task_path, &method, &params);
        }
    }
}

fn request_codex_skills(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let id = proc.request_id("skills_list");
    if let Err(message) = proc.write(&skills::list_request(&id, proc.cwd.as_deref())) {
        emit(app, session_id, json!({"type":"error","message":message}));
    }
}

fn handle_codex_notification(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    method: &str,
    params: Value,
) {
    if method == "skills/changed" {
        proc.commands.lock().unwrap().clear();
        emit(app, session_id, json!({"type":"commands","commands":[]}));
        request_codex_skills(app, session_id, proc);
        return;
    }
    // Resolution belongs to the request table, not a timeline. Process it even when the request came from
    // a child thread so a permission answered elsewhere cannot leave a stale card behind.
    if method == "serverRequest/resolved" {
        let Some(id) = params.get("requestId") else { return };
        let key = codex_protocol::request_key(id);
        if proc.permissions.lock().unwrap().remove(&key).is_some() {
            emit(app, session_id, json!({"type":"permissionResolved","id":key}));
            emit_state(app, session_id, AgentState::Working);
        }
        return;
    }

    // App-server multiplexes root and subagent threads over the same pipe. A child has its own recording;
    // route its work into the owning Task card rather than dropping it or spilling it into the root.
    if proc.ready.load(Ordering::Relaxed) {
        let current = proc.agent_session_id.lock().unwrap().clone();
        let incoming = codex_notification_thread_id(&params).map(str::to_string);
        if incoming.as_deref().is_some_and(|id| current.as_deref().is_some_and(|root| id != root)) {
            let thread_id = incoming.unwrap();
            let parent_path = proc.codex_subagent_ids.lock().unwrap().get(&thread_id).cloned();
            if let Some(parent_path) = parent_path {
                handle_codex_child_notification(proc, &parent_path, method, &params);
            } else {
                buffer_codex_child_notification(proc, &thread_id, method, params);
            }
            return;
        }
    }
    // What Codex says around its items: failures and warnings, context usage, streamed plan and summary
    // parts, the turn's diff, and typed input to a running command.
    if codex::handle_notification(app, session_id, proc, method, &params) {
        return;
    }
    match method {
        "thread/started" => {
            let thread = params.get("thread").unwrap_or(&params);
            if let Some(id) = thread.get("id").and_then(Value::as_str) {
                let model = thread.get("model").and_then(Value::as_str).map(str::to_string);
                let effort = thread
                    .get("reasoningEffort")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                remember_codex_thread(app, session_id, proc, id, model, effort);
            }
        }
        "turn/started" => {
            if let Some(id) = params.pointer("/turn/id").and_then(Value::as_str) {
                *proc.current_turn.lock().unwrap() = Some(id.to_string());
                for row_id in proc.pending_user_rows.lock().unwrap().drain(..) {
                    proc.user_targets.lock().unwrap().insert(
                        row_id,
                        super::history::RewindTarget {
                            message_id: id.to_string(),
                            turn_id: Some(id.to_string()),
                            text: String::new(),
                        },
                    );
                }
                if proc.turn.lock().unwrap().interrupted {
                    let request_id = proc.request_id("interrupt");
                    if let Some(thread_id) = proc.agent_session_id.lock().unwrap().clone() {
                        let _ = proc.write(&codex_protocol::turn_interrupt(&request_id, &thread_id, id));
                    }
                }
            }
            emit_state(app, session_id, AgentState::Working);
        }
        "turn/completed" => {
            *proc.current_turn.lock().unwrap() = None;
            let status = params
                .pointer("/turn/status")
                .and_then(Value::as_str)
                .unwrap_or("completed");
            let subtype = if matches!(status, "completed" | "success") {
                "success"
            } else {
                status
            };
            // Read before the turn ends, which is what clears the interrupt mark.
            let interrupted = proc.turn.lock().unwrap().interrupted;
            let failure = codex::turn_failure(proc, &params);
            handle_turn_end(app, session_id, proc, subtype, None);
            // The generic "ended with: failed" row is replaced in place by Codex's own reason — the
            // usage limit that was hit, the context window that overflowed — when it gave one.
            if subtype == "failed" && !interrupted {
                if let Some(message) = failure {
                    proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                        id: "e-failed".to_string(),
                        message,
                    });
                }
            }
        }
        "item/started" => {
            if let Some(item) = params.get("item") {
                upsert_codex_item_at(proc, item, true, None);
            }
        }
        "item/completed" => {
            if let Some(item) = params.get("item") {
                upsert_codex_item_at(proc, item, false, None);
            }
        }
        "item/agentMessage/delta" => codex_text_delta_at(proc, &params, false, None),
        "item/reasoning/textDelta" | "item/reasoning/summaryTextDelta" => {
            codex_text_delta_at(proc, &params, true, None)
        }
        "item/commandExecution/outputDelta" => codex_tool_delta_at(proc, &params, true, None),
        "item/fileChange/outputDelta" => codex_tool_delta_at(proc, &params, false, None),
        "turn/plan/updated" => upsert_codex_plan_at(proc, &params, None),
        // Current Codex emits a canonical `contextCompaction` item as well as aggregate telemetry. The
        // item is used above so one compaction cannot produce two markers.
        "thread/compacted" => {}
        _ => {}
    }
}

#[cfg(test)]
fn codex_text_delta(proc: &Arc<ChatProcess>, params: &Value, reasoning: bool) {
    codex_text_delta_at(proc, params, reasoning, None);
}

fn codex_text_delta_at(
    proc: &Arc<ChatProcess>,
    params: &Value,
    reasoning: bool,
    parent_path: Option<&[String]>,
) {
    let Some(id) = params.get("itemId").and_then(Value::as_str) else { return };
    let delta = params.get("delta").and_then(Value::as_str).unwrap_or("");
    let text = append(proc, id, delta);
    let row = if reasoning {
        ChatRow::Reasoning { id: id.to_string(), text, streaming: true }
    } else {
        ChatRow::Assistant {
            id: id.to_string(),
            text,
            streaming: true,
            at: Some(now_ms() as i64),
            model: None,
            duration_ms: None,
        }
    };
    place_codex(&mut proc.timeline.lock().unwrap(), parent_path, row);
}

#[cfg(test)]
fn codex_tool_delta(proc: &Arc<ChatProcess>, params: &Value, command_output: bool) {
    codex_tool_delta_at(proc, params, command_output, None);
}

fn codex_tool_delta_at(
    proc: &Arc<ChatProcess>,
    params: &Value,
    command_output: bool,
    parent_path: Option<&[String]>,
) {
    let Some(id) = params.get("itemId").and_then(Value::as_str) else { return };
    let raw = params.get("delta").and_then(Value::as_str).unwrap_or("");
    let decoded;
    let delta = if command_output {
        decoded = codex_output_text(raw);
        decoded.as_str()
    } else {
        raw
    };
    let mut timeline = proc.timeline.lock().unwrap();
    let (name, input, output, subagent, children) = match timeline.find_at(parent_path.unwrap_or_default(), id) {
        Some(ChatRow::Tool { name, input, output, subagent, children, .. }) => {
            (name.clone(), input.clone(), output.clone().unwrap_or_default(), subagent.clone(), children.clone())
        }
        _ => ("tool".to_string(), Value::Null, String::new(), None, Vec::new()),
    };
    place_codex(&mut timeline, parent_path, ChatRow::Tool {
        id: id.to_string(),
        name,
        input,
        output: Some(format!("{output}{delta}")),
        is_error: false,
        status: "running",
        subagent,
        children,
    });
}

/// Item output deltas are text. Base64 belongs to command/exec/outputDelta, a different endpoint.
/// Never infer an encoding from the payload: ordinary command output may itself look like Base64.
fn codex_output_text(value: &str) -> String {
    value.to_string()
}

#[cfg(test)]
fn upsert_codex_item(proc: &Arc<ChatProcess>, item: &Value, started: bool) {
    upsert_codex_item_at(proc, item, started, None);
}

fn upsert_codex_item_at(
    proc: &Arc<ChatProcess>,
    item: &Value,
    started: bool,
    parent_path: Option<&[String]>,
) {
    let Some(id) = item.get("id").and_then(Value::as_str) else { return };
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
    if item_type == "collabAgentToolCall" {
        let receiver_ids = item
            .get("receiverThreadIds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        let routes = proc.codex_subagent_ids.lock().unwrap();
        // Wait, status and message operations are collaboration plumbing, not additional agents. A card is
        // needed only when the call introduces a child whose SubAgentActivity frame has not arrived yet.
        if receiver_ids.is_empty() || receiver_ids.iter().all(|thread| routes.contains_key(*thread)) {
            return;
        }
    }
    if item_type == "subAgentActivity" {
        let child_thread = item.get("agentThreadId").and_then(Value::as_str);
        let existing_path = child_thread
            .and_then(|thread| proc.codex_subagent_ids.lock().unwrap().get(thread).cloned());
        if let Some(existing_path) = existing_path {
            let status = match item.get("kind").and_then(Value::as_str) {
                Some("completed") => "completed",
                Some("interrupted" | "canceled" | "cancelled") => "canceled",
                _ => "running",
            };
            let path = item.get("agentPath").and_then(Value::as_str).map(str::to_string);
            proc.timeline.lock().unwrap().update_subagent_at(
                &existing_path,
                |facts| {
                    if facts.description.is_none() {
                        facts.description = path;
                    }
                },
                Some(status),
                None,
            );
            return;
        }
    }
    let (previous_output, previous_subagent, previous_children) = match proc
        .timeline
        .lock()
        .unwrap()
        .find_at(parent_path.unwrap_or_default(), id)
    {
        Some(ChatRow::Tool { output, subagent, children, .. }) => {
            (output.clone(), subagent.clone(), children.clone())
        }
        _ => (None, None, Vec::new()),
    };
    let row = match item_type {
        "agentMessage" => {
            let text = item.get("text").and_then(Value::as_str).unwrap_or("");
            if text.trim().is_empty() {
                return;
            }
            proc.buffers.lock().unwrap().insert(id.to_string(), text.to_string());
            ChatRow::Assistant {
                id: id.to_string(),
                text: text.to_string(),
                streaming: started,
                at: Some(now_ms() as i64),
                model: None,
                duration_ms: None,
            }
        }
        "reasoning" => {
            // Summary is the user-facing form when present; `content` is its fallback, not a second copy
            // to append. Current app-server responses can contain both.
            let parts = ["summary", "content"]
                .into_iter()
                .find_map(|key| {
                    let values = item.get(key).and_then(Value::as_array)?;
                    let parts = values.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>();
                    (!parts.is_empty()).then_some(parts)
                })
                .unwrap_or_default();
            let text = if parts.is_empty() {
                proc.buffers.lock().unwrap().get(id).cloned().unwrap_or_default()
            } else {
                parts.join("\n\n")
            };
            if text.trim().is_empty() && !started {
                return;
            }
            proc.buffers.lock().unwrap().insert(id.to_string(), text.clone());
            ChatRow::Reasoning { id: id.to_string(), text, streaming: started }
        }
        "commandExecution" => {
            let status = codex_status(item, started);
            let output = item
                .get("aggregatedOutput")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or(previous_output);
            ChatRow::Tool {
                id: id.to_string(),
                name: "Bash".to_string(),
                input: json!({
                    "command":item.get("command").cloned().unwrap_or(Value::Null),
                    "cwd":item.get("cwd").cloned().unwrap_or(Value::Null)
                }),
                output,
                is_error: status == "failed",
                status,
                subagent: None,
                children: Vec::new(),
            }
        }
        "fileChange" => {
            let status = codex_status(item, started);
            let changes = item.get("changes").cloned().unwrap_or_else(|| json!([]));
            let first_path = changes
                .as_array()
                .and_then(|values| values.first())
                .and_then(|value| value.get("path"))
                .cloned()
                .unwrap_or(Value::Null);
            ChatRow::Tool {
                id: id.to_string(),
                name: "FileChange".to_string(),
                input: json!({"file_path":first_path,"changes":changes}),
                output: previous_output,
                is_error: status == "failed",
                status,
                subagent: None,
                children: Vec::new(),
            }
        }
        "mcpToolCall" => {
            let status = codex_status(item, started);
            let server = item.get("server").and_then(Value::as_str).unwrap_or("MCP");
            let tool = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
            ChatRow::Tool {
                id: id.to_string(),
                name: format!("{server}/{tool}"),
                input: item.get("arguments").cloned().unwrap_or(Value::Null),
                output: codex_result_text(item.get("result").or_else(|| item.get("error"))),
                is_error: status == "failed" || item.get("error").is_some_and(|value| !value.is_null()),
                status,
                subagent: None,
                children: Vec::new(),
            }
        }
        "dynamicToolCall" => {
            let status = codex_status(item, started);
            ChatRow::Tool {
                id: id.to_string(),
                name: item.get("tool").and_then(Value::as_str).unwrap_or("tool").to_string(),
                input: item.get("arguments").cloned().unwrap_or(Value::Null),
                output: codex_result_text(item.get("contentItems")),
                is_error: status == "failed" || item.get("success").and_then(Value::as_bool) == Some(false),
                status,
                subagent: None,
                children: Vec::new(),
            }
        }
        "subAgentActivity" => {
            let status = match item.get("kind").and_then(Value::as_str) {
                Some("completed") => "completed",
                Some("interrupted" | "canceled" | "cancelled") => "canceled",
                _ => "running",
            };
            let path = item.get("agentPath").and_then(Value::as_str);
            ChatRow::Tool {
                id: id.to_string(),
                name: "Task".to_string(),
                input: json!({
                    "description":path,
                    "agentThreadId":item.get("agentThreadId").cloned().unwrap_or(Value::Null)
                }),
                output: previous_output,
                is_error: false,
                status,
                subagent: Some(SubagentInfo {
                    title: Some("Codex subagent".to_string()),
                    description: path.map(str::to_string),
                    ..previous_subagent.unwrap_or_default()
                }),
                children: previous_children,
            }
        }
        "collabAgentToolCall" => {
            let status = codex_status(item, started);
            let mut facts = previous_subagent.unwrap_or_default();
            if let Some(value) = item.get("tool").and_then(Value::as_str) {
                facts.title = Some(value.to_string());
            }
            if let Some(value) = item.get("prompt").and_then(Value::as_str) {
                facts.description = Some(value.to_string());
            }
            if let Some(value) = item.get("model").and_then(Value::as_str) {
                facts.model = Some(value.to_string());
            }
            ChatRow::Tool {
                id: id.to_string(),
                name: "Task".to_string(),
                input: json!({
                    "description":item.get("tool").cloned().unwrap_or(Value::Null),
                    "prompt":item.get("prompt").cloned().unwrap_or(Value::Null),
                    "model":item.get("model").cloned().unwrap_or(Value::Null),
                    "receiverThreadIds":item.get("receiverThreadIds").cloned().unwrap_or_else(|| json!([]))
                }),
                output: None,
                is_error: status == "failed",
                status,
                subagent: Some(facts),
                children: previous_children,
            }
        }
        "webSearch" => {
            let status = if started { "running" } else { "completed" };
            ChatRow::Tool {
                id: id.to_string(),
                name: "WebSearch".to_string(),
                input: json!({"query":item.get("query").cloned().unwrap_or(Value::Null)}),
                output: codex_result_text(item.get("results")),
                is_error: false,
                status,
                subagent: None,
                children: Vec::new(),
            }
        }
        "imageView" => ChatRow::Tool {
            id: id.to_string(),
            name: "Read".to_string(),
            input: json!({"file_path":item.get("path").cloned().unwrap_or(Value::Null)}),
            output: None,
            is_error: false,
            status: if started { "running" } else { "completed" },
            subagent: None,
            children: Vec::new(),
        },
        "imageGeneration" => {
            let status = codex_status(item, started);
            ChatRow::Tool {
                id: id.to_string(),
                name: "ImageGeneration".to_string(),
                input: json!({"revisedPrompt":item.get("revisedPrompt").cloned().unwrap_or(Value::Null)}),
                output: codex_result_text(
                    item.get("savedPath")
                        .filter(|value| !value.is_null())
                        .or_else(|| item.get("result"))
                        .or_else(|| item.get("failure")),
                ),
                is_error: status == "failed",
                status,
                subagent: None,
                children: Vec::new(),
            }
        }
        "plan" => ChatRow::Tool {
            id: id.to_string(),
            name: "ExitPlanMode".to_string(),
            input: json!({"plan":item.get("text").cloned().unwrap_or(Value::Null)}),
            output: None,
            is_error: false,
            status: if started { "running" } else { "completed" },
            subagent: None,
            children: Vec::new(),
        },
        "contextCompaction" => {
            if let Some(parent_path) = parent_path {
                place_codex(
                    &mut proc.timeline.lock().unwrap(),
                    Some(parent_path),
                    ChatRow::Compaction {
                        id: id.to_string(),
                        status: if started { "loading" } else { "completed" },
                        trigger: Some("auto".to_string()),
                        pre_tokens: None,
                    },
                );
            } else {
                handle_compaction(proc, !started, Some("auto".to_string()), None);
            }
            return;
        }
        _ => return,
    };
    place_codex(&mut proc.timeline.lock().unwrap(), parent_path, row);
    if matches!(item_type, "collabAgentToolCall" | "subAgentActivity") {
        let mut task_path = parent_path.unwrap_or_default().to_vec();
        task_path.push(id.to_string());
        register_codex_subagent_threads(proc, task_path, item);
    }
}

fn codex_status(item: &Value, started: bool) -> &'static str {
    if started {
        return "running";
    }
    match item.get("status").and_then(Value::as_str) {
        Some("inProgress") => "running",
        Some("completed") | None => "completed",
        Some(_) => "failed",
    }
}

fn codex_result_text(value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(value) => serde_json::to_string_pretty(value).ok(),
    }
}

#[cfg(test)]
fn upsert_codex_plan(proc: &Arc<ChatProcess>, params: &Value) {
    upsert_codex_plan_at(proc, params, None);
}

fn upsert_codex_plan_at(
    proc: &Arc<ChatProcess>,
    params: &Value,
    parent_path: Option<&[String]>,
) {
    let Some(turn_id) = params.get("turnId").and_then(Value::as_str) else { return };
    let todos = params
        .get("plan")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|step| {
            let status = match step.get("status").and_then(Value::as_str) {
                Some("inProgress") => "in_progress",
                Some("completed") => "completed",
                _ => "pending",
            };
            json!({"content":step.get("step").cloned().unwrap_or(Value::Null),"status":status})
        })
        .collect::<Vec<_>>();
    place_codex(&mut proc.timeline.lock().unwrap(), parent_path, ChatRow::Tool {
        id: format!("plan-{turn_id}"),
        name: "TodoWrite".to_string(),
        input: json!({"todos":todos}),
        output: params.get("explanation").and_then(Value::as_str).map(str::to_string),
        is_error: false,
        status: "completed",
        subagent: None,
        children: Vec::new(),
    });
}

fn handle_codex_request(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    request_id: Value,
    method: &str,
    params: Value,
) {
    let key = codex_protocol::request_key(&request_id);
    let mut payload = match method {
        // The card offers every standing rule this request allows: for the session, as an execpolicy
        // prefix, or as a network rule for the host the command wants to reach.
        "item/commandExecution/requestApproval" => codex::command_approval_payload(&key, &params),
        "item/fileChange/requestApproval" => {
            let item_id = params.get("itemId").and_then(Value::as_str).unwrap_or("");
            let input = match proc.timeline.lock().unwrap().get(item_id) {
                Some(ChatRow::Tool { input, .. }) => input.clone(),
                _ => json!({"reason":params.get("reason").cloned().unwrap_or(Value::Null)}),
            };
            json!({
                "id":key,
                "tool_name":"FileChange",
                "display_name":"Codex",
                "description":params.get("reason").cloned().unwrap_or(Value::Null),
                "input":input
            })
        }
        "item/tool/requestUserInput" | "tool/requestUserInput" => {
            let raw = params.get("questions").and_then(Value::as_array).cloned().unwrap_or_default();
            let questions = raw
                .iter()
                .filter_map(|question| {
                    let text = question.get("question").and_then(Value::as_str)?;
                    let header = question.get("header").and_then(Value::as_str).unwrap_or(text);
                    Some(json!({
                        "id":question.get("id").cloned().unwrap_or_else(|| json!(text)),
                        "question":text,
                        "header":header,
                        "options":question.get("options").cloned().unwrap_or_else(|| json!([])),
                        "multiSelect":false,
                        "allowOther":question.get("isOther").and_then(Value::as_bool).unwrap_or(true),
                        "allowEmpty":!params.get("isBlocking").and_then(Value::as_bool).unwrap_or(true)
                    }))
                })
                .collect::<Vec<_>>();
            json!({
                "id":key,
                "tool_name":"AskUserQuestion",
                "display_name":"Codex",
                "input":{"questions":questions},
                "_codexQuestions":raw,
                "_codexItemId":params.get("itemId").cloned().unwrap_or(Value::Null)
            })
        }
        "item/permissions/requestApproval" => json!({
            "id":key,
            "tool_name":"Permissions",
            "display_name":"Codex",
            "description":params.get("reason").cloned().unwrap_or(Value::Null),
            "input":{
                "cwd":params.get("cwd").cloned().unwrap_or(Value::Null),
                "permissions":params.get("permissions").cloned().unwrap_or_else(|| json!({}))
            },
            "permission_suggestions":[{
                "type":"codexAcceptForSession",
                "destination":"session",
                "subject":"these permissions"
            }]
        }),
        // The same form card Claude's elicitations use: a schema drawn field by field, or a URL to
        // finish something in the browser, with accept, decline and cancel as its three answers.
        "mcpServer/elicitation/request" => codex::elicitation_payload(&key, &params),
        _ => {
            let _ = proc.write(&json!({
                "id":request_id,
                "error":{"code":-32601,"message":format!("Unsupported Codex request: {method}")}
            }));
            return;
        }
    };
    payload["_codexRequestId"] = request_id;
    payload["_codexMethod"] = json!(method);
    payload["_codexParams"] = params.clone();
    // File changes may be accepted for the rest of the session; commands carry their own, richer offers.
    if method == "item/fileChange/requestApproval" {
        payload["permission_suggestions"] = json!([{
            "type":"codexAcceptForSession",
            "destination":"session",
            "subject":"file changes"
        }]);
    }
    proc.permissions.lock().unwrap().insert(key.clone(), payload.clone());
    emit(app, session_id, json!({"type":"permission","request":payload}));
    emit_state(app, session_id, AgentState::Asking);
}

/// A turn finished. Whatever was typed while it ran now goes out, one message per turn.
///
/// The state stays `Working` while the queue drains: from where the user sits the agent never stopped, and
/// flashing the session back to idle between two of their own messages would only look like a stall.
fn handle_turn_end(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    subtype: &str,
    reported_duration_ms: Option<u64>,
) {
    let completed_at = now_ms();
    let (next, interrupted, started_at) = {
        let mut turn = proc.turn.lock().unwrap();
        let interrupted = std::mem::take(&mut turn.interrupted);
        let started_at = turn.started_at.take();
        let next = if turn.steering || turn.waiting.is_empty() {
            turn.running = false;
            None
        } else {
            // Stays running: the message about to go out is the next turn, without an idle gap between.
            Some(turn.waiting.remove(0))
        };
        (next, interrupted, started_at)
    };
    if let Some(duration_ms) = reported_duration_ms.or_else(|| {
        started_at.map(|started| completed_at.saturating_sub(started))
    }) {
        proc.timeline.lock().unwrap().finish_latest_turn(duration_ms);
    }
    // A turn stopped on purpose reports itself as failed. That is the interrupt working, not a fault.
    if subtype != "success" && !interrupted {
        proc.timeline.lock().unwrap().upsert(ChatRow::Error {
            id: format!("e-{subtype}"),
            message: format!("The turn ended with: {subtype}"),
        });
    }
    if interrupted {
        emit(app, session_id, json!({"type":"turnInterrupted"}));
    }
    let Some(item) = next else {
        emit_state(app, session_id, AgentState::Waiting);
        emit(app, session_id, json!({"type":"turnCompleted"}));
        // The view that asked for this went away while the turn was running; now that the turn is
        // over and nothing waits behind it, the process has nothing left to do.
        release_if_idle(app, session_id, proc);
        return;
    };
    emit_queue(app, session_id, &proc);
    if let Err(e) = dispatch(app, session_id, proc, &item.text, &item.images, Some(&item.id)) {
        let mut turn = proc.turn.lock().unwrap();
        turn.running = false;
        turn.started_at = None;
        drop(turn);
        emit(app, session_id, json!({"type":"error","message":e}));
        emit(app, session_id, json!({"type":"turnCompleted"}));
        emit_state(app, session_id, AgentState::Waiting);
    }
}

/// Ask the agent to stop what it is doing, with the turn's own lock already held.
///
/// The agent ends an interrupted turn by reporting a failure. That failure is the interrupt working, so
/// the turn is marked as asked-for and the timeline grows no error row for it — but only when a turn was
/// actually running, or the mark would sit there and swallow the next real failure.
fn interrupt_locked(proc: &Arc<ChatProcess>, turn: &mut TurnQueue) -> Result<(), String> {
    let previous = turn.interrupted;
    let result = send_interrupt_locked(proc, turn);
    if result.is_err() { turn.interrupted = previous; }
    result
}

fn send_interrupt_locked(proc: &Arc<ChatProcess>, turn: &mut TurnQueue) -> Result<(), String> {
    turn.interrupted = turn.running;
    if proc.kind == SessionKind::Opencode {
        if !turn.running {
            return Ok(());
        }
        return opencode::abort(proc);
    }
    if proc.kind == SessionKind::Codex {
        if !turn.running {
            return Ok(());
        }
        let thread_id = proc
            .agent_session_id
            .lock()
            .unwrap()
            .clone()
            .ok_or("Codex has not opened its thread yet")?;
        let Some(turn_id) = proc.current_turn.lock().unwrap().clone() else {
            // `turn/started` sends the interrupt as soon as the native turn id is known.
            return Ok(());
        };
        let id = proc.request_id("interrupt");
        proc.write(&codex_protocol::turn_interrupt(&id, &thread_id, &turn_id))
    } else {
        let id = proc.request_id("interrupt");
        proc.write(&protocol::control_request(&id, protocol::interrupt()))
    }
}

/// Submit a prompt to the active turn without changing its clock or draining its queue.
fn dispatch_steer(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    text: &str,
    images: &[ChatImage],
    message_id: Option<&str>,
) -> Result<(), String> {
    let commands = proc.commands.lock().unwrap().clone();
    let normalized = skills::native_text(text, &commands);
    let text = normalized.as_ref();
    let row_id = message_id.map(str::to_owned).unwrap_or_else(|| format!("u-{}", proc.next_request.fetch_add(1, Ordering::Relaxed)));
    let sent_at = now_ms();
    match proc.kind {
        SessionKind::Codex => {
            let thread_id = proc.agent_session_id.lock().unwrap().clone()
                .ok_or("Codex has not opened its thread yet")?;
            let turn_id = proc.current_turn.lock().unwrap().clone()
                .ok_or("Codex is still starting its turn. Retry steering once the turn has started.")?;
            // A refusal belongs to this submission, not to the running turn.
            proc.request_and_wait("turn_steer", |id| {
                skills::with_inputs(codex_protocol::turn_steer(id, &thread_id, &turn_id, text, images), text, &commands)
            })?;
            proc.user_targets.lock().unwrap().insert(row_id.clone(), super::history::RewindTarget {
                message_id: turn_id.clone(), turn_id: Some(turn_id), text: text.to_string(),
            });
        }
        SessionKind::Opencode => opencode::steer(proc, &row_id, text, images)?,
        _ => proc.write(&protocol::user_message(text, images))?,
    }
    if proc.kind != SessionKind::Opencode {
        crate::agent::server::try_auto_rename(app, session_id, text);
    }
    proc.timeline.lock().unwrap().upsert(ChatRow::User {
        at: Some(sent_at as i64), id: row_id, text: text.to_string(), images: images.to_vec(),
    });
    emit(app, session_id, json!({"type":"steerAccepted"}));
    Ok(())
}

/// Write a user turn and show it immediately, without waiting for the agent to echo it back.
///
/// The caller has already marked the turn running; this only puts the message on the wire.
fn dispatch(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    text: &str,
    images: &[ChatImage],
    message_id: Option<&str>,
) -> Result<(), String> {
    let commands = proc.commands.lock().unwrap().clone();
    let normalized = skills::native_text(text, &commands);
    let text = normalized.as_ref();
    let sent_at = now_ms();
    let (turn_started_at, newly_started) = {
        let mut turn = proc.turn.lock().unwrap();
        match turn.started_at {
            Some(started_at) => (started_at, false),
            None => {
                turn.started_at = Some(sent_at);
                (sent_at, true)
            }
        }
    };
    let row_id = message_id.map(str::to_owned).unwrap_or_else(|| format!("u-{}", proc.next_request.fetch_add(1, Ordering::Relaxed)));
    proc.timeline.lock().unwrap().upsert(ChatRow::User {
        at: Some(sent_at as i64),
        id: row_id.clone(),
        text: text.to_string(),
        images: images.to_vec(),
    });
    if proc.kind == SessionKind::Opencode {
        opencode::dispatch(app, session_id, proc, &row_id, text, images)?;
    } else if proc.kind == SessionKind::Codex {
        let thread_id = proc
            .agent_session_id
            .lock()
            .unwrap()
            .clone()
            .ok_or("Codex has not opened its thread yet")?;
        let current_turn = proc.current_turn.lock().unwrap().clone();
        let id = proc.request_id(if current_turn.is_some() { "turn_steer" } else { "turn_start" });
        if let Some(turn_id) = current_turn {
            proc.user_targets.lock().unwrap().insert(
                row_id,
                super::history::RewindTarget {
                    message_id: turn_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    text: text.to_string(),
                },
            );
            proc.write(&skills::with_inputs(codex_protocol::turn_steer(&id, &thread_id, &turn_id, text, images), text, &commands))?;
        } else {
            proc.pending_user_rows.lock().unwrap().push(row_id);
            let model = proc.model.lock().unwrap().clone();
            let effort = proc.effort.lock().unwrap().clone();
            let mode = proc.mode.lock().unwrap().clone();
            let collaboration_mode = proc.collaboration_mode.lock().unwrap().clone();
            let collaboration_preset_effort = collaboration_mode.as_deref().and_then(|selected| {
                proc.collaboration_modes
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|preset| preset.mode == selected)
                    .and_then(|preset| preset.reasoning_effort.clone())
            });
            let service_tier = proc.service_tier.lock().unwrap().clone();
            let personality = proc.personality.lock().unwrap().clone();
            proc.write_codex_mode_request(&id, &mode, &skills::with_inputs(codex_protocol::turn_start(
                &id,
                &thread_id,
                text,
                images,
                codex_protocol::TurnOptions {
                    model: model.as_deref(),
                    effort: effort.as_deref(),
                    mode: &mode,
                    collaboration_mode: collaboration_mode.as_deref(),
                    collaboration_preset_effort: collaboration_preset_effort.as_deref(),
                    service_tier: service_tier.as_deref(),
                    personality: personality.as_deref(),
                },
            ), text, &commands))?;
        }
    } else {
        proc.write(&protocol::user_message(text, images))?;
    }
    // Direct sends and dequeued messages share this path. Rename only after a successful write;
    // OpenCode supplies its own title through session.updated and must retain its placeholder until then.
    if proc.kind != SessionKind::Opencode {
        crate::agent::server::try_auto_rename(app, session_id, text);
    }
    if newly_started {
        emit(
            app,
            session_id,
            json!({"type":"turnStarted","startedAt":turn_started_at}),
        );
    }
    emit_state(app, session_id, AgentState::Working);
    Ok(())
}

/// Mark the conversation as summarized, and replace that mark once the summary is in place.
///
/// Both halves address the same row, so the marker turns from "compacting" into the finished note where
/// it already sits rather than leaving a stale one behind. The count only moves when a compaction ends,
/// which is what pairs a boundary with the start that preceded it — and a boundary arriving without one,
/// as an automatic compaction may, simply creates the row itself.
fn handle_compaction(
    proc: &Arc<ChatProcess>,
    done: bool,
    trigger: Option<String>,
    pre_tokens: Option<u64>,
) {
    let n = proc.compactions.load(Ordering::Relaxed);
    proc.timeline.lock().unwrap().upsert(ChatRow::Compaction {
        id: format!("c-{n}"),
        status: if done { "completed" } else { "loading" },
        trigger,
        pre_tokens,
    });
    if done {
        proc.compactions.fetch_add(1, Ordering::Relaxed);
    }
}

/// Row id for one content block of one message. Fragments and the final message share it, so the complete
/// text lands on the row its fragments built.
fn block_id(message_id: Option<&str>, index: usize) -> String {
    format!("{}:{index}", message_id.unwrap_or("msg"))
}

fn handle_delta(proc: &Arc<ChatProcess>, delta: Delta) {
    if matches!(delta.kind, DeltaKind::MessageStart) {
        // A message starts with no blocks delivered; a resumed id must not inherit a stale count.
        if let Some(id) = delta.message_id.as_deref() {
            proc.frame_blocks.lock().unwrap().remove(id);
        }
        *proc.current_message.lock().unwrap() = delta.message_id;
        return;
    }
    // Fragments name no message, so the one opened most recently is the one they belong to.
    let message_id = delta
        .message_id
        .or_else(|| proc.current_message.lock().unwrap().clone());
    let id = block_id(message_id.as_deref(), delta.index);
    match delta.kind {
        DeltaKind::Start { block_type, tool_name, tool_use_id } => {
            proc.buffers.lock().unwrap().insert(id.clone(), String::new());
            // A tool call gets its card as soon as it starts, so the view shows it running rather than
            // appearing only once it has finished.
            if block_type == "tool_use" {
                if let (Some(name), Some(call_id)) = (tool_name, tool_use_id) {
                    proc.timeline.lock().unwrap().upsert(ChatRow::Tool {
                        id: call_id,
                        name,
                        input: Value::Null,
                        output: None,
                        is_error: false,
                        status: "running",
                        subagent: None,
                        children: Vec::new(),
                    });
                }
            }
        }
        DeltaKind::Text(text) => {
            let full = append(proc, &id, &text);
            proc.timeline.lock().unwrap().upsert(ChatRow::Assistant {
                at: Some(now_ms() as i64),
                id,
                text: full,
                streaming: true,
                model: None,
                duration_ms: None,
            });
        }
        DeltaKind::Thinking(text) => {
            let full = append(proc, &id, &text);
            proc.timeline.lock().unwrap().upsert(ChatRow::Reasoning {
                id,
                text: full,
                streaming: true,
            });
        }
        DeltaKind::Stop => {}
        DeltaKind::MessageStop => *proc.current_message.lock().unwrap() = None,
        DeltaKind::MessageStart => {}
    }
}

fn append(proc: &Arc<ChatProcess>, id: &str, text: &str) -> String {
    let mut buffers = proc.buffers.lock().unwrap();
    let buf = buffers.entry(id.to_string()).or_default();
    buf.push_str(text);
    buf.clone()
}

/// Apply a complete assistant message, replacing whatever its fragments built.
/// The model an assistant frame reports when no inference happened behind it.
///
/// The agent answers a refused turn — a model the account cannot run, a context window it has not paid
/// for — with an ordinary-looking assistant message whose text is the API error. Only this model value
/// distinguishes it from a real answer, so without the check the failure reads as the agent calmly
/// reciting an error, and the turn looks like it ran.
const SYNTHETIC_MODEL: &str = "<synthetic>";

fn handle_assistant(proc: &Arc<ChatProcess>, message: &Value, parent: Option<&str>) {
    let message_id = message.get("id").and_then(Value::as_str);
    let Some(blocks) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    let refused = message.get("model").and_then(Value::as_str) == Some(SYNTHETIC_MODEL);
    // Blocks are numbered across the whole message, not within this frame. See `frame_blocks`.
    let offset = {
        let mut counts = proc.frame_blocks.lock().unwrap();
        let seen = counts.entry(message_id.unwrap_or("msg").to_string()).or_insert(0);
        let offset = *seen;
        *seen += blocks.len();
        offset
    };
    let mut timeline = proc.timeline.lock().unwrap();
    if let (Some(parent), Some(model)) = (
        parent,
        message
            .get("model")
            .and_then(Value::as_str)
            .filter(|model| !model.is_empty() && *model != SYNTHETIC_MODEL),
    ) {
        timeline.update_subagent(
            parent,
            |facts| facts.model = Some(model.to_string()),
            None,
            None,
        );
    }
    for (i, block) in blocks.iter().enumerate() {
        let index = offset + i;
        let id = block_id(message_id, index);
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = block.get("text").and_then(Value::as_str).unwrap_or("");
                if text.trim().is_empty() {
                    continue;
                }
                // Replace rather than add: the fragments already built an assistant row under this id
                // before the frame revealed that nothing ran behind it.
                if refused {
                    place(&mut timeline, parent, ChatRow::Error { id, message: text.to_string() });
                } else {
                    place(
                        &mut timeline,
                        parent,
                        ChatRow::Assistant {
                            id,
                            text: text.to_string(),
                            streaming: false,
                            at: Some(now_ms() as i64),
                            model: None,
                            duration_ms: None,
                        },
                    );
                }
            }
            Some("thinking") => {
                let text = block.get("thinking").and_then(Value::as_str).unwrap_or("");
                if !text.trim().is_empty() {
                    place(
                        &mut timeline,
                        parent,
                        ChatRow::Reasoning { id, text: text.to_string(), streaming: false },
                    );
                }
            }
            Some("tool_use") => {
                let Some(call_id) = block.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let (subagent, children) = match timeline.find(parent, call_id) {
                    Some(ChatRow::Tool { subagent, children, .. }) => {
                        (subagent.clone(), children.clone())
                    }
                    _ => (None, Vec::new()),
                };
                place(
                    &mut timeline,
                    parent,
                    ChatRow::Tool {
                        id: call_id.to_string(),
                        name: block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("tool")
                            .to_string(),
                        input: block.get("input").cloned().unwrap_or(Value::Null),
                        output: None,
                        is_error: false,
                        status: "running",
                        subagent,
                        children,
                    },
                );
            }
            _ => {}
        }
    }
}

/// Put a row where it belongs: in the conversation, or inside the tool call that produced it.
fn place(timeline: &mut Timeline, parent: Option<&str>, row: ChatRow) {
    match parent {
        Some(parent) => timeline.upsert_child(parent, row),
        None => timeline.upsert(row),
    }
}

/// Put a Codex row at the root or inside the complete path of nested Task cards.
fn place_codex(timeline: &mut Timeline, parent_path: Option<&[String]>, row: ChatRow) {
    timeline.upsert_at(parent_path.unwrap_or_default(), row);
}

/// Fill in the results of tool calls. They arrive as a user-role message, separate from the call itself.
fn handle_tool_results(proc: &Arc<ChatProcess>, message: &Value, parent: Option<&str>) {
    let Some(blocks) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    let mut timeline = proc.timeline.lock().unwrap();
    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }
        let Some(call_id) = block.get("tool_use_id").and_then(Value::as_str) else {
            continue;
        };
        let is_error = block.get("is_error").and_then(Value::as_bool).unwrap_or(false);
        let text = result_text(block);
        // Keep the name and input from the call this answers; a result without a known call still shows.
        let (name, input, subagent, children) = match timeline.find(parent, call_id) {
            Some(ChatRow::Tool { name, input, subagent, children, .. }) => {
                (name.clone(), input.clone(), subagent.clone(), children.clone())
            }
            _ => ("tool".to_string(), Value::Null, None, Vec::new()),
        };
        place(
            &mut timeline,
            parent,
            ChatRow::Tool {
                id: call_id.to_string(),
                name,
                input,
                output: Some(text),
                is_error,
                status: if is_error { "failed" } else { "completed" },
                subagent,
                // A subagent's own steps outlive its result: the card keeps showing what it did.
                children,
            },
        );
    }
}

/// Tool result content is either a string or a list of text and image blocks.
fn result_text(block: &Value) -> String {
    match block.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|it| match it.get("type").and_then(Value::as_str) {
                Some("text") => it.get("text").and_then(Value::as_str).map(str::to_string),
                Some("image") => Some("[image]".to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

/// The agent is asking us something. Only permission questions need a human, and they go to the view.
fn handle_control_request(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    request_id: &str,
    request: Value,
) {
    // Both are questions only a person can answer, and both wait on the same queue: a tool asking for
    // permission, and an MCP server asking for input. Anything else the agent might ask is left unanswered
    // on purpose; nothing here registered hooks or SDK-side MCP servers, so nothing else should arrive.
    if !matches!(
        request.get("subtype").and_then(Value::as_str),
        Some("can_use_tool" | "elicitation")
    ) {
        return;
    }
    let mut payload = request.clone();
    payload["id"] = json!(request_id);
    proc.permissions
        .lock()
        .unwrap()
        .insert(request_id.to_string(), payload.clone());
    emit(app, session_id, json!({"type":"permission","request":payload}));
    emit_state(app, session_id, AgentState::Asking);
}

/// An answer to something we asked. Only the startup handshake carries data we keep.
fn handle_control_response(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    request_id: &str,
    response: Value,
    error: Option<String>,
) {
    let kind = proc.pending.lock().unwrap().remove(request_id);
    if let Some(waiter) = proc.waiters.lock().unwrap().remove(request_id) {
        let answer = match error {
            Some(message) => Err(message),
            None => Ok(response),
        };
        let _ = waiter.send(answer);
        return;
    }
    if let Some(message) = error {
        emit(
            app,
            session_id,
            json!({"type":"error","message":message,"request":kind}),
        );
        return;
    }
    match kind {
        Some("list_models") => {
            let models = response
                .get("models")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if !models.is_empty() {
                *proc.claude_models.lock().unwrap() = models;
                emit(app, session_id, json!({"type":"models"}));
            }
            return;
        }
        Some("context_usage") => {
            if response.is_object() {
                proc.extras.lock().unwrap().context_usage = Some(response);
                emit_extras(app, session_id, proc);
            }
            return;
        }
        Some("initialize") => {}
        _ => return,
    }
    let commands = match skills::claude_commands(&response) {
        Ok(commands) => commands,
        Err(message) => {
            emit(app, session_id, json!({"type":"error","message":message}));
            Vec::new()
        }
    };
    *proc.commands.lock().unwrap() = commands.clone();
    emit(app, session_id, json!({"type":"commands","commands":commands}));
    if proc.kind != SessionKind::Claude {
        return;
    }
    // The handshake is done; ask for what the composer shows and apply the switches the launch could not
    // carry. Answers come back through this same function, keyed by request kind.
    let mut requests = vec![("list_models", protocol::list_models())];
    if proc.effort.lock().unwrap().as_deref() == Some(protocol::THINKING_OFF) {
        requests.push(("set_max_thinking_tokens", protocol::set_max_thinking_tokens(Some(0))));
    }
    if proc.extras.lock().unwrap().fast_mode {
        requests.push(("apply_flag_settings", protocol::apply_flag_settings(json!({"fastMode":true}))));
    }
    for (kind, request) in requests {
        let id = proc.request_id(kind);
        let _ = proc.write(&protocol::control_request(&id, request));
    }
}

/// Publish the whole `extras` object. See `ClaudeExtras`.
fn emit_extras(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let extras = proc.extras.lock().unwrap().published();
    emit(app, session_id, json!({"type":"extras","extras":extras}));
}

/// A frame that proves the API answered ends any retry banner still showing.
fn clear_api_retry(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let cleared = proc.extras.lock().unwrap().api_retry.take().is_some();
    if cleared {
        emit_extras(app, session_id, proc);
    }
}

/// Context carried by one assistant frame: what the API was sent, cache hits and writes included.
fn frame_context_tokens(message: &Value) -> Option<u64> {
    let usage = message.get("usage")?;
    let field = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let total = field("input_tokens") + field("cache_creation_input_tokens") + field("cache_read_input_tokens");
    (total > 0).then_some(total)
}

// ─────────────────────────── Emission ───────────────────────────

fn emit(app: &AppCtx, session_id: &str, payload: Value) {
    app.emit(&event_name(session_id), payload);
}

/// Publish the whole queue rather than one change to it. Every client showing this conversation has to
/// agree on what is waiting, and a list is the only description that cannot drift.
fn emit_queue(app: &AppCtx, session_id: &str, proc: &ChatProcess) {
    let (waiting, revision) = {
        let mut turn = proc.turn.lock().unwrap();
        turn.revision += 1;
        (turn.waiting.clone(), turn.revision)
    };
    emit(app, session_id, json!({"type":"queued","items":waiting,"revision":revision,"epoch":proc.started_at}));
}

/// Report work state through the same channel PTY sessions use, so the sidebar, tab dots, and notifications
/// treat a chat session exactly like any other agent session.
fn emit_state(app: &AppCtx, session_id: &str, state: AgentState) {
    app.emit(
        &StatusSignal::event_name(session_id),
        StatusSignal::State { state, silent: false, authoritative: true },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_info_falls_back_to_session_model_and_preserves_usage() {
        for kind in [SessionKind::Codex, SessionKind::Claude] {
            let manager = ChatManager::new();
            let proc = inert_process(kind);
            manager.sessions.lock().unwrap().insert("s".into(), proc.clone());
            *proc.model.lock().unwrap() = Some("session-model".into());
            for structured in [false, true] {
                for reported in [None, Some("usage-model")] {
                    {
                        let mut extras = proc.extras.lock().unwrap();
                        extras.context_tokens = Some(154000);
                        extras.context_window = Some(258400);
                        extras.usage_model = reported.map(str::to_string);
                        extras.context_usage = structured.then(|| json!({
                            "totalTokens": 154000, "maxTokens": 258400
                        }));
                    }
                    let info = manager.context_info("s").unwrap();
                    assert_eq!(info.model.as_deref(), Some(reported.unwrap_or("session-model")));
                    assert_eq!(info.context_tokens, Some(154000));
                    assert_eq!(info.context_limit, 258400);
                }
            }
            proc.extras.lock().unwrap().context_usage = Some(json!({
                "model": "reported-model", "totalTokens": 154000, "maxTokens": 258400
            }));
            assert_eq!(manager.context_info("s").unwrap().model.as_deref(), Some("reported-model"));
        }
    }

    pub(super) fn inert_process(kind: SessionKind) -> Arc<ChatProcess> {
        Arc::new(ChatProcess {
            kind,
            action: Mutex::new(()),
            cwd: None,
            stdin: Mutex::new(None),
            child: Mutex::new(Command::new("true").spawn().unwrap()),
            timeline: Mutex::new(Timeline::default()),
            permissions: Mutex::new(HashMap::new()),
            commands: Mutex::new(Vec::new()),
            config_keys: Mutex::new(Vec::new()),
            subagent_ids: Mutex::new(HashMap::new()),
            codex_subagent_ids: Mutex::new(HashMap::new()),
            codex_pending_child_events: Mutex::new(HashMap::new()),
            opencode: Mutex::new(opencode::OpencodeState::default()),
            service_tier: Mutex::new(None),
            personality: Mutex::new(None),
            codex_turn_error: Mutex::new(None),
            extras: Mutex::new(ClaudeExtras::default()),
            claude_models: Mutex::new(Vec::new()),
            agent_session_id: Mutex::new(None),
            settings_change: Mutex::new(()),
            model: Mutex::new(None),
            effort: Mutex::new(None),
            mode: Mutex::new("default".to_string()),
            codex_permission_state: Mutex::new(CodexPermissionState::default()),
            collaboration_mode: Mutex::new(None),
            collaboration_modes: Mutex::new(Vec::new()),
            current_turn: Mutex::new(None),
            ready: AtomicBool::new(true),
            buffers: Mutex::new(HashMap::new()),
            current_message: Mutex::new(None),
            frame_blocks: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            waiters: Mutex::new(HashMap::new()),
            user_targets: Mutex::new(HashMap::new()),
            pending_user_rows: Mutex::new(Vec::new()),
            turn: Mutex::new(TurnQueue::default()),
            stderr: Mutex::new(String::new()),
            alive: AtomicBool::new(true),
            release_when_idle: AtomicBool::new(false),
            released: AtomicBool::new(false),
            next_request: AtomicU64::new(1),
            pid: 0,
            started_at: 0,
            compactions: AtomicU64::new(0),
        })
    }

    fn timeline_of(lines: &[&str]) -> Vec<ChatRow> {
        let proc = inert_process(SessionKind::Claude);
        for line in lines {
            match protocol::parse_line(line) {
                Incoming::Delta(d) => {
                    if d.parent.is_none() {
                        handle_delta(&proc, d)
                    }
                }
                Incoming::Assistant { message, parent } => {
                    handle_assistant(&proc, &message, parent.as_deref())
                }
                Incoming::LocalCommand { id, text } => {
                    proc.timeline.lock().unwrap().upsert(ChatRow::Command { id, text });
                }
                Incoming::User { message, parent } => {
                    handle_tool_results(&proc, &message, parent.as_deref())
                }
                Incoming::CompactionStarted => handle_compaction(&proc, false, None, None),
                Incoming::CompactionFinished { trigger, pre_tokens } => {
                    handle_compaction(&proc, true, trigger, pre_tokens)
                }
                Incoming::Task { subtype, message } => {
                    handle_claude_task(&proc, &subtype, &message)
                }
                _ => {}
            }
        }
        let rows = proc.timeline.lock().unwrap().rows.clone();
        rows
    }

    /// Fragments build a row, and the complete message replaces it in place rather than adding a second one.
    ///
    /// This is the regression that matters most: the fragments name no message, so if the reader forgets
    /// which one is open, the finished answer lands on a different row and the reply appears twice.
    #[test]
    fn streamed_text_is_replaced_by_the_final_message() {
        let rows = timeline_of(&[
            r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"he"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"llo"}}}"#,
            r#"{"type":"assistant","message":{"id":"msg","content":[{"type":"text","text":"hello"}]}}"#,
        ]);
        assert_eq!(rows.len(), 1);
        match &rows[0] {
            ChatRow::Assistant { text, streaming, .. } => {
                assert_eq!(text, "hello");
                assert!(!streaming, "the final message is not still streaming");
            }
            other => panic!("expected an assistant row, got {other:?}"),
        }
    }

    /// A refused turn must not read as an answer.
    ///
    /// Switching to a model the account cannot run is accepted by the agent and then refused by the server,
    /// which reports the refusal as an assistant message carrying the API error. The `<synthetic>` model is
    /// the only thing distinguishing it, and the fragments have already drawn an assistant row by then, so
    /// the frame has to replace that row rather than leaving the error sitting there as prose.
    #[test]
    fn a_refused_turn_replaces_its_assistant_row_with_an_error() {
        let rows = timeline_of(&[
            r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"API Error"}}}"#,
            r#"{"type":"assistant","message":{"id":"msg","model":"<synthetic>","content":[{"type":"text","text":"API Error: Usage credits required for 1M context"}]}}"#,
        ]);
        assert_eq!(rows.len(), 1, "the error replaces the row, it does not add one");
        match &rows[0] {
            ChatRow::Error { message, .. } => {
                assert!(message.contains("Usage credits"), "the agent's own wording is kept: {message}");
            }
            other => panic!("expected an error row, got {other:?}"),
        }
    }

    /// A slash command the agent answered itself gets a row of its own, not an error and not an answer.
    #[test]
    fn a_locally_answered_command_lands_on_its_own_row() {
        let rows = timeline_of(&[
            r#"{"type":"assistant","message":{"id":"m1","model":"<synthetic>","content":[{"type":"text","text":"Set effort level to high"}]},"local_command_source":"<local-command-stdout>Set effort level to high</local-command-stdout>"}"#,
        ]);
        match rows.as_slice() {
            [ChatRow::Command { text, .. }] => assert_eq!(text, "Set effort level to high"),
            other => panic!("expected one command row, got {other:?}"),
        }
    }

    /// A real answer keeps reading as an answer; only the synthetic model turns one into an error.
    #[test]
    fn a_real_model_still_produces_an_assistant_row() {
        let rows = timeline_of(&[
            r#"{"type":"assistant","message":{"id":"msg","model":"claude-opus-4-6","content":[{"type":"text","text":"hello"}]}}"#,
        ]);
        assert!(matches!(rows.as_slice(), [ChatRow::Assistant { .. }]), "got {rows:?}");
    }

    /// A tool call and the result answering it are one row, and the result keeps the call's name and input.
    #[test]
    fn tool_result_lands_on_the_call_it_answers() {
        let rows = timeline_of(&[
            r#"{"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","id":"tu_1","name":"Bash","input":{"command":"ls"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu_1","content":"a.rs"}]}}"#,
        ]);
        assert_eq!(rows.len(), 1);
        match &rows[0] {
            ChatRow::Tool { name, input, output, status, is_error, .. } => {
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls");
                assert_eq!(output.as_deref(), Some("a.rs"));
                assert_eq!(*status, "completed");
                assert!(!is_error);
            }
            other => panic!("expected a tool row, got {other:?}"),
        }
    }

    /// A failed tool is marked failed, so the card can say so.
    #[test]
    fn failed_tool_result_is_marked() {
        let rows = timeline_of(&[
            r#"{"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","id":"tu_1","name":"Bash","input":{}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu_1","content":"boom","is_error":true}]}}"#,
        ]);
        match &rows[0] {
            ChatRow::Tool { status, is_error, .. } => {
                assert_eq!(*status, "failed");
                assert!(is_error);
            }
            other => panic!("expected a tool row, got {other:?}"),
        }
    }

    /// The agent delivers one frame per block, each numbered from zero. The text frame must land on the
    /// row the text deltas built — not on the thinking row, which is what happened before this was fixed:
    /// the thinking vanished and the answer showed twice.
    #[test]
    fn per_block_frames_keep_their_place_in_the_message() {
        let rows = timeline_of(&[
            r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"m"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"thinking"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}}"#,
            r#"{"type":"assistant","message":{"id":"m","content":[{"type":"thinking","thinking":"hmm"}]}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_start","index":1,"content_block":{"type":"text"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"answer"}}}"#,
            r#"{"type":"assistant","message":{"id":"m","content":[{"type":"text","text":"answer"}]}}"#,
        ]);
        assert_eq!(rows.len(), 2, "one reasoning row and one answer row, got {rows:?}");
        assert!(matches!(&rows[0], ChatRow::Reasoning { text, streaming: false, .. } if text == "hmm"));
        assert!(matches!(&rows[1], ChatRow::Assistant { text, streaming: false, .. } if text == "answer"));
    }

    /// Reasoning stays a separate row from the answer that follows it.
    #[test]
    fn reasoning_and_answer_are_separate_rows() {
        let rows = timeline_of(&[
            r#"{"type":"assistant","message":{"id":"m","content":[{"type":"thinking","thinking":"hmm"},{"type":"text","text":"answer"}]}}"#,
        ]);
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], ChatRow::Reasoning { .. }));
        assert!(matches!(rows[1], ChatRow::Assistant { .. }));
    }

    /// Canonical Codex items reuse the same rows the Claude adapter produces, including final replacement
    /// of streamed text and a readable file-change card.
    #[test]
    fn codex_items_map_to_shared_timeline_rows() {
        for text in ["TWFu", "SGVsbG8=", "  TWFu\n", "中文\n"] {
            assert_eq!(codex_output_text(text), text);
        }

        let proc = inert_process(SessionKind::Codex);
        codex_text_delta(
            &proc,
            &json!({"itemId":"answer-1","delta":"hel"}),
            false,
        );
        codex_text_delta(
            &proc,
            &json!({"itemId":"answer-1","delta":"lo"}),
            false,
        );
        upsert_codex_item(
            &proc,
            &json!({"id":"answer-1","type":"agentMessage","text":"hello"}),
            false,
        );
        upsert_codex_item(
            &proc,
            &json!({
                "id":"cmd-1",
                "type":"commandExecution",
                "command":"cargo check",
                "cwd":"/work",
                "status":"inProgress"
            }),
            true,
        );
        codex_tool_delta(
            &proc,
            &json!({"itemId":"cmd-1","delta":"ok"}),
            true,
        );
        upsert_codex_item(
            &proc,
            &json!({
                "id":"cmd-1",
                "type":"commandExecution",
                "command":"cargo check",
                "cwd":"/work",
                "status":"completed"
            }),
            false,
        );
        upsert_codex_item(
            &proc,
            &json!({
                "id":"edit-1",
                "type":"fileChange",
                "status":"completed",
                "changes":[{"path":"src/main.rs","kind":"update","diff":"+done"}]
            }),
            false,
        );
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert!(matches!(
            &rows[0],
            ChatRow::Assistant { text, streaming: false, .. } if text == "hello"
        ));
        assert!(matches!(
            &rows[1],
            ChatRow::Tool { name, output: Some(output), status, .. }
                if name == "Bash" && output == "ok" && *status == "completed"
        ));
        assert!(matches!(
            &rows[2],
            ChatRow::Tool { name, input, status, .. }
                if name == "FileChange" && input["file_path"] == "src/main.rs" && *status == "completed"
        ));
    }

    /// Codex plans and reasoning are not flattened into assistant prose: the view can fold reasoning and
    /// render plan progress with its existing structured cards.
    #[test]
    fn codex_reasoning_and_plan_stay_structured() {
        let proc = inert_process(SessionKind::Codex);
        codex_text_delta(
            &proc,
            &json!({"itemId":"reason-1","delta":"checking"}),
            true,
        );
        upsert_codex_item(
            &proc,
            &json!({
                "id":"reason-1",
                "type":"reasoning",
                "summary":["checked"],
                "content":["private duplicate"]
            }),
            false,
        );
        upsert_codex_plan(
            &proc,
            &json!({
                "turnId":"turn-1",
                "plan":[
                    {"step":"Inspect","status":"completed"},
                    {"step":"Implement","status":"inProgress"}
                ]
            }),
        );
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert!(matches!(&rows[0], ChatRow::Reasoning { text, streaming: false, .. } if text == "checked"));
        assert!(matches!(
            &rows[1],
            ChatRow::Tool { name, input, .. }
                if name == "TodoWrite" && input["todos"][1]["status"] == "in_progress"
        ));
    }

    /// A registered Codex child thread contributes steps to its Task card without changing root turn state.
    #[test]
    fn codex_child_thread_activity_stays_inside_its_task_card() {
        let app = ctx("codex-child");
        let proc = inert_process(SessionKind::Codex);
        *proc.agent_session_id.lock().unwrap() = Some("root-thread".to_string());
        proc.turn.lock().unwrap().running = true;
        *proc.current_turn.lock().unwrap() = Some("root-turn".to_string());

        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/started",
            json!({
                "threadId":"root-thread",
                "item":{
                    "id":"task-1",
                    "type":"collabAgentToolCall",
                    "tool":"explorer",
                    "prompt":"Inspect the parser",
                    "receiverThreadIds":["child-thread"]
                }
            }),
        );
        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/started",
            json!({
                "threadId":"child-thread",
                "item":{
                    "id":"cmd-1",
                    "type":"commandExecution",
                    "command":"rg parser",
                    "status":"inProgress"
                }
            }),
        );
        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/commandExecution/outputDelta",
            json!({"threadId":"child-thread","itemId":"cmd-1","delta":"found"}),
        );
        handle_codex_notification(
            &app,
            "s",
            &proc,
            "turn/completed",
            json!({"threadId":"child-thread","turn":{"id":"child-turn","status":"completed"}}),
        );
        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/completed",
            json!({
                "threadId":"root-thread",
                "item":{
                    "id":"task-finished-marker",
                    "type":"subAgentActivity",
                    "kind":"completed",
                    "agentThreadId":"child-thread",
                    "agentPath":"/root/explorer"
                }
            }),
        );
        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/completed",
            json!({
                "threadId":"root-thread",
                "item":{
                    "id":"wait-control",
                    "type":"collabAgentToolCall",
                    "tool":"wait",
                    "status":"completed",
                    "receiverThreadIds":[]
                }
            }),
        );

        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(), 1, "child items must not become root rows");
        match &rows[0] {
            ChatRow::Tool { id, status, subagent: Some(facts), children, .. } => {
                assert_eq!(id, "task-1");
                assert_eq!(*status, "completed");
                assert_eq!(facts.title.as_deref(), Some("explorer"));
                assert!(matches!(
                    &children[0],
                    ChatRow::Tool { name, output: Some(output), .. }
                        if name == "Bash" && output == "found"
                ));
            }
            other => panic!("expected a Task card, got {other:?}"),
        }
        assert_eq!(proc.current_turn.lock().unwrap().as_deref(), Some("root-turn"));
        assert!(proc.turn.lock().unwrap().running, "a child completion must not end the root turn");
    }

    /// App-server can announce child output before the root collab item. It is replayed once the route exists.
    #[test]
    fn codex_child_events_wait_for_their_parent_task() {
        let app = ctx("codex-child-early");
        let proc = inert_process(SessionKind::Codex);
        *proc.agent_session_id.lock().unwrap() = Some("root-thread".to_string());

        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/agentMessage/delta",
            json!({"threadId":"child-early","itemId":"answer-1","delta":"early"}),
        );
        assert!(proc.timeline.lock().unwrap().rows.is_empty());
        assert_eq!(
            proc.codex_pending_child_events
                .lock()
                .unwrap()
                .get("child-early")
                .map(Vec::len),
            Some(1)
        );

        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/started",
            json!({
                "threadId":"root-thread",
                "item":{
                    "id":"task-early",
                    "type":"collabAgentToolCall",
                    "prompt":"Work in parallel",
                    "receiverThreadIds":["child-early"]
                }
            }),
        );
        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/completed",
            json!({
                "threadId":"child-early",
                "item":{"id":"answer-1","type":"agentMessage","text":"early answer"}
            }),
        );

        assert!(proc.codex_pending_child_events.lock().unwrap().is_empty());
        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(), 1);
        assert!(matches!(
            &rows[0],
            ChatRow::Tool { children, .. }
                if matches!(&children[0], ChatRow::Assistant { text, streaming: false, .. } if text == "early answer")
        ));
    }

    /// A child that delegates again keeps the grandchild under its own Task card.
    #[test]
    fn codex_nested_child_threads_keep_their_real_task_hierarchy() {
        let app = ctx("codex-nested-child");
        let proc = inert_process(SessionKind::Codex);
        *proc.agent_session_id.lock().unwrap() = Some("root-thread".to_string());

        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/started",
            json!({
                "threadId":"root-thread",
                "item":{
                    "id":"task-parent",
                    "type":"subAgentActivity",
                    "kind":"started",
                    "agentThreadId":"child-thread",
                    "agentPath":"/root/parent"
                }
            }),
        );
        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/started",
            json!({
                "threadId":"child-thread",
                "item":{
                    "id":"task-nested",
                    "type":"subAgentActivity",
                    "kind":"started",
                    "agentThreadId":"grandchild-thread",
                    "agentPath":"/root/parent/nested"
                }
            }),
        );
        handle_codex_notification(
            &app,
            "s",
            &proc,
            "item/completed",
            json!({
                "threadId":"grandchild-thread",
                "item":{
                    "id":"grandchild-command",
                    "type":"commandExecution",
                    "command":"pwd",
                    "aggregatedOutput":"/workspace\n",
                    "status":"completed"
                }
            }),
        );

        let rows = proc.timeline.lock().unwrap().rows.clone();
        assert_eq!(rows.len(), 1, "nested work must not spill into the root timeline");
        match &rows[0] {
            ChatRow::Tool { id, children, .. } => {
                assert_eq!(id, "task-parent");
                assert_eq!(children.len(), 1);
                match &children[0] {
                    ChatRow::Tool { id, children, .. } => {
                        assert_eq!(id, "task-nested");
                        assert!(matches!(
                            &children[0],
                            ChatRow::Tool { id, name, output: Some(output), .. }
                                if id == "grandchild-command" && name == "Bash" && output == "/workspace\n"
                        ));
                    }
                    other => panic!("expected a nested Task card, got {other:?}"),
                }
            }
            other => panic!("expected a root Task card, got {other:?}"),
        }
        assert_eq!(
            proc.codex_subagent_ids.lock().unwrap().get("grandchild-thread"),
            Some(&vec!["task-parent".to_string(), "task-nested".to_string()])
        );
    }

    // ── The queue ──

    /// A headless context over a throwaway database, matching the helper used elsewhere.
    pub(super) fn ctx(tag: &str) -> AppCtx {
        let dir = std::env::temp_dir().join(format!("vlx-chat-queue-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("t.db")).unwrap();
        AppCtx::Headless(Arc::new(crate::host::HeadlessHost::new(dir, db)))
    }

    #[test]
    fn codex_model_switch_clears_effort_override_for_auto() {
        let app = ctx("codex-auto-effort");
        let manager = ChatManager::new();
        let proc = inert_process(SessionKind::Codex);
        manager.sessions.lock().unwrap().insert("s".into(), proc.clone());

        for automatic in [None, Some(""), Some(" \t ")] {
            manager.set_effort(&app, "s", Some("high")).unwrap();
            assert_eq!(manager.snapshot("s").effort.as_deref(), Some("high"));
            manager.set_model("s", Some("gpt-5.6-luna")).unwrap();
            manager.set_effort(&app, "s", automatic).unwrap();
            assert_eq!(manager.snapshot("s").model.as_deref(), Some("gpt-5.6-luna"));
            assert_eq!(manager.snapshot("s").effort, None);
        }
        manager.set_effort(&app, "s", Some("low")).unwrap();
        assert_eq!(manager.snapshot("s").effort.as_deref(), Some("low"));
    }

    /// A subagent's work belongs to the card that started it, not to the conversation.
    ///
    /// The agent runs a subagent inside a `Task` call and reports every line it produces as an ordinary
    /// frame tagged with that call's id. Without routing they land in the main timeline, so a Task that
    /// runs twenty tools buries the answer under twenty cards nobody asked for.
    #[test]
    fn a_subagents_work_lands_inside_its_task_card() {
        let rows = timeline_of(&[
            r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"task-1","name":"Task","input":{"description":"List files"}}]}}"#,
            r#"{"type":"assistant","parent_tool_use_id":"task-1","message":{"id":"m2","content":[{"type":"tool_use","id":"bash-1","name":"Bash","input":{"command":"ls"}}]}}"#,
            r#"{"type":"user","parent_tool_use_id":"task-1","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"bash-1","content":"a.txt"}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"task-1","content":"one file"}]}}"#,
        ]);
        assert_eq!(rows.len(), 1, "only the Task card belongs to the conversation");
        match &rows[0] {
            ChatRow::Tool { name, status, output, children, .. } => {
                assert_eq!(name, "Task");
                assert_eq!(*status, "completed");
                assert_eq!(output.as_deref(), Some("one file"));
                assert_eq!(children.len(), 1, "the subagent ran one tool");
                match &children[0] {
                    ChatRow::Tool { name, status, output, .. } => {
                        assert_eq!(name, "Bash");
                        assert_eq!(*status, "completed", "its result reached the child, not a new card");
                        assert_eq!(output.as_deref(), Some("a.txt"));
                    }
                    other => panic!("expected the subagent's tool call, got {other:?}"),
                }
            }
            other => panic!("expected the Task card, got {other:?}"),
        }
    }

    /// A subagent's prose stays in the card too, and the card survives its own result arriving.
    #[test]
    fn a_subagents_prose_stays_in_the_card_and_survives_the_result() {
        let rows = timeline_of(&[
            r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"task-1","name":"Task","input":{}}]}}"#,
            r#"{"type":"assistant","parent_tool_use_id":"task-1","message":{"id":"m2","content":[{"type":"text","text":"looking"}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"task-1","content":"done"}]}}"#,
        ]);
        assert_eq!(rows.len(), 1);
        match &rows[0] {
            ChatRow::Tool { children, output, .. } => {
                assert_eq!(output.as_deref(), Some("done"));
                assert!(matches!(&children[0], ChatRow::Assistant { text, .. } if text == "looking"));
            }
            other => panic!("expected the Task card, got {other:?}"),
        }
    }

    /// Background subagents may emit no sidechain frames. Their task lifecycle must still identify the
    /// card, show the original prompt, report live usage, and settle it when the provider does.
    #[test]
    fn task_protocol_keeps_background_subagent_identity_progress_and_status() {
        let rows = timeline_of(&[
            r#"{"type":"system","subtype":"task_started","task_id":"task-a","tool_use_id":"tool-a","task_type":"local_agent","subagent_type":"Explore","description":"Inspect the parser","prompt":"Find the relevant parser"}"#,
            r#"{"type":"system","subtype":"task_progress","task_id":"task-a","usage":{"total_tokens":1234,"tool_uses":3,"duration_ms":90}}"#,
            r#"{"type":"assistant","parent_tool_use_id":"tool-a","message":{"id":"child-a","model":"claude-sonnet-5","content":[{"type":"text","text":"Found it"}]}}"#,
            r#"{"type":"system","subtype":"task_updated","task_id":"task-a","patch":{"status":"completed"}}"#,
            r#"{"type":"system","subtype":"task_notification","task_id":"task-a","tool_use_id":"tool-a","status":"completed","summary":"Parser inspected","usage":{"total_tokens":1500,"tool_uses":4,"duration_ms":120}}"#,
        ]);
        assert_eq!(rows.len(), 1);
        match &rows[0] {
            ChatRow::Tool { status, output, subagent: Some(facts), children, .. } => {
                assert_eq!(*status, "completed");
                assert_eq!(output.as_deref(), Some("Parser inspected"));
                assert_eq!(facts.title.as_deref(), Some("Explore"));
                assert_eq!(facts.description.as_deref(), Some("Inspect the parser"));
                assert_eq!(facts.model.as_deref(), Some("claude-sonnet-5"));
                assert_eq!(facts.total_tokens, Some(1500));
                assert_eq!(facts.tool_uses, Some(4));
                assert_eq!(facts.duration_ms, Some(120));
                assert!(matches!(&children[0], ChatRow::User { text, .. } if text == "Find the relevant parser"));
                assert!(matches!(&children[1], ChatRow::Assistant { text, .. } if text == "Found it"));
            }
            other => panic!("expected a described subagent card, got {other:?}"),
        }
    }

    /// Background shell tasks use the same task protocol but are not subagents and must not appear as
    /// synthetic Task cards in the conversation.
    #[test]
    fn task_protocol_ignores_background_shells() {
        let rows = timeline_of(&[
            r#"{"type":"system","subtype":"task_started","task_id":"shell-a","tool_use_id":"tool-shell","task_type":"local_bash","description":"sleep"}"#,
            r#"{"type":"system","subtype":"task_progress","task_id":"shell-a","usage":{"total_tokens":99}}"#,
        ]);
        assert!(rows.is_empty());
    }

    /// Snapshot payloads carry lightweight references while the owning session can still resolve both
    /// timeline and queued attachments on demand.
    #[test]
    fn snapshots_reference_image_bytes_instead_of_retransmitting_them() {
        let session_id = "snapshot-images";
        let proc = inert_process(SessionKind::Claude);
        let image = ChatImage {
            mime_type: "image/png".to_string(),
            data: "QUJDREVGR0g=".to_string(),
        };
        proc.timeline.lock().unwrap().upsert(ChatRow::User {
            id: "user-image".to_string(),
            text: "look".to_string(),
            images: vec![image.clone()],
            at: None,
        });
        proc.turn.lock().unwrap().waiting.push(QueuedMessage {
            id: "queued-image".to_string(),
            text: "next".to_string(),
            images: vec![image.clone()],
        });
        let manager = ChatManager::new();
        manager.sessions.lock().unwrap().insert(session_id.to_string(), proc);

        let snapshot = manager.snapshot(session_id);
        let wire = serde_json::to_value(&snapshot).unwrap();
        assert!(!wire.to_string().contains(&image.data));
        let row_ref = wire["rows"][0]["images"][0]["attachmentId"].as_str().unwrap();
        let queue_ref = wire["queue"][0]["images"][0]["attachmentId"].as_str().unwrap();
        assert_eq!(manager.attachment(session_id, row_ref).unwrap(), image);
        assert_eq!(manager.attachment(session_id, queue_ref).unwrap().data, "QUJDREVGR0g=");
    }

    #[test]
    fn remote_windows_page_and_reconnect_without_skipping_history() {
        let proc = inert_process(SessionKind::Claude);
        let manager = ChatManager::new();
        manager.sessions.lock().unwrap().insert("window".into(), proc.clone());
        let user = |i: usize| ChatRow::User { id: format!("r{i}"), text: format!("message {i}"), images: vec![], at: None };
        for i in 0..150 { proc.timeline.lock().unwrap().upsert(user(i)); }
        let recent = manager.snapshot_window("window", Some(&ChatWindow::default()));
        assert_eq!(recent.rows.len(), 60);
        assert_eq!(recent.rows[0].id(), "r90");
        assert_eq!(recent.positions["r90"], 90);
        assert!(recent.has_more);
        let history = manager.snapshot_window("window", Some(&ChatWindow { before: Some("r90".into()), epoch: recent.started_at, ..Default::default() }));
        assert_eq!(history.page_kind, "history");
        assert_eq!(history.rows[0].id(), "r30");
        assert_eq!(history.rows.last().unwrap().id(), "r89");
        // An updated row outside the loaded window must not move its history cursor.
        for i in [2, 100, 150] { proc.timeline.lock().unwrap().upsert(user(i)); }
        let delta = manager.snapshot_window("window", Some(&ChatWindow { since: Some(recent.rows_revision), from: Some("r90".into()), epoch: recent.started_at, ..Default::default() }));
        assert_eq!(delta.page_kind, "delta");
        assert_eq!(delta.rows.iter().map(ChatRow::id).collect::<Vec<_>>(), vec!["r100", "r150"]);
        proc.timeline.lock().unwrap().replace_all(vec![user(0)]);
        let reset = manager.snapshot_window("window", Some(&ChatWindow { since: Some(delta.rows_revision), epoch: delta.started_at, ..Default::default() }));
        assert_eq!(reset.page_kind, "recent");
        assert_eq!(reset.rows.len(), 1);
    }

    #[test]
    fn large_tool_details_are_deferred_and_epoch_checked() {
        let proc = inert_process(SessionKind::Claude);
        let manager = ChatManager::new();
        let row = ChatRow::Tool { id: "tool".into(), name: "Bash".into(), input: json!({"command":"pwd"}), output: Some("x".repeat(100_000)), is_error: false, status: "completed", children: vec![], subagent: None };
        proc.timeline.lock().unwrap().upsert(row);
        manager.sessions.lock().unwrap().insert("detail".into(), proc.clone());
        let wire = serde_json::to_value(manager.snapshot_window("detail", Some(&ChatWindow::default()))).unwrap();
        assert_eq!(wire["rows"][0]["detailAvailable"], true);
        assert!(wire.to_string().len() < 5000);
        assert_eq!(manager.row_detail("detail", "tool", Some(proc.started_at)).unwrap()["output"].as_str().unwrap().len(), 100_000);
        assert!(manager.row_detail("detail", "tool", Some(proc.started_at + 1)).is_err());
    }

    /// Compaction draws one row that changes, not two rows that repeat each other.
    #[test]
    fn compaction_marks_the_conversation_once() {
        let rows = timeline_of(&[
            r#"{"type":"system","subtype":"status","status":"compacting","session_id":"s"}"#,
            r#"{"type":"system","subtype":"compact_boundary","session_id":"s","compact_metadata":{"trigger":"manual","pre_tokens":31039}}"#,
        ]);
        assert_eq!(rows.len(), 1);
        match &rows[0] {
            ChatRow::Compaction { status, trigger, pre_tokens, .. } => {
                assert_eq!(*status, "completed");
                assert_eq!(trigger.as_deref(), Some("manual"));
                assert_eq!(*pre_tokens, Some(31039));
            }
            other => panic!("expected a compaction row, got {other:?}"),
        }
    }

    /// Compacting twice leaves two marks: the second must not overwrite the first.
    #[test]
    fn a_second_compaction_gets_its_own_mark() {
        let rows = timeline_of(&[
            r#"{"type":"system","subtype":"status","status":"compacting","session_id":"s"}"#,
            r#"{"type":"system","subtype":"compact_boundary","session_id":"s"}"#,
            r#"{"type":"system","subtype":"status","status":"compacting","session_id":"s"}"#,
        ]);
        assert_eq!(rows.len(), 2);
        assert!(matches!(&rows[0], ChatRow::Compaction { status, .. } if *status == "completed"));
        assert!(matches!(&rows[1], ChatRow::Compaction { status, .. } if *status == "loading"));
    }

    /// A manager holding one session whose "agent" is `cat`: it accepts everything written to it and
    /// says nothing back, which is all these tests need — they drive the reader side by hand.
    fn manager_with_session(session_id: &str) -> ChatManager {
        let (proc, stdout) = cat_process();
        // Keep the echo pipe open; otherwise cat can exit before a second submission.
        std::thread::spawn(move || { for line in BufReader::new(stdout).lines() { if line.is_err() { break; } } });
        let manager = ChatManager::new();
        manager
            .sessions
            .lock()
            .unwrap()
            .insert(session_id.to_string(), proc);
        manager
    }

    #[test]
    fn codex_rewind_exposes_only_native_conversation_restore() {
        let app = ctx("codex-native-rewind");
        let manager = ChatManager::new();
        let proc = inert_process(SessionKind::Codex);
        proc.ready.store(true, Ordering::Relaxed);
        manager.sessions.lock().unwrap().insert("s".into(), proc.clone());
        assert_eq!(manager.snapshot("s").rewind_scopes, vec!["conversation"]);
        assert_eq!(rewind_scopes(SessionKind::Claude), vec!["conversation", "files", "both"]);
        assert_eq!(rewind_scopes(SessionKind::Opencode), vec!["both"]);
        assert!(manager.rewind_preview("s", "missing").unwrap_err().contains("native file restore"));
        for scope in ["files", "both"] {
            assert!(manager.rewind(&app, "s", "missing", scope).unwrap_err().contains("native file restore"));
        }
        assert!(proc.timeline.lock().unwrap().rows.is_empty());
        proc.child.lock().unwrap().wait().unwrap();
    }

    #[test]
    fn codex_turn_without_explicit_directory_needs_no_client_backup() {
        let app = ctx("codex-no-client-backup");
        let (mut proc, stdout) = cat_process();
        Arc::get_mut(&mut proc).unwrap().kind = SessionKind::Codex;
        *proc.agent_session_id.lock().unwrap() = Some("original".into());
        let manager = ChatManager::new();
        manager.sessions.lock().unwrap().insert("s".into(), proc.clone());
        manager.send(&app, "s", "Continue", vec![], "queue").unwrap();
        let line = BufReader::new(stdout).lines().next().unwrap().unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["method"], "turn/start");
        handle_codex_notification(&app, "s", &proc, "turn/started",
            json!({"threadId":"original","turn":{"id":"turn-native"}}));
        handle_codex_notification(&app, "s", &proc, "turn/completed",
            json!({"threadId":"original","turn":{"id":"turn-native","status":"completed"}}));
        assert_eq!(said(&manager, "s"), vec!["Continue"]);
        assert!(!proc.timeline.lock().unwrap().rows.iter().any(|row| matches!(row, ChatRow::Notice { .. })));
        assert!(!proc.turn.lock().unwrap().running);
        let conn = app.db().conn.lock().unwrap();
        let tables: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('chat_checkpoints','chat_checkpoint_objects','chat_restore_journal')",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(tables, 0);
        drop(conn);
        manager.stop(&app, "s").unwrap();
        proc.child.lock().unwrap().wait().unwrap();
    }

    #[test]
    fn codex_mcp_management_uses_native_requests_and_paginates() {
        let app = ctx("codex-mcp-management");
        let (mut proc, stdout) = cat_process();
        Arc::get_mut(&mut proc).unwrap().kind = SessionKind::Codex;
        *proc.agent_session_id.lock().unwrap() = Some("thread-test".into());
        let responder = proc.clone();
        let worker = std::thread::spawn(move || {
            let mut enabled = true;
            let mut methods = Vec::new();
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let request: Value = serde_json::from_str(&line).unwrap();
                let method = request["method"].as_str().unwrap();
                methods.push(method.to_string());
                let result = match method {
                    "config/read" => json!({"config":{"mcp_servers":{"local":{"enabled":enabled}}}}),
                    "mcpServerStatus/list" => {
                        assert_eq!(request["params"]["threadId"], "thread-test");
                        if request["params"]["cursor"].is_null() {
                            json!({"data":[{"name":"local","runtimeStatus":if enabled {"connected"} else {"disabled"},"tools":{"read":{}}}],"nextCursor":"page2"})
                        } else {
                            json!({"data":[{"name":"plugin","pluginId":"p","runtimeStatus":"connected","tools":{}}],"nextCursor":null})
                        }
                    },
                    "config/value/write" => {
                        assert_eq!(request["params"]["keyPath"], "mcp_servers.local.enabled");
                        assert_eq!(request["params"]["mergeStrategy"], "replace");
                        enabled = request["params"]["value"].as_bool().unwrap();
                        json!({})
                    },
                    "config/mcpServer/reload" => json!({}),
                    other => panic!("Unexpected method: {other}"),
                };
                handle_codex_line(&app, "s", &responder, &json!({"id":request["id"],"result":result}).to_string());
            }
            methods
        });
        let listed = codex::mcp_status(&proc).unwrap();
        assert_eq!(listed["mcpServers"].as_array().unwrap().len(), 2);
        let disabled = codex::mcp_toggle(&proc, "local", false).unwrap();
        assert_eq!(disabled["mcpServers"][0]["status"], "disabled");
        assert!(codex::mcp_toggle(&proc, "plugin", false).is_err());
        codex::mcp_reconnect(&proc, "local").unwrap();
        proc.child.lock().unwrap().kill().unwrap();
        proc.child.lock().unwrap().wait().unwrap();
        let methods = worker.join().unwrap();
        assert_eq!(methods.iter().filter(|m| *m == "config/value/write").count(), 1);
        assert_eq!(methods.iter().filter(|m| *m == "config/mcpServer/reload").count(), 2);
    }

    /// A session whose "agent" is `cat`, with its stdout handed back so a test can run the real reader.
    fn cat_process() -> (Arc<ChatProcess>, std::process::ChildStdout) {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let proc = Arc::new(ChatProcess {
            kind: SessionKind::Claude,
            action: Mutex::new(()),
            cwd: None,
            stdin: Mutex::new(Some(stdin)),
            child: Mutex::new(child),
            timeline: Mutex::new(Timeline::default()),
            permissions: Mutex::new(HashMap::new()),
            commands: Mutex::new(Vec::new()),
            config_keys: Mutex::new(Vec::new()),
            subagent_ids: Mutex::new(HashMap::new()),
            codex_subagent_ids: Mutex::new(HashMap::new()),
            codex_pending_child_events: Mutex::new(HashMap::new()),
            opencode: Mutex::new(opencode::OpencodeState::default()),
            service_tier: Mutex::new(None),
            personality: Mutex::new(None),
            codex_turn_error: Mutex::new(None),
            extras: Mutex::new(ClaudeExtras::default()),
            claude_models: Mutex::new(Vec::new()),
            agent_session_id: Mutex::new(None),
            settings_change: Mutex::new(()),
            model: Mutex::new(None),
            effort: Mutex::new(None),
            mode: Mutex::new("default".to_string()),
            codex_permission_state: Mutex::new(CodexPermissionState::default()),
            collaboration_mode: Mutex::new(None),
            collaboration_modes: Mutex::new(Vec::new()),
            current_turn: Mutex::new(None),
            ready: AtomicBool::new(true),
            buffers: Mutex::new(HashMap::new()),
            current_message: Mutex::new(None),
            frame_blocks: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            waiters: Mutex::new(HashMap::new()),
            user_targets: Mutex::new(HashMap::new()),
            pending_user_rows: Mutex::new(Vec::new()),
            turn: Mutex::new(TurnQueue::default()),
            stderr: Mutex::new(String::new()),
            alive: AtomicBool::new(true),
            release_when_idle: AtomicBool::new(false),
            released: AtomicBool::new(false),
            next_request: AtomicU64::new(1),
            pid: 0,
            started_at: 0,
            compactions: AtomicU64::new(0),
        });
        (proc, stdout)
    }

    /// Text of every user row on the timeline, in order — what the agent has actually been told.
    fn said(manager: &ChatManager, session_id: &str) -> Vec<String> {
        manager
            .snapshot(session_id)
            .rows
            .into_iter()
            .filter_map(|row| match row {
                ChatRow::User { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    fn queued(manager: &ChatManager, session_id: &str) -> Vec<String> {
        manager
            .snapshot(session_id)
            .queue
            .into_iter()
            .map(|item| item.text)
            .collect()
    }

    /// End the turn in progress the way the agent's `result` line does.
    fn finish_turn(manager: &ChatManager, app: &AppCtx, session_id: &str, subtype: &str) {
        let proc = manager.get(session_id).unwrap();
        handle_turn_end(app, session_id, &proc, subtype, None);
    }

    fn title_fixture(tag: &str, kind: SessionKind, name: &str) -> (AppCtx, ChatManager, Arc<AtomicU64>) {
        let app = ctx(tag);
        {
            let conn = app.db().conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,name,root_path,created_at) VALUES ('p','test','/tmp',0)", []).unwrap();
            conn.execute(
                "INSERT INTO sessions(id,project_id,name,kind,permission_mode,created_at) VALUES ('s','p',?1,?2,'default',0)",
                rusqlite::params![name, if kind == SessionKind::Codex { "codex" } else { "claude" }],
            ).unwrap();
        }
        let (mut proc, stdout) = cat_process();
        Arc::get_mut(&mut proc).unwrap().kind = kind;
        *proc.agent_session_id.lock().unwrap() = Some("thread-test".into());
        std::thread::spawn(move || { for line in BufReader::new(stdout).lines() { if line.is_err() { break; } } });
        let manager = ChatManager::new();
        manager.sessions.lock().unwrap().insert("s".into(), proc);
        let events = Arc::new(AtomicU64::new(0));
        let captured = events.clone();
        app.listen(crate::host::TREE_CHANGED, move |_| { captured.fetch_add(1, Ordering::Relaxed); });
        (app, manager, events)
    }

    fn stored_title(app: &AppCtx) -> String {
        let conn = app.db().conn.lock().unwrap();
        crate::db::repo::get_session_name(&conn, "s").unwrap().unwrap()
    }

    #[test]
    fn identified_messages_keep_their_id_from_queue_to_timeline() {
        let (app, manager, _) = title_fixture("submission-identity", SessionKind::Claude, "Test");
        let first = format!("msg-{}", uuid::Uuid::new_v4());
        let second = format!("msg-{}", uuid::Uuid::new_v4());
        assert_eq!(manager.send_identified(&app, "s", "First", vec![], "queue", Some(&first)).unwrap(), "sent");
        assert_eq!(manager.send_identified(&app, "s", "Second", vec![], "queue", Some(&second)).unwrap(), "queued");
        let before = manager.snapshot("s");
        assert_eq!(before.rows[0].id(), first);
        assert_eq!(before.queue[0].id, second);
        finish_turn(&manager, &app, "s", "success");
        let after = manager.snapshot("s");
        assert!(after.queue.is_empty());
        assert!(after.rows.iter().any(|row| row.id() == second));
        assert!(after.rows_revision > before.rows_revision);
        assert!(after.queue_revision > before.queue_revision);
        manager.stop(&app, "s").unwrap();
    }

    #[test]
    fn replaying_a_submission_returns_its_receipt_without_a_second_prompt() {
        let (app, manager, _) = title_fixture("submission-receipt", SessionKind::Claude, "Test");
        app.chat().sessions.lock().unwrap().insert("s".into(), manager.get("s").unwrap());
        let id = format!("msg-{}", uuid::Uuid::new_v4());
        for _ in 0..2 {
            assert_eq!(crate::command_core::chat_send(&app, "s", "Hello", vec![], Some("queue"), Some(&id)).unwrap(), "sent");
        }
        let snapshot = app.chat().snapshot("s");
        assert_eq!(snapshot.rows.iter().filter(|row| matches!(row, ChatRow::User { .. })).count(), 1);
        assert!(snapshot.queue.is_empty());
        app.chat().stop(&app, "s").unwrap();
    }

    #[test]
    fn chat_title_updates_once_after_dispatch_and_broadcasts_the_persisted_name() {
        for (tag, kind, name) in [
            ("title-claude", SessionKind::Claude, "Claude 1"),
            ("title-codex", SessionKind::Codex, "Codex 1"),
        ] {
            let (app, manager, events) = title_fixture(tag, kind, name);
            manager.send(&app, "s", "\n# 修复会话标题\n补充说明", Vec::new(), "queue").unwrap();
            assert_eq!(stored_title(&app), "修复会话标题");
            assert_eq!(events.load(Ordering::Relaxed), 1);
            manager.send(&app, "s", "后续消息", Vec::new(), "queue").unwrap();
            finish_turn(&manager, &app, "s", "success");
            assert_eq!(stored_title(&app), "修复会话标题");
            assert_eq!(events.load(Ordering::Relaxed), 1);
        }
    }

    #[test]
    fn chat_title_waits_for_startup_or_turn_queue_dispatch() {
        for startup in [true, false] {
            let (app, manager, events) = title_fixture(
                if startup { "title-startup" } else { "title-queue" }, SessionKind::Claude, "Claude 1",
            );
            let proc = manager.get("s").unwrap();
            proc.ready.store(!startup, Ordering::Relaxed);
            proc.turn.lock().unwrap().running = !startup;
            assert_eq!(manager.send(&app, "s", "原始草稿", Vec::new(), "queue").unwrap(), "queued");
            assert_eq!(stored_title(&app), "Claude 1");
            assert_eq!(events.load(Ordering::Relaxed), 0);
            let id = manager.snapshot("s").queue[0].id.clone();
            manager.queue_update(&app, "s", &id, "实际发送的内容").unwrap();
            if startup {
                proc.ready.store(true, Ordering::Relaxed);
                start_waiting_message(&app, "s", &proc);
            } else {
                finish_turn(&manager, &app, "s", "success");
            }
            assert_eq!(stored_title(&app), "实际发送的内容");
            assert_eq!(events.load(Ordering::Relaxed), 1);
        }
    }

    #[test]
    fn chat_title_preserves_custom_names_and_ignores_empty_or_failed_sends() {
        let (app, manager, events) = title_fixture("title-custom", SessionKind::Claude, "自定义标题");
        manager.send(&app, "s", "用户消息", Vec::new(), "queue").unwrap();
        assert_eq!(stored_title(&app), "自定义标题");
        assert_eq!(events.load(Ordering::Relaxed), 0);

        let (app, manager, events) = title_fixture("title-empty-failed", SessionKind::Claude, "Claude 1");
        manager.send(&app, "s", " \n\t", Vec::new(), "queue").unwrap();
        assert_eq!(stored_title(&app), "Claude 1");
        manager.send(&app, "s", "发送失败的消息", Vec::new(), "queue").unwrap();
        *manager.get("s").unwrap().stdin.lock().unwrap() = None;
        finish_turn(&manager, &app, "s", "success");
        assert_eq!(stored_title(&app), "Claude 1");
        assert!(manager.send(&app, "s", "重试仍然失败", Vec::new(), "queue").is_err());
        assert_eq!(stored_title(&app), "Claude 1");
        assert_eq!(events.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn chat_title_can_name_a_placeholder_when_steering_a_resumed_turn() {
        let (app, manager, events) = title_fixture("title-steer", SessionKind::Claude, "Claude 1");
        manager.get("s").unwrap().turn.lock().unwrap().running = true;
        manager.send(&app, "s", "补充任务要求", Vec::new(), "steer").unwrap();
        assert_eq!(stored_title(&app), "补充任务要求");
        assert_eq!(events.load(Ordering::Relaxed), 1);
    }

    /// Nothing running means nothing to wait for: the message goes straight out.
    #[test]
    fn a_message_sent_to_an_idle_agent_goes_out_at_once() {
        let app = ctx("idle");
        let m = manager_with_session("s");
        assert_eq!(m.send(&app, "s", "hello", Vec::new(), "queue").unwrap(), "sent");
        assert_eq!(said(&m, "s"), vec!["hello"]);
        assert!(queued(&m, "s").is_empty());
    }

    /// The whole point of the step: a follow-up typed mid-turn is held, not written into the agent, and
    /// it goes out — in order, one per turn — as the turns it was waiting for end.
    #[test]
    fn messages_typed_during_a_turn_wait_and_then_go_out_in_order() {
        let app = ctx("order");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        assert_eq!(m.send(&app, "s", "second", Vec::new(), "queue").unwrap(), "queued");
        assert_eq!(m.send(&app, "s", "third", Vec::new(), "queue").unwrap(), "queued");
        assert_eq!(said(&m, "s"), vec!["first"], "only the first was actually said");
        assert_eq!(queued(&m, "s"), vec!["second", "third"]);

        finish_turn(&m, &app, "s", "success");
        assert_eq!(said(&m, "s"), vec!["first", "second"]);
        assert_eq!(queued(&m, "s"), vec!["third"], "one message per turn, not the whole queue");

        finish_turn(&m, &app, "s", "success");
        assert_eq!(said(&m, "s"), vec!["first", "second", "third"]);
        assert!(queued(&m, "s").is_empty());

        // With the queue empty the conversation really is idle again.
        finish_turn(&m, &app, "s", "success");
        assert_eq!(said(&m, "s").len(), 3, "an empty queue sends nothing");
        assert_eq!(m.send(&app, "s", "later", Vec::new(), "queue").unwrap(), "sent");
    }

    #[test]
    fn opencode_reply_model_survives_late_metadata_deltas_and_idle() {
        let app = ctx("oc-reply-model");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        *proc.agent_session_id.lock().unwrap() = Some("native".into());
        let send = |event| opencode::handle_event(&app, "s", &proc, event);
        for (id, model, late) in [("a", "deepseek-v4-pro", false), ("b", "glm-5.3", true)] {
            let metadata = json!({"type":"message.updated","properties":{"info":{
                "id":id,"sessionID":"native","role":"assistant","modelID":model
            }}});
            if !late { send(metadata.clone()); }
            send(json!({"type":"message.part.updated","properties":{"part":{
                "id":id,"messageID":id,"sessionID":"native","type":"text","text":"hello"
            }}}));
            if late { send(metadata); }
            send(json!({"type":"message.part.delta","properties":{
                "sessionID":"native","messageID":id,"partID":id,"field":"text","delta":" world"
            }}));
        }
        send(json!({"type":"session.status","properties":{"sessionID":"native","status":{"type":"busy"}}}));
        send(json!({"type":"session.idle","properties":{"sessionID":"native"}}));
        let rows = manager.snapshot("s").rows;
        assert_eq!(rows.len(), 2);
        for (row, expected) in rows.iter().zip(["deepseek-v4-pro", "glm-5.3"]) {
            assert!(matches!(row, ChatRow::Assistant { model: Some(model), text, streaming: false, .. }
                if model == expected && text == "hello world"));
        }
        manager.stop(&app, "s").unwrap();
    }

    #[test]
    fn opencode_duplicate_idle_does_not_complete_the_next_queued_turn() {
        let app = ctx("oc-idle");
        let m = manager_with_session("s");
        let proc = m.get("s").unwrap();
        *proc.agent_session_id.lock().unwrap() = Some("native".into());
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "second", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "third", Vec::new(), "queue").unwrap();
        let status = |kind: &str| json!({"type":"session.status","properties":{
            "sessionID":"native","status":{"type":kind}
        }});
        opencode::handle_event(&app, "s", &proc, status("busy"));
        opencode::handle_event(&app, "s", &proc, status("idle"));
        opencode::handle_event(&app, "s", &proc, json!({"type":"session.idle","properties":{"sessionID":"native"}}));
        assert_eq!(said(&m, "s"), vec!["first", "second"]);
        assert_eq!(queued(&m, "s"), vec!["third"]);
        assert!(proc.turn.lock().unwrap().running);
        opencode::handle_event(&app, "s", &proc, status("busy"));
        opencode::handle_event(&app, "s", &proc, status("idle"));
        assert_eq!(said(&m, "s"), vec!["first", "second", "third"]);
        m.stop(&app, "s").unwrap();
    }

    #[test]
    fn opencode_failed_permission_reply_can_be_retried() {
        let app = ctx("oc-permission-retry");
        let (mut proc, _stdout) = cat_process();
        Arc::get_mut(&mut proc).unwrap().kind = SessionKind::Opencode;
        let request = json!({"id":"request","_opencodeKind":"permission"});
        proc.permissions.lock().unwrap().insert("request".into(), request.clone());
        let m = ChatManager::new();
        m.sessions.lock().unwrap().insert("s".into(), proc.clone());
        for _ in 0..2 {
            let error = m.respond_permission(&app, "s", "request", true, None, None, None).unwrap_err();
            assert_eq!(error, "OpenCode has not started yet");
            assert_eq!(proc.permissions.lock().unwrap().get("request"), Some(&request));
        }
        m.stop(&app, "s").unwrap();
    }

    #[test]
    fn opencode_failed_auto_approval_keeps_the_permission_card() {
        let app = ctx("oc-auto-approval");
        let m = manager_with_session("s");
        let proc = m.get("s").unwrap();
        *proc.mode.lock().unwrap() = "bypassPermissions".into();
        // A closed local port makes the reply fail without contacting a provider.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        proc.opencode.lock().unwrap().server = Some(Arc::new(opencode_protocol::Server::new(port, "test", None)));
        opencode::handle_event(&app, "s", &proc, json!({"type":"permission.asked","properties":{
            "id":"request","permission":"bash","patterns":["pwd"],"always":[]
        }}));
        assert!(proc.permissions.lock().unwrap().contains_key("request"));
        m.stop(&app, "s").unwrap();
    }

    /// Interrupting means "instead of that, this": the message goes ahead of anything already waiting.
    #[test]
    fn an_interrupting_message_goes_ahead_of_the_queue() {
        let app = ctx("interrupt");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "waiting", Vec::new(), "queue").unwrap();
        assert_eq!(m.send(&app, "s", "urgent", Vec::new(), "interrupt").unwrap(), "queued");
        assert_eq!(queued(&m, "s"), vec!["urgent", "waiting"]);
        // The agent ends an interrupted turn by reporting a failure; the interrupting message follows it.
        finish_turn(&m, &app, "s", "error_during_execution");
        assert_eq!(said(&m, "s"), vec!["first", "urgent"]);
    }

    /// A turn stopped on purpose is not a fault, and must not leave an error row behind.
    #[test]
    fn an_interrupted_turn_reports_no_error() {
        let app = ctx("no-error");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "urgent", Vec::new(), "interrupt").unwrap();
        finish_turn(&m, &app, "s", "error_during_execution");
        let rows = m.snapshot("s").rows;
        assert!(
            !rows.iter().any(|r| matches!(r, ChatRow::Error { .. })),
            "the interrupt worked; nothing failed: {rows:?}"
        );

        // A failure nobody asked for still shows.
        finish_turn(&m, &app, "s", "error_max_turns");
        assert!(m
            .snapshot("s")
            .rows
            .iter()
            .any(|r| matches!(r, ChatRow::Error { .. })));
    }

    /// Steering is the deliberate exception: the message is written into the turn already running.
    #[test]
    fn steering_writes_into_the_running_turn() {
        let app = ctx("steer");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        assert_eq!(m.send(&app, "s", "and also this", Vec::new(), "steer").unwrap(), "sent");
        assert_eq!(said(&m, "s"), vec!["first", "and also this"]);
        assert!(queued(&m, "s").is_empty(), "nothing is waiting; it was said");
    }

    #[test]
    fn steering_rejection_preserves_the_running_turn_and_queue() {
        for kind in [SessionKind::Claude, SessionKind::Codex, SessionKind::Opencode] {
            let app = ctx("steer-rejected");
            let (mut proc, _stdout) = cat_process();
            Arc::get_mut(&mut proc).unwrap().kind = kind;
            proc.stdin.lock().unwrap().take();
            {
                let mut turn = proc.turn.lock().unwrap();
                turn.running = true;
                turn.started_at = Some(123);
            }
            let manager = ChatManager::new();
            manager.sessions.lock().unwrap().insert("s".into(), proc.clone());
            assert!(manager.send(&app, "s", "retry me", Vec::new(), "steer").is_err());
            assert!(said(&manager, "s").is_empty());
            assert!(queued(&manager, "s").is_empty());
            assert!(proc.turn.lock().unwrap().running);
            assert_eq!(proc.turn.lock().unwrap().started_at, Some(123));
            proc.child.lock().unwrap().kill().unwrap();
            proc.child.lock().unwrap().wait().unwrap();
        }
    }

    #[test]
    fn opencode_steering_posts_an_async_prompt_and_preserves_the_turn_on_http_failure() {
        use std::io::Read;
        // Dedicated local mock port, checked against project configuration and listeners when selected.
        const MOCK_PORT: u16 = 28347;
        let listener = std::net::TcpListener::bind(("127.0.0.1", MOCK_PORT)).expect("OpenCode mock port is occupied");
        let worker = std::thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert_eq!(line.trim(), "POST /session/native-test/prompt_async HTTP/1.1");
                let mut length = 0;
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" { break; }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
                let mut bytes = vec![0; length];
                reader.read_exact(&mut bytes).unwrap();
                let body: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(body["parts"][0]["text"], "/compact is just prose here");
                let reply = if index == 0 { "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n" }
                    else { "HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\nConnection: close\r\n\r\n" };
                stream.write_all(reply.as_bytes()).unwrap();
            }
        });
        let app = ctx("opencode-steer-http");
        let (mut proc, _stdout) = cat_process();
        Arc::get_mut(&mut proc).unwrap().kind = SessionKind::Opencode;
        *proc.agent_session_id.lock().unwrap() = Some("native-test".into());
        proc.opencode.lock().unwrap().server = Some(Arc::new(super::super::opencode_protocol::Server::new(MOCK_PORT, "test-only", None)));
        proc.turn.lock().unwrap().running = true;
        proc.turn.lock().unwrap().started_at = Some(123);
        let manager = ChatManager::new();
        manager.sessions.lock().unwrap().insert("s".into(), proc.clone());
        assert_eq!(manager.send(&app, "s", "/compact is just prose here", Vec::new(), "steer").unwrap(), "sent");
        assert!(manager.send(&app, "s", "/compact is just prose here", Vec::new(), "steer").is_err());
        assert_eq!(said(&manager, "s"), vec!["/compact is just prose here"]);
        assert!(queued(&manager, "s").is_empty());
        assert!(proc.turn.lock().unwrap().running);
        assert_eq!(proc.turn.lock().unwrap().started_at, Some(123));
        assert_eq!(proc.user_targets.lock().unwrap().len(), 1);
        worker.join().unwrap();
        proc.child.lock().unwrap().kill().unwrap();
        proc.child.lock().unwrap().wait().unwrap();
    }

    #[test]
    fn codex_steering_waits_for_native_acceptance_without_ending_the_turn_on_refusal() {
        let app = ctx("codex-steer");
        let (mut proc, stdout) = cat_process();
        Arc::get_mut(&mut proc).unwrap().kind = SessionKind::Codex;
        *proc.agent_session_id.lock().unwrap() = Some("thread-test".into());
        proc.turn.lock().unwrap().running = true;
        proc.turn.lock().unwrap().started_at = Some(123);
        let manager = ChatManager::new();
        manager.sessions.lock().unwrap().insert("s".into(), proc.clone());
        assert!(manager.send(&app, "s", "too early", Vec::new(), "steer").is_err());
        assert!(queued(&manager, "s").is_empty());
        *proc.current_turn.lock().unwrap() = Some("turn-test".into());
        let responder = proc.clone();
        let response_app = app.clone();
        let worker = std::thread::spawn(move || {
            for (index, line) in BufReader::new(stdout).lines().take(2).enumerate() {
                let request: Value = serde_json::from_str(&line.unwrap()).unwrap();
                assert_eq!(request["method"], "turn/steer");
                assert_eq!(request["params"]["threadId"], "thread-test");
                assert_eq!(request["params"]["expectedTurnId"], "turn-test");
                let response = if index == 0 {
                    json!({"id":request["id"],"result":{"turnId":"turn-test"}})
                } else {
                    json!({"id":request["id"],"error":{"code":-32600,"message":"steer refused"}})
                };
                handle_codex_line(&response_app, "s", &responder, &response.to_string());
            }
        });
        assert_eq!(manager.send(&app, "s", "accepted", Vec::new(), "steer").unwrap(), "sent");
        assert!(manager.send(&app, "s", "rejected", Vec::new(), "steer").unwrap_err().contains("steer refused"));
        assert_eq!(said(&manager, "s"), vec!["accepted"]);
        assert!(queued(&manager, "s").is_empty());
        assert!(proc.turn.lock().unwrap().running);
        assert_eq!(proc.turn.lock().unwrap().started_at, Some(123));
        assert_eq!(proc.current_turn.lock().unwrap().as_deref(), Some("turn-test"));
        worker.join().unwrap();
        proc.child.lock().unwrap().kill().unwrap();
        proc.child.lock().unwrap().wait().unwrap();
    }

    #[test]
    fn queued_steering_sends_once_and_preserves_other_messages() {
        let app = ctx("queue-steer");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "correction", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "later", Vec::new(), "queue").unwrap();
        let id = m.snapshot("s").queue[0].id.clone();
        m.queue_steer(&app, "s", &id).unwrap();
        assert_eq!(said(&m, "s"), vec!["first", "correction"]);
        assert_eq!(queued(&m, "s"), vec!["later"]);
        assert!(m.queue_steer(&app, "s", &id).is_err());
        finish_turn(&m, &app, "s", "success");
        assert_eq!(said(&m, "s"), vec!["first", "correction", "later"]);
    }

    #[test]
    fn queued_steering_defers_queue_drain_until_native_acceptance() {
        let app = ctx("queue-steer-completion");
        let (mut proc, stdout) = cat_process();
        Arc::get_mut(&mut proc).unwrap().kind = SessionKind::Codex;
        *proc.agent_session_id.lock().unwrap() = Some("thread-test".into());
        *proc.current_turn.lock().unwrap() = Some("turn-test".into());
        {
            let mut turn = proc.turn.lock().unwrap();
            turn.running = true;
            turn.waiting.push(QueuedMessage { id: "q-correction".into(), text: "correction".into(), images: Vec::new() });
            turn.waiting.push(QueuedMessage { id: "q-later".into(), text: "later".into(), images: Vec::new() });
        }
        let manager = ChatManager::new();
        manager.sessions.lock().unwrap().insert("s".into(), proc.clone());
        let responder = proc.clone();
        let response_app = app.clone();
        let worker = std::thread::spawn(move || {
            let line = BufReader::new(stdout).lines().next().unwrap().unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["method"], "turn/steer");
            handle_turn_end(&response_app, "s", &responder, "success", None);
            assert_eq!(responder.turn.lock().unwrap().waiting.len(), 2);
            assert!(!responder.turn.lock().unwrap().running);
            handle_codex_line(&response_app, "s", &responder,
                &json!({"id":request["id"],"result":{"turnId":"turn-test"}}).to_string());
        });
        manager.queue_steer(&app, "s", "q-correction").unwrap();
        worker.join().unwrap();
        assert_eq!(said(&manager, "s"), vec!["correction", "later"]);
        assert!(queued(&manager, "s").is_empty());
        proc.child.lock().unwrap().kill().unwrap();
        proc.child.lock().unwrap().wait().unwrap();
    }

    #[test]
    fn queued_steering_refusal_keeps_the_message() {
        let app = ctx("queue-steer-refused");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "correction", Vec::new(), "queue").unwrap();
        let id = m.snapshot("s").queue[0].id.clone();
        let proc = m.get("s").unwrap();
        proc.stdin.lock().unwrap().take();
        assert!(m.queue_steer(&app, "s", &id).is_err());
        assert_eq!(queued(&m, "s"), vec!["correction"]);
        assert_eq!(said(&m, "s"), vec!["first"]);
        assert!(!proc.turn.lock().unwrap().steering);
    }

    /// A message dropped while it waited is never said.
    #[test]
    fn a_removed_message_is_never_sent() {
        let app = ctx("remove");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "regretted", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "kept", Vec::new(), "queue").unwrap();
        let id = m.snapshot("s").queue[0].id.clone();
        m.queue_remove(&app, "s", &id).unwrap();
        assert_eq!(queued(&m, "s"), vec!["kept"]);
        finish_turn(&m, &app, "s", "success");
        assert_eq!(said(&m, "s"), vec!["first", "kept"]);
    }

    /// A message rewritten while it waited goes out as rewritten.
    #[test]
    fn an_edited_message_is_sent_as_edited() {
        let app = ctx("edit");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "draft", Vec::new(), "queue").unwrap();
        let id = m.snapshot("s").queue[0].id.clone();
        m.queue_update(&app, "s", &id, "the real question").unwrap();
        finish_turn(&m, &app, "s", "success");
        assert_eq!(said(&m, "s"), vec!["first", "the real question"]);
    }

    // ── Attached images ──

    fn png(data: &str) -> ChatImage {
        ChatImage { mime_type: "image/png".into(), data: data.into() }
    }

    /// Images of every user row, in order — what was actually handed over with each message.
    fn shown(manager: &ChatManager, session_id: &str) -> Vec<Vec<String>> {
        manager
            .snapshot(session_id)
            .rows
            .into_iter()
            .filter_map(|row| match row {
                ChatRow::User { images, .. } => {
                    Some(images.into_iter().map(|i| i.data).collect())
                }
                _ => None,
            })
            .collect()
    }

    /// A picture sent with a message stays on its row, so the bubble can show what was handed over.
    #[test]
    fn an_image_sent_now_lands_on_the_message_it_came_with() {
        let app = ctx("image-now");
        let m = manager_with_session("s");
        m.send(&app, "s", "what is this", vec![png("AAAA")], "queue").unwrap();
        assert_eq!(said(&m, "s"), vec!["what is this"]);
        assert_eq!(shown(&m, "s"), vec![vec!["AAAA".to_string()]]);
    }

    /// A picture attached to a message typed mid-turn waits with it and goes out with it, not before.
    #[test]
    fn an_image_waits_with_the_message_it_belongs_to() {
        let app = ctx("image-queue");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "and this one", vec![png("BBBB"), png("CCCC")], "queue").unwrap();
        assert_eq!(shown(&m, "s"), vec![Vec::<String>::new()], "nothing shown yet but the first message");
        assert_eq!(m.snapshot("s").queue[0].images.len(), 2);
        finish_turn(&m, &app, "s", "success");
        assert_eq!(
            shown(&m, "s"),
            vec![Vec::<String>::new(), vec!["BBBB".to_string(), "CCCC".to_string()]]
        );
    }

    /// Rewriting the words of a waiting message keeps its pictures: the edit is of the caption, and
    /// losing the image it was written about would leave a question with nothing to answer.
    #[test]
    fn rewriting_a_waiting_message_keeps_its_images() {
        let app = ctx("image-edit");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "draft", vec![png("DDDD")], "queue").unwrap();
        let id = m.snapshot("s").queue[0].id.clone();
        m.queue_update(&app, "s", &id, "what is in this image").unwrap();
        finish_turn(&m, &app, "s", "success");
        assert_eq!(said(&m, "s"), vec!["first", "what is in this image"]);
        assert_eq!(shown(&m, "s")[1], vec!["DDDD".to_string()]);
    }

    /// Only changed rows are sent, and each is sent once however many times it changed.
    #[test]
    fn flush_reports_each_changed_row_once() {
        let mut timeline = Timeline::default();
        timeline.upsert(ChatRow::Assistant {
            id: "a".into(), text: "1".into(), streaming: true, at: None, model: None, duration_ms: None,
        });
        timeline.upsert(ChatRow::Assistant {
            id: "a".into(), text: "12".into(), streaming: true, at: None, model: None, duration_ms: None,
        });
        timeline.upsert(ChatRow::User { id: "u".into(), text: "hi".into(), images: Vec::new(), at: None });
        let dirty = timeline.take_dirty();
        assert_eq!(dirty.len(), 2);
        assert!(matches!(&dirty[0], ChatRow::Assistant { text, .. } if text == "12"));
        assert!(timeline.take_dirty().is_empty(), "a second flush has nothing to send");
    }

    #[test]
    fn a_turn_duration_belongs_only_to_its_last_answer() {
        let mut timeline = Timeline::default();
        timeline.upsert(ChatRow::User {
            id: "u1".into(), text: "question".into(), images: Vec::new(), at: Some(1_000),
        });
        timeline.upsert(ChatRow::Assistant {
            id: "a1".into(), text: "before the tool".into(), streaming: false, at: Some(2_000), model: None, duration_ms: None,
        });
        timeline.upsert(ChatRow::Assistant {
            id: "a2".into(), text: "final answer".into(), streaming: false, at: Some(3_000), model: None, duration_ms: None,
        });
        timeline.finish_latest_turn(78_123);

        assert!(matches!(&timeline.rows[1], ChatRow::Assistant { duration_ms: None, .. }));
        assert!(matches!(&timeline.rows[2], ChatRow::Assistant { duration_ms: Some(78_123), .. }));
        assert!(timeline.take_dirty().iter().any(|row| row.id() == "a2"));
    }

    #[test]
    fn steering_keeps_the_original_turn_clock() {
        let app = ctx("steer-clock");
        let manager = manager_with_session("s");
        manager.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        let proc = manager.get("s").unwrap();
        proc.turn.lock().unwrap().started_at = Some(123);

        assert_eq!(manager.send(&app, "s", "clarification", Vec::new(), "steer").unwrap(), "sent");
        assert_eq!(manager.snapshot("s").turn_started_at, Some(123));
    }

    #[test]
    fn completed_duration_is_fixed_and_a_queued_turn_gets_a_new_clock() {
        let app = ctx("duration-queue");
        let manager = manager_with_session("s");
        manager.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        let proc = manager.get("s").unwrap();
        assert!(manager.snapshot("s").turn_started_at.is_some());
        proc.timeline.lock().unwrap().upsert(ChatRow::Assistant {
            id: "a1".into(), text: "done".into(), streaming: false, at: Some(2_000), model: None, duration_ms: None,
        });
        manager.send(&app, "s", "second", Vec::new(), "queue").unwrap();

        handle_turn_end(&app, "s", &proc, "success", Some(78_123));
        let snapshot = manager.snapshot("s");
        assert!(snapshot.turn_started_at.is_some(), "the queued turn starts its own live clock");
        assert!(matches!(
            snapshot.rows.iter().find(|row| row.id() == "a1"),
            Some(ChatRow::Assistant { duration_ms: Some(78_123), .. })
        ));

        handle_turn_end(&app, "s", &proc, "success", None);
        assert_eq!(manager.snapshot("s").turn_started_at, None, "an idle session has no live clock");
    }

    #[test]
    fn a_failed_queued_dispatch_stops_the_live_clock() {
        let app = ctx("duration-queue-failure");
        let events = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        app.listen(&event_name("s"), move |payload| {
            captured.lock().unwrap().push(serde_json::from_str(payload).unwrap());
        });
        let manager = manager_with_session("s");
        manager.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        manager.send(&app, "s", "second", Vec::new(), "queue").unwrap();
        let proc = manager.get("s").unwrap();
        *proc.stdin.lock().unwrap() = None;

        handle_turn_end(&app, "s", &proc, "success", Some(1_000));

        assert_eq!(manager.snapshot("s").turn_started_at, None);
        assert!(events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.get("type").and_then(Value::as_str) == Some("turnCompleted")));
    }

    /// Exercise the actual Codex wire path without contacting a model or running its proposed tools.
    fn codex_permission_fixture(tag: &str) -> (AppCtx, Arc<ChatProcess>, BufReader<std::process::ChildStdout>) {
        let app = ctx(tag);
        {
            let conn = app.db().conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,name,root_path,created_at) VALUES ('p','test','/tmp',0)", []).unwrap();
            conn.execute("INSERT INTO sessions(id,project_id,name,kind,permission_mode,created_at) VALUES ('s','p','Permissions','codex','auto',0)", []).unwrap();
        }
        let (mut proc, stdout) = cat_process();
        Arc::get_mut(&mut proc).unwrap().kind = SessionKind::Codex;
        *proc.mode.lock().unwrap() = "auto".into();
        *proc.agent_session_id.lock().unwrap() = Some("thread-permissions".into());
        app.chat().sessions.lock().unwrap().insert("s".into(), proc.clone());
        (app, proc, BufReader::new(stdout))
    }

    fn read_codex_request(reader: &mut BufReader<std::process::ChildStdout>) -> Value {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn accept_codex_permission_turn(app: &AppCtx, proc: &Arc<ChatProcess>, request: &Value, turn_id: &str) {
        handle_codex_line(app, "s", proc, &json!({
            "id":request["id"], "result":{"turn":{"id":turn_id,"status":"inProgress"}}
        }).to_string());
        handle_codex_notification(app, "s", proc, "turn/started", json!({"turn":{"id":turn_id}}));
    }

    #[test]
    fn codex_permissions_wait_for_the_next_turn_without_interrupting_or_approving() {
        let (app, proc, mut reader) = codex_permission_fixture("codex-permission-next-turn");
        let events = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        app.listen(&event_name("s"), move |payload| {
            captured.lock().unwrap().push(serde_json::from_str(payload).unwrap());
        });
        let manager = app.chat();
        manager.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        let first = read_codex_request(&mut reader);
        assert_eq!(first["params"]["approvalPolicy"], "on-request");
        accept_codex_permission_turn(&app, &proc, &first, "turn-first");
        let started_at = manager.snapshot("s").turn_started_at;
        handle_codex_request(&app, "s", &proc, json!(77), "item/commandExecution/requestApproval", json!({
            "threadId":"thread-permissions", "turnId":"turn-first", "itemId":"cmd", "command":"test command"
        }));
        let permissions = manager.snapshot("s").permissions;
        assert_eq!(permissions.len(), 1);

        for (selected, pending) in [("full-access", true), ("read-only", true), ("auto", false), ("full-access", true)] {
            crate::command_core::chat_set_mode(&app, "s", selected).unwrap();
            let snapshot = manager.snapshot("s");
            assert_eq!(snapshot.mode.as_deref(), Some(selected));
            assert_eq!(snapshot.pending_permission_mode, pending.then(|| PendingPermissionMode {
                current: "auto".into(), next: selected.into(),
            }));
            assert_eq!(snapshot.permissions, permissions);
            assert_eq!(snapshot.turn_started_at, started_at);
            assert!(!proc.turn.lock().unwrap().interrupted);
            assert_eq!(proc.current_turn.lock().unwrap().as_deref(), Some("turn-first"));
            let event = events.lock().unwrap().last().unwrap().clone();
            assert_eq!(event["type"], "settingsChanged");
            assert_eq!(event["pendingPermissionMode"], serde_json::to_value(snapshot.pending_permission_mode).unwrap());
        }
        let stored: String = app.db().conn.lock().unwrap().query_row(
            "SELECT permission_mode FROM sessions WHERE id = 's'", [], |row| row.get(0),
        ).unwrap();
        assert_eq!(stored, "skip");

        // If changing mode wrote an interrupt or an approval, it would appear before this steer.
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| manager.send(&app, "s", "keep going", Vec::new(), "steer"));
            let request = read_codex_request(&mut reader);
            assert_eq!(request["method"], "turn/steer");
            assert!(request["params"].get("sandboxPolicy").is_none());
            handle_codex_line(&app, "s", &proc, &json!({"id":request["id"], "result":{"turnId":"turn-first"}}).to_string());
            assert_eq!(worker.join().unwrap().unwrap(), "sent");
        });
        assert!(manager.snapshot("s").pending_permission_mode.is_some());
        manager.send(&app, "s", "second", Vec::new(), "queue").unwrap();
        handle_codex_notification(&app, "s", &proc, "turn/completed", json!({"turn":{"id":"turn-first","status":"completed"}}));
        let second = read_codex_request(&mut reader);
        assert_eq!(second["method"], "turn/start");
        assert_eq!(second["params"]["approvalPolicy"], "never");
        assert_eq!(second["params"]["sandboxPolicy"]["type"], "dangerFullAccess");
        assert!(manager.snapshot("s").pending_permission_mode.is_some());
        accept_codex_permission_turn(&app, &proc, &second, "turn-second");
        assert!(manager.snapshot("s").pending_permission_mode.is_none());
        assert!(events.lock().unwrap().iter().any(|event| event["type"] == "settingsChanged"
            && event["mode"] == "full-access" && event.get("pendingPermissionMode") == Some(&Value::Null)));
        manager.stop(&app, "s").unwrap();
    }

    #[test]
    fn codex_permission_ack_keeps_newer_choices_and_rejections_keep_the_pending_mode() {
        let (app, proc, mut reader) = codex_permission_fixture("codex-permission-ack");
        let manager = app.chat();
        manager.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        let first = read_codex_request(&mut reader);
        accept_codex_permission_turn(&app, &proc, &first, "turn-first");
        crate::command_core::chat_set_mode(&app, "s", "full-access").unwrap();
        manager.send(&app, "s", "second", Vec::new(), "queue").unwrap();
        handle_codex_notification(&app, "s", &proc, "turn/completed", json!({"turn":{"status":"completed"}}));
        let second = read_codex_request(&mut reader);
        handle_codex_line(&app, "s", &proc, &json!({
            "id":second["id"],"error":{"code":-32600,"message":"policy rejected"}
        }).to_string());
        assert_eq!(manager.snapshot("s").pending_permission_mode, Some(PendingPermissionMode {
            current:"auto".into(), next:"full-access".into(),
        }));
        assert!(proc.codex_permission_state.lock().unwrap().requests.is_empty());
        manager.send(&app, "s", "retry", Vec::new(), "queue").unwrap();
        let retry = read_codex_request(&mut reader);
        assert_eq!(retry["params"]["approvalPolicy"], "never");
        crate::command_core::chat_set_mode(&app, "s", "read-only").unwrap();
        accept_codex_permission_turn(&app, &proc, &retry, "turn-retry");
        // The accepted request carried Full Access, even though the control now selects Read Only.
        assert_eq!(manager.snapshot("s").pending_permission_mode, Some(PendingPermissionMode {
            current:"full-access".into(), next:"read-only".into(),
        }));
        crate::command_core::chat_set_mode(&app, "s", "full-access").unwrap();
        assert!(manager.snapshot("s").pending_permission_mode.is_none());
        manager.stop(&app, "s").unwrap();
    }

    #[test]
    fn codex_permission_handshake_and_write_failure_preserve_the_confirmed_policy() {
        let (app, proc, mut reader) = codex_permission_fixture("codex-permission-handshake");
        open_codex_thread(&app, "s", &proc);
        let request = read_codex_request(&mut reader);
        assert_eq!(request["method"], "thread/resume");
        assert_eq!(request["params"]["excludeTurns"], true);
        crate::command_core::chat_set_mode(&app, "s", "full-access").unwrap();
        handle_codex_line(&app, "s", &proc, &json!({
            "id":request["id"],"result":{"thread":{"id":"thread-permissions"}}
        }).to_string());
        assert_eq!(app.chat().snapshot("s").pending_permission_mode, Some(PendingPermissionMode {
            current:"auto".into(), next:"full-access".into(),
        }));
        *proc.stdin.lock().unwrap() = None;
        assert!(app.chat().send(&app, "s", "cannot write", Vec::new(), "queue").is_err());
        assert!(proc.codex_permission_state.lock().unwrap().requests.is_empty());
        assert_eq!(app.chat().snapshot("s").pending_permission_mode.unwrap().current, "auto");
        app.chat().stop(&app, "s").unwrap();
    }

    #[test]
    fn claude_settings_commit_only_after_success_and_keep_previous_values_on_rejection() {
        let app = ctx("settings-ack");
        let (proc, stdout) = cat_process();
        *proc.model.lock().unwrap() = Some("old-model".into());
        let manager = app.chat();
        {
            let conn = app.db().conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,name,root_path,created_at) VALUES ('p','test','/tmp',0)", []).unwrap();
            conn.execute("INSERT INTO sessions(id,project_id,name,kind,permission_mode,created_at) VALUES ('s','p','test','claude','default',0)", []).unwrap();
        }
        let events = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        app.listen(&event_name("s"), move |payload| {
            captured.lock().unwrap().push(serde_json::from_str(payload).unwrap());
        });
        manager.sessions.lock().unwrap().insert("s".into(), proc.clone());
        let mut reader = BufReader::new(stdout);
        for mode_change in [false, true] {
            for accepted in [false, true] {
                std::thread::scope(|scope| {
                    let worker = scope.spawn(|| {
                        if mode_change { crate::command_core::chat_set_mode(&app, "s", "plan") }
                        else { crate::command_core::chat_set_model(&app, "s", Some("new-model")) }
                    });
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    let request: Value = serde_json::from_str(&line).unwrap();
                    let before = manager.snapshot("s");
                    if mode_change { assert_eq!(before.mode.as_deref(), Some("default")); }
                    else { assert_eq!(before.model.as_deref(), Some("old-model")); }
                    handle_control_response(&app, "s", &proc, request["request_id"].as_str().unwrap(),
                        json!({}), (!accepted).then(|| "not available".to_string()));
                    assert_eq!(worker.join().unwrap().is_ok(), accepted);
                });
                let emitted = std::mem::take(&mut *events.lock().unwrap());
                assert_eq!(emitted.iter().any(|event| event["type"] == "settingsChanged"), accepted);
                let stored: String = app.db().conn.lock().unwrap().query_row(
                    "SELECT permission_mode FROM sessions WHERE id = 's'", [], |row| row.get(0),
                ).unwrap();
                assert_eq!(stored, if mode_change && accepted { "plan" } else { "default" });
                let after = manager.snapshot("s");
                if mode_change { assert_eq!(after.mode.as_deref(), Some(if accepted { "plan" } else { "default" })); }
                else { assert_eq!(after.model.as_deref(), Some(if accepted { "new-model" } else { "old-model" })); }
            }
        }
        manager.stop(&app, "s").unwrap();
    }

    #[test]
    fn settings_write_failure_does_not_replace_confirmed_state() {
        let app = ctx("settings-write-failure");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        *proc.stdin.lock().unwrap() = None;
        assert!(manager.set_model("s", Some("unavailable")).is_err());
        assert!(manager.set_mode(&app, "s", "plan").is_err());
        assert_eq!(manager.snapshot("s").model, None);
        assert_eq!(manager.snapshot("s").mode.as_deref(), Some("default"));
        assert!(proc.pending.lock().unwrap().is_empty());
        assert!(proc.waiters.lock().unwrap().is_empty());
        manager.stop(&app, "s").unwrap();
    }

    #[test]
    fn detached_claude_waits_for_background_shell_and_subagent_before_releasing() {
        let app = ctx("detach-background");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        manager.send(&app, "s", "work", Vec::new(), "queue").unwrap();
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_started","task_id":"agent","task_type":"local_agent","tool_use_id":"tool"}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"shell","task_type":"local_bash"}]}"#);
        manager.detach(&app, "s");
        finish_turn(&manager, &app, "s", "success");
        assert!(running(&manager, "s"));
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[]}"#);
        assert!(running(&manager, "s"), "the subagent still owns work");
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_notification","task_id":"agent","status":"completed","summary":"done"}"#);
        assert!(running(&manager, "s"), "the dropped shell's terminal frame is still expected");
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_notification","task_id":"shell","status":"completed"}"#);
        assert!(!running(&manager, "s"));
        assert!(manager.snapshot("s").rows.iter().any(|row| matches!(row, ChatRow::Tool { output: Some(output), .. } if output == "done")));
    }

    #[test]
    fn reopening_during_background_work_cancels_release_and_stopping_remains_explicit() {
        for reopen in [false, true] {
            let app = ctx("detach-background-reopen");
            let manager = manager_with_session("s");
            let proc = manager.get("s").unwrap();
            handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"shell","task_type":"local_bash"}]}"#);
            manager.detach(&app, "s");
            assert!(running(&manager, "s"));
            if reopen {
                manager.attach("s");
                handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_notification","task_id":"shell","status":"failed"}"#);
                assert!(running(&manager, "s"));
            }
            manager.stop(&app, "s").unwrap();
            assert!(!running(&manager, "s"));
        }
    }

    /// The real wire order of a finishing task: the inventory empties FIRST, then `task_updated` and
    /// `task_notification` follow. A detached process must survive the drop until the notification has
    /// landed, or a task tab that outlived its conversation pane never gets status, summary or output file.
    #[test]
    fn a_detached_release_waits_for_the_terminal_frames_of_a_dropped_task() {
        let app = ctx("detach-settle");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"worwyzf75","task_type":"local_workflow","description":"protocol probe"}]}"#);
        manager.detach(&app, "s");
        assert!(running(&manager, "s"));

        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[]}"#);
        assert!(running(&manager, "s"), "the drop alone does not release: the terminal frames are still coming");
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_updated","task_id":"worwyzf75","patch":{"status":"completed","end_time":1789036359792}}"#);
        assert!(running(&manager, "s"), "the notification with summary and output file is still coming");
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_notification","task_id":"worwyzf75","status":"completed","output_file":"/tmp/tasks/worwyzf75.output","summary":"Dynamic workflow \"protocol probe\" completed"}"#);
        assert!(!running(&manager, "s"), "the last terminal frame releases the process");

        let listed = background_tasks(&manager);
        assert_eq!(listed[0].status, "completed");
        assert_eq!(listed[0].ended_at, Some(1789036359792));
        assert_eq!(listed[0].output_file.as_deref(), Some("/tmp/tasks/worwyzf75.output"));
        assert!(listed[0].summary.as_deref().unwrap_or("").starts_with("Dynamic workflow"));
        assert!(proc.released.load(Ordering::Relaxed));
    }

    /// A dropped task whose terminal frames never arrive must not keep the process alive forever.
    #[test]
    fn a_dropped_task_without_terminal_frames_releases_after_the_grace() {
        let app = ctx("detach-settle-grace");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"shell","task_type":"local_bash"}]}"#);
        manager.detach(&app, "s");
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[]}"#);
        assert!(running(&manager, "s"));

        let deadline = std::time::Instant::now() + TASK_SETTLE_GRACE + Duration::from_secs(5);
        while running(&manager, "s") {
            assert!(std::time::Instant::now() < deadline, "the process was never released after the grace");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(proc.released.load(Ordering::Relaxed));
        assert_eq!(background_tasks(&manager)[0].status, "ended");
    }

    // ── Background tasks as the composer and the task tabs see them ──

    /// The published task list for session `s`.
    fn background_tasks(manager: &ChatManager) -> Vec<BackgroundTask> {
        manager.snapshot("s").extras.background_tasks
    }

    /// The workflow probe (Claude Code 2.1.267) in wire order: the inventory empties BEFORE the terminal
    /// frames arrive, so a list that merely mirrored the inventory could never show a final state.
    #[test]
    fn background_task_extras_merge_the_task_protocol_and_survive_the_inventory_drop() {
        let app = ctx("task-merge");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"worwyzf75","task_type":"local_workflow","description":"protocol probe"}]}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].description, "protocol probe");
        assert_eq!(listed[0].status, "running");
        assert!(listed[0].started_at.is_some());

        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_started","task_id":"worwyzf75","tool_use_id":"toolu_01","description":"protocol probe","task_type":"local_workflow","workflow_name":"probe","prompt":"export const meta = {}"}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_progress","task_id":"worwyzf75","tool_use_id":"toolu_01","description":"Alpha: alpha-worker","usage":{"total_tokens":0,"tool_uses":0,"duration_ms":20},"last_tool_name":"alpha-worker","summary":"protocol probe"}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed.len(), 1, "progress frames update the entry rather than adding one");
        assert_eq!(listed[0].description, "Alpha: alpha-worker");
        assert_eq!(listed[0].last_tool_name.as_deref(), Some("alpha-worker"));
        assert_eq!(listed[0].summary.as_deref(), Some("protocol probe"));
        assert_eq!(listed[0].tool_use_id.as_deref(), Some("toolu_01"));
        assert_eq!(listed[0].workflow_name.as_deref(), Some("probe"));
        assert!(listed[0].workflow_progress.is_none());

        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_progress","task_id":"worwyzf75","description":"Beta: gamma-worker","usage":{"total_tokens":64131,"tool_uses":0,"duration_ms":5000},"last_tool_name":"gamma-worker","summary":"protocol probe","workflow_progress":[{"type":"workflow_phase","index":1,"title":"Alpha"},{"type":"workflow_phase","index":2,"title":"Beta"},{"type":"workflow_agent","index":1,"label":"alpha-worker","phaseIndex":1,"phaseTitle":"Alpha","model":"haiku","state":"done","startedAt":1789036353475,"attempt":1,"durationMs":1200,"tokens":300,"resultPreview":"ALPHA"},{"type":"workflow_agent","index":2,"label":"beta-worker","phaseIndex":1,"phaseTitle":"Alpha","model":"haiku","state":"done","resultPreview":"BETA"},{"type":"workflow_agent","index":3,"label":"gamma-worker","phaseIndex":2,"phaseTitle":"Beta","model":"haiku","state":"start","startedAt":1789036356000}]}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed[0].description, "Beta: gamma-worker");
        assert_eq!(listed[0].usage.as_ref().and_then(|usage| usage.get("total_tokens")).and_then(Value::as_u64), Some(64131));
        assert_eq!(listed[0].workflow_progress.as_ref().and_then(Value::as_array).map(Vec::len), Some(5));

        // Only about every second frame carries the tree; the last one seen stays.
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_progress","task_id":"worwyzf75","description":"Beta: gamma-worker","usage":{"total_tokens":64131,"tool_uses":0,"duration_ms":6000},"last_tool_name":"gamma-worker","summary":"protocol probe"}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed[0].workflow_progress.as_ref().and_then(Value::as_array).map(Vec::len), Some(5));

        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[]}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed.len(), 1, "an emptied inventory keeps the task with a final state");
        assert_eq!(listed[0].status, "ended");
        assert!(listed[0].ended_at.is_some());

        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_updated","task_id":"worwyzf75","patch":{"status":"completed","end_time":1789036359792}}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed[0].status, "completed");
        assert_eq!(listed[0].ended_at, Some(1789036359792));

        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_notification","task_id":"worwyzf75","tool_use_id":"toolu_01","status":"completed","output_file":"/tmp/tasks/worwyzf75.output","summary":"Dynamic workflow \"protocol probe\" completed","usage":{"total_tokens":64131,"tool_uses":0,"duration_ms":6324}}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, "completed");
        assert!(listed[0].summary.as_deref().unwrap_or("").starts_with("Dynamic workflow"));
        assert_eq!(listed[0].output_file.as_deref(), Some("/tmp/tasks/worwyzf75.output"));
        assert_eq!(listed[0].ended_at, Some(1789036359792), "the notification does not overwrite the reported end time");
        assert!(listed[0].started_at.is_some());
        assert_eq!(listed[0].usage.as_ref().and_then(|usage| usage.get("duration_ms")).and_then(Value::as_u64), Some(6324));
        manager.stop(&app, "s").unwrap();
    }

    /// The reverse wire order: a terminal frame lands BEFORE the inventory drops the task. The final status
    /// must not be downgraded to the synthesized "ended", and the client must see it in an `extras` event,
    /// not only in a snapshot it may never request.
    #[test]
    fn a_terminal_frame_before_the_inventory_drop_keeps_its_status_and_is_published_as_an_event() {
        let app = ctx("task-terminal-first");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        let events = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        app.listen(&event_name("s"), move |payload| {
            captured.lock().unwrap().push(serde_json::from_str(payload).unwrap());
        });
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"agent-z","task_type":"local_agent","description":"Inspect the parser"}]}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_updated","task_id":"agent-z","patch":{"status":"failed","end_time":1789036359792}}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[]}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, "failed", "the inventory drop must not overwrite a terminal status");
        assert_eq!(listed[0].ended_at, Some(1789036359792));

        let extras: Vec<Value> = events.lock().unwrap().iter().filter(|event| event["type"] == "extras").cloned().collect();
        assert!(extras.len() >= 3, "every inventory and task frame republishes extras, got {}", extras.len());
        let last = &extras.last().unwrap()["extras"]["backgroundTasks"];
        assert_eq!(last.as_array().map(Vec::len), Some(1), "the final state rides in the event, not only the snapshot");
        assert_eq!(last[0]["task_id"], "agent-z");
        assert_eq!(last[0]["status"], "failed");
        assert_eq!(last[0]["ended_at"], 1789036359792u64);
        assert!(last[0].get("listed").is_none(), "the internal `listed` flag never reaches the wire");
        manager.stop(&app, "s").unwrap();
    }

    /// Finished tasks stay on screen until the user's next turn, and never pile up without bound.
    #[test]
    fn finished_background_tasks_are_pruned_when_the_next_turn_starts() {
        let app = ctx("task-prune");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"shell-a","task_type":"local_bash","description":"npm test"}]}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[]}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_notification","task_id":"shell-a","status":"completed","summary":"ok"}"#);
        assert_eq!(background_tasks(&manager).len(), 1);
        manager.send(&app, "s", "next", Vec::new(), "queue").unwrap();
        assert!(background_tasks(&manager).is_empty(), "a new turn retires the finished tasks");

        // Twenty-one finished shells collapse to the cap, oldest first.
        let inventory: Vec<String> = (0..21).map(|n| format!(r#"{{"task_id":"shell-{n}","task_type":"local_bash","description":"job {n}"}}"#)).collect();
        handle_line(&app, "s", &proc, &format!(r#"{{"type":"system","subtype":"background_tasks_changed","tasks":[{}]}}"#, inventory.join(",")));
        assert_eq!(background_tasks(&manager).len(), 21);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[]}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed.len(), FINISHED_TASK_CAP);
        assert_eq!(listed[0].task_id, "shell-1");
        assert!(listed.iter().all(|task| task.status == "ended"));
        manager.stop(&app, "s").unwrap();
    }

    /// A foreground subagent is known from its task frames alone. It is not a background task until the
    /// inventory names it (the user moved the turn's work to the background), but its facts are kept so
    /// that moment shows the whole picture, not just the inventory's static line.
    #[test]
    fn foreground_subagents_stay_unlisted_until_the_inventory_names_them() {
        let app = ctx("task-foreground");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_started","task_id":"agent-a","tool_use_id":"tool-a","task_type":"local_agent","subagent_type":"Explore","description":"Inspect the parser","prompt":"Find it"}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_progress","task_id":"agent-a","description":"Reading parser.rs","usage":{"total_tokens":1234,"tool_uses":3,"duration_ms":90}}"#);
        assert!(background_tasks(&manager).is_empty(), "foreground work is not a background task");
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"agent-a","task_type":"local_agent","description":"Inspect the parser"}]}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].tool_use_id.as_deref(), Some("tool-a"));
        assert_eq!(listed[0].description, "Reading parser.rs", "the inventory's static text does not overwrite the live description");
        assert_eq!(listed[0].usage.as_ref().and_then(|usage| usage.get("tool_uses")).and_then(Value::as_u64), Some(3));
        manager.stop(&app, "s").unwrap();
    }

    /// Frames missing the identity or the fields this list reads must neither panic nor invent entries.
    #[test]
    fn task_frames_without_task_id_or_fields_are_ignored_safely() {
        let app = ctx("task-lenient");
        let manager = manager_with_session("s");
        let proc = manager.get("s").unwrap();
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_progress"}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_started","task_id":""}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"description":"no id"},{"task_id":"shell-b"}]}"#);
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_progress","task_id":"shell-b","usage":"not an object","workflow_progress":{"not":"an array"}}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].task_id, "shell-b");
        assert_eq!(listed[0].task_type, "");
        assert!(listed[0].usage.is_none());
        assert!(listed[0].workflow_progress.is_none());
        handle_line(&app, "s", &proc, r#"{"type":"system","subtype":"task_notification","task_id":"shell-b"}"#);
        let listed = background_tasks(&manager);
        assert_eq!(listed[0].status, "ended", "a notification without a status still ends the task");
        assert!(listed[0].ended_at.is_some());
        manager.stop(&app, "s").unwrap();
    }

    // ── Releasing the process when its view goes away ──

    /// Whether the session's process is still running, as the snapshot reports it.
    fn running(manager: &ChatManager, session_id: &str) -> bool {
        manager.snapshot(session_id).running
    }

    /// Closing the view of an idle conversation ends its process at once, but keeps the conversation:
    /// a pane opened later still shows what was said.
    #[test]
    fn detaching_an_idle_conversation_releases_its_process_and_keeps_the_timeline() {
        let app = ctx("detach-idle");
        let m = manager_with_session("s");
        m.send(&app, "s", "hello", Vec::new(), "queue").unwrap();
        finish_turn(&m, &app, "s", "success");

        m.detach(&app, "s");

        assert!(!running(&m, "s"));
        assert_eq!(said(&m, "s"), vec!["hello"]);
        let proc = m.get("s").unwrap();
        assert!(proc.released.load(Ordering::Relaxed));
        assert!(proc.stdin.lock().unwrap().is_none());
    }

    /// Closing the view mid-answer must not cut the answer off: the process stays until the turn ends.
    #[test]
    fn detaching_during_a_turn_waits_for_the_turn_to_end() {
        let app = ctx("detach-running");
        let m = manager_with_session("s");
        m.send(&app, "s", "work", Vec::new(), "queue").unwrap();

        m.detach(&app, "s");
        assert!(running(&m, "s"));

        finish_turn(&m, &app, "s", "success");
        assert!(!running(&m, "s"));
        assert!(m.get("s").unwrap().released.load(Ordering::Relaxed));
    }

    /// Messages typed before the view closed are still delivered; the release waits for the queue too.
    #[test]
    fn a_pending_release_waits_for_the_queue_to_drain() {
        let app = ctx("detach-queue");
        let m = manager_with_session("s");
        m.send(&app, "s", "first", Vec::new(), "queue").unwrap();
        m.send(&app, "s", "second", Vec::new(), "queue").unwrap();

        m.detach(&app, "s");
        finish_turn(&m, &app, "s", "success");
        assert!(running(&m, "s"));
        assert_eq!(said(&m, "s"), vec!["first", "second"]);

        finish_turn(&m, &app, "s", "success");
        assert!(!running(&m, "s"));
    }

    /// Opening the view again before the turn ends cancels the release.
    #[test]
    fn attaching_again_cancels_a_pending_release() {
        let app = ctx("detach-attach");
        let m = manager_with_session("s");
        m.send(&app, "s", "work", Vec::new(), "queue").unwrap();

        m.detach(&app, "s");
        m.attach("s");
        finish_turn(&m, &app, "s", "success");

        assert!(running(&m, "s"));
        assert!(!m.get("s").unwrap().released.load(Ordering::Relaxed));
    }

    /// The exit that follows a release is reported as released, so a pane still showing the
    /// conversation elsewhere does not draw it as the agent failing. This runs the real reader thread
    /// against the application's own manager, since that is where the reader looks the session up.
    #[test]
    fn a_released_process_reports_its_exit_as_released() {
        let app = ctx("release-exit");
        let events = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        app.listen(&event_name("s"), move |payload| {
            captured.lock().unwrap().push(serde_json::from_str(payload).unwrap());
        });
        let (proc, stdout) = cat_process();
        app.chat().sessions.lock().unwrap().insert("s".to_string(), proc.clone());
        spawn_stdout_reader(app.clone(), "s".to_string(), proc.clone(), stdout);

        app.chat().detach(&app, "s");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let exited = loop {
            let found = events
                .lock()
                .unwrap()
                .iter()
                .find(|event| event.get("type").and_then(Value::as_str) == Some("exited"))
                .cloned();
            if let Some(event) = found {
                break event;
            }
            assert!(std::time::Instant::now() < deadline, "the reader never reported the exit");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(exited.get("released"), Some(&Value::Bool(true)));
        assert!(!app.chat().snapshot("s").running);
    }

    /// A session that has no process, or whose process already went, has nothing to release.
    #[test]
    fn detaching_without_a_running_process_is_a_no_op() {
        let app = ctx("detach-none");
        let m = manager_with_session("s");
        m.detach(&app, "missing");
        let proc = m.get("s").unwrap();
        proc.alive.store(false, Ordering::Relaxed);

        m.detach(&app, "s");

        assert!(!proc.released.load(Ordering::Relaxed));
    }

    #[test]
    fn rewind_trim_drops_the_target_and_everything_after_it() {
        let mut timeline = Timeline::default();
        timeline.upsert(ChatRow::User {
            id: "u1".into(), text: "one".into(), images: Vec::new(), at: None,
        });
        timeline.upsert(ChatRow::Assistant {
            id: "a1".into(), text: "answer one".into(), streaming: false, at: None, model: None, duration_ms: None,
        });
        timeline.upsert(ChatRow::User {
            id: "u2".into(), text: "two".into(), images: Vec::new(), at: None,
        });
        timeline.upsert(ChatRow::Assistant {
            id: "a2".into(), text: "answer two".into(), streaming: false, at: None, model: None, duration_ms: None,
        });

        assert!(timeline.trim_from_user("u2"));
        assert_eq!(timeline.rows.len(), 2);
        assert_eq!(timeline.rows[0].id(), "u1");
        assert_eq!(timeline.rows[1].id(), "a1");
        assert!(matches!(timeline.take_flush(), TimelineFlush::Replace(rows) if rows.len() == 2));
    }

    #[test]
    fn unknown_rewind_target_never_changes_or_replaces_the_timeline() {
        let mut timeline = Timeline::default();
        timeline.upsert(ChatRow::User {
            id: "u1".into(), text: "one".into(), images: Vec::new(), at: None,
        });
        timeline.take_dirty();

        assert!(!timeline.trim_from_user("missing"));
        assert_eq!(timeline.rows.len(), 1);
        assert!(matches!(timeline.take_flush(), TimelineFlush::None));
    }
}
