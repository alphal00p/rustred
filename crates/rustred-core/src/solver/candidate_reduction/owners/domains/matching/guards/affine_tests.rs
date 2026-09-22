use super::*;
use crate::algebra::{
    CoefficientContext, IndexedAlgebraError, IndexedCoefficient, IndexedGuardLimits,
};

const OWNER: [bool; 3] = [true, true, false];
fn context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(&CoefficientContext::new(["d"]), "affine-box-guards", 3)
        .unwrap()
}
fn sum(c: &IndexedCoefficientContext, constant: i64) -> IndexedCoefficient {
    c.add(
        &c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
        &c.integer(constant),
    )
    .unwrap()
}
fn polynomial(c: &IndexedCoefficientContext, value: &IndexedCoefficient) -> IndexedPolynomial {
    c.numerator_condition_with_limits(value, Default::default())
        .unwrap()
}
fn resolve_in(
    c: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
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
        &polynomial(c, value),
        &LatticeBox::try_new(lower, upper).unwrap(),
        &OWNER,
        rank,
        Default::default(),
        &mut budget,
    )
}

#[test]
fn affine_guard_box_actual_positive_lower_bounds_exclude_zero_not_generic_corner() {
    let c = context();
    let p = sum(&c, -2);
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [3, 1, 0],
            [None, None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Nonzero
    ));
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [0; 3],
            [None, None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Unknown
    ));
    assert!(matches!(
        resolve_in(&c, &p, [0; 3], [Some(0); 3], Some(0), Default::default()).unwrap(),
        Resolution::Zero
    ));
}

#[test]
fn affine_guard_box_negative_slopes_reverse_finite_endpoint_and_infinity() {
    let c = context();
    let p = c.sub(&c.zero(), &sum(&c, -2)).unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [3, 1, 0],
            [None, None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Nonzero
    ));
    let q = sum(&c, -10);
    assert!(matches!(
        resolve_in(
            &c,
            &q,
            [0; 3],
            [Some(1), Some(2), Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Nonzero
    ));
    assert!(matches!(
        resolve_in(
            &c,
            &q,
            [0; 3],
            [None, None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Unknown
    ));
}

#[test]
fn affine_guard_box_actual_rank_tightens_inactive_not_positive_axes() {
    let c = context();
    let p = c.add(&c.index(2).unwrap(), &c.integer(11)).unwrap();
    assert!(matches!(
        resolve_in(&c, &p, [0; 3], [None; 3], Some(10), Default::default()).unwrap(),
        Resolution::Nonzero
    ));
    assert!(matches!(
        resolve_in(&c, &p, [0; 3], [None; 3], Some(11), Default::default()).unwrap(),
        Resolution::Planes { .. }
    ));
    assert!(matches!(
        resolve_in(&c, &p, [0; 3], [None; 3], None, Default::default()).unwrap(),
        Resolution::Planes { .. }
    ));
    let positive = c.sub(&c.index(0).unwrap(), &c.integer(100)).unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &positive,
            [0; 3],
            [None; 3],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Planes { .. }
    ));
}

#[test]
fn affine_guard_box_mixed_diagonals_and_nonlinear_monomials_keep_native_fallback() {
    let c = context();
    let diagonal = c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap();
    let nonlinear = c
        .add(
            &c.mul(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.one(),
        )
        .unwrap();
    for p in [diagonal, nonlinear] {
        assert!(matches!(
            resolve_in(
                &c,
                &p,
                [0; 3],
                [None, None, Some(0)],
                Some(0),
                Default::default()
            )
            .unwrap(),
            Resolution::Unknown
        ));
    }
}

#[test]
fn affine_guard_box_one_native_base_equation_suffices_without_parameter_sign() {
    let c = context();
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let nonlinear = c.mul(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap();
    let p = c
        .add(&nonlinear, &c.mul(&d, &sum(&c, -2)).unwrap())
        .unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [3, 1, 0],
            [None, None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Nonzero
    ));
    let diagonal = c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap();
    let q = c.mul(&d, &diagonal).unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &q,
            [0; 3],
            [None, None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Unknown
    ));
}

#[test]
fn affine_guard_box_native_fixed_specialization_precedes_affine_test() {
    let c = context();
    let p = c
        .add(
            &c.mul(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.one(),
        )
        .unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [0; 3],
            [Some(0), None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Nonzero
    ));
}

#[test]
fn affine_guard_box_large_unfixed_endpoint_is_exact_fixed_carrier_boundary_unchanged() {
    let c = context();
    let p = c.sub(&c.index(0).unwrap(), &c.integer(i64::MAX)).unwrap();
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [u64::MAX, 0, 0],
            [None, None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Nonzero
    ));
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [u64::MAX, 0, 0],
            [Some(u64::MAX), None, Some(0)],
            Some(0),
            Default::default()
        )
        .unwrap(),
        Resolution::Unknown
    ));
}

#[test]
fn affine_guard_box_input_limits_and_fallback_native_errors_are_not_bypassed() {
    let c = context();
    let p = sum(&c, 1);
    assert!(matches!(
        resolve_in(
            &c,
            &p,
            [0; 3],
            [None, None, Some(0)],
            Some(0),
            IndexedGuardLimits {
                max_input_terms: 1,
                ..Default::default()
            }
        ),
        Err(OwnerDomainMatchFailure::Algebra(
            IndexedAlgebraError::ResourceLimit { .. }
        ))
    ));
    assert!(matches!(
        resolve_in(
            &c,
            &p,
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

#[test]
fn affine_guard_box_optional_scratch_admission_and_empty_rank_do_not_invent_proof() {
    let c = context();
    let p = polynomial(&c, &sum(&c, 1));
    let system = c
        .base_coefficient_system(&p, Default::default(), Default::default())
        .unwrap();
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    assert!(
        !affine::misses_zero(
            &system,
            1,
            &cell,
            &OWNER,
            Some(0),
            Default::default(),
            IndexedGuardLimits {
                max_total_integer_bits: 1,
                ..Default::default()
            }
        )
        .unwrap()
    );
    assert!(
        !affine::misses_zero(
            &system,
            1,
            &cell,
            &OWNER,
            Some(0),
            IndexedAlgebraLimits {
                max_specialization_integer_bits: 1,
                ..Default::default()
            },
            Default::default()
        )
        .unwrap()
    );
    let empty = LatticeBox::try_new([0, 0, 11], [None; 3]).unwrap();
    assert!(
        !affine::misses_zero(
            &system,
            1,
            &empty,
            &OWNER,
            Some(10),
            Default::default(),
            Default::default()
        )
        .unwrap()
    );
}

#[test]
fn affine_guard_box_residual_rank_subtracts_other_inactive_lower_minima() {
    let c = context();
    let p = polynomial(&c, &c.add(&c.index(1).unwrap(), &c.integer(6)).unwrap());
    let system = c
        .base_coefficient_system(&p, Default::default(), Default::default())
        .unwrap();
    let cell = LatticeBox::try_new([0, 0, 5], [None; 3]).unwrap();
    let owner = [true, false, false];
    // x2 >= 5 leaves x1 <= 5 at rank10, so n1+6 >= 1. Rank11
    // includes x1=6,x2=5 and must not receive that nonzero proof.
    assert!(
        affine::misses_zero(
            &system,
            1,
            &cell,
            &owner,
            Some(10),
            Default::default(),
            Default::default()
        )
        .unwrap()
    );
    assert!(
        !affine::misses_zero(
            &system,
            1,
            &cell,
            &owner,
            Some(11),
            Default::default(),
            Default::default()
        )
        .unwrap()
    );
}
