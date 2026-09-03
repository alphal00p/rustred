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
    let reducible_ordinal = initial
        .elements()
        .iter()
        .position(|element| element.leading_shift() == &e2_squared)
        .unwrap();
    let divisor_ordinal = initial
        .elements()
        .iter()
        .position(|element| element.leading_shift() == &e1)
        .unwrap();
    let original_reducible =
        std::sync::Arc::clone(initial.elements()[reducible_ordinal].consequence_handle());
    let original_divisor =
        std::sync::Arc::clone(initial.elements()[divisor_ordinal].consequence_handle());

    // Differential oracle for the former eager path: identity-copy the row,
    // then run the ordinary owned normal form with the same self exclusion.
    let mut oracle_work = super::super::limits::InvolutiveWorkBudget::default();
    let eager_copy = original_reducible
        .try_copy_sealed(&ordering_2d, &context_2d, limits, &mut oracle_work)
        .unwrap();
    let eager_normal_form = super::super::normal_form::try_janet_normal_form_excluding(
        eager_copy,
        &initial,
        Some(reducible_ordinal),
        &ordering_2d,
        &context_2d,
        limits,
        &mut oracle_work,
    )
    .unwrap();
    let mut borrowed_work = super::super::limits::InvolutiveWorkBudget::default();
    let borrowed_normal_form =
        super::super::normal_form::try_janet_autoreduction_normal_form_excluding(
            &original_reducible,
            &initial,
            reducible_ordinal,
            &ordering_2d,
            &context_2d,
            limits,
            &mut borrowed_work,
        )
        .unwrap();
    let super::super::normal_form::JanetAutoreductionNormalForm::Materialized(borrowed_normal_form) =
        borrowed_normal_form
    else {
        panic!("the excluded lower-term divisor must force materialization");
    };
    assert_eq!(borrowed_normal_form, eager_normal_form);
    assert_eq!(
        borrowed_work.census().normal_form_divisor_visits(),
        oracle_work.census().normal_form_divisor_visits(),
    );
    assert_eq!(
        borrowed_work.census().divisor_index_query_operations(),
        oracle_work.census().divisor_index_query_operations(),
    );
    assert_eq!(
        borrowed_work.census().exact_coefficient_operations(),
        oracle_work.census().exact_coefficient_operations(),
    );
    let eager_remainder = eager_normal_form.into_remainder();
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
    assert_eq!(reduced.consequence(), &eager_remainder);
    assert!(!std::sync::Arc::ptr_eq(
        reduced.consequence_handle(),
        &original_reducible,
    ));
    let retained_divisor = autoreduced
        .epoch()
        .elements()
        .iter()
        .find(|element| element.leading_shift() == &e1)
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(
        retained_divisor.consequence_handle(),
        &original_divisor,
    ));
    assert_eq!(autoreduced.census().normal_form_steps(), 1);
    assert_eq!(autoreduced.census().dropped_rows(), 0);
    assert_eq!(autoreduced.census().passes(), 2);
    assert_eq!(autoreduced.census().materialized_rows(), 1);
    assert_eq!(autoreduced.census().shared_rows(), 3);
    assert_eq!(
        autoreduced.work_census().autoreduction_materialized_rows(),
        1
    );
    assert_eq!(autoreduced.work_census().autoreduction_shared_rows(), 3);
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
fn cow_autoreduction_hands_off_a_nonconstant_multistep_selection_exactly() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let zero = shift(&[0, 0], limits);
    let e1 = shift(&[1, 0], limits);
    let e1_squared = shift(&[2, 0], limits);
    let irreducible_leader = shift(&[0, 3], limits);
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();

    // In this Janet epoch E1 is multiplicative for the divisor.  The
    // irreducible degree-three leader keeps the subject in the basis while
    // its E1^2 and E1 lower terms reproduce the two translated cancellations
    // from `two_step_nonconstant_fixture`.
    let divisor = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [(e1.clone(), n.clone()), (zero.clone(), context.one())],
            &context,
            limits,
        )
        .unwrap(),
        &ordering,
        &context,
        limits,
    )
    .unwrap();
    let subject = OreConsequence::try_from_source(
        1,
        OreRow::try_new(
            &ordering,
            [
                (irreducible_leader.clone(), context.one()),
                (e1_squared.clone(), n_plus_one.clone()),
                (e1.clone(), n_plus_one),
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
    let basis = JanetBasisEpoch::try_initial(
        [divisor, subject],
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let subject_ordinal = basis
        .elements()
        .iter()
        .position(|element| element.leading_shift() == &irreducible_leader)
        .unwrap();
    let subject = std::sync::Arc::clone(basis.elements()[subject_ordinal].consequence_handle());

    // This is the former eager ownership boundary, retained as a test oracle;
    // the production COW path must produce the identical complete normal form
    // while handing its borrowed first selection to the owned reducer once.
    let mut eager_work = super::super::limits::InvolutiveWorkBudget::default();
    let eager_copy = subject
        .try_copy_sealed(&ordering, &context, limits, &mut eager_work)
        .unwrap();
    let eager = super::super::normal_form::try_janet_normal_form_excluding(
        eager_copy,
        &basis,
        Some(subject_ordinal),
        &ordering,
        &context,
        limits,
        &mut eager_work,
    )
    .unwrap();
    let mut cow_work = super::super::limits::InvolutiveWorkBudget::default();
    let cow = super::super::normal_form::try_janet_autoreduction_normal_form_excluding(
        &subject,
        &basis,
        subject_ordinal,
        &ordering,
        &context,
        limits,
        &mut cow_work,
    )
    .unwrap();
    let super::super::normal_form::JanetAutoreductionNormalForm::Materialized(cow) = cow else {
        panic!("two lower-term cancellations must materialize the subject");
    };

    assert_eq!(cow, eager);
    assert_eq!(cow.steps().len(), 2);
    assert_eq!(cow.steps()[0].target_shift(), &e1_squared);
    assert_eq!(cow.steps()[1].target_shift(), &e1);
    assert_eq!(cow.remainder().row().terms().len(), 1);
    assert_eq!(
        cow.remainder().row().terms()[0].shift(),
        &irreducible_leader
    );
    assert_eq!(cow.remainder().required_nonzero_guards().len(), 2);
    let provenance = cow.remainder().provenance().terms();
    assert_eq!(provenance.len(), 3);
    assert_eq!(provenance[0].source_ordinal(), 0);
    assert_eq!(provenance[0].left_shift(), &zero);
    assert_eq!(provenance[0].left_coefficient(), &context.integer(-1));
    assert_eq!(provenance[1].source_ordinal(), 0);
    assert_eq!(provenance[1].left_shift(), &e1);
    assert_eq!(provenance[1].left_coefficient(), &context.integer(-1));
    assert_eq!(provenance[2].source_ordinal(), 1);
    assert_eq!(provenance[2].left_shift(), &zero);
    assert_eq!(provenance[2].left_coefficient(), &context.one());
    assert_eq!(
        cow_work.census().normal_form_divisor_visits(),
        eager_work.census().normal_form_divisor_visits(),
    );
    assert_eq!(
        cow_work.census().divisor_index_query_operations(),
        eager_work.census().divisor_index_query_operations(),
    );
    assert_eq!(
        cow_work.census().exact_coefficient_operations(),
        eager_work.census().exact_coefficient_operations(),
    );
    assert_eq!(
        cow_work.census().normal_form_steps(),
        eager_work.census().normal_form_steps(),
    );
    assert_eq!(
        cow_work.census().normal_form_trace_bytes(),
        eager_work.census().normal_form_trace_bytes(),
    );
}

#[test]
fn stable_autoreduction_shares_every_sealed_row_without_exact_payload_work() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let initial = epoch(&[&[1, 0], &[0, 1]], &context, &ordering, limits);
    let handles = initial
        .elements()
        .iter()
        .map(|element| {
            (
                element.leading_shift().clone(),
                std::sync::Arc::clone(element.consequence_handle()),
            )
        })
        .collect::<Vec<_>>();

    let autoreduced = try_autoreduce_epoch(
        initial,
        &ordering,
        &context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();

    assert_eq!(autoreduced.epoch().epoch().revision(), 0);
    assert_eq!(autoreduced.census().passes(), 1);
    assert_eq!(autoreduced.census().shared_rows(), handles.len());
    assert_eq!(autoreduced.census().materialized_rows(), 0);
    assert_eq!(
        autoreduced.work_census().autoreduction_shared_rows(),
        handles.len()
    );
    assert_eq!(
        autoreduced.work_census().autoreduction_materialized_rows(),
        0
    );
    assert_eq!(autoreduced.work_census().exact_coefficient_operations(), 0);
    for (leading_shift, handle) in handles {
        let retained = autoreduced
            .epoch()
            .elements()
            .iter()
            .find(|element| element.leading_shift() == &leading_shift)
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            retained.consequence_handle(),
            &handle,
        ));
    }
}

#[test]
fn division_only_successor_seals_to_the_public_full_epoch() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let initial = epoch(&[&[2, 0], &[0, 2]], &context, &ordering, limits);
    let replacements = initial
        .elements()
        .iter()
        .map(|element| std::sync::Arc::clone(element.consequence_handle()))
        .collect::<Vec<_>>();

    let mut full_work = super::super::limits::InvolutiveWorkBudget::default();
    let full = initial
        .try_replacement_successor(
            replacements.clone(),
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
            &mut full_work,
        )
        .unwrap();

    let mut deferred_work = super::super::limits::InvolutiveWorkBudget::default();
    let division = initial
        .try_replacement_division_successor(
            replacements,
            &ordering,
            &context,
            limits,
            &mut deferred_work,
        )
        .unwrap();

    // Building the division layer performs exactly the same monic, ranking,
    // mask, and indexed-divisor work as the eager full successor.  Queue and
    // complement construction have no work-ledger side effects and happen
    // only at this explicit seal.
    assert_eq!(deferred_work.census(), full_work.census());
    let deferred = division
        .try_seal(&ordering, limits, CompletionGeometryLimits::default())
        .unwrap();
    assert_eq!(deferred, full);
}

