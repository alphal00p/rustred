use super::*;

#[test]
fn numeric_preconversion_envelope_checks_boundary_and_power_growth() {
    let boundary = compiler_with(|limits| limits.max_preconversion_integer_bits = 21);
    let _ = parse_base(&boundary, "12345")
        .expect("the exact conservative numeric-bit boundary should pass");
    let below = compiler_with(|limits| limits.max_preconversion_integer_bits = 20);
    let one_below = parse_base(&below, "12345");
    assert!(matches!(
        one_below,
        Err(Error::ResourceLimit {
            resource: "pre-conversion integer bits",
            ..
        })
    ));

    let huge_power = format!("{}^256", "9".repeat(200_000));
    let default_compiler = Compiler::new(Limits::default().with_diagnostics()).unwrap();
    let rejected = parse_base(&default_compiler, &huge_power);
    assert!(matches!(
        rejected,
        Err(Error::ResourceLimit {
            resource: "pre-conversion integer bits",
            ..
        }) | Err(Error::ResourceLimit {
            resource: "aggregate pre-conversion integer bits",
            ..
        })
    ));

    let inverse = compiler_with(|limits| limits.max_preconversion_integer_bits = 100);
    let inverse_growth = parse_base(&inverse, "1/(99^4)");
    assert!(matches!(
        inverse_growth,
        Err(Error::ResourceLimit {
            resource: "aggregate pre-conversion integer bits",
            ..
        }) | Err(Error::ResourceLimit {
            resource: "pre-conversion integer bits",
            ..
        })
    ));
}

#[test]
fn explicit_text_fields_share_one_preconversion_integer_budget() {
    let compiler = compiler_with(|limits| limits.max_preconversion_integer_bits = 20);
    let _ =
        parse_base(&compiler, "99").expect("the dimension field is individually below the budget");
    let _ = parse_base(&compiler, "k^2-99")
        .expect("the denominator field is individually below the budget");
    let aggregate = compiler.compile_text(TextProject {
        name: None,
        parameters: None,
        loop_momenta: vec!["k".to_owned()],
        external_momenta: vec![],
        dimension: "99".to_owned(),
        propagators: vec![TextPropagator {
            id: "D1".to_owned(),
            expression: "k^2-99".to_owned(),
            target_power: 1,
            power_shift: None,
        }],
        external_gram: vec![],
        numerator: None,
    });
    assert!(matches!(
        aggregate,
        Err(Error::ResourceLimit {
            resource: "aggregate pre-conversion integer bits",
            ..
        })
    ));
}

#[test]
fn caller_owned_large_atom_is_bounded_before_project_clones() {
    let huge_integer = "9"
        .repeat(2_000)
        .parse::<Integer>()
        .expect("test integer should parse");
    let huge_dimension = Atom::num(huge_integer);
    let default_compiler = Compiler::new(Limits::default().with_diagnostics()).unwrap();
    let denominator =
        parse_base(&default_compiler, "k^2-1").expect("small denominator should parse");
    let logical_bytes = huge_dimension
        .as_view()
        .get_byte_size()
        .checked_add(denominator.as_view().get_byte_size())
        .expect("test byte count should fit");
    let mut limits = Limits::default().with_diagnostics();
    limits.max_retained_atom_bytes = logical_bytes;
    let rejected = Compiler::new(limits)
        .expect("bounded compiler should initialize")
        .compile_atoms(AtomProject {
            name: None,
            parameters: None,
            loop_momenta: vec!["k".to_owned()],
            external_momenta: vec![],
            dimension: huge_dimension,
            propagators: vec![AtomPropagator {
                id: "D1".to_owned(),
                expression: denominator,
                target_power: 1,
                power_shift: None,
            }],
            external_gram: vec![],
            numerator: None,
        });
    assert!(matches!(
        rejected,
        Err(Error::ResourceLimit {
            resource: "retained project Atom bytes",
            ..
        })
    ));
}

