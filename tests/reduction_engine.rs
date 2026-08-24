use rustred::reduction_engine::{
    ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus,
};
use rustred::{
    AffineDenominator, CoefficientContext, ConcreteIntegralKey, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricElimination,
    ParametricEliminationLimits, ParametricEliminationOrdering, ParametricIbpGenerator,
    ParametricReductionEngine, ParametricReductionRule, ParametricRelation,
    ParametricRuleApplication, ParametricRuleError, ParametricRuleLimits, ReductionEngineError,
    ReductionEngineLimits, SectorMask,
};
use std::convert::Infallible;
use std::sync::Arc;

fn tadpole() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "reduction-engine-tadpole",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parameter("m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

struct GeneratedTadpoleProvider<'a> {
    context: &'a ParametricCoefficientContext,
    rule: &'a ParametricReductionRule,
}

impl ConcreteRuleProvider for GeneratedTadpoleProvider<'_> {
    type Error = ParametricRuleError;

    fn index_arity(&self) -> usize {
        self.context.index_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        Ok(match self.rule.apply(self.context, integral.powers())? {
            ParametricRuleApplication::Applicable(reduction) => {
                ConcreteRuleDecision::Reduction(reduction)
            }
            ParametricRuleApplication::Inapplicable(_)
            | ParametricRuleApplication::Undecidable(_) => {
                ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
            }
        })
    }
}

fn generated_rule<'a>(
    context: &'a ParametricCoefficientContext,
    rows: &'a [ParametricRelation],
    elimination: &'a ParametricElimination,
) -> ParametricReductionRule {
    ParametricReductionRule::try_from_elimination_pivot(
        context,
        rows,
        elimination,
        0,
        SectorMask::try_new([true]).unwrap(),
        ParametricRuleLimits::default(),
    )
    .unwrap()
}

#[test]
fn demand_driven_engine_reduces_generated_tadpole_rule_to_uncovered_leaf() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let elimination = ParametricElimination::build(
        generated.context(),
        &rows,
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    let rule = generated_rule(generated.context(), &rows, &elimination);
    let provider = GeneratedTadpoleProvider {
        context: generated.context(),
        rule: &rule,
    };
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let source = ConcreteIntegralKey::try_new([4]).unwrap();
    let result = engine.reduce(&source).unwrap();
    assert_eq!(result.terms().len(), 1);
    let (leaf, coefficient) = result.terms().first_key_value().unwrap();
    assert_eq!(leaf.powers(), &[1]);
    assert_eq!(
        coefficient,
        &family
            .coefficient_context()
            .parse("(6-d)*(4-d)*(2-d)/(48*m2^3)")
            .unwrap()
    );
    assert!(result.required_nonzero().iter().any(|guard| {
        guard
            .polynomial()
            .to_expression()
            .to_string()
            .contains("m2")
    }));
    assert_eq!(result.stats().rule_applications(), 3);
    assert_eq!(result.stats().cache_entries(), 4);
    assert_eq!(
        result
            .uncovered_leaves()
            .iter()
            .map(|key| key.powers())
            .collect::<Vec<_>>(),
        vec![&[1][..]]
    );
    assert!(result.selected_masters().is_empty());
    assert!(result.certified_masters().is_empty());
    assert_eq!(
        result.require_complete().unwrap_err().uncovered_leaves(),
        result.uncovered_leaves()
    );

    let repeated = engine.reduce(&source).unwrap();
    assert_eq!(repeated.terms(), result.terms());
    assert!(repeated.stats().cache_hits() >= 1);
}

