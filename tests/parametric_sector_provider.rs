//! Black-box audit of the generated sector-certificate application bridge.
//!
//! All installed rules originate in a freshly generated one-loop IBP row.
//! Concrete coefficients are used only as output oracles; no recurrence is
//! supplied to discovery or coverage construction.

use rustred::{
    AffineDenominator, CoefficientContext, ConcreteIntegralKey, ConcreteRuleApplicationTrace,
    ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus, IntegralFamily,
    IntegralOrderingPolicy, MasterPolicyProvider, ParametricCoefficientContext,
    ParametricElimination, ParametricEliminationLimits, ParametricEliminationOrdering,
    ParametricIbpGenerator, ParametricReductionEngine, ParametricReductionRuleCandidate,
    ParametricRelation, ParametricRuleLimits, ParametricSectorCoverageCertificate,
    ParametricSectorCoverageCompiler, ParametricSectorCoverageLimits, ParametricSectorRuleProvider,
    ParametricSectorRuleProviderError, ParametricSectorRuleProviderLimits, ReductionEngineLimits,
    SectorMask,
};

fn family(name: &str) -> IntegralFamily {
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

fn key(power: i64) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new([power]).unwrap()
}

fn candidate(
    context: &ParametricCoefficientContext,
    rows: &[ParametricRelation],
    sector: SectorMask,
    anchor: i64,
) -> ParametricReductionRuleCandidate {
    let elimination = ParametricElimination::build(
        context,
        rows,
        ParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [anchor],
        )
        .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    ParametricReductionRuleCandidate::try_from_elimination_pivot(
        context,
        rows,
        &elimination,
        0,
        sector,
        ParametricRuleLimits::default(),
    )
    .unwrap()
}

struct Fixture {
    family: IntegralFamily,
    context: ParametricCoefficientContext,
    active: ParametricSectorCoverageCertificate,
    inactive: ParametricSectorCoverageCertificate,
}

