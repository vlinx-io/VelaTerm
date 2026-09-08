//! The chat engine: an agent driven as a protocol rather than as a terminal.
//!
//! A session running this engine has no PTY. VelaTerm starts the agent CLI in its streaming JSON mode and
//! talks to it over stdin/stdout — or, for OpenCode, starts its local server and talks HTTP to it: messages go in as JSON, and everything the agent does — text, reasoning,
//! tool calls and their results, permission questions, model and mode changes — comes back as structured
//! events. That is what lets the session view draw real cards and controls instead of reading a screen.
//!
//! Three parts:
//! - `history` reads a finished conversation back from the agent's own recording, and serves both this
//!   engine and the read-only session view of a terminal-driven session;
//! - `protocol` is the wire format: what the agent emits, and the control requests both sides exchange;
//! - `config_schema` asks the agent which settings it accepts, so the composer can complete them;
//! - `engine` owns the running processes, one per session, and turns the wire into timeline rows.

pub mod config_schema;
pub mod codex_protocol;
pub mod engine;
pub mod history;
pub mod opencode_protocol;
pub mod opencode_timeline;
pub mod protocol;
pub mod skills;
pub mod submissions;

pub use history::{read, ChatEvent};