#[test]
fn demand_driven_engine_fails_typed_and_without_caching_partial_parent() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let elimination = ParametricElimination::build(
        generated.context(),
        &rows,
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    let rule = generated_rule(generated.context(), &rows, &elimination);
    let provider = GeneratedTadpoleProvider {
        context: generated.context(),
        rule: &rule,
    };
    let mut limits = ReductionEngineLimits::default();
    limits.max_rule_applications = 1;
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        limits,
    );
    let source = ConcreteIntegralKey::try_new([3]).unwrap();
    assert!(matches!(
        engine.reduce(&source),
        Err(ReductionEngineError::ResourceLimit {
            resource: "rule applications",
            ..
        })
    ));
    assert_eq!(engine.cache_len(), 0);
    assert_eq!(engine.stats(), Default::default());

    // A failed top-level call did not spend engine counters or retain its
    // successfully computed descendants. Provider-internal state is outside
    // this transaction, as documented by the engine contract.
    let recovered = engine
        .reduce(&ConcreteIntegralKey::try_new([1]).unwrap())
        .unwrap();
    assert_eq!(recovered.stats().recursive_calls(), 1);
    assert_eq!(recovered.stats().rule_applications(), 0);
    assert_eq!(recovered.stats().cache_entries(), 1);
}

#[derive(Clone)]
struct MutableTerminalProvider {
    arity: usize,
    status: ConcreteTerminalStatus,
    requests: usize,
}

impl ConcreteRuleProvider for MutableTerminalProvider {
    type Error = Infallible;

    fn index_arity(&self) -> usize {
        self.arity
    }

    fn decision_for(
        &mut self,
        _integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.requests += 1;
        Ok(ConcreteRuleDecision::Terminal(self.status.clone()))
    }
}

#[test]
fn terminal_statuses_are_explicit_and_provider_mutation_invalidates_cache() {
    let context = CoefficientContext::new(Vec::<String>::new());
    let provider = MutableTerminalProvider {
        arity: 1,
        status: ConcreteTerminalStatus::Uncovered,
        requests: 0,
    };
    let mut engine = ParametricReductionEngine::new(
        "terminal-status-family",
        &context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let source = ConcreteIntegralKey::try_new([1]).unwrap();

    let uncovered = engine.reduce(&source).unwrap();
    assert!(uncovered.uncovered_leaves().contains(&source));
    assert!(uncovered.require_complete().is_err());
    assert_eq!(uncovered.stats().maximum_result_terms(), 1);
    assert_eq!(engine.cache_len(), 1);

    engine.provider_mut().status = ConcreteTerminalStatus::SelectedMaster;
    assert_eq!(engine.cache_len(), 0);
    let selected = engine.reduce(&source).unwrap();
    assert!(selected.uncovered_leaves().is_empty());
    assert!(selected.selected_masters().contains(&source));
    selected.require_complete().unwrap();

    let certificate: Arc<str> = Arc::from("master-certificate-v1");
    engine.provider_mut().status = ConcreteTerminalStatus::CertifiedMaster {
        certificate_fingerprint: certificate.clone(),
    };
    let certified = engine.reduce(&source).unwrap();
    assert_eq!(
        certified.certified_masters().get(&source),
        Some(&certificate)
    );
    certified.require_complete().unwrap();
    assert_eq!(engine.provider().requests, 3);
}

#[test]
fn wrong_arity_is_rejected_before_querying_or_caching_the_provider() {
    let context = CoefficientContext::new(Vec::<String>::new());
    let provider = MutableTerminalProvider {
        arity: 1,
        status: ConcreteTerminalStatus::Uncovered,
        requests: 0,
    };
    let mut engine = ParametricReductionEngine::new(
        "arity-family",
        &context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let error = engine
        .reduce(&ConcreteIntegralKey::try_new([1, 1]).unwrap())
        .unwrap_err();
    assert!(matches!(
        error,
        ReductionEngineError::WrongArity {
            expected: 1,
            actual: 2
        }
    ));
    assert_eq!(engine.provider().requests, 0);
    assert_eq!(engine.cache_len(), 0);
    assert_eq!(engine.stats(), Default::default());
}

#[test]
fn uncovered_leaf_honors_term_limit_and_failure_is_atomic() {
    let context = CoefficientContext::new(Vec::<String>::new());
    let provider = MutableTerminalProvider {
        arity: 1,
        status: ConcreteTerminalStatus::Uncovered,
        requests: 0,
    };
    let mut limits = ReductionEngineLimits::default();
    limits.max_terms_per_result = 0;
    let mut engine = ParametricReductionEngine::new(
        "terminal-limit-family",
        &context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        limits,
    );
    let error = engine
        .reduce(&ConcreteIntegralKey::try_new([1]).unwrap())
        .unwrap_err();
    assert!(matches!(
        error,
        ReductionEngineError::ResourceLimit {
            resource: "terms per result",
            requested: 1,
            limit: 0
        }
    ));
    assert_eq!(engine.cache_len(), 0);
    assert_eq!(engine.stats(), Default::default());
}

#[test]
fn active_depth_limit_prevents_stack_growth_and_engine_recovers() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let elimination = ParametricElimination::build(
        generated.context(),
        &rows,
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    let rule = generated_rule(generated.context(), &rows, &elimination);
    let provider = GeneratedTadpoleProvider {
        context: generated.context(),
        rule: &rule,
    };
    let mut limits = ReductionEngineLimits::default();
    limits.max_active_depth = 2;
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        limits,
    );

    let error = engine
        .reduce(&ConcreteIntegralKey::try_new([4]).unwrap())
        .unwrap_err();
    assert!(matches!(
        error,
        ReductionEngineError::ResourceLimit {
            resource: "active depth",
            requested: 3,
            limit: 2
        }
    ));
    assert_eq!(engine.cache_len(), 0);
    assert_eq!(engine.stats(), Default::default());

    let recovered = engine
        .reduce(&ConcreteIntegralKey::try_new([1]).unwrap())
        .unwrap();
    assert!(
        recovered
            .uncovered_leaves()
            .contains(&ConcreteIntegralKey::try_new([1]).unwrap())
    );
    assert_eq!(recovered.stats().recursive_calls(), 1);
}

fn tadpole_engine_with_limits<'a>(
    family: &'a IntegralFamily,
    context: &'a ParametricCoefficientContext,
    rule: &'a ParametricReductionRule,
    limits: ReductionEngineLimits,
) -> ParametricReductionEngine<'a, GeneratedTadpoleProvider<'a>> {
    ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedTadpoleProvider { context, rule },
        limits,
    )
}

