use super::preset_k6::{
    k6_root_predecessor_for_ordering, shared_k6_algebra_inputs, shared_k6_root_predecessor,
    try_new_k6_full_rank_ledger, try_new_k6_full_rank_ledger_with_profile_and_ordering,
};
use super::*;
use crate::family::IntegralKey;
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
        [FoundryCampaignProbe::try_new(1_000_000_007, [37], [0; 6]).unwrap()],
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
    let probe = FoundryCampaignProbe::try_new(1_000_000_007, [37], [0; 6]).unwrap();
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
    let composite = FoundryCampaignProbe::try_new(9, [37], [0; 6]).unwrap();
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
fn probe_ingress_rejects_adversarial_iterators_at_the_public_limit() {
    use std::cell::Cell;

    let consumed_coordinates = Cell::new(0usize);
    let unbounded_coordinates = std::iter::from_fn(|| {
        consumed_coordinates.set(consumed_coordinates.get() + 1);
        Some(0_i64)
    });
    assert_eq!(
        FoundryCampaignProbe::try_new(1_000_000_007, unbounded_coordinates, []).unwrap_err(),
        FoundryCampaignConfigError::TooManyProbeCoordinates {
            requested: MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATES + 1,
            limit: MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATES,
        }
    );
    assert_eq!(
        consumed_coordinates.get(),
        MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATES + 1,
        "the fallible constructor must not drain an adversarial iterator"
    );
    let boundary_probe = FoundryCampaignProbe::try_new(
        1_000_000_007,
        std::iter::repeat_n(37, MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATES - 1),
        [0],
    )
    .unwrap();
    assert_eq!(
        boundary_probe.base_parameters().len() + boundary_probe.chart_offsets().len(),
        MAX_FOUNDRY_CAMPAIGN_PROBE_COORDINATES
    );

    let small = FoundryCampaignProbe::try_new(1_000_000_007, [37], [0; 6]).unwrap();
    let consumed_probes = Cell::new(0usize);
    let unbounded_probes = std::iter::from_fn(|| {
        consumed_probes.set(consumed_probes.get() + 1);
        Some(small.clone())
    });
    assert_eq!(
        FoundryCampaignExternalHints::try_new(
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            unbounded_probes,
            2,
            0,
            OrderingPolicy::default(),
            None,
        )
        .unwrap_err(),
        FoundryCampaignConfigError::TooManyProbes {
            requested: MAX_FOUNDRY_CAMPAIGN_PROBES + 1,
            limit: MAX_FOUNDRY_CAMPAIGN_PROBES,
        }
    );
    assert_eq!(consumed_probes.get(), MAX_FOUNDRY_CAMPAIGN_PROBES + 1);
}

#[test]
fn external_requested_domains_are_structural_bounded_and_arity_checked() {
    let anchor = IntegralKey::try_new([1, 1, 1, 0, -1, 2]).unwrap();
    let domain = FoundryCampaignDomainHint::try_new(anchor.clone(), [0, 2, 5]).unwrap();
    assert_eq!(domain.anchor(), &anchor);
    assert_eq!(domain.symbolic_axes(), [0, 2, 5]);

    assert_eq!(
        FoundryCampaignDomainHint::try_new(anchor.clone(), [0, 2, 2]).unwrap_err(),
        FoundryCampaignConfigError::DomainHintAxesNotStrictlyIncreasing {
            previous: 2,
            current: 2,
        }
    );
    assert_eq!(
        FoundryCampaignDomainHint::try_new(anchor.clone(), [2, 1]).unwrap_err(),
        FoundryCampaignConfigError::DomainHintAxesNotStrictlyIncreasing {
            previous: 2,
            current: 1,
        }
    );
    assert_eq!(
        FoundryCampaignDomainHint::try_new(anchor, [6]).unwrap_err(),
        FoundryCampaignConfigError::DomainHintAxisOutOfBounds { axis: 6, arity: 6 }
    );

    let wrong_arity =
        FoundryCampaignDomainHint::try_new(IntegralKey::try_new([1, 1, 1, 1, 1]).unwrap(), [0, 4])
            .unwrap();
    let hints = FoundryCampaignExternalHints::try_new_with_domains(
        FoundryCampaignItinerary::SingleSectorFixedPoint,
        [FoundryCampaignProbe::try_new(1_000_000_007, [37], [0; 6]).unwrap()],
        2,
        0,
        OrderingPolicy::default(),
        None,
        [wrong_arity],
    )
    .unwrap();
    assert_eq!(
        FoundryCampaignConfig::try_external_hints(
            FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
            hints,
            1,
            1,
        )
        .unwrap_err(),
        FoundryCampaignConfigError::WrongDomainHintAnchorArity {
            domain_ordinal: 0,
            expected: 6,
            actual: 5,
        }
    );
}

