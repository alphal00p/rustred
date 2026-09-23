//! Geometric starting-root proposals, never rule or reachability authority.
use super::RootRegionInput;
use crate::family::{IntegralKey, IntegralKeyError};
use crate::solver::{DomainPowerBounds, DomainPowerError, DomainPowerSummary};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryWitnessLimits {
    /// Counts every native projection, including validation of both inputs.
    /// At most N+4 calls are needed. No call is made after this allowance.
    pub max_projections: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryWitnessOutcome {
    /// The exact mathematical intersection is empty, not merely lacking i64 keys.
    Empty { projection_calls: usize },
    /// A deterministic representable point, lexicographically first in local
    /// coordinates among points with i64-valued physical powers.
    /// It still requires original-policy admission and exact native tracing.
    Point {
        key: IntegralKey,
        projection_calls: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryWitnessError {
    Geometry(DomainPowerError),
    /// A nonempty mathematical intersection has no representable IntegralKey.
    UnrepresentableIntegralKey,
    ProjectionAllowance {
        used: usize,
        limit: usize,
    },
    Cancelled,
    IntegralKey(IntegralKeyError),
    Invariant(&'static str),
}

impl fmt::Display for EntryWitnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Geometry(error) => write!(f, "invalid entry-witness geometry: {error}"),
            Self::UnrepresentableIntegralKey => write!(
                f,
                "nonempty entry intersection has no i64-valued integral key"
            ),
            Self::ProjectionAllowance { used, limit } => write!(
                f,
                "entry-witness projection allowance exhausted: {used}/{limit}"
            ),
            Self::Cancelled => write!(f, "entry-witness selection cancelled"),
            Self::IntegralKey(error) => error.fmt(f),
            Self::Invariant(reason) => write!(f, "entry-witness invariant failed: {reason}"),
        }
    }
}
impl std::error::Error for EntryWitnessError {}

struct Projection<'a> {
    calls: usize,
    limits: EntryWitnessLimits,
    cancellation: &'a AtomicBool,
}
impl Projection<'_> {
    fn check(&self) -> Result<(), EntryWitnessError> {
        if self.cancellation.load(Ordering::Acquire) {
            Err(EntryWitnessError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn project<const N: usize>(
        &mut self,
        support: [bool; N],
        lower: &[u64],
        upper: &[Option<u64>],
        rank: Option<u32>,
        powers: DomainPowerBounds,
    ) -> Result<DomainPowerSummary<N>, EntryWitnessError> {
        self.check()?;
        if self.calls >= self.limits.max_projections {
            return Err(EntryWitnessError::ProjectionAllowance {
                used: self.calls,
                limit: self.limits.max_projections,
            });
        }
        self.calls += 1; // The preceding allowance check prevents overflow.
        DomainPowerSummary::try_new(support, lower, upper, rank, powers)
            .map_err(EntryWitnessError::Geometry)
    }
}

fn tighter_upper<T: Ord>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
fn tighter_lower<T: Ord>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

/// Select a point of one entry-region/proposal intersection, retaining every
/// box, R, A and D constraint. Multiple entry regions must be passed separately:
/// replacing their union with a rectangular hull would admit holes.
///
/// Both inputs are validated before any disjointness shortcut. An empty valid
/// intersection is not malformed input. The inputs may be unbounded; finiteness
/// and original-entry authority remain the caller's separate policy checks.
///
/// Native exact disjoint-sum projection owns feasibility. Successive projected
/// minima are fixed one at a time, never combined without reprojecting. i64
/// representability restricts only this local point selection, not a domain or
/// descendant. No guard outcome, source validity, MissingRule, terminal or
/// coverage claim follows from returning a point.
pub fn pick_entry_intersection_witness<const N: usize>(
    entry: &RootRegionInput<N>,
    proposal: &RootRegionInput<N>,
    limits: EntryWitnessLimits,
    cancellation: &AtomicBool,
) -> Result<EntryWitnessOutcome, EntryWitnessError> {
    let mut projection = Projection {
        calls: 0,
        limits,
        cancellation,
    };
    let entry_summary = projection.project(
        entry.support,
        &entry.lower,
        &entry.upper,
        entry.rank,
        entry.powers,
    )?;
    let proposal_summary = projection.project(
        proposal.support,
        &proposal.lower,
        &proposal.upper,
        proposal.rank,
        proposal.powers,
    )?;
    projection.check()?;
    if entry.support != proposal.support || entry_summary.is_empty() || proposal_summary.is_empty()
    {
        return Ok(EntryWitnessOutcome::Empty {
            projection_calls: projection.calls,
        });
    }
    // Endpoint intersection, not a second feasibility or polyhedral service.
    let mut lower: [u64; N] = std::array::from_fn(|i| entry.lower[i].max(proposal.lower[i]));
    let mut upper: [Option<u64>; N] =
        std::array::from_fn(|i| tighter_upper(entry.upper[i], proposal.upper[i]));
    let rank = tighter_upper(entry.rank, proposal.rank);
    let powers = DomainPowerBounds {
        max_positive_power: tighter_upper(
            entry.powers.max_positive_power,
            proposal.powers.max_positive_power,
        ),
        min_power_difference: tighter_lower(
            entry.powers.min_power_difference,
            proposal.powers.min_power_difference,
        ),
        max_power_difference: tighter_upper(
            entry.powers.max_power_difference,
            proposal.powers.max_power_difference,
        ),
    };
    if (0..N).any(|i| upper[i].is_some_and(|u| lower[i] > u))
        || powers
            .min_power_difference
            .zip(powers.max_power_difference)
            .is_some_and(|(l, u)| l > u)
    {
        return Ok(EntryWitnessOutcome::Empty {
            projection_calls: projection.calls,
        });
    }
    let mut summary = projection.project(entry.support, &lower, &upper, rank, powers)?;
    let Some(extrema) = summary.extrema() else {
        return Ok(EntryWitnessOutcome::Empty {
            projection_calls: projection.calls,
        });
    };
    lower = *extrema.lower();
    upper = *extrema.upper();

    // Intersect with representability before choosing any coordinate. A first
    // unconstrained minimum might otherwise force a later coordinate to overflow
    // even when another fully representable point exists.
    let mut narrowed = false;
    for axis in 0..N {
        let maximum = if entry.support[axis] {
            i64::MAX as u64 - 1
        } else {
            i64::MIN.unsigned_abs()
        };
        if lower[axis] > maximum {
            return Err(EntryWitnessError::UnrepresentableIntegralKey);
        }
        if upper[axis].is_none_or(|value| value > maximum) {
            upper[axis] = Some(maximum);
            narrowed = true;
        }
    }
    if narrowed {
        summary = projection.project(entry.support, &lower, &upper, rank, powers)?;
        let Some(extrema) = summary.extrema() else {
            return Err(EntryWitnessError::UnrepresentableIntegralKey);
        };
        lower = *extrema.lower();
        upper = *extrema.upper();
    }

    for axis in 0..N {
        projection.check()?;
        let value = summary
            .extrema()
            .ok_or(EntryWitnessError::Invariant("lost nonempty domain"))?
            .lower()[axis];
        if upper[axis] == Some(value) && lower[axis] == value {
            continue;
        }
        lower[axis] = value;
        upper[axis] = Some(value);
        summary = projection.project(entry.support, &lower, &upper, rank, powers)?;
        let extrema = summary.extrema().ok_or(EntryWitnessError::Invariant(
            "tight endpoint has no integer completion",
        ))?;
        lower = *extrema.lower();
        upper = *extrema.upper();
    }
    projection.check()?;
    let mut physical = [0_i64; N];
    for axis in 0..N {
        if upper[axis] != Some(lower[axis]) {
            return Err(EntryWitnessError::Invariant("unfixed witness coordinate"));
        }
        let value = if entry.support[axis] {
            i128::from(lower[axis]) + 1
        } else {
            -i128::from(lower[axis])
        };
        physical[axis] = i64::try_from(value)
            .map_err(|_| EntryWitnessError::Invariant("representable witness overflowed"))?;
    }
    // The final projected summary is already this exact singleton. Reusing it
    // checks both original domains without an uncounted extra projection.
    if !entry_summary.contains(&summary) || !proposal_summary.contains(&summary) {
        return Err(EntryWitnessError::Invariant(
            "selected point escaped an original domain",
        ));
    }
    let key = IntegralKey::try_new(physical).map_err(EntryWitnessError::IntegralKey)?;
    Ok(EntryWitnessOutcome::Point {
        key,
        projection_calls: projection.calls,
    })
}

#[cfg(test)]
mod tests;
