use std::fmt;

use crate::algebra::{IndexedAlgebraError, IndexedGuardLimits};
use crate::foundry::completion::LatticeBox;

/// Aggregate dispatch/geometry counters; native guard limits are per predicate.
/// Geometry cells and coordinate entries are conservative cumulative charges,
/// including temporary BoxCover storage, not a measured byte count.
#[derive(Clone, Copy, Debug)]
pub struct OwnerDomainMatchLimits {
    pub max_rules: usize,
    pub max_terminal_checks: usize,
    pub max_predicates: usize,
    pub max_pieces: usize,
    pub max_cells: usize,
    pub max_split_operations: usize,
    pub max_coordinate_cells: usize,
    /// Optional cumulative allowance for exact singleton faces of bounded
    /// inactive coordinates when a native predicate remains unresolved or its
    /// next GCD/factor operation refuses prospective admission. Zero preserves
    /// the conservative diagnostic/native refusal without refinement. A split is
    /// admitted only in full, including its geometry allowance; otherwise the
    /// original unresolved piece or typed native refusal is retained. Positive
    /// axes are never sampled; prior native work/attempt charges are not undone.
    pub max_bounded_refinement_cells: usize,
    pub guard_algebra: IndexedGuardLimits,
}
impl Default for OwnerDomainMatchLimits {
    fn default() -> Self {
        Self {
            max_rules: 100_000,
            max_terminal_checks: 1_000_000,
            max_predicates: 100_000,
            max_pieces: 65_536,
            max_cells: 1_000_000,
            max_split_operations: 1_000_000,
            max_coordinate_cells: 32_000_000,
            max_bounded_refinement_cells: 0,
            guard_algebra: Default::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnerDomainMatchStats {
    pub rules: usize,
    pub terminal_checks: usize,
    pub predicates: usize,
    /// Charged before callback, including a consumer-rejected last piece.
    pub pieces: usize,
    pub cells: usize,
    pub split_operations: usize,
    pub coordinate_cells: usize,
    pub rank_empty_cells: usize,
    /// Singleton faces admitted up front. Cancellation may leave some unvisited.
    pub refinement_cells: usize,
    /// Entire bounded-coordinate refinements admitted (not predicate retries).
    pub refinement_steps: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerDomainPredicate {
    SourceCondition {
        ordinal: usize,
    },
    Equality {
        batch: usize,
        rule: usize,
        ordinal: usize,
    },
    ExcludedConjunction {
        batch: usize,
        rule: usize,
        branch: usize,
        ordinal: usize,
    },
    OriginalDenominator {
        batch: usize,
        rule: usize,
        term: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerDomainMatchDisposition {
    /// Exact ordered guard applicability only. RHS specialization, cancellation,
    /// descent, successors and recursive coverage have NOT been established.
    SelectedRule {
        batch: usize,
        rule: usize,
    },
    Terminal {
        batch: usize,
    },
    ExactGap,
    /// Native coupled/conservative geometry or an unrepresentable exact cut.
    /// This piece does not fall through to later rules or become a gap.
    Unresolved {
        predicate: OwnerDomainPredicate,
    },
    InvalidSourceCondition {
        ordinal: usize,
    },
    ExactZeroSector,
}

/// Exact box intersected with `sum(inactive local coordinates) <= rank`.
/// Active physical powers are 1+x, inactive powers are -x. None is mathematical
/// infinity, not a machine index bound. Callers bound their own retained output.
#[derive(Debug)]
pub struct OwnerDomainMatchPiece<const N: usize> {
    pub(super) owner: [bool; N],
    pub(super) cell: LatticeBox,
    pub(super) rank: Option<u32>,
    pub(super) disposition: OwnerDomainMatchDisposition,
}
impl<const N: usize> OwnerDomainMatchPiece<N> {
    pub fn owner(&self) -> &[bool; N] {
        &self.owner
    }
    pub fn lower(&self) -> &[u64] {
        self.cell.lower()
    }
    pub fn upper(&self) -> &[Option<u64>] {
        self.cell.upper()
    }
    pub fn max_numerator_rank(&self) -> Option<u32> {
        self.rank
    }
    pub fn disposition(&self) -> OwnerDomainMatchDisposition {
        self.disposition
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerDomainMatchFailure {
    UnknownOwner,
    InvalidInput(String),
    Cancelled,
    StoppedByConsumer,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    CountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
    },
    Algebra(IndexedAlgebraError),
    Geometry(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerDomainMatchError {
    pub failure: OwnerDomainMatchFailure,
    pub stats: OwnerDomainMatchStats,
    /// Present for a failed native predicate resolution/refinement admission.
    /// Identifies the actual failed cursor, not a reached missing-rule claim.
    pub predicate: Option<OwnerDomainPredicate>,
    /// Actual query simplex; None is unbounded, not the saved entry rank.
    pub max_numerator_rank: Option<u32>,
    pub(super) predicate_bounds: Option<(Box<[u64]>, Box<[Option<u64>]>)>,
}
impl OwnerDomainMatchError {
    pub fn predicate_lower(&self) -> Option<&[u64]> {
        self.predicate_bounds
            .as_ref()
            .map(|(lower, _)| lower.as_ref())
    }
    pub fn predicate_upper(&self) -> Option<&[Option<u64>]> {
        self.predicate_bounds
            .as_ref()
            .map(|(_, upper)| upper.as_ref())
    }
}
impl fmt::Display for OwnerDomainMatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incomplete owner domain match after {} pieces: {:?}",
            self.stats.pieces, self.failure
        )?;
        if let Some(predicate) = self.predicate {
            write!(
                f,
                " at {predicate:?}, lower={:?}, upper={:?}, rank={:?}",
                self.predicate_lower(),
                self.predicate_upper(),
                self.max_numerator_rank
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for OwnerDomainMatchError {}
