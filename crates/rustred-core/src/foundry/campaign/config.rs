use crate::foundry::completion::source_discovery::{CampaignLimits, TaskRelativeModularProbe};
use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, OrderingPolicy};

use super::{FoundryCampaignConfigError, FoundrySearchProvenance};

/// Stable schema identifier for the deterministic report payload.
pub const FOUNDRY_CAMPAIGN_CONFIG_SCHEMA: &str = "rustred.foundry-campaign-config.toml.v2";
pub const FOUNDRY_CAMPAIGN_REPORT_SCHEMA: &str = "rustred.foundry-campaign-report.toml.v1";

/// Built-in family/sector pressure target.
///
/// This is intentionally not a closing-family selector. Running this preset
/// neither claims K=6 closure nor publishes a closing artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FoundryCampaignPreset {
    ThreeLoopUnitMassVacuumK6Orbit0,
}

/// Deterministic frontier itinerary selected for one campaign request.
///
/// The itinerary is part of the semantic request: a single-sector fixed point
/// and the transactional full-rank wave driver retain different proof state
/// and therefore cannot be silently interchanged by an adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FoundryCampaignItinerary {
    #[default]
    SingleSectorFixedPoint,
    FullRankAtomicWaves,
}

impl FoundryCampaignItinerary {
    pub const SINGLE_SECTOR_FIXED_POINT_ID: &'static str = "single-sector-fixed-point";
    pub const FULL_RANK_ATOMIC_WAVES_ID: &'static str = "full-rank-atomic-waves";

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::SingleSectorFixedPoint => Self::SINGLE_SECTOR_FIXED_POINT_ID,
            Self::FullRankAtomicWaves => Self::FULL_RANK_ATOMIC_WAVES_ID,
        }
    }

    pub fn from_stable_id(value: &str) -> Option<Self> {
        match value {
            Self::SINGLE_SECTOR_FIXED_POINT_ID => Some(Self::SingleSectorFixedPoint),
            Self::FULL_RANK_ATOMIC_WAVES_ID => Some(Self::FullRankAtomicWaves),
            _ => None,
        }
    }
}

impl FoundryCampaignPreset {
    pub const THREE_LOOP_UNIT_MASS_VACUUM_K6_ORBIT_0_ID: &'static str =
        "three-loop-unit-mass-vacuum-k6-orbit-0";

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::ThreeLoopUnitMassVacuumK6Orbit0 => {
                Self::THREE_LOOP_UNIT_MASS_VACUUM_K6_ORBIT_0_ID
            }
        }
    }

    pub fn from_stable_id(value: &str) -> Option<Self> {
        match value {
            Self::THREE_LOOP_UNIT_MASS_VACUUM_K6_ORBIT_0_ID => {
                Some(Self::ThreeLoopUnitMassVacuumK6Orbit0)
            }
            _ => None,
        }
    }
}

/// One exact modular probe template relative to each canonical boundary task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryCampaignProbe {
    modulus: u64,
    base_parameters: Box<[i64]>,
    chart_offsets: Box<[u64]>,
}

impl FoundryCampaignProbe {
    pub fn new(
        modulus: u64,
        base_parameters: impl IntoIterator<Item = i64>,
        chart_offsets: impl IntoIterator<Item = u64>,
    ) -> Self {
        Self {
            modulus,
            base_parameters: base_parameters.into_iter().collect(),
            chart_offsets: chart_offsets.into_iter().collect(),
        }
    }

    pub const fn modulus(&self) -> u64 {
        self.modulus
    }

    pub fn base_parameters(&self) -> &[i64] {
        &self.base_parameters
    }

    pub fn chart_offsets(&self) -> &[u64] {
        &self.chart_offsets
    }
}

/// Reviewed, non-authoritative search metadata for the external-hints lane.
///
/// The type can express only existing RustRed search choices. It has no rule,
/// source-row, recurrence RHS, coefficient, or support payload. Constructing
/// this object does not grant proof or artifact publication authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryCampaignExternalHints {
    itinerary: FoundryCampaignItinerary,
    probes: Box<[FoundryCampaignProbe]>,
    interior_margin: u64,
    polynomial_degree_ceiling: usize,
    ordering: OrderingPolicy,
    discovery_coordinate_priority: Option<CoordinatePriority>,
}

