//! Exact finite candidate retention in original integral coordinates.
//!
//! This does not infer independent masters, cut sources/successors, or prove
//! recursive closure. A complete finite in-scope case may simply be retained.

use std::collections::BTreeSet;
use std::fmt;

use crate::solver::{AffineGeometryError, Case, CoordinateCase, Integral, Power};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FiniteCasePolicy {
    #[default]
    SearchFinite,
    /// Retain every admitted rank-bounded point once all positive axes are
    /// fixed. Requires an explicit numerator rank; leaves dots unbounded.
    RetainRankFinite,
}

impl FiniteCasePolicy {
    pub const EXPECTED_VALUES: &'static str = "search or retain-rank-finite";

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SearchFinite => "search",
            Self::RetainRankFinite => "retain-rank-finite",
        }
    }
}

impl std::str::FromStr for FiniteCasePolicy {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "search" => Ok(Self::SearchFinite),
            "retain-rank-finite" => Ok(Self::RetainRankFinite),
            _ => Err(Self::EXPECTED_VALUES),
        }
    }
}

/// Aggregate per-sector enumeration/storage budgets, not semantic bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiniteCaseLimits {
    /// Fully fixed simplex points tested, including rejected/duplicate points.
    pub max_visited_points: usize,
    /// Distinct concrete points retained across all finite cases in a sector.
    pub max_retained_terminals: usize,
}

impl Default for FiniteCaseLimits {
    fn default() -> Self {
        Self {
            max_visited_points: 1_000_000,
            max_retained_terminals: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FiniteRetentionError {
    MissingNumeratorRank,
    VisitedPointBudget {
        requested: usize,
        limit: usize,
    },
    RetainedTerminalBudget {
        requested: usize,
        limit: usize,
    },
    /// Fail conservatively rather than truncate an unrepresentable simplex.
    CompactOverflow {
        axis: usize,
        remaining_rank: u32,
    },
    Geometry(AffineGeometryError),
}

impl fmt::Display for FiniteRetentionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNumeratorRank => {
                f.write_str("retain-rank-finite requires an explicit maximum numerator rank")
            }
            Self::VisitedPointBudget { requested, limit } => write!(
                f,
                "finite-case enumeration needs {requested} visited points, exceeding limit {limit}"
            ),
            Self::RetainedTerminalBudget { requested, limit } => write!(
                f,
                "finite-case retention needs {requested} terminals, exceeding limit {limit}"
            ),
            Self::CompactOverflow {
                axis,
                remaining_rank,
            } => write!(
                f,
                "finite-case rank {remaining_rank} on inactive axis {axis} may exceed compact negative powers; no truncation is permitted"
            ),
            Self::Geometry(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for FiniteRetentionError {}

#[derive(Debug, Default)]
pub(super) struct Retention<const N: usize> {
    points: BTreeSet<Integral<N>>,
    visited: usize,
}

impl<const N: usize> Retention<N> {
    pub(super) fn visited(&self) -> usize {
        self.visited
    }

    pub(super) fn len(&self) -> usize {
        self.points.len()
    }

    pub(super) fn into_points(self) -> Vec<Integral<N>> {
        self.points.into_iter().collect()
    }

    /// Return false only when a positive original coordinate remains free.
    /// Successful true includes proved in-scope emptiness. Commit a complete
    /// case atomically: neither points nor counters change after an error.
    pub(super) fn retain_case(
        &mut self,
        case: &Case<N>,
        indices: &[usize; N],
        sector: &[bool; N],
        maximum: u32,
        limits: FiniteCaseLimits,
    ) -> Result<bool, FiniteRetentionError> {
        if sector
            .iter()
            .zip(case.fixed())
            .any(|(&active, value)| active && value.is_none())
        {
            return Ok(false);
        }
        let Some(admitted) = case
            .intersect(&[], indices, sector)
            .map_err(FiniteRetentionError::Geometry)?
        else {
            return Ok(true);
        };
        if !admitted.is_in_sector(sector) {
            return Ok(true);
        }
        let fixed_rank: u128 = admitted
            .fixed()
            .iter()
            .flatten()
            .filter(|&&n| n < 0)
            .map(|n| u128::from(n.unsigned_abs()))
            .sum();
        let Some(remaining) = u128::from(maximum).checked_sub(fixed_rank) else {
            return Ok(true);
        };
        let free: Vec<_> = (0..N)
            .filter(|&axis| !sector[axis] && admitted.fixed()[axis].is_none())
            .collect();
        if let Some(&axis) = free.first()
            && remaining > u128::from(Power::MIN.unsigned_abs())
        {
            return Err(FiniteRetentionError::CompactOverflow {
                axis,
                remaining_rank: remaining as u32,
            });
        }
        let mut enumeration = Enumeration {
            parent: &admitted,
            existing: &self.points,
            points: BTreeSet::new(),
            visited: self.visited,
            limits,
        };
        let mut fixed = *admitted.fixed();
        enumeration.visit(&free, &mut fixed, remaining as u32)?;
        let Enumeration {
            points, visited, ..
        } = enumeration;
        self.visited = visited;
        self.points.extend(points);
        Ok(true)
    }
}

struct Enumeration<'a, const N: usize> {
    parent: &'a Case<N>,
    existing: &'a BTreeSet<Integral<N>>,
    points: BTreeSet<Integral<N>>,
    visited: usize,
    limits: FiniteCaseLimits,
}

impl<const N: usize> Enumeration<'_, N> {
    fn visit(
        &mut self,
        free: &[usize],
        fixed: &mut [Option<i16>; N],
        remaining: u32,
    ) -> Result<(), FiniteRetentionError> {
        if let Some((&axis, rest)) = free.split_first() {
            for degree in 0..=remaining {
                fixed[axis] = Some(-(degree as i16));
                self.visit(rest, fixed, remaining - degree)?;
            }
            fixed[axis] = None;
            return Ok(());
        }
        let requested =
            self.visited
                .checked_add(1)
                .ok_or(FiniteRetentionError::VisitedPointBudget {
                    requested: usize::MAX,
                    limit: self.limits.max_visited_points,
                })?;
        if requested > self.limits.max_visited_points {
            return Err(FiniteRetentionError::VisitedPointBudget {
                requested,
                limit: self.limits.max_visited_points,
            });
        }
        self.visited = requested;
        let point = CoordinateCase::new(*fixed)
            .expect("finite enumeration preflighted every compact coordinate");
        if !self
            .parent
            .contains(&Case::from(point))
            .map_err(FiniteRetentionError::Geometry)?
        {
            return Ok(());
        }
        let integral = point.integral();
        if self.existing.contains(&integral) || self.points.contains(&integral) {
            return Ok(());
        }
        let requested = self
            .existing
            .len()
            .checked_add(self.points.len())
            .and_then(|count| count.checked_add(1))
            .ok_or(FiniteRetentionError::RetainedTerminalBudget {
                requested: usize::MAX,
                limit: self.limits.max_retained_terminals,
            })?;
        if requested > self.limits.max_retained_terminals {
            return Err(FiniteRetentionError::RetainedTerminalBudget {
                requested,
                limit: self.limits.max_retained_terminals,
            });
        }
        self.points.insert(integral);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
