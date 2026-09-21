use super::super::{CandidateOwnerPrograms, CandidateReductionError};
use crate::family::IntegralKey;
use crate::sector::Mask;
use crate::sector::symmetry::integral_transport;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// A caller-selected, exactly verified route to one installed owner.
#[derive(Clone, Debug)]
pub struct CandidateOwnerRoute {
    pub owner_sector: Mask,
    pub transport: Arc<integral_transport::Prepared>,
}

/// Extra per-request trace and routing caps. The programs' ReductionLimits
/// remain authoritative for every formula across every owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutedCandidateLimits {
    pub max_input_targets: usize,
    /// Distinct operational states, including Route and Apply for one key.
    pub max_unique_nodes: usize,
    pub max_transport_calls: usize,
    /// Sum of the existing expansion service's conservative operation bounds.
    pub max_transport_operations: usize,
    /// Sum of projected, pre-coalescing endpoint bounds across all routes.
    pub max_transport_endpoints: usize,
    pub expansion: integral_transport::ExpansionLimits,
}
impl Default for RoutedCandidateLimits {
    fn default() -> Self {
        Self {
            max_input_targets: 100_000,
            max_unique_nodes: 1_000_000,
            max_transport_calls: 100_000,
            max_transport_operations: 64_000_000,
            max_transport_endpoints: 4_000_000,
            expansion: Default::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidateRoutedFrontierReason<const N: usize> {
    MissingOwner,
    MissingRule { owner_sector: [bool; N] },
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CandidateRoutedFrontier<const N: usize> {
    pub target: IntegralKey,
    pub reason: CandidateRoutedFrontierReason<N>,
}

/// Finite successor inspection only. Exact coefficients are coalesced locally
/// but not back-substituted or globally cancelled. A frontier is never a master.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedTraceReport<const N: usize> {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) input_targets: usize,
    pub(super) requested_targets: usize,
    pub(super) reachable_integrals: usize,
    pub(super) operational_nodes: usize,
    pub(super) rule_applications: usize,
    pub(super) transport_calls: usize,
    pub(super) transport_operations: usize,
    pub(super) transport_endpoints: usize,
    pub(super) max_numerator_rank: u128,
    pub(super) max_dot_excess: u128,
    pub(super) frontier: BTreeSet<CandidateRoutedFrontier<N>>,
    pub(super) declared_terminals: BTreeSet<IntegralKey>,
    pub(super) visited_zeros: BTreeSet<IntegralKey>,
}
impl<const N: usize> CandidateRoutedTraceReport<N> {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn input_targets(&self) -> usize {
        self.input_targets
    }
    pub fn requested_targets(&self) -> usize {
        self.requested_targets
    }
    pub fn reachable_integrals(&self) -> usize {
        self.reachable_integrals
    }
    pub fn operational_nodes(&self) -> usize {
        self.operational_nodes
    }
    pub fn rule_applications(&self) -> usize {
        self.rule_applications
    }
    pub fn transport_calls(&self) -> usize {
        self.transport_calls
    }
    pub fn transport_operation_bound(&self) -> usize {
        self.transport_operations
    }
    pub fn transport_endpoint_bound(&self) -> usize {
        self.transport_endpoints
    }
    pub fn max_negative_index_degree(&self) -> u128 {
        self.max_numerator_rank
    }
    pub fn max_dot_excess(&self) -> u128 {
        self.max_dot_excess
    }
    pub fn frontier(&self) -> &BTreeSet<CandidateRoutedFrontier<N>> {
        &self.frontier
    }
    pub fn declared_terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.declared_terminals
    }
    pub fn visited_zeros(&self) -> &BTreeSet<IntegralKey> {
        &self.visited_zeros
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateRoutedError {
    Candidate(CandidateReductionError),
    Transport(integral_transport::Error),
    InvalidInput(String),
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    UnsupportedSupportTransition {
        target: IntegralKey,
        child: IntegralKey,
    },
    Cycle {
        target: IntegralKey,
    },
}
impl From<CandidateReductionError> for CandidateRoutedError {
    fn from(error: CandidateReductionError) -> Self {
        Self::Candidate(error)
    }
}
impl From<integral_transport::Error> for CandidateRoutedError {
    fn from(error: integral_transport::Error) -> Self {
        Self::Transport(error)
    }
}
impl From<crate::reduction::ReductionError> for CandidateRoutedError {
    fn from(error: crate::reduction::ReductionError) -> Self {
        Self::Candidate(error.into())
    }
}
impl fmt::Display for CandidateRoutedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Candidate(e) => e.fmt(f),
            Self::Transport(e) => e.fmt(f),
            Self::InvalidInput(e) => write!(f, "invalid routed candidate programs: {e}"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                f,
                "routed {resource} requires {requested}; limit is {limit}"
            ),
            Self::UnsupportedSupportTransition { target, child } => write!(
                f,
                "unsupported routed support transition {:?} -> {:?}",
                target.powers(),
                child.powers()
            ),
            Self::Cycle { target } => {
                write!(f, "routed operational cycle at {:?}", target.powers())
            }
        }
    }
}
impl std::error::Error for CandidateRoutedError {}

/// Trace-only owner dispatcher. Programs and maps are immutable and shared;
/// per-request work is local. This is not a ClosedArtifact or a decomposition.
#[derive(Debug)]
pub struct RoutedCandidateReducer<const N: usize> {
    pub(super) programs: Arc<CandidateOwnerPrograms<N>>,
    pub(super) routes: BTreeMap<[bool; N], CandidateOwnerRoute>,
    pub(super) limits: RoutedCandidateLimits,
}
impl<const N: usize> RoutedCandidateReducer<N> {
    /// Reuse the already verified transport objects with an appended immutable
    /// program snapshot. A separately constructed library, even in the same
    /// family, is not an admitted replacement. No old traversal cache is reused.
    pub fn with_programs(
        &self,
        programs: Arc<CandidateOwnerPrograms<N>>,
    ) -> Result<Self, CandidateRoutedError> {
        if !programs.extends(&self.programs) {
            return Err(CandidateRoutedError::InvalidInput(
                "replacement programs are not an append-only extension of this owner snapshot"
                    .into(),
            ));
        }
        Ok(Self {
            programs,
            routes: self.routes.clone(),
            limits: self.limits,
        })
    }
    pub fn programs(&self) -> &Arc<CandidateOwnerPrograms<N>> {
        &self.programs
    }
    pub fn limits(&self) -> RoutedCandidateLimits {
        self.limits
    }
}
