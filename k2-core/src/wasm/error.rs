//! Error types for the WASM sandbox module.
//!
//! Defines [`WasmError`] for runtime/load failures and [`WasmHostError`]
//! for host-function-specific failures. Both use `thiserror` following
//! K2's convention of typed, descriptive error variants.

use thiserror::Error;

/// Errors that can occur when loading or executing a WASM plugin.
#[derive(Error, Debug)]
pub enum WasmError {
    /// The WASM plugin could not be loaded (e.g. invalid binary, I/O error).
    #[error("load failed: {0}")]
    LoadFailed(String),

    /// A plugin function invocation returned an error.
    #[error("invocation failed: {0}")]
    InvocationFailed(String),

    /// An operation was attempted on a runtime with no plugin loaded.
    #[error("plugin not loaded")]
    PluginNotLoaded,

    /// A host function call failed.
    #[error("host function error: {0}")]
    HostFunctionError(#[from] WasmHostError),

    /// The runtime configuration is invalid.
    #[error("config error: {0}")]
    ConfigError(String),

    /// Plugin execution exceeded the configured timeout.
    #[error("execution timeout after {0}s")]
    Timeout(u64),

    /// Plugin exhausted its fuel (instruction count) budget.
    #[error("out of fuel")]
    OutOfFuel,

    /// Plugin exceeded its memory allowance.
    #[error("memory limit exceeded: {0}MB")]
    MemoryLimitExceeded(u32),

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Errors returned by host function implementations.
///
/// These are the typed failures that host function traits
/// ([`super::WasmHostHttp`], [`super::WasmHostSecrets`],
/// [`super::WasmHostTools`]) may return to deny or report problems
/// with a host resource access.
#[derive(Error, Debug)]
pub enum WasmHostError {
    /// HTTP access is not permitted for this plugin.
    #[error("http access denied")]
    HttpAccessDenied,

    /// The requested secret does not exist or is not accessible.
    #[error("secret '{0}' not found")]
    SecretNotFound(String),

    /// The requested tool is not available or access is denied.
    #[error("tool '{0}' not found or access denied")]
    ToolAccessDenied(String),

    /// A permitted tool invocation itself failed.
    #[error("tool invocation failed: {0}")]
    ToolInvocationFailed(String),

    /// Internal host infrastructure error (catch-all).
    #[error("internal host error: {0}")]
    Internal(String),
}
