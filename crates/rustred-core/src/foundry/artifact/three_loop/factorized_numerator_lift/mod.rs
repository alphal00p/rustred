//! Test-only proof probe for factorized inactive-numerator lifting.
//!
//! This module owns no artifact rule, cover, or production dispatch. It asks
//! whether the already authenticated factorization loop bases contain enough
//! exact information to derive, without an oracle, (1) routed affine
//! denominator relations and (2) a bounded-branch, one-factor-at-a-time
//! numerator recurrence. A second, deliberately corner-only probe contracts
//! the resulting cross-factor scalar products by exact spherical moments. It
//! is evidence for a future proof-backed action, not authority to close K=6.

mod derive;
mod error;
mod limits;
mod model;
mod recurrence;
mod tests;

use crate::algebra::ExactAlgebraLimits;

const ARITY: usize = 6;
const LOOP_COUNT: usize = 3;

fn exact_limits() -> ExactAlgebraLimits {
    ExactAlgebraLimits::default()
}
