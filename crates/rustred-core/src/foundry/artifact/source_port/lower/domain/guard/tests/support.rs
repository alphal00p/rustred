//! An inactive prepared chart is not an expanding algebra operation, but its
//! input admission and subsequent guard nonvanishing proof remain mandatory.

use super::*;

fn captured_domain(
    context: &IndexedCoefficientContext,
    sector: &[bool; 10],
    equations: &[IndexedCoefficient],
) -> Arc<AffineApplicationDomain> {
    let fixed = [
        Some(0),
        Some(1),
        Some(1),
        None,
        Some(1),
        None,
        Some(1),
        Some(1),
        None,
        Some(0),
    ];
    let indices = std::array::from_fn(|axis| context.base().variables().len() + axis);
    let equations: Vec<_> = equations
        .iter()
        .map(|e| e.raw().numerator.clone())
        .collect();
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::new(fixed).unwrap(),
        &equations,
        &indices,
        sector,
    )
    .unwrap() else {
        panic!("expected the captured rational affine domain")
    };
    Arc::new(AffineApplicationDomain::from_case(&case, sector).unwrap())
}

#[test]
fn captured_four_loop_guard_restricts_without_expansion_and_proves_its_exclusion() {
    let context = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "captured-four-loop-guard-support",
        10,
    )
    .unwrap();
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let a = context.index(5).unwrap();
    let b = context.index(8).unwrap();
    let equation = context
        .sub(
            &context
                .mul(&context.integer(2), &context.index(3).unwrap())
                .unwrap(),
            &context.add(&context.one(), &b).unwrap(),
        )
        .unwrap();
    let excluded = context
        .add(&context.add(&context.one(), &a).unwrap(), &b)
        .unwrap();
    // Exact inner/outer failed guard from FG214 rule74; it involves only
    // n5,n8,d, not the n3 pivot or any fixed coordinate.
    let mut q = context.integer(-10);
    for (scale, monomial) in [
        (-13, b.clone()),
        (-3, context.mul(&b, &b).unwrap()),
        (-12, a.clone()),
        (-5, context.mul(&a, &b).unwrap()),
        (-2, context.mul(&a, &a).unwrap()),
        (3, context.mul(&d, &excluded).unwrap()),
    ] {
        q = context
            .add(
                &q,
                &context.mul(&context.integer(scale), &monomial).unwrap(),
            )
            .unwrap();
    }
    let q = polynomial(&context, &q);
    let sector = [
        false, true, true, false, true, false, true, true, false, false,
    ];
    let target = captured_domain(&context, &sector, &[equation.clone()]);
    let exclusion = captured_domain(&context, &sector, &[equation, excluded]);
    let chart = target.prepare_restriction().unwrap();
    assert!(!chart.affects_polynomial(q.raw()).unwrap());
    let mut work = Work::default();
    assert_eq!(
        restrict(
            &context,
            q.raw(),
            Some((&target, &chart)),
            Default::default(),
            &mut work
        )
        .unwrap(),
        q
    );
    assert_eq!(work.input_terms, 9);
    assert_eq!(work.operations, 9);
    assert_eq!(work.substitutions, 1);
    let piece = LatticeBox::try_new(
        [0, 0, 0, 1, 0, 1, 0, 0, 1, 0],
        [
            Some(0),
            Some(0),
            Some(0),
            None,
            Some(0),
            Some(1),
            Some(0),
            Some(0),
            None,
            Some(0),
        ],
    )
    .unwrap();
    validate_guard_on_domain_with_limits(
        &context,
        &q,
        &piece,
        &sector,
        Some((&target, &chart)),
        &[exclusion],
        Default::default(),
    )
    .unwrap();
}

