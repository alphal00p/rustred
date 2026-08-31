use crate::family::IntegralKey;
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, StratumRegistryLimits};
use crate::identity::ParametricIbpGenerator;
use crate::sector::{Mask, OrderingPolicy};

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

#[test]
fn empty_executable_stage_is_an_exact_nonfinite_stop_without_publication() {
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
    let StagedSectorClosureStop::NonFinite(evidence) = &stops[0] else {
        panic!("the owner-free orthant must stop as nonfinite")
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
