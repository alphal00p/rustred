#[test]
fn every_chart_lift_resource_limit_is_tight_at_preflight() {
    let context = context("ordinary-chart-lift-limits", 2);
    let relation = build_mixed_relation(&context, false);
    let default = OrdinaryChartLiftLimits::default();
    let ordering = ordering(&[true, false], default.involutive);

    let cases = [
        (
            OrdinaryChartLiftLimits {
                max_source_rows: 0,
                ..default
            },
            "ordinary chart-lift source rows",
            1,
            0,
        ),
        (
            OrdinaryChartLiftLimits {
                max_input_terms: 1,
                ..default
            },
            "ordinary chart-lift input terms",
            2,
            1,
        ),
        (
            OrdinaryChartLiftLimits {
                max_input_conditions: 0,
                ..default
            },
            "ordinary chart-lift input conditions",
            1,
            0,
        ),
        (
            OrdinaryChartLiftLimits {
                max_input_guard_terms: 1,
                ..default
            },
            "ordinary chart-lift input guard terms",
            2,
            1,
        ),
        (
            OrdinaryChartLiftLimits {
                max_input_guard_exponent_cells: 3,
                ..default
            },
            "ordinary chart-lift input guard exponent cells",
            4,
            3,
        ),
        (
            OrdinaryChartLiftLimits {
                max_input_symbolic_terms: 8,
                ..default
            },
            "ordinary chart-lift input symbolic terms",
            9,
            8,
        ),
        (
            OrdinaryChartLiftLimits {
                max_input_symbolic_exponent_cells: 17,
                ..default
            },
            "ordinary chart-lift input symbolic exponent cells",
            18,
            17,
        ),
        (
            OrdinaryChartLiftLimits {
                max_input_coordinate_cells: 3,
                ..default
            },
            "ordinary chart-lift input coordinate cells",
            4,
            3,
        ),
        (
            OrdinaryChartLiftLimits {
                max_lifted_coordinate_cells: 5,
                ..default
            },
            "ordinary chart-lift retained coordinate cells",
            6,
            5,
        ),
        (
            OrdinaryChartLiftLimits {
                max_coefficient_translations: 1,
                ..default
            },
            "ordinary chart-lift coefficient translations",
            2,
            1,
        ),
        (
            OrdinaryChartLiftLimits {
                max_chart_conversion_work: 17,
                ..default
            },
            "ordinary chart-lift conversion work",
            18,
            17,
        ),
    ];
    for (limits, resource, requested, limit) in cases {
        assert_eq!(
            lift_relation(&relation, 0, &ordering, &context, limits),
            Err(OrdinaryChartLiftError::Involutive(
                InvolutiveError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                }
            ))
        );
    }

    for (limits, resource) in [
        (
            OrdinaryChartLiftLimits {
                max_input_guard_retained_bytes: 0,
                ..default
            },
            "ordinary chart-lift input guard retained bytes",
        ),
        (
            OrdinaryChartLiftLimits {
                max_input_symbolic_retained_bytes: 0,
                ..default
            },
            "ordinary chart-lift input symbolic retained bytes",
        ),
    ] {
        assert!(matches!(
            lift_relation(&relation, 0, &ordering, &context, limits),
            Err(OrdinaryChartLiftError::Involutive(
                InvolutiveError::ResourceLimit {
                    resource: actual,
                    requested,
                    limit: 0,
                }
            )) if actual == resource && requested > 0
        ));
    }

    let mut nested = default;
    nested.involutive.max_arity = 1;
    assert_eq!(
        lift_relation(&relation, 0, &ordering, &context, nested),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ResourceLimit {
                resource: "Ore arity",
                requested: 2,
                limit: 1,
            }
        ))
    );

    let mut nested = default;
    nested.involutive.max_axpy_input_terms = 1;
    assert_eq!(
        lift_relation(&relation, 0, &ordering, &context, nested),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ResourceLimit {
                resource: "Ore AXPY input terms",
                requested: 2,
                limit: 1,
            }
        ))
    );

    let mut nested = default;
    nested.involutive.max_provenance_terms = 0;
    assert_eq!(
        lift_relation(&relation, 0, &ordering, &context, nested),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ResourceLimit {
                resource: "Ore provenance terms",
                requested: 1,
                limit: 0,
            }
        ))
    );

    for (nested, resource) in [
        (
            InvolutiveLimits {
                max_localization_guards: 0,
                ..default.involutive
            },
            "Ore localization guards",
        ),
        (
            InvolutiveLimits {
                max_localization_guard_terms: 0,
                ..default.involutive
            },
            "Ore localization guard terms",
        ),
        (
            InvolutiveLimits {
                max_localization_guard_exponent_cells: 0,
                ..default.involutive
            },
            "Ore localization guard exponent cells",
        ),
        (
            InvolutiveLimits {
                max_localization_guard_retained_bytes: 0,
                ..default.involutive
            },
            "Ore localization guard retained bytes",
        ),
    ] {
        let limits = OrdinaryChartLiftLimits {
            involutive: nested,
            ..default
        };
        assert!(matches!(
            lift_relation(&relation, 0, &ordering, &context, limits),
            Err(OrdinaryChartLiftError::Involutive(
                InvolutiveError::ResourceLimit {
                    resource: actual,
                    requested,
                    limit: 0,
                }
            )) if actual == resource && requested > 0
        ));
    }

    let mut coordinate = default;
    coordinate.involutive.max_shift_coordinate = 3;
    assert_eq!(
        lift_relation(&relation, 0, &ordering, &context, coordinate),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ShiftCoordinateLimit {
                position: 1,
                requested: 4,
                limit: 3,
            }
        ))
    );

    let mut degree = default;
    degree.involutive.max_total_shift_degree = 6;
    assert_eq!(
        lift_relation(&relation, 0, &ordering, &context, degree),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ResourceLimit {
                resource: "ordinary chart-lift output degree",
                requested: 7,
                limit: 6,
            }
        ))
    );
}

