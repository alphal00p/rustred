//! Finite starting-root constraints, independent of saved owner search bounds.
use super::CandidateRoutedError;
use crate::family::IntegralKey;
use crate::sector::Mask;
use crate::solver::candidate_reduction::evaluator::validate_entry_rank;
use crate::solver::{DomainPowerBounds, DomainPowerError, DomainPowerSummary};
use std::fmt;

/// One fixed-support region. Coordinates are `n - 1` on positive axes and
/// `-n` on absent axes. Box, rank and power bounds are intersected exactly.
#[derive(Clone, Debug)]
pub struct RootRegionInput<const N: usize> {
    pub support: [bool; N],
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub rank: Option<u32>,
    pub powers: DomainPowerBounds,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootAdmissionError {
    EmptyPolicy,
    RegionAllowance {
        limit: usize,
    },
    InvalidRegion {
        region: usize,
        error: DomainPowerError,
    },
    EmptyRegion {
        region: usize,
    },
    UnboundedRegion {
        region: usize,
        axis: usize,
    },
    Allocation,
    WrongTargetArity {
        expected: usize,
        actual: usize,
    },
    InvalidTarget {
        error: DomainPowerError,
    },
    OutsideSelectedSupport {
        support: Mask,
    },
    OutsideStartingDomain {
        target: IntegralKey,
    },
}

impl fmt::Display for RootAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPolicy => write!(f, "finite root admission requires at least one region"),
            Self::RegionAllowance { limit } => {
                write!(f, "finite root admission exceeds {limit} regions")
            }
            Self::InvalidRegion { region, error } => {
                write!(f, "invalid root region {region}: {error}")
            }
            Self::EmptyRegion { region } => write!(f, "root region {region} is empty"),
            Self::UnboundedRegion { region, axis } => {
                write!(f, "root region {region} is unbounded on axis {axis}")
            }
            Self::Allocation => write!(f, "cannot allocate finite root admission data"),
            Self::WrongTargetArity { expected, actual } => {
                write!(f, "root target has {actual} powers; expected {expected}")
            }
            Self::InvalidTarget { error } => write!(f, "invalid root target: {error}"),
            Self::OutsideSelectedSupport { support } => {
                write!(f, "root support {support:?} is not selected")
            }
            Self::OutsideStartingDomain { target } => write!(
                f,
                "root target {:?} is outside the finite starting domain",
                target.powers()
            ),
        }
    }
}
impl std::error::Error for RootAdmissionError {}

/// Immutable union of nonempty, provably finite fixed-support regions.
///
/// A selected support need not have a literal Apply owner: verified routing may
/// map it to an owner, or report the existing MissingOwner frontier. This policy
/// does not authenticate a topology census or certify rules or family closure.
#[derive(Debug)]
pub struct FiniteRootAdmission<const N: usize> {
    regions: Vec<DomainPowerSummary<N>>,
}

impl<const N: usize> FiniteRootAdmission<N> {
    pub fn try_new(
        input: impl IntoIterator<Item = RootRegionInput<N>>,
        max_regions: usize,
    ) -> Result<Self, RootAdmissionError> {
        let mut regions = Vec::new();
        for (region, input) in input.into_iter().enumerate() {
            if region >= max_regions {
                return Err(RootAdmissionError::RegionAllowance { limit: max_regions });
            }
            let summary = DomainPowerSummary::try_new(
                input.support,
                &input.lower,
                &input.upper,
                input.rank,
                input.powers,
            )
            .map_err(|error| RootAdmissionError::InvalidRegion { region, error })?;
            let extrema = summary
                .extrema()
                .ok_or(RootAdmissionError::EmptyRegion { region })?;
            if let Some(axis) = extrema.upper().iter().position(Option::is_none) {
                return Err(RootAdmissionError::UnboundedRegion { region, axis });
            }
            regions
                .try_reserve(1)
                .map_err(|_| RootAdmissionError::Allocation)?;
            regions.push(summary);
        }
        if regions.is_empty() {
            return Err(RootAdmissionError::EmptyPolicy);
        }
        // Multiple regions on one support remain a union, never their hull.
        regions.sort_unstable_by(|a, b| a.owner().cmp(b.owner()));
        Ok(Self { regions })
    }

    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    pub fn regions(&self) -> &[DomainPowerSummary<N>] {
        &self.regions
    }

    /// Validate only an initial root, never a routing image or IBP descendant.
    /// Existing source-condition checks are separate and still mandatory.
    pub fn validate_entry(&self, target: &IntegralKey) -> Result<(), RootAdmissionError> {
        if target.powers().len() != N {
            return Err(RootAdmissionError::WrongTargetArity {
                expected: N,
                actual: target.powers().len(),
            });
        }
        let support = std::array::from_fn(|axis| target.powers()[axis] > 0);
        let first = self
            .regions
            .partition_point(|region| region.owner() < &support);
        let matching = &self.regions[first..];
        let count = matching.partition_point(|region| region.owner() == &support);
        if count == 0 {
            return Err(RootAdmissionError::OutsideSelectedSupport {
                support: Mask::try_new(support).map_err(|_| RootAdmissionError::Allocation)?,
            });
        }
        // Chart conversion only; native summaries handle all A/R/D arithmetic.
        // unsigned_abs handles i64::MIN and n-1 is safe for positive n.
        let lower: [u64; N] = std::array::from_fn(|axis| {
            let n = target.powers()[axis];
            if n > 0 {
                (n as u64) - 1
            } else {
                n.unsigned_abs()
            }
        });
        let point = DomainPowerSummary::try_new(
            support,
            &lower,
            &lower.map(Some),
            None,
            DomainPowerBounds::default(),
        )
        .map_err(|error| RootAdmissionError::InvalidTarget { error })?;
        if matching[..count]
            .iter()
            .any(|region| region.contains(&point))
        {
            Ok(())
        } else {
            Err(RootAdmissionError::OutsideStartingDomain {
                target: target.clone(),
            })
        }
    }
}

/// Entry-only policy. An explicit finite union replaces (not intersects) the
/// saved generation rank gate; it never changes source validity or owner scope.
#[derive(Clone, Copy, Debug, Default)]
pub enum CandidateEntryAdmission<'a, const N: usize> {
    #[default]
    SavedGenerationScope,
    ExplicitFinite(&'a FiniteRootAdmission<N>),
}

impl<const N: usize> CandidateEntryAdmission<'_, N> {
    pub(super) fn validate_entry(
        self,
        target: &IntegralKey,
        saved_rank: Option<u32>,
    ) -> Result<(), CandidateRoutedError> {
        match self {
            Self::SavedGenerationScope => {
                validate_entry_rank(target, saved_rank).map_err(Into::into)
            }
            Self::ExplicitFinite(policy) => policy.validate_entry(target).map_err(Into::into),
        }
    }
}

#[cfg(test)]
mod tests;
