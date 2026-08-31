//! Bounded discovery prototype for fraction-free exact-circuit replay.
//!
//! This test-only layer separates guards intrinsic to the ordinary sources
//! from guards introduced by one field-elimination path. It deliberately
//! does not produce a rule, owner, artifact, or closure certificate.

mod budget;
mod compile;
mod model;

pub(super) use compile::{try_clear_exact_circuit, try_compile_final_target_guard};
pub(super) use model::{ClearedCircuitError, ClearedCircuitLimits, ClearedSemanticGuardOrigin};
