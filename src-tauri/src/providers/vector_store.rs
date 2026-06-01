use async_trait::async_trait;

/// A single search result returned from the vector store.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk_id: String,
    pub source_id: String,
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

    /// Remove all chunks belonging to `source_id`.
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

/// Validate a collection ID before embedding it in a SurrealQL identifier.
///
/// Collection IDs originate from our own UUID generator (hex + hyphens), but
/// we validate defensively to prevent backtick-injection attacks in cases
/// where an unexpected value could reach the SQL builder.
///
/// Allowed characters: ASCII alphanumeric, `-`, `_`.
fn sanitize_collection_id(id: &str) -> Result<&str, VectorStoreError> {
    if id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        Ok(id)
    } else {
        Err(VectorStoreError::Db(format!(
            "Invalid collection ID (unexpected characters): {id}"
        )))
    }
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

            // Build the collection clause as a typed record link.
            // sanitize_collection_id rejects any ID that contains characters
            // outside [A-Za-z0-9\-_], guarding against backtick injection.
            let cid = sanitize_collection_id(&chunk.collection_id)?;
            let collection_clause = format!("collection = collection:`{cid}`");

            let sql = format!(
                "CREATE chunk SET
                    id = $id,
                    source = $source_id,
                    {collection_clause},
                    text = $text,
                    page_start = $page_start,
                    page_end = $page_end,
                    section_heading = $section_heading,
                    source_type = $source_type,
                    embedding = {embedding_field},
                    embed_model = $embed_model"
            );

            self.db
                .query(sql)
                .bind(("id", chunk.chunk_id.clone()))
                .bind(("source_id", source_id.to_owned()))
                .bind(("text", chunk.text.clone()))
                .bind(("page_start", chunk.page_start))
                .bind(("page_end", chunk.page_end))
                .bind(("section_heading", chunk.section_heading.clone()))
                .bind(("source_type", chunk.source_type.clone()))
                .bind(("embed_model", chunk.embed_model.clone()))
                .await
                .map_err(|e| VectorStoreError::Db(e.to_string()))?
                .check()
                .map_err(|e| VectorStoreError::Db(e.to_string()))?;
        }

        Ok(())
    }

    async fn search(
        &self,
        query_vector: &[f32],
        collection_ids: &[String],
        limit: u64,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        // Empty subscription list → nothing to search; return immediately
        // so we never hit the DB with a vacuous query.
        if collection_ids.is_empty() {
            return Ok(Vec::new());
        }

        let vec_str = query_vector
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",");

        // Build `collection IN [collection:`id1`, collection:`id2`, ...]`.
        // Using IN with a typed record-link list as a secondary WHERE condition
        // keeps the MTREE index path intact — OR at the top level would bypass
        // the vector index.
        // sanitize_collection_id validates each ID against backtick injection.
        let col_list = collection_ids
            .iter()
            .map(|id| sanitize_collection_id(id).map(|safe| format!("collection:`{safe}`")))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");

        // SurrealDB KNN pattern:
        //   WHERE embedding <|K|> $vec  — activates MTREE index / brute-force ANN
        //   SELECT vector::distance::knn() AS distance — retrieves the computed distance
        // The collection filter is ANDed into the same WHERE clause.
        // The vector literal must appear directly in the WHERE expression (not SELECT)
        // for the MTREE index to activate.
        let sql = format!(
            "SELECT
                id,
                source,
                text,
                page_start,
                page_end,
                section_heading,
                source_type,
                vector::distance::knn() AS distance
            FROM chunk
            WHERE embedding <|{limit}|> [{vec_str}]
            AND collection IN [{col_list}]
            ORDER BY distance ASC
            LIMIT {limit}"
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

    #[test]
    fn sanitize_collection_id_accepts_valid_ids() {
        assert!(sanitize_collection_id("abc123").is_ok());
        assert!(sanitize_collection_id("01234567-89ab-cdef-0123-456789abcdef").is_ok());
        assert!(sanitize_collection_id("col_1").is_ok());
    }

    #[test]
    fn sanitize_collection_id_rejects_backtick() {
        let result = sanitize_collection_id("col`DROP TABLE chunk");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid collection ID"));
    }

    #[test]
    fn sanitize_collection_id_rejects_other_special_chars() {
        assert!(sanitize_collection_id("col id").is_err()); // space
        assert!(sanitize_collection_id("col/id").is_err()); // slash
        assert!(sanitize_collection_id("col;DROP").is_err()); // semicolon
    }

    #[tokio::test]
    async fn test_search_empty_store() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();

        // Create the chunk table so search query references an existing table
        db.query(
            "DEFINE TABLE chunk SCHEMAFULL;
             DEFINE FIELD embedding ON chunk TYPE array<float>;",
        )
        .await
        .unwrap();

        let store = SurrealDbVector::new(db);
        // Empty collection_ids → immediate Ok(vec![]) without touching the DB
        let results = store.search(&[0.0; 768], &[], 10).await;

        // Should succeed and return empty results
        assert!(results.is_ok());
        assert!(results.unwrap().is_empty());
    }

    #[tokio::test]
    async fn search_only_returns_chunks_from_subscribed_collections() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        // Create two collections
        db.query(
            "CREATE collection SET id='col1', name='C1', created_at=time::now(), updated_at=time::now(); \
             CREATE collection SET id='col2', name='C2', created_at=time::now(), updated_at=time::now()"
        ).await.unwrap();

        // Create a source for each (source.collection is NON-NULLABLE)
        db.query(
            "CREATE source SET id='s1', collection=type::thing('collection','col1'), \
             filename='a.pdf', display_name='A', source_type='rules', page_count=1, \
             indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5'; \
             CREATE source SET id='s2', collection=type::thing('collection','col2'), \
             filename='b.pdf', display_name='B', source_type='rules', page_count=1, \
             indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5'",
        )
        .await
        .unwrap();

        // Use [1.0, 0.0, ..., 0.0] instead of all-zeros: cosine similarity is
        // undefined for zero vectors, causing SurrealDB to return `false` (no match).
        let ones_first = {
            let mut v = std::iter::repeat("0.0").take(768).collect::<Vec<_>>();
            v[0] = "1.0";
            v.join(",")
        };
        // SAFETY: {ones_first} is constructed from string literals — not user input —
        // so format! here is acceptable for test fixture data.
        db.query(format!(
            "CREATE chunk SET id='c1', source=type::thing('source','s1'), \
             collection=type::thing('collection','col1'), text='Alpha text', \
             page_start=1, page_end=1, section_heading='', source_type='rules', \
             embedding=[{ones_first}], embed_model='nomic-embed-text-v1.5'; \
             CREATE chunk SET id='c2', source=type::thing('source','s2'), \
             collection=type::thing('collection','col2'), text='Beta text', \
             page_start=1, page_end=1, section_heading='', source_type='rules', \
             embedding=[{ones_first}], embed_model='nomic-embed-text-v1.5'"
        ))
        .await
        .unwrap();

        let store = SurrealDbVector::new(db);
        // Match the stored embedding: [1.0, 0.0, ..., 0.0] for valid cosine similarity.
        let mut query = vec![0.0f32; 768];
        query[0] = 1.0;

        // Only subscribed to col1 — should only get Alpha
        let results = store
            .search(&query, &["col1".to_string()], 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Alpha text");

        // Subscribed to both — gets both
        let results = store
            .search(&query, &["col1".to_string(), "col2".to_string()], 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        // No subscriptions — empty result
        let results = store.search(&query, &[], 10).await.unwrap();
        assert!(results.is_empty());
    }
}
