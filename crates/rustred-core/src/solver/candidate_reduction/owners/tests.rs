use super::super::owner_test_support::*;
use super::super::preparation::shared::PREPARATION_COUNT;
use super::*;
use crate::solver::{CandidateReductionError, FiniteCasePolicy};
use std::sync::Arc;

#[test]
fn common_context_prepares_sources_once_for_multiple_owners_and_keeps_above_rank_terminals() {
    let family = Arc::new(crate::solver::tests::sunset());
    let before = PREPARATION_COUNT.with(|count| count.get());
    let context = context(family.clone(), Some(0), Default::default());
    let programs = CandidateOwnerPrograms::try_new(
        context.clone(),
        [
            input([true, true, true], Some(0), vec![], &[[1, 1, 1]]),
            input([true, true, false], Some(0), vec![], &[[1, 1, -2]]),
        ],
    )
    .unwrap();
    assert_eq!(PREPARATION_COUNT.with(|count| count.get()), before + 1);
    assert!(Arc::ptr_eq(programs.context(), &context));
    assert!(Arc::ptr_eq(context.family_owner(), &family));
    assert_eq!(programs.owner_count(), 2);
    assert_eq!(programs.terminal_count(), 2);
    assert!(
        programs.owners[&[true, true, false]]
            .terminals
            .contains(&key([1, 1, -2]))
    );
}

#[test]
fn owner_scope_duplicates_and_root_mismatch_fail_atomically() {
    let family = Arc::new(crate::solver::tests::sunset());
    let context = context(family, Some(10), Default::default());
    assert!(CandidateOwnerPrograms::try_new(context.clone(), Vec::new()).is_err());
    assert!(
        CandidateOwnerPrograms::try_new(
            context.clone(),
            [
                input([true; 3], Some(10), vec![], &[]),
                input([true; 3], Some(10), vec![], &[]),
            ]
        )
        .is_err()
    );
    assert!(matches!(
        CandidateOwnerPrograms::try_new(context.clone(), [input([true; 3], Some(9), vec![], &[]),]),
        Err(CandidateReductionError::InconsistentNumeratorRank { .. })
    ));
    let mut bad = input([true; 3], Some(10), vec![], &[]);
    bad.solution.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    assert!(matches!(
        CandidateOwnerPrograms::try_new(context.clone(), [bad]),
        Err(CandidateReductionError::InconsistentFiniteCasePolicy { .. })
    ));
    let mut bad = input([true; 3], Some(10), vec![], &[]);
    bad.saved_root = [true, true, false];
    assert!(CandidateOwnerPrograms::try_new(context, [bad]).is_err());
}

#[test]
fn context_rejects_unscoped_retention_foreign_proof_and_wrong_arity() {
    let family = Arc::new(crate::solver::tests::tadpole());
    assert!(
        CandidateOwnerContext::<1>::try_new(
            family.clone(),
            CandidateOwnerScope {
                max_numerator_rank: None,
                finite_case_policy: FiniteCasePolicy::RetainRankFinite,
            },
            vec![],
            Default::default()
        )
        .is_err()
    );
    assert!(
        CandidateOwnerContext::<2>::try_new(
            family.clone(),
            CandidateOwnerScope {
                max_numerator_rank: None,
                finite_case_policy: Default::default(),
            },
            vec![],
            Default::default()
        )
        .is_err()
    );
    let foreign = crate::solver::tests::sunset();
    let analyzer = crate::sector::zero::Analyzer::try_unrestricted(&foreign).unwrap();
    let crate::sector::zero::Decision::ProvedZero(proof) = analyzer
        .analyze(&crate::sector::Mask::try_new([false; 3]).unwrap())
        .unwrap()
    else {
        panic!("zero fixture")
    };
    assert!(
        CandidateOwnerContext::<1>::try_new(
            family,
            CandidateOwnerScope {
                max_numerator_rank: None,
                finite_case_policy: Default::default(),
            },
            vec![proof],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn owner_rule_target_and_terminal_support_checks_match_existing_admission() {
    let family = Arc::new(crate::solver::tests::sunset());
    let ctx = context(family.clone(), None, Default::default());
    let bad = input(
        [true, true, false],
        None,
        vec![rule(&family, [1, 1, 1], &[])],
        &[],
    );
    assert!(CandidateOwnerPrograms::try_new(ctx.clone(), [bad]).is_err());
    assert!(
        CandidateOwnerPrograms::try_new(ctx, [input([true; 3], None, vec![], &[[1, 1, 0]])])
            .is_err()
    );
}
