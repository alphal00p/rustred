use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::canonical_three_loop_family;
use crate::foundry::completion::frame::exact::{
    ExactCircuitLift, ExactCircuitLimits, try_lift_exact_circuit,
};
use crate::foundry::completion::frame::modular::{
    ModularKernelError, ModularKernelLimits, ModularTargetQuery,
};
use crate::foundry::completion::frame::{PhysicalFrameLimits, PhysicalFramePlan};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryError, StratumRegistryLimits,
    TargetColumnPartition,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::schedule::{
    retained_diagnostic_forbidden_entries, retained_modular_obstruction_entries, select_proposal,
};
use super::{
    CanonicalTraceIdentity, DiscoveryTraceGroup, EvidenceProbeOutcome, EvidenceProbePlan,
    EvidenceProbeRole, EvidenceProbeSpec, ExactProposalOutcome, HeldOutAssessment,
    ProbeRejectionStage, TargetEvidenceError, TargetEvidenceLimits, TargetEvidenceScheduler,
};

const P0: u64 = 1_000_000_007;
const P1: u64 = 1_000_000_009;
const P2: u64 = 998_244_353;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn tadpole(name: &str, massive: bool) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            if massive {
                context.integer(-1)
            } else {
                context.zero()
            },
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn tadpole_frame(
    name: &str,
    massive: bool,
    degree: usize,
) -> (IndexedCoefficientContext, PhysicalFramePlan) {
    tadpole_frame_in_sector(name, massive, degree, true)
}

fn tadpole_frame_in_sector(
    name: &str,
    massive: bool,
    degree: usize,
    active: bool,
) -> (IndexedCoefficientContext, PhysicalFramePlan) {
    let family = tadpole(name, massive);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let plan = PhysicalFramePlan::try_new(
        &generator,
        &completed,
        Mask::try_new([active]).unwrap(),
        degree,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    (context, plan)
}

fn guarded_tadpole_frame() -> (IndexedCoefficientContext, PhysicalFramePlan) {
    let base = CoefficientContext::new(["d", "x"]);
    let reciprocal = base
        .try_div(
            &base.one(),
            &base.parameter("x").unwrap(),
            Default::default(),
        )
        .unwrap();
    let family = IntegralFamily::new(
        "evidence-guarded-tadpole",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![reciprocal],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let plan = PhysicalFramePlan::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    (context, plan)
}

fn s4a_degree_one() -> (IndexedCoefficientContext, PhysicalFramePlan) {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let plan = PhysicalFramePlan::try_new(
        &generator,
        &completed,
        Mask::try_new([false, true, true, true, true, false]).unwrap(),
        1,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    (context, plan)
}

fn column(plan: &PhysicalFramePlan, shift: &[i64]) -> usize {
    plan.columns()
        .iter()
        .position(|candidate| candidate.values() == shift)
        .unwrap()
}

fn target_partition<'frame>(
    plan: &'frame PhysicalFramePlan,
    target: usize,
) -> TargetColumnPartition<'frame> {
    try_target_partition(plan, target).unwrap()
}

fn try_target_partition<'frame>(
    plan: &'frame PhysicalFramePlan,
    target: usize,
) -> Result<TargetColumnPartition<'frame>, StratumRegistryError> {
    let all_shifts = plan
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        plan.sector().clone(),
        plan.columns()[target].values(),
        &all_shifts,
    )
    .unwrap();
    let limits = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_guard_blind(
        plan.family_fingerprint(),
        plan.context_fingerprint(),
        domain,
        limits,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        plan.family_fingerprint(),
        plan.context_fingerprint(),
        plan.sector().arity(),
        limits,
    )
    .unwrap();
    TargetColumnPartition::try_new(
        plan,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        limits,
    )
}