#[test]
fn raw_preflight_rejects_depth_integer_and_unique_name_excesses() {
    let flat_boundary = compiler_with(|limits| limits.max_atom_nodes = 5);
    let _ = parse_base(&flat_boundary, "a+b")
        .expect("the exact conservative flat lexical boundary should pass");
    let flat_compiler = compiler_with(|limits| limits.max_atom_nodes = 4);
    let flat = parse_base(&flat_compiler, "a+b");
    assert!(matches!(
        flat,
        Err(Error::ResourceLimit {
            resource: "raw lexical tokens",
            ..
        })
    ));

    let units_compiler = compiler_with(|limits| limits.max_raw_parser_units = 2);
    let units = parse_base(&units_compiler, "a+b");
    assert!(matches!(
        units,
        Err(Error::ResourceLimit {
            resource: "raw parser units",
            ..
        })
    ));

    let depth = compiler_with(|limits| limits.max_nesting_depth = 2).compile_compact(
        "I(loops(k),externals(),dimension(d),prop(D1,(k)^2,1))",
        None,
    );
    assert!(matches!(
        depth,
        Err(Error::ResourceLimit {
            resource: "raw parser nesting depth",
            ..
        })
    ));

    let integer_compiler = compiler_with(|limits| limits.max_raw_integer_digits = 2);
    let integer = parse_base(&integer_compiler, "123");
    assert!(matches!(
        integer,
        Err(Error::ResourceLimit {
            resource: "raw integer digits",
            ..
        })
    ));

    let separated_compiler = compiler_with(|limits| limits.max_raw_integer_digits = 2);
    let separated_integer = parse_base(&separated_compiler, "1_2_3");
    assert!(matches!(
        separated_integer,
        Err(Error::ResourceLimit {
            resource: "raw integer digits",
            ..
        })
    ));

    let whitespace_compiler = compiler_with(|limits| limits.max_atom_nodes = 5);
    let parser_whitespace = parse_base(&whitespace_compiler, "a\\b\\c");
    assert!(matches!(
        parser_whitespace,
        Err(Error::ResourceLimit {
            resource: "raw lexical tokens",
            ..
        })
    ));

    let unary_compiler = compiler_with(|limits| limits.max_nesting_depth = 2);
    let unary_depth = parse_base(&unary_compiler, "-/-/x");
    assert!(matches!(
        unary_depth,
        Err(Error::ResourceLimit {
            resource: "raw parser nesting depth",
            ..
        })
    ));

    let power_boundary = compiler_with(|limits| limits.max_abs_power = 4);
    let _ = parse_base(&power_boundary, "x^4")
        .expect("the exact raw power boundary should be accepted");
    let power_compiler = compiler_with(|limits| limits.max_abs_power = 4);
    let power = parse_base(&power_compiler, "x^999999999");
    assert!(matches!(
        power,
        Err(Error::ResourceLimit {
            resource: "raw absolute power",
            ..
        })
    ));
    let symbolic_compiler = compiler();
    let symbolic_power = parse_base(&symbolic_compiler, "x^(a+1)");
    assert!(matches!(
        symbolic_power,
        Err(Error::UnsupportedToken { .. })
    ));

    let identifiers_compiler = compiler_with(|limits| limits.max_unique_identifiers = 2);
    let identifiers = parse_base(&identifiers_compiler, "a+b+c");
    assert!(matches!(
        identifiers,
        Err(Error::ResourceLimit {
            resource: "unique raw Symbolica identifiers",
            ..
        })
    ));
}

#[test]
fn ordinary_input_ignores_diagnostic_count_templates_and_keeps_ambient_resources() {
    assert_eq!(Limits::default().max_input_bytes, usize::MAX);
    assert_eq!(
        super::super::SymbolicaAffineDenominatorLimits::default().max_input_bytes,
        usize::MAX
    );
    let bounded = Compiler::new(Limits {
        max_input_bytes: 0,
        ..Limits::default()
    })
    .unwrap();
    assert!(matches!(
        bounded.compile_compact(&one_loop_source(1, "1"), None),
        Err(Error::ResourceLimit { .. })
    ));
    let limits = Limits {
        max_atom_nodes: 0,
        max_abs_power: 0,
        ..Limits::default()
    };
    let ordinary = Compiler::new(limits).unwrap();
    ordinary
        .compile_compact(&one_loop_source(1, "1"), None)
        .unwrap();
    let diagnostic = Compiler::new(limits.with_diagnostics()).unwrap();
    assert!(matches!(
        diagnostic.compile_compact(&one_loop_source(1, "1"), None),
        Err(Error::ResourceLimit { .. })
    ));
    let budget = symbolica::utils::resources::ResourceBudget::new(
        symbolica::utils::resources::ResourceLimits {
            max_elapsed: Some(std::time::Duration::ZERO),
            ..Default::default()
        },
    );
    assert!(matches!(
        budget.run(|| ordinary.compile_compact(&one_loop_source(1, "1"), None)),
        Err(Error::Resource(
            symbolica::utils::resources::ResourceLimitError::Elapsed { .. }
        ))
    ));
}

#[test]
fn ordinary_numeric_preconversion_does_not_reject_tiny_exact_power_via_estimate() {
    let plain = compiler();
    let value = parse_base(&plain, &format!("1^{}", i64::MAX)).unwrap();
    assert!(value.is_one());
}

#[test]
fn resource_errors_remain_discoverable_through_typed_input_wrappers() {
    use std::error::Error as _;
    use std::time::Duration;

    use crate::algebra::ExactAlgebraError;
    use crate::family::IntegralFamilyError;
    use crate::family::presentation::FamilyPresentationError;
    use symbolica::utils::resources::ResourceLimitError;

    let resource = ResourceLimitError::Elapsed {
        elapsed: Duration::from_secs(2),
        limit: Duration::from_secs(1),
    };
    let affine = SymbolicaAffineDenominatorError::ExactAlgebra(ExactAlgebraError::Resource(
        resource.clone(),
    ));
    let family = IntegralFamilyError::ExactAlgebra(ExactAlgebraError::Resource(resource.clone()));
    let presentation =
        FamilyPresentationError::ExactAlgebra(ExactAlgebraError::Resource(resource.clone()));
    let errors: Vec<Box<dyn std::error::Error>> = vec![
        Box::new(Error::Resource(resource.clone())),
        Box::new(LoweringError::Resource(resource.clone())),
        Box::new(LoweringError::AffineDenominator(affine)),
        Box::new(LoweringError::IntegralFamily(family)),
        Box::new(presentation),
    ];
    for error in &errors {
        let mut current = error.source();
        let mut found = None;
        while let Some(source) = current {
            if let Some(resource) = source.downcast_ref::<ResourceLimitError>() {
                found = Some(resource);
                break;
            }
            current = source.source();
        }
        assert_eq!(found, Some(&resource));
    }
    assert!(Error::WrongRoot.source().is_none());
}
