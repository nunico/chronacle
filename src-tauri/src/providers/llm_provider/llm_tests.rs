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
    let provider = OllamaProvider::new(
        "http://192.168.1.100:11434".to_string(),
        "mistral".to_string(),
    );
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