fn run_tadpole_report<'context, 'frame, 'partition>(
    context: &'context IndexedCoefficientContext,
    plan: &'frame PhysicalFramePlan,
    partition: &'partition TargetColumnPartition<'frame>,
) -> super::TargetEvidenceReport<'context, 'frame> {
    let probes = [
        // n = x + 1 = 0 in F_p, so the degree-zero target coefficient
        // vanishes and this remains a genuine modular NoHit.
        EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P0, &[37], &[P0 - 1]),
        EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P0, &[37], &[2]),
        // The same raw point at another prime is intentionally admissible.
        EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P1, &[37], &[2]),
        EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P2, &[37], &[2]),
    ];
    let plan =
        EvidenceProbePlan::try_new(context, plan, probes, TargetEvidenceLimits::default()).unwrap();
    TargetEvidenceScheduler::try_new(
        plan,
        partition,
        ModularKernelLimits::default(),
        ExactCircuitLimits::default(),
    )
    .unwrap()
    .run()
    .unwrap()
}

#[test]
fn ordered_tadpole_evidence_preserves_no_hit_groups_and_exact_replay() {
    let (context, plan) = tadpole_frame("evidence-ordered-tadpole", true, 0);
    let target = column(&plan, &[1]);
    let partition = target_partition(&plan, target);

    let first = run_tadpole_report(&context, &plan, &partition);
    let second = run_tadpole_report(&context, &plan, &partition);
    assert_eq!(first, second);
    assert!(std::ptr::eq(first.probe_plan().frame(), &plan));
    assert_eq!(first.outcomes().len(), 4);
    assert!(matches!(
        first.outcomes()[0],
        EvidenceProbeOutcome::ModularNoHit { .. }
    ));
    assert!(matches!(
        first.outcomes()[1],
        EvidenceProbeOutcome::Hit { .. }
    ));
    assert!(matches!(
        first.outcomes()[2],
        EvidenceProbeOutcome::Hit { .. }
    ));
    assert!(matches!(
        first.outcomes()[3],
        EvidenceProbeOutcome::Hit { .. }
    ));
    assert_eq!(first.discovery_groups().len(), 1);
    assert_eq!(first.discovery_groups()[0].probe_ordinals(), &[1, 2]);
    let trace = first.discovery_groups()[0].trace();
    assert_eq!(trace.target_column(), target);
    assert_eq!(trace.forbidden_columns(), partition.forbidden_columns());
    assert_eq!(trace.forbidden_rank(), trace.forbidden_pivot_shifts().len());
    assert_eq!(trace.augmented_rank(), trace.augmented_pivot_shifts().len());
    assert_eq!(
        trace.forbidden_rank(),
        trace.forbidden_source_instances().len()
    );
    assert_eq!(
        trace.augmented_rank(),
        trace.augmented_source_instances().len()
    );
    assert!(first.outcomes()[1].sample_fingerprint().is_some());
    assert!(first.outcomes()[1].diagnostics().is_some());
    assert_eq!(first.exact_proposal().probe_ordinal(), Some(1));
    let Some(Ok(ExactCircuitLift::Replayed(circuit))) = first.exact_proposal().result() else {
        panic!("the selected tadpole proposal must replay exactly")
    };
    assert_eq!(circuit.target_column(), target);
    assert_eq!(first.held_out_diagnostics().len(), 1);
    assert_eq!(first.held_out_diagnostics()[0].probe_ordinal(), 3);
    assert_eq!(
        first.held_out_diagnostics()[0].assessment(),
        HeldOutAssessment::TraceMatch
    );
    assert!(first.held_out_trace_stable());
}

