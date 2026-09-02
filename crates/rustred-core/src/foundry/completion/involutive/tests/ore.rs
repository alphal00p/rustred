use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, Mask, OrderingPolicy};

use super::super::super::CompletionGeometryLimits;
use super::super::*;
use super::support::*;

#[test]
fn ore_left_action_mutates_index_coefficients_with_sector_sign() {
    let limits = InvolutiveLimits::default();
    let context = context(1);
    let n = context.index(0).unwrap();
    let active = active_ordering(1, limits);
    let active_source_row = OreRow::try_new(
        &active,
        [(shift(&[0], limits), n.clone())],
        &context,
        limits,
    )
    .unwrap();
    let active_source =
        OreConsequence::try_from_source(7, active_source_row, &active, &context, limits).unwrap();
    let unit = shift(&[1], limits);
    let active_shifted = OreConsequence::try_zero(&active, &context, limits)
        .unwrap()
        .try_left_axpy(
            &context.one(),
            &unit,
            &active_source,
            &active,
            &context,
            limits,
        )
        .unwrap();
    let expected_active = context.add(&n, &context.one()).unwrap();
    assert_eq!(
        active_shifted.row().coefficient(&unit),
        Some(&expected_active)
    );
    assert_eq!(active_shifted.provenance().terms().len(), 1);
    assert_eq!(active_shifted.provenance().terms()[0].source_ordinal(), 7);
    assert_eq!(active_shifted.provenance().terms()[0].left_shift(), &unit);

    let inactive = OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new([false]).unwrap(),
        limits,
    )
    .unwrap();
    let inactive_source_row = OreRow::try_new(
        &inactive,
        [(shift(&[0], limits), n.clone())],
        &context,
        limits,
    )
    .unwrap();
    let inactive_source =
        OreConsequence::try_from_source(8, inactive_source_row, &inactive, &context, limits)
            .unwrap();
    let inactive_shifted = OreConsequence::try_zero(&inactive, &context, limits)
        .unwrap()
        .try_left_axpy(
            &context.one(),
            &unit,
            &inactive_source,
            &inactive,
            &context,
            limits,
        )
        .unwrap();
    let expected_inactive = context.sub(&n, &context.one()).unwrap();
    assert_eq!(
        inactive_shifted.row().coefficient(&unit),
        Some(&expected_inactive)
    );

    assert_eq!(
        OreConsequence::try_zero(&inactive, &context, limits)
            .unwrap()
            .try_left_axpy(
                &context.one(),
                &unit,
                &active_source,
                &inactive,
                &context,
                limits,
            ),
        Err(InvolutiveError::ForeignOreAction)
    );
}

#[test]
fn p1_p2_consequence_cancels_exactly_and_retains_sparse_source_witness() {
    let limits = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, limits);
    let zero = shift(&[0], limits);
    let e = shift(&[1], limits);
    let e2 = shift(&[2], limits);
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();

    // P1 = n E + 1 and P2 = (n + 1) E^2 + E = E P1.
    let p1 = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [(e.clone(), n), (zero.clone(), context.one())],
            &context,
            limits,
        )
        .unwrap(),
        &ordering,
        &context,
        limits,
    )
    .unwrap();
    let p2 = OreConsequence::try_from_source(
        1,
        OreRow::try_new(
            &ordering,
            [(e2, n_plus_one), (e.clone(), context.one())],
            &context,
            limits,
        )
        .unwrap(),
        &ordering,
        &context,
        limits,
    )
    .unwrap();
    let syzygy = p2
        .try_left_axpy(&context.integer(-1), &e, &p1, &ordering, &context, limits)
        .unwrap();

    assert!(syzygy.is_zero());
    assert_eq!(syzygy.provenance().terms().len(), 2);
    let p1_witness = &syzygy.provenance().terms()[0];
    assert_eq!(p1_witness.source_ordinal(), 0);
    assert_eq!(p1_witness.left_shift(), &e);
    assert_eq!(p1_witness.left_coefficient(), &context.integer(-1));
    let p2_witness = &syzygy.provenance().terms()[1];
    assert_eq!(p2_witness.source_ordinal(), 1);
    assert_eq!(p2_witness.left_shift(), &zero);
    assert_eq!(p2_witness.left_coefficient(), &context.one());
}

