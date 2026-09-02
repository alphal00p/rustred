use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ClosedArtifact, derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::frame::admission::{
    ExactCircuitOwnerCoverError, ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::guard::decision::GuardDecisionEvaluationLimits;
use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalObstructionScheduler, ProbeLocalSchedulerLimits,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, MaximalStratumAnchor, StratumRegistryError,
    StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::super::{
    CampaignModularProbe, CanonicalReplayBatch, CanonicalReplayDisposition, CanonicalReplayLimits,
    OrdinarySourceIncidenceIndex, SourceDiscoveryLimits, StagedSectorClosureCoordinator,
    StagedSectorClosureError, StagedSectorClosureLimits, StagedSectorClosureOutcome,
    try_canonicalize_replayed_probes,
};
use super::{
    ClosedExactExecutableOwnerCover, ClosedSectorLayer, ExactExecutableOwnerCover,
    ExactExecutableOwnerError, ExactExecutableOwnerLimits, ExactExecutableOwnerProposal,
    ExactExecutableOwnerSelection, try_compile_canonical_executable_owner,
};

const PRIME: u64 = 1_000_000_007;
const TADPOLE_CLOSURE_UPPER: u64 = 11;

fn tadpole_closure_carrier() -> LatticeBox {
    // The translated two-row discovery frame contains the physical +2
    // column, so its exact executable application region intentionally stops
    // before the i64::MAX representability fringe. Closure fixtures must name
    // a supported finite carrier rather than mint I(i64::MAX) as a terminal.
    LatticeBox::try_new([0], [Some(TADPOLE_CLOSURE_UPPER)]).unwrap()
}

fn compile_tadpole_cover(
    context: &crate::algebra::IndexedCoefficientContext,
    owners: Vec<Arc<super::ExactSemanticExecutableOwner>>,
    terminals: Vec<IntegralKey>,
    limits: ExactExecutableOwnerLimits,
) -> Result<ExactExecutableOwnerCover, ExactExecutableOwnerError> {
    ExactExecutableOwnerCover::try_compile_with_carrier(
        context,
        owners,
        terminals,
        &tadpole_closure_carrier(),
        limits,
    )
}

fn tadpole_staged_closure_carriers() -> [(Mask, OrderingPolicy, LatticeBox); 1] {
    [(
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        tadpole_closure_carrier(),
    )]
}

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
    let registry = StratumRegistryLimits::default();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        registry,
    )
    .unwrap();
    canonical_tadpole_batch_from_probes_and_owners(
        artifact,
        generator,
        completed,
        target_power,
        probes,
        owners,
    )
}

