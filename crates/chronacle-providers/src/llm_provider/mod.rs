use async_trait::async_trait;
use tokio::sync::mpsc;

mod anthropic;
mod ollama;
mod openai;
mod sse;

pub use anthropic::AnthropicProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;

pub use chronacle_core::llm::{ChatMessage, LlmError, LlmProvider};

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
