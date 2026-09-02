use super::super::super::{CompletionGeometryLimits, LatticeCardinality};
use super::super::limits::InvolutiveWorkBudget;
use super::super::*;
use super::support::*;

fn flat_janet_divisor(
    basis: &JanetBasisEpoch,
    target: &ForwardShift,
    excluded: Option<usize>,
) -> Option<usize> {
    basis.elements().iter().find_map(|element| {
        (excluded != Some(element.ordinal())
            && element
                .multiplicative()
                .janet_divides(element.leading_shift(), target))
        .then_some(element.ordinal())
    })
}

fn assert_index_matches_flat(
    basis: &JanetBasisEpoch,
    targets: impl IntoIterator<Item = ForwardShift>,
    limits: InvolutiveLimits,
) -> InvolutiveWorkCensus {
    let mut scratch = basis.try_divisor_scratch(limits).unwrap();
    let mut work = InvolutiveWorkBudget::default();
    for target in targets {
        for excluded in std::iter::once(None).chain((0..basis.elements().len()).map(Some)) {
            let expected = flat_janet_divisor(basis, &target, excluded);
            let actual = basis
                .try_janet_divisor_with_scratch(&target, excluded, &mut scratch, limits, &mut work)
                .unwrap();
            assert_eq!(
                actual,
                expected,
                "target={:?} excluded={excluded:?}",
                target.values()
            );
        }
    }
    work.census()
}

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

    let targets = (0..=3).flat_map(|x0| {
        (0..=3).flat_map(move |x1| (0..=3).map(move |x2| shift(&[x0, x1, x2], limits)))
    });
    let census = assert_index_matches_flat(&basis, targets, limits);
    assert!(census.divisor_index_query_operations() > 0);
    assert_eq!(census.normal_form_divisor_visits(), 0);
}

#[test]
fn indexed_division_matches_flat_oracle_on_deterministic_random_bases() {
    let limits = InvolutiveLimits::default();
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    for arity in 1..=6 {
        let row_count = arity + 4;
        let context = context(arity);
        let ordering = active_ordering(arity, limits);
        let mut leaders = std::collections::BTreeSet::new();
        for axis in 0..arity {
            let mut pure_power = vec![0; arity];
            pure_power[axis] = 5;
            leaders.insert(pure_power);
        }
        while leaders.len() < row_count {
            leaders.insert((0..arity).map(|_| next() % 6).collect::<Vec<_>>());
        }
        let leader_slices = leaders.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let basis = epoch(&leader_slices, &context, &ordering, limits);
        let targets = (0..32)
            .map(|_| shift(&(0..arity).map(|_| next() % 9).collect::<Vec<_>>(), limits))
            .collect::<Vec<_>>();
        assert_index_matches_flat(&basis, targets, limits);
    }
}

#[test]
fn k6_like_masks_preserve_lowest_ordinal_with_every_exclusion() {
    let limits = InvolutiveLimits::default();
    let context = context(6);
    let ordering = active_ordering(6, limits);
    let leaders: &[&[u64]] = &[
        &[0, 2, 1, 1, 0, 2],
        &[1, 1, 1, 0, 1, 1],
        &[1, 1, 1, 1, 0, 1],
        &[1, 1, 1, 1, 1, 1],
        &[1, 1, 2, 1, 1, 1],
        &[1, 2, 1, 1, 1, 1],
        &[2, 1, 1, 1, 0, 0],
        &[2, 1, 1, 1, 1, 0],
        &[2, 1, 1, 2, 0, 0],
    ];
    let forward = epoch(leaders, &context, &ordering, limits);
    let mut reversed = leaders.to_vec();
    reversed.reverse();
    let reversed = epoch(&reversed, &context, &ordering, limits);
    let targets = (0..512)
        .map(|ordinal| {
            shift(
                &[
                    ordinal % 4,
                    (ordinal / 4) % 4,
                    (ordinal / 16) % 4,
                    (ordinal / 64) % 4,
                    (ordinal / 256) % 2,
                    (ordinal / 128) % 4,
                ],
                limits,
            )
        })
        .collect::<Vec<_>>();
    let forward_census = assert_index_matches_flat(&forward, targets.clone(), limits);
    let reversed_census = assert_index_matches_flat(&reversed, targets, limits);
    assert_eq!(
        forward_census.divisor_index_query_operations(),
        reversed_census.divisor_index_query_operations()
    );
    assert_eq!(
        forward_census.normal_form_divisor_visits(),
        reversed_census.normal_form_divisor_visits()
    );
}