#[test]
fn query_failure_is_retained_as_the_fourth_distinct_probe_outcome() {
    let (context, plan) = tadpole_frame("evidence-query-rejection", true, 0);
    let target = column(&plan, &[1]);
    let partition = target_partition(&plan, target);
    let probe_plan = EvidenceProbePlan::try_new(
        &context,
        &plan,
        [EvidenceProbeSpec::new(
            EvidenceProbeRole::Discovery,
            P0,
            &[37],
            &[2],
        )],
        TargetEvidenceLimits::default(),
    )
    .unwrap();
    let mut modular_limits = ModularKernelLimits::default();
    modular_limits.max_projected_columns = 0;
    let report = TargetEvidenceScheduler::try_new(
        probe_plan,
        &partition,
        modular_limits,
        ExactCircuitLimits::default(),
    )
    .unwrap()
    .run()
    .unwrap();
    assert_eq!(
        report.outcomes()[0].rejection_stage(),
        Some(ProbeRejectionStage::Query)
    );
    assert!(matches!(
        report.outcomes()[0].rejection(),
        Some(ModularKernelError::ResourceLimit {
            resource: "modular projected columns",
            requested: 1,
            limit: 0,
        })
    ));
    assert!(report.outcomes()[0].sample_fingerprint().is_some());
    assert!(report.outcomes()[0].diagnostics().is_none());
    assert!(report.discovery_groups().is_empty());
    assert_eq!(
        report.exact_proposal(),
        &ExactProposalOutcome::NoDiscoveryHit
    );
}

fn synthetic_trace(target_column: usize) -> std::sync::Arc<CanonicalTraceIdentity> {
    std::sync::Arc::new(CanonicalTraceIdentity::from_parts(
        target_column,
        std::sync::Arc::from([]),
        0,
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))
}

#[test]
fn proposal_selection_prefers_largest_group_then_earliest_tie() {
    let first_trace = synthetic_trace(0);
    let second_trace = synthetic_trace(1);
    let largest = DiscoveryTraceGroup::new(first_trace.clone(), vec![7, 8, 9]);
    let earlier_tie = DiscoveryTraceGroup::new(first_trace, vec![2, 5]);
    let later_tie = DiscoveryTraceGroup::new(second_trace, vec![3, 4]);

    assert_eq!(
        select_proposal(&[later_tie.clone(), earlier_tie.clone()]),
        Some(2)
    );
    assert_eq!(select_proposal(&[earlier_tie, largest, later_tie]), Some(7));
}

#[test]
fn held_out_trace_mismatch_never_invalidates_exact_replay() {
    let (context, plan) = tadpole_frame("evidence-held-out-mismatch", true, 1);
    let target = column(&plan, &[1]);
    let partition = target_partition(&plan, target);
    let probes = [
        EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P0, &[37], &[2]),
        // n = -1 in F_p, so the harder-column pivot vanishes while a
        // different target Hit remains. This is telemetry, not an override of
        // the already replayed generic exact circuit.
        EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P1, &[37], &[P1 - 2]),
    ];
    let probe_plan =
        EvidenceProbePlan::try_new(&context, &plan, probes, TargetEvidenceLimits::default())
            .unwrap();
    let report = TargetEvidenceScheduler::try_new(
        probe_plan,
        &partition,
        ModularKernelLimits::default(),
        ExactCircuitLimits::default(),
    )
    .unwrap()
    .run()
    .unwrap();
    let Some(Ok(ExactCircuitLift::Replayed(circuit))) = report.exact_proposal().result() else {
        panic!("Discovery proposal must remain an exact replay")
    };
    assert_eq!(circuit.target_column(), target);
    assert!(matches!(
        report.outcomes()[1],
        EvidenceProbeOutcome::Hit { .. }
    ));
    assert_eq!(
        report.held_out_diagnostics()[0].assessment(),
        HeldOutAssessment::TraceMismatch
    );
    assert!(!report.held_out_trace_stable());
    assert!(matches!(
        report.exact_proposal().result(),
        Some(Ok(ExactCircuitLift::Replayed(_)))
    ));
}

