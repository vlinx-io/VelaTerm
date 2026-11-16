//! Bounded reads of native conversation history, indexed in memory from the providers' own records.

pub mod limits;
pub mod projection;
pub mod source;
pub mod store;
mod fields;
pub mod adapter;
pub mod api;
pub mod live_view;
pub mod search;
pub mod sqlite_source;
mod opencode;
pub mod deepseek_legacy;
mod kiro;
pub mod attachments;
pub mod children;
pub mod rewind;
mod shell_text;
pub mod compat;
pub mod event_stream;
pub mod markdown;
pub mod exports;
pub mod prose;
pub mod context;
pub mod refer;
pub mod work;
mod antigravity_adapter;

#[cfg(test)]
mod tests;
