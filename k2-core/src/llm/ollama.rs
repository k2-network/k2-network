use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::error::LlmError;
use super::provider::{
    FinishReason, LlmProvider, LlmRequest, LlmResponse, MessageRole, Usage,
};

const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";

/// Provider for [Ollama](https://ollama.com) — local LLM inference.
///
/// Connects to an Ollama server (default `http://localhost:11434`)
/// and supports model listing via `/api/tags` and chat via `/api/chat`.
///
/// No API key is required — Ollama runs on your own machine.
///
/// ## Example
/// ```no_run
/// use k2_core::llm::{OllamaProvider, LlmProvider, LlmRequest, LlmMessage};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = OllamaProvider::new("http://localhost:11434", "llama3");
///
/// // List available models
/// let models = provider.list_models().await?;
/// println!("Available: {:?}", models);
///
/// // Chat
/// let req = LlmRequest::new("llama3", vec![LlmMessage::user("Hello!")]);
/// let resp = provider.chat(req).await?;
/// println!("{}", resp.content.unwrap_or_default());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OllamaProvider {
    base_url: String,
    default_model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    ///
    /// * `base_url` – Ollama server URL (e.g. `http://localhost:11434`).
    /// * `default_model` – model name used when a request does not specify one.
    pub fn new(base_url: impl Into<String>, default_model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            default_model: default_model.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Convenience: use `http://localhost:11434` with the given model.
    pub fn localhost(default_model: impl Into<String>) -> Self {
        Self::new(DEFAULT_OLLAMA_BASE_URL, default_model)
    }

    /// Override the base URL after construction.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

// ---- Ollama wire types ----

#[derive(Serialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct OllamaChatResponse {
    message: Option<OllamaMessage>,
    done: bool,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Deserialize, Debug)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelInfo>,
}

#[derive(Deserialize, Debug)]
struct OllamaModelInfo {
    name: String,
}

fn role_to_str(role: &MessageRole) -> String {
    match role {
        MessageRole::System => "system".to_string(),
        MessageRole::User => "user".to_string(),
        MessageRole::Assistant => "assistant".to_string(),
        MessageRole::Tool => "user".to_string(), // Ollama doesn't have tool role
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn chat(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let model = if request.model.is_empty() {
            self.default_model.clone()
        } else {
            request.model.clone()
        };

        let ollama_messages: Vec<OllamaMessage> = request
            .messages
            .iter()
            .map(|m| OllamaMessage {
                role: role_to_str(&m.role),
                content: m.content.clone(),
            })
            .collect();

        let body = OllamaChatRequest {
            model: &model,
            messages: ollama_messages,
            stream: false,
        };

        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));

        let http_resp = self.client.post(&url).json(&body).send().await?;

        if !http_resp.status().is_success() {
            let status = http_resp.status().as_u16();
            let body_text = http_resp.text().await.unwrap_or_default();
            return Err(LlmError::provider(status, body_text));
        }

        let data: OllamaChatResponse = http_resp.json().await?;

        let content = data.message.map(|m| m.content);

        let usage = if data.prompt_eval_count.is_some() || data.eval_count.is_some() {
            Some(Usage {
                prompt_tokens: data.prompt_eval_count.unwrap_or(0),
                completion_tokens: data.eval_count.unwrap_or(0),
                total_tokens: data.prompt_eval_count.unwrap_or(0)
                    + data.eval_count.unwrap_or(0),
            })
        } else {
            None
        };

        Ok(LlmResponse {
            content,
            tool_calls: None,
            finish_reason: if data.done {
                FinishReason::Stop
            } else {
                FinishReason::Unknown("incomplete".to_string())
            },
            model,
            usage,
        })
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let url = format!("{}/api/tags", self.base_url.trim_end_matches('/'));

        let http_resp = self.client.get(&url).send().await?;

        if !http_resp.status().is_success() {
            let status = http_resp.status().as_u16();
            let body_text = http_resp.text().await.unwrap_or_default();
            return Err(LlmError::provider(status, body_text));
        }

        let data: OllamaTagsResponse = http_resp.json().await?;

        Ok(data.models.into_iter().map(|m| m.name).collect())
    }

    fn name(&self) -> &str {
        "ollama"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructor() {
        let p = OllamaProvider::new("http://127.0.0.1:11434", "mistral");
        assert_eq!(p.base_url, "http://127.0.0.1:11434");
        assert_eq!(p.default_model, "mistral");
    }

    #[test]
    fn test_localhost_constructor() {
        let p = OllamaProvider::localhost("llama3");
        assert_eq!(p.base_url, DEFAULT_OLLAMA_BASE_URL);
        assert_eq!(p.default_model, "llama3");
    }

    #[test]
    fn test_name() {
        let p = OllamaProvider::localhost("m");
        assert_eq!(p.name(), "ollama");
    }

    #[test]
    fn test_chat_request_serialization() {
        let messages = vec![
            OllamaMessage {
                role: "system".to_string(),
                content: "Be brief.".to_string(),
            },
            OllamaMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
        ];
        let body = OllamaChatRequest {
            model: "llama3",
            messages,
            stream: false,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("llama3"));
        assert!(json.contains("Be brief."));
        assert!(json.contains("\"stream\":false"));
    }

    #[test]
    fn test_chat_response_deserialization() {
        let json = r#"{
            "model": "llama3:latest",
            "created_at": "2024-01-01T00:00:00Z",
            "message": {
                "role": "assistant",
                "content": "Hi there! How can I help?"
            },
            "done": true,
            "total_duration": 1250000000,
            "prompt_eval_count": 15,
            "eval_count": 8
        }"#;
        let data: OllamaChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            data.message.as_ref().unwrap().content,
            "Hi there! How can I help?"
        );
        assert!(data.done);
        assert_eq!(data.prompt_eval_count, Some(15));
        assert_eq!(data.eval_count, Some(8));
    }

    #[test]
    fn test_tags_response_deserialization() {
        let json = r#"{
            "models": [
                {"name": "llama3:latest", "modified_at": "2024-01-01T00:00:00Z", "size": 4661224448},
                {"name": "mistral:7b", "modified_at": "2024-01-02T00:00:00Z", "size": 4108914688}
            ]
        }"#;
        let data: OllamaTagsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(data.models.len(), 2);
        assert_eq!(data.models[0].name, "llama3:latest");
        assert_eq!(data.models[1].name, "mistral:7b");
    }
}