fn canonical_tadpole_batch_from_probes_and_owners(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target_power: i64,
    probes: impl IntoIterator<Item = CampaignModularProbe>,
    owners: ImmutableOwnerSnapshot,
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

fn compiled_tadpole_owner_with_predecessor(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target_power: i64,
    probe_coordinates: [u64; 2],
    predecessor: ImmutableOwnerSnapshot,
) -> Arc<super::ExactSemanticExecutableOwner> {
    let campaign = ProbeLocalSchedulerLimits::default().campaign;
    let probes = probe_coordinates.map(|coordinate| {
        CampaignModularProbe::try_new(PRIME, [37], [coordinate], campaign).unwrap()
    });
    let batch = canonical_tadpole_batch_from_probes_and_owners(
        artifact,
        generator,
        completed,
        target_power,
        probes,
        predecessor,
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

fn published_tadpole_layer(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    predecessor: ImmutableOwnerSnapshot,
) -> Arc<ClosedSectorLayer> {
    let campaign = ProbeLocalSchedulerLimits::default().campaign;
    let batch = canonical_tadpole_batch_from_probes_and_owners(
        artifact,
        generator,
        completed,
        1,
        [
            CampaignModularProbe::try_new(PRIME, [37], [2], campaign).unwrap(),
            CampaignModularProbe::try_new(PRIME, [37], [3], campaign).unwrap(),
        ],
        predecessor,
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
        panic!("the tadpole layer candidate must compile")
    };
    assert!(obstructions.is_empty());
    let cover = compile_tadpole_cover(
        generator.context(),
        vec![owner],
        vec![IntegralKey::try_new([1]).unwrap()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    let sealed = ClosedExactExecutableOwnerCover::try_seal(cover).unwrap();
    ClosedSectorLayer::try_publish(sealed, StratumRegistryLimits::default()).unwrap()
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
    let cover = compile_tadpole_cover(
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
fn full_machine_carrier_keeps_the_unrepresentable_tadpole_fringe_incomplete() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let owner = compiled_tadpole_owner(&artifact, &generator, &completed, 1, [2, 3]);
    let cover = ExactExecutableOwnerCover::try_compile(
        generator.context(),
        vec![owner],
        vec![IntegralKey::try_new([1]).unwrap()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();

    assert_eq!(
        cover.proof_cover().status(),
        ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::FiniteTerminalOwnership)
    );
    assert_eq!(cover.proof_cover().missing_terminals().len(), 1);
    assert_eq!(
        cover.proof_cover().missing_terminals()[0].coordinates(),
        &[i64::MAX as u64 - 1]
    );
}

#[test]
fn executable_owner_canonical_encoding_has_a_hard_prepublication_byte_limit() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let batch = canonical_tadpole_batch(&artifact, &generator, &completed, 1, [2, 3]);
    let mut limits = ExactExecutableOwnerLimits::default();
    limits.max_owner_encoded_content_bytes = 0;
    assert!(matches!(
        try_compile_canonical_executable_owner(generator.context(), batch, limits),
        Err(ExactExecutableOwnerError::ContentOrder(
            StratumRegistryError::ResourceLimit {
                resource: "exact executable owner canonical encoded bytes",
                limit: 0,
                ..
            }
        ))
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
        compile_tadpole_cover(
            generator.context(),
            vec![first.clone(), duplicate.clone()],
            vec![IntegralKey::try_new([1]).unwrap()],
            ExactExecutableOwnerLimits::default(),
        ),
        Err(ExactExecutableOwnerError::Cover(
            ExactCircuitOwnerCoverError::DuplicateOwnerContent
        ))
    ));

    let mut cover = compile_tadpole_cover(
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
fn cover_insert_rejects_structural_duplicates_from_an_independent_authority() {
    let installed = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let independently_installed = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(installed.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let retained = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&installed),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let foreign = ImmutableOwnerSnapshot::try_from_closed_artifact(
        independently_installed,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(retained, foreign);
    assert!(!retained.same_authority_as(&foreign));

    let owner = compiled_tadpole_owner_with_predecessor(
        &installed,
        &generator,
        &completed,
        1,
        [2, 3],
        retained,
    );
    let foreign_duplicate = compiled_tadpole_owner_with_predecessor(
        &installed,
        &generator,
        &completed,
        1,
        [2, 3],
        foreign,
    );
    assert!(
        super::compare_exact_owner_group_content(&owner, &foreign_duplicate)
            .unwrap()
            .is_eq()
    );
    let mut cover = compile_tadpole_cover(
        generator.context(),
        vec![owner.clone()],
        vec![IntegralKey::try_new([1]).unwrap()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        cover.try_insert(
            generator.context(),
            foreign_duplicate,
            ExactExecutableOwnerLimits::default(),
        ),
        Err(ExactExecutableOwnerError::AuthorityMismatch {
            candidate: 0,
            detail: "inserted owner uses a structurally equal but independently installed predecessor authority",
        })
    ));
    assert_eq!(cover.owners().len(), 1);
    assert!(Arc::ptr_eq(&cover.owners()[0], &owner));
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
        compile_tadpole_cover(
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

    let forward = compile_tadpole_cover(
        generator.context(),
        vec![owner.clone()],
        vec![first.clone(), second.clone()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    let reversed = compile_tadpole_cover(
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
fn retry_anchors_use_distinct_canonical_coordinates() {
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
    let batch = canonical_tadpole_batch_from_probes(&artifact, &generator, &completed, 1, probes);
    assert_eq!(batch.candidates().len(), 1);
    assert_eq!(batch.candidates()[0].anchor(), &[2]);
    assert_eq!(batch.candidates()[0].supporting_probes().len(), 4);
    let retries = super::compile::try_canonical_retry_anchors(
        batch.epoch(),
        &batch.candidates()[0],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    // Probe coordinates are sector-chart coordinates; the active tadpole
    // chart maps them to physical powers by adding one.
    assert_eq!(retries, [Box::<[i64]>::from([3]), Box::from([4])]);
}

#[test]
fn duplicate_supporting_probes_yield_one_retry_anchor() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let campaign = ProbeLocalSchedulerLimits::default().campaign;
    let probes = [
        CampaignModularProbe::try_new(PRIME, [10], [1], campaign).unwrap(),
        CampaignModularProbe::try_new(PRIME, [20], [2], campaign).unwrap(),
        CampaignModularProbe::try_new(PRIME, [25], [2], campaign).unwrap(),
    ];
    let batch = canonical_tadpole_batch_from_probes(&artifact, &generator, &completed, 1, probes);
    assert_eq!(batch.candidates().len(), 1);
    assert_eq!(batch.candidates()[0].anchor(), &[2]);
    assert_eq!(batch.candidates()[0].supporting_probes().len(), 3);
    let retries = super::compile::try_canonical_retry_anchors(
        batch.epoch(),
        &batch.candidates()[0],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    assert_eq!(retries, [Box::<[i64]>::from([3])]);
}

#[test]
fn missing_finite_terminal_remains_explicitly_incomplete_through_pairing() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let owner = compiled_tadpole_owner(&artifact, &generator, &completed, 1, [2, 3]);
    let cover = compile_tadpole_cover(
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

#[test]
fn closed_seal_consumes_the_cover_without_losing_retained_authority() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let owner = compiled_tadpole_owner(&artifact, &generator, &completed, 1, [2, 3]);
    let retained_epoch = owner.epoch().clone();
    let retained_circuit = owner.semantic().candidates()[0].circuit().clone();
    let weak_owner = Arc::downgrade(&owner);
    let weak_epoch = Arc::downgrade(&retained_epoch);
    let weak_semantic = Arc::downgrade(owner.semantic());
    let weak_circuit = Arc::downgrade(&retained_circuit);
    let retained_cell = owner.executable_candidates()[0].cell_owner().clone();
    let weak_cell = Arc::downgrade(&retained_cell);
    let owner_address = Arc::as_ptr(&owner);
    let circuit_address = Arc::as_ptr(&retained_circuit);
    let cell_address = Arc::as_ptr(&retained_cell);
    let predecessor_id = retained_epoch.fixed_snapshot_id().as_str().to_owned();

    let closure_carrier = LatticeBox::try_new([0], [Some(11)]).unwrap();
    let cover = ExactExecutableOwnerCover::try_compile_with_carrier(
        generator.context(),
        vec![owner.clone()],
        vec![IntegralKey::try_new([1]).unwrap()],
        &closure_carrier,
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    drop(retained_circuit);
    drop(retained_cell);
    drop(retained_epoch);
    drop(owner);

    let sealed = ClosedExactExecutableOwnerCover::try_seal(cover).unwrap();
    assert_eq!(
        sealed.executable_cover().proof_cover().status(),
        ExactOwnerCoverStatus::Closed
    );
    assert_eq!(sealed.predecessor_snapshot().id().as_str(), predecessor_id);
    assert_eq!(
        sealed.predecessor_snapshot().family_fingerprint(),
        sealed.executable_cover().proof_cover().family_fingerprint()
    );
    assert_eq!(
        sealed.predecessor_snapshot().context_fingerprint(),
        sealed
            .executable_cover()
            .proof_cover()
            .context_fingerprint()
    );
    assert_eq!(
        sealed.predecessor_snapshot().arity(),
        sealed.executable_cover().proof_cover().sector().arity()
    );
    assert_eq!(
        sealed.predecessor_snapshot().id(),
        sealed.executable_cover().proof_cover().owner_snapshot_id()
    );
    assert_eq!(
        Arc::as_ptr(&sealed.executable_cover().owners()[0]),
        owner_address
    );
    assert_eq!(
        Arc::as_ptr(
            sealed.executable_cover().owners()[0]
                .semantic()
                .candidates()[0]
                .circuit()
        ),
        circuit_address
    );
    assert_eq!(
        Arc::as_ptr(sealed.executable_cover().owners()[0].executable_candidates()[0].cell_owner()),
        cell_address
    );

    let ExactExecutableOwnerSelection::Descending { cell, .. } = sealed
        .executable_cover()
        .try_select_at(
            generator.context(),
            &IntegralKey::try_new([2]).unwrap(),
            GuardDecisionEvaluationLimits::default(),
        )
        .unwrap()
    else {
        panic!("the sealed tadpole cover must retain its executable recurrence")
    };
    assert_eq!(cell as *const _, cell_address);
    assert!(weak_owner.upgrade().is_some());
    assert!(weak_epoch.upgrade().is_some());
    assert!(weak_semantic.upgrade().is_some());
    assert!(weak_circuit.upgrade().is_some());
    assert!(weak_cell.upgrade().is_some());
}

#[test]
fn published_layer_identity_and_domain_retain_the_exact_finite_carrier() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owner = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [2, 3],
        predecessor.clone(),
    );
    let publish = |upper| {
        let carrier = LatticeBox::try_new([0], [Some(upper)]).unwrap();
        let cover = ExactExecutableOwnerCover::try_compile_with_carrier(
            generator.context(),
            vec![owner.clone()],
            vec![IntegralKey::try_new([1]).unwrap()],
            &carrier,
            ExactExecutableOwnerLimits::default(),
        )
        .unwrap();
        let sealed = ClosedExactExecutableOwnerCover::try_seal(cover).unwrap();
        ClosedSectorLayer::try_publish(sealed, StratumRegistryLimits::default()).unwrap()
    };

    let first = publish(11);
    let second = publish(12);
    assert_eq!(first.proven_domain().bounds(), [InteriorBounds::new(1, 12)]);
    assert_eq!(
        second.proven_domain().bounds(),
        [InteriorBounds::new(1, 13)]
    );
    assert_ne!(
        first.content_id(),
        second.content_id(),
        "distinct closure carriers must not share a published layer identity"
    );

    let extended = predecessor
        .try_extend_with_closed_layers(vec![first], StratumRegistryLimits::default())
        .unwrap();
    assert!(extended.solved_owner_matches_layer(0));
    assert!(
        extended
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
}

#[test]
fn closed_seal_rejects_a_finite_cover_without_explicit_terminal_ownership() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let owner = compiled_tadpole_owner(&artifact, &generator, &completed, 1, [2, 3]);
    let cover = compile_tadpole_cover(
        generator.context(),
        vec![owner],
        Vec::new(),
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        ClosedExactExecutableOwnerCover::try_seal(cover),
        Err(ExactExecutableOwnerError::CoverNotClosed {
            obstruction: ExactOwnerCoverObstructionKind::FiniteTerminalOwnership,
        })
    ));
}

#[test]
fn closed_seal_rechecks_the_exact_predecessor_scope_of_every_owner() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let first_owner = compiled_tadpole_owner(&artifact, &generator, &completed, 1, [2, 3]);
    let second_owner = compiled_tadpole_owner(&artifact, &generator, &completed, 2, [3, 4]);
    let mut cover = compile_tadpole_cover(
        generator.context(),
        vec![first_owner, second_owner],
        vec![IntegralKey::try_new([1]).unwrap()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();

    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let campaign = ProbeLocalSchedulerLimits::default().campaign;
    let batch = canonical_tadpole_batch_from_probes_and_owners(
        &artifact,
        &generator,
        &completed,
        2,
        [
            CampaignModularProbe::try_new(PRIME, [37], [3], campaign).unwrap(),
            CampaignModularProbe::try_new(PRIME, [37], [4], campaign).unwrap(),
        ],
        predecessor,
    );
    let ExactExecutableOwnerProposal::Compiled {
        owner: foreign_snapshot_owner,
        obstructions,
    } = try_compile_canonical_executable_owner(
        generator.context(),
        batch,
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap()
    else {
        panic!("the alternate-snapshot tadpole candidate must compile")
    };
    assert!(obstructions.is_empty());

    // Test-only corruption seam: the proof and first executable owner still
    // name the original empty snapshot, while the second owner now retains
    // another exact epoch.
    cover.owners[1] = foreign_snapshot_owner;
    assert!(matches!(
        ClosedExactExecutableOwnerCover::try_seal(cover),
        Err(ExactExecutableOwnerError::ClosedCoverScopeMismatch {
            owner: 1,
            detail: "predecessor snapshot identity differs",
        })
    ));
}

#[test]
fn closed_tadpole_layer_extends_one_exact_predecessor_transactionally() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let predecessor_id = predecessor.id().clone();
    let layer = published_tadpole_layer(&artifact, &generator, &completed, predecessor.clone());

    assert_eq!(layer.family_fingerprint(), artifact.family_fingerprint());
    assert_eq!(layer.context_fingerprint(), artifact.context_fingerprint());
    assert_eq!(layer.sector().active_bits(), &[true]);
    assert_eq!(layer.ordering(), OrderingPolicy::default());
    assert_eq!(layer.predecessor_snapshot().id(), &predecessor_id);
    assert!(
        layer
            .content_id()
            .as_str()
            .contains("closed-sector-layer-content.v2")
    );
    let recomputed = layer
        .try_recompute_content_id(StratumRegistryLimits::default())
        .unwrap();
    assert_eq!(&recomputed, layer.content_id());

    let weak_layer = Arc::downgrade(&layer);
    let extended = predecessor
        .try_extend_with_closed_layers(vec![layer.clone()], StratumRegistryLimits::default())
        .unwrap();
    assert_eq!(extended.owner_count(), 1);
    assert_eq!(extended.closed_layer_count(), 1);
    assert!(extended.solved_owner_matches_layer(0));
    assert!(
        extended
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
    assert!(extended.id().as_str().contains(layer.content_id().as_str()));

    let clone = extended.clone();
    assert_eq!(clone, extended);
    assert!(clone.same_authority_as(&extended));
    drop(layer);
    assert!(weak_layer.upgrade().is_some());

    let same_sector =
        SectorInteriorDomain::try_new(Mask::try_new([true]).unwrap(), [InteriorBounds::new(1, 7)])
            .unwrap();
    assert!(
        extended
            .owner_for(
                &Mask::try_new([true]).unwrap(),
                OrderingPolicy::default(),
                &same_sector,
            )
            .is_none(),
        "owner lookup must reject a target that is not a strict subsector"
    );
}

#[test]
fn solved_layer_extension_preserves_a_closed_artifact_terminal_prefix() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let terminal_owner_count = predecessor.owner_count();
    let parent_sector = Mask::try_new([true]).unwrap();
    let zero_domain =
        SectorInteriorDomain::try_new(Mask::try_new([false]).unwrap(), [InteriorBounds::new(0, 0)])
            .unwrap();
    let retained_witness = predecessor
        .owner_for(&parent_sector, OrderingPolicy::default(), &zero_domain)
        .unwrap();
    let layer = published_tadpole_layer(&artifact, &generator, &completed, predecessor.clone());
    let extended = predecessor
        .try_extend_with_closed_layers(vec![layer], StratumRegistryLimits::default())
        .unwrap();

    assert_eq!(extended.owner_count(), terminal_owner_count + 1);
    assert!(extended.solved_owner_matches_layer(0));
    assert_eq!(
        extended.owner_for(&parent_sector, OrderingPolicy::default(), &zero_domain),
        Some(retained_witness),
        "append-only extension must preserve every pre-existing witness ordinal",
    );
    assert!(
        extended
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
}

#[test]
fn structurally_equal_solved_snapshots_do_not_alias_distinct_layer_arcs() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let first_layer =
        published_tadpole_layer(&artifact, &generator, &completed, predecessor.clone());
    let second_layer =
        published_tadpole_layer(&artifact, &generator, &completed, predecessor.clone());
    assert!(!Arc::ptr_eq(&first_layer, &second_layer));
    assert_eq!(first_layer.content_id(), second_layer.content_id());

    let first = predecessor
        .try_extend_with_closed_layers(vec![first_layer], StratumRegistryLimits::default())
        .unwrap();
    let second = predecessor
        .try_extend_with_closed_layers(vec![second_layer], StratumRegistryLimits::default())
        .unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(first, second);
    assert!(!first.same_authority_as(&second));
}

#[test]
fn closed_layer_content_id_covers_exact_circuit_and_rule_cell_payloads() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let layer = published_tadpole_layer(&artifact, &generator, &completed, predecessor);
    let candidate =
        &layer.executable_cover().executable_cover().owners()[0].executable_candidates()[0];

    // A cloned circuit with one exact source cofactor changed is deliberately
    // not republished: exact replay would reject it. The focused test seam
    // changes only that canonical-stream component and proves it is covered.
    let mut changed_circuit = candidate.circuit().as_ref().clone();
    let changed_coefficient = generator
        .context()
        .neg_with_limits(
            changed_circuit.source_combination()[0].coefficient(),
            Default::default(),
        )
        .unwrap();
    changed_circuit.replace_first_source_coefficient_for_test(changed_coefficient);
    let circuit_id = layer
        .try_content_id_with_first_circuit_for_test(
            &changed_circuit,
            StratumRegistryLimits::default(),
        )
        .unwrap();
    assert_ne!(&circuit_id, layer.content_id());

    assert!(!candidate.cell().guards().is_empty());
    let shifted = generator
        .context()
        .add(
            &generator.context().index(0).unwrap(),
            &generator.context().one(),
        )
        .unwrap();
    let changed_guard = generator
        .context()
        .numerator_condition_with_limits(&shifted, Default::default())
        .unwrap();
    let cell_id = layer
        .try_content_id_with_first_cell_guard_for_test(
            &changed_guard,
            StratumRegistryLimits::default(),
        )
        .unwrap();
    assert_ne!(&cell_id, layer.content_id());
    assert_ne!(cell_id, circuit_id);

    let mut exhausted = StratumRegistryLimits::default();
    exhausted.max_owner_identity_bytes = 0;
    assert!(matches!(
        layer.try_recompute_content_id(exhausted),
        Err(StratumRegistryError::ResourceLimit {
            resource: "closed-sector layer canonical content bytes",
            ..
        })
    ));
}

#[test]
fn closed_layer_batch_rejections_leave_the_predecessor_unchanged() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let original_id = predecessor.id().clone();
    let layer = published_tadpole_layer(&artifact, &generator, &completed, predecessor.clone());

    assert_eq!(
        predecessor.try_extend_with_closed_layers(Vec::new(), StratumRegistryLimits::default(),),
        Err(StratumRegistryError::EmptyClosedSectorLayerBatch)
    );
    assert!(matches!(
        predecessor.try_extend_with_closed_layers(
            vec![layer.clone(), layer.clone()],
            StratumRegistryLimits::default(),
        ),
        Err(StratumRegistryError::DuplicateClosedSectorOwner { .. })
    ));

    let foreign_predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        foreign_predecessor
            .try_extend_with_closed_layers(vec![layer.clone()], StratumRegistryLimits::default(),),
        Err(StratumRegistryError::WrongClosedSectorLayerPredecessor { layer: 0 })
    ));

    let wrong_family = ImmutableOwnerSnapshot::try_empty(
        "foreign-family",
        artifact.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        wrong_family
            .try_extend_with_closed_layers(vec![layer.clone()], StratumRegistryLimits::default(),),
        Err(StratumRegistryError::WrongClosedSectorLayerFamily { layer: 0 })
    ));

    let wrong_context = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        "foreign-context",
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        wrong_context
            .try_extend_with_closed_layers(vec![layer.clone()], StratumRegistryLimits::default(),),
        Err(StratumRegistryError::WrongClosedSectorLayerContext { layer: 0 })
    ));

    let wrong_arity = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        2,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        wrong_arity
            .try_extend_with_closed_layers(vec![layer.clone()], StratumRegistryLimits::default(),),
        Err(StratumRegistryError::WrongOwnerArity {
            owner: 0,
            expected: 2,
            actual: 1,
        })
    ));

    let first_wave = predecessor
        .try_extend_with_closed_layers(vec![layer.clone()], StratumRegistryLimits::default())
        .unwrap();
    let split_same_rank =
        published_tadpole_layer(&artifact, &generator, &completed, first_wave.clone());
    assert_eq!(
        first_wave.try_extend_with_closed_layers(
            vec![split_same_rank],
            StratumRegistryLimits::default(),
        ),
        Err(
            StratumRegistryError::NonIncreasingClosedSectorLayerFrontier {
                previous_active_count: 1,
                incoming_active_count: 1,
            }
        )
    );
    assert_eq!(first_wave.closed_layer_count(), 1);
    assert!(
        first_wave
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );

    let mut exhausted = StratumRegistryLimits::default();
    exhausted.max_owner_regions = 0;
    assert!(matches!(
        predecessor.try_extend_with_closed_layers(vec![layer.clone()], exhausted),
        Err(StratumRegistryError::ResourceLimit {
            resource: "immutable owner regions",
            requested: 1,
            limit: 0,
        })
    ));
    let mut route_exhausted = StratumRegistryLimits::default();
    route_exhausted.max_owner_routes = 0;
    assert!(matches!(
        predecessor.try_extend_with_closed_layers(vec![layer.clone()], route_exhausted),
        Err(StratumRegistryError::ResourceLimit {
            resource: "immutable owner symmetry routes",
            requested: 1,
            limit: 0,
        })
    ));
    let mut route_coordinate_exhausted = StratumRegistryLimits::default();
    route_coordinate_exhausted.max_owner_route_coordinate_cells = 0;
    assert!(matches!(
        predecessor.try_extend_with_closed_layers(vec![layer.clone()], route_coordinate_exhausted,),
        Err(StratumRegistryError::ResourceLimit {
            resource: "immutable owner symmetry-route coordinate cells",
            requested: 3,
            limit: 0,
        })
    ));
    let mut coordinate_exhausted = StratumRegistryLimits::default();
    coordinate_exhausted.max_owner_coordinate_cells = 0;
    assert!(matches!(
        predecessor.try_extend_with_closed_layers(vec![layer], coordinate_exhausted),
        Err(StratumRegistryError::ResourceLimit {
            resource: "immutable owner coordinate cells",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(predecessor.id(), &original_id);
    assert_eq!(predecessor.owner_count(), 0);
    assert_eq!(predecessor.closed_layer_count(), 0);
    assert!(
        predecessor
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
}

#[test]
fn staged_coordinator_seals_and_publishes_only_a_complete_consumed_wave() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let owner = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [2, 3],
        predecessor.clone(),
    );
    let duplicate = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [2, 3],
        predecessor.clone(),
    );
    let mut coordinator = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [(sector.clone(), OrderingPolicy::default())],
        StagedSectorClosureLimits::default(),
    )
    .unwrap();
    assert!(coordinator.try_insert_owner(owner).unwrap());
    assert!(!coordinator.try_insert_owner(duplicate).unwrap());
    assert!(
        coordinator
            .try_insert_terminal(
                &sector,
                OrderingPolicy::default(),
                IntegralKey::try_new([1]).unwrap(),
            )
            .unwrap()
    );

    let StagedSectorClosureOutcome::Closed(wave) = coordinator
        .try_finish_with_closure_carriers(tadpole_staged_closure_carriers())
        .unwrap()
    else {
        panic!("the exact tadpole recurrence plus its explicit corner must close")
    };
    assert!(wave.predecessor().same_authority_as(&predecessor));
    assert_eq!(wave.layers().len(), 1);
    assert_eq!(wave.layers()[0].sector(), &sector);
    assert!(
        wave.layers()[0]
            .predecessor_snapshot()
            .same_authority_as(&predecessor)
    );
    assert_eq!(
        wave.successor().owner_count(),
        predecessor.owner_count() + 1
    );
    assert_eq!(wave.successor().closed_layer_count(), 1);
    assert!(wave.successor().solved_owner_matches_layer(0));
    assert!(
        !wave
            .successor()
            .authenticates_explicit_terminal(&IntegralKey::try_new([2]).unwrap())
            .unwrap(),
        "a solved rewrite sector must never mint terminal authority"
    );
    assert!(
        wave.successor()
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
    assert_eq!(predecessor.owner_count(), 2);
    assert_eq!(predecessor.closed_layer_count(), 0);
}

#[test]
fn staged_proof_equivalent_owner_selection_is_arrival_order_independent() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let first = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [2, 3],
        predecessor.clone(),
    );
    let second = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [17, 19],
        predecessor.clone(),
    );
    assert!(super::compare_exact_owner_proof_content(&first, &second).is_eq());
    assert_ne!(
        first.executable_candidates()[0]
            .cell()
            .rule()
            .concrete_replay()
            .anchor(),
        second.executable_candidates()[0]
            .cell()
            .rule()
            .concrete_replay()
            .anchor(),
        "the regression needs distinct executable replay witnesses",
    );
    let full_order = super::compare_exact_owner_group_content(&first, &second).unwrap();
    assert!(!full_order.is_eq());
    let expected_key = if full_order.is_lt() {
        first.content_order_key()
    } else {
        second.content_order_key()
    };

    let publish = |earlier: Arc<super::ExactSemanticExecutableOwner>,
                   later: Arc<super::ExactSemanticExecutableOwner>| {
        let mut coordinator = StagedSectorClosureCoordinator::try_new(
            generator.context(),
            predecessor.clone(),
            [(sector.clone(), OrderingPolicy::default())],
            StagedSectorClosureLimits::default(),
        )
        .unwrap();
        assert!(coordinator.try_insert_owner(earlier).unwrap());
        let _ = coordinator.try_insert_owner(later).unwrap();
        assert_eq!(coordinator.owner_count(), 1);
        assert!(
            coordinator
                .try_insert_terminal(
                    &sector,
                    OrderingPolicy::default(),
                    IntegralKey::try_new([1]).unwrap(),
                )
                .unwrap()
        );
        let StagedSectorClosureOutcome::Closed(wave) = coordinator
            .try_finish_with_closure_carriers(tadpole_staged_closure_carriers())
            .unwrap()
        else {
            panic!("the canonical retained representative must still close")
        };
        wave
    };

    let forward = publish(first.clone(), second.clone());
    let reversed = publish(second, first);
    let forward_owner = &forward.layers()[0]
        .executable_cover()
        .executable_cover()
        .owners()[0];
    let reversed_owner = &reversed.layers()[0]
        .executable_cover()
        .executable_cover()
        .owners()[0];
    assert_eq!(forward_owner.content_order_key(), expected_key);
    assert_eq!(reversed_owner.content_order_key(), expected_key);
    assert_eq!(
        forward.layers()[0].content_id(),
        reversed.layers()[0].content_id()
    );
    assert_eq!(forward.successor().id(), reversed.successor().id());
}

#[test]
fn compact_owner_key_collision_uses_exact_fallback_and_stable_replacement() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let first = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [2, 3],
        predecessor.clone(),
    );
    let mut second = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [17, 19],
        predecessor.clone(),
    );
    assert_eq!(
        first.content_order_key().encoded_len(),
        second.content_order_key().encoded_len(),
        "fixed-width probe values should give equal exact encoding lengths",
    );
    Arc::get_mut(&mut second)
        .unwrap()
        .replace_content_order_key_for_test(first.content_order_key());

    let exact_order = super::compare_exact_owner_group_content(&first, &second).unwrap();
    assert!(
        !exact_order.is_eq(),
        "distinct exact encodings must not merge"
    );
    assert_eq!(
        super::compare_exact_owner_group_content(&second, &first).unwrap(),
        exact_order.reverse()
    );
    let retained_key_bytes = first.content_order_key().retained_bytes();
    let insert_pair = |earlier: Arc<super::ExactSemanticExecutableOwner>,
                       later: Arc<super::ExactSemanticExecutableOwner>| {
        let mut coordinator = StagedSectorClosureCoordinator::try_new(
            generator.context(),
            predecessor.clone(),
            [(sector.clone(), OrderingPolicy::default())],
            StagedSectorClosureLimits::default(),
        )
        .unwrap();
        assert!(coordinator.try_insert_owner(earlier).unwrap());
        let replaced = coordinator.try_insert_owner(later).unwrap();
        assert_eq!(coordinator.owner_count(), 1);
        assert_eq!(coordinator.owner_content_key_bytes(), retained_key_bytes);
        replaced
    };

    let forward_replaced = insert_pair(first.clone(), second.clone());
    let reversed_replaced = insert_pair(second, first);
    assert_eq!(forward_replaced, exact_order.is_gt());
    assert_eq!(reversed_replaced, exact_order.is_lt());
}

#[test]
fn collision_fallback_failure_propagates_through_both_replacement_boundaries() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let mut first = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [2, 3],
        predecessor.clone(),
    );
    let mut second = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [17, 19],
        predecessor.clone(),
    );
    let impossible_key = first.content_order_key().with_encoded_len_for_test(0);
    Arc::get_mut(&mut first)
        .unwrap()
        .replace_content_order_key_for_test(impossible_key);
    Arc::get_mut(&mut second)
        .unwrap()
        .replace_content_order_key_for_test(impossible_key);

    assert!(matches!(
        super::compare_exact_owner_group_content(&first, &second),
        Err(StratumRegistryError::ResourceLimit {
            resource: "exact executable owner collision-fallback canonical bytes",
            limit: 0,
            ..
        })
    ));

    let mut cover = compile_tadpole_cover(
        generator.context(),
        vec![first.clone()],
        vec![IntegralKey::try_new([1]).unwrap()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        cover.try_insert(
            generator.context(),
            second.clone(),
            ExactExecutableOwnerLimits::default(),
        ),
        Err(ExactExecutableOwnerError::ContentOrder(
            StratumRegistryError::ResourceLimit {
                resource: "exact executable owner collision-fallback canonical bytes",
                limit: 0,
                ..
            }
        ))
    ));
    assert!(Arc::ptr_eq(&cover.owners()[0], &first));

    let mut coordinator = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor,
        [(sector, OrderingPolicy::default())],
        StagedSectorClosureLimits::default(),
    )
    .unwrap();
    assert!(coordinator.try_insert_owner(first.clone()).unwrap());
    let retained_key_bytes = coordinator.owner_content_key_bytes();
    assert!(matches!(
        coordinator.try_insert_owner(second),
        Err(StagedSectorClosureError::Registry(
            StratumRegistryError::ResourceLimit {
                resource: "exact executable owner collision-fallback canonical bytes",
                limit: 0,
                ..
            }
        ))
    ));
    assert_eq!(coordinator.owner_count(), 1);
    assert_eq!(coordinator.owner_content_key_bytes(), retained_key_bytes);
}

