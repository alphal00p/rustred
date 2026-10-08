use super::*;
use crate::algebra::IndexedCoefficientContext;
use crate::solver::candidate_reduction::evaluator::CandidateEvaluator;
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{CandidateReducer, CoordinateCase, Integral, RuleDispatchPolicy};
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn every_applicable_candidate_survives_terminal_and_dispatch_stopping() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut alternative = rule(&family, [2], &[([4], 3)]);
    alternative.dispatch_policy = RuleDispatchPolicy::AfterBaselinePartitionWholePiece;
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![rule(&family, [2], &[([1], 2)]), alternative],
            &[[2]],
        )],
    )
    .unwrap();
    // Deferred stopping is also an execution policy, not an equation guard.
    let owner = Arc::get_mut(owners.owners.get_mut(&[true]).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().deferred_points = Some([key([2])].into());
    let rows = owners.applicable_identities(&key([2])).unwrap();
    assert_eq!(rows.len(), 2);
    let base = family.coefficient_context();
    assert_eq!(
        rows[0].terms,
        BTreeMap::from([(key([1]), base.integer(-2)), (key([2]), base.one())])
    );
    assert_eq!(
        rows[1].terms,
        BTreeMap::from([(key([2]), base.one()), (key([4]), base.integer(-3))])
    );
    assert_eq!(
        (&rows[0].owner, rows[0].batch, rows[0].rule),
        (&vec![true], 0, 0)
    );
    assert_eq!(rows[1].rule, 1);
    // Their difference is -2 I(1) + 3 I(4), including a non-descending RHS.
    assert_eq!(
        base.try_sub(
            &rows[0].terms[&key([2])],
            &rows[1].terms[&key([2])],
            Default::default()
        )
        .unwrap(),
        base.zero()
    );
}

#[test]
fn terminal_declaration_alone_and_other_owner_support_supply_no_identity() {
    let family = Arc::new(crate::solver::tests::sunset());
    let owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), Some(0), Default::default()),
        [input(
            [true, true, false],
            Some(0),
            vec![rule(&family, [2, 1, -3], &[([1, 1, -3], 2)])],
            &[[1, 1, 0]],
        )],
    )
    .unwrap();
    assert!(
        owners
            .applicable_identities(&key([1, 1, 0]))
            .unwrap()
            .is_empty()
    );
    assert!(
        owners
            .applicable_identities(&key([2, 1, 1]))
            .unwrap()
            .is_empty()
    );
    assert!(
        owners
            .applicable_identities(&key([2, 0, -3]))
            .unwrap()
            .is_empty()
    );
    // Internal equations do not clip by the public entry numerator-rank cap.
    assert_eq!(
        owners
            .applicable_identities(&key([2, 1, -3]))
            .unwrap()
            .len(),
        1
    );
    assert!(matches!(
        owners.applicable_identities(&key([2])),
        Err(CandidateReductionError::Application(
            ReductionError::WrongArity { .. }
        ))
    ));
}

#[test]
fn saved_identity_matches_the_existing_one_step_application() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let make_input = || {
        input(
            [true],
            None,
            vec![rule(&family, [2], &[([1], 2), ([1], 3)])],
            &[[1]],
        )
    };
    let owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [make_input()],
    )
    .unwrap();
    let mut reducer = CandidateReducer::try_new(
        &family,
        [true],
        crate::sector::OrderingPolicy::SpiredUncutV1,
        [([true], make_input().solution)],
        vec![],
        Default::default(),
    )
    .unwrap();
    let application = reducer.reduce_unit_mass(&key([2])).unwrap();
    let row = owners.applicable_identities(&key([2])).unwrap().remove(0);
    assert_eq!(row.terms[&key([2])], family.coefficient_context().one());
    for (child, coefficient) in application.terms() {
        assert_eq!(
            row.terms[child],
            family
                .coefficient_context()
                .try_neg(coefficient, Default::default())
                .unwrap()
        );
    }
    assert_eq!(row.terms.len(), application.terms().len() + 1);
}

