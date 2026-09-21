//! Synthetic formula fixtures test traversal semantics, not IBP provenance.
use std::collections::BTreeSet;

use super::*;
use crate::solver::CandidateTraceLimits;

fn fixed<const N: usize>(
    family: &IntegralFamily,
    target: [i16; N],
    rhs: Vec<([i16; N], i64)>,
) -> SectorRule<N> {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    SectorRule {
        candidate: RuleCandidate {
            case: CoordinateCase::new(target.map(Some)).unwrap().into(),
            target: Integral::numeric(target).unwrap(),
            rhs: rhs
                .into_iter()
                .map(|(powers, factor)| Term {
                    integral: Integral::numeric(powers).unwrap(),
                    coefficient: context.integer(factor).raw().clone(),
                })
                .collect(),
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions::default(),
    }
}

fn partial(specification: &[(i16, &[(i16, i64)])]) -> CandidateReducer<1> {
    let family = crate::solver::tests::tadpole();
    let rules = specification
        .iter()
        .map(|(target, rhs)| {
            fixed(
                &family,
                [*target],
                rhs.iter()
                    .map(|(child, factor)| ([*child], *factor))
                    .collect(),
            )
        })
        .collect();
    from_one_rule(
        &family,
        SectorSolution {
            rules,
            finite_residuals: Vec::new(),
            stats: SectorStats::default(),
            max_numerator_rank: None,
        },
    )
}

#[test]
fn frontier_retains_all_holes_and_deduplicates_shared_children() {
    let mut owner = partial(&[(6, &[(4, 1), (3, 1)]), (5, &[(4, 1), (2, 1)])]);
    let targets = [key([6]), key([5]), key([6]), key([5])];
    let report = owner
        .trace_targets(targets.clone(), Default::default())
        .unwrap();
    assert_eq!(report.input_targets(), 4);
    assert_eq!(report.requested_targets(), 2);
    assert_eq!(report.reachable_integrals(), 5);
    assert_eq!(report.rule_applications(), 2);
    assert_eq!(
        report.uncovered(),
        &BTreeSet::from([key([2]), key([3]), key([4])])
    );
    assert!(report.declared_terminals().is_empty());
    assert!(owner.terminals().is_empty());
    assert_eq!(owner.statistics().cached_integrals(), 0);
    let reversed = owner
        .trace_targets(targets.into_iter().rev(), Default::default())
        .unwrap();
    assert_eq!(report, reversed);
    assert!(matches!(
        owner.check_targets([key([6])]),
        Err(CandidateReductionError::Uncovered { .. })
    ));
}

#[test]
fn complete_trace_ignores_and_preserves_warm_cache_and_records_explicit_zeros() {
    let family = crate::solver::tests::tadpole();
    let mut owner = candidate::<1>(&family, Default::default());
    let cold = owner
        .trace_targets([key([3]), key([-4])], Default::default())
        .unwrap();
    let decomposition = owner.reduce_unit_mass(&key([3])).unwrap();
    let cache = format!("{:?}", owner.cache);
    let weight = owner.cache_weight;
    let before = owner.statistics();
    let warm = owner
        .trace_targets([key([-4]), key([3])], Default::default())
        .unwrap();
    assert_eq!(cold, warm);
    assert!(warm.uncovered().is_empty());
    assert_eq!(warm.declared_terminals(), &BTreeSet::from([key([1])]));
    assert_eq!(warm.visited_zeros(), &BTreeSet::from([key([-4])]));
    assert_eq!(warm.max_negative_index_degree(), 4);
    assert_eq!(warm.max_dot_excess(), 2);
    assert_eq!(warm.max_positive_power_sum(), 3);
    assert_eq!(cache, format!("{:?}", owner.cache));
    assert_eq!(weight, owner.cache_weight);
    assert_eq!(before.cache_hits(), owner.statistics().cache_hits());
    assert_eq!(
        before.cached_integrals(),
        owner.statistics().cached_integrals()
    );
    assert_eq!(
        before.cached_coefficient_terms(),
        owner.statistics().cached_coefficient_terms()
    );
    assert_eq!(
        before.cached_coefficient_bytes(),
        owner.statistics().cached_coefficient_bytes()
    );
    assert_eq!(owner.reduce_unit_mass(&key([3])).unwrap(), decomposition);
}

#[test]
fn trace_uses_raw_terminals_after_alias_installation() {
    let family = crate::solver::tests::sunset();
    let mut owner = candidate::<3>(&family, Default::default());
    let raw = owner.terminals().clone();
    let plan =
        crate::reduction::terminal_normalization::TerminalAliasPlan::independent_tadpole_products(
            &family,
            &raw,
            owner.ordering(),
        )
        .unwrap();
    owner.install_terminal_aliases(plan).unwrap();
    let report = owner
        .trace_targets(raw.clone(), Default::default())
        .unwrap();
    assert_eq!(report.declared_terminals(), &raw);
    assert_eq!(report.rule_applications(), 0);
}

#[test]
fn complete_guard_conjunction_and_affine_membership_remain_native() {
    let mut allowed = affine_test_owner(false);
    let report = allowed
        .trace_targets([key([2, 2, 1])], Default::default())
        .unwrap();
    assert!(report.uncovered().is_empty());
    assert_eq!(
        report.declared_terminals(),
        &BTreeSet::from([key([1, 2, 1])])
    );
    let report = affine_test_owner(true)
        .trace_targets([key([2, 2, 1])], Default::default())
        .unwrap();
    assert_eq!(report.uncovered(), &BTreeSet::from([key([2, 2, 1])]));
    assert_eq!(report.rule_applications(), 0);
}

#[test]
fn original_denominator_zero_skips_zero_coefficient_and_can_fall_through() {
    let mut owner = partial(&[(4, &[(3, 1)]), (4, &[(2, 1)])]);
    let context = owner.coefficient_context().clone();
    let denominator = context
        .admit_native_polynomial_result_with_limits(
            index_offset(&context, 0, 4),
            Default::default(),
        )
        .unwrap();
    let first = &mut owner.rules.get_mut(&[true]).unwrap()[0].rhs[0];
    first.denominator = denominator;
    first.coefficient = context.zero();
    let report = owner.trace_targets([key([4])], Default::default()).unwrap();
    assert_eq!(report.uncovered(), &BTreeSet::from([key([2])]));
    assert_eq!(report.rule_applications(), 1);
    owner.rules.get_mut(&[true]).unwrap().truncate(1);
    let report = owner.trace_targets([key([4])], Default::default()).unwrap();
    assert_eq!(report.uncovered(), &BTreeSet::from([key([4])]));
    assert_eq!(report.rule_applications(), 0);
}

#[test]
fn dimension_stays_symbolic_even_with_a_pole_at_four() {
    let mut owner = partial(&[(4, &[(3, 1)])]);
    let context = owner.coefficient_context().clone();
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let coefficient = context
        .div(
            &context.one(),
            &context.sub(&d, &context.integer(4)).unwrap(),
        )
        .unwrap();
    let denominator = context
        .admit_native_polynomial_result_with_limits(
            coefficient.raw().denominator.clone(),
            Default::default(),
        )
        .unwrap();
    let term = &mut owner.rules.get_mut(&[true]).unwrap()[0].rhs[0];
    term.coefficient = coefficient.clone();
    term.denominator = denominator;
    let report = owner.trace_targets([key([4])], Default::default()).unwrap();
    assert_eq!(report.uncovered(), &BTreeSet::from([key([3])]));
    assert_eq!(owner.rules[&[true]][0].rhs[0].coefficient, coefficient);
}

#[test]
fn empty_and_exactly_cancelled_rhs_are_successes_without_phantom_nodes() {
    let mut owner = partial(&[(4, &[]), (5, &[(3, 1), (3, -1)])]);
    let report = owner
        .trace_targets([key([4]), key([5])], Default::default())
        .unwrap();
    assert_eq!(report.reachable_integrals(), 2);
    assert_eq!(report.rule_applications(), 2);
    assert!(report.uncovered().is_empty());
    assert!(report.declared_terminals().is_empty());
    assert!(report.visited_zeros().is_empty());
}

#[test]
fn non_descent_to_an_already_scheduled_key_and_overflow_abort() {
    let mut owner = partial(&[(2, &[(3, 1)])]);
    assert!(matches!(
        owner.trace_targets([key([3]), key([2])], Default::default()),
        Err(CandidateReductionError::NonDescending { .. })
    ));
    let mut owner = partial(&[(2, &[(1, 1)])]);
    owner.rules.get_mut(&[true]).unwrap()[0].rhs[0].shift = [i64::MAX];
    assert!(matches!(
        owner.trace_targets([key([2])], Default::default()),
        Err(CandidateReductionError::IndexOverflow { .. })
    ));
}

#[test]
fn invalid_entries_source_conditions_and_algebra_limits_abort() {
    let mut owner = partial(&[(4, &[(3, 1)])]);
    assert!(matches!(
        owner.trace_targets([key([4, 1])], Default::default()),
        Err(CandidateReductionError::Application(
            ReductionError::WrongArity { .. }
        ))
    ));
    owner.root_sector = [false];
    assert!(matches!(
        owner.trace_targets([key([4])], Default::default()),
        Err(CandidateReductionError::OutsideRoot { .. })
    ));
    owner.root_sector = [true];
    let context = owner.coefficient_context().clone();
    owner.source_conditions.push(
        context
            .admit_native_polynomial_result_with_limits(
                index_offset(&context, 0, 4),
                Default::default(),
            )
            .unwrap(),
    );
    assert!(matches!(
        owner.trace_targets([key([4])], Default::default()),
        Err(CandidateReductionError::SourceConditionVanished { .. })
    ));
    owner.source_conditions.clear();
    owner
        .limits
        .indexed_algebra
        .exact_algebra
        .max_polynomial_terms = 0;
    assert!(matches!(
        owner.trace_targets([key([4])], Default::default()),
        Err(CandidateReductionError::Algebra(_))
    ));
}

#[test]
fn input_and_unique_limits_apply_before_unbounded_duplicate_or_graph_retention() {
    let mut owner = partial(&[]);
    let limits = CandidateTraceLimits {
        max_input_targets: 3,
        max_unique_integrals: 1,
    };
    assert_eq!(
        owner
            .trace_targets(std::iter::repeat(key([2])), limits)
            .unwrap_err(),
        CandidateReductionError::TraceInputLimit {
            requested: 4,
            limit: 3
        }
    );
    assert_eq!(
        owner
            .trace_targets([key([2]), key([3])], limits)
            .unwrap_err(),
        CandidateReductionError::TraceIntegralLimit {
            requested: 2,
            limit: 1
        }
    );
    let report = owner
        .trace_targets([key([2]), key([2]), key([2])], limits)
        .unwrap();
    assert_eq!(report.requested_targets(), 1);
    assert_eq!(owner.statistics().cached_integrals(), 0);
    assert!(
        owner
            .trace_targets(
                [],
                CandidateTraceLimits {
                    max_input_targets: 0,
                    max_unique_integrals: 0
                }
            )
            .is_ok()
    );
    assert!(matches!(
        owner.trace_targets(
            [key([2])],
            CandidateTraceLimits {
                max_input_targets: 0,
                ..limits
            }
        ),
        Err(CandidateReductionError::TraceInputLimit { .. })
    ));
    assert!(matches!(
        owner.trace_targets(
            [key([2])],
            CandidateTraceLimits {
                max_unique_integrals: 0,
                ..limits
            }
        ),
        Err(CandidateReductionError::TraceIntegralLimit { .. })
    ));
    let mut owner = partial(&[(4, &[(3, 1), (2, 1)])]);
    assert!(matches!(
        owner.trace_targets(
            [key([4])],
            CandidateTraceLimits {
                max_unique_integrals: 2,
                ..Default::default()
            }
        ),
        Err(CandidateReductionError::TraceIntegralLimit {
            requested: 3,
            limit: 2
        })
    ));
}

#[test]
fn pending_application_and_coalescing_caps_span_the_whole_batch() {
    let mut owner = partial(&[]);
    owner.limits.max_pending_frames = 1;
    assert!(matches!(
        owner.trace_targets([key([2]), key([3])], Default::default()),
        Err(CandidateReductionError::Application(
            ReductionError::PendingFrameLimit {
                requested: 2,
                limit: 1
            }
        ))
    ));
    owner.limits.max_pending_frames = 10;
    owner.limits.max_rule_applications = 1;
    assert!(matches!(
        owner.trace_targets([key([2]), key([3])], Default::default()),
        Err(CandidateReductionError::Application(
            ReductionError::RuleApplicationLimit {
                requested: 2,
                limit: 1
            }
        ))
    ));
    let mut owner = partial(&[(4, &[(2, 1), (2, -1)]), (5, &[(3, 1), (3, -1)])]);
    owner.limits.max_coalescing_additions = 1;
    assert!(matches!(
        owner.trace_targets([key([4]), key([5])], Default::default()),
        Err(CandidateReductionError::Application(
            ReductionError::CoalescingAdditionLimit {
                requested: 2,
                limit: 1
            }
        ))
    ));
    assert_eq!(owner.statistics().cached_integrals(), 0);
}

#[test]
fn descending_pinch_can_increase_numerator_degree() {
    let family = crate::solver::tests::sunset();
    let solution = SectorSolution {
        rules: vec![fixed(&family, [1, 1, 1], vec![([-3, 1, 1], 1)])],
        finite_residuals: Vec::new(),
        stats: SectorStats::default(),
        max_numerator_rank: None,
    };
    let mut owner = CandidateReducer::try_new(
        &family,
        [true; 3],
        OrderingPolicy::SpiredUncutV1,
        [([true; 3], solution)],
        Vec::new(),
        Default::default(),
    )
    .unwrap();
    let report = owner
        .trace_targets([key([1, 1, 1])], Default::default())
        .unwrap();
    assert_eq!(report.max_negative_index_degree(), 3);
    assert_eq!(report.uncovered(), &BTreeSet::from([key([-3, 1, 1])]));
}

#[test]
fn entry_rank_admission_precedes_cache_mutation_but_does_not_clip_descendants() {
    let family = crate::solver::tests::sunset();
    let root = key([1, 1, 1]);
    let child = key([-3, 1, 1]);
    let parent_solution = SectorSolution {
        rules: vec![fixed(&family, [1, 1, 1], vec![([-3, 1, 1], 1)])],
        finite_residuals: Vec::new(),
        stats: Default::default(),
        max_numerator_rank: Some(0),
    };
    let child_solution = SectorSolution {
        rules: Vec::new(),
        finite_residuals: vec![Integral::numeric([-3, 1, 1]).unwrap()],
        stats: Default::default(),
        max_numerator_rank: Some(0),
    };
    let mut owner = CandidateReducer::try_new(
        &family,
        [true; 3],
        OrderingPolicy::SpiredUncutV1,
        [
            ([true; 3], parent_solution),
            ([false, true, true], child_solution),
        ],
        Vec::new(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(owner.max_numerator_rank(), Some(0));
    let report = owner
        .trace_targets([root.clone()], Default::default())
        .unwrap();
    assert_eq!(report.max_negative_index_degree(), 3);
    assert_eq!(
        report.declared_terminals(),
        &BTreeSet::from([child.clone()])
    );
    assert!(report.uncovered().is_empty());
    let decomposition = owner.reduce_unit_mass(&root).unwrap();
    assert!(decomposition.terms().contains_key(&child));
    assert!(owner.cache.contains_key(&child));
    let before_cache = format!("{:?}", owner.cache);
    let before_weight = owner.cache_weight;
    let before_statistics = owner.statistics();
    let expected = CandidateReductionError::OutsideNumeratorRank {
        target: child.clone(),
        rank: 3,
        limit: 0,
    };
    assert_eq!(owner.reduce_unit_mass(&child).unwrap_err(), expected);
    assert_eq!(
        owner
            .check_targets([root.clone(), child.clone()])
            .unwrap_err(),
        expected
    );
    assert_eq!(
        owner
            .trace_targets([root.clone(), child], Default::default())
            .unwrap_err(),
        expected
    );
    assert_eq!(format!("{:?}", owner.cache), before_cache);
    assert_eq!(owner.cache_weight, before_weight);
    assert_eq!(owner.statistics(), before_statistics);
    // The negative-degree sum is exact even for the most negative i64.
    let extreme = key([i64::MIN, 1, 1]);
    assert_eq!(
        owner.reduce_unit_mass(&extreme).unwrap_err(),
        CandidateReductionError::OutsideNumeratorRank {
            target: extreme,
            rank: 1_u128 << 63,
            limit: 0
        }
    );
    // The rank is not a positive-power cap.
    assert_eq!(
        owner
            .trace_targets([key([100, 1, 1])], Default::default())
            .unwrap()
            .uncovered(),
        &BTreeSet::from([key([100, 1, 1])])
    );
}

#[test]
fn constructor_infers_consistent_scope_and_explicit_scope_survives_empty_records() {
    let family = crate::solver::tests::tadpole();
    let solution = |rank| SectorSolution::<1> {
        rules: Vec::new(),
        finite_residuals: Vec::new(),
        stats: Default::default(),
        max_numerator_rank: rank,
    };
    assert!(matches!(
        CandidateReducer::try_new(
            &family,
            [true],
            OrderingPolicy::SpiredUncutV1,
            [([true], solution(Some(2))), ([false], solution(None))],
            Vec::new(),
            Default::default()
        ),
        Err(CandidateReductionError::InconsistentNumeratorRank {
            expected: Some(2),
            actual: None
        })
    ));
    assert!(matches!(
        CandidateReducer::try_new_with_numerator_rank(
            &family,
            [true],
            OrderingPolicy::SpiredUncutV1,
            [([true], solution(Some(2)))],
            Vec::new(),
            Default::default(),
            Some(3)
        ),
        Err(CandidateReductionError::InconsistentNumeratorRank {
            expected: Some(3),
            actual: Some(2)
        })
    ));
    assert!(matches!(
        CandidateReducer::try_new_with_numerator_rank(
            &family,
            [true],
            OrderingPolicy::SpiredUncutV1,
            [([true], solution(Some(2)))],
            Vec::new(),
            Default::default(),
            None
        ),
        Err(CandidateReductionError::InconsistentNumeratorRank {
            expected: None,
            actual: Some(2)
        })
    ));
    let mut scoped = CandidateReducer::try_new_with_numerator_rank(
        &family,
        [true],
        OrderingPolicy::SpiredUncutV1,
        [],
        Vec::new(),
        Default::default(),
        Some(2),
    )
    .unwrap();
    assert_eq!(scoped.max_numerator_rank(), Some(2));
    assert!(matches!(
        scoped.reduce_unit_mass(&key([-3])),
        Err(CandidateReductionError::OutsideNumeratorRank {
            rank: 3,
            limit: 2,
            ..
        })
    ));
    let unscoped = CandidateReducer::try_new(
        &family,
        [true],
        OrderingPolicy::SpiredUncutV1,
        [],
        Vec::new(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(unscoped.max_numerator_rank(), None);
}
