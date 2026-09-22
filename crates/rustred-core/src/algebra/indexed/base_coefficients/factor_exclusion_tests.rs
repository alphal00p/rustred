use super::*;
use crate::algebra::{CoefficientContext, IndexedCoefficient};

fn context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(&CoefficientContext::new(["d"]), "factor-exclusion", 2)
        .unwrap()
}
fn system(c: &IndexedCoefficientContext, value: &IndexedCoefficient) -> BaseCoefficientSystem {
    let p = c
        .numerator_condition_with_limits(value, Default::default())
        .unwrap();
    c.base_coefficient_system(&p, Default::default(), Default::default())
        .unwrap()
}
fn positive(c: &IndexedCoefficientContext) -> IndexedCoefficient {
    c.add(
        &c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
        &c.one(),
    )
    .unwrap()
}

#[test]
fn native_factor_exclusion_false_callback_preserves_old_coupled_resolution() {
    let c = context();
    let s = system(&c, &c.mul(&positive(&c), &c.index(0).unwrap()).unwrap());
    let before = c
        .integer_zero_locus_domain_resolution(&s, Default::default(), |_, _| true)
        .unwrap();
    let mut calls = 0;
    let after = c
        .integer_zero_locus_domain_resolution_with_factor_exclusion(
            &s,
            Default::default(),
            |_, _| true,
            |factor| {
                calls += 1;
                assert!(!factor.is_zero());
                assert_eq!(index_support(factor, 1).len(), 2);
                Ok(false)
            },
        )
        .unwrap();
    assert_eq!(before, IntegerZeroLocusDomainResolution::UnsupportedCoupled);
    assert_eq!(after, before);
    assert_eq!(calls, 1);
}

#[test]
fn native_factor_exclusion_retains_remaining_exact_hyperplane_and_replay_budget() {
    let c = context();
    let x_minus_two = c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap();
    let s = system(&c, &c.mul(&positive(&c), &x_minus_two).unwrap());
    let positive_polynomial = c
        .numerator_condition_with_limits(&positive(&c), Default::default())
        .unwrap();
    let check = |factor: &crate::algebra::CoefficientPolynomial| {
        assert_eq!(factor, positive_polynomial.raw());
        // x,y >=1 makes this specific factor >=3; this is not a generic oracle.
        Ok(true)
    };
    let result = c
        .integer_zero_locus_domain_resolution_with_factor_exclusion(
            &s,
            Default::default(),
            |_, root| root >= &Integer::from(1),
            check,
        )
        .unwrap();
    let IntegerZeroLocusDomainResolution::IntersectsExactHyperplanes(roots) = result else {
        panic!("expected retained exact root");
    };
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].index_position(), 0);
    assert_eq!(roots[0].root(), &Integer::from(2));
    assert!(matches!(
        c.integer_zero_locus_domain_resolution_with_factor_exclusion(
            &s,
            IndexedGuardLimits {
                max_exact_hyperplane_replay_substitutions: 0,
                ..Default::default()
            },
            |_, root| root >= &Integer::from(1),
            check
        ),
        Err(IndexedAlgebraError::ResourceLimit { limit: 0, .. })
    ));
}

#[test]
fn native_factor_exclusion_mandatory_admission_precedes_callback_and_errors_propagate() {
    let c = context();
    let s = system(&c, &c.mul(&positive(&c), &positive(&c)).unwrap());
    let mut calls = 0;
    assert!(matches!(
        c.integer_zero_locus_domain_resolution_with_factor_exclusion(
            &s,
            IndexedGuardLimits {
                max_gcd_factor_work: 0,
                ..Default::default()
            },
            |_, _| true,
            |_| {
                calls += 1;
                Ok(true)
            }
        ),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard separable factor work",
            ..
        })
    ));
    assert_eq!(calls, 0);
    assert!(matches!(
        c.integer_zero_locus_domain_resolution_with_factor_exclusion(
            &s,
            IndexedGuardLimits {
                max_factor_terms: 0,
                ..Default::default()
            },
            |_, _| true,
            |_| {
                calls += 1;
                Ok(true)
            }
        ),
        Err(IndexedAlgebraError::ResourceLimit { limit: 0, .. })
    ));
    assert_eq!(calls, 0); // native prospective-output admission also stays strict
    assert!(
        matches!(c.integer_zero_locus_domain_resolution_with_factor_exclusion(
        &s, Default::default(), |_, _| true,
        |_| Err(IndexedAlgebraError::Symbolica("test callback failure".to_owned()))),
        Err(IndexedAlgebraError::Symbolica(message)) if message == "test callback failure")
    );
}

#[test]
fn native_factor_exclusion_zero_and_univariate_lanes_never_call_coupled_callback() {
    let c = context();
    for value in [c.zero(), c.one(), c.index(0).unwrap()] {
        let s = system(&c, &value);
        let before = c
            .integer_zero_locus_domain_resolution(&s, Default::default(), |_, _| true)
            .unwrap();
        let after = c
            .integer_zero_locus_domain_resolution_with_factor_exclusion(
                &s,
                Default::default(),
                |_, _| true,
                |_| panic!("not a coupled factor"),
            )
            .unwrap();
        assert_eq!(after, before);
    }
}
