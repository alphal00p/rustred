use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

fn domain(
    equations: &[&str],
    fixed: [Option<i16>; 3],
    sector: [bool; 3],
) -> AffineApplicationDomain {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations: Vec<_> = equations
        .iter()
        .map(|text| context.coefficient_fixture(text).numerator)
        .collect();
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::new(fixed).unwrap(),
        &equations,
        &[1, 2, 3],
        &sector,
    )
    .unwrap() else {
        panic!("expected coupled predicate")
    };
    AffineApplicationDomain::from_case(&case, &sector).unwrap()
}

#[test]
fn whole_excluded_face_does_not_remove_admitted_infinite_ray() {
    let domain = domain(&["a-b"], [None; 3], [false; 3]);
    let excluded = LatticeBox::try_new([0, 0, 1], [Some(0), Some(0), None]).unwrap();
    assert!(domain.is_proved_to_contain_box(&excluded));
    let admitted = LatticeBox::try_new([0, 1, 1], [Some(0), Some(1), None]).unwrap();
    assert!(!domain.is_proved_to_contain_box(&admitted));
    let overlapping = LatticeBox::try_new([0, 0, 1], [Some(0), None, None]).unwrap();
    assert!(!domain.is_proved_to_contain_box(&overlapping));
}

#[test]
fn complete_fixed_face_and_complete_conjunction_are_required() {
    let fixed = domain(&["a-b"], [None, None, Some(-1)], [false; 3]);
    assert!(!fixed.is_proved_to_contain_box(
        &LatticeBox::try_new([0, 0, 0], [Some(0), Some(0), None]).unwrap()
    ));
    assert!(
        !fixed.is_proved_to_contain_box(&LatticeBox::try_new([0, 0, 0], [Some(0); 3]).unwrap())
    );
    assert!(fixed.is_proved_to_contain_box(
        &LatticeBox::try_new([0, 0, 1], [Some(0), Some(0), Some(1)]).unwrap()
    ));
    let conjunction = domain(&["a-b", "b-c"], [None; 3], [false; 3]);
    assert!(!conjunction.is_proved_to_contain_box(
        &LatticeBox::try_new([0, 0, 0], [Some(0), Some(0), None]).unwrap()
    ));
    assert!(
        conjunction.is_proved_to_contain_box(&LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap())
    );
}

#[test]
fn exact_native_integer_endpoints_do_not_narrow() {
    let point = LatticeBox::try_new(
        [u64::MAX, u64::MAX, 0],
        [Some(u64::MAX), Some(u64::MAX), None],
    )
    .unwrap();
    for sector in [[true; 3], [false; 3]] {
        let domain = domain(&["a-b"], [None; 3], sector);
        assert!(domain.is_proved_to_contain_box(&point));
    }
    let mixed = domain(&["a+b-1"], [None; 3], [true, false, false]);
    assert!(mixed.is_proved_to_contain_box(&point));
    let neighbor = domain(&["a+b"], [None; 3], [true, false, false]);
    assert!(!neighbor.is_proved_to_contain_box(&point));
}

#[test]
fn unsupported_maps_equations_and_cached_metadata_cannot_supply_authority() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let point = LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap();
    let base = domain(&["a-b"], [None; 3], [false; 3]);
    let mut changed = base.clone();
    changed.primitive_matrix = None;
    changed.integral_chart = None;
    assert!(changed.is_proved_to_contain_box(&point));
    for text in ["a*b", "d*a", "1+a-b"] {
        changed.equations = vec![context.coefficient_fixture(text).numerator].into();
        assert!(!changed.is_proved_to_contain_box(&point));
    }
    changed = base.clone();
    changed.indices = vec![1, 1, 3].into();
    assert!(!changed.is_proved_to_contain_box(&point));
    changed = base.clone();
    changed.indices = vec![1, 2, 99].into();
    assert!(!changed.is_proved_to_contain_box(&point));
    changed = base;
    let foreign = CoefficientContext::new(["q", "x", "y", "z"]);
    changed.equations = vec![
        context.coefficient_fixture("a-b").numerator,
        foreign.coefficient_fixture("x-y").numerator,
    ]
    .into();
    assert!(!changed.is_proved_to_contain_box(&point));
}

#[test]
fn containment_work_and_shape_fail_closed() {
    let mut domain = domain(&["a-b"], [None; 3], [false; 3]);
    let point = LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap();
    domain.equations = vec![domain.equations[0].clone(); MAX_MATRIX_CELLS / 4 + 1].into();
    assert!(!domain.is_proved_to_contain_box(&point));
    domain.equations = Box::new([]);
    assert!(!domain.is_proved_to_contain_box(&point));
    let other = LatticeBox::try_new([0; 2], [Some(0); 2]).unwrap();
    assert!(!domain.is_proved_to_contain_box(&other));
}
