//! Fresh-root storage optimization, never a second rule evaluator.
use super::*;
use crate::reduction::{CacheWeight, SharedCacheBudget, coefficient_cache_weight};
use crate::solver::CandidateCacheRepresentation;
use crate::solver::candidate_reduction::owner_test_support::{input, rule};

fn guarded_fixture(shared_and_cancelled: bool) -> CandidateReducer<1> {
    let family = crate::solver::tests::tadpole();
    let rules = if shared_and_cancelled {
        vec![
            rule(&family, [5], &[([4], 1), ([3], -1)]),
            rule(&family, [4], &[([2], 1)]),
            rule(&family, [3], &[([2], 1)]),
            rule(&family, [2], &[([1], 1), ([1], -1)]),
        ]
    } else {
        (2..=24)
            .map(|n| rule(&family, [n], &[([n - 1], 1)]))
            .collect()
    };
    let record = input([true], None, rules, &[[1]]);
    let mut source = CandidateReducer::try_new(
        &family,
        [true],
        record.ordering,
        [([true], record.solution)],
        vec![],
        Default::default(),
    )
    .unwrap();
    let indexed = source.program.context.clone();
    let polynomial = |n| {
        indexed
            .lift_base_polynomial(
                &family
                    .coefficient_context()
                    .coefficient_fixture(&format!("d-{n}"))
                    .numerator,
            )
            .unwrap()
    };
    let program = Arc::get_mut(&mut source.program).unwrap();
    program.source_conditions.push(polynomial(100));
    for (ordinal, rule) in program
        .rules
        .get_mut(&[true])
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        rule.rhs[0].denominator = polynomial(ordinal + 2);
        rule.exceptions = vec![vec![polynomial(ordinal + 50), polynomial(ordinal + 150)]];
    }
    source
}

#[test]
fn fresh_root_matches_legacy_ordered_guards_terms_and_work_with_shared_cancelled_children() {
    for cancelled in [false, true] {
        for representation in [
            CandidateCacheRepresentation::Sparse,
            CandidateCacheRepresentation::Factorized,
        ] {
            let mut source = guarded_fixture(cancelled);
            source.set_cache_representation(representation).unwrap();
            let target = key([if cancelled { 5 } else { 24 }]);
            let mut legacy = source.fork_empty();
            let expected = legacy
                .reduce_unit_mass_guarded(&target, Default::default())
                .unwrap();
            let mut fresh = source.fork_empty();
            let actual = fresh
                .reduce_fresh_unit_mass_guarded(&target, Default::default())
                .unwrap();
            assert_eq!(
                actual, expected,
                "exact ordered guards and native work, not a set comparison"
            );
            assert_eq!(actual.0.is_zero(), cancelled);
            assert!(!actual.1.is_empty());
            assert!(fresh.cache.values().all(|entry| entry.conditions.is_none()));
            assert!(
                legacy
                    .cache
                    .values()
                    .all(|entry| entry.conditions.is_some())
            );
            assert!(
                fresh.statistics().cached_coefficient_terms()
                    < legacy.statistics().cached_coefficient_terms()
            );
            // Legacy subtree API remains capable of recomputing guardless
            // memo entries rather than treating them as guarded authority.
            let repeated = fresh
                .reduce_unit_mass_guarded(&target, Default::default())
                .unwrap();
            assert_eq!(repeated.0, expected.0);
            assert_eq!(repeated.1, expected.1);
            assert!(repeated.2.rule_applications() > 0);
        }
    }
}

#[test]
fn fresh_root_succeeds_under_same_allowance_that_refuses_duplicated_subtree_guards() {
    let source = guarded_fixture(false);
    let target = key([24]);
    let mut legacy = source.fork_empty();
    let expected = legacy
        .reduce_unit_mass_guarded(&target, Default::default())
        .unwrap();
    let mut ordinary = source.fork_empty();
    assert_eq!(ordinary.reduce_unit_mass(&target).unwrap(), expected.0);
    let unique = coefficient_cache_weight(expected.1.iter()).unwrap();
    let allowed = ordinary.statistics().cached_coefficient_terms() + unique.coefficient_terms;
    assert!(allowed < legacy.statistics().cached_coefficient_terms());
    let limits = ReductionLimits {
        max_cached_coefficient_terms: allowed,
        ..Default::default()
    };
    let mut old_bounded = source.fork_empty_with_limits(limits);
    assert!(matches!(
        old_bounded.reduce_unit_mass_guarded(&target, limits),
        Err(CandidateReductionError::Application(
            ReductionError::CacheCoefficientTermLimit { .. }
        ))
    ));
    let mut fresh = source.fork_empty_with_limits(limits);
    assert_eq!(
        fresh
            .reduce_fresh_unit_mass_guarded(&target, limits)
            .unwrap(),
        expected
    );
    assert_eq!(
        fresh.statistics().cached_coefficient_terms(),
        ordinary.statistics().cached_coefficient_terms()
    );
}

#[test]
fn fresh_root_refuses_nonempty_memo_without_modifying_it() {
    let mut source = guarded_fixture(false);
    source.reduce_unit_mass(&key([1])).unwrap();
    let before = source.statistics();
    assert!(matches!(
        source.reduce_fresh_unit_mass_guarded(&key([24]), Default::default()),
        Err(CandidateReductionError::Application(
            ReductionError::ReducerInvariant { .. }
        ))
    ));
    assert_eq!(source.statistics(), before);
}

#[test]
fn fresh_root_keeps_source_guards_on_native_zero_and_terminal_roots() {
    let family = crate::solver::tests::tadpole();
    let mut source = candidate::<1>(&family, Default::default());
    let d = family.coefficient_context().coefficient_fixture("d-17");
    let guard = source
        .program
        .context
        .lift_base_polynomial(&d.numerator)
        .unwrap();
    Arc::get_mut(&mut source.program)
        .unwrap()
        .source_conditions
        .push(guard);
    for target in [key([0]), key([1])] {
        let expected = source
            .fork_empty()
            .reduce_unit_mass_guarded(&target, Default::default())
            .unwrap();
        let actual = source
            .fork_empty()
            .reduce_fresh_unit_mass_guarded(&target, Default::default())
            .unwrap();
        assert_eq!(actual, expected);
        assert!(actual.1.contains(&d));
    }
}

#[test]
fn fresh_guard_lease_shares_budget_and_cleans_up_on_refusal_and_panic() {
    let source = guarded_fixture(false);
    for panic in [false, true] {
        let shared = Arc::new(SharedCacheBudget::default());
        let one = source.program.context.base().one();
        let outer = coefficient_cache_weight([&one, &one, &one]).unwrap();
        shared
            .replace(0, CacheWeight::default(), 1, outer, Default::default())
            .unwrap();
        let before = shared.snapshot();
        let limits = ReductionLimits {
            max_cached_coefficient_terms: 12,
            ..Default::default()
        };
        if panic {
            shared.set_pressure_handler(Arc::new(|| panic!("fresh root guard pressure unwind")));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut fresh = source.fork_empty_with_shared_budget(limits, Arc::clone(&shared));
            fresh.reduce_fresh_unit_mass_guarded(&key([24]), limits)
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(
            shared.snapshot(),
            before,
            "only the request's guard/memo ownership releases"
        );
    }
}
