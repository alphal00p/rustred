use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ClosedArtifact, derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::frame::admission::{
    ExactCircuitOwnerCoverError, ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::guard::decision::GuardDecisionEvaluationLimits;
use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalObstructionScheduler, ProbeLocalSchedulerLimits,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, MaximalStratumAnchor, StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{
    CampaignModularProbe, CanonicalReplayBatch, CanonicalReplayDisposition, CanonicalReplayLimits,
    OrdinarySourceIncidenceIndex, SourceDiscoveryLimits, try_canonicalize_replayed_probes,
};
use super::{
    ExactExecutableOwnerCover, ExactExecutableOwnerError, ExactExecutableOwnerLimits,
    ExactExecutableOwnerProposal, ExactExecutableOwnerSelection,
    try_compile_canonical_executable_owner,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn canonical_tadpole_batch(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target_power: i64,
    probe_coordinates: [u64; 2],
) -> CanonicalReplayBatch {
    let scheduler_limits = ProbeLocalSchedulerLimits::default();
    let probes = probe_coordinates.map(|coordinate| {
        CampaignModularProbe::try_new(PRIME, [37], [coordinate], scheduler_limits.campaign).unwrap()
    });
    canonical_tadpole_batch_from_probes(artifact, generator, completed, target_power, probes)
}

fn canonical_tadpole_batch_from_probes(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target_power: i64,
    probes: impl IntoIterator<Item = CampaignModularProbe>,
) -> CanonicalReplayBatch {
    let target = IntegralShift::try_new([target_power]).unwrap();
    let source_limits = SourceDiscoveryLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            completed,
            [IntegralShift::try_new([0]).unwrap()],
            source_limits.translation,
        )
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, source_limits).unwrap();
    let bootstrap = incidence
        .try_nominate_target_unit(&target, source_limits)
        .unwrap();
    let scheduler_limits = ProbeLocalSchedulerLimits::default();
    let selected = generator
        .translate_selected_completed_source_rows(
            completed,
            bootstrap.requests().iter().cloned(),
            scheduler_limits.campaign.translated_sources,
        )
        .unwrap();
    let physical_shifts = selected
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .map(|shift| shift.values().to_vec())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &physical_shifts,
    )
    .unwrap();
    let registry = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        registry,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        registry,
    )
    .unwrap();
    let anchor = MaximalStratumAnchor::try_new(stratum, registry).unwrap();
    let report = ProbeLocalObstructionScheduler::try_new(
        generator,
        completed,
        target.clone(),
        anchor.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        probes,
        scheduler_limits,
    )
    .unwrap()
    .run()
    .unwrap();
    let CanonicalReplayDisposition::Rebased(batch) = try_canonicalize_replayed_probes(
        generator,
        completed,
        target,
        anchor,
        owners,
        OrderingPolicy::default(),
        &report,
        CanonicalReplayLimits::default(),
    )
    .unwrap() else {
        panic!("the canonical tadpole probes must produce an exact replay batch")
    };
    batch
}

