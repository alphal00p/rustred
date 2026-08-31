use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ClosedArtifact, derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::frame::admission::ExactOwnerCoverStatus;
use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalObstructionScheduler, ProbeLocalSchedulerLimits,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, MaximalStratumAnchor, StratumRegistryLimits,
};
use crate::foundry::completion::{LatticeBox, UncoveredPartition};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{
    CampaignModularProbe, CanonicalReplayDisposition, CanonicalReplayLimits,
    ExactExecutableOwnerLimits, ExactExecutableOwnerProposal, OrdinarySourceIncidenceIndex,
    SourceDiscoveryLimits, try_canonicalize_replayed_probes,
    try_compile_canonical_executable_owner,
};
use super::geometry::{ExactPartitionDelta, try_compare_from_owner_free, try_compare_partitions};
use super::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaError, ExactOwnerCoverDeltaKind,
    ExactOwnerCoverDeltaLimits, ExactOwnerLedgerCoverStatus,
};

mod k6;

const PRIME: u64 = 1_000_000_007;

fn lattice_box(lower: &[u64], upper: &[Option<u64>]) -> LatticeBox {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).unwrap()
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn compiled_owner(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    predecessor: ImmutableOwnerSnapshot,
    sector: Mask,
    target: IntegralShift,
    probe_coordinates: &[&[u64]],
) -> Arc<super::super::ExactSemanticExecutableOwner> {
    let discovery = SourceDiscoveryLimits::default();
    let zero = IntegralShift::try_new(std::iter::repeat_n(0, sector.arity())).unwrap();
    let zero_sources = generator
        .translate_completed_source_rows(completed, [zero], discovery.translation)
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, discovery).unwrap();
    let bootstrap = incidence
        .try_nominate_target_unit(&target, discovery)
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
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, target.values(), &physical_shifts)
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
    let probes = probe_coordinates.iter().map(|coordinates| {
        CampaignModularProbe::try_new(
            PRIME,
            [37],
            coordinates.iter().copied(),
            scheduler_limits.campaign,
        )
        .unwrap()
    });
    let report = ProbeLocalObstructionScheduler::try_new(
        generator,
        completed,
        target.clone(),
        anchor.clone(),
        predecessor.clone(),
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
        predecessor,
        OrderingPolicy::default(),
        &report,
        CanonicalReplayLimits::default(),
    )
    .unwrap() else {
        panic!("the focused exact probes must produce a canonical replay batch")
    };
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
        panic!("the focused canonical batch must compile to an executable owner")
    };
    assert!(obstructions.is_empty());
    owner
}

struct TadpoleFixture {
    generator: ParametricIbpGenerator<'static>,
    predecessor: ImmutableOwnerSnapshot,
    first: Arc<super::super::ExactSemanticExecutableOwner>,
    redundant: Arc<super::super::ExactSemanticExecutableOwner>,
}

