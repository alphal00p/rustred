use rustred::reduction_engine::{
    ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus,
};
use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits, AffineDenominator,
    CoefficientContext, ConcreteIntegralKey, IndexSpace, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricIbpGenerator, ParametricReductionEngine,
    ParametricRelation, ParametricRowId, ReductionEngineLimits,
};

fn tadpole() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "adaptive-rule-tadpole",
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

#[test]
fn generated_rows_drive_adaptive_demand_reduction_without_a_hardcoded_recurrence() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let provider = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    let result = engine
        .reduce(&ConcreteIntegralKey::try_new([4]).unwrap())
        .unwrap();
    let (leaf, coefficient) = result.terms().first_key_value().unwrap();
    assert_eq!(result.terms().len(), 1);
    assert_eq!(leaf.powers(), &[1]);
    assert_eq!(
        coefficient,
        &family
            .coefficient_context()
            .parse("(6-d)*(4-d)*(2-d)/(48*m2^3)")
            .unwrap()
    );
    assert_eq!(result.stats().rule_applications(), 3);
    assert_eq!(
        result
            .uncovered_leaves()
            .iter()
            .map(|key| key.powers())
            .collect::<Vec<_>>(),
        vec![&[1][..]]
    );
    assert!(result.require_complete().is_err());
    assert!(result.selected_masters().is_empty());
    assert!(result.certified_masters().is_empty());
    assert!(result.required_nonzero().iter().any(|guard| {
        guard
            .polynomial()
            .to_expression()
            .to_string()
            .contains("m2")
    }));

    let discovery = engine.provider().stats();
    assert!(discovery.eliminations() >= 4);
    assert_eq!(discovery.applicable_candidates(), 3);
    assert_eq!(discovery.uncovered_requests(), 1);
}

#[test]
fn adaptive_search_resource_exhaustion_is_typed() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let mut limits = AdaptiveRuleSearchLimits::default();
    limits.max_scout_points_per_integral = 0;
    let provider = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let error = engine
        .reduce(&ConcreteIntegralKey::try_new([2]).unwrap())
        .unwrap_err();
    assert!(error.to_string().contains("scout points per integral"));
}

#[test]
fn pre_sector_offset_enumeration_has_an_independent_typed_budget() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let mut limits = AdaptiveRuleSearchLimits::default();
    limits.max_enumerated_offsets_per_integral = 0;
    let mut provider = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
    .unwrap();
    let error = provider
        .decision_for(&ConcreteIntegralKey::try_new([2]).unwrap())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("enumerated search offsets per layer")
    );
}

#[test]
fn generated_rows_reduce_nonpositive_tadpole_powers_without_special_recurrences() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let provider = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    for power in [0, -1, -2] {
        let result = engine
            .reduce(&ConcreteIntegralKey::try_new([power]).unwrap())
            .unwrap();
        assert!(result.terms().is_empty(), "power {power} must be scaleless");
        assert!(result.uncovered_leaves().is_empty());
        result.require_complete().unwrap();
    }
}

#[test]
fn cumulative_translated_stencil_finds_rule_absent_at_depth_zero() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let family = IntegralFamily::new(
        "adaptive-stencil-family",
        vec!["k".into()],
        vec!["p".into()],
        base.clone(),
        base.integer(4),
        vec![
            AffineDenominator::new(base.zero(), vec![base.one(), base.zero()]),
            AffineDenominator::new(base.zero(), vec![base.zero(), base.one()]),
        ],
        vec![vec![base.one()]],
        vec![base.zero(), base.zero()],
    )
    .unwrap();
    let context =
        ParametricCoefficientContext::try_new(&base, "adaptive-stencil-oracle", 2).unwrap();
    let space = IndexSpace::try_new(2).unwrap();

    // A = J(n+(-1,1)) + J(n+(0,-1)) + J(n+(1,-1)).
    let mut first = ParametricRelation::new(
        family.fingerprint_ref(),
        ParametricRowId::Derived { label: "A".into() },
        &context,
    );
    for shift in [[-1, 1], [0, -1], [1, -1]] {
        first
            .add_term(&context, space.shift(shift).unwrap(), context.one())
            .unwrap();
    }

    // B = J(n+(-1,-1)) - J(n+(0,1)) - J(n+(1,-1)).
    let mut second = ParametricRelation::new(
        family.fingerprint_ref(),
        ParametricRowId::Derived { label: "B".into() },
        &context,
    );
    second
        .add_term(&context, space.shift([-1, -1]).unwrap(), context.one())
        .unwrap();
    for shift in [[0, 1], [1, -1]] {
        second
            .add_term(&context, space.shift(shift).unwrap(), context.integer(-1))
            .unwrap();
    }
    let rows = vec![first, second];
    let target = ConcreteIntegralKey::try_new([1, 0]).unwrap();

    let mut depth_zero_limits = AdaptiveRuleSearchLimits::default();
    depth_zero_limits.max_search_depth = 0;
    let mut depth_zero = AdaptiveParametricRuleProvider::try_new(
        &context,
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        depth_zero_limits,
    )
    .unwrap();
    assert!(matches!(
        depth_zero.decision_for(&target).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
    ));
    assert_eq!(depth_zero.stats().eliminations(), 1);

    let mut depth_one_limits = AdaptiveRuleSearchLimits::default();
    depth_one_limits.max_search_depth = 1;
    let mut depth_one = AdaptiveParametricRuleProvider::try_new(
        &context,
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        depth_one_limits,
    )
    .unwrap();
    let ConcreteRuleDecision::Reduction(reduction) = depth_one.decision_for(&target).unwrap()
    else {
        panic!("the cumulative depth-one stencil must yield a descending rule")
    };
    assert_eq!(reduction.source(), &target);
    assert_eq!(reduction.rhs().len(), 1);
    assert_eq!(
        reduction.rhs().first_key_value().unwrap().0.powers(),
        &[-2, 0]
    );
    assert_eq!(
        reduction
            .anchored_candidate()
            .expect("adaptive rule has anchored provenance")
            .derivation()
            .source_rows()
            .len(),
        6
    );
    reduction
        .anchored_candidate()
        .expect("adaptive rule has anchored provenance")
        .replay_retained(&context)
        .unwrap();
    let retained_reduction = reduction.clone();
    assert_eq!(depth_one.stats().eliminations(), 2);
    assert_eq!(depth_one.stats().scout_points(), 3);
    drop(depth_one);
    assert!(
        retained_reduction
            .replay_application(&family, &context)
            .unwrap()
    );
}
