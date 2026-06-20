//! Error types for the LLM provider abstraction layer.
//!
//! Uses `thiserror` to provide structured, composable error variants
//! covering HTTP failures, JSON parsing, provider-specific errors, and
//! unsupported operations.

use thiserror::Error;

/// Errors that can occur when interacting with any LLM provider.
#[derive(Error, Debug)]
pub enum LlmError {
    /// An HTTP transport error (DNS, connection refused, TLS, timeout, etc.).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Failed to parse the provider's JSON response.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// The provider returned a non-2xx status code.
    #[error("Provider error (status {status}): {body}")]
    Provider {
        /// HTTP status code returned by the provider.
        status: u16,
        /// Raw response body (may be truncated).
        body: String,
    },

    /// The response body did not contain the expected content.
    #[error("No content in response")]
    NoContent,

    /// The requested model was not found on the provider.
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// The API key is missing or invalid.
    #[error("API key not configured. Set the appropriate environment variable or pass it explicitly")]
    ApiKeyMissing,

    /// The operation is not supported by this provider.
    #[error("Operation not supported: {0}")]
    NotSupported(String),

    /// A catch-all for provider-specific errors that don't fit other variants.
    #[error("{0}")]
    Other(String),
}

impl LlmError {
    /// Convenience constructor for provider HTTP errors.
    pub fn provider(status: u16, body: impl Into<String>) -> Self {
        LlmError::Provider {
            status,
            body: body.into(),
        }
    }

    /// Returns `true` if this is an authentication-related error.
    pub fn is_auth_error(&self) -> bool {
        matches!(self, LlmError::ApiKeyMissing)
            || matches!(self, LlmError::Provider { status: 401, .. })
            || matches!(self, LlmError::Provider { status: 403, .. })
    }

    /// Returns `true` if the error may succeed on retry.
    pub fn is_retryable(&self) -> bool {
        matches!(self, LlmError::Http(_)) // network errors are retryable
            || matches!(self, LlmError::Provider { status, .. } if *status >= 500)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = LlmError::NoContent;
        assert_eq!(err.to_string(), "No content in response");

        let err = LlmError::ModelNotFound("gpt-99".to_string());
        assert_eq!(err.to_string(), "Model not found: gpt-99");

        let err = LlmError::provider(429, "rate limited");
        assert_eq!(
            err.to_string(),
            "Provider error (status 429): rate limited"
        );
    }

    #[test]
    fn test_is_auth_error() {
        assert!(LlmError::ApiKeyMissing.is_auth_error());
        assert!(LlmError::provider(401, "unauthorized").is_auth_error());
        assert!(LlmError::provider(403, "forbidden").is_auth_error());
        assert!(!LlmError::NoContent.is_auth_error());
    }

    #[test]
    fn test_is_retryable() {
        assert!(LlmError::provider(500, "server error").is_retryable());
        assert!(LlmError::provider(503, "unavailable").is_retryable());
        assert!(!LlmError::provider(400, "bad request").is_retryable());
        assert!(!LlmError::ModelNotFound("x".into()).is_retryable());
    }

    #[test]
    fn test_http_error_conversion_compiles() {
        fn _assert_from(e: reqwest::Error) -> LlmError {
            e.into()
        }
    }
}
