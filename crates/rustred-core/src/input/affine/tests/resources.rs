use super::*;

#[test]
fn projection_denominator_replication_limit_has_exact_boundary() {
    let base = compiler(&["a"], &["k1", "k2"], &[], &[]);
    let expression = "(k1^2+k1*k2+k2^2)/(a+1)";
    let mut exact = base.test_limits();
    exact.max_projection_denominator_replication_terms = 6;
    let exact_compiler = base.test_with_limits(exact);
    exact_compiler.compile_expression(expression).unwrap();

    let mut below = exact;
    below.max_projection_denominator_replication_terms = 5;
    let below = base.test_with_limits(below);
    assert!(matches!(
        below.compile_expression(expression),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "projection denominator replication terms",
            requested: 6,
            limit: 5,
        })
    ));
}

#[test]
fn projection_group_and_retained_limits_precede_group_allocation() {
    let base = compiler(&["a"], &["k"], &[], &[]);
    let mut no_groups = base.test_limits();
    no_groups.max_projection_groups = 0;
    let no_groups = base.test_with_limits(no_groups);
    assert!(matches!(
        no_groups.compile_expression("k^2"),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "aggregate projection groups",
            requested: 1,
            limit: 0,
        })
    ));

    let mut no_projection_storage = base.test_limits();
    no_projection_storage.max_projected_retained_bytes = 0;
    let no_projection_storage = base.test_with_limits(no_projection_storage);
    assert!(matches!(
        no_projection_storage.compile_expression("k^2"),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "aggregate projected retained bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));
}

#[test]
fn componentwise_dense_degree_box_limit_has_exact_boundary() {
    let base = compiler(&["a", "b"], &["k"], &[], &[]);
    let expression = "(a+1)*(b+1)*k^2";
    let mut exact = base.test_limits();
    exact.max_dense_degree_box_terms = 12;
    let exact_compiler = base.test_with_limits(exact);
    exact_compiler.compile_expression(expression).unwrap();

    let mut below = exact;
    below.max_dense_degree_box_terms = 11;
    let below = base.test_with_limits(below);
    assert!(matches!(
        below.compile_expression(expression),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "dense numerator degree-box terms",
            requested: 12,
            limit: 11,
        })
    ));
}

#[test]
fn normalized_gmp_integer_limit_has_exact_boundary() {
    let base = compiler(&["a"], &["k"], &[], &[]);
    let normalized_expression = "(a+1)^16*sp(k,k)";
    let source = try_parse!(normalized_expression, default_namespace = RUSTRED_NAMESPACE).unwrap();
    let mut evaluator = CheckedEvaluator::new(&base);
    let evaluated = evaluator.evaluate(source.as_view(), true).unwrap();
    let normalized_bits = normalized_expression_census(&evaluated)
        .unwrap()
        .integer_bits;
    assert!(normalized_bits > 8);
    let mut normalized_exact = base.test_limits();
    normalized_exact.max_normalized_expression_integer_bits = normalized_bits;
    let normalized_exact_compiler = base.test_with_limits(normalized_exact);
    normalized_exact_compiler
        .compile_expression(normalized_expression)
        .unwrap();

    let mut below_normalized = normalized_exact;
    below_normalized.max_normalized_expression_integer_bits = normalized_bits - 1;
    let below_normalized = base.test_with_limits(below_normalized);
    assert!(matches!(
        below_normalized.compile_expression(normalized_expression),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "normalized expression integer bits",
            requested,
            limit,
        }) if requested == normalized_bits as u128 && limit + 1 == requested
    ));
}

