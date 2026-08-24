//! Independent black-box audit of generic guarded `K(n)` elimination.
//!
//! These fixtures contain no production recurrences.  One synthetic exact
//! matrix checks the field algebra, and one generated one-loop IBP checks the
//! real generic producer/solver boundary.

use rustred::{
    AffineDenominator, CoefficientContext, ExactAlgebraLimits, GuardOrigin, IndexSpace,
    IntegralFamily, IntegralOrderingPolicy, ParametricArithmeticLimits, ParametricCoefficient,
    ParametricCoefficientContext, ParametricElimination, ParametricEliminationError,
    ParametricEliminationLimits, ParametricEliminationOrdering, ParametricIbpGenerator,
    ParametricRelation, ParametricRelationError, ParametricRowId,
};

fn row(label: &'static str) -> ParametricRowId {
    ParametricRowId::Derived {
        label: label.into(),
    }
}

fn assert_coefficient_eq(
    context: &ParametricCoefficientContext,
    actual: &ParametricCoefficient,
    expected: &ParametricCoefficient,
) {
    assert!(context.sub(actual, expected).unwrap().is_zero());
}

fn synthetic_context(scope: &str) -> ParametricCoefficientContext {
    ParametricCoefficientContext::try_new(&CoefficientContext::new(Vec::<String>::new()), scope, 1)
        .unwrap()
}

fn synthetic_rows(
    context: &ParametricCoefficientContext,
) -> (ParametricRelation, ParametricRelation) {
    let space = IndexSpace::try_new(1).unwrap();
    let plus = space.unit(0, 1).unwrap();
    let zero = space.zero();
    let minus = space.unit(0, -1).unwrap();
    let n = context.index(0).unwrap();

    // r0 = n J(n+1) - J(n).
    let mut first = ParametricRelation::new("synthetic-family", row("r0"), context);
    first.add_term(context, plus.clone(), n).unwrap();
    first.add_term(context, zero, context.integer(-1)).unwrap();

    // r1 = J(n+1) + J(n-1).
    let mut second = ParametricRelation::new("synthetic-family", row("r1"), context);
    second.add_term(context, plus, context.one()).unwrap();
    second.add_term(context, minus, context.one()).unwrap();
    (first, second)
}

fn ordering(anchor: i64) -> ParametricEliminationOrdering {
    ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [anchor])
        .unwrap()
}

#[test]
fn exact_two_row_elimination_has_the_hand_derived_echelon_form() {
    let context = synthetic_context("elimination-black-box-two-row");
    let (first, second) = synthetic_rows(&context);
    let elimination = ParametricElimination::build(
        &context,
        &[first.clone(), second.clone()],
        ordering(1),
        ParametricEliminationLimits::default(),
    )
    .unwrap();

    assert_eq!(elimination.stats().rank(), 2);
    assert_eq!(elimination.stats().columns(), 3);
    assert_eq!(elimination.free_columns().len(), 1);
    assert_eq!(elimination.free_columns()[0].values(), &[-1]);
    assert_eq!(elimination.pivots()[0].pivot().values(), &[1]);
    assert_eq!(elimination.pivots()[1].pivot().values(), &[0]);

    let space = IndexSpace::try_new(1).unwrap();
    let plus = space.unit(0, 1).unwrap();
    let zero = space.zero();
    let minus = space.unit(0, -1).unwrap();
    let n = context.index(0).unwrap();
    let minus_inverse_n = context.checked_div(&context.integer(-1), &n).unwrap();
    let first_pivot = elimination.pivots()[0].unit_relation();
    assert_coefficient_eq(
        &context,
        first_pivot.terms().get(&plus).unwrap(),
        &context.one(),
    );
    assert_coefficient_eq(
        &context,
        first_pivot.terms().get(&zero).unwrap(),
        &minus_inverse_n,
    );

    // r1-r0/n = J(n-1)+J(n)/n, then normalization gives
    // J(n)+n J(n-1)=0.
    let second_pivot = elimination.pivots()[1].unit_relation();
    assert_coefficient_eq(
        &context,
        second_pivot.terms().get(&zero).unwrap(),
        &context.one(),
    );
    assert_coefficient_eq(&context, second_pivot.terms().get(&minus).unwrap(), &n);
    assert_eq!(elimination.pivots()[1].trace().reductions().len(), 1);
    assert_eq!(
        elimination.pivots()[1].trace().reductions()[0].prior_pivot_ordinal(),
        0
    );
    assert_coefficient_eq(
        &context,
        elimination.pivots()[1].trace().reductions()[0].factor(),
        &context.one(),
    );

    elimination.replay(&context, &[first, second]).unwrap();
    for pivot in elimination.pivots() {
        assert!(
            pivot
                .unit_relation()
                .guarded_nonzero_conditions()
                .iter()
                .any(|condition| condition
                    .origins()
                    .contains(&GuardOrigin::GuardedDivisionDivisorNumerator))
        );
    }
}

