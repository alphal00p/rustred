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
}
impl fmt::Display for OwnerDomainMatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incomplete owner domain match after {} pieces: {:?}",
            self.stats.pieces, self.failure
        )
    }
}
impl std::error::Error for OwnerDomainMatchError {}
