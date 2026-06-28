use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use super::sse::anthropic_parse_sse;
use super::{ChatMessage, LlmError, LlmProvider};

/// Provider that uses the Anthropic Messages API.
pub struct AnthropicProvider {
    pub(super) api_key: String,
    // Phase 1 stub — will be read in the Phase 2 implementation.
    #[allow(dead_code)] // stored for future use in Phase 2 implementation
    pub(super) model: String,
    pub(super) base_url: String,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    ///
    /// The default model is `claude-3-5-haiku-20241022` (fast and
    /// cost-effective for GM assistant use). When `api_key` is empty the
    /// provider returns a configuration error.
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model: if model.is_empty() {
                "claude-3-5-haiku-20241022".to_string()
            } else {
                model
            },
            base_url: "https://api.anthropic.com/v1".to_string(),
        }
    }

    /// Create a new Anthropic provider with a custom base URL (for
    /// Anthropic-compatible third-party services).
    pub fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        let base = if base_url.is_empty() {
            "https://api.anthropic.com/v1".to_string()
        } else {
            let trimmed = base_url.trim_end_matches('/').to_string();
            // Remove /messages suffix if accidentally included (e.g. user
            // pasted the full Anthropic endpoint URL as base URL).
            if trimmed.ends_with("/messages") {
                trimmed.trim_end_matches("/messages").to_string()
            } else {
                trimmed
            }
        };
        Self {
            api_key,
            model: if model.is_empty() {
                "claude-3-5-haiku-20241022".to_string()
            } else {
                model
            },
            base_url: base,
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn provider_type(&self) -> &'static str {
        "anthropic"
    }

    async fn chat_stream(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::Config(
                "Anthropic API key not configured.".to_string(),
            ));
        }

        let (tx, rx) = mpsc::channel(64);
        let client = Client::new();
        let url = format!("{}/messages", self.base_url);
        let model = self.model.clone();
        let api_key = self.api_key.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();

        tokio::spawn(async move {
            // Build Anthropic messages (no system in messages array — use top-level field)
            let body_messages: Vec<Value> = msgs
                .iter()
                .filter(|m| m.role != "system") // system goes in top-level field
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content
                    })
                })
                .collect();

            let mut body = serde_json::json!({
                "model": model,
                "messages": body_messages,
                "max_tokens": 4096,
                "stream": true
            });

            // Add system prompt if present
            if !system.is_empty() {
                body["system"] = Value::String(system);
            }

            let response = match client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let err: Result<String, LlmError> = Err(LlmError::Connection(format!(
                        "Failed to connect to Anthropic: {e}"
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
                let err: Result<String, LlmError> = match status.as_u16() {
                    429 => Err(LlmError::RateLimited),
                    401 => Err(LlmError::Config("Invalid Anthropic API key".to_string())),
                    _ => Err(LlmError::Api(format!("HTTP {status}: {text}"))),
                };
                let _ = tx.send(err).await;
                return;
            }

            anthropic_parse_sse(response, tx).await;
        });

        Ok(rx)
    }
}
