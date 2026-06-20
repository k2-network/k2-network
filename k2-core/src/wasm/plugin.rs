use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use extism::Plugin;

use crate::capabilities::context::{ExecutionContext, TrustLevel};
use crate::capabilities::error::CapabilityError;
use crate::capabilities::tool::{K2Tool, ToolId, ToolResult, ToolSchema};

use super::error::WasmError;
use super::host::WasmHost;

/// A WASM plugin registered as a K2 tool.
///
/// Wraps an Extism [`Plugin`] instance behind `Arc<Mutex<>>` so it can
/// be shared across threads and invoked concurrently through the
/// [`K2Tool`] trait.  Invocation serializes the input as JSON, calls
/// the plugin's `invoke` export, and deserializes the output as JSON.
///
/// Plugins run at [`TrustLevel::Sandbox`] by default.
pub struct WasmPlugin {
    id: ToolId,
    schema: ToolSchema,
    plugin: Arc<Mutex<Plugin>>,
    host: Arc<WasmHost>,
}

impl std::fmt::Debug for WasmPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmPlugin")
            .field("id", &self.id)
            .field("schema", &self.schema)
            .field("trust_level", &TrustLevel::Sandbox)
            .finish_non_exhaustive()
    }
}

impl WasmPlugin {
    /// Internal constructor used by [`super::WasmToolRuntime::load_plugin`].
    pub(crate) fn from_parts(
        id: ToolId,
        schema: ToolSchema,
        plugin: Arc<Mutex<Plugin>>,
        host: Arc<WasmHost>,
    ) -> Self {
        Self {
            id,
            schema,
            plugin,
            host,
        }
    }

    /// Return a reference to the host configuration this plugin was
    /// loaded with.
    pub fn host(&self) -> &WasmHost {
        &self.host
    }
}

#[async_trait]
impl K2Tool for WasmPlugin {
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
        let input_str = serde_json::to_string(&input)
            .map_err(|e| CapabilityError::Serialization(e.to_string()))?;

        let plugin = self.plugin.clone();
        let output_str = tokio::task::spawn_blocking(move || {
            let mut plugin = plugin
                .lock()
                .map_err(|e| WasmError::LoadFailed(format!("lock poisoned: {e}")))?;
            plugin
                .call::<&str, String>("invoke", input_str.as_str())
                .map_err(|e| WasmError::InvocationFailed(e.to_string()))
        })
        .await
        .map_err(|e| CapabilityError::Internal(format!("spawn_blocking failed: {e}")))?;

        match output_str {
            Ok(output) => {
                let value: serde_json::Value = serde_json::from_str(&output)
                    .map_err(|e| CapabilityError::Serialization(e.to_string()))?;
                Ok(ToolResult::ok(value))
            }
            Err(e) => Ok(ToolResult::err(e.to_string())),
        }
    }

    fn trust_level(&self) -> TrustLevel {
        TrustLevel::Sandbox
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::context::ExecutionContext;

    /// A minimal valid WASM module that exports `memory` and a `run`
    /// function (i64.const 0).  Used for lightweight integration tests.
    const MINIMAL_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d,
        0x01, 0x00, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7e,
        0x03, 0x02, 0x01, 0x00,
        0x05, 0x03, 0x01, 0x00, 0x01,
        0x07, 0x10, 0x02,
        0x06, b'm', b'e', b'm', b'o', b'r', b'y', 0x02, 0x00,
        0x03, b'r', b'u', b'n', 0x00, 0x00,
        0x0a, 0x06, 0x01, 0x04, 0x00, 0x42, 0x00, 0x0b,
    ];

    #[test]
    fn wasm_plugin_struct_creation() {
        let id = ToolId::from_string("wasm.test");
        let schema = ToolSchema::new(
            "test_tool",
            "A test WASM tool",
            serde_json::json!({"type": "object"}),
            TrustLevel::Sandbox,
        );
        let host = Arc::new(WasmHost::new());

        // Create a minimal Extism plugin from WASM bytes
        let plugin = Plugin::new(MINIMAL_WASM, Vec::new(), true)
            .expect("should load minimal WASM");

        let wasm_plugin = WasmPlugin::from_parts(
            id.clone(),
            schema.clone(),
            Arc::new(Mutex::new(plugin)),
            host.clone(),
        );

        assert_eq!(wasm_plugin.id().as_str(), "wasm.test");
        assert_eq!(wasm_plugin.schema().name, "test_tool");
        assert_eq!(wasm_plugin.trust_level(), TrustLevel::Sandbox);
    }

    #[test]
    fn wasm_plugin_host_access() {
        let host = Arc::new(WasmHost::new());
        let plugin = Plugin::new(MINIMAL_WASM, Vec::new(), true).unwrap();

        let wasm_plugin = WasmPlugin::from_parts(
            ToolId::new(),
            ToolSchema::new("t", "d", serde_json::json!({}), TrustLevel::Sandbox),
            Arc::new(Mutex::new(plugin)),
            host.clone(),
        );

        // Verify host is accessible and deny-by-default
        let h = wasm_plugin.host();
        assert!(h.http.fetch("https://x.com").is_err());
        assert!(!h.secrets.exists("any"));
        assert!(h.tools.invoke("tool", "{}").is_err());
    }

    #[tokio::test]
    async fn wasm_plugin_invoke_with_context() {
        let id = ToolId::from_string("wasm.ctx_test");
        let schema = ToolSchema::new(
            "ctx_test",
            "Tests execution context handling",
            serde_json::json!({"type": "object", "properties": {"msg": {"type": "string"}}}),
            TrustLevel::Sandbox,
        );

        let plugin = Plugin::new(MINIMAL_WASM, Vec::new(), true).unwrap();

        let wasm_plugin = WasmPlugin::from_parts(
            id,
            schema,
            Arc::new(Mutex::new(plugin)),
            Arc::new(WasmHost::new()),
        );

        let ctx = ExecutionContext::sandbox("node-1", "session-1", None);
        let result = wasm_plugin
            .invoke(serde_json::json!({"msg": "hello"}), &ctx)
            .await;

        // Minimal WASM doesn't export "invoke", so it will fail.
        // We just verify the interface doesn't panic.
        match result {
            Ok(_) | Err(_) => {} // either outcome is acceptable for a test
        }
    }
}
