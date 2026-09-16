use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

fn domain(equations: &[&str], sector: [bool; 3]) -> AffineApplicationDomain {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = equations
        .iter()
        .map(|q| context.coefficient_fixture(q).numerator)
        .collect::<Vec<_>>();
    let AffineIntersection::Affine(case) =
        AffineCase::from_coordinate(&CoordinateCase::generic(), &equations, &[1, 2, 3], &sector)
            .unwrap()
    else {
        panic!("expected coupled fixture");
    };
    AffineApplicationDomain::from_case(&case, &sector).unwrap()
}

#[test]
fn exhausting_width_two_faces_proves_the_observed_joint_contradictions() {
    let first = domain(&["a-3*c-3", "b-c-1"], [false; 3]);
    // a=3*b cannot lie in[-2,-1] while b ranges over[-1,0].
    let bad = LatticeBox::try_new([1, 0, 0], [Some(2), Some(1), None]).unwrap();
    assert!(first.is_proved_empty_in_box(&bad));
    // The neighboring a=-3,b=-1,c=-2 point must remain admitted.
    let neighbor = LatticeBox::try_new([1, 0, 0], [Some(3), Some(1), None]).unwrap();
    assert!(!first.is_proved_empty_in_box(&neighbor));
    let second = domain(&["2*a-b-1", "c+b+1"], [false; 3]);
    // c in[-1,0] and 2*a=-c contradict a<=-1.
    let bad = LatticeBox::try_new([1, 0, 0], [None, None, Some(1)]).unwrap();
    assert!(second.is_proved_empty_in_box(&bad));
    let neighbor = LatticeBox::try_new([0, 0, 0], [None, None, Some(1)]).unwrap();
    assert!(!second.is_proved_empty_in_box(&neighbor));
}

#[test]
fn infeasible_endpoints_do_not_replace_exhaustion_of_the_interior() {
    let domain = domain(&["2*a-b"], [false; 3]);
    let interval = LatticeBox::try_new([0, 1, 0], [None, Some(3), None]).unwrap();
    // b=-1 and -3 are impossible, but b=-2,a=-1 is feasible.
    assert!(!domain.is_proved_empty_in_box(&interval));
    for b in [1, 3] {
        assert!(domain.is_proved_empty_in_box(
            &LatticeBox::try_new([0, b, 0], [None, Some(b), None]).unwrap()
        ));
    }
    let middle = LatticeBox::try_new([1, 2, 0], [Some(1), Some(2), None]).unwrap();
    assert!(!domain.is_proved_empty_in_box(&middle));
}

#[test]
fn short_intervals_at_large_exact_endpoints_do_not_narrow() {
    for sector in [[false; 3], [true; 3]] {
        let domain = domain(&["a-b-c", "b-c"], sector);
        let b_lower = if sector[1] {
            u64::MAX - 2
        } else {
            u64::MAX - 1
        };
        let b_upper = b_lower + 1;
        let bad =
            LatticeBox::try_new([0, b_lower, 0], [Some(u64::MAX), Some(b_upper), None]).unwrap();
        assert!(domain.is_proved_empty_in_box(&bad));
        let feasible = LatticeBox::try_new([0, b_lower, 0], [None, Some(b_upper), None]).unwrap();
        assert!(!domain.is_proved_empty_in_box(&feasible));
    }
}

#[test]
fn bounded_refinement_caps_total_matrix_work_and_never_recurses() {
    let mut oversized = domain(&["a-3*c-3", "b-c-1"], [false; 3]);
    let bad = LatticeBox::try_new([1, 0, 0], [Some(2), Some(1), None]).unwrap();
    let equations = oversized.equations.clone();
    oversized.equations = (0..9000).map(|i| equations[i % 2].clone()).collect();
    // Each initial matrix fits, but two such face reductions exceed the
    // aggregate work cap. This is inconclusive despite mathematical emptiness.
    assert!(!oversized.is_proved_empty_in_box(&bad));
    let domain = domain(&["2*a-b"], [false; 3]);
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let rows =
        vec![linear_row(&context.coefficient_fixture("2*a-b").numerator, &[1, 2, 3]).unwrap()];
    let intervals = vec![
        (None, Some(Integer::zero())),
        (Some(Integer::from(-10)), Some(Integer::from(-1))),
        (Some(Integer::from(-1)), Some(Integer::zero())),
    ];
    // c has a short interval but occurs in no equation; b exceeds the face
    // cap. No irrelevant split or recursive alternative is attempted.
    assert!(!domain.bounded_face_refinement_proves_empty(&rows, &intervals));
}
