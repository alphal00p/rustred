use super::*;

#[test]
fn support_cache_matches_symbolica_verified_inactive_coefficients() {
    let permutation = fixture();
    let (_, affine) = multipinch_fixture();
    for transport in [&permutation.routes[&SOURCE].transport, &affine] {
        for target in 0..3 {
            let expected: Vec<_> = (0..3)
                .filter(|&source| {
                    !SOURCE[source]
                        && !transport
                            .verified_map()
                            .denominators()
                            .linear()
                            .get(source, target)
                            .unwrap()
                            .is_zero()
                })
                .collect();
            assert_eq!(
                transport.numerator_sources_for_target(target).unwrap(),
                expected
            );
        }
        assert!(transport.numerator_sources_for_target(3).is_none());
        assert!(transport.numerator_sources_for_target(usize::MAX).is_none());
    }
}

#[test]
fn support_route_zero_degree_affine_row_preserves_positive_lowers() {
    let (reducer, transport) = multipinch_fixture();
    let (events, _) = constrained(
        &reducer,
        SOURCE,
        [0, 1, 3],
        [Some(0), Some(3), Some(5)],
        Some(8),
        DomainPowerBounds {
            max_positive_power: Some(10),
            ..Default::default()
        },
    );
    assert_eq!(events.len(), 1);
    let (_, cover) = image(&events[0]);
    assert_eq!(cover.lower, [3, 1, 0]);
    assert_eq!(cover.actual_rank, Some(0));
    for a in 2..=4 {
        for b in 4..=6 {
            for term in transport
                .transport(
                    &IntegralKey::try_new([0, a, b]).unwrap(),
                    Default::default(),
                )
                .unwrap()
                .terms()
            {
                assert!(contains(&events[0], term.key()));
            }
        }
    }
}

#[test]
fn support_route_finite_row_power_tightens_lowers_without_total_rank_input() {
    let (reducer, transport) = multipinch_fixture();
    let powers = DomainPowerBounds {
        max_positive_power: Some(11),
        ..Default::default()
    };
    let (events, stats) = constrained(
        &reducer,
        SOURCE,
        [2, 4, 3],
        [Some(2), Some(5), Some(4)],
        None,
        powers,
    );
    assert_eq!(events.len(), 1);
    assert_eq!(image(&events[0]).1.lower, [1, 2, 0]);
    assert_eq!(stats.masks_examined, 4);
    assert_eq!(stats.masks_pruned, 3);
    let mut terms = 0;
    for a in 5..=6 {
        for b in 4..=5 {
            for term in transport
                .transport(
                    &IntegralKey::try_new([-2, a, b]).unwrap(),
                    Default::default(),
                )
                .unwrap()
                .terms()
            {
                terms += 1;
                assert!(contains(&events[0], term.key()));
            }
        }
    }
    assert!(terms > 20);
}

#[test]
fn support_route_exact_pinch_threshold_retains_total_weighted_cost() {
    let (reducer, transport) = multipinch_fixture();
    let powers = DomainPowerBounds {
        max_positive_power: Some(4),
        min_power_difference: Some(2),
        max_power_difference: None,
    };
    let (events, stats) = constrained(
        &reducer,
        SOURCE,
        [2, 1, 1],
        [Some(2), Some(1), Some(1)],
        Some(2),
        powers,
    );
    // Each active column individually admits a degree-two pinch; both together
    // would consume four units although the source numerator has only two.
    assert_eq!(events.len(), 3);
    assert_eq!(stats.masks_examined, 4);
    assert_eq!(stats.masks_pruned, 1);
    assert!(!events.iter().any(|event| image(event).0 == [false; 3]));
    let output = transport
        .transport(
            &IntegralKey::try_new([-2, 2, 2]).unwrap(),
            Default::default(),
        )
        .unwrap();
    for target in [[0, 2, 0], [2, 0, 0]] {
        assert!(
            output
                .terms()
                .iter()
                .any(|term| term.key().powers() == target)
        );
    }
    for term in output.terms() {
        assert!(events.iter().any(|event| contains(event, term.key())));
    }
}

#[test]
fn support_route_pinched_axis_resets_lower_and_retains_surviving_source_bound() {
    let (reducer, transport) = multipinch_fixture();
    let powers = DomainPowerBounds {
        max_positive_power: Some(4),
        min_power_difference: Some(3),
        max_power_difference: None,
    };
    let (events, _) = constrained(
        &reducer,
        SOURCE,
        [1, 0, 2],
        [Some(1), Some(0), Some(2)],
        None,
        powers,
    );
    assert_eq!(image(&events[0]).1.lower, [1, 0, 0]);
    let child = events
        .iter()
        .find(|event| image(event).0 == [true, false, false])
        .unwrap();
    assert_eq!(image(child).1.lower, [2, 0, 0]);
    assert_eq!(image(child).1.actual_rank, Some(0));
    for term in transport
        .transport(
            &IntegralKey::try_new([-1, 1, 3]).unwrap(),
            Default::default(),
        )
        .unwrap()
        .terms()
    {
        assert!(events.iter().any(|event| contains(event, term.key())));
    }
}

#[test]
fn support_route_empty_column_remains_zero_with_unbounded_numerator_rank() {
    let reducer = fixture();
    let (events, stats) = constrained(
        &reducer,
        SOURCE,
        [0, 1, 3],
        [None, Some(1), Some(3)],
        None,
        DomainPowerBounds {
            max_positive_power: Some(6),
            ..Default::default()
        },
    );
    assert_eq!(events.len(), 1);
    assert_eq!(image(&events[0]).1.lower, [1, 0, 3]);
    assert_eq!(image(&events[0]).1.actual_rank, None);
    assert_eq!(stats.masks_pruned, 3);
    let (affine, _) = multipinch_fixture();
    let (events, _) = constrained(
        &affine,
        SOURCE,
        [0, 1, 3],
        [None, Some(1), Some(3)],
        None,
        DomainPowerBounds {
            max_positive_power: Some(6),
            ..Default::default()
        },
    );
    assert_eq!(events.len(), 4);
    assert_eq!(image(&events[0]).1.lower, [0; 3]);
}

#[test]
fn support_route_refined_cover_keeps_streaming_cancellation_and_source_obligation() {
    let (mut reducer, _) = multipinch_fixture();
    let ctx = Arc::get_mut(&mut Arc::get_mut(&mut reducer.programs).unwrap().context).unwrap();
    ctx.shared.source_conditions.push(
        ctx.shared
            .context
            .numerator_condition_with_limits(
                &ctx.shared.context.index(0).unwrap(),
                Default::default(),
            )
            .unwrap(),
    );
    assert!(reducer.domain_routing_requires_source_conditions());
    let cancel = AtomicBool::new(false);
    let error = reducer
        .visit_power_bounded_domain_route_overcover(
            SOURCE,
            &[2, 4, 3],
            &[Some(2), Some(5), Some(4)],
            None,
            DomainPowerBounds {
                max_positive_power: Some(11),
                ..Default::default()
            },
            Default::default(),
            &cancel,
            |event| {
                assert_eq!(image(&event).1.lower, [1, 2, 0]);
                cancel.store(true, Ordering::Release);
                ControlFlow::Continue(())
            },
        )
        .unwrap_err();
    assert_eq!(error.failure, CandidateDomainRouteFailure::Cancelled);
    assert_eq!(error.stats.events, 1);
    assert_eq!(error.stats.masks_examined, 1);
}
