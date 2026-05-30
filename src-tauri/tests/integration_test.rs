/// Integration tests for the Chronacle backend.
///
/// These tests exercise the service layer directly against an in-memory
/// SurrealDB instance. They do **not** go through Tauri IPC — that is
/// covered by the E2E test suite.
use chronacle_lib::schema;
use std::sync::{Arc, RwLock};

/// Helper: set up an in-memory SurrealDB with the Phase 1 schema applied.
async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .expect("Failed to create in-memory SurrealDB");

    db.use_ns("test").use_db("test").await.unwrap();

    // Run the full Phase 1 schema
    schema::run_migrations(&db)
        .await
        .expect("Schema migration should succeed");

    db
}

#[tokio::test]
async fn test_schema_migration_creates_tables() {
    let db = setup_db().await;

    // Verify the schema was applied by checking the DB works
    let mut res = db
        .query("SELECT count() FROM campaign GROUP ALL")
        .await
        .unwrap();

    let count: Vec<i64> = res.take(0).unwrap_or_default();
    assert!(count.is_empty() || count == vec![0]);
}

#[tokio::test]
async fn test_redefine_chunk_campaign_field() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();

    // Step 1: Create chunk table with the ORIGINAL scaffold schema (record<campaign> WITHOUT | NULL)
    db.query(
        "DEFINE TABLE chunk SCHEMAFULL;
         DEFINE FIELD source ON chunk TYPE record<source>;
         DEFINE FIELD campaign ON chunk TYPE record<campaign>;
         DEFINE FIELD text ON chunk TYPE string;
         DEFINE FIELD embedding ON chunk TYPE array<float>;"
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    // Step 2: Try redefining with | NULL DEFAULT NULL (what 001_initial currently has)
    let res = db
        .query("DEFINE FIELD campaign ON chunk TYPE record<campaign> | NULL DEFAULT NULL;")
        .await
        .unwrap()
        .check();
    eprintln!("Redefine with | NULL: {:?}", res);

    // Step 3: Try redefining with option<record<campaign>>
    let res2 = db
        .query("DEFINE FIELD campaign ON chunk TYPE option<record<campaign>> DEFAULT NONE;")
        .await
        .unwrap()
        .check();
    eprintln!("Redefine with option<>: {:?}", res2);

    // Step 4: Try REMOVE + DEFINE approach
    db.query(
        "REMOVE FIELD campaign ON chunk;
         DEFINE FIELD campaign ON chunk TYPE option<record<campaign>> DEFAULT NONE;"
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    eprintln!("REMOVE + DEFINE succeeded");

    // Step 5: Verify it works by inserting without campaign
    let res3 = db
        .query("CREATE chunk SET id = 'test', source = type::thing('source', 's1'), text = 'hello', embedding = [0.1, 0.2];")
        .await
        .unwrap()
        .check();
    assert!(
        res3.is_ok(),
        "After REMOVE+DEFINE, omitted campaign should work: {:?}",
        res3
    );
    eprintln!("Create with omitted campaign succeeded");
}

#[tokio::test]
async fn test_campaign_crud() {
    let db = setup_db().await;

    // Create a campaign
    let mut res = db
        .query(
            "CREATE campaign:test1 SET
                name = 'Test Campaign',
                system = 'D&D 5e',
                created_at = time::now(),
                updated_at = time::now()",
        )
        .await
        .unwrap();

    // Take result to verify it worked
    #[derive(serde::Deserialize)]
    #[expect(dead_code)]
    struct CampaignCreated {
        id: surrealdb::sql::Thing,
    }
    let created: Vec<CampaignCreated> = res.take(0).unwrap();
    assert_eq!(created.len(), 1);

    // Verify it exists
    let mut res = db
        .query("SELECT * FROM campaign WHERE id = campaign:test1")
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    #[expect(dead_code)]
    struct CampaignRow {
        id: surrealdb::sql::Thing,
        name: String,
        system: String,
    }

    let rows: Vec<CampaignRow> = res.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Test Campaign");
}

#[tokio::test]
async fn test_source_crud() {
    let db = setup_db().await;

    let mut res = db
        .query(
            "CREATE source:src1 SET
                campaign = NULL,
                filename = 'test.pdf',
                display_name = 'Test PDF',
                source_type = 'rules',
                page_count = 10,
                indexed_at = time::now(),
                index_status = 'pending',
                embed_model = 'nomic-embed-text-v1.5'",
        )
        .await
        .unwrap();

    // Take result to verify
    #[derive(serde::Deserialize)]
    #[expect(dead_code)]
    struct SourceCreated {
        id: surrealdb::sql::Thing,
    }
    let created: Vec<SourceCreated> = res.take(0).unwrap();
    assert_eq!(created.len(), 1);

    // Update status
    db.query("UPDATE source:src1 SET index_status = 'done', campaign = NULL")
        .await
        .unwrap();
}

/// Generate a tiny valid PDF with known text content using `lopdf`.
fn create_test_pdf() -> Vec<u8> {
    use lopdf::*;

    let mut doc = Document::new();

    let mut font_dict = Dictionary::new();
    font_dict.set(b"Type", Object::Name(b"Font".to_vec()));
    font_dict.set(b"Subtype", Object::Name(b"Type1".to_vec()));
    font_dict.set(b"BaseFont", Object::Name(b"Helvetica".to_vec()));
    let font_id = doc.add_object(font_dict);

    let mut font_ref = Dictionary::new();
    font_ref.set(b"F1", Object::Reference(font_id));
    let mut resources_dict = Dictionary::new();
    resources_dict.set(b"Font", Object::Dictionary(font_ref));
    let resources_id = doc.add_object(resources_dict);

    let content_text = b"BT /F1 12 Tf 100 700 Td (Combat rules for the fighter class.) Tj ET";
    let mut stream_dict = Dictionary::new();
    stream_dict.set(b"Length", Object::Integer(content_text.len() as i64));
    let content_id = doc.add_object(Stream::new(stream_dict, content_text.to_vec()));

    let pages_id = doc.new_object_id();

    let mut page_dict = Dictionary::new();
    page_dict.set(b"Type", Object::Name(b"Page".to_vec()));
    page_dict.set(b"Parent", Object::Reference(pages_id));
    page_dict.set(
        b"MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ]),
    );
    page_dict.set(b"Contents", Object::Reference(content_id));
    page_dict.set(b"Resources", Object::Reference(resources_id));
    let page_id = doc.add_object(page_dict);

    let mut pages_dict = Dictionary::new();
    pages_dict.set(b"Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set(b"Kids", Object::Array(vec![Object::Reference(page_id)]));
    pages_dict.set(b"Count", Object::Integer(1));
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let mut catalog_dict = Dictionary::new();
    catalog_dict.set(b"Type", Object::Name(b"Catalog".to_vec()));
    catalog_dict.set(b"Pages", Object::Reference(pages_id));
    let catalog_id = doc.add_object(catalog_dict);
    doc.trailer.set("Root", Object::Reference(catalog_id));

    doc.compress();
    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

#[tokio::test]
async fn test_pdf_extract_text() {
    let pdf_data = create_test_pdf();

    let extracted = chronacle_lib::services::ingestion_service::extract_text(&pdf_data)
        .await
        .expect("extract_text should succeed");

    assert!(extracted.page_count >= 1, "should have at least 1 page");
    assert!(
        extracted.text.contains("Combat"),
        "extracted text should contain 'Combat': got '{}'",
        extracted.text
    );
    assert!(
        extracted.text.contains("fighter"),
        "extracted text should contain 'fighter': got '{}'",
        extracted.text
    );
    assert!(!extracted.pages.is_empty(), "should have page entries");
    assert_eq!(
        extracted.pages[0].page_num, 1,
        "first page should be page 1"
    );
}

#[tokio::test]
async fn test_full_ingest_and_query_cycle() {
    use chronacle_lib::providers::blob_store::BlobStore;
    use chronacle_lib::providers::embedding::{EmbeddingProvider, MockEmbeddingProvider};
    use chronacle_lib::providers::llm_provider::NoopProvider;
    use chronacle_lib::providers::vector_store::SurrealDbVector;
    use chronacle_lib::services::ingestion_service;

    let temp_dir = tempfile::tempdir().expect("tempdir should succeed");

    // Set up a real RocksDB so AppState type matches
    let db_path = temp_dir.path().join("test.db");
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::RocksDb>(db_path)
        .await
        .expect("Failed to create RocksDB");
    db.use_ns("test").use_db("test").await.unwrap();
    schema::run_migrations(&db)
        .await
        .expect("Migration should succeed");

    let pdfs_dir = temp_dir.path().join("pdfs");
    tokio::fs::create_dir_all(&pdfs_dir)
        .await
        .expect("create pdfs dir");

    let blob_store: Arc<dyn BlobStore> = Arc::new(
        chronacle_lib::providers::blob_store::LocalFileStore::new(pdfs_dir),
    );

    let vector_store: Arc<dyn chronacle_lib::providers::vector_store::VectorStore> =
        Arc::new(SurrealDbVector::new(db.clone()));

    let embedding_provider: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    let llm_provider = Arc::new(NoopProvider);

    let state = Arc::new(chronacle_lib::AppState {
        db: db.clone(),
        llm_provider: RwLock::new(
            llm_provider as Arc<dyn chronacle_lib::providers::llm_provider::LlmProvider>,
        ),
        vector_store: vector_store.clone(),
        blob_store: blob_store.clone(),
        embedding_provider: RwLock::new(embedding_provider.clone()),
    });

    // Create source record using the same pattern as the real upload_source command
    let source_id = "ingest-test-source";
    let filename = "test.pdf";
    let display_name = "Test Rules PDF";
    let embed_model = "nomic-embed-text-v1.5";

    let mut create_res = db
        .query(
            "CREATE source SET
                id = $id,
                campaign = NULL,
                filename = $filename,
                display_name = $display_name,
                source_type = 'rules',
                page_count = 0,
                indexed_at = time::now(),
                index_status = 'pending',
                embed_model = $embed_model",
        )
        .bind(("id", source_id.to_owned()))
        .bind(("filename", filename.to_owned()))
        .bind(("display_name", display_name.to_owned()))
        .bind(("embed_model", embed_model.to_owned()))
        .await
        .expect("create source should succeed");

    #[derive(serde::Deserialize)]
    struct CreatedId {
        id: surrealdb::sql::Thing,
    }
    let created: Vec<CreatedId> = create_res.take(0).expect("parse created source");
    assert_eq!(created.len(), 1, "source should be created");
    eprintln!(
        "Created source tb={:?} id={:?}",
        created[0].id.tb, created[0].id.id
    );

    // Store PDF blob
    let pdf_data = create_test_pdf();
    blob_store
        .store(source_id, filename, &pdf_data)
        .await
        .expect("blob store should succeed");

    // Debug: test extract_text directly
    let extracted = chronacle_lib::services::ingestion_service::extract_text(&pdf_data)
        .await
        .expect("extract_text should succeed");
    eprintln!(
        "Extracted {} pages, text length: {}",
        extracted.page_count,
        extracted.text.len()
    );
    eprintln!(
        "Extracted text: {}",
        &extracted.text[..std::cmp::min(200, extracted.text.len())]
    );

    // Run ingestion — this calls get_source_filename which queries WHERE id = $id
    // with the same source_id string. SurrealDB coerces the string to match the
    // record's Thing id when queried via bind parameters.
    ingestion_service::ingest_source(&state, source_id, std::sync::Arc::new(|_| {}))
        .await
        .expect("ingestion should succeed");

    // Debug: check chunk table schema + any records
    let mut debug2 = db
        .query("SELECT * FROM chunk LIMIT 5")
        .await
        .expect("debug query 2");
    #[derive(serde::Deserialize, Debug)]
    #[expect(dead_code)]
    struct ChunkRow {
        id: surrealdb::sql::Thing,
        text: String,
    }
    let chunk_rows: Vec<ChunkRow> = debug2.take(0).expect("parse chunks");
    eprintln!("Chunk rows: {:?}", chunk_rows);

    // Debug: check if any chunks exist at all
    let mut debug = db
        .query("SELECT count() FROM chunk GROUP ALL")
        .await
        .expect("debug query");
    #[derive(serde::Deserialize)]
    struct Cc {
        count: i64,
    }
    let all: Vec<Cc> = debug.take(0).expect("parse");
    eprintln!(
        "Total chunks in DB: {}",
        all.first().map(|c| c.count).unwrap_or(-1)
    );

    // Verify source status is 'done'
    let mut res = db
        .query("SELECT index_status, page_count FROM source WHERE id = type::thing('source', $id)")
        .bind(("id", source_id.to_owned()))
        .await
        .expect("query source should succeed");

    #[derive(serde::Deserialize)]
    struct SourceStatus {
        index_status: String,
        page_count: i64,
    }

    let statuses: Vec<SourceStatus> = res.take(0).expect("parse source status");
    assert_eq!(statuses.len(), 1, "source should exist");
    assert_eq!(statuses[0].index_status, "done", "source should be indexed");
    assert!(
        statuses[0].page_count >= 1,
        "should have page count >= 1, got {}",
        statuses[0].page_count
    );

    // Verify chunks exist using type::thing for proper record link
    let mut res = db
        .query(
            "SELECT count() FROM chunk WHERE source = type::thing('source', $source_id) GROUP ALL",
        )
        .bind(("source_id", source_id.to_owned()))
        .await
        .expect("query chunks should succeed");

    #[derive(serde::Deserialize)]
    struct ChunkCount {
        count: i64,
    }

    let counts: Vec<ChunkCount> = res.take(0).expect("parse chunk count");
    assert!(
        counts[0].count > 0,
        "should have at least one chunk, got {}",
        counts[0].count
    );

    // Search vector store for the known term
    let query_vec = embedding_provider
        .embed_query("Combat rules fighter")
        .await
        .expect("embed query should succeed");

    let results = vector_store
        .search(&query_vec, None, 5)
        .await
        .expect("vector search should succeed");

    assert!(!results.is_empty(), "vector search should return results");
    assert!(
        results.iter().any(|r| r.text.contains("Combat")),
        "at least one result should contain 'Combat'"
    );
    assert!(
        results.iter().any(|r| r.text.contains("fighter")),
        "at least one result should contain 'fighter'"
    );
}

#[tokio::test]
async fn test_custom_provider_crud() {
    let db = setup_db().await;

    // Create a custom provider
    let created = chronacle_lib::services::custom_provider_service::create(
        &db,
        "TestProvider",
        "openai",
        "https://test.api.com/v1",
        "sk-test-123",
    )
    .await
    .expect("create should succeed");
    assert_eq!(created.name, "TestProvider");
    assert_eq!(created.provider_type, "openai");

    // Add models
    let model1 = chronacle_lib::services::custom_provider_service::add_model(
        &db,
        &created.id,
        "gpt-4o",
        "GPT-4o",
    )
    .await
    .expect("add model should succeed");
    assert_eq!(model1.model_id, "gpt-4o");
    assert_eq!(model1.display_name, "GPT-4o");

    let _model2 = chronacle_lib::services::custom_provider_service::add_model(
        &db,
        &created.id,
        "claude-3-haiku",
        "Claude 3 Haiku",
    )
    .await
    .expect("add model should succeed");

    // Get models
    let models = chronacle_lib::services::custom_provider_service::get_models(&db, &created.id)
        .await
        .expect("get models should succeed");
    assert_eq!(models.len(), 2);

    // Get all providers
    let all = chronacle_lib::services::custom_provider_service::get_all(&db)
        .await
        .expect("get all should succeed");
    assert!(!all.is_empty());
    assert!(all.iter().any(|p| p.name == "TestProvider"));

    // Delete a model
    chronacle_lib::services::custom_provider_service::remove_model(&db, &model1.id)
        .await
        .expect("remove model should succeed");
    let models_after =
        chronacle_lib::services::custom_provider_service::get_models(&db, &created.id)
            .await
            .expect("get models after delete should succeed");
    assert_eq!(models_after.len(), 1);
    assert_eq!(models_after[0].model_id, "claude-3-haiku");
    // Delete the provider (should cascade-delete models)
    chronacle_lib::services::custom_provider_service::delete(&db, &created.id)
        .await
        .expect("delete should succeed");
    let after_delete = chronacle_lib::services::custom_provider_service::get_all(&db)
        .await
        .expect("get all after delete should succeed");
    assert!(
        after_delete.iter().all(|p| p.id != created.id),
        "provider should be deleted"
    );

    // Models should also be gone (cascade delete)
    let models_final =
        chronacle_lib::services::custom_provider_service::get_models(&db, &created.id)
            .await
            .expect("get models after provider delete should succeed");
    assert!(models_final.is_empty(), "models should be cascade-deleted");
}

#[tokio::test]
async fn test_custom_provider_duplicate_name() {
    let db = setup_db().await;

    chronacle_lib::services::custom_provider_service::create(
        &db,
        "Duplicate",
        "openai",
        "https://api1.com",
        "key1",
    )
    .await
    .expect("first create should succeed");

    let result = chronacle_lib::services::custom_provider_service::create(
        &db,
        "Duplicate",
        "anthropic",
        "https://api2.com",
        "key2",
    )
    .await;
    assert!(result.is_err(), "duplicate name should fail");
}

#[tokio::test]
async fn test_custom_provider_update() {
    let db = setup_db().await;

    let created = chronacle_lib::services::custom_provider_service::create(
        &db,
        "UpdateMe",
        "openai",
        "https://old.api.com",
        "old-key",
    )
    .await
    .expect("create should succeed");

    let updated = chronacle_lib::services::custom_provider_service::update(
        &db,
        &created.id,
        "UpdatedName",
        "anthropic",
        "https://new.api.com",
        "new-key",
    )
    .await
    .expect("update should succeed");

    assert_eq!(updated.name, "UpdatedName");
    assert_eq!(updated.provider_type, "anthropic");
    assert_eq!(updated.base_url, "https://new.api.com");
    assert_eq!(updated.api_key, "new-key");
}
