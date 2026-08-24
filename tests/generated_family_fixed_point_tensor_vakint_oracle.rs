//! Symbolica numerator -> tensor projection -> fixed-point generated-IBP oracle.
//!
//! This focused one-loop integration test covers three numerator structures in
//! one Symbolica expression:
//!
//! - the odd rank-one contraction `k(rho) p(1,rho)`, which must vanish by
//!   vacuum isotropy;
//! - the free rank-two tensor `k(mu) k(nu)`, which projects onto `g(mu,nu)`;
//! - the free rank-four tensor `k(a) k(b) k(c) k(e)`, which projects onto the
//!   three metric pairings.
//!
//! All even tensors multiply `I(4)`.  Consequently tensor lowering produces
//! scalar integrals through `I(2)`, `I(3)`, and `I(4)`, and those integrals are
//! reduced by `GeneratedFamilyFixedPointProvider` rules generated solely from
//! the declared family.  The expected coefficients are the FORM-free frozen
//! alphaLoop/Vakint one-loop oracle used elsewhere in this test suite; no
//! production recurrence, FORM invocation, or topology-specific rule is used.

use rustred::*;
use symbolica::{atom::Atom, try_parse};

fn parse_atom(input: &str) -> Atom {
    try_parse!(
        input,
        default_namespace = "rustred_fixed_point_tensor_oracle"
    )
    .unwrap()
}

fn family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2", "A", "B", "C"]);
    IntegralFamily::new(
        "fixed-point-provider-vakint-one-loop-tensor",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        // Vakint convention: k.k = D1 + mUV^2.
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

fn fixed_point_certificate(
    family: &IntegralFamily,
) -> (
    ParametricCoefficientContext,
    GeneratedFamilyFixedPointCertificate,
) {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    let mut base_limits = GeneratedFamilyRuleSystemLimits::default();
    base_limits.discovery.adaptive.max_search_depth = 0;
    base_limits.live_leaf_queue.translation_radius = 0;
    base_limits.live_leaf_queue.max_translation_points = 1;
    let base = GeneratedFamilyRuleSystemCompiler::compile(
        family,
        &context,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        base_limits,
    )
    .unwrap();
    let certificate = GeneratedFamilyFixedPointCompiler::compile(
        family,
        &context,
        base,
        GeneratedFamilyFixedPointConfig {
            base_search_depth: 0,
            maximum_rounds: 1,
            residual_frontier_depth: 1,
            residual_anchor_local_depth: 0,
            maximum_local_depth: 1,
            selection: GeneratedFamilyFixedPointSelectionPolicy::AllResidualSubsectorFirst,
            stop_on_no_strict_improvement: false,
        },
        GeneratedFamilyFixedPointLimits::default(),
    )
    .unwrap();
    certificate.replay(family, &context).unwrap();
    (context, certificate)
}

fn coefficient_for_rendered_covariant<'a>(
    compiled: &CompiledSymbolicaTensorNumerator,
    result: &'a AuthenticatedVacuumCovariantTensorPolynomialParametricReduction,
    rendered_covariant: &str,
) -> &'a Coefficient {
    let target = parse_atom(rendered_covariant);
    let (_, terms) = result
        .scalar_reduction()
        .structures()
        .iter()
        .find(|(covariant, _)| compiled.render_covariant(covariant).unwrap() == target)
        .unwrap_or_else(|| panic!("missing reduced covariant {target}"));
    assert_eq!(terms.len(), 1);
    terms.get(&key(1)).unwrap().coefficient()
}

#[test]
fn symbolica_rank_one_two_and_four_numerators_reduce_through_fixed_point_rules() {
    let family = family();
    let coefficients = family.coefficient_context();
    let compiler = SymbolicaTensorNumeratorCompiler::try_new(
        &family,
        SymbolicaTensorSyntax::vakint().unwrap(),
        [("k".to_owned(), parse_atom("vakint::k(3)"))],
        SymbolicaTensorNumeratorLimits::default(),
    )
    .unwrap();

    let mu = "user_space::mink4(4,11)";
    let nu = "user_space::mink4(4,22)";
    let rho = "user_space::mink4(4,33)";
    let a = "user_space::mink4(4,41)";
    let b = "user_space::mink4(4,42)";
    let c = "user_space::mink4(4,43)";
    let e = "user_space::mink4(4,44)";
    let source = parse_atom(&format!(
        "rustred::A*vakint::k(3,{mu})*vakint::k(3,{nu})\
         +rustred::B*vakint::k(3,{rho})*vakint::p(1,{rho})\
         +rustred::C*vakint::k(3,{a})*vakint::k(3,{b})\
          *vakint::k(3,{c})*vakint::k(3,{e})"
    ));
    let compiled = compiler.compile(source.as_view()).unwrap();
    compiled.verify_replay(&compiler).unwrap();

    let (parametric_context, certificate) = fixed_point_certificate(&family);
    let provider = GeneratedFamilyFixedPointProvider::try_with_selected(
        &family,
        &parametric_context,
        certificate,
        [key(1)],
        GeneratedFamilyFixedPointProviderLimits::default(),
    )
    .unwrap();
    provider.replay().unwrap();
    assert_eq!(provider.terminals().len(), 1);

    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        coefficients,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let projection = compiled
        .project(&family, GenericTensorPolynomialLimits::default())
        .unwrap();
    projection.verify(&family).unwrap();
    let lowering = projection.lower(&family, &key(4)).unwrap();
    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant_polynomial(lowering, &mut engine)
        .unwrap();
    result.require_complete().unwrap();
    result.verify(&family).unwrap();
    result.verify_with_engine(&family, &mut engine).unwrap();

    // The odd rank-one term proportional to B is absent.  The four retained
    // structures are one rank-two metric and the three rank-four pairings.
    assert_eq!(result.scalar_reduction().len(), 4);

    // alphaLoop: (k^2) I(4) / d = (d-4)(d-2)/(48 m2^2) I(1).
    let rank_two = coefficients.parse("A*(d-4)*(d-2)/(48*m2^2)").unwrap();
    assert_eq!(
        coefficient_for_rendered_covariant(&compiled, &result, &format!("vakint::g({mu},{nu})"),),
        &rank_two,
    );

    // alphaLoop: (k^2)^2 I(4) / (d(d+2)) = (d-2)/(48 m2) I(1).
    let rank_four = coefficients.parse("C*(d-2)/(48*m2)").unwrap();
    for pairing in [
        format!("vakint::g({a},{b})*vakint::g({c},{e})"),
        format!("vakint::g({a},{c})*vakint::g({b},{e})"),
        format!("vakint::g({a},{e})*vakint::g({b},{c})"),
    ] {
        assert_eq!(
            coefficient_for_rendered_covariant(&compiled, &result, &pairing),
            &rank_four,
        );
    }
}
