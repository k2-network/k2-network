use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::error::LlmError;
use super::provider::{
    FinishReason, LlmMessage, LlmProvider, LlmRequest, LlmResponse, ToolCall, Usage,
};

const DEFAULT_GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
const DEFAULT_GROQ_MODEL: &str = "llama-3.3-70b-versatile";

/// The Vietnamese marketplace-analysis system prompt used by K2.
///
/// Instructs the model to classify buying/selling intent into structured JSON.
pub const K2_MARKETPLACE_SYSTEM_PROMPT: &str = r#"Bạn là AI phân tích yêu cầu mua bán trên K2 Marketplace. Phân tích ý định của người dùng và trích xuất thông tin.

Các topic:
- "Digital Assets": Video, Images, Audio, Token, License | Key | Secret, Document, Source Code, Dataset
- "Goods": Fashion, Electronics & Devices, Books & Learning, Sports & Travel  
- "Freelance Job": Tech & IT, Design & Creative, Writing & Translation, Marketing & Sales

Các action:
- "buy": Người dùng muốn MUA
- "sell": Người dùng muốn BÁN
- "exchange": Người dùng muốn TRAO ĐỔI

Trả về JSON theo format:
{
  "topic": "Digital Assets" | "Goods" | "Freelance Job",
  "selection": { "subtopic": "..." } hoặc { "category": "...", "skill": "..." },
  "action": "buy" | "sell" | "exchange",
  "description": "mô tả yêu cầu"
}"#;

/// Provider for [Groq](https://groq.com) — the LPU inference cloud.
///
/// ## Authentication
/// Set `GROQ_API_KEY` in the environment, or pass the key to
/// [`GroqProvider::new`].
///
/// ## Example
/// ```no_run
/// use k2_core::llm::{GroqProvider, LlmProvider, LlmRequest, LlmMessage};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = GroqProvider::from_env()?;
/// let req = LlmRequest::new("llama-3.3-70b-versatile", vec![
///     LlmMessage::user("Xin chào!"),
/// ]);
/// let resp = provider.chat(req).await?;
/// println!("{}", resp.content.unwrap_or_default());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GroqProvider {
    api_key: String,
    base_url: String,
    default_model: String,
    client: reqwest::Client,
    system_prompt: String,
}

impl GroqProvider {
    /// Create a new Groq provider with an explicit API key.
    ///
    /// Uses `https://api.groq.com/openai/v1` as the base URL and
    /// `llama-3.3-70b-versatile` as the default model.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_GROQ_BASE_URL.to_string(),
            default_model: DEFAULT_GROQ_MODEL.to_string(),
            client: reqwest::Client::new(),
            system_prompt: K2_MARKETPLACE_SYSTEM_PROMPT.to_string(),
        }
    }

    /// Create a Groq provider reading `GROQ_API_KEY` from the environment.
    pub fn from_env() -> Result<Self, LlmError> {
        let key = std::env::var("GROQ_API_KEY").map_err(|_| LlmError::ApiKeyMissing)?;
        Ok(Self::new(key))
    }

    /// Override the base URL (e.g. for proxies or self-hosted gateways).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Override the default model used when a request does not specify one.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Replace the system prompt prepended to every request.
    ///
    /// Defaults to [`K2_MARKETPLACE_SYSTEM_PROMPT`] (Vietnamese marketplace classifier).
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Build the list of messages, inserting the system prompt first.
    fn build_messages(&self, request: &LlmRequest) -> Vec<LlmMessage> {
        let has_system = request
            .messages
            .iter()
            .any(|m| m.role == super::provider::MessageRole::System);

        if has_system || self.system_prompt.is_empty() {
            request.messages.clone()
        } else {
            let mut msgs = vec![LlmMessage::system(&self.system_prompt)];
            msgs.extend(request.messages.clone());
            msgs
        }
    }
}

// ---- OpenAI-compatible wire types (not part of the public API) ----

#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: &'a [LlmMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<super::provider::ResponseFormat>,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    model: Option<String>,
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

