use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{
    IntegralOrder, PolynomialRow, SearchOptions, SearchStats, SectorConfig, SourceSystem, Term,
};

fn limits() -> RuleTrialLimits {
    RuleTrialLimits {
        max_depth: 1,
        max_rows: 32,
        max_exact_trace_rows: 32,
        max_exact_trace_terms: 128,
    }
}

fn policy() -> RuleSelectionPolicy {
    RuleSelectionPolicy::BoundedPortfolio {
        alternatives: vec![SourceDiscoveryStrategy::Materialized(
            SourceVisitOrder::new(vec![1, 0], 2).unwrap(),
        )],
        limits: limits(),
        quality: vec![RuleQualityPriority {
            feature: RuleQualityFeature::RhsTerms,
            descending: false,
        }],
        trigger: RulePortfolioTrigger::Always,
    }
}

fn row(context: &CoefficientContext, terms: &[(i16, &str)]) -> PolynomialRow<1> {
    terms
        .iter()
        .map(|&(shift, coefficient)| Term {
            integral: Integral::symbolic([shift]).unwrap(),
            coefficient: context.coefficient_fixture(coefficient).numerator,
        })
        .collect()
}

fn system() -> SourceSystem<1> {
    let context = CoefficientContext::new(["n"]);
    SourceSystem::new(
        vec![
            row(&context, &[(0, "1"), (-1, "n-1"), (-2, "(n-1)*(n-2)")]),
            row(&context, &[(0, "1"), (-1, "n-1")]),
        ],
        [0],
    )
    .unwrap()
}

// Deliberately preserve the two independent source recurrences instead of
// preconditioning them together. Selected seeds are replayed against this
// exact basis below; actual constructor/default controls are separate.
fn solver(system: &SourceSystem<1>, policy: RuleSelectionPolicy) -> SectorSolver<'_, 1> {
    SectorSolver {
        system,
        basis: system.rows().to_vec(),
        order: IntegralOrder::new([true], [false]),
        config: SectorConfig {
            rule_selection: policy,
            ..Default::default()
        },
    }
}

fn select(solver: &SectorSolver<'_, 1>) -> (Admitted<1>, SectorStats, Vec<RuleTrialSummary>) {
    let mut stats = SectorStats::default();
    let mut trials = Vec::new();
    let selected = solver
        .select_rule(
            &Case::generic(),
            SectorSolveOptions::default(),
            &mut stats,
            &mut |event| {
                if let SectorEvent::RuleTrialFinished { summary, .. } = event {
                    trials.push(summary);
                }
            },
        )
        .unwrap();
    (selected, stats, trials)
}

#[test]
fn bounded_rule_portfolio_selects_exact_smaller_rhs_and_replays_original_source() {
    let system = system();
    let solver = solver(&system, policy());
    let (selected, stats, trials) = select(&solver);
    assert_eq!(selected.rule.candidate.rhs.len(), 1);
    assert_eq!(selected.rule.candidate.sources[0].basis_row, 1);
    assert!(selected.children.is_empty());
    assert_eq!(trials.len(), 2);
    assert!(
        trials
            .iter()
            .all(|t| t.outcome == RuleTrialOutcome::Admitted)
    );
    let totals = stats.rule_selection.unwrap();
    assert_eq!(
        (
            totals.attempted,
            totals.admitted,
            totals.selected_alternatives
        ),
        (2, 2, 1)
    );
    assert_eq!(totals.total.search.rows, 2);
    assert_eq!(stats.symbolic_rows, 2);
    assert_eq!(totals.total.exact_trace_terms, 5);
    let source = selected.rule.candidate.sources[0];
    let exact = crate::solver::instantiate::instantiate(
        &solver.basis[source.basis_row],
        &source.seed,
        &solver.system.indices,
        solver.system.fixed(),
        &solver.order,
        &[],
        None,
    )
    .unwrap();
    let (target, rhs) =
        crate::solver::instantiate::canonicalize(exact, &solver.system.indices).unwrap();
    assert_eq!(target, selected.rule.candidate.target);
    assert_eq!(rhs, selected.rule.candidate.rhs);
    let complete = solver.solve_sector(SectorSolveOptions::default()).unwrap();
    assert_eq!(complete.rules.len(), 1);
    assert_eq!(complete.rules[0].candidate.rhs, rhs);
    assert!(complete.finite_residuals.is_empty());
}

