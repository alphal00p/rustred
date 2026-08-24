use rustred::{
    AffineDenominator, CoefficientContext, ConcreteIntegralKey, ExactAlgebraError,
    GENERIC_TENSOR_FAMILY_LOWERING_V1_SCHEMA, GenericFamilyError, GenericScalarProductMonomial,
    GenericTensorFamilyError, GenericTensorFamilyLimits, GenericTensorFamilyReducer,
    GenericTensorNumerator, GenericTensorTerm, IndexedVector, IntegralFamily, LoopVector,
    LorentzIndex, Metric, MetricPairing, ScalarProductCoordinate, TensorLoweringGuardOrigin,
    TensorMonomial, VacuumTensorProjector,
};

fn one_loop_two_point_family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2", "s"]);
    let zero = context.zero();
    let one = context.one();
    let two = context.integer(2);
    IntegralFamily::new(
        "one-loop-two-point-tensor-test",
        vec!["k".to_owned()],
        vec!["p".to_owned()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            // D0 = k^2 - m2
            AffineDenominator::new(
                context.parse("-m2").unwrap(),
                vec![one.clone(), zero.clone()],
            ),
            // D1 = (k+p)^2 - m2 = k^2 + 2 k.p + s - m2
            AffineDenominator::new(context.parse("s-m2").unwrap(), vec![one, two]),
        ],
        vec![vec![context.parameter("s").unwrap()]],
        vec![zero.clone(), zero],
    )
    .unwrap()
}

fn one_loop_vacuum_family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "one-loop-vacuum-tensor-test",
        vec!["k".to_owned()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.parse("-m2").unwrap(),
            vec![context.one()],
        )],
        vec![],
        vec![context.zero()],
    )
    .unwrap()
}

fn key(powers: impl IntoIterator<Item = i64>) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

#[test]
fn generic_one_loop_loop_loop_and_loop_external_numerator_powers_lower_exactly() {
    let symmetric = GenericScalarProductMonomial::try_from_factors([
        (ScalarProductCoordinate::LoopLoop { left: 1, right: 0 }, 1),
        (ScalarProductCoordinate::LoopLoop { left: 0, right: 1 }, 2),
    ])
    .unwrap();
    assert_eq!(symmetric.factors().len(), 1);
    assert_eq!(
        symmetric.exponent(ScalarProductCoordinate::LoopLoop { left: 1, right: 0 }),
        3
    );

    let family = one_loop_two_point_family();
    let context = family.coefficient_context();
    let loop_loop = ScalarProductCoordinate::LoopLoop { left: 0, right: 0 };
    let loop_external = ScalarProductCoordinate::LoopExternal {
        loop_index: 0,
        external_index: 0,
    };
    let external_metrics =
        MetricPairing::new([Metric::new(LorentzIndex::new(10), LorentzIndex::new(11))]);
    let numerator = GenericTensorNumerator::try_new([
        GenericTensorTerm::new(
            context.one(),
            MetricPairing::empty(),
            GenericScalarProductMonomial::try_from_factors([(loop_loop, 2)]).unwrap(),
        ),
        GenericTensorTerm::new(
            context.one(),
            external_metrics.clone(),
            GenericScalarProductMonomial::try_from_factors([(loop_external, 2)]).unwrap(),
        ),
    ])
    .unwrap();
    let base = key([1, 1]);
    let lowered = GenericTensorFamilyReducer::new(&family)
        .lower(&base, &numerator)
        .unwrap();

    assert_eq!(lowered.schema(), GENERIC_TENSOR_FAMILY_LOWERING_V1_SCHEMA);
    assert_eq!(lowered.family_fingerprint(), family.fingerprint());
    assert_eq!(lowered.family_domain(), family.domain());
    assert_eq!(lowered.domain().family(), family.domain());
    assert_eq!(lowered.base_integral(), &base);
    assert_eq!(lowered.source_numerator(), &numerator);

    // k^4 = (D0 + m2)^2.
    for (powers, expected) in [([-1, 1], "1"), ([0, 1], "2*m2"), ([1, 1], "m2^2")] {
        let term = lowered.term(&MetricPairing::empty(), &key(powers)).unwrap();
        assert_eq!(term.coefficient(), &context.parse(expected).unwrap());
        assert_eq!(term.origins().len(), 1);
        let origin = term.origins().first().unwrap();
        assert_eq!(origin.input_term(), 0);
        assert_eq!(origin.scalar_products().exponent(loop_loop), 2);
    }

    // k.p = (-D0 + D1 - s)/2.  This explicitly exercises the generic
    // loop--external coordinate path, not the vacuum-projector adapter.
    for (powers, expected) in [
        ([-1, 1], "1/4"),
        ([1, -1], "1/4"),
        ([0, 0], "-1/2"),
        ([0, 1], "s/2"),
        ([1, 0], "-s/2"),
        ([1, 1], "s^2/4"),
    ] {
        let term = lowered.term(&external_metrics, &key(powers)).unwrap();
        assert_eq!(term.coefficient(), &context.parse(expected).unwrap());
        let origin = term.origins().first().unwrap();
        assert_eq!(origin.input_term(), 1);
        assert_eq!(origin.scalar_products().exponent(loop_external), 2);
    }
    assert_eq!(lowered.len(), 9);
}

