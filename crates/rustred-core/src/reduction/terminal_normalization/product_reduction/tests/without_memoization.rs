use super::*;
use crate::reduction::ReductionLimits;

#[test]
fn repeated_factor_occurrences_and_requests_never_reuse_completed_results() {
    let f = family("occurrence-products", 1);
    let c = f.coefficient_context();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut reducer = VacuumProductReducer::try_new_without_memoization(
        PolynomialProvider {
            calls: calls.clone(),
        },
        f.clone(),
        VacuumProductReductionLimits {
            reduction: ReductionLimits {
                max_cached_integrals: 0,
                max_cached_coefficient_terms: 0,
                max_cached_coefficient_bytes: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let input = || [(vec![factor(&f, [3]), factor(&f, [3])], c.one())];
    let mut memoized = VacuumProductReducer::try_new(
        PolynomialProvider {
            calls: Default::default(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    let expected = memoized.reduce_products(input(), []).unwrap();
    for repeat in 1..=2 {
        let actual = reducer.reduce_products(input(), []).unwrap();
        assert_eq!(actual.terms(), expected.terms());
        assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
        assert_eq!(actual.statistics().provider_calls, 2);
        assert_eq!(actual.statistics().cache_hits, 0);
        assert_eq!(actual.statistics().provider_cache_hits, 0);
        assert_eq!(actual.statistics().zero_cache_hits, 0);
        assert_eq!(calls.lock().unwrap().len(), repeat * 2);
        assert_eq!(reducer.cache_statistics().cached_factors, 0);
        assert_eq!(reducer.cache_statistics().cached_zero_supports, 0);
        assert_eq!(reducer.cache_statistics().coefficient_terms, 0);
        assert_eq!(reducer.cache_statistics().coefficient_bytes, 0);
    }
}

#[test]
fn repeated_recursive_lower_factors_are_consumed_once_per_occurrence() {
    let f = family("occurrence-recursive", 3);
    let c = f.coefficient_context();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut reducer = VacuumProductReducer::try_new_without_memoization(
        RecursiveProvider {
            calls: calls.clone(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    let mut memoized = VacuumProductReducer::try_new(
        RecursiveProvider {
            calls: Default::default(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    let input = || [(vec![factor(&f, [1, 1, 1, 1, 1, 1])], c.one())];
    let actual = reducer.reduce_products(input(), []).unwrap();
    let expected = memoized.reduce_products(input(), []).unwrap();
    assert_eq!(*calls.lock().unwrap(), [3, 1, 1, 1]);
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(actual.statistics().provider_calls, 4);
    assert_eq!(actual.statistics().zero_cache_hits, 0);
    assert_eq!(reducer.cache_statistics().cached_factors, 0);
}

#[test]
fn no_memoization_combination_preserves_original_poles_and_actual_shared_work() {
    let f = family("occurrence-weighted", 1);
    let c = f.coefficient_context();
    let mut remaining = Vec::new();
    let result = reduce_vacuum_factor_combination_without_memoization(
        f.clone(),
        [
            (key([3]), c.coefficient_fixture("1/(d-4)")),
            (key([4]), c.coefficient_fixture("-1/(d-4)")),
        ],
        [],
        ReductionLimits {
            max_rule_applications: 2,
            max_cached_integrals: 0,
            max_cached_coefficient_terms: 0,
            max_cached_coefficient_bytes: 0,
            ..Default::default()
        },
        |_, limits| -> Result<_, String> {
            remaining.push(limits.max_rule_applications);
            Ok(VacuumFactorReduction {
                terms: vec![term(&f, [1], 1)],
                nonzero_conditions: vec![c.coefficient_fixture("d-7")],
                work: VacuumFactorWork {
                    rule_applications: 1,
                    ..Default::default()
                },
            })
        },
    )
    .unwrap();
    assert_eq!(remaining, [2, 1]);
    assert!(result.terms.is_empty());
    assert_eq!(result.work.rule_applications, 2);
    assert_eq!(result.work.coalescing_additions, 1);
    assert_eq!(result.work.cache_hits, 0);
    assert!(
        result
            .nonzero_conditions
            .contains(&c.coefficient_fixture("d-4"))
    );
    assert!(
        result
            .nonzero_conditions
            .contains(&c.coefficient_fixture("d-7"))
    );

    let zero = reduce_vacuum_factor_combination_without_memoization(
        f.clone(),
        [
            (key([3]), c.coefficient_fixture("1/(d-4)")),
            (key([3]), c.coefficient_fixture("-1/(d-4)")),
        ],
        [],
        Default::default(),
        |_, _| -> Result<VacuumFactorReduction, String> {
            panic!("input expression algebra already canceled the root")
        },
    )
    .unwrap();
    assert!(zero.terms.is_empty());
    assert!(
        zero.nonzero_conditions
            .contains(&c.coefficient_fixture("d-4"))
    );
}

#[test]
fn no_memoization_combination_still_validates_zero_rows_and_provider_policy() {
    let f = family("occurrence-validation", 1);
    let c = f.coefficient_context();
    let foreign = family("foreign", 2);
    let result = reduce_vacuum_factor_combination_without_memoization(
        f.clone(),
        [(key([3]), c.one())],
        [],
        Default::default(),
        |_, _| -> Result<_, String> {
            Ok(VacuumFactorReduction {
                terms: vec![term(&foreign, [1, 1, 1], 0)],
                ..Default::default()
            })
        },
    );
    assert!(matches!(
        result,
        Err(VacuumProductReductionError::InvalidInput(_))
    ));
    let result = reduce_vacuum_factor_combination_without_memoization(
        f.clone(),
        [(key([3]), c.one())],
        [],
        Default::default(),
        |_, _| -> Result<_, String> {
            Ok(VacuumFactorReduction {
                work: VacuumFactorWork {
                    cache_hits: 1,
                    ..Default::default()
                },
                ..Default::default()
            })
        },
    );
    assert!(matches!(
        result,
        Err(VacuumProductReductionError::InvalidInput(_))
    ));
}
