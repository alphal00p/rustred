//! Source-support degree bounds for an already verified affine numerator map.
//! No expansion or CAS arithmetic: coefficients were inspected by native map
//! compilation, and the source geometry has already been projected exactly.

use super::model::CandidateDomainRouteFailure;
use crate::sector::symmetry::integral_transport::Prepared;
use std::sync::atomic::{AtomicBool, Ordering};

/// Reused O(N) scratch; rows are unioned once, in increasing source order.
pub(super) struct JointSourceSupport<'a, const N: usize> {
    transport: &'a Prepared,
    source: &'a [bool; N],
    lower: &'a [u64; N],
    upper: &'a [Option<u64>; N],
    inactive_lower: u128,
    rank: Option<u128>,
    selected: [bool; N],
    rows: Vec<usize>,
}

impl<'a, const N: usize> JointSourceSupport<'a, N> {
    pub(super) fn new(
        transport: &'a Prepared,
        source: &'a [bool; N],
        lower: &'a [u64; N],
        upper: &'a [Option<u64>; N],
        rank: Option<u128>,
    ) -> Result<Self, CandidateDomainRouteFailure> {
        let mut inactive_lower = 0;
        for (axis, &on) in source.iter().enumerate() {
            if !on {
                inactive_lower = checked_sum(inactive_lower, lower[axis])?;
            }
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(N)
            .map_err(|_| CandidateDomainRouteFailure::AllocationFailure {
                resource: "joint numerator support rows",
            })?;
        Ok(Self {
            transport,
            source,
            lower,
            upper,
            inactive_lower,
            rank,
            selected: [false; N],
            rows,
        })
    }

    pub(super) fn can_pinch(
        &mut self,
        targets: impl Iterator<Item = usize>,
        cost: u128,
        cancellation: &AtomicBool,
    ) -> Result<bool, CandidateDomainRouteFailure> {
        self.selected.fill(false);
        self.rows.clear();
        for target in targets {
            if cancellation.load(Ordering::Acquire) {
                return Err(CandidateDomainRouteFailure::Cancelled);
            }
            let rows = self.transport.numerator_sources_for_target(target).ok_or(
                CandidateDomainRouteFailure::InvalidAdmittedRoute(
                    "numerator support arity differs",
                ),
            )?;
            for &row in rows {
                if row >= N || self.source[row] {
                    return Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(
                        "numerator support is not a subset of inactive axes",
                    ));
                }
                self.selected[row] = true;
            }
        }
        for (row, &selected) in self.selected.iter().enumerate() {
            if selected {
                self.rows.push(row);
            }
        }
        let cap = column_cap(
            self.lower,
            self.upper,
            self.inactive_lower,
            self.rank,
            &self.rows,
        )?;
        Ok(cap.is_none_or(|cap| cost <= cap))
    }
}

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

    /// Endpoint numerator degree is e_j-B_j on an inactive destination.
    /// A wide optional cap is omitted, never narrowed to a machine maximum.
    pub(super) fn inactive_upper(
        &self,
        target: usize,
        positive_minimum: u128,
    ) -> Result<Option<u64>, CandidateDomainRouteFailure> {
        self.caps[target]
            .map(|cap| {
                cap.checked_sub(positive_minimum)
                    .ok_or(CandidateDomainRouteFailure::InvalidDomain(
                        "pinch degree is below the positive denominator minimum",
                    ))
                    .map(|value| u64::try_from(value).ok())
            })
            .transpose()
            .map(Option::flatten)
    }
}

/// Only rows with a nonzero affine constant can lose total polynomial degree.
/// Prepared::compile already validates these constants through Symbolica;
/// exact is_zero inspects the existing canonical coefficient, without algebra.
pub(super) fn constant_degree_cap<const N: usize>(
    transport: &Prepared,
    source: &[bool; N],
    lower: &[u64; N],
    upper: &[Option<u64>; N],
    rank: Option<u128>,
) -> Result<Option<u128>, CandidateDomainRouteFailure> {
    let constants = transport.verified_map().denominators().constant();
    if constants.len() > N {
        return Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(
            "affine constant arity differs",
        ));
    }
    let mut rows = [0_usize; N];
    let mut count = 0;
    let mut inactive_lower = 0;
    for (row, coefficient) in constants.iter().enumerate() {
        if !source[row] {
            inactive_lower = checked_sum(inactive_lower, lower[row])?;
            if !coefficient.is_zero() {
                rows[count] = row;
                count += 1;
            }
        }
    }
    column_cap(lower, upper, inactive_lower, rank, &rows[..count])
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
    let mut previous = None;
    for &row in rows {
        if row >= N || previous.is_some_and(|previous| previous >= row) {
            return Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(
                "numerator support rows are not unique sorted in-range axes",
            ));
        }
        previous = Some(row);
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
