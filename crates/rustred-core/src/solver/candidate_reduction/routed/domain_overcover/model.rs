use std::fmt;

/// Per-call logical work/storage envelope, not native expansion usage or RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateDomainRouteLimits {
    /// Candidate support masks examined, including literal/missing/zero entries.
    pub max_masks: usize,
    /// Cumulative logical lower/upper coordinate cells (2*N) for emitted domains.
    /// Domains are implicit full orthants, so this is not retained allocation.
    pub max_coordinate_cells: usize,
}
impl Default for CandidateDomainRouteLimits {
    fn default() -> Self {
        Self {
            max_masks: 65_536,
            max_coordinate_cells: 2_097_152,
        }
    }
}

/// A sufficient full-orthant cover in destination indexed coordinates.
/// Lower bounds are all zero, upper bounds all mathematical infinity; the
/// inactive-coordinate sum is bounded by actual_rank, or unbounded for None.
/// This does not establish source validity, a reached endpoint, nonzero
/// coefficients, target applicability, or family closure. Gaps in extra points
/// are NOT automatically reached missing-rule frontiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateDomainRouteCover<const N: usize> {
    pub source_sector: [bool; N],
    pub target_root: [bool; N],
    pub actual_rank: Option<u32>,
    pub conservative: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateDomainRouteEvent<const N: usize> {
    /// Literal owner or full mapped root: NEVER route this same root again.
    Apply {
        owner_sector: [bool; N],
        cover: CandidateDomainRouteCover<N>,
    },
    /// Strict subsupport of mapped root. Reentry strictly decreases support.
    Route {
        sector: [bool; N],
        cover: CandidateDomainRouteCover<N>,
    },
    /// Metadata lookup failure, not a declaration that the sector is a master.
    MissingRoute {
        source_sector: [bool; N],
        actual_rank: Option<u32>,
    },
    /// A known-zero SOURCE sector on visitor entry. Shared source conditions
    /// must still be checked before this can discharge work. When they exist,
    /// retain a validity obligation; this service does not evaluate them.
    ZeroSector {
        sector: [bool; N],
        actual_rank: Option<u32>,
        source_conditions_required: bool,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateDomainRouteStats {
    pub masks_examined: usize,
    pub events: usize,
    pub apply_domains: usize,
    pub route_domains: usize,
    pub zero_sectors: usize,
    pub missing_routes: usize,
    pub coordinate_cells: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateDomainRouteFailure {
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
    InvalidAdmittedRoute(&'static str),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateDomainRouteError {
    pub failure: CandidateDomainRouteFailure,
    pub stats: CandidateDomainRouteStats,
}
impl fmt::Display for CandidateDomainRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incomplete symbolic route cover after {} masks: {:?}",
            self.stats.masks_examined, self.failure
        )
    }
}
impl std::error::Error for CandidateDomainRouteError {}
