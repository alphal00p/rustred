use super::super::super::CompletionGeometryLimits;
use super::super::diagnostics::{
    self, JanetDiagnosticCoefficientComponentKind, JanetDiagnosticCoefficientSite,
};
use super::super::limits::InvolutiveWorkBudget;
use super::super::*;
use super::support::*;

fn repeated_denominator_source(
    count: usize,
    context: &crate::algebra::IndexedCoefficientContext,
    ordering: &OreOrderingAdapter,
    limits: InvolutiveLimits,
) -> OreConsequence {
    let denominator = context
        .add(&context.index(0).unwrap(), &context.integer(2))
        .unwrap();
    let coefficient = context.div(&context.one(), &denominator).unwrap();
    let terms = (0..count)
        .map(|ordinal| {
            (
                shift(&[u64::try_from(ordinal).unwrap()], limits),
                coefficient.clone(),
            )
        })
        .collect::<Vec<_>>();
    let row = OreRow::try_new(ordering, terms, context, limits).unwrap();
    OreConsequence::try_from_source(0, row, ordering, context, limits).unwrap()
}

#[test]
fn cheap_payload_diagnostic_splits_components_without_exact_denominator_work() {
    let limits = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, limits);
    let n = context.index(0).unwrap();
    let denominator = context.add(&n, &context.integer(2)).unwrap();
    let first = context
        .div(&context.add(&n, &context.one()).unwrap(), &denominator)
        .unwrap();
    let second = context.div(&context.integer(2), &denominator).unwrap();
    let row = OreRow::try_new(
        &ordering,
        [(shift(&[0], limits), first), (shift(&[1], limits), second)],
        &context,
        limits,
    )
    .unwrap();
    let source = OreConsequence::try_from_source(0, row, &ordering, &context, limits).unwrap();
    let mut inactive_work = InvolutiveWorkBudget::default();
    let expected = source
        .try_copy_sealed(&ordering, &context, limits, &mut inactive_work)
        .unwrap();

    diagnostics::begin();
    let mut work = InvolutiveWorkBudget::default();
    let actual = source
        .try_copy_sealed(&ordering, &context, limits, &mut work)
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(work.census(), inactive_work.census());
    let checkpoint = diagnostics::take().unwrap();
    assert_eq!(checkpoint.coefficient_payload_attempts, 1);
    let attempt = checkpoint.last_coefficient_payload.unwrap();
    assert_eq!(
        attempt.site,
        JanetDiagnosticCoefficientSite::AutoreductionMaterialization
    );
    assert_eq!(attempt.payload.row.coefficients, 2);
    assert_eq!(attempt.payload.provenance.coefficients, 1);
    assert_eq!(attempt.payload.row.numerator.terms, 3);
    assert_eq!(attempt.payload.row.denominator.terms, 4);
    assert_eq!(attempt.payload.provenance.numerator.terms, 1);
    assert_eq!(attempt.payload.provenance.denominator.terms, 1);
    assert_eq!(attempt.payload.row.numerator.exponent_cells, 3);
    assert_eq!(attempt.payload.row.denominator.exponent_cells, 4);
    assert_eq!(
        attempt.payload.max_single_coefficient.unwrap().component,
        JanetDiagnosticCoefficientComponentKind::Row
    );
    assert_eq!(attempt.payload.max_single_coefficient.unwrap().ordinal, 0);

    let denominators = attempt.payload.denominators;
    assert_eq!(denominators.instances, 3);
    assert_eq!(denominators.unit_instances, 1);
    assert_eq!(denominators.nonunit_instances, 2);
    assert!(!denominators.exact_tracking_attempted);
    assert_eq!(denominators.exact_tracked_instances, 0);
    assert_eq!(denominators.exact_distinct_representatives, 0);
    assert_eq!(denominators.exact_confirmed_shared_instances, 0);
    assert_eq!(denominators.exact_oversized_or_budget_skips, 0);
    assert!(!denominators.exact_tracking_truncated);

    let quadrant_terms = attempt
        .payload
        .row
        .numerator
        .terms
        .saturating_add(attempt.payload.row.denominator.terms)
        .saturating_add(attempt.payload.provenance.numerator.terms)
        .saturating_add(attempt.payload.provenance.denominator.terms);
    let quadrant_cells = attempt
        .payload
        .row
        .numerator
        .exponent_cells
        .saturating_add(attempt.payload.row.denominator.exponent_cells)
        .saturating_add(attempt.payload.provenance.numerator.exponent_cells)
        .saturating_add(attempt.payload.provenance.denominator.exponent_cells);
    let quadrant_bytes = attempt
        .payload
        .row
        .numerator
        .retained_bytes
        .saturating_add(attempt.payload.row.denominator.retained_bytes)
        .saturating_add(attempt.payload.provenance.numerator.retained_bytes)
        .saturating_add(attempt.payload.provenance.denominator.retained_bytes)
        .saturating_add(attempt.payload.row.coefficient_wrapper_bytes)
        .saturating_add(attempt.payload.provenance.coefficient_wrapper_bytes);
    assert_eq!(attempt.payload.total.terms, quadrant_terms);
    assert_eq!(attempt.payload.total.exponent_cells, quadrant_cells);
    assert_eq!(attempt.payload.total.retained_bytes, quadrant_bytes);
}

