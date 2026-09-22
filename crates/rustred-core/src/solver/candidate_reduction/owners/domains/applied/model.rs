use std::fmt;

use super::super::{OwnerDomainMatchLimits, OwnerDomainMatchPiece, OwnerDomainMatchStats};
use crate::algebra::{IndexedAlgebraError, IndexedCoefficient};
use crate::solver::candidate_reduction::power_domain::{DomainPowerBounds, DomainPowerError};

/// Aggregate local-query work. Existing indexed/native operation limits remain
/// those of the admitted programs; these counters are not hard RSS guarantees.
/// Native expressions have bounded simultaneous multiplicity (running sum,
/// restricted term, original denominator and condition scratch), with each
/// operation admitted independently, not an aggregate native-byte allowance.
#[derive(Clone, Copy, Debug)]
pub struct OwnerAppliedLimits {
    pub matching: OwnerDomainMatchLimits,
    pub max_term_visits: usize,
    pub max_shift_groups: usize,
    pub max_boundary_cells: usize,
    pub max_sign_splits: usize,
    pub max_native_operations: usize,
    pub max_events: usize,
    pub max_scratch_terms: usize,
    pub max_scratch_boxes: usize,
    pub max_scratch_coordinate_cells: usize,
}
impl Default for OwnerAppliedLimits {
    fn default() -> Self {
        Self {
            matching: Default::default(),
            max_term_visits: 1_000_000,
            max_shift_groups: 1_000_000,
            max_boundary_cells: 100_000,
            max_sign_splits: 1_000_000,
            max_native_operations: 4_000_000,
            max_events: 1_000_000,
            max_scratch_terms: 1_000_000,
            max_scratch_boxes: 65_536,
            max_scratch_coordinate_cells: 2_097_152,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnerAppliedStats {
    pub matching: OwnerDomainMatchStats,
    pub selected_pieces: usize,
    pub term_visits: usize,
    pub shift_groups: usize,
    pub boundary_cells: usize,
    pub sign_splits: usize,
    pub native_operations: usize,
    /// Recognized optional numerator preflight refusals, including attempts
    /// followed by cancellation, an event limit, or consumer stop. Equal to
    /// optional_original_refusals + optional_coalesced_refusals. No work refund.
    pub optional_coefficient_refusals: usize,
    pub optional_original_refusals: usize,
    pub optional_coalesced_refusals: usize,
    pub coalescing_additions: usize,
    pub events: usize,
    pub successors: usize,
    pub conditional_successors: usize,
    pub problems: usize,
    pub zero_terms: usize,
    pub cancelled_groups: usize,
    pub zero_sector_groups: usize,
    pub correlation_empty_cells: usize,
}

/// Nonzero as an indexed rational function need not mean nonzero at every
/// integer point. Conditional retains the exact predicate `coefficient != 0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerAppliedNonzero {
    Uniform,
    Conditional,
}

/// All fields are borrowed for this callback only. The target box intersected
/// with target_rank_limit AND target_power_bounds is the exact image of the
/// refined source domain. The target rectangle alone may contain extra points.
/// A Conditional coefficient makes it an over-cover of nonzero dependencies;
/// an uncovered target box is then NOT automatically a reached missing rule.
/// Events are streamed by shift group. Even Uniform successors are provisional
/// until the selected piece finishes without problems and the inspection is
/// untruncated: a later original term can still fail child validity. Uniform
/// describes this coefficient only, not successful application of the rule.
#[derive(Debug)]
pub struct OwnerAppliedSuccessor<'a, const N: usize> {
    pub source: &'a OwnerDomainMatchPiece<N>,
    pub source_lower: &'a [u64],
    pub source_upper: &'a [Option<u64>],
    pub target_sector: &'a [bool; N],
    pub target_lower: &'a [u64],
    pub target_upper: &'a [Option<u64>],
    pub target_rank_limit: Option<u32>,
    pub target_power_bounds: DomainPowerBounds,
    pub shift: &'a [i64; N],
    /// Coefficient in ORIGINAL SOURCE indexed variables, after fixed restriction.
    pub coefficient: &'a IndexedCoefficient,
    pub coefficient_nonzero: OwnerAppliedNonzero,
    /// Membership only, never target-domain applicability or a gap decision.
    pub has_installed_target_owner: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerAppliedProblemKind {
    InvalidChildRoot,
    InvalidChildSourceCondition { ordinal: usize },
    UnresolvedChildSourceCondition { ordinal: usize },
    DescentNotEstablished { detail: String },
    UnresolvedFixedCoordinate { axis: usize },
    UnresolvedImage { detail: &'static str },
}

/// Original-term validity precedes coalescing, exactly as in the evaluator.
/// With Conditional nonzero this is a conditional obligation, not a claim that
/// an invalid point is inhabited. No issue is promoted to a missing owner.
#[derive(Debug)]
pub struct OwnerAppliedProblem<'a, const N: usize> {
    pub source: &'a OwnerDomainMatchPiece<N>,
    pub source_lower: &'a [u64],
    pub source_upper: &'a [Option<u64>],
    pub shift: &'a [i64; N],
    pub original_term_ordinal: Option<usize>,
    pub coefficient: Option<&'a IndexedCoefficient>,
    pub coefficient_nonzero: OwnerAppliedNonzero,
    pub kind: OwnerAppliedProblemKind,
}

#[derive(Debug)]
pub enum OwnerAppliedEvent<'a, const N: usize> {
    Classified(&'a OwnerDomainMatchPiece<N>),
    /// Only the first original and first coalesced refusal in this query are
    /// emitted, with normal event admission. All refusals remain counted in
    /// stats. More refusals than retained records means provenance is partial;
    /// a stop can also prevent delivery of a counted first record. No exact
    /// coefficient is retained here, and this is not a missing-rule obligation.
    OptionalCoefficientRefusal {
        source: &'a OwnerDomainMatchPiece<N>,
        source_lower: &'a [u64],
        source_upper: &'a [Option<u64>],
        shift: &'a [i64; N],
        /// Some for an original RHS term, None for the final coalesced sum.
        original_term_ordinal: Option<usize>,
        failure: &'a IndexedAlgebraError,
    },
    Successor(OwnerAppliedSuccessor<'a, N>),
    Problem(OwnerAppliedProblem<'a, N>),
    /// Local RHS inspection ended; problems/conditional edges remain explicit.
    RuleFinished {
        source: &'a OwnerDomainMatchPiece<N>,
        successors: usize,
        problems: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerAppliedFailure {
    Matching(super::super::OwnerDomainMatchFailure),
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
    AffineRestriction(crate::solver::AffineGeometryError),
    Geometry(String),
    PowerDomain(DomainPowerError),
    InternalInvariant(&'static str),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerAppliedError {
    pub failure: OwnerAppliedFailure,
    pub stats: OwnerAppliedStats,
    pub max_numerator_rank: Option<u32>,
    pub power_bounds: DomainPowerBounds,
}
impl fmt::Display for OwnerAppliedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incomplete symbolic RHS inspection after {} successors: {:?}",
            self.stats.successors, self.failure
        )
    }
}
impl std::error::Error for OwnerAppliedError {}
