use super::openai::{decode_openai_embeddings_response, normalize_openai_base_url};
use super::*;

#[test]
fn openai_base_url_normalization() {
    assert_eq!(normalize_openai_base_url(""), "https://api.openai.com/v1");
    assert_eq!(
        normalize_openai_base_url("   "),
        "https://api.openai.com/v1"
    );
    assert_eq!(
        normalize_openai_base_url("https://api.openai.com/v1/"),
        "https://api.openai.com/v1"
    );
    assert_eq!(
        normalize_openai_base_url("https://proxy.local/v1/embeddings"),
        "https://proxy.local/v1"
    );
    assert_eq!(
        normalize_openai_base_url("https://azure.example/openai"),
        "https://azure.example/openai"
    );
}

#[test]
fn openai_model_identity_and_defaults() {
    let p = OpenAiEmbeddingProvider::new(String::new(), String::new(), String::new());
    // Empty model falls back to the default; identity encodes model + dim.
    assert_eq!(p.model_name(), "openai:text-embedding-3-small:768");
    assert_eq!(p.dimension(), CLOUD_EMBED_DIM);

    let p2 =
        OpenAiEmbeddingProvider::new("k".into(), "text-embedding-3-large".into(), String::new());
    assert_eq!(p2.model_name(), "openai:text-embedding-3-large:768");
}

#[tokio::test]
async fn openai_empty_key_is_a_configuration_error() {
    let p = OpenAiEmbeddingProvider::new(String::new(), String::new(), String::new());
    let err = p.embed_query("hello").await.unwrap_err();
    assert!(matches!(err, EmbeddingError::Init(_)), "got {err:?}");
}

#[test]
fn openai_embed_orders_by_index_and_checks_dim() {
    let v0: Vec<f32> = vec![0.1; CLOUD_EMBED_DIM];
    let v1: Vec<f32> = vec![0.2; CLOUD_EMBED_DIM];
    let out = decode_openai_embeddings_response(serde_json::json!({
        "data": [
            { "index": 1, "embedding": v1 },
            { "index": 0, "embedding": v0 },
        ]
    }))
    .unwrap();

    assert_eq!(out.len(), 2);
    assert_eq!(out[0].len(), CLOUD_EMBED_DIM);
    // Sorted by index: 0 -> 0.1, 1 -> 0.2.
    assert!((out[0][0] - 0.1).abs() < 1e-6);
    assert!((out[1][0] - 0.2).abs() < 1e-6);
}

#[test]
fn openai_embed_rejects_wrong_dimensions() {
    let err = decode_openai_embeddings_response(serde_json::json!({
        "data": [
            { "index": 0, "embedding": [0.1, 0.2] },
        ]
    }))
    .unwrap_err();

    assert!(matches!(err, EmbeddingError::Embed(_)), "got {err:?}");
    assert!(
        err.to_string().contains("expected 768-dim vectors"),
        "got {err:?}"
    );
}

#[tokio::test]
async fn openai_embed_documents_empty_is_noop() {
    let p = OpenAiEmbeddingProvider::new("k".into(), String::new(), String::new());
    assert!(p.embed_documents(vec![]).await.unwrap().is_empty());
}