#[test]
fn guard_free_axpy_accepts_an_exact_zero_localization_budget() {
    let defaults = InvolutiveLimits::default();
    let limits = InvolutiveLimits {
        max_localization_guards: 0,
        ..defaults
    };
    let context = context(1);
    let ordering = active_ordering(1, limits);
    let source = monomial_consequence(0, &[0], &ordering, &context, limits);
    let shifted = OreConsequence::try_zero(&ordering, &context, limits)
        .unwrap()
        .try_left_axpy(
            &context.one(),
            &shift(&[1], limits),
            &source,
            &ordering,
            &context,
            limits,
        )
        .unwrap();

    assert_eq!(shifted.required_nonzero_guards().len(), 0);
    assert_eq!(shifted.row().terms()[0].shift().values(), &[1]);
}

#[test]
fn bulk_guard_attachment_is_exact_and_preflights_the_whole_batch() {
    let defaults = InvolutiveLimits::default();
    let context = context(1);
    let ordering = active_ordering(1, defaults);
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let twice_n = context.mul(&context.integer(2), &n).unwrap();
    let guards = vec![
        context
            .numerator_condition_with_limits(&n, defaults.indexed_algebra.exact_algebra)
            .unwrap(),
        context
            .numerator_condition_with_limits(&n_plus_one, defaults.indexed_algebra.exact_algebra)
            .unwrap(),
        context
            .numerator_condition_with_limits(&context.one(), defaults.indexed_algebra.exact_algebra)
            .unwrap(),
    ];
    let attached = OreConsequence::try_zero(&ordering, &context, defaults)
        .unwrap()
        .try_require_nonzero_guards(guards, &context, defaults)
        .unwrap();
    assert_eq!(attached.required_nonzero_guards().len(), 2);
    let reverse_with_duplicate = vec![
        context
            .numerator_condition_with_limits(&n_plus_one, defaults.indexed_algebra.exact_algebra)
            .unwrap(),
        context
            .numerator_condition_with_limits(&twice_n, defaults.indexed_algebra.exact_algebra)
            .unwrap(),
        context
            .numerator_condition_with_limits(&n, defaults.indexed_algebra.exact_algebra)
            .unwrap(),
    ];
    let reverse_attached = OreConsequence::try_zero(&ordering, &context, defaults)
        .unwrap()
        .try_require_nonzero_guards(reverse_with_duplicate, &context, defaults)
        .unwrap();
    assert_eq!(
        reverse_attached.required_nonzero_guards(),
        attached.required_nonzero_guards()
    );

    let cap = InvolutiveLimits {
        max_localization_guards: 1,
        ..defaults
    };
    let guards = vec![
        context
            .numerator_condition_with_limits(&n, defaults.indexed_algebra.exact_algebra)
            .unwrap(),
        context
            .numerator_condition_with_limits(&n_plus_one, defaults.indexed_algebra.exact_algebra)
            .unwrap(),
    ];
    assert_eq!(
        OreConsequence::try_zero(&ordering, &context, defaults)
            .unwrap()
            .try_require_nonzero_guards(guards, &context, cap),
        Err(InvolutiveError::ResourceLimit {
            resource: "Ore localization guards",
            requested: 2,
            limit: 1,
        })
    );
}