#[async_trait]
impl LlmProvider for GroqProvider {
    async fn chat(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let model = if request.model.is_empty() {
            self.default_model.clone()
        } else {
            request.model.clone()
        };

        let messages = self.build_messages(&request);

        let body = ChatCompletionRequest {
            model: &model,
            messages: &messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            response_format: request.response_format,
        };

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

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

        let first_choice = data.choices.into_iter().next().ok_or(LlmError::NoContent)?;

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
            tool_calls: first_choice.message.tool_calls,
            finish_reason,
            model: data.model.unwrap_or(model),
            usage: data.usage,
        })
    }

    fn name(&self) -> &str {
        "groq"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::provider::{LlmMessage, MessageRole};

    fn make_provider() -> GroqProvider {
        GroqProvider::new("test-key")
    }

    #[test]
    fn test_build_messages_prepends_system_prompt() {
        let p = make_provider();
        let req = LlmRequest::new("m", vec![LlmMessage::user("hi")]);
        let msgs = p.build_messages(&req);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, MessageRole::System);
        assert!(msgs[0].content.contains("K2 Marketplace"));
        assert_eq!(msgs[1].role, MessageRole::User);
        assert_eq!(msgs[1].content, "hi");
    }

    #[test]
    fn test_build_messages_respects_existing_system() {
        let p = make_provider();
        let req = LlmRequest::new(
            "m",
            vec![
                LlmMessage::system("custom system"),
                LlmMessage::user("hi"),
            ],
        );
        let msgs = p.build_messages(&req);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "custom system");
        // Should NOT prepend the default system prompt
    }

    #[test]
    fn test_build_messages_empty_prompt_no_prepend() {
        let p = make_provider().with_system_prompt("");
        let req = LlmRequest::new("m", vec![LlmMessage::user("hi")]);
        let msgs = p.build_messages(&req);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, MessageRole::User);
    }

    #[test]
    fn test_empty_model_uses_default() {
        let p = make_provider();
        assert_eq!(p.default_model, DEFAULT_GROQ_MODEL);
    }

    #[test]
    fn test_custom_base_url() {
        let p = make_provider().with_base_url("https://proxy.example.com/v1");
        assert_eq!(p.base_url, "https://proxy.example.com/v1");
    }

    #[test]
    fn test_custom_model() {
        let p = make_provider().with_default_model("mixtral-8x7b");
        assert_eq!(p.default_model, "mixtral-8x7b");
    }

    #[test]
    fn test_name() {
        assert_eq!(make_provider().name(), "groq");
    }

    #[test]
    fn test_chat_request_serialization() {
        let messages = vec![
            LlmMessage::system("You are a helpful assistant."),
            LlmMessage::user("Hello!"),
        ];
        let body = ChatCompletionRequest {
            model: "test-model",
            messages: &messages,
            temperature: None,
            max_tokens: None,
            response_format: None,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("test-model"));
        assert!(json.contains("You are a helpful assistant."));
        assert!(json.contains("Hello!"));
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "model": "llama-3.3-70b-versatile",
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "Xin chào!"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 42,
                "completion_tokens": 5,
                "total_tokens": 47
            }
        }"#;
        let data: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(data.model.as_deref(), Some("llama-3.3-70b-versatile"));
        assert_eq!(data.choices[0].message.content.as_deref(), Some("Xin chào!"));
        assert_eq!(data.choices[0].finish_reason.as_deref(), Some("stop"));
        assert!(data.usage.is_some());
    }

    #[test]
    fn test_response_deserialization_tool_calls() {
        let json = r#"{
            "model": "gpt-4",
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [
                            {
                                "id": "call_abc",
                                "type": "function",
                                "function": {
                                    "name": "get_weather",
                                    "arguments": "{\"city\":\"Hanoi\"}"
                                }
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }
            ]
        }"#;
        let data: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        let tc = &data.choices[0].message.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.id, "call_abc");
        assert_eq!(tc.function.name, "get_weather");
    }
}
