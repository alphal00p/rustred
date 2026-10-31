use super::super::{CandidateOwnerPrograms, CandidateReductionError};
use crate::algebra::Coefficient;
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

/// An exact homogeneous row for terminal refinement of candidate programs.
///
/// Rows combine verified weighted transport with saved candidate formulas.
/// Their shared authority is still candidate-only: this interface performs
/// no replay of original-source derivations or recursive closure proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateRoutedIdentity {
    pub terms: BTreeMap<IntegralKey, Coefficient>,
    /// Sufficient generic-parameter nonzero conditions; saved-rule exception
    /// witnesses may restrict the domain (see `CandidateIntegralIdentity`).
    pub nonzero_conditions: Vec<Coefficient>,
}

/// Extra per-request trace and routing caps. The programs' ReductionLimits
/// remain authoritative for every formula across every owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutedCandidateLimits {
    /// Opt in to count-based diagnostic stops; disabled for ordinary work.
    pub diagnostics_enabled: bool,
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
            diagnostics_enabled: false,
            max_input_targets: 100_000,
            max_unique_nodes: 1_000_000,
            max_transport_calls: 100_000,
            max_transport_operations: 64_000_000,
            max_transport_endpoints: 4_000_000,
            expansion: Default::default(),
        }
    }
}

impl RoutedCandidateLimits {
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self.expansion = self.expansion.with_diagnostics();
        self
    }

    pub(super) const fn diagnostic_limit(self, limit: usize) -> Option<usize> {
        if self.diagnostics_enabled {
            Some(limit)
        } else {
            None
        }
    }
}

/// How Ordered requests of a Route/Apply application expand the transport of
/// a Route key (ignored by the depth-first strategies).
///
/// A Route key `R` of source sector `S` is the integral of
/// `prod_{i in S} D_i^-p_i * prod_a (c_a + sum_l L_al D_l)^n_a` in the owner's
/// denominators (the numerators `a` of `R` mapped through the route's
/// verified affine map). Without layering one step expands the whole product.
/// With layering a step expands one factor and emits *partially transported*
/// keys (owner powers so far, the source sector and the numerator powers not
/// yet expanded); keys of the same sector that reach the same partial key
/// merge before the next factor is expanded, so the remaining factors are
/// expanded once for their merged weight. The last factor emits the same
/// Route and Apply children as the whole transport, with the same exact
/// total weights (the product is only re-associated): every result, every
/// zero decision on a Route or Apply key and every guard is unchanged.
///
/// Partial keys live inside the Route phase of their source (below all Route
/// keys of the phase) and are never recorded in a step cache; a step cache
/// keeps only steps that do not depend on this setting, so caches written
/// with and without layering are interchangeable. Layering applies to keys
/// with at least two numerator factors of an application without source
/// conditions (whose transport validation retains no guards); other keys take
/// the whole transport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OrderedRouteLayering {
    /// The whole transport in one step (historical).
    Off,
    /// One factor per step, lowest source axis first.
    Index,
    /// One factor per step, smallest remaining numerator power first (ties:
    /// lowest source axis).
    #[default]
    SmallestPower,
    /// One factor per step, the factor with the fewest expansion terms first
    /// (`binomial(power + width - 1, width - 1)` for an affine form with
    /// `width` nonzero entries; ties: smaller power, then lower axis).
    FewestTerms,
    /// One factor per step, largest remaining numerator power first (ties:
    /// lowest source axis). For comparison.
    LargestPower,
}

impl OrderedRouteLayering {
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Index,
        Self::SmallestPower,
        Self::FewestTerms,
        Self::LargestPower,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Index => "index",
            Self::SmallestPower => "smallest-power",
            Self::FewestTerms => "fewest-terms",
            Self::LargestPower => "largest-power",
        }
    }
}

impl fmt::Display for OrderedRouteLayering {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for OrderedRouteLayering {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|layering| layering.as_str() == text)
            .ok_or_else(|| {
                format!(
                    "unknown Route layering {text:?} (expected one of {})",
                    Self::ALL.map(Self::as_str).join(", ")
                )
            })
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
    /// A caller requested cancellation at a safe application boundary.
    Cancelled,
    Candidate(CandidateReductionError),
    EntryAdmission(super::RootAdmissionError),
    Transport(integral_transport::Error),
    InvalidInput(String),
    MissingOwner {
        target: IntegralKey,
    },
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
impl From<super::RootAdmissionError> for CandidateRoutedError {
    fn from(error: super::RootAdmissionError) -> Self {
        Self::EntryAdmission(error)
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
            Self::Cancelled => write!(f, "routed candidate application cancelled"),
            Self::Candidate(e) => e.fmt(f),
            Self::EntryAdmission(e) => e.fmt(f),
            Self::Transport(e) => e.fmt(f),
            Self::InvalidInput(e) => write!(f, "invalid routed candidate programs: {e}"),
            Self::MissingOwner { target } => {
                write!(f, "no routed candidate owner for {:?}", target.powers())
            }
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
impl std::error::Error for CandidateRoutedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Candidate(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::EntryAdmission(error) => Some(error),
            _ => None,
        }
    }
}

/// Immutable owner dispatcher. Traces keep request-local work; `application`
/// creates a separate exact coefficient cache over this same snapshot.
/// This is not a ClosedArtifact or a family-closure certificate.
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
