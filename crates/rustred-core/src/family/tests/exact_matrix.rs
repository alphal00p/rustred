use super::*;

#[test]
fn symbolic_nonsymmetric_basis_has_domain_conditioned_exact_inverse() {
    let context = CoefficientContext::new(["d", "a", "b", "s"]);
    let d = context.parameter("d").unwrap();
    let a_over_s = context.coefficient_fixture("a/s");
    let b = context.parameter("b").unwrap();
    let c0 = context.coefficient_fixture("a+1");
    let c1 = context.coefficient_fixture("b-2");
    let family = IntegralFamily::new(
        "symbolic",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        d,
        vec![
            AffineDenominator::new(c0, vec![a_over_s, context.one()]),
            AffineDenominator::new(c1, vec![b, context.integer(2)]),
        ],
        vec![vec![context.coefficient_fixture("s")]],
        vec![context.coefficient_fixture("a/3"), context.zero()],
    )
    .unwrap();

    assert_eq!(
        family.domain().basis_determinant(),
        &context.coefficient_fixture("(2*a-b*s)/s")
    );
    let determinant_condition = family
        .domain()
        .conditions()
        .find(|condition| {
            condition
                .sources()
                .contains(&CoefficientLocation::BasisDeterminantNumerator)
        })
        .unwrap();
    assert_eq!(
        determinant_condition.polynomial(),
        &context.coefficient_fixture("2*a-b*s").numerator
    );
    assert!(family.domain().conditions().any(|guard| {
        guard
            .sources()
            .contains(&CoefficientLocation::DenominatorCoefficient {
                denominator: 0,
                coordinate: 0,
            })
            && guard.polynomial() == &context.coefficient_fixture("s").numerator
    }));
    assert!(family.domain().conditions().any(|guard| {
        guard
            .sources()
            .contains(&CoefficientLocation::PowerShift { denominator: 0 })
            && guard.polynomial() == &context.integer(3).numerator
    }));
    family.verify_exact_replay().unwrap();
}

#[test]
fn symbolica_matrix_backend_preserves_generic_sizes_orientation_and_replay() {
    let context = CoefficientContext::new(["d", "x"]);

    for size in 1..=6 {
        let family = one_loop_family_from_basis(
            &context,
            &format!("upper-bidiagonal-{size}"),
            upper_bidiagonal_basis(&context, size),
        )
        .unwrap();
        let determinant = (2..=size + 1).product::<usize>();
        assert_eq!(
            family.domain().basis_determinant(),
            &context.integer(i64::try_from(determinant).unwrap())
        );
        assert!(
            family
                .inverse_basis()
                .iter()
                .flatten()
                .all(|entry| context.contains(entry))
        );
        if size == 2 {
            assert_eq!(
                family.inverse_basis()[0][1],
                context.coefficient_fixture("-x/6")
            );
            assert_eq!(family.inverse_basis()[1][0], context.zero());
            assert_eq!(
                family.inverse_basis()[0][0],
                context.coefficient_fixture("1/2")
            );
            assert_eq!(
                family.inverse_basis()[1][1],
                context.coefficient_fixture("1/3")
            );
        }
        family.verify_exact_replay().unwrap();
    }
}

#[test]
fn exact_replay_detects_retained_determinant_and_inverse_tampering() {
    let context = CoefficientContext::new(["d", "x"]);
    let family = one_loop_family_from_basis(
        &context,
        "replay-tamper-seam",
        upper_bidiagonal_basis(&context, 4),
    )
    .unwrap();

    let mut determinant_tamper = family;
    determinant_tamper.domain.basis_determinant = context.integer(1);
    assert!(matches!(
        determinant_tamper.verify_exact_replay(),
        Err(IntegralFamilyError::InternalVerificationFailure { detail })
            if detail.contains("native determinant replay")
    ));

    let mut inverse_tamper = one_loop_family_from_basis(
        &context,
        "replay-tamper-seam",
        upper_bidiagonal_basis(&context, 4),
    )
    .unwrap();
    inverse_tamper.inverse_basis[0][0] = context.zero();
    assert!(matches!(
        inverse_tamper.verify_exact_replay(),
        Err(IntegralFamilyError::InternalVerificationFailure { detail })
            if detail.contains("differs from identity")
    ));
}

#[test]
fn symbolica_matrix_backend_rejects_singular_size_one_and_larger_matrices() {
    let context = CoefficientContext::new(["d", "x"]);
    // Cover both of Symbolica's specialized inverse branches (2x2 and
    // 3x3) as well as the augmented-matrix branch used at size 1 and at
    // sizes four and above.
    for size in [1, 2, 3, 4, 6] {
        let mut basis = upper_bidiagonal_basis(&context, size);
        if size == 1 {
            basis[0][0] = context.zero();
        } else {
            basis[size - 1] = basis[size - 2].clone();
        }
        assert!(matches!(
            one_loop_family_from_basis(&context, &format!("singular-{size}"), basis),
            Err(IntegralFamilyError::SingularDenominatorBasis)
        ));
    }
}

