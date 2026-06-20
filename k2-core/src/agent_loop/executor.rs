//! The `AgentLoopExecutor` — drives the multi-turn agent loop through
//! pipeline stages with checkpointing and resumption support.
//!
//! Follows ironclaw's `AgentLoopExecutor` pattern: each call to `tick`
//! processes one full iteration through all stages (Input → Model →
//! Capability → Stop). The loop can be paused (via `save_checkpoint`)
//! and resumed (via `resume`) across process restarts.

use std::sync::Arc;

use crate::capabilities::context::ExecutionContext;
use crate::capabilities::registry::ToolRegistry;
use crate::capabilities::tool::ToolId;
use crate::llm::provider::{LlmProvider, LlmRequest, LlmResponse, ToolCall, ToolSchema as LlmToolSchema};
use crate::security::trust::TrustPolicy;
use crate::store::{CheckpointId, CheckpointStore};

use super::checkpoint::{load_checkpoint_async, save_checkpoint_async};
use super::error::{AgentLoopError, AgentLoopResult};
use super::outcome::LoopOutcome;
use super::stages::{
    check_trust, extract_tool_calls, invoke_tool_call, TrustCheckResult,
};
use super::state::{LoopExecutionState, LoopStage};

/// Default maximum iterations before the loop is forced to stop.
const DEFAULT_MAX_ITERATIONS: u32 = 25;

/// The agent loop executor — drives the conversation through pipeline
/// stages with checkpointing and trust-gated tool invocation.
///
/// Construct via [`AgentLoopExecutorBuilder`].
pub struct AgentLoopExecutor {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<ToolRegistry>,
    store: Arc<dyn CheckpointStore>,
    trust_policy: Arc<dyn TrustPolicy>,
    max_iterations: u32,
    model_name: String,
}

impl AgentLoopExecutor {
    /// Process one full iteration through all pipeline stages.
    ///
    /// 1. **Input**: Append `user_input` to the conversation.
    /// 2. **Model**: Call the LLM with current messages + tool schemas.
    /// 3. **Capability**: Execute tool calls (trust-gated).
    /// 4. **Stop**: Check stop conditions.
    ///
    /// Returns the terminal [`LoopOutcome`]. If the outcome is
    /// `Blocked`, a checkpoint is saved before returning.
    pub async fn tick(
        &self,
        user_input: String,
        ctx: &ExecutionContext,
    ) -> AgentLoopResult<LoopOutcome> {
        let mut state = LoopExecutionState::new(ctx.session_id.clone());
        state.push_message(crate::llm::provider::LlmMessage::user(user_input));
        state.increment_iteration();

        self.run_pipeline(state, ctx).await
    }

    /// Resume execution from a previously saved checkpoint.
    ///
    /// Loads the checkpoint, deserialises the state, and continues
    /// the pipeline from the appropriate stage.
    pub async fn resume(
        &self,
        checkpoint_id: &CheckpointId,
        ctx: &ExecutionContext,
    ) -> AgentLoopResult<LoopOutcome> {
        let mut state = load_checkpoint_async(Arc::clone(&self.store), checkpoint_id).await?;

        state.increment_iteration();

        self.run_pipeline_from_model(state, ctx).await
    }

    /// Save the current loop state as a checkpoint.
    pub async fn save_checkpoint(
        &self,
        state: &LoopExecutionState,
    ) -> AgentLoopResult<CheckpointId> {
        let id = save_checkpoint_async(Arc::clone(&self.store), state.clone()).await?;
        Ok(id)
    }

    /// Run the full pipeline: Model → Capability → Stop.
    ///
    /// Assumes the input message has already been appended.
    async fn run_pipeline(
        &self,
        mut state: LoopExecutionState,
        ctx: &ExecutionContext,
    ) -> AgentLoopResult<LoopOutcome> {
        loop {
            let outcome = self.run_single_iteration(&mut state, ctx).await?;
            match outcome {
                IterationOutcome::Continue => continue,
                IterationOutcome::Stop(loop_outcome) => return Ok(loop_outcome),
            }
        }
    }

