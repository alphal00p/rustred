use rustred::{
    CoefficientContext, IndexShift, IntegralOrderingPolicy, ParametricCoefficient,
    ParametricCoefficientContext, ParametricElimination, ParametricEliminationLimits,
    ParametricEliminationOrdering, ParametricReductionRuleCandidate, ParametricRelation,
    ParametricRowId, ParametricRuleLimits, SectorMask, WhenBadCompilation, WhenBadCompiler,
    WhenBadCompilerError, WhenBadCompilerLimits, WhenBadLeafDisposition, WhenBadUnsupportedReason,
};
use std::sync::Arc;

struct Fixture {
    context: ParametricCoefficientContext,
    candidate: ParametricReductionRuleCandidate,
}

fn synthetic_candidate(
    scope: &str,
    base: CoefficientContext,
    sector: SectorMask,
    anchor: Vec<i64>,
    rhs_shift: Vec<i64>,
    coefficient: impl FnOnce(&ParametricCoefficientContext) -> ParametricCoefficient,
    decorate: impl FnOnce(&ParametricCoefficientContext, &mut ParametricRelation),
) -> Fixture {
    let context = ParametricCoefficientContext::try_new(&base, scope, sector.arity()).unwrap();
    let mut row = ParametricRelation::new(
        format!("when-bad-synthetic-{scope}"),
        ParametricRowId::Derived {
            label: Arc::from(format!("when-bad-source-{scope}")),
        },
        &context,
    );
    row.add_term(
        &context,
        IndexShift::try_new(vec![0; sector.arity()], sector.arity()).unwrap(),
        context.one(),
    )
    .unwrap();
    row.add_term(
        &context,
        IndexShift::try_new(rhs_shift, sector.arity()).unwrap(),
        coefficient(&context),
    )
    .unwrap();
    decorate(&context, &mut row);
    let rows = vec![row];
    let elimination = ParametricElimination::build(
        &context,
        &rows,
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, anchor)
            .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    assert_eq!(
        elimination.pivots()[0].pivot().values(),
        vec![0; sector.arity()]
    );
    let candidate = ParametricReductionRuleCandidate::try_from_elimination_pivot(
        &context,
        &rows,
        &elimination,
        0,
        sector,
        ParametricRuleLimits::default(),
    )
    .unwrap();
    Fixture { context, candidate }
}

fn synthetic_candidate_with_pivot_shift(
    scope: &str,
    base: CoefficientContext,
    sector: SectorMask,
    anchor: Vec<i64>,
    source_pivot_shift: Vec<i64>,
    source_rhs_shift: Vec<i64>,
    coefficient: impl FnOnce(&ParametricCoefficientContext) -> ParametricCoefficient,
) -> Fixture {
    let context = ParametricCoefficientContext::try_new(&base, scope, sector.arity()).unwrap();
    let mut row = ParametricRelation::new(
        format!("when-bad-shifted-{scope}"),
        ParametricRowId::Derived {
            label: Arc::from(format!("when-bad-shifted-source-{scope}")),
        },
        &context,
    );
    row.add_term(
        &context,
        IndexShift::try_new(source_pivot_shift.clone(), sector.arity()).unwrap(),
        context.one(),
    )
    .unwrap();
    row.add_term(
        &context,
        IndexShift::try_new(source_rhs_shift, sector.arity()).unwrap(),
        coefficient(&context),
    )
    .unwrap();
    let rows = vec![row];
    let elimination = ParametricElimination::build(
        &context,
        &rows,
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, anchor)
            .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    let pivot_ordinal = elimination
        .pivots()
        .iter()
        .position(|pivot| pivot.pivot().values() == source_pivot_shift)
        .expect("requested source pivot must be retained");
    let candidate = ParametricReductionRuleCandidate::try_from_elimination_pivot(
        &context,
        &rows,
        &elimination,
        pivot_ordinal,
        sector,
        ParametricRuleLimits::default(),
    )
    .unwrap();
    Fixture { context, candidate }
}

fn disposition(fixture: &Fixture, indices: &[i64]) -> WhenBadLeafDisposition {
    let WhenBadCompilation::Certified(certificate) = WhenBadCompiler::compile_algebraic_candidate(
        &fixture.context,
        &fixture.candidate,
        WhenBadCompilerLimits::default(),
    )
    .unwrap() else {
        panic!("fixture must compile")
    };
    certificate
        .classification_for_indices(&fixture.context, indices)
        .unwrap()
        .unwrap()
        .disposition()
        .clone()
}

#[test]
fn delta_two_leak_enumerates_both_boundaries_and_drops_identically_zero_gate() {
    let fixture = synthetic_candidate(
        "delta-two",
        CoefficientContext::new(Vec::<String>::new()),
        SectorMask::try_new([false]).unwrap(),
        vec![-3],
        vec![2],
        |context| {
            context
                .add(&context.index(0).unwrap(), &context.one())
                .unwrap()
        },
        |_, _| {},
    );
    let WhenBadCompilation::Certified(certificate) = WhenBadCompiler::compile_algebraic_candidate(
        &fixture.context,
        &fixture.candidate,
        WhenBadCompilerLimits::default(),
    )
    .unwrap() else {
        panic!("descending numerator rule must compile")
    };
    assert_eq!(certificate.stats().boundary_values_examined(), 2);
    assert_eq!(certificate.stats().leak_events(), 1);
    assert!(matches!(
        disposition(&fixture, &[-1]),
        WhenBadLeafDisposition::CoveredByCandidate
    ));
    assert!(matches!(
        disposition(&fixture, &[0]),
        WhenBadLeafDisposition::ExceptionalSectorLeak { .. }
    ));
    assert!(matches!(
        disposition(&fixture, &[-2]),
        WhenBadLeafDisposition::CoveredByCandidate
    ));
    certificate.replay(&fixture.context).unwrap();
}

