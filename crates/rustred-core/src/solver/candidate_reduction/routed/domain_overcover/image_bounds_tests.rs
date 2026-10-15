//! Exact endpoint oracles are the existing Symbolica-backed native transport.
use super::super::power::{mapped_bounds, with_constant_degree_bound};
use super::super::support::constant_degree_cap;
use super::*;
use crate::solver::DomainPowerBounds;

fn collect_image<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    source: [bool; N],
    lower: [u64; N],
    upper: [Option<u64>; N],
    rank: Option<u32>,
    powers: DomainPowerBounds,
    precise: bool,
    joint: bool,
) -> (Vec<CandidateDomainRouteEvent<N>>, CandidateDomainRouteStats) {
    let mut events = Vec::new();
    let stats = reducer
        .visit_power_bounded_domain_route_overcover_with_options(
            source,
            &lower,
            &upper,
            rank,
            powers,
            Default::default(),
            CandidateDomainRouteOptions {
                precise_image_bounds: precise,
                joint_source_support_pruning: joint,
            },
            &AtomicBool::new(false),
            |event| {
                events.push(event);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    (events, stats)
}

fn image<const N: usize>(
    event: &CandidateDomainRouteEvent<N>,
) -> ([bool; N], &CandidateDomainRouteCover<N>) {
    match event {
        CandidateDomainRouteEvent::Apply {
            owner_sector,
            cover,
        } => (*owner_sector, cover),
        CandidateDomainRouteEvent::Route { sector, cover } => (*sector, cover),
        _ => panic!("expected cover"),
    }
}

fn contains<const N: usize>(event: &CandidateDomainRouteEvent<N>, key: &IntegralKey) -> bool {
    let (sector, cover) = image(event);
    let mut a = 0_u128;
    let mut r = 0_u128;
    for (axis, &n) in key.powers().iter().enumerate() {
        if sector[axis] != (n > 0) {
            return false;
        }
        let local = if n > 0 {
            a += n as u128;
            (n - 1) as u64
        } else {
            r += u128::from(n.unsigned_abs());
            n.unsigned_abs()
        };
        if local < cover.lower[axis] || cover.upper[axis].is_some_and(|u| local > u) {
            return false;
        }
    }
    let d = a as i128 - r as i128;
    cover.actual_rank.is_none_or(|cap| r <= u128::from(cap))
        && cover
            .power_bounds
            .max_positive_power
            .is_none_or(|cap| a <= u128::from(cap))
        && cover
            .power_bounds
            .min_power_difference
            .is_none_or(|cap| d >= i128::from(cap))
        && cover
            .power_bounds
            .max_power_difference
            .is_none_or(|cap| d <= i128::from(cap))
}

const HOMOGENEOUS_SOURCE: [bool; 6] = [false, true, false, true, false, false];

fn homogeneous_shear() -> (RoutedCandidateReducer<6>, Arc<integral_transport::Prepared>) {
    // Keep the ordinary unit-mass owner admission. This determinant-one map
    // has a genuinely mixed homogeneous inactive row:
    // D0 -> -2D1-2D2+2D3+2D4+D5. The other affine inactive rows are fixed
    // at exponent zero in these tests, not asserted homogeneous or discarded.
    let family = Arc::new(crate::solver::tests::vacuum(&[
        vec![1, 0, 0],
        vec![0, 1, 0],
        vec![0, 0, 1],
        vec![1, 1, 0],
        vec![1, 0, 1],
        vec![0, 1, 1],
    ]));
    let c = family.coefficient_context();
    let map = symmetry::verify(
        &family,
        &family,
        MomentumMap::new(
            CoefficientMatrix::try_new(3, 3, [2, 1, 1, -1, 0, -1, 0, 1, 0].map(|v| c.integer(v)))
                .unwrap(),
            CoefficientMatrix::try_new(3, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        ),
        Default::default(),
    )
    .unwrap();
    assert!(map.denominators().constant()[0].is_zero());
    assert!(
        map.denominators()
            .constant()
            .iter()
            .enumerate()
            .any(|(axis, coefficient)| axis != 0
                && !HOMOGENEOUS_SOURCE[axis]
                && !coefficient.is_zero())
    );
    let target = [false, false, false, true, true, false];
    let transport = Arc::new(
        integral_transport::compile(
            &family,
            family.clone(),
            Arc::new(map),
            Mask::try_new(HOMOGENEOUS_SOURCE).unwrap(),
            Mask::try_new(target).unwrap(),
            Default::default(),
        )
        .unwrap(),
    );
    let reducer = RoutedCandidateReducer::try_new(
        programs(
            family,
            Some(0),
            vec![input(target, Some(0), vec![], &[])],
            Default::default(),
        ),
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
fn precise_image_preserves_homogeneous_d_and_source_derived_pinched_siblings() {
    let (reducer, transport) = homogeneous_shear();
    let powers = DomainPowerBounds {
        max_positive_power: Some(5),
        min_power_difference: Some(1),
        max_power_difference: Some(1),
    };
    let (events, _) = collect_image(
        &reducer,
        HOMOGENEOUS_SOURCE,
        [4, 3, 0, 0, 0, 0],
        [Some(4), Some(3), Some(0), Some(0), Some(0), Some(0)],
        Some(4),
        powers,
        true,
        false,
    );
    assert!(events.len() > 1);
    assert!(
        events
            .iter()
            .all(|event| image(event).1.power_bounds.max_power_difference == Some(1))
    );
    let key = IntegralKey::try_new([-4, 4, 0, 1, 0, 0]).unwrap();
    let exact = transport.transport(&key, Default::default()).unwrap();
    let mut pinches = 0;
    for term in exact.terms() {
        assert!(
            events.iter().any(|event| contains(event, term.key())),
            "{term:?}"
        );
        assert_eq!(term.key().powers().iter().sum::<i64>(), 1);
        pinches += usize::from(term.key().powers().iter().filter(|&&n| n > 0).count() < 2);
    }
    assert!(pinches > 0);
    // Pinching the first target needs four units; its residual numerator upper
    // is zero, not the whole rank four. Sibling positive projection may differ.
    let pinch = events
        .iter()
        .find(|event| image(event).0 == [false, false, false, true, false, false])
        .unwrap();
    assert_eq!(image(pinch).1.upper[4], Some(0));
}

#[test]
fn precise_image_keeps_affine_constant_endpoints_and_all_constant_source_rows() {
    let (reducer, transport) = joint_support_tests::fixture4();
    let source = joint_support_tests::SOURCE4;
    let key = IntegralKey::try_new([-2, 3, -2, 3, 2, 0, 0, 0, 0, 2]).unwrap();
    let lower = std::array::from_fn(|i| {
        if source[i] {
            (key.powers()[i] - 1) as u64
        } else {
            key.powers()[i].unsigned_abs()
        }
    });
    let upper = lower.map(Some);
    let powers = DomainPowerBounds {
        max_positive_power: Some(10),
        min_power_difference: Some(6),
        max_power_difference: Some(6),
    };
    assert_eq!(
        constant_degree_cap(&transport, &source, &lower, &upper, Some(4)).unwrap(),
        Some(4)
    );
    let (events, _) = collect_image(&reducer, source, lower, upper, Some(4), powers, true, false);
    let exact = transport.transport(&key, Default::default()).unwrap();
    assert!(
        exact
            .terms()
            .iter()
            .any(|term| term.key().powers().iter().sum::<i64>() == 10)
    );
    for term in exact.terms() {
        assert!(events.iter().any(|event| contains(event, term.key())));
    }
    assert_eq!(
        image(&events[0]).1.power_bounds.max_power_difference,
        Some(10)
    );
}

#[test]
fn precise_image_permutation_zeros_unreachable_inactive_axes_and_keeps_off_identity() {
    let reducer = fixture();
    let powers = DomainPowerBounds {
        max_positive_power: Some(7),
        min_power_difference: Some(2),
        max_power_difference: Some(4),
    };
    let (off, stats) = collect_image(
        &reducer,
        SOURCE,
        [0, 0, 0],
        [Some(2), Some(3), Some(3)],
        Some(3),
        powers,
        false,
        false,
    );
    let mut default = Vec::new();
    let default_stats = reducer
        .visit_power_bounded_domain_route_overcover(
            SOURCE,
            &[0, 0, 0],
            &[Some(2), Some(3), Some(3)],
            Some(3),
            powers,
            Default::default(),
            &AtomicBool::new(false),
            |event| {
                default.push(event);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!((off, stats), (default, default_stats));
    let (tight, _) = collect_image(
        &reducer,
        SOURCE,
        [0, 0, 0],
        [Some(2), Some(3), Some(3)],
        Some(3),
        powers,
        true,
        false,
    );
    assert_eq!(tight.len(), 1);
    assert_eq!(image(&tight[0]).1.upper[1], Some(2));
    assert_eq!(
        image(&tight[0]).1.power_bounds.max_power_difference,
        Some(4)
    );
}

#[test]
fn precise_image_differential_exact_transport_covers_constants_pinches_and_joint_mode() {
    fn check<const N: usize>(
        reducer: RoutedCandidateReducer<N>,
        transport: Arc<integral_transport::Prepared>,
        source: [bool; N],
        point: impl Fn(i64, i64, i64) -> [i64; N],
    ) -> usize {
        let mut checked = 0;
        for r in 0..=5 {
            for a in 1..=3 {
                for b in 1..=3 {
                    let values = point(r, a, b);
                    let key = IntegralKey::try_new(values).unwrap();
                    let lower = std::array::from_fn(|axis| {
                        if source[axis] {
                            0
                        } else {
                            values[axis].unsigned_abs()
                        }
                    });
                    let upper = std::array::from_fn(|axis| {
                        Some(if source[axis] {
                            (values[axis] - 1) as u64
                        } else {
                            values[axis].unsigned_abs()
                        })
                    });
                    let d = a + b - r;
                    let powers = DomainPowerBounds {
                        max_positive_power: Some((a + b) as u64),
                        min_power_difference: Some(d),
                        max_power_difference: Some(d),
                    };
                    for joint in [false, true] {
                        let (events, _) = collect_image(
                            &reducer,
                            source,
                            lower,
                            upper,
                            Some(r as u32),
                            powers,
                            true,
                            joint,
                        );
                        for term in transport
                            .transport(&key, Default::default())
                            .unwrap()
                            .terms()
                        {
                            assert!(
                                events.iter().any(|event| contains(event, term.key())),
                                "{key:?}->{:?}",
                                term.key()
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        checked
    }
    let (affine, affine_map) = bounded::multipinch_fixture();
    let (homogeneous, homogeneous_map) = homogeneous_shear();
    let checked = check(affine, affine_map, SOURCE, |r, a, b| [-r, a, b])
        + check(
            homogeneous,
            homogeneous_map,
            HOMOGENEOUS_SOURCE,
            |r, a, b| [-r, a, 0, b, 0, 0],
        );
    assert!(checked > 500);
}

#[test]
fn precise_image_constant_cap_is_a_sum_with_zero_row_lower_rank_reservation() {
    let (_, transport) = joint_support_tests::fixture4();
    let source = joint_support_tests::SOURCE4;
    let constants = transport.verified_map().denominators().constant();
    let zero = constants
        .iter()
        .enumerate()
        .find(|(axis, c)| !source[*axis] && c.is_zero())
        .unwrap()
        .0;
    let mut lower = [0; 10];
    let mut upper = [Some(0); 10];
    lower[zero] = 3;
    upper[zero] = Some(3);
    upper[0] = Some(2);
    upper[2] = Some(5);
    assert_eq!(
        constant_degree_cap(&transport, &source, &lower, &upper, Some(6)).unwrap(),
        Some(3)
    );
    assert_eq!(
        constant_degree_cap(&transport, &source, &lower, &upper, None).unwrap(),
        Some(7)
    );
    upper[2] = None;
    assert_eq!(
        constant_degree_cap(&transport, &source, &lower, &upper, None).unwrap(),
        None
    );
    assert_eq!(
        constant_degree_cap(&transport, &source, &lower, &upper, Some(6)).unwrap(),
        Some(3)
    );
}

#[test]
fn precise_image_signed_bound_does_not_double_subtract_pinches_or_narrow_overflow() {
    let source = DomainPowerBounds {
        max_positive_power: Some(10),
        min_power_difference: Some(-2),
        max_power_difference: Some(3),
    };
    let mapped = mapped_bounds(source, Some(10), 4).unwrap();
    assert_eq!(
        with_constant_degree_bound(mapped, source, Some(2)).max_power_difference,
        Some(5)
    );
    assert_eq!(
        with_constant_degree_bound(mapped, source, Some(0)).max_power_difference,
        Some(3)
    );
    assert_eq!(with_constant_degree_bound(mapped, source, None), mapped);
    assert_eq!(
        with_constant_degree_bound(mapped, source, Some(u128::MAX)),
        mapped
    );
    let wide = DomainPowerBounds {
        max_power_difference: Some(i64::MAX),
        ..Default::default()
    };
    assert_eq!(
        with_constant_degree_bound(Default::default(), wide, Some(1)),
        DomainPowerBounds::default()
    );
}

#[test]
fn precise_image_exact_multivariate_transport_covers_mixed_constant_rows() {
    let (reducer, transport) = joint_support_tests::fixture4();
    let source = joint_support_tests::SOURCE4;
    let inactive: Vec<_> = source
        .iter()
        .enumerate()
        .filter_map(|(i, &active)| (!active).then_some(i))
        .collect();
    let mut points = 0;
    let mut endpoints = 0;
    for mask in 0_u32..(1 << inactive.len()) {
        if mask.count_ones() > 2 {
            continue;
        }
        let mut powers = [2_i64; 10];
        for (bit, &axis) in inactive.iter().enumerate() {
            powers[axis] = -i64::from(mask & (1 << bit) != 0);
        }
        let key = IntegralKey::try_new(powers).unwrap();
        let rank = mask.count_ones();
        let lower = std::array::from_fn(|axis| {
            if source[axis] {
                1
            } else {
                powers[axis].unsigned_abs()
            }
        });
        let bounds = DomainPowerBounds {
            max_positive_power: Some(8),
            min_power_difference: Some(8 - i64::from(rank)),
            max_power_difference: Some(8 - i64::from(rank)),
        };
        let exact = transport.transport(&key, Default::default()).unwrap();
        for input_rank in [Some(rank), None] {
            let (events, _) = collect_image(
                &reducer,
                source,
                lower,
                lower.map(Some),
                input_rank,
                bounds,
                true,
                true,
            );
            for term in exact.terms() {
                assert!(
                    events.iter().any(|event| contains(event, term.key())),
                    "{key:?}->{:?}",
                    term.key()
                );
                endpoints += 1;
            }
        }
        points += 1;
    }
    assert_eq!(points, 22);
    assert!(endpoints > 100);
}

#[test]
fn precise_image_preserves_source_obligations_and_control_flow_failures() {
    let mut reducer = fixture();
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
    let powers = DomainPowerBounds {
        max_positive_power: Some(6),
        ..Default::default()
    };
    for source in [TARGET, [false; 3], [true, false, false]] {
        assert_eq!(
            collect_image(
                &reducer,
                source,
                [0; 3],
                [Some(3); 3],
                Some(2),
                powers,
                false,
                false
            ),
            collect_image(
                &reducer,
                source,
                [0; 3],
                [Some(3); 3],
                Some(2),
                powers,
                true,
                false
            )
        );
    }
    for already_cancelled in [false, true] {
        let cancellation = AtomicBool::new(already_cancelled);
        let error = reducer
            .visit_power_bounded_domain_route_overcover_with_options(
                SOURCE,
                &[0; 3],
                &[Some(2); 3],
                Some(2),
                powers,
                Default::default(),
                CandidateDomainRouteOptions {
                    precise_image_bounds: true,
                    ..Default::default()
                },
                &cancellation,
                |_| ControlFlow::Break(()),
            )
            .unwrap_err();
        assert_eq!(
            error.failure,
            if already_cancelled {
                CandidateDomainRouteFailure::Cancelled
            } else {
                CandidateDomainRouteFailure::StoppedByConsumer
            }
        );
        assert_eq!(error.stats.events, usize::from(!already_cancelled));
    }
}
