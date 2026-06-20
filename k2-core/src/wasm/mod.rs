//! WASM sandbox for executing untrusted tool code via Extism.
//!
//! This module provides a sandboxed runtime for loading and executing
//! WASM plugins as K2 tools.  It follows ironclaw's `WitToolRuntime`
//! pattern but is built on Extism's higher-level API instead of
//! wasmtime's component model.
//!
//! # Architecture
//!
//! * [`WasmToolRuntime`] — the runtime that enforces resource limits and
//!   loads plugins.
//! * [`WasmPlugin`] — a loaded plugin; implements [`K2Tool`] so it can
//!   be registered in the tool registry.
//! * [`WasmHost`] — aggregates host function implementations (HTTP,
//!   secrets, tools) that plugins may call.
//! * [`WasmRuntimeConfig`] — memory, timeout, and fuel limits.
//!
//! # Deny-by-default
//!
//! Every plugin starts with zero privileges.  The [`DenyWasmHostHttp`],
//! [`DenyWasmHostSecrets`], and [`DenyWasmHostTools`] structs provide
//! fail-closed implementations of each host function trait.
//!
//! # Example
//!
//! ```ignore
//! use k2_core::wasm::{
//!     WasmToolRuntime, WasmRuntimeConfig, WasmHost
//! };
//!
//! let config = WasmRuntimeConfig::default();
//! let mut runtime = WasmToolRuntime::new(config)?;
//! let host = WasmHost::new();
//! let plugin = runtime.load_plugin(&wasm_bytes, host)?;
//!
//! // Register plugin as a tool
//! registry.register(Box::new(plugin))?;
//! ```

mod error;
mod host;
mod limits;
mod plugin;
mod runtime;

pub use error::{WasmError, WasmHostError};
pub use host::{
    DenyWasmHostHttp, DenyWasmHostSecrets, DenyWasmHostTools,
    WasmHost, WasmHostHttp, WasmHostSecrets, WasmHostTools,
};
pub use limits::WasmRuntimeConfig;
pub use plugin::WasmPlugin;
pub use runtime::WasmToolRuntime;
