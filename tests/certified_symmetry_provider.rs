//! Black-box tests for proof-bearing symmetry canonicalization.  The concrete
//! families are fixtures only; the provider itself has no topology dispatch.

use std::convert::Infallible;
use std::sync::Arc;

use rustred::reduction_engine::{
    ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus,
};
use rustred::{
    AffineDenominator, CertifiedConcreteRewriteProof, CertifiedSymmetryCanonicalizingRuleProvider,
    CertifiedSymmetryCanonicalizingRuleProviderError,
    CertifiedSymmetryCanonicalizingRuleProviderLimits, CoefficientContext, ConcreteIntegralKey,
    CutConstraint, GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanCompiler,
    GeneratedSymbolicRowSpanConfig, GeneratedSymbolicRowSpanStrategy, GeneratedWhenBadLimits,
    IntegralFamily, IntegralOrderingPolicy, InternalSymmetrySearchLimits,
    ParametricCoefficientContext, ParametricIbpGenerator, SectorPattern, SectorRestrictions,
    discover_bounded_vacuum_internal_symmetries,
};

#[derive(Debug)]
struct RecordingProvider {
    arity: usize,
    requests: Vec<ConcreteIntegralKey>,
}

impl RecordingProvider {
    fn new(arity: usize) -> Self {
        Self {
            arity,
            requests: Vec::new(),
        }
    }
}

impl ConcreteRuleProvider for RecordingProvider {
    type Error = Infallible;

    fn index_arity(&self) -> usize {
        self.arity
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.requests.push(integral.clone());
        Ok(ConcreteRuleDecision::Terminal(
            ConcreteTerminalStatus::Uncovered,
        ))
    }
}

fn key(powers: impl IntoIterator<Item = i64>) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

fn one_loop_vacuum(name: &str) -> IntegralFamily {
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

fn one_loop_two_point(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2", "s"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    IntegralFamily::new(
        name,
        vec!["k".into()],
        vec!["p".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![one.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                coefficients.parse("s-m2").unwrap(),
                vec![one, coefficients.integer(2)],
            ),
        ],
        vec![vec![coefficients.parameter("s").unwrap()]],
        vec![zero.clone(), zero],
    )
    .unwrap()
}

fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn context(family: &IntegralFamily) -> ParametricCoefficientContext {
    ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone()
}

fn row_span(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    strategy: GeneratedSymbolicRowSpanStrategy,
) -> Arc<GeneratedSymbolicRowSpanCertificate> {
    let mut config = GeneratedSymbolicRowSpanConfig::default();
    config.strategy = strategy;
    Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            family,
            context,
            GeneratedWhenBadLimits::default().ibp,
            config,
        )
        .unwrap(),
    )
}

fn bounded_strategy() -> GeneratedSymbolicRowSpanStrategy {
    GeneratedSymbolicRowSpanStrategy::BoundedVacuumInternal {
        search: InternalSymmetrySearchLimits::default(),
        require_exhaustive: true,
    }
}

fn two_generator_sunset_row_span(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Arc<GeneratedSymbolicRowSpanCertificate> {
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let report = discover_bounded_vacuum_internal_symmetries(
        family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    let generators = [[1usize, 0, 2], [0, 2, 1]]
        .iter()
        .map(|permutation| {
            report
                .symmetries()
                .iter()
                .find(|symmetry| symmetry.denominator_permutation() == *permutation)
                .unwrap()
                .clone()
        })
        .collect::<Vec<_>>();
    Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile_with_verified_symmetries(
            family,
            context,
            GeneratedWhenBadLimits::default().ibp,
            &generators,
            GeneratedSymbolicRowSpanConfig::default().limits,
        )
        .unwrap(),
    )
}

fn sunset_provider<'family>(
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    restrictions: SectorRestrictions,
    limits: CertifiedSymmetryCanonicalizingRuleProviderLimits,
) -> CertifiedSymmetryCanonicalizingRuleProvider<'family, RecordingProvider> {
    CertifiedSymmetryCanonicalizingRuleProvider::try_new(
        family,
        context,
        restrictions,
        row_span,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        RecordingProvider::new(family.denominator_count()),
        limits,
    )
    .unwrap()
}