#[test]
fn aggregate_cached_trace_limit_bounds_quadratic_prefix_retention_transactionally() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let elimination = ParametricElimination::build(
        generated.context(),
        &rows,
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    let rule = generated_rule(generated.context(), &rows, &elimination);
    let source = ConcreteIntegralKey::try_new([4]).unwrap();

    // Cached prefixes J1..J4 own 0+1+2+3 proof traces. A per-result limit
    // alone would permit every prefix while retaining six aggregate entries.
    let mut too_small = ReductionEngineLimits::default();
    too_small.max_cached_application_trace_entries = 5;
    let mut engine = tadpole_engine_with_limits(&family, generated.context(), &rule, too_small);
    assert!(matches!(
        engine.reduce(&source),
        Err(ReductionEngineError::ResourceLimit {
            resource: "cached application trace entries",
            requested: 6,
            limit: 5,
        })
    ));
    assert_eq!(engine.cache_len(), 0);
    assert_eq!(engine.stats(), Default::default());

    let mut exact_boundary = ReductionEngineLimits::default();
    exact_boundary.max_cached_application_trace_entries = 6;
    let mut engine =
        tadpole_engine_with_limits(&family, generated.context(), &rule, exact_boundary);
    let result = engine.reduce(&source).unwrap();
    assert_eq!(result.cached_application_trace_entries(), 6);
    assert_eq!(result.stats().cached_application_trace_entries(), 6);
}

#[test]
fn aggregate_cached_proof_byte_limit_is_transactional() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let elimination = ParametricElimination::build(
        generated.context(),
        &rows,
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    let rule = generated_rule(generated.context(), &rows, &elimination);
    let mut limits = ReductionEngineLimits::default();
    limits.max_cached_proof_debug_bytes = 0;
    let mut engine = tadpole_engine_with_limits(&family, generated.context(), &rule, limits);
    let source = ConcreteIntegralKey::try_new([2]).unwrap();
    assert!(matches!(
        engine.reduce(&source),
        Err(ReductionEngineError::ResourceLimit {
            resource: "cached proof debug bytes",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(engine.cache_len(), 0);
    assert_eq!(engine.stats(), Default::default());
}