#[test]
fn public_batch_authenticates_every_coefficient_before_aggregate_admission() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let default = OrdinaryChartLiftLimits::default();
    let adapter = completed_ordering(&[true, false, true], &completed, default.involutive);
    let metrics = input_symbolic_metrics(completed.relations());
    assert!(completed.relations().len() > 1);
    assert!(metrics.terms > metrics.max_row_terms);
    assert!(metrics.exponent_cells > metrics.max_row_exponent_cells);
    assert!(metrics.retained_bytes > metrics.max_row_retained_bytes);

    let limits = OrdinaryChartLiftLimits {
        max_input_symbolic_terms: metrics.terms - 1,
        ..default
    };
    let scans_before = generator.context().authentication_scan_counts();
    assert_eq!(
        try_lift_completed_ordinary_sources(&completed, &adapter, generator.context(), limits,),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ResourceLimit {
                resource: "ordinary chart-lift input symbolic terms",
                requested: metrics.terms,
                limit: metrics.terms - 1,
            }
        ))
    );
    let scans_after = generator.context().authentication_scan_counts();
    let coefficients = completed
        .relations()
        .iter()
        .map(|relation| relation.terms().len())
        .sum::<usize>();
    assert_eq!(scans_after.0 - scans_before.0, coefficients);
    // Native-result authentication is the observable translation boundary;
    // this test deliberately makes no allocator-instrumentation claim.
    assert_eq!(scans_after.1, scans_before.1);

    let exponent_limits = OrdinaryChartLiftLimits {
        max_input_symbolic_exponent_cells: metrics.exponent_cells - 1,
        ..default
    };
    assert_eq!(
        try_lift_completed_ordinary_sources(
            &completed,
            &adapter,
            generator.context(),
            exponent_limits,
        ),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ResourceLimit {
                resource: "ordinary chart-lift input symbolic exponent cells",
                requested: metrics.exponent_cells,
                limit: metrics.exponent_cells - 1,
            }
        ))
    );

    let byte_limits = OrdinaryChartLiftLimits {
        max_input_symbolic_retained_bytes: metrics.retained_bytes - 1,
        ..default
    };
    assert_eq!(
        try_lift_completed_ordinary_sources(&completed, &adapter, generator.context(), byte_limits,),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ResourceLimit {
                resource: "ordinary chart-lift input symbolic retained bytes",
                requested: metrics.retained_bytes,
                limit: metrics.retained_bytes - 1,
            }
        ))
    );
    assert_eq!(
        generator.context().authentication_scan_counts().1,
        scans_before.1
    );
}

