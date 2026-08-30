use std::cmp::Ordering;
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::exact::{ExactCircuitLimits, try_lift_exact_circuit};
use crate::foundry::completion::frame::modular::{
    ModularKernelLimits, ModularRankDiagnostics, ModularTargetQuery,
};
use crate::foundry::completion::stratum::TargetColumnPartition;
use crate::identity::IntegralShift;

use super::super::{PhysicalFramePlan, SourceInstanceId};
use super::{
    CanonicalTraceIdentity, DiscoveryTraceGroup, EvidenceProbe, EvidenceProbeOutcome,
    EvidenceProbePlan, EvidenceProbeRole, EvidenceProbeSpec, ExactProposalOutcome,
    HeldOutAssessment, HeldOutDiagnostic, TargetEvidenceError, TargetEvidenceLimits,
    TargetEvidenceReport,
};

const PROBES: &str = "target-evidence probes";
const DISCOVERY_PROBES: &str = "target-evidence Discovery probes";
const HELD_OUT_PROBES: &str = "target-evidence HeldOut probes";
const BASE_PARAMETER_CELLS: &str = "target-evidence base-parameter cells";
const CHART_COORDINATE_CELLS: &str = "target-evidence chart-coordinate cells";
const PROBE_KEY_ORDER: &str = "target-evidence probe-key order";
const OUTCOMES: &str = "target-evidence probe outcomes";
const DIAGNOSTIC_SOURCES: &str = "target-evidence diagnostic source entries";
const DIAGNOSTIC_PIVOTS: &str = "target-evidence diagnostic pivot entries";
const RETAINED_DIAGNOSTIC_FORBIDDEN_COLUMNS: &str =
    "target-evidence retained diagnostic forbidden-column entries";
const TRACE_SCOPE: &str = "target-evidence canonical trace scope entries";
const CANONICAL_SOURCES: &str = "target-evidence canonical source entries";
const CANONICAL_PIVOTS: &str = "target-evidence canonical pivot entries";
const GROUPS: &str = "target-evidence discovery trace groups";
const GROUP_MEMBERS: &str = "target-evidence discovery group members";
const HELD_OUT_DIAGNOSTICS: &str = "target-evidence HeldOut diagnostics";

/// Serial executor for one frame-bound target partition and one predeclared
/// probe order. No scheduling decision depends on field values or worker
/// completion order.
#[derive(Debug)]
pub(crate) struct TargetEvidenceScheduler<'context, 'partition, 'frame> {
    probe_plan: EvidenceProbePlan<'context, 'frame>,
    partition: &'partition TargetColumnPartition<'frame>,
    modular_limits: ModularKernelLimits,
    exact_limits: ExactCircuitLimits,
}

impl<'context, 'partition, 'frame> TargetEvidenceScheduler<'context, 'partition, 'frame> {
    pub(crate) fn try_new(
        probe_plan: EvidenceProbePlan<'context, 'frame>,
        partition: &'partition TargetColumnPartition<'frame>,
        modular_limits: ModularKernelLimits,
        exact_limits: ExactCircuitLimits,
    ) -> Result<Self, TargetEvidenceError> {
        if !std::ptr::eq(probe_plan.frame(), partition.frame()) {
            return Err(TargetEvidenceError::ForeignFramePartition);
        }
        match partition.try_verify() {
            Ok(true) => {}
            Ok(false) => {
                return Err(TargetEvidenceError::Invariant {
                    detail: "target partition failed cold verification",
                });
            }
            Err(error) => return Err(TargetEvidenceError::PartitionVerification(error)),
        }
        let retained_forbidden_entries = retained_diagnostic_forbidden_entries(
            probe_plan.probes().len(),
            partition.forbidden_columns().len(),
        )?;
        check_limit(
            RETAINED_DIAGNOSTIC_FORBIDDEN_COLUMNS,
            retained_forbidden_entries,
            probe_plan
                .limits()
                .max_retained_diagnostic_forbidden_column_entries,
        )?;
        Ok(Self {
            probe_plan,
            partition,
            modular_limits,
            exact_limits,
        })
    }