#[test]
fn bounded_rule_portfolio_trigger_and_duplicate_skip_without_optional_search() {
    let system = system();
    let mut conditional = policy();
    let RuleSelectionPolicy::BoundedPortfolio { trigger, .. } = &mut conditional else {
        unreachable!()
    };
    *trigger = RulePortfolioTrigger::AnyAtLeast(vec![RuleQualityThreshold {
        feature: RuleQualityFeature::RhsTerms,
        minimum: 3,
    }]);
    let (selected, stats, trials) = select(&solver(&system, conditional));
    assert_eq!(selected.rule.candidate.sources[0].basis_row, 0);
    assert_eq!(trials.len(), 1);
    assert_eq!(stats.rule_selection.unwrap().trigger_skips, 1);
    let mut duplicate = policy();
    let RuleSelectionPolicy::BoundedPortfolio { alternatives, .. } = &mut duplicate else {
        unreachable!()
    };
    *alternatives = vec![SourceDiscoveryStrategy::InputOrder];
    let (_, stats, trials) = select(&solver(&system, duplicate));
    assert_eq!(trials.len(), 1);
    assert_eq!(stats.rule_selection.unwrap().duplicate_skips, 1);
}

#[test]
fn bounded_rule_portfolio_ties_keep_baseline_and_runtime_direction_changes_winner() {
    let system = system();
    let mut tied = policy();
    let RuleSelectionPolicy::BoundedPortfolio { quality, .. } = &mut tied else {
        unreachable!()
    };
    quality[0].feature = RuleQualityFeature::SourceRows;
    let (selected, stats, _) = select(&solver(&system, tied));
    assert_eq!(selected.rule.candidate.sources[0].basis_row, 0);
    assert_eq!(stats.rule_selection.unwrap().selected_alternatives, 0);
    let mut descending = policy();
    let RuleSelectionPolicy::BoundedPortfolio { quality, .. } = &mut descending else {
        unreachable!()
    };
    quality[0].descending = true;
    let (selected, _, _) = select(&solver(&system, descending));
    assert_eq!(selected.rule.candidate.sources[0].basis_row, 0);
}

#[test]
fn bounded_rule_portfolio_direct_hit_cap_precedes_canonicalization_and_counts_refusal() {
    let system = system();
    let mut bounded = policy();
    let RuleSelectionPolicy::BoundedPortfolio { limits, .. } = &mut bounded else {
        unreachable!()
    };
    limits.max_exact_trace_terms = 1;
    let (selected, stats, trials) = select(&solver(&system, bounded));
    assert_eq!(selected.rule.candidate.sources[0].basis_row, 0);
    assert_eq!(
        trials[1].outcome,
        RuleTrialOutcome::WorkLimit(RuleTrialBudget::ExactTraceTerms)
    );
    assert_eq!(trials[1].stats.search.rows, 1);
    assert_eq!(trials[1].stats.guard_branches, 0);
    assert_eq!(stats.rule_selection.unwrap().work_limited, 1);
    assert_eq!(stats.symbolic_rows, 2);
}

