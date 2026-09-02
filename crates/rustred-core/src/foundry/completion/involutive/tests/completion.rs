use super::super::super::CompletionGeometryLimits;
use super::super::*;
use super::support::*;

#[test]
fn nonconstant_multistep_normal_form_retains_exact_source_provenance_and_guards() {
    let limits = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, limits);
    let zero = shift(&[0], limits);
    let e = shift(&[1], limits);
    let e2 = shift(&[2], limits);
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let divisor = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [(e.clone(), n.clone()), (zero.clone(), context.one())],
            &context,
            limits,
        )
        .unwrap(),
        &ordering,
        &context,
        limits,
    )
    .unwrap();
    let basis = JanetBasisEpoch::try_initial(
        [divisor],
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let subject = OreConsequence::try_from_source(
        1,
        OreRow::try_new(
            &ordering,
            [
                (e2.clone(), n_plus_one.clone()),
                (e.clone(), n_plus_one),
                (zero.clone(), context.one()),
            ],
            &context,
            limits,
        )
        .unwrap(),
        &ordering,
        &context,
        limits,
    )
    .unwrap();
    let normal_form = try_janet_normal_form(subject, &basis, &ordering, &context, limits).unwrap();

    assert!(normal_form.is_zero());
    assert_eq!(normal_form.steps().len(), 2);
    assert_eq!(normal_form.steps()[0].target_shift(), &e2);
    assert_eq!(normal_form.steps()[1].target_shift(), &e);
    assert!(
        normal_form
            .steps()
            .iter()
            .all(|step| step.required_nonzero().is_none())
    );
    assert_eq!(normal_form.remainder().required_nonzero_guards().len(), 2);
    let provenance = normal_form.remainder().provenance().terms();
    assert_eq!(provenance.len(), 3);
    assert_eq!(provenance[0].source_ordinal(), 0);
    assert_eq!(provenance[0].left_shift(), &zero);
    assert_eq!(provenance[0].left_coefficient(), &context.integer(-1));
    assert_eq!(provenance[1].source_ordinal(), 0);
    assert_eq!(provenance[1].left_shift(), &e);
    assert_eq!(provenance[1].left_coefficient(), &context.integer(-1));
    assert_eq!(provenance[2].source_ordinal(), 1);
    assert_eq!(provenance[2].left_shift(), &zero);
    assert_eq!(provenance[2].left_coefficient(), &context.one());
}

