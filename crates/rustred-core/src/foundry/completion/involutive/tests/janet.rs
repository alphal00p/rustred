use super::super::super::{CompletionGeometryLimits, LatticeCardinality};
use super::super::*;
use super::support::*;

#[test]
fn basis_admission_is_exactly_monic_and_preserves_guarded_source_replay() {
    let limits = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, limits);
    let zero = shift(&[0], limits);
    let e = shift(&[1], limits);
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let leading = context.div(&n, &n_plus_one).unwrap();
    let lower = context.integer(2);
    let inverse = context.div(&context.one(), &leading).unwrap();
    let expected_lower = context.mul(&inverse, &lower).unwrap();
    let existing_guard = context
        .numerator_condition_with_limits(&n_plus_one, limits.indexed_algebra.exact_algebra)
        .unwrap();
    let required_guard = context
        .numerator_condition_with_limits(&n, limits.indexed_algebra.exact_algebra)
        .unwrap();
    let source = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [(zero.clone(), lower.clone()), (e.clone(), leading.clone())],
            &context,
            limits,
        )
        .unwrap(),
        &ordering,
        &context,
        limits,
    )
    .unwrap()
    .try_require_nonzero_guard(existing_guard.clone(), &context, limits)
    .unwrap()
    .0;
    let basis = JanetBasisEpoch::try_initial(
        [source],
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let normalized = basis.elements()[0].consequence();

    assert_eq!(normalized.row().coefficient(&e), Some(&context.one()));
    assert_eq!(normalized.row().coefficient(&zero), Some(&expected_lower));
    assert_eq!(normalized.provenance().terms().len(), 1);
    assert_eq!(
        normalized.provenance().terms()[0].left_coefficient(),
        &inverse
    );
    assert_eq!(normalized.required_nonzero_guards().len(), 2);
    for expected in [&existing_guard, &required_guard] {
        assert!(
            normalized
                .required_nonzero_guards()
                .iter()
                .any(|actual| actual.as_ref() == expected)
        );
    }

    // Exact source-module replay: multiplying every normalized row and
    // provenance coefficient by the old pivot reconstructs the input source.
    assert_eq!(
        context
            .mul(normalized.row().coefficient(&e).unwrap(), &leading)
            .unwrap(),
        leading
    );
    assert_eq!(
        context
            .mul(normalized.row().coefficient(&zero).unwrap(), &leading)
            .unwrap(),
        lower
    );
    assert_eq!(
        context
            .mul(
                normalized.provenance().terms()[0].left_coefficient(),
                &leading,
            )
            .unwrap(),
        context.one()
    );
}

#[test]
fn already_monic_admission_is_stable_and_adds_no_localization() {
    let limits = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, limits);
    let make = || {
        OreConsequence::try_from_source(
            0,
            OreRow::try_new(
                &ordering,
                [
                    (shift(&[0], limits), context.integer(3)),
                    (shift(&[1], limits), context.one()),
                ],
                &context,
                limits,
            )
            .unwrap(),
            &ordering,
            &context,
            limits,
        )
        .unwrap()
    };
    let expected = make();
    let basis = JanetBasisEpoch::try_initial(
        [make()],
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();

    assert_eq!(basis.elements()[0].consequence(), &expected);
    assert!(
        basis.elements()[0]
            .consequence()
            .required_nonzero_guards()
            .is_empty()
    );
}