#[test]
fn one_loop_vacuum_projector_composes_with_generic_concrete_keys() {
    let family = one_loop_vacuum_family();
    let context = family.coefficient_context();
    let mut projector = VacuumTensorProjector::with_dimension(context, family.dimension().clone());
    let base = key([1]);
    let metric = MetricPairing::new([Metric::new(LorentzIndex::new(20), LorentzIndex::new(21))]);

    // k(mu) k(nu) -> g(mu,nu) k^2/d, followed by k^2 = D0 + m2.
    let rank_two = projector
        .reduce(&TensorMonomial::new([
            IndexedVector::new(LoopVector::new(0), LorentzIndex::new(20)),
            IndexedVector::new(LoopVector::new(0), LorentzIndex::new(21)),
        ]))
        .unwrap();
    let lowered = GenericTensorFamilyReducer::new(&family)
        .lower_vacuum_projection(&base, &rank_two)
        .unwrap();
    assert_eq!(lowered.coefficient_nonzero_conditions().len(), 1);
    let projector_guard = &lowered.coefficient_nonzero_conditions()[0];
    assert_eq!(
        projector_guard.polynomial(),
        &context.parameter("d").unwrap().numerator
    );
    assert!(
        projector_guard
            .origins()
            .contains(&TensorLoweringGuardOrigin::InputCoefficientDenominator { input_term: 0 })
    );
    lowered.verify(&family).unwrap();
    assert_eq!(
        lowered.term(&metric, &key([0])).unwrap().coefficient(),
        &context.parse("1/d").unwrap()
    );
    assert_eq!(
        lowered.term(&metric, &key([1])).unwrap().coefficient(),
        &context.parse("m2/d").unwrap()
    );

    // A rank-zero projected numerator with an existing (k^2)^3 monomial is
    // lowered without a topology-specific recurrence.
    let cubic_scalar = rustred::ScalarProductMonomial::from_factors([(
        rustred::ScalarProduct::new(LoopVector::new(0), LoopVector::new(0)),
        3,
    )]);
    let projected = projector
        .reduce(&TensorMonomial::from_parts([], [], cubic_scalar))
        .unwrap();
    let lowered = GenericTensorFamilyReducer::new(&family)
        .lower_vacuum_projection(&base, &projected)
        .unwrap();
    for (power, expected) in [(-2, "1"), (-1, "3*m2"), (0, "3*m2^2"), (1, "m2^3")] {
        assert_eq!(
            lowered
                .term(&MetricPairing::empty(), &key([power]))
                .unwrap()
                .coefficient(),
            &context.parse(expected).unwrap()
        );
    }
}

