//! PTY subsystem: pseudoterminal creation, I/O, resizing, and cleanup.

pub mod manager;
pub mod monitor;
pub mod session;

/// Agent work state corresponding exactly to frontend `AgentState` using serde snake_case.
///
/// - working: actively processing.
/// - asking: paused for permission or user input.
/// - waiting: stopped with no pending confirmation, ready for review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Working,
    Asking,
    Waiting,
}

/// Backend session-status signals delivered through `pty://status/{id}`.
///
/// Two categories:
/// - Structured `State` signals come from official agent hooks/notifications. `authoritative=true`
///   means the source covers the complete lifecycle; legacy Codex notifications only report completion.
/// - `Agent` identifies the typed session and, for Codex, selects hook-only or legacy state semantics.
/// - Fallback `Busy`/`Bell`/`Title` signals infer state from byte activity and are consumed only by agent kinds
///   that explicitly retain fallback behavior. Codex never derives activity from them.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatusSignal {
    /// Structured agent work state from official events. `silent=true` corrects state without a system
    /// notification, used for Claude idle_prompt and stale snapshots replayed on attach.
    State {
        state: AgentState,
        #[serde(default)]
        silent: bool,
        /// True when the source covers the full turn lifecycle. This distinguishes complete Codex
        /// lifecycle hooks from legacy waiting-only notifications. Once seen, the frontend permanently
        /// disables screen/busy overrides for that session.
        #[serde(default)]
        authoritative: bool,
        /// True when terminal silence inferred this state instead of an agent lifecycle event.
        #[serde(default, skip_serializing_if = "is_false")]
        inferred: bool,
    },
    /// Agent kind set by typed spawn or detected from the process tree. Codex additionally reports whether
    /// lifecycle hooks are available so the frontend can select hook-only or legacy status semantics before the
    /// first lifecycle event arrives.
    Agent {
        agent: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        state_source: Option<String>,
    },
    /// Codex SessionStart hook health handshake. It proves that hook execution and callback transport work
    /// without falsely classifying a newly opened prompt as a completed turn.
    HookReady,
    /// Fallback output activity: true while output continues, false after the silence threshold.
    Busy { busy: bool },
    /// Fallback standalone bell.
    Bell,
    /// OSC terminal-title change for informational display.
    Title { title: String },
    /// Fallback OSC 9/777 notification with optional title and body. Covers tools run manually in a
    /// terminal and agents whose hooks failed. The backend emits without deduplication; the frontend
    /// rejects it after authoritative events and applies visibility guards. This transient signal is
    /// excluded from state snapshots.
    Notify { title: Option<String>, body: String },
    /// Active tool from Claude/Grok PreToolUse; None when Stop clears the display.
    Tool { tool: Option<String> },
    /// Agent executable missing from PATH, reported by the launch guard through `e=notfound`. The
    /// frontend shows installation guidance. This one-shot authoritative signal is excluded from
    /// snapshots and notifications, avoiding false positives from echoed guard commands.
    AgentMissing,
    /// PTY size or ownership change after resize or owner detachment. Mirrors adopt the new grid;
    /// `owner: None` means an unowned visible client may claim it with a normal resize.
    Resized {
        cols: u16,
        rows: u16,
        owner: Option<String>,
    },
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl StatusSignal {
    /// Construct a `pty://status/{id}` event name.
    pub fn event_name(id: &str) -> String {
        format!("pty://status/{id}")
    }
}
