use super::preset_k6::{
    k6_root_predecessor_for_ordering, shared_k6_algebra_inputs, shared_k6_root_predecessor,
    try_new_k6_full_rank_ledger, try_new_k6_full_rank_ledger_with_profile_and_ordering,
};
use super::*;
use crate::foundry::artifact::FULL_RANK_ORBITS;
use crate::foundry::completion::SectorChart;
use crate::foundry::completion::source_discovery::ProbeCampaignLimits;
use crate::sector::{CoordinatePriority, OrderingPolicy};

fn winner_ordering() -> OrderingPolicy {
    let priority = CoordinatePriority::try_new(6, &[5, 3, 4, 2, 0, 1], Default::default())
        .expect("winner coordinate priority");
    OrderingPolicy::try_with_coordinate_priority(&priority).expect("winner ordering")
}

fn external_hints(
    itinerary: FoundryCampaignItinerary,
    ordering: OrderingPolicy,
) -> FoundryCampaignExternalHints {
    FoundryCampaignExternalHints::try_new(
        itinerary,
        [FoundryCampaignProbe::new(1_000_000_007, [37], [0; 6])],
        2,
        0,
        ordering,
        None,
    )
    .unwrap()
}

#[test]
fn preset_id_and_config_validation_are_stable() {
    let preset = FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0;
    assert_eq!(
        FoundryCampaignPreset::from_stable_id(preset.stable_id()),
        Some(preset)
    );
    assert_eq!(FoundryCampaignPreset::from_stable_id("unknown"), None);
    assert_eq!(
        FoundryCampaignExternalHints::try_new(
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            [],
            2,
            0,
            OrderingPolicy::default(),
            None,
        )
        .unwrap_err(),
        FoundryCampaignConfigError::EmptyProbeProgram
    );
    let probe = FoundryCampaignProbe::new(1_000_000_007, [37], [0; 6]);
    assert_eq!(
        FoundryCampaignExternalHints::try_new(
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            [probe.clone()],
            0,
            0,
            OrderingPolicy::default(),
            None,
        )
        .unwrap_err(),
        FoundryCampaignConfigError::ZeroInteriorMargin
    );
    assert_eq!(
        FoundryCampaignConfig::try_autonomous_single_sector(preset, 0, 1).unwrap_err(),
        FoundryCampaignConfigError::ZeroTaskReportLimit
    );
    let composite = FoundryCampaignProbe::new(9, [37], [0; 6]);
    let hints = FoundryCampaignExternalHints::try_new(
        FoundryCampaignItinerary::SingleSectorFixedPoint,
        [composite],
        2,
        0,
        OrderingPolicy::default(),
        None,
    )
    .unwrap();
    assert!(matches!(
        FoundryCampaignConfig::try_external_hints(preset, hints, 1, 1),
        Err(FoundryCampaignConfigError::InvalidProbe {
            probe_ordinal: 0,
            ..
        })
    ));
}

#[test]
fn proof_ordering_is_arity_checked_and_becomes_the_default_discovery_order() {
    let config = FoundryCampaignConfig::try_external_hints(
        FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
        external_hints(
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            winner_ordering(),
        ),
        1,
        1,
    )
    .unwrap();
    assert_eq!(config.ordering(), winner_ordering());
    assert_eq!(
        config.discovery_coordinate_priority().rank_by_slot(),
        [5, 3, 4, 2, 0, 1]
    );

    let wrong = CoordinatePriority::try_new(5, &[4, 3, 2, 1, 0], Default::default()).unwrap();
    let wrong = OrderingPolicy::try_with_coordinate_priority(&wrong).unwrap();
    assert_eq!(
        FoundryCampaignConfig::try_external_hints(
            FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
            external_hints(FoundryCampaignItinerary::SingleSectorFixedPoint, wrong),
            1,
            1,
        )
        .unwrap_err(),
        FoundryCampaignConfigError::WrongOrderingPolicyArity {
            expected: 6,
            actual: 5,
        }
    );
}

