//! Pipeline stage trait and implementations for the agent loop.
//!
//! Each stage of the loop is an async operation that transforms the
//! [`LoopExecutionState`]. Stages are executed sequentially within a
//! single iteration (a "tick") of the [`AgentLoopExecutor`].

use async_trait::async_trait;

use crate::capabilities::context::ExecutionContext;
use crate::capabilities::tool::{ToolId, ToolResult};
use crate::capabilities::registry::ToolRegistry;
use crate::llm::provider::{LlmMessage, LlmRequest, LlmResponse, ToolCall};
use crate::security::trust::{TrustPolicy, TrustPolicyInput, TrustSource};

use super::error::{AgentLoopError, AgentLoopResult};
use super::outcome::{ApprovalRequest, LoopOutcome};
use super::state::{LoopExecutionState, LoopStage};

/// The result of running a pipeline stage.
///
/// A stage either continues the pipeline (returning the mutated state)
/// or short-circuits it by producing a terminal [`LoopOutcome`].
pub enum StageResult {
    /// The stage completed; continue to the next stage with this state.
    Continue(LoopExecutionState),
    /// The stage produced a terminal outcome; stop the loop.
    Stop(LoopOutcome),
}

/// A pipeline stage in the agent loop.
///
/// Each implementation processes the [`LoopExecutionState`] and either
/// returns the updated state for the next stage or a terminal
/// [`LoopOutcome`].
#[async_trait]
pub trait PipelineStage: Send + Sync {
    /// Execute this stage against the current loop state.
    async fn execute(
        &self,
        state: LoopExecutionState,
        ctx: &ExecutionContext,
    ) -> AgentLoopResult<StageResult>;
}

// ---------------------------------------------------------------------------
// InputStage
// ---------------------------------------------------------------------------

/// Appends user input to the conversation as a `LlmMessage::user(...)`.
pub struct InputStage;

impl InputStage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InputStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PipelineStage for InputStage {
    async fn execute(
        &self,
        mut state: LoopExecutionState,
        _ctx: &ExecutionContext,
    ) -> AgentLoopResult<StageResult> {
        state.current_stage = LoopStage::Input;
        Ok(StageResult::Continue(state))
    }
}

/// Helper: append user input to the state's message history.
pub fn apply_user_input(state: &mut LoopExecutionState, input: &str) {
    state.push_message(LlmMessage::user(input));
}

// ---------------------------------------------------------------------------
// ModelStage
// ---------------------------------------------------------------------------

/// Calls the LLM provider with the current messages and available tool
/// schemas. The response (including any tool calls) is appended to the
/// conversation history.
pub struct ModelStage<'a> {
    pub llm: &'a dyn crate::llm::provider::LlmProvider,
    pub model_name: String,
    pub tool_schemas: Vec<crate::llm::provider::ToolSchema>,
}

impl<'a> ModelStage<'a> {
    pub fn new(
        llm: &'a dyn crate::llm::provider::LlmProvider,
        model_name: impl Into<String>,
        tool_schemas: Vec<crate::llm::provider::ToolSchema>,
    ) -> Self {
        Self {
            llm,
            model_name: model_name.into(),
            tool_schemas,
        }
    }
}

#[async_trait]
impl<'a> PipelineStage for ModelStage<'a> {
    async fn execute(
        &self,
        mut state: LoopExecutionState,
        _ctx: &ExecutionContext,
    ) -> AgentLoopResult<StageResult> {
        state.current_stage = LoopStage::Model;

        let mut request = LlmRequest::new(self.model_name.clone(), state.messages.clone());
        if !self.tool_schemas.is_empty() {
            request = request.with_tools(self.tool_schemas.clone());
        }

        let response = self.llm.chat(request).await?;

        let assistant_msg = LlmMessage {
            role: crate::llm::provider::MessageRole::Assistant,
            content: response.content.clone().unwrap_or_default(),
            tool_calls: response.tool_calls.clone(),
            tool_call_id: None,
        };
        state.push_message(assistant_msg);

        Ok(StageResult::Continue(state))
    }
}

