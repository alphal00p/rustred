use super::*;
use crate::reduction::ReductionLimits;
use crate::sector::CoordinatePriority;
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::candidate_reduction::preparation::shared::PREPARATION_COUNT;
use crate::solver::{
    CoordinateCase, Integral, IntegralOrder, RoutedCandidateReducer, SectorRule, SectorStats,
};
use std::sync::atomic::AtomicBool;

fn scope(rank: u32) -> OwnerDomainScope {
    OwnerDomainScope {
        max_numerator_rank: Some(rank),
        finite_case_policy: FiniteCasePolicy::SearchFinite,
    }
}

#[test]
fn retained_guarded_rule_chart_is_in_prepared_overlay_byte_admission() {
    use crate::algebra::polynomial_clone_owned_heap_byte_bound;
    use crate::solver::{
        AffineCase, AffineIntersection, ExceptionalConditions, RuleCandidate, SearchStats,
    };
    let base = programs(
        Arc::new(crate::solver::tests::sunset()),
        Some(10),
        vec![input([true; 3], Some(10), vec![], &[])],
        Default::default(),
    );
    let c = base.context.coefficient_context();
    let equation = c
        .sub(&c.index(0).unwrap(), &c.index(1).unwrap())
        .unwrap()
        .raw()
        .numerator
        .clone();
    let AffineIntersection::Affine(affine) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &[equation],
        base.context.index_variables(),
        &[true; 3],
    )
    .unwrap() else {
        panic!("affine fixture");
    };
    let chart_bytes = affine.native_payload_bytes().unwrap();
    let case: Case<3> = affine.into();
    let make = || {
        synthetic(
            &base,
            [true; 3],
            vec![SectorRule {
                candidate: RuleCandidate {
                    target: case.integral(),
                    case: case.clone(),
                    rhs: vec![],
                    sources: vec![],
                    stats: SearchStats::default(),
                },
                exceptions: ExceptionalConditions::default(),
            }],
            &[],
        )
    };
    assert_eq!(
        make()
            .raw_payload_usage(Default::default())
            .unwrap()
            .native_bytes,
        chart_bytes
    );
    // Raw admission fits, but preparation retains BOTH the original chart and
    // the authenticated equality polynomials used by ordinary matching.
    assert!(
        base.append_domain_overlays(
            vec![make()],
            OwnerOverlayLimits {
                max_native_bytes: chart_bytes,
                ..Default::default()
            }
        )
        .is_err()
    );
    let next = base
        .append_domain_overlays(vec![make()], Default::default())
        .unwrap();
    // Preparation clones the chart equations before sealing them. Cloning can
    // change exposed polynomial buffer capacities, so account the actually
    // retained value rather than the original chart equation's byte envelope.
    let prepared = &next.owners[&[true; 3]].batches.last().unwrap().rules[0];
    let equality_bytes: usize = prepared
        .equalities
        .iter()
        .map(|p| {
            size_of::<crate::algebra::CoefficientPolynomial>()
                + polynomial_clone_owned_heap_byte_bound(p.raw()).unwrap()
        })
        .sum();
    assert!(equality_bytes > 0);
    let expected = chart_bytes + equality_bytes;
    assert_eq!(next.overlay_usage().native_bytes, expected);
    assert!(matches!(
        base.append_domain_overlays(
            vec![make()],
            OwnerOverlayLimits {
                max_native_bytes: expected - 1,
                ..Default::default()
            }
        ),
        Err(OwnerFeedbackError::ResourceLimit {
            resource: "native bytes",
            requested,
            limit,
        }) if requested == expected && limit == expected - 1
    ));
    assert_eq!(
        base.append_domain_overlays(
            vec![make()],
            OwnerOverlayLimits {
                max_native_bytes: expected,
                ..Default::default()
            }
        )
        .unwrap()
        .overlay_usage()
        .native_bytes,
        expected
    );
}
fn tadpole(
    rules: Vec<SectorRule<1>>,
    terminals: &[[i16; 1]],
    limits: ReductionLimits,
) -> Arc<CandidateOwnerPrograms<1>> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut owner = input([true], Some(10), rules, terminals);
    owner.ordering = OrderingPolicy::SpiredUncutV1;
    programs(family, Some(10), vec![owner], limits)
}
fn search<const N: usize>(
    programs: &Arc<CandidateOwnerPrograms<N>>,
    sector: [bool; N],
) -> BoundOwnerSearch<N> {
    programs
        .bind_owner_search(
            sector,
            OwnerFeedbackPolicy {
                numerical_depth: 0,
                ..Default::default()
            },
        )
        .unwrap()
}
fn real(programs: &Arc<CandidateOwnerPrograms<1>>) -> BoundOwnerOverlay<1> {
    search(programs, [true])
        .solve_domains_with_observer(vec![Case::generic()], scope(10), Default::default(), |_| {})
        .unwrap()
}
// Private synthetic partial records test dispatch/admission mechanics ONLY.
// Production callers cannot construct this result or attach arbitrary sources.
fn synthetic<const N: usize>(
    programs: &Arc<CandidateOwnerPrograms<N>>,
    sector: [bool; N],
    rules: Vec<SectorRule<N>>,
    terminals: &[[i16; N]],
) -> BoundOwnerOverlay<N> {
    let owner = &programs.owners[&sector];
    BoundOwnerOverlay {
        lineage: programs.lineage.clone(),
        sector,
        root: owner.root,
        ordering: owner.ordering,
        policy: Default::default(),
        attempt_limits: Default::default(),
        solution: SectorDomainSolution {
            requested_cases: vec![Case::generic()],
            max_numerator_rank: Some(10),
            finite_case_policy: FiniteCasePolicy::SearchFinite,
            rules,
            finite_residuals: terminals
                .iter()
                .map(|p| Integral::numeric(*p).unwrap())
                .collect(),
            stats: SectorStats::default(),
        },
    }
}
fn reducer<const N: usize>(programs: Arc<CandidateOwnerPrograms<N>>) -> RoutedCandidateReducer<N> {
    RoutedCandidateReducer::try_new(programs, [], Default::default()).unwrap()
}

