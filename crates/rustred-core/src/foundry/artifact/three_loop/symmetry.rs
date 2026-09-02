use crate::algebra::CoefficientContext;
use crate::family::IntegralFamily;
use crate::sector::OrderingPolicy;
use crate::sector::symmetry::permutation::compile;
use crate::sector::symmetry::{
    CanonicalizationLimits, Canonicalizer, CoefficientMatrix, Limits as SymmetryLimits,
    MomentumMap, verify,
};

use super::super::ArtifactError;

/// Authenticate two exact momentum maps whose induced denominator
/// permutations generate the complete order-24 `S4` edge action.
pub(crate) fn canonical_s4(family: &IntegralFamily) -> Result<Canonicalizer, ArtifactError> {
    canonical_s4_with_ordering(family, OrderingPolicy::default())
}

/// Authenticate the complete order-24 `S4` edge action while selecting orbit
/// representatives with the caller's persisted ordering policy.
///
/// A coordinate-priority policy need not be invariant under `S4`. Reusing a
/// default-order canonicalizer with a custom-order owner would therefore
/// break the reducer chain `parent > raw child >= canonical child`, despite
/// using the same exact symmetry group.
pub(crate) fn canonical_s4_with_ordering(
    family: &IntegralFamily,
    ordering: OrderingPolicy,
) -> Result<Canonicalizer, ArtifactError> {
    let coefficients = family.coefficient_context();
    let generators = [
        // Exchange the distinguished vertex with vertex 1.
        vacuum_map(coefficients, [-1, 0, 0, -1, 1, 0, -1, 0, 1])?,
        // Cycle the three loop-coordinate vertices.
        vacuum_map(coefficients, [0, 1, 0, 0, 0, 1, 1, 0, 0])?,
    ]
    .into_iter()
    .map(|map| {
        let verified = verify(family, family, map, SymmetryLimits::default())?;
        Ok(compile(family, verified)?)
    })
    .collect::<Result<Vec<_>, ArtifactError>>()?;
    Ok(Canonicalizer::try_new(
        ordering,
        generators,
        CanonicalizationLimits::default(),
    )?)
}

fn vacuum_map(
    coefficients: &CoefficientContext,
    entries: [i64; 9],
) -> Result<MomentumMap, ArtifactError> {
    Ok(MomentumMap::new(
        CoefficientMatrix::try_new(
            3,
            3,
            entries.into_iter().map(|entry| coefficients.integer(entry)),
        )?,
        CoefficientMatrix::try_new(3, 0, [])?,
        CoefficientMatrix::try_new(0, 0, [])?,
    ))
}
