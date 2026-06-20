use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::error::LlmError;

/// Role of a message in an LLM conversation.
///
/// Maps to the standard OpenAI chat roles with custom serde serialization
/// to their lowercase wire format.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MessageRole {
    /// System-level instruction (sets behaviour / context).
    System,
    /// End-user input.
    User,
    /// Model response.
    Assistant,
    /// Result of a tool call previously requested by the model.
    Tool,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "system" => Ok(MessageRole::System),
            "user" => Ok(MessageRole::User),
            "assistant" => Ok(MessageRole::Assistant),
            "tool" => Ok(MessageRole::Tool),
            other => Err(format!("unknown message role: {}", other)),
        }
    }
}

impl Serialize for MessageRole {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MessageRole {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        MessageRole::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool(content: impl Into<String>, call_id: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(call_id.into()),
        }
    }
}

/// A tool/function call requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

/// The function details within a [`ToolCall`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Schema for a tool/function that can be called by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub function: FunctionSchema,
}

/// Function definition within a [`ToolSchema`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// Natural stop or stop token hit.
    Stop,
    /// Token limit reached.
    Length,
    /// Model requested tool calls.
    ToolCalls,
    /// Content filtered by the provider's safety system.
    ContentFilter,
    /// Unknown reason (string preserved for forward compatibility).
    Unknown(String),
}

impl Serialize for FinishReason {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            FinishReason::Stop => serializer.serialize_str("stop"),
            FinishReason::Length => serializer.serialize_str("length"),
            FinishReason::ToolCalls => serializer.serialize_str("tool_calls"),
            FinishReason::ContentFilter => serializer.serialize_str("content_filter"),
            FinishReason::Unknown(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for FinishReason {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "stop" => Ok(FinishReason::Stop),
            "length" => Ok(FinishReason::Length),
            "tool_calls" => Ok(FinishReason::ToolCalls),
            "content_filter" => Ok(FinishReason::ContentFilter),
            other => Ok(FinishReason::Unknown(other.to_string())),
        }
    }
}

/// Controls the output format requested from the provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
}

impl ResponseFormat {
    pub fn json_object() -> Self {
        Self {
            format_type: "json_object".to_string(),
        }
    }

    pub fn text() -> Self {
        Self {
            format_type: "text".to_string(),
        }
    }
}

/// Token usage statistics returned by the provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// A complete request to an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub messages: Vec<LlmMessage>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(skip)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl LlmRequest {
    /// Create a new request with the given model and messages.
    pub fn new(model: impl Into<String>, messages: Vec<LlmMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            max_tokens: None,
            tools: None,
            response_format: None,
            extra: HashMap::new(),
        }
    }

    /// Set the sampling temperature (0.0–2.0).
    pub fn with_temperature(mut self, t: f32) -> Self {
        self.temperature = Some(t);
        self
    }

    /// Cap the maximum response tokens.
    pub fn with_max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = Some(n);
        self
    }

    /// Attach tool definitions the model may call.
    pub fn with_tools(mut self, tools: Vec<ToolSchema>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Request a specific response format (e.g. JSON).
    pub fn with_response_format(mut self, fmt: ResponseFormat) -> Self {
        self.response_format = Some(fmt);
        self
    }
}

/// A complete response from an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: FinishReason,
    pub model: String,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// Abstract LLM provider trait.