#[test]
fn real_bound_domain_search_appends_without_forging_a_sector_and_reuses_sources() {
    let before = PREPARATION_COUNT.with(|counter| counter.get());
    let base = tadpole(vec![], &[], Default::default());
    let old = reducer(base.clone()).trace_targets([key([3])]).unwrap();
    assert_eq!(old.frontier().len(), 1);
    let result = real(&base);
    assert!(result.rule_count() > 0);
    assert_eq!(result.terminal_count(), 1);
    assert_eq!(result.requested_cases(), &[Case::generic()]);
    assert_eq!(result.policy().numerical_depth, 0);
    let raw_usage = result.raw_payload_usage(Default::default()).unwrap();
    assert_eq!(raw_usage.rules, result.rule_count());
    assert!(raw_usage.native_bytes > 0);
    let next = base
        .append_domain_overlays(vec![result], Default::default())
        .unwrap();
    assert_eq!(PREPARATION_COUNT.with(|counter| counter.get()), before + 1);
    assert!(Arc::ptr_eq(
        &base.context.shared.sources,
        &next.context.shared.sources
    ));
    assert!(Arc::ptr_eq(
        &base.owners[&[true]].batches[0],
        &next.owners[&[true]].batches[0]
    ));
    assert_eq!(base.owners[&[true]].batches.len(), 1);
    assert_eq!(next.overlays(&[true]).count(), 1);
    let reduce = reducer(next);
    let serial = reduce.trace_targets([key([3]), key([2])]).unwrap();
    assert!(serial.frontier().is_empty());
    assert_eq!(serial.rule_applications(), 2);
    assert!(serial.declared_terminals().contains(&key([1])));
    for workers in [1, 2, 6] {
        let parallel = reduce
            .trace_targets_parallel_with_observer(
                [key([3]), key([2])],
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(parallel.trace(), &serial);
    }
    // Publication did not mutate the original frontier or expression library.
    assert_eq!(reducer(base).trace_targets([key([3])]).unwrap(), old);
}

#[test]
fn batch_local_terminals_preserve_base_and_earlier_overlay_rules() {
    let family = crate::solver::tests::tadpole();
    let base = tadpole(
        vec![rule(&family, [2], &[([1], 1)])],
        &[[1]],
        Default::default(),
    );
    let next = base
        .append_domain_overlays(
            vec![synthetic(
                &base,
                [true],
                vec![rule(&family, [3], &[([2], 1)])],
                &[[2]],
            )],
            Default::default(),
        )
        .unwrap();
    let last = next
        .append_domain_overlays(
            vec![synthetic(
                &next,
                [true],
                vec![rule(&family, [1], &[([1], 1)])],
                &[[3]],
            )],
            Default::default(),
        )
        .unwrap();
    let trace = reducer(last).trace_targets([key([3])]).unwrap();
    assert_eq!(trace.rule_applications(), 2);
    assert_eq!(
        trace.declared_terminals().iter().collect::<Vec<_>>(),
        vec![&key([1])]
    );
    assert!(trace.frontier().is_empty());
}

#[test]
fn earlier_native_descent_error_does_not_fall_through_to_overlay_terminal() {
    let family = crate::solver::tests::tadpole();
    let base = tadpole(
        vec![rule(&family, [2], &[([2], 1)])],
        &[],
        Default::default(),
    );
    let next = base
        .append_domain_overlays(
            vec![synthetic(&base, [true], vec![], &[[2]])],
            Default::default(),
        )
        .unwrap();
    let reduce = reducer(next);
    assert!(reduce.trace_targets([key([2])]).is_err());
    assert!(
        reduce
            .trace_targets_parallel_with_observer([key([2])], 2, &AtomicBool::new(false), |_| {})
            .is_err()
    );
}

#[test]
fn later_large_coalescing_reservation_does_not_change_base_budget_behavior() {
    let family = crate::solver::tests::tadpole();
    let base = tadpole(
        vec![rule(&family, [2], &[([1], 1)])],
        &[[1]],
        ReductionLimits {
            max_coalescing_additions: 0,
            ..Default::default()
        },
    );
    let next = base
        .append_domain_overlays(
            vec![synthetic(
                &base,
                [true],
                vec![rule(&family, [3], &[([1], 1), ([1], 1)])],
                &[],
            )],
            Default::default(),
        )
        .unwrap();
    let reduce = reducer(next);
    let outcome = reduce
        .trace_targets_parallel_with_observer([key([2])], 2, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(outcome.trace().rule_applications(), 1);
    assert_eq!(outcome.snapshot().coalescing_additions, 0);
    assert!(
        reduce
            .trace_targets_parallel_with_observer([key([3])], 2, &AtomicBool::new(false), |_| {})
            .is_err()
    );
}

#[test]
fn overlay_terminal_is_one_attempt_but_not_a_rule_application() {
    let base = tadpole(vec![], &[], Default::default());
    let next = base
        .append_domain_overlays(
            vec![synthetic(&base, [true], vec![], &[[2]])],
            Default::default(),
        )
        .unwrap();
    let reduce = reducer(next);
    let result = reduce
        .trace_targets_parallel_with_observer([key([2])], 2, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(result.snapshot().rule_attempts, 1);
    assert_eq!(result.trace().rule_applications(), 0);
    assert_eq!(result.trace(), &reduce.trace_targets([key([2])]).unwrap());
}

#[test]
fn wrong_lineage_or_order_cannot_install_even_with_the_same_family() {
    let base = tadpole(vec![], &[], Default::default());
    let foreign = tadpole(vec![], &[], Default::default());
    assert!(
        foreign
            .append_domain_overlays(vec![real(&base)], Default::default())
            .is_err()
    );
    let mut wrong = synthetic(&base, [true], vec![], &[[1]]);
    wrong.ordering = OrderingPolicy::RustRedUnshiftedV1;
    assert!(
        base.append_domain_overlays(vec![wrong], Default::default())
            .is_err()
    );
    assert_eq!(base.overlays(&[true]).count(), 0);
}

#[test]
fn unsupported_order_and_missing_owner_are_rejected_before_search() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let base = programs(
        family,
        Some(10),
        vec![input([true], Some(10), vec![], &[])],
        Default::default(),
    );
    assert!(base.bind_owner_search([true], Default::default()).is_err());
    assert!(base.bind_owner_search([false], Default::default()).is_err());
}

#[test]
fn noninvolutive_priority_is_inverted_for_the_native_solver() {
    let family = Arc::new(crate::solver::tests::sunset());
    let priority = CoordinatePriority::try_new(3, &[2, 0, 1], Default::default()).unwrap();
    let ordering = OrderingPolicy::try_spired_with_coordinate_priority(&priority).unwrap();
    let mut owner = input([true; 3], Some(10), vec![], &[]);
    owner.ordering = ordering;
    let base = programs(family, Some(10), vec![owner], Default::default());
    let bound = search(&base, [true; 3]);
    assert_eq!(bound.permutation, Some([1, 2, 0]));
    let native = IntegralOrder::new([true; 3], [false; 3])
        .with_permutation(bound.permutation.unwrap())
        .unwrap();
    for a in [[1, 2, 3], [2, 1, 3], [3, 2, 1]] {
        for b in [[1, 2, 3], [2, 1, 3], [3, 2, 1]] {
            assert_eq!(
                native
                    .compare(
                        &Integral::numeric(a).unwrap(),
                        &Integral::numeric(b).unwrap()
                    )
                    // Native source order is harder-first; persisted candidate
                    // complexity is simpler-first. Compare the same orientation.
                    .reverse(),
                ordering
                    .compare(&a.map(i64::from), &b.map(i64::from))
                    .unwrap()
            );
        }
    }
}

#[test]
fn real_rank_eleven_domain_does_not_clip_descendants_or_relabel_rank_ten() {
    let family = Arc::new(crate::solver::tests::sunset());
    let sector = [true, true, false];
    let mut owner = input(
        sector,
        Some(10),
        vec![rule(&family, [1, 3, -10], &[([1, 1, -11], 1)])],
        &[],
    );
    owner.ordering = OrderingPolicy::SpiredUncutV1;
    let base = programs(family, Some(10), vec![owner], Default::default());
    let bound = search(&base, sector);
    let result = bound
        .solve_domains_with_observer(
            vec![
                CoordinateCase::new([Some(1), Some(1), Some(-11)])
                    .unwrap()
                    .into(),
            ],
            OwnerDomainScope {
                max_numerator_rank: Some(11),
                finite_case_policy: FiniteCasePolicy::RetainRankFinite,
            },
            Default::default(),
            |_| {},
        )
        .unwrap();
    assert_eq!(result.terminal_count(), 1);
    assert_eq!(result.scope().max_numerator_rank, Some(11));
    let next = base
        .append_domain_overlays(vec![result], Default::default())
        .unwrap();
    assert_eq!(next.context().scope().max_numerator_rank, Some(10));
    let reduce = reducer(next);
    let trace = reduce.trace_targets([key([1, 3, -10])]).unwrap();
    assert_eq!(trace.max_negative_index_degree(), 11);
    assert!(trace.frontier().is_empty());
    assert!(reduce.trace_targets([key([1, 1, -11])]).is_err());
    // An independently admitted finite root uses this actual R11 overlay,
    // without relabeling the saved R10 context or regenerating its owners.
    let admission = crate::solver::FiniteRootAdmission::try_new(
        [crate::solver::RootRegionInput {
            support: sector,
            lower: vec![0, 0, 11],
            upper: vec![Some(0), Some(0), Some(11)],
            rank: Some(11),
            powers: Default::default(),
        }],
        1,
    )
    .unwrap();
    let policy = crate::solver::CandidateEntryAdmission::ExplicitFinite(&admission);
    let explicit = reduce
        .trace_targets_with_entry_admission([key([1, 1, -11])], policy)
        .unwrap();
    assert!(explicit.frontier().is_empty());
    assert_eq!(
        explicit.declared_terminals(),
        &std::collections::BTreeSet::from([key([1, 1, -11])])
    );
    for workers in [1, 2, 6] {
        let parallel = reduce
            .trace_targets_parallel_with_entry_admission_and_observer(
                [key([1, 1, -11])],
                policy,
                workers,
                &std::sync::atomic::AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(parallel.trace(), &explicit);
    }
    assert_eq!(
        reduce.programs().context().scope().max_numerator_rank,
        Some(10)
    );
}

#[test]
fn cumulative_limits_and_failed_atomic_append_leave_old_snapshot_usable() {
    let base = tadpole(vec![], &[[1]], Default::default());
    let next = base
        .append_domain_overlays(
            vec![synthetic(&base, [true], vec![], &[[2]])],
            OwnerOverlayLimits {
                max_batches: 1,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        next.append_domain_overlays(
            vec![synthetic(&next, [true], vec![], &[[3]])],
            OwnerOverlayLimits {
                max_batches: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        base.append_domain_overlays(
            vec![real(&base)],
            OwnerOverlayLimits {
                max_native_bytes: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(next.overlays(&[true]).count(), 1);
    assert!(
        reducer(next)
            .trace_targets([key([2])])
            .unwrap()
            .frontier()
            .is_empty()
    );
    assert_eq!(base.overlays(&[true]).count(), 0);
}

#[test]
fn source_limits_invalid_inputs_and_symbolic_terminals_fail_closed() {
    let base = tadpole(vec![], &[], Default::default());
    let bound = search(&base, [true]);
    let mut events = 0;
    assert!(
        bound
            .solve_domains_with_observer(
                vec![Case::generic()],
                scope(10),
                OwnerDomainAttemptLimits {
                    max_requested_cases: 0,
                    ..Default::default()
                },
                |_| events += 1
            )
            .is_err()
    );
    assert_eq!(events, 0);
    assert!(
        bound
            .solve_domains_with_observer(
                vec![Case::generic()],
                scope(10),
                OwnerDomainAttemptLimits {
                    max_symbolic_cases: Some(0),
                    ..Default::default()
                },
                |_| {}
            )
            .is_err()
    );
    assert!(
        bound
            .solve_domains_with_observer(
                vec![CoordinateCase::new([Some(0)]).unwrap().into()],
                scope(10),
                Default::default(),
                |_| {}
            )
            .is_err()
    );
    let mut bad = synthetic(&base, [true], vec![], &[]);
    bad.solution
        .finite_residuals
        .push(Integral::symbolic([0]).unwrap());
    assert!(
        base.append_domain_overlays(vec![bad], Default::default())
            .is_err()
    );
}

#[test]
fn untouched_owners_and_all_base_batches_are_arc_shared_after_publication() {
    let family = Arc::new(crate::solver::tests::sunset());
    let base = programs(
        family,
        Some(10),
        vec![
            input([true; 3], Some(10), vec![], &[[1, 1, 1]]),
            input([true, true, false], Some(10), vec![], &[[1, 1, 0]]),
        ],
        Default::default(),
    );
    let next = base
        .append_domain_overlays(
            vec![synthetic(&base, [true; 3], vec![], &[[2, 1, 1]])],
            Default::default(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        &base.owners[&[true, true, false]],
        &next.owners[&[true, true, false]]
    ));
    assert!(Arc::ptr_eq(
        &base.owners[&[true; 3]].batches[0],
        &next.owners[&[true; 3]].batches[0]
    ));
    assert!(Arc::ptr_eq(base.context(), next.context()));
}

#[test]
fn routed_rebind_requires_the_exact_append_only_prefix() {
    let base = tadpole(vec![], &[[1]], Default::default());
    let initial = reducer(base.clone());
    let next = base
        .append_domain_overlays(
            vec![synthetic(&base, [true], vec![], &[[2]])],
            Default::default(),
        )
        .unwrap();
    let updated = initial.with_programs(next.clone()).unwrap();
    assert!(
        updated
            .trace_targets([key([2])])
            .unwrap()
            .frontier()
            .is_empty()
    );
    assert!(updated.with_programs(base.clone()).is_err());
    let fork = base
        .append_domain_overlays(
            vec![synthetic(&base, [true], vec![], &[[3]])],
            Default::default(),
        )
        .unwrap();
    assert!(updated.with_programs(fork).is_err());
    assert!(
        initial
            .with_programs(tadpole(vec![], &[[1]], Default::default()))
            .is_err()
    );
    assert!(Arc::ptr_eq(updated.programs(), &next));
}

#[test]
fn retained_affine_metadata_counts_native_matrices_and_both_chart_kinds() {
    use crate::algebra::polynomial_clone_owned_heap_byte_bound;
    use crate::solver::{AffineCase, AffineIntersection};
    let family = Arc::new(crate::solver::tests::sunset());
    let base = programs(
        family,
        Some(10),
        vec![input([true; 3], Some(10), vec![], &[])],
        Default::default(),
    );
    let c = base.context.coefficient_context();
    for multiplier in [1, 2] {
        let equality = c
            .sub(
                &c.mul(&c.integer(multiplier), &c.index(0).unwrap()).unwrap(),
                &c.index(1).unwrap(),
            )
            .unwrap()
            .raw()
            .numerator
            .clone();
        let AffineIntersection::Affine(affine) = AffineCase::from_coordinate(
            &CoordinateCase::new([None, None, Some(1)]).unwrap(),
            &[equality],
            base.context.index_variables(),
            &[true; 3],
        )
        .unwrap() else {
            panic!("affine fixture");
        };
        assert_eq!(affine.has_integral_chart(), multiplier == 1);
        let equation_bytes: usize = affine
            .equations()
            .iter()
            .map(|p| {
                size_of::<crate::algebra::CoefficientPolynomial>()
                    + polynomial_clone_owned_heap_byte_bound(p).unwrap()
            })
            .sum();
        let all_bytes = affine.native_payload_bytes().unwrap();
        assert!(all_bytes > equation_bytes);
        let case: Case<3> = affine.into();
        let make = || {
            let mut result = synthetic(&base, [true; 3], vec![], &[]);
            result.solution.requested_cases = vec![case.clone()];
            result
        };
        assert_eq!(
            make()
                .raw_payload_usage(Default::default())
                .unwrap()
                .native_bytes,
            all_bytes
        );
        assert!(
            base.append_domain_overlays(
                vec![make()],
                OwnerOverlayLimits {
                    max_native_bytes: all_bytes - 1,
                    ..Default::default()
                }
            )
            .is_err()
        );
        let next = base
            .append_domain_overlays(
                vec![make()],
                OwnerOverlayLimits {
                    max_native_bytes: all_bytes,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(next.overlay_usage().native_bytes, all_bytes);
        assert!(
            next.append_domain_overlays(
                vec![],
                OwnerOverlayLimits {
                    max_native_bytes: all_bytes - 1,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
