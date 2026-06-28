//! Shared test mocks and fixtures for extraction_service tests.

use crate::providers::llm_provider::{ChatMessage, LlmProvider};
use crate::providers::vector_store::{IndexedChunk, SearchResult, VectorStore, VectorStoreError};

pub struct MockLlm {
    pub response: String,
}

#[async_trait::async_trait]
impl LlmProvider for MockLlm {
    fn provider_type(&self) -> &'static str {
        "mock_extraction"
    }

    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<
        tokio::sync::mpsc::Receiver<Result<String, crate::providers::llm_provider::LlmError>>,
        crate::providers::llm_provider::LlmError,
    > {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let resp = self.response.clone();
        tokio::spawn(async move {
            let _ = tx.send(Ok(resp)).await;
        });
        Ok(rx)
    }
}

pub struct MockVectorStore {
    pub results: Vec<SearchResult>,
}

#[async_trait::async_trait]
impl VectorStore for MockVectorStore {
    async fn upsert(&self, _s: &str, _c: &[IndexedChunk]) -> Result<(), VectorStoreError> {
        Ok(())
    }
    async fn search(
        &self,
        _q: &[f32],
        _cids: &[String],
        _limit: u64,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        Ok(self.results.clone())
    }
    async fn delete_by_source(&self, _s: &str) -> Result<(), VectorStoreError> {
        Ok(())
    }
}

/// Returns `seed` for prompts that request relations and `profile` for profile prompts.
pub struct BranchingLlm {
    pub seed: String,
    pub profile: String,
}

#[async_trait::async_trait]
impl LlmProvider for BranchingLlm {
    fn provider_type(&self) -> &'static str {
        "mock_branching"
    }
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<
        tokio::sync::mpsc::Receiver<Result<String, crate::providers::llm_provider::LlmError>>,
        crate::providers::llm_provider::LlmError,
    > {
        let is_seed = messages
            .first()
            .map(|m| m.content.contains("\"relations\""))
            .unwrap_or(false);
        let resp = if is_seed {
            self.seed.clone()
        } else {
            self.profile.clone()
        };
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let _ = tx.send(Ok(resp)).await;
        });
        Ok(rx)
    }
}

pub async fn setup_db_with_collection() -> (surrealdb::Surreal<surrealdb::engine::local::Db>, String)
{
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();

    let mut resp = db
        .query(
            "CREATE collection SET name='PHB', description=NULL, \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let rows: Vec<Row> = resp.take(0).unwrap();
    let col_id = rows.into_iter().next().unwrap().id.id.to_raw();

    db.query(
        "CREATE source SET id='src1', filename='test.pdf', display_name='Test', \
         source_type='lore', page_count=1, indexed_at=time::now(), index_status='done', \
         embed_model='mock', collection=type::thing('collection',$cid)",
    )
    .bind(("cid", col_id.clone()))
    .await
    .unwrap();
    let zeros = std::iter::repeat_n("0.0", 768)
        .collect::<Vec<_>>()
        .join(",");
    db.query(format!(
        "CREATE chunk SET id='chunk1', \
         text='The Iron Fist controls the eastern docks. Commander Varn leads them.', \
         page_start=1, page_end=1, section_heading='Factions', source_type='lore', \
         source=type::thing('source','src1'), \
         collection=type::thing('collection',$cid), \
         embedding=[{zeros}], embed_model='mock'",
    ))
    .bind(("cid", col_id.clone()))
    .await
    .unwrap()
    .check()
    .unwrap();

    (db, col_id)
}

pub async fn link_campaign_to_collection(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    col_id: &str,
) {
    db.query(
        "CREATE campaign SET id='camp1', name='C', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "LET $in  = type::thing('campaign',   'camp1'); \
         LET $out = type::thing('collection', $cid); \
         RELATE $in->subscribes_to->$out SET created_at=time::now()",
    )
    .bind(("cid", col_id.to_owned()))
    .await
    .unwrap();
}
