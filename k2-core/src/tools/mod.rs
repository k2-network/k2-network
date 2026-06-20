//! Built-in tools for K2 agents.
//!
//! This module provides concrete implementations of the [`K2Tool`] trait
//! for common operations that agents may need to perform:
//!
//! - `file_read` - Read file contents with size limits
//! - `file_write` - Write content to files with path validation
//! - `shell_exec` - Execute shell commands (high trust required)
//! - `http_get` - Fetch HTTP resources with timeout
//!
//! # Trust levels
//!
//! Different tools have different trust requirements:
//! - `Sandbox` tools: `http_get` (network I/O only)
//! - `UserTrusted` tools: `file_read`, `file_write` (filesystem access)
//! - `System` tools: `shell_exec` (arbitrary code execution)
//!
//! # Example
//!
//! ```ignore
//! use k2_core::tools::FileReadTool;
//! use k2_core::capabilities::ToolRegistry;
//!
//! let mut registry = ToolRegistry::new();
//! registry.register(FileReadTool::tool_id(), Arc::new(FileReadTool::new()))?;
//! ```

pub mod file_read;
pub mod file_write;
pub mod http_get;
pub mod shell_exec;

pub use file_read::FileReadTool;
pub use file_write::FileWriteTool;
pub use http_get::HttpGetTool;
pub use shell_exec::ShellExecTool;

/// Register all built-in tools with the given registry.
///
/// This is a convenience function to register all standard K2 tools
/// at once. Tools are registered with their default trust levels.
///
/// # Example
///
/// ```ignore
/// use k2_core::tools::register_all;
/// use k2_core::capabilities::ToolRegistry;
///
/// let mut registry = ToolRegistry::new();
/// register_all(&mut registry)?;
/// ```
pub fn register_all(registry: &mut crate::capabilities::ToolRegistry) -> Result<(), Box<dyn std::error::Error>> {
    registry.register(Box::new(FileReadTool::new()))?;
    registry.register(Box::new(FileWriteTool::new()))?;
    registry.register(Box::new(HttpGetTool::new()))?;
    registry.register(Box::new(ShellExecTool::new()))?;

    Ok(())
}