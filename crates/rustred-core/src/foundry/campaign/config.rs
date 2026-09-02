use crate::family::IntegralKey;
use crate::foundry::completion::source_discovery::{CampaignLimits, TaskRelativeModularProbe};
use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, OrderingPolicy};

use super::{
    FoundryAutonomousSelectionTelemetry, FoundryCampaignConfigError, FoundryCampaignError,
    FoundrySearchProvenance,
};

/// Stable schema identifier for the deterministic report payload.
pub const FOUNDRY_CAMPAIGN_CONFIG_SCHEMA: &str = "rustred.foundry-campaign-config.toml.v2";
pub const FOUNDRY_CAMPAIGN_REPORT_SCHEMA: &str = "rustred.foundry-campaign-report.toml.v2";

/// Cold-ingress ceiling for reviewed requested-domain hints.
///
/// A domain hint is proposal metadata only. Keeping this independent of the
/// task-report budget prevents an input document from retaining unbounded
/// steering state before the requested-domain planner applies its own limits.
pub const MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINTS: usize = 4_096;

/// Maximum arity retained by one reviewed requested-domain hint.
pub const MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINT_ARITY: usize = 4_096;

/// Maximum number of probe templates admitted at the public campaign ingress.
pub const MAX_FOUNDRY_CAMPAIGN_PROBES: usize = 4_096;

/// Maximum retained base-parameter plus chart-offset coordinates in one probe.
pub const MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATES: usize = 8_192;

/// Maximum aggregate retained coordinates across one probe program.
pub const MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATE_CELLS: usize = 4_194_304;

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
    pub fn try_new(
        modulus: u64,
        base_parameters: impl IntoIterator<Item = i64>,
        chart_offsets: impl IntoIterator<Item = u64>,
    ) -> Result<Self, FoundryCampaignConfigError> {
        let mut retained_base = Vec::new();
        let mut coordinate_count = 0usize;
        for parameter in base_parameters {
            try_push_probe_coordinate(
                &mut retained_base,
                parameter,
                &mut coordinate_count,
                "foundry campaign probe base parameters",
            )?;
        }
        let mut retained_chart = Vec::new();
        for offset in chart_offsets {
            try_push_probe_coordinate(
                &mut retained_chart,
                offset,
                &mut coordinate_count,
                "foundry campaign probe chart offsets",
            )?;
        }
        Ok(Self {
            modulus,
            base_parameters: retained_base.into_boxed_slice(),
            chart_offsets: retained_chart.into_boxed_slice(),
        })
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

    fn retained_coordinate_cells(&self) -> usize {
        self.base_parameters.len() + self.chart_offsets.len()
    }
}

fn try_push_probe_coordinate<T>(
    retained: &mut Vec<T>,
    value: T,
    total: &mut usize,
    resource: &'static str,
) -> Result<(), FoundryCampaignConfigError> {
    let requested = total
        .checked_add(1)
        .ok_or(FoundryCampaignConfigError::ProbeCoordinateCountOverflow)?;
    if requested > MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATES {
        return Err(FoundryCampaignConfigError::TooManyProbeCoordinates {
            requested,
            limit: MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATES,
        });
    }
    retained
        .try_reserve_exact(1)
        .map_err(|_| FoundryCampaignConfigError::AllocationFailure {
            resource,
            requested,
        })?;
    retained.push(value);
    *total = requested;
    Ok(())
}

fn try_collect_probes(
    probes: impl IntoIterator<Item = FoundryCampaignProbe>,
) -> Result<Box<[FoundryCampaignProbe]>, FoundryCampaignConfigError> {
    let mut retained = Vec::new();
    let mut coordinate_cells = 0usize;
    for probe in probes {
        let requested = retained
            .len()
            .checked_add(1)
            .ok_or(FoundryCampaignConfigError::ProbeCountOverflow)?;
        if requested > MAX_FOUNDRY_CAMPAIGN_PROBES {
            return Err(FoundryCampaignConfigError::TooManyProbes {
                requested,
                limit: MAX_FOUNDRY_CAMPAIGN_PROBES,
            });
        }
        coordinate_cells = coordinate_cells
            .checked_add(probe.retained_coordinate_cells())
            .ok_or(FoundryCampaignConfigError::ProbeCoordinateCountOverflow)?;
        if coordinate_cells > MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATE_CELLS {
            return Err(
                FoundryCampaignConfigError::TooManyAggregateProbeCoordinates {
                    requested: coordinate_cells,
                    limit: MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATE_CELLS,
                },
            );
        }
        retained.try_reserve_exact(1).map_err(|_| {
            FoundryCampaignConfigError::AllocationFailure {
                resource: "foundry campaign probes",
                requested,
            }
        })?;
        retained.push(probe);
    }
    if retained.is_empty() {
        return Err(FoundryCampaignConfigError::EmptyProbeProgram);
    }
    Ok(retained.into_boxed_slice())
}