    pub(crate) fn run(self) -> Result<TargetEvidenceReport<'context, 'frame>, TargetEvidenceError> {
        let limits = self.probe_plan.limits();
        let mut outcomes = try_vec(OUTCOMES, self.probe_plan.probes().len())?;
        let mut budget = TraceBudget::default();
        let forbidden_scope = copy_slice(
            self.partition.forbidden_columns(),
            TRACE_SCOPE,
            limits.max_trace_scope_entries,
        )?;
        let forbidden_scope: Arc<[usize]> = Arc::from(forbidden_scope);

        // Serial declared order is part of this API's deterministic contract.
        for probe in self.probe_plan.probes() {
            let sampled = match self.probe_plan.frame().try_modular_sample(
                self.probe_plan.context(),
                probe.modulus(),
                probe.base_parameters(),
                probe.chart_coordinates(),
                self.modular_limits,
            ) {
                Ok(sampled) => sampled,
                Err(error) => {
                    outcomes.push(EvidenceProbeOutcome::RejectedSample { error });
                    continue;
                }
            };
            let sample = sampled.sample_fingerprint().clone();
            let query = match sampled.query_target(
                self.partition.target_column(),
                self.partition.forbidden_columns(),
                self.modular_limits,
            ) {
                Ok(query) => query,
                Err(error) => {
                    outcomes.push(EvidenceProbeOutcome::RejectedQuery { sample, error });
                    continue;
                }
            };
            budget.admit_diagnostics(query.diagnostics(), limits)?;
            match query {
                ModularTargetQuery::ModularNoHit(no_hit) => {
                    outcomes.push(EvidenceProbeOutcome::ModularNoHit { sample, no_hit });
                }
                ModularTargetQuery::Hit(hit) => {
                    let trace = canonical_trace(
                        self.probe_plan.frame(),
                        self.partition,
                        forbidden_scope.clone(),
                        hit.diagnostics(),
                        &mut budget,
                        limits,
                    )?;
                    outcomes.push(EvidenceProbeOutcome::Hit {
                        hit,
                        trace: Arc::new(trace),
                    });
                }
            }
        }
        if outcomes.len() != self.probe_plan.probes().len() {
            return Err(TargetEvidenceError::Invariant {
                detail: "probe execution did not retain exactly one outcome per task",
            });
        }

        let discovery_groups = group_discovery_hits(self.probe_plan.probes(), &outcomes, limits)?;
        let selected = select_proposal(&discovery_groups);
        let exact_proposal = if let Some(probe_ordinal) = selected {
            let outcome = outcomes
                .get(probe_ordinal)
                .ok_or(TargetEvidenceError::Invariant {
                    detail: "selected Discovery ordinal is outside outcomes",
                })?;
            let hit = outcome.hit().ok_or(TargetEvidenceError::Invariant {
                detail: "selected Discovery proposal is not a modular Hit",
            })?;
            // Either successful exact-lift variant owns a second copy of the
            // selected hit's diagnostics. Charge its source/pivot sidecars
            // before exact reduction; the forbidden-column sidecar was
            // already admitted conservatively at scheduler construction.
            budget.admit_diagnostics(hit.diagnostics(), limits)?;
            let trace = outcome
                .trace()
                .ok_or(TargetEvidenceError::Invariant {
                    detail: "selected Discovery proposal has no canonical trace",
                })?
                .clone();
            ExactProposalOutcome::Checked {
                probe_ordinal,
                trace,
                result: try_lift_exact_circuit(
                    self.probe_plan.context(),
                    hit,
                    self.partition,
                    self.exact_limits,
                ),
            }
        } else {
            ExactProposalOutcome::NoDiscoveryHit
        };

        let held_out = classify_held_out(
            self.probe_plan.probes(),
            &outcomes,
            exact_proposal.trace(),
            limits,
        )?;
        let held_out_trace_stable = !held_out.is_empty()
            && exact_proposal.trace().is_some()
            && held_out
                .iter()
                .all(|diagnostic| diagnostic.assessment() == HeldOutAssessment::TraceMatch);

        Ok(TargetEvidenceReport::from_parts(
            self.probe_plan,
            outcomes,
            discovery_groups,
            exact_proposal,
            held_out,
            held_out_trace_stable,
        ))
    }
}