#[test]
fn rejected_discovery_and_held_out_only_hit_remain_inconclusive() {
    let (context, plan) = guarded_tadpole_frame();
    let target = column(&plan, &[1]);
    let partition = target_partition(&plan, target);
    let probes = [
        EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P0, &[4, 0], &[0]),
        EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P0, &[4, 1], &[0]),
    ];
    let probe_plan =
        EvidenceProbePlan::try_new(&context, &plan, probes, TargetEvidenceLimits::default())
            .unwrap();
    let report = TargetEvidenceScheduler::try_new(
        probe_plan,
        &partition,
        ModularKernelLimits::default(),
        ExactCircuitLimits::default(),
    )
    .unwrap()
    .run()
    .unwrap();
    assert!(matches!(
        report.outcomes()[0],
        EvidenceProbeOutcome::RejectedSample {
            error: ModularKernelError::SourceConditionZero { .. }
        }
    ));
    assert!(matches!(
        report.outcomes()[1],
        EvidenceProbeOutcome::Hit { .. }
    ));
    assert!(report.discovery_groups().is_empty());
    assert_eq!(
        report.exact_proposal(),
        &ExactProposalOutcome::NoDiscoveryHit
    );
    assert_eq!(
        report.held_out_diagnostics()[0].assessment(),
        HeldOutAssessment::NoSelectedDiscoveryTrace
    );
    assert!(!report.held_out_trace_stable());
}

#[test]
fn probe_plan_rejects_bad_or_duplicate_tasks_before_execution() {
    let (context, plan) = tadpole_frame("evidence-plan-preflight", true, 0);
    let limits = TargetEvidenceLimits::default();
    let valid = EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P0, &[37], &[2]);

    assert_eq!(
        EvidenceProbePlan::try_new(
            &context,
            &plan,
            [EvidenceProbeSpec::new(
                EvidenceProbeRole::Discovery,
                10,
                &[37],
                &[2]
            )],
            limits,
        )
        .unwrap_err(),
        TargetEvidenceError::UnsupportedEvenModulus {
            probe_ordinal: 0,
            modulus: 10,
        }
    );
    assert_eq!(
        EvidenceProbePlan::try_new(
            &context,
            &plan,
            [EvidenceProbeSpec::new(
                EvidenceProbeRole::Discovery,
                9,
                &[37],
                &[2]
            )],
            limits,
        )
        .unwrap_err(),
        TargetEvidenceError::NonPrimeModulus {
            probe_ordinal: 0,
            modulus: 9,
        }
    );
    assert_eq!(
        EvidenceProbePlan::try_new(
            &context,
            &plan,
            [EvidenceProbeSpec::new(
                EvidenceProbeRole::Discovery,
                P0,
                &[],
                &[2]
            )],
            limits,
        )
        .unwrap_err(),
        TargetEvidenceError::WrongBaseParameterArity {
            probe_ordinal: 0,
            expected: 1,
            actual: 0,
        }
    );
    assert_eq!(
        EvidenceProbePlan::try_new(
            &context,
            &plan,
            [EvidenceProbeSpec::new(
                EvidenceProbeRole::Discovery,
                P0,
                &[37],
                &[]
            )],
            limits,
        )
        .unwrap_err(),
        TargetEvidenceError::WrongChartCoordinateArity {
            probe_ordinal: 0,
            expected: 1,
            actual: 0,
        }
    );
    assert_eq!(
        EvidenceProbePlan::try_new(
            &context,
            &plan,
            [
                valid,
                EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P0, &[37], &[2]),
            ],
            limits,
        )
        .unwrap_err(),
        TargetEvidenceError::DuplicateProbeTask {
            first_ordinal: 0,
            duplicate_ordinal: 1,
        }
    );
    assert_eq!(
        EvidenceProbePlan::try_new(
            &context,
            &plan,
            [EvidenceProbeSpec::new(
                EvidenceProbeRole::HeldOut,
                P0,
                &[37],
                &[2]
            )],
            limits,
        )
        .unwrap_err(),
        TargetEvidenceError::MissingDiscoveryProbe
    );

    let mut probe_limits = limits;
    probe_limits.max_probes = 1;
    assert_eq!(
        EvidenceProbePlan::try_new(&context, &plan, std::iter::repeat(valid), probe_limits,)
            .unwrap_err(),
        TargetEvidenceError::ResourceLimit {
            resource: "target-evidence probes",
            requested: 2,
            limit: 1,
        }
    );
    let mut coordinate_limits = limits;
    coordinate_limits.max_chart_coordinate_cells = 0;
    assert_eq!(
        EvidenceProbePlan::try_new(&context, &plan, [valid], coordinate_limits).unwrap_err(),
        TargetEvidenceError::ResourceLimit {
            resource: "target-evidence chart-coordinate cells",
            requested: 1,
            limit: 0,
        }
    );

    // Repeating a raw point across primes and using multiple points at one
    // prime are both legal; only the full task key is unique.
    let admitted = EvidenceProbePlan::try_new(
        &context,
        &plan,
        [
            valid,
            EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P1, &[37], &[2]),
            EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P0, &[37], &[3]),
        ],
        limits,
    )
    .unwrap();
    assert_eq!(admitted.probes().len(), 3);

    let p0_i64 = i64::try_from(P0).unwrap();
    for alias in [
        EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P0, &[37 + p0_i64], &[2 + P0]),
        EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P0, &[-1], &[2]),
    ] {
        let original_base = if alias.base_parameters() == [-1] {
            [p0_i64 - 1]
        } else {
            [37]
        };
        assert_eq!(
            EvidenceProbePlan::try_new(
                &context,
                &plan,
                [
                    EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P0, &original_base, &[2],),
                    alias,
                ],
                limits,
            )
            .unwrap_err(),
            TargetEvidenceError::DuplicateProbeTask {
                first_ordinal: 0,
                duplicate_ordinal: 1,
            }
        );
    }

    let (inactive_context, inactive_plan) =
        tadpole_frame_in_sector("evidence-inactive-residue-alias", true, 0, false);
    assert_eq!(
        EvidenceProbePlan::try_new(
            &inactive_context,
            &inactive_plan,
            [
                EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P0, &[37], &[2]),
                EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P0, &[37], &[2 + P0],),
            ],
            limits,
        )
        .unwrap_err(),
        TargetEvidenceError::DuplicateProbeTask {
            first_ordinal: 0,
            duplicate_ordinal: 1,
        }
    );
}

