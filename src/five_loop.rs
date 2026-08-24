//! Structural equal-mass five-loop banana vacuum family.
//!
//! Six physical lines carry `k1`, ..., `k5`, and `k1+...+k5`.  Nine
//! deterministic auxiliary scalar products complete the 15-entry vacuum
//! basis.  This establishes exact denominator lowering and all 25 raw IBPs;
//! it does not claim a five-loop reduction or master basis.

use crate::{CoefficientContext, Denominator, ExactRational, FamilyError, VacuumFamily};

pub const FIVE_LOOP_BANANA_ROUTINGS: [[i8; 5]; 6] = [
    [1, 0, 0, 0, 0],
    [0, 1, 0, 0, 0],
    [0, 0, 1, 0, 0],
    [0, 0, 0, 1, 0],
    [0, 0, 0, 0, 1],
    [1, 1, 1, 1, 1],
];

pub fn equal_mass_five_loop_banana() -> Result<VacuumFamily, FamilyError> {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients
        .parameter("m2")
        .expect("the built-in context defines m2");
    let propagators = FIVE_LOOP_BANANA_ROUTINGS
        .iter()
        .map(|routing| {
            Denominator::propagator(
                routing
                    .iter()
                    .map(|&component| ExactRational::from(i64::from(component)))
                    .collect(),
                mass.clone(),
            )
        })
        .collect();
    VacuumFamily::new_with_standard_auxiliaries(
        "equal_mass_five_loop_banana",
        5,
        coefficients,
        "d",
        propagators,
        Vec::new(),
    )
}