pub(super) fn admit_probe_plan<'context, 'frame, 'point>(
    context: &'context IndexedCoefficientContext,
    frame: &'frame PhysicalFramePlan,
    probes: impl IntoIterator<Item = EvidenceProbeSpec<'point>>,
    limits: TargetEvidenceLimits,
) -> Result<EvidenceProbePlan<'context, 'frame>, TargetEvidenceError> {
    if context.fingerprint() != frame.context_fingerprint() {
        return Err(TargetEvidenceError::WrongFrameContext);
    }
    if context.index_count() != frame.sector().arity() {
        return Err(TargetEvidenceError::WrongContextIndexArity {
            expected: frame.sector().arity(),
            actual: context.index_count(),
        });
    }
    let expected_base = context.base().parameter_names().len();
    let expected_chart = frame.sector().arity();
    let mut retained = Vec::new();
    let mut discovery_count = 0usize;
    let mut held_out_count = 0usize;
    let mut base_cells = 0usize;
    let mut chart_cells = 0usize;

    for spec in probes {
        let probe_ordinal = retained.len();
        let requested = checked_add(PROBES, probe_ordinal, 1)?;
        check_limit(PROBES, requested, limits.max_probes)?;
        validate_modulus(probe_ordinal, spec.modulus())?;
        if spec.base_parameters().len() != expected_base {
            return Err(TargetEvidenceError::WrongBaseParameterArity {
                probe_ordinal,
                expected: expected_base,
                actual: spec.base_parameters().len(),
            });
        }
        if spec.chart_coordinates().len() != expected_chart {
            return Err(TargetEvidenceError::WrongChartCoordinateArity {
                probe_ordinal,
                expected: expected_chart,
                actual: spec.chart_coordinates().len(),
            });
        }
        match spec.role() {
            EvidenceProbeRole::Discovery => {
                discovery_count = checked_add(DISCOVERY_PROBES, discovery_count, 1)?;
                check_limit(
                    DISCOVERY_PROBES,
                    discovery_count,
                    limits.max_discovery_probes,
                )?;
            }
            EvidenceProbeRole::HeldOut => {
                held_out_count = checked_add(HELD_OUT_PROBES, held_out_count, 1)?;
                check_limit(HELD_OUT_PROBES, held_out_count, limits.max_held_out_probes)?;
            }
        }
        base_cells = checked_add(
            BASE_PARAMETER_CELLS,
            base_cells,
            spec.base_parameters().len(),
        )?;
        check_limit(
            BASE_PARAMETER_CELLS,
            base_cells,
            limits.max_base_parameter_cells,
        )?;
        chart_cells = checked_add(
            CHART_COORDINATE_CELLS,
            chart_cells,
            spec.chart_coordinates().len(),
        )?;
        check_limit(
            CHART_COORDINATE_CELLS,
            chart_cells,
            limits.max_chart_coordinate_cells,
        )?;
        let base_parameters = copy_slice(
            spec.base_parameters(),
            BASE_PARAMETER_CELLS,
            limits.max_base_parameter_cells,
        )?;
        let chart_coordinates = copy_slice(
            spec.chart_coordinates(),
            CHART_COORDINATE_CELLS,
            limits.max_chart_coordinate_cells,
        )?;
        try_reserve(&mut retained, 1, PROBES)?;
        retained.push(EvidenceProbe::from_parts(
            spec.role(),
            spec.modulus(),
            base_parameters,
            chart_coordinates,
        ));
    }
    if discovery_count == 0 {
        return Err(TargetEvidenceError::MissingDiscoveryProbe);
    }
    reject_duplicate_tasks(&retained, frame.sector().active_bits())?;
    Ok(EvidenceProbePlan {
        context,
        frame,
        probes: retained.into_boxed_slice(),
        limits,
    })
}

fn validate_modulus(probe_ordinal: usize, modulus: u64) -> Result<(), TargetEvidenceError> {
    if modulus.is_multiple_of(2) {
        return Err(TargetEvidenceError::UnsupportedEvenModulus {
            probe_ordinal,
            modulus,
        });
    }
    if modulus == u64::MAX || !Integer::from(modulus).is_prime(0) {
        return Err(TargetEvidenceError::NonPrimeModulus {
            probe_ordinal,
            modulus,
        });
    }
    Ok(())
}

fn reject_duplicate_tasks(
    probes: &[EvidenceProbe],
    active_bits: &[bool],
) -> Result<(), TargetEvidenceError> {
    let mut order = try_vec(PROBE_KEY_ORDER, probes.len())?;
    order.extend(0..probes.len());
    order.sort_unstable_by(|&left, &right| {
        compare_task_key(&probes[left], &probes[right], active_bits).then_with(|| left.cmp(&right))
    });
    for pair in order.windows(2) {
        if compare_task_key(&probes[pair[0]], &probes[pair[1]], active_bits) == Ordering::Equal {
            return Err(TargetEvidenceError::DuplicateProbeTask {
                first_ordinal: pair[0],
                duplicate_ordinal: pair[1],
            });
        }
    }
    Ok(())
}

