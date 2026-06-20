//! Host function traits and deny-by-default implementations.
//!
//! WASM plugins request access to external resources (HTTP, secrets,
//! other tools) through host functions registered with the Extism
//! runtime.  Each resource category is represented by a trait:
//!
//! * [`WasmHostHttp`] — outbound HTTP requests.
//! * [`WasmHostSecrets`] — secret/key lookups.
//! * [`WasmHostTools`] — invoke other K2 tools from within a plugin.
//!
//! Every trait ships with a corresponding **deny-by-default**
//! implementation (e.g. [`DenyWasmHostHttp`]) that immediately rejects
//! all requests.  This fail-closed pattern ensures that a freshly
//! loaded plugin starts with zero privileges.
//!
//! The [`WasmHost`] struct aggregates all three host-function
//! implementations and is passed to
//! [`super::WasmToolRuntime::load_plugin`].

use std::sync::Arc;

use super::error::WasmHostError;

// ---------------------------------------------------------------------------
// Host function traits
// ---------------------------------------------------------------------------

/// Host function trait: outbound HTTP.
///
/// Plugins that attempt to `fetch()` a URL will reach this trait
/// implementation registered with the Extism runtime.
pub trait WasmHostHttp: Send + Sync {
    /// Fetch the contents of `url` and return the response body.
    ///
    /// # Errors
    ///
    /// Returns [`WasmHostError::HttpAccessDenied`] when HTTP access is
    /// not granted (the default), or [`WasmHostError::Internal`] for
    /// transport-level failures.
    fn fetch(&self, url: &str) -> Result<Vec<u8>, WasmHostError>;
}

/// Host function trait: secrets and key access.
///
/// Plugins check for the existence of a named secret before attempting
/// to use it.
pub trait WasmHostSecrets: Send + Sync {
    /// Returns `true` if the secret identified by `name` is available
    /// to the plugin.
    fn exists(&self, name: &str) -> bool;
}

/// Host function trait: invoke other K2 tools.
///
/// A plugin may call an allowed tool by its alias, passing JSON
/// parameters, and receive the tool's JSON output.
pub trait WasmHostTools: Send + Sync {
    /// Invoke the tool identified by `alias` with the given JSON
    /// parameters and return the tool's JSON output.
    ///
    /// # Errors
    ///
    /// Returns [`WasmHostError::ToolAccessDenied`] when the alias is
    /// not permitted, or [`WasmHostError::ToolInvocationFailed`] when
    /// the tool itself fails.
    fn invoke(&self, alias: &str, params_json: &str) -> Result<String, WasmHostError>;
}

// ---------------------------------------------------------------------------
// Deny-by-default implementations
// ---------------------------------------------------------------------------

/// Deny-by-default HTTP host — all requests are rejected.
///
/// Use this as a starting point when constructing a [`WasmHost`]; opt
/// in to specific HTTP access by providing a custom
/// [`WasmHostHttp`] implementation via [`WasmHost::with_http`].
pub struct DenyWasmHostHttp;

impl WasmHostHttp for DenyWasmHostHttp {
    fn fetch(&self, _url: &str) -> Result<Vec<u8>, WasmHostError> {
        Err(WasmHostError::HttpAccessDenied)
    }
}

/// Deny-by-default secrets host — all secrets are inaccessible.
///
/// Use this as a starting point; grant access by providing a custom
/// [`WasmHostSecrets`] implementation via [`WasmHost::with_secrets`].
pub struct DenyWasmHostSecrets;

impl WasmHostSecrets for DenyWasmHostSecrets {
    fn exists(&self, _name: &str) -> bool {
        false
    }
}

/// Deny-by-default tools host — all tool invocations are denied.
///
/// Use this as a starting point; whitelist tools by providing a custom
/// [`WasmHostTools`] implementation via [`WasmHost::with_tools`].
pub struct DenyWasmHostTools;

impl WasmHostTools for DenyWasmHostTools {
    fn invoke(&self, alias: &str, _params_json: &str) -> Result<String, WasmHostError> {
        Err(WasmHostError::ToolAccessDenied(alias.to_string()))
    }
}

// ---------------------------------------------------------------------------
// WasmHost aggregate
// ---------------------------------------------------------------------------

