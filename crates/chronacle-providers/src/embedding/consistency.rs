/// Report of sources indexed with a different embedding model than the active one.
///
/// Returned by [`check_embedding_model_consistency`]. The `stale_models` field
/// lists the distinct `embed_model` values found in the `source` table that
/// disagree with the active embedding provider's model ID, along with the count
/// of affected sources per model.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbeddingModelMismatch {
    /// The model ID currently active in the embedding provider.
    pub active_model: String,
    /// Per stale model: how many sources are indexed with it.
    pub stale: Vec<StaleModelCount>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StaleModelCount {
    pub embed_model: String,
    pub source_count: u64,
}

impl EmbeddingModelMismatch {
    pub fn is_clean(&self) -> bool {
        self.stale.is_empty()
    }

    pub fn total_stale_sources(&self) -> u64 {
        self.stale.iter().map(|s| s.source_count).sum()
    }
}

/// Check whether any indexed sources were embedded with a different model than
/// the active embedding provider.
///
/// Returns the report describing affected sources. An empty `stale` list means
/// every indexed source matches the active model (or there are no sources yet).
///
/// See ADR-003: silently changing embedding models corrupts retrieval because
/// query vectors and indexed vectors live in different latent spaces.
pub async fn check_embedding_model_consistency<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    active_model: &str,
) -> Result<EmbeddingModelMismatch, Box<surrealdb::Error>> {
    #[derive(serde::Deserialize)]
    struct Row {
        embed_model: Option<String>,
    }
    let mut response = db.query("SELECT embed_model FROM source").await?.check()?;
    let rows: Vec<Row> = response.take(0)?;

    let mut counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for row in rows {
        let model = match row.embed_model {
            Some(m) if !m.is_empty() => m,
            _ => continue,
        };
        if model == active_model {
            continue;
        }
        *counts.entry(model).or_insert(0) += 1;
    }

    let mut stale: Vec<StaleModelCount> = counts
        .into_iter()
        .map(|(embed_model, source_count)| StaleModelCount {
            embed_model,
            source_count,
        })
        .collect();
    stale.sort_by(|a, b| a.embed_model.cmp(&b.embed_model));

    Ok(EmbeddingModelMismatch {
        active_model: active_model.to_string(),
        stale,
    })
}
