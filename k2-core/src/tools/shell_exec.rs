//! Shell execution tool for running commands.

use crate::capabilities::context::{ExecutionContext, TrustLevel};
use crate::capabilities::tool::{ToolId, ToolResult, ToolSchema};
use crate::capabilities::{K2Tool, error::CapabilityError};
use async_trait::async_trait;

/// Shell execution tool - runs shell commands.
///
/// This tool allows agents to execute arbitrary shell commands. It is
/// inherently dangerous and therefore requires the highest trust level.
/// Commands run with a timeout to prevent hanging.
///
/// # Trust level
///
/// Requires `TrustLevel::System` - this is the most dangerous tool as it
/// allows arbitrary code execution. Only system processes should be allowed
/// to use this tool.
///
/// # Parameters
///
/// - `command` (string, required): Shell command to execute
/// - `timeout_ms` (number, optional): Command timeout in milliseconds (default: 30000)
/// - `working_dir` (string, optional): Working directory for the command
///
/// # Returns
///
/// A JSON object with:
/// - `success` (boolean): True if command exited with code 0
/// - `exit_code` (number): Exit code of the command
/// - `stdout` (string): Standard output
/// - `stderr` (string): Standard error output
/// - `timed_out` (boolean): True if command was killed due to timeout
/// - `error` (string, optional): Error message if command failed to execute
#[derive(Debug, Clone)]
pub struct ShellExecTool {
    /// Tool ID
    id: ToolId,
    
    /// Default timeout for command execution.
    timeout: std::time::Duration,
}

impl Default for ShellExecTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellExecTool {
    /// Create a new shell execution tool with default settings.
    ///
    /// Default timeout: 30 seconds.
    pub fn new() -> Self {
        Self {
            id: ToolId::from_string("shell_exec"),
            timeout: std::time::Duration::from_secs(30),
        }
    }

    /// Create a new shell execution tool with custom timeout.
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl K2Tool for ShellExecTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            "Shell Execute",
            "Execute a shell command with a timeout. Requires System trust level.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    },
                    "timeout_ms": {
                        "type": "number",
                        "description": "Command timeout in milliseconds (default: 30000)"
                    },
                    "working_dir": {
                        "type": "string",
                        "description": "Working directory for the command"
                    }
                },
                "required": ["command"]
            }),
            TrustLevel::System,
        )
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        _context: &ExecutionContext,
    ) -> Result<ToolResult, CapabilityError> {
        // Extract parameters
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CapabilityError::Internal("Missing required parameter: command".to_string()))?;

        let timeout_ms = input
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.timeout.as_millis() as u64);

        let working_dir = input.get("working_dir").and_then(|v| v.as_str());

        // Build command
        let mut cmd = if cfg!(windows) {
            let mut cmd = std::process::Command::new("cmd");
            cmd.args(["/C", command]);
            cmd
        } else {
            let mut cmd = std::process::Command::new("sh");
            cmd.args(["-c", command]);
            cmd
        };

        // Set working directory if provided
        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        // Spawn command with timeout
        let timeout_duration = std::time::Duration::from_millis(timeout_ms);

        // Use std::process::Command then convert to tokio
        let std_cmd = cmd;
        let output = match tokio::time::timeout(
            timeout_duration,
            tokio::process::Command::from(std_cmd).output()
        ).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Ok(ToolResult::ok(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to execute command: {}", e)
                })))
            }
            Err(_) => {
                return Ok(ToolResult::ok(serde_json::json!({
                    "success": false,
                    "timed_out": true,
                    "error": format!("Command timed out after {}ms", timeout_ms)
                })))
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(ToolResult::ok(serde_json::json!({
            "success": exit_code == 0,
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
            "timed_out": false
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_id() {
        let tool = ShellExecTool::new();
        assert_eq!(tool.id().as_str(), "shell_exec");
    }

    #[test]
    fn test_tool_schema() {
        let tool = ShellExecTool::new();
        let schema = tool.schema();
        assert_eq!(schema.name, "Shell Execute");
        assert!(!schema.description.is_empty());
        assert_eq!(schema.required_trust_level, TrustLevel::System);
    }

    #[tokio::test]
    async fn test_invoke_missing_command() {
        let tool = ShellExecTool::new();
        let context = ExecutionContext::new(
            "node-1".to_string(),
            "session-1".to_string(),
            TrustLevel::System,
            None,
        );
        let input = serde_json::json!({});

        let result = tool.invoke(input, &context).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("command"));
    }

    #[tokio::test]
    async fn test_invoke_echo_command() {
        let tool = ShellExecTool::new().with_timeout(std::time::Duration::from_secs(5));
        let context = ExecutionContext::new(
            "node-1".to_string(),
            "session-1".to_string(),
            TrustLevel::System,
            None,
        );

        let command = if cfg!(windows) { "echo hello" } else { "echo hello" };

        let input = serde_json::json!({
            "command": command
        });

        let result = tool.invoke(input, &context).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.output["success"], true);
        assert_eq!(output.output["exit_code"], 0);
        assert!(output.output["stdout"].as_str().unwrap().contains("hello"));
    }
}