fn two_source_axpy(
    limits: InvolutiveLimits,
    construction_limits: InvolutiveLimits,
) -> Result<OreConsequence, InvolutiveError> {
    let context = context(1);
    let ordering = active_ordering(1, construction_limits);
    let left = monomial_consequence(0, &[0], &ordering, &context, construction_limits);
    let right = monomial_consequence(1, &[1], &ordering, &context, construction_limits);
    left.try_left_axpy(
        &context.one(),
        &shift(&[0], construction_limits),
        &right,
        &ordering,
        &context,
        limits,
    )
}

#[test]
fn failed_cell_limit_records_the_complete_payload_without_changing_error_order() {
    let defaults = InvolutiveLimits::default();
    let expected = two_source_axpy(defaults, defaults).unwrap();
    let expected = expected.coefficient_census();
    let limits = InvolutiveLimits {
        max_consequence_coefficient_terms: expected.terms(),
        max_consequence_coefficient_exponent_cells: expected.exponent_cells() - 1,
        max_consequence_coefficient_retained_bytes: expected.retained_bytes() - 1,
        ..defaults
    };

    let expected_error = InvolutiveError::ResourceLimit {
        resource: "Ore consequence coefficient exponent cells",
        requested: expected.exponent_cells(),
        limit: expected.exponent_cells() - 1,
    };
    assert_eq!(
        two_source_axpy(limits, defaults).unwrap_err(),
        expected_error
    );

    diagnostics::begin();
    let error = two_source_axpy(limits, defaults).unwrap_err();
    assert_eq!(error, expected_error);
    let checkpoint = diagnostics::take().unwrap();
    let attempt = checkpoint.last_coefficient_payload.unwrap();
    assert_eq!(attempt.site, JanetDiagnosticCoefficientSite::DirectAxpy);
    assert_eq!(
        attempt.payload.total.exponent_cells,
        expected.exponent_cells()
    );
    assert!(!attempt.exceeds.terms);
    assert!(attempt.exceeds.exponent_cells);
    assert!(attempt.exceeds.retained_bytes);
}

#[test]
fn exact_denominator_tracking_has_a_hard_instance_bound() {
    let defaults = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, defaults);
    let source = repeated_denominator_source(257, &context, &ordering, defaults);
    let expected = source.coefficient_census();
    let limits = InvolutiveLimits {
        max_consequence_coefficient_terms: expected.terms() - 1,
        ..defaults
    };
    let mut inactive_work = InvolutiveWorkBudget::default();
    let expected_error = source
        .try_copy_sealed(&ordering, &context, limits, &mut inactive_work)
        .unwrap_err();

    diagnostics::begin();
    let mut work = InvolutiveWorkBudget::default();
    let error = source
        .try_copy_sealed(&ordering, &context, limits, &mut work)
        .unwrap_err();
    assert_eq!(error, expected_error);
    assert_eq!(work.census(), inactive_work.census());
    assert!(matches!(
        error,
        InvolutiveError::ResourceLimit {
            resource: "Ore consequence coefficient terms",
            ..
        }
    ));
    let checkpoint = diagnostics::take().unwrap();
    let denominators = checkpoint
        .last_coefficient_payload
        .unwrap()
        .payload
        .denominators;
    assert_eq!(denominators.instances, 258);
    assert_eq!(denominators.unit_instances, 1);
    assert_eq!(denominators.nonunit_instances, 257);
    assert!(denominators.exact_tracking_attempted);
    assert_eq!(denominators.exact_tracked_instances, 256);
    assert_eq!(denominators.exact_distinct_representatives, 1);
    assert_eq!(denominators.exact_confirmed_shared_instances, 255);
    assert_eq!(denominators.exact_oversized_or_budget_skips, 1);
    assert!(denominators.exact_tracking_truncated);
}

