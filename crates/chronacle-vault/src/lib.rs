//! Backend-agnostic Markdown vault sync engine (ADR-008).
//!
//! Owns markdown rendering, key mapping, and the three-way sync decision.
//! Reaches storage and records only through the `chronacle-core` ports, so a
//! future S3 / WebDAV backend needs no change here. **This crate must never
//! depend on `std::fs`, `tokio::fs`, or `notify`.**

/// Errors surfaced by the vault engine.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("store error: {0}")]
    Store(#[from] chronacle_core::VaultStoreError),
    #[error("record error: {0}")]
    Record(#[from] chronacle_core::VaultRecordError),
    #[error("frontmatter error: {0}")]
    Frontmatter(String),
}
