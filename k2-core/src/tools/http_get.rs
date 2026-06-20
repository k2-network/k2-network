//! HTTP GET tool for fetching web resources.

use std::time::Duration;

use crate::capabilities::context::{ExecutionContext, TrustLevel};
use crate::capabilities::tool::{ToolId, ToolResult, ToolSchema};
use crate::capabilities::{K2Tool, error::CapabilityError};
use async_trait::async_trait;

/// HTTP GET tool - fetches resources from the web.
///
/// This tool allows agents to make HTTP GET requests to fetch web pages,
/// APIs, or other HTTP resources. It has a built-in timeout and enforces
/// a maximum response size to prevent memory exhaustion.
///
/// # Trust level
///
/// Requires `TrustLevel::Sandbox` or higher - this tool is relatively safe
/// as it only performs network I/O and does not access the local filesystem.
///
/// # Parameters
///
/// - `url` (string, required): The URL to fetch
/// - `timeout_ms` (number, optional): Request timeout in milliseconds (default: 10000)
/// - `max_bytes` (number, optional): Maximum response size in bytes (default: 1048576)
///
/// # Returns
///
/// A JSON object with:
/// - `status_code` (number): HTTP status code
/// - `headers` (object): Response headers
/// - `body` (string): Response body (truncated if exceeds max_bytes)
/// - `success` (boolean): True if status code is 2xx
/// - `error` (string, optional): Error message if the request failed
#[derive(Debug, Clone)]
pub struct HttpGetTool {
    /// Tool ID
    id: ToolId,
    
    /// Default timeout for HTTP requests.
    timeout: Duration,

    /// Maximum response size in bytes.
    max_bytes: usize,
}

impl Default for HttpGetTool {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpGetTool {
    /// Create a new HTTP GET tool with default settings.
    ///
    /// Defaults:
    /// - Timeout: 10 seconds
    /// - Max response size: 1 MiB
    pub fn new() -> Self {
        Self {
            id: ToolId::from_string("http_get"),
            timeout: Duration::from_secs(10),
            max_bytes: 1024 * 1024, // 1 MiB
        }
    }

    /// Create a new HTTP GET tool with custom timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Create a new HTTP GET tool with custom max response size.
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }
}

#[async_trait]
impl K2Tool for HttpGetTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            "HTTP GET",
            "Fetch a resource from the web via HTTP GET. Returns status code, headers, and body.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    },
                    "timeout_ms": {
                        "type": "number",
                        "description": "Request timeout in milliseconds (default: 10000)"
                    },
                    "max_bytes": {
                        "type": "number",
                        "description": "Maximum response size in bytes (default: 1048576)"
                    }
                },
                "required": ["url"]
            }),
            TrustLevel::Sandbox,
        )
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        _context: &ExecutionContext,
    ) -> Result<ToolResult, CapabilityError> {
        // Extract parameters
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CapabilityError::Internal("Missing required parameter: url".to_string()))?;

        let timeout_ms = input
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(10000);

        let max_bytes = input
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.max_bytes as u64) as usize;

        // Build HTTP client with timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| CapabilityError::Internal(format!("Failed to build HTTP client: {}", e)))?;

        // Make the request
        let response = match client.get(url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                return Ok(ToolResult::ok(serde_json::json!({
                    "success": false,
                    "error": format!("HTTP request failed: {}", e)
                })))
            }
        };

        let status_code = response.status().as_u16();

        // Collect headers
        let headers: std::collections::HashMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|vv| (k.as_str().to_string(), vv.to_string())))
            .collect();

        // Read body with size limit
        let body_bytes = match response.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                return Ok(ToolResult::ok(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to read response body: {}", e)
                })))
            }
        };

        let truncated = body_bytes.len() > max_bytes;
        let body = String::from_utf8_lossy(&body_bytes[..body_bytes.len().min(max_bytes)]).to_string();

        Ok(ToolResult::ok(serde_json::json!({
            "success": (200..300).contains(&status_code),
            "status_code": status_code,
            "headers": headers,
            "body": body,
            "truncated": truncated,
            "size": body_bytes.len()
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_id() {
        let tool = HttpGetTool::new();
        assert_eq!(tool.id().as_str(), "http_get");
    }

    #[test]
    fn test_tool_schema() {
        let tool = HttpGetTool::new();
        let schema = tool.schema();
        assert_eq!(schema.name, "HTTP GET");
        assert!(!schema.description.is_empty());
        assert_eq!(schema.required_trust_level, TrustLevel::Sandbox);
    }

    #[tokio::test]
    async fn test_invoke_missing_url() {
        let tool = HttpGetTool::new();
        let context = ExecutionContext::new(
            "node-1".to_string(),
            "session-1".to_string(),
            TrustLevel::Sandbox,
            None,
        );
        let input = serde_json::json!({});

        let result = tool.invoke(input, &context).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("url"));
    }
}
