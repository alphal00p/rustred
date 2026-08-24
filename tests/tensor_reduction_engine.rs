use std::convert::Infallible;
use std::sync::Arc;

use rustred::reduction_engine::{
    ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus,
};
use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits, AffineDenominator,
    AuthenticatedVacuumCovariantTensorLowering, CoefficientContext, ConcreteIntegralKey,
    CovariantTensorMonomial, GenericScalarProductMonomial, GenericTensorFamilyReducer,
    GenericTensorNumerator, GenericTensorProjectorLimits, GenericTensorTerm,
    GenericVacuumTensorProjector, IndexedSpectatorVector, IndexedVector, IntegralFamily,
    IntegralOrderingPolicy, LoopVector, LorentzIndex, MasterPolicyProvider, Metric, MetricPairing,
    ParametricIbpGenerator, ParametricReductionEngine, ReductionEngineLimits,
    ScalarProductCoordinate, ScalarProductMonomial, SpectatorScalarProduct,
    SpectatorScalarProductMonomial, SpectatorVector, TensorConstructionLimits, TensorMonomial,
    TensorParametricReductionComposer, TensorReductionCertificateError, TensorReductionEngineError,
    TensorReductionEngineLimits, VacuumTensorProjector,
};

fn tadpole_in_context(name: &str, context: CoefficientContext) -> IntegralFamily {
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.parameter("m2").unwrap(),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn tadpole(name: &str) -> IntegralFamily {
    tadpole_in_context(name, CoefficientContext::new(["d", "m2"]))
}

fn key(power: i64) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new([power]).unwrap()
}

fn rank_two_lowering(
    family: &IntegralFamily,
) -> (rustred::GenericTensorIntegralReduction, MetricPairing) {
    let mut projector = VacuumTensorProjector::with_dimension(
        family.coefficient_context(),
        family.dimension().clone(),
    );
    let left = LorentzIndex::new(10);
    let right = LorentzIndex::new(11);
    let projected = projector
        .reduce(&TensorMonomial::new([
            IndexedVector::new(LoopVector::new(0), left),
            IndexedVector::new(LoopVector::new(0), right),
        ]))
        .unwrap();
    let lowering = GenericTensorFamilyReducer::new(family)
        .lower_vacuum_projection(&key(2), &projected)
        .unwrap();
    (lowering, MetricPairing::new([Metric::new(left, right)]))
}

#[test]
fn authenticated_rank_two_projection_to_selected_master_is_complete_and_replayable() {
    let family = tadpole("tensor-engine-authenticated-rank-two");
    let context = family.coefficient_context();
    let left = LorentzIndex::new(70);
    let right = LorentzIndex::new(71);
    let metric = MetricPairing::new([Metric::new(left, right)]);
    let source = TensorMonomial::try_new_with_limits(
        [
            IndexedVector::new(LoopVector::new(0), left),
            IndexedVector::new(LoopVector::new(0), right),
        ],
        TensorConstructionLimits::default(),
    )
    .unwrap();
    let projection = GenericVacuumTensorProjector::new()
        .project(&family, &source)
        .unwrap();
    let authenticated_lowering = projection.lower(&family, &key(2)).unwrap();

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
        context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated(authenticated_lowering, &mut engine)
        .unwrap();

    result.require_complete().unwrap();
    assert_eq!(result.scalar_reduction().len(), 1);
    assert_eq!(
        result
            .scalar_reduction()
            .term(&metric, &key(1))
            .unwrap()
            .coefficient(),
        &context.parse("1/2").unwrap()
    );
    assert_eq!(result.scalar_reduction().selected_masters().len(), 1);
    assert!(
        result
            .scalar_reduction()
            .selected_masters()
            .iter()
            .any(|leaf| leaf.metrics() == &metric && leaf.integral() == &key(1))
    );

    let domains = result.domains();
    assert!(
        !domains
            .projection()
            .projection_nonzero_conditions()
            .is_empty()
    );
    assert!(
        !domains
            .lowering()
            .coefficient_nonzero_conditions()
            .is_empty()
    );
    assert!(domains.scalar_guards().iter().any(|guard| {
        guard
            .condition()
            .polynomial()
            .to_expression()
            .to_string()
            .contains("m2")
    }));
    assert_eq!(
        domains.scalar_certified_domains().len(),
        result.scalar_reduction().scalar_witnesses().len()
    );
    for (source, witness) in result.scalar_reduction().scalar_witnesses() {
        assert_eq!(
            domains.scalar_certified_domain(source),
            Some(witness.certified_domain())
        );
    }
    result.verify(&family).unwrap();
    result.verify_with_engine(&family, &mut engine).unwrap();
}

