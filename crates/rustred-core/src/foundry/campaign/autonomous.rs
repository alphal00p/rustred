//! Autonomous, proposal-only selection of a K6 search program.
//!
//! The selector deliberately has no input for external hints.  It enumerates
//! all `6!` coordinate priorities, scores their exact sparse leading pattern
//! across every unresolved registered K6 sector, and admits only a small
//! finalist set to ordering-specific symmetry/root construction. Successive
//! bounded finite-field screens use the regenerated nine ordinary IBP rows. Their
//! ledgers, samples, queries, and caches are dropped before the selected
//! ordering is returned; none can authorize an owner or publication.

use std::cmp::Ordering;
use std::mem::size_of;
use std::sync::OnceLock;

use crate::family::IntegralKey;
use crate::foundry::artifact::FULL_RANK_ORBITS;
use crate::foundry::completion::SectorChart;
use crate::foundry::completion::frame::modular::{
    ModularKernelError, ModularKernelLimits, ModularTargetQuery,
};
use crate::foundry::completion::frame::{OneSidedChartFrame, PhysicalFrameLimits};
use crate::foundry::completion::source_discovery::{
    CanonicalExactOwnerLedger, ProbeCampaignLimits,
};
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, OrderingPolicy};

use super::k6_resource::K6CampaignResourceProfile;
use super::preset_k6::{
    K6AlgebraInputs, k6_root_predecessor_for_ordering, shared_k6_algebra_inputs,
    try_new_k6_full_rank_ledger_with_profile_and_ordering,
};
use super::{FoundryCampaignError, FoundryCampaignProbe, FoundryCampaignSetupStage};

const K6_ARITY: usize = 6;
const K6_PRIORITY_COUNT: usize = 720;
const K6_SOURCE_ROW_COUNT: usize = 9;
const K6_SYMMETRY_GROUP_ORDER: usize = 24;
const K6_STATIC_FINALIST_COUNT: usize = 8;
const K6_STATIC_SOURCE_TERM_CEILING: usize = K6_SOURCE_ROW_COUNT * 96;
const K6_RETAINED_TARGET_ORDER_CELL_CEILING: usize =
    K6_PRIORITY_COUNT * K6_STATIC_SOURCE_TERM_CEILING;
const K6_RETAINED_HIT_LEDGER_CELL_CEILING: usize =
    K6_STATIC_FINALIST_COUNT * K6_STATIC_SOURCE_TERM_CEILING;
const K6_SELECTION_ALGORITHM: &str = "rustred.autonomous-k6-selector.v1";

/// Proposal-only telemetry for one deterministic autonomous selection.
///
/// This value contains scalar counts and the selected public search choices;
/// it carries no source row, modular value, ledger identity, owner, terminal,
/// rule, or publication capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryAutonomousSelectionTelemetry {
    algorithm: &'static str,
    coordinate_priority_count: usize,
    authenticated_source_row_count: usize,
    authenticated_source_term_count: usize,
    screened_sector_count: usize,
    symmetry_group_order: usize,
    caller_hint_count: usize,
    static_finalist_count: usize,
    ordering_specific_screen_root_count: usize,
    modular_physical_sample_count: usize,
    modular_target_query_count: usize,
    modular_bounded_query_count: usize,
    retained_target_order_cell_count: usize,
    retained_target_order_byte_count: usize,
    retained_hit_ledger_cell_count: usize,
    retained_hit_ledger_byte_count: usize,
    rounds: Box<[FoundryAutonomousSelectionRound]>,
    selected_priority: CoordinatePriority,
    selected_probe_count: usize,
    fresh_proof_root_required: bool,
}

impl FoundryAutonomousSelectionTelemetry {
    pub const fn algorithm(&self) -> &'static str {
        self.algorithm
    }

    pub const fn coordinate_priority_count(&self) -> usize {
        self.coordinate_priority_count
    }

    pub const fn authenticated_source_row_count(&self) -> usize {
        self.authenticated_source_row_count
    }

    pub const fn authenticated_source_term_count(&self) -> usize {
        self.authenticated_source_term_count
    }

    pub const fn screened_sector_count(&self) -> usize {
        self.screened_sector_count
    }

    pub const fn symmetry_group_order(&self) -> usize {
        self.symmetry_group_order
    }

    /// Autonomous selection has no external-hint ingress.  This pinned zero
    /// is retained in reports so that provenance audits need not infer the
    /// absence of an input from a missing field.
    pub const fn caller_hint_count(&self) -> usize {
        self.caller_hint_count
    }

    pub const fn static_finalist_count(&self) -> usize {
        self.static_finalist_count
    }

    pub const fn ordering_specific_screen_root_count(&self) -> usize {
        self.ordering_specific_screen_root_count
    }

    /// Number of finite-field frames physically sampled, i.e. built-in probes
    /// multiplied by unresolved sector screens. Candidate queries reuse each
    /// physical sample and are counted separately.
    pub const fn modular_physical_sample_count(&self) -> usize {
        self.modular_physical_sample_count
    }

    pub const fn modular_target_query_count(&self) -> usize {
        self.modular_target_query_count
    }

    pub const fn modular_bounded_query_count(&self) -> usize {
        self.modular_bounded_query_count
    }

    /// Peak number of retained `usize` chronology cells across the exhaustive
    /// static pass and the ordering-specific finalist rebuild.
    pub const fn retained_target_order_cell_count(&self) -> usize {
        self.retained_target_order_cell_count
    }

    pub const fn retained_target_order_byte_count(&self) -> usize {
        self.retained_target_order_byte_count
    }

    /// Number of retained `usize` modular-hit cells. Hit ledgers exist only for
    /// the admitted finalists, never for all 720 static candidates.
    pub const fn retained_hit_ledger_cell_count(&self) -> usize {
        self.retained_hit_ledger_cell_count
    }

    pub const fn retained_hit_ledger_byte_count(&self) -> usize {
        self.retained_hit_ledger_byte_count
    }

    pub fn rounds(&self) -> &[FoundryAutonomousSelectionRound] {
        &self.rounds
    }

    pub const fn selected_priority(&self) -> &CoordinatePriority {
        &self.selected_priority
    }

    pub const fn selected_probe_count(&self) -> usize {
        self.selected_probe_count
    }

    pub const fn fresh_proof_root_required(&self) -> bool {
        self.fresh_proof_root_required
    }
}

/// One deterministic successive-deepening step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoundryAutonomousSelectionRound {
    input_candidate_count: usize,
    new_probe_count: usize,
    retained_candidate_count: usize,
    target_query_count: usize,
    bounded_query_count: usize,
}

impl FoundryAutonomousSelectionRound {
    pub const fn input_candidate_count(self) -> usize {
        self.input_candidate_count
    }

    /// Newly admitted built-in probe points in this deepening round. Each is
    /// evaluated once per unresolved sector, as reported by the top-level
    /// physical-sample count.
    pub const fn new_probe_count(self) -> usize {
        self.new_probe_count
    }

    pub const fn retained_candidate_count(self) -> usize {
        self.retained_candidate_count
    }

    pub const fn target_query_count(self) -> usize {
        self.target_query_count
    }

