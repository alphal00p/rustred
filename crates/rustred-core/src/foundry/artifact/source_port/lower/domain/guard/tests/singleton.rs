use super::conjunction::{exact_domain, expression};
use super::*;

// Captured H229 guard. Only test data knows the topology; the proof retains
// an ordinary fixed face before native factorization and affine restriction.
const CAPTURED: &str = "-24-86*n4-116*n4^2-72*n4^3-20*n4^4-2*n4^5-30*n3-82*n3*n4-78*n3*n4^2-30*n3*n4^3-4*n3*n4^4+36*n3^2+48*n3^2*n4+12*n3^2*n4^2-57*n1*n3-79*n1*n3*n4-23*n1*n3*n4^2-n1*n3*n4^3+18*n1*n3^2+24*n1*n3^2*n4+6*n1*n3^2*n4^2+15*n1^2+17*n1^2*n4+n1^2*n4^2-n1^2*n4^3-27*n1^2*n3-36*n1^2*n3*n4-9*n1^2*n3*n4^2+9*n1^3+12*n1^3*n4+3*n1^3*n4^2+17*d+56*d*n4+66*d*n4^2+32*d*n4^3+5*d*n4^4+19*d*n3+45*d*n3*n4+33*d*n3*n4^2+7*d*n3*n4^3-30*d*n3^2-36*d*n3^2*n4-6*d*n3^2*n4^2+46*d*n1*n3+56*d*n1*n3*n4+10*d*n1*n3*n4^2-6*d*n1*n3^2-6*d*n1*n3^2*n4-14*d*n1^2-16*d*n1^2*n4-2*d*n1^2*n4^2+9*d*n1^2*n3+9*d*n1^2*n3*n4-3*d*n1^3-3*d*n1^3*n4-3*d^2-9*d^2*n4-9*d^2*n4^2-3*d^2*n4^3-3*d^2*n3-6*d^2*n3*n4-3*d^2*n3*n4^2+6*d^2*n3^2+6*d^2*n3^2*n4-9*d^2*n1*n3-9*d^2*n1*n3*n4+3*d^2*n1^2+3*d^2*n1^2*n4";

#[test]
fn captured_guard_keeps_singleton_through_all_conjunction_charts() {
    let context = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "singleton-conjunction-capture",
        10,
    )
    .unwrap();
    for order in [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        [0, 4, 2, 1, 3, 5, 6, 7, 8, 9],
    ] {
        let mut source = CAPTURED.to_owned();
        for axis in 0..10 {
            source = source.replace(&format!("n{axis}"), &format!("{{{axis}}}"));
        }
        for (axis, replacement) in order.into_iter().enumerate() {
            source = source.replace(&format!("{{{axis}}}"), &format!("n{replacement}"));
        }
        let guard = polynomial(&context, &expression(&context, &source));
        let mut sector = [false; 10];
        for axis in [0, 2, 5, 6, 7] {
            sector[order[axis]] = true;
        }
        let mut lower = [0; 10];
        lower[order[3]] = 1;
        lower[order[4]] = 2;
        let mut upper = [Some(0); 10];
        upper[order[3]] = None;
        upper[order[4]] = None;
        let piece = LatticeBox::try_new(lower, upper).unwrap();
        // Default limits: the former 83,951-bit prospective chart must not
        // be admitted by raising the unchanged 65,536-bit allowance.
        validate_guard_on_domain_with_limits(
            &context,
            &guard,
            &piece,
            &sector,
            None,
            &[],
            Default::default(),
        )
        .unwrap();
        lower[order[4]] = 1;
        let widened = LatticeBox::try_new(lower, upper).unwrap();
        assert!(
            validate_guard_on_domain_with_limits(
                &context,
                &guard,
                &widened,
                &sector,
                None,
                &[],
                Default::default(),
            )
            .is_err()
        );
        let zero = context
            .specialize_fixed_polynomial(
                &guard,
                &[(order[1], 0), (order[3], -1), (order[4], -1)],
                Default::default(),
            )
            .unwrap();
        assert!(zero.is_zero(), "the widened-domain counterexample is exact");
    }
}

#[test]
fn common_singleton_restriction_keeps_whole_exclusion_semantics() {
    let context = context();
    let guard = expression(&context, "(n0+1)*(n0+d*(n0+n1-n2))");
    let piece = LatticeBox::try_new([0; 3], [Some(0), None, None]).unwrap();
    let excluded = exact_domain(&context, &[false; 3], [None; 3], &["n0+n1-n2"]);
    check(
        &context,
        &guard,
        &piece,
        None,
        &[excluded],
        Default::default(),
    )
    .unwrap();
    assert!(check(&context, &guard, &piece, None, &[], Default::default()).is_err());
    let incomplete = exact_domain(&context, &[false; 3], [None; 3], &["n0+n1-n2", "n1+1"]);
    assert!(
        check(
            &context,
            &guard,
            &piece,
            None,
            &[incomplete],
            Default::default()
        )
        .is_err()
    );
    // Widening the fixed face must not preserve a face-only implication.
    let broad = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let face_only = exact_domain(&context, &[false; 3], [Some(0), None, None], &["n1-n2"]);
    assert!(
        check(
            &context,
            &guard,
            &broad,
            None,
            &[face_only],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn retained_singleton_uses_i64_not_compact_i16_chart_values() {
    let context = context();
    let guard = polynomial(&context, &expression(&context, "n0+40000"));
    let piece = LatticeBox::try_new([40000, 0, 0], [Some(40000), None, None]).unwrap();
    let restricted = super::super::singleton::restrict(
        &context,
        &guard,
        &piece,
        &[false; 3],
        Default::default(),
        &mut Work::default(),
    )
    .unwrap()
    .unwrap();
    assert!(restricted.is_zero());
    assert!(!guard.is_zero());
}
