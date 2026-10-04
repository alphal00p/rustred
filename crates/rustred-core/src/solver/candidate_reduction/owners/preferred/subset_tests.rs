//! Synthetic dispatch-only fixtures; no newly asserted IBP identity.
use super::*;
use crate::reduction::{ReductionRequest, ReductionStatistics};
use crate::solver::candidate_reduction::{
    evaluator::CandidateEvaluator,
    owner_test_support::{context, input, key, rule},
    owners::OwnerStep,
};
use crate::solver::{
    Case, Integral, OwnerAppliedEvent, OwnerDomainMatchDisposition as Disposition,
    OwnerDomainMatchLimits, OwnerGuardedEvent, RuleDispatchPolicy,
};
use std::{ops::ControlFlow, sync::atomic::AtomicBool};

fn fixture(selected: Option<&[usize]>, count: usize, optional: bool) -> CandidateOwnerPrograms<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let shared = context(family.clone(), None, Default::default());
    let mut preferred: Vec<_> = (0..count)
        .map(|_| rule(&family, [3], &[([1], 2)]))
        .collect();
    let last = preferred.last_mut().unwrap();
    if optional {
        last.dispatch_policy = RuleDispatchPolicy::AfterBaselinePartitionWholePiece;
        last.candidate.rhs.push(last.candidate.rhs[0].clone());
    } else {
        last.candidate.case = Case::generic();
        last.candidate.target = Integral::symbolic([0]).unwrap();
        last.candidate.rhs[0].integral = Integral::symbolic([-1]).unwrap();
    }
    let subsets = selected.map_or_else(BTreeMap::new, |ids| BTreeMap::from([([true], ids)]));
    CandidateOwnerPrograms::try_new_with_preference_rule_subsets(
        shared,
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
        [input([true], None, preferred, &[[2]])],
        &subsets,
    )
    .unwrap()
}
fn step(
    p: &CandidateOwnerPrograms<1>,
    n: i64,
) -> Result<(OwnerStep, Vec<usize>), CandidateReductionError> {
    let owner = &p.owners[&[true]];
    let mut visits = Vec::new();
    let result = owner.evaluate_step::<CandidateReductionError>(&key([n]), |batch| {
        visits.push(if std::ptr::eq(batch, owner.batches[0].as_ref()) {
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
            &key([n]),
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        ))
    })?;
    Ok((result, visits))
}
fn matches(
    p: &CandidateOwnerPrograms<1>,
    lo: u64,
    hi: u64,
) -> Vec<(Vec<u64>, Vec<Option<u64>>, Disposition)> {
    let mut out = Vec::new();
    p.visit_owner_domain_matches(
        [true],
        &[lo],
        &[Some(hi)],
        None,
        Default::default(),
        &AtomicBool::new(false),
        |piece| {
            out.push((
                piece.lower().to_vec(),
                piece.upper().to_vec(),
                piece.disposition(),
            ));
            ControlFlow::Continue(())
        },
    )
    .unwrap();
    out
}
#[test]
fn absent_and_explicit_all_subsets_have_identical_dispatch_and_metadata() {
    let a = fixture(None, 3, false);
    let b = fixture(Some(&[0, 1, 2]), 3, false);
    assert_eq!(matches(&a, 0, 5), matches(&b, 0, 5));
    for p in [&a, &b] {
        assert_eq!(p.terminal_count(), 1);
        assert_eq!(p.owners[&[true]].batches[0].rules.len(), 3);
    }
    for n in 1..=6 {
        let (x, xv) = step(&a, n).unwrap();
        let (y, yv) = step(&b, n).unwrap();
        assert_eq!(xv, yv);
        match (x, y) {
            (OwnerStep::Applied(x), OwnerStep::Applied(y)) => assert_eq!(x, y),
            (OwnerStep::Terminal, OwnerStep::Terminal)
            | (OwnerStep::Uncovered, OwnerStep::Uncovered) => {}
            _ => panic!("dispatch differs"),
        }
    }
}
#[test]
fn empty_subset_keeps_only_baseline_outcomes_and_all_original_holes() {
    let p = fixture(Some(&[]), 3, false);
    let first = &p.owners[&[true]].batches[0];
    assert!(first.rules.is_empty());
    assert_eq!(first.terminals, [key([1])].into());
    assert_eq!(first.deferred_points, Some([key([2])].into()));
    assert!(matches!(step(&p, 1).unwrap().0, OwnerStep::Terminal));
    for n in 2..=4 {
        let (result, visits) = step(&p, n).unwrap();
        assert!(matches!(result, OwnerStep::Applied(_)));
        assert_eq!(visits.last(), Some(&1));
    }
    assert!(matches!(step(&p, 5).unwrap().0, OwnerStep::Uncovered));
    assert!(
        matches(&p, 0, 4)
            .iter()
            .all(|(_, _, d)| !matches!(d, Disposition::SelectedRule { batch: 0, .. }))
    );
}
#[test]
fn sparse_saved_ordinal_survives_slot_zero_in_match_apply_guarded_and_literal() {
    let p = fixture(Some(&[110]), 111, false);
    assert_eq!(p.owners[&[true]].batches[0].rules[0].ordinal, 110);
    assert_eq!(
        matches(&p, 2, 2)[0].2,
        Disposition::SelectedRule {
            batch: 0,
            rule: 110
        }
    );
    assert_eq!(step(&p, 3).unwrap().1, vec![0]);
    let mut selected = None;
    let stats = p
        .visit_owner_applied_successors(
            [true],
            &[2],
            &[Some(2)],
            None,
            Default::default(),
            &AtomicBool::new(false),
            |event| {
                if let OwnerAppliedEvent::Classified(piece) = event {
                    selected = Some(piece.disposition());
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!(
        selected,
        Some(Disposition::SelectedRule {
            batch: 0,
            rule: 110
        })
    );
    assert_eq!(stats.problems, 0);
    assert!(stats.successors > 0);
    let mut guarded = false;
    p.visit_owner_guarded_rule_successors(
        [true],
        0,
        110,
        &[2],
        &[Some(2)],
        None,
        Default::default(),
        &AtomicBool::new(false),
        |event| {
            if matches!(event, OwnerGuardedEvent::Successor(_)) {
                guarded = true;
            }
            ControlFlow::Continue(())
        },
    )
    .unwrap();
    assert!(guarded);
    assert!(
        p.visit_owner_guarded_rule_successors(
            [true],
            0,
            0,
            &[2],
            &[Some(2)],
            None,
            Default::default(),
            &AtomicBool::new(false),
            |_| ControlFlow::Continue(())
        )
        .is_err()
    );
}
#[test]
fn selected_generic_rule_cannot_override_terminal_or_preferred_residual_hole() {
    let p = fixture(Some(&[2]), 3, false);
    assert!(matches!(step(&p, 1).unwrap().0, OwnerStep::Terminal));
    assert_eq!(step(&p, 2).unwrap().1, vec![1]);
    assert_eq!(step(&p, 3).unwrap().1, vec![0]);
    let at = matches(&p, 0, 2);
    assert!(at.iter().any(|(lower, upper, d)| lower == &[0]
        && upper == &[Some(0)]
        && *d == Disposition::Terminal { batch: 0 }));
    assert!(
        at.iter()
            .any(|(_, _, d)| *d == Disposition::SelectedRule { batch: 1, rule: 0 })
    );
}
#[test]
fn rebuilt_optional_positions_require_enabled_ordinary_coverage() {
    let only = fixture(Some(&[2]), 3, true);
    let paired = fixture(Some(&[0, 2]), 3, true);
    assert_eq!(
        only.owners[&[true]].batches[0].whole_piece_alternatives,
        vec![0]
    );
    assert_eq!(
        paired.owners[&[true]].batches[0].whole_piece_alternatives,
        vec![1]
    );
    assert_eq!(paired.owners[&[true]].batches[0].coalescing_bound, 1);
    assert_eq!(step(&only, 3).unwrap().1, vec![0, 1]);
    assert_eq!(step(&paired, 3).unwrap().1, vec![0]);
    assert_eq!(
        matches(&only, 2, 2)[0].2,
        Disposition::SelectedRule { batch: 1, rule: 1 }
    );
    assert_eq!(
        matches(&paired, 2, 2)[0].2,
        Disposition::SelectedRule { batch: 0, rule: 2 }
    );
}
#[test]
fn unknown_duplicate_unsorted_and_foreign_owner_selectors_are_refused() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let shared = context(family.clone(), None, Default::default());
    for (owner, ids) in [
        ([true], vec![1]),
        ([true], vec![0, 0]),
        ([true], vec![1, 0]),
        ([false], vec![0]),
    ] {
        let input = || input([true], None, vec![rule(&family, [2], &[([1], 1)])], &[[1]]);
        assert!(
            CandidateOwnerPrograms::try_new_with_preference_rule_subsets(
                shared.clone(),
                [input()],
                [input()],
                &BTreeMap::from([(owner, ids.as_slice())])
            )
            .is_err()
        );
    }
}
#[test]
fn unselected_invalid_rule_still_fails_full_native_admission() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let shared = context(family.clone(), None, Default::default());
    let mut invalid = rule(&family, [2], &[([1], 1)]);
    invalid.candidate.target = Integral::numeric([3]).unwrap();
    let error = CandidateOwnerPrograms::try_new_with_preference_rule_subsets(
        shared,
        [input([true], None, vec![], &[[1]])],
        [input([true], None, vec![invalid], &[])],
        &BTreeMap::from([([true], &[][..])]),
    )
    .unwrap_err();
    assert!(error.to_string().contains("canonical case"));
}
#[test]
fn retained_hard_error_and_mixed_box_resource_refusal_never_enable_fallback() {
    let mut p = fixture(Some(&[2]), 3, false);
    let mut emitted = Vec::new();
    let limits = OwnerDomainMatchLimits {
        max_rules: 0,
        ..Default::default()
    };
    assert!(
        p.visit_owner_domain_matches(
            [true],
            &[2],
            &[Some(4)],
            None,
            limits,
            &AtomicBool::new(false),
            |piece| {
                emitted.push(piece.disposition());
                ControlFlow::Continue(())
            }
        )
        .is_err()
    );
    assert!(
        !emitted
            .iter()
            .any(|d| matches!(d, Disposition::SelectedRule { batch: 1, .. }))
    );
    let owner = Arc::get_mut(p.owners.get_mut(&[true]).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0].rhs[0].shift = [1];
    assert!(matches!(
        step(&p, 3),
        Err(CandidateReductionError::NonDescending { .. })
    ));
}
