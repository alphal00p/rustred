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

use super::super::{
    SpiredCase, SpiredCompactLift, SpiredCoordinateFace, SpiredExecutionCase,
    SpiredModularStreamOutcome, SpiredPreparedStreamingDiscovery, SpiredStreamingError,
    SpiredStructuralPreparation, try_lift_spired_compact_support,
    try_lift_spired_rooted_compact_support,
};
use super::run::{BranchCandidateAttempt, BranchCandidateProposal, try_attempt_branch_candidate};
use super::{
    SpiredTargetRunErrorCause, SpiredTargetRunLimits, SpiredTargetRunOutcome, SpiredTargetRunStage,
    SpiredTargetRunWorkspace, try_run_spired_guarded_target, try_run_spired_target,
};

const PRIME: u64 = 1_000_000_007;
const SECOND_PRIME: u64 = 1_000_000_021;

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
    sunset_probe_with_modulus(PRIME)
}

fn sunset_probe_with_modulus(modulus: u64) -> CampaignModularProbe {
    CampaignModularProbe::try_new(modulus, [37], [2, 2, 2], CampaignLimits::default()).unwrap()
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
    let mut limits = SpiredTargetRunLimits::default();
    // A one-shot invalid probe must be rejected before the otherwise failing
    // exact-source workspace is constructed.
    limits.streaming.evaluation.max_source_rows = 0;

    let error = try_run_spired_guarded_target(
        &generator,
        &completed,
        &case,
        &stale_probe,
        &witness,
        limits,
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
    let mut limits = SpiredTargetRunLimits::default();
    // The unlucky modulus is a cheap probe failure and must win over this
    // deliberately impossible structural-preparation budget.
    limits.streaming.evaluation.max_source_rows = 0;

    let error =
        try_run_spired_guarded_target(&generator, &completed, &case, &probe, &witness, limits)
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
    limits.max_post_hit_streamed_rows = 0;

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
fn canonical_k1_target_is_discovered_from_the_depth_one_minus_one_translation() {
    let authority = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(authority.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let target = IntegralShift::try_new([0]).unwrap();
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
    let owners = ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(&authority),
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
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 1;
    limits.request_chunk_size = 1;

    let report = try_run_spired_target(&generator, &completed, &case, &probe(), limits).unwrap();
    let census = report.census();
    assert_eq!(census.hit_depth(), Some(1));
    assert_eq!(census.hit_shell_request_ordinal(), Some(0));
    let SpiredTargetRunOutcome::RuleCell(ExactRuleCellPromotionDisposition::Admitted(candidate)) =
        report.into_outcome()
    else {
        panic!("the canonical K=1 target must admit the depth-one translated source")
    };
    assert_eq!(candidate.epoch().requests().len(), 1);
    let scheduled = &candidate.epoch().requests().requests()[0];
    assert_eq!(scheduled.source_ordinal(), 0);
    assert_eq!(scheduled.offset().values(), &[-1]);
    assert_eq!(candidate.cell().rule().pivot().values(), &[0]);
    let expected_guard = one_loop_guard_polynomial(&generator, -1);
    assert!(
        candidate
            .cell()
            .guards()
            .iter()
            .any(|guard| guard.polynomial() == &expected_guard)
    );
    assert!(
        candidate
            .cell()
            .terms()
            .iter()
            .all(|term| term.descent().verify())
    );
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
        limits.max_post_hit_streamed_rows = 0;
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
fn k3_target_workspace_reuses_exact_structure_across_independent_probes() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 4;
    limits.max_post_hit_streamed_rows = 0;
    let mut workspace =
        SpiredTargetRunWorkspace::try_new(&generator, &completed, &case, limits).unwrap();

    let first = workspace.try_run_probe(&sunset_probe()).unwrap();
    assert!(matches!(
        first.outcome(),
        SpiredTargetRunOutcome::RuleCell(_)
    ));
    let shared = workspace.structural_census();
    assert_eq!(shared.exact_sources().source_rows(), 4);
    assert_eq!(shared.prepared_rows(), 3);
    assert_eq!(workspace.retained_prepared_rows(), 3);
    assert_eq!(
        workspace.retained_prepared_term_roles(),
        shared.emitted_term_roles()
    );
    assert_eq!(shared.request_inspections(), 3);
    assert_eq!(
        shared.source_term_inspections(),
        completed.relations()[..3]
            .iter()
            .map(|source| source.terms().len())
            .sum::<usize>()
    );

    let second = workspace
        .try_run_probe(&sunset_probe_with_modulus(SECOND_PRIME))
        .unwrap();
    assert!(matches!(
        second.outcome(),
        SpiredTargetRunOutcome::RuleCell(_)
    ));
    assert_eq!(workspace.structural_census(), shared);
    assert_eq!(workspace.retained_prepared_rows(), 3);
    assert_eq!(
        workspace.retained_prepared_term_roles(),
        shared.emitted_term_roles()
    );
    assert_eq!(first.census(), second.census());
}

#[test]
fn bounded_post_hit_window_exactly_checks_later_rooted_sunset_candidates() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 4;
    limits.max_post_hit_streamed_rows = 1;
    limits.max_distinct_compact_lift_attempts = 4;

    let report =
        try_run_spired_target(&generator, &completed, &case, &sunset_probe(), limits).unwrap();
    let census = report.census();
    assert!(matches!(
        report.outcome(),
        SpiredTargetRunOutcome::RuleCell(_)
    ));
    assert_eq!(census.post_hit_rows_streamed(), 1);
    assert!(census.post_hit_candidates_seen() > 0);
    assert_eq!(
        census.rooted_compact_lift_attempts(),
        census.post_hit_candidates_seen()
    );
    assert_eq!(census.distinct_compact_lift_attempts(), 2);
    assert_eq!(census.viable_candidates_compared(), 2);
    assert_eq!(census.best_candidate_replacements(), 0);
}

#[test]
fn later_sunset_nomination_materializes_a_distinct_rooted_exact_circuit() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let probe = sunset_probe();
    let limits = SpiredTargetRunLimits::default();
    let mut preparation = SpiredStructuralPreparation::try_new(
        generator.context(),
        &completed,
        &case,
        limits.streaming.structural_preparation(),
    )
    .unwrap();
    let requests = (0..completed.source_row_count())
        .map(|source_ordinal| {
            crate::identity::TranslatedSourceRequest::new(
                source_ordinal,
                IntegralShift::try_new([0, 0, 0]).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let prepared = preparation.try_prepare_requests(&requests).unwrap();
    let mut discovery =
        SpiredPreparedStreamingDiscovery::try_new(&preparation, &probe, limits.streaming.modular)
            .unwrap();
    let mut first = None;
    let mut later = None;
    for row in prepared.rows() {
        match discovery
            .try_consume_prepared_row_continuing(prepared.scope(), row)
            .unwrap()
        {
            SpiredModularStreamOutcome::Pending => {}
            SpiredModularStreamOutcome::FirstHit(hit) => first = Some(hit),
            SpiredModularStreamOutcome::PostHitCandidate(candidate) => {
                later = Some(candidate);
                break;
            }
        }
    }
    let first = first.expect("the sunset stream must retain its first target pivot");
    let later = later.expect("the next ordinary sunset row must nominate a rooted circuit");
    assert_ne!(
        later.root_source(),
        first.dependency_order().last().unwrap()
    );

    let mut bounded = limits.compact_lift;
    bounded.exact.max_selected_rows = later.dependency_trace().nodes().len() - 1;
    let rejected = try_lift_spired_rooted_compact_support(
        &case, &generator, &completed, &later, &probe, bounded,
    )
    .unwrap();
    assert!(matches!(
        &rejected,
        SpiredCompactLift::ExactSupportBudgetExceeded {
            requested_rows,
            limit,
            ..
        } if *requested_rows == later.dependency_trace().nodes().len()
            && *limit + 1 == *requested_rows
    ));
    assert!(rejected.epoch().is_none());

    let SpiredCompactLift::Replayed(first_lift) = try_lift_spired_compact_support(
        &case,
        &generator,
        &completed,
        &first,
        &probe,
        limits.compact_lift,
    )
    .unwrap() else {
        panic!("the first sunset circuit must replay exactly")
    };
    let SpiredCompactLift::Replayed(later_lift) = try_lift_spired_rooted_compact_support(
        &case,
        &generator,
        &completed,
        &later,
        &probe,
        limits.compact_lift,
    )
    .unwrap() else {
        panic!("the nominated later sunset circuit must replay exactly")
    };
    let first_sources = first_lift
        .circuit()
        .source_combination()
        .iter()
        .map(|term| {
            crate::identity::TranslatedSourceRequest::new(
                term.source_instance().provenance().source_ordinal(),
                term.source_instance().provenance().offset().clone(),
            )
        })
        .collect::<Vec<_>>();
    let later_sources = later_lift
        .circuit()
        .source_combination()
        .iter()
        .map(|term| {
            crate::identity::TranslatedSourceRequest::new(
                term.source_instance().provenance().source_ordinal(),
                term.source_instance().provenance().offset().clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_ne!(later_sources, first_sources);
    assert_eq!(later_sources.last(), Some(later.root_source()));
}

#[test]
fn oversized_first_support_is_inconclusive_and_a_later_bounded_candidate_is_admitted() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let probe = sunset_probe();
    let mut limits = SpiredTargetRunLimits::default();
    limits.compact_lift.exact.max_selected_rows = 4;
    let mut preparation = SpiredStructuralPreparation::try_new(
        generator.context(),
        &completed,
        &case,
        limits.streaming.structural_preparation(),
    )
    .unwrap();
    let requests = (0..completed.source_row_count())
        .map(|source_ordinal| {
            crate::identity::TranslatedSourceRequest::new(
                source_ordinal,
                IntegralShift::try_new([0, 0, 0]).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let prepared = preparation.try_prepare_requests(&requests).unwrap();
    let mut discovery =
        SpiredPreparedStreamingDiscovery::try_new(&preparation, &probe, limits.streaming.modular)
            .unwrap();
    let mut first = None;
    let mut later = None;
    for row in prepared.rows() {
        match discovery
            .try_consume_prepared_row_continuing(prepared.scope(), row)
            .unwrap()
        {
            SpiredModularStreamOutcome::Pending => {}
            SpiredModularStreamOutcome::FirstHit(hit) => first = Some(hit),
            SpiredModularStreamOutcome::PostHitCandidate(candidate) => {
                later = Some(candidate);
                break;
            }
        }
    }
    let mut first = first.expect("the sunset stream must retain its first target pivot");
    let later = later.expect("the next sunset row must nominate a rooted circuit");
    assert_eq!(later.dependency_trace().nodes().len(), 4);

    // The kernel normally returns a self-consistent trace. This deliberately
    // inflates only the early budget preflight projection so the control-flow
    // regression can exercise an oversized first proposal followed by the
    // real, independently materializable rooted proposal without allocating a
    // large exact polynomial frame.
    let repeated = first.dependency_order()[0].clone();
    let mut inflated = first.dependency_order().to_vec();
    while inflated.len() <= limits.compact_lift.exact.max_selected_rows {
        inflated.push(repeated.clone());
    }
    first.replace_dependency_order_for_budget_regression(inflated);

    let workspace =
        SpiredTargetRunWorkspace::try_new(&generator, &completed, &case, limits).unwrap();
    let mut census = super::SpiredTargetRunCensus::default();
    let rejected = try_attempt_branch_candidate(
        &workspace,
        &probe,
        BranchCandidateProposal::First(&first),
        &mut census,
        limits,
    )
    .unwrap();
    assert!(matches!(
        rejected,
        BranchCandidateAttempt::Rejected(super::run::AlternativeCandidateRejection::Compact(
            SpiredCompactLift::ExactSupportBudgetExceeded {
                requested_rows: 5,
                limit: 4,
                ..
            }
        ))
    ));
    assert_eq!(census.distinct_compact_lift_attempts(), 1);

    let admitted = try_attempt_branch_candidate(
        &workspace,
        &probe,
        BranchCandidateProposal::Rooted(&later),
        &mut census,
        limits,
    )
    .unwrap();
    assert!(matches!(admitted, BranchCandidateAttempt::Viable(_)));
    assert_eq!(census.distinct_compact_lift_attempts(), 2);
    assert_eq!(census.rooted_compact_lift_attempts(), 1);
}

#[test]
fn post_hit_resource_stop_returns_the_retained_exact_first_rule() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let first_hit_term_count = completed.relations()[..3]
        .iter()
        .map(|source| source.terms().len())
        .sum();
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 4;
    limits.max_post_hit_streamed_rows = 1;
    limits.streaming.max_preparation_request_inspections = 3;
    limits.streaming.max_preparation_source_term_inspections = first_hit_term_count;

    let mut workspace =
        SpiredTargetRunWorkspace::try_new(&generator, &completed, &case, limits).unwrap();
    let report = workspace.try_run_probe(&sunset_probe()).unwrap();
    assert!(matches!(
        report.outcome(),
        SpiredTargetRunOutcome::RuleCell(_)
    ));
    assert_eq!(report.census().distinct_compact_lift_attempts(), 1);
    assert_eq!(report.census().post_hit_rows_streamed(), 0);
    assert_eq!(workspace.structural_census().prepared_rows(), 3);
}

#[test]
fn post_hit_chunk_cap_returns_the_retained_exact_first_rule() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 1;
    limits.max_chunks = 3;
    limits.max_post_hit_streamed_rows = 1;

    let report =
        try_run_spired_target(&generator, &completed, &case, &sunset_probe(), limits).unwrap();
    assert!(matches!(
        report.outcome(),
        SpiredTargetRunOutcome::RuleCell(_)
    ));
    assert_eq!(report.census().chunks_materialized(), 3);
    assert_eq!(report.census().distinct_compact_lift_attempts(), 1);
    assert_eq!(report.census().post_hit_rows_streamed(), 0);
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

#[test]
fn zero_alternative_authority_caps_fail_before_any_row_is_scheduled() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case_for_target(&artifact, &completed, 1);
    let cases: [(&str, fn(&mut SpiredTargetRunLimits)); 2] = [
        ("target-run canonical source-exclusion branches", |limits| {
            limits.max_search_branches = 0
        }),
        ("target-run distinct compact-lift attempts", |limits| {
            limits.max_distinct_compact_lift_attempts = 0
        }),
    ];

    for (resource, configure) in cases {
        let mut limits = SpiredTargetRunLimits::default();
        configure(&mut limits);
        let error =
            try_run_spired_target(&generator, &completed, &case, &probe(), limits).unwrap_err();
        assert_eq!(error.stage(), SpiredTargetRunStage::Scheduling);
        assert!(matches!(
            error.cause(),
            SpiredTargetRunErrorCause::ResourceLimit {
                resource: actual,
                requested: 1,
                limit: 0,
            } if *actual == resource
        ));
        assert_eq!(error.census().scheduled_requests(), 0);
        assert_eq!(error.census().streamed_rows(), 0);
        assert_eq!(error.census().modular_hits_seen(), 0);
    }
}

#[test]
fn zero_streamed_row_cap_precedes_fresh_structural_row_preparation() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case_for_target(&artifact, &completed, 1);
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 1;
    limits.max_total_streamed_rows = 0;

    let mut workspace =
        SpiredTargetRunWorkspace::try_new(&generator, &completed, &case, limits).unwrap();
    assert_eq!(workspace.structural_census().prepared_rows(), 0);
    let error = workspace.try_run_probe(&probe()).unwrap_err();
    assert_eq!(error.stage(), SpiredTargetRunStage::Scheduling);
    assert!(matches!(
        error.cause(),
        SpiredTargetRunErrorCause::ResourceLimit {
            resource: "target-run aggregate streamed rows",
            requested: 1,
            limit: 0,
        }
    ));
    assert_eq!(error.census().streamed_rows(), 0);
    assert_eq!(workspace.structural_census().prepared_rows(), 0);
}

#[test]
fn zero_retained_tape_cap_fails_before_plan_or_modular_row_retention() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case_for_target(&artifact, &completed, 1);
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 1;
    limits.max_retained_prepared_rows = 0;

    let mut workspace =
        SpiredTargetRunWorkspace::try_new(&generator, &completed, &case, limits).unwrap();
    let error = workspace.try_run_probe(&probe()).unwrap_err();
    assert_eq!(error.stage(), SpiredTargetRunStage::Streaming);
    assert!(matches!(
        error.cause(),
        SpiredTargetRunErrorCause::ResourceLimit {
            resource: "target-run prepared structural tape rows",
            requested: 1,
            limit: 0,
        }
    ));
    assert_eq!(error.census().streamed_rows(), 0);
    assert_eq!(workspace.retained_prepared_rows(), 0);
    assert_eq!(workspace.retained_prepared_term_roles(), 0);
    assert_eq!(workspace.structural_census().prepared_rows(), 0);
}

#[test]
fn retained_role_cap_is_exact_and_fails_before_structural_classification() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case_for_target(&artifact, &completed, 1);
    let source_term_count = completed.relations()[0].terms().len();
    assert!(source_term_count > 0);

    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 0;
    limits.request_chunk_size = 1;
    limits.max_retained_prepared_term_roles = source_term_count - 1;
    let mut workspace =
        SpiredTargetRunWorkspace::try_new(&generator, &completed, &case, limits).unwrap();
    let error = workspace.try_run_probe(&probe()).unwrap_err();
    assert_eq!(error.stage(), SpiredTargetRunStage::Streaming);
    assert!(matches!(
        error.cause(),
        SpiredTargetRunErrorCause::ResourceLimit {
            resource: "target-run prepared structural tape term roles",
            requested,
            limit,
        } if *requested == source_term_count && *limit == source_term_count - 1
    ));
    assert_eq!(error.census().streamed_rows(), 0);
    assert_eq!(workspace.retained_prepared_rows(), 0);
    assert_eq!(workspace.retained_prepared_term_roles(), 0);
    let structural = workspace.structural_census();
    assert_eq!(structural.prepared_rows(), 0);
    assert_eq!(structural.request_inspections(), 0);
    assert_eq!(structural.source_term_inspections(), 0);
    assert_eq!(structural.unique_shifts(), 0);
    assert_eq!(structural.emitted_term_roles(), 0);

    limits.max_retained_prepared_term_roles = source_term_count;
    let mut exact =
        SpiredTargetRunWorkspace::try_new(&generator, &completed, &case, limits).unwrap();
    let report = exact.try_run_probe(&probe()).unwrap();
    assert!(matches!(
        report.outcome(),
        SpiredTargetRunOutcome::RuleCell(_)
    ));
    assert_eq!(exact.retained_prepared_rows(), 1);
    assert_eq!(exact.retained_prepared_term_roles(), source_term_count);
    assert_eq!(
        exact.structural_census().emitted_term_roles(),
        source_term_count
    );
}

#[test]
fn unlucky_sunset_support_resumes_canonically_and_exhausts_the_exact_attempt_budget() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    // At this deliberately tiny singular sample, the first several
    // rank-shaped supports do not lift over the exact coefficient field.
    let unlucky =
        CampaignModularProbe::try_new(3, [0], [0, 0, 0], CampaignLimits::default()).unwrap();
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 1;
    limits.request_chunk_size = 4;
    limits.max_distinct_compact_lift_attempts = 2;
    limits.max_post_hit_streamed_rows = 0;

    let mut workspace =
        SpiredTargetRunWorkspace::try_new(&generator, &completed, &case, limits).unwrap();
    let report = workspace.try_run_probe(&unlucky).unwrap();
    let SpiredTargetRunOutcome::CompactInconclusive { last, exhaustion } = report.outcome() else {
        panic!("the unlucky sample must remain explicitly inconclusive")
    };
    assert!(matches!(
        last,
        super::super::SpiredCompactLift::ExactSupportDidNotLift { .. }
    ));
    assert_eq!(
        *exhaustion,
        super::SpiredAlternativeExhaustion::DistinctCompactLiftAttempts
    );
    let census = report.census();
    assert_eq!(census.distinct_compact_lift_attempts(), 2);
    assert_eq!(census.modular_hits_seen(), 3);
    assert!(census.search_branches_started() >= 3);
    assert!(census.alternative_branches_enqueued() >= 4);
    assert!(census.excluded_requests_skipped() > 0);
    assert!(census.alternative_streamed_rows() > 0);
    assert!(census.retained_rejected_support_requests() > 0);
    let structural = workspace.structural_census();
    assert_eq!(structural.request_inspections(), structural.prepared_rows());
    assert!(
        structural.prepared_rows() < census.scheduled_requests(),
        "alternative branches must replay the shared tape instead of multiplying structural work"
    );
}

#[test]
fn zero_alternative_row_window_stops_before_preparing_a_later_row() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let unlucky =
        CampaignModularProbe::try_new(3, [0], [0, 0, 0], CampaignLimits::default()).unwrap();
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 1;
    limits.request_chunk_size = 4;
    limits.max_alternative_streamed_rows = 0;
    limits.max_post_hit_streamed_rows = 0;

    let mut workspace =
        SpiredTargetRunWorkspace::try_new(&generator, &completed, &case, limits).unwrap();
    let report = workspace.try_run_probe(&unlucky).unwrap();
    assert!(matches!(
        report.outcome(),
        SpiredTargetRunOutcome::CompactInconclusive {
            exhaustion: super::SpiredAlternativeExhaustion::AlternativeStreamedRows,
            ..
        }
    ));
    assert_eq!(report.census().distinct_compact_lift_attempts(), 1);
    assert_eq!(report.census().alternative_streamed_rows(), 0);
    assert_eq!(
        workspace.structural_census().prepared_rows(),
        report.census().streamed_rows(),
        "the zero post-hit window must not prepare a new alternative row"
    );
}

#[test]
fn exhausted_frontier_cap_rejects_children_before_retaining_requests() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let unlucky =
        CampaignModularProbe::try_new(3, [0], [0, 0, 0], CampaignLimits::default()).unwrap();
    let mut limits = SpiredTargetRunLimits::default();
    limits.max_depth_inclusive = 1;
    limits.request_chunk_size = 4;
    limits.max_search_branches = 1;
    limits.max_post_hit_streamed_rows = 0;

    let report = try_run_spired_target(&generator, &completed, &case, &unlucky, limits).unwrap();
    assert!(matches!(
        report.outcome(),
        SpiredTargetRunOutcome::CompactInconclusive {
            exhaustion: super::SpiredAlternativeExhaustion::SearchBranches,
            ..
        }
    ));
    let census = report.census();
    assert_eq!(census.distinct_compact_lift_attempts(), 1);
    assert_eq!(census.alternative_branches_enqueued(), 0);
    assert_eq!(census.retained_exclusion_requests(), 0);
    assert_eq!(census.retained_exclusion_coordinate_cells(), 0);
}

#[test]
fn independent_probe_after_an_inconclusive_lane_reaches_exact_sunset_authority() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = sunset_case(&artifact, 0);
    let unlucky =
        CampaignModularProbe::try_new(3, [0], [0, 0, 0], CampaignLimits::default()).unwrap();
    let mut unlucky_limits = SpiredTargetRunLimits::default();
    unlucky_limits.max_depth_inclusive = 1;
    unlucky_limits.request_chunk_size = 4;
    unlucky_limits.max_distinct_compact_lift_attempts = 1;
    unlucky_limits.max_post_hit_streamed_rows = 0;
    let first =
        try_run_spired_target(&generator, &completed, &case, &unlucky, unlucky_limits).unwrap();
    assert!(matches!(
        first.outcome(),
        SpiredTargetRunOutcome::CompactInconclusive { .. }
    ));

    let mut exact_limits = SpiredTargetRunLimits::default();
    exact_limits.max_depth_inclusive = 0;
    exact_limits.request_chunk_size = 4;
    exact_limits.max_post_hit_streamed_rows = 0;
    let recovered =
        try_run_spired_target(&generator, &completed, &case, &sunset_probe(), exact_limits)
            .unwrap();
    assert!(matches!(
        recovered.outcome(),
        SpiredTargetRunOutcome::RuleCell(_)
    ));
    assert_eq!(recovered.census().distinct_compact_lift_attempts(), 1);
}
