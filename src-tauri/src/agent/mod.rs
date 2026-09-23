//! Agent status integration: local loopback HTTP service for official hooks/Codex notify plus launch-injection construction.
//!
//! Agents report state authoritatively instead of relying on terminal-output guesses. vlx-term launches the agent,
//! embeds the session ID and one-time token in the hook URL, and emits when the agent POSTs state changes here.

pub mod antigravity;
pub mod chat;
pub mod codex_models;
pub mod claude_models;
pub mod cli_client;
pub mod cline;
pub mod copilot;
pub mod crush;
pub mod cursor;
pub mod export;
pub mod executable;
pub mod gitbash;
pub mod grok;
pub mod headless;
pub mod history;
pub mod inject;
pub mod install;
pub mod kimi;
pub mod kiro;
pub mod model_catalog;
pub mod launch_options;
pub mod launch_models;
pub mod remote_model_catalog;
pub mod cli_model_catalog;
pub mod permission_catalog;
pub mod omp;
pub mod opencode;
pub mod opencode_models;
pub mod opencode_store;
pub mod pi;
pub mod pi_models;
pub mod resume;
pub mod session_settings;
pub mod server;
pub mod spawn_cli;
pub mod status_watch;
pub mod transcript;
pub mod usage;
pub mod usage_store;

pub mod permission_state;
pub mod plan_execute;
pub mod tell;