fn compare_task_key(left: &EvidenceProbe, right: &EvidenceProbe, active_bits: &[bool]) -> Ordering {
    let modulus_order = left.modulus().cmp(&right.modulus());
    if modulus_order != Ordering::Equal {
        return modulus_order;
    }
    let modulus = left.modulus();
    compare_base_residues(left.base_parameters(), right.base_parameters(), modulus).then_with(
        || {
            compare_chart_index_residues(
                left.chart_coordinates(),
                right.chart_coordinates(),
                active_bits,
                modulus,
            )
        },
    )
}

fn compare_base_residues(left: &[i64], right: &[i64], modulus: u64) -> Ordering {
    for (&left, &right) in left.iter().zip(right) {
        let order = signed_residue(left, modulus).cmp(&signed_residue(right, modulus));
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_chart_index_residues(
    left: &[u64],
    right: &[u64],
    active_bits: &[bool],
    modulus: u64,
) -> Ordering {
    for ((&left, &right), &active) in left.iter().zip(right).zip(active_bits) {
        let order = chart_index_residue(left, active, modulus)
            .cmp(&chart_index_residue(right, active, modulus));
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

fn signed_residue(value: i64, modulus: u64) -> u64 {
    if value >= 0 {
        value.unsigned_abs() % modulus
    } else {
        let magnitude = value.unsigned_abs() % modulus;
        if magnitude == 0 {
            0
        } else {
            modulus - magnitude
        }
    }
}

fn chart_index_residue(coordinate: u64, active: bool, modulus: u64) -> u64 {
    let coordinate = coordinate % modulus;
    if active {
        if coordinate == modulus - 1 {
            0
        } else {
            coordinate + 1
        }
    } else if coordinate == 0 {
        0
    } else {
        modulus - coordinate
    }
}

#[derive(Default)]
struct TraceBudget {
    diagnostic_sources: usize,
    diagnostic_pivots: usize,
    canonical_sources: usize,
    canonical_pivots: usize,
}

impl TraceBudget {
    fn admit_diagnostics(
        &mut self,
        diagnostics: &ModularRankDiagnostics,
        limits: TargetEvidenceLimits,
    ) -> Result<(), TargetEvidenceError> {
        let sources = checked_add(
            DIAGNOSTIC_SOURCES,
            diagnostics.forbidden_independent_source_rows.len(),
            diagnostics.augmented_independent_source_rows.len(),
        )?;
        self.diagnostic_sources =
            checked_add(DIAGNOSTIC_SOURCES, self.diagnostic_sources, sources)?;
        check_limit(
            DIAGNOSTIC_SOURCES,
            self.diagnostic_sources,
            limits.max_diagnostic_source_entries,
        )?;
        let pivots = checked_add(
            DIAGNOSTIC_PIVOTS,
            diagnostics.forbidden_pivot_columns.len(),
            diagnostics.augmented_pivot_columns.len(),
        )?;
        self.diagnostic_pivots = checked_add(DIAGNOSTIC_PIVOTS, self.diagnostic_pivots, pivots)?;
        check_limit(
            DIAGNOSTIC_PIVOTS,
            self.diagnostic_pivots,
            limits.max_diagnostic_pivot_entries,
        )
    }

    fn admit_canonical(
        &mut self,
        diagnostics: &ModularRankDiagnostics,
        limits: TargetEvidenceLimits,
    ) -> Result<(), TargetEvidenceError> {
        let sources = checked_add(
            CANONICAL_SOURCES,
            diagnostics.forbidden_independent_source_rows.len(),
            diagnostics.augmented_independent_source_rows.len(),
        )?;
        self.canonical_sources = checked_add(CANONICAL_SOURCES, self.canonical_sources, sources)?;
        check_limit(
            CANONICAL_SOURCES,
            self.canonical_sources,
            limits.max_canonical_source_entries,
        )?;
        let pivots = checked_add(
            CANONICAL_PIVOTS,
            diagnostics.forbidden_pivot_columns.len(),
            diagnostics.augmented_pivot_columns.len(),
        )?;
        self.canonical_pivots = checked_add(CANONICAL_PIVOTS, self.canonical_pivots, pivots)?;
        check_limit(
            CANONICAL_PIVOTS,
            self.canonical_pivots,
            limits.max_canonical_pivot_entries,
        )
    }
}

fn canonical_trace(
    frame: &PhysicalFramePlan,
    partition: &TargetColumnPartition<'_>,
    forbidden_scope: Arc<[usize]>,
    diagnostics: &ModularRankDiagnostics,
    budget: &mut TraceBudget,
    limits: TargetEvidenceLimits,
) -> Result<CanonicalTraceIdentity, TargetEvidenceError> {
    if diagnostics.target_column != partition.target_column()
        || diagnostics.forbidden_columns.as_ref() != partition.forbidden_columns()
        || forbidden_scope.as_ref() != partition.forbidden_columns()
        || diagnostics.forbidden_rank != diagnostics.forbidden_pivot_columns.len()
        || diagnostics.forbidden_rank != diagnostics.forbidden_independent_source_rows.len()
        || diagnostics.augmented_rank != diagnostics.augmented_pivot_columns.len()
        || diagnostics.augmented_rank != diagnostics.augmented_independent_source_rows.len()
    {
        return Err(TargetEvidenceError::Invariant {
            detail: "modular diagnostics do not define a complete target-local trace",
        });
    }
    budget.admit_canonical(diagnostics, limits)?;
    Ok(CanonicalTraceIdentity::from_parts(
        partition.target_column(),
        forbidden_scope,
        diagnostics.forbidden_rank,
        diagnostics.augmented_rank,
        map_pivots(frame, &diagnostics.forbidden_pivot_columns)?,
        map_pivots(frame, &diagnostics.augmented_pivot_columns)?,
        map_sources(frame, &diagnostics.forbidden_independent_source_rows)?,
        map_sources(frame, &diagnostics.augmented_independent_source_rows)?,
    ))
}

fn map_pivots(
    frame: &PhysicalFramePlan,
    columns: &[usize],
) -> Result<Vec<IntegralShift>, TargetEvidenceError> {
    if columns.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(TargetEvidenceError::Invariant {
            detail: "modular pivot columns are not strictly increasing",
        });
    }
    let mut mapped = try_vec(CANONICAL_PIVOTS, columns.len())?;
    for &column in columns {
        mapped.push(
            frame
                .columns()
                .get(column)
                .ok_or(TargetEvidenceError::Invariant {
                    detail: "modular pivot column is outside its physical frame",
                })?
                .clone(),
        );
    }
    Ok(mapped)
}

fn map_sources(
    frame: &PhysicalFramePlan,
    rows: &[usize],
) -> Result<Vec<SourceInstanceId>, TargetEvidenceError> {
    if rows.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(TargetEvidenceError::Invariant {
            detail: "modular independent-source rows are not strictly increasing",
        });
    }
    let mut mapped = try_vec(CANONICAL_SOURCES, rows.len())?;
    for &row in rows {
        mapped.push(
            frame
                .source_instances()
                .get(row)
                .ok_or(TargetEvidenceError::Invariant {
                    detail: "modular independent-source row is outside its physical frame",
                })?
                .clone(),
        );
    }
    Ok(mapped)
}

fn group_discovery_hits<'frame>(
    probes: &[EvidenceProbe],
    outcomes: &[EvidenceProbeOutcome<'frame>],
    limits: TargetEvidenceLimits,
) -> Result<Vec<DiscoveryTraceGroup>, TargetEvidenceError> {
    let mut hit_ordinals = try_vec(GROUP_MEMBERS, probes.len().min(limits.max_group_members))?;
    for (probe_ordinal, (probe, outcome)) in probes.iter().zip(outcomes).enumerate() {
        if probe.role() == EvidenceProbeRole::Discovery && outcome.trace().is_some() {
            let requested = checked_add(GROUP_MEMBERS, hit_ordinals.len(), 1)?;
            check_limit(GROUP_MEMBERS, requested, limits.max_group_members)?;
            hit_ordinals.push(probe_ordinal);
        }
    }
    hit_ordinals.sort_unstable_by(|&left, &right| {
        outcomes[left]
            .trace()
            .expect("Discovery Hit ordinal was filtered above")
            .cmp(
                outcomes[right]
                    .trace()
                    .expect("Discovery Hit ordinal was filtered above"),
            )
            .then_with(|| left.cmp(&right))
    });

    let mut groups = try_vec(GROUPS, hit_ordinals.len().min(limits.max_trace_groups))?;
    let mut start = 0usize;
    while start < hit_ordinals.len() {
        let trace = outcomes[hit_ordinals[start]]
            .trace()
            .ok_or(TargetEvidenceError::Invariant {
                detail: "grouped Discovery outcome lost its canonical trace",
            })?
            .clone();
        let mut end = start + 1;
        while end < hit_ordinals.len()
            && outcomes[hit_ordinals[end]]
                .trace()
                .is_some_and(|candidate| candidate == &trace)
        {
            end += 1;
        }
        let requested = checked_add(GROUPS, groups.len(), 1)?;
        check_limit(GROUPS, requested, limits.max_trace_groups)?;
        let mut members = try_vec(GROUP_MEMBERS, end - start)?;
        members.extend_from_slice(&hit_ordinals[start..end]);
        groups.push(DiscoveryTraceGroup::new(trace, members));
        start = end;
    }
    Ok(groups)
}

pub(super) fn select_proposal(groups: &[DiscoveryTraceGroup]) -> Option<usize> {
    let mut selected: Option<&DiscoveryTraceGroup> = None;
    for group in groups {
        let replace = match selected {
            None => true,
            Some(current) => {
                group.probe_ordinals().len() > current.probe_ordinals().len()
                    || (group.probe_ordinals().len() == current.probe_ordinals().len()
                        && (group.probe_ordinals()[0] < current.probe_ordinals()[0]
                            || (group.probe_ordinals()[0] == current.probe_ordinals()[0]
                                && group.trace() < current.trace())))
            }
        };
        if replace {
            selected = Some(group);
        }
    }
    selected.map(|group| group.probe_ordinals()[0])
}

fn classify_held_out<'frame>(
    probes: &[EvidenceProbe],
    outcomes: &[EvidenceProbeOutcome<'frame>],
    selected_trace: Option<&Arc<CanonicalTraceIdentity>>,
    limits: TargetEvidenceLimits,
) -> Result<Vec<HeldOutDiagnostic>, TargetEvidenceError> {
    let held_out_count = probes
        .iter()
        .filter(|probe| probe.role() == EvidenceProbeRole::HeldOut)
        .count();
    let mut diagnostics = try_vec(
        HELD_OUT_DIAGNOSTICS,
        held_out_count.min(limits.max_held_out_probes),
    )?;
    for (probe_ordinal, (probe, outcome)) in probes.iter().zip(outcomes).enumerate() {
        if probe.role() != EvidenceProbeRole::HeldOut {
            continue;
        }
        let assessment = match outcome {
            EvidenceProbeOutcome::RejectedSample { .. } => HeldOutAssessment::RejectedSample,
            EvidenceProbeOutcome::RejectedQuery { .. } => HeldOutAssessment::RejectedQuery,
            EvidenceProbeOutcome::ModularNoHit { .. } => HeldOutAssessment::ModularNoHit,
            EvidenceProbeOutcome::Hit { trace, .. } => match selected_trace {
                None => HeldOutAssessment::NoSelectedDiscoveryTrace,
                Some(selected) if selected == trace => HeldOutAssessment::TraceMatch,
                Some(_) => HeldOutAssessment::TraceMismatch,
            },
        };
        diagnostics.push(HeldOutDiagnostic::new(probe_ordinal, assessment));
    }
    Ok(diagnostics)
}

fn copy_slice<T: Copy>(
    values: &[T],
    resource: &'static str,
    limit: usize,
) -> Result<Vec<T>, TargetEvidenceError> {
    check_limit(resource, values.len(), limit)?;
    let mut retained = try_vec(resource, values.len())?;
    retained.extend_from_slice(values);
    Ok(retained)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TargetEvidenceError> {
    left.checked_add(right)
        .ok_or(TargetEvidenceError::ResourceCountOverflow { resource })
}

pub(super) fn retained_diagnostic_forbidden_entries(
    probe_count: usize,
    forbidden_column_count: usize,
) -> Result<usize, TargetEvidenceError> {
    let retained_copies = checked_add(RETAINED_DIAGNOSTIC_FORBIDDEN_COLUMNS, probe_count, 1)?;
    checked_mul(
        RETAINED_DIAGNOSTIC_FORBIDDEN_COLUMNS,
        retained_copies,
        forbidden_column_count,
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TargetEvidenceError> {
    left.checked_mul(right)
        .ok_or(TargetEvidenceError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TargetEvidenceError> {
    if requested > limit {
        Err(TargetEvidenceError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), TargetEvidenceError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| TargetEvidenceError::AllocationFailure {
            resource,
            requested,
        })
}

fn try_vec<T>(resource: &'static str, capacity: usize) -> Result<Vec<T>, TargetEvidenceError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| TargetEvidenceError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