#[test]
fn bounded_rule_portfolio_baseline_geometry_failure_remains_fatal_and_observed() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(
        vec![
            row(&context, &[(0, "n-2"), (-1, "1")]),
            row(&context, &[(0, "1"), (-1, "n-1")]),
        ],
        [0],
    )
    .unwrap();
    let solver = solver(&system, policy());
    let mut trials = Vec::new();
    let mut published = 0;
    let error = solver
        .solve_sector_with_observer(
            SectorSolveOptions {
                case_intersection_limits: CaseIntersectionLimits {
                    max_work_items: 0,
                    ..Default::default()
                },
                ..Default::default()
            },
            |event| match event {
                SectorEvent::RuleTrialFinished { summary, .. } => trials.push(summary),
                SectorEvent::RuleFound { .. } => published += 1,
                _ => (),
            },
        )
        .unwrap_err();
    assert!(matches!(error, SectorSolveError::Intersection(_)));
    assert_eq!(published, 0);
    assert_eq!(trials.len(), 1);
    assert_eq!(trials[0].outcome, RuleTrialOutcome::Fatal);
    assert!(trials[0].stats.geometry_calls > 0);
    assert!(trials[0].stats.search.rows > 0);
}

#[test]
fn bounded_rule_portfolio_optional_geometry_budget_preserves_incumbent_and_all_costs() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(
        vec![
            row(&context, &[(0, "1"), (-1, "n-1")]),
            row(&context, &[(0, "n-2"), (-1, "1")]),
        ],
        [0],
    )
    .unwrap();
    let solver = solver(&system, policy());
    let mut stats = SectorStats::default();
    let mut trials = Vec::new();
    let selected = solver
        .select_rule(
            &Case::generic(),
            SectorSolveOptions {
                case_intersection_limits: CaseIntersectionLimits {
                    max_work_items: 0,
                    ..Default::default()
                },
                ..Default::default()
            },
            &mut stats,
            &mut |event| {
                if let SectorEvent::RuleTrialFinished { summary, .. } = event {
                    trials.push(summary);
                }
            },
        )
        .unwrap();
    assert_eq!(selected.rule.candidate.sources[0].basis_row, 0);
    assert_eq!(trials[1].outcome, RuleTrialOutcome::GeometryBudget);
    assert!(trials[1].stats.guard_branches > 0);
    assert_eq!(
        stats.discarded_cases, 0,
        "loser geometry is not logically discarded input"
    );
}

#[test]
fn bounded_rule_portfolio_validation_and_refusal_taxonomy_fail_closed() {
    assert!(RuleSelectionPolicy::FirstValid.validate(1).is_ok());
    let mut invalid = policy();
    let RuleSelectionPolicy::BoundedPortfolio { quality, .. } = &mut invalid else {
        unreachable!()
    };
    quality.push(quality[0]);
    assert!(invalid.validate(1).is_err());
    for source in [
        SolverError::InvalidInput("bad".into()),
        SolverError::ExactReplay("bad".into()),
    ] {
        assert_eq!(
            optional_refusal(&SectorSolveError::<1>::Search {
                case: Case::generic(),
                source
            }),
            None
        );
    }
    assert_eq!(
        optional_refusal(&SectorSolveError::<1>::Search {
            case: Case::generic(),
            source: SolverError::UnluckySample
        }),
        Some(RuleTrialOutcome::UnluckySample)
    );
    assert_eq!(
        optional_refusal(&SectorSolveError::<1>::NonProgress {
            case: Case::generic()
        }),
        Some(RuleTrialOutcome::NonProgress)
    );
    assert_eq!(
        optional_refusal(&SectorSolveError::<1>::Exceptions {
            case: Case::generic(),
            source: ExceptionError::NativeAlgebra
        }),
        None
    );
}

#[test]
fn bounded_rule_portfolio_broken_descent_after_valid_incumbent_is_fatal_not_fallback() {
    let context = CoefficientContext::new(["n"]);
    // A deliberately corrupted preconditioned row with duplicate heads must
    // trip the unchanged strict-descent gate even after a valid baseline.
    let system = SourceSystem::new(
        vec![
            row(&context, &[(0, "1"), (-1, "n-1")]),
            row(&context, &[(0, "1"), (0, "1")]),
        ],
        [0],
    )
    .unwrap();
    let solver = solver(&system, policy());
    let mut stats = SectorStats::default();
    let mut trials = Vec::new();
    let result = solver.select_rule(
        &Case::generic(),
        SectorSolveOptions::default(),
        &mut stats,
        &mut |event| {
            if let SectorEvent::RuleTrialFinished { summary, .. } = event {
                trials.push(summary);
            }
        },
    );
    assert!(matches!(
        result,
        Err(SectorSolveError::Search {
            source: SolverError::ExactReplay(_),
            ..
        })
    ));
    assert_eq!(trials.len(), 2);
    assert_eq!(trials[0].outcome, RuleTrialOutcome::Admitted);
    assert_eq!(trials[1].outcome, RuleTrialOutcome::Fatal);
    assert_eq!(stats.rule_selection.unwrap().fatal, 1);
    assert_eq!(stats.symbolic_rows, 2);
}

