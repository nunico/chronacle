//! Finding 4 (tranche-5 whole-branch review): a vault-path switch must tear
//! the OLD runtime (watcher + drain) down BEFORE clearing bases and
//! reconciling the NEW root, all under one continuous lock hold — and a
//! failing switch must leave the app with a live runtime (the old one,
//! restored), never none at all.

use std::sync::{Arc, RwLock};

use chronacle_lib::commands::vault_commands::set_vault_path_inner;
use chronacle_lib::AppState;
use chronacle_providers::blob_store::LocalFileStore;
use chronacle_providers::embedding::MockEmbeddingProvider;
use chronacle_providers::llm_provider::NoopProvider;
use chronacle_providers::vector_store::SurrealDbVector;

/// `PdfExtractor` is required by `AppState` but never exercised by these
/// tests — a body that panics if called is a deliberate tripwire, not a
/// missing implementation.
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
        panic!("PdfExtractor must not be called by the vault-path-switch tests");
    }
}

async fn build_state() -> (Arc<AppState>, tempfile::TempDir) {
    let db = surrealdb::engine::any::connect("mem://")
        .await
        .expect("mem");
    db.use_ns("t").use_db("t").await.unwrap();
    chronacle_db::run_migrations(&db).await.expect("migrations");

    let scratch = tempfile::TempDir::new().unwrap();
    let blob_store: Arc<dyn chronacle_providers::blob_store::BlobStore> =
        Arc::new(LocalFileStore::new(scratch.path().join("pdfs")));
    let vector_store: Arc<dyn chronacle_providers::vector_store::VectorStore> =
        Arc::new(SurrealDbVector::new(db.clone()));
    let embedding_provider: Arc<dyn chronacle_providers::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    let llm_provider: Arc<dyn chronacle_providers::llm_provider::LlmProvider> =
        Arc::new(NoopProvider);

    let state = Arc::new(AppState {
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
    });
    (state, scratch)
}

/// Switching from root A to root B replaces the runtime wholesale: exactly
/// one runtime is ever installed, bound to the latest root, with a live
/// watcher and drain handle. There is no window where two runtimes coexist.
#[tokio::test]
async fn switching_the_vault_path_replaces_the_runtime_wholesale() {
    let (state, _scratch) = build_state().await;
    let dir_a = tempfile::TempDir::new().unwrap();
    let dir_b = tempfile::TempDir::new().unwrap();
    let path_a = dir_a.path().to_str().unwrap().to_string();
    let path_b = dir_b.path().to_str().unwrap().to_string();

    set_vault_path_inner(&state, Some(path_a.clone()))
        .await
        .expect("first switch");
    {
        let guard = state.vault.read().await;
        let rt = guard.as_ref().expect("runtime installed");
        assert_eq!(rt.root, path_a);
        assert!(rt.watcher_task.is_some());
        assert!(rt.outbound_task.is_some());
    }

    set_vault_path_inner(&state, Some(path_b.clone()))
        .await
        .expect("second switch");
    {
        let guard = state.vault.read().await;
        let rt = guard.as_ref().expect("runtime replaced, not removed");
        assert_eq!(
            rt.root, path_b,
            "the runtime must now be bound to the new root, not the old one"
        );
        assert!(rt.watcher_task.is_some());
        assert!(rt.outbound_task.is_some());
    }
}

/// A failing switch (the new root cannot even be listed) must restore the
/// OLD runtime rather than leave the app with no vault runtime at all, and
/// must not persist the broken path.
#[tokio::test]
async fn a_failing_switch_restores_the_old_runtime() {
    let (state, scratch) = build_state().await;
    let dir_a = tempfile::TempDir::new().unwrap();
    let path_a = dir_a.path().to_str().unwrap().to_string();

    set_vault_path_inner(&state, Some(path_a.clone()))
        .await
        .expect("initial switch succeeds");

    // A regular file as the "root" makes `VaultStore::list` fail (read_dir on
    // a non-directory), which fails `VaultIndex::scan`, which fails
    // `reconcile()`, which fails `configure_vault_path` — without needing a
    // mock store.
    let bad_root = scratch.path().join("not-a-directory");
    std::fs::write(&bad_root, "nope").unwrap();

    let err = set_vault_path_inner(&state, Some(bad_root.to_str().unwrap().to_string()))
        .await
        .expect_err("a broken root must fail the switch");
    assert!(!err.is_empty());

    // The app must still have a live runtime: the OLD one, restored.
    let guard = state.vault.read().await;
    let rt = guard
        .as_ref()
        .expect("a failing vault-path switch must not leave the app with no vault runtime at all");
    assert_eq!(rt.root, path_a, "the OLD root must be back in force");
    assert!(
        rt.watcher_task.is_some(),
        "the restored watcher must be live"
    );
    assert!(
        rt.outbound_task.is_some(),
        "the restored drain must be live"
    );
    drop(guard);

    // The persisted setting must also still point at the old path.
    let settings = chronacle_lib::services::settings_service::get_all(&state.db)
        .await
        .unwrap();
    let persisted = settings
        .into_iter()
        .find(|s| s.key == "vault_sync_path")
        .map(|s| s.value);
    assert_eq!(persisted.as_deref(), Some(path_a.as_str()));
}

/// Clearing the vault path (`None`) leaves no runtime behind.
#[tokio::test]
async fn clearing_the_vault_path_leaves_no_runtime() {
    let (state, _scratch) = build_state().await;
    let dir_a = tempfile::TempDir::new().unwrap();

    set_vault_path_inner(&state, Some(dir_a.path().to_str().unwrap().to_string()))
        .await
        .expect("initial switch");
    assert!(state.vault.read().await.is_some());

    set_vault_path_inner(&state, None).await.expect("clear");
    assert!(state.vault.read().await.is_none());
}
