//! Source commands — upload, list, delete PDF sources, re-index, and the
//! citation→chunk lookup that backs the chat citation popover.

mod citation;
mod query;
mod reindex;
mod upload;

// Glob re-exports propagate both the public functions AND the hidden
// `__cmd__*` / `__tauri_command_name_*` items that Tauri's `#[tauri::command]`
// macro generates in each submodule. Without them, `generate_handler!` in
// lib.rs cannot find the command descriptors.
pub use citation::*;
pub use query::*;
pub use reindex::*;
pub use upload::*;

use serde::Serialize;

/// Response shape for a source record returned over IPC.
#[derive(Debug, Clone, Serialize)]
pub struct SourceResponse {
    pub id: String,
    pub filename: String,
    pub display_name: String,
    pub source_type: String,
    pub page_count: i64,
    pub index_status: String,
    pub embed_model: String,
    pub collection_id: Option<String>,
}

/// The chunk text + locator returned for a citation popover.
#[derive(Serialize)]
pub struct CitationChunk {
    pub text: String,
    pub page_start: i64,
    pub page_end: i64,
    pub section_heading: String,
}

#[cfg(test)]
#[path = "citation_tests.rs"]
mod citation_tests;
#[cfg(test)]
#[path = "reindex_tests.rs"]
mod reindex_tests;
#[cfg(test)]
#[path = "source_tests.rs"]
mod source_tests;
