use std::num::NonZeroUsize;

/// Inspect-all stays the default; transfer must be explicitly requested.
/// The lookahead is a logical-ID fence, never derived from worker count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SchedulingPolicy {
    #[default]
    InspectAll,
    TransferUnreserved {
        lookahead: NonZeroUsize,
    },
}

impl SchedulingPolicy {
    pub fn validate(self, max_containment_checks: Option<usize>) -> Result<(), Error> {
        if matches!(self, Self::TransferUnreserved { .. }) && max_containment_checks.is_some() {
            return Err(Error::FiniteContainmentCap);
        }
        Ok(())
    }
}

/// Produced only at canonical publication of an actual native inspection.
/// The caller retains the detailed native error/statistics/source diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeOutcome {
    /// All native callbacks have been admitted. A Finished without an error
    /// may still retain unresolved guard/source/routing frontiers.
    Completed {
        unresolved_frontiers: usize,
    },
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publication {
    DelegatedNotInspected { id: usize, representative: usize },
    Native { id: usize, outcome: NativeOutcome },
}

impl Publication {
    /// Pool cleanup must subtract genuine native publications, not cursor delta.
    pub fn native_publications(self) -> usize {
        usize::from(matches!(self, Self::Native { .. }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    Installed,
    AlreadyDelegated,
    ReservedOrStarted,
    ProtectedInitial,
    IdentityMismatch,
    InvalidForwardEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    ZeroCapacity,
    Capacity,
    Allocation,
    AdmissionIdMismatch,
    AdmissionNotReserved,
    InvalidId,
    OutsideFence,
    InvalidNativeState,
    NotCurrentPublisher,
    NotDelegated,
    InvalidForwardEdge,
    IdentityMismatch,
    Halted,
    FiniteContainmentCap,
    InvalidInitialPhase,
    InvalidInitialAnchor,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ZeroCapacity => "delegation ledger requires a positive domain allowance",
            Self::Capacity => "delegation ledger domain allowance exceeded",
            Self::Allocation => "delegation ledger allocation failed",
            Self::AdmissionIdMismatch => "delegation ledger admission ID mismatch",
            Self::AdmissionNotReserved => "delegation ledger admission storage was not reserved",
            Self::InvalidId => "invalid delegation ledger domain ID",
            Self::OutsideFence => "native dispatch exceeds the canonical delegation fence",
            Self::InvalidNativeState => "invalid native inspection state for delegation ledger",
            Self::NotCurrentPublisher => "delegation publication is not the canonical cursor",
            Self::NotDelegated => "requested delegated publication has no responsibility edge",
            Self::InvalidForwardEdge => "invalid forward delegation edge",
            Self::IdentityMismatch => "delegation phase/owner identity mismatch",
            Self::Halted => "delegation ledger halted after a failed or cancelled native publisher",
            Self::FiniteContainmentCap => {
                "responsibility transfer requires unlimited semantic containment"
            }
            Self::InvalidInitialPhase => "invalid protected initial-admission phase",
            Self::InvalidInitialAnchor => "invalid initial-overlap responsibility anchor",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionStatus {
    Pending,
    Discharged,
    UnresolvedFrontiers { count: usize },
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub representative: usize,
    pub status: ResolutionStatus,
    pub delegated: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub admitted: usize,
    pub logical_publications: usize,
    pub native_publications: usize,
    /// Effective local-obligation statuses, including a partial endpoint's
    /// protected initial anchor. Not unique native failure/frontier sources.
    pub native_discharged: usize,
    pub native_frontier_blocked: usize,
    pub native_failed: usize,
    pub native_cancelled: usize,
    pub native_pending: usize,
    pub delegated: usize,
    pub delegated_publications: usize,
    pub delegated_resolved: usize,
    pub delegated_pending: usize,
    pub delegated_frontier_blocked: usize,
    pub delegated_failure_blocked: usize,
    pub delegated_cancelled: usize,
    pub maximum_alias_depth: usize,
    pub partial_initial_inspections: usize,
    pub partial_initial_blocked: usize,
}

impl Summary {
    /// Ledger-local only: the caller must ALSO require zero global/initial
    /// frontiers, no outer error/cancellation and every native event admitted.
    /// This is not a mathematical family-closure or descent certificate.
    pub fn all_ledger_obligations_discharged(&self) -> bool {
        self.logical_publications == self.admitted
            && self.native_pending == 0
            && self.native_frontier_blocked == 0
            && self.native_failed == 0
            && self.native_cancelled == 0
            && self.delegated_pending == 0
            && self.delegated_frontier_blocked == 0
            && self.delegated_failure_blocked == 0
            && self.delegated_cancelled == 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolutionReport {
    pub by_id: Vec<Resolution>,
    pub summary: Summary,
}