#[test]
fn symbolic_size_four_tracks_pivot_sign_rational_determinant_and_sources() {
    let context = CoefficientContext::new(["d", "x", "s", "t"]);
    let zero = context.zero();
    let one = context.one();
    let basis = vec![
        vec![
            zero.clone(),
            context.coefficient_fixture("x/s"),
            zero.clone(),
            zero.clone(),
        ],
        vec![one.clone(), zero.clone(), zero.clone(), zero.clone()],
        vec![
            zero.clone(),
            zero.clone(),
            context.coefficient_fixture("(x+1)/t"),
            one,
        ],
        vec![zero.clone(), zero.clone(), zero, context.integer(2)],
    ];
    let family = one_loop_family_from_basis(&context, "symbolic-size-four", basis).unwrap();
    let determinant = context.coefficient_fixture("-2*x*(x+1)/(s*t)");

    assert_eq!(family.domain().basis_determinant(), &determinant);
    let determinant_condition = family
        .domain()
        .conditions()
        .find(|condition| {
            condition
                .sources()
                .contains(&CoefficientLocation::BasisDeterminantNumerator)
        })
        .unwrap();
    assert_eq!(determinant_condition.polynomial(), &determinant.numerator);
    assert_eq!(
        determinant_condition.sources(),
        &BTreeSet::from([CoefficientLocation::BasisDeterminantNumerator])
    );
    for (source, parameter) in [
        (
            CoefficientLocation::DenominatorCoefficient {
                denominator: 0,
                coordinate: 1,
            },
            "s",
        ),
        (
            CoefficientLocation::DenominatorCoefficient {
                denominator: 2,
                coordinate: 2,
            },
            "t",
        ),
    ] {
        let guard = family
            .domain()
            .conditions()
            .find(|guard| guard.sources().contains(&source))
            .unwrap();
        assert_eq!(
            guard.polynomial(),
            &context.parameter(parameter).unwrap().numerator
        );
        assert!(guard.sources().contains(&source));
    }
    assert!(
        family
            .inverse_basis()
            .iter()
            .flatten()
            .all(|entry| context.contains(entry))
    );
    family.verify_exact_replay().unwrap();
}

#[test]
fn matrix_boundary_preserves_gmp_coefficients_and_rejects_foreign_maps() {
    let context = CoefficientContext::new(["x"]);
    let mut huge = context.one();
    huge.numerator.coefficients[0] = format!("1{}", "0".repeat(1_500))
        .parse::<Integer>()
        .unwrap();
    let matrix = vec![
        vec![huge.clone(), context.parameter("x").unwrap()],
        vec![context.zero(), context.one()],
    ];
    let (inverse, determinant) =
        invert_symbolic_matrix(&context, &matrix, IntegralFamilyLimits::default()).unwrap();
    assert_eq!(determinant, huge);
    assert!(
        inverse
            .iter()
            .flatten()
            .all(|entry| context.contains(entry))
    );
    verify_inverse(&context, &matrix, &inverse, IntegralFamilyLimits::default()).unwrap();

    let foreign = CoefficientContext::new(["foreign"]);
    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![foreign.one()]],
            IntegralFamilyLimits::default(),
        ),
        Err(IntegralFamilyError::ExactAlgebra(
            ExactAlgebraError::VariableMapMismatch { .. }
        ))
    ));
}

#[test]
fn matrix_boundary_propagates_typed_exact_algebra_limits() {
    let context = CoefficientContext::new(["x"]);
    let x_plus_one = context.coefficient_fixture("x+1");
    let matrix = vec![
        vec![x_plus_one.clone(), context.one()],
        vec![context.one(), x_plus_one],
    ];
    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![context.one()]],
            IntegralFamilyLimits {
                max_matrix_exact_operations: 7,
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ResourceLimit {
            resource: "Symbolica coefficient matrix exact operations",
            requested,
            limit: 7,
        }) if requested > 7
    ));

    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![context.one()]],
            IntegralFamilyLimits {
                max_matrix_input_retained_bytes: 0,
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ResourceLimit {
            resource: "coefficient matrix input retained bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));
    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![context.one()]],
            IntegralFamilyLimits {
                max_matrix_output_retained_bytes: 0,
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ResourceLimit {
            resource: "coefficient matrix output retained bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &matrix,
            IntegralFamilyLimits {
                exact_algebra: ExactAlgebraLimits {
                    max_term_operations: 1,
                    ..ExactAlgebraLimits::default()
                },
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit { .. }
        ))
    ));

    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![context.parameter("x").unwrap()]],
            IntegralFamilyLimits {
                exact_algebra: ExactAlgebraLimits {
                    max_exponent: 0,
                    ..ExactAlgebraLimits::default()
                },
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ExactAlgebra(
            ExactAlgebraError::ExponentLimit { .. }
        ))
    ));
}
