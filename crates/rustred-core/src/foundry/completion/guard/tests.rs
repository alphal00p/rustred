use crate::algebra::{
    CoefficientContext, IndexedAlgebraError, IndexedCoefficientContext, IndexedGuardLimits,
};

use super::{CoefficientIdealGuardAtom, CoefficientIdealGuardError, CoefficientIdealGuardLimits};

fn polynomial(
    context: &IndexedCoefficientContext,
    value: &crate::algebra::IndexedCoefficient,
) -> crate::algebra::IndexedPolynomial {
    context
        .numerator_condition_with_limits(value, Default::default())
        .unwrap()
}

#[test]
fn generic_parameter_guard_becomes_one_simultaneous_coefficient_ideal() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "semantic-guard", 2).unwrap();
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let n0_minus_one = context
        .sub(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let n1_minus_one = context
        .sub(&context.index(1).unwrap(), &context.one())
        .unwrap();
    let guard = context
        .add(&context.mul(&d, &n0_minus_one).unwrap(), &n1_minus_one)
        .unwrap();
    let atom = CoefficientIdealGuardAtom::try_from_pulled_back(
        &context,
        polynomial(&context, &guard),
        Default::default(),
    )
    .unwrap();

    assert_eq!(atom.coefficient_system().equations().len(), 2);
    assert_eq!(atom.id().generators().len(), 2);
    assert!(!atom.has_literal_unit_generator());
    assert!(atom.try_verify(&context, Default::default()).unwrap());
}

#[test]
fn primitive_duplicate_generators_merge_without_claiming_radical_canonicality() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "semantic-associates", 1).unwrap();
    let d_plus_one = context
        .add(
            &context.lift(&base.parameter("d").unwrap()).unwrap(),
            &context.one(),
        )
        .unwrap();
    let n_minus_one = context
        .sub(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let product = context.mul(&d_plus_one, &n_minus_one).unwrap();
    let expanded = CoefficientIdealGuardAtom::try_from_pulled_back(
        &context,
        polynomial(&context, &product),
        Default::default(),
    )
    .unwrap();
    let direct = CoefficientIdealGuardAtom::try_from_pulled_back(
        &context,
        polynomial(&context, &n_minus_one),
        Default::default(),
    )
    .unwrap();

    assert_eq!(expanded.coefficient_system().equations().len(), 2);
    assert_eq!(expanded.id().generators().len(), 1);
    assert!(expanded.same_retained_ideal(&direct));

    let foreign =
        IndexedCoefficientContext::try_new(&base, "semantic-associates-foreign", 1).unwrap();
    let foreign_guard = foreign
        .sub(&foreign.index(0).unwrap(), &foreign.one())
        .unwrap();
    let foreign = CoefficientIdealGuardAtom::try_from_pulled_back(
        &foreign,
        polynomial(&foreign, &foreign_guard),
        Default::default(),
    )
    .unwrap();
    assert!(!direct.same_retained_ideal(&foreign));
}

#[test]
fn literal_unit_zero_and_resource_boundaries_are_typed() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "semantic-limits", 2).unwrap();
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let unit = CoefficientIdealGuardAtom::try_from_pulled_back(
        &context,
        polynomial(&context, &d),
        Default::default(),
    )
    .unwrap();
    assert!(unit.has_literal_unit_generator());

    assert_eq!(
        CoefficientIdealGuardAtom::try_from_pulled_back(
            &context,
            polynomial(&context, &context.zero()),
            Default::default(),
        )
        .unwrap_err(),
        CoefficientIdealGuardError::IdenticallyZeroGuard
    );

    let multivariate = context
        .add(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap();
    let mut byte_limited = CoefficientIdealGuardLimits::default();
    byte_limited.max_generator_identity_bytes = 0;
    assert!(matches!(
        CoefficientIdealGuardAtom::try_from_pulled_back(
            &context,
            polynomial(&context, &multivariate),
            byte_limited,
        ),
        Err(CoefficientIdealGuardError::ResourceLimit {
            resource: "coefficient-ideal guard generator identity bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let d_n0_plus_n1 = context
        .add(
            &context.mul(&d, &context.index(0).unwrap()).unwrap(),
            &context.index(1).unwrap(),
        )
        .unwrap();
    let mut equation_limited = CoefficientIdealGuardLimits::default();
    equation_limited.guard_algebra = IndexedGuardLimits {
        max_coefficient_equations: 1,
        ..IndexedGuardLimits::default()
    };
    assert!(matches!(
        CoefficientIdealGuardAtom::try_from_pulled_back(
            &context,
            polynomial(&context, &d_n0_plus_n1),
            equation_limited,
        ),
        Err(CoefficientIdealGuardError::IndexedAlgebra(
            IndexedAlgebraError::ResourceLimit {
                resource: "guard coefficient equations",
                requested: 2,
                limit: 1,
            }
        ))
    ));
}

#[test]
fn target_pullback_precedes_parameter_coefficient_split() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "semantic-target-pullback", 2).unwrap();
    let source = context
        .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap();
    let pulled = CoefficientIdealGuardAtom::try_for_target(
        &context,
        polynomial(&context, &source),
        &[1, 0],
        Default::default(),
    )
    .unwrap();
    let expected = context
        .sub(
            &context
                .sub(&context.index(0).unwrap(), &context.one())
                .unwrap(),
            &context.index(1).unwrap(),
        )
        .unwrap();
    let expected = CoefficientIdealGuardAtom::try_from_pulled_back(
        &context,
        polynomial(&context, &expected),
        Default::default(),
    )
    .unwrap();
    assert!(pulled.same_retained_ideal(&expected));

    assert!(matches!(
        CoefficientIdealGuardAtom::try_for_target(
            &context,
            polynomial(&context, &source),
            &[1],
            Default::default(),
        ),
        Err(CoefficientIdealGuardError::IndexedAlgebra(
            IndexedAlgebraError::WrongIndexArity {
                expected: 2,
                actual: 1,
            }
        ))
    ));

    assert_eq!(
        CoefficientIdealGuardAtom::try_for_target(
            &context,
            polynomial(&context, &source),
            &[i64::MIN, 0],
            Default::default(),
        )
        .unwrap_err(),
        CoefficientIdealGuardError::TargetPullbackOverflow {
            index: 0,
            shift: i64::MIN,
        }
    );
}