#[test]
fn search_provenance_is_non_authoritative_for_identical_typed_inputs() {
    let autonomous =
        FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 1).unwrap();
    let informed = FoundryCampaignConfig::try_external_hints(
        FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
        external_hints(
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            OrderingPolicy::default(),
        ),
        1,
        1,
    )
    .unwrap();
    assert_eq!(
        autonomous.search_provenance(),
        FoundrySearchProvenance::Autonomous
    );
    assert_eq!(
        informed.search_provenance(),
        FoundrySearchProvenance::ExternalHintsOnly
    );
    assert_eq!(
        run_foundry_campaign(&autonomous).unwrap().into_report(),
        run_foundry_campaign(&informed).unwrap().into_report(),
        "audit-only provenance must not enter exact campaign semantics"
    );
}

#[test]
fn campaign_ledger_rejects_a_predecessor_installed_under_another_ordering() {
    let inputs = shared_k6_algebra_inputs().unwrap();
    let natural = shared_k6_root_predecessor().unwrap();
    let error = try_new_k6_full_rank_ledger_with_profile_and_ordering(
        inputs,
        FULL_RANK_ORBITS[0].representative,
        natural,
        winner_ordering(),
        super::k6_resource::K6CampaignResourceProfile::try_for_task_report_ceiling(1).unwrap(),
        ProbeCampaignLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.setup_stage(), Some(FoundryCampaignSetupStage::Ledger));

    let ordered = k6_root_predecessor_for_ordering(winner_ordering()).unwrap();
    let ledger = try_new_k6_full_rank_ledger_with_profile_and_ordering(
        inputs,
        FULL_RANK_ORBITS[0].representative,
        ordered,
        winner_ordering(),
        super::k6_resource::K6CampaignResourceProfile::try_for_task_report_ceiling(1).unwrap(),
        ProbeCampaignLimits::default(),
    )
    .unwrap();
    assert_eq!(ledger.ordering(), winner_ordering());
}

#[test]
fn public_orbit_campaign_runs_with_one_coherent_persisted_ordering() {
    let config = FoundryCampaignConfig::try_external_hints(
        FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
        external_hints(
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            winner_ordering(),
        ),
        1,
        1,
    )
    .unwrap();
    let report = run_foundry_campaign(&config).unwrap().into_report();
    assert_eq!(report.ordering(), winner_ordering());
    assert_eq!(report.census().task_reports(), 1);
}

#[test]
fn k6_orbit_zero_campaign_starts_fresh_and_returns_deterministic_detached_state() {
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 2).unwrap();
    assert_eq!(config.schema(), FOUNDRY_CAMPAIGN_CONFIG_SCHEMA);
    let first = run_foundry_campaign(&config).unwrap().into_report();
    let second = run_foundry_campaign(&config).unwrap().into_report();
    assert_eq!(second, first);
    assert_eq!(first.schema(), FOUNDRY_CAMPAIGN_REPORT_SCHEMA);
    assert_eq!(first.preset(), config.preset());
    assert_eq!(first.census().task_reports(), 1);
    assert_eq!(first.census().declared_probes(), 1);
    assert_eq!(first.snapshot().revision(), 1);
    assert_eq!(first.snapshot().owner_count(), 1);
    assert_eq!(first.snapshot().terminal_count(), 1);
    assert_eq!(
        first.snapshot().coverage(),
        FoundryCampaignCoverageStatus::Incomplete(FoundryCampaignCoverageObstruction::NonFinite)
    );
    assert_eq!(
        first.stop(),
        FoundryCampaignStop::OperationallyBounded {
            location: Some(FoundryCampaignTaskLocation::new(1, 1, 4, 5, 1, 0)),
            limit: FoundryCampaignOperationalLimit::TaskReport {
                requested: 2,
                limit: 1,
            },
        }
    );
    // Carrier normalization keeps large faces symbolic while materializing
    // the thin endpoint tails that must not be sampled outside `IntegralKey`.
    assert_eq!(first.total_uncovered_box_count(), 6);
    assert_eq!(first.reported_uncovered_box_count(), 2);
    assert!(first.uncovered_boxes_truncated());
    assert!(
        first
            .uncovered_boxes()
            .iter()
            .all(|lattice_box| lattice_box.lower().len() == 6 && lattice_box.upper().len() == 6)
    );
}

