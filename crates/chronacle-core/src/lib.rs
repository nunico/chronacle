//! Storage-agnostic dependency contracts (traits) and their DTOs for Chronacle.
//!
//! Concrete implementations live in `chronacle-providers`; consumers depend on
//! these traits so they can be reused by a future cloud server.
pub mod blob_store;
pub mod embedding;
pub mod llm;
pub mod vault;
pub mod vector_store;

pub use blob_store::BlobStore;
pub use embedding::{EmbeddingError, EmbeddingProvider};
pub use llm::{ChatMessage, LlmError, LlmProvider};
pub use vault::{
    EntityRecord, NoopOutbound, RuleEntryRecord, RulePageRef, SessionRecord, VaultEvent, VaultKey,
    VaultMetadata, VaultOutbound, VaultRecord, VaultRecordError, VaultRecordStore, VaultRef,
    VaultScope, VaultStore, VaultStoreError, VaultWatcher,
};
pub use vector_store::{IndexedChunk, SearchResult, VectorStore};
