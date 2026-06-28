use async_trait::async_trait;

/// A single search result returned from the vector store.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk_id: String,
    pub source_id: String,
    pub source_name: String,
    pub text: String,
    pub page_start: i64,
    pub page_end: i64,
    pub section_heading: String,
    pub source_type: String,
    pub distance: f64,
}

/// Errors that can arise from vector-store operations.
#[derive(Debug, thiserror::Error)]
pub enum VectorStoreError {
    #[error("Database error: {0}")]
    Db(String),
    #[error("Embedding dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("Not found: {0}")]
    NotFound(String),
}

/// Trait abstracting vector index operations over SurrealDB.
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(
        &self,
        source_id: &str,
        chunks: &[IndexedChunk],
    ) -> Result<(), VectorStoreError>;

    /// Search for the `limit` nearest neighbours to `query_vector`.
    ///
    /// Only chunks whose `collection` record link is in `collection_ids` are
    /// returned. Passing an empty slice returns `Ok(Vec::new())` immediately —
    /// callers must subscribe to at least one collection to receive results.
    ///
    /// # Note on MTREE and OR clauses
    ///
    /// SurrealDB's MTREE vector index cannot be combined with OR at the top
    /// level of a WHERE clause. The collection filter uses `IN [...]` which
    /// the query planner handles as a single AND clause, keeping the index
    /// path intact.
    async fn search(
        &self,
        query_vector: &[f32],
        collection_ids: &[String],
        limit: u64,
    ) -> Result<Vec<SearchResult>, VectorStoreError>;

    async fn delete_by_source(&self, source_id: &str) -> Result<(), VectorStoreError>;
}

/// A chunk of text that has been embedded and is ready for storage.
#[derive(Debug, Clone)]
pub struct IndexedChunk {
    pub chunk_id: String,
    /// Every chunk must belong to a collection — the schema defines
    /// `chunk.collection TYPE record<collection>` as non-nullable.
    pub collection_id: String,
    pub text: String,
    pub page_start: i64,
    pub page_end: i64,
    pub section_heading: String,
    pub source_type: String,
    pub embedding: Vec<f32>,
    pub embed_model: String,
}
