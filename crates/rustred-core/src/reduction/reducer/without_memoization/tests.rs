use super::*;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset, k6_product_reducer_fixture,
};

fn key(powers: impl IntoIterator<Item = i64>) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

#[test]
fn repeated_closed_descendants_are_recomputed_without_reading_or_mutating_a_cache() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let context = artifact.coefficient_context();
    let mut memoized = Reducer::new(&artifact).unwrap();
    let mut expected = BTreeMap::new();
    for target in [key([4]), key([3])] {
        for (master, coefficient) in memoized.reduce_unit_mass(&target).unwrap().into_terms() {
            accumulate_master(
                context,
                &mut expected,
                &master,
                coefficient,
                ReductionLimits::default(),
            )
            .unwrap();
        }
    }
    assert_eq!(memoized.statistics().rule_applications(), 3);
    let before = memoized.statistics();
    let cache = memoized.cache.clone();
    let result = Reducer::reduce_artifact_linear_combination_without_memoization(
        &artifact,
        [(key([4]), context.one()), (key([3]), context.one())],
        ReductionLimits::default(),
    )
    .unwrap();
    assert_eq!(result.terms, expected);
    assert_eq!(result.work.rule_applications(), 5);
    assert_eq!(result.work.cache_hits(), 0);
    assert_eq!(result.work.cached_integrals(), 0);
    assert_eq!(result.work.cached_coefficient_bytes(), 0);
    assert_eq!(memoized.statistics(), before);
    assert_eq!(memoized.cache, cache);
}

#[test]
fn canceled_closed_roots_are_still_recomputed_and_keep_input_poles() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let context = artifact.coefficient_context();
    let result = Reducer::reduce_artifact_linear_combination_without_memoization(
        &artifact,
        [
            (key([3]), context.coefficient_fixture("1/(d-1)")),
            (key([3]), context.coefficient_fixture("-1/(d-1)")),
        ],
        ReductionLimits::default(),
    )
    .unwrap();
    assert!(result.terms.is_empty());
    assert_eq!(result.work.rule_applications(), 4);
    assert_eq!(result.work.cache_hits(), 0);
    assert_eq!(
        result.nonzero_conditions,
        [context.coefficient_fixture("d-1")]
    );
    assert_eq!(
        Reducer::reduce_artifact_linear_combination_without_memoization(
            &artifact,
            [(key([1, 1]), context.zero())],
            ReductionLimits::default(),
        ),
        Err(ReductionError::WrongArity {
            expected: 1,
            actual: 2
        })
    );
}

fn assert_guarded_parity(artifact: &ClosedArtifact, target: IntegralKey) {
    let (expected, expected_guards, _) =
        Reducer::reduce_artifact_unit_mass_guarded(artifact, &target, ReductionLimits::default())
            .unwrap();
    let (actual, actual_guards, work) =
        Reducer::reduce_artifact_unit_mass_guarded_without_memoization(
            artifact,
            &target,
            ReductionLimits::default(),
        )
        .unwrap();
    assert_eq!(
        actual.terms(),
        expected.terms(),
        "target {:?}",
        target.powers()
    );
    assert_eq!(
        actual_guards.iter().collect::<HashSet<_>>(),
        expected_guards.iter().collect::<HashSet<_>>(),
        "target {:?}",
        target.powers()
    );
    assert_eq!(work.cache_hits(), 0);
    assert_eq!(work.cached_integrals(), 0);
    assert_eq!(work.cached_coefficient_terms(), 0);
    assert_eq!(work.cached_coefficient_bytes(), 0);
}

#[test]
fn closed_without_memoization_keeps_raw_rule_and_singleton_factorization_results_and_guards() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    for target in [[2, 2, 1], [3, 2, 1], [-2, 2, 3], [-1, 2, 1], [0, 2, 2]] {
        assert_guarded_parity(&artifact, key(target));
    }
}

#[test]
fn closed_without_memoization_propagates_through_correlated_product_moments() {
    let artifact = k6_product_reducer_fixture();
    for target in [
        [-1, -1, 1, -1, 1, 1],
        [-1, -1, 1, 1, -1, 1],
        [-2, 0, 1, 1, 1, 1],
        [-1, -1, 1, 1, 1, 1],
    ] {
        assert_guarded_parity(&artifact, key(target));
    }
}

#[test]
fn disabled_closed_execution_constructs_no_parent_or_dependency_memo_tables() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let reducer = Reducer::build_for_execution(
        &artifact,
        ReductionLimits::default(),
        Arc::new(SharedCacheBudget::default()),
        false,
    )
    .unwrap();
    assert!(reducer.cache.is_empty());
    assert!(
        reducer
            .dependency_reducers
            .iter()
            .all(|dependency| dependency.cache.is_empty())
    );
    assert_eq!(reducer.statistics().cached_integrals(), 0);
    assert_eq!(reducer.statistics().cached_coefficient_bytes(), 0);
}

#[test]
fn no_memoization_bypasses_completed_cache_budgets_and_guard_census() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let target = key([-2, 2, 3]);
    let (expected, expected_guards, _) =
        Reducer::reduce_artifact_unit_mass_guarded(&artifact, &target, ReductionLimits::default())
            .unwrap();
    let limits = ReductionLimits {
        diagnostics_enabled: false,
        max_cached_integrals: 0,
        max_cached_coefficient_terms: 0,
        max_cached_coefficient_bytes: 0,
        ..Default::default()
    };
    let (actual, guards, work) =
        Reducer::reduce_artifact_unit_mass_guarded_without_memoization(&artifact, &target, limits)
            .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(
        guards.iter().collect::<HashSet<_>>(),
        expected_guards.iter().collect::<HashSet<_>>()
    );
    assert_eq!(work.cache_hits(), 0);
    assert_eq!(work.cached_integrals(), 0);
    assert_eq!(work.cached_coefficient_bytes(), 0);
}