/// Extract tool calls from an LLM response, if any.
pub fn extract_tool_calls(response: &LlmResponse) -> Vec<ToolCall> {
    response.tool_calls.clone().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// CapabilityStage
// ---------------------------------------------------------------------------

/// Executes tool calls requested by the model, enforcing trust policy.
///
/// If a tool call requires higher trust than the context allows, the
/// stage returns [`LoopOutcome::Blocked`] with an [`ApprovalRequest`].
pub struct CapabilityStage<'a> {
    pub tools: &'a ToolRegistry,
    pub trust_policy: &'a dyn TrustPolicy,
}

impl<'a> CapabilityStage<'a> {
    pub fn new(tools: &'a ToolRegistry, trust_policy: &'a dyn TrustPolicy) -> Self {
        Self { tools, trust_policy }
    }
}

/// The outcome of evaluating a single tool call through the trust policy.
pub enum TrustCheckResult {
    /// The tool call is allowed — proceed with invocation.
    Allowed,
    /// The tool call is blocked — produce a `Blocked` outcome.
    Blocked(ApprovalRequest),
}

/// Check whether a tool call is permitted by the trust policy.
///
/// Looks up the tool's schema to determine its required trust level,
/// then evaluates the trust policy against the execution context.
pub fn check_trust(
    tool_id: &ToolId,
    tools: &ToolRegistry,
    trust_policy: &dyn TrustPolicy,
    ctx: &ExecutionContext,
    session_id: &str,
    input: &serde_json::Value,
) -> AgentLoopResult<TrustCheckResult> {
    let schema = tools
        .get_schema(tool_id)
        .map_err(AgentLoopError::ToolError)?;

    let requested_effects = effects_for_trust_level(&schema.required_trust_level);

    let trust_input = TrustPolicyInput {
        peer_id: ctx.peer_id.clone(),
        source: trust_source_from_context(ctx),
        requested_effects,
    };

    let decision = trust_policy
        .evaluate(&trust_input)
        .map_err(AgentLoopError::TrustError)?;

    let required_class = trust_class_for_level(&schema.required_trust_level);
    if decision.effective_trust.at_least(&required_class) {
        Ok(TrustCheckResult::Allowed)
    } else {
        let reason = format!(
            "tool '{}' requires {} trust but context only provides {}",
            tool_id,
            schema.required_trust_level,
            decision.effective_trust.label()
        );
        let approval = ApprovalRequest::new(
            tool_id.to_string(),
            session_id,
            input.clone(),
            reason,
        );
        Ok(TrustCheckResult::Blocked(approval))
    }
}

/// Map a [`crate::capabilities::context::TrustLevel`] to the set of
/// effects it implies.
fn effects_for_trust_level(
    level: &crate::capabilities::context::TrustLevel,
) -> Vec<crate::security::trust::EffectKind> {
    use crate::capabilities::context::TrustLevel;
    use crate::security::trust::EffectKind;

    match level {
        TrustLevel::Sandbox => vec![EffectKind::Read],
        TrustLevel::UserTrusted => vec![
            EffectKind::Read,
            EffectKind::Write,
            EffectKind::Network,
        ],
        TrustLevel::FirstParty => vec![
            EffectKind::Read,
            EffectKind::Write,
            EffectKind::Network,
            EffectKind::Filesystem,
            EffectKind::Execute,
        ],
        TrustLevel::System => vec![
            EffectKind::Read,
            EffectKind::Write,
            EffectKind::Network,
            EffectKind::Filesystem,
            EffectKind::Execute,
            EffectKind::Admin,
        ],
    }
}

/// Map a [`crate::capabilities::context::TrustLevel`] to an
/// [`crate::security::trust::EffectiveTrustClass`].
fn trust_class_for_level(
    level: &crate::capabilities::context::TrustLevel,
) -> crate::security::trust::EffectiveTrustClass {
    use crate::capabilities::context::TrustLevel;

    match level {
        TrustLevel::Sandbox => crate::security::trust::EffectiveTrustClass::sandbox(),
        TrustLevel::UserTrusted => crate::security::trust::EffectiveTrustClass::user_trusted(),
        TrustLevel::FirstParty => crate::security::trust::EffectiveTrustClass::first_party(),
        TrustLevel::System => crate::security::trust::EffectiveTrustClass::system(),
    }
}

/// Determine the [`TrustSource`] from an execution context.
fn trust_source_from_context(
    ctx: &ExecutionContext,
) -> TrustSource {
    if ctx.peer_id.is_some() {
        TrustSource::RemotePeer
    } else {
        TrustSource::LocalUser
    }
}

