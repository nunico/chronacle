use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::codex_service::rules::compile_rules_with_cap;
use crate::codex_service::{compile_rules, list_rule_entries, redo_rule_entry, update_rule_notes};
use crate::extraction_service::test_support::{
    setup_db_with_collection, MockEmbeddingProvider, MockLlm,
};
use async_trait::async_trait;
use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::{ChatMessage, LlmError, LlmProvider};

/// Proves "nothing to compile → no LLM cost": panics if ever invoked.
struct PanickingLlm;

#[async_trait]
impl LlmProvider for PanickingLlm {
    fn provider_type(&self) -> &'static str {
        "panicking"
    }

    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        panic!("compile_rules must not call the LLM when there is no rules/supplement content");
    }
}

/// Seed a `rules`-typed source + chunk in `col_id` with the given text.
///
/// Mirrors production ingestion's write shape exactly: `chunk.source_type` is
/// left as an empty string (see `chronacle-ingestion`'s pipeline, which never
/// populates it) — the reliable signal is `source.source_type`, which the
/// upload flow defaults to `"rules"` and the schema `ASSERT`s. Rules-compile
/// queries must filter via the `chunk.source` link, not `chunk.source_type`.
async fn seed_rules_chunk(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    col_id: &str,
    chunk_id: &str,
    source_id: &str,
    text: &str,
    page_start: i64,
    page_end: i64,
) {
    let zeros = std::iter::repeat_n("0.0", 768)
        .collect::<Vec<_>>()
        .join(",");
    db.query(format!(
        "CREATE source SET id='{source_id}', filename='rules.pdf', display_name='Core Rules', \
             source_type='rules', page_count=10, indexed_at=time::now(), index_status='done', \
             embed_model='mock', collection=type::thing('collection',$cid);
         CREATE chunk SET id='{chunk_id}', text=$text, page_start=$ps, page_end=$pe, \
             section_heading='Combat', source_type='', \
             source=type::thing('source','{source_id}'), \
             collection=type::thing('collection',$cid), \
             embedding=[{zeros}], embed_model='mock';"
    ))
    .bind(("cid", col_id.to_string()))
    .bind(("text", text.to_string()))
    .bind(("ps", page_start))
    .bind(("pe", page_end))
    .await
    .unwrap()
    .check()
    .unwrap();
}

