//! Tool registry — the central hub for discovering and invoking tools.
//!
//! [`ToolRegistry`] follows ironclaw's `CapabilityHost` builder pattern:
//! build it with well-known tools, then register/unregister dynamically
//! at runtime. Thread-safe via `Arc<Mutex<HashMap>>`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::context::ExecutionContext;
use super::error::{CapabilityError, CapabilityResult};
use super::tool::{K2Tool, ToolId, ToolInvocation, ToolResult, ToolSchema};

/// Errors specific to registry operations.
#[derive(Debug)]
pub enum RegisterError {
    AlreadyRegistered,
}

/// A thread-safe, dynamically extensible registry of [`K2Tool`] instances.
///
/// Uses a builder pattern for construction and supports runtime
/// registration, unregistration, schema queries, and invocation.
///
/// # Thread safety
///
/// `ToolRegistry` is `Send + Sync` and can be shared across async tasks.
/// Internally it wraps a `HashMap<ToolId, Box<dyn K2Tool>>` behind an
/// `Arc<Mutex<...>>`.
pub struct ToolRegistry {
    tools: Arc<Mutex<HashMap<ToolId, Box<dyn K2Tool>>>>,
}

impl ToolRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            tools: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for constructing a [`ToolRegistry`] with initial tools.
///
/// # Examples
///
/// ```ignore
/// let registry = ToolRegistryBuilder::new()
///     .with(my_tool)
///     .with(another_tool)
///     .build();
/// ```
pub struct ToolRegistryBuilder {
    registry: ToolRegistry,
}

impl ToolRegistryBuilder {
    /// Create a new builder backed by an empty registry.
    pub fn new() -> Self {
        Self {
            registry: ToolRegistry::new(),
        }
    }

    /// Add a tool to the registry during build time.
    ///
    /// # Panics
    ///
    /// Panics if a tool with the same ID is already registered. Build-time
    /// duplicates are considered programmer error.
    pub fn with(self, tool: Box<dyn K2Tool>) -> Self {
        let id = tool.id().clone();
        {
            let mut tools = self.registry.tools.lock().expect("lock poisoned");
            if tools.contains_key(&id) {
                panic!("duplicate tool ID in builder: {}", id);
            }
            tools.insert(id, tool);
        }
        self
    }

    /// Consume the builder and return the populated [`ToolRegistry`].
    pub fn build(self) -> ToolRegistry {
        self.registry
    }
}

impl Default for ToolRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Registry methods
// ---------------------------------------------------------------------------

impl ToolRegistry {
    /// Register a tool in the registry.
    ///
    /// Returns `Err(CapabilityError::AlreadyRegistered)` if a tool with the
    /// same ID already exists.
    pub fn register(&self, tool: Box<dyn K2Tool>) -> CapabilityResult<()> {
        let id = tool.id().clone();
        let mut tools = self
            .tools
            .lock()
            .map_err(|_| CapabilityError::LockPoisoned)?;
        if tools.contains_key(&id) {
            return Err(CapabilityError::AlreadyRegistered(id.to_string()));
        }
        tools.insert(id, tool);
        Ok(())
    }

    /// Remove a tool from the registry by its ID.
    ///
    /// Returns `Ok(true)` if a tool was removed, `Ok(false)` if the tool was
    /// not found.
    pub fn unregister(&self, id: &ToolId) -> CapabilityResult<bool> {
        let mut tools = self
            .tools
            .lock()
            .map_err(|_| CapabilityError::LockPoisoned)?;
        Ok(tools.remove(id).is_some())
    }

    /// Check whether a tool with the given ID exists.
    pub fn contains(&self, id: &ToolId) -> CapabilityResult<bool> {
        let tools = self
            .tools
            .lock()
            .map_err(|_| CapabilityError::LockPoisoned)?;
        Ok(tools.contains_key(id))
    }

    /// Return the [`ToolSchema`] for a registered tool.
    ///
    /// Returns `Err(CapabilityError::NotFound)` if the tool is unknown.
    pub fn get_schema(&self, id: &ToolId) -> CapabilityResult<ToolSchema> {
        let tools = self
            .tools
            .lock()
            .map_err(|_| CapabilityError::LockPoisoned)?;
        let tool = tools
            .get(id)
            .ok_or_else(|| CapabilityError::NotFound(id.to_string()))?;
        Ok(tool.schema())
    }

    /// List every tool currently registered, returning their IDs and schemas.
    pub fn list_tools(&self) -> CapabilityResult<Vec<(ToolId, ToolSchema)>> {
        let tools = self
            .tools
            .lock()
            .map_err(|_| CapabilityError::LockPoisoned)?;
        let entries: Vec<_> = tools
            .iter()
            .map(|(id, tool)| (id.clone(), tool.schema()))
            .collect();
        Ok(entries)
    }

