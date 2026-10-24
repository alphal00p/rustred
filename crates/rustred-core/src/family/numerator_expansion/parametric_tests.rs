//! Parameter-dependent affine constants: virtual native axes plus exact
//! coefficient-context substitution.

use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};

use super::{
    MultiAffineNumeratorExpansionError, MultiAffineNumeratorExpansionLimits,
    MultiAffineNumeratorFactor, try_expand_multi_affine_numerator,
    try_expand_parametric_multi_affine_numerator_with_usage,
};

/// Two-loop sunrise-like family with one `eta`-deformed line:
/// `D0 = k1^2 - 1 - eta`, `D1 = k2^2 - 1`, `D2 = (k1 + k2)^2 - 1`.
fn family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "eta"]);
    let eta = context.parameter("eta").unwrap();
    let minus_one = context.integer(-1);
    let deformed = context
        .try_sub(&minus_one, &eta, ExactAlgebraLimits::default())
        .unwrap();
    let row = |values: [i64; 3]| values.map(|value| context.integer(value)).to_vec();
    IntegralFamily::new(
        "deformed_sunrise",
        vec!["k1".into(), "k2".into()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(deformed, row([1, 0, 0])),
            AffineDenominator::new(minus_one.clone(), row([0, 0, 1])),
            AffineDenominator::new(minus_one, row([1, 2, 1])),
        ],
        vec![],
        vec![context.zero(); 3],
    )
    .unwrap()
}

fn factor(
    family: &IntegralFamily,
    constant: Coefficient,
    linear: [i64; 3],
    power: u64,
) -> MultiAffineNumeratorFactor {
    let context = family.coefficient_context();
    MultiAffineNumeratorFactor::try_new(constant, linear.map(|value| context.integer(value)), power)
        .unwrap()
}

fn eta(family: &IntegralFamily) -> Coefficient {
    family.coefficient_context().parameter("eta").unwrap()
}

fn expand(
    family: &IntegralFamily,
    base: [i64; 3],
    factors: &[MultiAffineNumeratorFactor],
) -> Result<BTreeMap<IntegralKey, Coefficient>, MultiAffineNumeratorExpansionError> {
    Ok(try_expand_parametric_multi_affine_numerator_with_usage(
        family,
        &IntegralKey::try_new(base).unwrap(),
        factors,
        MultiAffineNumeratorExpansionLimits::default(),
        |_| Ok(()),
    )?
    .iter()
    .map(|endpoint| (endpoint.key().clone(), endpoint.coefficient().clone()))
    .collect())
}

