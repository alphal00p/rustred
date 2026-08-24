//! First bounded four-loop generated-rule scalar/tensor rung.
//!
//! The concrete topology is the massive factorized vacuum `B4 * T1`: a
//! three-loop four-cycle and an independent one-loop tadpole.  It is a test
//! oracle only.  Production receives five physical propagators, completes the
//! ten-dimensional four-loop scalar-product basis generically, generates all
//! sixteen IBPs, and discovers the requested dot rule from those rows.  No
//! loop-named reducer or authored recurrence is used.
//!
//! This is deliberately not a connected four-loop reduction or a completed
//! four-loop milestone.  It is the first cheap rung that exercises the same
//! loop-count-agnostic production path at four loops.

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

const BASIS: usize = 10;
const EXPECTED_COORDINATES: [ScalarProductCoordinate; BASIS] = [
    ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
    ScalarProductCoordinate::LoopLoop { left: 0, right: 1 },
    ScalarProductCoordinate::LoopLoop { left: 0, right: 2 },
    ScalarProductCoordinate::LoopLoop { left: 0, right: 3 },
    ScalarProductCoordinate::LoopLoop { left: 1, right: 1 },
    ScalarProductCoordinate::LoopLoop { left: 1, right: 2 },
    ScalarProductCoordinate::LoopLoop { left: 1, right: 3 },
    ScalarProductCoordinate::LoopLoop { left: 2, right: 2 },
    ScalarProductCoordinate::LoopLoop { left: 2, right: 3 },
    ScalarProductCoordinate::LoopLoop { left: 3, right: 3 },
];

fn square_row(context: &CoefficientContext, routing: [i64; 4]) -> Vec<Coefficient> {
    let mut row = Vec::with_capacity(BASIS);
    for left in 0..4 {
        for right in left..4 {
            let multiplicity = if left == right { 1 } else { 2 };
            row.push(context.integer(multiplicity * routing[left] * routing[right]));
        }
    }
    row
}

fn completed_family() -> AutomaticIspCompletion {
    let context = CoefficientContext::new(["d", "m2"]);
    let minus_m2 = context.parse("-m2").unwrap();
    // D1..D4 form a three-loop four-cycle in (k0,k1,k2), while D5
    // depends only on k3.  Thus the physical integrand factorizes exactly as
    // B4(k0,k1,k2) * T1(k3); the remaining five rows are generated ISPs.
    let physical_routings = [
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [1, 0, -1, 0],
        [0, 1, -1, 0],
        [0, 0, 0, 1],
    ];
    AutomaticIspCompletion::try_new(
        "certified-generic-four-loop-b4-times-tadpole",
        vec!["k0".into(), "k1".into(), "k2".into(), "k3".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        physical_routings
            .into_iter()
            .map(|routing| AffineDenominator::new(minus_m2.clone(), square_row(&context, routing)))
            .collect(),
        Vec::new(),
        vec![context.zero(); physical_routings.len()],
    )
    .unwrap()
}

fn key(powers: [i64; BASIS]) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

#[test]
fn generated_ibps_reduce_four_loop_factorized_dot_and_rank_two_tensor() {
    let completion = completed_family();
    assert_eq!(completion.input_denominator_count(), 5);
    assert_eq!(completion.appended_coordinate_ordinals(), &[1, 2, 3, 6, 8]);
    assert_eq!(completion.rank_progression(), &[5, 6, 7, 8, 9, 10]);
    assert_eq!(completion.stats().rank_tests(), 10);
    assert_eq!(completion.stats().appended_isps(), 5);
    assert!(completion.stats().rank_operations() > 0);
    completion.replay().unwrap();
    let family = completion.into_family();
    assert_eq!(family.loop_count(), 4);
    assert_eq!(family.denominator_count(), BASIS);
    assert_eq!(family.coordinates(), &EXPECTED_COORDINATES);

    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    assert_eq!(rows.len(), 16, "L(L+E)=16 generated ordinary IBPs");
    assert!(generated.lorentz_invariance().is_empty());

    // Keep every sector admissible.  A sector pattern restricts the domain; it
    // does not prove omitted sectors zero.  The concrete LiteRed-style
    // quotient may need physical pinches and negative powers of the generated
    // ISP rows as numerator intermediates before they cancel.  Analytically
    // scaleless sectors are still removed by the generic zero-sector proof.
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
    let master = key([1, 1, 1, 1, 1, 0, 0, 0, 0, 0]);
    let dotted_tadpole = key([1, 1, 1, 1, 2, 0, 0, 0, 0, 0]);
    let provider = MasterPolicyProvider::with_selected(provider, [master.clone()]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    let scalar = engine.reduce(&dotted_tadpole).unwrap();
    scalar.require_complete().unwrap();
    assert_eq!(scalar.terms().len(), 1);
    // For D=k3^2-m2,
    //   0 = integral d/dk3 . (k3 / D^a)
    //     = (d-2a) I(a) - 2a*m2 I(a+1).
    // At a=1 this gives the frozen factorized oracle below.  The production
    // engine sees only the generated 16 IBPs, never this formula.
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

    // k3_mu k3_nu on the dotted tadpole factor projects to
    // g_mu_nu*k3^2/d.  With k3^2=D5+m2 and the generated dot rule above, the
    // exact coefficient is 1/2 times the undotted B4*T1 master.
    let tensor_source = CovariantTensorMonomial::try_from_parts_with_limits(
        [
            IndexedVector::new(LoopVector::new(3), LorentzIndex::new(40)),
            IndexedVector::new(LoopVector::new(3), LorentzIndex::new(41)),
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
        AuthenticatedVacuumCovariantTensorLowering::try_new(projection, &family, &dotted_tadpole)
            .unwrap();
    let tensor = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant(lowering, &mut engine)
        .unwrap();
    tensor.require_complete().unwrap();
    let metric = TensorCovariantStructure::new(
        MetricPairing::new([Metric::new(LorentzIndex::new(40), LorentzIndex::new(41))]),
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
    // The adaptive K(n) candidate provider is present only as the generic
    // fallback.  Zero work here authenticates that this rung was derived by
    // the concrete pre-quotient elimination of the freshly generated rows.
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
