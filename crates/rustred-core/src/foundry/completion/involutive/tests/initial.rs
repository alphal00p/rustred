use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::super::super::CompletionGeometryLimits;
use super::super::*;
use super::support::*;

fn consequence(
    source_ordinal: usize,
    terms: Vec<(Vec<u64>, IndexedCoefficient)>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> OreConsequence {
    let row = OreRow::try_new(
        ordering,
        terms
            .into_iter()
            .map(|(powers, coefficient)| (shift(&powers, limits), coefficient)),
        context,
        limits,
    )
    .unwrap();
    OreConsequence::try_from_source(source_ordinal, row, ordering, context, limits).unwrap()
}

fn preprocess(
    rows: Vec<OreConsequence>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> JanetInitialReduction {
    try_preprocess_initial_basis(
        rows,
        ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap()
}

#[test]
fn coincident_heads_produce_a_strictly_lower_nonzero_row_without_losing_provenance() {
    let limits = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, limits);
    let rows = vec![
        consequence(
            0,
            vec![(vec![2], context.one()), (vec![0], context.one())],
            &ordering,
            &context,
            limits,
        ),
        consequence(
            1,
            vec![(vec![2], context.one()), (vec![1], context.one())],
            &ordering,
            &context,
            limits,
        ),
    ];
    let initial = preprocess(rows, &ordering, &context, limits);

    assert_eq!(initial.census().input_rows(), 2);
    assert_eq!(initial.census().retained_rows(), 2);
    assert_eq!(initial.census().equal_head_eliminations(), 1);
    assert_eq!(initial.census().nonzero_remainders(), 1);
    assert_eq!(initial.census().zero_remainders(), 0);
    assert_eq!(initial.census().max_head_class(), 2);
    assert_eq!(initial.work_census().normal_form_steps(), 1);
    let leaders = initial
        .epoch()
        .elements()
        .iter()
        .map(|element| element.leading_shift().values().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(leaders, vec![vec![1], vec![2]]);
    let lower = initial
        .epoch()
        .elements()
        .iter()
        .find(|element| element.leading_shift().values() == [1])
        .unwrap();
    assert_eq!(lower.consequence().provenance().terms().len(), 2);
    assert_eq!(
        lower
            .consequence()
            .provenance()
            .terms()
            .iter()
            .map(|term| term.source_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn exact_zero_duplicate_keeps_its_sparse_witness_and_guard_through_queue_exhaustion() {
    let defaults = InvolutiveLimits::default();
    let limits = InvolutiveLimits {
        max_localization_guards: 1,
        ..defaults
    };
    let context = context(1);
    let ordering = active_ordering(1, defaults);
    let n = context.index(0).unwrap();
    let make_rows = || {
        vec![
            consequence(0, vec![(vec![1], n.clone())], &ordering, &context, defaults),
            consequence(
                1,
                vec![(vec![1], context.one())],
                &ordering,
                &context,
                defaults,
            ),
        ]
    };
    let initial = preprocess(make_rows(), &ordering, &context, limits);
    assert_eq!(initial.census().zero_remainders(), 1);
    assert_eq!(initial.census().retained_rows(), 1);
    assert_eq!(initial.zero_remainders().len(), 1);
    let zero = &initial.zero_remainders()[0];
    assert!(zero.is_zero());
    assert_eq!(zero.provenance().terms().len(), 2);
    assert_eq!(
        zero.provenance().terms()[0].left_coefficient(),
        &context
            .div(&context.integer(-1), &n)
            .expect("the exact source-module coefficient is -1/n")
    );
    assert_eq!(
        zero.provenance().terms()[1].left_coefficient(),
        &context.one()
    );

    let proposal = try_complete_janet_proposal_from_consequences(
        make_rows(),
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(proposal.census().initial_reduction().zero_remainders(), 1);
    assert_eq!(proposal.census().attempted_prolongations(), 0);
    assert_eq!(proposal.localization_witness().guards().len(), 1);
    assert_eq!(
        proposal.localization_witness().guards()[0].as_ref(),
        &context
            .numerator_condition_with_limits(&n, defaults.indexed_algebra.exact_algebra)
            .unwrap()
    );
    assert_eq!(proposal.work_census().normal_form_steps(), 1);
    assert_eq!(proposal.work_census().autoreduction_passes(), 1);
}

#[test]
fn three_row_permutations_and_a_generated_lower_head_collision_are_deterministic() {
    let limits = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, limits);
    let make = |source_ordinal| match source_ordinal {
        0 => consequence(
            0,
            vec![(vec![3], context.one())],
            &ordering,
            &context,
            limits,
        ),
        1 => consequence(
            1,
            vec![(vec![3], context.one()), (vec![2], context.one())],
            &ordering,
            &context,
            limits,
        ),
        2 => consequence(
            2,
            vec![
                (vec![3], context.one()),
                (vec![2], context.one()),
                (vec![1], context.one()),
            ],
            &ordering,
            &context,
            limits,
        ),
        _ => unreachable!(),
    };
    let forward = preprocess(vec![make(0), make(1), make(2)], &ordering, &context, limits);
    let permuted = preprocess(vec![make(2), make(0), make(1)], &ordering, &context, limits);

    assert_eq!(forward.census(), permuted.census());
    assert_eq!(forward.work_census(), permuted.work_census());
    assert_eq!(
        forward.localization_witness(),
        permuted.localization_witness()
    );
    assert_eq!(forward.census().equal_head_eliminations(), 3);
    assert_eq!(forward.census().nonzero_remainders(), 3);
    assert_eq!(forward.census().zero_remainders(), 0);
    assert_eq!(forward.census().cascading_collisions(), 1);
    assert_eq!(forward.census().max_collision_chain(), 2);
    assert_eq!(forward.census().max_head_class(), 3);
    assert_eq!(
        forward
            .epoch()
            .elements()
            .iter()
            .map(|element| element.consequence())
            .collect::<Vec<_>>(),
        permuted
            .epoch()
            .elements()
            .iter()
            .map(|element| element.consequence())
            .collect::<Vec<_>>()
    );
    assert_eq!(forward.zero_remainders(), permuted.zero_remainders());
}

#[test]
fn equal_head_work_caps_are_cumulative_and_fail_atomically_before_an_epoch_exists() {
    let defaults = InvolutiveLimits::default();
    let context_1d = context(1);
    let ordering = active_ordering(1, defaults);
    let make_rows = || {
        vec![
            monomial_consequence(0, &[1], &ordering, &context_1d, defaults),
            monomial_consequence(1, &[1], &ordering, &context_1d, defaults),
        ]
    };
    let step_cap = InvolutiveLimits {
        max_normal_form_steps: 0,
        ..defaults
    };
    assert_eq!(
        try_preprocess_initial_basis(
            make_rows(),
            &ordering,
            &context_1d,
            step_cap,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet normal-form steps",
            requested: 1,
            limit: 0,
        })
    );
    let coefficient_cap = InvolutiveLimits {
        max_exact_coefficient_operations: 1,
        ..defaults
    };
    assert_eq!(
        try_preprocess_initial_basis(
            make_rows(),
            &ordering,
            &context_1d,
            coefficient_cap,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet exact coefficient operations",
            requested: 2,
            limit: 1,
        })
    );

    let guard_cap = InvolutiveLimits {
        max_localization_guards: 0,
        ..defaults
    };
    let n = context_1d.index(0).unwrap();
    assert_eq!(
        try_preprocess_initial_basis(
            vec![
                consequence(0, vec![(vec![1], n)], &ordering, &context_1d, defaults,),
                monomial_consequence(1, &[1], &ordering, &context_1d, defaults),
            ],
            &ordering,
            &context_1d,
            guard_cap,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Ore localization guards",
            requested: 1,
            limit: 0,
        })
    );

    // One duplicate-head cancellation consumes the sole allowed normal-form
    // step. The first mandatory Janet queue reduction must see that same
    // cumulative ledger rather than starting from zero.
    let context_2d = context(2);
    let ordering_2d = active_ordering(2, defaults);
    let cumulative_cap = InvolutiveLimits {
        max_normal_form_steps: 1,
        ..defaults
    };
    assert_eq!(
        try_complete_janet_proposal_from_consequences(
            vec![
                monomial_consequence(0, &[1, 0], &ordering_2d, &context_2d, defaults),
                monomial_consequence(1, &[1, 0], &ordering_2d, &context_2d, defaults),
                monomial_consequence(2, &[0, 1], &ordering_2d, &context_2d, defaults),
            ],
            &ordering_2d,
            &context_2d,
            cumulative_cap,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet normal-form steps",
            requested: 2,
            limit: 1,
        })
    );
}
