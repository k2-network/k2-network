use async_trait::async_trait;

use super::error::LlmError;
use super::provider::{
    FinishReason, LlmProvider, LlmRequest, LlmResponse, Usage,
};

/// A generic provider for any OpenAI-compatible API endpoint.
///
/// Works with OpenAI, Azure OpenAI, DeepSeek, Together AI, Fireworks,
/// Groq, and any self-hosted vLLM/llama.cpp server exposing the
/// `/chat/completions` route.
///
/// ## Authentication
/// Pass the API key explicitly via [`OpenAiCompatProvider::new`] or
/// set the `OPENAI_API_KEY` environment variable.
///
/// ## Example
/// ```no_run
/// use k2_core::llm::{OpenAiCompatProvider, LlmProvider, LlmRequest, LlmMessage};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = OpenAiCompatProvider::new(
///     "https://api.openai.com/v1",
///     "sk-...",
///     "gpt-4o",
/// );
/// let req = LlmRequest::new("gpt-4o", vec![LlmMessage::user("Hello!")]);
/// let resp = provider.chat(req).await?;
/// println!("{}", resp.content.unwrap_or_default());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OpenAiCompatProvider {
    api_key: String,
    base_url: String,
    default_model: String,
    client: reqwest::Client,
}

impl OpenAiCompatProvider {
    /// Create a new OpenAI-compatible provider.
    ///
    /// * `base_url` – root URL (e.g. `https://api.openai.com/v1`).
    ///   The `/chat/completions` path is appended automatically.
    /// * `api_key` – bearer token sent in the `Authorization` header.
    /// * `default_model` – model name used when a request does not specify one.
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        default_model: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            default_model: default_model.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Create a provider reading `OPENAI_API_KEY` from the environment.
    ///
    /// Uses `https://api.openai.com/v1` as the base URL and `gpt-4o`
    /// as the default model.
    pub fn from_env() -> Result<Self, LlmError> {
        let key =
            std::env::var("OPENAI_API_KEY").map_err(|_| LlmError::ApiKeyMissing)?;
        Ok(Self::new("https://api.openai.com/v1", key, "gpt-4o"))
    }

    /// Override the base URL after construction.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Override the default model.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }
}

/// Internal OpenAI-compatible response structures.
#[derive(serde::Deserialize)]
struct ChatCompletionResponse {
    model: Option<String>,
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(serde::Deserialize)]
struct Choice {
    message: ChoiceMessage,
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    async fn chat(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let model = if request.model.is_empty() {
            self.default_model.clone()
        } else {
            request.model.clone()
        };

        let url = format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/')
        );

        let body = serde_json::json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "response_format": request.response_format,
        });

        let http_resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !http_resp.status().is_success() {
            let status = http_resp.status().as_u16();
            let body_text = http_resp.text().await.unwrap_or_default();
            return Err(LlmError::provider(status, body_text));
        }

        let data: ChatCompletionResponse = http_resp.json().await?;

        let first_choice = data
            .choices
            .into_iter()
            .next()
            .ok_or(LlmError::NoContent)?;

        let finish_reason = match first_choice.finish_reason.as_deref() {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("tool_calls") => FinishReason::ToolCalls,
            Some("content_filter") => FinishReason::ContentFilter,
            Some(other) => FinishReason::Unknown(other.to_string()),
            None => FinishReason::Stop,
        };

        Ok(LlmResponse {
            content: first_choice.message.content,
            tool_calls: None,
            finish_reason,
            model: data.model.unwrap_or(model),
            usage: data.usage,
        })
    }

    fn name(&self) -> &str {
        "openai_compat"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructor() {
        let p = OpenAiCompatProvider::new(
            "https://custom.api/v1",
            "sk-test",
            "gpt-3.5-turbo",
        );
        assert_eq!(p.base_url, "https://custom.api/v1");
        assert_eq!(p.default_model, "gpt-3.5-turbo");
        assert_eq!(p.name(), "openai_compat");
    }

    #[test]
    fn test_builder_overrides() {
        let p = OpenAiCompatProvider::new("a", "k", "m")
            .with_base_url("https://api.openai.com/v1")
            .with_default_model("gpt-4o");
        assert_eq!(p.base_url, "https://api.openai.com/v1");
        assert_eq!(p.default_model, "gpt-4o");
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "model": "gpt-4o",
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "Hello! How can I help you?"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 7,
                "total_tokens": 17
            }
        }"#;
        let data: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(data.model.as_deref(), Some("gpt-4o"));
        assert_eq!(
            data.choices[0].message.content.as_deref(),
            Some("Hello! How can I help you?")
        );
        assert_eq!(data.choices[0].finish_reason.as_deref(), Some("stop"));
        assert!(data.usage.is_some());
    }
}
