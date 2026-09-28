use async_trait::async_trait;
use menzi_common::{MenziError, Result};
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;

use super::types::*;

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse>;
    async fn embed(&self, request: &EmbedRequest) -> Result<EmbedResponse>;
}

pub struct OpenAICompatibleAdapter {
    pub base_url: String,
    pub api_key: String,
    pub provider_id: String,
    http: Client,
}

impl OpenAICompatibleAdapter {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        provider_id: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            provider_id: provider_id.into(),
            http: Client::new(),
        }
    }
}

#[async_trait]
impl ProviderAdapter for OpenAICompatibleAdapter {
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(request)
            .send()
            .await
            .map_err(|e| MenziError::Gateway(format!("provider request failed: {e}")))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| MenziError::Gateway(format!("provider response failed: {e}")))?;
        if !status.is_success() {
            return Err(MenziError::Gateway(format!(
                "provider returned {status}: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        serde_json::from_slice(&bytes)
            .map_err(|e| MenziError::Gateway(format!("invalid provider response: {e}")))
    }

    async fn embed(&self, request: &EmbedRequest) -> Result<EmbedResponse> {
        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));
        let body = json!({ "model": request.model, "input": request.input });
        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| MenziError::Gateway(format!("provider request failed: {e}")))?;
        let status = response.status();
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| MenziError::Gateway(format!("invalid provider response: {e}")))?;
        if !status.is_success() {
            return Err(MenziError::Gateway(format!("provider returned {status}")));
        }
        let model = value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(&request.model)
            .to_string();
        let embedding = value
            .get("data")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("embedding"))
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_f64()).collect())
            .unwrap_or_default();
        Ok(EmbedResponse { model, embedding })
    }
}

pub struct AnthropicAdapter {
    pub api_key: String,
    pub base_url: String,
    http: Client,
}

impl AnthropicAdapter {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.anthropic.com".to_string(),
            http: Client::new(),
        }
    }

    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let max_tokens = request.max_tokens.unwrap_or(4096);
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|message| json!({ "role": message.role, "content": message.content }))
            .collect();
        let body = json!({
            "model": request.model,
            "max_tokens": max_tokens,
            "messages": messages,
        });
        let response = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| MenziError::Gateway(format!("provider request failed: {e}")))?;
        let status = response.status();
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| MenziError::Gateway(format!("invalid provider response: {e}")))?;
        if !status.is_success() {
            return Err(MenziError::Gateway(format!("provider returned {status}")));
        }
        let id = value
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("chatcmpl-anthropic")
            .to_string();
        let model = value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(&request.model)
            .to_string();
        let content = value
            .get("content")
            .and_then(|v| v.as_array())
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| block.get("text").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();
        let prompt_tokens = value
            .pointer("/usage/input_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let completion_tokens = value
            .pointer("/usage/output_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        Ok(ChatResponse {
            id,
            model,
            choices: vec![Choice {
                index: 0,
                message: Message {
                    role: "assistant".to_string(),
                    content,
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
        })
    }

    async fn embed(&self, request: &EmbedRequest) -> Result<EmbedResponse> {
        Err(MenziError::Gateway(format!(
            "provider does not expose embeddings model {}",
            request.model
        )))
    }
}

pub struct VLLMAdapter {
    pub base_url: String,
    pub model: String,
    http: Client,
}

impl VLLMAdapter {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            http: Client::new(),
        }
    }
}