    /// Invoke a tool by ID with the given input and context.
    ///
    /// Performs trust-level gating before dispatching to the tool.
    /// Returns the tool's [`ToolResult`] on success, or a
    /// [`CapabilityError`] if the tool is not found, the trust level is
    /// insufficient, or the tool itself fails.
    pub async fn invoke(
        &self,
        id: &ToolId,
        input: serde_json::Value,
        ctx: &ExecutionContext,
    ) -> CapabilityResult<ToolResult> {
        let tools = self
            .tools
            .lock()
            .map_err(|_| CapabilityError::LockPoisoned)?;

        let tool = tools
            .get(id)
            .ok_or_else(|| CapabilityError::NotFound(id.to_string()))?;

        let required = tool.trust_level();
        if !ctx.trust_level.satisfies(required) {
            return Err(CapabilityError::InsufficientTrust {
                required: required.to_string(),
                caller: ctx.trust_level.to_string(),
            });
        }

        tool.invoke(input, ctx).await
    }

    /// Shortcut: invoke from a [`ToolInvocation`] struct.
    pub async fn invoke_from(
        &self,
        inv: &ToolInvocation,
    ) -> CapabilityResult<ToolResult> {
        self.invoke(&inv.tool_id, inv.input.clone(), &inv.context)
            .await
    }

    /// Return the number of registered tools.
    pub fn len(&self) -> CapabilityResult<usize> {
        let tools = self
            .tools
            .lock()
            .map_err(|_| CapabilityError::LockPoisoned)?;
        Ok(tools.len())
    }

    /// Returns `true` if no tools are registered.
    pub fn is_empty(&self) -> CapabilityResult<bool> {
        Ok(self.len()? == 0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::context::{ExecutionContext, TrustLevel};
    use crate::capabilities::error::CapabilityError;
    use crate::capabilities::tool::{K2Tool, ToolId, ToolResult, ToolSchema};
    use async_trait::async_trait;

    // --- Mock tool for testing ---

    struct MockTool {
        id: ToolId,
        schema: ToolSchema,
        behaviour: MockBehaviour,
    }

    enum MockBehaviour {
        Success(serde_json::Value),
        Failure(String),
    }

    impl MockTool {
        fn new_echo() -> Self {
            let id = ToolId::from_string("mock.echo");
            let schema = ToolSchema::new(
                "echo",
                "Echoes the input back",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    }
                }),
                TrustLevel::Sandbox,
            );
            Self {
                id,
                schema,
                behaviour: MockBehaviour::Success(serde_json::Value::Null),
            }
        }

        fn new_failing(msg: impl Into<String>) -> Self {
            let id = ToolId::from_string("mock.failing");
            let schema = ToolSchema::new(
                "failing",
                "Always fails",
                serde_json::json!({"type": "object"}),
                TrustLevel::UserTrusted,
            );
            Self {
                id,
                schema,
                behaviour: MockBehaviour::Failure(msg.into()),
            }
        }

