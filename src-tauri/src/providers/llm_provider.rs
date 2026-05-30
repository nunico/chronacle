use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        Err(LlmError::Config("No LLM provider configured".to_string()))
    }
}

// ── SSE streaming helpers ──────────────────────────────────────────────

/// Parse an OpenAI-style SSE stream and push tokens through the channel.
///
/// OpenAI SSE format:
/// ```text
/// data: {"id":"...","object":"chat.completion.chunk","choices":[{"delta":{"content":"Hello"},"index":0}]}
///
/// data: [DONE]
/// ```
async fn openai_parse_sse(
    mut response: reqwest::Response,
    tx: mpsc::Sender<Result<String, LlmError>>,
) {
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk);

                // Process all complete events in the buffer.
                // Each SSE event is: `data: <json>\n\n`, possibly with intermediate `data: [DONE]\n\n`.
                let mut consumed = 0usize;
                while consumed < buf.len() {
                    // Find the next \n\n boundary
                    let remaining = &buf[consumed..];
                    if let Some(dd) = remaining
                        .windows(2)
                        .position(|w| w == b"\n\n")
                    {
                        let event_end = consumed + dd + 2;
                        let event_slice = &buf[consumed..event_end];
                        let raw = String::from_utf8_lossy(event_slice);

                        for line in raw.lines() {
                            let line = line.trim();
                            if let Some(data) = line.strip_prefix("data: ") {
                                let data = data.trim();
                                if data == "[DONE]" {
                                    // Signal the end (no more tokens).
                                    // We still want to flush final tokens, then return.
                                    return;
                                }

                                match serde_json::from_str::<Value>(data) {
                                    Ok(json) => {
                                        // Extract text from choices[0].delta.content
                                        if let Some(content) = json
                                            .pointer("/choices/0/delta/content")
                                            .and_then(|v| v.as_str())
                                        {
                                            if !content.is_empty()
                                                && tx.send(Ok(content.to_string())).await.is_err()
                                            {
                                                return; // receiver dropped
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("SSE parse warning — invalid JSON: {e}");
                                    }
                                }
                            }
                        }

                        consumed = event_end;
                    } else {
                        break; // wait for more data
                    }
                }

                if consumed > 0 {
                    buf.drain(..consumed);
                }
            }
            Ok(None) => return, // stream ended normally
            Err(e) => {
                let _ = tx.send(Err(LlmError::Connection(e.to_string()))).await;
                return;
            }
        }
    }
}

/// Parse an Anthropic-style SSE stream and push tokens through the channel.
///
/// Anthropic SSE format:
/// ```text
/// event: content_block_delta
/// data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}
///
/// event: message_stop
/// data: {"type":"message_stop"}
/// ```
async fn anthropic_parse_sse(
    mut response: reqwest::Response,
    tx: mpsc::Sender<Result<String, LlmError>>,
) {
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk);

                let mut consumed = 0usize;
                while consumed < buf.len() {
                    let remaining = &buf[consumed..];
                    if let Some(dd) = remaining
                        .windows(2)
                        .position(|w| w == b"\n\n")
                    {
                        let event_end = consumed + dd + 2;
                        let event_slice = &buf[consumed..event_end];
                        let raw = String::from_utf8_lossy(event_slice);

                        // Track the current event type from `event:` lines
                        let mut event_type: Option<&str> = None;
                        let mut data_json: Option<&str> = None;

                        for line in raw.lines() {
                            let line = line.trim();
                            if let Some(ev) = line.strip_prefix("event: ") {
                                event_type = Some(ev.trim());
                            } else if let Some(d) = line.strip_prefix("data: ") {
                                data_json = Some(d.trim());
                            }
                        }

                        // Only extract text from content_block_delta events
                        if event_type == Some("content_block_delta") {
                            if let Some(json_str) = data_json {
                                if let Ok(json) = serde_json::from_str::<Value>(json_str) {
                                    if let Some(text) = json
                                        .pointer("/delta/text")
                                        .and_then(|v| v.as_str())
                                    {
                                        if !text.is_empty()
                                            && tx.send(Ok(text.to_string())).await.is_err()
                                        {
                                            return;
                                        }
                                    }
                                }
                            }
                        }

                        consumed = event_end;
                    } else {
                        break;
                    }
                }

                if consumed > 0 {
                    buf.drain(..consumed);
                }
            }
            Ok(None) => return,
            Err(e) => {
                let _ = tx.send(Err(LlmError::Connection(e.to_string()))).await;
                return;
            }
        }
    }
}

