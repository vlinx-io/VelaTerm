//! Bottom layer of the VelaTerm workspace. It depends on no other workspace crate, so every domain crate
//! may depend on it; it must stay free of Tauri, storage and agent logic.

pub mod diagnostics;
pub mod models;
