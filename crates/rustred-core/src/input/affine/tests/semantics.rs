use super::*;

#[test]
fn one_loop_external_square_contracts_gram_and_preserves_cross_factor() {
    let compiler = compiler(&["d", "m2", "s"], &["k"], &["p"], &[&["s"]]);
    let compiled = compiler.compile_expression("(k+p)^2-m2").unwrap();
    assert_coefficients(&compiler, &compiled, "s-m2", &["1", "2"]);
    assert_eq!(
        compiler.coordinates,
        &[
            ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 0,
                external_index: 0,
            },
        ]
    );
}

#[test]
fn two_loop_square_lowers_in_generic_upper_triangular_order() {
    let compiler = compiler(&["m2"], &["k1", "k2"], &[], &[]);
    let compiled = compiler.compile_expression("(k1+k2)^2-m2").unwrap();
    assert_coefficients(&compiler, &compiled, "-m2", &["1", "2", "1"]);
}

#[test]
fn validated_sp_accepts_rational_parameter_vector_coefficients() {
    let compiler = compiler(&["a", "s", "g"], &["k1", "k2"], &["p"], &[&["g"]]);
    let compiled = compiler
        .compile_expression("sp(a/s*k1+p,k2-2*p)+a/s*k1^2")
        .unwrap();
    assert_coefficients(
        &compiler,
        &compiled,
        "-2*g",
        &["a/s", "a/s", "0", "-2*a/s", "1"],
    );
}

#[test]
fn exact_parameter_denominators_are_retained_without_map_extension() {
    let compiler = compiler(&["a"], &["k"], &[], &[]);
    let compiled = compiler.compile_expression("k^2/(a+1)").unwrap();
    assert_coefficients(&compiler, &compiled, "0", &["1/(a+1)"]);
    assert_eq!(
        compiler.parse_base_expression("(a-1)/(a+1)").unwrap(),
        compiler
            .test_coefficient_context()
            .coefficient_fixture("(a-1)/(a+1)")
    );
}

#[test]
fn unknown_symbols_and_functions_are_rejected() {
    let compiler = compiler(&["a"], &["k"], &[], &[]);
    assert!(matches!(
        compiler.compile_expression("q^2"),
        Err(SymbolicaAffineDenominatorError::UnknownSymbol(_))
    ));
    assert!(matches!(
        compiler.compile_expression("f(k)"),
        Err(SymbolicaAffineDenominatorError::UnsupportedFunction(_))
    ));
}

#[test]
fn momentum_denominators_noninteger_powers_and_wrong_degrees_are_rejected() {
    let compiler = compiler(&["a"], &["k"], &[], &[]);
    assert!(matches!(
        compiler.compile_expression("1/k"),
        Err(SymbolicaAffineDenominatorError::NegativeMomentumPower { .. })
    ));
    assert!(matches!(
        compiler.compile_expression("k^(1/2)"),
        Err(SymbolicaAffineDenominatorError::UnsupportedPower(_))
    ));
    assert!(matches!(
        compiler.compile_expression("k+a"),
        Err(SymbolicaAffineDenominatorError::MomentumDegreeOne { .. })
    ));
    assert!(matches!(
        compiler.compile_expression("k^3"),
        Err(SymbolicaAffineDenominatorError::MomentumDegreeTooHigh { degree: 3, .. })
    ));
}

#[test]
fn scalar_product_requires_two_homogeneous_vector_linear_arguments() {
    let compiler = compiler(&["a"], &["k"], &[], &[]);
    assert!(matches!(
        compiler.compile_expression("sp(k+1,k)"),
        Err(SymbolicaAffineDenominatorError::InvalidScalarProductArgument { argument: 0, .. })
    ));
    assert!(matches!(
        compiler.compile_expression("sp(k)"),
        Err(SymbolicaAffineDenominatorError::MalformedScalarProduct { arguments: 1, .. })
    ));
    assert!(matches!(
        compiler.compile_expression("sp(sp(k,k)*k,k)"),
        Err(SymbolicaAffineDenominatorError::NestedScalarProduct(_))
    ));
}

