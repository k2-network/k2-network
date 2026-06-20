//! Resumable agent loop executor with pipeline stages.
//!
//! This module provides the [`AgentLoopExecutor`] that drives a multi-turn
//! conversation through pipeline stages (Input → Model → Capability → Stop),
//! with checkpointing and resumption support.
//!
//! # Architecture
//!
//! The executor follows ironclaw's `AgentLoopExecutor` pattern:
//!
//! 1. **InputStage** — Appends user input to the conversation.
//! 2. **ModelStage** — Calls the LLM provider with current messages and
//!    available tool schemas.
//! 3. **CapabilityStage** — Executes tool calls requested by the model,
//!    enforcing trust policy. Blocked calls produce a [`LoopOutcome::Blocked`].
//! 4. **StopStage** — Evaluates stop conditions (no tool calls, max
//!    iterations reached).
//!
//! State is serialised into [`LoopCheckpoint`]s via the [`CheckpointStore`],
//! allowing the loop to be paused and resumed across process restarts.
//!
//! # Example
//!
//! ```ignore
//! use std::sync::Arc;
//! use k2_core::agent_loop::{AgentLoopExecutorBuilder, LoopOutcome};
//!
//! let executor = AgentLoopExecutorBuilder::new()
//!     .with_llm(Arc::new(llm_provider))
//!     .with_tools(Arc::new(tool_registry))
//!     .with_store(Arc::new(checkpoint_store))
//!     .with_trust_policy(Arc::new(trust_policy))
//!     .with_model("gpt-4")
//!     .with_max_iterations(25)
//!     .build();
//!
//! let outcome = executor.tick("Hello!".to_string(), &ctx).await?;
//! ```

pub mod checkpoint;
pub mod error;
pub mod executor;
pub mod outcome;
pub mod stages;
pub mod state;

pub use checkpoint::{
    build_checkpoint, deserialize_state, list_checkpoints_async, load_checkpoint_async,
    save_checkpoint_async, save_checkpoint_sync, serialize_state, InMemoryCheckpointStore,
};
pub use error::{AgentLoopError, AgentLoopResult};
pub use executor::{AgentLoopExecutor, AgentLoopExecutorBuilder};
pub use outcome::{ApprovalRequest, LoopOutcome};
pub use stages::{
    check_trust, extract_tool_calls, invoke_tool_call, CapabilityStage, InputStage, ModelStage,
    PipelineStage, StageResult, StopStage, TrustCheckResult,
};
pub use state::{AgentState, LoopExecutionState, LoopStage};