fn key(powers: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

#[test]
fn rational_constants_take_the_unchanged_native_lane() {
    let family = family();
    let context = family.coefficient_context();
    let factors = [
        factor(&family, context.integer(3), [1, -2, 0], 2),
        factor(&family, context.integer(-1), [0, 1, 1], 1),
    ];
    let base = IntegralKey::try_new([2, 1, 1]).unwrap();
    let native =
        try_expand_multi_affine_numerator(&family, &base, &factors, Default::default()).unwrap();
    let parametric = try_expand_parametric_multi_affine_numerator_with_usage(
        &family,
        &base,
        &factors,
        Default::default(),
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(native, parametric);
}

#[test]
fn deformed_constant_powers_are_substituted_exactly() {
    let family = family();
    let context = family.coefficient_context();
    let limits = ExactAlgebraLimits::default();
    let eta = eta(&family);
    // (eta + D0)^2 = eta^2 + 2 eta D0 + D0^2 on the base key [1, 1, 1].
    let endpoints = expand(
        &family,
        [1, 1, 1],
        &[factor(&family, eta.clone(), [1, 0, 0], 2)],
    )
    .unwrap();
    let eta_squared = context.try_mul(&eta, &eta, limits).unwrap();
    let two_eta = context.try_mul(&context.integer(2), &eta, limits).unwrap();
    assert_eq!(
        endpoints,
        BTreeMap::from([
            (key([1, 1, 1]), eta_squared),
            (key([0, 1, 1]), two_eta),
            (key([-1, 1, 1]), context.one()),
        ])
    );
}

#[test]
fn lifted_monomials_coalesce_and_cancel_exactly() {
    let family = family();
    let context = family.coefficient_context();
    let limits = ExactAlgebraLimits::default();
    let eta = eta(&family);
    let minus_eta = context.try_neg(&eta, limits).unwrap();
    // (eta + D0)(-eta + D0) = D0^2 - eta^2: the two linear terms live on
    // distinct virtual monomials but land on one key and cancel exactly.
    let endpoints = expand(
        &family,
        [1, 1, 1],
        &[
            factor(&family, eta.clone(), [1, 0, 0], 1),
            factor(&family, minus_eta, [1, 0, 0], 1),
        ],
    )
    .unwrap();
    let minus_eta_squared = context
        .try_neg(&context.try_mul(&eta, &eta, limits).unwrap(), limits)
        .unwrap();
    assert_eq!(
        endpoints,
        BTreeMap::from([
            (key([1, 1, 1]), minus_eta_squared),
            (key([-1, 1, 1]), context.one()),
        ])
    );
}

#[test]
fn mixed_rational_and_parametric_factors_match_a_hand_expansion() {
    let family = family();
    let context = family.coefficient_context();
    let limits = ExactAlgebraLimits::default();
    let eta = eta(&family);
    // (1 + eta + D1) * (2 + D2) on [1, 1, 1]:
    //   (1+eta)*2 [1,1,1] + (1+eta) [1,1,0] + 2 [1,0,1] + [1,0,0].
    let one_plus_eta = context.try_add(&context.one(), &eta, limits).unwrap();
    let endpoints = expand(
        &family,
        [1, 1, 1],
        &[
            factor(&family, one_plus_eta.clone(), [0, 1, 0], 1),
            factor(&family, context.integer(2), [0, 0, 1], 1),
        ],
    )
    .unwrap();
    assert_eq!(
        endpoints,
        BTreeMap::from([
            (
                key([1, 1, 1]),
                context
                    .try_mul(&context.integer(2), &one_plus_eta, limits)
                    .unwrap()
            ),
            (key([1, 1, 0]), one_plus_eta),
            (key([1, 0, 1]), context.integer(2)),
            (key([1, 0, 0]), context.one()),
        ])
    );
}

#[test]
fn parametric_linear_coefficients_remain_rejected() {
    let family = family();
    let context = family.coefficient_context();
    let factor = MultiAffineNumeratorFactor::try_new(
        eta(&family),
        [eta(&family), context.zero(), context.zero()],
        1,
    )
    .unwrap();
    assert!(matches!(
        expand(&family, [1, 1, 1], &[factor]),
        Err(MultiAffineNumeratorExpansionError::NonconstantExpansionCoefficient { .. })
    ));
}

#[test]
fn shared_budgets_are_charged_for_the_substitution_boundary() {
    let family = family();
    let context = family.coefficient_context();
    let eta = eta(&family);
    let base = IntegralKey::try_new([1, 1, 1]).unwrap();
    let rational = [factor(&family, context.integer(2), [1, 0, 0], 2)];
    let deformed = [factor(&family, eta, [1, 0, 0], 2)];
    let usage = |factors: &[MultiAffineNumeratorFactor]| {
        let seen = std::cell::Cell::new(None);
        try_expand_parametric_multi_affine_numerator_with_usage(
            &family,
            &base,
            factors,
            Default::default(),
            |usage| {
                seen.set(Some((usage.operations, usage.endpoints)));
                Ok(())
            },
        )
        .unwrap();
        seen.get().expect("one reservation")
    };
    let (native_operations, _) = usage(&rational);
    let (operations, endpoints) = usage(&deformed);
    // One virtual axis: two checked operations per projected lifted term on
    // top of the native expansion work.
    assert!(endpoints > 0);
    assert!(operations >= native_operations + 2 * endpoints);
    // An exhausted shared budget refuses the work before any expansion.
    assert!(matches!(
        try_expand_parametric_multi_affine_numerator_with_usage(
            &family,
            &base,
            &deformed,
            Default::default(),
            |usage| {
                if usage.operations > native_operations {
                    Err(MultiAffineNumeratorExpansionError::ResourceLimit {
                        resource: "shared test budget",
                        requested: usage.operations,
                        limit: native_operations,
                    })
                } else {
                    Ok(())
                }
            },
        ),
        Err(MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: "shared test budget",
            ..
        })
    ));
}