#[test]
fn public_batch_rejects_one_below_nested_symbolic_ingress_limits() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let default = OrdinaryChartLiftLimits::default();
    let adapter = completed_ordering(&[true, false, true], &completed, default.involutive);
    let metrics = input_symbolic_metrics(completed.relations());
    assert!(metrics.max_polynomial_terms > 0);
    assert!(metrics.max_exponent > 0);
    assert!(metrics.max_integer_bits > 0);
    let authenticated_results_before = generator.context().authentication_scan_counts().1;

    let mut term_limits = default;
    term_limits
        .involutive
        .indexed_algebra
        .exact_algebra
        .max_polynomial_terms = metrics.max_polynomial_terms - 1;
    assert!(matches!(
        try_lift_completed_ordinary_sources(
            &completed,
            &adapter,
            generator.context(),
            term_limits,
        ),
        Err(OrdinaryChartLiftError::Involutive(InvolutiveError::Algebra(
            IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                resource: "authenticated polynomial terms",
                requested,
                limit,
            })
        ))) if requested == metrics.max_polynomial_terms
            && limit == metrics.max_polynomial_terms - 1
    ));

    let mut exponent_limits = default;
    exponent_limits
        .involutive
        .indexed_algebra
        .exact_algebra
        .max_exponent = metrics.max_exponent - 1;
    assert!(matches!(
        try_lift_completed_ordinary_sources(
            &completed,
            &adapter,
            generator.context(),
            exponent_limits,
        ),
        Err(OrdinaryChartLiftError::Involutive(InvolutiveError::Algebra(
            IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::ExponentLimit {
                operation: ExactAlgebraOperation::Authenticate,
                requested,
                limit,
                ..
            })
        ))) if requested == u64::from(metrics.max_exponent)
            && limit == metrics.max_exponent - 1
    ));

    let mut integer_limits = default;
    integer_limits
        .involutive
        .indexed_algebra
        .max_specialization_integer_bits = metrics.max_integer_bits - 1;
    assert_eq!(
        try_lift_completed_ordinary_sources(
            &completed,
            &adapter,
            generator.context(),
            integer_limits,
        ),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ResourceLimit {
                resource: "ordinary chart-lift input integer bits",
                requested: metrics.max_integer_bits,
                limit: metrics.max_integer_bits - 1,
            }
        ))
    );
    assert_eq!(
        generator.context().authentication_scan_counts().1,
        authenticated_results_before
    );
}

#[test]
fn zero_rows_wrong_chart_arity_and_unrepresentable_left_actions_are_rejected() {
    let limits = OrdinaryChartLiftLimits::default();
    let two_context = context("ordinary-chart-lift-boundaries", 2);
    let empty = RelationBuilder::new(
        Arc::new("ordinary-chart-lift-test-family".to_owned()),
        RowId::Derived {
            label: Arc::from("zero-source"),
        },
        &two_context,
    )
    .finish();
    let correct = ordering(&[true, false], limits.involutive);
    assert_eq!(
        lift_relation(&empty, 5, &correct, &two_context, limits),
        Err(OrdinaryChartLiftError::EmptySourceRelation { source_ordinal: 5 })
    );

    let relation = build_mixed_relation(&two_context, false);
    let wrong_arity = ordering(&[true], limits.involutive);
    assert_eq!(
        lift_relation(&relation, 0, &wrong_arity, &two_context, limits),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::WrongArity {
                object: "ordinary source integral shift",
                expected: 1,
                actual: 2,
            }
        ))
    );

    let one = context("ordinary-chart-lift-i64-min", 1);
    let mut builder = RelationBuilder::new(
        Arc::new("ordinary-chart-lift-test-family".to_owned()),
        RowId::Derived {
            label: Arc::from("i64-min-source"),
        },
        &one,
    );
    builder
        .add_term(
            &one,
            IndexShift::try_new([i64::MIN], 1).unwrap(),
            one.one(),
            RelationLimits::default(),
        )
        .unwrap();
    let endpoint = builder.finish();
    let mut endpoint_limits = OrdinaryChartLiftLimits::default();
    endpoint_limits.involutive.max_shift_coordinate = u64::MAX;
    endpoint_limits.involutive.max_total_shift_degree = usize::MAX;
    let active = ordering(&[true], endpoint_limits.involutive);
    assert_eq!(
        lift_relation(&endpoint, 0, &active, &one, endpoint_limits),
        Err(OrdinaryChartLiftError::Involutive(
            InvolutiveError::ShiftCoordinateNotRepresentable {
                position: 0,
                coordinate: 1_u64 << 63,
            }
        ))
    );
}
