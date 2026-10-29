//! Exact DFS operands are transient occurrences, never reusable completed keys.

use super::*;
use crate::reduction::{CompletedCachePolicy, ReductionStatistics};
use crate::solver::CandidateUnitMassProgram;
use crate::solver::candidate_reduction::owner_test_support::{input, rule};

fn diamond() -> CandidateReducer<1> {
    let family = crate::solver::tests::tadpole();
    let record = input(
        [true],
        None,
        vec![
            rule(&family, [5], &[([4], 1), ([3], 1)]),
            rule(&family, [4], &[([2], 1)]),
            rule(&family, [3], &[([2], 1)]),
            rule(&family, [2], &[([1], 1)]),
        ],
        &[[1]],
    );
    CandidateReducer::try_new(
        &family,
        [true],
        record.ordering,
        [([true], record.solution)],
        vec![],
        Default::default(),
    )
    .unwrap()
}

fn assert_no_memo_work(work: ReductionStatistics) {
    assert_eq!(work.cache_hits(), 0);
    assert_eq!(work.cached_integrals(), 0);
    assert_eq!(work.cached_coefficient_terms(), 0);
    assert_eq!(work.cached_coefficient_bytes(), 0);
}

#[test]
fn candidate_unmemoized_diamond_recomputes_shared_descendants_and_repeated_roots() {
    let mut source = diamond();
    let expected = source
        .reduce_unit_mass_guarded(&key([5]), Default::default())
        .unwrap();
    assert_eq!(expected.2.rule_applications(), 4);
    assert_eq!(
        expected.0.terms(),
        &BTreeMap::from([(key([1]), source.coefficient_context().base().integer(2))])
    );
    let before = source.statistics();
    let before_keys = source.cache.keys().cloned().collect::<Vec<_>>();
    for _ in 0..2 {
        let actual = source
            .reduce_unit_mass_without_memoization_guarded(&key([5]), Default::default())
            .unwrap();
        assert_eq!(actual.0, expected.0);
        assert_eq!(actual.1, expected.1);
        assert_eq!(actual.2.rule_applications(), 5);
        assert_no_memo_work(actual.2);
        assert_eq!(source.statistics(), before);
        assert_eq!(
            source.cache.keys().cloned().collect::<Vec<_>>(),
            before_keys
        );
    }
    // The old resident result remains usable; the alternative did not clear it.
    let warm = source
        .reduce_unit_mass_guarded(&key([5]), Default::default())
        .unwrap();
    assert_eq!(warm.0, expected.0);
    assert_eq!(warm.1, expected.1);
    assert_eq!(warm.2.rule_applications(), 0);
    assert!(warm.2.cache_hits() > 0);
}

#[test]
fn candidate_unmemoized_guards_match_exactly_even_for_shared_canceled_subtrees() {
    for cancelled in [false, true] {
        for representation in [
            crate::solver::CandidateCacheRepresentation::Sparse,
            crate::solver::CandidateCacheRepresentation::Factorized,
        ] {
            let mut source = super::root_guards::guarded_fixture(cancelled);
            source.set_cache_representation(representation).unwrap();
            let target = key([if cancelled { 5 } else { 24 }]);
            let expected = source
                .reduce_unit_mass_guarded(&target, Default::default())
                .unwrap();
            let before = source.statistics();
            let actual = source
                .reduce_unit_mass_without_memoization_guarded(&target, Default::default())
                .unwrap();
            assert_eq!(actual.0, expected.0);
            assert_eq!(actual.1, expected.1, "ordered native guard vector");
            assert_eq!(actual.0.is_zero(), cancelled);
            assert!(
                !actual.1.is_empty(),
                "zero outputs retain source/rule poles"
            );
            assert_no_memo_work(actual.2);
            assert_eq!(source.statistics(), before);
        }
    }
}

#[test]
fn candidate_unmemoized_ignores_memo_allowances_without_skipping_math_limits() {
    let source = diamond();
    let limits = ReductionLimits {
        max_cached_integrals: 0,
        max_cached_coefficient_terms: 0,
        max_cached_coefficient_bytes: 0,
        ..ReductionLimits::default().with_diagnostics()
    };
    let actual = source
        .reduce_unit_mass_without_memoization_guarded(&key([5]), limits)
        .unwrap();
    assert_eq!(actual.2.rule_applications(), 5);
    assert_no_memo_work(actual.2);
    assert!(source.cache.is_empty());
    assert_eq!(source.statistics(), Default::default());
    assert!(matches!(
        source.reduce_unit_mass_without_memoization_guarded(
            &key([5]),
            ReductionLimits {
                max_rule_applications: 0,
                ..limits
            },
        ),
        Err(CandidateReductionError::Application(
            ReductionError::RuleApplicationLimit { .. }
        ))
    ));
    assert!(
        source
            .reduce_unit_mass_without_memoization_guarded(&key([5]), Default::default())
            .is_ok()
    );
}

#[test]
fn candidate_program_unmemoized_bypasses_outer_completed_root_state() {
    let source = diamond();
    let program = CandidateUnitMassProgram::new(&source, [81; 32]);
    let mut cache = program.new_cache(
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    let expected = program
        .reduce_guarded_root(&key([5]), &mut cache, Default::default(), None)
        .unwrap();
    let before = cache.statistics();
    let actual = program
        .reduce_guarded_root_without_memoization(&key([5]), Default::default())
        .unwrap();
    assert_eq!(actual.0.terms(), &expected.record.terms);
    assert_eq!(actual.1, expected.record.nonzero_conditions);
    assert_eq!(actual.2.rule_applications(), 5);
    assert_no_memo_work(actual.2);
    assert_eq!(cache.statistics(), before);
    assert_eq!(source.statistics(), Default::default());
}

#[test]
fn candidate_unmemoized_errors_do_not_leave_state_and_keep_elapsed_checks() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

    let source = diamond();
    assert!(matches!(
        source.reduce_unit_mass_without_memoization_guarded(&key([6]), Default::default()),
        Err(CandidateReductionError::Uncovered { .. })
    ));
    assert!(matches!(
        source.reduce_unit_mass_without_memoization_guarded(&key([1, 0]), Default::default()),
        Err(CandidateReductionError::Application(
            ReductionError::WrongArity { .. }
        ))
    ));
    let budget = ResourceBudget::new(ResourceLimits {
        max_elapsed: Some(std::time::Duration::ZERO),
        ..Default::default()
    });
    assert!(matches!(
        budget
            .run(|| source
                .reduce_unit_mass_without_memoization_guarded(&key([1]), Default::default())),
        Err(CandidateReductionError::Application(
            ReductionError::Resources(ResourceLimitError::Elapsed { .. })
        ))
    ));
    let healthy = source
        .reduce_unit_mass_without_memoization_guarded(&key([5]), Default::default())
        .unwrap();
    assert_eq!(healthy.2.rule_applications(), 5);
    assert_no_memo_work(healthy.2);
    assert!(source.cache.is_empty());
    assert_eq!(source.statistics(), Default::default());
}