#[test]
fn bounded_rule_portfolio_search_caps_precede_second_row_and_exact_lift() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(
        vec![
            row(&context, &[(1, "1"), (0, "1")]),
            row(&context, &[(1, "1"), (-1, "1")]),
        ],
        [0],
    )
    .unwrap();
    let solver = solver(&system, policy());
    let case = Case::from(CoordinateCase::new([Some(1)]).unwrap());
    for (caps, expected, rows) in [
        (
            RuleTrialLimits {
                max_rows: 1,
                ..limits()
            },
            RuleTrialBudget::SourceRows,
            1,
        ),
        (
            RuleTrialLimits {
                max_exact_trace_rows: 1,
                ..limits()
            },
            RuleTrialBudget::ExactTraceRows,
            2,
        ),
        (
            RuleTrialLimits {
                max_exact_trace_terms: 1,
                ..limits()
            },
            RuleTrialBudget::ExactTraceTerms,
            2,
        ),
    ] {
        let mut exact = 0;
        let (result, work) = solver.search_attempt(
            case.clone(),
            SearchOptions::default(),
            None,
            Some(caps),
            true,
            |event| {
                if matches!(
                    event,
                    SearchEvent::ExactStarted { .. } | SearchEvent::CanonicalizationStarted { .. }
                ) {
                    exact += 1;
                }
            },
        );
        assert!(matches!(result, Err(TrialSearchError::Limit(kind)) if kind == expected));
        assert_eq!(work.search.rows, rows);
        assert_eq!(work.exact_lifts, 0);
        assert_eq!(exact, 0);
    }
    let (result, work) = solver.search_attempt(
        case,
        SearchOptions::default(),
        None,
        Some(limits()),
        true,
        |_| {},
    );
    let candidate = result.unwrap_or_else(|_| panic!("unrestricted fixture must lift"));
    assert_eq!(candidate.stats.exact_trace_rows, 2);
    assert_eq!(work.exact_trace_terms, 4);
    assert_eq!(work.exact_lifts, 1);
}

#[test]
fn bounded_rule_portfolio_depth_exhaustion_counts_failed_search_without_lift() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(
        vec![
            row(&context, &[(1, "1"), (0, "1")]),
            row(&context, &[(1, "1"), (0, "1")]),
        ],
        [0],
    )
    .unwrap();
    let solver = solver(&system, policy());
    let case = Case::from(CoordinateCase::new([Some(1)]).unwrap());
    let (result, work) = solver.search_attempt(
        case,
        SearchOptions::default(),
        None,
        Some(RuleTrialLimits {
            max_depth: 0,
            ..limits()
        }),
        true,
        |_| {},
    );
    assert!(matches!(
        result,
        Err(TrialSearchError::Solver(SolverError::SearchExhausted {
            depth: 0,
            rows: 2
        }))
    ));
    assert_eq!(work.search.rows, 2);
    assert_eq!(work.search.independent_rows, 1);
    assert_eq!(work.exact_lifts, 0);
}

