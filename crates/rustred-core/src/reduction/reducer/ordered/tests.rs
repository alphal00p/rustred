use super::*;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};

fn key(powers: impl IntoIterator<Item = i64>) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

#[test]
fn combines_recurrence_children_before_reducing_them_and_finishes_phases() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let context = artifact.coefficient_context();
    let mut observations = Vec::new();
    let result = Reducer::reduce_artifact_linear_combination_ordered_with_observation(
        &artifact,
        [
            (key([3]), context.one()),
            (key([2]), context.coefficient_fixture("-(d-4)/4")),
        ],
        ReductionLimits::default(),
        &mut |progress, _| {
            observations.push(progress.clone());
            Ok(())
        },
    )
    .unwrap();
    assert!(result.terms.is_empty());
    assert_eq!(result.work.rule_applications(), 1);
    assert_eq!(result.work.cache_hits(), 0);
    assert_eq!(result.progress.cancelled_terms, 1);
    assert_eq!(
        result.progress.completed_phases,
        result.progress.total_phases
    );
    assert!(
        observations
            .windows(2)
            .all(|pair| pair[0].completed_phases <= pair[1].completed_phases)
    );
    assert!(
        observations
            .iter()
            .all(|progress| progress.total_phases == 2)
    );
}

#[test]
fn canceled_input_denominators_remain_explicit_conditions() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let context = artifact.coefficient_context();
    let result = Reducer::reduce_artifact_linear_combination_ordered(
        &artifact,
        [
            (key([3]), context.coefficient_fixture("1/(d-1)")),
            (key([3]), context.coefficient_fixture("-1/(d-1)")),
        ],
        ReductionLimits::default(),
    )
    .unwrap();
    assert!(result.terms.is_empty());
    assert_eq!(result.work.rule_applications(), 0);
    assert_eq!(
        result.nonzero_conditions,
        vec![context.coefficient_fixture("d-1")]
    );
}

#[test]
fn admits_even_zero_or_canceling_roots_before_any_reduction() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    assert_eq!(
        Reducer::reduce_artifact_linear_combination_ordered(
            &artifact,
            [(key([2, 1]), artifact.coefficient_context().zero())],
            ReductionLimits::default(),
        ),
        Err(ReductionError::WrongArity {
            expected: 1,
            actual: 2
        }),
    );
}

#[test]
fn canonical_orbit_images_cancel_before_cell_selection() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let context = artifact.coefficient_context();
    let result = Reducer::reduce_artifact_linear_combination_ordered(
        &artifact,
        [
            (key([2, 1, 1]), context.one()),
            (key([1, 2, 1]), context.integer(-1)),
        ],
        ReductionLimits::default(),
    )
    .unwrap();
    assert!(result.terms.is_empty());
    assert_eq!(result.work.rule_applications(), 0);
    assert_eq!(result.work.cache_hits(), 0);
}

#[test]
fn exact_sunset_dots_numerators_pinches_and_factorization_match_dfs() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let context = artifact.coefficient_context();
    for powers in [
        [2, 1, 1],
        [2, 2, 1],
        [3, 1, 1],
        [0, 2, 2],
        [-1, 2, 1],
        [-2, 2, 3],
        [-1, 0, 2],
    ] {
        let target = key(powers);
        let (baseline, native_guards, _) = Reducer::reduce_artifact_unit_mass_guarded(
            &artifact,
            &target,
            ReductionLimits::default(),
        )
        .unwrap();
        let result = Reducer::reduce_artifact_linear_combination_ordered(
            &artifact,
            [(target.clone(), context.one())],
            ReductionLimits::default(),
        )
        .unwrap();
        assert_eq!(&result.terms, baseline.terms(), "target {powers:?}");
        // The global schedule may avoid an exactly canceled branch. Every
        // remaining obligation must come from the same native checked path.
        assert!(
            result
                .nonzero_conditions
                .iter()
                .all(|guard| native_guards.contains(guard)),
            "new guard at {powers:?}: {:?} vs {:?}",
            result.nonzero_conditions,
            native_guards
        );
        assert_eq!(
            result.progress.completed_phases,
            result.progress.total_phases
        );
    }
}

#[test]
fn opaque_numerator_factorization_preserves_all_dependency_guard_obligations() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let context = artifact.coefficient_context();
    let target = key([-2, 2, 3]);
    // This numerator moment follows the same complete factorization primitive
    // in both engines, so there is no alternative/canceled branch whose guards
    // may legitimately disappear under global grouping.
    assert!(
        Reducer::new(&artifact)
            .unwrap()
            .select_first_factorization(&target)
            .unwrap()
            .is_some()
    );
    let (baseline, native_guards, _) =
        Reducer::reduce_artifact_unit_mass_guarded(&artifact, &target, ReductionLimits::default())
            .unwrap();
    let result = Reducer::reduce_artifact_linear_combination_ordered(
        &artifact,
        [(target, context.one())],
        ReductionLimits::default(),
    )
    .unwrap();
    assert!(
        !native_guards.is_empty(),
        "the control must exercise genuine angular poles"
    );
    assert_eq!(&result.terms, baseline.terms());
    let native_set = native_guards.iter().collect::<HashSet<_>>();
    let ordered_set = result.nonzero_conditions.iter().collect::<HashSet<_>>();
    assert_eq!(ordered_set, native_set);
}

#[test]
fn fresh_ordered_requests_never_reuse_depth_first_completed_results() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let target = key([5]);
    let mut dfs = Reducer::new(&artifact).unwrap();
    dfs.reduce_unit_mass(&target).unwrap();
    let before = dfs.statistics();
    let run = || {
        Reducer::reduce_artifact_linear_combination_ordered(
            &artifact,
            [(target.clone(), artifact.coefficient_context().one())],
            ReductionLimits::default(),
        )
        .unwrap()
    };
    let first = run();
    let second = run();
    assert_eq!(first, second);
    assert_eq!(first.work.rule_applications(), 4);
    assert_eq!(first.work.cache_hits(), 0);
    assert_eq!(dfs.statistics(), before);
}

#[test]
fn rejects_conflicting_rule_order_even_for_in_memory_test_artifacts() {
    let mut artifact = derive_two_loop_unit_mass_sunset().unwrap();
    artifact.replace_all_cell_rule_orderings_for_test(OrderingPolicy::SpiredUncutV1);
    assert!(matches!(
        Reducer::reduce_artifact_linear_combination_ordered(
            &artifact,
            [(key([2, 1, 1]), artifact.coefficient_context().one())],
            ReductionLimits::default(),
        ),
        Err(ReductionError::ReducerInvariant {
            detail: "ordered artifact has inconsistent rule or canonicalizer ordering",
        }),
    ));
}