fn fixture(name: &str) -> Fixture {
    let family = family(name);
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let context = generated.context().clone();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();

    let active_sector = SectorMask::try_new([true]).unwrap();
    let active_candidate = candidate(&context, &rows, active_sector.clone(), 2);
    let active = ParametricSectorCoverageCompiler::compile(
        &family,
        &context,
        active_sector,
        &[active_candidate],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();

    let inactive_sector = SectorMask::try_new([false]).unwrap();
    let inactive_candidate = candidate(&context, &rows, inactive_sector.clone(), 0);
    let inactive = ParametricSectorCoverageCompiler::compile(
        &family,
        &context,
        inactive_sector,
        &[inactive_candidate],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();

    Fixture {
        family,
        context,
        active,
        inactive,
    }
}

#[test]
fn generated_active_sector_applies_only_on_its_certified_integer_locus() {
    let fixture = fixture("sector-provider-active-query");
    fixture
        .active
        .replay(&fixture.family, &fixture.context)
        .unwrap();
    let mut provider = ParametricSectorRuleProvider::try_new(
        &fixture.family,
        &fixture.context,
        [fixture.active.clone()],
        ParametricSectorRuleProviderLimits::default(),
    )
    .unwrap();
    assert_eq!(provider.certificates().len(), 1);
    provider
        .certificates()
        .values()
        .next()
        .unwrap()
        .replay(&fixture.family, &fixture.context)
        .unwrap();

    for (power, expected) in [
        (2, Some("(d-2)/(2*m2)")),
        (3, Some("(d-4)/(4*m2)")),
        (4, Some("(d-6)/(6*m2)")),
        (41, None),
        (i64::MAX, None),
    ] {
        let ConcreteRuleDecision::Reduction(reduction) =
            provider.decision_for(&key(power)).unwrap()
        else {
            panic!("n={power} must be on the certified descending leaf")
        };
        assert_eq!(reduction.source(), &key(power));
        assert_eq!(reduction.rhs().len(), 1);
        let actual = reduction.rhs().get(&key(power - 1)).unwrap();
        if let Some(expected) = expected {
            assert_eq!(
                actual,
                &fixture
                    .family
                    .coefficient_context()
                    .parse(expected)
                    .unwrap()
            );
        }
        reduction
            .replay_application(&fixture.family, &fixture.context)
            .unwrap();
    }

    assert!(matches!(
        provider.decision_for(&key(1)).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
    ));
    // n=0 belongs to the absent inactive sector, rather than to an active
    // certificate leaf.
    assert!(matches!(
        provider.decision_for(&key(0)).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
    ));
    assert_eq!(provider.stats().queries(), 7);
    assert_eq!(provider.stats().reductions(), 5);
    assert_eq!(provider.stats().uncovered(), 2);
    assert_eq!(provider.stats().unsupported(), 0);
}

#[test]
fn authenticated_inactive_leaf_is_typed_unsupported_and_never_a_terminal() {
    let fixture = fixture("sector-provider-inactive-unsupported");
    let mut provider = ParametricSectorRuleProvider::try_new(
        &fixture.family,
        &fixture.context,
        [fixture.inactive],
        ParametricSectorRuleProviderLimits::default(),
    )
    .unwrap();
    let inactive = SectorMask::try_new([false]).unwrap();
    assert!(matches!(
        provider.decision_for(&key(0)),
        Err(ParametricSectorRuleProviderError::UnsupportedLeaf {
            sector,
            candidate_ordinals,
        }) if sector == inactive && candidate_ordinals.as_ref() == [0]
    ));
    assert_eq!(provider.stats().queries(), 1);
    assert_eq!(provider.stats().unsupported(), 1);
    assert_eq!(provider.stats().reductions(), 0);
    assert_eq!(provider.stats().uncovered(), 0);
}

#[test]
fn construction_replays_bindings_and_fails_closed_on_duplicates_and_budgets() {
    let fixture = fixture("sector-provider-construction-audit");

    let foreign_family = family("sector-provider-foreign-family");
    assert!(matches!(
        ParametricSectorRuleProvider::try_new(
            &foreign_family,
            &fixture.context,
            [fixture.active.clone()],
            ParametricSectorRuleProviderLimits::default(),
        ),
        Err(ParametricSectorRuleProviderError::WrongFamily)
    ));

    assert!(matches!(
        ParametricSectorRuleProvider::try_new(
            &fixture.family,
            &fixture.context,
            [fixture.active.clone(), fixture.active.clone()],
            ParametricSectorRuleProviderLimits::default(),
        ),
        Err(ParametricSectorRuleProviderError::DuplicateSector { sector })
            if sector == SectorMask::try_new([true]).unwrap()
    ));

    let foreign_context = ParametricCoefficientContext::try_new(
        fixture.family.coefficient_context(),
        "sector-provider-foreign-context",
        1,
    )
    .unwrap();
    assert!(matches!(
        ParametricSectorRuleProvider::try_new(
            &fixture.family,
            &foreign_context,
            [fixture.active.clone()],
            ParametricSectorRuleProviderLimits::default(),
        ),
        Err(ParametricSectorRuleProviderError::WrongContext)
    ));

    let mut limits = ParametricSectorRuleProviderLimits::default();
    limits.max_sector_certificates = 0;
    assert!(matches!(
        ParametricSectorRuleProvider::try_new(
            &fixture.family,
            &fixture.context,
            [fixture.active.clone()],
            limits,
        ),
        Err(ParametricSectorRuleProviderError::ResourceLimit {
            resource: "sector rule-provider certificates",
            requested: 1,
            limit: 0,
        })
    ));

    let mut limits = ParametricSectorRuleProviderLimits::default();
    limits.max_total_candidate_attempts = 0;
    assert!(matches!(
        ParametricSectorRuleProvider::try_new(
            &fixture.family,
            &fixture.context,
            [fixture.active.clone()],
            limits,
        ),
        Err(ParametricSectorRuleProviderError::ResourceLimit {
            resource: "sector rule-provider candidate attempts",
            requested: 1,
            limit: 0,
        })
    ));

    let mut limits = ParametricSectorRuleProviderLimits::default();
    limits.max_total_global_leaves = 1;
    assert!(matches!(
        ParametricSectorRuleProvider::try_new(
            &fixture.family,
            &fixture.context,
            [fixture.active],
            limits,
        ),
        Err(ParametricSectorRuleProviderError::ResourceLimit {
            resource: "sector rule-provider global leaves",
            requested: 2,
            limit: 1,
        })
    ));
}

#[test]
fn query_budget_failures_leave_the_runtime_census_transactional() {
    let fixture = fixture("sector-provider-query-transaction");

    let mut limits = ParametricSectorRuleProviderLimits::default();
    limits.max_queries = 0;
    let mut no_queries = ParametricSectorRuleProvider::try_new(
        &fixture.family,
        &fixture.context,
        [fixture.active],
        limits,
    )
    .unwrap();
    let before = no_queries.stats();
    assert!(matches!(
        no_queries.decision_for(&key(2)),
        Err(ParametricSectorRuleProviderError::ResourceLimit {
            resource: "sector rule-provider queries",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(no_queries.stats(), before);

    let mut limits = ParametricSectorRuleProviderLimits::default();
    limits.max_unsupported_ordinals_per_query = 0;
    let mut unsupported_budget = ParametricSectorRuleProvider::try_new(
        &fixture.family,
        &fixture.context,
        [fixture.inactive],
        limits,
    )
    .unwrap();
    let before = unsupported_budget.stats();
    assert!(matches!(
        unsupported_budget.decision_for(&key(0)),
        Err(ParametricSectorRuleProviderError::ResourceLimit {
            resource: "unsupported candidate ordinals per provider query",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(unsupported_budget.stats(), before);

    let wrong_arity = ConcreteIntegralKey::try_new([2, 1]).unwrap();
    assert!(matches!(
        unsupported_budget.decision_for(&wrong_arity),
        Err(ParametricSectorRuleProviderError::WrongArity {
            expected: 1,
            actual: 2,
        })
    ));
    assert_eq!(unsupported_budget.stats(), before);
}

#[test]
fn selected_one_loop_master_completes_i4_with_the_vakint_oracle_coefficient() {
    let fixture = fixture("sector-provider-complete-one-loop");
    let provider = ParametricSectorRuleProvider::try_new(
        &fixture.family,
        &fixture.context,
        [fixture.active],
        ParametricSectorRuleProviderLimits::default(),
    )
    .unwrap();
    let provider = MasterPolicyProvider::with_selected(provider, [key(1)]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        fixture.family.fingerprint(),
        fixture.family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    let result = engine.reduce(&key(4)).unwrap();
    result.require_complete().unwrap();
    assert!(result.uncovered_leaves().is_empty());
    assert_eq!(result.selected_masters().len(), 1);
    assert!(result.selected_masters().contains(&key(1)));
    assert_eq!(result.terms().len(), 1);
    assert_eq!(
        result.terms().get(&key(1)).unwrap(),
        &fixture
            .family
            .coefficient_context()
            .parse("(d-6)*(d-4)*(d-2)/(48*m2^3)")
            .unwrap()
    );
    assert_eq!(result.application_traces().len(), 3);
    for trace in result.application_traces() {
        let ConcreteRuleApplicationTrace::Parametric(proof) = trace else {
            panic!("sector provider must retain a parametric application proof")
        };
        proof
            .replay_application(&fixture.family, &fixture.context)
            .unwrap();
    }
    assert_eq!(engine.provider().inner().stats().queries(), 3);
    assert_eq!(engine.provider().inner().stats().reductions(), 3);
}
