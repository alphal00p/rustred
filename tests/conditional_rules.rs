//! Condition-bound application tests.  Concrete powers appear only as
//! validation points; every relation is regenerated from the generic family.

use std::sync::Arc;

use rustred::{
    AffineDenominator, BasePolynomial, CoefficientContext, CoefficientLocation,
    ConcreteIntegralKey, ConcreteRuleApplicationTrace, ConcreteRuleDecision, ConcreteRuleProvider,
    ConcreteTerminalStatus, ConditionalParametricRule, ConditionalParametricRuleApplication,
    ConditionalParametricRuleError, ConditionalParametricRuleInapplicability,
    ConditionalParametricRuleLimits, GeneratedPartialReeliminationCompilation,
    GeneratedPartialReeliminationCompiler, GeneratedPartialReeliminationLimits,
    GenericScalarProductMonomial, GenericTensorFamilyReducer, GenericTensorNumerator,
    GenericTensorTerm, GuardOrigin, IndexSpace, IntegralFamily, IntegralOrderingPolicy,
    MetricPairing, ParametricCoefficientContext, ParametricEliminationOrdering,
    ParametricIbpGenerator, ParametricReductionEngine, PartialIndexAssignment,
    ReductionEngineLimits, SectorMask, TensorParametricReductionComposer,
};

fn one_loop_family(name: &str, guarded_dimension: bool) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let dimension = if guarded_dimension {
        coefficients.parse("d/m2").unwrap()
    } else {
        coefficients.parameter("d").unwrap()
    };
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        dimension,
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn context(family: &IntegralFamily) -> ParametricCoefficientContext {
    ParametricIbpGenerator::try_new(family)
        .unwrap()
        .generate()
        .unwrap()
        .context()
        .clone()
}