#[test]
fn replay_rejects_guard_provenance_changes_in_a_used_source_row() {
    let context = synthetic_context("elimination-black-box-guard-replay");
    let (source, _) = synthetic_rows(&context);
    let elimination = ParametricElimination::build(
        &context,
        &[source.clone()],
        ordering(1),
        ParametricEliminationLimits::default(),
    )
    .unwrap();

    let mut changed = source;
    let n_plus_one = context
        .add(&context.index(0).unwrap(), &context.one())
        .unwrap();
    changed
        .add_nonzero_condition(&context, context.numerator_condition(&n_plus_one).unwrap())
        .unwrap();
    assert!(matches!(
        elimination.replay(&context, &[changed]),
        Err(ParametricEliminationError::InternalReplayFailure { .. })
    ));
}

#[test]
fn replay_binds_the_complete_ordered_source_manifest_not_only_its_span() {
    let context = synthetic_context("elimination-black-box-source-manifest");
    let (source, _) = synthetic_rows(&context);
    let elimination = ParametricElimination::build(
        &context,
        &[source.clone()],
        ordering(1),
        ParametricEliminationLimits::default(),
    )
    .unwrap();

    // Algebraically this is the same equation. Its stable source identity is
    // deliberately different, so it must not authenticate the certificate.
    let mut renamed = ParametricRelation::new(
        "synthetic-family",
        row("same-equation-different-source"),
        &context,
    );
    for (shift, coefficient) in source.terms() {
        renamed
            .add_term(&context, shift.clone(), coefficient.clone())
            .unwrap();
    }
    assert!(matches!(
        elimination.replay(&context, &[renamed]),
        Err(ParametricEliminationError::InternalReplayFailure { detail })
            if detail.contains("source-row manifest")
    ));
}

