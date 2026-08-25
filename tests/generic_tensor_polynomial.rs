use std::convert::Infallible;

use rustred::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider};
use rustred::*;

fn family(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        // Frozen Vakint convention: k.k = D1 + m2.
        vec![AffineDenominator::new(
            context.parse("-m2").unwrap(),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn key(power: i64) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new([power]).unwrap()
}

fn k(index: u32) -> IndexedVector {
    IndexedVector::new(LoopVector::new(0), LorentzIndex::new(index))
}

fn p(vector: u32, index: u32) -> IndexedSpectatorVector {
    IndexedSpectatorVector::new(SpectatorVector::new(vector), LorentzIndex::new(index))
}

fn source(
    coefficient: Coefficient,
    loops: impl IntoIterator<Item = IndexedVector>,
    spectators: impl IntoIterator<Item = IndexedSpectatorVector>,
    metrics: impl IntoIterator<Item = Metric>,
) -> WeightedCovariantTensorMonomial {
    WeightedCovariantTensorMonomial::new(
        coefficient,
        CovariantTensorMonomial::try_from_parts_with_limits(
            loops,
            spectators,
            metrics,
            ScalarProductMonomial::one(),
            SpectatorScalarProductMonomial::one(),
            GenericTensorProjectorLimits::default(),
        )
        .unwrap(),
    )
}

#[test]
fn frozen_vakint_b_actual_three_term_sum_reduces_and_replays() {
    let family = family("vakint-b-source-polynomial");
    let context = family.coefficient_context();
    let one = context.one();

    // Exact source from Vakint's tensor_reduction_tests.rs:
    // (k(1,1) k(1,2))^2 g(1,2)
    let quartic = source(
        one.clone(),
        [k(1), k(2), k(1), k(2)],
        [],
        [Metric::new(LorentzIndex::new(1), LorentzIndex::new(2))],
    );
    // Odd source k(1,3) p(1,3), retained as an authenticated zero.
    let odd = source(one.clone(), [k(3)], [p(1, 3)], []);
    // k(1,1) k(1,2) p(2,1) p(3,2)
    let mixed = source(one, [k(1), k(2)], [p(2, 1), p(3, 2)], []);

    let projection = GenericVacuumTensorPolynomialProjector::new()
        .project(&family, [quartic, odd, mixed])
        .unwrap();
    assert_eq!(projection.sources().len(), 3);
    assert_eq!(projection.source_projections().len(), 3);
    assert!(
        projection
            .source_projection(1)
            .unwrap()
            .numerator()
            .is_zero()
    );
    assert_eq!(
        projection
            .source_projection(0)
            .unwrap()
            .witness()
            .precontraction()
            .vector_contractions()
            .len(),
        2
    );
    projection.verify(&family).unwrap();

    let metric = TensorCovariantStructure::new(
        MetricPairing::new([Metric::new(LorentzIndex::new(1), LorentzIndex::new(2))]),
        Vec::new(),
        SpectatorScalarProductMonomial::one(),
    );
    let spectator_product = SpectatorScalarProductMonomial::try_from_factors_with_limits(
        [(
            SpectatorScalarProduct::new(SpectatorVector::new(2), SpectatorVector::new(3)),
            1,
        )],
        GenericTensorProjectorLimits::default(),
    )
    .unwrap();
    let spectator =
        TensorCovariantStructure::new(MetricPairing::empty(), Vec::new(), spectator_product);

    // Tensor stage reproduces Vakint before integral reduction.
    assert_eq!(projection.numerator().terms().len(), 2);
    assert!(projection.numerator().terms().iter().any(|term| {
        term.covariant() == &metric
            && term
                .loop_scalar_products()
                .exponent(ScalarProductCoordinate::LoopLoop { left: 0, right: 0 })
                == 2
            && term.coefficient() == &context.one()
    }));
    assert!(projection.numerator().terms().iter().any(|term| {
        term.covariant() == &spectator
            && term
                .loop_scalar_products()
                .exponent(ScalarProductCoordinate::LoopLoop { left: 0, right: 0 })
                == 1
            && term.coefficient() == &context.parse("1/d").unwrap()
    }));

    let lowering = projection.lower(&family, &key(1)).unwrap();
    lowering.verify(&family).unwrap();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let provider = MasterPolicyProvider::with_selected(adaptive, [key(1)]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant_polynomial(lowering, &mut engine)
        .unwrap();
    result.require_complete().unwrap();
    assert_eq!(result.scalar_reduction().len(), 2);
    assert_eq!(
        result
            .scalar_reduction()
            .term(&metric, &key(1))
            .unwrap()
            .coefficient(),
        &context.parse("m2^2").unwrap()
    );
    assert_eq!(
        result
            .scalar_reduction()
            .term(&spectator, &key(1))
            .unwrap()
            .coefficient(),
        &context.parse("m2/d").unwrap()
    );
    assert_eq!(result.scalar_reduction().selected_masters().len(), 2);
    result.verify(&family).unwrap();
    result.verify_with_engine(&family, &mut engine).unwrap();
}

#[test]
fn cancellation_retains_source_proofs_and_all_projected_provenance() {
    let family = family("tensor-polynomial-cancellation");
    let context = family.coefficient_context();
    let plus = source(context.one(), [k(10), k(11)], [], []);
    let minus = source(context.parse("-1").unwrap(), [k(10), k(11)], [], []);
    let zero = source(context.zero(), [k(12), k(13)], [], []);

    let projection = GenericVacuumTensorPolynomialProjector::new()
        .project(&family, [plus, minus, zero])
        .unwrap();
    assert!(projection.is_zero());
    assert_eq!(projection.source_projections().len(), 3);
    assert_eq!(projection.contributions().len(), 3);
    assert_eq!(projection.provenance().len(), 2);
    assert!(
        projection
            .contributions()
            .iter()
            .any(|contribution| contribution.coefficient().is_zero())
    );
    projection.verify(&family).unwrap();

    let lowering = projection.lower(&family, &key(1)).unwrap();
    assert!(lowering.lowerings().is_empty());
    let provider = PowerStatusProvider;
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant_polynomial(lowering, &mut engine)
        .unwrap();
    assert!(result.scalar_reduction().is_zero());
    assert!(result.scalar_reduction().terminal_statuses().is_empty());
    result.require_complete().unwrap();
    result.verify(&family).unwrap();
}

#[test]
fn aggregate_projector_family_domain_origins_are_bounded_before_child_projection() {
    let family = family("tensor-polynomial-projector-family-origins");
    let context = family.coefficient_context();
    let sources = [
        source(context.one(), [k(14), k(15)], [], []),
        source(context.one(), [k(16), k(17)], [], []),
    ];
    let baseline = GenericVacuumTensorPolynomialProjector::new()
        .project(&family, sources.clone())
        .unwrap();
    let requested = baseline.stats().family_domain_origins;
    assert!(requested > 0);
    assert_eq!(
        requested,
        baseline
            .source_projections()
            .iter()
            .map(|projection| projection.stats().family_domain_origins)
            .sum::<usize>()
    );

    let exact = GenericTensorPolynomialLimits {
        max_family_domain_origins: requested,
        ..GenericTensorPolynomialLimits::default()
    };
    let at_boundary = GenericVacuumTensorPolynomialProjector::with_limits(exact)
        .project(&family, sources.clone())
        .unwrap();
    assert_eq!(at_boundary.stats().family_domain_origins, requested);

    let below = GenericTensorPolynomialLimits {
        max_family_domain_origins: requested - 1,
        ..GenericTensorPolynomialLimits::default()
    };
    assert!(matches!(
        GenericVacuumTensorPolynomialProjector::with_limits(below).project(&family, sources),
        Err(GenericTensorPolynomialError::ResourceLimit {
            resource: "tensor polynomial family-domain origins",
            requested: actual,
            limit,
        }) if actual == requested && limit == requested - 1
    ));
}

#[test]
fn aggregate_symbolica_matrix_census_sums_bytes_and_maximizes_peak_live_entries() {
    let family = family("tensor-polynomial-symbolica-matrix-limits");
    let context = family.coefficient_context();
    let rank_two = source(context.one(), [k(40), k(41)], [], []);
    let rank_four = source(context.one(), [k(50), k(51), k(52), k(53)], [], []);
    let sources = [rank_two, rank_four];

    let child_projector = GenericVacuumTensorProjector::new();
    let child_stats = sources
        .iter()
        .map(|source| {
            child_projector
                .project_covariant(&family, source.monomial())
                .unwrap()
                .stats()
        })
        .collect::<Vec<_>>();
    assert!(child_stats[1].matrix_peak_live_entries > child_stats[0].matrix_peak_live_entries);
    let expected_input = child_stats
        .iter()
        .map(|stats| stats.matrix_input_retained_bytes)
        .sum::<usize>();
    let expected_output = child_stats
        .iter()
        .map(|stats| stats.matrix_output_retained_bytes)
        .sum::<usize>();
    let expected_peak = child_stats
        .iter()
        .map(|stats| stats.matrix_peak_live_entries)
        .max()
        .unwrap();
    assert!(expected_input > 0);
    assert!(expected_output > 0);
    assert!(expected_peak > 0);

    let baseline = GenericVacuumTensorPolynomialProjector::new()
        .project(&family, sources.clone())
        .unwrap();
    assert_eq!(
        baseline.stats().projection_matrix_input_retained_bytes,
        expected_input
    );
    assert_eq!(
        baseline.stats().projection_matrix_output_retained_bytes,
        expected_output
    );
    assert_eq!(
        baseline.stats().projection_matrix_peak_live_entries,
        expected_peak
    );

    let exact = GenericTensorPolynomialLimits {
        max_projection_matrix_peak_live_entries: expected_peak,
        max_projection_matrix_input_retained_bytes: expected_input,
        max_projection_matrix_output_retained_bytes: expected_output,
        ..GenericTensorPolynomialLimits::default()
    };
    let at_boundary = GenericVacuumTensorPolynomialProjector::with_limits(exact)
        .project(&family, sources.clone())
        .unwrap();
    assert_eq!(
        at_boundary.stats().projection_matrix_input_retained_bytes,
        expected_input
    );
    assert_eq!(
        at_boundary.stats().projection_matrix_output_retained_bytes,
        expected_output
    );
    assert_eq!(
        at_boundary.stats().projection_matrix_peak_live_entries,
        expected_peak
    );

    let below_live = GenericTensorPolynomialLimits {
        max_projection_matrix_peak_live_entries: expected_peak - 1,
        ..GenericTensorPolynomialLimits::default()
    };
    assert!(matches!(
        GenericVacuumTensorPolynomialProjector::with_limits(below_live)
            .project(&family, sources.clone()),
        Err(GenericTensorPolynomialError::Projector(
            GenericTensorProjectorError::ResourceLimit {
                resource: "live Symbolica matrix entries",
                requested,
                limit,
            }
        )) if requested == expected_peak && limit == expected_peak - 1
    ));

    let below_input = GenericTensorPolynomialLimits {
        max_projection_matrix_input_retained_bytes: expected_input - 1,
        ..GenericTensorPolynomialLimits::default()
    };
    assert!(matches!(
        GenericVacuumTensorPolynomialProjector::with_limits(below_input)
            .project(&family, sources.clone()),
        Err(GenericTensorPolynomialError::Projector(
            GenericTensorProjectorError::ResourceLimit {
                resource: "coefficient matrix input retained bytes",
                requested,
                limit,
            }
        )) if requested == child_stats[1].matrix_input_retained_bytes
            && limit == child_stats[1].matrix_input_retained_bytes - 1
    ));

    let below_output = GenericTensorPolynomialLimits {
        max_projection_matrix_output_retained_bytes: expected_output - 1,
        ..GenericTensorPolynomialLimits::default()
    };
    assert!(matches!(
        GenericVacuumTensorPolynomialProjector::with_limits(below_output)
            .project(&family, sources),
        Err(GenericTensorPolynomialError::Projector(
            GenericTensorProjectorError::ResourceLimit {
                resource: "coefficient matrix output retained bytes",
                requested,
                limit,
            }
        )) if requested == child_stats[1].matrix_output_retained_bytes
            && limit == child_stats[1].matrix_output_retained_bytes - 1
    ));
}

#[test]
fn aggregate_covariant_lowering_and_retained_clone_limits_are_enforced() {
    let family = family("tensor-polynomial-aggregate-limits");
    let context = family.coefficient_context();
    let polynomial = GenericVacuumTensorPolynomialProjector::new()
        .project(
            &family,
            [
                source(context.one(), [k(20), k(21)], [], []),
                source(context.one(), [k(22), k(23)], [p(2, 22), p(3, 23)], []),
            ],
        )
        .unwrap();
    let limits = GenericTensorFamilyLimits {
        max_covariant_structures: 1,
        ..GenericTensorFamilyLimits::default()
    };
    assert!(matches!(
        polynomial
            .clone()
            .lower_with_limits(&family, &key(1), limits),
        Err(GenericTensorPolynomialError::Certificate(
            TensorReductionCertificateError::ResourceLimit {
                resource: "covariant tensor structures",
                requested: 2,
                limit: 1,
            }
        ))
    ));

    let lowering = polynomial.lower(&family, &key(1)).unwrap();
    assert_eq!(lowering.stats().covariant_structures, 2);
    let engine_limits = TensorReductionEngineLimits {
        max_retained_covariant_structure_entries: 0,
        ..TensorReductionEngineLimits::default()
    };
    let provider = PowerStatusProvider;
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    assert!(matches!(
        TensorParametricReductionComposer::with_limits(&family, engine_limits)
            .reduce_authenticated_covariant_polynomial(lowering, &mut engine),
        Err(TensorPolynomialReductionEngineError::Engine(
            TensorReductionEngineError::Certificate(
                TensorReductionCertificateError::ResourceLimit {
                    resource: "retained covariant tensor structure entries",
                    ..
                }
            )
        ))
    ));
}

#[test]
fn aggregate_family_domain_copy_budgets_accept_the_exact_boundary_and_reject_the_next_copy() {
    let family = family("tensor-polynomial-family-domain-copy-limits");
    let context = family.coefficient_context();
    let polynomial = GenericVacuumTensorPolynomialProjector::new()
        .project(
            &family,
            [
                source(context.one(), [k(24), k(25)], [], []),
                source(context.one(), [k(26), k(27)], [p(2, 26), p(3, 27)], []),
            ],
        )
        .unwrap();
    let baseline = polynomial.clone().lower(&family, &key(1)).unwrap();
    let stats = baseline.stats();
    assert_eq!(stats.family_domain_copies, 2);
    assert!(stats.family_domain_conditions > 0);
    assert!(stats.family_domain_origins > 0);
    assert!(stats.family_domain_polynomial_terms > 0);
    assert!(stats.family_domain_exponent_entries > 0);
    assert!(stats.family_manifest_bytes > 0);

    let exact = GenericTensorFamilyLimits {
        max_family_domain_copies: stats.family_domain_copies,
        max_family_domain_conditions: stats.family_domain_conditions,
        max_family_domain_origins: stats.family_domain_origins,
        max_family_domain_polynomial_terms: stats.family_domain_polynomial_terms,
        max_family_domain_exponent_entries: stats.family_domain_exponent_entries,
        max_family_manifest_bytes: stats.family_manifest_bytes,
        ..GenericTensorFamilyLimits::default()
    };
    let at_boundary = polynomial
        .clone()
        .lower_with_limits(&family, &key(1), exact)
        .unwrap();
    assert_eq!(at_boundary.stats(), stats);

    let cases: [(
        &'static str,
        usize,
        fn(&mut GenericTensorFamilyLimits, usize),
    ); 6] = [
        (
            "covariant tensor family-domain copies",
            stats.family_domain_copies,
            |limits, value| limits.max_family_domain_copies = value,
        ),
        (
            "covariant tensor family-domain conditions",
            stats.family_domain_conditions,
            |limits, value| limits.max_family_domain_conditions = value,
        ),
        (
            "covariant tensor family-domain origins",
            stats.family_domain_origins,
            |limits, value| limits.max_family_domain_origins = value,
        ),
        (
            "covariant tensor family-domain polynomial terms",
            stats.family_domain_polynomial_terms,
            |limits, value| limits.max_family_domain_polynomial_terms = value,
        ),
        (
            "covariant tensor family-domain exponent entries",
            stats.family_domain_exponent_entries,
            |limits, value| limits.max_family_domain_exponent_entries = value,
        ),
        (
            "covariant tensor family manifest bytes",
            stats.family_manifest_bytes,
            |limits, value| limits.max_family_manifest_bytes = value,
        ),
    ];
    for (resource, requested, restrict) in cases {
        let mut below = exact;
        restrict(&mut below, requested - 1);
        assert!(matches!(
            polynomial
                .clone()
                .lower_with_limits(&family, &key(1), below),
            Err(GenericTensorPolynomialError::Certificate(
                TensorReductionCertificateError::ResourceLimit {
                    resource: actual,
                    requested: actual_requested,
                    limit,
                }
            )) if actual == resource
                && actual_requested == requested
                && limit == requested - 1
        ));
    }
}

#[derive(Clone)]
struct AlternatingStatusProvider {
    calls: usize,
}

#[derive(Clone)]
struct PowerStatusProvider;

#[derive(Clone)]
struct CertifiedZeroProvider {
    zero: CertifiedZeroReduction,
}

impl ConcreteRuleProvider for PowerStatusProvider {
    type Error = Infallible;

    fn index_arity(&self) -> usize {
        1
    }

    fn decision_for(
        &mut self,
        _integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        Ok(ConcreteRuleDecision::Terminal(
            ConcreteTerminalStatus::SelectedMaster,
        ))
    }
}

impl ConcreteRuleProvider for CertifiedZeroProvider {
    type Error = Infallible;

    fn index_arity(&self) -> usize {
        1
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        debug_assert_eq!(integral, self.zero.source());
        Ok(ConcreteRuleDecision::ProvedZero(self.zero.clone()))
    }
}

impl ConcreteRuleProvider for AlternatingStatusProvider {
    type Error = Infallible;

    fn index_arity(&self) -> usize {
        1
    }

    fn decision_for(
        &mut self,
        _integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.calls += 1;
        Ok(ConcreteRuleDecision::Terminal(if self.calls == 1 {
            ConcreteTerminalStatus::SelectedMaster
        } else {
            ConcreteTerminalStatus::CertifiedMaster {
                certificate_fingerprint: "conflicting".into(),
            }
        }))
    }
}

#[test]
fn shared_scalar_source_has_one_witness_so_covariants_cannot_conflict() {
    let family = family("tensor-polynomial-status-consistency");
    let context = family.coefficient_context();
    let projection = GenericVacuumTensorPolynomialProjector::new()
        .project(
            &family,
            [
                source(
                    context.one(),
                    [],
                    [],
                    [Metric::new(LorentzIndex::new(30), LorentzIndex::new(31))],
                ),
                source(
                    context.one(),
                    [],
                    [],
                    [Metric::new(LorentzIndex::new(32), LorentzIndex::new(33))],
                ),
            ],
        )
        .unwrap();
    let lowering = projection.lower(&family, &key(1)).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        AlternatingStatusProvider { calls: 0 },
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant_polynomial(lowering, &mut engine)
        .unwrap();
    assert_eq!(result.scalar_reduction().scalar_witnesses().len(), 1);
    assert_eq!(result.scalar_reduction().selected_masters().len(), 2);
    assert!(result.scalar_reduction().certified_masters().is_empty());
    result.require_complete().unwrap();
}

#[test]
fn certified_zero_decision_survives_tensor_composition_and_fresh_engine_replay() {
    let context = CoefficientContext::new(["d", "nu"]);
    let family = IntegralFamily::new(
        "tensor-polynomial-certified-zero",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(context.zero(), vec![context.one()])],
        Vec::new(),
        vec![context.parameter("nu").unwrap()],
    )
    .unwrap();
    let source_key = key(0);
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&family, PowerShiftPolicy::FormalGeneric).unwrap();
    let certificate = match analyzer.analyze_sector(&SectorMask::try_from_bit_string("0").unwrap())
    {
        ZeroSectorDecision::ProvedZero(certificate) => certificate,
        decision => panic!("expected a certified scaleless sector, received {decision:?}"),
    };
    let zero = CertifiedZeroReduction::try_new(
        &family,
        source_key.clone(),
        std::sync::Arc::new(certificate),
        CertifiedRewriteLimits::default(),
    )
    .unwrap();
    assert!(!zero.domain().is_empty());

    let projection = GenericVacuumTensorPolynomialProjector::new()
        .project(
            &family,
            [source(
                context.one(),
                [],
                [],
                [Metric::new(LorentzIndex::new(40), LorentzIndex::new(41))],
            )],
        )
        .unwrap();
    let lowering = projection.lower(&family, &source_key).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CertifiedZeroProvider { zero: zero.clone() },
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant_polynomial(lowering, &mut engine)
        .unwrap();

    assert!(result.scalar_reduction().is_zero());
    let witness = result
        .scalar_reduction()
        .scalar_witnesses()
        .get(&source_key)
        .unwrap();
    assert_eq!(witness.certified_domain(), zero.domain());
    assert!(matches!(
        witness.application_traces(),
        [ConcreteRuleApplicationTrace::ProvedZero(_)]
    ));
    assert_eq!(
        result
            .scalar_reduction()
            .stats()
            .scalar_witness_certified_domain_conditions(),
        zero.domain().len()
    );
    assert_eq!(
        result
            .scalar_reduction()
            .stats()
            .scalar_witness_certified_domain_origins(),
        zero.domain()
            .iter()
            .map(|condition| condition.origins().len())
            .sum::<usize>()
    );
    assert_eq!(
        result
            .scalar_reduction()
            .stats()
            .scalar_witness_application_traces(),
        1
    );
    result.require_complete().unwrap();
    result.verify(&family).unwrap();

    let mut fresh_engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CertifiedZeroProvider { zero },
        ReductionEngineLimits::default(),
    );
    result
        .verify_with_engine(&family, &mut fresh_engine)
        .unwrap();
}