#[test]
fn autoreduction_reduces_lower_terms_and_small_closed_example_reaches_fixed_point() {
    let limits = InvolutiveLimits::default();
    let context_2d = context(2);
    let ordering_2d = active_ordering(2, limits);
    let e1 = shift(&[1, 0], limits);
    let e2_squared = shift(&[0, 2], limits);
    let reducible_lower_term = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering_2d,
            [
                (e2_squared.clone(), context_2d.one()),
                (e1.clone(), context_2d.one()),
            ],
            &context_2d,
            limits,
        )
        .unwrap(),
        &ordering_2d,
        &context_2d,
        limits,
    )
    .unwrap();
    let divisor = monomial_consequence(1, &[1, 0], &ordering_2d, &context_2d, limits);
    let initial = JanetBasisEpoch::try_initial(
        [reducible_lower_term, divisor],
        &ordering_2d,
        &context_2d,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let autoreduced = try_autoreduce_epoch(
        initial,
        &ordering_2d,
        &context_2d,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(autoreduced.epoch().elements().len(), 2);
    let reduced = autoreduced
        .epoch()
        .elements()
        .iter()
        .find(|element| element.leading_shift() == &e2_squared)
        .unwrap();
    assert_eq!(reduced.consequence().row().terms().len(), 1);
    assert_eq!(reduced.consequence().row().terms()[0].shift(), &e2_squared);
    assert_eq!(autoreduced.census().normal_form_steps(), 1);
    assert_eq!(autoreduced.census().dropped_rows(), 0);
    assert_eq!(autoreduced.census().passes(), 2);
    assert_eq!(autoreduced.epoch().epoch().revision(), 1);

    let complete_basis = epoch(&[&[1, 0], &[0, 1]], &context_2d, &ordering_2d, limits);
    let proposal = try_complete_janet_proposal(
        complete_basis,
        &ordering_2d,
        &context_2d,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(proposal.census().attempted_prolongations(), 1);
    assert_eq!(proposal.census().zero_remainders(), 1);
    assert_eq!(proposal.census().inserted_remainders(), 0);
    assert_eq!(proposal.census().autoreduction().passes(), 1);
    assert_eq!(proposal.epoch().epoch().revision(), 0);
}

#[test]
fn normal_form_localization_and_fixed_point_caps_are_tight() {
    let defaults = InvolutiveLimits::default();
    let context_1d = context(1);
    let ordering = active_ordering(1, defaults);
    let (baseline_basis, baseline_subject) =
        two_step_nonconstant_fixture(&context_1d, &ordering, defaults);
    let baseline = try_janet_normal_form(
        baseline_subject,
        &baseline_basis,
        &ordering,
        &context_1d,
        defaults,
    )
    .unwrap();
    assert_eq!(baseline.steps().len(), 2);

    let run = |limits| {
        let (basis, subject) = two_step_nonconstant_fixture(&context_1d, &ordering, defaults);
        try_janet_normal_form(subject, &basis, &ordering, &context_1d, limits)
    };
    let step_cap = InvolutiveLimits {
        max_normal_form_steps: baseline.steps().len() - 1,
        ..defaults
    };
    assert_eq!(
        run(step_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet normal-form steps",
            requested: baseline.steps().len(),
            limit: baseline.steps().len() - 1,
        })
    );
    let visit_cap = InvolutiveLimits {
        max_normal_form_divisor_visits: baseline.divisor_visits() - 1,
        ..defaults
    };
    assert_eq!(
        run(visit_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet normal-form divisor visits",
            requested: baseline.divisor_visits(),
            limit: baseline.divisor_visits() - 1,
        })
    );
    let trace_cap = InvolutiveLimits {
        max_normal_form_trace_bytes: baseline.trace_bytes() - 1,
        ..defaults
    };
    assert_eq!(
        run(trace_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet normal-form trace bytes",
            requested: baseline.trace_bytes(),
            limit: baseline.trace_bytes() - 1,
        })
    );
    let provenance_cap = InvolutiveLimits {
        max_provenance_terms: 2,
        ..defaults
    };
    assert_eq!(
        run(provenance_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Ore provenance terms",
            requested: 3,
            limit: 2,
        })
    );
    let guard_count_cap = InvolutiveLimits {
        max_localization_guards: 1,
        ..defaults
    };
    assert_eq!(
        run(guard_count_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Ore localization guards",
            requested: 2,
            limit: 1,
        })
    );
    let n = context_1d.index(0).unwrap();
    let n_plus_one = context_1d.add(&n, &context_1d.one()).unwrap();
    let translated_pivot_guard = context_1d
        .numerator_condition_with_limits(&n_plus_one, defaults.indexed_algebra.exact_algebra)
        .unwrap();
    let first_guard = baseline
        .remainder()
        .required_nonzero_guards()
        .iter()
        .find(|guard| guard.as_ref() == &translated_pivot_guard)
        .expect("the normalized divisor guard is translated with its Ore action");
    let guard_terms = first_guard.raw().coefficients.len();
    let guard_term_cap = InvolutiveLimits {
        max_localization_guard_terms: guard_terms - 1,
        ..defaults
    };
    assert_eq!(
        run(guard_term_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Ore localization guard terms",
            requested: guard_terms,
            limit: guard_terms - 1,
        })
    );
    let guard_cells = first_guard.raw().exponents.len();
    let guard_cell_cap = InvolutiveLimits {
        max_localization_guard_exponent_cells: guard_cells - 1,
        ..defaults
    };
    assert_eq!(
        run(guard_cell_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Ore localization guard exponent cells",
            requested: guard_cells,
            limit: guard_cells - 1,
        })
    );
    let mut guard_bytes = std::mem::size_of::<crate::algebra::IndexedPolynomial>()
        + std::mem::size_of::<std::sync::Arc<crate::algebra::IndexedPolynomial>>()
        + guard_terms * std::mem::size_of::<symbolica::prelude::Integer>()
        + guard_cells * std::mem::size_of::<u16>();
    for coefficient in &first_guard.raw().coefficients {
        if let symbolica::prelude::Integer::Large(value) = coefficient {
            guard_bytes += usize::try_from(value.significant_bits())
                .unwrap()
                .div_ceil(8);
        }
    }
    let guard_byte_cap = InvolutiveLimits {
        max_localization_guard_retained_bytes: guard_bytes - 1,
        ..defaults
    };
    assert_eq!(
        run(guard_byte_cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Ore localization guard retained bytes",
            requested: guard_bytes,
            limit: guard_bytes - 1,
        })
    );

    let context_2d = context(2);
    let ordering_2d = active_ordering(2, defaults);
    let complete_basis = epoch(&[&[1, 0], &[0, 1]], &context_2d, &ordering_2d, defaults);
    let autoreduction_cap = InvolutiveLimits {
        max_autoreduction_passes: 0,
        ..defaults
    };
    assert_eq!(
        try_autoreduce_epoch(
            complete_basis,
            &ordering_2d,
            &context_2d,
            autoreduction_cap,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet autoreduction passes",
            requested: 1,
            limit: 0,
        })
    );
    let complete_basis = epoch(&[&[1, 0], &[0, 1]], &context_2d, &ordering_2d, defaults);
    let completion_cap = InvolutiveLimits {
        max_completion_iterations: 0,
        ..defaults
    };
    assert_eq!(
        try_complete_janet_proposal(
            complete_basis,
            &ordering_2d,
            &context_2d,
            completion_cap,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet completion iterations",
            requested: 1,
            limit: 0,
        })
    );
}