#[test]
fn gcd_normalization_densification_is_bounded_before_add_mul_and_div() {
    let base = compiler(&["a", "b"], &["k"], &[], &[]);
    let cases = [
        (
            BinaryOperation::Add,
            "1/(a-1)",
            "(a^8-2)/(a-1)",
            8usize,
            9usize,
        ),
        (BinaryOperation::Multiply, "a^8-1", "1/(a-1)", 8, 9),
        (BinaryOperation::Divide, "a^8-1", "a-1", 8, 9),
        (BinaryOperation::Divide, "a^2*b^2-1", "a*b-1", 2, 9),
    ];
    for (operation, left, right, actual_terms, planned_terms) in cases {
        let left = base.combined.coefficient_fixture(left);
        let right = base.combined.coefficient_fixture(right);
        let allocation = exact_operation_allocation_envelope(
            &left,
            &right,
            operation,
            base.combined.parameter_names().len(),
        )
        .unwrap();
        assert!(allocation.numerator_terms >= planned_terms);

        let actual = checked_test_operation(&base, &left, &right, operation).unwrap();
        assert_eq!(actual.numerator.nterms(), actual_terms);
        assert!(actual.denominator.is_one());
        let actual_census = coefficient_census(&actual).unwrap();
        verify_operation_result_envelope(&actual, actual_census, allocation).unwrap();

        let mut exact = base.test_clone();
        exact.limits.max_combined_polynomial_terms =
            allocation.numerator_terms.max(allocation.denominator_terms);
        exact.limits.max_combined_exponent_entries = allocation.census.exponent_entries;
        exact.limits.max_coefficient_integer_bits = allocation.census.integer_bits;
        exact.limits.max_combined_retained_bytes = allocation.census.retained_bytes;
        checked_test_operation(&exact, &left, &right, operation).unwrap();

        let mut below_support = base.test_clone();
        below_support.limits.max_combined_polynomial_terms = allocation.numerator_terms - 1;
        assert!(matches!(
            checked_test_operation(&below_support, &left, &right, operation),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "combined exact-operation numerator term envelope",
                requested,
                limit,
            }) if requested == allocation.numerator_terms as u128 && limit + 1 == requested
        ));

        let mut below_integer = base.test_clone();
        below_integer.limits.max_coefficient_integer_bits = allocation.census.integer_bits - 1;
        assert!(matches!(
            checked_test_operation(&below_integer, &left, &right, operation),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "combined exact-operation integer bits",
                requested,
                limit,
            }) if requested == allocation.census.integer_bits as u128 && limit + 1 == requested
        ));

        let mut below_storage = base.test_clone();
        below_storage.limits.max_combined_retained_bytes = allocation.census.retained_bytes - 1;
        assert!(matches!(
            checked_test_operation(&below_storage, &left, &right, operation),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "combined exact-operation retained bytes",
                requested,
                limit,
            }) if requested == allocation.census.retained_bytes as u128 && limit + 1 == requested
        ));
    }
}