impl FoundryCampaignExternalHints {
    pub fn try_new(
        itinerary: FoundryCampaignItinerary,
        probes: impl IntoIterator<Item = FoundryCampaignProbe>,
        interior_margin: u64,
        polynomial_degree_ceiling: usize,
        ordering: OrderingPolicy,
        discovery_coordinate_priority: Option<CoordinatePriority>,
    ) -> Result<Self, FoundryCampaignConfigError> {
        let probes = probes.into_iter().collect::<Box<[_]>>();
        if probes.is_empty() {
            return Err(FoundryCampaignConfigError::EmptyProbeProgram);
        }
        if interior_margin == 0 {
            return Err(FoundryCampaignConfigError::ZeroInteriorMargin);
        }
        Ok(Self {
            itinerary,
            probes,
            interior_margin,
            polynomial_degree_ceiling,
            ordering,
            discovery_coordinate_priority,
        })
    }

    pub const fn itinerary(&self) -> FoundryCampaignItinerary {
        self.itinerary
    }
}

/// Deterministic semantic inputs for one bounded diagnostic campaign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryCampaignConfig {
    preset: FoundryCampaignPreset,
    itinerary: FoundryCampaignItinerary,
    search_provenance: FoundrySearchProvenance,
    probes: Box<[FoundryCampaignProbe]>,
    interior_margin: u64,
    polynomial_degree_ceiling: usize,
    ordering: OrderingPolicy,
    discovery_coordinate_priority: CoordinatePriority,
    max_task_reports: usize,
    max_reported_uncovered_boxes: usize,
}