/// Invoke a tool call and return the result, appending it to the state.
pub async fn invoke_tool_call(
    state: &mut LoopExecutionState,
    tools: &ToolRegistry,
    tool_call: &ToolCall,
    ctx: &ExecutionContext,
) -> AgentLoopResult<ToolResult> {
    let tool_id = ToolId::from_string(tool_call.function.name.clone());
    let input: serde_json::Value = if tool_call.function.arguments.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&tool_call.function.arguments)
            .unwrap_or(serde_json::Value::Null)
    };

    let result = tools
        .invoke(&tool_id, input.clone(), ctx)
        .await
        .map_err(AgentLoopError::ToolError)?;

    let tool_msg = LlmMessage::tool(
        serde_json::to_string(&result.output).unwrap_or_default(),
        tool_call.id.clone(),
    );
    state.push_message(tool_msg);
    state.push_tool_result(result.clone());

    Ok(result)
}

// ---------------------------------------------------------------------------
// StopStage
// ---------------------------------------------------------------------------

/// Evaluates stop conditions: if no tool calls were made or the max
/// iteration count was reached, the loop terminates.
pub struct StopStage {
    pub max_iterations: u32,
    pub tool_calls: Vec<ToolCall>,
}

impl StopStage {
    pub fn new(max_iterations: u32, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            max_iterations,
            tool_calls,
        }
    }
}

#[async_trait]
impl PipelineStage for StopStage {
    async fn execute(
        &self,
        mut state: LoopExecutionState,
        _ctx: &ExecutionContext,
    ) -> AgentLoopResult<StageResult> {
        state.current_stage = LoopStage::Stop;

        if state.iteration_count >= self.max_iterations {
            state.current_stage = LoopStage::Completed;
            return Ok(StageResult::Stop(LoopOutcome::MaxIterationsReached {
                max: self.max_iterations,
                iterations: state.iteration_count,
            }));
        }

        if self.tool_calls.is_empty() {
            state.current_stage = LoopStage::Completed;
            let final_content = state
                .messages
                .last()
                .filter(|m| {
                    m.role == crate::llm::provider::MessageRole::Assistant
                })
                .map(|m| m.content.clone());

            return Ok(StageResult::Stop(LoopOutcome::Completed {
                final_content,
                iterations: state.iteration_count,
            }));
        }

        Ok(StageResult::Continue(state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::context::{ExecutionContext, TrustLevel};

    #[test]
    fn test_apply_user_input() {
        let mut state = LoopExecutionState::new("sess-1");
        apply_user_input(&mut state, "hello world");
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].content, "hello world");
        assert_eq!(state.messages[0].role, crate::llm::provider::MessageRole::User);
    }

    #[test]
    fn test_extract_tool_calls_empty() {
        let response = LlmResponse {
            content: Some("hi".to_string()),
            tool_calls: None,
            finish_reason: crate::llm::provider::FinishReason::Stop,
            model: "test".to_string(),
            usage: None,
        };
        assert!(extract_tool_calls(&response).is_empty());
    }

    #[test]
    fn test_extract_tool_calls_present() {
        let call = ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: crate::llm::provider::ToolCallFunction {
                name: "echo".to_string(),
                arguments: "{}".to_string(),
            },
        };
        let response = LlmResponse {
            content: None,
            tool_calls: Some(vec![call.clone()]),
            finish_reason: crate::llm::provider::FinishReason::ToolCalls,
            model: "test".to_string(),
            usage: None,
        };
        let calls = extract_tool_calls(&response);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "echo");
    }

    #[test]
    fn test_effects_for_trust_level() {
        assert_eq!(
            effects_for_trust_level(&TrustLevel::Sandbox).len(),
            1
        );
        assert_eq!(
            effects_for_trust_level(&TrustLevel::System).len(),
            6
        );
    }

    #[test]
    fn test_trust_source_from_context_local() {
        let ctx = ExecutionContext::system("node-1", "sess-1");
        assert!(matches!(trust_source_from_context(&ctx), TrustSource::LocalUser));
    }

    #[test]
    fn test_trust_source_from_context_remote() {
        let ctx = ExecutionContext::new(
            "node-1".to_string(),
            "sess-1".to_string(),
            TrustLevel::UserTrusted,
            Some("peer-abc".to_string()),
        );
        assert!(matches!(trust_source_from_context(&ctx), TrustSource::RemotePeer));
    }
}