use std::collections::BTreeMap;

use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits, AffineDenominator,
    AuthenticatedVacuumCovariantTensorLowering, CertifiedConcreteRewriteProof,
    CertifiedFamilyRuleProvider, CertifiedFamilyRuleProviderLimits, Coefficient,
    CoefficientContext, ConcreteIntegralKey, ConcreteRuleApplicationTrace, CovariantTensorMonomial,
    GenericTensorProjectorLimits, GenericVacuumTensorProjector, IndexedVector, IntegralFamily,
    IntegralOrderingPolicy, InternalSymmetrySearchLimits, LoopVector, LorentzIndex,
    MasterPolicyProvider, Metric, MetricPairing, ParametricIbpGenerator, ParametricReductionEngine,
    ReductionEngineLimits, ScalarProductMonomial, SectorRestrictions,
    SpectatorScalarProductMonomial, TensorCovariantStructure, TensorParametricReductionComposer,
    discover_bounded_vacuum_internal_symmetries,
};

fn equal_mass_tetrahedron_family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let z = coefficients.zero();
    let o = coefficients.one();
    let n2 = coefficients.integer(-2);
    let minus_m2 = coefficients.parse("-m2").unwrap();
    let affine = |constant, coefficients| AffineDenominator::new(constant, coefficients);
    IntegralFamily::new(
        "certified-generic-equal-mass-three-loop-tetrahedron",
        vec!["k1".into(), "k2".into(), "k3".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        // Routings k1,k2,k3,k3-k1,k1-k2,k2-k3. Coordinate order is
        // k1^2,k1.k2,k1.k3,k2^2,k2.k3,k3^2.
        vec![
            affine(
                minus_m2.clone(),
                vec![
                    o.clone(),
                    z.clone(),
                    z.clone(),
                    z.clone(),
                    z.clone(),
                    z.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                vec![
                    z.clone(),
                    z.clone(),
                    z.clone(),
                    o.clone(),
                    z.clone(),
                    z.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                vec![
                    z.clone(),
                    z.clone(),
                    z.clone(),
                    z.clone(),
                    z.clone(),
                    o.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                vec![
                    o.clone(),
                    z.clone(),
                    n2.clone(),
                    z.clone(),
                    z.clone(),
                    o.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                vec![
                    o.clone(),
                    n2.clone(),
                    z.clone(),
                    o.clone(),
                    z.clone(),
                    z.clone(),
                ],
            ),
            affine(
                minus_m2,
                vec![z.clone(), z.clone(), z.clone(), o, n2, coefficients.one()],
            ),
        ],
        Vec::new(),
        vec![z.clone(), z.clone(), z.clone(), z.clone(), z.clone(), z],
    )
    .unwrap()
}

fn key(powers: [i64; 6]) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

fn expected(
    context: &CoefficientContext,
    terms: impl IntoIterator<Item = (ConcreteIntegralKey, &'static str)>,
) -> BTreeMap<ConcreteIntegralKey, Coefficient> {
    terms
        .into_iter()
        .map(|(master, coefficient)| (master, context.parse(coefficient).unwrap()))
        .collect()
}

fn canonical(
    source: ConcreteIntegralKey,
    symmetries: &[rustred::VerifiedInternalFamilyPermutationSymmetry],
) -> ConcreteIntegralKey {
    let ordering = IntegralOrderingPolicy::RustRedUnshiftedV1;
    symmetries.iter().fold(source.clone(), |best, symmetry| {
        let image = symmetry.transport_source_key(&source).unwrap();
        if ordering
            .compare(image.powers(), best.powers())
            .unwrap()
            .is_lt()
        {
            image
        } else {
            best
        }
    })
}

#[test]
fn generated_ibps_and_radius_one_symmetries_match_three_loop_dot_oracles() {
    let family = equal_mass_tetrahedron_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    assert_eq!(rows.len(), 9, "L(L+E)=9 generated ordinary IBPs");
    assert!(generated.lorentz_invariance().is_empty());

    let restrictions = SectorRestrictions::unrestricted(6).unwrap();
    let symmetry_report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(symmetry_report.completion().is_exhaustive_within_bounds());
    assert_eq!(
        symmetry_report.stats().enumerated_matrices(),
        3_usize.pow(9)
    );
    assert_eq!(symmetry_report.symmetries().len(), 24, "tetrahedron S4");
    assert_eq!(symmetry_report.stats().retained_symmetries(), 24);

    // Master policy is applied before provider discovery, while discovery
    // first canonicalizes every request. Select exactly the five requested
    // symmetry classes using the same ordering representatives, not all 120
    // labelled images and not the historical labels themselves.
    let p3 = canonical(key([1, 1, 1, 0, 0, 0]), symmetry_report.symmetries());
    let st = canonical(key([1, 1, 1, 1, 0, 0]), symmetry_report.symmetries());
    let b4 = canonical(key([1, 1, 0, 1, 0, 1]), symmetry_report.symmetries());
    let f5 = canonical(key([1, 1, 1, 1, 1, 0]), symmetry_report.symmetries());
    let m6 = canonical(key([1, 1, 1, 1, 1, 1]), symmetry_report.symmetries());
    assert_eq!(p3, key([0, 0, 1, 1, 0, 1]));
    assert_eq!(st, key([0, 0, 1, 1, 1, 1]));
    assert_eq!(b4, key([0, 1, 1, 1, 1, 0]));
    assert_eq!(f5, key([0, 1, 1, 1, 1, 1]));
    assert_eq!(m6, key([1, 1, 1, 1, 1, 1]));
    assert_eq!(
        [p3.clone(), st.clone(), b4.clone(), f5.clone(), m6.clone()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        5
    );

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
        symmetry_report.symmetries().iter().cloned(),
        adaptive,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CertifiedFamilyRuleProviderLimits::default(),
    )
    .unwrap();
    let provider = MasterPolicyProvider::with_selected(
        provider,
        [p3.clone(), st.clone(), b4.clone(), f5.clone(), m6.clone()],
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    // Primary oracle source (read only; FORM is never executed): Vakint's
    // `form_src/alphaloop/integrateduv.frm`. Its 3L topology/index mapping is
    // at lines 162-189, D=q^2-mUV^2 follows directly from the numerator rule
    // k^2 -> pinched + mUV^2*unpinched at lines 191-199, and d=4-2*ep is fixed
    // at lines 1105-1107. The relevant recursively applied dot blocks are
    // B4 lines 730-742, the two F5 edge orbits lines 827-850, and M6 lines
    // 1005-1023; the five terminal master classes are listed at 1170-1187.
    // For example, specializing line 837 gives
    // (1-2*ep)/(3*m2)=(d-3)/(3*m2), while line 849 gives
    // (-ep-1)/(3*m2)=(d-6)/(6*m2), distinguishing the two F5 orbits.
    // Homogeneity independently checks B4 and M6 and the F5 sum:
    //   4*B4dot=(3*d-8)/(2*m2)*B4,
    //   6*M6dot=3*(d-4)/(2*m2)*M6,
    //   F5central+4*F5outer=(3*d-10)/(2*m2)*F5.
    // The old `three_loop_pipeline` fixture used D=q^2+legacy_m2; its values
    // agree after legacy_m2 -> -m2 (odd inverse powers flip sign).
    let oracles = [
        (
            [2, 1, 0, 1, 0, 1],
            expected(
                family.coefficient_context(),
                [(b4.clone(), "(3*d-8)/(8*m2)")],
            ),
        ),
        (
            [2, 1, 1, 1, 1, 0],
            expected(
                family.coefficient_context(),
                [
                    (b4.clone(), "(8-3*d)/(6*m2^2)"),
                    (st.clone(), "2*(d-2)/(3*m2^2)"),
                    (f5.clone(), "(d-6)/(6*m2)"),
                ],
            ),
        ),
        (
            [1, 2, 1, 1, 1, 0],
            expected(
                family.coefficient_context(),
                [
                    (b4.clone(), "(3*d-8)/(24*m2^2)"),
                    (st.clone(), "(2-d)/(6*m2^2)"),
                    (f5.clone(), "(d-3)/(3*m2)"),
                ],
            ),
        ),
        (
            [2, 1, 1, 1, 1, 1],
            expected(family.coefficient_context(), [(m6.clone(), "(d-4)/(4*m2)")]),
        ),
    ];

    let mut retained_results = Vec::new();
    for (source, oracle) in oracles {
        let result = engine
            .reduce(&key(source))
            .unwrap_or_else(|error| panic!("generic reduction failed for {source:?}: {error:?}"));
        result.require_complete().unwrap_or_else(|error| {
            panic!(
                "generic reduction remained uncovered for {source:?}: {:?}",
                error.uncovered_leaves()
            )
        });
        assert_eq!(result.terms(), &oracle, "wrong reduction for {source:?}");
        assert!(
            result.application_traces().iter().any(|trace| matches!(
                trace,
                ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite)
                    if matches!(
                        rewrite.proof(),
                        CertifiedConcreteRewriteProof::ConcreteQuotientElimination { .. }
                    )
            )),
            "{source:?} must retain a concrete pre-quotient elimination proof"
        );
        assert!(result.application_traces().iter().all(|trace| !matches!(
            trace,
            ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite)
                if matches!(
                    rewrite.proof(),
                    CertifiedConcreteRewriteProof::ParametricQuotient { .. }
                )
        )));
        retained_results.push(result);
    }

    // A nontrivial generic three-loop tensor fixture. Isotropy projects
    // k1_mu k1_nu to g_mu_nu*k1^2/d. On the dotted B4 source, denominator
    // lowering gives (B4 + m2*B4_dot)/d. Combining it with the independently
    // frozen B4_dot relation above yields exactly 3/8*g_mu_nu*B4.
    let tensor_source = CovariantTensorMonomial::try_from_parts_with_limits(
        [
            IndexedVector::new(LoopVector::new(0), LorentzIndex::new(10)),
            IndexedVector::new(LoopVector::new(0), LorentzIndex::new(11)),
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
    let lowering = AuthenticatedVacuumCovariantTensorLowering::try_new(
        projection,
        &family,
        &key([2, 1, 0, 1, 0, 1]),
    )
    .unwrap();
    let tensor_result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant(lowering, &mut engine)
        .unwrap();
    tensor_result.require_complete().unwrap();
    let metric = TensorCovariantStructure::new(
        MetricPairing::new([Metric::new(LorentzIndex::new(10), LorentzIndex::new(11))]),
        Vec::new(),
        SpectatorScalarProductMonomial::one(),
    );
    assert_eq!(tensor_result.scalar_reduction().len(), 1);
    assert_eq!(
        tensor_result
            .scalar_reduction()
            .term(&metric, &b4)
            .unwrap()
            .coefficient(),
        &family.coefficient_context().parse("3/8").unwrap()
    );
    assert!(
        tensor_result
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
    tensor_result
        .verify_with_engine(&family, &mut engine)
        .unwrap();

    drop(engine);
    for result in retained_results {
        assert!(!result.application_traces().is_empty());
        for trace in result.application_traces() {
            match trace {
                ConcreteRuleApplicationTrace::Parametric(proof) => assert!(
                    proof
                        .replay_application(&family, generated.context())
                        .unwrap()
                ),
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
    }
    tensor_result.verify(&family).unwrap();
}
