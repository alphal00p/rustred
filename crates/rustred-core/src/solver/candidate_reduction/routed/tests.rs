//! Synthetic candidate formulas exercise traversal, not source certification.
use super::super::owner_test_support::*;
use super::*;
use crate::family::IntegralFamily;
use crate::identity::ParametricIbpGenerator;
use crate::reduction::ReductionLimits;
use crate::sector::symmetry::{self, CoefficientMatrix, MomentumMap, integral_transport};
use crate::sector::{Mask, OrderingPolicy};
use crate::solver::{CandidateReducer, CandidateTraceLimits};
use std::collections::BTreeSet;
use std::sync::Arc;

mod entry;

fn routed<const N: usize>(
    family: Arc<IntegralFamily>,
    rank: Option<u32>,
    inputs: Vec<crate::solver::CandidateOwnerInput<N>>,
) -> RoutedCandidateReducer<N> {
    RoutedCandidateReducer::try_new(
        programs(family, rank, inputs, Default::default()),
        [],
        Default::default(),
    )
    .unwrap()
}

fn swap_route(
    family: Arc<IntegralFamily>,
    source: [bool; 3],
    owner: [bool; 3],
) -> CandidateOwnerRoute {
    let c = family.coefficient_context();
    let map = symmetry::verify(
        &family,
        &family,
        MomentumMap::new(
            CoefficientMatrix::try_new(2, 2, [c.zero(), c.one(), c.one(), c.zero()]).unwrap(),
            CoefficientMatrix::try_new(2, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        ),
        Default::default(),
    )
    .unwrap();
    let owner_sector = Mask::try_new(owner).unwrap();
    let transport = integral_transport::compile(
        &family,
        family.clone(),
        Arc::new(map),
        Mask::try_new(source).unwrap(),
        owner_sector.clone(),
        Default::default(),
    )
    .unwrap();
    CandidateOwnerRoute {
        owner_sector,
        transport: Arc::new(transport),
    }
}

#[test]
fn one_owner_matches_existing_trace_and_never_touches_a_cache() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let make = || {
        input(
            [true],
            Some(0),
            vec![
                rule(&family, [3], &[([2], 1)]),
                rule(&family, [2], &[([1], 1)]),
            ],
            &[[1]],
        )
    };
    let mut old = CandidateReducer::try_new_with_numerator_rank(
        &family,
        [true],
        OrderingPolicy::default(),
        [([true], make().solution)],
        vec![],
        Default::default(),
        Some(0),
    )
    .unwrap();
    let legacy = old
        .trace_targets([key([3]), key([4])], CandidateTraceLimits::default())
        .unwrap();
    let owner = routed(family.clone(), Some(0), vec![make()]);
    let result = owner.trace_targets([key([4]), key([3]), key([3])]).unwrap();
    assert_eq!(result.input_targets(), 3);
    assert_eq!(result.requested_targets(), 2);
    assert_eq!(result.rule_applications(), legacy.rule_applications());
    assert_eq!(result.reachable_integrals(), legacy.reachable_integrals());
    assert_eq!(result.declared_terminals(), legacy.declared_terminals());
    assert_eq!(
        result
            .frontier()
            .iter()
            .map(|f| f.target.clone())
            .collect::<BTreeSet<_>>(),
        *legacy.uncovered()
    );
    assert_eq!(result.transport_calls(), 0);
    assert_eq!(old.statistics().cached_integrals(), 0);
    assert_eq!(
        result,
        owner.trace_targets([key([3]), key([4]), key([3])]).unwrap()
    );
}

