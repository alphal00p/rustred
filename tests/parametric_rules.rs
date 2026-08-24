use rustred::{
    AffineDenominator, CoefficientContext, IntegralFamily, IntegralOrderingPolicy,
    ParametricElimination, ParametricEliminationLimits, ParametricEliminationOrdering,
    ParametricIbpGenerator, ParametricReductionRule, ParametricRuleApplication,
    ParametricRuleInapplicability, ParametricRuleLimits, ParametricRuleUndecidability, SectorMask,
};

fn tadpole() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "parametric-rule-tadpole",
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
fn generated_active_tadpole_rule_reduces_dots_and_replays() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let ordering =
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
            .unwrap();
    let elimination = ParametricElimination::build(
        generated.context(),
        &rows,
        ordering,
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    assert_eq!(elimination.pivots()[0].pivot().values(), &[1]);

    let rule = ParametricReductionRule::try_from_elimination_pivot(
        generated.context(),
        &rows,
        &elimination,
        0,
        SectorMask::try_new([true]).unwrap(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    assert_eq!(rule.discovery_anchor(), &[2]);
    assert_eq!(rule.source_row_count(), rows.len());
    assert_eq!(rule.source_manifest(), elimination.source_manifest());
    assert_eq!(rule.derivation().source_rows(), rows.as_slice());
    rule.replay_retained(generated.context()).unwrap();
    rule.replay(generated.context(), &elimination, &rows)
        .unwrap();

    let ParametricRuleApplication::Applicable(reduction) =
        rule.apply(generated.context(), &[3]).unwrap()
    else {
        panic!("the generated dot recurrence must apply at n=3")
    };
    assert_eq!(reduction.source().powers(), &[3]);
    assert_eq!(reduction.rhs().len(), 1);
    let (target, coefficient) = reduction.rhs().first_key_value().unwrap();
    assert_eq!(target.powers(), &[2]);
    assert_eq!(
        coefficient,
        &family.coefficient_context().parse("(4-d)/(4*m2)").unwrap()
    );
    assert!(reduction.verify_descent(IntegralOrderingPolicy::RustRedUnshiftedV1));
    assert!(
        reduction
            .replay_application(&family, generated.context())
            .unwrap()
    );

    assert!(matches!(
        rule.apply(generated.context(), &[1]).unwrap(),
        ParametricRuleApplication::Inapplicable(
            ParametricRuleInapplicability::NonzeroGuardVanished
        )
    ));
    assert!(matches!(
        rule.apply(generated.context(), &[0]).unwrap(),
        ParametricRuleApplication::Inapplicable(ParametricRuleInapplicability::OutsideSector)
    ));
    assert!(matches!(
        rule.symbolic_applicability(),
        ParametricRuleApplication::Undecidable(
            ParametricRuleUndecidability::ConcreteIndicesRequired
        )
    ));
}

#[test]
fn generated_numerator_rule_uses_coefficient_aware_boundary_guard() {
    let family = tadpole();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let ordering =
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [-2])
            .unwrap();
    let elimination = ParametricElimination::build(
        generated.context(),
        &rows,
        ordering,
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    assert_eq!(elimination.pivots()[0].pivot().values(), &[0]);
    let rule = ParametricReductionRule::try_from_elimination_pivot(
        generated.context(),
        &rows,
        &elimination,
        0,
        SectorMask::try_new([false]).unwrap(),
        ParametricRuleLimits::default(),
    )
    .unwrap();

    let ParametricRuleApplication::Applicable(reduction) =
        rule.apply(generated.context(), &[-2]).unwrap()
    else {
        panic!("the generated numerator recurrence must apply at n=-2")
    };
    assert_eq!(reduction.source().powers(), &[-2]);
    assert_eq!(reduction.rhs().first_key_value().unwrap().0.powers(), &[-1]);
    assert!(reduction.verify_descent(IntegralOrderingPolicy::RustRedUnshiftedV1));

    // The formal RHS shift would cross n=0 into the active sector.  Its exact
    // coefficient is proportional to n and vanishes at this point, so the
    // specialized rule safely proves the scaleless n=0 integral zero.
    let ParametricRuleApplication::Applicable(boundary) =
        rule.apply(generated.context(), &[0]).unwrap()
    else {
        panic!("the zero coefficient must remove the boundary leak")
    };
    assert!(boundary.rhs().is_empty());

    // The same centered identity is not a descending active-sector rule.
    let active_rule = ParametricReductionRule::try_from_elimination_pivot(
        generated.context(),
        &rows,
        &elimination,
        0,
        SectorMask::try_new([true]).unwrap(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        active_rule.apply(generated.context(), &[2]).unwrap(),
        ParametricRuleApplication::Inapplicable(
            ParametricRuleInapplicability::NonDescendingRhs { .. }
        )
    ));
}

#[test]
fn rule_compilation_honors_retained_rhs_limit() {
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
    let mut limits = ParametricRuleLimits::default();
    limits.max_rhs_terms = 0;
    assert!(
        ParametricReductionRule::try_from_elimination_pivot(
            generated.context(),
            &rows,
            &elimination,
            0,
            SectorMask::try_new([true]).unwrap(),
            limits,
        )
        .is_err()
    );
}