impl FoundryCampaignConfig {
    pub const fn schema(&self) -> &'static str {
        FOUNDRY_CAMPAIGN_CONFIG_SCHEMA
    }

    fn try_build(
        preset: FoundryCampaignPreset,
        itinerary: FoundryCampaignItinerary,
        search_provenance: FoundrySearchProvenance,
        probes: impl IntoIterator<Item = FoundryCampaignProbe>,
        interior_margin: u64,
        polynomial_degree_ceiling: usize,
        ordering: OrderingPolicy,
        discovery_coordinate_priority: Option<CoordinatePriority>,
        max_task_reports: usize,
        max_reported_uncovered_boxes: usize,
    ) -> Result<Self, FoundryCampaignConfigError> {
        let probes = probes.into_iter().collect::<Box<[_]>>();
        if probes.is_empty() {
            return Err(FoundryCampaignConfigError::EmptyProbeProgram);
        }
        if interior_margin == 0 {
            return Err(FoundryCampaignConfigError::ZeroInteriorMargin);
        }
        if max_task_reports == 0 {
            return Err(FoundryCampaignConfigError::ZeroTaskReportLimit);
        }
        let (expected_base, expected_chart) = match preset {
            FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0 => (1, 6),
        };
        if let Some(actual) = ordering.coordinate_priority_arity()
            && actual != expected_chart
        {
            return Err(FoundryCampaignConfigError::WrongOrderingPolicyArity {
                expected: expected_chart,
                actual,
            });
        }
        for (probe_ordinal, probe) in probes.iter().enumerate() {
            if probe.base_parameters().len() != expected_base {
                return Err(FoundryCampaignConfigError::WrongProbeBaseParameterArity {
                    probe_ordinal,
                    expected: expected_base,
                    actual: probe.base_parameters().len(),
                });
            }
            if probe.chart_offsets().len() != expected_chart {
                return Err(FoundryCampaignConfigError::WrongProbeChartOffsetArity {
                    probe_ordinal,
                    expected: expected_chart,
                    actual: probe.chart_offsets().len(),
                });
            }
            TaskRelativeModularProbe::try_new(
                probe.modulus(),
                probe.base_parameters().iter().copied(),
                probe.chart_offsets().iter().copied(),
                CampaignLimits::default(),
            )
            .map_err(|error| FoundryCampaignConfigError::InvalidProbe {
                probe_ordinal,
                message: error.to_string(),
            })?;
        }
        let ordering_priority = ordering.try_coordinate_priority().map_err(|error| {
            FoundryCampaignConfigError::InvalidOrderingPolicy {
                message: error.to_string(),
            }
        })?;
        let discovery_coordinate_priority = match discovery_coordinate_priority
            .or(ordering_priority)
        {
            Some(priority) if priority.arity() == expected_chart => priority,
            Some(priority) => {
                return Err(
                    FoundryCampaignConfigError::WrongDiscoveryCoordinatePriorityArity {
                        expected: expected_chart,
                        actual: priority.arity(),
                    },
                );
            }
            None => {
                CoordinatePriority::try_natural(expected_chart, CoordinatePriorityLimits::default())
                    .map_err(|error| {
                        FoundryCampaignConfigError::InvalidDiscoveryCoordinatePriority {
                            message: error.to_string(),
                        }
                    })?
            }
        };
        Ok(Self {
            preset,
            itinerary,
            search_provenance,
            probes,
            interior_margin,
            polynomial_degree_ceiling,
            ordering,
            discovery_coordinate_priority,
            max_task_reports,
            max_reported_uncovered_boxes,
        })
    }

    fn try_autonomous(
        preset: FoundryCampaignPreset,
        itinerary: FoundryCampaignItinerary,
        max_task_reports: usize,
        max_reported_uncovered_boxes: usize,
    ) -> Result<Self, FoundryCampaignConfigError> {
        match preset {
            FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0 => Self::try_build(
                preset,
                itinerary,
                FoundrySearchProvenance::Autonomous,
                [FoundryCampaignProbe::new(
                    1_000_000_007,
                    [37],
                    [0, 0, 0, 0, 0, 0],
                )],
                2,
                0,
                OrderingPolicy::default(),
                None,
                max_task_reports,
                max_reported_uncovered_boxes,
            ),
        }
    }

    pub fn try_autonomous_single_sector(
        preset: FoundryCampaignPreset,
        max_task_reports: usize,
        max_reported_uncovered_boxes: usize,
    ) -> Result<Self, FoundryCampaignConfigError> {
        Self::try_autonomous(
            preset,
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            max_task_reports,
            max_reported_uncovered_boxes,
        )
    }

    pub fn try_autonomous_full_rank_waves(
        preset: FoundryCampaignPreset,
        max_task_reports: usize,
        max_reported_uncovered_boxes: usize,
    ) -> Result<Self, FoundryCampaignConfigError> {
        Self::try_autonomous(
            preset,
            FoundryCampaignItinerary::FullRankAtomicWaves,
            max_task_reports,
            max_reported_uncovered_boxes,
        )
    }

    pub fn try_external_hints(
        preset: FoundryCampaignPreset,
        hints: FoundryCampaignExternalHints,
        max_task_reports: usize,
        max_reported_uncovered_boxes: usize,
    ) -> Result<Self, FoundryCampaignConfigError> {
        Self::try_build(
            preset,
            hints.itinerary,
            FoundrySearchProvenance::ExternalHintsOnly,
            hints.probes,
            hints.interior_margin,
            hints.polynomial_degree_ceiling,
            hints.ordering,
            hints.discovery_coordinate_priority,
            max_task_reports,
            max_reported_uncovered_boxes,
        )
    }

    /// Canonical one-probe K=6 orbit-0 diagnostic used by current Stage 1
    /// experiments. The caller still chooses the bounded report budget.
    pub fn try_three_loop_unit_mass_vacuum_k6_orbit_0(
        max_task_reports: usize,
        max_reported_uncovered_boxes: usize,
    ) -> Result<Self, FoundryCampaignConfigError> {
        Self::try_autonomous_single_sector(
            FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
            max_task_reports,
            max_reported_uncovered_boxes,
        )
    }

    pub const fn preset(&self) -> FoundryCampaignPreset {
        self.preset
    }

    pub const fn itinerary(&self) -> FoundryCampaignItinerary {
        self.itinerary
    }

    pub const fn search_provenance(&self) -> FoundrySearchProvenance {
        self.search_provenance
    }

    pub fn probes(&self) -> &[FoundryCampaignProbe] {
        &self.probes
    }

    pub const fn interior_margin(&self) -> u64 {
        self.interior_margin
    }

    pub const fn polynomial_degree_ceiling(&self) -> usize {
        self.polynomial_degree_ceiling
    }

    /// Exact persisted proof order used coherently by symmetry routing,
    /// terminal authority, owner ledgers, replay descent, and any artifact
    /// produced from this campaign. This is semantic, unlike the optional
    /// proposal-only discovery chronology below.
    pub const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub const fn discovery_coordinate_priority(&self) -> &CoordinatePriority {
        &self.discovery_coordinate_priority
    }

    pub const fn max_task_reports(&self) -> usize {
        self.max_task_reports
    }

    pub const fn max_reported_uncovered_boxes(&self) -> usize {
        self.max_reported_uncovered_boxes
    }
}
