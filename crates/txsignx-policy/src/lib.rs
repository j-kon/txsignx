//! Deterministic interpretation of inspection facts, not a global safety verdict.
mod config;
mod error;
mod model;
pub use config::PolicyConfig;
pub use error::PolicyError;
pub use model::*;
mod engine;
pub use engine::{PolicyContext, PolicyEngine, PolicyRule};
mod registry;
pub mod rules;
pub use registry::{DeferredRuleMetadata, RuleCatalog, rule_catalog};