fn tadpole_fixture() -> TadpoleFixture {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    // The process-local test leak avoids adding a fixture lifetime to the
    // production ledger API.
    let leaked = Box::leak(Box::new(artifact.clone()));
    let generator = ParametricIbpGenerator::try_new(leaked.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        artifact.clone(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let first = compiled_owner(
        &artifact,
        &generator,
        &completed,
        predecessor.clone(),
        sector.clone(),
        IntegralShift::try_new([1]).unwrap(),
        &[&[2], &[3]],
    );
    let redundant = compiled_owner(
        &artifact,
        &generator,
        &completed,
        predecessor.clone(),
        sector,
        IntegralShift::try_new([2]).unwrap(),
        &[&[3], &[4]],
    );
    TadpoleFixture {
        generator,
        predecessor,
        first,
        redundant,
    }
}

fn tadpole_ledger(
    fixture: &TadpoleFixture,
    limits: ExactOwnerCoverDeltaLimits,
) -> CanonicalExactOwnerLedger {
    CanonicalExactOwnerLedger::try_new(
        fixture.generator.context(),
        fixture.predecessor.clone(),
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        [IntegralKey::try_new([1]).unwrap()],
        limits,
    )
    .unwrap()
}

#[test]
fn one_loop_first_owner_strictly_shrinks_and_only_the_compiler_closes() {
    let fixture = tadpole_fixture();
    let mut ledger = tadpole_ledger(&fixture, ExactOwnerCoverDeltaLimits::default());
    assert_eq!(
        ledger.snapshot().status(),
        ExactOwnerLedgerCoverStatus::OwnerFree
    );
    let owner_free = ledger.try_clone_uncovered_partition().unwrap();
    assert_eq!(owner_free.boxes().len(), 1);
    assert_eq!(owner_free.boxes()[0].free_dimension(), 1);

    let delta = ledger.try_apply_owner(fixture.first.clone()).unwrap();
    assert_eq!(
        delta.kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert!(delta.strictly_shrank());
    assert_eq!(delta.baseline().owner_count(), 0);
    assert_eq!(delta.updated().owner_count(), 1);
    assert_eq!(
        delta.updated().status(),
        ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Closed)
    );
    assert!(delta.updated().status().is_compiler_closed());
    let compiled = ledger.try_clone_uncovered_partition().unwrap();
    assert!(compiled.is_finite());
    assert!(Arc::ptr_eq(&ledger.owners()[0], &fixture.first));
    assert!(
        ledger
            .predecessor_snapshot()
            .same_authority_as(&fixture.predecessor)
    );
}

#[test]
fn duplicate_and_redundant_owner_are_typed_without_false_shrink() {
    let fixture = tadpole_fixture();
    let mut ledger = tadpole_ledger(&fixture, ExactOwnerCoverDeltaLimits::default());
    ledger.try_apply_owner(fixture.first.clone()).unwrap();

    let duplicate = ledger.try_apply_owner(fixture.first.clone()).unwrap();
    assert_eq!(duplicate.kind(), ExactOwnerCoverDeltaKind::Duplicate);
    assert_eq!(duplicate.baseline(), duplicate.updated());
    assert_eq!(ledger.owners().len(), 1);

    let redundant = ledger.try_apply_owner(fixture.redundant.clone()).unwrap();
    assert_eq!(
        redundant.kind(),
        ExactOwnerCoverDeltaKind::ChangedWithoutGeometricShrink
    );
    assert!(!redundant.strictly_shrank());
    assert_eq!(ledger.owners().len(), 2);
    assert!(redundant.updated().status().is_compiler_closed());
}

#[test]
fn exact_comparison_limit_one_below_is_transactional() {
    let fixture = tadpole_fixture();
    let mut limits = ExactOwnerCoverDeltaLimits::default();
    limits.max_comparison_box_inputs = 1;
    let mut ledger = tadpole_ledger(&fixture, limits);
    let error = ledger.try_apply_owner(fixture.first).unwrap_err();
    assert!(matches!(
        error,
        ExactOwnerCoverDeltaError::ResourceLimit {
            resource: "exact cover-delta comparison box inputs",
            requested: 2,
            limit: 1,
        }
    ));
    assert!(ledger.owners().is_empty());
    assert_eq!(
        ledger.snapshot().status(),
        ExactOwnerLedgerCoverStatus::OwnerFree
    );
}

#[test]
fn final_owner_ledger_is_arrival_order_independent() {
    let fixture = tadpole_fixture();
    let mut forward = tadpole_ledger(&fixture, ExactOwnerCoverDeltaLimits::default());
    let mut reverse = tadpole_ledger(&fixture, ExactOwnerCoverDeltaLimits::default());
    forward.try_apply_owner(fixture.first.clone()).unwrap();
    forward.try_apply_owner(fixture.redundant.clone()).unwrap();
    reverse.try_apply_owner(fixture.redundant.clone()).unwrap();
    reverse.try_apply_owner(fixture.first.clone()).unwrap();

    assert_eq!(forward.snapshot(), reverse.snapshot());
    let forward_keys = forward
        .owners()
        .iter()
        .map(|owner| owner.content_order_key())
        .collect::<Vec<_>>();
    let reverse_keys = reverse
        .owners()
        .iter()
        .map(|owner| owner.content_order_key())
        .collect::<Vec<_>>();
    assert_eq!(forward_keys, reverse_keys);
}

#[test]
fn exact_union_comparison_is_independent_of_box_decomposition() {
    let split_full = UncoveredPartition::new(
        vec![
            lattice_box(&[0, 0], &[Some(0), None]),
            lattice_box(&[1, 0], &[None, None]),
        ],
        0,
    );
    let unsplit_full = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[None, None])], 0);
    assert_eq!(
        try_compare_partitions(
            &split_full,
            &unsplit_full,
            2,
            ExactOwnerCoverDeltaLimits::default(),
        )
        .unwrap(),
        ExactPartitionDelta::Equal
    );

    let staircase = UncoveredPartition::new(
        vec![
            lattice_box(&[0, 0], &[Some(0), None]),
            lattice_box(&[1, 0], &[None, Some(0)]),
        ],
        0,
    );
    let refined = UncoveredPartition::new(
        vec![
            lattice_box(&[0, 0], &[None, Some(0)]),
            lattice_box(&[0, 1], &[Some(0), Some(1)]),
        ],
        0,
    );
    assert_eq!(
        try_compare_partitions(
            &staircase,
            &refined,
            2,
            ExactOwnerCoverDeltaLimits::default(),
        )
        .unwrap(),
        ExactPartitionDelta::StrictSubset
    );
}

#[test]
fn owner_free_full_orthant_is_preflighted_before_endpoint_allocation() {
    let mut limits = ExactOwnerCoverDeltaLimits::default();
    limits.max_comparison_coordinate_cells = 1;
    let empty = UncoveredPartition::new(Vec::new(), 0);
    assert!(matches!(
        try_compare_from_owner_free(1, &empty, limits),
        Err(ExactOwnerCoverDeltaError::ResourceLimit {
            resource: "exact cover-delta comparison coordinate cells",
            requested: 2,
            limit: 1,
        })
    ));
}

#[test]
fn two_loop_dot_owner_uses_the_same_topology_neutral_delta_path() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let leaked = Box::leak(Box::new(artifact.clone()));
    let generator = ParametricIbpGenerator::try_new(leaked.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        artifact.clone(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true, true, true]).unwrap();
    let owner = compiled_owner(
        &artifact,
        &generator,
        &completed,
        predecessor.clone(),
        sector.clone(),
        IntegralShift::try_new([2, 1, 1]).unwrap(),
        &[&[2, 3, 5], &[3, 5, 7]],
    );
    let mut ledger = CanonicalExactOwnerLedger::try_new(
        generator.context(),
        predecessor,
        sector,
        OrderingPolicy::default(),
        [IntegralKey::try_new([1, 1, 1]).unwrap()],
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();

    let delta = ledger.try_apply_owner(owner.clone()).unwrap();
    assert_eq!(
        delta.kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert!(Arc::ptr_eq(&ledger.owners()[0], &owner));
    assert_eq!(ledger.sector().arity(), 3);
    assert_eq!(ledger.ordering(), OrderingPolicy::default());
}
