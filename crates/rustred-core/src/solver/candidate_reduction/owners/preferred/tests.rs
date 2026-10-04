//! Synthetic dispatch fixtures, not newly asserted IBP identities.
use super::*;
use crate::family::IntegralKey;
use crate::reduction::{ReductionRequest, ReductionStatistics};
use crate::solver::candidate_reduction::{
    evaluator::CandidateEvaluator,
    owner_test_support::{context, input, key, rule},
    owners::{OwnerDomainMatchDisposition as Disposition, OwnerStep},
};
use std::ops::ControlFlow;
use std::sync::atomic::AtomicBool;

fn fixture() -> CandidateOwnerPrograms<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    CandidateOwnerPrograms::try_new_with_preferences(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![
                rule(&family, [2], &[([1], 1)]),
                rule(&family, [3], &[([1], 1)]),
                rule(&family, [4], &[([1], 1)]),
            ],
            &[[1]],
        )],
        [input(
            [true],
            None,
            vec![
                rule(&family, [1], &[([0], 1)]),
                rule(&family, [2], &[([1], 7)]),
                rule(&family, [3], &[([1], 9)]),
            ],
            &[[2]],
        )],
    )
    .unwrap()
}

fn step(
    p: &CandidateOwnerPrograms<1>,
    key: &IntegralKey,
) -> Result<(OwnerStep, Vec<usize>), CandidateReductionError> {
    let owner = &p.owners[&[true]];
    let mut visited = vec![];
    let result = owner.evaluate_step::<CandidateReductionError>(key, |batch| {
        visited.push(if std::ptr::eq(batch, owner.batches[0].as_ref()) {
            0
        } else {
            1
        });
        let evaluator = CandidateEvaluator {
            context: &p.context.shared.context,
            root_sector: owner.root,
            ordering: &owner.ordering,
            rules: &batch.rules,
            whole_piece_alternatives: &batch.whole_piece_alternatives,
            source_conditions: &p.context.shared.source_conditions,
            zero_sectors: &p.context.shared.zero_sectors,
            limits: p.context.limits,
        };
        Ok(evaluator.apply(
            key,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        ))
    })?;
    Ok((result, visited))
}

#[test]
fn baseline_terminals_and_explicit_residual_holes_precede_every_preferred_rule() {
    let p = fixture();
    assert_eq!(p.terminal_count(), 1);
    assert_eq!(p.owners[&[true]].batches[0].terminals, [key([1])].into());
    assert_eq!(
        p.owners[&[true]].batches[0].deferred_points,
        Some([key([2])].into())
    );
    assert!(p.owners[&[true]].batches[1].terminals.is_empty());
    let (result, visits) = step(&p, &key([1])).unwrap();
    assert!(matches!(result, OwnerStep::Terminal));
    assert!(
        visits.is_empty(),
        "baseline terminal shadows even overlapping preferred formula"
    );
    for (target, expected) in [(2, vec![1]), (3, vec![0]), (4, vec![0, 1])] {
        let (result, visits) = step(&p, &key([target])).unwrap();
        assert!(matches!(result, OwnerStep::Applied(_)));
        assert_eq!(visits, expected);
    }
    assert!(matches!(
        step(&p, &key([5])).unwrap().0,
        OwnerStep::Uncovered
    ));
}

#[test]
fn mixed_box_partitions_terminal_hole_preferred_fallback_and_gap_exactly() {
    let p = fixture();
    let mut pieces = vec![];
    p.visit_owner_domain_matches(
        [true],
        &[0],
        &[Some(4)],
        None,
        Default::default(),
        &AtomicBool::new(false),
        |piece| {
            pieces.push(piece);
            ControlFlow::Continue(())
        },
    )
    .unwrap();
    let expected = [
        Disposition::Terminal { batch: 0 },
        Disposition::SelectedRule { batch: 1, rule: 0 },
        Disposition::SelectedRule { batch: 0, rule: 2 },
        Disposition::SelectedRule { batch: 1, rule: 2 },
        Disposition::ExactGap,
    ];
    for (x, expected) in expected.into_iter().enumerate() {
        let at: Vec<_> = pieces
            .iter()
            .filter(|piece| {
                piece.lower()[0] <= x as u64
                    && piece.upper()[0].is_none_or(|upper| x as u64 <= upper)
            })
            .collect();
        assert_eq!(at.len(), 1);
        assert_eq!(at[0].disposition(), expected);
    }
    let mut limits = crate::solver::OwnerDomainMatchLimits::default();
    limits.max_terminal_checks = 1;
    assert!(
        p.visit_owner_domain_matches(
            [true],
            &[1],
            &[Some(1)],
            None,
            limits,
            &AtomicBool::new(false),
            |_| ControlFlow::Continue(())
        )
        .is_err(),
        "deferral partition consumes the existing finite-boundary budget"
    );
    assert!(
        p.visit_owner_domain_matches(
            [true],
            &[0],
            &[Some(4)],
            None,
            Default::default(),
            &AtomicBool::new(true),
            |_| ControlFlow::Continue(())
        )
        .is_err()
    );
}

