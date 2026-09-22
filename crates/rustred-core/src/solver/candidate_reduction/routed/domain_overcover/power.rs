//! Checked aggregate-bound propagation for admitted affine momentum maps.
//! This is endpoint bookkeeping, not polynomial or inequality solving.

use super::model::{CandidateDomainRouteCover, CandidateDomainRouteFailure};
use crate::solver::candidate_reduction::power_domain::{DomainPowerBounds, project};

/// A lost positive source subset consumes at least `cost` of both A and R.
/// Every endpoint obeys D' >= D. Affine constants can increase D, so the old
/// upper D bound is deliberately discarded; D' <= A' supplies a safe bound.
pub(super) fn mapped_bounds(
    source: DomainPowerBounds,
    positive_upper: Option<u128>,
    cost: u128,
) -> Option<DomainPowerBounds> {
    let positive_upper = match positive_upper {
        Some(upper) => Some(upper.checked_sub(cost)?),
        None => None,
    };
    if positive_upper.is_some_and(|upper| {
        source
            .min_power_difference
            .is_some_and(|lower| lower > 0 && lower as u128 > upper)
    }) {
        return None;
    }
    Some(DomainPowerBounds {
        // An unrepresentably large inferred upper can be omitted as optional
        // tightening. A supplied public A cap always bounds this by u64::MAX.
        max_positive_power: positive_upper.and_then(|upper| u64::try_from(upper).ok()),
        min_power_difference: source.min_power_difference,
        max_power_difference: positive_upper.and_then(|upper| i64::try_from(upper).ok()),
    })
}

pub(super) fn project_cover<const N: usize>(
    sector: &[bool; N],
    cover: CandidateDomainRouteCover<N>,
) -> Result<Option<CandidateDomainRouteCover<N>>, CandidateDomainRouteFailure> {
    Ok(project(
        sector,
        &cover.lower,
        &cover.upper,
        cover.actual_rank,
        cover.power_bounds,
    )
    .map_err(CandidateDomainRouteFailure::PowerDomain)?
    .map(|projected| CandidateDomainRouteCover {
        lower: projected.lower,
        upper: projected.upper,
        actual_rank: projected.effective_rank,
        power_bounds: projected.powers,
        ..cover
    }))
}
