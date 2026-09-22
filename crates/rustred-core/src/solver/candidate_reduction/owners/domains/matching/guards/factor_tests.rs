use super::*;
use crate::algebra::{
    CoefficientContext, IndexedAlgebraError, IndexedCoefficient, IndexedGuardLimits,
};

fn context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(&CoefficientContext::new(["d"]), "factor-box", 3).unwrap()
}
fn sum(c: &IndexedCoefficientContext, constant: i64) -> IndexedCoefficient {
    c.add(
        &c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
        &c.integer(constant),
    )
    .unwrap()
}
fn polynomial(c: &IndexedCoefficientContext, p: &IndexedCoefficient) -> IndexedPolynomial {
    c.numerator_condition_with_limits(p, Default::default())
        .unwrap()
}
fn resolve_in(
    c: &IndexedCoefficientContext,
    p: &IndexedCoefficient,
    owner: [bool; 3],
    lower: [u64; 3],
    upper: [Option<u64>; 3],
    rank: Option<u32>,
    limits: IndexedGuardLimits,
) -> Result<Resolution, OwnerDomainMatchFailure> {
    let mut budget = Budget {
        limits: super::super::model::OwnerDomainMatchLimits {
            guard_algebra: limits,
            ..Default::default()
        },
        stats: Default::default(),
    };
    resolve(
        c,
        &polynomial(c, p),
        &LatticeBox::try_new(lower, upper).unwrap(),
        &owner,
        rank,
        Default::default(),
        &mut budget,
    )
}

#[test]
fn factor_guard_box_products_and_repeated_coupled_factors_are_nonzero_on_positive_tails() {
    let c = context();
    let product = c
        .mul(
            &c.sub(&c.index(0).unwrap(), &c.one()).unwrap(),
            &sum(&c, -2),
        )
        .unwrap();
    for p in [&product, &c.mul(&product, &product).unwrap()] {
        assert!(matches!(
            resolve_in(
                &c,
                p,
                [true, true, false],
                [3, 1, 0],
                [None, None, Some(0)],
                Some(0),
                Default::default()
            )
            .unwrap(),
            Resolution::Nonzero
        ));
    }
}

#[test]
fn factor_guard_box_diagonal_and_actual_zero_face_remain_strict() {
    let c = context();
    let diagonal = c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap();
    let p = c.mul(&sum(&c, 1), &diagonal).unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [true, true, false],
            [0; 3],
            [None, None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Unknown
    ));
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [true, true, false],
            [2, 2, 0],
            [Some(2), Some(2), Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Zero
    ));
    let root = c
        .mul(
            &sum(&c, 1),
            &c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap(),
        )
        .unwrap();
    assert!(matches!(resolve_in(&c, &root, [true, true, false], [0; 3],
        [None, None, Some(0)], Some(0), Default::default()).unwrap(),
        Resolution::Planes { roots, exact: true } if roots == vec![(0, 1)]));
}

#[test]
fn factor_guard_box_native_base_equations_preserve_formal_parameter_semantics() {
    let c = context();
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let nonzero_product = c.mul(&sum(&c, 1), &sum(&c, 2)).unwrap();
    let diagonal = c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap();
    let p = c
        .add(
            &c.mul(&diagonal, &diagonal).unwrap(),
            &c.mul(&d, &nonzero_product).unwrap(),
        )
        .unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [true, true, false],
            [0; 3],
            [None, None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Nonzero
    ));
}

#[test]
fn factor_guard_box_negative_inactive_factor_uses_actual_rank_envelope() {
    let c = context();
    let factor = c
        .add(
            &c.add(&c.index(1).unwrap(), &c.index(2).unwrap()).unwrap(),
            &c.integer(11),
        )
        .unwrap();
    let p = c.mul(&factor, &factor).unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [true, false, false],
            [0, 2, 1],
            [None; 3],
            Some(6),
            Default::default()
        )
        .unwrap(),
        Resolution::Nonzero
    ));
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [true, false, false],
            [0, 2, 1],
            [None; 3],
            Some(11),
            Default::default()
        )
        .unwrap(),
        Resolution::Unknown
    ));
}

#[test]
fn factor_guard_box_optional_allowance_is_shared_across_equations_and_factors() {
    let c = context();
    let row = polynomial(&c, &sum(&c, 1));
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let owner = [true, true, false];
    // Three terms in four variables: scan27 + exact Integer work96 =123.
    let limits = IndexedGuardLimits {
        max_gcd_factor_work: 123,
        ..Default::default()
    };
    let mut probe = affine::Probe::new(1, &cell, &owner, Some(0), Default::default(), limits);
    assert!(probe.misses_polynomial(row.raw()).unwrap());
    assert!(!probe.misses_polynomial(row.raw()).unwrap());
    assert!(!probe.misses_polynomial(row.raw()).unwrap()); // exhaustion remains sticky
    let system = c
        .base_coefficient_system(&row, Default::default(), Default::default())
        .unwrap();
    let mut probe = affine::Probe::new(1, &cell, &owner, Some(0), Default::default(), limits);
    assert!(probe.misses_system(&system).unwrap());
    assert!(!probe.misses_polynomial(row.raw()).unwrap()); // no reset at factor transition
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let diagonal = c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap();
    let two = polynomial(
        &c,
        &c.add(&diagonal, &c.mul(&d, &sum(&c, 1)).unwrap()).unwrap(),
    );
    let two = c
        .base_coefficient_system(&two, Default::default(), Default::default())
        .unwrap();
    let mut probe = affine::Probe::new(1, &cell, &owner, Some(0), Default::default(), limits);
    // The first diagonal consumes work without a proof. The second equation
    // would fit alone, but not after the first; no per-equation reset.
    assert!(!probe.misses_system(&two).unwrap());
}

#[test]
fn factor_guard_box_admission_failure_is_not_optional_denominator_fallback() {
    let c = context();
    let p = c.mul(&sum(&c, 1), &sum(&c, 1)).unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [true, true, false],
            [0; 3],
            [None, None, Some(0)],
            Some(0),
            IndexedGuardLimits {
                max_gcd_factor_work: 0,
                ..Default::default()
            }
        ),
        Err(OwnerDomainMatchFailure::Algebra(
            IndexedAlgebraError::ResourceLimit {
                resource: "guard separable factor work",
                ..
            }
        ))
    ));
}
