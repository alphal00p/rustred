//! Frozen, FORM-free alphaLoop one-loop scalar oracle.
//!
//! The expected recurrence is read from Vakint's checked-in
//! `form_src/alphaloop/integrateduv.frm`, but the rules exercised here are
//! freshly derived from RustRed's generic `IntegralFamily` IBP rows.

use rustred::{
    AffineDenominator, CertifiedRewriteLimits, CertifiedZeroSectorRuleProvider, CoefficientContext,
    ConcreteIntegralKey, GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    IndexSpace, IntegralFamily, IntegralOrderingPolicy, MasterPolicyProvider,
    ParametricCoefficientContext, ParametricIbpGenerator, ParametricReductionEngine,
    ParametricSectorCoverageCertificate, ParametricSectorRuleProvider,
    ParametricSectorRuleProviderLimits, PowerShiftPolicy, ReductionEngineLimits, SectorMask,
};

fn vakint_one_loop_vacuum() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "vakint-frozen-one-loop-vacuum",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        // Vakint's numerator identity is k^2 = D1 + mUV^2.
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn key(power: i64) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new([power]).unwrap()
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

#[test]
fn generated_parametric_ibp_is_exactly_the_vakint_one_loop_oracle_identity() {
    let family = vakint_one_loop_vacuum();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    assert_eq!(generated.ordinary_ibp().len(), 1);
    assert!(generated.lorentz_invariance().is_empty());

    let context = generated.context();
    let row = &generated.ordinary_ibp()[0];
    let space = IndexSpace::try_new(1).unwrap();
    let zero = space.zero();
    let raised = space.unit(0, 1).unwrap();
    let n = context.index(0).unwrap();
    let two_n = context.mul(&context.integer(2), &n).unwrap();
    let d = context.lift(family.dimension()).unwrap();
    let m2 = context
        .lift(&family.coefficient_context().parameter("m2").unwrap())
        .unwrap();

    // 0 = (d-2 n) I(n) - 2 n m^2 I(n+1).
    assert_eq!(
        row.terms().get(&zero).unwrap(),
        &context.sub(&d, &two_n).unwrap()
    );
    assert_eq!(
        row.terms().get(&raised).unwrap(),
        &context.neg(&context.mul(&two_n, &m2).unwrap()).unwrap()
    );
    assert_eq!(row.terms().len(), 2);
}

#[test]
fn freshly_discovered_rules_match_vakint_integrate_uv_1l_for_powers_one_to_six() {
    let family = vakint_one_loop_vacuum();
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

    // Vakint/alphaLoop oracle:
    // I(n) = (d+2-2n)/(2(n-1)m^2) I(n-1), n>1.
    let expected = [
        "1",
        "(d-2)/(2*m2)",
        "(d-4)*(d-2)/(8*m2^2)",
        "(d-6)*(d-4)*(d-2)/(48*m2^3)",
        "(d-8)*(d-6)*(d-4)*(d-2)/(384*m2^4)",
        "(d-10)*(d-8)*(d-6)*(d-4)*(d-2)/(3840*m2^5)",
    ];
    for (offset, expected) in expected.into_iter().enumerate() {
        let power = i64::try_from(offset).unwrap() + 1;
        let result = engine.reduce(&key(power)).unwrap();
        result.require_complete().unwrap();
        assert!(result.uncovered_leaves().is_empty());
        assert_eq!(result.selected_masters().len(), 1);
        assert!(result.selected_masters().contains(&key(1)));
        assert_eq!(result.terms().len(), 1);
        assert_eq!(
            result.terms().get(&key(1)).unwrap(),
            &family.coefficient_context().parse(expected).unwrap(),
            "alphaLoop one-loop recurrence mismatch at power {power}",
        );
        if power > 1 {
            assert!(result.required_nonzero().iter().any(|condition| {
                condition
                    .polynomial()
                    .to_expression()
                    .to_string()
                    .contains("m2")
            }));
        }
    }

    for power in [0, -1, -2] {
        let result = engine.reduce(&key(power)).unwrap();
        result.require_complete().unwrap();
        assert!(result.terms().is_empty());
        assert!(result.terminal_statuses().is_empty());
    }

    let applications = engine.provider().inner().inner().stats();
    assert_eq!(applications.queries(), 5);
    assert_eq!(applications.reductions(), 5);
    assert_eq!(applications.uncovered(), 0);
    assert_eq!(applications.unsupported(), 0);
}
