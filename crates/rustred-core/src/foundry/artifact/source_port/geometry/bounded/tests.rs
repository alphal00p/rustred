use crate::algebra::CoefficientContext;

use super::*;

fn context(arity: usize) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "bounded-source-port-face",
        arity,
    )
    .unwrap()
}

fn cell(lower: &[u64], upper: &[Option<u64>]) -> LatticeBox {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).unwrap()
}

fn vanishes(
    context: &IndexedCoefficientContext,
    coefficient: &IndexedCoefficient,
    cell: &LatticeBox,
    sector: &[bool],
) -> bool {
    coefficient_vanishes(
        context,
        coefficient,
        cell,
        sector,
        Default::default(),
        Default::default(),
    )
    .unwrap()
}

fn two_roots(context: &IndexedCoefficientContext, axis: usize) -> IndexedCoefficient {
    let n = context.index(axis).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    context.mul(&n, &n_plus_one).unwrap()
}

#[test]
fn finite_roots_are_exhaustive_and_an_infinite_tail_is_not_sampled() {
    let c = context(1);
    let n = c.index(0).unwrap();
    let both = cell(&[0], &[Some(1)]); // Physical n=0,-1.
    let roots = two_roots(&c, 0);
    assert!(vanishes(&c, &roots, &both, &[false]));
    let one_root = c.add(&n, &c.one()).unwrap();
    assert!(!vanishes(&c, &one_root, &both, &[false]));
    assert!(vanishes(&c, &one_root, &cell(&[1], &[Some(1)]), &[false]));
    assert!(!vanishes(&c, &roots, &cell(&[0], &[None]), &[false]));
    assert!(!vanishes(&c, &roots, &cell(&[0], &[Some(2)]), &[false]));
}

#[test]
fn two_finite_axes_are_a_cartesian_product_with_symbolic_spectators() {
    let c = context(3);
    let sum = c.add(&two_roots(&c, 0), &two_roots(&c, 1)).unwrap();
    let domain = cell(&[0, 0, 0], &[Some(1), Some(1), None]);
    assert!(vanishes(&c, &sum, &domain, &[false; 3]));
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let symbolic = c
        .add(&sum, &c.mul(&d, &c.index(2).unwrap()).unwrap())
        .unwrap();
    assert!(!vanishes(&c, &symbolic, &domain, &[false; 3]));
    assert!(vanishes(
        &c,
        &c.mul(&sum, &symbolic).unwrap(),
        &domain,
        &[false; 3],
    ));
}

#[test]
fn denominator_is_restricted_before_rational_cancellation_at_every_leaf() {
    let c = context(2);
    let numerator = c.index(0).unwrap();
    let denominator = c.add(&c.index(1).unwrap(), &c.one()).unwrap();
    let quotient = c.div(&numerator, &denominator).unwrap();
    let safe = cell(&[0, 0], &[Some(0), Some(0)]);
    assert!(vanishes(&c, &quotient, &safe, &[false; 2]));
    // The numerator vanishes on the fixed n0=0 face, but n1=-1 in the
    // second finite leaf makes the original denominator zero too.
    let singular = cell(&[0, 0], &[Some(0), Some(1)]);
    assert!(!vanishes(&c, &quotient, &singular, &[false; 2]));
    let only_pole = cell(&[0, 1], &[Some(0), Some(1)]);
    assert!(!vanishes(&c, &quotient, &only_pole, &[false; 2]));
    // A nonzero symbolic denominator is not a claim that its pole is
    // absent. The caller must retain n1+1 != 0 as an owner obligation.
    let symbolic_guarded = cell(&[0, 0], &[Some(0), None]);
    assert!(vanishes(&c, &quotient, &symbolic_guarded, &[false; 2]));
}

#[test]
fn fixed_face_can_remove_an_unrepresentable_but_then_irrelevant_axis() {
    let c = context(2);
    let n0 = c.index(0).unwrap();
    let n1 = c.index(1).unwrap();
    let huge = cell(&[0, u64::MAX], &[Some(0), Some(u64::MAX)]);
    assert!(vanishes(
        &c,
        &c.mul(&n0, &n1).unwrap(),
        &huge,
        &[false, true]
    ));
    assert!(vanishes(&c, &n0, &huge, &[false, true]));
    let still_needed_denominator = c.div(&n0, &c.add(&n1, &c.one()).unwrap()).unwrap();
    let failure = coefficient_vanishes(
        &c,
        &still_needed_denominator,
        &huge,
        &[false, true],
        Default::default(),
        Default::default(),
    )
    .unwrap_err();
    assert!(failure.to_string().contains("index 1 outside i64"));
}

