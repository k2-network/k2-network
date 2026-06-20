//! LLM Provider Abstraction Layer for K2.
//!
//! Provides a trait-based abstraction for interacting with multiple
//! Large Language Model backends through a unified interface.
//!
//! ## Supported providers
//!
//! | Module          | Provider       | Notes                            |
//! |-----------------|----------------|----------------------------------|
//! | [`groq`]        | Groq (LPU)     | Requires `GROQ_API_KEY` env var  |
//! | [`openai_compat`] | OpenAI-compat | Works with any `/chat/completions` API |
//! | [`ollama`]      | Ollama (local) | No API key; must have Ollama running |
//!
//! ## Quick start
//!
//! ```no_run
//! use k2_core::llm::{
//!     LlmRegistry, GroqProvider,
//!     LlmProvider, LlmRequest, LlmMessage,
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Option A: use the registry (auto-detects Groq from env)
//! let registry = LlmRegistry::default();
//! if let Some(provider) = registry.get_or_first("groq") {
//!     let resp = provider.chat(LlmRequest::new(
//!         "llama-3.3-70b-versatile",
//!         vec![LlmMessage::user("Xin chào!")],
//!     )).await?;
//!     println!("{}", resp.content.unwrap_or_default());
//! }
//!
//! // Option B: construct provider directly
//! let provider = GroqProvider::from_env()?;
//! let resp = provider.chat(LlmRequest::new(
//!     "llama-3.3-70b-versatile",
//!     vec![LlmMessage::user("Phân tích yêu cầu: cần mua laptop")],
//! )).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! ```text
//! LlmRequest  ──►  LlmProvider::chat()  ──►  LlmResponse
//!                     │
//!     ┌───────────────┼───────────────┐
//!     │               │               │
//! GroqProvider  OpenAiCompat  OllamaProvider
//!     │               │               │
//! Groq API     OpenAI API     Ollama /api/chat
//! ```

pub mod error;
pub mod provider;
pub mod groq;
pub mod openai_compat;
pub mod ollama;
pub mod registry;

pub use error::LlmError;
pub use groq::{GroqProvider, K2_MARKETPLACE_SYSTEM_PROMPT};
pub use ollama::OllamaProvider;
pub use openai_compat::OpenAiCompatProvider;
pub use provider::{
    FinishReason, FunctionSchema, LlmMessage, LlmProvider, LlmRequest,
    LlmResponse, MessageRole, ResponseFormat, ToolCall, ToolCallFunction,
    ToolSchema, Usage,
};
pub use registry::LlmRegistry;
