//! Source-support degree bounds for an already verified affine numerator map.
//! No expansion or CAS arithmetic: coefficients were inspected by native map
//! compilation, and the source geometry has already been projected exactly.

use super::model::CandidateDomainRouteFailure;
use crate::sector::symmetry::integral_transport::Prepared;

/// Independent necessary degree caps, not a jointly attainable exponent set.
pub(super) struct NumeratorDegrees<const N: usize> {
    caps: [Option<u128>; N],
}

impl<const N: usize> NumeratorDegrees<N> {
    pub(super) fn from_source(
        transport: &Prepared,
        source: &[bool; N],
        root: &[bool; N],
        lower: &[u64; N],
        upper: &[Option<u64>; N],
        rank: Option<u128>,
    ) -> Result<Self, CandidateDomainRouteFailure> {
        let mut inactive_lower = 0_u128;
        for (axis, &on) in source.iter().enumerate() {
            if !on {
                inactive_lower = checked_sum(inactive_lower, lower[axis])?;
            }
        }
        let mut caps = [None; N];
        for (target, &on) in root.iter().enumerate() {
            if on {
                let rows = transport.numerator_sources_for_target(target).ok_or(
                    CandidateDomainRouteFailure::InvalidAdmittedRoute(
                        "numerator support arity differs",
                    ),
                )?;
                caps[target] = column_cap(lower, upper, inactive_lower, rank, rows)?;
            }
        }
        Ok(Self { caps })
    }

    pub(super) fn surviving_lower(&self, target: usize, source_lower: u64) -> u64 {
        self.caps[target].map_or(0, |cap| {
            // The positive difference is bounded by source_lower, so this
            // conversion cannot narrow a wide finite cap or fabricate infinity.
            u128::from(source_lower).saturating_sub(cap) as u64
        })
    }

    pub(super) fn can_pinch(&self, target: usize, source_lower: u64) -> bool {
        // A pinch consumes at least source_lower+1, without overflowing u64.
        self.caps[target].is_none_or(|cap| cap > u128::from(source_lower))
    }
}

fn checked_sum(sum: u128, value: u64) -> Result<u128, CandidateDomainRouteFailure> {
    sum.checked_add(u128::from(value))
        .ok_or(CandidateDomainRouteFailure::CountOverflow {
            resource: "numerator support degree",
        })
}

fn column_cap<const N: usize>(
    lower: &[u64; N],
    upper: &[Option<u64>; N],
    inactive_lower: u128,
    rank: Option<u128>,
    rows: &[usize],
) -> Result<Option<u128>, CandidateDomainRouteFailure> {
    let mut relevant_lower = 0_u128;
    let mut relevant_upper = Some(0_u128);
    for &row in rows {
        relevant_lower = checked_sum(relevant_lower, lower[row])?;
        relevant_upper = match (relevant_upper, upper[row]) {
            (Some(sum), Some(value)) => Some(checked_sum(sum, value)?),
            _ => None,
        };
    }
    let irrelevant_lower = inactive_lower.checked_sub(relevant_lower).ok_or(
        CandidateDomainRouteFailure::InvalidAdmittedRoute(
            "numerator support is not a subset of inactive axes",
        ),
    )?;
    let rank_available = rank
        .map(|rank| {
            rank.checked_sub(irrelevant_lower)
                .ok_or(CandidateDomainRouteFailure::InvalidDomain(
                    "projected rank is below mandatory inactive degree",
                ))
        })
        .transpose()?;
    Ok(match (relevant_upper, rank_available) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    })
}

#[cfg(test)]
mod tests;
