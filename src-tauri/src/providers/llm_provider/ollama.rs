use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use super::sse::ollama_parse_ndjson;
use super::{ChatMessage, LlmError, LlmProvider};

/// Provider that speaks the Ollama NDJSON wire format.
///
/// Ollama *does not* expose an OpenAI-compatible endpoint; requests must use
/// the raw Ollama API (`POST /api/chat` with NDJSON streaming).
pub struct OllamaProvider {
    // Phase 1 stub — fields are stored and will be used in the Phase 2 implementation.
    #[allow(dead_code)] // stored for future use in Phase 2 implementation
    pub(super) base_url: String,
    #[allow(dead_code)] // stored for future use in Phase 2 implementation
    pub(super) model: String,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    ///
    /// Defaults to `http://localhost:11434` for the base URL and `llama3.2`
    /// for the model. Empty strings are replaced with defaults.
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url: if base_url.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                let trimmed = base_url.trim_end_matches('/').to_string();
                // Remove /api/chat suffix if accidentally included
                if trimmed.ends_with("/api/chat") {
                    trimmed.trim_end_matches("/api/chat").to_string()
                } else if trimmed.ends_with("/api") {
                    trimmed.trim_end_matches("/api").to_string()
                } else {
                    trimmed
                }
            },
            model: if model.is_empty() {
                "llama3.2".to_string()
            } else {
                model
            },
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn provider_type(&self) -> &'static str {
        "ollama"
    }

    async fn chat_stream(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        let (tx, rx) = mpsc::channel(64);
        let client = Client::new();
        let url = format!("{}/api/chat", self.base_url);
        let model = self.model.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();

        tokio::spawn(async move {
            // Build Ollama messages array
            let mut body_messages: Vec<Value> = Vec::new();
            if !system.is_empty() {
                body_messages.push(serde_json::json!({
                    "role": "system",
                    "content": system
                }));
            }
            for msg in &msgs {
                body_messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                }));
            }

            let body = serde_json::json!({
                "model": model,
                "messages": body_messages,
                "stream": true
            });

            let response = match client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let err: Result<String, LlmError> = Err(LlmError::Connection(format!(
                        "Failed to connect to Ollama at {url}: {e}"
                    )));
                    if tx.send(err).await.is_err() {
                        return;
                    }
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                let err: Result<String, LlmError> =
                    Err(LlmError::Api(format!("HTTP {status}: {text}")));
                let _ = tx.send(err).await;
                return;
            }

            ollama_parse_ndjson(response, tx).await;
        });

        Ok(rx)
    }
}
