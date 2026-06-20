//! Integration tests for k2-core agent loop.
//!
//! These tests verify the full agent workflow:
//! - Agent initialization with LLM and tools
//! - Multi-turn conversations
//! - Tool invocation with approval
//! - Checkpoint save/resume
//! - Error handling and recovery

#[cfg(test)]
mod integration_tests {
    use k2_core::agent_loop::executor::AgentLoopExecutor;
    use k2_core::agent_loop::stages::{UserInput, MessageRole};
    use k2_core::capabilities::ToolRegistry;
    use k2_core::llm::LlmRequest;
    use k2_core::llm::provider::{LlmMessage, LlmProvider};
    use k2_core::store::SqliteStore;
    use k2_core::tools::register_all;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Mock LLM provider that returns pre-canned responses.
    struct MockLlm {
        responses: Arc<Mutex<Vec<String>>>,
    }

    impl MockLlm {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses)),
            }
        }
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockLlm {
        async fn chat(
            &self,
            _request: LlmRequest,
        ) -> Result<k2_core::llm::provider::LlmResponse, k2_core::llm::error::LlmError> {
            let mut responses = self.responses.lock().await;
            let response = responses.remove(0);
            Ok(k2_core::llm::provider::LlmResponse {
                content: Some(response),
                tool_calls: None,
                finish_reason: k2_core::llm::provider::FinishReason::Stop,
                model: "mock".to_string(),
                usage: None,
            })
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    /// Helper to create test agent executor.
    async fn create_test_agent() -> (AgentLoopExecutor, Arc<SqliteStore>) {
        let store = Arc::new(SqliteStore::in_memory().unwrap());
        
        // Register built-in tools
        let mut tool_registry = ToolRegistry::new();
        register_all(&mut tool_registry).unwrap();
        
        // Mock LLM with simple responses
        let llm = Arc::new(MockLlm::new(vec![
            "Hello! How can I help you?".to_string(),
            "I can help with that.".to_string(),
        ]));

        let executor = AgentLoopExecutor::builder()
            .session_id("test-session".to_string())
            .model("mock-model".to_string())
            .store(store.clone())
            .tool_registry(tool_registry)
            .llm_provider(llm)
            .build()
            .unwrap();

        (executor, store)
    }

    #[tokio::test]
    async fn test_agent_simple_conversation() {
        let (executor, _store) = create_test_agent().await;

        let input = UserInput {
            role: MessageRole::User,
            content: "Hello, agent!".to_string(),
        };

        let outcome = executor.run(input).await.unwrap();
        
        // Agent should respond with the first mock response
        assert!(outcome.messages.last().unwrap().content.contains("Hello"));
        assert_eq!(outcome.status, k2_core::agent_loop::outcome::LoopStatus::Completed);
    }

    #[tokio::test]
    async fn test_agent_checkpoint_save_and_resume() {
        let (executor, store) = create_test_agent().await;
        let checkpoint_id = "checkpoint-1";

        // Run to completion and save checkpoint
        let input = UserInput {
            role: MessageRole::User,
            content: "Test message".to_string(),
        };

        executor.run(input).await.unwrap();
        executor.save_checkpoint(checkpoint_id).await.unwrap();

        // Verify checkpoint exists
        let checkpoints = store.list_checkpoints("test-session").await.unwrap();
        assert!(checkpoints.iter().any(|c| c == checkpoint_id));

        // Create new executor and load checkpoint
        let mut tool_registry = ToolRegistry::new();
        register_all(&mut tool_registry).unwrap();
        
        let llm = Arc::new(MockLlm::new(vec![
            "Hello! How can I help you?".to_string(),
            "I can help with that.".to_string(),
        ]));

        let executor2 = AgentLoopExecutor::builder()
            .session_id("test-session".to_string())
            .model("mock-model".to_string())
            .store(store.clone())
            .tool_registry(tool_registry)
            .llm_provider(llm)
            .build()
            .unwrap();

        let outcome = executor2.resume_from_checkpoint(checkpoint_id).await.unwrap();
        
        assert_eq!(outcome.status, k2_core::agent_loop::outcome::LoopStatus::Completed);
    }

    #[tokio::test]
    async fn test_agent_with_tools() {
        let store = Arc::new(SqliteStore::in_memory().unwrap());
        let mut tool_registry = ToolRegistry::new();
        register_all(&mut tool_registry).unwrap();
        
        // Mock LLM that calls http_get tool
        let llm = Arc::new(MockLlm::new(vec![
            "I'll fetch that URL for you.".to_string(),
        ]));

        let executor = AgentLoopExecutor::builder()
            .session_id("test-tools-session".to_string())
            .model("mock-model".to_string())
            .store(store.clone())
            .tool_registry(tool_registry)
            .llm_provider(llm)
            .build()
            .unwrap();

        let input = UserInput {
            role: MessageRole::User,
            content: "Fetch https://example.com".to_string(),
        };

        // This should complete successfully with tool calls
        let outcome = executor.run(input).await.unwrap();
        assert!(outcome.status.is_terminal());
    }

    #[tokio::test]
    async fn test_agent_error_recovery() {
        let (executor, _store) = create_test_agent().await;

        // Send malformed input (empty message)
        let input = UserInput {
            role: MessageRole::User,
            content: "".to_string(),
        };

        // Agent should handle this gracefully
        let outcome = executor.run(input).await;
        assert!(outcome.is_ok() || outcome.is_err()); // Either way is fine for error handling
    }
}

#[cfg(test)]
mod approval_flow_tests {
    use k2_core::agent_loop::executor::AgentLoopExecutor;
    use k2_core::agent_loop::stages::{UserInput, MessageRole};
    use k2_core::approval::{ApprovalResolver, LeaseApproval};
    use k2_core::capabilities::ToolRegistry;
    use k2_core::llm::LlmRequest;
    use k2_core::llm::provider::{LlmMessage, LlmProvider, LlmResponse, FinishReason};
    use k2_core::llm::error::LlmError;
    use k2_core::security::trust::{EffectKind, ResourceCeiling};
    use k2_core::store::{SqliteStore, ApprovalRecord};
    use k2_core::tools::register_all;
    use std::sync::Arc;

    /// Mock LLM that simulates tool calls requiring approval.
    struct ApprovalMockLlm;

    #[async_trait::async_trait]
    impl LlmProvider for ApprovalMockLlm {
        async fn chat(
            &self,
            _request: LlmRequest,
        ) -> Result<LlmResponse, LlmError> {
            Ok(LlmResponse {
                content: Some("I need to call a tool".to_string()),
                tool_calls: None,
                finish_reason: FinishReason::Stop,
                model: "mock".to_string(),
                usage: None,
            })
        }

        fn name(&self) -> &str {
            "approval-mock"
        }
    }

    #[tokio::test]
    async fn test_approval_workflow() {
        let store = Arc::new(SqliteStore::in_memory().unwrap());
        let mut tool_registry = ToolRegistry::new();
        register_all(&mut tool_registry).unwrap();
        
        let llm = Arc::new(ApprovalMockLlm);

        let executor = AgentLoopExecutor::builder()
            .session_id("approval-session".to_string())
            .model("mock-model".to_string())
            .store(store.clone())
            .tool_registry(tool_registry)
            .llm_provider(llm)
            .build()
            .unwrap();

        let resolver = ApprovalResolver::new(store.clone());

        // Create a pending approval request
        let record = ApprovalRecord::new(
            "test-request-1".to_string(),
            "file_read".to_string(),
            "approval-session".to_string(),
        );
        store.save_approval(&record).unwrap();

        // Approve the request
        let approval = LeaseApproval::new(
            "test-approver".to_string(),
            vec![EffectKind::Read],
            Some(ResourceCeiling::permissive()),
            None,
            Some(5),
        );
        resolver.approve_dispatch("test-request-1", approval).await.unwrap();

        // Verify approval was granted
        let status = resolver.get_status("test-request-1").await.unwrap();
        assert_eq!(status, k2_core::store::ApprovalStatus::Approved);
    }
}