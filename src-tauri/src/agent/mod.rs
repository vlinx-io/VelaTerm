//! Agent status integration: local loopback HTTP service for official hooks/Codex notify plus launch-injection construction.
//!
//! Agents report state authoritatively instead of relying on terminal-output guesses. vlx-term launches the agent,
//! embeds the session ID and one-time token in the hook URL, and emits when the agent POSTs state changes here.

pub mod antigravity;
pub mod cli_client;
pub mod ctl;
pub mod ctl_client;
pub mod cline;
pub mod copilot;
pub mod crush;
pub mod cursor;
pub mod export;
pub mod gitbash;
pub mod grok;
pub mod inject;
pub mod install;
pub mod kimi;
pub mod kiro;
pub mod opencode;
pub mod orchestration;
pub mod pi;
pub mod resume;
pub mod server;
pub mod spawn_cli;
pub mod transcript;
pub mod usage;