#[test]
fn translated_janet_cancellation_uses_the_unit_pivot_and_carries_its_chart_guard() {
    let limits = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, limits);
    let e = shift(&[1], limits);
    let e2 = shift(&[2], limits);
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let rational_pivot = context.div(&n, &n_plus_one).unwrap();
    let divisor = OreConsequence::try_from_source(
        0,
        OreRow::try_new(&ordering, [(e.clone(), rational_pivot)], &context, limits).unwrap(),
        &ordering,
        &context,
        limits,
    )
    .unwrap()
    .try_require_nonzero_guard(
        context
            .numerator_condition_with_limits(&n_plus_one, limits.indexed_algebra.exact_algebra)
            .unwrap(),
        &context,
        limits,
    )
    .unwrap()
    .0;
    let basis = JanetBasisEpoch::try_initial(
        [divisor],
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(
        basis.elements()[0].consequence().row().coefficient(&e),
        Some(&context.one())
    );

    let subject = OreConsequence::try_from_source(
        1,
        OreRow::try_new(&ordering, [(e2, context.one())], &context, limits).unwrap(),
        &ordering,
        &context,
        limits,
    )
    .unwrap();
    let normal_form = try_janet_normal_form(subject, &basis, &ordering, &context, limits).unwrap();

    assert!(normal_form.is_zero());
    assert_eq!(normal_form.steps().len(), 1);
    assert!(normal_form.steps()[0].required_nonzero().is_none());
    let expected_shifted_n = context.add(&n, &context.one()).unwrap();
    let expected_shifted_n_plus_one = context.add(&n, &context.integer(2)).unwrap();
    let expected_guards = [expected_shifted_n, expected_shifted_n_plus_one].map(|value| {
        context
            .numerator_condition_with_limits(&value, limits.indexed_algebra.exact_algebra)
            .unwrap()
    });
    assert_eq!(normal_form.remainder().required_nonzero_guards().len(), 2);
    for expected in &expected_guards {
        assert!(
            normal_form
                .remainder()
                .required_nonzero_guards()
                .iter()
                .any(|actual| actual.as_ref() == expected)
        );
    }
    assert_eq!(normal_form.steps()[0].operator_shift(), &e);
    assert_eq!(normal_form.steps()[0].target_shift(), &shift(&[2], limits));
}

