use crate::family::IntegralKey;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, StratumRegistryLimits};
use crate::identity::ParametricIbpGenerator;
use crate::sector::{Mask, OrderingPolicy};

use super::coordinator::validate_same_active_count;
use super::{
    StagedSectorClosureCoordinator, StagedSectorClosureError, StagedSectorClosureLimits,
    StagedSectorClosureOutcome, StagedSectorClosureStop,
};

fn tadpole_fixture() -> (ParametricIbpGenerator<'static>, ImmutableOwnerSnapshot) {
    // The installed artifact is leaked only inside this process-local unit
    // test so the generator can retain its borrowed family without inventing
    // a second fixture API in production code.
    let artifact = Box::leak(Box::new(derive_one_loop_unit_mass_tadpole().unwrap()));
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let predecessor = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        generator.context().fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    (generator, predecessor)
}

fn sunset_fixture() -> (ParametricIbpGenerator<'static>, ImmutableOwnerSnapshot) {
    let artifact = Box::leak(Box::new(derive_two_loop_unit_mass_sunset().unwrap()));
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let predecessor = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        generator.context().fingerprint(),
        3,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    (generator, predecessor)
}

#[test]
fn empty_executable_stage_has_a_typed_owner_absence_stop_without_publication() {
    let (generator, predecessor) = tadpole_fixture();
    let sector = Mask::try_new([true]).unwrap();
    let coordinator = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [(sector.clone(), OrderingPolicy::default())],
        StagedSectorClosureLimits::default(),
    )
    .unwrap();

    let StagedSectorClosureOutcome::Stopped(stops) = coordinator.try_finish().unwrap() else {
        panic!("an owner-free infinite orthant cannot publish a closed layer")
    };
    assert_eq!(stops.len(), 1);
    let StagedSectorClosureStop::NoExecutableOwners(evidence) = &stops[0] else {
        panic!("the owner-free orthant must retain its distinct typed stop")
    };
    assert_eq!(evidence.sector(), &sector);
    assert_eq!(evidence.ordering(), OrderingPolicy::default());
    assert_eq!(evidence.owner_count(), 0);
    assert_eq!(evidence.terminal_count(), 0);
    assert_eq!(evidence.uncovered_box_count(), 1);
    assert_eq!(evidence.missing_terminal_count(), 0);
    assert_eq!(evidence.guard_incomplete_owner_count(), 0);
    assert_eq!(predecessor.owner_count(), 0);
    assert_eq!(predecessor.closed_layer_count(), 0);

    let bounded = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [(sector.clone(), OrderingPolicy::default())],
        StagedSectorClosureLimits::default(),
    )
    .unwrap();
    let StagedSectorClosureOutcome::Stopped(stops) = bounded
        .try_finish_with_closure_carriers([(
            sector,
            OrderingPolicy::default(),
            LatticeBox::try_new([0], [Some(0)]).unwrap(),
        )])
        .unwrap()
    else {
        panic!("a terminal-free point carrier cannot publish a rewrite layer")
    };
    assert!(matches!(
        &stops[0],
        StagedSectorClosureStop::NoExecutableOwners(evidence)
            if evidence.uncovered_box_count() == 1
    ));
    assert_eq!(predecessor.closed_layer_count(), 0);
}

#[test]
fn frontier_and_terminal_ingress_are_exact_and_bounded() {
    let (generator, predecessor) = tadpole_fixture();
    let sector = Mask::try_new([true]).unwrap();
    assert!(matches!(
        StagedSectorClosureCoordinator::try_new(
            generator.context(),
            predecessor.clone(),
            [],
            StagedSectorClosureLimits::default(),
        ),
        Err(StagedSectorClosureError::EmptyFrontier)
    ));
    assert!(matches!(
        StagedSectorClosureCoordinator::try_new(
            generator.context(),
            predecessor.clone(),
            [
                (sector.clone(), OrderingPolicy::default()),
                (sector.clone(), OrderingPolicy::default()),
            ],
            StagedSectorClosureLimits::default(),
        ),
        Err(StagedSectorClosureError::DuplicateSector)
    ));

    let mut coordinator = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor,
        [(sector.clone(), OrderingPolicy::default())],
        StagedSectorClosureLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        coordinator.try_insert_terminal(
            &sector,
            OrderingPolicy::default(),
            IntegralKey::try_new([0]).unwrap(),
        ),
        Err(StagedSectorClosureError::TerminalOutsideSector)
    ));
    assert!(matches!(
        coordinator.try_insert_terminal(
            &sector,
            OrderingPolicy::default(),
            IntegralKey::try_new([1]).unwrap(),
        ),
        Err(StagedSectorClosureError::UnauthenticatedTerminal)
    ));
    assert_eq!(coordinator.terminal_count(), 0);
}

