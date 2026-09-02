use super::*;
use crate::family::IntegralKey;
use crate::foundry::artifact::fresh_k6_terminal_authority_for_test;
use crate::foundry::campaign::{
    FoundryCampaignConfig, FoundryCampaignError, FoundryCampaignExternalHints,
    FoundryCampaignOperationalLimit, FoundryCampaignPreset, FoundryCampaignProbe,
    FoundryCampaignSetupStage, FoundryCampaignStop,
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
            assert!(
                !ledger.snapshot().status().is_compiler_closed(),
                "exact product preimages deliberately leave coupled endpoint fringes"
            );
            assert_eq!(ledger.terminals().len(), 1);
            assert_eq!(ledger.terminals()[0].powers(), orbit.representative);
            assert_eq!(ledger.sector().active_count(), wave_ordinal + 3);
        }
        orbit_start += wave_width;
    }
    assert_eq!(orbit_start, FULL_RANK_ORBITS.len());
}

#[test]
fn exact_product_preimages_leave_the_coupled_first_wave_fringe_for_discovery() {
    let root = super::super::preset_k6::shared_k6_root_predecessor().unwrap();
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 1).unwrap();
    let K6WaveCampaignOutcome::Incomplete(incomplete) = try_run_k6_full_rank_waves(
        &config,
        root.clone(),
        StagedSectorClosureLimits::default(),
        1,
    )
    .unwrap() else {
        panic!("one task per unresolved sector cannot close every K6 wave")
    };
    assert_eq!(incomplete.wave_ordinal(), 0);
    assert_eq!(incomplete.active_count(), 3);
    assert_eq!(incomplete.closed_sector_count(), 0);
    assert_eq!(incomplete.stops().len(), K6_FULL_RANK_WAVE_WIDTHS[0]);
    assert_eq!(
        incomplete.incomplete_orbits().len(),
        K6_FULL_RANK_WAVE_WIDTHS[0]
    );
    assert_eq!(incomplete.predecessor().closed_layer_count(), 0);
    assert_eq!(incomplete.progress().len(), 1);
    let wave = &incomplete.progress()[0];
    assert_eq!(wave.wave_ordinal(), 0);
    assert_eq!(wave.active_count(), 3);
    assert_eq!(wave.state(), K6WaveCampaignState::Incomplete);
    assert_eq!(wave.orbits().len(), K6_FULL_RANK_WAVE_WIDTHS[0]);
    for (expected_ordinal, orbit) in wave.orbits().iter().enumerate() {
        assert_eq!(orbit.orbit_ordinal(), expected_ordinal);
        assert_eq!(orbit.active_count(), 3);
        assert_eq!(orbit.state(), K6OrbitCampaignState::OperationallyBounded);
        assert_eq!(orbit.ledger_revision(), 0);
        assert_eq!(orbit.owner_count(), 0);
        assert_eq!(orbit.task_reports(), 1);
    }
    for (expected_ordinal, stop) in incomplete.stops().iter().enumerate() {
        assert_eq!(stop.orbit_ordinal(), expected_ordinal);
        assert!(
            stop.ledger()
                .predecessor_snapshot()
                .same_authority_as(incomplete.predecessor())
        );
        assert_eq!(stop.ledger().revision().get(), 0);
        assert!(stop.ledger().owners().is_empty());
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
    let residual = &incomplete.incomplete_orbits()[0];
    assert_eq!(residual.orbit_ordinal(), 0);
    assert_eq!(
        residual.report().total_uncovered_box_count(),
        incomplete.stops()[0]
            .ledger()
            .snapshot()
            .uncovered_box_count()
    );
    assert_eq!(residual.report().reported_uncovered_box_count(), 1);
    assert_eq!(
        residual.report().census().task_reports(),
        incomplete.stops()[0].final_census().task_reports()
    );
    assert!(
        incomplete.stops()[0].final_census().task_reports()
            >= incomplete.stops()[0]
                .terminal_stop()
                .census()
                .task_reports()
    );
    assert_eq!(
        residual.report().uncovered_boxes_truncated(),
        residual.report().total_uncovered_box_count() > 1
    );
    assert!(matches!(
        residual.report().stop(),
        FoundryCampaignStop::OperationallyBounded {
            limit: FoundryCampaignOperationalLimit::TaskReport {
                requested: 2,
                limit: 1,
            },
            ..
        }
    ));

    let partition = incomplete.stops()[0]
        .ledger()
        .try_clone_uncovered_partition()
        .unwrap();
    let exact_box = &partition.boxes()[0];
    let reported_box = &residual.report().uncovered_boxes()[0];
    assert_eq!(reported_box.lower(), exact_box.lower());
    assert_eq!(reported_box.upper(), exact_box.upper());
    assert_eq!(reported_box.free_dimension(), exact_box.free_dimension());

    let zero_cap = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 0).unwrap();
    let zero_report = detach_report(
        &zero_cap,
        incomplete.stops()[0].ledger(),
        incomplete.stops()[0].terminal_stop(),
        incomplete.stops()[0].final_census(),
    )
    .unwrap();
    assert_eq!(zero_report.total_uncovered_box_count(), 1);
    assert_eq!(zero_report.reported_uncovered_box_count(), 0);
    assert!(zero_report.uncovered_boxes_truncated());
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
        [FoundryCampaignProbe::try_new(1_000_000_007, [37], [0; 6]).unwrap()],
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
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 1)
        .unwrap()
        .try_resolve_search_program()
        .unwrap();
    let root =
        super::super::preset_k6::k6_root_predecessor_for_ordering(config.ordering()).unwrap();
    let mut discard_progress = |_: K6WaveCampaignProgress| {};
    let serial = try_run_k6_full_rank_waves_with_progress_against_root(
        &config,
        root.clone(),
        &root,
        StagedSectorClosureLimits::default(),
        1,
        &mut discard_progress,
    )
    .unwrap();
    let parallel = try_run_k6_full_rank_waves_with_progress_against_root(
        &config,
        root.clone(),
        &root,
        StagedSectorClosureLimits::default(),
        2,
        &mut discard_progress,
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
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 1).unwrap();
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
        panic!("one task per sibling unexpectedly closed every K6 wave")
    };
    assert!(!events.is_empty());
    let mut previous_revision = [0_u64; FULL_RANK_ORBITS.len()];
    let mut previous_reports = [0_usize; FULL_RANK_ORBITS.len()];
    for event in &events {
        let wave_ordinal = event.wave_ordinal();
        assert!(wave_ordinal <= incomplete.wave_ordinal());
        let orbit_start = K6_FULL_RANK_WAVE_WIDTHS[..wave_ordinal]
            .iter()
            .sum::<usize>();
        assert_eq!(event.active_count(), wave_ordinal + 3);
        assert_eq!(event.orbits().len(), K6_FULL_RANK_WAVE_WIDTHS[wave_ordinal]);
        for (local_ordinal, orbit) in event.orbits().iter().enumerate() {
            let orbit_ordinal = orbit_start + local_ordinal;
            assert_eq!(orbit.orbit_ordinal(), orbit_ordinal);
            assert_eq!(
                orbit.representative(),
                &FULL_RANK_ORBITS[orbit_ordinal].representative
            );
            assert!(orbit.ledger_revision() >= previous_revision[orbit_ordinal]);
            assert!(orbit.task_reports() >= previous_reports[orbit_ordinal]);
            previous_revision[orbit_ordinal] = orbit.ledger_revision();
            previous_reports[orbit_ordinal] = orbit.task_reports();
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

    // Materialize the same proof-bearing predecessor-closed wave twice. The
    // factorization program, rather than an ordinary rule cell or a declared
    // master, owns this exact one-point carrier.
    let fixture = OracleDisabledK6Fixture::shared();
    let make_wave = || {
        let arity = fixture.sector().arity();
        let ledger = CanonicalExactOwnerLedger::try_new_with_closure_carrier(
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
        assert!(ledger.snapshot().status().is_compiler_closed());
        assert!(ledger.owners().is_empty());
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

#[test]
fn public_error_kind_preserves_nested_campaign_resource_and_invariant_categories() {
    for nested in [
        FoundryCampaignError::ResourceCountOverflow {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource: "requested domains",
        },
        FoundryCampaignError::ResourceLimit {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource: "requested domains",
            requested: 2,
            limit: 1,
        },
    ] {
        let public = K6WaveCampaignRunError(K6WaveCampaignError::Campaign(nested));
        assert_eq!(public.kind(), K6WaveCampaignErrorKind::ResourceLimit);
    }

    let invariant = K6WaveCampaignRunError(K6WaveCampaignError::Campaign(
        FoundryCampaignError::Invariant {
            detail: "nested campaign invariant",
        },
    ));
    assert_eq!(invariant.kind(), K6WaveCampaignErrorKind::Invariant);

    let execution = K6WaveCampaignRunError(K6WaveCampaignError::Campaign(
        FoundryCampaignError::Execution {
            message: "campaign execution".to_owned(),
        },
    ));
    assert_eq!(execution.kind(), K6WaveCampaignErrorKind::Campaign);
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
    assert_eq!(left.incomplete_orbits(), right.incomplete_orbits());
    assert_eq!(left.progress(), right.progress());
    assert_eq!(root.closed_layer_count(), 0);
    assert_eq!(left.predecessor().closed_layer_count(), 0);
    assert!(left.predecessor().same_authority_as(root));
    assert!(right.predecessor().same_authority_as(root));
    assert!(left.predecessor().same_authority_as(right.predecessor()));
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
