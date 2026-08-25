use rustred::*;

fn vacuum_family(name: &str, loops: usize) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    let scalar_products = loops * (loops + 1) / 2;
    let mut denominators = Vec::with_capacity(scalar_products);
    for coordinate in 0..scalar_products {
        let mut coefficients = vec![context.zero(); scalar_products];
        coefficients[coordinate] = context.one();
        denominators.push(AffineDenominator::new(
            if coordinate == 0 {
                context.parse("-m2").unwrap()
            } else {
                context.zero()
            },
            coefficients,
        ));
    }
    IntegralFamily::new(
        name,
        (0..loops).map(|loop_id| format!("k{loop_id}")).collect(),
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        vec![],
        vec![context.zero(); scalar_products],
    )
    .unwrap()
}

fn vacuum_family_at_integer_dimension(name: &str, loops: usize, dimension: i64) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    let scalar_products = loops * (loops + 1) / 2;
    let denominators = (0..scalar_products)
        .map(|coordinate| {
            let mut coefficients = vec![context.zero(); scalar_products];
            coefficients[coordinate] = context.one();
            AffineDenominator::new(
                if coordinate == 0 {
                    context.parse("-m2").unwrap()
                } else {
                    context.zero()
                },
                coefficients,
            )
        })
        .collect();
    IntegralFamily::new(
        name,
        (0..loops).map(|loop_id| format!("k{loop_id}")).collect(),
        vec![],
        context.clone(),
        context.integer(dimension),
        denominators,
        vec![],
        vec![context.zero(); scalar_products],
    )
    .unwrap()
}

fn external_family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2", "s"]);
    IntegralFamily::new(
        "external-projector-rejection",
        vec!["k".to_owned()],
        vec!["p".to_owned()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                context.parse("-m2").unwrap(),
                vec![context.one(), context.zero()],
            ),
            AffineDenominator::new(
                context.parse("s-m2").unwrap(),
                vec![context.one(), context.integer(2)],
            ),
        ],
        vec![vec![context.parameter("s").unwrap()]],
        vec![context.zero(), context.zero()],
    )
    .unwrap()
}

fn vector(loop_id: u16, index: u32) -> IndexedVector {
    IndexedVector::new(LoopVector::new(loop_id), LorentzIndex::new(index))
}

fn spectator(vector_id: u32, index: u32) -> IndexedSpectatorVector {
    IndexedSpectatorVector::new(SpectatorVector::new(vector_id), LorentzIndex::new(index))
}

#[test]
fn family_authenticated_rank_two_projection_replays_and_retains_d_guard() {
    let family = vacuum_family("rank-two-authenticated", 1);
    let context = family.coefficient_context();
    let source = TensorMonomial::new([vector(0, 10), vector(0, 11)]);
    let result = GenericVacuumTensorProjector::new()
        .project(&family, &source)
        .unwrap();

    assert_eq!(result.schema(), GENERIC_VACUUM_TENSOR_PROJECTION_V2_SCHEMA);
    assert_eq!(result.family_fingerprint(), family.fingerprint());
    assert_eq!(result.loop_order(), &["k0".to_owned()]);
    assert_eq!(result.dimension(), family.dimension());
    assert_eq!(result.domain().family(), family.domain());
    assert_eq!(result.source(), &source);
    assert_eq!(result.witness().rank(), 2);
    assert_eq!(result.witness().pairings().len(), 1);
    assert_eq!(
        result.witness().inverse_gram()[0][0],
        context.parse("1/d").unwrap()
    );
    assert_eq!(result.numerator().terms().len(), 1);
    assert_eq!(
        result.numerator().terms()[0].coefficient(),
        &context.parse("1/d").unwrap()
    );
    let guards = result.domain().projection_nonzero_conditions();
    assert_eq!(guards.len(), 1);
    assert_eq!(
        guards[0].polynomial(),
        &context.parameter("d").unwrap().numerator
    );
    assert!(
        guards[0]
            .origins()
            .contains(&TensorProjectionGuardOrigin::ProjectorGramDeterminantNumerator { rank: 2 })
    );
    result.verify(&family).unwrap();
}

