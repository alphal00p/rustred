use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

fn domain(equations: &[&str], sector: [bool; 3]) -> AffineApplicationDomain {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = equations
        .iter()
        .map(|e| context.coefficient_fixture(e).numerator)
        .collect::<Vec<_>>();
    let AffineIntersection::Affine(case) =
        AffineCase::from_coordinate(&CoordinateCase::generic(), &equations, &[1, 2, 3], &sector)
            .unwrap()
    else {
        panic!("expected coupled equations");
    };
    AffineApplicationDomain::from_case(&case, &sector).unwrap()
}

#[test]
fn fixed_face_information_propagates_between_exact_equations() {
    let first = domain(&["a-3*c-3", "b-c-1"], [false; 3]);
    let bad = LatticeBox::try_new([1, 0, 0], [Some(2), Some(0), None]).unwrap();
    assert!(first.is_proved_empty_in_box(&bad));
    // a=-3,b=-1,c=-2 is an admitted integer point.
    let feasible = LatticeBox::try_new([3, 1, 2], [Some(3), Some(1), Some(2)]).unwrap();
    assert!(!first.is_proved_empty_in_box(&feasible));
    let second = domain(&["2*a-b-1", "c+b+1"], [false; 3]);
    let bad = LatticeBox::try_new([1, 0, 0], [None, None, Some(0)]).unwrap();
    assert!(second.is_proved_empty_in_box(&bad));
    let feasible = LatticeBox::try_new([0, 1, 0], [Some(0), Some(1), Some(0)]).unwrap();
    assert!(!second.is_proved_empty_in_box(&feasible));
}

#[test]
fn singleton_values_never_narrow_to_compact_or_machine_signed_powers() {
    for sector in [[false; 3], [true; 3]] {
        let domain = domain(&["a-b-c", "b-c"], sector);
        // The fixed b power is -u64::MAX, or u64::MAX+1 in the
        // positive sector. The equations force a=2*b, outside a's box.
        let bad =
            LatticeBox::try_new([0, u64::MAX, 0], [Some(u64::MAX), Some(u64::MAX), None]).unwrap();
        assert!(domain.is_proved_empty_in_box(&bad));
        // Dropping a's finite bound restores a valid unbounded domain.
        let feasible = LatticeBox::try_new([0, u64::MAX, 0], [None, Some(u64::MAX), None]).unwrap();
        assert!(!domain.is_proved_empty_in_box(&feasible));
    }
}

#[test]
fn malformed_or_unsupported_domains_cannot_gain_authority_after_specialization() {
    let mut original = domain(&["a-3*c-3", "b-c-1"], [false; 3]);
    let bad = LatticeBox::try_new([1, 0, 0], [Some(2), Some(0), None]).unwrap();
    let saved = original.indices.clone();
    original.indices = vec![1, 1, 3].into();
    assert!(!original.is_proved_empty_in_box(&bad));
    original.indices = vec![1, 2, usize::MAX].into();
    assert!(!original.is_proved_empty_in_box(&bad));
    original.indices = saved;
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    for unsupported in ["1+b*c", "1+d*b"] {
        original.equations = vec![context.coefficient_fixture(unsupported).numerator].into();
        // b=0 would make either equation a nonzero constant, but neither
        // original equation belongs to the admitted affine index service.
        assert!(!original.is_proved_empty_in_box(&bad));
    }
    original.equations = vec![context.coefficient_fixture("a-3*c-3").numerator].into();
    let foreign = CoefficientContext::new(["q", "x", "y", "z"]);
    original.equations = vec![
        original.equations[0].clone(),
        foreign.coefficient_fixture("y-z-1").numerator,
    ]
    .into();
    assert!(!original.is_proved_empty_in_box(&bad));
}

#[test]
fn native_specialization_ignores_cached_rows_and_respects_work_limits() {
    let mut domain = domain(&["a-3*c-3", "b-c-1"], [false; 3]);
    let bad = LatticeBox::try_new([1, 0, 0], [Some(2), Some(0), None]).unwrap();
    domain.primitive_matrix = None;
    domain.integral_chart = None;
    assert!(domain.is_proved_empty_in_box(&bad));
    domain.equations = vec![domain.equations[0].clone(); MAX_MATRIX_CELLS].into();
    assert!(!domain.is_proved_empty_in_box(&bad));
}