#[test]
fn combined_integer_and_storage_envelopes_are_preoperation() {
    let base = compiler(&["a"], &["k"], &[], &[]);
    let expanded = base.compile_expression("(a+1)^256*k^2").unwrap();
    assert_eq!(
        expanded.affine_denominator().coefficients()[0]
            .numerator
            .nterms(),
        257
    );

    let half_power = base.combined.coefficient_fixture("(a+1)^128");
    let power_step = exact_operation_allocation_envelope(
        &half_power,
        &half_power,
        BinaryOperation::Multiply,
        base.combined.parameter_names().len(),
    )
    .unwrap();
    assert_eq!(power_step.numerator_terms, 257);
    let squared =
        checked_test_operation(&base, &half_power, &half_power, BinaryOperation::Multiply).unwrap();
    assert_eq!(squared.numerator.nterms(), 257);
    let mut exact_power_step = base.test_clone();
    exact_power_step.limits.max_combined_polynomial_terms = 257;
    exact_power_step.limits.max_combined_exponent_entries = power_step.census.exponent_entries;
    exact_power_step.limits.max_coefficient_integer_bits = power_step.census.integer_bits;
    exact_power_step.limits.max_combined_retained_bytes = power_step.census.retained_bytes;
    checked_test_operation(
        &exact_power_step,
        &half_power,
        &half_power,
        BinaryOperation::Multiply,
    )
    .unwrap();

    let mut integer_limits = base.test_limits();
    integer_limits.max_coefficient_integer_bits = 128;
    let integer_bounded = base.test_with_limits(integer_limits);
    assert!(matches!(
        integer_bounded.compile_expression("(a+1)^256*k^2"),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "combined exact-operation integer bits",
            requested,
            limit: 128,
        }) if requested > 128
    ));

    let mut support_bounded = base.test_clone();
    support_bounded.limits.max_combined_polynomial_terms = 256;
    assert!(matches!(
        support_bounded.compile_expression("(a+1)^256*k^2"),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "combined exact-operation numerator term envelope",
            requested: 257,
            limit: 256,
        })
    ));

    let left = base.combined.parameter_at(0);
    let right = base.combined.parameter_at(0);
    let allocation = exact_operation_allocation_envelope(
        &left,
        &right,
        BinaryOperation::Multiply,
        base.combined.parameter_names().len(),
    )
    .unwrap();
    let mut storage_bounded = base.test_clone();
    storage_bounded.limits.max_combined_retained_bytes = allocation.census.retained_bytes - 1;
    let mut work = ExactWorkBudget::default();
    assert!(matches!(
        storage_bounded.checked_mul(&left, &right, &mut work),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "combined exact-operation retained bytes",
            requested,
            limit,
        }) if requested == allocation.census.retained_bytes as u128 && limit + 1 == requested
    ));

    let one = base.combined.one();
    let mut no_storage = base.test_clone();
    no_storage.limits.max_combined_retained_bytes = 0;
    assert!(matches!(
        checked_test_operation(&no_storage, &one, &one, BinaryOperation::Multiply),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "combined exact-operation retained bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));
    let mut deterministic = no_storage;
    deterministic.limits.max_coefficient_integer_bits = 0;
    assert!(matches!(
        checked_test_operation(&deterministic, &one, &one, BinaryOperation::Multiply),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "combined exact-operation integer bits",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let large_base = base.combined.coefficient_fixture("(a+1)^16");
    let unit = planned_unit_coefficient_census(base.combined.parameter_names().len()).unwrap();
    let mut zero_power_compiler = base.test_clone();
    zero_power_compiler.limits.max_coefficient_integer_bits = unit.integer_bits;
    zero_power_compiler.limits.max_combined_retained_bytes = unit.retained_bytes;
    let mut evaluator = CheckedEvaluator::new(&zero_power_compiler);
    assert_eq!(
        evaluator.checked_power(&large_base, 0).unwrap(),
        zero_power_compiler.combined.one()
    );
    let mut zero_power_rejected = base.test_clone();
    zero_power_rejected.limits.max_coefficient_integer_bits = 0;
    zero_power_rejected.limits.max_combined_retained_bytes = 0;
    let mut evaluator = CheckedEvaluator::new(&zero_power_rejected);
    assert!(matches!(
        evaluator.checked_power(&large_base, 0),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "combined power-result integer bits",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let overflowing_base = base.combined.coefficient_fixture("a^40000");
    let mut evaluator = CheckedEvaluator::new(&base);
    assert!(matches!(
        evaluator.checked_power(&overflowing_base, 2),
        Err(SymbolicaAffineDenominatorError::ExactAlgebra(
            ExactAlgebraError::ExponentLimit {
                operation: crate::algebra::ExactAlgebraOperation::Power,
                variable: 0,
                requested: 80_000,
                limit: crate::algebra::SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            }
        ))
    ));
    assert_eq!(evaluator.arithmetic_operations, 0);
}

#[test]
fn projection_coordinate_and_gmp_denominator_storage_boundaries_are_exact() {
    let compiler = compiler(&["a"], &["k"], &[], &[]);
    let zero = compiler.test_coefficient_context().zero();
    let coordinate_baseline = multiply_census(
        coefficient_census(&zero).unwrap(),
        2,
        "test coordinate baseline",
    )
    .unwrap();
    let mut exact_limits = compiler.test_limits();
    exact_limits.max_projected_retained_bytes = coordinate_baseline.retained_bytes;
    let mut exact_budget = ProjectionAllocationBudget::default();
    exact_budget
        .charge(
            coordinate_baseline,
            exact_limits,
            "test coordinate baseline terms",
        )
        .unwrap();
    let mut below_limits = exact_limits;
    below_limits.max_projected_retained_bytes = coordinate_baseline.retained_bytes - 1;
    let mut below_budget = ProjectionAllocationBudget::default();
    assert!(matches!(
        below_budget.charge(
            coordinate_baseline,
            below_limits,
            "test coordinate baseline terms"
        ),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "aggregate projected retained bytes",
            ..
        })
    ));

    let large = compiler
        .test_coefficient_context()
        .coefficient_fixture("1/(12345678901234567890123456789012345678901234567890*a+1)");
    assert!(
        large
            .denominator
            .coefficients
            .iter()
            .any(|integer| matches!(integer, Integer::Large(_)))
    );
    let denominator_replication = multiply_census(
        polynomial_census(&large.denominator).unwrap(),
        3,
        "test denominator replication",
    )
    .unwrap();
    let mut denominator_limits = compiler.test_limits();
    denominator_limits.max_projected_retained_bytes = denominator_replication.retained_bytes;
    let mut denominator_budget = ProjectionAllocationBudget::default();
    denominator_budget
        .charge(
            denominator_replication,
            denominator_limits,
            "test denominator replication terms",
        )
        .unwrap();
    denominator_limits.max_projected_retained_bytes -= 1;
    let mut below_denominator = ProjectionAllocationBudget::default();
    assert!(matches!(
        below_denominator.charge(
            denominator_replication,
            denominator_limits,
            "test denominator replication terms"
        ),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "aggregate projected retained bytes",
            ..
        })
    ));
}

#[test]
fn normalized_render_byte_preflight_has_exact_boundary() {
    let base = compiler(&["a"], &["k"], &[], &[]);
    let source = try_parse!("(a+1)*k^2", default_namespace = RUSTRED_NAMESPACE).unwrap();
    let mut evaluator = CheckedEvaluator::new(&base);
    let evaluated = evaluator.evaluate(source.as_view(), true).unwrap();
    let census = normalized_expression_census(&evaluated).unwrap();
    let maximum_symbol_bytes = maximum_combined_symbol_bytes(&base.combined).unwrap();
    let bound = normalized_expression_render_byte_bound(census, maximum_symbol_bytes).unwrap();
    let mut exact = base.test_limits();
    exact.max_normalized_expression_bytes = bound;
    let exact_compiler = base.test_with_limits(exact);
    exact_compiler.compile_expression("(a+1)*k^2").unwrap();
    let mut below = exact;
    below.max_normalized_expression_bytes = bound - 1;
    let below = base.test_with_limits(below);
    assert!(matches!(
        below.compile_expression("(a+1)*k^2"),
        Err(SymbolicaAffineDenominatorError::NormalizedExpressionTooLarge {
            requested,
            limit,
        }) if requested == bound && limit + 1 == requested
    ));
}

#[test]
fn complete_compiled_retained_bound_has_exact_boundary() {
    let base = compiler(&["a"], &["k"], &[], &[]);
    let expression = "(a+1)*k^2";
    let baseline = base.compile_expression(expression).unwrap();
    let mut projected = coefficient_census(baseline.affine_denominator().constant()).unwrap();
    for coefficient in baseline.affine_denominator().coefficients() {
        projected
            .checked_add_assign(
                coefficient_census(coefficient).unwrap(),
                "test affine census",
            )
            .unwrap();
    }
    let variable_maps = retained_variable_map_arc_bytes(
        std::iter::once(baseline.affine_denominator().constant())
            .chain(baseline.affine_denominator().coefficients()),
    )
    .unwrap();
    let retained = compiled_retained_byte_bound(
        baseline.source().as_view().get_byte_size(),
        baseline.normalized_expression().as_view().get_byte_size(),
        projected.retained_bytes,
        variable_maps,
    )
    .unwrap();
    assert!(retained > std::mem::size_of::<CompiledSymbolicaAffineDenominator>());

    let mut exact = base.test_limits();
    exact.max_compiled_retained_bytes = retained;
    base.test_with_limits(exact)
        .compile_expression(expression)
        .unwrap();

    let mut below = exact;
    below.max_compiled_retained_bytes = retained - 1;
    let below = base.test_with_limits(below);
    assert!(matches!(
        below.compile_expression(expression),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "compiled retained bytes",
            requested,
            limit,
        }) if requested == retained as u128 && limit + 1 == requested
    ));

    let mut zero = base.test_limits();
    zero.max_compiled_retained_bytes = 0;
    let zero = base.test_with_limits(zero);
    assert!(matches!(
        zero.compile_expression(expression),
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource: "compiled fixed retained bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));
}

#[test]
fn retained_variable_maps_are_charged_once_per_distinct_arc() {
    let first_context = CoefficientContext::new(["a"]);
    let second_context = CoefficientContext::new(["a"]);
    let first = first_context.coefficient_fixture("a+1");
    let second = second_context.coefficient_fixture("a+1");
    let one = retained_variable_map_arc_bytes([&first]).unwrap();
    assert!(one > 0);
    assert_eq!(
        retained_variable_map_arc_bytes([&first, &first]).unwrap(),
        one
    );
    assert_eq!(
        retained_variable_map_arc_bytes([&first, &second]).unwrap(),
        one.checked_mul(2).unwrap()
    );
}