///
/// Every backend (Groq, OpenAI, Ollama, etc.) implements this trait so
/// callers can switch providers without changing their logic.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a chat request and receive the model's reply.
    async fn chat(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;

    /// List available model names.
    ///
    /// Default implementation returns [`LlmError::NotSupported`].
    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        Err(LlmError::NotSupported("list_models".to_string()))
    }

    /// Human-readable name of this provider (e.g. `"groq"`, `"ollama"`).
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_role_serde() {
        let cases = vec![
            (MessageRole::System, "\"system\""),
            (MessageRole::User, "\"user\""),
            (MessageRole::Assistant, "\"assistant\""),
            (MessageRole::Tool, "\"tool\""),
        ];
        for (role, expected) in cases {
            let json = serde_json::to_string(&role).unwrap();
            assert_eq!(json, expected);
            let roundtrip: MessageRole = serde_json::from_str(&json).unwrap();
            assert_eq!(roundtrip, role);
        }
    }

    #[test]
    fn test_message_role_invalid_deser() {
        let result: Result<MessageRole, _> = serde_json::from_str("\"invalid\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_finish_reason_serde() {
        let cases = vec![
            (FinishReason::Stop, "\"stop\""),
            (FinishReason::Length, "\"length\""),
            (FinishReason::ToolCalls, "\"tool_calls\""),
            (FinishReason::ContentFilter, "\"content_filter\""),
        ];
        for (reason, expected) in cases {
            let json = serde_json::to_string(&reason).unwrap();
            assert_eq!(json, expected);
            let roundtrip: FinishReason = serde_json::from_str(&json).unwrap();
            assert_eq!(roundtrip, reason);
        }
    }

    #[test]
    fn test_finish_reason_unknown() {
        let json = "\"some_new_reason\"";
        let reason: FinishReason = serde_json::from_str(json).unwrap();
        assert_eq!(reason, FinishReason::Unknown("some_new_reason".to_string()));
        // Round-trip preserves the string
        let back = serde_json::to_string(&reason).unwrap();
        assert_eq!(back, json);
    }

    #[test]
    fn test_llm_message_constructors() {
        let sys = LlmMessage::system("You are helpful.");
        assert_eq!(sys.role, MessageRole::System);
        assert_eq!(sys.content, "You are helpful.");

        let usr = LlmMessage::user("Hello");
        assert_eq!(usr.role, MessageRole::User);

        let ast = LlmMessage::assistant("Hi there!");
        assert_eq!(ast.role, MessageRole::Assistant);

        let tool = LlmMessage::tool("result", "call_123");
        assert_eq!(tool.role, MessageRole::Tool);
        assert_eq!(tool.tool_call_id, Some("call_123".to_string()));
    }

    #[test]
    fn test_llm_message_serde_roundtrip() {
        let msg = LlmMessage {
            role: MessageRole::Assistant,
            content: "Hello!".to_string(),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                call_type: "function".to_string(),
                function: ToolCallFunction {
                    name: "get_weather".to_string(),
                    arguments: "{\"city\":\"Hanoi\"}".to_string(),
                },
            }]),
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let roundtrip: LlmMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.role, msg.role);
        assert_eq!(roundtrip.content, msg.content);
        assert!(roundtrip.tool_calls.is_some());
    }

    #[test]
    fn test_llm_request_builder() {
        let req = LlmRequest::new("gpt-4", vec![LlmMessage::user("ping")])
            .with_temperature(0.7)
            .with_max_tokens(256)
            .with_response_format(ResponseFormat::json_object());

        assert_eq!(req.model, "gpt-4");
        assert_eq!(req.temperature, Some(0.7));
        assert_eq!(req.max_tokens, Some(256));
        assert!(req.response_format.is_some());
    }

    #[test]
    fn test_llm_request_serde() {
        let req = LlmRequest::new(
            "test-model",
            vec![
                LlmMessage::system("Be concise."),
                LlmMessage::user("What is Rust?"),
            ],
        )
        .with_temperature(0.3);

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("test-model"));
        assert!(json.contains("Be concise."));
        assert!(json.contains("What is Rust?"));
        assert!(json.contains("0.3"));

        let roundtrip: LlmRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.model, "test-model");
        assert_eq!(roundtrip.messages.len(), 2);
    }

    #[test]
    fn test_llm_response_serde() {
        let resp = LlmResponse {
            content: Some("Rust is a systems programming language.".to_string()),
            tool_calls: None,
            finish_reason: FinishReason::Stop,
            model: "gpt-4".to_string(),
            usage: Some(Usage {
                prompt_tokens: 20,
                completion_tokens: 8,
                total_tokens: 28,
            }),
        };

        let json = serde_json::to_string(&resp).unwrap();
        let roundtrip: LlmResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.content, resp.content);
        assert_eq!(roundtrip.finish_reason, FinishReason::Stop);
        assert_eq!(roundtrip.model, "gpt-4");
        assert!(roundtrip.usage.is_some());
    }

    #[test]
    fn test_tool_schema_serde() {
        let schema = ToolSchema {
            schema_type: "function".to_string(),
            function: FunctionSchema {
                name: "get_weather".to_string(),
                description: "Get current weather".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    }
                }),
            },
        };
        let json = serde_json::to_string(&schema).unwrap();
        let roundtrip: ToolSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.function.name, "get_weather");
    }

    #[test]
    fn test_response_format_helpers() {
        let json_fmt = ResponseFormat::json_object();
        assert_eq!(json_fmt.format_type, "json_object");

        let text_fmt = ResponseFormat::text();
        assert_eq!(text_fmt.format_type, "text");
    }
}