#[test]
fn nonmonic_completion_is_deterministic_across_input_permutations() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let n0 = context.index(0).unwrap();
    let n1_plus_one = context
        .add(&context.index(1).unwrap(), &context.one())
        .unwrap();
    let make = |source_ordinal, powers: &[u64], coefficient| {
        OreConsequence::try_from_source(
            source_ordinal,
            OreRow::try_new(
                &ordering,
                [(shift(powers, limits), coefficient)],
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
    let run = |reverse| {
        let first = make(0, &[2, 0], n0.clone());
        let second = make(1, &[0, 3], n1_plus_one.clone());
        let rows = if reverse {
            vec![second, first]
        } else {
            vec![first, second]
        };
        try_complete_janet_proposal_from_consequences(
            rows,
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        )
        .unwrap()
    };
    let forward = run(false);
    let reverse = run(true);

    assert_eq!(forward.census(), reverse.census());
    assert_eq!(forward.work_census(), reverse.work_census());
    assert_eq!(
        forward.localization_witness(),
        reverse.localization_witness()
    );
    assert_eq!(
        forward
            .epoch()
            .elements()
            .iter()
            .map(JanetBasisElement::consequence)
            .collect::<Vec<_>>(),
        reverse
            .epoch()
            .elements()
            .iter()
            .map(JanetBasisElement::consequence)
            .collect::<Vec<_>>()
    );
    assert!(forward.epoch().elements().iter().all(|element| {
        element
            .consequence()
            .row()
            .coefficient(element.leading_shift())
            == Some(&context.one())
    }));
}

#[test]
fn zero_prolongation_localization_survives_queue_exhaustion() {
    let defaults = InvolutiveLimits::default();
    let limits = InvolutiveLimits {
        max_localization_guards: 1,
        ..defaults
    };
    let context = context(2);
    let ordering = active_ordering(2, defaults);
    let n1 = context.index(0).unwrap();
    let n1_e1 = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [(shift(&[1, 0], defaults), n1.clone())],
            &context,
            defaults,
        )
        .unwrap(),
        &ordering,
        &context,
        defaults,
    )
    .unwrap();
    let e2 = monomial_consequence(1, &[0, 1], &ordering, &context, defaults);
    let initial = JanetBasisEpoch::try_initial(
        [n1_e1, e2],
        &ordering,
        &context,
        defaults,
        CompletionGeometryLimits::default(),
    )
    .unwrap();

    // The sole nonmultiplicative obligation is E1(E2). It reduces to zero by
    // (1/n1) E2(n1 E1), so queue exhaustion is valid only on the n1 != 0 chart.
    let proposal = try_complete_janet_proposal(
        initial,
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(proposal.census().zero_remainders(), 1);
    assert_eq!(proposal.census().inserted_remainders(), 0);
    assert_eq!(proposal.localization_witness().census().count(), 1);
    assert_eq!(proposal.localization_witness().guards().len(), 1);
    let expected = context
        .numerator_condition_with_limits(&n1, defaults.indexed_algebra.exact_algebra)
        .unwrap();
    assert_eq!(
        proposal.localization_witness().guards()[0].as_ref(),
        &expected
    );
}

#[test]
fn numerator_and_denominator_guard_paths_deduplicate_under_an_exact_one_guard_cap() {
    let defaults = InvolutiveLimits::default();
    let limits = InvolutiveLimits {
        max_localization_guards: 1,
        ..defaults
    };
    let context = context(1);
    let ordering = active_ordering(1, defaults);
    let n = context.index(0).unwrap();
    let divisor = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [(shift(&[1], defaults), n.clone())],
            &context,
            defaults,
        )
        .unwrap(),
        &ordering,
        &context,
        defaults,
    )
    .unwrap();
    let basis = JanetBasisEpoch::try_initial(
        [divisor],
        &ordering,
        &context,
        defaults,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let subject = monomial_consequence(1, &[1], &ordering, &context, defaults);

    let normal_form = try_janet_normal_form(subject, &basis, &ordering, &context, limits).unwrap();
    assert!(normal_form.is_zero());
    assert_eq!(normal_form.remainder().required_nonzero_guards().len(), 1);
    assert_eq!(
        normal_form
            .remainder()
            .localization_witness()
            .census()
            .count(),
        1
    );
}

#[test]
fn completion_work_limits_are_cumulative_across_obligations_and_autoreductions() {
    let defaults = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, defaults);

    // This already Janet-complete monomial set has two zero obligations, each
    // requiring one normal-form step. A per-call ledger would incorrectly pass.
    let complete = epoch(&[&[2, 0], &[1, 2], &[0, 3]], &context, &ordering, defaults);
    let step_cap = InvolutiveLimits {
        max_normal_form_steps: 1,
        ..defaults
    };
    assert_eq!(
        try_complete_janet_proposal(
            complete,
            &ordering,
            &context,
            step_cap,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet normal-form steps",
            requested: 2,
            limit: 1,
        })
    );

    // Initial autoreduction consumes one pass. The nonzero E1(E2^3)
    // prolongation is inserted, and its successor must consume a second pass.
    let incomplete = epoch(&[&[2, 0], &[0, 3]], &context, &ordering, defaults);
    let pass_cap = InvolutiveLimits {
        max_autoreduction_passes: 1,
        ..defaults
    };
    assert_eq!(
        try_complete_janet_proposal(
            incomplete,
            &ordering,
            &context,
            pass_cap,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet autoreduction passes",
            requested: 2,
            limit: 1,
        })
    );

    let complete = epoch(&[&[2, 0], &[1, 2], &[0, 3]], &context, &ordering, defaults);
    let proposal = try_complete_janet_proposal(
        complete,
        &ordering,
        &context,
        defaults,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(proposal.work_census().normal_form_steps(), 2);
    assert_eq!(proposal.work_census().completion_iterations(), 2);
    assert_eq!(proposal.work_census().autoreduction_passes(), 1);
    assert!(proposal.work_census().normal_form_divisor_visits() >= 2);
    assert!(proposal.work_census().exact_coefficient_operations() > 0);
}
