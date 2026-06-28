use async_trait::async_trait;

/// Errors that can arise from blob-store operations.
#[derive(Debug, thiserror::Error)]
pub enum BlobStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Trait abstracting file storage for PDFs and other binary assets.
///
/// Files are organised under a root directory with sub-directories per
/// source ID.
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Persist a blob (e.g. a PDF) to the store.
    async fn store(
        &self,
        source_id: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<(), BlobStoreError>;

    /// Read a previously stored blob back into memory.
    async fn retrieve(&self, source_id: &str, filename: &str) -> Result<Vec<u8>, BlobStoreError>;

    /// Remove all blobs associated with `source_id`.
    async fn delete(&self, source_id: &str) -> Result<(), BlobStoreError>;
}