#[test]
fn scheduler_preflights_every_retained_forbidden_and_obstruction_copy() {
    let (context, plan) = tadpole_frame("evidence-forbidden-copy-limit", true, 1);
    let target = column(&plan, &[1]);
    let partition = target_partition(&plan, target);
    assert_eq!(partition.forbidden_columns().len(), 1);
    let probes = || {
        [
            EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P0, &[37], &[2]),
            EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P1, &[37], &[2]),
        ]
    };

    let mut rejected_limits = TargetEvidenceLimits::default();
    rejected_limits.max_retained_diagnostic_forbidden_column_entries = 2;
    let rejected = EvidenceProbePlan::try_new(&context, &plan, probes(), rejected_limits).unwrap();
    assert_eq!(
        TargetEvidenceScheduler::try_new(
            rejected,
            &partition,
            ModularKernelLimits::default(),
            ExactCircuitLimits::default(),
        )
        .unwrap_err(),
        TargetEvidenceError::ResourceLimit {
            resource: "target-evidence retained diagnostic forbidden-column entries",
            requested: 3,
            limit: 2,
        }
    );

    let mut exact_limits = TargetEvidenceLimits::default();
    exact_limits.max_retained_diagnostic_forbidden_column_entries = 3;
    let exact = EvidenceProbePlan::try_new(&context, &plan, probes(), exact_limits).unwrap();
    TargetEvidenceScheduler::try_new(
        exact,
        &partition,
        ModularKernelLimits::default(),
        ExactCircuitLimits::default(),
    )
    .unwrap();

    let mut obstruction_rejected_limits = TargetEvidenceLimits::default();
    // Two probes can each retain two logical columns and at most two sparse
    // obstruction entries: eight aggregate sidecar entries.
    obstruction_rejected_limits.max_retained_modular_obstruction_entries = 7;
    let obstruction_rejected =
        EvidenceProbePlan::try_new(&context, &plan, probes(), obstruction_rejected_limits).unwrap();
    assert_eq!(
        TargetEvidenceScheduler::try_new(
            obstruction_rejected,
            &partition,
            ModularKernelLimits::default(),
            ExactCircuitLimits::default(),
        )
        .unwrap_err(),
        TargetEvidenceError::ResourceLimit {
            resource: "target-evidence retained modular-obstruction entries",
            requested: 8,
            limit: 7,
        }
    );

    let mut obstruction_exact_limits = TargetEvidenceLimits::default();
    obstruction_exact_limits.max_retained_modular_obstruction_entries = 8;
    let obstruction_exact =
        EvidenceProbePlan::try_new(&context, &plan, probes(), obstruction_exact_limits).unwrap();
    TargetEvidenceScheduler::try_new(
        obstruction_exact,
        &partition,
        ModularKernelLimits::default(),
        ExactCircuitLimits::default(),
    )
    .unwrap();

    assert_eq!(
        retained_diagnostic_forbidden_entries(usize::MAX, 1).unwrap_err(),
        TargetEvidenceError::ResourceCountOverflow {
            resource: "target-evidence retained diagnostic forbidden-column entries",
        }
    );
    assert_eq!(
        retained_diagnostic_forbidden_entries(usize::MAX - 1, 2).unwrap_err(),
        TargetEvidenceError::ResourceCountOverflow {
            resource: "target-evidence retained diagnostic forbidden-column entries",
        }
    );
    assert_eq!(
        retained_modular_obstruction_entries(1, usize::MAX).unwrap_err(),
        TargetEvidenceError::ResourceCountOverflow {
            resource: "target-evidence retained modular-obstruction entries",
        }
    );
    assert_eq!(
        retained_modular_obstruction_entries(usize::MAX, 1).unwrap_err(),
        TargetEvidenceError::ResourceCountOverflow {
            resource: "target-evidence retained modular-obstruction entries",
        }
    );
}