#[test]
fn hidden_division_revisions_preserve_the_last_sealed_predecessor_and_staleness() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let initial = epoch(&[&[2, 0], &[0, 2]], &context, &ordering, limits);
    let initial_id = initial.epoch().clone();
    let stale_prolongation = initial.prolongations()[0].clone();
    let mut stale_scratch = initial.try_divisor_scratch(limits).unwrap();

    let first_replacements = initial
        .elements()
        .iter()
        .map(|element| std::sync::Arc::clone(element.consequence_handle()))
        .collect::<Vec<_>>();
    let mut work = super::super::limits::InvolutiveWorkBudget::default();
    let first_hidden = initial
        .try_replacement_division_successor(
            first_replacements,
            &ordering,
            &context,
            limits,
            &mut work,
        )
        .unwrap();
    assert_eq!(first_hidden.epoch().revision(), 1);

    let second_replacements = first_hidden
        .elements()
        .iter()
        .map(|element| std::sync::Arc::clone(element.consequence_handle()))
        .collect::<Vec<_>>();
    let second_hidden = first_hidden
        .try_replacement_successor(second_replacements, &ordering, &context, limits, &mut work)
        .unwrap();
    let sealed = second_hidden
        .try_seal(&ordering, limits, CompletionGeometryLimits::default())
        .unwrap();

    assert_eq!(sealed.epoch().revision(), 2);
    assert_eq!(sealed.predecessor(), Some(&initial_id));
    assert_eq!(
        sealed.require_current(&stale_prolongation),
        Err(InvolutiveError::StaleEpoch {
            expected: sealed.epoch().clone(),
            actual: initial_id.clone(),
        })
    );
    assert!(
        sealed
            .prolongations()
            .iter()
            .all(|prolongation| sealed.require_current(prolongation).is_ok())
    );

    let mut query_work = super::super::limits::InvolutiveWorkBudget::default();
    assert_eq!(
        sealed.try_janet_divisor_with_scratch(
            &shift(&[2, 0], limits),
            None,
            &mut stale_scratch,
            limits,
            &mut query_work,
        ),
        Err(InvolutiveError::StaleEpoch {
            expected: sealed.epoch().clone(),
            actual: initial_id,
        })
    );
}