#[test]
fn pinches_dispatch_to_lower_owner_and_above_entry_rank_children_are_not_readmitted() {
    let family = Arc::new(crate::solver::tests::sunset());
    // The second edge increases rank but is not necessarily descending under
    // every policy: use a dotted intermediate so the old evaluator proves it.
    let root = input(
        [true; 3],
        Some(0),
        vec![rule(&family, [2, 1, 1], &[([0, 3, 1], 1)])],
        &[],
    );
    let lower = input(
        [false, true, true],
        Some(0),
        vec![rule(&family, [0, 3, 1], &[([-1, 1, 1], 1)])],
        &[[-1, 1, 1]],
    );
    let owner = routed(family, Some(0), vec![root, lower]);
    let result = owner.trace_targets([key([2, 1, 1])]).unwrap();
    assert!(result.frontier().is_empty());
    assert_eq!(result.rule_applications(), 2);
    assert_eq!(result.max_negative_index_degree(), 1);
    assert_eq!(
        result.declared_terminals(),
        &BTreeSet::from([key([-1, 1, 1])])
    );
    assert!(matches!(
        owner.trace_targets([key([-1, 1, 1])]),
        Err(CandidateRoutedError::Candidate(
            crate::solver::CandidateReductionError::OutsideNumeratorRank { .. }
        ))
    ));
}

#[test]
fn verified_route_is_one_way_and_phase_nodes_keep_physical_identity_separate() {
    let family = Arc::new(crate::solver::tests::sunset());
    let programs = programs(
        family.clone(),
        Some(10),
        vec![input(
            [true, false, true],
            Some(10),
            vec![],
            &[[1, -1, 1], [1, 0, 1]],
        )],
        Default::default(),
    );
    let route = swap_route(family, [false, true, true], [true, false, true]);
    let owner = RoutedCandidateReducer::try_new(programs, [route], Default::default()).unwrap();
    let report = owner
        .trace_targets([key([-1, 1, 1]), key([0, 1, 1])])
        .unwrap();
    assert!(report.frontier().is_empty());
    assert_eq!(report.transport_calls(), 2);
    assert_eq!(report.operational_nodes(), 4);
    assert_eq!(
        report.declared_terminals(),
        &BTreeSet::from([key([1, -1, 1]), key([1, 0, 1])])
    );
}

#[test]
fn installed_owner_cannot_be_redirected_and_duplicate_routes_are_rejected() {
    let family = Arc::new(crate::solver::tests::sunset());
    let both = programs(
        family.clone(),
        None,
        vec![
            input([true, false, true], None, vec![], &[]),
            input([false, true, true], None, vec![], &[]),
        ],
        Default::default(),
    );
    assert!(
        RoutedCandidateReducer::try_new(
            both,
            [swap_route(
                family.clone(),
                [false, true, true],
                [true, false, true]
            )],
            Default::default()
        )
        .is_err()
    );
    let single = programs(
        family.clone(),
        None,
        vec![input([true, false, true], None, vec![], &[])],
        Default::default(),
    );
    let route = swap_route(family, [false, true, true], [true, false, true]);
    assert!(
        RoutedCandidateReducer::try_new(single, [route.clone(), route], Default::default())
            .is_err()
    );
}

#[test]
fn support_changing_same_count_edge_fails_instead_of_trying_a_later_rule() {
    let family = Arc::new(crate::solver::tests::sunset());
    let a = [3, 1, 0];
    let b = [0, 1, 1];
    let (target, child) = if OrderingPolicy::default().compare(&a, &b).unwrap().is_gt() {
        (a, b)
    } else {
        (b, a)
    };
    let mask = target.map(|x| x > 0);
    let owner = routed(
        family.clone(),
        None,
        vec![input(
            mask,
            None,
            vec![
                rule(
                    &family,
                    target.map(|x| x as i16),
                    &[(child.map(|x| x as i16), 1)],
                ),
                rule(&family, target.map(|x| x as i16), &[]),
            ],
            &[],
        )],
    );
    assert!(matches!(
        owner.trace_targets([key(child), key(target)]),
        Err(CandidateRoutedError::UnsupportedSupportTransition { .. })
    ));
}

#[test]
fn missing_owner_and_missing_rule_are_distinct_frontiers_not_terminals() {
    let family = Arc::new(crate::solver::tests::sunset());
    let owner = routed(family, None, vec![input([true; 3], None, vec![], &[])]);
    let report = owner
        .trace_targets([key([1, 1, 1]), key([0, 1, 1])])
        .unwrap();
    assert_eq!(report.frontier().len(), 2);
    assert!(
        report
            .frontier()
            .iter()
            .any(|f| matches!(f.reason, CandidateRoutedFrontierReason::MissingOwner))
    );
    assert!(
        report
            .frontier()
            .iter()
            .any(|f| matches!(f.reason, CandidateRoutedFrontierReason::MissingRule { .. }))
    );
    assert!(report.declared_terminals().is_empty());
}