#[test]
#[cfg(feature = "capacity-dispatch")]
fn padded_capacity_identities_and_evaluator_keep_physical_keys_and_guards() {
    let family = Arc::new(crate::solver::tests::sunset());
    let sector = [true, true, true, false];
    let mut owner_input = input(
        sector,
        None,
        vec![rule(&family, [2, 2, 1, 0], &[([1, 2, 1, 0], 2)])],
        &[],
    );
    owner_input.saved_root = sector;
    let mut owners = CandidateOwnerPrograms::<4>::try_new(
        context(family.clone(), None, Default::default()),
        [owner_input],
    )
    .unwrap();
    let ctx = owners.context.coefficient_context().clone();
    let owner = Arc::get_mut(owners.owners.get_mut(&sector).unwrap()).unwrap();
    let prepared = &mut Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0];
    prepared.rhs[0].denominator = polynomial(&ctx, "d-4");
    prepared.exceptions = vec![vec![polynomial(&ctx, "n0-2"), polynomial(&ctx, "d-5")]];
    let target = key([2, 2, 1]);
    let rows = owners.applicable_identities(&target).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].owner, vec![true; 3]);
    assert_eq!(
        rows[0].terms,
        BTreeMap::from([
            (target.clone(), family.coefficient_context().one()),
            (key([1, 2, 1]), family.coefficient_context().integer(-2)),
        ])
    );
    for condition in ["d-4", "d-5"] {
        assert!(
            rows[0]
                .nonzero_conditions
                .contains(&family.coefficient_context().coefficient_fixture(condition))
        );
    }
    let owner = &owners.owners[&sector];
    let batch = &owner.batches[0];
    let evaluator = CandidateEvaluator {
        context: owners.context.coefficient_context(),
        root_sector: owner.root,
        ordering: &owner.ordering,
        rules: &batch.rules,
        whole_piece_alternatives: &batch.whole_piece_alternatives,
        source_conditions: &owners.context.shared.source_conditions,
        zero_sectors: &owners.context.shared.zero_sectors,
        limits: owners.context.limits,
    };
    evaluator.validate_target(&target).unwrap();
    assert_eq!(
        evaluator
            .apply(&target, &mut Default::default(), &mut Default::default())
            .unwrap(),
        BTreeMap::from([(key([1, 2, 1]), family.coefficient_context().integer(2))])
    );
    assert!(matches!(
        owners.applicable_identities(&key([2, 2, 1, 0])),
        Err(CandidateReductionError::Application(
            ReductionError::WrongArity {
                expected: 3,
                actual: 4
            }
        ))
    ));
}

fn polynomial(
    context: &IndexedCoefficientContext,
    expression: &str,
) -> crate::algebra::IndexedPolynomial {
    let value = match expression {
        "n0-n1" => context
            .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
            .unwrap(),
        "n0-1" => context
            .sub(&context.index(0).unwrap(), &context.integer(1))
            .unwrap(),
        "n0-2" => context
            .sub(&context.index(0).unwrap(), &context.integer(2))
            .unwrap(),
        "n1-2" => context
            .sub(&context.index(1).unwrap(), &context.integer(2))
            .unwrap(),
        "n1-3" => context
            .sub(&context.index(1).unwrap(), &context.integer(3))
            .unwrap(),
        _ => context
            .lift(&context.base().coefficient_fixture(expression))
            .unwrap(),
    };
    context
        .admit_native_polynomial_result_with_limits(
            value.raw().numerator.clone(),
            Default::default(),
        )
        .unwrap()
}

#[test]
fn fixed_equalities_and_exception_conjunctions_reuse_exact_applicability() {
    let family = Arc::new(crate::solver::tests::sunset());
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [input(
            [true; 3],
            None,
            vec![rule(&family, [2, 2, 1], &[([1, 2, 1], 1)])],
            &[],
        )],
    )
    .unwrap();
    let ctx = owners.context.coefficient_context().clone();
    let owner = Arc::get_mut(owners.owners.get_mut(&[true; 3]).unwrap()).unwrap();
    let rule = &mut Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0];
    rule.fixed = [None, None, Some(1)];
    rule.equalities = vec![polynomial(&ctx, "n0-n1")];
    rule.exceptions = vec![vec![polynomial(&ctx, "n0-2"), polynomial(&ctx, "n1-3")]];
    assert_eq!(
        owners.applicable_identities(&key([2, 2, 1])).unwrap().len(),
        1
    );
    assert!(
        owners
            .applicable_identities(&key([2, 3, 1]))
            .unwrap()
            .is_empty()
    );
    assert!(
        owners
            .applicable_identities(&key([2, 2, 2]))
            .unwrap()
            .is_empty()
    );
    let owner = Arc::get_mut(owners.owners.get_mut(&[true; 3]).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0].exceptions[0][1] =
        polynomial(&ctx, "n1-2");
    assert!(
        owners
            .applicable_identities(&key([2, 2, 1]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn generic_dimension_conditions_keep_original_poles_and_one_branch_witness() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![rule(&family, [2], &[([1], 1)])],
            &[],
        )],
    )
    .unwrap();
    let ctx = owners.context.coefficient_context().clone();
    let owner = Arc::get_mut(owners.owners.get_mut(&[true]).unwrap()).unwrap();
    let prepared = &mut Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0];
    // Original d-4 remains a guard even though the normalized coefficient is 1.
    prepared.rhs[0].denominator = polynomial(&ctx, "d-4");
    prepared.exceptions = vec![vec![polynomial(&ctx, "d-5"), polynomial(&ctx, "d-6")]];
    let row = owners.applicable_identities(&key([2])).unwrap().remove(0);
    let base = family.coefficient_context();
    assert!(
        row.nonzero_conditions
            .contains(&base.coefficient_fixture("d-4"))
    );
    assert!(
        row.nonzero_conditions
            .contains(&base.coefficient_fixture("d-5"))
    );
    assert!(
        !row.nonzero_conditions
            .contains(&base.coefficient_fixture("d-6"))
    );
}