#[test]
fn external_requested_domain_retention_has_cold_resource_limits() {
    let oversized_anchor =
        IntegralKey::try_new(vec![0; MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINT_ARITY + 1]).unwrap();
    assert_eq!(
        FoundryCampaignDomainHint::try_new(oversized_anchor, []).unwrap_err(),
        FoundryCampaignConfigError::DomainHintArityLimit {
            actual: MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINT_ARITY + 1,
            limit: MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINT_ARITY,
        }
    );

    let domain =
        FoundryCampaignDomainHint::try_new(IntegralKey::try_new([1, 1, 1, 1, 1, 1]).unwrap(), [0])
            .unwrap();
    let too_many = (0..=MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINTS).map(|_| domain.clone());
    assert_eq!(
        FoundryCampaignExternalHints::try_new_with_domains(
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            [FoundryCampaignProbe::try_new(1_000_000_007, [37], [0; 6]).unwrap()],
            2,
            0,
            OrderingPolicy::default(),
            None,
            too_many,
        )
        .unwrap_err(),
        FoundryCampaignConfigError::TooManyDomainHints {
            requested: MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINTS + 1,
            limit: MAX_FOUNDRY_CAMPAIGN_DOMAIN_HINTS,
        }
    );
}

#[test]
fn autonomous_core_configuration_rejects_external_domains() {
    let domain = FoundryCampaignDomainHint::try_new(
        IntegralKey::try_new([1, 1, 1, 1, 1, 1]).unwrap(),
        [0, 1],
    )
    .unwrap();
    assert_eq!(
        FoundryCampaignConfig::try_build(
            FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            FoundrySearchProvenance::Autonomous,
            [FoundryCampaignProbe::try_new(1_000_000_007, [37], [0; 6]).unwrap()],
            2,
            0,
            OrderingPolicy::default(),
            None,
            [domain],
            1,
            1,
        )
        .unwrap_err(),
        FoundryCampaignConfigError::AutonomousDomainHints
    );
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
    let autonomous = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 1)
        .unwrap()
        .try_resolve_search_program()
        .unwrap();
    let identical_hints = FoundryCampaignExternalHints::try_new(
        FoundryCampaignItinerary::SingleSectorFixedPoint,
        autonomous.probes().iter().cloned(),
        autonomous.interior_margin(),
        autonomous.polynomial_degree_ceiling(),
        autonomous.ordering(),
        Some(autonomous.discovery_coordinate_priority().clone()),
    )
    .unwrap();
    let informed = FoundryCampaignConfig::try_external_hints(
        FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
        identical_hints,
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
    assert_eq!(
        report.snapshot().coverage(),
        FoundryCampaignCoverageStatus::OwnerFree
    );
    assert_eq!(report.snapshot().terminal_count(), 1);
    assert!(matches!(
        report.stop(),
        FoundryCampaignStop::OperationallyBounded {
            limit: FoundryCampaignOperationalLimit::TaskReport {
                requested: 2,
                limit: 1,
            },
            ..
        }
    ));
}

#[test]
fn k6_orbit_zero_campaign_returns_deterministic_exact_fringe_state() {
    let config = FoundryCampaignConfig::try_three_loop_unit_mass_vacuum_k6_orbit_0(1, 2).unwrap();
    assert_eq!(config.schema(), FOUNDRY_CAMPAIGN_CONFIG_SCHEMA);
    let first = run_foundry_campaign(&config).unwrap().into_report();
    let second = run_foundry_campaign(&config).unwrap().into_report();
    assert_eq!(second, first);
    assert_eq!(first.schema(), FOUNDRY_CAMPAIGN_REPORT_SCHEMA);
    assert_eq!(first.preset(), config.preset());
    // This preset continues to mean orbit 0; it must not silently retarget the
    // first unresolved orbit. Exact product authority owns its certified
    // sparse preimage while ordinary discovery retains the coupled endpoint
    // fringe rather than accepting the former over-broad rectangular hull.
    assert_eq!(first.census().task_reports(), 1);
    assert_eq!(first.census().declared_probes(), 6);
    assert_eq!(first.snapshot().revision(), 0);
    assert_eq!(first.snapshot().owner_count(), 0);
    assert_eq!(first.snapshot().terminal_count(), 1);
    assert_eq!(
        first.snapshot().coverage(),
        FoundryCampaignCoverageStatus::OwnerFree
    );
    assert!(matches!(
        first.stop(),
        FoundryCampaignStop::OperationallyBounded {
            limit: FoundryCampaignOperationalLimit::TaskReport {
                requested: 2,
                limit: 1,
            },
            ..
        }
    ));
    assert_eq!(first.total_uncovered_box_count(), 1);
    assert_eq!(first.reported_uncovered_box_count(), 1);
    assert!(!first.uncovered_boxes_truncated());
    assert_eq!(first.uncovered_boxes()[0].lower(), [0; 6]);
    assert_eq!(first.uncovered_boxes()[0].upper(), [None; 6]);
    assert_eq!(first.uncovered_boxes()[0].free_dimension(), 6);
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
        assert!(!ledger.snapshot().status().is_compiler_closed());
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
fn malformed_probe_shape_fails_at_the_public_setup_boundary() {
    let hints = FoundryCampaignExternalHints::try_new(
        FoundryCampaignItinerary::SingleSectorFixedPoint,
        [FoundryCampaignProbe::try_new(1_000_000_007, [37], [0; 5]).unwrap()],
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
