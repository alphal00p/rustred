//! Partial results retain exact identities, never new master declarations.
use super::*;
use crate::persistence::application_cache::{ApplicationCacheKey, ApplicationCacheRecord};
use crate::solver::candidate_reduction::owner_test_support::{input, rule};
use crate::solver::{CandidateUnitMassProgram, UnresolvedPolicy};

fn partial_fixture() -> CandidateReducer<1> {
    let family = crate::solver::tests::tadpole();
    let record = input(
        [true],
        None,
        vec![
            rule(&family, [4], &[([3], 1), ([2], 3), ([1], -2)]),
            rule(&family, [3], &[([2], -3), ([1], 2)]),
        ],
        &[[1]],
    );
    let mut reducer = from_one_rule(&family, record.solution);
    let indexed = reducer.program.context.clone();
    let guard = indexed
        .lift_base_polynomial(
            &family
                .coefficient_context()
                .coefficient_fixture("d-4")
                .numerator,
        )
        .unwrap();
    Arc::get_mut(&mut reducer.program)
        .unwrap()
        .source_conditions
        .push(guard);
    reducer
}

#[test]
fn unmemoized_partial_candidate_retains_and_cancels_unknowns_without_memo_state() {
    let mut source = partial_fixture();
    assert!(matches!(
        source.reduce_unit_mass_without_memoization_guarded(&key([3]), Default::default()),
        Err(CandidateReductionError::Uncovered { .. })
    ));
    assert!(source.cache.is_empty());
    source
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    // The zero-support key also has no sector record at all; Preserve applies
    // to that dispatch miss as well as a missing rule in a known sector.
    for target in [key([3]), key([4]), key([0])] {
        let mut legacy = source.fork_empty();
        let expected = legacy
            .reduce_unit_mass_guarded(&target, Default::default())
            .unwrap();
        let actual = source
            .reduce_unit_mass_without_memoization_guarded(&target, Default::default())
            .unwrap();
        assert_eq!(actual.0, expected.0);
        assert_eq!(actual.1, expected.1);
        assert_eq!(actual.2.cache_hits(), 0);
        assert!(source.cache.is_empty());
    }
    let canceled = source
        .reduce_unit_mass_without_memoization_guarded(&key([4]), Default::default())
        .unwrap();
    assert!(canceled.0.is_zero());
    assert!(canceled.0.unresolved().is_empty());
    // No clear is required: the alternative never populated the ordinary memo.
    source
        .set_unresolved_policy(UnresolvedPolicy::Reject)
        .unwrap();
}

#[test]
fn partial_candidate_preserves_guards_back_substitution_and_cancelled_residuals() {
    let mut reducer = partial_fixture();
    assert_eq!(reducer.unresolved_policy(), UnresolvedPolicy::Reject);
    assert!(matches!(
        reducer.reduce_unit_mass(&key([3])),
        Err(CandidateReductionError::Uncovered { .. })
    ));
    reducer.clear_cache().unwrap();
    reducer
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    let (result, conditions, _) = reducer
        .reduce_unit_mass_guarded(&key([3]), Default::default())
        .unwrap();
    let c = reducer.coefficient_context().base();
    assert_eq!(
        result.terms(),
        &BTreeMap::from([(key([2]), c.integer(-3)), (key([1]), c.integer(2))])
    );
    assert_eq!(
        result.unresolved(),
        &std::collections::BTreeSet::from([key([2])])
    );
    assert_eq!(conditions, vec![c.coefficient_fixture("d-4")]);
    assert_eq!(
        reducer.terminals(),
        &std::collections::BTreeSet::from([key([1])])
    );
    let warm = reducer
        .reduce_unit_mass_guarded(&key([3]), Default::default())
        .unwrap();
    assert_eq!(warm.0, result);
    assert_eq!(warm.1, conditions);
    assert_eq!(warm.2.rule_applications(), 0);
    let cancelled = reducer
        .reduce_unit_mass_guarded(&key([4]), Default::default())
        .unwrap();
    assert!(cancelled.0.is_zero());
    assert!(cancelled.0.unresolved().is_empty());
    assert_eq!(cancelled.1, conditions);
    assert!(
        reducer
            .set_unresolved_policy(UnresolvedPolicy::Reject)
            .is_err()
    );
    assert!(reducer.check_targets([key([3])]).is_err());
    reducer.clear_cache().unwrap();
    reducer
        .set_unresolved_policy(UnresolvedPolicy::Reject)
        .unwrap();
    assert!(matches!(
        reducer.reduce_unit_mass(&key([3])),
        Err(CandidateReductionError::Uncovered { .. })
    ));
}

#[test]
fn partial_candidate_cannot_publish_or_import_incomplete_guarded_records() {
    let mut reducer = partial_fixture();
    reducer
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    let (result, conditions, _) = reducer
        .reduce_unit_mass_guarded(&key([3]), Default::default())
        .unwrap();
    let program = CandidateUnitMassProgram::new(&reducer, [73; 32]);
    let record = ApplicationCacheRecord {
        key: ApplicationCacheKey::guarded_root(&key([3])),
        terms: result.terms().clone(),
        nonzero_conditions: conditions,
    };
    assert!(
        program
            .validate_completed_record(&record, Default::default())
            .is_err()
    );
    let mut cache = program.new_cache(Default::default(), Default::default());
    assert!(
        program
            .reduce_guarded_root(&key([3]), &mut cache, Default::default(), None)
            .is_err()
    );
}

#[test]
fn partial_candidate_keeps_arity_source_and_work_failures() {
    let mut reducer = partial_fixture();
    reducer
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    assert!(matches!(
        reducer.reduce_unit_mass(&key([1, 0])),
        Err(CandidateReductionError::Application(
            ReductionError::WrongArity { .. }
        ))
    ));
    let limits = ReductionLimits {
        diagnostics_enabled: true,
        max_rule_applications: 0,
        ..Default::default()
    };
    assert!(reducer.reduce_unit_mass_guarded(&key([3]), limits).is_err());
    let indexed = reducer.program.context.clone();
    let vanished = indexed
        .admit_native_polynomial_result_with_limits(
            indexed
                .sub(&indexed.index(0).unwrap(), &indexed.integer(2))
                .unwrap()
                .raw()
                .numerator
                .clone(),
            Default::default(),
        )
        .unwrap();
    Arc::get_mut(&mut reducer.program)
        .unwrap()
        .source_conditions = vec![vanished];
    assert!(matches!(
        reducer.reduce_unit_mass(&key([3])),
        Err(CandidateReductionError::SourceConditionVanished { .. })
    ));
}