#[test]
fn loop_order_external_family_and_limits_are_typed_failures() {
    let family = vacuum_family("loop-id-validation", 1);
    assert!(matches!(
        GenericVacuumTensorProjector::new().project(
            &family,
            &TensorMonomial::new([vector(1, 0), vector(1, 1)])
        ),
        Err(GenericTensorProjectorError::LoopVectorOutOfRange {
            vector: bad,
            loops: 1,
            ..
        }) if bad == LoopVector::new(1)
    ));
    assert!(matches!(
        GenericVacuumTensorProjector::new().project(&external_family(), &TensorMonomial::default()),
        Err(GenericTensorProjectorError::ExternalMomentaUnsupported { externals: 1 })
    ));

    let mut limits = GenericTensorProjectorLimits::default();
    limits.max_arithmetic_operations = 0;
    assert!(matches!(
        GenericVacuumTensorProjector::with_limits(limits)
            .project(&family, &TensorMonomial::new([vector(0, 0), vector(0, 1)])),
        Err(GenericTensorProjectorError::ArithmeticOperationLimit {
            attempted: 1,
            limit: 0
        })
    ));
}

#[test]
fn symbolica_matrix_limits_accept_exact_projector_census_and_reject_one_below() {
    let family = vacuum_family("projector-symbolica-matrix-resource-limits", 1);
    let source = TensorMonomial::new([vector(0, 40), vector(0, 41), vector(0, 42), vector(0, 43)]);
    let baseline = GenericVacuumTensorProjector::new()
        .project(&family, &source)
        .unwrap();
    let stats = baseline.stats();
    assert!(stats.matrix_peak_live_entries > 0);
    assert!(stats.matrix_input_retained_bytes > 0);
    assert!(stats.matrix_output_retained_bytes > 0);
    assert!(stats.symbolica_algebra_operations > 0);

    let exact = GenericTensorProjectorLimits {
        max_matrix_live_entries: stats.matrix_peak_live_entries,
        max_matrix_input_retained_bytes: stats.matrix_input_retained_bytes,
        max_matrix_output_retained_bytes: stats.matrix_output_retained_bytes,
        ..GenericTensorProjectorLimits::default()
    };
    let at_boundary = GenericVacuumTensorProjector::with_limits(exact)
        .project(&family, &source)
        .unwrap();
    assert_eq!(
        at_boundary.stats().matrix_peak_live_entries,
        stats.matrix_peak_live_entries
    );
    assert_eq!(
        at_boundary.stats().matrix_input_retained_bytes,
        stats.matrix_input_retained_bytes
    );
    assert_eq!(
        at_boundary.stats().matrix_output_retained_bytes,
        stats.matrix_output_retained_bytes
    );

    let below_live = GenericTensorProjectorLimits {
        max_matrix_live_entries: stats.matrix_peak_live_entries - 1,
        ..GenericTensorProjectorLimits::default()
    };
    assert!(matches!(
        GenericVacuumTensorProjector::with_limits(below_live).project(&family, &source),
        Err(GenericTensorProjectorError::ResourceLimit {
            resource: "live Symbolica matrix entries",
            requested,
            limit,
        }) if requested == stats.matrix_peak_live_entries
            && limit == stats.matrix_peak_live_entries - 1
    ));

    let below_input = GenericTensorProjectorLimits {
        max_matrix_input_retained_bytes: stats.matrix_input_retained_bytes - 1,
        ..GenericTensorProjectorLimits::default()
    };
    assert!(matches!(
        GenericVacuumTensorProjector::with_limits(below_input).project(&family, &source),
        Err(GenericTensorProjectorError::ResourceLimit {
            resource: "coefficient matrix input retained bytes",
            requested,
            limit,
        }) if requested == stats.matrix_input_retained_bytes
            && limit == stats.matrix_input_retained_bytes - 1
    ));

    let below_output = GenericTensorProjectorLimits {
        max_matrix_output_retained_bytes: stats.matrix_output_retained_bytes - 1,
        ..GenericTensorProjectorLimits::default()
    };
    assert!(matches!(
        GenericVacuumTensorProjector::with_limits(below_output).project(&family, &source),
        Err(GenericTensorProjectorError::ResourceLimit {
            resource: "coefficient matrix output retained bytes",
            requested,
            limit,
        }) if requested == stats.matrix_output_retained_bytes
            && limit == stats.matrix_output_retained_bytes - 1
    ));
}