#[test]
fn divisor_index_construction_scratch_and_query_caps_are_exact_one_below() {
    let defaults = InvolutiveLimits::default();
    let context = context(3);
    let ordering = active_ordering(3, defaults);
    let leaders: &[&[u64]] = &[&[0, 0, 3], &[0, 2, 1], &[1, 0, 2], &[1, 1, 0], &[2, 0, 0]];
    let build = |limits: InvolutiveLimits| {
        let consequences = leaders.iter().enumerate().map(|(ordinal, powers)| {
            monomial_consequence(ordinal, powers, &ordering, &context, limits)
        });
        let mut work = InvolutiveWorkBudget::default();
        let epoch = JanetBasisEpoch::try_initial_with_budget(
            consequences,
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
            &mut work,
        )?;
        Ok::<_, InvolutiveError>((epoch, work.census()))
    };
    let (basis, build_census) = build(defaults).unwrap();
    let build_operations = build_census.divisor_index_build_operations();
    assert!(build_operations > 0);
    let build_cap = InvolutiveLimits {
        max_divisor_index_build_operations: build_operations - 1,
        ..defaults
    };
    assert_eq!(
        build(build_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet divisor index build operations",
            requested: build_operations,
            limit: build_operations - 1,
        })
    );

    let build_scratch_bytes = 2 * leaders.len() * std::mem::size_of::<(u64, usize)>();
    let build_scratch_cap = InvolutiveLimits {
        max_divisor_index_build_scratch_bytes: build_scratch_bytes - 1,
        ..defaults
    };
    assert_eq!(
        build(build_scratch_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet divisor index build scratch bytes",
            requested: build_scratch_bytes,
            limit: build_scratch_bytes - 1,
        })
    );

    let retained_bytes = basis.divisor_index_retained_bytes();
    let retained_cap = InvolutiveLimits {
        max_divisor_index_retained_bytes: retained_bytes - 1,
        ..defaults
    };
    assert_eq!(
        build(retained_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet divisor index retained bytes",
            requested: retained_bytes,
            limit: retained_bytes - 1,
        })
    );

    let scratch = basis.try_divisor_scratch(defaults).unwrap();
    let scratch_bytes = scratch.retained_bytes();
    let scratch_cap = InvolutiveLimits {
        max_divisor_index_scratch_bytes: scratch_bytes - 1,
        ..defaults
    };
    assert_eq!(
        basis.try_divisor_scratch(scratch_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet divisor index scratch bytes",
            requested: scratch_bytes,
            limit: scratch_bytes - 1,
        })
    );

    let targets = [shift(&[3, 2, 3], defaults), shift(&[2, 4, 1], defaults)];
    let mut scratch = basis.try_divisor_scratch(defaults).unwrap();
    let mut query_work = InvolutiveWorkBudget::default();
    for target in &targets {
        basis
            .try_janet_divisor_with_scratch(target, None, &mut scratch, defaults, &mut query_work)
            .unwrap();
    }
    let query_operations = query_work.census().divisor_index_query_operations();
    assert!(query_operations > 0);
    let query_cap = InvolutiveLimits {
        max_divisor_index_query_operations: query_operations - 1,
        ..defaults
    };
    let mut scratch = basis.try_divisor_scratch(query_cap).unwrap();
    let mut query_work = InvolutiveWorkBudget::default();
    basis
        .try_janet_divisor_with_scratch(&targets[0], None, &mut scratch, query_cap, &mut query_work)
        .unwrap();
    assert_eq!(
        basis.try_janet_divisor_with_scratch(
            &targets[1],
            None,
            &mut scratch,
            query_cap,
            &mut query_work,
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet divisor index query operations",
            requested: query_operations,
            limit: query_operations - 1,
        })
    );
}

#[test]
fn divisor_index_rejects_wrong_targets_foreign_scratch_and_bad_exclusions() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let basis = epoch(&[&[2, 0], &[0, 3]], &context, &ordering, limits);
    assert_eq!(
        basis.try_janet_divisor(&shift(&[1], limits)),
        Err(InvolutiveError::WrongArity {
            object: "Janet divisibility target",
            expected: 2,
            actual: 1,
        })
    );

    let mut old_scratch = basis.try_divisor_scratch(limits).unwrap();
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
    let target = shift(&[2, 3], limits);
    let mut work = InvolutiveWorkBudget::default();
    assert_eq!(
        successor.try_janet_divisor_with_scratch(
            &target,
            None,
            &mut old_scratch,
            limits,
            &mut work,
        ),
        Err(InvolutiveError::StaleEpoch {
            expected: successor.epoch().clone(),
            actual: basis.epoch().clone(),
        })
    );

    let mut scratch = successor.try_divisor_scratch(limits).unwrap();
    assert_eq!(
        successor.try_janet_divisor_with_scratch(
            &target,
            Some(successor.elements().len()),
            &mut scratch,
            limits,
            &mut work,
        ),
        Err(InvolutiveError::InvalidProlongation {
            detail: "excluded Janet divisor is outside the current epoch",
        })
    );

    let no_candidate = shift(&[0, 0], limits);
    assert_eq!(
        successor
            .try_janet_divisor_with_scratch(&no_candidate, None, &mut scratch, limits, &mut work,)
            .unwrap(),
        None
    );
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