#[test]
fn generic_nonvacuum_disabled_span_delegates_without_topology_dispatch() {
    let family = one_loop_two_point("symmetry-provider-generic-nonvacuum");
    let context = context(&family);
    let row_span = row_span(
        &family,
        &context,
        GeneratedSymbolicRowSpanStrategy::Disabled,
    );
    assert!(row_span.symmetries().is_empty());
    let mut provider = CertifiedSymmetryCanonicalizingRuleProvider::try_new(
        &family,
        &context,
        SectorRestrictions::unrestricted(2).unwrap(),
        row_span,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        RecordingProvider::new(2),
        Default::default(),
    )
    .unwrap();

    let source = key([2, -1]);
    assert!(matches!(
        provider.decision_for(&source).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
    ));
    assert_eq!(provider.inner().requests, [source]);
    assert_eq!(provider.stats().delegated_queries(), 1);
}

#[test]
fn identity_only_span_delegates_and_replays() {
    let family = one_loop_vacuum("symmetry-provider-identity-only");
    let context = context(&family);
    let row_span = row_span(&family, &context, bounded_strategy());
    assert!(!row_span.symmetries().is_empty());
    assert!(
        row_span
            .symmetries()
            .iter()
            .all(|symmetry| symmetry.denominator_permutation() == [0])
    );
    let mut provider = CertifiedSymmetryCanonicalizingRuleProvider::try_new(
        &family,
        &context,
        SectorRestrictions::unrestricted(1).unwrap(),
        row_span,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        RecordingProvider::new(1),
        Default::default(),
    )
    .unwrap();

    let source = key([3]);
    provider.decision_for(&source).unwrap();
    assert_eq!(provider.inner().requests, [source]);
    provider.replay().unwrap();
}

#[test]
fn equal_mass_sunset_maps_noncanonical_source_by_strict_replayable_rewrite() {
    let family = equal_mass_sunset("symmetry-provider-sunset-rewrite");
    let context = context(&family);
    let row_span = row_span(&family, &context, bounded_strategy());
    assert_eq!(row_span.symmetries().len(), 6);
    let mut provider = sunset_provider(
        &family,
        &context,
        row_span,
        SectorRestrictions::unrestricted(3).unwrap(),
        Default::default(),
    );

    let orbit_samples = [key([2, 1, 1]), key([1, 2, 1]), key([1, 1, 2])];
    let (source, canonical) = orbit_samples
        .iter()
        .find_map(|source| {
            let canonical = provider.canonical_key(source).unwrap();
            (canonical != *source).then(|| (source.clone(), canonical))
        })
        .expect("the nontrivial S3 orbit has a noncanonical dotted sunset");

    let decision = provider.decision_for(&source).unwrap();
    assert!(provider.inner().requests.is_empty());
    let ConcreteRuleDecision::CertifiedRewrite(rewrite) = decision else {
        panic!("a noncanonical source must produce a proof-bearing rewrite");
    };
    assert_eq!(rewrite.source(), &source);
    assert_eq!(rewrite.rhs().len(), 1);
    assert_eq!(rewrite.rhs().first_key_value().unwrap().0, &canonical);
    assert!(matches!(
        rewrite.proof(),
        CertifiedConcreteRewriteProof::Symmetry { path } if !path.is_empty()
    ));
    assert!(rewrite.descent_witnesses()[&canonical].verify());
    assert!(
        rewrite
            .verify_application(
                family.coefficient_context(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Default::default(),
            )
            .unwrap()
    );
    rewrite
        .replay(
            &family,
            &context,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
        )
        .unwrap();

    provider.decision_for(&canonical).unwrap();
    assert_eq!(provider.inner().requests, [canonical]);
    assert_eq!(provider.stats().symmetry_rewrites(), 1);
    assert_eq!(provider.stats().delegated_queries(), 1);
    provider.replay().unwrap();
}

#[test]
fn cut_policy_filters_incompatible_maps_but_retains_the_allowed_suborbit() {
    let family = equal_mass_sunset("symmetry-provider-cut-policy");
    let context = context(&family);
    let row_span = row_span(&family, &context, bounded_strategy());
    let restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(3, [0]).unwrap(),
        SectorPattern::any(3).unwrap(),
    )
    .unwrap();
    let mut provider = sunset_provider(
        &family,
        &context,
        row_span.clone(),
        restrictions,
        Default::default(),
    );

    assert_eq!(provider.compatible_symmetry_ordinals().len(), 2);
    assert!(
        provider
            .compatible_symmetry_ordinals()
            .iter()
            .all(|&ordinal| { row_span.symmetries()[ordinal].denominator_permutation()[0] == 0 })
    );
    let candidates = [key([1, 2, 1]), key([1, 1, 2])];
    let source = candidates
        .iter()
        .find(|source| provider.canonical_key(source).unwrap() != **source)
        .unwrap()
        .clone();
    assert!(matches!(
        provider.decision_for(&source).unwrap(),
        ConcreteRuleDecision::CertifiedRewrite(_)
    ));
    assert!(provider.inner().requests.is_empty());
}

