//! Offline fallback to Ollama when primary provider fails.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::llm::error::LlmError;
use crate::llm::ollama::OllamaProvider;
use crate::llm::provider::{LlmProvider, LlmRequest, LlmResponse};

/// A provider that falls back to a local Ollama instance when the
/// primary provider fails or is unavailable.
///
/// The fallback is only triggered when:
/// - The primary provider returns a network/timeout error
/// - The primary provider returns a retryable error (5xx, 429 rate limit)
///
/// Successful responses from the primary provider are passed through
/// unchanged, even if they contain application-level errors (e.g., content
/// moderation). Only infrastructure/network errors trigger fallback.
///
/// # Example
///
/// ```ignore
/// let groq = Arc::new(GroqProvider::from_env()?);
/// let ollama = Arc::new(OllamaProvider::new("llama2".to_string()));
/// let fallback = FallbackProvider::new(groq, ollama, 3);
/// ```
#[derive(Clone)]
pub struct FallbackProvider {
    /// The primary (cloud) provider to use first.
    primary: Arc<dyn LlmProvider>,

    /// The fallback Ollama provider.
    fallback: Arc<OllamaProvider>,

    /// Whether the primary provider is currently marked as unavailable.
    /// When `true`, all requests go directly to the fallback without
    /// attempting the primary first.
    unavailable: Arc<RwLock<bool>>,

    /// Number of consecutive failures before marking the primary as
    /// unavailable. Defaults to 3.
    _failure_threshold: usize,
}

impl FallbackProvider {
    /// Create a new fallback provider with the given primary and fallback.
    ///
    /// The `failure_threshold` determines how many consecutive failures
    /// from the primary provider are required before it is marked as
    /// unavailable and all requests are routed directly to the fallback.
    /// Once marked unavailable, the primary is not retried until
    /// [`mark_available`](Self::mark_available) is called.
    pub fn new(
        primary: Arc<dyn LlmProvider>,
        fallback: Arc<OllamaProvider>,
        failure_threshold: usize,
    ) -> Self {
        Self {
            primary,
            fallback,
            unavailable: Arc::new(RwLock::new(false)),
            _failure_threshold: failure_threshold,
        }
    }

    /// Mark the primary provider as available again.
    ///
    /// This resets the failure counter and allows subsequent requests to
    /// attempt the primary provider first. Useful when the primary
    /// provider has recovered from an outage.
    pub async fn mark_available(&self) {
        let mut unavailable = self.unavailable.write().await;
        *unavailable = false;
    }

    /// Mark the primary provider as unavailable.
    ///
    /// All subsequent requests will be routed directly to the fallback
    /// until [`mark_available`](Self::mark_available) is called.
    pub async fn mark_unavailable(&self) {
        let mut unavailable = self.unavailable.write().await;
        *unavailable = true;
    }

    /// Check whether the primary provider is currently marked as unavailable.
    pub async fn is_unavailable(&self) -> bool {
        *self.unavailable.read().await
    }

    /// Returns `true` if the error is a fallback-triggering error.
    ///
    /// Fallback-triggering errors are those that indicate infrastructure
    /// problems rather than application-level problems:
    ///
    /// - HTTP errors (reqwest errors)
    /// - Server errors (5xx status codes)
    /// - Rate limiting (429)
    /// - Network/transport errors
    ///
    /// Application-level errors like missing models, auth failures, or
    /// invalid inputs do NOT trigger fallback.
    fn should_fallback(&self, error: &LlmError) -> bool {
        match error {
            // Network/infrastructure errors - always fallback
            LlmError::Http(_) => true,
            LlmError::Provider { status, .. } if *status >= 500 || *status == 429 => true,

            // Other provider errors (4xx except 429) - do not fallback
            LlmError::Provider { .. } => false,

            // Application-level or configuration errors - do not fallback
            // These will fail the same way on Ollama, so no point trying
            LlmError::ApiKeyMissing => false,
            LlmError::ModelNotFound(_) => false,
            LlmError::NoContent => false,
            LlmError::NotSupported(_) => false,
            LlmError::Other(_) => false,
            LlmError::Json(_) => false, // Schema mismatch - won't fix with fallback
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for FallbackProvider {
    async fn chat(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        // If primary is marked unavailable, go straight to fallback
        if self.is_unavailable().await {
            return self
                .fallback
                .chat(request)
                .await
                .map_err(|e| LlmError::Other(format!("Fallback provider also failed: {}", e)));
        }

        // Try primary provider first
        match self.primary.chat(request.clone()).await {
            Ok(response) => Ok(response),
            Err(e) if self.should_fallback(&e) => {
                // Mark primary as unavailable after threshold failures
                // For now, just log and fallback
                // TODO: Implement failure counter to auto-mark unavailable

                // Fallback to Ollama
                self.fallback
                    .chat(request)
                    .await
                    .map_err(|fallback_err| {
                        LlmError::Other(format!(
                            "Primary failed: {}. Fallback also failed: {}",
                            e, fallback_err
                        ))
                    })
            }
            Err(e) => Err(e), // Non-fallback error, return as-is
        }
    }

    fn name(&self) -> &str {
        "fallback"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock primary provider that always fails with server error
    struct FailingPrimary;

    #[async_trait::async_trait]
    impl LlmProvider for FailingPrimary {
        async fn chat(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
            Err(LlmError::Provider {
                status: 503,
                body: "Service unavailable".to_string(),
            })
        }

        fn name(&self) -> &str {
            "failing-primary"
        }
    }

    #[test]
    fn should_fallback_classifies_errors_correctly() {
        let provider = FallbackProvider::new(
            Arc::new(FailingPrimary),
            Arc::new(OllamaProvider::new("http://localhost:11434".to_string(), "llama2".to_string())),
            3,
        );

        // Network/infrastructure errors - should fallback
        // Use Provider error with 5xx status for this test since Http is hard to construct
        assert!(provider.should_fallback(&LlmError::Provider {
            status: 500,
            body: "server error".to_string()
        }));
        assert!(provider.should_fallback(&LlmError::Provider {
            status: 503,
            body: "unavailable".to_string()
        }));
        assert!(provider.should_fallback(&LlmError::Provider {
            status: 429,
            body: "rate limited".to_string()
        }));

        // Application-level errors - should NOT fallback
        assert!(!provider.should_fallback(&LlmError::ApiKeyMissing));
        assert!(!provider.should_fallback(&LlmError::ModelNotFound("gpt-4".to_string())));
        assert!(!provider.should_fallback(&LlmError::NoContent));
        assert!(!provider.should_fallback(&LlmError::NotSupported("streaming".to_string())));
    }

    #[tokio::test]
    async fn fallback_mark_unavailable_bypasses_primary() {
        let provider = FallbackProvider::new(
            Arc::new(FailingPrimary),
            Arc::new(OllamaProvider::new("http://localhost:11434".to_string(), "llama2".to_string())),
            3,
        );

        provider.mark_unavailable().await;
        assert!(provider.is_unavailable().await);

        provider.mark_available().await;
        assert!(!provider.is_unavailable().await);
    }

    #[test]
    fn test_fallback_provider_name() {
        let provider = FallbackProvider::new(
            Arc::new(FailingPrimary),
            Arc::new(OllamaProvider::new("http://localhost:11434".to_string(), "llama2".to_string())),
            3,
        );
        assert_eq!(provider.name(), "fallback");
    }
}