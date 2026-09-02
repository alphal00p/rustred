use super::*;
use crate::family::IntegralKey;
use crate::foundry::artifact::fresh_k6_terminal_authority_for_test;
use crate::foundry::campaign::{
    FoundryCampaignConfig, FoundryCampaignExternalHints, FoundryCampaignPreset,
    FoundryCampaignProbe,
};
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::source_discovery::{
    ExactOwnerCoverDeltaLimits, ProbeCoordinatorOperationalReason,
    test_fixtures::OracleDisabledK6Fixture,
};
use crate::sector::Mask;

#[test]
fn full_rank_wave_manifest_and_fresh_siblings_share_each_predecessor() {
    validate_wave_manifest().unwrap();
    let inputs = shared_k6_algebra_inputs().unwrap();
    let root = super::super::preset_k6::shared_k6_root_predecessor().unwrap();
    let mut orbit_start = 0usize;
    for (wave_ordinal, &wave_width) in K6_FULL_RANK_WAVE_WIDTHS.iter().enumerate() {
        let resource_profile = K6CampaignResourceProfile::try_for_task_report_ceiling(1).unwrap();
        let ledgers =
            try_build_wave_ledgers(inputs, orbit_start, wave_width, &root, resource_profile)
                .unwrap();
        assert_eq!(ledgers.len(), wave_width);
        assert!(ledgers.windows(2).all(|pair| {
            !pair[0]
                .snapshot_identity()
                .same_ledger_as(&pair[1].snapshot_identity())
        }));
        for (local_ordinal, ledger) in ledgers.iter().enumerate() {
            let orbit = FULL_RANK_ORBITS[orbit_start + local_ordinal];
            assert_eq!(
                ledger.sector(),
                &Mask::try_from_indices(&orbit.representative).unwrap()
            );
            assert!(ledger.predecessor_snapshot().same_authority_as(&root));
            assert_eq!(ledger.revision().get(), 0);
            assert_eq!(ledger.terminals().len(), 1);
            assert_eq!(ledger.terminals()[0].powers(), orbit.representative);
            assert_eq!(ledger.sector().active_count(), wave_ordinal + 3);
        }
        orbit_start += wave_width;
    }
    assert_eq!(orbit_start, FULL_RANK_ORBITS.len());
}

#[test]
fn bounded_first_wave_drives_both_siblings_without_partial_publication() {
    let root = super::super::preset_k6::shared_k6_root_predecessor().unwrap();
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 1).unwrap();
    let K6WaveCampaignOutcome::Incomplete(incomplete) = try_run_k6_full_rank_waves(
        &config,
        root.clone(),
        StagedSectorClosureLimits::default(),
        1,
    )
    .unwrap() else {
        panic!("one task per sector cannot close the first K6 wave")
    };
    assert_eq!(incomplete.wave_ordinal(), 0);
    assert_eq!(incomplete.active_count(), 3);
    assert_eq!(incomplete.closed_sector_count(), 0);
    assert_eq!(incomplete.stops().len(), K6_FULL_RANK_WAVE_WIDTHS[0]);
    assert!(incomplete.predecessor().same_authority_as(&root));
    assert_eq!(incomplete.progress().len(), 1);
    let wave = &incomplete.progress()[0];
    assert_eq!(wave.wave_ordinal(), 0);
    assert_eq!(wave.active_count(), 3);
    assert_eq!(wave.state(), K6WaveCampaignState::Incomplete);
    assert_eq!(wave.orbits().len(), K6_FULL_RANK_WAVE_WIDTHS[0]);
    assert!(wave.orbits().iter().enumerate().all(|(ordinal, orbit)| {
        orbit.orbit_ordinal() == ordinal
            && orbit.active_count() == 3
            && orbit.state() == K6OrbitCampaignState::OperationallyBounded
            && orbit.ledger_revision() == 1
            && orbit.owner_count() == 1
            && orbit.task_reports() == 1
    }));
    for (local_ordinal, stop) in incomplete.stops().iter().enumerate() {
        assert_eq!(stop.orbit_ordinal(), local_ordinal);
        assert!(
            stop.ledger()
                .predecessor_snapshot()
                .same_authority_as(&root)
        );
        assert_eq!(stop.ledger().revision().get(), 1);
        assert!(matches!(
            stop.terminal_stop(),
            ProbeCoordinatorStop::OperationallyBounded(bound)
                if matches!(
                    bound.reason(),
                    ProbeCoordinatorOperationalReason::TaskReportLimit {
                        requested: 2,
                        limit: 1,
                    }
                )
        ));
    }
    assert_eq!(root.closed_layer_count(), 0);
}

