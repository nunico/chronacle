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

    async fn search(
        &self,
        query_vector: &[f32],
        campaign_id: Option<&str>,
        limit: u64,
    ) -> Result<Vec<SearchResult>, VectorStoreError>;

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
            let vec_str = chunk
                .embedding
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let embedding_field = format!("[{}]", vec_str);

            // Omit campaign field for global chunks (None branch). The DEFINE FIELD
            // uses option<record<campaign>> DEFAULT NONE, so omitting the field
            // produces a valid NONE value without triggering type validation errors.
            let sql = match &chunk.campaign_id {
                Some(cid) => format!(
                    "CREATE chunk SET
                        id = $id,
                        source = type::thing('source', $source_id),
                        campaign = campaign:`{cid}`,
                        text = $text,
                        page_start = $page_start,
                        page_end = $page_end,
                        section_heading = $section_heading,
                        source_type = $source_type,
                        embedding = {},
                        embed_model = $embed_model",
                    embedding_field,
                ),
                None => format!(
                    "CREATE chunk SET
                        id = $id,
                        source = type::thing('source', $source_id),
                        text = $text,
                        page_start = $page_start,
                        page_end = $page_end,
                        section_heading = $section_heading,
                        source_type = $source_type,
                        embedding = {},
                        embed_model = $embed_model",
                    embedding_field,
                ),
            };

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
                .map_err(|e| VectorStoreError::Db(format!("Chunk upsert error: {e}")))?;
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

        // Canonical SurrealDB KNN pattern:
        //   - `embedding <|K|> $vec` in WHERE uses the MTREE index to filter
        //     to the K nearest neighbours (metric inherited from the index —
        //     ours is DIST COSINE).
        //   - `vector::distance::knn()` in SELECT retrieves the distance the
        //     KNN scan computed for the current row.
        //
        // The previous form (`embedding <|1|> [vec] AS distance` in SELECT)
        // returned a boolean, which deserialize_distance silently coerced to
        // f64::MAX — every row tied, ORDER BY did nothing, results came back
        // in storage order. Retrieval was effectively random.
        //
        // We over-fetch (K = limit * 5, min 50) so the campaign filter has
        // candidates to narrow from without leaving us short of `limit`.
        let knn_k = std::cmp::max(limit * 5, 50);

        // Don't combine an `OR` predicate with the KNN operator — the MTREE
        // optimizer can't push it down and returns zero rows. Since the
        // schema (post 002_fix_chunk_campaign_type) defines `campaign` as
        // `option<record<campaign>> DEFAULT NONE`, IS NULL is impossible, so
        // we only need IS NONE.
        let campaign_clause = match campaign_id {
            Some(cid) => format!(
                " AND (campaign = campaign:`{cid}` OR campaign IS NONE)"
            ),
            None => " AND campaign IS NONE".to_string(),
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
                vector::distance::knn() AS distance
            FROM chunk
            WHERE embedding <|{knn_k}|> [{vec_str}]{campaign_clause}
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
            source_name: Option<String>,
            text: String,
            page_start: i64,
            page_end: i64,
            section_heading: String,
            source_type: String,
            #[serde(deserialize_with = "deserialize_distance")]
            distance: f64,
        }

        fn deserialize_distance<'de, D>(deserializer: D) -> Result<f64, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            // Accept numbers (the happy path) and numeric strings (some
            // SurrealDB versions return distance as a string). REJECT bool —
            // a boolean here means the KNN expression was used incorrectly
            // (`embedding <|K|> $vec` in SELECT returns membership, not
            // distance); silently coercing it to f64::MAX hides the bug and
            // makes retrieval return random rows. See the inline comment on
            // the search query above.
            #[derive(serde::Deserialize)]
            #[serde(untagged)]
            enum Num {
                F64(f64),
                I64(i64),
                String(String),
            }
            match serde::Deserialize::deserialize(deserializer)? {
                Num::F64(v) => Ok(v),
                Num::I64(v) => Ok(v as f64),
                Num::String(s) => s.parse::<f64>().map_err(serde::de::Error::custom),
            }
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
            .query("DELETE chunk WHERE source = type::thing('source', $source_id)")
            .bind(("source_id", source_id.to_owned()))
            .await
            .map_err(|e| VectorStoreError::Db(e.to_string()))?
            .check()
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

        db.query(
            "DEFINE TABLE chunk SCHEMAFULL;
             DEFINE FIELD embedding ON chunk TYPE array<float>;",
        )
        .await
        .unwrap();

        let store = SurrealDbVector::new(db);
        let results = store.search(&[0.0; 768], None, 10).await;

        assert!(results.is_ok());
        assert!(results.unwrap().is_empty());
    }
}