#[test]
fn physical_i64_boundaries_are_exact_and_needed_wider_values_are_rejected() {
    let c = context(1);
    let n = c.index(0).unwrap();
    let min_local = 1_u64 << 63;
    let at_min = c.sub(&n, &c.integer(i64::MIN)).unwrap();
    assert!(vanishes(
        &c,
        &at_min,
        &cell(&[min_local], &[Some(min_local)]),
        &[false]
    ));
    let max_local = i64::MAX as u64 - 1;
    let at_max = c.sub(&n, &c.integer(i64::MAX)).unwrap();
    assert!(vanishes(
        &c,
        &at_max,
        &cell(&[max_local], &[Some(max_local)]),
        &[true]
    ));
    for (local, positive) in [(min_local + 1, false), (max_local + 1, true)] {
        let failure = coefficient_vanishes(
            &c,
            &n,
            &cell(&[local], &[Some(local)]),
            &[positive],
            Default::default(),
            Default::default(),
        )
        .unwrap_err();
        assert!(failure.to_string().contains("outside i64"));
    }
    // Infinite endpoints remain symbolic, even when their finite lower
    // bound is beyond the concrete runtime carrier.
    assert!(!vanishes(&c, &n, &cell(&[u64::MAX], &[None]), &[true]));
    assert!(vanishes(
        &c,
        &c.zero(),
        &cell(&[u64::MAX], &[None]),
        &[true]
    ));
}

#[test]
fn an_out_of_range_finite_endpoint_is_not_clipped_to_the_runtime_carrier() {
    let c = context(1);
    let n = c.index(0).unwrap();
    let failure = coefficient_vanishes(
        &c,
        &n,
        &cell(&[0], &[Some(1_u64 << 63)]),
        &[true],
        Default::default(),
        Default::default(),
    )
    .unwrap_err();
    assert!(failure.to_string().contains("outside i64"));
}

#[test]
fn full_leaf_and_coordinate_budgets_are_checked_before_the_first_leaf() {
    let c = context(2);
    let value = c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap();
    // The very first leaf would already refute vanishing. The requested
    // full traversal still has four leaves, and must be budgeted first.
    let domain = cell(&[0, 0], &[Some(1), Some(1)]);
    let mut limits = CompletionGeometryLimits::default();
    limits.max_uncovered_boxes = 3;
    assert!(
        coefficient_vanishes(&c, &value, &domain, &[true; 2], Default::default(), limits,)
            .unwrap_err()
            .to_string()
            .contains("finite leaves")
    );
    limits = CompletionGeometryLimits::default();
    limits.max_uncovered_box_coordinate_cells = 7;
    assert!(
        coefficient_vanishes(&c, &value, &domain, &[true; 2], Default::default(), limits,)
            .unwrap_err()
            .to_string()
            .contains("finite leaf coordinates")
    );
    limits = CompletionGeometryLimits::default();
    limits.max_split_operations = 7;
    assert!(
        coefficient_vanishes(&c, &value, &domain, &[true; 2], Default::default(), limits,)
            .unwrap_err()
            .to_string()
            .contains("finite assignments")
    );
}

#[test]
fn constants_do_not_bypass_geometry_or_context_validation() {
    let c = context(1);
    let domain = cell(&[0], &[None]);
    let mut limits = CompletionGeometryLimits::default();
    limits.max_uncovered_boxes = 0;
    assert!(
        coefficient_vanishes(&c, &c.zero(), &domain, &[true], Default::default(), limits,).is_err()
    );
    limits = CompletionGeometryLimits::default();
    limits.max_arity = 0;
    assert!(
        coefficient_vanishes(&c, &c.zero(), &domain, &[true], Default::default(), limits,).is_err()
    );
    let foreign = IndexedCoefficientContext::try_new(c.base(), "foreign-bounded-face", 1).unwrap();
    assert!(
        coefficient_vanishes(
            &c,
            &foreign.zero(),
            &domain,
            &[true],
            Default::default(),
            Default::default(),
        )
        .is_err()
    );
    assert!(
        coefficient_vanishes(
            &c,
            &c.zero(),
            &domain,
            &[],
            Default::default(),
            Default::default(),
        )
        .is_err()
    );
    assert!(
        coefficient_vanishes(
            &c,
            &c.zero(),
            &cell(&[0, 0], &[None; 2]),
            &[true],
            Default::default(),
            Default::default(),
        )
        .is_err()
    );
}

#[test]
fn native_specialization_limits_remain_authoritative() {
    let c = context(1);
    let n = c.index(0).unwrap();
    let square = c.mul(&n, &n).unwrap();
    let domain = cell(&[3], &[Some(3)]); // Physical n=4.
    let mut limits = IndexedAlgebraLimits::default();
    limits.max_specialization_integer_bits = 1;
    assert!(
        coefficient_vanishes(&c, &square, &domain, &[true], limits, Default::default(),)
            .unwrap_err()
            .to_string()
            .contains("integer bits")
    );
    limits = IndexedAlgebraLimits::default();
    limits.max_specialization_power_operations = 0;
    assert!(
        coefficient_vanishes(&c, &square, &domain, &[true], limits, Default::default(),)
            .unwrap_err()
            .to_string()
            .contains("power operations")
    );
    limits = IndexedAlgebraLimits::default();
    limits.exact_algebra.max_polynomial_terms = 1;
    assert!(
        coefficient_vanishes(
            &c,
            &c.add(&n, &c.one()).unwrap(),
            &domain,
            &[true],
            limits,
            Default::default(),
        )
        .is_err()
    );
}
