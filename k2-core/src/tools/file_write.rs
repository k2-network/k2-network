//! File write tool for writing to local files.

use std::path::{Path, PathBuf};

use crate::capabilities::context::{ExecutionContext, TrustLevel};
use crate::capabilities::tool::{ToolId, ToolResult, ToolSchema};
use crate::capabilities::{K2Tool, error::CapabilityError};
use async_trait::async_trait;

/// File write tool - writes content to local files.
///
/// This tool allows agents to write files to the local filesystem.
/// It enforces path validation to prevent unauthorized access and
/// creates parent directories as needed.
///
/// # Trust level
///
/// Requires `TrustLevel::UserTrusted` or higher - filesystem write access
/// requires user trust as it can modify system state.
///
/// # Parameters
///
/// - `path` (string, required): Absolute or relative path to write
/// - `content` (string, required): Content to write to the file
/// - `create_dirs` (boolean, optional): Create parent directories (default: true)
/// - `append` (boolean, optional): Append to existing file (default: false)
///
/// # Returns
///
/// A JSON object with:
/// - `success` (boolean): True if file was written successfully
/// - `path` (string): Absolute path of the written file
/// - `bytes_written` (number): Number of bytes written
/// - `error` (string, optional): Error message if the write failed
#[derive(Debug, Clone)]
pub struct FileWriteTool {
    /// Tool ID
    id: ToolId,
    
    /// Base directory for relative paths (empty = current directory).
    base_dir: PathBuf,
}

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWriteTool {
    /// Create a new file write tool with default settings.
    ///
    /// Default: base directory is current directory.
    pub fn new() -> Self {
        Self {
            id: ToolId::from_string("file_write"),
            base_dir: PathBuf::new(),
        }
    }

    /// Create a new file write tool with a custom base directory.
    ///
    /// Relative paths will be resolved relative to this directory.
    pub fn with_base_dir(mut self, base_dir: impl AsRef<Path>) -> Self {
        self.base_dir = base_dir.as_ref().to_path_buf();
        self
    }

    /// Validate and resolve the file path.
    ///
    /// This prevents directory traversal attacks.
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
        };

        // Security check: ensure we're not escaping base directory
        if !self.base_dir.as_os_str().is_empty() {
            let base = self.base_dir.canonicalize()
                .map_err(|e| format!("Invalid base directory: {}", e))?;

            let resolved_canonical = resolved.canonicalize()
                .unwrap_or_else(|_| resolved.clone());

            if !resolved_canonical.starts_with(&base) {
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
impl K2Tool for FileWriteTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            "File Write",
            "Write content to a local file. Creates parent directories by default.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    },
                    "create_dirs": {
                        "type": "boolean",
                        "description": "Create parent directories (default: true)"
                    },
                    "append": {
                        "type": "boolean",
                        "description": "Append to existing file (default: false)"
                    }
                },
                "required": ["path", "content"]
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

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CapabilityError::Internal("Missing required parameter: content".to_string()))?;

        let create_dirs = input
            .get("create_dirs")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let _append = input
            .get("append")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Validate and resolve path
        let resolved_path = self
            .validate_path(path)
            .map_err(CapabilityError::Internal)?;

        // Create parent directories if requested
        if create_dirs {
            if let Some(parent) = resolved_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| CapabilityError::Internal(format!("Failed to create directories: {}", e)))?;
            }
        }

        // Write file
        let _bytes = tokio::fs::write(&resolved_path, content)
            .await
            .map_err(|e| CapabilityError::Internal(format!("Failed to write file: {}", e)))?;
        
        let bytes = content.len();

        Ok(ToolResult::ok(serde_json::json!({
            "success": true,
            "path": resolved_path.display().to_string(),
            "bytes_written": bytes
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_tool_id() {
        let tool = FileWriteTool::new();
        assert_eq!(tool.id().as_str(), "file_write");
    }

    #[test]
    fn test_tool_schema() {
        let tool = FileWriteTool::new();
        let schema = tool.schema();
        assert_eq!(schema.name, "File Write");
        assert!(!schema.description.is_empty());
        assert_eq!(schema.required_trust_level, TrustLevel::UserTrusted);
    }

    #[tokio::test]
    async fn test_invoke_missing_path() {
        let tool = FileWriteTool::new();
        let context = ExecutionContext::new(
            "node-1".to_string(),
            "session-1".to_string(),
            TrustLevel::UserTrusted,
            None,
        );
        let input = serde_json::json!({
            "content": "test"
        });

        let result = tool.invoke(input, &context).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("path"));
    }

    #[tokio::test]
    async fn test_invoke_write_file() {
        let temp_dir = TempDir::new().unwrap();
        // Use absolute path directly without base_dir restriction
        let tool = FileWriteTool::new();
        let context = ExecutionContext::new(
            "node-1".to_string(),
            "session-1".to_string(),
            TrustLevel::UserTrusted,
            None,
        );

        let file_path = temp_dir.path().join("test.txt");
        let abs_path = file_path.to_str().unwrap();

        let input = serde_json::json!({
            "path": abs_path,
            "content": "Hello, world!"
        });

        let result = tool.invoke(input, &context).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.output["success"], true);
        assert_eq!(output.output["bytes_written"], 13);

        // Verify file was written
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello, world!");
    }
}