#[test]
fn guards_are_whole_conjunctions_and_original_poles_select_the_next_rule() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let c = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let offset = |n: i64| {
        c.sub(&c.index(0).unwrap(), &c.integer(n))
            .unwrap()
            .raw()
            .numerator
            .clone()
    };
    let mut first = rule(&family, [2], &[([1], 1)]);
    first.exceptions.branches = vec![vec![offset(2), offset(3)]];
    let owner = routed(
        family.clone(),
        None,
        vec![input([true], None, vec![first], &[[1]])],
    );
    assert_eq!(
        owner
            .trace_targets([key([2])])
            .unwrap()
            .declared_terminals(),
        &BTreeSet::from([key([1])])
    );
    let mut pole = rule(&family, [2], &[([1], 1)]);
    pole.candidate.rhs[0].coefficient = c
        .div(
            &c.one(),
            &c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap(),
        )
        .unwrap()
        .raw()
        .clone();
    let owner = routed(
        family.clone(),
        None,
        vec![input(
            [true],
            None,
            vec![pole, rule(&family, [2], &[])],
            &[[1]],
        )],
    );
    let report = owner.trace_targets([key([2])]).unwrap();
    assert_eq!(report.rule_applications(), 1);
    assert!(report.declared_terminals().is_empty());
    assert!(report.frontier().is_empty());
}

#[test]
fn cumulative_transport_and_rule_limits_do_not_reset_between_owners_or_entries() {
    let family = Arc::new(crate::solver::tests::sunset());
    let initial_programs = programs(
        family.clone(),
        None,
        vec![input(
            [true, false, true],
            None,
            vec![],
            &[[1, -1, 1], [1, -2, 1]],
        )],
        Default::default(),
    );
    let route = swap_route(family.clone(), [false, true, true], [true, false, true]);
    let single = route
        .transport
        .transport(&key([-1, 1, 1]), Default::default())
        .unwrap();
    assert_eq!(single.terms().len(), 1);
    let owner = RoutedCandidateReducer::try_new(
        initial_programs,
        [route],
        RoutedCandidateLimits {
            max_transport_endpoints: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        owner.trace_targets([key([-1, 1, 1]), key([-2, 1, 1])]),
        Err(CandidateRoutedError::Transport(
            integral_transport::Error::Expansion(
                integral_transport::ExpansionError::ResourceLimit {
                    resource: "aggregate routed endpoints",
                    ..
                }
            )
        ))
    ));
    let p = programs(
        family.clone(),
        None,
        vec![
            input(
                [true; 3],
                None,
                vec![rule(&family, [2, 1, 1], &[([0, 2, 1], 1)])],
                &[],
            ),
            input(
                [false, true, true],
                None,
                vec![rule(&family, [0, 2, 1], &[([0, 1, 1], 1)])],
                &[[0, 1, 1]],
            ),
        ],
        ReductionLimits {
            max_rule_applications: 1,
            ..Default::default()
        },
    );
    let owner = RoutedCandidateReducer::try_new(p, [], Default::default()).unwrap();
    assert!(matches!(
        owner.trace_targets([key([2, 1, 1])]),
        Err(CandidateRoutedError::Candidate(
            crate::solver::CandidateReductionError::Application(
                crate::reduction::ReductionError::RuleApplicationLimit { .. }
            )
        ))
    ));
}

#[test]
fn duplicate_input_and_distinct_initial_node_limits_are_pre_retention() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let p = programs(
        family,
        None,
        vec![input([true], None, vec![], &[[1]])],
        Default::default(),
    );
    let owner = RoutedCandidateReducer::try_new(
        p.clone(),
        [],
        RoutedCandidateLimits {
            max_input_targets: 3,
            max_unique_nodes: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        owner
            .trace_targets([key([1]), key([1]), key([1])])
            .unwrap()
            .requested_targets(),
        1
    );
    assert!(matches!(
        owner.trace_targets(std::array::from_fn::<_, 4, _>(|_| key([1]))),
        Err(CandidateRoutedError::ResourceLimit {
            resource: "input targets",
            ..
        })
    ));
    let owner = RoutedCandidateReducer::try_new(
        p,
        [],
        RoutedCandidateLimits {
            max_input_targets: 100,
            max_unique_nodes: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        owner.trace_targets([key([1]), key([2])]),
        Err(CandidateRoutedError::ResourceLimit {
            resource: "initial operational nodes",
            ..
        })
    ));
}