#[test]
fn generic_tensor_lowering_collects_source_provenance_and_is_bounded() {
    let family = one_loop_vacuum_family();
    let context = family.coefficient_context();
    let scalar = GenericScalarProductMonomial::try_from_factors([(
        ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
        1,
    )])
    .unwrap();
    let numerator = GenericTensorNumerator::try_new([
        GenericTensorTerm::new(context.one(), MetricPairing::empty(), scalar.clone()),
        GenericTensorTerm::new(context.integer(2), MetricPairing::empty(), scalar.clone()),
    ])
    .unwrap();
    let base = key([1]);
    let lowered = GenericTensorFamilyReducer::new(&family)
        .lower(&base, &numerator)
        .unwrap();
    let denominator_term = lowered.term(&MetricPairing::empty(), &key([0])).unwrap();
    assert_eq!(denominator_term.coefficient(), &context.integer(3));
    assert_eq!(
        denominator_term
            .origins()
            .iter()
            .map(|origin| origin.input_term())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );

    let mut limits = GenericTensorFamilyLimits::default();
    limits.max_expansion_operations = 0;
    assert!(matches!(
        GenericTensorFamilyReducer::with_limits(&family, limits).lower(&base, &numerator),
        Err(GenericTensorFamilyError::OperationLimit {
            attempted: 1,
            limit: 0
        })
    ));

    let overflow_numerator = GenericTensorNumerator::try_new([GenericTensorTerm::new(
        context.one(),
        MetricPairing::empty(),
        scalar,
    )])
    .unwrap();
    assert!(matches!(
        GenericTensorFamilyReducer::new(&family).lower(&key([i64::MIN]), &overflow_numerator),
        Err(GenericTensorFamilyError::IntegralPowerOverflow {
            denominator: 0,
            power: i64::MIN,
            numerator_power: 1,
        })
    ));

    let foreign_coordinate = GenericTensorNumerator::try_new([GenericTensorTerm::new(
        context.one(),
        MetricPairing::empty(),
        GenericScalarProductMonomial::try_from_factors([(
            ScalarProductCoordinate::LoopExternal {
                loop_index: 0,
                external_index: 0,
            },
            1,
        )])
        .unwrap(),
    )])
    .unwrap();
    assert!(matches!(
        GenericTensorFamilyReducer::new(&family).lower(&base, &foreign_coordinate),
        Err(GenericTensorFamilyError::Family(
            GenericFamilyError::ExternalMomentumOutOfRange {
                index: 0,
                externals: 0,
            }
        ))
    ));

    let foreign_context = CoefficientContext::new(["foreign_tensor_parameter"]);
    let foreign_coefficient = GenericTensorNumerator::try_new([GenericTensorTerm::new(
        foreign_context.one(),
        MetricPairing::empty(),
        GenericScalarProductMonomial::one(),
    )])
    .unwrap();
    assert!(matches!(
        GenericTensorFamilyReducer::new(&family).lower(&base, &foreign_coefficient),
        Err(GenericTensorFamilyError::InvalidInputCoefficient {
            input_term: 0,
            error: ExactAlgebraError::VariableMapMismatch { .. },
        })
    ));

    let mut degree_limits = GenericTensorFamilyLimits::default();
    degree_limits.max_scalar_product_degree = 0;
    assert!(matches!(
        GenericTensorFamilyReducer::with_limits(&family, degree_limits)
            .lower(&base, &overflow_numerator),
        Err(GenericTensorFamilyError::ScalarProductDegreeLimit {
            input_term: 0,
            requested: 1,
            limit: 0,
        })
    ));

    assert!(matches!(
        GenericScalarProductMonomial::try_from_factors_with_limits(
            [
                (ScalarProductCoordinate::LoopLoop { left: 0, right: 0 }, 1),
                (ScalarProductCoordinate::LoopLoop { left: 0, right: 0 }, 1),
            ],
            1,
            10,
        ),
        Err(GenericTensorFamilyError::ResourceLimit {
            resource: "tensor scalar-product factor entries",
            requested: 2,
            limit: 1,
        })
    ));
    assert!(matches!(
        GenericTensorNumerator::try_new_with_limit(
            [GenericTensorTerm::new(
                context.one(),
                MetricPairing::empty(),
                GenericScalarProductMonomial::one(),
            )],
            0,
        ),
        Err(GenericTensorFamilyError::ResourceLimit {
            resource: "tensor input terms",
            requested: 1,
            limit: 0,
        })
    ));

    let external_family = one_loop_two_point_family();
    let external_context = external_family.coefficient_context();
    let mut external_projector = VacuumTensorProjector::with_dimension(
        external_context,
        external_family.dimension().clone(),
    );
    let scalar_projection = external_projector
        .reduce(&TensorMonomial::default())
        .unwrap();
    assert!(matches!(
        GenericTensorFamilyReducer::new(&external_family)
            .lower_vacuum_projection(&key([1, 1]), &scalar_projection),
        Err(GenericTensorFamilyError::VacuumProjectionNeedsVacuumFamily { externals: 1 })
    ));
}

#[test]
fn expansion_exponent_entry_limit_rejects_the_next_monomial_at_its_boundary() {
    let family = one_loop_vacuum_family();
    let numerator = GenericTensorNumerator::try_new([GenericTensorTerm::new(
        family.coefficient_context().one(),
        MetricPairing::empty(),
        GenericScalarProductMonomial::try_from_factors([(
            ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
            1,
        )])
        .unwrap(),
    )])
    .unwrap();
    let mut limits = GenericTensorFamilyLimits::default();
    // The seed monomial has one denominator-exponent entry. Expanding
    // k^2 = D0 + m2 attempts to retain a second one, which must be rejected
    // at the insertion boundary rather than after growing the expansion map.
    limits.max_expansion_exponent_entries = 1;

    assert!(matches!(
        GenericTensorFamilyReducer::with_limits(&family, limits).lower(&key([1]), &numerator),
        Err(GenericTensorFamilyError::ResourceLimit {
            resource: "tensor expansion exponent entries",
            requested: 2,
            limit: 1,
        })
    ));
}