#[async_trait]
impl ProviderAdapter for VLLMAdapter {
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let mut forwarded = request.clone();
        if !self.model.is_empty() {
            forwarded.model = self.model.clone();
        }
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let response = self
            .http
            .post(&url)
            .json(&forwarded)
            .send()
            .await
            .map_err(|e| MenziError::Gateway(format!("provider request failed: {e}")))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| MenziError::Gateway(format!("provider response failed: {e}")))?;
        if !status.is_success() {
            return Err(MenziError::Gateway(format!(
                "provider returned {status}: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        serde_json::from_slice(&bytes)
            .map_err(|e| MenziError::Gateway(format!("invalid provider response: {e}")))
    }

    async fn embed(&self, request: &EmbedRequest) -> Result<EmbedResponse> {
        Err(MenziError::Gateway(format!(
            "provider does not expose embeddings model {}",
            request.model
        )))
    }
}

#[derive(Clone, Default)]
pub struct ProviderAdapterFactory {
    pub providers: std::collections::HashMap<String, ProviderInfo>,
    pub adapters: std::collections::HashMap<String, Arc<dyn ProviderAdapter>>,
}

impl ProviderAdapterFactory {
    pub fn new() -> Self {
        Self {
            providers: std::collections::HashMap::new(),
            adapters: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, provider: ProviderInfo) {
        self.providers.insert(provider.id.clone(), provider);
    }

    pub fn register_adapter(&mut self, id: impl Into<String>, adapter: Arc<dyn ProviderAdapter>) {
        self.adapters.insert(id.into(), adapter);
    }

    pub fn select(&self, id: &str) -> Option<Arc<dyn ProviderAdapter>> {
        self.adapters.get(id).cloned()
    }

    pub fn get(&self, id: &str) -> Option<&ProviderInfo> {
        self.providers.get(id)
    }

    pub fn list(&self) -> Vec<&ProviderInfo> {
        self.providers.values().collect()
    }

    pub fn is_available(&self, id: &str) -> bool {
        self.providers.contains_key(id) || self.adapters.contains_key(id)
    }

    pub fn default_for_env() -> Self {
        let mut factory = Self::new();
        let base_url = std::env::var("MENZI_LLM_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:17998/v1".to_string());
        let api_key =
            std::env::var("MENZI_LLM_API_KEY").unwrap_or_else(|_| "menzi-dev".to_string());
        let providers =
            std::env::var("MENZI_LLM_PROVIDERS").unwrap_or_else(|_| "openrouter".to_string());
        for id in providers.split(',').map(str::trim) {
            if id.is_empty() {
                continue;
            }
            factory.register_adapter(
                id.to_string(),
                Arc::new(OpenAICompatibleAdapter::new(
                    base_url.clone(),
                    api_key.clone(),
                    id.to_string(),
                )),
            );
            factory.register(ProviderInfo {
                id: id.to_string(),
                name: id.to_string(),
                models: vec![],
            });
        }
        factory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_ai_adapter_stores_config() {
        let adapter =
            OpenAICompatibleAdapter::new("https://llm-gw.example.com/v1", "key-123", "openrouter");
        assert_eq!(adapter.base_url, "https://llm-gw.example.com/v1");
        assert_eq!(adapter.api_key, "key-123");
        assert_eq!(adapter.provider_id, "openrouter");
    }

    #[test]
    fn anthropic_adapter_stores_config() {
        let adapter = AnthropicAdapter::new("sk-ant-123");
        assert_eq!(adapter.api_key, "sk-ant-123");
        assert_eq!(adapter.base_url, "https://api.anthropic.com");
    }

    #[test]
    fn vllm_adapter_stores_config() {
        let adapter = VLLMAdapter::new("http://localhost:8000/v1", "meta-llama/Llama-3.1-70B");
        assert_eq!(adapter.base_url, "http://localhost:8000/v1");
        assert_eq!(adapter.model, "meta-llama/Llama-3.1-70B");
    }

    #[test]
    fn factory_registers_providers() {
        let mut factory = ProviderAdapterFactory::new();
        factory.register(ProviderInfo {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            models: vec!["gpt-4".to_string()],
        });
        assert!(factory.is_available("openai"));
        assert!(!factory.is_available("anthropic"));
    }

    #[test]
    fn factory_lists_providers() {
        let mut factory = ProviderAdapterFactory::new();
        factory.register(ProviderInfo {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            models: vec![],
        });
        factory.register(ProviderInfo {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            models: vec![],
        });
        assert_eq!(factory.list().len(), 2);
    }

    #[test]
    fn factory_gets_provider() {
        let mut factory = ProviderAdapterFactory::new();
        factory.register(ProviderInfo {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            models: vec!["gpt-4".to_string()],
        });
        let provider = factory.get("openai").unwrap();
        assert_eq!(provider.name, "OpenAI");
    }

    #[test]
    fn factory_selects_registered_adapter() {
        let mut factory = ProviderAdapterFactory::new();
        factory.register_adapter(
            "openrouter",
            Arc::new(OpenAICompatibleAdapter::new(
                "http://localhost:17998/v1",
                "key",
                "openrouter",
            )),
        );
        assert!(factory.select("openrouter").is_some());
        assert!(factory.select("missing").is_none());
    }

    async fn start_fake_openai_provider(echo_model: bool) -> String {
        let app = axum::Router::new().route(
            "/v1/chat/completions",
            axum::routing::post(
                move |axum::Json(body): axum::Json<serde_json::Value>| async move {
                    let model = if echo_model {
                        body.get("model").and_then(|v| v.as_str()).unwrap_or("")
                    } else {
                        "openrouter/mock-gpt"
                    };
                    axum::Json(json!({
                        "id": "chatcmpl-fake",
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "message": {"role": "assistant", "content": "hello"},
                            "finish_reason": "stop"
                        }],
                        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
                    }))
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{address}")
    }

    fn chat_request(model: &str) -> ChatRequest {
        ChatRequest {
            model: model.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
            stream: false,
            max_tokens: None,
            temperature: None,
        }
    }

    #[tokio::test]
    async fn openai_adapter_chats_with_provider() {
        let target = start_fake_openai_provider(false).await;
        let adapter = OpenAICompatibleAdapter::new(format!("{target}/v1"), "key", "openrouter");
        let response = adapter
            .chat(&chat_request("openrouter/mock-gpt"))
            .await
            .unwrap();
        assert_eq!(response.model, "openrouter/mock-gpt");
        assert_eq!(response.choices[0].message.content, "hello");
        assert_eq!(response.usage.total_tokens, 15);
    }

    #[tokio::test]
    async fn vllm_adapter_overrides_model() {
        let target = start_fake_openai_provider(true).await;
        let adapter = VLLMAdapter::new(format!("{target}/v1"), "meta-llama/Llama-3.1-70B");
        let response = adapter
            .chat(&chat_request("openrouter/mock-any"))
            .await
            .unwrap();
        assert_eq!(response.model, "meta-llama/Llama-3.1-70B");
    }

    #[tokio::test]
    async fn openai_adapter_embed_parses_response() {
        let app = axum::Router::new().route(
            "/v1/embeddings",
            axum::routing::post(|| async move {
                axum::Json(json!({
                    "model": "text-embedding-3-small",
                    "data": [{"embedding": [0.1, 0.2, 0.3]}]
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let adapter =
            OpenAICompatibleAdapter::new(format!("http://{address}/v1"), "key", "openrouter");
        let response = adapter
            .embed(&EmbedRequest {
                model: "text-embedding-3-small".to_string(),
                input: "hello".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(response.embedding.len(), 3);
    }
}
