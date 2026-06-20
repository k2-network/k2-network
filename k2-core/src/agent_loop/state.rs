//! Loop execution state, stage tracking, and agent state types.

use serde::{Deserialize, Serialize};

use crate::capabilities::tool::ToolResult;
use crate::llm::provider::LlmMessage;
use crate::store::CheckpointId;

/// The pipeline stages of the agent loop.
///
/// Each iteration of the loop progresses through these stages in order.
/// The `Completed` variant marks a loop that has finished (either naturally
/// or because the stop condition was met).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopStage {
    /// Accepting user input and appending it to the conversation.
    Input,
    /// Calling the LLM provider with the current message history.
    Model,
    /// Executing tool calls requested by the model.
    Capability,
    /// Evaluating stop conditions (no tool calls, max iterations, etc.).
    Stop,
    /// The loop has finished — no further processing.
    Completed,
}

impl LoopStage {
    /// Return the string label for this stage (used in checkpoint metadata).
    pub fn as_str(&self) -> &'static str {
        match self {
            LoopStage::Input => "input",
            LoopStage::Model => "model",
            LoopStage::Capability => "capability",
            LoopStage::Stop => "stop",
            LoopStage::Completed => "completed",
        }
    }
}

impl std::fmt::Display for LoopStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The mutable state carried through each iteration of the agent loop.
///
/// This struct is serialised into a [`crate::store::checkpoint::LoopCheckpoint`]
/// so that execution can be paused and resumed across process restarts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopExecutionState {
    /// The session this loop belongs to.
    pub session_id: String,
    /// Which pipeline stage the loop is currently in.
    pub current_stage: LoopStage,
    /// The full conversation history sent to the LLM.
    pub messages: Vec<LlmMessage>,
    /// Results from tool invocations accumulated during this loop.
    pub tool_results: Vec<ToolResult>,
    /// The ID of the most recently saved checkpoint, if any.
    pub checkpoint_id: Option<CheckpointId>,
    /// How many iterations have been executed so far.
    pub iteration_count: u32,
}

impl LoopExecutionState {
    /// Create a new, empty execution state for the given session.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            current_stage: LoopStage::Input,
            messages: Vec::new(),
            tool_results: Vec::new(),
            checkpoint_id: None,
            iteration_count: 0,
        }
    }

    /// Advance to the next pipeline stage.
    pub fn advance_stage(&mut self) {
        self.current_stage = match self.current_stage {
            LoopStage::Input => LoopStage::Model,
            LoopStage::Model => LoopStage::Capability,
            LoopStage::Capability => LoopStage::Stop,
            LoopStage::Stop => LoopStage::Completed,
            LoopStage::Completed => LoopStage::Completed,
        };
    }

    /// Increment the iteration counter.
    pub fn increment_iteration(&mut self) {
        self.iteration_count += 1;
    }

    /// Append a message to the conversation history.
    pub fn push_message(&mut self, msg: LlmMessage) {
        self.messages.push(msg);
    }

    /// Append a tool result.
    pub fn push_tool_result(&mut self, result: ToolResult) {
        self.tool_results.push(result);
    }
}

/// Lightweight snapshot of the agent's high-level status.
///
/// Unlike [`LoopExecutionState`] this does not include the full message
/// history — it is intended for logging, metrics, and UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub session_id: String,
    pub stage: LoopStage,
    pub iteration_count: u32,
    pub message_count: usize,
    pub tool_result_count: usize,
    pub has_checkpoint: bool,
}

impl AgentState {
    /// Build a snapshot from the full execution state.
    pub fn from_execution(state: &LoopExecutionState) -> Self {
        Self {
            session_id: state.session_id.clone(),
            stage: state.current_stage,
            iteration_count: state.iteration_count,
            message_count: state.messages.len(),
            tool_result_count: state.tool_results.len(),
            has_checkpoint: state.checkpoint_id.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_stage_as_str() {
        assert_eq!(LoopStage::Input.as_str(), "input");
        assert_eq!(LoopStage::Model.as_str(), "model");
        assert_eq!(LoopStage::Capability.as_str(), "capability");
        assert_eq!(LoopStage::Stop.as_str(), "stop");
        assert_eq!(LoopStage::Completed.as_str(), "completed");
    }

    #[test]
    fn test_loop_stage_serde() {
        let json = serde_json::to_string(&LoopStage::Model).unwrap();
        assert_eq!(json, "\"model\"");
        let back: LoopStage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, LoopStage::Model);
    }

    #[test]
    fn test_loop_stage_advance() {
        let mut state = LoopExecutionState::new("sess-1");
        assert_eq!(state.current_stage, LoopStage::Input);

        state.advance_stage();
        assert_eq!(state.current_stage, LoopStage::Model);

        state.advance_stage();
        assert_eq!(state.current_stage, LoopStage::Capability);

        state.advance_stage();
        assert_eq!(state.current_stage, LoopStage::Stop);

        state.advance_stage();
        assert_eq!(state.current_stage, LoopStage::Completed);

        // Completed stays completed.
        state.advance_stage();
        assert_eq!(state.current_stage, LoopStage::Completed);
    }

    #[test]
    fn test_loop_execution_state_new() {
        let state = LoopExecutionState::new("test-session");
        assert_eq!(state.session_id, "test-session");
        assert_eq!(state.current_stage, LoopStage::Input);
        assert!(state.messages.is_empty());
        assert!(state.tool_results.is_empty());
        assert!(state.checkpoint_id.is_none());
        assert_eq!(state.iteration_count, 0);
    }

    #[test]
    fn test_push_message_and_tool_result() {
        let mut state = LoopExecutionState::new("sess");
        state.push_message(LlmMessage::user("hello"));
        assert_eq!(state.messages.len(), 1);

        state.push_tool_result(ToolResult::ok(serde_json::json!({"ok": true})));
        assert_eq!(state.tool_results.len(), 1);
    }

    #[test]
    fn test_increment_iteration() {
        let mut state = LoopExecutionState::new("sess");
        assert_eq!(state.iteration_count, 0);
        state.increment_iteration();
        state.increment_iteration();
        assert_eq!(state.iteration_count, 2);
    }

    #[test]
    fn test_agent_state_from_execution() {
        let mut state = LoopExecutionState::new("sess-42");
        state.push_message(LlmMessage::user("hi"));
        state.push_message(LlmMessage::assistant("hello"));
        state.increment_iteration();
        state.checkpoint_id = Some(CheckpointId::from_string("cp-1".to_string()));

        let snapshot = AgentState::from_execution(&state);
        assert_eq!(snapshot.session_id, "sess-42");
        assert_eq!(snapshot.stage, LoopStage::Input);
        assert_eq!(snapshot.iteration_count, 1);
        assert_eq!(snapshot.message_count, 2);
        assert_eq!(snapshot.tool_result_count, 0);
        assert!(snapshot.has_checkpoint);
    }
}