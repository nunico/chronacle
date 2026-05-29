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
///
/// All methods are asynchronous and accept `&self` so implementations can
/// share a pooled or cloned database handle.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Insert or update a batch of indexed chunks.
    async fn upsert(
        &self,
        source_id: &str,
        chunks: &[IndexedChunk],
    ) -> Result<(), VectorStoreError>;

    /// Search for the `limit` nearest neighbours to `query_vector`.
    ///
    /// When `campaign_id` is `Some` the search is scoped to that campaign
    /// plus global sources (`campaign IS NULL`). When `None` only global
    /// sources are searched.
    async fn search(
        &self,
        query_vector: &[f32],
        campaign_id: Option<&str>,
        limit: u64,
    ) -> Result<Vec<SearchResult>, VectorStoreError>;

    /// Remove all chunks belonging to `source_id`.
    async fn delete_by_source(&self, source_id: &str) -> Result<(), VectorStoreError>;
}

/// A chunk of text that has been embedded and is ready for storage.
#[derive(Debug, Clone)]
pub struct IndexedChunk {
    pub chunk_id: String,
    pub campaign_id: Option<String>,
    pub text: String,
    pub page_start: i64,
    pub page_end: i64,
    pub section_heading: String,
    pub source_type: String,
    pub embedding: Vec<f32>,
    pub embed_model: String,
}

/// SurrealDB-backed implementation of [`VectorStore`].
///
/// Stores chunk records in the `chunk` table and uses SurrealDB's built-in
/// MTREE vector index for ANN search.
pub struct SurrealDbVector<C: surrealdb::Connection> {
    db: surrealdb::Surreal<C>,
}

impl<C: surrealdb::Connection> SurrealDbVector<C> {
    pub fn new(db: surrealdb::Surreal<C>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl<C> VectorStore for SurrealDbVector<C>
where
    C: surrealdb::Connection + Send + Sync,
{
    async fn upsert(
        &self,
        source_id: &str,
        chunks: &[IndexedChunk],
    ) -> Result<(), VectorStoreError> {
        for chunk in chunks {
            // SurrealDB requires arrays as SurrealQL literals for the
            // embedding field. We build the query string inline.
            let vec_str = chunk
                .embedding
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let embedding_field = format!("[{}]", vec_str);

            let sql = format!(
                "CREATE chunk SET
                    id = $id,
                    source = $source_id,
                    campaign = $campaign_id,
                    text = $text,
                    page_start = $page_start,
                    page_end = $page_end,
                    section_heading = $section_heading,
                    source_type = $source_type,
                    embedding = {},
                    embed_model = $embed_model",
                embedding_field
            );

            let _ = self
                .db
                .query(sql)
                .bind(("id", chunk.chunk_id.clone()))
                .bind(("source_id", source_id.to_owned()))
                .bind(("campaign_id", chunk.campaign_id.clone()))
                .bind(("text", chunk.text.clone()))
                .bind(("page_start", chunk.page_start))
                .bind(("page_end", chunk.page_end))
                .bind(("section_heading", chunk.section_heading.clone()))
                .bind(("source_type", chunk.source_type.clone()))
                .bind(("embed_model", chunk.embed_model.clone()))
                .await
                .map_err(|e| VectorStoreError::Db(e.to_string()))?;
        }

        Ok(())
    }

    async fn search(
        &self,
        query_vector: &[f32],
        campaign_id: Option<&str>,
        limit: u64,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        let vec_str = query_vector
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let campaign_filter = match campaign_id {
            Some(cid) => format!(
                "WHERE campaign = {} OR campaign IS NONE",
                // SurrealQL record links: campaign:uuid
                format!("campaign:`{cid}`")
            ),
            None => "WHERE campaign IS NONE".to_string(),
        };

        let sql = format!(
            "SELECT
                id,
                source,
                source.filename AS source_name,
                text,
                page_start,
                page_end,
                section_heading,
                source_type,
                embedding <|1|> [{}] AS distance
            FROM chunk
            {}
            ORDER BY distance ASC
            LIMIT {}",
            vec_str, campaign_filter, limit
        );

        let mut response = self
            .db
            .query(sql)
            .await
            .map_err(|e| VectorStoreError::Db(e.to_string()))?;

        #[derive(Debug, serde::Deserialize)]
        struct RawResult {
            id: surrealdb::sql::Thing,
            source: surrealdb::sql::Thing,
            source_name: Option<String>,
            text: String,
            page_start: i64,
            page_end: i64,
            section_heading: String,
            source_type: String,
            distance: f64,
        }

        let raw: Vec<RawResult> = response
            .take(0)
            .map_err(|e| VectorStoreError::Db(e.to_string()))?;

        Ok(raw
            .into_iter()
            .map(|r| SearchResult {
                chunk_id: r.id.to_string(),
                source_id: r.source.to_string(),
                source_name: r.source_name.unwrap_or_else(|| r.source.to_string()),
                text: r.text,
                page_start: r.page_start,
                page_end: r.page_end,
                section_heading: r.section_heading,
                source_type: r.source_type,
                distance: r.distance,
            })
            .collect())
    }

    async fn delete_by_source(&self, source_id: &str) -> Result<(), VectorStoreError> {
        self.db
            .query("DELETE chunk WHERE source = $source_id")
            .bind(("source_id", source_id.to_owned()))
            .await
            .map_err(|e| VectorStoreError::Db(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_empty_store() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();

        // Create the chunk table so search query references an existing table
        db.query(
            "DEFINE TABLE chunk SCHEMAFULL;
             DEFINE FIELD embedding ON chunk TYPE array<float>;"
        )
        .await
        .unwrap();

        let store = SurrealDbVector::new(db);
        let results = store.search(&[0.0; 768], None, 10).await;

        // Should succeed and return empty results
        assert!(results.is_ok());
        assert!(results.unwrap().is_empty());
    }
}
