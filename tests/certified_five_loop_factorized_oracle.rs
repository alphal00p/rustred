//! First bounded five-loop generated-rule scalar/tensor rung.
//!
//! Five independent equal-mass tadpoles provide an elementary oracle while
//! exercising the loop-count-agnostic machinery at five loops: five physical
//! propagators are completed to all fifteen loop scalar products, all 25 IBPs
//! are generated, and a dotted propagator is reduced without a loop-specific
//! production rule.
//!
//! This is deliberately not a connected five-loop reduction or a completed
//! five-loop milestone.  It is a bounded first rung through the generic path.

use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits, AffineDenominator,
    AuthenticatedVacuumCovariantTensorLowering, AutomaticIspCompletion,
    CertifiedConcreteRewriteProof, CertifiedFamilyRuleProvider, CertifiedFamilyRuleProviderLimits,
    Coefficient, CoefficientContext, ConcreteIntegralKey, ConcreteRuleApplicationTrace,
    CovariantTensorMonomial, GenericTensorProjectorLimits, GenericVacuumTensorProjector,
    IndexedVector, IntegralOrderingPolicy, LoopVector, LorentzIndex, MasterPolicyProvider, Metric,
    MetricPairing, ParametricIbpGenerator, ParametricReductionEngine, ReductionEngineLimits,
    ScalarProductCoordinate, ScalarProductMonomial, SectorRestrictions,
    SpectatorScalarProductMonomial, TensorCovariantStructure, TensorParametricReductionComposer,
    VerifiedInternalFamilyPermutationSymmetry,
};

const LOOPS: usize = 5;
const BASIS: usize = LOOPS * (LOOPS + 1) / 2;
const EXPECTED_COORDINATES: [ScalarProductCoordinate; BASIS] = [
    ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
    ScalarProductCoordinate::LoopLoop { left: 0, right: 1 },
    ScalarProductCoordinate::LoopLoop { left: 0, right: 2 },
    ScalarProductCoordinate::LoopLoop { left: 0, right: 3 },
    ScalarProductCoordinate::LoopLoop { left: 0, right: 4 },
    ScalarProductCoordinate::LoopLoop { left: 1, right: 1 },
    ScalarProductCoordinate::LoopLoop { left: 1, right: 2 },
    ScalarProductCoordinate::LoopLoop { left: 1, right: 3 },
    ScalarProductCoordinate::LoopLoop { left: 1, right: 4 },
    ScalarProductCoordinate::LoopLoop { left: 2, right: 2 },
    ScalarProductCoordinate::LoopLoop { left: 2, right: 3 },
    ScalarProductCoordinate::LoopLoop { left: 2, right: 4 },
    ScalarProductCoordinate::LoopLoop { left: 3, right: 3 },
    ScalarProductCoordinate::LoopLoop { left: 3, right: 4 },
    ScalarProductCoordinate::LoopLoop { left: 4, right: 4 },
];

fn square_row(context: &CoefficientContext, loop_index: usize) -> Vec<Coefficient> {
    let mut row = vec![context.zero(); BASIS];
    let mut ordinal = 0;
    for left in 0..LOOPS {
        for right in left..LOOPS {
            if left == loop_index && right == loop_index {
                row[ordinal] = context.one();
            }
            ordinal += 1;
        }
    }
    row
}

fn completed_family() -> AutomaticIspCompletion {
    let context = CoefficientContext::new(["d", "m2"]);
    let minus_m2 = context.parse("-m2").unwrap();
    AutomaticIspCompletion::try_new(
        "certified-generic-five-loop-five-tadpoles",
        (0..LOOPS).map(|index| format!("k{index}")).collect(),
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        (0..LOOPS)
            .map(|loop_index| {
                AffineDenominator::new(minus_m2.clone(), square_row(&context, loop_index))
            })
            .collect(),
        Vec::new(),
        vec![context.zero(); LOOPS],
    )
    .unwrap()
}

fn key(powers: [i64; BASIS]) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