fn certificate(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Arc<rustred::GeneratedPartialReeliminationCertificate> {
    let ordering =
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap();
    let compilation = GeneratedPartialReeliminationCompiler::compile(
        family,
        context,
        &[IndexSpace::try_new(1).unwrap().zero()],
        PartialIndexAssignment::try_new([(0, 1)], 1, 1).unwrap(),
        ordering,
        GeneratedPartialReeliminationLimits::default(),
    )
    .unwrap();
    let GeneratedPartialReeliminationCompilation::Certified(certificate) = compilation else {
        panic!("the one-loop partial system must retain a pivot")
    };
    Arc::new(certificate)
}

fn active_rule(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    certificate: Arc<rustred::GeneratedPartialReeliminationCertificate>,
    limits: ConditionalParametricRuleLimits,
) -> ConditionalParametricRule {
    ConditionalParametricRule::try_from_certificate_pivot(
        family,
        context,
        certificate,
        0,
        SectorMask::try_new([true]).unwrap(),
        limits,
    )
    .unwrap()
}

#[test]
fn one_loop_conditional_pivot_applies_only_on_its_centered_locus_and_replays() {
    let family = one_loop_family("conditional-rule-one-loop", false);
    let context = context(&family);
    let rule = active_rule(
        &family,
        &context,
        certificate(&family, &context),
        ConditionalParametricRuleLimits::default(),
    );
    assert_eq!(rule.centered_assignment().entries(), &[(0, 2)]);
    rule.replay(&family, &context).unwrap();

    let ConditionalParametricRuleApplication::Applicable(reduction) =
        rule.apply(&context, &[2]).unwrap()
    else {
        panic!("n=2 is the authenticated centered locus")
    };
    assert_eq!(reduction.source().powers(), &[2]);
    assert_eq!(reduction.rhs().len(), 1);
    assert!(reduction.coordinate_rule().is_some());
    assert_eq!(reduction.sector(), rule.sector());
    assert_eq!(
        reduction.ordering_policy(),
        IntegralOrderingPolicy::RustRedUnshiftedV1
    );
    let debug = format!("{reduction:?}");
    assert!(debug.contains("CoordinateEquality(<redacted>)"));
    assert!(!debug.contains(ConditionalParametricRule::SCHEMA));
    let target = ConcreteIntegralKey::try_new([1]).unwrap();
    let coefficient = reduction.rhs().get(&target).unwrap();
    let expected = family.coefficient_context().parse("(d-2)/(2*m2)").unwrap();
    assert!(
        family
            .coefficient_context()
            .try_sub(coefficient, &expected, Default::default())
            .unwrap()
            .is_zero()
    );
    assert!(
        reduction
            .verify_application(
                family.coefficient_context(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Default::default(),
            )
            .unwrap()
    );
    let foreign_base = CoefficientContext::new(["x"]);
    assert!(
        !reduction
            .verify_application(
                &foreign_base,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Default::default(),
            )
            .unwrap(),
        "even a future empty-RHS conditional rule must reject a foreign base map",
    );
    reduction.replay(&family, &context).unwrap();

    assert!(matches!(
        rule.apply(&context, &[3]).unwrap(),
        ConditionalParametricRuleApplication::Inapplicable(
            ConditionalParametricRuleInapplicability::OutsideEqualityLocus {
                position: 0,
                expected: 2,
                actual: 3,
            }
        )
    ));
    assert!(matches!(
        rule.apply(&context, &[0]).unwrap(),
        ConditionalParametricRuleApplication::Inapplicable(
            ConditionalParametricRuleInapplicability::OutsideSector
        )
    ));
}

#[test]
fn centered_locus_cannot_be_rebound_to_a_contradictory_sector() {
    let family = one_loop_family("conditional-rule-sector", false);
    let context = context(&family);
    let certificate = certificate(&family, &context);
    assert!(matches!(
        ConditionalParametricRule::try_from_certificate_pivot(
            &family,
            &context,
            certificate,
            0,
            SectorMask::try_new([false]).unwrap(),
            ConditionalParametricRuleLimits::default(),
        ),
        Err(
            ConditionalParametricRuleError::EmptyConditionalSectorLocus {
                position: 0,
                value: 2,
                active: false,
            }
        )
    ));
}

#[test]
fn base_assumptions_survive_as_exact_concrete_guards() {
    let family = one_loop_family("conditional-rule-base-guard", true);
    let context = context(&family);
    let rule = active_rule(
        &family,
        &context,
        certificate(&family, &context),
        ConditionalParametricRuleLimits::default(),
    );
    let ConditionalParametricRuleApplication::Applicable(reduction) =
        rule.apply(&context, &[2]).unwrap()
    else {
        panic!("the guarded conditional pivot must apply")
    };
    let m2 = family.coefficient_context().parameter("m2").unwrap();
    let expected_m2 = BasePolynomial::try_from_raw(
        m2.numerator.clone(),
        family.coefficient_context(),
        Default::default(),
    )
    .unwrap();
    let guard = reduction
        .required_nonzero()
        .iter()
        .find(|condition| condition.polynomial() == &expected_m2)
        .expect("m2 != 0 must remain attached to the concrete rewrite");
    assert!(
        guard
            .origins()
            .contains(&GuardOrigin::FamilyInputCoefficientDenominator {
                location: CoefficientLocation::Dimension,
            })
    );
    assert!(guard.origins().iter().any(|origin| matches!(
        origin,
        GuardOrigin::PartialIndexSpecialization { assignments }
            if assignments.as_ref() == [(0, 1)]
    )));
    assert!(guard.origins().iter().any(|origin| matches!(
        origin,
        GuardOrigin::IndexSpecialization { assignment }
            if assignment.as_ref() == [2]
    )));
    reduction.replay(&family, &context).unwrap();
}

#[test]
fn concrete_guard_retention_budget_is_checked_before_durable_clone() {
    let family = one_loop_family("conditional-rule-guard-budget", true);
    let context = context(&family);
    let mut limits = ConditionalParametricRuleLimits::default();
    limits.max_required_nonzero_conditions = 0;
    let rule = active_rule(&family, &context, certificate(&family, &context), limits);
    assert!(matches!(
        rule.apply(&context, &[2]),
        Err(ConditionalParametricRuleError::ResourceLimit {
            resource: "required nonzero conditions",
            limit: 0,
            ..
        })
    ));
}

#[test]
fn pivot_scope_and_resource_failures_are_typed_and_fail_closed() {
    let family = one_loop_family("conditional-rule-adversarial", false);
    let context = context(&family);
    let certificate = certificate(&family, &context);
    assert!(matches!(
        ConditionalParametricRule::try_from_certificate_pivot(
            &family,
            &context,
            certificate.clone(),
            1,
            SectorMask::try_new([true]).unwrap(),
            ConditionalParametricRuleLimits::default(),
        ),
        Err(ConditionalParametricRuleError::PivotOutOfRange {
            pivot: 1,
            available: 1,
        })
    ));

    let mut limits = ConditionalParametricRuleLimits::default();
    limits.max_rhs_terms = 0;
    assert!(matches!(
        ConditionalParametricRule::try_from_certificate_pivot(
            &family,
            &context,
            certificate,
            0,
            SectorMask::try_new([true]).unwrap(),
            limits,
        ),
        Err(ConditionalParametricRuleError::ResourceLimit {
            resource: "conditional symbolic RHS terms",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn wrong_family_context_and_arity_never_replay_or_apply() {
    let family = one_loop_family("conditional-rule-scope", false);
    let context = context(&family);
    let rule = active_rule(
        &family,
        &context,
        certificate(&family, &context),
        ConditionalParametricRuleLimits::default(),
    );
    let wrong_family = one_loop_family("conditional-rule-other", false);
    assert!(matches!(
        rule.replay(&wrong_family, &context),
        Err(ConditionalParametricRuleError::WrongFamily)
    ));
    let wrong_context = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        "conditional-rule-wrong-scope",
        1,
    )
    .unwrap();
    assert!(matches!(
        rule.replay(&family, &wrong_context),
        Err(ConditionalParametricRuleError::WrongContext)
    ));
    assert!(matches!(
        rule.apply(&context, &[2, 1]),
        Err(ConditionalParametricRuleError::WrongArity {
            expected: 1,
            actual: 2,
        })
    ));
}

struct ConditionalProvider<'context> {
    context: &'context ParametricCoefficientContext,
    rule: ConditionalParametricRule,
}

impl ConcreteRuleProvider for ConditionalProvider<'_> {
    type Error = ConditionalParametricRuleError;

    fn index_arity(&self) -> usize {
        1
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        if integral.powers() == [1] {
            return Ok(ConcreteRuleDecision::Terminal(
                ConcreteTerminalStatus::SelectedMaster,
            ));
        }
        Ok(match self.rule.apply(self.context, integral.powers())? {
            ConditionalParametricRuleApplication::Applicable(reduction) => {
                ConcreteRuleDecision::ConditionalReduction(reduction)
            }
            ConditionalParametricRuleApplication::Inapplicable(_) => {
                ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
            }
        })
    }
}

#[test]
fn generic_reduction_engine_retains_conditional_proof_after_provider_drop() {
    let family = one_loop_family("conditional-rule-engine", false);
    let context = context(&family);
    let rule = active_rule(
        &family,
        &context,
        certificate(&family, &context),
        ConditionalParametricRuleLimits::default(),
    );
    let provider = ConditionalProvider {
        context: &context,
        rule,
    };
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = engine
        .reduce(&ConcreteIntegralKey::try_new([2]).unwrap())
        .unwrap();
    result.require_complete().unwrap();
    assert_eq!(result.application_traces().len(), 1);
    assert_eq!(result.selected_masters().len(), 1);
    drop(engine);

    let ConcreteRuleApplicationTrace::ConditionalParametric(reduction) =
        &result.application_traces()[0]
    else {
        panic!("the engine must retain a condition-bound trace variant")
    };
    assert_eq!(reduction.source().powers(), &[2]);
    reduction.replay(&family, &context).unwrap();
}

#[test]
fn tensor_certificate_replays_a_retained_conditional_scalar_trace() {
    let family = one_loop_family("conditional-rule-tensor-trace", false);
    let context = context(&family);
    let numerator = GenericTensorNumerator::try_new([GenericTensorTerm::new(
        family.coefficient_context().one(),
        MetricPairing::empty(),
        GenericScalarProductMonomial::one(),
    )])
    .unwrap();
    let source = ConcreteIntegralKey::try_new([2]).unwrap();
    let lowering = GenericTensorFamilyReducer::new(&family)
        .lower(&source, &numerator)
        .unwrap();
    let provider = ConditionalProvider {
        context: &context,
        rule: active_rule(
            &family,
            &context,
            certificate(&family, &context),
            ConditionalParametricRuleLimits::default(),
        ),
    };
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
    result.require_complete().unwrap();
    let witness = result.scalar_witnesses().get(&source).unwrap();
    assert!(matches!(
        witness.application_traces(),
        [ConcreteRuleApplicationTrace::ConditionalParametric(_)]
    ));
    result.verify_collected(&family).unwrap();
}