#[test]
fn exact_denominator_tracking_is_claimed_only_by_the_first_failing_payload() {
    let defaults = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, defaults);
    let larger = repeated_denominator_source(4, &context, &ordering, defaults);
    let smaller = repeated_denominator_source(2, &context, &ordering, defaults);

    diagnostics::begin();
    for source in [&larger, &smaller] {
        let census = source.coefficient_census();
        let limits = InvolutiveLimits {
            max_consequence_coefficient_terms: census.terms() - 1,
            ..defaults
        };
        let mut work = InvolutiveWorkBudget::default();
        assert!(
            source
                .try_copy_sealed(&ordering, &context, limits, &mut work)
                .is_err()
        );
    }
    let checkpoint = diagnostics::take().unwrap();
    assert_eq!(checkpoint.coefficient_payload_attempts, 2);
    let first = checkpoint.peak_coefficient_payload.unwrap();
    let second = checkpoint.last_coefficient_payload.unwrap();
    assert!(first.payload.denominators.exact_tracking_attempted);
    assert!(!second.payload.denominators.exact_tracking_attempted);
    assert!(first.payload.total.terms > second.payload.total.terms);
}

#[test]
fn real_initial_monic_prolongation_and_normal_form_seams_are_attributed() {
    let limits = InvolutiveLimits::default();

    let one_context = context(1);
    let one_ordering = active_ordering(1, limits);
    let initial_rows = vec![
        OreConsequence::try_from_source(
            0,
            OreRow::try_new(
                &one_ordering,
                [
                    (shift(&[2], limits), one_context.one()),
                    (shift(&[0], limits), one_context.one()),
                ],
                &one_context,
                limits,
            )
            .unwrap(),
            &one_ordering,
            &one_context,
            limits,
        )
        .unwrap(),
        OreConsequence::try_from_source(
            1,
            OreRow::try_new(
                &one_ordering,
                [
                    (shift(&[2], limits), one_context.one()),
                    (shift(&[1], limits), one_context.one()),
                ],
                &one_context,
                limits,
            )
            .unwrap(),
            &one_ordering,
            &one_context,
            limits,
        )
        .unwrap(),
    ];
    diagnostics::begin();
    try_preprocess_initial_basis(
        initial_rows,
        &one_ordering,
        &one_context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let checkpoint = diagnostics::take().unwrap();
    assert_eq!(
        checkpoint
            .coefficient_payload_attempts_by_site
            .initial_head_reduction,
        1
    );

    let nonmonic = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &one_ordering,
            [
                (shift(&[0], limits), one_context.one()),
                (shift(&[1], limits), one_context.integer(2)),
            ],
            &one_context,
            limits,
        )
        .unwrap(),
        &one_ordering,
        &one_context,
        limits,
    )
    .unwrap();
    diagnostics::begin();
    JanetBasisEpoch::try_initial(
        [nonmonic],
        &one_ordering,
        &one_context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let checkpoint = diagnostics::take().unwrap();
    assert_eq!(
        checkpoint
            .coefficient_payload_attempts_by_site
            .monic_normalization,
        1
    );

    let two_context = context(2);
    let two_ordering = active_ordering(2, limits);
    let prolongation_epoch = epoch(&[&[2, 0], &[0, 3]], &two_context, &two_ordering, limits);
    let prolongation = prolongation_epoch.prolongations()[0].clone();
    diagnostics::begin();
    prolongation_epoch
        .try_apply_prolongation(&prolongation, &two_ordering, &two_context, limits)
        .unwrap();
    let checkpoint = diagnostics::take().unwrap();
    assert_eq!(
        checkpoint.coefficient_payload_attempts_by_site.prolongation,
        1
    );

    let (normal_form_basis, subject) =
        two_step_nonconstant_fixture(&one_context, &one_ordering, limits);
    diagnostics::begin();
    try_janet_normal_form(
        subject,
        &normal_form_basis,
        &one_ordering,
        &one_context,
        limits,
    )
    .unwrap();
    let checkpoint = diagnostics::take().unwrap();
    assert_eq!(
        checkpoint
            .coefficient_payload_attempts_by_site
            .normal_form_cancellation,
        2
    );
}
