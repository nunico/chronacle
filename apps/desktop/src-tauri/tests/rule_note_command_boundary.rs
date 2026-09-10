use std::sync::{Arc, RwLock};

use chronacle_lib::{commands, AppState};
use chronacle_providers::blob_store::LocalFileStore;
use chronacle_providers::embedding::MockEmbeddingProvider;
use chronacle_providers::llm_provider::NoopProvider;
use chronacle_providers::vector_store::SurrealDbVector;
use serde_json::{json, Value};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};

struct UnusedPdfExtractor;

#[async_trait::async_trait]
impl chronacle_ingestion::pdf_extractor::PdfExtractor for UnusedPdfExtractor {
    async fn extract_with_progress(
        &self,
        _data: &[u8],
        _on_page: chronacle_ingestion::pdf_extractor::PageProgressFn,
    ) -> Result<
        chronacle_ingestion::chunker::ExtractedDoc,
        chronacle_ingestion::pdf_extractor::PdfExtractError,
    > {
        panic!("rule-note command tests must not extract PDFs");
    }
}

async fn build_state() -> (Arc<AppState>, tempfile::TempDir) {
    let db = surrealdb::engine::any::connect("mem://")
        .await
        .expect("in-memory database");
    db.use_ns("rule_note_command")
        .use_db("rule_note_command")
        .await
        .expect("namespace and database");
    chronacle_db::run_migrations(&db).await.expect("migrations");

    let scratch = tempfile::TempDir::new().expect("scratch directory");
    let vector_store: Arc<dyn chronacle_providers::vector_store::VectorStore> =
        Arc::new(SurrealDbVector::new(db.clone()));
    let blob_store: Arc<dyn chronacle_providers::blob_store::BlobStore> =
        Arc::new(LocalFileStore::new(scratch.path().join("pdfs")));
    let embedding_provider: Arc<dyn chronacle_providers::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    let llm_provider: Arc<dyn chronacle_providers::llm_provider::LlmProvider> =
        Arc::new(NoopProvider);

    (
        Arc::new(AppState {
            db,
            llm_provider: RwLock::new(llm_provider),
            vector_store,
            blob_store,
            embedding_provider: RwLock::new(embedding_provider),
            pdf_extractor: Arc::new(UnusedPdfExtractor),
            chat_task: tokio::sync::Mutex::new(None),
            extract_task: tokio::sync::Mutex::new(None),
            compile_task: tokio::sync::Mutex::new(None),
            vault: tokio::sync::RwLock::new(None),
            outbound: tokio::sync::RwLock::new(Arc::new(chronacle_core::NoopOutbound)),
        }),
        scratch,
    )
}

fn invoke_rule_note(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    id: &str,
    notes: Option<&str>,
) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        tauri::webview::InvokeRequest {
            cmd: "update_rule_notes".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().expect("invoke URL"),
            body: tauri::ipc::InvokeBody::Json(json!({ "id": id, "notes": notes })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize::<Value>().expect("JSON command response"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rule_note_command_serializes_canonical_success_and_typed_not_found() {
    let (state, _scratch) = build_state().await;
    state
        .db
        .query(
            "CREATE collection:rules SET name = 'Rules', description = NULL, \
             created_at = time::now(), updated_at = time::now(); \
             CREATE rule_entry:initiative SET collection = collection:rules, \
             name = 'Initiative', category = 'mechanic', body = 'Roll initiative.', \
             notes = NULL, page_refs = [], sources = [], compiled_at = time::now(), stale = false;",
        )
        .await
        .expect("seed rule entry")
        .check()
        .expect("valid seed statements");

    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![commands::update_rule_notes])
        .build(mock_context(noop_assets()))
        .expect("mock Tauri application");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview");

    let saved = invoke_rule_note(&webview, "initiative", Some("Roll once per combatant."))
        .expect("existing target succeeds");
    assert_eq!(saved["id"], "initiative");
    assert_eq!(saved["name"], "Initiative");
    assert_eq!(saved["category"], "mechanic");
    assert_eq!(saved["body"], "Roll initiative.");
    assert_eq!(saved["notes"], "Roll once per combatant.");
    assert_eq!(saved["page_refs"], json!([]));
    assert_eq!(saved["stale"], false);

    let missing = invoke_rule_note(&webview, "missing-rule", Some("Retain this locally."))
        .expect_err("missing target returns the Tauri error channel");
    assert_eq!(
        missing,
        json!({
            "code": "NOT_FOUND",
            "message": "Rule entry missing-rule not found",
        })
    );
}
