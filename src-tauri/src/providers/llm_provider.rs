use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// A single chat message exchanged between user, assistant, or system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Errors that can arise during LLM interactions.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Rate limited — try again later")]
    RateLimited,
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Unified trait for LLM providers.
///
/// Each provider (OpenAI, Anthropic, Ollama) implements this trait with its
/// own wire format. Consumers receive a streaming channel of result tokens.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a chat completion request and return a channel receiver that
    /// yields tokens as they arrive.
    async fn chat_stream(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError>;
}

/// A no-op LLM provider for tests and placeholder state.
///
/// Used when constructing `AppState` without a real provider configured.
/// Always returns an error so callers are forced to configure properly.
pub struct NoopProvider;

#[async_trait]
impl LlmProvider for NoopProvider {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        Err(LlmError::Config("No LLM provider configured".to_string()))
    }
}

/// Provider that uses the OpenAI chat-completion API.
pub struct OpenAIProvider {
    api_key: String,
    model: String,
    base_url: String,
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
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        // ── Phase 1 stub ────────────────────────────────────────────
        // TODO: Implement real OpenAI streaming via `async-openai`.
        //       Use `reqwest` for the raw HTTP streaming fallback.
        //
        // 1. Build request payload with system prompt + messages.
        // 2. POST to `{base_url}/chat/completions` with `stream: true`.
        // 3. Parse SSE / JSON-stream and push tokens into the channel.

        if self.api_key.is_empty() {
            return Err(LlmError::Config(
                "OpenAI API key not configured. Use the settings page to set it.".to_string(),
            ));
        }

        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            let _ = tx.send(Ok("[OpenAI response not yet implemented — Phase 1 stub. Please configure a provider on the Settings page.]".to_string())).await;
        });

        Ok(rx)
    }
}

/// Provider that uses the Anthropic Messages API.
pub struct AnthropicProvider {
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model: if model.is_empty() {
                "claude-3-5-sonnet-20241022".to_string()
            } else {
                model
            },
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        // ── Phase 1 stub ────────────────────────────────────────────
        // TODO: Implement real Anthropic streaming via `reqwest`.
        //       Anthropic uses its own SSE format (not OpenAI-compatible).

        if self.api_key.is_empty() {
            return Err(LlmError::Config(
                "Anthropic API key not configured.".to_string(),
            ));
        }

        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            let _ = tx
                .send(Ok("[Anthropic response not yet implemented — Phase 1 stub.]".to_string()))
                .await;
        });

        Ok(rx)
    }
}

/// Provider that speaks the Ollama NDJSON wire format.
///
/// Ollama *does not* expose an OpenAI-compatible endpoint; requests must use
/// the raw Ollama API (`POST /api/chat` with NDJSON streaming).
pub struct OllamaProvider {
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url: if base_url.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                base_url
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
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        // ── Phase 1 stub ────────────────────────────────────────────
        // TODO: Implement Ollama NDJSON streaming via `reqwest`.
        //       POST `{base_url}/api/chat` with NDJSON body, read
        //       newline-delimited JSON responses.

        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            let _ = tx
                .send(Ok("[Ollama response not yet implemented — Phase 1 stub.]".to_string()))
                .await;
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_openai_provider_returns_config_error_without_key() {
        let provider = OpenAIProvider::new(String::new(), "gpt-4o-mini".to_string());
        let result = provider
            .chat_stream("test", &[])
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::Config(_)));
    }
}
