//! Frozen, FORM-free Vakint one-loop tensor-plus-IBP oracle.
//!
//! The expected tensor structures come from
//! `vendor/gammaloop/crates/vakint/tests/tensor_reduction_tests.rs`; the
//! scalar identity comes from `form_src/alphaloop/integrateduv.frm`.  Neither
//! file is executed.  RustRed projects and lowers each independently encoded
//! tensor monomial, then applies freshly generated generic IBPs.  The complete
//! Vakint B input is a sum, so its independently authenticated pieces are
//! compared here without claiming that RustRed yet has a sum-level projector
//! certificate.

use rustred::*;

fn family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "vakint-frozen-one-loop-tensor-ibp",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        // Vakint convention: k^2 = D1 + mUV^2.
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

fn loop_vector(index: u32) -> IndexedVector {
    IndexedVector::new(LoopVector::new(0), LorentzIndex::new(index))
}

fn spectator(vector: u32, index: u32) -> IndexedSpectatorVector {
    IndexedSpectatorVector::new(SpectatorVector::new(vector), LorentzIndex::new(index))
}

fn generated_active_coverage(
    family: &IntegralFamily,
) -> (
    ParametricCoefficientContext,
    ParametricSectorCoverageCertificate,
) {
    let generated = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .generate()
        .unwrap();
    let context = generated.context().clone();
    let sector = SectorMask::try_new([true]).unwrap();
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        family,
        &context,
        sector.clone(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    discovery.replay(family, &context).unwrap();
    (context, discovery.coverage().clone())
}

fn reduce(
    source: CovariantTensorMonomial,
) -> (
    IntegralFamily,
    AuthenticatedVacuumCovariantTensorParametricReduction,
) {
    let family = family();
    let projection = GenericVacuumTensorProjector::new()
        .project_covariant(&family, &source)
        .unwrap();
    let lowering =
        AuthenticatedVacuumCovariantTensorLowering::try_new(projection, &family, &key(1)).unwrap();

    let (parametric_context, coverage) = generated_active_coverage(&family);
    let sector_provider = ParametricSectorRuleProvider::try_new(
        &family,
        &parametric_context,
        [coverage],
        ParametricSectorRuleProviderLimits::default(),
    )
    .unwrap();
    let master_provider = MasterPolicyProvider::with_selected(sector_provider, [key(1)]).unwrap();
    let provider = CertifiedZeroSectorRuleProvider::try_unrestricted(
        &family,
        PowerShiftPolicy::FormalGeneric,
        master_provider,
        CertifiedRewriteLimits::default(),
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant(lowering, &mut engine)
        .unwrap();
    result.require_complete().unwrap();
    result.verify(&family).unwrap();
    result.verify_with_engine(&family, &mut engine).unwrap();
    (family, result)
}

#[test]
fn vakint_a_even_and_odd_pieces_reduce_to_the_unreplaced_master() {
    let limits = GenericTensorProjectorLimits::default();
    let even = CovariantTensorMonomial::try_from_parts_with_limits(
        [loop_vector(1), loop_vector(2)],
        [],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let (family, even) = reduce(even);
    let metric = TensorCovariantStructure::new(
        MetricPairing::new([Metric::new(LorentzIndex::new(1), LorentzIndex::new(2))]),
        Vec::new(),
        SpectatorScalarProductMonomial::one(),
    );
    assert_eq!(even.scalar_reduction().len(), 1);
    assert_eq!(
        even.scalar_reduction()
            .term(&metric, &key(1))
            .unwrap()
            .coefficient(),
        &family.coefficient_context().parse("m2/d").unwrap()
    );

    // The second summand in Vakint fixture A is k(rho) p1(rho) and vanishes
    // before scalar reduction because its loop-tensor rank is odd.
    let odd = CovariantTensorMonomial::try_from_parts_with_limits(
        [loop_vector(3)],
        [spectator(1, 3)],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let (_, odd) = reduce(odd);
    assert!(odd.scalar_reduction().is_zero());
    assert!(odd.scalar_reduction().selected_masters().is_empty());
}

#[test]
fn vakint_b_spectator_and_scalar_quartic_outputs_reduce_exactly() {
    let limits = GenericTensorProjectorLimits::default();
    let mixed = CovariantTensorMonomial::try_from_parts_with_limits(
        [loop_vector(1), loop_vector(2)],
        [spectator(2, 1), spectator(3, 2)],
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let (family, mixed) = reduce(mixed);
    let spectator_product = SpectatorScalarProductMonomial::try_from_factors_with_limits(
        [(
            SpectatorScalarProduct::new(SpectatorVector::new(2), SpectatorVector::new(3)),
            1,
        )],
        limits,
    )
    .unwrap();
    let mixed_structure =
        TensorCovariantStructure::new(MetricPairing::empty(), Vec::new(), spectator_product);
    assert_eq!(
        mixed
            .scalar_reduction()
            .term(&mixed_structure, &key(1))
            .unwrap()
            .coefficient(),
        &family.coefficient_context().parse("m2/d").unwrap()
    );

    // Vakint's first B output term is g(1,2)*(k.k)^2.  Feed that frozen
    // tensor-reduction output to the generic family/IBP stages: expanding
    // (D1+m2)^2 leaves m2^2 I(1), while I(0) and I(-1) are proved zero.
    let kk_squared = ScalarProductMonomial::try_from_factors_with_limits(
        [(
            ScalarProduct::new(LoopVector::new(0), LoopVector::new(0)),
            2,
        )],
        TensorConstructionLimits::default(),
    )
    .unwrap();
    let quartic = CovariantTensorMonomial::try_from_parts_with_limits(
        [],
        [],
        [Metric::new(LorentzIndex::new(1), LorentzIndex::new(2))],
        kk_squared,
        SpectatorScalarProductMonomial::one(),
        limits,
    )
    .unwrap();
    let (_, quartic) = reduce(quartic);
    let metric = TensorCovariantStructure::new(
        MetricPairing::new([Metric::new(LorentzIndex::new(1), LorentzIndex::new(2))]),
        Vec::new(),
        SpectatorScalarProductMonomial::one(),
    );
    assert_eq!(
        quartic
            .scalar_reduction()
            .term(&metric, &key(1))
            .unwrap()
            .coefficient(),
        &family.coefficient_context().parse("m2^2").unwrap()
    );
}
