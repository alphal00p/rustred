use super::*;

fn bounded(
    reducer: &RoutedCandidateReducer<3>,
    source: [bool; 3],
    lower: [u64; 3],
    upper: [Option<u64>; 3],
    rank: Option<u32>,
) -> (Vec<CandidateDomainRouteEvent<3>>, CandidateDomainRouteStats) {
    let mut events = Vec::new();
    let stats = reducer
        .visit_bounded_domain_route_overcover(
            source,
            &lower,
            &upper,
            rank,
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

fn image(event: &CandidateDomainRouteEvent<3>) -> ([bool; 3], &CandidateDomainRouteCover<3>) {
    match event {
        CandidateDomainRouteEvent::Apply {
            owner_sector,
            cover,
        } => (*owner_sector, cover),
        CandidateDomainRouteEvent::Route { sector, cover } => (*sector, cover),
        _ => panic!("expected a domain image"),
    }
}

fn contains(event: &CandidateDomainRouteEvent<3>, key: &IntegralKey) -> bool {
    let (sector, cover) = image(event);
    if sector != std::array::from_fn(|i| key.powers()[i] > 0) {
        return false;
    }
    let mut rank = 0_u64;
    for (i, &power) in key.powers().iter().enumerate() {
        let x = if sector[i] {
            (power - 1) as u64
        } else {
            rank += power.unsigned_abs();
            power.unsigned_abs()
        };
        if x < cover.lower[i] || cover.upper[i].is_some_and(|upper| x > upper) {
            return false;
        }
    }
    cover
        .actual_rank
        .is_none_or(|bound| rank <= u64::from(bound))
}

fn multipinch_fixture() -> (RoutedCandidateReducer<3>, Arc<integral_transport::Prepared>) {
    let family = Arc::new(crate::solver::tests::sunset());
    let c = family.coefficient_context();
    // D0 -> 2D0+2D1-D2+2m², D1 -> D1, D2 -> D0. The
    // inactive numerator has both affine and genuinely multipinching terms.
    let map = symmetry::verify(
        &family,
        &family,
        MomentumMap::new(
            CoefficientMatrix::try_new(2, 2, [c.one(), c.integer(-1), c.zero(), c.one()]).unwrap(),
            CoefficientMatrix::try_new(2, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        ),
        Default::default(),
    )
    .unwrap();
    assert!(!map.denominators().constant()[0].is_zero());
    let target = [true, true, false];
    let transport = Arc::new(
        integral_transport::compile(
            &family,
            family.clone(),
            Arc::new(map),
            Mask::try_new(SOURCE).unwrap(),
            Mask::try_new(target).unwrap(),
            Default::default(),
        )
        .unwrap(),
    );
    let p = programs(
        family,
        Some(0),
        vec![input(target, Some(0), vec![], &[])],
        Default::default(),
    );
    let reducer = RoutedCandidateReducer::try_new(
        p,
        [CandidateOwnerRoute {
            owner_sector: Mask::try_new(target).unwrap(),
            transport: transport.clone(),
        }],
        Default::default(),
    )
    .unwrap();
    (reducer, transport)
}

#[test]
fn bounded_route_literal_preserves_all_bounds_without_entry_rank_clipping() {
    let reducer = fixture();
    let lower = [4, 3, 7];
    let upper = [Some(8), Some(9), None];
    for rank in [Some(11), None] {
        let (events, stats) = bounded(&reducer, TARGET, lower, upper, rank);
        let (_, cover) = image(&events[0]);
        assert_eq!(cover.lower, lower);
        assert_eq!(cover.upper, upper);
        assert_eq!(cover.actual_rank, rank);
        assert_eq!(stats.events, 1);
        assert_eq!(stats.masks_pruned, 0);
    }
}

#[test]
fn bounded_route_permutation_transfers_only_positive_upper_bounds() {
    let reducer = fixture();
    let (events, _) = bounded(
        &reducer,
        SOURCE,
        [2, 3, 4],
        [Some(2), Some(8), Some(9)],
        Some(3),
    );
    assert_eq!(events.len(), 1);
    let (_, cover) = image(&events[0]);
    assert_eq!(cover.lower, [0; 3]);
    assert_eq!(cover.upper, [Some(8), None, Some(9)]);
    assert_eq!(cover.actual_rank, Some(3));
    for a in 4..=9 {
        for b in 5..=10 {
            let output = reducer.routes[&SOURCE]
                .transport
                .transport(
                    &IntegralKey::try_new([-2, a, b]).unwrap(),
                    Default::default(),
                )
                .unwrap();
            for term in output.terms() {
                assert!(events.iter().any(|event| contains(event, term.key())));
            }
        }
    }
}

#[test]
fn bounded_route_native_affine_multipinch_differential() {
    let (reducer, transport) = multipinch_fixture();
    let lower = [0, 1, 3]; // minimum lost physical powers 2 and 4
    let upper = [Some(8), Some(2), Some(4)];
    let mut saw_double_pinch = false;
    let mut saw_tight_double_pinch = false;
    let mut saw_constant_term = false;
    let mut tested = 0;
    for rank in 0..=8_u32 {
        let (events, _) = bounded(&reducer, SOURCE, lower, upper, Some(rank));
        for k in 0..=rank {
            for a in 2..=3 {
                for b in 4..=5 {
                    let expanded = transport
                        .transport(
                            &IntegralKey::try_new([-i64::from(k), a, b]).unwrap(),
                            Default::default(),
                        )
                        .unwrap();
                    for term in expanded.terms() {
                        tested += 1;
                        let key = term.key();
                        assert!(
                            events.iter().any(|event| contains(event, key)),
                            "uncovered native endpoint {key:?} at source rank {rank}"
                        );
                        let is_scalar = key.powers().iter().all(|&n| n <= 0);
                        let actual_rank: u64 = key
                            .powers()
                            .iter()
                            .filter(|&&n| n < 0)
                            .map(|n| n.unsigned_abs())
                            .sum();
                        saw_double_pinch |= is_scalar;
                        saw_tight_double_pinch |=
                            is_scalar && rank >= 6 && actual_rank == u64::from(rank - 6);
                        saw_constant_term |= k > 0 && key.powers() == [b, a, 0];
                    }
                }
            }
        }
        for event in events {
            let (sector, cover) = image(&event);
            let cost = if !sector[0] { 4 } else { 0 } + if !sector[1] { 2 } else { 0 };
            assert_eq!(cover.actual_rank, Some(rank - cost));
            assert_eq!(cover.lower, [0; 3]);
            assert_eq!(
                cover.upper,
                [sector[0].then_some(4), sector[1].then_some(2), None]
            );
        }
    }
    assert!(tested > 1000);
    assert!(saw_double_pinch);
    assert!(saw_tight_double_pinch);
    assert!(saw_constant_term);
}

#[test]
fn bounded_route_impossible_weighted_pinches_consume_mask_allowance() {
    let reducer = fixture();
    let (events, stats) = bounded(&reducer, SOURCE, [0, 4, 4], [None; 3], Some(2));
    assert_eq!(events.len(), 1);
    assert_eq!(stats.masks_examined, 4);
    assert_eq!(stats.masks_pruned, 3);
    assert_eq!(stats.coordinate_cells, 6);
    let mut events = 0;
    let error = reducer
        .visit_bounded_domain_route_overcover(
            SOURCE,
            &[0, 4, 4],
            &[None; 3],
            Some(2),
            CandidateDomainRouteLimits {
                max_masks: 2,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_| {
                events += 1;
                ControlFlow::Continue(())
            },
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CandidateDomainRouteFailure::ResourceLimit {
            resource: "route masks",
            requested: 3,
            limit: 2
        }
    ));
    assert_eq!(events, 1);
    assert_eq!(error.stats.masks_examined, 2);
    assert_eq!(error.stats.masks_pruned, 1);
}

#[test]
fn bounded_route_u64_lower_extremes_and_unbounded_rank_do_not_wrap() {
    let reducer = fixture();
    let lower = [0, u64::MAX, u64::MAX];
    let upper = [Some(0), Some(u64::MAX), None];
    let (finite, stats) = bounded(&reducer, SOURCE, lower, upper, Some(u32::MAX));
    assert_eq!(finite.len(), 1);
    assert_eq!(stats.masks_pruned, 3);
    assert_eq!(image(&finite[0]).1.upper, [Some(u64::MAX), None, None]);
    let (unbounded, stats) = bounded(&reducer, SOURCE, lower, upper, None);
    assert_eq!(unbounded.len(), 4);
    assert_eq!(stats.masks_pruned, 0);
    assert!(
        unbounded
            .iter()
            .all(|event| image(event).1.actual_rank.is_none())
    );
}

#[test]
fn bounded_route_rank_zero_and_empty_source_are_distinct() {
    let reducer = fixture();
    let (events, _) = bounded(
        &reducer,
        SOURCE,
        [0, 8, 9],
        [Some(0), Some(8), Some(9)],
        Some(0),
    );
    assert_eq!(events.len(), 1);
    assert_eq!(image(&events[0]).1.upper, [Some(8), None, Some(9)]);
    for source in [SOURCE, TARGET, [false; 3]] {
        // One inactive lower bound exceeds R. This is empty even when the
        // sector would otherwise be a literal owner or have a missing route.
        let axis = source.iter().position(|&on| !on).unwrap();
        let mut lower = [0; 3];
        lower[axis] = u64::MAX;
        let (events, stats) = bounded(&reducer, source, lower, [None; 3], Some(u32::MAX));
        assert!(events.is_empty());
        assert_eq!(stats, CandidateDomainRouteStats::default());
    }
    let (events, _) = bounded(
        &reducer,
        [false; 3],
        [u64::MAX; 3],
        [None; 3],
        Some(u32::MAX),
    );
    assert!(events.is_empty());
}

#[test]
fn bounded_route_invalid_shape_and_inverted_bounds_fail_before_dispatch() {
    let mut reducer = fixture();
    Arc::get_mut(&mut Arc::get_mut(&mut reducer.programs).unwrap().context)
        .unwrap()
        .shared
        .zero_sectors
        .insert([false; 3]);
    for (source, lower, upper) in [
        (SOURCE, vec![0; 2], vec![None; 3]),
        (TARGET, vec![0; 3], vec![None; 2]),
        ([false; 3], vec![1, 0, 0], vec![Some(0), None, None]),
    ] {
        let error = reducer
            .visit_bounded_domain_route_overcover(
                source,
                &lower,
                &upper,
                None,
                Default::default(),
                &AtomicBool::new(false),
                |_| panic!("invalid input dispatched"),
            )
            .unwrap_err();
        assert!(matches!(
            error.failure,
            CandidateDomainRouteFailure::InvalidDomain(_)
        ));
        assert_eq!(error.stats, CandidateDomainRouteStats::default());
    }
}

#[test]
fn bounded_route_cancellation_stop_and_coordinate_budget_preserve_prefix() {
    let reducer = fixture();
    for (cancel, stop, cells, expected) in [
        (true, false, usize::MAX, 0),
        (false, true, usize::MAX, 1),
        (false, false, 6, 1),
    ] {
        let mut seen = 0;
        let error = reducer
            .visit_bounded_domain_route_overcover(
                SOURCE,
                &[0, 1, 1],
                &[None; 3],
                Some(4),
                CandidateDomainRouteLimits {
                    max_coordinate_cells: cells,
                    ..Default::default()
                },
                &AtomicBool::new(cancel),
                |_| {
                    seen += 1;
                    if stop {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(seen, expected);
        assert_eq!(error.stats.events, expected);
        if cancel {
            assert_eq!(error.failure, CandidateDomainRouteFailure::Cancelled);
        } else if stop {
            assert_eq!(
                error.failure,
                CandidateDomainRouteFailure::StoppedByConsumer
            );
        } else {
            assert!(matches!(
                error.failure,
                CandidateDomainRouteFailure::ResourceLimit {
                    resource: "route coordinate cells",
                    ..
                }
            ));
        }
    }
}

#[test]
fn bounded_route_full_orthant_wrapper_is_identical() {
    let reducer = fixture();
    for source in [SOURCE, TARGET, [false; 3]] {
        for rank in [Some(0), Some(1), Some(13), None] {
            assert_eq!(
                collect(&reducer, source, rank),
                bounded(&reducer, source, [0; 3], [None; 3], rank)
            );
        }
    }
}

#[test]
fn bounded_route_weighted_zero_reentry_keeps_source_conditions() {
    let (mut reducer, _) = multipinch_fixture();
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
    let (events, _) = bounded(
        &reducer,
        SOURCE,
        [0, 1, 3],
        [Some(6), Some(1), Some(3)],
        Some(6),
    );
    let scalar = events
        .iter()
        .find(|event| image(event).0 == [false; 3])
        .unwrap();
    let (_, cover) = image(scalar);
    assert_eq!(cover.actual_rank, Some(0));
    let (reentry, _) = bounded(
        &reducer,
        [false; 3],
        cover.lower,
        cover.upper,
        cover.actual_rank,
    );
    assert_eq!(
        reentry,
        [CandidateDomainRouteEvent::ZeroSector {
            sector: [false; 3],
            actual_rank: Some(0),
            source_conditions_required: true,
        }]
    );
}
