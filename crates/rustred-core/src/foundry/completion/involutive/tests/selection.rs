use super::super::super::CompletionGeometryLimits;
use super::super::limits::InvolutiveWorkBudget;
use super::super::selection::try_select_janet_reduction;
use super::super::*;
use super::support::*;

#[test]
fn empty_support_rejects_a_stale_divisor_scratch() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let basis = epoch(&[&[2, 0], &[0, 3]], &context, &ordering, limits);
    let mut stale_scratch = basis.try_divisor_scratch(limits).unwrap();
    let successor = basis
        .try_successor(
            [monomial_consequence(
                2,
                &[1, 3],
                &ordering,
                &context,
                limits,
            )],
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        )
        .unwrap();
    let mut divisor_visits = 0;
    let mut work = InvolutiveWorkBudget::default();

    assert_eq!(
        try_select_janet_reduction(
            successor.division(),
            std::iter::empty::<&ForwardShift>(),
            None,
            &ordering,
            limits,
            &mut divisor_visits,
            &mut stale_scratch,
            &mut work,
        ),
        Err(InvolutiveError::StaleEpoch {
            expected: successor.epoch().clone(),
            actual: basis.epoch().clone(),
        })
    );
    assert_eq!(divisor_visits, 0);
    assert_eq!(work.census(), InvolutiveWorkBudget::default().census());
}

#[test]
fn empty_support_rejects_bad_exclusion_and_malformed_scratch() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let basis = epoch(&[&[2, 0], &[0, 3]], &context, &ordering, limits);
    let mut scratch = basis.try_divisor_scratch(limits).unwrap();
    let mut divisor_visits = 0;
    let mut work = InvolutiveWorkBudget::default();

    assert_eq!(
        try_select_janet_reduction(
            basis.division(),
            std::iter::empty::<&ForwardShift>(),
            Some(basis.elements().len()),
            &ordering,
            limits,
            &mut divisor_visits,
            &mut scratch,
            &mut work,
        ),
        Err(InvolutiveError::InvalidProlongation {
            detail: "excluded Janet divisor is outside the current epoch",
        })
    );

    scratch.corrupt_sealed_shape_for_test();
    assert_eq!(
        try_select_janet_reduction(
            basis.division(),
            std::iter::empty::<&ForwardShift>(),
            None,
            &ordering,
            limits,
            &mut divisor_visits,
            &mut scratch,
            &mut work,
        ),
        Err(InvolutiveError::Invariant {
            detail: "Janet divisor query scratch has a malformed sealed shape",
        })
    );
    assert_eq!(divisor_visits, 0);
    assert_eq!(work.census(), InvolutiveWorkBudget::default().census());
}

#[test]
fn valid_empty_support_is_a_no_work_no_selection_query() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let basis = epoch(&[&[2, 0], &[0, 3]], &context, &ordering, limits);
    let mut scratch = basis.try_divisor_scratch(limits).unwrap();
    let mut divisor_visits = 0;
    let mut work = InvolutiveWorkBudget::default();

    assert_eq!(
        try_select_janet_reduction(
            basis.division(),
            std::iter::empty::<&ForwardShift>(),
            None,
            &ordering,
            limits,
            &mut divisor_visits,
            &mut scratch,
            &mut work,
        )
        .unwrap(),
        None
    );
    assert_eq!(divisor_visits, 0);
    assert_eq!(work.census(), InvolutiveWorkBudget::default().census());
}

#[test]
fn empty_support_rejects_sibling_scratch_at_the_same_revision_depth() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let base = epoch(&[&[2, 0], &[0, 3]], &context, &ordering, limits);
    let left = base
        .try_successor(
            [monomial_consequence(
                30,
                &[1, 3],
                &ordering,
                &context,
                limits,
            )],
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        )
        .unwrap();
    let right = base
        .try_successor(
            [monomial_consequence(
                40,
                &[0, 4],
                &ordering,
                &context,
                limits,
            )],
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        )
        .unwrap();
    assert_eq!(left.epoch().revision(), right.epoch().revision());
    assert!(left.epoch().same_instance(right.epoch()));
    assert_ne!(left.epoch(), right.epoch());

    let mut sibling_scratch = left.try_divisor_scratch(limits).unwrap();
    let mut divisor_visits = 0;
    let mut work = InvolutiveWorkBudget::default();
    assert_eq!(
        try_select_janet_reduction(
            right.division(),
            std::iter::empty::<&ForwardShift>(),
            None,
            &ordering,
            limits,
            &mut divisor_visits,
            &mut sibling_scratch,
            &mut work,
        ),
        Err(InvolutiveError::StaleEpoch {
            expected: right.epoch().clone(),
            actual: left.epoch().clone(),
        })
    );
    assert_eq!(divisor_visits, 0);
    assert_eq!(work.census(), InvolutiveWorkBudget::default().census());
}