#[test]
fn public_wave_runner_uses_the_configured_persisted_order_for_every_sibling() {
    let priority =
        crate::sector::CoordinatePriority::try_new(6, &[5, 3, 4, 2, 0, 1], Default::default())
            .unwrap();
    let ordering = OrderingPolicy::try_with_coordinate_priority(&priority).unwrap();
    let hints = FoundryCampaignExternalHints::try_new(
        FoundryCampaignItinerary::FullRankAtomicWaves,
        [FoundryCampaignProbe::new(1_000_000_007, [37], [0; 6])],
        2,
        0,
        ordering,
        None,
    )
    .unwrap();
    let config = FoundryCampaignConfig::try_external_hints(
        FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
        hints,
        1,
        1,
    )
    .unwrap();
    let K6WaveCampaignOutcome::Incomplete(incomplete) =
        run_k6_full_rank_wave_campaign(&config, 1).unwrap()
    else {
        panic!("one task per sibling cannot publish all K6 waves")
    };
    assert_eq!(incomplete.wave_ordinal(), 0);
    assert_eq!(incomplete.stops().len(), K6_FULL_RANK_WAVE_WIDTHS[0]);
    assert!(
        incomplete
            .stops()
            .iter()
            .all(|stop| stop.ledger().ordering() == ordering)
    );
}

#[test]
fn full_rank_wave_driver_rejects_a_structurally_compatible_foreign_root() {
    let root = super::super::preset_k6::shared_k6_root_predecessor().unwrap();
    let foreign = ImmutableOwnerSnapshot::try_from_terminal_authority(
        fresh_k6_terminal_authority_for_test().unwrap(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(foreign, root);
    assert!(!foreign.same_authority_as(&root));
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 1).unwrap();
    assert!(matches!(
        try_run_k6_full_rank_waves(&config, foreign, StagedSectorClosureLimits::default(), 1,),
        Err(K6WaveCampaignError::Invariant {
            detail: "K6 full-rank waves did not start from the exact installed root authority",
        })
    ));
}

#[test]
fn bounded_wave_results_are_identical_for_one_and_two_workers() {
    if ParallelExecution::preflight_requested_core_budget(2).is_err() {
        return;
    }
    let root = super::super::preset_k6::shared_k6_root_predecessor().unwrap();
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 1).unwrap();
    let serial = try_run_k6_full_rank_waves(
        &config,
        root.clone(),
        StagedSectorClosureLimits::default(),
        1,
    )
    .unwrap();
    let parallel = try_run_k6_full_rank_waves(
        &config,
        root.clone(),
        StagedSectorClosureLimits::default(),
        2,
    )
    .unwrap();
    assert_same_bounded_outcome(serial, parallel, &root);
}

#[test]
fn live_wave_progress_is_monotone_orbit_ordered_and_matches_the_final_outcome() {
    if ParallelExecution::preflight_requested_core_budget(2).is_err() {
        return;
    }
    let root = super::super::preset_k6::shared_k6_root_predecessor().unwrap();
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(4, 1).unwrap();
    let mut events = Vec::new();
    let outcome = try_run_k6_full_rank_waves_with_progress(
        &config,
        root,
        StagedSectorClosureLimits::default(),
        2,
        &mut |event| events.push(event),
    )
    .unwrap();
    let K6WaveCampaignOutcome::Incomplete(incomplete) = outcome else {
        panic!("four tasks per sibling unexpectedly closed the first K6 wave")
    };
    assert!(!events.is_empty());
    let mut previous_revision = [0_u64; K6_FULL_RANK_WAVE_WIDTHS[0]];
    let mut previous_reports = [0_usize; K6_FULL_RANK_WAVE_WIDTHS[0]];
    for event in &events {
        assert_eq!(event.wave_ordinal(), 0);
        assert_eq!(event.active_count(), 3);
        assert_eq!(event.orbits().len(), K6_FULL_RANK_WAVE_WIDTHS[0]);
        for (local_ordinal, orbit) in event.orbits().iter().enumerate() {
            assert_eq!(orbit.orbit_ordinal(), local_ordinal);
            assert_eq!(
                orbit.representative(),
                &FULL_RANK_ORBITS[local_ordinal].representative
            );
            assert!(orbit.ledger_revision() >= previous_revision[local_ordinal]);
            assert!(orbit.task_reports() >= previous_reports[local_ordinal]);
            previous_revision[local_ordinal] = orbit.ledger_revision();
            previous_reports[local_ordinal] = orbit.task_reports();
        }
    }
    assert_eq!(events.last(), incomplete.progress().last());
    assert_eq!(
        events.last().unwrap().state(),
        K6WaveCampaignState::Incomplete
    );
}

