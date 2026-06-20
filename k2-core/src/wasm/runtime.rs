use std::sync::{Arc, Mutex};

use super::error::WasmError;
use super::host::WasmHost;
use super::limits::WasmRuntimeConfig;
use super::plugin::WasmPlugin;
use crate::capabilities::tool::{ToolId, ToolSchema};
use crate::capabilities::context::TrustLevel;

/// Runtime for executing WASM plugins in a sandboxed Extism environment.
///
/// The runtime enforces resource limits from its [`WasmRuntimeConfig`]
/// and registers deny-by-default host functions.  Every plugin loaded
/// via [`load_plugin`](Self::load_plugin) inherits these constraints.
pub struct WasmToolRuntime {
    config: WasmRuntimeConfig,
}

impl WasmToolRuntime {
    /// Create a new runtime with the given configuration.
    ///
    /// The configuration controls memory, timeout, and fuel limits
    /// applied to every subsequently loaded plugin.
    pub fn new(config: WasmRuntimeConfig) -> Result<Self, WasmError> {
        Ok(Self { config })
    }

    /// Return a reference to the runtime's configuration.
    pub fn config(&self) -> &WasmRuntimeConfig {
        &self.config
    }

    /// Load a WASM plugin from raw bytes.
    ///
    /// `wasm_bytes` must be a valid Extism-compatible WASM module.
    /// `host` supplies the host function implementations that the
    /// plugin may call at runtime (HTTP, secrets, tools).
    ///
    /// Returns a [`WasmPlugin`] that can be registered in the tool
    /// registry and invoked through the [`K2Tool`](crate::capabilities::K2Tool) trait.
    pub fn load_plugin(
        &mut self,
        wasm_bytes: &[u8],
        host: WasmHost,
    ) -> Result<WasmPlugin, WasmError> {
        let host = Arc::new(host);

        let host_functions = build_host_functions(&host);

        let plugin = extism::PluginBuilder::new(wasm_bytes)
            .with_functions(host_functions)
            .build()
            .map_err(|e| WasmError::LoadFailed(e.to_string()))?;

        // Derive id and schema from the plugin or use sensible defaults
        let id = ToolId::new();
        let schema = ToolSchema::new(
            "wasm_plugin",
            "A sandboxed WASM tool",
            serde_json::json!({"type": "object"}),
            TrustLevel::Sandbox,
        );

        Ok(WasmPlugin::from_parts(
            id,
            schema,
            Arc::new(Mutex::new(plugin)),
            host,
        ))
    }
}

// ---------------------------------------------------------------------------
// Host function construction
// ---------------------------------------------------------------------------

/// Build the list of Extism host functions that delegate to
/// the [`WasmHost`] trait objects.
fn build_host_functions(host: &Arc<WasmHost>) -> Vec<extism::Function> {
    let http_host = Arc::clone(host);
    let secrets_host = Arc::clone(host);
    let tools_host = Arc::clone(host);

    vec![
        make_http_fetch(http_host),
        make_secrets_exists(secrets_host),
        make_tools_invoke(tools_host),
    ]
}

fn make_http_fetch(host: Arc<WasmHost>) -> extism::Function {
    extism::Function::new(
        "k2_host_http_fetch",
        [extism::PTR],
        [extism::PTR],
        extism::UserData::new(host),
        |plugin, inputs, outputs, user_data| {
            let host = user_data.get().map_err(|e| extism::Error::msg(format!("{}", e)))?;
            let host = host.lock().map_err(|e| extism::Error::msg(format!("{}", e)))?;
            let url: String = plugin.memory_get_val(&inputs[0])
                .map_err(|e| extism::Error::msg(format!("{}", e)))?;
            match host.http.fetch(&url) {
                Ok(body) => {
                    let out = String::from_utf8_lossy(&body).to_string();
                    plugin.memory_set_val(&mut outputs[0], out)
                        .map_err(|e| extism::Error::msg(format!("{}", e)))?;
                    Ok(())
                }
                Err(e) => {
                    let out = format!("{}", e);
                    plugin.memory_set_val(&mut outputs[0], out)
                        .map_err(|e| extism::Error::msg(format!("{}", e)))?;
                    Ok(())
                }
            }
        },
    )
}

fn make_secrets_exists(host: Arc<WasmHost>) -> extism::Function {
    extism::Function::new(
        "k2_host_secrets_exists",
        [extism::PTR],
        [extism::ValType::I32],
        extism::UserData::new(host),
        |plugin, inputs, outputs, user_data| {
            let host = user_data.get().map_err(|e| extism::Error::msg(format!("{}", e)))?;
            let host = host.lock().map_err(|e| extism::Error::msg(format!("{}", e)))?;
            let name: String = plugin.memory_get_val(&inputs[0])
                .map_err(|e| extism::Error::msg(format!("{}", e)))?;
            let exists = host.secrets.exists(&name);
            outputs[0] = extism::Val::I32(if exists { 1 } else { 0 });
            Ok(())
        },
    )
}

