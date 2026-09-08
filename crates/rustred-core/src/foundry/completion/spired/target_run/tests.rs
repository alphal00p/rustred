use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ClosedArtifact, derive_one_loop_unit_mass_tadpole,
    derive_one_loop_unit_mass_tadpole_terminal_authority, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::guard::{
    ExactGuardPredicateCatalog, ExactGuardPredicateCatalogLimits, ExactGuardProbeError,
    ExactGuardProbeLimits, ExactGuardProbeWitness,
};
use crate::foundry::completion::source_discovery::{
    CampaignLimits, CampaignModularProbe, CanonicalExactOwnerLedger, ExactExecutableOwnerError,
    ExactExecutableOwnerLimits, ExactExecutableOwnerProposal, ExactOwnerCoverDeltaKind,
    ExactOwnerCoverDeltaLimits, ExactRuleCellPromotionDisposition, StagedSectorClosureLimits,
    try_compile_single_canonical_probe_executable_owner, try_publish_sealed_sector_wave,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, ImmutableOwnerSnapshot,
    StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use crate::foundry::completion::LatticeBox;

use super::super::{SpiredCase, SpiredCoordinateFace, SpiredExecutionCase, SpiredStreamingError};
use super::{
    SpiredTargetRunErrorCause, SpiredTargetRunLimits, SpiredTargetRunOutcome, SpiredTargetRunStage,
    try_run_spired_guarded_target, try_run_spired_target,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn one_loop_case_for_target(
    artifact: &ClosedArtifact,
    completed: &CompletedIbpSourceRows,
    target_power: i64,
) -> SpiredExecutionCase {
    let target = IntegralShift::try_new([target_power]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &structural_shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        owners,
    )
    .unwrap()
}

fn probe() -> CampaignModularProbe {
    CampaignModularProbe::try_new(PRIME, [37], [2], CampaignLimits::default()).unwrap()
}

fn one_loop_guard_polynomial(
    generator: &ParametricIbpGenerator<'_>,
    constant: i64,
) -> crate::algebra::IndexedPolynomial {
    let value = generator
        .context()
        .add(
            &generator.context().index(0).unwrap(),
            &generator.context().integer(constant),
        )
        .unwrap();
    generator
        .context()
        .numerator_condition_with_limits(&value, Default::default())
        .unwrap()
}

fn one_loop_guarded_case(
    artifact: &ClosedArtifact,
    completed: &CompletedIbpSourceRows,
    generator: &ParametricIbpGenerator<'_>,
    guard: &crate::algebra::IndexedPolynomial,
    branch: GuardBranch,
) -> SpiredExecutionCase {
    let target = IntegralShift::try_new([1]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &structural_shifts,
    )
    .unwrap();
    let registry_limits = StratumRegistryLimits::default();
    let identity = GuardBranchIdentity::try_from_indexed_polynomial(
        generator.context(),
        guard,
        branch,
        Default::default(),
        registry_limits,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_new(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        [identity],
        registry_limits,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        registry_limits,
    )
    .unwrap();
    SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        owners,
    )
    .unwrap()
}

fn one_loop_guard_witness(
    generator: &ParametricIbpGenerator<'_>,
    case: &SpiredExecutionCase,
    probe: &CampaignModularProbe,
    guard: crate::algebra::IndexedPolynomial,
) -> ExactGuardProbeWitness {
    let catalog = ExactGuardPredicateCatalog::try_from_pulled_back(
        generator.context(),
        [guard],
        ExactGuardPredicateCatalogLimits::default(),
    )
    .unwrap();
    let anchor = probe.try_index_anchor_for_stratum(case.stratum()).unwrap();
    ExactGuardProbeWitness::try_new(
        generator.context(),
        case.stratum(),
        probe.base_parameters(),
        &anchor,
        &catalog,
        ExactGuardProbeLimits::default(),
    )
    .unwrap()
}

fn sunset_case(artifact: &Arc<ClosedArtifact>, cell_ordinal: usize) -> SpiredExecutionCase {
    let cell = &artifact.rule_cells()[cell_ordinal];
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        cell.application_domain().clone(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        IntegralShift::try_new(cell.rule().pivot().values().iter().copied()).unwrap(),
        artifact.ordering(),
        owners,
    )
    .unwrap()
}

fn sunset_probe() -> CampaignModularProbe {
    CampaignModularProbe::try_new(PRIME, [37], [2, 2, 2], CampaignLimits::default()).unwrap()
}

#[test]
fn exact_guarded_probe_runs_through_streaming_and_reaches_a_real_k1_hit() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    // The raw active chart coordinate x=2 maps exactly to n=x+1=3.
    let guard = one_loop_guard_polynomial(&generator, -3);
    let case = one_loop_guarded_case(&artifact, &completed, &generator, &guard, GuardBranch::Zero);
    let probe = probe();
    let witness = one_loop_guard_witness(&generator, &case, &probe, guard);
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 1;

    let report =
        try_run_spired_guarded_target(&generator, &completed, &case, &probe, &witness, limits)
            .unwrap();
    assert_eq!(report.census().streamed_rows(), 1);
    assert_eq!(report.census().hit_depth(), Some(0));
    assert!(matches!(
        report.outcome(),
        SpiredTargetRunOutcome::RuleCell(_)
    ));
}

#[test]
fn guarded_target_run_rejects_a_stale_exact_probe_before_scheduling_rows() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let guard = one_loop_guard_polynomial(&generator, -3);
    let case = one_loop_guarded_case(&artifact, &completed, &generator, &guard, GuardBranch::Zero);
    let admitted_probe = probe();
    let witness = one_loop_guard_witness(&generator, &case, &admitted_probe, guard);
    let stale_probe =
        CampaignModularProbe::try_new(PRIME, [41], [2], CampaignLimits::default()).unwrap();

    let error = try_run_spired_guarded_target(
        &generator,
        &completed,
        &case,
        &stale_probe,
        &witness,
        SpiredTargetRunLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.stage(), SpiredTargetRunStage::Streaming);
    assert_eq!(error.census().streamed_rows(), 0);
    assert!(matches!(
        error.cause(),
        SpiredTargetRunErrorCause::Streaming(SpiredStreamingError::GuardProbe(
            ExactGuardProbeError::WitnessBasePointMismatch
        ))
    ));
}