    /// Resume from the model stage (skip input since it was already
    /// applied before the checkpoint).
    async fn run_pipeline_from_model(
        &self,
        mut state: LoopExecutionState,
        ctx: &ExecutionContext,
    ) -> AgentLoopResult<LoopOutcome> {
        loop {
            let outcome = self.run_single_iteration(&mut state, ctx).await?;
            match outcome {
                IterationOutcome::Continue => continue,
                IterationOutcome::Stop(loop_outcome) => return Ok(loop_outcome),
            }
        }
    }

    /// Execute a single iteration: Model → Capability → Stop.
    async fn run_single_iteration(
        &self,
        state: &mut LoopExecutionState,
        ctx: &ExecutionContext,
    ) -> AgentLoopResult<IterationOutcome> {
        // --- Model Stage ---
        state.current_stage = LoopStage::Model;
        let tool_schemas = self.collect_tool_schemas();
        let response = self.call_llm(state, &tool_schemas).await?;
        let tool_calls = extract_tool_calls(&response);

        // --- Capability Stage ---
        state.current_stage = LoopStage::Capability;
        for tool_call in &tool_calls {
            let input = self.parse_tool_arguments(tool_call)?;
            let tool_id = ToolId::from_string(tool_call.function.name.clone());

            let trust_check = check_trust(
                &tool_id,
                &self.tools,
                self.trust_policy.as_ref(),
                ctx,
                &state.session_id,
                &input,
            )?;

            match trust_check {
                TrustCheckResult::Allowed => {
                    let _result = invoke_tool_call(state, &self.tools, tool_call, ctx).await?;
                }
                TrustCheckResult::Blocked(approval) => {
                    state.checkpoint_id =
                        Some(self.save_checkpoint(state).await?);
                    return Ok(IterationOutcome::Stop(LoopOutcome::Blocked(approval)));
                }
            }
        }

        // --- Stop Stage ---
        state.current_stage = LoopStage::Stop;

        if state.iteration_count >= self.max_iterations {
            state.current_stage = LoopStage::Completed;
            return Ok(IterationOutcome::Stop(LoopOutcome::MaxIterationsReached {
                max: self.max_iterations,
                iterations: state.iteration_count,
            }));
        }

        if tool_calls.is_empty() {
            state.current_stage = LoopStage::Completed;
            let final_content = state
                .messages
                .last()
                .filter(|m| {
                    m.role == crate::llm::provider::MessageRole::Assistant
                })
                .map(|m| m.content.clone());

            return Ok(IterationOutcome::Stop(LoopOutcome::Completed {
                final_content,
                iterations: state.iteration_count,
            }));
        }

        state.increment_iteration();
        Ok(IterationOutcome::Continue)
    }

    /// Collect tool schemas from the registry in the LLM provider's
    /// expected format.
    fn collect_tool_schemas(&self) -> Vec<LlmToolSchema> {
        let tools = match self.tools.list_tools() {
            Ok(list) => list,
            Err(_) => return Vec::new(),
        };

        tools
            .into_iter()
            .map(|(_, schema)| LlmToolSchema {
                schema_type: "function".to_string(),
                function: crate::llm::provider::FunctionSchema {
                    name: schema.name,
                    description: schema.description,
                    parameters: schema.parameters,
                },
            })
            .collect()
    }

    /// Call the LLM with the current messages and tool schemas, then
    /// append the assistant response to the conversation history.
    async fn call_llm(
        &self,
        state: &mut LoopExecutionState,
        tool_schemas: &[LlmToolSchema],
    ) -> AgentLoopResult<LlmResponse> {
        let mut request = LlmRequest::new(self.model_name.clone(), state.messages.clone());
        if !tool_schemas.is_empty() {
            request = request.with_tools(tool_schemas.to_vec());
        }
        let response = self.llm.chat(request).await?;

        let assistant_msg = crate::llm::provider::LlmMessage {
            role: crate::llm::provider::MessageRole::Assistant,
            content: response.content.clone().unwrap_or_default(),
            tool_calls: response.tool_calls.clone(),
            tool_call_id: None,
        };
        state.push_message(assistant_msg);

        Ok(response)
    }