#[test]
fn bounded_rule_portfolio_default_and_explicit_first_valid_have_no_trial_events() {
    let system = SourceSystem::<1>::from_family(&crate::solver::tests::tadpole()).unwrap();
    let baseline = SectorSolver::new(&system, [true], SectorConfig::default())
        .unwrap()
        .solve_sector(SectorSolveOptions::default())
        .unwrap();
    let solver = SectorSolver::new(
        &system,
        [true],
        SectorConfig {
            rule_selection: RuleSelectionPolicy::FirstValid,
            ..Default::default()
        },
    )
    .unwrap();
    let observed = solver
        .solve_sector_with_observer(SectorSolveOptions::default(), |event| {
            assert!(!matches!(event, SectorEvent::RuleTrialFinished { .. }));
        })
        .unwrap();
    assert!(observed.stats.rule_selection.is_none());
    assert_eq!(baseline.finite_residuals, observed.finite_residuals);
    assert_eq!(baseline.rules.len(), observed.rules.len());
    for (left, right) in baseline.rules.iter().zip(&observed.rules) {
        assert_eq!(left.candidate.case, right.candidate.case);
        assert_eq!(left.candidate.target, right.candidate.target);
        assert_eq!(left.candidate.rhs, right.candidate.rhs);
        assert_eq!(left.candidate.sources, right.candidate.sources);
        assert_eq!(left.exceptions.branches, right.exceptions.branches);
    }
}

#[test]
fn bounded_rule_portfolio_fixed_and_symbolic_excursions_use_actual_target_and_axis_role() {
    use crate::solver::Power;
    let p = |symbolic, value| Power::new(symbolic, value).unwrap();
    assert_eq!(
        quality::excursion(p(true, 2), p(true, -1), false).unwrap(),
        3
    );
    assert_eq!(
        quality::excursion(p(true, 2), p(true, -1), true).unwrap(),
        0
    );
    assert_eq!(
        quality::excursion(p(false, 2), p(false, -1), false).unwrap(),
        1
    );
    assert_eq!(
        quality::excursion(p(false, -2), p(false, 1), true).unwrap(),
        1
    );
    assert!(quality::excursion(p(true, 0), p(false, 0), true).is_err());
}

fn quality_candidate<const N: usize>(target: Integral<N>, rhs: Vec<Integral<N>>) -> Admitted<N> {
    let context = CoefficientContext::new(["n"]);
    Admitted {
        rule: SectorRule {
            dispatch_policy: Default::default(),
            candidate: RuleCandidate {
                case: Case::generic(),
                target,
                rhs: rhs
                    .into_iter()
                    .map(|integral| Term {
                        integral,
                        coefficient: context.one(),
                    })
                    .collect(),
                sources: Vec::new(),
                stats: SearchStats::default(),
            },
            exceptions: ExceptionalConditions {
                branches: Vec::new(),
            },
        },
        children: Vec::new(),
        discarded: 0,
        features: Features::default(),
    }
}

#[test]
fn bounded_rule_portfolio_total_positive_distinguishes_repeated_symbolic_increases() {
    let target = Integral::symbolic([2, -3, 5]).unwrap();
    let repeated = quality_candidate(
        target,
        vec![
            Integral::symbolic([3, -3, -9]).unwrap(),
            Integral::symbolic([2, -2, 9]).unwrap(),
            Integral::symbolic([1, -3, 5]).unwrap(),
        ],
    );
    let once = quality_candidate(
        target,
        vec![
            Integral::symbolic([3, -3, -9]).unwrap(),
            Integral::symbolic([1, -3, 9]).unwrap(),
            Integral::symbolic([2, -3, 5]).unwrap(),
        ],
    );
    let active = [true, true, false];
    let a = Features::read(&repeated, &active).unwrap();
    let b = Features::read(&once, &active).unwrap();
    assert_eq!(a.get(RuleQualityFeature::MaxPositiveShiftExcursion), 1);
    assert_eq!(b.get(RuleQualityFeature::MaxPositiveShiftExcursion), 1);
    assert_eq!(a.get(RuleQualityFeature::TotalPositiveShiftExcursion), 2);
    assert_eq!(b.get(RuleQualityFeature::TotalPositiveShiftExcursion), 1);
    assert_eq!(
        b.compare(
            &a,
            &[RuleQualityPriority {
                feature: RuleQualityFeature::TotalPositiveShiftExcursion,
                descending: false,
            }],
        ),
        Ordering::Less
    );
    assert_eq!(
        Features::read(&repeated, &[false; 3])
            .unwrap()
            .get(RuleQualityFeature::TotalPositiveShiftExcursion),
        0,
        "inactive displacements never contribute"
    );
    assert_eq!(
        Features::read(&repeated, &[true; 3])
            .unwrap()
            .get(RuleQualityFeature::TotalPositiveShiftExcursion),
        6,
        "all active axes contribute relative to the target, not zero"
    );
}

