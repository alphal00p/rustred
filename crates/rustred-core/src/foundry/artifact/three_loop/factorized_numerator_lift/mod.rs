//! Test-only angular-moment oracle for production factorized routing.
//!
//! The topology-generic affine derivation and one-factor recurrence live in
//! [`crate::foundry::artifact::factorized_numerator_lift`]. This module keeps
//! only the deliberately K6, undotted-corner spherical-moment evaluation used
//! to validate that production routing. It owns no artifact rule, cover, or
//! production dispatch and does not confer authority to close K6.

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
