use super::local;
use super::*;
use fastembed::EmbeddingModel;

#[test]
fn ort_dylib_name_matches_target_platform() {
    if cfg!(target_os = "macos") {
        assert!(local::ORT_DYLIB_NAME.ends_with(".dylib"));
    } else if cfg!(target_os = "windows") {
        assert!(local::ORT_DYLIB_NAME.ends_with(".dll"));
    } else {
        assert!(local::ORT_DYLIB_NAME.ends_with(".so"));
    }
}

#[test]
fn resolve_onnxruntime_library_path_honors_ort_dylib_path_env() {
    use std::env;
    // The seam the desktop shell relies on in a source checkout / --no-bundle
    // build: it sets ORT_DYLIB_PATH so the provider can locate the lib whose
    // path only the shell's (correct) CARGO_MANIFEST_DIR knows. An existing
    // path wins; a bogus one is ignored so resolution falls through to the
    // exe-adjacent / system candidates instead of returning a dead path.
    let prev = env::var_os("ORT_DYLIB_PATH");

    let tmp = tempfile::NamedTempFile::new().unwrap();
    env::set_var("ORT_DYLIB_PATH", tmp.path());
    assert_eq!(
        local::resolve_onnxruntime_library_path().as_deref(),
        Some(tmp.path()),
        "an existing ORT_DYLIB_PATH should be resolved verbatim",
    );

    let bogus = std::path::Path::new("/nonexistent/onnxruntime/does-not-exist.so");
    env::set_var("ORT_DYLIB_PATH", bogus);
    assert_ne!(
        local::resolve_onnxruntime_library_path().as_deref(),
        Some(bogus),
        "a non-existent ORT_DYLIB_PATH must not be returned",
    );

    match prev {
        Some(v) => env::set_var("ORT_DYLIB_PATH", v),
        None => env::remove_var("ORT_DYLIB_PATH"),
    }
}

#[tokio::test]
async fn test_mock_embed_query_returns_correct_dims() {
    let provider = MockEmbeddingProvider::new(768);
    let vec = provider.embed_query("test").await.unwrap();
    assert_eq!(vec.len(), 768);
}

#[tokio::test]
async fn test_mock_embed_batch() {
    let provider = MockEmbeddingProvider::new(384);
    let result = provider
        .embed_documents(vec!["hello".into(), "world".into()])
        .await
        .unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].len(), 384);
    assert_eq!(result[1].len(), 384);
}

#[tokio::test]
async fn test_mock_provider_implements_split_trait() {
    let provider = MockEmbeddingProvider::new(384);
    let docs = provider
        .embed_documents(vec!["hello".into(), "world".into()])
        .await
        .unwrap();
    assert_eq!(docs.len(), 2);
    let q = provider.embed_query("hello").await.unwrap();
    assert_eq!(q.len(), 384);
}

#[tokio::test]
#[ignore = "downloads ~80 MB model; run locally with: cargo test -- --ignored"]
async fn test_fastembed_document_and_query_paths_compile() {
    // Confirms the trait surface is wired correctly. Returns same-dimension
    // vectors for both methods. all-MiniLM-L6-v2 doesn't use prefixes, but
    // the Nomic-prefix logic is gated on the model name (see
    // uses_nomic_prefixes), so both paths exercise the trait shape.
    let Ok(provider) = FastEmbedProvider::try_new_small() else {
        eprintln!("Skipping — small model not cached");
        return;
    };
    let raw = "Lantern orbits the planet Mirovia";
    let as_doc = provider
        .embed_documents(vec![raw.to_string()])
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let as_query = provider.embed_query(raw).await.unwrap();
    assert_eq!(as_doc.len(), as_query.len());
    assert!(as_doc.iter().any(|&v| v != 0.0));
}

#[tokio::test]
async fn test_mock_model_name() {
    let provider = MockEmbeddingProvider::new(768);
    assert_eq!(provider.model_name(), "mock");
}

#[test]
fn test_model_constants() {
    assert_eq!(
        FastEmbedProvider::model_dimension(&EmbeddingModel::NomicEmbedTextV15),
        768
    );
    assert_eq!(
        FastEmbedProvider::model_dimension(&EmbeddingModel::AllMiniLML6V2),
        384
    );
    assert_eq!(
        FastEmbedProvider::model_to_name(&EmbeddingModel::NomicEmbedTextV15),
        "nomic-embed-text-v1.5"
    );
}

#[test]
fn test_is_cached_returns_false_for_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!FastEmbedProvider::is_cached(dir.path()));
}

#[tokio::test]
#[ignore = "downloads ~80 MB model; run locally with: cargo test -- --ignored"]
async fn test_fastembed_try_new_small() {
    match FastEmbedProvider::try_new_small() {
        Ok(provider) => {
            assert_eq!(provider.dimension(), 384);
            assert_eq!(provider.model_name(), "all-MiniLM-L6-v2");

            let vec = provider.embed_query("hello world").await.unwrap();
            assert_eq!(vec.len(), 384);
            let has_nonzero = vec.iter().any(|&v| v != 0.0);
            assert!(has_nonzero, "embedding should have non-zero values");
        }
        Err(e) => {
            eprintln!(
                "Skipping real fastembed test — model not cached ({e}). \
                 Run again after model is downloaded."
            );
        }
    }
}
