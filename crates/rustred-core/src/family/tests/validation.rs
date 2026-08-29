use super::*;

#[test]
fn validates_labels_gram_arities_and_contexts() {
    let context = CoefficientContext::new(["d"]);
    let result = IntegralFamily::new(
        "none",
        Vec::new(),
        Vec::new(),
        context.clone(),
        context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert!(matches!(result, Err(IntegralFamilyError::NoLoopMomenta)));

    let result = IntegralFamily::new(
        "overlap",
        vec!["q".into()],
        vec!["q".into()],
        context.clone(),
        context.one(),
        identity_denominators(&context, 2),
        vec![vec![context.one()]],
        vec![context.zero(); 2],
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::MomentumLabelOverlap { .. })
    ));

    let result = IntegralFamily::new(
        "wrong-denominator-count",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.one(),
        identity_denominators(&context, 1),
        vec![vec![context.one()]],
        vec![context.zero(); 2],
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::WrongDenominatorCount {
            expected: 2,
            actual: 1
        })
    ));

    let result = IntegralFamily::new(
        "wrong-power-shift-count",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.one(),
        identity_denominators(&context, 1),
        Vec::new(),
        Vec::new(),
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::WrongPowerShiftCount {
            expected: 1,
            actual: 0
        })
    ));

    let result = IntegralFamily::new(
        "bad-gram",
        vec!["k".into()],
        vec!["p".into(), "q".into()],
        context.clone(),
        context.one(),
        identity_denominators(&context, 3),
        vec![
            vec![context.one(), context.one()],
            vec![context.zero(), context.one()],
        ],
        vec![context.zero(); 3],
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::AsymmetricExternalGram { row: 0, column: 1 })
    ));

    let foreign = CoefficientContext::new(["x"]);
    let result = IntegralFamily::new(
        "foreign",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        foreign.one(),
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::ForeignCoefficientContext {
            location: CoefficientLocation::Dimension
        })
    ));
}

#[test]
fn singular_symbolic_basis_is_rejected_but_singular_external_gram_is_allowed() {
    let context = CoefficientContext::new(["d"]);
    let singular = IntegralFamily::new(
        "singular",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.one(),
        vec![
            AffineDenominator::new(context.zero(), vec![context.one(), context.integer(2)]),
            AffineDenominator::new(context.zero(), vec![context.integer(2), context.integer(4)]),
        ],
        vec![vec![context.zero()]],
        vec![context.zero(); 2],
    );
    assert!(matches!(
        singular,
        Err(IntegralFamilyError::SingularDenominatorBasis)
    ));

    let valid = IntegralFamily::new(
        "null-external",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.one(),
        identity_denominators(&context, 2),
        vec![vec![context.zero()]],
        vec![context.zero(); 2],
    )
    .unwrap();
    valid.verify_exact_replay().unwrap();
}

#[test]
fn rational_base_field_without_parameters_is_supported() {
    let context = CoefficientContext::new(Vec::<String>::new());
    let family = IntegralFamily::new(
        "rational",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.integer(4),
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap();
    assert!(family.coefficient_context().parameter_names().is_empty());
    family.verify_exact_replay().unwrap();
}

#[test]
fn family_authentication_rejects_malformed_coefficients_and_resource_limits() {
    let context = CoefficientContext::new(["x"]);
    let mut malformed_dimension = context.one();
    malformed_dimension.numerator.exponents.push(0);
    let malformed = IntegralFamily::new(
        "malformed",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        malformed_dimension,
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
    );
    assert!(matches!(
        malformed,
        Err(IntegralFamilyError::InvalidCoefficient {
            location: CoefficientLocation::Dimension,
            error: ExactAlgebraError::MalformedExponentLayout { .. },
        })
    ));

    let limited = IntegralFamily::new_with_limits(
        "limited",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.one(),
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
        IntegralFamilyLimits {
            max_scalar_products: 0,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        limited,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family scalar products",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn derivative_cache_limits_count_expansions_and_all_dense_coefficient_cells() {
    let context = CoefficientContext::new(["d"]);
    let build = |limits| {
        IntegralFamily::new_with_limits(
            "two-loop-derivative-cache-census",
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            context.clone(),
            context.one(),
            identity_denominators(&context, 3),
            Vec::new(),
            vec![context.zero(); 3],
            limits,
        )
    };

    // D=3, L=2, E=0: 3*2*2=12 affine expansions, each retaining
    // one constant and three denominator coefficients, hence 48 cells.
    let exact = IntegralFamilyLimits {
        max_derivative_contractions: 12,
        max_derivative_contraction_coefficient_cells: 48,
        ..IntegralFamilyLimits::default()
    };
    build(exact).unwrap().verify_exact_replay().unwrap();

    assert!(matches!(
        build(IntegralFamilyLimits {
            max_derivative_contractions: 11,
            ..exact
        }),
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family derivative contractions",
            requested: 12,
            limit: 11,
        })
    ));
    assert!(matches!(
        build(IntegralFamilyLimits {
            max_derivative_contraction_coefficient_cells: 47,
            ..exact
        }),
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family derivative contraction coefficient cells",
            requested: 48,
            limit: 47,
        })
    ));
}