/// Parse an Ollama NDJSON stream and push tokens through the channel.
///
/// Ollama NDJSON format (one JSON object per line, `done: true` signals end):
/// ```json
/// {"model":"...","message":{"role":"assistant","content":"Hello"},"done":false}
/// {"model":"...","message":{"role":"assistant","content":""},"done_reason":"stop","done":true}
/// ```
async fn ollama_parse_ndjson(
    mut response: reqwest::Response,
    tx: mpsc::Sender<Result<String, LlmError>>,
) {
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk);

                // Process complete newline-delimited JSON objects
                let mut consumed = 0usize;
                while consumed < buf.len() {
                    let remaining = &buf[consumed..];
                    if let Some(nl) = remaining.iter().position(|&b| b == b'\n') {
                        let line_end = consumed + nl + 1; // include the newline
                        let line_slice = &buf[consumed..(consumed + nl)];
                        let line = String::from_utf8_lossy(line_slice).trim().to_string();

                        if !line.is_empty() {
                            match serde_json::from_str::<Value>(&line) {
                                Ok(json) => {
                                    // Extract message.content
                                    if let Some(content) = json
                                        .pointer("/message/content")
                                        .and_then(|v| v.as_str())
                                    {
                                        if !content.is_empty()
                                            && tx.send(Ok(content.to_string())).await.is_err()
                                        {
                                            return;
                                        }
                                    }

                                    // Check if done
                                    if json.get("done").and_then(|v| v.as_bool()) == Some(true) {
                                        return;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Ollama NDJSON parse warning: {e}");
                                }
                            }
                        }

                        consumed = line_end;
                    } else {
                        break; // wait for more data
                    }
                }

                if consumed > 0 {
                    buf.drain(..consumed);
                }
            }
            Ok(None) => return,
            Err(e) => {
                let _ = tx.send(Err(LlmError::Connection(e.to_string()))).await;
                return;
            }
        }
    }
}

// ── OpenAI Provider ────────────────────────────────────────────────────

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

// ── Anthropic Provider ─────────────────────────────────────────────────

/// Provider that uses the Anthropic Messages API.
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
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

// ── Ollama Provider ────────────────────────────────────────────────────

/// Provider that speaks the Ollama NDJSON wire format.
///
/// Ollama *does not* expose an OpenAI-compatible endpoint; requests must use
/// the raw Ollama API (`POST /api/chat` with NDJSON streaming).
pub struct OllamaProvider {
    base_url: String,
    model: String,
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
                let err: Result<String, LlmError> = Err(LlmError::Api(format!("HTTP {status}: {text}")));
                let _ = tx.send(err).await;
                return;
            }

            ollama_parse_ndjson(response, tx).await;
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
        let result = provider.chat_stream("test", &[]).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::Config(_)));
    }

    #[tokio::test]
    async fn test_anthropic_provider_returns_config_error_without_key() {
        let provider = AnthropicProvider::new(String::new(), String::new());
        let result = provider.chat_stream("test", &[]).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::Config(_)));
    }

    #[tokio::test]
    async fn test_openai_default_model() {
        let provider = OpenAIProvider::new("sk-test".to_string(), String::new());
        // We can't call chat_stream (would hit network), but verify construction
        assert!(provider.api_key == "sk-test");
        assert!(provider.model == "gpt-4o-mini");
        assert!(provider.base_url == "https://api.openai.com/v1");
    }

    #[tokio::test]
    async fn test_anthropic_default_model() {
        let provider = AnthropicProvider::new("sk-ant-test".to_string(), String::new());
        assert!(provider.api_key == "sk-ant-test");
        assert!(provider.model == "claude-3-5-haiku-20241022");
        assert!(provider.base_url == "https://api.anthropic.com/v1");
    }

    #[tokio::test]
    async fn test_anthropic_with_base_url() {
        let provider = AnthropicProvider::with_base_url(
            "sk-ant-test".to_string(),
            "claude-3-opus-20240229".to_string(),
            "https://custom.anthropic.com/v1".to_string(),
        );
        assert!(provider.api_key == "sk-ant-test");
        assert!(provider.model == "claude-3-opus-20240229");
        assert!(provider.base_url == "https://custom.anthropic.com/v1");
    }

    #[tokio::test]
    async fn test_anthropic_with_base_url_trailing_slash() {
        let provider = AnthropicProvider::with_base_url(
            "sk-ant-test".to_string(),
            String::new(),
            "https://custom.anthropic.com/v1/".to_string(),
        );
        assert!(provider.base_url == "https://custom.anthropic.com/v1");
    }

    #[tokio::test]
    async fn test_anthropic_with_base_url_empty_falls_back() {
        let provider = AnthropicProvider::with_base_url(
            "sk-ant-test".to_string(),
            String::new(),
            String::new(),
        );
        assert!(provider.base_url == "https://api.anthropic.com/v1");
    }

    #[tokio::test]
    async fn test_ollama_defaults() {
        let provider = OllamaProvider::new(String::new(), String::new());
        assert!(provider.base_url == "http://localhost:11434");
        assert!(provider.model == "llama3.2");
    }

    #[tokio::test]
    async fn test_ollama_custom_url() {
        let provider =
            OllamaProvider::new("http://192.168.1.100:11434".to_string(), "mistral".to_string());
        assert!(provider.base_url == "http://192.168.1.100:11434");
        assert!(provider.model == "mistral");
    }

    #[tokio::test]
    async fn test_openai_with_base_url_trailing_slash() {
        let provider = OpenAIProvider::with_base_url(
            "sk-test".to_string(),
            "gpt-4".to_string(),
            "https://openrouter.ai/api/v1/".to_string(),
        );
        assert!(provider.base_url == "https://openrouter.ai/api/v1");
    }

    #[tokio::test]
    async fn test_ollama_url_normalization() {
        let provider = OllamaProvider::new(
            "http://localhost:11434/api/chat".to_string(),
            "llama3.2".to_string(),
        );
        assert!(
            provider.base_url == "http://localhost:11434",
            "expected http://localhost:11434, got {}",
            provider.base_url
        );
    }

    #[tokio::test]
    async fn test_noop_provider_returns_config_error() {
        let provider = NoopProvider;
        let result = provider.chat_stream("test", &[]).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::Config(_)));
    }
}
