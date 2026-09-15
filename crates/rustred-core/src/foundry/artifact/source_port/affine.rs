//! Compatibility re-export for source-port diagnostics.
//!
//! The exact domain carrier lives in the lower `foundry::parametric` layer so
//! RuleCell/runtime code does not depend on the artifact bridge. This module
//! remains as a source-port-local import point for existing geometry callers.

pub use crate::foundry::parametric::AffineApplicationDomain;