fn try_collect_domain_hints(
    domains: impl IntoIterator<Item = FoundryCampaignDomainHint>,
) -> Result<Vec<FoundryCampaignDomainHint>, FoundryCampaignConfigError> {
    let mut retained = Vec::new();
    for domain in domains {
        let requested = retained
            .len()
            .checked_add(1)
            .ok_or(FoundryCampaignConfigError::DomainHintCountOverflow)?;
        if requested > MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINTS {
            return Err(FoundryCampaignConfigError::TooManyDomainHints {
                requested,
                limit: MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINTS,
            });
        }
        retained.try_reserve_exact(1).map_err(|_| {
            FoundryCampaignConfigError::AllocationFailure {
                resource: "foundry campaign domain hints",
                requested,
            }
        })?;
        retained.push(domain);
    }
    Ok(retained)
}

/// One structurally restricted requested-domain hint.
///
/// The anchor is an exact integral key. `symbolic_axes` is a strictly
/// increasing subset of its coordinates; those axes will eventually denote
/// the unbounded directions of the requested rectangle while all remaining
/// powers stay fixed at the anchor. This value cannot carry a source row,
/// support, coefficient, recurrence, owner, terminal, master, or reduction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryCampaignDomainHint {
    anchor: IntegralKey,
    symbolic_axes: Box<[usize]>,
}

impl FoundryCampaignDomainHint {
    pub fn try_new(
        anchor: IntegralKey,
        symbolic_axes: impl IntoIterator<Item = usize>,
    ) -> Result<Self, FoundryCampaignConfigError> {
        let arity = anchor.powers().len();
        if arity > MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINT_ARITY {
            return Err(FoundryCampaignConfigError::DomainHintArityLimit {
                actual: arity,
                limit: MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINT_ARITY,
            });
        }
        let mut retained = Vec::new();
        retained.try_reserve_exact(arity).map_err(|_| {
            FoundryCampaignConfigError::AllocationFailure {
                resource: "foundry campaign domain symbolic axes",
                requested: arity,
            }
        })?;
        for axis in symbolic_axes {
            if axis >= arity {
                return Err(FoundryCampaignConfigError::DomainHintAxisOutOfBounds { axis, arity });
            }
            if let Some(&previous) = retained.last()
                && previous >= axis
            {
                return Err(
                    FoundryCampaignConfigError::DomainHintAxesNotStrictlyIncreasing {
                        previous,
                        current: axis,
                    },
                );
            }
            retained.push(axis);
        }
        Ok(Self {
            anchor,
            symbolic_axes: retained.into_boxed_slice(),
        })
    }

    pub const fn anchor(&self) -> &IntegralKey {
        &self.anchor
    }

    pub fn symbolic_axes(&self) -> &[usize] {
        &self.symbolic_axes
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
    domains: Box<[FoundryCampaignDomainHint]>,
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
        Self::try_new_with_domains(
            itinerary,
            probes,
            interior_margin,
            polynomial_degree_ceiling,
            ordering,
            discovery_coordinate_priority,
            [],
        )
    }

    pub fn try_new_with_domains(
        itinerary: FoundryCampaignItinerary,
        probes: impl IntoIterator<Item = FoundryCampaignProbe>,
        interior_margin: u64,
        polynomial_degree_ceiling: usize,
        ordering: OrderingPolicy,
        discovery_coordinate_priority: Option<CoordinatePriority>,
        domains: impl IntoIterator<Item = FoundryCampaignDomainHint>,
    ) -> Result<Self, FoundryCampaignConfigError> {
        if interior_margin == 0 {
            return Err(FoundryCampaignConfigError::ZeroInteriorMargin);
        }
        let probes = try_collect_probes(probes)?;
        let retained_domains = try_collect_domain_hints(domains)?;
        Ok(Self {
            itinerary,
            probes,
            interior_margin,
            polynomial_degree_ceiling,
            ordering,
            discovery_coordinate_priority,
            domains: retained_domains.into_boxed_slice(),
        })
    }