#[test]
fn retained_family_domain_origin_limit_accepts_exact_boundary_and_rejects_one_below() {
    let family = vacuum_family("projector-family-domain-origin-limit", 1);
    let source = TensorMonomial::default();
    let baseline = GenericVacuumTensorProjector::new()
        .project(&family, &source)
        .unwrap();
    let requested = baseline.stats().family_domain_origins;
    assert!(requested > 0);

    let exact = GenericTensorProjectorLimits {
        max_family_domain_origins: requested,
        ..GenericTensorProjectorLimits::default()
    };
    let at_boundary = GenericVacuumTensorProjector::with_limits(exact)
        .project(&family, &source)
        .unwrap();
    assert_eq!(at_boundary.stats().family_domain_origins, requested);

    let below = GenericTensorProjectorLimits {
        max_family_domain_origins: requested - 1,
        ..GenericTensorProjectorLimits::default()
    };
    assert!(matches!(
        GenericVacuumTensorProjector::with_limits(below).project(&family, &source),
        Err(GenericTensorProjectorError::ResourceLimit {
            resource: "retained tensor projector family-domain origins",
            requested: actual,
            limit,
        }) if actual == requested && limit == requested - 1
    ));
}

#[test]
fn rank_four_and_metric_contraction_match_frozen_vacuum_equations() {
    let family = vacuum_family("rank-four-authenticated", 1);
    let context = family.coefficient_context();
    let projector = GenericVacuumTensorProjector::new();
    let rank_four = projector
        .project(
            &family,
            &TensorMonomial::new([vector(0, 20), vector(0, 21), vector(0, 22), vector(0, 23)]),
        )
        .unwrap();
    assert_eq!(rank_four.numerator().terms().len(), 3);
    for term in rank_four.numerator().terms() {
        assert_eq!(term.coefficient(), &context.parse("1/(d*(d+2))").unwrap());
        assert_eq!(term.metrics().metrics().len(), 2);
        assert_eq!(
            term.scalar_products()
                .exponent(ScalarProductCoordinate::LoopLoop { left: 0, right: 0 }),
            2
        );
    }

    // g(mu,nu) k(mu) k(nu) is first contracted to k^2 and therefore carries
    // no spurious projector denominator.
    let trace = projector
        .project(
            &family,
            &TensorMonomial::from_parts(
                [vector(0, 30), vector(0, 31)],
                [Metric::new(LorentzIndex::new(30), LorentzIndex::new(31))],
                ScalarProductMonomial::one(),
            ),
        )
        .unwrap();
    assert_eq!(trace.witness().rank(), 0);
    assert_eq!(trace.numerator().terms().len(), 1);
    assert_eq!(trace.numerator().terms()[0].coefficient(), &context.one());
    assert_eq!(
        trace.numerator().terms()[0]
            .scalar_products()
            .exponent(ScalarProductCoordinate::LoopLoop { left: 0, right: 0 }),
        1
    );

    let odd = projector
        .project(
            &family,
            &TensorMonomial::new([vector(0, 40), vector(0, 41), vector(0, 42)]),
        )
        .unwrap();
    assert!(odd.numerator().is_zero());
    odd.verify(&family).unwrap();
}

#[test]
fn determinant_guard_accepts_regular_dimension_and_rejects_singular_gram_matrices() {
    let rank_four_source =
        TensorMonomial::new([vector(0, 60), vector(0, 61), vector(0, 62), vector(0, 63)]);

    // A pivot-order transcript can introduce the spurious locus d+1.  The
    // determinant-based V2 domain correctly accepts d=-1.
    let regular = vacuum_family_at_integer_dimension("rank-four-d-minus-one", 1, -1);
    let projected = GenericVacuumTensorProjector::new()
        .project(&regular, &rank_four_source)
        .unwrap();
    assert_eq!(projected.numerator().terms().len(), 3);
    assert!(
        projected
            .domain()
            .projection_nonzero_conditions()
            .is_empty()
    );
    for term in projected.numerator().terms() {
        assert_eq!(
            term.coefficient(),
            &regular.coefficient_context().integer(-1)
        );
    }

    for (dimension, rank, source) in [
        (0, 2, TensorMonomial::new([vector(0, 70), vector(0, 71)])),
        (1, 4, rank_four_source.clone()),
        (-2, 4, rank_four_source.clone()),
    ] {
        let singular = vacuum_family_at_integer_dimension(
            &format!("rank-{rank}-singular-d-{dimension}"),
            1,
            dimension,
        );
        assert!(matches!(
            GenericVacuumTensorProjector::new().project(&singular, &source),
            Err(GenericTensorProjectorError::SingularProjector { rank: actual })
                if actual == rank
        ));
    }
}

