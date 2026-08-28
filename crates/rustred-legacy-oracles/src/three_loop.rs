//! Structural foundation for the equal-mass three-loop tetrahedron vacuum.
//!
//! This module defines a complete six-denominator scalar-product basis for
//! three loop momenta.  It deliberately does not claim a complete three-loop
//! reduction: generic boundary formulae and a proven finite-coverage solver
//! are later milestones.

use crate::{Denominator, FamilyError, VacuumFamily};
use rustred::{CoefficientContext, ExactRational};

/// Momentum routing vectors for the six edges of the tetrahedron.
///
/// In order these are
/// `k1, k2, k3, k3-k1, k1-k2, k2-k3`.
pub const THREE_LOOP_TETRAHEDRON_ROUTINGS: [[i8; 3]; 6] = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [-1, 0, 1],
    [1, -1, 0],
    [0, 1, -1],
];

/// The corresponding edges of `K4`, with vertex zero chosen as the momentum
/// origin and `k1,k2,k3` assigned to vertices one, two, and three.
pub const THREE_LOOP_TETRAHEDRON_EDGES: [[usize; 2]; 6] =
    [[0, 1], [0, 2], [0, 3], [1, 3], [1, 2], [2, 3]];

/// Proven edge permutations induced by adjacent transpositions of the four
/// tetrahedron vertices.
///
/// The generators are not graph guesses.  They arise from the following
/// unit-Jacobian loop transformations:
///
/// ```text
/// (0 1): (k1,k2,k3) -> (-k1, k2-k1, k3-k1)
/// (1 2): (k1,k2,k3) -> ( k2, k1,    k3   )
/// (2 3): (k1,k2,k3) -> ( k1, k3,    k2   )
/// ```
///
/// These adjacent transpositions generate the full vertex group `S4`; its
/// induced action on the six edges has 24 elements.  `VacuumFamily::new`
/// independently verifies every element through an exact unimodular momentum
/// transformation.
pub const THREE_LOOP_TETRAHEDRON_SYMMETRY_GENERATORS: [[usize; 6]; 3] =
    [[0, 4, 3, 2, 1, 5], [1, 0, 2, 5, 4, 3], [0, 2, 1, 4, 3, 5]];

/// Construct the equal-mass three-loop tetrahedron (Mercedes) family.
///
/// The Euclidean denominator convention is
///
/// ```text
/// D_i = q_i^2 + m2,
/// ```
///
/// with `q_i` in [`THREE_LOOP_TETRAHEDRON_ROUTINGS`].  Six denominators are
/// exactly the `3*(3+1)/2` scalar products required for a complete vacuum
/// basis, so there is no irreducible scalar product.
pub fn equal_mass_three_loop_tetrahedron() -> Result<VacuumFamily, FamilyError> {
    equal_mass_three_loop_tetrahedron_in_context(CoefficientContext::new(["d", "m2"]))
}

/// Construct the standard equal-mass tetrahedron in a caller-supplied exact
/// coefficient context.
///
/// Higher-loop component closure uses this surface so transported three-loop
/// coefficients and the induced IBP table share one Symbolica variable map.
/// The context must define both `d` and `m2`.
pub fn equal_mass_three_loop_tetrahedron_in_context(
    coefficients: CoefficientContext,
) -> Result<VacuumFamily, FamilyError> {
    let mass = coefficients
        .parameter("m2")
        .ok_or_else(|| FamilyError::UnknownCoefficientParameter("m2".to_owned()))?;
    let denominators = THREE_LOOP_TETRAHEDRON_ROUTINGS
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
    let symmetry_generators = THREE_LOOP_TETRAHEDRON_SYMMETRY_GENERATORS
        .iter()
        .map(|permutation| permutation.to_vec())
        .collect();

    VacuumFamily::new(
        "equal_mass_three_loop_tetrahedron",
        3,
        coefficients,
        "d",
        denominators,
        symmetry_generators,
    )
}

/// Descriptive alias for [`equal_mass_three_loop_tetrahedron`].
pub fn equal_mass_three_loop_vacuum() -> Result<VacuumFamily, FamilyError> {
    equal_mass_three_loop_tetrahedron()
}
