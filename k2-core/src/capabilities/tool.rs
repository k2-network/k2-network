//! Core tool abstractions — the `K2Tool` trait, IDs, schemas, and results.
//!
//! Every tool in K2 implements [`K2Tool`]. The trait defines how a tool
//! describes itself (schema), what trust it requires, and how it executes.
//!
//! Built-in tools (p2p_send_file, sync_folder, etc.) and WASM-hosted tools
//! both implement this trait so they can coexist in the same [`ToolRegistry`].

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::context::ExecutionContext;
use super::error::CapabilityError;
use super::context::TrustLevel;

/// Unique identifier for a tool in the registry.
///
/// Wraps a [`Uuid`] v4 string for guaranteed uniqueness across nodes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(String);

impl ToolId {
    /// Generate a new random `ToolId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a `ToolId` from a string.
    ///
    /// Useful when registering tools with well-known IDs.
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Return the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ToolId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ToolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ToolId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ToolId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// JSON Schema describing a tool's interface.
///
/// Used for validation and for exposing tool metadata to consumers (e.g.
/// LLM tool-use or remote peers). The `parameters` field holds the JSON
/// Schema object that describes the tool's input shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    /// Human-readable name of the tool.
    pub name: String,
    /// Short description of what the tool does.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub parameters: serde_json::Value,
    /// Minimum trust level required to invoke this tool.
    pub required_trust_level: TrustLevel,
}

impl ToolSchema {
    /// Create a new schema.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        required_trust_level: TrustLevel,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            required_trust_level,
        }
    }
}

/// The result of invoking a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// The tool's output data (JSON).
    pub output: serde_json::Value,
    /// Whether the invocation completed successfully.
    pub success: bool,
    /// Human-readable error message if `success` is `false`.
    pub error: Option<String>,
}

impl ToolResult {
    /// Create a successful result.
    pub fn ok(output: serde_json::Value) -> Self {
        Self {
            output,
            success: true,
            error: None,
        }
    }

    /// Create a failed result.
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            output: serde_json::Value::Null,
            success: false,
            error: Some(message.into()),
        }
    }
}

/// Describes a single tool invocation request.
///
/// Analogous to ironclaw's `CapabilityInvocationRequest`, carrying the tool
/// ID, the JSON input, and the execution context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    /// Which tool to invoke.
    pub tool_id: ToolId,
    /// JSON-encoded input for the tool.
    pub input: serde_json::Value,
    /// Execution context carrying node, session, and trust info.
    pub context: ExecutionContext,
}

impl ToolInvocation {
    /// Create a new invocation request.
    pub fn new(tool_id: ToolId, input: serde_json::Value, context: ExecutionContext) -> Self {
        Self {
            tool_id,
            input,
            context,
        }
    }
}

/// The `K2Tool` trait — every tool must implement this.
///
/// Tools are async, self-describing (via `schema()`), and gated by trust
/// level. The registry stores trait objects of this type.
///
/// # Examples
///
/// ```ignore
/// struct EchoTool;
///
/// #[async_trait]
/// impl K2Tool for EchoTool {
///     fn id(&self) -> &ToolId { ... }
///     fn schema(&self) -> ToolSchema { ... }
///     async fn invoke(&self, input: serde_json::Value, ctx: &ExecutionContext)
///         -> Result<ToolResult, CapabilityError> { ... }
///     fn trust_level(&self) -> TrustLevel { TrustLevel::FirstParty }
/// }
/// ```
#[async_trait]
pub trait K2Tool: Send + Sync {
    /// Return the unique identifier for this tool.
    fn id(&self) -> &ToolId;

    /// Return the JSON Schema describing this tool's interface.
    fn schema(&self) -> ToolSchema;

    /// Invoke the tool with the given input and execution context.
    ///
    /// Implementors should validate `input` against their schema and return
    /// a [`ToolResult`] indicating success or failure.
    async fn invoke(
        &self,
        input: serde_json::Value,
        ctx: &ExecutionContext,
    ) -> Result<ToolResult, CapabilityError>;

    /// The minimum trust level required to call this tool.
    fn trust_level(&self) -> TrustLevel {
        self.schema().required_trust_level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_id_generation() {
        let id1 = ToolId::new();
        let id2 = ToolId::new();
        assert_ne!(id1, id2, "generated IDs must be unique");
    }

    #[test]
    fn tool_id_from_string() {
        let id = ToolId::from_string("p2p.send_file");
        assert_eq!(id.as_str(), "p2p.send_file");
        assert_eq!(id.to_string(), "p2p.send_file");
    }

    #[test]
    fn tool_id_from_trait() {
        let id: ToolId = "my.tool".into();
        assert_eq!(id.as_str(), "my.tool");

        let id: ToolId = String::from("other.tool").into();
        assert_eq!(id.as_str(), "other.tool");
    }

    #[test]
    fn tool_schema_creation() {
        let params = serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        });
        let schema = ToolSchema::new(
            "send_file",
            "Send a file to a peer",
            params.clone(),
            TrustLevel::UserTrusted,
        );
        assert_eq!(schema.name, "send_file");
        assert_eq!(schema.required_trust_level, TrustLevel::UserTrusted);
        assert_eq!(schema.parameters, params);
    }

    #[test]
    fn tool_result_ok() {
        let result = ToolResult::ok(serde_json::json!({"status": "done"}));
        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.output, serde_json::json!({"status": "done"}));
    }

    #[test]
    fn tool_result_err() {
        let result = ToolResult::err("something went wrong");
        assert!(!result.success);
        assert_eq!(result.error, Some("something went wrong".into()));
        assert_eq!(result.output, serde_json::Value::Null);
    }

    #[test]
    fn tool_invocation_creation() {
        let ctx = ExecutionContext::system("node-1", "sess-1");
        let inv = ToolInvocation::new(
            "test.tool".into(),
            serde_json::json!({"key": "value"}),
            ctx.clone(),
        );
        assert_eq!(inv.tool_id.as_str(), "test.tool");
        assert_eq!(inv.input, serde_json::json!({"key": "value"}));
        assert_eq!(inv.context.node_id, "node-1");
    }
}