#[test]
fn orbit_state_cap_fails_before_delegating_or_retaining_a_rewrite() {
    let family = equal_mass_sunset("symmetry-provider-orbit-cap");
    let context = context(&family);
    let row_span = row_span(&family, &context, bounded_strategy());
    let mut limits = CertifiedSymmetryCanonicalizingRuleProviderLimits::default();
    limits.max_orbit_states_per_query = 1;
    let mut provider = sunset_provider(
        &family,
        &context,
        row_span,
        SectorRestrictions::unrestricted(3).unwrap(),
        limits,
    );

    assert!(matches!(
        provider.decision_for(&key([2, 1, 1])),
        Err(
            CertifiedSymmetryCanonicalizingRuleProviderError::ResourceLimit {
                resource: "symmetry orbit states",
                requested: 2,
                limit: 1,
            }
        )
    ));
    assert!(provider.inner().requests.is_empty());
    assert_eq!(provider.stats().queries(), 0);
}

#[test]
fn clone_count_cap_zero_fails_before_compilation_and_keeps_cache_stats_empty() {
    let family = equal_mass_sunset("symmetry-provider-clone-cap-zero");
    let context = context(&family);
    let row_span = row_span(&family, &context, bounded_strategy());
    let mut limits = CertifiedSymmetryCanonicalizingRuleProviderLimits::default();
    limits.max_retained_cloned_symmetries = 0;
    let mut provider = sunset_provider(
        &family,
        &context,
        row_span,
        SectorRestrictions::unrestricted(3).unwrap(),
        limits,
    );
    let source = [key([2, 1, 1]), key([1, 2, 1]), key([1, 1, 2])]
        .into_iter()
        .find(|source| provider.canonical_key(source).unwrap() != *source)
        .unwrap();

    for _ in 0..2 {
        assert!(matches!(
            provider.decision_for(&source),
            Err(
                CertifiedSymmetryCanonicalizingRuleProviderError::ResourceLimit {
                    resource: "retained cloned symmetry certificates",
                    requested: 1,
                    limit: 0,
                }
            )
        ));
        assert_eq!(provider.stats().retained_cloned_symmetries(), 0);
        assert_eq!(provider.stats().retained_cloned_symmetry_debug_bytes(), 0);
        assert_eq!(provider.stats().queries(), 0);
    }
}