#[test]
fn k6_ledgers_use_the_bounded_recursive_source_safe_carrier() {
    let inputs = shared_k6_algebra_inputs().unwrap();
    let predecessor = shared_k6_root_predecessor().unwrap();
    for orbit in FULL_RANK_ORBITS {
        let ledger =
            try_new_k6_full_rank_ledger(inputs, orbit.representative, predecessor.clone()).unwrap();
        let carrier = ledger.closure_carrier();
        let sector = ledger.sector();
        assert_eq!(carrier.lower(), [0; 6]);
        assert_eq!(ledger.terminals().len(), 1);
        assert_eq!(ledger.terminals()[0].powers(), orbit.representative);
        assert!(
            ledger
                .predecessor_snapshot()
                .same_authority_as(&predecessor)
        );
        let chart_carrier = SectorChart::new(sector.clone()).carrier_box().unwrap();
        for position in 0..sector.arity() {
            assert!(carrier.upper()[position].unwrap() < chart_carrier.upper()[position].unwrap());
        }
    }
}

#[test]
fn k6_campaign_four_task_smoke_has_only_strict_owner_shrinks() {
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(4, 4).unwrap();
    let mut progress = Vec::new();
    let report = run_foundry_campaign_with_progress(&config, |event| progress.push(event))
        .unwrap()
        .into_report();
    assert_eq!(report.census().task_reports(), 4);
    assert_eq!(report.census().strict_geometric_shrink(), 4);
    assert_eq!(report.census().scheduler_rejections(), 0);
    assert_eq!(report.census().scheduler_exact_lift_errors(), 0);
    assert_eq!(report.snapshot().revision(), 4);
    assert_eq!(report.snapshot().owner_count(), 4);
    assert_eq!(progress.len(), 4);
    for (ordinal, event) in progress.iter().enumerate() {
        let revision = (ordinal + 1) as u64;
        assert_eq!(event.revision(), revision);
        assert_eq!(event.snapshot().revision(), revision);
        assert_eq!(event.snapshot().owner_count(), ordinal + 1);
        assert_eq!(event.census().task_reports(), ordinal + 1);
        assert_eq!(event.census().strict_geometric_shrink(), ordinal + 1);
        assert_eq!(event.task_report_ceiling(), 4);
        assert_eq!(event.maximum_dimension(), 6);
        let location = event.location().expect("owner mutation task location");
        assert!(location.effective_dimension() <= location.parent_free_dimension());
        assert!(location.parent_free_dimension() <= event.maximum_dimension());
    }
    let last = progress.last().unwrap();
    assert_eq!(last.snapshot(), report.snapshot());
    assert_eq!(last.census().task_reports(), report.census().task_reports());
    assert_eq!(
        last.census().strict_geometric_shrink(),
        report.census().strict_geometric_shrink()
    );
    // The terminal budget check opens one final immutable planning epoch but
    // emits no progress event because it commits no owner mutation.
    assert_eq!(
        report.census().epochs_started(),
        last.census().epochs_started() + 1
    );
}

#[test]
fn malformed_probe_shape_fails_at_the_public_setup_boundary() {
    let hints = FoundryCampaignExternalHints::try_new(
        FoundryCampaignItinerary::SingleSectorFixedPoint,
        [FoundryCampaignProbe::new(1_000_000_007, [37], [0; 5])],
        2,
        0,
        OrderingPolicy::default(),
        None,
    )
    .unwrap();
    let config = FoundryCampaignConfig::try_external_hints(
        FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
        hints,
        1,
        1,
    )
    .unwrap_err();
    assert_eq!(
        config,
        FoundryCampaignConfigError::WrongProbeChartOffsetArity {
            probe_ordinal: 0,
            expected: 6,
            actual: 5,
        }
    );
}