#[test]
fn explicit_closure_carrier_ingress_rejects_count_scope_and_geometry_drift() {
    let (generator, predecessor) = tadpole_fixture();
    let sector = Mask::try_new([true]).unwrap();
    let coordinator = || {
        StagedSectorClosureCoordinator::try_new(
            generator.context(),
            predecessor.clone(),
            [(sector.clone(), OrderingPolicy::default())],
            StagedSectorClosureLimits::default(),
        )
        .unwrap()
    };
    let valid = || LatticeBox::try_new([0], [Some(7)]).unwrap();

    assert!(matches!(
        coordinator().try_finish_with_closure_carriers([]),
        Err(StagedSectorClosureError::ClosureCarrierCountMismatch {
            expected: 1,
            actual: 0,
        })
    ));
    assert!(matches!(
        coordinator().try_finish_with_closure_carriers([
            (sector.clone(), OrderingPolicy::default(), valid()),
            (sector.clone(), OrderingPolicy::default(), valid()),
        ]),
        Err(StagedSectorClosureError::ClosureCarrierCountMismatch {
            expected: 1,
            actual: 2,
        })
    ));

    let foreign_sector = Mask::try_new([false]).unwrap();
    assert!(matches!(
        coordinator().try_finish_with_closure_carriers([(
            foreign_sector,
            OrderingPolicy::default(),
            LatticeBox::try_new([0], [Some(0)]).unwrap(),
        )]),
        Err(StagedSectorClosureError::ClosureCarrierScopeMismatch { carrier: 0 })
    ));
    assert!(matches!(
        coordinator().try_finish_with_closure_carriers([(
            sector.clone(),
            OrderingPolicy::default(),
            LatticeBox::try_new([1], [Some(1)]).unwrap(),
        )]),
        Err(StagedSectorClosureError::InvalidClosureCarrier { carrier: 0 })
    ));
    assert!(matches!(
        coordinator().try_finish_with_closure_carriers([(
            sector,
            OrderingPolicy::default(),
            LatticeBox::try_new([0], [Some(u64::MAX)]).unwrap(),
        )]),
        Err(StagedSectorClosureError::InvalidClosureCarrier { carrier: 0 })
    ));

    assert_eq!(predecessor.closed_layer_count(), 0);
}

#[test]
fn explicit_closure_carrier_ingress_rejects_duplicate_scope_before_compilation() {
    let (generator, predecessor) = sunset_fixture();
    let first = Mask::try_new([true, false, false]).unwrap();
    let second = Mask::try_new([false, true, false]).unwrap();
    let coordinator = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        predecessor.clone(),
        [
            (first.clone(), OrderingPolicy::default()),
            (second, OrderingPolicy::default()),
        ],
        StagedSectorClosureLimits::default(),
    )
    .unwrap();
    let carrier = || LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap();
    assert!(matches!(
        coordinator.try_finish_with_closure_carriers([
            (first.clone(), OrderingPolicy::default(), carrier()),
            (first, OrderingPolicy::default(), carrier()),
        ]),
        Err(StagedSectorClosureError::DuplicateClosureCarrier)
    ));
    assert_eq!(predecessor.closed_layer_count(), 0);
}

#[test]
fn sealed_wave_rank_validation_rejects_mixed_active_counts() {
    let rank_one = Mask::try_new([true, false, false]).unwrap();
    let rank_two = Mask::try_new([true, true, false]).unwrap();
    assert!(matches!(
        validate_same_active_count([(0, &rank_one), (1, &rank_two)]),
        Err(StagedSectorClosureError::MixedFrontierActiveCount {
            sector: 1,
            expected: 1,
            actual: 2,
        })
    ));
    assert!(validate_same_active_count([(0, &rank_two), (1, &rank_two)]).is_ok());
}