#[test]
fn autoreduction_sharing_and_materialization_caps_are_cumulative_and_precede_copying() {
    let defaults = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, defaults);

    let make_stable = || epoch(&[&[1, 0], &[0, 1]], &context, &ordering, defaults);
    let stable_baseline = try_autoreduce_epoch(
        make_stable(),
        &ordering,
        &context,
        defaults,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let shared_rows = stable_baseline.census().shared_rows();
    assert_eq!(shared_rows, 2);
    let shared_one_below = InvolutiveLimits {
        max_autoreduction_shared_rows: shared_rows - 1,
        ..defaults
    };
    super::super::diagnostics::begin();
    assert_eq!(
        try_autoreduce_epoch(
            make_stable(),
            &ordering,
            &context,
            shared_one_below,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet autoreduction shared rows",
            requested: shared_rows,
            limit: shared_rows - 1,
        })
    );
    let shared_stop = super::super::diagnostics::take().unwrap();
    assert_eq!(
        shared_stop
            .work_at_last_checkpoint
            .autoreduction_shared_rows(),
        shared_rows - 1,
    );
    assert_eq!(
        shared_stop
            .work_at_last_checkpoint
            .exact_coefficient_operations(),
        0,
    );

    let make_reducible = || {
        let e1 = shift(&[1, 0], defaults);
        let e2_squared = shift(&[0, 2], defaults);
        let reducible = OreConsequence::try_from_source(
            0,
            OreRow::try_new(
                &ordering,
                [(e2_squared, context.one()), (e1.clone(), context.one())],
                &context,
                defaults,
            )
            .unwrap(),
            &ordering,
            &context,
            defaults,
        )
        .unwrap();
        let divisor = monomial_consequence(1, &[1, 0], &ordering, &context, defaults);
        JanetBasisEpoch::try_initial(
            [reducible, divisor],
            &ordering,
            &context,
            defaults,
            CompletionGeometryLimits::default(),
        )
        .unwrap()
    };
    let materialized_baseline = try_autoreduce_epoch(
        make_reducible(),
        &ordering,
        &context,
        defaults,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let materialized_rows = materialized_baseline.census().materialized_rows();
    assert_eq!(materialized_rows, 1);
    let materialized_one_below = InvolutiveLimits {
        max_autoreduction_materialized_rows: materialized_rows - 1,
        ..defaults
    };
    super::super::diagnostics::begin();
    assert_eq!(
        try_autoreduce_epoch(
            make_reducible(),
            &ordering,
            &context,
            materialized_one_below,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet autoreduction materialized rows",
            requested: materialized_rows,
            limit: materialized_rows - 1,
        })
    );
    let materialized_stop = super::super::diagnostics::take().unwrap();
    assert_eq!(
        materialized_stop
            .work_at_last_checkpoint
            .autoreduction_materialized_rows(),
        materialized_rows - 1,
    );
    assert_eq!(
        materialized_stop
            .work_at_last_checkpoint
            .exact_coefficient_operations(),
        0,
    );
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
