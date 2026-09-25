//! Exact endpoint oracles use the existing Symbolica-backed native transport.
use super::*;
use crate::solver::DomainPowerBounds;

const SOURCE4: [bool; 10] = [
    false, true, false, true, true, false, false, false, false, true,
];
const ROOT4: [bool; 10] = [
    true, true, true, true, false, false, false, false, false, false,
];

fn fixture4() -> (
    RoutedCandidateReducer<10>,
    Arc<integral_transport::Prepared>,
) {
    // Two independent sunset shears in a complete four-loop scalar basis.
    // Rows 0/2 feed disjoint target pairs {0,1}/{2,3}; each pair shares a row.
    let family = Arc::new(crate::solver::tests::vacuum(&[
        vec![1, 0, 0, 0],
        vec![0, 1, 0, 0],
        vec![0, 0, 1, 0],
        vec![0, 0, 0, 1],
        vec![1, 1, 0, 0],
        vec![1, 0, 1, 0],
        vec![1, 0, 0, 1],
        vec![0, 1, 1, 0],
        vec![0, 1, 0, 1],
        vec![0, 0, 1, 1],
    ]));
    let c = family.coefficient_context();
    let map = symmetry::verify(
        &family,
        &family,
        MomentumMap::new(
            CoefficientMatrix::try_new(
                4,
                4,
                [1, -1, 0, 0, 0, 1, 0, 0, 0, 0, 1, -1, 0, 0, 0, 1].map(|v| c.integer(v)),
            )
            .unwrap(),
            CoefficientMatrix::try_new(4, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        ),
        Default::default(),
    )
    .unwrap();
    assert!(!map.denominators().constant()[0].is_zero());
    assert!(!map.denominators().constant()[2].is_zero());
    let transport = Arc::new(
        integral_transport::compile(
            &family,
            family.clone(),
            Arc::new(map),
            Mask::try_new(SOURCE4).unwrap(),
            Mask::try_new(ROOT4).unwrap(),
            Default::default(),
        )
        .unwrap(),
    );
    let reducer = RoutedCandidateReducer::try_new(
        programs(
            family,
            Some(0),
            vec![input(ROOT4, Some(0), vec![], &[])],
            Default::default(),
        ),
        [CandidateOwnerRoute {
            owner_sector: Mask::try_new(ROOT4).unwrap(),
            transport: transport.clone(),
        }],
        Default::default(),
    )
    .unwrap();
    (reducer, transport)
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

fn collect4(
    reducer: &RoutedCandidateReducer<10>,
    lower: [u64; 10],
    upper: [Option<u64>; 10],
    rank: Option<u32>,
    enabled: bool,
    constrained: bool,
) -> (
    Vec<CandidateDomainRouteEvent<10>>,
    CandidateDomainRouteStats,
) {
    let mut events = Vec::new();
    let stats = reducer
        .visit_power_bounded_domain_route_overcover_with_options(
            SOURCE4,
            &lower,
            &upper,
            rank,
            if constrained {
                DomainPowerBounds {
                    max_positive_power: Some(8),
                    ..Default::default()
                }
            } else {
                Default::default()
            },
            Default::default(),
            CandidateDomainRouteOptions {
                joint_source_support_pruning: enabled,
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

fn contains<const N: usize>(event: &CandidateDomainRouteEvent<N>, key: &IntegralKey) -> bool {
    let (sector, cover) = image(event);
    if sector != std::array::from_fn(|i| key.powers()[i] > 0) {
        return false;
    }
    let mut rank = 0_u64;
    for (axis, &power) in key.powers().iter().enumerate() {
        let coordinate = if power > 0 {
            (power - 1) as u64
        } else {
            rank += power.unsigned_abs();
            power.unsigned_abs()
        };
        if coordinate < cover.lower[axis] || cover.upper[axis].is_some_and(|hi| coordinate > hi) {
            return false;
        }
    }
    cover.actual_rank.is_none_or(|hi| rank <= u64::from(hi)) && {
        let positive: i128 = key.powers().iter().map(|&p| i128::from(p.max(0))).sum();
        let difference: i128 = key.powers().iter().map(|&p| i128::from(p)).sum();
        cover
            .power_bounds
            .max_positive_power
            .is_none_or(|hi| positive <= i128::from(hi))
            && cover
                .power_bounds
                .min_power_difference
                .is_none_or(|lo| difference >= i128::from(lo))
            && cover
                .power_bounds
                .max_power_difference
                .is_none_or(|hi| difference <= i128::from(hi))
    }
}

#[test]
fn joint_support_admitted_four_loop_witness_prunes_shared_rows_and_preserves_exact_endpoints() {
    let (reducer, transport) = fixture4();
    let mut lower = [0; 10];
    lower[0] = 1;
    lower[2] = 9;
    let upper = lower.map(Some);
    let (off, old) = collect4(&reducer, lower, upper, Some(10), false, true);
    let (on, new) = collect4(&reducer, lower, upper, Some(10), true, true);
    let both = [
        false, false, true, true, false, false, false, false, false, false,
    ];
    assert!(off.iter().any(|event| image(event).0 == both));
    assert!(!on.iter().any(|event| image(event).0 == both));
    assert_eq!(old.joint_support_masks_pruned, 0);
    assert!(new.joint_support_masks_pruned > 0);
    assert_eq!(old.masks_examined, new.masks_examined);
    assert_eq!(old.events - new.events, new.joint_support_masks_pruned);
    assert!(on.iter().all(|event| off.contains(event)));
    let exact = transport
        .transport(
            &IntegralKey::try_new([-1, 1, -9, 1, 1, 0, 0, 0, 0, 1]).unwrap(),
            Default::default(),
        )
        .unwrap();
    assert!(exact.terms().len() > 100);
    for term in exact.terms() {
        assert!(on.iter().any(|event| contains(event, term.key())));
        assert_ne!(
            std::array::from_fn::<_, 10, _>(|i| term.key().powers()[i] > 0),
            both
        );
    }
}

#[test]
fn joint_support_exhaustive_small_boxes_keep_symbolica_endpoints_constants_disjoint_rows_and_equality()
 {
    let (reducer, transport) = fixture4();
    let mut pruned = 0;
    let mut endpoints = 0;
    for p in 0..=2_u64 {
        for q in 0..=2_u64 {
            for b0 in 1..=2_u64 {
                for b1 in 1..=2_u64 {
                    let mut lower = [0; 10];
                    lower[1] = b1 - 1;
                    lower[4] = b0 - 1;
                    let mut upper = lower.map(Some);
                    upper[0] = Some(p);
                    upper[2] = Some(q);
                    for constrained in [false, true] {
                        let (off, old) =
                            collect4(&reducer, lower, upper, Some(6), false, constrained);
                        let (on, new) =
                            collect4(&reducer, lower, upper, Some(6), true, constrained);
                        assert_eq!(old.masks_examined, new.masks_examined);
                        assert_eq!(old.events - new.events, new.joint_support_masks_pruned);
                        assert!(on.iter().all(|event| off.contains(event)));
                        pruned += new.joint_support_masks_pruned;
                        for pp in 0..=p {
                            for qq in 0..=q {
                                let exact = transport
                                    .transport(
                                        &IntegralKey::try_new([
                                            -(pp as i64),
                                            b1 as i64,
                                            -(qq as i64),
                                            1,
                                            b0 as i64,
                                            0,
                                            0,
                                            0,
                                            0,
                                            1,
                                        ])
                                        .unwrap(),
                                        Default::default(),
                                    )
                                    .unwrap();
                                for term in exact.terms() {
                                    endpoints += 1;
                                    assert!(
                                        on.iter().any(|event| contains(event, term.key())),
                                        "{term:?}"
                                    );
                                }
                                if pp == 1 && qq == 1 && b0 == 1 && b1 == 1 {
                                    // Independent rows can each cancel one target; equality is attainable.
                                    assert!(exact.terms().iter().any(|term| {
                                        let n = term.key().powers();
                                        n[0] == 0 && n[2] == 0
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(pruned > 0 && endpoints > 1000);
}

#[test]
fn joint_support_infinity_and_mandatory_other_rows_preserve_necessary_bound() {
    let (reducer, _) = fixture4();
    let mut lower = [0; 10];
    lower[2] = 9;
    let mut upper = [Some(0); 10];
    upper[0] = None;
    upper[2] = None;
    let both = [
        false, false, true, true, false, false, false, false, false, false,
    ];
    for constrained in [false, true] {
        let (finite, stats) = collect4(&reducer, lower, upper, Some(10), true, constrained);
        assert!(stats.joint_support_masks_pruned > 0);
        assert!(!finite.iter().any(|event| image(event).0 == both));
        let (infinite, _) = collect4(&reducer, lower, upper, None, true, constrained);
        assert!(infinite.iter().any(|event| image(event).0 == both));
        let (equality, _) = collect4(&reducer, lower, upper, Some(11), true, constrained);
        assert!(equality.iter().any(|event| image(event).0 == both));
    }
}

#[test]
fn joint_support_budget_consumer_stop_and_cancellation_preserve_prefixes() {
    let (reducer, _) = fixture4();
    let mut lower = [0; 10];
    lower[0] = 1;
    lower[2] = 9;
    for limit in [1, 5, 8] {
        let mut events = 0;
        let error = reducer
            .visit_power_bounded_domain_route_overcover_with_options(
                SOURCE4,
                &lower,
                &lower.map(Some),
                Some(10),
                Default::default(),
                CandidateDomainRouteLimits {
                    max_masks: limit,
                    ..Default::default()
                },
                CandidateDomainRouteOptions {
                    joint_source_support_pruning: true,
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
                ..
            }
        ));
        assert_eq!(error.stats.masks_examined, limit);
        assert_eq!(error.stats.events, events);
        assert_eq!(error.stats.events + error.stats.masks_pruned, limit);
    }
    for stop in [false, true] {
        let cancel = AtomicBool::new(false);
        let error = reducer
            .visit_power_bounded_domain_route_overcover_with_options(
                SOURCE4,
                &lower,
                &lower.map(Some),
                Some(10),
                Default::default(),
                Default::default(),
                CandidateDomainRouteOptions {
                    joint_source_support_pruning: true,
                },
                &cancel,
                |_| {
                    if stop {
                        ControlFlow::Break(())
                    } else {
                        cancel.store(true, Ordering::Release);
                        ControlFlow::Continue(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(error.stats.masks_examined, 1);
        assert_eq!(error.stats.events, 1);
        assert_eq!(error.stats.joint_support_masks_pruned, 0);
        assert_eq!(
            error.failure,
            if stop {
                CandidateDomainRouteFailure::StoppedByConsumer
            } else {
                CandidateDomainRouteFailure::Cancelled
            }
        );
    }
}