fn tadpole_family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "elimination-black-box-tadpole",
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
fn generated_one_loop_tadpole_ibp_eliminates_and_centers_parametrically() {
    let family = tadpole_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let generated = generator.generate_ordinary_ibp().unwrap();
    assert_eq!(generated.len(), 1);
    let context = generator.context();
    let n = context.index(0).unwrap();
    let d = context.lift(family.dimension()).unwrap();
    let m2 = context
        .lift(&family.coefficient_context().parameter("m2").unwrap())
        .unwrap();
    let two_n = context.mul(&context.integer(2), &n).unwrap();
    let expected_zero = context.sub(&d, &two_n).unwrap();
    let expected_plus = context.mul(&two_n, &m2).unwrap();
    let space = IndexSpace::try_new(1).unwrap();
    assert_coefficient_eq(
        context,
        generated[0].terms().get(&space.zero()).unwrap(),
        &expected_zero,
    );
    assert_coefficient_eq(
        context,
        generated[0]
            .terms()
            .get(&space.unit(0, 1).unwrap())
            .unwrap(),
        &expected_plus,
    );

    let elimination = ParametricElimination::build(
        context,
        &generated,
        ordering(1),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    assert_eq!(elimination.stats().rank(), 1);
    assert_eq!(elimination.pivots()[0].pivot().values(), &[1]);
    elimination.replay(context, &generated).unwrap();
    assert!(
        elimination.pivots()[0]
            .unit_relation()
            .guarded_nonzero_conditions()
            .iter()
            .any(|condition| condition
                .origins()
                .contains(&GuardOrigin::GuardedDivisionDivisorNumerator))
    );

    let centered = elimination.pivots()[0]
        .centered_relation(context, ParametricArithmeticLimits::default())
        .unwrap();
    assert_eq!(centered.terms().get(&space.zero()), Some(&context.one()));
    assert!(matches!(
        centered.specialize(context, &[1], ParametricArithmeticLimits::default()),
        Err(ParametricRelationError::UnsatisfiableDomain)
    ));
    let concrete = centered
        .specialize(context, &[2], ParametricArithmeticLimits::default())
        .unwrap();
    assert_eq!(concrete.terms().len(), 2);
    let expected = family.coefficient_context().parse("(d-2)/(2*m2)").unwrap();
    let lower = concrete
        .terms()
        .iter()
        .find(|(key, _)| key.powers() == [1])
        .map(|(_, coefficient)| coefficient)
        .unwrap();
    assert_eq!(lower, &expected);
}

#[test]
fn malformed_scope_and_preallocation_limits_fail_typed() {
    let context = synthetic_context("elimination-black-box-errors");
    assert_eq!(
        ParametricElimination::build(
            &context,
            &[],
            ordering(1),
            ParametricEliminationLimits::default(),
        )
        .unwrap_err(),
        ParametricEliminationError::EmptySourceRows
    );

    let (source, _) = synthetic_rows(&context);
    let wrong_arity =
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1, 1])
            .unwrap();
    assert!(matches!(
        ParametricElimination::build(
            &context,
            &[source.clone()],
            wrong_arity,
            ParametricEliminationLimits::default(),
        ),
        Err(ParametricEliminationError::WrongArity {
            expected: 1,
            actual: 2,
        })
    ));

    let mut strict = ParametricEliminationLimits::default();
    strict.max_columns = 1;
    assert!(matches!(
        ParametricElimination::build(&context, &[source.clone()], ordering(1), strict),
        Err(ParametricEliminationError::ResourceLimit {
            resource: "parametric columns",
            requested: 2,
            limit: 1,
        })
    ));

    assert!(matches!(
        ParametricElimination::build(
            &context,
            &[source],
            ordering(i64::MAX),
            ParametricEliminationLimits::default(),
        ),
        Err(ParametricEliminationError::IndexOverflow { position: 0 })
    ));
}

/// Audit regression: source guards must be authenticated under caller limits
/// even when a zero mathematical row contributes no pivot.
#[test]
fn strict_arithmetic_limits_cover_source_guard_polynomials() {
    let context = synthetic_context("elimination-audit-source-guard-limit");
    let n = context.index(0).unwrap();
    let n_squared = context.mul(&n, &n).unwrap();
    let mut zero = ParametricRelation::new("synthetic-family", row("guarded-zero"), &context);
    zero.add_nonzero_condition(&context, context.numerator_condition(&n_squared).unwrap())
        .unwrap();
    let limits = ParametricEliminationLimits {
        arithmetic: ParametricArithmeticLimits {
            exact_algebra: ExactAlgebraLimits {
                max_exponent: 1,
                ..ExactAlgebraLimits::default()
            },
            ..ParametricArithmeticLimits::default()
        },
        ..ParametricEliminationLimits::default()
    };
    assert!(ParametricElimination::build(&context, &[zero], ordering(1), limits).is_err());
}

/// Audit regression: normalizing a pivot performs sparse updates and must be
/// charged to the construction budget even when there are no prior pivots.
#[test]
fn pivot_normalization_obeys_the_sparse_update_budget() {
    let context = synthetic_context("elimination-audit-normalization-updates");
    let (source, _) = synthetic_rows(&context);
    let limits = ParametricEliminationLimits {
        max_sparse_updates: 0,
        ..ParametricEliminationLimits::default()
    };
    assert!(matches!(
        ParametricElimination::build(&context, &[source], ordering(1), limits),
        Err(ParametricEliminationError::ResourceLimit {
            resource: "sparse updates",
            ..
        })
    ));
}