#[test]
fn unaffected_chart_retains_all_input_degree_and_work_admission() {
    let context = context();
    let target = standard_domain(&context, [None; 3], &[g(&context)]);
    let chart = target.prepare_restriction().unwrap();
    let c = context.index(2).unwrap();
    let value = polynomial(
        &context,
        &context
            .add(&context.mul(&c, &c).unwrap(), &context.one())
            .unwrap(),
    );
    assert!(!chart.affects_polynomial(value.raw()).unwrap());
    let mut term_limit = RuleCellLimits::default();
    term_limit.guard_algebra.max_input_terms = 1;
    let mut bit_limit = RuleCellLimits::default();
    bit_limit.guard_algebra.max_total_integer_bits = 0;
    let mut degree_limit = RuleCellLimits::default();
    degree_limit.guard_algebra.max_factor_total_degree = 1;
    let mut work_limit = RuleCellLimits::default();
    work_limit.guard_algebra.max_exact_hyperplane_replay_work = 0;
    let mut operation_limit = RuleCellLimits::default();
    operation_limit
        .guard_algebra
        .max_exact_hyperplane_replay_substitutions = 0;
    for (limits, expected) in [
        (term_limit, "aggregate input-term budget"),
        (bit_limit, "guard coefficient split input integer bits"),
        (degree_limit, "input exceeds degree budget"),
        (work_limit, "exceeds work budget"),
        (operation_limit, "exceeds work budget"),
    ] {
        let result = restrict(
            &context,
            value.raw(),
            Some((&target, &chart)),
            limits,
            &mut Work::default(),
        );
        assert!(
            result.unwrap_err().to_string().contains(expected),
            "{expected}"
        );
    }
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_exact_hyperplane_replay_terms = 3;
    let mut work = Work::default();
    restrict(
        &context,
        value.raw(),
        Some((&target, &chart)),
        limits,
        &mut work,
    )
    .unwrap();
    assert!(
        restrict(
            &context,
            value.raw(),
            Some((&target, &chart)),
            limits,
            &mut work
        )
        .unwrap_err()
        .to_string()
        .contains("aggregate input-term budget")
    );
    let foreign = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["other_d"]),
        "foreign-guard-support",
        3,
    )
    .unwrap();
    let value = polynomial(&foreign, &foreign.index(2).unwrap());
    assert!(
        restrict(
            &context,
            value.raw(),
            Some((&target, &chart)),
            Default::default(),
            &mut Work::default()
        )
        .is_err()
    );
}

#[test]
fn affected_pivots_and_fixed_coordinates_keep_prospective_limits() {
    let context = context();
    let target = standard_domain(&context, [None, None, Some(0)], &[g(&context)]);
    let chart = target.prepare_restriction().unwrap();
    for axis in [0, 2] {
        let value = polynomial(&context, &context.index(axis).unwrap());
        assert!(chart.affects_polynomial(value.raw()).unwrap());
        let mut terms = RuleCellLimits::default();
        terms.guard_algebra.max_input_terms = 1;
        let mut bits = RuleCellLimits::default();
        bits.guard_algebra.max_total_integer_bits = 8;
        for (limits, expected) in [
            (terms, "prospective term budget"),
            (bits, "prospective integer-bit budget"),
        ] {
            if axis == 2 && expected == "prospective term budget" {
                // A fixed scalar has one prospective term, even when its
                // actual value is zero. No cancellation is needed for this
                // tighter native-replacement support bound.
                assert!(
                    restrict(
                        &context,
                        value.raw(),
                        Some((&target, &chart)),
                        limits,
                        &mut Work::default(),
                    )
                    .unwrap()
                    .is_zero()
                );
                continue;
            }
            assert!(
                restrict(
                    &context,
                    value.raw(),
                    Some((&target, &chart)),
                    limits,
                    &mut Work::default()
                )
                .unwrap_err()
                .to_string()
                .contains(expected),
                "axis={axis} {expected}"
            );
        }
    }
}

#[test]
fn no_op_restriction_is_not_a_nonvanishing_certificate() {
    let context = context();
    let target = standard_domain(&context, [None; 3], &[g(&context)]);
    let chart = target.prepare_restriction().unwrap();
    let piece = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    for coefficient in [context.zero(), context.index(2).unwrap()] {
        let value = polynomial(&context, &coefficient);
        assert!(!chart.affects_polynomial(value.raw()).unwrap());
        assert_eq!(
            restrict(
                &context,
                value.raw(),
                Some((&target, &chart)),
                Default::default(),
                &mut Work::default()
            )
            .unwrap(),
            value
        );
        assert!(
            check(
                &context,
                &coefficient,
                &piece,
                Some(&target),
                &[],
                Default::default()
            )
            .is_err()
        );
    }
}