#[test]
fn latest_value_progress_never_backpressures_and_retains_only_the_newest_revision() {
    let (latest, receiver) = LatestK6WaveProgress::try_new(0, 3, 0, 2).unwrap();
    const UPDATES: u64 = 100_000;
    std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            for revision in 1..=UPDATES {
                latest.publish_scalars(
                    0,
                    K6OrbitProgressScalars {
                        state: K6OrbitCampaignState::Running,
                        ledger_revision: revision,
                        owner_count: revision as usize,
                        uncovered_box_count: UPDATES.saturating_sub(revision) as usize,
                        task_reports: revision as usize,
                    },
                );
            }
        });
        writer.join().unwrap();
    });
    assert!(receiver.try_recv().is_ok());
    assert!(matches!(
        receiver.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    let snapshot = latest.try_snapshot(K6WaveCampaignState::Running).unwrap();
    assert_eq!(snapshot.orbits()[0].ledger_revision(), UPDATES);
    assert_eq!(snapshot.orbits()[0].owner_count(), UPDATES as usize);
    assert_eq!(snapshot.orbits()[0].task_reports(), UPDATES as usize);
    assert_eq!(snapshot.orbits()[1].state(), K6OrbitCampaignState::Pending);
}

#[test]
fn published_progress_promotes_every_closed_sibling_atomically() {
    let (latest, _receiver) = LatestK6WaveProgress::try_new(0, 3, 0, 2).unwrap();
    for local_ordinal in 0..2 {
        latest.publish_scalars(
            local_ordinal,
            K6OrbitProgressScalars {
                state: K6OrbitCampaignState::ClosedUnpublished,
                ledger_revision: 7,
                owner_count: 7,
                uncovered_box_count: 0,
                task_reports: 7,
            },
        );
    }
    let published = finalize_wave_progress(
        latest.try_snapshot(K6WaveCampaignState::Running).unwrap(),
        K6WaveCampaignState::Published,
    )
    .unwrap();
    assert_eq!(published.state(), K6WaveCampaignState::Published);
    assert!(published.orbits().iter().all(|orbit| {
        orbit.state() == K6OrbitCampaignState::Published && orbit.ledger_revision() == 7
    }));
}

#[test]
fn artifact_wave_extraction_strips_nonempty_detached_search_progress() {
    let (latest, _receiver) = LatestK6WaveProgress::try_new(0, 3, 0, 2).unwrap();
    let running = latest.try_snapshot(K6WaveCampaignState::Running).unwrap();
    let published = latest.try_snapshot(K6WaveCampaignState::Published).unwrap();
    assert_ne!(running, published);

    // Materialize the same nonempty, proof-bearing wave twice from autonomous
    // RustRed evidence.  The one-point carrier is discharged by its exact
    // corner terminal; retaining an ordinary-source owner forces the ledger
    // through the real compiler/seal/publication path without importing any
    // external reduction algebra or claiming positive-dimensional closure.
    let fixture = OracleDisabledK6Fixture::shared();
    let seed = fixture.new_ledger();
    let plan = fixture.plan(&seed, 2, 0);
    let owner = fixture.replay_owner(&plan.tasks()[0]);
    let make_wave = || {
        let arity = fixture.sector().arity();
        let mut ledger = CanonicalExactOwnerLedger::try_new_with_closure_carrier(
            fixture.generator().context(),
            fixture.predecessor().clone(),
            fixture.sector().clone(),
            OrderingPolicy::default(),
            [IntegralKey::try_new(fixture.sector().corner_indices()).unwrap()],
            LatticeBox::try_new(
                std::iter::repeat_n(0_u64, arity),
                std::iter::repeat_n(Some(0_u64), arity),
            )
            .unwrap(),
            ExactOwnerCoverDeltaLimits::default(),
        )
        .unwrap();
        let delta = ledger.try_apply_owner(owner.clone()).unwrap();
        assert!(delta.updated().status().is_compiler_closed());
        let sealed = ledger.try_into_closed_cover().unwrap();
        try_publish_sealed_sector_wave(
            fixture.predecessor().clone(),
            vec![sealed],
            StagedSectorClosureLimits::default(),
        )
        .unwrap()
    };

    let first = K6PublishedSectorWaves {
        waves: vec![make_wave()].into_boxed_slice(),
        progress: vec![running].into_boxed_slice(),
    }
    .into_artifact_waves();
    let second = K6PublishedSectorWaves {
        waves: vec![make_wave()].into_boxed_slice(),
        progress: vec![published].into_boxed_slice(),
    }
    .into_artifact_waves();
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(first[0].layers().len(), 1);
    assert_eq!(second[0].layers().len(), 1);
    assert!(
        first[0]
            .predecessor()
            .same_authority_as(second[0].predecessor())
    );
    let first_layer = &first[0].layers()[0];
    let second_layer = &second[0].layers()[0];
    assert_eq!(first_layer.content_id(), second_layer.content_id());
    assert_eq!(first_layer.proven_domain(), second_layer.proven_domain());
    assert_eq!(first_layer.sector(), second_layer.sector());
}

#[test]
fn zero_sibling_worker_count_is_rejected_before_campaign_execution() {
    let root = super::super::preset_k6::shared_k6_root_predecessor().unwrap();
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 1).unwrap();
    assert!(matches!(
        try_run_k6_full_rank_waves(&config, root, StagedSectorClosureLimits::default(), 0,),
        Err(K6WaveCampaignError::ParallelExecution(
            ParallelExecutionError::ZeroCoreBudget
        ))
    ));
}