    pub const fn bounded_query_count(self) -> usize {
        self.bounded_query_count
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AutonomousK6SearchProgram {
    ordering: OrderingPolicy,
    priority: CoordinatePriority,
    probes: Box<[FoundryCampaignProbe]>,
    telemetry: FoundryAutonomousSelectionTelemetry,
}

impl AutonomousK6SearchProgram {
    pub(super) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(super) const fn priority(&self) -> &CoordinatePriority {
        &self.priority
    }

    pub(super) fn probes(&self) -> &[FoundryCampaignProbe] {
        &self.probes
    }

    pub(super) const fn telemetry(&self) -> &FoundryAutonomousSelectionTelemetry {
        &self.telemetry
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AutonomousK6SelectorLimits {
    max_coordinate_priorities: usize,
    max_priority_rank_cells: usize,
    max_static_source_term_visits: usize,
    max_static_finalists: usize,
    max_ordering_specific_roots: usize,
    max_modular_physical_samples: usize,
    max_modular_target_queries: usize,
    max_retained_target_order_cells: usize,
    max_retained_target_order_bytes: usize,
    max_retained_hit_ledger_cells: usize,
    max_retained_hit_ledger_bytes: usize,
}

impl Default for AutonomousK6SelectorLimits {
    fn default() -> Self {
        Self {
            max_coordinate_priorities: K6_PRIORITY_COUNT,
            max_priority_rank_cells: K6_PRIORITY_COUNT * K6_ARITY,
            max_static_source_term_visits: (K6_PRIORITY_COUNT + K6_STATIC_FINALIST_COUNT)
                * K6_SOURCE_ROW_COUNT
                * 96,
            max_static_finalists: K6_STATIC_FINALIST_COUNT,
            max_ordering_specific_roots: K6_STATIC_FINALIST_COUNT * FULL_RANK_ORBITS.len(),
            max_modular_physical_samples: 6 * FULL_RANK_ORBITS.len(),
            max_modular_target_queries: 8_192,
            max_retained_target_order_cells: K6_RETAINED_TARGET_ORDER_CELL_CEILING,
            max_retained_target_order_bytes: K6_RETAINED_TARGET_ORDER_CELL_CEILING
                * size_of::<usize>(),
            max_retained_hit_ledger_cells: K6_RETAINED_HIT_LEDGER_CELL_CEILING,
            max_retained_hit_ledger_bytes: K6_RETAINED_HIT_LEDGER_CELL_CEILING * size_of::<usize>(),
        }
    }
}

impl AutonomousK6SelectorLimits {
    fn try_admit_coordinate_priority_portfolio(self) -> Result<(), FoundryCampaignError> {
        admit(
            "coordinate priorities",
            K6_PRIORITY_COUNT,
            self.max_coordinate_priorities,
        )?;
        admit(
            "coordinate-priority rank cells",
            checked_product(
                "coordinate-priority rank cells",
                K6_PRIORITY_COUNT,
                K6_ARITY,
            )?,
            self.max_priority_rank_cells,
        )
    }

    fn try_admit_static_finalists(self) -> Result<(), FoundryCampaignError> {
        admit(
            "static finalists",
            K6_STATIC_FINALIST_COUNT,
            self.max_static_finalists,
        )
    }

    fn try_admit_static_source_term_visits(
        self,
        requested: usize,
    ) -> Result<(), FoundryCampaignError> {
        admit(
            "static source-term visits",
            requested,
            self.max_static_source_term_visits,
        )
    }

    fn try_admit_ordering_specific_roots(
        self,
        requested: usize,
    ) -> Result<(), FoundryCampaignError> {
        admit(
            "ordering-specific screen roots",
            requested,
            self.max_ordering_specific_roots,
        )
    }

    fn try_admit_modular_physical_samples(
        self,
        requested: usize,
    ) -> Result<(), FoundryCampaignError> {
        admit(
            "modular physical samples",
            requested,
            self.max_modular_physical_samples,
        )
    }

    fn try_admit_modular_target_queries(
        self,
        requested: usize,
    ) -> Result<(), FoundryCampaignError> {
        admit(
            "modular target queries",
            requested,
            self.max_modular_target_queries,
        )
    }

    fn try_admit_retained_target_order_cells(
        self,
        requested: usize,
    ) -> Result<(), FoundryCampaignError> {
        admit_retained_cells(
            "retained target-order cells",
            "retained target-order bytes",
            requested,
            self.max_retained_target_order_cells,
            self.max_retained_target_order_bytes,
        )
    }

    fn try_admit_retained_hit_ledger_cells(
        self,
        requested: usize,
    ) -> Result<(), FoundryCampaignError> {
        admit_retained_cells(
            "retained hit-ledger cells",
            "retained hit-ledger bytes",
            requested,
            self.max_retained_hit_ledger_cells,
            self.max_retained_hit_ledger_bytes,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StructuralScore {
    distinct_leading_targets: usize,
    rows_with_uncovered_leader: usize,
    leading_free_dimension_sum: usize,
    frontier_weight: usize,
    leading_rank_sum: usize,
}

#[derive(Clone, Debug)]
struct PriorityCandidate {
    priority: CoordinatePriority,
    ordering: OrderingPolicy,
    target_orders: Vec<Vec<usize>>,
    sector_structural: Vec<StructuralScore>,
    structural: StructuralScore,
    modular: ModularScore,
}

#[derive(Clone, Debug, Default)]
struct ModularScore {
    samples: usize,
    target_hit_counts: Vec<Vec<usize>>,
    total_hits: usize,
    bounded_queries: usize,
    total_fill: usize,
}

impl ModularScore {
    fn robust_hits(&self) -> usize {
        if self.samples == 0 {
            return 0;
        }
        self.target_hit_counts
            .iter()
            .flatten()
            .filter(|&&hits| hits == self.samples)
            .count()
    }

    fn robust_prefix_min(&self) -> usize {
        if self.samples == 0 {
            return 0;
        }
        self.target_hit_counts
            .iter()
            .map(|sector| {
                sector
                    .iter()
                    .take_while(|&&hits| hits == self.samples)
                    .count()
            })
            .min()
            .unwrap_or(0)
    }

    fn robust_prefix_sum(&self) -> usize {
        if self.samples == 0 {
            return 0;
        }
        self.target_hit_counts.iter().fold(0usize, |total, sector| {
            total.saturating_add(
                sector
                    .iter()
                    .take_while(|&&hits| hits == self.samples)
                    .count(),
            )
        })
    }

    fn worst_robust_coverage(&self) -> (usize, usize) {
        if self.samples == 0 {
            return (0, 1);
        }
        self.target_hit_counts
            .iter()
            .map(|sector| {
                (
                    sector.iter().filter(|&&hits| hits == self.samples).count(),
                    sector.len(),
                )
            })
            .min_by(compare_coverage)
            .unwrap_or((0, 1))
    }
}

#[derive(Debug)]
struct GeometryProfile {
    eligible_columns: Vec<usize>,
    free_dimension_by_column: Vec<usize>,
}

#[derive(Debug)]
struct SectorScreen {
    representative: [i64; K6_ARITY],
    frame: crate::foundry::completion::frame::PhysicalFramePlan,
    natural_geometry: GeometryProfile,
}

static AUTONOMOUS_K6_PROGRAM: OnceLock<Result<AutonomousK6SearchProgram, FoundryCampaignError>> =
    OnceLock::new();

pub(super) fn try_select_autonomous_k6_search_program()
-> Result<AutonomousK6SearchProgram, FoundryCampaignError> {
    AUTONOMOUS_K6_PROGRAM
        .get_or_init(|| try_select_uncached(AutonomousK6SelectorLimits::default()))
        .clone()
}

fn try_select_uncached(
    limits: AutonomousK6SelectorLimits,
) -> Result<AutonomousK6SearchProgram, FoundryCampaignError> {
    limits.try_admit_coordinate_priority_portfolio()?;
    limits.try_admit_static_finalists()?;

    let inputs = shared_k6_algebra_inputs()?;
    if inputs.completed().source_row_count() != K6_SOURCE_ROW_COUNT {
        return Err(invariant(
            "autonomous K6 selector did not receive the nine ordinary source rows",
        ));
    }
    let natural_predecessor = k6_root_predecessor_for_ordering(OrderingPolicy::default())?;
    let canonicalizer = natural_predecessor
        .canonicalizer()
        .ok_or_else(|| invariant("autonomous K6 screen root has no symmetry authority"))?;
    if canonicalizer.group_order() != K6_SYMMETRY_GROUP_ORDER {
        return Err(invariant(
            "autonomous K6 screen root does not retain the exact order-24 K4 action",
        ));
    }
    let mut screens = Vec::new();
    screens
        .try_reserve_exact(FULL_RANK_ORBITS.len())
        .map_err(|_| allocation("autonomous K6 sector screens", FULL_RANK_ORBITS.len()))?;
    for orbit in FULL_RANK_ORBITS {
        let ledger = try_new_screen_ledger_against_predecessor(
            inputs,
            orbit.representative,
            natural_predecessor.clone(),
            OrderingPolicy::default(),
        )?;
        if ledger.snapshot().status().is_compiler_closed() {
            continue;
        }
        let frame = OneSidedChartFrame::try_new(
            inputs.generator(),
            inputs.completed(),
            ledger.sector().clone(),
            0,
            PhysicalFrameLimits::default(),
        )
        .map_err(selection_setup)?
        .into_plan();
        if frame.row_count() != K6_SOURCE_ROW_COUNT {
            return Err(invariant(
                "autonomous K6 selector physical frame changed the ordinary row count",
            ));
        }
        let natural_geometry = try_geometry_profile(&ledger, &frame, orbit.representative)?;
        screens.push(SectorScreen {
            representative: orbit.representative,
            frame,
            natural_geometry,
        });
    }
    if screens.is_empty() {
        return Err(invariant(
            "autonomous K6 selector found no unresolved registered sector",
        ));
    }
    let source_term_count = screens.iter().try_fold(0usize, |count, screen| {
        count
            .checked_add(screen.frame.entry_count())
            .ok_or_else(|| overflow("authenticated source terms across sector screens"))
    })?;
    let term_visits = static_source_term_visits(source_term_count)?;
    limits.try_admit_static_source_term_visits(term_visits)?;
    let static_target_cells_per_candidate = screens.iter().try_fold(0usize, |count, screen| {
        count
            .checked_add(screen.natural_geometry.eligible_columns.len())
            .ok_or_else(|| overflow("retained target-order cells"))
    })?;
    let mut retained_target_order_cell_peak = checked_product(
        "retained target-order cells",
        K6_PRIORITY_COUNT,
        static_target_cells_per_candidate,
    )?;
    limits.try_admit_retained_target_order_cells(retained_target_order_cell_peak)?;
    let probes = try_built_in_probe_portfolio()?;
    limits.try_admit_modular_physical_samples(checked_product(
        "modular physical samples",
        probes.len(),
        screens.len(),
    )?)?;
    let candidates = enumerate_all_priorities(&screens, limits)?;
    let mut candidates = retain_static_finalists(candidates, screens.len())?;
    if candidates.len() != K6_STATIC_FINALIST_COUNT {
        return Err(invariant(
            "autonomous K6 selector did not retain its admitted static finalists",
        ));
    }

    limits.try_admit_ordering_specific_roots(checked_product(
        "ordering-specific screen roots",
        candidates.len(),
        screens.len(),
    )?)?;
    let mut retained_target_order_cells =
        candidates.iter().try_fold(0usize, |count, candidate| {
            candidate
                .target_orders
                .iter()
                .try_fold(count, |count, targets| {
                    count
                        .checked_add(targets.len())
                        .ok_or_else(|| overflow("retained target-order cells"))
                })
        })?;
    for candidate in &mut candidates {
        let predecessor = k6_root_predecessor_for_ordering(candidate.ordering)?;
        require_authenticated_k6_symmetry(&predecessor)?;
        let mut geometries = Vec::new();
        geometries
            .try_reserve_exact(screens.len())
            .map_err(|_| allocation("ordering-specific sector geometries", screens.len()))?;
        for screen in &screens {
            let ledger = try_new_screen_ledger_against_predecessor(
                inputs,
                screen.representative,
                predecessor.clone(),
                candidate.ordering,
            )?;
            if ledger.snapshot().status().is_compiler_closed() {
                return Err(invariant(
                    "an unresolved natural-order screen became terminal under a finalist ordering",
                ));
            }
            geometries.push(try_geometry_profile(
                &ledger,
                &screen.frame,
                screen.representative,
            )?);
        }
        let (target_orders, sector_structural, structural) =
            try_structural_program(&screens, &geometries, candidate.ordering)?;
        let replacement_cells = target_orders.iter().try_fold(0usize, |count, targets| {
            count
                .checked_add(targets.len())
                .ok_or_else(|| overflow("retained target-order cells"))
        })?;
        let transient_cells = retained_target_order_cells
            .checked_add(replacement_cells)
            .ok_or_else(|| overflow("retained target-order cells"))?;
        retained_target_order_cell_peak = retained_target_order_cell_peak.max(transient_cells);
        limits.try_admit_retained_target_order_cells(retained_target_order_cell_peak)?;
        let replaced_cells =
            candidate
                .target_orders
                .iter()
                .try_fold(0usize, |count, targets| {
                    count
                        .checked_add(targets.len())
                        .ok_or_else(|| overflow("retained target-order cells"))
                })?;
        retained_target_order_cells = retained_target_order_cells
            .checked_sub(replaced_cells)
            .and_then(|count| count.checked_add(replacement_cells))
            .ok_or_else(|| invariant("retained target-order accounting became inconsistent"))?;
        candidate.target_orders = target_orders;
        candidate.sector_structural = sector_structural;
        candidate.structural = structural;
    }
    let retained_hit_ledger_cells = retained_target_order_cells;
    limits.try_admit_retained_hit_ledger_cells(retained_hit_ledger_cells)?;
    for candidate in &mut candidates {
        candidate.modular.target_hit_counts = try_new_hit_ledgers(&candidate.target_orders)?;
    }
    let rounds = [(1usize, 4usize), (2, 2), (3, 1)];
    if rounds.iter().map(|round| round.0).sum::<usize>() != probes.len() {
        return Err(invariant(
            "autonomous K6 successive-deepening rounds do not consume the probe portfolio",
        ));
    }
    let mut round_reports = Vec::new();
    round_reports
        .try_reserve_exact(rounds.len())
        .map_err(|_| allocation("successive-deepening round telemetry", rounds.len()))?;
    let mut probe_start = 0usize;
    let mut total_queries = 0usize;
    let mut total_bounded = 0usize;
    for (new_probe_count, retained_count) in rounds {
        let input_candidate_count = candidates.len();
        let probe_end = probe_start
            .checked_add(new_probe_count)
            .ok_or_else(|| overflow("successive-deepening probe cursor"))?;
        let selected_probes = probes.get(probe_start..probe_end).ok_or_else(|| {
            invariant("successive-deepening probe slice is outside its portfolio")
        })?;
        let before_queries = total_queries;
        let before_bounded = total_bounded;
        for probe in selected_probes {
            let mut chart = Vec::new();
            chart
                .try_reserve_exact(probe.chart_offsets().len())
                .map_err(|_| {
                    allocation(
                        "autonomous K6 modular chart coordinates",
                        probe.chart_offsets().len(),
                    )
                })?;
            for &offset in probe.chart_offsets() {
                chart.push(offset.checked_add(1).ok_or_else(|| {
                    invariant("autonomous K6 screen chart coordinate overflowed u64")
                })?);
            }
            for (sector_ordinal, screen) in screens.iter().enumerate() {
                let sample = screen
                    .frame
                    .try_modular_sample(
                        inputs.generator().context(),
                        probe.modulus(),
                        probe.base_parameters(),
                        &chart,
                        ModularKernelLimits::default(),
                    )
                    .map_err(selection_setup)?;
                for candidate in &mut candidates {
                    let query_count = candidate.target_orders[sector_ordinal].len();
                    let requested = total_queries
                        .checked_add(query_count)
                        .ok_or_else(|| overflow("modular target queries"))?;
                    limits.try_admit_modular_target_queries(requested)?;
                    let bounded = try_screen_candidate_sector(&sample, sector_ordinal, candidate)?;
                    total_queries = requested;
                    total_bounded = total_bounded
                        .checked_add(bounded)
                        .ok_or_else(|| overflow("bounded modular target queries"))?;
                }
            }
            for candidate in &mut candidates {
                candidate.modular.samples = candidate
                    .modular
                    .samples
                    .checked_add(1)
                    .ok_or_else(|| overflow("modular finalist samples"))?;
            }
        }
        candidates.sort_unstable_by(compare_candidates);
        candidates.truncate(retained_count);
        round_reports.push(FoundryAutonomousSelectionRound {
            input_candidate_count,
            new_probe_count,
            retained_candidate_count: candidates.len(),
            target_query_count: total_queries - before_queries,
            bounded_query_count: total_bounded - before_bounded,
        });
        probe_start = probe_end;
    }
    let winner = candidates
        .into_iter()
        .next()
        .ok_or_else(|| invariant("autonomous K6 selector discarded every candidate"))?;
    let telemetry = FoundryAutonomousSelectionTelemetry {
        algorithm: K6_SELECTION_ALGORITHM,
        coordinate_priority_count: K6_PRIORITY_COUNT,
        authenticated_source_row_count: checked_product(
            "authenticated source rows across sector screens",
            K6_SOURCE_ROW_COUNT,
            screens.len(),
        )?,
        authenticated_source_term_count: source_term_count,
        screened_sector_count: screens.len(),
        symmetry_group_order: K6_SYMMETRY_GROUP_ORDER,
        caller_hint_count: 0,
        static_finalist_count: K6_STATIC_FINALIST_COUNT,
        ordering_specific_screen_root_count: checked_product(
            "ordering-specific screen roots",
            K6_STATIC_FINALIST_COUNT,
            screens.len(),
        )?,
        modular_physical_sample_count: checked_product(
            "modular physical samples",
            probes.len(),
            screens.len(),
        )?,
        modular_target_query_count: total_queries,
        modular_bounded_query_count: total_bounded,
        retained_target_order_cell_count: retained_target_order_cell_peak,
        retained_target_order_byte_count: retained_bytes(
            "retained target-order bytes",
            retained_target_order_cell_peak,
        )?,
        retained_hit_ledger_cell_count: retained_hit_ledger_cells,
        retained_hit_ledger_byte_count: retained_bytes(
            "retained hit-ledger bytes",
            retained_hit_ledger_cells,
        )?,
        rounds: round_reports.into_boxed_slice(),
        selected_priority: winner.priority.clone(),
        selected_probe_count: probes.len(),
        fresh_proof_root_required: true,
    };
    Ok(AutonomousK6SearchProgram {
        ordering: winner.ordering,
        priority: winner.priority,
        probes: probes.into_boxed_slice(),
        telemetry,
    })
}

fn enumerate_all_priorities(
    screens: &[SectorScreen],
    limits: AutonomousK6SelectorLimits,
) -> Result<Vec<PriorityCandidate>, FoundryCampaignError> {
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(K6_PRIORITY_COUNT)
        .map_err(|_| allocation("coordinate-priority candidates", K6_PRIORITY_COUNT))?;
    let mut ranks = [0, 1, 2, 3, 4, 5];
    loop {
        let priority =
            CoordinatePriority::try_new(K6_ARITY, &ranks, CoordinatePriorityLimits::default())
                .map_err(selection_setup)?;
        let ordering =
            OrderingPolicy::try_with_coordinate_priority(&priority).map_err(selection_setup)?;
        let (target_orders, sector_structural, structural) =
            try_structural_program_natural(screens, ordering)?;
        candidates.push(PriorityCandidate {
            priority,
            ordering,
            target_orders,
            sector_structural,
            structural,
            modular: ModularScore::default(),
        });
        if !next_permutation(&mut ranks) {
            break;
        }
    }
    if candidates.len() != K6_PRIORITY_COUNT {
        return Err(invariant(
            "autonomous K6 selector did not exhaust all 6! coordinate priorities",
        ));
    }
    admit(
        "coordinate priorities",
        candidates.len(),
        limits.max_coordinate_priorities,
    )?;
    Ok(candidates)
}

/// Retain the globally strongest static candidates while reserving one slot
/// for the best proposal seen by each unresolved sector.  The protected union
/// prevents an aggregate-rich ordering from eliminating every candidate that
/// treats one difficult sector well before the bounded modular rounds begin.
fn retain_static_finalists(
    mut candidates: Vec<PriorityCandidate>,
    sector_count: usize,
) -> Result<Vec<PriorityCandidate>, FoundryCampaignError> {
    if sector_count == 0 || candidates.is_empty() {
        return Err(invariant(
            "autonomous K6 static retention has no candidate or sector",
        ));
    }
    candidates.sort_unstable_by(compare_candidates);
    let mut selected_priorities = Vec::new();
    selected_priorities
        .try_reserve_exact(K6_STATIC_FINALIST_COUNT)
        .map_err(|_| {
            allocation(
                "protected static finalist priorities",
                K6_STATIC_FINALIST_COUNT,
            )
        })?;
    for sector in 0..sector_count {
        let candidate = candidates
            .iter()
            .min_by(|left, right| compare_candidates_for_sector(left, right, sector))
            .ok_or_else(|| invariant("a protected sector has no static candidate"))?;
        if !selected_priorities.contains(&candidate.priority) {
            selected_priorities.push(candidate.priority.clone());
        }
    }
    if selected_priorities.len() > K6_STATIC_FINALIST_COUNT {
        return Err(invariant(
            "protected sector finalists exceed the admitted finalist portfolio",
        ));
    }
    for candidate in &candidates {
        if selected_priorities.len() == K6_STATIC_FINALIST_COUNT {
            break;
        }
        if !selected_priorities.contains(&candidate.priority) {
            selected_priorities.push(candidate.priority.clone());
        }
    }
    let mut finalists = Vec::new();
    finalists
        .try_reserve_exact(K6_STATIC_FINALIST_COUNT)
        .map_err(|_| allocation("protected static finalists", K6_STATIC_FINALIST_COUNT))?;
    for candidate in candidates {
        if selected_priorities.contains(&candidate.priority) {
            finalists.push(candidate);
        }
    }
    finalists.sort_unstable_by(compare_candidates);
    Ok(finalists)
}

fn try_structural_score(
    frame: &crate::foundry::completion::frame::PhysicalFramePlan,
    geometry: &GeometryProfile,
    ordering: OrderingPolicy,
) -> Result<(Vec<usize>, StructuralScore), FoundryCampaignError> {
    let mut keyed = Vec::new();
    keyed
        .try_reserve_exact(geometry.eligible_columns.len())
        .map_err(|_| {
            allocation(
                "static ordered target columns",
                geometry.eligible_columns.len(),
            )
        })?;
    for &column in &geometry.eligible_columns {
        let shift = frame
            .columns()
            .get(column)
            .ok_or_else(|| invariant("static target column is outside its exact frame"))?;
        keyed.push((
            ordering
                .shift_complexity_key(frame.sector(), shift.values())
                .map_err(selection_setup)?,
            column,
        ));
    }
    keyed.sort_unstable_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    let mut target_order = Vec::new();
    target_order
        .try_reserve_exact(keyed.len())
        .map_err(|_| allocation("static target chronology", keyed.len()))?;
    for (_, column) in keyed {
        target_order.push(column);
    }
    let mut rank_by_column = try_filled_usize_vec(
        "static target ranks by exact-frame column",
        frame.columns().len(),
        usize::MAX,
    )?;
    for (rank, &column) in target_order.iter().enumerate() {
        rank_by_column[column] = rank;
    }
    let mut leading = Vec::new();
    leading
        .try_reserve_exact(frame.row_count())
        .map_err(|_| allocation("distinct static leading targets", frame.row_count()))?;
    let mut score = StructuralScore::default();
    for row in 0..frame.row_count() {
        let columns = frame
            .column_indices_for_row(row)
            .ok_or_else(|| invariant("static source row has malformed CSR bounds"))?;
        let leader = columns
            .iter()
            .filter_map(|&column| usize::try_from(column).ok())
            .filter(|&column| rank_by_column.get(column).copied() != Some(usize::MAX))
            .min_by_key(|&column| rank_by_column[column]);
        if let Some(column) = leader {
            let rank = rank_by_column[column];
            score.rows_with_uncovered_leader += 1;
            score.leading_rank_sum = score
                .leading_rank_sum
                .checked_add(rank)
                .ok_or_else(|| overflow("static leading-rank sum"))?;
            score.frontier_weight = score
                .frontier_weight
                .checked_add(target_order.len() - rank)
                .ok_or_else(|| overflow("static frontier weight"))?;
            score.leading_free_dimension_sum = score
                .leading_free_dimension_sum
                .checked_add(geometry.free_dimension_by_column[column])
                .ok_or_else(|| overflow("static leading free-dimension sum"))?;
            if !leading.contains(&column) {
                leading.push(column);
            }
        }
    }
    score.distinct_leading_targets = leading.len();
    Ok((target_order, score))
}

fn try_structural_program(
    screens: &[SectorScreen],
    geometries: &[GeometryProfile],
    ordering: OrderingPolicy,
) -> Result<(Vec<Vec<usize>>, Vec<StructuralScore>, StructuralScore), FoundryCampaignError> {
    try_structural_program_iter(screens, geometries.iter(), ordering)
}

fn try_structural_program_natural(
    screens: &[SectorScreen],
    ordering: OrderingPolicy,
) -> Result<(Vec<Vec<usize>>, Vec<StructuralScore>, StructuralScore), FoundryCampaignError> {
    try_structural_program_iter(
        screens,
        screens.iter().map(|screen| &screen.natural_geometry),
        ordering,
    )
}

fn try_structural_program_iter<'a>(
    screens: &[SectorScreen],
    geometries: impl ExactSizeIterator<Item = &'a GeometryProfile>,
    ordering: OrderingPolicy,
) -> Result<(Vec<Vec<usize>>, Vec<StructuralScore>, StructuralScore), FoundryCampaignError> {
    if screens.is_empty() || screens.len() != geometries.len() {
        return Err(invariant(
            "autonomous K6 structural program has inconsistent sector geometry",
        ));
    }
    let mut target_orders = Vec::new();
    target_orders
        .try_reserve_exact(screens.len())
        .map_err(|_| allocation("structural sector target orders", screens.len()))?;
    let mut sector_scores = Vec::new();
    sector_scores
        .try_reserve_exact(screens.len())
        .map_err(|_| allocation("structural sector scores", screens.len()))?;
    let mut aggregate = StructuralScore::default();
    for (screen, geometry) in screens.iter().zip(geometries) {
        let (target_order, score) = try_structural_score(&screen.frame, geometry, ordering)?;
        aggregate.distinct_leading_targets = aggregate
            .distinct_leading_targets
            .checked_add(score.distinct_leading_targets)
            .ok_or_else(|| overflow("aggregate distinct leading targets"))?;
        aggregate.rows_with_uncovered_leader = aggregate
            .rows_with_uncovered_leader
            .checked_add(score.rows_with_uncovered_leader)
            .ok_or_else(|| overflow("aggregate rows with uncovered leader"))?;
        aggregate.leading_free_dimension_sum = aggregate
            .leading_free_dimension_sum
            .checked_add(score.leading_free_dimension_sum)
            .ok_or_else(|| overflow("aggregate leading free-dimension sum"))?;
        aggregate.frontier_weight = aggregate
            .frontier_weight
            .checked_add(score.frontier_weight)
            .ok_or_else(|| overflow("aggregate structural frontier weight"))?;
        aggregate.leading_rank_sum = aggregate
            .leading_rank_sum
            .checked_add(score.leading_rank_sum)
            .ok_or_else(|| overflow("aggregate structural leading-rank sum"))?;
        target_orders.push(target_order);
        sector_scores.push(score);
    }
    Ok((target_orders, sector_scores, aggregate))
}

fn try_geometry_profile(
    ledger: &CanonicalExactOwnerLedger,
    frame: &crate::foundry::completion::frame::PhysicalFramePlan,
    representative: [i64; K6_ARITY],
) -> Result<GeometryProfile, FoundryCampaignError> {
    if ledger.revision().get() != 0 || ledger.snapshot().owner_count() != 0 {
        return Err(invariant(
            "autonomous K6 screen geometry did not come from a fresh owner-free ledger",
        ));
    }
    let partition = ledger
        .try_clone_uncovered_partition()
        .map_err(|error| selection_setup(error))?;
    let chart = SectorChart::new(ledger.sector().clone());
    let mut eligible_columns = Vec::new();
    eligible_columns
        .try_reserve_exact(frame.columns().len())
        .map_err(|_| allocation("screen geometry target columns", frame.columns().len()))?;
    let mut free_dimension_by_column = try_filled_usize_vec(
        "screen free dimensions by exact-frame column",
        frame.columns().len(),
        0,
    )?;
    for (column, shift) in frame.columns().iter().enumerate() {
        let mut powers = Vec::new();
        powers
            .try_reserve_exact(K6_ARITY)
            .map_err(|_| allocation("screen geometry integral powers", K6_ARITY))?;
        let mut representable = true;
        for (&base, &delta) in representative.iter().zip(shift.values()) {
            let Some(power) = base.checked_add(delta) else {
                representable = false;
                break;
            };
            powers.push(power);
        }
        if !representable {
            continue;
        }
        let integral = IntegralKey::try_new(powers).map_err(selection_setup)?;
        let Ok(point) = chart.to_lattice(&integral) else {
            continue;
        };
        if let Some(lattice_box) = partition.containing_box(&point) {
            eligible_columns.push(column);
            free_dimension_by_column[column] = lattice_box.free_dimension();
        }
    }
    if eligible_columns.is_empty() {
        return Err(invariant(
            "autonomous K6 screen root exposes no uncovered ordinary-source target",
        ));
    }
    Ok(GeometryProfile {
        eligible_columns,
        free_dimension_by_column,
    })
}

fn try_screen_candidate_sector(
    sample: &crate::foundry::completion::frame::modular::ModularPhysicalFrame<'_>,
    sector_ordinal: usize,
    candidate: &mut PriorityCandidate,
) -> Result<usize, FoundryCampaignError> {
    let (target_orders, modular) = (&candidate.target_orders, &mut candidate.modular);
    let target_order = target_orders
        .get(sector_ordinal)
        .ok_or_else(|| invariant("autonomous K6 modular screen has no sector chronology"))?;
    let hit_counts = modular
        .target_hit_counts
        .get_mut(sector_ordinal)
        .ok_or_else(|| invariant("autonomous K6 modular screen has no sector hit ledger"))?;
    if hit_counts.len() != target_order.len() {
        return Err(invariant(
            "autonomous K6 modular score lost its target chronology",
        ));
    }
    let mut bounded = 0usize;
    for (target_rank, &target) in target_order.iter().enumerate() {
        let forbidden = &target_order[..target_rank];
        match sample.query_target(target, forbidden, ModularKernelLimits::default()) {
            Ok(query) => {
                let diagnostics = query.diagnostics();
                modular.total_fill = modular
                    .total_fill
                    .checked_add(diagnostics.augmented_total_fill_nonzeros)
                    .ok_or_else(|| overflow("modular finalist total fill"))?;
                if matches!(query, ModularTargetQuery::Hit(_)) {
                    hit_counts[target_rank] = hit_counts[target_rank]
                        .checked_add(1)
                        .ok_or_else(|| overflow("modular target hit count"))?;
                    modular.total_hits = modular
                        .total_hits
                        .checked_add(1)
                        .ok_or_else(|| overflow("modular finalist total hits"))?;
                }
            }
            Err(ModularKernelError::ResourceLimit { .. }) => {
                bounded = bounded
                    .checked_add(1)
                    .ok_or_else(|| overflow("bounded modular target queries"))?;
                modular.bounded_queries = modular
                    .bounded_queries
                    .checked_add(1)
                    .ok_or_else(|| overflow("modular finalist bounded queries"))?;
            }
            Err(error) => return Err(selection_setup(error)),
        }
    }
    Ok(bounded)
}

fn require_authenticated_k6_symmetry(
    predecessor: &ImmutableOwnerSnapshot,
) -> Result<(), FoundryCampaignError> {
    let canonicalizer = predecessor
        .canonicalizer()
        .ok_or_else(|| invariant("autonomous K6 screen root has no symmetry authority"))?;
    if canonicalizer.group_order() != K6_SYMMETRY_GROUP_ORDER {
        return Err(invariant(
            "autonomous K6 screen root does not retain the exact order-24 K4 action",
        ));
    }
    Ok(())
}

fn try_new_screen_ledger_against_predecessor(
    inputs: &K6AlgebraInputs,
    representative: [i64; K6_ARITY],
    predecessor: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
) -> Result<CanonicalExactOwnerLedger, FoundryCampaignError> {
    let profile =
        K6CampaignResourceProfile::try_for_task_report_ceiling(1).map_err(selection_setup)?;
    try_new_k6_full_rank_ledger_with_profile_and_ordering(
        inputs,
        representative,
        predecessor,
        ordering,
        profile,
        ProbeCampaignLimits::default(),
    )
}

#[cfg(test)]
fn try_new_screen_ledger(
    inputs: &K6AlgebraInputs,
    representative: [i64; K6_ARITY],
    ordering: OrderingPolicy,
) -> Result<CanonicalExactOwnerLedger, FoundryCampaignError> {
    let predecessor = k6_root_predecessor_for_ordering(ordering)?;
    try_new_screen_ledger_against_predecessor(inputs, representative, predecessor, ordering)
}

fn try_built_in_probe_portfolio() -> Result<Vec<FoundryCampaignProbe>, FoundryCampaignError> {
    let specifications = [
        (998_244_353, 29),
        (998_244_353, 43),
        (1_000_000_007, 29),
        (1_000_000_007, 43),
        (1_000_000_009, 29),
        (1_000_000_009, 43),
    ];
    let mut probes = Vec::new();
    probes
        .try_reserve_exact(specifications.len())
        .map_err(|_| allocation("built-in modular probes", specifications.len()))?;
    for (modulus, dimension) in specifications {
        probes.push(
            FoundryCampaignProbe::try_new(modulus, [dimension], [0; K6_ARITY]).map_err(
                |error| FoundryCampaignError::Setup {
                    stage: FoundryCampaignSetupStage::AutonomousSelection,
                    message: error.to_string(),
                },
            )?,
        );
    }
    Ok(probes)
}

fn compare_candidates(left: &PriorityCandidate, right: &PriorityCandidate) -> Ordering {
    compare_coverage(
        &right.modular.worst_robust_coverage(),
        &left.modular.worst_robust_coverage(),
    )
    .then_with(|| right.modular.robust_hits().cmp(&left.modular.robust_hits()))
    .then_with(|| {
        right
            .modular
            .robust_prefix_min()
            .cmp(&left.modular.robust_prefix_min())
    })
    .then_with(|| {
        right
            .modular
            .robust_prefix_sum()
            .cmp(&left.modular.robust_prefix_sum())
    })
    .then_with(|| right.modular.total_hits.cmp(&left.modular.total_hits))
    .then_with(|| {
        left.modular
            .bounded_queries
            .cmp(&right.modular.bounded_queries)
    })
    .then_with(|| left.modular.total_fill.cmp(&right.modular.total_fill))
    .then_with(|| structural_quality_cmp(&right.worst_structural(), &left.worst_structural()))
    .then_with(|| structural_quality_cmp(&right.structural, &left.structural))
    .then_with(|| left.priority.cmp(&right.priority))
}

fn compare_candidates_for_sector(
    left: &PriorityCandidate,
    right: &PriorityCandidate,
    sector: usize,
) -> Ordering {
    match (
        left.sector_structural.get(sector),
        right.sector_structural.get(sector),
    ) {
        (Some(left_score), Some(right_score)) => structural_quality_cmp(right_score, left_score)
            .then_with(|| left.priority.cmp(&right.priority)),
        _ => left.priority.cmp(&right.priority),
    }
}

impl PriorityCandidate {
    fn worst_structural(&self) -> StructuralScore {
        self.sector_structural
            .iter()
            .copied()
            .min_by(structural_quality_cmp)
            .unwrap_or_default()
    }
}

/// Natural ordering of structural quality: `Greater` means that the left
/// score is stronger.  Leading rank is the sole minimization component.
fn structural_quality_cmp(left: &StructuralScore, right: &StructuralScore) -> Ordering {
    left.distinct_leading_targets
        .cmp(&right.distinct_leading_targets)
        .then_with(|| {
            left.rows_with_uncovered_leader
                .cmp(&right.rows_with_uncovered_leader)
        })
        .then_with(|| {
            left.leading_free_dimension_sum
                .cmp(&right.leading_free_dimension_sum)
        })
        .then_with(|| left.frontier_weight.cmp(&right.frontier_weight))
        .then_with(|| right.leading_rank_sum.cmp(&left.leading_rank_sum))
}

/// Compare exact coverage fractions without floating-point or a sector-size
/// bias. `Greater` means that the left fraction is larger.
fn compare_coverage(left: &(usize, usize), right: &(usize, usize)) -> Ordering {
    let left_scaled = (left.0 as u128) * (right.1 as u128);
    let right_scaled = (right.0 as u128) * (left.1 as u128);
    left_scaled.cmp(&right_scaled)
}

fn next_permutation(values: &mut [usize; K6_ARITY]) -> bool {
    let Some(pivot) = (1..values.len()).rfind(|&slot| values[slot - 1] < values[slot]) else {
        return false;
    };
    let pivot = pivot - 1;
    let successor = (pivot + 1..values.len())
        .rfind(|&slot| values[pivot] < values[slot])
        .expect("a lexicographic successor exists after the pivot");
    values.swap(pivot, successor);
    values[pivot + 1..].reverse();
    true
}

fn checked_product(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, FoundryCampaignError> {
    left.checked_mul(right).ok_or_else(|| overflow(resource))
}

fn retained_bytes(
    resource: &'static str,
    retained_cells: usize,
) -> Result<usize, FoundryCampaignError> {
    checked_product(resource, retained_cells, size_of::<usize>())
}

fn admit_retained_cells(
    cell_resource: &'static str,
    byte_resource: &'static str,
    retained_cells: usize,
    max_retained_cells: usize,
    max_retained_bytes: usize,
) -> Result<(), FoundryCampaignError> {
    admit(cell_resource, retained_cells, max_retained_cells)?;
    admit(
        byte_resource,
        retained_bytes(byte_resource, retained_cells)?,
        max_retained_bytes,
    )
}

fn try_filled_usize_vec(
    resource: &'static str,
    len: usize,
    value: usize,
) -> Result<Vec<usize>, FoundryCampaignError> {
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(len)
        .map_err(|_| allocation(resource, len))?;
    cells.resize(len, value);
    Ok(cells)
}

fn try_new_hit_ledgers(
    target_orders: &[Vec<usize>],
) -> Result<Vec<Vec<usize>>, FoundryCampaignError> {
    let mut ledgers = Vec::new();
    ledgers
        .try_reserve_exact(target_orders.len())
        .map_err(|_| allocation("modular sector hit ledgers", target_orders.len()))?;
    for targets in target_orders {
        ledgers.push(try_filled_usize_vec(
            "modular target hit-ledger cells",
            targets.len(),
            0,
        )?);
    }
    Ok(ledgers)
}

fn static_source_term_visits(
    authenticated_source_term_count: usize,
) -> Result<usize, FoundryCampaignError> {
    let exhaustive = checked_product(
        "static source-term visits",
        K6_PRIORITY_COUNT,
        authenticated_source_term_count,
    )?;
    let finalist_rescans = checked_product(
        "ordering-specific structural source-term visits",
        K6_STATIC_FINALIST_COUNT,
        authenticated_source_term_count,
    )?;
    exhaustive
        .checked_add(finalist_rescans)
        .ok_or_else(|| overflow("static source-term visits"))
}

fn admit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FoundryCampaignError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(FoundryCampaignError::ResourceLimit {
            stage: FoundryCampaignSetupStage::AutonomousSelection,
            resource,
            requested,
            limit,
        })
    }
}

fn selection_setup(error: impl std::fmt::Display) -> FoundryCampaignError {
    FoundryCampaignError::setup(FoundryCampaignSetupStage::AutonomousSelection, error)
}

fn allocation(resource: &'static str, requested: usize) -> FoundryCampaignError {
    FoundryCampaignError::Setup {
        stage: FoundryCampaignSetupStage::AutonomousSelection,
        message: format!("could not reserve {requested} {resource}"),
    }
}

fn overflow(resource: &'static str) -> FoundryCampaignError {
    FoundryCampaignError::ResourceCountOverflow {
        stage: FoundryCampaignSetupStage::AutonomousSelection,
        resource,
    }
}

fn invariant(detail: &'static str) -> FoundryCampaignError {
    FoundryCampaignError::Invariant { detail }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_is_deterministic_exhaustive_and_has_no_hint_ingress() {
        // Exercise two complete selections. The process-wide cache is
        // deliberately bypassed so this proves deterministic construction,
        // not merely deterministic cloning of one cached result.
        let first = try_select_uncached(AutonomousK6SelectorLimits::default()).unwrap();
        let second = try_select_uncached(AutonomousK6SelectorLimits::default()).unwrap();
        assert_eq!(first, second);
        let telemetry = first.telemetry();
        assert_eq!(telemetry.algorithm(), K6_SELECTION_ALGORITHM);
        assert_eq!(telemetry.coordinate_priority_count(), 720);
        assert!(telemetry.screened_sector_count() > 0);
        assert_eq!(
            telemetry.authenticated_source_row_count(),
            9 * telemetry.screened_sector_count()
        );
        assert_eq!(telemetry.symmetry_group_order(), 24);
        assert_eq!(telemetry.caller_hint_count(), 0);
        assert_eq!(telemetry.static_finalist_count(), 8);
        assert_eq!(
            telemetry.ordering_specific_screen_root_count(),
            8 * telemetry.screened_sector_count()
        );
        assert_eq!(
            telemetry.modular_physical_sample_count(),
            6 * telemetry.screened_sector_count()
        );
        assert_eq!(telemetry.rounds().len(), 3);
        assert_eq!(
            telemetry
                .rounds()
                .iter()
                .map(|round| (
                    round.input_candidate_count(),
                    round.new_probe_count(),
                    round.retained_candidate_count(),
                ))
                .collect::<Vec<_>>(),
            [(8, 1, 4), (4, 2, 2), (2, 3, 1)]
        );
        assert_eq!(first.probes().len(), telemetry.selected_probe_count());
        assert_eq!(first.priority(), telemetry.selected_priority());
        assert_eq!(
            telemetry.retained_target_order_byte_count(),
            telemetry.retained_target_order_cell_count() * size_of::<usize>()
        );
        assert_eq!(
            telemetry.retained_hit_ledger_byte_count(),
            telemetry.retained_hit_ledger_cell_count() * size_of::<usize>()
        );
        assert!(telemetry.retained_hit_ledger_cell_count() > 0);
        assert!(
            telemetry.retained_hit_ledger_cell_count()
                < telemetry.retained_target_order_cell_count()
        );
        assert!(telemetry.fresh_proof_root_required());
    }

    #[test]
    fn selected_proof_root_is_fresh_from_an_equal_order_screen_root() {
        let selected = try_select_autonomous_k6_search_program().unwrap();
        let inputs = shared_k6_algebra_inputs().unwrap();
        let representative = FULL_RANK_ORBITS[0].representative;
        let screen = try_new_screen_ledger(inputs, representative, selected.ordering()).unwrap();
        let screen_identity = screen.snapshot_identity();
        drop(screen);
        let proof = try_new_screen_ledger(inputs, representative, selected.ordering()).unwrap();
        assert!(!screen_identity.same_ledger_as(&proof.snapshot_identity()));
        assert_eq!(proof.revision().get(), 0);
        assert_eq!(proof.snapshot().owner_count(), 0);
    }

    #[test]
    fn logical_caps_reject_every_one_below_boundary() {
        assert_eq!(static_source_term_visits(17).unwrap(), (720 + 8) * 17);
        let too_few_priorities = AutonomousK6SelectorLimits {
            max_coordinate_priorities: 719,
            ..AutonomousK6SelectorLimits::default()
        };
        assert_resource_limit(
            too_few_priorities.try_admit_coordinate_priority_portfolio(),
            "coordinate priorities",
            720,
            719,
        );

        let too_few_rank_cells = AutonomousK6SelectorLimits {
            max_priority_rank_cells: 4_319,
            ..AutonomousK6SelectorLimits::default()
        };
        assert_resource_limit(
            too_few_rank_cells.try_admit_coordinate_priority_portfolio(),
            "coordinate-priority rank cells",
            4_320,
            4_319,
        );

        let finalists = AutonomousK6SelectorLimits {
            max_static_finalists: K6_STATIC_FINALIST_COUNT - 1,
            ..AutonomousK6SelectorLimits::default()
        };
        assert_resource_limit(
            finalists.try_admit_static_finalists(),
            "static finalists",
            K6_STATIC_FINALIST_COUNT,
            K6_STATIC_FINALIST_COUNT - 1,
        );

        let source_visits = static_source_term_visits(17).unwrap();
        let limits = AutonomousK6SelectorLimits {
            max_static_source_term_visits: source_visits - 1,
            ..AutonomousK6SelectorLimits::default()
        };
        assert_resource_limit(
            limits.try_admit_static_source_term_visits(source_visits),
            "static source-term visits",
            source_visits,
            source_visits - 1,
        );

        let roots = K6_STATIC_FINALIST_COUNT * 3;
        let limits = AutonomousK6SelectorLimits {
            max_ordering_specific_roots: roots - 1,
            ..AutonomousK6SelectorLimits::default()
        };
        assert_resource_limit(
            limits.try_admit_ordering_specific_roots(roots),
            "ordering-specific screen roots",
            roots,
            roots - 1,
        );

        let samples = 6 * 3;
        let limits = AutonomousK6SelectorLimits {
            max_modular_physical_samples: samples - 1,
            ..AutonomousK6SelectorLimits::default()
        };
        assert_resource_limit(
            limits.try_admit_modular_physical_samples(samples),
            "modular physical samples",
            samples,
            samples - 1,
        );

        let queries = 317;
        let limits = AutonomousK6SelectorLimits {
            max_modular_target_queries: queries - 1,
            ..AutonomousK6SelectorLimits::default()
        };
        assert_resource_limit(
            limits.try_admit_modular_target_queries(queries),
            "modular target queries",
            queries,
            queries - 1,
        );

        assert_retained_cap_boundaries(false, 123);
        assert_retained_cap_boundaries(true, 37);
    }

    fn assert_retained_cap_boundaries(hit_ledger: bool, cells: usize) {
        let bytes = cells * size_of::<usize>();
        let mut cell_limits = AutonomousK6SelectorLimits::default();
        let mut byte_limits = AutonomousK6SelectorLimits::default();
        let (cell_resource, byte_resource) = if hit_ledger {
            cell_limits.max_retained_hit_ledger_cells = cells - 1;
            byte_limits.max_retained_hit_ledger_cells = cells;
            byte_limits.max_retained_hit_ledger_bytes = bytes - 1;
            ("retained hit-ledger cells", "retained hit-ledger bytes")
        } else {
            cell_limits.max_retained_target_order_cells = cells - 1;
            byte_limits.max_retained_target_order_cells = cells;
            byte_limits.max_retained_target_order_bytes = bytes - 1;
            ("retained target-order cells", "retained target-order bytes")
        };
        let admit_cells = |limits: AutonomousK6SelectorLimits| {
            if hit_ledger {
                limits.try_admit_retained_hit_ledger_cells(cells)
            } else {
                limits.try_admit_retained_target_order_cells(cells)
            }
        };
        assert_resource_limit(admit_cells(cell_limits), cell_resource, cells, cells - 1);
        assert_resource_limit(admit_cells(byte_limits), byte_resource, bytes, bytes - 1);
    }

    fn assert_resource_limit(
        result: Result<(), FoundryCampaignError>,
        expected_resource: &'static str,
        expected_requested: usize,
        expected_limit: usize,
    ) {
        match result {
            Err(FoundryCampaignError::ResourceLimit {
                stage: FoundryCampaignSetupStage::AutonomousSelection,
                resource,
                requested,
                limit,
            }) => {
                assert_eq!(resource, expected_resource);
                assert_eq!(requested, expected_requested);
                assert_eq!(limit, expected_limit);
            }
            other => panic!("expected autonomous-selection resource limit, got {other:?}"),
        }
    }

    fn score(level: usize) -> StructuralScore {
        StructuralScore {
            distinct_leading_targets: level,
            rows_with_uncovered_leader: level,
            leading_free_dimension_sum: level,
            frontier_weight: level,
            leading_rank_sum: 0,
        }
    }

    fn synthetic_candidate(
        ranks: [usize; K6_ARITY],
        sector_structural: Vec<StructuralScore>,
        samples: usize,
        target_hit_counts: Vec<Vec<usize>>,
    ) -> PriorityCandidate {
        let priority =
            CoordinatePriority::try_new(K6_ARITY, &ranks, CoordinatePriorityLimits::default())
                .unwrap();
        let ordering = OrderingPolicy::try_with_coordinate_priority(&priority).unwrap();
        let structural = sector_structural.iter().copied().fold(
            StructuralScore::default(),
            |mut total, sector| {
                total.distinct_leading_targets += sector.distinct_leading_targets;
                total.rows_with_uncovered_leader += sector.rows_with_uncovered_leader;
                total.leading_free_dimension_sum += sector.leading_free_dimension_sum;
                total.frontier_weight += sector.frontier_weight;
                total.leading_rank_sum += sector.leading_rank_sum;
                total
            },
        );
        let target_orders = target_hit_counts
            .iter()
            .map(|sector| (0..sector.len()).collect())
            .collect();
        let total_hits = target_hit_counts.iter().flatten().sum();
        PriorityCandidate {
            priority,
            ordering,
            target_orders,
            sector_structural,
            structural,
            modular: ModularScore {
                samples,
                target_hit_counts,
                total_hits,
                bounded_queries: 0,
                total_fill: 0,
            },
        }
    }

    #[test]
    fn balanced_worst_sector_beats_larger_imbalanced_aggregate() {
        let imbalanced_static = synthetic_candidate(
            [0, 1, 2, 3, 4, 5],
            vec![score(10), score(0)],
            0,
            vec![vec![0; 6], vec![0; 6]],
        );
        let balanced_static = synthetic_candidate(
            [0, 1, 2, 3, 5, 4],
            vec![score(4), score(4)],
            0,
            vec![vec![0; 6], vec![0; 6]],
        );
        assert_eq!(
            compare_candidates(&balanced_static, &imbalanced_static),
            Ordering::Less
        );

        let imbalanced_modular = synthetic_candidate(
            [0, 1, 2, 3, 4, 5],
            vec![score(4), score(4)],
            1,
            vec![vec![1, 1, 1, 1, 1, 1], vec![0, 0, 0, 0, 0, 0]],
        );
        let balanced_modular = synthetic_candidate(
            [0, 1, 2, 3, 5, 4],
            vec![score(4), score(4)],
            1,
            vec![vec![1, 1, 0, 0, 0, 0], vec![1, 1, 0, 0, 0, 0]],
        );
        assert_eq!(
            compare_candidates(&balanced_modular, &imbalanced_modular),
            Ordering::Less
        );
    }

    #[test]
    fn sector_order_does_not_change_balanced_comparison() {
        let left = synthetic_candidate(
            [0, 1, 2, 3, 4, 5],
            vec![score(8), score(2), score(5)],
            2,
            vec![vec![2, 2, 0], vec![2, 0, 0], vec![2, 2, 2]],
        );
        let right = synthetic_candidate(
            [0, 1, 2, 3, 5, 4],
            vec![score(6), score(4), score(4)],
            2,
            vec![vec![2, 2, 0], vec![2, 2, 0], vec![2, 0, 0]],
        );
        let comparison = compare_candidates(&left, &right);

        let mut reversed_left = left.clone();
        reversed_left.sector_structural.reverse();
        reversed_left.modular.target_hit_counts.reverse();
        reversed_left.target_orders.reverse();
        let mut reversed_right = right.clone();
        reversed_right.sector_structural.reverse();
        reversed_right.modular.target_hit_counts.reverse();
        reversed_right.target_orders.reverse();
        assert_eq!(
            comparison,
            compare_candidates(&reversed_left, &reversed_right)
        );
    }

    #[test]
    fn static_retention_protects_each_sectors_best_candidate() {
        let sector_zero = synthetic_candidate(
            [0, 1, 2, 3, 4, 5],
            vec![score(100), score(0)],
            0,
            vec![vec![0], vec![0]],
        );
        let sector_one = synthetic_candidate(
            [0, 1, 2, 3, 5, 4],
            vec![score(0), score(100)],
            0,
            vec![vec![0], vec![0]],
        );
        let mut candidates = vec![sector_zero.clone(), sector_one.clone()];
        for ranks in [
            [0, 1, 2, 4, 3, 5],
            [0, 1, 2, 4, 5, 3],
            [0, 1, 2, 5, 3, 4],
            [0, 1, 2, 5, 4, 3],
            [0, 1, 3, 2, 4, 5],
            [0, 1, 3, 2, 5, 4],
            [0, 1, 3, 4, 2, 5],
        ] {
            candidates.push(synthetic_candidate(
                ranks,
                vec![score(10), score(10)],
                0,
                vec![vec![0], vec![0]],
            ));
        }
        let retained = retain_static_finalists(candidates, 2).unwrap();
        assert_eq!(retained.len(), K6_STATIC_FINALIST_COUNT);
        assert!(
            retained
                .iter()
                .any(|candidate| candidate.priority == sector_zero.priority)
        );
        assert!(
            retained
                .iter()
                .any(|candidate| candidate.priority == sector_one.priority)
        );
    }
}
