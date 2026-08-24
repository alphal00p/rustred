//! The provider fixtures use a concrete one-loop family only as an oracle.
//! Queue construction, conditional pivot discovery, and routing remain fully
//! generated and contain no recurrence formula or selected pivot.

use std::fmt;

use rustred::{
    AffineDenominator, CoefficientContext, ConcreteIntegralKey, ConcreteRuleDecision,
    ConcreteRuleProvider, ConcreteTerminalStatus, GeneratedSectorConditionalRuleProvider,
    GeneratedSectorConditionalRuleProviderError, GeneratedSectorConditionalRuleProviderLimits,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueCompiler,
    GeneratedSectorLiveLeafQueueLimits, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricIbpGenerator, ParametricSectorRuleProvider,
    ParametricSectorRuleProviderLimits, SectorMask,
};

fn tadpole_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
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
        .context()
        .clone()
}

fn build_queue(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> GeneratedSectorLiveLeafQueueCertificate {
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        family,
        context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
    limits.translation_radius = 0;
    GeneratedSectorLiveLeafQueueCompiler::compile(family, context, &discovery, limits).unwrap()
}

fn key(power: i64) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new([power]).unwrap()
}

fn base_provider<'a>(
    family: &'a IntegralFamily,
    context: &'a ParametricCoefficientContext,
    queue: &GeneratedSectorLiveLeafQueueCertificate,
) -> ParametricSectorRuleProvider<'a> {
    ParametricSectorRuleProvider::try_new(
        family,
        context,
        [queue.discovery().coverage().clone()],
        ParametricSectorRuleProviderLimits::default(),
    )
    .unwrap()
}

#[test]
fn root_global_rule_is_authoritative_and_conditional_scan_is_not_entered() {
    let family = tadpole_family("conditional-queue-global-route");
    let context = context(&family);
    let queue = build_queue(&family, &context);
    let sector = queue.sector().clone();
    let inner = base_provider(&family, &context, &queue);
    let mut provider = GeneratedSectorConditionalRuleProvider::try_new(
        &family,
        &context,
        [queue],
        inner,
        GeneratedSectorConditionalRuleProviderLimits::default(),
    )
    .unwrap();

    assert!(provider.build_stats().installed_rules() > 0);
    assert_eq!(
        provider
            .rule_provenance(&sector)
            .unwrap()
            .map(|source| (source.work_item_ordinal(), source.pivot_ordinal()))
            .collect::<Vec<_>>()
            .len(),
        provider.build_stats().installed_rules()
    );
    provider.replay().unwrap();

    assert!(matches!(
        provider.decision_for(&key(2)).unwrap(),
        ConcreteRuleDecision::Reduction(_)
    ));
    assert_eq!(provider.stats().global_rule_delegations(), 1);
    assert_eq!(provider.stats().terminal_fallback_queries(), 0);
    assert_eq!(provider.stats().conditional_rule_attempts(), 0);
    assert_eq!(provider.inner().stats().reductions(), 1);
}

#[test]
fn terminal_leaf_scans_conditionals_then_preserves_uncovered_inner_decision() {
    let family = tadpole_family("conditional-queue-uncovered");
    let context = context(&family);
    let queue = build_queue(&family, &context);
    let inner = base_provider(&family, &context, &queue);
    let mut provider = GeneratedSectorConditionalRuleProvider::try_new(
        &family,
        &context,
        [queue],
        inner,
        GeneratedSectorConditionalRuleProviderLimits::default(),
    )
    .unwrap();
    let installed = provider.build_stats().installed_rules();
    assert!(installed > 0);

    assert!(matches!(
        provider.decision_for(&key(1)).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
    ));
    assert_eq!(provider.stats().terminal_fallback_queries(), 1);
    assert_eq!(provider.stats().conditional_rule_attempts(), installed);
    assert_eq!(provider.stats().inapplicable_conditional_rules(), installed);
    assert_eq!(provider.stats().conditional_reductions(), 0);
    assert_eq!(provider.stats().exhausted_fallback_delegations(), 1);
    assert_eq!(provider.inner().stats().uncovered(), 1);
}

#[derive(Debug)]
struct StubError;

impl fmt::Display for StubError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("stub provider error")
    }
}

impl std::error::Error for StubError {}

struct UncoveredProvider {
    arity: usize,
    queries: usize,
}

impl ConcreteRuleProvider for UncoveredProvider {
    type Error = StubError;

    fn index_arity(&self) -> usize {
        self.arity
    }

    fn decision_for(
        &mut self,
        _integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.queries += 1;
        Ok(ConcreteRuleDecision::Terminal(
            ConcreteTerminalStatus::Uncovered,
        ))
    }
}

