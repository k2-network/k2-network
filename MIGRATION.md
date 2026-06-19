# Iroh 0.95 → 1.0.0 Migration Guide

## Status

This migration is **IN PROGRESS**. The following changes have been applied to the source code. You must resolve external dependencies before the project will compile.

## Critical Blockers

### 1. Missing External Path Dependencies

The following local path dependencies are **MISSING** from this machine and must be provided by you:

| Dependency | Expected Path | Status | Action Required |
|------------|---------------|--------|-----------------|
| `iroh-content-discovery` | `../../iroh-test-space/iroh-content-discovery` | NOT FOUND | Provide the crate or remove the feature |
| `amulet` | `../../amulet` | NOT FOUND | Provide the crate or build without `credential-store` feature |

**Resolution options:**

**Option A - Provide the dependencies:**
```bash
# Clone or copy the dependencies to the expected paths
cp -r /path/to/iroh-content-discovery ../../iroh-test-space/
cp -r /path/to/amulet ../../amulet/
```

**Option B - Build without the missing features:**
```bash
# Build k2-core without content-discovery and credential-store
cd k2-core && cargo build --no-default-features
```

### 2. No Rust Toolchain Configured

Run the following to configure Rust:
```bash
rustup default stable
rustup target add aarch64-linux-android  # For Android builds
```

### 3. iroh-content-discovery Compatibility

The `iroh-content-discovery` crate was built for iroh 0.95. It **must be upgraded** to iroh 1.0.0 compatibility, or replaced. The tracker-based discovery code in K2 has been feature-gated behind `content-discovery`.

## What Changed

### Cargo.toml Changes

**k2-core/Cargo.toml:**
- `iroh`: `0.95.0` → `1.0.0` (features changed: `discovery-pkarr-dht`, `discovery-local-network` removed; `tls-ring`, `portmapper` added)
- `iroh-base`: `0.95` → `1.0.0`
- `iroh-blobs`: `0.97` → `0.103`
- `iroh-gossip`: `0.95.0` → `0.101`
- `iroh-docs`: `0.95.0` → `0.101`
- Added: `iroh-mainline-address-lookup`, `iroh-mdns-address-lookup`
- `iroh-content-discovery`: now `optional = true`, gated by `content-discovery` feature
- `amulet`: now `optional = true`, gated by `credential-store` feature
- Added `[features]` section with `default = ["content-discovery", "credential-store"]`

**k2-app-tauri/src-tauri/Cargo.toml:**
- `iroh-gossip`: `0.95.0` → `0.101`

### API Migration Summary

| API (0.95) | API (1.0.0) | Files Affected |
|------------|-------------|----------------|
| `iroh::discovery::*` | `iroh::address_lookup::*` + separate crates | `lib.rs`, `sync.rs` |
| `DhtDiscovery` | `iroh_mainline_address_lookup::DhtLookup` | `lib.rs` |
| `MdnsDiscovery` | `iroh_mdns_address_lookup::MdnsLookup` | `lib.rs` |
| `ConcurrentDiscovery` | `Builder::address_lookup()` composes | `lib.rs` |
| `Endpoint::builder()` | `Endpoint::builder(presets::N0)` | `lib.rs` |
| `SecretKey::generate(&mut rand::rng())` | `SecretKey::generate()` | `identity.rs` |
| `ConnectionInfo` | `WeakConnectionHandle` | `sync.rs` (if used) |
| `PathInfo` | `Path` | `sync.rs` (if used) |

### Files Modified

- [x] `k2-core/Cargo.toml`
- [x] `k2-app-tauri/src-tauri/Cargo.toml`
- [x] `k2-core/src/identity.rs`
- [ ] `k2-core/src/lib.rs` (pending agent)
- [ ] `k2-core/src/blobs.rs` (pending agent)
- [ ] `k2-core/src/docs.rs` (pending agent)
- [ ] `k2-core/src/sync.rs` (pending agent)
- [ ] `k2-app-tauri/src-tauri/src/lib.rs` (pending agent)

## Verification Steps

After resolving blockers, run:

```bash
# 1. Update dependencies
cargo update

# 2. Check compilation
cargo check --all

# 3. Run tests
cd k2-core && cargo test
cd ../k2-app-tauri/src-tauri && cargo test

# 4. Build frontend
cd ../../k2-app-tauri
npm install
npm run build

# 5. Build Tauri app
npm run tauri build
```

## Known Issues

1. **Tracker-based discovery disabled by default**: The `content-discovery` feature gates the tracker-based peer discovery (`subscribe_topic_with_discovery`). Without this feature, topics are joined without tracker lookup.
2. **OS Secure Store disabled by default**: The `credential-store` feature gates the Amulet/WindowsStore integration. Without it, identity is always stored in the encrypted backup file.
3. **Android build untested**: The `.cargo/config.toml` NDK configuration may need updates for the new iroh versions.

## References

- [Iroh 1.0.0 Release Notes](https://github.com/n0-computer/iroh/releases/tag/v1.0.0)
- [Iroh Changelog](https://github.com/n0-computer/iroh/blob/v1.0.0/CHANGELOG.md)
- [Iroh Documentation](https://docs.iroh.computer/)