#[test]
fn vakint_b_spectator_covariant_reaches_selected_master_end_to_end() {
    let family = tadpole("tensor-engine-vakint-b-covariant");
    let context = family.coefficient_context();
    let limits = GenericTensorProjectorLimits::default();
    let mu = LorentzIndex::new(80);
    let nu = LorentzIndex::new(81);
    let p2 = SpectatorVector::new(2);
    let p3 = SpectatorVector::new(3);
    let source = CovariantTensorMonomial::try_from_parts_with_limits(
        [
            IndexedVector::new(LoopVector::new(0), mu),
            IndexedVector::new(LoopVector::new(0), nu),
        ],
        [
            IndexedSpectatorVector::new(p2, mu),
            IndexedSpectatorVector::new(p3, nu),
        ],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let projection = GenericVacuumTensorProjector::with_limits(limits)
        .project_covariant(&family, &source)
        .unwrap();
    assert_eq!(projection.numerator().terms().len(), 1);
    let covariant = projection.numerator().terms()[0].covariant().clone();
    assert!(covariant.metrics().is_empty());
    assert!(covariant.spectator_vectors().is_empty());
    assert_eq!(
        covariant
            .spectator_scalar_products()
            .exponent(SpectatorScalarProduct::new(p2, p3)),
        1
    );
    let lowering =
        AuthenticatedVacuumCovariantTensorLowering::try_new(projection, &family, &key(2)).unwrap();

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
        context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant(lowering, &mut engine)
        .unwrap();

    result.require_complete().unwrap();
    assert_eq!(result.scalar_reduction().len(), 1);
    assert_eq!(
        result
            .scalar_reduction()
            .term(&covariant, &key(1))
            .unwrap()
            .coefficient(),
        &context.parse("1/2").unwrap()
    );
    let selected = result.scalar_reduction().selected_masters();
    assert_eq!(selected.len(), 1);
    assert!(
        selected
            .iter()
            .any(|leaf| leaf.covariant() == &covariant && leaf.integral() == &key(1))
    );
    let domains = result.domains();
    assert!(
        !domains
            .projection()
            .projection_nonzero_conditions()
            .is_empty()
    );
    assert_eq!(domains.lowerings().len(), 1);
    assert!(domains.scalar_guards().iter().any(|guard| {
        guard
            .condition()
            .polynomial()
            .to_expression()
            .to_string()
            .contains("m2")
    }));
    assert_eq!(
        domains.scalar_certified_domains().len(),
        result.scalar_reduction().scalar_witnesses().len()
    );
    for (source, witness) in result.scalar_reduction().scalar_witnesses() {
        assert_eq!(
            domains.scalar_certified_domain(source),
            Some(witness.certified_domain())
        );
    }
    result.verify(&family).unwrap();
    result.verify_with_engine(&family, &mut engine).unwrap();
}

#[test]
fn generated_one_loop_ibps_reduce_rank_two_tensor_with_preserved_domains_and_replay() {
    let family = tadpole("tensor-engine-generated-tadpole");
    let (lowering, metric) = rank_two_lowering(&family);
    let context = family.coefficient_context();

    // k(mu) k(nu)/(k^2+m2)^2 first lowers to
    // g(mu,nu)/d * [I(1) - m2 I(2)].
    assert_eq!(
        lowering.term(&metric, &key(1)).unwrap().coefficient(),
        &context.parse("1/d").unwrap()
    );
    assert_eq!(
        lowering.term(&metric, &key(2)).unwrap().coefficient(),
        &context.parse("-m2/d").unwrap()
    );
    assert_eq!(lowering.coefficient_nonzero_conditions().len(), 1);
    assert_eq!(
        lowering.coefficient_nonzero_conditions()[0].polynomial(),
        &context.parameter("d").unwrap().numerator
    );

    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let provider = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce(&lowering, &mut engine)
        .unwrap();

    // The generated IBP gives I(2)=(2-d)/(2*m2) I(1), so the tensor
    // coefficient is exactly 1/2. No recurrence was embedded in the bridge.
    assert_eq!(result.len(), 1);
    assert_eq!(
        result.term(&metric, &key(1)).unwrap().coefficient(),
        &context.parse("1/2").unwrap()
    );
    let origins = result.term(&metric, &key(1)).unwrap().origins();
    assert_eq!(origins.len(), 2);
    assert_eq!(
        origins
            .iter()
            .map(|origin| origin.scalar_source().powers()[0])
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    // The projector's pre-cancellation d guard remains in the tensor domain,
    // while the generated scalar rule contributes m2 != 0 with source-row
    // provenance. The final coefficient 1/2 does not erase either proof fact.
    assert_eq!(result.domain().coefficient_nonzero_conditions().len(), 1);
    assert!(result.guards().iter().any(|guard| {
        guard
            .condition()
            .polynomial()
            .to_expression()
            .to_string()
            .contains("m2")
    }));
    assert!(
        result
            .guards()
            .iter()
            .all(|guard| !guard.sources().is_empty())
    );

    assert_eq!(result.scalar_witnesses().len(), 2);
    assert_eq!(result.uncovered_leaves().len(), 1);
    let uncovered = result.uncovered_leaves().first().unwrap();
    assert_eq!(uncovered.metrics(), &metric);
    assert_eq!(uncovered.integral(), &key(1));
    assert!(result.selected_masters().is_empty());
    assert!(result.certified_masters().is_empty());
    assert_eq!(
        result.require_complete().unwrap_err().uncovered_leaves(),
        result.uncovered_leaves()
    );
    assert_eq!(result.stats().unique_scalar_reductions(), 2);
    assert_eq!(result.stats().output_terms(), 1);

    result.verify_collected(&family).unwrap();
    result.verify_with_engine(&family, &mut engine).unwrap();
}

#[test]
fn exact_cancellation_removes_stale_tensor_terminal_statuses() {
    let family = tadpole("tensor-engine-cancellation");
    let context = family.coefficient_context();
    let numerator = GenericTensorNumerator::try_new([
        GenericTensorTerm::new(
            context.parse("m2*d").unwrap(),
            MetricPairing::empty(),
            GenericScalarProductMonomial::one(),
        ),
        GenericTensorTerm::new(
            context.parse("d-2").unwrap(),
            MetricPairing::empty(),
            GenericScalarProductMonomial::try_from_factors([(
                ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
                1,
            )])
            .unwrap(),
        ),
    ])
    .unwrap();
    let lowering = GenericTensorFamilyReducer::new(&family)
        .lower(&key(2), &numerator)
        .unwrap();

    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let provider = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce(&lowering, &mut engine)
        .unwrap();

    // I(2)=(2-d)/(2*m2) I(1), making
    // m2*d I(2)+(d-2)k^2 I(2) exactly zero.
    assert!(result.is_zero());
    assert!(result.terminal_statuses().is_empty());
    assert!(result.uncovered_leaves().is_empty());
    assert!(result.selected_masters().is_empty());
    assert!(result.certified_masters().is_empty());
    result.require_complete().unwrap();
    result.verify_collected(&family).unwrap();
}

#[derive(Clone)]
struct PowerStatusProvider {
    arity: usize,
}

impl ConcreteRuleProvider for PowerStatusProvider {
    type Error = Infallible;

    fn index_arity(&self) -> usize {
        self.arity
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        let status = match integral.powers()[0] {
            1 => ConcreteTerminalStatus::SelectedMaster,
            _ => ConcreteTerminalStatus::CertifiedMaster {
                certificate_fingerprint: Arc::from("tensor-test-certificate-v1"),
            },
        };
        Ok(ConcreteRuleDecision::Terminal(status))
    }
}

#[test]
fn selected_and_certified_tensor_terminals_remain_distinct_and_complete() {
    let family = tadpole("tensor-engine-explicit-masters");
    let (lowering, metric) = rank_two_lowering(&family);
    let provider = PowerStatusProvider { arity: 1 };
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce(&lowering, &mut engine)
        .unwrap();
    assert!(result.uncovered_leaves().is_empty());
    assert_eq!(result.selected_masters().len(), 1);
    assert_eq!(result.certified_masters().len(), 1);
    assert!(
        result
            .selected_masters()
            .iter()
            .any(|leaf| leaf.metrics() == &metric && leaf.integral() == &key(1))
    );
    assert!(
        result
            .certified_masters()
            .iter()
            .any(|(leaf, certificate)| {
                leaf.metrics() == &metric
                    && leaf.integral() == &key(2)
                    && certificate.as_ref() == "tensor-test-certificate-v1"
            })
    );
    result.require_complete().unwrap();
    result.verify_collected(&family).unwrap();
}

#[test]
fn composition_rejects_resource_family_context_and_arity_mismatches() {
    let context = CoefficientContext::new(["d", "m2"]);
    let family = tadpole_in_context("tensor-engine-failure-family", context.clone());
    let (lowering, _) = rank_two_lowering(&family);

    let resource_limits = TensorReductionEngineLimits {
        max_unique_scalar_reductions: 1,
        ..TensorReductionEngineLimits::default()
    };
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        PowerStatusProvider { arity: 1 },
        ReductionEngineLimits::default(),
    );
    assert!(matches!(
        TensorParametricReductionComposer::with_limits(&family, resource_limits)
            .reduce(&lowering, &mut engine),
        Err(TensorReductionEngineError::Certificate(
            TensorReductionCertificateError::ResourceLimit {
                resource: "unique scalar reductions",
                requested: 2,
                limit: 1,
            }
        ))
    ));

    let wrong_family = tadpole_in_context("tensor-engine-wrong-family", context.clone());
    assert!(matches!(
        TensorParametricReductionComposer::new(&wrong_family).reduce(&lowering, &mut engine),
        Err(TensorReductionEngineError::Certificate(
            TensorReductionCertificateError::Lowering(
                rustred::GenericTensorFamilyError::WrongFamilyFingerprint { .. }
            )
        ))
    ));

    let mut wrong_fingerprint_engine = ParametricReductionEngine::new(
        "wrong-scalar-engine-family",
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        PowerStatusProvider { arity: 1 },
        ReductionEngineLimits::default(),
    );
    assert!(matches!(
        TensorParametricReductionComposer::new(&family)
            .reduce(&lowering, &mut wrong_fingerprint_engine),
        Err(TensorReductionEngineError::Certificate(
            TensorReductionCertificateError::WrongEngineFamily { .. }
        ))
    ));

    let foreign_context = CoefficientContext::new(["foreign"]);
    let mut wrong_context_engine = ParametricReductionEngine::new(
        family.fingerprint(),
        &foreign_context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        PowerStatusProvider { arity: 1 },
        ReductionEngineLimits::default(),
    );
    assert!(matches!(
        TensorParametricReductionComposer::new(&family)
            .reduce(&lowering, &mut wrong_context_engine),
        Err(TensorReductionEngineError::Certificate(
            TensorReductionCertificateError::WrongEngineContext
        ))
    ));

    let mut wrong_arity_engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        PowerStatusProvider { arity: 2 },
        ReductionEngineLimits::default(),
    );
    assert!(matches!(
        TensorParametricReductionComposer::new(&family).reduce(&lowering, &mut wrong_arity_engine),
        Err(TensorReductionEngineError::Certificate(
            TensorReductionCertificateError::WrongEngineArity {
                expected: 1,
                actual: 2,
            }
        ))
    ));
}
