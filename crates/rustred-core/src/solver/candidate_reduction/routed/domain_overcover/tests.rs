use super::*;
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::{
    Mask,
    symmetry::{self, CoefficientMatrix, MomentumMap, integral_transport},
};
use crate::solver::candidate_reduction::{
    owner_test_support::{input, programs},
    routed::{CandidateOwnerRoute, RoutedCandidateLimits, RoutedCandidateReducer},
};
use std::collections::BTreeSet;
use std::ops::ControlFlow;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const SOURCE: [bool; 3] = [false, true, true];
const TARGET: [bool; 3] = [true, false, true];

fn prepared(
    family: &Arc<IntegralFamily>,
    source: [bool; 3],
    target: [bool; 3],
    affine: bool,
) -> Arc<integral_transport::Prepared> {
    let c = family.coefficient_context();
    let map = symmetry::verify(
        family,
        family,
        MomentumMap::new(
            CoefficientMatrix::try_new(
                2,
                2,
                [
                    c.zero(),
                    c.one(),
                    c.one(),
                    if affine { c.one() } else { c.zero() },
                ],
            )
            .unwrap(),
            CoefficientMatrix::try_new(2, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        ),
        Default::default(),
    )
    .unwrap();
    Arc::new(
        integral_transport::compile(
            family,
            family.clone(),
            Arc::new(map),
            Mask::try_new(source).unwrap(),
            Mask::try_new(target).unwrap(),
            Default::default(),
        )
        .unwrap(),
    )
}
fn fixture() -> RoutedCandidateReducer<3> {
    let family = Arc::new(crate::solver::tests::sunset());
    let p = programs(
        family.clone(),
        Some(10),
        vec![input(TARGET, Some(10), vec![], &[])],
        Default::default(),
    );
    let route = CandidateOwnerRoute {
        owner_sector: Mask::try_new(TARGET).unwrap(),
        transport: prepared(&family, SOURCE, TARGET, false),
    };
    RoutedCandidateReducer::try_new(p, [route], Default::default()).unwrap()
}
fn collect<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    source: [bool; N],
    rank: Option<u32>,
) -> (Vec<CandidateDomainRouteEvent<N>>, CandidateDomainRouteStats) {
    let mut events = Vec::new();
    let stats = reducer
        .visit_domain_route_overcover(
            source,
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

#[test]
fn route_overcover_literal_owner_and_above_entry_rank_keep_apply_phase() {
    let reducer = fixture();
    for rank in [Some(11), None] {
        let (events, stats) = collect(&reducer, TARGET, rank);
        assert_eq!(
            events,
            [CandidateDomainRouteEvent::Apply {
                owner_sector: TARGET,
                cover: CandidateDomainRouteCover {
                    source_sector: TARGET,
                    target_root: TARGET,
                    actual_rank: rank,
                    conservative: true,
                }
            }]
        );
        assert_eq!(stats.apply_domains, 1);
        assert_eq!(stats.route_domains, 0);
        assert_eq!(stats.coordinate_cells, 6);
    }
}

#[test]
fn route_overcover_rank_zero_and_degree_bounded_pinches_preserve_actual_rank() {
    let reducer = fixture();
    for (rank, total) in [(0, 1), (1, 3), (2, 4), (11, 4)] {
        let (events, stats) = collect(&reducer, SOURCE, Some(rank));
        assert_eq!(events.len(), total);
        assert_eq!(stats.masks_examined, total);
        assert_eq!(stats.apply_domains, 1);
        assert_eq!(stats.route_domains, total - 1);
        assert!(matches!(
            events[0],
            CandidateDomainRouteEvent::Apply {
                owner_sector: TARGET,
                ..
            }
        ));
        let mut seen = BTreeSet::new();
        for event in events {
            let (sector, cover) = match event {
                CandidateDomainRouteEvent::Apply {
                    owner_sector,
                    cover,
                } => (owner_sector, cover),
                CandidateDomainRouteEvent::Route { sector, cover } => {
                    assert_ne!(sector, TARGET);
                    (sector, cover)
                }
                _ => panic!("cover only"),
            };
            assert!(sector.iter().zip(TARGET).all(|(&a, b)| !a || b));
            assert!(2 - sector.iter().filter(|&&b| b).count() <= rank as usize);
            assert_eq!(cover.actual_rank, Some(rank)); // never R-removed
            assert_eq!(cover.source_sector, SOURCE);
            assert_eq!(cover.target_root, TARGET);
            assert!(cover.conservative);
            assert!(seen.insert(sector));
        }
    }
}

#[test]
fn route_overcover_deterministic_combination_order_and_unbounded_rank() {
    let reducer = fixture();
    let (first, stats) = collect(&reducer, SOURCE, None);
    assert_eq!(first, collect(&reducer, SOURCE, None).0);
    assert_eq!(first.len(), 4);
    assert_eq!(stats.coordinate_cells, 24);
    let masks: Vec<_> = first
        .into_iter()
        .map(|e| match e {
            CandidateDomainRouteEvent::Apply {
                owner_sector,
                cover,
            } => {
                assert_eq!(cover.actual_rank, None);
                owner_sector
            }
            CandidateDomainRouteEvent::Route { sector, cover } => {
                assert_eq!(cover.actual_rank, None);
                sector
            }
            _ => panic!("cover only"),
        })
        .collect();
    assert_eq!(
        masks,
        [
            TARGET,
            [false, false, true],
            [true, false, false],
            [false; 3]
        ]
    );
}

#[test]
fn route_overcover_known_zero_retains_source_condition_obligation() {
    let mut reducer = fixture();
    let ctx = Arc::get_mut(&mut Arc::get_mut(&mut reducer.programs).unwrap().context).unwrap();
    ctx.shared.zero_sectors.insert([false; 3]);
    assert!(!reducer.domain_routing_requires_source_conditions());
    let (events, stats) = collect(&reducer, [false; 3], Some(12));
    assert_eq!(
        events,
        [CandidateDomainRouteEvent::ZeroSector {
            sector: [false; 3],
            actual_rank: Some(12),
            source_conditions_required: false
        }]
    );
    assert_eq!(stats.zero_sectors, 1);
    assert_eq!(stats.coordinate_cells, 0);
    let ctx = Arc::get_mut(&mut Arc::get_mut(&mut reducer.programs).unwrap().context).unwrap();
    let condition = ctx
        .shared
        .context
        .numerator_condition_with_limits(&ctx.shared.context.index(0).unwrap(), Default::default())
        .unwrap();
    ctx.shared.source_conditions.push(condition);
    assert!(reducer.domain_routing_requires_source_conditions());
    let (events, _) = collect(&reducer, [false; 3], Some(12));
    assert_eq!(
        events,
        [CandidateDomainRouteEvent::ZeroSector {
            sector: [false; 3],
            actual_rank: Some(12),
            source_conditions_required: true
        }]
    );
    // Nonzero transport can still produce only a mathematical image cover;
    // the presence of conditions is NOT erased or claimed validated.
    assert_eq!(collect(&reducer, SOURCE, Some(1)).0.len(), 3);
    assert!(reducer.domain_routing_requires_source_conditions());
}

#[test]
fn route_overcover_missing_route_and_scalar_support_are_explicit() {
    let reducer = fixture();
    let (events, stats) = collect(&reducer, [false; 3], None);
    assert_eq!(
        events,
        [CandidateDomainRouteEvent::MissingRoute {
            source_sector: [false; 3],
            actual_rank: None
        }]
    );
    assert_eq!(stats.missing_routes, 1);
    assert_eq!(stats.coordinate_cells, 0);
    let family = Arc::new(crate::solver::tests::sunset());
    let p = programs(
        family,
        Some(10),
        vec![input([false; 3], Some(10), vec![], &[])],
        Default::default(),
    );
    let scalar = RoutedCandidateReducer::try_new(p, [], Default::default()).unwrap();
    let (events, stats) = collect(&scalar, [false; 3], Some(13));
    assert!(
        matches!(events.as_slice(),[CandidateDomainRouteEvent::Apply{owner_sector:[false,false,false],cover}] if cover.actual_rank==Some(13))
    );
    assert_eq!(stats.events, 1);
}

#[test]
fn route_overcover_subsupports_reenter_route_even_when_literal_or_zero() {
    let family = Arc::new(crate::solver::tests::sunset());
    let mut p = programs(
        family.clone(),
        Some(10),
        vec![
            input(TARGET, Some(10), vec![], &[]),
            input([false, false, true], Some(10), vec![], &[]),
        ],
        Default::default(),
    );
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .zero_sectors
        .insert([false; 3]);
    let route = CandidateOwnerRoute {
        owner_sector: Mask::try_new(TARGET).unwrap(),
        transport: prepared(&family, SOURCE, TARGET, false),
    };
    let reducer = RoutedCandidateReducer::try_new(p, [route], Default::default()).unwrap();
    let (events, _) = collect(&reducer, SOURCE, Some(2));
    assert!(events.iter().any(|e| matches!(
        e,
        CandidateDomainRouteEvent::Route {
            sector: [false, false, true],
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        CandidateDomainRouteEvent::Route {
            sector: [false, false, false],
            ..
        }
    )));
    assert!(matches!(
        collect(&reducer, [false, false, true], Some(2))
            .0
            .as_slice(),
        [CandidateDomainRouteEvent::Apply { .. }]
    ));
    assert!(matches!(
        collect(&reducer, [false; 3], Some(2)).0.as_slice(),
        [CandidateDomainRouteEvent::ZeroSector { .. }]
    ));
}

#[test]
fn route_overcover_budget_cancel_and_consumer_stop_preserve_prefix() {
    let reducer = fixture();
    for (limits, resource, expected) in [
        (
            CandidateDomainRouteLimits {
                max_masks: 0,
                ..Default::default()
            },
            "route masks",
            0,
        ),
        (
            CandidateDomainRouteLimits {
                max_masks: 2,
                ..Default::default()
            },
            "route masks",
            2,
        ),
        (
            CandidateDomainRouteLimits {
                max_coordinate_cells: 5,
                ..Default::default()
            },
            "route coordinate cells",
            0,
        ),
        (
            CandidateDomainRouteLimits {
                max_coordinate_cells: 6,
                ..Default::default()
            },
            "route coordinate cells",
            1,
        ),
    ] {
        let mut events = Vec::new();
        let error = reducer
            .visit_domain_route_overcover(SOURCE, None, limits, &AtomicBool::new(false), |e| {
                events.push(e);
                ControlFlow::Continue(())
            })
            .unwrap_err();
        assert!(
            matches!(error.failure,CandidateDomainRouteFailure::ResourceLimit{resource:r,..} if r==resource)
        );
        assert_eq!(events.len(), expected);
        assert_eq!(error.stats.events, expected);
        assert_eq!(error.stats.masks_examined, expected);
    }
    let error = reducer
        .visit_domain_route_overcover(
            SOURCE,
            None,
            Default::default(),
            &AtomicBool::new(true),
            |_| panic!("cancelled"),
        )
        .unwrap_err();
    assert_eq!(error.failure, CandidateDomainRouteFailure::Cancelled);
    assert_eq!(error.stats.events, 0);
    let error = reducer
        .visit_domain_route_overcover(
            SOURCE,
            None,
            Default::default(),
            &AtomicBool::new(false),
            |_| ControlFlow::Break(()),
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        CandidateDomainRouteFailure::StoppedByConsumer
    );
    assert_eq!(error.stats.events, 1);
    let cancellation = AtomicBool::new(false);
    let error = reducer
        .visit_domain_route_overcover(SOURCE, None, Default::default(), &cancellation, |_| {
            cancellation.store(true, Ordering::Release);
            ControlFlow::Continue(())
        })
        .unwrap_err();
    assert_eq!(error.failure, CandidateDomainRouteFailure::Cancelled);
    assert_eq!(error.stats.events, 1);
}

#[test]
fn route_overcover_does_not_invoke_expansion_or_claim_every_support_is_reached() {
    let mut reducer = fixture();
    reducer.limits = RoutedCandidateLimits {
        max_transport_calls: 0,
        expansion: integral_transport::ExpansionLimits {
            max_total_power: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let (events, _) = collect(&reducer, SOURCE, Some(11));
    assert_eq!(events.len(), 4);
    // This particular map is a permutation: its actual single endpoint does
    // not pinch. Strict subsupport covers are deliberately NOT actual endpoints.
    let actual = reducer.routes[&SOURCE]
        .transport
        .transport(
            &IntegralKey::try_new([-1, 2, 3]).unwrap(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(actual.terms().len(), 1);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CandidateDomainRouteEvent::Route { .. }))
    );
}

#[test]
fn route_overcover_contains_native_affine_endpoints_with_unbounded_positive_policy() {
    let family = Arc::new(crate::solver::tests::sunset());
    let source = [true, false, false];
    let target = [false, true, false];
    let transport = prepared(&family, source, target, true);
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
    for rank in 0..=3_u32 {
        let (events, _) = collect(&reducer, source, Some(rank));
        for positive in [1, 2, 7, 1000] {
            for a in 0..=rank {
                for b in 0..=rank - a {
                    let integral =
                        IntegralKey::try_new([positive, -i64::from(a), -i64::from(b)]).unwrap();
                    let expanded = transport.transport(&integral, Default::default()).unwrap();
                    for term in expanded.terms() {
                        let endpoint = term.key();
                        let sector: [bool; 3] = std::array::from_fn(|i| endpoint.powers()[i] > 0);
                        let degree: u64 = endpoint
                            .powers()
                            .iter()
                            .filter(|&&n| n < 0)
                            .map(|n| n.unsigned_abs())
                            .sum();
                        assert!(degree <= u64::from(rank));
                        assert!(
                            events.iter().any(|e| match e {
                                CandidateDomainRouteEvent::Apply {
                                    owner_sector,
                                    cover,
                                } => *owner_sector == sector && cover.actual_rank == Some(rank),
                                CandidateDomainRouteEvent::Route { sector: s, cover } =>
                                    *s == sector && cover.actual_rank == Some(rank),
                                _ => false,
                            }),
                            "uncovered endpoint {endpoint:?}"
                        );
                    }
                }
            }
        }
    }
}
