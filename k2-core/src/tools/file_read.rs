//! File read tool for reading local files.

use std::path::{Path, PathBuf};

use crate::capabilities::context::{ExecutionContext, TrustLevel};
use crate::capabilities::tool::{ToolId, ToolResult, ToolSchema};
use crate::capabilities::{K2Tool, error::CapabilityError};
use async_trait::async_trait;

/// File read tool - reads contents of local files.
///
/// This tool allows agents to read files from the local filesystem.
/// It enforces path validation to prevent unauthorized access and
/// supports optional size limits to prevent memory exhaustion.
///
/// # Trust level
///
/// Requires `TrustLevel::UserTrusted` or higher - filesystem access
/// requires user trust as it can expose sensitive data.
///
/// # Parameters
///
/// - `path` (string, required): Absolute or relative path to the file
/// - `max_bytes` (number, optional): Maximum bytes to read (default: 1048576)
///
/// # Returns
///
/// A JSON object with:
/// - `success` (boolean): True if file was read successfully
/// - `content` (string): File contents
/// - `size` (number): File size in bytes
/// - `truncated` (boolean): True if content was truncated due to max_bytes
/// - `error` (string, optional): Error message if the read failed
#[derive(Debug, Clone)]
pub struct FileReadTool {
    /// Tool ID
    id: ToolId,
    
    /// Base directory for relative paths (empty = current directory).
    base_dir: PathBuf,

    /// Maximum file size to read in bytes.
    max_bytes: usize,
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileReadTool {
    /// Create a new file read tool with default settings.
    ///
    /// Defaults:
    /// - Base directory: current directory
    /// - Max file size: 1 MiB
    pub fn new() -> Self {
        Self {
            id: ToolId::from_string("file_read"),
            max_bytes: 1024 * 1024, // 1 MiB
            base_dir: PathBuf::new(),
        }
    }

    /// Create a new file read tool with a custom base directory.
    ///
    /// Relative paths will be resolved relative to this directory.
    pub fn with_base_dir(mut self, base_dir: impl AsRef<Path>) -> Self {
        self.base_dir = base_dir.as_ref().to_path_buf();
        self
    }

    /// Create a new file read tool with custom max file size.
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// Validate and resolve the file path.
    ///
    /// This prevents directory traversal attacks by ensuring the resolved
    /// path is within the allowed base directory (if set).
    fn validate_path(&self, path: &str) -> Result<PathBuf, String> {
        let input_path = PathBuf::from(path);

        // Resolve to absolute path
        let resolved = if input_path.is_absolute() {
            input_path
        } else if !self.base_dir.as_os_str().is_empty() {
            self.base_dir.join(&input_path)
        } else {
            std::env::current_dir()
                .unwrap()
                .join(&input_path)
                .canonicalize()
                .map_err(|e| format!("Failed to resolve path: {}", e))?
        };

        // Security check: ensure we're not escaping base directory
        if !self.base_dir.as_os_str().is_empty() {
            let base = self.base_dir.canonicalize()
                .map_err(|e| format!("Invalid base directory: {}", e))?;

            if !resolved.starts_with(&base) {
                return Err(format!(
                    "Path {} escapes allowed base directory {}",
                    path,
                    base.display()
                ));
            }
        }

        Ok(resolved)
    }
}

#[async_trait]
impl K2Tool for FileReadTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            "File Read",
            "Read the contents of a local file. Requires absolute or relative path.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    },
                    "max_bytes": {
                        "type": "number",
                        "description": "Maximum bytes to read (default: 1048576)"
                    }
                },
                "required": ["path"]
            }),
            TrustLevel::UserTrusted,
        )
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        _context: &ExecutionContext,
    ) -> Result<ToolResult, CapabilityError> {
        // Extract parameters
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CapabilityError::Internal("Missing required parameter: path".to_string()))?;

        let max_bytes = input
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.max_bytes as u64) as usize;

        // Validate and resolve path
        let resolved_path = self
            .validate_path(path)
            .map_err(|e| CapabilityError::Internal(e))?;

        // Read file
        let metadata = tokio::fs::metadata(&resolved_path)
            .await
            .map_err(|e| CapabilityError::Internal(format!("Failed to access file: {}", e)))?;

        if !metadata.is_file() {
            return Ok(ToolResult::ok(serde_json::json!({
                "success": false,
                "error": format!("Path is not a file: {}", path)
            })));
        }

        let file_size = metadata.len() as usize;
        let bytes_to_read = file_size.min(max_bytes);

        let content = tokio::fs::read(&resolved_path)
            .await
            .map_err(|e| CapabilityError::Internal(format!("Failed to read file: {}", e)))?;

        let truncated = bytes_to_read < file_size;
        let content_string = String::from_utf8_lossy(&content[..bytes_to_read]).to_string();

        Ok(ToolResult::ok(serde_json::json!({
            "success": true,
            "content": content_string,
            "size": file_size,
            "truncated": truncated
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_tool_id() {
        let tool = FileReadTool::new();
        assert_eq!(tool.id().as_str(), "file_read");
    }

    #[test]
    fn test_tool_schema() {
        let tool = FileReadTool::new();
        let schema = tool.schema();
        assert_eq!(schema.name, "File Read");
        assert!(!schema.description.is_empty());
        assert_eq!(schema.required_trust_level, TrustLevel::UserTrusted);
    }

    #[tokio::test]
    async fn test_invoke_missing_path() {
        let tool = FileReadTool::new();
        let context = ExecutionContext::new(
            "node-1".to_string(), 
            "session-1".to_string(),
            TrustLevel::UserTrusted,
            None,
        );
        let input = serde_json::json!({});

        let result = tool.invoke(input, &context).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("path"));
    }

    #[tokio::test]
    async fn test_invoke_read_file() {
        let tool = FileReadTool::new();
        let context = ExecutionContext::new(
            "node-1".to_string(),
            "session-1".to_string(),
            TrustLevel::UserTrusted,
            None,
        );

        // Create a temporary file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Hello, world!").unwrap();
        let path = temp_file.path().to_str().unwrap();

        let input = serde_json::json!({
            "path": path
        });

        let result = tool.invoke(input, &context).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.output["success"], true);
        assert!(output.output["content"].as_str().unwrap().contains("Hello, world!"));
        assert_eq!(output.output["size"], 14);
    }
}