    pub const fn itinerary(&self) -> FoundryCampaignItinerary {
        self.itinerary
    }

    pub fn domains(&self) -> &[FoundryCampaignDomainHint] {
        &self.domains
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
    domain_hints: Box<[FoundryCampaignDomainHint]>,
    max_task_reports: usize,
    max_reported_uncovered_boxes: usize,
    autonomous_selection: Option<FoundryAutonomousSelectionTelemetry>,
}

impl FoundryCampaignConfig {
    pub const fn schema(&self) -> &'static str {
        FOUNDRY_CAMPAIGN_CONFIG_SCHEMA
    }

    pub(super) fn try_build(
        preset: FoundryCampaignPreset,
        itinerary: FoundryCampaignItinerary,
        search_provenance: FoundrySearchProvenance,
        probes: impl IntoIterator<Item = FoundryCampaignProbe>,
        interior_margin: u64,
        polynomial_degree_ceiling: usize,
        ordering: OrderingPolicy,
        discovery_coordinate_priority: Option<CoordinatePriority>,
        domain_hints: impl IntoIterator<Item = FoundryCampaignDomainHint>,
        max_task_reports: usize,
        max_reported_uncovered_boxes: usize,
    ) -> Result<Self, FoundryCampaignConfigError> {
        if interior_margin == 0 {
            return Err(FoundryCampaignConfigError::ZeroInteriorMargin);
        }
        if max_task_reports == 0 {
            return Err(FoundryCampaignConfigError::ZeroTaskReportLimit);
        }
        let probes = try_collect_probes(probes)?;
        let domain_hints = try_collect_domain_hints(domain_hints)?.into_boxed_slice();
        if search_provenance == FoundrySearchProvenance::Autonomous && !domain_hints.is_empty() {
            return Err(FoundryCampaignConfigError::AutonomousDomainHints);
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
        for (domain_ordinal, domain) in domain_hints.iter().enumerate() {
            if domain.anchor().powers().len() != expected_chart {
                return Err(FoundryCampaignConfigError::WrongDomainHintAnchorArity {
                    domain_ordinal,
                    expected: expected_chart,
                    actual: domain.anchor().powers().len(),
                });
            }
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
            domain_hints,
            max_task_reports,
            max_reported_uncovered_boxes,
            autonomous_selection: None,
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
                [FoundryCampaignProbe::try_new(
                    1_000_000_007,
                    [37],
                    [0, 0, 0, 0, 0, 0],
                )?],
                2,
                0,
                OrderingPolicy::default(),
                None,
                [],
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
            hints.domains,
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

    /// Reviewed proposal domains. Autonomous configurations always return an
    /// empty slice; these values never carry proof or publication authority.
    pub fn domain_hints(&self) -> &[FoundryCampaignDomainHint] {
        &self.domain_hints
    }

    pub const fn max_task_reports(&self) -> usize {
        self.max_task_reports
    }

    pub const fn max_reported_uncovered_boxes(&self) -> usize {
        self.max_reported_uncovered_boxes
    }

    /// Resolve RustRed's internal autonomous ordering/probe selector at the
    /// execution boundary. Parsing and construction remain free of source,
    /// symmetry, modular, and terminal setup. External-hints configurations
    /// and previously resolved autonomous values are returned unchanged.
    pub fn try_resolve_search_program(&self) -> Result<Self, FoundryCampaignError> {
        if self.search_provenance != FoundrySearchProvenance::Autonomous
            || self.autonomous_selection.is_some()
        {
            return Ok(self.clone());
        }
        if !self.domain_hints.is_empty() {
            return Err(FoundryCampaignError::Invariant {
                detail: "autonomous search-program resolution retained external domains",
            });
        }
        let selected = super::autonomous::try_select_autonomous_k6_search_program()?;
        let mut resolved = self.clone();
        resolved.ordering = selected.ordering();
        resolved.discovery_coordinate_priority = selected.priority().clone();
        resolved.probes = selected.probes().to_vec().into_boxed_slice();
        resolved.autonomous_selection = Some(selected.telemetry().clone());
        Ok(resolved)
    }

    /// Proposal-only autonomous selector telemetry, present only after
    /// [`Self::try_resolve_search_program`]. It carries no proof authority.
    pub const fn autonomous_selection(&self) -> Option<&FoundryAutonomousSelectionTelemetry> {
        self.autonomous_selection.as_ref()
    }
}
