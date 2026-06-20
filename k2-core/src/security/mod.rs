//! Security and trust policy module for K2.
//!
//! # Architecture
//!
//! - **[`trust`]** — Core types: [`EffectiveTrustClass`], [`TrustDecision`],
//!   [`AuthorityCeiling`], [`TrustPolicy`] trait.
//! - **[`policy`]** — Concrete implementations: [`HostTrustPolicy`].
//! - **[`invalidation`]** — Publish/subscribe bus for trust-change
//!   notifications.
//! - **[`error`]** — [`TrustError`] and [`TrustResult`].

pub mod error;
pub mod invalidation;
pub mod policy;
pub mod trust;

// ---------------------------------------------------------------------------
// Re-exports — everything public a consumer needs.
// ---------------------------------------------------------------------------

pub use error::{TrustError, TrustResult};
pub use invalidation::{InvalidationBus, TrustChange, TrustChangeListener};
pub use policy::HostTrustPolicy;
pub use trust::{
    AuthorityCeiling, EffectKind, EffectiveTrustClass, ResourceCeiling, TrustDecision,
    TrustPolicy, TrustPolicyInput, TrustProvenance, TrustSource,
};