#[tokio::test]
async fn rules_compile_creates_entries_with_categories_and_page_refs() {
    let (db, col_id) = setup_db_with_collection().await;
    seed_rules_chunk(
        &db,
        &col_id,
        "chunk_rules1",
        "src_rules1",
        "Initiative is rolled at the start of combat by every combatant.",
        10,
        11,
    )
    .await;

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entries":[{"name":"Initiative","category":"mechanic",
            "body":"Roll initiative at the start of combat.",
            "page_refs":[{"source_name":"Core Rules","page_start":10,"page_end":11}]}]}"#
            .into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    let res = compile_rules(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();
    assert_eq!(res.entries_created, 1);
    assert_eq!(res.entries_updated, 0);
    assert_eq!(res.remaining_batches, 0);

    #[derive(serde::Deserialize)]
    struct Row {
        category: String,
        body: String,
        stale: bool,
        embedding: Option<Vec<f32>>,
        page_refs: Vec<serde_json::Value>,
    }
    let mut resp = db
        .query(
            "SELECT category, body, stale, embedding, page_refs FROM rule_entry \
             WHERE collection = type::thing('collection', $cid) AND name = 'Initiative'",
        )
        .bind(("cid", col_id.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    let row = rows.first().expect("rule_entry row must exist");
    assert_eq!(row.category, "mechanic");
    assert!(row.body.contains("initiative") || row.body.contains("Initiative"));
    assert!(!row.stale);
    assert!(
        row.embedding.as_ref().is_some_and(|v| !v.is_empty()),
        "embedding must be present"
    );
    assert_eq!(row.page_refs[0]["page_start"], 10);
}

#[tokio::test]
async fn rules_compile_skips_lore_only_sources() {
    // setup_db_with_collection seeds only a 'lore' chunk — no rules/supplement content.
    let (db, col_id) = setup_db_with_collection().await;

    let llm: Arc<dyn LlmProvider> = Arc::new(PanickingLlm);
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    let res = compile_rules(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();
    assert_eq!(res.entries_created, 0);
    assert_eq!(res.entries_updated, 0);
}

#[tokio::test]
async fn rules_recompile_merges_by_name_preserving_notes() {
    let (db, col_id) = setup_db_with_collection().await;
    seed_rules_chunk(
        &db,
        &col_id,
        "chunk_rules2",
        "src_rules2",
        "Armor class determines how hard a creature is to hit.",
        20,
        20,
    )
    .await;

    db.query(
        "CREATE rule_entry SET collection = type::thing('collection', $cid), \
             name = 'Armor Class', category = 'statistic', body = 'old body', \
             notes = 'table ruling', page_refs = [], sources = [], \
             compiled_at = time::now(), stale = false",
    )
    .bind(("cid", col_id.clone()))
    .await
    .unwrap()
    .check()
    .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entries":[{"name":"Armor Class","category":"statistic",
            "body":"new body: armor class is a defensive statistic.",
            "page_refs":[{"source_name":"Core Rules","page_start":20,"page_end":20}]}]}"#
            .into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    let res = compile_rules(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();
    assert_eq!(res.entries_created, 0);
    assert_eq!(res.entries_updated, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        body: String,
        notes: Option<String>,
    }
    let mut resp = db
        .query(
            "SELECT body, notes FROM rule_entry \
             WHERE collection = type::thing('collection', $cid) AND name = 'Armor Class'",
        )
        .bind(("cid", col_id.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1, "must not create a duplicate row");
    assert!(rows[0].body.contains("new body"));
    assert_eq!(rows[0].notes.as_deref(), Some("table ruling"));
}

#[tokio::test]
async fn invalid_llm_category_falls_back_to_entry() {
    let (db, col_id) = setup_db_with_collection().await;
    seed_rules_chunk(
        &db,
        &col_id,
        "chunk_rules3",
        "src_rules3",
        "The party may take a moment to vibe before the next encounter.",
        30,
        30,
    )
    .await;

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entries":[{"name":"Vibe Check","category":"vibe",
            "body":"Something freeform.",
            "page_refs":[{"source_name":"Core Rules","page_start":30,"page_end":30}]}]}"#
            .into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    let _ = compile_rules(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct Row {
        category: String,
    }
    let mut resp = db
        .query(
            "SELECT category FROM rule_entry \
             WHERE collection = type::thing('collection', $cid) AND name = 'Vibe Check'",
        )
        .bind(("cid", col_id.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.first().unwrap().category, "entry");
}

#[tokio::test]
async fn redo_rule_entry_stores_objection_and_regenerates() {
    let (db, col_id) = setup_db_with_collection().await;
    seed_rules_chunk(
        &db,
        &col_id,
        "chunk_rules4",
        "src_rules4",
        "Range increments determine ranged attack penalties.",
        40,
        40,
    )
    .await;

    let mut resp = db
        .query(
            "CREATE rule_entry SET collection = type::thing('collection', $cid), \
                 name = 'Range Increments', category = 'mechanic', body = 'old range text', \
                 notes = 'house rule applies', page_refs = [], sources = [], \
                 compiled_at = time::now(), stale = false RETURN VALUE id",
        )
        .bind(("cid", col_id.clone()))
        .await
        .unwrap();
    let ids: Vec<surrealdb::sql::Thing> = resp.take(0).unwrap();
    let entry_id = ids.first().unwrap().id.to_raw();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entries":[{"name":"Range Increments","category":"mechanic",
            "body":"regenerated range text honoring the objection",
            "page_refs":[{"source_name":"Core Rules","page_start":40,"page_end":40}]}]}"#
            .into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    redo_rule_entry(&db, &llm, &embed, &entry_id, "the range is wrong")
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct Row {
        body: String,
        notes: Option<String>,
        sources: Vec<serde_json::Value>,
    }
    let mut resp = db
        .query("SELECT body, notes, sources FROM type::thing('rule_entry', $id)")
        .bind(("id", entry_id.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    let row = rows.first().unwrap();
    assert!(row.body.contains("regenerated"));
    assert_eq!(row.notes.as_deref(), Some("house rule applies"));
    assert!(row
        .sources
        .iter()
        .any(|s| s["kind"] == "objection" && s["text"] == "the range is wrong"));
}

#[tokio::test]
async fn redo_rule_entry_merges_page_refs_without_clobbering() {
    let (db, col_id) = setup_db_with_collection().await;
    seed_rules_chunk(
        &db,
        &col_id,
        "chunk_rules_redo_pr",
        "src_rules_redo_pr",
        "Range increments determine ranged attack penalties.",
        40,
        40,
    )
    .await;

    let mut resp = db
        .query(
            "CREATE rule_entry SET collection = type::thing('collection', $cid), \
                 name = 'Range Increments Redo', category = 'mechanic', body = 'old range text', \
                 notes = NONE, \
                 page_refs = [{ source_name: 'Core Rules', page_start: 40, page_end: 40 }], \
                 sources = [], compiled_at = time::now(), stale = false RETURN VALUE id",
        )
        .bind(("cid", col_id.clone()))
        .await
        .unwrap();
    let ids: Vec<surrealdb::sql::Thing> = resp.take(0).unwrap();
    let entry_id = ids.first().unwrap().id.to_raw();

    // LLM's redo response returns no page_refs at all.
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entries":[{"name":"Range Increments Redo","category":"mechanic",
            "body":"regenerated range text honoring the objection","page_refs":[]}]}"#
            .into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    redo_rule_entry(&db, &llm, &embed, &entry_id, "the range is wrong")
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct Row {
        body: String,
        page_refs: Vec<serde_json::Value>,
    }
    let mut resp = db
        .query("SELECT body, page_refs FROM type::thing('rule_entry', $id)")
        .bind(("id", entry_id.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    let row = rows.first().unwrap();
    assert!(row.body.contains("regenerated"));
    assert_eq!(
        row.page_refs.len(),
        1,
        "pre-existing page_refs must survive a redo whose LLM response has none, got {:?}",
        row.page_refs
    );
    assert_eq!(row.page_refs[0]["page_start"], 40);
}

#[tokio::test]
async fn rules_recompile_preserves_stored_objections_in_sources() {
    let (db, col_id) = setup_db_with_collection().await;
    seed_rules_chunk(
        &db,
        &col_id,
        "chunk_rules5",
        "src_rules5",
        "Grappling requires a contested strength check against the target.",
        50,
        50,
    )
    .await;

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entries":[{"name":"Grappling","category":"mechanic",
            "body":"A contested strength check resolves a grapple attempt.",
            "page_refs":[{"source_name":"Core Rules","page_start":50,"page_end":50}]}]}"#
            .into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    // First compile: creates the entry.
    let res = compile_rules(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();
    assert_eq!(res.entries_created, 1);

    #[derive(serde::Deserialize)]
    struct IdRow {
        id: surrealdb::sql::Thing,
    }
    let mut resp = db
        .query(
            "SELECT id FROM rule_entry \
             WHERE collection = type::thing('collection', $cid) AND name = 'Grappling'",
        )
        .bind(("cid", col_id.clone()))
        .await
        .unwrap();
    let rows: Vec<IdRow> = resp.take(0).unwrap();
    let entry_id = rows.first().unwrap().id.id.to_raw();

    // Inject a GM objection directly, mirroring what `redo_rule_entry` stores.
    db.query(
        "UPDATE type::thing('rule_entry', $id) SET \
             sources = array::append(sources, { kind: 'objection', \
                 text: 'this ignores size differences', at: time::now() })",
    )
    .bind(("id", entry_id.clone()))
    .await
    .unwrap()
    .check()
    .unwrap();

    // Recompile with the same rule (same name+category) re-emitted by the LLM.
    let res = compile_rules(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();
    assert_eq!(res.entries_created, 0, "must update, not duplicate");
    assert_eq!(res.entries_updated, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        sources: Vec<serde_json::Value>,
    }
    let mut resp = db
        .query(
            "SELECT sources FROM rule_entry \
             WHERE collection = type::thing('collection', $cid) AND name = 'Grappling'",
        )
        .bind(("cid", col_id.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1, "recompile must not create a duplicate entry");
    assert!(
        rows[0]
            .sources
            .iter()
            .any(|s| s["kind"] == "objection" && s["text"] == "this ignores size differences"),
        "the dedup-merge UPDATE must not clobber sources, sources = {:?}",
        rows[0].sources
    );
}

/// Counts how many times the LLM is invoked, to prove the batch cap is
/// honored (only `cap` batches ever reach the LLM).
struct CountingLlm {
    response: String,
    calls: AtomicUsize,
}

#[async_trait]
impl LlmProvider for CountingLlm {
    fn provider_type(&self) -> &'static str {
        "counting"
    }

    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let resp = self.response.clone();
        tokio::spawn(async move {
            let _ = tx.send(Ok(resp)).await;
        });
        Ok(rx)
    }
}