#[test]
fn generated_ibps_reduce_five_loop_factorized_dot_and_rank_two_tensor() {
    let completion = completed_family();
    assert_eq!(completion.input_denominator_count(), LOOPS);
    assert_eq!(
        completion.appended_coordinate_ordinals(),
        &[1, 2, 3, 4, 6, 7, 8, 10, 11, 13]
    );
    assert_eq!(
        completion.rank_progression(),
        &[5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
    );
    assert_eq!(completion.stats().rank_tests(), 15);
    assert_eq!(completion.stats().appended_isps(), 10);
    assert!(completion.stats().rank_operations() > 0);
    completion.replay().unwrap();
    let family = completion.into_family();
    assert_eq!(family.loop_count(), LOOPS);
    assert_eq!(family.denominator_count(), BASIS);
    assert_eq!(family.coordinates(), &EXPECTED_COORDINATES);

    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    assert_eq!(rows.len(), 25, "L(L+E)=25 generated ordinary IBPs");
    assert!(generated.lorentz_invariance().is_empty());

    // The generated concrete quotient may pass through pinches and through
    // negative powers of the ten generated ISP rows.  Those are numerator
    // intermediates, not grounds for an authored sector-zero rule, so keep
    // every sector admissible and let generic scaleless proofs remove zeros.
    let restrictions = SectorRestrictions::unrestricted(BASIS).unwrap();
    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    adaptive_limits.max_search_depth = 1;
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        adaptive_limits,
    )
    .unwrap();
    let provider = CertifiedFamilyRuleProvider::try_new(
        family.clone(),
        restrictions,
        std::iter::empty::<VerifiedInternalFamilyPermutationSymmetry>(),
        adaptive,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CertifiedFamilyRuleProviderLimits::default(),
    )
    .unwrap();
    let master = key([1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let dotted = key([1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let provider = MasterPolicyProvider::with_selected(provider, [master.clone()]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    let scalar = engine.reduce(&dotted).unwrap();
    scalar.require_complete().unwrap();
    assert_eq!(scalar.terms().len(), 1);
    // The five physical rows are exactly k_i^2-m2 at coordinate ordinals
    // [0,5,9,12,14], hence the integral is T1^5.  For the dotted fifth factor
    // 0=(d-2a)I(a)-2a*m2*I(a+1), so a=1 fixes this independent oracle.
    assert_eq!(
        scalar.terms().get(&master).unwrap(),
        &family.coefficient_context().parse("(d-2)/(2*m2)").unwrap()
    );
    assert!(scalar.application_traces().iter().any(|trace| matches!(
        trace,
        ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite)
            if matches!(
                rewrite.proof(),
                CertifiedConcreteRewriteProof::ConcreteQuotientElimination { .. }
            )
    )));
    assert!(scalar.application_traces().iter().all(|trace| match trace {
        ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite) => matches!(
            rewrite.proof(),
            CertifiedConcreteRewriteProof::ConcreteQuotientElimination { .. }
        ),
        ConcreteRuleApplicationTrace::Parametric(_) => false,
        ConcreteRuleApplicationTrace::ConditionalParametric(_) => false,
        ConcreteRuleApplicationTrace::ProvedZero(_) => true,
    }));

    let tensor_source = CovariantTensorMonomial::try_from_parts_with_limits(
        [
            IndexedVector::new(LoopVector::new(4), LorentzIndex::new(50)),
            IndexedVector::new(LoopVector::new(4), LorentzIndex::new(51)),
        ],
        [],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        GenericTensorProjectorLimits::default(),
    )
    .unwrap();
    let projection = GenericVacuumTensorProjector::new()
        .project_covariant(&family, &tensor_source)
        .unwrap();
    let lowering =
        AuthenticatedVacuumCovariantTensorLowering::try_new(projection, &family, &dotted).unwrap();
    let tensor = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant(lowering, &mut engine)
        .unwrap();
    tensor.require_complete().unwrap();
    let metric = TensorCovariantStructure::new(
        MetricPairing::new([Metric::new(LorentzIndex::new(50), LorentzIndex::new(51))]),
        Vec::new(),
        SpectatorScalarProductMonomial::one(),
    );
    assert_eq!(tensor.scalar_reduction().len(), 1);
    assert_eq!(
        tensor
            .scalar_reduction()
            .term(&metric, &master)
            .unwrap()
            .coefficient(),
        &family.coefficient_context().parse("1/2").unwrap()
    );
    assert!(
        tensor
            .scalar_reduction()
            .scalar_witnesses()
            .values()
            .flat_map(|witness| witness.application_traces())
            .any(|trace| matches!(
                trace,
                ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite)
                    if matches!(
                        rewrite.proof(),
                        CertifiedConcreteRewriteProof::ConcreteQuotientElimination { .. }
                    )
            ))
    );
    let adaptive_stats = engine.provider().inner().adaptive().stats();
    assert_eq!(adaptive_stats.eliminations(), 0);
    assert_eq!(adaptive_stats.pivot_candidates(), 0);
    tensor.verify_with_engine(&family, &mut engine).unwrap();

    drop(engine);
    for trace in scalar.application_traces() {
        match trace {
            ConcreteRuleApplicationTrace::Parametric(proof) => {
                assert!(
                    proof
                        .replay_application(&family, generated.context())
                        .unwrap()
                )
            }
            ConcreteRuleApplicationTrace::ConditionalParametric(proof) => {
                proof.replay(&family, proof.parametric_context()).unwrap()
            }
            ConcreteRuleApplicationTrace::CertifiedRewrite(proof) => proof
                .replay(
                    &family,
                    generated.context(),
                    IntegralOrderingPolicy::RustRedUnshiftedV1,
                )
                .unwrap(),
            ConcreteRuleApplicationTrace::ProvedZero(proof) => proof.replay(&family).unwrap(),
        }
    }
    tensor.verify(&family).unwrap();
}
