use crate::solver::candidate_reduction::power_domain::{DomainPowerBounds, DomainPowerError};
use std::fmt;

/// Per-call logical work/storage envelope, not native expansion usage or RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateDomainRouteLimits {
    /// Candidate support masks examined, including literal/missing/zero entries.
    pub max_masks: usize,
    /// Cumulative logical lower/upper coordinate cells (2*N) for emitted domains.
    /// This accounts for emitted bounds, not native expansion usage or RSS.
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

/// A sufficient box cover in destination indexed coordinates. Positive axes
/// use n=x+1, inactive axes n=-x. The inactive-coordinate sum is additionally
/// bounded by actual_rank, or unbounded for None. Aggregate power predicates
/// remain part of the cover; its coordinate rectangle is not a substitute.
/// This does not establish source validity, a reached endpoint, nonzero
/// coefficients, target applicability, or family closure. Gaps in extra points
/// are NOT automatically reached missing-rule frontiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateDomainRouteCover<const N: usize> {
    pub source_sector: [bool; N],
    pub target_root: [bool; N],
    pub lower: [u64; N],
    pub upper: [Option<u64>; N],
    /// Rank bound of this emitted cover, not necessarily the incoming bound.
    /// A strict Route loses at least sum(source_lower[j]+1 for j in P) from
    /// the current R bound. Correlated projection can tighten R even for a
    /// literal/full-root Apply, or infer finite R from an incoming None.
    /// None means no separately representable rank cap; retained coordinate
    /// and A/D bounds still apply. Never clipped to the saved entry rank.
    pub actual_rank: Option<u32>,
    pub power_bounds: DomainPowerBounds,
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
        power_bounds: DomainPowerBounds,
    },
    /// A known-zero SOURCE sector on visitor entry. Shared source conditions
    /// must still be checked before this can discharge work. When they exist,
    /// retain a validity obligation; this service does not evaluate them.
    ZeroSector {
        sector: [bool; N],
        actual_rank: Option<u32>,
        power_bounds: DomainPowerBounds,
        source_conditions_required: bool,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateDomainRouteStats {
    pub masks_examined: usize,
    /// Examined masks excluded by weighted pinch cost or empty power geometry.
    /// These masks still consume the mask allowance.
    pub masks_pruned: usize,
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
    InvalidDomain(&'static str),
    PowerDomain(DomainPowerError),
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