fn assert_same_bounded_outcome(
    left: K6WaveCampaignOutcome,
    right: K6WaveCampaignOutcome,
    root: &ImmutableOwnerSnapshot,
) {
    let K6WaveCampaignOutcome::Incomplete(left) = left else {
        panic!("the serial bounded campaign unexpectedly published a wave")
    };
    let K6WaveCampaignOutcome::Incomplete(right) = right else {
        panic!("the parallel bounded campaign unexpectedly published a wave")
    };
    assert_eq!(left.wave_ordinal(), right.wave_ordinal());
    assert_eq!(left.active_count(), right.active_count());
    assert_eq!(left.closed_sector_count(), right.closed_sector_count());
    assert_eq!(left.stops().len(), right.stops().len());
    assert_eq!(left.progress(), right.progress());
    assert!(left.predecessor().same_authority_as(root));
    assert!(right.predecessor().same_authority_as(root));
    assert_eq!(left.predecessor().id(), right.predecessor().id());

    for (left, right) in left.stops().iter().zip(right.stops()) {
        assert_eq!(left.orbit_ordinal(), right.orbit_ordinal());
        assert_eq!(left.ledger().sector(), right.ledger().sector());
        assert_eq!(
            left.ledger().closure_carrier(),
            right.ledger().closure_carrier()
        );
        assert_eq!(left.ledger().snapshot(), right.ledger().snapshot());
        assert_eq!(left.ledger().terminals(), right.ledger().terminals());
        assert_eq!(
            left.ledger()
                .owners()
                .iter()
                .map(|owner| owner.content_order_key())
                .collect::<Vec<_>>(),
            right
                .ledger()
                .owners()
                .iter()
                .map(|owner| owner.content_order_key())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            left.terminal_stop().census(),
            right.terminal_stop().census()
        );
        match (left.terminal_stop(), right.terminal_stop()) {
            (
                ProbeCoordinatorStop::OperationallyBounded(left),
                ProbeCoordinatorStop::OperationallyBounded(right),
            ) => assert_eq!(left, right),
            _ => panic!("serial and parallel bounded stops changed kind"),
        }
    }
}