#[test]
fn inactive_rational_chart_preserves_nonunit_input_content() {
    let context = context();
    let a = context.index(0).unwrap();
    let b = context.index(1).unwrap();
    let target = standard_domain(
        &context,
        [None; 3],
        &[context
            .sub(&context.mul(&context.integer(2), &a).unwrap(), &b)
            .unwrap()],
    );
    let chart = target.prepare_restriction().unwrap();
    assert_eq!(target.has_integral_chart(), Some(false));
    let safe = context
        .mul(
            &context.integer(6),
            &context
                .sub(&context.index(2).unwrap(), &context.one())
                .unwrap(),
        )
        .unwrap();
    let value = polynomial(&context, &safe);
    assert!(!chart.affects_polynomial(value.raw()).unwrap());
    let actual = restrict(
        &context,
        value.raw(),
        Some((&target, &chart)),
        Default::default(),
        &mut Work::default(),
    )
    .unwrap();
    assert_eq!(actual, value);
    assert_eq!(
        chart.restrict_polynomial_value(value.raw()).unwrap(),
        safe.raw().clone()
    );
    assert!(
        check(
            &context,
            &safe,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            Some(&target),
            &[],
            Default::default()
        )
        .is_ok()
    );
}

#[test]
fn captured_h_guard_uses_actual_piece_singleton_after_inactive_chart() {
    let context = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "captured-h-guard-support",
        10,
    )
    .unwrap();
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let a = context.index(3).unwrap();
    let b = context.index(7).unwrap();
    let a2 = context.mul(&a, &a).unwrap();
    let b2 = context.mul(&b, &b).unwrap();
    let mut q = context.zero();
    for (scale, monomial) in [
        (-9, b.clone()),
        (-12, b2.clone()),
        (-3, context.mul(&b2, &b).unwrap()),
        (126, a.clone()),
        (66, context.mul(&a, &b).unwrap()),
        (6, context.mul(&a, &b2).unwrap()),
        (96, a2.clone()),
        (27, context.mul(&a2, &b).unwrap()),
        (18, context.mul(&a2, &a).unwrap()),
        (3, context.mul(&d, &b).unwrap()),
        (3, context.mul(&d, &b2).unwrap()),
        (-96, context.mul(&d, &a).unwrap()),
        (-27, context.mul(&d, &context.mul(&a, &b).unwrap()).unwrap()),
        (-36, context.mul(&d, &a2).unwrap()),
        (18, context.mul(&context.mul(&d, &d).unwrap(), &a).unwrap()),
    ] {
        q = context
            .add(
                &q,
                &context.mul(&context.integer(scale), &monomial).unwrap(),
            )
            .unwrap();
    }
    let q = polynomial(&context, &q);
    let equation = context
        .sub(
            &context
                .mul(&context.integer(2), &context.index(0).unwrap())
                .unwrap(),
            &context
                .add(&context.add(&context.one(), &a).unwrap(), &b)
                .unwrap(),
        )
        .unwrap();
    let sector = [
        false, true, false, false, true, true, true, false, true, false,
    ];
    let fixed = [
        None,
        Some(1),
        Some(0),
        None,
        Some(1),
        Some(1),
        Some(1),
        None,
        Some(1),
        Some(0),
    ];
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::new(fixed).unwrap(),
        &[equation.raw().numerator.clone()],
        &std::array::from_fn(|axis| axis + 1),
        &sector,
    )
    .unwrap() else {
        panic!("expected captured H chart")
    };
    let target = AffineApplicationDomain::from_case(&case, &sector).unwrap();
    let chart = target.prepare_restriction().unwrap();
    assert!(!chart.affects_polynomial(q.raw()).unwrap());
    let piece = LatticeBox::try_new(
        [2, 0, 0, 0, 0, 0, 0, 2, 0, 0],
        [
            None,
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            None,
            Some(0),
            Some(0),
        ],
    )
    .unwrap();
    // On n3=0, the d coefficient is 3*n7*(n7+1), nonzero for n7<=-2.
    validate_guard_on_domain_with_limits(
        &context,
        &q,
        &piece,
        &sector,
        Some((&target, &chart)),
        &[],
        Default::default(),
    )
    .unwrap();
}