#[tokio::test]
async fn rules_compile_honors_batch_cap_and_reports_honest_remainder() {
    let (db, col_id) = setup_db_with_collection().await;
    // Two chunks whose combined labeled text exceeds BATCH_CHAR_BUDGET
    // (16_000 chars), forcing `batch_labeled_chunks` to split them into two
    // distinct batches: a large one, then a second.
    let big_text = "a".repeat(17_000);
    seed_rules_chunk(
        &db,
        &col_id,
        "chunk_rules_cap1",
        "src_rules_cap1",
        &big_text,
        60,
        60,
    )
    .await;
    seed_rules_chunk(
        &db,
        &col_id,
        "chunk_rules_cap2",
        "src_rules_cap2",
        "A short second passage that lands in its own batch.",
        61,
        61,
    )
    .await;

    let llm = Arc::new(CountingLlm {
        response: r#"{"entries":[{"name":"Whatever","category":"entry",
            "body":"filler body","page_refs":[]}]}"#
            .into(),
        calls: AtomicUsize::new(0),
    });
    let llm_dyn: Arc<dyn LlmProvider> = llm.clone();
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    let res = compile_rules_with_cap(&db, &llm_dyn, &embed, &col_id, 1, |_| {})
        .await
        .unwrap();

    assert_eq!(
        llm.calls.load(Ordering::SeqCst),
        1,
        "only `cap` batches may reach the LLM"
    );
    assert_eq!(
        res.remaining_batches, 1,
        "the leftover batch count must be honest, not zero or clamped"
    );
}

#[tokio::test]
async fn list_and_update_rule_notes_round_trip() {
    let (db, col_id) = setup_db_with_collection().await;
    db.query(
        "CREATE rule_entry SET collection = type::thing('collection', $cid), \
             name = 'Grappling', category = 'mechanic', body = 'grapple rules', \
             notes = NULL, page_refs = [], sources = [], \
             compiled_at = time::now(), stale = false RETURN VALUE id",
    )
    .bind(("cid", col_id.clone()))
    .await
    .unwrap();

    let entries = list_rule_entries(&db, &col_id).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "Grappling");
    assert!(entries[0].notes.is_none());

    update_rule_notes(&db, &entries[0].id, Some("clarify escape DC".to_string()))
        .await
        .unwrap();
    let entries = list_rule_entries(&db, &col_id).await.unwrap();
    assert_eq!(entries[0].notes.as_deref(), Some("clarify escape DC"));
}