/// Aggregate container for all host function implementations.
///
/// Follows a builder pattern: start with [`WasmHost::new`] (which uses
/// deny-by-default for every category), then selectively grant
/// permissions by calling e.g. `.with_http(...)`.
///
/// # Examples
///
/// ```ignore
/// use k2_core::wasm::{WasmHost, DenyWasmHostHttp, DenyWasmHostSecrets, DenyWasmHostTools};
///
/// let host = WasmHost::new()
///     .with_http(Box::new(DenyWasmHostHttp))       // HTTP still denied
///     .with_secrets(Box::new(DenyWasmHostSecrets))  // secrets still denied
///     .with_tools(Box::new(DenyWasmHostTools));     // tools still denied
/// ```
pub struct WasmHost {
    /// HTTP host function implementation.
    pub http: Arc<Box<dyn WasmHostHttp>>,
    /// Secrets host function implementation.
    pub secrets: Arc<Box<dyn WasmHostSecrets>>,
    /// Tools host function implementation.
    pub tools: Arc<Box<dyn WasmHostTools>>,
}

impl WasmHost {
    /// Create a new `WasmHost` with **all** host function categories
    /// set to deny-by-default.
    pub fn new() -> Self {
        Self {
            http: Arc::new(Box::new(DenyWasmHostHttp)),
            secrets: Arc::new(Box::new(DenyWasmHostSecrets)),
            tools: Arc::new(Box::new(DenyWasmHostTools)),
        }
    }

    /// Replace the HTTP host function implementation.
    ///
    /// Builder-style: consumes `self` and returns a new `WasmHost`
    /// with `http` replaced.
    pub fn with_http(mut self, http: Box<dyn WasmHostHttp>) -> Self {
        self.http = Arc::new(http);
        self
    }

    /// Replace the secrets host function implementation.
    ///
    /// Builder-style: consumes `self` and returns a new `WasmHost`
    /// with `secrets` replaced.
    pub fn with_secrets(mut self, secrets: Box<dyn WasmHostSecrets>) -> Self {
        self.secrets = Arc::new(secrets);
        self
    }

    /// Replace the tools host function implementation.
    ///
    /// Builder-style: consumes `self` and returns a new `WasmHost`
    /// with `tools` replaced.
    pub fn with_tools(mut self, tools: Box<dyn WasmHostTools>) -> Self {
        self.tools = Arc::new(tools);
        self
    }
}

impl Default for WasmHost {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for WasmHost {
    fn clone(&self) -> Self {
        Self {
            http: Arc::clone(&self.http),
            secrets: Arc::clone(&self.secrets),
            tools: Arc::clone(&self.tools),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Deny-by-default tests ---

    #[test]
    fn deny_http_blocks_all() {
        let http = DenyWasmHostHttp;
        let result = http.fetch("https://example.com");
        assert!(result.is_err());
        match result.unwrap_err() {
            WasmHostError::HttpAccessDenied => {}
            other => panic!("expected HttpAccessDenied, got {:?}", other),
        }
    }

    #[test]
    fn deny_secrets_blocks_all() {
        let secrets = DenyWasmHostSecrets;
        assert!(!secrets.exists("API_KEY"));
        assert!(!secrets.exists(""));
        assert!(!secrets.exists("anything"));
    }

    #[test]
    fn deny_tools_blocks_all() {
        let tools = DenyWasmHostTools;
        let result = tools.invoke("my_tool", r#"{"key":"val"}"#);
        assert!(result.is_err());
        match result.unwrap_err() {
            WasmHostError::ToolAccessDenied(alias) => {
                assert_eq!(alias, "my_tool");
            }
            other => panic!("expected ToolAccessDenied, got {:?}", other),
        }
    }

    // --- WasmHost builder ---

    #[test]
    fn host_default_is_deny_all() {
        let host = WasmHost::new();

        // HTTP should be denied
        let http_result = host.http.fetch("https://example.com");
        assert!(http_result.is_err());

        // Secrets should all be inaccessible
        assert!(!host.secrets.exists("any"));

        // Tools should be denied
        let tools_result = host.tools.invoke("x", "{}");
        assert!(tools_result.is_err());
    }

    #[test]
    fn host_builder_replaces_components() {
        // A permissive HTTP host
        struct AllowHttp;
        impl WasmHostHttp for AllowHttp {
            fn fetch(&self, url: &str) -> Result<Vec<u8>, WasmHostError> {
                Ok(url.as_bytes().to_vec())
            }
        }

        let host = WasmHost::new()
            .with_http(Box::new(AllowHttp));

        // HTTP should now succeed
        let result = host.http.fetch("hello");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"hello");

        // Secrets and tools should still be denied
        assert!(!host.secrets.exists("x"));
        assert!(host.tools.invoke("x", "{}").is_err());
    }

    #[test]
    fn host_clone_shares_state() {
        let host1 = WasmHost::new();
        let host2 = host1.clone();

        // Both should behave identically (deny-all)
        assert!(!host2.secrets.exists("test"));
        assert!(host2.http.fetch("url").is_err());
        assert!(host2.tools.invoke("t", "{}").is_err());
    }
}