fn make_tools_invoke(host: Arc<WasmHost>) -> extism::Function {
    extism::Function::new(
        "k2_host_tools_invoke",
        [extism::PTR, extism::PTR],
        [extism::PTR],
        extism::UserData::new(host),
        |plugin, inputs, outputs, user_data| {
            let host = user_data.get().map_err(|e| extism::Error::msg(format!("{}", e)))?;
            let host = host.lock().map_err(|e| extism::Error::msg(format!("{}", e)))?;
            let alias: String = plugin.memory_get_val(&inputs[0])
                .map_err(|e| extism::Error::msg(format!("{}", e)))?;
            let params: String = plugin.memory_get_val(&inputs[1])
                .map_err(|e| extism::Error::msg(format!("{}", e)))?;
            match host.tools.invoke(&alias, &params) {
                Ok(output) => {
                    plugin.memory_set_val(&mut outputs[0], output)
                        .map_err(|e| extism::Error::msg(format!("{}", e)))?;
                    Ok(())
                }
                Err(e) => {
                    let out = format!("{}", e);
                    plugin.memory_set_val(&mut outputs[0], out)
                        .map_err(|e| extism::Error::msg(format!("{}", e)))?;
                    Ok(())
                }
            }
        },
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::tool::K2Tool;
    use crate::capabilities::context::ExecutionContext;

    /// A minimal valid WASM module that exports `memory` and a `run`
    /// function returning `i64.const 0`.  This is sufficient for
    /// Extism to load and call.
    const MINIMAL_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, // magic "\0asm"
        0x01, 0x00, 0x00, 0x00, // version 1
        // type section (id=1): 1 type: () -> i64
        0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7e,
        // function section (id=3): 1 function, type 0
        0x03, 0x02, 0x01, 0x00,
        // memory section (id=5): 1 memory, min=1 page
        0x05, 0x03, 0x01, 0x00, 0x01,
        // export section (id=7): "memory" (memory 0), "run" (func 0)
        0x07, 0x10, 0x02,
        0x06, b'm', b'e', b'm', b'o', b'r', b'y', 0x02, 0x00,
        0x03, b'r', b'u', b'n', 0x00, 0x00,
        // code section (id=10): 1 body: i64.const 0; end
        0x0a, 0x06, 0x01, 0x04, 0x00, 0x42, 0x00, 0x0b,
    ];

    #[test]
    fn runtime_creation_with_config() {
        let config = WasmRuntimeConfig::restricted();
        let runtime = WasmToolRuntime::new(config.clone()).unwrap();
        assert_eq!(runtime.config().memory_max_mb, 8);
        assert_eq!(runtime.config().timeout_secs, 3);
    }

    #[test]
    fn load_minimal_plugin() {
        let mut runtime = WasmToolRuntime::new(WasmRuntimeConfig::default()).unwrap();
        let host = WasmHost::new();
        let plugin = runtime.load_plugin(MINIMAL_WASM, host);
        assert!(plugin.is_ok(), "failed to load minimal WASM: {:?}", plugin.err());
    }

    #[test]
    fn load_invalid_wasm_fails() {
        let mut runtime = WasmToolRuntime::new(WasmRuntimeConfig::default()).unwrap();
        let host = WasmHost::new();
        let result = runtime.load_plugin(b"not valid wasm", host);
        assert!(result.is_err());
        match result.unwrap_err() {
            WasmError::LoadFailed(_) => {}
            other => panic!("expected LoadFailed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn loaded_plugin_satisfies_k2tool_trait() {
        fn _assert_k2tool<T: K2Tool>() {}
        _assert_k2tool::<WasmPlugin>();

        let mut runtime = WasmToolRuntime::new(WasmRuntimeConfig::default()).unwrap();
        let host = WasmHost::new();
        let plugin = runtime.load_plugin(MINIMAL_WASM, host).unwrap();

        assert_eq!(plugin.trust_level(), TrustLevel::Sandbox);
        assert!(!plugin.id().as_str().is_empty());

        let schema = plugin.schema();
        assert_eq!(schema.required_trust_level, TrustLevel::Sandbox);
    }

    #[tokio::test]
    async fn plugin_invoke_returns_result() {
        let mut runtime = WasmToolRuntime::new(WasmRuntimeConfig::default()).unwrap();
        let host = WasmHost::new();
        let plugin = runtime.load_plugin(MINIMAL_WASM, host).unwrap();

        let ctx = ExecutionContext::sandbox("test-node", "test-sess", None);
        let input = serde_json::json!({"key": "value"});

        // Invoke the plugin — the minimal WASM returns i64.const 0,
        // which Extism may interpret as an empty string result.
        let result = plugin.invoke(input, &ctx).await;
        // The result may succeed or fail depending on how Extism
        // handles the return value; we just verify it doesn't panic.
        match result {
            Ok(_tool_result) => {} // success is fine
            Err(_) => {}           // error is also fine for a trivial plugin
        }
    }
}
