use super::*;
use crate::solver::{DomainPowerBounds, DomainPowerError};

fn constrained(
    reducer: &RoutedCandidateReducer<3>,
    source: [bool; 3],
    lower: [u64; 3],
    upper: [Option<u64>; 3],
    rank: Option<u32>,
    powers: DomainPowerBounds,
) -> (Vec<CandidateDomainRouteEvent<3>>, CandidateDomainRouteStats) {
    let mut events = Vec::new();
    let stats = reducer
        .visit_power_bounded_domain_route_overcover(
            source,
            &lower,
            &upper,
            rank,
            powers,
            Default::default(),
            &AtomicBool::new(false),
            |event| {
                events.push(event);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    (events, stats)
}

#[test]
fn power_route_literal_retains_predicates_not_just_projection_rectangle() {
    let reducer = fixture();
    let powers = DomainPowerBounds {
        max_positive_power: Some(4),
        min_power_difference: Some(1),
        max_power_difference: Some(3),
    };
    let (events, _) = constrained(&reducer, TARGET, [0; 3], [Some(2); 3], None, powers);
    let (_, cover) = image(&events[0]);
    assert_eq!(cover.power_bounds, powers);
    assert_eq!(cover.actual_rank, Some(2));
    assert!(!contains(
        &events[0],
        &IntegralKey::try_new([3, -2, 3]).unwrap()
    ));
    for a in 1..=3 {
        for b in 1..=3 {
            for k in 0..=2 {
                let key = IntegralKey::try_new([a, -k, b]).unwrap();
                assert_eq!(contains(&events[0], &key), power_contains(powers, &key));
            }
        }
    }
}

#[test]
fn power_route_implied_rank_prunes_weighted_masks_and_charges_them() {
    let reducer = fixture();
    let powers = DomainPowerBounds {
        max_positive_power: Some(10),
        min_power_difference: Some(8),
        max_power_difference: None,
    };
    let (events, stats) = constrained(&reducer, SOURCE, [0, 4, 4], [None; 3], None, powers);
    assert_eq!(events.len(), 1);
    assert_eq!(image(&events[0]).1.actual_rank, Some(2));
    assert_eq!(stats.masks_examined, 4);
    assert_eq!(stats.masks_pruned, 3);
    let error = reducer
        .visit_power_bounded_domain_route_overcover(
            SOURCE,
            &[0, 4, 4],
            &[None; 3],
            None,
            powers,
            CandidateDomainRouteLimits {
                max_masks: 2,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_| ControlFlow::Continue(()),
        )
        .unwrap_err();
    assert_eq!(error.stats.masks_examined, 2);
    assert_eq!(error.stats.masks_pruned, 1);
    assert!(matches!(
        error.failure,
        CandidateDomainRouteFailure::ResourceLimit {
            resource: "route masks",
            requested: 3,
            limit: 2,
        }
    ));
}

#[test]
fn power_route_affine_constant_must_weaken_upper_difference() {
    let (reducer, transport) = multipinch_fixture();
    let powers = DomainPowerBounds {
        max_positive_power: Some(3),
        min_power_difference: Some(2),
        max_power_difference: Some(2),
    };
    let key = IntegralKey::try_new([-1, 1, 2]).unwrap();
    let (events, _) = constrained(
        &reducer,
        SOURCE,
        [1, 0, 1],
        [Some(1), Some(0), Some(1)],
        None,
        powers,
    );
    let expanded = transport.transport(&key, Default::default()).unwrap();
    assert!(
        expanded
            .terms()
            .iter()
            .any(|term| term.key().powers() == [2, 1, 0])
    );
    let constant_endpoint = IntegralKey::try_new([2, 1, 0]).unwrap();
    assert!(!power_contains(powers, &constant_endpoint));
    assert!(
        events
            .iter()
            .any(|event| contains(event, &constant_endpoint))
    );
    for term in expanded.terms() {
        assert!(events.iter().any(|event| contains(event, term.key())));
    }
    assert_eq!(
        image(&events[0]).1.power_bounds.max_power_difference,
        Some(3)
    );
}

#[test]
fn power_route_weighted_multiple_pinches_translate_both_aggregate_caps() {
    let (reducer, _) = multipinch_fixture();
    let powers = DomainPowerBounds {
        max_positive_power: Some(6),
        min_power_difference: Some(-2),
        max_power_difference: None,
    };
    let (events, _) = constrained(
        &reducer,
        SOURCE,
        [0, 1, 3],
        [Some(8), Some(1), Some(3)],
        None,
        powers,
    );
    assert_eq!(events.len(), 4);
    for event in events {
        let (sector, cover) = image(&event);
        let cost = if !sector[0] { 4 } else { 0 } + if !sector[1] { 2 } else { 0 };
        assert_eq!(cover.actual_rank, Some(8 - cost));
        assert_eq!(
            cover.power_bounds.max_positive_power,
            Some(u64::from(6 - cost))
        );
        assert_eq!(cover.power_bounds.min_power_difference, Some(-2));
        assert_eq!(
            cover.power_bounds.max_power_difference,
            Some(i64::from(6 - cost))
        );
    }
}

#[test]
fn power_route_differential_affine_transport_over_constrained_integer_sources() {
    let (reducer, transport) = multipinch_fixture();
    let cases = [
        (Some(6), Some(1), Some(4)),
        (Some(5), Some(2), Some(2)),
        (Some(7), Some(-2), Some(0)),
        (None, None, Some(1)),
        (Some(6), Some(0), None),
    ];
    let mut tested = 0;
    let mut saw_constant = false;
    let mut saw_double_pinch = false;
    for (max_positive_power, min_power_difference, max_power_difference) in cases {
        let powers = DomainPowerBounds {
            max_positive_power,
            min_power_difference,
            max_power_difference,
        };
        for rank in [Some(6), None] {
            let (events, _) = constrained(
                &reducer,
                SOURCE,
                [0, 1, 0],
                [Some(6), Some(2), Some(2)],
                rank,
                powers,
            );
            for k in 0..=6 {
                for a in 2..=3 {
                    for b in 1..=3 {
                        let source = IntegralKey::try_new([-k, a, b]).unwrap();
                        if !power_contains(powers, &source) {
                            continue;
                        }
                        for term in transport
                            .transport(&source, Default::default())
                            .unwrap()
                            .terms()
                        {
                            tested += 1;
                            let target = term.key();
                            assert!(
                                events.iter().any(|event| contains(event, target)),
                                "uncovered {source:?} -> {target:?} with {powers:?}"
                            );
                            saw_constant |= k > 0 && target.powers() == [b, a, 0];
                            saw_double_pinch |= target.powers().iter().all(|&n| n <= 0);
                        }
                    }
                }
            }
        }
    }
    assert!(tested > 500);
    assert!(saw_constant);
    assert!(saw_double_pinch);
}

#[test]
fn power_route_empty_and_invalid_inputs_remain_distinct_before_dispatch() {
    let mut reducer = fixture();
    Arc::get_mut(&mut Arc::get_mut(&mut reducer.programs).unwrap().context)
        .unwrap()
        .shared
        .zero_sectors
        .insert([false; 3]);
    for source in [SOURCE, TARGET, [false; 3]] {
        let empty = DomainPowerBounds {
            min_power_difference: Some(1),
            max_positive_power: Some(0),
            ..Default::default()
        };
        let (events, stats) = constrained(&reducer, source, [0; 3], [None; 3], None, empty);
        assert!(events.is_empty());
        assert_eq!(stats, CandidateDomainRouteStats::default());
    }
    let error = reducer
        .visit_power_bounded_domain_route_overcover(
            SOURCE,
            &[1, 0, 0],
            &[None; 3],
            Some(0),
            DomainPowerBounds {
                min_power_difference: Some(2),
                max_power_difference: Some(1),
                ..Default::default()
            },
            Default::default(),
            &AtomicBool::new(false),
            |_| panic!("invalid domain dispatched"),
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        CandidateDomainRouteFailure::PowerDomain(DomainPowerError::InvertedDifferenceBounds)
    );
}

#[test]
fn power_route_missing_and_zero_preserve_power_provenance_and_source_obligation() {
    let mut reducer = fixture();
    let powers = DomainPowerBounds {
        max_positive_power: Some(0),
        min_power_difference: Some(-4),
        max_power_difference: Some(-2),
    };
    let (missing, _) = constrained(&reducer, [false; 3], [0; 3], [None; 3], None, powers);
    assert_eq!(
        missing,
        [CandidateDomainRouteEvent::MissingRoute {
            source_sector: [false; 3],
            actual_rank: Some(4),
            power_bounds: powers,
        }]
    );
    let ctx = Arc::get_mut(&mut Arc::get_mut(&mut reducer.programs).unwrap().context).unwrap();
    ctx.shared.zero_sectors.insert([false; 3]);
    ctx.shared.source_conditions.push(
        ctx.shared
            .context
            .numerator_condition_with_limits(
                &ctx.shared.context.index(0).unwrap(),
                Default::default(),
            )
            .unwrap(),
    );
    let (zero, _) = constrained(&reducer, [false; 3], [0; 3], [None; 3], None, powers);
    assert_eq!(
        zero,
        [CandidateDomainRouteEvent::ZeroSector {
            sector: [false; 3],
            actual_rank: Some(4),
            power_bounds: powers,
            source_conditions_required: true,
        }]
    );
}

#[test]
fn power_route_wide_finite_rank_is_never_truncated_or_silently_unbounded() {
    let reducer = fixture();
    let wide = u64::from(u32::MAX) + 1;
    let powers = DomainPowerBounds {
        max_positive_power: Some(2),
        ..Default::default()
    };
    let (literal, _) = constrained(
        &reducer,
        TARGET,
        [0, wide, 0],
        [Some(0), Some(wide), Some(0)],
        None,
        powers,
    );
    assert_eq!(image(&literal[0]).1.actual_rank, None);
    assert_eq!(image(&literal[0]).1.lower[1], wide);
    assert_eq!(image(&literal[0]).1.upper[1], Some(wide));
    assert_eq!(image(&literal[0]).1.power_bounds, powers);
    let error = reducer
        .visit_power_bounded_domain_route_overcover(
            SOURCE,
            &[wide, 0, 0],
            &[Some(wide), Some(0), Some(0)],
            None,
            powers,
            Default::default(),
            &AtomicBool::new(false),
            |_| panic!("wide affine rank dispatched"),
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        CandidateDomainRouteFailure::PowerDomain(DomainPowerError::OutOfRange(
            "routed numerator bound"
        ))
    );
}

#[test]
fn power_route_default_visitor_is_exactly_the_legacy_bounded_visitor() {
    let reducer = fixture();
    for source in [SOURCE, TARGET, [false; 3]] {
        for rank in [Some(0), Some(4), None] {
            let lower = [0, 1, 2];
            let upper = [Some(5), Some(8), None];
            assert_eq!(
                constrained(&reducer, source, lower, upper, rank, Default::default()),
                bounded(&reducer, source, lower, upper, rank),
            );
        }
    }
}
