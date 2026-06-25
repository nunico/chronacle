use async_trait::async_trait;
use tokio::sync::mpsc;

mod anthropic;
mod ollama;
mod openai;
mod sse;

pub use anthropic::AnthropicProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;

/// A single chat message exchanged between user, assistant, or system.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

    /// Return a short identifier for this provider type, e.g. "openai",
    /// "anthropic", or "ollama".
    fn provider_type(&self) -> &'static str;
}

/// A no-op LLM provider for tests and placeholder state.
///
/// Used when constructing `AppState` without a real provider configured.
/// Always returns an error so callers are forced to configure properly.
pub struct NoopProvider;

#[async_trait]
impl LlmProvider for NoopProvider {
    fn provider_type(&self) -> &'static str {
        "noop"
    }

    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        Err(LlmError::Config("No LLM provider configured".to_string()))
    }
}

#[cfg(test)]
#[path = "llm_tests.rs"]
mod llm_tests;
