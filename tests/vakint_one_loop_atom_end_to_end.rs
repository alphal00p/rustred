//! FORM-free Symbolica-Atom -> tensor projection -> generated-IBP validation.
//!
//! The fixtures are taken from Vakint's one-loop free-form/tensor tests. The
//! expected scalar recurrence is independently frozen from alphaLoop's
//! `IntegrateUV1L`; the master `I(1)` is deliberately left unsubstituted.

use rustred::*;
use symbolica::{atom::Atom, try_parse};

fn parse_atom(input: &str) -> Atom {
    try_parse!(input, default_namespace = "rustred_vakint_atom_e2e").unwrap()
}

fn family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2", "A", "B"]);
    IntegralFamily::new(
        "vakint-one-loop-atom-end-to-end",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
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

fn checked_mul(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
) -> Coefficient {
    context
        .try_mul(left, right, ExactAlgebraLimits::default())
        .unwrap()
}

fn checked_add(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
) -> Coefficient {
    context
        .try_add(left, right, ExactAlgebraLimits::default())
        .unwrap()
}

fn checked_div(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
) -> Coefficient {
    context
        .try_div(left, right, ExactAlgebraLimits::default())
        .unwrap()
}

/// Frozen alphaLoop one-loop recurrence with `I(1)` left as the master.
fn alphaloop_integral_coefficient(context: &CoefficientContext, power: i64) -> Coefficient {
    if power <= 0 {
        return context.zero();
    }
    let d = context.parameter("d").unwrap();
    let m2 = context.parameter("m2").unwrap();
    let mut result = context.one();
    for current in 2..=power {
        let numerator = checked_add(context, &d, &context.integer(2 - 2 * current));
        let denominator = checked_mul(context, &context.integer(2 * (current - 1)), &m2);
        result = checked_mul(
            context,
            &result,
            &checked_div(context, &numerator, &denominator),
        );
    }
    result
}

fn compiler(family: &IntegralFamily) -> SymbolicaTensorNumeratorCompiler {
    SymbolicaTensorNumeratorCompiler::try_new(
        family,
        SymbolicaTensorSyntax::vakint().unwrap(),
        [("k".to_owned(), parse_atom("vakint::k(3)"))],
        SymbolicaTensorNumeratorLimits::default(),
    )
    .unwrap()
}

fn generated_family_certificate(
    family: &IntegralFamily,
) -> (
    ParametricCoefficientContext,
    GeneratedFamilyRuleSystemCertificate,
) {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    let mut limits = GeneratedFamilyRuleSystemLimits::default();
    limits.live_leaf_queue.translation_radius = 0;
    limits.live_leaf_queue.max_translation_points = 1;
    let certificate = GeneratedFamilyRuleSystemCompiler::compile(
        family,
        &context,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        limits,
    )
    .unwrap();
    certificate.replay(family, &context).unwrap();
    (context, certificate)
}

#[test]
fn atom_tensor_sum_reduces_with_generated_ibps_to_the_vakint_master_basis() {
    let family = family();
    let context = family.coefficient_context();
    let compiler = compiler(&family);
    let mu = "user_space::mink4(4,11)";
    let nu = "user_space::mink4(4,22)";
    let source = parse_atom(&format!(
        "rustred::A*vakint::k(3,{mu})*vakint::k(3,{nu})\
         +rustred::B*vakint::k(3,user_space::mink4(4,77))\
          *vakint::p(1,user_space::mink4(4,77))"
    ));
    let compiled = compiler.compile(source.as_view()).unwrap();
    compiled.verify_replay(&compiler).unwrap();

    let (parametric_context, certificate) = generated_family_certificate(&family);
    let provider = GeneratedFamilyRuleSystemProvider::try_with_selected(
        &family,
        &parametric_context,
        certificate,
        [key(1)],
        GeneratedFamilyRuleSystemProviderLimits::default(),
    )
    .unwrap();
    provider.replay().unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    let projection = compiled
        .project(&family, GenericTensorPolynomialLimits::default())
        .unwrap();
    let rendered_projection = compiled.render_projected(projection.numerator()).unwrap();
    assert_eq!(
        rendered_projection,
        parse_atom(&format!(
            "rustred::A*rustred::d^-1*vakint::dot(vakint::k(3),vakint::k(3))\
             *vakint::g({mu},{nu})"
        ))
    );

    let lowering = projection.lower(&family, &key(3)).unwrap();
    let result = TensorParametricReductionComposer::new(&family)
        .reduce_authenticated_covariant_polynomial(lowering, &mut engine)
        .unwrap();
    result.require_complete().unwrap();
    result.verify(&family).unwrap();

    assert_eq!(result.scalar_reduction().len(), 1);
    let (covariant, terms) = result
        .scalar_reduction()
        .structures()
        .iter()
        .next()
        .unwrap();
    assert_eq!(terms.len(), 1);
    let reduced = terms.get(&key(1)).unwrap().coefficient();
    // k^2 I(3) = I(2) + m2 I(3), then generated IBPs reduce both terms.
    let expected_moment = checked_add(
        context,
        &alphaloop_integral_coefficient(context, 2),
        &checked_mul(
            context,
            &context.parameter("m2").unwrap(),
            &alphaloop_integral_coefficient(context, 3),
        ),
    );
    let expected = checked_mul(
        context,
        &context.parameter("A").unwrap(),
        &checked_div(context, &expected_moment, &context.parameter("d").unwrap()),
    );
    assert_eq!(reduced, &expected);
    assert_eq!(
        compiled.render_covariant(covariant).unwrap(),
        parse_atom(&format!("vakint::g({mu},{nu})"))
    );
}
