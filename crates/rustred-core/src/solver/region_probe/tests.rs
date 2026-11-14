use symbolica::domains::RingOps;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::prelude::{Integer, Ring};

use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::{region_point, vanishes_at};

fn polys(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|e| context.coefficient_fixture(e).numerator)
        .collect()
}

fn base(field: &Zp64, n: usize) -> Vec<symbolica::domains::finite_field::FiniteFieldElement<u64>> {
    (0..n)
        .map(|i| field.to_element(1000 + 17 * i as u64))
        .collect()
}

const P: u64 = (1 << 61) - 1;

/// T266 prefix region R05 on its chart (n0, n1 solved by the chart equations): one quadric in n2, n3, n11, n12
/// (graph type: an infinite non-affine integer family). The point must lie on it and leave `d` alone.
#[test]
fn point_on_the_t266_r05_quadric_and_d_untouched() {
    let context = CoefficientContext::new(["d", "n2", "n3", "n11", "n12"]);
    let system = polys(
        &context,
        &["2*n2^2-10*n2*n3+8*n3^2-7*n2*n11+7*n3*n11-4*n2*n12+4*n3*n12+7*n2-n3+3*n11+4*n12-8"],
    );
    let field = Zp64::new(P);
    let b = base(&field, 5);
    for seed in 0..8 {
        let point = region_point(&system, &b, &field, seed).expect("a Zp point of a quadric");
        assert!(vanishes_at(&system, &point, &field));
        assert_eq!(point[0], b[0], "d is not constrained by the region");
    }
    // a polynomial that is not in the ideal does not vanish at the point
    let other = polys(&context, &["n2-n3"]);
    let point = region_point(&system, &b, &field, 1).unwrap();
    assert!(!vanishes_at(&other, &point, &field));
}

/// T266 prefix region R12, cubic scroll component (codim 2, three quadric generators; Singular minAssGTZ).
#[test]
fn point_on_a_codim_two_cubic_scroll() {
    let context = CoefficientContext::new(["d", "n0", "n1", "n2", "n3", "n9", "n13"]);
    let system = polys(
        &context,
        &[
            "5*n1^2-2*n0*n2+14*n1*n2-6*n2^2+8*n0*n3-36*n1*n3+18*n2*n3-n3^2+4*n0*n9-8*n1*n9+12*n2*n9-8*n3*n9-2*n0*n13+14*n1*n13+14*n2*n13-26*n3*n13-2*n0-36*n1-48*n2+62*n3-16*n9-22*n13+83",
            "5*n0*n1-8*n0*n2+16*n1*n2-4*n2^2+27*n0*n3-39*n1*n3+12*n2*n3-9*n3^2+16*n0*n9-12*n1*n9+8*n2*n9-12*n3*n9+2*n0*n13+6*n1*n13+16*n2*n13-24*n3*n13-33*n0-9*n1-42*n2+48*n3-24*n9-18*n13+87",
            "5*n0^2-57*n0*n2+54*n1*n2+4*n2^2+173*n0*n3-126*n1*n3+13*n2*n3-66*n3^2+84*n0*n9-48*n1*n9+12*n2*n9-48*n3*n9+18*n0*n13-6*n1*n13+54*n2*n13-66*n3*n13-167*n0+69*n1-103*n2+57*n3-96*n9-42*n13+243",
        ],
    );
    let field = Zp64::new(P);
    let b = base(&field, 7);
    let point = region_point(&system, &b, &field, 3).expect("a Zp point of the scroll");
    assert!(vanishes_at(&system, &point, &field));
}

/// Chart constraints are part of the system: a fixed slot and an affine equation hold at the point.
#[test]
fn chart_constraints_hold_at_the_point() {
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    let system = polys(&context, &["x*y-z^2-3", "x-2", "y-z-1"]);
    let field = Zp64::new(P);
    let b = base(&field, 4);
    let point = region_point(&system, &b, &field, 0).expect("x = 2, y = z + 1, z^2 - 2z + 1 = 0");
    assert_eq!(field.from_element(&point[1]), 2);
    // z = 1 (double root), y = 2
    assert_eq!(field.from_element(&point[3]), 1);
    assert_eq!(field.from_element(&point[2]), 2);
}

/// An inconsistent system has no point (and the caller keeps its random point).
#[test]
fn empty_system_has_no_point() {
    let context = CoefficientContext::new(["d", "x", "y"]);
    let field = Zp64::new(P);
    let b = base(&field, 3);
    assert!(region_point(&polys(&context, &["x-y", "x-y-1"]), &b, &field, 0).is_none());
    // x^2 + 1 has no root mod 2^61-1 (= 3 mod 4): only points over an extension
    assert!(region_point(&polys(&context, &["x^2+1"]), &b, &field, 0).is_none());
    let _ = Integer::one();
    assert!(field.is_zero(&field.zero()));
}