#[test]
fn vakint_spectators_remain_covariants_but_not_family_external_momenta() {
    let family = vacuum_family("vakint-spectator-covariants", 2);
    let context = family.coefficient_context();
    let limits = GenericTensorProjectorLimits::default();
    let projector = GenericVacuumTensorProjector::with_limits(limits);

    // Frozen Vakint equation: k(mu) p(mu) integrates to zero for a vacuum
    // family because the loop tensor rank is odd.  The spectator p is not an
    // IntegralFamily external momentum.
    let odd = CovariantTensorMonomial::try_from_parts_with_limits(
        [vector(0, 3)],
        [spectator(1, 3)],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let odd_projection = projector.project_covariant(&family, &odd).unwrap();
    assert!(odd_projection.numerator().is_zero());
    odd_projection.verify(&family).unwrap();

    // Frozen one-/two-loop Vakint equation:
    // k_a(mu) k_b(nu) p_2(mu) p_3(nu)
    //   -> (p_2.p_3) g-projector contraction (k_a.k_b)/d.
    let source = CovariantTensorMonomial::try_from_parts_with_limits(
        [vector(0, 10), vector(1, 11)],
        [spectator(2, 10), spectator(3, 11)],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let projected = projector.project_covariant(&family, &source).unwrap();
    assert_eq!(
        projected.schema(),
        GENERIC_VACUUM_COVARIANT_TENSOR_PROJECTION_V2_SCHEMA
    );
    assert_eq!(projected.numerator().terms().len(), 1);
    let term = &projected.numerator().terms()[0];
    assert_eq!(term.coefficient(), &context.parse("1/d").unwrap());
    assert!(term.covariant().metrics().is_empty());
    assert!(term.covariant().spectator_vectors().is_empty());
    assert_eq!(
        term.covariant()
            .spectator_scalar_products()
            .exponent(SpectatorScalarProduct::new(
                SpectatorVector::new(2),
                SpectatorVector::new(3)
            )),
        1
    );
    assert_eq!(
        term.loop_scalar_products()
            .exponent(ScalarProductCoordinate::LoopLoop { left: 0, right: 1 }),
        1
    );
    assert!(matches!(
        projected.numerator().try_metric_numerator(10),
        Err(GenericTensorProjectorError::SpectatorCovariantCannotUseMetricBridge { term: 0 })
    ));
    projected.verify(&family).unwrap();
}

#[test]
fn projection_bound_scalar_lowering_preserves_both_proof_domains() {
    let family = vacuum_family("projection-bound-lowering", 1);
    let context = family.coefficient_context();
    let projection = GenericVacuumTensorProjector::new()
        .project(
            &family,
            &TensorMonomial::new([vector(0, 50), vector(0, 51)]),
        )
        .unwrap();
    let base = ConcreteIntegralKey::try_new([1]).unwrap();
    let bound = projection.lower(&family, &base).unwrap();
    assert_eq!(
        bound.schema(),
        AUTHENTICATED_VACUUM_TENSOR_LOWERING_V2_SCHEMA
    );
    assert_eq!(bound.projection(), &projection);
    assert_eq!(
        bound
            .lowering()
            .term(
                &MetricPairing::new([Metric::new(LorentzIndex::new(50), LorentzIndex::new(51))]),
                &ConcreteIntegralKey::try_new([0]).unwrap()
            )
            .unwrap()
            .coefficient(),
        &context.parse("1/d").unwrap()
    );
    assert_eq!(
        bound
            .lowering()
            .term(
                &MetricPairing::new([Metric::new(LorentzIndex::new(50), LorentzIndex::new(51))]),
                &ConcreteIntegralKey::try_new([1]).unwrap()
            )
            .unwrap()
            .coefficient(),
        &context.parse("m2/d").unwrap()
    );
    assert!(
        !bound
            .projection()
            .domain()
            .projection_nonzero_conditions()
            .is_empty()
    );
    assert!(!bound.lowering().coefficient_nonzero_conditions().is_empty());
    bound.verify(&family).unwrap();
}

#[test]
fn frozen_vakint_one_loop_a_and_b_tensor_numerators_match_exactly() {
    let family = vacuum_family("frozen-vakint-one-loop-a-b", 1);
    let context = family.coefficient_context();
    let limits = GenericTensorProjectorLimits::default();
    let projector = GenericVacuumTensorProjector::with_limits(limits);
    let output_metric =
        MetricPairing::new([Metric::new(LorentzIndex::new(1), LorentzIndex::new(2))]);
    let kk = ScalarProductCoordinate::LoopLoop { left: 0, right: 0 };

    // Vakint fixture A:
    // k(mu)k(nu) + k(rho)p_1(rho) -> g(mu,nu) k^2/d.
    let even_a = CovariantTensorMonomial::try_from_parts_with_limits(
        [vector(0, 1), vector(0, 2)],
        [],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let odd_a = CovariantTensorMonomial::try_from_parts_with_limits(
        [vector(0, 3)],
        [spectator(1, 3)],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let even_a = projector.project_covariant(&family, &even_a).unwrap();
    let odd_a = projector.project_covariant(&family, &odd_a).unwrap();
    assert!(odd_a.numerator().is_zero());
    assert_eq!(even_a.numerator().terms().len(), 1);
    let term = &even_a.numerator().terms()[0];
    assert_eq!(term.coefficient(), &context.parse("1/d").unwrap());
    assert_eq!(term.covariant().metrics(), &output_metric);
    assert!(term.covariant().spectator_vectors().is_empty());
    assert!(term.covariant().spectator_scalar_products().is_one());
    assert_eq!(term.loop_scalar_products().exponent(kk), 1);

    // Vakint fixture B consists of three pieces.  Test their exact sum:
    // (k(mu)k(nu))^2 g(mu,nu) -> g(mu,nu) (k^2)^2,
    // k(rho)p_1(rho) -> 0,
    // k(mu)k(nu)p_2(mu)p_3(nu) -> (p_2.p_3) k^2/d.
    let quartic_scalar = ScalarProductMonomial::from_factors([(
        ScalarProduct::new(LoopVector::new(0), LoopVector::new(0)),
        2,
    )]);
    let traced_quartic = CovariantTensorMonomial::try_from_parts_with_limits(
        [],
        [],
        [Metric::new(LorentzIndex::new(1), LorentzIndex::new(2))],
        quartic_scalar,
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let spectator_rank_two = CovariantTensorMonomial::try_from_parts_with_limits(
        [vector(0, 1), vector(0, 2)],
        [spectator(2, 1), spectator(3, 2)],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let quartic = projector
        .project_covariant(&family, &traced_quartic)
        .unwrap();
    let mixed = projector
        .project_covariant(&family, &spectator_rank_two)
        .unwrap();
    assert_eq!(quartic.numerator().terms().len(), 1);
    let quartic = &quartic.numerator().terms()[0];
    assert_eq!(quartic.coefficient(), &context.one());
    assert_eq!(quartic.covariant().metrics(), &output_metric);
    assert_eq!(quartic.loop_scalar_products().exponent(kk), 2);
    assert_eq!(mixed.numerator().terms().len(), 1);
    let mixed = &mixed.numerator().terms()[0];
    assert_eq!(mixed.coefficient(), &context.parse("1/d").unwrap());
    assert!(mixed.covariant().metrics().is_empty());
    assert_eq!(mixed.loop_scalar_products().exponent(kk), 1);
    assert_eq!(
        mixed
            .covariant()
            .spectator_scalar_products()
            .exponent(SpectatorScalarProduct::new(
                SpectatorVector::new(2),
                SpectatorVector::new(3),
            )),
        1
    );
}
