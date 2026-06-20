use std::collections::HashMap;

use crate::llm::provider::LlmProvider;

/// A registry that holds multiple named [`LlmProvider`] implementations
/// and allows selecting one by name at runtime.
///
/// Supports fallback: if `get("groq")` returns `None`, the first
/// registered provider is returned instead.
///
/// ## Example
/// ```no_run
/// use k2_core::llm::{LlmRegistry, GroqProvider};
///
/// let mut registry = LlmRegistry::new();
/// registry.register("groq", Box::new(
///     GroqProvider::from_env().unwrap()
/// ));
///
/// if let Some(provider) = registry.get("groq") {
///     println!("Using provider: {}", provider.name());
/// }
/// ```
#[derive(Default)]
pub struct LlmRegistry {
    providers: HashMap<String, Box<dyn LlmProvider + Send + Sync>>,
}

impl LlmRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Build a registry pre-populated with the default Groq provider.
    ///
    /// Reads `GROQ_API_KEY` from the environment. If the key is not set,
    /// the registry starts empty (callers can add providers later).
    pub fn default() -> Self {
        let mut registry = Self::new();
        if let Ok(groq) = crate::llm::GroqProvider::from_env() {
            registry.register("groq", Box::new(groq));
        }
        registry
    }

    /// Register a provider under the given name.
    ///
    /// Replaces any existing provider with the same name.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        provider: Box<dyn LlmProvider + Send + Sync>,
    ) {
        self.providers.insert(name.into(), provider);
    }

    /// Look up a provider by name.
    ///
    /// Returns `None` if not found. Does NOT apply fallback — use
    /// [`get_or_first`] for fallback behaviour.
    pub fn get(&self, name: &str) -> Option<&(dyn LlmProvider + Send + Sync)> {
        self.providers.get(name).map(|b| b.as_ref())
    }

    /// Look up a provider by name, falling back to the first registered
    /// provider if the requested name is not found.
    pub fn get_or_first(&self, name: &str) -> Option<&(dyn LlmProvider + Send + Sync)> {
        self.get(name).or_else(|| {
            self.providers
                .values()
                .next()
                .map(|b| b.as_ref())
        })
    }

    /// Remove a provider by name. Returns the removed provider, if any.
    pub fn remove(&mut self, name: &str) -> Option<Box<dyn LlmProvider + Send + Sync>> {
        self.providers.remove(name)
    }

    /// Return the names of all registered providers.
    pub fn names(&self) -> Vec<&str> {
        self.providers.keys().map(|k| k.as_str()).collect()
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl std::fmt::Debug for LlmRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmRegistry")
            .field("providers", &self.names())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::error::LlmError;
    use crate::llm::provider::{
        FinishReason, LlmRequest, LlmResponse,
    };
    use async_trait::async_trait;

    /// A stub provider for testing the registry.
    struct StubProvider {
        model: String,
    }

    #[async_trait]
    impl LlmProvider for StubProvider {
        async fn chat(
            &self,
            _request: LlmRequest,
        ) -> Result<LlmResponse, LlmError> {
            Ok(LlmResponse {
                content: Some(self.model.clone()),
                tool_calls: None,
                finish_reason: FinishReason::Stop,
                model: self.model.clone(),
                usage: None,
            })
        }

        fn name(&self) -> &str {
            "stub"
        }
    }

    #[test]
    fn test_new_registry_is_empty() {
        let r = LlmRegistry::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(r.names().is_empty());
    }

    #[test]
    fn test_register_and_get() {
        let mut r = LlmRegistry::new();
        r.register("my-provider", Box::new(StubProvider {
            model: "test-model".to_string(),
        }));

        assert_eq!(r.len(), 1);
        assert!(!r.is_empty());
        assert_eq!(r.names(), vec!["my-provider"]);

        let p = r.get("my-provider").unwrap();
        assert_eq!(p.name(), "stub");
    }

    #[test]
    fn test_get_missing_returns_none() {
        let r = LlmRegistry::new();
        assert!(r.get("nope").is_none());
    }

    #[test]
    fn test_get_or_first_fallback() {
        let mut r = LlmRegistry::new();
        r.register("a", Box::new(StubProvider {
            model: "model-a".to_string(),
        }));
        r.register("b", Box::new(StubProvider {
            model: "model-b".to_string(),
        }));

        // Exact match
        assert!(r.get_or_first("b").is_some());

        // Missing → first inserted
        let fallback = r.get_or_first("missing").unwrap();
        assert!(!fallback.name().is_empty());
    }

    #[test]
    fn test_get_or_first_empty_registry() {
        let r = LlmRegistry::new();
        assert!(r.get_or_first("anything").is_none());
    }

    #[test]
    fn test_remove() {
        let mut r = LlmRegistry::new();
        r.register("temp", Box::new(StubProvider {
            model: "x".to_string(),
        }));
        assert_eq!(r.len(), 1);

        let removed = r.remove("temp");
        assert!(removed.is_some());
        assert!(r.is_empty());
        assert!(r.get("temp").is_none());
    }

    #[test]
    fn test_remove_missing() {
        let mut r = LlmRegistry::new();
        assert!(r.remove("no-such").is_none());
    }

    #[test]
    fn test_register_overwrites() {
        let mut r = LlmRegistry::new();
        r.register("p", Box::new(StubProvider {
            model: "first".to_string(),
        }));
        r.register("p", Box::new(StubProvider {
            model: "second".to_string(),
        }));
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn test_debug_fmt() {
        let mut r = LlmRegistry::new();
        r.register("groq", Box::new(StubProvider {
            model: "m".to_string(),
        }));
        let dbg = format!("{:?}", r);
        assert!(dbg.contains("groq"));
        assert!(dbg.contains("LlmRegistry"));
    }
}