#[test]
fn bounded_rule_portfolio_total_positive_fixed_powers_use_positive_degree_difference() {
    let candidate = quality_candidate(
        Integral::numeric([2, -2, 5]).unwrap(),
        vec![
            Integral::numeric([-1, 1, 6]).unwrap(),
            Integral::numeric([4, -4, -1]).unwrap(),
        ],
    );
    let features = Features::read(&candidate, &[true, true, false]).unwrap();
    assert_eq!(
        features.get(RuleQualityFeature::MaxPositiveShiftExcursion),
        2
    );
    assert_eq!(
        features.get(RuleQualityFeature::TotalPositiveShiftExcursion),
        3
    );
    // -2 -> 1 creates one positive degree, not three; 2 -> 4 adds two,
    // not four. This score test is not a claim that the fixture is a rule.
}

#[test]
fn bounded_rule_portfolio_total_positive_exact_tie_retains_baseline() {
    let system = system();
    let mut policy = policy();
    let RuleSelectionPolicy::BoundedPortfolio { quality, .. } = &mut policy else {
        unreachable!()
    };
    *quality = vec![RuleQualityPriority {
        feature: RuleQualityFeature::TotalPositiveShiftExcursion,
        descending: false,
    }];
    let (selected, stats, _) = select(&solver(&system, policy));
    assert_eq!(selected.rule.candidate.sources[0].basis_row, 0);
    assert_eq!(selected.rule.candidate.rhs.len(), 2);
    assert_eq!(
        selected
            .features
            .get(RuleQualityFeature::TotalPositiveShiftExcursion),
        0
    );
    assert_eq!(stats.rule_selection.unwrap().selected_alternatives, 0);
}

#[test]
fn bounded_rule_portfolio_unpruned_children_and_guard_priorities_remain_distinct_from_rhs() {
    let context = CoefficientContext::new(["n"]);
    let candidate = |children: Vec<Case<1>>, rhs: usize| Admitted {
        rule: SectorRule {
            dispatch_policy: Default::default(),
            candidate: RuleCandidate {
                case: Case::generic(),
                target: Integral::symbolic([0]).unwrap(),
                rhs: (0..rhs)
                    .map(|_| Term {
                        integral: Integral::symbolic([-1]).unwrap(),
                        coefficient: context.one(),
                    })
                    .collect(),
                sources: Vec::new(),
                stats: SearchStats::default(),
            },
            exceptions: ExceptionalConditions {
                branches: Vec::new(),
            },
        },
        children,
        discarded: 0,
        features: Features::default(),
    };
    let child = Case::from(CoordinateCase::new([Some(2)]).unwrap());
    let worse = candidate(vec![child.clone(), child], 1);
    let better = candidate(Vec::new(), 2);
    let a = Features::read(&worse, &[true]).unwrap();
    let b = Features::read(&better, &[true]).unwrap();
    assert_eq!(
        a.get(RuleQualityFeature::ExceptionalCases),
        2,
        "unpruned OR count, not distinct union"
    );
    let priorities = [
        RuleQualityPriority {
            feature: RuleQualityFeature::ExceptionalCases,
            descending: false,
        },
        RuleQualityPriority {
            feature: RuleQualityFeature::RhsTerms,
            descending: false,
        },
    ];
    assert_eq!(b.compare(&a, &priorities), Ordering::Less);
}