#[test]
fn multi_generator_clone_limit_failure_is_transactional() {
    let family = equal_mass_sunset("symmetry-provider-multi-generator-cap");
    let context = context(&family);
    let row_span = two_generator_sunset_row_span(&family, &context);
    assert_eq!(row_span.symmetries().len(), 2);
    let mut limits = CertifiedSymmetryCanonicalizingRuleProviderLimits::default();
    limits.max_retained_cloned_symmetries = 1;
    let mut provider = sunset_provider(
        &family,
        &context,
        row_span.clone(),
        SectorRestrictions::unrestricted(3).unwrap(),
        limits,
    );
    let source = key([3, 2, 1]);
    let canonical = provider.canonical_key(&source).unwrap();
    assert_ne!(canonical, source);
    assert!(
        row_span
            .symmetries()
            .iter()
            .all(|symmetry| { symmetry.transport_source_key(&source).unwrap() != canonical })
    );

    for _ in 0..2 {
        assert!(matches!(
            provider.decision_for(&source),
            Err(
                CertifiedSymmetryCanonicalizingRuleProviderError::ResourceLimit {
                    resource: "retained cloned symmetry certificates",
                    requested: 2,
                    limit: 1,
                }
            )
        ));
        assert_eq!(provider.stats().retained_cloned_symmetries(), 0);
        assert_eq!(provider.stats().retained_cloned_symmetry_debug_bytes(), 0);
        assert_eq!(provider.stats().queries(), 0);
    }
}

#[test]
fn explicit_terminals_are_canonicalized_deduplicated_and_conflicts_rejected() {
    let family = equal_mass_sunset("symmetry-provider-terminal-policy");
    let context = context(&family);
    let row_span = row_span(&family, &context, bounded_strategy());
    let provider = sunset_provider(
        &family,
        &context,
        row_span,
        SectorRestrictions::unrestricted(3).unwrap(),
        Default::default(),
    );
    let candidates = [key([2, 1, 1]), key([1, 2, 1]), key([1, 1, 2])];
    let (source, canonical) = candidates
        .iter()
        .find_map(|source| {
            let canonical = provider.canonical_key(source).unwrap();
            (canonical != *source).then(|| (source.clone(), canonical))
        })
        .unwrap();

    let deduplicated = provider
        .canonicalize_terminals([(source.clone(), "master"), (canonical.clone(), "master")])
        .unwrap();
    assert_eq!(deduplicated, vec![(canonical.clone(), "master")]);
    assert!(matches!(
        provider.canonicalize_terminals([(source, "first"), (canonical.clone(), "second")]),
        Err(CertifiedSymmetryCanonicalizingRuleProviderError::ConflictingCanonicalTerminal {
            canonical: conflict,
        }) if conflict == canonical
    ));
}

#[test]
fn terminal_canonicalization_cap_counts_duplicates_before_deduplication() {
    let family = equal_mass_sunset("symmetry-provider-terminal-cap");
    let context = context(&family);
    let row_span = row_span(&family, &context, bounded_strategy());
    let mut limits = CertifiedSymmetryCanonicalizingRuleProviderLimits::default();
    limits.max_terminal_canonicalizations = 1;
    let provider = sunset_provider(
        &family,
        &context,
        row_span,
        SectorRestrictions::unrestricted(3).unwrap(),
        limits,
    );
    let duplicate = key([2, 1, 1]);
    assert!(matches!(
        provider.canonicalize_terminals([(duplicate.clone(), "master"), (duplicate, "master"),]),
        Err(
            CertifiedSymmetryCanonicalizingRuleProviderError::ResourceLimit {
                resource: "terminal canonicalizations",
                requested: 2,
                limit: 1,
            }
        )
    ));
}

#[test]
fn public_constructor_authenticates_the_row_span_context() {
    let family = one_loop_vacuum("symmetry-provider-context-auth");
    let context = context(&family);
    let row_span = row_span(
        &family,
        &context,
        GeneratedSymbolicRowSpanStrategy::Disabled,
    );
    let foreign = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        "symmetry-provider-foreign-context",
        1,
    )
    .unwrap();

    assert!(matches!(
        CertifiedSymmetryCanonicalizingRuleProvider::try_new(
            &family,
            &foreign,
            SectorRestrictions::unrestricted(1).unwrap(),
            row_span,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            RecordingProvider::new(1),
            Default::default(),
        ),
        Err(CertifiedSymmetryCanonicalizingRuleProviderError::WrongContext)
    ));
}