#[test]
fn aggregate_scheduler_trace_limits_fail_closed() {
    let (context, plan) = tadpole_frame("evidence-trace-limit", true, 0);
    let target = column(&plan, &[1]);
    let partition = target_partition(&plan, target);
    let mut limits = TargetEvidenceLimits::default();
    limits.max_diagnostic_source_entries = 0;
    let probe_plan = EvidenceProbePlan::try_new(
        &context,
        &plan,
        [EvidenceProbeSpec::new(
            EvidenceProbeRole::Discovery,
            P0,
            &[37],
            &[2],
        )],
        limits,
    )
    .unwrap();
    assert_eq!(
        TargetEvidenceScheduler::try_new(
            probe_plan,
            &partition,
            ModularKernelLimits::default(),
            ExactCircuitLimits::default(),
        )
        .unwrap()
        .run()
        .unwrap_err(),
        TargetEvidenceError::ResourceLimit {
            resource: "target-evidence diagnostic source entries",
            requested: 1,
            limit: 0,
        }
    );
}

#[test]
fn selected_exact_result_diagnostic_clone_is_charged() {
    let (context, plan) = tadpole_frame("evidence-selected-clone-limit", true, 0);
    let target = column(&plan, &[1]);
    let partition = target_partition(&plan, target);
    let build = |limits| {
        EvidenceProbePlan::try_new(
            &context,
            &plan,
            [EvidenceProbeSpec::new(
                EvidenceProbeRole::Discovery,
                P0,
                &[37],
                &[2],
            )],
            limits,
        )
        .unwrap()
    };

    let mut source_limits = TargetEvidenceLimits::default();
    source_limits.max_diagnostic_source_entries = 1;
    assert_eq!(
        TargetEvidenceScheduler::try_new(
            build(source_limits),
            &partition,
            ModularKernelLimits::default(),
            ExactCircuitLimits::default(),
        )
        .unwrap()
        .run()
        .unwrap_err(),
        TargetEvidenceError::ResourceLimit {
            resource: "target-evidence diagnostic source entries",
            requested: 2,
            limit: 1,
        }
    );

    let mut pivot_limits = TargetEvidenceLimits::default();
    pivot_limits.max_diagnostic_pivot_entries = 1;
    assert_eq!(
        TargetEvidenceScheduler::try_new(
            build(pivot_limits),
            &partition,
            ModularKernelLimits::default(),
            ExactCircuitLimits::default(),
        )
        .unwrap()
        .run()
        .unwrap_err(),
        TargetEvidenceError::ResourceLimit {
            resource: "target-evidence diagnostic pivot entries",
            requested: 2,
            limit: 1,
        }
    );
}