#[test]
fn native_affine_numerator_route_expands_multiple_endpoints_and_a_strict_pinch() {
    let family = Arc::new(crate::solver::tests::sunset());
    let c = family.coefficient_context();
    // k1 -> k2, k2 -> k1+k2: D0 -> D1 while the inactive D2
    // becomes -D0+2D1+2D2+2. Symbolica verifies and expands this identity.
    let map = symmetry::verify(
        &family,
        &family,
        MomentumMap::new(
            CoefficientMatrix::try_new(2, 2, [c.zero(), c.one(), c.one(), c.one()]).unwrap(),
            CoefficientMatrix::try_new(2, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        ),
        Default::default(),
    )
    .unwrap();
    let source = Mask::try_new([true, false, false]).unwrap();
    let target = Mask::try_new([false, true, false]).unwrap();
    let transport = Arc::new(
        integral_transport::compile(
            &family,
            family.clone(),
            Arc::new(map),
            source,
            target.clone(),
            Default::default(),
        )
        .unwrap(),
    );
    let expanded = transport
        .transport(&key([1, 0, -1]), Default::default())
        .unwrap();
    assert_eq!(expanded.terms().len(), 4);
    let analyzer = crate::sector::zero::Analyzer::try_unrestricted(&family).unwrap();
    let crate::sector::zero::Decision::ProvedZero(zero) = analyzer
        .analyze(&Mask::try_new([false; 3]).unwrap())
        .unwrap()
    else {
        panic!("zero fixture")
    };
    let ctx = Arc::new(
        crate::solver::CandidateOwnerContext::try_new(
            family.clone(),
            crate::solver::CandidateOwnerScope {
                max_numerator_rank: Some(10),
                finite_case_policy: Default::default(),
            },
            vec![zero],
            Default::default(),
        )
        .unwrap(),
    );
    let p = Arc::new(
        crate::solver::CandidateOwnerPrograms::try_new(
            ctx,
            [input(
                [false, true, false],
                Some(10),
                vec![],
                &[[0, 1, 0], [-1, 1, 0], [0, 1, -1]],
            )],
        )
        .unwrap(),
    );
    let route = CandidateOwnerRoute {
        owner_sector: target,
        transport,
    };
    let owner =
        RoutedCandidateReducer::try_new(p.clone(), [route.clone()], Default::default()).unwrap();
    let result = owner.trace_targets([key([1, 0, -1])]).unwrap();
    assert!(result.frontier().is_empty());
    assert_eq!(result.transport_calls(), 1);
    assert_eq!(
        result.declared_terminals(),
        &BTreeSet::from([key([0, 1, 0]), key([-1, 1, 0]), key([0, 1, -1])])
    );
    assert_eq!(result.visited_zeros(), &BTreeSet::from([key([0, 0, 0])]));
    assert_eq!(result.max_negative_index_degree(), 1);
    assert!(result.transport_operation_bound() > 0);
    let limited = RoutedCandidateReducer::try_new(
        p,
        [route],
        RoutedCandidateLimits {
            max_transport_operations: result.transport_operation_bound(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        limited.trace_targets([key([1, 0, -1]), key([2, 0, -1])]),
        Err(CandidateRoutedError::Transport(
            integral_transport::Error::Expansion(
                integral_transport::ExpansionError::ResourceLimit {
                    resource: "aggregate routed operations",
                    ..
                }
            )
        ))
    ));
}

#[test]
fn pending_budget_late_invalid_entry_and_seen_self_edge_remain_errors() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let p = programs(
        family.clone(),
        None,
        vec![input([true], None, vec![], &[[1]])],
        ReductionLimits {
            max_pending_frames: 1,
            ..Default::default()
        },
    );
    let owner = RoutedCandidateReducer::try_new(p, [], Default::default()).unwrap();
    assert!(matches!(
        owner.trace_targets([key([1]), key([2])]),
        Err(CandidateRoutedError::ResourceLimit {
            resource: "initial pending frames",
            ..
        })
    ));
    let owner = routed(
        family.clone(),
        None,
        vec![input([true], None, vec![], &[[1]])],
    );
    assert!(owner.trace_targets([key([1]), key([1, 2])]).is_err());
    let owner = routed(
        family.clone(),
        None,
        vec![input(
            [true],
            None,
            vec![rule(&family, [2], &[([2], 1)])],
            &[],
        )],
    );
    assert!(matches!(
        owner.trace_targets([key([2])]),
        Err(CandidateRoutedError::Candidate(
            crate::solver::CandidateReductionError::NonDescending { .. }
        ))
    ));
}

#[test]
fn route_target_owner_and_common_family_are_admitted_before_traversal() {
    let family = Arc::new(crate::solver::tests::sunset());
    let p = programs(
        family.clone(),
        None,
        vec![input([true, false, true], None, vec![], &[])],
        Default::default(),
    );
    let mut route = swap_route(family.clone(), [false, true, true], [true, false, true]);
    route.owner_sector = Mask::try_new([true, true, false]).unwrap();
    assert!(RoutedCandidateReducer::try_new(p.clone(), [route], Default::default()).is_err());
    let other = Arc::new(crate::solver::tests::vacuum(&[
        vec![1, 0],
        vec![0, 1],
        vec![1, -1],
    ]));
    let route = swap_route(other, [false, true, true], [true, false, true]);
    assert!(RoutedCandidateReducer::try_new(p, [route], Default::default()).is_err());
}

#[test]
fn declared_affine_chart_and_saved_order_match_the_existing_evaluator() {
    use crate::solver::{
        AffineCase, AffineIntersection, CoordinateCase, ExceptionalConditions, Integral, Power,
        RuleCandidate, SearchStats, SectorRule, Term,
    };
    let family = Arc::new(crate::solver::tests::sunset());
    let c = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let ctx = context(family.clone(), None, Default::default());
    let make = || {
        let equality = c
            .sub(&c.index(0).unwrap(), &c.index(1).unwrap())
            .unwrap()
            .raw()
            .numerator
            .clone();
        let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
            &CoordinateCase::new([None, None, Some(1)]).unwrap(),
            &[equality],
            ctx.index_variables(),
            &[true; 3],
        )
        .unwrap() else {
            panic!("declared affine chart")
        };
        let target = case.face().integral();
        let rule = SectorRule {
            candidate: RuleCandidate {
                case: case.into(),
                target,
                rhs: vec![Term {
                    integral: Integral::new([
                        Power::new(true, -1).unwrap(),
                        Power::new(true, 0).unwrap(),
                        Power::new(false, 1).unwrap(),
                    ]),
                    coefficient: c.one().raw().clone(),
                }],
                sources: vec![],
                stats: SearchStats::default(),
            },
            exceptions: ExceptionalConditions::default(),
        };
        let mut i = input([true; 3], None, vec![rule], &[[1, 2, 1]]);
        i.ordering = OrderingPolicy::SpiredUncutV1;
        i
    };
    let mut old = CandidateReducer::try_new(
        &family,
        [true; 3],
        OrderingPolicy::SpiredUncutV1,
        [([true; 3], make().solution)],
        vec![],
        Default::default(),
    )
    .unwrap();
    let p =
        Arc::new(crate::solver::CandidateOwnerPrograms::try_new(ctx.clone(), [make()]).unwrap());
    let routed = RoutedCandidateReducer::try_new(p, [], Default::default()).unwrap();
    let expected = old
        .trace_targets([key([2, 2, 1]), key([2, 3, 1])], Default::default())
        .unwrap();
    let actual = routed
        .trace_targets([key([2, 2, 1]), key([2, 3, 1])])
        .unwrap();
    assert_eq!(actual.declared_terminals(), expected.declared_terminals());
    assert_eq!(
        actual
            .frontier()
            .iter()
            .map(|f| f.target.clone())
            .collect::<BTreeSet<_>>(),
        *expected.uncovered()
    );
    assert_eq!(actual.rule_applications(), expected.rule_applications());
}