#[test]
fn singleton_specialization_keeps_genuine_zeros_and_full_precharges() {
    let context = context();
    let a = context.index(0).unwrap();
    let b = context.index(1).unwrap();
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let q = polynomial(
        &context,
        &context.add(&context.mul(&d, &a).unwrap(), &b).unwrap(),
    );
    let zero_piece = LatticeBox::try_new([0; 3], [Some(0), Some(0), None]).unwrap();
    assert!(
        !misses_target(
            &context,
            &q,
            &zero_piece,
            &[false; 3],
            None,
            Default::default(),
            &mut Work::default()
        )
        .unwrap()
    );
    let nonzero_piece = LatticeBox::try_new([1, 0, 0], [Some(1), Some(0), None]).unwrap();
    assert!(
        misses_target(
            &context,
            &q,
            &nonzero_piece,
            &[false; 3],
            None,
            Default::default(),
            &mut Work::default()
        )
        .unwrap()
    );
    let mut work_limit = RuleCellLimits::default();
    work_limit.guard_algebra.max_exact_hyperplane_replay_work = 3;
    let mut term_limit = RuleCellLimits::default();
    term_limit.guard_algebra.max_exact_hyperplane_replay_terms = 3;
    let mut count_limit = RuleCellLimits::default();
    count_limit
        .guard_algebra
        .max_exact_hyperplane_replay_substitutions = 1;
    let mut bit_limit = RuleCellLimits::default();
    bit_limit.indexed_algebra.max_specialization_integer_bits = 0;
    for limits in [work_limit, term_limit, count_limit, bit_limit] {
        assert!(
            misses_target(
                &context,
                &q,
                &nonzero_piece,
                &[false; 3],
                None,
                limits,
                &mut Work::default()
            )
            .is_err()
        );
    }
}

#[test]
fn singleton_physical_mapping_never_narrows_unrepresentable_integer_roots() {
    let context = context();
    let n = context.index(0).unwrap();
    let huge = context
        .mul(&context.integer(i64::MIN), &context.integer(2))
        .unwrap();
    // n = 2*i64::MIN+1 = -u64::MAX is a genuine root in a mathematical
    // singleton box; it is outside the shared i64 specialization carrier.
    let huge_root = context.add(&huge, &context.one()).unwrap();
    let q = polynomial(&context, &context.sub(&n, &huge_root).unwrap());
    let piece = LatticeBox::try_new([u64::MAX, 0, 0], [Some(u64::MAX), None, None]).unwrap();
    let mut box_work = Work::default();
    super::super::conjunction::precharge_box_equation(
        q.raw(),
        context.base().variables().len(),
        &piece,
        &[false; 3],
        Default::default(),
        &mut box_work,
    )
    .unwrap();
    let mut work = Work::default();
    assert!(
        !misses_target(
            &context,
            &q,
            &piece,
            &[false; 3],
            None,
            Default::default(),
            &mut work
        )
        .unwrap()
    );
    // The native box check is precharged even when it finds a genuine root.
    // No additional i64 singleton specialization can represent this root.
    assert_eq!(work.operations, box_work.operations);
    assert_eq!(work.input_terms, box_work.input_terms);
    assert_eq!(work.substitutions, box_work.substitutions);
    // Positive x=u64::MAX corresponds to n=u64::MAX+1, not a wrapped zero.
    let positive_root = context
        .mul(&context.integer(i64::MIN), &context.integer(-2))
        .unwrap();
    let q = polynomial(&context, &context.sub(&n, &positive_root).unwrap());
    assert!(
        !misses_target(
            &context,
            &q,
            &piece,
            &[true, false, false],
            None,
            Default::default(),
            &mut Work::default()
        )
        .unwrap()
    );
    // The exactly representable boundary n=i64::MIN is still specialized.
    let q = polynomial(
        &context,
        &context.sub(&n, &context.integer(i64::MIN)).unwrap(),
    );
    let local = 1u64 << 63;
    let piece = LatticeBox::try_new([local, 0, 0], [Some(local), None, None]).unwrap();
    let mut box_work = Work::default();
    super::super::conjunction::precharge_box_equation(
        q.raw(),
        context.base().variables().len(),
        &piece,
        &[false; 3],
        Default::default(),
        &mut box_work,
    )
    .unwrap();
    let mut work = Work::default();
    assert!(
        !misses_target(
            &context,
            &q,
            &piece,
            &[false; 3],
            None,
            Default::default(),
            &mut work
        )
        .unwrap()
    );
    assert_eq!(work.operations, box_work.operations + q.raw().nterms());
    assert_eq!(work.input_terms, box_work.input_terms + q.raw().nterms());
    assert_eq!(work.substitutions, box_work.substitutions + 1);
}