fn first_exact_k6_target(context: &IndexedCoefficientContext, plan: &PhysicalFramePlan) -> usize {
    let sampled = plan
        .try_modular_sample(
            context,
            P0,
            &[37],
            &[1, 2, 3, 4, 5, 6],
            ModularKernelLimits::default(),
        )
        .unwrap();
    for target in 0..plan.columns().len() {
        let Ok(partition) = try_target_partition(plan, target) else {
            continue;
        };
        if partition.forbidden_columns().is_empty() || partition.allowed_columns().is_empty() {
            continue;
        }
        let ModularTargetQuery::Hit(hit) = sampled
            .query_target(
                target,
                partition.forbidden_columns(),
                ModularKernelLimits::default(),
            )
            .unwrap()
        else {
            continue;
        };
        if matches!(
            try_lift_exact_circuit(context, &hit, &partition, ExactCircuitLimits::default(),)
                .unwrap(),
            ExactCircuitLift::Replayed(_)
        ) {
            return target;
        }
    }
    panic!("canonical K6 S4a fixture has no exact nonempty target")
}

#[test]
fn canonical_k6_s4a_multi_prime_proposal_replays_exactly() {
    let (context, plan) = s4a_degree_one();
    let target = first_exact_k6_target(&context, &plan);
    let partition = target_partition(&plan, target);
    assert!(!partition.forbidden_columns().is_empty());
    assert!(!partition.allowed_columns().is_empty());
    let probes = [
        EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P0, &[37], &[1, 2, 3, 4, 5, 6]),
        EvidenceProbeSpec::new(EvidenceProbeRole::Discovery, P1, &[37], &[1, 2, 3, 4, 5, 6]),
        EvidenceProbeSpec::new(EvidenceProbeRole::HeldOut, P2, &[37], &[1, 2, 3, 4, 5, 6]),
    ];
    let probe_plan =
        EvidenceProbePlan::try_new(&context, &plan, probes, TargetEvidenceLimits::default())
            .unwrap();
    let report = TargetEvidenceScheduler::try_new(
        probe_plan,
        &partition,
        ModularKernelLimits::default(),
        ExactCircuitLimits::default(),
    )
    .unwrap()
    .run()
    .unwrap();
    let Some(Ok(ExactCircuitLift::Replayed(circuit))) = report.exact_proposal().result() else {
        panic!("the chosen K6 S4a modular proposal must replay exactly")
    };
    assert_eq!(circuit.target_column(), target);
    assert_eq!(circuit.stratum_id(), partition.stratum_id());
    assert!(
        circuit
            .residual_terms()
            .iter()
            .all(|term| term.descent().verify())
    );
    assert_eq!(report.held_out_diagnostics().len(), 1);
}
