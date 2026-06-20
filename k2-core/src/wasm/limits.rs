//! Resource limits for the WASM sandbox runtime.
//!
//! [`WasmRuntimeConfig`] controls memory, execution time, and fuel
//! (instruction count) budgets applied to every plugin loaded by the
//! runtime.  These limits are enforced by Extism's built-in resource
//! management.

/// Configuration for the WASM runtime sandbox.
///
/// Every plugin loaded by a [`super::WasmToolRuntime`] inherits these
/// limits.  The `Default` implementation provides reasonable
/// conservative values suitable for untrusted code.
///
/// # Examples
///
/// ```ignore
/// let config = WasmRuntimeConfig {
///     memory_max_mb: 16,
///     timeout_secs: 5,
///     fuel_limit: Some(1_000_000),
/// };
/// let runtime = WasmToolRuntime::new(config)?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmRuntimeConfig {
    /// Maximum heap memory (in megabytes) a plugin may allocate.
    ///
    /// Default: `64` (64 MB).
    pub memory_max_mb: u32,

    /// Maximum wall-clock execution time (in seconds) for a single
    /// invocation.  The runtime will kill the plugin if it runs longer.
    ///
    /// Default: `10` (10 seconds).
    pub timeout_secs: u64,

    /// Optional fuel limit — the maximum number of WASM instructions
    /// the plugin is allowed to execute before being terminated.
    ///
    /// `None` (the default) means no instruction limit beyond the
    /// wall-clock timeout.
    ///
    /// Default: `None`.
    pub fuel_limit: Option<u64>,
}

impl Default for WasmRuntimeConfig {
    fn default() -> Self {
        Self {
            memory_max_mb: 64,
            timeout_secs: 10,
            fuel_limit: None,
        }
    }
}

impl WasmRuntimeConfig {
    /// Create a new config with custom values.
    pub fn new(memory_max_mb: u32, timeout_secs: u64, fuel_limit: Option<u64>) -> Self {
        Self {
            memory_max_mb,
            timeout_secs,
            fuel_limit,
        }
    }

    /// Create a tightly-restricted config suitable for completely
    /// untrusted or unknown plugins.
    pub fn restricted() -> Self {
        Self {
            memory_max_mb: 8,
            timeout_secs: 3,
            fuel_limit: Some(500_000),
        }
    }

    /// Create a permissive config (still sandboxed) for trusted
    /// internal plugins that need more resources.
    pub fn permissive() -> Self {
        Self {
            memory_max_mb: 256,
            timeout_secs: 30,
            fuel_limit: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_conservative() {
        let config = WasmRuntimeConfig::default();
        assert_eq!(config.memory_max_mb, 64);
        assert_eq!(config.timeout_secs, 10);
        assert_eq!(config.fuel_limit, None);
    }

    #[test]
    fn restricted_config_is_tight() {
        let config = WasmRuntimeConfig::restricted();
        assert_eq!(config.memory_max_mb, 8);
        assert_eq!(config.timeout_secs, 3);
        assert_eq!(config.fuel_limit, Some(500_000));
    }

    #[test]
    fn permissive_config_is_loose() {
        let config = WasmRuntimeConfig::permissive();
        assert_eq!(config.memory_max_mb, 256);
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.fuel_limit, None);
    }

    #[test]
    fn custom_config() {
        let config = WasmRuntimeConfig::new(128, 15, Some(2_000_000));
        assert_eq!(config.memory_max_mb, 128);
        assert_eq!(config.timeout_secs, 15);
        assert_eq!(config.fuel_limit, Some(2_000_000));
    }
}