#[test]
fn ordering_adapter_reuses_the_persisted_coordinate_priority_key() {
    let limits = InvolutiveLimits::default();
    let sector = Mask::try_new([true, true]).unwrap();
    let natural =
        OreOrderingAdapter::try_new(OrderingPolicy::default(), sector.clone(), limits).unwrap();
    let priority =
        CoordinatePriority::try_new(2, &[1, 0], CoordinatePriorityLimits::default()).unwrap();
    let custom_policy = OrderingPolicy::try_with_coordinate_priority(&priority).unwrap();
    let custom = OreOrderingAdapter::try_new(custom_policy, sector, limits).unwrap();
    let first = shift(&[1, 0], limits);
    let second = shift(&[0, 1], limits);

    assert!(natural.try_key(&first).unwrap() > natural.try_key(&second).unwrap());
    assert!(custom.try_key(&first).unwrap() < custom.try_key(&second).unwrap());
    assert_eq!(custom.variable_sequence(), &[1, 0]);
}

#[test]
fn ore_scope_rejects_equal_arity_foreign_sector_order_and_adapter_instances() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let natural = active_ordering(2, limits);
    let source = monomial_consequence(0, &[1, 0], &natural, &context, limits);

    let inactive = OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new([true, false]).unwrap(),
        limits,
    )
    .unwrap();
    assert_eq!(
        JanetBasisEpoch::try_initial(
            [source],
            &inactive,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ForeignOreAction)
    );

    let priority =
        CoordinatePriority::try_new(2, &[1, 0], CoordinatePriorityLimits::default()).unwrap();
    let custom = OreOrderingAdapter::try_new(
        OrderingPolicy::try_with_coordinate_priority(&priority).unwrap(),
        Mask::try_new([true, true]).unwrap(),
        limits,
    )
    .unwrap();
    let natural_source = monomial_consequence(1, &[0, 1], &natural, &context, limits);
    assert_eq!(
        JanetBasisEpoch::try_initial(
            [natural_source],
            &custom,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ForeignOreAction)
    );

    let independently_rebuilt = active_ordering(2, limits);
    let natural_source = monomial_consequence(2, &[1, 1], &natural, &context, limits);
    assert_eq!(
        JanetBasisEpoch::try_initial(
            [natural_source],
            &independently_rebuilt,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ForeignOreAction)
    );
}

#[test]
fn persisted_monomial_order_is_translation_invariant_on_a_small_exhaustive_grid() {
    let limits = InvolutiveLimits::default();
    let priority =
        CoordinatePriority::try_new(2, &[1, 0], CoordinatePriorityLimits::default()).unwrap();
    let orderings = [
        OreOrderingAdapter::try_new(
            OrderingPolicy::default(),
            Mask::try_new([true, false]).unwrap(),
            limits,
        )
        .unwrap(),
        OreOrderingAdapter::try_new(
            OrderingPolicy::try_with_coordinate_priority(&priority).unwrap(),
            Mask::try_new([false, true]).unwrap(),
            limits,
        )
        .unwrap(),
    ];

    for ordering in &orderings {
        for a0 in 0..=3 {
            for a1 in 0..=3 {
                let alpha = shift(&[a0, a1], limits);
                for b0 in 0..=3 {
                    for b1 in 0..=3 {
                        let beta = shift(&[b0, b1], limits);
                        let comparison = ordering
                            .try_key(&alpha)
                            .unwrap()
                            .cmp(&ordering.try_key(&beta).unwrap());
                        for g0 in 0..=2 {
                            for g1 in 0..=2 {
                                let gamma = shift(&[g0, g1], limits);
                                let translated_alpha =
                                    alpha.try_checked_add(&gamma, limits).unwrap();
                                let translated_beta = beta.try_checked_add(&gamma, limits).unwrap();
                                assert_eq!(
                                    ordering
                                        .try_key(&translated_alpha)
                                        .unwrap()
                                        .cmp(&ordering.try_key(&translated_beta).unwrap()),
                                    comparison,
                                    "translation changed the order of {:?} and {:?} by {:?}",
                                    alpha.values(),
                                    beta.values(),
                                    gamma.values(),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