#[test]
fn symbolic_numerator_gate_recovers_a_safe_subcase_sharper_than_litered() {
    // On n1=0 the target activates an inactive line. LiteRed keeps that whole
    // boundary because n0-2 is not identically zero. RustRed proves the exact
    // refinement: n0=2 is safe, while the rest of n1=0 is exceptional.
    let fixture = synthetic_candidate(
        "sharp-gate",
        CoefficientContext::new(Vec::<String>::new()),
        SectorMask::try_new([true, false]).unwrap(),
        vec![2, -2],
        vec![0, 1],
        |context| {
            context
                .sub(&context.index(0).unwrap(), &context.integer(2))
                .unwrap()
        },
        |_, _| {},
    );
    assert!(matches!(
        disposition(&fixture, &[2, 0]),
        WhenBadLeafDisposition::CoveredByCandidate
    ));
    assert!(matches!(
        disposition(&fixture, &[3, 0]),
        WhenBadLeafDisposition::ExceptionalSectorLeak { .. }
    ));
    assert!(matches!(
        disposition(&fixture, &[3, -1]),
        WhenBadLeafDisposition::CoveredByCandidate
    ));
}

#[test]
fn base_only_guard_is_an_assumption_while_index_denominator_splits() {
    let base = CoefficientContext::new(["theta"]);
    let fixture = synthetic_candidate(
        "domain-guards",
        base.clone(),
        SectorMask::try_new([true]).unwrap(),
        vec![3],
        vec![-1],
        |context| {
            let denominator = context
                .sub(&context.index(0).unwrap(), &context.one())
                .unwrap();
            context.checked_div(&context.one(), &denominator).unwrap()
        },
        |context, row| {
            let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
            row.add_nonzero_condition(context, context.numerator_condition(&theta).unwrap())
                .unwrap();
        },
    );
    let WhenBadCompilation::Certified(certificate) = WhenBadCompiler::compile_algebraic_candidate(
        &fixture.context,
        &fixture.candidate,
        WhenBadCompilerLimits::default(),
    )
    .unwrap() else {
        panic!("guarded descending rule must compile")
    };
    assert!(certificate.base_domain_guards().any(|condition| {
        condition
            .polynomial()
            .to_expression()
            .to_string()
            .contains("theta")
    }));
    assert!(certificate.index_domain_guards().any(|condition| {
        condition
            .polynomial()
            .to_expression()
            .to_string()
            .contains("n0")
    }));
    assert!(matches!(
        certificate
            .classification_for_indices(&fixture.context, &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        WhenBadLeafDisposition::ExceptionalDomain { .. }
    ));
    assert!(matches!(
        certificate
            .classification_for_indices(&fixture.context, &[2])
            .unwrap()
            .unwrap()
            .disposition(),
        WhenBadLeafDisposition::CoveredByCandidate
    ));
}

#[test]
fn same_sector_non_descent_is_explicitly_unsupported() {
    let fixture = synthetic_candidate_with_pivot_shift(
        "unsupported-ascent",
        CoefficientContext::new(Vec::<String>::new()),
        SectorMask::try_new([true]).unwrap(),
        // At the discovery anchor both points are in the inactive orthant and
        // +1 is simpler, so elimination legitimately retains the zero-shift
        // pivot.  The candidate is deliberately compiled for the active
        // sector, where the same +1 shift is a uniform ascent.
        vec![-2],
        vec![0],
        vec![1],
        |context| context.one(),
    );
    let compilation = WhenBadCompiler::compile_algebraic_candidate(
        &fixture.context,
        &fixture.candidate,
        WhenBadCompilerLimits::default(),
    )
    .unwrap();
    let WhenBadCompilation::Unsupported(unsupported) = compilation else {
        panic!("non-descending candidate must never be certified")
    };
    assert!(matches!(
        unsupported.reason(),
        WhenBadUnsupportedReason::NonUniformSameSectorDescent { delta: 1, .. }
    ));
    unsupported.replay(&fixture.context).unwrap();
}

#[test]
fn huge_boundary_range_is_rejected_before_enumeration() {
    let fixture = synthetic_candidate_with_pivot_shift(
        "bounded-range",
        CoefficientContext::new(Vec::<String>::new()),
        SectorMask::try_new([false]).unwrap(),
        // Both scout points remain inactive and the positive target is closer
        // to zero, so the requested source remains the elimination pivot.
        vec![-2_000_000],
        vec![0],
        vec![1_000_001],
        |context| context.one(),
    );
    let limits = WhenBadCompilerLimits::default();
    assert!(matches!(
        WhenBadCompiler::compile_algebraic_candidate(&fixture.context, &fixture.candidate, limits,),
        Err(WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad boundary values per RHS",
            requested: 1_000_001,
            limit: 1_000_000,
        })
    ));
}
