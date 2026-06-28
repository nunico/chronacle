use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use super::sse::openai_parse_sse;
use super::{ChatMessage, LlmError, LlmProvider};

/// Provider that uses the OpenAI chat-completion API.
pub struct OpenAIProvider {
    pub(super) api_key: String,
    // Phase 1 stub — fields are stored and will be used in the Phase 2 implementation.
    #[allow(dead_code)] // stored for future use in Phase 2 implementation
    pub(super) model: String,
    #[allow(dead_code)] // stored for future use in Phase 2 implementation
    pub(super) base_url: String,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider.
    ///
    /// When `api_key` is empty the provider returns a configuration error at
    /// invocation time.
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model: if model.is_empty() {
                "gpt-4o-mini".to_string()
            } else {
                model
            },
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }

    /// Create a new OpenAI provider with a custom base URL (for OpenRouter,
    /// Azure OpenAI, etc.).
    pub fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        let base = if base_url.is_empty() {
            "https://api.openai.com/v1".to_string()
        } else {
            let trimmed = base_url.trim_end_matches('/').to_string();
            // If the URL doesn't already end with /chat/completions suffix path,
            // ensure it's the API root
            if trimmed.ends_with("/chat/completions") {
                trimmed.trim_end_matches("/chat/completions").to_string()
            } else {
                trimmed
            }
        };
        Self {
            api_key,
            model: if model.is_empty() {
                "gpt-4o-mini".to_string()
            } else {
                model
            },
            base_url: base,
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn provider_type(&self) -> &'static str {
        "openai"
    }

    async fn chat_stream(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::Config(
                "OpenAI API key not configured. Use the settings page to set it.".to_string(),
            ));
        }

        let (tx, rx) = mpsc::channel(64);
        let client = Client::new();
        let url = format!("{}/chat/completions", self.base_url);
        let model = self.model.clone();
        let api_key = self.api_key.clone();
        let system = system_prompt.to_string();
        let msgs = messages.to_vec();

        tokio::spawn(async move {
            // Build the messages array
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
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let err: Result<String, LlmError> = Err(LlmError::Connection(format!(
                        "Failed to connect to OpenAI: {e}"
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
                    401 => Err(LlmError::Config("Invalid OpenAI API key".to_string())),
                    _ => Err(LlmError::Api(format!("HTTP {status}: {text}"))),
                };
                let _ = tx.send(err).await;
                return;
            }

            openai_parse_sse(response, tx).await;
        });

        Ok(rx)
    }
}