#[test]
fn guarded_target_run_reports_an_unlucky_prime_as_typed_pre_stream_failure() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    // At n=3 this predicate evaluates exactly to PRIME.
    let guard = one_loop_guard_polynomial(&generator, i64::try_from(PRIME).unwrap() - 3);
    let case = one_loop_guarded_case(
        &artifact,
        &completed,
        &generator,
        &guard,
        GuardBranch::NonZero,
    );
    let probe = probe();
    let witness = one_loop_guard_witness(&generator, &case, &probe, guard);

    let error = try_run_spired_guarded_target(
        &generator,
        &completed,
        &case,
        &probe,
        &witness,
        SpiredTargetRunLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.stage(), SpiredTargetRunStage::Streaming);
    assert_eq!(error.census().shells_opened(), 0);
    assert_eq!(error.census().streamed_rows(), 0);
    assert!(matches!(
        error.cause(),
        SpiredTargetRunErrorCause::Streaming(SpiredStreamingError::GuardProbe(
            ExactGuardProbeError::UnluckyGuardPrime {
                guard_ordinal: 0,
                modulus: PRIME,
            }
        ))
    ));
}

#[test]
fn k1_depth_zero_schedule_drives_exact_rule_cell_admission() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let target = IntegralShift::try_new([1]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &structural_shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let case = SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        owners,
    )
    .unwrap();
    let probe = probe();
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 1;

    // No translated source request is supplied by this test: depth-zero
    // scheduling must derive source ordinal 0 at the zero offset itself.
    let report = try_run_spired_target(&generator, &completed, &case, &probe, limits).unwrap();
    let census = report.census();
    assert_eq!(census.shells_opened(), 1);
    assert_eq!(census.shells_completed(), 0);
    assert_eq!(census.chunks_materialized(), 1);
    assert_eq!(census.scheduled_requests(), 1);
    assert_eq!(census.scheduled_request_coordinate_cells(), 1);
    assert_eq!(census.streamed_rows(), 1);
    assert_eq!(census.hit_depth(), Some(0));
    assert_eq!(census.hit_chunk_ordinal(), Some(0));
    assert_eq!(census.hit_shell_request_ordinal(), Some(0));
    assert_eq!(census.hit_request_ordinal_in_chunk(), Some(0));
    assert_eq!(census.compact_support_requests(), 1);

    let SpiredTargetRunOutcome::RuleCell(ExactRuleCellPromotionDisposition::Admitted(candidate)) =
        report.into_outcome()
    else {
        panic!("the scheduler-derived K=1 source must admit one exact RuleCell")
    };
    assert_eq!(candidate.epoch().requests().len(), 1);
    let scheduled = &candidate.epoch().requests().requests()[0];
    assert_eq!(scheduled.source_ordinal(), 0);
    assert_eq!(scheduled.offset().values(), &[0]);
    assert!(candidate.circuit().is_bound_to(candidate.epoch().plan()));
    assert_eq!(
        candidate.circuit().replay().source_contributions(),
        candidate.circuit().source_combination().len()
    );
    assert!(
        candidate
            .cell()
            .terms()
            .iter()
            .all(|term| term.descent().verify())
    );
    assert_eq!(
        candidate
            .cell()
            .assignment_for_target(&IntegralKey::try_new([3]).unwrap())
            .unwrap(),
        Some(vec![2])
    );

    let mut owner_limits = ExactExecutableOwnerLimits::default();
    owner_limits.max_candidates_per_owner = 0;
    assert!(matches!(
        try_compile_single_canonical_probe_executable_owner(
            generator.context(),
            candidate,
            owner_limits,
        ),
        Err(ExactExecutableOwnerError::ResourceLimit {
            resource: "semantic executable owner candidates",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn k1_target_run_candidate_compiles_and_publishes_a_bounded_wave() {
    let authority = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    assert_eq!(authority.declared_master_manifest().terminals().len(), 1);
    assert!(
        authority
            .declared_master_manifest()
            .terminals()
            .contains(&IntegralKey::try_new([1]).unwrap())
    );
    assert_eq!(authority.zero_sectors().len(), 1);
    assert_eq!(authority.zero_sectors()[0].sector().active_bits(), [false]);
    assert!(authority.dependencies().is_empty());
    assert!(authority.factorization_rules().is_empty());
    let generator = ParametricIbpGenerator::try_new(authority.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let target = IntegralShift::try_new([1]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &structural_shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        authority.family_fingerprint(),
        authority.context_fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    // This independently installed predecessor authenticates only I(1) and
    // the scaleless zero sector. It contains no recurrence that could act as
    // a closing-rule oracle for the target run below.
    let predecessor = ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(&authority),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let case = SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        predecessor.clone(),
    )
    .unwrap();
    let mut run_limits = SpiredTargetRunLimits::default();
    run_limits.max_depth_inclusive = 0;
    run_limits.request_chunk_size = 1;
    let report =
        try_run_spired_target(&generator, &completed, &case, &probe(), run_limits).unwrap();
    let SpiredTargetRunOutcome::RuleCell(ExactRuleCellPromotionDisposition::Admitted(admitted)) =
        report.into_outcome()
    else {
        panic!("the depth-zero K=1 target run must admit an exact RuleCell")
    };

    let retained_epoch = admitted.epoch().clone();
    let retained_circuit = admitted.circuit().clone();
    let retained_cell = Arc::as_ptr(admitted.cell_owner());
    let ExactExecutableOwnerProposal::Compiled {
        owner,
        obstructions,
    } = try_compile_single_canonical_probe_executable_owner(
        generator.context(),
        admitted,
        Default::default(),
    )
    .unwrap()
    else {
        panic!("one admitted K=1 candidate must compile to an executable owner")
    };
    assert!(obstructions.is_empty());
    assert_eq!(owner.executable_candidates().len(), 1);
    assert!(Arc::ptr_eq(owner.epoch(), &retained_epoch));
    assert!(Arc::ptr_eq(
        owner.semantic().candidates()[0].circuit(),
        &retained_circuit,
    ));
    assert!(Arc::ptr_eq(
        owner.executable_candidates()[0].circuit(),
        &retained_circuit,
    ));
    assert_eq!(
        Arc::as_ptr(owner.executable_candidates()[0].cell_owner()),
        retained_cell,
    );

    let sector = Mask::try_new([true]).unwrap();
    // This is deliberately a bounded wave-integration test, not the later
    // full-carrier K1 artifact-closure claim. Coordinate zero is declared
    // explicitly as the evaluation-policy terminal I(1); it is not inferred
    // from bounded search exhaustion.
    let carrier = LatticeBox::try_new([0], [Some(11)]).unwrap();
    let expected_carrier = LatticeBox::try_new([0], [Some(11)]).unwrap();
    let mut ledger = CanonicalExactOwnerLedger::try_new_with_closure_carrier(
        generator.context(),
        predecessor.clone(),
        sector,
        OrderingPolicy::default(),
        [IntegralKey::try_new([1]).unwrap()],
        carrier,
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let delta = ledger.try_apply_owner(owner).unwrap();
    assert_eq!(
        delta.kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert!(!delta.baseline().status().is_compiler_closed());
    assert!(delta.updated().status().is_compiler_closed());
    let sealed = ledger.try_into_closed_cover().unwrap();
    assert_eq!(
        sealed.executable_cover().proof_cover().closure_carrier(),
        &expected_carrier,
    );
    let wave = try_publish_sealed_sector_wave(
        predecessor.clone(),
        vec![sealed],
        StagedSectorClosureLimits::default(),
    )
    .unwrap();
    assert_eq!(wave.layers().len(), 1);
    assert!(wave.predecessor().same_authority_as(&predecessor));
    assert_eq!(wave.layers()[0].proven_domain().bounds()[0].lower(), 1);
    assert_eq!(wave.layers()[0].proven_domain().bounds()[0].upper(), 12);
    assert_eq!(wave.successor().closed_layer_count(), 1);
}

#[test]
fn early_hit_is_independent_of_future_chunk_preparation() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 4);
    let case = sunset_case(&artifact, 0);
    let consumed_source_term_count = completed.relations()[..3]
        .iter()
        .map(|source| source.terms().len())
        .sum();

    let run = |request_chunk_size| {
        let mut limits = SpiredTargetRunLimits::default();
        limits.max_depth_inclusive = 0;
        limits.request_chunk_size = request_chunk_size;
        // The first three ordinary rows expose this sunset target. The fourth
        // row in a wide depth-zero chunk is a future tail and must not be
        // structurally inspected before the third request hits.
        limits.streaming.max_preparation_request_inspections = 3;
        limits.streaming.max_preparation_source_term_inspections = consumed_source_term_count;
        try_run_spired_target(&generator, &completed, &case, &sunset_probe(), limits).unwrap()
    };

    let narrow = run(1);
    let wide = run(64);
    for report in [&narrow, &wide] {
        let census = report.census();
        assert_eq!(census.shells_opened(), 1);
        assert_eq!(census.shells_completed(), 0);
        assert_eq!(census.streamed_rows(), 3);
        assert_eq!(census.hit_depth(), Some(0));
        assert_eq!(census.hit_shell_request_ordinal(), Some(2));
        assert!(matches!(
            report.outcome(),
            SpiredTargetRunOutcome::RuleCell(_)
        ));
    }
    assert_eq!(narrow.census().chunks_materialized(), 3);
    assert_eq!(narrow.census().hit_chunk_ordinal(), Some(2));
    assert_eq!(narrow.census().hit_request_ordinal_in_chunk(), Some(0));
    assert_eq!(narrow.census().scheduled_requests(), 3);
    assert_eq!(narrow.census().scheduled_request_coordinate_cells(), 9);
    assert_eq!(wide.census().chunks_materialized(), 1);
    assert_eq!(wide.census().hit_chunk_ordinal(), Some(0));
    assert_eq!(wide.census().hit_request_ordinal_in_chunk(), Some(2));
    assert_eq!(wide.census().scheduled_requests(), 4);
    assert_eq!(wide.census().scheduled_request_coordinate_cells(), 12);
}

#[test]
fn residual_aggregate_budget_materializes_an_admissible_early_hit_prefix() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 64;
    limits.max_total_scheduled_requests = 3;
    limits.max_total_request_coordinate_cells = 9;

    let report =
        try_run_spired_target(&generator, &completed, &case, &sunset_probe(), limits).unwrap();
    let census = report.census();
    assert_eq!(census.chunks_materialized(), 1);
    assert_eq!(census.scheduled_requests(), 3);
    assert_eq!(census.scheduled_request_coordinate_cells(), 9);
    assert_eq!(census.streamed_rows(), 3);
    assert_eq!(census.hit_depth(), Some(0));
    assert_eq!(census.hit_shell_request_ordinal(), Some(2));
    assert_eq!(census.hit_request_ordinal_in_chunk(), Some(2));
    assert!(matches!(
        report.outcome(),
        SpiredTargetRunOutcome::RuleCell(_)
    ));
}

#[test]
fn exhausted_residual_request_budget_reports_the_consumed_prefix_exactly() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case_for_target(&artifact, &completed, 0);
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 1;
    limits.request_chunk_size = 64;
    limits.max_total_scheduled_requests = 1;
    limits.max_total_request_coordinate_cells = 1;

    let error = try_run_spired_target(&generator, &completed, &case, &probe(), limits).unwrap_err();
    assert_eq!(error.stage(), SpiredTargetRunStage::Scheduling);
    assert!(matches!(
        error.cause(),
        SpiredTargetRunErrorCause::ResourceLimit {
            resource: "target-run scheduled requests",
            requested: 2,
            limit: 1,
        }
    ));
    let census = error.census();
    assert_eq!(census.shells_opened(), 2);
    assert_eq!(census.shells_completed(), 1);
    assert_eq!(census.chunks_materialized(), 1);
    assert_eq!(census.scheduled_requests(), 1);
    assert_eq!(census.scheduled_request_coordinate_cells(), 1);
    assert_eq!(census.streamed_rows(), 1);
    assert_eq!(census.current_depth(), Some(1));
    assert_eq!(census.hit_depth(), None);
}

#[test]
fn exact_aggregate_boundary_allows_a_complete_depth_exhaustion() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case_for_target(&artifact, &completed, 0);
    let source_term_count = completed.relations()[0].terms().len();
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 64;
    limits.max_total_scheduled_requests = 1;
    limits.max_total_request_coordinate_cells = 1;
    limits.streaming.max_preparation_request_inspections = 1;
    limits.streaming.max_preparation_source_term_inspections = source_term_count;

    let report = try_run_spired_target(&generator, &completed, &case, &probe(), limits).unwrap();
    assert!(matches!(
        report.outcome(),
        SpiredTargetRunOutcome::SearchDepthExhausted
    ));
    let census = report.census();
    assert_eq!(census.shells_opened(), 1);
    assert_eq!(census.shells_completed(), 1);
    assert_eq!(census.chunks_materialized(), 1);
    assert_eq!(census.scheduled_requests(), 1);
    assert_eq!(census.scheduled_request_coordinate_cells(), 1);
    assert_eq!(census.streamed_rows(), 1);
    assert_eq!(census.current_depth(), Some(0));
    assert_eq!(census.hit_depth(), None);
}