#[test]
fn original_denominator_zero_blocks_even_cancelled_rhs_terms() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![rule(&family, [2], &[([1], 1), ([1], -1)])],
            &[],
        )],
    )
    .unwrap();
    let ctx = owners.context.coefficient_context().clone();
    let owner = Arc::get_mut(owners.owners.get_mut(&[true]).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0].rhs[0].denominator =
        polynomial(&ctx, "n0-2");
    assert!(owners.applicable_identities(&key([2])).unwrap().is_empty());
}

#[test]
fn source_conditions_and_saved_root_are_checked_on_children() {
    let family = Arc::new(crate::solver::tests::sunset());
    let mut owner_input = input(
        [true, true, false],
        None,
        vec![rule(&family, [2, 1, 0], &[([2, 1, 1], 1)])],
        &[],
    );
    owner_input.saved_root = [true, true, false];
    let owners =
        CandidateOwnerPrograms::try_new(context(family, None, Default::default()), [owner_input])
            .unwrap();
    assert!(matches!(
        owners.applicable_identities(&key([2, 1, 0])),
        Err(CandidateReductionError::OutsideRoot { .. })
    ));

    let family = Arc::new(crate::solver::tests::tadpole());
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family, None, Default::default()),
        [input(
            [true],
            None,
            vec![rule(&crate::solver::tests::tadpole(), [2], &[([1], 1)])],
            &[],
        )],
    )
    .unwrap();
    let ctx = owners.context.coefficient_context().clone();
    Arc::get_mut(&mut owners.context)
        .unwrap()
        .shared
        .source_conditions = vec![polynomial(&ctx, "n0-1")];
    assert!(
        matches!(owners.applicable_identities(&key([2])), Err(CandidateReductionError::SourceConditionVanished { target, .. }) if target == key([1]))
    );
}

#[test]
fn identity_allows_a_cycle_while_execution_still_rejects_it() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut source_rule = rule(&family, [2], &[([3], 1)]);
    source_rule.candidate.case = CoordinateCase::new([None]).unwrap().into();
    source_rule.candidate.target = Integral::symbolic([0]).unwrap();
    source_rule.candidate.rhs[0].integral = Integral::symbolic([1]).unwrap();
    let owners = CandidateOwnerPrograms::try_new(
        context(family, None, Default::default()),
        [input([true], None, vec![source_rule], &[])],
    )
    .unwrap();
    assert_eq!(owners.applicable_identities(&key([2])).unwrap().len(), 1);
    assert!(matches!(
        owners.applicable_identities(&key([i64::MAX])),
        Err(CandidateReductionError::IndexOverflow { .. })
    ));
    let owner = &owners.owners[&[true]];
    let batch = &owner.batches[0];
    let evaluator = CandidateEvaluator {
        context: owners.context.coefficient_context(),
        root_sector: owner.root,
        ordering: &owner.ordering,
        rules: &batch.rules,
        whole_piece_alternatives: &batch.whole_piece_alternatives,
        source_conditions: &owners.context.shared.source_conditions,
        zero_sectors: &owners.context.shared.zero_sectors,
        limits: owners.context.limits,
    };
    assert!(matches!(
        evaluator.apply(&key([2]), &mut Default::default(), &mut Default::default()),
        Err(CandidateReductionError::NonDescending { .. })
    ));
}