#[test]
fn hard_errors_propagate_but_native_pole_inapplicability_can_fall_through() {
    let mut p = fixture();
    let owner = Arc::get_mut(p.owners.get_mut(&[true]).unwrap()).unwrap();
    let preferred = Arc::get_mut(&mut owner.batches[0]).unwrap();
    preferred.rules[2].rhs[0].shift = [1];
    assert!(matches!(
        step(&p, &key([3])),
        Err(CandidateReductionError::NonDescending { .. })
    ));
    // An original denominator exclusion is ordinary inapplicability, not an
    // undefined-evaluation error. Preserve that existing native distinction.
    let zero = p
        .context
        .coefficient_context()
        .numerator_condition_with_limits(
            &p.context.coefficient_context().zero(),
            Default::default(),
        )
        .unwrap();
    let owner = Arc::get_mut(p.owners.get_mut(&[true]).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[2].rhs[0].denominator = zero;
    let (result, visits) = step(&p, &key([3])).unwrap();
    assert!(matches!(result, OwnerStep::Applied(_)));
    assert_eq!(visits, vec![0, 1]);
    let mut calls = 0;
    let error = CandidateReductionError::SourceConditionVanished {
        target: key([3]),
        ordinal: 0,
    };
    let result: Result<OwnerStep, CandidateReductionError> =
        p.owners[&[true]].evaluate_step(&key([3]), |_| {
            calls += 1;
            Ok(Err(error.clone()))
        });
    assert!(matches!(
        result,
        Err(CandidateReductionError::SourceConditionVanished { .. })
    ));
    assert_eq!(calls, 1);
}

#[test]
fn admitted_nonconstant_pole_excludes_only_its_face_in_both_dispatch_paths() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let shared = context(family.clone(), None, Default::default());
    let c = shared.coefficient_context();
    let coefficient = c
        .div(
            &c.integer(1),
            &c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap(),
        )
        .unwrap();
    let mut preferred = rule(&family, [2], &[([1], 1)]);
    preferred.candidate.case = crate::solver::Case::generic();
    preferred.candidate.target = crate::solver::Integral::symbolic([0]).unwrap();
    preferred.candidate.rhs[0].integral = crate::solver::Integral::symbolic([-1]).unwrap();
    preferred.candidate.rhs[0].coefficient = coefficient.raw().clone();
    let p = CandidateOwnerPrograms::try_new_with_preferences(
        shared,
        [input(
            [true],
            None,
            vec![rule(&family, [3], &[([1], 1)])],
            &[[1]],
        )],
        [input([true], None, vec![preferred], &[])],
    )
    .unwrap();
    assert_eq!(step(&p, &key([2])).unwrap().1, vec![0]);
    assert_eq!(step(&p, &key([3])).unwrap().1, vec![0, 1]);
    let mut pieces = vec![];
    p.visit_owner_domain_matches(
        [true],
        &[1],
        &[Some(2)],
        None,
        Default::default(),
        &AtomicBool::new(false),
        |piece| {
            pieces.push(piece);
            ControlFlow::Continue(())
        },
    )
    .unwrap();
    for (x, batch) in [(1, 0), (2, 1)] {
        let at: Vec<_> = pieces
            .iter()
            .filter(|piece| {
                piece.lower()[0] <= x && piece.upper()[0].is_none_or(|upper| x <= upper)
            })
            .collect();
        assert_eq!(at.len(), 1);
        assert_eq!(
            at[0].disposition(),
            Disposition::SelectedRule { batch, rule: 0 }
        );
    }
}

#[test]
fn empty_preferences_preserve_batch_shape_and_default_dispatch() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let p = CandidateOwnerPrograms::try_new_with_preferences(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![rule(&family, [2], &[([1], 1)])],
            &[[1]],
        )],
        [],
    )
    .unwrap();
    assert_eq!(p.owners[&[true]].batches.len(), 1);
    assert!(p.owners[&[true]].batches[0].deferred_points.is_none());
    assert!(matches!(
        step(&p, &key([1])).unwrap().0,
        OwnerStep::Terminal
    ));
    assert!(matches!(
        step(&p, &key([2])).unwrap().0,
        OwnerStep::Applied(_)
    ));
}

#[test]
fn preferred_inputs_remain_fully_admitted_and_scope_bound() {
    let family = Arc::new(crate::solver::tests::sunset());
    let context = context(family, None, Default::default());
    let baseline = || input([true, true, false], None, vec![], &[[1, 1, 0]]);
    let mut wrong = baseline();
    wrong.saved_root = [true, true, false];
    assert!(
        CandidateOwnerPrograms::try_new_with_preferences(context.clone(), [baseline()], [wrong])
            .is_err()
    );
    assert!(
        CandidateOwnerPrograms::try_new_with_preferences(
            context.clone(),
            [baseline()],
            [input([true; 3], None, vec![], &[])]
        )
        .is_err()
    );
    assert!(
        CandidateOwnerPrograms::try_new_with_preferences(
            context.clone(),
            [baseline()],
            [baseline(), baseline()]
        )
        .is_err()
    );
    let mut wrong = baseline();
    wrong.solution.finite_residuals = vec![crate::solver::Integral::numeric([1, 1, 1]).unwrap()];
    assert!(
        CandidateOwnerPrograms::try_new_with_preferences(context.clone(), [baseline()], [wrong])
            .is_err(),
        "malformed candidate-only residuals must not be hidden by deferral"
    );
    let mut wrong = baseline();
    wrong.solution.max_numerator_rank = Some(1);
    assert!(
        CandidateOwnerPrograms::try_new_with_preferences(context, [baseline()], [wrong]).is_err()
    );
}