#[test]
fn bounded_rule_portfolio_retains_exact_admitted_or_order_without_geometry_replay() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(
        vec![
            // The pivot coefficient must have a genuine zero to exercise
            // exceptional-case retention; a constant pivot has no such face.
            row(&context, &[(0, "n-2"), (-1, "1"), (-2, "1")]),
            row(&context, &[(0, "1"), (-1, "n-1")]),
        ],
        [0],
    )
    .unwrap();
    let mut policy = policy();
    let RuleSelectionPolicy::BoundedPortfolio { alternatives, .. } = &mut policy else {
        unreachable!()
    };
    *alternatives = vec![SourceDiscoveryStrategy::InputOrder];
    let solver = solver(&system, policy);
    let candidate = solver
        .solve_case(Case::generic(), SearchOptions::default())
        .unwrap();
    let (baseline, _) = solver.finish_rule(candidate).unwrap();
    let expected = baseline
        .admit_exceptional_cases_in_scope(&[0], &[true], None, Default::default())
        .unwrap();
    let (selected, stats, trials) = select(&solver);
    assert!(!expected.0.is_empty());
    assert_eq!(selected.children, expected.0);
    assert_eq!(selected.discarded, expected.1);
    assert_eq!(trials.len(), 1);
    assert_eq!(
        trials[0].stats.geometry_calls,
        selected.rule.exceptions.branches.len()
    );
    assert_eq!(
        stats.rule_selection.unwrap().total.geometry_calls,
        trials[0].stats.geometry_calls
    );
}

#[test]
fn bounded_rule_portfolio_invalid_policy_is_rejected_before_native_preparation() {
    let mut policies = Vec::new();
    let mut empty = policy();
    let RuleSelectionPolicy::BoundedPortfolio { alternatives, .. } = &mut empty else {
        unreachable!()
    };
    alternatives.clear();
    policies.push(empty);
    let mut zero = policy();
    let RuleSelectionPolicy::BoundedPortfolio { limits, .. } = &mut zero else {
        unreachable!()
    };
    limits.max_rows = 0;
    policies.push(zero);
    let mut trigger = policy();
    let RuleSelectionPolicy::BoundedPortfolio { trigger: value, .. } = &mut trigger else {
        unreachable!()
    };
    *value = RulePortfolioTrigger::AnyAtLeast(Vec::new());
    policies.push(trigger);
    let system = system();
    for policy in policies {
        let result = SectorSolver::new(
            &system,
            [true],
            SectorConfig {
                rule_selection: policy,
                ..Default::default()
            },
        );
        assert!(matches!(result, Err(SolverError::InvalidInput(_))));
    }
}

#[test]
fn bounded_rule_portfolio_constructor_materializes_alternatives_on_the_same_basis() {
    use crate::solver::{SourceRowFeature, SourceRowPriority};
    let system = SourceSystem::<1>::from_family(&crate::solver::tests::tadpole()).unwrap();
    let baseline = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let configured = SectorSolver::new(
        &system,
        [true],
        SectorConfig {
            rule_selection: RuleSelectionPolicy::BoundedPortfolio {
                alternatives: vec![SourceDiscoveryStrategy::Features(vec![SourceRowPriority {
                    feature: SourceRowFeature::Terms,
                    descending: false,
                }])],
                limits: limits(),
                quality: vec![RuleQualityPriority {
                    feature: RuleQualityFeature::RhsTerms,
                    descending: false,
                }],
                trigger: RulePortfolioTrigger::Always,
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(configured.basis(), baseline.basis());
    assert_eq!(configured.ordering(), baseline.ordering());
    let RuleSelectionPolicy::BoundedPortfolio { alternatives, .. } =
        &configured.config.rule_selection
    else {
        unreachable!()
    };
    let SourceDiscoveryStrategy::Materialized(plan) = &alternatives[0] else {
        panic!("materialize once, not per case")
    };
    assert_eq!(plan.ordinals().len(), configured.basis().len());
    let result = configured
        .solve_sector(SectorSolveOptions::default())
        .unwrap();
    assert!(result.stats.rule_selection.is_some());
}
