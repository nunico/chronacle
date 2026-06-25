//! Tauri IPC command handlers, grouped by business domain.
//!
//! Each submodule owns the commands, request/response DTOs, and tests for one
//! domain. They are re-exported flat so `commands::<name>` keeps working in
//! `lib.rs`'s `generate_handler!` list and elsewhere.

pub mod entity_commands;
pub use entity_commands::*;

pub mod session_commands;
pub use session_commands::*;

pub mod extraction_commands;
pub use extraction_commands::*;

pub mod settings_commands;
pub use settings_commands::*;

pub mod chat_commands;
pub use chat_commands::*;

pub mod source_commands;
pub use source_commands::*;

pub mod collection_commands;
pub use collection_commands::*;

pub mod campaign_commands;
pub use campaign_commands::*;

pub mod llm_commands;
pub use llm_commands::*;

pub mod embedding_commands;
pub use embedding_commands::*;

pub mod custom_provider_commands;
pub use custom_provider_commands::*;