#[test]
fn gram_and_declaration_authentication_is_strict() {
    let coefficients = CoefficientContext::new(["s", "t"]);
    let s = coefficients.coefficient_fixture("s");
    let t = coefficients.coefficient_fixture("t");
    assert!(matches!(
        SymbolicaAffineDenominatorCompiler::try_new(
            coefficients.clone(),
            vec!["k".to_owned()],
            vec!["p".to_owned(), "q".to_owned()],
            vec![vec![s.clone(), t.clone()], vec![s, t.clone()]],
            SymbolicaAffineDenominatorLimits::default(),
        ),
        Err(SymbolicaAffineDenominatorError::AsymmetricExternalGram { .. })
    ));
    assert!(matches!(
        SymbolicaAffineDenominatorCompiler::try_new(
            coefficients,
            vec!["k".to_owned()],
            vec!["k".to_owned()],
            vec![vec![t]],
            SymbolicaAffineDenominatorLimits::default(),
        ),
        Err(SymbolicaAffineDenominatorError::DuplicateLabel { .. })
    ));
}

#[test]
fn exact_and_one_below_input_node_limits_are_deterministic() {
    let compiler = compiler(&["a"], &["k"], &[], &[]);
    let source = try_parse!("k^2+a", default_namespace = RUSTRED_NAMESPACE).unwrap();
    let (_, exact_nodes) = {
        let shape = checked_atom_shape(
            source.as_view(),
            SymbolicaAffineDenominatorLimits::default(),
        )
        .unwrap();
        (shape.1, shape.0)
    };
    let mut exact = compiler.test_limits();
    exact.max_input_nodes = exact_nodes;
    compiler
        .test_with_limits(exact)
        .compile(source.as_view())
        .unwrap();

    let mut below = exact;
    below.max_input_nodes = exact_nodes - 1;
    let below = compiler.test_with_limits(below);
    assert!(matches!(
        below.compile(source.as_view()),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "input Atom nodes",
            ..
        })
    ));
}

#[test]
fn two_external_momenta_use_the_complete_off_diagonal_gram_matrix() {
    let compiler = compiler(
        &["spp", "spq", "sqq"],
        &["k"],
        &["p", "q"],
        &[&["spp", "spq"], &["spq", "sqq"]],
    );
    let square = compiler.compile_expression("(k+p+q)^2").unwrap();
    assert_coefficients(&compiler, &square, "spp+2*spq+sqq", &["1", "2", "2"]);
    let explicit = compiler.compile_expression("sp(p,q)+k^2").unwrap();
    assert_coefficients(&compiler, &explicit, "spq", &["1", "0", "0"]);
}

#[test]
fn zero_parameter_fields_are_supported() {
    let compiler = compiler(&[], &["k"], &[], &[]);
    let compiled = compiler.compile_expression("k^2+1").unwrap();
    assert_coefficients(&compiler, &compiled, "1", &["1"]);
}

#[test]
fn signed_constants_coefficients_and_parameter_powers_are_exact() {
    let compiler = compiler(&["a"], &["k"], &[], &[]);
    let signed = compiler.compile_expression("-2*k^2-3").unwrap();
    assert_coefficients(&compiler, &signed, "-3", &["-2"]);
    let inverse_parameter = compiler.compile_expression("a^-2*k^2").unwrap();
    assert_coefficients(&compiler, &inverse_parameter, "0", &["1/a^2"]);
}

#[test]
fn explicit_external_scalar_products_close_compositionally() {
    let compiler = compiler(
        &["spp", "spq", "sqq"],
        &["k"],
        &["p", "q"],
        &[&["spp", "spq"], &["spq", "sqq"]],
    );
    let product = compiler.compile_expression("sp(p,q)*k^2").unwrap();
    assert_coefficients(&compiler, &product, "0", &["spq", "0", "0"]);
    let square = compiler.compile_expression("sp(p,p)^2+k^2").unwrap();
    assert_coefficients(&compiler, &square, "spp^2", &["1", "0", "0"]);
    let quotient = compiler.compile_expression("k^2/sp(p,p)").unwrap();
    assert_coefficients(&compiler, &quotient, "0", &["1/spp", "0", "0"]);
}

#[test]
fn loop_coordinate_scalar_products_remain_nonlinear_under_products() {
    let compiler = compiler(&["spp"], &["k"], &["p"], &[&["spp"]]);
    assert!(matches!(
        compiler.compile_expression("sp(k,k)^2"),
        Err(SymbolicaAffineDenominatorError::MomentumDegreeTooHigh { degree: 4, .. })
    ));
    assert!(matches!(
        compiler.compile_expression("sp(k,p)*sp(k,p)"),
        Err(SymbolicaAffineDenominatorError::MomentumDegreeTooHigh { degree: 4, .. })
    ));
    assert!(matches!(
        compiler.compile_expression("1/sp(k,k)"),
        Err(SymbolicaAffineDenominatorError::NegativeMomentumPower { exponent: -1, .. })
    ));
}