#[test]
fn missing_sector_delegates_and_inner_arity_changes_fail_closed() {
    let family = tadpole_family("conditional-queue-inner-scope");
    let context = context(&family);
    let mut provider = GeneratedSectorConditionalRuleProvider::try_new(
        &family,
        &context,
        std::iter::empty(),
        UncoveredProvider {
            arity: 1,
            queries: 0,
        },
        GeneratedSectorConditionalRuleProviderLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        provider.decision_for(&key(7)).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
    ));
    assert_eq!(provider.stats().missing_sector_delegations(), 1);
    assert_eq!(provider.inner().queries, 1);

    provider.inner_mut().arity = 2;
    assert!(matches!(
        provider.decision_for(&key(7)),
        Err(
            GeneratedSectorConditionalRuleProviderError::InnerProviderArityChanged {
                expected: 1,
                actual: 2,
            }
        )
    ));
}

#[test]
fn construction_and_query_resource_limits_fail_before_overretention() {
    let family = tadpole_family("conditional-queue-limits");
    let context = context(&family);

    let mut no_queues = GeneratedSectorConditionalRuleProviderLimits::default();
    no_queues.max_queue_certificates = 0;
    assert!(matches!(
        GeneratedSectorConditionalRuleProvider::try_new(
            &family,
            &context,
            [build_queue(&family, &context)],
            UncoveredProvider {
                arity: 1,
                queries: 0,
            },
            no_queues,
        ),
        Err(GeneratedSectorConditionalRuleProviderError::ResourceLimit {
            resource: "conditional queue certificates",
            requested: 1,
            limit: 0,
        })
    ));

    let queue = build_queue(&family, &context);
    let mut no_rules = GeneratedSectorConditionalRuleProviderLimits::default();
    no_rules.max_total_installed_rules = 0;
    // Prove the aggregate installed-rule census is checked before entering
    // condition-bound rule construction and its independent RHS budget.
    no_rules.conditional_rule.max_rhs_terms = 0;
    assert!(matches!(
        GeneratedSectorConditionalRuleProvider::try_new(
            &family,
            &context,
            [queue],
            UncoveredProvider {
                arity: 1,
                queries: 0,
            },
            no_rules,
        ),
        Err(GeneratedSectorConditionalRuleProviderError::ResourceLimit {
            resource: "installed conditional rules",
            requested: 1,
            limit: 0,
        })
    ));

    let queue = build_queue(&family, &context);
    let mut no_attempts = GeneratedSectorConditionalRuleProviderLimits::default();
    no_attempts.max_rules_considered_per_query = 0;
    let mut provider = GeneratedSectorConditionalRuleProvider::try_new(
        &family,
        &context,
        [queue],
        UncoveredProvider {
            arity: 1,
            queries: 0,
        },
        no_attempts,
    )
    .unwrap();
    assert!(matches!(
        provider.decision_for(&key(1)),
        Err(GeneratedSectorConditionalRuleProviderError::ResourceLimit {
            resource: "conditional rules considered per query",
            requested,
            limit: 0,
        }) if requested > 0
    ));
    assert_eq!(provider.inner().queries, 0);
}

#[test]
fn family_context_duplicate_sector_and_arity_failures_are_typed() {
    let family = tadpole_family("conditional-queue-scope");
    let context = context(&family);
    let queue = build_queue(&family, &context);
    let wrong_family = tadpole_family("conditional-queue-other-family");
    assert!(matches!(
        GeneratedSectorConditionalRuleProvider::try_new(
            &wrong_family,
            &context,
            [queue.clone()],
            UncoveredProvider {
                arity: 1,
                queries: 0,
            },
            GeneratedSectorConditionalRuleProviderLimits::default(),
        ),
        Err(GeneratedSectorConditionalRuleProviderError::WrongFamily)
    ));

    let wrong_context = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        "conditional-queue-wrong-context",
        1,
    )
    .unwrap();
    assert!(matches!(
        GeneratedSectorConditionalRuleProvider::try_new(
            &family,
            &wrong_context,
            [queue.clone()],
            UncoveredProvider {
                arity: 1,
                queries: 0,
            },
            GeneratedSectorConditionalRuleProviderLimits::default(),
        ),
        Err(GeneratedSectorConditionalRuleProviderError::WrongContext)
    ));

    assert!(matches!(
        GeneratedSectorConditionalRuleProvider::try_new(
            &family,
            &context,
            [queue.clone(), queue],
            UncoveredProvider {
                arity: 1,
                queries: 0,
            },
            GeneratedSectorConditionalRuleProviderLimits::default(),
        ),
        Err(GeneratedSectorConditionalRuleProviderError::DuplicateSector { .. })
    ));

    assert!(matches!(
        GeneratedSectorConditionalRuleProvider::try_new(
            &family,
            &context,
            std::iter::empty(),
            UncoveredProvider {
                arity: 2,
                queries: 0,
            },
            GeneratedSectorConditionalRuleProviderLimits::default(),
        ),
        Err(
            GeneratedSectorConditionalRuleProviderError::InnerProviderArityChanged {
                expected: 1,
                actual: 2,
            }
        )
    ));
}