    /// Parse tool call arguments from JSON string to a Value.
    fn parse_tool_arguments(&self, tool_call: &ToolCall) -> AgentLoopResult<serde_json::Value> {
        if tool_call.function.arguments.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&tool_call.function.arguments).map_err(AgentLoopError::from)
    }
}

/// Internal outcome of a single iteration.
enum IterationOutcome {
    Continue,
    Stop(LoopOutcome),
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for constructing an [`AgentLoopExecutor`].
///
/// # Example
///
/// ```ignore
/// let executor = AgentLoopExecutorBuilder::new()
///     .with_llm(Arc::new(provider))
///     .with_tools(Arc::new(registry))
///     .with_store(Arc::new(store))
///     .with_trust_policy(Arc::new(policy))
/// .with_model("gpt-4")
///     .with_max_iterations(10)
///     .build();
/// ```
pub struct AgentLoopExecutorBuilder {
    llm: Option<Arc<dyn LlmProvider>>,
    tools: Option<Arc<ToolRegistry>>,
    store: Option<Arc<dyn CheckpointStore>>,
    trust_policy: Option<Arc<dyn TrustPolicy>>,
    max_iterations: u32,
    model_name: Option<String>,
}

impl AgentLoopExecutorBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self {
            llm: None,
            tools: None,
            store: None,
            trust_policy: None,
            max_iterations: DEFAULT_MAX_ITERATIONS,
            model_name: None,
        }
    }

    /// Set the LLM provider.
    pub fn with_llm(mut self, llm: Arc<dyn LlmProvider>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Set the tool registry.
    pub fn with_tools(mut self, tools: Arc<ToolRegistry>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set the checkpoint store.
    pub fn with_store(mut self, store: Arc<dyn CheckpointStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Set the trust policy.
    pub fn with_trust_policy(mut self, policy: Arc<dyn TrustPolicy>) -> Self {
        self.trust_policy = Some(policy);
        self
    }

    /// Set the model name to use for LLM requests.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model_name = Some(model.into());
        self
    }

    /// Set the maximum number of iterations.
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Build the executor.
    ///
    /// # Panics
    ///
    /// Panics if any required dependency (llm, tools, store, trust_policy)
    /// has not been set.
    pub fn build(self) -> AgentLoopExecutor {
        AgentLoopExecutor {
            llm: self.llm.expect("AgentLoopExecutorBuilder: llm is required"),
            tools: self
                .tools
                .expect("AgentLoopExecutorBuilder: tools is required"),
            store: self
                .store
                .expect("AgentLoopExecutorBuilder: store is required"),
            trust_policy: self
                .trust_policy
                .expect("AgentLoopExecutorBuilder: trust_policy is required"),
            max_iterations: self.max_iterations,
            model_name: self.model_name.unwrap_or_else(|| "default".to_string()),
        }
    }
}

impl Default for AgentLoopExecutorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use super::*;
    use crate::capabilities::context::{ExecutionContext, TrustLevel};
    use crate::capabilities::error::CapabilityError;
    use crate::capabilities::tool::{K2Tool, ToolId, ToolResult, ToolSchema};
    use crate::llm::error::LlmError;
    use crate::llm::provider::{
        FinishReason, LlmRequest, LlmResponse, ToolCall, ToolCallFunction,
    };
    use crate::security::error::TrustError;
    use crate::security::trust::{
        AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustPolicyInput,
    };

    use super::super::checkpoint::InMemoryCheckpointStore;

    // --- Mock LLM Provider ---

    struct MockLlmProvider {
        responses: std::sync::Mutex<Vec<LlmResponse>>,
    }

    impl MockLlmProvider {
        fn new_simple(content: &str) -> Self {
            Self {
                responses: std::sync::Mutex::new(vec![LlmResponse {
                    content: Some(content.to_string()),
                    tool_calls: None,
                    finish_reason: FinishReason::Stop,
                    model: "mock-model".to_string(),
                    usage: None,
                }]),
            }
        }

        fn new_with_tool_call(tool_name: &str, args: &str) -> Self {
            Self {
                responses: std::sync::Mutex::new(vec![LlmResponse {
                    content: Some("I'll use a tool".to_string()),
                    tool_calls: Some(vec![ToolCall {
                        id: "call_1".to_string(),
                        call_type: "function".to_string(),
                        function: ToolCallFunction {
                            name: tool_name.to_string(),
                            arguments: args.to_string(),
                        },
                    }]),
                    finish_reason: FinishReason::ToolCalls,
                    model: "mock-model".to_string(),
                    usage: None,
                }]),
            }
        }

        fn new_multi_stage(first: LlmResponse, second: LlmResponse) -> Self {
            Self {
                responses: std::sync::Mutex::new(vec![first, second]),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockLlmProvider {
        async fn chat(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
            let mut responses = self.responses.lock().unwrap();
            if responses.len() == 1 {
                Ok(responses[0].clone())
            } else {
                Ok(responses.remove(0))
            }
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    // --- Mock Tool ---

    struct MockEchoTool {
        id: ToolId,
        schema: ToolSchema,
    }

    impl MockEchoTool {
        fn new() -> Self {
            Self {
                id: ToolId::from_string("echo"),
                schema: ToolSchema::new(
                    "echo",
                    "Echoes input back",
                    serde_json::json!({"type": "object"}),
                    TrustLevel::Sandbox,
                ),
            }
        }
    }

    #[async_trait]
    impl K2Tool for MockEchoTool {
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
            Ok(ToolResult::ok(input))
        }
    }

    struct MockProtectedTool {
        id: ToolId,
        schema: ToolSchema,
    }

    impl MockProtectedTool {
        fn new() -> Self {
            Self {
                id: ToolId::from_string("protected"),
                schema: ToolSchema::new(
                    "protected",
                    "Requires system trust",
                    serde_json::json!({"type": "object"}),
                    TrustLevel::System,
                ),
            }
        }
    }

    #[async_trait]
    impl K2Tool for MockProtectedTool {
        fn id(&self) -> &ToolId {
            &self.id
        }
        fn schema(&self) -> ToolSchema {
            self.schema.clone()
        }
        async fn invoke(
            &self,
            _input: serde_json::Value,
            _ctx: &ExecutionContext,
        ) -> Result<ToolResult, CapabilityError> {
            Ok(ToolResult::ok(serde_json::json!({"ok": true})))
        }
    }

    // --- Mock Trust Policies ---

    struct PermissiveTrustPolicy;

    impl TrustPolicy for PermissiveTrustPolicy {
        fn evaluate(&self, _input: &TrustPolicyInput) -> Result<TrustDecision, TrustError> {
            Ok(TrustDecision {
                effective_trust: EffectiveTrustClass::system(),
                authority_ceiling: AuthorityCeiling::permissive(),
                provenance: crate::security::trust::TrustProvenance::new(
                    crate::security::trust::TrustSource::LocalUser,
                    "permissive test policy",
                ),
            })
        }
    }

    struct SandboxOnlyTrustPolicy;

    impl TrustPolicy for SandboxOnlyTrustPolicy {
        fn evaluate(&self, _input: &TrustPolicyInput) -> Result<TrustDecision, TrustError> {
            Ok(TrustDecision {
                effective_trust: EffectiveTrustClass::sandbox(),
                authority_ceiling: AuthorityCeiling::default(),
                provenance: crate::security::trust::TrustProvenance::new(
                    crate::security::trust::TrustSource::LocalUser,
                    "sandbox-only test policy",
                ),
            })
        }
    }

    // --- Helpers ---

    fn build_executor(
        llm: Arc<dyn LlmProvider>,
        tools: Arc<ToolRegistry>,
        store: Arc<dyn CheckpointStore>,
        policy: Arc<dyn TrustPolicy>,
        max_iter: u32,
    ) -> AgentLoopExecutor {
        AgentLoopExecutorBuilder::new()
            .with_llm(llm)
            .with_tools(tools)
            .with_store(store)
            .with_trust_policy(policy)
            .with_model("mock-model")
            .with_max_iterations(max_iter)
            .build()
    }

    fn system_ctx() -> ExecutionContext {
        ExecutionContext::system("node-1", "test-session")
    }

    // --- Tests ---

    #[tokio::test]
    async fn test_simple_conversation_completes() {
        let llm = Arc::new(MockLlmProvider::new_simple("Hello! How can I help?"));
        let tools = Arc::new(ToolRegistry::new());
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let policy: Arc<dyn TrustPolicy> = Arc::new(PermissiveTrustPolicy);

        let executor = build_executor(llm, tools, store, policy, 10);
        let outcome = executor
            .tick("Hi there".to_string(), &system_ctx())
            .await
            .unwrap();

        assert!(outcome.is_completed());
        match outcome {
            LoopOutcome::Completed {
                final_content,
                iterations,
            } => {
                assert_eq!(final_content, Some("Hello! How can I help?".to_string()));
                assert_eq!(iterations, 1);
            }
            _ => panic!("expected Completed"),
        }
    }

    #[tokio::test]
    async fn test_tool_call_invoked() {
        let first = LlmResponse {
            content: Some("I'll use a tool".to_string()),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                call_type: "function".to_string(),
                function: ToolCallFunction {
                    name: "echo".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            finish_reason: FinishReason::ToolCalls,
            model: "mock-model".to_string(),
            usage: None,
        };
        let second = LlmResponse {
            content: Some("Done!".to_string()),
            tool_calls: None,
            finish_reason: FinishReason::Stop,
            model: "mock-model".to_string(),
            usage: None,
        };
        let llm = Arc::new(MockLlmProvider::new_multi_stage(first, second));
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Box::new(MockEchoTool::new())).unwrap();
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let policy: Arc<dyn TrustPolicy> = Arc::new(PermissiveTrustPolicy);

        let executor = build_executor(llm, tools, store, policy, 10);
        let outcome = executor
            .tick("echo please".to_string(), &system_ctx())
            .await
            .unwrap();

        assert!(outcome.is_completed());
    }

    #[tokio::test]
    async fn test_blocked_by_trust_policy() {
        let llm = Arc::new(MockLlmProvider::new_with_tool_call("protected", "{}"));
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Box::new(MockProtectedTool::new())).unwrap();
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let policy: Arc<dyn TrustPolicy> = Arc::new(SandboxOnlyTrustPolicy);

        let executor = build_executor(llm, tools, store, policy, 10);
        let outcome = executor
            .tick("use protected tool".to_string(), &system_ctx())
            .await
            .unwrap();

        assert!(outcome.is_blocked());
        match outcome {
            LoopOutcome::Blocked(approval) => {
                assert_eq!(approval.tool_id, "protected");
                assert_eq!(approval.session_id, "test-session");
                assert!(!approval.reason.is_empty());
            }
            _ => panic!("expected Blocked"),
        }
    }

    #[tokio::test]
    async fn test_checkpoint_save_and_resume() {
        let llm = Arc::new(MockLlmProvider::new_with_tool_call("protected", "{}"));
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Box::new(MockProtectedTool::new())).unwrap();
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let policy: Arc<dyn TrustPolicy> = Arc::new(SandboxOnlyTrustPolicy);

        let executor = build_executor(llm, Arc::clone(&tools), Arc::clone(&store), policy, 10);

        let outcome = executor
            .tick("use protected".to_string(), &system_ctx())
            .await
            .unwrap();

        assert!(outcome.is_blocked());

        let checkpoints = store
            .list_checkpoints("test-session")
            .unwrap();
        assert_eq!(checkpoints.len(), 1);

        let checkpoint_id = checkpoints[0].id.clone();

        let permissive_policy: Arc<dyn TrustPolicy> = Arc::new(PermissiveTrustPolicy);
        let permissive_llm = Arc::new(MockLlmProvider::new_simple("Done!"));
        let executor2 = build_executor(
            permissive_llm,
            tools,
            Arc::clone(&store),
            permissive_policy,
            10,
        );

        let resumed = executor2
            .resume(&checkpoint_id, &system_ctx())
            .await
            .unwrap();

        assert!(resumed.is_completed());
    }

    #[tokio::test]
    async fn test_max_iterations_enforced() {
        let tool_call_response = LlmResponse {
            content: Some("calling tool".to_string()),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                call_type: "function".to_string(),
                function: ToolCallFunction {
                    name: "echo".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            finish_reason: FinishReason::ToolCalls,
            model: "mock-model".to_string(),
            usage: None,
        };
        let stop_response = LlmResponse {
            content: Some("done".to_string()),
            tool_calls: None,
            finish_reason: FinishReason::Stop,
            model: "mock-model".to_string(),
            usage: None,
        };

        let mut responses = Vec::new();
        for _ in 0..10 {
            responses.push(tool_call_response.clone());
        }
        responses.push(stop_response);

        let llm = Arc::new(MockLlmProvider {
            responses: std::sync::Mutex::new(responses),
        });
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Box::new(MockEchoTool::new())).unwrap();
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let policy: Arc<dyn TrustPolicy> = Arc::new(PermissiveTrustPolicy);

        let executor = build_executor(llm, tools, store, policy, 3);
        let outcome = executor
            .tick("keep calling echo".to_string(), &system_ctx())
            .await
            .unwrap();

        match outcome {
            LoopOutcome::MaxIterationsReached { max, iterations } => {
                assert_eq!(max, 3);
                assert_eq!(iterations, 3);
            }
            _ => panic!("expected MaxIterationsReached, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_save_checkpoint_directly() {
        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::new_simple("hi"));
        let tools = Arc::new(ToolRegistry::new());
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let policy: Arc<dyn TrustPolicy> = Arc::new(PermissiveTrustPolicy);

        let executor = build_executor(llm, tools, Arc::clone(&store), policy, 10);

        let state = LoopExecutionState::new("direct-save");
        let id = executor.save_checkpoint(&state).await.unwrap();

        let loaded = store.load_checkpoint(&id).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().session_id, "direct-save");
    }

    #[tokio::test]
    async fn test_resume_nonexistent_checkpoint() {
        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::new_simple("hi"));
        let tools = Arc::new(ToolRegistry::new());
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let policy: Arc<dyn TrustPolicy> = Arc::new(PermissiveTrustPolicy);

        let executor = build_executor(llm, tools, store, policy, 10);
        let fake_id = CheckpointId::from_string("nonexistent".to_string());

        let result = executor.resume(&fake_id, &system_ctx()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentLoopError::CheckpointNotFound(_)
        ));
    }

    #[tokio::test]
    async fn test_builder_panics_without_dependencies() {
        let result = std::panic::catch_unwind(|| {
            AgentLoopExecutorBuilder::new().build()
        });
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multi_turn_with_tool_then_stop() {
        let first = LlmResponse {
            content: Some("calling echo".to_string()),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                call_type: "function".to_string(),
                function: ToolCallFunction {
                    name: "echo".to_string(),
                    arguments: "{\"msg\":\"hello\"}".to_string(),
                },
            }]),
            finish_reason: FinishReason::ToolCalls,
            model: "mock-model".to_string(),
            usage: None,
        };
        let second = LlmResponse {
            content: Some("All done!".to_string()),
            tool_calls: None,
            finish_reason: FinishReason::Stop,
            model: "mock-model".to_string(),
            usage: None,
        };

        let llm = Arc::new(MockLlmProvider::new_multi_stage(first, second));
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Box::new(MockEchoTool::new())).unwrap();
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let policy: Arc<dyn TrustPolicy> = Arc::new(PermissiveTrustPolicy);

        let executor = build_executor(llm, tools, store, policy, 10);
        let outcome = executor
            .tick("echo then stop".to_string(), &system_ctx())
            .await
            .unwrap();

        assert!(outcome.is_completed());
        match outcome {
            LoopOutcome::Completed {
                final_content,
                iterations,
            } => {
                assert_eq!(final_content, Some("All done!".to_string()));
                assert!(iterations >= 2);
            }
            _ => panic!("expected Completed"),
        }
    }
}