#[test]
fn staged_owner_terminal_and_pairing_envelopes_reject_transactionally() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let first = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [2, 3],
        predecessor.clone(),
    );
    let proof_equivalent = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [17, 19],
        predecessor.clone(),
    );
    let second_proof = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        2,
        [3, 4],
        predecessor.clone(),
    );
    assert!(
        first.content_order_key().retained_bytes() < first.content_order_key().encoded_len(),
        "staged accounting must charge the compact resident key, not its canonical replay",
    );

    let mut limits = StagedSectorClosureLimits::default();
    limits.max_staged_owner_content_key_bytes = first.content_order_key().retained_bytes() - 1;
    let mut bytes_limited = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [(sector.clone(), OrderingPolicy::default())],
        limits,
    )
    .unwrap();
    assert!(matches!(
        bytes_limited.try_insert_owner(first.clone()),
        Err(StagedSectorClosureError::ResourceLimit {
            resource: "staged sector-closure retained owner content-key bytes",
            ..
        })
    ));
    assert_eq!(bytes_limited.owner_count(), 0);
    assert_eq!(bytes_limited.owner_content_key_bytes(), 0);

    let mut limits = StagedSectorClosureLimits::default();
    limits.max_owner_order_comparisons = 0;
    let mut comparison_limited = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [(sector.clone(), OrderingPolicy::default())],
        limits,
    )
    .unwrap();
    assert!(comparison_limited.try_insert_owner(first.clone()).unwrap());
    let retained_bytes = comparison_limited.owner_content_key_bytes();
    assert!(matches!(
        comparison_limited.try_insert_owner(proof_equivalent),
        Err(StagedSectorClosureError::ResourceLimit {
            resource: "staged sector-closure owner order comparisons",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(comparison_limited.owner_count(), 1);
    assert_eq!(comparison_limited.owner_content_key_bytes(), retained_bytes);
    assert_eq!(comparison_limited.owner_order_comparisons(), 0);

    let mut limits = StagedSectorClosureLimits::default();
    limits.max_staged_terminal_coordinate_cells = 0;
    let mut terminal_limited = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [(sector.clone(), OrderingPolicy::default())],
        limits,
    )
    .unwrap();
    assert!(matches!(
        terminal_limited.try_insert_terminal(
            &sector,
            OrderingPolicy::default(),
            IntegralKey::try_new([1]).unwrap(),
        ),
        Err(StagedSectorClosureError::ResourceLimit {
            resource: "staged sector-closure terminal coordinate cells",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(terminal_limited.terminal_count(), 0);
    assert_eq!(terminal_limited.terminal_coordinate_cells(), 0);

    let mut limits = StagedSectorClosureLimits::default();
    limits.max_compiled_pairing_probes = 3;
    let mut pairing_limited = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [(sector.clone(), OrderingPolicy::default())],
        limits,
    )
    .unwrap();
    assert!(pairing_limited.try_insert_owner(first).unwrap());
    assert!(pairing_limited.try_insert_owner(second_proof).unwrap());
    assert!(
        pairing_limited
            .try_insert_terminal(
                &sector,
                OrderingPolicy::default(),
                IntegralKey::try_new([1]).unwrap(),
            )
            .unwrap()
    );
    assert!(matches!(
        pairing_limited.try_finish_with_closure_carriers(tadpole_staged_closure_carriers()),
        Err(StagedSectorClosureError::ResourceLimit {
            resource: "staged sector-closure compiled pairing probes",
            requested: 4,
            limit: 3,
        })
    ));
    assert_eq!(predecessor.closed_layer_count(), 0);
}

#[test]
fn staged_compiled_work_envelope_accepts_its_exact_census_boundary() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let terminal = IntegralKey::try_new([1]).unwrap();
    let owner = compiled_tadpole_owner_with_predecessor(
        &artifact,
        &generator,
        &completed,
        1,
        [2, 3],
        predecessor.clone(),
    );
    let baseline = compile_tadpole_cover(
        generator.context(),
        vec![owner.clone()],
        vec![terminal.clone()],
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    let proof = baseline.proof_cover();
    assert_eq!(proof.status(), ExactOwnerCoverStatus::Closed);
    assert!(proof.finite_complement_point_count() > 0);
    assert!(proof.compiled_uncovered_box_count() > 0);
    assert!(proof.compiled_uncovered_box_coordinate_cells() > 0);

    let mut limits = StagedSectorClosureLimits::default();
    limits.max_compiled_pairing_probes = 1;
    limits.max_compiled_finite_complement_points = proof.finite_complement_point_count();
    limits.max_compiled_finite_complement_coordinate_cells =
        proof.finite_complement_point_count() * sector.arity();
    limits.max_compiled_point_owner_probes = proof.point_owner_probe_count();
    limits.max_compiled_uncovered_boxes = proof.compiled_uncovered_box_count();
    limits.max_compiled_uncovered_box_coordinate_cells =
        proof.compiled_uncovered_box_coordinate_cells();
    limits.max_compiled_split_operations = proof.compiled_split_operation_count();

    // Isolate the finite-point census failure. Tightening every other census
    // simultaneously changes the structural normalization path when the
    // point budget reaches zero and would test a different resource first.
    let mut insufficient = StagedSectorClosureLimits::default();
    insufficient.max_compiled_pairing_probes = 1;
    insufficient.max_compiled_finite_complement_points = proof.finite_complement_point_count() - 1;
    let mut rejected = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [(sector.clone(), OrderingPolicy::default())],
        insufficient,
    )
    .unwrap();
    assert!(rejected.try_insert_owner(owner.clone()).unwrap());
    assert!(
        rejected
            .try_insert_terminal(&sector, OrderingPolicy::default(), terminal.clone(),)
            .unwrap()
    );
    let rejection = rejected
        .try_finish_with_closure_carriers(tadpole_staged_closure_carriers())
        .unwrap_err();
    assert!(matches!(
        rejection,
        StagedSectorClosureError::ResourceLimit {
            resource: "staged sector-closure finite complement points",
            requested: 1,
            limit: 0,
        }
    ));
    assert_eq!(predecessor.closed_layer_count(), 0);

    let mut coordinator = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [(sector.clone(), OrderingPolicy::default())],
        limits,
    )
    .unwrap();
    assert!(coordinator.try_insert_owner(owner).unwrap());
    assert!(
        coordinator
            .try_insert_terminal(&sector, OrderingPolicy::default(), terminal)
            .unwrap()
    );
    let StagedSectorClosureOutcome::Closed(wave) = coordinator
        .try_finish_with_closure_carriers(tadpole_staged_closure_carriers())
        .unwrap()
    else {
        panic!("the exact aggregate census boundary must admit its closed cover")
    };
    assert_eq!(wave.layers().len(), 1);
    assert_eq!(wave.successor().closed_layer_count(), 1);
}
