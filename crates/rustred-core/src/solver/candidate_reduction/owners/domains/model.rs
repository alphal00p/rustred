use std::fmt;

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::foundry::completion::LatticeBox;
use crate::solver::candidate_reduction::model::{PreparedRule, PreparedTerm};

/// Work and scratch limits; callers separately bound retained callback output.
#[derive(Clone, Copy, Debug)]
pub struct OwnerSuccessorLimits {
    pub max_rules: usize,
    pub max_terms: usize,
    pub max_regions: usize,
    pub max_split_operations: usize,
    /// Conservative peak box-equivalent scratch, including the input and
    /// splitter construction temporaries (partition size plus four).
    pub max_scratch_boxes: usize,
    /// Logical lower/upper coordinate entries in the same scratch envelope;
    /// not allocator byte capacity. Callback-retained regions are caller-owned.
    pub max_scratch_coordinate_cells: usize,
}
impl Default for OwnerSuccessorLimits {
    fn default() -> Self {
        Self {
            max_rules: 100_000,
            max_terms: 1_000_000,
            max_regions: 4_000_000,
            max_split_operations: 4_000_000,
            max_scratch_boxes: 65_536,
            max_scratch_coordinate_cells: 2_097_152,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnerSuccessorStats {
    pub rules: usize,
    pub terms: usize,
    pub regions: usize,
    pub split_operations: usize,
    pub exact_zero_terms: usize,
    pub rank_empty_prefilters: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerSuccessorTransition {
    SameSupport,
    StrictPinch,
    /// Not discarded: coefficient/domain restrictions may still remove it.
    UnsupportedSupportChange,
}

/// One potential successor, expressed in ORIGINAL source index coordinates.
///
/// Intersect the coordinate box with ALL equalities, excluded zero-conjunctions,
/// original denominators, source conditions, nonvanishing term coefficient and
/// `sum(inactive local coordinates) <= source_rank_limit`. Each active source
/// index is `1+x`, each inactive index is `-x`; target indices are source+shift.
/// Affine guards may further bound a nominally infinite positive box direction.
/// None of these descriptors proves that the domain is nonempty or missing a
/// rule. Every installed rule is scanned, regardless of earlier rule priority.
#[derive(Debug)]
pub struct OwnerSuccessorRegion<'a, const N: usize> {
    pub(super) source_sector: [bool; N],
    pub(super) target_sector: [bool; N],
    pub(super) source_rank_limit: Option<u32>,
    pub(super) target_rank_upper: Option<u128>,
    pub(super) same_support_rank_delta: Option<i128>,
    pub(super) source_box: LatticeBox,
    pub(super) batch: usize,
    pub(super) term_ordinal: usize,
    pub(super) rule: &'a PreparedRule<N>,
    pub(super) term: &'a PreparedTerm<N>,
    pub(super) source_conditions: &'a [IndexedPolynomial],
    pub(super) transition: OwnerSuccessorTransition,
    pub(super) installed_target_owner: bool,
    pub(super) exact_zero_sector: bool,
}
impl<'a, const N: usize> OwnerSuccessorRegion<'a, N> {
    pub fn source_sector(&self) -> &[bool; N] {
        &self.source_sector
    }
    pub fn target_sector(&self) -> &[bool; N] {
        &self.target_sector
    }
    pub fn source_rank_limit(&self) -> Option<u32> {
        self.source_rank_limit
    }
    /// A conservative ONE-STEP bound, never a recursive envelope or rank clip.
    pub fn target_rank_upper_bound(&self) -> Option<u128> {
        self.target_rank_upper
    }
    /// Exact constant change on same-support sign cells; None on pinches.
    pub fn same_support_rank_delta(&self) -> Option<i128> {
        self.same_support_rank_delta
    }
    pub fn source_local_lower(&self) -> &[u64] {
        self.source_box.lower()
    }
    pub fn source_local_upper(&self) -> &[Option<u64>] {
        self.source_box.upper()
    }
    pub fn batch_ordinal(&self) -> usize {
        self.batch
    }
    pub fn rule_ordinal(&self) -> usize {
        self.rule.ordinal
    }
    pub fn term_ordinal(&self) -> usize {
        self.term_ordinal
    }
    pub fn fixed(&self) -> &[Option<i16>; N] {
        &self.rule.fixed
    }
    pub fn equalities(&self) -> &'a [IndexedPolynomial] {
        &self.rule.equalities
    }
    /// Excluded if ANY entire inner AND-conjunction vanishes. Do not flatten.
    pub fn excluded_zero_conjunctions(&self) -> &'a [Vec<IndexedPolynomial>] {
        &self.rule.exceptions
    }
    pub fn original_denominators(&self) -> impl Iterator<Item = &'a IndexedPolynomial> {
        self.rule.rhs.iter().map(|term| &term.denominator)
    }
    pub fn source_conditions(&self) -> &'a [IndexedPolynomial] {
        self.source_conditions
    }
    pub fn coefficient(&self) -> &'a IndexedCoefficient {
        &self.term.coefficient
    }
    pub fn shift(&self) -> &'a [i64; N] {
        &self.term.shift
    }
    pub fn transition(&self) -> OwnerSuccessorTransition {
        self.transition
    }
    /// Membership only; an installed program is not a claim of domain coverage.
    pub fn has_installed_target_owner(&self) -> bool {
        self.installed_target_owner
    }
    pub fn is_exact_zero_sector(&self) -> bool {
        self.exact_zero_sector
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerSuccessorFailure {
    UnknownOwner,
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
    Geometry(String),
}

/// Prefix counts only. Previously delivered descriptors do not mean scan success.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerSuccessorError {
    pub failure: OwnerSuccessorFailure,
    pub stats: OwnerSuccessorStats,
}
impl fmt::Display for OwnerSuccessorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incomplete owner successor scan after {} regions: {:?}",
            self.stats.regions, self.failure
        )
    }
}
impl std::error::Error for OwnerSuccessorError {}
