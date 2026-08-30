//! Exact active-edge momentum rank for the test-only K=6 pressure family.

use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits, rank_of_coefficient_matrix,
};
use crate::family::IntegralFamily;
use crate::sector::Mask;

/// Canonical K4 edge momenta in the stable six-denominator slot order.
pub(super) const EDGE_MOMENTA: [[i64; 3]; 6] = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [-1, 0, 1],
    [1, -1, 0],
    [0, 1, -1],
];

/// Compute the exact rank of the active K4 edge momenta through Symbolica's
/// authenticated matrix-rank boundary.
///
/// Inactive rows are represented by exact zero rows.  Retaining the fixed
/// nonempty `6 x 3` shape makes the empty sector an ordinary rank-zero case
/// and keeps every raw-mask proof on the identical checked primitive.
pub(super) fn active_momentum_rank(
    family: &IntegralFamily,
    sector: &Mask,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    debug_assert_eq!(family.denominator_count(), EDGE_MOMENTA.len());
    debug_assert_eq!(family.loop_count(), EDGE_MOMENTA[0].len());
    debug_assert_eq!(sector.arity(), EDGE_MOMENTA.len());
    let rows = EDGE_MOMENTA
        .iter()
        .zip(sector.active_bits())
        .map(|(momentum, &active)| {
            momentum
                .iter()
                .map(|&component| {
                    family
                        .coefficient_context()
                        .integer(if active { component } else { 0 })
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    rank_of_coefficient_matrix(
        family.coefficient_context(),
        &rows,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .map(|(rank, _statistics)| rank)
}