fn compiled_tadpole_owner(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target_power: i64,
    probe_coordinates: [u64; 2],
) -> Arc<super::ExactSemanticExecutableOwner> {
    let batch = canonical_tadpole_batch(
        artifact,
        generator,
        completed,
        target_power,
        probe_coordinates,
    );
    let ExactExecutableOwnerProposal::Compiled {
        owner,
        obstructions,
    } = try_compile_canonical_executable_owner(
        generator.context(),
        batch,
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap()
    else {
        panic!("the canonical tadpole candidate must be globally executable")
    };
    assert!(obstructions.is_empty());
    owner
}

#[test]
fn canonical_batch_compiles_to_a_pointer_paired_closed_executable_owner() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let batch = canonical_tadpole_batch(&artifact, &generator, &completed, 1, [2, 3]);
    let retained_epoch = batch.epoch().clone();
    let retained_circuit = batch.candidates()[0].circuit().clone();
    let ExactExecutableOwnerProposal::Compiled {
        owner,
        obstructions,
    } = try_compile_canonical_executable_owner(
        generator.context(),
        batch,
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap()
    else {
        panic!("the canonical tadpole candidate must compile")
    };
    assert!(obstructions.is_empty());
    assert!(Arc::ptr_eq(owner.epoch(), &retained_epoch));
    assert_eq!(owner.executable_candidates().len(), 1);
    assert!(Arc::ptr_eq(
        owner.executable_candidates()[0].epoch(),
        &retained_epoch
    ));
    assert!(Arc::ptr_eq(
        owner.executable_candidates()[0].circuit(),
        &retained_circuit
    ));
    assert!(Arc::ptr_eq(
        owner.semantic().candidates()[0].circuit(),
        &retained_circuit
    ));

    let terminal = IntegralKey::try_new([1]).unwrap();
    let cover = ExactExecutableOwnerCover::try_compile(
        generator.context(),
        vec![owner.clone()],
        vec![terminal.clone()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    assert_eq!(cover.proof_cover().status(), ExactOwnerCoverStatus::Closed);
    assert!(Arc::ptr_eq(&cover.owners()[0], &owner));

    let target = IntegralKey::try_new([2]).unwrap();
    let ExactExecutableOwnerSelection::Descending {
        owner_ordinal,
        candidate_ordinal,
        circuit,
        cell,
    } = cover
        .try_select_at(
            generator.context(),
            &target,
            GuardDecisionEvaluationLimits::default(),
        )
        .unwrap()
    else {
        panic!("I(2) must select the exact tadpole recurrence")
    };
    assert_eq!((owner_ordinal, candidate_ordinal), (0, 0));
    assert!(Arc::ptr_eq(circuit, &retained_circuit));
    assert!(std::ptr::eq(cell, owner.executable_candidates()[0].cell()));
    assert_eq!(
        cell.rule().pivot().values(),
        retained_epoch.target_shift().values()
    );
    assert_eq!(
        cell.application_domain(),
        retained_epoch.fixed_stratum().domain()
    );

    assert!(matches!(
        cover
            .try_select_at(
                generator.context(),
                &terminal,
                GuardDecisionEvaluationLimits::default(),
            )
            .unwrap(),
        ExactExecutableOwnerSelection::Terminal(selected) if selected == &terminal
    ));
}

#[test]
fn failed_whole_cover_recompute_leaves_the_published_pairing_untouched() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let first = compiled_tadpole_owner(&artifact, &generator, &completed, 1, [2, 3]);
    let duplicate = compiled_tadpole_owner(&artifact, &generator, &completed, 1, [2, 3]);
    let additional = compiled_tadpole_owner(&artifact, &generator, &completed, 2, [3, 4]);
    assert!(!Arc::ptr_eq(&first, &duplicate));
    assert!(!Arc::ptr_eq(first.semantic(), duplicate.semantic()));

    assert!(matches!(
        ExactExecutableOwnerCover::try_compile(
            generator.context(),
            vec![first.clone(), duplicate.clone()],
            vec![IntegralKey::try_new([1]).unwrap()],
            ExactExecutableOwnerLimits::default(),
        ),
        Err(ExactExecutableOwnerError::Cover(
            ExactCircuitOwnerCoverError::DuplicateOwnerContent
        ))
    ));

    let mut cover = ExactExecutableOwnerCover::try_compile(
        generator.context(),
        vec![first.clone()],
        vec![IntegralKey::try_new([1]).unwrap()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    let target = IntegralKey::try_new([2]).unwrap();
    let (before_circuit, before_cell) = match cover
        .try_select_at(
            generator.context(),
            &target,
            GuardDecisionEvaluationLimits::default(),
        )
        .unwrap()
    {
        ExactExecutableOwnerSelection::Descending { circuit, cell, .. } => {
            (Arc::as_ptr(circuit), cell as *const _)
        }
        _ => panic!("baseline tadpole cover must select an executable rule"),
    };
    let before_status = cover.proof_cover().status();
    let before_boxes = format!("{:?}", cover.proof_cover().uncovered_partition().boxes());

    let sunset = derive_two_loop_unit_mass_sunset().unwrap();
    let sunset_generator = ParametricIbpGenerator::try_new(sunset.family()).unwrap();
    assert!(matches!(
        cover.try_insert(
            sunset_generator.context(),
            duplicate.clone(),
            ExactExecutableOwnerLimits::default(),
        ),
        Err(ExactExecutableOwnerError::WrongContext)
    ));
    assert_eq!(cover.owners().len(), 1);
    assert!(Arc::ptr_eq(&cover.owners()[0], &first));

    assert!(
        !cover
            .try_insert(
                generator.context(),
                duplicate,
                ExactExecutableOwnerLimits::default(),
            )
            .unwrap()
    );

    let mut failing_limits = ExactExecutableOwnerLimits::default();
    failing_limits.max_pairing_probes = 0;
    assert!(matches!(
        cover.try_insert(generator.context(), additional, failing_limits),
        Err(ExactExecutableOwnerError::ResourceLimit {
            resource: "semantic executable owner pairing probes",
            requested: 1,
            limit: 0,
        })
    ));

    assert_eq!(cover.owners().len(), 1);
    assert!(Arc::ptr_eq(&cover.owners()[0], &first));
    assert_eq!(cover.proof_cover().status(), before_status);
    assert_eq!(
        format!("{:?}", cover.proof_cover().uncovered_partition().boxes()),
        before_boxes
    );
    match cover
        .try_select_at(
            generator.context(),
            &target,
            GuardDecisionEvaluationLimits::default(),
        )
        .unwrap()
    {
        ExactExecutableOwnerSelection::Descending { circuit, cell, .. } => {
            assert_eq!(Arc::as_ptr(circuit), before_circuit);
            assert_eq!(cell as *const _, before_cell);
        }
        _ => panic!("failed recomputation must not replace the baseline selection"),
    }
}

#[test]
fn paired_owner_cover_is_deterministic_under_reversed_group_input() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let first = compiled_tadpole_owner(&artifact, &generator, &completed, 1, [2, 3]);
    let second = compiled_tadpole_owner(&artifact, &generator, &completed, 2, [3, 4]);
    assert_eq!(
        first.executable_candidates()[0]
            .cell()
            .rule()
            .pivot()
            .values(),
        &[1]
    );
    assert_eq!(
        second.executable_candidates()[0]
            .cell()
            .rule()
            .pivot()
            .values(),
        &[2]
    );

    let compile = |owners| {
        ExactExecutableOwnerCover::try_compile(
            generator.context(),
            owners,
            vec![IntegralKey::try_new([1]).unwrap()],
            ExactExecutableOwnerLimits::default(),
        )
        .unwrap()
    };
    let forward = compile(vec![first.clone(), second.clone()]);
    let reversed = compile(vec![second.clone(), first.clone()]);

    assert_eq!(
        forward.proof_cover().status(),
        reversed.proof_cover().status()
    );
    assert_eq!(
        format!("{:?}", forward.proof_cover().uncovered_partition().boxes()),
        format!("{:?}", reversed.proof_cover().uncovered_partition().boxes())
    );
    assert_eq!(forward.owners().len(), 2);
    assert_eq!(reversed.owners().len(), 2);
    for ordinal in 0..2 {
        assert!(Arc::ptr_eq(
            &forward.owners()[ordinal],
            &reversed.owners()[ordinal]
        ));
        assert!(Arc::ptr_eq(
            forward.proof_cover().owners()[ordinal].semantic(),
            forward.owners()[ordinal].semantic()
        ));
        assert!(Arc::ptr_eq(
            reversed.proof_cover().owners()[ordinal].semantic(),
            reversed.owners()[ordinal].semantic()
        ));
    }
    assert!(Arc::ptr_eq(&forward.owners()[0], &first));
    assert!(Arc::ptr_eq(&forward.owners()[1], &second));
}

#[test]
fn executable_terminal_sidecar_uses_the_proof_covers_canonical_order() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let owner = compiled_tadpole_owner(&artifact, &generator, &completed, 2, [3, 4]);
    let first = IntegralKey::try_new([1]).unwrap();
    let second = IntegralKey::try_new([2]).unwrap();

    let forward = ExactExecutableOwnerCover::try_compile(
        generator.context(),
        vec![owner.clone()],
        vec![first.clone(), second.clone()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    let reversed = ExactExecutableOwnerCover::try_compile(
        generator.context(),
        vec![owner],
        vec![second.clone(), first.clone()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();

    assert_eq!(
        forward.proof_cover().status(),
        ExactOwnerCoverStatus::Closed
    );
    assert_eq!(
        reversed.proof_cover().status(),
        ExactOwnerCoverStatus::Closed
    );
    assert_eq!(forward.terminals(), &[first.clone(), second.clone()]);
    assert_eq!(reversed.terminals(), &[first, second]);
}

#[test]
fn guard_wall_retries_use_distinct_anchors_in_canonical_coordinate_order() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let campaign = ProbeLocalSchedulerLimits::default().campaign;
    let probes = [
        CampaignModularProbe::try_new(PRIME, [10], [1], campaign).unwrap(),
        CampaignModularProbe::try_new(PRIME, [20], [3], campaign).unwrap(),
        CampaignModularProbe::try_new(PRIME, [25], [3], campaign).unwrap(),
        CampaignModularProbe::try_new(PRIME, [30], [2], campaign).unwrap(),
    ];
    let mut batch =
        canonical_tadpole_batch_from_probes(&artifact, &generator, &completed, 1, probes);
    assert_eq!(batch.candidates().len(), 1);
    assert_eq!(batch.candidates()[0].anchor(), &[2]);
    assert_eq!(batch.candidates()[0].supporting_probes().len(), 4);
    let second = generator
        .context()
        .sub(
            &generator.context().index(0).unwrap(),
            &generator.context().integer(2),
        )
        .unwrap();
    let fourth = generator
        .context()
        .sub(
            &generator.context().index(0).unwrap(),
            &generator.context().integer(4),
        )
        .unwrap();
    let guard = generator.context().mul(&second, &fourth).unwrap();
    batch.replace_first_candidate_guard_polynomial_for_test(
        generator
            .context()
            .numerator_condition_with_limits(&guard, Default::default())
            .unwrap(),
    );

    let mut limits = ExactExecutableOwnerLimits::default();
    limits.max_promotion_attempts = 2;
    let ExactExecutableOwnerProposal::Incomplete(incomplete) =
        try_compile_canonical_executable_owner(generator.context(), batch, limits).unwrap()
    else {
        panic!("the interior guard roots must remain an explicit incomplete proposal")
    };
    assert_eq!(incomplete.obstructions().len(), 1);
    assert!(matches!(
        incomplete.obstructions()[0].obstruction(),
        super::ExactExecutableOwnerObstruction::NeedsGuardedStratum { .. }
    ));
}

#[test]
fn duplicate_retry_anchors_do_not_turn_wall_exhaustion_into_a_budget_error() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let campaign = ProbeLocalSchedulerLimits::default().campaign;
    let probes = [
        CampaignModularProbe::try_new(PRIME, [10], [1], campaign).unwrap(),
        CampaignModularProbe::try_new(PRIME, [20], [2], campaign).unwrap(),
        CampaignModularProbe::try_new(PRIME, [25], [2], campaign).unwrap(),
    ];
    let mut batch =
        canonical_tadpole_batch_from_probes(&artifact, &generator, &completed, 1, probes);
    let second = generator
        .context()
        .sub(
            &generator.context().index(0).unwrap(),
            &generator.context().integer(2),
        )
        .unwrap();
    let third = generator
        .context()
        .sub(
            &generator.context().index(0).unwrap(),
            &generator.context().integer(3),
        )
        .unwrap();
    let guard = generator.context().mul(&second, &third).unwrap();
    batch.replace_first_candidate_guard_polynomial_for_test(
        generator
            .context()
            .numerator_condition_with_limits(&guard, Default::default())
            .unwrap(),
    );

    let mut limits = ExactExecutableOwnerLimits::default();
    limits.max_promotion_attempts = 2;
    let ExactExecutableOwnerProposal::Incomplete(incomplete) =
        try_compile_canonical_executable_owner(generator.context(), batch, limits).unwrap()
    else {
        panic!("exhausted distinct wall anchors must remain an incomplete proposal")
    };
    assert_eq!(incomplete.obstructions().len(), 1);
    assert!(matches!(
        incomplete.obstructions()[0].obstruction(),
        super::ExactExecutableOwnerObstruction::AnchorOnGuardWall { .. }
    ));
}

#[test]
fn partially_obstructed_batch_publishes_only_the_admitted_exact_candidate() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut batch = canonical_tadpole_batch(&artifact, &generator, &completed, 1, [2, 3]);
    let guard = generator
        .context()
        .sub(
            &generator.context().index(0).unwrap(),
            &generator.context().one(),
        )
        .unwrap();
    let obstructed_circuit = batch.append_guard_modified_first_candidate_for_test(
        generator
            .context()
            .numerator_condition_with_limits(&guard, Default::default())
            .unwrap(),
    );

    let ExactExecutableOwnerProposal::Compiled {
        owner,
        obstructions,
    } = try_compile_canonical_executable_owner(
        generator.context(),
        batch,
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap()
    else {
        panic!("one total candidate must remain publishable")
    };
    assert_eq!(owner.executable_candidates().len(), 1);
    assert_eq!(owner.semantic().candidates().len(), 1);
    assert!(Arc::ptr_eq(
        owner.semantic().candidates()[0].circuit(),
        owner.executable_candidates()[0].circuit(),
    ));
    assert_eq!(obstructions.len(), 1);
    assert!(Arc::ptr_eq(obstructions[0].circuit(), &obstructed_circuit,));
    assert!(matches!(
        obstructions[0].obstruction(),
        super::ExactExecutableOwnerObstruction::NeedsGuardedStratum { .. }
    ));

    let cover = ExactExecutableOwnerCover::try_compile(
        generator.context(),
        vec![owner],
        vec![IntegralKey::try_new([1]).unwrap()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    assert_eq!(cover.proof_cover().status(), ExactOwnerCoverStatus::Closed);
}

#[test]
fn missing_finite_terminal_remains_explicitly_incomplete_through_pairing() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let owner = compiled_tadpole_owner(&artifact, &generator, &completed, 1, [2, 3]);
    let cover = ExactExecutableOwnerCover::try_compile(
        generator.context(),
        vec![owner],
        Vec::new(),
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    assert_eq!(
        cover.proof_cover().status(),
        ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::FiniteTerminalOwnership)
    );
    assert!(matches!(
        cover
            .try_select_at(
                generator.context(),
                &IntegralKey::try_new([1]).unwrap(),
                GuardDecisionEvaluationLimits::default(),
            )
            .unwrap(),
        ExactExecutableOwnerSelection::Incomplete
    ));
}
