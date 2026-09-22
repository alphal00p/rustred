use super::*;
use crate::algebra::indexed::base_coefficients::IntegerZeroLocusDomainResolution;
use crate::algebra::{CoefficientContext, IndexedCoefficientContext, IndexedGuardLimits};
use symbolica::prelude::Integer;

fn context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["a", "b"]),
        "guard-cost-ordering",
        2,
    )
    .unwrap()
}
fn fixture(c: &IndexedCoefficientContext) -> BaseCoefficientSystem {
    let x = c.index(0).unwrap();
    let y = c.index(1).unwrap();
    let cheap = c
        .mul(&c.sub(&x, &c.one()).unwrap(), &c.sub(&y, &c.one()).unwrap())
        .unwrap();
    let mut costly = c.one();
    let sum = c.add(&c.add(&x, &y).unwrap(), &c.one()).unwrap();
    for _ in 0..8 {
        costly = c.mul(&costly, &sum).unwrap();
    }
    let a = c.lift(&c.base().parameter("a").unwrap()).unwrap();
    let p = c
        .numerator_condition_with_limits(
            &c.add(&costly, &c.mul(&a, &cheap).unwrap()).unwrap(),
            Default::default(),
        )
        .unwrap();
    c.base_coefficient_system(&p, Default::default(), Default::default())
        .unwrap()
}

#[test]
fn guard_cost_ordering_borrows_without_changing_canonical_storage() {
    let c = context();
    let system = fixture(&c);
    let sorted: Vec<_> = inexpensive_first(&system, 2).unwrap().collect();
    assert_eq!(system.equations[0].base_monomial.as_ref(), &[0, 0]);
    assert_eq!(system.equations[1].base_monomial.as_ref(), &[1, 0]);
    assert!(std::ptr::eq(sorted[0], &system.equations[1]));
    assert!(std::ptr::eq(sorted[1], &system.equations[0]));
}

#[test]
fn guard_cost_ordering_later_cheap_native_equation_avoids_unneeded_factor_refusal() {
    let c = context();
    let system = fixture(&c);
    let limits = IndexedGuardLimits {
        max_gcd_factor_work: 5000,
        ..Default::default()
    };
    assert!(
        super::super::separable_factor_work(system.equations[0].index_polynomial.raw(), 2, limits)
            .unwrap()
            > 5000
    );
    assert_eq!(
        c.integer_zero_locus_domain_resolution(&system, limits, |_, root| root
            >= &Integer::from(2))
            .unwrap(),
        IntegerZeroLocusDomainResolution::MissesDomain
    );
}

#[test]
fn guard_cost_ordering_keeps_actual_admission_failure() {
    let c = context();
    let system = fixture(&c);
    assert!(matches!(
        c.integer_zero_locus_domain_resolution(
            &system,
            IndexedGuardLimits {
                max_gcd_factor_work: 0,
                ..Default::default()
            },
            |_, _| true
        ),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard separable factor work",
            limit: 0,
            ..
        })
    ));
}

#[test]
fn guard_cost_ordering_ties_follow_original_ordinal() {
    let c = context();
    let a = c.lift(&c.base().parameter("a").unwrap()).unwrap();
    let b = c.lift(&c.base().parameter("b").unwrap()).unwrap();
    let p = c
        .numerator_condition_with_limits(
            &c.add(
                &c.mul(&a, &c.index(0).unwrap()).unwrap(),
                &c.mul(&b, &c.index(1).unwrap()).unwrap(),
            )
            .unwrap(),
            Default::default(),
        )
        .unwrap();
    let system = c
        .base_coefficient_system(&p, Default::default(), Default::default())
        .unwrap();
    let sorted: Vec<_> = inexpensive_first(&system, 2).unwrap().collect();
    assert_eq!(sorted.len(), 2);
    assert!(std::ptr::eq(sorted[0], &system.equations[0]));
    assert!(std::ptr::eq(sorted[1], &system.equations[1]));
}