#[test]
fn janet_masks_queues_and_epochs_are_deterministic_and_stale_safe() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let forward = JanetBasisEpoch::try_initial(
        [
            monomial_consequence(10, &[2, 0], &ordering, &context, limits),
            monomial_consequence(20, &[0, 3], &ordering, &context, limits),
        ],
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let reversed = JanetBasisEpoch::try_initial(
        [
            monomial_consequence(20, &[0, 3], &ordering, &context, limits),
            monomial_consequence(10, &[2, 0], &ordering, &context, limits),
        ],
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();

    let forward_shape = forward
        .elements()
        .iter()
        .map(|element| {
            (
                element.leading_shift().values().to_vec(),
                element.multiplicative().bits().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    let reversed_shape = reversed
        .elements()
        .iter()
        .map(|element| {
            (
                element.leading_shift().values().to_vec(),
                element.multiplicative().bits().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(forward_shape, reversed_shape);
    assert_eq!(
        forward_shape,
        vec![
            (vec![2, 0], vec![true, true]),
            (vec![0, 3], vec![false, true])
        ]
    );
    let forward_obligations = forward
        .prolongations()
        .iter()
        .map(|obligation| {
            (
                obligation.basis_ordinal(),
                obligation.variable(),
                obligation.target_leading_shift().values().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    let reversed_obligations = reversed
        .prolongations()
        .iter()
        .map(|obligation| {
            (
                obligation.basis_ordinal(),
                obligation.variable(),
                obligation.target_leading_shift().values().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(forward_obligations, reversed_obligations);
    assert_eq!(forward.prolongations().len(), 1);
    let obligation = forward.prolongations()[0].clone();
    assert_eq!(obligation.basis_ordinal(), 1);
    assert_eq!(obligation.variable(), 0);
    assert_eq!(obligation.target_leading_shift().values(), &[1, 3]);
    assert_eq!(
        reversed.require_current(&obligation),
        Err(InvolutiveError::StaleEpoch {
            expected: reversed.epoch().clone(),
            actual: forward.epoch().clone(),
        })
    );
    assert_eq!(
        forward.try_janet_divisor(&shift(&[3, 0], limits)).unwrap(),
        Some(0)
    );
    assert_eq!(
        forward.try_janet_divisor(&shift(&[1, 3], limits)).unwrap(),
        None
    );

    let shifted = forward
        .try_apply_prolongation(&obligation, &ordering, &context, limits)
        .unwrap();
    assert_eq!(
        shifted
            .row()
            .try_leading_term(&ordering)
            .unwrap()
            .unwrap()
            .0
            .shift()
            .values(),
        &[1, 3]
    );

    let successor = forward
        .try_successor(
            [monomial_consequence(
                30,
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
    assert_eq!(successor.epoch().revision(), 1);
    assert_eq!(successor.predecessor(), Some(forward.epoch()));
    assert_eq!(
        successor.require_current(&obligation),
        Err(InvolutiveError::StaleEpoch {
            expected: successor.epoch().clone(),
            actual: forward.epoch().clone(),
        })
    );
}

#[test]
fn pure_powers_characterize_the_finite_monomial_complement() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let finite = epoch(&[&[2, 0], &[0, 3]], &context, &ordering, limits);

    assert_eq!(
        finite.try_uncovered_cardinality(6).unwrap(),
        LatticeCardinality::Finite(6)
    );
    assert!(finite.uncovered_partition().is_finite());
    assert!(finite.pure_power_coverage().is_complete());
    assert_eq!(finite.pure_power_coverage().exponent(0), Some(2));
    assert_eq!(finite.pure_power_coverage().exponent(1), Some(3));
    assert_eq!(finite.leading_ideal().generators().len(), 2);

    let missing = epoch(&[&[2, 0]], &context, &ordering, limits);
    assert_eq!(
        missing.try_uncovered_cardinality(usize::MAX).unwrap(),
        LatticeCardinality::Infinite
    );
    assert!(!missing.pure_power_coverage().is_complete());
    assert_eq!(
        missing
            .pure_power_coverage()
            .missing_axes()
            .collect::<Vec<_>>(),
        vec![1]
    );
}

#[test]
fn blind_priority_is_residual_driven_and_never_drops_on_truncation() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let residual = epoch(&[&[2, 2]], &context, &ordering, limits);
    let schedule =
        BlindDomainSchedule::try_from_partition(residual.uncovered_partition(), &ordering, limits)
            .unwrap();
    assert_eq!(schedule.total_box_count(), 2);
    assert!(schedule.has_complete_priority_view());
    assert_eq!(schedule.entries()[0].lower().values(), &[0, 0]);
    assert_eq!(schedule.entries()[0].free_dimension(), 1);

    let candidates = epoch(&[&[2, 0], &[1, 2], &[0, 3]], &context, &ordering, limits);
    assert_eq!(candidates.prolongations().len(), 2);
    let ranked = schedule
        .try_rank_prolongation_ordinals(&candidates, &ordering, limits)
        .unwrap();
    assert_eq!(ranked.len(), candidates.prolongations().len());
    let mut ranked_permutation = ranked.to_vec();
    ranked_permutation.sort_unstable();
    assert_eq!(ranked_permutation, (0..ranked.len()).collect::<Vec<_>>());
    assert_eq!(
        candidates.prolongations()[ranked[0]]
            .target_leading_shift()
            .values(),
        &[1, 3]
    );

    let truncated_limits = InvolutiveLimits {
        max_blind_boxes_retained: 1,
        ..limits
    };
    let truncated = BlindDomainSchedule::try_from_partition(
        residual.uncovered_partition(),
        &ordering,
        truncated_limits,
    )
    .unwrap();
    assert!(truncated.is_truncated());
    assert!(!truncated.has_complete_priority_view());
    let truncated_ranked = truncated
        .try_rank_prolongation_ordinals(&candidates, &ordering, truncated_limits)
        .unwrap();
    assert_eq!(truncated_ranked.len(), candidates.prolongations().len());
    let mut truncated_permutation = truncated_ranked.to_vec();
    truncated_permutation.sort_unstable();
    assert_eq!(truncated_permutation, (0..ranked.len()).collect::<Vec<_>>());
}

#[test]
fn literal_two_dimensional_prolongation_has_a_nonzero_janet_remainder() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let basis = epoch(&[&[2, 0], &[0, 3]], &context, &ordering, limits);
    let prolongation = basis.prolongations()[0].clone();
    let subject = basis
        .try_apply_prolongation(&prolongation, &ordering, &context, limits)
        .unwrap();
    let normal_form = try_janet_normal_form(subject, &basis, &ordering, &context, limits).unwrap();

    assert!(!normal_form.is_zero());
    assert!(normal_form.steps().is_empty());
    let remainder = normal_form.into_remainder();
    assert_eq!(
        remainder
            .row()
            .try_leading_term(&ordering)
            .unwrap()
            .unwrap()
            .0
            .shift()
            .values(),
        &[1, 3]
    );
    let successor = basis
        .try_successor(
            [remainder],
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        )
        .unwrap();
    assert_eq!(successor.epoch().revision(), 1);
    assert!(
        successor
            .elements()
            .iter()
            .any(|element| element.leading_shift().values() == [1, 3])
    );
}

#[test]
fn masks_and_janet_divisibility_match_the_quadratic_definition() {
    let limits = InvolutiveLimits::default();
    let context = context(3);
    let ordering = active_ordering(3, limits);
    let leaders: &[&[u64]] = &[&[0, 0, 3], &[0, 2, 1], &[1, 0, 2], &[1, 1, 0], &[2, 0, 0]];
    let basis = epoch(leaders, &context, &ordering, limits);

    for element in basis.elements() {
        let leader = element.leading_shift().values();
        let expected = (0..3)
            .map(|variable| {
                let maximum = leaders
                    .iter()
                    .filter(|candidate| candidate[..variable] == leader[..variable])
                    .map(|candidate| candidate[variable])
                    .max()
                    .unwrap();
                leader[variable] == maximum
            })
            .collect::<Vec<_>>();
        assert_eq!(element.multiplicative().bits(), expected);
    }

    for x0 in 0..=3 {
        for x1 in 0..=3 {
            for x2 in 0..=3 {
                let target = shift(&[x0, x1, x2], limits);
                let oracle = basis.elements().iter().find_map(|element| {
                    let leader = element.leading_shift().values();
                    leader
                        .iter()
                        .zip(target.values())
                        .zip(element.multiplicative().bits())
                        .all(|((&left, &right), &multiplicative)| {
                            left <= right && (left == right || multiplicative)
                        })
                        .then_some(element.ordinal())
                });
                assert_eq!(basis.try_janet_divisor(&target).unwrap(), oracle);
            }
        }
    }
}

#[test]
fn duplicate_leaders_are_rejected_before_masks_or_queues_are_built() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    assert_eq!(
        JanetBasisEpoch::try_initial(
            [
                monomial_consequence(0, &[1, 1], &ordering, &context, limits),
                monomial_consequence(1, &[1, 1], &ordering, &context, limits),
            ],
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::DuplicateLeadingShift)
    );
}

#[test]
fn blind_schedule_rejects_a_foreign_ore_action_and_truncation_keeps_the_true_prefix() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let residual = epoch(&[&[2, 2]], &context, &ordering, limits);
    let full =
        BlindDomainSchedule::try_from_partition(residual.uncovered_partition(), &ordering, limits)
            .unwrap();
    let truncated_limits = InvolutiveLimits {
        max_blind_boxes_retained: 1,
        ..limits
    };
    let truncated = BlindDomainSchedule::try_from_partition(
        residual.uncovered_partition(),
        &ordering,
        truncated_limits,
    )
    .unwrap();
    assert_eq!(truncated.entries(), &full.entries()[..1]);

    let foreign_ordering = active_ordering(2, limits);
    let foreign_basis = epoch(
        &[&[2, 0], &[1, 2], &[0, 3]],
        &context,
        &foreign_ordering,
        limits,
    );
    assert_eq!(
        full.try_rank_prolongation_ordinals(&foreign_basis, &foreign_ordering, limits),
        Err(InvolutiveError::ForeignOreAction)
    );
    assert_eq!(
        full.try_rank_prolongation_ordinals(&foreign_basis, &ordering, limits),
        Err(InvolutiveError::ForeignOreAction)
    );
}