        fn new_protected() -> Self {
            let id = ToolId::from_string("mock.protected");
            let schema = ToolSchema::new(
                "protected",
                "Requires system trust",
                serde_json::json!({"type": "object"}),
                TrustLevel::System,
            );
            Self {
                id,
                schema,
                behaviour: MockBehaviour::Success(serde_json::json!({"ok": true})),
            }
        }
    }

    #[async_trait]
    impl K2Tool for MockTool {
        fn id(&self) -> &ToolId {
            &self.id
        }

        fn schema(&self) -> ToolSchema {
            self.schema.clone()
        }

        async fn invoke(
            &self,
            input: serde_json::Value,
            _ctx: &ExecutionContext,
        ) -> Result<ToolResult, CapabilityError> {
            match &self.behaviour {
                MockBehaviour::Success(default) => {
                    let output = if input.is_null() || input == serde_json::Value::Null {
                        default.clone()
                    } else {
                        input.clone()
                    };
                    Ok(ToolResult::ok(output))
                }
                MockBehaviour::Failure(msg) => Ok(ToolResult::err(msg.as_str())),
            }
        }

        fn trust_level(&self) -> TrustLevel {
            self.schema.required_trust_level
        }
    }

    // --- Tests ---

    #[tokio::test]
    async fn register_and_contains() {
        let registry = ToolRegistry::new();
        let tool = MockTool::new_echo();
        let id = tool.id().clone();

        assert!(!registry.contains(&id).unwrap());
        registry.register(Box::new(tool)).unwrap();
        assert!(registry.contains(&id).unwrap());
    }

    #[tokio::test]
    async fn register_duplicate_fails() {
        let registry = ToolRegistry::new();
        let tool1 = MockTool::new_echo();
        let tool2 = MockTool::new_echo(); // same ID

        registry.register(Box::new(tool1)).unwrap();
        let result = registry.register(Box::new(tool2));
        assert!(result.is_err());
        match result.unwrap_err() {
            CapabilityError::AlreadyRegistered(id) => {
                assert!(id.contains("mock.echo"));
            }
            other => panic!("expected AlreadyRegistered, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn invoke_success() {
        let registry = ToolRegistry::new();
        let id = ToolId::from_string("mock.echo");
        let tool = MockTool::new_echo();
        registry.register(Box::new(tool)).unwrap();

        let ctx = ExecutionContext::system("node-1", "sess-1");
        let result = registry
            .invoke(&id, serde_json::json!({"message": "hello"}), &ctx)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.output, serde_json::json!({"message": "hello"}));
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn invoke_failure() {
        let registry = ToolRegistry::new();
        let id = ToolId::from_string("mock.failing");
        let tool = MockTool::new_failing("tool is broken");
        registry.register(Box::new(tool)).unwrap();

        let ctx = ExecutionContext::system("node-1", "sess-1");
        let result = registry
            .invoke(&id, serde_json::json!({}), &ctx)
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.error, Some("tool is broken".into()));
    }

    #[tokio::test]
    async fn invoke_not_found() {
        let registry = ToolRegistry::new();
        let id = ToolId::from_string("nonexistent");
        let ctx = ExecutionContext::system("node-1", "sess-1");

        let result = registry.invoke(&id, serde_json::json!({}), &ctx).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CapabilityError::NotFound(id_str) => {
                assert!(id_str.contains("nonexistent"));
            }
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn trust_level_gating() {
        let registry = ToolRegistry::new();
        let id = ToolId::from_string("mock.protected");
        let tool = MockTool::new_protected();
        registry.register(Box::new(tool)).unwrap();

        // Sandbox context should be blocked from System-level tool.
        let sandbox_ctx = ExecutionContext::sandbox("node-1", "sess-1", None);
        let result = registry
            .invoke(&id, serde_json::json!({}), &sandbox_ctx)
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CapabilityError::InsufficientTrust { .. } => {}
            other => panic!("expected InsufficientTrust, got {:?}", other),
        }

        // System context should succeed.
        let sys_ctx = ExecutionContext::system("node-1", "sess-1");
        let result = registry
            .invoke(&id, serde_json::json!({}), &sys_ctx)
            .await
            .unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn list_tools() {
        let registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new_echo())).unwrap();
        registry.register(Box::new(MockTool::new_failing("err"))).unwrap();
        registry
            .register(Box::new(MockTool::new_protected()))
            .unwrap();

        let list = registry.list_tools().unwrap();
        assert_eq!(list.len(), 3);

        let names: Vec<&str> = list.iter().map(|(_, s)| s.name.as_str()).collect();
        assert!(names.contains(&"echo"));
        assert!(names.contains(&"failing"));
        assert!(names.contains(&"protected"));
    }

    #[tokio::test]
    async fn unregister() {
        let registry = ToolRegistry::new();
        let id = ToolId::from_string("mock.echo");
        registry.register(Box::new(MockTool::new_echo())).unwrap();
        assert!(registry.contains(&id).unwrap());

        let removed = registry.unregister(&id).unwrap();
        assert!(removed);
        assert!(!registry.contains(&id).unwrap());

        // Unregistering again should return false.
        let removed = registry.unregister(&id).unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    async fn get_schema() {
        let registry = ToolRegistry::new();
        let id = ToolId::from_string("mock.echo");
        registry.register(Box::new(MockTool::new_echo())).unwrap();

        let schema = registry.get_schema(&id).unwrap();
        assert_eq!(schema.name, "echo");
        assert_eq!(schema.required_trust_level, TrustLevel::Sandbox);
    }

    #[tokio::test]
    async fn get_schema_not_found() {
        let registry = ToolRegistry::new();
        let id = ToolId::from_string("missing");
        let result = registry.get_schema(&id);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn builder_pattern() {
        let registry = ToolRegistryBuilder::new()
            .with(Box::new(MockTool::new_echo()))
            .with(Box::new(MockTool::new_failing("oops")))
            .build();

        assert_eq!(registry.len().unwrap(), 2);
        assert!(registry.contains(&ToolId::from_string("mock.echo")).unwrap());
    }

    #[tokio::test]
    async fn invoke_from_struct() {
        let registry = ToolRegistry::new();
        let id = ToolId::from_string("mock.echo");
        registry.register(Box::new(MockTool::new_echo())).unwrap();

        let inv = ToolInvocation::new(
            id.clone(),
            serde_json::json!({"msg": "hi"}),
            ExecutionContext::system("node-1", "sess-1"),
        );

        let result = registry.invoke_from(&inv).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, serde_json::json!({"msg": "hi"}));
    }

    #[tokio::test]
    async fn len_and_is_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty().unwrap());
        assert_eq!(registry.len().unwrap(), 0);

        registry.register(Box::new(MockTool::new_echo())).unwrap();
        assert!(!registry.is_empty().unwrap());
        assert_eq!(registry.len().unwrap(), 1);
    }
}
